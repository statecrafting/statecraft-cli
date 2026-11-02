//! Typed run operations under one controller lease (spec 048).
//!
//! Three operations, `attempt.retry`, `attempt.continue` and `attempt.abort`,
//! submitted by the one controller that holds a run's lease, each carrying an
//! idempotency key and the run version it was decided against. Everything is
//! in the run record (section 3.4): a lease is an `Accounting` record, an
//! accepted operation is bracketed as every effect is (an `Intent` durable
//! before the effect, an `Outcome` after it), and a refusal for a stale
//! version is an `Accounting` record holding its key. [`fold`] reads them
//! back and nothing else, so replaying the chain reproduces every lease,
//! operation outcome and attempt (I-1), and a key is remembered for as long as
//! the run is: there is no table beside the record and nothing is evicted.
//!
//! Every record this module writes carries one of the subjects
//! [`is_operation_record`] names. A reader that takes "the run's last outcome"
//! or "the run's last accounting" skips them, because neither an operation's
//! outcome nor a lease is an attempt's outcome or its refusal count.
//!
//! The rules are checked in section 3.3's order, and the first that applies
//! decides: the lease (rule 0), a held key with a different operation or
//! version (rule 1), a held key with the same ones (rule 2), a stale version
//! (rule 3), and only then the operation's own precondition (section 3.1).

use crate::attempt::{Attempt, Outcome};
use crate::record::{Chain, EffectId, Entry, Identity, Kind, RecordError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The subject of an accepted operation's intent and outcome.
pub const OPERATION_SUBJECT: &str = "operation";
/// The subject of a lease record (section 3.2).
pub const LEASE_SUBJECT: &str = "operation-lease";
/// The subject of a recorded `stale-version` refusal (section 3.3 rule 3).
pub const REFUSED_SUBJECT: &str = "operation-refused";
/// The subject of an abort's `stop-confirmed` point (section 3.1).
pub const STOP_CONFIRMED_SUBJECT: &str = "operation-stop-confirmed";

/// Whether a record is one this module wrote.
#[must_use]
pub fn is_operation_record(entry: &Entry) -> bool {
    [
        OPERATION_SUBJECT,
        LEASE_SUBJECT,
        REFUSED_SUBJECT,
        STOP_CONFIRMED_SUBJECT,
    ]
    .contains(&entry.subject.as_str())
}

/// The three operations (section 3.1).
///
/// Written as the dotted name the spec gives it, the way the envelope's `verb`
/// is dotted, so serialization is by hand rather than a derived variant name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// A new attempt from the base revision resolved now.
    Retry,
    /// A new attempt resuming the previous attempt's provider session.
    Continue,
    /// A request that the live attempt stop.
    Abort,
}

impl Operation {
    /// The name the operation is recorded under.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Operation::Retry => "attempt.retry",
            Operation::Continue => "attempt.continue",
            Operation::Abort => "attempt.abort",
        }
    }

    /// Every operation.
    #[must_use]
    pub fn all() -> [Operation; 3] {
        [Operation::Retry, Operation::Continue, Operation::Abort]
    }

    /// The operation a name or its last word names: `attempt.retry` or `retry`.
    #[must_use]
    pub fn parse(word: &str) -> Option<Operation> {
        Operation::all()
            .into_iter()
            .find(|o| o.word() == word || o.word().strip_prefix("attempt.") == Some(word))
    }

    /// Whether it launches an attempt, and so takes the slot (section 3.5).
    #[must_use]
    pub fn launches(self) -> bool {
        !matches!(self, Operation::Abort)
    }
}

impl Serialize for Operation {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.word())
    }
}

impl<'de> Deserialize<'de> for Operation {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let word = String::deserialize(deserializer)?;
        Operation::all()
            .into_iter()
            .find(|o| o.word() == word)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown operation `{word}`")))
    }
}

