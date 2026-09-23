//! Replacing a drifted managed file, one named path at a time.
//!
//! Spec 002 section 3.4: "An upgrade never resolves a conflict by choosing.
//! Replacing a drifted managed file requires the operator to say so per path."
//! This module is how the operator says so, and it is the only way a drifted
//! managed file is ever rewritten.
//!
//! The shape is the plan-then-consent shape the rest of this product uses:
//!
//! 1. The operator names a path to `env plan`. The plan reports what the file
//!    holds now, what the manifest says this product last wrote, what the
//!    replacement would be, and a **plan identity** binding all of it.
//! 2. The operator hands the same path and that identity to `env apply` or
//!    `env upgrade`. The plan is recomputed; an identity that no longer matches
//!    is a stale plan, refused, and nothing at all is written.
//!
//! What can be named is narrow on purpose. Only a path a claiming adapter
//! declares, that the manifest records as `managed`, and whose bytes differ from
//! the manifest's digest. An adopted path, a foreign or occupied path, a user
//! path, a path no adapter declares, a symbolic link and a path that is not
//! drifted are each refused by name, whatever identity is supplied.
//!
//! The write is staged under `.statecraft/state/`, checked against the digest
//! the plan observed, and renamed over the target. A rename is atomic on one
//! filesystem, so an interrupted replacement leaves the old bytes or the new
//! ones and never a mixture. The manifest is written after the file, so the
//! one torn state a crash can leave is "the file holds the replacement and the
//! manifest has not caught up", and repeating the request recognises it as
//! already satisfied and records it.

use crate::adapter::Declaration;
use crate::claimant::resolve;
use crate::digest::{digest_bytes, digest_file};
use crate::manifest::Manifest;
use crate::plan::{Plan, Withholding};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Where staged replacements are written before they are renamed into place.
///
/// Runtime state, under the one ignored path of spec 002 section 3.12, so a
/// staged file an interruption leaves behind is never a new file in the
/// operator's tree.
pub const STAGING_DIR: &str = ".statecraft/state/replace";

/// The domain separator the plan identity is computed under. Changing it
/// invalidates every outstanding plan, which is what a change to what the
/// identity binds should do.
pub const PLAN_ID_DOMAIN: &str = "statecraft/env-replace/v1";

/// The operator's consent to replace one path, as given to `env apply`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consent {
    /// The repository-relative path, exactly as `env plan` was given it.
    pub path: String,
    /// The plan identity `env plan` reported for it.
    pub plan_id: String,
}

/// A replacement that the plan admits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Replacement {
    /// Repository-relative path.
    pub path: String,
    /// The adapter whose declared content replaces it.
    pub adapter: String,
    /// The digest the manifest records: what this product last wrote.
    pub recorded: String,
    /// The digest of the file on disk now: the drift the plan saw.
    pub found: String,
    /// The length of the file on disk now.
    pub found_bytes: u64,
    /// The digest of the bytes that would replace it.
    pub replacement: String,
    /// Their length.
    pub replacement_bytes: u64,
    /// The plan identity: SHA-256 over the domain, the path, the adapter and
    /// the three digests above.
    pub plan_id: String,
    /// The bytes that would be written. Carried so the apply writes exactly
    /// what the plan described.
    #[serde(skip)]
    pub contents: Vec<u8>,
}

/// What a named path turned out to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum Named {
    /// Drifted, and replaceable with the operator's consent.
    Replace(Replacement),
    /// The file already holds the bytes a replacement would write.
    ///
    /// Either a replacement already succeeded, or an earlier one was
    /// interrupted after the file was renamed into place and before the
    /// manifest recorded it. In the second case `records` is true and applying
    /// records the digest; no byte of the file is written.
    AlreadySatisfied {
        /// Repository-relative path.
        path: String,
        /// The adapter that declares it.
        adapter: String,
        /// The digest on disk, equal to the replacement's.
        digest: String,
        /// Whether the manifest still has to record it.
        records: bool,
    },
    /// Not something this operation may replace.
    Refused {
        /// Repository-relative path, as named.
        path: String,
        /// Why, in one line.
        reason: String,
    },
}

