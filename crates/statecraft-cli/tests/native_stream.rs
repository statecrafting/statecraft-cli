//! Spec 008's provider binding through the real CLI and run record. Discovery
//! is a test double; provider output is the committed redacted native stream.
//! No provider executable or credential is used.

#![cfg(unix)]

use std::path::Path;
use std::process::{Command, Output};

fn executable(path: &Path, script: &str) {
    // Staged and copied, never written in place: a script this process wrote
    // and then exec'd is the `ETXTBSY` race spec 004 records.
    statecraft_adapter::fixture::install_script(path, script, 0o755).unwrap();
}

fn git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(path)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

fn native_run(
    fixture: &str,
    expected: &str,
    claim: &str,
    refusals: u32,
    subtype: Option<&str>,
    missing_init: bool,
) {
    let target = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    git(target.path(), &["init", "--quiet"]);
    git(target.path(), &["config", "user.name", "fixture"]);
    git(
        target.path(),
        &["config", "user.email", "fixture@example.com"],
    );
    std::fs::write(target.path().join("source.txt"), "base").unwrap();
    git(target.path(), &["add", "source.txt"]);
    git(target.path(), &["commit", "--quiet", "-m", "fixture base"]);

    // Synthetic scheduler input keeps this regression independent of discovery.
    // It does not create or ratify a spec in the governed repository.
    executable(
        &bin.path().join("spec-spine"),
        r#"#!/bin/sh
case "$*" in
  --version) echo 'spec-spine 0.20.0' ;;
  check) exit 0 ;;
  'registry plan --json') echo '{"ready":[{"id":"replay","title":"recorded stream"}]}' ;;
  'registry list --json') echo '{"items":[{"id":"replay","status":"approved","implementation":"pending"}]}' ;;
  *) exit 3 ;;
esac
"#,
    );
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../statecraft-adapter-claude-code/testdata/stream")
        .join(fixture);
    let mut native = std::fs::read_to_string(&fixture_path).unwrap();
    if let Some(subtype) = subtype {
        // An explicitly synthetic perturbation of the recorded terminal state.
        let mut lines: Vec<String> = native.lines().map(str::to_string).collect();
        let mut result: serde_json::Value = serde_json::from_str(lines.last().unwrap()).unwrap();
        result["subtype"] = subtype.into();
        *lines.last_mut().unwrap() = result.to_string();
        native = lines.join("\n");
    }
    if missing_init {
        // Keep only the recorded result, without inventing initialization.
        native = native.lines().last().unwrap().to_string();
    }
    std::fs::write(bin.path().join("native.jsonl"), native).unwrap();
    // Spec 002 section 3.27: a managed run receives the floor as the exact
    // payload bytes an observation is bound to. The child compares them byte
    // for byte, so a document that parsed the same and was formatted
    // differently would fail here.
    std::fs::write(
        bin.path().join("expected-settings"),
        statecraft_home::session::payload_json(),
    )
    .unwrap();
    executable(
        &bin.path().join("claude"),
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.267'; exit 0; fi
[ "$#" = 6 ] || exit 3
[ "$1 $2 $3 $4" = '--print --output-format stream-json --verbose' ] || exit 3
[ "$5" = --settings ] || exit 3
/usr/bin/cmp -s "$6" "$(dirname "$0")/expected-settings" || exit 3
/bin/cat "$6" > child-settings
printf '%s' "$6" > child-settings-path
[ "$USER" = fixture-operator ] || exit 3
[ "${HOME+x}" != x ] || exit 3
/bin/cat > child-prompt
pwd > child-cwd
/bin/cat "$(dirname "$0")/native.jsonl"
case "$(/bin/cat "$(dirname "$0")/native.jsonl")" in
  *error_max_turns*) exit 1 ;;
  *api_error*) exit 1 ;;
