//! Spec 014's ordered admission and its section 3.5 requirement, against
//! portable fixtures in the producer's exact shape and a run record reopened
//! from disk. Each section 4 row this crate decides is one test; the rows the
//! envelope or the run record decide alone are tested there, and the
//! renderings are `statecraft-cli`'s.

use serde_json::{Value, json};
use statecraft_acceptance::wire::{Candidate, admit, eligibility, judge};
use statecraft_envelope::dimensions::{Integrity, Signature, SubjectBinding};
use statecraft_envelope::hash::sha256_hex;
use statecraft_envelope::wire::fixture;
use statecraft_envelope::wire::manifest::Completeness;
use statecraft_envelope::wire::policy::{DimensionName, PermittedAbsence, Requirement, WirePolicy};
use statecraft_envelope::wire::record::{AdmissionRecord, Binding, Custody, Handoff};
use statecraft_envelope::wire::screen::{
    Category, DEFAULT_DETECTOR, DefaultDetector, ScanFailure, SecretDetector,
};
use statecraft_envelope::wire::{EVIDENCE_TYPE, Outcome, Reason, Stated, Subject};
use statecraft_run::record::{Chain, EffectId, Entry, Identity, Kind};
use statecraft_run::wire::{SIDECAR_SUBJECT, Target, admissions, record_policy};

const RUN: &str = "run-014";
const TS: &str = "2026-10-18T00:00:00Z";
const REGISTERED: &str = "registered-value-0001";

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

fn target(attempt: u32) -> Target {
    Target {
        run_id: RUN.into(),
        attempt,
    }
}

fn put(chain: &mut Chain, kind: Kind, attempt: u32, subject: &str, detail: Value, eff: bool) {
    let id = format!("r{}", chain.records().len());
    let entry = Entry {
        kind,
        run_id: RUN.into(),
        attempt,
        subject: subject.into(),
        idempotency_key: None,
        detail,
        effect_id: if eff {
            Identity::Valid(EffectId::new(&effect(attempt)).unwrap())
        } else {
            Identity::Absent
        },
    };
    chain.append(&id, TS, &entry).unwrap();
}

/// An attempt whose sidecar bracket closed on `capture`, written in spec
/// 013's wire shape, with the producer revision it recorded.
fn witnessed(chain: &mut Chain, attempt: u32, capture: &[u8], revision: &str) {
    put(
        chain,
        Kind::Intent,
        attempt,
        "prepare-workspace",
        json!({ "baseCommit": "b" }),
        false,
    );
    put(
        chain,
        Kind::Intent,
        attempt,
        SIDECAR_SUBJECT,
        json!({
            "binding": { "runId": RUN, "attempt": attempt, "effectId": effect(attempt) },
            "producer": {
                "name": "wire-witness",
                "version": revision,
                "executableDigest": fixture::producer_digest(),
            }
        }),
        true,
    );
    put(
        chain,
        Kind::Outcome,
        attempt,
        SIDECAR_SUBJECT,
        json!({
            "effectId": effect(attempt),
            "captureDigest": format!("sha256:{}", sha256_hex(capture)),
        }),
        true,
    );
}

/// The attempt's own outcome, as spec 013 writes it.
fn concluded(chain: &mut Chain, attempt: u32, posture: &str, required: bool) {
    put(
        chain,
        Kind::Outcome,
        attempt,
        "attempt",
        json!({ "outcome": "completed", "witnessPosture": posture, "witnessRequired": required }),
        false,
    );
}

fn custody() -> Custody {
    Custody {
        obtained_from: Stated::known("sidecar-area/wire-exchange.json".into()),
        writer: Stated::known("wire-witness@0.1.0".into()),
        reader: Stated::known("statecraft".into()),
        handoff: Handoff::Complete,
    }
}

fn optional() -> WirePolicy {
    WirePolicy {
        secret_detector: Some(DEFAULT_DETECTOR.into()),
        requirement: None,
    }
}

fn requiring(minimum: Completeness, dims: Vec<DimensionName>) -> WirePolicy {
    WirePolicy {
        secret_detector: Some(DEFAULT_DETECTOR.into()),
        requirement: Some(Requirement {
            artifact_type: EVIDENCE_TYPE.into(),
            minimum_completeness: minimum,
            required_dimensions: dims,
            permitted_absence: Vec::new(),
        }),
    }
}

