//! The contract an attempt is bound to, through the built binary: bound by
//! `run`, compared by `accept`.
//!
//! Spec 003 section 3.1.3 and spec 005 section 3.18. The producer is a stub
//! that answers `registry closure` from files this test writes, in the shapes
//! the unreleased spec-spine `3b67b63d` was measured answering on 2026-09-22:
//! a digest and members on success, exit 1 naming an unresolved member, exit
//! 2 for a stale ledger. A second stub answers `registry closure --help` with
//! the pinned 0.20.0's usage code, 3, as that version was measured doing. No
//! stub claims a producer release carries anything.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn git(at: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "maintenance.auto=false", "-c", "gc.auto=0"])
        .args(args)
        .current_dir(at)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}");
}

fn executable(path: &Path, script: &str) {
    statecraft_adapter::fixture::install_script(path, script, 0o755).unwrap();
}

struct Fixture {
    target: tempfile::TempDir,
    home: tempfile::TempDir,
    bin: tempfile::TempDir,
}

const EXPANSION: &str = r#"#!/bin/sh
here="$(dirname "$0")"
case "$*" in
  --version) echo 'spec-spine 0.22.0' ;;
  check) exit 0 ;;
  'registry plan --json') echo '{"ready":[{"id":"107-x","status":"approved","title":"with obligations"}]}' ;;
  'registry list --json') echo '{"items":[{"id":"107-x","status":"approved","implementation":"pending","obligations":[{"id":"R-1","kind":"requirement","text":"t","anchor":"a"},{"id":"R-2","kind":"requirement","text":"u","anchor":"a","withdrawn":true}]}]}' ;;
  'registry closure --help') exit 0 ;;
  'registry closure --request - --json')
    /bin/cat > "$here/closure-request"
    code="$(/bin/cat "$here/closure-exit" 2>/dev/null || echo 0)"
    if [ "$code" = 0 ]; then /bin/cat "$here/closure-answer"; else echo "spec-spine: $(/bin/cat "$here/closure-stderr")" >&2; fi
    exit "$code" ;;
  *) exit 3 ;;
esac
"#;

const RELEASED: &str = r#"#!/bin/sh
case "$*" in
  --version) echo 'spec-spine 0.20.0' ;;
  check) exit 0 ;;
  'registry plan --json') echo '{"ready":[{"id":"107-x","title":"with obligations"}]}' ;;
  'registry list --json') echo '{"items":[{"id":"107-x","status":"approved","implementation":"pending"}]}' ;;
  'registry closure --help') echo "error: unrecognized subcommand 'closure'" >&2; exit 3 ;;
  *) exit 3 ;;
esac
"#;

const PROVIDER: &str = r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.267'; exit 0; fi
/bin/cat > /dev/null
/bin/cat "$(dirname "$0")/native.jsonl"
"#;

fn answer(digest: &str, r1_text: &str, r2_withdrawn: bool) -> String {
    serde_json::json!({
        "digest": digest,
        "members": [
            {"kind": "spec", "spec": "107-x", "contentHash": format!("hash-of-{digest}")},
            {"kind": "obligation", "spec": "107-x", "id": "R-1", "obligationKind": "requirement",
             "text": r1_text, "anchor": "a", "sectionDigest": "s1"},
            {"kind": "obligation", "spec": "107-x", "id": "R-2", "obligationKind": "requirement",
             "text": "u", "anchor": "a", "sectionDigest": "s1", "withdrawn": r2_withdrawn}
        ],
        "schemaVersion": "0.6.0"
    })
    .to_string()
}

impl Fixture {
    fn new(producer: &str) -> Self {
        let f = Self {
            target: tempfile::tempdir().unwrap(),
            home: tempfile::tempdir().unwrap(),
            bin: tempfile::tempdir().unwrap(),
        };
        let t = f.target.path();
        git(t, &["init", "--quiet"]);
        git(t, &["config", "user.name", "fixture"]);
        git(t, &["config", "user.email", "fixture@example.com"]);
        std::fs::write(t.join("source.txt"), "base").unwrap();
        git(t, &["add", "source.txt"]);
        git(t, &["commit", "--quiet", "-m", "fixture base"]);
        executable(&f.bin.path().join("spec-spine"), producer);
        executable(&f.bin.path().join("claude"), PROVIDER);
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../statecraft-adapter-claude-code/testdata/stream/success.jsonl"),
            f.bin.path().join("native.jsonl"),
        )
        .unwrap();
        // Bound as the stub's list declares: R-2 already withdrawn.
        f.closure(&answer("d1", "t", true), 0, "");
        let root = f.root();
        assert!(
            f.cli(&["project", "register", &root])
                .status
                .code()
                .unwrap()
                <= 1
        );
        assert_eq!(f.cli(&["project", "arm", &root]).status.code(), Some(0));
        f
    }

    fn root(&self) -> String {
        self.target.path().display().to_string()
    }

    fn bin(&self) -> PathBuf {
        self.bin.path().to_path_buf()
    }

    fn closure(&self, answer: &str, exit: i32, stderr: &str) {
        std::fs::write(self.bin().join("closure-answer"), answer).unwrap();
        std::fs::write(self.bin().join("closure-exit"), exit.to_string()).unwrap();
        std::fs::write(self.bin().join("closure-stderr"), stderr).unwrap();
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", self.home.path())
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin().display()))
            .env("USER", "fixture-operator")
            .output()
            .unwrap()
    }

    fn json(&self, args: &[&str]) -> (i32, serde_json::Value) {
        let out = self.cli(args);
        let v = serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)));
        (out.status.code().unwrap(), v)
    }

    fn run(&self) -> (i32, serde_json::Value) {
        self.json(&["run", &self.root(), "107-x", "--json"])
    }

    fn accept(&self) -> (i32, serde_json::Value) {
        self.json(&["accept", &self.root(), "107-x", "--json"])
    }
}

