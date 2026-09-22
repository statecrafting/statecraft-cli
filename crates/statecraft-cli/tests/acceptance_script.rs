//! The acceptance script's control flow, run locally against a fake provider.
//!
//! Spec 002 section 3.30, "The experiment these rules judge", and its local test
//! route. Every run here executes `scripts/acceptance/managed-session.sh` as a
//! program with this build's binary and `SC_ACCEPTANCE_FAKE_PROVIDER` naming
//! `tests/fixtures/fake-provider.sh`. **No provider is spawned, no approval
//! variable is set to reach the fake path, and no result here is live
//! evidence.** Where an approval variable appears, it is to show the script
//! refusing, and the provider it names does not exist.
//!
//! Every run gets its own `ACC`, containing a space and JSON-sensitive
//! characters, and its own `HOME`, so nothing here reads or writes the
//! operator's home.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository")
}

fn fake() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake-provider.sh")
}

struct Run {
    dir: tempfile::TempDir,
}

impl Run {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("a temporary directory"),
        }
    }

    /// A path with a space, both quote kinds, a backslash and a dollar sign.
    fn acc(&self) -> PathBuf {
        self.dir.path().join("acc \"q\" 'x' \\ $y")
    }

    fn script(&self, stage: &str, env: &[(&str, &str)]) -> Output {
        let mut command = Command::new("sh");
        command
            .arg(repo().join("scripts/acceptance/managed-session.sh"))
            .arg(stage)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", self.dir.path())
            .env("TMPDIR", self.dir.path())
            .env("ACC", self.acc())
            .env("CLI", env!("CARGO_BIN_EXE_statecraft-cli"))
            // A provider that does not exist: nothing here can reach one.
            .env("PROVIDER", "/nonexistent/provider");
        for (k, v) in env {
            command.env(k, v);
        }
        command.output().expect("sh runs")
    }

    fn fake_run(&self, mode: &str, extra: &[(&str, &str)]) -> Output {
        let fake = fake().display().to_string();
        let mut env = vec![
            ("SC_ACCEPTANCE_FAKE_PROVIDER", fake.as_str()),
            ("FAKE_PROVIDER_MODE", mode),
            ("STEP_TIMEOUT", "3"),
        ];
        env.extend_from_slice(extra);
        self.script("permission-experiment", &env)
    }

    fn preflight(&self) {
        let out = self.script("preflight", &[]);
        assert_eq!(code(&out), 0, "preflight: {}", both(&out));
    }

    fn captures(&self) -> PathBuf {
        self.acc().join("synthetic")
    }
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("the script exited")
}

fn both(o: &Output) -> String {
    format!(
        "{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn the_local_route_runs_the_whole_stage_and_says_synthetic() {
    let run = Run::new();
    run.preflight();
    // The preflight's own check: a prose claim, launched and submitted through
    // the product, was refused as prose.
    let prose = std::fs::read_to_string(run.acc().join("steps/08-prose-qualify.out")).unwrap();
    assert!(
        prose.contains("not the harness's structured output"),
        "{prose}"
    );

    let out = run.fake_run("faithful", &[]);
    let text = both(&out);
    assert_eq!(code(&out), 0, "{text}");
    assert!(text.contains("ADMITTED (SYNTHETIC)"), "{text}");
    assert!(text.contains("session 3 of at most 3"), "{text}");
    assert_eq!(
        std::fs::read_to_string(run.acc().join("result")).unwrap(),
        "synthetic-admitted\n"
    );
    let record: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            run.acc()
                .join("project/.statecraft/state/startup/acc-synthetic.json"),
        )
        .expect("the record"),
    )
    .unwrap();
    assert_eq!(
        record["observation"]["evidence"]["refusal"]["launch"]["origin"],
        serde_json::json!("synthetic")
    );
    // The record the product wrote reads back as synthetic and not qualified.
    let read =
        statecraft_home::startup::StartupRecord::read(&run.acc().join("project"), "acc-synthetic")
            .unwrap()
            .unwrap();
    assert!(read.observation.synthetic());
    assert!(!read.qualifies());
    // The paths with quotes, a backslash and a dollar sign reached the product
    // intact: the settings argument names the file the launch wrote.
    let refusal: serde_json::Value =
        serde_json::from_slice(&std::fs::read(run.captures().join("refusal.json")).unwrap())
            .unwrap();
    let settings = refusal["launch"]["settingsPath"].as_str().unwrap();
    assert!(settings.contains("acc \"q\" 'x' \\ $y"), "{settings}");
    assert!(Path::new(settings).is_file());

    // A launch happens once: a second run against the same fixture refuses.
    let again = run.fake_run("faithful", &[]);
    assert_eq!(code(&again), 2, "{}", both(&again));
}

#[test]
fn a_provider_that_did_not_enforce_is_unverified() {
    let run = Run::new();
    run.preflight();
    let out = run.fake_run("ignores-settings", &[]);
    let text = both(&out);
    assert_eq!(code(&out), 1, "{text}");
    assert!(text.contains("UNVERIFIED"), "{text}");
    assert_eq!(
        std::fs::read_to_string(run.acc().join("result")).unwrap(),
        "synthetic-unverified\n"
    );
    let verdict = std::fs::read_to_string(run.acc().join("steps/11-qualify.out")).unwrap();
    assert!(verdict.contains("no structured denial"), "{verdict}");
}

