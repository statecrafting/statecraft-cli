//! Spec 052 section 3.5 through the binary: `provider capture` runs every
//! scenario against a **fake** `claude` on `PATH`, which prints canned stream
//! lines carrying what a redaction must remove, and the captures it writes
//! are redacted and load as one version's capture set. No real provider runs,
//! and nothing here opens a connection.

#[path = "support/json_naming.rs"]
mod json_naming;

use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const VERSION: &str = "2.1.400";

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_statecraft-cli"))
}

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../statecraft-adapter-claude-code/testdata/stream")
}

/// A fake `claude`. It records its arguments, working directory, environment
/// and prompt under `$FAKE_LOG/<n>`, then prints a stream shaped by its
/// flags: a deny rule in `--settings` gives a denial, `--max-turns` a capped
/// turn, `--disallowedTools Bash` a tool set without `Bash`, and an
/// environment without `HOME` the unauthenticated `api_error`. Every stream
/// carries a thinking-tokens event, a rate-limit event, a thinking-only
/// assistant event and the operator's paths and names.
const FAKE: &str = r#"#!/bin/sh
log="${FAKE_LOG:-/nonexistent}"
[ -n "${HOME:-}" ] || log="$(/bin/cat "$(dirname "$0")/log-path")"
n=$(/bin/ls "$log" | /usr/bin/wc -l | /usr/bin/tr -d ' ')
d="$log/$n"; /bin/mkdir -p "$d"
echo "$@" > "$d/args"; pwd > "$d/cwd"; /usr/bin/env > "$d/env"; /bin/ls -A > "$d/tree"
/bin/cat > "$d/prompt"
tools='"Task","Bash","Read","mcp__secret__x","Glob"'; subtype=success; err=false; reason=completed
denials='[]'; cost=0.01; settings=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --settings) settings="$2"; shift ;;
    --max-turns) subtype=error_max_turns; err=true; reason=max_turns ;;
    --disallowedTools) tools='"Task","Read","mcp__secret__x","Glob","WebFetch"' ;;
  esac
  shift
done
if [ -n "$settings" ] && /usr/bin/grep -q 'Bash(echo' "$settings"; then
  denials='[{"tool_name":"Bash","tool_use_id":"toolu_1","tool_input":{"command":"echo hello"}}]'
fi
if [ -z "${HOME:-}" ]; then err=true; reason=api_error; cost=0; fi
cat <<EOF
{"type":"system","subtype":"hook_started","hook_id":"real-hook","hook_name":"SessionStart:startup","hook_event":"SessionStart","uuid":"real-uuid-1","session_id":"real-session"}
{"type":"system","subtype":"init","cwd":"$PWD","session_id":"real-session","tools":[$tools],"mcp_servers":[{"name":"secret-server","status":"connected"}],"model":"claude-x","slash_commands":["private-cmd","other"],"terminal_slash_commands":["term"],"claude_code_version":"$VERSION","agents":["my-agent"],"skills":["my-skill"],"plugins":[{"name":"p","path":"/Users/operator/plugins/p"}],"uuid":"real-uuid-2","memory_paths":{"user":"/Users/operator/.claude/CLAUDE.md"},"messaging_socket_path":"/var/folders/operator/sock"}
{"type":"system","subtype":"thinking_tokens","tokens":40,"session_id":"real-session"}
{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"},"session_id":"real-session"}
{"type":"assistant","message":{"id":"msg_1","content":[{"type":"thinking","thinking":"","signature":"long-opaque"}]},"session_id":"real-session","uuid":"real-uuid-3","timestamp":"2026-11-02T10:11:12.131Z"}
{"type":"assistant","message":{"id":"msg_2","content":[{"type":"text","text":"OK"}]},"session_id":"real-session","uuid":"real-uuid-4","timestamp":"2026-11-02T10:11:13.000Z"}
{"type":"result","subtype":"$subtype","is_error":$err,"terminal_reason":"$reason","num_turns":1,"total_cost_usd":$cost,"permission_denials":$denials,"result":"OK","session_id":"real-session","uuid":"real-uuid-5"}
EOF
"#;

struct Fake {
    bin: tempfile::TempDir,
    log: tempfile::TempDir,
}

fn fake() -> Fake {
    let bin = tempfile::tempdir().unwrap();
    let log = tempfile::tempdir().unwrap();
    let script = bin.path().join("claude");
    fs::write(&script, FAKE.replace("$VERSION", VERSION)).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    // The constructed environment carries only PATH, so the fake finds its
    // log through a file beside itself.
    fs::write(
        bin.path().join("log-path"),
        log.path().display().to_string(),
    )
    .unwrap();
    Fake { bin, log }
}

fn path_with(fake: &Fake) -> String {
    format!("{}:/usr/bin:/bin", fake.bin.path().display())
}

fn capture(fake: &Fake, out: &Path) -> Output {
    Command::new(binary())
        .args(["provider", "capture", &out.display().to_string(), "--json"])
        .env("PATH", path_with(fake))
        .env("HOME", fake.log.path())
        .env("FAKE_LOG", fake.log.path())
        .env("OPERATOR_SECRET", "must-not-reach-api-error")
        .output()
        .unwrap()
}

fn report(out: &Output) -> Value {
    let v = json_naming::from_output(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not JSON ({e}): {}",
            String::from_utf8_lossy(&out.stdout)
        )
    });
    assert_eq!(v["verb"], "provider.capture", "{v}");
    v
}

