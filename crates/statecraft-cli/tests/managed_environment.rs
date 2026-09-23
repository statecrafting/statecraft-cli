//! The managed-environment verbs, through the built binary.
//!
//! Spec 006 section 3.3's exit-code vocabulary is a property of a process, so
//! these spawn the executable rather than calling a function and inspecting an
//! enum. What each operation does is asserted in `statecraft-home`'s own suite;
//! what is asserted here is that the binary reaches it, renders it two ways
//! from one value, and ends with the right code.
//!
//! Every run is given a temporary `STATECRAFT_HOME` and a temporary
//! `STATECRAFT_NATIVE_ROOT`, so nothing here reads or writes the operator's own
//! home.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_statecraft-cli"))
}

struct Sandbox {
    dir: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let sandbox = Self { dir };
        sandbox.init_repo(&sandbox.project());
        sandbox
    }

    fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }

    fn native(&self) -> PathBuf {
        self.dir.path().join("native")
    }

    fn project(&self) -> PathBuf {
        self.dir.path().join("project")
    }

    fn init_repo(&self, at: &Path) {
        std::fs::create_dir_all(at).expect("the directory");
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(at)
                .output()
                .expect("git runs");
            assert!(out.status.success(), "git {args:?}");
        };
        git(&["init", "--quiet", "--initial-branch=main"]);
        git(&["config", "user.email", "test@example.invalid"]);
        git(&["config", "user.name", "test"]);
        git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(at.join("README.md"), b"x").expect("a file");
        git(&["add", "."]);
        git(&["commit", "--quiet", "-m", "one"]);
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(binary())
            .args(args)
            .env("STATECRAFT_HOME", self.home())
            .env("STATECRAFT_NATIVE_ROOT", self.native())
            // The child must not reach the operator's own home by accident.
            .env("HOME", self.dir.path())
            .env("PATH", stub_path(self.dir.path()))
            .output()
            .expect("the binary runs")
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("the process exited normally")
}

#[test]
fn home_show_answers_on_a_machine_with_no_home_yet_and_reports_what_is_missing() {
    let sandbox = Sandbox::new();
    let out = sandbox.run(&["home", "show"]);
    // A finding, not a failure: there is nothing wrong with a fresh machine.
    assert_eq!(code(&out), 1, "{}", stdout(&out));
    assert!(stdout(&out).contains("home "));
    assert!(stdout(&out).contains("no account, login, token or hosted connection was consulted"));
}

#[test]
fn home_apply_creates_the_home_and_home_show_then_exits_zero() {
    let sandbox = Sandbox::new();
    assert_eq!(code(&sandbox.run(&["home", "apply"])), 0);
    let out = sandbox.run(&["home", "show"]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    assert!(sandbox.home().join("home.json").is_file());
    assert!(sandbox.home().join("tools.json").is_file());
    assert!(sandbox.home().join("harness").is_dir());
}

#[test]
fn home_plan_writes_nothing() {
    let sandbox = Sandbox::new();
    assert_eq!(code(&sandbox.run(&["home", "plan"])), 0);
    assert!(
        !sandbox.home().exists(),
        "a preview created the home it was previewing"
    );
}

#[test]
fn init_plan_writes_nothing_inside_the_project() {
    let sandbox = Sandbox::new();
    let before: Vec<_> = std::fs::read_dir(sandbox.project())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    let out = sandbox.run(&["init", "plan", sandbox.project().to_str().unwrap()]);
    // The pinned producer is non-conforming today, so the preview is a
    // finding. What matters here is that it wrote nothing.
    assert!(matches!(code(&out), 0 | 1), "{}", stdout(&out));
    let after: Vec<_> = std::fs::read_dir(sandbox.project())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(before, after);
    assert!(!sandbox.project().join(".statecraft").exists());
}

#[test]
fn init_apply_reports_every_step_and_stops_before_arming() {
    let sandbox = Sandbox::new();
    let out = sandbox.run(&["init", "apply", sandbox.project().to_str().unwrap()]);
    let text = stdout(&out);
    for step in [
        "home",
        "plan",
        "reconcile",
        "governance",
        "project",
        "register",
    ] {
        assert!(text.contains(step), "the report omits {step}: {text}");
    }
    assert!(
        text.contains("Arming and execution are separate explicit acts."),
        "{text}"
    );
    assert!(
        sandbox
            .project()
            .join(".statecraft/environment.json")
            .is_file()
    );
    assert!(sandbox.project().join(".statecraft/AGENTS.md").is_file());
}

#[test]
fn the_json_rendering_carries_the_same_facts_as_the_human_one() {
    let sandbox = Sandbox::new();
    sandbox.run(&["init", "apply", sandbox.project().to_str().unwrap()]);
    let human = sandbox.run(&["home", "show"]);
    let json = sandbox.run(&["home", "show", "--json"]);
    assert_eq!(code(&human), code(&json), "one value, two renderings");

    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&json)).expect("--json is parseable");
    assert!(parsed.get("value").is_some());
    assert!(parsed.get("exit").is_some());
    assert_eq!(
        parsed["value"]["operation"], "home",
        "the operation names itself"
    );
    assert_eq!(parsed["value"]["value"]["platformConsulted"], false);
}

