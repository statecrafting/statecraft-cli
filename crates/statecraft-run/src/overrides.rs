//! The override journal: how an operator's single-spec override is made,
//! kept, removed and read.
//!
//! Spec 003 section 3.1.4. Section 3.1.1 point 2 fixed that this product's own
//! state holds only an explicit, recorded override for a single named spec id
//! per repository; this module is where that state lives and the only code that
//! writes it.
//!
//! # The journal
//!
//! One append-only file per registered repository, beside that repository's
//! run record in the product home, never in the target. Each line is one grant
//! or one revocation and carries the SHA-256 of the line before it, so a line
//! removed, reordered or edited reads as a broken journal rather than as a
//! different set of overrides (rule 3). The overrides in force are the fold:
//! granted and not later revoked.
//!
//! # What this module does not decide
//!
//! Whether a spec id exists, and whether a repository is registered, are read
//! by the caller from the report and the registry this crate does not own, and
//! passed in as facts. The operator's name is **supplied, not authenticated**
//! (rule 2): nothing here can check it, and the line says so.

use crate::policy::{Override, Overrides};
use serde::{Deserialize, Serialize};
use statecraft_environment::digest::digest_bytes;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The word every journal line carries beside the operator's name.
pub const OPERATOR_PROVENANCE: &str = "operator-supplied";

/// The journal file for a repository inside a product home.
///
/// Keyed like the run record, so the two sit side by side and one repository's
/// journal is never read for another (rule 4), and by the registration's
/// stored root ([`crate::repository`]), so one repository has one journal
/// however its path was typed.
pub fn journal_path(home: &Path, target: &Path) -> PathBuf {
    let key = crate::repository::key(target);
    home.join("records").join(format!("{key}.overrides.jsonl"))
}

/// Grant or revoke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    /// An override comes into force.
    Grant,
    /// It stops being in force.
    Revoke,
}

/// One line of the journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Line {
    /// Grant or revoke.
    pub action: Action,
    /// The repository, as registered.
    pub repository: String,
    /// The one spec id.
    pub spec_id: String,
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
}

/// An override in force, with the facts rule 5 records on an attempt.
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
}

/// A journal read and verified.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Journal {
    lines: Vec<(Line, String)>,
    /// A final line with no line break: a write that did not complete. Not in
    /// force, reported, and truncated before the next append (rule 3).
    torn: Option<String>,
    /// The byte length of the complete lines, which is where the next append
    /// starts when there is a torn line to cut off.
    complete_len: u64,
}

/// Why the journal could not be used.
#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    /// A precondition failed and nothing was written.
    #[error("{0}")]
    Refused(String),
    /// The journal did not read or verify, or a write was not durable.
    ///
    /// Never read as an empty journal: an unreadable one that read as "no
    /// override" would be indistinguishable from one nobody wrote (rule 3).
    #[error("the override journal {path} {detail}")]
    Failed {
        /// The file.
        path: String,
        /// What went wrong.
        detail: String,
    },
}

impl Journal {
    /// The overrides in force: granted and not later revoked, in grant order.
    pub fn in_force(&self) -> Vec<InForce> {
        let mut out: Vec<InForce> = Vec::new();
        for (line, digest) in &self.lines {
            match line.action {
                Action::Grant => out.push(InForce {
                    spec_id: line.spec_id.clone(),
                    operator: line.operator.clone(),
                    operator_provenance: line.operator_provenance.clone(),
                    reason: line.reason.clone(),
                    granted_at: line.at.clone(),
                    grant_line: digest.clone(),
                }),
                Action::Revoke => out.retain(|o| o.spec_id != line.spec_id),
            }
        }
        out
    }

    /// The same, in the shape work selection reads.
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
                })
                .collect(),
        }
    }

    /// The torn last line, if the journal ends in one.
    pub fn torn(&self) -> Option<&str> {
        self.torn.as_deref()
    }

    fn last_digest(&self) -> Option<String> {
        self.lines.last().map(|(_, d)| d.clone())
    }
}

/// Read and verify a repository's journal. An absent file is an empty journal,
/// which is the one case that is: nobody has granted anything.
pub fn read(home: &Path, target: &Path) -> Result<Journal, JournalError> {
    let path = journal_path(home, target);
    let failed = |detail: String| JournalError::Failed {
        path: path.display().to_string(),
        detail,
    };
    // A journal filed under another spelling of this path is this
    // repository's; an absent file here beside it is not "no override".
    if let Some(records) = path.parent() {
        let found = crate::repository::elsewhere(records, target, ".overrides.jsonl");
        if !found.is_empty() {
            return Err(failed(crate::repository::elsewhere_detail(&found)));
        }
    }
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Journal::default()),
        Err(e) => return Err(failed(format!("could not be read: {e}"))),
    };
    // A final segment with no line break is a torn line: a write that did not
    // complete. It is set aside, never read as a grant or a revocation.
    let (complete, torn) = match text.rfind('\n') {
        Some(i) if i + 1 < text.len() => (&text[..=i], Some(text[i + 1..].to_string())),
        Some(_) => (text.as_str(), None),
        None if text.is_empty() => ("", None),
        None => ("", Some(text.clone())),
    };
    let repository = target.display().to_string();
    let mut journal = Journal {
        torn,
        complete_len: complete.len() as u64,
        ..Journal::default()
    };
    for (i, raw) in complete.lines().enumerate() {
        let n = i + 1;
        let line: Line = serde_json::from_str(raw)
            .map_err(|e| failed(format!("line {n} is not a journal line: {e}")))?;
        if line.previous != journal.last_digest() {
            return Err(failed(format!(
                "line {n} does not follow line {}: the journal was edited or reordered",
                i
            )));
        }
        if line.repository != repository {
            return Err(failed(format!(
                "line {n} names repository {}, and this journal is {repository}'s",
                line.repository
            )));
        }
        journal.lines.push((line, digest_bytes(raw.as_bytes())));
    }
    Ok(journal)
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

