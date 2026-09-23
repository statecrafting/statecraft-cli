//! The run record: append-only, hash-linked, durable before acknowledge.
//!
//! Spec 003 section 3.3. One chain per registered repository, over the
//! `attest-ledger` record envelope. Two properties this product owns rather than
//! inherits: **fsync before acknowledge**, and an **in-memory head** so an
//! append stays O(1).
//!
//! Every effect is bracketed: intent written and durable before the effect,
//! outcome written after it. A crash between the two is therefore detectable,
//! which is the entire basis of section 3.6's recovery.
//!
//! # Where the chain lives, and why it is not in the target
//!
//! In the **product home**, keyed by repository, not inside the target. Section
//! 3.5 requires the refusal count to be written somewhere the supervised process
//! cannot reach, and the supervised process runs inside a worktree under the
//! target's `.statecraft/state/`. A chain in the target would be a chain the
//! thing being judged can edit.

use attest_ledger_core::{LedgerRecord, RecordChain, verify_chain};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};

/// What a record is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Written and made durable BEFORE the effect it describes.
    Intent,
    /// Written after the effect.
    Outcome,
    /// A reconciliation verdict for an intent that had no outcome.
    Reconciliation,
    /// Supervisor-owned accounting the supervised process cannot reach.
    Accounting,
}

/// A validated effect identity: a non-empty string.
///
/// Spec 003 section 3.3.1 clause 1. Uniqueness is **per run** and is the fold's
/// business, not this type's: `EffectId` answers only whether a value is a
/// well-formed identity at all.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EffectId(String);

impl EffectId {
    /// The identity, or `None` when the string is empty.
    #[must_use]
    pub fn new(s: &str) -> Option<Self> {
        if s.is_empty() {
            None
        } else {
            Some(Self(s.to_string()))
        }
    }

    /// The identity as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EffectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The identity field's three states (section 3.3.1 clause 3).
///
/// Presence is decided by the **key**, never by the value: `Absent` is
/// reachable only from a missing key, which is why this is a three-state value
/// of its own rather than an `Option`. A self-describing format deserializes an
/// explicit `null` to the same `None` as a missing key, and clause 3 forbids
/// reading the two as one state.
///
/// An invalid value is retained rather than refused. The fold reads payloads
/// through a decoder that discards what it cannot decode
/// ([`Chain::entries`]), so a stricter codec would lose the very defect
/// clause 3 requires to be reported.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Identity {
    /// The record carries no `effectId` key. The only state without an
    /// identity, and the one folded by the legacy pairing of clause 7.
    #[default]
    Absent,
    /// The key is present and its value is a non-empty string.
    Valid(EffectId),
    /// The key is present with any other value, JSON `null` included.
    ///
    /// A defect the fold reports, never read as [`Identity::Absent`], never a
    /// decode failure, and never a reason to drop the record.
    Invalid(Value),
}

impl Identity {
    /// Whether the key was missing.
    ///
    /// The `skip_serializing_if` predicate: it is what keeps a record without
    /// an identity byte-identical to what this product writes today.
    #[must_use]
    pub fn is_absent(&self) -> bool {
        matches!(self, Identity::Absent)
    }

    /// The identity, when there is a valid one.
    #[must_use]
    pub fn valid(&self) -> Option<&EffectId> {
        match self {
            Identity::Valid(id) => Some(id),
            _ => None,
        }
    }

    /// Classify a **present** JSON value. A missing key never reaches here.
    fn from_present(value: Value) -> Self {
        match &value {
            Value::String(s) if !s.is_empty() => Identity::Valid(EffectId(s.clone())),
            _ => Identity::Invalid(value),
        }
    }
}

impl Serialize for Identity {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            // Unreachable through `Entry`, which skips an absent identity. A
            // direct `serialize` of one writes null rather than panicking.
            Identity::Absent => serializer.serialize_none(),
            Identity::Valid(id) => serializer.serialize_str(id.as_str()),
            Identity::Invalid(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Identity {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Accepts ANY present value, so `null` reaches `Invalid(Value::Null)`.
        // `Absent` comes from serde's `default`, which runs only when the key
        // is missing.
        Ok(Identity::from_present(Value::deserialize(deserializer)?))
    }
}

/// The payload this product puts in an attest-ledger envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Intent, outcome, reconciliation or accounting.
    pub kind: Kind,
    /// The run this belongs to.
    pub run_id: String,
    /// The attempt within that run. A retry appends a new number.
    pub attempt: u32,
    /// What the record is about, for example `prepare-workspace` or `turn`.
    pub subject: String,
    /// An intent's idempotency key, where the effect has one.
    ///
    /// Section 3.6: where an effect is idempotent by a key, the key is recorded
    /// in the intent and named in the record. Where it is not, this is `None`
    /// and the record says so, which is how "no exactly-once promise" is made
    /// visible rather than merely true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    /// Anything else the caller wants recorded.
    #[serde(default)]
    pub detail: Value,
    /// This effect's own identity, where it has one (section 3.3.1).
    ///
    /// `default` is what makes a missing key [`Identity::Absent`];
    /// `skip_serializing_if` is what keeps a record without an identity
    /// byte-identical to what this product wrote before the field existed.
    /// The wire spelling is `effectId`, stated here rather than inherited from
    /// the Rust field name, because this record is read through the product's
    /// JSON contracts (section 5, 2026-09-19).
    #[serde(
        default,
        rename = "effectId",
        skip_serializing_if = "Identity::is_absent"
    )]
    pub effect_id: Identity,
}

