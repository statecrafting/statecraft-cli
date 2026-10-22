//! Spec 020's adapter half: the documentation task, its result schema and
//! its skill ride the request beside the prompt, are refused before spawn
//! when they are not the recorded task's, reach a real child on its input
//! stream and never on its command line, and come back only as the init
//! event's testimony and the terminal event's raw bytes.
#![cfg(unix)]

use std::path::Path;

use serde_json::json;

use statecraft_adapter::capability::{Capability, Requested};
use statecraft_adapter::documentation::{self, Bundle, BundleError, RawResult, Returned};
use statecraft_adapter::environment::{Blueprint, CheckSuiteCommands, construct};
use statecraft_adapter::fixture::install_script;
use statecraft_adapter::protocol::{AttemptIdentity, Event, Request};
use statecraft_adapter::supervisor::supervise;
use statecraft_run::context::custody_digest;
use statecraft_run::documentation::delivery::{self, Reported};
use statecraft_run::documentation::{
    self as doc, Artifact, Candidate, ClockSource, CostCeiling, Deadline, Disposition,
    DocumentationTask, Format, NoSpend, NotRequired, Operations, OutputFormat, Publication,
    ResultSchemaRef, ReviewPolicy, SemanticReview, Skill, SkillIdentity,
};

const RUN: &str = "run-020";

