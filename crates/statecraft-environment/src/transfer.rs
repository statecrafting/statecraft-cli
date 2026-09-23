//! Per-path ownership transfer, as an operator's act.
//!
//! Spec 002 section 3.35. Section 3.21 retains, from the withdrawn section 3.7,
//! that ownership transfer is per path, explicit, operator-initiated,
//! reversible, and recorded with the digest observed at the moment of transfer
//! and the producer revision it was evaluated against. This module is that
//! operation: a plan that writes nothing ([`plan`]), an apply bound to the
//! plan's identity ([`apply`]), and a reversal bound to the record it reverses
//! ([`revert`]).
//!
//! Nothing here reads a file's bytes for anything but its digest, nothing
//! writes a byte of any file but the manifest, and nothing moves a path
//! because of what its bytes resemble (rule 2). Every refusal names the rule
//! it enforces and writes nothing.

use crate::adapter::{Declaration, HarnessProbe, Readiness, readiness};
use crate::claimant::{Claimant, ForeignClaims, resolve};
use crate::digest::digest_bytes;
use crate::manifest::{
    Class, Entry, MANIFEST_PATH, Manifest, ManifestError, Source, SourceKind, Written,
};
use crate::time::{Clock, rfc3339_utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The source identity an entry adopted by transfer records.
///
/// An adopted file has no source in this product: it existed before and is
/// never rewritten. The entry format requires one, so the operator's act is
/// named as the source, under the existing `template` kind, which keeps the
/// manifest readable by a build that predates section 3.35 (its compatibility
/// paragraph assumes one can).
pub const TRANSFER_SOURCE: &str = "operator-transfer";

/// The word every record carries beside the operator's name: supplied, never
/// authenticated (rule 4).
pub const OPERATOR_PROVENANCE: &str = "operator-supplied";

/// The user instruction files section 3.8 keeps `user`: a closed list, by file
/// name at any depth (rule 2).
pub const INSTRUCTION_FILES: [&str; 5] = [
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".cursorrules",
    ".windsurfrules",
];

/// The one instruction file the list names with its directory.
pub const COPILOT_INSTRUCTIONS: [&str; 2] = [".github", "copilot-instructions.md"];

/// How a plan identity token begins.
pub const PLAN_TOKEN_PREFIX: &str = "pi1";

/// How many hex digits of each component digest a plan token carries.
const SHORT: usize = 16;

/// The three ownership classes of section 3.2, as a transfer names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ownership {
    /// Not in the manifest.
    User,
    /// Recorded as adopted.
    Adopted,
    /// Recorded as managed.
    Managed,
}

impl Ownership {
    /// Read a class word: `user`, `adopted` or `managed`.
    pub fn from_word(word: &str) -> Option<Self> {
        match word {
            "user" => Some(Self::User),
            "adopted" => Some(Self::Adopted),
            "managed" => Some(Self::Managed),
            _ => None,
        }
    }

    /// The word.
    pub fn word(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Adopted => "adopted",
            Self::Managed => "managed",
        }
    }

    /// The class the manifest records for a path. Absent is `user`.
    pub fn of(manifest: &Manifest, path: &str) -> Self {
        match manifest.entry(path).map(|e| e.class) {
            None => Self::User,
            Some(Class::Adopted) => Self::Adopted,
            Some(Class::Managed) => Self::Managed,
        }
    }

    /// Rule 1's four moves. `adopted` to `managed` and back is two transfers
    /// through `user`.
    pub fn admitted(from: Self, to: Self) -> bool {
        matches!(
            (from, to),
            (Self::User, Self::Adopted)
                | (Self::User, Self::Managed)
                | (Self::Adopted, Self::User)
                | (Self::Managed, Self::User)
        )
    }
}

/// One journal record (rule 5): an applied transfer or a reversal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferRecord {
    /// Its identity: the SHA-256 of every other field.
    pub id: String,
    /// The repository-relative path.
    pub path: String,
    /// The class it had.
    pub from: Ownership,
    /// The class it took.
    pub to: Ownership,
    /// The file's digest when the transfer was applied.
    pub digest: String,
    /// Its length in bytes.
    pub bytes: u64,
    /// The producer revision the transfer was evaluated against: the
    /// `spec-spine-core` version this build links.
    pub producer: String,
    /// Who asked, as supplied.
    pub operator: String,
    /// Always [`OPERATOR_PROVENANCE`].
    pub operator_provenance: String,
    /// Why.
    pub reason: String,
    /// When, RFC 3339 UTC.
    pub at: String,
    /// The SHA-256 of the manifest's bytes before this record was applied.
    pub manifest_before: String,
    /// The record this one reverses, for a reversal (rule 6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverts: Option<String>,
}

impl TransferRecord {
    /// The identity of a record's content.
    fn identity(&self) -> String {
        let content = serde_json::json!([
            self.path,
            self.from,
            self.to,
            self.digest,
            self.bytes,
            self.producer,
            self.operator,
            self.operator_provenance,
            self.reason,
            self.at,
            self.manifest_before,
            self.reverts,
        ]);
        digest_bytes(content.to_string().as_bytes())
    }
}

