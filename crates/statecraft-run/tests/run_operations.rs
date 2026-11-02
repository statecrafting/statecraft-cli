//! Spec 048 in the run crate: the lease, section 3.3's rules in order, the
//! record as the state, the abort's two points and the event cursor.
//!
//! V-1: a record built by a sequence of operations replays to the same state;
//! a repeated key appends nothing; a stale version is refused naming
//! `stale-version`; a reconnecting cursor reads exactly the events after it.

use serde_json::json;
use statecraft_run::attempt::Outcome;
use statecraft_run::operation::{
    self, Completion, Controller, Operation, Reason, Request, RunState, Stop, Submitted,
};
use statecraft_run::record::{Chain, Entry, Identity, Kind, chain_path};
use statecraft_run::session::{INTENT_SUBJECT, OUTCOME_SUBJECT};

const RUN: &str = "048-fixture";
const AT: &str = "2026-11-01T00:00:00Z";

struct Fixture {
    home: tempfile::TempDir,
    target: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        Self {
            home: tempfile::tempdir().unwrap(),
            target: tempfile::tempdir().unwrap(),
        }
    }

    fn chain(&self) -> Chain {
        Chain::open(self.home.path(), self.target.path()).unwrap().0
    }
}

fn append(
    chain: &mut Chain,
    kind: Kind,
    run: &str,
    attempt: u32,
    subject: &str,
    detail: serde_json::Value,
) {
    let id = format!("fixture/{}", chain.records().len());
    chain
        .append(
            &id,
            AT,
            &Entry {
                kind,
                run_id: run.to_string(),
                attempt,
                subject: subject.to_string(),
                idempotency_key: None,
                detail,
                effect_id: Identity::Absent,
            },
        )
        .unwrap();
}

/// An attempt's intent, as spec 003's session writes it: live until an
/// outcome follows.
fn start(chain: &mut Chain, run: &str, attempt: u32) {
    append(
        chain,
        Kind::Intent,
        run,
        attempt,
        INTENT_SUBJECT,
        json!({ "baseCommit": "aaa" }),
    );
}

fn end(chain: &mut Chain, run: &str, attempt: u32, outcome: Outcome) {
    append(
        chain,
        Kind::Outcome,
        run,
        attempt,
        OUTCOME_SUBJECT,
        json!({ "outcome": outcome.word() }),
    );
}

fn me() -> Controller<'static> {
    Controller {
        name: "cockpit",
        pid: std::process::id(),
    }
}

fn lease(chain: &mut Chain) -> u64 {
    operation::acquire(chain, RUN, &me(), AT)
        .unwrap()
        .unwrap()
        .lease
        .id
}

fn request<'a>(lease: u64, key: &'a str, version: u64, op: Operation) -> Request<'a> {
    Request {
        run_id: RUN,
        controller: "cockpit",
        lease,
        key,
        version,
        operation: op,
        reason: "fixture",
        at: AT,
    }
}

fn version(chain: &Chain) -> u64 {
    operation::fold(chain, RUN).version
}

/// A process id that has ended: a child that ran and was reaped.
fn ended_pid() -> u32 {
    let mut child = std::process::Command::new("true").spawn().unwrap();
    let pid = child.id();
    child.wait().unwrap();
    pid
}

/// The fold of the first `records` records, read from a fresh home.
fn replay(f: &Fixture, records: usize) -> RunState {
    let bytes = std::fs::read(chain_path(f.home.path(), f.target.path())).unwrap();
    let prefix: Vec<u8> = bytes
        .split_inclusive(|b| *b == b'\n')
        .take(records)
        .flatten()
        .copied()
        .collect();
    let home = tempfile::tempdir().unwrap();
    let path = chain_path(home.path(), f.target.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, prefix).unwrap();
    let (chain, _) = Chain::open(home.path(), f.target.path()).unwrap();
    operation::fold(&chain, RUN)
}

fn snapshot(chain: &Chain, seen: &mut Vec<(usize, RunState)>) {
    seen.push((chain.records().len(), operation::fold(chain, RUN)));
}

