//! A run attempt's startup records, and the harness revision that answered it.
//!
//! Spec 002 section 3.31, settled by the owner on 2026-09-22.
//!
//! # Two records per attempt, each written once
//!
//! [`prepare`] writes `intent.json` after every preflight has passed and
//! **before** the process is created. [`finalize`] writes `record.json`, the
//! section 3.26 record, after the process ended or the launch failed, naming
//! the digest of the intent bytes it finalizes. Both refuse to replace a file
//! already present under the attempt's identity, so one attempt's evidence is
//! never overwritten and never adopted by another. If this product's own
//! process ends between the two, the intent stays and nothing fabricates the
//! record: [`inspect`] reads that as launched and interrupted.
//!
//! # Selected is not observed
//!
//! The run **selects** the required revision once the standing has shown it is
//! installed and intact, and carries the selection, with the attempt binding,
//! in the environment it constructs for the process. What **answered** is read
//! from the shipped `SessionStart` hook's acknowledgment in the attempt's own
//! stream, judged by [`observe`] against the intent. An acknowledgment that does
//! not bind is `unverified`, by kind, and an unverified observation leaves the
//! resolved identity absent: absent is not a match.
//!
//! # What this module does not claim
//!
//! The acknowledgment is launcher-attested. It establishes that a hook script
//! in the named revision directory ran inside this attempt's session with this
//! attempt's binding. It does not establish that any other file of that
//! revision was loaded, that a model read anything, or who printed the line:
//! the nonce is in the session's environment and nothing signs it.

use crate::delivery::Delivery;
use crate::home::Layout;
use crate::required::Standing;
use crate::startup::{
    AdapterIdentity, FileIdentity, Observation, ProjectIdentity, StartupRecord, Supply,
};
use serde::{Deserialize, Serialize};
use statecraft_adapter_claude_code::execution::HookResponse;
use statecraft_environment::digest::digest_bytes;
use statecraft_environment::manifest::Manifest;
use std::path::{Path, PathBuf};

/// The schema version of both attempt records.
pub const LAUNCH_VERSION: u32 = 2;

/// Where attempt records live, under the project's runtime state.
pub const DIRECTORY: &str = "startup/runs";

/// The acknowledgment line's first field.
pub const ACKNOWLEDGMENT: &str = "statecraft-startup";

/// The acknowledgment format this build reads.
pub const ACKNOWLEDGMENT_VERSION: &str = "v1";

/// The environment names that carry the attempt binding (section 3.31 rule 17).
pub const ENV_RUN: &str = "STATECRAFT_RUN_ID";
/// The attempt number.
pub const ENV_ATTEMPT: &str = "STATECRAFT_ATTEMPT";
/// The binding nonce.
pub const ENV_NONCE: &str = "STATECRAFT_STARTUP_NONCE";
/// The selected revision's full digest, or [`NONE_SELECTED`].
pub const ENV_SELECTED: &str = "STATECRAFT_HARNESS_SELECTED";

/// What the selection variable carries when nothing was selected.
pub const NONE_SELECTED: &str = "none";

/// The guard a mismatch observed after launch is counted under (rule 19).
pub const HARNESS_IDENTITY_GUARD: &str = "harness-identity";

/// The guard an unwritable intent refuses the attempt under (rule 15).
pub const STARTUP_RECORD_GUARD: &str = "startup-record";

/// Which attempt, of which run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptIdentity {
    /// The run id spec 003 assigned.
    pub run_id: String,
    /// The attempt number within it, from 1.
    pub attempt: u32,
}

impl AttemptIdentity {
    /// The directory an attempt's records live in.
    pub fn directory(&self, root: &Path) -> PathBuf {
        statecraft_environment::claimant::resolve(
            root,
            &format!(
                "{}/{DIRECTORY}/{}/{}",
                crate::project::STATE,
                self.run_id,
                self.attempt
            ),
        )
    }

    /// Where the intent is.
    pub fn intent_path(&self, root: &Path) -> PathBuf {
        self.directory(root).join("intent.json")
    }

    /// Where the record is.
    pub fn record_path(&self, root: &Path) -> PathBuf {
        self.directory(root).join("record.json")
    }

    /// Why this identity cannot name a directory, if it cannot.
    fn invalid(&self) -> Option<String> {
        let ok = !self.run_id.is_empty()
            && self.run_id != "."
            && self.run_id != ".."
            && self
                .run_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if !ok {
            return Some(format!(
                "the run id {:?} cannot name a record directory; it must be letters, digits, \
                 `-`, `_` and `.`",
                self.run_id
            ));
        }
        (self.attempt == 0).then(|| "attempt numbers start at 1".to_string())
    }
}

/// The revision a run selected, by full digest and by where it is installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Selected {
    /// The full digest: the integrity proof.
    pub digest: String,
    /// The display identifier, for a report only.
    pub display: String,
    /// The installed directory.
    pub root: String,
}

/// The payload a session is given, by identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadIdentity {
    /// SHA-256 of the exact bytes.
    pub digest: String,
    /// Their length.
    pub bytes: u64,
    /// The argument that carries them.
    pub argument: String,
}

impl PayloadIdentity {
    /// This build's payload.
    pub fn of_this_build() -> Self {
        let bytes = crate::session::payload_json();
        Self {
            digest: digest_bytes(bytes.as_bytes()),
            bytes: bytes.len() as u64,
            argument: crate::session::SETTINGS_ARGUMENT.to_string(),
        }
    }
}

/// What a run records before it creates the process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Intent {
    /// Schema version.
    pub version: u32,
    /// Which attempt.
    pub attempt: AttemptIdentity,
    /// When, RFC 3339 UTC.
    pub recorded_at: String,
    /// The project, by its manifest.
    pub project: ProjectIdentity,
    /// The workspace the session starts in, canonical.
    pub workspace: String,
    /// The commit the workspace was prepared at.
    pub base_commit: String,
    /// The section 3.14 verdict, evaluated in the workspace.
    pub delivery: Delivery,
    /// The files the chain traverses in the workspace, entry first.
    pub load_chain: Vec<String>,
    /// Their identities, read from the workspace immediately before the spawn.
    pub instructions: Vec<FileIdentity>,
    /// The committed requirement, full digest.
    pub required_harness: Option<String>,
    /// The standing before launch, with nothing resolved.
    pub standing: Standing,
    /// The revision this run selected, where it selected one.
    pub selected: Option<Selected>,
    /// The adapter performing the delivery.
    pub adapter: AdapterIdentity,
    /// The program about to be launched, resolved.
    pub program: String,
    /// The payload the session is to be given.
    pub payload: PayloadIdentity,
    /// The binding nonce, fresh for this attempt.
    pub nonce: String,
}

impl Intent {
    /// The environment that carries this attempt's binding to the process.
    pub fn environment(&self) -> Vec<(String, String)> {
        vec![
            (ENV_RUN.to_string(), self.attempt.run_id.clone()),
            (ENV_ATTEMPT.to_string(), self.attempt.attempt.to_string()),
            (ENV_NONCE.to_string(), self.nonce.clone()),
            (ENV_SELECTED.to_string(), self.selected_word().to_string()),
        ]
    }

    fn selected_word(&self) -> &str {
        self.selected
            .as_ref()
            .map_or(NONE_SELECTED, |s| s.digest.as_str())
    }
}

/// What a run needs to prepare an attempt's intent.
#[derive(Debug, Clone)]
pub struct Preparation<'a> {
    /// The project root, which holds the manifest and the runtime state.
    pub root: &'a Path,
    /// The workspace the session starts in.
    pub workspace: &'a Path,
    /// The commit it was prepared at.
    pub base_commit: &'a str,
    /// Which attempt.
    pub attempt: AttemptIdentity,
    /// When, RFC 3339 UTC.
    pub recorded_at: &'a str,
    /// The product home.
    pub layout: &'a Layout,
    /// The project's manifest, as read for this run.
    pub manifest: &'a Manifest,
    /// The adapter performing the delivery.
    pub adapter: AdapterIdentity,
    /// The resolved program.
    pub program: &'a str,
}

