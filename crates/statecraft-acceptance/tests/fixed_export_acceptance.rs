//! Spec 022's workflow judgment: spec 021's judgment of the current subject
//! is never bettered by the workflow; a workflow finding withholds
//! acceptance and a prohibited effect rejects; a corrected generation is a
//! new subject; a proposed patch is read as data and never applied (sections
//! 3.6, 3.8, 3.9 and 3.12). Every input is a fixture.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde_json::json;

use statecraft_acceptance::context::{
    Citation, CitationResult, CitationState, ClosureState, ContextResult, EvidenceResult,
    FreshnessState,
};
use statecraft_acceptance::documentation_acceptance::{
    self as da, Grant, Inputs, ManifestFreshness, Need, Outcome, Policy,
};
use statecraft_acceptance::fixed_export::{self as fx, WorkflowJudgment};
use statecraft_run::context::{ContextPolicy, Delivery, Need as ContextNeed, custody_digest};
use statecraft_run::documentation::delivery::Delivery as DocDelivery;
use statecraft_run::documentation::review::{
    ArtifactDisposition, ArtifactIdentity, ArtifactReview, Authority, ClaimDisposition,
    ClaimReview, CoreTuple, Correction, Read, Recommendation, Review, Reviewer,
};
use statecraft_run::documentation::workflow::{
    self, AdapterRef, Subjects, Transition, Workflow, WorkflowState,
};
use statecraft_run::documentation::{
    self as doc, Artifact, Candidate, ClockSource, CostCeiling, Deadline, Disposition,
    DocumentationResult, DocumentationTask, Format, NoSpend, NotRequired, Operations, OutputFormat,
    Publication, ResultSchemaRef, ReviewPolicy, SemanticReview, Skill,
};
use statecraft_run::record::Chain;

const DRAFT: &[u8] = b"# Recovery\n\nAt the next statement.\n";
const CLEAN_PATCH: &[u8] = b"base-commit: cccccccccccccccccccccccccccccccccccccccc\n\
--- a/docs/recovery.md\n+++ b/docs/recovery.md\n@@ -1 +1 @@\n-old\n+new\n";
const NOW: &str = "2026-10-28T00:00:00Z";
const OWNER: &str = "owner@example.invalid";

fn d(t: &str) -> String {
    custody_digest(t.as_bytes())
}

fn task(disposition: Disposition) -> DocumentationTask {
    DocumentationTask {
        schema: doc::TASK_SCHEMA.into(),
        task_id: "task-022".into(),
        run_id: "run-022".into(),
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
            path: "docs/recovery.md".into(),
            disposition,
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

fn result(t: &DocumentationTask, artifact: &[u8]) -> DocumentationResult {
    let c = citation();
    let v = json!({
        "schema": doc::RESULT_SCHEMA,
        "taskId": t.task_id, "runId": t.run_id, "attempt": 1,
        "taskDigest": t.task_digest, "contextManifestDigest": t.context_manifest_digest,
        "adapter": {"name": "fixed-export-fake", "version": "1"},
        "provider": {"name": "fixed-export-fake-provider", "version": "1"},
        "artifacts": [{
            "logicalPath": "docs/recovery.md", "mediaType": "text/markdown",
            "digest": custody_digest(artifact), "size": artifact.len(), "rawRef": "a0"
        }],
        "claims": [{
            "claimId": "c1", "text": "Recovery happens at the next statement.",
            "lifecycleGrade": "implemented",
            "citations": [{"repository": c.repository, "pageDigest": c.page_digest,
                           "member": c.member, "contentDigest": c.content_digest}],
            "evidenceRefs": ["test-run-1"], "unsupportedReason": "not-applicable",
            "contradictionRefs": [], "reviewDisposition": "not-reviewed"
        }],
        "unknowns": [], "contradictions": [], "unsupportedClaims": [],
        "providerOmissions": [], "statecraftCitationFailures": [],
        "executionEvidence": [{"reference": "test-run-1", "purpose": "the suite"}],
        "providerCompletion": "claimed-complete",
        "reviewDisposition": "not-reviewed", "acceptanceResult": "not-attempted"
    });
    DocumentationResult::decode(&serde_json::to_vec(&v).unwrap(), t).unwrap()
}

fn context(t: &DocumentationTask) -> ContextResult {
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
        freshness_aggregate: FreshnessState::Fresh,
        citations: vec![CitationResult {
            citation: citation(),
            state: CitationState::Resolved,
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
            identity: OWNER.into(),
            authority: Authority::RepositoryOwner,
            not_after: None,
        }],
        quorum: 1,
        observed_transport: Need::Optional,
        max_artifact_bytes: 65_536,
    };
    Policy::decode(&serde_json::to_vec(&p).unwrap()).unwrap()
}

fn review(subject: CoreTuple, who: &str, rec: Recommendation) -> Read<Review> {
    Read::Valid(
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
                disposition: ClaimDisposition::Supported,
                reasons: vec![],
            }],
            artifacts: vec![ArtifactReview {
                logical_path: "docs/recovery.md".into(),
                disposition: if rec == Recommendation::Accept {
                    ArtifactDisposition::Acceptable
                } else {
                    ArtifactDisposition::NeedsCorrection
                },
                reasons: vec![],
            }],
            corrections: vec![],
            recommendation: rec,
            digest_construction: String::new(),
            review_digest: String::new(),
        }
        .seal(),
    )
}

