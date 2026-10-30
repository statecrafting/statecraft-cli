//! Spec 019 through the binary: `run --context` composes, records and
//! delivers a manifest; `run show` renders it from the record; `accept`
//! judges it under the recorded policy (sections 3.2, 3.5, 3.6, 3.8, 3.9).
//!
//! The provider is a fake that reads its input and replays a recorded
//! stream. The producer's `context` verbs are the real pinned executable,
//! over a clone of this repository's own corpus; every other spec-spine read
//! the run makes is the same fixed fake the other run tests use.

#![cfg(unix)]

#[path = "support/json_naming.rs"]
mod json_naming;

#[path = "support/provider_token.rs"]
mod provider_token;

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const RUN: &str = "replay";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

fn real_spec_spine() -> PathBuf {
    let local = repo_root().join(".bin/spec-spine");
    assert!(
        local.is_file(),
        "no .bin/spec-spine; run `make tools`. This suite runs the real pinned tool."
    );
    local
}

fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(path)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

struct Fixture {
    target: tempfile::TempDir,
    home: tempfile::TempDir,
    bin: tempfile::TempDir,
    operator: tempfile::TempDir,
    corpus: tempfile::TempDir,
}

fn fixture() -> Fixture {
    let f = Fixture {
        target: tempfile::tempdir().unwrap(),
        home: tempfile::tempdir().unwrap(),
        bin: tempfile::tempdir().unwrap(),
        operator: tempfile::tempdir().unwrap(),
        corpus: tempfile::tempdir().unwrap(),
    };
    let t = f.target.path();
    git(t, &["init", "--quiet"]);
    git(t, &["config", "user.name", "fixture"]);
    git(t, &["config", "user.email", "fixture@example.com"]);
    std::fs::write(t.join("source.txt"), "base").unwrap();
    git(t, &["add", "source.txt"]);
    git(t, &["commit", "--quiet", "-m", "fixture base"]);

    let url = format!("file://{}", repo_root().display());
    let out = Command::new("git")
        .args(["clone", "--quiet", "--depth", "1", &url])
        .arg(f.corpus.path().join("corpus"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");

    let spine = format!(
        r#"#!/bin/sh
case "$1" in context) exec '{}' "$@" ;; esac
case "$*" in
  --version) echo 'spec-spine {}' ;;
  check|'check --help') exit 0 ;;
  'registry plan --json') echo '{{"ready":[{{"id":"replay","title":"recorded stream"}}]}}' ;;
  'registry list --json') echo '{{"items":[{{"id":"replay","status":"approved","implementation":"pending"}}]}}' ;;
  'verify '*' --plan --json') printf '{{"exitCode":0,"ok":true,"report":{{"commands":[],"skipped":[],"specId":"%s"}},"schemaVersion":"0.6.0","verb":"verify"}}' $2 ;;
  *) exit 3 ;;
esac
"#,
        real_spec_spine().display(),
        // The stub answers as the repository's own pinned engine, whichever
        // release D-06 last adopted.
        statecraft_home::setup::exact_pin(
            &std::fs::read_to_string(repo_root().join("spec-spine.toml")).unwrap()
        )
        .unwrap()
    );
    statecraft_adapter::fixture::install_script(&f.bin.path().join("spec-spine"), spine, 0o755)
        .unwrap();
    let stream = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../statecraft-adapter-claude-code/testdata/stream/success.jsonl");
    std::fs::copy(stream, f.bin.path().join("native.jsonl")).unwrap();
    statecraft_adapter::fixture::install_script(
        &f.bin.path().join("claude"),
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.267'; exit 0; fi
/bin/cat > /dev/null
/bin/cat "$(dirname "$0")/native.jsonl"
"#,
        0o755,
    )
    .unwrap();
    provider_token::with_token(f.home.path());
    f
}

/// The answer's value: `report` on success, `error.details` otherwise.
fn body(value: &Value) -> &Value {
    match value.get("report") {
        Some(r) => r,
        None => &value["error"]["details"],
    }
}

