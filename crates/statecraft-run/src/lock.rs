//! The repository lock (spec 003 section 3.1.4 rule 7).
//!
//! One lock file per repository in the product home, beside the run record.
//! `run` holds it exclusively from before it appends an intent until after its
//! outcome is durable; every other write this product makes to the run record
//! or the override journal takes it too, and refuses while another process
//! holds it. The operating system releases it when the holding process ends,
//! however it ends. Nothing here infers anything about an effect from that:
//! the lock says only whether a process is still holding it.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

/// The lock file for a repository inside a product home.
pub fn lock_path(home: &Path, target: &Path) -> PathBuf {
    let key = statecraft_environment::digest::digest_bytes(target.to_string_lossy().as_bytes());
    home.join("records").join(format!("{key}.lock"))
}

/// A held lock. Dropping it releases it.
#[derive(Debug)]
pub struct Held {
    _file: File,
}

/// Why the lock was not taken.
#[derive(Debug, thiserror::Error)]
pub enum LockError {
    /// Another process holds it: a `run` is supervising an attempt.
    #[error(
        "another process holds this repository's lock ({path}): a run is still supervising an \
         attempt here, so nothing was written"
    )]
    Busy {
        /// The lock file.
        path: String,
    },
    /// The lock file could not be opened or locked.
    #[error("the repository lock {path} could not be taken: {detail}")]
    Failed {
        /// The lock file.
        path: String,
        /// What went wrong.
        detail: String,
    },
}

/// Take the lock without waiting.
pub fn try_acquire(home: &Path, target: &Path) -> Result<Held, LockError> {
    let path = lock_path(home, target);
    let failed = |detail: String| LockError::Failed {
        path: path.display().to_string(),
        detail,
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| failed(e.to_string()))?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|e| failed(e.to_string()))?;
    match rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Ok(Held { _file: file }),
        Err(rustix::io::Errno::WOULDBLOCK) => Err(LockError::Busy {
            path: path.display().to_string(),
        }),
        Err(e) => Err(failed(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_holder_is_refused_until_the_first_drops() {
        let home = tempfile::tempdir().unwrap();
        let target = Path::new("/fixture/a");
        let first = try_acquire(home.path(), target).unwrap();
        assert!(matches!(
            try_acquire(home.path(), target),
            Err(LockError::Busy { .. })
        ));
        // Another repository's lock is independent.
        assert!(try_acquire(home.path(), Path::new("/fixture/b")).is_ok());
        drop(first);
        assert!(try_acquire(home.path(), target).is_ok());
    }
}
