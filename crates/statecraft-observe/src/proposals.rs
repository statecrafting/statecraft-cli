//! What the proposal verbs do to the chain (spec 027 sections 3.6, 3.8 and
//! 3.9): recovery of an orphaned admission, the two closing dispositions, and
//! the reads.
//!
//! Every function that appends requires the observation lock to be held by
//! its caller.

use crate::proposal::{Disposition, Entry, Ledger, Proposal, Release, State};
use crate::store::{Held, Store, StoreError};
use crate::subscription::Kind;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

/// The attempt a run record says a proposal caused: the intent whose
/// `proposal` member names it (section 3.7).
pub fn caused_attempt(
    home: &Path,
    root: &Path,
    proposal: &str,
) -> Result<Option<(String, u32)>, String> {
    let (chain, _) = statecraft_run::record::Chain::open(home, root).map_err(|e| e.to_string())?;
    Ok(chain
        .entries()
        .into_iter()
        .find(|e| {
            e.kind == statecraft_run::record::Kind::Intent
                && e.detail.get("proposal").and_then(Value::as_str) == Some(proposal)
        })
        .map(|e| (e.run_id, e.attempt)))
}

/// Recover every orphaned `admitting` entry (section 3.8): with an attempt
/// intent naming the proposal, `disposed: admitted`; without one, `released`
/// with reason `admission-interrupted`. Runs at every chain writer, once it
/// holds the lock and before its own work. Returns what it appended.
pub fn recover(
    held: &Held,
    store: &mut Store,
    home: &Path,
    root: &Path,
    at: &str,
) -> Result<Vec<Entry>, RecoverError> {
    let ledger = Ledger::fold(store.entries().to_vec());
    let mut written = Vec::new();
    for p in ledger.orphaned() {
        let State::Admitting { position } = p.state else {
            continue;
        };
        let entry = match caused_attempt(home, root, &p.id).map_err(RecoverError::RunRecord)? {
            Some((run_id, attempt)) => Entry::Disposed {
                proposal: p.id.clone(),
                disposition: Disposition::Admitted { run_id, attempt },
            },
            None => Entry::Released {
                proposal: p.id.clone(),
                admitting: position,
                reason: Release::AdmissionInterrupted,
            },
        };
        store
            .append(held, at, &entry)
            .map_err(RecoverError::Store)?;
        written.push(entry);
    }
    Ok(written)
}

/// Why recovery could not finish.
#[derive(Debug)]
pub enum RecoverError {
    /// The run record could not be read, so no orphan could be judged.
    RunRecord(String),
    /// The chain could not be written.
    Store(StoreError),
}

impl std::fmt::Display for RecoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecoverError::RunRecord(d) => write!(f, "the run record could not be read: {d}"),
            RecoverError::Store(e) => write!(f, "{e}"),
        }
    }
}

/// Why a proposal verb refused, writing nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotAdmissible {
    /// No proposal has this id.
    Unknown(String),
    /// The verb does not close this kind; names the verb that does.
    WrongKind {
        /// The proposal's kind.
        kind: Kind,
        /// The verb that closes it.
        closed_by: &'static str,
    },
    /// The proposal is not open; names where it stands.
    NotOpen(&'static str),
}

impl std::fmt::Display for NotAdmissible {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotAdmissible::Unknown(id) => write!(f, "no proposal {id} is recorded"),
            NotAdmissible::WrongKind { kind, closed_by } => write!(
                f,
                "the proposal is a `{}`, which `{closed_by}` closes",
                kind.word()
            ),
            NotAdmissible::NotOpen(state) => {
                write!(f, "the proposal is `{state}`, not open")
            }
        }
    }
}

/// The open proposal `id`, of `kind`, or why not. `closed_by` names the verb
/// that closes the other kind.
pub fn open_of_kind<'a>(
    ledger: &'a Ledger,
    id: &str,
    kind: Kind,
    closed_by: &'static str,
) -> Result<&'a Proposal, NotAdmissible> {
    let p = ledger
        .proposal(id)
        .ok_or_else(|| NotAdmissible::Unknown(id.to_string()))?;
    if p.kind != kind {
        return Err(NotAdmissible::WrongKind {
            kind: p.kind,
            closed_by,
        });
    }
    if p.state != State::Open {
        return Err(NotAdmissible::NotOpen(p.state.word()));
    }
    Ok(p)
}

/// An operator's closing act (section 3.9): the only two dispositions
/// [`close`] writes. The others are written by `observe` and `proposal admit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Closing {
    /// `proposal dismiss` closes a `run` without running it.
    Dismissed {
        /// Who.
        operator: String,
        /// Why.
        reason: String,
    },
    /// `proposal acknowledge` closes a `notice`.
    Acknowledged {
        /// Who.
        operator: String,
    },
}

