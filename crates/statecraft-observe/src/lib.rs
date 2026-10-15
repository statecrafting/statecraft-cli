//! Observation and proposals (spec 027).
//!
//! One observation reads exactly one commit through spec-spine's structured
//! reports, reduces the answers to three closed projections
//! ([`projection`]), and compares them with the last observation recorded for
//! the repository ([`transition`]). Each transition a built-in subscription
//! names ([`subscription`]) becomes a proposal with a deterministic identity,
//! recorded in an append-only chain ([`proposal`]). Nothing in this crate runs
//! a proposal: admission is an operator act, and it enters spec 003's run path.
//!
//! The modules here are pure functions of values (section 3.10), so the
//! comparison, matching and proposal logic is tested without git, spec-spine
//! or a product home.

pub mod observe;
pub mod projection;
pub mod proposal;
pub mod proposals;
pub mod store;
pub mod subscription;
pub mod transition;
