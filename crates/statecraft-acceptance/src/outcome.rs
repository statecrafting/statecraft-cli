//! The reviewable outcome: one read-only account per run, folded from records.
//!
//! Spec 005 section 3.9. Folded **from the records and nothing else**, and every
//! value names the record it came from. It shows what was requested beside what
//! was applied, the claim beside the independent result, each dimension
//! separately, the receipt or its absence by name, and every refusal.
//!
//! **Publication is not part of this spec.** An accepted candidate with a
//! receipt is a reviewable outcome; whether anything is published from it is a
//! later decision, and no verb in this corpus publishes. There is deliberately
//! no method here that does anything but describe.

use crate::absence::{Absence, Recorded, Statement};
use crate::authority::Verdict as AuthorityVerdict;
use crate::dimensions::{Admission, Dimensions};
use crate::judged::{NoAcceptance, NotAttemptedReason};
use crate::receipt::{NoReceipt, Receipt};
use serde::{Deserialize, Serialize};

/// What acceptance concluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "acceptance")]
pub enum Acceptance {
    /// The suite ran over an identified candidate and passed.
    Accepted {
        /// The receipt.
        receipt: Box<Receipt>,
    },
    /// The suite ran and did not pass.
    Failed {
        /// Why no receipt was minted.
        reason: NoReceipt,
    },
    /// Acceptance was not attempted, because the attempt was not eligible.
    ///
    /// A fourth thing, distinct from an acceptance that failed and from the
    /// three names for absence.
    NotAttempted {
        /// Which outcome made it ineligible.
        reason: NotAttemptedReason,
        /// The refusal count, for an attempt that was `refused`.
        #[serde(skip_serializing_if = "Option::is_none")]
        refusal_count: Option<u32>,
    },
    /// There was nothing to accept, because something could not be identified.
    None {
        /// Why.
        reason: NoAcceptance,
    },
}

impl Acceptance {
    /// The word this is reported as.
    pub fn word(&self) -> &'static str {
        match self {
            Acceptance::Accepted { .. } => "accepted",
            Acceptance::Failed { .. } => "failed",
            Acceptance::NotAttempted { .. } => "not-attempted",
            Acceptance::None { .. } => "no-acceptance",
        }
    }

    /// Whether a receipt exists.
    pub fn receipt(&self) -> Option<&Receipt> {
        match self {
            Acceptance::Accepted { receipt } => Some(receipt),
            _ => None,
        }
    }
}

/// One value in the account, with the record it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sourced<T> {
    /// The value.
    pub value: T,
    /// Which record it was folded from.
    pub from_record: String,
}

impl<T> Sourced<T> {
    /// A value and its record.
    pub fn new(value: T, from_record: &str) -> Self {
        Self {
            value,
            from_record: from_record.to_string(),
        }
    }
}

/// The reviewable account of one run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewableOutcome {
    /// Which run.
    pub run_id: String,
    /// What the agent claimed, kept apart from what was observed.
    pub claim: Recorded<Statement>,
    /// What an independent run of the suite found.
    pub independent_result: Sourced<String>,
    /// What was requested of the adapter.
    pub requested: Sourced<Vec<String>>,
    /// What it actually applied.
    pub applied: Sourced<Vec<String>>,
    /// The four dimensions, separately.
    pub dimensions: Sourced<Dimensions>,
    /// Admission, kept apart from the dimensions.
    pub admission: Sourced<Admission>,
    /// The authority-set verdict.
    pub authority: Sourced<AuthorityVerdict>,
    /// Acceptance, including the receipt or its absence by name.
    pub acceptance: Sourced<Acceptance>,
    /// Whether the receipt, if any, is stale.
    pub receipt_freshness: Recorded<String>,
    /// Every refusal recorded during the run.
    pub refusals: Sourced<Vec<String>>,
}

impl ReviewableOutcome {
    /// Whether this account contains a receipt.
    pub fn has_receipt(&self) -> bool {
        self.acceptance.value.receipt().is_some()
    }

    /// A rendering for a reader.
    pub fn render(&self) -> String {
        let mut out = format!("run {}\n", self.run_id);
        out.push_str(&match &self.claim {
            Recorded::Present(Statement::Narrative { text }) => {
                format!("claim (narrative, not a result): {text}\n")
            }
            Recorded::Present(Statement::Observed { record, text }) => {
                format!("observed ({record}): {text}\n")
            }
            Recorded::Absent(a) => format!("claim: {}\n", a.word()),
        });
        out.push_str(&format!(
            "independent result ({}): {}\n",
            self.independent_result.from_record, self.independent_result.value
        ));
        out.push_str(&format!(
            "requested [{}] applied [{}]\n",
            self.requested.value.join(", "),
            self.applied.value.join(", ")
        ));
        let d = &self.dimensions.value;
        out.push_str(&format!(
            "integrity {:?} | signature {:?} | issuerTrust {:?} | subjectBinding {:?}\n",
            d.integrity, d.signature, d.issuer_trust, d.subject_binding
        ));
        out.push_str(&format!("admission {:?}\n", self.admission.value));
        out.push_str(&format!("authority: {}\n", self.authority.value.note));
        out.push_str(&format!("acceptance {}\n", self.acceptance.value.word()));
        out.push_str(&match &self.receipt_freshness {
            Recorded::Present(s) => format!("receipt {s}\n"),
            Recorded::Absent(a) => format!("receipt {}\n", a.word()),
        });
        for r in &self.refusals.value {
            out.push_str(&format!("refusal: {r}\n"));
        }
        out
    }
}

