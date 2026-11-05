//! Spec 053: operator input recorded before delivery, its states, its expiry,
//! the cockpit's control grant, and the input as evidence on its attempt.
//!
//! Every test folds a chain re-opened from disk, so what it asserts is what
//! the record holds, not what a process remembers.

use serde_json::json;
use statecraft_run::input::{
    self, ATTEMPT_ENDED, CHILD_ATTESTED, Desk, EXPIRY_PASSED, Grant, Inbox, Input, InputError,
    InputKind, Origin, Placement, State, Submission,
};
use statecraft_run::record::{Chain, Entry, Identity, Kind};
use statecraft_run::session::{self, INTENT_SUBJECT, OUTCOME_SUBJECT};
use std::path::PathBuf;

const NOW: i64 = 1_793_000_000;
const RUN: &str = "run-1";

struct Fixture {
    home: tempfile::TempDir,
    target: tempfile::TempDir,
}

impl Fixture {
    /// A repository whose run `run-1` has a live attempt 1.
    fn live() -> Self {
        let f = Self {
            home: tempfile::tempdir().unwrap(),
            target: tempfile::tempdir().unwrap(),
        };
        let mut chain = f.open();
        chain
            .append(
                "run-1/1/intent",
                "t0",
                &entry(Kind::Intent, INTENT_SUBJECT, json!({ "baseCommit": "abc" })),
            )
            .unwrap();
        f
    }

    fn open(&self) -> Chain {
        Chain::open(self.home.path(), self.target.path()).unwrap().0
    }

    fn chain_file(&self) -> PathBuf {
        statecraft_run::record::chain_path(self.home.path(), self.target.path())
    }

    fn conclude(&self) {
        self.open()
            .append(
                "run-1/1/outcome",
                "t9",
                &entry(
                    Kind::Outcome,
                    OUTCOME_SUBJECT,
                    json!({ "outcome": "completed" }),
                ),
            )
            .unwrap();
    }
}

fn entry(kind: Kind, subject: &str, detail: serde_json::Value) -> Entry {
    Entry {
        kind,
        run_id: RUN.to_string(),
        attempt: 1,
        subject: subject.to_string(),
        idempotency_key: None,
        detail,
        effect_id: Identity::Absent,
    }
}

fn note(origin: Origin, expires_at: i64) -> Submission {
    Submission {
        kind: InputKind::Note,
        body: "the fix is in the parser, not the lexer".to_string(),
        author: "bart".to_string(),
        reason: "the agent is editing the wrong module".to_string(),
        run_id: RUN.to_string(),
        attempt: 1,
        expires_at,
        origin,
    }
}

/// A scripted inbox: what a channel from another process would hand over.
struct Scripted(Vec<Submission>);

impl Inbox for Scripted {
    fn take(&mut self) -> Vec<Submission> {
        std::mem::take(&mut self.0)
    }
}

/// A placement that checks I-1 at the moment of placing: the input's pending
/// record is already in the chain file on disk.
struct Checked {
    chain_file: PathBuf,
    placed: Vec<String>,
    withdrawn: Vec<String>,
    returned: Vec<String>,
}

impl Checked {
    fn new(f: &Fixture) -> Self {
        Self {
            chain_file: f.chain_file(),
            placed: Vec::new(),
            withdrawn: Vec::new(),
            returned: Vec::new(),
        }
    }
}

impl Placement for Checked {
    fn place(&mut self, input: &Input) -> Result<(), String> {
        let on_disk = std::fs::read_to_string(&self.chain_file).unwrap();
        assert!(
            on_disk.contains(input::INPUT_SUBJECT) && on_disk.contains(&input.id),
            "I-1: {} was placed before its pending record was durable",
            input.id
        );
        self.placed.push(input.id.clone());
        Ok(())
    }
    fn withdraw(&mut self, id: &str) -> Result<(), String> {
        self.withdrawn.push(id.to_string());
        Ok(())
    }
    fn returned(&mut self) -> Vec<String> {
        self.returned.clone()
    }
}

fn states(f: &Fixture) -> Vec<(State, Option<String>, Option<String>)> {
    let fold = input::fold(&f.open());
    assert!(fold.defects.is_empty(), "{:?}", fold.defects);
    fold.inputs[0]
        .history
        .iter()
        .map(|t| (t.state, t.attestation.clone(), t.why.clone()))
        .collect()
}

