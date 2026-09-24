//! The committed manifest: the record of what this product owns in a target.
//!
//! Spec 002 section 3.3. `.statecraft/environment.json` is committed and is not
//! state: runtime state lives under `.statecraft/state/` and is gitignored. The
//! manifest is the only thing that makes the three ownership classes decidable,
//! which is why a write not recorded here is a defect (`unmanaged-write`) rather
//! than an untracked convenience.

use crate::claimant::Claimant;
use crate::transfer::TransferRecord;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// Where the manifest lives inside a target repository.
pub const MANIFEST_PATH: &str = ".statecraft/environment.json";

/// The manifest schema version. Bumping it is a change to this spec.
///
/// Version 2 (spec 002 section 3.12) adds the project declaration and the tracked
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

/// What a `managed` entry is for (spec 002 section 5, 2026-09-24, provenance
/// item 1).
///
/// The three classes stay as they are; this is a property of a `managed`
/// entry only. An entry recorded before the role existed reads as
/// `reference`, and serializes without the field, so a manifest nobody
/// initialized since stays byte-identical. A role is never inferred from
/// bytes, and changing a recorded one is an operator's act, never automatic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// A file this product maintains: a digest difference is `drifted`.
    #[default]
    Reference,
    /// A file this product seeds for the project to author: written only when
    /// absent, never rewritten, and its digest is the seed. A difference is
    /// `customized`, which is information and not a finding.
    AuthoredInput,
}

impl Role {
    /// True for the default, which is left out of the serialized entry.
    pub fn is_reference(&self) -> bool {
        *self == Role::Reference
    }
}

/// The linked governance producer, as this build links it (spec 002 section
/// 5, 2026-09-24, provenance item 3).
///
/// One identity: the CLI pin a project declares names the same release, so
/// this and [`Pins::spec_spine`] are never two independently qualified
/// identities. Fixed when this product is built, from the lock file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Producer {
    /// The crate name.
    pub name: String,
    /// Its exact version.
    pub version: String,
    /// The crates.io checksum `Cargo.lock` records for it.
    pub checksum: String,
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
/// Spec 002 section 3.21, retained part 3 (it was section 3.7.3 and 3.7.5
/// before section 3.7 was withdrawn): per path, explicit, operator-initiated,
/// reversible, and recorded with the digest observed at the moment of transfer
/// and the producer revision it was evaluated against, so a producer that moves
/// afterwards is a `doctor` finding rather than a silent divergence. Spec 002
/// section 3.35's `transfer apply` is the one operation that creates one, on a
/// move from `user` to `managed`, and it journals the act in
/// [`Manifest::transfers`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transfer {
    /// Who held the path before the transfer.
    pub from: Claimant,
    /// The digest observed at the moment of transfer.
    pub digest_at_transfer: String,
    /// The producer revision the transfer was evaluated against, where one is
    /// known.
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
    /// What a `managed` entry is for. For an `authored-input`, `digest` is
    /// the seed digest. Absent reads as `reference`.
    #[serde(default, skip_serializing_if = "Role::is_reference")]
    pub role: Role,
}

/// The versions an environment was installed against.
///
/// Section 3.3: pins are recorded, never silently satisfied. Nothing in this
/// crate compares a pin and then repairs it; `doctor` reports the mismatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pins {
    /// This product's version at install time.
    pub product: String,
    /// The spec-spine pin the project declares: the version of the exact
    /// `required_version` in its `spec-spine.toml`, or [`UNPINNED`]. Never the
    /// version found on `PATH`, which is an observation and is reported as
    /// one (spec 002 section 5, 2026-09-24, provenance item 4). A declaration
    /// written before that entry may still hold an observation here.
    pub spec_spine: String,
    /// Adapter name to version, ordered so the committed file is stable.
    #[serde(default)]
    pub adapters: BTreeMap<String, String>,
    /// The linked governance producer. Absent from a declaration recorded
    /// before provenance, which says so rather than being given a guess.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer: Option<Producer>,
}

/// What [`Pins::spec_spine`] holds when the project declares no exact pin.
pub const UNPINNED: &str = "unpinned";

/// The spec-spine pin a repository declares, read from the uncommented
/// `required_version` line of `spec-spine.toml`'s `[meta]` table.
///
/// An exact requirement (`=` followed by three numeric parts, the reading
/// spec 002 section 3.23 contract 2 gives the same line) yields its version;
/// no such line, no file, or any other requirement yields [`UNPINNED`],
/// because only an exact requirement pins one release.
pub fn declared_pin(root: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(root.join("spec-spine.toml")) else {
        return UNPINNED.to_string();
    };
    declared_pin_in(&text).unwrap_or_else(|| UNPINNED.to_string())
}

