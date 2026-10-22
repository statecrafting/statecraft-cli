//! Spec 021's judgment: twelve dimensions reported separately, reviewer
//! authority from the trusted-base policy, corrections that unbind earlier
//! reviews, and `accepted`, `rejected` or `no-acceptance` (sections 3.3 to
//! 3.9 and 3.11's negative cases). Every input is a fixture; nothing is
//! fetched, applied or published.

use std::collections::BTreeMap;

use serde_json::json;

use statecraft_acceptance::context::{
    Citation, CitationResult, CitationState, ClosureState, ContextResult, EvidenceResult,
    FreshnessState,
};
use statecraft_acceptance::documentation_acceptance::{
    self as da, Dimension, Grant, Inputs, ManifestFreshness, Need, Outcome, Policy, State,
};
use statecraft_run::context::{ContextPolicy, Delivery, Need as ContextNeed, custody_digest};
use statecraft_run::documentation::delivery::Delivery as DocDelivery;
use statecraft_run::documentation::review::{
    ArtifactDisposition, ArtifactIdentity, ArtifactReview, Authority, ClaimDisposition,
    ClaimReview, CoreTuple, Correction, Read, Recommendation, Review, Reviewer,
};
use statecraft_run::documentation::{
    self as doc, Artifact, Candidate, ClockSource, CostCeiling, Deadline, Disposition,
    DocumentationResult, DocumentationTask, Format, NoSpend, NotRequired, Operations, OutputFormat,
    Publication, ResultSchemaRef, ReviewPolicy, SemanticReview, Skill,
};

const ARTIFACT: &[u8] =
    b"# Parser\n\nSee [recovery](#recovery).\n\n## Recovery\n\nAt the next statement.\n";
const NOW: &str = "2026-10-22T00:00:00Z";

fn d(t: &str) -> String {
    custody_digest(t.as_bytes())
}

fn task() -> DocumentationTask {
    DocumentationTask {
        schema: doc::TASK_SCHEMA.into(),
        task_id: "task-021".into(),
        run_id: "run-021".into(),
        attempt: 1,
        candidate: Candidate {
            repository: "app".into(),
            revision: "c".repeat(40),
            tree: "tree".into(),
        },
        context_manifest_digest: d("manifest"),
        objective: "Describe recovery.".into(),
        audiences: vec!["operators".into()],
        output_format: OutputFormat {
            format: Format::Markdown,
            version: "commonmark-0.31.2".into(),
        },
        operations: Operations::v1(),
        required_capabilities: vec![],
        skill: Skill::NotRequired(NotRequired::NotRequired),
        result_schema: ResultSchemaRef {
            id: doc::RESULT_SCHEMA.into(),
            digest: doc::result_schema_digest(),
        },
        deadline: Deadline {
            not_after: "2026-12-31T00:00:00Z".into(),
            clock: ClockSource::SupervisorUtc,
        },
        cost_ceiling: CostCeiling::NoSpend(NoSpend::NoSpend),
        artifacts: vec![Artifact {
            path: "docs/parser.md".into(),
            disposition: Disposition::Draft,
            required_context: vec![],
        }],
        review_policy: ReviewPolicy {
            reviewers: vec!["owner".into()],
            semantic_review: SemanticReview::Required,
            acceptance_route: "accept".into(),
            candidate_divergence: None,
        },
        publication: Publication::Prohibited,
        digest_construction: String::new(),
        task_digest: String::new(),
    }
    .seal()
}

fn citation() -> Citation {
    Citation {
        repository: "app".into(),
        page_digest: d("page-1"),
        member: "src/parser.rs".into(),
        content_digest: d("member"),
        span: None,
    }
}

