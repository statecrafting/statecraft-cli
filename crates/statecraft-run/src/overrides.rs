//! The override journal: how an operator's single-spec override is made,
//! kept, removed, read and recovered.
//!
//! Spec 003 sections 3.1.4 and 3.1.5. Section 3.1.1 point 2 fixed that this
//! product's own state holds only an explicit, recorded override for a single
//! named spec id per repository; this module is where that state lives and the
//! only code that writes it.
//!
//! # The journal
//!
//! One append-only file per registered repository, beside that repository's
//! run record in the product home, never in the target. Each line is one grant,
//! one revocation or one recovery, and carries the SHA-256 of the line before
//! it, so a line removed, reordered or edited before the last one reads as a
//! broken journal rather than as a different set of overrides (3.1.4 rule 3).
//! The overrides in force are the fold: granted, not later revoked, and not
//! voided by a recovery.
//!
//! # The state authority
//!
//! Beside each journal is one state authority (3.1.5 rule 2): the number of
//! complete lines, the digest of the last one and an optional intended line.
//! It is replaced, never appended to. The journal agrees with it when it has
//! exactly that many complete lines and the last digests to the recorded
//! digest; every other shape is either one of rule 4's interruptions of this
//! module's own write, or rule 3's disagreement, which is a failure until the
//! operator recovers it (rule 5). A consistent rewrite of both files is not
//! detected: the claim rests on the supervised process being unable to reach
//! either (spec 004 section 3.18).
//!
//! # What this module does not decide
//!
//! Whether a spec id exists, and whether a repository is registered, are read
//! by the caller from the report and the registry this crate does not own, and
//! passed in as facts. The operator's name is **supplied, not authenticated**
//! (3.1.4 rule 2): nothing here can check it, and the line says so.

use crate::policy::{Override, Overrides};
use serde::{Deserialize, Serialize};
use statecraft_environment::digest::digest_bytes;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The word every journal line carries beside the operator's name.
pub const OPERATOR_PROVENANCE: &str = "operator-supplied";

/// How a missing file's digest is written, in a report and in a recovery.
pub const ABSENT: &str = "absent";

const JOURNAL_SUFFIX: &str = ".overrides.jsonl";
const AUTHORITY_SUFFIX: &str = ".overrides.authority.json";

/// The journal file for a repository inside a product home.
///
/// Keyed like the run record, so the two sit side by side and one repository's
/// journal is never read for another (3.1.4 rule 4), and by the registration's
/// stored root ([`crate::repository`]), so one repository has one journal
/// however its path was typed.
pub fn journal_path(home: &Path, target: &Path) -> PathBuf {
    let key = crate::repository::key(target);
    home.join("records").join(format!("{key}{JOURNAL_SUFFIX}"))
}

/// The state authority beside that journal (3.1.5 rule 2).
pub fn authority_path(home: &Path, target: &Path) -> PathBuf {
    let key = crate::repository::key(target);
    home.join("records")
        .join(format!("{key}{AUTHORITY_SUFFIX}"))
}

/// Grant, revoke or recover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    /// An override comes into force.
    Grant,
    /// It stops being in force.
    Revoke,
    /// An operator brought the journal and its authority back into agreement
    /// (3.1.5 rule 5).
    Recover,
}

/// One line of the journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Line {
    /// Grant, revoke or recover.
    pub action: Action,
    /// The repository, as registered.
    pub repository: String,
    /// The one spec id, for a grant or a revocation; none for a recovery.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_id: Option<String>,
    /// Who asked, as they said.
    pub operator: String,
    /// Always [`OPERATOR_PROVENANCE`]: the name is not authenticated.
    pub operator_provenance: String,
    /// Why.
    pub reason: String,
    /// When this product wrote the line, RFC 3339 UTC.
    pub at: String,
    /// The SHA-256 of the previous line's bytes, or `None` for the first.
    pub previous: Option<String>,
    /// What this write cleared of an interrupted earlier one (3.1.5 rule 4:
    /// "which records that it did").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleared: Option<Cleared>,
    /// For a recovery line, what was found and chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery: Option<Recovery>,
}

/// An interrupted write this line's write cleared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Cleared {
    /// The intended line the authority recorded and the journal did not hold
    /// complete.
    pub intended: Intended,
    /// The byte length of the torn final line cut off, if there was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub torn_bytes: Option<u64>,
}

/// A line the authority records as about to be appended (3.1.5 rule 4 step 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Intended {
    /// Its 1-based position in the journal.
    pub position: u64,
    /// The SHA-256 of its bytes.
    pub digest: String,
}

/// The five recovery choices (3.1.5 rule 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Choice {
    /// A pending line comes into force.
    CompletePending,
    /// A pending line is voided; a torn line or an intent with no line is
    /// cleared.
    DiscardPending,
    /// A journal that reads and verifies becomes the baseline as it reads.
    AdoptAsRead,
    /// The longest verifying prefix of a journal that does not becomes the
    /// baseline.
    AdoptPrefix,
    /// No override is in force from here, where the journal is missing.
    AdoptEmpty,
}

impl Choice {
    /// Every choice, in the order rule 5 lists them.
    pub const ALL: [Choice; 5] = [
        Choice::CompletePending,
        Choice::DiscardPending,
        Choice::AdoptAsRead,
        Choice::AdoptPrefix,
        Choice::AdoptEmpty,
    ];

    /// The word an operator types.
    pub fn word(self) -> &'static str {
        match self {
            Choice::CompletePending => "complete-pending",
            Choice::DiscardPending => "discard-pending",
            Choice::AdoptAsRead => "adopt-as-read",
            Choice::AdoptPrefix => "adopt-prefix",
            Choice::AdoptEmpty => "adopt-empty",
        }
    }

    /// The choice an operator's word names, if any.
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.word() == word)
    }

    /// Whether this choice makes an operator-adopted baseline.
    fn adopts(self) -> bool {
        matches!(
            self,
            Choice::AdoptAsRead | Choice::AdoptPrefix | Choice::AdoptEmpty
        )
    }
}

/// What a recovery line records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Recovery {
    /// The choice.
    pub choice: Choice,
    /// The state as found: the counts and digests of both files.
    pub found: FoundDigests,
    /// The digest of the pending line `discard-pending` voided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voids: Option<String>,
    /// The whole journal as `adopt-prefix` found it, preserved beside it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preserved: Option<Preserved>,
}

/// The counts and digests of both files as a recovery found them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FoundDigests {
    /// The state's word ([`State::word`]).
    pub state: String,
    /// The journal's SHA-256, or [`ABSENT`].
    pub journal: String,
    /// Its complete lines.
    pub journal_lines: u64,
    /// The authority's SHA-256, or [`ABSENT`].
    pub authority: String,
    /// The count it records, where it parsed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority_lines: Option<u64>,
}

/// A file preserved by `adopt-prefix`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preserved {
    /// Where it was put.
    pub path: String,
    /// The SHA-256 of its bytes.
    pub sha256: String,
}