/// An intent written to disk, with the digest of the bytes written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    /// The intent.
    pub intent: Intent,
    /// Where it was written.
    pub path: PathBuf,
    /// SHA-256 of the bytes written.
    pub digest: String,
}

/// Why an intent was not written, and nothing may be launched.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotPrepared {
    /// The standing refuses a run. Section 3.25; the caller should have
    /// refused before the attempt, and this is the library saying so again.
    #[error("the harness requirement refuses a managed run: {0}")]
    Standing(String),
    /// The attempt identity cannot name a directory.
    #[error("{0}")]
    Identity(String),
    /// The attempt already has an intent. Its evidence is not replaced.
    #[error(
        "attempt {attempt} of run {run} already has startup evidence at {path}; an attempt's \
         evidence is written once and is not replaced"
    )]
    Exists {
        /// The run.
        run: String,
        /// The attempt.
        attempt: u32,
        /// The file.
        path: String,
    },
    /// Anything else that stopped the write.
    #[error("the startup intent could not be recorded: {0}")]
    Io(String),
}

/// Write an attempt's intent. Nothing may be launched unless this succeeds.
pub fn prepare(p: &Preparation<'_>) -> Result<Prepared, NotPrepared> {
    if let Some(why) = p.attempt.invalid() {
        return Err(NotPrepared::Identity(why));
    }
    let standing = crate::required::evaluate(p.layout, p.manifest, None);
    if let Some(why) = standing.refuses_a_run() {
        return Err(NotPrepared::Standing(why));
    }
    let io = |e: std::io::Error| NotPrepared::Io(e.to_string());
    let intent_path = p.attempt.intent_path(p.root);
    for existing in [&intent_path, &p.attempt.record_path(p.root)] {
        if existing.exists() {
            return Err(NotPrepared::Exists {
                run: p.attempt.run_id.clone(),
                attempt: p.attempt.attempt,
                path: existing.display().to_string(),
            });
        }
    }

    let project = ProjectIdentity::read(p.root).map_err(io)?.ok_or_else(|| {
        NotPrepared::Io(format!(
            "{} holds no {}",
            p.root.display(),
            statecraft_environment::manifest::MANIFEST_PATH
        ))
    })?;
    let workspace = std::fs::canonicalize(p.workspace).map_err(io)?;
    let rule = crate::delivery::load_rules()
        .into_iter()
        .find(|r| r.harness == crate::session::SUPPORTED_HARNESS)
        .ok_or_else(|| NotPrepared::Io("no documented load rule for the harness".into()))?;
    let delivery = crate::delivery::evaluate(&workspace, &rule);
    let load_chain = match &delivery {
        Delivery::Reached { via } => via.clone(),
        _ => Vec::new(),
    };
    let mut instructions = Vec::new();
    for rel in &load_chain {
        match FileIdentity::read(&workspace, rel).map_err(io)? {
            Some(identity) => instructions.push(identity),
            None => {
                return Err(NotPrepared::Io(format!(
                    "the load chain names {rel} and the workspace does not hold it"
                )));
            }
        }
    }
    let selected = match &standing {
        Standing::Exact { required, .. } => {
            let display = crate::harness::display_id(required);
            Some(Selected {
                digest: required.clone(),
                root: p
                    .layout
                    .harness_revision_dir(&display)
                    .display()
                    .to_string(),
                display,
            })
        }
        // Unrequired selects nothing. Every other standing refused above.
        _ => None,
    };

    let intent = Intent {
        version: LAUNCH_VERSION,
        attempt: p.attempt.clone(),
        recorded_at: p.recorded_at.to_string(),
        project,
        workspace: workspace.display().to_string(),
        base_commit: p.base_commit.to_string(),
        delivery,
        load_chain,
        instructions,
        required_harness: crate::required::required_of(p.manifest).map(str::to_string),
        standing,
        selected,
        adapter: p.adapter.clone(),
        program: p.program.to_string(),
        payload: PayloadIdentity::of_this_build(),
        nonce: nonce().map_err(io)?,
    };
    let mut json =
        serde_json::to_string_pretty(&intent).map_err(|e| NotPrepared::Io(e.to_string()))?;
    json.push('\n');
    write_once(&intent_path, &json).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => NotPrepared::Exists {
            run: p.attempt.run_id.clone(),
            attempt: p.attempt.attempt,
            path: intent_path.display().to_string(),
        },
        _ => NotPrepared::Io(e.to_string()),
    })?;
    Ok(Prepared {
        digest: digest_bytes(json.as_bytes()),
        intent,
        path: intent_path,
    })
}

/// Sixteen bytes from the operating system's generator, as hex.
fn nonce() -> std::io::Result<String> {
    use std::io::Read;
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Write a file that must not already exist, through a temporary file and a
/// rename, so a reader never sees half of it.
fn write_once(path: &Path, contents: &str) -> std::io::Result<()> {
    if path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("{} already exists", path.display()),
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    crate::settings::write_atomically(path, contents)
}

// ---------------------------------------------------------------------------
// The observation.
// ---------------------------------------------------------------------------

/// Why an acknowledgment did not establish which revision answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Unverified {
    /// No acknowledgment in any `SessionStart` response.
    Absent,
    /// A line that begins the acknowledgment and does not parse.
    Malformed,
    /// Another attempt's nonce.
    Replayed,
    /// Another run or attempt number, or another selection.
    WrongAttempt,
    /// Another project directory.
    WrongProject,
    /// A session id that is not the stream's own.
    WrongSession,
    /// A hook that exited non-zero.
    HookFailed,
    /// Two acknowledgments naming different directories.
    Conflicting,
    /// A revision directory not directly inside this home's harness store.
    ForeignRevision,
    /// A revision directory that cannot be read and digested.
    UnreadableRevision,
    /// The launch failed, so there was no session to acknowledge anything.
    NotLaunched,
}

impl Unverified {
    /// The word section 3.31's table uses.
    pub fn word(self) -> &'static str {
        match self {
            Unverified::Absent => "absent",
            Unverified::Malformed => "malformed",
            Unverified::Replayed => "replayed",
            Unverified::WrongAttempt => "wrong-attempt",
            Unverified::WrongProject => "wrong-project",
            Unverified::WrongSession => "wrong-session",
            Unverified::HookFailed => "hook-failed",
            Unverified::Conflicting => "conflicting",
            Unverified::ForeignRevision => "foreign-revision",
            Unverified::UnreadableRevision => "unreadable-revision",
            Unverified::NotLaunched => "not-launched",
        }
    }
}

/// Which revision answered this attempt, and the grade of that evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "grade")]
pub enum HarnessObservation {
    /// A runtime acknowledgment bound to this attempt named this revision.
    Acknowledged {
        /// The revision directory the hook ran from.
        root: String,
        /// Its display identifier.
        display: String,
        /// The full digest its files have now: the observed identity.
        digest: String,
        /// The session the acknowledgment arrived in.
        session_id: String,
        /// The hook's name as the provider reported it.
        hook_name: Option<String>,
        /// The nonce it echoed, which is this attempt's.
        nonce: String,
    },
    /// Nothing established which revision answered.
    Unverified {
        /// Which kind of failure.
        kind: Unverified,
        /// What was seen.
        detail: String,
    },
}

impl HarnessObservation {
    /// The observed full digest, where one was admitted.
    pub fn digest(&self) -> Option<&str> {
        match self {
            HarnessObservation::Acknowledged { digest, .. } => Some(digest),
            HarnessObservation::Unverified { .. } => None,
        }
    }

