//! Independent acceptance, the receipt, and the separately reported evidence.
//!
//! This crate is the implementation of
//! [spec 005](../../../specs/005-acceptance-and-evidence/spec.md).
//!
//! # The shape of it
//!
//! - [`judged`] identifies the candidate, the base and the policy, and says
//!   which attempt outcomes are even eligible for acceptance.
//! - [`independence`] runs the suite this product owns: a claim is a claim, a
//!   structured report beats an exit code, and an unrun check is `unknown`.
//! - [`authority`] applies the authority-set rule, and refuses to build the
//!   change classifier that belongs to spec-spine.
//! - [`delta`] reads spec-spine's change-classification report (its spec 088)
//!   and maps its structural classes onto the authority-set members `001`
//!   section 3.5 enumerates.
//! - [`receipt`] mints a receipt, or says precisely why it did not.
//! - [`dimensions`] is the four evidence dimensions and the admission decision
//!   kept apart from them.
//! - [`trust`] judges issuers against roots supplied independently of the
//!   evidence.
//! - [`evidence`] preserves bytes, digests and the construction that produced
//!   them.
//! - [`absence`] is the three names for absence, none of which reads as success.
//! - [`outcome`] folds the reviewable account.
//! - [`suite`] runs the declared acceptance and folds one run's account out of
//!   the record. Added by spec 009's additive edge: a command may not be a
//!   second implementation, so the entry point a binding needs lives here.
//!
//! # What it does not do
//!
//! **Nothing here publishes**, and nothing here authorizes a publication. A
//! receipt is evidence that a specific suite passed over specific bytes under a
//! specific policy; it is not a permission. Section 3.9 is explicit, and the
//! absence of any verb that acts is how this crate keeps to it.
//!
//! It also builds no change classifier. Classifying a change under the base's
//! rules is spec-spine's job; its spec 088 is released, the pin carries it, and
//! this product now **reads** that report rather than answering the question
//! itself. What it still does not do is obtain the report: `001` section 3.5
//! requires every authority-set member to be read at the trusted base revision,
//! and a candidate that chose its own classifier would classify itself, so the
//! bytes are handed in by the caller. Where there is no usable report the
//! corpus-side verdict reads `not-recorded` and acceptance is refused rather
//! than guessed. Refusing without the report is available; classifying without
//! it is not.

#![forbid(unsafe_code)]

pub mod absence;
pub mod authority;
pub mod delta;
pub mod dimensions;
pub mod evidence;
pub mod independence;
pub mod judged;
pub mod outcome;
pub mod receipt;
pub mod suite;
pub mod trust;

pub use absence::{Absence, Recorded, Statement};
pub use authority::{CorpusAnswer, Declared, DeltaReport, Verdict as AuthorityVerdict};
pub use delta::{ClassReading, SpecSpineDeltaReport, reading_of};
pub use dimensions::{Admission, AdmissionPolicy, Dimensions, admit};
pub use evidence::{Construction, Reference};
pub use independence::{Check, SuiteResult};
pub use judged::{Base, Candidate, Judged, NoAcceptance, NotAttemptedReason, Policy};
pub use outcome::{Acceptance, ReviewableOutcome};
pub use receipt::{NoReceipt, Receipt, mint};
pub use trust::{Anchor, RootSet, issuer_trust};