/// Which rule a refusal enforces, as a word a caller can script against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RefusalKind {
    /// Rule 1: a move the table does not admit.
    MoveNotAdmitted,
    /// Rule 3: not a relative, forward-slash path.
    NotRelative,
    /// Rule 3: an empty, `.` or `..` component.
    Escaping,
    /// Rule 3: under a `.statecraft` or a `.git` component.
    Protected,
    /// Rule 3: a component is a symbolic link.
    SymbolicLink,
    /// Rule 3: a directory.
    Directory,
    /// Rule 3: not a regular file (a fifo, a socket, a device).
    NotARegularFile,
    /// Rule 3 or 6: the file does not exist.
    Missing,
    /// Rule 3: a component is spelled differently from how the directory
    /// holding it spells it (case or Unicode normalization), so one file could
    /// be recorded under two names.
    Spelling,
    /// Rule 3: the path names the same file as another path the manifest, the
    /// journal or an adapter already records under a different name.
    Alias,
    /// Rule 2: a user instruction file.
    InstructionFile,
    /// Rule 2: the root instruction bridge is a modification, not an entry.
    Modification,
    /// Rule 1: `managed` needs a source, and no installed adapter declares it.
    NoSource,
    /// Rule 1: the adapter that declares the path does not claim its paths
    /// here, so this product would not itself write it.
    AdapterNotClaiming,
    /// Rule 7: the named current class is not the path's class.
    ClassMismatch,
    /// Rule 4: the plan identity is not the current one.
    StalePlan,
    /// Rule 5: the journal disagrees with the entries.
    JournalDisagrees,
    /// There is no manifest to record a transfer in.
    NoManifest,
    /// Rule 6: no such transfer is recorded.
    UnknownTransfer,
    /// Rule 6: a later transfer names the same path.
    LaterTransfer,
    /// Rule 6: the path's class is no longer the one the transfer produced.
    ClassChanged,
    /// Rule 6: the file changed without this product recording it.
    UnrecordedChange,
    /// An empty operator or reason.
    MissingOperatorOrReason,
    /// Another process holds the manifest lock.
    Busy,
}

/// A refusal: which rule, and what it found. Nothing was written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refusal {
    /// The rule, as a word.
    pub kind: RefusalKind,
    /// What was found, naming the path, the class or the record involved.
    pub detail: String,
    /// For a stale plan, which of the plan's inputs changed: `classes`,
    /// `path`, `file`, `manifest`, or `identity` for a token this build did
    /// not issue.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub changed: Vec<String>,
}

/// Why a transfer operation did not complete.
#[derive(Debug, thiserror::Error)]
pub enum TransferError {
    /// A precondition failed and nothing was written.
    #[error("{}; nothing was written", .0.detail)]
    Refused(Refusal),
    /// The manifest could not be read or written durably.
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    /// The transfer is in force, but the manifest's directory could not be
    /// flushed, so it is not known to survive a crash.
    #[error("{outcome_word}, but not durably: {detail}", outcome_word = .outcome.word())]
    NotDurable {
        /// What is in force.
        outcome: Box<Outcome>,
        /// Why durability is not known.
        detail: String,
    },
    /// A file could not be read.
    #[error("{path} could not be read: {detail}")]
    Io {
        /// Which.
        path: String,
        /// Why.
        detail: String,
    },
}

fn refuse<T>(kind: RefusalKind, detail: impl Into<String>) -> Result<T, TransferError> {
    Err(TransferError::Refused(Refusal {
        kind,
        detail: detail.into(),
        changed: Vec::new(),
    }))
}

fn io_error(path: &str, e: impl std::fmt::Display) -> TransferError {
    TransferError::Io {
        path: path.to_string(),
        detail: e.to_string(),
    }
}

/// What the caller established.
#[derive(Clone, Copy)]
pub struct Context<'a> {
    /// The repository root.
    pub root: &'a Path,
    /// The producer revision this build links, as `name@version`.
    pub producer: &'a str,
    /// The installed adapters: the only sources a `managed` entry can name.
    pub declarations: &'a [Declaration],
    /// What decides whether an installed adapter claims its paths here.
    pub probe: &'a dyn HarnessProbe,
    /// Who else claims paths, for naming a prior claimant.
    pub foreign: &'a ForeignClaims,
}

impl Context<'_> {
    /// The installed adapter that declares a path, if any.
    fn adapter_for(&self, path: &str) -> Option<&Declaration> {
        self.declarations
            .iter()
            .find(|d| d.paths().any(|p| p == path))
    }
}

/// Who asked and why.
#[derive(Debug, Clone, Copy)]
pub struct Act<'a> {
    /// Who, as supplied. Recorded verbatim and not authenticated.
    pub operator: &'a str,
    /// Why.
    pub reason: &'a str,
}

/// What the manifest and the file say about a path now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Standing {
    /// The class the manifest records (`user` when it records none).
    pub class: Ownership,
    /// The digest the manifest records for it, where it records one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recorded_digest: Option<String>,
    /// Whether the file's bytes are the bytes the manifest records, where it
    /// records any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matches_record: Option<bool>,
    /// The installed adapter that declares the path, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_by: Option<String>,
}

/// The entry a transfer would leave, before it has a time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResultingEntry {
    /// Managed or adopted.
    pub class: Class,
    /// Its source.
    pub source: Source,
    /// The digest recorded.
    pub digest: String,
    /// The length recorded.
    pub bytes: u64,
    /// The transfer the entry carries, for a move to `managed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transfer: Option<crate::manifest::Transfer>,
}

impl ResultingEntry {
    fn into_entry(self, path: &str, at: &str) -> Entry {
        Entry {
            path: path.to_string(),
            class: self.class,
            source: self.source,
            digest: self.digest,
            bytes: self.bytes,
            written_at: at.to_string(),
            transfer: self.transfer,
        }
    }
}

/// What `transfer plan` reports (rule 4). Nothing is written to compute it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Plan {
    /// The path.
    pub path: String,
    /// The class named as current.
    pub from: Ownership,
    /// The class it is to take.
    pub to: Ownership,
    /// The path's current class, as the manifest and the file say.
    pub current: Standing,
    /// The file's digest now.
    pub digest: String,
    /// Its length now.
    pub bytes: u64,
    /// The manifest's digest now.
    pub manifest_digest: String,
    /// The entry the manifest would hold afterwards; `None` for `user`.
    pub resulting: Option<ResultingEntry>,
    /// The producer revision the transfer is evaluated against.
    pub producer: String,
    /// Rule 4's plan identity: the SHA-256 of the path, both classes, the
    /// file's digest and the manifest's digest.
    pub identity: String,
    /// What `transfer apply` must be given: the identity, carried in a token
    /// beside a short digest of each input, so a stale plan can be refused
    /// naming which input changed.
    pub plan_id: String,
    /// Entries carrying a `transfer` with no journal record: read as before,
    /// reported, and never rewritten (the compatibility paragraph).
    pub recorded_without_journal: Vec<String>,
    /// Where the journal disagrees with the entries. Non-empty means `transfer
    /// apply` and `transfer revert` refuse (rule 5).
    pub journal_disagreements: Vec<String>,
}

