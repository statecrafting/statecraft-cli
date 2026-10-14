//! The observation chain and its lock (spec 027 section 3.8).
//!
//! One append-only, hash-linked chain per repository in the product home,
//! filed under spec 003's repository key, built and made durable the way spec
//! 003's run record is: the attest-ledger envelope, fsync before
//! acknowledgment, a torn tail reported and appended after, earlier entries
//! never rewritten. Beside it, the observation lock: one writer per chain,
//! distinct from spec 003's repository lock.

use crate::proposal::Entry;
use attest_ledger_core::{LedgerRecord, RecordChain, verify_chain};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Why the chain or its lock could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// Another writer holds the observation lock.
    Busy {
        /// The lock file.
        path: String,
    },
    /// A filesystem operation failed.
    Io {
        /// The file.
        path: String,
        /// What failed.
        detail: String,
    },
    /// A complete entry could not be read back.
    Unreadable {
        /// The chain file.
        path: String,
        /// Which line.
        line: usize,
        /// What went wrong.
        detail: String,
    },
    /// The hash links do not verify.
    Broken {
        /// The chain file.
        path: String,
        /// What attest-ledger said.
        detail: String,
    },
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Busy { path } => write!(
                f,
                "the observation lock {path} is held by another observation or admission"
            ),
            StoreError::Io { path, detail } => {
                write!(f, "observation chain i/o at {path}: {detail}")
            }
            StoreError::Unreadable { path, line, detail } => write!(
                f,
                "observation chain entry at line {line} of {path} is not readable: {detail}"
            ),
            StoreError::Broken { path, detail } => {
                write!(f, "observation chain {path} does not verify: {detail}")
            }
        }
    }
}

impl std::error::Error for StoreError {}

/// The directory every repository's chain and lock are filed in.
pub fn directory(home: &Path) -> PathBuf {
    home.join("observations")
}

/// The chain file for a registered root.
pub fn chain_path(home: &Path, root: &Path) -> PathBuf {
    directory(home).join(format!("{}.jsonl", statecraft_run::repository::key(root)))
}

/// The observation lock file for a registered root.
pub fn lock_path(home: &Path, root: &Path) -> PathBuf {
    directory(home).join(format!("{}.lock", statecraft_run::repository::key(root)))
}

/// The observation lock, held until dropped. Not reentrant: each verb that
/// appends takes it once at entry (section 3.8).
#[derive(Debug)]
pub struct Held {
    _file: File,
}

/// Take the observation lock without waiting: a held lock is a refusal, never
/// a wait.
pub fn lock(home: &Path, root: &Path) -> Result<Held, StoreError> {
    let path = lock_path(home, root);
    let io = |detail: String| StoreError::Io {
        path: path.display().to_string(),
        detail,
    };
    std::fs::create_dir_all(directory(home)).map_err(|e| io(e.to_string()))?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|e| io(e.to_string()))?;
    match rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Ok(Held { _file: file }),
        Err(rustix::io::Errno::WOULDBLOCK) => Err(StoreError::Busy {
            path: path.display().to_string(),
        }),
        Err(e) => Err(io(e.to_string())),
    }
}

/// An incomplete trailing entry: the last append did not finish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TornTail {
    /// Byte offset of its first byte.
    pub offset: u64,
    /// How many bytes it has.
    pub bytes: u64,
}

/// One repository's observation chain.
pub struct Store {
    path: PathBuf,
    anchor: String,
    records: Vec<LedgerRecord>,
    entries: Vec<Entry>,
    torn_tail: Option<TornTail>,
}

