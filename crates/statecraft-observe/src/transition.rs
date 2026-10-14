//! Transitions between two observations' projections (spec 027 sections 3.2
//! and 3.3).
//!
//! `work-eligibility` and `contract-identity` compare the current answered
//! observation with the last answered one before it, so a refusal between two
//! answers is spanned rather than read as every spec leaving and re-entering.
//! `corpus-state` compares the current observation with the one immediately
//! before it, answered or refused.

use crate::projection::{Contract, CorpusState, Eligibility, Projections};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// One transition, as section 3.3 names them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Transition {
    /// Absent or `excluded` before, `eligible` now. `initial` marks the first
    /// answered observation of a repository, which has no comparison base.
    Entered {
        /// The spec.
        spec: String,
        /// Whether this is the first answered observation's report.
        initial: bool,
    },
    /// `eligible` before, absent or `excluded` now.
    Exited {
        /// The spec.
        spec: String,
        /// The value now: `excluded` with spec 003's reason, or `None` when
        /// the plan report no longer names the spec as ready.
        now: Option<Eligibility>,
    },
    /// `excluded` in both, with different reasons.
    ReasonChanged {
        /// The spec.
        spec: String,
        /// The earlier reason.
        before: String,
        /// The current reason.
        after: String,
    },
    /// `eligible` in both, with a different `contract-identity`.
    ContractMoved {
        /// The spec.
        spec: String,
        /// The earlier value.
        before: Contract,
        /// The current value.
        after: Contract,
    },
    /// The `corpus-state` value differs from the previous observation's.
    CorpusChanged {
        /// The earlier value.
        before: CorpusState,
        /// The current value.
        after: CorpusState,
    },
}

impl Transition {
    /// The projection this transition is over.
    pub fn projection(&self) -> &'static str {
        match self {
            Transition::Entered { .. }
            | Transition::Exited { .. }
            | Transition::ReasonChanged { .. } => "work-eligibility",
            Transition::ContractMoved { .. } => "contract-identity",
            Transition::CorpusChanged { .. } => "corpus-state",
        }
    }

    /// The spec it is keyed by; `corpus-state` is keyed by nothing.
    pub fn spec(&self) -> Option<&str> {
        match self {
            Transition::Entered { spec, .. }
            | Transition::Exited { spec, .. }
            | Transition::ReasonChanged { spec, .. }
            | Transition::ContractMoved { spec, .. } => Some(spec),
            Transition::CorpusChanged { .. } => None,
        }
    }

    /// The kind, as section 3.3 spells it.
    pub fn kind(&self) -> &'static str {
        match self {
            Transition::Entered { .. } => "entered",
            Transition::Exited { .. } => "exited",
            Transition::ReasonChanged { .. } => "reason-changed",
            Transition::ContractMoved { .. } => "contract-moved",
            Transition::CorpusChanged { .. } => "corpus-changed",
        }
    }
}

/// Every transition of the current observation, sorted by projection, then
/// spec id, then kind, so one pair of observations always yields one
/// sequence.
///
/// `previous` is the observation immediately before this one, answered or
/// refused; `base` is the last answered observation before this one. Either
/// is `None` when the chain has none.
pub fn compute(
    previous: Option<&Projections>,
    base: Option<&Projections>,
    current: &Projections,
) -> Vec<Transition> {
    let mut out = Vec::new();
    // A repository's first observation has no predecessor, so its corpus
    // state is no transition, whatever it is (section 3.2).
    if let Some(previous) = previous
        && previous.corpus_state != current.corpus_state
    {
        out.push(Transition::CorpusChanged {
            before: previous.corpus_state.clone(),
            after: current.corpus_state.clone(),
        });
    }
    if current.corpus_state.answered() {
        match base {
            None => {
                for spec in current.eligible() {
                    out.push(Transition::Entered {
                        spec: spec.to_string(),
                        initial: true,
                    });
                }
            }
            Some(base) => compare(base, current, &mut out),
        }
    }
    out.sort_by(|a, b| {
        (a.projection(), a.spec().unwrap_or(""), a.kind()).cmp(&(
            b.projection(),
            b.spec().unwrap_or(""),
            b.kind(),
        ))
    });
    out
}

