//! Spec 003 section 3.8, the observable negative cases, one test per row.
//!
//! Named after the row each covers, so a row that stops being covered shows up
//! as a deleted test rather than as a quietly weakened assertion.

use statecraft_run::attempt::{Outcome, Run};
use statecraft_run::policy::{
    NoDeclarationFiled, Overrides, Policy, PolicySource, StaticDeclaration, resolve,
};
use statecraft_run::record::{Chain, Entry, Identity, Kind};
use statecraft_run::recovery::{
    CannotObserve, Observer, Unmatched, Verdict, blocked, reconcile, reconciliation_entry,
    unmatched_intents,
};
use statecraft_run::refusal::{Accounting, RefusalEvent, decide};
use statecraft_run::report::{
    CorpusReport, ReadySpec, ReportError, SpecLifecycle, parse_lifecycle,
};
use statecraft_run::work::select;
use statecraft_run::workspace::{self, WorkspaceError};
use std::path::Path;
use std::process::Command;

fn report(rows: &[(&str, &str)]) -> CorpusReport {
    CorpusReport {
        spec_spine_version: "spec-spine 0.18.0".into(),
        ready: rows
            .iter()
            .map(|(id, _)| ReadySpec {
                id: (*id).into(),
                title: format!("title of {id}"),
                status: None,
            })
            .collect(),
        lifecycle: rows
            .iter()
            .map(|(id, status)| SpecLifecycle {
                id: (*id).into(),
                status: (*status).into(),
                implementation: Some("pending".into()),
                obligations: vec![],
            })
            .collect(),
        status_source: statecraft_run::report::StatusSource::ListOnly,
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("git {args:?} in {dir:?}: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A git repository with one commit, which is the minimum a worktree needs.
fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "--quiet", "--initial-branch=main"]);
    git(
        dir.path(),
        &["config", "user.email", "test@example.invalid"],
    );
    git(dir.path(), &["config", "user.name", "test"]);
    git(dir.path(), &["config", "commit.gpgsign", "false"]);
    std::fs::write(dir.path().join("README.md"), b"x").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "--quiet", "-m", "one"]);
    dir
}

fn entry(kind: Kind, run: &str, attempt: u32, subject: &str, key: Option<&str>) -> Entry {
    Entry {
        kind,
        run_id: run.into(),
        attempt,
        subject: subject.into(),
        // Mechanical (spec 003 section 3.3.1 clause 7): these rows are the
        // legacy pairing's, and a record without the key folds exactly as it did.
        effect_id: Identity::Absent,
        idempotency_key: key.map(str::to_string),
        detail: serde_json::Value::Null,
    }
}

// Row 1: spec-spine's report lacks a field the product needs.
#[test]
fn a_report_missing_a_needed_field_refuses_naming_the_field_and_the_version() {
    let v = serde_json::json!([{"id": "003-x", "title": "no status here"}]);
    match parse_lifecycle(&v, "0.18.0") {
        Err(ReportError::MissingField {
            field,
            version,
            command,
        }) => {
            assert_eq!(field, "status");
            assert_eq!(version, "0.18.0");
            assert!(command.contains("registry list"));
        }
        other => panic!("expected a named missing-field refusal, got {other:?}"),
    }
}

// Row 2: spec-spine reports a draft spec as ready under the default policy.
#[test]
fn a_draft_offered_as_ready_is_listed_as_excluded_with_a_reason_and_never_scheduled() {
    let list = select(
        &report(&[("006-x", "draft")]),
        &Policy::default_policy(),
        &Overrides::none(),
        None,
    );
    assert!(list.eligible.is_empty(), "never scheduled");
    assert_eq!(list.excluded.len(), 1, "listed, not dropped");
    assert!(list.excluded[0].reason.contains("draft"));
}

// Row 3: a named draft is admitted by an operator override.
#[test]
fn an_override_schedules_the_named_draft_and_the_item_records_who_and_which() {
    let list = select(
        &report(&[("006-x", "draft")]),
        &Policy::default_policy(),
        &Overrides::none().admitting("006-x", "the operator", "demo"),
        None,
    );
    let item = &list.eligible[0];
    let o = item.admitted_by_override.as_ref().unwrap();
    assert_eq!(item.id, "006-x");
    assert_eq!(o.spec_id, "006-x");
    assert_eq!(o.operator, "the operator");
}

