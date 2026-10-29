//! Spec 022's deterministic pilot through the command bindings (sections
//! 3.10 to 3.12): two revision-pinned exports, a task asking for a draft and
//! a proposed patch, the fake adapter, a first generation that is not
//! accepted, a corrected second generation that is, and the boundary that
//! nothing is applied or published. Producer answers are fixtures; every
//! record is the real chain, reopened from disk.

use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};

use statecraft_acceptance::context::{
    Citation, CitationResult, CitationState, ClosureState, ContextResult, EvidenceResult,
    FreshnessState,
};
use statecraft_acceptance::documentation_acceptance::{
    self as da, Inputs, ManifestFreshness, Outcome,
};
use statecraft_adapter::documentation::RawArtifact;
use statecraft_adapter::fixed_export::{self as fx, Effect, ProfileCapability, Step};
use statecraft_cli::fixed_export::{self as cli, AcceptanceView, WorkflowView};
use statecraft_environment::time::FixedClock;
use statecraft_run::context::{
    self, AcceptedPage, AcceptedRepository, Completeness, ContextPolicy, Delivery, DeliveryPolicy,
    Intended, Manifest, Need as ContextNeed, OptionalOmission, PageFacts, Producer, ProducerBuild,
    Requirement, Snapshot, Task,
};
use statecraft_run::documentation::review::{
    self, ArtifactDisposition, ArtifactReview, Authority, ClaimDisposition, ClaimReview, CoreTuple,
    Correction, Recommendation, Review, Reviewer,
};
use statecraft_run::documentation::workflow::WorkflowState;
use statecraft_run::documentation::{
    self as doc, Artifact, Candidate, ClockSource, CostCeiling, Deadline, Disposition,
    DocumentationResult, DocumentationTask, Format, NoSpend, NotRequired, Operations, OutputFormat,
    Projection, Publication, ResultProjection, ResultSchemaRef, ReviewPolicy, SemanticReview,
    Skill,
};
use statecraft_run::record::Chain;

const RUN: &str = "run-022";
const OWNER: &str = "owner@example.invalid";
const NOW: &str = "2026-10-28T00:00:00Z";
const DRAFT_1: &str = "# Recovery\n\nAt the next statement.\n";
const DRAFT_2: &str = "# Recovery\n\nAt the next statement!\n";
const PATCH: &str = "base-commit: 0000000000000000000000000000000000000000\n\
--- a/docs/recovery.md\n+++ b/docs/recovery.md\n@@ -1 +1 @@\n-old\n+new\n";
const PAGES: [(&str, &[u8]); 2] = [
    ("app", b"{\"page\":\"app\"}"),
    ("lib", b"{\"page\":\"lib\"}"),
];

fn clock() -> FixedClock {
    FixedClock(1_790_000_000)
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// A repository whose trusted base grants `OWNER` review authority.
fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        &["init", "--quiet", "--initial-branch=main"][..],
        &["config", "user.email", "test@example.invalid"],
        &["config", "user.name", "test"],
        &["config", "commit.gpgsign", "false"],
    ] {
        git(dir.path(), args);
    }
    std::fs::create_dir_all(dir.path().join(".statecraft")).unwrap();
    std::fs::write(
        dir.path().join(da::POLICY_PATH),
        serde_json::to_vec(&json!({
            "schema": da::POLICY_SCHEMA,
            "reviewers": [{"identity": OWNER, "authority": "repository-owner"}],
            "quorum": 1,
            "observedTransport": "optional",
            "maxArtifactBytes": 65536
        }))
        .unwrap(),
    )
    .unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "--quiet", "-m", "policy"]);
    dir
}

fn d(t: &str) -> String {
    context::custody_digest(t.as_bytes())
}

