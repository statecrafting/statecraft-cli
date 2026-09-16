//! Recovery: fold the record, reconcile before retrying.
//!
//! Spec 003 section 3.6. On start, the product folds the record and finds every
//! **intent with no outcome**. Each is reconciled before anything is retried,
//! and an `unknown` verdict **blocks** that retry and is reported. It is never
//! resolved by assuming either answer.
//!
//! This product makes **no exactly-once promise** for an external effect. Where
//! an effect is idempotent by a key, the key is recorded in the intent and named
//! in the record. Where it is not, the record says so, and a repeat is possible
//! and visible rather than impossible and claimed.

use crate::record::{Chain, Entry, Kind};
use serde::{Deserialize, Serialize};

/// What observing the world concluded about an intent's effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// The effect happened.
    Confirmed,
    /// The effect did not happen.
    Absent,
    /// It could not be determined. **Blocks** the retry.
    Unknown,
}

impl Verdict {
    /// Whether the effect this verdict concerns may be retried.
    pub fn retry_allowed(self) -> bool {
        // `confirmed` needs no retry and `absent` permits one. `unknown` is the
        // one that blocks, and it is the only reason this type is not a bool.
        !matches!(self, Verdict::Unknown)
    }
}

/// An intent the record has no outcome for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unmatched {
    /// The run.
    pub run_id: String,
    /// The attempt.
    pub attempt: u32,
    /// What the intent was about.
    pub subject: String,
    /// The intent's idempotency key, where the effect had one.
    ///
    /// `None` means this effect is **not** idempotent by a key, so a repeat is
    /// possible. That is recorded rather than glossed.
    pub idempotency_key: Option<String>,
}

impl Unmatched {
    /// Whether a repeat of this effect is safe by construction.
    pub fn idempotent(&self) -> bool {
        self.idempotency_key.is_some()
    }
}

/// A reconciled intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconciled {
    /// The intent.
    pub intent: Unmatched,
    /// What observation concluded.
    pub verdict: Verdict,
}

impl Reconciled {
    /// Whether the effect may now be retried.
    pub fn retry_allowed(&self) -> bool {
        self.verdict.retry_allowed()
    }
}

/// How the world is observed for an intent's effect.
///
/// A trait because only the caller knows what the effect was. The important
/// property is that it may answer [`Verdict::Unknown`], and that answering so
/// is respected rather than retried around.
pub trait Observer {
    /// Did the effect this intent describes happen?
    fn observe(&self, intent: &Unmatched) -> Verdict;
}

/// An observer that never knows.
///
/// Not a placeholder: an effect nobody can observe is a real case, and the
/// correct behavior for it is to block the retry, which this makes easy to test
/// and impossible to skip.
#[derive(Debug, Clone, Copy, Default)]
pub struct CannotObserve;

impl Observer for CannotObserve {
    fn observe(&self, _intent: &Unmatched) -> Verdict {
        Verdict::Unknown
    }
}

/// Fold the record and find every intent with no outcome.
///
/// Matching is by (run, attempt, subject): an outcome record names the intent it
/// closes by repeating those three, which is what lets a fold pair them without
/// the chain carrying back-references.
pub fn unmatched_intents(chain: &Chain) -> Vec<Unmatched> {
    let entries = chain.entries();
    let closed: Vec<(String, u32, String)> = entries
        .iter()
        .filter(|e| e.kind == Kind::Outcome)
        .map(|e| (e.run_id.clone(), e.attempt, e.subject.clone()))
        .collect();

    entries
        .iter()
        .filter(|e| e.kind == Kind::Intent)
        .filter(|e| !closed.contains(&(e.run_id.clone(), e.attempt, e.subject.clone())))
        .map(|e| Unmatched {
            run_id: e.run_id.clone(),
            attempt: e.attempt,
            subject: e.subject.clone(),
            idempotency_key: e.idempotency_key.clone(),
        })
        .collect()
}

/// Reconcile every unmatched intent. Observes; decides nothing by assumption.
pub fn reconcile(chain: &Chain, observer: &dyn Observer) -> Vec<Reconciled> {
    unmatched_intents(chain)
        .into_iter()
        .map(|intent| {
            let verdict = observer.observe(&intent);
            Reconciled { intent, verdict }
        })
        .collect()
}

/// The reconciliation record to append for a verdict.
pub fn reconciliation_entry(reconciled: &Reconciled) -> Entry {
    Entry {
        kind: Kind::Reconciliation,
        run_id: reconciled.intent.run_id.clone(),
        attempt: reconciled.intent.attempt,
        subject: reconciled.intent.subject.clone(),
        idempotency_key: reconciled.intent.idempotency_key.clone(),
        detail: serde_json::json!({
            "verdict": reconciled.verdict,
            "retryAllowed": reconciled.retry_allowed(),
            "idempotentByKey": reconciled.intent.idempotent(),
        }),
    }
}

/// Whether anything blocks a retry, and what.
///
/// Returned rather than logged: an `unknown` must be **reported to the
/// operator**, and a caller that ignores this value has to ignore it visibly.
pub fn blocked(reconciled: &[Reconciled]) -> Vec<&Reconciled> {
    reconciled.iter().filter(|r| !r.retry_allowed()).collect()
}