/// The state authority's document (3.1.5 rule 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Authority {
    /// The journal's key.
    pub journal: String,
    /// Its complete lines.
    pub lines: u64,
    /// The digest of the last one, or `None` for none.
    pub last: Option<String>,
    /// A line about to be appended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intended: Option<Intended>,
    /// When this product wrote the authority, RFC 3339 UTC.
    pub at: String,
}

/// Whether a grant's line verifies, or precedes an operator's adoption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verification {
    /// The grant's line verifies against the authority.
    Verified,
    /// The grant precedes an `adopt-` recovery: operator-adopted, not verified.
    OperatorAdopted,
}

impl Verification {
    /// The word a rendering shows.
    pub fn word(self) -> &'static str {
        match self {
            Verification::Verified => "verified",
            Verification::OperatorAdopted => "operator-adopted, not verified",
        }
    }
}

/// An override in force, with the facts 3.1.4 rule 5 and 3.1.5 rule 7 record
/// on an attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InForce {
    /// The spec id.
    pub spec_id: String,
    /// The operator, as supplied.
    pub operator: String,
    /// Always [`OPERATOR_PROVENANCE`].
    pub operator_provenance: String,
    /// Why.
    pub reason: String,
    /// When it was granted.
    pub granted_at: String,
    /// The SHA-256 of the grant's journal line.
    pub grant_line: String,
    /// Verified, or operator-adopted.
    pub verification: Verification,
}

/// The state a journal and its authority are in (3.1.5 rules 2 to 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Neither file, or an authority of zero lines and no journal: no
    /// override was ever granted.
    Empty,
    /// The two agree and no line is intended.
    Agree,
    /// An intended line, and a journal without it.
    IntentWithoutLine,
    /// An intended line, and a torn final line.
    IntentTorn,
    /// An intended line, and a complete final line that digests to it.
    Pending,
    /// The journal reads and verifies, and disagrees with its authority, or the
    /// authority is missing or does not parse.
    Disagreement(String),
    /// The journal does not read or verify.
    JournalInvalid(String),
    /// An authority whose journal is missing.
    JournalMissing,
}

impl State {
    /// The word a report and a recovery line carry.
    pub fn word(&self) -> &'static str {
        match self {
            State::Empty => "none",
            State::Agree => "agree",
            State::IntentWithoutLine => "intended-without-line",
            State::IntentTorn => "intended-torn",
            State::Pending => "pending",
            State::Disagreement(_) => "disagreement",
            State::JournalInvalid(_) => "journal-invalid",
            State::JournalMissing => "journal-missing",
        }
    }

    /// One sentence an operator reads.
    pub fn describe(&self) -> String {
        match self {
            State::Empty => "no override journal: no override was ever granted".into(),
            State::Agree => "the journal and its state authority agree".into(),
            State::IntentWithoutLine => "the authority records an intended line the journal \
                does not hold: a write stopped before its line was appended; it was never in \
                force, and the next write clears it"
                .into(),
            State::IntentTorn => "the authority records an intended line and the journal ends in \
                a torn line: a write stopped while appending; it is not in force, and the next \
                write cuts it off"
                .into(),
            State::Pending => "a pending line: this product appended it and never acknowledged \
                the write; it is not in force until the operator settles it with `override \
                recover`"
                .into(),
            State::Disagreement(d) => format!("the journal and its state authority disagree: {d}"),
            State::JournalInvalid(d) => format!("the journal does not verify: {d}"),
            State::JournalMissing => {
                "the state authority records lines and the journal is missing".into()
            }
        }
    }

    /// The recovery choices this state allows (3.1.5 rule 5's table).
    pub fn allowed(&self) -> Vec<Choice> {
        match self {
            State::Empty | State::Agree => Vec::new(),
            State::IntentWithoutLine | State::IntentTorn => vec![Choice::DiscardPending],
            State::Pending => vec![Choice::CompletePending, Choice::DiscardPending],
            State::Disagreement(_) => vec![Choice::AdoptAsRead],
            State::JournalInvalid(_) => vec![Choice::AdoptPrefix],
            State::JournalMissing => vec![Choice::AdoptEmpty],
        }
    }

    /// Whether this is one of rule 4's interruptions of this product's write.
    fn interrupted(&self) -> bool {
        matches!(
            self,
            State::IntentWithoutLine | State::IntentTorn | State::Pending
        )
    }
}

/// One file as found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileFound {
    /// Where it is.
    pub path: String,
    /// Its SHA-256, or [`ABSENT`].
    pub sha256: String,
}

/// Both files as found, and the state they are in.
#[derive(Debug, Clone)]
pub struct Found {
    /// The journal.
    pub journal: FileFound,
    /// The state authority.
    pub authority: FileFound,
    /// The state.
    pub state: State,
    scan: Option<Scan>,
    parsed: Option<Authority>,
}

impl Found {
    /// The journal's complete lines, verifying or not.
    pub fn journal_lines(&self) -> u64 {
        self.scan.as_ref().map_or(0, |s| s.complete_lines)
    }

    /// The count the authority records, where it parsed.
    pub fn authority_lines(&self) -> Option<u64> {
        self.parsed.as_ref().map(|a| a.lines)
    }

    /// The intended line the authority records.
    pub fn intended(&self) -> Option<&Intended> {
        self.parsed.as_ref().and_then(|a| a.intended.as_ref())
    }

    /// The torn final line's byte length.
    pub fn torn_bytes(&self) -> Option<u64> {
        self.scan
            .as_ref()
            .and_then(|s| s.torn.as_ref())
            .map(|t| t.len() as u64)
    }

    fn digests(&self) -> FoundDigests {
        FoundDigests {
            state: self.state.word().to_string(),
            journal: self.journal.sha256.clone(),
            journal_lines: self.journal_lines(),
            authority: self.authority.sha256.clone(),
            authority_lines: self.authority_lines(),
        }
    }

    /// The lines the fold reads: those the authority covers.
    fn covered(&self) -> &[(Line, String)] {
        let Some(scan) = &self.scan else {
            return &[];
        };
        let n = self
            .parsed
            .as_ref()
            .map_or(scan.lines.len(), |a| a.lines as usize);
        &scan.lines[..n.min(scan.lines.len())]
    }
}

/// A journal's bytes, split and verified as far as they verify.
#[derive(Debug, Clone)]
struct Scan {
    /// The whole file.
    bytes: Vec<u8>,
    /// The lines that parsed and linked, in order, with their digests.
    lines: Vec<(Line, String)>,
    /// The byte offset after each verified line.
    ends: Vec<u64>,
    /// Every complete line, verifying or not.
    complete_lines: u64,
    /// The byte length of the complete lines.
    complete_len: u64,
    /// A final segment with no line break.
    torn: Option<String>,
    /// Why the journal does not verify, at its first failing line.
    invalid: Option<String>,
}

