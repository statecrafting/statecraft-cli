//! Spec 006 section 3.7, the observable negative cases, one test per row.
//!
//! These spawn the **built binary**. The exit-code vocabulary is the thing this
//! spec exists to fix, and an exit code is a property of a process: a test that
//! called a function and inspected a returned enum would be checking the mapping
//! without ever checking that the binary uses it.

#[path = "support/json_naming.rs"]
mod json_naming;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The binary this test was built alongside.
///
/// `CARGO_BIN_EXE_<name>` is set by Cargo for an integration test of a crate
/// with a `[[bin]]`, so this is the executable produced by the same build and
/// not whatever happens to be on PATH.
fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_statecraft-cli"))
}

fn run_in(home: &Path, args: &[&str]) -> Output {
    Command::new(binary())
        .args(args)
        .env("STATECRAFT_HOME", home)
        .output()
        .expect("the binary runs")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("the process exited normally")
}

/// A git repository with one commit and no corpus: `ungoverned`.
fn git_repo_without_corpus() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .output()
            .expect("git runs");
        assert!(out.status.success(), "git {args:?}");
    };
    git(&["init", "--quiet", "--initial-branch=main"]);
    git(&["config", "user.email", "test@example.invalid"]);
    git(&["config", "user.name", "test"]);
    git(&["config", "commit.gpgsign", "false"]);
    std::fs::write(dir.path().join("README.md"), b"x").unwrap();
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "one"]);
    dir
}

// Row 1: an unknown verb.
#[test]
fn an_unknown_verb_exits_3_names_it_and_lists_the_ones_that_exist() {
    let home = tempfile::tempdir().unwrap();
    let out = run_in(home.path(), &["env", "publish"]);

    assert_eq!(code(&out), 3, "usage");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("env publish"), "names the verb");
    assert!(err.contains("env apply"), "lists the ones that exist");
    assert!(err.contains("doctor"));
    assert!(stdout(&out).is_empty(), "nothing else happens");
}

#[test]
fn no_arguments_at_all_is_also_a_usage_error() {
    let home = tempfile::tempdir().unwrap();
    let out = run_in(home.path(), &[]);
    assert_eq!(code(&out), 3);
}

// Row 2: `env apply` against an unregistered target.
//
// Every environment verb, not only apply: registration is the precondition all
// five share, and a test covering one of them would leave the other four free
// to answer differently.
#[test]
fn an_environment_verb_against_an_unregistered_target_refuses_naming_the_path() {
    let home = tempfile::tempdir().unwrap();
    let target = git_repo_without_corpus();
    let path = target.path().to_string_lossy().to_string();

    for verb in [
        vec!["env", "plan"],
        vec!["env", "apply"],
        vec!["env", "upgrade"],
        vec!["env", "remove"],
        vec!["doctor"],
    ] {
        let mut args = verb.clone();
        args.push(&path);
        let out = run_in(home.path(), &args);
        assert_eq!(code(&out), 2, "{verb:?} refuses");
        let text = stdout(&out);
        assert!(text.contains("not registered"), "{verb:?}: {text}");
        assert!(text.contains(&path), "{verb:?} names the path");
    }

    // No write, which is the half of the row an exit code cannot carry.
    assert!(!target.path().join(".statecraft").exists());
}

#[test]
fn an_environment_verb_with_no_target_at_all_is_a_usage_error() {
    let home = tempfile::tempdir().unwrap();
    for verb in [
        vec!["env", "plan"],
        vec!["env", "apply"],
        vec!["env", "upgrade"],
        vec!["env", "remove"],
        vec!["doctor"],
    ] {
        let out = run_in(home.path(), &verb);
        assert_eq!(code(&out), 3, "{verb:?} is a usage error");
        assert!(stdout(&out).is_empty(), "nothing else happens");
    }
}

// Row 4: `env remove` with no manifest.
#[test]
fn env_remove_with_no_manifest_refuses_and_deletes_nothing() {
    let home = tempfile::tempdir().unwrap();
    let target = git_repo_without_corpus();
    let path = target.path().to_string_lossy().to_string();
    let registered = run_in(home.path(), &["project", "register", &path]);
    assert!(code(&registered) <= 1, "registration reports a verdict");

    let out = run_in(home.path(), &["env", "remove", &path]);
    assert_eq!(code(&out), 2);
    let text = stdout(&out);
    assert!(text.contains("refused"));
    // Spec 002 section 3.6's own reason, not one this binding invented.
    assert!(
        text.contains("will not guess which files were ours"),
        "{text}"
    );
    assert!(!target.path().join(".statecraft").exists());
}

