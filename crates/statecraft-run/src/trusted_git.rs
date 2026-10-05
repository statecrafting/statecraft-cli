//! Supervisor Git invocations use the operator's target, never child metadata.

use crate::workspace::WorkspaceError;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn error(target: &Path, detail: impl ToString) -> WorkspaceError {
    WorkspaceError::Git {
        args: "trusted Git".into(),
        dir: target.display().to_string(),
        detail: detail.to_string(),
    }
}

/// Resolve an executable from absolute, existing PATH directories.
pub fn program() -> Result<PathBuf, WorkspaceError> {
    let path = std::env::var_os("PATH").ok_or_else(|| error(Path::new("/"), "PATH absent"))?;
    for directory in std::env::split_paths(&path) {
        if !directory.is_absolute() {
            return Err(error(&directory, "relative PATH directory"));
        }
        let text = directory.to_string_lossy();
        if text.contains("/.statecraft/state/workspaces/")
            || text.ends_with("/.statecraft/state/workspaces")
        {
            return Err(error(&directory, "workspace PATH directory"));
        }
        // Compared as resolved paths, so a symlinked HOME hides neither root.
        let resolved = canonical_root(&directory);
        if let Some(home) = std::env::var_os("HOME")
            && resolved.starts_with(canonical_root(&Path::new(&home).join(".claude")))
        {
            return Err(error(&directory, "provider-writable PATH directory"));
        }
        if let Some(exchange) = exchange_root()
            && resolved.starts_with(canonical_root(&exchange))
        {
            return Err(error(&directory, "exchange PATH directory"));
        }
    }
    // Git is an operating-system executable. A workspace cannot supply it.
    Path::new("/usr/bin/git")
        .canonicalize()
        .map_err(|e| error(Path::new("/usr/bin/git"), e))
}

/// The product home's exchange directory: `STATECRAFT_HOME`, or `~/.statecraft`.
fn exchange_root() -> Option<PathBuf> {
    match std::env::var_os("STATECRAFT_HOME") {
        Some(home) => Some(Path::new(&home).join("exchange")),
        None => std::env::var_os("HOME").map(|home| Path::new(&home).join(".statecraft/exchange")),
    }
}

fn safe(program: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.attributesFile=/dev/null",
            "-c",
            "diff.external=",
            "-c",
            "core.pager=cat",
        ]);
    command
}

fn ordinary_bytes(path: &Path, limit: usize) -> Result<Vec<u8>, WorkspaceError> {
    use rustix::fs::{Mode, OFlags, open};
    use std::io::Read;
    let file = open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|e| error(path, e))?;
    let file = std::fs::File::from(file);
    if !file.metadata().map_err(|e| error(path, e))?.is_file() {
        return Err(error(path, "configuration is not an ordinary file"));
    }
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| error(path, e))?;
    if bytes.len() > limit {
        return Err(error(path, "configuration exceeds its size bound"));
    }
    Ok(bytes)
}

fn operator_path(target: &Path, common: &Path, path: &Path) -> Result<PathBuf, WorkspaceError> {
    let resolved = path.canonicalize().map_err(|e| error(path, e))?;
    let mut prohibited = vec![
        target.join(".statecraft/state/workspaces"),
        common.join("worktrees"),
        common.join("refs/heads/statecraft"),
        common.join("logs/refs/heads/statecraft"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        prohibited.extend([
            Path::new(&home).join(".claude"),
            Path::new(&home).join(".claude.json"),
        ]);
    }
    prohibited.extend(exchange_root());
    let text = resolved.to_string_lossy();
    if text.contains("/.statecraft/state/workspaces/")
        || resolved
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with(".claude.json."))
    {
        return Err(error(
            path,
            "supervisor configuration points into a child-writable root",
        ));
    }
    if prohibited
        .iter()
        .any(|root| resolved.starts_with(canonical_root(root)))
    {
        return Err(error(
            path,
            "supervisor configuration points into a child-writable root",
        ));
    }
    Ok(resolved)
}

/// A prohibited root as the resolved path would spell it. A root that does not
/// exist yet keeps its name under its nearest existing, canonical ancestor, so
/// a symlinked `HOME` cannot hide it.
fn canonical_root(root: &Path) -> PathBuf {
    let mut missing = Vec::new();
    let mut existing = root;
    loop {
        if let Ok(canonical) = existing.canonicalize() {
            return missing
                .iter()
                .rev()
                .fold(canonical, |path, name| path.join(name));
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name);
                existing = parent;
            }
            _ => return root.to_path_buf(),
        }
    }
}

