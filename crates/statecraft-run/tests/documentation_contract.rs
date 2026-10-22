//! Spec 020's run-record half: the closed documentation task and the intent
//! that records it before any effect. Every fact here is a
//! portable fixture: no adapter, no provider, and every chain read is
//! reopened from disk.

use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};

use statecraft_environment::time::FixedClock;
use statecraft_run::context::{
    self, AcceptedPage, AcceptedRepository, Completeness, DeliveryPolicy, Intended, Manifest,
    OptionalOmission, PageFacts, Producer, ProducerBuild, Requirement, Snapshot, Task,
};
use statecraft_run::documentation::{
    self, Artifact, Candidate, CandidateDivergence, ClockSource, CostCeiling, Deadline,
    Disposition, DocumentError, DocumentationTask, Format, NotRequired, Operations, OutputFormat,
    Publication, RequiredContext, ResultSchemaRef, ReviewPolicy, SemanticReview, Skill,
};
use statecraft_run::record::Chain;
use statecraft_run::session::{self, IntentDetail, SessionError};

const RUN: &str = "run-020";
const TASK: &str = "task-020";

fn digest(tag: &str) -> String {
    context::custody_digest(tag.as_bytes())
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

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "--quiet", "--initial-branch=main"]);
    git(
        dir.path(),
        &["config", "user.email", "test@example.invalid"],
    );
    git(dir.path(), &["config", "user.name", "test"]);
    git(dir.path(), &["config", "commit.gpgsign", "false"]);
    std::fs::write(dir.path().join("README.md"), b"x").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "--quiet", "-m", "one"]);
    dir
}

fn manifest(attempt: u32, task: &str) -> Manifest {
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
        request_digest: digest("request"),
        closure_digest: digest("closure"),
        packet_digest: digest("page-1"),
        completeness: Completeness::Complete,
        continues: false,
        omissions: vec![],
        warnings: vec![],
    };
    let page = AcceptedPage {
        bytes: b"{\"page\":1}".to_vec(),
        source_operation: "context_packet_pages".into(),
        facts,
        delivery: Intended::Required,
    };
    Manifest::build(
        RUN,
        attempt,
        Task::Id(task.into()),
        &Producer {
            package: "spec-spine".into(),
            version: "0.31.0".into(),
            packet_interface: "1.1.0".into(),
        },
        &[AcceptedRepository {
            id: "app".into(),
            role: "source".into(),
            requirement: Requirement::Required,
            pages: vec![page],
        }],
        DeliveryPolicy::new(OptionalOmission::Refuse),
    )
    .unwrap()
}