/// The freshness field for an outcome.
pub fn freshness_field(receipt: Option<&Receipt>, branch_head: Option<&str>) -> Recorded<String> {
    match (receipt, branch_head) {
        (None, _) => Recorded::Absent(Absence::NotRecorded),
        (Some(_), None) => Recorded::Absent(Absence::NotRecorded),
        (Some(r), Some(head)) => match crate::receipt::freshness(r, head) {
            None => Recorded::Present("current".to_string()),
            Some(a) => Recorded::Absent(a),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dimensions::{Integrity, SubjectBinding};

    fn outcome(acceptance: Acceptance, freshness: Recorded<String>) -> ReviewableOutcome {
        ReviewableOutcome {
            run_id: "run-1".into(),
            claim: Recorded::Present(Statement::Narrative {
                text: "all done".into(),
            }),
            independent_result: Sourced::new("1 of 2 checks passed".into(), "suite-result"),
            requested: Sourced::new(vec!["turn-limit".into()], "adapter-request"),
            applied: Sourced::new(vec![], "adapter-init"),
            dimensions: Sourced::new(
                Dimensions::unsigned_today(Integrity::Pass, SubjectBinding::NotApplicable),
                "evidence-check",
            ),
            admission: Sourced::new(Admission::Admit, "admission"),
            authority: Sourced::new(
                AuthorityVerdict {
                    repository_members_touched: vec![],
                    environment_manifest_touched: false,
                    corpus_members: crate::authority::CorpusVerdict::Absent(Absence::NotRecorded),
                    corpus_classes: vec![],
                    prior_policy_required: Recorded::Absent(Absence::NotRecorded),
                    authority_change: false,
                    may_accept_on_own_suite: false,
                    note: "not-recorded".into(),
                },
                "authority",
            ),
            acceptance: Sourced::new(acceptance, "acceptance"),
            receipt_freshness: freshness,
            refusals: Sourced::new(vec!["a guard refused a write".into()], "accounting"),
        }
    }

    #[test]
    fn every_value_names_the_record_it_came_from() {
        let o = outcome(
            Acceptance::NotAttempted {
                reason: NotAttemptedReason::AttemptFailed,
                refusal_count: None,
            },
            Recorded::Absent(Absence::NotRecorded),
        );
        assert!(!o.independent_result.from_record.is_empty());
        assert!(!o.dimensions.from_record.is_empty());
        assert!(!o.admission.from_record.is_empty());
        assert!(!o.acceptance.from_record.is_empty());
    }

    #[test]
    fn the_claim_is_rendered_as_a_claim_and_never_as_a_result() {
        let o = outcome(
            Acceptance::NotAttempted {
                reason: NotAttemptedReason::AttemptFailed,
                refusal_count: None,
            },
            Recorded::Absent(Absence::NotRecorded),
        );
        let text = o.render();
        assert!(text.contains("claim (narrative, not a result)"));
        assert!(text.contains("independent result"));
    }

    #[test]
    fn not_attempted_is_distinct_from_failed_and_from_no_acceptance() {
        let words: Vec<&str> = [
            Acceptance::Failed {
                reason: NoReceipt::HeadMoved,
            },
            Acceptance::NotAttempted {
                reason: NotAttemptedReason::AttemptCancelled,
                refusal_count: None,
            },
            Acceptance::None {
                reason: NoAcceptance::SuiteDidNotRun { unrun_checks: 2 },
            },
        ]
        .iter()
        .map(Acceptance::word)
        .collect();
        assert_eq!(words, ["failed", "not-attempted", "no-acceptance"]);
    }

    #[test]
    fn a_refused_attempt_carries_its_refusal_count() {
        let a = Acceptance::NotAttempted {
            reason: NotAttemptedReason::AttemptRefused,
            refusal_count: Some(3),
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains("attempt-refused"));
        assert!(json.contains('3'));
    }

    #[test]
    fn with_no_receipt_the_freshness_field_reads_not_recorded() {
        assert_eq!(
            freshness_field(None, Some("abc")),
            Recorded::Absent(Absence::NotRecorded)
        );
    }

    #[test]
    fn the_rendering_shows_requested_beside_applied() {
        let o = outcome(
            Acceptance::NotAttempted {
                reason: NotAttemptedReason::AttemptFailed,
                refusal_count: None,
            },
            Recorded::Absent(Absence::NotRecorded),
        );
        assert!(o.render().contains("requested [turn-limit] applied []"));
    }
}
