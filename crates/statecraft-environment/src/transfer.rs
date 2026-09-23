//! Per-path ownership transfer, as an operator's act.
//!
//! Spec 002 section 3.35. Section 3.21 retains, from the withdrawn section 3.7,
//! that ownership transfer is per path, explicit, operator-initiated, reversible
//! and recorded with the digest observed at the moment of transfer and the
//! producer revision it was evaluated against. This module is that operation:
//! a plan that writes nothing, an apply bound to the plan's identity, and a
//! reversal bound to the record it reverses.
//!
//! Nothing here reads a file's bytes for anything but its digest, nothing
//! writes a byte of any file but the manifest, and nothing moves a path
//! because of what its bytes resemble (rule 2).

use crate::claimant::resolve;
use crate::digest::{digest_bytes, digest_file};
use crate::manifest::{
    Class, Entry, MANIFEST_PATH, Manifest, ManifestError, Source, SourceKind, Transfer,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The source identity an entry adopted by transfer records.
pub const TRANSFER_SOURCE: &str = "operator-transfer";

/// The word every record carries beside the operator's name.
pub const OPERATOR_PROVENANCE: &str = "operator-supplied";

/// The user instruction files section 3.8 keeps `user`, a closed list by file
/// name at any depth, plus one path suffix (rule 2).
pub const INSTRUCTION_FILES: [&str; 5] = [
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".cursorrules",
    ".windsurfrules",
];

/// The one instruction file named by its directory as well as its name.
pub const COPILOT_INSTRUCTIONS: &str = ".github/copilot-instructions.md";

/// The three ownership classes of section 3.2.
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
    /// Read a class word.
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

    fn of(manifest: &Manifest, path: &str) -> Self {
        match manifest.entry(path).map(|e| e.class) {
            None => Self::User,
            Some(Class::Adopted) => Self::Adopted,
            Some(Class::Managed) => Self::Managed,
        }
    }

    fn admitted(from: Self, to: Self) -> bool {
        matches!(
            (from, to),
            (Self::User, Self::Adopted)
                | (Self::User, Self::Managed)
                | (Self::Adopted, Self::User)
                | (Self::Managed, Self::User)
        )
    }
}

/// One applied transfer, as the manifest's journal records it (rule 5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferRecord {
    /// Its identity: the SHA-256 of its own content fields.
    pub id: String,
    /// The path.
    pub path: String,
    /// The class it had.
    pub from: Ownership,
    /// The class it took.
    pub to: Ownership,
    /// The file's digest when the transfer was applied.
    pub digest: String,
    /// Its length in bytes.
    pub bytes: u64,
    /// The producer revision the transfer was evaluated against.
    pub producer: String,
    /// Who, as supplied.
    pub operator: String,
    /// Always [`OPERATOR_PROVENANCE`].
    pub operator_provenance: String,
    /// Why.
    pub reason: String,
    /// When, RFC 3339 UTC.
    pub at: String,
    /// The manifest's digest before this transfer.
    pub manifest_before: String,
    /// The record this one reverses, for a reversal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverts: Option<String>,
}

/// What `transfer plan` reports. Writing nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// The path.
    pub path: String,
    /// The class named as current.
    pub from: Ownership,
    /// The class it is to take.
    pub to: Ownership,
    /// The file's digest now.
    pub digest: String,
    /// Its length now.
    pub bytes: u64,
    /// The producer revision this is evaluated against.
    pub producer: String,
    /// The entry the manifest will hold afterwards, or `None` for `user`.
    pub resulting: Option<Entry>,
    /// The plan identity `transfer apply` must be given.
    pub plan_id: String,
    /// Entries carrying a `transfer` with no journal record, reported only.
    pub unjournaled: Vec<String>,
}

/// What `transfer apply` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "result")]
pub enum Applied {
    /// The transfer was applied and recorded.
    Applied {
        /// The record.
        record: TransferRecord,
    },
    /// The latest record already made this move and nothing changed since
    /// (rule 7). Nothing was written.
    AlreadySatisfied {
        /// The record that satisfied it.
        record: TransferRecord,
    },
}

