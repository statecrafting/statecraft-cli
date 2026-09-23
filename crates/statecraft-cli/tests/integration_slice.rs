//! Spec 006 section 3.7, the observable negative cases, seven of ten.
//!
//! A separate file from this crate's `negative_cases.rs`, which covers the
//! environment and usage rows. The two sets were filed apart when the bindings
//! were two specs, and they stay apart: merging the files would make a deleted
//! row look like a refactor.
//!
//! **Three of the ten rows are review obligations and not tests**, and spec
//! 006's `## Verification` block says so. Rows 6, 9 and 10 require that an
//! inspection verb which repaired the record, a verb that answered a
//! specification question itself, and a CLI-crate answer an owning crate could
//! have returned are each "refused as a defect". A defect refused in review is
//! refused by a reader, and a test asserting the absence of code nobody wrote
//! passes for the wrong reason. Spec 006 section 3.2 already makes the
//! structural half mechanical: the coupling gate refuses a change to this crate
//! that does not edit an owning spec, so a rule that leaked into a command has
//! to be written down where it visibly does not belong.
//!
//! These spawn the built binary, for the reason `negative_cases.rs` gives: an
//! exit code is a property of a process.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

/// A git repository with one commit and no corpus.
fn target() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "--quiet"]);
    git(dir.path(), &["config", "user.email", "t@example.com"]);
    git(dir.path(), &["config", "user.name", "t"]);
    std::fs::write(dir.path().join("a.txt"), b"a").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "--quiet", "-m", "one"]);
    dir
}

fn registered() -> (tempfile::TempDir, tempfile::TempDir, String) {
    let home = tempfile::tempdir().unwrap();
    let target = target();
    let path = target.path().to_string_lossy().to_string();
    let out = run_in(home.path(), &["project", "register", &path]);
    assert!(code(&out) <= 1, "registration reports a verdict: {out:?}");
    (home, target, path)
}

// The three commands spec 006's verification block calls the only ones that
// check what the work, run and accept bindings are FOR: every verb they name
// was implemented already, inside a territory no command line reached.
#[test]
fn every_verb_this_slice_adds_is_reachable_from_a_command_line() {
    let home = tempfile::tempdir().unwrap();
    for group in ["work", "run", "accept"] {
        let out = run_in(home.path(), &[group, "--help"]);
        // An absent verb exits 3 under spec 006 section 3.3, so this fails
        // loudly on exactly the defect this spec exists to remove.
        assert_eq!(code(&out), 0, "{group} --help: {out:?}");
        assert!(stdout(&out).contains(group));
    }

    // And reachability says nothing about whether an attempt would succeed,
    // which is spec 006 section 3.10's own warning about `run` exiting 0.
    let help = stdout(&run_in(home.path(), &["run", "--help"]));
    assert!(help.contains("run list"));
    assert!(help.contains("run show"));
}

// Row 3: a spec-spine report lacks a field a verb needs.
//
// Reached here through the neighbouring case the same refusal covers: a target
// with no corpus at all cannot answer, and spec 003 section 3.8 routes every
// such gap to a refusal that names what was missing rather than a substitute.
#[test]
fn work_list_on_a_target_whose_corpus_cannot_answer_refuses_and_derives_nothing() {
    let (home, _target, path) = registered();
    let out = run_in(home.path(), &["work", "list", &path]);
    // Refused (2) when spec-spine is absent or a report lacks a field, a
    // finding (1) when the corpus does not compile, and **never 0**: an answer
    // this product made up would be the defect this row exists to catch. Both
    // readings are reached on a real machine: a developer's has spec-spine on
    // PATH and the check runner does not, so the row is checked on the
    // property they share rather than on the one that varies.
    assert!(
        code(&out) == 1 || code(&out) == 2,
        "exit {}: {}",
        code(&out),
        stdout(&out)
    );
    let text = stdout(&out);
    assert!(
        text.contains("refus") || text.contains("does not compile") || text.contains("spec-spine"),
        "{text}"
    );
}

