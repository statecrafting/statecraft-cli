//! Spec 042's acceptance through the real `run` path and the real
//! confinement, with a fake provider and no network: the per-run
//! configuration directory (3.1), the provisioned token (3.2), and the
//! withdrawn grants on the operator's provider configuration (rule 12's first
//! open item). The child is hostile: it tries to write the operator's
//! `~/.claude/settings.json` and `~/.claude.json`, which an unconfined
//! session would later load.

#![cfg(unix)]

#[path = "support/json_naming.rs"]
mod json_naming;

#[path = "support/provider_token.rs"]
mod provider_token;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn executable(path: &Path, script: &str) {
    statecraft_adapter::fixture::install_script(path, script, 0o755).unwrap();
}

fn git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(path)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

struct Fixture {
    target: tempfile::TempDir,
    home: tempfile::TempDir,
    bin: tempfile::TempDir,
    operator: tempfile::TempDir,
}

const SETTINGS: &str = "{\"operator\": \"settings\"}\n";
const CONFIG: &str = "{\"operator\": \"config\"}\n";

fn fixture() -> Fixture {
    let f = Fixture {
        target: tempfile::tempdir().unwrap(),
        home: tempfile::tempdir().unwrap(),
        bin: tempfile::tempdir().unwrap(),
        operator: tempfile::tempdir().unwrap(),
    };
    std::fs::create_dir_all(f.operator.path().join(".claude")).unwrap();
    std::fs::write(f.operator.path().join(".claude/settings.json"), SETTINGS).unwrap();
    std::fs::write(f.operator.path().join(".claude.json"), CONFIG).unwrap();
    git(f.target.path(), &["init", "--quiet"]);
    git(f.target.path(), &["config", "user.name", "fixture"]);
    git(
        f.target.path(),
        &["config", "user.email", "fixture@example.com"],
    );
    std::fs::write(f.target.path().join("source.txt"), "base").unwrap();
    git(f.target.path(), &["add", "source.txt"]);
    git(
        f.target.path(),
        &["commit", "--quiet", "-m", "fixture base"],
    );
    executable(
        &f.bin.path().join("spec-spine"),
        r#"#!/bin/sh
case "$*" in
  --version) echo 'spec-spine 0.20.0' ;;
  check|'check --help') exit 0 ;;
  'registry plan --json') echo '{"ready":[{"id":"replay","title":"recorded stream"}]}' ;;
  'registry list --json') echo '{"items":[{"id":"replay","status":"approved","implementation":"pending"}]}' ;;
  'verify '*' --plan --json') printf '{"exitCode":0,"ok":true,"report":{"commands":[],"skipped":[],"specId":"%s"},"schemaVersion":"0.6.0","verb":"verify"}' $2 ;;
  *) exit 3 ;;
esac
"#,
    );
    let stream = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../statecraft-adapter-claude-code/testdata/stream/success.jsonl");
    std::fs::copy(stream, f.bin.path().join("native.jsonl")).unwrap();
    // The hostile child: it reports what it was given, writes inside its
    // per-run directory, and tries both of the operator's files. No special
    // builtin carries a refused redirection: dash exits on one.
    let operator = f.operator.path().display().to_string();
    executable(
        &f.bin.path().join("claude"),
        &format!(
            r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.267'; exit 0; fi
/bin/cat > /dev/null
printf '%s' "$CLAUDE_CONFIG_DIR" > child-config-dir
if [ "$CLAUDE_CODE_OAUTH_TOKEN" = {token} ]; then echo yes > child-token-matches; fi
echo provider-state > "$CLAUDE_CONFIG_DIR/.claude.json" && echo yes > child-wrote-own-config
echo '{{"hooks": "attacker"}}' > {operator}/.claude/settings.json 2>/dev/null && echo yes > child-wrote-settings
echo '{{"attacker": true}}' > {operator}/.claude.json 2>/dev/null && echo yes > child-wrote-config
/usr/bin/touch {operator}/.claude/new-file 2>/dev/null && echo yes > child-created-file
/bin/cat "$(dirname "$0")/native.jsonl"
"#,
            token = provider_token::FIXTURE_TOKEN,
        ),
    );
    f
}

impl Fixture {
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", self.home.path())
            .env("HOME", self.operator.path())
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.bin.path().display()),
            )
            .env("USER", "fixture-operator")
            .output()
            .unwrap()
    }

    fn arm(&self) {
        let root = self.target.path().to_str().unwrap();
        let registered = self.run(&["project", "register", root]);
        assert!(registered.status.code().unwrap() <= 1, "{registered:?}");
        let armed = self.run(&["project", "arm", root]);
        assert_eq!(armed.status.code(), Some(0), "{armed:?}");
    }

    fn operator_unchanged(&self) {
        let o = self.operator.path();
        assert_eq!(
            std::fs::read_to_string(o.join(".claude/settings.json")).unwrap(),
            SETTINGS
        );
        assert_eq!(
            std::fs::read_to_string(o.join(".claude.json")).unwrap(),
            CONFIG
        );
        assert!(!o.join(".claude/new-file").exists());
    }
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        match std::fs::symlink_metadata(&p) {
            Ok(m) if m.is_dir() => files_under(&p, out),
            Ok(m) if m.is_file() => out.push(p),
            _ => {}
        }
    }
}

