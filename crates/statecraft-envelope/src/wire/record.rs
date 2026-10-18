//! The admission record section 3.6 appends, and the cross-field rules of
//! step 9 that compare the manifest with its reference.
//!
//! The record keeps the manifest digest, the policy identity, the subject
//! identity, the four dimensions, the outcome and the reason. Its content
//! findings keep a path, a detector and a category. The manifest's decoded
//! facts are kept **only once the screen has passed**, which is why `facts`
//! is a [`Stated`]: a refusal at or before step 7 records them as
//! `not-recorded`, and nothing the screen did not judge is ever copied here.

use serde::{Deserialize, Serialize};

use super::manifest::Facts;
use super::reference::WireReference;
use super::screen::Finding;
use super::{EVIDENCE_TYPE, Outcome, Producer, Reason, SCHEMA_VERSION, Stated, Subject};
use crate::dimensions::Dimensions;

/// The record's own schema name.
pub const RECORD_SCHEMA: &str = "statecraft/wire-evidence-admission/v1";

/// What the reference named: identities only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    /// The artifact type.
    pub evidence_type: String,
    /// The schema version.
    pub schema_version: u64,
    /// The construction.
    pub construction: String,
    /// The manifest digest, lowercase hex.
    pub manifest_digest: String,
    /// The manifest byte length.
    pub byte_length: u64,
    /// The producer.
    pub producer: Producer,
    /// The subject the reference repeated.
    pub subject: Stated<Subject>,
    /// The producer's capture completeness word.
    pub completeness: Stated<String>,
}

impl From<&WireReference> for Artifact {
    fn from(r: &WireReference) -> Self {
        Artifact {
            evidence_type: r.evidence_type.clone(),
            schema_version: r.schema_version,
            construction: r.construction.clone(),
            manifest_digest: r.digest.clone(),
            byte_length: r.byte_length,
            producer: r.producer.clone(),
            subject: r.subject.clone(),
            completeness: r.completeness.clone(),
        }
    }
}

/// Step 4's result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Binding {
    /// Bound to the exact run, attempt, effect and producer.
    Bound,
    /// The run, attempt or effect does not bind.
    SubjectMismatch,
    /// The producer identity is missing or differs.
    ProducerMismatch,
    /// Bound to an earlier attempt than the one judged.
    Stale,
    /// An earlier step refused, so binding was not judged.
    NotEvaluated,
}

/// Whether the bounded handoff completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Handoff {
    /// Every byte arrived within the bounds.
    Complete,
    /// The handoff stopped early.
    Incomplete,
    /// Not known.
    Unknown,
}

/// Custody of the judged bytes (section 3.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Custody {
    /// Where the bytes were obtained.
    pub obtained_from: Stated<String>,
    /// The process that wrote them.
    pub writer: Stated<String>,
    /// The Statecraft process that read them.
    pub reader: Stated<String>,
    /// Whether the bounded handoff completed.
    pub handoff: Handoff,
}

impl Custody {
    /// `recorded` when every identity is known and the handoff completed;
    /// otherwise `unknown`. A claim without the identities is unknown custody.
    pub fn result(&self) -> &'static str {
        let known = self.obtained_from.value().is_some()
            && self.writer.value().is_some()
            && self.reader.value().is_some();
        if known && self.handoff == Handoff::Complete {
            "recorded"
        } else {
            "unknown"
        }
    }
}

/// A duplicate's pointer to the record it duplicates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prior {
    /// The prior admission record's hash in the run record's chain.
    pub record_digest: String,
    /// The prior record's reason.
    pub reason: Reason,
}

/// One admission record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionRecord {
    /// Always [`RECORD_SCHEMA`].
    pub schema: String,
    /// What the reference named, or `unavailable` when it could not be read.
    pub artifact: Stated<Artifact>,
    /// Step 4's result.
    pub binding: Binding,
    /// The manifest's facts, kept only after the screen passed.
    pub facts: Stated<Facts>,
    /// Custody of the judged bytes.
    pub custody: Custody,
    /// The four evidence dimensions.
    pub dimensions: Dimensions,
    /// Admit or refuse.
    pub outcome: Outcome,
    /// The one stable reason.
    pub reason: Reason,
    /// The policy identity judged under.
    pub policy: String,
    /// For a duplicate, the prior record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prior: Option<Prior>,
    /// Content findings: path, detector and category only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<Finding>,
    /// For `malformed`, the bound or position that broke; never a value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub malformed: Option<String>,
}

/// Step 9's cross-field rules: the manifest says what its reference says.
///
/// # Errors
///
/// `unsupported-artifact` for another type or version, `subject-mismatch`
/// for another subject, `producer-mismatch` for another producer.
pub fn consistent(facts: &Facts, reference: &WireReference) -> Result<(), Reason> {
    if facts.artifact_type != EVIDENCE_TYPE
        || facts.schema_version != SCHEMA_VERSION
        || facts.record_digest_construction != reference.construction
    {
        return Err(Reason::UnsupportedArtifact);
    }
    if reference.subject.value() != Some(&facts.subject) {
        return Err(Reason::SubjectMismatch);
    }
    let digest = |s: &Stated<String>| s.value().map(|d| super::bare_digest(d).to_string());
    if reference.producer.name != facts.producer.name
        || reference.producer.revision != facts.producer.revision
        || digest(&reference.producer.content_digest) != digest(&facts.producer.content_digest)
    {
        return Err(Reason::ProducerMismatch);
    }
    Ok(())
}