/// The closed refusal reasons this spec adds (sections 3.1 to 3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    /// Another controller holds the lease.
    LeaseHeld,
    /// The operation does not present the run's current lease (rule 0).
    LeaseNotHeld,
    /// The key is held with a different operation or version (rule 1).
    KeyConflict,
    /// The version is not the run's current one (rule 3).
    StaleVersion,
    /// No adapter declares that it can resume a provider session.
    CapabilityUndeclared,
}

impl Reason {
    /// The word a reason is written as.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Reason::LeaseHeld => "lease-held",
            Reason::LeaseNotHeld => "lease-not-held",
            Reason::KeyConflict => "key-conflict",
            Reason::StaleVersion => "stale-version",
            Reason::CapabilityUndeclared => "capability-undeclared",
        }
    }
}

/// A refusal. Exit 2 wherever it is rendered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Refusal {
    /// The closed reason, or none for an unmet precondition of section 3.1's
    /// table, which the detail names instead (no other reason is added).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<Reason>,
    /// What was found, in one sentence.
    pub detail: String,
    /// Whether the refusal was appended to the record (rule 3 alone is).
    pub recorded: bool,
    /// The version the operation was decided against, for `stale-version`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<u64>,
    /// The run's version when it was refused, for `stale-version`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_version: Option<u64>,
}

impl Refusal {
    fn closed(reason: Reason, detail: String) -> Self {
        Self {
            reason: Some(reason),
            detail,
            recorded: false,
            expected_version: None,
            current_version: None,
        }
    }

    fn precondition(detail: String) -> Self {
        Self {
            reason: None,
            detail,
            recorded: false,
            expected_version: None,
            current_version: None,
        }
    }

    /// The reason's word, or `precondition` for a refusal with none.
    #[must_use]
    pub fn word(&self) -> &'static str {
        self.reason.map_or("precondition", Reason::word)
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.word(), self.detail)
    }
}

/// The run's lease (section 3.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lease {
    /// The position of the record that acquired it: what an operation
    /// presents, and different for every acquisition.
    pub id: u64,
    /// The controller holding it.
    pub controller: String,
    /// The controller's process, read only to decide that it has ended.
    pub pid: u32,
    /// The run version when it was acquired.
    pub observed_version: u64,
    /// When, RFC 3339 UTC.
    pub at: String,
}

/// Where an abort's stop stands (section 3.1). Never read as one another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stop {
    /// The abort was accepted.
    StopRequested,
    /// The attempt's `cancelled` outcome is durable.
    StopConfirmed,
}

/// One key the record holds for a run, and what it answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recorded {
    /// The idempotency key.
    pub key: String,
    /// The operation submitted under it.
    pub operation: Operation,
    /// The version it was decided against.
    pub version: u64,
    /// The controller that submitted it.
    pub controller: String,
    /// The position of its intent, or of its recorded refusal.
    pub position: u64,
    /// The attempt it acted on: the run's latest when it was decided.
    pub attempt: u32,
    /// Present when the key holds a refusal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<Refusal>,
    /// The outcome record's detail; absent while the effect is in flight or
    /// for an intent no outcome followed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Value>,
    /// For an abort, where its stop stands.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Stop>,
}

