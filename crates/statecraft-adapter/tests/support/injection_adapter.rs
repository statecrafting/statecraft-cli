//! Spec 025's adapter-crate runner: one case seeded on the `provider-output`,
//! `tool-facing`, `skill-content` or `context-packet` surface, run end to end through a real
//! child and spec 022's offline workflow, and observed as decisions.
//!
//! The child is the fixture adapter's hostile mode ([`fixture::Hostile`]).
//! Everything else is the product as a run uses it: the manifest and task
//! recorded in the intent, the workflow prepared from retained bytes, the
//! request's bundles checked before spawn, the supervisor, and the recording
//! of every event, the delivery testimony and the raw result.
//!
//! Requires `super::injection` in scope as `injection`.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

use statecraft_adapter::capability::{Capability, Requested, negotiate};
use statecraft_adapter::context::Bundle as ContextBundle;
use statecraft_adapter::documentation::{self as adapter_doc, Bundle as DocumentationBundle};
use statecraft_adapter::environment::{Blueprint, CheckSuiteCommands, construct};
use statecraft_adapter::fixed_export::{self as fx, Denial};
use statecraft_adapter::fixture::{self, Hostile};
use statecraft_adapter::manifest::qualification;
use statecraft_adapter::posture::Posture;
use statecraft_adapter::protocol::{
    AttemptIdentity, Classification, Event, Request, read_stream, refusals,
};
use statecraft_adapter::supervisor::supervise;
use statecraft_environment::time::FixedClock;
use statecraft_run::context;
use statecraft_run::documentation::review::{self, Read};
use statecraft_run::documentation::workflow;
use statecraft_run::documentation::{
    self as doc, Binding, DocumentationResult, DocumentationTask, NotRequired, Skill, SkillIdentity,
};
use statecraft_run::record::Chain;
use statecraft_run::session::{self, IntentDetail};

use super::injection::fixtures::{AT, RUN, git, manifest, pages, repo, task};
use super::injection::{Case, Framing, Observed, Runner, Side};

const PROMPT: &[u8] = b"Describe recovery.";
const SKILL_HEADER: &str = "# documentation skill\n\n";
const SHIMMED: [&str; 8] = ["curl", "wget", "git", "gh", "nc", "ssh", "scp", "cargo"];

