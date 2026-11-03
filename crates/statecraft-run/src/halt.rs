//! The local halt record (spec 050).
//!
//! A halt is a standing refusal an operator sets and lifts, per registered
//! repository, in three independent scopes. Its state is a **fold over halt
//! and resume entries in the run record** (section 3.1): the record is already
//! hash-linked, fsynced before acknowledge and out of the supervised process's
//! reach (spec 003 section 3.3), so a halt that was edited or truncated is
//! detected the way a tampered record is. Nothing in the target says whether a
//! repository is halted.
//!
//! It fails closed (section 3.3). A halt entry that does not parse, a chain
//! that ended mid-record, and a chain whose links do not verify are each
//! `halt-state-unreadable`, and none of them admits a launch. There is no cached
//! answer: [`admit`] folds the chain it is handed every time, and the caller
//! holds the repository lock while it does.
//!
//! Stopping a live attempt is two facts, not one (section 3.4): `stop-requested`
//! is written by the halt, `stop-confirmed` only once that attempt's
//! `cancelled` outcome is durable. The fold reports exactly what is recorded.
//!
//! A halt or resume made while a `run` holds the repository lock cannot be
//! appended by the operator's process (spec 003 section 3.1.4 rule 7), so it
//! is **queued** as a request file beside the chain, and the supervisor
//! holding the lock appends it ([`queue`], [`apply_pending`]; section 5,
//! 2026-11-01). A request is not the halt state: the record is, once the
//! request is appended. Every writer that takes the lock to append a halt,
//! a resume, an attempt or a reconciliation applies what is queued first, so
//! a request a crashed supervisor left behind is applied, never lost.

use crate::record::{Chain, Entry, Identity, Kind, RecordError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The three scopes a halt names (section 3.2). Each is halted and resumed
/// independently; a halt with no scope halts all three.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    /// Launching any attempt: `run`, a retry, and an operation that launches.
    RunAdmission,
    /// Releasing a governed tool call during a live attempt.
    ToolEffects,
    /// Handing work to another process or host. Recorded; refuses nothing
    /// until a local dispatcher exists.
    Dispatch,
}

impl Scope {
    /// The word a scope is recorded and spelled as.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Scope::RunAdmission => "run-admission",
            Scope::ToolEffects => "tool-effects",
            Scope::Dispatch => "dispatch",
        }
    }

    /// Every scope, in the order section 3.2 lists them.
    #[must_use]
    pub fn all() -> [Scope; 3] {
        [Scope::RunAdmission, Scope::ToolEffects, Scope::Dispatch]
    }

    /// The scope a word names, if any.
    #[must_use]
    pub fn parse(word: &str) -> Option<Scope> {
        Scope::all().into_iter().find(|s| s.word() == word)
    }
}

/// The subject a halt entry carries.
pub const HALT_SUBJECT: &str = "halt";
/// The subject a resume entry carries.
pub const RESUME_SUBJECT: &str = "resume";
/// The subject the entry confirming a stop carries (section 3.4).
pub const STOP_CONFIRMED_SUBJECT: &str = "stop-confirmed";

/// The closed reason a halted scope refuses with.
pub const HALTED: &str = "halted";
/// The closed reason an unreadable or unverifiable halt record refuses with.
pub const UNREADABLE: &str = "halt-state-unreadable";

/// The attempt a halt asked to stop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StopTarget {
    /// The live run.
    pub run_id: String,
    /// Its live attempt.
    pub attempt: u32,
}

/// What a halt or resume entry records (section 3.1): the scopes, the
/// operator, a reason and the time. Decoded strictly, so an entry with a
/// field this product does not know is unreadable rather than half-read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HaltDetail {
    /// The scopes halted or lifted; never empty once recorded.
    pub scopes: Vec<Scope>,
    /// Who acted.
    pub operator: String,
    /// Why.
    pub reason: String,
    /// When, RFC 3339 UTC.
    pub at: String,
    /// The live attempt a halt asked to stop (section 3.4), on a halt only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_requested: Option<StopTarget>,
}

/// One standing halt of one scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Halted {
    /// The halted scope.
    pub scope: Scope,
    /// Who halted it.
    pub operator: String,
    /// Why.
    pub reason: String,
    /// When.
    pub at: String,
}

impl std::fmt::Display for Halted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} is halted for this repository by {} at {}: {}",
            self.scope.word(),
            self.operator,
            self.at,
            self.reason
        )
    }
}

