//! Two denial mechanisms with opposite evidence properties.
//!
//! Spec 008 section 3.4, measured on Claude Code 2.1.267 with the same prompt:
//!
//! | Mechanism | Reflected in the init event | Produces a refusal record |
//! |---|---|---|
//! | `--disallowedTools Bash` | yes: `tools` had 89 entries and `Bash` was absent | **no**: `permission_denials` was empty, and the session said in prose that it had no such tool |
//! | a `permissions.deny` rule | **no**: `tools` had 88 entries and `Bash` was present | yes: one structured entry with the tool, the id and the input |
//! | `--allowedTools Read` | no: `tools` had 88 entries including `Bash` and `Edit` | not measured |
//!
//! Neither mechanism alone satisfies spec 004 section 3.3, which wants the init
//! event to carry what was actually applied, and spec 004 section 3.1, which
//! wants a refusal to be a structured event rather than text parsed out of a
//! transcript. One is observable and unrecorded; the other is recorded and
//! unobservable.
//!
//! So this adapter **must use both, for different purposes**, and section 3.4
//! writes that down as a requirement rather than leaving it to whoever
//! implements it:
//!
//! - anything whose refusal must survive as evidence is a
//!   [`DenialMechanism::PermissionDenyRule`];
//! - the applied tool set is read from the init event's `tools`, which reflects
//!   removal, and the applied **allowlist** is reported `not-recorded`.
//!
//! An adapter that reported the requested allowlist as though it were the
//! applied one would satisfy spec 004 section 3.3's letter and defeat its
//! purpose. [`ToolRestriction::applied`] is why it cannot.

use serde::{Deserialize, Serialize};
use statecraft_envelope::absence::{Absence, Recorded};

/// How a tool restriction was expressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DenialMechanism {
    /// A `permissions.deny` rule in the settings document.
    ///
    /// Recorded: produces a `permission_denials` entry. Unobservable: the init
    /// event still lists the tool.
    PermissionDenyRule,
    /// `--disallowedTools`, which removes the tool from the set.
    ///
    /// Observable: the init event's `tools` reflects it. Unrecorded: a removal
    /// the model then wants produces no structured entry and reaches the
    /// supervisor only as prose.
    ToolSetRemoval,
    /// `--allowedTools`, which was not measured for what it applies.
    Allowlist,
}

impl DenialMechanism {
    /// Whether a refusal expressed this way survives as a structured record.
    ///
    /// This is the predicate section 3.4 turns into a requirement, and
    /// [`refusal_bearing`] is the check that enforces it.
    pub fn produces_a_refusal_record(self) -> bool {
        matches!(self, DenialMechanism::PermissionDenyRule)
    }

    /// Whether the init event reflects what this mechanism applied.
    pub fn observable_in_the_init_event(self) -> bool {
        matches!(self, DenialMechanism::ToolSetRemoval)
    }

    /// The token as it is written in a record.
    pub fn word(self) -> &'static str {
        match self {
            DenialMechanism::PermissionDenyRule => "permission-deny-rule",
            DenialMechanism::ToolSetRemoval => "tool-set-removal",
            DenialMechanism::Allowlist => "allowlist",
        }
    }
}

/// A restriction expressed the wrong way for what it has to prove.
///
/// Spec 008 section 3.9: a tool restriction expressed as tool-set removal where
/// a refusal record is required **fails qualification**, because the manifest
/// declared `structured-refusals` and that path produces none.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "restriction on {tools:?} was expressed as {mechanism} and must survive as evidence; \
     this adapter declares `structured-refusals` and that mechanism produces none, \
     so the declaration and the invocation disagree and the binary fails qualification \
     (spec 008 section 3.4)"
)]
pub struct NotRefusalBearing {
    /// The tools the restriction covers.
    pub tools: Vec<String>,
    /// The mechanism that was chosen, written as it is recorded.
    pub mechanism: &'static str,
}

/// Check that a restriction whose refusal must be evidence uses the right
/// mechanism.
///
/// Refuses **before** anything is spawned, which is the only point at which the
/// answer is still cheap: once the session has run, the refusal either exists as
/// a record or is gone.
pub fn refusal_bearing(
    tools: &[String],
    mechanism: DenialMechanism,
) -> Result<(), NotRefusalBearing> {
    if mechanism.produces_a_refusal_record() {
        return Ok(());
    }
    Err(NotRefusalBearing {
        tools: tools.to_vec(),
        mechanism: mechanism.word(),
    })
}

