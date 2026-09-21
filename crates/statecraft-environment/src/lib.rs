//! Project registration and the managed working environment.
//!
//! This crate is the implementation of
//! [spec 002](../../../specs/002-environment-lifecycle/spec.md): how a
//! repository becomes a target this product may work in, and how the product
//! installs and maintains the working environment inside it.
//!
//! # The shape of it
//!
//! - [`registry`] records targets and their read-only [`qualify`] verdict.
//!   Registering writes nothing inside the target, and arming is a separate act.
//! - [`manifest`] is the committed record of every byte this product owns in a
//!   target, plus the project declaration and the lines this product owns inside
//!   files it does not (spec 002). A path it does not mention is `user` class,
//!   which is what makes the three ownership classes exhaustive by construction.
//! - [`adapter`] is what an agent-harness adapter declares, and when it refuses.
//! - [`plan`] computes what an apply would do; [`apply`] performs it and can
//!   remove it again.
//! - [`doctor`] diagnoses and never repairs.
//!
//! # What it does not do
//!
//! There is no binary here. The commands spec 002 names (`project register`,
//! `env plan`, `env apply`, `env upgrade`, `env remove`, `doctor`) are the
//! operator's vocabulary; wiring them to a process belongs to a spec that owns a
//! binary crate, and no such spec exists yet.
//!
//! It also answers no specification question itself. "Does this corpus compile?"
//! is asked of `spec-spine` and its exit status believed ([`probe`]), because
//! spec 001 makes specification semantics spec-spine's and forbids this product
//! from growing a second compiler or reading `.derived/` behind its back.

#![forbid(unsafe_code)]

pub mod adapter;
pub mod apply;
pub mod claimant;
pub mod digest;
pub mod doctor;
pub mod manifest;
pub mod plan;
pub mod probe;
pub mod qualify;
pub mod registry;
pub mod time;

pub use apply::{Outcome, apply, remove};
pub use doctor::{Report, doctor};
pub use manifest::{
    Class, Enrollment, Entry, Manifest, Modification, ModificationKind, Pins, Project,
};
pub use plan::{Plan, plan};
pub use qualify::{Qualification, Verdict};
pub use registry::{Registration, Registry};