/// A stop asked of a live attempt, and whether it is confirmed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stop {
    /// The run.
    pub run_id: String,
    /// The attempt.
    pub attempt: u32,
    /// Who asked.
    pub operator: String,
    /// Why.
    pub reason: String,
    /// When the stop was requested.
    pub requested_at: String,
    /// When the `cancelled` outcome was durable and the stop confirmed, or
    /// `None` while it is only requested.
    pub confirmed_at: Option<String>,
}

impl Stop {
    /// `stop-requested` or `stop-confirmed`.
    #[must_use]
    pub fn word(&self) -> &'static str {
        if self.confirmed_at.is_some() {
            "stop-confirmed"
        } else {
            "stop-requested"
        }
    }
}

/// The halt state of one repository, folded from its record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HaltState {
    /// Each scope currently halted, at most one entry per scope.
    pub halted: Vec<Halted>,
    /// Every stop asked of an attempt, oldest first.
    pub stops: Vec<Stop>,
}

impl HaltState {
    /// The standing halt of a scope, if it is halted.
    #[must_use]
    pub fn of(&self, scope: Scope) -> Option<&Halted> {
        self.halted.iter().find(|h| h.scope == scope)
    }

    /// The latest stop asked of an attempt, if any.
    #[must_use]
    pub fn stop_of(&self, run_id: &str, attempt: u32) -> Option<&Stop> {
        self.stops
            .iter()
            .rev()
            .find(|s| s.run_id == run_id && s.attempt == attempt)
    }
}

/// Why `run` refuses before launch (section 3.3).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    /// The run-admission scope is halted.
    #[error("halted: {0}")]
    Halted(Halted),
    /// The halt record could not be read or verified; nothing is admitted.
    #[error("halt-state-unreadable: {detail}")]
    Unreadable {
        /// What was found.
        detail: String,
    },
}

impl Refusal {
    /// The closed reason word.
    #[must_use]
    pub fn reason(&self) -> &'static str {
        match self {
            Refusal::Halted(_) => HALTED,
            Refusal::Unreadable { .. } => UNREADABLE,
        }
    }
}

impl From<RecordError> for Refusal {
    /// A chain that cannot be opened is a halt state that cannot be read.
    fn from(e: RecordError) -> Self {
        Refusal::Unreadable {
            detail: e.to_string(),
        }
    }
}

/// Fold the halt state from the chain, failing closed.
///
/// Reads the raw records, not [`Chain::entries`]: that decoder discards what
/// it cannot decode, and a halt entry that does not decode is the very thing
/// this fold must report. A torn tail is unreadable too, because the fold
/// cannot know what the torn bytes would have said (section 5, 2026-10-31).
pub fn fold(chain: &Chain) -> Result<HaltState, Refusal> {
    if let Some(t) = chain.torn_tail() {
        return Err(Refusal::Unreadable {
            detail: format!(
                "the record ends mid-entry: {} incomplete byte(s) at offset {}",
                t.bytes, t.offset
            ),
        });
    }
    let mut state = HaltState::default();
    for (position, record) in chain.records().iter().enumerate() {
        if record.payload.get("kind").and_then(|k| k.as_str()) != Some("halt") {
            continue;
        }
        let unreadable = |what: String| Refusal::Unreadable {
            detail: format!("halt entry at record {position} {what}"),
        };
        let entry: Entry = serde_json::from_value(record.payload.clone())
            .map_err(|e| unreadable(format!("does not parse: {e}")))?;
        match entry.subject.as_str() {
            HALT_SUBJECT | RESUME_SUBJECT => {
                let detail: HaltDetail = serde_json::from_value(entry.detail.clone())
                    .map_err(|e| unreadable(format!("does not parse: {e}")))?;
                if detail.scopes.is_empty() {
                    return Err(unreadable("names no scope".into()));
                }
                state.halted.retain(|h| !detail.scopes.contains(&h.scope));
                if entry.subject == HALT_SUBJECT {
                    for scope in &detail.scopes {
                        state.halted.push(Halted {
                            scope: *scope,
                            operator: detail.operator.clone(),
                            reason: detail.reason.clone(),
                            at: detail.at.clone(),
                        });
                    }
                    state.halted.sort_by_key(|h| h.scope);
                    if let Some(target) = detail.stop_requested {
                        state.stops.push(Stop {
                            run_id: target.run_id,
                            attempt: target.attempt,
                            operator: detail.operator,
                            reason: detail.reason,
                            requested_at: detail.at,
                            confirmed_at: None,
                        });
                    }
                }
            }
            STOP_CONFIRMED_SUBJECT => {
                let at = entry
                    .detail
                    .get("at")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| unreadable("confirms a stop without a time".into()))?;
                let stop = state
                    .stops
                    .iter_mut()
                    .rev()
                    .find(|s| s.run_id == entry.run_id && s.attempt == entry.attempt)
                    .ok_or_else(|| unreadable("confirms a stop nobody requested".into()))?;
                if stop.confirmed_at.is_none() {
                    stop.confirmed_at = Some(at.to_string());
                }
            }
            other => return Err(unreadable(format!("has the unknown subject `{other}`"))),
        }
    }
    Ok(state)
}

