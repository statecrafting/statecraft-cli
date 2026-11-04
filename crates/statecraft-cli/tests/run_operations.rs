//! Spec 048 at the command surface: `run lease`, `run op` and `run events`.
//!
//! V-1 through the binary: a stale version exits 2 naming `stale-version`
//! and records the refusal, not the effect; a repeated key appends nothing; a
//! caller without the lease is refused `lease-not-held` before any key rule;
//! a reconnecting cursor reads exactly the records after it. The attempts are
//! written into the record directly, so no provider is involved; a retry runs
//! `run`'s own path, which refuses or launches as `run` would, and the
//! operation's bracket closes either way.

#![cfg(unix)]

#[path = "support/json_naming.rs"]
mod json_naming;

#[path = "support/provider_token.rs"]
mod provider_token;

use serde_json::{Value, json};
use statecraft_run::attempt::Outcome;
use statecraft_run::record::{Chain, Entry, Identity, Kind};
use statecraft_run::session::{INTENT_SUBJECT, OUTCOME_SUBJECT};
use std::process::{Command, Output};

const RUN: &str = "fixture";

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
        statecraft_adapter::fixture::install_script(
            &bin.path().join("spec-spine"),
            r#"#!/bin/sh
case "$*" in
  --version) echo 'spec-spine 0.20.0' ;;
  check|'check --help') exit 0 ;;
  'registry plan --json') echo '{"ready":[{"id":"fixture","title":"operations fixture"}]}' ;;
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

    fn chain(&self) -> Chain {
        Chain::open(self.home.path(), self.target.path()).unwrap().0
    }

    fn records(&self) -> usize {
        self.chain().records().len()
    }

    /// Spec 003's attempt records, written as the session writes them.
    fn attempt(&self, attempt: u32, outcome: Option<Outcome>) {
        let mut chain = self.chain();
        let mut put = |kind, subject: &str, detail: Value| {
            let id = format!("fixture/{}", chain.records().len());
            chain
                .append(
                    &id,
                    "2026-11-01T00:00:00Z",
                    &Entry {
                        kind,
                        run_id: RUN.to_string(),
                        attempt,
                        subject: subject.to_string(),
                        idempotency_key: None,
                        detail,
                        effect_id: Identity::Absent,
                    },
                )
                .unwrap();
        };
        put(Kind::Intent, INTENT_SUBJECT, json!({ "baseCommit": "aaa" }));
        if let Some(o) = outcome {
            put(
                Kind::Outcome,
                OUTCOME_SUBJECT,
                json!({ "outcome": o.word() }),
            );
        }
    }

    /// Acquire the lease as controller `ctl`: its id and the run version.
    fn lease(&self, ctl: &str) -> (String, u64) {
        let (exit, v) = self.json(&["run", "lease", self.root(), RUN, "acquire", ctl]);
        assert_eq!(exit, 0, "{v}");
        (
            v["report"]["lease"]["id"].as_u64().unwrap().to_string(),
            v["report"]["version"].as_u64().unwrap(),
        )
    }
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("the process exited normally")
}

/// What an answer carries: its report, or a refusal's details.
fn payload(v: &Value) -> &Value {
    if v.get("report").is_some() {
        &v["report"]
    } else {
        &v["error"]["details"]
    }
}

#[test]
fn a_stale_version_exits_2_naming_stale_version_and_records_only_the_refusal() {
    let f = Fixture::new();
    f.attempt(1, None);
    let (lease, version) = f.lease("cockpit");
    let before = f.records();

    let stale = (version - 1).to_string();
    let args = [
        "run",
        "op",
        f.root(),
        RUN,
        "abort",
        "cockpit",
        lease.as_str(),
        "k1",
        stale.as_str(),
        "stop",
        "it",
    ];
    let (exit, v) = f.json(&args);
    assert_eq!(exit, 2, "{v}");
    assert_eq!(v["error"]["kind"], "refused", "{v}");
    let details = &v["error"]["details"];
    assert_eq!(details["reason"], "stale-version", "{v}");
    assert_eq!(details["expectedVersion"], version - 1, "{v}");
    assert_eq!(details["currentVersion"], version, "{v}");
    assert_eq!(details["recorded"], true, "{v}");
    assert_eq!(f.records(), before + 1);
    assert!(f.chain().entries().iter().all(
        |e| e.kind != Kind::Intent || e.subject != statecraft_run::operation::OPERATION_SUBJECT
    ));

    // The same key and version again: the recorded refusal, nothing appended.
    let (exit, again) = f.json(&args);
    assert_eq!(exit, 2, "{again}");
    assert_eq!(again["error"]["details"]["reason"], "stale-version");
    assert_eq!(f.records(), before + 1);

    // The same key with another version: key-conflict, nothing appended.
    let current = (version + 1).to_string();
    let (exit, conflict) = f.json(&[
        "run",
        "op",
        f.root(),
        RUN,
        "abort",
        "cockpit",
        lease.as_str(),
        "k1",
        current.as_str(),
    ]);
    assert_eq!(exit, 2, "{conflict}");
    assert_eq!(conflict["error"]["details"]["reason"], "key-conflict");
    assert_eq!(f.records(), before + 1);
}

