//! Spec 050: the local halt record, its fold, and the pre-launch check.
//!
//! R-3 and I-1 at the record level: the halt lives in the product home's run
//! record and nothing launches while run-admission is halted or while the
//! record cannot be read. R-1: the refusal words. R-2: stop-requested is
//! recorded by the halt, stop-confirmed only by a durable `cancelled` outcome.

use statecraft_environment::time::FixedClock;
use statecraft_run::halt::{self, HaltError, Refusal, Request, Scope};
use statecraft_run::record::{Chain, Entry, Identity, Kind, RecordError, chain_path};
use statecraft_run::refusal::Accounting;
use statecraft_run::session::{self, SessionError};
use statecraft_run::{Outcome, runs};
use std::path::Path;

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "--quiet"]);
    git(dir.path(), &["config", "user.email", "t@example.com"]);
    git(dir.path(), &["config", "user.name", "t"]);
    std::fs::write(dir.path().join("a.txt"), b"a").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "--quiet", "-m", "one"]);
    dir
}

fn request<'a>(scopes: &'a [Scope], at: &'a str) -> Request<'a> {
    Request {
        scopes,
        operator: "bart",
        reason: "incident review",
        at,
    }
}

fn begin(
    chain: &mut Chain,
    target: &Path,
    run: &str,
    t: i64,
) -> Result<session::Session, SessionError> {
    session::begin(chain, target, run, "HEAD", &FixedClock(t))
}

#[test]
fn a_halted_run_admission_scope_refuses_run_naming_scope_operator_and_reason() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    let applied = halt::halt(&mut chain, &request(&[Scope::RunAdmission], "t1")).unwrap();
    assert_eq!(applied.change, "halt");
    assert!(applied.state.of(Scope::RunAdmission).is_some());
    assert!(applied.state.of(Scope::ToolEffects).is_none());

    match begin(&mut chain, target.path(), "r1", 0) {
        Err(SessionError::Halted(Refusal::Halted(h))) => {
            assert_eq!(h.scope, Scope::RunAdmission);
            assert_eq!(h.operator, "bart");
            assert_eq!(h.reason, "incident review");
            assert_eq!(h.at, "t1");
            assert_eq!(Refusal::Halted(h).reason(), "halted");
        }
        other => panic!("expected the closed reason halted, got {other:?}"),
    }
    // I-1: nothing launched, nothing prepared, no attempt appended.
    assert!(runs(&chain).iter().all(|r| r.attempts.is_empty()));
    assert!(!statecraft_run::workspace::workspace_path(target.path(), "r1").exists());
}

#[test]
fn the_other_two_scopes_do_not_refuse_run_and_a_bare_halt_halts_all_three() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    halt::halt(
        &mut chain,
        &request(&[Scope::ToolEffects, Scope::Dispatch], "t1"),
    )
    .unwrap();
    let session = begin(&mut chain, target.path(), "r1", 0).expect("admitted");
    session::conclude(
        &mut chain,
        target.path(),
        &session,
        Outcome::Completed,
        &Accounting::default(),
        serde_json::json!({}),
        &FixedClock(1),
    )
    .unwrap();

    let applied = halt::halt(&mut chain, &request(&[], "t2")).unwrap();
    assert_eq!(applied.detail.scopes, Scope::all().to_vec());
    assert_eq!(applied.state.halted.len(), 3);
    assert!(matches!(
        begin(&mut chain, target.path(), "r1", 2),
        Err(SessionError::Halted(Refusal::Halted(_)))
    ));
}

#[test]
fn resume_lifts_one_scope_or_all_and_run_is_admitted_again() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    halt::halt(&mut chain, &request(&[], "t1")).unwrap();
    let lifted = halt::resume(&mut chain, &request(&[Scope::Dispatch], "t2")).unwrap();
    assert_eq!(lifted.change, "resume");
    assert_eq!(lifted.state.halted.len(), 2);
    assert!(lifted.state.of(Scope::Dispatch).is_none());
    assert!(matches!(
        begin(&mut chain, target.path(), "r1", 0),
        Err(SessionError::Halted(Refusal::Halted(_)))
    ));
    let lifted = halt::resume(&mut chain, &request(&[], "t3")).unwrap();
    assert!(lifted.state.halted.is_empty());
    assert_eq!(
        begin(&mut chain, target.path(), "r1", 1).unwrap().attempt,
        1
    );
    // The fold is the record and nothing else: reopened from bytes it agrees.
    let (reopened, _) = Chain::open(home.path(), target.path()).unwrap();
    assert_eq!(halt::fold(&reopened).unwrap(), halt::fold(&chain).unwrap());
}

