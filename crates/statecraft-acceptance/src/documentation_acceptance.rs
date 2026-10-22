//! Independent documentation acceptance: spec 021, through its additive
//! `extends` edge on this crate (spec 005 owns it).
//!
//! [`judge`] reports section 3.3's twelve dimensions separately, each `pass`,
//! `fail`, `unknown`, `unsupported`, `unreadable` or `not-recorded` with its
//! reasons, before deriving `accepted`, `rejected` or `no-acceptance`
//! (section 3.9). It reads only what it is handed: the recorded task and
//! result, the delivery folds, spec 019's judged context facts, the retained
//! artifact bytes, the review and correction records, the trusted-base
//! policy and the candidate's changed paths. It fetches nothing, repairs
//! nothing, and never picks the more favorable of two records.
//!
//! No row is copied into another: a resolved citation is not semantic
//! support, adapter testimony is not transport, a recommendation is not
//! acceptance, and acceptance authorizes no application or publication.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use statecraft_run::context::custody_digest;
use statecraft_run::documentation::delivery::Delivery as DocumentationDelivery;
use statecraft_run::documentation::review::{
    ArtifactDisposition, ArtifactIdentity, Authority, ClaimDisposition, CoreTuple, Correction,
    Read, Recommendation, Review,
};
use statecraft_run::documentation::{DocumentationResult, DocumentationTask, SemanticReview};

use crate::context::{CitationState, ContextResult, FreshnessState};

/// The trusted-base policy's closed schema word.
pub const POLICY_SCHEMA: &str = "statecraft/documentation-acceptance-policy/1";
/// Where the policy is read, at the trusted base.
pub const POLICY_PATH: &str = ".statecraft/documentation-acceptance.json";
/// What every judgment repeats (section 3.9).
pub const BOUNDARY_NOTICE: &str = "acceptance does not authorize applying a patch, modifying an \
     authored file, committing, pushing, opening a pull request, publishing, releasing, or deploying";

/// Whether a dimension is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum Need {
    Required,
    Optional,
}

/// One reviewer grant in the trusted-base policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Grant {
    /// The reviewer's identity.
    pub identity: String,
    /// The authority granted.
    pub authority: Authority,
    /// `YYYY-MM-DDTHH:MM:SSZ` after which the grant is expired.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_after: Option<String>,
}

/// The trusted-base acceptance policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Policy {
    /// Always [`POLICY_SCHEMA`].
    pub schema: String,
    /// Who may review, under which authority.
    pub reviewers: Vec<Grant>,
    /// How many eligible reviews must recommend `accept`.
    pub quorum: u32,
    /// Whether independent transport evidence is required.
    pub observed_transport: Need,
    /// The largest artifact accepted, in bytes.
    pub max_artifact_bytes: u64,
}

impl Policy {
    /// Decode the policy's exact bytes; the digest is theirs.
    pub fn decode(bytes: &[u8]) -> Result<(Self, String), String> {
        let policy: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if policy.schema != POLICY_SCHEMA {
            return Err(format!("schema {:?} is not {POLICY_SCHEMA}", policy.schema));
        }
        if policy.quorum == 0 {
            return Err("quorum 0".into());
        }
        Ok((policy, custody_digest(bytes)))
    }
}

/// Section 3.3's states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum State {
    Pass,
    Fail,
    Unknown,
    Unsupported,
    Unreadable,
    NotRecorded,
}

/// Section 3.3's twelve dimensions, in its order. Serialized as values, so
/// kebab-case under spec 006 (section 5, 2026-10-22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum Dimension {
    TaskBinding,
    IntendedDelivery,
    AdapterObservedDelivery,
    ObservedTransport,
    SchemaValidation,
    CitationResolution,
    EvidenceAdmission,
    SemanticReview,
    ArtifactValidation,
    ContextFreshness,
    Authority,
    PublicationBoundary,
}

/// One dimension's result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DimensionResult {
    /// Which.
    pub dimension: Dimension,
    /// Whether the policy requires it.
    pub need: Need,
    /// Its state.
    pub state: State,
    /// Every reason, with the subject it concerns.
    pub reasons: Vec<String>,
}

