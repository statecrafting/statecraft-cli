//! Witness posture and policy, spec 013 section 3.7.
//!
//! Every attempt created under spec 013 records exactly one
//! [`WitnessPosture`]. An attempt that predates it has no field, and readers
//! render that as [`WitnessPosture::NotRecorded`], never as
//! `witness-not-requested`. A new attempt never records `not-recorded`.
//!
//! Policy, not the producer, decides whether a posture refuses acceptance:
//! [`Requirement::refuses_acceptance`] is that one rule.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Section 3.7's closed vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WitnessPosture {
    /// The bound sidecar finished, required artifacts passed integrity, and
    /// capture is complete. Not acceptance.
    Witnessed,
    /// Neither the task nor policy asked for witness evidence.
    WitnessNotRequested,
    /// Requested, but the exact producer or the platform mechanism was
    /// unavailable before child spawn.
    WitnessUnavailable,
    /// A started sidecar or its protocol failed, including an exit before
    /// `ready`.
    WitnessFailed,
    /// A terminal result names incomplete or absent capture, or gaps.
    WitnessIncomplete,
    /// Policy required witness evidence, the task did not request capture,
    /// and no evidence exists.
    WitnessRequiredAndMissing,
    /// Evidence exists, but an identity, confinement, integrity or
    /// qualification fact is not one policy admits.
    WitnessUnqualified,
    /// The attempt predates spec 013. Compatibility rendering only.
    NotRecorded,
}

impl WitnessPosture {
    /// The recorded word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Witnessed => "witnessed",
            Self::WitnessNotRequested => "witness-not-requested",
            Self::WitnessUnavailable => "witness-unavailable",
            Self::WitnessFailed => "witness-failed",
            Self::WitnessIncomplete => "witness-incomplete",
            Self::WitnessRequiredAndMissing => "witness-required-and-missing",
            Self::WitnessUnqualified => "witness-unqualified",
            Self::NotRecorded => "not-recorded",
        }
    }

    /// Read a recorded posture member. A missing member is `not-recorded`;
    /// an unrecognised one is also refused as a posture rather than guessed.
    #[must_use]
    pub fn from_record(member: Option<&Value>) -> Self {
        member
            .and_then(|v| serde_json::from_value::<Self>(v.clone()).ok())
            .unwrap_or(Self::NotRecorded)
    }
}

/// What the task and policy asked of witness evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    /// The task asked for capture, so a sidecar is started.
    pub requested: bool,
    /// Task or acceptance policy requires admissible witness evidence.
    pub required: bool,
}

impl Requirement {
    /// Optional by default: nothing requested, nothing required.
    pub const DEFAULT: Self = Self {
        requested: false,
        required: false,
    };

    /// Whether `posture` refuses acceptance under this requirement.
    ///
    /// Only a requirement refuses, and it refuses everything except
    /// `witnessed`. An optional witness never refuses by its absence.
    #[must_use]
    pub fn refuses_acceptance(self, posture: WitnessPosture) -> bool {
        self.required && posture != WitnessPosture::Witnessed
    }
}

/// What was observed of one attempt's witness, in order of occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observed {
    /// No sidecar was started because none was requested.
    NotStarted,
    /// The producer or platform mechanism was unavailable before spawn.
    Unavailable,
    /// The sidecar started and failed: no `ready`, a protocol failure, no
    /// terminal message, or a non-finished termination.
    Failed,
    /// A terminal message reported incomplete or absent capture.
    Incomplete,
    /// A terminal message reported complete capture.
    Complete {
        /// The manifest hashed to the reported digest.
        intact: bool,
        /// Producer, interface, confinement and qualification identities are
        /// all ones policy admits.
        qualified: bool,
    },
}

