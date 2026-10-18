//! Spec 014's envelope half, against portable fixtures in the producer's own
//! shape: the bounded reader, the reference, the v1 member set, the
//! prohibited-content screen and step 9's facts. Section 4's rows that this
//! crate can decide alone are each one test here; the rows that need the run
//! record are `statecraft-run`'s and `statecraft-acceptance`'s.

use serde_json::{Value, json};
use statecraft_envelope::wire::bounded::{self, Refusal};
use statecraft_envelope::wire::fixture;
use statecraft_envelope::wire::manifest::{Completeness, Facts, Redaction, Retention, SCHEMA};
use statecraft_envelope::wire::record::consistent;
use statecraft_envelope::wire::reference::WireReference;
use statecraft_envelope::wire::screen::{
    Category, DEFAULT_DETECTOR, DefaultDetector, ScanFailure, SecretDetector, screen,
};
use statecraft_envelope::wire::{Reason, Stated, Subject};

fn subject() -> Subject {
    Subject {
        run_id: "run-014".into(),
        attempt: 1,
        sidecar_effect_id: "run-014/1/witness-sidecar".into(),
    }
}

fn decode(v: &Value) -> bounded::Json {
    bounded::decode(&fixture::bytes(v)).unwrap()
}

fn findings(v: &Value) -> Vec<statecraft_envelope::wire::screen::Finding> {
    let detector = DefaultDetector::with_registered(["registered-value-0001".to_string()]);
    screen(&decode(v), &SCHEMA, Some(DEFAULT_DETECTOR), Some(&detector))
}

/// A synthetic token in a shape the default rules flag. Assembled at run
/// time so no credential-shaped literal sits in the source.
fn token_shaped() -> String {
    format!("ghp_{}", "A1b2C3d4E5f6G7h8I9j0".repeat(2))
}

#[test]
fn the_producers_reference_names_every_section_3_1_identity() {
    let bytes = fixture::bytes(&fixture::manifest(&subject()));
    let r =
        WireReference::decode(&fixture::bytes(&fixture::reference(&bytes, &subject()))).unwrap();
    assert!(r.supported());
    assert_eq!(r.byte_length, bytes.len() as u64);
    assert_eq!(r.producer.revision, Stated::known(fixture::REVISION.into()));
    assert_eq!(
        r.producer.content_digest,
        Stated::known(fixture::producer_digest())
    );
    assert_eq!(r.subject, Stated::known(subject()));
    assert_eq!(r.completeness, Stated::known("complete".into()));
}

#[test]
fn the_producers_manifest_screens_clean_and_reads_every_fact() {
    let m = fixture::manifest(&subject());
    assert_eq!(findings(&m), Vec::new());
    let facts = Facts::read(&decode(&m)).unwrap();
    assert_eq!(facts.completeness.state, Completeness::Complete);
    assert_eq!(facts.retention.result, Retention::MetadataOnly);
    assert_eq!(facts.redaction.result, Redaction::Applied);
    assert_eq!(facts.attestation.identity, Stated::Unknown);
    let reference = WireReference::decode(&fixture::bytes(&fixture::reference(
        &fixture::bytes(&m),
        &subject(),
    )))
    .unwrap();
    assert!(reference.supported());
    assert_eq!(consistent(&facts, &reference), Ok(()));
}

#[test]
fn requested_and_served_identities_stay_separate() {
    let facts = Facts::read(&decode(&fixture::manifest(&subject()))).unwrap();
    let e = &facts.exchanges[0];
    assert_eq!(e.requested.model, Stated::known("model-requested".into()));
    assert_eq!(e.served.model, Stated::known("model-served".into()));
}

#[test]
fn usage_is_testimony_an_estimate_is_labeled_and_an_absent_attester_is_unknown() {
    let mut m = fixture::manifest(&subject());
    m["exchanges"][0]["estimatedCost"] = json!({
        "label": "estimate", "value": 0.0012, "currency": "USD",
        "method": "price-table/2026-10", "inputs": ["usage:input_tokens"]
    });
    assert_eq!(findings(&m), Vec::new());
    let facts = Facts::read(&decode(&m)).unwrap();
    let e = &facts.exchanges[0];
    assert_eq!(e.usage.label, "provider-testimony");
    let estimate = e.estimated_cost.value().unwrap();
    assert_eq!(estimate.label, "estimate");
    assert_eq!(estimate.method, "price-table/2026-10");
    assert_eq!(facts.attestation.identity.word(), "unknown");
}