#[test]
fn the_halt_record_is_in_the_product_home_and_nothing_in_the_target() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let before: Vec<_> = std::fs::read_dir(target.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    halt::halt(&mut chain, &request(&[], "t1")).unwrap();
    let path = chain_path(home.path(), target.path());
    assert!(path.starts_with(home.path()) && path.is_file());
    let after: Vec<_> = std::fs::read_dir(target.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(before, after, "the target holds no halt state");
    // R-3: the change is an appended record of the chain, hash-linked.
    assert_eq!(chain.records().len(), 1);
    assert_eq!(chain.records()[0].payload["kind"], "halt");
}

#[test]
fn an_empty_operator_or_reason_is_refused_and_records_nothing() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    let blank = Request {
        scopes: &[],
        operator: " ",
        reason: "x",
        at: "t",
    };
    assert!(matches!(
        halt::halt(&mut chain, &blank),
        Err(HaltError::Refused(_))
    ));
    let no_reason = Request {
        scopes: &[],
        operator: "bart",
        reason: "",
        at: "t",
    };
    assert!(matches!(
        halt::resume(&mut chain, &no_reason),
        Err(HaltError::Refused(_))
    ));
    assert!(chain.records().is_empty());
}

#[test]
fn a_truncated_record_refuses_with_halt_state_unreadable() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    halt::halt(&mut chain, &request(&[Scope::Dispatch], "t1")).unwrap();
    let path = chain_path(home.path(), target.path());
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, &bytes[..bytes.len() - 7]).unwrap();

    let (mut torn, report) = Chain::open(home.path(), target.path()).unwrap();
    assert!(report.torn_tail.is_some());
    assert!(matches!(halt::fold(&torn), Err(Refusal::Unreadable { .. })));
    match begin(&mut torn, target.path(), "r1", 0) {
        Err(SessionError::Halted(r @ Refusal::Unreadable { .. })) => {
            assert_eq!(r.reason(), "halt-state-unreadable");
        }
        other => panic!("expected halt-state-unreadable, got {other:?}"),
    }
    assert!(runs(&torn).iter().all(|r| r.attempts.is_empty()));
    // The dispatch halt was the torn entry: nothing is halted, and nothing is
    // admitted either, which is what failing closed means here.
    assert!(matches!(
        halt::halt(&mut torn, &request(&[], "t2")),
        Err(HaltError::Unreadable(_))
    ));
}

#[test]
fn an_unparsable_halt_entry_refuses_with_halt_state_unreadable() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    // Well-formed in the chain (the links verify) and not a halt this product
    // can read: an unknown scope word.
    chain
        .append(
            "halt/0",
            "t1",
            &Entry {
                kind: Kind::Halt,
                run_id: String::new(),
                attempt: 0,
                subject: "halt".into(),
                idempotency_key: None,
                detail: serde_json::json!({
                    "scopes": ["everything"], "operator": "bart", "reason": "r", "at": "t1"
                }),
                effect_id: Identity::Absent,
            },
        )
        .unwrap();
    let (reopened, _) = Chain::open(home.path(), target.path()).unwrap();
    match halt::fold(&reopened) {
        Err(Refusal::Unreadable { detail }) => {
            assert!(detail.contains("does not parse"), "{detail}")
        }
        other => panic!("{other:?}"),
    }
    let mut reopened = reopened;
    assert!(matches!(
        begin(&mut reopened, target.path(), "r1", 0),
        Err(SessionError::Halted(Refusal::Unreadable { .. }))
    ));
}

