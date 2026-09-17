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
//! # Nothing here publishes
//!
//! There is no verb that publishes, releases or tags, and a test asserts the
//! command tree contains none.

#![forbid(unsafe_code)]

pub mod bind;
pub mod commands;
pub mod exit;
pub mod render;

pub use commands::{Invocation, UsageError, Verb, parse};
pub use exit::Exit;
pub use render::{Answer, Format};

/// Where the product keeps its own state, outside any target.
///
/// Spec 002 section 3.1: the register lives under the product's own home, which
/// is what makes registration write nothing inside a target. Overridable by
/// `STATECRAFT_HOME` so a test, or an operator with two setups, is not forced to
/// share one.
pub fn product_home() -> std::path::PathBuf {
    if let Ok(explicit) = std::env::var("STATECRAFT_HOME") {
        return std::path::PathBuf::from(explicit);
    }
    let base = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::Path::new(&base).join(".statecraft")
}