fn task(skill: Skill, capabilities: &[&str]) -> DocumentationTask {
    DocumentationTask {
        schema: doc::TASK_SCHEMA.into(),
        task_id: "task-020".into(),
        run_id: RUN.into(),
        attempt: 1,
        candidate: Candidate {
            repository: "app".into(),
            revision: "0".repeat(40),
            tree: "tree".into(),
        },
        context_manifest_digest: custody_digest(b"manifest"),
        objective: "Describe the parser's error recovery.".into(),
        audiences: vec!["operators".into()],
        output_format: OutputFormat {
            format: Format::Markdown,
            version: "commonmark-0.31.2".into(),
        },
        operations: Operations::v1(),
        required_capabilities: capabilities.iter().map(|c| c.to_string()).collect(),
        skill,
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

fn plain() -> DocumentationTask {
    task(
        Skill::NotRequired(NotRequired::NotRequired),
        &["turn-limit"],
    )
}

fn request(workspace: &Path, bundle: Option<Bundle>) -> Request {
    Request {
        workspace: workspace.to_path_buf(),
        base_commit: "0".repeat(40),
        prompt: b"write the documentation".to_vec(),
        capabilities: Requested::none(),
        deadline_seconds: 60,
        attempt: AttemptIdentity {
            run_id: RUN.into(),
            number: 1,
        },
        context: None,
        documentation: bundle,
    }
}

fn env() -> statecraft_adapter::ChildEnvironment {
    construct(
        &Blueprint::empty()
            .allowing("PATH", "/usr/bin:/bin")
            .needing_command("sh"),
        &CheckSuiteCommands(vec!["sh".into()]),
    )
}

/// A child that keeps its input and its arguments, then prints `lines`.
fn child(dir: &Path, lines: &[String]) -> std::path::PathBuf {
    let path = dir.join("documentation-child.sh");
    let mut script = String::from("#!/bin/sh\ncat > received\nprintf '%s' \"$*\" > argv\n");
    for line in lines {
        script.push_str(&format!("printf '%s\\n' '{line}'\n"));
    }
    install_script(&path, script, 0o755).unwrap();
    path
}

fn init(report: Option<Reported>) -> String {
    serde_json::to_string(&Event::Init {
        applied: vec![Capability::TurnLimit],
        adapter_version: "1.0.0".into(),
        provider_version: "fixture".into(),
        context: None,
        documentation: report,
    })
    .unwrap()
}

fn result_bytes(task: &DocumentationTask) -> String {
    json!({
        "schema": doc::RESULT_SCHEMA,
        "taskId": task.task_id,
        "runId": task.run_id,
        "attempt": task.attempt,
        "taskDigest": task.task_digest,
        "contextManifestDigest": task.context_manifest_digest,
        "adapter": {"name": "fixture-adapter", "version": "1.0.0"},
        "provider": {"name": "fixture", "version": "fixture"},
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
}

fn terminal(raw: Option<RawResult>) -> String {
    serde_json::to_string(&Event::Result {
        classification: statecraft_adapter::protocol::Classification::Completed,
        cost: None,
        documentation: raw,
    })
    .unwrap()
}

fn raw(bytes: &str) -> RawResult {
    RawResult {
        media_type: "application/json".into(),
        byte_length: bytes.len() as u64,
        custody_digest: custody_digest(bytes.as_bytes()),
        bytes: bytes.to_string(),
    }
}

// Section 3.4: separate members, checked before spawn.
#[test]
fn the_bundle_carries_the_recorded_task_schema_and_skill_and_checks_before_spawn() {
    let t = plain();
    let b = Bundle::assemble(&t, None).unwrap();
    assert_eq!(b.task.as_bytes(), t.to_bytes());
    assert_eq!(b.result_schema.as_bytes(), doc::RESULT_SCHEMA_BYTES);
    documentation::check(Some(&b), Some(&t), &[Capability::TurnLimit]).unwrap();
    documentation::check(None, None, &[]).unwrap();
    assert_eq!(
        documentation::check(Some(&b), None, &[]),
        Err(BundleError::Presence)
    );
    assert_eq!(
        documentation::check(None, Some(&t), &[]),
        Err(BundleError::Presence)
    );
}

#[test]
fn a_bundle_that_is_not_the_recorded_task_refuses_before_spawn() {
    let t = plain();
    let declared = [Capability::TurnLimit];
    let mut tampered = Bundle::assemble(&t, None).unwrap();
    tampered.task = tampered.task.replace("operators", "attackers");
    assert!(matches!(
        documentation::check(Some(&tampered), Some(&t), &declared),
        Err(BundleError::Task(_))
    ));
    let mut reformatted = Bundle::assemble(&t, None).unwrap();
    let value: serde_json::Value = serde_json::from_str(&reformatted.task).unwrap();
    reformatted.task = serde_json::to_string_pretty(&value).unwrap();
    assert_eq!(
        documentation::check(Some(&reformatted), Some(&t), &declared),
        Err(BundleError::Task(
            "not the recorded task's exact bytes".into()
        ))
    );
    let mut schema = Bundle::assemble(&t, None).unwrap();
    schema.result_schema.push(' ');
    assert_eq!(
        documentation::check(Some(&schema), Some(&t), &declared),
        Err(BundleError::ResultSchema)
    );
    let mut skill = Bundle::assemble(&t, None).unwrap();
    skill.skill = Some(documentation::SkillBytes {
        digest: custody_digest(b"x"),
        bytes: "x".into(),
    });
    assert!(matches!(
        documentation::check(Some(&skill), Some(&t), &declared),
        Err(BundleError::Skill(_))
    ));
}

#[test]
fn a_pinned_skill_must_be_carried_as_its_exact_bytes() {
    let bytes = b"# the documentation skill\n";
    let t = task(
        Skill::Pinned(SkillIdentity {
            id: "docs".into(),
            version: "1".into(),
            digest: custody_digest(bytes),
        }),
        &[],
    );
    assert!(matches!(
        Bundle::assemble(&t, None),
        Err(BundleError::Skill(_))
    ));
    let b = Bundle::assemble(&t, Some(bytes)).unwrap();
    documentation::check(Some(&b), Some(&t), &[]).unwrap();
    let mut wrong = b.clone();
    wrong.skill.as_mut().unwrap().bytes.push('!');
    assert!(matches!(
        documentation::check(Some(&wrong), Some(&t), &[]),
        Err(BundleError::Skill(_))
    ));
}

// Section 3.4: every required capability is declared by the selected adapter.
#[test]
fn a_required_capability_the_adapter_does_not_declare_refuses() {
    let t = task(
        Skill::NotRequired(NotRequired::NotRequired),
        &["structured-refusals"],
    );
    let b = Bundle::assemble(&t, None).unwrap();
    assert_eq!(
        documentation::check(Some(&b), Some(&t), &[Capability::TurnLimit]),
        Err(BundleError::Capability("structured-refusals".into()))
    );
    let unknown = task(
        Skill::NotRequired(NotRequired::NotRequired),
        &["structured-result"],
    );
    let b = Bundle::assemble(&unknown, None).unwrap();
    assert_eq!(
        documentation::check(Some(&b), Some(&unknown), &Capability::all()),
        Err(BundleError::Capability("structured-result".into()))
    );
}

// Sections 3.4 and 3.10 row 3: bytes on a stream, testimony back, raw kept.
#[test]
fn a_real_child_receives_the_task_on_its_input_and_returns_testimony_and_raw_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let t = plain();
    let b = Bundle::assemble(&t, None).unwrap();
    let body = result_bytes(&t);
    let program = child(
        dir.path(),
        &[
            init(Some(b.delivered(&t, &[Capability::TurnLimit]))),
            terminal(Some(raw(&body))),
        ],
    );
    let req = request(dir.path(), Some(b.clone()));
    let run = supervise(&program, &[], &req, &env()).unwrap();

    let received = std::fs::read(dir.path().join("received")).unwrap();
    assert_eq!(received, req.provider_input());
    assert!(received.starts_with(b"write the documentation"));
    // Every frame header is one line: the preamble stands on its own line
    // and the task's header begins `=== task, digest`.
    let header = format!("\n=== task, digest {}, ", b.task_digest);
    assert!(
        received
            .windows(header.len())
            .any(|w| w == header.as_bytes())
    );
    let framed = format!(
        "{} bytes follow, then one newline ===\n{}\n",
        b.task.len(),
        b.task
    );
    assert_eq!(
        received
            .windows(framed.len())
            .filter(|w| *w == framed.as_bytes())
            .count(),
        1,
        "the task appears once, inside its declared length"
    );
    assert!(std::fs::read(dir.path().join("argv")).unwrap().is_empty());

    let reports = documentation::reported(&run.events);
    let d = delivery::fold(&t, &reports);
    assert!(d.eligible, "{:?}", d.findings);
    match documentation::returned(&run.events) {
        Returned::Raw { raw, finding } => {
            assert_eq!(finding, None);
            assert_eq!(raw.bytes, body);
            let parsed = doc::DocumentationResult::decode(raw.bytes.as_bytes(), &t).unwrap();
            assert_eq!(
                parsed.provider_completion,
                doc::ProviderCompletion::ClaimedIncomplete
            );
        }
        other => panic!("expected one raw result, got {other:?}"),
    }
}

// Section 3.10 row 2: an omitted schema digest observation is incomplete.
#[test]
fn missing_mismatched_duplicated_or_contradictory_testimony_is_ineligible() {
    let t = plain();
    let b = Bundle::assemble(&t, None).unwrap();
    let good = b.delivered(&t, &[Capability::TurnLimit]);

    let d = delivery::fold(&t, &[]);
    assert!(!d.eligible);
    assert!(d.findings[0].starts_with("not-reported"));

    let mut no_schema = good.clone();
    no_schema.result_schema_digest = String::new();
    let d = delivery::fold(&t, &[no_schema]);
    assert_eq!(d.findings, vec!["mismatch: resultSchemaDigest".to_string()]);

    let mut skill = good.clone();
    skill.skill = custody_digest(b"a skill");
    let d = delivery::fold(&t, &[skill]);
    assert_eq!(d.findings, vec!["mismatch: skill".to_string()]);

    let mut fewer = good.clone();
    fewer.capabilities.clear();
    let d = delivery::fold(&t, &[fewer.clone()]);
    assert!(!d.eligible);

    let d = delivery::fold(&t, &[good.clone(), good.clone()]);
    assert!(d.findings[0].starts_with("duplicate"));
    let d = delivery::fold(&t, &[good, fewer]);
    assert!(d.findings[0].starts_with("contradictory"));
}

#[test]
fn a_raw_result_whose_statement_disagrees_with_its_bytes_is_kept_with_a_finding() {
    let t = plain();
    let body = result_bytes(&t);
    let mut lying = raw(&body);
    lying.byte_length += 1;
    match documentation::returned(&[Event::Result {
        classification: statecraft_adapter::protocol::Classification::Completed,
        cost: None,
        documentation: Some(lying),
    }]) {
        Returned::Raw { raw, finding } => {
            assert_eq!(finding.as_deref(), Some("raw-length-mismatch"));
            assert_eq!(raw.bytes, body, "the bytes are kept as returned");
        }
        other => panic!("expected a raw result, got {other:?}"),
    }
    let two = [
        Event::Result {
            classification: statecraft_adapter::protocol::Classification::Completed,
            cost: None,
            documentation: Some(raw(&body)),
        },
        Event::Result {
            classification: statecraft_adapter::protocol::Classification::Completed,
            cost: None,
            documentation: Some(raw("{}")),
        },
    ];
    assert!(matches!(
        documentation::returned(&two),
        Returned::Duplicate(r) if r.len() == 2
    ));
}

#[test]
fn a_request_without_a_documentation_bundle_reads_as_before() {
    let dir = tempfile::tempdir().unwrap();
    let program = child(dir.path(), &[init(None), terminal(None)]);
    let req = request(dir.path(), None);
    let run = supervise(&program, &[], &req, &env()).unwrap();
    let received = std::fs::read(dir.path().join("received")).unwrap();
    assert_eq!(received, b"write the documentation");
    assert!(documentation::reported(&run.events).is_empty());
    assert_eq!(documentation::returned(&run.events), Returned::NotReturned);
    let wire = serde_json::to_value(&req).unwrap();
    assert!(wire.get("documentation").is_none());
}
