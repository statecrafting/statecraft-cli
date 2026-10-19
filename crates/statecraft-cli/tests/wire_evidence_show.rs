//! Spec 014 through the binary: `run show`'s additive wire-evidence member
//! (section 3.7) in both renderings, and `accept`'s section 3.5 requirement.
//!
//! A real run is made with a fake provider, and its witness evidence is then
//! written through the owning crates' own APIs in spec 013's wire shape, so
//! no producer, network or provider is involved. Every rendering is scanned
//! for the secret values the fixtures planted.

#![cfg(unix)]

#[path = "support/json_naming.rs"]
mod json_naming;

#[path = "support/provider_token.rs"]
mod provider_token;

use serde_json::{Value, json};
use statecraft_acceptance::wire::{Candidate, admit};
use statecraft_envelope::hash::sha256_hex;
use statecraft_envelope::wire::fixture;
use statecraft_envelope::wire::policy::WirePolicy;
use statecraft_envelope::wire::record::{Custody, Handoff};
use statecraft_envelope::wire::screen::{DEFAULT_DETECTOR, DefaultDetector};
use statecraft_envelope::wire::{Stated, Subject};
use statecraft_run::record::{Chain, EffectId, Entry, Identity, Kind};
use statecraft_run::wire::{ADMISSION_SUBJECT, SIDECAR_SUBJECT, Target, record_policy};
use std::path::Path;
use std::process::{Command, Output};

const RUN: &str = "replay";
const TS: &str = "2026-10-18T00:00:00Z";

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

fn fixture() -> Fixture {
    let f = Fixture {
        target: tempfile::tempdir().unwrap(),
        home: tempfile::tempdir().unwrap(),
        bin: tempfile::tempdir().unwrap(),
        operator: tempfile::tempdir().unwrap(),
    };
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
    executable(
        &f.bin.path().join("claude"),
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.267'; exit 0; fi
/bin/cat > /dev/null
/bin/cat "$(dirname "$0")/native.jsonl"
"#,
    );
    provider_token::with_token(f.home.path());
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

    fn root(&self) -> &str {
        self.target.path().to_str().unwrap()
    }

    /// Registered, armed, and one unwitnessed attempt run.
    fn ran(&self, flags: &[&str]) {
        let registered = self.run(&["project", "register", self.root()]);
        assert!(registered.status.code().unwrap() <= 1, "{registered:?}");
        let armed = self.run(&["project", "arm", self.root()]);
        assert_eq!(armed.status.code(), Some(0), "{armed:?}");
        let mut args = vec!["run", self.root(), RUN];
        args.extend_from_slice(flags);
        args.push("--json");
        let output = self.run(&args);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
    }

    fn chain(&self) -> Chain {
        Chain::open(self.home.path(), self.target.path()).unwrap().0
    }

    fn show(&self) -> (Value, String) {
        let json = self.run(&["run", "show", self.root(), RUN, "--json"]);
        assert_eq!(json.status.code(), Some(0), "{json:?}");
        let human = self.run(&["run", "show", self.root(), RUN]);
        assert_eq!(human.status.code(), Some(0), "{human:?}");
        (
            json_naming::from_output(&json.stdout).unwrap(),
            String::from_utf8(human.stdout).unwrap(),
        )
    }
}

fn subject() -> Subject {
    Subject {
        run_id: RUN.into(),
        attempt: 1,
        sidecar_effect_id: format!("{RUN}/1/witness-sidecar"),
    }
}

fn put(chain: &mut Chain, kind: Kind, subject: &str, detail: Value, effect: bool) {
    let id = format!("wire-{}", chain.records().len());
    let entry = Entry {
        kind,
        run_id: RUN.into(),
        attempt: 1,
        subject: subject.into(),
        idempotency_key: None,
        detail,
        effect_id: if effect {
            Identity::Valid(EffectId::new(&format!("{RUN}/1/witness-sidecar")).unwrap())
        } else {
            Identity::Absent
        },
    };
    chain.append(&id, TS, &entry).unwrap();
}

