//! Operator input to a live attempt (spec 053).
//!
//! An operator steers a live attempt with a **note**, read by the agent at its
//! next governed tool call, or a **command**, which starts a new turn as a
//! spec 048 continue. Every input is appended to the run record before
//! anything else happens to it (I-1), naming its kind, body, author, reason,
//! the attempt it is for and an expiry, and its later states are appended
//! after it. Nothing about an input lives only in memory: [`fold`] reads them
//! back from the chain and nothing else.
//!
//! # The states (section 3.2)
//!
//! `pending` to `delivered` to `acknowledged`, or `pending` or `delivered` to
//! `expired`, and no other move (R-1). `delivered` is the supervisor's own
//! act, recorded once it has placed the input where the child reads it, and
//! is a fact. `acknowledged` is read from the gate log, which the gate writes
//! from inside the confinement, so it is recorded with the attestation
//! [`CHILD_ATTESTED`] and is never proof the agent acted on the input.
//!
//! # Evidence, not an effect bracket
//!
//! Every record this module writes is [`Kind::Evidence`] (section 5,
//! 2026-11-01). An input has up to three later states where an effect has one
//! outcome, and an input is not an effect this product performs on the
//! target, so neither spec 003's effect fold nor its reconciliation reads
//! them, and a pending input never blocks a retry.
//!
//! # Control grants (section 3.4)
//!
//! A cockpit session submits nothing until an operator grants it control for
//! a stated duration of at most [`MAX_GRANT_SECONDS`]; the grant names the
//! operator and ends at its expiry or when revoked. The CLI needs no grant.
//! There is no remote origin to refuse: [`Origin`] names the two accepted
//! ones and has no third (R-3).
//!
//! # What this module does not decide
//!
//! How a submission made by another process reaches the supervisor of a live
//! attempt. The supervisor holds the repository lock for the whole attempt
//! (spec 003 section 3.1.4 rule 7), so no other process can append while it
//! runs; [`Desk`] takes submissions from an [`Inbox`], and which inbox exists
//! is an owner decision (section 5, 2026-11-01).

use crate::record::{Chain, Entry, Identity, Kind, RecordError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The subject of an input's first record, the one that makes it `pending`.
pub const INPUT_SUBJECT: &str = "operator-input";
/// The subject of every later state of an input.
pub const STATE_SUBJECT: &str = "operator-input-state";
/// The subject of a cockpit control grant.
pub const GRANT_SUBJECT: &str = "control-grant";
/// The subject of a control grant's revocation.
pub const REVOKE_SUBJECT: &str = "control-revoke";

/// The longest control grant an operator may give, in seconds (section 3.4).
pub const MAX_GRANT_SECONDS: i64 = 3600;

/// The attestation an acknowledgement carries: the gate log it was read from
/// is written inside the confinement (spec 004 section 3.18 rule 5).
pub const CHILD_ATTESTED: &str = "child-attested";

/// Why the supervisor expired an input that was not acknowledged.
pub const EXPIRY_PASSED: &str = "expiry-passed";
/// Why the supervisor expired an input when its attempt ended.
pub const ATTEMPT_ENDED: &str = "attempt-ended";

/// Whether a record is a repository-wide one this module wrote: a grant or a
/// revocation, which belongs to no run and which the run fold skips.
#[must_use]
pub fn is_repository_record(entry: &Entry) -> bool {
    entry.kind == Kind::Evidence
        && (entry.subject == GRANT_SUBJECT || entry.subject == REVOKE_SUBJECT)
}

/// A note or a command (section 3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputKind {
    /// Read at the agent's next governed tool call.
    Note,
    /// Starts a turn, as a spec 048 continue; `abort` ends the attempt.
    Command,
}

/// Where an input in the record stands (section 3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// The record is durable; nothing has been placed.
    Pending,
    /// The supervisor has placed it where the child reads it.
    Delivered,
    /// The gate log says the gate returned it. Child-attested.
    Acknowledged,
    /// Its expiry passed, or its attempt ended, before acknowledgement.
    Expired,
}

