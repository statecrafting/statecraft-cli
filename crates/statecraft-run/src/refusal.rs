//! Refusal accounting, owned by the supervisor.
//!
//! Spec 003 section 3.5. The count and a bounded sample are derived by the
//! supervisor from the adapter's **structured event stream**, independently of
//! the adapter's classification of its own termination, the supervised
//! process's exit code, and anything the supervised process says about itself.
//!
//! A refusal followed by a `completed` turn is therefore evidence and not
//! silence, which is the concrete failure the archived predecessor measured
//! before its spec 129.

use crate::attempt::Outcome;
use serde::{Deserialize, Serialize};

/// How many refusals are kept as examples.
///
/// Bounded because the count is the fact and the sample is the illustration: an
/// unbounded sample turns a record into a log.
pub const SAMPLE_LIMIT: usize = 8;

/// One refusal observed on the event stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefusalEvent {
    /// What refused.
    pub guard: String,
    /// What it refused, in one line.
    pub detail: String,
}

/// The supervisor's accounting for one attempt.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accounting {
    /// Every refusal counted, not only the sampled ones.
    pub count: u32,
    /// Up to [`SAMPLE_LIMIT`] examples.
    pub sample: Vec<RefusalEvent>,
    /// Recorded when the supervised process tried to write here.
    ///
    /// Impossible by placement: the chain lives in the product home and the
    /// supervised process runs inside a worktree in the target. The attempt is
    /// still recorded, because an attempt to tamper is itself evidence.
    pub tamper_attempts: Vec<String>,
}

impl Accounting {
    /// Count a refusal observed on the stream.
    pub fn observe(&mut self, event: RefusalEvent) {
        self.count += 1;
        if self.sample.len() < SAMPLE_LIMIT {
            self.sample.push(event);
        }
    }

    /// Record that the supervised process tried to reach this accounting.
    pub fn note_tamper_attempt(&mut self, detail: &str) {
        self.tamper_attempts.push(detail.to_string());
    }

    /// Whether any refusal was counted.
    pub fn any(&self) -> bool {
        self.count > 0
    }
}

/// Decide an attempt's outcome from the supervisor's own accounting.
///
/// The rule that matters: a refusal recorded beside an otherwise completed turn
/// still makes the attempt `refused`, and the completed turn is retained beside
/// it. The adapter's own verdict and the process's exit code are arguments here,
/// not authorities: neither can turn a counted refusal into a pass.
pub fn decide(adapter_said: Outcome, accounting: &Accounting) -> Outcome {
    if accounting.any() {
        return Outcome::Refused;
    }
    adapter_said
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refusal(n: usize) -> RefusalEvent {
        RefusalEvent {
            guard: format!("guard-{n}"),
            detail: "refused".into(),
        }
    }

    #[test]
    fn a_refusal_beside_a_completed_turn_makes_the_attempt_refused() {
        let mut a = Accounting::default();
        a.observe(refusal(1));
        assert_eq!(decide(Outcome::Completed, &a), Outcome::Refused);
    }

    #[test]
    fn with_no_refusals_the_adapters_own_outcome_stands() {
        let a = Accounting::default();
        assert_eq!(decide(Outcome::Completed, &a), Outcome::Completed);
        assert_eq!(decide(Outcome::Failed, &a), Outcome::Failed);
    }

    #[test]
    fn the_sample_is_bounded_and_the_count_is_not() {
        let mut a = Accounting::default();
        for i in 0..(SAMPLE_LIMIT + 5) {
            a.observe(refusal(i));
        }
        assert_eq!(a.count as usize, SAMPLE_LIMIT + 5);
        assert_eq!(a.sample.len(), SAMPLE_LIMIT);
    }

    #[test]
    fn a_tamper_attempt_is_itself_recorded() {
        let mut a = Accounting::default();
        a.note_tamper_attempt("wrote to the product home from the worktree");
        assert_eq!(a.tamper_attempts.len(), 1);
    }
}
