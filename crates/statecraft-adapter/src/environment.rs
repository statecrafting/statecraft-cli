//! The child's environment is constructed, not filtered.
//!
//! Spec 004 section 3.6. The supervisor builds the child's environment from an
//! **allowed set** rather than removing names from its own. A deny list is a
//! list of the paths somebody thought of; a constructed environment is the
//! complete set of what the child gets.
//!
//! # What this buys, stated exactly
//!
//! The supervisor's path is the only publishing path this product *provides*.
//! That is a statement about what the product hands the child, not about what
//! the child can reach. Constitution VIII requires the mechanism and the
//! residual to be named together, so [`RESIDUALS`] is part of the API and
//! [`Posture`] carries it into every attempt record.
//!
//! **This module claims no isolation.** No document in this repository may say
//! the child cannot publish, and section 3.8 makes such a claim a review
//! refusal.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The ways a capable child defeats a constructed environment.
///
/// Named here, in the code, and carried into every attempt: constitution VIII
/// forbids claiming a protection without naming its enforcement, and two of
/// these were **measured** in the archived predecessor rather than imagined.
pub const RESIDUALS: [&str; 4] = [
    "a reachable absolute path bypasses any path-based redirection",
    "the home directory remains readable unless an operating-system mechanism is applied, \
     which this product does not apply",
    "a credential held in an OS keychain answers a process that asks for it, and at least \
     one supported provider authenticates from that same keychain, so denying it breaks \
     the provider",
    "nothing here prevents the child from using a credential it finds by any of the above; \
     a publish that goes around the supervisor leaves no record",
];

/// Whether the constructed environment was applied, degraded, or refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum EnvironmentState {
    /// Built as specified.
    Applied,
    /// Built, but something asked for could not be provided.
    Degraded {
        /// What could not be provided.
        reasons: Vec<String>,
    },
    /// Not built, so nothing was spawned.
    Refused {
        /// Why.
        reasons: Vec<String>,
    },
}

/// A constructed child environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildEnvironment {
    /// Exactly what the child gets. Nothing else is inherited.
    pub variables: BTreeMap<String, String>,
    /// The commands the posture declares the run will need.
    pub commands: Vec<String>,
    /// Applied, degraded, or refused.
    pub state: EnvironmentState,
}

/// What the supervisor was asked to construct.
#[derive(Debug, Clone, Default)]
pub struct Blueprint {
    allowed: Vec<(String, String)>,
    commands: Vec<String>,
}

impl Blueprint {
    /// An empty environment. The child gets nothing it is not given.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Allow one name with one value.
    #[must_use]
    pub fn allowing(mut self, name: &str, value: &str) -> Self {
        self.allowed.push((name.to_string(), value.to_string()));
        self
    }

    /// Declare a command the run will need.
    #[must_use]
    pub fn needing_command(mut self, command: &str) -> Self {
        self.commands.push(command.to_string());
        self
    }
}

/// Names that must never be placed in a child's environment.
///
/// Section 3.6: publication credentials and approval authority are not placed
/// in the child, and every publishing effect is one the supervisor performs and
/// records. This is a refusal on construction, not a filter on inheritance: the
/// child inherits nothing, so the only way one of these could reach it is by
/// being written into a blueprint, and that is what this refuses.
pub const WITHHELD_PREFIXES: [&str; 6] = [
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "CARGO_REGISTRY_TOKEN",
    "NPM_TOKEN",
    "AWS_SECRET",
    "STATECRAFT_APPROVAL",
];

fn withheld(name: &str) -> bool {
    WITHHELD_PREFIXES
        .iter()
        .any(|p| name.eq_ignore_ascii_case(p) || name.to_ascii_uppercase().starts_with(p))
}

/// What the check suite needs, so a posture omitting one of them can be refused.
#[derive(Debug, Clone, Default)]
pub struct CheckSuiteCommands(pub Vec<String>);

