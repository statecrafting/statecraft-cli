//! Section 3.5: a qualification record binds this build **and** a provider
//! version, earned by this adapter's own pass of spec 004's suite (obligation
//! I-1). A record for another adapter never counts: the seam's
//! [`qualification`] matches on adapter name, and this module adds the
//! provider half. With no record the adapter runs, labelled `unqualified`.

use crate::capabilities::ADAPTER_NAME;
use serde::{Deserialize, Serialize};
use statecraft_adapter::manifest::{Manifest, Qualification, QualificationRecord, qualification};

/// A recorded suite pass naming both versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairedRecord {
    /// The seam's record, which qualifies the adapter build.
    pub adapter: QualificationRecord,
    /// The Codex version the fixtures were recorded from.
    pub provider_version: String,
}

/// The record this build would write on a pass. `date` is the operator's,
/// never a clock read here.
pub fn record(provider_version: &str, suite_version: &str, date: &str) -> PairedRecord {
    PairedRecord {
        adapter: QualificationRecord {
            adapter: ADAPTER_NAME.to_string(),
            binary_version: env!("CARGO_PKG_VERSION").to_string(),
            suite_version: suite_version.to_string(),
            date: date.to_string(),
        },
        provider_version: provider_version.to_string(),
    }
}

/// Whether this build is qualified against the Codex version observed.
pub fn qualification_for_pair(
    manifest: &Manifest,
    observed_provider_version: &str,
    records: &[PairedRecord],
) -> Qualification {
    let matching: Vec<QualificationRecord> = records
        .iter()
        .filter(|r| r.provider_version == observed_provider_version)
        .map(|r| r.adapter.clone())
        .collect();
    qualification(manifest, &matching)
}
