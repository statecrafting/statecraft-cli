//! Work selection, workspace preparation, the run record, and recovery.
//!
//! This crate is the implementation of
//! [spec 003](../../../specs/003-work-and-run-semantics/spec.md), the core
//! loop's semantics.
//!
//! # The shape of it
//!
//! - [`report`] reads spec-spine's structured output. Readiness is read, never
//!   computed here, and a report missing a field is a refusal that names the
//!   field and the version rather than a local substitute.
//! - [`policy`] decides which statuses a repository schedules. The answer
//!   belongs to the target, not to this product's state.
//! - [`work`] joins the two: every ready spec is either eligible or excluded
//!   with a reason, and nothing is silently dropped.
//! - [`workspace`] prepares an isolated git worktree. The operator's checkout is
//!   never edited, and the workspace is also the concurrency lock.
//! - [`record`] is the append-only hash-linked chain, fsynced before it
//!   acknowledges, kept in the product home so the supervised process cannot
//!   reach it.
//! - [`attempt`] holds the closed outcome set and the rule that a retry appends.
//! - [`refusal`] is the supervisor's own accounting, which no exit code can
//!   overrule.
//! - [`recovery`] folds the record, reconciles every intent with no outcome, and
//!   blocks a retry it cannot resolve.
//! - [`session`] is one run from intent to outcome, and the fold that reads runs
//!   back out of the record. Added by spec 006's additive edge, because a
//!   command may not be a second implementation and a binding needs an entry
//!   point.
//!
//! # What it does not do
//!
//! No binary, for the reason spec 002's crate has none: the operator verbs
//! belong to spec 006. No adapter protocol, which is 004. No acceptance, which
//! is 005: an attempt that `completed` has said nothing about whether its work
//! is any good.

#![forbid(unsafe_code)]

pub mod attempt;
pub mod contract;
pub mod lock;
pub mod overrides;
pub mod policy;
pub mod reconcile;
pub mod record;
pub mod recovery;
pub mod refusal;
pub mod report;
pub mod session;
pub mod work;
pub mod workspace;

pub use attempt::{Attempt, Outcome, Run};
pub use policy::{Overrides, Policy, PolicySource};
pub use record::{Chain, EffectId, Entry, Identity, Kind};
pub use recovery::{
    EffectFold, EffectKey, FoldDefect, UnmatchedEffect, Verdict, fold_effects, reconcile,
};
pub use report::{CorpusReport, ReportError};
pub use session::{Concluded, Session, SessionError, begin, conclude, runs};
pub use work::{WorkItem, WorkList, select};
pub use workspace::Workspace;
