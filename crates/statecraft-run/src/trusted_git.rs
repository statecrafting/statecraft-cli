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
        if let Some(home) = std::env::var_os("HOME")
            && directory.starts_with(Path::new(&home).join(".claude"))
        {
            return Err(error(&directory, "provider-writable PATH directory"));
        }
        if let Some(home) = std::env::var_os("STATECRAFT_HOME")
            && directory.starts_with(Path::new(&home).join("exchange"))
        {
            return Err(error(&directory, "exchange PATH directory"));
        }
    }
    // Git is an operating-system executable. A workspace cannot supply it.
    Path::new("/usr/bin/git")
        .canonicalize()
        .map_err(|e| error(Path::new("/usr/bin/git"), e))
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
    if let Some(home) = std::env::var_os("STATECRAFT_HOME") {
        prohibited.push(Path::new(&home).join("exchange"));
    }
    if std::env::var_os("STATECRAFT_HOME").is_none()
        && let Some(home) = std::env::var_os("HOME")
    {
        prohibited.push(Path::new(&home).join(".statecraft/exchange"));
    }
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
        .any(|root| resolved.starts_with(root.canonicalize().unwrap_or_else(|_| root.clone())))
    {
        return Err(error(
            path,
            "supervisor configuration points into a child-writable root",
        ));
    }
    Ok(resolved)
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
    let bytes = ordinary_bytes(&path, 64 * 1024)?;
    let text = std::str::from_utf8(&bytes).map_err(|e| error(&path, e))?;
    for line in text.lines().filter(|line| !line.is_empty()) {
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
    if common.join("config.worktree").exists() {
        configuration(target, &common, &common.join("config.worktree"), 0)?;
    }
    object_alternates(target, &common, &common.join("objects"), 0)?;
    Ok(common)
}

/// Build a command anchored to the trusted common directory.
pub fn command(target: &Path) -> Result<Command, WorkspaceError> {
    let mut command = safe(&program()?);
    command
        .arg("--git-dir")
        .arg(common(target)?)
        .arg("--work-tree")
        .arg(target);
    // Local filter configuration cannot select an executable during checkout.
    let configuration = safe(&program()?)
        .arg("--git-dir")
        .arg(common(target)?)
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
    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW;
    let mut directory = open(common(target)?, flags | OFlags::DIRECTORY, Mode::empty())
        .map_err(|e| error(target, e))?;
    for component in &components[..components.len() - 1] {
        directory = openat(
            &directory,
            component.as_os_str(),
            flags | OFlags::DIRECTORY,
            Mode::empty(),
        )
        .map_err(|e| error(target, e))?;
    }
    let file = openat(
        &directory,
        components.last().unwrap().as_os_str(),
        flags | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|e| error(target, e))?;
    let file = std::fs::File::from(file);
    if !file.metadata().map_err(|e| error(target, e))?.is_file() {
        return Err(error(target, "private reference is not an ordinary file"));
    }
    let mut bytes = Vec::new();
    file.take(129)
        .read_to_end(&mut bytes)
        .map_err(|e| error(target, e))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| error(target, e))?
        .trim();
    if bytes.len() > 128
        || !matches!(text.len(), 40 | 64)
        || !text.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(error(
            target,
            "private reference is not a bounded object identity",
        ));
    }
    Ok(text.into())
}

/// Export trusted commit bytes without applying attributes or filter drivers.
/// The destination must be an empty supervisor-owned directory.
pub fn export(target: &Path, revision: &str, destination: &Path) -> Result<(), WorkspaceError> {
    use std::os::unix::fs::PermissionsExt;
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
}
