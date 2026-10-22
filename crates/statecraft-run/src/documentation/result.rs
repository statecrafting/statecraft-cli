//! The result half of spec 020: `statecraft/documentation-result/1`, the
//! binding a result must repeat, the raw bytes kept before decoding, and the
//! `run show` projection (sections 3.4 to 3.9).

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    DocumentError, DocumentationTask, RESULT_BOUNDS, RESULT_SCHEMA, RESULT_SUBJECT, decode_closed,
    digest_form, logical_path, nonempty, recorded_task, safe_id, unique,
};
use crate::context::{self, CustodyError};
use crate::record::{Chain, Entry, Identity as RecordIdentity, Kind, RecordError};

/// An adapter's or a provider's identity and version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Party {
    /// The name.
    pub name: String,
    /// The version.
    pub version: String,
}

/// One output artifact descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct ArtifactDescriptor {
    pub logical_path: String,
    pub media_type: String,
    pub digest: String,
    pub size: u64,
    /// A reference to the exact raw bytes.
    pub raw_ref: String,
}

/// A reference into the retained spec 019 packets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct Citation {
    pub repository: String,
    pub page_digest: String,
    pub member: String,
    pub content_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<(usize, usize)>,
}

/// A claim's lifecycle grade: an assertion needing support at exactly it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum LifecycleGrade {
    Specified,
    Approved,
    Implemented,
    Tested,
    Released,
}

/// The sentinel `not-reviewed`, which the provider emits and nobody rewrites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotReviewed {
    /// The only value.
    #[serde(rename = "not-reviewed")]
    NotReviewed,
}

/// The sentinel `not-attempted`, which the provider emits and nobody
/// rewrites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotAttempted {
    /// The only value.
    #[serde(rename = "not-attempted")]
    NotAttempted,
}

/// One claim (section 3.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct Claim {
    pub claim_id: String,
    pub text: String,
    pub lifecycle_grade: LifecycleGrade,
    pub citations: Vec<Citation>,
    pub evidence_refs: Vec<String>,
    /// A reason, or `not-applicable`.
    pub unsupported_reason: String,
    pub contradiction_refs: Vec<String>,
    pub review_disposition: NotReviewed,
}

/// An unknown: never replaced by a guess.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct Unknown {
    pub unknown_id: String,
    pub question: String,
    pub missing_fact: String,
    pub reason: String,
}

/// A contradiction: every incompatible reference, and no winner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct Contradiction {
    pub contradiction_id: String,
    pub claim_refs: Vec<String>,
    pub source_refs: Vec<Citation>,
    pub description: String,
}

/// A claim and the support it lacks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct Unsupported {
    pub claim_id: String,
    pub missing_support: String,
}

/// A task-required field or artifact the result lacks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct Omission {
    pub member: String,
    pub reason: String,
}

/// A citation and its exact spec 019 resolution state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct CitationFailure {
    pub citation: Citation,
    pub resolution: String,
}

/// An execution evidence reference and its claimed purpose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct EvidenceRef {
    pub reference: String,
    pub purpose: String,
}

/// What the provider says about completion: its claim, not acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum ProviderCompletion {
    ClaimedComplete,
    ClaimedIncomplete,
    NotReported,
}

/// One `statecraft/documentation-result/1` document (section 3.5). Every
/// collection is required, even when empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct DocumentationResult {
    pub schema: String,
    pub task_id: String,
    pub run_id: String,
    pub attempt: u32,
    pub task_digest: String,
    pub context_manifest_digest: String,
    pub adapter: Party,
    pub provider: Party,
    pub artifacts: Vec<ArtifactDescriptor>,
    pub claims: Vec<Claim>,
    pub unknowns: Vec<Unknown>,
    pub contradictions: Vec<Contradiction>,
    pub unsupported_claims: Vec<Unsupported>,
    pub provider_omissions: Vec<Omission>,
    pub statecraft_citation_failures: Vec<CitationFailure>,
    pub execution_evidence: Vec<EvidenceRef>,
    pub provider_completion: ProviderCompletion,
    pub review_disposition: NotReviewed,
    pub acceptance_result: NotAttempted,
}

