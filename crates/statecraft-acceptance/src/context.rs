//! Context-packet acceptance: spec 019 sections 3.7 and 3.8, through its
//! additive `extends` edge on this crate (spec 005 owns it; spec 019 owns no
//! code).
//!
//! [`judge`] evaluates delivery and the five checks separately and returns
//! every result. No dimension is inferred from another, and none is computed
//! here: every selector, closure, digest, snapshot and member answer comes
//! from the adopted producer through [`Producer`], which `statecraft-home`
//! implements, the one crate that links it. What this module owns is the
//! comparison of those answers with what the manifest recorded, and the
//! section 3.8 rule that turns the results and the recorded policy into
//! eligibility.
//!
//! A failed context check does not rewrite the attempt's outcome, and a
//! passing one is not a publication permission or a substitute for spec 005's
//! suite: it is one more input to that judgment.

use serde::Serialize;

use statecraft_run::context::{
    Completeness, ContextPolicy, Delivery, Dimension, Intended, Manifest, Need, PageFacts,
    Requirement, Snapshot, Visibility, custody_digest,
};

/// The producer's answer to validating one page's exact bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validated {
    /// The bytes decode under the adopted schema and the producer's own
    /// digest operation reproduces the page digest they carry.
    Valid(Box<PageFacts>),
    /// The producer's validation rejected the page.
    Invalid(String),
    /// The adopted producer offers no validation for this page's schema.
    Unsupported(String),
    /// The bytes cannot be decoded far enough to ask.
    Unreadable(String),
}

/// The producer's current answer for a recorded closure request at the
/// recorded snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosureAnswer {
    /// The producer resolved it. `differences` are the producer-reported
    /// members present in one answer and not the other.
    Answered {
        /// The producer's current closure digest.
        digest: String,
        /// Every member difference, as the producer names them.
        differences: Vec<String>,
    },
    /// The recorded snapshot or a closure reference no longer resolves.
    Missing(String),
    /// A member the closure needs has been withdrawn.
    Withdrawn(String),
    /// The producer reports its own index stale for the snapshot.
    Stale(String),
    /// The adopted producer cannot answer this request.
    Unsupported(String),
    /// The answer could not be decoded.
    Unreadable(String),
}

/// The producer's binding of a caller-supplied current snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurrentAnswer {
    /// A stable, clean snapshot the producer identified.
    Resolved(Snapshot),
    /// Absent, dirty, moving, unsupported or unidentifiable, with why.
    Unverified(String),
}

/// The producer's answer for one member identity in one retained page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberAnswer {
    /// The page carries the member.
    Found {
        /// The member's content digest, as the producer recorded it.
        content_digest: String,
        /// The member's first line.
        start_line: usize,
        /// The member's last line.
        end_line: usize,
    },
    /// The page records the member as omitted.
    Omitted,
    /// The page neither carries nor omits it.
    Absent,
    /// The producer cannot validate this member.
    Unsupported(String),
    /// The page could not be decoded far enough to ask.
    Unreadable(String),
}

/// The adopted producer, as acceptance asks it (section 3.3).
pub trait Producer {
    /// Section 3.7.1: validate one page's exact retained bytes.
    fn validate_page(&self, bytes: &[u8]) -> Validated;
    /// Section 3.7.2: the current closure for the request recorded in
    /// `page`, at the snapshot it recorded, for repository `id`.
    fn closure(&self, id: &str, page: &[u8]) -> ClosureAnswer;
    /// Section 3.7.3: the caller-supplied current snapshot for repository
    /// `id`, bound by the same contract that assembled the packet.
    fn current(&self, id: &str) -> CurrentAnswer;
    /// Section 3.7.4: what `page` holds under `identity`.
    fn member(&self, page: &[u8], identity: &str) -> MemberAnswer;
}

/// Section 3.7.1's states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum IntegrityState {
    /// Every field the page must carry is present and the producer validates
    /// it.
    Pass,
    /// Custody, validation or a recorded identity disagrees.
    Fail,
    /// The producer offers no validation.
    Unsupported,
    /// The bytes could not be decoded far enough to ask.
    Unreadable,
    /// No retained bytes.
    NotRecorded,
}