fn detector() -> DefaultDetector {
    DefaultDetector::with_registered([REGISTERED.to_string()])
}

/// One scene: a run whose attempt 1 was witnessed over `manifest`, with the
/// policy recorded, judged once.
struct Scene {
    homes: Homes,
    manifest: Vec<u8>,
    reference: Vec<u8>,
}

impl Scene {
    fn new(manifest: &Value, policy: Option<&WirePolicy>) -> Self {
        let bytes = fixture::bytes(manifest);
        let reference = fixture::bytes(&fixture::reference(&bytes, &subject(1)));
        Self::with(bytes, reference, policy, fixture::REVISION)
    }

    fn with(
        manifest: Vec<u8>,
        reference: Vec<u8>,
        policy: Option<&WirePolicy>,
        revision: &str,
    ) -> Self {
        let homes = Homes::new();
        let mut c = homes.open();
        if let Some(p) = policy {
            record_policy(&mut c, &target(1), p, TS).unwrap();
        }
        witnessed(&mut c, 1, &manifest, revision);
        Scene {
            homes,
            manifest,
            reference,
        }
    }

    fn admit(&self, attempt: u32) -> AdmissionRecord {
        self.admit_with(attempt, Some(&detector()))
    }

    fn admit_with(&self, attempt: u32, d: Option<&dyn SecretDetector>) -> AdmissionRecord {
        let mut c = self.homes.open();
        let candidate = Candidate {
            reference: &self.reference,
            manifest: &self.manifest,
            custody: custody(),
        };
        let record = admit(&mut c, &target(attempt), &candidate, d, TS).unwrap();
        consistent_binding(&record);
        record
    }

    fn records(&self) -> Vec<AdmissionRecord> {
        admissions(&self.homes.open(), RUN)
            .into_iter()
            .filter_map(|p| p.record)
            .collect()
    }
}

/// Every record's binding, subject-binding dimension and reason agree: a
/// bound subject passes, a binding refusal fails, and an unjudged one is
/// `unknown` under an earlier step's reason.
fn consistent_binding(r: &AdmissionRecord) {
    let expected = match r.binding {
        Binding::Bound => SubjectBinding::Pass,
        Binding::NotEvaluated => SubjectBinding::Unknown,
        Binding::SubjectMismatch | Binding::ProducerMismatch | Binding::Stale => {
            SubjectBinding::Fail
        }
    };
    assert_eq!(r.dimensions.subject_binding, expected, "{r:?}");
    let binding_reason = matches!(
        r.reason,
        Reason::SubjectMismatch | Reason::ProducerMismatch | Reason::Stale
    );
    let refused_at_binding = matches!(
        r.binding,
        Binding::SubjectMismatch | Binding::ProducerMismatch | Binding::Stale
    );
    assert_eq!(binding_reason, refused_at_binding, "{r:?}");
}

fn base() -> Value {
    fixture::manifest(&subject(1))
}

#[test]
fn an_exact_complete_artifact_is_admitted_only_when_the_policy_permits_every_dimension() {
    let s = Scene::new(&base(), Some(&optional()));
    let r = s.admit(1);
    assert_eq!(
        (r.outcome, r.reason.clone()),
        (Outcome::Admit, Reason::Satisfied)
    );
    assert_eq!(r.binding, Binding::Bound);
    assert_eq!(r.dimensions.integrity, Integrity::Pass);
    assert_eq!(r.dimensions.subject_binding, SubjectBinding::Pass);
    assert_eq!(r.dimensions.signature, Signature::Unsigned);
    assert!(r.facts.value().is_some());
    assert_eq!(r.policy, optional().identity());

    let both = vec![DimensionName::Integrity, DimensionName::SubjectBinding];
    let s = Scene::new(&base(), Some(&requiring(Completeness::Complete, both)));
    assert_eq!(s.admit(1).outcome, Outcome::Admit);

    // The producer signs nothing, so a policy requiring a signature refuses.
    let signed = requiring(Completeness::Complete, vec![DimensionName::Signature]);
    let s = Scene::new(&base(), Some(&signed));
    let r = s.admit(1);
    assert_eq!(
        (r.outcome, r.reason),
        (Outcome::Refuse, Reason::PolicyUnsatisfied)
    );
}