impl Store {
    /// Read the chain for a registered root. Creates nothing: a chain nobody
    /// has written is empty, so the read-only verbs leave no file behind.
    ///
    /// Reads to the last complete entry and reports an incomplete tail
    /// without interpreting it. A complete entry that does not decode is an
    /// error, never a gap.
    pub fn open(home: &Path, root: &Path) -> Result<Store, StoreError> {
        let path = chain_path(home, root);
        let shown = path.display().to_string();
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => {
                return Err(StoreError::Io {
                    path: shown,
                    detail: e.to_string(),
                });
            }
        };
        let mut records = Vec::new();
        let mut entries = Vec::new();
        let mut consumed = 0u64;
        let mut torn_tail = None;
        for (i, line) in bytes.split_inclusive(|b| *b == b'\n').enumerate() {
            if !line.ends_with(b"\n") {
                torn_tail = Some(TornTail {
                    offset: consumed,
                    bytes: line.len() as u64,
                });
                break;
            }
            consumed += line.len() as u64;
            let trimmed = &line[..line.len() - 1];
            if trimmed.is_empty() {
                continue;
            }
            let unreadable = |detail: String| StoreError::Unreadable {
                path: shown.clone(),
                line: i + 1,
                detail,
            };
            let record: LedgerRecord =
                serde_json::from_slice(trimmed).map_err(|e| unreadable(e.to_string()))?;
            let entry: Entry = serde_json::from_value(record.payload.clone())
                .map_err(|e| unreadable(e.to_string()))?;
            records.push(record);
            entries.push(entry);
        }
        if !records.is_empty() {
            verify_chain(&records).map_err(|e| StoreError::Broken {
                path: shown.clone(),
                detail: format!("{e:?}"),
            })?;
        }
        Ok(Store {
            path,
            anchor: format!(
                "statecraft-observe:{}",
                statecraft_run::repository::key(root)
            ),
            records,
            entries,
            torn_tail,
        })
    }

    /// Every complete entry, oldest first.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The incomplete tail this chain was read with, if any.
    pub fn torn_tail(&self) -> Option<&TornTail> {
        self.torn_tail.as_ref()
    }

    /// The chain file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one entry under the observation lock, durable before this
    /// returns. A torn tail is truncated away first: it was never
    /// acknowledged, so it was never an entry.
    pub fn append(&mut self, _held: &Held, at: &str, entry: &Entry) -> Result<(), StoreError> {
        let shown = self.path.display().to_string();
        let io = |e: std::io::Error| StoreError::Io {
            path: shown.clone(),
            detail: e.to_string(),
        };
        let payload = serde_json::to_value(entry).map_err(|e| StoreError::Io {
            path: shown.clone(),
            detail: e.to_string(),
        })?;
        let mut chain = RecordChain::new(self.anchor.clone());
        for r in &self.records {
            chain.append(r.id.clone(), r.timestamp.clone(), r.payload.clone());
        }
        let id = format!("observe-{}", self.records.len());
        let record = chain.append(id, at.to_string(), payload);
        let mut line = serde_json::to_vec(&record).map_err(|e| StoreError::Io {
            path: shown.clone(),
            detail: e.to_string(),
        })?;
        line.push(b'\n');
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        if let Some(t) = self.torn_tail.take() {
            let file = OpenOptions::new()
                .write(true)
                .open(&self.path)
                .map_err(io)?;
            file.set_len(t.offset).map_err(io)?;
            file.sync_all().map_err(io)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(io)?;
        file.write_all(&line).map_err(io)?;
        file.sync_all().map_err(io)?;
        self.records.push(record);
        self.entries.push(entry.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proposal::{Disposition, Ledger};

    fn entry(n: u32) -> Entry {
        Entry::Disposed {
            proposal: format!("p{n}"),
            disposition: Disposition::Admitted {
                run_id: "001-a".to_string(),
                attempt: n,
            },
        }
    }

    #[test]
    fn entries_survive_reopening_and_the_lock_is_one_writer() {
        let home = tempfile::tempdir().unwrap();
        let root = Path::new("/fixture/a");
        let empty = Store::open(home.path(), root).unwrap();
        assert!(empty.entries().is_empty());
        assert!(!chain_path(home.path(), root).exists());
        let held = lock(home.path(), root).unwrap();
        assert!(matches!(
            lock(home.path(), root),
            Err(StoreError::Busy { .. })
        ));
        let mut store = Store::open(home.path(), root).unwrap();
        store.append(&held, "t1", &entry(1)).unwrap();
        store.append(&held, "t2", &entry(2)).unwrap();
        drop(held);
        let again = Store::open(home.path(), root).unwrap();
        assert_eq!(again.entries(), [entry(1), entry(2)]);
        assert_eq!(Ledger::fold(again.entries().to_vec()).len(), 2);
        assert!(lock(home.path(), root).is_ok());
    }

    #[test]
    fn a_torn_tail_is_reported_and_appended_after() {
        let home = tempfile::tempdir().unwrap();
        let root = Path::new("/fixture/a");
        let held = lock(home.path(), root).unwrap();
        let mut store = Store::open(home.path(), root).unwrap();
        store.append(&held, "t1", &entry(1)).unwrap();
        let path = chain_path(home.path(), root);
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"{\"partial").unwrap();
        let mut torn = Store::open(home.path(), root).unwrap();
        assert_eq!(torn.entries().len(), 1);
        assert_eq!(torn.torn_tail().unwrap().bytes, 9);
        torn.append(&held, "t2", &entry(2)).unwrap();
        let again = Store::open(home.path(), root).unwrap();
        assert!(again.torn_tail().is_none());
        assert_eq!(again.entries(), [entry(1), entry(2)]);
    }

    #[test]
    fn an_edited_entry_breaks_the_chain() {
        let home = tempfile::tempdir().unwrap();
        let root = Path::new("/fixture/a");
        let held = lock(home.path(), root).unwrap();
        let mut store = Store::open(home.path(), root).unwrap();
        store.append(&held, "t1", &entry(1)).unwrap();
        store.append(&held, "t2", &entry(2)).unwrap();
        let path = chain_path(home.path(), root);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, text.replacen("\"attempt\":1", "\"attempt\":7", 1)).unwrap();
        assert!(matches!(
            Store::open(home.path(), root),
            Err(StoreError::Broken { .. })
        ));
    }
}