#[test]
fn a_tool_request_without_execution_is_unverified_at_the_admission() {
    let run = Run::new();
    run.preflight();
    let out = run.fake_run("request-only", &[]);
    let text = both(&out);
    assert_eq!(code(&out), 1, "{text}");
    let verdict = std::fs::read_to_string(run.acc().join("steps/11-qualify.out")).unwrap();
    assert!(verdict.contains("the request has no result"), "{verdict}");
}

/// A launch that does not complete stops the stage: no further session.
#[test]
fn an_incomplete_launch_stops_the_stage_before_another_session() {
    for (mode, says) in [
        ("startup-fails", "is empty"),
        ("malformed", "not the harness's structured output"),
        ("conflicting", "more than one init"),
        ("hang", "the deadline ended it"),
        ("signal", "signal 15"),
    ] {
        let run = Run::new();
        run.preflight();
        let out = run.fake_run(mode, &[]);
        let text = both(&out);
        assert_eq!(code(&out), 1, "{mode}: {text}");
        assert!(
            text.contains("no further session was started (1 of 3 launched)"),
            "{mode}: {text}"
        );
        assert!(text.contains(says), "{mode}: {text}");
        assert!(run.captures().join("refusal.json").is_file(), "{mode}");
        assert!(
            !run.captures().join("allowed-command.json").exists(),
            "{mode}: a second session ran"
        );
        assert!(
            !run.acc().join("steps/11-qualify.out").exists(),
            "{mode}: it went on to qualify"
        );
        if mode == "hang" {
            let record: serde_json::Value = serde_json::from_slice(
                &std::fs::read(run.captures().join("refusal.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(
                record["launch"]["process"]["timedOut"],
                serde_json::json!(true)
            );
            assert!(
                record["launch"]["process"]["survivingProcesses"].is_null(),
                "{record}"
            );
        }
    }
}

/// Captures that a session removed are unreadable: a failure, not a verdict.
#[test]
fn captures_that_cannot_be_read_are_a_failure() {
    let run = Run::new();
    run.preflight();
    let target = run.captures().join("refusal.json").display().to_string();
    let out = run.fake_run("tamper", &[("FAKE_TAMPER", &target)]);
    let text = both(&out);
    assert_eq!(code(&out), 4, "{text}");
    assert!(text.contains("could not be read"), "{text}");
}

/// Insufficient evidence of another kind: the version probe answered nothing,
/// and the claim is unverified rather than admitted against no version.
#[test]
fn a_provider_whose_version_cannot_be_read_is_unverified() {
    let run = Run::new();
    run.preflight();
    let out = run.fake_run("version-fails", &[]);
    assert_eq!(code(&out), 1, "{}", both(&out));
    assert!(both(&out).contains("UNVERIFIED"));
}

#[test]
fn both_approvals_refuse_when_unset_and_the_routes_do_not_mix() {
    let run = Run::new();
    run.preflight();

    let out = run.script("permission-experiment", &[]);
    assert_eq!(code(&out), 2, "{}", both(&out));
    assert!(both(&out).contains("APPROVED_PROVIDER_SESSION=yes"));
    assert!(!run.acc().join("launched").exists());
    assert!(
        !run.acc().join("steps/10-refusal.cmd").exists(),
        "a launch was attempted"
    );

    let out = run.script("coexistence", &[]);
    assert_eq!(code(&out), 2, "{}", both(&out));
    assert!(both(&out).contains("APPROVED_REAL_HOME_COEXISTENCE=yes"));

    // The provider approval does not satisfy the coexistence approval.
    let out = run.script("coexistence", &[("APPROVED_PROVIDER_SESSION", "yes")]);
    assert_eq!(code(&out), 2, "{}", both(&out));

    // The local route refuses to run beside the provider approval. The
    // provider named is one that does not exist.
    let fake = fake().display().to_string();
    let out = run.script(
        "permission-experiment",
        &[
            ("SC_ACCEPTANCE_FAKE_PROVIDER", &fake),
            ("APPROVED_PROVIDER_SESSION", "yes"),
        ],
    );
    assert_eq!(code(&out), 2, "{}", both(&out));
    assert!(both(&out).contains("both set"));
    assert!(
        !run.acc().join("steps/10-refusal.cmd").exists(),
        "a launch was attempted"
    );
}

#[test]
fn the_stage_refuses_without_a_preflight_and_clean_removes_only_its_own() {
    let run = Run::new();
    let out = run.fake_run("faithful", &[]);
    assert_eq!(code(&out), 2, "{}", both(&out));
    assert!(both(&out).contains("run the preflight"));

    // Not created by the script: neither preflight nor clean removes it.
    std::fs::create_dir_all(run.acc()).unwrap();
    std::fs::write(run.acc().join("keep"), "x").unwrap();
    assert_eq!(code(&run.script("preflight", &[])), 2);
    assert_eq!(code(&run.script("clean", &[])), 2);
    assert!(run.acc().join("keep").is_file());
    std::fs::remove_dir_all(run.acc()).unwrap();

    run.preflight();
    assert_eq!(code(&run.script("clean", &[])), 0);
    assert!(!run.acc().exists());

    assert_eq!(code(&run.script("bogus", &[])), 3);
}
