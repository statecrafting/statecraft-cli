//! What records an attempted write to the record, and what does not.
//!
//! Spec 003 section 3.5.1. Two kinds of evidence and only these: a structured
//! request to write a protected path, read from the session's own stream
//! (rule 1, first kind), and a change to the record while the process ran,
//! found by digesting the protected files before the spawn and after the
//! process ends (rule 1, second kind). Each kind is answered for each attempt
//! as `observed`, `none-observed` or `unknown`, and none implies another
//! (rule 6).
//!
//! Nothing here interprets a shell command's text, and nothing here changes an
//! attempt's outcome (rule 5).

use serde::{Deserialize, Serialize};
use statecraft_environment::digest::digest_bytes;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

/// One kind's answer for one attempt (rule 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Answer {
    /// The kind was established, and at least one finding was recorded.
    Observed,
    /// The kind was established, and nothing was found.
    NoneObserved,
    /// The kind could not be established. Never rendered as `none-observed`.
    Unknown,
}

impl Answer {
    /// The word a report prints.
    pub fn word(self) -> &'static str {
        match self {
            Answer::Observed => "observed",
            Answer::NoneObserved => "none observed",
            Answer::Unknown => "unknown",
        }
    }

    /// `observed` when anything was found, otherwise `none-observed`.
    pub fn of(found: bool) -> Self {
        if found {
            Answer::Observed
        } else {
            Answer::NoneObserved
        }
    }
}

/// Both kinds' answers for one attempt.
///
/// The default is `unknown` for both, so an attempt nothing examined can never
/// read as one that was checked and found clean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Answers {
    /// The first kind: a structured request to write a protected path.
    pub write_request: Answer,
    /// The second kind: a change to the record while the process ran.
    pub record_change: Answer,
}

impl Default for Answers {
    fn default() -> Self {
        Self {
            write_request: Answer::Unknown,
            record_change: Answer::Unknown,
        }
    }
}

impl Answers {
    /// An attempt refused before any process existed: there was nothing that
    /// could have written, so both kinds are established and empty.
    pub fn without_process() -> Self {
        Self {
            write_request: Answer::NoneObserved,
            record_change: Answer::NoneObserved,
        }
    }
}

/// What became of a write request, as the stream reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Classification {
    /// The provider refused it, or reported that it did not execute.
    Refused,
    /// A result reported it executed without error.
    Executed,
    /// Neither: no result, or an error result that does not say which.
    Unresolved,
}

/// A file-write request the adapter read from the session's structured
/// stream, before this product has judged its target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requested {
    /// The tool, as the stream names it.
    pub tool: String,
    /// The tool-use id.
    pub tool_use_id: String,
    /// The target as the request names it, unresolved.
    pub target: String,
    /// What the stream says became of it.
    pub classification: Classification,
}

/// A finding of the first kind (rule 2): recorded in the accounting record
/// under `tamperFindings`, beside the refusal count, which it does not change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteRequest {
    /// Always `write-request`.
    pub kind: String,
    /// The target as the request named it.
    pub target: String,
    /// The target resolved against the workspace, as compared.
    pub resolved: String,
    /// The tool.
    pub tool: String,
    /// The tool-use id.
    pub tool_use_id: String,
    /// What the stream says became of it.
    pub classification: Classification,
}

/// The paths a confined child may not write, and the ones it may.
#[derive(Debug, Clone, Default)]
pub struct Protected {
    /// Every root a write inside is forbidden: the product home always, and
    /// the confinement's inaccessible and read-only roots where one applies.
    pub forbidden: Vec<PathBuf>,
    /// The confinement's write grants, which win inside a forbidden root.
    pub writable: Vec<PathBuf>,
}

impl Protected {
    /// The judged set for an attempt. Each root is canonicalized where it
    /// exists, so a request spelled through a link compares as its target.
    pub fn new(home: &Path, forbidden: &[PathBuf], writable: &[PathBuf]) -> Self {
        let mut all = vec![resolve_existing(home)];
        all.extend(forbidden.iter().map(|p| resolve_existing(p)));
        Self {
            forbidden: all,
            writable: writable.iter().map(|p| resolve_existing(p)).collect(),
        }
    }