#[test]
fn every_section_3_1_bound_refuses_while_tokenizing() {
    let deep = format!("{}{}", "[".repeat(33), "]".repeat(33));
    let wide = format!("[{}]", vec!["0"; 1025].join(","));
    let many = format!("[{}]", vec!["[0,0,0,0]"; 900].join(","));
    let name = format!("{{\"{}\":0}}", "n".repeat(129));
    let long = format!("\"{}\"", "s".repeat(65_537));
    let large = vec![b' '; bounded::MAX_BYTES + 1];
    for (bytes, refusal) in [
        (deep.into_bytes(), Refusal::TooDeep),
        (wide.into_bytes(), Refusal::ArrayTooLong),
        (many.into_bytes(), Refusal::TooManyItems),
        (name.into_bytes(), Refusal::NameTooLong),
        (long.into_bytes(), Refusal::StringTooLong),
        (large, Refusal::TooLarge),
        (br#"{"a":1,"a":2}"#.to_vec(), Refusal::DuplicateMember),
        (br#"{"a":{"b":1,"b":1}}"#.to_vec(), Refusal::DuplicateMember),
        (b"{\"a\":".to_vec(), Refusal::Syntax),
    ] {
        assert_eq!(bounded::decode(&bytes), Err(refusal));
    }
    // Exactly at each bound is still readable.
    let edge = format!("{}{}", "[".repeat(32), "]".repeat(32));
    assert!(bounded::decode(edge.as_bytes()).is_ok());
}

#[test]
fn a_hostile_string_is_only_a_string() {
    // A path, a URL and a command are decoded as text and nothing follows them.
    let v =
        bounded::decode(br#"{"a":"../../etc/passwd","b":"file:///x","c":"$(rm -rf /)"}"#).unwrap();
    assert_eq!(
        v.get("a").and_then(bounded::Json::as_str),
        Some("../../etc/passwd")
    );
}

#[test]
fn an_unknown_schema_or_construction_is_unsupported_not_guessed_at() {
    let bytes = fixture::bytes(&fixture::manifest(&subject()));
    for (member, value) in [
        ("evidence_type", json!("statecraft/wire-exchange/v2")),
        ("schema_version", json!(2)),
        ("construction", json!("canonical-record-sha256")),
    ] {
        let mut r = fixture::reference(&bytes, &subject());
        r[member] = value;
        let r = WireReference::decode(&fixture::bytes(&r)).unwrap();
        assert!(!r.supported(), "{member}");
    }
}

#[test]
fn changed_bytes_or_length_fail_integrity() {
    let bytes = fixture::bytes(&fixture::manifest(&subject()));
    let r =
        WireReference::decode(&fixture::bytes(&fixture::reference(&bytes, &subject()))).unwrap();
    assert!(r.as_reference().matches(&bytes));
    let mut changed = bytes.clone();
    changed[10] ^= 1;
    assert!(!r.as_reference().matches(&changed));
    let mut longer = bytes.clone();
    longer.push(b' ');
    assert!(!r.as_reference().matches(&longer));
}

#[test]
fn a_raw_body_or_credential_member_is_outside_the_schema() {
    for (at, member) in [
        ("/exchanges/0", "body"),
        ("/exchanges/0/requested", "headers"),
        ("", "authorization"),
        ("/exchanges/0/served", "set-cookie"),
    ] {
        let mut m = fixture::manifest(&subject());
        m.pointer_mut(at)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(member.into(), json!("x"));
        let f = findings(&m);
        assert_eq!(f.len(), 1, "{member}");
        assert_eq!(f[0].category, Category::OutsideSchema);
        assert!(f[0].path.ends_with(member));
    }
}

#[test]
fn a_registered_secret_or_detector_match_is_found_and_never_copied() {
    let token = token_shaped();
    for value in ["registered-value-0001".to_string(), token.clone()] {
        let mut m = fixture::manifest(&subject());
        m["hostPolicyId"] = json!(format!("prefix {value} suffix"));
        let f = findings(&m);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].category, Category::Secret);
        assert_eq!(f[0].path, "$.hostPolicyId");
        assert!(
            f[0].detector
                .as_deref()
                .unwrap()
                .starts_with(DEFAULT_DETECTOR)
        );
        let written = serde_json::to_string(&f).unwrap();
        assert!(!written.contains(&value));
    }
}

#[test]
fn a_secret_member_name_is_kept_out_of_its_own_path() {
    let token = token_shaped();
    let mut m = fixture::manifest(&subject());
    m.as_object_mut().unwrap().insert(token.clone(), json!(1));
    let f = findings(&m);
    assert!(f.iter().all(|f| f.path == "$.[secret-name]"));
    assert!(f.iter().any(|f| f.category == Category::OutsideSchema));
    assert!(!serde_json::to_string(&f).unwrap().contains(&token));
}

struct Failing;

impl SecretDetector for Failing {
    fn identity(&self) -> &str {
        "failing"
    }
    fn scan(&self, _: &str) -> Result<Option<String>, ScanFailure> {
        Err(ScanFailure)
    }
}

#[test]
fn an_absent_mismatched_or_failing_detector_fails_closed() {
    let m = decode(&fixture::manifest(&subject()));
    let d = DefaultDetector::default();
    let category = |f: Vec<statecraft_envelope::wire::screen::Finding>| f[0].category;
    assert_eq!(
        category(screen(&m, &SCHEMA, None, Some(&d))),
        Category::DetectorMissing
    );
    assert_eq!(
        category(screen(&m, &SCHEMA, Some(DEFAULT_DETECTOR), None)),
        Category::DetectorMissing
    );
    assert_eq!(
        category(screen(&m, &SCHEMA, Some("another-detector"), Some(&d))),
        Category::DetectorMismatch
    );
    let failed = screen(&m, &SCHEMA, Some("failing"), Some(&Failing));
    assert!(!failed.is_empty());
    assert!(
        failed
            .iter()
            .all(|f| f.category == Category::ScanIncomplete)
    );
}

#[test]
fn contradictory_completeness_is_malformed_not_normalized() {
    for completeness in [
        json!({ "gaps": ["deadline-reached"], "state": "complete" }),
        json!({ "gaps": [], "state": "incomplete" }),
        json!({ "gaps": [], "state": "unknown" }),
        json!({ "gaps": ["b", "a"], "state": "incomplete" }),
        json!({ "gaps": ["a", "a"], "state": "incomplete" }),
    ] {
        let mut m = fixture::manifest(&subject());
        m["completeness"] = completeness;
        assert!(Facts::read(&decode(&m)).is_err());
    }
    let mut m = fixture::manifest(&subject());
    m["completeness"] = json!({ "gaps": [], "reason": "terminal-missing", "state": "unknown" });
    assert_eq!(
        Facts::read(&decode(&m)).unwrap().completeness.state,
        Completeness::Unknown
    );
}

#[test]
fn an_incomplete_capture_keeps_every_named_gap() {
    let mut m = fixture::manifest(&subject());
    m["completeness"] =
        json!({ "gaps": ["deadline-reached", "record-missing"], "state": "incomplete" });
    let facts = Facts::read(&decode(&m)).unwrap();
    assert_eq!(
        facts.completeness.gaps,
        ["deadline-reached", "record-missing"]
    );
}

#[test]
fn encrypted_content_and_failures_read_as_their_exact_results() {
    let mut m = fixture::manifest(&subject());
    m["retention"]["result"] = json!("encrypted-content");
    m["redaction"]["result"] = json!("failed");
    m["redaction"]["reason"] = json!("body-unscannable");
    let facts = Facts::read(&decode(&m)).unwrap();
    assert_eq!(facts.retention.result, Retention::EncryptedContent);
    assert_eq!(facts.redaction.result, Redaction::Failed);
    assert_eq!(
        facts.redaction.reason,
        Stated::known("body-unscannable".into())
    );
}

#[test]
fn the_manifest_must_say_what_its_reference_says() {
    let m = fixture::manifest(&subject());
    let bytes = fixture::bytes(&m);
    let facts = Facts::read(&decode(&m)).unwrap();
    let read = |r: Value| WireReference::decode(&fixture::bytes(&r)).unwrap();

    let mut other = fixture::reference(&bytes, &subject());
    other["subject"]["attempt"] = json!(2);
    assert_eq!(
        consistent(&facts, &read(other)),
        Err(Reason::SubjectMismatch)
    );

    let mut other = fixture::reference(&bytes, &subject());
    other["producer_identity"] = json!("wire-witness@0.0.9");
    assert_eq!(
        consistent(&facts, &read(other)),
        Err(Reason::ProducerMismatch)
    );

    let mut other = fixture::reference(&bytes, &subject());
    other["producer_digest"] = json!(null);
    assert_eq!(
        consistent(&facts, &read(other)),
        Err(Reason::ProducerMismatch)
    );
}