#[test]
fn an_optional_artifact_absent_is_not_recorded_and_changes_nothing() {
    let h = Homes::new();
    let mut c = h.open();
    put(
        &mut c,
        Kind::Intent,
        1,
        "prepare-workspace",
        json!({}),
        false,
    );
    concluded(&mut c, 1, "witness-not-requested", false);
    let e = eligibility(&h.open(), &target(1));
    assert!(!e.required);
    assert_eq!(e.state, "not-recorded");
    assert!(e.eligible());
}

#[test]
fn a_required_artifact_absent_makes_acceptance_ineligible() {
    let h = Homes::new();
    let mut c = h.open();
    put(
        &mut c,
        Kind::Intent,
        1,
        "prepare-workspace",
        json!({}),
        false,
    );
    concluded(&mut c, 1, "witness-unavailable", true);
    let e = eligibility(&h.open(), &target(1));
    assert!(e.required);
    assert_eq!(e.state, "unavailable");
    assert_eq!(e.ineligible, Some(Reason::PolicyUnsatisfied));

    // A requirement that permits that absence is satisfied by it.
    let h = Homes::new();
    let mut c = h.open();
    let mut p = requiring(Completeness::Complete, Vec::new());
    p.requirement.as_mut().unwrap().permitted_absence = vec![PermittedAbsence::Unavailable];
    record_policy(&mut c, &target(1), &p, TS).unwrap();
    concluded(&mut c, 1, "witness-unavailable", true);
    assert!(eligibility(&h.open(), &target(1)).eligible());
}

#[test]
fn a_judgment_the_run_could_not_make_reads_unavailable_not_absent() {
    let h = Homes::new();
    let mut c = h.open();
    put(
        &mut c,
        Kind::Intent,
        1,
        "prepare-workspace",
        json!({}),
        false,
    );
    put(
        &mut c,
        Kind::Outcome,
        1,
        "attempt",
        json!({
            "outcome": "completed",
            "witnessPosture": "witnessed",
            "witnessRequired": false,
            "witness": { "wireEvidence": { "unavailable": "manifest: not found" } }
        }),
        false,
    );
    let e = eligibility(&h.open(), &target(1));
    assert_eq!((e.state, e.eligible()), ("unavailable", true));
}

#[test]
fn a_required_artifact_admitted_is_eligible_and_a_refused_one_names_its_reason() {
    let s = Scene::new(
        &base(),
        Some(&requiring(Completeness::Complete, Vec::new())),
    );
    s.admit(1);
    concluded(&mut s.homes.open(), 1, "witnessed", true);
    let e = eligibility(&s.homes.open(), &target(1));
    assert_eq!((e.state, e.eligible()), ("admitted", true));

    let mut m = base();
    m["completeness"] = json!({ "gaps": ["stream-truncated"], "state": "incomplete" });
    let s = Scene::new(&m, Some(&requiring(Completeness::Complete, Vec::new())));
    s.admit(1);
    concluded(&mut s.homes.open(), 1, "witness-incomplete", true);
    let e = eligibility(&s.homes.open(), &target(1));
    assert_eq!(e.state, "refused");
    assert_eq!(e.ineligible, Some(Reason::Incomplete));
}

#[test]
fn a_wrong_run_attempt_or_effect_is_a_subject_mismatch() {
    for wrong in [
        Subject {
            run_id: "another-run".into(),
            ..subject(1)
        },
        Subject {
            sidecar_effect_id: "another-effect".into(),
            ..subject(1)
        },
        Subject {
            attempt: 3,
            ..subject(1)
        },
    ] {
        let bytes = fixture::bytes(&fixture::manifest(&wrong));
        let reference = fixture::bytes(&fixture::reference(&bytes, &wrong));
        let s = Scene::with(bytes, reference, Some(&optional()), fixture::REVISION);
        let r = s.admit(1);
        assert_eq!(
            (r.outcome, r.reason.clone()),
            (Outcome::Refuse, Reason::SubjectMismatch)
        );
        assert_eq!(r.dimensions.subject_binding, SubjectBinding::Fail);
        assert_eq!(r.facts, Stated::NotRecorded);
    }
}

