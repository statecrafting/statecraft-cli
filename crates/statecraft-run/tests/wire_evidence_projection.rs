//! Spec 014's run-record half: step 4's subject and producer binding, the
//! attempt's recorded policy, the append-only admission records, and the
//! additive per-attempt projection `run show` renders. Every test reads a
//! chain reopened from disk and nothing else.
//!
//! The sidecar bracket is written here in spec 013's wire shape (subject
//! `witness-sidecar`, an intent carrying `binding` and `producer`, an outcome
//! carrying `captureDigest`), so these tests need no adapter.

use serde_json::{Value, json};
use statecraft_envelope::dimensions::{Dimensions, Integrity, SubjectBinding};
use statecraft_envelope::wire::fixture;
use statecraft_envelope::wire::policy::WirePolicy;
use statecraft_envelope::wire::record::{
    AdmissionRecord, Artifact, Binding, Custody, Handoff, RECORD_SCHEMA,
};
use statecraft_envelope::wire::{Outcome, Producer, Reason, Stated, Subject};
use statecraft_run::record::{Chain, EffectId, Entry, Identity, Kind};
use statecraft_run::wire::{
    self, Earlier, RecordedPolicy, SIDECAR_SUBJECT, Target, append_admission, bind, prior, project,
    record_policy, recorded_policy,
};

const RUN: &str = "run-014";
const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct Homes {
    home: tempfile::TempDir,
    target: tempfile::TempDir,
}

impl Homes {
    fn new() -> Self {
        Homes {
            home: tempfile::tempdir().unwrap(),
            target: tempfile::tempdir().unwrap(),
        }
    }
    fn open(&self) -> Chain {
        Chain::open(self.home.path(), self.target.path()).unwrap().0
    }
}

fn effect(attempt: u32) -> String {
    format!("{RUN}/{attempt}/witness-sidecar")
}

fn subject(attempt: u32) -> Subject {
    Subject {
        run_id: RUN.into(),
        attempt,
        sidecar_effect_id: effect(attempt),
    }
}

fn entry(kind: Kind, attempt: u32, subject: &str, detail: Value, effect: Option<&str>) -> Entry {
    Entry {
        kind,
        run_id: RUN.into(),
        attempt,
        subject: subject.into(),
        idempotency_key: None,
        detail,
        effect_id: effect.map_or(Identity::Absent, |e| {
            Identity::Valid(EffectId::new(e).unwrap())
        }),
    }
}

fn put(chain: &mut Chain, e: &Entry) {
    let id = format!("r{}", chain.records().len());
    chain.append(&id, "2026-10-17T00:00:00Z", e).unwrap();
}

/// The attempt's own intent, which is what makes it an attempt.
fn attempt(chain: &mut Chain, n: u32) {
    put(
        chain,
        &entry(
            Kind::Intent,
            n,
            "prepare-workspace",
            json!({ "baseCommit": "b" }),
            None,
        ),
    );
}

fn sidecar_intent(attempt: u32) -> Entry {
    entry(
        Kind::Intent,
        attempt,
        SIDECAR_SUBJECT,
        json!({
            "binding": { "runId": RUN, "attempt": attempt, "effectId": effect(attempt) },
            "producer": {
                "name": "wire-witness",
                "version": fixture::REVISION,
                "interface": "wire-witness.sidecar/1",
                "target": "test-target",
                "executableDigest": fixture::producer_digest(),
                "qualification": "qualification/0.1.0/test.md",
                "path": "/bin/true"
            }
        }),
        Some(&effect(attempt)),
    )
}

fn sidecar_outcome(attempt: u32, capture: Value) -> Entry {
    entry(
        Kind::Outcome,
        attempt,
        SIDECAR_SUBJECT,
        json!({ "effectId": effect(attempt), "captureDigest": capture }),
        Some(&effect(attempt)),
    )
}

/// A closed sidecar bracket for `n`, naming the fixture's capture digest.
fn bracket(chain: &mut Chain, n: u32) {
    put(chain, &sidecar_intent(n));
    put(
        chain,
        &sidecar_outcome(n, json!(format!("sha256:{DIGEST}"))),
    );
}

fn producer() -> Producer {
    Producer {
        name: Stated::known("wire-witness".into()),
        revision: Stated::known(fixture::REVISION.into()),
        content_digest: Stated::known(fixture::producer_digest()),
    }
}

fn target(attempt: u32) -> Target {
    Target {
        run_id: RUN.into(),
        attempt,
    }
}

fn judged(attempt: u32) -> Binding {
    let h = Homes::new();
    let mut c = h.open();
    attempt_with_bracket(&mut c, attempt);
    bind(
        &h.open(),
        &target(attempt),
        &Stated::known(subject(attempt)),
        &producer(),
        DIGEST,
    )
}