/// The one posture an attempt records.
#[must_use]
pub fn derive(requirement: Requirement, observed: Observed) -> WitnessPosture {
    match (requirement.requested, observed) {
        (false, _) | (true, Observed::NotStarted) => {
            if requirement.required {
                WitnessPosture::WitnessRequiredAndMissing
            } else {
                WitnessPosture::WitnessNotRequested
            }
        }
        (true, Observed::Unavailable) => WitnessPosture::WitnessUnavailable,
        (true, Observed::Failed) => WitnessPosture::WitnessFailed,
        (true, Observed::Incomplete) => WitnessPosture::WitnessIncomplete,
        (true, Observed::Complete { intact, qualified }) => {
            if intact && qualified {
                WitnessPosture::Witnessed
            } else {
                WitnessPosture::WitnessUnqualified
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPTIONAL: Requirement = Requirement {
        requested: true,
        required: false,
    };
    const REQUIRED: Requirement = Requirement {
        requested: true,
        required: true,
    };

    #[test]
    fn an_attempt_without_the_field_is_not_recorded_never_not_requested() {
        assert_eq!(
            WitnessPosture::from_record(None),
            WitnessPosture::NotRecorded
        );
        assert_eq!(
            WitnessPosture::from_record(Some(&Value::from("witness-not-requested"))),
            WitnessPosture::WitnessNotRequested
        );
        assert_eq!(
            WitnessPosture::from_record(Some(&Value::from("witnessed-ish"))),
            WitnessPosture::NotRecorded
        );
    }

    #[test]
    fn every_word_round_trips() {
        for posture in [
            WitnessPosture::Witnessed,
            WitnessPosture::WitnessNotRequested,
            WitnessPosture::WitnessUnavailable,
            WitnessPosture::WitnessFailed,
            WitnessPosture::WitnessIncomplete,
            WitnessPosture::WitnessRequiredAndMissing,
            WitnessPosture::WitnessUnqualified,
            WitnessPosture::NotRecorded,
        ] {
            let value = serde_json::to_value(posture).unwrap();
            assert_eq!(value, Value::from(posture.word()));
            assert_eq!(WitnessPosture::from_record(Some(&value)), posture);
        }
    }

    #[test]
    fn absence_refuses_only_when_required() {
        let unrequested = derive(Requirement::DEFAULT, Observed::NotStarted);
        assert_eq!(unrequested, WitnessPosture::WitnessNotRequested);
        assert!(!Requirement::DEFAULT.refuses_acceptance(unrequested));

        let policy_only = Requirement {
            requested: false,
            required: true,
        };
        let missing = derive(policy_only, Observed::NotStarted);
        assert_eq!(missing, WitnessPosture::WitnessRequiredAndMissing);
        assert!(policy_only.refuses_acceptance(missing));
    }

    #[test]
    fn a_started_sidecar_that_fails_is_failed_never_unavailable() {
        assert_eq!(
            derive(OPTIONAL, Observed::Failed),
            WitnessPosture::WitnessFailed
        );
        assert_eq!(
            derive(REQUIRED, Observed::Failed),
            WitnessPosture::WitnessFailed
        );
        assert_eq!(
            derive(OPTIONAL, Observed::Unavailable),
            WitnessPosture::WitnessUnavailable
        );
        assert!(!OPTIONAL.refuses_acceptance(WitnessPosture::WitnessFailed));
        assert!(REQUIRED.refuses_acceptance(WitnessPosture::WitnessFailed));
    }

    #[test]
    fn complete_capture_is_witnessed_only_when_intact_and_qualified() {
        let complete = |intact, qualified| Observed::Complete { intact, qualified };
        assert_eq!(
            derive(REQUIRED, complete(true, true)),
            WitnessPosture::Witnessed
        );
        assert_eq!(
            derive(REQUIRED, complete(false, true)),
            WitnessPosture::WitnessUnqualified
        );
        assert_eq!(
            derive(REQUIRED, complete(true, false)),
            WitnessPosture::WitnessUnqualified
        );
        assert_eq!(
            derive(OPTIONAL, Observed::Incomplete),
            WitnessPosture::WitnessIncomplete
        );
        assert!(!REQUIRED.refuses_acceptance(WitnessPosture::Witnessed));
    }
}
