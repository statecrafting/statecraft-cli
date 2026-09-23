//! What is judged, and when there is nothing to judge.
//!
//! Spec 005 sections 3.1 and 3.1.1. Acceptance is evaluated over three
//! identified things, all named in the record: the candidate, the trusted base,
//! and the policy as it exists at that base.
//!
//! An acceptance whose candidate, base or policy cannot be identified is **not a
//! failing acceptance. It is no acceptance**, recorded as such. The distinction
//! is the module's reason to exist: a reader must never have to infer from a
//! missing receipt whether the suite ran and failed, or never ran at all.

use serde::{Deserialize, Serialize};
use statecraft_run::attempt::Outcome;

/// The candidate under judgement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    /// A commit sha in the prepared workspace.
    pub sha: String,
    /// Whether the work tree was clean when the suite ended.
    pub work_tree_clean: bool,
    /// Whether HEAD stayed put for the whole suite.
    pub head_stable: bool,
    /// The paths that were dirty, named when the tree was not clean.
    #[serde(default)]
    pub dirty_paths: Vec<String>,
}

/// The trusted base the authority set is read from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Base {
    /// A commit resolved at run start.
    pub sha: String,
}

/// The policy, identified by a digest over its bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    /// SHA-256 over the authority set's bytes as they exist at the base.
    pub digest: String,
}

/// The three identified things, when all three could be identified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Judged {
    /// The candidate.
    pub candidate: Candidate,
    /// The base.
    pub base: Base,
    /// The policy at that base.
    pub policy: Policy,
}

/// Why there is no acceptance at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "reason")]
pub enum NoAcceptance {
    /// The candidate could not be identified.
    CandidateUnidentified {
        /// What went wrong.
        detail: String,
    },
    /// The base could not be identified.
    BaseUnidentified {
        /// What went wrong.
        detail: String,
    },
    /// The policy digest could not be computed at the base.
    PolicyDigestUncomputable {
        /// What went wrong.
        detail: String,
    },
    /// The suite never ran.
    ///
    /// Not a pass and not a fail. The count of checks that did not run is part
    /// of the outcome.
    SuiteDidNotRun {
        /// How many checks never ran.
        unrun_checks: u32,
    },
    /// The contract the attempt was bound to is not the one the producer
    /// resolves now (section 3.18): it changed, lost a member, or withdrew an
    /// obligation. The candidate was built against a different contract.
    ContractMoved {
        /// The comparison, naming every member involved.
        comparison: Box<crate::contract::Comparison>,
    },
}

/// Why acceptance was not attempted for this attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotAttemptedReason {
    /// The attempt failed.
    AttemptFailed,
    /// A guard refused.
    AttemptRefused,
    /// The attempt was interrupted, so nothing identifies stable candidate bytes.
    AttemptInterrupted,
    /// An operator cancelled it.
    AttemptCancelled,
}

impl NotAttemptedReason {
    /// The reason as it is written.
    pub fn word(self) -> &'static str {
        match self {
            NotAttemptedReason::AttemptFailed => "attempt-failed",
            NotAttemptedReason::AttemptRefused => "attempt-refused",
            NotAttemptedReason::AttemptInterrupted => "attempt-interrupted",
            NotAttemptedReason::AttemptCancelled => "attempt-cancelled",
        }
    }
}

/// Whether acceptance is even attempted for an attempt's outcome.
///
/// Exactly one of spec 003's five outcomes is eligible: `completed`. It says
/// nothing about acceptance on its own, which is why acceptance is a separate
/// judgement; it is simply the only outcome that left a candidate to judge.
///
/// For the other four this returns the reason, which is **recorded rather than
/// left blank**.
pub fn eligibility(outcome: Outcome) -> Result<(), NotAttemptedReason> {
    match outcome {
        Outcome::Completed => Ok(()),
        Outcome::Failed => Err(NotAttemptedReason::AttemptFailed),
        Outcome::Refused => Err(NotAttemptedReason::AttemptRefused),
        Outcome::Interrupted => Err(NotAttemptedReason::AttemptInterrupted),
        Outcome::Cancelled => Err(NotAttemptedReason::AttemptCancelled),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_completed_is_eligible_for_acceptance() {
        assert!(eligibility(Outcome::Completed).is_ok());
        for o in [
            Outcome::Failed,
            Outcome::Refused,
            Outcome::Interrupted,
            Outcome::Cancelled,
        ] {
            assert!(eligibility(o).is_err(), "{o:?} must not be attempted");
        }
    }

    #[test]
    fn each_ineligible_outcome_has_its_own_named_reason() {
        let mut words: Vec<&str> = [
            Outcome::Failed,
            Outcome::Refused,
            Outcome::Interrupted,
            Outcome::Cancelled,
        ]
        .iter()
        .map(|o| eligibility(*o).unwrap_err().word())
        .collect();
        let before = words.len();
        words.sort_unstable();
        words.dedup();
        assert_eq!(words.len(), before, "no two outcomes share a reason");
    }

    #[test]
    fn a_suite_that_never_ran_is_no_acceptance_and_counts_the_unrun_checks() {
        let n = NoAcceptance::SuiteDidNotRun { unrun_checks: 4 };
        let json = serde_json::to_string(&n).unwrap();
        assert!(json.contains("suite-did-not-run"));
        assert!(json.contains('4'));
    }
}
