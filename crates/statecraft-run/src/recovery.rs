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

use crate::record::{Chain, EffectId, Entry, Identity, Kind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
///
/// Section 3.3.1 clause 7 refines what this considers: **only records whose
/// `effectId` key is absent**. A record carrying an identity, valid or invalid,
/// is not folded here. No record written before section 3.3.1 carries the key,
/// so this answer is unchanged for every chain that exists.
pub fn unmatched_intents(chain: &Chain) -> Vec<Unmatched> {
    let entries: Vec<Entry> = chain
        .entries()
        .into_iter()
        .filter(|e| e.effect_id.is_absent())
        .collect();
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
        // Mechanical. Carrying an identity into a reconciliation record is
        // identity-aware reconciliation, which section 3.3.1 clause 9 excludes.
        effect_id: Identity::Absent,
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

// ---------------------------------------------------------------------------
// Section 3.3.1: the identity-aware fold.
//
// Beside the legacy pairing above, never replacing it. `Unmatched`,
// `Reconciled`, `Verdict`, `Observer`, `CannotObserve`, `reconcile`,
// `reconciliation_entry` and `blocked` keep their signatures and their behavior
// on every record that exists today.
// ---------------------------------------------------------------------------

/// The whole correlation key: `(run_id, effectId)` and nothing else.
///
/// One type, so no result can carry half of it. Section 3.3.1 clause 2:
/// `subject` is never a correlation key, `attempt` is never a correlation key,
/// and neither is consulted as a tie-breaker.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EffectKey {
    /// The run the identity is unique within.
    pub run_id: String,
    /// The identity, as the record carried it.
    pub effect_id: EffectId,
}

impl std::fmt::Display for EffectKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.run_id, self.effect_id)
    }
}

/// An identity-bearing intent with no outcome.
///
/// Keeps the full key, and carries the legacy view beside it rather than
/// replacing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmatchedEffect {
    /// Both halves of the correlation key.
    pub key: EffectKey,
    /// The same intent as the legacy fold would describe it.
    pub intent: Unmatched,
}

/// What the fold found that it must not resolve by inference.
///
/// Section 3.3.1 clause 6: every one of these is reported, unfiltered and
/// unsummarized. A defect does not make an unrelated effect unmatched and does
/// not make a matched effect unresolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoldDefect {
    /// An `effectId` key present with a value that is not a non-empty string.
    ///
    /// The one variant with no [`EffectKey`], because there is no valid
    /// identity to key it by. It carries the run, the record's position in the
    /// chain, and the value as the record carried it.
    InvalidIdentity {
        /// The run the record belongs to.
        run_id: String,
        /// The record's position in the chain.
        record_index: usize,
        /// The value as carried.
        raw: serde_json::Value,
    },
    /// A second intent of one run carrying an identity an earlier intent
    /// already carried. Clause 4: both are retained and neither is chosen.
    DuplicateIntent {
        /// The ambiguous key.
        key: EffectKey,
        /// The first intent's position.
        first: usize,
        /// The second intent's position.
        second: usize,
        /// Whether the second intent followed the first's closure. Closure does
        /// not release an identity for reuse; this records which shape it was.
        second_follows_closure: bool,
    },
    /// A second outcome naming an identity that is already closed. Clause 5:
    /// it neither replaces the first nor is absorbed by it.
    DoubleClose {
        /// The key both outcomes named.
        key: EffectKey,
        /// The first outcome's position.
        first: usize,
        /// The second outcome's position.
        second: usize,
    },
    /// An outcome naming an identity no **preceding** intent carries.
    ///
    /// Clause 5: records are folded in chain order, so an intent recorded after
    /// this outcome does not retroactively match it. The intent stays open
    /// until a subsequent outcome closes it, and this defect stays reported
    /// even then.
    OrphanOutcome {
        /// The key the outcome named.
        key: EffectKey,
        /// The outcome's position.
        record_index: usize,
    },
    /// An outcome naming an ambiguous identity. It closes nothing.
    AmbiguousClose {
        /// The ambiguous key.
        key: EffectKey,
        /// The outcome's position.
        record_index: usize,
    },
}

