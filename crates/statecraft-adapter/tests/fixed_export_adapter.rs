//! Spec 022's offline boundary: the adapter the workflow names is selected
//! exactly, refused when a profile capability is missing, and admitted only
//! when the request carries the prepared subjects' exact bytes; the fake is
//! deterministic, its events are kept before parsing, its first terminal
//! result is the raw result, and every prohibited effect it attempts is
//! refused, kept and named as a finding. Every chain read is reopened.

use std::path::Path;
use std::process::Command;

use serde_json::json;

use statecraft_adapter::context::Bundle as ContextBundle;
use statecraft_adapter::documentation::{Bundle as DocumentationBundle, RawResult};
use statecraft_adapter::fixed_export::{
    self as fx, Declaration, Denial, Effect, ProfileCapability, Refusal, Step,
};
use statecraft_adapter::protocol::{AttemptIdentity, Request};
use statecraft_environment::time::FixedClock;
use statecraft_run::context::{
    self, AcceptedPage, AcceptedRepository, Completeness, DeliveryPolicy, Intended, Manifest,
    OptionalOmission, PageFacts, Producer, ProducerBuild, Requirement, Snapshot, Task,
};
use statecraft_run::documentation::workflow::{self, Workflow, WorkflowState};
use statecraft_run::documentation::{
    self as doc, Artifact, Binding, Candidate, ClockSource, CostCeiling, Deadline, Disposition,
    DocumentationTask, Format, NoSpend, NotRequired, Operations, OutputFormat, Projection,
    Publication, ResultProjection, ResultSchemaRef, ReviewPolicy, SemanticReview, Skill,
    Validation,
};
use statecraft_run::record::Chain;
use statecraft_run::session::{self, IntentDetail};

const RUN: &str = "run-022";
const PAGE: &[u8] = b"{\"page\":1}";
const AT: &str = "2026-10-28T00:00:00Z";

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
                        revision: "a".repeat(40),
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

fn result_bytes(t: &DocumentationTask) -> Vec<u8> {
    let (adapter, provider) = fx::fake_parties();
    json!({
        "schema": doc::RESULT_SCHEMA,
        "taskId": t.task_id,
        "runId": t.run_id,
        "attempt": t.attempt,
        "taskDigest": t.task_digest,
        "contextManifestDigest": t.context_manifest_digest,
        "adapter": adapter,
        "provider": provider,
        "artifacts": [],
        "claims": [],
        "unknowns": [],
        "contradictions": [],
        "unsupportedClaims": [],
        "providerOmissions": [],
        "statecraftCitationFailures": [],
        "executionEvidence": [],
        "providerCompletion": "claimed-incomplete",
        "reviewDisposition": "not-reviewed",
        "acceptanceResult": "not-attempted"
    })
    .to_string()
    .into_bytes()
}

struct Fixture {
    home: tempfile::TempDir,
    target: tempfile::TempDir,
    manifest: Manifest,
    task: DocumentationTask,
    prepared: Workflow,
}

fn prepared() -> Fixture {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let head = git(target.path(), &["rev-parse", "HEAD"]);
    let m = manifest();
    let t = task(&m, &head);
    context::retain(home.path(), PAGE).unwrap();
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
    let prepared = workflow::prepare(
        home.path(),
        &mut chain,
        "example/app",
        &head,
        &m,
        &t,
        None,
        &fx::fake_declaration().policy(),
        AT,
    )
    .unwrap()
    .workflow;
    Fixture {
        home,
        target,
        manifest: m,
        task: t,
        prepared,
    }
}

impl Fixture {
    fn chain(&self) -> Chain {
        Chain::open(self.home.path(), self.target.path()).unwrap().0
    }

    fn request(&self) -> Request {
        let read = |c: &str| context::read_retained(self.home.path(), c).unwrap();
        Request {
            workspace: self.target.path().to_path_buf(),
            base_commit: self.task.candidate.revision.clone(),
            prompt: b"Describe recovery.".to_vec(),
            capabilities: Default::default(),
            deadline_seconds: 60,
            attempt: AttemptIdentity {
                run_id: RUN.into(),
                number: 1,
            },
            context: Some(ContextBundle::assemble(&self.manifest, read).unwrap()),
            documentation: Some(DocumentationBundle::assemble(&self.task, None).unwrap()),
        }
    }