#[test]
fn a_record_built_by_a_sequence_of_operations_replays_to_the_same_state() {
    let f = Fixture::new();
    let mut chain = f.chain();
    let mut seen = Vec::new();
    start(&mut chain, RUN, 1);
    end(&mut chain, RUN, 1, Outcome::Failed);
    snapshot(&chain, &mut seen);

    let id = lease(&mut chain);
    snapshot(&chain, &mut seen);

    // A retry: intent, the attempt `run` appends, then the outcome.
    let v = version(&chain);
    let Submitted::Accepted(retry) =
        operation::submit(&mut chain, &request(id, "k-retry", v, Operation::Retry)).unwrap()
    else {
        panic!("the retry was not accepted");
    };
    snapshot(&chain, &mut seen);
    start(&mut chain, RUN, 2);
    end(&mut chain, RUN, 2, Outcome::Completed);
    let second = operation::fold(&chain, RUN).attempts[1].clone();
    operation::complete(
        &mut chain,
        &retry,
        Completion::Launched {
            attempt: Some(&second),
            exit: 0,
        },
        AT,
    )
    .unwrap();
    snapshot(&chain, &mut seen);

    // A stale version is recorded as a refusal.
    let refused =
        operation::submit(&mut chain, &request(id, "k-stale", 1, Operation::Abort)).unwrap();
    assert!(matches!(refused, Submitted::Refused(ref r) if r.recorded));
    snapshot(&chain, &mut seen);

    // An abort of a live attempt, then its cancelled outcome and confirmation.
    start(&mut chain, RUN, 3);
    let v = version(&chain);
    let Submitted::Accepted(abort) =
        operation::submit(&mut chain, &request(id, "k-abort", v, Operation::Abort)).unwrap()
    else {
        panic!("the abort was not accepted");
    };
    operation::complete(&mut chain, &abort, Completion::StopRequested, AT).unwrap();
    snapshot(&chain, &mut seen);
    end(&mut chain, RUN, 3, Outcome::Cancelled);
    assert_eq!(operation::confirm_stop(&mut chain, RUN, 3, AT).unwrap(), 1);
    snapshot(&chain, &mut seen);

    operation::release(&mut chain, RUN, "cockpit", id, AT)
        .unwrap()
        .unwrap();
    snapshot(&chain, &mut seen);

    // I-1: every intermediate state is the fold of the record up to it, read
    // back from disk in a home that never saw the operations happen.
    for (records, state) in &seen {
        assert_eq!(&replay(&f, *records), state, "after {records} records");
    }
    let last = &seen.last().unwrap().1;
    assert_eq!(operation::fold(&f.chain(), RUN), *last);
    assert!(last.lease.is_none());
    assert_eq!(last.attempts.len(), 3);
    assert_eq!(last.operations.len(), 3);
    assert_eq!(last.operations[2].stop, Some(Stop::StopConfirmed));
    // The brackets close as every effect's do (spec 003 section 3.3.1).
    let effects = statecraft_run::fold_effects(&chain);
    assert!(effects.open().is_empty() && effects.is_clean());
}

#[test]
fn a_repeated_key_appends_nothing_and_answers_what_the_record_holds() {
    let f = Fixture::new();
    let mut chain = f.chain();
    start(&mut chain, RUN, 1);
    let id = lease(&mut chain);
    let v = version(&chain);
    let Submitted::Accepted(abort) =
        operation::submit(&mut chain, &request(id, "k1", v, Operation::Abort)).unwrap()
    else {
        panic!("not accepted");
    };
    let outcome = operation::complete(&mut chain, &abort, Completion::StopRequested, AT).unwrap();
    let records = chain.records().len();

    // Rule 2: the same key, operation and version, even after the version
    // moved, is answered from the record.
    match operation::submit(&mut chain, &request(id, "k1", v, Operation::Abort)).unwrap() {
        Submitted::Replayed(held) => {
            assert_eq!(held.outcome, Some(outcome));
            assert_eq!(held.stop, Some(Stop::StopRequested));
        }
        other => panic!("expected a replay, got {other:?}"),
    }
    assert_eq!(chain.records().len(), records);

    // Rule 1: the same key with another version or operation is refused.
    for (version, op) in [(v + 1, Operation::Abort), (v, Operation::Retry)] {
        match operation::submit(&mut chain, &request(id, "k1", version, op)).unwrap() {
            Submitted::Refused(r) => {
                assert_eq!(r.reason, Some(Reason::KeyConflict));
                assert!(!r.recorded);
            }
            other => panic!("expected key-conflict, got {other:?}"),
        }
    }
    assert_eq!(chain.records().len(), records);
}