#[test]
fn a_chain_mismatched_halt_entry_does_not_open_and_reads_as_unreadable() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    halt::halt(&mut chain, &request(&[Scope::RunAdmission], "t1")).unwrap();
    let path = chain_path(home.path(), target.path());
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("run-admission"));
    // The same bytes with the scope rewritten: the record's hash no longer
    // matches what was linked, so the chain refuses to open at all.
    std::fs::write(&path, text.replace("run-admission", "dispatch")).unwrap();
    match Chain::open(home.path(), target.path()) {
        Err(e @ RecordError::BrokenChain { .. }) => {
            assert_eq!(Refusal::from(e).reason(), "halt-state-unreadable");
        }
        other => panic!("expected a broken chain, got {:?}", other.map(|(_, r)| r)),
    }
}

#[test]
fn a_halt_during_a_live_attempt_is_stop_requested_until_cancelled_is_durable() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    let session = begin(&mut chain, target.path(), "r1", 0).unwrap();

    let applied = halt::halt(&mut chain, &request(&[Scope::RunAdmission], "t1")).unwrap();
    let target_of = applied
        .detail
        .stop_requested
        .clone()
        .expect("a live attempt was named");
    assert_eq!((target_of.run_id.as_str(), target_of.attempt), ("r1", 1));
    let stop = applied.state.stop_of("r1", 1).unwrap();
    assert_eq!(stop.word(), "stop-requested");
    // R-2: no reader reports the halt as effective on the attempt yet.
    assert!(session::live_attempt(&chain).is_some());
    assert_eq!(runs(&chain).len(), 1, "the halt entry is not a run");

    session::conclude(
        &mut chain,
        target.path(),
        &session,
        Outcome::Cancelled,
        &Accounting::default(),
        serde_json::json!({}),
        &FixedClock(2),
    )
    .unwrap();
    let state = halt::fold(&chain).unwrap();
    let stop = state.stop_of("r1", 1).unwrap();
    assert_eq!(stop.word(), "stop-confirmed");
    assert!(session::live_attempt(&chain).is_none());
    // Confirmed once: a second confirmation appends nothing.
    assert!(!halt::confirm_stop(&mut chain, "r1", 1, "t3").unwrap());
}

#[test]
fn an_attempt_that_ends_any_other_way_leaves_the_stop_requested_never_confirmed() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    let session = begin(&mut chain, target.path(), "r1", 0).unwrap();
    halt::halt(&mut chain, &request(&[Scope::ToolEffects], "t1")).unwrap();
    assert_eq!(
        halt::fold(&chain).unwrap().stop_of("r1", 1).unwrap().word(),
        "stop-requested"
    );
    session::conclude(
        &mut chain,
        target.path(),
        &session,
        Outcome::Failed,
        &Accounting::default(),
        serde_json::json!({}),
        &FixedClock(2),
    )
    .unwrap();
    let state = halt::fold(&chain).unwrap();
    assert_eq!(state.stop_of("r1", 1).unwrap().word(), "stop-requested");
    assert!(state.stop_of("r1", 1).unwrap().confirmed_at.is_none());
    // A dispatch halt names no attempt: it stops no process.
    let dispatch = halt::halt(&mut chain, &request(&[Scope::Dispatch], "t3")).unwrap();
    assert!(dispatch.detail.stop_requested.is_none());
}

// ---- Requests queued for a live supervisor (section 5, 2026-11-01) ----

