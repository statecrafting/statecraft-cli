//! Spec 021 through the command surface's values: `accept` performs and
//! records the documentation judgment only when the attempt has a task,
//! reading the trusted-base policy at the attempt's base commit; `run show`
//! lists review and correction identities without judging them (section
//! 3.10). Nothing is applied or published, and nothing absent reads as a
//! pass.

use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};

use statecraft_acceptance::documentation_acceptance::{Outcome, State};
use statecraft_cli::documentation::{ACCEPTANCE_SUBJECT, AcceptanceView};
use statecraft_environment::time::FixedClock;
use statecraft_run::context::{
    self, AcceptedPage, AcceptedRepository, Completeness, DeliveryPolicy, Intended, Manifest,
    OptionalOmission, PageFacts, Producer, ProducerBuild, Requirement, Snapshot, Task,
};
use statecraft_run::documentation::delivery::{self, Reported};
use statecraft_run::documentation::review::{
    self, ArtifactIdentity, Authority, ClaimDisposition, ClaimReview, CoreTuple, Recommendation,
    Review, Reviewer,
};
use statecraft_run::documentation::{
    self as doc, Artifact, Binding, Candidate, ClockSource, CostCeiling, Deadline, Disposition,
    DocumentationTask, Format, NoSpend, NotRequired, Operations, OutputFormat, Party, Publication,
    ResultSchemaRef, ReviewPolicy, SemanticReview, Skill,
};
use statecraft_run::record::Chain;
use statecraft_run::session::{self, IntentDetail};

const RUN: &str = "run-020";

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

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
    std::fs::write(dir.path().join("README.md"), b"x").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "--quiet", "-m", "one"]);
    dir
}

fn manifest() -> Manifest {
    let d = |t: &str| context::custody_digest(t.as_bytes());
    let facts = PageFacts {
        schema_version: "1.1.0".into(),
        producer: ProducerBuild {
            package: "spec-spine".into(),
            version: "0.31.0".into(),
            build: "unverified".into(),
        },
        snapshot: Snapshot {
            repository: "example/app".into(),
            revision: "r1".into(),
            tree: "tree-r1".into(),
            dirty_state: "clean-export".into(),
        },
        request_digest: d("request"),
        closure_digest: d("closure"),
        packet_digest: d("page-1"),
        completeness: Completeness::Complete,
        continues: false,
        omissions: vec![],
        warnings: vec![],
    };
    Manifest::build(
        RUN,
        1,
        Task::Id("task-020".into()),
        &Producer {
            package: "spec-spine".into(),
            version: "0.31.0".into(),
            packet_interface: "1.1.0".into(),
        },
        &[AcceptedRepository {
            id: "app".into(),
            role: "source".into(),
            requirement: Requirement::Required,
            pages: vec![AcceptedPage {
                bytes: b"{}".to_vec(),
                source_operation: "context_packet_pages".into(),
                facts,
                delivery: Intended::Required,
            }],
        }],
        DeliveryPolicy::new(OptionalOmission::Refuse),
    )
    .unwrap()
}