/// What `transfer apply` or `transfer revert` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "result")]
pub enum Outcome {
    /// The transfer was applied and journaled.
    Applied {
        /// The record appended.
        record: TransferRecord,
        /// What the manifest write did besides writing.
        write: Written,
    },
    /// The reversal was applied and journaled.
    Reverted {
        /// The record appended, naming the one it reverses.
        record: TransferRecord,
        /// What the manifest write did besides writing.
        write: Written,
    },
    /// The latest record already made this move and nothing changed since
    /// (rule 7). Nothing was written.
    AlreadySatisfied {
        /// The record that satisfies the request.
        record: TransferRecord,
    },
}

impl Outcome {
    /// The word the outcome is reported as.
    pub fn word(&self) -> &'static str {
        match self {
            Outcome::Applied { .. } => "applied",
            Outcome::Reverted { .. } => "reverted",
            Outcome::AlreadySatisfied { .. } => "already-satisfied",
        }
    }

    /// The record the outcome names.
    pub fn record(&self) -> &TransferRecord {
        match self {
            Outcome::Applied { record, .. }
            | Outcome::Reverted { record, .. }
            | Outcome::AlreadySatisfied { record } => record,
        }
    }

    /// Temporary manifest files the write removed.
    pub fn removed_leftovers(&self) -> &[String] {
        match self {
            Outcome::Applied { write, .. } | Outcome::Reverted { write, .. } => {
                &write.removed_leftovers
            }
            Outcome::AlreadySatisfied { .. } => &[],
        }
    }
}

fn eq_name(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Rule 3's syntax, and rule 2's closed list of instruction files. Reads
/// nothing.
///
/// Names are compared ignoring ASCII case, because on a case-insensitive
/// filesystem `claude.md` opens `CLAUDE.md` and `.Statecraft/` is a project
/// area: a comparison that honored case would let a spelling carry a protected
/// file past the rule. `.statecraft` is protected at any depth, like `.git`.
fn check_syntax(path: &str, to: Ownership) -> Result<Vec<&str>, TransferError> {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') {
        return refuse(
            RefusalKind::NotRelative,
            format!("rule 3: `{path}` is not a relative path written with forward slashes"),
        );
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == "..")
    {
        return refuse(
            RefusalKind::Escaping,
            format!("rule 3: `{path}` has an empty, `.` or `..` component"),
        );
    }
    if parts.iter().any(|p| eq_name(p, ".statecraft")) {
        return refuse(
            RefusalKind::Protected,
            format!(
                "rule 3: `{path}` is under a `.statecraft` component, which is never transferred"
            ),
        );
    }
    if parts.iter().any(|p| eq_name(p, ".git")) {
        return refuse(
            RefusalKind::Protected,
            format!("rule 3: `{path}` is under a `.git` component, which is never transferred"),
        );
    }
    if to != Ownership::User {
        let name = parts[parts.len() - 1];
        let copilot = parts.len() >= 2
            && eq_name(parts[parts.len() - 2], COPILOT_INSTRUCTIONS[0])
            && eq_name(name, COPILOT_INSTRUCTIONS[1]);
        if copilot || INSTRUCTION_FILES.iter().any(|f| eq_name(f, name)) {
            return refuse(
                RefusalKind::InstructionFile,
                format!(
                    "rule 2: `{path}` is a user instruction file, which section 3.8 keeps \
                     `user`; it cannot become `{}`",
                    to.word()
                ),
            );
        }
    }
    Ok(parts)
}

/// Rule 3 against the filesystem, for messages: every component exists
/// spelled exactly as the directory holding it spells it, none is a symbolic
/// link, and the last is a regular file.
///
/// The spelling check is what keeps one file from being recorded under two
/// names on a case-insensitive or normalization-insensitive volume, where
/// `Notes.md` opens `notes.md` and a composed `é` opens a decomposed one: the
/// directory's own listing is compared byte for byte with the name given.
fn check_on_disk(root: &Path, path: &str, parts: &[&str]) -> Result<(), TransferError> {
    let mut at = root.to_path_buf();
    for (i, part) in parts.iter().enumerate() {
        let parent = at.clone();
        at.push(part);
        let meta = match std::fs::symlink_metadata(&at) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return refuse(
                    RefusalKind::Missing,
                    format!("rule 3: `{path}` does not name a file that exists"),
                );
            }
            Err(e) => return Err(io_error(path, e)),
        };
        let listed = std::fs::read_dir(&parent).map_err(|e| io_error(path, e))?;
        let mut exact = false;
        let mut spelled: Option<String> = None;
        for entry in listed {
            let name = entry.map_err(|e| io_error(path, e))?.file_name();
            if name.as_encoded_bytes() == part.as_bytes() {
                exact = true;
                break;
            }
            let name = name.to_string_lossy().to_string();
            if spelled.is_none() && eq_name(&name, part) {
                spelled = Some(name);
            }
        }
        if !exact {
            let which = parts[..=i].join("/");
            return refuse(
                RefusalKind::Spelling,
                match spelled {
                    Some(on_disk) => format!(
                        "rule 3: `{which}` is spelled `{on_disk}` in its directory; name the \
                         file as the directory spells it"
                    ),
                    None => format!(
                        "rule 3: `{which}` opens a file its directory lists under another \
                         spelling (case or Unicode normalization); name the file as the \
                         directory spells it"
                    ),
                },
            );
        }
        if meta.file_type().is_symlink() {
            let which = parts[..=i].join("/");
            return refuse(
                RefusalKind::SymbolicLink,
                format!("rule 3: `{which}`, a component of `{path}`, is a symbolic link"),
            );
        }
        let last = i + 1 == parts.len();
        if last && meta.is_dir() {
            return refuse(
                RefusalKind::Directory,
                format!("rule 3: `{path}` is a directory; nothing is transferred wholesale"),
            );
        }
        if last && !meta.is_file() {
            return refuse(
                RefusalKind::NotARegularFile,
                format!("rule 3: `{path}` is not a regular file"),
            );
        }
    }
    Ok(())
}