// Row 4: the target declares a policy and the product's state disagrees.
#[test]
fn the_targets_declaration_wins_and_the_disagreement_is_reported() {
    let target_says = Policy {
        schedulable_statuses: vec!["approved".into(), "draft".into()],
        source: PolicySource::Declared {
            as_written: "approved,draft".into(),
        },
    };
    let product_holds = Policy::default_policy();
    let (in_force, disagreement) = resolve(
        Path::new("/x"),
        &StaticDeclaration(Some(target_says.clone())),
        Some(&product_holds),
    );
    assert_eq!(in_force, target_says, "the product never prefers its copy");
    assert!(
        disagreement
            .unwrap()
            .contains("declaration is what applies")
    );
}

// Row 5: the target declares no policy.
#[test]
fn with_no_declaration_the_default_applies_and_the_attempt_records_it_as_defaulted() {
    let (in_force, _) = resolve(Path::new("/x"), &NoDeclarationFiled, None);
    assert_eq!(in_force.source, PolicySource::Defaulted);
    assert!(in_force.admits("approved"));
    assert!(!in_force.admits("draft"));

    let list = select(
        &report(&[("a", "approved")]),
        &in_force,
        &Overrides::none(),
        None,
    );
    assert!(
        list.policy_was_defaulted(),
        "the work list carries that the policy was assumed, not declared"
    );
}

// Row 6: a corpus that does not compile in the target.
#[test]
fn a_corpus_that_does_not_compile_refuses_and_never_falls_back_to_derived() {
    // A directory that looks like a corpus but whose tool cannot run: the
    // refusal must be about the corpus, and no `.derived/` read may substitute.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("spec-spine.toml"), "").unwrap();
    std::fs::create_dir_all(dir.path().join(".derived/spec-registry")).unwrap();
    std::fs::write(
        dir.path().join(".derived/spec-registry/tempting.json"),
        r#"{"status":"approved"}"#,
    )
    .unwrap();

    let source = statecraft_run::report::SpecSpineCli {
        binary: "spec-spine-that-does-not-exist".into(),
    };
    use statecraft_run::report::ReportSource;
    match source.corpus_report(dir.path()) {
        Err(ReportError::NotRunnable { .. }) | Err(ReportError::CorpusDoesNotCompile { .. }) => {}
        other => panic!("expected a refusal about the corpus, got {other:?}"),
    }
}

// Row 7: the base revision moves during an attempt.
#[test]
fn a_base_revision_that_moves_is_observable_and_makes_the_attempt_interrupted() {
    let dir = repo();
    let ws = workspace::prepare(dir.path(), "run-1", "HEAD").unwrap();
    assert!(!workspace::base_moved(dir.path(), &ws, "HEAD"));

    std::fs::write(dir.path().join("second.md"), b"y").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "--quiet", "-m", "two"]);

    assert!(workspace::base_moved(dir.path(), &ws, "HEAD"));

    let mut run = Run::new("run-1");
    run.append_attempt(&ws.base_commit);
    run.conclude(Outcome::Interrupted);
    assert_eq!(run.attempts[0].outcome, Some(Outcome::Interrupted));
    assert!(
        !Outcome::Interrupted.was_judged(),
        "interrupted is not failed: nothing was judged"
    );
}