    /// One line.
    pub fn describe(&self) -> String {
        match self {
            HarnessObservation::Acknowledged {
                display,
                session_id,
                ..
            } => {
                format!("{display}, acknowledged by the SessionStart hook in session {session_id}")
            }
            HarnessObservation::Unverified { kind, detail } => {
                format!("unverified: {}: {detail}", kind.word())
            }
        }
    }
}

fn unverified(kind: Unverified, detail: impl Into<String>) -> HarnessObservation {
    HarnessObservation::Unverified {
        kind,
        detail: detail.into(),
    }
}

/// One acknowledgment line's fields.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Acknowledgment {
    nonce: String,
    run: String,
    attempt: String,
    selected: String,
    project: String,
    root: String,
}

/// Parse one line that begins the acknowledgment. `None` is malformed.
fn parse(line: &str) -> Option<Acknowledgment> {
    let fields: Vec<&str> = line.split('\t').collect();
    let [prefix, version, rest @ ..] = fields.as_slice() else {
        return None;
    };
    if *prefix != ACKNOWLEDGMENT || *version != ACKNOWLEDGMENT_VERSION || rest.len() != 6 {
        return None;
    }
    let mut values = Vec::new();
    for (field, key) in rest
        .iter()
        .zip(["nonce", "run", "attempt", "selected", "project", "root"])
    {
        values.push(field.strip_prefix(key)?.strip_prefix('=')?.to_string());
    }
    let [nonce, run, attempt, selected, project, root] = values.try_into().ok()?;
    Some(Acknowledgment {
        nonce,
        run,
        attempt,
        selected,
        project,
        root,
    })
}

/// Judge the acknowledgments an attempt's stream carried, against its intent.
///
/// `session_id` is the stream's own init session. Section 3.31 rule 18: exactly
/// one binding acknowledgment admits the revision; anything else is unverified
/// and says which way.
pub fn observe(
    intent: &Intent,
    responses: &[HookResponse],
    session_id: Option<&str>,
    layout: &Layout,
) -> HarnessObservation {
    let mut found: Vec<(&HookResponse, &str)> = Vec::new();
    for response in responses {
        let is_session_start = response.hook_event.as_deref() == Some("SessionStart");
        for line in response.stdout.as_deref().unwrap_or_default().lines() {
            if line.starts_with(ACKNOWLEDGMENT) {
                if !is_session_start {
                    return unverified(
                        Unverified::Malformed,
                        format!(
                            "an acknowledgment arrived from the {} event, and only SessionStart \
                             acknowledges a start",
                            response.hook_event.as_deref().unwrap_or("unnamed")
                        ),
                    );
                }
                found.push((response, line));
            }
        }
    }
    if found.is_empty() {
        return unverified(
            Unverified::Absent,
            format!(
                "no SessionStart response in this attempt's stream carried an acknowledgment \
                 ({} hook response(s) read)",
                responses.len()
            ),
        );
    }

    let mut roots = Vec::new();
    for (response, line) in &found {
        let Some(ack) = parse(line) else {
            return unverified(Unverified::Malformed, format!("{line:?} does not parse"));
        };
        if response.exit_code != Some(0) {
            return unverified(
                Unverified::HookFailed,
                format!("the hook exited {:?}", response.exit_code),
            );
        }
        match (response.session_id.as_deref(), session_id) {
            (Some(a), Some(b)) if a == b => {}
            (a, b) => {
                return unverified(
                    Unverified::WrongSession,
                    format!("the acknowledgment names session {a:?} and the stream's is {b:?}"),
                );
            }
        }
        if ack.nonce != intent.nonce {
            return unverified(
                Unverified::Replayed,
                "the acknowledgment carries a nonce this attempt did not issue",
            );
        }
        if ack.run != intent.attempt.run_id
            || ack.attempt != intent.attempt.attempt.to_string()
            || ack.selected != intent.selected_word()
        {
            return unverified(
                Unverified::WrongAttempt,
                format!(
                    "the acknowledgment names run {:?} attempt {:?} selected {:?}",
                    ack.run, ack.attempt, ack.selected
                ),
            );
        }
        if ack.project != intent.workspace {
            return unverified(
                Unverified::WrongProject,
                format!(
                    "the acknowledgment ran in {} and the session started in {}",
                    ack.project, intent.workspace
                ),
            );
        }
        roots.push((ack, response.hook_name.clone()));
    }
    let first = roots[0].0.root.clone();
    if roots.iter().any(|(a, _)| a.root != first) {
        return unverified(
            Unverified::Conflicting,
            format!(
                "acknowledgments name {}",
                roots
                    .iter()
                    .map(|(a, _)| a.root.as_str())
                    .collect::<Vec<_>>()
                    .join(" and ")
            ),
        );
    }

    let root = PathBuf::from(&first);
    let store = std::fs::canonicalize(layout.harness_dir()).ok();
    let (Some(store), Some(parent), Some(name)) = (
        store,
        root.parent(),
        root.file_name().and_then(|n| n.to_str()),
    ) else {
        return unverified(
            Unverified::ForeignRevision,
            format!("{first} is not a revision directory in this home's harness store"),
        );
    };
    if parent != store {
        return unverified(
            Unverified::ForeignRevision,
            format!(
                "{first} is not directly inside this home's harness store {}",
                store.display()
            ),
        );
    }
    let files = match crate::harness::read_installed(layout, name) {
        Ok(Some(files)) => files,
        Ok(None) => {
            return unverified(
                Unverified::UnreadableRevision,
                format!("{first} is not there any more"),
            );
        }
        Err(e) => {
            return unverified(
                Unverified::UnreadableRevision,
                format!("{first} could not be read: {e}"),
            );
        }
    };
    let revision = crate::harness::revision_of(&files);
    HarnessObservation::Acknowledged {
        root: first,
        display: crate::harness::display_id(&revision.digest),
        digest: revision.digest,
        session_id: session_id.unwrap_or_default().to_string(),
        hook_name: roots[0].1.clone(),
        nonce: intent.nonce.clone(),
    }
}

// ---------------------------------------------------------------------------
// The record.
// ---------------------------------------------------------------------------

/// How the process ended, as the supervisor decided it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessEnd {
    /// Whether a process was created.
    pub launched: bool,
    /// The observed outcome word, `completed`, `interrupted` and so on.
    pub outcome: Option<String>,
    /// A transport diagnostic, where there was one.
    pub stream_error: Option<String>,
    /// A survivor report, where there was one.
    pub surviving_processes: Option<String>,
    /// Why the launch failed, where it did.
    pub failure: Option<String>,
}

/// The settings bytes the adapter wrote, by identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Written {
    /// SHA-256 of the bytes.
    pub digest: String,
    /// Their length.
    pub bytes: u64,
}

/// Section 3.31's additions to a run attempt's section 3.26 record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchEvidence {
    /// SHA-256 of the `intent.json` bytes this record finalizes.
    pub intent_digest: String,
    /// The workspace the session started in.
    pub workspace: String,
    /// The revision selected before launch.
    pub selected: Option<Selected>,
    /// The payload the session was to be given.
    pub payload: PayloadIdentity,
    /// The settings bytes the adapter wrote, where it wrote any.
    pub settings_written: Option<Written>,
    /// The provider's own session id, from its init event.
    pub provider_session: Option<String>,
    /// The provider's version, from its init event.
    pub provider_version: Option<String>,
    /// How the process ended.
    pub process: ProcessEnd,
    /// Which revision answered, and the grade of that evidence.
    pub harness: HarnessObservation,
}

/// What the launch produced, as the run hands it over.
#[derive(Debug, Clone)]
pub enum Launch<'a> {
    /// The process was created and supervised.
    Spawned {
        /// Every hook response, in stream order.
        hook_responses: &'a [HookResponse],
        /// The init event's session id.
        session_id: Option<&'a str>,
        /// The init event's version.
        provider_version: Option<&'a str>,
        /// The exact settings bytes the adapter wrote.
        settings_written: &'a [u8],
        /// The observed outcome word.
        outcome: &'a str,
        /// A transport diagnostic.
        stream_error: Option<String>,
        /// A survivor report.
        surviving_processes: Option<String>,
    },
    /// The launch failed before a process could be supervised.
    Failed {
        /// Why.
        reason: String,
    },
}