// Row 6: `doctor` on any finding.
//
// Row 5, a clean environment exiting 0, needs the configured adapter to be
// available, which on this provider means a resolvable `claude`, a
// qualification record for the version in front of it and a darwin credential
// path. Two of those cannot be arranged portably, so the clean-report mapping is
// covered where it is a function of the report alone, in `bind`'s own suite.
// What is checked here is the half a process can show: a finding exits 1 and
// nothing is repaired.
#[test]
fn doctor_reports_an_unavailable_adapter_as_a_finding_and_repairs_nothing() {
    let home = tempfile::tempdir().unwrap();
    let target = git_repo_without_corpus();
    let path = target.path().to_string_lossy().to_string();
    run_in(home.path(), &["project", "register", &path]);

    let out = run_in(home.path(), &["doctor", &path]);
    // No qualification record exists under this temporary product home, so the
    // adapter is unavailable whichever platform this runs on.
    assert_eq!(code(&out), 1, "{}", stdout(&out));
    let text = stdout(&out);
    assert!(text.contains("claude-code"), "{text}");
    assert!(text.contains("missing"), "{text}");
    // Diagnoses, and repairs nothing.
    assert!(!target.path().join(".claude").exists());
    assert!(!target.path().join("CLAUDE.md").exists());
}

// `env plan` writes nothing, and reports the ratified adapter by name.
#[test]
fn env_plan_writes_nothing_and_names_the_configured_adapter() {
    let home = tempfile::tempdir().unwrap();
    let target = git_repo_without_corpus();
    let path = target.path().to_string_lossy().to_string();
    run_in(home.path(), &["project", "register", &path]);

    let out = run_in(home.path(), &["env", "plan", &path, "--json"]);
    // No plan-level refusal: one adapter cannot collide with itself. So the
    // previewed apply writes nothing and reports `applied` with an empty list,
    // and the plan takes the same exit.
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    assert!(!target.path().join(".claude").exists());
    assert!(!target.path().join("CLAUDE.md").exists());

    let parsed: serde_json::Value = json_naming::from_text(&stdout(&out)).expect("valid json");
    let adapters = parsed["report"]["adapters"]
        .as_array()
        .expect("the plan reports every configured adapter");
    assert_eq!(adapters.len(), 1, "one ratified adapter");
    assert_eq!(adapters[0]["name"], "claude-code");

    // No qualification record under this temporary product home, so it refuses
    // to claim its paths and names what is absent, on every platform.
    assert_eq!(adapters[0]["readiness"], "refused");
    assert!(
        !adapters[0]["missing"]
            .as_array()
            .expect("names what is absent")
            .is_empty()
    );
    // And the facts it cannot express are stated rather than dropped.
    assert!(
        !adapters[0]["unexpressible"]
            .as_array()
            .expect("stated rather than dropped")
            .is_empty()
    );
    assert!(parsed["report"]["writes"].as_array().unwrap().is_empty());
}

// Row 7: a command given `--json`.
#[test]
fn json_and_human_renderings_carry_the_same_facts_from_the_same_value() {
    let home = tempfile::tempdir().unwrap();
    let target = git_repo_without_corpus();
    let path = target.path().to_string_lossy().to_string();

    let human = run_in(home.path(), &["project", "register", &path]);
    let json = run_in(home.path(), &["project", "register", &path, "--json"]);

    assert_eq!(code(&human), code(&json), "the same exit either way");

    let parsed: serde_json::Value =
        json_naming::from_text(&stdout(&json)).expect("--json emits JSON");
    assert_eq!(parsed["report"]["verdict"], "ungoverned");
    assert_eq!(parsed["outcome"], "finding");

    // The same fact, in both renderings, because there is one value behind them.
    assert!(stdout(&human).contains("ungoverned"));
    let reasons = parsed["report"]["reasons"].as_array().unwrap();
    let first = reasons[0].as_str().unwrap();
    assert!(
        stdout(&human).contains(first),
        "the human rendering carries the same reason: {first}"
    );
}

