//! Spec 022's run-record half: preparation from fixed exports refuses a
//! moving input, an unretained page, spend and a mismatched skill before any
//! adapter is chosen; repeating it reports the same record; events are kept
//! byte for byte before parsing; transitions append versions. Every chain
//! read is reopened from disk.

use std::path::Path;
use std::process::Command;

use statecraft_environment::time::FixedClock;
use statecraft_run::context::{
    self, AcceptedPage, AcceptedRepository, Completeness, DeliveryPolicy, Intended, Manifest,
    OptionalOmission, PageFacts, Producer, ProducerBuild, Requirement, Snapshot, Task,
};
use statecraft_run::documentation::workflow::{self, PrepareError, WorkflowState};
use statecraft_run::documentation::{
    self as doc, Artifact, Candidate, Ceiling, ClockSource, CostCeiling, Deadline, Disposition,
    DocumentationTask, Enforcement, Format, NoSpend, NotRequired, Operations, OutputFormat,
    Publication, ResultSchemaRef, ReviewPolicy, SemanticReview, Skill, SkillIdentity,
};
use statecraft_run::record::Chain;
use statecraft_run::session::{self, IntentDetail};

const RUN: &str = "run-022";
const PAGE: &[u8] = b"{\"page\":1}";

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

fn manifest(revision: &str) -> Manifest {
    let d = |t: &str| context::custody_digest(t.as_bytes());
    Manifest::build(
        RUN,
        1,
        Task::Id("task-022".into()),
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
                bytes: PAGE.to_vec(),
                source_operation: "context_packet_pages".into(),
                facts: PageFacts {
                    schema_version: "1.1.0".into(),
                    producer: ProducerBuild {
                        package: "spec-spine".into(),
                        version: "0.31.0".into(),
                        build: "unverified".into(),
                    },
                    snapshot: Snapshot {
                        repository: "example/app".into(),
                        revision: revision.into(),
                        tree: "t".repeat(40),
                        dirty_state: "clean-export".into(),
                    },
                    request_digest: d("request"),
                    closure_digest: d("closure"),
                    packet_digest: d("page-1"),
                    completeness: Completeness::Complete,
                    continues: false,
                    omissions: vec![],
                    warnings: vec![],
                },
                delivery: Intended::Required,
            }],
        }],
        DeliveryPolicy::new(OptionalOmission::Refuse),
    )
    .unwrap()
}

