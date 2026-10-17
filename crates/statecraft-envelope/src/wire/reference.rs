//! The producer's evidence reference, read into this product's terms.
//!
//! wire-witness 0.1.0 writes the reference in its own snake_case wire form
//! (`evidence_type`, `schema_version`, `byte_length`, `sha256`,
//! `construction`, `producer_identity`, `producer_digest`, `subject` with
//! `run_id`, `attempt` and `effect_id`, and `completeness`). Those names are
//! the producer's, so they are read here by hand rather than declared on a
//! type that would also be this product's JSON. What comes out is
//! [`WireReference`], whose own JSON follows this product's naming.
//!
//! Decoding is through [`super::bounded`], so the reference is held to the
//! same section 3.1 limits as the manifest. A member this version does not
//! know is left uninterpreted: it gains no meaning here.

use serde::{Deserialize, Serialize};

use super::bounded::{self, Json};
use super::{CONSTRUCTION, EVIDENCE_TYPE, Producer, SCHEMA_VERSION, Stated, Subject};
use crate::reference::{Construction, Reference};

/// The reference section 3.1 requires, typed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireReference {
    /// The artifact type, as written.
    pub evidence_type: String,
    /// The schema version, as written.
    pub schema_version: u64,
    /// The manifest's SHA-256, lowercase hex.
    pub digest: String,
    /// The manifest's byte length.
    pub byte_length: u64,
    /// The digest construction's name, as written.
    pub construction: String,
    /// The producer identity in its three parts.
    pub producer: Producer,
    /// The subject tuple, when the reference carries a well-formed one.
    pub subject: Stated<Subject>,
    /// The producer's capture completeness word.
    pub completeness: Stated<String>,
}

/// Why a reference could not be read. Reported as `malformed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceError {
    /// A section 3.1 bound or strict-decoding rule.
    Bounded(bounded::Refusal),
    /// A required member is missing or has the wrong type; names the member.
    Member(&'static str),
}

impl ReferenceError {
    /// The word a finding records; never the member's value.
    pub fn detail(&self) -> String {
        match self {
            ReferenceError::Bounded(r) => r.word().to_string(),
            ReferenceError::Member(name) => format!("member {name}"),
        }
    }
}

impl WireReference {
    /// Read the producer's reference bytes.
    ///
    /// # Errors
    ///
    /// The bound or member that made the bytes unreadable.
    pub fn decode(bytes: &[u8]) -> Result<Self, ReferenceError> {
        let json = bounded::decode(bytes).map_err(ReferenceError::Bounded)?;
        let text = |name: &'static str| {
            json.get(name)
                .and_then(Json::as_str)
                .map(str::to_string)
                .ok_or(ReferenceError::Member(name))
        };
        let number = |name: &'static str| {
            json.get(name)
                .and_then(Json::as_u64)
                .ok_or(ReferenceError::Member(name))
        };
        let digest = text("sha256")?;
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(ReferenceError::Member("sha256"));
        }
        Ok(WireReference {
            evidence_type: text("evidence_type")?,
            schema_version: number("schema_version")?,
            digest,
            byte_length: number("byte_length")?,
            construction: text("construction")?,
            producer: producer(
                optional(&json, "producer_identity")?,
                optional(&json, "producer_digest")?,
            ),
            subject: subject(json.get("subject")),
            completeness: match json.get("completeness") {
                Some(Json::String(word)) => Stated::known(word.clone()),
                Some(Json::Null) | None => Stated::Absent,
                Some(_) => return Err(ReferenceError::Member("completeness")),
            },
        })
    }

    /// Whether this is the one artifact, schema and construction section 3.1
    /// recognizes. Anything else is `unsupported-artifact`, never guessed at.
    pub fn supported(&self) -> bool {
        self.evidence_type == EVIDENCE_TYPE
            && self.schema_version == SCHEMA_VERSION
            && self.construction == CONSTRUCTION
    }

    /// The same reference in spec 005's reference contract, so the bytes are
    /// checked by the one implementation that checks every reference.
    pub fn as_reference(&self) -> Reference {
        Reference {
            evidence_type: self.evidence_type.clone(),
            schema_version: self.schema_version.to_string(),
            digest: self.digest.clone(),
            bytes: self.byte_length,
            construction: if self.construction == CONSTRUCTION {
                Construction::FileBytesSha256
            } else {
                Construction::Unknown(serde_json::Value::String(self.construction.clone()))
            },
            embedded: None,
            producer_digest: None,
            native: None,
            subject: None,
        }
    }
}