#[test]
fn a_note_is_recorded_pending_then_delivered_then_acknowledged_child_attested() {
    let f = Fixture::live();
    let mut chain = f.open();
    let mut place = Checked::new(&f);
    let mut inbox = Scripted(vec![note(Origin::Cli, NOW + 600)]);
    let mut desk = Desk::new(RUN, 1);

    let tick = desk.tick(&mut chain, &mut place, &mut inbox, NOW, "t1");
    assert!(tick.failures.is_empty(), "{:?}", tick.failures);
    assert_eq!(tick.recorded, 2, "pending and delivered");
    assert_eq!(place.placed, ["input-000001"]);

    // The gate returned it at the next governed call.
    place.returned = vec!["input-000001".to_string()];
    let tick = desk.tick(&mut chain, &mut place, &mut Scripted(vec![]), NOW + 1, "t2");
    assert_eq!(tick.recorded, 1);
    assert_eq!(place.withdrawn, ["input-000001"]);
    assert_eq!(
        states(&f),
        [
            (State::Pending, None, None),
            (State::Delivered, None, None),
            (State::Acknowledged, Some(CHILD_ATTESTED.to_string()), None),
        ]
    );
}

#[test]
fn a_note_no_call_reads_before_its_expiry_expires_and_is_withdrawn() {
    let f = Fixture::live();
    let mut chain = f.open();
    let mut place = Checked::new(&f);
    let mut desk = Desk::new(RUN, 1);
    desk.tick(
        &mut chain,
        &mut place,
        &mut Scripted(vec![note(Origin::Cli, NOW + 10)]),
        NOW,
        "t1",
    );
    // Before its expiry and unreturned, nothing moves.
    let quiet = desk.tick(&mut chain, &mut place, &mut Scripted(vec![]), NOW + 9, "t2");
    assert_eq!(quiet.recorded, 0);
    let tick = desk.tick(
        &mut chain,
        &mut place,
        &mut Scripted(vec![]),
        NOW + 10,
        "t3",
    );
    assert_eq!(tick.recorded, 1);
    assert_eq!(place.withdrawn, ["input-000001"]);
    assert_eq!(
        states(&f).last().unwrap(),
        &(State::Expired, None, Some(EXPIRY_PASSED.to_string()))
    );
}

#[test]
fn a_delivered_note_expires_when_its_attempt_ends_unread() {
    let f = Fixture::live();
    let mut chain = f.open();
    let mut place = Checked::new(&f);
    let mut desk = Desk::new(RUN, 1);
    desk.tick(
        &mut chain,
        &mut place,
        &mut Scripted(vec![note(Origin::Cli, NOW + 600)]),
        NOW,
        "t1",
    );
    let tick = desk.close(&mut chain, &mut place, NOW + 5, "t2");
    assert_eq!(tick.recorded, 1);
    assert_eq!(
        states(&f).last().unwrap(),
        &(State::Expired, None, Some(ATTEMPT_ENDED.to_string()))
    );
}

#[test]
fn a_note_for_another_attempt_or_a_concluded_one_is_refused_and_not_recorded() {
    let f = Fixture::live();
    let mut chain = f.open();
    let mut other = note(Origin::Cli, NOW + 600);
    other.attempt = 2;
    let tick = Desk::new(RUN, 1).tick(
        &mut chain,
        &mut Checked::new(&f),
        &mut Scripted(vec![other]),
        NOW,
        "t1",
    );
    assert_eq!(tick.recorded, 0);
    assert_eq!(tick.failures.len(), 1);

    f.conclude();
    let mut chain = f.open();
    let refused = input::record_pending(&mut chain, &note(Origin::Cli, NOW + 600), NOW, "t2");
    assert!(
        matches!(refused, Err(InputError::NoAttempt { .. })),
        "{refused:?}"
    );
    assert!(input::fold(&f.open()).inputs.is_empty());
}

