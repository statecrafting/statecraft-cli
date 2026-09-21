//! The one Statecraft-managed global environment.
//!
//! Spec 002 section 3.11. `~/.statecraft/`, overridden by `STATECRAFT_HOME`.
//! The override is not new: spec 006 section 5 recorded it, and the resolution
//! moves here so one function answers "where is the home" for the binary, the
//! library and every test.
//!
//! Three rules the shape keeps, and each is a property a test can hold:
//!
//! 1. **Extend, never replace.** This module reads and writes only the files it
//!    owns. `projects.json` and `qualifications.json` belong to specs 002 and
//!    004 and are never rewritten here, so adopting this shape loses no record.
//! 2. **An absent home is an empty home.** Every read answers without one.
//! 3. **No credential lives here.** Nothing in this module reads, writes or
//!    relocates a provider credential or a platform token.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The environment variable that relocates the product home.
pub const HOME_ENV: &str = "STATECRAFT_HOME";

/// The directory name under a user's home.
pub const HOME_DIR: &str = ".statecraft";

/// The schema version of the files this module owns.
pub const HOME_VERSION: u32 = 1;

/// Where the product home is for this process.
///
/// `STATECRAFT_HOME` wins. Otherwise `$HOME/.statecraft`, and with no `HOME`
/// either, `./.statecraft`, which keeps a process in a stripped environment
/// answering rather than panicking.
pub fn resolve() -> PathBuf {
    if let Ok(explicit) = std::env::var(HOME_ENV) {
        if !explicit.is_empty() {
            return PathBuf::from(explicit);
        }
    }
    let base = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    Path::new(&base).join(HOME_DIR)
}

/// The declared shape of a product home.
///
/// A type rather than a set of free functions so a caller cannot half-name a
/// home: every path below is derived from one root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    /// A layout rooted at an explicit directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The layout for this process's resolved home.
    pub fn resolved() -> Self {
        Self::new(resolve())
    }

    /// The home's root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Personal defaults and the operator's name.
    pub fn personal_file(&self) -> PathBuf {
        self.root.join("home.json")
    }

    /// Installed tools and harness revisions.
    pub fn tools_file(&self) -> PathBuf {
        self.root.join("tools.json")
    }

    /// What each native-discovery adapter was delivered.
    pub fn delivery_file(&self) -> PathBuf {
        self.root.join("delivery.json")
    }

    /// Modifications of files this product does not own.
    ///
    /// Spec 002 sections 3.13 and 3.24: a modification records the path, the
    /// exact content and the digest either side. It is never a managed entry
    /// and never ownership of the file, which is why it lives in the product's
    /// own home rather than in the file it describes.
    pub fn modifications_file(&self) -> PathBuf {
        self.root.join("modifications.json")
    }

    /// The canonical harness source.
    pub fn harness_dir(&self) -> PathBuf {
        self.root.join("harness")
    }

    /// One harness revision's directory.
    pub fn harness_revision_dir(&self, revision: &str) -> PathBuf {
        self.harness_dir().join(revision)
    }

    /// Local approval records, one file per project.
    pub fn approvals_dir(&self) -> PathBuf {
        self.root.join("approvals")
    }

    /// The register. Spec 002's file, named here and never rewritten here.
    pub fn registry_file(&self) -> PathBuf {
        self.root.join("projects.json")
    }

    /// Provider qualification records. Spec 004's file, same rule.
    pub fn qualifications_file(&self) -> PathBuf {
        self.root.join("qualifications.json")
    }

    /// Every directory a complete home has.
    pub fn directories(&self) -> Vec<PathBuf> {
        vec![self.root.clone(), self.harness_dir(), self.approvals_dir()]
    }

    /// The files this module owns, in a stable order.
    ///
    /// Deliberately not the register or the qualification records: naming them
    /// here is how a later "repair the home" would start rewriting them.
    pub fn owned_files(&self) -> Vec<PathBuf> {
        vec![
            self.personal_file(),
            self.tools_file(),
            self.delivery_file(),
            self.modifications_file(),
        ]
    }
}

/// The operator's personal defaults.
///
/// A default is the weakest layer in section 3.6: it supplies a value for a key
/// the project does not require and no team constrains, and it supplies nothing
/// else. It is deliberately a flat map of strings, because a richer shape here
/// would be a second settings engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Personal {
    /// Schema version.
    pub version: u32,
    /// Who this operator is, for an approval record to name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,
    /// Defaults, keyed the same way a project's overrides are.
    #[serde(default)]
    pub defaults: BTreeMap<String, String>,
}

impl Default for Personal {
    fn default() -> Self {
        Self {
            version: HOME_VERSION,
            operator: None,
            defaults: BTreeMap::new(),
        }
    }
}

