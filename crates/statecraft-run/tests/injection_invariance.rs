//! Spec 025's product-invariance suite, run half: every corpus case on the
//! `context-packet` and `repository-text` surfaces is seeded with its payload
//! and with its benign twin, and the intent, the recorded manifest, policy and
//! task, the attempt's record and the prepared workspace are the twin's
//! except where the case declares (sections 2, 3.1 and 3.4).
//!
//! The corpus and the comparison are the adapter crate's
//! (`tests/support/injection.rs`), included by path. No provider, network,
//! credential or spend is involved.
#![cfg(unix)]

#[path = "../../statecraft-adapter/tests/support/injection.rs"]
mod injection;

use serde_json::json;

use statecraft_environment::time::FixedClock;
use statecraft_run::context::{self, ContextPolicy, Need};
use statecraft_run::documentation::workflow::{self, AdapterRef};
use statecraft_run::documentation::{self as doc, NotRequired, Skill};
use statecraft_run::record::Chain;
use statecraft_run::session::{self, IntentDetail};
use statecraft_run::workspace;

use injection::fixtures::{RUN, git, manifest, pages, repo, task};
use injection::{Case, Framing, Observed, Runner, Side, Status, load, ran, run_suite};

fn policy() -> ContextPolicy {
    ContextPolicy {
        schema: "statecraft/context-acceptance-policy/1".into(),
        delivery: Need::Required,
        integrity: Need::Required,
        closure: Need::Required,
        freshness: Need::Required,
        citations: Need::Required,
        evidence: Need::Required,
        permitted_absences: vec![],
    }
}

fn adapter() -> AdapterRef {
    AdapterRef {
        name: "fixed-export-fake".into(),
        version: "1".into(),
        capabilities: vec!["deterministic-fake".into()],
    }
}

/// The attempt's records as the chain holds them, in order.
fn entries(chain: &Chain) -> Vec<statecraft_run::record::Entry> {
    chain
        .entries()
        .into_iter()
        .filter(|e| e.run_id == RUN)
        .collect()
}