#[test]
fn a_cockpit_input_without_a_live_grant_is_refused_and_nothing_is_appended() {
    let f = Fixture::live();
    let cockpit = Origin::Cockpit {
        session: "s-1".to_string(),
    };
    let mut chain = f.open();
    let before = chain.records().len();
    let refused = input::record_pending(&mut chain, &note(cockpit.clone(), NOW + 600), NOW, "t1")
        .unwrap_err();
    assert_eq!(refused.reason(), "control-not-granted");
    let mut command = note(cockpit.clone(), NOW + 600);
    command.kind = InputKind::Command;
    command.body = "abort".to_string();
    let refused = input::record_pending(&mut chain, &command, NOW, "t1").unwrap_err();
    assert_eq!(refused.reason(), "control-not-granted");
    assert_eq!(f.open().records().len(), before);

    // Granted for ten minutes by an operator at the CLI: accepted.
    let granted = Grant {
        session: "s-1".to_string(),
        operator: "bart".to_string(),
        reason: "pairing on the parser fix".to_string(),
        granted_at: NOW,
        until: NOW + 600,
    };
    input::grant(&mut chain, &granted, "t2").unwrap();
    assert_eq!(input::live_grant(&chain, "s-1", NOW + 1), Some(granted));
    assert_eq!(input::live_grant(&chain, "s-2", NOW + 1), None);
    input::record_pending(&mut chain, &note(cockpit.clone(), NOW + 600), NOW + 1, "t3").unwrap();

    // Past its end, and after a revocation, it grants nothing.
    assert_eq!(input::live_grant(&chain, "s-1", NOW + 600), None);
    input::revoke(&mut chain, "s-1", "bart", "done", NOW + 2, "t4").unwrap();
    let refused =
        input::record_pending(&mut chain, &note(cockpit, NOW + 600), NOW + 3, "t5").unwrap_err();
    assert_eq!(refused.reason(), "control-not-granted");

    // A grant is the repository's, not a run's.
    let runs = session::runs(&f.open());
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, RUN);
}

#[test]
fn a_grant_longer_than_an_hour_or_not_positive_is_refused() {
    let f = Fixture::live();
    let mut chain = f.open();
    for until in [NOW + input::MAX_GRANT_SECONDS + 1, NOW, NOW - 1] {
        let g = Grant {
            session: "s-1".to_string(),
            operator: "bart".to_string(),
            reason: "r".to_string(),
            granted_at: NOW,
            until,
        };
        let refused = input::grant(&mut chain, &g, "t1").unwrap_err();
        assert_eq!(refused.reason(), "invalid-input");
    }
    let exactly_an_hour = Grant {
        session: "s-1".to_string(),
        operator: "bart".to_string(),
        reason: "r".to_string(),
        granted_at: NOW,
        until: NOW + input::MAX_GRANT_SECONDS,
    };
    input::grant(&mut chain, &exactly_an_hour, "t1").unwrap();
}

#[test]
fn only_the_moves_r1_names_are_recorded_and_a_forged_one_is_a_defect() {
    let f = Fixture::live();
    let mut chain = f.open();
    let id = input::record_pending(&mut chain, &note(Origin::Cli, NOW + 600), NOW, "t1").unwrap();
    let skipped = input::acknowledge(&mut chain, &id, "t2").unwrap_err();
    assert_eq!(skipped.reason(), "illegal-transition");
    assert!(input::acknowledge(&mut chain, "input-999999", "t2").is_err());

    // A record that skips `delivered` is reported, never applied.
    chain
        .append(
            "forged",
            "t3",
            &entry(
                Kind::Evidence,
                input::STATE_SUBJECT,
                json!({ "id": id, "state": "acknowledged", "attestation": CHILD_ATTESTED }),
            ),
        )
        .unwrap();
    let fold = input::fold(&f.open());
    assert_eq!(fold.inputs[0].state, State::Pending);
    assert_eq!(fold.defects.len(), 1, "{:?}", fold.defects);

    input::deliver(&mut chain, &id, "t4").unwrap();
    input::expire(&mut chain, &id, EXPIRY_PASSED, "t5").unwrap();
    assert!(
        input::deliver(&mut chain, &id, "t6").is_err(),
        "expired is final"
    );
}

#[test]
fn an_empty_field_or_a_past_expiry_is_refused() {
    let f = Fixture::live();
    let mut chain = f.open();
    for broken in [
        Submission {
            author: " ".to_string(),
            ..note(Origin::Cli, NOW + 600)
        },
        Submission {
            reason: String::new(),
            ..note(Origin::Cli, NOW + 600)
        },
        Submission {
            body: String::new(),
            ..note(Origin::Cli, NOW + 600)
        },
        note(Origin::Cli, NOW),
    ] {
        let refused = input::record_pending(&mut chain, &broken, NOW, "t1").unwrap_err();
        assert_eq!(refused.reason(), "invalid-input");
    }
    assert!(input::fold(&f.open()).inputs.is_empty());
}