/// Section 3.9's overall result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum Outcome {
    Accepted,
    Rejected,
    NoAcceptance,
}

/// The documentation-manifest freshness the adopted producer answered
/// (spec-spine `documentation manifest freshness`), or its absence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum ManifestFreshness {
    Fresh,
    Stale,
    Incomplete,
    Unverified,
    Unsupported,
    Unreadable,
    NotRecorded,
}

/// Everything a judgment reads.
pub struct Inputs<'a> {
    /// The registered repository.
    pub repository: &'a str,
    /// The trusted base.
    pub trusted_base: &'a str,
    /// The prepared workspace's candidate revision; `Err` names why it could
    /// not be read. An unread candidate is never an identity.
    pub candidate: Result<&'a str, String>,
    /// The recorded task.
    pub task: &'a DocumentationTask,
    /// The 020 delivery fold, if recorded.
    pub delivery: Option<&'a DocumentationDelivery>,
    /// The raw result's custody digest, if one result was recorded.
    pub result_custody: Option<&'a str>,
    /// The decoded, bound result; `Err` names why there is none.
    pub result: Result<&'a DocumentationResult, String>,
    /// Spec 019's judged facts over the claims' citations and evidence.
    pub context: Option<&'a ContextResult>,
    /// The documentation-manifest freshness.
    pub manifest_freshness: ManifestFreshness,
    /// The retained bytes, by digest.
    pub retained: &'a dyn Fn(&str) -> Option<Vec<u8>>,
    /// Every review, in record order.
    pub reviews: &'a [Read<Review>],
    /// Every correction, in record order.
    pub corrections: &'a [Read<Correction>],
    /// The trusted-base policy and its digest; `Err` names why there is none.
    pub policy: Result<(&'a Policy, &'a str), String>,
    /// Paths the candidate changed against the base, when known.
    pub changed_paths: Option<&'a [String]>,
    /// The judgment time, `YYYY-MM-DDTHH:MM:SSZ`.
    pub now: &'a str,
}

/// One documentation acceptance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Judgment {
    /// The core tuple judged, after corrections; absent when it could not be
    /// identified.
    pub subject: Option<CoreTuple>,
    /// The reviews that bind that subject, by digest, in record order.
    pub bound_reviews: Vec<String>,
    /// The corrections applied, by digest, in record order.
    pub corrections: Vec<String>,
    /// Every dimension, in section 3.3's order.
    pub dimensions: Vec<DimensionResult>,
    /// The overall result.
    pub outcome: Outcome,
    /// Why, when it is not `accepted`.
    pub reasons: Vec<String>,
    /// Section 3.9's boundary, repeated.
    pub notice: &'static str,
}

/// `fail` dominates; otherwise the weaker state of the two.
fn combine(a: State, b: State) -> State {
    if a == State::Fail || b == State::Fail {
        State::Fail
    } else {
        a.max(b)
    }
}

fn dim(dimension: Dimension, need: Need, state: State, reasons: Vec<String>) -> DimensionResult {
    DimensionResult {
        dimension,
        need,
        state,
        reasons,
    }
}

/// Apply the corrections in record order to the result's artifacts: each
/// one that names the then-current subject replaces the identities it
/// changes. One naming another subject is skipped with a reason.
fn effective(
    base: CoreTuple,
    corrections: &[Read<Correction>],
    reasons: &mut Vec<String>,
) -> (CoreTuple, Vec<String>, Vec<String>) {
    let mut subject = base;
    let mut applied = Vec::new();
    let mut changed_claims = Vec::new();
    for c in corrections {
        match c {
            Read::Unreadable(why) => reasons.push(format!("correction-unreadable: {why}")),
            Read::Valid(c) if c.subject != subject => reasons.push(format!(
                "correction-not-binding: {} names another subject",
                c.correction_digest
            )),
            Read::Valid(c) => {
                for changed in &c.changed_artifacts {
                    match subject
                        .artifacts
                        .iter_mut()
                        .find(|a| a.logical_path == changed.logical_path)
                    {
                        Some(a) => a.digest = changed.digest.clone(),
                        None => subject.artifacts.push(changed.clone()),
                    }
                }
                if let Some(r) = &c.corrected_result {
                    subject.result_custody_digest = r.clone();
                }
                changed_claims.extend(c.changed_claims.iter().cloned());
                applied.push(c.correction_digest.clone());
            }
        }
    }
    (subject, applied, changed_claims)
}

