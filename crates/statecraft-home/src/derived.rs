//! The one-time relocation of the compiled artifacts.
//!
//! Spec 010 section 3.9. `.derived/` moves to `.statecraft/derived/`, and the
//! declared configuration and the ignore rules move with it in the same
//! operation. It touches exactly one repository root: it sweeps no sibling
//! repository and no home directory, and there is nothing in this module that
//! could, because every path it forms is joined to the root it was given.
//!
//! A repository holding **both** a non-empty old location and a non-empty new
//! one is **refused**, naming both. Choosing one would silently discard a shard
//! tree, and the two trees are exactly the thing a freshness gate compares
//! against.

use serde::Serialize;
use std::path::Path;

/// Where the compiled artifacts used to live.
pub const OLD: &str = ".derived";

/// Where they live now.
pub const NEW: &str = crate::project::DERIVED;

/// What a relocation would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Verdict {
    /// Nothing to move.
    NotNeeded {
        /// Why not.
        reason: String,
    },
    /// This is what would move.
    Planned {
        /// Repository-relative paths, old location, in order.
        files: Vec<String>,
        /// The configuration lines that would change.
        configuration: Vec<String>,
    },
    /// Both locations hold something.
    Refused {
        /// Why.
        reason: String,
    },
}

impl Verdict {
    /// A one-word rendering.
    pub fn word(&self) -> &'static str {
        match self {
            Verdict::NotNeeded { .. } => "not-needed",
            Verdict::Planned { .. } => "planned",
            Verdict::Refused { .. } => "refused",
        }
    }

    /// A human-readable rendering.
    pub fn render(&self) -> String {
        match self {
            Verdict::NotNeeded { reason } => format!("not needed: {reason}\n"),
            Verdict::Refused { reason } => format!("refused: {reason}\n"),
            Verdict::Planned {
                files,
                configuration,
            } => {
                let mut out = format!("move {} file(s) from {OLD}/ to {NEW}/\n", files.len());
                for line in configuration {
                    out.push_str(&format!("rewrite {line}\n"));
                }
                out
            }
        }
    }
}

/// What a performed relocation did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    /// What was planned.
    pub verdict: Verdict,
    /// Paths moved, at their new location.
    pub moved: Vec<String>,
    /// Configuration files rewritten.
    pub rewritten: Vec<String>,
}

