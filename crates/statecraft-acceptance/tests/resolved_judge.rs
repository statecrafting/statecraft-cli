//! Spec 029 sections 3.1 and 3.3, the acceptance half: the suite runs the
//! executable it is given and nothing else, and a receipt names the judge.
//!
//! The suite source has no default program, so the only way to build one is
//! with a resolved path; a stub engine at that path answers `verify --json` and
//! records the arguments it ran with.

#![cfg(unix)]

use statecraft_acceptance::independence::{Check, SuiteResult};
use statecraft_acceptance::judged::{Base, Candidate, Judged, Policy};
use statecraft_acceptance::receipt::{MintContext, Receipt, mint};
use statecraft_acceptance::suite::{SpecSpineVerify, SuiteSource};
use statecraft_environment::judge::JudgeRecord;

const PASSING: &str = r#"{
  "exitCode": 0,
  "outcome": "ok",
  "report": { "declared": true, "outcome": "passed", "ran": 1, "total": 1 },
  "summary": "verify: 1 of 1 passed",
  "tool": "spec-spine",
  "verb": "verify"
}"#;

fn judged() -> Judged {
    Judged {
        candidate: Candidate {
            sha: "c".repeat(40),
            work_tree_clean: true,
            head_stable: true,
            dirty_paths: vec![],
        },
        base: Base {
            sha: "b".repeat(40),
        },
        policy: Policy {
            digest: "d".repeat(64),
        },
    }
}

fn context(judge: Option<JudgeRecord>) -> MintContext {
    MintContext {
        repository: "statecraft-cli".into(),
        product_version: "0.0.0".into(),
        spec_spine_version: "0.23.0".into(),
        adapter_version: "1.0.0".into(),
        attempt: "run-1/1".into(),
        authority_paths_touched: vec![],
        judge,
    }
}

fn record() -> JudgeRecord {
    JudgeRecord {
        program: "/repo/.local/spec-spine".into(),
        rule: "repository-local".into(),
        version: Some("0.23.0".into()),
        digest: Some(format!("sha256:{}", "a".repeat(64))),
        passed_over: vec!["passed over /bin/spec-spine (PATH, reports 0.99.0)".into()],
    }
}

#[test]
fn the_suite_runs_the_resolved_program_it_was_given() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let marker = dir.path().join("ran");
    let resolved = dir.path().join("resolved/spec-spine");
    std::fs::create_dir_all(resolved.parent().unwrap()).unwrap();
    statecraft_adapter::fixture::install_script(
        &resolved,
        format!(
            "#!/bin/sh\necho \"$@\" > '{}'\ncat <<'EOF'\n{PASSING}\nEOF\nexit 0\n",
            marker.display()
        ),
        0o755,
    )
    .unwrap();

    let suite = SpecSpineVerify {
        binary: resolved.display().to_string(),
    }
    .run_suite(&workspace, "029-one-resolved-judge");

    assert!(suite.passed(), "{suite:?}");
    assert_eq!(
        std::fs::read_to_string(&marker).unwrap().trim(),
        "verify 029-one-resolved-judge --json"
    );
}

#[test]
fn a_receipt_names_the_judge_its_suite_ran() {
    let suite = SuiteResult::new(vec![Check::ran("spec-spine verify", 0, None)], None);
    let receipt = mint(&judged(), &suite, &context(Some(record())), None).unwrap();
    assert_eq!(receipt.judge, Some(record()));

    let text = serde_json::to_value(&receipt).unwrap();
    let judge = &text["judge"];
    assert_eq!(judge["program"], "/repo/.local/spec-spine");
    assert_eq!(judge["rule"], "repository-local");
    assert_eq!(judge["version"], "0.23.0");
    assert!(judge["digest"].as_str().unwrap().starts_with("sha256:"));
    assert_eq!(judge["passedOver"].as_array().unwrap().len(), 1);

    let back: Receipt = serde_json::from_value(text).unwrap();
    assert_eq!(back, receipt);
}

#[test]
fn a_receipt_with_no_recorded_judge_writes_no_key_and_older_receipts_still_read() {
    let suite = SuiteResult::new(vec![Check::ran("spec-spine verify", 0, None)], None);
    let receipt = mint(&judged(), &suite, &context(None), None).unwrap();
    let text = serde_json::to_value(&receipt).unwrap();
    assert!(text.get("judge").is_none(), "{text}");

    let back: Receipt = serde_json::from_value(text).unwrap();
    assert_eq!(back.judge, None);
}
