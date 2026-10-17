//! Wire-exchange evidence: the `statecraft/wire-exchange/v1` artifact, read
//! and judged without trusting it. Spec 014, through its additive `extends`
//! edge on this crate (spec 005 owns the crate; spec 014 owns no code).
//!
//! What lives here is the pure half of section 3.6: everything that can be
//! decided from the reference, the manifest bytes and the recorded policy
//! alone. The half that reads the run record (subject binding, duplicates,
//! the append) is `statecraft-run::wire`, and the ordered procedure that joins
//! the two is `statecraft-acceptance::wire`.
//!
//! - [`bounded`] decodes under section 3.1's limits, refusing duplicate member
//!   names, before anything is interpreted.
//! - [`reference`] reads the producer's evidence reference, in the producer's
//!   own snake_case wire form, into this product's typed reference.
//! - [`fixture`] builds portable fixtures in the producer's exact shape.
//!
//! The v1 member set, the prohibited-content screen, the policy and the
//! admission record follow in the next change of the same series.
//!
//! The six grades of section 3.8 stay separate here by construction: nothing
//! in this module reads provider usage, an estimate or an attestation as
//! anything but what the producer said.

pub mod bounded;
pub mod fixture;
pub mod reference;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The one artifact type this version recognizes (section 3.1).
pub const EVIDENCE_TYPE: &str = "statecraft/wire-exchange/v1";
/// The one schema version this version recognizes.
pub const SCHEMA_VERSION: u64 = 1;
/// The one digest construction this version recognizes.
pub const CONSTRUCTION: &str = "file-bytes-sha256";

/// The subject tuple an artifact repeats (section 3.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subject {
    /// The run, exactly as Statecraft supplied it.
    pub run_id: String,
    /// The attempt within it; positive.
    pub attempt: u32,
    /// The sidecar effect's identity.
    pub sidecar_effect_id: String,
}

/// A fact as recorded: a known value, or the explicit state that stands in
/// for one (section 3.1). Never an empty value, a zero or a nearby version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Stated<T> {
    /// The value is known.
    Known {
        /// The value.
        value: T,
    },
    /// The producer stated that there is none.
    Absent,
    /// Could not be decided.
    Unknown,
    /// Could not be read.
    Unavailable,
    /// No record of it exists.
    NotRecorded,
}

impl<T> Stated<T> {
    /// A known value.
    pub fn known(value: T) -> Self {
        Stated::Known { value }
    }

    /// The value, when known.
    pub fn value(&self) -> Option<&T> {
        match self {
            Stated::Known { value } => Some(value),
            _ => None,
        }
    }

    /// The word for the state, `known` included.
    pub fn word(&self) -> &'static str {
        match self {
            Stated::Known { .. } => "known",
            Stated::Absent => "absent",
            Stated::Unknown => "unknown",
            Stated::Unavailable => "unavailable",
            Stated::NotRecorded => "not-recorded",
        }
    }
}

/// The producer identity, in its three parts (section 3.1 item 4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Producer {
    /// The producer's name.
    pub name: Stated<String>,
    /// The producer's exact revision.
    pub revision: Stated<String>,
    /// The producer's content digest, `sha256:<hex>`.
    pub content_digest: Stated<String>,
}

/// A stable reason code (section 3.6).
///
/// Written as its kebab-case word. A word this build does not know is kept as
/// written and never reinterpreted: an older reader preserves a newer reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// The admitting reason: every step passed under the recorded policy.
    Satisfied,
    /// Not one supported artifact type, schema version and construction.
    UnsupportedArtifact,
    /// Broke a section 3.1 bound, a strict decoding rule or a cross-field rule.
    Malformed,
    /// The bytes or their length differ from the reference.
    DigestMismatch,
    /// The producer identity is missing or differs from the attempt's.
    ProducerMismatch,
    /// The run, attempt or effect does not bind.
    SubjectMismatch,
    /// The same manifest, policy and subject were already judged.
    Duplicate,
    /// Bound to an earlier attempt.
    Stale,
    /// Below the policy's minimum completeness.
    Incomplete,
    /// Redaction failed or is unknown where policy requires it.
    RedactionFailed,
    /// `encrypted-content` without a ratified retention policy.
    RetentionForbidden,
    /// The screen found prohibited content, or could not complete.
    ProhibitedContent,
    /// The policy identity differs from the attempt's recorded one.
    PolicyMismatch,
    /// The policy's requirement is not met.
    PolicyUnsatisfied,
    /// A word this build does not know, preserved.
    Other(String),
}