/// Every file under a directory, as paths relative to it.
fn files_under(dir: &Path) -> std::io::Result<Vec<String>> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path
                    .strip_prefix(dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push(rel);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Compute the relocation. Reads the repository; writes nothing.
pub fn plan(root: &Path) -> std::io::Result<Verdict> {
    let old = root.join(OLD);
    let new = statecraft_environment::claimant::resolve(root, NEW);
    let old_files = files_under(&old)?;
    let new_files = files_under(&new)?;

    if old_files.is_empty() {
        return Ok(Verdict::NotNeeded {
            reason: if new_files.is_empty() {
                format!("neither {OLD}/ nor {NEW}/ holds a compiled artifact")
            } else {
                format!("{NEW}/ already holds the compiled artifacts and {OLD}/ is empty")
            },
        });
    }
    if !new_files.is_empty() {
        return Ok(Verdict::Refused {
            reason: format!(
                "{OLD}/ holds {} file(s) and {NEW}/ holds {}; both destinations are \
                 populated and choosing one would discard a shard tree",
                old_files.len(),
                new_files.len()
            ),
        });
    }

    Ok(Verdict::Planned {
        files: old_files,
        configuration: configuration_changes(root)?,
    })
}

/// The configuration lines a relocation would rewrite.
fn configuration_changes(root: &Path) -> std::io::Result<Vec<String>> {
    let mut out = Vec::new();
    for (file, rewrite) in [
        (
            "spec-spine.toml",
            rewrite_toml as fn(&str) -> (String, usize),
        ),
        (".gitignore", rewrite_gitignore),
    ] {
        let path = root.join(file);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let (_, changed) = rewrite(&text);
        if changed > 0 {
            out.push(format!("{file}: {changed} line(s)"));
        }
    }
    Ok(out)
}

/// Rewrite the quoted old path inside a configuration file.
///
/// Only a **quoted string literal** is rewritten. Prose in a comment that
/// mentions the old location is left alone: a relocation that edited English
/// would be guessing at meaning, and the documents are updated by the change
/// that performs the move.
fn rewrite_toml(text: &str) -> (String, usize) {
    let mut changed = 0;
    let out = text
        .lines()
        .map(|line| {
            if line.trim_start().starts_with('#') || !line.contains("\".derived\"") {
                return line.to_string();
            }
            changed += 1;
            line.replace("\".derived\"", &format!("\"{NEW}\""))
        })
        .collect::<Vec<_>>()
        .join("\n");
    (with_trailing_newline(out, text), changed)
}

/// Rewrite ignore patterns that name the old location.
fn rewrite_gitignore(text: &str) -> (String, usize) {
    let mut changed = 0;
    let out = text
        .lines()
        .map(|line| {
            let t = line.trim();
            if t.starts_with('#') || !t.starts_with(".derived/") {
                return line.to_string();
            }
            changed += 1;
            line.replacen(".derived/", &format!("{NEW}/"), 1)
        })
        .collect::<Vec<_>>()
        .join("\n");
    (with_trailing_newline(out, text), changed)
}

fn with_trailing_newline(mut out: String, original: &str) -> String {
    if original.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Perform the relocation.
pub fn apply(root: &Path) -> std::io::Result<Outcome> {
    let verdict = plan(root)?;
    let Verdict::Planned { ref files, .. } = verdict else {
        return Ok(Outcome {
            verdict,
            moved: Vec::new(),
            rewritten: Vec::new(),
        });
    };

    let old = root.join(OLD);
    let new = statecraft_environment::claimant::resolve(root, NEW);
    let mut moved = Vec::new();
    for rel in files {
        let from = statecraft_environment::claimant::resolve(&old, rel);
        let to = statecraft_environment::claimant::resolve(&new, rel);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Rename where the two sit on one filesystem, copy otherwise. The
        // fallback matters: `.statecraft/` can be on a different mount than the
        // repository root is not possible, but a rename across a bind mount in
        // a container is, and losing a shard tree to an `EXDEV` is not a risk
        // worth taking for one syscall.
        match std::fs::rename(&from, &to) {
            Ok(()) => {}
            Err(_) => {
                std::fs::copy(&from, &to)?;
                std::fs::remove_file(&from)?;
            }
        }
        moved.push(format!("{NEW}/{rel}"));
    }
    remove_empty_dirs(&old)?;

    let mut rewritten = Vec::new();
    for (file, rewrite) in [
        (
            "spec-spine.toml",
            rewrite_toml as fn(&str) -> (String, usize),
        ),
        (".gitignore", rewrite_gitignore),
    ] {
        let path = root.join(file);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let (next, changed) = rewrite(&text);
        if changed > 0 {
            std::fs::write(&path, next)?;
            rewritten.push(file.to_string());
        }
    }

    Ok(Outcome {
        verdict,
        moved,
        rewritten,
    })
}

/// Remove a directory tree that holds no file.
fn remove_empty_dirs(dir: &Path) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            remove_empty_dirs(&path)?;
        }
    }
    if std::fs::read_dir(dir)?.next().is_none() {
        std::fs::remove_dir(dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, contents) in files {
            let at = statecraft_environment::claimant::resolve(dir.path(), path);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, contents).unwrap();
        }
        dir
    }

    fn at(dir: &tempfile::TempDir, rel: &str) -> std::path::PathBuf {
        statecraft_environment::claimant::resolve(dir.path(), rel)
    }

    #[test]
    fn a_repository_with_nothing_to_move_says_so() {
        let dir = repo(&[]);
        assert_eq!(plan(dir.path()).unwrap().word(), "not-needed");
    }

    #[test]
    fn a_repository_already_relocated_says_so_too() {
        let dir = repo(&[(".statecraft/derived/spec-registry/a.json", "{}")]);
        assert_eq!(plan(dir.path()).unwrap().word(), "not-needed");
    }

    #[test]
    fn the_shards_move_and_the_configuration_moves_with_them() {
        let dir = repo(&[
            (".derived/spec-registry/by-spec/000.json", "{\"a\":1}"),
            (".derived/codebase-index/by-spec/000.json", "{\"b\":2}"),
            (
                "spec-spine.toml",
                "[layout]\nderived_dir   = \".derived\"\nresolver_exclusions = [\"target\", \".derived\"]\n",
            ),
            (".gitignore", "target\n.derived/**/build-meta.json\n"),
        ]);
        let outcome = apply(dir.path()).unwrap();
        assert_eq!(outcome.moved.len(), 2);
        assert!(at(&dir, ".statecraft/derived/spec-registry/by-spec/000.json").is_file());
        assert!(!dir.path().join(".derived").exists());

        let toml = std::fs::read_to_string(at(&dir, "spec-spine.toml")).unwrap();
        assert!(toml.contains("derived_dir   = \".statecraft/derived\""));
        assert!(toml.contains("\".statecraft/derived\"]"));
        assert!(!toml.contains("\".derived\""));

        let ignore = std::fs::read_to_string(at(&dir, ".gitignore")).unwrap();
        assert!(ignore.contains(".statecraft/derived/**/build-meta.json"));
        assert!(ignore.contains("target"));
        assert_eq!(outcome.rewritten, ["spec-spine.toml", ".gitignore"]);
    }

    #[test]
    fn both_destinations_populated_is_refused_and_moves_nothing() {
        let dir = repo(&[
            (".derived/spec-registry/a.json", "old"),
            (".statecraft/derived/spec-registry/a.json", "new"),
        ]);
        let verdict = plan(dir.path()).unwrap();
        assert_eq!(verdict.word(), "refused");
        match &verdict {
            Verdict::Refused { reason } => {
                assert!(reason.contains(OLD));
                assert!(reason.contains(NEW));
            }
            other => panic!("{other:?}"),
        }
        let outcome = apply(dir.path()).unwrap();
        assert!(outcome.moved.is_empty());
        assert_eq!(
            std::fs::read_to_string(at(&dir, ".derived/spec-registry/a.json")).unwrap(),
            "old"
        );
        assert_eq!(
            std::fs::read_to_string(at(&dir, ".statecraft/derived/spec-registry/a.json")).unwrap(),
            "new"
        );
    }

    #[test]
    fn a_comment_that_mentions_the_old_path_is_not_rewritten() {
        let (out, changed) =
            rewrite_toml("# see .derived and \".derived\" below\nx = \".derived\"\n");
        assert_eq!(changed, 1);
        assert!(out.contains("# see .derived and \".derived\" below"));
        assert!(out.contains("x = \".statecraft/derived\""));
    }

    #[test]
    fn performing_it_twice_is_not_needed_the_second_time() {
        let dir = repo(&[(".derived/a.json", "{}")]);
        apply(dir.path()).unwrap();
        assert_eq!(plan(dir.path()).unwrap().word(), "not-needed");
    }

    #[test]
    fn nothing_outside_the_given_root_is_touched() {
        let parent = tempfile::tempdir().unwrap();
        let sibling = parent.path().join("sibling");
        std::fs::create_dir_all(sibling.join(".derived")).unwrap();
        std::fs::write(sibling.join(".derived/a.json"), "{}").unwrap();

        let target = parent.path().join("target");
        std::fs::create_dir_all(target.join(".derived")).unwrap();
        std::fs::write(target.join(".derived/b.json"), "{}").unwrap();

        apply(&target).unwrap();
        assert!(sibling.join(".derived/a.json").is_file());
        assert!(!sibling.join(".statecraft").exists());
    }
}