impl Named {
    /// The path this is about.
    pub fn path(&self) -> &str {
        match self {
            Named::Replace(r) => &r.path,
            Named::AlreadySatisfied { path, .. } | Named::Refused { path, .. } => path,
        }
    }

    /// A one-line rendering for a report.
    pub fn describe(&self) -> String {
        match self {
            Named::Replace(r) => format!(
                "replace {} ({}): found {}, recorded {}, replacement {}; plan {}",
                r.path, r.adapter, r.found, r.recorded, r.replacement, r.plan_id
            ),
            Named::AlreadySatisfied {
                path,
                records: false,
                ..
            } => format!("already-satisfied {path}: it holds the replacement"),
            Named::AlreadySatisfied {
                path,
                records: true,
                ..
            } => format!(
                "already-satisfied {path}: it holds the replacement, and the manifest will record it"
            ),
            Named::Refused { path, reason } => format!("refused {path}: {reason}"),
        }
    }
}

/// The plan identity for one replacement.
pub fn plan_id(
    path: &str,
    adapter: &str,
    recorded: &str,
    found: &str,
    replacement: &str,
) -> String {
    digest_bytes(
        format!("{PLAN_ID_DOMAIN}\n{path}\n{adapter}\n{recorded}\n{found}\n{replacement}\n")
            .as_bytes(),
    )
}

/// Why a named path is not a repository-relative file path this product can
/// name, if it is not.
pub(crate) fn malformed(path: &str) -> Option<&'static str> {
    if path.is_empty() {
        return Some("an empty path names nothing");
    }
    if path.starts_with('/') || path.contains('\\') {
        return Some("not a repository-relative path with forward slashes");
    }
    if path
        .split('/')
        .any(|c| c.is_empty() || c == "." || c == "..")
    {
        return Some("a path with an empty, `.` or `..` component is refused");
    }
    None
}

/// Whether the path, or any directory on the way to it, is a symbolic link.
///
/// A rename through a linked directory lands somewhere the manifest does not
/// name, and a linked file is somebody's decision about where the bytes live.
pub(crate) fn link_on_path(root: &Path, repo_relative: &str) -> std::io::Result<bool> {
    let mut at = root.to_path_buf();
    for segment in repo_relative.split('/').filter(|s| !s.is_empty()) {
        at.push(segment);
        match std::fs::symlink_metadata(&at) {
            Ok(m) if m.file_type().is_symlink() => return Ok(true),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e),
        }
    }
    Ok(false)
}

