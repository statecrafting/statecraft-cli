//! Compatibility with the bytes statecraft-cli already writes.
//!
//! Spec 005 section 3.14. Three properties, and they are not the same property:
//!
//! 1. **Provenance.** Every fixture under `testdata/fixtures/cli/` is what the
//!    CLI's own serializer emits. Asserted against the frozen transcription in
//!    [`legacy_cli`], which is an encoder this crate does not share a line of
//!    code with.
//! 2. **Byte preservation.** Each fixture deserializes into the shared type and
//!    re-serializes to the identical bytes.
//! 3. **Semantic interpretation.** The value it deserializes into means what
//!    the CLI meant: the right absence, the right dimension, the right
//!    defaults, and an unknown member preserved rather than coerced or
//!    rejected. A round trip proves none of this on its own, which is why every
//!    fixture below is also read for its meaning.

mod legacy_cli;

use statecraft_envelope::absence::{Absence, Recorded};
use statecraft_envelope::dimensions::{
    Admission, AdmissionPolicy, Dimensions, Integrity, IssuerTrust, RefusalCode, Signature,
    SubjectBinding,
};
use statecraft_envelope::hash::sha256_hex;
use statecraft_envelope::reference::{Construction, Reference};

fn dir() -> String {
    format!("{}/testdata/fixtures/cli", env!("CARGO_MANIFEST_DIR"))
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/{name}", dir()))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

/// Deserialize a fixture into the shared type and require the bytes back.
fn round_trip<T: serde::de::DeserializeOwned + serde::Serialize>(name: &str) -> T {
    let text = fixture(name);
    let value: T = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(
        serde_json::to_string(&value).unwrap(),
        text,
        "{name} did not re-serialize byte for byte"
    );
    value
}

// ------------------------------------------------------------- 1. provenance

#[test]
fn every_committed_fixture_is_what_the_cli_serializer_writes() {
    for (name, expected) in legacy_cli::fixtures() {
        assert_eq!(
            fixture(name),
            expected,
            "{name} is not what the pre-transfer statecraft-cli encoder produces"
        );
    }
}

#[test]
fn no_fixture_is_committed_without_the_case_that_produced_it() {
    let cased: Vec<&str> = legacy_cli::fixtures().iter().map(|(n, _)| *n).collect();
    let mut orphans: Vec<String> = std::fs::read_dir(dir())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".json") && !cased.contains(&n.as_str()))
        .collect();
    orphans.sort();
    assert!(
        orphans.is_empty(),
        "fixtures with no emitting case in legacy_cli: {orphans:?}"
    );
}

#[test]
fn the_shared_reference_still_computes_the_digest_the_fixtures_were_written_with() {
    // The one value the transcription writes out as a literal. If this crate's
    // SHA-256 ever stopped agreeing with the CLI's, every reference fixture
    // would still round-trip and every digest in them would be wrong.
    assert_eq!(sha256_hex(b"test"), legacy_cli::SHA256_OF_TEST);
    assert_eq!(
        Reference::over_file_bytes("statecraft-cli/receipt", "1", b"test").digest,
        legacy_cli::SHA256_OF_TEST
    );
}

// ----------------------------------------- 2 and 3. bytes, and what they mean

#[test]
fn a_file_bytes_reference_round_trips_and_is_checkable_against_its_bytes() {
    let r: Reference = round_trip("reference-file-bytes.json");
    assert_eq!(r.evidence_type, "statecraft-cli/receipt");
    assert_eq!(r.schema_version, "1");
    assert_eq!(r.bytes, 4);
    assert_eq!(r.construction, Construction::FileBytesSha256);
    assert_eq!(r.construction.name(), "file-bytes-sha256");
    assert!(r.checkable_here());
    assert!(r.matches(b"test"));
    assert!(!r.matches(b"tesu"));
    // The three fields the envelope added are absent, and stay absent.
    assert_eq!(r.embedded, None);
    assert_eq!(r.producer_digest, None);
    assert_eq!(r.native, None);
    assert_eq!(r.subject, None);
}