impl FoldDefect {
    /// The correlation key, where the defect has one.
    #[must_use]
    pub fn key(&self) -> Option<&EffectKey> {
        match self {
            FoldDefect::InvalidIdentity { .. } => None,
            FoldDefect::DuplicateIntent { key, .. }
            | FoldDefect::DoubleClose { key, .. }
            | FoldDefect::OrphanOutcome { key, .. }
            | FoldDefect::AmbiguousClose { key, .. } => Some(key),
        }
    }

    /// What was found, naming the run, the identity where there is one, and the
    /// condition. For a report an operator reads, not for a machine to parse.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            FoldDefect::InvalidIdentity {
                run_id,
                record_index,
                raw,
            } => format!(
                "run {run_id}, record {record_index}: effectId is present and is not a \
                 non-empty string: {raw}"
            ),
            FoldDefect::DuplicateIntent {
                key,
                first,
                second,
                second_follows_closure,
            } => {
                let shape = if *second_follows_closure {
                    "after the first was closed"
                } else {
                    "while the first was still open"
                };
                format!(
                    "{key}: a second intent at record {second} repeats the identity of \
                     record {first}, {shape}; the identity is ambiguous"
                )
            }
            FoldDefect::DoubleClose { key, first, second } => format!(
                "{key}: record {second} closes an identity already closed by record \
                 {first}; the first outcome stands"
            ),
            FoldDefect::OrphanOutcome { key, record_index } => format!(
                "{key}: record {record_index} is an outcome for an identity no \
                 preceding intent carries"
            ),
            FoldDefect::AmbiguousClose { key, record_index } => format!(
                "{key}: record {record_index} closes an ambiguous identity; it closes \
                 nothing"
            ),
        }
    }
}

/// The identity-aware answer. Every list is keyed by `(run_id, effectId)`.
///
/// The fields are private so the lists cannot fall out of step with each other;
/// that is consistency, not a mechanism forcing anyone to read [`Self::defects`].
/// Section 3.3.1 clause 6 puts that obligation on the caller: a caller that uses
/// this result to authorize an action must check the defects first. S1
/// introduces no production caller and no activation policy, so there is nothing
/// yet for such a rule to bind.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectFold {
    open: Vec<UnmatchedEffect>,
    closed: Vec<EffectKey>,
    ambiguous: Vec<EffectKey>,
    defects: Vec<FoldDefect>,
}

impl EffectFold {
    /// Identity-bearing intents with no outcome.
    #[must_use]
    pub fn open(&self) -> &[UnmatchedEffect] {
        &self.open
    }

    /// Identities closed exactly once.
    #[must_use]
    pub fn closed(&self) -> &[EffectKey] {
        &self.closed
    }

    /// Identities carried by more than one intent of one run. Not reported as
    /// one open effect, and no outcome closes them.
    #[must_use]
    pub fn ambiguous(&self) -> &[EffectKey] {
        &self.ambiguous
    }

    /// Every defect the fold detected, with none filtered away, collapsed or
    /// summarized out.
    #[must_use]
    pub fn defects(&self) -> &[FoldDefect] {
        &self.defects
    }

    /// Whether the fold found no defect.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.defects.is_empty()
    }
}

/// How one identity was seen while folding.
#[derive(Debug)]
struct EffectState {
    intent_index: usize,
    closed_at: Option<usize>,
    ambiguous: bool,
    intent: Unmatched,
}