esac
"#,
    );
    let path = format!("{}:/usr/bin:/bin", bin.path().display());
    let run = |args: &[&str]| -> Output {
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
    let output = run(&["run", root, "replay", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(if expected == "completed" { 0 } else { 1 }),
        "{output:?}"
    );
    let answer: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(answer["value"]["outcome"], expected);
    assert_eq!(answer["value"]["adapterClaimed"], claim);
    assert_eq!(answer["value"]["refusals"], refusals);
    // Spec 006 section 3.4 permits additive fields. The original seven stay,
    // the run exposes its recorded posture alongside them, and section 3.11.3
    // adds its startup evidence.
    assert_eq!(answer["value"].as_object().unwrap().len(), 9);
    // This fixture holds no manifest, so it is not a managed session and
    // records no startup evidence, and the answer says so rather than
    // claiming a record (spec 002 section 3.31).
    assert_eq!(answer["value"]["startup"]["managed"], false);
    assert_eq!(
        answer["value"]["startup"]["record"],
        serde_json::Value::Null
    );
    assert_eq!(
        answer["value"]["posture"]["value"]["qualification"],
        "unqualified"
    );

    let workspace = Path::new(answer["value"]["workspaceRetained"].as_str().unwrap());
    let cwd = std::fs::read_to_string(workspace.join("child-cwd")).unwrap();
    assert_eq!(
        Path::new(cwd.trim()).canonicalize().unwrap(),
        workspace.canonicalize().unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(workspace.join("child-prompt")).unwrap(),
        "Implement replay in this workspace."
    );
    assert!(!target.path().join("child-cwd").exists());
    assert_eq!(
        std::fs::read_to_string(workspace.join("child-settings")).unwrap(),
        statecraft_home::session::payload_json(),
        "the run did not deliver the managed-session payload"
    );
    let settings_path = std::fs::read_to_string(workspace.join("child-settings-path")).unwrap();
    assert!(!Path::new(&settings_path).exists());
    assert!(!Path::new(&settings_path).starts_with(workspace));

    let (chain, _) = statecraft_run::record::Chain::open(home.path(), target.path()).unwrap();
    let entries = chain.entries();
    let outcome = entries.iter().find(|e| e.subject == "attempt").unwrap();
    assert_eq!(outcome.detail["outcome"], expected);
    assert_eq!(outcome.detail["refusals"], refusals);
    assert_eq!(outcome.detail["adapterClaimed"], claim);
    assert_eq!(
        statecraft_run::session::runs(&chain)[0].attempts[0]
            .outcome
            .unwrap()
            .word(),
        expected
    );
    // What the run delivered and what it stood on, recorded with the attempt.
    assert_eq!(
        outcome.detail["payload"]["digest"],
        serde_json::json!(statecraft_home::startup::payload_identity())
    );
    assert_eq!(outcome.detail["harnessStanding"], "unrequired");
    let posture = &outcome.detail["posture"];
    assert_eq!(*posture, answer["value"]["posture"]["value"]);
    assert_eq!(posture["qualification"], "unqualified");
    assert_eq!(posture["applied"], outcome.detail["applied"]);
    let evidence = &outcome.detail["execution"];
    if missing_init {
        assert_eq!(
            evidence["streamError"],
            statecraft_adapter::StreamError::NoInit.to_string()
        );
        assert_eq!(
            evidence["events"].as_array().unwrap().len(),
            refusals as usize
        );
    } else if subtype.is_some() {
        assert!(
            evidence["streamError"]
                .as_str()
                .unwrap()
                .contains("not in spec 004")
        );
    } else {
        assert!(evidence["streamError"].is_null());
    }
    if !missing_init {
        assert!(evidence["events"].as_array().unwrap().len() > 2);
    }
    assert!(evidence["providerTerminal"]["num_turns"].as_u64().unwrap() > 0);
    let accounting = entries.iter().find(|e| e.subject == "refusals").unwrap();
    assert_eq!(accounting.detail["count"], refusals);
    if refusals > 0 {
        let denial: serde_json::Value =
            serde_json::from_str(accounting.detail["sample"][0]["detail"].as_str().unwrap())
                .unwrap();
        assert_eq!(
            denial,
            evidence["providerTerminal"]["permission_denials"][0]
        );
        if !missing_init {
            assert!(
                evidence["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["message"] == "system/permission_denied")
            );
        }
    }
    if expected != "completed" {
        let accepted = run(&["accept", root, "replay", "--json"]);
        assert_eq!(accepted.status.code(), Some(1), "{accepted:?}");
        let answer: serde_json::Value = serde_json::from_slice(&accepted.stdout).unwrap();
        assert_eq!(answer["value"]["reason"], format!("attempt-{expected}"));
    }
}

#[test]
fn run_maps_recorded_success_and_keeps_the_workspace_cwd() {
    native_run("success.jsonl", "completed", "completed", 0, None, false);
}

#[test]
fn run_counts_recorded_denial_once_and_accept_does_not_run() {
    native_run("denied.jsonl", "refused", "completed", 1, None, false);
}

#[test]
fn run_maps_recorded_turn_cap_to_interrupted_and_accept_does_not_run() {
    native_run(
        "max-turns.jsonl",
        "interrupted",
        "interrupted",
        0,
        None,
        false,
    );
}

#[test]
fn run_maps_a_recorded_api_error_to_interrupted_and_never_to_its_success_subtype() {
    // The recorded terminal calls itself a success and flags an error. The run
    // records `interrupted`, keeps the provider's `failed` claim beside it, and
    // `accept` refuses for `attempt-interrupted`.
    native_run("api-error.jsonl", "interrupted", "failed", 0, None, false);
}

#[test]
fn run_retains_an_unmapped_completed_claim_beside_interrupted_and_its_diagnostic() {
    native_run(
        "success.jsonl",
        "interrupted",
        "completed",
        0,
        Some("unmeasured"),
        false,
    );
}

#[test]
fn run_without_init_persists_denial_accounting_claim_and_diagnostic() {
    native_run("denied.jsonl", "refused", "completed", 1, None, true);
}

#[test]
fn run_without_init_or_denials_stays_interrupted() {
    native_run("success.jsonl", "interrupted", "completed", 0, None, true);
}

/// Spec 002 section 3.25 through `run`: a committed requirement whose content
/// cannot be established stops the run before an attempt exists, and the
/// provider is never launched. Missing first, then corrupt.
#[test]
fn run_refuses_a_required_harness_it_cannot_establish_and_records_nothing() {
    let target = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    git(target.path(), &["init", "--quiet"]);
    git(target.path(), &["config", "user.name", "fixture"]);
    git(
        target.path(),
        &["config", "user.email", "fixture@example.com"],
    );
    std::fs::write(target.path().join("source.txt"), "base").unwrap();
    git(target.path(), &["add", "source.txt"]);
    git(target.path(), &["commit", "--quiet", "-m", "fixture base"]);
    executable(
        &bin.path().join("spec-spine"),
        r#"#!/bin/sh
case "$*" in
  --version) echo 'spec-spine 0.20.0' ;;
  check) exit 0 ;;
  'registry plan --json') echo '{"ready":[{"id":"replay","title":"recorded stream"}]}' ;;
  'registry list --json') echo '{"items":[{"id":"replay","status":"approved","implementation":"pending"}]}' ;;
  *) exit 3 ;;