#[test]
fn a_canonical_embedded_reference_round_trips_and_is_not_answered_with_a_file_hash() {
    let r: Reference = round_trip("reference-canonical-embedded.json");
    assert_eq!(r.construction.name(), "canonical-record-sha256/1");
    let embedded = r.embedded.as_ref().expect("the fixture carries embedded");
    assert_eq!(embedded.container, "statecraft-cli/journal-bundle");
    assert_eq!(embedded.selector, "work/484");
    assert!(!r.checkable_here());
    assert!(
        !r.matches(b"test"),
        "a canonical record hash never substitutes for a file-byte digest"
    );
}

#[test]
fn each_dimensions_fixture_reads_as_the_statuses_the_cli_recorded() {
    let unsigned: Dimensions = round_trip("dimensions-unsigned-today.json");
    assert_eq!(unsigned.integrity, Integrity::Pass);
    assert_eq!(unsigned.signature, Signature::Unsigned);
    assert_eq!(unsigned.issuer_trust, IssuerTrust::Unknown);
    assert_eq!(unsigned.subject_binding, SubjectBinding::NotApplicable);
    assert_eq!(
        unsigned,
        Dimensions::unsigned_today(Integrity::Pass, SubjectBinding::NotApplicable)
    );

    let unknown: Dimensions = round_trip("dimensions-all-unknown.json");
    assert_eq!(unknown, Dimensions::all_unknown());

    let pass: Dimensions = round_trip("dimensions-all-pass.json");
    assert_eq!(pass.integrity, Integrity::Pass);
    assert_eq!(pass.signature, Signature::Pass);
    assert_eq!(pass.issuer_trust, IssuerTrust::Pass);
    assert_eq!(pass.subject_binding, SubjectBinding::Pass);
}

#[test]
fn admission_round_trips_and_both_cli_refusal_codes_keep_their_fields() {
    let admit: Admission = round_trip("admission-admit.json");
    assert!(admit.is_admit());
    assert!(admit.reasons().is_empty());

    let dimension: Admission = round_trip("admission-refuse-dimension.json");
    match &dimension {
        Admission::Refuse {
            reason: RefusalCode::RequiredDimensionNotPassed { dimension, value },
            reasons,
        } => {
            assert_eq!(dimension, "signature");
            assert_eq!(value, "unsigned");
            assert!(reasons.is_empty(), "the CLI writes no reasons array");
        }
        other => panic!("expected a named refusal, got {other:?}"),
    }
    // A CLI refusal carries one reason, and reads back as a list of one.
    assert_eq!(dimension.reasons().len(), 1);

    let incomplete: Admission = round_trip("admission-refuse-incomplete.json");
    match incomplete.reasons().as_slice() {
        [RefusalCode::IncompleteEvidence { missing }] => {
            assert_eq!(
                missing,
                &["integrity".to_string(), "issuerTrust".to_string()]
            );
        }
        other => panic!("expected incomplete evidence, got {other:?}"),
    }
}

#[test]
fn a_cli_policy_reads_with_every_field_this_crate_added_at_its_default() {
    let strict: AdmissionPolicy = round_trip("policy-strict.json");
    assert_eq!(strict, AdmissionPolicy::strict());
    assert!(strict.require_integrity && strict.require_signature && strict.require_issuer_trust);
    // Absent in the bytes, defaulted on read, and skipped on write: that last
    // part is what keeps the round trip byte-identical.
    assert_eq!(strict.policy_version, None);
    assert!(strict.require_artifacts.is_empty());
    assert!(!strict.require_subject_binding);
    assert_eq!(strict.min_approvals, 0);
    assert!(!strict.approvers_distinct);
    assert!(!strict.approver_may_not_be_submitter);

    let permissive: AdmissionPolicy = round_trip("policy-permissive.json");
    assert_eq!(permissive, AdmissionPolicy::permissive());
    assert_eq!(permissive, AdmissionPolicy::default());

    // Two CLI policies that differ still have different digests; the added
    // fields defaulting does not collapse them.
    assert_ne!(strict.digest(), permissive.digest());
}

#[test]
fn the_four_recorded_forms_round_trip_and_read_as_the_cli_meant_them() {
    let text = fixture("recorded-forms.json");
    let forms: Vec<Recorded<String>> = serde_json::from_str(&text).unwrap();
    assert_eq!(
        forms,
        vec![
            Recorded::Present("present-value".to_string()),
            Recorded::Absent(Absence::None),
            Recorded::Absent(Absence::NotRecorded),
            Recorded::Absent(Absence::Stale),
        ]
    );
    assert_eq!(serde_json::to_string(&forms).unwrap(), text);
}

