//! Spec 020's acceptance half: the inputs a later documentation acceptance
//! contract reads, decoded from the record and the custody store, with every
//! reason a result is ineligible, and acceptance `not-declared`, kept apart
//! from the provider result's own `acceptanceResult` (section 3.9).

use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};

use statecraft_acceptance::documentation::{self, Inputs};
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

fn declared(inputs: Inputs) -> documentation::Declared {
    match inputs {
        Inputs::Declared(d) => *d,
        other => panic!("expected declared inputs, got {other:?}"),
    }
}

#[test]
fn a_historical_attempt_has_no_documentation_inputs() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
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
    assert_eq!(
        documentation::inputs(&chain, home.path(), RUN, 1),
        Inputs::NotRecorded
    );
}

// Absence never reads as success: no testimony and no result are reasons.
#[test]
fn a_task_with_nothing_returned_is_ineligible_with_every_reason() {
    let f = begun();
    let d = declared(f.inputs());
    assert_eq!(d.task, f.task);
    assert_eq!(
        d.ineligible,
        vec!["delivery-not-recorded", "result-not-recorded"]
    );
    assert_eq!(d.decoded, None);
    assert_eq!(d.acceptance, documentation::Acceptance::NotDeclared);
}

// Section 3.9: the provider's sentinels and Statecraft's not-declared are
// two sources, labelled, and neither is written over the other.
#[test]
fn a_bound_result_is_decoded_again_and_its_sources_stay_apart() {
    let f = begun();
    f.deliver(&[f.report()]);
    f.return_bytes(result(&f.task).to_string().as_bytes());
    let d = declared(f.inputs());
    assert!(d.ineligible.is_empty(), "{:?}", d.ineligible);
    let decoded = d.decoded.as_ref().unwrap();
    assert_eq!(
        decoded.provider_completion,
        doc::ProviderCompletion::ClaimedComplete
    );
    let wire = serde_json::to_value(Inputs::Declared(Box::new(d))).unwrap();
    assert_eq!(wire["acceptance"], "not-declared");
    assert_eq!(wire["provider"]["source"], "provider-result");
    assert_eq!(wire["provider"]["acceptanceResult"], "not-attempted");
    assert_eq!(wire["decoded"]["acceptanceResult"], "not-attempted");
    // A patch is a review artifact: nothing here applies it.
    assert!(!f.target.path().join("docs/parser.md").exists());
}

#[test]
fn testimony_findings_and_an_invalid_result_are_reasons() {
    let f = begun();
    let mut wrong = f.report();
    wrong.result_schema_digest = context::custody_digest(b"another schema");
    f.deliver(&[wrong]);
    f.return_bytes(b"the documentation is done");
    let d = declared(f.inputs());
    assert_eq!(d.ineligible.len(), 2, "{:?}", d.ineligible);
    assert_eq!(d.ineligible[0], "delivery mismatch: resultSchemaDigest");
    assert!(d.ineligible[1].starts_with("result-schema-invalid"));
    assert_eq!(d.provider, None);
}

#[test]
fn bound_bytes_no_longer_under_their_custody_digest_are_a_reason() {
    let f = begun();
    f.deliver(&[f.report()]);
    let bytes = result(&f.task).to_string();
    f.return_bytes(bytes.as_bytes());
    let custody = context::custody_digest(bytes.as_bytes());
    std::fs::write(context::custody_path(f.home.path(), &custody), b"replaced").unwrap();
    let d = declared(f.inputs());
    assert_eq!(d.ineligible, vec!["result-bytes-not-retained"]);
    assert_eq!(d.decoded, None);
}

#[test]
fn an_unreadable_result_record_is_a_reason_never_an_absence() {
    let f = begun();
    f.deliver(&[f.report()]);
    {
        let mut chain = f.chain();
        chain
            .append(
                "run-020/1/documentation-result/x",
                "2026-10-22T00:00:02Z",
                &statecraft_run::record::Entry {
                    kind: statecraft_run::record::Kind::Evidence,
                    run_id: RUN.into(),
                    attempt: 1,
                    subject: doc::RESULT_SUBJECT.into(),
                    idempotency_key: None,
                    detail: json!({
                        "binding": { "taskDigest": f.task.task_digest },
                        "raw": "not a raw record"
                    }),
                    effect_id: statecraft_run::record::Identity::Absent,
                },
            )
            .unwrap();
    }
    let d = declared(f.inputs());
    assert_eq!(d.ineligible.len(), 1, "{:?}", d.ineligible);
    assert!(d.ineligible[0].starts_with("result-unreadable"));
}