esac
"#,
    );
    // A provider that records that it was launched, which it must not be.
    executable(
        &bin.path().join("claude"),
        "#!/bin/sh\nif [ \"$1\" = --version ]; then echo '2.1.267'; exit 0; fi\n: > \"$(dirname \"$0\")/launched\"\nexit 3\n",
    );
    let path = format!("{}:/usr/bin:/bin", bin.path().display());
    let run = |args: &[&str]| -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", home.path())
            .env("PATH", &path)
            .env("USER", "fixture-operator")
            .output()
            .unwrap()
    };
    let root = target.path().to_str().unwrap();
    assert!(run(&["project", "register", root]).status.code().unwrap() <= 1);
    assert_eq!(run(&["project", "arm", root]).status.code(), Some(0));

    // A manifest committing a requirement that names a revision this home
    // does not hold. Registration writes no manifest, so one is written the way
    // initialization would, carrying only what this test needs.
    let mut manifest =
        statecraft_environment::manifest::Manifest::new(statecraft_environment::manifest::Pins {
            product: "0.1.0".into(),
            spec_spine: "0.20.0".into(),
            adapters: Default::default(),
        });
    manifest.project.requirements.insert(
        statecraft_home::required::REQUIREMENT_KEY.to_string(),
        "a".repeat(64),
    );
    manifest.write(target.path()).unwrap();

    let refused = run(&["run", root, "replay", "--json"]);
    assert_eq!(refused.status.code(), Some(2), "{refused:?}");
    assert!(
        String::from_utf8_lossy(&refused.stdout).contains("harness upgrade"),
        "{refused:?}"
    );
    assert!(
        !bin.path().join("launched").exists(),
        "the provider was launched"
    );
    let (chain, _) = statecraft_run::record::Chain::open(home.path(), target.path()).unwrap();
    assert!(
        statecraft_run::session::runs(&chain).is_empty(),
        "a refused run appended an attempt"
    );

    // Install and commit the shipped revision, then damage one installed file.
    assert_eq!(run(&["harness", "upgrade", root]).status.code(), Some(0));
    let revisions = home.path().join("harness");
    let revision = std::fs::read_dir(&revisions)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let hook = revision.join("hooks/statecraft-session-start.sh");
    let mut bytes = std::fs::read(&hook).unwrap();
    bytes.extend_from_slice(b"\n# changed\n");
    std::fs::write(&hook, bytes).unwrap();

    let refused = run(&["run", root, "replay", "--json"]);
    assert_eq!(refused.status.code(), Some(2), "{refused:?}");
    assert!(
        !bin.path().join("launched").exists(),
        "the provider was launched"
    );
    let (chain, _) = statecraft_run::record::Chain::open(home.path(), target.path()).unwrap();
    assert!(statecraft_run::session::runs(&chain).is_empty());
}
