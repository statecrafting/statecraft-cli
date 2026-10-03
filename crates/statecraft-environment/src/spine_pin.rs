//! Spec 033: one canonical pin representation and a byte-preserving atomic edit.

use crate::digest::digest_bytes;
use serde::{Deserialize, Serialize};
use std::ops::Range;
use std::path::{Path, PathBuf};

/// The exact request's canonical release, without its leading equals sign.
pub fn exact(request: &str) -> Result<&str, String> {
    let version = request
        .strip_prefix('=')
        .ok_or("not an exact pin: required_version must be =X.Y.Z")?;
    let parts: Vec<_> = version.split('.').collect();
    if parts.len() != 3
        || parts.iter().any(|p| {
            p.is_empty()
                || !p.bytes().all(|b| b.is_ascii_digit())
                || (p.len() > 1 && p.starts_with('0'))
        })
    {
        return Err(
            "the pin must be canonical =X.Y.Z, without whitespace, leading zeros or suffixes"
                .into(),
        );
    }
    Ok(version)
}

/// A supported required_version value and the bytes inside its double quotes.
#[derive(Debug, Clone)]
pub struct Value {
    /// The complete requirement, including its equals sign when exact.
    pub requirement: String,
    /// Its byte range in the original document.
    pub range: Range<usize>,
}

/// Validate TOML and locate a single-line double-quoted value in explicit [meta].
/// Alternate TOML representations are deliberately refused for this edit.
pub fn value(text: &str) -> Result<Value, String> {
    let parsed: toml::Value =
        toml::from_str(text).map_err(|e| format!("invalid spec-spine.toml: {e}"))?;
    let expected = parsed
        .get("meta")
        .and_then(|m| m.get("required_version"))
        .ok_or("[meta] carries no required_version")?
        .as_str()
        .ok_or("required_version is not a quoted version string")?;
    // Bind the lexical edit to the TOML parser's actual value span. Text
    // resembling a table/key inside an unrelated multiline string must
    // never become the authored value we replace.
    #[derive(Deserialize)]
    struct Meta {
        required_version: toml::Spanned<String>,
    }
    #[derive(Deserialize)]
    struct Document {
        meta: Meta,
    }
    let document: Document = toml::from_str(text).map_err(|e| e.to_string())?;
    let span = document.meta.required_version.span();
    let mut meta = false;
    let mut offset = 0;
    let mut found = None;
    for raw in text.split_inclusive('\n') {
        let line = raw.trim();
        if line.starts_with('[') {
            meta = line
                .split('#')
                .next()
                .unwrap_or("")
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                == "[meta]";
        } else if meta
            && !line.starts_with('#')
            && let Some(rest) = line
                .strip_prefix("required_version")
                .and_then(|r| r.trim_start().strip_prefix('='))
        {
            let quoted = rest.trim_start();
            let inner = quoted
                .strip_prefix('"')
                .ok_or("required_version needs a single-line double-quoted value")?;
            let end = inner
                .find('"')
                .ok_or("required_version has no closing quote")?;
            let requirement = &inner[..end];
            let tail = inner[end + 1..].trim();
            if requirement.contains('\\')
                || (!tail.is_empty() && !tail.starts_with('#'))
                || requirement != expected
                || found.is_some()
            {
                return Err("unsupported required_version representation".into());
            }
            let start = offset + raw.len() - raw.trim_start().len() + line.len() - quoted.len() + 1;
            if start != span.start + 1 || start + end + 1 != span.end {
                offset += raw.len();
                continue;
            }
            found = Some(Value {
                requirement: requirement.into(),
                range: start..start + end,
            });
        }
        offset += raw.len();
    }
    found.ok_or_else(|| {
        "required_version must use an explicit [meta] table and a plain double-quoted value".into()
    })
}

/// The consented pin action reported by plan and apply and recorded in the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Disposition {
    /// Original canonical release.
    pub source: String,
    /// Requested canonical release.
    pub target: String,
    /// Digest of the original complete configuration.
    pub original_digest: String,
    /// Byte range of the consented value edit.
    pub value_range: Range<usize>,
    /// Whether a write is necessary.
    pub changes: bool,
    /// Complete initialization consent identity.
    pub plan_identity: String,
    /// Observed disposition: planned, unchanged, written, or not-written.
    pub outcome: String,
    /// Whether the committed environment manifest durably records this action.
    #[serde(default)]
    pub recorded: bool,
}

/// A validated edit, with its original and proposed bytes kept only in memory.
#[derive(Debug, Clone)]
pub struct Edit {
    /// Canonical project root.
    pub root: PathBuf,
    /// The reportable consent facts.
    pub disposition: Disposition,
    original: Vec<u8>,
    /// Proposed complete configuration, never persisted during planning.
    pub proposed: String,
}

impl Edit {
    /// Read a contained regular configuration and prepare a minimal value edit.
    pub fn plan(root: &Path, request: &str) -> Result<Self, String> {
        let target = exact(request)?.to_string();
        let root = std::fs::canonicalize(root).map_err(|e| e.to_string())?;
        let original = read(&root)?;
        let text = std::str::from_utf8(&original).map_err(|e| e.to_string())?;
        let value = value(text)?;
        let source = exact(&value.requirement)?.to_string();
        let mut proposed = text.to_string();
        proposed.replace_range(value.range.clone(), request);
        Ok(Self {
            root,
            disposition: Disposition {
                changes: source != target,
                source,
                target,
                original_digest: digest_bytes(&original),
                value_range: value.range,
                plan_identity: String::new(),
                outcome: "planned".into(),
                recorded: false,
            },
            original,
            proposed,
        })
    }

