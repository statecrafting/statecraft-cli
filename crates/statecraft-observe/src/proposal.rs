//! The observation chain's entries, the state folded from them, and what an
//! observation appends for the candidates it found (spec 027 sections 3.5 and
//! 3.8).
//!
//! There is no mutable checkpoint: the last answered observation, and every
//! proposal's state, is read by folding the chain. This module never touches a
//! file; the store appends what [`plan`] returns.

use crate::projection::{ProjectionDigests, Projections};
use crate::subscription::{Candidate, Kind, Scope, Subscription};
use crate::transition::Transition;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// An `observed` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observed {
    /// The observation id.
    pub observation: String,
    /// The input identity (section 3.1 rule 2).
    pub input_identity: String,
    /// The ref as named.
    #[serde(rename = "ref")]
    pub git_ref: String,
    /// The commit it resolved to.
    pub commit: String,
    /// spec-spine's message, for a refused observation.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub message: Option<String>,
    /// The projections' values, which later observations compare with.
    pub projections: Projections,
    /// Each projection's canonical digest.
    pub digests: ProjectionDigests,
    /// The transitions, in section 3.3's order.
    pub transitions: Vec<Transition>,
}

/// Why an `admitting` entry ended without a disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Release {
    /// spec 003 refused the run (section 3.6).
    RunRefused,
    /// The admitting process died before any attempt intent (section 3.8).
    AdmissionInterrupted,
}

/// How a proposal was disposed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "disposition", rename_all = "kebab-case")]
pub enum Disposition {
    /// Entered spec 003's run path as this run's attempt.
    Admitted {
        /// The run.
        #[serde(rename = "runId")]
        run_id: String,
        /// The attempt.
        attempt: u32,
    },
    /// The repository moved: the value that differs.
    Stale {
        /// What differed.
        differing: String,
    },
    /// The operator closed a `run` without running it.
    Dismissed {
        /// Who.
        operator: String,
        /// Why.
        reason: String,
    },
    /// The operator saw a `notice`.
    Acknowledged {
        /// Who.
        operator: String,
    },
    /// Another `run` proposal for the same spec is open in its place.
    Superseded {
        /// The proposal that superseded it.
        by: String,
    },
}

impl Disposition {
    /// The disposition, as section 3.8 spells it.
    pub fn word(&self) -> &'static str {
        match self {
            Disposition::Admitted { .. } => "admitted",
            Disposition::Stale { .. } => "stale",
            Disposition::Dismissed { .. } => "dismissed",
            Disposition::Acknowledged { .. } => "acknowledged",
            Disposition::Superseded { .. } => "superseded",
        }
    }

    /// Whether this is a decision about the work, which a recurrence does
    /// not reopen: only `admitted` and `dismissed` (section 5, 2026-09-29).
    pub fn decided(&self) -> bool {
        matches!(
            self,
            Disposition::Admitted { .. } | Disposition::Dismissed { .. }
        )
    }
}

/// One chain entry. The set is closed (section 3.8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "entry", rename_all = "kebab-case")]
pub enum Entry {
    /// One observation.
    Observed(Box<Observed>),
    /// A new proposal.
    Proposed {
        /// Its identity.
        proposal: String,
        /// What it proposes.
        kind: Kind,
        /// What it is about.
        scope: Scope,
        /// The subscription that matched.
        subscription: Subscription,
        /// The justifying row.
        row: Value,
        /// What the proposal names beside its identity (for a notice, the
        /// open proposals and attempts it concerns); never part of the
        /// identity.
        #[serde(default, skip_serializing_if = "Value::is_null")]
        detail: Value,
        /// The observation that made it.
        observation: String,
        /// That observation's ref.
        #[serde(rename = "ref")]
        git_ref: String,
    },
    /// An existing proposal, caused again.
    Recurred {
        /// The proposal.
        proposal: String,
        /// The observation that caused it again.
        observation: String,
        /// That observation's ref.
        #[serde(rename = "ref")]
        git_ref: String,
    },
    /// An admission on its way into the run path.
    Admitting {
        /// The proposal.
        proposal: String,
        /// The admission's fresh observation.
        observation: String,
        /// Who admitted it.
        operator: String,
        /// Why.
        reason: String,
    },
    /// An `admitting` entry ended without a disposition; the proposal is open.
    Released {
        /// The proposal.
        proposal: String,
        /// The chain position of the `admitting` entry this ends.
        admitting: usize,
        /// Why.
        reason: Release,
    },
    /// A proposal disposed.
    Disposed {
        /// The proposal.
        proposal: String,
        /// How.
        #[serde(flatten)]
        disposition: Disposition,
    },
}

