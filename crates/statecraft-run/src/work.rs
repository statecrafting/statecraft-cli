//! Work selection: what the report offers, filtered by what the repository agreed to.
//!
//! Spec 003 sections 3.1 and 3.1.1. A unit of work is one spec in a registered
//! repository's corpus that spec-spine's own report names as ready **and** that
//! the repository's lifecycle policy admits.
//!
//! A ready spec the policy excludes is **listed with the reason**, never
//! silently dropped and never scheduled. That is the whole difference between a
//! tool that schedules what it is offered and a tool that schedules what its
//! owner agreed to.

use crate::policy::{Overrides, Policy, PolicySource};
use crate::report::CorpusReport;
use serde::{Deserialize, Serialize};

/// A unit of work, and how it came to be eligible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkItem {
    /// The spec id.
    pub id: String,
    /// Its title, as the report gave it.
    pub title: String,
    /// The spec's status at selection time.
    pub status: String,
    /// The report field this row came from, which `work list` prints.
    pub from_field: String,
    /// The report fields its `status` came from (section 3.1.2 rule 5).
    #[serde(default)]
    pub status_from: String,
    /// Present when only an operator override made this eligible.
    ///
    /// Surfaced on **every** attempt it admits: an override that stopped being
    /// visible after the first run would be indistinguishable from a policy.
    pub admitted_by_override: Option<crate::policy::Override>,
}

/// A ready spec the policy did not admit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Excluded {
    /// The spec id.
    pub id: String,
    /// Its status.
    pub status: String,
    /// Why it was excluded, in words an operator can act on.
    pub reason: String,
}

/// What `work list` produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkList {
    /// Eligible units, in the order the report offered them.
    pub eligible: Vec<WorkItem>,
    /// Ready specs the policy excluded, each with its reason.
    pub excluded: Vec<Excluded>,
    /// The policy that decided, recorded so an attempt can say whether it was
    /// declared or defaulted.
    pub policy: Policy,
    /// A disagreement between the target's declaration and what this product
    /// holds, if there was one. The declaration still won.
    pub policy_disagreement: Option<String>,
    /// The spec-spine version whose reports this was computed from.
    pub spec_spine_version: String,
}

impl WorkList {
    /// Whether the policy was assumed rather than declared by the target.
    pub fn policy_was_defaulted(&self) -> bool {
        self.policy.source == PolicySource::Defaulted
    }
}

/// What `work show` found for one spec id.
///
/// Added by spec 006's additive edge. Spec 006 section 3.11 forbids the CLI crate
/// from deriving an answer an owning crate could have returned, and "is this
/// spec eligible, and if not why" is exactly such an answer: [`WorkList`]
/// already holds both halves, so the lookup belongs beside them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "eligibility")]
pub enum Eligibility {
    /// The policy admits it, and here is the row.
    Eligible(WorkItem),
    /// It was offered as ready and the policy did not admit it.
    ///
    /// A finding rather than a refusal: the reason **is** the answer.
    Excluded(Excluded),
    /// spec-spine did not offer it as ready at all.
    ///
    /// Distinct from excluded, because nothing about this spec was judged: it
    /// was never in the ready set to judge.
    NotOffered {
        /// The id that was asked about.
        id: String,
    },
}

impl Eligibility {
    /// Whether work may be scheduled for this spec.
    pub fn schedulable(&self) -> bool {
        matches!(self, Eligibility::Eligible(_))
    }

    /// A one-line rendering naming the report field each answer came from.
    pub fn describe(&self) -> String {
        match self {
            Eligibility::Eligible(item) => format!(
                "{} eligible, status {} (from {}; status from {})",
                item.id, item.status, item.from_field, item.status_from
            ),
            Eligibility::Excluded(e) => {
                format!("{} excluded, status {}: {}", e.id, e.status, e.reason)
            }
            Eligibility::NotOffered { id } => format!(
                "{id} is not in `registry plan --json: ready[]`, so nothing about it was judged"
            ),
        }
    }
}

