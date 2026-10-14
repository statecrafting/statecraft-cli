//! The closed, built-in subscriptions (spec 027 section 3.4) and the proposal
//! identity they produce (section 3.5).
//!
//! A subscription maps one transition kind to one proposal kind. The set is
//! part of the spec, not of any repository: a target cannot add, remove or
//! reshape one, and nothing here reads a file to find one.

use crate::projection::{Contract, CorpusState, Projections, canonical_digest};
use crate::transition::Transition;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The four subscriptions that propose something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Subscription {
    /// `entered` with a digest, or `contract-moved` to a digest: a `run`.
    EligibleWork,
    /// `exited`: a `notice`.
    WithdrawnWork,
    /// `contract-moved` away from a digest: a `notice`.
    MovedContract,
    /// `corpus-changed` into `refused`: a `notice`.
    CorpusRefusal,
}

impl Subscription {
    /// The subscription id, as section 3.4 spells it.
    pub fn id(self) -> &'static str {
        match self {
            Subscription::EligibleWork => "eligible-work",
            Subscription::WithdrawnWork => "withdrawn-work",
            Subscription::MovedContract => "moved-contract",
            Subscription::CorpusRefusal => "corpus-refusal",
        }
    }
}

/// What a proposal offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// One unit of work, admitted or dismissed by the operator.
    Run,
    /// Something the operator should see; closed only by acknowledgment.
    Notice,
}

impl Kind {
    /// The kind, as section 3.4 spells it.
    pub fn word(self) -> &'static str {
        match self {
            Kind::Run => "run",
            Kind::Notice => "notice",
        }
    }
}

/// What a proposal is about: one spec, or the repository as a whole. Tagged,
/// so no spec id can collide with the repository scope (section 3.5).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "kebab-case")]
pub enum Scope {
    /// One spec.
    Spec {
        /// The spec id.
        id: String,
    },
    /// The repository: `corpus-state` is keyed by nothing.
    Repository,
}

impl Scope {
    /// The spec, for a spec scope.
    pub fn spec(&self) -> Option<&str> {
        match self {
            Scope::Spec { id } => Some(id),
            Scope::Repository => None,
        }
    }
}

/// A proposal an observation's transitions call for, before the chain says
/// whether it is new or a recurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The subscription that matched.
    pub subscription: Subscription,
    /// What it proposes.
    pub kind: Kind,
    /// What it is about.
    pub scope: Scope,
    /// The projection row that justified it: for a `run`, the eligibility row
    /// and its closure digest; for a `notice`, the two values the transition
    /// compared.
    pub row: Value,
}

impl Candidate {
    /// The proposal identity (section 3.5): the canonical digest of the
    /// subscription, the kind, the scope and the justifying row. The
    /// observation, the commit and the time are not in it, so one semantic
    /// fact reached by two commits is one proposal.
    pub fn id(&self) -> String {
        canonical_digest(&json!({
            "subscription": self.subscription.id(),
            "kind": self.kind.word(),
            "scope": self.scope,
            "row": self.row,
        }))
    }
}

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

fn notice(subscription: Subscription, scope: Scope, before: Value, after: Value) -> Candidate {
    Candidate {
        subscription,
        kind: Kind::Notice,
        scope,
        row: json!({ "before": before, "after": after }),
    }
}

fn value<T: Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("projection values serialize")
}