/// Why a transfer was not performed.
#[derive(Debug, thiserror::Error)]
pub enum TransferError {
    /// A precondition failed; nothing was written.
    #[error("{0}")]
    Refused(String),
    /// The manifest could not be read or written.
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    /// A file could not be read.
    #[error("{path} could not be read: {detail}")]
    Io {
        /// Which.
        path: String,
        /// Why.
        detail: String,
    },
}

fn refused<T>(why: String) -> Result<T, TransferError> {
    Err(TransferError::Refused(format!("{why}; nothing was written")))
}

/// What the caller established: the repository, the producer revision this
/// build links, and the paths each installed adapter declares.
#[derive(Debug, Clone)]
pub struct Context<'a> {
    /// The repository root.
    pub root: &'a Path,
    /// The producer revision (the `spec-spine-core` version this build links).
    pub producer: &'a str,
    /// Every path an installed adapter declares, with that adapter's name.
    pub adapter_paths: Vec<(String, String)>,
}

/// Rule 3, and rule 2's instruction files: whether a path may be named at all.
fn check_path(root: &Path, path: &str, to: Ownership) -> Result<(), TransferError> {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') {
        return refused(format!("`{path}` is not a relative path with forward slashes"));
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.iter().any(|p| p.is_empty() || *p == "." || *p == "..") {
        return refused(format!(
            "`{path}` has an empty, `.` or `..` component, which could name something \
             outside the repository"
        ));
    }
    if parts[0] == ".statecraft" {
        return refused(format!(
            "`{path}` is under `.statecraft/`, which this product keeps for itself"
        ));
    }
    if parts.contains(&".git") {
        return refused(format!("`{path}` is under a `.git` component"));
    }
    let name = parts.last().copied().unwrap_or_default();
    if to != Ownership::User
        && (INSTRUCTION_FILES.contains(&name) || path.ends_with(COPILOT_INSTRUCTIONS))
    {
        return refused(format!(
            "`{path}` is a user instruction file, which section 3.8 keeps `user`; it is never \
             adopted or managed"
        ));
    }
    let mut at = root.to_path_buf();
    for (i, part) in parts.iter().enumerate() {
        at.push(part);
        let meta = std::fs::symlink_metadata(&at).map_err(|e| {
            TransferError::Refused(format!(
                "`{path}` does not name a file that exists ({e}); nothing was written"
            ))
        })?;
        if meta.file_type().is_symlink() {
            return refused(format!("a component of `{path}` is a symbolic link"));
        }
        if i + 1 == parts.len() && !meta.is_file() {
            return refused(format!(
                "`{path}` is not a regular file; a directory is never adopted wholesale"
            ));
        }
    }
    Ok(())
}

fn manifest_digest(root: &Path) -> Result<String, TransferError> {
    let path = resolve(root, MANIFEST_PATH);
    match std::fs::read(&path) {
        Ok(b) => Ok(digest_bytes(&b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok("absent".to_string()),
        Err(e) => Err(TransferError::Io {
            path: MANIFEST_PATH.to_string(),
            detail: e.to_string(),
        }),
    }
}

fn file_digest(root: &Path, path: &str) -> Result<(String, u64), TransferError> {
    match digest_file(&resolve(root, path)) {
        Ok(Some(d)) => Ok(d),
        Ok(None) => refused(format!("`{path}` does not exist")),
        Err(e) => Err(TransferError::Io {
            path: path.to_string(),
            detail: e.to_string(),
        }),
    }
}

fn plan_id(path: &str, from: Ownership, to: Ownership, digest: &str, manifest: &str) -> String {
    digest_bytes(format!("{path}|{}|{}|{digest}|{manifest}", from.word(), to.word()).as_bytes())
}

/// The latest journal record for a path.
fn latest<'m>(manifest: &'m Manifest, path: &str) -> Option<&'m TransferRecord> {
    manifest.transfers.iter().rev().find(|r| r.path == path)
}

/// Rule 5: a journal whose latest record for a path disagrees with the path's
/// class is refused by apply and revert.
fn disagreement(manifest: &Manifest) -> Option<String> {
    let mut seen: Vec<&str> = Vec::new();
    for r in manifest.transfers.iter().rev() {
        if seen.contains(&r.path.as_str()) {
            continue;
        }
        seen.push(&r.path);
        let now = Ownership::of(manifest, &r.path);
        if now != r.to {
            return Some(format!(
                "the transfer journal's latest record for `{}` says `{}`, and the manifest \
                 records `{}`",
                r.path,
                r.to.word(),
                now.word()
            ));
        }
    }
    None
}

