//! Detecting this adapter's three prerequisites, for real.
//!
//! Spec 004 section 3.15's refusal is only as honest as the detection behind it,
//! so this is a real [`HarnessProbe`] rather than a table. Each answer says what
//! it observed and, where it cannot observe the thing itself, what it observed
//! instead.
//!
//! # `provider-executable`
//!
//! Resolved against the **constructed** environment's `PATH` (spec 004 section
//! 3.6), not the operator's. A `claude` on the operator's own `PATH` is not
//! evidence it is on the child's, and the two differ by construction here: the
//! child inherits nothing.
//!
//! # `qualification-record`
//!
//! Satisfied when a record on hand names this adapter build **and** the observed
//! provider version. Note what this prerequisite does not do: an unqualified
//! adapter still runs (spec 004 section 3.4). It is declared here because spec
//! 004 section 3.15 lists it, and what it gates is the adapter claiming the
//! target's *paths*, not the adapter running.
//!
//! # `credential-path`
//!
//! The presence of the **mechanism**, not of a working credential. Section 3.6
//! measured `apiKeySource: "none"` on a working session and concluded that on
//! `darwin` this provider resolves credentials through the operating-system
//! keychain. Checking that a credential works means spending one, and section 4
//! puts anything that touches a credential out of scope. So the observable fact
//! is the platform, and on a platform where section 3 took no measurement the
//! prerequisite reads **absent** rather than assumed: every finding in sections
//! 3.1 to 3.6 is a fact about darwin.

use crate::capabilities::ADAPTER_NAME;
use crate::environment::{CREDENTIAL_PATH, HARNESS, PROVIDER_EXECUTABLE, QUALIFICATION_RECORD};
use crate::qualification::{PairedRecord, qualification_for_pair};
use statecraft_adapter::manifest::Qualification;
use statecraft_environment::adapter::HarnessProbe;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The platform the provider measurements were taken on.
pub const MEASURED_PLATFORM: &str = "macos";

/// A probe that answers from the constructed environment and the records on hand.
#[derive(Debug, Clone, Default)]
pub struct ConstructedEnvironmentProbe {
    path: Option<String>,
    provider_version: Option<String>,
    records: Vec<PairedRecord>,
    platform: String,
}

impl ConstructedEnvironmentProbe {
    /// A probe reading the `PATH` of a constructed child environment.
    ///
    /// Takes the variables the child will actually get, which is why this cannot
    /// accidentally answer from the parent's `PATH`.
    pub fn from_child_environment(variables: &BTreeMap<String, String>) -> Self {
        Self {
            path: variables.get("PATH").cloned(),
            provider_version: None,
            records: Vec::new(),
            platform: std::env::consts::OS.to_string(),
        }
    }

    /// The provider version observed in front of us, however the caller got it.
    ///
    /// A parameter and not a `claude --version` call: spec 004 section 3.16 makes
    /// the qualification an act performed against a named binary version and
    /// recorded, and a probe that discovered the version itself would let the
    /// candidate under test choose what it is compared against.
    #[must_use]
    pub fn observing_provider_version(mut self, version: &str) -> Self {
        self.provider_version = Some(version.to_string());
        self
    }

    /// The qualification records on hand.
    #[must_use]
    pub fn with_records(mut self, records: Vec<PairedRecord>) -> Self {
        self.records = records;
        self
    }

    /// Override the platform, so the darwin-only reading is testable elsewhere.
    #[must_use]
    pub fn on_platform(mut self, platform: &str) -> Self {
        self.platform = platform.to_string();
        self
    }