/// A run's operation state: a fold over the record alone (section 3.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunState {
    /// The run.
    pub run_id: String,
    /// The count of records about the run in the chain (section 3.3).
    pub version: u64,
    /// The current lease, if one is held.
    pub lease: Option<Lease>,
    /// Every key the record holds, in chain order.
    pub operations: Vec<Recorded>,
    /// The run's attempts, as spec 003's fold reads them.
    pub attempts: Vec<Attempt>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntentDetail {
    operation: Operation,
    version: u64,
    controller: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RefusedDetail {
    operation: Operation,
    version: u64,
    current_version: u64,
    controller: String,
    detail: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LeaseDetail {
    lease: String,
    #[serde(default)]
    lease_id: Option<u64>,
    #[serde(default)]
    controller: String,
    #[serde(default)]
    pid: u32,
    #[serde(default)]
    observed_version: u64,
    #[serde(default)]
    at: String,
}

/// Fold one run's operation state out of the record, and nothing else.
#[must_use]
pub fn fold(chain: &Chain, run_id: &str) -> RunState {
    let mut state = RunState {
        run_id: run_id.to_string(),
        version: 0,
        lease: None,
        operations: Vec::new(),
        attempts: crate::session::runs(chain)
            .into_iter()
            .find(|r| r.id == run_id)
            .map(|r| r.attempts)
            .unwrap_or_default(),
    };
    for (position, entry) in chain.positioned_entries() {
        if entry.run_id != run_id {
            continue;
        }
        state.version += 1;
        let position = position as u64;
        let key = entry.idempotency_key.clone().unwrap_or_default();
        match (entry.kind, entry.subject.as_str()) {
            (Kind::Accounting, LEASE_SUBJECT) => {
                let Ok(d) = serde_json::from_value::<LeaseDetail>(entry.detail.clone()) else {
                    continue;
                };
                if d.lease == "acquired" {
                    state.lease = Some(Lease {
                        id: position,
                        controller: d.controller,
                        pid: d.pid,
                        observed_version: d.observed_version,
                        at: d.at,
                    });
                } else if state.lease.as_ref().map(|l| l.id) == d.lease_id {
                    // Released, broken or expired: each names the lease it ends.
                    state.lease = None;
                }
            }
            (Kind::Intent, OPERATION_SUBJECT) => {
                if let Ok(d) = serde_json::from_value::<IntentDetail>(entry.detail.clone()) {
                    state.operations.push(Recorded {
                        key,
                        operation: d.operation,
                        version: d.version,
                        controller: d.controller,
                        position,
                        attempt: entry.attempt,
                        refusal: None,
                        outcome: None,
                        stop: None,
                    });
                }
            }
            (Kind::Accounting, REFUSED_SUBJECT) => {
                if let Ok(d) = serde_json::from_value::<RefusedDetail>(entry.detail.clone()) {
                    state.operations.push(Recorded {
                        key,
                        operation: d.operation,
                        version: d.version,
                        controller: d.controller,
                        position,
                        attempt: entry.attempt,
                        refusal: Some(Refusal {
                            reason: Some(Reason::StaleVersion),
                            detail: d.detail,
                            recorded: true,
                            expected_version: Some(d.version),
                            current_version: Some(d.current_version),
                        }),
                        outcome: None,
                        stop: None,
                    });
                }
            }
            (Kind::Outcome, OPERATION_SUBJECT) => {
                if let Some(op) = state
                    .operations
                    .iter_mut()
                    .find(|o| o.key == key && o.refusal.is_none() && o.outcome.is_none())
                {
                    if op.operation == Operation::Abort
                        && entry.detail.get("result").and_then(Value::as_str)
                            == Some("stop-requested")
                    {
                        op.stop = Some(Stop::StopRequested);
                    }
                    op.outcome = Some(entry.detail.clone());
                }
            }
            (Kind::Accounting, STOP_CONFIRMED_SUBJECT) => {
                if let Some(op) = state
                    .operations
                    .iter_mut()
                    .find(|o| o.key == key && o.stop == Some(Stop::StopRequested))
                {
                    op.stop = Some(Stop::StopConfirmed);
                }
            }
            _ => {}
        }
    }
    state
}

/// One operation as a controller submits it (section 3.3).
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    /// The run.
    pub run_id: &'a str,
    /// The controller submitting it.
    pub controller: &'a str,
    /// The lease it presents: [`Lease::id`].
    pub lease: u64,
    /// Its idempotency key, chosen by the controller.
    pub key: &'a str,
    /// The run version it was decided against.
    pub version: u64,
    /// What it asks for.
    pub operation: Operation,
    /// Why, recorded with the intent.
    pub reason: &'a str,
    /// When, RFC 3339 UTC.
    pub at: &'a str,
}

/// An operation whose intent is durable and whose effect is the caller's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Accepted {
    /// The run.
    pub run_id: String,
    /// Its key.
    pub key: String,
    /// The operation.
    pub operation: Operation,
    /// The version it was decided against.
    pub version: u64,
    /// The attempt it acts on: the live one for an abort, the latest
    /// concluded one for a launch.
    pub attempt: u32,
    /// The position of its intent.
    pub position: u64,
}