/// Spec 013's bracket for attempt 1, closed on `manifest`, and the policy.
fn witnessed(chain: &mut Chain, manifest: &[u8]) {
    let target = Target {
        run_id: RUN.into(),
        attempt: 1,
    };
    let policy = WirePolicy {
        secret_detector: Some(DEFAULT_DETECTOR.into()),
        requirement: None,
    };
    record_policy(chain, &target, &policy, TS).unwrap();
    put(
        chain,
        Kind::Intent,
        SIDECAR_SUBJECT,
        json!({
            "binding": { "runId": RUN, "attempt": 1, "effectId": subject().sidecar_effect_id },
            "producer": {
                "name": "wire-witness",
                "version": fixture::REVISION,
                "executableDigest": fixture::producer_digest(),
            }
        }),
        true,
    );
    put(
        chain,
        Kind::Outcome,
        SIDECAR_SUBJECT,
        json!({
            "effectId": subject().sidecar_effect_id,
            "captureDigest": format!("sha256:{}", sha256_hex(manifest)),
        }),
        true,
    );
}

fn judge(chain: &mut Chain, manifest: &[u8]) {
    let reference = fixture::bytes(&fixture::reference(manifest, &subject()));
    let candidate = Candidate {
        reference: &reference,
        manifest,
        custody: Custody {
            obtained_from: Stated::known("witness/capture/wire-exchange.json".into()),
            writer: Stated::known("wire-witness@0.1.0".into()),
            reader: Stated::known("statecraft-cli".into()),
            handoff: Handoff::Complete,
        },
    };
    let target = Target {
        run_id: RUN.into(),
        attempt: 1,
    };
    admit(
        chain,
        &target,
        &candidate,
        Some(&DefaultDetector::default()),
        TS,
    )
    .unwrap();
}

fn token_shaped() -> String {
    format!("ghp_{}", "A1b2C3d4E5f6G7h8I9j0".repeat(2))
}

fn assert_no_secret(json: &Value, human: &str) {
    let text = format!("{json}{human}");
    assert!(!text.contains(&token_shaped()), "a token was rendered");
    assert!(
        !text.contains("raw request body"),
        "a raw body was rendered"
    );
}

fn with_estimate() -> Value {
    let mut m = fixture::manifest(&subject());
    m["exchanges"][0]["estimatedCost"] = json!({
        "currency": "USD",
        "inputs": ["usage:input_tokens"],
        "label": "estimate",
        "method": "price-table/2026-10",
        "value": 0.0001
    });
    m
}

#[test]
fn an_admitted_artifact_and_its_duplicate_show_every_fact_in_both_renderings() {
    let f = fixture();
    f.ran(&[]);
    let manifest = fixture::bytes(&with_estimate());
    let mut chain = f.chain();
    witnessed(&mut chain, &manifest);
    judge(&mut chain, &manifest);
    judge(&mut chain, &manifest);
    let (json, human) = f.show();

    let attempt = &json["report"]["wireEvidence"]["attempts"][0];
    assert_eq!(attempt["attempt"], 1);
    assert_eq!(attempt["state"], "admitted");
    assert_eq!(attempt["required"], false);
    let admissions = attempt["admissions"].as_array().unwrap();
    assert_eq!(admissions.len(), 2, "both observations stay visible");
    let first = &admissions[0]["record"];
    assert_eq!(first["outcome"], "admit");
    assert_eq!(first["reason"], "satisfied");
    assert_eq!(first["binding"], "bound");
    let artifact = &first["artifact"]["value"];
    assert_eq!(artifact["evidenceType"], "statecraft/wire-exchange/v1");
    assert_eq!(artifact["byteLength"], manifest.len());
    assert_eq!(artifact["manifestDigest"], sha256_hex(&manifest));
    assert_eq!(
        artifact["subject"]["value"]["sidecarEffectId"],
        subject().sidecar_effect_id
    );
    let facts = &first["facts"]["value"];
    let exchange = &facts["exchanges"][0];
    assert_ne!(exchange["requested"], exchange["served"]);
    assert_eq!(exchange["usage"]["label"], "provider-testimony");
    assert_eq!(exchange["estimatedCost"]["value"]["label"], "estimate");
    assert_eq!(facts["attestation"]["identity"]["state"], "unknown");
    assert_eq!(first["dimensions"]["integrity"], "pass");
    let duplicate = &admissions[1]["record"];
    assert_eq!(duplicate["reason"], "duplicate");
    assert_eq!(
        duplicate["prior"]["recordDigest"],
        admissions[0]["recordDigest"]
    );
    assert_eq!(duplicate["prior"]["reason"], "satisfied");

    for line in [
        "attempt 1 wire evidence admitted",
        "admit satisfied under policy sha256:",
        "refuse duplicate",
        "prior record ",
        "artifact statecraft/wire-exchange/v1 schema 1 construction file-bytes-sha256",
        "producer wire-witness 0.1.0 sha256:",
        "binding bound",
        "completeness complete gaps none",
        "retention metadata-only",
        "redaction applied",
        "requested api.provider.example:443 http/1.1 model model-requested; served api.provider.example:443 http/1.1 model model-served",
        "usage (provider-testimony) input_tokens 3 tokens",
        "estimated cost (estimate) 0.0001 USD method price-table/2026-10",
        "attestation unknown",
        "dimensions integrity pass signature unsigned issuerTrust unknown subjectBinding pass",
        "custody recorded",
    ] {
        assert!(human.contains(line), "missing {line:?} in\n{human}");
    }
    assert_no_secret(&json, &human);
}