impl State {
    /// The word a state is recorded and shown as.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            State::Pending => "pending",
            State::Delivered => "delivered",
            State::Acknowledged => "acknowledged",
            State::Expired => "expired",
        }
    }

    /// Whether an input in `from` may move to `self` (R-1).
    #[must_use]
    pub fn may_follow(self, from: State) -> bool {
        matches!(
            (from, self),
            (State::Pending, State::Delivered)
                | (State::Delivered, State::Acknowledged)
                | (State::Pending | State::Delivered, State::Expired)
        )
    }
}

/// Where an input was submitted from (section 3.5). There is no third.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Origin {
    /// The local CLI, whose operator is already the local user.
    Cli,
    /// A loopback cockpit session (spec 039), which needs a live grant.
    Cockpit {
        /// The cockpit session's identity.
        session: String,
    },
}

/// What an operator submits, recorded whole in the `pending` record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Submission {
    /// Note or command.
    pub kind: InputKind,
    /// The steering text, or the command.
    pub body: String,
    /// Who sent it.
    pub author: String,
    /// Why.
    pub reason: String,
    /// The run it is for.
    pub run_id: String,
    /// The attempt within that run it is for.
    pub attempt: u32,
    /// When it expires, in seconds since the Unix epoch.
    pub expires_at: i64,
    /// Where it was submitted from.
    pub origin: Origin,
}

/// The detail of an input's `pending` record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Recorded {
    id: String,
    submission: Submission,
}

/// The detail of a later state's record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Moved {
    id: String,
    state: State,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    attestation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    why: Option<String>,
}

/// One state an input reached, as the record holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transition {
    /// The state.
    pub state: State,
    /// The record's timestamp.
    pub at: String,
    /// The record's position in the chain.
    pub position: usize,
    /// [`CHILD_ATTESTED`] on an acknowledgement, and nothing otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestation: Option<String>,
    /// Why it expired, where it did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// One input, folded from the record: what was submitted and every state it
/// reached, oldest first. The evidence section 3.3 puts on its attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    /// Its identity, `input-NNNNNN`.
    pub id: String,
    /// What was submitted.
    pub submission: Submission,
    /// Where it stands now: the last state in `history`.
    pub state: State,
    /// Every state, starting with `pending`.
    pub history: Vec<Transition>,
}

/// Every input in a chain, and every record that could not be read as one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fold {
    /// The inputs, in the order they were recorded pending.
    pub inputs: Vec<Input>,
    /// A record that does not decode, names no recorded input, or moves an
    /// input in a way R-1 forbids. Reported, never applied.
    pub defects: Vec<String>,
}

impl Fold {
    /// Every input recorded for one attempt.
    pub fn for_attempt<'a>(
        &'a self,
        run_id: &'a str,
        attempt: u32,
    ) -> impl Iterator<Item = &'a Input> + 'a {
        self.inputs
            .iter()
            .filter(move |i| i.submission.run_id == run_id && i.submission.attempt == attempt)
    }

    fn get(&self, id: &str) -> Option<&Input> {
        self.inputs.iter().find(|i| i.id == id)
    }
}