/// One page's integrity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageIntegrity {
    /// The repository.
    pub repository: String,
    /// The page.
    pub page_digest: String,
    /// Statecraft's custody comparison, reported separately.
    pub custody: &'static str,
    /// The state.
    pub state: IntegrityState,
    /// Why, when it is not `pass`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Section 3.7.2's states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClosureState {
    /// The producer's current answer equals the recorded digest.
    Current,
    /// It does not.
    Changed,
    /// The snapshot or a reference no longer resolves.
    Missing,
    /// A needed member is withdrawn.
    Withdrawn,
    /// The producer's index is stale.
    Stale,
    /// The producer cannot answer.
    Unsupported,
    /// The answer could not be decoded.
    Unreadable,
    /// No retained page to ask from.
    NotRecorded,
}

/// One repository's closure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryClosure {
    /// The repository.
    pub repository: String,
    /// The recorded digest.
    pub recorded: String,
    /// The state.
    pub state: ClosureState,
    /// Every producer-reported member difference.
    pub differences: Vec<String>,
    /// Why, when it is not `current`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Section 3.7.3's states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FreshnessState {
    /// Same repository and tree, clean, and complete.
    Fresh,
    /// Another tree or repository.
    Stale,
    /// The same tree, but the packet is not complete.
    Incomplete,
    /// No usable current snapshot.
    Unverified,
}

/// One repository's freshness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryFreshness {
    /// The repository.
    pub repository: String,
    /// The state.
    pub state: FreshnessState,
    /// The producer-reported current snapshot, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<Snapshot>,
    /// Why, when it is not `fresh`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// One claimed citation (section 3.7.4).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Citation {
    /// The manifest's repository id.
    pub repository: String,
    /// The packet page.
    pub page_digest: String,
    /// The canonical producer member identity.
    pub member: String,
    /// The member's content digest.
    pub content_digest: String,
    /// An optional bounded span, first and last line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<(usize, usize)>,
}

/// Section 3.7.4's states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CitationState {
    /// The exact packet member was cited.
    Resolved,
    /// The cited page records the member as omitted.
    Missing,
    /// The member is there under another content digest.
    DigestMismatch,
    /// Not a member of the cited page, or the page or span is not in the
    /// packet.
    OutsidePacket,
    /// The producer cannot validate it.
    Unsupported,
    /// The page could not be decoded far enough to ask.
    Unreadable,
    /// No retained bytes for the cited page.
    NotRecorded,
}

/// One citation's resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CitationResult {
    /// The citation as claimed.
    pub citation: Citation,
    /// The state.
    pub state: CitationState,
}

/// One evidence reference's admission, as spec 005 decided it (section
/// 3.7.5). Supplied by the caller; never derived from another check here.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceResult {
    /// The reference.
    pub reference: String,
    /// `admit`, `refuse`, or one of spec 005's absence words.
    pub decision: String,
}

/// Everything acceptance supplies beside the producer.
pub struct Inputs<'a> {
    /// The manifest the attempt's intent recorded.
    pub manifest: &'a Manifest,
    /// The policy the attempt's intent recorded.
    pub policy: &'a ContextPolicy,
    /// The folded delivery for the attempt.
    pub delivery: &'a Delivery,
    /// The retained bytes, by custody digest.
    pub retained: &'a dyn Fn(&str) -> Option<Vec<u8>>,
    /// The claimed citations, in order.
    pub citations: &'a [Citation],
    /// The evidence admissions, in order.
    pub evidence: &'a [EvidenceResult],
}

/// Why acceptance is ineligible: the dimension, what it is about, and the
/// state that made it so.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reason {
    /// The dimension.
    pub dimension: Dimension,
    /// The repository, page or citation the state belongs to.
    pub subject: String,
    /// The state word.
    pub state: String,
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let d = serde_json::to_value(self.dimension).expect("a dimension serializes");
        write!(
            f,
            "context-{}-{}: {}",
            d.as_str().unwrap_or("unknown"),
            self.state,
            self.subject
        )
    }
}