fn check_path(root: &Path, path: &str, to: Ownership) -> Result<(), TransferError> {
    let parts = check_syntax(path, to)?;
    check_on_disk(root, path, &parts)
}

/// The file's identity and contents, read through one handle.
struct Opened {
    digest: String,
    bytes: u64,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
}

/// Open the file component by component without following a link, confirm
/// on the handle that it is a regular file, and digest it from that same
/// handle, so nothing swapped in after [`check_on_disk`] is what gets
/// recorded.
#[cfg(unix)]
fn open_regular(root: &Path, path: &str) -> Result<Opened, TransferError> {
    use rustix::fs::{FileType, Mode, OFlags, fstat, openat};
    use rustix::io::Errno;
    use std::io::Read as _;
    let parts: Vec<&str> = path.split('/').collect();
    let mut dir = rustix::fs::open(
        root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|e| io_error(path, e))?;
    for (i, part) in parts.iter().enumerate() {
        let last = i + 1 == parts.len();
        let flags = OFlags::RDONLY
            | OFlags::NOFOLLOW
            | OFlags::CLOEXEC
            | if last {
                OFlags::NONBLOCK
            } else {
                OFlags::DIRECTORY
            };
        let fd = match openat(&dir, *part, flags, Mode::empty()) {
            Ok(fd) => fd,
            Err(Errno::NOENT) => {
                return refuse(
                    RefusalKind::Missing,
                    format!("rule 3: `{path}` does not name a file that exists"),
                );
            }
            Err(Errno::LOOP) | Err(Errno::NOTDIR) => {
                return refuse(
                    RefusalKind::SymbolicLink,
                    format!(
                        "rule 3: a component of `{path}` is a symbolic link or not a directory \
                         when opened"
                    ),
                );
            }
            Err(e) => return Err(io_error(path, e)),
        };
        if !last {
            dir = fd;
            continue;
        }
        let stat = fstat(&fd).map_err(|e| io_error(path, e))?;
        match FileType::from_raw_mode(stat.st_mode) {
            FileType::RegularFile => {}
            FileType::Directory => {
                return refuse(
                    RefusalKind::Directory,
                    format!("rule 3: `{path}` is a directory; nothing is transferred wholesale"),
                );
            }
            _ => {
                return refuse(
                    RefusalKind::NotARegularFile,
                    format!("rule 3: `{path}` is not a regular file"),
                );
            }
        }
        let mut file = std::fs::File::from(fd);
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|e| io_error(path, e))?;
        return Ok(Opened {
            digest: digest_bytes(&bytes),
            bytes: bytes.len() as u64,
            dev: stat.st_dev as u64,
            ino: stat.st_ino as u64,
        });
    }
    refuse(
        RefusalKind::NotRelative,
        format!("rule 3: `{path}` names nothing"),
    )
}

#[cfg(not(unix))]
fn open_regular(root: &Path, path: &str) -> Result<Opened, TransferError> {
    let _ = (root, path);
    Err(io_error(
        path,
        "reading a file without following a link needs `openat`, which this platform lacks",
    ))
}

