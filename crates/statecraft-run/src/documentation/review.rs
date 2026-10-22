//! Documentation review and correction records: spec 021 sections 3.1, 3.4,
//! 3.5 and 3.8, through its additive `extends` edge on this crate.
//!
//! A review (`statecraft/documentation-review/1`) and a correction
//! (`statecraft/documentation-correction/1`) are closed, digest-sealed,
//! append-only records. Each names the immutable core tuple it judges or
//! changes; neither names itself, and nothing here rewrites one. Whether a
//! reviewer has authority, and what the records mean together, is
//! `statecraft-acceptance`'s judgment; this module keeps the bytes and reads
//! them back in record order.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use statecraft_envelope::hash::sha256_hex;

use super::{DocumentError, decode_closed, digest_form, logical_path, nonempty, safe_id, unique};
use crate::context::Bounds;
use crate::record::{Chain, Entry, Identity as RecordIdentity, Kind, RecordError};

/// The review's closed schema word.
pub const REVIEW_SCHEMA: &str = "statecraft/documentation-review/1";
/// The correction's closed schema word.
pub const CORRECTION_SCHEMA: &str = "statecraft/documentation-correction/1";
/// The construction a review or correction digest is computed by.
pub const RECORD_DIGEST_CONSTRUCTION: &str = "statecraft/documentation-record-digest/1";
/// The subject of a recorded review.
pub const REVIEW_SUBJECT: &str = "documentation-review";
/// The subject of a recorded correction.
pub const CORRECTION_SUBJECT: &str = "documentation-correction";

/// Raw-byte limits on a review or a correction.
pub const RECORD_BOUNDS: Bounds = Bounds {
    total_bytes: 4 * 1024 * 1024,
    pages: 1,
    members: 262_144,
    depth: 16,
    string_bytes: 64 * 1024,
};

/// One output artifact, by its logical path and the digest of its bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactIdentity {
    /// The logical path.
    pub logical_path: String,
    /// `sha256:` over the exact bytes.
    pub digest: String,
}

/// Section 3.1's immutable core tuple.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct CoreTuple {
    pub repository: String,
    pub trusted_base: String,
    pub run_id: String,
    pub attempt: u32,
    /// The prepared workspace's candidate revision.
    pub candidate: String,
    pub context_manifest_digest: String,
    pub task_digest: String,
    /// The raw provider result's custody digest.
    pub result_custody_digest: String,
    pub result_schema_digest: String,
    /// Every output artifact, in the result's order.
    pub artifacts: Vec<ArtifactIdentity>,
    /// Admitted evidence references, in order.
    pub evidence: Vec<String>,
    /// The acceptance policy's digest at the trusted base.
    pub policy_digest: String,
}

/// The authority a reviewer claims; version 1 knows two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum Authority {
    RepositoryOwner,
    OwnerDelegatedReviewer,
}

/// Who reviewed, under what authority, according to whom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Reviewer {
    /// The reviewer's identity.
    pub identity: String,
    /// The authority claimed.
    pub authority: Authority,
    /// Where the authority comes from: the trusted-base policy's entry.
    pub authority_source: String,
}

/// Section 3.5's claim dispositions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum ClaimDisposition {
    Supported,
    Unsupported,
    Contradicted,
    InsufficientGrade,
    NeedsCorrection,
    NotReviewed,
}

/// One claim's disposition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaimReview {
    /// The claim.
    pub claim_id: String,
    /// The disposition.
    pub disposition: ClaimDisposition,
    /// Why, in order.
    pub reasons: Vec<String>,
}

/// An artifact's disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum ArtifactDisposition {
    Acceptable,
    NeedsCorrection,
    Rejected,
    NotReviewed,
}

/// One artifact's disposition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactReview {
    /// The logical path.
    pub logical_path: String,
    /// The disposition.
    pub disposition: ArtifactDisposition,
    /// Why, in order.
    pub reasons: Vec<String>,
}

/// A correction the reviewer requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestedCorrection {
    /// A claim id or an artifact's logical path.
    pub target: String,
    /// What must change.
    pub request: String,
}