/// Every result and the eligibility they add up to under the policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextResult {
    /// The manifest judged.
    pub manifest_digest: String,
    /// The exact policy used.
    pub policy: ContextPolicy,
    /// Delivery, intended beside observed.
    pub delivery: Delivery,
    /// Every page's integrity.
    pub integrity: Vec<PageIntegrity>,
    /// Every repository's closure.
    pub closure: Vec<RepositoryClosure>,
    /// `current` only when every required repository is `current`;
    /// otherwise the first required repository's other state.
    pub closure_aggregate: ClosureState,
    /// Every repository's freshness.
    pub freshness: Vec<RepositoryFreshness>,
    /// `fresh` only when every required repository is `fresh`; otherwise the
    /// first required repository's other state. It still does
    /// not say the snapshots were atomic, compatible, correct or accepted.
    pub freshness_aggregate: FreshnessState,
    /// Every citation's resolution.
    pub citations: Vec<CitationResult>,
    /// Every evidence admission, as supplied.
    pub evidence: Vec<EvidenceResult>,
    /// Whether these facts permit acceptance.
    pub eligible: bool,
    /// Every reason they do not, in dimension order.
    pub reasons: Vec<Reason>,
}

fn word<T: Serialize>(state: T) -> String {
    serde_json::to_value(state)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn integrity(inputs: &Inputs<'_>, producer: &dyn Producer) -> Vec<PageIntegrity> {
    let mut out = Vec::new();
    for repo in &inputs.manifest.repositories {
        let last = repo.pages.len();
        for page in &repo.pages {
            let result = |custody: &'static str, state, reason: Option<String>| PageIntegrity {
                repository: repo.id.clone(),
                page_digest: page.page_digest.clone(),
                custody,
                state,
                reason,
            };
            let Some(bytes) = (inputs.retained)(&page.custody_digest) else {
                out.push(result("not-recorded", IntegrityState::NotRecorded, None));
                continue;
            };
            // Custody first: bytes that are not the retained bytes are never
            // given to the producer for semantic use.
            if custody_digest(&bytes) != page.custody_digest || bytes.len() != page.byte_length {
                out.push(result(
                    "mismatch",
                    IntegrityState::Fail,
                    Some("custody digest".into()),
                ));
                continue;
            }
            let state = match producer.validate_page(&bytes) {
                Validated::Unsupported(r) => (IntegrityState::Unsupported, Some(r)),
                Validated::Unreadable(r) => (IntegrityState::Unreadable, Some(r)),
                Validated::Invalid(r) => (IntegrityState::Fail, Some(r)),
                Validated::Valid(f) => {
                    let mismatch = [
                        (f.packet_digest != page.page_digest, "page digest"),
                        (f.snapshot != repo.snapshot, "snapshot"),
                        (f.request_digest != repo.request_digest, "request digest"),
                        (f.closure_digest != repo.closure_digest, "closure digest"),
                        (f.schema_version != repo.packet_schema_version, "schema"),
                        (f.producer != page.producer_build, "producer"),
                        (f.continues == (page.ordinal == last), "continuation"),
                        (
                            f.omissions.len() != page.omissions.len()
                                || f.omissions
                                    .iter()
                                    .zip(&page.omissions)
                                    .any(|(a, b)| *a != b.fact),
                            "omissions",
                        ),
                        (
                            f.warnings.len() != page.warnings.len()
                                || f.warnings
                                    .iter()
                                    .zip(&page.warnings)
                                    .any(|(a, b)| *a != b.fact),
                            "warnings",
                        ),
                    ]
                    .into_iter()
                    .find_map(|(bad, what)| bad.then_some(what));
                    match mismatch {
                        Some(what) => (IntegrityState::Fail, Some(format!("recorded {what}"))),
                        None => (IntegrityState::Pass, None),
                    }
                }
            };
            out.push(result("match", state.0, state.1));
        }
    }
    out
}