impl Reason {
    /// The word this reason is written as.
    pub fn word(&self) -> &str {
        match self {
            Reason::Satisfied => "satisfied",
            Reason::UnsupportedArtifact => "unsupported-artifact",
            Reason::Malformed => "malformed",
            Reason::DigestMismatch => "digest-mismatch",
            Reason::ProducerMismatch => "producer-mismatch",
            Reason::SubjectMismatch => "subject-mismatch",
            Reason::Duplicate => "duplicate",
            Reason::Stale => "stale",
            Reason::Incomplete => "incomplete",
            Reason::RedactionFailed => "redaction-failed",
            Reason::RetentionForbidden => "retention-forbidden",
            Reason::ProhibitedContent => "prohibited-content",
            Reason::PolicyMismatch => "policy-mismatch",
            Reason::PolicyUnsatisfied => "policy-unsatisfied",
            Reason::Other(word) => word,
        }
    }

    /// The reason a word names; an unknown word is preserved.
    pub fn from_word(word: &str) -> Self {
        [
            Reason::Satisfied,
            Reason::UnsupportedArtifact,
            Reason::Malformed,
            Reason::DigestMismatch,
            Reason::ProducerMismatch,
            Reason::SubjectMismatch,
            Reason::Duplicate,
            Reason::Stale,
            Reason::Incomplete,
            Reason::RedactionFailed,
            Reason::RetentionForbidden,
            Reason::ProhibitedContent,
            Reason::PolicyMismatch,
            Reason::PolicyUnsatisfied,
        ]
        .into_iter()
        .find(|r| r.word() == word)
        .unwrap_or_else(|| Reason::Other(word.to_string()))
    }
}

impl Serialize for Reason {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.word())
    }
}

impl<'de> Deserialize<'de> for Reason {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Reason::from_word(&String::deserialize(d)?))
    }
}

/// Admit or refuse: the only two outcomes a completed judgment has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Admitted under the recorded policy.
    Admit,
    /// Refused, with the reason beside it.
    Refuse,
}

impl Outcome {
    /// The word.
    pub fn word(self) -> &'static str {
        match self {
            Outcome::Admit => "admit",
            Outcome::Refuse => "refuse",
        }
    }
}

/// Lowercase hex without a `sha256:` prefix, so the reference's bare digest
/// and a prefixed one compare as the same value.
pub fn bare_digest(digest: &str) -> &str {
    digest.strip_prefix("sha256:").unwrap_or(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_reason_word_is_preserved_not_reinterpreted() {
        let r: Reason = serde_json::from_str("\"a-later-reason\"").unwrap();
        assert_eq!(r, Reason::Other("a-later-reason".into()));
        assert_eq!(serde_json::to_string(&r).unwrap(), "\"a-later-reason\"");
        assert_eq!(Reason::from_word("stale"), Reason::Stale);
    }

    #[test]
    fn a_stated_fact_names_its_state() {
        let known = serde_json::to_string(&Stated::known("x".to_string())).unwrap();
        assert_eq!(known, r#"{"state":"known","value":"x"}"#);
        let absent = serde_json::to_string(&Stated::<String>::NotRecorded).unwrap();
        assert_eq!(absent, r#"{"state":"not-recorded"}"#);
    }
}