fn configuration(
    target: &Path,
    common: &Path,
    file: &Path,
    depth: usize,
) -> Result<(), WorkspaceError> {
    if depth > 8 {
        return Err(error(file, "configuration include depth exceeds bound"));
    }
    let file = operator_path(target, common, file)?;
    ordinary_bytes(&file, 1024 * 1024)?;
    // Parse an explicit inert file, with include expansion disabled. Validate
    // every destination before any repository operation can expand it.
    let output = safe(&program()?)
        .current_dir("/")
        .args(["config", "--no-includes", "--null", "--list", "--file"])
        .arg(&file)
        .output()
        .map_err(|e| error(&file, e))?;
    if !output.status.success() {
        return Err(error(&file, "operator Git configuration cannot be parsed"));
    }
    for entry in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let entry = std::str::from_utf8(entry).map_err(|e| error(&file, e))?;
        let Some((key, value)) = entry.split_once('\n') else {
            continue;
        };
        if key == "include.path" || (key.starts_with("includeif.") && key.ends_with(".path")) {
            let included = if let Some(relative) = value.strip_prefix("~/") {
                PathBuf::from(
                    std::env::var_os("HOME")
                        .ok_or_else(|| error(&file, "HOME absent for Git include"))?,
                )
                .join(relative)
            } else if Path::new(value).is_absolute() {
                PathBuf::from(value)
            } else {
                file.parent().unwrap().join(value)
            };
            configuration(target, common, &included, depth + 1)?;
        }
    }
    Ok(())
}

fn object_alternates(
    target: &Path,
    common: &Path,
    objects: &Path,
    depth: usize,
) -> Result<(), WorkspaceError> {
    if depth > 8 {
        return Err(error(objects, "object alternate depth exceeds bound"));
    }
    let objects = operator_path(target, common, objects)?;
    let path = objects.join("info/alternates");
    if path
        .symlink_metadata()
        .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
    {
        return Ok(());
    }
    // The file is judged where it resolves, so a linked `info` cannot carry it
    // into a child-writable root.
    let path = operator_path(target, common, &path)?;
    let bytes = ordinary_bytes(&path, 64 * 1024)?;
    let text = std::str::from_utf8(&bytes).map_err(|e| error(&path, e))?;
    // Git skips empty lines and lines that begin with `#`.
    for line in text
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let alternate = if Path::new(line).is_absolute() {
            PathBuf::from(line)
        } else {
            objects.join(line)
        };
        object_alternates(target, common, &alternate, depth + 1)?;
    }
    Ok(())
}

/// Resolve trusted checkout metadata as data before starting Git. A child's
/// worktree pointers and configuration are never used for this resolution.
pub fn common(target: &Path) -> Result<PathBuf, WorkspaceError> {
    let marker = target.join(".git");
    let metadata = marker.symlink_metadata().map_err(|e| error(target, e))?;
    let directory = if metadata.is_dir() {
        marker
    } else if metadata.is_file() {
        let bytes = ordinary_bytes(&marker, 4096)?;
        let text = std::str::from_utf8(&bytes).map_err(|e| error(target, e))?;
        let path = Path::new(
            text.trim()
                .strip_prefix("gitdir: ")
                .ok_or_else(|| error(target, "invalid operator gitdir"))?,
        );
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            target.join(path)
        }
    } else {
        return Err(error(
            target,
            "operator Git marker is not an ordinary file or directory",
        ));
    };
    let directory = directory.canonicalize().map_err(|e| error(target, e))?;
    let pointer = directory.join("commondir");
    let common = if pointer
        .symlink_metadata()
        .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
    {
        directory
    } else {
        let bytes = ordinary_bytes(&pointer, 4096)?;
        let text = std::str::from_utf8(&bytes).map_err(|e| error(target, e))?;
        let path = Path::new(text.trim());
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            directory.join(path)
        }
    }
    .canonicalize()
    .map_err(|e| error(target, e))?;
    configuration(target, &common, &common.join("config"), 0)?;
    if !common
        .join("config.worktree")
        .symlink_metadata()
        .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
    {
        configuration(target, &common, &common.join("config.worktree"), 0)?;
    }
    object_alternates(target, &common, &common.join("objects"), 0)?;
    Ok(common)
}