fn closure(inputs: &Inputs<'_>, producer: &dyn Producer) -> Vec<RepositoryClosure> {
    inputs
        .manifest
        .repositories
        .iter()
        .map(|repo| {
            let result = |state, differences, reason| RepositoryClosure {
                repository: repo.id.clone(),
                recorded: repo.closure_digest.clone(),
                state,
                differences,
                reason,
            };
            let first = repo.pages.first().and_then(|p| {
                (inputs.retained)(&p.custody_digest)
                    .filter(|b| custody_digest(b) == p.custody_digest)
            });
            let Some(bytes) = first else {
                return result(ClosureState::NotRecorded, vec![], None);
            };
            match producer.closure(&repo.id, &bytes) {
                ClosureAnswer::Answered {
                    digest,
                    differences,
                } => {
                    if digest == repo.closure_digest {
                        result(ClosureState::Current, differences, None)
                    } else {
                        result(ClosureState::Changed, differences, None)
                    }
                }
                ClosureAnswer::Missing(r) => result(ClosureState::Missing, vec![], Some(r)),
                ClosureAnswer::Withdrawn(r) => result(ClosureState::Withdrawn, vec![], Some(r)),
                ClosureAnswer::Stale(r) => result(ClosureState::Stale, vec![], Some(r)),
                ClosureAnswer::Unsupported(r) => result(ClosureState::Unsupported, vec![], Some(r)),
                ClosureAnswer::Unreadable(r) => result(ClosureState::Unreadable, vec![], Some(r)),
            }
        })
        .collect()
}

fn freshness(inputs: &Inputs<'_>, producer: &dyn Producer) -> Vec<RepositoryFreshness> {
    inputs
        .manifest
        .repositories
        .iter()
        .map(|repo| match producer.current(&repo.id) {
            CurrentAnswer::Unverified(reason) => RepositoryFreshness {
                repository: repo.id.clone(),
                state: FreshnessState::Unverified,
                current: None,
                reason: Some(reason),
            },
            CurrentAnswer::Resolved(current) => {
                let same = current.repository == repo.snapshot.repository
                    && current.tree == repo.snapshot.tree;
                let (state, reason) = if !same {
                    (
                        FreshnessState::Stale,
                        Some("another repository or tree".into()),
                    )
                } else if repo.completeness != Completeness::Complete {
                    (FreshnessState::Incomplete, Some(word(repo.completeness)))
                } else {
                    (FreshnessState::Fresh, None)
                };
                RepositoryFreshness {
                    repository: repo.id.clone(),
                    state,
                    current: Some(current),
                    reason,
                }
            }
        })
        .collect()
}

fn citation(inputs: &Inputs<'_>, producer: &dyn Producer, c: &Citation) -> CitationState {
    let page = inputs
        .manifest
        .repositories
        .iter()
        .filter(|r| r.id == c.repository)
        .flat_map(|r| r.pages.iter())
        .find(|p| p.page_digest == c.page_digest);
    let Some(page) = page else {
        return CitationState::OutsidePacket;
    };
    let Some(bytes) = (inputs.retained)(&page.custody_digest) else {
        return CitationState::NotRecorded;
    };
    if custody_digest(&bytes) != page.custody_digest {
        return CitationState::Unreadable;
    }
    match producer.member(&bytes, &c.member) {
        MemberAnswer::Found {
            content_digest,
            start_line,
            end_line,
        } => {
            if content_digest != c.content_digest {
                CitationState::DigestMismatch
            } else if c
                .span
                .is_some_and(|(a, b)| a > b || a < start_line || b > end_line)
            {
                CitationState::OutsidePacket
            } else {
                CitationState::Resolved
            }
        }
        MemberAnswer::Omitted => CitationState::Missing,
        MemberAnswer::Absent => CitationState::OutsidePacket,
        MemberAnswer::Unsupported(_) => CitationState::Unsupported,
        MemberAnswer::Unreadable(_) => CitationState::Unreadable,
    }
}