impl DocumentationResult {
    /// Decode raw result bytes under the closed schema and check them against
    /// the task they answer (section 3.5): unknown members, duplicate
    /// identifiers, path escapes, a sentinel other than its literal, and a
    /// reference outside the task all refuse.
    pub fn decode(bytes: &[u8], task: &DocumentationTask) -> Result<Self, DocumentError> {
        let result: Self = decode_closed(bytes, &RESULT_BOUNDS)?;
        if result.schema != RESULT_SCHEMA {
            return Err(DocumentError::Malformed(format!(
                "schema {:?} is not {RESULT_SCHEMA}",
                result.schema
            )));
        }
        safe_id("task id", &result.task_id)?;
        safe_id("run id", &result.run_id)?;
        digest_form("task digest", &result.task_digest)?;
        digest_form("context manifest digest", &result.context_manifest_digest)?;
        let expected: BTreeSet<&str> = task.artifacts.iter().map(|a| a.path.as_str()).collect();
        for a in &result.artifacts {
            logical_path(&a.logical_path)?;
            digest_form("artifact digest", &a.digest)?;
            if !expected.contains(a.logical_path.as_str()) {
                return Err(DocumentError::OutsideTask(format!(
                    "artifact {} is not one the task expects",
                    a.logical_path
                )));
            }
        }
        unique(
            "artifact",
            result.artifacts.iter().map(|a| a.logical_path.as_str()),
        )?;
        for c in &result.claims {
            safe_id("claim id", &c.claim_id)?;
            nonempty("claim text", &c.text)?;
            nonempty("unsupported reason", &c.unsupported_reason)?;
        }
        // Every citation's digests have the one form, wherever it appears.
        let citations = result
            .claims
            .iter()
            .flat_map(|c| c.citations.iter())
            .chain(
                result
                    .contradictions
                    .iter()
                    .flat_map(|c| c.source_refs.iter()),
            )
            .chain(
                result
                    .statecraft_citation_failures
                    .iter()
                    .map(|f| &f.citation),
            );
        for c in citations {
            safe_id("citation repository", &c.repository)?;
            digest_form("citation page digest", &c.page_digest)?;
            digest_form("citation content digest", &c.content_digest)?;
            nonempty("citation member", &c.member)?;
        }
        unique("claim", result.claims.iter().map(|c| c.claim_id.as_str()))?;
        unique(
            "unknown",
            result.unknowns.iter().map(|u| u.unknown_id.as_str()),
        )?;
        unique(
            "contradiction",
            result
                .contradictions
                .iter()
                .map(|c| c.contradiction_id.as_str()),
        )?;
        let claims: BTreeSet<&str> = result.claims.iter().map(|c| c.claim_id.as_str()).collect();
        let contradictions: BTreeSet<&str> = result
            .contradictions
            .iter()
            .map(|c| c.contradiction_id.as_str())
            .collect();
        for c in &result.claims {
            for r in &c.contradiction_refs {
                if !contradictions.contains(r.as_str()) {
                    return Err(DocumentError::OutsideTask(format!(
                        "claim {} names contradiction {r}, which the result does not hold",
                        c.claim_id
                    )));
                }
            }
        }
        let evidence: BTreeSet<&str> = result
            .execution_evidence
            .iter()
            .map(|e| e.reference.as_str())
            .collect();
        for c in &result.claims {
            for r in &c.evidence_refs {
                if !evidence.contains(r.as_str()) {
                    return Err(DocumentError::OutsideTask(format!(
                        "claim {} names evidence {r}, which the result does not hold",
                        c.claim_id
                    )));
                }
            }
        }
        for c in &result.contradictions {
            for r in &c.claim_refs {
                if !claims.contains(r.as_str()) {
                    return Err(DocumentError::OutsideTask(format!(
                        "contradiction {} names claim {r}, which the result does not hold",
                        c.contradiction_id
                    )));
                }
            }
        }
        for u in &result.unsupported_claims {
            if !claims.contains(u.claim_id.as_str()) {
                return Err(DocumentError::OutsideTask(format!(
                    "unsupported claim {} is not a claim of the result",
                    u.claim_id
                )));
            }
        }
        Ok(result)
    }

    /// The claims that carry neither a citation nor an evidence reference.
    /// Each must say why (section 3.6); a claim that does not is returned
    /// here and stays unsupported however the provider marks completion.
    pub fn unexplained_claims(&self) -> Vec<&str> {
        self.claims
            .iter()
            .filter(|c| {
                c.citations.is_empty()
                    && c.evidence_refs.is_empty()
                    && c.unsupported_reason == "not-applicable"
            })
            .map(|c| c.claim_id.as_str())
            .collect()
    }
}