fn task(m: &Manifest, candidate: &str) -> DocumentationTask {
    DocumentationTask {
        schema: doc::TASK_SCHEMA.into(),
        task_id: "task-022".into(),
        run_id: RUN.into(),
        attempt: 1,
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
        artifacts: vec![Artifact {
            path: "docs/recovery.md".into(),
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

fn adapter() -> workflow::AdapterRef {
    workflow::AdapterRef {
        name: "fixed-export-fake".into(),
        version: "1".into(),
        capabilities: vec![],
    }
}

struct Fixture {
    home: tempfile::TempDir,
    target: tempfile::TempDir,
    head: String,
}

fn fixture() -> Fixture {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let head = git(target.path(), &["rev-parse", "HEAD"]);
    Fixture { home, target, head }
}

impl Fixture {
    fn chain(&self) -> Chain {
        Chain::open(self.home.path(), self.target.path()).unwrap().0
    }

    fn begin(&self, m: &Manifest, t: &DocumentationTask) {
        let mut chain = self.chain();
        session::begin_with(
            &mut chain,
            self.target.path(),
            RUN,
            "main",
            &FixedClock(1_790_000_000),
            &IntentDetail {
                context_manifest: Some(m),
                documentation_task: Some(t),
                ..IntentDetail::default()
            },
        )
        .unwrap();
    }

    fn prepare(
        &self,
        m: &Manifest,
        t: &DocumentationTask,
        skill: Option<&[u8]>,
    ) -> Result<workflow::Prepared, PrepareError> {
        workflow::prepare(
            self.home.path(),
            &mut self.chain(),
            "example/app",
            &self.head,
            m,
            t,
            skill,
            &adapter(),
            "2026-10-22T00:00:00Z",
        )
    }
}

// Sections 3.1 and 3.2: the record names every subject; repeating it reports
// the same identity and writes nothing.
#[test]
fn preparation_records_every_subject_once_and_repeats_as_the_same_identity() {
    let f = fixture();
    let m = manifest(&"a".repeat(40));
    let t = task(&m, &f.head);
    f.begin(&m, &t);
    context::retain(f.home.path(), PAGE).unwrap();
    let first = f.prepare(&m, &t, None).unwrap();
    assert!(!first.existing);
    let w = &first.workflow;
    assert_eq!(w.profile, workflow::PROFILE);
    assert_eq!(w.subjects.task_digest, t.task_digest);
    assert_eq!(w.subjects.manifest_digest, m.manifest_digest);
    assert_eq!(w.subjects.packets.len(), 1);
    assert_eq!(w.subjects.skill, "not-required");
    assert_eq!(w.subjects.adapter, Some(adapter()));
    assert_eq!(w.state(), Some(WorkflowState::Prepared));
    assert_eq!(w.computed_digest(), w.workflow_digest);

    let again = f.prepare(&m, &t, None).unwrap();
    assert!(again.existing);
    assert_eq!(again.workflow, first.workflow);
    assert_eq!(workflow::versions(&f.chain(), RUN, 1).len(), 1);
}

// Section 3.12 row 1: a moving input refuses before any adapter.
#[test]
fn a_packet_naming_a_branch_refuses() {
    let f = fixture();
    let m = manifest("main");
    let t = task(&m, &f.head);
    context::retain(f.home.path(), PAGE).unwrap();
    assert!(matches!(
        f.prepare(&m, &t, None),
        Err(PrepareError::MovingInput(_))
    ));
    assert!(workflow::versions(&f.chain(), RUN, 1).is_empty());
}

// Section 3.2: no step fetches or completes an export.
#[test]
fn an_unretained_page_refuses_and_nothing_fetches_it() {
    let f = fixture();
    let m = manifest(&"a".repeat(40));
    let t = task(&m, &f.head);
    assert!(matches!(
        f.prepare(&m, &t, None),
        Err(PrepareError::NotRetained(_))
    ));
}

#[test]
fn spend_a_mismatched_skill_or_another_manifest_refuses() {
    let f = fixture();
    let m = manifest(&"a".repeat(40));
    context::retain(f.home.path(), PAGE).unwrap();
    let mut spending = task(&m, &f.head);
    spending.cost_ceiling = CostCeiling::Ceiling(Ceiling {
        currency: "USD".into(),
        maximum_minor_units: 100,
        enforcement: Enforcement::StopAttempt,
    });
    assert!(matches!(
        f.prepare(&m, &spending.seal(), None),
        Err(PrepareError::Spend)
    ));
    let mut skilled = task(&m, &f.head);
    skilled.skill = Skill::Pinned(SkillIdentity {
        id: "docs".into(),
        version: "1".into(),
        digest: context::custody_digest(b"the skill"),
    });
    let skilled = skilled.seal();
    assert!(matches!(
        f.prepare(&m, &skilled, Some(b"another skill")),
        Err(PrepareError::Skill(_))
    ));
    let w = f.prepare(&m, &skilled, Some(b"the skill")).unwrap();
    assert_eq!(
        w.workflow.subjects.skill,
        context::custody_digest(b"the skill")
    );
    let other = manifest(&"b".repeat(40));
    assert!(matches!(
        f.prepare(&other, &task(&m, &f.head), None),
        Err(PrepareError::Binding(_))
    ));
}

#[test]
fn different_inputs_for_a_prepared_attempt_compete_and_refuse() {
    let f = fixture();
    let m = manifest(&"a".repeat(40));
    context::retain(f.home.path(), PAGE).unwrap();
    f.prepare(&m, &task(&m, &f.head), None).unwrap();
    let mut other = task(&m, &f.head);
    other.objective = "Something else.".into();
    assert!(matches!(
        f.prepare(&m, &other.seal(), None),
        Err(PrepareError::Competing { .. })
    ));
}

// Section 3.4: kept before parsing; a second terminal and a late event are
// findings, and the first terminal is the one named.
#[test]
fn events_are_kept_byte_for_byte_and_the_sequence_is_judged() {
    let f = fixture();
    let lines: Vec<Vec<u8>> = vec![
        br#"{"event":"init","applied":[],"adapterVersion":"1","providerVersion":"fake"}"#.to_vec(),
        b"not json at all".to_vec(),
        br#"{"event":"result","classification":"completed","cost":null}"#.to_vec(),
        br#"{"event":"progress","message":"late"}"#.to_vec(),
        br#"{"event":"result","classification":"failed","cost":null}"#.to_vec(),
    ];
    let r = workflow::retain_events(f.home.path(), &lines).unwrap();
    assert_eq!(r.events.len(), 5);
    assert_eq!(r.events[1].media_type, "application/octet-stream");
    for (e, bytes) in r.events.iter().zip(&lines) {
        assert_eq!(
            context::read_retained(f.home.path(), &e.custody_digest)
                .unwrap()
                .as_deref(),
            Some(&bytes[..])
        );
    }
    assert_eq!(r.first_terminal, Some(r.events[2].custody_digest.clone()));
    assert_eq!(
        r.findings,
        vec!["after-terminal: event 3", "second-terminal: event 4"]
    );
    let none = workflow::retain_events(f.home.path(), &lines[..2]).unwrap();
    assert!(none.findings[0].starts_with("no-terminal"));
}

// Section 3.1: each transition is a new version; none is rewritten.
#[test]
fn transitions_append_versions_and_keep_the_earlier_ones() {
    let f = fixture();
    let m = manifest(&"a".repeat(40));
    context::retain(f.home.path(), PAGE).unwrap();
    let w = f.prepare(&m, &task(&m, &f.head), None).unwrap().workflow;
    let next = workflow::advance(
        &mut f.chain(),
        &w,
        WorkflowState::Executed,
        "the fake adapter returned",
        "2026-10-22T00:00:01Z",
        |w| w.subjects.raw_result = Some(context::custody_digest(b"raw")),
    )
    .unwrap();
    let versions = workflow::versions(&f.chain(), RUN, 1);
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0].as_ref().unwrap(), &w);
    assert_eq!(versions[1].as_ref().unwrap(), &next);
    assert_eq!(next.state(), Some(WorkflowState::Executed));
    assert_ne!(next.workflow_digest, w.workflow_digest);
}

// Section 3.3: the run policy's adapter is recorded at preparation; another
// one for the same attempt competes, and computing the subjects alone writes
// nothing.
#[test]
fn the_policy_adapter_is_part_of_the_prepared_inputs() {
    let f = fixture();
    let m = manifest(&"a".repeat(40));
    let t = task(&m, &f.head);
    f.begin(&m, &t);
    context::retain(f.home.path(), PAGE).unwrap();
    let before = f.chain().entries().len();
    let s = workflow::subjects(f.home.path(), &m, &t, None, &adapter()).unwrap();
    assert_eq!(s.adapter, Some(adapter()));
    assert_eq!(f.chain().entries().len(), before);
    f.prepare(&m, &t, None).unwrap();
    let mut other = adapter();
    other.name = "installed-provider".into();
    assert!(matches!(
        workflow::prepare(
            f.home.path(),
            &mut f.chain(),
            "example/app",
            &f.head,
            &m,
            &t,
            None,
            &other,
            "2026-10-22T00:00:00Z",
        ),
        Err(PrepareError::Competing { .. })
    ));
}