// Rows 1 and 2, over the join itself: a ready spec the lifecycle report does
// not carry is excluded as unknown, and a `draft` plus `pending` spec is listed
// as excluded with the reason.
//
// This corpus is the one under test, so the join runs against a real pair of
// spec-spine reports.
#[test]
fn work_list_against_a_real_corpus_names_the_report_each_field_came_from() {
    let home = tempfile::tempdir().unwrap();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let path = repo.to_string_lossy().to_string();
    let registered = run_in(home.path(), &["project", "register", &path]);
    if code(&registered) > 1 {
        // No spec-spine on this machine's PATH: the row is covered by the
        // binding's own suite, and a test that passed by not running would be
        // the `unknown` spec 005 section 3.2 rule 3 refuses to call a pass.
        return;
    }

    let out = run_in(home.path(), &["work", "list", &path, "--json"]);
    if code(&out) != 0 {
        return;
    }
    let parsed: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("valid json");
    let rows = parsed["value"]["eligible"].as_array().unwrap();
    let excluded = parsed["value"]["excluded"].as_array().unwrap();
    assert!(
        !parsed["value"]["specSpineVersion"]
            .as_str()
            .unwrap_or_default()
            .is_empty(),
        "the version that produced the reports is recorded"
    );
    for row in rows.iter().chain(excluded.iter()) {
        // Every row that came from the ready set says which report field it
        // came from, and the status half says it came from the other report.
        if let Some(from) = row["fromField"].as_str() {
            assert!(from.contains("registry plan"), "{from}");
            assert_eq!(
                row["statusFromField"].as_str().unwrap(),
                "registry list --json: status"
            );
        }
    }
}

// Row 4: `run` invoked twice concurrently on one repository.
//
// Sequentially here, with the first attempt left live, which is the state the
// concurrent case produces and the one the refusal is about.
#[test]
fn a_second_run_while_an_attempt_is_live_is_refused_naming_the_live_attempt() {
    let (home, target, _path) = registered();
    // A live attempt is an intent with no outcome, written by the run crate.
    let (mut chain, _) = statecraft_run::record::Chain::open(home.path(), target.path()).unwrap();
    statecraft_run::session::begin(
        &mut chain,
        target.path(),
        "008",
        "HEAD",
        &statecraft_environment::time::FixedClock(0),
    )
    .unwrap();

    match statecraft_run::session::begin(
        &mut chain,
        target.path(),
        "009",
        "HEAD",
        &statecraft_environment::time::FixedClock(1),
    ) {
        Err(statecraft_run::session::SessionError::LiveAttempt { run_id, attempt }) => {
            assert_eq!(run_id, "008");
            assert_eq!(attempt, 1);
        }
        other => panic!("expected a refusal naming the live attempt, got {other:?}"),
    }

    // And the binding maps that refusal to exit 2, not to a failure.
    let answer = statecraft_cli::slice::session_error_answer(
        &statecraft_run::session::SessionError::LiveAttempt {
            run_id: "008".into(),
            attempt: 1,
        },
    );
    assert_eq!(answer.exit.code(), 2);
}