/// Section 3.7: evaluate delivery and the five checks, every one, and
/// section 3.8: decide eligibility under the recorded policy alone.
pub fn judge(inputs: &Inputs<'_>, producer: &dyn Producer) -> ContextResult {
    let m = inputs.manifest;
    let required = |id: &str| {
        m.repositories
            .iter()
            .any(|r| r.id == id && r.requirement == Requirement::Required)
    };
    let integrity = integrity(inputs, producer);
    let closure = closure(inputs, producer);
    let freshness = freshness(inputs, producer);
    let citations: Vec<CitationResult> = inputs
        .citations
        .iter()
        .map(|c| CitationResult {
            citation: c.clone(),
            state: citation(inputs, producer, c),
        })
        .collect();

    // An aggregate is the first required repository's non-passing state, in
    // manifest order, or the passing state when there is none.
    let closure_aggregate = closure
        .iter()
        .filter(|c| required(&c.repository))
        .map(|c| c.state)
        .find(|s| *s != ClosureState::Current)
        .unwrap_or(ClosureState::Current);
    let freshness_aggregate = freshness
        .iter()
        .filter(|f| required(&f.repository))
        .map(|f| f.state)
        .find(|s| *s != FreshnessState::Fresh)
        .unwrap_or(FreshnessState::Fresh);

    // Section 3.8. Each fact is (dimension, subject, state, passes, is a
    // required fact). A fact of an optional repository or an optional
    // intended page is optional whatever the dimension needs.
    let mut facts: Vec<(Dimension, String, String, bool, bool)> = Vec::new();
    for (intended, observed) in inputs
        .delivery
        .intended
        .iter()
        .zip(&inputs.delivery.observed)
    {
        let pass = observed.state == Visibility::ReportedVisible;
        let req = intended.delivery == Intended::Required && required(&intended.repository);
        if intended.delivery != Intended::Withheld {
            facts.push((
                Dimension::Delivery,
                format!("{}/{}", intended.repository, intended.page_digest),
                word(observed.state),
                pass,
                req,
            ));
        }
    }
    for p in &integrity {
        facts.push((
            Dimension::Integrity,
            format!("{}/{}", p.repository, p.page_digest),
            word(p.state),
            p.state == IntegrityState::Pass,
            required(&p.repository),
        ));
    }
    for c in &closure {
        facts.push((
            Dimension::Closure,
            c.repository.clone(),
            word(c.state),
            c.state == ClosureState::Current,
            required(&c.repository),
        ));
    }
    for f in &freshness {
        facts.push((
            Dimension::Freshness,
            f.repository.clone(),
            word(f.state),
            f.state == FreshnessState::Fresh,
            required(&f.repository),
        ));
    }
    if citations.is_empty() {
        facts.push((
            Dimension::Citations,
            "citations".into(),
            "not-recorded".into(),
            false,
            true,
        ));
    }
    for (i, c) in citations.iter().enumerate() {
        facts.push((
            Dimension::Citations,
            format!("citation {}: {}", i + 1, c.citation.member),
            word(c.state),
            c.state == CitationState::Resolved,
            required(&c.citation.repository),
        ));
    }
    if inputs.evidence.is_empty() {
        facts.push((
            Dimension::Evidence,
            "evidence".into(),
            "not-recorded".into(),
            false,
            true,
        ));
    }
    for e in inputs.evidence {
        facts.push((
            Dimension::Evidence,
            e.reference.clone(),
            e.decision.clone(),
            e.decision == "admit",
            true,
        ));
    }

    let policy = inputs.policy;
    let reasons: Vec<Reason> = facts
        .into_iter()
        .filter(|(dimension, _, state, pass, req)| {
            !pass
                && match (policy.need(*dimension), req) {
                    (Need::Required, true) => true,
                    _ => !policy.permits(*dimension, state),
                }
        })
        .map(|(dimension, subject, state, _, _)| Reason {
            dimension,
            subject,
            state,
        })
        .collect();

    ContextResult {
        manifest_digest: m.manifest_digest.clone(),
        policy: policy.clone(),
        delivery: inputs.delivery.clone(),
        integrity,
        closure,
        closure_aggregate,
        freshness,
        freshness_aggregate,
        citations,
        evidence: inputs.evidence.to_vec(),
        eligible: reasons.is_empty(),
        reasons,
    }
}