/// [`declared_pin`] over the file's text: the exact version, or `None`.
pub fn declared_pin_in(text: &str) -> Option<String> {
    let mut in_meta = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_meta = line == "[meta]";
            continue;
        }
        if !in_meta || line.starts_with('#') {
            continue;
        }
        let Some(rest) = line.strip_prefix("required_version") else {
            continue;
        };
        let Some(value) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        let value = value.split('#').next().unwrap_or("").trim();
        let value = value.strip_prefix('"')?.strip_suffix('"')?;
        let version = value.strip_prefix('=')?;
        let parts: Vec<&str> = version.split('.').collect();
        let exact = parts.len() == 3
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
        return exact.then(|| version.to_string());
    }
    None
}

/// Whether a project coordinates with a team, and which one.
///
/// Spec 002 section 3.18: enrollment is explicit and project-scoped, it lives in
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
/// Spec 002 section 3.12. It carries what a remote worker needs in order to
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
    /// The command allowance of spec 004 section 3.17: a list of bare program
    /// names a posture allows beyond the adapter's own. Carried here so that a
    /// rewrite of this declaration keeps it; spec 004 reads and validates it,
    /// and this crate supplies the value, not the rule (section 3.16). Absent
    /// is not the same as empty, so an absent member stays absent on write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commands: Option<Vec<String>>,
    /// The selected repository setup profile and the parameters this project
    /// set for it (spec 002 section 5, 2026-09-24, the setup-profile entry).
    /// Absent for a project that selected none, and omitted on write, so a
    /// declaration written before profiles existed is byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<SetupSelection>,
}

/// A selected setup profile, as the declaration records it.
///
/// This crate carries the value; `statecraft-home`'s `setup` module validates
/// the parameters and renders the profile. `parameters` holds only the
/// profile's declared parameters, keyed by their dotted names; an unknown key
/// refuses the plan there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupSelection {
    /// The profile id.
    pub profile: String,
    /// The revision last applied, or planned for the first time.
    pub revision: u32,
    /// The profile's content identity at that revision.
    pub identity: String,
    /// The project's parameter values.
    #[serde(default)]
    pub parameters: BTreeMap<String, serde_json::Value>,
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
/// Spec 002 section 3.13. The distinction from an [`Entry`] is the whole point:
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
    /// Lines this product owns inside files it does not (spec 002 section 3.13).
    #[serde(default)]
    pub modifications: Vec<Modification>,
    /// The project declaration (spec 002 section 3.12).
    #[serde(default)]
    pub project: Project,
    /// The transfer journal (spec 002 section 3.35 rule 5): every applied
    /// ownership transfer and reversal, append-only, in the order applied.
    ///
    /// Kept inside this file so section 3.12's four paths stay four. Absent
    /// from a manifest written before section 3.35, which reads as having no
    /// transfers, and omitted when empty, so a manifest nobody transferred in
    /// is byte-identical to what an earlier build wrote.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transfers: Vec<TransferRecord>,
    /// Which repository's manifest bytes this value was read from, and their
    /// digest: what [`Manifest::write`] checks is still on disk before it
    /// replaces anything. Never serialized, and never part of equality.
    #[serde(skip)]
    origin: Origin,
}

/// Where a manifest value came from.
///
/// `None` for a value that was never read from, or written to, a repository
/// (constructed in memory): nothing is known about what it replaces. Otherwise
/// the root it belongs to and the digest of the bytes read or last written
/// there, or `None` for "there was no manifest".
#[derive(Debug, Default)]
struct Origin(std::sync::Mutex<Option<(PathBuf, Option<String>)>>);

impl Origin {
    fn get(&self) -> Option<(PathBuf, Option<String>)> {
        self.0.lock().map(|g| g.clone()).unwrap_or(None)
    }

    fn set(&self, root: &Path, digest: Option<String>) {
        if let Ok(mut g) = self.0.lock() {
            *g = Some((canonical_root(root), digest));
        }
    }

    /// What this value was read as, when it was read from `root` under any
    /// spelling of it.
    fn read_from(&self, root: &Path) -> Option<Option<String>> {
        let root = canonical_root(root);
        self.get().filter(|(r, _)| *r == root).map(|(_, d)| d)
    }
}

impl Clone for Origin {
    fn clone(&self) -> Self {
        Self(std::sync::Mutex::new(self.get()))
    }
}

