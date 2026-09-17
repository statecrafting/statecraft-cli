//! The three names for absence, and the two kinds of statement.
//!
//! Spec 005 section 3.8. In every reported outcome, absence is one of three, and
//! **none reads as success**. The distinction is the whole reason a reader can
//! trust a report: a blank field could mean anything, and these three cannot be
//! confused with each other or with a pass.

use serde::{Deserialize, Serialize};

/// Why something is not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Absence {
    /// The run recorded that nothing of this kind happened.
    ///
    /// A positive statement: somebody looked, and there was nothing.
    None,
    /// No record of this kind exists.
    ///
    /// Including **every field a future contract will add**. A field that will
    /// exist later is present today reading `not-recorded`, never omitted, so a
    /// record minted before the contract is not silently unanswerable on the
    /// point.
    NotRecorded,
    /// A record exists for a revision that is no longer current.
    ///
    /// A reported state, not an error and not a pass.
    Stale,
}

impl Absence {
    /// The word this is written as.
    pub fn word(self) -> &'static str {
        match self {
            Absence::None => "none",
            Absence::NotRecorded => "not-recorded",
            Absence::Stale => "stale",
        }
    }

    /// Every name. A test asserts there are exactly three.
    pub fn all() -> [Absence; 3] {
        [Absence::None, Absence::NotRecorded, Absence::Stale]
    }
}

/// A value that may be absent, carrying which kind of absence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", untagged)]
pub enum Recorded<T> {
    /// A value was recorded.
    Present(T),
    /// It was not, and this says which kind of absence that is.
    Absent(Absence),
}

impl<T> Recorded<T> {
    /// Whether a value is actually here.
    pub fn is_present(&self) -> bool {
        matches!(self, Recorded::Present(_))
    }

    /// The value, if there is one.
    pub fn value(&self) -> Option<&T> {
        match self {
            Recorded::Present(v) => Some(v),
            Recorded::Absent(_) => None,
        }
    }

    /// The kind of absence, if absent.
    pub fn absence(&self) -> Option<Absence> {
        match self {
            Recorded::Present(_) => None,
            Recorded::Absent(a) => Some(*a),
        }
    }
}

/// Where a statement came from.
///
/// Section 3.8: a narrative supplied by a model is labelled `narrative` and is
/// **kept apart** from what a machine observed, which is labelled with the
/// record it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "source")]
pub enum Statement {
    /// A model said this. It is a claim, and the field is named for a claim.
    Narrative {
        /// What was said.
        text: String,
    },
    /// A machine observed this, and the record it came from is named.
    Observed {
        /// Which record.
        record: String,
        /// What it said.
        text: String,
    },
}

impl Statement {
    /// Whether this is something a machine observed rather than something a
    /// model asserted.
    pub fn is_observed(&self) -> bool {
        matches!(self, Statement::Observed { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_exactly_three_names_for_absence() {
        assert_eq!(Absence::all().len(), 3);
    }

    #[test]
    fn no_name_for_absence_reads_as_a_pass() {
        for a in Absence::all() {
            assert_ne!(a.word(), "pass");
            assert_ne!(a.word(), "ok");
            assert_ne!(a.word(), "");
        }
    }

    #[test]
    fn a_future_field_is_present_reading_not_recorded_rather_than_omitted() {
        let field: Recorded<String> = Recorded::Absent(Absence::NotRecorded);
        let json = serde_json::to_string(&field).unwrap();
        assert_eq!(json, "\"not-recorded\"");
        assert!(!field.is_present());
    }

    #[test]
    fn a_narrative_is_kept_apart_from_an_observation() {
        let claim = Statement::Narrative {
            text: "I finished the work".into(),
        };
        let observed = Statement::Observed {
            record: "suite-result".into(),
            text: "3 of 4 checks passed".into(),
        };
        assert!(!claim.is_observed());
        assert!(observed.is_observed());
        assert!(serde_json::to_string(&claim).unwrap().contains("narrative"));
    }
}