/// What [`submit`] decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Submitted {
    /// Every rule passed and the intent is durable. The caller performs the
    /// effect and then calls [`complete`].
    Accepted(Accepted),
    /// Rule 2: the key is held with the same operation and version. Nothing
    /// was appended; this is what the record holds under it.
    Replayed(Recorded),
    /// Refused. Only a `stale-version` refusal was appended.
    Refused(Refusal),
}

fn effect_of(key: &str) -> Identity {
    EffectId::new(&format!("operation/{key}")).map_or(Identity::Absent, Identity::Valid)
}

/// Decide an operation by section 3.3's rules, then its precondition, and
/// make its intent durable when it is accepted.
///
/// The caller holds the repository lock, as for every write to the record.
pub fn submit(chain: &mut Chain, request: &Request<'_>) -> Result<Submitted, RecordError> {
    // A request without a key is malformed, not decided: no rule can apply to
    // it, and rule 3's recorded refusal holds its key, which an empty key
    // cannot be. It is refused before any rule, and nothing is appended.
    if request.key.is_empty() {
        return Ok(Submitted::Refused(Refusal::precondition(
            "an operation needs a non-empty idempotency key".to_string(),
        )));
    }
    let state = fold(chain, request.run_id);
    let run_id = request.run_id;

    // Rule 0: before anything about keys is consulted.
    let holds = state
        .lease
        .as_ref()
        .is_some_and(|l| l.id == request.lease && l.controller == request.controller);
    if !holds {
        return Ok(Submitted::Refused(Refusal::closed(
            Reason::LeaseNotHeld,
            format!(
                "controller {} does not hold run {run_id}'s current lease {}; nothing was \
                 checked or appended",
                request.controller, request.lease
            ),
        )));
    }

    // Rules 1 and 2.
    if let Some(held) = state.operations.iter().find(|o| o.key == request.key) {
        if held.operation != request.operation || held.version != request.version {
            return Ok(Submitted::Refused(Refusal::closed(
                Reason::KeyConflict,
                format!(
                    "key {} is recorded for run {run_id} as {} at version {}; this request is {} \
                     at version {}",
                    request.key,
                    held.operation.word(),
                    held.version,
                    request.operation.word(),
                    request.version
                ),
            )));
        }
        return Ok(Submitted::Replayed(held.clone()));
    }

    let attempt = state.attempts.last().map_or(0, |a| a.number);

    // Rule 3: the refusal is recorded, holding its key; the effect is not.
    if request.version != state.version {
        let detail = format!(
            "run {run_id} is at version {}, and this {} was decided against version {}",
            state.version,
            request.operation.word(),
            request.version
        );
        chain.append(
            &format!("{run_id}/operation/{}/refused", request.key),
            request.at,
            &Entry {
                kind: Kind::Accounting,
                run_id: run_id.to_string(),
                attempt,
                subject: REFUSED_SUBJECT.to_string(),
                idempotency_key: Some(request.key.to_string()),
                detail: json!({
                    "reason": Reason::StaleVersion.word(),
                    "operation": request.operation.word(),
                    "version": request.version,
                    "currentVersion": state.version,
                    "controller": request.controller,
                    "detail": detail,
                    "at": request.at,
                }),
                effect_id: Identity::Absent,
            },
        )?;
        return Ok(Submitted::Refused(Refusal {
            reason: Some(Reason::StaleVersion),
            detail,
            recorded: true,
            expected_version: Some(request.version),
            current_version: Some(state.version),
        }));
    }

    if let Some(refusal) = precondition(chain, &state, request.operation) {
        return Ok(Submitted::Refused(refusal));
    }

    let position = chain.records().len() as u64;
    chain.append(
        &format!("{run_id}/operation/{}/intent", request.key),
        request.at,
        &Entry {
            kind: Kind::Intent,
            run_id: run_id.to_string(),
            attempt,
            subject: OPERATION_SUBJECT.to_string(),
            idempotency_key: Some(request.key.to_string()),
            detail: json!({
                "operation": request.operation.word(),
                "version": request.version,
                "controller": request.controller,
                "lease": request.lease,
                "reason": request.reason,
                "at": request.at,
            }),
            effect_id: effect_of(request.key),
        },
    )?;
    Ok(Submitted::Accepted(Accepted {
        run_id: run_id.to_string(),
        key: request.key.to_string(),
        operation: request.operation,
        version: request.version,
        attempt,
        position,
    }))
}