/// Rule 3's alias check: the path must not be the same file as another path
/// the manifest, the journal, a modification or an adapter already names
/// under a different spelling (or a hard link). Compared by device and inode,
/// which is what the filesystem itself treats as identity.
#[cfg(unix)]
fn check_alias(
    ctx: &Context<'_>,
    manifest: &Manifest,
    path: &str,
    opened: &Opened,
) -> Result<(), TransferError> {
    use std::os::unix::fs::MetadataExt as _;
    let mut named: Vec<&str> = Vec::new();
    named.extend(manifest.entries.iter().map(|e| e.path.as_str()));
    named.extend(manifest.transfers.iter().map(|r| r.path.as_str()));
    named.extend(manifest.modifications.iter().map(|m| m.path.as_str()));
    for d in ctx.declarations {
        named.extend(d.paths());
    }
    for other in named {
        if other == path {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(resolve(ctx.root, other)) else {
            continue;
        };
        if meta.dev() == opened.dev && meta.ino() == opened.ino {
            return refuse(
                RefusalKind::Alias,
                format!(
                    "rule 3: `{path}` is the same file this product already names as `{other}`; \
                     one file is never recorded under two names"
                ),
            );
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_alias(
    _ctx: &Context<'_>,
    _manifest: &Manifest,
    _path: &str,
    _opened: &Opened,
) -> Result<(), TransferError> {
    Ok(())
}

/// Take the manifest lock without waiting, and read the manifest under it.
fn locked_manifest(
    root: &Path,
) -> Result<(crate::manifest::ManifestLock, Manifest, String), TransferError> {
    // No manifest is refused before the lock, so the refusal creates not even
    // the runtime lock file; it is read again under the lock below.
    read_manifest(root)?;
    let held = match crate::manifest::lock(root, std::time::Duration::ZERO) {
        Ok(l) => l,
        Err(ManifestError::Busy { .. }) => {
            return refuse(
                RefusalKind::Busy,
                format!(
                    "another process holds the manifest lock on {}, so this transfer did not \
                     start",
                    root.display()
                ),
            );
        }
        Err(e) => return Err(e.into()),
    };
    let (manifest, digest) = read_manifest(root)?;
    Ok((held, manifest, digest))
}

/// The manifest, parsed from the bytes whose digest is returned beside it.
fn read_manifest(root: &Path) -> Result<(Manifest, String), TransferError> {
    match Manifest::read_tracked(root)? {
        Some(read) => Ok(read),
        None => refuse(
            RefusalKind::NoManifest,
            format!(
                "{} has no {MANIFEST_PATH}; a transfer is recorded in the manifest, and \
                 creating one is `env apply`'s act, not this one's",
                root.display()
            ),
        ),
    }
}

/// Rule 4's plan identity: the SHA-256 of the path, both classes, the file's
/// digest and the manifest's digest, encoded as a JSON array so no path can
/// be spelled to collide with another tuple.
pub fn plan_identity(
    path: &str,
    from: Ownership,
    to: Ownership,
    file_digest: &str,
    manifest_digest: &str,
) -> String {
    let content = serde_json::json!([path, from.word(), to.word(), file_digest, manifest_digest]);
    digest_bytes(content.to_string().as_bytes())
}

/// The token `transfer plan` prints and `transfer apply` is given:
/// `pi1-<from>-<to>-<path>-<file>-<manifest>-<identity>`, where the three
/// middle fields are the first 16 hex digits of the path's digest, the file's
/// digest and the manifest's digest, and the last is the full identity.
pub fn plan_token(
    path: &str,
    from: Ownership,
    to: Ownership,
    file_digest: &str,
    manifest_digest: &str,
) -> String {
    format!(
        "{PLAN_TOKEN_PREFIX}-{}-{}-{}-{}-{}-{}",
        from.word(),
        to.word(),
        short(&digest_bytes(path.as_bytes())),
        short(file_digest),
        short(manifest_digest),
        plan_identity(path, from, to, file_digest, manifest_digest)
    )
}

fn short(digest: &str) -> &str {
    &digest[..SHORT.min(digest.len())]
}

/// A token's fields.
struct Token<'a> {
    from: &'a str,
    to: &'a str,
    path: &'a str,
    file: &'a str,
    manifest: &'a str,
    identity: &'a str,
}

fn parse_token(token: &str) -> Option<Token<'_>> {
    let fields: Vec<&str> = token.split('-').collect();
    let [prefix, from, to, path, file, manifest, identity] = fields.as_slice() else {
        return None;
    };
    (*prefix == PLAN_TOKEN_PREFIX).then_some(Token {
        from,
        to,
        path,
        file,
        manifest,
        identity,
    })
}

/// Rule 4: refuse a stale plan naming exactly which input changed.
///
/// Two spellings are accepted: the `pi1` token `transfer plan` prints as
/// `plan_id`, whose fields name each input, and the bare identity it prints as
/// `identity`, which is what rule 4 calls the plan identity. A still-current
/// bare identity is never refused. A stale one is matched against what the
/// inputs could have been: every admitted pair of classes, the file's digest
/// now and every digest the manifest and the journal record for this path,
/// and the manifest's digest now and every digest the journal records it had.
/// The first combination that reproduces it names what changed; when none
/// does, the refusal says that what changed could not be determined.
fn stale(given: &str, now: &Plan, manifest: &Manifest) -> Result<(), TransferError> {
    if given == now.identity || parse_token(given).is_some_and(|t| t.identity == now.identity) {
        return Ok(());
    }
    let mut changed = Vec::new();
    let mut said = Vec::new();
    if let Some(t) = parse_token(given) {
        if t.from != now.from.word() || t.to != now.to.word() {
            changed.push("classes".to_string());
            said.push(format!(
                "the plan was for `{}` to `{}`, and this request is `{}` to `{}`",
                t.from,
                t.to,
                now.from.word(),
                now.to.word()
            ));
        }
        if t.path != short(&digest_bytes(now.path.as_bytes())) {
            changed.push("path".into());
            said.push(format!("the plan was for another path than `{}`", now.path));
        }
        if t.file != short(&now.digest) {
            changed.push("file".into());
            said.push(format!(
                "the file changed: planned at {}.., now {}",
                t.file, now.digest
            ));
        }
        if t.manifest != short(&now.manifest_digest) {
            changed.push("manifest".into());
            said.push(format!(
                "the manifest changed: planned at {}.., now {}",
                t.manifest, now.manifest_digest
            ));
        }
        if changed.is_empty() {
            changed.push("identity".into());
            said.push(
                "the token's identity does not match its own fields; it was not issued by \
                 `transfer plan`"
                    .into(),
            );
        }
    } else if !(given.len() == 64 && given.bytes().all(|b| b.is_ascii_hexdigit())) {
        changed.push("identity".into());
        said.push(format!(
            "`{given}` is neither a plan identity nor a plan token this build issues; plan again"
        ));
    } else if let Some((classes, file, manifest_moved)) = reproduce(given, now, manifest) {
        if classes {
            changed.push("classes".into());
            said.push("the plan was for another pair of classes".into());
        }
        if file {
            changed.push("file".into());
            said.push(format!("the file changed: now {}", now.digest));
        }
        if manifest_moved {
            changed.push("manifest".into());
            said.push(format!("the manifest changed: now {}", now.manifest_digest));
        }
    } else {
        changed.push("undetermined".into());
        said.push(format!(
            "`{given}` is not the current plan identity, and which input changed could not be \
             determined from it (the path, the classes, the file and the manifest are what it \
             binds)"
        ));
    }
    Err(TransferError::Refused(Refusal {
        kind: RefusalKind::StalePlan,
        detail: format!(
            "rule 4: the plan is stale: {}; the current plan identity is {} ({})",
            said.join("; "),
            now.identity,
            now.plan_id
        ),
        changed,
    }))
}

/// Find which inputs a bare identity was computed from, among the values they
/// could have had: `(classes changed, file changed, manifest changed)`.
fn reproduce(given: &str, now: &Plan, manifest: &Manifest) -> Option<(bool, bool, bool)> {
    use Ownership::*;
    let mut files = vec![now.digest.clone()];
    if let Some(e) = manifest.entry(&now.path) {
        files.push(e.digest.clone());
    }
    for r in manifest.transfers.iter().filter(|r| r.path == now.path) {
        files.push(r.digest.clone());
    }
    let mut manifests = vec![now.manifest_digest.clone()];
    manifests.extend(manifest.transfers.iter().map(|r| r.manifest_before.clone()));
    let pairs = [
        (now.from, now.to),
        (User, Adopted),
        (User, Managed),
        (Adopted, User),
        (Managed, User),
    ];
    for (from, to) in pairs {
        for file in &files {
            for m in &manifests {
                if plan_identity(&now.path, from, to, file, m) == given {
                    return Some((
                        (from, to) != (now.from, now.to),
                        *file != now.digest,
                        *m != now.manifest_digest,
                    ));
                }
            }
        }
    }
    None
}

/// The latest journal record for a path.
pub fn latest<'m>(manifest: &'m Manifest, path: &str) -> Option<&'m TransferRecord> {
    manifest.transfers.iter().rev().find(|r| r.path == path)
}