/// Build a command anchored to the trusted common directory.
pub fn command(target: &Path) -> Result<Command, WorkspaceError> {
    let program = program()?;
    // One resolution, so the command and its filter probe name one directory.
    let common = common(target)?;
    let mut command = safe(&program);
    command
        .arg("--git-dir")
        .arg(&common)
        .arg("--work-tree")
        .arg(target);
    // Local filter configuration cannot select an executable during checkout.
    let configuration = safe(&program)
        .arg("--git-dir")
        .arg(&common)
        .args([
            "config",
            "--local",
            "--name-only",
            "--get-regexp",
            "^filter\\.",
        ])
        .output()
        .map_err(|e| error(target, e))?;
    for name in String::from_utf8_lossy(&configuration.stdout).lines() {
        if let Some(driver) = name
            .strip_prefix("filter.")
            .and_then(|name| name.rsplit_once('.').map(|(driver, _)| driver))
        {
            for suffix in ["clean", "smudge", "process"] {
                command.args(["-c", &format!("filter.{driver}.{suffix}=")]);
            }
            command.args(["-c", &format!("filter.{driver}.required=false")]);
        }
    }
    Ok(command)
}

/// Run a supervisor Git operation, returning bytes without text conversion.
pub fn output(target: &Path, args: &[&str]) -> Result<Output, WorkspaceError> {
    let output = command(target)?
        .args(args)
        .current_dir(target)
        .output()
        .map_err(|e| error(target, e))?;
    if !output.status.success() {
        return Err(error(target, String::from_utf8_lossy(&output.stderr)));
    }
    Ok(output)
}

/// Read a loose private reference as bounded data, without reading its objects.
/// Each component is opened without following links in the trusted common dir.
pub fn private_reference(target: &Path, branch: &str) -> Result<String, WorkspaceError> {
    use rustix::fs::{Mode, OFlags, open, openat};
    use std::io::Read;
    let relative = Path::new(branch);
    let components: Vec<_> = relative.components().collect();
    if !branch.starts_with("refs/heads/statecraft/")
        || components
            .iter()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(error(target, "invalid private reference path"));
    }
    let common = common(target)?;
    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW;
    let loose = || -> rustix::io::Result<_> {
        let mut directory = open(&common, flags | OFlags::DIRECTORY, Mode::empty())?;
        for component in &components[..components.len() - 1] {
            directory = openat(
                &directory,
                component.as_os_str(),
                flags | OFlags::DIRECTORY,
                Mode::empty(),
            )?;
        }
        openat(
            &directory,
            components.last().unwrap().as_os_str(),
            flags | OFlags::NONBLOCK,
            Mode::empty(),
        )
    };
    let file = match loose() {
        Ok(file) => file,
        // Git packs references on gc; an absent loose file is read there.
        Err(rustix::io::Errno::NOENT) => return packed_reference(target, &common, branch),
        Err(e) => return Err(error(target, e)),
    };
    let file = std::fs::File::from(file);
    if !file.metadata().map_err(|e| error(target, e))?.is_file() {
        return Err(error(target, "private reference is not an ordinary file"));
    }
    let mut bytes = Vec::new();
    file.take(129)
        .read_to_end(&mut bytes)
        .map_err(|e| error(target, e))?;
    if bytes.len() > 128 {
        return Err(error(
            target,
            "private reference is not a bounded object identity",
        ));
    }
    let text = std::str::from_utf8(&bytes).map_err(|e| error(target, e))?;
    object_identity(target, text.trim())
}

fn object_identity(target: &Path, text: &str) -> Result<String, WorkspaceError> {
    if !matches!(text.len(), 40 | 64) || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(error(
            target,
            "private reference is not a bounded object identity",
        ));
    }
    Ok(text.into())
}

/// A private reference from the common directory's `packed-refs`, read as an
/// ordinary file with the same bounds as configuration.
fn packed_reference(target: &Path, common: &Path, branch: &str) -> Result<String, WorkspaceError> {
    let bytes = ordinary_bytes(&common.join("packed-refs"), 64 * 1024 * 1024)?;
    let text = std::str::from_utf8(&bytes).map_err(|e| error(target, e))?;
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.starts_with('^'))
        .find_map(|line| {
            line.split_once(' ')
                .filter(|(_, name)| *name == branch)
                .map(|(identity, _)| identity)
        })
        .ok_or_else(|| error(target, "private reference not found"))
        .and_then(|identity| object_identity(target, identity))
}