/// The overall recommendation: an input to acceptance, never acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum Recommendation {
    Accept,
    Revise,
    Reject,
}

/// One `statecraft/documentation-review/1` record (section 3.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct Review {
    pub schema: String,
    pub subject: CoreTuple,
    pub reviewer: Reviewer,
    /// `YYYY-MM-DDTHH:MM:SSZ`.
    pub reviewed_at: String,
    pub claims: Vec<ClaimReview>,
    pub artifacts: Vec<ArtifactReview>,
    pub corrections: Vec<RequestedCorrection>,
    pub recommendation: Recommendation,
    /// Always [`RECORD_DIGEST_CONSTRUCTION`].
    pub digest_construction: String,
    /// The digest of every other member.
    pub review_digest: String,
}

/// One `statecraft/documentation-correction/1` record (section 3.8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct Correction {
    pub schema: String,
    /// The subject the correction changes.
    pub subject: CoreTuple,
    /// The digest it supersedes: the result's custody digest or an
    /// artifact's digest.
    pub supersedes: String,
    pub author: String,
    pub reason: String,
    pub changed_claims: Vec<String>,
    /// The new artifact bytes' identities.
    pub changed_artifacts: Vec<ArtifactIdentity>,
    /// The corrected result's custody digest, when the result itself changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrected_result: Option<String>,
    /// Always [`RECORD_DIGEST_CONSTRUCTION`].
    pub digest_construction: String,
    /// The digest of every other member.
    pub correction_digest: String,
}

fn sealed_digest<T: Serialize>(unsigned: &T) -> String {
    let mut bytes = RECORD_DIGEST_CONSTRUCTION.as_bytes().to_vec();
    bytes.push(b'\n');
    bytes.extend(serde_json::to_vec(unsigned).expect("a record serializes"));
    format!("sha256:{}", sha256_hex(&bytes))
}

fn timestamp(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 20
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            19 => *c == b'Z',
            _ => c.is_ascii_digit(),
        })
}

impl CoreTuple {
    fn validate(&self) -> Result<(), DocumentError> {
        nonempty("repository", &self.repository)?;
        nonempty("trusted base", &self.trusted_base)?;
        safe_id("run id", &self.run_id)?;
        if self.attempt == 0 {
            return Err(DocumentError::Invalid("attempt 0".into()));
        }
        nonempty("candidate", &self.candidate)?;
        for (what, d) in [
            ("context manifest digest", &self.context_manifest_digest),
            ("task digest", &self.task_digest),
            ("result custody digest", &self.result_custody_digest),
            ("result schema digest", &self.result_schema_digest),
            ("policy digest", &self.policy_digest),
        ] {
            digest_form(what, d)?;
        }
        for a in &self.artifacts {
            logical_path(&a.logical_path)?;
            digest_form("artifact digest", &a.digest)?;
        }
        unique(
            "artifact",
            self.artifacts.iter().map(|a| a.logical_path.as_str()),
        )?;
        for e in &self.evidence {
            nonempty("evidence reference", e)?;
        }
        unique(
            "evidence reference",
            self.evidence.iter().map(String::as_str),
        )?;
        Ok(())
    }
}

impl Review {
    /// The digest of every member but `reviewDigest`.
    pub fn computed_digest(&self) -> String {
        let mut unsigned = self.clone();
        unsigned.review_digest = String::new();
        sealed_digest(&unsigned)
    }

    /// Set the construction and the digest from the other members.
    pub fn seal(mut self) -> Self {
        self.digest_construction = RECORD_DIGEST_CONSTRUCTION.to_string();
        self.review_digest = self.computed_digest();
        self
    }

    /// Decode raw review bytes under the closed schema and validate them.
    pub fn decode(bytes: &[u8]) -> Result<Self, DocumentError> {
        let review: Self = decode_closed(bytes, &RECORD_BOUNDS)?;
        review.validate()?;
        Ok(review)
    }