/// Section 3.1's preconditions, and 3.5's slot, after every rule of 3.3.
fn precondition(chain: &Chain, state: &RunState, operation: Operation) -> Option<Refusal> {
    let run_id = &state.run_id;
    let latest = state.attempts.last();
    match operation {
        Operation::Retry | Operation::Continue => {
            let Some(latest) = latest else {
                return Some(Refusal::precondition(format!(
                    "the record holds no attempt of run {run_id}"
                )));
            };
            if latest.live() {
                return Some(Refusal::precondition(format!(
                    "run {run_id} attempt {} has no outcome yet",
                    latest.number
                )));
            }
            if let Some(live) = crate::session::live_attempt(chain) {
                return Some(Refusal::precondition(format!(
                    "run {} attempt {} is live in this repository; one live attempt per \
                     repository (spec 003 section 3.7)",
                    live.run_id, live.number
                )));
            }
            if operation == Operation::Continue {
                // Section 3.1: a capability, never an adapter name. Spec 047's
                // declaration names no resume capability, so none declares it.
                return Some(Refusal::closed(
                    Reason::CapabilityUndeclared,
                    "no adapter declares that it can resume a provider session, so \
                     attempt.continue is refused for every adapter"
                        .to_string(),
                ));
            }
            None
        }
        Operation::Abort => match latest {
            Some(a) if a.live() => None,
            _ => Some(Refusal::precondition(format!(
                "run {run_id} has no live attempt to abort"
            ))),
        },
    }
}

/// What an accepted operation's effect came to.
#[derive(Debug, Clone, Copy)]
pub enum Completion<'a> {
    /// An abort's stop was requested: its `stop-requested` point.
    StopRequested,
    /// A launch ran `run`'s path: the attempt it appended, if it appended
    /// one, and the exit that path answered.
    Launched {
        /// The attempt appended, if any.
        attempt: Option<&'a Attempt>,
        /// The launch's exit code.
        exit: i32,
    },
}

/// Append an accepted operation's outcome, after its effect.
pub fn complete(
    chain: &mut Chain,
    accepted: &Accepted,
    completion: Completion<'_>,
    at: &str,
) -> Result<Value, RecordError> {
    let detail = match completion {
        Completion::StopRequested => json!({
            "operation": accepted.operation.word(),
            "result": "stop-requested",
            "attempt": accepted.attempt,
            "at": at,
        }),
        Completion::Launched { attempt, exit } => {
            let result = if attempt.is_some() {
                "launched"
            } else {
                "not-launched"
            };
            let number = attempt.map(|a| a.number);
            let outcome = attempt.and_then(|a| a.outcome).map(Outcome::word);
            json!({
                "operation": accepted.operation.word(),
                "result": result,
                "attempt": number,
                "attemptOutcome": outcome,
                "exit": exit,
                "at": at,
            })
        }
    };
    chain.append(
        &format!("{}/operation/{}/outcome", accepted.run_id, accepted.key),
        at,
        &Entry {
            kind: Kind::Outcome,
            run_id: accepted.run_id.clone(),
            attempt: accepted.attempt,
            subject: OPERATION_SUBJECT.to_string(),
            idempotency_key: Some(accepted.key.clone()),
            detail: detail.clone(),
            effect_id: effect_of(&accepted.key),
        },
    )?;
    Ok(detail)
}