#[test]
fn a_wrong_producer_revision_or_digest_is_a_producer_mismatch() {
    let bytes = fixture::bytes(&base());
    let reference = fixture::bytes(&fixture::reference(&bytes, &subject(1)));
    let s = Scene::with(bytes.clone(), reference, Some(&optional()), "0.1.1");
    assert_eq!(s.admit(1).reason, Reason::ProducerMismatch);

    let mut r = fixture::reference(&bytes, &subject(1));
    r["producer_digest"] = json!(format!("sha256:{}", "ef".repeat(32)));
    let s = Scene::with(
        bytes,
        fixture::bytes(&r),
        Some(&optional()),
        fixture::REVISION,
    );
    assert_eq!(s.admit(1).reason, Reason::ProducerMismatch);
}

#[test]
fn changed_bytes_or_length_fail_integrity() {
    let bytes = fixture::bytes(&base());
    let reference = fixture::bytes(&fixture::reference(&bytes, &subject(1)));
    let mut changed = bytes.clone();
    changed.push(b' ');
    let s = Scene::with(changed, reference, Some(&optional()), fixture::REVISION);
    let r = s.admit(1);
    assert_eq!(r.reason, Reason::DigestMismatch);
    assert_eq!(r.dimensions.integrity, Integrity::Fail);
}

#[test]
fn a_duplicate_keeps_both_records_and_names_the_prior() {
    let s = Scene::new(&base(), Some(&optional()));
    s.admit(1);
    let again = s.admit(1);
    assert_eq!(
        (again.outcome, again.reason.clone()),
        (Outcome::Refuse, Reason::Duplicate)
    );
    let positioned = admissions(&s.homes.open(), RUN);
    assert_eq!(positioned.len(), 2);
    let prior = again.prior.unwrap();
    assert_eq!(prior.record_digest, positioned[0].record_digest);
    assert_eq!(prior.reason, Reason::Satisfied);
    // The duplicate copies no candidate content.
    assert_eq!(again.facts, Stated::NotRecorded);
}

#[test]
fn an_earlier_attempts_artifact_is_stale_and_retry_does_not_adopt_it() {
    let s = Scene::new(&base(), Some(&optional()));
    let mut c = s.homes.open();
    put(
        &mut c,
        Kind::Intent,
        2,
        "prepare-workspace",
        json!({}),
        false,
    );
    let r = s.admit(2);
    assert_eq!((r.outcome, r.reason), (Outcome::Refuse, Reason::Stale));
    concluded(&mut s.homes.open(), 2, "witnessed", true);
    let e = eligibility(&s.homes.open(), &target(2));
    assert_eq!(e.ineligible, Some(Reason::Stale));
}

#[test]
fn a_retry_under_a_changed_policy_begins_no_replacement() {
    // Judged first under the historical empty policy, which names no
    // detector and so admits nothing; then a policy is recorded.
    let s = Scene::new(&base(), None);
    assert_eq!(s.admit(1).reason, Reason::ProhibitedContent);
    record_policy(&mut s.homes.open(), &target(1), &optional(), TS).unwrap();
    let r = s.admit(1);
    assert_eq!(r.reason, Reason::PolicyMismatch);
    assert_eq!(r.prior.unwrap().reason, Reason::ProhibitedContent);

    // A second policy for the attempt is ambiguous; none is chosen.
    record_policy(&mut s.homes.open(), &target(1), &optional(), TS).unwrap();
    let r = s.admit(1);
    assert_eq!(
        (r.reason, r.policy.as_str()),
        (Reason::PolicyMismatch, "ambiguous")
    );
}