/// Construct the child's environment.
///
/// Two ways this refuses, both before anything is spawned:
///
/// 1. A blueprint that places a publication credential or approval authority in
///    the child.
/// 2. A posture that omits a command the repository's check suite invokes
///    (section 3.5.8). **An ambient allowance on the operator's machine must not
///    be what makes a run work**, because it is absent on the next machine.
pub fn construct(blueprint: &Blueprint, suite: &CheckSuiteCommands) -> ChildEnvironment {
    let mut reasons = Vec::new();
    let mut variables = BTreeMap::new();

    for (name, value) in &blueprint.allowed {
        if withheld(name) {
            reasons.push(format!(
                "{name} is a publication credential or approval authority and is never placed \
                 in the child; every publishing effect is the supervisor's and is recorded"
            ));
            continue;
        }
        variables.insert(name.clone(), value.clone());
    }

    let missing: Vec<&String> = suite
        .0
        .iter()
        .filter(|c| !blueprint.commands.contains(c))
        .collect();
    if !missing.is_empty() {
        for c in &missing {
            reasons.push(format!(
                "the posture omits `{c}`, which this repository's check suite invokes; \
                 refused at plan time rather than discovered mid-attempt, and not covered \
                 by whatever happens to be on this machine"
            ));
        }
        return ChildEnvironment {
            variables,
            commands: blueprint.commands.clone(),
            state: EnvironmentState::Refused { reasons },
        };
    }

    let state = if reasons.is_empty() {
        EnvironmentState::Applied
    } else {
        EnvironmentState::Degraded { reasons }
    };

    ChildEnvironment {
        variables,
        commands: blueprint.commands.clone(),
        state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_child_gets_exactly_what_the_blueprint_allows_and_nothing_inherited() {
        // The parent process has a populated environment; a filtered approach
        // would carry some of it through. A constructed one carries exactly the
        // blueprint, so the count is the assertion.
        let parent_names: Vec<String> = std::env::vars().map(|(k, _)| k).collect();
        assert!(
            parent_names.len() > 1,
            "this test is only meaningful with a populated parent environment"
        );

        let env = construct(
            &Blueprint::empty().allowing("PATH", "/usr/bin"),
            &CheckSuiteCommands::default(),
        );
        assert_eq!(env.variables.len(), 1);
        assert_eq!(
            env.variables.get("PATH").map(String::as_str),
            Some("/usr/bin")
        );
        for name in parent_names {
            if name != "PATH" {
                assert!(
                    !env.variables.contains_key(&name),
                    "{name} leaked from the parent into a constructed environment"
                );
            }
        }
    }

    #[test]
    fn a_publication_credential_is_withheld_and_the_degradation_is_named() {
        let env = construct(
            &Blueprint::empty()
                .allowing("PATH", "/usr/bin")
                .allowing("GITHUB_TOKEN", "secret"),
            &CheckSuiteCommands::default(),
        );
        assert!(!env.variables.contains_key("GITHUB_TOKEN"));
        match env.state {
            EnvironmentState::Degraded { reasons } => {
                assert!(reasons[0].contains("GITHUB_TOKEN"));
                assert!(reasons[0].contains("never placed"));
            }
            other => panic!("expected degraded, got {other:?}"),
        }
    }

    #[test]
    fn a_token_with_a_prefix_match_is_withheld_too() {
        let env = construct(
            &Blueprint::empty().allowing("AWS_SECRET_ACCESS_KEY", "x"),
            &CheckSuiteCommands::default(),
        );
        assert!(env.variables.is_empty());
    }

    #[test]
    fn a_posture_omitting_a_check_suite_command_is_refused_naming_the_command() {
        let env = construct(
            &Blueprint::empty().needing_command("git"),
            &CheckSuiteCommands(vec!["git".into(), "cargo".into(), "make".into()]),
        );
        match env.state {
            EnvironmentState::Refused { reasons } => {
                assert!(reasons.iter().any(|r| r.contains("`cargo`")));
                assert!(reasons.iter().any(|r| r.contains("`make`")));
            }
            other => panic!("expected refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_posture_declaring_every_command_applies() {
        let env = construct(
            &Blueprint::empty()
                .needing_command("git")
                .needing_command("cargo"),
            &CheckSuiteCommands(vec!["git".into(), "cargo".into()]),
        );
        assert_eq!(env.state, EnvironmentState::Applied);
    }

    #[test]
    fn the_residuals_are_four_and_are_part_of_the_api() {
        assert_eq!(RESIDUALS.len(), 4);
        assert!(RESIDUALS.iter().any(|r| r.contains("keychain")));
        assert!(RESIDUALS.iter().any(|r| r.contains("leaves no record")));
    }
}
