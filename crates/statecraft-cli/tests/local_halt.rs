//! Spec 050 at the command surface: `halt`, `resume`, the pre-launch refusal
//! of `run`, and what `run list` shows.
//!
//! R-1 and I-1: a halted run-admission scope refuses `run` with exit 2, kind
//! `refused` and the closed reason `halted` naming the scope, the operator and
//! the reason; a record that cannot be read refuses with
//! `halt-state-unreadable`. R-3: nothing is written inside the target. No
//! provider is involved: a halted run is refused before the provider is
//! looked for, and an admitted one is refused *by* that look, which is how
//! the two refusals are told apart.

#![cfg(unix)]

#[path = "support/json_naming.rs"]
mod json_naming;

#[path = "support/provider_token.rs"]
mod provider_token;

use serde_json::Value;
use std::process::{Command, Output};

struct Fixture {
    home: tempfile::TempDir,
    target: tempfile::TempDir,
    bin: tempfile::TempDir,
    operator_home: tempfile::TempDir,
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
        // One schedulable spec, so a refusal from `run` is never the policy's.
        statecraft_adapter::fixture::install_script(
            &bin.path().join("spec-spine"),
            r#"#!/bin/sh
case "$*" in
  --version) echo 'spec-spine 0.20.0' ;;
  check|'check --help') exit 0 ;;
  'registry plan --json') echo '{"ready":[{"id":"fixture","title":"halt fixture"}]}' ;;
  'registry list --json') echo '{"items":[{"id":"fixture","status":"approved","implementation":"pending"}]}' ;;
  'verify '*' --plan --json') printf '{"exitCode":0,"ok":true,"report":{"commands":[],"skipped":[],"specId":"%s"},"schemaVersion":"0.6.0","verb":"verify"}' $2 ;;
  *) exit 3 ;;
esac
"#,
            0o755,
        )
        .unwrap();
        let operator_home = tempfile::tempdir().unwrap();
        std::fs::write(operator_home.path().join(".claude.json"), "{}").unwrap();
        let f = Self {
            home,
            target,
            bin,
            operator_home,
        };
        assert!(code(&f.run(&["project", "register", f.root()])) <= 1);
        assert_eq!(code(&f.run(&["project", "arm", f.root()])), 0);
        f
    }

    fn root(&self) -> &str {
        self.target.path().to_str().unwrap()
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env(
                "STATECRAFT_HOME",
                provider_token::with_token(self.home.path()),
            )
            .env("HOME", self.operator_home.path())
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.bin.path().display()),
            )
            .env("USER", "fixture-operator")
            .output()
            .unwrap()
    }

    fn json(&self, args: &[&str]) -> (i32, Value) {
        let mut args = args.to_vec();
        args.push("--json");
        let out = self.run(&args);
        let v = json_naming::from_output(&out.stdout)
            .unwrap_or_else(|e| panic!("{args:?}: {e}: {}", String::from_utf8_lossy(&out.stdout)));
        (code(&out), v)
    }

    fn chain_path(&self) -> std::path::PathBuf {
        statecraft_run::record::chain_path(self.home.path(), self.target.path())
    }

    fn target_entries(&self) -> Vec<std::ffi::OsString> {
        let mut names: Vec<_> = std::fs::read_dir(self.target.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        names.sort();
        names
    }
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("the process exited normally")
}

