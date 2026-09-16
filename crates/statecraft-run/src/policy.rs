//! The lifecycle policy: which statuses a repository schedules.
//!
//! Spec 003 section 3.1.1. spec-spine's readiness answer and this product's
//! scheduling decision are different questions. `registry plan` offers a spec
//! whose status is `draft`, because `draft` plus `pending` is schedulable in the
//! lifecycle table; `draft` withholds ratification, not schedulability.
//!
//! **The policy belongs to the target repository, not to this product's state.**
//! If this product kept the answer in its own registry, two tools would answer
//! "may this draft build" differently, and the one a human reads would not be
//! the one that acts.
//!
//! How a repository spells its declaration is deliberately **not decided here**.
//! spec-spine's design note 06 is explicit that a machine-readable schema is one
//! candidate among several and that nothing should be read as having chosen one.
//! This module fixes where the answer comes from and who may override it; until
//! the format is filed upstream, [`PolicySource::Defaulted`] is what a real
//! target produces and the seam is [`PolicyDeclaration`].

use serde::{Deserialize, Serialize};

/// Where the policy in force came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicySource {
    /// The target declared it. The declaration wins over anything this product
    /// holds.
    Declared {
        /// How the target spelled it, verbatim, for the record.
        as_written: String,
    },
    /// The target declared nothing, so the default applies. An attempt records
    /// that the policy was **defaulted, not declared**, which is the difference
    /// between "this repository agreed to schedule approved specs" and "nobody
    /// said, so we assumed the strict answer".
    Defaulted,
}

/// A repository's lifecycle policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    /// The statuses this repository schedules.
    pub schedulable_statuses: Vec<String>,
    /// Where it came from.
    pub source: PolicySource,
}

impl Policy {
    /// The default: `approved` only.
    pub fn default_policy() -> Self {
        Self {
            schedulable_statuses: vec!["approved".to_string()],
            source: PolicySource::Defaulted,
        }
    }

    /// Whether this policy admits a status.
    pub fn admits(&self, status: &str) -> bool {
        self.schedulable_statuses.iter().any(|s| s == status)
    }

    /// Whether the target declared this policy, as opposed to it being assumed.
    pub fn declared(&self) -> bool {
        matches!(self.source, PolicySource::Declared { .. })
    }
}

/// How a target's declaration is found, if it makes one.
///
/// The seam that keeps the format decision upstream. The shipped implementation
/// finds nothing, which is correct: no format is filed, and inventing one here
/// is what section 3.1.1 forbids in as many words.
pub trait PolicyDeclaration {
    /// The policy the target declares, if any.
    fn declared_policy(&self, target: &std::path::Path) -> Option<Policy>;
}

/// The declaration reader that reads nothing, because nothing is filed yet.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoDeclarationFiled;

impl PolicyDeclaration for NoDeclarationFiled {
    fn declared_policy(&self, _target: &std::path::Path) -> Option<Policy> {
        None
    }
}

/// A declaration reader backed by an explicit value, for tests and for a caller
/// that has resolved the target's declaration by some other means.
#[derive(Debug, Clone)]
pub struct StaticDeclaration(pub Option<Policy>);

impl PolicyDeclaration for StaticDeclaration {
    fn declared_policy(&self, _target: &std::path::Path) -> Option<Policy> {
        self.0.clone()
    }
}

/// An operator's explicit admission of one named spec in one repository.
///
/// Section 3.1.1 point 2: operator-initiated, journaled, and surfaced on every
/// attempt it admits. **Never inferred from the spec being offered as ready.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Override {
    /// The spec id this admits, and only this one.
    pub spec_id: String,
    /// Who asked for it.
    pub operator: String,
    /// Why, recorded so the attempt can surface it.
    pub reason: String,
}

/// The overrides in force for one repository.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Overrides {
    /// One per admitted spec id.
    #[serde(default)]
    pub entries: Vec<Override>,
}

impl Overrides {
    /// No overrides.
    pub fn none() -> Self {
        Self::default()
    }

    /// Record an override.
    #[must_use]
    pub fn admitting(mut self, spec_id: &str, operator: &str, reason: &str) -> Self {
        self.entries.push(Override {
            spec_id: spec_id.to_string(),
            operator: operator.to_string(),
            reason: reason.to_string(),
        });
        self
    }

    /// The override for a spec id, if one exists.
    pub fn for_spec(&self, spec_id: &str) -> Option<&Override> {
        self.entries.iter().find(|o| o.spec_id == spec_id)
    }
}

/// Resolve the policy in force for a target.
///
/// The repository's declaration wins. When this product also holds a policy of
/// its own, the disagreement is reported and the declaration is still what
/// applies: the product never prefers its own copy.
pub fn resolve(
    target: &std::path::Path,
    declaration: &dyn PolicyDeclaration,
    product_held: Option<&Policy>,
) -> (Policy, Option<String>) {
    match declaration.declared_policy(target) {
        Some(declared) => {
            let disagreement = product_held.and_then(|held| {
                if held.schedulable_statuses != declared.schedulable_statuses {
                    Some(format!(
                        "the target declares [{}] and this product holds [{}]; \
                         the target's declaration is what applies",
                        declared.schedulable_statuses.join(", "),
                        held.schedulable_statuses.join(", ")
                    ))
                } else {
                    None
                }
            });
            (declared, disagreement)
        }
        None => (Policy::default_policy(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn declared(statuses: &[&str]) -> Policy {
        Policy {
            schedulable_statuses: statuses.iter().map(|s| s.to_string()).collect(),
            source: PolicySource::Declared {
                as_written: statuses.join(","),
            },
        }
    }

    #[test]
    fn the_default_admits_approved_only_and_records_that_it_was_defaulted() {
        let p = Policy::default_policy();
        assert!(p.admits("approved"));
        assert!(!p.admits("draft"));
        assert!(!p.declared());
        assert_eq!(p.source, PolicySource::Defaulted);
    }

    #[test]
    fn a_target_with_no_declaration_gets_the_default() {
        let (p, disagreement) = resolve(Path::new("/x"), &NoDeclarationFiled, None);
        assert_eq!(p.source, PolicySource::Defaulted);
        assert!(disagreement.is_none());
    }

    #[test]
    fn the_targets_declaration_wins_over_what_the_product_holds() {
        let target_says = declared(&["approved", "draft"]);
        let product_holds = Policy {
            schedulable_statuses: vec!["approved".into()],
            source: PolicySource::Defaulted,
        };
        let (p, disagreement) = resolve(
            Path::new("/x"),
            &StaticDeclaration(Some(target_says.clone())),
            Some(&product_holds),
        );
        assert_eq!(p, target_says);
        let d = disagreement.expect("the disagreement is reported");
        assert!(d.contains("the target's declaration is what applies"));
    }

    #[test]
    fn agreement_reports_nothing() {
        let both = declared(&["approved"]);
        let (_, disagreement) = resolve(
            Path::new("/x"),
            &StaticDeclaration(Some(both.clone())),
            Some(&both),
        );
        assert!(disagreement.is_none());
    }

    #[test]
    fn an_override_names_one_spec_and_only_that_spec() {
        let o = Overrides::none().admitting("003-x", "bart", "urgent");
        assert!(o.for_spec("003-x").is_some());
        assert!(o.for_spec("004-y").is_none());
    }
}