    fn binding(&self) -> Binding {
        let (adapter, provider) = fx::fake_parties();
        Binding::of(&self.task, adapter, provider)
    }

    fn execute(&self, script: &[Step]) -> (Workflow, Vec<Vec<u8>>, Denial) {
        let selected = fx::fake_declaration();
        let request = self.request();
        fx::admit(
            &request,
            &self.prepared,
            &self.manifest,
            &self.task,
            &selected,
        )
        .unwrap();
        let denial = Denial::new();
        let lines = fx::run_fake(&request, &self.task, script, &denial);
        let w = fx::record(
            self.home.path(),
            &mut self.chain(),
            &self.prepared,
            &self.manifest,
            &self.task,
            &selected,
            &self.binding(),
            &lines,
            &denial,
            AT,
        )
        .unwrap();
        (w, lines, denial)
    }
}

fn happy(t: &DocumentationTask) -> Vec<Step> {
    vec![
        Step::Init,
        Step::Progress("drafting".into()),
        Step::Terminal(result_bytes(t)),
    ]
}

// Section 3.3 and section 3.12's last row: only the exact adapter the
// workflow names is selected, even beside an installed real provider.
#[test]
fn selection_is_exact_and_a_real_provider_is_never_a_fallback() {
    let fake = fx::fake_declaration();
    let real = Declaration {
        name: "installed-provider".into(),
        version: "9.9.9".into(),
        profile: fx::REQUIRED.to_vec(),
        supports: vec![],
    };
    let available = [real.clone(), fake.clone()];
    assert_eq!(fx::select(&fake.policy(), &available).unwrap(), &fake);
    let mut other_version = fake.policy();
    other_version.version = "2".into();
    assert!(matches!(
        fx::select(&other_version, &available),
        Err(Refusal::NotSelected(_))
    ));
    assert!(matches!(
        fx::select(&fake.policy(), &[real]),
        Err(Refusal::NotSelected(_))
    ));
}

// Section 3.12 row 2: a missing capability refuses selection.
#[test]
fn a_missing_profile_capability_refuses_selection() {
    let mut lacking = fx::fake_declaration();
    lacking
        .profile
        .retain(|c| *c != ProfileCapability::OfflineInput);
    let err = fx::select(&lacking.policy(), &[lacking.clone()]).unwrap_err();
    assert_eq!(err.to_string(), "capability-missing: offline-input");
}

// Section 3.3: every byte reference is checked before spawn, and only the
// adapter the prepared workflow names is admitted.
#[test]
fn admission_refuses_a_request_that_is_not_the_prepared_subjects() {
    let f = prepared();
    let selected = fx::fake_declaration();
    let good = f.request();
    fx::admit(&good, &f.prepared, &f.manifest, &f.task, &selected).unwrap();

    let mut moved = good.clone();
    moved.context.as_mut().unwrap().pages[0].bytes = "{\"page\":2}".into();
    assert!(matches!(
        fx::admit(&moved, &f.prepared, &f.manifest, &f.task, &selected),
        Err(Refusal::Context(_))
    ));

    let mut no_task = good.clone();
    no_task.documentation = None;
    assert!(matches!(
        fx::admit(&no_task, &f.prepared, &f.manifest, &f.task, &selected),
        Err(Refusal::Documentation(_))
    ));

    let mut other = good.clone();
    other.attempt.number = 2;
    assert!(matches!(
        fx::admit(&other, &f.prepared, &f.manifest, &f.task, &selected),
        Err(Refusal::Workflow(_))
    ));

    let mut lookalike = selected.clone();
    lookalike.name = "installed-provider".into();
    assert!(matches!(
        fx::admit(&good, &f.prepared, &f.manifest, &f.task, &lookalike),
        Err(Refusal::NotSelected(_))
    ));

    let mut executed = f.prepared.clone();
    executed.transitions.push(workflow::Transition {
        state: WorkflowState::Executed,
        reason: "already".into(),
        at: AT.into(),
    });
    assert!(matches!(
        fx::admit(&good, &executed, &f.manifest, &f.task, &selected),
        Err(Refusal::Workflow(_))
    ));
}

