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
//! - [`receipt`] mints a receipt, or says precisely why it did not.
//! - [`dimensions`] is the four evidence dimensions and the admission decision
//!   kept apart from them.
//! - [`trust`] judges issuers against roots supplied independently of the
//!   evidence.
//! - [`evidence`] preserves bytes, digests and the construction that produced
//!   them.
//! - [`absence`] is the three names for absence, none of which reads as success.
//! - [`outcome`] folds the reviewable account.
//!
//! # What it does not do
//!
//! **Nothing here publishes**, and nothing here authorizes a publication. A
//! receipt is evidence that a specific suite passed over specific bytes under a
//! specific policy; it is not a permission. Section 3.9 is explicit, and the
//! absence of any verb that acts is how this crate keeps to it.
//!
//! It also builds no change classifier. Classifying a change under the base's
//! rules is spec-spine's job; its spec 088 is released and the pin carries it,
//! and this product does not read the report, so the corpus-side verdict reads
//! `not-recorded` and acceptance is refused rather than guessed. Refusing
//! without the report is available; classifying without it is not.

#![forbid(unsafe_code)]

pub mod absence;
pub mod authority;
pub mod dimensions;
pub mod evidence;
pub mod independence;
pub mod judged;
pub mod outcome;
pub mod receipt;
pub mod trust;

pub use absence::{Absence, Recorded, Statement};
pub use authority::{Declared, Verdict as AuthorityVerdict};
pub use dimensions::{Admission, AdmissionPolicy, Dimensions, admit};
pub use evidence::{Construction, Reference};
pub use independence::{Check, SuiteResult};
pub use judged::{Base, Candidate, Judged, NoAcceptance, NotAttemptedReason, Policy};
pub use outcome::{Acceptance, ReviewableOutcome};
pub use receipt::{NoReceipt, Receipt, mint};
pub use trust::{Anchor, RootSet, issuer_trust};