/// What a tool's identity was asked to be, and what it turned out to be.
///
/// Spec 002 section 3.16: a run records the requested identity **and** the
/// identity that actually resolved. Recording only the request is how a report
/// says `=0.20.0` about a machine running something else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRecord {
    /// The tool's name, for example `spec-spine`.
    pub name: String,
    /// What was asked for, verbatim.
    pub requested: String,
    /// What actually answered, or the reserved absence word when nothing did.
    pub resolved: String,
    /// Where the resolved identity was observed, for example a path or `path`.
    pub observed_from: String,
    /// When, RFC 3339 UTC.
    pub recorded_at: String,
}

/// The word recorded when nothing answered.
///
/// The same word spec 005 section 3.8 uses for "no record of this kind exists".
/// An empty string or a zero would read as a version.
pub const NOT_RECORDED: &str = "not-recorded";

/// Installed tools and the harness revisions present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tools {
    /// Schema version.
    pub version: u32,
    /// One record per tool, ordered by name so the file is stable.
    #[serde(default)]
    pub tools: Vec<ToolRecord>,
    /// Harness revision identities present under `harness/`, ordered.
    #[serde(default)]
    pub harness_revisions: Vec<String>,
}

impl Default for Tools {
    fn default() -> Self {
        Self {
            version: HOME_VERSION,
            tools: Vec::new(),
            harness_revisions: Vec::new(),
        }
    }
}

impl Tools {
    /// Insert or replace a tool record, keeping records ordered by name.
    pub fn upsert(&mut self, record: ToolRecord) {
        match self.tools.binary_search_by(|t| t.name.cmp(&record.name)) {
            Ok(i) => self.tools[i] = record,
            Err(i) => self.tools.insert(i, record),
        }
    }

    /// The record for a tool, if there is one.
    pub fn get(&self, name: &str) -> Option<&ToolRecord> {
        self.tools.iter().find(|t| t.name == name)
    }

    /// Record a harness revision as present, without duplicating it.
    pub fn record_revision(&mut self, revision: &str) {
        if let Err(i) = self
            .harness_revisions
            .binary_search_by(|r| r.as_str().cmp(revision))
        {
            self.harness_revisions.insert(i, revision.to_string());
        }
    }
}

/// What went wrong reading or writing a file this module owns.
#[derive(Debug, thiserror::Error)]
pub enum HomeError {
    /// The file could not be read or written.
    #[error("home i/o at {path}: {source}")]
    Io {
        /// The file involved.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The file exists and is not readable as this schema.
    #[error("{path} is not readable as version {HOME_VERSION}: {source}")]
    Malformed {
        /// The file involved.
        path: PathBuf,
        /// The underlying error.
        source: serde_json::Error,
    },
    /// The file declares a schema version this build does not know.
    #[error("{path} declares version {found}, this build understands {HOME_VERSION}")]
    UnknownVersion {
        /// The file involved.
        path: PathBuf,
        /// The version it declares.
        found: u32,
    },
}

/// Read a JSON document this module owns, defaulting when it is absent.
fn read_versioned<T>(path: &Path, version_of: fn(&T) -> u32) -> Result<T, HomeError>
where
    T: serde::de::DeserializeOwned + Default,
{
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(T::default()),
        Err(source) => {
            return Err(HomeError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let value: T = serde_json::from_slice(&bytes).map_err(|source| HomeError::Malformed {
        path: path.to_path_buf(),
        source,
    })?;
    let found = version_of(&value);
    if found != HOME_VERSION {
        return Err(HomeError::UnknownVersion {
            path: path.to_path_buf(),
            found,
        });
    }
    Ok(value)
}

/// Write a JSON document this module owns, pretty and newline-terminated.
fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), HomeError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| HomeError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }
    let mut json = serde_json::to_string_pretty(value).map_err(|source| HomeError::Malformed {
        path: path.to_path_buf(),
        source,
    })?;
    json.push('\n');
    std::fs::write(path, json).map_err(|source| HomeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

impl Personal {
    /// Read the personal defaults, or the empty set when there are none.
    pub fn read(layout: &Layout) -> Result<Self, HomeError> {
        read_versioned(&layout.personal_file(), |p: &Personal| p.version)
    }

    /// Write the personal defaults.
    pub fn write(&self, layout: &Layout) -> Result<(), HomeError> {
        write_json(&layout.personal_file(), self)
    }
}

impl Tools {
    /// Read the tools record, or the empty one when there is none.
    pub fn read(layout: &Layout) -> Result<Self, HomeError> {
        read_versioned(&layout.tools_file(), |t: &Tools| t.version)
    }

    /// Write the tools record.
    pub fn write(&self, layout: &Layout) -> Result<(), HomeError> {
        write_json(&layout.tools_file(), self)
    }
}

/// What a read of the home found, without writing anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presence {
    /// The home's root.
    pub root: PathBuf,
    /// True when the root exists.
    pub exists: bool,
    /// Directories a complete home has and this one does not.
    pub missing_directories: Vec<String>,
    /// Files this module owns and this home does not have.
    pub missing_files: Vec<String>,
}

impl Presence {
    /// True when the home is complete.
    pub fn complete(&self) -> bool {
        self.exists && self.missing_directories.is_empty() && self.missing_files.is_empty()
    }
}

/// Look at a home. Reads only.
pub fn presence(layout: &Layout) -> Presence {
    let name = |p: &Path| {
        p.strip_prefix(layout.root())
            .unwrap_or(p)
            .display()
            .to_string()
    };
    Presence {
        root: layout.root().to_path_buf(),
        exists: layout.root().is_dir(),
        missing_directories: layout
            .directories()
            .iter()
            .filter(|d| !d.is_dir())
            .map(|d| name(d))
            .collect(),
        missing_files: layout
            .owned_files()
            .iter()
            .filter(|f| !f.is_file())
            .map(|f| name(f))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_home_reads_as_empty_rather_than_failing() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path().join("nothing-here"));
        assert_eq!(Personal::read(&layout).unwrap(), Personal::default());
        assert_eq!(Tools::read(&layout).unwrap(), Tools::default());
        assert!(!presence(&layout).exists);
    }