// Sections 3.3, 3.4 and 3.5: a deterministic run keeps every event, folds
// delivery, keeps the raw result, and appends executed then validated.
#[test]
fn the_fake_runs_deterministically_and_every_output_is_recorded_before_interpretation() {
    let f = prepared();
    let script = happy(&f.task);
    let again = fx::run_fake(&f.request(), &f.task, &script, &Denial::new());
    let (w, lines, denial) = f.execute(&script);
    assert_eq!(lines, again);
    assert!(denial.attempted().is_empty());
    assert_eq!(w.state(), Some(WorkflowState::Validated));
    assert_eq!(w.transitions[2].reason, "bound");
    assert!(w.findings.is_empty(), "{:?}", w.findings);
    assert_eq!(w.subjects.events.len(), 3);
    for (e, bytes) in w.subjects.events.iter().zip(&lines) {
        let kept = context::read_retained(f.home.path(), &e.custody_digest)
            .unwrap()
            .unwrap();
        assert_eq!(&kept, bytes);
    }
    let raw = w.subjects.raw_result.clone().unwrap();
    assert_eq!(raw, context::custody_digest(&result_bytes(&f.task)));
    assert_eq!(
        w.subjects.adapter.as_ref().unwrap(),
        &fx::fake_declaration().reference()
    );

    let chain = f.chain();
    let versions = workflow::versions(&chain, RUN, 1);
    assert_eq!(versions.len(), 3);
    assert_eq!(versions[0].as_ref().unwrap(), &f.prepared);
    match doc::project(&chain, RUN, 1) {
        Projection::Recorded {
            delivery, result, ..
        } => {
            assert!(delivery.unwrap().eligible);
            match result {
                ResultProjection::Recorded(r) => {
                    assert!(matches!(r.validation, Validation::Bound { .. }));
                    assert_eq!(r.raw.custody_digest, raw);
                }
                other => panic!("{other:?}"),
            }
        }
        other => panic!("{other:?}"),
    }
}

// Section 3.12 row 3: intended bytes with no init observation fail
// adapter-observed delivery on their own.
#[test]
fn a_missing_init_event_fails_delivery_independently() {
    let f = prepared();
    let (w, _, _) = f.execute(&[Step::Terminal(result_bytes(&f.task))]);
    assert!(
        w.findings
            .iter()
            .any(|x| x.starts_with("delivery not-reported")),
        "{:?}",
        w.findings
    );
    assert_eq!(w.transitions[2].reason, "bound");
}

// Section 3.4: a second terminal is a finding and the first is the result.
#[test]
fn a_second_terminal_is_kept_as_a_finding_and_never_chosen() {
    let f = prepared();
    let mut script = happy(&f.task);
    script.push(Step::Terminal(b"{\"better\":true}".to_vec()));
    let (w, _, _) = f.execute(&script);
    assert!(w.findings.iter().any(|x| x.starts_with("second-terminal")));
    assert_eq!(
        w.subjects.raw_result.unwrap(),
        context::custody_digest(&result_bytes(&f.task))
    );
    assert_eq!(w.subjects.events.len(), 4);
}

// Section 3.12 row 5: malformed terminal bytes stay durable and validation
// fails; nothing repairs them.
#[test]
fn a_malformed_result_stays_retained_and_validation_fails() {
    let f = prepared();
    let truncated = result_bytes(&f.task)[..40].to_vec();
    let (w, _, _) = f.execute(&[Step::Init, Step::Terminal(truncated.clone())]);
    assert!(w.transitions[2].reason.starts_with("schema-invalid"));
    let raw = w.subjects.raw_result.unwrap();
    assert_eq!(
        context::read_retained(f.home.path(), &raw)
            .unwrap()
            .unwrap(),
        truncated
    );
}