/// Every place the journal disagrees with the entries (rule 5).
///
/// A path's latest record says which class the transfer left it in, and the
/// manifest has to agree. Two differences are not disagreements, because this
/// product's own recorded operations produce them and neither is an unrecorded
/// change to ownership: a move to `managed` whose entry `env remove` then
/// removed (whether or not a file is at the path again since), and a move to
/// `user` whose file was deleted and which `env apply` then wrote afresh (a
/// `managed` entry from an adapter, carrying no `transfer`). Beyond the
/// classes, a journal whose identities repeat, or whose reversal names a
/// record not before it, is itself malformed.
pub fn disagreements(manifest: &Manifest) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen_ids: Vec<&str> = Vec::new();
    for r in &manifest.transfers {
        if seen_ids.contains(&r.id.as_str()) {
            out.push(format!("the journal records identity {} twice", r.id));
        }
        if let Some(reverted) = &r.reverts {
            if !seen_ids.contains(&reverted.as_str()) {
                out.push(format!(
                    "record {} reverses {reverted}, which the journal does not record before it",
                    r.id
                ));
            }
        }
        seen_ids.push(&r.id);
    }
    let mut paths: Vec<&str> = Vec::new();
    for r in manifest.transfers.iter().rev() {
        if paths.contains(&r.path.as_str()) {
            continue;
        }
        paths.push(&r.path);
        let entry = manifest.entry(&r.path);
        let now = Ownership::of(manifest, &r.path);
        if now == r.to {
            continue;
        }
        let removed_by_env_remove = r.to == Ownership::Managed && entry.is_none();
        let rewritten_by_env_apply = r.to == Ownership::User
            && entry.is_some_and(|e| {
                e.class == Class::Managed
                    && e.transfer.is_none()
                    && e.source.kind == SourceKind::Adapter
            });
        if removed_by_env_remove || rewritten_by_env_apply {
            continue;
        }
        out.push(format!(
            "the journal's latest record for `{}` ({}) left it `{}`, and the manifest records \
             `{}`",
            r.path,
            r.id,
            r.to.word(),
            now.word()
        ));
    }
    out
}

fn recorded_without_journal(manifest: &Manifest) -> Vec<String> {
    manifest
        .entries
        .iter()
        .filter(|e| e.transfer.is_some() && latest(manifest, &e.path).is_none())
        .map(|e| e.path.clone())
        .collect()
}

/// The digest rule 6 says the file should have after `record`: the digest the
/// manifest entry records now, where the transfer produced an entry, so a
/// rewrite this product made and recorded is not an intervening change; and
/// the digest the transfer observed, where it removed one.
fn expected_digest(manifest: &Manifest, record: &TransferRecord) -> Option<String> {
    match record.to {
        Ownership::User => Some(record.digest.clone()),
        _ => manifest.entry(&record.path).map(|e| e.digest.clone()),
    }
}

/// Rule 1's resulting entry.
fn resulting_entry(
    ctx: &Context<'_>,
    path: &str,
    to: Ownership,
    digest: &str,
    bytes: u64,
) -> Result<Option<ResultingEntry>, TransferError> {
    Ok(match to {
        Ownership::User => None,
        Ownership::Adopted => Some(ResultingEntry {
            class: Class::Adopted,
            source: Source {
                kind: SourceKind::Template,
                identity: TRANSFER_SOURCE.to_string(),
            },
            digest: digest.to_string(),
            bytes,
            transfer: None,
        }),
        Ownership::Managed => {
            let Some(adapter) = ctx.adapter_for(path) else {
                return refuse(
                    RefusalKind::NoSource,
                    format!(
                        "rule 1: `{path}` is not a path any installed adapter declares, so this \
                         product has no source to manage it from; adopt it instead"
                    ),
                );
            };
            if let Readiness::Refused { missing } = readiness(adapter, ctx.probe) {
                return refuse(
                    RefusalKind::AdapterNotClaiming,
                    format!(
                        "rule 1: adapter `{}` declares `{path}` but does not claim its paths \
                         here (missing {}), so this product would not itself write it",
                        adapter.name,
                        missing.join(", ")
                    ),
                );
            }
            let prior = ctx
                .foreign
                .claimant_of(path)
                .cloned()
                .unwrap_or(Claimant::Path {
                    path: path.to_string(),
                });
            Some(ResultingEntry {
                class: Class::Managed,
                source: Source {
                    kind: SourceKind::Adapter,
                    identity: adapter.name.clone(),
                },
                digest: digest.to_string(),
                bytes,
                transfer: Some(crate::manifest::Transfer {
                    from: prior,
                    digest_at_transfer: digest.to_string(),
                    evaluated_against: Some(ctx.producer.to_string()),
                }),
            })
        }
    })
}

/// Everything a plan says, computed from one reading of the manifest and one
/// handle on the file.
fn compute(
    ctx: &Context<'_>,
    manifest: &Manifest,
    manifest_digest: &str,
    path: &str,
    from: Ownership,
    to: Ownership,
) -> Result<Plan, TransferError> {
    if to != Ownership::User && manifest.modification(path).is_some() {
        return refuse(
            RefusalKind::Modification,
            format!(
                "rule 2: `{path}` carries this product's instruction bridge, which is a \
                 modification and not an entry; it is not transferable"
            ),
        );
    }
    let current = Ownership::of(manifest, path);
    if current != from {
        return refuse(
            RefusalKind::ClassMismatch,
            format!(
                "rule 7: `{path}` is `{}`, not `{}` as named",
                current.word(),
                from.word()
            ),
        );
    }
    let opened = open_regular(ctx.root, path)?;
    check_alias(ctx, manifest, path, &opened)?;
    let resulting = resulting_entry(ctx, path, to, &opened.digest, opened.bytes)?;
    let entry = manifest.entry(path);
    let standing = Standing {
        class: current,
        recorded_digest: entry.map(|e| e.digest.clone()),
        matches_record: entry.map(|e| e.digest == opened.digest),
        declared_by: ctx.adapter_for(path).map(|d| d.name.clone()),
    };
    Ok(Plan {
        path: path.to_string(),
        from,
        to,
        current: standing,
        identity: plan_identity(path, from, to, &opened.digest, manifest_digest),
        plan_id: plan_token(path, from, to, &opened.digest, manifest_digest),
        digest: opened.digest,
        bytes: opened.bytes,
        manifest_digest: manifest_digest.to_string(),
        resulting,
        producer: ctx.producer.to_string(),
        recorded_without_journal: recorded_without_journal(manifest),
        journal_disagreements: disagreements(manifest),
    })
}