#[test]
fn malformed_oversized_nested_or_duplicate_key_input_is_a_bounded_refusal() {
    let duplicate = br#"{"artifactType":"a","artifactType":"b"}"#.to_vec();
    let nested = format!("{}1{}", "[".repeat(40), "]".repeat(40)).into_bytes();
    let mut long_string = b"\"".to_vec();
    long_string.extend(std::iter::repeat_n(b'a', 65_537));
    long_string.push(b'"');
    for bytes in [duplicate, nested, long_string] {
        let reference = fixture::bytes(&fixture::reference(&bytes, &subject(1)));
        let s = Scene::with(bytes, reference, Some(&optional()), fixture::REVISION);
        let r = s.admit(1);
        assert_eq!(r.reason, Reason::Malformed);
        assert!(r.malformed.unwrap().starts_with("manifest: "));
    }
    // A manifest over the bound refuses on its reference's length, before
    // the bytes are read.
    let mut oversized = b"\"".to_vec();
    oversized.extend(std::iter::repeat_n(b'a', 1_048_577));
    oversized.push(b'"');
    let over = fixture::reference(&oversized, &subject(1));
    let s = Scene::with(oversized, fixture::bytes(&over), Some(&optional()), "0.1.0");
    let r = s.admit(1);
    assert_eq!(r.reason, Reason::Malformed);
    assert!(r.malformed.unwrap().starts_with("reference: "));

    let s = Scene::with(
        fixture::bytes(&base()),
        b"{\"sha256\":".to_vec(),
        Some(&optional()),
        fixture::REVISION,
    );
    let r = s.admit(1);
    assert_eq!(
        (r.reason, r.artifact),
        (Reason::Malformed, Stated::Unavailable)
    );
}

fn token_shaped() -> String {
    format!("ghp_{}", "A1b2C3d4E5f6G7h8I9j0".repeat(2))
}

#[test]
fn prohibited_content_refuses_and_is_never_retained() {
    let mut body = base();
    body["exchanges"][0]["body"] = json!("raw request body");
    let mut token = base();
    token["hostPolicyId"] = json!(token_shaped());
    let mut registered = base();
    registered["exchanges"][0]["served"]["destination"] = json!(REGISTERED);
    for (m, category) in [
        (body, Category::OutsideSchema),
        (token, Category::Secret),
        (registered, Category::Secret),
    ] {
        let s = Scene::new(&m, Some(&optional()));
        let r = s.admit(1);
        assert_eq!(r.reason, Reason::ProhibitedContent);
        assert_eq!(r.findings[0].category, category);
        assert_eq!(r.facts, Stated::NotRecorded);
        no_secret_survives(&s);
    }
}

struct Failing;

impl SecretDetector for Failing {
    fn identity(&self) -> &str {
        DEFAULT_DETECTOR
    }
    fn scan(&self, _: &str) -> Result<Option<String>, ScanFailure> {
        Err(ScanFailure)
    }
}

#[test]
fn an_absent_detector_or_a_scan_that_cannot_complete_fails_closed() {
    let none = WirePolicy::default();
    let s = Scene::new(&base(), Some(&none));
    let r = s.admit(1);
    assert_eq!(r.reason, Reason::ProhibitedContent);
    assert_eq!(r.findings[0].category, Category::DetectorMissing);

    let s = Scene::new(&base(), Some(&optional()));
    assert_eq!(
        s.admit_with(1, None).findings[0].category,
        Category::DetectorUnavailable
    );
    let s = Scene::new(&base(), Some(&optional()));
    let r = s.admit_with(1, Some(&Failing));
    assert_eq!(r.reason, Reason::ProhibitedContent);
    assert_eq!(r.findings[0].category, Category::ScanIncomplete);
}

#[test]
fn an_unknown_schema_or_construction_is_unsupported_and_not_guessed_at() {
    let bytes = fixture::bytes(&base());
    for (member, value) in [
        ("schema_version", json!(2)),
        ("construction", json!("canonical-json-sha256")),
        ("evidence_type", json!("statecraft/wire-exchange/v2")),
    ] {
        let mut r = fixture::reference(&bytes, &subject(1));
        r[member] = value;
        let s = Scene::with(
            bytes.clone(),
            fixture::bytes(&r),
            Some(&optional()),
            "0.1.0",
        );
        let rec = s.admit(1);
        assert_eq!(rec.reason, Reason::UnsupportedArtifact);
        assert_eq!(rec.dimensions.integrity, Integrity::Unknown);
    }
}

#[test]
fn an_incomplete_capture_shows_its_gaps_and_follows_the_minimum() {
    let mut m = base();
    m["completeness"] = json!({ "gaps": ["stream-truncated"], "state": "incomplete" });
    let s = Scene::new(&m, Some(&requiring(Completeness::Complete, Vec::new())));
    let r = s.admit(1);
    assert_eq!(r.reason, Reason::Incomplete);
    let facts = r.facts.value().unwrap();
    assert_eq!(
        facts.completeness.gaps,
        vec!["stream-truncated".to_string()]
    );

    let s = Scene::new(&m, Some(&requiring(Completeness::Incomplete, Vec::new())));
    assert_eq!(s.admit(1).outcome, Outcome::Admit);
}