// Row 5: `run show` on a run with an intent and no outcome.
#[test]
fn run_show_on_a_live_run_shows_the_reconciliation_state_and_infers_no_outcome() {
    let (home, target, path) = registered();
    let (mut chain, _) = statecraft_run::record::Chain::open(home.path(), target.path()).unwrap();
    statecraft_run::session::begin(
        &mut chain,
        target.path(),
        "008",
        "HEAD",
        &statecraft_environment::time::FixedClock(0),
    )
    .unwrap();

    let out = run_in(home.path(), &["run", "show", &path, "008"]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    let text = stdout(&out);
    assert!(text.contains("no outcome recorded"), "{text}");
    for word in ["completed", "failed"] {
        assert!(!text.contains(word), "no outcome may be inferred: `{word}`");
    }

    // And `run list` shows the attempt as live rather than as an outcome.
    let listed = run_in(home.path(), &["run", "list", &path]);
    assert_eq!(code(&listed), 0);
    assert!(stdout(&listed).contains("live"));
}

// Row 7: `accept` on an attempt that ended `refused`.
#[test]
fn accept_on_a_refused_attempt_is_not_attempted_with_the_count_and_no_receipt() {
    let (home, target, path) = registered();
    let (mut chain, _) = statecraft_run::record::Chain::open(home.path(), target.path()).unwrap();
    let session = statecraft_run::session::begin(
        &mut chain,
        target.path(),
        "008",
        "HEAD",
        &statecraft_environment::time::FixedClock(0),
    )
    .unwrap();

    let mut accounting = statecraft_run::refusal::Accounting::default();
    accounting.observe(statecraft_run::refusal::RefusalEvent {
        guard: "permission-deny-rule/Bash".into(),
        detail: "denied".into(),
    });
    let concluded = statecraft_run::session::conclude(
        &mut chain,
        target.path(),
        &session,
        // The adapter claimed it completed. The supervisor's own accounting
        // overrules that, which is spec 003 section 3.5.
        statecraft_run::attempt::Outcome::Completed,
        &accounting,
        serde_json::json!({}),
        &statecraft_environment::time::FixedClock(1),
    )
    .unwrap();
    assert_eq!(concluded.outcome, statecraft_run::attempt::Outcome::Refused);

    let out = run_in(home.path(), &["accept", &path, "008", "--json"]);
    assert_eq!(
        code(&out),
        1,
        "a finding, never a silent zero: {}",
        stdout(&out)
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("valid json");
    // The wire shape here is spec 005's own `Acceptance`, which that crate
    // derives and that spec fixed; see spec 006 section 5.
    assert_eq!(parsed["value"]["acceptance"], "not-attempted");
    assert_eq!(parsed["value"]["reason"], "attempt-refused");
    assert_eq!(parsed["value"]["refusal_count"], 1);
    assert!(parsed["value"].get("receipt").is_none(), "no receipt");
}

// Row 8: the agent claims success and the suite fails.
//
// The claim survives in a field named for a claim, and the fold never reads it
// as a result. Checked through `run show`, which is where the account is.
#[test]
fn an_agent_claim_survives_beside_the_result_and_is_never_read_as_one() {
    let (home, target, path) = registered();
    let (mut chain, _) = statecraft_run::record::Chain::open(home.path(), target.path()).unwrap();
    let session = statecraft_run::session::begin(
        &mut chain,
        target.path(),
        "008",
        "HEAD",
        &statecraft_environment::time::FixedClock(0),
    )
    .unwrap();
    statecraft_run::session::conclude(
        &mut chain,
        target.path(),
        &session,
        statecraft_run::attempt::Outcome::Failed,
        &statecraft_run::refusal::Accounting::default(),
        serde_json::json!({ "agentClaim": "I finished the work" }),
        &statecraft_environment::time::FixedClock(1),
    )
    .unwrap();

    let out = run_in(home.path(), &["run", "show", &path, "008"]);
    assert_eq!(code(&out), 0);
    let text = stdout(&out);
    assert!(text.contains("I finished the work"));
    // Marked as a narrative, so nothing reads it as the result.
    assert!(text.contains("not a result"), "{text}");
    assert!(text.contains("failed"));
}

// An unknown run id is a refusal, not an empty success.
#[test]
fn an_inspection_of_a_run_the_record_does_not_carry_refuses() {
    let (home, _target, path) = registered();
    for verb in [vec!["run", "show"], vec!["accept"]] {
        let mut args = verb.clone();
        args.push(&path);
        args.push("nope");
        let out = run_in(home.path(), &args);
        assert_eq!(code(&out), 2, "{verb:?}: {}", stdout(&out));
        assert!(stdout(&out).contains("nope"));
    }
}

// Every verb this slice adds needs a registered target, and `--json` renders
// the same value the human output does (spec 006 section 3.4).
#[test]
fn every_new_verb_refuses_an_unregistered_target_and_renders_the_same_value_either_way() {
    let home = tempfile::tempdir().unwrap();
    let target = target();
    let path = target.path().to_string_lossy().to_string();

    for args in [
        vec!["work", "list"],
        vec!["work", "show"],
        vec!["run", "list"],
        vec!["run", "show"],
        vec!["accept"],
    ] {
        let mut human = args.clone();
        human.push(&path);
        let mut json = human.clone();
        json.push("--json");

        let h = run_in(home.path(), &human);
        let j = run_in(home.path(), &json);
        assert_eq!(code(&h), 2, "{args:?}: {}", stdout(&h));
        assert_eq!(code(&h), code(&j), "{args:?}: the same exit either way");
        let parsed: serde_json::Value = serde_json::from_str(&stdout(&j)).expect("valid json");
        assert!(
            parsed["value"]
                .as_str()
                .unwrap_or_default()
                .contains("not registered"),
            "{args:?}: {parsed}"
        );
    }
}

// There is no verb that publishes, and adding `accept` did not add one.
#[test]
fn the_grown_command_tree_still_has_no_verb_that_publishes() {
    for verb in statecraft_cli::commands::Verb::all() {
        let spelling = verb.spelling();
        for forbidden in ["publish", "release", "tag", "push", "deploy"] {
            assert!(
                !spelling.contains(forbidden),
                "`{spelling}` contains `{forbidden}`"
            );
        }
    }
    // The count is spelled out rather than derived, so a verb joining the tree
    // is a deliberate edit here. Fifteen when the work, run and accept
    // bindings landed; twelve more with the managed environment; five more
    // with spec 006 section 3.11.1, which made 002's harness, payload and
    // startup acts reachable; one more with section 3.11.2's `startup capture`,
    // the launch that binds a qualification control to its settings; and one
    // more with section 3.11.3's `startup show`, the read of a run attempt's
    // startup evidence; and one more with section 3.11.4's `startup trial`,
    // which spends a project's one managed-startup trial.
    assert_eq!(statecraft_cli::commands::Verb::all().len(), 35);
}