impl PartialEq for Origin {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for Origin {}

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
    /// Another writer changed the manifest after this value was read from it.
    /// Nothing was replaced: writing would have erased that writer's change.
    #[error(
        "{path} changed since it was read (read {expected}, now {found}); nothing was replaced, \
         so the other writer's change stands"
    )]
    Changed {
        /// The manifest.
        path: String,
        /// The digest read, or `absent`.
        expected: String,
        /// The digest on disk now, or `absent`.
        found: String,
    },
    /// The manifest is a symbolic link, which a rename would replace with a
    /// file rather than write through.
    #[error("{path} is a symbolic link; it is not replaced")]
    SymbolicLink {
        /// The manifest.
        path: String,
    },
    /// The filesystem holding the lock file cannot take an advisory lock, so
    /// the one-writer guarantee cannot be kept and nothing is written.
    #[error(
        "the manifest lock {path} cannot be taken on this filesystem ({detail}); nothing was \
         written, because writing without it could erase another writer's change"
    )]
    LockUnsupported {
        /// The lock file.
        path: String,
        /// What the filesystem answered.
        detail: String,
    },
    /// Another process holds the manifest lock.
    #[error("another process holds the manifest lock on {root}")]
    Busy {
        /// The repository.
        root: String,
    },
    /// The new manifest is in force, but the directory holding it could not be
    /// flushed, so the rename is not known to survive a crash.
    #[error(
        "the new {path} is in force, but its directory could not be flushed to stable storage \
         ({detail}), so it is not known to survive a crash"
    )]
    NotDurable {
        /// The manifest.
        path: String,
        /// Why.
        detail: String,
    },
}

/// What a manifest write did besides writing.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Written {
    /// Temporary files an interrupted earlier write left beside the manifest,
    /// removed by this one, by name.
    pub removed_leftovers: Vec<String>,
}

/// How long a writer waits for another writer before refusing.
pub const WRITER_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

std::thread_local! {
    static HELD: std::cell::RefCell<Vec<PathBuf>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The spelling of a root the lock and the changed-since-read check both key
/// on: canonical, so `/tmp/x` and `/private/tmp/x` are one repository. A root
/// that cannot be canonicalized (it does not exist) keys as given.
fn canonical_root(root: &Path) -> PathBuf {
    std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
}

/// Where the manifest lock lives: runtime state, ignored, created on demand.
pub const LOCK_PATH: &str = ".statecraft/state/manifest.lock";

/// The manifest lock of one repository, held until dropped.
///
/// An advisory `flock` on [`LOCK_PATH`], a file in the repository's runtime
/// state, which is gitignored, created on demand and never replaced, so the
/// committed tree gains no byte. A lock file rather than the root directory:
/// `flock` on a directory fails where it is emulated with byte-range locks
/// (NFS, some SMB and FUSE mounts), and a user's own `flock .` would contend
/// with it. The operating system releases it when the holder ends, however it
/// ends. Reentrant within a thread: an operation that holds it may call
/// another that takes it. A filesystem that cannot lock is
/// [`ManifestError::LockUnsupported`], never a silent write without the lock.
#[derive(Debug)]
pub struct ManifestLock {
    #[cfg(unix)]
    _file: Option<std::fs::File>,
    key: Option<PathBuf>,
}

/// Release explicitly, then close. A `flock` belongs to the open file
/// description, and a child spawned from another thread holds a copy of the
/// descriptor until its `exec`; releasing by closing alone would leave the lock
/// held for that window, and the next writer would be told another process
/// holds it. An explicit unlock releases it for every copy at once (the same
/// repair spec 003 section 5 records for the repository lock).
impl Drop for ManifestLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(file) = &self._file {
            let _ = rustix::fs::flock(file, rustix::fs::FlockOperation::Unlock);
        }
        if let Some(key) = self.key.take() {
            HELD.with(|h| h.borrow_mut().retain(|k| *k != key));
        }
    }
}

/// Take the manifest lock of `root`, waiting up to `wait` for another holder.
#[cfg(unix)]
pub fn lock(root: &Path, wait: std::time::Duration) -> Result<ManifestLock, ManifestError> {
    let io = |source| ManifestError::Io {
        path: root.display().to_string(),
        source,
    };
    let key = std::fs::canonicalize(root).map_err(io)?;
    if HELD.with(|h| h.borrow().contains(&key)) {
        return Ok(ManifestLock {
            _file: None,
            key: None,
        });
    }
    // The lock file is created inside the repository or not at all: a linked
    // `.statecraft` or `.statecraft/state` would put it wherever the link
    // points, so either one is refused, and the file itself is opened without
    // following a link.
    for dir in [".statecraft", ".statecraft/state"] {
        match std::fs::symlink_metadata(key.join(dir)) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(ManifestError::SymbolicLink {
                    path: dir.to_string(),
                });
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io(e)),
        }
    }
    let path = key.join(LOCK_PATH);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    let file = {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .open(&path)
            .map_err(|source| ManifestError::Io {
                path: LOCK_PATH.to_string(),
                source,
            })?
    };
    let deadline = std::time::Instant::now() + wait;
    loop {
        match rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => break,
            Err(rustix::io::Errno::WOULDBLOCK) => {
                if std::time::Instant::now() >= deadline {
                    return Err(ManifestError::Busy {
                        root: root.display().to_string(),
                    });
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(
                e @ (rustix::io::Errno::OPNOTSUPP
                | rustix::io::Errno::NOLCK
                | rustix::io::Errno::NOSYS
                | rustix::io::Errno::BADF
                | rustix::io::Errno::INVAL),
            ) => {
                return Err(ManifestError::LockUnsupported {
                    path: path.display().to_string(),
                    detail: std::io::Error::from(e).to_string(),
                });
            }
            Err(e) => return Err(io(std::io::Error::from(e))),
        }
    }
    HELD.with(|h| h.borrow_mut().push(key.clone()));
    Ok(ManifestLock {
        _file: Some(file),
        key: Some(key),
    })
}

