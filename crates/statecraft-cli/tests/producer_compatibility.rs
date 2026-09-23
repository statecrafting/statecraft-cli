//! Compatibility fixtures for producer capabilities that are specified in
//! spec-spine and **not released**.
//!
//! Spec 006 section 5, 2026-09-22. Each test here drives the built binary
//! against a stub `spec-spine` that emits a shape a spec-spine draft specifies,
//! and asserts what **this** consumer does with it today. None of them claims
//! the producer ships the shape, and none changes a dependency pin: the pin is
//! `=0.20.0`, and adopting a newer one is its own change (`D-06`).

#![cfg(unix)]

use std::path::Path;
use std::process::Command;

fn git(at: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(at)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}");
}

/// spec-spine spec 102 adds `status` to each `registry plan --json` ready
/// entry as a read-schema MINOR. It is merged and **unreleased**: the stub
/// emits the shape the producer at spec-spine `3b67b63d` (self-reported
/// `0.22.0`, unpublished) was measured emitting on 2026-09-22, ready entries of
/// `id`, `status` and `title`. The pinned `0.20.0` emits `id` and `title` only.
///
/// This test first asserted that a plan `status` contradicting `registry list`
/// was ignored and the list decided. Spec 003 section 3.1.2, recorded before
/// this change, replaced that: the two answers are compared, a contradiction is
/// refused naming both, and neither is chosen. Where they agree, the join still
/// decides, so a draft is still excluded however the plan offers it.
fn stub_target(
    plan: &str,
    list: &str,
) -> (tempfile::TempDir, tempfile::TempDir, tempfile::TempDir) {
    let target = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    git(target.path(), &["init", "--quiet"]);
    git(target.path(), &["config", "user.name", "fixture"]);
    git(
        target.path(),
        &["config", "user.email", "fixture@example.com"],
    );
    std::fs::write(target.path().join("x"), "x").unwrap();
    git(target.path(), &["add", "x"]);
    git(target.path(), &["commit", "--quiet", "-m", "base"]);
    statecraft_adapter::fixture::install_script(
        &bin.path().join("spec-spine"),
        format!(
            "#!/bin/sh\ncase \"$*\" in\n  --version) echo 'spec-spine 0.22.0' ;;\n  check) exit 0 ;;\n  'registry plan --json') echo '{plan}' ;;\n  'registry list --json') echo '{list}' ;;\n  *) exit 3 ;;\nesac\n"
        ),
        0o755,
    )
    .unwrap();
    (target, home, bin)
}

fn work_list(target: &Path, home: &Path, bin: &Path) -> std::process::Output {
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", home)
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .output()
            .unwrap()
    };
    let root = target.to_str().unwrap();
    assert!(run(&["project", "register", root]).status.code().unwrap() <= 1);
    run(&["work", "list", root, "--json"])
}

const LIST: &str = r#"{"items":[{"id":"002-draft","status":"draft","implementation":"pending"},{"id":"003-approved","status":"approved","implementation":"pending"}]}"#;

#[test]
fn a_ready_entry_whose_status_contradicts_the_list_is_refused_naming_both() {
    let (target, home, bin) = stub_target(
        r#"{"blocked":[],"ready":[{"id":"002-draft","status":"approved","title":"a draft"},{"id":"003-approved","status":"draft","title":"approved"}],"schemaVersion":"0.6.0"}"#,
        LIST,
    );
    let out = work_list(target.path(), home.path(), bin.path());
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("contradicts itself about 002-draft"),
        "{text}"
    );
    assert!(
        text.contains("`approved`") && text.contains("`draft`"),
        "{text}"
    );
}

#[test]
fn a_ready_entry_carrying_an_agreeing_status_is_accepted_and_the_join_still_decides() {
    let (target, home, bin) = stub_target(
        r#"{"blocked":[],"ready":[{"id":"002-draft","status":"draft","title":"a draft"},{"id":"003-approved","status":"approved","title":"approved"}],"schemaVersion":"0.6.0"}"#,
        LIST,
    );
    let out = work_list(target.path(), home.path(), bin.path());
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let value = &v["value"];
    // The draft is offered as ready, and the policy, not the plan, excludes it.
    let excluded: Vec<&str> = value["excluded"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(excluded, ["002-draft"], "{v}");
    let eligible = value["eligible"].as_array().unwrap();
    assert_eq!(eligible.len(), 1, "{v}");
    assert_eq!(eligible[0]["id"], "003-approved");
    assert_eq!(eligible[0]["status"], "approved");
    assert_eq!(
        eligible[0]["statusFromField"],
        "registry list --json: items[].status, agreeing with registry plan --json: ready[].status",
        "{v}"
    );
}
