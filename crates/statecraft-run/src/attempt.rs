//! Attempts and the closed outcome set.
//!
//! Spec 003 section 3.4. Five outcomes, none a synonym for another, none
//! widenable without an amendment. A **retry appends a new attempt** with its
//! own number, base revision and outcome; it never edits, deletes or supersedes
//! an earlier attempt's records. History is the point of the record.

use serde::{Deserialize, Serialize};

/// How an attempt ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Ran to its own end. **Says nothing about acceptance**, which is 005.
    Completed,
    /// Ran and its work did not hold: a check failed, or the adapter reported a
    /// failure result.
    Failed,
    /// A guard refused. Fails the attempt **even when the underlying command
    /// exits zero**.
    Refused,
    /// Stopped without reaching an outcome: a crash, a kill, a lost supervisor,
    /// a base revision that moved. Distinct from `failed`, because nothing was
    /// judged.
    Interrupted,
    /// An operator stopped it deliberately. Distinct from `interrupted`, because
    /// the reason is recorded and is not a fault.
    Cancelled,
}

impl Outcome {
    /// The word this outcome is recorded as.
    pub fn word(self) -> &'static str {
        match self {
            Outcome::Completed => "completed",
            Outcome::Failed => "failed",
            Outcome::Refused => "refused",
            Outcome::Interrupted => "interrupted",
            Outcome::Cancelled => "cancelled",
        }
    }

    /// Whether anything was judged at all.
    ///
    /// `interrupted` is the one that was not, which is why it is not `failed`.
    pub fn was_judged(self) -> bool {
        !matches!(self, Outcome::Interrupted)
    }

    /// Every outcome. Used by a test that asserts the set stays closed.
    pub fn all() -> [Outcome; 5] {
        [
            Outcome::Completed,
            Outcome::Failed,
            Outcome::Refused,
            Outcome::Interrupted,
            Outcome::Cancelled,
        ]
    }
}

/// One attempt at a unit of work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attempt {
    /// The run this attempt belongs to.
    pub run_id: String,
    /// Its number within the run, starting at 1.
    pub number: u32,
    /// The base revision this attempt resolved at its start.
    pub base_commit: String,
    /// Its outcome, absent while it is live.
    pub outcome: Option<Outcome>,
    /// Set when an operator override admitted the work this attempt does.
    pub admitted_by_override: Option<crate::policy::Override>,
    /// Whether the lifecycle policy was declared by the target or defaulted.
    pub policy_was_declared: bool,
    /// How the intent says the spec was admitted, or `None` when the intent
    /// predates spec 003 section 3.1.4 and so records nothing (not recorded).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admission: Option<crate::work::Admission>,
}

impl Attempt {
    /// A live attempt.
    pub fn start(run_id: &str, number: u32, base_commit: &str) -> Self {
        Self {
            run_id: run_id.to_string(),
            number,
            base_commit: base_commit.to_string(),
            outcome: None,
            admitted_by_override: None,
            policy_was_declared: false,
            admission: None,
        }
    }

    /// Whether this attempt is still running.
    pub fn live(&self) -> bool {
        self.outcome.is_none()
    }
}

/// Every attempt at one unit of work, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    /// The run id.
    pub id: String,
    /// The attempts, append-only.
    pub attempts: Vec<Attempt>,
}

impl Run {
    /// A run with no attempts yet.
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            attempts: Vec::new(),
        }
    }

    /// The live attempt, if one is running.
    pub fn live_attempt(&self) -> Option<&Attempt> {
        self.attempts.iter().find(|a| a.live())
    }

    /// Append a new attempt.
    ///
    /// The only way to add one. There is deliberately no method that mutates an
    /// earlier attempt's outcome: a retry is an append, and an outcome is
    /// written once.
    pub fn append_attempt(&mut self, base_commit: &str) -> &Attempt {
        let number = self.attempts.len() as u32 + 1;
        self.attempts
            .push(Attempt::start(&self.id, number, base_commit));
        self.attempts.last().expect("just pushed")
    }

    /// Record the outcome of the live attempt.
    ///
    /// Returns `false` when there is no live attempt to conclude, rather than
    /// reaching back and rewriting a concluded one.
    pub fn conclude(&mut self, outcome: Outcome) -> bool {
        match self.attempts.iter_mut().find(|a| a.live()) {
            Some(a) => {
                a.outcome = Some(outcome);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_outcome_set_has_exactly_five_members() {
        assert_eq!(Outcome::all().len(), 5);
    }

    #[test]
    fn interrupted_is_the_one_where_nothing_was_judged() {
        for o in Outcome::all() {
            assert_eq!(o.was_judged(), o != Outcome::Interrupted);
        }
    }

    #[test]
    fn a_retry_appends_and_leaves_the_earlier_attempt_readable() {
        let mut run = Run::new("r1");
        run.append_attempt("aaa");
        run.conclude(Outcome::Failed);
        run.append_attempt("bbb");

        assert_eq!(run.attempts.len(), 2);
        assert_eq!(run.attempts[0].outcome, Some(Outcome::Failed));
        assert_eq!(run.attempts[0].base_commit, "aaa");
        assert_eq!(run.attempts[1].number, 2);
        assert_eq!(run.attempts[1].base_commit, "bbb");
    }

    #[test]
    fn a_retry_after_a_completed_attempt_leaves_it_unchanged() {
        let mut run = Run::new("r1");
        run.append_attempt("aaa");
        run.conclude(Outcome::Completed);
        let before = run.attempts[0].clone();
        run.append_attempt("aaa");
        assert_eq!(run.attempts[0], before);
        assert_eq!(run.attempts.len(), 2);
    }

    #[test]
    fn concluding_with_no_live_attempt_does_not_rewrite_history() {
        let mut run = Run::new("r1");
        run.append_attempt("aaa");
        assert!(run.conclude(Outcome::Completed));
        assert!(!run.conclude(Outcome::Failed));
        assert_eq!(run.attempts[0].outcome, Some(Outcome::Completed));
    }

    #[test]
    fn only_one_attempt_is_live_at_a_time() {
        let mut run = Run::new("r1");
        run.append_attempt("aaa");
        assert!(run.live_attempt().is_some());
        run.conclude(Outcome::Cancelled);
        assert!(run.live_attempt().is_none());
    }
}