/// Fold every input out of the chain.
#[must_use]
pub fn fold(chain: &Chain) -> Fold {
    let mut out = Fold::default();
    let records = chain.records();
    for (position, entry) in chain.positioned_entries() {
        if entry.kind != Kind::Evidence {
            continue;
        }
        let at = records
            .get(position)
            .map(|r| r.timestamp.clone())
            .unwrap_or_default();
        if entry.subject == INPUT_SUBJECT {
            match serde_json::from_value::<Recorded>(entry.detail.clone()) {
                Ok(r) if out.get(&r.id).is_none() => out.inputs.push(Input {
                    id: r.id,
                    submission: r.submission,
                    state: State::Pending,
                    history: vec![Transition {
                        state: State::Pending,
                        at,
                        position,
                        attestation: None,
                        why: None,
                    }],
                }),
                Ok(r) => out
                    .defects
                    .push(format!("record {position} records input {} again", r.id)),
                Err(e) => out
                    .defects
                    .push(format!("record {position} is not an input: {e}")),
            }
        } else if entry.subject == STATE_SUBJECT {
            let moved = match serde_json::from_value::<Moved>(entry.detail.clone()) {
                Ok(m) => m,
                Err(e) => {
                    out.defects
                        .push(format!("record {position} is not an input state: {e}"));
                    continue;
                }
            };
            let Some(input) = out.inputs.iter_mut().find(|i| i.id == moved.id) else {
                out.defects.push(format!(
                    "record {position} moves input {}, which is not recorded",
                    moved.id
                ));
                continue;
            };
            if !moved.state.may_follow(input.state) {
                out.defects.push(format!(
                    "record {position} moves input {} from {} to {}",
                    moved.id,
                    input.state.word(),
                    moved.state.word()
                ));
                continue;
            }
            input.state = moved.state;
            input.history.push(Transition {
                state: moved.state,
                at,
                position,
                attestation: moved.attestation,
                why: moved.why,
            });
        }
    }
    out
}

/// Why an input or a grant was not recorded.
#[derive(Debug, thiserror::Error)]
pub enum InputError {
    /// A cockpit session submitted with no live grant (R-2).
    #[error("the cockpit session `{session}` holds no live control grant")]
    ControlNotGranted {
        /// The session.
        session: String,
    },
    /// The attempt named is not one an input of this kind can be for.
    #[error("run {run_id} has no {wanted} attempt {attempt}")]
    NoAttempt {
        /// The run.
        run_id: String,
        /// The attempt.
        attempt: u32,
        /// `live` for a note, `recorded` for a command.
        wanted: &'static str,
    },
    /// A field is empty, an expiry has passed, or a grant is too long.
    #[error("{0}")]
    Invalid(String),
    /// A move R-1 forbids, or of an input that is not recorded.
    #[error("input {id} cannot become {to}: {why}")]
    Illegal {
        /// The input.
        id: String,
        /// The state asked for.
        to: &'static str,
        /// What the record says instead.
        why: String,
    },
    /// The chain refused the append; nothing was recorded.
    #[error(transparent)]
    Record(#[from] RecordError),
}

impl InputError {
    /// The closed refusal word, for an answer's `reason`.
    #[must_use]
    pub fn reason(&self) -> &'static str {
        match self {
            InputError::ControlNotGranted { .. } => "control-not-granted",
            InputError::NoAttempt { .. } => "no-such-attempt",
            InputError::Invalid(_) => "invalid-input",
            InputError::Illegal { .. } => "illegal-transition",
            InputError::Record(_) => "record-unwritable",
        }
    }
}

fn append(
    chain: &mut Chain,
    run_id: &str,
    attempt: u32,
    subject: &str,
    detail: Value,
    timestamp: &str,
) -> Result<(), RecordError> {
    let id = format!("{run_id}/{attempt}/{subject}/{}", chain.records().len());
    chain.append(
        &id,
        timestamp,
        &Entry {
            kind: Kind::Evidence,
            run_id: run_id.to_string(),
            attempt,
            subject: subject.to_string(),
            idempotency_key: None,
            detail,
            effect_id: Identity::Absent,
        },
    )
}

fn required(field: &str, value: &str) -> Result<(), InputError> {
    if value.trim().is_empty() {
        Err(InputError::Invalid(format!("an input needs a {field}")))
    } else {
        Ok(())
    }
}

