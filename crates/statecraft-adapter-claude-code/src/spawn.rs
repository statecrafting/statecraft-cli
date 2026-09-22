//! What a spawn of this provider consists of, as a value.
//!
//! Spec 004 section 3.9: the adapter spawns
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

/// The flags spec 004 section 3.9 fixes for every spawn.
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
    /// The exact bytes the settings file receives, when a caller has bytes an
    /// observation is bound to. `None` writes [`Invocation::settings`] in
    /// serde's compact form, which is the behavior every existing caller has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_document: Option<String>,
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
            settings_document: None,
        }
    }

    /// Register hooks in this invocation's settings, beside its deny rules.
    ///
    /// `hooks` is the provider's own `hooks` settings value, event name to
    /// matcher groups. Spec 004 section 5, 2026-09-22: spec 002 section 3.32
    /// rule 25 supplies a managed run's startup hook and admission gate this
    /// way, per invocation, rather than relying on a registration in the
    /// operator's home. It replaces any hooks set before, and must be called
    /// before [`Invocation::with_settings_document`], whose bytes must parse to
    /// the settings including these.
    pub fn with_hooks(mut self, hooks: serde_json::Value) -> Self {
        self.settings["hooks"] = hooks;
        self
    }

    /// Deliver these exact settings bytes rather than a serialization of
    /// [`Invocation::settings`].
    ///
    /// Spec 002 section 3.29 rule 4 binds an observation to the settings
    /// payload **by digest**, so a session that is to be covered by an
    /// observation has to receive the bytes the observation was made with, not
    /// an equivalent document formatted differently. The document must parse
    /// to the settings this invocation already carries, so the value its deny
    /// rules are read from and the bytes the child reads cannot disagree.
    pub fn with_settings_document(mut self, document: String) -> Result<Self, String> {
        let parsed: serde_json::Value = serde_json::from_str(&document)
            .map_err(|e| format!("the settings document is not JSON: {e}"))?;
        if parsed != self.settings {
            return Err(format!(
                "the settings document {parsed} is not the settings {} this invocation carries",
                self.settings
            ));
        }
        self.settings_document = Some(document);
        Ok(self)
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
    fn hooks_join_the_deny_rules_and_the_document_must_carry_both() {
        let hooks = serde_json::json!({"SessionStart": [{"matcher": "startup",
            "hooks": [{"type": "command", "command": "/x/hook.sh"}]}]});
        let invocation =
            Invocation::new("claude", &["Bash(x*)".to_string()], None).with_hooks(hooks.clone());
        assert_eq!(invocation.settings["hooks"], hooks);
        assert_eq!(invocation.deny_rules(), ["Bash(x*)"]);
        // The floor alone is no longer this invocation's settings.
        let floor_only = r#"{"permissions":{"deny":["Bash(x*)"]}}"#.to_string();
        assert!(
            invocation
                .clone()
                .with_settings_document(floor_only)
                .is_err()
        );
        let both = serde_json::to_string(&invocation.settings).unwrap();
        assert!(invocation.with_settings_document(both).is_ok());
    }

    #[test]
    fn a_settings_document_must_be_the_settings_the_invocation_carries() {
        let rules = ["Bash(echo:*)".to_string()];
        let i = Invocation::new("claude", &rules, None);
        let pretty = format!("{}\n", serde_json::to_string_pretty(&i.settings).unwrap());
        let with = i.clone().with_settings_document(pretty.clone()).unwrap();
        assert_eq!(with.settings_document.as_deref(), Some(pretty.as_str()));
        assert_eq!(with.deny_rules(), rules);
        assert!(i.clone().with_settings_document("{}".into()).is_err());
        assert!(i.with_settings_document("not json".into()).is_err());
    }

    #[test]
    fn an_invocations_applied_tool_set_starts_not_recorded() {
        let i = Invocation::new("claude", &["Bash".to_string()], None);
        let r = i.tool_restriction.unwrap();
        assert_eq!(r.mechanism, DenialMechanism::PermissionDenyRule);
        assert!(r.removal_took_effect().is_none());
    }
}