impl WorkList {
    /// What this list says about one spec id.
    pub fn eligibility_of(&self, id: &str) -> Eligibility {
        if let Some(item) = self.eligible.iter().find(|i| i.id == id) {
            return Eligibility::Eligible(item.clone());
        }
        if let Some(e) = self.excluded.iter().find(|e| e.id == id) {
            return Eligibility::Excluded(e.clone());
        }
        Eligibility::NotOffered { id: id.to_string() }
    }
}

/// Select work from a report under a policy.
///
/// Every ready spec lands in exactly one of `eligible` or `excluded`, which is
/// what makes "never silently dropped" a property of the type rather than a
/// promise in prose.
pub fn select(
    report: &CorpusReport,
    policy: &Policy,
    overrides: &Overrides,
    disagreement: Option<String>,
) -> WorkList {
    let mut eligible = Vec::new();
    let mut excluded = Vec::new();

    for ready in &report.ready {
        let Some(lifecycle) = report.lifecycle_of(&ready.id) else {
            excluded.push(Excluded {
                id: ready.id.clone(),
                status: "unknown".into(),
                reason: format!(
                    "`registry plan` offered {} but `registry list` does not carry it, \
                     so its status is unknown; refusing to schedule an unknown status",
                    ready.id
                ),
            });
            continue;
        };

        if policy.admits(&lifecycle.status) {
            eligible.push(WorkItem {
                id: ready.id.clone(),
                title: ready.title.clone(),
                status: lifecycle.status.clone(),
                from_field: "registry plan --json: ready[]".into(),
                status_from: report.status_source.describe().into(),
                admitted_by_override: None,
            });
            continue;
        }

        // Not admitted by the policy. An override can still admit it, but only
        // by naming this exact spec, and the attempt will say so.
        match overrides.for_spec(&ready.id) {
            Some(o) => eligible.push(WorkItem {
                id: ready.id.clone(),
                title: ready.title.clone(),
                status: lifecycle.status.clone(),
                from_field: "registry plan --json: ready[]".into(),
                status_from: report.status_source.describe().into(),
                admitted_by_override: Some(o.clone()),
            }),
            None => excluded.push(Excluded {
                id: ready.id.clone(),
                status: lifecycle.status.clone(),
                reason: format!(
                    "status `{}` is not scheduled by this repository's policy ({}); \
                     spec-spine offers it as ready because `draft` plus `pending` is \
                     schedulable, which withholds ratification, not schedulability",
                    lifecycle.status,
                    policy.schedulable_statuses.join(", ")
                ),
            }),
        }
    }

    WorkList {
        eligible,
        excluded,
        policy: policy.clone(),
        policy_disagreement: disagreement,
        spec_spine_version: report.spec_spine_version.clone(),
    }
}

#[cfg(test)]
mod eligibility_tests {
    use super::*;
    use crate::report::{CorpusReport, ReadySpec, SpecLifecycle};

    fn report() -> CorpusReport {
        CorpusReport {
            spec_spine_version: "0.20.0".into(),
            ready: vec![
                ReadySpec {
                    id: "010".into(),
                    title: "approved".into(),
                    status: None,
                },
                ReadySpec {
                    id: "011".into(),
                    title: "draft".into(),
                    status: None,
                },
            ],
            lifecycle: vec![
                SpecLifecycle {
                    id: "010".into(),
                    status: "approved".into(),
                    implementation: Some("pending".into()),
                    obligations: vec![],
                },
                SpecLifecycle {
                    id: "011".into(),
                    status: "draft".into(),
                    implementation: Some("pending".into()),
                    obligations: vec![],
                },
            ],
            status_source: crate::report::StatusSource::ListOnly,
        }
    }

    fn list() -> WorkList {
        select(
            &report(),
            &Policy::default_policy(),
            &Overrides::none(),
            None,
        )
    }

    #[test]
    fn an_admitted_spec_is_eligible_and_names_the_report_field() {
        let e = list().eligibility_of("010");
        assert!(e.schedulable());
        assert!(e.describe().contains("registry plan --json: ready[]"));
    }