/// The pre-launch check (section 3.3): admit, or refuse with the closed reason.
pub fn admit(chain: &Chain) -> Result<(), Refusal> {
    let state = fold(chain)?;
    match state.of(Scope::RunAdmission) {
        Some(halted) => Err(Refusal::Halted(halted.clone())),
        None => Ok(()),
    }
}

/// What an operator asks of `halt` or `resume`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request<'a> {
    /// The scopes named; empty means all three (section 3.2).
    pub scopes: &'a [Scope],
    /// Who.
    pub operator: &'a str,
    /// Why.
    pub reason: &'a str,
    /// When, RFC 3339 UTC.
    pub at: &'a str,
}

/// Why a halt or resume was not recorded.
#[derive(Debug, thiserror::Error)]
pub enum HaltError {
    /// The record as it stands cannot be read, so nothing is appended to it.
    #[error(transparent)]
    Unreadable(#[from] Refusal),
    /// The operator or the reason is empty.
    #[error("refused: {0}")]
    Refused(String),
    /// The record could not be written.
    #[error(transparent)]
    Record(#[from] RecordError),
    /// A request could not be queued, or an applied one could not be removed.
    #[error("the halt request {path} could not be {what}: {source}")]
    Queue {
        /// The request file, or its directory.
        path: String,
        /// `queued` or `removed after it was appended`.
        what: &'static str,
        /// Why.
        source: std::io::Error,
    },
}

/// What a halt or resume did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    /// `halt` or `resume`.
    pub change: String,
    /// The entry as recorded.
    pub detail: HaltDetail,
    /// The halt state after it.
    pub state: HaltState,
}

fn scopes_of(request: &Request<'_>) -> Vec<Scope> {
    let mut scopes: Vec<Scope> = if request.scopes.is_empty() {
        Scope::all().to_vec()
    } else {
        request.scopes.to_vec()
    };
    scopes.sort();
    scopes.dedup();
    scopes
}

fn check(request: &Request<'_>) -> Result<(), HaltError> {
    if request.operator.trim().is_empty() {
        return Err(HaltError::Refused("an operator must be named".into()));
    }
    if request.reason.trim().is_empty() {
        return Err(HaltError::Refused("a reason must be given".into()));
    }
    Ok(())
}

fn append(
    chain: &mut Chain,
    subject: &str,
    target: Option<&StopTarget>,
    detail: &HaltDetail,
) -> Result<(), RecordError> {
    let key = format!("{subject}/{}", chain.records().len());
    chain.append(
        &key,
        &detail.at,
        &Entry {
            kind: Kind::Halt,
            run_id: target.map(|t| t.run_id.clone()).unwrap_or_default(),
            attempt: target.map_or(0, |t| t.attempt),
            subject: subject.to_string(),
            idempotency_key: None,
            detail: serde_json::to_value(detail).unwrap_or(serde_json::Value::Null),
            effect_id: Identity::Absent,
        },
    )
}

/// Halt the named scopes, or all three (sections 3.1, 3.2 and 3.4).
///
/// Halting `run-admission` or `tool-effects` while an attempt is live records
/// `stop-requested` for that attempt. The caller holds the repository lock.
pub fn halt(chain: &mut Chain, request: &Request<'_>) -> Result<Applied, HaltError> {
    check(request)?;
    fold(chain)?;
    let scopes = scopes_of(request);
    let stop_requested = if scopes
        .iter()
        .any(|s| matches!(s, Scope::RunAdmission | Scope::ToolEffects))
    {
        crate::session::live_attempt(chain).map(|a| StopTarget {
            run_id: a.run_id,
            attempt: a.number,
        })
    } else {
        None
    };
    let detail = HaltDetail {
        scopes,
        operator: request.operator.to_string(),
        reason: request.reason.to_string(),
        at: request.at.to_string(),
        stop_requested,
    };
    append(chain, HALT_SUBJECT, detail.stop_requested.as_ref(), &detail)?;
    Ok(Applied {
        change: HALT_SUBJECT.to_string(),
        state: fold(chain)?,
        detail,
    })
}

/// Lift the named scopes, or all three (section 3.5). Restarts nothing.
pub fn resume(chain: &mut Chain, request: &Request<'_>) -> Result<Applied, HaltError> {
    check(request)?;
    fold(chain)?;
    let detail = HaltDetail {
        scopes: scopes_of(request),
        operator: request.operator.to_string(),
        reason: request.reason.to_string(),
        at: request.at.to_string(),
        stop_requested: None,
    };
    append(chain, RESUME_SUBJECT, None, &detail)?;
    Ok(Applied {
        change: RESUME_SUBJECT.to_string(),
        state: fold(chain)?,
        detail,
    })
}

/// Record `stop-confirmed` for an attempt whose `cancelled` outcome is now
/// durable, where a stop was requested of it and not yet confirmed (section
/// 3.4). Called by the supervisor after the outcome record; a record that
/// cannot be folded confirms nothing, and says so.
pub fn confirm_stop(
    chain: &mut Chain,
    run_id: &str,
    attempt: u32,
    at: &str,
) -> Result<bool, HaltError> {
    let state = fold(chain)?;
    let Some(stop) = state.stop_of(run_id, attempt) else {
        return Ok(false);
    };
    if stop.confirmed_at.is_some() {
        return Ok(false);
    }
    let key = format!("{STOP_CONFIRMED_SUBJECT}/{}", chain.records().len());
    chain.append(
        &key,
        at,
        &Entry {
            kind: Kind::Halt,
            run_id: run_id.to_string(),
            attempt,
            subject: STOP_CONFIRMED_SUBJECT.to_string(),
            idempotency_key: None,
            detail: serde_json::json!({ "at": at }),
            effect_id: Identity::Absent,
        },
    )?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// Requests queued for a live supervisor (section 5, 2026-11-01).
// ---------------------------------------------------------------------------

/// Where halt and resume requests wait for the supervisor holding the lock:
/// beside the chain, in the product home's run records, keyed as the chain is,
/// so outside any target and outside the confinement (section 3.1).
#[must_use]
pub fn requests_dir(home: &Path, target: &Path) -> PathBuf {
    crate::record::chain_path(home, target).with_extension("halt-requests")
}

/// One queued request: what `halt` or `resume` would have appended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Queued {
    /// `halt` or `resume`.
    pub change: String,
    /// The scopes, resolved: never empty.
    pub scopes: Vec<Scope>,
    /// Who asked.
    pub operator: String,
    /// Why.
    pub reason: String,
    /// When the operator asked, RFC 3339 UTC; the entry carries this time.
    pub at: String,
}

fn queue_error(path: &Path, what: &'static str) -> impl FnOnce(std::io::Error) -> HaltError {
    let path = path.display().to_string();
    move |source| HaltError::Queue { path, what, source }
}

fn sync_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::File::open(dir)?.sync_all()
}