/// One spec 021 judgment over fixtures, and the workflow that produced it.
struct Case {
    task: DocumentationTask,
    result: DocumentationResult,
    artifact: Vec<u8>,
    reviews: Vec<Read<Review>>,
    corrections: Vec<Read<Correction>>,
    store: BTreeMap<String, Vec<u8>>,
    policy: (Policy, String),
    workflow: Workflow,
}

fn sealed(mut w: Workflow) -> Workflow {
    w.digest_construction = workflow::WORKFLOW_DIGEST_CONSTRUCTION.into();
    w.workflow_digest = w.computed_digest();
    w
}

fn validated(t: &DocumentationTask) -> Workflow {
    let at = |state| Transition {
        state,
        reason: "fixture".into(),
        at: NOW.into(),
    };
    sealed(Workflow {
        schema: workflow::WORKFLOW_SCHEMA.into(),
        profile: workflow::PROFILE.into(),
        project: "example/app".into(),
        run_id: t.run_id.clone(),
        attempt: 1,
        candidate: "c".repeat(40),
        subjects: Subjects {
            manifest_digest: t.context_manifest_digest.clone(),
            task_digest: t.task_digest.clone(),
            skill: "not-required".into(),
            result_schema_digest: t.result_schema.digest.clone(),
            adapter: Some(AdapterRef {
                name: "fixed-export-fake".into(),
                version: "1".into(),
                capabilities: vec![],
            }),
            raw_result: Some(d("raw")),
            ..Subjects::default()
        },
        transitions: vec![
            at(WorkflowState::Prepared),
            at(WorkflowState::Executed),
            at(WorkflowState::Validated),
        ],
        findings: vec![],
        digest_construction: String::new(),
        workflow_digest: String::new(),
    })
}

impl Case {
    fn with(disposition: Disposition, artifact: &[u8]) -> Self {
        let t = task(disposition);
        let mut store = BTreeMap::new();
        store.insert(custody_digest(artifact), artifact.to_vec());
        let mut c = Case {
            result: result(&t, artifact),
            workflow: validated(&t),
            task: t,
            artifact: artifact.to_vec(),
            reviews: vec![],
            corrections: vec![],
            store,
            policy: policy(),
        };
        c.reviews = vec![review(c.subject(), OWNER, Recommendation::Accept)];
        c
    }

    fn passing() -> Self {
        Self::with(Disposition::Draft, DRAFT)
    }

    fn subject(&self) -> CoreTuple {
        CoreTuple {
            repository: "example/app".into(),
            trusted_base: "b".repeat(40),
            run_id: self.task.run_id.clone(),
            attempt: 1,
            candidate: "c".repeat(40),
            context_manifest_digest: self.task.context_manifest_digest.clone(),
            task_digest: self.task.task_digest.clone(),
            result_custody_digest: d("raw"),
            result_schema_digest: self.task.result_schema.digest.clone(),
            artifacts: vec![ArtifactIdentity {
                logical_path: "docs/recovery.md".into(),
                digest: custody_digest(&self.artifact),
            }],
            evidence: vec!["test-run-1".into()],
            policy_digest: self.policy.1.clone(),
        }
    }

    fn documentation(&self) -> da::Judgment {
        let retained = |digest: &str| self.store.get(digest).cloned();
        let delivery = DocDelivery {
            reported: vec![],
            findings: vec![],
            eligible: true,
        };
        let ctx = context(&self.task);
        da::judge(&Inputs {
            repository: "example/app",
            trusted_base: &"b".repeat(40),
            candidate: Ok(&"c".repeat(40)),
            task: &self.task,
            delivery: Some(&delivery),
            result_custody: Some(&d("raw")),
            result: Ok(&self.result),
            context: Some(&ctx),
            manifest_freshness: ManifestFreshness::Fresh,
            retained: &retained,
            reviews: &self.reviews,
            corrections: &self.corrections,
            policy: Ok((&self.policy.0, &self.policy.1)),
            changed_paths: Some(&[]),
            now: NOW,
        })
    }

