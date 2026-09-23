//! Spec 008 qualification labels through disposable CLI subprocesses and the
//! persisted fold. All qualification evidence here is synthetic fixture data.
//! No live provider, real qualification record, or credential is used.

#![cfg(unix)]

use serde_json::Value;
use statecraft_adapter_claude_code::qualification::{PairedRecord, record};
use std::path::Path;
use std::process::Command;

fn executable(path: &Path, script: &str) {
    // Staged and copied, never written in place: a script this process wrote
    // and then exec'd is the `ETXTBSY` race spec 004 records.
    statecraft_adapter::fixture::install_script(path, script, 0o755).unwrap();
}

fn check(records: Option<Vec<PairedRecord>>, version: &str, expected: &str) {
    let target = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    for args in [
        vec!["init", "--quiet"],
        vec!["config", "user.name", "fixture"],
        vec!["config", "user.email", "fixture@example.com"],
        vec!["commit", "--quiet", "--allow-empty", "-m", "fixture base"],
    ] {
        assert!(
            Command::new("git")
                .current_dir(target.path())
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }
    executable(
        &bin.path().join("spec-spine"),
        r#"#!/bin/sh
case "$*" in
  --version) echo 'spec-spine 0.20.0' ;;
  check|'check --help') exit 0 ;;
  'registry plan --json') echo '{"ready":[{"id":"fixture","title":"qualification fixture"}]}' ;;
  'registry list --json') echo '{"items":[{"id":"fixture","status":"approved","implementation":"pending"}]}' ;;
  'verify '*' --plan --json') printf '{"exitCode":0,"ok":true,"report":{"commands":[],"skipped":[],"specId":"%s"},"schemaVersion":"0.6.0","verb":"verify"}' $2 ;;
  *) exit 3 ;;
