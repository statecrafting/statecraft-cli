//! What an adapter declares about itself, and what qualifies a binary version.
//!
//! Spec 004 sections 3.2 and 3.4. An adapter's manifest lists the tokens it
//! supports. A **binary version** is qualified only by a recorded pass of the
//! negative suite, and a record for a different version does not transfer.
//!
//! An adapter with no qualification record **runs**, labelled `unqualified`
//! everywhere it appears. Refusing it would make adding a provider impossible;
//! hiding it would make constitution XI a slogan.

use crate::capability::Capability;
use serde::{Deserialize, Serialize};

/// An adapter's declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// The adapter's name. Not a provider's name: see this crate's own test.
    pub adapter: String,
    /// The binary version this manifest describes.
    pub version: String,
    /// The tokens it supports.
    pub supports: Vec<Capability>,
    /// Commands the run will need present in the constructed environment.
    ///
    /// Section 3.5.8: the posture declares every command the run will need, and
    /// one the check suite invokes but the posture omits is refused at plan
    /// time, naming the command.
    #[serde(default)]
    pub requires_commands: Vec<String>,
}

/// A recorded pass of the negative suite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualificationRecord {
    /// The adapter this qualifies.
    pub adapter: String,
    /// The **binary version** it qualifies, and only that one.
    pub binary_version: String,
    /// The version of the suite that was run.
    pub suite_version: String,
    /// When, as an RFC 3339 UTC timestamp.
    pub date: String,
}

/// Whether an adapter binary is qualified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Qualification {
    /// A recorded pass exists for exactly this binary version.
    Qualified,
    /// No record for this binary version. The adapter still runs.
    Unqualified,
}

impl Qualification {
    /// The word this is labelled with, in posture, attempt and outcome alike.
    pub fn word(self) -> &'static str {
        match self {
            Qualification::Qualified => "qualified",
            Qualification::Unqualified => "unqualified",
        }
    }
}

/// Decide an adapter binary's qualification from the records on hand.
///
/// Matching is on adapter name **and** binary version. A record for a different
/// version does not transfer, and neither does a sibling adapter's pass, however
/// similar its prompts, skills or configuration files.
pub fn qualification(manifest: &Manifest, records: &[QualificationRecord]) -> Qualification {
    let found = records
        .iter()
        .any(|r| r.adapter == manifest.adapter && r.binary_version == manifest.version);
    if found {
        Qualification::Qualified
    } else {
        Qualification::Unqualified
    }
}

/// A discrepancy between what a manifest declared and what an init event applied.
///
/// Section 3.5.6 and section 3.8: a manifest declaring a token the adapter does
/// not honor **fails qualification**, and the applied set is recorded as
/// observed rather than as declared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discrepancy {
    /// Tokens declared but not applied.
    pub declared_not_applied: Vec<Capability>,
}

impl Discrepancy {
    /// Whether anything was declared and not honored.
    pub fn any(&self) -> bool {
        !self.declared_not_applied.is_empty()
    }

    /// A one-line rendering for a report.
    pub fn describe(&self, manifest: &Manifest) -> String {
        format!(
            "adapter {} {} declares [{}] which its init event did not apply; \
             the applied set is recorded as observed and the binary fails qualification",
            manifest.adapter,
            manifest.version,
            self.declared_not_applied
                .iter()
                .map(|c| c.token())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// Compare what was declared with what the init event says was applied.
///
/// Only the tokens that were actually asked for are compared: a manifest may
/// legitimately support more than a given run requested, and calling that a
/// discrepancy would fail every adapter that is better than its request.
pub fn discrepancy(granted: &[Capability], applied: &[Capability]) -> Discrepancy {
    Discrepancy {
        declared_not_applied: granted
            .iter()
            .filter(|c| !applied.contains(c))
            .copied()
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(version: &str) -> Manifest {
        Manifest {
            adapter: "fixture".into(),
            version: version.into(),
            supports: vec![Capability::TurnLimit],
            requires_commands: vec![],
        }
    }

    fn record(adapter: &str, version: &str) -> QualificationRecord {
        QualificationRecord {
            adapter: adapter.into(),
            binary_version: version.into(),
            suite_version: "1".into(),
            date: "2026-09-16T00:00:00Z".into(),
        }
    }

    #[test]
    fn an_adapter_with_no_record_is_unqualified_and_still_runs() {
        assert_eq!(
            qualification(&manifest("1.0.0"), &[]),
            Qualification::Unqualified
        );
        // "Still runs" is the absence of a refusal here: nothing in this module
        // can refuse, which is the implementation of section 3.4.
    }

    #[test]
    fn a_record_for_the_same_binary_version_qualifies() {
        assert_eq!(
            qualification(&manifest("1.0.0"), &[record("fixture", "1.0.0")]),
            Qualification::Qualified
        );
    }

    #[test]
    fn a_record_for_a_different_version_does_not_transfer() {
        assert_eq!(
            qualification(&manifest("1.0.1"), &[record("fixture", "1.0.0")]),
            Qualification::Unqualified
        );
    }

    #[test]
    fn a_siblings_pass_does_not_transfer() {
        assert_eq!(
            qualification(&manifest("1.0.0"), &[record("other-adapter", "1.0.0")]),
            Qualification::Unqualified
        );
    }

    #[test]
    fn declaring_a_token_the_init_event_did_not_apply_is_a_discrepancy() {
        let d = discrepancy(
            &[Capability::TurnLimit, Capability::WorkspaceWrite],
            &[Capability::TurnLimit],
        );
        assert!(d.any());
        assert_eq!(d.declared_not_applied, [Capability::WorkspaceWrite]);
        assert!(
            d.describe(&manifest("1.0.0"))
                .contains("fails qualification")
        );
    }

    #[test]
    fn applying_more_than_was_asked_for_is_not_a_discrepancy() {
        let d = discrepancy(
            &[Capability::TurnLimit],
            &[Capability::TurnLimit, Capability::CostReport],
        );
        assert!(!d.any());
    }
}