fn eligible_grant<'p>(policy: &'p Policy, review: &Review, now: &str) -> Result<&'p Grant, String> {
    let matching: Vec<&Grant> = policy
        .reviewers
        .iter()
        .filter(|g| g.identity == review.reviewer.identity)
        .collect();
    match matching.as_slice() {
        [] => Err(format!(
            "{} has no grant in the trusted-base policy",
            review.reviewer.identity
        )),
        [g] if g.authority != review.reviewer.authority => Err(format!(
            "{} claims an authority the policy does not grant",
            review.reviewer.identity
        )),
        [g] if g.not_after.as_deref().is_some_and(|t| t < now) => {
            Err(format!("{}'s grant expired", review.reviewer.identity))
        }
        [g] => Ok(g),
        _ => Err(format!(
            "{} has conflicting grants",
            review.reviewer.identity
        )),
    }
}

fn freshness_state(
    context: Option<&ContextResult>,
    manifest: ManifestFreshness,
) -> (State, Vec<String>) {
    let mut reasons = Vec::new();
    let packets = match context {
        None => {
            reasons.push("packet freshness not-recorded".into());
            State::NotRecorded
        }
        Some(c) => match c.freshness_aggregate {
            FreshnessState::Fresh => State::Pass,
            FreshnessState::Stale => {
                reasons.push("packets stale".into());
                State::Fail
            }
            FreshnessState::Incomplete => {
                reasons.push("packets incomplete".into());
                State::Fail
            }
            FreshnessState::Unverified => {
                reasons.push("packets unverified".into());
                State::Unknown
            }
        },
    };
    let docs = match manifest {
        ManifestFreshness::Fresh => State::Pass,
        ManifestFreshness::Stale | ManifestFreshness::Incomplete => State::Fail,
        ManifestFreshness::Unverified => State::Unknown,
        ManifestFreshness::Unsupported => State::Unsupported,
        ManifestFreshness::Unreadable => State::Unreadable,
        ManifestFreshness::NotRecorded => State::NotRecorded,
    };
    if docs != State::Pass {
        reasons.push(format!(
            "documentation manifest {}",
            serde_json::to_value(manifest)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default()
        ));
    }
    (combine(packets, docs), reasons)
}

fn citation_state(context: Option<&ContextResult>, claimed: usize) -> (State, Vec<String>) {
    let Some(c) = context else {
        return if claimed == 0 {
            (State::Pass, vec![])
        } else {
            (
                State::NotRecorded,
                vec![format!("{claimed} citation(s) never resolved")],
            )
        };
    };
    if c.citations.len() != claimed {
        return (
            State::NotRecorded,
            vec![format!(
                "{claimed} citation(s) claimed, {} resolved",
                c.citations.len()
            )],
        );
    }
    let mut state = State::Pass;
    let mut reasons = Vec::new();
    for r in &c.citations {
        let s = match r.state {
            CitationState::Resolved => continue,
            CitationState::Missing
            | CitationState::DigestMismatch
            | CitationState::OutsidePacket => State::Fail,
            CitationState::Unsupported => State::Unsupported,
            CitationState::Unreadable => State::Unreadable,
            CitationState::NotRecorded => State::NotRecorded,
        };
        reasons.push(format!(
            "{} {}: {}",
            r.citation.page_digest,
            r.citation.member,
            serde_json::to_value(r.state)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default()
        ));
        state = combine(state, s);
    }
    (state, reasons)
}

fn evidence_state(context: Option<&ContextResult>, refs: &BTreeSet<&str>) -> (State, Vec<String>) {
    if refs.is_empty() {
        return (State::Pass, vec![]);
    }
    let decided: BTreeMap<&str, &str> = context
        .map(|c| {
            c.evidence
                .iter()
                .map(|e| (e.reference.as_str(), e.decision.as_str()))
                .collect()
        })
        .unwrap_or_default();
    let mut state = State::Pass;
    let mut reasons = Vec::new();
    for r in refs {
        let (s, word) = match decided.get(r) {
            Some(&"admit") => continue,
            Some(&"refuse") => (State::Fail, "refuse"),
            Some(other) => (State::Unknown, *other),
            None => (State::NotRecorded, "not-recorded"),
        };
        reasons.push(format!("{r}: {word}"));
        state = combine(state, s);
    }
    (state, reasons)
}

