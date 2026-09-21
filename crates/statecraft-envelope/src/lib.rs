//! The shared evidence envelope: the wire types two products exchange.
//!
//! This crate is the implementation of
//! [spec 005](../../../specs/005-acceptance-and-evidence/spec.md), and it is
//! the **one owner** of the reference, the four evidence dimensions, admission
//! and the three names for absence. `statecraft-acceptance` re-exports them
//! rather than declaring them a second time, and the statecraft platform
//! depends on this crate rather than carrying a copy.
//!
//! It was written in the platform repository and transferred here; `PROVENANCE.md`
//! records where it came from, under what licence, and what changed on the way.
//! Its contents derive from hqgit's format descriptions (011, 013, 017, 019,
//! 020, 027, 064 at `e4450f6`), from the platform's specs 003 and 004, and from
//! statecraft-cli spec 005, whose serializations it preserves byte for byte:
//! `tests/cli_compat.rs` is that claim's evidence.
//!
//! Nothing here reads a clock, the environment or the network, and nothing
//! here executes anything: every function is a pure function of its
//! arguments. The one place a signature is produced is [`sign::Signer`].

#![forbid(unsafe_code)]

pub mod absence;
pub mod admission;
pub mod attestation;
pub mod cbor;
pub mod dimensions;
pub mod entry;
pub mod error;
pub mod fact;
pub mod hash;
pub mod hlc;
pub mod portable;
pub mod reference;
pub mod roots;
pub mod sign;
pub mod value;
pub mod verdict;

pub use error::Error;

/// The name this crate reports as the verifier (spec 003 B-17).
pub const VERIFIER_NAME: &str = "statecraft-envelope";
/// The version this crate reports as the verifier.
pub const VERIFIER_VERSION: &str = env!("CARGO_PKG_VERSION");