fn attempt_with_bracket(c: &mut Chain, n: u32) {
    attempt(c, n);
    bracket(c, n);
}

#[test]
fn the_exact_effect_of_the_attempt_binds() {
    assert_eq!(judged(1), Binding::Bound);
}

#[test]
fn a_wrong_run_attempt_or_effect_is_a_subject_mismatch() {
    let h = Homes::new();
    let mut c = h.open();
    attempt_with_bracket(&mut c, 1);
    let c = h.open();
    let mut wrong_run = subject(1);
    wrong_run.run_id = "another-run".into();
    let mut wrong_effect = subject(1);
    wrong_effect.sidecar_effect_id = "another-effect".into();
    let mut wrong_attempt = subject(1);
    wrong_attempt.attempt = 3;
    for s in [wrong_run, wrong_effect, wrong_attempt] {
        assert_eq!(
            bind(
                &c,
                &target(1),
                &Stated::known(s.clone()),
                &producer(),
                DIGEST
            ),
            Binding::SubjectMismatch,
            "{s:?}"
        );
    }
    assert_eq!(
        bind(&c, &target(1), &Stated::Unknown, &producer(), DIGEST),
        Binding::SubjectMismatch
    );
}

#[test]
fn an_unclosed_doubly_closed_or_ambiguous_effect_does_not_bind() {
    let cases: [&dyn Fn(&mut Chain); 3] = [
        &|c| put(c, &sidecar_intent(1)),
        &|c| {
            bracket(c, 1);
            put(c, &sidecar_outcome(1, json!(null)));
        },
        &|c| {
            put(c, &sidecar_intent(1));
            bracket(c, 1);
        },
    ];
    for write in cases {
        let h = Homes::new();
        let mut c = h.open();
        attempt(&mut c, 1);
        write(&mut c);
        assert_eq!(
            bind(
                &h.open(),
                &target(1),
                &Stated::known(subject(1)),
                &producer(),
                DIGEST
            ),
            Binding::SubjectMismatch
        );
    }
}

#[test]
fn an_outcome_closing_another_capture_does_not_bind_this_one() {
    let h = Homes::new();
    let mut c = h.open();
    attempt(&mut c, 1);
    put(&mut c, &sidecar_intent(1));
    put(
        &mut c,
        &sidecar_outcome(1, json!(format!("sha256:{}", "f".repeat(64)))),
    );
    assert_eq!(
        bind(
            &h.open(),
            &target(1),
            &Stated::known(subject(1)),
            &producer(),
            DIGEST
        ),
        Binding::SubjectMismatch
    );
}

#[test]
fn an_earlier_attempts_artifact_is_stale_for_the_current_attempt() {
    let h = Homes::new();
    let mut c = h.open();
    attempt_with_bracket(&mut c, 1);
    attempt_with_bracket(&mut c, 2);
    let c = h.open();
    assert_eq!(
        bind(
            &c,
            &target(2),
            &Stated::known(subject(1)),
            &producer(),
            DIGEST
        ),
        Binding::Stale
    );
    assert_eq!(
        bind(
            &c,
            &target(2),
            &Stated::known(subject(2)),
            &producer(),
            DIGEST
        ),
        Binding::Bound
    );
}

#[test]
fn a_missing_or_unequal_producer_identity_is_a_producer_mismatch() {
    let h = Homes::new();
    let mut c = h.open();
    attempt_with_bracket(&mut c, 1);
    let c = h.open();
    let mut revision = producer();
    revision.revision = Stated::known("0.0.9".into());
    let mut digest = producer();
    digest.content_digest = Stated::Absent;
    let mut name = producer();
    name.name = Stated::known("wire-witness-compatible".into());
    for p in [revision, digest, name] {
        assert_eq!(
            bind(&c, &target(1), &Stated::known(subject(1)), &p, DIGEST),
            Binding::ProducerMismatch
        );
    }
}

#[test]
fn a_recorded_policy_is_one_and_a_second_makes_it_ambiguous() {
    let h = Homes::new();
    let mut c = h.open();
    attempt(&mut c, 1);
    assert_eq!(
        recorded_policy(&h.open(), &target(1)),
        RecordedPolicy::NotRecorded
    );
    assert_eq!(
        RecordedPolicy::NotRecorded.policy(),
        Some(WirePolicy::default())
    );
    let p = WirePolicy {
        secret_detector: Some("d".into()),
        requirement: None,
    };
    record_policy(&mut c, &target(1), &p, "t").unwrap();
    assert_eq!(
        recorded_policy(&h.open(), &target(1)),
        RecordedPolicy::One(p.clone())
    );
    record_policy(&mut c, &target(1), &WirePolicy::default(), "t").unwrap();
    assert_eq!(
        recorded_policy(&h.open(), &target(1)),
        RecordedPolicy::Ambiguous
    );
    assert_eq!(RecordedPolicy::Ambiguous.policy(), None);
}