/// Queue a halt or resume for the supervisor holding the lock. One file per
/// request, named so that names sort in the order requests were made, and
/// published without replacing anything: written and synced under a hidden
/// name, then linked to its final name, which fails rather than replaces.
/// Returns the request's path once it and its directory entry are durable.
pub fn queue(
    home: &Path,
    target: &Path,
    change: &str,
    request: &Request<'_>,
) -> Result<PathBuf, HaltError> {
    check(request)?;
    if change != HALT_SUBJECT && change != RESUME_SUBJECT {
        return Err(HaltError::Refused(format!(
            "`{change}` is not a halt change"
        )));
    }
    let queued = Queued {
        change: change.to_string(),
        scopes: scopes_of(request),
        operator: request.operator.to_string(),
        reason: request.reason.to_string(),
        at: request.at.to_string(),
    };
    let mut bytes = serde_json::to_vec(&queued).unwrap_or_default();
    bytes.push(b'\n');
    publish_request(&requests_dir(home, target), &bytes)
        .map_err(|(path, e)| queue_error(&path, "queued")(e))
}

/// Publish one request file in `dir`, named so that names sort in the order
/// requests were made, without replacing anything: written and synced under a
/// hidden name, then linked to its final name, which fails rather than
/// replaces. Returns its path once it and its directory entry are durable, or
/// the path that failed and why. Shared by the halt queue, spec 048's and
/// spec 053's.
pub(crate) fn publish_request(
    dir: &Path,
    bytes: &[u8],
) -> Result<PathBuf, (PathBuf, std::io::Error)> {
    use std::io::Write;
    std::fs::create_dir_all(dir).map_err(|e| (dir.to_path_buf(), e))?;
    if let Some(parent) = dir.parent() {
        sync_dir(parent).map_err(|e| (dir.to_path_buf(), e))?;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let pid = std::process::id();
    let hidden = dir.join(format!(".{nanos:024}-{pid}.partial"));
    let written = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&hidden)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&hidden);
        return Err((hidden, e));
    }
    let mut published = None;
    for n in 0..64u32 {
        let name = dir.join(format!("{nanos:024}-{pid}-{n:02}.json"));
        match std::fs::hard_link(&hidden, &name) {
            Ok(()) => {
                published = Some(name);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                let _ = std::fs::remove_file(&hidden);
                return Err((name, e));
            }
        }
    }
    let _ = std::fs::remove_file(&hidden);
    let Some(path) = published else {
        return Err((
            dir.to_path_buf(),
            std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "every candidate name is taken",
            ),
        ));
    };
    sync_dir(dir).map_err(|e| (dir.to_path_buf(), e))?;
    Ok(path)
}