fn result(t: &DocumentationTask, grade: &str) -> DocumentationResult {
    let c = citation();
    let v = json!({
        "schema": doc::RESULT_SCHEMA,
        "taskId": t.task_id, "runId": t.run_id, "attempt": 1,
        "taskDigest": t.task_digest, "contextManifestDigest": t.context_manifest_digest,
        "adapter": {"name": "fixture-adapter", "version": "1"},
        "provider": {"name": "fixture-provider", "version": "1"},
        "artifacts": [{
            "logicalPath": "docs/parser.md", "mediaType": "text/markdown",
            "digest": custody_digest(ARTIFACT), "size": ARTIFACT.len(), "rawRef": "a0"
        }],
        "claims": [{
            "claimId": "c1", "text": "Recovery happens at the next statement.",
            "lifecycleGrade": grade,
            "citations": [{"repository": c.repository, "pageDigest": c.page_digest,
                           "member": c.member, "contentDigest": c.content_digest}],
            "evidenceRefs": ["test-run-1"], "unsupportedReason": "not-applicable",
            "contradictionRefs": [], "reviewDisposition": "not-reviewed"
        }],
        "unknowns": [], "contradictions": [], "unsupportedClaims": [],
        "providerOmissions": [], "statecraftCitationFailures": [],
        "executionEvidence": [{"reference": "test-run-1", "purpose": "the parser suite"}],
        "providerCompletion": "claimed-complete",
        "reviewDisposition": "not-reviewed", "acceptanceResult": "not-attempted"
    });
    DocumentationResult::decode(&serde_json::to_vec(&v).unwrap(), t).unwrap()
}

fn context(
    t: &DocumentationTask,
    citation_state: CitationState,
    freshness: FreshnessState,
) -> ContextResult {
    ContextResult {
        manifest_digest: t.context_manifest_digest.clone(),
        policy: ContextPolicy {
            schema: "statecraft/context-acceptance-policy/1".into(),
            delivery: ContextNeed::Required,
            integrity: ContextNeed::Required,
            closure: ContextNeed::Required,
            freshness: ContextNeed::Required,
            citations: ContextNeed::Required,
            evidence: ContextNeed::Required,
            permitted_absences: vec![],
        },
        delivery: Delivery {
            intended: vec![],
            observed: vec![],
            findings: vec![],
            transport: "not-recorded".into(),
        },
        integrity: vec![],
        closure: vec![],
        closure_aggregate: ClosureState::Current,
        freshness: vec![],
        freshness_aggregate: freshness,
        citations: vec![CitationResult {
            citation: citation(),
            state: citation_state,
        }],
        evidence: vec![EvidenceResult {
            reference: "test-run-1".into(),
            decision: "admit".into(),
        }],
        eligible: true,
        reasons: vec![],
    }
}

fn policy() -> (Policy, String) {
    let p = Policy {
        schema: da::POLICY_SCHEMA.into(),
        reviewers: vec![Grant {
            identity: "owner@example.invalid".into(),
            authority: Authority::RepositoryOwner,
            not_after: None,
        }],
        quorum: 1,
        observed_transport: Need::Optional,
        max_artifact_bytes: 65_536,
    };
    let bytes = serde_json::to_vec(&p).unwrap();
    Policy::decode(&bytes).unwrap()
}

fn subject(t: &DocumentationTask, policy_digest: &str, artifact: &[u8]) -> CoreTuple {
    CoreTuple {
        repository: "example/app".into(),
        trusted_base: "b".repeat(40),
        run_id: t.run_id.clone(),
        attempt: 1,
        candidate: "c".repeat(40),
        context_manifest_digest: t.context_manifest_digest.clone(),
        task_digest: t.task_digest.clone(),
        result_custody_digest: d("raw"),
        result_schema_digest: t.result_schema.digest.clone(),
        artifacts: vec![ArtifactIdentity {
            logical_path: "docs/parser.md".into(),
            digest: custody_digest(artifact),
        }],
        evidence: vec!["test-run-1".into()],
        policy_digest: policy_digest.into(),
    }
}

fn review(
    subject: CoreTuple,
    who: &str,
    disposition: ClaimDisposition,
    rec: Recommendation,
) -> Review {
    Review {
        schema: "statecraft/documentation-review/1".into(),
        subject,
        reviewer: Reviewer {
            identity: who.into(),
            authority: Authority::RepositoryOwner,
            authority_source: "policy reviewers[0]".into(),
        },
        reviewed_at: NOW.into(),
        claims: vec![ClaimReview {
            claim_id: "c1".into(),
            disposition,
            reasons: vec![],
        }],
        artifacts: vec![ArtifactReview {
            logical_path: "docs/parser.md".into(),
            disposition: ArtifactDisposition::Acceptable,
            reasons: vec![],
        }],
        corrections: vec![],
        recommendation: rec,
        digest_construction: String::new(),
        review_digest: String::new(),
    }
    .seal()
}