/// Close an open proposal by the operator's act: `dismissed` for a `run`,
/// `acknowledged` for a `notice` (section 3.9). Refuses, writing nothing, a
/// proposal of the other kind or one that is not open.
pub fn close(
    held: &Held,
    store: &mut Store,
    id: &str,
    closing: Closing,
    at: &str,
) -> Result<Result<Entry, NotAdmissible>, StoreError> {
    let ledger = Ledger::fold(store.entries().to_vec());
    let (kind, closed_by, disposition) = match closing {
        Closing::Dismissed { operator, reason } => (
            Kind::Run,
            "proposal acknowledge",
            Disposition::Dismissed { operator, reason },
        ),
        Closing::Acknowledged { operator } => (
            Kind::Notice,
            "proposal dismiss",
            Disposition::Acknowledged { operator },
        ),
    };
    if let Err(why) = open_of_kind(&ledger, id, kind, closed_by) {
        return Ok(Err(why));
    }
    let entry = Entry::Disposed {
        proposal: id.to_string(),
        disposition,
    };
    store.append(held, at, &entry)?;
    Ok(Ok(entry))
}

/// One row of `proposal list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listed {
    /// The proposal.
    pub proposal: String,
    /// `run` or `notice`.
    pub kind: Kind,
    /// Its spec, or `None` for the repository.
    pub spec: Option<String>,
    /// The subscription.
    pub subscription: &'static str,
    /// The ref of its latest cause.
    #[serde(rename = "ref")]
    pub git_ref: String,
    /// `open`, or `admission-interrupted, not yet recovered`.
    pub state: &'static str,
}

/// `proposal list`: the open proposals in the order first proposed, and each
/// orphaned admission, reported as not yet recovered because a read holds no
/// lock and repairs nothing.
pub fn list(ledger: &Ledger) -> Vec<Listed> {
    ledger
        .proposals
        .iter()
        .filter_map(|p| {
            let state = match p.state {
                State::Open => "open",
                State::Admitting { .. } => "admission-interrupted, not yet recovered",
                State::Disposed(_) => return None,
            };
            Some(Listed {
                proposal: p.id.clone(),
                kind: p.kind,
                spec: p.scope.spec().map(str::to_string),
                subscription: p.subscription.id(),
                git_ref: p.latest_ref().to_string(),
                state,
            })
        })
        .collect()
}

/// One cause in a proposal's trace: the observation, its commit and ref, the
/// commit it was compared with (`None` for an `initial` entry), and the
/// transitions that touched the proposal's scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cause {
    /// `proposed` or `recurred`.
    pub entry: &'static str,
    /// The observation.
    pub observation: String,
    /// Its ref.
    #[serde(rename = "ref")]
    pub git_ref: String,
    /// Its commit.
    pub commit: String,
    /// The commit its transitions compared with: the last answered
    /// observation before it, or the observation before it for a
    /// `corpus-state` transition. `None` for the first observation.
    pub compared_with: Option<String>,
    /// Its transitions in the proposal's scope.
    pub transitions: Vec<crate::transition::Transition>,
}

/// Every cause of a proposal, oldest first, and each disposition entry for it
/// with its chain position.
pub fn trace(
    ledger: &Ledger,
    entries: &[Entry],
    p: &Proposal,
) -> (Vec<Cause>, Vec<(usize, Entry)>) {
    let mut causes = Vec::new();
    for (i, (observation, git_ref, _)) in p.causes.iter().enumerate() {
        let Some(index) = ledger
            .observations
            .iter()
            .position(|(_, o)| &o.observation == observation)
        else {
            continue;
        };
        let o = &ledger.observations[index].1;
        let before = &ledger.observations[..index];
        let compared_with = if p.scope.spec().is_none() {
            before.last().map(|(_, b)| b.commit.clone())
        } else {
            before
                .iter()
                .rev()
                .find(|(_, b)| b.projections.corpus_state.answered())
                .map(|(_, b)| b.commit.clone())
        };
        causes.push(Cause {
            entry: if i == 0 { "proposed" } else { "recurred" },
            observation: observation.clone(),
            git_ref: git_ref.clone(),
            commit: o.commit.clone(),
            compared_with,
            transitions: o
                .transitions
                .iter()
                .filter(|t| t.spec() == p.scope.spec())
                .cloned()
                .collect(),
        });
    }
    let dispositions = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| match e {
            Entry::Admitting { proposal, .. }
            | Entry::Released { proposal, .. }
            | Entry::Disposed { proposal, .. } => proposal == &p.id,
            _ => false,
        })
        .map(|(i, e)| (i, e.clone()))
        .collect();
    (causes, dispositions)
}