/// What was asked of the tool set, and what is known to have been applied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolRestriction {
    /// How it was expressed.
    pub mechanism: DenialMechanism,
    /// What was asked for.
    pub requested: Vec<String>,
    /// What was applied, when the mechanism lets the init event say.
    ///
    /// `not-recorded` for an allowlist and for a deny rule, which is section
    /// 3.4's conclusion and not a gap in this implementation. Spec 005 section
    /// 3.8's three names for absence are the vocabulary, and this is the
    /// `not-recorded` one: no record of this kind exists, as opposed to somebody
    /// looked and there was nothing.
    pub applied: Recorded<Vec<String>>,
}

impl ToolRestriction {
    /// A restriction whose applied set the init event reflects.
    ///
    /// Only [`DenialMechanism::ToolSetRemoval`] qualifies. Calling this with
    /// another mechanism records `not-recorded`, because the alternative is an
    /// adapter that restates its own request as an observation.
    pub fn observed(
        mechanism: DenialMechanism,
        requested: Vec<String>,
        applied_tools: Vec<String>,
    ) -> Self {
        let applied = if mechanism.observable_in_the_init_event() {
            Recorded::Present(applied_tools)
        } else {
            Recorded::Absent(Absence::NotRecorded)
        };
        Self {
            mechanism,
            requested,
            applied,
        }
    }

    /// Whether every requested removal is actually absent from the applied set.
    ///
    /// `None` when the mechanism does not let the init event answer, which is
    /// the difference between "the removal did not take" and "nobody can say".
    pub fn removal_took_effect(&self) -> Option<bool> {
        match &self.applied {
            Recorded::Absent(_) => None,
            Recorded::Present(applied) => Some(self.requested.iter().all(|t| !applied.contains(t))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_that_must_be_evidence_may_not_be_a_tool_set_removal() {
        let e =
            refusal_bearing(&["Bash".to_string()], DenialMechanism::ToolSetRemoval).unwrap_err();
        assert!(e.to_string().contains("fails qualification"));
        assert!(e.to_string().contains("structured-refusals"));
        assert!(
            refusal_bearing(&["Bash".to_string()], DenialMechanism::PermissionDenyRule).is_ok()
        );
    }

    #[test]
    fn an_allowlist_is_not_refusal_bearing_either() {
        assert!(refusal_bearing(&["Read".to_string()], DenialMechanism::Allowlist).is_err());
    }

    #[test]
    fn an_applied_allowlist_is_not_recorded_and_never_the_request_restated() {
        let r = ToolRestriction::observed(
            DenialMechanism::Allowlist,
            vec!["Read".into()],
            vec!["Bash".into(), "Edit".into(), "Read".into()],
        );
        assert_eq!(r.applied, Recorded::Absent(Absence::NotRecorded));
        assert_eq!(
            serde_json::to_value(&r.applied).unwrap(),
            serde_json::json!("not-recorded")
        );
        assert_eq!(r.removal_took_effect(), None);
    }

    #[test]
    fn a_deny_rules_applied_set_is_not_recorded_because_the_init_event_still_lists_the_tool() {
        let r = ToolRestriction::observed(
            DenialMechanism::PermissionDenyRule,
            vec!["Bash".into()],
            vec!["Bash".into()],
        );
        assert_eq!(r.applied, Recorded::Absent(Absence::NotRecorded));
    }

    #[test]
    fn a_removal_is_observable_and_says_whether_it_took() {
        let took = ToolRestriction::observed(
            DenialMechanism::ToolSetRemoval,
            vec!["Bash".into()],
            vec!["Read".into(), "Edit".into()],
        );
        assert_eq!(took.removal_took_effect(), Some(true));

        let did_not = ToolRestriction::observed(
            DenialMechanism::ToolSetRemoval,
            vec!["Bash".into()],
            vec!["Bash".into()],
        );
        assert_eq!(did_not.removal_took_effect(), Some(false));
    }
}