/// The request files in `dir`, oldest first; none when it does not exist.
/// Hidden names are a request still being written, and are not yet requests.
pub(crate) fn listed_requests(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        paths.push(entry.path());
    }
    paths.sort();
    Ok(paths)
}

/// Remove an applied request and make the removal durable.
pub(crate) fn remove_request(path: &Path) -> std::io::Result<()> {
    std::fs::remove_file(path)?;
    if let Some(dir) = path.parent() {
        sync_dir(dir)?;
    }
    Ok(())
}

/// Every queued request, oldest first. A request that cannot be read is
/// `halt-state-unreadable`, like an entry that does not parse: it might be a
/// halt, so nothing reads it as absent. Hidden names are a request still
/// being written, and are not yet requests.
pub fn pending(home: &Path, target: &Path) -> Result<Vec<(PathBuf, Queued)>, Refusal> {
    let dir = requests_dir(home, target);
    let paths = listed_requests(&dir).map_err(|e| Refusal::Unreadable {
        detail: format!("the halt requests at {} cannot be read: {e}", dir.display()),
    })?;
    let mut out = Vec::new();
    for path in paths {
        let queued = std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| serde_json::from_slice::<Queued>(&bytes).map_err(|e| e.to_string()))
            .and_then(|q| {
                if q.change != HALT_SUBJECT && q.change != RESUME_SUBJECT {
                    Err(format!("the unknown change `{}`", q.change))
                } else if q.scopes.is_empty() {
                    Err("no scope".to_string())
                } else {
                    Ok(q)
                }
            })
            .map_err(|why| Refusal::Unreadable {
                detail: format!("the halt request {} does not read: {why}", path.display()),
            })?;
        out.push((path, queued));
    }
    Ok(out)
}

/// Whether the record's last halt or resume entry is `change` with this
/// request's scopes, operator, reason and time.
fn last_change_is(chain: &Chain, change: &str, request: &Request<'_>) -> bool {
    let scopes = scopes_of(request);
    for record in chain.records().iter().rev() {
        if record.payload.get("kind").and_then(|k| k.as_str()) != Some("halt") {
            continue;
        }
        let Ok(entry) = serde_json::from_value::<Entry>(record.payload.clone()) else {
            // fold, which every writer runs first, refuses an unreadable one.
            return false;
        };
        if entry.subject != HALT_SUBJECT && entry.subject != RESUME_SUBJECT {
            continue;
        }
        let Ok(detail) = serde_json::from_value::<HaltDetail>(entry.detail) else {
            return false;
        };
        return entry.subject == change
            && detail.scopes == scopes
            && detail.operator == request.operator
            && detail.reason == request.reason
            && detail.at == request.at;
    }
    false
}

/// Append every queued request, oldest first, as the entry it asked for, and
/// remove each file only once its entry is durable. The caller holds the
/// repository lock: the supervisor of a live attempt, or `halt`, `resume`,
/// `run` or `run reconcile` before its own write. A request that does not
/// read refuses the whole call and is left in place, appending nothing.
pub fn apply_pending(
    chain: &mut Chain,
    home: &Path,
    target: &Path,
) -> Result<Vec<Applied>, HaltError> {
    let queued = pending(home, target)?;
    let mut applied = Vec::new();
    for (path, q) in queued {
        let request = Request {
            scopes: &q.scopes,
            operator: &q.operator,
            reason: &q.reason,
            at: &q.at,
        };
        // A request whose entry is already the record's last halt or resume
        // was appended by a writer that could not then remove its file. Every
        // writer applies queued requests in order before its own, so such a
        // file is always the oldest queued, and appending it again would
        // replay it over what followed. It is only removed.
        if !last_change_is(chain, &q.change, &request) {
            applied.push(if q.change == HALT_SUBJECT {
                halt(chain, &request)?
            } else {
                resume(chain, &request)?
            });
        }
        remove_request(&path).map_err(queue_error(&path, "removed after it was appended"))?;
    }
    Ok(applied)
}