/// Record a submission `pending`, durable before this returns, and answer its
/// identity (I-1). The grant is checked first, so a cockpit session without
/// one learns that before anything else (R-2).
///
/// # Errors
///
/// [`InputError`]; nothing is recorded.
pub fn record_pending(
    chain: &mut Chain,
    submission: &Submission,
    now: i64,
    timestamp: &str,
) -> Result<String, InputError> {
    if let Origin::Cockpit { session } = &submission.origin
        && live_grant(chain, session, now).is_none()
    {
        return Err(InputError::ControlNotGranted {
            session: session.clone(),
        });
    }
    required("body", &submission.body)?;
    required("author", &submission.author)?;
    required("reason", &submission.reason)?;
    if submission.expires_at <= now {
        return Err(InputError::Invalid(format!(
            "an input's expiry must be after now ({now}); it was {}",
            submission.expires_at
        )));
    }
    let runs = crate::session::runs(chain);
    let attempt = runs
        .iter()
        .find(|r| r.id == submission.run_id)
        .and_then(|r| r.attempts.iter().find(|a| a.number == submission.attempt));
    let (found, wanted) = match submission.kind {
        InputKind::Note => (attempt.is_some_and(|a| a.live()), "live"),
        InputKind::Command => (attempt.is_some(), "recorded"),
    };
    if !found {
        return Err(InputError::NoAttempt {
            run_id: submission.run_id.clone(),
            attempt: submission.attempt,
            wanted,
        });
    }
    let id = format!("input-{:06}", fold(chain).inputs.len() + 1);
    let detail = serde_json::to_value(Recorded {
        id: id.clone(),
        submission: submission.clone(),
    })
    .map_err(|e| InputError::Invalid(e.to_string()))?;
    append(
        chain,
        &submission.run_id,
        submission.attempt,
        INPUT_SUBJECT,
        detail,
        timestamp,
    )?;
    Ok(id)
}

fn moved(
    chain: &mut Chain,
    id: &str,
    to: State,
    attestation: Option<&str>,
    why: Option<&str>,
    timestamp: &str,
) -> Result<(), InputError> {
    let fold = fold(chain);
    let Some(input) = fold.get(id) else {
        return Err(InputError::Illegal {
            id: id.to_string(),
            to: to.word(),
            why: "it is not recorded".to_string(),
        });
    };
    if !to.may_follow(input.state) {
        return Err(InputError::Illegal {
            id: id.to_string(),
            to: to.word(),
            why: format!("it is {}", input.state.word()),
        });
    }
    let detail = serde_json::to_value(Moved {
        id: id.to_string(),
        state: to,
        attestation: attestation.map(str::to_string),
        why: why.map(str::to_string),
    })
    .map_err(|e| InputError::Invalid(e.to_string()))?;
    let (run_id, attempt) = (input.submission.run_id.clone(), input.submission.attempt);
    append(chain, &run_id, attempt, STATE_SUBJECT, detail, timestamp)?;
    Ok(())
}

/// Record that the supervisor has placed a pending input where the child
/// reads it.
///
/// # Errors
///
/// [`InputError::Illegal`] unless the input is `pending`.
pub fn deliver(chain: &mut Chain, id: &str, timestamp: &str) -> Result<(), InputError> {
    moved(chain, id, State::Delivered, None, None, timestamp)
}

/// Record that the gate log says the gate returned a delivered input,
/// labelled [`CHILD_ATTESTED`].
///
/// # Errors
///
/// [`InputError::Illegal`] unless the input is `delivered`.
pub fn acknowledge(chain: &mut Chain, id: &str, timestamp: &str) -> Result<(), InputError> {
    moved(
        chain,
        id,
        State::Acknowledged,
        Some(CHILD_ATTESTED),
        None,
        timestamp,
    )
}

/// Record that an input expired before acknowledgement, and why.
///
/// # Errors
///
/// [`InputError::Illegal`] unless the input is `pending` or `delivered`.
pub fn expire(chain: &mut Chain, id: &str, why: &str, timestamp: &str) -> Result<(), InputError> {
    moved(chain, id, State::Expired, None, Some(why), timestamp)
}

/// A control grant to one cockpit session (section 3.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Grant {
    /// The cockpit session it grants.
    pub session: String,
    /// The operator who granted it, at the CLI.
    pub operator: String,
    /// Why.
    pub reason: String,
    /// When, in seconds since the Unix epoch.
    pub granted_at: i64,
    /// When it ends, at most [`MAX_GRANT_SECONDS`] after `granted_at`.
    pub until: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Revoked {
    session: String,
    operator: String,
    reason: String,
    at: i64,
}