/// Assemble and write an attempt's record. The only place `record.json` is
/// written, and it is written once.
pub fn finalize(
    root: &Path,
    layout: &Layout,
    manifest: &Manifest,
    prepared: &Prepared,
    launch: &Launch<'_>,
    recorded_at: &str,
) -> std::io::Result<(StartupRecord, PathBuf)> {
    let intent = &prepared.intent;
    let (harness, process, settings_written, provider_session, provider_version) = match launch {
        Launch::Failed { reason } => (
            unverified(Unverified::NotLaunched, reason.clone()),
            ProcessEnd {
                launched: false,
                outcome: None,
                stream_error: None,
                surviving_processes: None,
                failure: Some(reason.clone()),
            },
            None,
            None,
            None,
        ),
        Launch::Spawned {
            hook_responses,
            session_id,
            provider_version,
            settings_written,
            outcome,
            stream_error,
            surviving_processes,
        } => (
            observe(intent, hook_responses, *session_id, layout),
            ProcessEnd {
                launched: true,
                outcome: Some((*outcome).to_string()),
                stream_error: stream_error.clone(),
                surviving_processes: surviving_processes.clone(),
                failure: None,
            },
            Some(Written {
                digest: digest_bytes(settings_written),
                bytes: settings_written.len() as u64,
            }),
            session_id.map(str::to_string),
            provider_version.map(str::to_string),
        ),
    };

    // Rule 16: supply is what the launch handed over, and nothing else.
    let supply = match (&process.launched, &settings_written) {
        (false, _) => Supply::Failed {
            reason: format!(
                "the launch failed, so nothing was handed to a session: {}",
                process.failure.clone().unwrap_or_default()
            ),
            partial: Vec::new(),
        },
        (true, _) if intent.load_chain.is_empty() => Supply::NotAttempted {
            reason: "the documented load rule does not reach the managed file in the workspace, \
                     so no instruction chain was supplied"
                .to_string(),
        },
        (true, Some(w)) if w.digest != intent.payload.digest => Supply::Failed {
            reason: format!(
                "the settings file held bytes digesting to {} and the payload is {}",
                w.digest, intent.payload.digest
            ),
            partial: intent.instructions.clone(),
        },
        (true, Some(_)) => Supply::Supplied {
            files: intent.instructions.clone(),
        },
        (true, None) => Supply::Failed {
            reason: "the adapter reported no settings bytes".to_string(),
            partial: intent.instructions.clone(),
        },
    };

    let resolved = harness.digest().map(str::to_string);
    let standing = crate::required::evaluate(layout, manifest, resolved.as_deref());
    let record = StartupRecord {
        version: LAUNCH_VERSION,
        session_id: format!("{}/{}", intent.attempt.run_id, intent.attempt.attempt),
        recorded_at: recorded_at.to_string(),
        project: intent.project.clone(),
        instructions: intent.instructions.clone(),
        required_harness: intent.required_harness.clone(),
        resolved_harness: resolved,
        adapter: intent.adapter.clone(),
        load_chain: intent.load_chain.clone(),
        delivery: intent.delivery.clone(),
        standing,
        supply,
        observation: Observation::NotObserved {
            reason: RUN_OBSERVATION.to_string(),
        },
        attempt: Some(intent.attempt.clone()),
        launch: Some(Box::new(LaunchEvidence {
            intent_digest: prepared.digest.clone(),
            workspace: intent.workspace.clone(),
            selected: intent.selected.clone(),
            payload: intent.payload.clone(),
            settings_written,
            provider_session,
            provider_version,
            process,
            harness,
        })),
    };
    if let Some(why) = record.incomplete() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            why.to_string(),
        ));
    }
    let path = intent.attempt.record_path(root);
    let mut json = serde_json::to_string_pretty(&record)?;
    json.push('\n');
    write_once(&path, &json)?;
    Ok((record, path))
}

/// Why a run attempt's observation is always absent (rule 21).
pub const RUN_OBSERVATION: &str = "a run session is not one of section 3.29's three controls, and \
     rule 4 of that section refuses evidence for another invocation as evidence for this one";

/// Whether this record's harness evidence refuses the attempt (rule 19), and
/// why.
pub fn refuses_the_attempt(record: &StartupRecord) -> Option<String> {
    match &record.standing {
        Standing::Mismatched { .. } => record.standing.refusal(),
        _ => None,
    }
}

/// What `run` says about its attempt's startup evidence (spec 006 section
/// 3.11.3's additive `startup` field).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStartup {
    /// Whether this run was a managed session, one in a project holding the
    /// manifest, and so one that records startup evidence at all.
    pub managed: bool,
    /// Whether a process was created for the attempt.
    pub launched: bool,
    /// Where the intent is, when one was written.
    pub intent: Option<String>,
    /// Where the record is, when one was written.
    pub record: Option<String>,
    /// The verdict read back from what was written, when it could be read.
    pub verdict: Option<Verdict>,
    /// The observed harness, in one line.
    pub observed: Option<String>,
    /// Why evidence that should have been stored was not.
    pub error: Option<String>,
}

impl RunStartup {
    /// A run in a repository holding no manifest: not a managed session.
    pub fn unmanaged() -> Self {
        Self {
            managed: false,
            launched: false,
            intent: None,
            record: None,
            verdict: None,
            observed: None,
            error: None,
        }
    }

    /// Whether the run must report failure: launched, and the record that
    /// says what the launch established was not stored (rule 15).
    pub fn not_stored(&self) -> bool {
        self.launched && self.error.is_some()
    }

    /// The lines `run` prints.
    pub fn describe(&self) -> String {
        if !self.managed {
            return "  startup: not recorded, the repository holds no manifest, so this is not a \
                    managed session\n"
                .to_string();
        }
        let mut out = String::new();
        if let Some(verdict) = self.verdict {
            out.push_str(&format!("  startup: {}\n", verdict.word()));
        }
        if let Some(observed) = &self.observed {
            out.push_str(&format!("  harness observed: {observed}\n"));
        }
        if let Some(error) = &self.error {
            out.push_str(&format!("  startup evidence NOT STORED: {error}\n"));
        }
        for path in self.intent.iter().chain(self.record.iter()) {
            out.push_str(&format!("  evidence: {path}\n"));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// The judgement, read back.
// ---------------------------------------------------------------------------

/// What the run record says about an attempt, passed in by the caller that
/// reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptFact {
    /// The attempt number.
    pub number: u32,
    /// Its outcome word, absent while it is live.
    pub outcome: Option<String>,
}

/// A run attempt's verdict (rule 21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// No intent: no process was created for it.
    NotLaunched,
    /// Launched and not finalized, or finalized and the process did not end
    /// by itself as one readable session.
    Interrupted,
    /// An admitted acknowledgment named another revision than the required.
    Mismatched,
    /// Launched and recorded, and not qualified.
    Unverified,
    /// Every evidence class holds. Not reachable through a run.
    Qualified,
}

impl Verdict {
    /// The word.
    pub fn word(self) -> &'static str {
        match self {
            Verdict::NotLaunched => "not-launched",
            Verdict::Interrupted => "interrupted",
            Verdict::Mismatched => "mismatched",
            Verdict::Unverified => "unverified",
            Verdict::Qualified => "qualified",
        }
    }
}