/// Why a record operation failed.
#[derive(Debug, thiserror::Error)]
pub enum RecordError {
    /// A filesystem operation failed.
    #[error("record i/o at {path}: {source}")]
    Io {
        /// The chain file.
        path: String,
        /// The underlying error.
        source: std::io::Error,
    },
    /// A stored record could not be read back.
    #[error("record at line {line} of {path} is not readable: {detail}")]
    Unreadable {
        /// The chain file.
        path: String,
        /// Which line.
        line: usize,
        /// What went wrong.
        detail: String,
    },
    /// This repository has a chain filed under another spelling of its path.
    #[error("chain {path} {detail}")]
    FiledElsewhere {
        /// The chain file this spelling keys.
        path: String,
        /// Which files, under which spellings.
        detail: String,
    },
    /// The chain's hash links do not verify.
    #[error("chain {path} does not verify: {detail}")]
    BrokenChain {
        /// The chain file.
        path: String,
        /// What attest-ledger said.
        detail: String,
    },
}

/// What was found when the chain was opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenReport {
    /// How many complete records were read.
    pub records: usize,
    /// Present when the file ended mid-record.
    ///
    /// Section 3.8: recovery reads to the last complete record, **reports** the
    /// torn tail, and appends after it. Earlier records are never rewritten.
    pub torn_tail: Option<TornTail>,
}

/// A trailing partial record left by a crash mid-append.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TornTail {
    /// Byte offset of the first byte of the incomplete record.
    pub offset: u64,
    /// How many bytes were incomplete.
    pub bytes: u64,
}

/// An append-only hash-linked chain for one repository.
///
/// No `Debug`: `RecordChain` does not implement it, and deriving it here by
/// skipping that field would print a chain whose head is invisible, which is
/// the one thing worth seeing.
pub struct Chain {
    path: PathBuf,
    chain: RecordChain,
    records: Vec<LedgerRecord>,
    torn_tail: Option<TornTail>,
}

/// The chain file for a repository inside a product home.
///
/// Keyed by a digest of the absolute path, because a repository's path is not a
/// filename and two targets can share a basename. `target` is the
/// registration's stored root ([`crate::repository`]), so every spelling of one
/// registered path reads and appends the one chain.
pub fn chain_path(home: &Path, target: &Path) -> PathBuf {
    let key = crate::repository::key(target);
    home.join("records").join(format!("{key}.jsonl"))
}

impl Chain {
    /// Open, or create, the chain for a target.
    ///
    /// Reads every complete record to rebuild the in-memory head. A trailing
    /// incomplete record is reported rather than repaired silently, and the next
    /// append lands after the last complete one.
    pub fn open(home: &Path, target: &Path) -> Result<(Self, OpenReport), RecordError> {
        let path = chain_path(home, target);
        // A chain filed under another spelling of this path is this
        // repository's history. Reading this one as empty beside it would say
        // "no history", and reading both would merge two chains; neither.
        if let Some(records) = path.parent() {
            let found = crate::repository::elsewhere(records, target, ".jsonl");
            if !found.is_empty() {
                return Err(RecordError::FiledElsewhere {
                    path: path.display().to_string(),
                    detail: crate::repository::elsewhere_detail(&found),
                });
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| RecordError::Io {
                path: path.display().to_string(),
                source,
            })?;
        }

        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(source) => {
                return Err(RecordError::Io {
                    path: path.display().to_string(),
                    source,
                });
            }
        };

        let mut records = Vec::new();
        let mut consumed: u64 = 0;
        let mut torn_tail = None;