#[test]
fn every_input_is_evidence_on_its_attempt_with_author_and_reason() {
    let f = Fixture::live();
    let mut chain = f.open();
    input::record_pending(&mut chain, &note(Origin::Cli, NOW + 600), NOW, "t1").unwrap();
    let mut command = note(Origin::Cli, NOW + 600);
    command.kind = InputKind::Command;
    command.body = "now run the parser tests".to_string();
    input::record_pending(&mut chain, &command, NOW, "t2").unwrap();
    f.conclude();

    let fold = input::fold(&f.open());
    let on_attempt: Vec<&Input> = fold.for_attempt(RUN, 1).collect();
    assert_eq!(on_attempt.len(), 2);
    for i in &on_attempt {
        assert_eq!(i.submission.author, "bart");
        assert_eq!(i.submission.reason, "the agent is editing the wrong module");
    }
    assert_eq!(on_attempt[1].submission.kind, InputKind::Command);
    // Evidence, not an effect: no intent is left open, and the attempt's own
    // outcome is what the run fold reads.
    assert!(
        statecraft_run::recovery::unmatched_intents(&f.open())
            .iter()
            .all(|u| !u.subject.starts_with("operator-input"))
    );
    let runs = session::runs(&f.open());
    assert_eq!(
        runs[0].attempts[0].outcome,
        Some(statecraft_run::Outcome::Completed)
    );
}

// ---- Section 5, 2026-11-02: the request file a live supervisor takes ----

/// A request queued while the lock is held is recorded `pending` by the next
/// writer, its file removed once the record is durable; the same request
/// left behind is taken again and appends nothing.
#[test]
fn a_queued_note_is_recorded_once_even_when_its_file_is_left_behind() {
    let f = Fixture::live();
    let (home, target) = (f.home.path(), f.target.path());
    let request = input::Request {
        submission: note(Origin::Cli, NOW + 60),
        operation: None,
    };
    let file = input::queue(home, target, &request).unwrap();
    assert!(file.starts_with(home.join("records")), "{}", file.display());
    assert!(file.starts_with(input::requests_dir(home, target)));

    let mut chain = f.open();
    let drained = input::apply_pending(&mut chain, home, target, NOW, "t1").unwrap();
    assert_eq!(drained.recorded, 1);
    assert_eq!(drained.applied[0].taken.id.as_deref(), Some("input-000001"));
    assert!(!file.exists(), "a taken request is removed");
    assert_eq!(states(&f), [(State::Pending, None, None)]);

    // A writer that could not remove the file: it is still queued.
    let left = input::queue(home, target, &request).unwrap();
    let before = f.open().records().len();
    let mut chain = f.open();
    let drained = input::apply_pending(&mut chain, home, target, NOW + 1, "t2").unwrap();
    assert_eq!(drained.recorded, 0);
    assert!(drained.applied[0].taken.already_recorded);
    assert!(!left.exists());
    assert_eq!(f.open().records().len(), before, "nothing appended again");
}

/// A file that does not read is skipped, reported and left in place; the
/// requests after it are still taken.
#[test]
fn an_unreadable_request_is_skipped_reported_and_left_in_place() {
    let f = Fixture::live();
    let (home, target) = (f.home.path(), f.target.path());
    let dir = input::requests_dir(home, target);
    std::fs::create_dir_all(&dir).unwrap();
    let junk = dir.join("0-junk.json");
    std::fs::write(&junk, b"{ not a request").unwrap();
    input::queue(
        home,
        target,
        &input::Request {
            submission: note(Origin::Cli, NOW + 60),
            operation: None,
        },
    )
    .unwrap();
    let mut chain = f.open();
    let drained = input::apply_pending(&mut chain, home, target, NOW, "t1").unwrap();
    assert_eq!(drained.unreadable.len(), 1, "{:?}", drained.unreadable);
    assert!(drained.unreadable[0].contains("0-junk.json"));
    assert!(junk.exists(), "left for an operator");
    assert_eq!(drained.recorded, 1);
}

/// A queued cockpit note without a live grant is refused by the record and
/// its file removed: it was decided, and nothing was appended.
#[test]
fn a_queued_cockpit_note_without_a_grant_is_refused_and_appends_nothing() {
    let f = Fixture::live();
    let (home, target) = (f.home.path(), f.target.path());
    let file = input::queue(
        home,
        target,
        &input::Request {
            submission: note(
                Origin::Cockpit {
                    session: "s-1".to_string(),
                },
                NOW + 60,
            ),
            operation: None,
        },
    )
    .unwrap();
    let before = f.open().records().len();
    let mut chain = f.open();
    let drained = input::apply_pending(&mut chain, home, target, NOW, "t1").unwrap();
    let refused = drained.applied[0].taken.refused.as_ref().unwrap();
    assert_eq!(refused["reason"], "control-not-granted");
    assert!(!file.exists());
    assert_eq!(f.open().records().len(), before);
}