    fn judge(&self) -> WorkflowJudgment {
        let retained = |digest: &str| self.store.get(digest).cloned();
        fx::judge(
            Ok(&self.workflow),
            &self.task,
            self.documentation(),
            &retained,
        )
    }
}

// Section 3.9: acceptance is spec 021's result over the full tuple, and the
// boundary is repeated.
#[test]
fn a_clean_workflow_over_an_accepted_subject_accepts_and_stays_inert() {
    let c = Case::passing();
    let j = c.judge();
    assert_eq!(j.documentation.outcome, Outcome::Accepted);
    assert_eq!(j.outcome, Outcome::Accepted, "{:?}", j.reasons);
    assert_eq!(j.profile, workflow::PROFILE);
    assert_eq!(
        j.workflow_digest.as_deref(),
        Some(c.workflow.workflow_digest.as_str())
    );
    assert_eq!(j.subject_digest, Some(fx::subject_digest(&c.subject())));
    assert!(j.notice.contains("publishing"));
}

// Section 3.9: neither successful fake execution nor a clean workflow can
// produce `accepted` on its own.
#[test]
fn the_workflow_never_betters_the_documentation_judgment() {
    let mut c = Case::passing();
    c.reviews.clear();
    let j = c.judge();
    assert_eq!(j.documentation.outcome, Outcome::NoAcceptance);
    assert_eq!(j.outcome, Outcome::NoAcceptance);

    let mut c = Case::passing();
    c.reviews = vec![review(c.subject(), OWNER, Recommendation::Reject)];
    let j = c.judge();
    assert_eq!(j.documentation.outcome, j.outcome);
    assert_ne!(j.outcome, Outcome::Accepted);
}

// Sections 3.4 and 3.12 row 11: a workflow finding withholds acceptance and a
// prohibited effect rejects.
#[test]
fn a_workflow_finding_withholds_and_a_prohibited_effect_rejects() {
    let mut c = Case::passing();
    c.workflow.findings = vec!["second-terminal: event 3".into()];
    c.workflow = sealed(c.workflow);
    let j = c.judge();
    assert_eq!(j.documentation.outcome, Outcome::Accepted);
    assert_eq!(j.outcome, Outcome::NoAcceptance);
    assert!(
        j.reasons
            .contains(&"workflow-finding: second-terminal: event 3".to_string())
    );

    c.workflow.findings = vec!["prohibited-effect: network https://example.invalid".into()];
    c.workflow = sealed(c.workflow);
    assert_eq!(c.judge().outcome, Outcome::Rejected);
}

/// One change to a validated workflow.
type Change = fn(&mut Workflow);

// Section 3.1: the workflow and the judged subject must name the same things.
#[test]
fn a_workflow_that_is_not_validated_or_names_other_subjects_withholds() {
    let base = Case::passing();
    let cases: [(&str, Change); 4] = [
        ("workflow-not-validated", |w| w.transitions.truncate(2)),
        ("workflow-subject: raw result", |w| {
            w.subjects.raw_result = Some(custody_digest(b"other"))
        }),
        ("workflow-adapter", |w| w.subjects.adapter = None),
        ("workflow-digest", |_| {}),
    ];
    for (code, change) in cases {
        let mut c = Case::passing();
        change(&mut c.workflow);
        if code == "workflow-digest" {
            c.workflow.workflow_digest = d("tampered");
        } else {
            c.workflow = sealed(c.workflow);
        }
        let j = c.judge();
        assert_eq!(j.outcome, Outcome::NoAcceptance, "{code}");
        assert!(
            j.reasons.iter().any(|r| r.starts_with(code)),
            "{code}: {:?}",
            j.reasons
        );
    }
    let retained = |digest: &str| base.store.get(digest).cloned();
    let j = fx::judge(
        Err("unreadable"),
        &base.task,
        base.documentation(),
        &retained,
    );
    assert_eq!(j.outcome, Outcome::NoAcceptance);
    assert_eq!(j.workflow_digest, None);
}