/// Export trusted commit bytes without applying attributes or filter drivers.
/// The destination must be an empty supervisor-owned directory.
pub fn export(target: &Path, revision: &str, destination: &Path) -> Result<(), WorkspaceError> {
    use std::os::unix::fs::PermissionsExt;
    if revision.is_empty() || revision.starts_with('-') {
        return Err(error(target, "export revision is not a revision"));
    }
    let entries = output(target, &["ls-tree", "-rz", "--full-tree", revision])?;
    let mut total = 0usize;
    for entry in entries
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let tab = entry
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| error(target, "malformed tree entry"))?;
        let header = std::str::from_utf8(&entry[..tab]).map_err(|e| error(target, e))?;
        let fields: Vec<_> = header.split_whitespace().collect();
        let name = std::str::from_utf8(&entry[tab + 1..]).map_err(|e| error(target, e))?;
        let relative = Path::new(name);
        if relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
            || relative.components().any(|component| {
                component
                    .as_os_str()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(".git")
            })
        {
            return Err(error(target, "unsafe export path"));
        }
        if fields.len() != 3 || fields[1] != "blob" {
            return Err(error(target, "unsupported export tree entry"));
        }
        let size = output(target, &["cat-file", "-s", fields[2]])?.stdout;
        let size: usize = std::str::from_utf8(&size)
            .map_err(|e| error(target, e))?
            .trim()
            .parse()
            .map_err(|e| error(target, e))?;
        if size > 64 * 1024 * 1024 {
            return Err(error(target, "export blob exceeds bound"));
        }
        let bytes = output(target, &["cat-file", "blob", fields[2]])?.stdout;
        total = total
            .checked_add(bytes.len())
            .ok_or_else(|| error(target, "export size overflow"))?;
        if bytes.len() > 64 * 1024 * 1024 || total > 1024 * 1024 * 1024 {
            return Err(error(target, "export exceeds bounded size"));
        }
        let path = destination.join(relative);
        let mut parent = destination.to_path_buf();
        for component in relative
            .components()
            .take(relative.components().count().saturating_sub(1))
        {
            parent.push(component.as_os_str());
            match std::fs::symlink_metadata(&parent) {
                Ok(metadata) if metadata.is_dir() && !metadata.is_symlink() => {}
                Ok(_) => {
                    return Err(error(
                        target,
                        "export ancestor is not an ordinary directory",
                    ));
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    std::fs::create_dir(&parent).map_err(|e| error(target, e))?
                }
                Err(e) => return Err(error(target, e)),
            }
        }
        if std::fs::symlink_metadata(&path).is_ok() {
            return Err(error(target, "duplicate export path"));
        }
        match fields[0] {
            "120000" => {
                use std::os::unix::ffi::OsStrExt;
                if !contained_link(relative, &bytes) {
                    return Err(error(target, "export symlink leaves the tree"));
                }
                std::os::unix::fs::symlink(std::ffi::OsStr::from_bytes(&bytes), &path)
                    .map_err(|e| error(target, e))?;
            }
            "100644" | "100755" => {
                std::fs::write(&path, bytes).map_err(|e| error(target, e))?;
                std::fs::set_permissions(
                    &path,
                    std::fs::Permissions::from_mode(if fields[0] == "100755" {
                        0o755
                    } else {
                        0o644
                    }),
                )
                .map_err(|e| error(target, e))?;
            }
            _ => return Err(error(target, "unsupported export file mode")),
        }
    }
    Ok(())
}

