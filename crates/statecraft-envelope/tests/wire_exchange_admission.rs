//! Spec 014's envelope half, against portable fixtures in the producer's own
//! shape. This change covers the bounded reader and the reference: section
//! 4's malformed, unsupported and changed-bytes rows. The manifest, screen
//! and policy rows join this file with the change that adds them; the rows
//! that need the run record are `statecraft-run`'s and
//! `statecraft-acceptance`'s.

use serde_json::json;
use statecraft_envelope::wire::bounded::{self, Refusal};
use statecraft_envelope::wire::fixture;
use statecraft_envelope::wire::reference::WireReference;
use statecraft_envelope::wire::{Stated, Subject};

fn subject() -> Subject {
    Subject {
        run_id: "run-014".into(),
        attempt: 1,
        sidecar_effect_id: "run-014/1/witness-sidecar".into(),
    }
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
