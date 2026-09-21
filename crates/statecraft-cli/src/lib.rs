//! The statecraft command surface.
//!
//! This crate is the implementation of
//! [spec 006](../../../specs/006-command-surface/spec.md): the verbs specs 002
//! to 005 name, bound to a process.
//!
//! # A command is a binding, never a second implementation
//!
//! Each function in [`bind`] parses what it was given, calls **exactly one**
//! library operation, and maps the value it returns onto [`exit::Exit`]. None of
//! them contains a rule an owning spec did not state, and the coupling gate can
//! enforce that: this crate is spec 006's territory, so a rule that leaked in
//! would have to be written into 006, where it visibly does not belong.
//!
//! # The distinction a caller scripts against
//!
//! A **finding** (1) is the operation reporting what it found. A **refusal** (2)
//! is a precondition that stopped it. A **failure** (4) is something nobody
//! asked for. A `partial` apply is 1, a removal with no manifest is 2, an
//! unreadable manifest is 4.
//!
//! # The configured adapter set
//!
//! [`adapters`] names it. Spec 008 ratified the first provider adapter, so the
//! environment verbs have an input where they previously had none; what that
//! adapter declares and how its prerequisites are detected is spec 008's, in
//! the adapter's own crate.
//!
//! # The integration slice
//!
//! [`slice`] holds the `work`, `run` and `accept` bindings. Same
//! rule as [`bind`], and spec 006 section 3.11 sharpens it: where a verb needed
//! something an owning library did not expose, the entry point was added there.
//!
//! # The managed environment
//!
//! [`manage`] holds the bindings spec 010 added: the global home, the one
//! initialization flow, the one-time relocation, enrollment, the resolved
//! configuration and local approvals. Each calls
//! `statecraft_home::service::execute` and maps what it returns, which is the
//! same rule as [`bind`] against a different owning spec.
//!
//! # Nothing here publishes
//!
//! There is no verb that publishes, releases or tags, and a test asserts the
//! command tree contains none.

#![forbid(unsafe_code)]

pub mod accept;
pub mod adapters;
pub mod bind;
pub mod commands;
pub mod exit;
pub mod manage;
pub mod render;
pub mod slice;

pub use commands::{Invocation, UsageError, Verb, parse};
pub use exit::Exit;
pub use render::{Answer, Format};

/// Where the product keeps its own state, outside any target.
///
/// Spec 002 section 3.1: the register lives under the product's own home, which
/// is what makes registration write nothing inside a target. Overridable by
/// `STATECRAFT_HOME` so a test, or an operator with two setups, is not forced to
/// share one.
/// Spec 010 section 3.1 gives the home a declared shape, so the resolution
/// moves to the crate that owns that shape and this stays the one name the
/// binary calls. Two functions answering "where is the home" is how a test
/// home and a real one end up being different places.
pub fn product_home() -> std::path::PathBuf {
    statecraft_home::home::resolve()
}