/// What a result must repeat to bind to an attempt (section 3.8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Binding {
    /// The task.
    pub task_id: String,
    /// The run.
    pub run_id: String,
    /// The attempt.
    pub attempt: u32,
    /// The task digest.
    pub task_digest: String,
    /// The context manifest digest.
    pub context_manifest_digest: String,
    /// The adapter the attempt record names.
    pub adapter: Party,
    /// The provider the attempt record names.
    pub provider: Party,
}

impl Binding {
    /// The task's half of a binding, with the attempt's adapter and provider.
    pub fn of(task: &DocumentationTask, adapter: Party, provider: Party) -> Self {
        Self {
            task_id: task.task_id.clone(),
            run_id: task.run_id.clone(),
            attempt: task.attempt,
            task_digest: task.task_digest.clone(),
            context_manifest_digest: task.context_manifest_digest.clone(),
            adapter,
            provider,
        }
    }

    /// The first member the result does not repeat, if any.
    pub fn mismatch(&self, result: &DocumentationResult) -> Option<&'static str> {
        if result.task_id != self.task_id {
            Some("taskId")
        } else if result.run_id != self.run_id {
            Some("runId")
        } else if result.attempt != self.attempt {
            Some("attempt")
        } else if result.task_digest != self.task_digest {
            Some("taskDigest")
        } else if result.context_manifest_digest != self.context_manifest_digest {
            Some("contextManifestDigest")
        } else if result.adapter != self.adapter {
            Some("adapter")
        } else if result.provider != self.provider {
            Some("provider")
        } else {
            None
        }
    }
}

/// The raw bytes as retained, before any decoding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Raw {
    /// The media type the adapter reported.
    pub media_type: String,
    /// The exact length.
    pub byte_length: u64,
    /// `sha256:` over exactly the bytes.
    pub custody_digest: String,
}

/// The counts `run show` renders for a valid, bound result (section 3.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct Summary {
    pub provider_completion: ProviderCompletion,
    pub artifacts: Vec<ArtifactDescriptor>,
    pub claims: usize,
    pub unknowns: usize,
    pub contradictions: usize,
    pub unsupported_claims: usize,
    pub provider_omissions: usize,
    pub statecraft_citation_failures: usize,
    /// Claims with no support and no stated reason.
    pub unexplained_claims: usize,
}

impl Summary {
    fn of(result: &DocumentationResult) -> Self {
        Self {
            provider_completion: result.provider_completion,
            artifacts: result.artifacts.clone(),
            claims: result.claims.len(),
            unknowns: result.unknowns.len(),
            contradictions: result.contradictions.len(),
            unsupported_claims: result.unsupported_claims.len(),
            provider_omissions: result.provider_omissions.len(),
            statecraft_citation_failures: result.statecraft_citation_failures.len(),
            unexplained_claims: result.unexplained_claims().len(),
        }
    }
}

/// What the supervisor found when it decoded the retained bytes, recorded
/// once beside them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum Validation {
    /// The bytes are a result that binds this attempt.
    Bound {
        /// Its counts.
        summary: Summary,
    },
    /// The bytes are not the closed schema; they stay retained as returned.
    SchemaInvalid {
        /// The refusal, with its code word.
        reason: String,
    },
    /// A valid result for another binding: a finding, never bound.
    Unbound {
        /// The first member that differs.
        field: String,
    },
}

