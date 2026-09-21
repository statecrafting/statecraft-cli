//! The first provider adapter: Claude Code.
//!
//! This crate is the implementation of
//! [spec 004](../../../specs/004-execution-adapter/spec.md). It is the one
//! crate in this workspace permitted to name a provider. Spec 004 section 3.8
//! makes a provider name inside the seam a defect, and
//! `statecraft-adapter/tests/no_provider_names.rs` enforces that mechanically;
//! this crate exists so the seam never has to.
//!
//! # Everything here is a measurement, and it binds to one version pair
//!
//! Every fact in spec 004 sections 3.9 to 3.14 was measured against
//! **Claude Code 2.1.267 on darwin, on 2026-09-17**, with
//! `claude --print --output-format stream-json --verbose`. Section 3.8 binds the
//! qualification to the pair of this adapter's build and that provider version:
//! a provider upgrade invalidates the qualification even when this crate is
//! byte-identical, because what was qualified was the pair.
//!
//! The recorded streams under `testdata/stream/` are the measurement, committed.
//! See that directory's `README.md` for what was captured and what was redacted.
//! **A fixture is evidence of what the provider emitted on a named version. It
//! is not evidence that the provider still emits it**, and nothing here claims
//! otherwise.
//!
//! # The load-bearing finding
//!
//! A denied session classifies itself as a **success**, and the process exits
//! **0** (section 3.3). So:
//!
//! - [`stream`] carries the provider's terminal classification as the
//!   provider's *claim*, in a field named for a claim, and reports
//!   `permission_denials` verbatim as the refusal record.
//! - [`outcome`] holds section 3.5's fixed mapping onto spec 003 section 3.4's
//!   closed set. `success` with a non-empty denial list is `refused`.
//! - **The exit code is read nowhere in this crate.** It was unreliable in both
//!   directions: a refused session exited 0 and a turn-capped one exited 1.
//!
//! # What this crate does not do
//!
//! It does not spawn. [`statecraft_adapter::supervisor`] spawns, holds the
//! deadline and kills descendants; [`spawn`] only says what argv and what
//! settings document a spawn would use, so the invocation is a value a record
//! can carry rather than a string built at the call site.
//!
//! It declares no `workspace-write` (section 3.2), because nobody measured it.
//! Spec 004 section 3.3 makes an undeclared token a refusal when a run requires
//! it and a recorded degradation when a run prefers it, which is the correct
//! answer for a claim nobody has checked.

#![forbid(unsafe_code)]

pub mod capabilities;
pub mod denial;
pub mod environment;
pub mod execution;
pub mod outcome;
pub mod probe;
pub mod qualification;
pub mod spawn;
pub mod stream;

pub use capabilities::{ADAPTER_NAME, MEASURED_PROVIDER_VERSION, manifest, supported};
pub use denial::{DenialMechanism, ToolRestriction};
pub use environment::{
    CREDENTIAL_PATH, PROVIDER_EXECUTABLE, QUALIFICATION_RECORD, UNEXPRESSIBLE, declaration,
};
pub use outcome::{TerminalReading, UnmappedTerminalState, outcome};
pub use probe::{ConstructedEnvironmentProbe, observe_provider_version};
pub use qualification::{PairedRecord, ProviderPair, qualification_for_pair, record};
pub use spawn::{Invocation, SETTINGS_DENY_KEY};
pub use stream::{
    PermissionDenial, ProviderEvent, ResultEvent, SystemEvent, map_stream, read_jsonl,
};