/// Assess every named path against a computed plan. Reads the target; writes
/// nothing.
///
/// `computed` is the plan [`crate::plan::plan`] returned for the same root,
/// manifest and declarations, so a path's standing here is the standing an
/// ordinary apply would give it.
pub fn assess(
    root: &Path,
    manifest: Option<&Manifest>,
    computed: &Plan,
    declarations: &[Declaration],
    named: &[String],
) -> std::io::Result<Vec<Named>> {
    let mut out = Vec::new();
    for (i, path) in named.iter().enumerate() {
        let refused = |reason: String| Named::Refused {
            path: path.clone(),
            reason,
        };
        if named[..i].contains(path) {
            out.push(refused("named more than once".to_string()));
            continue;
        }
        if let Some(reason) = malformed(path) {
            out.push(refused(reason.to_string()));
            continue;
        }
        let Some((declaration, file)) = declarations
            .iter()
            .find_map(|d| d.files.iter().find(|f| &f.path == path).map(|f| (d, f)))
        else {
            out.push(refused(
                "no configured adapter declares this path, so this product has no replacement for it"
                    .to_string(),
            ));
            continue;
        };
        let replacement = digest_bytes(&file.contents);
        let recorded = manifest.and_then(|m| m.entry(path));

        if let Some(w) = computed.withheld.iter().find(|w| &w.path == path) {
            match &w.reason {
                Withholding::Drifted { expected, found } => {
                    let target = resolve(root, path);
                    if link_on_path(root, path)? {
                        out.push(refused(
                            "the path or one of its directories is a symbolic link; this product does not replace through a link"
                                .to_string(),
                        ));
                        continue;
                    }
                    if *found == replacement {
                        out.push(Named::AlreadySatisfied {
                            path: path.clone(),
                            adapter: declaration.name.clone(),
                            digest: found.clone(),
                            records: true,
                        });
                        continue;
                    }
                    let found_bytes = digest_file(&target)?.map(|(_, n)| n).unwrap_or(0);
                    out.push(Named::Replace(Replacement {
                        path: path.clone(),
                        adapter: declaration.name.clone(),
                        recorded: expected.clone(),
                        found: found.clone(),
                        found_bytes,
                        plan_id: plan_id(path, &declaration.name, expected, found, &replacement),
                        replacement,
                        replacement_bytes: file.contents.len() as u64,
                        contents: file.contents.clone(),
                    }));
                }
                other => out.push(refused(format!(
                    "{}; only a drifted managed file is replaced",
                    other.describe()
                ))),
            }
            continue;
        }

        if computed.writes.iter().any(|w| &w.path == path) {
            let on_disk = digest_file(&resolve(root, path))?;
            match (recorded, on_disk) {
                (Some(e), Some((found, _))) if found == replacement && e.digest == replacement => {
                    out.push(Named::AlreadySatisfied {
                        path: path.clone(),
                        adapter: declaration.name.clone(),
                        digest: found,
                        records: false,
                    });
                }
                (_, None) => out.push(refused(
                    "not drifted: the file is absent, and an ordinary apply writes it".to_string(),
                )),
                _ => out.push(refused(
                    "not drifted: it holds the bytes this product last wrote, and an ordinary apply rewrites it"
                        .to_string(),
                )),
            }
            continue;
        }

        // Declared, but the plan neither writes nor withholds it: the adapter
        // is not claiming its paths.
        let why = computed
            .adapters
            .iter()
            .find(|a| a.name == declaration.name)
            .map(|a| match &a.readiness {
                crate::adapter::Readiness::Refused { missing } => {
                    format!("missing {}", missing.join(", "))
                }
                _ => "not claiming".to_string(),
            })
            .unwrap_or_else(|| "not configured".to_string());
        out.push(refused(format!(
            "adapter {} does not claim its paths ({why}), so nothing is written for it",
            declaration.name
        )));
    }
    Ok(out)
}

/// Why a path is not one this product may rewrite in place, if it is not.
///
/// Anything [`malformed`] refuses, and anything under `.statecraft/` or under a
/// `.git` component: the product's own area and the repository's database are
/// never the target of a rewrite on a record's say-so.
pub(crate) fn protected(path: &str) -> Option<&'static str> {
    if let Some(reason) = malformed(path) {
        return Some(reason);
    }
    let mut components = path.split('/');
    if components.next() == Some(".statecraft") {
        return Some("a path under `.statecraft/` is this product's own area");
    }
    if path.split('/').any(|c| c == ".git") {
        return Some("a path under a `.git` component is refused");
    }
    None
}

fn not_regular(what: &str) -> std::io::Error {
    std::io::Error::other(what.to_string())
}

