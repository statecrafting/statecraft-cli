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
///
/// `target` is the registration's stored root ([`crate::repository`]), so every
/// spelling of one registered path takes the one lock.
pub fn lock_path(home: &Path, target: &Path) -> PathBuf {
    let key = crate::repository::key(target);
    home.join("records").join(format!("{key}.lock"))
}

/// A held lock. Dropping it releases it.
#[derive(Debug)]
pub struct Held {
    file: File,
}

/// Release explicitly, then close.
///
/// A `flock` belongs to the open file description, not to the descriptor, and
/// closing a descriptor releases it only when no other descriptor refers to that
/// description. A child this process spawns from another thread holds a copy of
/// every descriptor, close-on-exec ones included, from its creation until its
/// `exec`; that is so for `fork` and, measured on macOS, for `posix_spawn` too.
/// A lock released by closing alone could therefore outlive its holder for that
/// window, and the next taker would be told another process holds it. An
/// explicit unlock releases it for every copy at once.
impl Drop for Held {
    fn drop(&mut self) {
        let _ = rustix::fs::flock(&self.file, rustix::fs::FlockOperation::Unlock);
    }
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
        Ok(()) => Ok(Held { file }),
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

    /// The copy a spawned child holds between its creation and its `exec` is
    /// a second descriptor on the same open file description. Held here
    /// deterministically, as a duplicate that outlives the holder.
    #[test]
    fn a_copy_of_the_descriptor_does_not_keep_a_released_lock() {
        let home = tempfile::tempdir().unwrap();
        let target = Path::new("/fixture/a");
        let held = try_acquire(home.path(), target).unwrap();
        let copy = held.file.try_clone().unwrap();
        drop(held);
        assert!(try_acquire(home.path(), target).is_ok());
        drop(copy);
    }

    /// The measured failure, as it occurred: processes spawned from other
    /// threads while this one takes and releases the lock.
    #[test]
    fn spawning_on_other_threads_never_makes_a_released_lock_busy() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        let home = tempfile::tempdir().unwrap();
        let target = Path::new("/fixture/a");
        let stop = Arc::new(AtomicBool::new(false));
        let spawners: Vec<_> = (0..4)
            .map(|_| {
                let stop = stop.clone();
                std::thread::spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        let _ = std::process::Command::new("true").output();
                    }
                })
            })
            .collect();
        let mut busy = 0;
        for _ in 0..2000 {
            match try_acquire(home.path(), target) {
                Ok(held) => drop(held),
                Err(LockError::Busy { .. }) => busy += 1,
                Err(e) => panic!("{e}"),
            }
        }
        stop.store(true, Ordering::Relaxed);
        for s in spawners {
            s.join().unwrap();
        }
        assert_eq!(busy, 0, "a released lock was reported held {busy} times");
    }
}