/// Every candidate the transitions of one observation call for, in transition
/// order. Matched once, after every projection and transition is computed, so
/// a condition on another projection reads its value in `current`.
pub fn matches(transitions: &[Transition], current: &Projections) -> Vec<Candidate> {
    let mut out = Vec::new();
    for t in transitions {
        match t {
            // A spec that enters with no digest gets no `run`: nothing could
            // bind to it, and admission would only reach spec 003's refusal.
            Transition::Entered { spec, .. } => {
                if let Some(digest) = current
                    .contract_identity
                    .get(spec)
                    .and_then(Contract::digest)
                {
                    out.push(run(spec, digest));
                }
            }
            Transition::ContractMoved {
                spec,
                before,
                after,
            } => {
                if let Some(digest) = after.digest() {
                    out.push(run(spec, digest));
                }
                // A first digest is not a move away from anything bound.
                if before.digest().is_some() {
                    out.push(notice(
                        Subscription::MovedContract,
                        Scope::Spec { id: spec.clone() },
                        value(before),
                        value(after),
                    ));
                }
            }
            Transition::Exited { spec, now } => out.push(notice(
                Subscription::WithdrawnWork,
                Scope::Spec { id: spec.clone() },
                json!({ "eligibility": "eligible" }),
                now.as_ref()
                    .map_or(json!({ "eligibility": "absent" }), value),
            )),
            Transition::CorpusChanged { before, after } => {
                if matches!(after, CorpusState::Refused { .. }) {
                    out.push(notice(
                        Subscription::CorpusRefusal,
                        Scope::Repository,
                        value(before),
                        value(after),
                    ));
                }
            }
            // Recorded as transitions; they propose nothing.
            Transition::ReasonChanged { .. } => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projection::Eligibility;
    use std::collections::BTreeMap;

    fn digest(d: &str) -> Contract {
        Contract::Digest {
            digest: d.to_string(),
        }
    }
    fn unavailable() -> Contract {
        Contract::Unavailable {
            reason: "no closure".to_string(),
        }
    }
    fn with(contracts: &[(&str, Contract)]) -> Projections {
        Projections::answered(
            contracts
                .iter()
                .map(|(id, _)| (id.to_string(), Eligibility::Eligible))
                .collect::<BTreeMap<_, _>>(),
            |id| contracts.iter().find(|c| c.0 == id).unwrap().1.clone(),
        )
    }
    fn subs(c: &[Candidate]) -> Vec<(&'static str, Kind)> {
        c.iter().map(|c| (c.subscription.id(), c.kind)).collect()
    }

    #[test]
    fn an_entry_proposes_a_run_only_when_a_contract_is_bound() {
        let t = [
            Transition::Entered {
                spec: "001-a".to_string(),
                initial: true,
            },
            Transition::Entered {
                spec: "002-b".to_string(),
                initial: false,
            },
        ];
        let now = with(&[("001-a", digest("c")), ("002-b", unavailable())]);
        let got = matches(&t, &now);
        assert_eq!(subs(&got), [("eligible-work", Kind::Run)]);
        assert_eq!(got[0].scope.spec(), Some("001-a"));
    }

    #[test]
    fn a_contract_move_proposes_the_new_work_and_notices_only_a_move_away_from_a_digest() {
        let spec = "001-a".to_string();
        let now = with(&[("001-a", digest("c2"))]);
        let moved = |before, after| Transition::ContractMoved {
            spec: spec.clone(),
            before,
            after,
        };
        assert_eq!(
            subs(&matches(&[moved(digest("c1"), digest("c2"))], &now)),
            [
                ("eligible-work", Kind::Run),
                ("moved-contract", Kind::Notice)
            ]
        );
        assert_eq!(
            subs(&matches(&[moved(unavailable(), digest("c2"))], &now)),
            [("eligible-work", Kind::Run)]
        );
        assert_eq!(
            subs(&matches(&[moved(digest("c1"), unavailable())], &now)),
            [("moved-contract", Kind::Notice)]
        );
    }

    #[test]
    fn withdrawal_and_refusal_are_notices_and_the_rest_propose_nothing() {
        let now = Projections::refused("stale");
        let t = [
            Transition::CorpusChanged {
                before: CorpusState::Answered,
                after: now.corpus_state.clone(),
            },
            Transition::Exited {
                spec: "001-a".to_string(),
                now: None,
            },
            Transition::ReasonChanged {
                spec: "002-b".to_string(),
                before: "x".to_string(),
                after: "y".to_string(),
            },
            Transition::CorpusChanged {
                before: now.corpus_state.clone(),
                after: CorpusState::Answered,
            },
        ];
        let got = matches(&t, &now);
        assert_eq!(
            subs(&got),
            [
                ("corpus-refusal", Kind::Notice),
                ("withdrawn-work", Kind::Notice)
            ]
        );
        assert_eq!(got[0].scope, Scope::Repository);
    }

    #[test]
    fn identity_is_the_cause_not_the_moment_and_scopes_do_not_collide() {
        let a = run("001-a", "c");
        assert_eq!(a.id(), run("001-a", "c").id());
        assert_ne!(a.id(), run("001-a", "d").id());
        assert_ne!(a.id(), run("001-b", "c").id());
        let repo = notice(
            Subscription::CorpusRefusal,
            Scope::Repository,
            json!(1),
            json!(2),
        );
        let named = notice(
            Subscription::CorpusRefusal,
            Scope::Spec {
                id: "repository".to_string(),
            },
            json!(1),
            json!(2),
        );
        assert_ne!(repo.id(), named.id());
    }
}