    /// Every rule a decoded review must meet on its own.
    pub fn validate(&self) -> Result<(), DocumentError> {
        if self.schema != REVIEW_SCHEMA {
            return Err(DocumentError::Malformed(format!(
                "schema {:?} is not {REVIEW_SCHEMA}",
                self.schema
            )));
        }
        if self.digest_construction != RECORD_DIGEST_CONSTRUCTION {
            return Err(DocumentError::Malformed("digest construction".into()));
        }
        self.subject.validate()?;
        nonempty("reviewer", &self.reviewer.identity)?;
        nonempty("authority source", &self.reviewer.authority_source)?;
        if !timestamp(&self.reviewed_at) {
            return Err(DocumentError::Invalid(format!(
                "reviewed at {:?}",
                self.reviewed_at
            )));
        }
        for c in &self.claims {
            safe_id("claim id", &c.claim_id)?;
        }
        unique("claim", self.claims.iter().map(|c| c.claim_id.as_str()))?;
        for a in &self.artifacts {
            logical_path(&a.logical_path)?;
        }
        unique(
            "artifact",
            self.artifacts.iter().map(|a| a.logical_path.as_str()),
        )?;
        for c in &self.corrections {
            nonempty("correction target", &c.target)?;
            nonempty("correction request", &c.request)?;
        }
        let computed = self.computed_digest();
        if computed != self.review_digest {
            return Err(DocumentError::Digest {
                stated: self.review_digest.clone(),
                computed,
            });
        }
        Ok(())
    }
}

impl Correction {
    /// The digest of every member but `correctionDigest`.
    pub fn computed_digest(&self) -> String {
        let mut unsigned = self.clone();
        unsigned.correction_digest = String::new();
        sealed_digest(&unsigned)
    }

    /// Set the construction and the digest from the other members.
    pub fn seal(mut self) -> Self {
        self.digest_construction = RECORD_DIGEST_CONSTRUCTION.to_string();
        self.correction_digest = self.computed_digest();
        self
    }

    /// Decode raw correction bytes under the closed schema and validate them.
    pub fn decode(bytes: &[u8]) -> Result<Self, DocumentError> {
        let correction: Self = decode_closed(bytes, &RECORD_BOUNDS)?;
        correction.validate()?;
        Ok(correction)
    }

    /// Every rule a decoded correction must meet on its own.
    pub fn validate(&self) -> Result<(), DocumentError> {
        if self.schema != CORRECTION_SCHEMA {
            return Err(DocumentError::Malformed(format!(
                "schema {:?} is not {CORRECTION_SCHEMA}",
                self.schema
            )));
        }
        if self.digest_construction != RECORD_DIGEST_CONSTRUCTION {
            return Err(DocumentError::Malformed("digest construction".into()));
        }
        self.subject.validate()?;
        digest_form("supersedes", &self.supersedes)?;
        nonempty("author", &self.author)?;
        nonempty("reason", &self.reason)?;
        for c in &self.changed_claims {
            safe_id("changed claim", c)?;
        }
        for a in &self.changed_artifacts {
            logical_path(&a.logical_path)?;
            digest_form("changed artifact digest", &a.digest)?;
        }
        unique(
            "changed claim",
            self.changed_claims.iter().map(String::as_str),
        )?;
        unique(
            "changed artifact",
            self.changed_artifacts
                .iter()
                .map(|a| a.logical_path.as_str()),
        )?;
        if let Some(r) = &self.corrected_result {
            digest_form("corrected result", r)?;
        }
        if self.changed_claims.is_empty()
            && self.changed_artifacts.is_empty()
            && self.corrected_result.is_none()
        {
            return Err(DocumentError::Invalid(
                "a correction changes at least one claim, artifact or result".into(),
            ));
        }
        let computed = self.computed_digest();
        if computed != self.correction_digest {
            return Err(DocumentError::Digest {
                stated: self.correction_digest.clone(),
                computed,
            });
        }
        Ok(())
    }
}

fn append(
    chain: &mut Chain,
    run_id: &str,
    attempt: u32,
    subject: &str,
    record: Value,
    timestamp: &str,
) -> Result<(), RecordError> {
    let n = chain.entries().len();
    chain.append(
        &format!("{run_id}/{attempt}/{subject}/{n}"),
        timestamp,
        &Entry {
            kind: Kind::Evidence,
            run_id: run_id.to_string(),
            attempt,
            subject: subject.to_string(),
            idempotency_key: None,
            detail: serde_json::json!({ "record": record }),
            effect_id: RecordIdentity::Absent,
        },
    )
}