/// Where a proposal stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Open: it may be admitted, dismissed or acknowledged by its kind.
    Open,
    /// An admission wrote `admitting` at this chain position and nothing has
    /// ended it yet.
    Admitting {
        /// The `admitting` entry's position.
        position: usize,
    },
    /// Disposed.
    Disposed(Disposition),
}

impl State {
    /// The state as a word.
    pub fn word(&self) -> &'static str {
        match self {
            State::Open => "open",
            State::Admitting { .. } => "admitting",
            State::Disposed(d) => d.word(),
        }
    }
}

/// One proposal, as the chain says it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    /// Its identity.
    pub id: String,
    /// What it proposes.
    pub kind: Kind,
    /// What it is about.
    pub scope: Scope,
    /// The subscription that matched.
    pub subscription: Subscription,
    /// The justifying row.
    pub row: Value,
    /// Every observation that caused it, with its ref and chain position:
    /// the first is the `proposed` entry, the rest `recurred`.
    pub causes: Vec<(String, String, usize)>,
    /// Where it stands.
    pub state: State,
}

impl Proposal {
    /// The ref of its latest cause, which admission re-observes.
    pub fn latest_ref(&self) -> &str {
        self.causes.last().map(|c| c.1.as_str()).unwrap_or_default()
    }

    /// The closure digest a `run` proposal was made under.
    pub fn contract(&self) -> Option<&str> {
        self.row.get("contract").and_then(Value::as_str)
    }
}

/// The state folded from a chain.
#[derive(Debug, Clone, Default)]
pub struct Ledger {
    /// Every `observed` entry, oldest first, with its chain position.
    pub observations: Vec<(usize, Observed)>,
    /// Every proposal, in the order first proposed.
    pub proposals: Vec<Proposal>,
    index: BTreeMap<String, usize>,
    len: usize,
}

impl Ledger {
    /// Fold a chain's entries, oldest first.
    pub fn fold(entries: impl IntoIterator<Item = Entry>) -> Ledger {
        let mut ledger = Ledger::default();
        for entry in entries {
            ledger.apply(entry);
        }
        ledger
    }

    /// How many entries were folded: the next entry's position.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether nothing was folded.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The observation immediately before the next one.
    pub fn previous(&self) -> Option<&Observed> {
        self.observations.last().map(|(_, o)| o)
    }

    /// The last answered observation.
    pub fn last_answered(&self) -> Option<&Observed> {
        self.observations
            .iter()
            .rev()
            .map(|(_, o)| o)
            .find(|o| o.projections.corpus_state.answered())
    }

    /// A proposal by id.
    pub fn proposal(&self, id: &str) -> Option<&Proposal> {
        self.index.get(id).map(|&i| &self.proposals[i])
    }

    /// The open proposals, in the order first proposed.
    pub fn open(&self) -> impl Iterator<Item = &Proposal> {
        self.proposals.iter().filter(|p| p.state == State::Open)
    }

    /// Every proposal whose `admitting` entry nothing has ended. Only a dead
    /// writer leaves one, because a live admission holds the lock.
    pub fn orphaned(&self) -> impl Iterator<Item = &Proposal> {
        self.proposals
            .iter()
            .filter(|p| matches!(p.state, State::Admitting { .. }))
    }

    /// Apply one entry.
    pub fn apply(&mut self, entry: Entry) {
        let position = self.len;
        self.len += 1;
        match entry {
            Entry::Observed(o) => self.observations.push((position, *o)),
            Entry::Proposed {
                proposal,
                kind,
                scope,
                subscription,
                row,
                observation,
                git_ref,
                ..
            } => {
                if self.index.contains_key(&proposal) {
                    return;
                }
                self.index.insert(proposal.clone(), self.proposals.len());
                self.proposals.push(Proposal {
                    id: proposal,
                    kind,
                    scope,
                    subscription,
                    row,
                    causes: vec![(observation, git_ref, position)],
                    state: State::Open,
                });
            }
            Entry::Recurred {
                proposal,
                observation,
                git_ref,
            } => {
                if let Some(p) = self.get_mut(&proposal) {
                    p.causes.push((observation, git_ref, position));
                    // Section 3.5 rule 2: a recurrence reopens what nobody
                    // decided, and leaves `admitted` and `dismissed` standing.
                    if let State::Disposed(d) = &p.state
                        && !d.decided()
                    {
                        p.state = State::Open;
                    }
                }
            }
            Entry::Admitting { proposal, .. } => {
                if let Some(p) = self.get_mut(&proposal) {
                    p.state = State::Admitting { position };
                }
            }
            Entry::Released { proposal, .. } => {
                if let Some(p) = self.get_mut(&proposal) {
                    p.state = State::Open;
                }
            }
            Entry::Disposed {
                proposal,
                disposition,
            } => {
                if let Some(p) = self.get_mut(&proposal) {
                    p.state = State::Disposed(disposition);
                }
            }
        }
    }