/// Record `stop-confirmed` for every abort whose stop is requested of this
/// attempt, once the attempt's `cancelled` outcome is durable, and for no
/// other outcome. Returns how many were confirmed.
pub fn confirm_stop(
    chain: &mut Chain,
    run_id: &str,
    attempt: u32,
    at: &str,
) -> Result<usize, RecordError> {
    let state = fold(chain, run_id);
    let cancelled = state
        .attempts
        .iter()
        .any(|a| a.number == attempt && a.outcome == Some(Outcome::Cancelled));
    if !cancelled {
        return Ok(0);
    }
    let keys: Vec<String> = state
        .operations
        .iter()
        .filter(|o| o.attempt == attempt && o.stop == Some(Stop::StopRequested))
        .map(|o| o.key.clone())
        .collect();
    for key in &keys {
        chain.append(
            &format!("{run_id}/operation/{key}/stop-confirmed"),
            at,
            &Entry {
                kind: Kind::Accounting,
                run_id: run_id.to_string(),
                attempt,
                subject: STOP_CONFIRMED_SUBJECT.to_string(),
                idempotency_key: Some(key.clone()),
                detail: json!({ "attempt": attempt, "at": at }),
                effect_id: Identity::Absent,
            },
        )?;
    }
    Ok(keys.len())
}

/// A controller as it names itself (section 3.2).
#[derive(Debug, Clone, Copy)]
pub struct Controller<'a> {
    /// Its name.
    pub name: &'a str,
    /// Its process.
    pub pid: u32,
}

/// What a lease act changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaseChange {
    /// `acquired`, `already-held`, `released` or `broken`.
    pub change: &'static str,
    /// The lease acquired, kept or ended.
    pub lease: Lease,
    /// A lease found ended because its holder's process had ended and the run
    /// had no live attempt, recorded before this acquisition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expired: Option<Lease>,
    /// The run version after the act: what the next operation is decided
    /// against.
    pub version: u64,
}

fn append_lease(
    chain: &mut Chain,
    run_id: &str,
    attempt: u32,
    at: &str,
    detail: Value,
) -> Result<(), RecordError> {
    let position = chain.records().len();
    chain.append(
        &format!("{run_id}/lease/{position}"),
        at,
        &Entry {
            kind: Kind::Accounting,
            run_id: run_id.to_string(),
            attempt,
            subject: LEASE_SUBJECT.to_string(),
            idempotency_key: None,
            detail,
            effect_id: Identity::Absent,
        },
    )
}

/// Acquire a run's lease (section 3.2).
///
/// Refused `lease-held` while another controller holds it. A lease whose
/// holder's process has ended, on a run with no live attempt, has ended: that
/// is recorded first, so the fold reproduces it without observing anything.
pub fn acquire(
    chain: &mut Chain,
    run_id: &str,
    controller: &Controller<'_>,
    at: &str,
) -> Result<Result<LeaseChange, Refusal>, RecordError> {
    let state = fold(chain, run_id);
    let Some(latest) = state.attempts.last().map(|a| a.number) else {
        return Ok(Err(Refusal::precondition(format!(
            "the record holds no attempt of run {run_id}, so it has no lease to take"
        ))));
    };
    let mut expired = None;
    if let Some(current) = &state.lease {
        if current.controller == controller.name && current.pid == controller.pid {
            return Ok(Ok(LeaseChange {
                change: "already-held",
                lease: current.clone(),
                expired: None,
                version: state.version,
            }));
        }
        let live = state.attempts.iter().any(Attempt::live);
        if live || crate::reconcile::process_exists(current.pid) {
            return Ok(Err(Refusal::closed(
                Reason::LeaseHeld,
                format!(
                    "run {run_id}'s lease {} is held by controller {} (process {})",
                    current.id, current.controller, current.pid
                ),
            )));
        }
        append_lease(
            chain,
            run_id,
            latest,
            at,
            json!({
                "lease": "expired",
                "leaseId": current.id,
                "controller": current.controller,
                "pid": current.pid,
                "at": at,
            }),
        )?;
        expired = Some(current.clone());
    }
    let observed = fold(chain, run_id).version;
    let id = chain.records().len() as u64;
    append_lease(
        chain,
        run_id,
        latest,
        at,
        json!({
            "lease": "acquired",
            "controller": controller.name,
            "pid": controller.pid,
            "observedVersion": observed,
            "at": at,
        }),
    )?;
    Ok(Ok(LeaseChange {
        change: "acquired",
        lease: Lease {
            id,
            controller: controller.name.to_string(),
            pid: controller.pid,
            observed_version: observed,
            at: at.to_string(),
        },
        expired,
        version: fold(chain, run_id).version,
    }))
}

