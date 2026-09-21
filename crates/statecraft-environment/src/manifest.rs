//! The committed manifest: the record of what this product owns in a target.
//!
//! Spec 002 section 3.3. `.statecraft/environment.json` is committed and is not
//! state: runtime state lives under `.statecraft/state/` and is gitignored. The
//! manifest is the only thing that makes the three ownership classes decidable,
//! which is why a write not recorded here is a defect (`unmanaged-write`) rather
//! than an untracked convenience.

use crate::claimant::Claimant;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Where the manifest lives inside a target repository.
pub const MANIFEST_PATH: &str = ".statecraft/environment.json";

/// The manifest schema version. Bumping it is a change to this spec.
///
/// Version 2 (spec 010) adds the project declaration and the tracked
/// modifications. There is no migration from version 1: nothing is released,
/// so no version-1 file exists outside a test, and a migration framework for a
/// schema with no adopters would be machinery maintained for nobody. A
/// version-1 file is refused by version, which is the existing behavior and
/// names the number it found.
pub const MANIFEST_VERSION: u32 = 2;

/// Which of the three ownership classes an entry records.
///
/// Only two of the three appear here. `user` is everything the manifest does
/// NOT mention, which is what makes the classes exhaustive by construction
/// rather than by an assertion somebody has to maintain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Class {
    /// Written by this product. Rewritable on upgrade, removed on removal.
    Managed,
    /// Existed before. Depended on, never rewritten.
    Adopted,
}

/// What produced a managed path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceKind {
    /// An agent-harness adapter (section 3.9).
    Adapter,
    /// A template this product carries.
    Template,
}

/// The source of an entry's content, and the identity of that source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// Adapter or template.
    pub kind: SourceKind,
    /// The source's identity: an adapter name, or a template identifier.
    pub identity: String,
}

/// An ownership transfer for a path another installer claims.
///
/// Section 3.7.3: per path, explicit, operator-initiated, and recorded with the
/// digest observed at the moment of transfer. Section 3.7.5: it also records
/// which kit revision the transfer was evaluated against, so a kit that moves
/// afterwards is a `doctor` finding rather than a silent divergence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transfer {
    /// Who held the path before the transfer.
    pub from: Claimant,
    /// The digest observed at the moment of transfer.
    pub digest_at_transfer: String,
    /// The kit revision the transfer was evaluated against, where one is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluated_against: Option<String>,
}

/// One manifested path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Repository-relative, forward slashes.
    pub path: String,
    /// Managed or adopted.
    pub class: Class,
    /// What produced it.
    pub source: Source,
    /// SHA-256 of the content written (managed) or observed (adopted).
    pub digest: String,
    /// Length in bytes of that same content.
    pub bytes: u64,
    /// When the write or the adoption happened, RFC 3339 UTC.
    pub written_at: String,
    /// Present when this path was taken over from another claimant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transfer: Option<Transfer>,
}

/// The versions an environment was installed against.
///
/// Section 3.3: pins are recorded, never silently satisfied. Nothing in this
/// crate compares a pin and then repairs it; `doctor` reports the mismatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pins {
    /// This product's version at install time.
    pub product: String,
    /// The spec-spine version the environment was installed against.
    pub spec_spine: String,
    /// Adapter name to version, ordered so the committed file is stable.
    #[serde(default)]
    pub adapters: BTreeMap<String, String>,
}

/// Whether a project coordinates with a team, and which one.
///
/// Spec 010 section 3.8: enrollment is explicit and project-scoped, it lives in
/// this committed declaration and nowhere else, and the default is solo. A
/// platform this product cannot reach does not change what this field says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Enrollment {
    /// No team. Every authority is local.
    #[default]
    Solo,
    /// Enrolled into one team, which is the coordination authority for shared
    /// approvals, eligibility and policy.
    Team {
        /// The team's identifier, as the operator gave it.
        team: String,
    },
}

impl Enrollment {
    /// The team, when there is one.
    pub fn team(&self) -> Option<&str> {
        match self {
            Enrollment::Solo => None,
            Enrollment::Team { team } => Some(team.as_str()),
        }
    }
}