/// The identity-aware read of the record (section 3.3.1).
///
/// Considers only records carrying an `effectId` key. A record whose key is
/// absent is the legacy pairing's business ([`unmatched_intents`]), and a record
/// whose key is invalid is folded by neither: it is reported as a defect and is
/// never read as a record without an identity.
///
/// Records are visited in chain order, so "the first" and "the second" in a
/// defect mean what they say. `record_index` is the record's position in the
/// chain, counting records this product cannot decode as well, because the
/// position is the thing an operator looks at.
#[must_use]
pub fn fold_effects(chain: &Chain) -> EffectFold {
    let mut fold = EffectFold::default();
    // Ordered, so the answer does not depend on hashing.
    let mut seen: BTreeMap<EffectKey, EffectState> = BTreeMap::new();

    for (index, entry) in decodable_entries(chain) {
        match &entry.effect_id {
            Identity::Absent => continue,
            Identity::Invalid(raw) => {
                fold.defects.push(FoldDefect::InvalidIdentity {
                    run_id: entry.run_id.clone(),
                    record_index: index,
                    raw: raw.clone(),
                });
            }
            Identity::Valid(effect_id) => {
                let key = EffectKey {
                    run_id: entry.run_id.clone(),
                    effect_id: effect_id.clone(),
                };
                match entry.kind {
                    Kind::Intent => record_intent(&mut fold, &mut seen, key, index, &entry),
                    Kind::Outcome => record_outcome(&mut fold, &mut seen, key, index),
                    // Clause 9: no reconciliation record carries an identity,
                    // and nothing here decides what one would mean. Accounting
                    // is the supervisor's ledger, not a bracket. Neither opens
                    // nor closes an effect.
                    Kind::Reconciliation | Kind::Accounting => {}
                }
            }
        }
    }

    for (key, state) in seen {
        if state.ambiguous {
            fold.ambiguous.push(key);
        } else if state.closed_at.is_some() {
            fold.closed.push(key);
        } else {
            fold.open.push(UnmatchedEffect {
                key,
                intent: state.intent,
            });
        }
    }

    fold
}

/// Every decodable payload with its **position in the chain**.
///
/// [`Chain::entries`] discards what it cannot decode and returns no positions,
/// so the index it would give is an index into the surviving subset. Decoding
/// here keeps the position an operator can look up.
fn decodable_entries(chain: &Chain) -> Vec<(usize, Entry)> {
    chain
        .records()
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            serde_json::from_value::<Entry>(record.payload.clone())
                .ok()
                .map(|entry| (index, entry))
        })
        .collect()
}

fn record_intent(
    fold: &mut EffectFold,
    seen: &mut BTreeMap<EffectKey, EffectState>,
    key: EffectKey,
    index: usize,
    entry: &Entry,
) {
    if let Some(state) = seen.get_mut(&key) {
        // Clause 4: closure does not release an identity for reuse, and the
        // report says which of the two shapes it found. Neither intent is
        // discarded, neither is chosen, neither overwrites the other.
        fold.defects.push(FoldDefect::DuplicateIntent {
            key,
            first: state.intent_index,
            second: index,
            second_follows_closure: state.closed_at.is_some(),
        });
        state.ambiguous = true;
        return;
    }
    seen.insert(
        key,
        EffectState {
            intent_index: index,
            closed_at: None,
            ambiguous: false,
            intent: Unmatched {
                run_id: entry.run_id.clone(),
                attempt: entry.attempt,
                subject: entry.subject.clone(),
                idempotency_key: entry.idempotency_key.clone(),
            },
        },
    );
}