    /// Judge each request (rule 1, first kind). A request counts when its
    /// target, resolved against the workspace and following links through the
    /// part of the path that exists, lies inside a forbidden root and outside
    /// every write grant. Its classification is kept beside it.
    pub fn judge(&self, workspace: &Path, requests: &[Requested]) -> Vec<WriteRequest> {
        requests
            .iter()
            .filter_map(|r| {
                let named = Path::new(&r.target);
                let joined = if named.is_absolute() {
                    named.to_path_buf()
                } else {
                    workspace.join(named)
                };
                let resolved = resolve_existing(&joined);
                let forbidden = self.forbidden.iter().any(|f| resolved.starts_with(f))
                    && !self.writable.iter().any(|w| resolved.starts_with(w));
                forbidden.then(|| WriteRequest {
                    kind: "write-request".to_string(),
                    target: r.target.clone(),
                    resolved: resolved.display().to_string(),
                    tool: r.tool.clone(),
                    tool_use_id: r.tool_use_id.clone(),
                    classification: r.classification,
                })
            })
            .collect()
    }
}

/// Resolve a path lexically, then follow links through its longest existing
/// prefix and append the rest. A path that does not exist at all is returned
/// with `.` and `..` removed.
pub fn resolve_existing(path: &Path) -> PathBuf {
    let mut lexical = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                lexical.pop();
            }
            Component::CurDir => {}
            other => lexical.push(other.as_os_str()),
        }
    }
    let mut prefix = lexical.clone();
    let mut rest = Vec::new();
    loop {
        if let Ok(real) = std::fs::canonicalize(&prefix) {
            let mut out = real;
            for part in rest.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (prefix.file_name().map(ToOwned::to_owned), prefix.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                prefix = parent.to_path_buf();
            }
            _ => return lexical,
        }
    }
}

/// One file's state, as digested.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileState {
    /// sha256 of the bytes, hex.
    pub sha256: String,
    /// Length in bytes.
    pub length: u64,
}

impl FileState {
    fn of(bytes: &[u8]) -> Self {
        Self {
            sha256: digest_bytes(bytes),
            length: bytes.len() as u64,
        }
    }
}

/// The protected files of one repository at one moment (rule 1, second kind):
/// the run record, the override journal and its state authority where they
/// exist, and every launch record under the repository's records directory.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snapshot {
    files: BTreeMap<PathBuf, FileState>,
}

/// The files a snapshot covers, for a repository in a home.
fn covered_roots(home: &Path, target: &Path) -> (Vec<PathBuf>, PathBuf) {
    let chain = crate::record::chain_path(home, target);
    let files = vec![
        chain.clone(),
        crate::overrides::journal_path(home, target),
        crate::overrides::authority_path(home, target),
    ];
    (files, chain.with_extension("startup"))
}

impl Snapshot {
    /// Digest every covered file now. A file that is absent is absent from the
    /// snapshot; one that cannot be read is an error, never a silent absence.
    pub fn take(home: &Path, target: &Path) -> std::io::Result<Self> {
        let (files, launch_records) = covered_roots(home, target);
        let mut out = BTreeMap::new();
        for file in files {
            match std::fs::read(&file) {
                Ok(bytes) => {
                    out.insert(file, FileState::of(&bytes));
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        walk(&launch_records, &mut out)?;
        Ok(Self { files: out })
    }

    /// Every difference between `before` and `self`, except a file whose
    /// bytes now are exactly what the supervisor itself wrote during the
    /// window (`launched.json`, `admission.json`), which is compared with
    /// those bytes rather than with `before`.
    pub fn changes_since(&self, before: &Snapshot, wrote: &[(PathBuf, Vec<u8>)]) -> Vec<Change> {
        let expected: BTreeMap<&Path, FileState> = wrote
            .iter()
            .map(|(p, b)| (p.as_path(), FileState::of(b)))
            .collect();
        let mut paths: Vec<&PathBuf> = before.files.keys().chain(self.files.keys()).collect();
        paths.sort();
        paths.dedup();
        paths
            .into_iter()
            .filter_map(|path| {
                let was = before.files.get(path);
                let now = self.files.get(path);
                let unchanged = match expected.get(path.as_path()) {
                    Some(wrote) => now == Some(wrote),
                    None => was == now,
                };
                (!unchanged).then(|| Change {
                    path: path.display().to_string(),
                    before: was.cloned(),
                    after: now.cloned(),
                })
            })
            .collect()
    }
}

fn walk(dir: &Path, out: &mut BTreeMap<PathBuf, FileState>) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_dir() {
            walk(&path, out)?;
        } else {
            // A link is digested as its own bytes, never followed.
            let bytes = if kind.is_symlink() {
                std::fs::read_link(&path)?
                    .into_os_string()
                    .into_encoded_bytes()
            } else {
                std::fs::read(&path)?
            };
            out.insert(path, FileState::of(&bytes));
        }
    }
    Ok(())
}

/// One protected file that differs (rule 2): the before and after digests and
/// lengths, never the content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    /// The file.
    pub path: String,
    /// Its state before the spawn; absent when it did not exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<FileState>,
    /// Its state after the process ended; absent when it no longer exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<FileState>,
}