/// The committed project declaration.
///
/// Spec 010 section 3.2. It carries what a remote worker needs in order to
/// resolve the same requirements independently, and therefore carries no
/// secret, no machine-specific absolute path and no personal preference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Project {
    /// What this project requires, keyed by tool or capability. A requirement
    /// binds every configuration layer: no personal default and no run choice
    /// may contradict one.
    #[serde(default)]
    pub requirements: BTreeMap<String, String>,
    /// What this project sets for itself, keyed the same way. An override
    /// supplies project behavior, within any team constraint.
    #[serde(default)]
    pub overrides: BTreeMap<String, String>,
    /// Subjects that require a shared approval before they are eligible. Only
    /// meaningful for an enrolled project; recorded regardless, so unenrolling
    /// and re-enrolling does not lose the requirement.
    #[serde(default)]
    pub shared_approval_required: Vec<String>,
    /// Solo, or the team this project is enrolled into.
    #[serde(default)]
    pub enrollment: Enrollment,
}

/// A declared value this product refuses to commit, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortabilityViolation {
    /// Which map it came from: `requirements` or `overrides`.
    pub field: String,
    /// The key.
    pub key: String,
    /// The value, verbatim.
    pub value: String,
    /// Why it cannot be committed.
    pub reason: String,
}

impl PortabilityViolation {
    /// A one-line rendering for a report.
    pub fn describe(&self) -> String {
        format!("{}.{}: {}", self.field, self.key, self.reason)
    }
}

impl Project {
    /// Every declared value this product will not commit.
    ///
    /// An absolute path or a `~` prefix names one machine's filesystem, and a
    /// remote worker resolving the same declaration cannot satisfy it. Refused
    /// at write time rather than reported later, because a committed value
    /// nobody else can satisfy is already the defect.
    pub fn portability_violations(&self) -> Vec<PortabilityViolation> {
        let mut out = Vec::new();
        for (field, map) in [
            ("requirements", &self.requirements),
            ("overrides", &self.overrides),
        ] {
            for (key, value) in map {
                if let Some(reason) = non_portable(value) {
                    out.push(PortabilityViolation {
                        field: field.to_string(),
                        key: key.clone(),
                        value: value.clone(),
                        reason: reason.to_string(),
                    });
                }
            }
        }
        out
    }
}

/// Why a declared value names one machine rather than a requirement.
fn non_portable(value: &str) -> Option<&'static str> {
    if value.starts_with('/') {
        return Some("an absolute path is not portable to another machine");
    }
    if value.starts_with('~') {
        return Some("a home-relative path is not portable to another operator");
    }
    None
}

/// What kind of modification this product made to a file it does not own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModificationKind {
    /// One import line inserted as the first line of a user instruction file.
    ImportBridge,
}

/// A tracked modification of a file this product does NOT own.
///
/// Spec 010 section 3.3. The distinction from an [`Entry`] is the whole point:
/// an entry says "these bytes are ours"; a modification says "one line of
/// somebody else's file is ours, and here is what the file looked like before
/// and after". Removal takes back the line and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modification {
    /// Repository-relative, forward slashes.
    pub path: String,
    /// What was done.
    pub kind: ModificationKind,
    /// The exact line inserted.
    pub line: String,
    /// SHA-256 of the file before the insertion. Absent when the file did not
    /// exist, which is the case where this product created it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest_before: Option<String>,
    /// SHA-256 of the file as this product left it.
    pub digest_after: String,
    /// When, RFC 3339 UTC.
    pub written_at: String,
}

/// The manifest itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Schema version.
    pub version: u32,
    /// The recorded pins.
    pub pins: Pins,
    /// Every managed and adopted path.
    #[serde(default)]
    pub entries: Vec<Entry>,
    /// Lines this product owns inside files it does not (spec 010 section 3.3).
    #[serde(default)]
    pub modifications: Vec<Modification>,
    /// The project declaration (spec 010 section 3.2).
    #[serde(default)]
    pub project: Project,
}