/// One attempt's startup evidence, read back and judged again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptStartup {
    /// The project.
    pub root: String,
    /// Which attempt.
    pub attempt: AttemptIdentity,
    /// The attempt's outcome, as the run record has it.
    pub outcome: Option<String>,
    /// Where the intent is, and whether it is there.
    pub intent_path: String,
    /// Where the record is, and whether it is there.
    pub record_path: String,
    /// Whether this product was about to create the process.
    pub launched: bool,
    /// The intent, where there is one.
    pub intent: Option<Box<Intent>>,
    /// The record, where there is one.
    pub record: Option<Box<StartupRecord>>,
    /// The verdict.
    pub verdict: Verdict,
    /// Every reason for it, in order.
    pub reasons: Vec<String>,
}

impl AttemptStartup {
    /// Whether the verdict is `qualified`.
    pub fn qualified(&self) -> bool {
        self.verdict == Verdict::Qualified
    }

    /// The human rendering: the six things an operator asks, in order.
    pub fn describe(&self) -> String {
        let short = |d: &str| crate::harness::display_id(d);
        let mut out = format!(
            "run {} attempt {} in {}\n",
            self.attempt.run_id, self.attempt.attempt, self.root
        );
        out.push_str(&format!(
            "outcome   {}\n",
            self.outcome
                .as_deref()
                .unwrap_or("live, no outcome recorded")
        ));
        out.push_str(&format!(
            "launched  {}\n",
            if self.launched {
                "yes: an intent was recorded before the process was created"
            } else {
                "no: no intent was recorded, so no process was created for this attempt"
            }
        ));
        if let Some(intent) = &self.intent {
            out.push_str(&format!(
                "required  {}\n",
                intent.required_harness.as_deref().map_or(
                    "none: the project commits no requirement".to_string(),
                    |d| { format!("{} ({d})", short(d)) }
                )
            ));
            out.push_str(&format!(
                "selected  {}\n",
                intent
                    .selected
                    .as_ref()
                    .map_or("none".to_string(), |s| format!(
                        "{} at {}",
                        s.display, s.root
                    ))
            ));
            out.push_str(&format!(
                "payload   {} ({} bytes, via {})\n",
                intent.payload.digest, intent.payload.bytes, intent.payload.argument
            ));
        }
        match self.record.as_ref().and_then(|r| r.launch.as_ref()) {
            Some(launch) => {
                out.push_str(&format!("observed  {}\n", launch.harness.describe()));
                out.push_str(&format!(
                    "written   {}\n",
                    launch.settings_written.as_ref().map_or(
                        "no settings bytes were written".to_string(),
                        |w| format!("{} ({} bytes)", w.digest, w.bytes)
                    )
                ));
            }
            None => out.push_str("observed  nothing recorded\n"),
        }
        if let Some(record) = &self.record {
            out.push_str(&format!("supply    {}\n", record.supply.describe()));
            out.push_str(&format!("standing  {}\n", record.standing.describe()));
        }
        out.push_str(&format!(
            "evidence  {}{}\n          {}{}\n",
            self.intent_path,
            if self.intent.is_some() {
                ""
            } else {
                " (absent)"
            },
            self.record_path,
            if self.record.is_some() {
                ""
            } else {
                " (absent)"
            }
        ));
        out.push_str(&format!("verdict   {}\n", self.verdict.word()));
        for reason in &self.reasons {
            out.push_str(&format!("  - {reason}\n"));
        }
        out
    }
}

/// Why an attempt's startup evidence could not be read.
#[derive(Debug, thiserror::Error)]
pub enum NotRead {
    /// The run record has no such attempt.
    #[error("{0}")]
    NoSuchAttempt(String),
    /// A record is present and could not be read.
    #[error("{path} could not be read: {detail}")]
    Unreadable {
        /// Which file.
        path: String,
        /// Why.
        detail: String,
    },
}

/// Read an attempt's two records and judge them, from their bytes alone.
///
/// `facts` is every attempt the run record holds for the run. `attempt`
/// `None` means the latest. Nothing here writes.
pub fn inspect(
    root: &Path,
    run_id: &str,
    attempt: Option<u32>,
    facts: &[AttemptFact],
) -> Result<AttemptStartup, NotRead> {
    let fact = match attempt {
        Some(n) => facts.iter().find(|f| f.number == n),
        None => facts.iter().max_by_key(|f| f.number),
    }
    .ok_or_else(|| {
        NotRead::NoSuchAttempt(match attempt {
            Some(n) => format!("run {run_id} has no attempt {n}"),
            None => format!("run {run_id} has no attempt"),
        })
    })?;
    let identity = AttemptIdentity {
        run_id: run_id.to_string(),
        attempt: fact.number,
    };
    if let Some(why) = identity.invalid() {
        return Err(NotRead::NoSuchAttempt(why));
    }
    let intent_path = identity.intent_path(root);
    let record_path = identity.record_path(root);
    let read = |path: &Path| -> Result<Option<Vec<u8>>, NotRead> {
        match std::fs::read(path) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(NotRead::Unreadable {
                path: path.display().to_string(),
                detail: e.to_string(),
            }),
        }
    };
    let intent_bytes = read(&intent_path)?;
    let record_bytes = read(&record_path)?;
    let intent: Option<Intent> = match &intent_bytes {
        Some(b) => Some(serde_json::from_slice(b).map_err(|e| NotRead::Unreadable {
            path: intent_path.display().to_string(),
            detail: e.to_string(),
        })?),
        None => None,
    };
    let record: Option<StartupRecord> = match &record_bytes {
        Some(b) => Some(serde_json::from_slice(b).map_err(|e| NotRead::Unreadable {
            path: record_path.display().to_string(),
            detail: e.to_string(),
        })?),
        None => None,
    };

    let (verdict, reasons) = judge(
        &identity,
        intent.as_ref(),
        intent_bytes.as_deref(),
        record.as_ref(),
        fact.outcome.as_deref(),
    );
    Ok(AttemptStartup {
        root: root.display().to_string(),
        attempt: identity,
        outcome: fact.outcome.clone(),
        intent_path: intent_path.display().to_string(),
        record_path: record_path.display().to_string(),
        launched: intent.is_some(),
        intent: intent.map(Box::new),
        record: record.map(Box::new),
        verdict,
        reasons,
    })
}

