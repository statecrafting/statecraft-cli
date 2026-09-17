//! Independence: the suite is run by this product, and a claim is only a claim.
//!
//! Spec 005 section 3.2. Three consequences, all of them types here:
//!
//! 1. An agent's statement that it finished is recorded as a **claim**, in a
//!    field named for a claim, and is never read as a result.
//! 2. Where a tool emits a structured report, **that report is what is read**. A
//!    zero exit code is not substituted for one.
//! 3. A check that did not run is `unknown`. It is never a pass, and **the count
//!    of checks that did not run is part of the outcome**.

use serde::{Deserialize, Serialize};

/// What one check did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum CheckState {
    /// It ran and passed.
    Passed,
    /// It ran and failed.
    Failed,
    /// It did not run. **Never a pass.**
    Unknown {
        /// Why it did not run.
        reason: String,
    },
}

/// One check in the ordered suite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    /// The command, as it was run.
    pub command: String,
    /// Its exit code, when it ran.
    pub exit_code: Option<i32>,
    /// The structured report's verdict, when the tool emits one.
    ///
    /// `Some(false)` with an exit code of zero is the case section 3.10 names:
    /// **the report wins**.
    pub structured_pass: Option<bool>,
    /// What this check amounts to.
    pub state: CheckState,
}

impl Check {
    /// A check that ran, judged the way section 3.2 requires.
    ///
    /// The structured report is read where one exists; the exit code is the
    /// fallback, not the authority.
    pub fn ran(command: &str, exit_code: i32, structured_pass: Option<bool>) -> Self {
        let passed = match structured_pass {
            Some(report) => report,
            None => exit_code == 0,
        };
        Self {
            command: command.to_string(),
            exit_code: Some(exit_code),
            structured_pass,
            state: if passed {
                CheckState::Passed
            } else {
                CheckState::Failed
            },
        }
    }

    /// A check that did not run.
    pub fn did_not_run(command: &str, reason: &str) -> Self {
        Self {
            command: command.to_string(),
            exit_code: None,
            structured_pass: None,
            state: CheckState::Unknown {
                reason: reason.to_string(),
            },
        }
    }

    /// A check whose tool should have emitted a structured report and did not.
    ///
    /// Section 3.10: `unknown` for what the report would have carried, and the
    /// missing report is named. The exit code is retained but does not decide.
    pub fn missing_report(command: &str, exit_code: i32) -> Self {
        Self {
            command: command.to_string(),
            exit_code: Some(exit_code),
            structured_pass: None,
            state: CheckState::Unknown {
                reason: format!(
                    "{command} emitted no structured report where one was expected; \
                     its exit code {exit_code} is recorded and is not read as the verdict"
                ),
            },
        }
    }

    /// Whether this check passed.
    pub fn passed(&self) -> bool {
        matches!(self.state, CheckState::Passed)
    }

    /// Whether this check did not run.
    pub fn unknown(&self) -> bool {
        matches!(self.state, CheckState::Unknown { .. })
    }
}

/// The ordered suite, and what the agent claimed about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuiteResult {
    /// Every check, in order.
    pub checks: Vec<Check>,
    /// What the agent said, kept in a field named for a claim.
    ///
    /// Never read as a result. Its only role in any decision is that it is
    /// retained beside one.
    pub agent_claim: Option<String>,
}

impl SuiteResult {
    /// The suite as run, with whatever the agent claimed.
    pub fn new(checks: Vec<Check>, agent_claim: Option<String>) -> Self {
        Self {
            checks,
            agent_claim,
        }
    }

    /// How many checks did not run. Part of the outcome, not a footnote.
    pub fn unrun_count(&self) -> u32 {
        self.checks.iter().filter(|c| c.unknown()).count() as u32
    }

    /// Whether every check ran and passed.
    ///
    /// An unrun check makes this false, because `unknown` is never a pass.
    pub fn passed(&self) -> bool {
        !self.checks.is_empty() && self.checks.iter().all(Check::passed)
    }

    /// Whether the suite ran at all.
    pub fn ran(&self) -> bool {
        self.checks.iter().any(|c| !c.unknown())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_structured_report_beats_a_zero_exit_code() {
        let c = Check::ran("spec-spine verify 001", 0, Some(false));
        assert!(!c.passed(), "the report wins");
        assert_eq!(c.exit_code, Some(0), "and the exit code is still recorded");
    }

    #[test]
    fn with_no_structured_report_the_exit_code_is_the_fallback() {
        assert!(Check::ran("make gate", 0, None).passed());
        assert!(!Check::ran("make gate", 1, None).passed());
    }

    #[test]
    fn a_missing_expected_report_is_unknown_and_names_the_omission() {
        let c = Check::missing_report("spec-spine verify 001", 0);
        assert!(c.unknown());
        match &c.state {
            CheckState::Unknown { reason } => assert!(reason.contains("no structured report")),
            other => panic!("expected unknown, got {other:?}"),
        }
    }

    #[test]
    fn an_unrun_check_is_never_a_pass_and_is_counted() {
        let suite = SuiteResult::new(
            vec![
                Check::ran("a", 0, None),
                Check::did_not_run("b", "the runner never reached it"),
            ],
            None,
        );
        assert!(!suite.passed());
        assert_eq!(suite.unrun_count(), 1);
    }

    #[test]
    fn the_agents_claim_is_retained_and_changes_nothing() {
        let suite = SuiteResult::new(
            vec![Check::ran("a", 1, None)],
            Some("I finished the work and it all passes".into()),
        );
        assert!(!suite.passed());
        assert!(suite.agent_claim.is_some(), "retained");
        let json = serde_json::to_string(&suite).unwrap();
        assert!(json.contains("agent_claim"), "in a field named for a claim");
    }

    #[test]
    fn a_suite_of_only_unrun_checks_did_not_run() {
        let suite = SuiteResult::new(vec![Check::did_not_run("a", "never started")], None);
        assert!(!suite.ran());
        assert!(!suite.passed());
    }
}