        for (i, line) in bytes.split_inclusive(|b| *b == b'\n').enumerate() {
            let complete = line.ends_with(b"\n");
            let trimmed = line.strip_suffix(b"\n").unwrap_or(line);
            if !complete {
                // The file ended without a newline: the last append did not
                // finish. Whether it parses is irrelevant; it was never
                // acknowledged, so it is not part of the chain.
                torn_tail = Some(TornTail {
                    offset: consumed,
                    bytes: line.len() as u64,
                });
                break;
            }
            if trimmed.is_empty() {
                consumed += line.len() as u64;
                continue;
            }
            match serde_json::from_slice::<LedgerRecord>(trimmed) {
                Ok(r) => records.push(r),
                Err(e) => {
                    return Err(RecordError::Unreadable {
                        path: path.display().to_string(),
                        line: i + 1,
                        detail: e.to_string(),
                    });
                }
            }
            consumed += line.len() as u64;
        }

        // An empty chain is a chain nobody has appended to yet, which is the
        // ordinary state of a repository on its first run. attest-ledger's
        // `verify_chain` reports `EmptyChain` for it, correctly for a ledger
        // that is supposed to have an anchor; here it would turn "new" into
        // "broken", so the verification starts once there is something to link.
        if !records.is_empty() {
            verify_chain(&records).map_err(|e| RecordError::BrokenChain {
                path: path.display().to_string(),
                detail: format!("{e:?}"),
            })?;
        }

        // The in-memory head: rebuilt once here, then carried, so an append is
        // O(1) rather than a re-read of the whole chain.
        let anchor = format!("statecraft:{}", crate::repository::key(target));
        let mut chain = RecordChain::new(anchor);
        for r in &records {
            chain.append(r.id.clone(), r.timestamp.clone(), r.payload.clone());
        }

        let report = OpenReport {
            records: records.len(),
            torn_tail: torn_tail.clone(),
        };
        Ok((
            Self {
                path,
                chain,
                records,
                torn_tail,
            },
            report,
        ))
    }

    /// The torn tail this chain opened with, if any.
    pub fn torn_tail(&self) -> Option<&TornTail> {
        self.torn_tail.as_ref()
    }

    /// Every complete record, oldest first.
    pub fn records(&self) -> &[LedgerRecord] {
        &self.records
    }

    /// Append a record and make it durable before returning.
    ///
    /// **fsync before acknowledge.** A caller that gets `Ok` may rely on the
    /// record surviving a crash, which is the only thing that makes "intent
    /// durable before the effect" true rather than hopeful.
    pub fn append(&mut self, id: &str, timestamp: &str, entry: &Entry) -> Result<(), RecordError> {
        let payload = serde_json::to_value(entry).map_err(|e| RecordError::Unreadable {
            path: self.path.display().to_string(),
            line: 0,
            detail: e.to_string(),
        })?;
        let record = self
            .chain
            .append(id.to_string(), timestamp.to_string(), payload);

        let mut line = serde_json::to_vec(&record).map_err(|e| RecordError::Unreadable {
            path: self.path.display().to_string(),
            line: 0,
            detail: e.to_string(),
        })?;
        line.push(b'\n');

        // A torn tail is truncated away before the first append after it. That
        // is not rewriting an earlier record: the torn bytes were never a
        // record, because they were never acknowledged.
        if let Some(t) = self.torn_tail.take() {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .open(&self.path)
                .map_err(|source| RecordError::Io {
                    path: self.path.display().to_string(),
                    source,
                })?;
            file.set_len(t.offset).map_err(|source| RecordError::Io {
                path: self.path.display().to_string(),
                source,
            })?;
            file.sync_all().map_err(|source| RecordError::Io {
                path: self.path.display().to_string(),
                source,
            })?;
        }

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|source| RecordError::Io {
                path: self.path.display().to_string(),
                source,
            })?;
        file.write_all(&line).map_err(|source| RecordError::Io {
            path: self.path.display().to_string(),
            source,
        })?;
        file.sync_all().map_err(|source| RecordError::Io {
            path: self.path.display().to_string(),
            source,
        })?;

        self.records.push(record);
        Ok(())
    }

    /// Every record's payload, decoded.
    ///
    /// State is recovered by **folding the record**. Memory is never the
    /// authority, and a recovered state is never reconstructed from the
    /// filesystem alone.
    pub fn entries(&self) -> Vec<Entry> {
        self.records
            .iter()
            .filter_map(|r| serde_json::from_value(r.payload.clone()).ok())
            .collect()
    }

    /// Every record's payload, decoded, with its position in the chain.
    ///
    /// The position is the record's index among [`Chain::records`], so it
    /// stays the record's true chain position where an earlier payload does
    /// not decode and [`Chain::entries`] skips it.
    pub fn positioned_entries(&self) -> Vec<(usize, Entry)> {
        self.records
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                serde_json::from_value(r.payload.clone())
                    .ok()
                    .map(|e| (i, e))
            })
            .collect()
    }
}