// A verdict that is not `qualified` is a finding, not a failure.
#[test]
fn registering_an_ungoverned_repository_is_a_finding_with_exit_1() {
    let home = tempfile::tempdir().unwrap();
    let target = git_repo_without_corpus();
    let out = run_in(
        home.path(),
        &["project", "register", &target.path().to_string_lossy()],
    );

    assert_eq!(code(&out), 1, "a finding, not a failure");
    assert!(stdout(&out).contains("ungoverned"));
    assert!(stdout(&out).contains("no spec-spine corpus is present"));
}

#[test]
fn registering_a_path_that_is_not_a_git_work_tree_is_a_finding_and_writes_nothing_inside_it() {
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let out = run_in(
        home.path(),
        &["project", "register", &target.path().to_string_lossy()],
    );

    assert_eq!(code(&out), 1);
    assert!(stdout(&out).contains("unqualified"));
    assert_eq!(
        std::fs::read_dir(target.path()).unwrap().count(),
        0,
        "registration writes nothing inside the target"
    );
}

#[test]
fn a_relative_path_is_resolved_not_a_usage_error() {
    let home = tempfile::tempdir().unwrap();
    // The binary makes a relative path absolute against the working directory
    // before the library sees it, so this asserts the resulting path is judged
    // rather than rejected for its spelling.
    let out = run_in(home.path(), &["project", "register", "."]);
    assert_ne!(code(&out), 3, "a relative path is resolved, not refused");
}

// The register survives across invocations, and arming is separate.
#[test]
fn arming_is_a_separate_invocation_and_the_register_persists_between_them() {
    let home = tempfile::tempdir().unwrap();
    let target = git_repo_without_corpus();
    let path = target.path().to_string_lossy().to_string();

    run_in(home.path(), &["project", "register", &path]);

    let listed = run_in(home.path(), &["project", "list", "--json"]);
    let parsed: serde_json::Value = json_naming::from_text(&stdout(&listed)).unwrap();
    assert_eq!(
        parsed["report"][0]["armed"], false,
        "registered is not armed"
    );
    assert_eq!(code(&listed), 0, "listing is never a finding");

    let armed = run_in(home.path(), &["project", "arm", &path]);
    assert_eq!(code(&armed), 0);

    let after: serde_json::Value = json_naming::from_text(&stdout(&run_in(
        home.path(),
        &["project", "list", "--json"],
    )))
    .unwrap();
    assert_eq!(after["report"][0]["armed"], true);
    assert_eq!(
        after["report"][0]["eligible"], false,
        "armed but not qualified is still not eligible"
    );
}

#[test]
fn arming_a_target_that_was_never_registered_refuses_with_exit_2() {
    let home = tempfile::tempdir().unwrap();
    let out = run_in(home.path(), &["project", "arm", "/nowhere/at/all"]);
    assert_eq!(code(&out), 2, "a precondition, so a refusal");
}

// The vocabulary itself: three different answers, three different codes.
#[test]
fn a_finding_a_refusal_and_a_usage_error_are_three_distinct_exit_codes() {
    let home = tempfile::tempdir().unwrap();
    let target = git_repo_without_corpus();

    let finding = run_in(
        home.path(),
        &["project", "register", &target.path().to_string_lossy()],
    );
    let refusal = run_in(home.path(), &["project", "arm", "/nowhere"]);
    let usage = run_in(home.path(), &["nonsense"]);

    assert_eq!(code(&finding), 1);
    assert_eq!(code(&refusal), 2);
    assert_eq!(code(&usage), 3);
}

// Nothing in the command tree publishes.
#[test]
fn no_verb_the_binary_offers_publishes_anything() {
    let home = tempfile::tempdir().unwrap();
    let out = run_in(home.path(), &["nonsense"]);
    let listing = String::from_utf8_lossy(&out.stderr).to_lowercase();
    for forbidden in ["publish", "release", "tag", "push"] {
        assert!(
            !listing.contains(forbidden),
            "the command tree offers no `{forbidden}` verb"
        );
    }
}

// The executable's name is stated, not defaulted.
#[test]
fn the_executable_is_named_statecraft_cli() {
    let name = binary()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .replace(".exe", "");
    assert_eq!(
        name, "statecraft-cli",
        "spec 006 section 3.5 states the name rather than inheriting one"
    );
}