fn admitted(from: Ownership, to: Ownership) -> Result<(), TransferError> {
    if Ownership::admitted(from, to) {
        return Ok(());
    }
    refuse(
        RefusalKind::MoveNotAdmitted,
        format!(
            "rule 1: `{}` to `{}` is not a move section 3.35 admits; the moves are `user` to \
             `adopted` or `managed` and back, and `adopted` to `managed` passes through `user`",
            from.word(),
            to.word()
        ),
    )
}

/// `transfer plan` (rule 4). Writes nothing.
pub fn plan(
    ctx: &Context<'_>,
    path: &str,
    from: Ownership,
    to: Ownership,
) -> Result<Plan, TransferError> {
    admitted(from, to)?;
    check_path(ctx.root, path, to)?;
    let (manifest, manifest_digest) = read_manifest(ctx.root)?;
    compute(ctx, &manifest, &manifest_digest, path, from, to)
}

fn required(act: &Act<'_>) -> Result<(), TransferError> {
    if act.operator.trim().is_empty() || act.reason.trim().is_empty() {
        return refuse(
            RefusalKind::MissingOperatorOrReason,
            "a transfer records an operator and a reason, and one of them is empty",
        );
    }
    Ok(())
}

fn refuse_disagreement(manifest: &Manifest) -> Result<(), TransferError> {
    let disagreeing = disagreements(manifest);
    if disagreeing.is_empty() {
        return Ok(());
    }
    refuse(
        RefusalKind::JournalDisagrees,
        format!("rule 5: {}", disagreeing.join("; ")),
    )
}

/// Append a record and commit the manifest, durably and atomically, under
/// the lock the caller holds.
fn commit(
    root: &Path,
    mut manifest: Manifest,
    mut record: TransferRecord,
    resulting: Option<ResultingEntry>,
    reversal: bool,
) -> Result<Outcome, TransferError> {
    match resulting {
        Some(r) => manifest.upsert(r.into_entry(&record.path, &record.at)),
        None => {
            manifest.remove(&record.path);
        }
    }
    record.id = record.identity();
    manifest.transfers.push(record.clone());
    let outcome = |write: Written| {
        if reversal {
            Outcome::Reverted { record, write }
        } else {
            Outcome::Applied { record, write }
        }
    };
    match manifest.write(root) {
        Ok(write) => Ok(outcome(write)),
        Err(ManifestError::NotDurable { detail, .. }) => Err(TransferError::NotDurable {
            outcome: Box::new(outcome(Written::default())),
            detail,
        }),
        Err(e) => Err(e.into()),
    }
}

/// `transfer apply` (rules 4, 5 and 7).
///
/// The order is the contract's: a journal that disagrees refuses first (rule
/// 5), a request the latest record already satisfies is reported before the
/// plan identity is looked at (rule 7), a named class that is not the path's
/// refuses, and only then is the identity compared, naming which input moved.
pub fn apply(
    ctx: &Context<'_>,
    path: &str,
    from: Ownership,
    to: Ownership,
    given_plan: &str,
    act: &Act<'_>,
    clock: &dyn Clock,
) -> Result<Outcome, TransferError> {
    required(act)?;
    admitted(from, to)?;
    check_path(ctx.root, path, to)?;
    let (_held, manifest, manifest_digest) = locked_manifest(ctx.root)?;
    refuse_disagreement(&manifest)?;

    if let Some(last) = latest(&manifest, path) {
        if last.from == from && last.to == to && Ownership::of(&manifest, path) == to {
            let now = open_regular(ctx.root, path)?;
            if expected_digest(&manifest, last).as_deref() == Some(now.digest.as_str()) {
                return Ok(Outcome::AlreadySatisfied {
                    record: last.clone(),
                });
            }
        }
    }

    let plan = compute(ctx, &manifest, &manifest_digest, path, from, to)?;
    stale(given_plan, &plan, &manifest)?;

    let record = TransferRecord {
        id: String::new(),
        path: path.to_string(),
        from,
        to,
        digest: plan.digest,
        bytes: plan.bytes,
        producer: ctx.producer.to_string(),
        operator: act.operator.to_string(),
        operator_provenance: OPERATOR_PROVENANCE.to_string(),
        reason: act.reason.to_string(),
        at: rfc3339_utc(clock.now_unix()),
        manifest_before: manifest_digest,
        reverts: None,
    };
    commit(ctx.root, manifest, record, plan.resulting, false)
}

