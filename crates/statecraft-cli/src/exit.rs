//! The closed exit-code vocabulary.
//!
//! Spec 006 section 3.3. The same for every command, and the distinction
//! between 1, 2 and 4 is the whole point: it is what a caller scripts against.
//!
//! A `partial` apply is 1, because every withheld path was named and the
//! contract held. A removal with no manifest is 2, because it declined. An
//! unreadable manifest is 4, because nobody asked for that.

use serde::{Deserialize, Serialize};

/// What a command's exit code means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Exit {
    /// The operation did what was asked, and found nothing wrong.
    Ok,
    /// The operation ran and reports a **finding**: a diagnostic state, a
    /// withheld write, a verdict that is not `qualified`. Nothing failed.
    Finding,
    /// The operation **refused**: a precondition was not met and nothing was
    /// done.
    Refused,
    /// The arguments do not name an operation this binary has.
    Usage,
    /// Something went wrong that neither the operator nor the target asked for.
    Failed,
}

impl Exit {
    /// The process exit code.
    pub fn code(self) -> i32 {
        match self {
            Exit::Ok => 0,
            Exit::Finding => 1,
            Exit::Refused => 2,
            Exit::Usage => 3,
            Exit::Failed => 4,
        }
    }

    /// The word this is reported as.
    pub fn word(self) -> &'static str {
        match self {
            Exit::Ok => "ok",
            Exit::Finding => "finding",
            Exit::Refused => "refused",
            Exit::Usage => "usage",
            Exit::Failed => "failed",
        }
    }

    /// Every code. A test asserts the set stays closed and the numbers stay put.
    pub fn all() -> [Exit; 5] {
        [
            Exit::Ok,
            Exit::Finding,
            Exit::Refused,
            Exit::Usage,
            Exit::Failed,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vocabulary_is_five_codes_with_fixed_numbers() {
        assert_eq!(Exit::all().len(), 5);
        assert_eq!(Exit::Ok.code(), 0);
        assert_eq!(Exit::Finding.code(), 1);
        assert_eq!(Exit::Refused.code(), 2);
        assert_eq!(Exit::Usage.code(), 3);
        assert_eq!(Exit::Failed.code(), 4);
    }

    #[test]
    fn a_finding_a_refusal_and_a_failure_are_three_different_answers() {
        assert_ne!(Exit::Finding.code(), Exit::Refused.code());
        assert_ne!(Exit::Refused.code(), Exit::Failed.code());
        assert_ne!(Exit::Finding.code(), Exit::Failed.code());
    }

    #[test]
    fn only_ok_is_zero() {
        for e in Exit::all() {
            assert_eq!(e.code() == 0, e == Exit::Ok);
        }
    }
}