/// Append one line durably, first cutting off a torn last line if there is one.
fn append(
    home: &Path,
    target: &Path,
    journal: &Journal,
    line: &Line,
) -> Result<String, JournalError> {
    let path = journal_path(home, target);
    let failed = |detail: String| JournalError::Failed {
        path: path.display().to_string(),
        detail,
    };
    let raw = serde_json::to_string(line).map_err(|e| failed(e.to_string()))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| failed(format!("could not be created: {e}")))?;
    }
    if journal.torn.is_some() {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|e| failed(format!("could not be opened: {e}")))?;
        file.set_len(journal.complete_len)
            .and_then(|()| file.sync_all())
            .map_err(|e| failed(format!("its torn last line could not be cut off: {e}")))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| failed(format!("could not be opened: {e}")))?;
    file.write_all(format!("{raw}\n").as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| failed(format!("could not be written durably: {e}")))?;
    Ok(digest_bytes(raw.as_bytes()))
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

/// Grant an override (rule 1). `known` is whether the corpus report names the
/// spec id, which the caller read.
pub fn grant(request: &Request<'_>, known: bool) -> Result<InForce, JournalError> {
    let operator = required("operator", request.operator)?;
    let reason = required("reason", request.reason)?;
    if !known {
        return Err(JournalError::Refused(format!(
            "{} is not a spec the corpus report names; nothing was written",
            request.spec_id
        )));
    }
    let _held = crate::lock::try_acquire(request.home, request.target).map_err(lock_refusal)?;
    let journal = read(request.home, request.target)?;
    if journal
        .in_force()
        .iter()
        .any(|o| o.spec_id == request.spec_id)
    {
        return Err(JournalError::Refused(format!(
            "an override for {} is already in force in this repository; revoke it first to \
             change it; nothing was written",
            request.spec_id
        )));
    }
    let line = Line {
        action: Action::Grant,
        repository: request.target.display().to_string(),
        spec_id: request.spec_id.to_string(),
        operator,
        operator_provenance: OPERATOR_PROVENANCE.to_string(),
        reason,
        at: request.at.to_string(),
        previous: journal.last_digest(),
    };
    let digest = append(request.home, request.target, &journal, &line)?;
    Ok(InForce {
        spec_id: line.spec_id,
        operator: line.operator,
        operator_provenance: line.operator_provenance,
        reason: line.reason,
        granted_at: line.at,
        grant_line: digest,
    })
}

/// Revoke an override (rule 1). Returns the one it revoked.
pub fn revoke(request: &Request<'_>) -> Result<InForce, JournalError> {
    let operator = required("operator", request.operator)?;
    let reason = required("reason", request.reason)?;
    let _held = crate::lock::try_acquire(request.home, request.target).map_err(lock_refusal)?;
    let journal = read(request.home, request.target)?;
    let Some(revoked) = journal
        .in_force()
        .into_iter()
        .find(|o| o.spec_id == request.spec_id)
    else {
        return Err(JournalError::Refused(format!(
            "no override for {} is in force in this repository; nothing was written",
            request.spec_id
        )));
    };
    let line = Line {
        action: Action::Revoke,
        repository: request.target.display().to_string(),
        spec_id: request.spec_id.to_string(),
        operator,
        operator_provenance: OPERATOR_PROVENANCE.to_string(),
        reason,
        at: request.at.to_string(),
        previous: journal.last_digest(),
    };
    append(request.home, request.target, &journal, &line)?;
    Ok(revoked)
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

    #[test]
    fn grant_revoke_and_the_fold() {
        let home = tempfile::tempdir().unwrap();
        let target = Path::new("/fixture/a");
        assert!(read(home.path(), target).unwrap().in_force().is_empty());
        grant(&request(home.path(), target, "009-x"), true).unwrap();
        let j = read(home.path(), target).unwrap();
        assert_eq!(j.in_force().len(), 1);
        assert_eq!(j.in_force()[0].operator_provenance, OPERATOR_PROVENANCE);
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
        // Removing the first line breaks the second's link too.
        let second = text.lines().nth(1).unwrap();
        std::fs::write(&path, format!("{second}\n")).unwrap();
        assert!(matches!(
            read(home.path(), target),
            Err(JournalError::Failed { .. })
        ));
    }

    #[test]
    fn a_torn_last_line_is_not_in_force_and_is_cut_off_by_the_next_write() {
        let home = tempfile::tempdir().unwrap();
        let target = Path::new("/fixture/a");
        grant(&request(home.path(), target, "009-x"), true).unwrap();
        let path = journal_path(home.path(), target);
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str("{\"action\":\"revoke\",\"repos");
        std::fs::write(&path, &text).unwrap();
        let j = read(home.path(), target).unwrap();
        assert!(j.torn().is_some());
        assert_eq!(j.in_force().len(), 1, "a torn revocation revokes nothing");
        grant(&request(home.path(), target, "010-y"), true).unwrap();
        let j = read(home.path(), target).unwrap();
        assert!(j.torn().is_none());
        assert_eq!(j.in_force().len(), 2);
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