/// Section 3.6: each expected artifact exactly once, its bytes retained
/// under its digest, within the size limit, and its internal anchors present.
fn artifact_state(
    task: &DocumentationTask,
    subject: &CoreTuple,
    retained: &dyn Fn(&str) -> Option<Vec<u8>>,
    max: u64,
) -> (State, Vec<String>) {
    let mut state = State::Pass;
    let mut reasons = Vec::new();
    let mut worse = |s: State, why: String, state: &mut State| {
        reasons.push(why);
        *state = combine(*state, s);
    };
    for expected in &task.artifacts {
        let found: Vec<&ArtifactIdentity> = subject
            .artifacts
            .iter()
            .filter(|a| a.logical_path == expected.path)
            .collect();
        let a = match found.as_slice() {
            [a] => a,
            [] => {
                worse(
                    State::Fail,
                    format!("{} missing", expected.path),
                    &mut state,
                );
                continue;
            }
            _ => {
                worse(
                    State::Fail,
                    format!("{} repeated", expected.path),
                    &mut state,
                );
                continue;
            }
        };
        let Some(bytes) = retained(&a.digest) else {
            worse(
                State::NotRecorded,
                format!("{} bytes not retained", a.logical_path),
                &mut state,
            );
            continue;
        };
        if custody_digest(&bytes) != a.digest {
            worse(
                State::Fail,
                format!("{} digest", a.logical_path),
                &mut state,
            );
            continue;
        }
        if bytes.len() as u64 > max {
            worse(
                State::Fail,
                format!("{} exceeds {max} bytes", a.logical_path),
                &mut state,
            );
            continue;
        }
        let Ok(text) = std::str::from_utf8(&bytes) else {
            worse(
                State::Fail,
                format!("{} is not text", a.logical_path),
                &mut state,
            );
            continue;
        };
        for anchor in missing_anchors(text) {
            worse(
                State::Fail,
                format!("{} links to a missing anchor #{anchor}", a.logical_path),
                &mut state,
            );
        }
    }
    for extra in subject
        .artifacts
        .iter()
        .filter(|a| !task.artifacts.iter().any(|e| e.path == a.logical_path))
    {
        worse(
            State::Fail,
            format!("{} is not an expected artifact", extra.logical_path),
            &mut state,
        );
    }
    (state, reasons)
}

/// Markdown links of the form `](#anchor)` whose anchor no heading in the
/// same text produces. External links are not checked here: this spec
/// grants no network access, and the task names no link policy.
fn missing_anchors(text: &str) -> Vec<String> {
    // The slug is of the rendered text, so an underscore that delimits
    // emphasis at a word's edge is dropped; one inside a word is kept.
    let slug = |h: &str| -> String {
        h.split_whitespace()
            .map(|w| w.trim_matches('_'))
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
            .chars()
            .filter_map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    Some(c)
                } else if c == ' ' {
                    Some('-')
                } else {
                    None
                }
            })
            .collect()
    };
    let headings: BTreeSet<String> = text
        .lines()
        .filter_map(|l| l.trim_start().strip_prefix('#'))
        .map(|h| slug(h.trim_start_matches('#')))
        .collect();
    let mut missing = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("](#") {
        rest = &rest[i + 3..];
        let Some(end) = rest.find(')') else { break };
        let anchor = &rest[..end];
        // Heading slugs are lower case, and a link's fragment matches one
        // regardless of case; the anchor is reported as written.
        if !headings.contains(&anchor.to_lowercase()) && !missing.iter().any(|m| m == anchor) {
            missing.push(anchor.to_string());
        }
        rest = &rest[end..];
    }
    missing
}