fn unjournaled(manifest: &Manifest) -> Vec<String> {
    manifest
        .entries
        .iter()
        .filter(|e| e.transfer.is_some() && latest(manifest, &e.path).is_none())
        .map(|e| e.path.clone())
        .collect()
}

/// The digest rule 6 says the file should have after `record`: the digest the
/// manifest entry records now where the transfer produced an entry, and the
/// digest the transfer observed where it removed one.
fn expected_digest(manifest: &Manifest, record: &TransferRecord) -> Option<String> {
    match record.to {
        Ownership::User => Some(record.digest.clone()),
        _ => manifest.entry(&record.path).map(|e| e.digest.clone()),
    }
}

fn resulting_entry(
    ctx: &Context<'_>,
    path: &str,
    from: Ownership,
    to: Ownership,
    digest: &str,
    bytes: u64,
    at: &str,
) -> Result<Option<Entry>, TransferError> {
    Ok(match to {
        Ownership::User => None,
        Ownership::Adopted => Some(Entry {
            path: path.to_string(),
            class: Class::Adopted,
            source: Source {
                kind: SourceKind::Template,
                identity: TRANSFER_SOURCE.to_string(),
            },
            digest: digest.to_string(),
            bytes,
            written_at: at.to_string(),
            transfer: None,
        }),
        Ownership::Managed => {
            let Some((_, adapter)) = ctx.adapter_paths.iter().find(|(p, _)| p == path) else {
                return refused(format!(
                    "`{path}` is not a path any installed adapter declares, so this product has \
                     no source to manage it from; adopt it instead"
                ));
            };
            Some(Entry {
                path: path.to_string(),
                class: Class::Managed,
                source: Source {
                    kind: SourceKind::Adapter,
                    identity: adapter.clone(),
                },
                digest: digest.to_string(),
                bytes,
                written_at: at.to_string(),
                transfer: Some(Transfer {
                    from: crate::claimant::Claimant::Path {
                        path: path.to_string(),
                    },
                    digest_at_transfer: digest.to_string(),
                    evaluated_against: Some(ctx.producer.to_string()),
                }),
            })
        }
    })
    .map(|e| {
        let _ = from;
        e
    })
}

fn read_manifest(root: &Path) -> Result<Manifest, TransferError> {
    match Manifest::read(root)? {
        Some(m) => Ok(m),
        None => refused(format!(
            "{} has no {MANIFEST_PATH}; there is nothing to record a transfer in",
            root.display()
        )),
    }
}

/// `transfer plan` (rule 4). Writes nothing.
pub fn plan(
    ctx: &Context<'_>,
    path: &str,
    from: Ownership,
    to: Ownership,
) -> Result<Plan, TransferError> {
    if !Ownership::admitted(from, to) {
        return refused(format!(
            "`{}` to `{}` is not a move section 3.35 admits; `adopted` and `managed` pass \
             through `user`",
            from.word(),
            to.word()
        ));
    }
    check_path(ctx.root, path, to)?;
    let manifest = read_manifest(ctx.root)?;
    let current = Ownership::of(&manifest, path);
    if current != from {
        return refused(format!(
            "`{path}` is `{}`, not `{}`",
            current.word(),
            from.word()
        ));
    }
    let (digest, bytes) = file_digest(ctx.root, path)?;
    let manifest_now = manifest_digest(ctx.root)?;
    let resulting = resulting_entry(ctx, path, from, to, &digest, bytes, "(at apply)")?;
    Ok(Plan {
        path: path.to_string(),
        from,
        to,
        plan_id: plan_id(path, from, to, &digest, &manifest_now),
        digest,
        bytes,
        producer: ctx.producer.to_string(),
        resulting,
        unjournaled: unjournaled(&manifest),
    })
}

/// Who asked, why and when.
#[derive(Debug, Clone)]
pub struct Act<'a> {
    /// Who, as supplied.
    pub operator: &'a str,
    /// Why.
    pub reason: &'a str,
    /// Now, RFC 3339 UTC.
    pub at: &'a str,
}

