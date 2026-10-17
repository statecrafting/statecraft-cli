//! One supervised wire-witness sidecar per attempt, spec 013.
//!
//! Spec 013 amends specs 003 and 004 so this product may adopt one exact
//! linked producer and bracket one sidecar as its own durable effect. This
//! module is the provider-neutral half of that contract:
//!
//! - [`ledger`] is the adoption ledger of section 3.1: the exact released
//!   identity, and the refusal of any executable that is not it. There is no
//!   `PATH` lookup and no version range.
//! - [`protocol`] is the supervisor's side of the `wire-witness.sidecar/1`
//!   stdio protocol (section 3.2): the one `start` line it writes, and the
//!   bounded reader of what comes back. Every sidecar string is untrusted
//!   testimony (section 3.4); nothing here parses proxy traffic.
//! - [`posture`] is section 3.7's closed witness-posture vocabulary and the
//!   policy rule that alone decides whether a posture refuses acceptance.
//!
//! Witness testimony is never authority. A `witnessed` posture says the bound
//! sidecar finished with complete, integral capture; it does not say the work
//! is accepted, the provider was used, or the model complied.

pub mod ledger;
pub mod posture;
pub mod protocol;

pub use ledger::{ADOPTED, Adoption, ProducerRefusal, VerifiedProducer};
pub use posture::{Requirement, WitnessPosture};
pub use protocol::{AttemptBinding, Closing, Finding, Notice, Reader, Ready};