#[test]
fn an_operation_on_a_project_with_no_declaration_refuses_and_names_the_path() {
    let sandbox = Sandbox::new();
    let out = sandbox.run(&["config", "show", sandbox.project().to_str().unwrap()]);
    assert_eq!(code(&out), 2, "{}", stdout(&out));
    assert!(stdout(&out).contains(".statecraft/environment.json"));
}

#[test]
fn a_verb_missing_its_argument_is_a_usage_error_and_not_a_refusal() {
    let sandbox = Sandbox::new();
    for args in [
        vec!["init", "apply"],
        vec!["approval", "show", sandbox.project().to_str().unwrap()],
        vec!["project", "enroll", sandbox.project().to_str().unwrap()],
    ] {
        let out = sandbox.run(&args);
        assert_eq!(code(&out), 3, "{args:?}: {}", stdout(&out));
        assert!(stdout(&out).is_empty(), "{args:?}");
    }
}

#[test]
fn the_help_lists_every_new_group() {
    let sandbox = Sandbox::new();
    let out = sandbox.run(&["--help"]);
    let text = stdout(&out);
    for verb in [
        "home show",
        "home apply",
        "init plan",
        "init apply",
        "migrate plan",
        "project enroll",
        "config show",
        "approval grant",
    ] {
        assert!(text.contains(verb), "the help omits `{verb}`");
    }
    assert!(text.contains("002-environment-lifecycle"));
}

#[test]
fn enrollment_and_approval_round_trip_through_the_binary() {
    let sandbox = Sandbox::new();
    let project = sandbox.project();
    let path = project.to_str().unwrap();
    sandbox.run(&["init", "apply", path]);

    let granted = sandbox.run(&["approval", "grant", path, "003-x", "bart", "reviewed", "it"]);
    assert_eq!(code(&granted), 0, "{}", stdout(&granted));
    assert!(stdout(&granted).contains("local operator's authority"));

    let enrolled = sandbox.run(&["project", "enroll", path, "acme"]);
    assert_eq!(code(&enrolled), 0, "{}", stdout(&enrolled));
    assert!(stdout(&enrolled).contains("enrolled in acme"));

    let shown = sandbox.run(&["approval", "show", path, "003-x"]);
    // Still local: the project reserves no subject for the team, so enrollment
    // alone does not move this subject's authority.
    assert_eq!(code(&shown), 0, "{}", stdout(&shown));

    let unenrolled = sandbox.run(&["project", "unenroll", path]);
    assert_eq!(code(&unenrolled), 0);
    assert!(stdout(&unenrolled).contains("solo"));
}

#[test]
fn nothing_the_binary_does_here_reaches_the_operators_own_home() {
    // Two homes: the one the binary is told about, and the one it must not
    // touch. The second is a real directory, so a stray write would land in it
    // rather than failing.
    let sandbox = Sandbox::new();
    let untouched = sandbox.dir.path().join(".statecraft");
    sandbox.run(&["init", "apply", sandbox.project().to_str().unwrap()]);
    sandbox.run(&["home", "apply"]);
    assert!(
        !untouched.exists(),
        "the binary wrote to $HOME/.statecraft despite STATECRAFT_HOME"
    );
}

#[test]
fn migrate_plan_on_a_project_with_nothing_to_move_says_so_and_exits_zero() {
    let sandbox = Sandbox::new();
    let out = sandbox.run(&["migrate", "plan", sandbox.project().to_str().unwrap()]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    assert!(stdout(&out).contains("not needed"));
}

/// A `spec-spine` that answers every verb these suites' flows ask, first on
/// `PATH`, so they do not depend on whichever one the machine has. Spec 002
/// section 3.23: an absent producer is a refused corpus step and an
/// unregistered project, which is not what these suites are about.
const STUB_SPEC_SPINE: &str =
    "#!/bin/sh\ncase \"$1\" in\n  --version) echo 'spec-spine 0.23.0' ;;\n  *) exit 0 ;;\nesac\n";

fn stub_path(dir: &std::path::Path) -> String {
    let bin = dir.join("stub-bin");
    let spine = bin.join("spec-spine");
    if !spine.exists() {
        std::fs::create_dir_all(&bin).expect("the stub directory");
        statecraft_adapter::fixture::install_script(&spine, STUB_SPEC_SPINE, 0o755)
            .expect("the stub");
    }
    format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}