fn required(act: &Act<'_>) -> Result<(), TransferError> {
    if act.operator.trim().is_empty() || act.reason.trim().is_empty() {
        return refused("a transfer needs a non-empty operator and reason".into());
    }
    Ok(())
}

fn record(
    ctx: &Context<'_>,
    manifest: &mut Manifest,
    path: &str,
    from: Ownership,
    to: Ownership,
    act: &Act<'_>,
    reverts: Option<String>,
) -> Result<TransferRecord, TransferError> {
    let (digest, bytes) = file_digest(ctx.root, path)?;
    let manifest_before = manifest_digest(ctx.root)?;
    match resulting_entry(ctx, path, from, to, &digest, bytes, act.at)? {
        Some(entry) => manifest.upsert(entry),
        None => {
            manifest.remove(path);
        }
    }
    let mut r = TransferRecord {
        id: String::new(),
        path: path.to_string(),
        from,
        to,
        digest,
        bytes,
        producer: ctx.producer.to_string(),
        operator: act.operator.trim().to_string(),
        operator_provenance: OPERATOR_PROVENANCE.to_string(),
        reason: act.reason.trim().to_string(),
        at: act.at.to_string(),
        manifest_before,
        reverts,
    };
    r.id = digest_bytes(serde_json::to_string(&r).unwrap_or_default().as_bytes());
    manifest.transfers.push(r.clone());
    manifest.write(ctx.root)?;
    Ok(r)
}

/// `transfer apply` (rules 4, 5 and 7).
pub fn apply(
    ctx: &Context<'_>,
    path: &str,
    from: Ownership,
    to: Ownership,
    given_plan: &str,
    act: &Act<'_>,
) -> Result<Applied, TransferError> {
    required(act)?;
    check_path(ctx.root, path, to)?;
    let mut manifest = read_manifest(ctx.root)?;
    // Rule 7, before the plan identity.
    if let Some(last) = latest(&manifest, path) {
        if last.from == from
            && last.to == to
            && Ownership::of(&manifest, path) == to
            && expected_digest(&manifest, last).as_deref()
                == Some(file_digest(ctx.root, path)?.0.as_str())
        {
            return Ok(Applied::AlreadySatisfied {
                record: last.clone(),
            });
        }
    }
    if let Some(why) = disagreement(&manifest) {
        return refused(why);
    }
    let current = plan(ctx, path, from, to)?;
    if current.plan_id != given_plan {
        return refused(format!(
            "the plan is stale: `{path}`'s digest, the manifest or its class changed since the \
             plan was made; plan again (now {})",
            current.plan_id
        ));
    }
    let r = record(ctx, &mut manifest, path, from, to, act, None)?;
    Ok(Applied::Applied { record: r })
}

/// `transfer revert` (rule 6).
pub fn revert(
    ctx: &Context<'_>,
    transfer_id: &str,
    act: &Act<'_>,
) -> Result<TransferRecord, TransferError> {
    required(act)?;
    let mut manifest = read_manifest(ctx.root)?;
    let Some(target) = manifest
        .transfers
        .iter()
        .find(|r| r.id == transfer_id)
        .cloned()
    else {
        return refused(format!("no transfer `{transfer_id}` is recorded"));
    };
    if latest(&manifest, &target.path).map(|r| &r.id) != Some(&target.id) {
        return refused(format!(
            "a later transfer names `{}`; only the latest can be reversed",
            target.path
        ));
    }
    if let Some(why) = disagreement(&manifest) {
        return refused(why);
    }
    if Ownership::of(&manifest, &target.path) != target.to {
        return refused(format!(
            "`{}` is no longer `{}`",
            target.path,
            target.to.word()
        ));
    }
    check_path(ctx.root, &target.path, target.from)?;
    let now = file_digest(ctx.root, &target.path)?.0;
    if expected_digest(&manifest, &target).as_deref() != Some(now.as_str()) {
        return refused(format!(
            "`{}` changed since this product last recorded it; that change is not one a \
             reversal may overwrite in the record",
            target.path
        ));
    }
    record(
        ctx,
        &mut manifest,
        &target.path,
        target.to,
        target.from,
        act,
        Some(target.id.clone()),
    )
}