/// One configurable judgment over fixtures.
struct Case {
    task: DocumentationTask,
    result: DocumentationResult,
    context: Option<ContextResult>,
    manifest: ManifestFreshness,
    reviews: Vec<Read<Review>>,
    corrections: Vec<Read<Correction>>,
    store: BTreeMap<String, Vec<u8>>,
    changed: Vec<String>,
    policy: (Policy, String),
    delivery_findings: Vec<String>,
    candidate: Result<String, String>,
}

impl Case {
    fn passing() -> Self {
        let t = task();
        let r = result(&t, "implemented");
        let p = policy();
        let ctx = context(&t, CitationState::Resolved, FreshnessState::Fresh);
        let s = subject(&t, &p.1, ARTIFACT);
        let reviews = vec![Read::Valid(review(
            s,
            "owner@example.invalid",
            ClaimDisposition::Supported,
            Recommendation::Accept,
        ))];
        let mut store = BTreeMap::new();
        store.insert(custody_digest(ARTIFACT), ARTIFACT.to_vec());
        Case {
            task: t,
            result: r,
            context: Some(ctx),
            manifest: ManifestFreshness::Fresh,
            reviews,
            corrections: vec![],
            store,
            changed: vec![],
            policy: p,
            delivery_findings: vec![],
            candidate: Ok("c".repeat(40)),
        }
    }

    fn judge(&self) -> da::Judgment {
        let retained = |digest: &str| self.store.get(digest).cloned();
        let delivery = DocDelivery {
            reported: vec![],
            findings: self.delivery_findings.clone(),
            eligible: self.delivery_findings.is_empty(),
        };
        da::judge(&Inputs {
            repository: "example/app",
            trusted_base: &"b".repeat(40),
            candidate: self.candidate.as_deref().map_err(Clone::clone),
            task: &self.task,
            delivery: Some(&delivery),
            result_custody: Some(&d("raw")),
            result: Ok(&self.result),
            context: self.context.as_ref(),
            manifest_freshness: self.manifest,
            retained: &retained,
            reviews: &self.reviews,
            corrections: &self.corrections,
            policy: Ok((&self.policy.0, &self.policy.1)),
            changed_paths: Some(&self.changed),
            now: NOW,
        })
    }

    fn subject(&self) -> CoreTuple {
        subject(&self.task, &self.policy.1, ARTIFACT)
    }
}

fn artifact_reasons(j: &da::Judgment) -> Vec<String> {
    j.dimensions
        .iter()
        .find(|d| d.dimension == Dimension::ArtifactValidation)
        .unwrap()
        .reasons
        .clone()
}

fn state(j: &da::Judgment, which: Dimension) -> State {
    j.dimensions
        .iter()
        .find(|d| d.dimension == which)
        .unwrap()
        .state
}

// Section 3.11, last row: every dimension passes; no authority follows.
#[test]
fn every_required_dimension_passing_accepts_and_repeats_the_boundary() {
    let j = Case::passing().judge();
    assert_eq!(j.outcome, Outcome::Accepted, "{:?}", j.reasons);
    assert_eq!(j.dimensions.len(), 12);
    assert_eq!(state(&j, Dimension::ObservedTransport), State::NotRecorded);
    assert!(j.notice.contains("publishing"));
    assert_eq!(j.bound_reviews.len(), 1);
}

// Row 1 and section 3.2: schema passes, review decides.
#[test]
fn a_valid_complete_result_without_review_is_no_acceptance() {
    let mut c = Case::passing();
    c.reviews.clear();
    let j = c.judge();
    assert_eq!(state(&j, Dimension::SchemaValidation), State::Pass);
    assert_eq!(state(&j, Dimension::SemanticReview), State::NotRecorded);
    assert_eq!(j.outcome, Outcome::NoAcceptance);
}

// Rows 3 and 4: a resolved citation is not support; a grade needs its evidence.
#[test]
fn resolved_citations_with_an_insufficient_grade_reject() {
    let mut c = Case::passing();
    c.result = result(&c.task, "tested");
    c.reviews = vec![Read::Valid(review(
        c.subject(),
        "owner@example.invalid",
        ClaimDisposition::InsufficientGrade,
        Recommendation::Revise,
    ))];
    let j = c.judge();
    assert_eq!(state(&j, Dimension::CitationResolution), State::Pass);
    assert_eq!(state(&j, Dimension::SemanticReview), State::Fail);
    assert_eq!(j.outcome, Outcome::Rejected);
}