    /// Where `claude` resolves to on the child's `PATH`, if anywhere.
    pub fn resolved_executable(&self) -> Option<PathBuf> {
        let path = self.path.as_deref()?;
        for dir in path.split(':').filter(|d| !d.is_empty()) {
            let candidate = Path::new(dir).join("claude");
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    /// Whether the records on hand qualify this adapter against the observed
    /// provider version.
    pub fn qualification(&self) -> Qualification {
        let Some(observed) = self.provider_version.as_deref() else {
            return Qualification::Unqualified;
        };
        qualification_for_pair(&crate::capabilities::manifest(), observed, &self.records)
    }
}

fn is_executable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

impl HarnessProbe for ConstructedEnvironmentProbe {
    fn harness_present(&self, harness: &str) -> bool {
        // The harness IS the provider binary for this adapter: there is no
        // separate installation to detect, and inventing one would be a
        // heuristic spec 002 section 3.9 does not ask for.
        harness == HARNESS && self.resolved_executable().is_some()
    }

    fn prerequisite_satisfied(&self, harness: &str, prerequisite: &str) -> bool {
        if harness != HARNESS {
            return false;
        }
        match prerequisite {
            PROVIDER_EXECUTABLE => self.resolved_executable().is_some(),
            QUALIFICATION_RECORD => self.qualification() == Qualification::Qualified,
            CREDENTIAL_PATH => self.platform == MEASURED_PLATFORM,
            // An unknown prerequisite is not satisfied. A probe that answered
            // `true` for a name it did not recognise would make adding a
            // prerequisite a silent no-op.
            _ => false,
        }
    }
}

/// Ask the resolved executable which version it is.
///
/// Observation only. Spec 004 section 3.16 makes the qualification an act
/// performed by an operator against a **named** binary version and recorded;
/// this exists so a record can be compared with the binary in front of us, and
/// it is deliberately not wired into [`ConstructedEnvironmentProbe`]: a probe
/// that discovered the version itself would let the thing under test choose what
/// it is compared against.
///
/// The measured output is `2.1.267 (Claude Code)`, so the version is the first
/// whitespace-separated token. A shape this does not recognise reads `None`,
/// which makes the adapter `unqualified` and leaves it running.
pub fn observe_provider_version(program: &Path) -> Option<String> {
    let output = std::process::Command::new(program)
        .arg("--version")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let token = text.split_whitespace().next()?;
    // A version is digits and dots. Anything else is a shape this build does not
    // read, and guessing at it would put an invented version in a record.
    if token.is_empty() || !token.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    Some(token.to_string())
}

/// The adapter name, re-exported so a caller naming it in a report need not
/// reach into [`crate::capabilities`].
pub const fn adapter_name() -> &'static str {
    ADAPTER_NAME
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qualification::record;

    fn child_with_claude(dir: &Path) -> BTreeMap<String, String> {
        let exe = dir.join("claude");
        std::fs::write(&exe, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut vars = BTreeMap::new();
        vars.insert("PATH".to_string(), dir.display().to_string());
        vars
    }

    #[test]
    fn an_empty_child_environment_resolves_no_executable() {
        let probe = ConstructedEnvironmentProbe::from_child_environment(&BTreeMap::new());
        assert!(probe.resolved_executable().is_none());
        assert!(!probe.harness_present(HARNESS));
        assert!(!probe.prerequisite_satisfied(HARNESS, PROVIDER_EXECUTABLE));
    }

    #[test]
    fn a_claude_on_the_childs_path_satisfies_the_executable_prerequisite() {
        let dir = tempfile::tempdir().unwrap();
        let probe =
            ConstructedEnvironmentProbe::from_child_environment(&child_with_claude(dir.path()));
        assert!(probe.resolved_executable().is_some());
        assert!(probe.harness_present(HARNESS));
    }

    #[test]
    fn a_record_for_another_provider_version_does_not_satisfy_the_qualification() {
        let dir = tempfile::tempdir().unwrap();
        let probe =
            ConstructedEnvironmentProbe::from_child_environment(&child_with_claude(dir.path()))
                .observing_provider_version("2.1.268")
                .with_records(vec![record("2.1.267", "008.3.9", "2026-09-17T00:00:00Z")]);
        assert!(!probe.prerequisite_satisfied(HARNESS, QUALIFICATION_RECORD));
        assert_eq!(probe.qualification(), Qualification::Unqualified);
    }

    #[test]
    fn a_record_for_the_observed_pair_satisfies_it() {
        let dir = tempfile::tempdir().unwrap();
        let probe =
            ConstructedEnvironmentProbe::from_child_environment(&child_with_claude(dir.path()))
                .observing_provider_version("2.1.267")
                .with_records(vec![record("2.1.267", "008.3.9", "2026-09-17T00:00:00Z")]);
        assert!(probe.prerequisite_satisfied(HARNESS, QUALIFICATION_RECORD));
    }

    #[test]
    fn the_credential_path_is_the_platform_and_an_unmeasured_platform_reads_absent() {
        let probe = ConstructedEnvironmentProbe::default().on_platform("macos");
        assert!(probe.prerequisite_satisfied(HARNESS, CREDENTIAL_PATH));
        let elsewhere = ConstructedEnvironmentProbe::default().on_platform("linux");
        assert!(!elsewhere.prerequisite_satisfied(HARNESS, CREDENTIAL_PATH));
    }

    #[test]
    fn an_unknown_prerequisite_is_not_satisfied() {
        let probe = ConstructedEnvironmentProbe::default().on_platform("macos");
        assert!(!probe.prerequisite_satisfied(HARNESS, "something-nobody-declared"));
    }

    #[test]
    fn an_executable_that_does_not_answer_with_a_version_reads_none() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("claude");
        std::fs::write(&exe, "#!/bin/sh\necho 'not a version'\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!(observe_provider_version(&exe), None);
    }

    #[test]
    fn a_version_shaped_answer_is_read_as_the_version() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("claude");
        std::fs::write(&exe, "#!/bin/sh\necho '2.1.267 (Claude Code)'\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!(observe_provider_version(&exe).as_deref(), Some("2.1.267"));
    }

    #[test]
    fn another_harnesss_prerequisite_is_never_answered_here() {
        let probe = ConstructedEnvironmentProbe::default().on_platform("macos");
        assert!(!probe.prerequisite_satisfied("some-other-harness", CREDENTIAL_PATH));
        assert!(!probe.harness_present("some-other-harness"));
        assert_eq!(adapter_name(), "claude-code");
    }
}