#[test]
fn a_halted_run_admission_scope_refuses_run_and_resume_admits_it_again() {
    let f = Fixture::new();
    let root = f.root();
    let before = f.target_entries();

    let (exit, halted) = f.json(&["halt", root, "bart", "incident", "review"]);
    assert_eq!(exit, 0, "{halted}");
    assert_eq!(halted["report"]["change"], "halt");
    assert_eq!(
        halted["report"]["detail"]["scopes"],
        serde_json::json!(["run-admission", "tool-effects", "dispatch"])
    );
    assert_eq!(halted["report"]["detail"]["reason"], "incident review");
    assert!(
        f.chain_path().is_file(),
        "the halt is in the product home's run record"
    );
    assert_eq!(
        f.target_entries(),
        before,
        "nothing in the target says halted"
    );

    // R-1: exit 2, kind refused, the closed reason, scope, operator and reason.
    let (exit, refused) = f.json(&["run", root, "fixture"]);
    assert_eq!(exit, 2, "{refused}");
    assert_eq!(refused["error"]["kind"], "refused", "{refused}");
    let details = &refused["error"]["details"];
    assert_eq!(details["outcome"], "refused");
    assert_eq!(details["reason"], "halted");
    assert_eq!(details["halt"]["scope"], "run-admission");
    assert_eq!(details["halt"]["operator"], "bart");
    assert_eq!(details["halt"]["reason"], "incident review");
    let human = f.run(&["run", root, "fixture"]);
    assert_eq!(code(&human), 2);
    let text = String::from_utf8_lossy(&human.stdout);
    assert!(
        text.contains("halted") && text.contains("bart") && text.contains("incident review"),
        "{text}"
    );
    // I-1: no attempt was appended and no workspace prepared.
    let (_, listed) = f.json(&["run", "list", root]);
    assert_eq!(listed["report"]["runs"], serde_json::json!([]));
    assert_eq!(
        listed["report"]["halt"]["halted"].as_array().unwrap().len(),
        3
    );
    assert_eq!(listed["report"]["halt"]["unreadable"], Value::Null);
    assert!(
        !f.target
            .path()
            .join(statecraft_run::workspace::WORKSPACES_DIR)
            .exists()
    );

    // Lifting one scope leaves run-admission halted.
    let (exit, lifted) = f.json(&[
        "resume", root, "--scope", "dispatch", "bart", "dispatch", "is", "fine",
    ]);
    assert_eq!(exit, 0, "{lifted}");
    assert_eq!(
        lifted["report"]["state"]["halted"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let (exit, refused) = f.json(&["run", root, "fixture"]);
    assert_eq!(exit, 2);
    assert_eq!(refused["error"]["details"]["reason"], "halted");

    // Lifting all admits the run past the halt check: the next refusal is the
    // missing provider, which is this fixture's and not the halt's.
    let (exit, _) = f.json(&["resume", root, "bart", "resolved"]);
    assert_eq!(exit, 0);
    let (exit, after) = f.json(&["run", root, "fixture"]);
    assert_eq!(exit, 2, "{after}");
    assert_ne!(after["error"]["details"]["reason"], "halted", "{after}");
    assert_ne!(
        after["error"]["details"]["reason"], "halt-state-unreadable",
        "{after}"
    );
    let (_, listed) = f.json(&["run", "list", root]);
    assert_eq!(listed["report"]["halt"]["halted"], serde_json::json!([]));
}

#[test]
fn a_halt_of_another_scope_alone_does_not_refuse_run() {
    let f = Fixture::new();
    let root = f.root();
    let (exit, _) = f.json(&[
        "halt",
        root,
        "--scope=tool-effects",
        "--scope",
        "dispatch",
        "bart",
        "r",
    ]);
    assert_eq!(exit, 0);
    let (exit, answer) = f.json(&["run", root, "fixture"]);
    assert_eq!(exit, 2);
    assert_ne!(answer["error"]["details"]["reason"], "halted", "{answer}");
}

#[test]
fn a_truncated_record_refuses_run_with_halt_state_unreadable_and_run_list_says_so() {
    let f = Fixture::new();
    let root = f.root();
    let (exit, _) = f.json(&["halt", root, "--scope", "dispatch", "bart", "r"]);
    assert_eq!(exit, 0);
    let bytes = std::fs::read(f.chain_path()).unwrap();
    std::fs::write(f.chain_path(), &bytes[..bytes.len() - 5]).unwrap();

    let (exit, refused) = f.json(&["run", root, "fixture"]);
    assert_eq!(exit, 2, "{refused}");
    assert_eq!(refused["error"]["kind"], "refused");
    assert_eq!(
        refused["error"]["details"]["reason"],
        "halt-state-unreadable"
    );
    let (exit, listed) = f.json(&["run", "list", root]);
    assert_eq!(exit, 0);
    assert!(
        listed["report"]["halt"]["unreadable"]
            .as_str()
            .unwrap()
            .contains("halt-state-unreadable"),
        "{listed}"
    );
    // A halt against an unreadable record is refused too: nothing is written
    // on top of a state nobody can read.
    let (exit, refused) = f.json(&["halt", root, "bart", "again"]);
    assert_eq!(exit, 2, "{refused}");
    assert_eq!(
        refused["error"]["details"]["reason"],
        "halt-state-unreadable"
    );
}

#[test]
fn a_chain_mismatched_record_refuses_run_with_halt_state_unreadable() {
    let f = Fixture::new();
    let root = f.root();
    let (exit, _) = f.json(&["halt", root, "--scope", "dispatch", "bart", "r"]);
    assert_eq!(exit, 0);
    let text = std::fs::read_to_string(f.chain_path()).unwrap();
    std::fs::write(f.chain_path(), text.replace("dispatch", "run-admission")).unwrap();
    let (exit, refused) = f.json(&["run", root, "fixture"]);
    assert_eq!(exit, 2, "{refused}");
    assert_eq!(
        refused["error"]["details"]["reason"],
        "halt-state-unreadable"
    );
}

#[test]
fn halt_and_resume_need_a_path_an_operator_and_a_reason() {
    let f = Fixture::new();
    let root = f.root();
    for args in [
        vec!["halt"],
        vec!["halt", root],
        vec!["halt", root, "bart"],
        vec!["resume", root, "--scope", "everything", "bart", "r"],
    ] {
        let (exit, v) = f.json(&args);
        assert_eq!(exit, 3, "{args:?}: {v}");
        assert_eq!(v["error"]["kind"], "usage");
    }
    let unregistered = tempfile::tempdir().unwrap();
    let (exit, v) = f.json(&["halt", unregistered.path().to_str().unwrap(), "bart", "r"]);
    assert_eq!(exit, 2, "{v}");
    assert!(!f.chain_path().exists(), "no refused halt wrote a record");
}
