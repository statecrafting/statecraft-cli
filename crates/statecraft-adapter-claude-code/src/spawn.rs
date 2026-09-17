//! What a spawn of this provider consists of, as a value.
//!
//! Spec 008 section 3.1: the adapter spawns
//! `claude --print --output-format stream-json --verbose`, and **the prompt is
//! delivered on a stream and never interpolated into a command line** (spec 004
//! section 3.1).
//!
//! Nothing here spawns. [`statecraft_adapter::supervisor::supervise`] does that,
//! holds the deadline and kills descendants. This module exists so the
//! invocation is a value a run record can carry and a test can assert on, rather
//! than a string assembled at the call site where nobody can see it.

use crate::denial::{DenialMechanism, ToolRestriction};
use serde::{Deserialize, Serialize};

/// Where a refusal-bearing restriction lives in the settings document.
///
/// Section 3.4: this is the mechanism that produces a structured
/// `permission_denials` entry, and it is the only one permitted for a
/// restriction whose refusal must survive as evidence.
pub const SETTINGS_DENY_KEY: &str = "permissions.deny";

/// The flags spec 008 section 3.1 fixes for every spawn.
pub const BASE_ARGS: [&str; 4] = ["--print", "--output-format", "stream-json", "--verbose"];

/// One spawn, as a value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Invocation {
    /// The program, resolved from the constructed environment's `PATH`.
    pub program: String,
    /// Its arguments. **Never carries the prompt**: see [`Invocation::args`].
    pub args: Vec<String>,
    /// The settings document, where a refusal-bearing restriction lives.
    pub settings: serde_json::Value,
    /// What was asked of the tool set, and what is known to have been applied.
    pub tool_restriction: Option<ToolRestriction>,
}

impl Invocation {
    /// The invocation for one attempt.
    ///
    /// `deny` is expressed as a `permissions.deny` rule, which is the only
    /// mechanism section 3.4 permits for a restriction whose refusal must be
    /// evidence. `max_turns` is the `turn-limit` token, whose basis was measured
    /// with `--max-turns 1`.
    pub fn new(program: &str, deny: &[String], max_turns: Option<u32>) -> Self {
        let mut args: Vec<String> = BASE_ARGS.iter().map(|a| (*a).to_string()).collect();
        if let Some(turns) = max_turns {
            args.push("--max-turns".to_string());
            args.push(turns.to_string());
        }
        Self {
            program: program.to_string(),
            args,
            settings: serde_json::json!({ "permissions": { "deny": deny } }),
            tool_restriction: Some(ToolRestriction::observed(
                DenialMechanism::PermissionDenyRule,
                deny.to_vec(),
                Vec::new(),
            )),
        }
    }

    /// The arguments, borrowed for a supervisor call.
    pub fn args(&self) -> Vec<&str> {
        self.args.iter().map(String::as_str).collect()
    }

    /// The deny rules this invocation carries.
    pub fn deny_rules(&self) -> Vec<String> {
        self.settings["permissions"]["deny"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stream_json_flags_are_fixed_by_the_spec_and_always_present() {
        let i = Invocation::new("claude", &[], None);
        assert_eq!(
            i.args[..4],
            ["--print", "--output-format", "stream-json", "--verbose"]
        );
    }

    #[test]
    fn a_turn_limit_is_a_flag_and_its_absence_adds_nothing() {
        assert!(
            !Invocation::new("claude", &[], None)
                .args
                .contains(&"--max-turns".to_string())
        );
        let capped = Invocation::new("claude", &[], Some(1));
        assert_eq!(capped.args[4..], ["--max-turns", "1"]);
    }

    #[test]
    fn a_denial_goes_in_the_settings_document_and_never_on_the_command_line() {
        let i = Invocation::new("claude", &["Bash(echo:*)".to_string()], None);
        assert_eq!(i.deny_rules(), ["Bash(echo:*)"]);
        // Section 3.4: tool-set removal is never used for a refusal that must
        // survive, so no `--disallowedTools` is ever built here.
        assert!(!i.args.iter().any(|a| a.contains("disallowedTools")));
        assert!(!i.args.iter().any(|a| a.contains("Bash")));
    }

    #[test]
    fn no_prompt_can_reach_the_argument_vector() {
        // There is no parameter for one, which is the assertion. The prompt
        // rides on `Request::prompt` and the supervisor writes it to stdin.
        let i = Invocation::new("claude", &["Bash".to_string()], Some(3));
        let joined = i.args.join(" ");
        assert!(!joined.contains("prompt"));
        assert_eq!(i.args.len(), 6);
    }

    #[test]
    fn an_invocations_applied_tool_set_starts_not_recorded() {
        let i = Invocation::new("claude", &["Bash".to_string()], None);
        let r = i.tool_restriction.unwrap();
        assert_eq!(r.mechanism, DenialMechanism::PermissionDenyRule);
        assert!(r.removal_took_effect().is_none());
    }
}