/// How an admission ended (section 3.6).
#[derive(Debug)]
pub enum Admission<A> {
    /// The fresh observation disagrees with the proposal: `disposed: stale`,
    /// naming what differs. No run.
    Stale {
        /// What differs.
        differing: String,
        /// The fresh observation.
        observation: crate::observe::Observation,
    },
    /// The run path appended an attempt naming the proposal: `disposed:
    /// admitted`. `answer` is the run's own.
    Admitted {
        /// The run.
        run_id: String,
        /// The attempt.
        attempt: u32,
        /// What the run path answered.
        answer: A,
    },
    /// The run path refused before any attempt named the proposal:
    /// `released`, reason `run-refused`; the proposal is open again.
    Released {
        /// What the run path answered.
        answer: A,
    },
}

/// Why an admission recorded nothing of its own.
#[derive(Debug)]
pub enum AdmitError {
    /// Not a `run` proposal, or not open.
    NotAdmissible(NotAdmissible),
    /// The fresh observation recorded nothing.
    Observe(crate::observe::Refusal),
    /// The run record could not be read after the run path returned.
    RunRecord(String),
    /// The chain records a `run` proposal with no spec in its scope, which
    /// no observation writes; nothing is admitted against it.
    Unscoped(String),
    /// The chain could not be written.
    Store(StoreError),
}