    #[test]
    fn the_layout_never_names_the_register_among_the_files_it_owns() {
        let layout = Layout::new("/home");
        assert!(!layout.owned_files().contains(&layout.registry_file()));
        assert!(!layout.owned_files().contains(&layout.qualifications_file()));
    }

    #[test]
    fn personal_defaults_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        let mut p = Personal {
            operator: Some("bart".into()),
            ..Personal::default()
        };
        p.defaults.insert("harness".into(), "h-abc".into());
        p.write(&layout).unwrap();
        assert_eq!(Personal::read(&layout).unwrap(), p);
    }

    #[test]
    fn a_tool_record_carries_both_the_request_and_what_resolved() {
        let mut t = Tools::default();
        t.upsert(ToolRecord {
            name: "spec-spine".into(),
            requested: "=0.20.0".into(),
            resolved: "0.20.0".into(),
            observed_from: "/usr/local/bin/spec-spine".into(),
            recorded_at: "1970-01-01T00:00:00Z".into(),
        });
        let r = t.get("spec-spine").unwrap();
        assert_eq!(r.requested, "=0.20.0");
        assert_eq!(r.resolved, "0.20.0");
    }

    #[test]
    fn nothing_that_resolved_is_recorded_as_a_version_of_zero() {
        let mut t = Tools::default();
        t.upsert(ToolRecord {
            name: "absent-tool".into(),
            requested: "any".into(),
            resolved: NOT_RECORDED.into(),
            observed_from: NOT_RECORDED.into(),
            recorded_at: "1970-01-01T00:00:00Z".into(),
        });
        assert_eq!(t.get("absent-tool").unwrap().resolved, "not-recorded");
    }

    #[test]
    fn tool_records_and_revisions_stay_ordered_and_unique() {
        let mut t = Tools::default();
        for name in ["z", "a", "a"] {
            t.upsert(ToolRecord {
                name: name.into(),
                requested: "1".into(),
                resolved: "1".into(),
                observed_from: "x".into(),
                recorded_at: "1970-01-01T00:00:00Z".into(),
            });
        }
        assert_eq!(
            t.tools.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            ["a", "z"]
        );
        t.record_revision("h-2");
        t.record_revision("h-1");
        t.record_revision("h-1");
        assert_eq!(t.harness_revisions, ["h-1", "h-2"]);
    }

    #[test]
    fn a_future_schema_version_names_the_version_it_found() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        std::fs::write(layout.personal_file(), r#"{"version":99,"defaults":{}}"#).unwrap();
        match Personal::read(&layout) {
            Err(HomeError::UnknownVersion { found, .. }) => assert_eq!(found, 99),
            other => panic!("expected UnknownVersion, got {other:?}"),
        }
    }

    #[test]
    fn a_complete_home_reports_nothing_missing() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        for d in layout.directories() {
            std::fs::create_dir_all(d).unwrap();
        }
        Personal::default().write(&layout).unwrap();
        Tools::default().write(&layout).unwrap();
        std::fs::write(layout.delivery_file(), "[]\n").unwrap();
        std::fs::write(layout.modifications_file(), "[]\n").unwrap();
        assert!(presence(&layout).complete());
    }
}