fn task(revision: &str, m: &Manifest) -> DocumentationTask {
    DocumentationTask {
        schema: doc::TASK_SCHEMA.into(),
        task_id: "task-020".into(),
        run_id: RUN.into(),
        attempt: 1,
        candidate: Candidate {
            repository: "app".into(),
            revision: revision.into(),
            tree: "tree".into(),
        },
        context_manifest_digest: m.manifest_digest.clone(),
        objective: "Describe the parser.".into(),
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
            disposition: Disposition::Patch,
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

fn party(name: &str) -> Party {
    Party {
        name: name.into(),
        version: "1.0.0".into(),
    }
}

fn result(t: &DocumentationTask) -> Value {
    json!({
        "schema": doc::RESULT_SCHEMA,
        "taskId": t.task_id, "runId": t.run_id, "attempt": t.attempt,
        "taskDigest": t.task_digest, "contextManifestDigest": t.context_manifest_digest,
        "adapter": {"name": "fixture-adapter", "version": "1.0.0"},
        "provider": {"name": "fixture-provider", "version": "1.0.0"},
        "artifacts": [{
            "logicalPath": "docs/parser.md", "mediaType": "text/x-diff",
            "digest": context::custody_digest(b"patch"), "size": 5, "rawRef": "artifact-0"
        }],
        "claims": [], "unknowns": [], "contradictions": [], "unsupportedClaims": [],
        "providerOmissions": [], "statecraftCitationFailures": [], "executionEvidence": [],
        "providerCompletion": "claimed-complete",
        "reviewDisposition": "not-reviewed",
        "acceptanceResult": "not-attempted"
    })
}

struct Fixture {
    home: tempfile::TempDir,
    target: tempfile::TempDir,
    task: DocumentationTask,
}

fn begun() -> Fixture {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let m = manifest();
    let t = task(&git(target.path(), &["rev-parse", "HEAD"]), &m);
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    session::begin_with(
        &mut chain,
        target.path(),
        RUN,
        "main",
        &FixedClock(1_790_000_000),
        &IntentDetail {
            context_manifest: Some(&m),
            documentation_task: Some(&t),
            ..IntentDetail::default()
        },
    )
    .unwrap();
    Fixture {
        home,
        target,
        task: t,
    }
}

impl Fixture {
    fn chain(&self) -> Chain {
        Chain::open(self.home.path(), self.target.path()).unwrap().0
    }

    fn deliver(&self, reported: &[Reported]) {
        let mut chain = self.chain();
        let fold = delivery::fold(&self.task, reported);
        delivery::record_delivery(&mut chain, &self.task, &fold, "2026-10-22T00:00:00Z").unwrap();
    }

    fn report(&self) -> Reported {
        Reported {
            task_digest: self.task.task_digest.clone(),
            context_manifest_digest: self.task.context_manifest_digest.clone(),
            skill: delivery::SKILL_NOT_REQUIRED.into(),
            result_schema_digest: self.task.result_schema.digest.clone(),
            capabilities: vec![],
        }
    }

    fn return_bytes(&self, bytes: &[u8]) {
        let mut chain = self.chain();
        let binding = Binding::of(
            &self.task,
            party("fixture-adapter"),
            party("fixture-provider"),
        );
        doc::record_result(
            self.home.path(),
            &mut chain,
            &self.task,
            &binding,
            "application/json",
            bytes,
            "2026-10-22T00:00:01Z",
        )
        .unwrap();
    }
}

fn attempt(f: &Fixture) -> statecraft_run::Attempt {
    let chain = f.chain();
    session::runs(&chain)
        .into_iter()
        .find(|r| r.id == RUN)
        .unwrap()
        .attempts
        .last()
        .cloned()
        .unwrap()
}

fn commit_policy(f: &Fixture) {
    let dir = f.target.path().join(".statecraft");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("documentation-acceptance.json"),
        serde_json::to_vec(&json!({
            "schema": "statecraft/documentation-acceptance-policy/1",
            "reviewers": [{"identity": "owner@example.invalid", "authority": "repository-owner"}],
            "quorum": 1,
            "observedTransport": "optional",
            "maxArtifactBytes": 65536
        }))
        .unwrap(),
    )
    .unwrap();
    git(f.target.path(), &["add", "."]);
    git(f.target.path(), &["commit", "--quiet", "-m", "policy"]);
}

fn begun_with_policy() -> Fixture {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let f = Fixture {
        home,
        target,
        task: task("unused", &manifest()),
    };
    commit_policy(&f);
    let m = manifest();
    let t = task(&git(f.target.path(), &["rev-parse", "HEAD"]), &m);
    let (mut chain, _) = Chain::open(f.home.path(), f.target.path()).unwrap();
    session::begin_with(
        &mut chain,
        f.target.path(),
        RUN,
        "main",
        &FixedClock(1_790_000_000),
        &IntentDetail {
            context_manifest: Some(&m),
            documentation_task: Some(&t),
            ..IntentDetail::default()
        },
    )
    .unwrap();
    Fixture { task: t, ..f }
}

fn judged(view: AcceptanceView) -> Box<statecraft_acceptance::documentation_acceptance::Judgment> {
    match view {
        AcceptanceView::Judged { judgment } => judgment,
        other => panic!("expected a judgment, got {other:?}"),
    }
}

#[test]
fn accept_judges_and_records_a_documentation_attempt_without_passing_absences() {
    let f = begun_with_policy();
    f.deliver(&[f.report()]);
    f.return_bytes(result(&f.task).to_string().as_bytes());
    let a = attempt(&f);
    let j = judged(statecraft_cli::documentation::accept(
        f.target.path(),
        f.home.path(),
        &f.chain(),
        &a,
        RUN,
        &Default::default(),
    ));
    assert_eq!(j.dimensions.len(), 12);
    assert_eq!(j.outcome, Outcome::NoAcceptance, "{:?}", j.reasons);
    let state = |name: &str| {
        j.dimensions
            .iter()
            .find(|d| serde_json::to_value(d.dimension).unwrap() == name)
            .unwrap()
            .state
    };
    // The policy was read at the trusted base; no review exists yet.
    assert_eq!(state("schemaValidation"), State::Pass);
    assert_eq!(state("semanticReview"), State::NotRecorded);
    // No documentation manifest is supplied: never fresh by absence.
    assert_eq!(state("contextFreshness"), State::NotRecorded);
    let recorded = f
        .chain()
        .entries()
        .into_iter()
        .filter(|e| e.subject == ACCEPTANCE_SUBJECT)
        .count();
    assert_eq!(recorded, 1);
    assert!(!f.target.path().join("docs/parser.md").exists());
}

#[test]
fn accept_without_a_policy_at_the_base_names_its_absence() {
    let f = begun();
    f.deliver(&[f.report()]);
    f.return_bytes(result(&f.task).to_string().as_bytes());
    let j = judged(statecraft_cli::documentation::accept(
        f.target.path(),
        f.home.path(),
        &f.chain(),
        &attempt(&f),
        RUN,
        &Default::default(),
    ));
    assert!(j.reasons.iter().any(|r| r.starts_with("policy-unreadable")));
    assert_eq!(j.subject, None);
    assert_eq!(j.outcome, Outcome::NoAcceptance);
}

#[test]
fn accept_on_an_attempt_without_a_task_records_nothing() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    {
        let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
        session::begin_with(
            &mut chain,
            target.path(),
            RUN,
            "main",
            &FixedClock(1_790_000_000),
            &IntentDetail::default(),
        )
        .unwrap();
    }
    let f = Fixture {
        home,
        target,
        task: task("unused", &manifest()),
    };
    let before = f.chain().entries().len();
    assert_eq!(
        statecraft_cli::documentation::accept(
            f.target.path(),
            f.home.path(),
            &f.chain(),
            &attempt(&f),
            RUN,
            &Default::default(),
        ),
        AcceptanceView::NotRecorded
    );
    assert_eq!(f.chain().entries().len(), before);
}