/// Record a control grant.
///
/// # Errors
///
/// [`InputError::Invalid`] for an empty field or a duration that is not
/// positive or exceeds [`MAX_GRANT_SECONDS`]; nothing is recorded.
pub fn grant(chain: &mut Chain, grant: &Grant, timestamp: &str) -> Result<(), InputError> {
    required("session", &grant.session)?;
    required("operator", &grant.operator)?;
    required("reason", &grant.reason)?;
    let seconds = grant.until - grant.granted_at;
    if seconds <= 0 || seconds > MAX_GRANT_SECONDS {
        return Err(InputError::Invalid(format!(
            "a control grant lasts more than 0 and at most {MAX_GRANT_SECONDS} seconds; this \
             one lasts {seconds}"
        )));
    }
    let detail = serde_json::to_value(grant).map_err(|e| InputError::Invalid(e.to_string()))?;
    append(chain, "", 0, GRANT_SUBJECT, detail, timestamp)?;
    Ok(())
}

/// Record that a session's control ends now.
///
/// # Errors
///
/// [`InputError::Invalid`] for an empty field; nothing is recorded.
pub fn revoke(
    chain: &mut Chain,
    session: &str,
    operator: &str,
    reason: &str,
    at: i64,
    timestamp: &str,
) -> Result<(), InputError> {
    required("session", session)?;
    required("operator", operator)?;
    required("reason", reason)?;
    let detail = serde_json::to_value(Revoked {
        session: session.to_string(),
        operator: operator.to_string(),
        reason: reason.to_string(),
        at,
    })
    .map_err(|e| InputError::Invalid(e.to_string()))?;
    append(chain, "", 0, REVOKE_SUBJECT, detail, timestamp)?;
    Ok(())
}

/// The grant under which `session` holds control at `now`, if any: the
/// latest grant to it that has not ended and was not revoked after it was
/// given. A record that does not decode grants nothing.
#[must_use]
pub fn live_grant(chain: &Chain, session: &str, now: i64) -> Option<Grant> {
    let mut live: Option<Grant> = None;
    for entry in chain.entries() {
        if !is_repository_record(&entry) {
            continue;
        }
        if entry.subject == GRANT_SUBJECT {
            if let Ok(g) = serde_json::from_value::<Grant>(entry.detail)
                && g.session == session
            {
                live = Some(g);
            }
        } else if let Ok(r) = serde_json::from_value::<Revoked>(entry.detail)
            && r.session == session
        {
            live = None;
        }
    }
    live.filter(|g| g.granted_at <= now && now < g.until)
}

/// Where the supervisor puts a note so the child's gate can return it, and
/// where it reads which notes the gate returned. Implemented over the
/// attempt's exchange directory by `statecraft-home`.
pub trait Placement {
    /// Make a pending note readable to the gate.
    ///
    /// # Errors
    ///
    /// Why it could not; the note stays `pending`.
    fn place(&mut self, input: &Input) -> Result<(), String>;
    /// Take a note away, so the gate cannot return it again.
    ///
    /// # Errors
    ///
    /// Why it could not.
    fn withdraw(&mut self, id: &str) -> Result<(), String>;
    /// The identities the gate log says the gate returned. Child-attested.
    fn returned(&mut self) -> Vec<String>;
}

/// Where the supervisor takes submissions made while its attempt is live.
pub trait Inbox {
    /// Every submission waiting, oldest first; each is returned once.
    fn take(&mut self) -> Vec<Submission>;
}

/// The inbox a supervisor has until the owner decides how another process
/// reaches it (section 5, 2026-11-01): it never holds anything.
pub struct NoInbox;

impl Inbox for NoInbox {
    fn take(&mut self) -> Vec<Submission> {
        Vec::new()
    }
}

/// What one pass of the desk did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tick {
    /// How many records it appended.
    pub recorded: usize,
    /// Every submission it refused and every placement it could not make.
    pub failures: Vec<String>,
}