fn scan(bytes: Vec<u8>, repository: &str) -> Scan {
    let complete_len = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |i| i + 1);
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let (complete, torn) = match text.rfind('\n') {
        Some(i) if i + 1 < text.len() => (&text[..=i], Some(text[i + 1..].to_string())),
        Some(_) => (text.as_str(), None),
        None if text.is_empty() => ("", None),
        None => ("", Some(text.clone())),
    };
    let mut out = Scan {
        bytes: Vec::new(),
        lines: Vec::new(),
        ends: Vec::new(),
        complete_lines: complete.lines().count() as u64,
        complete_len: complete_len as u64,
        torn,
        invalid: std::str::from_utf8(&bytes)
            .err()
            .map(|e| format!("is not UTF-8: {e}")),
    };
    let mut offset = 0u64;
    if out.invalid.is_none() {
        for (i, raw) in complete.split_inclusive('\n').enumerate() {
            let n = i + 1;
            let body = raw.strip_suffix('\n').unwrap_or(raw);
            let line: Line = match serde_json::from_str(body) {
                Ok(l) => l,
                Err(e) => {
                    out.invalid = Some(format!("line {n} is not a journal line: {e}"));
                    break;
                }
            };
            let last = out.lines.last().map(|(_, d): &(Line, String)| d.clone());
            if line.previous != last {
                out.invalid = Some(format!(
                    "line {n} does not follow line {i}: the journal was edited or reordered"
                ));
                break;
            }
            if line.repository != repository {
                out.invalid = Some(format!(
                    "line {n} names repository {}, and this journal is {repository}'s",
                    line.repository
                ));
                break;
            }
            offset += raw.len() as u64;
            out.lines.push((line, digest_bytes(body.as_bytes())));
            out.ends.push(offset);
        }
    }
    out.bytes = bytes;
    out
}

/// Why the journal could not be used.
#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    /// A precondition failed and nothing was written.
    #[error("{0}")]
    Refused(String),
    /// A pending line (3.1.5 rule 4): this product began a write and never
    /// acknowledged it, and only the operator can settle it.
    #[error(
        "the override journal {path} ends in a pending line this product appended and never \
         acknowledged; nothing is read as in force and nothing was written; settle it with \
         `override recover`"
    )]
    Pending {
        /// The journal.
        path: String,
    },
    /// Another process holds the repository lock and the files are in one of
    /// rule 4's interrupted states: a write in progress.
    #[error(
        "a write to the override journal {path} is in progress in another process; nothing was \
         read as in force and nothing was written"
    )]
    InProgress {
        /// The journal.
        path: String,
    },
    /// The journal did not read or verify, disagrees with its authority, or a
    /// write was not durable.
    ///
    /// Never read as an empty journal: an unreadable one that read as "no
    /// override" would be indistinguishable from one nobody wrote (3.1.4 rule 3).
    #[error("the override journal {path} {detail}")]
    Failed {
        /// The file.
        path: String,
        /// What went wrong.
        detail: String,
    },
}

impl JournalError {
    /// Whether this is a refusal (exit 2) rather than a failure (exit 4).
    pub fn is_refusal(&self) -> bool {
        !matches!(self, JournalError::Failed { .. })
    }
}

/// A journal read and verified.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Journal {
    lines: Vec<(Line, String)>,
    /// A final line with no line break: a write that did not complete. Not in
    /// force, reported, and truncated before the next append.
    torn: Option<String>,
    /// An intended line the journal does not hold complete.
    intended: Option<Intended>,
    /// The authority's SHA-256 as read.
    authority: Option<String>,
}

impl Journal {
    fn of(found: &Found) -> Self {
        Journal {
            lines: found.covered().to_vec(),
            torn: found.scan.as_ref().and_then(|s| s.torn.clone()),
            intended: found.intended().cloned(),
            authority: (found.authority.sha256 != ABSENT).then(|| found.authority.sha256.clone()),
        }
    }

    /// The overrides in force: granted, not later revoked and not voided, in
    /// grant order.
    pub fn in_force(&self) -> Vec<InForce> {
        let voided: std::collections::HashSet<&str> = self
            .lines
            .iter()
            .filter_map(|(l, _)| l.recovery.as_ref()?.voids.as_deref())
            .collect();
        let adopted_at = self.lines.iter().rposition(|(l, d)| {
            !voided.contains(d.as_str()) && l.recovery.as_ref().is_some_and(|r| r.choice.adopts())
        });
        let mut out: Vec<InForce> = Vec::new();
        for (i, (line, digest)) in self.lines.iter().enumerate() {
            if voided.contains(digest.as_str()) {
                continue;
            }
            let Some(spec_id) = &line.spec_id else {
                continue;
            };
            match line.action {
                Action::Grant => out.push(InForce {
                    spec_id: spec_id.clone(),
                    operator: line.operator.clone(),
                    operator_provenance: line.operator_provenance.clone(),
                    reason: line.reason.clone(),
                    granted_at: line.at.clone(),
                    grant_line: digest.clone(),
                    verification: if adopted_at.is_some_and(|k| i < k) {
                        Verification::OperatorAdopted
                    } else {
                        Verification::Verified
                    },
                }),
                Action::Revoke => out.retain(|o| &o.spec_id != spec_id),
                Action::Recover => {}
            }
        }
        out
    }

    /// The same, in the shape work selection reads, with the authority's
    /// digest each admission records (3.1.5 rule 7).
    pub fn overrides(&self) -> Overrides {
        Overrides {
            entries: self
                .in_force()
                .into_iter()
                .map(|o| Override {
                    spec_id: o.spec_id,
                    operator: o.operator,
                    reason: o.reason,
                    operator_provenance: Some(o.operator_provenance),
                    granted_at: Some(o.granted_at),
                    grant_line: Some(o.grant_line),
                    authority: self.authority.clone(),
                    verification: Some(o.verification.word().to_string()),
                })
                .collect(),
        }
    }

    /// The torn last line, if the journal ends in one.
    pub fn torn(&self) -> Option<&str> {
        self.torn.as_deref()
    }

    /// An intended line the journal does not hold complete: an interrupted
    /// write, never in force.
    pub fn intended(&self) -> Option<&Intended> {
        self.intended.as_ref()
    }

    /// The authority's SHA-256 as read, or `None` where there is none.
    pub fn authority(&self) -> Option<&str> {
        self.authority.as_deref()
    }
}

fn failed(path: &Path, detail: String) -> JournalError {
    JournalError::Failed {
        path: path.display().to_string(),
        detail,
    }
}

fn read_bytes(path: &Path) -> Result<Option<Vec<u8>>, JournalError> {
    match std::fs::read(path) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(failed(path, format!("could not be read: {e}"))),
    }
}