/// A finding of the second kind, as the audit file records it (rule 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditFinding {
    /// Always `record-change`.
    pub kind: String,
    /// The run.
    pub run_id: String,
    /// The attempt.
    pub attempt: u32,
    /// When the supervisor found it, RFC 3339 UTC.
    pub found_at: String,
    /// Every protected file that differs.
    pub changes: Vec<Change>,
}

impl AuditFinding {
    /// One line for a report.
    pub fn describe(&self) -> String {
        format!(
            "{}/{}: a change to the record this product did not make ({})",
            self.run_id,
            self.attempt,
            self.changes
                .iter()
                .map(|c| c.path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// The append-only audit file beside a repository's record.
pub fn audit_path(home: &Path, target: &Path) -> PathBuf {
    crate::record::chain_path(home, target).with_extension("audit.jsonl")
}

/// Append one finding and make it durable, with the directory when the append
/// created the file (rule 3: a finding that cannot be made durable is an
/// error the caller reports, never a silence).
pub fn append_audit(home: &Path, target: &Path, finding: &AuditFinding) -> std::io::Result<()> {
    let path = audit_path(home, target);
    let created = !path.exists();
    let line = serde_json::to_string(finding).map_err(std::io::Error::other)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    file.write_all(format!("{line}\n").as_bytes())?;
    file.sync_all()?;
    if created && let Some(parent) = path.parent() {
        std::fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

/// Every finding the audit file holds, oldest first. An absent file holds
/// none. A line that does not parse is reported, never skipped.
pub fn read_audit(home: &Path, target: &Path) -> Result<Vec<AuditFinding>, String> {
    let path = audit_path(home, target);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(n, l)| {
            serde_json::from_str(l).map_err(|e| format!("{} line {}: {e}", path.display(), n + 1))
        })
        .collect()
}

/// What an accounting record says about this section (rule 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checked {
    /// Written before this section: no `tamperFindings` member. Read as "no
    /// structured finding recorded", never as a clean result.
    NotRecorded,
    /// Written under this section's rules, findings empty or not.
    Checked {
        /// The first kind's findings.
        findings: Vec<WriteRequest>,
        /// Both kinds' answers; absent only from a record that predates them.
        answers: Option<Answers>,
    },
}

/// Read an accounting record's detail. The member's presence is the marker,
/// so an old empty `tamper_attempts` and a new empty `tamperFindings` are
/// never confused.
pub fn checked(detail: &serde_json::Value) -> Result<Checked, String> {
    let Some(findings) = detail.get("tamperFindings") else {
        return Ok(Checked::NotRecorded);
    };
    let findings: Vec<WriteRequest> =
        serde_json::from_value(findings.clone()).map_err(|e| format!("tamperFindings: {e}"))?;
    let answers = detail
        .get("tamperAnswers")
        .map(|a| serde_json::from_value(a.clone()).map_err(|e| format!("tamperAnswers: {e}")))
        .transpose()?;
    Ok(Checked::Checked { findings, answers })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(target: &str) -> Requested {
        Requested {
            tool: "Write".into(),
            tool_use_id: "toolu_1".into(),
            target: target.into(),
            classification: Classification::Refused,
        }
    }

    #[test]
    fn a_request_into_the_home_counts_and_one_into_the_workspace_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let workspace = dir.path().join("work");
        std::fs::create_dir_all(home.join("records")).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();
        let protected = Protected::new(&home, &[], &[]);
        let target = home.join("records/x.jsonl");
        let found = protected.judge(
            &workspace,
            &[
                request(target.to_str().unwrap()),
                request("src/lib.rs"),
                request("../home/records/y.jsonl"),
            ],
        );
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].classification, Classification::Refused);
        assert!(found[1].resolved.ends_with("home/records/y.jsonl"));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_the_workspace_resolves_to_where_it_points() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let workspace = dir.path().join("work");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();
        std::os::unix::fs::symlink(&home, workspace.join("innocent")).unwrap();
        let found = Protected::new(&home, &[], &[]).judge(&workspace, &[request("innocent/new")]);
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn a_write_grant_inside_a_forbidden_root_is_not_a_finding() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        let grant = target.join("work");
        std::fs::create_dir_all(&grant).unwrap();
        let home = dir.path().join("home");
        let protected = Protected::new(
            &home,
            std::slice::from_ref(&target),
            std::slice::from_ref(&grant),
        );
        let found = protected.judge(&grant, &[request("a.rs"), request("../README.md")]);
        assert_eq!(found.len(), 1);
        assert!(found[0].resolved.ends_with("target/README.md"));
    }

    #[test]
    fn a_change_is_found_and_the_supervisors_own_writes_are_not() {
        let dir = tempfile::tempdir().unwrap();
        let (home, target) = (dir.path().join("home"), dir.path().join("t"));
        let chain = crate::record::chain_path(&home, &target);
        std::fs::create_dir_all(chain.parent().unwrap()).unwrap();
        std::fs::write(&chain, "one\n").unwrap();
        let attempt = chain.with_extension("startup").join("r/1");
        std::fs::create_dir_all(&attempt).unwrap();
        std::fs::write(attempt.join("intent.json"), "{}").unwrap();
        let before = Snapshot::take(&home, &target).unwrap();

        let launched = attempt.join("launched.json");
        std::fs::write(&launched, "pid").unwrap();
        let wrote = vec![(launched.clone(), b"pid".to_vec())];
        let after = Snapshot::take(&home, &target).unwrap();
        assert!(after.changes_since(&before, &wrote).is_empty());

        std::fs::write(&chain, "one\ntwo\n").unwrap();
        std::fs::write(&launched, "forged").unwrap();
        let changed = Snapshot::take(&home, &target)
            .unwrap()
            .changes_since(&before, &wrote);
        assert_eq!(changed.len(), 2, "{changed:?}");
        assert_eq!(changed[0].path, chain.display().to_string());
        assert_eq!(changed[0].before.as_ref().unwrap().length, 4);
        assert_eq!(changed[0].after.as_ref().unwrap().length, 8);
    }

    #[test]
    fn the_audit_file_appends_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let (home, target) = (dir.path().join("home"), dir.path().join("t"));
        std::fs::create_dir_all(home.join("records")).unwrap();
        assert!(read_audit(&home, &target).unwrap().is_empty());
        let finding = AuditFinding {
            kind: "record-change".into(),
            run_id: "r".into(),
            attempt: 1,
            found_at: "t".into(),
            changes: vec![],
        };
        append_audit(&home, &target, &finding).unwrap();
        append_audit(&home, &target, &finding).unwrap();
        assert_eq!(read_audit(&home, &target).unwrap().len(), 2);
    }

    #[test]
    fn an_older_shape_is_not_recorded_and_an_empty_new_one_is_checked() {
        let old = serde_json::json!({"count": 0, "sample": [], "tamper_attempts": []});
        assert_eq!(checked(&old).unwrap(), Checked::NotRecorded);
        assert_eq!(
            checked(&serde_json::json!({"count": 0})).unwrap(),
            Checked::NotRecorded
        );
        let new = serde_json::json!({"count": 0, "tamperFindings": []});
        assert_eq!(
            checked(&new).unwrap(),
            Checked::Checked {
                findings: vec![],
                answers: None
            }
        );
    }

    #[test]
    fn an_unexamined_attempt_is_unknown_never_none_observed() {
        let a = Answers::default();
        assert_eq!(a.write_request, Answer::Unknown);
        assert_eq!(a.record_change, Answer::Unknown);
        assert_eq!(Answer::Unknown.word(), "unknown");
    }
}
