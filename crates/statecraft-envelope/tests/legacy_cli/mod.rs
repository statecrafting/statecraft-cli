//! The statecraft-cli serializer as it stood **before** this crate arrived,
//! transcribed verbatim, and the cases it was asked to emit.
//!
//! # Why a second copy of types this repository just finished de-duplicating
//!
//! The fixtures under `testdata/fixtures/cli/` were emitted by
//! `statecraft-acceptance` at `8f6591f`, the commit before spec 007. That crate
//! now re-exports this one, so asking it to re-emit them would be asking the
//! shared types whether they agree with themselves: a test that cannot fail and
//! therefore cannot be evidence.
//!
//! What is frozen here is the **encoder**, not the crate. These definitions are
//! copied character for character from
//! `crates/statecraft-acceptance/src/{absence,dimensions,evidence,receipt}.rs`
//! at `8f6591f`, with the doc comments and the judgment functions dropped. They
//! are deliberately not `pub use`d from anywhere: nothing links them to the
//! shared types, so a change to a shared serde attribute moves one encoder and
//! not the other, and `cli_compat.rs` sees the two disagree.
//!
//! **They are never edited to make a test pass.** They are what the CLI wrote.
//! A disagreement between this encoder and the shared types is a compatibility
//! break to be decided, not a transcription to be corrected. The one exception
//! is the ambiguity spec 007 closed, which is recorded in `legacy_collision`
//! below and asserted as a known, named difference.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------- absence.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Absence {
    None,
    NotRecorded,
    Stale,
}

/// The pre-007 `Recorded<T>`: untagged, `Present` declared **first**. For
/// `T = String` this is the reader that takes `"not-recorded"` as a present
/// string, which is the divergence spec 007 resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", untagged)]
pub enum Recorded<T> {
    Present(T),
    Absent(Absence),
}

