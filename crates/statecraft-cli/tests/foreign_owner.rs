//! A `foreign` finding names an owner, not only a path: spec 002 section 3.21
//! part 1, with section 3.2's `foreign` row and section 3.8's pointer rule,
//! through the built binary's `env plan`, `env apply` and `doctor`.
//!
//! Every invocation gets a temporary `STATECRAFT_HOME` and `HOME` and a `PATH`
//! holding only the fixture's own directory and the system directories.
//!
//! `doctor` reports a declared path whatever the adapter's readiness, so its
//! half runs everywhere. `env plan` and `env apply` only reach a declared path
//! when the adapter claims its paths, and one of its prerequisites is the
//! darwin credential path (spec 004 section 3.14), so that half runs on macOS
//! and is compiled out elsewhere rather than skipped at run time.

#![cfg(unix)]

#[path = "support/json_naming.rs"]
mod json_naming;

use std::path::PathBuf;
use std::process::{Command, Output};

/// The adapter's pointer path and the bytes it would write there.
const POINTER: &str = "CLAUDE.md";
const USER_BYTES: &[u8] = b"# my own instructions\n";

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let f = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir_all(f.home()).unwrap();
        std::fs::create_dir_all(f.bin()).unwrap();
        std::fs::create_dir_all(f.project()).unwrap();
        for args in [
            vec!["init", "--quiet", "--initial-branch=main"],
            vec!["config", "user.email", "fixture@example.invalid"],
            vec!["config", "user.name", "fixture"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["commit", "--quiet", "--allow-empty", "-m", "base"],
        ] {
            let out = Command::new("git")
                .args(&args)
                .current_dir(f.project())
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}");
        }
        let out = f.cli(&["project", "register", &f.root()]);
        assert!(code(&out) <= 1, "{}", text(&out));
        f
    }

    fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }
    fn bin(&self) -> PathBuf {
        self.dir.path().join("bin")
    }
    fn project(&self) -> PathBuf {
        self.dir.path().join("project")
    }
    fn root(&self) -> String {
        self.project().display().to_string()
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", self.home())
            .env("HOME", self.dir.path())
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin().display()))
            .env("USER", "fixture-operator")
            .output()
            .unwrap()
    }

    fn json(&self, args: &[&str]) -> (i32, serde_json::Value) {
        let mut args = args.to_vec();
        args.push("--json");
        let out = self.cli(&args);
        let value =
            json_naming::from_output(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(&out)));
        (code(&out), value)
    }
}