#[test]
fn a_confined_child_gets_its_own_configuration_and_never_the_operators() {
    let f = fixture();
    provider_token::with_token(f.home.path());
    f.arm();
    let root = f.target.path().to_str().unwrap();
    let output = f.run(&["run", root, "replay", "--json"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let answer: serde_json::Value = json_naming::from_output(&output.stdout).unwrap();
    let workspace = PathBuf::from(answer["report"]["workspaceRetained"].as_str().unwrap());
    let read = |name: &str| std::fs::read_to_string(workspace.join(name)).ok();

    // 3.1: a per-run directory, inside the attempt, the child could write.
    let config_dir = PathBuf::from(read("child-config-dir").expect("the child ran"));
    assert!(config_dir.is_absolute());
    assert!(
        config_dir.starts_with(f.home.path().canonicalize().unwrap())
            || config_dir.starts_with(f.home.path()),
        "{}",
        config_dir.display()
    );
    assert!(config_dir.ends_with("provider-config"));
    assert_eq!(read("child-wrote-own-config").as_deref(), Some("yes\n"));
    // Discarded after the process tree exits.
    assert!(
        !config_dir.exists(),
        "{} survived the attempt",
        config_dir.display()
    );

    // 3.2: the provisioned token arrived in the environment.
    assert_eq!(read("child-token-matches").as_deref(), Some("yes\n"));

    // Rule 12: the operator's provider configuration was not writable.
    assert!(read("child-wrote-settings").is_none());
    assert!(read("child-wrote-config").is_none());
    assert!(read("child-created-file").is_none());
    f.operator_unchanged();

    // No output, record or remaining file carries the token, except the
    // owner-only file it is held in.
    let token = provider_token::FIXTURE_TOKEN;
    assert!(!String::from_utf8_lossy(&output.stdout).contains(token));
    assert!(!String::from_utf8_lossy(&output.stderr).contains(token));
    let mut files = Vec::new();
    files_under(f.home.path(), &mut files);
    files_under(&workspace, &mut files);
    let held = f.home.path().join("credentials/claude-code-token");
    for file in files.iter().filter(|p| **p != held) {
        let bytes = std::fs::read(file).unwrap_or_default();
        assert!(
            !String::from_utf8_lossy(&bytes).contains(token),
            "{} carries the token",
            file.display()
        );
    }
    let show = f.run(&["run", "show", root, "--json"]);
    assert!(!String::from_utf8_lossy(&show.stdout).contains(token));
}

#[test]
fn a_launch_without_a_provisioned_token_refuses_and_starts_nothing() {
    let f = fixture();
    f.arm();
    let root = f.target.path().to_str().unwrap();
    let output = f.run(&["run", root, "replay", "--json"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let text = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(text.contains("provider token set"), "{text}");
    f.operator_unchanged();
}

#[test]
fn the_token_verbs_store_once_show_presence_and_never_print_it() {
    let f = fixture();
    let set = |input: &str| {
        use std::io::Write;
        let mut child = Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(["provider", "token", "set", "--json"])
            .env_clear()
            .env("STATECRAFT_HOME", f.home.path())
            .env("HOME", f.operator.path())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    };
    let shown = f.run(&["provider", "token", "show", "--json"]);
    assert_eq!(shown.status.code(), Some(0));
    let v: serde_json::Value = json_naming::from_output(&shown.stdout).unwrap();
    assert_eq!(v["report"]["stored"], false);
    let stored = set("secret-from-stdin-0042\n");
    assert_eq!(stored.status.code(), Some(0), "{stored:?}");
    assert!(!String::from_utf8_lossy(&stored.stdout).contains("secret-from-stdin-0042"));
    let again = set("another-0042");
    assert_eq!(again.status.code(), Some(2), "{again:?}");
    let shown = f.run(&["provider", "token", "show", "--json"]);
    let v: serde_json::Value = json_naming::from_output(&shown.stdout).unwrap();
    assert_eq!(v["report"]["stored"], true);
    assert!(!String::from_utf8_lossy(&shown.stdout).contains("secret-from-stdin-0042"));
    let held = f.home.path().join("credentials/claude-code-token");
    assert_eq!(
        std::fs::read_to_string(&held).unwrap(),
        "secret-from-stdin-0042"
    );
    let cleared = f.run(&["provider", "token", "clear", "--json"]);
    assert_eq!(cleared.status.code(), Some(0));
    assert!(!held.exists());
    // A token is never an argument.
    let operand = f.run(&["provider", "token", "set", "secret-as-argument"]);
    assert_eq!(operand.status.code(), Some(3));
}