#[test]
fn without_the_lease_the_answer_is_lease_not_held_before_any_key_rule() {
    let f = Fixture::new();
    f.attempt(1, None);
    let (lease, version) = f.lease("cockpit");
    let stale = (version - 1).to_string();
    let (exit, _) = f.json(&[
        "run",
        "op",
        f.root(),
        RUN,
        "abort",
        "cockpit",
        lease.as_str(),
        "k1",
        stale.as_str(),
    ]);
    assert_eq!(exit, 2);
    let before = f.records();
    // A held key, at a stale version, from a controller without the lease.
    let (exit, v) = f.json(&[
        "run",
        "op",
        f.root(),
        RUN,
        "abort",
        "intruder",
        lease.as_str(),
        "k1",
        stale.as_str(),
    ]);
    assert_eq!(exit, 2, "{v}");
    assert_eq!(v["error"]["details"]["reason"], "lease-not-held", "{v}");
    assert_eq!(f.records(), before);
    // A second controller cannot take the lease while it is held.
    let (exit, v) = f.json(&["run", "lease", f.root(), RUN, "acquire", "intruder"]);
    assert_eq!(exit, 2, "{v}");
    assert_eq!(v["error"]["details"]["reason"], "lease-held", "{v}");
}

#[test]
fn an_accepted_abort_is_stop_requested_and_a_repeated_key_appends_nothing() {
    let f = Fixture::new();
    f.attempt(1, None);
    let (lease, version) = f.lease("cockpit");
    let version = version.to_string();
    let args = [
        "run",
        "op",
        f.root(),
        RUN,
        "abort",
        "cockpit",
        lease.as_str(),
        "k1",
        version.as_str(),
        "operator",
        "stop",
    ];
    let (exit, v) = f.json(&args);
    assert_eq!(exit, 0, "{v}");
    assert_eq!(v["report"]["replayed"], false, "{v}");
    assert_eq!(v["report"]["outcome"]["result"], "stop-requested", "{v}");
    let after = f.records();

    let (exit, again) = f.json(&args);
    assert_eq!(exit, 0, "{again}");
    assert_eq!(again["report"]["replayed"], true, "{again}");
    assert_eq!(
        again["report"]["operation"]["outcome"],
        v["report"]["outcome"]
    );
    assert_eq!(again["report"]["operation"]["stop"], "stop-requested");
    assert_eq!(f.records(), after);
}

#[test]
fn a_retry_runs_runs_path_and_its_bracket_closes_whatever_that_path_answers() {
    let f = Fixture::new();
    f.attempt(1, Some(Outcome::Failed));
    let (lease, version) = f.lease("cockpit");
    let version = version.to_string();
    let args = [
        "run",
        "op",
        f.root(),
        RUN,
        "retry",
        "cockpit",
        lease.as_str(),
        "k1",
        version.as_str(),
    ];
    let (exit, v) = f.json(&args);
    let report = payload(&v);
    let outcome = &report["outcome"];
    assert_eq!(outcome["exit"], exit, "{v}");
    let state = statecraft_run::operation::fold(&f.chain(), RUN);
    let launched = state.attempts.len() == 2;
    assert_eq!(
        outcome["result"],
        if launched { "launched" } else { "not-launched" },
        "{v}"
    );
    assert_eq!(state.operations.len(), 1);
    assert!(state.operations[0].outcome.is_some(), "{v}");
    // The operation's own effect is closed exactly once.
    let effects = statecraft_run::fold_effects(&f.chain());
    assert!(
        effects
            .closed()
            .iter()
            .any(|k| k.run_id == RUN && k.effect_id.as_str() == "operation/k1"),
        "{v}"
    );

    let records = f.records();
    let (exit, again) = f.json(&args);
    assert_eq!(exit, 0, "{again}");
    assert_eq!(again["report"]["replayed"], true, "{again}");
    assert_eq!(f.records(), records);
}

#[test]
fn a_reconnecting_cursor_reads_exactly_the_records_after_it() {
    let f = Fixture::new();
    f.attempt(1, Some(Outcome::Failed));
    f.lease("cockpit");
    let (exit, all) = f.json(&["run", "events", f.root(), RUN]);
    assert_eq!(exit, 0, "{all}");
    let events = all["report"]["events"].as_array().unwrap().clone();
    assert_eq!(events.len(), 3, "{all}");
    let cursor = events[0]["cursor"].as_u64().unwrap().to_string();
    let next = all["report"]["next"].as_u64().unwrap().to_string();

    let (_, resumed) = f.json(&["run", "events", f.root(), RUN, "--after", cursor.as_str()]);
    assert_eq!(
        resumed["report"]["events"].as_array().unwrap()[..],
        events[1..]
    );

    f.attempt(2, None);
    let (_, tail) = f.json(&["run", "events", f.root(), RUN, "--after", next.as_str()]);
    let tail_events = tail["report"]["events"].as_array().unwrap();
    assert_eq!(tail_events.len(), 1, "{tail}");
    assert_eq!(tail_events[0]["entry"]["attempt"], 2);

    let (exit, bad) = f.json(&["run", "events", f.root(), RUN, "--after", "x"]);
    assert_eq!(exit, 3, "{bad}");
}
