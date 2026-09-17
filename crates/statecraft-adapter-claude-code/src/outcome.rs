//! Spec 008 section 3.5's fixed outcome mapping.
//!
//! | Provider terminal state | `003` section 3.4 outcome | Why |
//! |---|---|---|
//! | `success`, no denials | `completed` | Reached its own end. Says nothing about acceptance. |
//! | `success`, denials present | `refused` | Section 3.3. The completed turns are retained beside the refusal. |
//! | `terminal_reason: "max_turns"` | `interrupted` | The cap stopped the attempt before anything was judged. **Not** `failed`: nothing about the work was found not to hold. The provider calls it an error and that reading is not adopted. |
//! | deadline passed, child killed with descendants | `interrupted` | Spec 004 section 3.5 case 3, decided by the supervisor and not here. |
//! | malformed or truncated stream | reported as malformed | Spec 004 section 3.5 case 4. Never a clean completion with missing fields. |
//!
//! `cancelled` is never produced here: it means an operator stopped the attempt
//! deliberately, which the supervisor knows and the provider does not.
//!
//! **The exit code is not read.** A refused session exited 0 and a turn-capped
//! one exited 1, so the code is unreliable in both directions and this module
//! takes no parameter for it.

use crate::stream::ResultEvent;
use statecraft_adapter::protocol::Classification;
use statecraft_run::attempt::Outcome;

/// A terminal state section 3.5's table does not cover.
///
/// Guessing would be the one thing this module exists to prevent. Spec 004
/// section 3.5 case 4 already says a stream that cannot be read as a result is
/// reported rather than interpreted, and a terminal subtype nobody measured is
/// the same situation one field along.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "provider terminal state (subtype `{subtype}`, terminal_reason {terminal_reason:?}) is not \
     in spec 008 section 3.5's mapping; reported rather than mapped, because an outcome this \
     adapter invented would be an outcome nobody measured"
)]
pub struct UnmappedTerminalState {
    /// The subtype the provider reported.
    pub subtype: String,
    /// The terminal reason it reported, when it reported one.
    pub terminal_reason: Option<String>,
}

/// What section 3.5's mapping produced, with the provider's own claim beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalReading {
    /// The outcome spec 003 section 3.4 records.
    pub outcome: Outcome,
    /// The provider's own terminal classification, kept as a claim.
    ///
    /// Section 3.3 rule 2: carried in a field named for a claim, and **never
    /// promoted to the attempt's outcome**.
    pub provider_claim: Classification,
    /// How many refusal entries the result carried.
    pub refusal_entries: usize,
    /// How many turns completed, retained beside a refusal.
    pub completed_turns: u32,
}

/// The provider's own classification of its termination, as a claim.
///
/// Read straight off the provider's fields with no correction applied. That is
/// the point: the correction is [`outcome`], and keeping the two separate is
/// what makes the disagreement in section 3.3 visible in a record instead of
/// resolved silently in a match arm.
pub fn provider_claim(result: &ResultEvent) -> Classification {
    if result.terminal_reason.as_deref() == Some("max_turns") {
        return Classification::Stopped;
    }
    if result.is_error {
        return Classification::Failed;
    }
    Classification::Completed
}

/// Map a result event onto spec 003 section 3.4's closed set.
pub fn outcome(result: &ResultEvent) -> Result<TerminalReading, UnmappedTerminalState> {
    let reading = |outcome| {
        Ok(TerminalReading {
            outcome,
            provider_claim: provider_claim(result),
            refusal_entries: result.permission_denials.len(),
            completed_turns: result.num_turns,
        })
    };

    // Checked first, and before the subtype: the provider reports this state as
    // `error_max_turns` with `is_error: true`, and section 3.5 does not adopt
    // that reading. Nothing about the work was found not to hold.
    if result.terminal_reason.as_deref() == Some("max_turns") {
        return reading(Outcome::Interrupted);
    }

    match result.subtype.as_str() {
        "success" if result.refused_anything() => reading(Outcome::Refused),
        "success" => reading(Outcome::Completed),
        _ => Err(UnmappedTerminalState {
            subtype: result.subtype.clone(),
            terminal_reason: result.terminal_reason.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::PermissionDenial;

    fn result(
        subtype: &str,
        terminal: Option<&str>,
        denials: usize,
        is_error: bool,
    ) -> ResultEvent {
        serde_json::from_value(serde_json::json!({
            "subtype": subtype,
            "is_error": is_error,
            "terminal_reason": terminal,
            "num_turns": 2,
            "permission_denials": (0..denials)
                .map(|i| serde_json::to_value(PermissionDenial {
                    tool_name: "Bash".into(),
                    tool_use_id: format!("t{i}"),
                    tool_input: serde_json::json!({"command": "echo hello"}),
                }).unwrap())
                .collect::<Vec<_>>(),
        }))
        .unwrap()
    }

    #[test]
    fn a_denied_session_that_calls_itself_a_success_is_refused() {
        let r = outcome(&result("success", Some("completed"), 1, false)).unwrap();
        assert_eq!(r.outcome, Outcome::Refused);
        // The provider's claim is retained, unchanged, beside the correction.
        assert_eq!(r.provider_claim, Classification::Completed);
        assert_eq!(r.refusal_entries, 1);
        // Section 3.3: the completed turns are retained beside the refusal.
        assert_eq!(r.completed_turns, 2);
    }

    #[test]
    fn an_undenied_success_is_completed_and_claims_nothing_about_acceptance() {
        let r = outcome(&result("success", Some("completed"), 0, false)).unwrap();
        assert_eq!(r.outcome, Outcome::Completed);
        assert_eq!(r.refusal_entries, 0);
    }

    #[test]
    fn a_turn_cap_is_interrupted_and_not_failed() {
        let r = outcome(&result("error_max_turns", Some("max_turns"), 0, true)).unwrap();
        assert_eq!(r.outcome, Outcome::Interrupted);
        assert_ne!(r.outcome, Outcome::Failed);
        // The provider calls it an error; that reading is not adopted as the
        // outcome, and it is still recorded as the provider's claim.
        assert_eq!(r.provider_claim, Classification::Stopped);
    }

    #[test]
    fn cancelled_is_never_produced_here() {
        for (subtype, terminal, denials, is_error) in [
            ("success", Some("completed"), 0, false),
            ("success", Some("completed"), 1, false),
            ("error_max_turns", Some("max_turns"), 0, true),
        ] {
            let r = outcome(&result(subtype, terminal, denials, is_error)).unwrap();
            assert_ne!(r.outcome, Outcome::Cancelled);
        }
    }

    #[test]
    fn a_terminal_state_nobody_measured_is_reported_rather_than_guessed() {
        let e = outcome(&result("error_during_execution", Some("errored"), 0, true)).unwrap_err();
        assert_eq!(e.subtype, "error_during_execution");
        assert!(e.to_string().contains("reported rather than mapped"));
        assert!(e.to_string().contains("error_during_execution"));
    }
}