// ------------------------------------------------------------- dimensions.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Integrity {
    Pass,
    Fail,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Signature {
    Pass,
    Fail,
    Unsigned,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IssuerTrust {
    Pass,
    Fail,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubjectBinding {
    Pass,
    Fail,
    Unknown,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dimensions {
    pub integrity: Integrity,
    pub signature: Signature,
    pub issuer_trust: IssuerTrust,
    pub subject_binding: SubjectBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RefusalCode {
    RequiredDimensionNotPassed { dimension: String, value: String },
    IncompleteEvidence { missing: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "admission")]
pub enum Admission {
    Admit,
    Refuse { reason: RefusalCode },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionPolicy {
    pub require_integrity: bool,
    pub require_signature: bool,
    pub require_issuer_trust: bool,
}

// --------------------------------------------------------------- evidence.rs

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Construction {
    FileBytesSha256,
    CanonicalRecordSha256 { canonicalization_version: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Embedded {
    pub container: String,
    pub selector: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub evidence_type: String,
    pub schema_version: String,
    pub digest: String,
    pub bytes: u64,
    pub construction: Construction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedded: Option<Embedded>,
}

// ---------------------------------------------------------------- receipt.rs

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub repository: String,
    pub base: String,
    pub candidate: String,
    pub suite: Vec<SuiteEntry>,
    pub policy_digest: String,
    pub product_version: String,
    pub spec_spine_version: String,
    pub adapter_version: String,
    pub harness_revision: Recorded<String>,
    pub attempt: String,
    #[serde(default)]
    pub authority_paths_touched: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuiteEntry {
    pub command: String,
    pub exit_code: i32,
}

// ------------------------------------------------------------------ the case

/// SHA-256 of `b"test"`, as `statecraft-environment::digest::digest_bytes`
/// computed it for the committed fixtures. Written out rather than recomputed
/// so this encoder borrows nothing from the crate it is checking; that the
/// shared `Reference::over_file_bytes` still produces exactly this is asserted
/// separately in `cli_compat.rs`.
pub const SHA256_OF_TEST: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("the legacy encoder serializes")
}

fn receipt(harness_revision: Recorded<String>) -> Receipt {
    Receipt {
        repository: "https://github.com/acme/widget".into(),
        base: "0123456789abcdef0123456789abcdef01234567".into(),
        candidate: "89abcdef0123456789abcdef0123456789abcdef".into(),
        suite: vec![SuiteEntry {
            command: "make gate".into(),
            exit_code: 0,
        }],
        policy_digest: "sha256:abc".into(),
        product_version: "0.0.0".into(),
        spec_spine_version: "0.18.0".into(),
        adapter_version: "0.0.0".into(),
        harness_revision,
        attempt: "attempt-1".into(),
        authority_paths_touched: vec![],
    }
}

/// Every fixture, by file name, as the pre-007 CLI serializer writes it.
///
/// The case list is the one that produced the committed files, run against the
/// live `statecraft-acceptance` at `8f6591f`.
pub fn fixtures() -> Vec<(&'static str, String)> {
    let mut out = vec![
        (
            "reference-file-bytes.json",
            json(&Reference {
                evidence_type: "statecraft-cli/receipt".into(),
                schema_version: "1".into(),
                digest: SHA256_OF_TEST.into(),
                bytes: 4,
                construction: Construction::FileBytesSha256,
                embedded: None,
            }),
        ),
        (
            "reference-canonical-embedded.json",
            json(&Reference {
                evidence_type: "statecraft-cli/journal-record".into(),
                schema_version: "1".into(),
                digest: SHA256_OF_TEST.into(),
                bytes: 4,
                construction: Construction::CanonicalRecordSha256 {
                    canonicalization_version: "1".into(),
                },
                embedded: Some(Embedded {
                    container: "statecraft-cli/journal-bundle".into(),
                    selector: "work/484".into(),
                }),
            }),
        ),
        (
            "dimensions-unsigned-today.json",
            json(&Dimensions {
                integrity: Integrity::Pass,
                signature: Signature::Unsigned,
                issuer_trust: IssuerTrust::Unknown,
                subject_binding: SubjectBinding::NotApplicable,
            }),
        ),
        (
            "dimensions-all-unknown.json",
            json(&Dimensions {
                integrity: Integrity::Unknown,
                signature: Signature::Unknown,
                issuer_trust: IssuerTrust::Unknown,
                subject_binding: SubjectBinding::Unknown,
            }),
        ),
        (
            "dimensions-all-pass.json",
            json(&Dimensions {
                integrity: Integrity::Pass,
                signature: Signature::Pass,
                issuer_trust: IssuerTrust::Pass,
                subject_binding: SubjectBinding::Pass,
            }),
        ),
        ("admission-admit.json", json(&Admission::Admit)),
        (
            "admission-refuse-dimension.json",
            json(&Admission::Refuse {
                reason: RefusalCode::RequiredDimensionNotPassed {
                    dimension: "signature".into(),
                    value: "unsigned".into(),
                },
            }),
        ),
        (
            "admission-refuse-incomplete.json",
            json(&Admission::Refuse {
                reason: RefusalCode::IncompleteEvidence {
                    missing: vec!["integrity".into(), "issuerTrust".into()],
                },
            }),
        ),
        (
            "policy-strict.json",
            json(&AdmissionPolicy {
                require_integrity: true,
                require_signature: true,
                require_issuer_trust: true,
            }),
        ),
        ("policy-permissive.json", json(&AdmissionPolicy::default())),
        (
            "recorded-forms.json",
            json(&vec![
                Recorded::Present("present-value".to_string()),
                Recorded::Absent(Absence::None),
                Recorded::Absent(Absence::NotRecorded),
                Recorded::Absent(Absence::Stale),
            ]),
        ),
        ("recorded-collision-legacy.json", json(&legacy_collision())),
        (
            "receipt-not-recorded.json",
            json(&receipt(Recorded::Absent(Absence::NotRecorded))),
        ),
        (
            "receipt-harness-present.json",
            json(&receipt(Recorded::Present("harness-2026.09".into()))),
        ),
    ];
    // A receipt written before `authority_paths_touched` existed. String
    // surgery on the CLI's own output, so member order stays the CLI's.
    let legacy_receipt = json(&receipt(Recorded::Absent(Absence::NotRecorded)))
        .replace(",\"authority_paths_touched\":[]", "");
    out.push(("receipt-legacy-no-authority-paths.json", legacy_receipt));
    out
}

/// The one value this encoder can write that the shared types refuse: a
/// **present** string equal to a reserved absence word. The pre-007 CLI had
/// nothing to stop it, and the bytes it produces are indistinguishable from
/// the absence's.
pub fn legacy_collision() -> Recorded<String> {
    Recorded::Present("not-recorded".to_string())
}