/// What went wrong reading or writing a manifest.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// The file could not be read or written.
    #[error("manifest i/o at {path}: {source}")]
    Io {
        /// The path being read or written.
        path: String,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The file exists but is not a manifest this version understands.
    #[error("manifest at {path} is not readable as version {MANIFEST_VERSION}: {source}")]
    Malformed {
        /// The path being read.
        path: String,
        /// The underlying error.
        source: serde_json::Error,
    },
    /// A declared value names one machine rather than a requirement.
    #[error("declaration at {path} is not portable: {}", .violations.iter().map(PortabilityViolation::describe).collect::<Vec<_>>().join("; "))]
    NonPortable {
        /// The path being written.
        path: String,
        /// Every value that cannot be committed.
        violations: Vec<PortabilityViolation>,
    },
    /// The schema version is one this build does not know.
    #[error(
        "manifest at {path} declares version {found}, this build understands {MANIFEST_VERSION}"
    )]
    UnknownVersion {
        /// The path being read.
        path: String,
        /// The version the file declares.
        found: u32,
    },
}

impl Manifest {
    /// An empty manifest with the given pins.
    pub fn new(pins: Pins) -> Self {
        Self {
            version: MANIFEST_VERSION,
            pins,
            entries: Vec::new(),
            modifications: Vec::new(),
            project: Project::default(),
        }
    }