/// Append a validated review to its subject's attempt. A review is never
/// rewritten; a second review is another record.
pub fn record_review(
    chain: &mut Chain,
    review: &Review,
    timestamp: &str,
) -> Result<(), RecordError> {
    append(
        chain,
        &review.subject.run_id,
        review.subject.attempt,
        REVIEW_SUBJECT,
        serde_json::to_value(review).expect("a review serializes"),
        timestamp,
    )
}

/// Append a validated correction to its subject's attempt.
pub fn record_correction(
    chain: &mut Chain,
    correction: &Correction,
    timestamp: &str,
) -> Result<(), RecordError> {
    append(
        chain,
        &correction.subject.run_id,
        correction.subject.attempt,
        CORRECTION_SUBJECT,
        serde_json::to_value(correction).expect("a correction serializes"),
        timestamp,
    )
}

/// One record as the chain holds it: valid, or unreadable with why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Read<T> {
    /// It decodes and its digest holds.
    Valid(T),
    /// It does not; never dropped and never repaired.
    Unreadable(String),
}

fn read_all<T: serde::de::DeserializeOwned>(
    chain: &Chain,
    run_id: &str,
    attempt: u32,
    subject: &str,
    validate: impl Fn(&T) -> Result<(), DocumentError>,
) -> Vec<Read<T>> {
    chain
        .entries()
        .into_iter()
        .filter(|e| {
            e.kind == Kind::Evidence
                && e.subject == subject
                && e.run_id == run_id
                && e.attempt == attempt
        })
        .map(|e| match e.detail.get("record").cloned() {
            None => Read::Unreadable("the record carries no document".into()),
            Some(v) => match serde_json::from_value::<T>(v) {
                Err(e) => Read::Unreadable(e.to_string()),
                Ok(t) => match validate(&t) {
                    Ok(()) => Read::Valid(t),
                    Err(e) => Read::Unreadable(e.to_string()),
                },
            },
        })
        .collect()
}

/// Every review recorded for the attempt, in record order.
pub fn recorded_reviews(chain: &Chain, run_id: &str, attempt: u32) -> Vec<Read<Review>> {
    read_all(chain, run_id, attempt, REVIEW_SUBJECT, Review::validate)
}

/// Every correction recorded for the attempt, in record order.
pub fn recorded_corrections(chain: &Chain, run_id: &str, attempt: u32) -> Vec<Read<Correction>> {
    read_all(
        chain,
        run_id,
        attempt,
        CORRECTION_SUBJECT,
        Correction::validate,
    )
}

/// One review or correction as `run show` names it, without judging it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum Listed {
    /// A review.
    Review {
        /// Its digest.
        digest: String,
        /// Who.
        reviewer: String,
        /// The recommendation, as the reviewer gave it.
        recommendation: Recommendation,
    },
    /// A correction.
    Correction {
        /// Its digest.
        digest: String,
        /// What it supersedes.
        supersedes: String,
    },
    /// A record that could not be read.
    Unreadable {
        /// Why.
        reason: String,
    },
}

/// The review and correction identities of one attempt, reviews first, each
/// in record order (spec 021 section 3.10).
pub fn identities(chain: &Chain, run_id: &str, attempt: u32) -> Vec<Listed> {
    let mut out: Vec<Listed> = recorded_reviews(chain, run_id, attempt)
        .into_iter()
        .map(|r| match r {
            Read::Valid(r) => Listed::Review {
                digest: r.review_digest,
                reviewer: r.reviewer.identity,
                recommendation: r.recommendation,
            },
            Read::Unreadable(reason) => Listed::Unreadable { reason },
        })
        .collect();
    out.extend(
        recorded_corrections(chain, run_id, attempt)
            .into_iter()
            .map(|c| match c {
                Read::Valid(c) => Listed::Correction {
                    digest: c.correction_digest,
                    supersedes: c.supersedes,
                },
                Read::Unreadable(reason) => Listed::Unreadable { reason },
            }),
    );
    out
}