/// The judgement, recomputed from the records every time they are read.
fn judge(
    identity: &AttemptIdentity,
    intent: Option<&Intent>,
    intent_bytes: Option<&[u8]>,
    record: Option<&StartupRecord>,
    outcome: Option<&str>,
) -> (Verdict, Vec<String>) {
    let (intent, intent_bytes) = match (intent, intent_bytes) {
        (Some(i), Some(b)) => (i, b),
        _ => {
            let mut reasons = vec![match outcome {
                Some(word) => format!(
                    "the attempt ended {word} and no intent was recorded, so it was concluded \
                     before a process was created"
                ),
                None => "no intent was recorded for this attempt".to_string(),
            }];
            if record.is_some() {
                reasons.push(
                    "a record is present without its intent, and a record read without its \
                     intent is not evidence of a launch"
                        .to_string(),
                );
            }
            return (Verdict::NotLaunched, reasons);
        }
    };
    let Some(record) = record else {
        return (
            Verdict::Interrupted,
            vec![
                "launched, and no record was written: this product's process ended, or the \
                 record could not be stored, before the attempt was finalized"
                    .to_string(),
            ],
        );
    };

    // The pair must agree with itself before anything in it is believed.
    let mut unbound = Vec::new();
    if intent.attempt != *identity {
        unbound.push(format!(
            "the intent names run {} attempt {}",
            intent.attempt.run_id, intent.attempt.attempt
        ));
    }
    if record.attempt.as_ref() != Some(identity) {
        unbound.push("the record names another attempt, or none".to_string());
    }
    let Some(launch) = record.launch.as_ref() else {
        return (
            Verdict::Unverified,
            vec!["the record carries no launch evidence, so it is not a run's record".into()],
        );
    };
    if launch.intent_digest != digest_bytes(intent_bytes) {
        unbound.push(
            "the intent's bytes are not the ones the record finalized; it was changed after \
             the record bound it"
                .to_string(),
        );
    }
    if record.required_harness != intent.required_harness {
        unbound.push("the record and the intent name different requirements".to_string());
    }
    if record.resolved_harness.as_deref() != launch.harness.digest() {
        unbound.push(
            "the resolved identity is not the one the acknowledgment established".to_string(),
        );
    }
    if record.standing.required() != record.required_harness.as_deref() {
        unbound.push("the standing is about another requirement".to_string());
    }
    let standing_resolved = match &record.standing {
        Standing::Exact { resolved, .. } => resolved.clone(),
        Standing::Mismatched { resolved, .. } => Some(resolved.clone()),
        _ => record.resolved_harness.clone(),
    };
    if standing_resolved != record.resolved_harness {
        unbound.push("the standing was evaluated against another resolution".to_string());
    }
    if let HarnessObservation::Acknowledged { nonce, .. } = &launch.harness {
        if *nonce != intent.nonce {
            unbound.push("the acknowledgment's nonce is not this attempt's".to_string());
        }
    }
    if record.observation.observed() {
        unbound.push(format!(
            "the record carries an observation, and a run attempt carries none: {RUN_OBSERVATION}"
        ));
    }
    if !unbound.is_empty() {
        let mut reasons = vec!["the records do not agree with themselves".to_string()];
        reasons.extend(unbound);
        return (Verdict::Unverified, reasons);
    }

    if !launch.process.launched {
        return (
            Verdict::Interrupted,
            vec![format!(
                "the launch failed: {}",
                launch.process.failure.clone().unwrap_or_default()
            )],
        );
    }
    if launch.process.outcome.as_deref() == Some("interrupted") {
        let mut reasons = vec!["the process did not end by itself as one readable session".into()];
        reasons.extend(launch.process.stream_error.clone());
        reasons.extend(launch.process.surviving_processes.clone());
        return (Verdict::Interrupted, reasons);
    }
    if let Standing::Mismatched { .. } = record.standing {
        return (
            Verdict::Mismatched,
            vec![record.standing.refusal().unwrap_or_default()],
        );
    }
    if record.qualifies() {
        return (Verdict::Qualified, Vec::new());
    }

    let mut reasons = Vec::new();
    if let HarnessObservation::Unverified { .. } = &launch.harness {
        reasons.push(format!("observed harness {}", launch.harness.describe()));
    }
    if let Some(why) = record.standing.refusal() {
        reasons.push(format!("standing {}: {why}", record.standing.word()));
    }
    if !record.delivery.reached() {
        reasons.push(format!("reachability {}", record.delivery.describe()));
    }
    if !record.supply.supplied() {
        reasons.push(format!("supply {}", record.supply.describe()));
    }
    reasons.push(format!(
        "no live observation of this invocation: {RUN_OBSERVATION}"
    ));
    (Verdict::Unverified, reasons)
}

#[cfg(test)]
mod tests {
    // The one evidence fixture, shared with the startup module's tests; see
    // the note there on why it is `include!`d.
    #[allow(dead_code)]
    mod evidence {
        use crate as statecraft_home;
        include!("../tests/support/evidence.rs");
    }

    use super::*;
    use crate::harness;

    fn adapter() -> AdapterIdentity {
        AdapterIdentity {
            name: "claude-code".into(),
            harness: "claude-code".into(),
            version: "0.1.0".into(),
        }
    }

    /// A home holding the shipped revision, a project requiring it, and a
    /// workspace whose instruction chain reaches the managed file.
    struct World {
        _home: tempfile::TempDir,
        _project: tempfile::TempDir,
        layout: Layout,
        root: PathBuf,
        workspace: PathBuf,
        manifest: Manifest,
        digest: String,
    }

    fn world(require: bool) -> World {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::new(home.path());
        let digest = harness::install(&layout, &harness::shipped())
            .unwrap()
            .revision
            .digest;
        let project = tempfile::tempdir().unwrap();
        let root = project.path().to_path_buf();
        let mut manifest = Manifest::new(statecraft_environment::manifest::Pins {
            product: "0.1.0".into(),
            spec_spine: "0.20.0".into(),
            adapters: Default::default(),
        });
        if require {
            manifest
                .project
                .requirements
                .insert(crate::required::REQUIREMENT_KEY.into(), digest.clone());
        }
        std::fs::create_dir_all(root.join(".statecraft")).unwrap();
        manifest.write(&root).unwrap();
        let workspace = root.join(".statecraft/state/workspaces/003-x");
        std::fs::create_dir_all(workspace.join(".statecraft")).unwrap();
        manifest.write(&workspace).unwrap();
        std::fs::write(
            workspace.join(crate::project::INSTRUCTIONS),
            crate::project::managed_instructions(),
        )
        .unwrap();
        std::fs::write(
            workspace.join("AGENTS.md"),
            format!("@{}\n", crate::project::INSTRUCTIONS),
        )
        .unwrap();
        World {
            _home: home,
            _project: project,
            layout,
            root,
            workspace,
            manifest,
            digest,
        }
    }

    fn prepared(w: &World, attempt: u32) -> Prepared {
        prepare(&Preparation {
            root: &w.root,
            workspace: &w.workspace,
            base_commit: "0".repeat(40).as_str(),
            attempt: AttemptIdentity {
                run_id: "003-x".into(),
                attempt,
            },
            recorded_at: "2026-09-22T00:00:00Z",
            layout: &w.layout,
            manifest: &w.manifest,
            adapter: adapter(),
            program: "/bin/claude",
        })
        .unwrap()
    }

    /// The line the shipped hook would print for this intent, from `root`.
    fn ack(intent: &Intent, root: &Path) -> String {
        format!(
            "statecraft-startup\tv1\tnonce={}\trun={}\tattempt={}\tselected={}\tproject={}\troot={}",
            intent.nonce,
            intent.attempt.run_id,
            intent.attempt.attempt,
            intent.selected_word(),
            intent.workspace,
            std::fs::canonicalize(root).unwrap().display()
        )
    }

    fn response(stdout: &str) -> HookResponse {
        HookResponse {
            session_id: Some("s-1".into()),
            hook_event: Some("SessionStart".into()),
            hook_name: Some("SessionStart:startup".into()),
            exit_code: Some(0),
            outcome: Some("success".into()),
            stdout: Some(format!(
                "{stdout}\n[session-freshness] spec registry: fresh\n"
            )),
        }
    }

    fn required_root(w: &World) -> PathBuf {
        w.layout
            .harness_revision_dir(&harness::display_id(&w.digest))
    }

    fn spawned<'a>(responses: &'a [HookResponse], written: &'a [u8]) -> Launch<'a> {
        Launch::Spawned {
            hook_responses: responses,
            session_id: Some("s-1"),
            provider_version: Some("2.1.267"),
            settings_written: written,
            outcome: "completed",
            stream_error: None,
            surviving_processes: None,
        }
    }

    fn facts(n: u32) -> Vec<AttemptFact> {
        (1..=n)
            .map(|number| AttemptFact {
                number,
                outcome: Some("completed".into()),
            })
            .collect()
    }

    #[test]
    fn a_matching_acknowledgment_is_the_observed_revision_and_the_attempt_is_unverified() {
        let w = world(true);
        let p = prepared(&w, 1);
        assert_eq!(p.intent.selected.as_ref().unwrap().digest, w.digest);
        let responses = [response(&ack(&p.intent, &required_root(&w)))];
        let payload = crate::session::payload_json();
        let (record, _) = finalize(
            &w.root,
            &w.layout,
            &w.manifest,
            &p,
            &spawned(&responses, payload.as_bytes()),
            "2026-09-22T00:00:01Z",
        )
        .unwrap();
        assert_eq!(record.resolved_harness.as_deref(), Some(w.digest.as_str()));
        assert!(record.standing.permits_managed_execution());
        assert!(record.supply.supplied());
        assert!(record.delivery.reached());
        // Every class but the live observation, which a run never has.
        assert!(!record.qualifies());
        assert_eq!(refuses_the_attempt(&record), None);

        let shown = inspect(&w.root, "003-x", None, &facts(1)).unwrap();
        assert_eq!(shown.verdict, Verdict::Unverified, "{:?}", shown.reasons);
        assert!(
            shown
                .reasons
                .iter()
                .any(|r| r.contains("no live observation")),
            "{:?}",
            shown.reasons
        );
        assert_eq!(shown.reasons.len(), 1, "{:?}", shown.reasons);
    }