#[test]
fn redaction_that_removed_a_required_fact_is_incomplete_and_both_stay_visible() {
    let mut m = base();
    m["completeness"] = json!({ "gaps": ["redacted-required-fact"], "state": "incomplete" });
    let s = Scene::new(&m, Some(&requiring(Completeness::Complete, Vec::new())));
    let r = s.admit(1);
    assert_eq!(r.reason, Reason::Incomplete);
    let facts = r.facts.value().unwrap();
    assert_eq!(
        facts.redaction.result.clone(),
        statecraft_envelope::wire::manifest::Redaction::Applied
    );
    assert_eq!(
        facts.completeness.gaps,
        vec!["redacted-required-fact".to_string()]
    );
}

#[test]
fn encrypted_content_is_retention_forbidden_and_enters_no_custody() {
    let mut m = base();
    m["retention"] =
        json!({ "reason": null, "requested": "encrypted-content", "result": "encrypted-content" });
    let s = Scene::new(&m, Some(&optional()));
    let r = s.admit(1);
    assert_eq!(r.reason, Reason::RetentionForbidden);
    assert_eq!(r.facts, Stated::NotRecorded);
}

#[test]
fn a_redaction_or_retention_failure_stays_visible_and_policy_refuses_when_required() {
    let mut m = base();
    m["redaction"] = json!({ "fieldClasses": [], "reason": "detector-error", "result": "failed" });
    let s = Scene::new(&m, Some(&optional()));
    let r = s.admit(1);
    assert_eq!(r.outcome, Outcome::Admit);
    let s = Scene::new(&m, Some(&requiring(Completeness::Complete, Vec::new())));
    let r = s.admit(1);
    assert_eq!(r.reason, Reason::RedactionFailed);
    assert_eq!(
        r.facts.value().unwrap().redaction.reason,
        Stated::known("detector-error".into())
    );

    let mut m = base();
    m["retention"] =
        json!({ "reason": "disk-full", "requested": "metadata-only", "result": "failed" });
    let s = Scene::new(&m, Some(&requiring(Completeness::Complete, Vec::new())));
    assert_eq!(s.admit(1).reason, Reason::PolicyUnsatisfied);
}

#[test]
fn an_interruption_before_judgment_records_nothing_and_the_same_inputs_resume() {
    let s = Scene::new(&base(), Some(&optional()));
    let candidate = Candidate {
        reference: &s.reference,
        manifest: &s.manifest,
        custody: custody(),
    };
    // Judged, and then interrupted before the append.
    let _ = judge(&s.homes.open(), &target(1), &candidate, Some(&detector()));
    assert!(s.records().is_empty());
    assert_eq!(s.admit(1).outcome, Outcome::Admit);
}

#[test]
fn a_historical_run_and_policy_read_as_not_recorded() {
    let h = Homes::new();
    let mut c = h.open();
    put(
        &mut c,
        Kind::Intent,
        1,
        "prepare-workspace",
        json!({}),
        false,
    );
    put(
        &mut c,
        Kind::Outcome,
        1,
        "attempt",
        json!({ "outcome": "completed" }),
        false,
    );
    let e = eligibility(&h.open(), &target(1));
    assert_eq!(
        (e.required, e.state, e.eligible()),
        (false, "not-recorded", true)
    );
}

/// Section 4's closing scan: nothing retained carries a secret value, a
/// credential, a token or a raw body.
fn no_secret_survives(s: &Scene) {
    let retained: String = admissions(&s.homes.open(), RUN)
        .iter()
        .map(|p| serde_json::to_string(&p.record).unwrap())
        .collect();
    for needle in [token_shaped().as_str(), REGISTERED, "raw request body"] {
        assert!(!retained.contains(needle), "retained {needle}");
    }
}

#[test]
fn every_retained_record_across_the_matrix_is_free_of_secrets() {
    let mut m = base();
    m["exchanges"][0]["served"]["model"] = json!({ "state": "known", "value": token_shaped() });
    let s = Scene::new(&m, Some(&optional()));
    s.admit(1);
    s.admit(1);
    no_secret_survives(&s);
    assert_eq!(s.records().len(), 2);
}