fn policy(need: &str, permitted: Value) -> Value {
    json!({
        "schema": "statecraft/context-acceptance-policy/1",
        "delivery": need, "integrity": need, "closure": need,
        "freshness": need, "citations": need, "evidence": need,
        "permittedAbsences": permitted,
    })
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

    fn root(&self) -> &str {
        self.target.path().to_str().unwrap()
    }

    fn corpus(&self) -> PathBuf {
        self.corpus.path().join("corpus")
    }

    fn register(&self) {
        let registered = self.run(&["project", "register", self.root()]);
        assert!(registered.status.code().unwrap() <= 1, "{registered:?}");
        let armed = self.run(&["project", "arm", self.root()]);
        assert_eq!(armed.status.code(), Some(0), "{armed:?}");
    }

    /// A task file binding one required repository: this corpus's spec 019.
    fn task(&self, extra_member: Option<Value>, policy: Value) -> PathBuf {
        let mut members = vec![json!({
            "selector": { "kind": "spec", "spec": "019-context-packet-consumption" },
            "requirement": "required"
        })];
        members.extend(extra_member);
        let file = self.corpus.path().join("task.json");
        std::fs::write(
            &file,
            serde_json::to_vec(&json!({
                "optionalOmission": "refuse",
                "policy": policy,
                "repositories": [{
                    "id": "statecraft",
                    "role": "source",
                    "requirement": "required",
                    "path": "corpus",
                    "repository": "example/statecraft",
                    "delivery": "required",
                    "request": {
                        "closure": { "root": "019-context-packet-consumption" },
                        "members": members,
                        "consumerSchemaVersion": "1.1.0"
                    }
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        file
    }

    fn accept_file(&self, citations: Value, evidence: Value) -> PathBuf {
        let file = self.corpus.path().join("accept.json");
        std::fs::write(
            &file,
            serde_json::to_vec(&json!({
                "checkouts": { "statecraft": { "path": "corpus", "repository": "example/statecraft" } },
                "citations": citations,
                "evidence": evidence,
            }))
            .unwrap(),
        )
        .unwrap();
        file
    }

    fn show(&self) -> Value {
        let out = self.run(&["run", "show", self.root(), RUN, "--json"]);
        assert_eq!(out.status.code(), Some(0), "{out:?}");
        let human = self.run(&["run", "show", self.root(), RUN]);
        assert_eq!(human.status.code(), Some(0), "{human:?}");
        json_naming::from_output(&out.stdout).unwrap()
    }
}

#[test]
fn a_run_records_delivers_and_shows_its_manifest_and_accept_judges_it() {
    let f = fixture();
    f.register();
    let task = f.task(None, policy("required", json!([])));
    let out = f.run(&[
        "run",
        f.root(),
        RUN,
        "--context",
        task.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");

    let shown = f.show();
    let ctx = &shown["report"]["contextManifest"][0];
    assert_eq!(ctx["attempt"], 1);
    assert_eq!(ctx["state"], "recorded", "{ctx}");
    let head = git(&f.corpus(), &["rev-parse", "HEAD"]);
    assert_eq!(
        ctx["manifest"]["repositories"][0]["snapshot"]["revision"],
        head
    );
    let observed = ctx["delivery"]["observed"].as_array().unwrap();
    assert!(!observed.is_empty());
    assert!(
        observed.iter().all(|p| p["state"] == "reported-visible"),
        "{observed:?}"
    );
    assert_eq!(ctx["delivery"]["transport"], "not-recorded");

    // A cited member the packet carries resolves; with evidence admitted and
    // a clean current checkout, every context fact holds.
    let page: Value = {
        let digest = ctx["manifest"]["repositories"][0]["pages"][0]["custodyDigest"]
            .as_str()
            .unwrap()
            .trim_start_matches("sha256:")
            .to_string();
        let bytes =
            std::fs::read(f.home.path().join("context").join(format!("{digest}.json"))).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    };
    let member = &page["members"][0];
    let citations = json!([{
        "repository": "statecraft",
        "pageDigest": page["packetDigest"],
        "member": member["identity"],
        "contentDigest": member["item"]["digest"],
    }]);
    let evidence = json!([{ "reference": "evidence-1", "decision": "admit" }]);
    let accepted = f.run(&[
        "accept",
        f.root(),
        RUN,
        "--context",
        f.accept_file(citations.clone(), evidence.clone())
            .to_str()
            .unwrap(),
        "--json",
    ]);
    let value: Value = json_naming::from_output(&accepted.stdout).unwrap();
    let judged = &body(&value)["contextManifest"];
    assert_eq!(judged["state"], "judged", "{value}");
    assert_eq!(judged["eligible"], true, "{judged}");
    assert_eq!(judged["closureAggregate"], "current");
    assert_eq!(judged["freshnessAggregate"], "fresh");
    assert_eq!(judged["citations"][0]["state"], "resolved");

    // A dirty current checkout leaves freshness unverified, and acceptance is
    // ineligible with that reason; the attempt outcome is not rewritten.
    std::fs::write(f.corpus().join("README.md"), "dirty\n").unwrap();
    let refused = f.run(&[
        "accept",
        f.root(),
        RUN,
        "--context",
        f.accept_file(citations, evidence).to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(refused.status.code(), Some(1), "{refused:?}");
    let value: Value = json_naming::from_output(&refused.stdout).unwrap();
    let reasons = body(&value)["reason"]["reasons"].as_array().unwrap();
    assert!(
        reasons
            .iter()
            .any(|r| r == "context-freshness-unverified: statecraft"),
        "{value}"
    );
    assert_eq!(
        f.show()["report"]["contextManifest"][0]["state"],
        "recorded"
    );
}

#[test]
fn a_required_omission_refuses_before_any_attempt_is_appended() {
    let f = fixture();
    f.register();
    let task = f.task(
        Some(json!({
            "selector": { "kind": "spec", "spec": "999-absent" },
            "requirement": "required"
        })),
        policy("required", json!([])),
    );
    let out = f.run(&[
        "run",
        f.root(),
        RUN,
        "--context",
        task.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let value: Value = json_naming::from_output(&out.stdout).unwrap();
    let details = &value["error"]["details"];
    assert_eq!(details["reason"], "context-ineligible", "{value}");
    assert!(
        details["detail"]
            .as_str()
            .unwrap()
            .contains("required-incomplete")
    );
    // Nothing was launched: the record holds no attempt intent at all.
    let (chain, _) = statecraft_run::record::Chain::open(f.home.path(), f.target.path()).unwrap();
    assert!(
        !chain
            .entries()
            .iter()
            .any(|e| e.kind == statecraft_run::record::Kind::Intent
                && e.subject == statecraft_run::session::INTENT_SUBJECT),
        "no attempt intent was recorded"
    );
    let shown = f.run(&["run", "show", f.root(), RUN, "--json"]);
    assert_ne!(shown.status.code(), Some(0), "no run was appended");
}

#[test]
fn a_historical_run_shows_and_accepts_not_recorded() {
    let f = fixture();
    f.register();
    let out = f.run(&["run", f.root(), RUN, "--json"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let shown = f.show();
    assert_eq!(
        shown["report"]["contextManifest"],
        json!([{ "attempt": 1, "state": "not-recorded" }])
    );
    let accepted = f.run(&["accept", f.root(), RUN, "--json"]);
    let value: Value = json_naming::from_output(&accepted.stdout).unwrap();
    assert_eq!(
        body(&value)["contextManifest"],
        json!({ "state": "not-recorded" })
    );
}