#[test]
fn run_show_lists_reviews_without_judging_them() {
    let f = begun_with_policy();
    let r = Review {
        schema: review::REVIEW_SCHEMA.into(),
        subject: CoreTuple {
            repository: "example/app".into(),
            trusted_base: "b".repeat(40),
            run_id: RUN.into(),
            attempt: 1,
            candidate: "c".repeat(40),
            context_manifest_digest: f.task.context_manifest_digest.clone(),
            task_digest: f.task.task_digest.clone(),
            result_custody_digest: context::custody_digest(b"raw"),
            result_schema_digest: f.task.result_schema.digest.clone(),
            artifacts: vec![ArtifactIdentity {
                logical_path: "docs/parser.md".into(),
                digest: context::custody_digest(b"patch"),
            }],
            evidence: vec![],
            policy_digest: context::custody_digest(b"policy"),
        },
        reviewer: Reviewer {
            identity: "fixture-provider".into(),
            authority: Authority::RepositoryOwner,
            authority_source: "self-declared".into(),
        },
        reviewed_at: "2026-10-22T00:00:00Z".into(),
        claims: vec![ClaimReview {
            claim_id: "c1".into(),
            disposition: ClaimDisposition::Supported,
            reasons: vec![],
        }],
        artifacts: vec![],
        corrections: vec![],
        recommendation: Recommendation::Accept,
        digest_construction: String::new(),
        review_digest: String::new(),
    }
    .seal();
    {
        let mut chain = f.chain();
        review::record_review(&mut chain, &r, "2026-10-22T00:00:02Z").unwrap();
    }
    let chain = f.chain();
    let run = session::runs(&chain)
        .into_iter()
        .find(|x| x.id == RUN)
        .unwrap();
    let views = statecraft_cli::documentation::of_run(&chain, &run);
    let wire: Value = serde_json::to_value(&views).unwrap();
    assert_eq!(wire[0]["records"][0]["state"], "review");
    assert_eq!(wire[0]["records"][0]["recommendation"], "accept");
    let human = statecraft_cli::documentation::describe(&views);
    assert!(human.contains(&format!(
        "review {} by fixture-provider, recommends accept",
        r.review_digest
    )));
    // Listing is not judging: a provider-authored review is shown, and only
    // the judgment refuses its authority.
}
