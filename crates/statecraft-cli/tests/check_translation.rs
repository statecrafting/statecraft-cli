//! Two exit vocabularies, and no numeric passthrough between them: spec 002
//! section 3.23's translation of `spec-spine check`, one test per row of its
//! table, through the built binary's `project register` and `init apply`.
//!
//! | spec-spine `check` answered | this product reports |
//! |---|---|
//! | 0 fresh | 0 |
//! | 1 the corpus does not validate | 1, a finding |
//! | 2 stale, or an unresolved claim | 1, a finding, the readings told apart in the text |
//! | 3 the read was not performed | 4, a failure |
//! | the binary is absent, or lacks the verb | 2, a refusal |
//!
//! The `spec-spine` here is a stub whose answers are files beside it, so one
//! installed script serves every row. Every invocation gets a temporary
//! `STATECRAFT_HOME` and `HOME`, and a `PATH` of the stub's directory and the
//! system directories only.

#![cfg(unix)]

#[path = "support/json_naming.rs"]
mod json_naming;

use std::path::PathBuf;
use std::process::{Command, Output};

const STUB: &str = r#"#!/bin/sh
here="$(dirname "$0")"
case "$*" in
  --version) echo 'spec-spine 0.23.0' ;;
  'check --help') exit "$(/bin/cat "$here/help-exit")" ;;
  check) /bin/cat "$here/check-text"; exit "$(/bin/cat "$here/check-exit")" ;;
  compile|index) exit 0 ;;
  *) exit 3 ;;
esac
"#;

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    /// A git work tree carrying a corpus marker, and a stub `spec-spine`
    /// answering `check` with `code` and `text`.
    fn new(help: i32, code: i32, text: &str) -> Self {
        let f = Self {
            dir: tempfile::tempdir().unwrap(),
        };
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
        statecraft_adapter::fixture::install_script(&f.bin().join("spec-spine"), STUB, 0o755)
            .unwrap();
        std::fs::write(f.bin().join("help-exit"), help.to_string()).unwrap();
        std::fs::write(f.bin().join("check-exit"), code.to_string()).unwrap();
        std::fs::write(f.bin().join("check-text"), text).unwrap();
        f
    }

    /// The same, with no `spec-spine` anywhere on `PATH`.
    fn without_spec_spine() -> Self {
        let f = Self::new(0, 0, "");
        std::fs::remove_file(f.bin().join("spec-spine")).unwrap();
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
            .env("STATECRAFT_NATIVE_ROOT", self.dir.path().join("native"))
            .env("HOME", self.dir.path())
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin().display()))
            .env("USER", "fixture-operator")
            .output()
            .unwrap()
    }

    /// `project register` on the corpus: its exit, and what it said.
    fn register(&self) -> (i32, String) {
        std::fs::write(self.project().join("spec-spine.toml"), "").unwrap();
        let out = self.cli(&["project", "register", &self.root()]);
        (code(&out), text(&out))
    }

    fn registered(&self) -> bool {
        self.home().join("projects.json").exists()
    }

    /// `init apply`: its exit, and the corpus and register steps.
    fn init(&self) -> (i32, serde_json::Value, serde_json::Value, serde_json::Value) {
        let out = self.cli(&["init", "apply", &self.root(), "--json"]);
        let answer: serde_json::Value =
            json_naming::from_output(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(&out)));
        let steps = json_naming::payload(&answer)["value"]["steps"]
            .as_array()
            .unwrap_or_else(|| panic!("{answer}"))
            .clone();
        let step = |name: &str| {
            steps
                .iter()
                .find(|s| s["step"] == name)
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        };
        (code(&out), step("corpus"), step("register"), answer)
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

// 0 fresh: 0.
#[test]
fn fresh_is_zero_and_the_corpus_step_is_done() {
    let f = Fixture::new(0, 0, "");
    let (exit, said) = f.register();
    assert_eq!(exit, 0, "{said}");
    assert!(said.contains("qualified"), "{said}");

    let g = Fixture::new(0, 0, "");
    let (_, corpus, register, answer) = g.init();
    assert_eq!(corpus["state"]["state"], "done", "{answer}");
    assert_eq!(register["state"]["state"], "done", "{answer}");
}

// 1 the corpus does not validate: 1, a finding.
#[test]
fn a_corpus_that_does_not_validate_is_a_finding() {
    let f = Fixture::new(0, 1, "E-001: a spec does not validate\n");
    let (exit, said) = f.register();
    assert_eq!(exit, 1, "{said}");
    assert!(said.contains("unqualified"), "{said}");
    assert!(said.contains("does not validate: E-001"), "{said}");
    assert!(f.registered(), "a finding is a verdict, and it is recorded");

    let g = Fixture::new(0, 1, "E-001: a spec does not validate\n");
    let (exit, corpus, _, answer) = g.init();
    assert_eq!(exit, 1, "{answer}");
    assert_eq!(corpus["state"]["state"], "withheld", "{answer}");
    assert!(
        corpus["detail"]
            .as_str()
            .unwrap()
            .contains("does not validate"),
        "{answer}"
    );
}

