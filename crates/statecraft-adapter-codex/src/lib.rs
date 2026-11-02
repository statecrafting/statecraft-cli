//! The second provider adapter: Codex.
//!
//! This crate is the implementation of
//! [spec 055](../../../specs/055-codex-adapter/spec.md). It is the one crate in
//! this workspace permitted to name Codex, and it names no other provider:
//! `statecraft-adapter/tests/no_provider_names.rs` checks both directions.
//!
//! # What is measured and what is not
//!
//! The invocation (section 3.1) and the event names the mapping reads (section
//! 3.2) were taken from `codex-cli 0.163.0` on 2026-10-21. **The recorded
//! streams under `testdata/` are synthetic** until the owner records a real
//! capture from a named version; each directory's `README.md` says so. So the
//! adapter is `unqualified` (section 3.5), and its spec 047 declaration sets
//! every field nobody has measured to `false` (section 3.6).
//!
//! # The shape of it
//!
//! - [`capabilities`]: the manifest and the spec 047 declaration.
//! - [`spawn`]: the argv section 3.1 names, as a value a record can carry.
//! - [`stream`]: the JSONL events and section 3.2's outcome mapping. Exactly
//!   one terminal turn; anything else is `interrupted`.
//! - [`home`]: the per-run `CODEX_HOME` of section 3.3. The user's own Codex
//!   directory is never read for configuration and never written.
//! - [`hooks`]: section 3.4's query of the effective hooks, and the refusal
//!   unless this product's gate is a managed, synchronous, enabled
//!   `PreToolUse` command hook with an unrestricted matcher.
//! - [`execution`]: the supervised child, mapped onto the seam.
//! - [`qualification`]: a record binds this build and a provider version.
//!
//! It does not spawn on its own account: the seam's supervisor spawns, holds
//! the deadline and kills descendants, for the turn and for the hook query.

#![forbid(unsafe_code)]

pub mod capabilities;
pub mod execution;
pub mod home;
pub mod hooks;
pub mod qualification;
pub mod spawn;
pub mod stream;

pub use capabilities::{ADAPTER_NAME, MEASURED_PROVIDER_VERSION, declared, manifest, supported};
pub use execution::{Execution, supervise};
pub use home::{CODEX_HOME, RunHome};
pub use hooks::{Admitted, HookRefusal};
pub use spawn::Invocation;
pub use stream::{CodexEvent, Fault, Reading, Usage, read, read_jsonl};