fn code(out: &Output) -> i32 {
    out.status.code().expect("exited by itself")
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn findings(value: &serde_json::Value) -> Vec<String> {
    value["report"]["findings"]
        .as_array()
        .unwrap_or_else(|| panic!("{value}"))
        .iter()
        .map(|f| f.as_str().unwrap().to_string())
        .collect()
}

/// A user's file at the adapter's pointer path is `foreign`, owned by the
/// user, and never an unrecorded write of this product's.
#[test]
fn doctor_names_the_user_as_the_owner_of_a_pointer_path_it_did_not_write() {
    let f = Fixture::new();
    std::fs::write(f.project().join(POINTER), USER_BYTES).unwrap();

    let (exit, value) = f.json(&["doctor", &f.root()]);
    assert_eq!(exit, 1, "foreign is a finding: {value}");
    let found = findings(&value);
    assert!(
        found.iter().any(
            |l| l.starts_with("foreign CLAUDE.md, owner user, declared by adapter claude-code")
        ),
        "{found:?}"
    );
    assert!(
        !found.iter().any(|l| l.contains("unmanaged-write")),
        "the user's file is not this product's write: {found:?}"
    );

    let human = f.cli(&["doctor", &f.root()]);
    assert_eq!(code(&human), 1);
    assert!(text(&human).contains("owner user"), "{}", text(&human));

    // Diagnoses, and repairs nothing.
    assert_eq!(
        std::fs::read(f.project().join(POINTER)).unwrap(),
        USER_BYTES
    );
    assert!(!f.project().join(".claude").exists());
    assert!(!f.project().join(".statecraft").exists());
}

/// The distinction the owner rests on: the adapter's own bytes at its own
/// path, unrecorded, are still reported as the write this product did not
/// record.
#[test]
fn doctor_still_reports_the_adapters_own_bytes_unrecorded_as_an_unmanaged_write() {
    let f = Fixture::new();
    std::fs::write(
        f.project().join(POINTER),
        statecraft_adapter_claude_code::environment::pointer_contents(),
    )
    .unwrap();

    let (exit, value) = f.json(&["doctor", &f.root()]);
    assert_eq!(exit, 1, "{value}");
    let found = findings(&value);
    assert!(
        found
            .iter()
            .any(|l| l == "unmanaged-write CLAUDE.md, declared by adapter claude-code"),
        "{found:?}"
    );
    assert!(!found.iter().any(|l| l.starts_with("foreign")), "{found:?}");
}

/// `env plan` and `env apply` name the owner of every occupied path, in the
/// reason and as its own JSON field, and write over neither.
#[cfg(target_os = "macos")]
#[test]
fn env_plan_and_apply_name_the_owner_of_every_path_they_withhold() {
    let f = Fixture::new();
    qualify_the_adapter(&f);
    let owned = statecraft_adapter_claude_code::environment::OWNED_INSTRUCTIONS;
    std::fs::write(f.project().join(POINTER), USER_BYTES).unwrap();

    let (exit, plan) = f.json(&["env", "plan", &f.root()]);
    assert_eq!(exit, 1, "a withheld path is a finding: {plan}");
    assert_eq!(
        plan["report"]["adapters"][0]["readiness"], "degraded",
        "{plan}"
    );
    let withheld = plan["report"]["withheld"].as_array().unwrap();
    assert_eq!(withheld.len(), 1, "{plan}");
    assert_eq!(withheld[0]["path"], POINTER);
    assert_eq!(withheld[0]["owner"], "user");
    assert!(
        withheld[0]["reason"]
            .as_str()
            .unwrap()
            .contains("owner user: CLAUDE.md"),
        "{plan}"
    );
    assert!(
        !f.project().join(".claude").exists(),
        "a plan writes nothing"
    );

    let (exit, applied) = f.json(&["env", "apply", &f.root()]);
    assert_eq!(exit, 1, "partial: {applied}");
    let withheld = applied["report"]["withheld"].as_array().unwrap();
    assert_eq!(withheld[0]["path"], POINTER);
    assert_eq!(withheld[0]["owner"], "user");
    assert_eq!(
        std::fs::read(f.project().join(POINTER)).unwrap(),
        USER_BYTES
    );
    assert!(f.project().join(owned).is_file(), "its own path is written");

    // The adapter's own path, occupied by the user before any apply, is
    // withheld with the same owner.
    let g = Fixture::new();
    qualify_the_adapter(&g);
    let at = g.project().join(owned);
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(&at, USER_BYTES).unwrap();
    let (exit, applied) = g.json(&["env", "apply", &g.root()]);
    assert_eq!(exit, 1, "partial: {applied}");
    let withheld = applied["report"]["withheld"].as_array().unwrap();
    let entry = withheld
        .iter()
        .find(|w| w["path"] == owned)
        .unwrap_or_else(|| panic!("{applied}"));
    assert_eq!(entry["owner"], "user");
    assert!(
        entry["reason"]
            .as_str()
            .unwrap()
            .starts_with("claimed by owner user")
    );
    assert_eq!(
        std::fs::read(&at).unwrap(),
        USER_BYTES,
        "never written over"
    );

    // And `doctor` agrees with the apply about who holds it.
    let (_, value) = g.json(&["doctor", &g.root()]);
    let found = findings(&value);
    assert!(
        found
            .iter()
            .any(|l| l.starts_with(&format!("foreign {owned}, owner user"))),
        "{found:?}"
    );
}

/// A fake `claude` and a synthetic qualification record for it, so the
/// adapter's prerequisites hold and it claims its paths. No provider runs.
#[cfg(target_os = "macos")]
fn qualify_the_adapter(f: &Fixture) {
    statecraft_adapter::fixture::install_script(
        &f.bin().join("claude"),
        "#!/bin/sh\n[ \"$1\" = --version ] && { echo '2.1.267 (Claude Code)'; exit 0; }\nexit 3\n",
        0o755,
    )
    .unwrap();
    let record = statecraft_adapter_claude_code::qualification::record(
        "2.1.267",
        "synthetic-fixture-only",
        "2026-09-23T00:00:00Z",
    );
    std::fs::write(
        f.home().join("qualifications.json"),
        serde_json::to_vec(&vec![record]).unwrap(),
    )
    .unwrap();
}