    #[test]
    fn a_draft_the_policy_excluded_is_a_reason_and_not_a_refusal() {
        let e = list().eligibility_of("011");
        assert!(!e.schedulable());
        match &e {
            Eligibility::Excluded(x) => assert_eq!(x.status, "draft"),
            other => panic!("expected excluded, got {other:?}"),
        }
        assert!(e.describe().contains("excluded"));
    }

    #[test]
    fn a_spec_the_ready_set_never_offered_is_distinct_from_one_it_excluded() {
        let e = list().eligibility_of("999");
        assert!(!e.schedulable());
        assert!(matches!(e, Eligibility::NotOffered { .. }));
        assert!(e.describe().contains("nothing about it was judged"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{ReadySpec, SpecLifecycle};

    fn report(rows: &[(&str, &str)]) -> CorpusReport {
        CorpusReport {
            spec_spine_version: "spec-spine 0.18.0".into(),
            ready: rows
                .iter()
                .map(|(id, _)| ReadySpec {
                    id: (*id).into(),
                    title: format!("title of {id}"),
                    status: None,
                })
                .collect(),
            lifecycle: rows
                .iter()
                .map(|(id, status)| SpecLifecycle {
                    id: (*id).into(),
                    status: (*status).into(),
                    implementation: Some("pending".into()),
                    obligations: vec![],
                })
                .collect(),
            status_source: crate::report::StatusSource::ListOnly,
        }
    }

    #[test]
    fn an_approved_ready_spec_is_eligible_and_names_the_field_it_came_from() {
        let list = select(
            &report(&[("003-x", "approved")]),
            &Policy::default_policy(),
            &Overrides::none(),
            None,
        );
        assert_eq!(list.eligible.len(), 1);
        assert!(list.eligible[0].from_field.contains("registry plan"));
        assert!(list.excluded.is_empty());
    }

    #[test]
    fn a_draft_ready_spec_is_excluded_with_a_reason_never_dropped() {
        let list = select(
            &report(&[("006-x", "draft")]),
            &Policy::default_policy(),
            &Overrides::none(),
            None,
        );
        assert!(list.eligible.is_empty());
        assert_eq!(list.excluded.len(), 1);
        assert_eq!(list.excluded[0].status, "draft");
        assert!(list.excluded[0].reason.contains("not scheduled"));
    }

    #[test]
    fn an_override_admits_exactly_the_named_draft_and_is_surfaced() {
        let list = select(
            &report(&[("006-x", "draft"), ("007-y", "draft")]),
            &Policy::default_policy(),
            &Overrides::none().admitting("006-x", "bart", "needed for the demo"),
            None,
        );
        assert_eq!(list.eligible.len(), 1);
        assert_eq!(list.eligible[0].id, "006-x");
        let o = list.eligible[0]
            .admitted_by_override
            .as_ref()
            .expect("the override is surfaced on the item it admits");
        assert_eq!(o.operator, "bart");
        assert_eq!(list.excluded.len(), 1, "the other draft stays excluded");
    }

    #[test]
    fn every_ready_spec_lands_in_exactly_one_list() {
        let list = select(
            &report(&[("a", "approved"), ("b", "draft"), ("c", "retired")]),
            &Policy::default_policy(),
            &Overrides::none(),
            None,
        );
        assert_eq!(list.eligible.len() + list.excluded.len(), 3);
    }

    #[test]
    fn a_ready_spec_absent_from_the_lifecycle_report_is_excluded_not_assumed() {
        let mut r = report(&[("003-x", "approved")]);
        r.lifecycle.clear();
        let list = select(&r, &Policy::default_policy(), &Overrides::none(), None);
        assert!(list.eligible.is_empty());
        assert!(list.excluded[0].reason.contains("status is unknown"));
    }

    #[test]
    fn a_defaulted_policy_is_recorded_as_defaulted() {
        let list = select(
            &report(&[("a", "approved")]),
            &Policy::default_policy(),
            &Overrides::none(),
            None,
        );
        assert!(list.policy_was_defaulted());
    }
}
