//! The closed capability vocabulary, and what required and preferred mean.
//!
//! Spec 004 section 3.2. Six tokens in the first slice; **adding one is an
//! amendment, not a configuration change**. That is why this is an enum and not
//! a string: a seventh token cannot be introduced by a caller, only by editing
//! this file, which the coupling gate ties to editing the spec.
//!
//! Section 3.3 is the other half. A **required** token the manifest lacks is a
//! refusal before any process is spawned; a **preferred** token the manifest
//! lacks runs, and the degradation is recorded. A degradation is never silent
//! and never inferred from its absence.

use serde::{Deserialize, Serialize};

/// What an adapter can assert about its provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    /// The provider can restrict which tools the session may use.
    ToolAllowlist,
    /// The provider can cap the number of turns.
    TurnLimit,
    /// The provider can confine writes to the prepared workspace.
    WorkspaceWrite,
    /// The provider runs the repository's hooks and reports a blocked action as
    /// a structured event.
    HookEnforcement,
    /// The provider reports a cost for the session.
    CostReport,
    /// A refusal is a structured event, not text the supervisor would have to
    /// parse from a transcript.
    ///
    /// No special category: section 3.3 is explicit that this is an ordinary
    /// token and that the **run** decides whether to require it. Required and
    /// absent, it refuses like any other. Preferred and absent, the attempt runs
    /// and is labelled as carrying an unverifiable refusal account.
    StructuredRefusals,
}

impl Capability {
    /// The token as it is written.
    pub fn token(self) -> &'static str {
        match self {
            Capability::ToolAllowlist => "tool-allowlist",
            Capability::TurnLimit => "turn-limit",
            Capability::WorkspaceWrite => "workspace-write",
            Capability::HookEnforcement => "hook-enforcement",
            Capability::CostReport => "cost-report",
            Capability::StructuredRefusals => "structured-refusals",
        }
    }

    /// Every token in the vocabulary.
    ///
    /// A test asserts this has exactly six members, so widening the vocabulary
    /// without amending the spec fails the suite rather than passing quietly.
    pub fn all() -> [Capability; 6] {
        [
            Capability::ToolAllowlist,
            Capability::TurnLimit,
            Capability::WorkspaceWrite,
            Capability::HookEnforcement,
            Capability::CostReport,
            Capability::StructuredRefusals,
        ]
    }
}

/// What a run asks of an adapter.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Requested {
    /// Absent from the manifest: refused before spawn.
    #[serde(default)]
    pub required: Vec<Capability>,
    /// Absent from the manifest: runs, degraded, and recorded.
    #[serde(default)]
    pub preferred: Vec<Capability>,
}

impl Requested {
    /// Nothing asked for.
    pub fn none() -> Self {
        Self::default()
    }

    /// Require a token.
    #[must_use]
    pub fn requiring(mut self, c: Capability) -> Self {
        self.required.push(c);
        self
    }

    /// Prefer a token.
    #[must_use]
    pub fn preferring(mut self, c: Capability) -> Self {
        self.preferred.push(c);
        self
    }
}

/// How a request and a manifest line up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Negotiation {
    /// Required tokens the manifest lacks. Non-empty means refuse before spawn.
    pub missing_required: Vec<Capability>,
    /// Preferred tokens the manifest lacks. The attempt runs degraded.
    pub degraded: Vec<Capability>,
    /// Tokens that will be asked of the provider.
    pub granted: Vec<Capability>,
}

impl Negotiation {
    /// Whether this must refuse before any process is spawned.
    pub fn refuses(&self) -> bool {
        !self.missing_required.is_empty()
    }

    /// Whether the refusal account of this attempt can be trusted.
    ///
    /// Section 3.3: without `structured-refusals` the supervisor cannot satisfy
    /// constitution IX, so an attempt that merely preferred it is labelled as
    /// carrying an **unverifiable refusal account** rather than being quietly
    /// treated the same as one that has it.
    pub fn unverifiable_refusal_account(&self) -> bool {
        self.degraded.contains(&Capability::StructuredRefusals)
    }
}

/// Line a request up against what a manifest supports.
pub fn negotiate(requested: &Requested, supported: &[Capability]) -> Negotiation {
    let has = |c: &Capability| supported.contains(c);
    Negotiation {
        missing_required: requested
            .required
            .iter()
            .filter(|c| !has(c))
            .copied()
            .collect(),
        degraded: requested
            .preferred
            .iter()
            .filter(|c| !has(c))
            .copied()
            .collect(),
        granted: requested
            .required
            .iter()
            .chain(requested.preferred.iter())
            .filter(|c| has(c))
            .copied()
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vocabulary_has_exactly_six_tokens() {
        assert_eq!(Capability::all().len(), 6);
    }

    #[test]
    fn every_token_has_a_distinct_spelling() {
        let mut tokens: Vec<_> = Capability::all().iter().map(|c| c.token()).collect();
        tokens.sort_unstable();
        let before = tokens.len();
        tokens.dedup();
        assert_eq!(tokens.len(), before);
    }

    #[test]
    fn a_missing_required_token_refuses() {
        let n = negotiate(
            &Requested::none().requiring(Capability::WorkspaceWrite),
            &[Capability::TurnLimit],
        );
        assert!(n.refuses());
        assert_eq!(n.missing_required, [Capability::WorkspaceWrite]);
    }

    #[test]
    fn a_missing_preferred_token_degrades_rather_than_refusing() {
        let n = negotiate(
            &Requested::none().preferring(Capability::CostReport),
            &[Capability::TurnLimit],
        );
        assert!(!n.refuses());
        assert_eq!(n.degraded, [Capability::CostReport]);
    }

    #[test]
    fn structured_refusals_is_an_ordinary_token_in_whichever_category_the_run_puts_it() {
        let required = negotiate(
            &Requested::none().requiring(Capability::StructuredRefusals),
            &[],
        );
        assert!(required.refuses());

        let preferred = negotiate(
            &Requested::none().preferring(Capability::StructuredRefusals),
            &[],
        );
        assert!(!preferred.refuses());
        assert!(preferred.unverifiable_refusal_account());
    }

    #[test]
    fn an_attempt_that_has_structured_refusals_has_a_verifiable_account() {
        let n = negotiate(
            &Requested::none().preferring(Capability::StructuredRefusals),
            &[Capability::StructuredRefusals],
        );
        assert!(!n.unverifiable_refusal_account());
        assert_eq!(n.granted, [Capability::StructuredRefusals]);
    }
}