esac
"#,
    );
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../statecraft-adapter-claude-code/testdata/stream/success.jsonl"),
        bin.path().join("stream.jsonl"),
    )
    .unwrap();
    std::fs::write(bin.path().join("version"), version).unwrap();
    // Spec 002 section 3.27: the run delivers the managed-session payload, byte
    // for byte, which the child checks before it answers.
    std::fs::write(
        bin.path().join("expected-settings"),
        statecraft_home::session::payload_json(),
    )
    .unwrap();
    executable(
        &bin.path().join("claude"),
        r#"#!/bin/sh
if [ "$1" = --version ]; then /bin/cat "$(dirname "$0")/version"; exit 0; fi
[ "$#" = 6 ] || exit 3
[ "$1 $2 $3 $4" = '--print --output-format stream-json --verbose' ] || exit 3
[ "$5" = --settings ] || exit 3
/usr/bin/cmp -s "$6" "$(dirname "$0")/expected-settings" || exit 3
[ "$USER" = fixture-operator ] || exit 3
[ "${HOME+x}" != x ] || exit 3
/bin/cat > child-prompt
/bin/cat "$(dirname "$0")/stream.jsonl"
"#,
    );
    let qualification_path = home.path().join("qualifications.json");
    if let Some(records) = records {
        std::fs::write(&qualification_path, serde_json::to_vec(&records).unwrap()).unwrap();
    }
    let path = format!("{}:/usr/bin:/bin", bin.path().display());
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", home.path())
            .env("PATH", &path)
            // Spec 004 section 3.14: the account name is carried to the child
            // with this process's own value, and `HOME` still is not.
            .env("USER", "fixture-operator")
            .output()
            .unwrap()
    };
    let root = target.path().to_str().unwrap();
    let registered = run(&["project", "register", root]);
    assert!(registered.status.code().unwrap() <= 1, "{registered:?}");
    // Arming is the consent to being driven (spec 002 section 3.1), and `run`
    // refuses without it. A fixture that drives a target states it.
    let armed = run(&["project", "arm", root]);
    assert_eq!(armed.status.code(), Some(0), "{armed:?}");
    let output = run(&["run", root, "fixture", "--json"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let answer: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(answer["value"]["outcome"], "completed");
    assert_eq!(
        answer["value"]["posture"]["value"]["qualification"], expected,
        "missing or incorrect adapter qualification in successful run: {answer}"
    );
    assert_eq!(
        answer["value"]["posture"]["from_record"],
        "attempt#fixture/1"
    );
    let manifest = statecraft_adapter_claude_code::manifest();
    let posture = &answer["value"]["posture"]["value"];
    assert_eq!(posture["adapter"], manifest.adapter);
    assert_eq!(posture["adapter_version"], manifest.version);
    assert_eq!(
        posture["required"],
        serde_json::json!(["structured-refusals"])
    );
    assert!(!posture["residuals"].as_array().unwrap().is_empty());
    // Spec 004 section 3.17: a target with no declaration file declares
    // nothing, and a suite that says it is empty requires nothing.
    assert_eq!(posture["coverage"]["verdict"], "direct", "{posture}");
    assert_eq!(posture["coverage"]["declaration"]["state"], "absent");

    let human = run(&["run", root, "fixture"]);
    assert_eq!(human.status.code(), Some(0), "{human:?}");
    let label = format!(
        "adapter {} {} ({expected})",
        manifest.adapter, manifest.version
    );
    assert!(
        String::from_utf8_lossy(&human.stdout).contains(&label),
        "{human:?}"
    );

    let (chain, _) = statecraft_run::record::Chain::open(home.path(), target.path()).unwrap();
    let entries = chain.entries();
    let outcomes: Vec<_> = entries.iter().filter(|e| e.subject == "attempt").collect();
    assert_eq!(outcomes.len(), 2);
    for entry in outcomes {
        assert_eq!(entry.detail["posture"], *posture);
    }
    assert_eq!(statecraft_run::session::runs(&chain)[0].attempts.len(), 2);
    let before = std::fs::read(statecraft_run::record::chain_path(
        home.path(),
        target.path(),
    ))
    .unwrap();

    // Inspection must survive changed evidence and missing executables. It
    // reports what ran, not what could be qualified today.
    std::fs::write(&qualification_path, b"[]").unwrap();
    std::fs::remove_file(bin.path().join("claude")).unwrap();
    std::fs::remove_file(bin.path().join("spec-spine")).unwrap();
    let shown = run(&["run", "show", root, "fixture", "--json"]);
    assert_eq!(shown.status.code(), Some(0), "{shown:?}");
    let shown: Value = serde_json::from_slice(&shown.stdout).unwrap();
    assert_eq!(shown["value"]["posture"]["value"], *posture);
    assert_eq!(
        shown["value"]["posture"]["from_record"],
        "attempt#fixture/2"
    );
    let shown = run(&["run", "show", root, "fixture"]);
    assert_eq!(shown.status.code(), Some(0), "{shown:?}");
    assert!(
        String::from_utf8_lossy(&shown.stdout).contains(&label),
        "{shown:?}"
    );
    assert_eq!(
        std::fs::read(statecraft_run::record::chain_path(
            home.path(),
            target.path()
        ))
        .unwrap(),
        before
    );
}

fn fixture_record() -> PairedRecord {
    record("2.1.267", "synthetic-fixture-only", "2026-09-17T00:00:00Z")
}

#[test]
fn absent_record_runs_visibly_unqualified() {
    check(None, "2.1.267", "unqualified");
}

#[test]
fn another_provider_version_cannot_qualify_the_run() {
    check(Some(vec![fixture_record()]), "2.1.268", "unqualified");
}

#[test]
fn another_adapter_version_cannot_qualify_the_run() {
    let mut record = fixture_record();
    record.adapter.binary_version = "different-build".into();
    check(Some(vec![record]), "2.1.267", "unqualified");
}

#[test]
fn another_adapter_cannot_qualify_the_run() {
    let mut record = fixture_record();
    record.adapter.adapter = "another-adapter".into();
    check(Some(vec![record]), "2.1.267", "unqualified");
}

#[test]
fn unobserved_provider_version_cannot_qualify_the_run() {
    check(Some(vec![fixture_record()]), "unreadable", "unqualified");
}

#[test]
fn matching_fixture_evidence_qualifies_only_the_fixture_pair() {
    check(Some(vec![fixture_record()]), "2.1.267", "qualified");
}