fn record(attempt: u32, outcome: Outcome, reason: Reason, policy: &str) -> AdmissionRecord {
    let s = subject(attempt);
    AdmissionRecord {
        schema: RECORD_SCHEMA.into(),
        artifact: Stated::known(Artifact {
            evidence_type: statecraft_envelope::wire::EVIDENCE_TYPE.into(),
            schema_version: 1,
            construction: statecraft_envelope::wire::CONSTRUCTION.into(),
            manifest_digest: DIGEST.into(),
            byte_length: 10,
            producer: producer(),
            subject: Stated::known(s),
            completeness: Stated::known("complete".into()),
        }),
        binding: Binding::Bound,
        facts: Stated::NotRecorded,
        custody: Custody {
            obtained_from: Stated::known("attempt-directory/wire-exchange.json".into()),
            writer: Stated::known("wire-witness@0.1.0".into()),
            reader: Stated::known("statecraft-cli".into()),
            handoff: Handoff::Complete,
        },
        dimensions: Dimensions::unsigned_today(Integrity::Pass, SubjectBinding::Pass),
        outcome,
        reason,
        policy: policy.into(),
        prior: None,
        findings: Vec::new(),
        malformed: None,
    }
}

#[test]
fn step_six_names_a_duplicate_or_a_changed_policy_by_the_prior_record() {
    let h = Homes::new();
    let mut c = h.open();
    attempt_with_bracket(&mut c, 1);
    let s = Stated::known(subject(1));
    assert_eq!(
        prior(&h.open(), &target(1), DIGEST, "p1", &s),
        Earlier::None
    );
    append_admission(
        &mut c,
        &target(1),
        &record(1, Outcome::Admit, Reason::Satisfied, "p1"),
        "t",
    )
    .unwrap();
    let c = h.open();
    let hash = c.records().last().unwrap().record_hash.clone();
    let Earlier::Duplicate(p) = prior(&c, &target(1), DIGEST, "p1", &s) else {
        panic!("a duplicate");
    };
    assert_eq!(
        (p.record_digest.as_str(), p.reason),
        (hash.as_str(), Reason::Satisfied)
    );
    assert!(matches!(
        prior(&c, &target(1), DIGEST, "p2", &s),
        Earlier::OtherPolicy(_)
    ));
    // Another attempt never inherits this one's record.
    assert_eq!(prior(&c, &target(2), DIGEST, "p1", &s), Earlier::None);
}

#[test]
fn a_historical_run_projects_not_recorded_and_its_attempts_are_unchanged() {
    let h = Homes::new();
    let mut c = h.open();
    attempt(&mut c, 1);
    let before = statecraft_run::session::runs(&h.open());
    let p = project(&h.open(), RUN);
    assert_eq!(p.attempts.len(), 1);
    assert_eq!(p.attempts[0].state, "not-recorded");
    assert!(p.malformed.is_empty());
    // Evidence records add no attempt and change no attempt.
    bracket(&mut c, 1);
    record_policy(&mut c, &target(1), &WirePolicy::default(), "t").unwrap();
    append_admission(
        &mut c,
        &target(1),
        &record(1, Outcome::Admit, Reason::Satisfied, "p"),
        "t",
    )
    .unwrap();
    assert_eq!(statecraft_run::session::runs(&h.open()), before);
}

#[test]
fn every_record_projects_and_a_malformed_one_suppresses_nothing() {
    let h = Homes::new();
    let mut c = h.open();
    attempt_with_bracket(&mut c, 1);
    attempt(&mut c, 2);
    append_admission(
        &mut c,
        &target(1),
        &record(1, Outcome::Admit, Reason::Satisfied, "p"),
        "t",
    )
    .unwrap();
    put(
        &mut c,
        &entry(
            Kind::Evidence,
            1,
            wire::ADMISSION_SUBJECT,
            json!({ "not": "a record" }),
            None,
        ),
    );
    let mut duplicate = record(1, Outcome::Refuse, Reason::Duplicate, "p");
    duplicate.prior = Some(statecraft_envelope::wire::record::Prior {
        record_digest: "h".into(),
        reason: Reason::Satisfied,
    });
    append_admission(&mut c, &target(1), &duplicate, "t").unwrap();
    let p = project(&h.open(), RUN);
    assert_eq!(p.attempts.len(), 2);
    let first = &p.attempts[0];
    assert_eq!(first.state, "recorded");
    assert_eq!(first.admissions.len(), 2);
    assert_eq!(
        first.admissions[1].record.as_ref().unwrap().reason,
        Reason::Duplicate
    );
    assert_eq!(p.attempts[1].state, "not-recorded");
    assert_eq!(p.malformed.len(), 1);
    assert_eq!(p.malformed[0].attempt, 1);
    assert_eq!(p.malformed[0].position, first.admissions[0].position + 1);
}
