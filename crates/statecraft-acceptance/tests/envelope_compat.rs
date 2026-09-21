//! The other side of the shared contract: this product still writes the bytes
//! the fixtures record, now that it writes them through `statecraft-envelope`.
//!
//! Spec 005 section 3.14. `crates/statecraft-envelope/tests/cli_compat.rs`
//! checks the shared types against fixtures emitted by this crate's serializer
//! before the transfer. This file checks the direction that matters to an
//! operator: that the functions this product actually calls to mint a receipt,
//! judge evidence and decide admission still produce those exact bytes.
//!
//! A round trip through a type proves the type is self-consistent. These
//! assertions go through `mint`, `admit`, `unsigned_today` and
//! `over_file_bytes`, so what is pinned is the output of the product, not the
//! symmetry of a serializer.

use statecraft_acceptance::absence::{Absence, Recorded};
use statecraft_acceptance::dimensions::{
    Admission, AdmissionPolicy, Dimensions, Integrity, IssuerTrust, RefusalCode, Signature,
    SubjectBinding, admit,
};
use statecraft_acceptance::evidence::Reference;
use statecraft_acceptance::independence::{Check, SuiteResult};
use statecraft_acceptance::judged::{Base, Candidate, Judged, Policy};
use statecraft_acceptance::receipt::{MintContext, Receipt, mint};

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/../statecraft-envelope/testdata/fixtures/cli/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn emits<T: serde::Serialize>(name: &str, value: &T) {
    assert_eq!(
        serde_json::to_string(value).unwrap(),
        fixture(name),
        "{name}: this product no longer writes the bytes it wrote at 8f6591f"
    );
}

#[test]
fn a_reference_over_file_bytes_is_written_exactly_as_before() {
    emits(
        "reference-file-bytes.json",
        &Reference::over_file_bytes("statecraft-cli/receipt", "1", b"test"),
    );
}

#[test]
fn the_dimensions_this_product_reports_today_are_written_exactly_as_before() {
    emits(
        "dimensions-unsigned-today.json",
        &Dimensions::unsigned_today(Integrity::Pass, SubjectBinding::NotApplicable),
    );
    emits("dimensions-all-unknown.json", &Dimensions::all_unknown());
    emits(
        "dimensions-all-pass.json",
        &Dimensions {
            integrity: Integrity::Pass,
            signature: Signature::Pass,
            issuer_trust: IssuerTrust::Pass,
            subject_binding: SubjectBinding::Pass,
        },
    );
}

#[test]
fn the_admission_this_product_decides_is_written_exactly_as_before() {
    let intact_unsigned =
        Dimensions::unsigned_today(Integrity::Pass, SubjectBinding::NotApplicable);
    emits(
        "admission-admit.json",
        &admit(&intact_unsigned, &AdmissionPolicy::permissive()),
    );
    emits(
        "admission-refuse-dimension.json",
        &admit(&intact_unsigned, &AdmissionPolicy::strict()),
    );
    // The incomplete-evidence fixture names two missing dimensions, which is
    // what a policy requiring integrity and issuer trust reports over evidence
    // that was never checked.
    emits(
        "admission-refuse-incomplete.json",
        &admit(
            &Dimensions::all_unknown(),
            &AdmissionPolicy {
                require_integrity: true,
                require_signature: false,
                require_issuer_trust: true,
                ..AdmissionPolicy::permissive()
            },
        ),
    );
}

#[test]
fn both_named_policies_are_written_exactly_as_before() {
    emits("policy-strict.json", &AdmissionPolicy::strict());
    emits("policy-permissive.json", &AdmissionPolicy::permissive());
}

#[test]
fn the_four_recorded_forms_are_written_exactly_as_before() {
    emits(
        "recorded-forms.json",
        &vec![
            Recorded::<String>::present("present-value".into()).unwrap(),
            Recorded::Absent(Absence::None),
            Recorded::Absent(Absence::NotRecorded),
            Recorded::Absent(Absence::Stale),
        ],
    );
}

#[test]
fn a_minted_receipt_is_written_exactly_as_before() {
    let judged = Judged {
        candidate: Candidate {
            sha: "89abcdef0123456789abcdef0123456789abcdef".into(),
            work_tree_clean: true,
            head_stable: true,
            dirty_paths: vec![],
        },
        base: Base {
            sha: "0123456789abcdef0123456789abcdef01234567".into(),
        },
        policy: Policy {
            digest: "sha256:abc".into(),
        },
    };
    let context = MintContext {
        repository: "https://github.com/acme/widget".into(),
        product_version: "0.0.0".into(),
        spec_spine_version: "0.18.0".into(),
        adapter_version: "0.0.0".into(),
        attempt: "attempt-1".into(),
        authority_paths_touched: vec![],
    };
    let suite = SuiteResult::new(vec![Check::ran("make gate", 0, None)], None);
    let receipt = mint(&judged, &suite, &context, None).expect("the fixture's receipt mints");

    emits("receipt-not-recorded.json", &receipt);
    // And the field that motivated the reserved-word contract is still an
    // absence, not the string that looks like one.
    assert_eq!(
        receipt.harness_revision,
        Recorded::Absent(Absence::NotRecorded)
    );
}

#[test]
fn a_receipt_read_back_through_this_products_types_means_what_it_said() {
    let receipt: Receipt = serde_json::from_str(&fixture("receipt-not-recorded.json")).unwrap();
    assert_eq!(
        receipt.harness_revision,
        Recorded::Absent(Absence::NotRecorded),
        "the reserved word is an absence to this reader too"
    );
    assert!(receipt.authority_paths_touched.is_empty());

    let present: Receipt = serde_json::from_str(&fixture("receipt-harness-present.json")).unwrap();
    assert_eq!(
        present.harness_revision,
        Recorded::Present("harness-2026.09".into())
    );

    let older: Receipt =
        serde_json::from_str(&fixture("receipt-legacy-no-authority-paths.json")).unwrap();
    assert!(
        older.authority_paths_touched.is_empty(),
        "a field added later defaults rather than failing the read"
    );
}

#[test]
fn the_legacy_collision_reads_as_an_absence_through_this_products_types() {
    // The same reinterpretation the envelope documents, asserted at the API
    // this product's callers use. A receipt whose `harness_revision` was
    // written as a present `"not-recorded"` by a pre-transfer build reads as the
    // absence it was always meant to be.
    let text = fixture("recorded-collision-legacy.json");
    let value: Recorded<String> = serde_json::from_str(&text).unwrap();
    assert_eq!(value, Recorded::Absent(Absence::NotRecorded));
    assert!(!value.is_present());
}

#[test]
fn an_unknown_refusal_code_does_not_stop_this_product_reading_a_refusal() {
    // A refusal minted by the platform with a code this build does not have is
    // still a refusal here, and still re-serializes to the platform's bytes.
    let text = r#"{"admission":"refuse","reason":"approver-is-submitter"}"#;
    let a: Admission = serde_json::from_str(text).unwrap();
    assert!(!a.is_admit());
    assert_eq!(a.reasons(), vec![&RefusalCode::ApproverIsSubmitter]);
    assert_eq!(serde_json::to_string(&a).unwrap(), text);
}