// Section 3.4: a stated length, digest or media type that disagrees with
// the bytes is a finding; the bytes are what is kept.
#[test]
fn stated_custody_that_disagrees_with_the_bytes_is_a_finding() {
    let f = prepared();
    let bytes = String::from_utf8(result_bytes(&f.task)).unwrap();
    let (w, _, _) = f.execute(&[
        Step::Init,
        Step::TerminalStated(RawResult {
            media_type: "text/plain".into(),
            byte_length: 1,
            custody_digest: context::custody_digest(b"other"),
            bytes: bytes.clone(),
            artifacts: vec![],
        }),
    ]);
    for code in ["raw-length-mismatch", "raw-digest-mismatch", "media-type"] {
        assert!(
            w.findings.iter().any(|x| x.starts_with(code)),
            "{code}: {:?}",
            w.findings
        );
    }
    assert_eq!(
        w.subjects.raw_result.unwrap(),
        context::custody_digest(bytes.as_bytes())
    );
}

// Section 3.4: an init event for another task is a delivery finding.
#[test]
fn testimony_for_another_task_is_a_delivery_finding() {
    let f = prepared();
    let (w, _, _) = f.execute(&[
        Step::InitForTask(context::custody_digest(b"another")),
        Step::Terminal(result_bytes(&f.task)),
    ]);
    assert!(
        w.findings
            .iter()
            .any(|x| x == "delivery mismatch: taskDigest"),
        "{:?}",
        w.findings
    );
}

// Sections 3.3 and 3.12 row 11: every attempted prohibited effect is
// refused by the boundary, emitted as a structured refusal, and named on
// the workflow; nothing is performed.
#[test]
fn attempted_network_credential_write_and_publication_are_refused_and_recorded() {
    let f = prepared();
    let marker = f.target.path().join("written.md");
    let effects = [
        Effect::Network("https://example.invalid".into()),
        Effect::CredentialRead("provider-token".into()),
        Effect::RepositoryWrite(marker.display().to_string()),
        Effect::Publication("pull-request".into()),
        Effect::Clock,
        Effect::Random,
    ];
    let mut script = vec![Step::Init];
    script.extend(effects.iter().cloned().map(Step::Attempt));
    script.push(Step::Terminal(result_bytes(&f.task)));
    let before = git(
        f.target.path(),
        &["status", "--porcelain", "--untracked-files=all"],
    );
    let (w, lines, denial) = f.execute(&script);
    assert_eq!(denial.attempted(), effects.to_vec());
    for e in &effects {
        let want = format!("prohibited-effect: {}", e.describe());
        assert!(w.findings.contains(&want), "{want}: {:?}", w.findings);
    }
    let refusals = lines
        .iter()
        .filter(|l| {
            let v: serde_json::Value = serde_json::from_slice(l).unwrap();
            v["event"] == "refusal" && v["guard"] == fx::BOUNDARY_GUARD
        })
        .count();
    assert_eq!(refusals, effects.len());
    assert!(!marker.exists());
    assert_eq!(
        git(
            f.target.path(),
            &["status", "--porcelain", "--untracked-files=all"]
        ),
        before
    );
}

// Section 3.4: a line that is not an event is kept and named.
#[test]
fn an_unreadable_line_is_kept_and_named() {
    let f = prepared();
    let (w, lines, _) = f.execute(&[
        Step::Init,
        Step::Raw(b"not json".to_vec()),
        Step::Terminal(result_bytes(&f.task)),
    ]);
    assert!(w.findings.iter().any(|x| x == "unreadable-event: event 1"));
    assert_eq!(w.subjects.events[1].media_type, "application/octet-stream");
    assert_eq!(
        context::read_retained(f.home.path(), &w.subjects.events[1].custody_digest)
            .unwrap()
            .unwrap(),
        lines[1]
    );
}

// Section 3.6: the artifacts a terminal event carries are kept as blobs in
// the run record before the result naming them is decoded.
#[test]
fn artifact_bytes_are_kept_before_the_result_is_decoded() {
    let f = prepared();
    let artifact = "# Recovery\n";
    f.execute(&[
        Step::Init,
        Step::TerminalWithArtifacts(
            result_bytes(&f.task),
            vec![statecraft_adapter::documentation::RawArtifact {
                raw_ref: "a0".into(),
                bytes: artifact.into(),
            }],
        ),
    ]);
    assert_eq!(
        context::read_retained(f.home.path(), &context::custody_digest(artifact.as_bytes()))
            .unwrap()
            .unwrap(),
        artifact.as_bytes()
    );
}