    /// The entry for a repository-relative path, if the manifest records one.
    pub fn entry(&self, repo_relative: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.path == repo_relative)
    }

    /// True when the manifest records this path in any class.
    ///
    /// The negation is the definition of `user` class, so this is the predicate
    /// the whole ownership model rests on.
    pub fn records(&self, repo_relative: &str) -> bool {
        self.entry(repo_relative).is_some()
    }

    /// Insert or replace an entry, keeping entries ordered by path.
    ///
    /// Ordered because the manifest is committed: an unordered list would
    /// produce a diff on every write that touched any path.
    pub fn upsert(&mut self, entry: Entry) {
        match self.entries.binary_search_by(|e| e.path.cmp(&entry.path)) {
            Ok(i) => self.entries[i] = entry,
            Err(i) => self.entries.insert(i, entry),
        }
    }

    /// Remove an entry by path, returning it.
    pub fn remove(&mut self, repo_relative: &str) -> Option<Entry> {
        let i = self.entries.iter().position(|e| e.path == repo_relative)?;
        Some(self.entries.remove(i))
    }

    /// The tracked modification of a path, if this product made one.
    pub fn modification(&self, repo_relative: &str) -> Option<&Modification> {
        self.modifications.iter().find(|m| m.path == repo_relative)
    }

    /// Insert or replace a tracked modification, keeping them ordered by path.
    pub fn upsert_modification(&mut self, modification: Modification) {
        match self
            .modifications
            .binary_search_by(|m| m.path.cmp(&modification.path))
        {
            Ok(i) => self.modifications[i] = modification,
            Err(i) => self.modifications.insert(i, modification),
        }
    }

    /// Drop a tracked modification, returning it.
    pub fn remove_modification(&mut self, repo_relative: &str) -> Option<Modification> {
        let i = self
            .modifications
            .iter()
            .position(|m| m.path == repo_relative)?;
        Some(self.modifications.remove(i))
    }

    /// Every managed entry, in path order.
    pub fn managed(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|e| e.class == Class::Managed)
    }

    /// Read a manifest from a target repository root.
    ///
    /// `Ok(None)` when no manifest exists, which several operations treat as a
    /// distinct case rather than as an empty one: `env remove` refuses on it.
    pub fn read(root: &std::path::Path) -> Result<Option<Self>, ManifestError> {
        let path = crate::claimant::resolve(root, MANIFEST_PATH);
        let display = MANIFEST_PATH.to_string();
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(ManifestError::Io {
                    path: display,
                    source,
                });
            }
        };
        // Read the version before the body, so a future schema fails with the
        // version it found rather than with a field-level deserialization error
        // that makes the cause look like corruption.
        let probe: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|source| ManifestError::Malformed {
                path: display.clone(),
                source,
            })?;
        match probe.get("version").and_then(serde_json::Value::as_u64) {
            Some(v) if v == u64::from(MANIFEST_VERSION) => {}
            Some(found) => {
                return Err(ManifestError::UnknownVersion {
                    path: display,
                    found: found as u32,
                });
            }
            None => {
                // No version field at all: let the typed read produce the
                // precise error rather than inventing one.
            }
        }
        serde_json::from_slice(&bytes).map_err(|source| ManifestError::Malformed {
            path: display,
            source,
        })
    }

    /// Write the manifest into a target repository root.
    ///
    /// Pretty-printed with a trailing newline, because it is committed and a
    /// human reads its diff.
    pub fn write(&self, root: &std::path::Path) -> Result<(), ManifestError> {
        // Spec 010 section 3.2: a value naming one machine's filesystem is
        // refused here rather than reported later, because the committed file
        // is exactly what a remote worker has to resolve from.
        let violations = self.project.portability_violations();
        if !violations.is_empty() {
            return Err(ManifestError::NonPortable {
                path: MANIFEST_PATH.to_string(),
                violations,
            });
        }
        let path = crate::claimant::resolve(root, MANIFEST_PATH);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ManifestError::Io {
                path: MANIFEST_PATH.to_string(),
                source,
            })?;
        }
        let mut json =
            serde_json::to_string_pretty(self).map_err(|source| ManifestError::Malformed {
                path: MANIFEST_PATH.to_string(),
                source,
            })?;
        json.push('\n');
        std::fs::write(&path, json).map_err(|source| ManifestError::Io {
            path: MANIFEST_PATH.to_string(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pins() -> Pins {
        Pins {
            product: "0.0.0".into(),
            spec_spine: "0.18.0".into(),
            adapters: BTreeMap::new(),
        }
    }

    fn entry(path: &str) -> Entry {
        Entry {
            path: path.into(),
            class: Class::Managed,
            source: Source {
                kind: SourceKind::Template,
                identity: "t".into(),
            },
            digest: "d".into(),
            bytes: 1,
            written_at: "1970-01-01T00:00:00Z".into(),
            transfer: None,
        }
    }

    #[test]
    fn a_path_the_manifest_does_not_record_is_user_class() {
        let m = Manifest::new(pins());
        assert!(!m.records("src/main.rs"));
    }

    #[test]
    fn entries_stay_ordered_by_path_however_they_are_inserted() {
        let mut m = Manifest::new(pins());
        m.upsert(entry("z"));
        m.upsert(entry("a"));
        m.upsert(entry("m"));
        let paths: Vec<_> = m.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["a", "m", "z"]);
    }

    #[test]
    fn upsert_replaces_rather_than_duplicates() {
        let mut m = Manifest::new(pins());
        m.upsert(entry("a"));
        let mut second = entry("a");
        second.digest = "other".into();
        m.upsert(second);
        assert_eq!(m.entries.len(), 1);
        assert_eq!(m.entries[0].digest, "other");
    }

    #[test]
    fn an_absent_manifest_reads_as_none_not_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Manifest::read(dir.path()).unwrap().is_none());
    }

    #[test]
    fn a_written_manifest_reads_back_identical() {
        let dir = tempfile::tempdir().unwrap();
        let mut m = Manifest::new(pins());
        m.upsert(entry("AGENTS.md"));
        m.write(dir.path()).unwrap();
        assert_eq!(Manifest::read(dir.path()).unwrap().unwrap(), m);
    }

    #[test]
    fn a_future_schema_version_names_the_version_it_found() {
        let dir = tempfile::tempdir().unwrap();
        let path = crate::claimant::resolve(dir.path(), MANIFEST_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"version":99,"pins":{},"entries":[]}"#).unwrap();
        match Manifest::read(dir.path()) {
            Err(ManifestError::UnknownVersion { found, .. }) => assert_eq!(found, 99),
            other => panic!("expected UnknownVersion, got {other:?}"),
        }
    }

    #[test]
    fn an_absolute_path_in_the_declaration_is_refused_at_write_time() {
        let dir = tempfile::tempdir().unwrap();
        let mut m = Manifest::new(pins());
        m.project.requirements.insert(
            "spec-spine".into(),
            "/Users/someone/.tooling/bin/spec-spine".into(),
        );
        match m.write(dir.path()) {
            Err(ManifestError::NonPortable { violations, .. }) => {
                assert_eq!(violations.len(), 1);
                assert_eq!(violations[0].key, "spec-spine");
            }
            other => panic!("expected NonPortable, got {other:?}"),
        }
        assert!(!crate::claimant::resolve(dir.path(), MANIFEST_PATH).exists());
    }

    #[test]
    fn a_home_relative_value_is_refused_with_its_own_reason() {
        let mut p = Project::default();
        p.overrides
            .insert("harness".into(), "~/.statecraft/harness".into());
        let v = p.portability_violations();
        assert_eq!(v.len(), 1);
        assert!(v[0].reason.contains("home-relative"));
    }

    #[test]
    fn an_ordinary_declared_value_is_portable() {
        let mut p = Project::default();
        p.requirements.insert("spec-spine".into(), "=0.20.0".into());
        p.overrides
            .insert("harness".into(), "h-0123456789ab".into());
        assert!(p.portability_violations().is_empty());
    }

    #[test]
    fn the_default_enrollment_is_solo() {
        assert_eq!(Manifest::new(pins()).project.enrollment, Enrollment::Solo);
        assert_eq!(Enrollment::Solo.team(), None);
        assert_eq!(Enrollment::Team { team: "t".into() }.team(), Some("t"));
    }

    #[test]
    fn a_modification_is_tracked_separately_from_an_entry() {
        let mut m = Manifest::new(pins());
        m.upsert_modification(Modification {
            path: "AGENTS.md".into(),
            kind: ModificationKind::ImportBridge,
            line: "@.statecraft/AGENTS.md".into(),
            digest_before: Some("before".into()),
            digest_after: "after".into(),
            written_at: "1970-01-01T00:00:00Z".into(),
        });
        // Tracked, and NOT ownership: the manifest does not record the file.
        assert!(m.modification("AGENTS.md").is_some());
        assert!(!m.records("AGENTS.md"));
        assert!(m.remove_modification("AGENTS.md").is_some());
        assert!(m.modification("AGENTS.md").is_none());
    }

    #[test]
    fn modifications_stay_ordered_and_never_duplicate() {
        let mut m = Manifest::new(pins());
        for path in ["z", "a", "a"] {
            m.upsert_modification(Modification {
                path: path.into(),
                kind: ModificationKind::ImportBridge,
                line: "@x".into(),
                digest_before: None,
                digest_after: "d".into(),
                written_at: "1970-01-01T00:00:00Z".into(),
            });
        }
        let paths: Vec<_> = m.modifications.iter().map(|m| m.path.as_str()).collect();
        assert_eq!(paths, ["a", "z"]);
    }

    #[test]
    fn a_version_one_manifest_is_refused_by_version_with_no_migration() {
        let dir = tempfile::tempdir().unwrap();
        let path = crate::claimant::resolve(dir.path(), MANIFEST_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{"version":1,"pins":{"product":"0","specSpine":"0"},"entries":[]}"#,
        )
        .unwrap();
        match Manifest::read(dir.path()) {
            Err(ManifestError::UnknownVersion { found, .. }) => assert_eq!(found, 1),
            other => panic!("expected UnknownVersion, got {other:?}"),
        }
    }

    #[test]
    fn the_committed_file_ends_with_a_newline() {
        let dir = tempfile::tempdir().unwrap();
        Manifest::new(pins()).write(dir.path()).unwrap();
        let raw =
            std::fs::read_to_string(crate::claimant::resolve(dir.path(), MANIFEST_PATH)).unwrap();
        assert!(raw.ends_with("}\n"));
    }
}