/// The supervisor's desk for one live attempt (sections 3.2 and 3.3).
///
/// Each pass records what its inbox holds `pending`, places each pending note
/// and records it `delivered`, records `acknowledged` each delivered note the
/// gate log names, and expires each note whose expiry has passed. Commands
/// are recorded and left `pending`: acting on one is spec 048's continue and
/// abort (section 5, 2026-11-01).
pub struct Desk {
    run_id: String,
    attempt: u32,
    open: bool,
}

impl Desk {
    /// A desk for one attempt.
    #[must_use]
    pub fn new(run_id: &str, attempt: u32) -> Self {
        Self {
            run_id: run_id.to_string(),
            attempt,
            open: true,
        }
    }

    /// One pass while the attempt is live.
    pub fn tick(
        &mut self,
        chain: &mut Chain,
        placement: &mut dyn Placement,
        inbox: &mut dyn Inbox,
        now: i64,
        timestamp: &str,
    ) -> Tick {
        let mut tick = Tick::default();
        for submission in inbox.take() {
            if submission.run_id != self.run_id || submission.attempt != self.attempt {
                tick.failures.push(format!(
                    "a submission for attempt {} of run {} reached the supervisor of attempt {} \
                     of run {}",
                    submission.attempt, submission.run_id, self.attempt, self.run_id
                ));
                continue;
            }
            match record_pending(chain, &submission, now, timestamp) {
                Ok(_) => {
                    tick.recorded += 1;
                    self.open = true;
                }
                Err(e) => tick.failures.push(format!("{}: {e}", e.reason())),
            }
        }
        if self.open {
            self.settle(chain, placement, now, timestamp, false, &mut tick);
        }
        tick
    }

    /// The last pass, once the attempt's process has ended: what the gate
    /// returned is acknowledged, and every other note is expired.
    pub fn close(
        &mut self,
        chain: &mut Chain,
        placement: &mut dyn Placement,
        now: i64,
        timestamp: &str,
    ) -> Tick {
        let mut tick = Tick::default();
        self.settle(chain, placement, now, timestamp, true, &mut tick);
        tick
    }

    fn settle(
        &mut self,
        chain: &mut Chain,
        placement: &mut dyn Placement,
        now: i64,
        timestamp: &str,
        ended: bool,
        tick: &mut Tick,
    ) {
        let notes: Vec<Input> = fold(chain)
            .for_attempt(&self.run_id, self.attempt)
            .filter(|i| i.submission.kind == InputKind::Note)
            .filter(|i| matches!(i.state, State::Pending | State::Delivered))
            .cloned()
            .collect();
        if notes.is_empty() {
            self.open = false;
            return;
        }
        let returned = placement.returned();
        let mut open = false;
        for note in notes {
            let due = ended || now >= note.submission.expires_at;
            let why = if ended { ATTEMPT_ENDED } else { EXPIRY_PASSED };
            let result = match note.state {
                State::Pending if due => expire(chain, &note.id, why, timestamp),
                State::Pending => match placement.place(&note) {
                    Ok(()) => {
                        open = true;
                        deliver(chain, &note.id, timestamp)
                    }
                    Err(e) => {
                        open = true;
                        tick.failures
                            .push(format!("note {} could not be placed: {e}", note.id));
                        continue;
                    }
                },
                State::Delivered if returned.contains(&note.id) => {
                    if let Err(e) = placement.withdraw(&note.id) {
                        tick.failures.push(e);
                    }
                    acknowledge(chain, &note.id, timestamp)
                }
                State::Delivered if due => {
                    if let Err(e) = placement.withdraw(&note.id) {
                        tick.failures.push(e);
                    }
                    expire(chain, &note.id, why, timestamp)
                }
                _ => {
                    open = true;
                    continue;
                }
            };
            match result {
                Ok(()) => tick.recorded += 1,
                Err(e) => tick.failures.push(e.to_string()),
            }
        }
        self.open = open;
    }
}
