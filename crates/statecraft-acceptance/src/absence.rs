//! The three names for absence, and the two kinds of statement.
//!
//! Spec 005 section 3.8. In every reported outcome, absence is one of three, and
//! **none reads as success**. The distinction is the whole reason a reader can
//! trust a report: a blank field could mean anything, and these three cannot be
//! confused with each other or with a pass.
//!
//! # Where these types live now
//!
//! [`Absence`] and [`Recorded`] are defined in `statecraft-envelope` and
//! re-exported here, so the name a reader of this crate uses is unchanged.
//! Spec 007 moved them because the platform reads and writes the same bytes: a
//! second definition of a wire type is a second answer to the same question,
//! and the two answers had already diverged on how `"not-recorded"` reads.
//!
//! The envelope's [`Recorded`] carries the resolution of that divergence: the
//! three words are reserved, a present value that would serialize to one is
//! refused rather than written, and every reader takes a reserved word as the
//! absence. The encoding is byte for byte what this crate wrote before;
//! `statecraft-envelope::absence` documents the contract and how records
//! written under the old reading decode.
//!
//! [`Statement`] stays here. It is spec 005's own vocabulary, nothing outside
//! this product writes it, and it is not part of the shared envelope.

use serde::{Deserialize, Serialize};

pub use statecraft_envelope::absence::{
    Absence, RESERVED_WORDS, Recordable, Recorded, ReservedValue,
};

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

/// A statement is a tagged object on the wire, so it can never be mistaken for
/// one of the reserved absence words.
impl Recordable for Statement {}

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

    #[test]
    fn a_statement_survives_the_recorded_wrapper_in_both_directions() {
        let r = Recorded::present(Statement::Narrative {
            text: "all done".into(),
        })
        .unwrap();
        let text = serde_json::to_string(&r).unwrap();
        assert_eq!(text, r#"{"source":"narrative","text":"all done"}"#);
        assert_eq!(
            serde_json::from_str::<Recorded<Statement>>(&text).unwrap(),
            r
        );
        assert_eq!(
            serde_json::from_str::<Recorded<Statement>>("\"not-recorded\"").unwrap(),
            Recorded::Absent(Absence::NotRecorded)
        );
    }

    #[test]
    fn a_harness_revision_named_after_an_absence_is_refused_rather_than_written() {
        // The collision spec 007 closes, at the field that motivated the type.
        assert!(Recorded::<String>::present("not-recorded".into()).is_err());
        assert!(Recorded::<String>::present("harness-2026.09".into()).is_ok());
    }
}