fn optional(json: &Json, name: &'static str) -> Result<Option<String>, ReferenceError> {
    match json.get(name) {
        Some(Json::String(s)) if !s.is_empty() => Ok(Some(s.clone())),
        Some(Json::Null) | None => Ok(None),
        Some(_) => Err(ReferenceError::Member(name)),
    }
}

/// `producer_identity` is `<name>@<revision>`; a value without the separator
/// names the producer and leaves the revision unknown.
fn producer(identity: Option<String>, digest: Option<String>) -> Producer {
    let (name, revision) = match identity.as_deref().map(|i| i.rsplit_once('@')) {
        Some(Some((name, revision))) if !name.is_empty() && !revision.is_empty() => (
            Stated::known(name.to_string()),
            Stated::known(revision.to_string()),
        ),
        Some(_) => (
            identity.map_or(Stated::Absent, Stated::known),
            Stated::Unknown,
        ),
        None => (Stated::Absent, Stated::Absent),
    };
    Producer {
        name,
        revision,
        content_digest: digest.map_or(Stated::Absent, Stated::known),
    }
}

fn subject(value: Option<&Json>) -> Stated<Subject> {
    let Some(value) = value else {
        return Stated::Absent;
    };
    let text = |name| {
        value
            .get(name)
            .and_then(Json::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let attempt = value
        .get("attempt")
        .and_then(Json::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n > 0);
    match (text("run_id"), attempt, text("effect_id")) {
        (Some(run_id), Some(attempt), Some(sidecar_effect_id)) => Stated::known(Subject {
            run_id,
            attempt,
            sidecar_effect_id,
        }),
        _ => Stated::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRODUCED: &str = concat!(
        r#"{"byte_length":8,"completeness":"complete","construction":"file-bytes-sha256","#,
        r#""evidence_type":"statecraft/wire-exchange/v1","producer_digest":"sha256:00","#,
        r#""producer_identity":"wire-witness@0.1.0","schema_version":1,"#,
        r#""sha256":"05b3abf2579a5eb66403cd78be557fd860633a1fe2103c7642030defe32c657f","#,
        r#""subject":{"attempt":2,"effect_id":"effect-B","run_id":"run-A"}}"#
    );

    #[test]
    fn the_producers_reference_reads_into_this_products_terms() {
        let r = WireReference::decode(PRODUCED.as_bytes()).unwrap();
        assert!(r.supported());
        assert_eq!(r.producer.name, Stated::known("wire-witness".into()));
        assert_eq!(r.producer.revision, Stated::known("0.1.0".into()));
        assert_eq!(
            r.subject,
            Stated::known(Subject {
                run_id: "run-A".into(),
                attempt: 2,
                sidecar_effect_id: "effect-B".into()
            })
        );
        assert!(r.as_reference().matches(b"manifest"));
    }

    #[test]
    fn an_attempt_of_zero_is_not_a_subject() {
        let zero = PRODUCED.replace(r#""attempt":2"#, r#""attempt":0"#);
        let r = WireReference::decode(zero.as_bytes()).unwrap();
        assert_eq!(r.subject, Stated::Unknown);
    }

    #[test]
    fn a_missing_digest_names_the_member_and_not_its_value() {
        let gone = PRODUCED.replace(r#""sha256":"#, r#""sha512":"#);
        assert_eq!(
            WireReference::decode(gone.as_bytes()),
            Err(ReferenceError::Member("sha256"))
        );
    }
}
