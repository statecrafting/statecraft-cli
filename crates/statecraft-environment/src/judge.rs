//! The record of which spec-spine judged an operation (spec 029 section 3.3).
//!
//! The selection itself is `statecraft-home`'s. This is only its answer as
//! every report about a judged operation writes it, kept here because the run
//! record, the acceptance receipt and the initialization report are written by
//! three crates that all depend on this one.

use serde::{Deserialize, Serialize};

/// `{program, rule, version, digest, passedOver}`: the executable one
/// resolution selected, the rule that chose it, what it reported to
/// `--version`, the SHA-256 of its bytes, and every candidate passed over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgeRecord {
    /// The executable, as an absolute path.
    pub program: String,
    /// The rule that selected it: `supervisor`, `override`, `launcher`,
    /// `repository-local`, `repository-build` or `path`.
    pub rule: String,
    /// What it answered to `--version`, when it answered.
    pub version: Option<String>,
    /// `sha256:<hex>` of its bytes, when they could be read.
    pub digest: Option<String>,
    /// Every candidate passed over, one sentence each, in order.
    #[serde(default)]
    pub passed_over: Vec<String>,
}
