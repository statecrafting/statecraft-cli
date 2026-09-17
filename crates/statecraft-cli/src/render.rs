//! Two renderings of one value.
//!
//! Spec 006 section 3.4. Every command supports `--json`. Human output and JSON
//! output are two renderings of the **same** returned value, produced from it by
//! this crate, never two code paths that compute their own answers.
//!
//! JSON output is a contract: adding a field is compatible, removing or
//! retyping one is a change to spec 006. Human output is not a contract and may
//! be reshaped freely, which is exactly why a caller is given `--json` instead.

use crate::exit::Exit;
use serde::Serialize;

/// Which rendering the caller asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// For a person.
    Human,
    /// For a caller. The contract.
    Json,
}

/// A command's answer: one value, plus how it should end the process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Answer<T: Serialize> {
    /// What the command found.
    pub value: T,
    /// The exit this implies.
    pub exit: Exit,
    /// What a person should read.
    ///
    /// Derived from `value` by the command, and carried in the JSON too, so the
    /// two renderings cannot disagree about what happened: there is one value
    /// and this is a view of it.
    pub summary: String,
}

impl<T: Serialize> Answer<T> {
    /// An answer.
    pub fn new(value: T, exit: Exit, summary: impl Into<String>) -> Self {
        Self {
            value,
            exit,
            summary: summary.into(),
        }
    }

    /// Render for the chosen format.
    pub fn render(&self, format: Format) -> String {
        match format {
            Format::Human => {
                if self.summary.ends_with('\n') {
                    self.summary.clone()
                } else {
                    format!("{}\n", self.summary)
                }
            }
            Format::Json => {
                let mut s = serde_json::to_string_pretty(self)
                    // A value this crate built and cannot serialize is a defect
                    // in this crate, and saying so is better than a panic in an
                    // operator's terminal.
                    .unwrap_or_else(|e| {
                        format!("{{\"error\":\"could not serialize the answer: {e}\"}}")
                    });
                s.push('\n');
                s
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, PartialEq, Eq, Debug, Clone)]
    struct Value {
        verdict: String,
        reasons: Vec<String>,
    }

    fn answer() -> Answer<Value> {
        Answer::new(
            Value {
                verdict: "ungoverned".into(),
                reasons: vec!["no spec-spine corpus is present".into()],
            },
            Exit::Finding,
            "ungoverned: no spec-spine corpus is present",
        )
    }

    #[test]
    fn both_renderings_come_from_one_value() {
        let a = answer();
        let human = a.render(Format::Human);
        let json = a.render(Format::Json);
        // The same fact appears in both, because there is one value behind them.
        assert!(human.contains("no spec-spine corpus is present"));
        assert!(json.contains("no spec-spine corpus is present"));
    }

    #[test]
    fn the_json_rendering_carries_the_exit_and_the_summary() {
        let json = answer().render(Format::Json);
        assert!(json.contains("\"exit\": \"finding\""));
        assert!(json.contains("\"summary\""));
    }

    #[test]
    fn the_human_rendering_ends_with_exactly_one_newline() {
        let out = answer().render(Format::Human);
        assert!(out.ends_with('\n'));
        assert!(!out.ends_with("\n\n"));
    }
}