/// `run` asks for the spec and every obligation it declares, withdrawn ones
/// included, and records the producer's answer verbatim in the intent and in
/// its answer.
#[test]
fn run_binds_the_spec_and_every_declared_obligation_before_any_effect() {
    let f = Fixture::new(EXPANSION);
    let (code, v) = f.run();
    assert_eq!(code, 0, "{v}");
    let contract = &v["value"]["contract"];
    assert_eq!(contract["state"], "bound", "{v}");
    assert_eq!(contract["digest"], "d1");
    assert_eq!(contract["producer"], "0.22.0");
    assert_eq!(contract["members"].as_array().unwrap().len(), 3);
    let request: serde_json::Value =
        serde_json::from_slice(&std::fs::read(f.bin().join("closure-request")).unwrap()).unwrap();
    assert_eq!(
        request,
        serde_json::json!({"specs": ["107-x"], "obligations": ["107-x#R-1", "107-x#R-2"]})
    );
    assert!(
        v["summary"]
            .as_str()
            .unwrap()
            .contains("contract  bound d1: 3 member(s)")
    );
}

/// The same answer at `accept` is `current`, and the acceptance is judged as
/// it would have been, with the comparison beside it. This bare fixture
/// declares no authority-set path, so the acceptance is its usual
/// `policy-digest-uncomputable`; what matters is that the contract did not
/// decide it.
#[test]
fn an_unchanged_contract_is_current_and_changes_nothing_about_the_acceptance() {
    let f = Fixture::new(EXPANSION);
    assert_eq!(f.run().0, 0);
    let (_, v) = f.accept();
    assert_eq!(v["value"]["contract"]["word"], "current", "{v}");
    assert_eq!(v["value"]["contract"]["boundDigest"], "d1");
    assert_eq!(
        v["value"]["reason"]["reason"], "policy-digest-uncomputable",
        "{v}"
    );
}

/// An obligation's text moved: no acceptance, `contract-moved`, naming the
/// member with both identities. The run record keeps what it was bound to.
#[test]
fn a_changed_obligation_is_no_acceptance_naming_it() {
    let f = Fixture::new(EXPANSION);
    assert_eq!(f.run().0, 0);
    f.closure(&answer("d2", "t, amended", true), 0, "");
    let (code, v) = f.accept();
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["value"]["acceptance"], "none");
    assert_eq!(v["value"]["reason"]["reason"], "contract-moved");
    let c = &v["value"]["contract"];
    assert_eq!(c["word"], "changed");
    assert_eq!(c["boundDigest"], "d1");
    assert_eq!(c["nowDigest"], "d2");
    let members: Vec<&str> = c["changed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["member"].as_str().unwrap())
        .collect();
    assert!(members.contains(&"obligation:107-x#R-1"), "{members:?}");
    // Never rewritten: a second `run` shows attempt 1 still bound to d1.
    let summary = v["summary"].as_str().unwrap();
    assert!(
        summary.contains("changed   obligation:107-x#R-1"),
        "{summary}"
    );
}

/// An obligation withdrawn since binding: `withdrawn`, and no acceptance.
#[test]
fn an_obligation_withdrawn_since_binding_is_no_acceptance() {
    let f = Fixture::new(EXPANSION);
    // R-2 was already withdrawn when bound, so only R-1 is newly withdrawn.
    assert_eq!(f.run().0, 0);
    let mut now: serde_json::Value = serde_json::from_str(&answer("d3", "t", true)).unwrap();
    now["members"][1]["withdrawn"] = serde_json::json!(true);
    f.closure(&now.to_string(), 0, "");
    let (code, v) = f.accept();
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["value"]["reason"]["reason"], "contract-moved");
    assert_eq!(v["value"]["contract"]["word"], "withdrawn");
    assert_eq!(
        v["value"]["contract"]["withdrawn"],
        serde_json::json!(["obligation:107-x#R-1"])
    );
}

/// A member that no longer resolves is `missing`; a stale ledger is refused
/// before anything is judged.
#[test]
fn a_missing_member_is_no_acceptance_and_a_stale_ledger_is_refused() {
    let f = Fixture::new(EXPANSION);
    assert_eq!(f.run().0, 0);
    f.closure("", 1, "not found: obligation '107-x#R-1'");
    let (code, v) = f.accept();
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["value"]["contract"]["word"], "missing");
    assert!(
        v["value"]["contract"]["reason"]
            .as_str()
            .unwrap()
            .contains("107-x#R-1")
    );
    f.closure("", 2, "registry is stale");
    let (code, v) = f.accept();
    assert_eq!(code, 2, "{v}");
    assert_eq!(v["value"]["word"], "stale");
}

/// The pinned producer has no closure verb: `run` records `unsupported`,
/// naming the version that answered and asserting nothing about a release,
/// and `accept` reads the contract as `not-recorded` and proceeds.
#[test]
fn a_producer_without_closures_binds_unsupported_and_accept_reads_not_recorded() {
    let f = Fixture::new(RELEASED);
    let (code, v) = f.run();
    assert_eq!(code, 0, "{v}");
    let contract = &v["value"]["contract"];
    assert_eq!(contract["state"], "unsupported", "{v}");
    let detail = contract["detail"].as_str().unwrap();
    assert!(detail.contains("spec-spine 0.20.0 was asked"), "{detail}");
    assert!(!detail.contains("release"), "{detail}");
    let (_, v) = f.accept();
    assert_eq!(v["value"]["contract"]["word"], "not-recorded", "{v}");
    assert_eq!(
        v["value"]["reason"]["reason"], "policy-digest-uncomputable",
        "{v}"
    );
}
