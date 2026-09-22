//! Compatibility fixtures for producer capabilities that are specified in
//! spec-spine and **not released**.
//!
//! Spec 006 section 5, 2026-09-22. Each test here drives the built binary
//! against a stub `spec-spine` that emits a shape a spec-spine draft specifies,
//! and asserts what **this** consumer does with it today. None of them claims
//! the producer ships the shape, and none changes a dependency pin: the pin is
//! `=0.20.0`, and adopting a newer one is its own change (`D-06`).

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
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

/// spec-spine spec 102 (`draft`, `implementation: pending`, in no release)
/// adds `status` to each `registry plan --json` ready entry as a read-schema
/// MINOR. This consumer joins `ready` with `registry list` because the plan
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
    git(target.path(), &["config", "user.email", "fixture@example.com"]);
    std::fs::write(target.path().join("x"), "x").unwrap();
    git(target.path(), &["add", "x"]);
    git(target.path(), &["commit", "--quiet", "-m", "base"]);
    let stub = bin.path().join("spec-spine");
    std::fs::write(
        &stub,
        r#"#!/bin/sh
case "$*" in
  --version) echo 'spec-spine 0.20.0' ;;
  check) exit 0 ;;
  'registry plan --json') echo '{"schemaVersion":"1.1.0","ready":[{"id":"002-draft","title":"a draft","status":"approved"},{"id":"003-approved","title":"approved","status":"draft"}],"blocked":[]}' ;;
  'registry list --json') echo '{"items":[{"id":"002-draft","status":"draft","implementation":"pending"},{"id":"003-approved","status":"approved","implementation":"pending"}]}' ;;
  *) exit 3 ;;
esac
"#,
    )
    .unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
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