/// Read both files and name their state. Writes nothing and takes no lock.
pub fn inspect(home: &Path, target: &Path) -> Result<Found, JournalError> {
    let path = journal_path(home, target);
    let apath = authority_path(home, target);
    // Records filed under another spelling of this path are this repository's;
    // an absent file here beside them is not "no override".
    if let Some(records) = path.parent() {
        for suffix in [JOURNAL_SUFFIX, AUTHORITY_SUFFIX] {
            let found = crate::repository::elsewhere(records, target, suffix);
            if !found.is_empty() {
                return Err(failed(
                    &path,
                    crate::repository::elsewhere_detail(records, target, &found),
                ));
            }
        }
    }
    let repository = target.display().to_string();
    let scan = read_bytes(&path)?.map(|b| scan(b, &repository));
    let abytes = read_bytes(&apath)?;
    let parsed: Option<Result<Authority, String>> = abytes.as_ref().map(|b| {
        serde_json::from_slice::<Authority>(b).map_err(|e| format!("does not parse: {e}"))
    });
    let key = crate::repository::key(target);
    let state = classify(scan.as_ref(), parsed.as_ref(), &key);
    Ok(Found {
        journal: FileFound {
            path: path.display().to_string(),
            sha256: scan
                .as_ref()
                .map_or(ABSENT.to_string(), |s| digest_bytes(&s.bytes)),
        },
        authority: FileFound {
            path: apath.display().to_string(),
            sha256: abytes
                .as_ref()
                .map_or(ABSENT.to_string(), |b| digest_bytes(b)),
        },
        state,
        scan,
        parsed: parsed.and_then(Result::ok),
    })
}

fn classify(
    scan: Option<&Scan>,
    authority: Option<&Result<Authority, String>>,
    key: &str,
) -> State {
    let invalid = scan.and_then(|s| s.invalid.clone());
    let a = match authority {
        None => {
            return match (scan, invalid) {
                (None, _) => State::Empty,
                (Some(_), Some(why)) => State::JournalInvalid(why),
                (Some(_), None) => State::Disagreement(
                    "a journal with no state authority; whether it predates section 3.1.5 or had \
                     its authority removed cannot be told"
                        .into(),
                ),
            };
        }
        Some(Err(why)) => {
            return match (scan, invalid) {
                (None, _) => State::JournalMissing,
                (Some(_), Some(j)) => State::JournalInvalid(j),
                (Some(_), None) => State::Disagreement(format!("the state authority {why}")),
            };
        }
        Some(Ok(a)) => a,
    };
    if a.journal != key {
        return State::Disagreement(format!(
            "the state authority names journal {}, and this one is {key}",
            a.journal
        ));
    }
    let Some(scan) = scan else {
        return match (a.lines, &a.intended) {
            (0, None) => State::Empty,
            (0, Some(_)) => State::IntentWithoutLine,
            _ => State::JournalMissing,
        };
    };
    if let Some(why) = invalid {
        return State::JournalInvalid(why);
    }
    let n = scan.lines.len() as u64;
    let c = a.lines;
    if n < c {
        return State::Disagreement(format!(
            "the journal has {n} complete line(s) and its authority records {c}: lines were \
             removed from its end, or an older journal was restored"
        ));
    }
    let at = |k: u64| scan.lines.get((k as usize).wrapping_sub(1)).map(|(_, d)| d);
    if a.last.as_ref() != at(c) {
        return State::Disagreement(format!(
            "line {c}, the last the authority records, does not digest to the recorded digest: \
             it was edited, or an older journal was restored"
        ));
    }
    match &a.intended {
        None if n > c => State::Disagreement(format!(
            "the journal has {n} complete line(s) and its authority records {c}: a line was \
             appended outside this product's write"
        )),
        None if scan.torn.is_some() => State::Disagreement(
            "the journal ends in a torn line its authority records no intended write for".into(),
        ),
        None => State::Agree,
        Some(i) if i.position != c + 1 => State::Disagreement(format!(
            "the authority records an intended line at position {} after {c} line(s)",
            i.position
        )),
        Some(_) if n == c && scan.torn.is_some() => State::IntentTorn,
        Some(_) if n == c => State::IntentWithoutLine,
        Some(i) if n == c + 1 && scan.torn.is_none() && at(n) == Some(&i.digest) => State::Pending,
        Some(_) => State::Disagreement(
            "a complete line past the authority does not digest to the recorded intended line"
                .into(),
        ),
    }
}

/// What a reader makes of a state: in force, or why not.
fn journal_of(found: &Found, busy: bool) -> Result<Journal, JournalError> {
    let path = found.journal.path.clone();
    if busy && found.state.interrupted() {
        return Err(JournalError::InProgress { path });
    }
    match &found.state {
        State::Empty | State::Agree | State::IntentWithoutLine | State::IntentTorn => {
            Ok(Journal::of(found))
        }
        State::Pending => Err(JournalError::Pending { path }),
        other => Err(JournalError::Failed {
            path,
            detail: format!(
                "cannot be read as a set of overrides: {}; nothing is in force; `override \
                 recover` names the choices",
                other.describe()
            ),
        }),
    }
}

/// Read and verify a repository's journal against its authority.
///
/// Takes the repository lock when it is free and reads under it; while another
/// process holds it, a file in one of rule 4's interrupted states is a write in
/// progress, never a pending line (3.1.5 rule 4).
pub fn read(home: &Path, target: &Path) -> Result<Journal, JournalError> {
    match crate::lock::try_acquire(home, target) {
        Ok(held) => read_held(home, target, &held),
        Err(crate::lock::LockError::Busy { .. }) => journal_of(&inspect(home, target)?, true),
        Err(crate::lock::LockError::Failed { path, detail }) => {
            Err(JournalError::Failed { path, detail })
        }
    }
}

/// Read under a lock the caller holds: how `run` reads the journal once,
/// before it appends its intent (3.1.5 rule 8).
pub fn read_held(
    home: &Path,
    target: &Path,
    _held: &crate::lock::Held,
) -> Result<Journal, JournalError> {
    journal_of(&inspect(home, target)?, false)
}

/// Whether a trimmed value is present.
fn required(what: &str, value: &str) -> Result<String, JournalError> {
    let v = value.trim();
    if v.is_empty() {
        return Err(JournalError::Refused(format!(
            "an override needs a non-empty {what}; nothing was written"
        )));
    }
    Ok(v.to_string())
}

/// A point in rule 4's write protocol at which a test stops the write, so the
/// state it leaves is the product's own. Never set outside a test.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// After step 1 made the zero-line authority durable.
    AfterStep1,
    /// After step 2 recorded the intended line.
    AfterStep2,
    /// During step 3, half of the line written.
    DuringStep3,
    /// Between step 3 and step 4.
    BetweenStep3And4,
}

fn injected(fault: Option<Fault>, at: Fault, path: &Path) -> Result<(), JournalError> {
    if fault == Some(at) {
        return Err(failed(
            path,
            format!("stopped by an injected fault: {at:?}"),
        ));
    }
    Ok(())
}

fn sync_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::File::open(dir)?.sync_all()
}

/// Replace a file durably: a new file, made durable, renamed over the old
/// one, and the directory made durable.
fn replace(path: &Path, bytes: &[u8]) -> Result<(), JournalError> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let tmp = dir.join(format!(
        ".{}.tmp-{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        std::process::id()
    ));
    let write = || -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)?;
        sync_dir(dir)
    };
    write().map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        failed(path, format!("could not be replaced durably: {e}"))
    })
}