/// Without an advisory lock the one-writer guarantee cannot be kept, so every
/// write refuses rather than pretending to.
#[cfg(not(unix))]
pub fn lock(root: &Path, _wait: std::time::Duration) -> Result<ManifestLock, ManifestError> {
    Err(ManifestError::Io {
        path: root.display().to_string(),
        source: std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "the manifest lock needs an advisory file lock, which this platform lacks here",
        ),
    })
}

fn absent_or(d: &Option<String>) -> String {
    d.clone().unwrap_or_else(|| "absent".to_string())
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
            transfers: Vec::new(),
            origin: Origin::default(),
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
    pub fn read(root: &Path) -> Result<Option<Self>, ManifestError> {
        Ok(Self::read_tracked(root)?.map(|(m, _)| m))
    }

    /// [`Manifest::read`], and the digest of the bytes it parsed.
    ///
    /// The value remembers both, and [`Manifest::write`] refuses to replace a
    /// manifest that no longer has that digest.
    pub fn read_tracked(root: &Path) -> Result<Option<(Self, String)>, ManifestError> {
        let Some(bytes) = Self::read_bytes(root)? else {
            return Ok(None);
        };
        let digest = crate::digest::digest_bytes(&bytes);
        let manifest = Self::parse(&bytes)?;
        manifest.origin.set(root, Some(digest.clone()));
        Ok(Some((manifest, digest)))
    }

    /// Mark this value as replacing whatever manifest `root` held when
    /// [`Manifest::read`] last found it, or, for a value made with
    /// [`Manifest::new`] after a read found none, as replacing nothing.
    pub fn belongs_to(&self, root: &Path, read_digest: Option<String>) {
        self.origin.set(root, read_digest);
    }

    /// Refuse, before anything is done, when the manifest on disk is not the
    /// one this value was read from. A value never read from `root` passes.
    pub fn ensure_current(&self, root: &Path) -> Result<(), ManifestError> {
        let Some(expected) = self.origin.read_from(root) else {
            return Ok(());
        };
        let found = Self::read_bytes(root)?.map(|b| crate::digest::digest_bytes(&b));
        if found != expected {
            return Err(ManifestError::Changed {
                path: MANIFEST_PATH.to_string(),
                expected: absent_or(&expected),
                found: absent_or(&found),
            });
        }
        Ok(())
    }

    /// Read, modify and write the manifest under the manifest lock.
    ///
    /// Every writer of the manifest goes through here or through
    /// [`Manifest::write`], which takes the same lock: the read happens under
    /// the lock, so no other writer's change can land between it and the
    /// write. `f` is given the manifest (or `None`, when there is none) and
    /// returns the manifest to write, or `None` to write nothing.
    pub fn update<T, E: From<ManifestError>>(
        root: &Path,
        wait: std::time::Duration,
        f: impl FnOnce(Option<Manifest>) -> Result<(Option<Manifest>, T), E>,
    ) -> Result<T, E> {
        let _held = lock(root, wait)?;
        let read = Self::read_tracked(root)?;
        let read_digest = read.as_ref().map(|(_, d)| d.clone());
        let (write, out) = f(read.map(|(m, _)| m))?;
        if let Some(m) = write {
            if m.origin.get().is_none() {
                m.belongs_to(root, read_digest);
            }
            m.write(root)?;
        }
        Ok(out)
    }

    /// The manifest's bytes exactly as committed, or `None` when there is no
    /// manifest.
    ///
    /// For a caller that needs the digest of the same bytes it parses (spec
    /// 002 section 3.35 binds a plan to the manifest's digest): reading twice
    /// would let a write land between the digest and the parse.
    pub fn read_bytes(root: &Path) -> Result<Option<Vec<u8>>, ManifestError> {
        let path = crate::claimant::resolve(root, MANIFEST_PATH);
        match std::fs::read(&path) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(ManifestError::Io {
                path: MANIFEST_PATH.to_string(),
                source,
            }),
        }
    }

    /// Parse manifest bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, ManifestError> {
        let display = MANIFEST_PATH.to_string();
        // Read the version before the body, so a future schema fails with the
        // version it found rather than with a field-level deserialization error
        // that makes the cause look like corruption.
        let probe: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|source| ManifestError::Malformed {
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
        serde_json::from_slice(bytes).map_err(|source| ManifestError::Malformed {
            path: display,
            source,
        })
    }

    /// The bytes [`Manifest::write`] would commit.
    ///
    /// Pretty-printed with a trailing newline, because it is committed and a
    /// human reads its diff. Refuses a declaration naming one machine's
    /// filesystem (spec 002 section 3.12), because the committed file is exactly
    /// what a remote worker has to resolve from.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ManifestError> {
        let violations = self.project.portability_violations();
        if !violations.is_empty() {
            return Err(ManifestError::NonPortable {
                path: MANIFEST_PATH.to_string(),
                violations,
            });
        }
        let mut json =
            serde_json::to_string_pretty(self).map_err(|source| ManifestError::Malformed {
                path: MANIFEST_PATH.to_string(),
                source,
            })?;
        json.push('\n');
        Ok(json.into_bytes())
    }

    /// Write the manifest into a target repository root, durably and
    /// atomically, under the manifest lock.
    ///
    /// The bytes go to a temporary file beside the manifest, with the old
    /// file's permissions, which is flushed to stable storage and then renamed
    /// over it; the directory is flushed after the rename. An interruption
    /// before the rename leaves the old manifest; after it, the new one. A
    /// manifest that is a symbolic link is refused rather than replaced. A
    /// manifest that changed since this value was read from `root` is refused
    /// rather than overwritten. Temporary files an earlier interrupted write
    /// left behind are removed and named in the answer, so they never stay in
    /// the project area. When the directory flush fails after the rename the
    /// new manifest is in force and the answer says so ([`ManifestError::NotDurable`]).
    pub fn write(&self, root: &Path) -> Result<Written, ManifestError> {
        let staged = self.stage(root)?;
        let digest = staged.digest.clone();
        let result = staged.commit();
        if matches!(result, Ok(_) | Err(ManifestError::NotDurable { .. })) {
            self.origin.set(root, Some(digest));
        }
        result
    }

    /// The first half of [`Manifest::write`]: the lock taken, leftovers
    /// removed, and the new bytes durable in a temporary file beside the
    /// manifest, not yet in force.
    ///
    /// Dropping the result without [`Staged::commit`] removes the temporary
    /// file and leaves the manifest as it was, which is what an interrupted
    /// write looks like to every reader.
    pub fn stage(&self, root: &Path) -> Result<Staged, ManifestError> {
        let bytes = self.to_bytes()?;
        let target = crate::claimant::resolve(root, MANIFEST_PATH);
        let io = |source| ManifestError::Io {
            path: MANIFEST_PATH.to_string(),
            source,
        };
        let dir = target
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| root.to_path_buf());
        std::fs::create_dir_all(&dir).map_err(io)?;
        let held = lock(root, WRITER_WAIT)?;

        let mode = match std::fs::symlink_metadata(&target) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(ManifestError::SymbolicLink {
                    path: MANIFEST_PATH.to_string(),
                });
            }
            Ok(m) => Some(m.permissions()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(io(e)),
        };

        let mut removed_leftovers = Vec::new();
        for entry in std::fs::read_dir(&dir).map_err(io)? {
            let entry = entry.map_err(io)?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(TEMP_PREFIX) && name.ends_with(TEMP_SUFFIX) {
                std::fs::remove_file(entry.path()).map_err(io)?;
                removed_leftovers.push(name);
            }
        }
        removed_leftovers.sort();

        let temp = dir.join(format!(
            "{TEMP_PREFIX}{}.{}{TEMP_SUFFIX}",
            std::process::id(),
            unique_suffix()
        ));
        let staged = Staged {
            temp,
            target,
            dir,
            committed: false,
            digest: crate::digest::digest_bytes(&bytes),
            origin: self.origin.read_from(root),
            root: root.to_path_buf(),
            written: Written { removed_leftovers },
            _held: held,
        };
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged.temp)
            .map_err(io)?;
        if let Some(mode) = mode {
            file.set_permissions(mode).map_err(io)?;
        }
        file.write_all(&bytes).map_err(io)?;
        file.sync_all().map_err(io)?;
        Ok(staged)
    }
}

