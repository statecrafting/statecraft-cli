//! Arming is the consent to being driven, and `run` is what drives.
//!
//! Spec 002 section 3.1 separates arming from registration: registering says
//! the target exists, arming says it may be driven. Spec 006 section 3.3 makes
//! an unmet precondition a refusal (2). This file is the command-level
//! regression for the two together.
//!
//! A separate file from `integration_slice.rs` and `negative_cases.rs`, which
//! carry spec 006's negative-case rows: a row deleted from
//! one of those files should read as a deleted row, not as a refactor.
//!
//! The whole difficulty of testing a refusal is proving it refused for the
//! reason claimed. A target that is unarmed *and* has no ready spec refuses
//! either way, so every assertion here is made against **one** target whose
//! only changing property is its consent: unarmed refuses, armed completes,
//! disarmed refuses again. The stub provider is a shell script; no live
//! provider, session or credential is involved.

#![cfg(unix)]

use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn executable(path: &Path, script: &str) {
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// The target, the product home, and the stub tools the run would reach.
struct Fixture {
    home: tempfile::TempDir,
    target: tempfile::TempDir,
    bin: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();

        for args in [
            vec!["init", "--quiet"],
            vec!["config", "user.name", "fixture"],
            vec!["config", "user.email", "fixture@example.com"],
            vec!["commit", "--quiet", "--allow-empty", "-m", "fixture base"],
        ] {
            assert!(
                Command::new("git")
                    .current_dir(target.path())
                    .args(args)
                    .status()
                    .unwrap()
                    .success()
            );
        }

        // One ready spec that is `approved` plus `pending`, so the unit of work
        // is schedulable under the default policy and cannot be what a refusal
        // is really about. Discovery is the join spec 003 section 3.1.1
        // prescribes, so both reports are answered.
        executable(
            &bin.path().join("spec-spine"),
            r#"#!/bin/sh
case "$*" in
  --version) echo 'spec-spine 0.20.0' ;;
  check) exit 0 ;;
  'registry plan --json') echo '{"ready":[{"id":"fixture","title":"arming fixture"}]}' ;;
  'registry list --json') echo '{"items":[{"id":"fixture","status":"approved","implementation":"pending"}]}' ;;
  *) exit 3 ;;
esac
"#,
        );

        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../statecraft-adapter-claude-code/testdata/stream/success.jsonl"),
            bin.path().join("stream.jsonl"),
        )
        .unwrap();

        // The spawn marker is written beside the stub rather than in the
        // workspace, so "was a provider spawned" stays answerable even when no
        // workspace was prepared. It is written only on a session invocation:
        // `--version` is a probe and returns before it.
        executable(
            &bin.path().join("claude"),
            r#"#!/bin/sh
here="$(dirname "$0")"
if [ "$1" = --version ]; then echo '2.1.267'; exit 0; fi
: > "$here/spawned"
/bin/cat > /dev/null
/bin/cat "$here/stream.jsonl"
"#,
        );

        Self { home, target, bin }
    }

    fn root(&self) -> &str {
        self.target.path().to_str().unwrap()
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", self.home.path())
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.bin.path().display()),
            )
            // Spec 008 section 3.6: the account name is carried to the child
            // with this process's own value, and `HOME` still is not.
            .env("USER", "fixture-operator")
            .output()
            .unwrap()
    }

    /// Whether a provider process was created.
    fn spawned(&self) -> bool {
        self.bin.path().join("spawned").exists()
    }

    fn clear_spawn_marker(&self) {
        let marker = self.bin.path().join("spawned");
        if marker.exists() {
            std::fs::remove_file(marker).unwrap();
        }
    }

    /// The run record for this target, or nothing if none was ever opened.
    fn chain_path(&self) -> PathBuf {
        statecraft_run::record::chain_path(self.home.path(), self.target.path())
    }

    /// Every attempt recorded for the fixture run, folded from the record.
    fn attempts(&self) -> usize {
        if !self.chain_path().exists() {
            return 0;
        }
        let (chain, _) =
            statecraft_run::record::Chain::open(self.home.path(), self.target.path()).unwrap();
        statecraft_run::session::runs(&chain)
            .iter()
            .find(|r| r.id == "fixture")
            .map(|r| r.attempts.len())
            .unwrap_or(0)
    }

    /// Whether any run workspace exists inside the target.
    fn workspaces(&self) -> bool {
        self.target
            .path()
            .join(statecraft_run::workspace::WORKSPACES_DIR)
            .exists()
    }
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("the process exited normally")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