#[test]
fn a_receipts_absent_harness_revision_reads_as_an_absence_and_its_bytes_survive() {
    let text = fixture("receipt-not-recorded.json");
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let harness: Recorded<String> =
        serde_json::from_value(value["harness_revision"].clone()).unwrap();
    assert_eq!(harness, Recorded::Absent(Absence::NotRecorded));
    assert!(!harness.is_present());
    // The receipt is a CLI type; this crate keeps it as bytes, and those bytes
    // are valid under the portable-input policy (spec 005 section 3.7).
    assert_eq!(statecraft_envelope::portable::scan(text.as_bytes()), Ok(()));
}

#[test]
fn a_receipts_present_harness_revision_stays_a_value() {
    let value: serde_json::Value =
        serde_json::from_str(&fixture("receipt-harness-present.json")).unwrap();
    let harness: Recorded<String> =
        serde_json::from_value(value["harness_revision"].clone()).unwrap();
    assert_eq!(harness, Recorded::Present("harness-2026.09".into()));
    assert_eq!(harness.value().map(String::as_str), Some("harness-2026.09"));
}

#[test]
fn a_record_written_before_a_field_existed_reads_with_that_field_defaulted() {
    let text = fixture("receipt-legacy-no-authority-paths.json");
    assert!(!text.contains("authority_paths_touched"));
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(value.get("authority_paths_touched").is_none());
    // The absent field is the CLI's `#[serde(default)]`, and the older bytes
    // are still a complete receipt to a reader that has the newer field.
    let harness: Recorded<String> =
        serde_json::from_value(value["harness_revision"].clone()).unwrap();
    assert_eq!(harness, Recorded::Absent(Absence::NotRecorded));
}

// -------------------------------- the three reserved strings, and legacy bytes

#[test]
fn all_three_reserved_strings_read_as_absences_wherever_they_appear() {
    for absence in Absence::all() {
        let word = absence.word();
        // Bare.
        assert_eq!(
            serde_json::from_str::<Recorded<String>>(&format!("\"{word}\"")).unwrap(),
            Recorded::Absent(absence)
        );
        // In the receipt field that motivated the type, in a full record.
        let receipt = fixture("receipt-not-recorded.json").replace(
            "\"harness_revision\":\"not-recorded\"",
            &format!("\"harness_revision\":\"{word}\""),
        );
        let value: serde_json::Value = serde_json::from_str(&receipt).unwrap();
        let harness: Recorded<String> =
            serde_json::from_value(value["harness_revision"].clone()).unwrap();
        assert_eq!(harness, Recorded::Absent(absence), "word {word}");
    }
}

#[test]
fn the_bytes_the_old_cli_wrote_for_a_colliding_present_value_now_read_as_the_absence() {
    let text = fixture("recorded-collision-legacy.json");
    assert_eq!(text, "\"not-recorded\"");

    // What the pre-transfer CLI reader made of them: a present string. The
    // divergence is asserted rather than described, so it cannot quietly stop
    // being true.
    let legacy: legacy_cli::Recorded<String> = serde_json::from_str(&text).unwrap();
    assert_eq!(
        legacy,
        legacy_cli::Recorded::Present("not-recorded".to_string())
    );

    // What every reader of the shared contract makes of them.
    let shared: Recorded<String> = serde_json::from_str(&text).unwrap();
    assert_eq!(shared, Recorded::Absent(Absence::NotRecorded));
    assert_eq!(serde_json::to_string(&shared).unwrap(), text);
}

#[test]
fn the_colliding_value_can_no_longer_be_written() {
    for absence in Absence::all() {
        assert!(
            Recorded::<String>::present(absence.word().to_string()).is_err(),
            "{} must be refused as a present value",
            absence.word()
        );
        let built = Recorded::Present(absence.word().to_string());
        assert!(
            serde_json::to_string(&built).is_err(),
            "{} must be refused at serialization too",
            absence.word()
        );
    }
    // A value that merely contains a reserved word is untouched.
    let ok = Recorded::<String>::present("not-recorded-by-the-adapter".into()).unwrap();
    assert_eq!(
        serde_json::to_string(&ok).unwrap(),
        "\"not-recorded-by-the-adapter\""
    );
}

// ------------------------------------------------------- 4. unknown members

