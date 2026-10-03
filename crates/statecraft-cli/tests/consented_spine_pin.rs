//! Spec 033 through the binary: exact consent, exit classes, and real judge use.
#![cfg(unix)]
use serde_json::Value;
use statecraft_home::producer::{self, Library};
use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};
struct Fixture {
    dir: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        let f = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir(f.root()).unwrap();
        let starter = producer::produce(&Library).unwrap();
        let pin = starter
            .governance
            .iter()
            .find(|p| p.rel_path == "spec-spine.toml")
            .unwrap();
        std::fs::write(
            f.root().join("spec-spine.toml"),
            pin.contents
                .replace(&format!("={}", producer::PRODUCER_VERSION), "=0.1.0"),
        )
        .unwrap();
        for args in [
            vec!["init", "--quiet"],
            vec![
                "-c",
                "commit.gpgsign=false",
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "base",
            ],
        ] {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(f.root())
                    .status()
                    .unwrap()
                    .success()
            );
        }
        f
    }
    fn root(&self) -> PathBuf {
        self.dir.path().join("project")
    }
    fn bin() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join(".bin/spec-spine")
    }
    fn cli(&self, verb: &str, flags: &[&str], json: bool, judge: &Path) -> Output {
        let mut c = Command::new(env!("CARGO_BIN_EXE_statecraft-cli"));
        c.args(["init", verb]).arg(self.root()).args(flags);
        if json {
            c.arg("--json");
        }
        c.env("STATECRAFT_HOME", self.dir.path().join("home"))
            .env("STATECRAFT_NATIVE_ROOT", self.dir.path().join("native"))
            .env("STATECRAFT_SPEC_SPINE", judge)
            .env_remove("STATECRAFT_RUN_ID")
            .output()
            .unwrap()
    }
    fn init(&self, verb: &str, flags: &[&str]) -> (i32, Value) {
        let out = self.cli(verb, flags, true, &Self::bin());
        let value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
            panic!(
                "{e}: {} {}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            )
        });
        (out.status.code().unwrap(), value)
    }
}
fn report(v: &Value) -> &Value {
    &v["report"]["value"]
}
fn request() -> String {
    format!("={}", producer::PRODUCER_VERSION)
}
#[test]
fn plan_and_apply_use_real_judge_and_preserve_unrelated_configuration() {
    let f = Fixture::new();
    let before = std::fs::read(f.root().join("spec-spine.toml")).unwrap();
    let request = request();
    let (code, plan) = f.init(
        "plan",
        &["--profile", "github-actions-rust", "--spine", &request],
    );
    assert!(code <= 1, "{plan}");
    let r = report(&plan);
    let identity = r["spinePin"]["planIdentity"].as_str().unwrap();
    assert_eq!(r["judge"]["version"], producer::PRODUCER_VERSION);
    assert!(
        r["judge"]["digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(!f.root().join(".statecraft").exists());
    assert!(!f.dir.path().join("home").exists());
    let (code, done) = f.init(
        "apply",
        &[
            "--profile",
            "github-actions-rust",
            "--spine",
            &request,
            "--plan",
            identity,
        ],
    );
    assert!(code <= 1, "{done}");
    assert_eq!(report(&done)["spinePin"]["outcome"], "written");
    assert_eq!(
        std::fs::read_to_string(f.root().join("spec-spine.toml")).unwrap(),
        String::from_utf8(before)
            .unwrap()
            .replace("=0.1.0", &request)
    );
    let (code, stale) = f.init(
        "apply",
        &[
            "--profile",
            "github-actions-rust",
            "--spine",
            &request,
            "--plan",
            identity,
        ],
    );
    assert_eq!(code, 2, "{stale}");
}
#[test]
fn both_argument_forms_work_and_missing_duplicate_or_wrong_verb_are_usage_errors() {
    let f = Fixture::new();
    let req = request();
    let equals = format!("--spine={req}");
    assert!(
        f.init("plan", &["--profile", "github-actions-rust", &equals])
            .0
            <= 1
    );
    for flags in [
        vec!["--spine"],
        vec!["--spine="],
        vec!["--spine", &req, &equals],
        vec!["--spine", &req, "--spine", &req],
    ] {
        let (code, value) = f.init("plan", &flags);
        assert_eq!(code, 3, "{flags:?}: {value}");
    }
    let out = Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
        .args(["doctor"])
        .arg(f.root())
        .args(["--spine", &req, "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3));
}
#[test]
fn invalid_requests_and_exclusive_wrong_version_judges_refuse_in_human_and_json() {
    let f = Fixture::new();
    let wrong = f.dir.path().join("wrong");
    std::fs::write(&wrong, "#!/bin/sh\necho 'spec-spine 9.9.9'\n").unwrap();
    std::fs::set_permissions(&wrong, std::fs::Permissions::from_mode(0o755)).unwrap();
    for value in [
        "=01.2.3",
        "=1.2",
        "=1.2.3-beta",
        "=1.2.3+build",
        "=9.9.9",
        ">=0.1.0",
    ] {
        let (code, answer) = f.init(
            "plan",
            &["--profile", "github-actions-rust", "--spine", value],
        );
        assert_eq!(code, 2, "{answer}");
    }
    for json in [false, true] {
        let out = f.cli(
            "plan",
            &["--profile", "github-actions-rust", "--spine", &request()],
            json,
            &wrong,
        );
        assert_eq!(
            out.status.code(),
            Some(2),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("refused"));
    }
    assert!(!f.root().join(".statecraft").exists());
    assert!(!f.dir.path().join("home").exists());
}