fn write_authority(
    home: &Path,
    target: &Path,
    lines: u64,
    last: Option<String>,
    intended: Option<Intended>,
    at: &str,
) -> Result<(), JournalError> {
    let doc = Authority {
        journal: crate::repository::key(target),
        lines,
        last,
        intended,
        at: at.to_string(),
    };
    let path = authority_path(home, target);
    let mut bytes = serde_json::to_vec_pretty(&doc).map_err(|e| failed(&path, e.to_string()))?;
    bytes.push(b'\n');
    replace(&path, &bytes)
}

/// How the journal is prepared under the new line.
enum Base {
    /// Append to the journal as it is, first cutting it to this many bytes
    /// when it ends in a torn line.
    Append { cut_to: Option<u64> },
    /// Replace the journal with these bytes and the line (`adopt-prefix`).
    Replace(Vec<u8>),
}

/// Rule 4's protocol: one line into the journal, under the lock the caller
/// holds. `lines` and `last` are the baseline the line follows.
#[allow(clippy::too_many_arguments)]
fn commit(
    home: &Path,
    target: &Path,
    step1: bool,
    lines: u64,
    last: Option<String>,
    base: Base,
    line: &Line,
    fault: Option<Fault>,
) -> Result<String, JournalError> {
    let path = journal_path(home, target);
    let raw = serde_json::to_string(line).map_err(|e| failed(&path, e.to_string()))?;
    let digest = digest_bytes(raw.as_bytes());
    let at = line.at.as_str();
    // Step 1: the baseline authority, durable before anything else.
    if step1 {
        write_authority(home, target, lines, last.clone(), None, at)?;
    }
    injected(fault, Fault::AfterStep1, &path)?;
    // Step 2: the intended line.
    let intended = Intended {
        position: lines + 1,
        digest: digest.clone(),
    };
    write_authority(home, target, lines, last, Some(intended), at)?;
    injected(fault, Fault::AfterStep2, &path)?;
    // Step 3: the line.
    let bytes = format!("{raw}\n").into_bytes();
    let dir = path.parent().unwrap_or(Path::new("."));
    match base {
        Base::Replace(prefix) => {
            if fault == Some(Fault::DuringStep3) {
                let mut torn = prefix.clone();
                torn.extend_from_slice(&bytes[..bytes.len() / 2]);
                replace(&path, &torn)?;
                return injected(fault, Fault::DuringStep3, &path).map(|()| digest);
            }
            let mut whole = prefix;
            whole.extend_from_slice(&bytes);
            replace(&path, &whole)?;
        }
        Base::Append { cut_to } => {
            let created = !path.exists();
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|e| failed(&path, format!("could not be opened: {e}")))?;
            if let Some(len) = cut_to {
                file.set_len(len)
                    .and_then(|()| file.sync_all())
                    .map_err(|e| {
                        failed(
                            &path,
                            format!("its torn last line could not be cut off: {e}"),
                        )
                    })?;
            }
            let written = if fault == Some(Fault::DuringStep3) {
                &bytes[..bytes.len() / 2]
            } else {
                &bytes[..]
            };
            file.write_all(written)
                .and_then(|()| file.sync_all())
                .and_then(|()| if created { sync_dir(dir) } else { Ok(()) })
                .map_err(|e| failed(&path, format!("could not be written durably: {e}")))?;
            injected(fault, Fault::DuringStep3, &path)?;
        }
    }
    injected(fault, Fault::BetweenStep3And4, &path)?;
    // Step 4: the new count, and no intended line.
    write_authority(home, target, lines + 1, Some(digest.clone()), None, at)?;
    Ok(digest)
}

/// The lock is a precondition: busy is a refusal, anything else a failure.
fn lock_refusal(e: crate::lock::LockError) -> JournalError {
    match e {
        crate::lock::LockError::Busy { .. } => JournalError::Refused(e.to_string()),
        crate::lock::LockError::Failed { path, detail } => JournalError::Failed { path, detail },
    }
}

/// What the caller has established before a grant or a revocation.
#[derive(Debug, Clone)]
pub struct Request<'a> {
    /// The product home.
    pub home: &'a Path,
    /// The repository, registered (the caller checked).
    pub target: &'a Path,
    /// The spec id.
    pub spec_id: &'a str,
    /// Who, as supplied.
    pub operator: &'a str,
    /// Why.
    pub reason: &'a str,
    /// Now, RFC 3339 UTC.
    pub at: &'a str,
}

/// Grant an override (3.1.4 rule 1). `known` is whether the corpus report
/// names the spec id, which the caller read.
pub fn grant(request: &Request<'_>, known: bool) -> Result<InForce, JournalError> {
    change(request, Action::Grant, known, None)
}

/// Revoke an override (3.1.4 rule 1). Returns the one it revoked.
pub fn revoke(request: &Request<'_>) -> Result<InForce, JournalError> {
    change(request, Action::Revoke, true, None)
}

/// [`grant`] or [`revoke`], stopped at a point of rule 4's protocol. For the
/// acceptance of 3.1.5 only.
#[doc(hidden)]
pub fn change_with_fault(
    request: &Request<'_>,
    action: Action,
    fault: Fault,
) -> Result<InForce, JournalError> {
    change(request, action, true, Some(fault))
}

fn change(
    request: &Request<'_>,
    action: Action,
    known: bool,
    fault: Option<Fault>,
) -> Result<InForce, JournalError> {
    let operator = required("operator", request.operator)?;
    let reason = required("reason", request.reason)?;
    if !known {
        return Err(JournalError::Refused(format!(
            "{} is not a spec the corpus report names; nothing was written",
            request.spec_id
        )));
    }
    let (home, target) = (request.home, request.target);
    let held = crate::lock::try_acquire(home, target).map_err(lock_refusal)?;
    let found = inspect(home, target)?;
    let journal = journal_of(&found, false)?;
    let in_force = journal.in_force();
    let existing = in_force.iter().find(|o| o.spec_id == request.spec_id);
    let revoked = match (action, existing) {
        (Action::Grant, Some(_)) => {
            return Err(JournalError::Refused(format!(
                "an override for {} is already in force in this repository; revoke it first to \
                 change it; nothing was written",
                request.spec_id
            )));
        }
        (Action::Revoke, None) => {
            return Err(JournalError::Refused(format!(
                "no override for {} is in force in this repository; nothing was written",
                request.spec_id
            )));
        }
        (_, existing) => existing.cloned(),
    };
    let (lines, last) = baseline(&found);
    let line = Line {
        action,
        repository: target.display().to_string(),
        spec_id: Some(request.spec_id.to_string()),
        operator,
        operator_provenance: OPERATOR_PROVENANCE.to_string(),
        reason,
        at: request.at.to_string(),
        previous: last.clone(),
        cleared: cleared(&found),
        recovery: None,
    };
    let step1 = found.parsed.is_none();
    let base = Base::Append {
        cut_to: found
            .torn_bytes()
            .map(|_| found.scan.as_ref().map_or(0, |s| s.complete_len)),
    };
    let digest = commit(home, target, step1, lines, last, base, &line, fault)?;
    drop(held);
    Ok(revoked.unwrap_or(InForce {
        spec_id: request.spec_id.to_string(),
        operator: line.operator,
        operator_provenance: line.operator_provenance,
        reason: line.reason,
        granted_at: line.at,
        grant_line: digest,
        verification: Verification::Verified,
    }))
}

