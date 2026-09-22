//! The Statecraft-managed environment.
//!
//! This crate is the implementation of
//! [spec 002](../../../specs/002-environment-lifecycle/spec.md), sections 3.11 to 3.21:
//! one global environment, one per-project area, one initialization flow, and
//! where a configuration value's authority comes from.
//!
//! # Why it exists
//!
//! spec-spine withdrew its kit and its public initializer, so spec 002 section
//! 3.7's transition contract has no second installer to coexist with and this
//! product is the only user-facing initializer. That answers the open question
//! inside `D-04` by the other side of it disappearing, and it hands this
//! repository obligations the corpus had not written down: the global
//! environment an operator has, the project area a repository gets, the
//! reusable harness that must not be copied into every repository, the
//! governance starter files that now come from a library rather than a
//! command, the user's own instruction file this product must not take over,
//! and which configuration layer is allowed to decide what.
//!
//! # The shape of it
//!
//! - [`home`] is the global environment: where it is, what it holds, and the
//!   rule that this crate extends it rather than replacing what is there.
//! - [`harness`] is the one canonical, content-addressed harness source.
//! - [`delivery`] is what actually reaches a session: native links into an
//!   agent's own home, and an **evaluated** verdict on whether the managed
//!   instructions are reachable at all.
//! - [`project`] is the per-project area; [`bridge`] is the one line of the
//!   user's own instruction file this product owns; [`ignore`] merges the
//!   producer's ignore fragment without touching an unrelated entry.
//! - [`producer`] is the governance boundary: the spec-spine library, called
//!   as a library, with a closed contract set of paths this product will place.
//! - [`authority`] resolves configuration with provenance; [`team`] is the solo
//!   and team boundary and the coordination-authority seam; [`resolved`]
//!   freezes a run's resolution so a global upgrade cannot reach into it.
//! - [`derived`] is the one-time relocation of the compiled artifacts.
//! - [`flow`] is the initialization; [`service`] is the typed operation
//!   boundary the command surface and a future dashboard both call.
//!
//! # What it does not do
//!
//! There is no binary here, and no hosted platform. Spec 006 owns the command
//! surface and these verbs are bindings inside that crate. The platform is
//! a trait with one shipped implementation that reaches nothing and reports
//! `unavailable`: this repository implements the local boundary and the honest
//! unavailable states, and nothing here presents a fixture as a live
//! integration.
//!
//! It also answers no specification question itself. "Does this corpus
//! compile?" is asked of `spec-spine` and its exit status believed, which is
//! spec 001 section 3.2.

#![forbid(unsafe_code)]

pub mod admission;
pub mod authority;
pub mod bridge;
pub mod capture;
pub mod delivery;
pub mod derived;
pub mod flow;
pub mod harness;
pub mod home;
pub mod ignore;
pub mod producer;
pub mod project;
pub mod required;
pub mod resolved;
pub mod service;
pub mod session;
pub mod settings;
pub mod startup;
pub mod team;

pub use flow::{Mode, Outcome, Report, Step};
pub use home::{Layout, Personal, Tools, resolve as resolve_home};
pub use service::{Answer, Operation, Ports, Severity, execute};