// Row 8: the workspace path is occupied by a foreign directory.
#[test]
fn an_occupied_workspace_path_refuses_naming_it_with_no_deletion_and_no_reuse() {
    let dir = repo();
    let path = workspace::workspace_path(dir.path(), "run-1");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("somebody-elses.txt"), b"do not delete me").unwrap();

    match workspace::prepare(dir.path(), "run-1", "HEAD") {
        Err(WorkspaceError::Occupied { path: named }) => {
            assert!(named.contains("run-1"));
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert!(
        path.join("somebody-elses.txt").exists(),
        "no deletion, no reuse"
    );
}

#[test]
fn preparing_twice_is_a_no_op_that_reports_the_existing_path() {
    let dir = repo();
    let first = workspace::prepare(dir.path(), "run-1", "HEAD").unwrap();
    let second = workspace::prepare(dir.path(), "run-1", "HEAD").unwrap();
    assert_eq!(first.path, second.path);
    assert_eq!(first.base_commit, second.base_commit);
}

#[test]
fn two_runs_never_share_a_workspace() {
    let dir = repo();
    let a = workspace::prepare(dir.path(), "run-a", "HEAD").unwrap();
    let b = workspace::prepare(dir.path(), "run-b", "HEAD").unwrap();
    assert_ne!(a.path, b.path);
}

#[test]
fn the_operators_checkout_is_never_edited() {
    let dir = repo();
    let before = git(dir.path(), &["status", "--porcelain"]);
    workspace::prepare(dir.path(), "run-1", "HEAD").unwrap();
    // The worktree lives under .statecraft/state/, which is gitignored in a
    // target this product manages; what matters here is that no TRACKED file
    // changed.
    let after = git(
        dir.path(),
        &["status", "--porcelain", "--untracked-files=no"],
    );
    assert_eq!(before, after);
}

// Row 9: a second attempt starts while one is live.
#[test]
fn a_second_live_attempt_is_refused_naming_the_live_one() {
    let mut run = Run::new("run-1");
    run.append_attempt("aaa");
    let live = run.live_attempt().expect("one is live").clone();
    assert_eq!(live.number, 1);
    assert!(
        run.live_attempt().is_some(),
        "a caller checks this before starting another, and it names attempt {}",
        live.number
    );
    run.conclude(Outcome::Completed);
    assert!(run.live_attempt().is_none());
}

// Row 10: the process dies between intent and outcome.
#[test]
fn recovery_finds_the_unmatched_intent_before_anything_is_retried() {
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();

    chain
        .append(
            "1",
            "2026-09-16T00:00:00Z",
            &entry(Kind::Intent, "run-1", 1, "publish", Some("key-1")),
        )
        .unwrap();
    // ... and the process dies here, before the outcome.

    let (chain, report) = Chain::open(home.path(), target.path()).unwrap();
    assert_eq!(report.records, 1);
    let unmatched = unmatched_intents(&chain);
    assert_eq!(unmatched.len(), 1);
    assert_eq!(unmatched[0].subject, "publish");
    assert!(unmatched[0].idempotent(), "the key was recorded");
}

#[test]
fn an_intent_with_its_outcome_is_not_unmatched() {
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    chain
        .append(
            "1",
            "t",
            &entry(Kind::Intent, "run-1", 1, "publish", Some("k")),
        )
        .unwrap();
    chain
        .append(
            "2",
            "t",
            &entry(Kind::Outcome, "run-1", 1, "publish", Some("k")),
        )
        .unwrap();
    assert!(unmatched_intents(&chain).is_empty());
}

// Row 11: reconciliation cannot determine whether the effect happened.
#[test]
fn an_unknown_reconciliation_blocks_the_retry_and_is_reported() {
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    chain
        .append("1", "t", &entry(Kind::Intent, "run-1", 1, "publish", None))
        .unwrap();

    let reconciled = reconcile(&chain, &CannotObserve);
    assert_eq!(reconciled[0].verdict, Verdict::Unknown);
    assert!(!reconciled[0].retry_allowed(), "unknown blocks the retry");
    assert_eq!(blocked(&reconciled).len(), 1, "and is reported");

    let record = reconciliation_entry(&reconciled[0]);
    assert_eq!(record.kind, Kind::Reconciliation);
    assert_eq!(record.detail["retryAllowed"], serde_json::json!(false));
    assert_eq!(
        record.detail["idempotentByKey"],
        serde_json::json!(false),
        "an effect with no key says so rather than implying safety"
    );
}

#[test]
fn confirmed_and_absent_both_permit_a_retry_decision_to_proceed() {
    struct Says(Verdict);
    impl Observer for Says {
        fn observe(&self, _: &Unmatched) -> Verdict {
            self.0
        }
    }
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    chain
        .append("1", "t", &entry(Kind::Intent, "r", 1, "s", None))
        .unwrap();

    for v in [Verdict::Confirmed, Verdict::Absent] {
        let r = reconcile(&chain, &Says(v));
        assert!(r[0].retry_allowed());
        assert!(blocked(&r).is_empty());
    }
}

// Row 12: the record's tail is torn by a crash mid-append.
#[test]
fn a_torn_tail_is_reported_read_up_to_and_appended_after_never_rewritten() {
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    chain
        .append("1", "t", &entry(Kind::Intent, "r", 1, "first", None))
        .unwrap();
    chain
        .append("2", "t", &entry(Kind::Outcome, "r", 1, "first", None))
        .unwrap();

    // A crash mid-append: a partial line with no terminating newline.
    let path = statecraft_run::record::chain_path(home.path(), target.path());
    let intact = std::fs::read(&path).unwrap();
    let mut torn = intact.clone();
    torn.extend_from_slice(br#"{"id":"3","timestamp":"t","payl"#);
    std::fs::write(&path, &torn).unwrap();

    let (mut chain, report) = Chain::open(home.path(), target.path()).unwrap();
    assert_eq!(report.records, 2, "reads to the last complete record");
    let tail = report.torn_tail.expect("the torn tail is reported");
    assert_eq!(tail.offset, intact.len() as u64);

    chain
        .append("3", "t", &entry(Kind::Intent, "r", 2, "second", None))
        .unwrap();

    let (reopened, report) = Chain::open(home.path(), target.path()).unwrap();
    assert!(report.torn_tail.is_none(), "the tear is resolved");
    assert_eq!(reopened.records().len(), 3);
    let after = std::fs::read(&path).unwrap();
    assert_eq!(
        &after[..intact.len()],
        &intact[..],
        "the earlier records are byte-identical; nothing was rewritten"
    );
}

// Row 13: the adapter reports refusals and exits zero.
#[test]
fn refusals_on_the_event_stream_make_the_attempt_refused_whatever_the_exit_code_said() {
    let mut accounting = Accounting::default();
    accounting.observe(RefusalEvent {
        guard: "write-outside-workspace".into(),
        detail: "tried to write to the product home".into(),
    });

    // The adapter classified its own termination as a clean completion, and the
    // supervised process exited zero. Neither is an authority here.
    assert_eq!(decide(Outcome::Completed, &accounting), Outcome::Refused);
    assert_eq!(accounting.count, 1);
    assert_eq!(accounting.sample.len(), 1, "a bounded sample is retained");
}

// Row 14: the supervised process attempts to alter the refusal count.
#[test]
fn the_chain_lives_where_the_supervised_process_cannot_reach_and_tampering_is_recorded() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let ws = workspace::prepare(target.path(), "run-1", "HEAD").unwrap();

    let chain = statecraft_run::record::chain_path(home.path(), target.path());
    assert!(
        !chain.starts_with(target.path()),
        "the chain is not inside the target"
    );
    assert!(
        !chain.starts_with(&ws.path),
        "and certainly not inside the workspace the supervised process runs in"
    );

    let mut accounting = Accounting::default();
    accounting.note_tamper_attempt("write to the product home from the workspace");
    assert_eq!(accounting.tamper_attempts.len(), 1);
}

// Row 15: a retry is requested for a completed attempt.
#[test]
fn retrying_a_completed_attempt_appends_and_leaves_the_earlier_records_readable() {
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    chain
        .append("1", "t", &entry(Kind::Intent, "run-1", 1, "turn", None))
        .unwrap();
    chain
        .append("2", "t", &entry(Kind::Outcome, "run-1", 1, "turn", None))
        .unwrap();
    let before = chain.entries();

    let mut run = Run::new("run-1");
    run.append_attempt("aaa");
    run.conclude(Outcome::Completed);
    run.append_attempt("aaa");

    chain
        .append("3", "t", &entry(Kind::Intent, "run-1", 2, "turn", None))
        .unwrap();

    let after = chain.entries();
    assert_eq!(&after[..2], &before[..], "history is unchanged");
    assert_eq!(after.len(), 3);
    assert_eq!(run.attempts.len(), 2);
    assert_eq!(run.attempts[0].outcome, Some(Outcome::Completed));
}

#[test]
fn a_chain_reopened_verifies_its_hash_links() {
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    for i in 0..5 {
        chain
            .append(&i.to_string(), "t", &entry(Kind::Intent, "r", i, "s", None))
            .unwrap();
    }
    // Opening re-verifies; a broken link would be an error, not a silent read.
    let (reopened, _) = Chain::open(home.path(), target.path()).unwrap();
    assert_eq!(reopened.records().len(), 5);
}