/// The count and last digest the next line follows, for a state a write may
/// proceed from.
fn baseline(found: &Found) -> (u64, Option<String>) {
    match &found.parsed {
        Some(a) => (a.lines, a.last.clone()),
        None => (0, None),
    }
}

/// What a write clears of rule 4's interrupted states.
fn cleared(found: &Found) -> Option<Cleared> {
    match found.state {
        State::IntentWithoutLine | State::IntentTorn => Some(Cleared {
            intended: found.intended()?.clone(),
            torn_bytes: found.torn_bytes(),
        }),
        _ => None,
    }
}

/// What an operator asks of `override recover` (3.1.5 rule 5).
#[derive(Debug, Clone)]
pub struct RecoverRequest<'a> {
    /// The product home.
    pub home: &'a Path,
    /// The repository, registered (the caller checked).
    pub target: &'a Path,
    /// The choice.
    pub choice: Choice,
    /// The journal's digest as the report showed it, or [`ABSENT`].
    pub journal: &'a str,
    /// The authority's digest as the report showed it, or [`ABSENT`].
    pub authority: &'a str,
    /// Who, as supplied.
    pub operator: &'a str,
    /// Why.
    pub reason: &'a str,
    /// Now, RFC 3339 UTC.
    pub at: &'a str,
}

/// What a recovery recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recovered {
    /// The choice.
    pub choice: Choice,
    /// The state it found.
    pub found: FoundDigests,
    /// The SHA-256 of the recovery line.
    pub line: String,
    /// The pending line voided, if one was.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voids: Option<String>,
    /// The file preserved, if one was.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preserved: Option<Preserved>,
    /// The overrides in force after it.
    pub in_force: Vec<InForce>,
}