/// Release the lease its holder presents.
pub fn release(
    chain: &mut Chain,
    run_id: &str,
    controller: &str,
    lease: u64,
    at: &str,
) -> Result<Result<LeaseChange, Refusal>, RecordError> {
    let state = fold(chain, run_id);
    let Some(current) = state
        .lease
        .filter(|l| l.id == lease && l.controller == controller)
    else {
        return Ok(Err(Refusal::closed(
            Reason::LeaseNotHeld,
            format!("controller {controller} does not hold run {run_id}'s current lease {lease}"),
        )));
    };
    let attempt = state.attempts.last().map_or(0, |a| a.number);
    append_lease(
        chain,
        run_id,
        attempt,
        at,
        json!({ "lease": "released", "leaseId": current.id, "controller": controller, "at": at }),
    )?;
    Ok(Ok(LeaseChange {
        change: "released",
        lease: current,
        expired: None,
        version: fold(chain, run_id).version,
    }))
}

/// An operator breaks the current lease, recorded with the operator's name.
pub fn break_lease(
    chain: &mut Chain,
    run_id: &str,
    operator: &str,
    reason: &str,
    at: &str,
) -> Result<Result<LeaseChange, Refusal>, RecordError> {
    let state = fold(chain, run_id);
    let Some(current) = state.lease else {
        return Ok(Err(Refusal::precondition(format!(
            "run {run_id} has no lease to break"
        ))));
    };
    let attempt = state.attempts.last().map_or(0, |a| a.number);
    append_lease(
        chain,
        run_id,
        attempt,
        at,
        json!({
            "lease": "broken",
            "leaseId": current.id,
            "controller": current.controller,
            "operator": operator,
            "reason": reason,
            "at": at,
        }),
    )?;
    Ok(Ok(LeaseChange {
        change: "broken",
        lease: current,
        expired: None,
        version: fold(chain, run_id).version,
    }))
}

/// One record of a run, at its position in the chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// The record's position: the cursor to resume after it.
    pub cursor: u64,
    /// The record's payload.
    pub entry: Entry,
}

/// The run's records after a cursor (section 3.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Events {
    /// The run.
    pub run_id: String,
    /// The cursor read after, or none for the start of the chain.
    pub after: Option<u64>,
    /// The run's records after it, in chain order.
    pub events: Vec<Event>,
    /// The cursor to resume from: the last record returned, or `after` when
    /// there was none.
    pub next: Option<u64>,
}

/// Read a run's records after a cursor. Needs no lease and writes nothing.
#[must_use]
pub fn events(chain: &Chain, run_id: &str, after: Option<u64>) -> Events {
    let events: Vec<Event> = chain
        .positioned_entries()
        .into_iter()
        .filter(|(position, entry)| {
            entry.run_id == run_id && after.is_none_or(|a| *position as u64 > a)
        })
        .map(|(position, entry)| Event {
            cursor: position as u64,
            entry,
        })
        .collect();
    let next = events.last().map(|e| e.cursor).or(after);
    Events {
        run_id: run_id.to_string(),
        after,
        events,
        next,
    }
}