/// Why a result could not be recorded.
#[derive(Debug, thiserror::Error)]
pub enum RecordResultError {
    /// The bytes could not be kept.
    #[error(transparent)]
    Custody(#[from] CustodyError),
    /// The record could not be appended.
    #[error(transparent)]
    Record(#[from] RecordError),
}

/// Section 3.4 and 3.8: retain the exact raw bytes first, then decode and
/// bind them, then append one evidence record carrying the custody digest
/// and what the decoding found. A failure never replaces the bytes.
pub fn record_result(
    home: &Path,
    chain: &mut Chain,
    task: &DocumentationTask,
    binding: &Binding,
    media_type: &str,
    bytes: &[u8],
    timestamp: &str,
) -> Result<Validation, RecordResultError> {
    let custody = context::retain(home, bytes)?;
    let validation = match DocumentationResult::decode(bytes, task) {
        Err(e) => Validation::SchemaInvalid {
            reason: e.to_string(),
        },
        Ok(result) => match binding.mismatch(&result) {
            Some(field) => Validation::Unbound {
                field: field.to_string(),
            },
            None => Validation::Bound {
                summary: Summary::of(&result),
            },
        },
    };
    let raw = Raw {
        media_type: media_type.to_string(),
        byte_length: bytes.len() as u64,
        custody_digest: custody,
    };
    let n = chain.entries().len();
    chain.append(
        &format!("{}/{}/{RESULT_SUBJECT}/{n}", task.run_id, task.attempt),
        timestamp,
        &Entry {
            kind: Kind::Evidence,
            run_id: task.run_id.clone(),
            attempt: task.attempt,
            subject: RESULT_SUBJECT.to_string(),
            idempotency_key: None,
            detail: serde_json::json!({
                "binding": binding,
                "raw": raw,
                "validation": validation,
            }),
            effect_id: RecordIdentity::Absent,
        },
    )?;
    Ok(validation)
}

/// One recorded result, as the chain holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Recorded {
    /// The raw bytes' custody.
    pub raw: Raw,
    /// What decoding found.
    pub validation: Validation,
}

/// The result half of the projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum ResultProjection {
    /// No result was recorded for the attempt.
    NotRecorded,
    /// Exactly one, with what decoding found.
    Recorded(Recorded),
    /// More than one terminal result: a finding, and none is chosen.
    Duplicate {
        /// Every recorded result's custody, in record order.
        custody: Vec<String>,
    },
    /// A record that names this attempt could not be read.
    Unreadable {
        /// Why.
        reason: String,
    },
}

/// The task's identity as `run show` renders it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskIdentity {
    /// The task id.
    pub task_id: String,
    /// The task digest.
    pub task_digest: String,
}

/// One attempt's documentation, as `run show` renders it (section 3.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum Projection {
    /// The intent carries no task; a historical record renders this.
    NotRecorded,
    /// The intent carries a value that binds nothing.
    Unreadable {
        /// Why.
        reason: String,
    },
    /// The task, the adapter's delivery testimony, and the result.
    Recorded {
        /// The task's identity.
        task: TaskIdentity,
        /// The delivery fold, or `None` when none was recorded.
        delivery: Option<Box<super::delivery::Delivery>>,
        /// The result.
        result: ResultProjection,
    },
}

fn recorded_results(chain: &Chain, task: &DocumentationTask) -> ResultProjection {
    let mut found = Vec::new();
    for e in chain.entries() {
        if e.kind != Kind::Evidence
            || e.subject != RESULT_SUBJECT
            || e.run_id != task.run_id
            || e.attempt != task.attempt
        {
            continue;
        }
        let digest = e
            .detail
            .get("binding")
            .and_then(|b| b.get("taskDigest"))
            .and_then(Value::as_str);
        if digest != Some(task.task_digest.as_str()) {
            continue;
        }
        let raw = e.detail.get("raw").cloned().map(serde_json::from_value);
        let validation = e
            .detail
            .get("validation")
            .cloned()
            .map(serde_json::from_value);
        match (raw, validation) {
            (Some(Ok(raw)), Some(Ok(validation))) => found.push(Recorded { raw, validation }),
            _ => {
                return ResultProjection::Unreadable {
                    reason: "a result record for this attempt is unreadable".to_string(),
                };
            }
        }
    }
    match found.len() {
        0 => ResultProjection::NotRecorded,
        1 => ResultProjection::Recorded(found.remove(0)),
        _ => ResultProjection::Duplicate {
            custody: found.into_iter().map(|r| r.raw.custody_digest).collect(),
        },
    }
}

/// Fold one attempt's documentation from the chain and nothing else. It
/// performs no validation or review: it reads what was recorded.
pub fn project(chain: &Chain, run_id: &str, attempt: u32) -> Projection {
    match recorded_task(chain, run_id, attempt) {
        Ok(None) => Projection::NotRecorded,
        Err(reason) => Projection::Unreadable { reason },
        Ok(Some(task)) => Projection::Recorded {
            task: TaskIdentity {
                task_id: task.task_id.clone(),
                task_digest: task.task_digest.clone(),
            },
            delivery: super::delivery::recorded_delivery(chain, &task).map(Box::new),
            result: recorded_results(chain, &task),
        },
    }
}