/// `transfer revert` (rule 6): the inverse move of a recorded transfer, when
/// it is the latest for its path, the class is still the one it produced, and
/// the file's digest is the one this product's record says it should be.
/// Reversal changes ownership, never content.
pub fn revert(
    ctx: &Context<'_>,
    transfer_id: &str,
    act: &Act<'_>,
    clock: &dyn Clock,
) -> Result<Outcome, TransferError> {
    required(act)?;
    let (_held, manifest, manifest_digest) = locked_manifest(ctx.root)?;
    refuse_disagreement(&manifest)?;

    let Some(target) = manifest
        .transfers
        .iter()
        .find(|r| r.id == transfer_id)
        .cloned()
    else {
        return refuse(
            RefusalKind::UnknownTransfer,
            format!("rule 6: no transfer `{transfer_id}` is recorded in the journal"),
        );
    };
    let path = target.path.clone();
    if let Some(later) = latest(&manifest, &path).filter(|r| r.id != target.id) {
        return refuse(
            RefusalKind::LaterTransfer,
            format!(
                "rule 6: a later transfer, {}, names `{path}`; only the latest can be reversed",
                later.id
            ),
        );
    }
    let now_class = Ownership::of(&manifest, &path);
    if now_class != target.to {
        return refuse(
            RefusalKind::ClassChanged,
            format!(
                "rule 6: `{path}` is `{}`, no longer the `{}` transfer {} left it",
                now_class.word(),
                target.to.word(),
                target.id
            ),
        );
    }
    let inverse = target.from;
    // Rule 3's path checks, then the digest, then what the inverse move itself
    // refuses (rule 2's instruction files, rule 1's source).
    let parts = check_syntax(&path, Ownership::User)?;
    match check_on_disk(ctx.root, &path, &parts) {
        Ok(()) => {}
        Err(TransferError::Refused(r)) => {
            return refuse(
                r.kind,
                format!("rule 6: {}; the reversal needs the file", r.detail),
            );
        }
        Err(e) => return Err(e),
    }
    // The digest first: an unrecorded edit is the answer rule 6 names, and it
    // is reported ahead of anything the inverse move itself would refuse.
    let now = open_regular(ctx.root, &path)?;
    let expected = expected_digest(&manifest, &target).unwrap_or_default();
    if now.digest != expected {
        return refuse(
            RefusalKind::UnrecordedChange,
            format!(
                "rule 6: `{path}` was edited without this product recording it: this product's \
                 record says {expected}, and the file is {}",
                now.digest
            ),
        );
    }
    check_syntax(&path, inverse)?;
    let plan = compute(ctx, &manifest, &manifest_digest, &path, target.to, inverse)?;
    let record = TransferRecord {
        id: String::new(),
        path,
        from: target.to,
        to: inverse,
        digest: plan.digest,
        bytes: plan.bytes,
        producer: ctx.producer.to_string(),
        operator: act.operator.to_string(),
        operator_provenance: OPERATOR_PROVENANCE.to_string(),
        reason: act.reason.to_string(),
        at: rfc3339_utc(clock.now_unix()),
        manifest_before: manifest_digest,
        reverts: Some(target.id),
    };
    commit(ctx.root, manifest, record, plan.resulting, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exactly_four_moves_are_admitted() {
        use Ownership::*;
        let all = [User, Adopted, Managed];
        let admitted: Vec<_> = all
            .iter()
            .flat_map(|f| all.iter().map(move |t| (*f, *t)))
            .filter(|(f, t)| Ownership::admitted(*f, *t))
            .collect();
        assert_eq!(
            admitted,
            [
                (User, Adopted),
                (User, Managed),
                (Adopted, User),
                (Managed, User)
            ]
        );
    }

    #[test]
    fn class_words_round_trip_and_nothing_else_parses() {
        for o in [Ownership::User, Ownership::Adopted, Ownership::Managed] {
            assert_eq!(Ownership::from_word(o.word()), Some(o));
        }
        assert_eq!(Ownership::from_word("foreign"), None);
        assert_eq!(Ownership::from_word("User"), None);
    }

    #[test]
    fn the_plan_identity_binds_every_input_and_no_path_can_collide_by_spelling() {
        use Ownership::*;
        let base = plan_identity("a", User, Adopted, "d", "m");
        assert_eq!(base, plan_identity("a", User, Adopted, "d", "m"));
        assert_ne!(base, plan_identity("b", User, Adopted, "d", "m"));
        assert_ne!(base, plan_identity("a", User, Managed, "d", "m"));
        assert_ne!(base, plan_identity("a", User, Adopted, "e", "m"));
        assert_ne!(base, plan_identity("a", User, Adopted, "d", "n"));
        // A path spelled like a separator and the next field is still one path.
        assert_ne!(
            plan_identity("a|user", Adopted, User, "d", "m"),
            plan_identity("a", User, Adopted, "d", "m")
        );
    }

    #[test]
    fn a_plan_token_carries_each_input_and_the_full_identity() {
        use Ownership::*;
        let d = digest_bytes(b"file");
        let m = digest_bytes(b"manifest");
        let token = plan_token("a.md", User, Adopted, &d, &m);
        let t = parse_token(&token).unwrap();
        assert_eq!((t.from, t.to), ("user", "adopted"));
        assert_eq!(t.file, &d[..16]);
        assert_eq!(t.manifest, &m[..16]);
        assert_eq!(t.identity, plan_identity("a.md", User, Adopted, &d, &m));
        assert!(parse_token("not-a-token").is_none());
    }

    #[test]
    fn a_record_identity_covers_its_content() {
        let mut r = TransferRecord {
            id: String::new(),
            path: "p".into(),
            from: Ownership::User,
            to: Ownership::Adopted,
            digest: "d".into(),
            bytes: 1,
            producer: "spec-spine-core@0.23.0".into(),
            operator: "o".into(),
            operator_provenance: OPERATOR_PROVENANCE.into(),
            reason: "r".into(),
            at: "t".into(),
            manifest_before: "m".into(),
            reverts: None,
        };
        let first = r.identity();
        r.reason = "another".into();
        assert_ne!(first, r.identity());
    }

    #[test]
    fn syntax_is_refused_before_anything_is_read() {
        for (path, kind) in [
            ("", RefusalKind::NotRelative),
            ("a/../b", RefusalKind::Escaping),
            (".statecraft/x", RefusalKind::Protected),
            ("deep/.Statecraft/x", RefusalKind::Protected),
            ("x/.GIT/y", RefusalKind::Protected),
        ] {
            match check_syntax(path, Ownership::Adopted) {
                Err(TransferError::Refused(r)) => assert_eq!(r.kind, kind, "{path}"),
                other => panic!("{path}: {other:?}"),
            }
        }
        // Releasing an instruction file to `user` is not refused by rule 2.
        assert!(check_syntax("CLAUDE.md", Ownership::User).is_ok());
        assert!(check_syntax("CLAUDE.md", Ownership::Adopted).is_err());
    }
}