/// Whether a symlink at `link`, relative to the export root, resolves inside
/// the tree whatever the other links in it say. The target is relative, its
/// `..` components all lead and climb no higher than the root, and the rest
/// descends. Every ancestor the export writes is an ordinary directory, so a
/// leading `..` is lexical, and a link the descent meets obeys this rule too.
/// A `..` after a name is refused even when it reads as inside (`c/../d`): if
/// `c` is itself a link, the kernel climbs from its target, not from `c`.
fn contained_link(link: &Path, target: &[u8]) -> bool {
    use std::os::unix::ffi::OsStrExt;
    use std::path::Component;
    let target = Path::new(std::ffi::OsStr::from_bytes(target));
    let mut climbs = 0usize;
    let mut descending = false;
    for component in target.components() {
        match component {
            Component::ParentDir if !descending => climbs += 1,
            Component::CurDir => {}
            Component::Normal(_) => descending = true,
            _ => return false,
        }
    }
    !target.as_os_str().is_empty() && climbs < link.components().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository() -> tempfile::TempDir {
        let target = tempfile::tempdir().unwrap();
        assert!(
            Command::new("/usr/bin/git")
                .args(["init", "--quiet"])
                .arg(target.path())
                .status()
                .unwrap()
                .success()
        );
        target
    }

    #[test]
    fn child_configuration_includes_are_refused_before_repository_git() {
        let target = repository();
        let child = target.path().join(".statecraft/state/workspaces/attempt");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::write(child.join("config"), "[core]\n hooksPath = /untrusted\n").unwrap();
        let config = target.path().join(".git/config");
        let initial = std::fs::read_to_string(&config).unwrap();
        std::fs::write(
            config,
            format!(
                "{initial}\n[include]\n path = {}\n",
                child.join("config").display()
            ),
        )
        .unwrap();
        assert!(
            common(target.path())
                .unwrap_err()
                .to_string()
                .contains("child-writable root")
        );
    }

    #[test]
    fn child_object_alternates_are_refused_before_repository_git() {
        let target = repository();
        let objects = target
            .path()
            .join(".statecraft/state/workspaces/attempt/objects");
        std::fs::create_dir_all(&objects).unwrap();
        std::fs::write(
            target.path().join(".git/objects/info/alternates"),
            format!("{}\n", objects.display()),
        )
        .unwrap();
        assert!(
            common(target.path())
                .unwrap_err()
                .to_string()
                .contains("child-writable root")
        );
    }

    #[test]
    fn a_missing_root_is_spelled_under_its_canonical_ancestor() {
        let scratch = tempfile::tempdir().unwrap();
        let real = scratch.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let link = scratch.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert_eq!(
            canonical_root(&link.join(".claude/projects")),
            real.canonicalize().unwrap().join(".claude/projects")
        );
    }

    #[test]
    fn an_option_is_not_an_export_revision() {
        let target = repository();
        let destination = tempfile::tempdir().unwrap();
        let refused = export(target.path(), "--output=/tmp/x", destination.path()).unwrap_err();
        assert!(refused.to_string().contains("not a revision"));
    }

    #[test]
    fn commented_object_alternates_are_skipped() {
        let target = repository();
        std::fs::write(
            target.path().join(".git/objects/info/alternates"),
            "# /does/not/exist\n\n",
        )
        .unwrap();
        assert!(common(target.path()).is_ok());
    }

    #[test]
    fn export_symlinks_must_stay_inside_the_tree() {
        let link = Path::new("a/b/link");
        for inside in ["c", "./c/d", "../c", "../../c", "..", "../.."] {
            assert!(contained_link(link, inside.as_bytes()), "{inside}");
        }
        for outside in [
            "",
            "/etc/passwd",
            "../../..",
            "c/../..",
            "c/../d",
            "../c/../..",
        ] {
            assert!(!contained_link(link, outside.as_bytes()), "{outside}");
        }
        assert!(!contained_link(Path::new("link"), b".."));
    }

    #[test]
    fn private_reference_is_inert_even_before_its_object_is_imported() {
        let target = repository();
        let directory = target.path().join(".git/refs/heads/statecraft/attempt");
        std::fs::create_dir_all(&directory).unwrap();
        let identity = "a".repeat(40);
        std::fs::write(directory.join("work"), format!("{identity}\n")).unwrap();
        assert_eq!(
            private_reference(target.path(), "refs/heads/statecraft/attempt/work").unwrap(),
            identity
        );
        std::fs::write(directory.join("work"), "ref: /untrusted\n").unwrap();
        assert!(private_reference(target.path(), "refs/heads/statecraft/attempt/work").is_err());
        std::fs::remove_file(directory.join("work")).unwrap();
        std::os::unix::fs::symlink(target.path().join(".git/config"), directory.join("work"))
            .unwrap();
        assert!(private_reference(target.path(), "refs/heads/statecraft/attempt/work").is_err());
    }

    #[test]
    fn a_packed_private_reference_is_read_from_packed_refs() {
        let target = repository();
        let identity = "b".repeat(40);
        std::fs::write(
            target.path().join(".git/packed-refs"),
            format!(
                "# pack-refs with: peeled fully-peeled sorted\n{} refs/heads/main\n{identity} refs/heads/statecraft/attempt/work\n^{}\n",
                "c".repeat(40),
                "d".repeat(40)
            ),
        )
        .unwrap();
        assert_eq!(
            private_reference(target.path(), "refs/heads/statecraft/attempt/work").unwrap(),
            identity
        );
        assert!(private_reference(target.path(), "refs/heads/statecraft/other/work").is_err());
    }
}
