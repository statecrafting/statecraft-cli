//! Isolated workspaces: a git worktree the operator's checkout never notices.
//!
//! Spec 003 section 3.2. A run prepares an isolated git worktree under
//! `.statecraft/state/`, branched from a base revision **resolved to a commit at
//! run start and recorded**. The operator's checkout is never edited, and no
//! session, check or commit runs in it.
//!
//! Preparation is idempotent by identity: a run has exactly one workspace, and
//! re-preparing an existing one is a no-op that reports the existing path. Two
//! runs never share a workspace, which is also how section 3.7 gets its lock:
//! the workspace IS the lock.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where workspaces live inside a target.
pub const WORKSPACES_DIR: &str = ".statecraft/state/workspaces";

/// A prepared workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    /// The run this belongs to. One run, one workspace.
    pub run_id: String,
    /// Absolute path of the worktree.
    pub path: PathBuf,
    /// The base revision, resolved to a commit at preparation time.
    ///
    /// Recorded rather than re-resolved: a branch name evaluated twice can name
    /// two commits, and section 3.8 makes a base that moved an `interrupted`
    /// outcome, which is only detectable against a recorded commit.
    pub base_commit: String,
    /// The branch created for the worktree.
    pub branch: String,
}

/// Why preparation did not happen.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    /// The path a workspace would occupy holds something else.
    ///
    /// Refused, naming the path. **No deletion, no reuse.** Reusing it would
    /// silently adopt somebody's directory; deleting it would destroy it.
    #[error("{path} exists and is not a workspace this run prepared; refusing, and leaving it")]
    Occupied {
        /// The contested path.
        path: String,
    },
    /// The base revision does not resolve in the target.
    #[error("base revision `{revision}` does not resolve in {target}")]
    UnresolvableBase {
        /// What was asked for.
        revision: String,
        /// The target.
        target: String,
    },
    /// A git invocation failed.
    #[error("git {args} failed in {dir}: {detail}")]
    Git {
        /// The arguments.
        args: String,
        /// Where it ran.
        dir: String,
        /// What git said.
        detail: String,
    },
    /// A filesystem operation failed.
    #[error("i/o at {path}: {source}")]
    Io {
        /// The path.
        path: String,
        /// The underlying error.
        source: std::io::Error,
    },
}

fn git(dir: &Path, args: &[&str]) -> Result<String, WorkspaceError> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| WorkspaceError::Git {
            args: args.join(" "),
            dir: dir.display().to_string(),
            detail: e.to_string(),
        })?;
    if !out.status.success() {
        return Err(WorkspaceError::Git {
            args: args.join(" "),
            dir: dir.display().to_string(),
            detail: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Resolve a revision to a commit in the target.
pub fn resolve_base(target: &Path, revision: &str) -> Result<String, WorkspaceError> {
    git(
        target,
        &["rev-parse", "--verify", &format!("{revision}^{{commit}}")],
    )
    .map_err(|_| WorkspaceError::UnresolvableBase {
        revision: revision.to_string(),
        target: target.display().to_string(),
    })
}

/// The path a run's workspace occupies.
pub fn workspace_path(target: &Path, run_id: &str) -> PathBuf {
    statecraft_environment::claimant::resolve(target, WORKSPACES_DIR).join(run_id)
}

/// Prepare a run's workspace, or report the existing one unchanged.
///
/// Idempotent by identity: called twice for the same run it prepares once and
/// then reports. Called for a different run it produces a different path, so
/// two runs cannot share one.
pub fn prepare(
    target: &Path,
    run_id: &str,
    base_revision: &str,
) -> Result<Workspace, WorkspaceError> {
    let base_commit = resolve_base(target, base_revision)?;
    let path = workspace_path(target, run_id);
    let branch = format!("statecraft/run/{run_id}");

    if path.exists() {
        // Ours, and already prepared? Then this is the no-op the spec requires.
        // Anything else at this path is refused rather than adopted.
        let is_worktree = path.join(".git").exists();
        if !is_worktree {
            return Err(WorkspaceError::Occupied {
                path: path.display().to_string(),
            });
        }
        let head = git(&path, &["rev-parse", "HEAD"])?;
        return Ok(Workspace {
            run_id: run_id.to_string(),
            path,
            // The base recorded is the one this workspace actually sits on, not
            // the one just re-resolved: re-resolving could disagree, and the
            // workspace is the fact.
            base_commit: head,
            branch,
        });
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| WorkspaceError::Io {
            path: parent.display().to_string(),
            source,
        })?;
    }

    git(
        target,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            &branch,
            &path.to_string_lossy(),
            &base_commit,
        ],
    )?;

    Ok(Workspace {
        run_id: run_id.to_string(),
        path,
        base_commit,
        branch,
    })
}

/// Whether the target's base revision still points where the workspace expects.
///
/// Section 3.8: a base revision that moves during an attempt makes the outcome
/// `interrupted`. This is the observation that decides it.
pub fn base_moved(target: &Path, workspace: &Workspace, base_revision: &str) -> bool {
    match resolve_base(target, base_revision) {
        Ok(now) => now != workspace.base_commit,
        // A base that no longer resolves has certainly moved.
        Err(_) => true,
    }
}

/// Remove a run's worktree. Used when a run ends; never during one.
pub fn release(target: &Path, workspace: &Workspace) -> Result<(), WorkspaceError> {
    git(
        target,
        &[
            "worktree",
            "remove",
            "--force",
            &workspace.path.to_string_lossy(),
        ],
    )?;
    Ok(())
}
