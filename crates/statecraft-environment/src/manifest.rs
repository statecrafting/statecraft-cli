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
pub const MANIFEST_VERSION: u32 = 1;

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
    fn the_committed_file_ends_with_a_newline() {
        let dir = tempfile::tempdir().unwrap();
        Manifest::new(pins()).write(dir.path()).unwrap();
        let raw =
            std::fs::read_to_string(crate::claimant::resolve(dir.path(), MANIFEST_PATH)).unwrap();
        assert!(raw.ends_with("}\n"));
    }
}
