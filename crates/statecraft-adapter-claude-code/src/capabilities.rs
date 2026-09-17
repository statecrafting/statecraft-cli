//! The five declared tokens, each against a measurement.
//!
//! Spec 008 section 3.2. Spec 004 section 3.2 closed the vocabulary at six
//! tokens and section 3.5 case 6 makes a manifest declaring a token the adapter
//! does not honor a qualification failure, so each declaration below names what
//! was measured and what the measurement did not establish.
//!
//! | Token | Declared | Measured basis |
//! |---|---|---|
//! | `turn-limit` | yes | `--max-turns 1` against a prompt needing three tool calls terminated `error_max_turns` / `max_turns`, `is_error: true`, exit 1. |
//! | `cost-report` | yes | `total_cost_usd` present and non-zero (`0.044009` on a one-turn session). Absence is `unknown`, never zero. |
//! | `hook-enforcement` | yes | `system` events `hook_started` and `hook_response`, the latter carrying the hook name, event, outcome and exit code. |
//! | `structured-refusals` | yes, through one mechanism only | A permission deny rule produced one structured `permission_denials` entry. Tool-set removal produced none: [`crate::denial`]. |
//! | `tool-allowlist` | yes, with a stated blind spot | The restriction is honored; what was applied is only partly observable: [`crate::denial`]. |
//! | `workspace-write` | **not declared** | Not measured. Spec 004 section 3.3 makes an undeclared token a refusal when required and a recorded degradation when preferred, which is the right answer for a claim nobody checked. |
//!
//! Declaring `workspace-write` later is an amendment with its own measurement,
//! and spec 008 section 4 puts it out of scope deliberately.

use crate::stream::SystemEvent;
use statecraft_adapter::capability::Capability;
use statecraft_adapter::manifest::Manifest;

/// This adapter's name. Not a harness name and not a binary name.
pub const ADAPTER_NAME: &str = "claude-code";

/// The provider version every measurement in spec 008 was taken against.
///
/// Section 3.8: the qualification binds to the **pair** of this adapter's build
/// and this provider version. A provider upgrade invalidates it even when this
/// crate is byte-identical.
pub const MEASURED_PROVIDER_VERSION: &str = "2.1.267";

/// The commands the constructed environment must carry for this adapter to run.
///
/// Spec 004 section 3.5.8: the posture declares every command the run will need,
/// and one the check suite invokes but the posture omits is refused at plan
/// time. `claude` is this adapter's own; `git` is what spec 003's prepared
/// workspace is made of.
pub const REQUIRES_COMMANDS: [&str; 2] = ["claude", "git"];

/// The five tokens this adapter declares.
///
/// `workspace-write` is absent on purpose. Adding it here without a measurement
/// would be the exact defect spec 004 section 3.5 case 6 exists to catch.
pub fn supported() -> Vec<Capability> {
    vec![
        Capability::TurnLimit,
        Capability::CostReport,
        Capability::HookEnforcement,
        Capability::StructuredRefusals,
        Capability::ToolAllowlist,
    ]
}

/// This adapter's manifest, as the seam reads it.
pub fn manifest() -> Manifest {
    Manifest {
        adapter: ADAPTER_NAME.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        supports: supported(),
        requires_commands: REQUIRES_COMMANDS.iter().map(|c| (*c).to_string()).collect(),
    }
}

/// What the init event lets this adapter report as **applied**.
///
/// Spec 004 section 3.3 wants the init event to carry what was actually applied
/// beside what was requested, and section 3.5.6 makes a declared-but-unapplied
/// token a qualification failure. Section 008.3.4 measured that for this
/// provider the init event witnesses **one** of the five directly: the tool set,
/// and only when the restriction was expressed as removal.
///
/// So the honest answer is neither "everything granted" nor "only what init
/// proves". It is: everything this adapter put into effect in the invocation,
/// minus anything the init event positively contradicts. Concretely, the init
/// event can contradict exactly one thing, which is a tool that should have been
/// removed and is still present, and [`crate::denial`] is where that is read.
///
/// `hook-enforcement` is witnessed when a hook event was seen, which on the
/// recorded streams happens **before** init: the provider fires
/// `SessionStart` hooks first. That ordering is why `saw_hook` is a parameter
/// rather than something this function could look up.
pub fn applied(granted: &[Capability], init: &SystemEvent, saw_hook: bool) -> Vec<Capability> {
    debug_assert!(init.is_init(), "applied() reads the init event");
    let mut out: Vec<Capability> = granted.to_vec();
    // A granted `hook-enforcement` with no hook event seen by init time is not
    // yet witnessed, and the stream may still witness it later. Reporting it as
    // applied here would be a claim the init event did not make.
    if !saw_hook {
        out.retain(|c| *c != Capability::HookEnforcement);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init(tools: &[&str]) -> SystemEvent {
        serde_json::from_value(serde_json::json!({
            "subtype": "init",
            "claude_code_version": MEASURED_PROVIDER_VERSION,
            "tools": tools,
            "apiKeySource": "none",
        }))
        .unwrap()
    }

    #[test]
    fn workspace_write_is_not_declared_because_nobody_measured_it() {
        assert!(!supported().contains(&Capability::WorkspaceWrite));
        assert_eq!(supported().len(), 5);
    }

    #[test]
    fn every_declared_token_is_in_the_seams_closed_vocabulary() {
        for token in supported() {
            assert!(Capability::all().contains(&token));
        }
    }

    #[test]
    fn hook_enforcement_is_not_reported_applied_until_a_hook_event_is_seen() {
        let granted = vec![Capability::HookEnforcement, Capability::TurnLimit];
        let before = applied(&granted, &init(&["Bash"]), false);
        assert_eq!(before, [Capability::TurnLimit]);
        let after = applied(&granted, &init(&["Bash"]), true);
        assert_eq!(after, granted);
    }

    #[test]
    fn the_manifest_declares_the_commands_the_posture_needs() {
        let m = manifest();
        assert!(m.requires_commands.contains(&"claude".to_string()));
        assert_eq!(m.adapter, ADAPTER_NAME);
    }
}