#[test]
fn a_refusal_names_its_finding_and_renders_no_candidate_value() {
    let f = fixture();
    f.ran(&[]);
    let mut m = fixture::manifest(&subject());
    m["hostPolicyId"] = json!(token_shaped());
    m["exchanges"][0]["body"] = json!("raw request body");
    let manifest = fixture::bytes(&m);
    let mut chain = f.chain();
    witnessed(&mut chain, &manifest);
    judge(&mut chain, &manifest);
    let (json, human) = f.show();
    let attempt = &json["report"]["wireEvidence"]["attempts"][0];
    assert_eq!(attempt["state"], "refused");
    assert_eq!(
        attempt["admissions"][0]["record"]["reason"],
        "prohibited-content"
    );
    assert_eq!(
        attempt["admissions"][0]["record"]["facts"]["state"],
        "not-recorded"
    );
    assert!(human.contains("refuse prohibited-content"), "{human}");
    assert!(human.contains("facts not-recorded"), "{human}");
    assert!(human.contains("finding outside-schema at "), "{human}");
    assert_no_secret(&json, &human);
}

#[test]
fn a_malformed_record_is_a_finding_with_its_position_and_suppresses_nothing() {
    let f = fixture();
    f.ran(&[]);
    let manifest = fixture::bytes(&fixture::manifest(&subject()));
    let mut chain = f.chain();
    witnessed(&mut chain, &manifest);
    put(
        &mut chain,
        Kind::Evidence,
        ADMISSION_SUBJECT,
        json!({ "schema": "not-a-record" }),
        false,
    );
    let position = chain.records().len() - 1;
    judge(&mut chain, &manifest);
    let (json, human) = f.show();
    let wire = &json["report"]["wireEvidence"];
    assert_eq!(wire["malformed"][0]["position"], position);
    assert_eq!(wire["attempts"][0]["state"], "admitted");
    assert!(
        human.contains(&format!(
            "attempt 1 wire evidence record at position {position} is malformed"
        )),
        "{human}"
    );
}

#[test]
fn a_historical_run_shows_not_recorded_and_everything_else_unchanged() {
    let f = fixture();
    f.ran(&[]);
    let (json, human) = f.show();
    let attempt = &json["report"]["wireEvidence"]["attempts"][0];
    assert_eq!(attempt["state"], "not-recorded");
    assert_eq!(attempt["required"], false);
    assert!(attempt["ineligible"].is_null());
    assert_eq!(
        json["report"]["witness"][0]["posture"],
        "witness-not-requested"
    );
    assert!(
        human.contains("attempt 1 wire evidence not-recorded\n"),
        "{human}"
    );
}

#[test]
fn required_evidence_that_is_absent_makes_accept_ineligible_without_rewriting_the_outcome() {
    let f = fixture();
    f.ran(&["--require-witness"]);
    let (json, human) = f.show();
    let attempt = &json["report"]["wireEvidence"]["attempts"][0];
    assert_eq!(attempt["state"], "unavailable");
    assert_eq!(attempt["required"], true);
    assert_eq!(attempt["ineligible"], "policy-unsatisfied");
    assert!(
        human.contains(
            "attempt 1 wire evidence unavailable, required, acceptance ineligible: policy-unsatisfied"
        ),
        "{human}"
    );

    let accepted = f.run(&["accept", f.root(), RUN, "--json"]);
    assert_eq!(accepted.status.code(), Some(1), "{accepted:?}");
    let answer: Value = json_naming::from_output(&accepted.stdout).unwrap();
    let report = &answer["report"];
    assert_eq!(report["acceptance"], "none", "{answer}");
    assert_eq!(report["reason"]["reason"], "wire-evidence-ineligible");
    assert_eq!(report["reason"]["state"], "unavailable");
    assert_eq!(report["reason"]["evidence"], "policy-unsatisfied");
    // The attempt's outcome is what it was.
    let (after, _) = f.show();
    assert_eq!(
        json["report"]["independentResult"],
        after["report"]["independentResult"]
    );
}