fn task(attempt: u32, revision: &str, manifest: &Manifest) -> DocumentationTask {
    DocumentationTask {
        schema: documentation::TASK_SCHEMA.into(),
        task_id: TASK.into(),
        run_id: RUN.into(),
        attempt,
        candidate: Candidate {
            repository: "app".into(),
            revision: revision.into(),
            tree: "tree".into(),
        },
        context_manifest_digest: manifest.manifest_digest.clone(),
        objective: "Describe the parser's error recovery for operators.".into(),
        audiences: vec!["operators".into()],
        output_format: OutputFormat {
            format: Format::Markdown,
            version: "commonmark-0.31.2".into(),
        },
        operations: Operations::v1(),
        required_capabilities: vec!["structured-result".into()],
        skill: Skill::NotRequired(NotRequired::NotRequired),
        result_schema: ResultSchemaRef {
            id: documentation::RESULT_SCHEMA.into(),
            digest: documentation::result_schema_digest(),
        },
        deadline: Deadline {
            not_after: "2026-12-31T00:00:00Z".into(),
            clock: ClockSource::SupervisorUtc,
        },
        cost_ceiling: CostCeiling::NoSpend(documentation::NoSpend::NoSpend),
        artifacts: vec![Artifact {
            path: "docs/parser.md".into(),
            disposition: Disposition::Draft,
            required_context: vec![RequiredContext {
                repository: "app".into(),
                pages: vec![digest("page-1")],
            }],
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

fn bytes(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap()
}

fn refuse_task(mutate: impl FnOnce(&mut Value)) -> DocumentError {
    let m = manifest(1, TASK);
    let mut v = serde_json::to_value(task(1, "r", &m)).unwrap();
    mutate(&mut v);
    DocumentationTask::decode(&bytes(&v)).unwrap_err()
}

fn begin(
    chain: &mut Chain,
    target: &Path,
    manifest: Option<&Manifest>,
    task: Option<&DocumentationTask>,
) -> Result<u32, SessionError> {
    session::begin_with(
        chain,
        target,
        RUN,
        "main",
        &FixedClock(1_790_000_000),
        &IntentDetail {
            context_manifest: manifest,
            documentation_task: task,
            ..IntentDetail::default()
        },
    )
    .map(|s| s.attempt)
}

// Section 3.2: a sealed task round-trips through its exact bytes.
#[test]
fn a_sealed_task_decodes_from_its_own_bytes() {
    let m = manifest(1, TASK);
    let t = task(1, "r", &m);
    assert_eq!(DocumentationTask::decode(&t.to_bytes()).unwrap(), t);
    assert_eq!(t.computed_digest(), t.task_digest);
    t.check_manifest(&m).unwrap();
}

// Section 3.2: unknown members refuse decoding; nothing is dropped.
#[test]
fn the_task_schema_is_closed() {
    let e = refuse_task(|v| v["grantedOperations"] = json!(["push"]));
    assert!(matches!(e, DocumentError::Malformed(_)), "{e}");
    let e = refuse_task(|v| {
        v.as_object_mut().unwrap().remove("deadline");
    });
    assert!(matches!(e, DocumentError::Malformed(_)), "{e}");
}

// Section 3.3, invariant I-2: both sets are exactly version 1's.
#[test]
fn an_extra_or_missing_operation_token_is_malformed() {
    let e = refuse_task(|v| {
        v["operations"]["allowed"]
            .as_array_mut()
            .unwrap()
            .push(json!("push"))
    });
    assert!(matches!(e, DocumentError::Operations(_)), "{e}");
    let e = refuse_task(|v| {
        v["operations"]["forbidden"]
            .as_array_mut()
            .unwrap()
            .retain(|t| t != "commit")
    });
    assert!(matches!(e, DocumentError::Operations(_)), "{e}");
    let e = refuse_task(|v| {
        v["operations"]["allowed"]
            .as_array_mut()
            .unwrap()
            .push(json!("read-context"))
    });
    assert!(matches!(e, DocumentError::Operations(_)), "{e}");
    let e = refuse_task(|v| {
        v["operations"]["allowed"]
            .as_array_mut()
            .unwrap()
            .push(json!("grant-all"))
    });
    assert!(matches!(e, DocumentError::Malformed(_)), "{e}");
}

// Section 3.10: raw task bytes omitting the publish prohibition, or giving
// another publication value, refuse before spawn.
#[test]
fn publication_is_prohibited_twice_and_both_are_checked() {
    let e = refuse_task(|v| {
        v["operations"]["forbidden"]
            .as_array_mut()
            .unwrap()
            .retain(|t| t != "publish")
    });
    assert!(matches!(e, DocumentError::Operations(_)), "{e}");
    let e = refuse_task(|v| v["publication"] = json!("permitted"));
    assert!(matches!(e, DocumentError::Malformed(_)), "{e}");
}

// Section 3.2: packet content cannot replace the task's result schema.
#[test]
fn the_result_schema_is_the_delivered_one() {
    let e = refuse_task(|v| v["resultSchema"]["digest"] = json!(digest("another schema")));
    assert!(matches!(e, DocumentError::ResultSchema(_)), "{e}");
    let e = refuse_task(|v| v["resultSchema"]["id"] = json!("vendor/result/9"));
    assert!(matches!(e, DocumentError::ResultSchema(_)), "{e}");
}

// Section 3.2: the digest covers every other member.
#[test]
fn a_changed_member_breaks_the_task_digest() {
    let e = refuse_task(|v| v["objective"] = json!("Something else."));
    assert!(matches!(e, DocumentError::Digest { .. }), "{e}");
}

// Section 3.2: bounds, explicit, before and after decoding.
#[test]
fn bounds_and_paths_refuse() {
    let e = refuse_task(|v| v["objective"] = json!("x".repeat(documentation::TEXT_BYTES + 1)));
    assert!(matches!(e, DocumentError::MemberBounds(_)), "{e}");
    let huge = vec![b' '; documentation::TASK_BOUNDS.total_bytes + 1];
    assert!(matches!(
        DocumentationTask::decode(&huge),
        Err(DocumentError::Bounds(_))
    ));
    let m = manifest(1, TASK);
    for escape in ["../outside.md", "/abs.md", "docs/./x.md", "a\\b.md"] {
        let mut t = task(1, "r", &m);
        t.artifacts[0].path = escape.into();
        let t = t.seal();
        assert!(
            matches!(t.validate(), Err(DocumentError::Invalid(_))),
            "{escape}"
        );
    }
}

// Section 3.3: the task may narrow the manifest, never add to it.
#[test]
fn a_task_cannot_require_a_page_the_manifest_does_not_hold() {
    let m = manifest(1, TASK);
    let mut t = task(1, "r", &m);
    t.artifacts[0].required_context[0].pages = vec![digest("page-9")];
    let t = t.seal();
    assert!(matches!(
        t.check_manifest(&m),
        Err(DocumentError::OutsideTask(_))
    ));
    let other = manifest(1, "another-task");
    let t = task(1, "r", &other);
    assert!(matches!(
        t.check_manifest(&other),
        Err(DocumentError::OutsideTask(_))
    ));
}

// Section 3.10 row 8: a silent review policy prohibits divergence.
#[test]
fn candidate_divergence_needs_an_explicit_permission() {
    let m = manifest(1, TASK);
    let t = task(1, "task-rev", &m);
    match t.check_candidate("workspace-rev") {
        Err(DocumentError::CandidateDivergence { task, workspace }) => {
            assert_eq!(
                (task.as_str(), workspace.as_str()),
                ("task-rev", "workspace-rev")
            );
        }
        other => panic!("expected divergence, got {other:?}"),
    }
    let mut t = t;
    t.review_policy.candidate_divergence = Some(CandidateDivergence::Permitted);
    t.seal().check_candidate("workspace-rev").unwrap();
}

// Requirement R-1: the task is in the intent before any effect.
#[test]
fn the_intent_records_the_task_before_any_effect() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let head = git(target.path(), &["rev-parse", "HEAD"]);
    let m = manifest(1, TASK);
    let t = task(1, &head, &m);
    {
        let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
        assert_eq!(
            begin(&mut chain, target.path(), Some(&m), Some(&t)).unwrap(),
            1
        );
    }
    let (chain, _) = Chain::open(home.path(), target.path()).unwrap();
    assert_eq!(
        documentation::recorded_task(&chain, RUN, 1).unwrap(),
        Some(t.clone())
    );
}

#[test]
fn a_task_without_its_manifest_or_for_another_attempt_refuses_before_any_record() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let head = git(target.path(), &["rev-parse", "HEAD"]);
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    let m = manifest(1, TASK);
    let t = task(1, &head, &m);
    let refused = begin(&mut chain, target.path(), None, Some(&t)).unwrap_err();
    assert!(
        refused.to_string().contains("documentation-task"),
        "{refused}"
    );
    let m2 = manifest(2, TASK);
    let t2 = task(2, &head, &m2);
    let refused = begin(&mut chain, target.path(), Some(&m), Some(&t2)).unwrap_err();
    assert!(
        refused.to_string().contains("documentation-task"),
        "{refused}"
    );
    let diverged = task(1, "another-revision", &m);
    let refused = begin(&mut chain, target.path(), Some(&m), Some(&diverged)).unwrap_err();
    assert!(
        refused.to_string().contains("candidate-divergence"),
        "{refused}"
    );
    assert!(chain.entries().is_empty(), "nothing was recorded");
}

#[test]
fn a_historical_intent_renders_not_recorded() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    begin(&mut chain, target.path(), None, None).unwrap();
    assert_eq!(documentation::recorded_task(&chain, RUN, 1).unwrap(), None);
}

// Section 3.8: a retry with equal semantic fields still has its own task.
#[test]
fn a_retry_writes_a_new_task_identity() {
    let m1 = manifest(1, TASK);
    let m2 = manifest(2, TASK);
    let first = task(1, "r", &m1);
    let second = task(2, "r", &m2);
    assert_ne!(first.task_digest, second.task_digest);
    assert_eq!(first.objective, second.objective);
}