#[test]
fn a_stale_version_is_refused_naming_both_versions_and_only_the_refusal_is_recorded() {
    let f = Fixture::new();
    let mut chain = f.chain();
    start(&mut chain, RUN, 1);
    let id = lease(&mut chain);
    let current = version(&chain);
    let records = chain.records().len();
    match operation::submit(
        &mut chain,
        &request(id, "k1", current - 1, Operation::Abort),
    )
    .unwrap()
    {
        Submitted::Refused(r) => {
            assert_eq!(r.reason, Some(Reason::StaleVersion));
            assert_eq!(r.word(), "stale-version");
            assert!(r.recorded);
            assert_eq!(r.expected_version, Some(current - 1));
            assert_eq!(r.current_version, Some(current));
        }
        other => panic!("expected stale-version, got {other:?}"),
    }
    assert_eq!(chain.records().len(), records + 1);
    let last = chain.entries().pop().unwrap();
    assert_eq!(last.kind, Kind::Accounting);
    assert_eq!(last.subject, operation::REFUSED_SUBJECT);
    // The effect is not: no operation intent, and the attempt is still live.
    assert!(
        !chain
            .entries()
            .iter()
            .any(|e| e.kind == Kind::Intent && e.subject == operation::OPERATION_SUBJECT)
    );
    assert!(operation::fold(&chain, RUN).attempts[0].live());
    // The key now holds the refusal, and a resend is answered with it.
    match operation::submit(
        &mut chain,
        &request(id, "k1", current - 1, Operation::Abort),
    )
    .unwrap()
    {
        Submitted::Replayed(held) => {
            assert_eq!(held.refusal.unwrap().reason, Some(Reason::StaleVersion));
        }
        other => panic!("expected the recorded refusal, got {other:?}"),
    }
    assert_eq!(chain.records().len(), records + 1);
}

#[test]
fn without_the_lease_a_caller_learns_nothing_about_keys_and_appends_nothing() {
    let f = Fixture::new();
    let mut chain = f.chain();
    start(&mut chain, RUN, 1);
    let id = lease(&mut chain);
    let v = version(&chain);
    operation::submit(&mut chain, &request(id, "k1", v, Operation::Abort)).unwrap();
    let records = chain.records().len();
    // A held key, a stale version and a wrong lease: rule 0 answers first.
    for (controller, lease) in [("intruder", id), ("cockpit", id + 1)] {
        let mut r = request(lease, "k1", 0, Operation::Retry);
        r.controller = controller;
        match operation::submit(&mut chain, &r).unwrap() {
            Submitted::Refused(r) => {
                assert_eq!(r.reason, Some(Reason::LeaseNotHeld));
                assert!(!r.detail.contains("k1"), "{}", r.detail);
            }
            other => panic!("expected lease-not-held, got {other:?}"),
        }
    }
    assert_eq!(chain.records().len(), records);
}

#[test]
fn a_second_controller_is_refused_lease_held_until_the_holder_ends() {
    let f = Fixture::new();
    let mut chain = f.chain();
    start(&mut chain, RUN, 1);
    end(&mut chain, RUN, 1, Outcome::Failed);
    // No attempt, no lease.
    assert!(
        operation::acquire(&mut chain, "no-such-run", &me(), AT)
            .unwrap()
            .is_err()
    );

    let gone = Controller {
        name: "gone",
        pid: ended_pid(),
    };
    let first = operation::acquire(&mut chain, RUN, &gone, AT)
        .unwrap()
        .unwrap();
    // Its process has ended and the run has no live attempt: the lease has
    // ended, which is recorded before the new one.
    let second = operation::acquire(&mut chain, RUN, &me(), AT)
        .unwrap()
        .unwrap();
    assert_eq!(second.change, "acquired");
    assert_eq!(second.expired.as_ref().map(|l| l.id), Some(first.lease.id));
    // The same controller asking again keeps it and appends nothing.
    let records = chain.records().len();
    let again = operation::acquire(&mut chain, RUN, &me(), AT)
        .unwrap()
        .unwrap();
    assert_eq!(
        (again.change, again.lease.id),
        ("already-held", second.lease.id)
    );
    assert_eq!(chain.records().len(), records);
    // A live holder refuses another controller, naming itself.
    let other = Controller {
        name: "other",
        pid: std::process::id(),
    };
    let refused = operation::acquire(&mut chain, RUN, &other, AT)
        .unwrap()
        .unwrap_err();
    assert_eq!(refused.reason, Some(Reason::LeaseHeld));
    assert!(refused.detail.contains("cockpit"), "{}", refused.detail);
    // An operator breaks it, and the break is recorded with the operator.
    let broken = operation::break_lease(&mut chain, RUN, "bart", "controller hung", AT)
        .unwrap()
        .unwrap();
    assert_eq!(broken.change, "broken");
    assert!(operation::fold(&chain, RUN).lease.is_none());
    let last = chain.entries().pop().unwrap();
    assert_eq!(last.detail["operator"], "bart");
    assert!(
        operation::acquire(&mut chain, RUN, &other, AT)
            .unwrap()
            .is_ok()
    );
}