#[test]
fn an_unknown_construction_is_preserved_verbatim_and_reported_unknown() {
    let text = r#"{"evidence_type":"x","schema_version":"1","digest":"00","bytes":1,"construction":{"canonical-record-blake3":{"canonicalization_version":"9"}}}"#;
    let r: Reference = serde_json::from_str(text).unwrap();
    assert!(matches!(r.construction, Construction::Unknown(_)));
    assert!(!r.checkable_here());
    assert!(!r.matches(b"x"), "an unknown construction is never guessed");
    assert!(r.construction.name().starts_with("unknown:"));
    assert_eq!(serde_json::to_string(&r).unwrap(), text);
}

#[test]
fn an_unknown_refusal_code_is_preserved_verbatim_in_both_shapes() {
    for text in [
        r#"{"admission":"refuse","reason":"quota-exhausted"}"#,
        r#"{"admission":"refuse","reason":{"rate-limited":{"retry_after":30}}}"#,
    ] {
        let a: Admission = serde_json::from_str(text).unwrap();
        assert!(matches!(
            a,
            Admission::Refuse {
                reason: RefusalCode::Unknown(_),
                ..
            }
        ));
        assert!(!a.is_admit(), "an unreadable reason is still a refusal");
        assert_eq!(a.reasons().len(), 1);
        assert_eq!(serde_json::to_string(&a).unwrap(), text);
    }
}

#[test]
fn a_refusal_code_this_build_added_is_readable_by_a_reader_that_has_it() {
    // The members resolution R-3 added, exercised through the wire form so the
    // CLI's `admission` tag is what carries them.
    let text = r#"{"admission":"refuse","reason":{"approvals-insufficient":{"have":1,"need":2}}}"#;
    let a: Admission = serde_json::from_str(text).unwrap();
    assert_eq!(
        a.reasons(),
        vec![&RefusalCode::ApprovalsInsufficient { have: 1, need: 2 }]
    );
    assert_eq!(serde_json::to_string(&a).unwrap(), text);
}

#[test]
fn an_unknown_dimension_value_is_refused_and_never_coerced_to_unknown() {
    // The closed sets are closed. `unknown` means "this verifier did not
    // check", not "this verifier did not understand", and silently mapping the
    // second onto the first would report a check that never happened.
    for (field, bad) in [
        ("integrity", "maybe"),
        ("signature", "not-applicable"),
        ("issuer_trust", "unsigned"),
        ("subject_binding", "revoked"),
    ] {
        let text = r#"{"integrity":"pass","signature":"pass","issuer_trust":"pass","subject_binding":"pass"}"#
        .replace(
            &format!("\"{field}\":\"pass\""),
            &format!("\"{field}\":\"{bad}\""),
        );
        assert!(
            serde_json::from_str::<Dimensions>(&text).is_err(),
            "{field} accepted {bad}"
        );
    }
}

#[test]
fn a_dimension_value_belonging_to_another_dimension_is_still_refused() {
    // `unsigned` belongs only to `signature`, `not-applicable` only to
    // `subjectBinding`. The vocabulary rule is enforced by the types, so the
    // wire forms that would break it do not decode.
    assert!(serde_json::from_str::<Integrity>("\"unsigned\"").is_err());
    assert!(serde_json::from_str::<IssuerTrust>("\"not-applicable\"").is_err());
    assert!(serde_json::from_str::<Signature>("\"unsigned\"").is_ok());
    assert!(serde_json::from_str::<SubjectBinding>("\"not-applicable\"").is_ok());
}

#[test]
fn an_unknown_field_is_dropped_rather_than_preserved_and_that_is_recorded_here() {
    // Known limitation, spec 005 section 5. Unknown *members* of an enum are
    // preserved; an unknown *field* of a struct is not, because no type here
    // carries an extras map. A reader that re-serializes a record written by a
    // newer producer therefore loses the newer field. The test exists so the
    // loss is a recorded decision and not a discovery.
    let text = r#"{"evidence_type":"x","schema_version":"1","digest":"00","bytes":1,"construction":"file-bytes-sha256","future_field":"kept?"}"#;
    let r: Reference = serde_json::from_str(text).unwrap();
    let back = serde_json::to_string(&r).unwrap();
    assert!(!back.contains("future_field"));
    assert_ne!(back, text);
}
