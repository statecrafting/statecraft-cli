//! The execution adapter seam.
//!
//! This crate is the implementation of
//! [spec 004](../../../specs/004-execution-adapter/spec.md): one protocol,
//! declared capabilities, a qualification suite, and a constructed child
//! environment.
//!
//! # No provider name appears here
//!
//! Spec 004 section 3.8 makes one a defect, and section 2 says the seam's
//! purpose is that it cannot need one. `tests/no_provider_names.rs` greps this
//! crate's own sources, so the rule is mechanical rather than remembered.
//!
//! # The shape of it
//!
//! - [`capability`] is the closed six-token vocabulary, and the rule that a
//!   required token refuses while a preferred one degrades.
//! - [`protocol`] is the request, the typed event stream and the result. An
//!   absent cost is `unknown`, never zero.
//! - [`manifest`] is what an adapter declares, and what qualifies one **binary
//!   version** and no other.
//! - [`environment`] constructs the child's environment from an allowed set, and
//!   names the residuals that a constructed environment does not close.
//! - [`posture`] is what every attempt reports, so an operator never has to ask.
//! - [`supervisor`] spawns, holds a deadline, and kills descendants.
//! - [`fixture`] is the adapter the negative suite runs against with no real
//!   provider installed.
//!
//! # What this crate does not claim
//!
//! **No isolation.** Section 3.6 is explicit, and so is
//! [`environment::RESIDUALS`]: the supervisor's path is the only publishing path
//! this product *provides*, which is a statement about what it hands the child
//! and not about what the child can reach. No document in this repository may
//! say the child cannot publish.

#![forbid(unsafe_code)]

pub mod capability;
pub mod environment;
pub mod fixture;
pub mod manifest;
pub mod posture;
pub mod protocol;
pub mod supervisor;

pub use capability::{Capability, Negotiation, Requested, negotiate};
pub use environment::{Blueprint, ChildEnvironment, EnvironmentState, RESIDUALS, construct};
pub use manifest::{Manifest, Qualification, QualificationRecord, qualification};
pub use posture::Posture;
pub use protocol::{AdapterResult, Classification, Event, Request, StreamError};
pub use supervisor::{SpawnRefusal, Supervised, preflight, supervise};
