//! Spec 020 section 3.9 through the command surface's values: `run show`
//! adds the task identity, the delivery testimony, the result's identity and
//! counts, and `accept` adds the documentation inputs with acceptance
//! `not-declared`, labelled apart from the provider's own sentinels. The
//! human summary and the JSON are two renderings of one value, and an
//! attempt with no task renders `not-recorded`.

use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};

use statecraft_acceptance::documentation::{self, Inputs};
use statecraft_acceptance::judged::NoAcceptance;
use statecraft_acceptance::outcome::Acceptance;
use statecraft_cli::slice;
use statecraft_environment::time::FixedClock;
use statecraft_run::context::{
    self, AcceptedPage, AcceptedRepository, Completeness, DeliveryPolicy, Intended, Manifest,
    OptionalOmission, PageFacts, Producer, ProducerBuild, Requirement, Snapshot, Task,
};
use statecraft_run::documentation::delivery::{self, Reported};
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

    fn inputs(&self) -> Inputs {
        documentation::inputs(&self.chain(), self.home.path(), RUN, 1)
    }
}

fn run_show(f: &Fixture) -> (Value, String) {
    let chain = f.chain();
    let run = session::runs(&chain)
        .into_iter()
        .find(|r| r.id == RUN)
        .unwrap();
    let answer = slice::run_show_with_admissions(
        statecraft_acceptance::suite::fold(RUN, &chain.entries()),
        &run,
        slice::ReconciliationView::of_run(&chain.positioned_entries(), RUN),
        vec![],
        slice::WireEvidenceView::of_run(&chain, RUN),
        statecraft_cli::context::of_run(&chain, &run),
        statecraft_cli::documentation::of_run(&chain, &run),
    );
    (serde_json::to_value(&answer.value).unwrap(), answer.summary)
}

#[test]
fn run_show_renders_the_task_the_testimony_and_the_result_from_the_record() {
    let f = begun();
    f.deliver(&[f.report()]);
    f.return_bytes(result(&f.task).to_string().as_bytes());
    let (value, summary) = run_show(&f);
    let shown = &value["documentation"][0];
    assert_eq!(shown["attempt"], 1);
    assert_eq!(shown["state"], "recorded");
    assert_eq!(shown["task"]["taskDigest"], json!(f.task.task_digest));
    assert_eq!(shown["delivery"]["eligible"], true);
    assert_eq!(shown["result"]["validation"]["state"], "bound");
    assert_eq!(
        shown["result"]["validation"]["summary"]["providerCompletion"],
        "claimed-complete"
    );
    assert_eq!(
        shown["result"]["validation"]["summary"]["artifacts"][0]["logicalPath"],
        "docs/parser.md"
    );
    assert!(summary.contains(&format!(
        "documentation task task-020 {}",
        f.task.task_digest
    )));
    assert!(summary.contains("provider claimed-complete (the provider's claim)"));
    // Showing reads; it applies no patch and writes no expected path.
    assert!(!f.target.path().join("docs/parser.md").exists());
}

#[test]
fn run_show_and_accept_render_not_recorded_for_an_attempt_with_no_task() {
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
    let (value, summary) = run_show(&f);
    assert_eq!(
        value["documentation"],
        json!([{ "attempt": 1, "state": "not-recorded", "records": [] }])
    );
    assert!(!summary.contains("documentation"));
    let answer = slice::accept_answer(Acceptance::None {
        reason: NoAcceptance::SuiteDidNotRun { unrun_checks: 0 },
    });
    let wire = serde_json::to_value(&answer.value).unwrap();
    assert_eq!(wire["documentation"], json!({ "state": "not-recorded" }));
}

#[test]
fn accept_labels_not_declared_apart_from_the_provider_sentinel() {
    let f = begun();
    f.deliver(&[f.report()]);
    f.return_bytes(result(&f.task).to_string().as_bytes());
    let mut answer = slice::accept_answer(Acceptance::None {
        reason: NoAcceptance::SuiteDidNotRun { unrun_checks: 0 },
    });
    answer.value.documentation = f.inputs();
    let wire = serde_json::to_value(&answer.value).unwrap();
    let d = &wire["documentation"];
    assert_eq!(d["state"], "declared");
    assert_eq!(d["acceptance"], "not-declared");
    assert_eq!(d["provider"]["source"], "provider-result");
    assert_eq!(d["provider"]["acceptanceResult"], "not-attempted");
    assert_eq!(d["ineligible"], json!([]));
    // The provider said complete; acceptance stays independent of it.
    assert_eq!(d["decoded"]["providerCompletion"], "claimed-complete");
    assert_ne!(d["acceptance"], d["decoded"]["providerCompletion"]);
}