fn manifest(attempt: u32, task_id: &str, revision: &str) -> Manifest {
    let repositories: Vec<AcceptedRepository> = PAGES
        .iter()
        .map(|(id, bytes)| AcceptedRepository {
            id: (*id).into(),
            role: "source".into(),
            requirement: Requirement::Required,
            pages: vec![AcceptedPage {
                bytes: bytes.to_vec(),
                source_operation: "context_packet_pages".into(),
                facts: PageFacts {
                    schema_version: "1.1.0".into(),
                    producer: ProducerBuild {
                        package: "spec-spine".into(),
                        version: "0.31.0".into(),
                        build: "unverified".into(),
                    },
                    snapshot: Snapshot {
                        repository: format!("example/{id}"),
                        revision: revision.into(),
                        tree: "e".repeat(40),
                        dirty_state: "clean-export".into(),
                    },
                    request_digest: d("request"),
                    closure_digest: d("closure"),
                    packet_digest: d(&format!("page-{id}")),
                    completeness: Completeness::Complete,
                    continues: false,
                    omissions: vec![],
                    warnings: vec![],
                },
                delivery: Intended::Required,
            }],
        })
        .collect();
    Manifest::build(
        RUN,
        attempt,
        Task::Id(task_id.into()),
        &Producer {
            package: "spec-spine".into(),
            version: "0.31.0".into(),
            packet_interface: "1.1.0".into(),
        },
        &repositories,
        DeliveryPolicy::new(OptionalOmission::Refuse),
    )
    .unwrap()
}