/// Seed `case` on `side` and observe the run crate's decisions. Framing is
/// the adapter's concern and changes nothing this crate decides, so both
/// framings run the same path here.
fn observe(runner: &Runner, case: &Case, side: Side, _framing: Framing) -> Observed {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let target = root.path().join("target");
    std::fs::create_dir_all(&home).unwrap();
    let mut seen = Observed::rooted_at(&[root.path()]);
    let payload = case.bytes(side);

    // Repository text: each fragment in its own committed file.
    let files: Vec<(String, String)> = if runner.surface == "repository-text" {
        payload
            .fragments
            .iter()
            .enumerate()
            .map(|(i, f)| (format!("docs/notes-{i}.md"), f.clone()))
            .collect()
    } else {
        vec![]
    };
    repo(&target, &files);
    let head = git(&target, &["rev-parse", "HEAD"]);

    let packet = pages(case, side);
    let m = manifest(&packet);
    for p in &packet {
        context::retain(&home, p).unwrap();
    }
    let t = task(&m, &head, Skill::NotRequired(NotRequired::NotRequired));
    let p = policy();
    let (mut chain, _) = Chain::open(&home, &target).unwrap();
    let began = session::begin_with(
        &mut chain,
        &target,
        RUN,
        "main",
        &FixedClock(1_790_000_000),
        &IntentDetail {
            context_manifest: Some(&m),
            context_policy: Some(&p),
            documentation_task: Some(&t),
            ..IntentDetail::default()
        },
    )
    .map(|s| json!({"run": s.run_id, "attempt": s.attempt}))
    .map_err(|e| e.to_string());
    let chain = Chain::open(&home, &target).unwrap().0;

    let recorded = doc::recorded_task(&chain, RUN, 1);
    seen.decide(
        "task",
        &json!({
            "recorded": recorded,
            "manifestCheck": t.check_manifest(&m).map_err(|e| e.to_string()),
            "candidateCheck": t.check_candidate(&head).map_err(|e| e.to_string()),
        }),
    );
    let subjects = workflow::subjects(&home, &m, &t, None, &adapter()).map_err(|e| e.to_string());
    if runner.surface == "context-packet" {
        seen.decide(
            "context-delivery",
            &json!({
                "eligibility": m.eligibility().map_err(|e| format!("{e:?}")),
                "recordedManifest": context::recorded_manifest(&chain, RUN, 1),
                "recordedPolicy": context::recorded_policy(&chain, RUN, 1),
                "projection": context::project(&chain, RUN, 1),
            }),
        );
        seen.decide(
            "attempt-record",
            &json!({"began": began, "entries": entries(&chain), "subjects": subjects}),
        );
        seen.collection("packet-pages");
        for (_, page) in m.pages() {
            if let Ok(Some(b)) = context::read_retained(&home, &page.custody_digest) {
                seen.record_json("packet-pages", &b);
            }
        }
    } else {
        // Spec 003 section 3.2: the workspace is prepared from the recorded
        // base, and the operator's checkout is never edited.
        let prepared = workspace::prepare(&target, RUN, &head).map_err(|e| e.to_string());
        seen.decide(
            "attempt-record",
            &json!({"began": began, "entries": entries(&chain), "subjects": subjects}),
        );
        let path = workspace::workspace_path(&target, RUN);
        seen.decide(
            "application",
            &json!({
                "workspace": prepared,
                "workspaceHead": git(&path, &["rev-parse", "HEAD"]),
                "workspaceStatus": git(&path, &["status", "--porcelain"]),
                "checkoutHead": git(&target, &["rev-parse", "HEAD"]),
                "checkoutStatus": git(&target, &["status", "--porcelain", "--ignored"]),
                "branches": git(&target, &["for-each-ref", "--format=%(refname)"]),
            }),
        );
        seen.collection("repository-files");
        for (name, _) in &files {
            let text = std::fs::read_to_string(path.join(name)).unwrap_or_default();
            seen.record("repository-files", text);
        }
    }
    seen
}

#[test]
fn every_run_surface_case_leaves_every_decision_equal_to_its_twin() {
    let corpus = load();
    let statuses = run_suite(&corpus, "run", observe);
    let failed: Vec<String> = statuses
        .iter()
        .filter_map(|(id, s)| match s {
            Status::Failed(f) => Some(format!("{id}:\n    {}", f.join("\n    "))),
            _ => None,
        })
        .collect();
    assert!(failed.is_empty(), "{}\n{}", ran(&corpus), failed.join("\n"));
    let passed = statuses.values().filter(|s| **s == Status::Passed).count();
    assert_eq!(passed, 16, "two surfaces of eight classes each");
}

// Section 3.8's first row: a packet page that asks for `push` leaves the task
// and its operations exactly the twin's; the request stays packet data.
#[test]
fn a_packet_page_asking_for_push_leaves_the_task_operations_unchanged() {
    let corpus = load();
    let case = corpus
        .cases
        .iter()
        .find(|c| c.case_id == "context-packet/authority-expansion")
        .unwrap();
    assert!(case.payload.assembled().contains("`push`"));
    let runner = injection::RUNNERS
        .iter()
        .find(|r| r.harness == "run" && r.surface == "context-packet")
        .unwrap();
    let hostile = observe(runner, case, Side::Hostile, Framing::Intact);
    let twin = observe(runner, case, Side::Twin, Framing::Intact);
    let operations = |o: &Observed| o.decisions["task"]["recorded"]["Ok"]["operations"].clone();
    assert_eq!(operations(&hostile), operations(&twin));
    let ops = operations(&hostile);
    assert!(
        !ops["allowed"].as_array().unwrap().contains(&json!("push")),
        "{ops}"
    );
    assert!(
        ops["forbidden"]
            .as_array()
            .unwrap()
            .contains(&json!("push")),
        "{ops}"
    );
    assert!(
        hostile.recordings["packet-pages"]
            .iter()
            .any(|p| p.contains("`push`")),
        "the request is kept as packet data"
    );
}