/// A bound result; on `provider-output` the payload rides an unknown's
/// question, which is free text the schema keeps as data.
fn result_bytes(t: &DocumentationTask, question: Option<&str>) -> String {
    let (adapter, provider) = fx::fake_parties();
    let unknowns = match question {
        None => json!([]),
        Some(q) => json!([{
            "unknownId": "u-1",
            "question": q,
            "missingFact": "none",
            "reason": "reported by the provider",
        }]),
    };
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
        "unknowns": unknowns,
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

fn line(e: &Event) -> String {
    serde_json::to_string(e).unwrap()
}

/// A PATH directory of shims for every program a hostile instruction names:
/// each records that it ran and its arguments, and does nothing else.
fn shims(dir: &Path, trace: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    for name in SHIMMED {
        fixture::install_script(
            &dir.join(name),
            format!(
                "#!/bin/sh\nprintf '%s %s\\n' '{name}' \"$*\" >> '{}'\nexit 0\n",
                trace.display()
            ),
            0o755,
        )
        .unwrap();
    }
}

/// Every file under `root`, by relative path, with the custody digest of its
/// bytes; `.git` internals are summarised by the refs that matter instead.
fn tree(root: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
                out.insert(
                    rel,
                    context::custody_digest(&std::fs::read(&p).unwrap_or_default()),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// The home directory's layout with every digest-named component folded,
/// so the twin's custody blobs line up with the hostile run's by position.
fn layout(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = tree(root)
        .into_keys()
        .map(|k| {
            k.split('/')
                .map(|c| {
                    let stem = c.split('.').next().unwrap_or(c);
                    if stem.len() >= 40 && stem.bytes().all(|b| b.is_ascii_hexdigit()) {
                        "<digest>".to_string()
                    } else {
                        c.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("/")
        })
        .collect();
    names.sort();
    names
}

fn reads<T: serde::Serialize>(items: Vec<Read<T>>) -> Vec<Value> {
    items
        .into_iter()
        .map(|r| match r {
            Read::Valid(t) => json!({"valid": t}),
            Read::Unreadable(why) => json!({"unreadable": why}),
        })
        .collect()
}

/// Run one side of `case` on `runner`'s surface and observe it.
pub fn observe(runner: &Runner, case: &Case, side: Side, framing: Framing) -> Observed {
    observe_with(runner, case, side, framing, &[])
}

/// [`observe`], with `trailing` event lines the child prints after its
/// terminal event: a provider that forges a second result, say.
pub fn observe_with(
    runner: &Runner,
    case: &Case,
    side: Side,
    framing: Framing,
    trailing: &[String],
) -> Observed {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let target = root.path().join("target");
    let bin = root.path().join("shims");
    let trace = root.path().join("trace");
    let child_env = root.path().join("child-environment");
    let fixture_dir = root.path().join("fixture");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&fixture_dir).unwrap();
    std::fs::write(&trace, b"").unwrap();
    repo(&target, &[]);
    shims(&bin, &trace);
    let payload = case.bytes(side);
    let mut seen = Observed::rooted_at(&[root.path()]);

    // The surface's seeding.
    let skill_bytes = (runner.surface == "skill-content")
        .then(|| format!("{SKILL_HEADER}{}\n", payload.assembled()).into_bytes());
    let mut tool_reads = Vec::new();
    if runner.surface == "tool-facing" {
        for (i, f) in payload.fragments.iter().enumerate() {
            let name = format!("tool-output-{i}.txt");
            std::fs::write(target.join(&name), f).unwrap();
            tool_reads.push(name);
        }
    }

    let head = git(&target, &["rev-parse", "HEAD"]);
    let packet = pages(case, side);
    let m = manifest(&packet);
    let skill = match &skill_bytes {
        None => Skill::NotRequired(NotRequired::NotRequired),
        Some(b) => Skill::Pinned(SkillIdentity {
            id: "docs".into(),
            version: "1".into(),
            digest: context::custody_digest(b),
        }),
    };
    let t = task(&m, &head, skill);
    for p in &packet {
        context::retain(&home, p).unwrap();
    }
    let (mut chain, _) = Chain::open(&home, &target).unwrap();
    session::begin_with(
        &mut chain,
        &target,
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
    let selected = fx::fake_declaration();
    let prepared = workflow::prepare(
        &home,
        &mut chain,
        "example/app",
        &head,
        &m,
        &t,
        skill_bytes.as_deref(),
        &selected.policy(),
        AT,
    )
    .unwrap()
    .workflow;

    // The request, framed as the product frames it or with every frame
    // removed and the same bytes concatenated (section 3.6).
    let read = |c: &str| context::read_retained(&home, c).unwrap();
    let context_bundle = ContextBundle::assemble(&m, read).unwrap();
    let doc_bundle = DocumentationBundle::assemble(&t, skill_bytes.as_deref()).unwrap();
    let mut request = Request {
        workspace: target.clone(),
        base_commit: head.clone(),
        prompt: PROMPT.to_vec(),
        capabilities: Requested::none(),
        deadline_seconds: 60,
        attempt: AttemptIdentity {
            run_id: RUN.into(),
            number: 1,
        },
        context: Some(context_bundle.clone()),
        documentation: Some(doc_bundle.clone()),
    };
    if framing == Framing::Removed {
        let mut raw = PROMPT.to_vec();
        for p in &context_bundle.pages {
            raw.extend(p.bytes.as_bytes());
        }
        raw.extend(doc_bundle.task.as_bytes());
        raw.extend(doc_bundle.result_schema.as_bytes());
        if let Some(s) = &doc_bundle.skill {
            raw.extend(s.bytes.as_bytes());
        }
        request.prompt = raw;
        request.context = None;
        request.documentation = None;
    }
    let admission = fx::admit(&request, &prepared, &m, &t, &selected).map_err(|e| e.to_string());

    // The child's script: what it says is the surface's payload as data.
    let applied = vec![Capability::StructuredRefusals];
    let mut lines = vec![line(&Event::Init {
        applied: applied.clone(),
        adapter_version: fx::FAKE_VERSION.to_string(),
        provider_version: format!("{}/{}", fx::FAKE_NAME, fx::FAKE_VERSION),
        context: Some(context_bundle.delivered()),
        documentation: Some(doc_bundle.delivered(&t, &applied)),
    })];
    let question = match runner.surface {
        // The case's recording names the carrier: the stream's events, or
        // the result document's free text.
        "provider-output" if case.expected_recording.as_deref() == Some("stream-events") => {
            for f in &payload.fragments {
                lines.push(line(&Event::Progress { message: f.clone() }));
            }
            None
        }
        "provider-output" => {
            lines.push(line(&Event::Progress {
                message: "drafting".into(),
            }));
            Some(payload.assembled())
        }
        "tool-facing" => {
            for f in &payload.fragments {
                lines.push(line(&Event::Progress {
                    message: format!("tool output: {f}"),
                }));
            }
            None
        }
        _ => {
            lines.push(line(&Event::Progress {
                message: "drafting".into(),
            }));
            None
        }
    };
    let bytes = result_bytes(&t, question.as_deref());
    lines.push(line(&Event::Result {
        classification: Classification::Completed,
        cost: None,
        documentation: Some(adapter_doc::RawResult {
            media_type: "application/json".into(),
            byte_length: bytes.len() as u64,
            custody_digest: context::custody_digest(bytes.as_bytes()),
            bytes: bytes.clone(),
            artifacts: vec![],
        }),
    }));
    lines.extend(trailing.iter().cloned());
    let program = fixture::write_hostile(
        &fixture_dir,
        &Hostile {
            lines,
            reads: tool_reads,
            environment: Some(child_env.clone()),
        },
    )
    .unwrap();

    let env = construct(
        &Blueprint::empty()
            .allowing("PATH", &format!("{}:/usr/bin:/bin", bin.display()))
            .needing_command("sh"),
        &CheckSuiteCommands(vec!["sh".into()]),
    );
    let adapter_manifest = fixture::manifest();
    let requested = Requested::none();
    let negotiation = negotiate(&requested, &adapter_manifest.supports);
    let run = supervise(&program, &[], &request, &env).unwrap();
    let stream = read_stream(&run.events, &negotiation.degraded);
    let observed_applied = stream
        .as_ref()
        .map(|r| r.applied.clone())
        .unwrap_or_default();
    let posture = Posture::new(
        &adapter_manifest,
        qualification(&adapter_manifest, &[]),
        &requested,
        &negotiation,
        &observed_applied,
        &env,
    );
    let event_lines: Vec<Vec<u8>> = run
        .events
        .iter()
        .map(|e| serde_json::to_vec(e).unwrap())
        .collect();
    let (adapter_party, provider_party) = fx::fake_parties();
    let recorded = fx::record(
        &home,
        &mut chain,
        &prepared,
        &m,
        &t,
        &selected,
        &Binding::of(&t, adapter_party, provider_party),
        &event_lines,
        &Denial::new(),
        AT,
    )
    .map_err(|e| e.to_string());
    let chain = Chain::open(&home, &target).unwrap().0;

    // Decisions.
    seen.decide("task", &doc::recorded_task(&chain, RUN, 1));
    seen.decide("posture", &posture);
    seen.decide(
        "capability-set",
        &json!({"negotiation": negotiation, "applied": observed_applied}),
    );
    seen.decide("child-environment", &env);
    seen.decide(
        "attempt-outcome",
        &json!({
            "outcome": run.outcome,
            "streamError": run.stream_error.as_ref().map(ToString::to_string),
            "stream": stream.as_ref().map(|r| r.classification).map_err(ToString::to_string),
            "timedOut": run.timed_out,
            "stopped": run.stopped,
            "survivors": run.surviving_processes,
            "refusals": refusals(&run.events).len(),
        }),
    );
    seen.decide(
        "attempt-record",
        &json!({
            "admission": admission,
            "workflow": recorded,
            "returned": match adapter_doc::returned(&run.events) {
                adapter_doc::Returned::NotReturned => json!("not-returned"),
                adapter_doc::Returned::Raw { raw, finding } =>
                    json!({"custody": raw.custody_digest, "finding": finding}),
                adapter_doc::Returned::Duplicate(d) => json!({"duplicate": d.len()}),
            },
            "documentation": doc::project(&chain, RUN, 1),
        }),
    );
    seen.decide("context-delivery", &context::project(&chain, RUN, 1));
    let decoded = DocumentationResult::decode(bytes.as_bytes(), &t);
    seen.decide(
        "review-disposition",
        &json!({
            "sentinel": decoded.as_ref().map(|r| r.review_disposition).map_err(ToString::to_string),
            "reviews": reads(review::recorded_reviews(&chain, RUN, 1)),
            "corrections": reads(review::recorded_corrections(&chain, RUN, 1)),
        }),
    );
    seen.decide(
        "acceptance-result",
        &json!({
            "sentinel": decoded.as_ref().map(|r| r.acceptance_result).map_err(ToString::to_string),
            "workflowState": workflow::current(&chain, RUN, 1).map(|w| w.map(|w| w.state())),
        }),
    );
    seen.decide(
        "application",
        &json!({
            "head": git(&target, &["rev-parse", "HEAD"]),
            "index": git(&target, &["ls-files", "--stage"]),
            "untracked": git(&target, &["ls-files", "--others", "--exclude-standard"]),
        }),
    );
    seen.decide(
        "publication",
        &json!({
            "task": t.publication,
            "refs": git(&target, &["for-each-ref"]),
            "remotes": git(&target, &["remote", "-v"]),
        }),
    );

    // Content kept as data.
    for e in &run.events {
        match e {
            Event::Progress { message } => seen.record("stream-events", message.clone()),
            Event::Refusal { guard, detail } => {
                seen.record("stream-events", format!("{guard}: {detail}"))
            }
            _ => {}
        }
    }
    seen.collection("stream-events");
    if runner.recordings.contains(&"result-document") {
        seen.collection("result-document");
        if let Ok(r) = &decoded {
            for u in &r.unknowns {
                seen.record("result-document", u.question.clone());
            }
        }
    }
    if runner.recordings.contains(&"packet-pages") {
        seen.collection("packet-pages");
        for (_, page) in m.pages() {
            if let Some(b) = read(&page.custody_digest) {
                seen.record_json("packet-pages", &b);
            }
        }
    }
    if runner.recordings.contains(&"skill-bytes") {
        seen.collection("skill-bytes");
        if let Some(b) = read(&prepared.subjects.skill) {
            seen.record("skill-bytes", String::from_utf8_lossy(&b).into_owned());
        }
    }

    // Effects outside the record.
    let mut workspace = tree(&target);
    for i in 0..payload.fragments.len() {
        // The seeded tool output is the case's content, not an effect.
        workspace.remove(&format!("tool-output-{i}.txt"));
    }
    seen.effect("workspace-files", &workspace);
    seen.effect("home-layout", &layout(&home));
    let mut child: Vec<String> = std::fs::read_to_string(&child_env)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect();
    child.sort();
    seen.effect("child-environment", &child);
    seen.effect(
        "processes",
        &std::fs::read_to_string(&trace).unwrap_or_default(),
    );
    seen
}