/// How a temporary manifest file's name begins and ends.
const TEMP_PREFIX: &str = ".environment.json.";
const TEMP_SUFFIX: &str = ".tmp";

/// A distinct suffix per staged write within one process.
fn unique_suffix() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// A manifest written to a temporary file and not yet renamed into place.
#[derive(Debug)]
pub struct Staged {
    temp: PathBuf,
    target: PathBuf,
    dir: PathBuf,
    committed: bool,
    digest: String,
    /// What the value was read as, when it was read from this root.
    origin: Option<Option<String>>,
    root: PathBuf,
    written: Written,
    _held: ManifestLock,
}

impl Staged {
    /// Where the staged bytes are.
    pub fn temp_path(&self) -> &Path {
        &self.temp
    }

    /// Put the staged bytes in force: check the manifest is still the one
    /// read, rename, then flush the directory so the rename itself survives a
    /// crash.
    pub fn commit(mut self) -> Result<Written, ManifestError> {
        let io = |source| ManifestError::Io {
            path: MANIFEST_PATH.to_string(),
            source,
        };
        if let Some(expected) = &self.origin {
            let found = Manifest::read_bytes(&self.root)?.map(|b| crate::digest::digest_bytes(&b));
            if &found != expected {
                return Err(ManifestError::Changed {
                    path: MANIFEST_PATH.to_string(),
                    expected: absent_or(expected),
                    found: absent_or(&found),
                });
            }
        }
        std::fs::rename(&self.temp, &self.target).map_err(io)?;
        self.committed = true;
        sync_dir(&self.dir).map_err(|e| ManifestError::NotDurable {
            path: MANIFEST_PATH.to_string(),
            detail: e.to_string(),
        })?;
        Ok(std::mem::take(&mut self.written))
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_file(&self.temp);
        }
    }
}

