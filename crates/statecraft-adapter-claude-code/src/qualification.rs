//! Qualification binds to a **pair** of versions.
//!
//! Spec 004 section 3.16. Spec 004 section 3.4 already says an adapter binary
//! version is qualified only by a recorded pass of the negative suite, and that
//! an adapter with no record **runs** and is labelled `unqualified` everywhere
//! it appears.
//!
//! For this adapter the record names two versions, not one: this crate's own
//! build and the **provider** binary it was measured against. Every finding in
//! spec 004 sections 3.9 to 3.14 is a fact about Claude Code
//! [`crate::capabilities::MEASURED_PROVIDER_VERSION`]. A provider upgrade
//! invalidates the qualification even when this crate is byte-identical, because
//! what was qualified was the pair.
//!
//! The seam's [`statecraft_adapter::manifest::qualification`] matches on adapter
//! name and binary version, which is one half. [`qualification_for_pair`] is the
//! other half, and it is here rather than in the seam because only a spec that
//! names a provider can say that a provider version is part of the record.

use crate::capabilities::{ADAPTER_NAME, MEASURED_PROVIDER_VERSION};
use serde::{Deserialize, Serialize};
use statecraft_adapter::manifest::{Manifest, Qualification, QualificationRecord, qualification};

/// The two versions a qualification record binds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderPair {
    /// This adapter's build.
    pub adapter_version: String,
    /// The provider binary the suite was run against.
    pub provider_version: String,
}

impl ProviderPair {
    /// The pair the provider measurements were taken against.
    pub fn measured() -> Self {
        Self {
            adapter_version: env!("CARGO_PKG_VERSION").to_string(),
            provider_version: MEASURED_PROVIDER_VERSION.to_string(),
        }
    }
}

/// A recorded pass, for this adapter, naming both versions.
///
/// The seam's record has one `binary_version` field, so the pair is encoded as
/// the adapter version there and the provider version in `suite_version`'s
/// company: rather than overload a field, the pair is carried separately and
/// [`qualification_for_pair`] reads both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairedRecord {
    /// The seam's own record, which qualifies the adapter binary.
    pub adapter: QualificationRecord,
    /// The provider binary that record was earned against.
    pub provider_version: String,
}

/// The record this adapter's build would write on a suite pass.
///
/// `date` is a parameter rather than a clock read: a record is written by the
/// operator performing a qualification act (section 3.8), and a library that
/// stamped it with its own clock would let a record appear without one.
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

/// Whether this adapter is qualified against the provider in front of it.
///
/// Both halves must match. A record for this adapter build earned against a
/// different provider version does **not** transfer, which is the whole content
/// of section 3.8, and the adapter still runs: spec 004 section 3.8's last row
/// but one says it is labelled `unqualified` in the posture, the attempt record
/// and the outcome, and that it still runs.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::manifest as adapter_manifest;

    fn measured_record() -> PairedRecord {
        record(MEASURED_PROVIDER_VERSION, "008.3.9", "2026-09-17T00:00:00Z")
    }

    #[test]
    fn a_record_for_the_measured_pair_qualifies() {
        assert_eq!(
            qualification_for_pair(
                &adapter_manifest(),
                MEASURED_PROVIDER_VERSION,
                &[measured_record()]
            ),
            Qualification::Qualified
        );
    }

    #[test]
    fn the_same_adapter_against_a_newer_provider_is_unqualified_and_still_runs() {
        let q = qualification_for_pair(&adapter_manifest(), "2.1.268", &[measured_record()]);
        assert_eq!(q, Qualification::Unqualified);
        assert_eq!(q.word(), "unqualified");
        // "Still runs" is the absence of a refusal: nothing in this module can
        // refuse, which is how spec 004 section 3.4 is implemented.
    }

    #[test]
    fn no_record_at_all_is_unqualified() {
        assert_eq!(
            qualification_for_pair(&adapter_manifest(), MEASURED_PROVIDER_VERSION, &[]),
            Qualification::Unqualified
        );
    }

    #[test]
    fn the_measured_pair_names_the_version_every_finding_binds_to() {
        assert_eq!(ProviderPair::measured().provider_version, "2.1.267");
    }
}