/// `proposal admit` (section 3.6), under the caller's hold of the
/// observation lock, after recovery: a fresh observation of the proposal's
/// latest ref; `stale` when it is refused, the spec is not eligible, or the
/// closure digest differs; otherwise `admitting`, then `enter` (spec 003's run
/// path at the fresh commit, naming the proposal in its intent), then
/// `admitted` when an attempt intent names the proposal and `released` when
/// none does. Which of the two is read from the run record, never from the
/// run path's answer.
#[allow(clippy::too_many_arguments)]
pub fn admit<A>(
    held: &Held,
    store: &mut Store,
    request: &crate::observe::Request<'_>,
    sources: &crate::observe::Sources<'_>,
    id: &str,
    operator: &str,
    reason: &str,
    enter: impl FnOnce(&str, &str) -> A,
) -> Result<Admission<A>, AdmitError> {
    use crate::projection::Eligibility;
    let ledger = Ledger::fold(store.entries().to_vec());
    let p = open_of_kind(&ledger, id, Kind::Run, "proposal acknowledge")
        .map_err(AdmitError::NotAdmissible)?
        .clone();
    let spec = p
        .scope
        .spec()
        .ok_or_else(|| AdmitError::Unscoped(format!("the run proposal {id} names no spec")))?
        .to_string();
    let fresh = crate::observe::observe(
        held,
        store,
        &crate::observe::Request {
            git_ref: Some(p.latest_ref()),
            ..*request
        },
        sources,
    )
    .map_err(AdmitError::Observe)?;
    let ledger = Ledger::fold(store.entries().to_vec());
    let now = ledger
        .previous()
        .expect("the fresh observation is recorded or equal to the last");
    let projections = &now.projections;
    let differing = if !projections.corpus_state.answered() {
        Some(format!(
            "the fresh observation is {}",
            projections.corpus_state.word()
        ))
    } else {
        match projections.work_eligibility.get(&spec) {
            None => Some(format!("{spec} is no longer offered by the plan")),
            Some(Eligibility::Excluded { reason }) => Some(format!("{spec} is excluded: {reason}")),
            Some(Eligibility::Eligible) => {
                let was = p.contract().unwrap_or_default();
                match projections.contract_identity.get(&spec) {
                    Some(c) if c.digest() == Some(was) => None,
                    Some(c) => Some(format!(
                        "the contract moved from {was} to {}",
                        c.digest()
                            .map_or_else(|| "unavailable".to_string(), str::to_string)
                    )),
                    None => Some(format!("no contract is bound for {spec}")),
                }
            }
        }
    };
    if let Some(differing) = differing {
        store
            .append(
                held,
                request.at,
                &Entry::Disposed {
                    proposal: id.to_string(),
                    disposition: Disposition::Stale {
                        differing: differing.clone(),
                    },
                },
            )
            .map_err(AdmitError::Store)?;
        return Ok(Admission::Stale {
            differing,
            observation: fresh,
        });
    }
    let admitting = store.entries().len();
    store
        .append(
            held,
            request.at,
            &Entry::Admitting {
                proposal: id.to_string(),
                observation: now.observation.clone(),
                operator: operator.to_string(),
                reason: reason.to_string(),
            },
        )
        .map_err(AdmitError::Store)?;
    let commit = now.commit.clone();
    let answer = enter(&spec, &commit);
    match caused_attempt(request.home, request.root, id).map_err(AdmitError::RunRecord)? {
        Some((run_id, attempt)) => {
            store
                .append(
                    held,
                    request.at,
                    &Entry::Disposed {
                        proposal: id.to_string(),
                        disposition: Disposition::Admitted {
                            run_id: run_id.clone(),
                            attempt,
                        },
                    },
                )
                .map_err(AdmitError::Store)?;
            Ok(Admission::Admitted {
                run_id,
                attempt,
                answer,
            })
        }
        None => {
            store
                .append(
                    held,
                    request.at,
                    &Entry::Released {
                        proposal: id.to_string(),
                        admitting,
                        reason: Release::RunRefused,
                    },
                )
                .map_err(AdmitError::Store)?;
            Ok(Admission::Released { answer })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proposal::plan;
    use crate::subscription::{Candidate, Scope, Subscription};
    use serde_json::json;

    fn candidate(kind: Kind) -> Candidate {
        Candidate {
            subscription: match kind {
                Kind::Run => Subscription::EligibleWork,
                Kind::Notice => Subscription::WithdrawnWork,
            },
            kind,
            scope: Scope::Spec {
                id: "001-a".to_string(),
            },
            row: json!({ "contract": "c1" }),
        }
    }

    fn seeded(home: &Path, root: &Path, kinds: &[Kind]) -> (Held, Store, Vec<String>) {
        let held = crate::store::lock(home, root).unwrap();
        let mut store = Store::open(home, root).unwrap();
        let candidates: Vec<_> = kinds.iter().map(|k| candidate(*k)).collect();
        let ledger = Ledger::fold(store.entries().to_vec());
        for e in plan(&ledger, "o1", "main", &candidates, |_, _| Value::Null) {
            store.append(&held, "t", &e).unwrap();
        }
        (held, store, candidates.iter().map(Candidate::id).collect())
    }

    #[test]
    fn each_kind_is_closed_only_by_its_own_verb_and_only_once() {
        let home = tempfile::tempdir().unwrap();
        let root = Path::new("/fixture/a");
        let (held, mut store, ids) = seeded(home.path(), root, &[Kind::Run, Kind::Notice]);
        let ack = Closing::Acknowledged {
            operator: "sam".to_string(),
        };
        let dismiss = Closing::Dismissed {
            operator: "sam".to_string(),
            reason: "no".to_string(),
        };
        assert_eq!(
            close(&held, &mut store, &ids[0], ack.clone(), "t").unwrap(),
            Err(NotAdmissible::WrongKind {
                kind: Kind::Run,
                closed_by: "proposal dismiss"
            })
        );
        assert!(
            close(&held, &mut store, &ids[0], dismiss.clone(), "t")
                .unwrap()
                .is_ok()
        );
        assert_eq!(
            close(&held, &mut store, &ids[0], dismiss, "t").unwrap(),
            Err(NotAdmissible::NotOpen("dismissed"))
        );
        assert!(close(&held, &mut store, &ids[1], ack, "t").unwrap().is_ok());
        assert!(list(&Ledger::fold(store.entries().to_vec())).is_empty());
    }

    #[test]
    fn an_orphaned_admission_without_an_intent_is_released_and_listed_until_then() {
        let home = tempfile::tempdir().unwrap();
        let root = Path::new("/fixture/a");
        let (held, mut store, ids) = seeded(home.path(), root, &[Kind::Run]);
        store
            .append(
                &held,
                "t",
                &Entry::Admitting {
                    proposal: ids[0].clone(),
                    observation: "o2".to_string(),
                    operator: "sam".to_string(),
                    reason: "go".to_string(),
                },
            )
            .unwrap();
        let listed = list(&Ledger::fold(store.entries().to_vec()));
        assert_eq!(listed[0].state, "admission-interrupted, not yet recovered");
        let written = recover(&held, &mut store, home.path(), root, "t").unwrap();
        assert!(matches!(
            written.as_slice(),
            [Entry::Released {
                reason: Release::AdmissionInterrupted,
                ..
            }]
        ));
        let ledger = Ledger::fold(store.entries().to_vec());
        assert_eq!(ledger.proposal(&ids[0]).unwrap().state, State::Open);
        assert!(
            recover(&held, &mut store, home.path(), root, "t")
                .unwrap()
                .is_empty()
        );
    }
}