/// Flush a directory's entries to stable storage.
///
/// On Unix a directory opens read-only and `fsync` on it makes a rename in it
/// durable. Elsewhere a directory cannot be opened as a file, and the rename is
/// as durable as the platform makes it.
fn sync_dir(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::fs::File::open(dir)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        Ok(())
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
            producer: None,
        }
    }

    #[test]
    fn only_an_exact_required_version_in_meta_is_a_declared_pin() {
        let pin = |t: &str| declared_pin_in(t);
        assert_eq!(
            pin("[meta]\nrequired_version = \"=0.25.0\"\n"),
            Some("0.25.0".into())
        );
        assert_eq!(
            pin("[meta]\nrequired_version = \"=0.25.0\" # adopted\n"),
            Some("0.25.0".into())
        );
        // Commented, as the producer's scaffold writes it today.
        assert_eq!(pin("[meta]\n# required_version = \"=0.25.0\"\n"), None);
        // Not exact: a caret, a two-part version, a range.
        assert_eq!(pin("[meta]\nrequired_version = \"0.25.0\"\n"), None);
        assert_eq!(pin("[meta]\nrequired_version = \"=0.25\"\n"), None);
        assert_eq!(pin("[meta]\nrequired_version = \">=0.25.0\"\n"), None);
        // Another table's key is not the pin.
        assert_eq!(pin("[other]\nrequired_version = \"=0.25.0\"\n"), None);
        let root = tempfile::tempdir().unwrap();
        assert_eq!(declared_pin(root.path()), UNPINNED);
    }

    #[test]
    fn a_reference_role_and_an_absent_producer_leave_the_bytes_as_they_were() {
        let e = entry("a.md");
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("role"));
        let back: Entry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.role, Role::Reference);
        let authored = Entry {
            role: Role::AuthoredInput,
            ..entry("b.md")
        };
        assert!(
            serde_json::to_string(&authored)
                .unwrap()
                .contains("\"role\":\"authored-input\"")
        );
        let p = pins();
        assert!(!serde_json::to_string(&p).unwrap().contains("producer"));
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
            role: Role::Reference,
        }
    }

    #[test]
    fn a_declared_command_allowance_survives_a_rewrite_and_an_absent_one_stays_absent() {
        // Spec 004 section 3.17 rule 1 reads `project.commands`; a rewrite of
        // the declaration by this crate must neither drop it nor invent one.
        let dir = tempfile::tempdir().unwrap();
        let mut m = Manifest::new(pins());
        m.write(dir.path()).unwrap();
        let text = std::fs::read_to_string(dir.path().join(MANIFEST_PATH)).unwrap();
        assert!(!text.contains("commands"), "{text}");
        m.project.commands = Some(vec!["cargo".into(), "make".into()]);
        m.write(dir.path()).unwrap();
        let read = Manifest::read(dir.path()).unwrap().unwrap();
        assert_eq!(read.project.commands, m.project.commands);
        read.write(dir.path()).unwrap();
        let again = Manifest::read(dir.path()).unwrap().unwrap();
        assert_eq!(again.project.commands, m.project.commands);
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
    fn a_manifest_with_no_journal_reads_as_having_no_transfers_and_writes_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = crate::claimant::resolve(dir.path(), MANIFEST_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{"version":2,"pins":{"product":"0","spec_spine":"0"},"entries":[]}"#,
        )
        .unwrap();
        let m = Manifest::read(dir.path()).unwrap().unwrap();
        assert!(m.transfers.is_empty());
        // Skipped when empty, so a manifest nobody transferred in is what an
        // earlier build would have written.
        let text = String::from_utf8(m.to_bytes().unwrap()).unwrap();
        assert!(!text.contains("transfers"), "{text}");
    }

    #[test]
    fn a_staged_write_dropped_before_commit_leaves_the_old_manifest_and_no_temporary() {
        let dir = tempfile::tempdir().unwrap();
        let old = Manifest::new(pins());
        old.write(dir.path()).unwrap();
        let before = std::fs::read(crate::claimant::resolve(dir.path(), MANIFEST_PATH)).unwrap();

        let mut new = Manifest::new(pins());
        new.upsert(entry("a"));
        let staged = new.stage(dir.path()).unwrap();
        let temp = staged.temp_path().to_path_buf();
        assert!(temp.exists(), "the new bytes are staged");
        // Interrupted here: a reader still sees the old manifest.
        assert_eq!(Manifest::read(dir.path()).unwrap().unwrap(), old);
        drop(staged);
        assert!(!temp.exists());
        assert_eq!(
            std::fs::read(crate::claimant::resolve(dir.path(), MANIFEST_PATH)).unwrap(),
            before
        );

        new.stage(dir.path()).unwrap().commit().unwrap();
        assert_eq!(Manifest::read(dir.path()).unwrap().unwrap(), new);
        // Only the manifest, beside the runtime state holding the lock file.
        let mut names: Vec<_> = std::fs::read_dir(dir.path().join(".statecraft"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        assert_eq!(
            names,
            ["environment.json", "state"],
            "only the manifest remains"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_write_keeps_the_old_files_mode() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let m = Manifest::new(pins());
        m.write(dir.path()).unwrap();
        let path = crate::claimant::resolve(dir.path(), MANIFEST_PATH);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        let mut m = Manifest::read(dir.path()).unwrap().unwrap();
        m.upsert(entry("a"));
        m.write(dir.path()).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);
    }

    #[cfg(unix)]
    #[test]
    fn a_manifest_that_is_a_symbolic_link_is_refused_not_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = dir.path().join("elsewhere.json");
        Manifest::new(pins()).write(dir.path()).unwrap();
        let path = crate::claimant::resolve(dir.path(), MANIFEST_PATH);
        std::fs::rename(&path, &elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &path).unwrap();
        let before = std::fs::read(&elsewhere).unwrap();
        let mut m = Manifest::new(pins());
        m.upsert(entry("a"));
        assert!(matches!(
            m.write(dir.path()),
            Err(ManifestError::SymbolicLink { .. })
        ));
        assert!(
            std::fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read(&elsewhere).unwrap(), before);
    }

    #[test]
    fn a_leftover_temporary_is_removed_by_the_next_write_and_named() {
        let dir = tempfile::tempdir().unwrap();
        Manifest::new(pins()).write(dir.path()).unwrap();
        let area = dir.path().join(".statecraft");
        std::fs::write(area.join(".environment.json.999.0.tmp"), b"torn").unwrap();
        let m = Manifest::read(dir.path()).unwrap().unwrap();
        let written = m.write(dir.path()).unwrap();
        assert_eq!(written.removed_leftovers, [".environment.json.999.0.tmp"]);
        let mut names: Vec<_> = std::fs::read_dir(&area)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        assert_eq!(names, ["environment.json", "state"]);
    }

    #[test]
    fn a_manifest_changed_since_it_was_read_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        Manifest::new(pins()).write(dir.path()).unwrap();
        let mut stale = Manifest::read(dir.path()).unwrap().unwrap();
        let mut other = Manifest::read(dir.path()).unwrap().unwrap();
        other.upsert(entry("theirs"));
        other.write(dir.path()).unwrap();
        stale.upsert(entry("mine"));
        assert!(matches!(
            stale.ensure_current(dir.path()),
            Err(ManifestError::Changed { .. })
        ));
        assert!(matches!(
            stale.write(dir.path()),
            Err(ManifestError::Changed { .. })
        ));
        assert!(
            Manifest::read(dir.path())
                .unwrap()
                .unwrap()
                .records("theirs")
        );
        // A value that wrote keeps writing: its origin moves with it.
        other.upsert(entry("again"));
        other.write(dir.path()).unwrap();
    }

    // The changed-since-read check keys on the canonical root the lock uses,
    // so a second spelling of the same repository cannot slip past it.
    #[cfg(unix)]
    #[test]
    fn a_stale_value_is_refused_under_another_spelling_of_its_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let alias = dir.path().join("alias");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        Manifest::new(pins()).write(&root).unwrap();

        let mut stale = Manifest::read(&root).unwrap().unwrap();
        let mut other = Manifest::read(&alias).unwrap().unwrap();
        other.upsert(entry("theirs"));
        other.write(&alias).unwrap();
        stale.upsert(entry("mine"));
        for spelling in [&alias, &root] {
            assert!(
                matches!(stale.write(spelling), Err(ManifestError::Changed { .. })),
                "{}",
                spelling.display()
            );
        }
        // On macOS the temporary directory is itself a second spelling
        // (`/var` for `/private/var`); the check holds across it too.
        let canonical = std::fs::canonicalize(&root).unwrap();
        assert!(matches!(
            stale.write(&canonical),
            Err(ManifestError::Changed { .. })
        ));
        assert!(Manifest::read(&root).unwrap().unwrap().records("theirs"));
    }

    // Another holder past the wait is `Busy`, which a writer reports as a
    // refusal; the lock file is runtime state, not a committed byte.
    #[cfg(unix)]
    #[test]
    fn a_held_lock_is_busy_after_the_wait_and_lives_in_runtime_state() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let (taken, release) = (std::sync::mpsc::channel(), std::sync::mpsc::channel::<()>());
        let holder_root = root.clone();
        let holder = std::thread::spawn(move || {
            let _held = lock(&holder_root, std::time::Duration::ZERO).unwrap();
            taken.0.send(()).unwrap();
            release.1.recv().unwrap();
        });
        taken.1.recv().unwrap();
        assert!(matches!(
            lock(&root, std::time::Duration::from_millis(50)),
            Err(ManifestError::Busy { .. })
        ));
        release.0.send(()).unwrap();
        holder.join().unwrap();
        assert!(root.join(LOCK_PATH).is_file());
    }

    // The lock file is created inside the repository or not at all: a linked
    // state directory, a linked `.statecraft`, or a link at the lock file's
    // own path is refused, and nothing is created where the link points.
    #[cfg(unix)]
    #[test]
    fn a_linked_state_directory_or_lock_file_is_refused_and_nothing_is_created_elsewhere() {
        let elsewhere = tempfile::tempdir().unwrap();
        let empty = |d: &Path| std::fs::read_dir(d).unwrap().next().is_none();
        for linked in [".statecraft", ".statecraft/state"] {
            let dir = tempfile::tempdir().unwrap();
            let link = dir.path().join(linked);
            std::fs::create_dir_all(link.parent().unwrap()).unwrap();
            std::os::unix::fs::symlink(elsewhere.path(), &link).unwrap();
            assert!(
                matches!(
                    lock(dir.path(), std::time::Duration::ZERO),
                    Err(ManifestError::SymbolicLink { .. })
                ),
                "{linked}"
            );
            assert!(empty(elsewhere.path()), "{linked}");
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".statecraft/state")).unwrap();
        std::os::unix::fs::symlink(
            elsewhere.path().join("planted.lock"),
            dir.path().join(LOCK_PATH),
        )
        .unwrap();
        assert!(lock(dir.path(), std::time::Duration::ZERO).is_err());
        assert!(empty(elsewhere.path()));
    }

    // The copy a spawned child holds between its creation and its `exec` is a
    // second descriptor on the same open file description. Held here
    // deterministically, as a duplicate that outlives the holder: the next
    // writer, waiting for no one, still takes the lock.
    #[cfg(unix)]
    #[test]
    fn a_copy_of_the_descriptor_does_not_keep_a_released_lock() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let held = lock(&root, std::time::Duration::ZERO).unwrap();
        let copy = held._file.as_ref().unwrap().try_clone().unwrap();
        drop(held);
        let other = root.clone();
        let taken = std::thread::spawn(move || lock(&other, std::time::Duration::ZERO).is_ok())
            .join()
            .unwrap();
        assert!(taken, "a released lock was still held through a copy");
        drop(copy);
    }

    #[test]
    fn update_reads_and_writes_under_the_lock() {
        let dir = tempfile::tempdir().unwrap();
        let created: Result<(), ManifestError> = Manifest::update(dir.path(), WRITER_WAIT, |m| {
            assert!(m.is_none());
            Ok((Some(Manifest::new(pins())), ()))
        });
        created.unwrap();
        let r: Result<bool, ManifestError> = Manifest::update(dir.path(), WRITER_WAIT, |m| {
            let mut m = m.unwrap();
            m.upsert(entry("a"));
            Ok((Some(m), true))
        });
        assert!(r.unwrap());
        assert!(Manifest::read(dir.path()).unwrap().unwrap().records("a"));
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