fn compare(base: &Projections, current: &Projections, out: &mut Vec<Transition>) {
    let specs: BTreeSet<&String> = base
        .work_eligibility
        .keys()
        .chain(current.work_eligibility.keys())
        .collect();
    for spec in specs {
        let before = base.work_eligibility.get(spec);
        let after = current.work_eligibility.get(spec);
        match (before, after) {
            (Some(Eligibility::Eligible), Some(Eligibility::Eligible)) => {
                if let (Some(b), Some(a)) = (
                    base.contract_identity.get(spec),
                    current.contract_identity.get(spec),
                ) && b.moved_to(a)
                {
                    out.push(Transition::ContractMoved {
                        spec: spec.clone(),
                        before: b.clone(),
                        after: a.clone(),
                    });
                }
            }
            (Some(Eligibility::Eligible), now) => out.push(Transition::Exited {
                spec: spec.clone(),
                now: now.cloned(),
            }),
            (_, Some(Eligibility::Eligible)) => out.push(Transition::Entered {
                spec: spec.clone(),
                initial: false,
            }),
            (
                Some(Eligibility::Excluded { reason: b }),
                Some(Eligibility::Excluded { reason: a }),
            ) if a != b => out.push(Transition::ReasonChanged {
                spec: spec.clone(),
                before: b.clone(),
                after: a.clone(),
            }),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn answered(rows: &[(&str, Option<&str>, Option<&str>)]) -> Projections {
        // (spec, excluded reason or None for eligible, contract digest or None
        // for unavailable)
        let work = rows
            .iter()
            .map(|(id, excluded, _)| {
                (
                    id.to_string(),
                    match excluded {
                        None => Eligibility::Eligible,
                        Some(r) => Eligibility::Excluded {
                            reason: r.to_string(),
                        },
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        Projections::answered(work, |id| {
            match rows.iter().find(|r| r.0 == id).and_then(|r| r.2) {
                Some(d) => Contract::Digest {
                    digest: d.to_string(),
                },
                None => Contract::Unavailable {
                    reason: "no closure".to_string(),
                },
            }
        })
    }

    #[test]
    fn the_first_observation_reports_every_eligible_row_as_initial() {
        let now = answered(&[("002-b", None, Some("c")), ("001-a", Some("draft"), None)]);
        assert_eq!(
            compute(None, None, &now),
            [Transition::Entered {
                spec: "002-b".to_string(),
                initial: true
            }]
        );
    }

    #[test]
    fn a_first_refused_observation_is_no_transition() {
        assert!(compute(None, None, &Projections::refused("stale")).is_empty());
    }

    #[test]
    fn a_refusal_between_two_answers_is_spanned() {
        let first = answered(&[("001-a", None, Some("c"))]);
        let refused = Projections::refused("stale");
        // Into the refusal: corpus-state only.
        assert_eq!(
            compute(Some(&first), Some(&first), &refused),
            [Transition::CorpusChanged {
                before: CorpusState::Answered,
                after: refused.corpus_state.clone()
            }]
        );
        // Out of it: corpus-state back, and nothing about 001-a, which was
        // eligible under the same contract on both sides of the refusal.
        assert_eq!(
            compute(Some(&refused), Some(&first), &first),
            [Transition::CorpusChanged {
                before: refused.corpus_state.clone(),
                after: CorpusState::Answered
            }]
        );
    }

    #[test]
    fn each_eligibility_and_contract_change_is_named_and_sorted() {
        let before = answered(&[
            ("001-a", None, Some("c1")),
            ("002-b", None, Some("c2")),
            ("003-c", Some("draft"), None),
            ("004-d", Some("draft"), None),
            ("005-e", None, Some("c5")),
            ("006-f", None, None),
        ]);
        let after = answered(&[
            ("001-a", None, Some("c1")),
            ("002-b", Some("blocked"), None),
            ("003-c", None, Some("c3")),
            ("004-d", Some("superseded"), None),
            ("005-e", None, None),
            ("006-f", None, None),
            ("007-g", None, Some("c7")),
        ]);
        let got = compute(Some(&before), Some(&before), &after);
        let kinds: Vec<_> = got
            .iter()
            .map(|t| (t.projection(), t.spec().unwrap(), t.kind()))
            .collect();
        assert_eq!(
            kinds,
            [
                ("contract-identity", "005-e", "contract-moved"),
                ("work-eligibility", "002-b", "exited"),
                ("work-eligibility", "003-c", "entered"),
                ("work-eligibility", "004-d", "reason-changed"),
                ("work-eligibility", "007-g", "entered"),
            ]
        );
        assert!(matches!(
            &got[1],
            Transition::Exited { now: Some(Eligibility::Excluded { reason }), .. } if reason == "blocked"
        ));
        assert!(matches!(
            &got[2],
            Transition::Entered { initial: false, .. }
        ));
    }

    #[test]
    fn a_spec_no_longer_offered_exits_with_no_value() {
        let before = answered(&[("001-a", None, Some("c"))]);
        let after = answered(&[]);
        assert_eq!(
            compute(Some(&before), Some(&before), &after),
            [Transition::Exited {
                spec: "001-a".to_string(),
                now: None
            }]
        );
    }
}