#[test]
fn a_holder_that_ended_keeps_the_lease_while_the_run_has_a_live_attempt() {
    let f = Fixture::new();
    let mut chain = f.chain();
    start(&mut chain, RUN, 1);
    let gone = Controller {
        name: "gone",
        pid: ended_pid(),
    };
    operation::acquire(&mut chain, RUN, &gone, AT)
        .unwrap()
        .unwrap();
    let refused = operation::acquire(&mut chain, RUN, &me(), AT)
        .unwrap()
        .unwrap_err();
    assert_eq!(refused.reason, Some(Reason::LeaseHeld));
}

#[test]
fn an_abort_is_stop_requested_and_confirmed_only_by_a_cancelled_outcome() {
    for (outcome, confirmed) in [(Outcome::Failed, false), (Outcome::Cancelled, true)] {
        let f = Fixture::new();
        let mut chain = f.chain();
        start(&mut chain, RUN, 1);
        let id = lease(&mut chain);
        let v = version(&chain);
        let Submitted::Accepted(abort) =
            operation::submit(&mut chain, &request(id, "k1", v, Operation::Abort)).unwrap()
        else {
            panic!("not accepted");
        };
        operation::complete(&mut chain, &abort, Completion::StopRequested, AT).unwrap();
        // Requested is not done: the attempt is still live.
        assert!(operation::fold(&chain, RUN).attempts[0].live());
        assert_eq!(operation::confirm_stop(&mut chain, RUN, 1, AT).unwrap(), 0);
        end(&mut chain, RUN, 1, outcome);
        operation::confirm_stop(&mut chain, RUN, 1, AT).unwrap();
        let stop = operation::fold(&chain, RUN).operations[0].stop;
        let want = if confirmed {
            Stop::StopConfirmed
        } else {
            Stop::StopRequested
        };
        assert_eq!(stop, Some(want), "{outcome:?}");
    }
}

#[test]
fn concluding_an_attempt_cancelled_confirms_the_stop_asked_of_it() {
    use statecraft_environment::time::FixedClock;
    let f = Fixture::new();
    let target = f.target.path();
    for args in [
        &["init", "--quiet"][..],
        &["config", "user.email", "t@example.com"],
        &["config", "user.name", "t"],
        &["commit", "--quiet", "--allow-empty", "-m", "one"],
    ] {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(target)
                .status()
                .unwrap()
                .success()
        );
    }
    let mut chain = f.chain();
    let session =
        statecraft_run::session::begin(&mut chain, target, RUN, "HEAD", &FixedClock(0)).unwrap();
    let id = lease(&mut chain);
    let v = version(&chain);
    let Submitted::Accepted(abort) =
        operation::submit(&mut chain, &request(id, "k1", v, Operation::Abort)).unwrap()
    else {
        panic!("not accepted");
    };
    operation::complete(&mut chain, &abort, Completion::StopRequested, AT).unwrap();
    statecraft_run::session::conclude(
        &mut chain,
        target,
        &session,
        Outcome::Cancelled,
        &statecraft_run::refusal::Accounting::default(),
        json!({}),
        &FixedClock(1),
    )
    .unwrap();
    let state = operation::fold(&chain, RUN);
    assert_eq!(state.attempts[0].outcome, Some(Outcome::Cancelled));
    assert_eq!(state.operations[0].stop, Some(Stop::StopConfirmed));
    // Confirmed after the outcome, as two records in that order.
    let entries = chain.entries();
    let outcome = entries
        .iter()
        .position(|e| e.kind == Kind::Outcome && e.subject == OUTCOME_SUBJECT)
        .unwrap();
    let confirmed = entries
        .iter()
        .position(|e| e.subject == operation::STOP_CONFIRMED_SUBJECT)
        .unwrap();
    assert!(confirmed > outcome);
}