    /// Every way an acknowledgment fails to bind, and the word for each.
    #[test]
    fn each_failed_acknowledgment_is_unverified_by_its_own_kind() {
        let w = world(true);
        let p = prepared(&w, 1);
        let good = ack(&p.intent, &required_root(&w));
        let other_nonce = good.replace(&p.intent.nonce, &"f".repeat(32));
        let other_attempt = good.replace("\tattempt=1\t", "\tattempt=2\t");
        let other_project = good.replace(&p.intent.workspace, "/elsewhere");
        let foreign = tempfile::tempdir().unwrap();
        let foreign_root = foreign.path().join("h-000000000000");
        std::fs::create_dir_all(&foreign_root).unwrap();
        let foreign_line = ack(&p.intent, &foreign_root);
        // A second revision directory, installed in this home, named twice.
        let second = w.layout.harness_revision_dir("h-aaaaaaaaaaaa");
        std::fs::create_dir_all(second.join("hooks")).unwrap();
        std::fs::write(second.join("hooks/x.sh"), "x").unwrap();
        let second_line = ack(&p.intent, &second);

        type Case = (
            &'static str,
            Vec<HookResponse>,
            Option<&'static str>,
            Unverified,
        );
        let cases: Vec<Case> = vec![
            (
                "absent",
                vec![response("nothing")],
                Some("s-1"),
                Unverified::Absent,
            ),
            (
                "malformed",
                vec![response("statecraft-startup\tv1\tnonce")],
                Some("s-1"),
                Unverified::Malformed,
            ),
            (
                "replayed",
                vec![response(&other_nonce)],
                Some("s-1"),
                Unverified::Replayed,
            ),
            (
                "wrong-attempt",
                vec![response(&other_attempt)],
                Some("s-1"),
                Unverified::WrongAttempt,
            ),
            (
                "wrong-project",
                vec![response(&other_project)],
                Some("s-1"),
                Unverified::WrongProject,
            ),
            (
                "wrong-session",
                vec![response(&good)],
                Some("s-2"),
                Unverified::WrongSession,
            ),
            (
                "hook-failed",
                vec![HookResponse {
                    exit_code: Some(1),
                    ..response(&good)
                }],
                Some("s-1"),
                Unverified::HookFailed,
            ),
            (
                "conflicting",
                vec![response(&good), response(&second_line)],
                Some("s-1"),
                Unverified::Conflicting,
            ),
            (
                "foreign-revision",
                vec![response(&foreign_line)],
                Some("s-1"),
                Unverified::ForeignRevision,
            ),
            (
                "another event",
                vec![HookResponse {
                    hook_event: Some("Stop".into()),
                    ..response(&good)
                }],
                Some("s-1"),
                Unverified::Malformed,
            ),
        ];
        for (name, responses, session, kind) in cases {
            match observe(&p.intent, &responses, session, &w.layout) {
                HarnessObservation::Unverified { kind: got, detail } => {
                    assert_eq!(got, kind, "{name}: {detail}");
                }
                other => panic!("{name}: admitted {other:?}"),
            }
        }
        // And the good line, alone, is admitted.
        assert!(matches!(
            observe(&p.intent, &[response(&good)], Some("s-1"), &w.layout),
            HarnessObservation::Acknowledged { .. }
        ));
    }

    #[test]
    fn a_corrupt_answering_revision_reads_as_what_it_now_is() {
        let w = world(true);
        let p = prepared(&w, 1);
        let root = required_root(&w);
        let line = ack(&p.intent, &root);
        std::fs::write(root.join("rules/statecraft-governed-work.md"), "changed\n").unwrap();
        let payload = crate::session::payload_json();
        let (record, _) = finalize(
            &w.root,
            &w.layout,
            &w.manifest,
            &p,
            &spawned(&[response(&line)], payload.as_bytes()),
            "t",
        )
        .unwrap();
        // Observed is what the directory digests to now, and the required
        // directory itself no longer digests to the requirement.
        assert_ne!(record.resolved_harness.as_deref(), Some(w.digest.as_str()));
        assert_eq!(record.standing.word(), "corrupt");
        assert!(!record.qualifies());
        let shown = inspect(&w.root, "003-x", Some(1), &facts(1)).unwrap();
        assert_eq!(shown.verdict, Verdict::Unverified);
    }

    #[test]
    fn a_mismatched_revision_refuses_the_attempt() {
        let w = world(true);
        let p = prepared(&w, 1);
        let other = w.layout.harness_revision_dir("h-bbbbbbbbbbbb");
        std::fs::create_dir_all(other.join("hooks")).unwrap();
        std::fs::write(other.join("hooks/statecraft-session-start.sh"), "older").unwrap();
        let payload = crate::session::payload_json();
        let (record, _) = finalize(
            &w.root,
            &w.layout,
            &w.manifest,
            &p,
            &spawned(&[response(&ack(&p.intent, &other))], payload.as_bytes()),
            "t",
        )
        .unwrap();
        assert_eq!(record.standing.word(), "mismatched");
        assert!(refuses_the_attempt(&record).is_some());
        let shown = inspect(&w.root, "003-x", None, &facts(1)).unwrap();
        assert_eq!(shown.verdict, Verdict::Mismatched);
    }

    #[test]
    fn an_unrequired_project_selects_nothing_and_never_qualifies() {
        let w = world(false);
        let p = prepared(&w, 1);
        assert_eq!(p.intent.selected, None);
        assert!(
            p.intent
                .environment()
                .contains(&(ENV_SELECTED.into(), NONE_SELECTED.into()))
        );
        let line = ack(&p.intent, &required_root(&w));
        let payload = crate::session::payload_json();
        let (record, _) = finalize(
            &w.root,
            &w.layout,
            &w.manifest,
            &p,
            &spawned(&[response(&line)], payload.as_bytes()),
            "t",
        )
        .unwrap();
        // It observed something, and unrequired is still unrequired.
        assert!(record.resolved_harness.is_some());
        assert_eq!(record.standing, Standing::Unrequired);
        assert!(!record.qualifies());
        assert_eq!(refuses_the_attempt(&record), None);
    }

    #[test]
    fn an_invalid_requirement_prepares_nothing() {
        let mut w = world(true);
        w.manifest
            .project
            .requirements
            .insert(crate::required::REQUIREMENT_KEY.into(), "e".repeat(64));
        let err = prepare(&Preparation {
            root: &w.root,
            workspace: &w.workspace,
            base_commit: "x",
            attempt: AttemptIdentity {
                run_id: "003-x".into(),
                attempt: 1,
            },
            recorded_at: "t",
            layout: &w.layout,
            manifest: &w.manifest,
            adapter: adapter(),
            program: "p",
        })
        .unwrap_err();
        assert!(matches!(err, NotPrepared::Standing(_)), "{err}");
        assert!(
            !AttemptIdentity {
                run_id: "003-x".into(),
                attempt: 1
            }
            .directory(&w.root)
            .exists()
        );
    }