#[test]
fn a_queued_request_is_outside_the_record_until_applied_then_appended_in_order_and_removed() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    let session = begin(&mut chain, target.path(), "r1", 0).unwrap();

    let first = halt::queue(
        home.path(),
        target.path(),
        "halt",
        &request(&[Scope::ToolEffects], "t1"),
    )
    .unwrap();
    let second = halt::queue(
        home.path(),
        target.path(),
        "resume",
        &request(&[Scope::Dispatch], "t2"),
    )
    .unwrap();
    // Beside the chain, in the product home, never in the target.
    assert_eq!(
        first.parent().unwrap(),
        halt::requests_dir(home.path(), target.path())
    );
    assert!(first.starts_with(home.path()));
    assert_eq!(
        chain_path(home.path(), target.path()).parent(),
        halt::requests_dir(home.path(), target.path()).parent()
    );
    // A request is not the halt state: the record says nothing yet.
    assert!(halt::fold(&chain).unwrap().halted.is_empty());
    let pending = halt::pending(home.path(), target.path()).unwrap();
    assert_eq!(
        pending.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
        vec![first.clone(), second.clone()]
    );

    let applied = halt::apply_pending(&mut chain, home.path(), target.path()).unwrap();
    assert_eq!(
        applied
            .iter()
            .map(|a| a.change.as_str())
            .collect::<Vec<_>>(),
        ["halt", "resume"]
    );
    assert!(!first.exists() && !second.exists());
    assert!(
        halt::pending(home.path(), target.path())
            .unwrap()
            .is_empty()
    );
    let state = halt::fold(&chain).unwrap();
    let h = state
        .of(Scope::ToolEffects)
        .expect("the queued halt is recorded");
    assert_eq!((h.operator.as_str(), h.at.as_str()), ("bart", "t1"));
    // Applied while the attempt was live, so it asked that attempt to stop.
    assert_eq!(state.stop_of("r1", 1).unwrap().word(), "stop-requested");
    // Durable in the record: a fresh open reads the same state.
    let (reopened, _) = Chain::open(home.path(), target.path()).unwrap();
    assert_eq!(halt::fold(&reopened).unwrap(), state);
    drop(session);
}

#[test]
fn a_request_that_does_not_read_refuses_with_halt_state_unreadable_and_appends_nothing() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    let good = halt::queue(
        home.path(),
        target.path(),
        "halt",
        &request(&[Scope::RunAdmission], "t1"),
    )
    .unwrap();
    let bad = halt::requests_dir(home.path(), target.path()).join("0-torn.json");
    std::fs::write(&bad, b"{\"change\":\"halt\",\"scop").unwrap();
    let before = chain.records().len();
    match halt::apply_pending(&mut chain, home.path(), target.path()) {
        Err(HaltError::Unreadable(r)) => assert_eq!(r.reason(), "halt-state-unreadable"),
        other => panic!("expected halt-state-unreadable, got {other:?}"),
    }
    assert_eq!(chain.records().len(), before, "nothing was appended");
    assert!(good.exists() && bad.exists(), "nothing was removed");
    // A hidden name is a request still being written, not a request.
    std::fs::remove_file(&bad).unwrap();
    std::fs::write(
        halt::requests_dir(home.path(), target.path()).join(".partial"),
        b"",
    )
    .unwrap();
    assert_eq!(halt::pending(home.path(), target.path()).unwrap().len(), 1);
}

#[test]
fn a_request_with_no_operator_or_reason_is_not_queued() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let empty = Request {
        scopes: &[],
        operator: " ",
        reason: "why",
        at: "t1",
    };
    assert!(matches!(
        halt::queue(home.path(), target.path(), "halt", &empty),
        Err(HaltError::Refused(_))
    ));
    assert!(!halt::requests_dir(home.path(), target.path()).exists());
}

#[test]
fn a_request_appended_but_not_removed_is_removed_and_never_appended_again() {
    let home = tempfile::tempdir().unwrap();
    let target = repo();
    let (mut chain, _) = Chain::open(home.path(), target.path()).unwrap();
    let halted = halt::queue(
        home.path(),
        target.path(),
        "halt",
        &request(&[Scope::Dispatch], "t1"),
    )
    .unwrap();
    // A writer appended the halt and then failed to remove its file.
    halt::halt(&mut chain, &request(&[Scope::Dispatch], "t1")).unwrap();
    let resumed = halt::queue(
        home.path(),
        target.path(),
        "resume",
        &request(&[Scope::Dispatch], "t2"),
    )
    .unwrap();
    let before = chain.records().len();

    let applied = halt::apply_pending(&mut chain, home.path(), target.path()).unwrap();
    // Only the resume is appended; the stale halt is not replayed over it.
    assert_eq!(
        applied
            .iter()
            .map(|a| a.change.as_str())
            .collect::<Vec<_>>(),
        ["resume"]
    );
    assert_eq!(chain.records().len(), before + 1);
    assert!(!halted.exists() && !resumed.exists());
    assert!(halt::fold(&chain).unwrap().halted.is_empty());
}