/// Bring a journal and its authority back into agreement by the operator's
/// named choice (3.1.5 rule 5). Never picks a choice by itself.
pub fn recover(request: &RecoverRequest<'_>) -> Result<Recovered, JournalError> {
    let operator = required("operator", request.operator)?;
    let reason = required("reason", request.reason)?;
    let (home, target) = (request.home, request.target);
    let held = crate::lock::try_acquire(home, target).map_err(lock_refusal)?;
    let found = inspect(home, target)?;
    if found.journal.sha256 != request.journal || found.authority.sha256 != request.authority {
        return Err(JournalError::Refused(format!(
            "the files are not the state the report showed (journal {}, authority {} now); run \
             `override recover` without a choice to read them again; nothing was written",
            found.journal.sha256, found.authority.sha256
        )));
    }
    let allowed = found.state.allowed();
    if allowed.is_empty() {
        return Err(JournalError::Refused(
            "the journal and its state authority already agree and record no intended line; \
             there is nothing to recover; nothing was written"
                .into(),
        ));
    }
    if !allowed.contains(&request.choice) {
        let words: Vec<&str> = allowed.iter().map(|c| c.word()).collect();
        return Err(JournalError::Refused(format!(
            "{} is not a choice for this state ({}); it allows {}; nothing was written",
            request.choice.word(),
            found.state.word(),
            words.join(", ")
        )));
    }
    let path = journal_path(home, target);
    let scan = found.scan.as_ref();
    let verified = scan.map_or(0, |s| s.lines.len() as u64);
    let last_verified = scan.and_then(|s| s.lines.last().map(|(_, d)| d.clone()));
    let complete_len = scan.map_or(0, |s| s.complete_len);
    let mut voids = None;
    let mut preserved = None;
    let mut cleared_now = None;
    // The baseline the recovery line follows, and how the journal is prepared.
    let (lines, last, base) = match request.choice {
        Choice::CompletePending => (verified, last_verified, Base::Append { cut_to: None }),
        Choice::DiscardPending => {
            if found.state == State::Pending {
                voids = last_verified.clone();
                (verified, last_verified, Base::Append { cut_to: None })
            } else {
                cleared_now = cleared(&found);
                let (lines, last) = baseline(&found);
                let cut_to = found.torn_bytes().map(|_| complete_len);
                (lines, last, Base::Append { cut_to })
            }
        }
        Choice::AdoptAsRead => {
            let cut_to = found.torn_bytes().map(|_| complete_len);
            (verified, last_verified, Base::Append { cut_to })
        }
        Choice::AdoptPrefix => {
            let s = scan.expect("an invalid journal exists");
            let kept = s.ends.last().copied().unwrap_or(0) as usize;
            let sha256 = digest_bytes(&s.bytes);
            let keep = path.with_file_name(format!(
                "{}.found-{}",
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("journal"),
                &sha256[..16]
            ));
            replace(&keep, &s.bytes)?;
            preserved = Some(Preserved {
                path: keep.display().to_string(),
                sha256,
            });
            (
                verified,
                last_verified,
                Base::Replace(s.bytes[..kept].to_vec()),
            )
        }
        Choice::AdoptEmpty => (0, None, Base::Append { cut_to: None }),
    };
    let recovery = Recovery {
        choice: request.choice,
        found: found.digests(),
        voids: voids.clone(),
        preserved: preserved.clone(),
    };
    let line = Line {
        action: Action::Recover,
        repository: target.display().to_string(),
        spec_id: None,
        operator,
        operator_provenance: OPERATOR_PROVENANCE.to_string(),
        reason,
        at: request.at.to_string(),
        previous: last.clone(),
        cleared: cleared_now,
        recovery: Some(recovery.clone()),
    };
    // Every choice but the ones that follow the authority as it stands writes
    // its baseline first, as step 1 does for a first write.
    let step1 = !matches!(
        (request.choice, &found.parsed),
        (Choice::DiscardPending, Some(_)) if found.state != State::Pending
    );
    let digest = commit(home, target, step1, lines, last, base, &line, None)?;
    let after = journal_of(&inspect(home, target)?, false)?;
    drop(held);
    Ok(Recovered {
        choice: request.choice,
        found: recovery.found,
        line: digest,
        voids,
        preserved,
        in_force: after.in_force(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request<'a>(home: &'a Path, target: &'a Path, spec: &'a str) -> Request<'a> {
        Request {
            home,
            target,
            spec_id: spec,
            operator: "alice",
            reason: "reviewing the draft",
            at: "2026-09-23T00:00:00Z",
        }
    }

    fn recover_as<'a>(
        home: &'a Path,
        target: &'a Path,
        found: &'a Found,
        choice: Choice,
    ) -> RecoverRequest<'a> {
        RecoverRequest {
            home,
            target,
            choice,
            journal: &found.journal.sha256,
            authority: &found.authority.sha256,
            operator: "carol",
            reason: "settling it",
            at: "2026-09-24T00:00:00Z",
        }
    }

    fn state(home: &Path, target: &Path) -> State {
        inspect(home, target).unwrap().state
    }

    #[test]
    fn scan_measures_complete_lines_in_raw_bytes() {
        let found = scan(vec![0xff, b'\n', b'x'], "/fixture/a");
        assert!(found.invalid.is_some());
        assert_eq!(found.complete_len, 2);
        assert_eq!(found.torn.as_deref(), Some("x"));
    }

    #[test]
    fn grant_revoke_and_the_fold() {
        let home = tempfile::tempdir().unwrap();
        let target = Path::new("/fixture/a");
        assert!(read(home.path(), target).unwrap().in_force().is_empty());
        grant(&request(home.path(), target, "009-x"), true).unwrap();
        assert_eq!(state(home.path(), target), State::Agree);
        let j = read(home.path(), target).unwrap();
        assert_eq!(j.in_force().len(), 1);
        assert_eq!(j.in_force()[0].operator_provenance, OPERATOR_PROVENANCE);
        assert_eq!(j.in_force()[0].verification, Verification::Verified);
        assert!(j.authority().is_some());
        assert!(matches!(
            grant(&request(home.path(), target, "009-x"), true),
            Err(JournalError::Refused(_))
        ));
        revoke(&request(home.path(), target, "009-x")).unwrap();
        assert!(read(home.path(), target).unwrap().in_force().is_empty());
        assert!(matches!(
            revoke(&request(home.path(), target, "009-x")),
            Err(JournalError::Refused(_))
        ));
    }

    #[test]
    fn empty_operator_reason_or_unknown_spec_writes_nothing() {
        let home = tempfile::tempdir().unwrap();
        let target = Path::new("/fixture/a");
        let mut r = request(home.path(), target, "009-x");
        r.operator = "  ";
        assert!(matches!(grant(&r, true), Err(JournalError::Refused(_))));
        let mut r = request(home.path(), target, "009-x");
        r.reason = "";
        assert!(matches!(grant(&r, true), Err(JournalError::Refused(_))));
        assert!(matches!(
            grant(&request(home.path(), target, "009-x"), false),
            Err(JournalError::Refused(_))
        ));
        assert!(!journal_path(home.path(), target).exists());
        assert!(!authority_path(home.path(), target).exists());
    }

    #[test]
    fn an_edited_line_breaks_the_journal_rather_than_changing_the_set() {
        let home = tempfile::tempdir().unwrap();
        let target = Path::new("/fixture/a");
        grant(&request(home.path(), target, "009-x"), true).unwrap();
        grant(&request(home.path(), target, "010-y"), true).unwrap();
        let path = journal_path(home.path(), target);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, text.replacen("009-x", "009-z", 1)).unwrap();
        assert!(matches!(
            read(home.path(), target),
            Err(JournalError::Failed { .. })
        ));
        // The last line edited: the authority's digest catches it.
        std::fs::write(&path, text.replacen("010-y", "010-z", 1)).unwrap();
        assert!(matches!(state(home.path(), target), State::Disagreement(_)));
        assert!(matches!(
            read(home.path(), target),
            Err(JournalError::Failed { .. })
        ));
    }

    /// 3.1.5 rule 3: each shape of disagreement is a failure, never a
    /// different set of overrides.
    #[test]
    fn every_disagreement_is_a_failure() {
        let home = tempfile::tempdir().unwrap();
        let target = Path::new("/fixture/a");
        let path = journal_path(home.path(), target);
        let apath = authority_path(home.path(), target);
        grant(&request(home.path(), target, "009-x"), true).unwrap();
        let one = std::fs::read(&path).unwrap();
        let one_authority = std::fs::read(&apath).unwrap();
        revoke(&request(home.path(), target, "009-x")).unwrap();
        let two = std::fs::read(&path).unwrap();
        let reset = || std::fs::write(&path, &two).unwrap();
        // The final revocation deleted, which would resurrect the grant.
        std::fs::write(&path, &one).unwrap();
        assert!(matches!(state(home.path(), target), State::Disagreement(_)));
        assert!(read(home.path(), target).is_err());
        reset();
        // A line appended by hand past the authority.
        let mut more = two.clone();
        let extra = String::from_utf8(one.clone()).unwrap();
        more.extend_from_slice(extra.as_bytes());
        std::fs::write(&path, &more).unwrap();
        assert!(read(home.path(), target).is_err());
        reset();
        // The authority deleted, the journal remaining.
        let kept = std::fs::read(&apath).unwrap();
        std::fs::remove_file(&apath).unwrap();
        assert!(matches!(state(home.path(), target), State::Disagreement(_)));
        std::fs::write(&apath, &kept).unwrap();
        // The journal deleted, the authority remaining.
        std::fs::remove_file(&path).unwrap();
        assert_eq!(state(home.path(), target), State::JournalMissing);
        assert!(read(home.path(), target).is_err());
        // An older, internally valid journal and authority pair is not
        // detected by design (rule 1); an older journal alone is.
        std::fs::write(&path, &one).unwrap();
        std::fs::write(&apath, &one_authority).unwrap();
        assert_eq!(state(home.path(), target), State::Agree);
    }

    /// 3.1.5 rule 4: a fault at each boundary leaves the state the table names,
    /// and nothing is in force from it.
    #[test]
    fn each_interruption_leaves_the_state_rule_4_names() {
        let cases = [
            (Fault::AfterStep1, State::Empty),
            (Fault::AfterStep2, State::IntentWithoutLine),
            (Fault::DuringStep3, State::IntentTorn),
            (Fault::BetweenStep3And4, State::Pending),
        ];
        for (fault, expected) in cases {
            let home = tempfile::tempdir().unwrap();
            let target = Path::new("/fixture/a");
            assert!(
                change_with_fault(&request(home.path(), target, "009-x"), Action::Grant, fault)
                    .is_err()
            );
            assert_eq!(state(home.path(), target), expected, "{fault:?}");
            match read(home.path(), target) {
                Ok(j) => assert!(j.in_force().is_empty(), "{fault:?}"),
                Err(e) => assert!(matches!(e, JournalError::Pending { .. }), "{fault:?}"),
            }
            // While another process holds the lock, an interrupted state is a
            // write in progress.
            if expected != State::Empty {
                let held = crate::lock::try_acquire(home.path(), target).unwrap();
                assert!(matches!(
                    read(home.path(), target),
                    Err(JournalError::InProgress { .. })
                ));
                drop(held);
            }
        }
    }

    #[test]
    fn the_next_write_clears_an_intent_or_a_torn_line_and_records_it() {
        for fault in [Fault::AfterStep2, Fault::DuringStep3] {
            let home = tempfile::tempdir().unwrap();
            let target = Path::new("/fixture/a");
            grant(&request(home.path(), target, "009-x"), true).unwrap();
            let _ = change_with_fault(
                &request(home.path(), target, "009-x"),
                Action::Revoke,
                fault,
            );
            let j = read(home.path(), target).unwrap();
            assert_eq!(
                j.in_force().len(),
                1,
                "an interrupted revocation revokes nothing"
            );
            grant(&request(home.path(), target, "010-y"), true).unwrap();
            assert_eq!(state(home.path(), target), State::Agree);
            assert_eq!(read(home.path(), target).unwrap().in_force().len(), 2);
            let text = std::fs::read_to_string(journal_path(home.path(), target)).unwrap();
            let last: Line = serde_json::from_str(text.lines().last().unwrap()).unwrap();
            let cleared = last.cleared.expect("the clearing is recorded");
            assert_eq!(cleared.torn_bytes.is_some(), fault == Fault::DuringStep3);
        }
    }

    #[test]
    fn a_pending_line_refuses_writes_until_the_operator_settles_it() {
        for choice in [Choice::CompletePending, Choice::DiscardPending] {
            let home = tempfile::tempdir().unwrap();
            let target = Path::new("/fixture/a");
            let _ = change_with_fault(
                &request(home.path(), target, "009-x"),
                Action::Grant,
                Fault::BetweenStep3And4,
            );
            assert!(matches!(
                grant(&request(home.path(), target, "010-y"), true),
                Err(JournalError::Pending { .. })
            ));
            let found = inspect(home.path(), target).unwrap();
            // A choice the state does not allow is refused.
            assert!(matches!(
                recover(&recover_as(home.path(), target, &found, Choice::AdoptEmpty)),
                Err(JournalError::Refused(_))
            ));
            let done = recover(&recover_as(home.path(), target, &found, choice)).unwrap();
            assert_eq!(state(home.path(), target), State::Agree);
            let in_force = read(home.path(), target).unwrap().in_force();
            assert_eq!(
                in_force.len(),
                usize::from(choice == Choice::CompletePending)
            );
            assert_eq!(done.voids.is_some(), choice == Choice::DiscardPending);
            // The report the recovery used is now stale.
            assert!(matches!(
                recover(&recover_as(home.path(), target, &found, choice)),
                Err(JournalError::Refused(_))
            ));
        }
    }

    #[test]
    fn adoption_labels_every_earlier_grant_operator_adopted() {
        let home = tempfile::tempdir().unwrap();
        let target = Path::new("/fixture/a");
        grant(&request(home.path(), target, "009-x"), true).unwrap();
        std::fs::remove_file(authority_path(home.path(), target)).unwrap();
        let found = inspect(home.path(), target).unwrap();
        assert_eq!(found.state.allowed(), vec![Choice::AdoptAsRead]);
        recover(&recover_as(
            home.path(),
            target,
            &found,
            Choice::AdoptAsRead,
        ))
        .unwrap();
        grant(&request(home.path(), target, "010-y"), true).unwrap();
        let in_force = read(home.path(), target).unwrap().in_force();
        assert_eq!(in_force[0].verification, Verification::OperatorAdopted);
        assert_eq!(in_force[1].verification, Verification::Verified);
    }

    #[test]
    fn adopt_prefix_keeps_what_verifies_and_preserves_the_whole_file() {
        let home = tempfile::tempdir().unwrap();
        let target = Path::new("/fixture/a");
        grant(&request(home.path(), target, "009-x"), true).unwrap();
        grant(&request(home.path(), target, "010-y"), true).unwrap();
        let path = journal_path(home.path(), target);
        let text = std::fs::read_to_string(&path).unwrap();
        let mut lines: Vec<&str> = text.lines().collect();
        lines[1] = "not a line";
        std::fs::write(&path, format!("{}\n", lines.join("\n"))).unwrap();
        let found = inspect(home.path(), target).unwrap();
        assert!(matches!(found.state, State::JournalInvalid(_)));
        let done = recover(&recover_as(
            home.path(),
            target,
            &found,
            Choice::AdoptPrefix,
        ))
        .unwrap();
        let kept = done.preserved.unwrap();
        assert_eq!(
            digest_bytes(&std::fs::read(&kept.path).unwrap()),
            kept.sha256
        );
        let in_force = read(home.path(), target).unwrap().in_force();
        assert_eq!(in_force.len(), 1);
        assert_eq!(in_force[0].spec_id, "009-x");
        assert_eq!(in_force[0].verification, Verification::OperatorAdopted);
    }

    #[test]
    fn adopt_empty_leaves_nothing_in_force() {
        let home = tempfile::tempdir().unwrap();
        let target = Path::new("/fixture/a");
        grant(&request(home.path(), target, "009-x"), true).unwrap();
        std::fs::remove_file(journal_path(home.path(), target)).unwrap();
        let found = inspect(home.path(), target).unwrap();
        assert_eq!(found.journal.sha256, ABSENT);
        recover(&recover_as(home.path(), target, &found, Choice::AdoptEmpty)).unwrap();
        assert_eq!(state(home.path(), target), State::Agree);
        assert!(read(home.path(), target).unwrap().in_force().is_empty());
    }

    #[test]
    fn discard_pending_clears_an_intent_without_a_line() {
        let home = tempfile::tempdir().unwrap();
        let target = Path::new("/fixture/a");
        let _ = change_with_fault(
            &request(home.path(), target, "009-x"),
            Action::Grant,
            Fault::DuringStep3,
        );
        let found = inspect(home.path(), target).unwrap();
        assert_eq!(found.state, State::IntentTorn);
        recover(&recover_as(
            home.path(),
            target,
            &found,
            Choice::DiscardPending,
        ))
        .unwrap();
        assert_eq!(state(home.path(), target), State::Agree);
        assert!(read(home.path(), target).unwrap().in_force().is_empty());
    }

    #[test]
    fn agreeing_files_have_nothing_to_recover() {
        let home = tempfile::tempdir().unwrap();
        let target = Path::new("/fixture/a");
        grant(&request(home.path(), target, "009-x"), true).unwrap();
        let found = inspect(home.path(), target).unwrap();
        assert!(matches!(
            recover(&recover_as(
                home.path(),
                target,
                &found,
                Choice::AdoptAsRead
            )),
            Err(JournalError::Refused(_))
        ));
    }

    #[test]
    fn a_grant_while_the_lock_is_held_is_refused_and_writes_nothing() {
        let home = tempfile::tempdir().unwrap();
        let target = Path::new("/fixture/a");
        let held = crate::lock::try_acquire(home.path(), target).unwrap();
        assert!(matches!(
            grant(&request(home.path(), target, "009-x"), true),
            Err(JournalError::Refused(_))
        ));
        assert!(!journal_path(home.path(), target).exists());
        drop(held);
        assert!(grant(&request(home.path(), target, "009-x"), true).is_ok());
    }

    #[test]
    fn one_repository_never_reads_anothers_overrides() {
        let home = tempfile::tempdir().unwrap();
        grant(
            &request(home.path(), Path::new("/fixture/a"), "009-x"),
            true,
        )
        .unwrap();
        assert!(
            read(home.path(), Path::new("/fixture/b"))
                .unwrap()
                .in_force()
                .is_empty()
        );
    }
}