// Sections 3.8 and 3.12 row 9: a corrected byte is a new subject; the first
// judgment does not bind it, and only a new complete judgment accepts it.
#[test]
fn a_correction_is_a_new_subject_that_only_a_new_judgment_accepts() {
    let mut c = Case::passing();
    c.reviews = vec![review(c.subject(), OWNER, Recommendation::Revise)];
    let first = c.judge();
    assert_ne!(first.outcome, Outcome::Accepted);

    let corrected = b"# Recovery\n\nAt the next statement boundary.\n";
    c.store
        .insert(custody_digest(corrected), corrected.to_vec());
    c.corrections = vec![Read::Valid(
        Correction {
            schema: "statecraft/documentation-correction/1".into(),
            subject: c.subject(),
            supersedes: custody_digest(DRAFT),
            author: OWNER.into(),
            reason: "precision".into(),
            changed_claims: vec![],
            changed_artifacts: vec![ArtifactIdentity {
                logical_path: "docs/recovery.md".into(),
                digest: custody_digest(corrected),
            }],
            corrected_result: None,
            digest_construction: String::new(),
            correction_digest: String::new(),
        }
        .seal(),
    )];
    let between = c.judge();
    assert_ne!(between.subject_digest, first.subject_digest);
    assert_eq!(between.outcome, Outcome::NoAcceptance);

    let second_subject = between.documentation.subject.clone().unwrap();
    c.reviews
        .push(review(second_subject, OWNER, Recommendation::Accept));
    let second = c.judge();
    assert_eq!(second.outcome, Outcome::Accepted, "{:?}", second.reasons);
    assert_eq!(second.subject_digest, between.subject_digest);
}

// Sections 3.6 and 3.12 row 7: a clean patch is read and stays unapplied.
#[test]
fn a_proposed_patch_is_read_as_data_and_never_applied() {
    let r = fx::inspect_patch("docs/recovery.patch", CLEAN_PATCH);
    assert_eq!(r.base, "c".repeat(40));
    assert_eq!(r.paths, vec!["docs/recovery.md".to_string()]);
    assert!(!r.binary);
    assert_eq!(r.applicability, "not-recorded");
    assert!(r.findings.is_empty(), "{:?}", r.findings);
    assert_eq!(r.digest, custody_digest(CLEAN_PATCH));

    let binary = fx::inspect_patch("p", b"--- a/x.png\n+++ b/x.png\nGIT binary patch\n");
    assert!(binary.binary);

    let c = Case::with(Disposition::Patch, CLEAN_PATCH);
    let j = c.judge();
    assert_eq!(j.patches.len(), 1);
    assert!(
        j.reasons.iter().all(|r| !r.starts_with("patch")),
        "{:?}",
        j.reasons
    );
}

// Section 3.10's path-escape fixture: a patch that climbs out rejects.
#[test]
fn a_patch_whose_paths_escape_rejects() {
    let escaping: &[u8] = b"--- a/../outside.md\n+++ b/../outside.md\n@@ -1 +1 @@\n-a\n+b\n";
    let r = fx::inspect_patch("docs/recovery.md", escaping);
    assert!(r.findings.iter().any(|f| f.starts_with("path-escape")));
    let absolute = fx::inspect_patch("p", b"--- /etc/passwd\n+++ /etc/passwd\n");
    assert!(
        absolute
            .findings
            .iter()
            .any(|f| f.starts_with("path-escape"))
    );

    let c = Case::with(Disposition::Patch, escaping);
    let j = c.judge();
    assert_eq!(j.outcome, Outcome::Rejected, "{:?}", j.reasons);
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}");
}

// Section 3.9: recording appends the judgment and a `judged` version; the
// earlier version stays, and nothing is applied.
#[test]
fn recording_appends_the_judgment_and_a_judged_version() {
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    git(target.path(), &["init", "--quiet"]);
    let c = Case::passing();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    let w = workflow::advance(
        &mut chain,
        &c.workflow,
        WorkflowState::Validated,
        "bound",
        NOW,
        |_| {},
    )
    .unwrap();
    let j = c.judge();
    let judged = fx::record(&mut chain, &w, &j, NOW).unwrap();
    assert_eq!(judged.state(), Some(WorkflowState::Judged));
    assert_eq!(judged.transitions.last().unwrap().reason, "accepted");
    assert_eq!(judged.subjects.acceptances, vec![j.digest()]);

    let chain = Chain::open(home.path(), target.path()).unwrap().0;
    let versions = workflow::versions(&chain, "run-022", 1);
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0].as_ref().unwrap(), &w);
    let recorded = chain
        .entries()
        .into_iter()
        .filter(|e| e.subject == fx::ACCEPTANCE_SUBJECT)
        .count();
    assert_eq!(recorded, 1);
    assert!(!target.path().join("docs/recovery.md").exists());
}