    /// Refuse an original configuration that changed after review.
    pub fn revalidate(&self) -> Result<(), String> {
        if read(&self.root)? != self.original {
            return Err("spec-spine.toml changed after planning; review a new plan".into());
        }
        Ok(())
    }

    /// Stage, revalidate and atomically replace only the value bytes.
    pub fn apply(&self) -> Result<(), String> {
        self.revalidate()?;
        if !self.disposition.changes {
            return Ok(());
        }
        atomic(self)
    }

    /// Whether the proposed configuration is actually in force after an attempt.
    pub fn committed(&self) -> bool {
        read(&self.root).is_ok_and(|b| b == self.proposed.as_bytes())
    }
}

#[cfg(unix)]
fn directory(root: &Path) -> Result<std::fs::File, String> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::DIRECTORY).bits() as i32)
        .open(root)
        .map_err(|e| format!("pin root: {e}"))
}

#[cfg(unix)]
fn read_at(dir: &std::fs::File) -> Result<Vec<u8>, String> {
    use rustix::fs::{Mode, OFlags, openat};
    use std::io::Read;
    let fd = openat(
        dir,
        "spec-spine.toml",
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|e| format!("spec-spine.toml must be a contained regular file: {e}"))?;
    let mut file = std::fs::File::from(fd);
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("spec-spine.toml is not a regular file".into());
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

#[cfg(unix)]
fn read(root: &Path) -> Result<Vec<u8>, String> {
    read_at(&directory(root)?)
}

#[cfg(unix)]
fn atomic(edit: &Edit) -> Result<(), String> {
    use rustix::fs::{AtFlags, Mode, OFlags, openat, renameat, unlinkat};
    use std::io::Write;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let dir = directory(&edit.root)?;
    let name = format!(
        ".spec-spine.toml.{}.{}.tmp",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    );
    let mode = std::fs::symlink_metadata(edit.root.join("spec-spine.toml"))
        .map_err(|e| e.to_string())?
        .permissions()
        .mode();
    let fd = openat(
        &dir,
        name.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW,
        Mode::from_raw_mode(mode as _),
    )
    .map_err(|e| format!("stage pin: {e}"))?;
    let result = (|| {
        let mut file = std::fs::File::from(fd);
        file.set_permissions(std::fs::Permissions::from_mode(mode))
            .map_err(|e| e.to_string())?;
        file.write_all(edit.proposed.as_bytes())
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        let held = dir.metadata().map_err(|e| e.to_string())?;
        let live = directory(&edit.root)?
            .metadata()
            .map_err(|e| e.to_string())?;
        if held.dev() != live.dev() || held.ino() != live.ino() || read_at(&dir)? != edit.original {
            return Err("pin containment or bytes changed before rename".into());
        }
        renameat(&dir, name.as_str(), &dir, "spec-spine.toml")
            .map_err(|e| format!("commit pin: {e}"))?;
        dir.sync_all()
            .map_err(|e| format!("pin written but directory durability failed: {e}"))
    })();
    let _ = unlinkat(&dir, name.as_str(), AtFlags::empty());
    result
}

#[cfg(not(unix))]
fn read(_root: &Path) -> Result<Vec<u8>, String> {
    Err("safe pin edits require no-follow filesystem operations".into())
}
#[cfg(not(unix))]
fn atomic(_edit: &Edit) -> Result<(), String> {
    Err("safe atomic pin edits are unavailable on this platform".into())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn text_inside_multiline_values_cannot_authorize_an_edit_of_a_dotted_pin() {
        let text = "decoy = '''\n[meta]\nrequired_version = \"=0.1.0\"\n'''\nmeta.required_version = \"=0.1.0\"\n";
        assert!(value(text).is_err());
    }
    #[test]
    fn staging_revalidates_bytes_before_rename_and_cleans_its_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec-spine.toml");
        std::fs::write(&path, "[meta]\nrequired_version=\"=0.1.0\"\n").unwrap();
        let edit = Edit::plan(dir.path(), "=0.2.0").unwrap();
        let changed = "[meta]\nrequired_version=\"=0.1.0\" # concurrent edit\n";
        std::fs::write(&path, changed).unwrap();
        assert!(atomic(&edit).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), changed);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn a_link_replaced_after_planning_is_not_followed_or_renamed_over() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec-spine.toml");
        std::fs::write(&path, "[meta]\nrequired_version=\"=0.1.0\"\n").unwrap();
        let edit = Edit::plan(dir.path(), "=0.2.0").unwrap();
        let foreign = dir.path().join("foreign");
        std::fs::write(&foreign, "foreign bytes").unwrap();
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&foreign, &path).unwrap();
        assert!(atomic(&edit).is_err());
        assert!(path.is_symlink());
        assert_eq!(std::fs::read_to_string(&foreign).unwrap(), "foreign bytes");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }
}