/// The launch the fake recorded for a scenario, by the order the verb runs
/// them in (corpus.json's).
fn launch(fake: &Fake, index: usize, file: &str) -> String {
    fs::read_to_string(fake.log.path().join(index.to_string()).join(file)).unwrap()
}

#[test]
fn every_scenario_is_captured_redacted_and_comparable() {
    let fake = fake();
    let parent = tempfile::tempdir().unwrap();
    let out = parent.path().join("captures");
    let o = capture(&fake, &out);
    let v = report(&o);
    assert_eq!(o.status.code(), Some(0), "{v}");
    assert_eq!(v["report"]["scenarios"].as_array().unwrap().len(), 5);

    // The manifest has corpus.json's shape and loads as one version's set.
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(out.join("captures.json")).unwrap()).unwrap();
    let entries = manifest["captures"].as_array().unwrap();
    let names: Vec<&str> = entries
        .iter()
        .map(|e| e["scenario"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "success",
            "denied",
            "max-turns",
            "tool-removed",
            "api-error"
        ]
    );
    let golden: Value =
        serde_json::from_str(&fs::read_to_string(corpus().join("corpus.json")).unwrap()).unwrap();
    for (entry, golden) in entries.iter().zip(golden["captures"].as_array().unwrap()) {
        assert_eq!(entry["providerVersion"], VERSION);
        assert_eq!(entry["invocation"], golden["invocation"]);
        assert_eq!(entry["redactions"], golden["redactions"]);
        assert_eq!(entry["captured"].as_str().unwrap().len(), 10);
    }

    // Every capture is redacted.
    for entry in entries {
        let text = fs::read_to_string(out.join(entry["file"].as_str().unwrap())).unwrap();
        for leaked in [
            "real-session",
            "real-uuid",
            "real-hook",
            "operator",
            "statecraft-capture-",
            "secret",
            "my-agent",
            "my-skill",
            "private-cmd",
            "thinking",
            "rate_limit_event",
            "10:11:12",
        ] {
            assert!(!text.contains(leaked), "{leaked} in {entry}: {text}");
        }
        assert!(text.contains(r#""tools":["Task","#), "{text}");
        assert!(text.contains("/tmp/statecraft-probe"), "{text}");
    }

    // What each scenario asked of the provider.
    let args: Vec<String> = (0..5).map(|i| launch(&fake, i, "args")).collect();
    assert_eq!(
        args[0].trim(),
        "--print --output-format stream-json --verbose"
    );
    assert!(args[1].contains("--settings "), "{}", args[1]);
    assert!(args[2].trim().ends_with("--max-turns 1"), "{}", args[2]);
    assert!(
        args[3].trim().ends_with("--disallowedTools Bash"),
        "{}",
        args[3]
    );
    assert_eq!(args[4].trim(), args[0].trim());
    for i in 0..5 {
        // The prompt arrives on standard input and never on the command line.
        let prompt = launch(&fake, i, "prompt");
        assert!(!prompt.is_empty());
        assert!(!args[i].contains(prompt.trim()));
        // A scratch git work tree, holding the three files.
        let tree = launch(&fake, i, "tree");
        for name in [".git", "one.txt", "two.txt", "three.txt"] {
            assert!(tree.lines().any(|l| l == name), "{name} missing: {tree}");
        }
        assert!(launch(&fake, i, "cwd").contains("statecraft-capture-"));
    }
    // api-error runs with PATH alone; the others keep the operator's env.
    assert!(launch(&fake, 0, "env").contains("OPERATOR_SECRET"));
    let only = launch(&fake, 4, "env");
    assert!(only.lines().any(|l| l.starts_with("PATH=")), "{only}");
    for absent in ["OPERATOR_SECRET=", "HOME=", "FAKE_LOG="] {
        assert!(!only.lines().any(|l| l.starts_with(absent)), "{only}");
    }

    // The compare verb reads the directory as written: the fake's streams
    // mean what the goldens mean, except where the fake is cruder.
    let compared = Command::new(binary())
        .args([
            "provider",
            "compare",
            &out.display().to_string(),
            "--corpus",
            &corpus().display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    let c = json_naming::from_output(&compared.stdout).unwrap();
    assert_eq!(c["report"]["providerVersion"], VERSION, "{c}");
    assert_eq!(c["report"]["rows"].as_array().unwrap().len(), 5, "{c}");
}

#[test]
fn a_scenario_with_no_stream_is_named_and_the_rest_are_written() {
    let fake = fake();
    // A provider that writes nothing for the turn-capped scenario.
    let script = fake.bin.path().join("claude");
    let body = fs::read_to_string(&script).unwrap().replacen(
        "cat <<EOF",
        "[ \"$subtype\" = error_max_turns ] && exit 1\ncat <<EOF",
        1,
    );
    fs::write(&script, body).unwrap();
    let parent = tempfile::tempdir().unwrap();
    let out = parent.path().join("captures");
    let o = capture(&fake, &out);
    let v = report(&o);
    assert_eq!(o.status.code(), Some(4), "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("max-turns"),
        "{v}"
    );
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(out.join("captures.json")).unwrap()).unwrap();
    assert_eq!(manifest["captures"].as_array().unwrap().len(), 4);
    assert!(!out.join("max-turns.jsonl").exists());
}

#[test]
fn a_directory_that_is_not_empty_or_no_provider_is_refused_before_any_spawn() {
    let fake = fake();
    let out = tempfile::tempdir().unwrap();
    fs::write(out.path().join("keep"), "").unwrap();
    let o = capture(&fake, out.path());
    assert_eq!(o.status.code(), Some(2), "{}", report(&o));
    assert_eq!(fs::read_dir(fake.log.path()).unwrap().count(), 0);

    let empty = tempfile::tempdir().unwrap();
    let fresh = empty.path().join("captures");
    let o = Command::new(binary())
        .args([
            "provider",
            "capture",
            &fresh.display().to_string(),
            "--json",
        ])
        .env("PATH", empty.path())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2), "{}", report(&o));
    assert!(!fresh.exists());

    let o = Command::new(binary())
        .args(["provider", "capture", "--json"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(3));
    assert_eq!(report(&o)["error"]["kind"], "usage");
}