#[test]
fn continue_is_refused_for_every_adapter_and_a_launch_needs_the_slot() {
    let f = Fixture::new();
    let mut chain = f.chain();
    start(&mut chain, RUN, 1);
    end(&mut chain, RUN, 1, Outcome::Failed);
    let id = lease(&mut chain);
    let v = version(&chain);
    match operation::submit(&mut chain, &request(id, "k1", v, Operation::Continue)).unwrap() {
        Submitted::Refused(r) => assert_eq!(r.reason, Some(Reason::CapabilityUndeclared)),
        other => panic!("expected capability-undeclared, got {other:?}"),
    }
    // Another run's live attempt holds the repository's one slot.
    start(&mut chain, "other-run", 1);
    let v = version(&chain);
    match operation::submit(&mut chain, &request(id, "k2", v, Operation::Retry)).unwrap() {
        Submitted::Refused(r) => {
            assert_eq!(r.reason, None);
            assert!(
                r.detail.contains("other-run attempt 1 is live"),
                "{}",
                r.detail
            );
        }
        other => panic!("expected the slot refusal, got {other:?}"),
    }
    // Neither appended anything, so neither key is held.
    assert!(operation::fold(&chain, RUN).operations.is_empty());
}

#[test]
fn a_reconnecting_cursor_reads_exactly_the_events_after_it() {
    let f = Fixture::new();
    let mut chain = f.chain();
    start(&mut chain, RUN, 1);
    start(&mut chain, "other-run", 1);
    end(&mut chain, RUN, 1, Outcome::Failed);
    lease(&mut chain);

    let first = operation::events(&chain, RUN, None);
    assert_eq!(first.events.len(), 3);
    assert!(first.events.iter().all(|e| e.entry.run_id == RUN));
    let cursor = first.events[1].cursor;
    let held = first.next.unwrap();

    // The connection drops; more happens; the controller resumes.
    end(&mut chain, "other-run", 1, Outcome::Failed);
    start(&mut chain, RUN, 2);
    let reopened = f.chain();
    let resumed = operation::events(&reopened, RUN, Some(cursor));
    let cursors: Vec<u64> = resumed.events.iter().map(|e| e.cursor).collect();
    let expected: Vec<u64> = operation::events(&reopened, RUN, None)
        .events
        .iter()
        .map(|e| e.cursor)
        .filter(|c| *c > cursor)
        .collect();
    assert_eq!(cursors, expected);
    assert_eq!(cursors.len(), 2);

    // From the last cursor held: only what is new, nothing repeated.
    let from_held = operation::events(&reopened, RUN, Some(held));
    assert_eq!(from_held.events.len(), 1);
    assert_eq!(from_held.events[0].entry.attempt, 2);
    let tail = operation::events(&reopened, RUN, from_held.next);
    assert!(tail.events.is_empty());
    assert_eq!(tail.next, from_held.next);
}

#[test]
fn operation_records_do_not_change_the_attempts_spec_003_folds() {
    let f = Fixture::new();
    let mut chain = f.chain();
    start(&mut chain, RUN, 1);
    let before = statecraft_run::session::runs(&chain);
    let id = lease(&mut chain);
    operation::submit(&mut chain, &request(id, "stale", 0, Operation::Abort)).unwrap();
    let v = version(&chain);
    operation::submit(&mut chain, &request(id, "k1", v, Operation::Abort)).unwrap();
    assert_eq!(statecraft_run::session::runs(&chain), before);
    assert!(
        chain
            .entries()
            .iter()
            .filter(|e| e.subject != INTENT_SUBJECT)
            .all(operation::is_operation_record)
    );
    // An open operation is an open effect with its own identity, and the
    // legacy pairing of spec 003 never sees it.
    assert_eq!(statecraft_run::fold_effects(&chain).open().len(), 1);
    assert!(
        statecraft_run::recovery::unmatched_intents(&chain)
            .iter()
            .all(|u| u.subject == INTENT_SUBJECT)
    );
}