// Row 6: a required claim left not-reviewed.
#[test]
fn a_not_reviewed_claim_is_incomplete_not_rejected() {
    let mut c = Case::passing();
    c.reviews = vec![Read::Valid(review(
        c.subject(),
        "owner@example.invalid",
        ClaimDisposition::NotReviewed,
        Recommendation::Accept,
    ))];
    let j = c.judge();
    assert_eq!(state(&j, Dimension::SemanticReview), State::NotRecorded);
    assert_eq!(j.outcome, Outcome::NoAcceptance);
}

// Row 9 and section 3.8: changed bytes unbind the earlier review.
#[test]
fn a_correction_unbinds_the_review_of_the_bytes_it_replaced() {
    let mut c = Case::passing();
    let corrected = b"# Parser\n\nCorrected.\n";
    c.store
        .insert(custody_digest(corrected), corrected.to_vec());
    c.corrections = vec![Read::Valid(
        Correction {
            schema: "statecraft/documentation-correction/1".into(),
            subject: c.subject(),
            supersedes: custody_digest(ARTIFACT),
            author: "owner@example.invalid".into(),
            reason: "wording".into(),
            changed_claims: vec![],
            changed_artifacts: vec![ArtifactIdentity {
                logical_path: "docs/parser.md".into(),
                digest: custody_digest(corrected),
            }],
            corrected_result: None,
            digest_construction: String::new(),
            correction_digest: String::new(),
        }
        .seal(),
    )];
    let j = c.judge();
    assert!(j.bound_reviews.is_empty());
    assert!(
        j.reasons
            .iter()
            .any(|r| r.starts_with("review-not-binding"))
    );
    assert_eq!(j.outcome, Outcome::NoAcceptance);
    // A re-review of the corrected subject binds.
    let new_subject = j.subject.clone().unwrap();
    c.reviews.push(Read::Valid(review(
        new_subject,
        "owner@example.invalid",
        ClaimDisposition::Supported,
        Recommendation::Accept,
    )));
    let j = c.judge();
    assert_eq!(j.bound_reviews.len(), 1);
    assert_eq!(j.outcome, Outcome::Accepted, "{:?}", j.reasons);
}

// Row 10: stale context fails regardless of review.
#[test]
fn stale_context_or_an_unrecorded_documentation_manifest_prevents_acceptance() {
    let mut c = Case::passing();
    c.context = Some(context(
        &c.task,
        CitationState::Resolved,
        FreshnessState::Stale,
    ));
    let j = c.judge();
    assert_eq!(state(&j, Dimension::ContextFreshness), State::Fail);
    assert_eq!(j.outcome, Outcome::Rejected);
    let mut c = Case::passing();
    c.manifest = ManifestFreshness::NotRecorded;
    let j = c.judge();
    assert_eq!(state(&j, Dimension::ContextFreshness), State::NotRecorded);
    assert_eq!(j.outcome, Outcome::NoAcceptance);
}

// Rows 11 and 12: the provider cannot review; unknown authority is not
// rejection.
#[test]
fn a_provider_reviewer_fails_and_an_ungranted_reviewer_is_unknown() {
    let mut c = Case::passing();
    c.reviews = vec![Read::Valid(review(
        c.subject(),
        "fixture-provider",
        ClaimDisposition::Supported,
        Recommendation::Accept,
    ))];
    let j = c.judge();
    assert_eq!(state(&j, Dimension::Authority), State::Fail);
    assert_eq!(j.outcome, Outcome::Rejected);

    let mut c = Case::passing();
    c.reviews = vec![Read::Valid(review(
        c.subject(),
        "stranger@example.invalid",
        ClaimDisposition::Supported,
        Recommendation::Accept,
    ))];
    let j = c.judge();
    assert_eq!(state(&j, Dimension::Authority), State::Unknown);
    assert_eq!(j.outcome, Outcome::NoAcceptance);

    let mut c = Case::passing();
    c.policy.0.reviewers[0].not_after = Some("2026-01-01T00:00:00Z".into());
    let j = c.judge();
    assert_eq!(state(&j, Dimension::Authority), State::Unknown);
}