/// Registered, unarmed, armed, disarmed: one target, one changing property.
///
/// The three obligations are asserted in sequence on the same fixture on
/// purpose. Split across three fixtures, the first would prove only that
/// *something* refused, and the setup each one shares is exactly the thing that
/// would have to be trusted.
#[test]
fn run_is_refused_until_the_target_is_armed_and_again_once_it_is_disarmed() {
    let f = Fixture::new();
    let root = f.root();

    let registered = f.run(&["project", "register", root]);
    assert!(code(&registered) <= 1, "{registered:?}");

    // The control. Registration alone leaves the target unarmed, and the
    // product says so: an assertion against the state the rest of this test
    // turns on, rather than an assumption about it.
    let listed = f.run(&["project", "list", "--json"]);
    let parsed: Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(parsed["value"][0]["armed"], false, "{listed:?}");

    // And the unit of work is schedulable while the target is unarmed, so a
    // refusal from `run` cannot be the policy declining this spec. `work show`
    // exits 0 only for a schedulable unit (spec 006 section 3.10).
    let shown = f.run(&["work", "show", root, "fixture"]);
    assert_eq!(code(&shown), 0, "the fixture spec is eligible: {shown:?}");

    // 1. Unarmed refuses, and nothing was done.
    let refused = f.run(&["run", root, "fixture", "--json"]);
    assert_eq!(code(&refused), 2, "{refused:?}");
    let answer: Value = serde_json::from_slice(&refused.stdout).unwrap();
    assert!(
        answer["summary"].as_str().unwrap().contains("not armed"),
        "the reason is the answer: {answer}"
    );
    assert!(
        answer["summary"].as_str().unwrap().contains("project arm"),
        "and it names the act that would consent: {answer}"
    );
    // Spec 006 section 3.3's "nothing was done", checked as three separate
    // absences rather than inferred from the exit code.
    assert!(!f.chain_path().exists(), "no attempt was appended");
    assert!(!f.workspaces(), "no run workspace was created");
    assert!(!f.spawned(), "no provider process was created");

    // The human rendering refuses identically: spec 006 section 3.4 makes the
    // two renderings views of one value.
    let human = f.run(&["run", root, "fixture"]);
    assert_eq!(code(&human), 2, "{human:?}");
    assert!(stdout(&human).contains("not armed"), "{human:?}");
    assert!(!f.chain_path().exists(), "still no attempt");

    // 2. Arming the same target reaches the existing execution path, which is
    // what makes consent the deciding condition and not a coincidence.
    let armed = f.run(&["project", "arm", root]);
    assert_eq!(code(&armed), 0, "{armed:?}");

    let ran = f.run(&["run", root, "fixture", "--json"]);
    assert_eq!(code(&ran), 0, "{ran:?}");
    let answer: Value = serde_json::from_slice(&ran.stdout).unwrap();
    assert_eq!(answer["value"]["outcome"], "completed", "{answer}");
    assert!(f.spawned(), "the provider was reached");
    assert!(f.workspaces(), "a workspace was prepared");
    assert_eq!(f.attempts(), 1, "one attempt was appended");

    // 3. Disarming withdraws the consent for the next invocation. It does not
    // touch the attempt that already ran: this change refuses a run, it does
    // not cancel one.
    f.clear_spawn_marker();
    let disarmed = f.run(&["project", "disarm", root]);
    assert_eq!(code(&disarmed), 0, "{disarmed:?}");

    let refused_again = f.run(&["run", root, "fixture", "--json"]);
    assert_eq!(code(&refused_again), 2, "{refused_again:?}");
    assert!(
        String::from_utf8_lossy(&refused_again.stdout).contains("not armed"),
        "{refused_again:?}"
    );
    assert!(!f.spawned(), "no second provider process");
    assert_eq!(f.attempts(), 1, "no second attempt was appended");

    // The record written by the armed run is still readable, because
    // inspection is not gated on consent.
    let inspected = f.run(&["run", "show", root, "fixture", "--json"]);
    assert_eq!(code(&inspected), 0, "{inspected:?}");
    let listed = f.run(&["run", "list", root]);
    assert_eq!(code(&listed), 0, "{listed:?}");
    assert!(stdout(&listed).contains("fixture"), "{listed:?}");
}

/// Discovery and inspection are read-only and stay available while unarmed.
///
/// Spec 002 section 3.1's whole reason for recording a target the product may
/// not drive is that it is still visible. A gate that took the read verbs with
/// it would have removed that.
#[test]
fn discovery_and_inspection_are_not_gated_on_consent() {
    let f = Fixture::new();
    let root = f.root();
    assert!(code(&f.run(&["project", "register", root])) <= 1);

    let listed = f.run(&["work", "list", root, "--json"]);
    assert_eq!(code(&listed), 0, "{listed:?}");
    let answer: Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(answer["value"]["eligible"][0]["id"], "fixture", "{answer}");

    // `run list` on a target with no runs is an empty answer, not a refusal.
    let runs = f.run(&["run", "list", root]);
    assert_eq!(code(&runs), 0, "{runs:?}");

    assert!(!f.spawned(), "reading never reaches a provider");
    assert!(!f.workspaces(), "reading prepares no workspace");
}