// ---- Section 5, 2026-11-02: requests queued while a live run holds the lock ----

fn queued_abort(lease: u64, version: u64) -> operation::Queued {
    operation::Queued::Abort {
        run_id: RUN.to_string(),
        controller: "cockpit".to_string(),
        lease,
        key: "k-queued".to_string(),
        version,
        reason: "fixture".to_string(),
        at: AT.to_string(),
    }
}

#[test]
fn a_queued_abort_is_decided_by_the_same_rules_and_applied_twice_appends_once() {
    let f = Fixture::new();
    let mut chain = f.chain();
    start(&mut chain, RUN, 1);
    let held = lease(&mut chain);
    let request = queued_abort(held, version(&chain));
    let file = operation::queue(f.home.path(), f.target.path(), &request).unwrap();
    // Beside the chain, in the product home, never in the target.
    assert_eq!(
        file.parent().unwrap(),
        operation::requests_dir(f.home.path(), f.target.path())
    );
    assert_eq!(
        chain_path(f.home.path(), f.target.path()).parent(),
        operation::requests_dir(f.home.path(), f.target.path()).parent()
    );

    let applied = operation::apply_pending(&mut chain, f.home.path(), f.target.path())
        .unwrap()
        .applied;
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].answer["outcome"]["result"], "stop-requested");
    assert!(!file.exists());
    let stop = operation::stop_requested_of(&chain, RUN, 1).expect("the abort asks the stop");
    assert_eq!(
        (stop.key.as_str(), stop.stop),
        ("k-queued", Some(Stop::StopRequested))
    );

    // A writer that appended it and could not remove its file: applying it
    // again replays the key and appends nothing.
    let records = chain.records().len();
    let answer = operation::apply(&mut chain, &request).unwrap();
    assert!(answer.get("replayed").is_some(), "{answer}");
    assert_eq!(chain.records().len(), records);
}

#[test]
fn a_queued_break_left_behind_is_not_recorded_again_over_a_newer_lease() {
    let f = Fixture::new();
    let mut chain = f.chain();
    start(&mut chain, RUN, 1);
    lease(&mut chain);
    let request = operation::Queued::LeaseBreak {
        run_id: RUN.to_string(),
        operator: "alice".to_string(),
        reason: "stuck".to_string(),
        at: AT.to_string(),
    };
    let answer = operation::apply(&mut chain, &request).unwrap();
    assert_eq!(answer["lease"]["change"], "broken", "{answer}");
    // A newer lease is taken, then the left file is applied again.
    let newer = lease(&mut chain);
    let answer = operation::apply(&mut chain, &request).unwrap();
    assert_eq!(answer["alreadyApplied"], true, "{answer}");
    assert_eq!(
        operation::fold(&chain, RUN).lease.map(|l| l.id),
        Some(newer)
    );
}

#[test]
fn a_queued_request_that_does_not_read_is_left_in_place_and_stops_nothing() {
    let f = Fixture::new();
    let mut chain = f.chain();
    start(&mut chain, RUN, 1);
    let dir = operation::requests_dir(f.home.path(), f.target.path());
    std::fs::create_dir_all(&dir).unwrap();
    // Sorts first, so a request behind it must still be applied.
    let bad = dir.join("0-torn.json");
    std::fs::write(&bad, b"{\"request\":\"abort\",\"run").unwrap();
    let good = operation::queue(
        f.home.path(),
        f.target.path(),
        &operation::Queued::LeaseAcquire {
            run_id: RUN.to_string(),
            controller: "cockpit".to_string(),
            pid: std::process::id(),
            at: AT.to_string(),
        },
    )
    .unwrap();
    let drained = operation::apply_pending(&mut chain, f.home.path(), f.target.path()).unwrap();
    assert_eq!(drained.unreadable.len(), 1, "{:?}", drained.unreadable);
    assert!(drained.unreadable[0].contains("0-torn.json"));
    assert_eq!(drained.applied.len(), 1);
    assert!(bad.exists(), "an unreadable request is left in place");
    assert!(!good.exists());
    assert!(operation::fold(&chain, RUN).lease.is_some());
}