// 2 stale: 1, a finding, and the text says stale. Never 2, which would read as
// a refusal.
#[test]
fn a_stale_tree_is_a_finding_that_says_stale() {
    let stale = "spec-registry: STALE\n1 stale shard(s):\n";
    let f = Fixture::new(0, 2, stale);
    let (exit, said) = f.register();
    assert_eq!(exit, 1, "{said}");
    assert!(said.contains("stale, which regenerating cures"), "{said}");

    let g = Fixture::new(0, 2, stale);
    let (exit, corpus, _, answer) = g.init();
    assert_eq!(exit, 1, "{answer}");
    assert_eq!(corpus["state"]["state"], "withheld", "{answer}");
    let detail = corpus["detail"].as_str().unwrap();
    assert!(
        detail.contains("stale, which regenerating cures"),
        "{detail}"
    );
    assert!(!detail.contains("unresolved"), "{detail}");
}

// 2 an unresolved claim: 1, a finding, and the text tells it from stale.
#[test]
fn an_unresolved_claim_is_a_finding_whose_text_is_not_stale() {
    let unresolved = "codebase-index: 1 unresolved claim: crates/missing/\n";
    let f = Fixture::new(0, 2, unresolved);
    let (exit, said) = f.register();
    assert_eq!(exit, 1, "{said}");
    assert!(said.contains("an unresolved claim, which regenerating does not cure"));

    let g = Fixture::new(0, 2, unresolved);
    let (exit, corpus, _, answer) = g.init();
    assert_eq!(exit, 1, "{answer}");
    let detail = corpus["detail"].as_str().unwrap();
    assert!(detail.contains("unresolved claim"), "{detail}");
    assert!(!detail.contains("which regenerating cures"), "{detail}");
}

// 3 the read was not performed: 4, a failure, and nothing is recorded as a
// verdict about the corpus.
#[test]
fn a_read_that_was_not_performed_is_a_failure() {
    let pin = "spec-spine 0.23.0 does not satisfy required_version =0.99.0\n";
    let f = Fixture::new(0, 3, pin);
    let (exit, said) = f.register();
    assert_eq!(exit, 4, "{said}");
    assert!(said.contains("did not perform its read (exit 3)"), "{said}");
    assert!(
        !f.registered(),
        "no verdict was reached, so none is recorded"
    );

    let g = Fixture::new(0, 3, pin);
    let (exit, corpus, _, answer) = g.init();
    assert_eq!(exit, 4, "{answer}");
    assert_eq!(corpus["state"]["state"], "failed", "{answer}");
}

// The binary is absent: 2, a refusal, and nothing is done. Where `init
// apply` had already written its files, the corpus and register steps are
// refused and nothing in them was done, and the initialization is `partial`
// (1): section 3.17 keeps `refused` for a precondition that stopped it before
// any write.
#[test]
fn an_absent_binary_is_a_refusal() {
    let f = Fixture::without_spec_spine();
    let (exit, said) = f.register();
    assert_eq!(exit, 2, "{said}");
    assert!(said.contains("spec-spine is not available"), "{said}");
    assert!(!f.registered(), "a refusal records nothing");

    let g = Fixture::without_spec_spine();
    let (exit, corpus, register, answer) = g.init();
    assert_eq!(exit, 1, "{answer}");
    assert_eq!(answer["report"]["value"]["outcome"], "partial", "{answer}");
    assert_eq!(corpus["state"]["state"], "refused", "{answer}");
    assert_eq!(register["state"]["state"], "refused", "{answer}");
    assert!(
        answer["report"]["value"]["writes"]
            .as_array()
            .is_some_and(|w| !w.is_empty()),
        "{answer}"
    );
    assert!(!g.registered());
}

// The binary lacks the verb: 2, a refusal. Contract 5: the verb is established
// before `check`'s code is read, so a binary whose unknown subcommand exits 2
// (clap) or 3 (spec-spine 0.23.0) is never read as stale or as a read not
// performed, and one whose `check` would have answered 0 is not trusted
// either.
#[test]
fn a_binary_lacking_the_verb_is_a_refusal_whatever_check_would_have_answered() {
    for (help, check) in [(2, 2), (3, 3), (2, 0)] {
        let f = Fixture::new(help, check, "error: unrecognized subcommand 'check'\n");
        let (exit, said) = f.register();
        assert_eq!(exit, 2, "help {help}, check {check}: {said}");
        assert!(said.contains("does not carry `check`"), "{said}");
        assert!(!f.registered());

        // After its writes, init is partial with the corpus step refused
        // (section 3.17), as for an absent binary.
        let g = Fixture::new(help, check, "error: unrecognized subcommand 'check'\n");
        let (exit, corpus, _, answer) = g.init();
        assert_eq!(exit, 1, "help {help}, check {check}: {answer}");
        assert_eq!(answer["report"]["value"]["outcome"], "partial", "{answer}");
        assert_eq!(corpus["state"]["state"], "refused", "{answer}");
        // Refused before anything ran: no compile, no index.
        assert!(
            corpus["detail"]
                .as_str()
                .unwrap()
                .contains("does not carry `check`"),
            "{answer}"
        );
    }
}