fn task(m: &Manifest, task_id: &str, candidate: &str) -> DocumentationTask {
    DocumentationTask {
        schema: doc::TASK_SCHEMA.into(),
        task_id: task_id.into(),
        run_id: RUN.into(),
        attempt: m.attempt,
        candidate: Candidate {
            repository: "app".into(),
            revision: candidate.into(),
            tree: "tree".into(),
        },
        context_manifest_digest: m.manifest_digest.clone(),
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
        artifacts: vec![
            Artifact {
                path: "docs/recovery.md".into(),
                disposition: Disposition::Draft,
                required_context: vec![],
            },
            Artifact {
                path: "docs/recovery.patch".into(),
                disposition: Disposition::Patch,
                required_context: vec![],
            },
        ],
        review_policy: ReviewPolicy {
            reviewers: vec![OWNER.into()],
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

fn citation(repository: &str) -> Citation {
    Citation {
        repository: repository.into(),
        page_digest: d(&format!("page-{repository}")),
        member: "src/recovery.rs".into(),
        content_digest: d(&format!("member-{repository}")),
        span: None,
    }
}

fn claim(id: &str, grade: &str, c: &Citation) -> Value {
    json!({
        "claimId": id, "text": format!("Claim {id}."), "lifecycleGrade": grade,
        "citations": [{"repository": c.repository, "pageDigest": c.page_digest,
                       "member": c.member, "contentDigest": c.content_digest}],
        "evidenceRefs": ["suite-1"], "unsupportedReason": "not-applicable",
        "contradictionRefs": [], "reviewDisposition": "not-reviewed"
    })
}

/// The fake's result: generation 1 carries an insufficient-grade claim on
/// an unresolved citation; both carry an unknown, a contradiction and a
/// provider omission.
fn result(t: &DocumentationTask, draft: &str, first: bool) -> Vec<u8> {
    let (adapter, provider) = fx::fake_parties();
    let mut claims = vec![claim("c1", "implemented", &citation("app"))];
    if first {
        claims.push(claim("c2", "tested", &citation("lib")));
    }
    let artifact = |path: &str, media: &str, bytes: &str, raw: &str| {
        json!({"logicalPath": path, "mediaType": media, "digest": d(bytes),
               "size": bytes.len(), "rawRef": raw})
    };
    serde_json::to_vec(&json!({
        "schema": doc::RESULT_SCHEMA,
        "taskId": t.task_id, "runId": t.run_id, "attempt": t.attempt,
        "taskDigest": t.task_digest, "contextManifestDigest": t.context_manifest_digest,
        "adapter": adapter, "provider": provider,
        "artifacts": [
            artifact("docs/recovery.md", "text/markdown", draft, "a0"),
            artifact("docs/recovery.patch", "text/x-diff", PATCH, "a1"),
        ],
        "claims": claims,
        "unknowns": [{"unknownId": "u1", "question": "Is recovery bounded?",
                      "missingFact": "a bound", "reason": "not in the exports"}],
        "contradictions": [{"contradictionId": "x1", "claimRefs": ["c1"],
                            "sourceRefs": [], "description": "Two pages disagree."}],
        "unsupportedClaims": [],
        "providerOmissions": [{"member": "benches/recovery.rs", "reason": "out of scope"}],
        "statecraftCitationFailures": [],
        "executionEvidence": [{"reference": "suite-1", "purpose": "the suite"}],
        "providerCompletion": "claimed-complete",
        "reviewDisposition": "not-reviewed", "acceptanceResult": "not-attempted"
    }))
    .unwrap()
}

fn script(t: &DocumentationTask, draft: &str, first: bool) -> Vec<Step> {
    vec![
        Step::Init,
        Step::Progress("drafting".into()),
        Step::TerminalWithArtifacts(
            result(t, draft, first),
            vec![
                RawArtifact {
                    raw_ref: "a0".into(),
                    bytes: draft.into(),
                },
                RawArtifact {
                    raw_ref: "a1".into(),
                    bytes: PATCH.into(),
                },
            ],
        ),
    ]
}

struct Pilot {
    home: tempfile::TempDir,
    target: tempfile::TempDir,
    base: String,
}

fn pilot() -> Pilot {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let base = git(target.path(), &["rev-parse", "HEAD"]);
    for (_, bytes) in PAGES {
        context::retain(home.path(), bytes).unwrap();
    }
    Pilot { home, target, base }
}

impl Pilot {
    fn chain(&self) -> Chain {
        Chain::open(self.home.path(), self.target.path()).unwrap().0
    }

    fn prepare(&self, m: &Manifest, t: &DocumentationTask) -> Result<cli::Prepared, String> {
        cli::prepare(
            self.target.path(),
            self.home.path(),
            "main",
            m,
            t,
            None,
            &fx::fake_declaration().policy(),
            &clock(),
        )
    }

    /// Prepare and execute one generation; returns its task.
    fn generation(
        &self,
        attempt: u32,
        task_id: &str,
        draft: &str,
        first: bool,
    ) -> DocumentationTask {
        let m = manifest(attempt, task_id, &"a".repeat(40));
        let t = task(&m, task_id, &self.base);
        let session = self.prepare(&m, &t).unwrap().session.unwrap();
        let w = cli::execute(
            self.target.path(),
            self.home.path(),
            &session,
            &[fx::fake_declaration()],
            &script(&t, draft, first),
            &clock(),
        )
        .unwrap();
        assert_eq!(w.state(), Some(WorkflowState::Validated));
        assert!(w.findings.is_empty(), "{:?}", w.findings);
        t
    }

    fn attempt(&self, n: u32) -> statecraft_run::Attempt {
        statecraft_run::session::runs(&self.chain())
            .into_iter()
            .find(|r| r.id == RUN)
            .unwrap()
            .attempts
            .into_iter()
            .find(|a| a.number == n)
            .unwrap()
    }

    /// Spec 021's judgment of the attempt from the record, with the
    /// producer's answers as fixtures: `c1`'s citation resolves, `c2`'s does
    /// not, and the documentation manifest is fresh.
    fn documentation(&self, t: &DocumentationTask) -> da::Judgment {
        let chain = self.chain();
        let a = self.attempt(t.attempt);
        let custody = match doc::project(&chain, RUN, t.attempt) {
            Projection::Recorded {
                result: ResultProjection::Recorded(r),
                ..
            } => r.raw.custody_digest,
            other => panic!("{other:?}"),
        };
        let bytes = context::read_retained(self.home.path(), &custody)
            .unwrap()
            .unwrap();
        let decoded = DocumentationResult::decode(&bytes, t).unwrap();
        let delivery = doc::delivery::recorded_delivery(&chain, t);
        let policy_bytes = std::fs::read(self.target.path().join(da::POLICY_PATH)).unwrap();
        let (policy, policy_digest) = da::Policy::decode(&policy_bytes).unwrap();
        let workspace = statecraft_run::workspace::workspace_path(self.target.path(), RUN);
        let candidate = git(&workspace, &["rev-parse", "HEAD"]);
        let ctx = context_result(t, &decoded);
        let reviews = review::recorded_reviews(&chain, RUN, t.attempt);
        let corrections = review::recorded_corrections(&chain, RUN, t.attempt);
        let retained = |digest: &str| {
            context::read_retained(self.home.path(), digest)
                .ok()
                .flatten()
        };
        da::judge(&Inputs {
            repository: &self.target.path().display().to_string(),
            trusted_base: &a.base_commit,
            candidate: Ok(&candidate),
            task: t,
            delivery: delivery.as_ref(),
            result_custody: Some(&custody),
            result: Ok(&decoded),
            context: Some(&ctx),
            manifest_freshness: ManifestFreshness::Fresh,
            retained: &retained,
            reviews: &reviews,
            corrections: &corrections,
            policy: Ok((&policy, &policy_digest)),
            changed_paths: Some(&[]),
            now: NOW,
        })
    }

    /// Spec 021's judgment of an attempt that has no result yet.
    fn documentation_unjudged(&self, t: &DocumentationTask) -> da::Judgment {
        let retained = |_: &str| None;
        da::judge(&Inputs {
            repository: &self.target.path().display().to_string(),
            trusted_base: &self.base,
            candidate: Ok(&self.base),
            task: t,
            delivery: None,
            result_custody: None,
            result: Err("result-not-recorded".into()),
            context: None,
            manifest_freshness: ManifestFreshness::NotRecorded,
            retained: &retained,
            reviews: &[],
            corrections: &[],
            policy: Err("not read".into()),
            changed_paths: None,
            now: NOW,
        })
    }

    fn review(&self, subject: CoreTuple, who: &str, t: &DocumentationTask, accept: bool) -> Review {
        let claims = doc_claims(t, accept);
        Review {
            schema: review::REVIEW_SCHEMA.into(),
            subject,
            reviewer: Reviewer {
                identity: who.into(),
                authority: Authority::RepositoryOwner,
                authority_source: "policy reviewers[0]".into(),
            },
            reviewed_at: NOW.into(),
            claims,
            artifacts: ["docs/recovery.md", "docs/recovery.patch"]
                .into_iter()
                .map(|p| ArtifactReview {
                    logical_path: p.into(),
                    disposition: if accept {
                        ArtifactDisposition::Acceptable
                    } else {
                        ArtifactDisposition::NeedsCorrection
                    },
                    reasons: vec![],
                })
                .collect(),
            corrections: vec![],
            recommendation: if accept {
                Recommendation::Accept
            } else {
                Recommendation::Revise
            },
            digest_construction: String::new(),
            review_digest: String::new(),
        }
        .seal()
    }

    fn submit(&self, r: &Review, attempt: u32) -> Result<(), String> {
        cli::submit_review(
            self.target.path(),
            self.home.path(),
            RUN,
            attempt,
            &serde_json::to_vec(r).unwrap(),
            &clock(),
        )
        .map(|_| ())
    }

    /// The operator checkout's status, apart from the supervisor's own run
    /// state, and the prepared workspace's: neither may change.
    fn status(&self) -> String {
        let operator = git(
            self.target.path(),
            &["status", "--porcelain", "--untracked-files=all"],
        );
        let operator: Vec<&str> = operator
            .lines()
            .filter(|l| !l.starts_with("?? .statecraft/state/"))
            .collect();
        let workspace = statecraft_run::workspace::workspace_path(self.target.path(), RUN);
        let workspace = if workspace.exists() {
            git(
                &workspace,
                &["status", "--porcelain", "--untracked-files=all"],
            ) + &git(&workspace, &["rev-parse", "HEAD"])
        } else {
            String::new()
        };
        format!("{}\n{workspace}", operator.join("\n"))
    }
}

/// Generation 1 asks for `c2`'s grade to be revised; generation 2 has no
/// `c2`.
fn doc_claims(t: &DocumentationTask, accept: bool) -> Vec<ClaimReview> {
    let mut claims = vec![ClaimReview {
        claim_id: "c1".into(),
        disposition: ClaimDisposition::Supported,
        reasons: vec![],
    }];
    if t.attempt == 1 && !accept {
        claims.push(ClaimReview {
            claim_id: "c2".into(),
            disposition: ClaimDisposition::InsufficientGrade,
            reasons: vec!["tested is claimed, the evidence is a design".into()],
        });
    }
    claims
}

fn context_result(t: &DocumentationTask, r: &DocumentationResult) -> ContextResult {
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
        citations: r
            .claims
            .iter()
            .flat_map(|c| c.citations.iter())
            .map(|c| {
                let citation = citation(&c.repository);
                CitationResult {
                    state: if citation.repository == "app" {
                        CitationState::Resolved
                    } else {
                        CitationState::Missing
                    },
                    citation,
                }
            })
            .collect(),
        evidence: vec![EvidenceResult {
            reference: "suite-1".into(),
            decision: "admit".into(),
        }],
        eligible: true,
        reasons: vec![],
    }
}

// Section 3.10: the complete pilot, task to acceptance, with exact bytes at
// every boundary; nothing is applied or published.
#[test]
fn the_pilot_runs_task_to_acceptance_across_a_corrected_generation() {
    let p = pilot();

    // Generation 1: executed, reviewed, judged, not accepted.
    let t1 = p.generation(1, "task-022-a", DRAFT_1, true);
    let before = p.status();
    let probe = p.documentation(&t1);
    let subject = probe.subject.clone().unwrap();
    assert_eq!(subject.artifacts[0].digest, d(DRAFT_1));
    assert_eq!(
        context::read_retained(p.home.path(), &d(PATCH))
            .unwrap()
            .unwrap(),
        PATCH.as_bytes()
    );
    p.submit(&p.review(subject.clone(), OWNER, &t1, false), 1)
        .unwrap();
    // The same review again is refused and counts once.
    let err = p
        .submit(&p.review(subject.clone(), OWNER, &t1, false), 1)
        .unwrap_err();
    assert!(err.starts_with("duplicate: review"), "{err}");
    let first = p.documentation(&t1);
    assert_ne!(first.outcome, Outcome::Accepted);
    let AcceptanceView::Judged { judgment: j1 } = cli::accept(
        p.target.path(),
        p.home.path(),
        RUN,
        1,
        Some(&first),
        &clock(),
    ) else {
        panic!("not judged")
    };
    assert_ne!(j1.outcome, Outcome::Accepted);
    assert_eq!(j1.patches.len(), 1);
    assert_eq!(j1.patches[0].applicability, "not-recorded");

    // A human correction of generation 1 is appended, keeps its bytes, and
    // unbinds the earlier review.
    let correction = Correction {
        schema: review::CORRECTION_SCHEMA.into(),
        subject: subject.clone(),
        supersedes: d(DRAFT_1),
        author: OWNER.into(),
        reason: "precision".into(),
        changed_claims: vec![],
        changed_artifacts: vec![review::ArtifactIdentity {
            logical_path: "docs/recovery.md".into(),
            digest: d(DRAFT_2),
        }],
        corrected_result: None,
        digest_construction: String::new(),
        correction_digest: String::new(),
    }
    .seal();
    let w = cli::submit_correction(
        p.target.path(),
        p.home.path(),
        RUN,
        1,
        &serde_json::to_vec(&correction).unwrap(),
        &[DRAFT_2.as_bytes()],
        &clock(),
    )
    .unwrap();
    assert_eq!(w.state(), Some(WorkflowState::Corrected));
    let corrected = p.documentation(&t1);
    assert!(corrected.bound_reviews.is_empty());
    assert_ne!(corrected.subject, first.subject);

    // Generation 2: a new attempt and task over the same exports, without
    // the insufficient claim, reviewed and accepted.
    let t2 = p.generation(2, "task-022-b", DRAFT_2, false);
    assert_ne!(t2.task_digest, t1.task_digest);
    let subject2 = p.documentation(&t2).subject.unwrap();
    p.submit(&p.review(subject2, OWNER, &t2, true), 2).unwrap();
    let second = p.documentation(&t2);
    assert_eq!(second.outcome, Outcome::Accepted, "{:?}", second.reasons);
    let AcceptanceView::Judged { judgment: j2 } = cli::accept(
        p.target.path(),
        p.home.path(),
        RUN,
        2,
        Some(&second),
        &clock(),
    ) else {
        panic!("not judged")
    };
    assert_eq!(j2.outcome, Outcome::Accepted, "{:?}", j2.reasons);
    assert_ne!(j2.subject_digest, j1.subject_digest);

    // `run show`: both workflows, every version, without recomputing.
    let chain = p.chain();
    let run = statecraft_run::session::runs(&chain)
        .into_iter()
        .find(|r| r.id == RUN)
        .unwrap();
    let views = statecraft_cli::documentation::of_run(&chain, &run);
    let wire = serde_json::to_value(&views).unwrap();
    assert_eq!(wire[1]["fixedExport"]["state"], "recorded");
    assert_eq!(
        wire[1]["fixedExport"]["workflow"]["transitions"][4]["state"],
        "judged"
    );
    assert_eq!(
        wire[1]["fixedExport"]["workflow"]["transitions"][4]["reason"],
        "accepted"
    );
    // The judged version is stamped by the injected clock, not the system's.
    assert_eq!(
        wire[1]["fixedExport"]["workflow"]["transitions"][4]["at"],
        statecraft_environment::time::rfc3339_utc(1_790_000_000)
    );
    match &views[0].fixed_export {
        WorkflowView::Recorded { workflow, versions } => {
            assert_eq!(*versions, 6);
            assert_eq!(workflow.state(), Some(WorkflowState::Corrected));
        }
        other => panic!("{other:?}"),
    }
    let human = statecraft_cli::documentation::describe(&views);
    assert!(human.contains("attempt 2 fixed-export-documentation/1 judged"));

    // Section 3.9: acceptance applies and publishes nothing.
    assert_eq!(p.status(), before);
    assert!(!p.target.path().join("docs/recovery.md").exists());
}

// Section 3.12 row 1 and section 3.2: a moving input refuses before any
// intent; the same inputs report the existing record and begin nothing.
#[test]
fn preparation_refuses_before_the_intent_and_repeats_as_the_same_identity() {
    let p = pilot();
    let moving = manifest(1, "task-022-a", "main");
    let t = task(&moving, "task-022-a", &p.base);
    let err = p.prepare(&moving, &t).unwrap_err();
    assert!(err.starts_with("moving-input"), "{err}");
    assert!(p.chain().entries().is_empty());

    let m = manifest(1, "task-022-a", &"a".repeat(40));
    let t = task(&m, "task-022-a", &p.base);
    let first = p.prepare(&m, &t).unwrap();
    assert!(first.session.is_some());
    let n = p.chain().entries().len();
    let again = p.prepare(&m, &t).unwrap();
    assert!(again.session.is_none());
    assert_eq!(again.workflow, first.workflow);
    assert_eq!(p.chain().entries().len(), n);
}

// Section 3.12 rows 2 and 12: a missing capability refuses before anything
// runs, and the attempt is concluded `refused` with no event recorded.
#[test]
fn a_missing_capability_refuses_and_concludes_the_attempt() {
    let p = pilot();
    let m = manifest(1, "task-022-a", &"a".repeat(40));
    let t = task(&m, "task-022-a", &p.base);
    let session = p.prepare(&m, &t).unwrap().session.unwrap();
    let mut lacking = fx::fake_declaration();
    lacking
        .profile
        .retain(|c| *c != ProfileCapability::DeterministicFake);
    let err = cli::execute(
        p.target.path(),
        p.home.path(),
        &session,
        &[lacking],
        &script(&t, DRAFT_1, true),
        &clock(),
    )
    .unwrap_err();
    assert_eq!(err, "capability-missing: deterministic-fake");
    // The attempt is concluded once; executing it again writes nothing.
    let n = p.chain().entries().len();
    let again = cli::execute(
        p.target.path(),
        p.home.path(),
        &session,
        &[fx::fake_declaration()],
        &script(&t, DRAFT_1, true),
        &clock(),
    );
    assert_eq!(
        again.unwrap_err(),
        format!("workflow: run {RUN} attempt 1 is already concluded")
    );
    assert_eq!(p.chain().entries().len(), n);
    assert_eq!(
        p.attempt(1).outcome,
        Some(statecraft_run::attempt::Outcome::Refused)
    );
    match cli::view(&p.chain(), RUN, 1) {
        WorkflowView::Recorded { workflow, .. } => {
            assert_eq!(workflow.state(), Some(WorkflowState::Prepared));
            assert!(workflow.subjects.events.is_empty());
        }
        other => panic!("{other:?}"),
    }
}

// Section 3.7 and section 3.12 row 8: the fake provider and an ungranted
// identity cannot review; nothing is appended.
#[test]
fn unauthorized_reviewers_are_refused_and_append_nothing() {
    let p = pilot();
    let t = p.generation(1, "task-022-a", DRAFT_1, true);
    let subject = p.documentation(&t).subject.unwrap();
    let n = p.chain().entries().len();
    let (_, provider) = fx::fake_parties();
    for who in [
        provider.name.as_str(),
        fx::FAKE_NAME,
        "stranger@example.invalid",
    ] {
        let err = p
            .submit(&p.review(subject.clone(), who, &t, true), 1)
            .unwrap_err();
        assert!(err.starts_with("authority"), "{who}: {err}");
    }
    assert_eq!(p.chain().entries().len(), n);
}

// Section 3.12 rows 11 and 10: an attempted publication is refused, kept,
// and rejects the workflow; the command's own `accept` path, with no
// documentation manifest, never accepts.
#[test]
fn an_attempted_publication_rejects_and_the_command_path_never_accepts_by_absence() {
    let p = pilot();
    let m = manifest(1, "task-022-a", &"a".repeat(40));
    let t = task(&m, "task-022-a", &p.base);
    let session = p.prepare(&m, &t).unwrap().session.unwrap();
    let before = p.status();
    let mut steps = script(&t, DRAFT_2, false);
    steps.insert(1, Step::Attempt(Effect::Publication("pull-request".into())));
    let w = cli::execute(
        p.target.path(),
        p.home.path(),
        &session,
        &[fx::fake_declaration()],
        &steps,
        &clock(),
    )
    .unwrap();
    assert!(
        w.findings
            .contains(&"prohibited-effect: publication pull-request".to_string())
    );
    let subject = p.documentation(&t).subject.unwrap();
    p.submit(&p.review(subject, OWNER, &t, true), 1).unwrap();
    let documentation = p.documentation(&t);
    assert_eq!(documentation.outcome, Outcome::Accepted);
    let AcceptanceView::Judged { judgment } = cli::accept(
        p.target.path(),
        p.home.path(),
        RUN,
        1,
        Some(&documentation),
        &clock(),
    ) else {
        panic!("not judged")
    };
    assert_eq!(judgment.outcome, Outcome::Rejected);

    // The command's path judges with no documentation manifest supplied, so
    // context freshness is `not-recorded` and nothing is accepted.
    let chain = p.chain();
    let a = p.attempt(1);
    let view = statecraft_cli::documentation::accept(
        p.target.path(),
        p.home.path(),
        &chain,
        &a,
        RUN,
        &Default::default(),
    );
    let statecraft_cli::documentation::AcceptanceView::Judged { judgment: live } = view else {
        panic!("{view:?}")
    };
    assert_ne!(live.outcome, Outcome::Accepted);
    let AcceptanceView::Judged { judgment } = cli::accept(
        p.target.path(),
        p.home.path(),
        RUN,
        1,
        Some(&live),
        &clock(),
    ) else {
        panic!("not judged")
    };
    assert_ne!(judgment.outcome, Outcome::Accepted);
    assert_eq!(p.status(), before);
}

// Section 3.11: an attempt outside the profile renders `not-recorded` and is
// never synthesized into one.
#[test]
fn an_attempt_without_a_workflow_renders_not_recorded() {
    let p = pilot();
    {
        let mut chain = p.chain();
        statecraft_run::session::begin_with(
            &mut chain,
            p.target.path(),
            RUN,
            "main",
            &clock(),
            &Default::default(),
        )
        .unwrap();
    }
    assert_eq!(cli::view(&p.chain(), RUN, 1), WorkflowView::NotRecorded);
    assert_eq!(
        cli::accept(p.target.path(), p.home.path(), RUN, 1, None, &clock()),
        AcceptanceView::NotRecorded
    );
    assert_eq!(cli::describe(1, &WorkflowView::NotRecorded), "");
}

// An unreadable workflow record binds nothing: preparation refuses rather
// than substitute an empty candidate, and nothing is appended.
#[test]
fn an_unreadable_workflow_record_refuses_preparation() {
    let p = pilot();
    let m = manifest(1, "task-022-a", &"a".repeat(40));
    let t = task(&m, "task-022-a", &p.base);
    p.prepare(&m, &t).unwrap();
    {
        let mut chain = p.chain();
        let n = chain.entries().len();
        chain
            .append(
                &format!("{RUN}/1/fixed-export-workflow/{n}"),
                NOW,
                &statecraft_run::record::Entry {
                    kind: statecraft_run::record::Kind::Evidence,
                    run_id: RUN.into(),
                    attempt: 1,
                    subject: "fixed-export-workflow".into(),
                    idempotency_key: None,
                    detail: json!({ "workflow": { "schema": "garbled" } }),
                    effect_id: statecraft_run::record::Identity::Absent,
                },
            )
            .unwrap();
    }
    let n = p.chain().entries().len();
    let err = p.prepare(&m, &t).unwrap_err();
    assert!(err.starts_with("workflow-unreadable"), "{err}");
    assert_eq!(p.chain().entries().len(), n);
    // Nor can it be judged: nothing is recorded, and the view says so.
    let documentation = p.documentation_unjudged(&t);
    match cli::accept(
        p.target.path(),
        p.home.path(),
        RUN,
        1,
        Some(&documentation),
        &clock(),
    ) {
        AcceptanceView::Unrecorded { reason } => {
            assert!(reason.starts_with("workflow-unreadable"), "{reason}")
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(p.chain().entries().len(), n);
}

// Section 3.7: a review before any result is readable is refused, so the
// provider and adapter exclusion is never skipped by an absence.
#[test]
fn a_review_without_a_readable_result_fails_closed() {
    let p = pilot();
    let m = manifest(1, "task-022-a", &"a".repeat(40));
    let t = task(&m, "task-022-a", &p.base);
    p.prepare(&m, &t).unwrap();
    let subject = CoreTuple {
        repository: p.target.path().display().to_string(),
        trusted_base: p.base.clone(),
        run_id: RUN.into(),
        attempt: 1,
        candidate: p.base.clone(),
        context_manifest_digest: m.manifest_digest.clone(),
        task_digest: t.task_digest.clone(),
        result_custody_digest: d("raw"),
        result_schema_digest: t.result_schema.digest.clone(),
        artifacts: vec![],
        evidence: vec![],
        policy_digest: d("policy"),
    };
    let n = p.chain().entries().len();
    let err = p
        .submit(&p.review(subject, OWNER, &t, true), 1)
        .unwrap_err();
    assert!(
        err.starts_with("authority: the attempt has no single recorded result"),
        "{err}"
    );
    assert_eq!(p.chain().entries().len(), n);
}

// Section 3.7: a grant whose `notAfter` is not the `Z` form grants nothing,
// whatever instant it spells.
#[test]
fn a_grant_with_an_offset_expiry_grants_nothing() {
    let p = pilot();
    std::fs::write(
        p.target.path().join(da::POLICY_PATH),
        serde_json::to_vec(&json!({
            "schema": da::POLICY_SCHEMA,
            "reviewers": [{"identity": OWNER, "authority": "repository-owner",
                           "notAfter": "2999-01-01T00:00:00+00:00"}],
            "quorum": 1,
            "observedTransport": "optional",
            "maxArtifactBytes": 65536
        }))
        .unwrap(),
    )
    .unwrap();
    git(
        p.target.path(),
        &["commit", "--quiet", "-am", "offset expiry"],
    );
    let p = Pilot {
        base: git(p.target.path(), &["rev-parse", "HEAD"]),
        ..p
    };
    let t = p.generation(1, "task-022-a", DRAFT_1, true);
    let subject = p.documentation(&t).subject.unwrap();
    let err = p
        .submit(&p.review(subject, OWNER, &t, true), 1)
        .unwrap_err();
    assert!(
        err.starts_with("authority: the trusted-base policy grants"),
        "{err}"
    );
}

// Section 3.8: a correction binds only the attempt's own task.
#[test]
fn a_correction_naming_another_task_is_refused() {
    let p = pilot();
    let t = p.generation(1, "task-022-a", DRAFT_1, true);
    let mut subject = p.documentation(&t).subject.unwrap();
    subject.task_digest = d("another task");
    let correction = Correction {
        schema: review::CORRECTION_SCHEMA.into(),
        subject,
        supersedes: d(DRAFT_1),
        author: OWNER.into(),
        reason: "precision".into(),
        changed_claims: vec![],
        changed_artifacts: vec![review::ArtifactIdentity {
            logical_path: "docs/recovery.md".into(),
            digest: d(DRAFT_2),
        }],
        corrected_result: None,
        digest_construction: String::new(),
        correction_digest: String::new(),
    }
    .seal();
    let n = p.chain().entries().len();
    let err = cli::submit_correction(
        p.target.path(),
        p.home.path(),
        RUN,
        1,
        &serde_json::to_vec(&correction).unwrap(),
        &[DRAFT_2.as_bytes()],
        &clock(),
    )
    .unwrap_err();
    assert_eq!(err, "binding: the correction names another task");
    assert_eq!(p.chain().entries().len(), n);
}