    fn get_mut(&mut self, id: &str) -> Option<&mut Proposal> {
        self.index.get(id).map(|&i| &mut self.proposals[i])
    }
}

/// What one observation appends for its candidates, in order, given the chain
/// as folded before it (with the `observed` entry already applied).
///
/// A new identity is `proposed`; an existing one `recurred` (section 3.5). A
/// `run` proposal that is open after this, new or reopened, supersedes any
/// other open `run` proposal for the same spec, so at most one is open per
/// spec. `detail` supplies what a new proposal names beside its identity.
pub fn plan(
    ledger: &Ledger,
    observation: &str,
    git_ref: &str,
    candidates: &[Candidate],
    mut detail: impl FnMut(&Candidate, &Ledger) -> Value,
) -> Vec<Entry> {
    let mut working = ledger.clone();
    let mut out = Vec::new();
    let push = |working: &mut Ledger, out: &mut Vec<Entry>, entry: Entry| {
        working.apply(entry.clone());
        out.push(entry);
    };
    for candidate in candidates {
        let id = candidate.id();
        let entry = if working.proposal(&id).is_some() {
            Entry::Recurred {
                proposal: id.clone(),
                observation: observation.to_string(),
                git_ref: git_ref.to_string(),
            }
        } else {
            Entry::Proposed {
                proposal: id.clone(),
                kind: candidate.kind,
                scope: candidate.scope.clone(),
                subscription: candidate.subscription,
                row: candidate.row.clone(),
                detail: detail(candidate, &working),
                observation: observation.to_string(),
                git_ref: git_ref.to_string(),
            }
        };
        push(&mut working, &mut out, entry);
        let now_open = working
            .proposal(&id)
            .is_some_and(|p| p.state == State::Open);
        if candidate.kind == Kind::Run && now_open {
            let superseded: Vec<String> = working
                .open()
                .filter(|p| p.kind == Kind::Run && p.scope == candidate.scope && p.id != id)
                .map(|p| p.id.clone())
                .collect();
            for old in superseded {
                push(
                    &mut working,
                    &mut out,
                    Entry::Disposed {
                        proposal: old,
                        disposition: Disposition::Superseded { by: id.clone() },
                    },
                );
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run(spec: &str, digest: &str) -> Candidate {
        Candidate {
            subscription: Subscription::EligibleWork,
            kind: Kind::Run,
            scope: Scope::Spec {
                id: spec.to_string(),
            },
            row: json!({ "eligibility": "eligible", "contract": digest }),
        }
    }
    fn notice(spec: &str) -> Candidate {
        Candidate {
            subscription: Subscription::WithdrawnWork,
            kind: Kind::Notice,
            scope: Scope::Spec {
                id: spec.to_string(),
            },
            row: json!({ "before": 1, "after": 2 }),
        }
    }
    fn observe(ledger: &mut Ledger, n: &str, candidates: &[Candidate]) -> Vec<Entry> {
        let entries = plan(ledger, n, "main", candidates, |_, _| Value::Null);
        for e in entries.clone() {
            ledger.apply(e);
        }
        entries
    }
    fn words(entries: &[Entry]) -> Vec<&'static str> {
        entries
            .iter()
            .map(|e| match e {
                Entry::Observed(_) => "observed",
                Entry::Proposed { .. } => "proposed",
                Entry::Recurred { .. } => "recurred",
                Entry::Admitting { .. } => "admitting",
                Entry::Released { .. } => "released",
                Entry::Disposed { disposition, .. } => disposition.word(),
            })
            .collect()
    }
    fn dispose(ledger: &mut Ledger, id: &str, d: Disposition) {
        ledger.apply(Entry::Disposed {
            proposal: id.to_string(),
            disposition: d,
        });
    }

    #[test]
    fn the_same_fact_from_a_second_commit_recurs_and_stays_open() {
        let mut l = Ledger::default();
        let c = run("001-a", "c1");
        assert_eq!(
            words(&observe(&mut l, "o1", std::slice::from_ref(&c))),
            ["proposed"]
        );
        assert_eq!(
            words(&observe(&mut l, "o2", std::slice::from_ref(&c))),
            ["recurred"]
        );
        let p = l.proposal(&c.id()).unwrap();
        assert_eq!(p.state, State::Open);
        assert_eq!(p.causes.len(), 2);
    }

    #[test]
    fn a_dismissal_and_an_admission_stand_against_a_recurrence() {
        for d in [
            Disposition::Dismissed {
                operator: "sam".to_string(),
                reason: "no".to_string(),
            },
            Disposition::Admitted {
                run_id: "001-a".to_string(),
                attempt: 1,
            },
        ] {
            let mut l = Ledger::default();
            let c = run("001-a", "c1");
            observe(&mut l, "o1", std::slice::from_ref(&c));
            dispose(&mut l, &c.id(), d.clone());
            assert_eq!(
                words(&observe(&mut l, "o2", std::slice::from_ref(&c))),
                ["recurred"]
            );
            assert_eq!(l.proposal(&c.id()).unwrap().state, State::Disposed(d));
            assert_eq!(l.open().count(), 0);
        }
    }

    #[test]
    fn stale_superseded_and_acknowledged_reopen_on_recurrence() {
        for (c, d) in [
            (
                run("001-a", "c1"),
                Disposition::Stale {
                    differing: "x".to_string(),
                },
            ),
            (
                run("001-a", "c1"),
                Disposition::Superseded {
                    by: "other".to_string(),
                },
            ),
            (
                notice("001-a"),
                Disposition::Acknowledged {
                    operator: "sam".to_string(),
                },
            ),
        ] {
            let mut l = Ledger::default();
            observe(&mut l, "o1", std::slice::from_ref(&c));
            dispose(&mut l, &c.id(), d);
            observe(&mut l, "o2", std::slice::from_ref(&c));
            assert_eq!(l.proposal(&c.id()).unwrap().state, State::Open);
        }
    }

    #[test]
    fn a_new_contract_supersedes_the_open_run_and_a_reopened_one_supersedes_too() {
        let mut l = Ledger::default();
        let (c1, c2) = (run("001-a", "c1"), run("001-a", "c2"));
        observe(&mut l, "o1", std::slice::from_ref(&c1));
        assert_eq!(
            words(&observe(&mut l, "o2", std::slice::from_ref(&c2))),
            ["proposed", "superseded"]
        );
        assert_eq!(
            l.open().map(|p| p.id.clone()).collect::<Vec<_>>(),
            [c2.id()]
        );
        // c1 recurs: it was superseded, nobody decided it, so it reopens and
        // supersedes c2 in turn.
        assert_eq!(
            words(&observe(&mut l, "o3", std::slice::from_ref(&c1))),
            ["recurred", "superseded"]
        );
        assert_eq!(
            l.open().map(|p| p.id.clone()).collect::<Vec<_>>(),
            [c1.id()]
        );
        // Another spec's run is untouched.
        observe(&mut l, "o4", &[run("002-b", "c9")]);
        assert_eq!(l.open().count(), 2);
    }

    #[test]
    fn admitting_and_release_and_the_orphan_fold() {
        let mut l = Ledger::default();
        let c = run("001-a", "c1");
        observe(&mut l, "o1", std::slice::from_ref(&c));
        let position = l.len();
        l.apply(Entry::Admitting {
            proposal: c.id(),
            observation: "o2".to_string(),
            operator: "sam".to_string(),
            reason: "go".to_string(),
        });
        assert_eq!(
            l.orphaned().map(|p| p.state.clone()).collect::<Vec<_>>(),
            [State::Admitting { position }]
        );
        l.apply(Entry::Released {
            proposal: c.id(),
            admitting: position,
            reason: Release::AdmissionInterrupted,
        });
        assert_eq!(l.proposal(&c.id()).unwrap().state, State::Open);
        assert_eq!(l.orphaned().count(), 0);
    }

    #[test]
    fn entries_round_trip_and_the_last_answered_observation_is_read_from_the_chain() {
        let observed = |n: &str, p: Projections| {
            Entry::Observed(Box::new(Observed {
                observation: n.to_string(),
                input_identity: format!("i-{n}"),
                git_ref: "main".to_string(),
                commit: "abc".to_string(),
                message: None,
                digests: p.digests(),
                projections: p,
                transitions: Vec::new(),
            }))
        };
        let entries = vec![
            observed(
                "o1",
                Projections::answered(Default::default(), |_| unreachable!()),
            ),
            observed("o2", Projections::refused("stale")),
            Entry::Disposed {
                proposal: "p".to_string(),
                disposition: Disposition::Admitted {
                    run_id: "001-a".to_string(),
                    attempt: 2,
                },
            },
        ];
        for e in &entries {
            let text = serde_json::to_string(e).unwrap();
            assert_eq!(&serde_json::from_str::<Entry>(&text).unwrap(), e, "{text}");
        }
        let l = Ledger::fold(entries);
        assert_eq!(l.previous().unwrap().observation, "o2");
        assert_eq!(l.last_answered().unwrap().observation, "o1");
        assert_eq!(l.len(), 3);
    }
}