/// Judge one attempt's documentation (sections 3.1 to 3.9).
pub fn judge(i: &Inputs<'_>) -> Judgment {
    let mut reasons = Vec::new();
    let mut dims = Vec::new();
    let task = i.task;

    let (policy, policy_digest) = match &i.policy {
        Ok((p, d)) => (Some(*p), d.to_string()),
        Err(why) => {
            reasons.push(format!("policy-unreadable: {why}"));
            (None, String::new())
        }
    };

    let candidate = match &i.candidate {
        Ok(c) => Some(*c),
        Err(why) => {
            reasons.push(format!("candidate-unidentified: {why}"));
            None
        }
    };

    // Section 3.1: the core tuple, from the record alone.
    let base_subject = match (&i.result, i.result_custody, candidate) {
        (Ok(result), Some(custody), Some(candidate)) if policy.is_some() => Some(CoreTuple {
            repository: i.repository.to_string(),
            trusted_base: i.trusted_base.to_string(),
            run_id: task.run_id.clone(),
            attempt: task.attempt,
            candidate: candidate.to_string(),
            context_manifest_digest: task.context_manifest_digest.clone(),
            task_digest: task.task_digest.clone(),
            result_custody_digest: custody.to_string(),
            result_schema_digest: task.result_schema.digest.clone(),
            artifacts: result
                .artifacts
                .iter()
                .map(|a| ArtifactIdentity {
                    logical_path: a.logical_path.clone(),
                    digest: a.digest.clone(),
                })
                .collect(),
            evidence: result
                .claims
                .iter()
                .flat_map(|c| c.evidence_refs.iter().cloned())
                .collect(),
            policy_digest: policy_digest.clone(),
        }),
        _ => None,
    };
    let (subject, applied, changed_claims) = match base_subject {
        Some(s) => {
            let (s, a, c) = effective(s, i.corrections, &mut reasons);
            (Some(s), a, c)
        }
        None => (None, vec![], vec![]),
    };

    // 1. taskBinding.
    dims.push(match (&i.result, &subject) {
        (Ok(_), Some(_)) => dim(Dimension::TaskBinding, Need::Required, State::Pass, vec![]),
        (Err(why), _) => dim(
            Dimension::TaskBinding,
            Need::Required,
            State::NotRecorded,
            vec![why.clone()],
        ),
        (Ok(_), None) => dim(
            Dimension::TaskBinding,
            Need::Required,
            State::NotRecorded,
            vec!["the subject could not be identified".into()],
        ),
    });

    // 2. intendedDelivery: the task and the manifest it names are recorded,
    // and the manifest is eligible to execute.
    dims.push(match i.context {
        Some(c) if c.manifest_digest == task.context_manifest_digest => dim(
            Dimension::IntendedDelivery,
            Need::Required,
            State::Pass,
            vec![],
        ),
        Some(c) => dim(
            Dimension::IntendedDelivery,
            Need::Required,
            State::Fail,
            vec![format!(
                "the task names manifest {}, the context judged {}",
                task.context_manifest_digest, c.manifest_digest
            )],
        ),
        None => dim(
            Dimension::IntendedDelivery,
            Need::Required,
            State::NotRecorded,
            vec!["no context manifest was judged".into()],
        ),
    });

    // 3. adapterObservedDelivery: both testimonies, neither proof.
    let mut observed = Vec::new();
    let mut observed_state = State::Pass;
    match i.delivery {
        None => {
            observed.push("documentation delivery not-recorded".to_string());
            observed_state = State::NotRecorded;
        }
        Some(d) if !d.eligible => {
            observed.extend(d.findings.iter().cloned());
            observed_state = State::Fail;
        }
        Some(_) => {}
    }
    match i.context {
        None => {
            observed.push("context delivery not-recorded".into());
            observed_state = combine(observed_state, State::NotRecorded);
        }
        Some(c) if !c.delivery.findings.is_empty() => {
            observed.extend(c.delivery.findings.iter().map(|f| format!("context {f}")));
            observed_state = State::Fail;
        }
        Some(_) => {}
    }
    dims.push(dim(
        Dimension::AdapterObservedDelivery,
        Need::Required,
        observed_state,
        observed,
    ));

    // 4. observedTransport: no adopted evidence contract supplies it.
    dims.push(dim(
        Dimension::ObservedTransport,
        policy
            .map(|p| p.observed_transport)
            .unwrap_or(Need::Required),
        State::NotRecorded,
        vec!["no adopted transport evidence contract".into()],
    ));

    // 5. schemaValidation.
    dims.push(match &i.result {
        Ok(_) => dim(
            Dimension::SchemaValidation,
            Need::Required,
            State::Pass,
            vec![],
        ),
        Err(why) if why.starts_with("result-schema-invalid") => dim(
            Dimension::SchemaValidation,
            Need::Required,
            State::Fail,
            vec![why.clone()],
        ),
        Err(why) => dim(
            Dimension::SchemaValidation,
            Need::Required,
            State::NotRecorded,
            vec![why.clone()],
        ),
    });

    let claims: Vec<&statecraft_run::documentation::Claim> = match &i.result {
        Ok(r) => r.claims.iter().collect(),
        Err(_) => vec![],
    };

    // 6. citationResolution.
    let claimed = claims.iter().map(|c| c.citations.len()).sum();
    let (s, r) = citation_state(i.context, claimed);
    dims.push(dim(Dimension::CitationResolution, Need::Required, s, r));

    // 7. evidenceAdmission.
    let refs: BTreeSet<&str> = claims
        .iter()
        .flat_map(|c| c.evidence_refs.iter().map(String::as_str))
        .collect();
    let (s, r) = evidence_state(i.context, &refs);
    dims.push(dim(Dimension::EvidenceAdmission, Need::Required, s, r));

    // Reviews that bind the effective subject, and whose reviewer is not the
    // provider or adapter that produced the result.
    let mut authority_reasons = Vec::new();
    let mut authority_state = State::Pass;
    let mut bound: Vec<&Review> = Vec::new();
    for r in i.reviews {
        match r {
            Read::Unreadable(why) => {
                authority_reasons.push(format!("review-unreadable: {why}"));
                authority_state = combine(authority_state, State::Unreadable);
            }
            Read::Valid(r) if Some(&r.subject) != subject.as_ref() => {
                reasons.push(format!(
                    "review-not-binding: {} names another subject",
                    r.review_digest
                ));
            }
            Read::Valid(r) => {
                let producer = match &i.result {
                    Ok(res) => {
                        r.reviewer.identity == res.provider.name
                            || r.reviewer.identity == res.adapter.name
                    }
                    Err(_) => false,
                };
                if producer {
                    authority_reasons.push(format!(
                        "{} produced the result and cannot review it",
                        r.reviewer.identity
                    ));
                    authority_state = State::Fail;
                    continue;
                }
                match policy.map(|p| eligible_grant(p, r, i.now)) {
                    Some(Ok(_)) => bound.push(r),
                    Some(Err(why)) => {
                        authority_reasons.push(why);
                        authority_state = combine(authority_state, State::Unknown);
                    }
                    None => {
                        authority_reasons.push("no trusted-base policy".into());
                        authority_state = combine(authority_state, State::NotRecorded);
                    }
                }
            }
        }
    }

    // 8. semanticReview: every claim supported by every bound review.
    let semantic_need = match task.review_policy.semantic_review {
        SemanticReview::Required => Need::Required,
        SemanticReview::NotRequired => Need::Optional,
    };
    let mut semantic = Vec::new();
    let semantic_state = if bound.is_empty() {
        semantic.push("no eligible review binds the subject".into());
        State::NotRecorded
    } else {
        let mut state = State::Pass;
        for r in &bound {
            for c in &claims {
                let d = r
                    .claims
                    .iter()
                    .find(|x| x.claim_id == c.claim_id)
                    .map(|x| x.disposition)
                    .unwrap_or(ClaimDisposition::NotReviewed);
                let s = match d {
                    ClaimDisposition::Supported => continue,
                    ClaimDisposition::NotReviewed => State::NotRecorded,
                    _ => State::Fail,
                };
                semantic.push(format!(
                    "{} {}: {}",
                    r.review_digest,
                    c.claim_id,
                    serde_json::to_value(d)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_string))
                        .unwrap_or_default()
                ));
                state = combine(state, s);
            }
            for c in &changed_claims {
                if !r.claims.iter().any(|x| &x.claim_id == c) {
                    semantic.push(format!(
                        "{} did not review corrected claim {c}",
                        r.review_digest
                    ));
                    state = combine(state, State::NotRecorded);
                }
            }
        }
        state
    };
    dims.push(dim(
        Dimension::SemanticReview,
        semantic_need,
        semantic_state,
        semantic,
    ));

    // 9. artifactValidation.
    dims.push(match (&subject, policy) {
        (Some(s), Some(p)) => {
            let (st, r) = artifact_state(task, s, i.retained, p.max_artifact_bytes);
            let mut st = st;
            let mut r = r;
            for rv in &bound {
                for a in &rv.artifacts {
                    match a.disposition {
                        ArtifactDisposition::Acceptable => {}
                        ArtifactDisposition::NotReviewed => {
                            r.push(format!("{} not reviewed", a.logical_path));
                            st = combine(st, State::NotRecorded);
                        }
                        _ => {
                            r.push(format!(
                                "{} {}",
                                a.logical_path,
                                serde_json::to_value(a.disposition)
                                    .ok()
                                    .and_then(|v| v.as_str().map(str::to_string))
                                    .unwrap_or_default()
                            ));
                            st = State::Fail;
                        }
                    }
                }
            }
            dim(Dimension::ArtifactValidation, Need::Required, st, r)
        }
        _ => dim(
            Dimension::ArtifactValidation,
            Need::Required,
            State::NotRecorded,
            vec!["no subject or no policy".into()],
        ),
    });

    // 10. contextFreshness.
    let (s, r) = freshness_state(i.context, i.manifest_freshness);
    dims.push(dim(Dimension::ContextFreshness, Need::Required, s, r));

    // 11. authority: every review eligible, and the quorum met.
    let accepting = bound
        .iter()
        .filter(|r| r.recommendation == Recommendation::Accept && r.corrections.is_empty())
        .count() as u32;
    if let Some(p) = policy
        && accepting < p.quorum
        && authority_state == State::Pass
    {
        authority_reasons.push(format!(
            "{accepting} of {} required accepting review(s)",
            p.quorum
        ));
        authority_state = State::NotRecorded;
    }
    if policy.is_none() && authority_state == State::Pass {
        authority_state = State::NotRecorded;
        authority_reasons.push("no trusted-base policy".into());
    }
    dims.push(dim(
        Dimension::Authority,
        Need::Required,
        authority_state,
        authority_reasons,
    ));

    // 12. publicationBoundary: no expected artifact path was applied to the
    // candidate.
    dims.push(match i.changed_paths {
        None => dim(
            Dimension::PublicationBoundary,
            Need::Required,
            State::NotRecorded,
            vec!["the candidate's changed paths are not recorded".into()],
        ),
        Some(paths) => {
            let applied: Vec<String> = task
                .artifacts
                .iter()
                .filter(|a| paths.iter().any(|p| p == &a.path))
                .map(|a| format!("{} was written into the candidate", a.path))
                .collect();
            if applied.is_empty() {
                dim(
                    Dimension::PublicationBoundary,
                    Need::Required,
                    State::Pass,
                    vec![],
                )
            } else {
                dim(
                    Dimension::PublicationBoundary,
                    Need::Required,
                    State::Fail,
                    applied,
                )
            }
        }
    });

    // Section 3.9.
    let required: Vec<&DimensionResult> =
        dims.iter().filter(|d| d.need == Need::Required).collect();
    let outcome = if required.iter().any(|d| d.state == State::Fail) {
        Outcome::Rejected
    } else if required.iter().all(|d| d.state == State::Pass) && subject.is_some() {
        Outcome::Accepted
    } else {
        Outcome::NoAcceptance
    };
    if outcome != Outcome::Accepted {
        for d in &required {
            if d.state != State::Pass {
                reasons.push(format!(
                    "{}-{}",
                    serde_json::to_value(d.dimension)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_string))
                        .unwrap_or_default(),
                    serde_json::to_value(d.state)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_string))
                        .unwrap_or_default()
                ));
            }
        }
    }
    Judgment {
        subject,
        bound_reviews: bound.iter().map(|r| r.review_digest.clone()).collect(),
        corrections: applied,
        dimensions: dims,
        outcome,
        reasons,
        notice: BOUNDARY_NOTICE,
    }
}