    #[test]
    fn an_attempts_evidence_is_written_once_and_never_adopted_by_another() {
        let w = world(true);
        let first = prepared(&w, 1);
        let payload = crate::session::payload_json();
        finalize(
            &w.root,
            &w.layout,
            &w.manifest,
            &first,
            &spawned(
                &[response(&ack(&first.intent, &required_root(&w)))],
                payload.as_bytes(),
            ),
            "t",
        )
        .unwrap();
        let before = std::fs::read(first.intent.attempt.record_path(&w.root)).unwrap();

        // The same attempt again is refused, before and after its record.
        let again = prepare(&Preparation {
            root: &w.root,
            workspace: &w.workspace,
            base_commit: "x",
            attempt: first.intent.attempt.clone(),
            recorded_at: "t",
            layout: &w.layout,
            manifest: &w.manifest,
            adapter: adapter(),
            program: "p",
        })
        .unwrap_err();
        assert!(matches!(again, NotPrepared::Exists { .. }));
        assert!(
            finalize(
                &w.root,
                &w.layout,
                &w.manifest,
                &first,
                &Launch::Failed { reason: "x".into() },
                "t"
            )
            .is_err()
        );

        // A second attempt that replays the first attempt's acknowledgment
        // does not inherit its observation, and the first record is intact.
        let second = prepared(&w, 2);
        assert_ne!(second.intent.nonce, first.intent.nonce);
        let replay = ack(&first.intent, &required_root(&w));
        let (record, _) = finalize(
            &w.root,
            &w.layout,
            &w.manifest,
            &second,
            &spawned(&[response(&replay)], payload.as_bytes()),
            "t",
        )
        .unwrap();
        assert!(matches!(
            record.launch.as_ref().unwrap().harness,
            HarnessObservation::Unverified {
                kind: Unverified::Replayed,
                ..
            }
        ));
        assert_eq!(record.resolved_harness, None);
        assert_eq!(
            std::fs::read(first.intent.attempt.record_path(&w.root)).unwrap(),
            before
        );
        assert_eq!(
            inspect(&w.root, "003-x", Some(1), &facts(2))
                .unwrap()
                .verdict,
            Verdict::Unverified
        );
        let latest = inspect(&w.root, "003-x", None, &facts(2)).unwrap();
        assert_eq!(latest.attempt.attempt, 2);
        assert!(
            latest.reasons.iter().any(|r| r.contains("replayed")),
            "{:?}",
            latest.reasons
        );
    }

    #[test]
    fn an_intent_with_no_record_is_interrupted_and_nothing_fabricates_one() {
        let w = world(true);
        let p = prepared(&w, 1);
        let shown = inspect(&w.root, "003-x", None, &facts(1)).unwrap();
        assert!(shown.launched);
        assert_eq!(shown.verdict, Verdict::Interrupted);
        assert!(!p.intent.attempt.record_path(&w.root).exists());
    }

    #[test]
    fn a_refused_attempt_with_no_intent_was_not_launched() {
        let w = world(true);
        let shown = inspect(
            &w.root,
            "003-x",
            None,
            &[AttemptFact {
                number: 1,
                outcome: Some("refused".into()),
            }],
        )
        .unwrap();
        assert!(!shown.launched);
        assert_eq!(shown.verdict, Verdict::NotLaunched);
        assert!(matches!(
            inspect(&w.root, "003-x", Some(4), &facts(1)),
            Err(NotRead::NoSuchAttempt(_))
        ));
    }

    #[test]
    fn a_failed_launch_supplies_nothing_and_is_interrupted() {
        let w = world(true);
        let p = prepared(&w, 1);
        let (record, _) = finalize(
            &w.root,
            &w.layout,
            &w.manifest,
            &p,
            &Launch::Failed {
                reason: "no such file".into(),
            },
            "t",
        )
        .unwrap();
        assert_eq!(record.supply.word(), "failed");
        assert_eq!(record.resolved_harness, None);
        let shown = inspect(&w.root, "003-x", None, &facts(1)).unwrap();
        assert_eq!(shown.verdict, Verdict::Interrupted);
    }

    #[test]
    fn other_settings_bytes_are_a_failed_supply() {
        let w = world(true);
        let p = prepared(&w, 1);
        let (record, _) = finalize(
            &w.root,
            &w.layout,
            &w.manifest,
            &p,
            &spawned(&[], b"{}"),
            "t",
        )
        .unwrap();
        assert_eq!(record.supply.word(), "failed");
        assert!(record.supply.describe().contains("payload"));
    }

    /// Section 3.31 rule 21: the judgement is the bytes', every time, and a
    /// hand edit changes it rather than asserting one.
    #[test]
    fn a_record_read_back_is_judged_again_and_hand_edits_do_not_qualify_it() {
        let w = world(true);
        let p = prepared(&w, 1);
        let payload = crate::session::payload_json();
        let (_, path) = finalize(
            &w.root,
            &w.layout,
            &w.manifest,
            &p,
            &spawned(
                &[response(&ack(&p.intent, &required_root(&w)))],
                payload.as_bytes(),
            ),
            "t",
        )
        .unwrap();
        let original: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let evidence =
            serde_json::to_value(crate::startup::admit(&evidence::admissible()).unwrap()).unwrap();
        type Edit = (&'static str, Box<dyn Fn(&mut serde_json::Value)>);
        let edits: Vec<Edit> = vec![
            (
                "an admitted observation pasted in",
                Box::new(move |v| v["observation"] = evidence.clone()),
            ),
            (
                "the resolution changed",
                Box::new(|v| v["resolvedHarness"] = serde_json::json!("0".repeat(64))),
            ),
            (
                "the attempt renamed",
                Box::new(|v| v["attempt"]["attempt"] = serde_json::json!(9)),
            ),
            (
                "the acknowledgment's nonce changed",
                Box::new(|v| {
                    v["launch"]["harness"]["nonce"] = serde_json::json!("0".repeat(32));
                }),
            ),
        ];
        for (name, edit) in edits {
            let mut v = original.clone();
            edit(&mut v);
            std::fs::write(&path, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
            let shown = inspect(&w.root, "003-x", None, &facts(1)).unwrap();
            assert_ne!(shown.verdict, Verdict::Qualified, "{name}");
            assert!(
                shown.reasons.iter().any(|r| r.contains("do not agree")),
                "{name}: {:?}",
                shown.reasons
            );
        }
        // The intent edited after the record bound it.
        std::fs::write(&path, serde_json::to_vec_pretty(&original).unwrap()).unwrap();
        let mut intent: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&p.path).unwrap()).unwrap();
        intent["nonce"] = serde_json::json!("1".repeat(32));
        std::fs::write(&p.path, serde_json::to_vec_pretty(&intent).unwrap()).unwrap();
        let shown = inspect(&w.root, "003-x", None, &facts(1)).unwrap();
        assert!(
            shown.reasons.iter().any(|r| r.contains("changed after")),
            "{:?}",
            shown.reasons
        );
        // And an unreadable record is an error, not an absence.
        std::fs::write(&path, "{ truncated").unwrap();
        assert!(matches!(
            inspect(&w.root, "003-x", None, &facts(1)),
            Err(NotRead::Unreadable { .. })
        ));
    }

    #[test]
    fn the_acknowledgment_parser_takes_only_the_exact_shape() {
        let good =
            "statecraft-startup\tv1\tnonce=a\trun=b\tattempt=1\tselected=none\tproject=/p\troot=/r";
        assert!(parse(good).is_some());
        for bad in [
            "statecraft-startup\tv2\tnonce=a\trun=b\tattempt=1\tselected=none\tproject=/p\troot=/r",
            "statecraft-startup\tv1\trun=b\tnonce=a\tattempt=1\tselected=none\tproject=/p\troot=/r",
            "statecraft-startup\tv1\tnonce=a\trun=b\tattempt=1\tselected=none\tproject=/p",
            "statecraft-startup v1 nonce=a run=b attempt=1 selected=none project=/p root=/r",
            "statecraft-startup\tv1\tnonce=a\trun=b\tattempt=1\tselected=none\tproject=/p\troot=/r\textra=1",
        ] {
            assert!(parse(bad).is_none(), "{bad}");
        }
    }
}