// Section 3.6: missing bytes, a broken internal anchor, an unexpected artifact.
#[test]
fn artifact_validation_reads_only_retained_bytes() {
    let mut c = Case::passing();
    c.store.clear();
    assert_eq!(
        state(&c.judge(), Dimension::ArtifactValidation),
        State::NotRecorded
    );
    let broken = b"# Parser\n\nSee [missing](#nowhere).\n";
    // Bytes that are not the recorded digest's fail before any anchor is read.
    let mut c = Case::passing();
    c.store.insert(custody_digest(ARTIFACT), broken.to_vec());
    let j = c.judge();
    assert_eq!(state(&j, Dimension::ArtifactValidation), State::Fail);
    assert!(
        artifact_reasons(&j)
            .iter()
            .any(|r| r.contains("docs/parser.md digest")),
        "{:?}",
        artifact_reasons(&j)
    );
    // The recorded digest is the broken bytes' own, so only the anchor fails.
    let mut c = Case::passing();
    c.result.artifacts[0].digest = custody_digest(broken);
    c.store.insert(custody_digest(broken), broken.to_vec());
    let j = c.judge();
    assert_eq!(state(&j, Dimension::ArtifactValidation), State::Fail);
    assert!(
        artifact_reasons(&j)
            .iter()
            .any(|r| r.contains("links to a missing anchor #nowhere")),
        "{:?}",
        artifact_reasons(&j)
    );
    assert!(
        !artifact_reasons(&j).iter().any(|r| r.contains("digest")),
        "{:?}",
        artifact_reasons(&j)
    );
    // A fragment matches its heading regardless of case.
    let cased = b"# Parser\n\n## Recovery\n\nSee [recovery](#Recovery).\n";
    let mut c = Case::passing();
    c.result.artifacts[0].digest = custody_digest(cased);
    c.store.insert(custody_digest(cased), cased.to_vec());
    let j = c.judge();
    assert_eq!(
        state(&j, Dimension::ArtifactValidation),
        State::Pass,
        "{:?}",
        artifact_reasons(&j)
    );
}

// Section 3.3 row 12: an expected artifact written into the candidate.
#[test]
fn an_artifact_applied_to_the_candidate_breaks_the_publication_boundary() {
    let mut c = Case::passing();
    c.changed = vec!["docs/parser.md".into()];
    let j = c.judge();
    assert_eq!(state(&j, Dimension::PublicationBoundary), State::Fail);
    assert_eq!(j.outcome, Outcome::Rejected);
}

// An unread candidate leaves the subject unidentified; it is never an empty
// identity.
#[test]
fn an_unread_candidate_identifies_no_subject() {
    let mut c = Case::passing();
    c.candidate = Err("the workspace is absent".into());
    let j = c.judge();
    assert!(j.subject.is_none());
    assert_eq!(state(&j, Dimension::TaskBinding), State::NotRecorded);
    assert_ne!(j.outcome, Outcome::Accepted);
    assert!(
        j.reasons
            .iter()
            .any(|r| r == "candidate-unidentified: the workspace is absent"),
        "{:?}",
        j.reasons
    );
}

// Required transport cannot be satisfied by adapter testimony.
#[test]
fn required_transport_is_never_filled_by_testimony() {
    let mut c = Case::passing();
    c.policy.0.observed_transport = Need::Required;
    let j = c.judge();
    assert_eq!(state(&j, Dimension::ObservedTransport), State::NotRecorded);
    assert_eq!(j.outcome, Outcome::NoAcceptance);
}

#[test]
fn a_failed_citation_and_a_refused_evidence_reference_fail_their_own_rows() {
    let mut c = Case::passing();
    let mut ctx = context(&c.task, CitationState::OutsidePacket, FreshnessState::Fresh);
    ctx.evidence[0].decision = "refuse".into();
    c.context = Some(ctx);
    let j = c.judge();
    assert_eq!(state(&j, Dimension::CitationResolution), State::Fail);
    assert_eq!(state(&j, Dimension::EvidenceAdmission), State::Fail);
    assert_eq!(state(&j, Dimension::SemanticReview), State::Pass);
    assert_eq!(j.outcome, Outcome::Rejected);
}

// A failed testimony is never softened by another absence.
#[test]
fn failed_delivery_testimony_stays_failed_beside_missing_context() {
    let mut c = Case::passing();
    c.context = None;
    c.delivery_findings = vec!["mismatch: taskDigest".into()];
    let j = c.judge();
    assert_eq!(state(&j, Dimension::AdapterObservedDelivery), State::Fail);
    assert_eq!(j.outcome, Outcome::Rejected);
}
