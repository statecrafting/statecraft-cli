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
/// emits, key for key, the shape the producer at spec-spine `45becbb`
/// (`0.22.0`, unpublished) was measured emitting on 2026-09-22, plan
/// `schemaVersion` `0.3.0` with ready entries of `id`, `status` and `title`.
/// The pinned `0.20.0` emits `0.1.0` with `id` and `title` only. This consumer joins `ready` with `registry list` because the plan
/// carries no status (spec 006 section 3.8), and spec 003 section 3.1.1 keeps
/// approval its own rule.
///
/// The fixture makes the new field **disagree** with `registry list`, so the
/// test can tell which one was read: an additive field must not be refused as
/// unknown, and must not silently replace the status the join reads.
#[test]
fn a_ready_entry_carrying_status_is_accepted_and_the_join_still_decides() {
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
    let stub = bin.path().join("spec-spine");
    statecraft_adapter::fixture::install_script(
        &stub,
        r#"#!/bin/sh
case "$*" in
  --version) echo 'spec-spine 0.20.0' ;;
  check) exit 0 ;;
  'registry plan --json') echo '{"blocked":[],"notSchedulable":[],"ready":[{"id":"002-draft","status":"approved","title":"a draft"},{"id":"003-approved","status":"draft","title":"approved"}],"schemaVersion":"0.3.0"}' ;;
  'registry list --json') echo '{"items":[{"id":"002-draft","status":"draft","implementation":"pending"},{"id":"003-approved","status":"approved","implementation":"pending"}]}' ;;
  *) exit 3 ;;
esac
"#,
        0o755,
    )
    .unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", home.path())
            .env("PATH", format!("{}:/usr/bin:/bin", bin.path().display()))
            .output()
            .unwrap()
    };
    let root = target.path().to_str().unwrap();
    assert!(run(&["project", "register", root]).status.code().unwrap() <= 1);
    let out = run(&["work", "list", root, "--json"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let value = &v["value"];

    // The draft is excluded on the lifecycle report's word, although the plan
    // entry says `approved`.
    let excluded: Vec<&str> = value["excluded"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(excluded, ["002-draft"], "{v}");
    // The approved spec is eligible on the lifecycle report's word, although
    // the plan entry says `draft`, and the row still names both sources.
    let eligible = value["eligible"].as_array().unwrap();
    assert_eq!(eligible.len(), 1, "{v}");
    assert_eq!(eligible[0]["id"], "003-approved");
    assert_eq!(eligible[0]["status"], "approved");
    assert!(
        eligible[0]["statusFromField"]
            .as_str()
            .unwrap()
            .contains("registry list"),
        "{v}"
    );
}