/// Refuse, as an i/o error naming why, unless the path is a regular file
/// reached through no symbolic link, or is absent when `absent_ok`.
///
/// Called immediately before every rename, so what the plan checked is checked
/// again at the last moment the standard library allows. The standard library
/// offers no rename relative to a directory handle, so a window of one system
/// call remains between this check and the rename.
pub(crate) fn check_target(
    root: &Path,
    repo_relative: &str,
    absent_ok: bool,
) -> std::io::Result<()> {
    if link_on_path(root, repo_relative)? {
        return Err(not_regular(&format!(
            "{repo_relative} or a directory on the way to it is a symbolic link"
        )));
    }
    match std::fs::symlink_metadata(resolve(root, repo_relative)) {
        Ok(m) if m.file_type().is_file() => Ok(()),
        Ok(_) => Err(not_regular(&format!(
            "{repo_relative} is not a regular file"
        ))),
        Err(e) if absent_ok && e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Remove every `*.tmp` a staged write interrupted before its rename left
/// under [`STAGING_DIR`], returning their names. Never follows a link.
pub fn sweep(root: &Path) -> std::io::Result<Vec<String>> {
    if link_on_path(root, STAGING_DIR)? {
        return Err(not_regular(&format!(
            "{STAGING_DIR} is reached through a symbolic link"
        )));
    }
    let dir = resolve(root, STAGING_DIR);
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) if e.kind() == std::io::ErrorKind::NotADirectory => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".tmp") && entry.file_type()?.is_file() {
            std::fs::remove_file(entry.path())?;
            out.push(format!("{STAGING_DIR}/{name}"));
        }
    }
    out.sort();
    Ok(out)
}

/// Stage bytes under [`STAGING_DIR`], flushed and ready to be renamed into
/// place, carrying `like`'s permissions when it names an existing file.
pub(crate) fn stage(
    root: &Path,
    contents: &[u8],
    n: usize,
    like: Option<&Path>,
) -> std::io::Result<PathBuf> {
    use std::io::Write as _;
    if link_on_path(root, STAGING_DIR)? {
        return Err(not_regular(&format!(
            "{STAGING_DIR} is reached through a symbolic link"
        )));
    }
    let dir = resolve(root, STAGING_DIR);
    std::fs::create_dir_all(&dir)?;
    let staged = dir.join(format!("{}-{n}.tmp", std::process::id()));
    let mut file = std::fs::File::create(&staged)?;
    file.write_all(contents)?;
    if let Some(like) = like {
        match std::fs::symlink_metadata(like) {
            Ok(m) => file.set_permissions(m.permissions())?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    file.sync_all()?;
    Ok(staged)
}

/// Write a file by staging it and renaming it into place, keeping its
/// permissions. Readers see the old bytes or the new ones. The target must be
/// a regular file reached through no link, checked immediately before the
/// rename.
pub(crate) fn write_atomically(
    root: &Path,
    repo_relative: &str,
    contents: &[u8],
) -> std::io::Result<()> {
    let target = resolve(root, repo_relative);
    let staged = stage(root, contents, 0, Some(&target))?;
    let renamed =
        check_target(root, repo_relative, false).and_then(|()| std::fs::rename(&staged, &target));
    if renamed.is_err() {
        let _ = std::fs::remove_file(&staged);
    }
    renamed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_identity_binds_every_digest_and_the_path() {
        let base = plan_id("p", "a", "r", "f", "x");
        assert_eq!(base.len(), 64);
        assert_ne!(base, plan_id("q", "a", "r", "f", "x"));
        assert_ne!(base, plan_id("p", "b", "r", "f", "x"));
        assert_ne!(base, plan_id("p", "a", "s", "f", "x"));
        assert_ne!(base, plan_id("p", "a", "r", "g", "x"));
        assert_ne!(base, plan_id("p", "a", "r", "f", "y"));
        assert_eq!(base, plan_id("p", "a", "r", "f", "x"));
    }

    #[test]
    fn a_path_that_escapes_or_is_not_relative_is_malformed() {
        for p in ["", "/abs", "a/../b", "./a", "a//b", "a\\b", ".."] {
            assert!(malformed(p).is_some(), "{p}");
        }
        assert!(malformed(".claude/statecraft/instructions.md").is_none());
    }

    #[test]
    fn the_products_area_and_git_are_protected() {
        for p in [
            ".statecraft/environment.json",
            ".statecraft",
            "a/.git/x",
            ".git",
            "../x",
        ] {
            assert!(protected(p).is_some(), "{p}");
        }
        assert!(protected("AGENTS.md").is_none());
    }
}