fn record_outcome(
    fold: &mut EffectFold,
    seen: &mut BTreeMap<EffectKey, EffectState>,
    key: EffectKey,
    index: usize,
) {
    let Some(state) = seen.get_mut(&key) else {
        // Clause 5, in chain order: no intent for this identity has been read
        // yet, and a later one does not match this outcome retroactively.
        fold.defects.push(FoldDefect::OrphanOutcome {
            key,
            record_index: index,
        });
        return;
    };
    if state.ambiguous {
        fold.defects.push(FoldDefect::AmbiguousClose {
            key,
            record_index: index,
        });
        return;
    }
    match state.closed_at {
        Some(first) => fold.defects.push(FoldDefect::DoubleClose {
            key,
            first,
            second: index,
        }),
        None => state.closed_at = Some(index),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::chain_path;
    use attest_ledger_core::RecordChain;
    use serde_json::{Value, json};
    use std::io::Write;

    /// Write a chain file directly, so a record can carry a payload this
    /// product cannot decode as an [`Entry`].
    ///
    /// `Chain::append` only takes an `Entry`, so an undecodable payload cannot
    /// be produced through it. The records are linked with the same anchor
    /// `Chain::open` rebuilds, so the chain verifies and the only unusual thing
    /// about it is the payload.
    fn chain_with_payloads(home: &std::path::Path, target: &std::path::Path, payloads: &[Value]) {
        let anchor = format!("statecraft:{}", crate::repository::key(target));
        let mut writer = RecordChain::new(anchor);
        let path = chain_path(home, target);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
        let mut file = std::fs::File::create(&path).expect("create");
        for (i, payload) in payloads.iter().enumerate() {
            let record = writer.append(
                format!("r{i}"),
                "2026-09-19T00:00:00Z".to_string(),
                payload.clone(),
            );
            let mut line = serde_json::to_vec(&record).expect("serialize");
            line.push(b'\n');
            file.write_all(&line).expect("write");
        }
    }

    fn payload(kind: Kind, subject: &str, effect_id: Identity) -> Value {
        serde_json::to_value(Entry {
            kind,
            run_id: "run-1".to_string(),
            attempt: 1,
            subject: subject.to_string(),
            idempotency_key: None,
            detail: Value::Null,
            effect_id,
        })
        .expect("an entry serializes")
    }

    /// A defect names the record's position in the **chain**, not its position
    /// among the records this build happens to decode.
    ///
    /// The undecodable payload sits before both defects. An index taken over
    /// the decoded subset would name 1 and 2; the chain positions are 2 and 3,
    /// and those are what an operator can look up in the file.
    ///
    /// Section 3.3.1 does not change what the decoder does with a payload it
    /// cannot read: that record is still discarded, silently, and repairing
    /// that is a separate change in a separate spec's territory. This test
    /// pins the position arithmetic, not the discard.
    #[test]
    fn a_defect_names_its_position_in_the_chain_and_not_in_the_decoded_subset() {
        let home = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        chain_with_payloads(
            home.path(),
            target.path(),
            &[
                payload(Kind::Intent, "legacy", Identity::Absent),
                json!({"this": "is not an entry"}),
                payload(Kind::Intent, "broken", Identity::Invalid(json!(null))),
                payload(
                    Kind::Outcome,
                    "orphan",
                    Identity::Valid(EffectId::new("ghost").expect("non-empty")),
                ),
            ],
        );

        let (chain, report) = Chain::open(home.path(), target.path()).expect("open");
        assert_eq!(report.records, 4, "four records were written and read");
        assert_eq!(
            chain.entries().len(),
            3,
            "the undecodable payload is discarded by the decoder, as it was before S1"
        );

        let fold = fold_effects(&chain);
        assert_eq!(fold.defects().len(), 2, "got {:?}", fold.defects());
        match &fold.defects()[0] {
            FoldDefect::InvalidIdentity {
                record_index, raw, ..
            } => {
                assert_eq!(*record_index, 2, "the chain position, not 1");
                assert_eq!(*raw, json!(null));
            }
            other => panic!("expected an invalid identity, got {other:?}"),
        }
        match &fold.defects()[1] {
            FoldDefect::OrphanOutcome { key, record_index } => {
                assert_eq!(*record_index, 3, "the chain position, not 2");
                assert_eq!(key.effect_id.as_str(), "ghost");
            }
            other => panic!("expected an orphan outcome, got {other:?}"),
        }

        let legacy = unmatched_intents(&chain);
        assert_eq!(
            legacy.len(),
            1,
            "the key-absent intent is still the legacy pairing's"
        );
        assert_eq!(legacy[0].subject, "legacy");
    }
}
