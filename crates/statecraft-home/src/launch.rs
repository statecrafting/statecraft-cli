//! A run attempt's launch records, its startup decision, and the harness
//! revision whose acknowledgment it correlated.
//!
//! Spec 002 sections 3.31 and 3.32, settled by the owner on 2026-09-22.
//!
//! # Four records per attempt, each written once
//!
//! [`prepare`] writes `intent.json` after every preflight has passed and
//! **before** a spawn is attempted. [`LaunchWatch`] writes `launched.json`
//! when the spawn call has returned a process and **before** the prompt is
//! delivered, and `admission.json` at the startup decision. [`finalize`]
//! writes `record.json`. Each is created exclusively, so one attempt's evidence
//! is never overwritten and never adopted by another.
//!
//! A spawn and a file write are not one transaction. An intent alone says this
//! product was about to attempt a spawn, and nothing more; a confirmation
//! alone says a process was created, and nothing about what it did. [`inspect`]
//! reads what is on disk as the state it establishes: `launch-unknown` and
//! `outcome-unknown` are the words for a crash, and neither is `interrupted`.
//!
//! # Selected, correlated, admitted
//!
//! The run **selects** the required revision once the standing has shown it is
//! installed and intact, and registers that revision's `SessionStart` hook and
//! this attempt's admission gate in the settings document it passes to the
//! session. The hook's acknowledgment in the attempt's own stream is judged by
//! [`observe`]: one that binds is **correlated**, anything else is
//! `unverified`, by kind. At the first event after the `SessionStart` hooks the
//! watch decides, and the gate releases tool calls only on `admitted`.
//!
//! # What this module does not claim
//!
//! A correlated acknowledgment is a line in a hook response that carries this
//! attempt's binding and names a revision directory whose files digest to the
//! recorded identity. Nothing authenticates which process printed it: the
//! provider's response does not name the command, and the nonce is in the
//! session's environment. So "executed" is never claimed, of the hook or of
//! anything else in the revision. The gate holds a tool call the provider
//! routes through it; it cannot hold one the provider does not.

use crate::delivery::Delivery;
use crate::home::Layout;
use crate::required::Standing;
use crate::startup::{
    AdapterIdentity, FileIdentity, Observation, ProjectIdentity, StartupRecord, Supply,
};
use serde::{Deserialize, Serialize};
use statecraft_adapter::supervisor::{Control, Watch};
use statecraft_adapter_claude_code::execution::HookResponse;
use statecraft_adapter_claude_code::stream::ProviderEvent;
use statecraft_environment::digest::digest_bytes;
use statecraft_environment::manifest::Manifest;
use std::path::{Path, PathBuf};

/// The schema version of the attempt records this build writes. Version 2
/// records, written before section 3.32, are read and judged.
pub const LAUNCH_VERSION: u32 = 3;

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

/// The guard a mismatched startup decision refuses the attempt under.
pub const HARNESS_IDENTITY_GUARD: &str = "harness-identity";

/// The guard a startup decision that could not establish the revision refuses
/// the attempt under (section 3.32 rule 26).
pub const STARTUP_ADMISSION_GUARD: &str = "startup-admission";

/// The guard an unwritable intent refuses the attempt under (rule 15).
pub const STARTUP_RECORD_GUARD: &str = "startup-record";

/// How long the gate waits for a decision not yet written, in tenths of a
/// second, before it refuses the tool call.
pub const GATE_WAIT_TENTHS: u32 = 300;

/// The hook timeout the gate is registered with, in seconds: longer than its
/// own wait, so the gate's refusal is what a provider reports, not a timeout.
pub const GATE_HOOK_TIMEOUT_SECONDS: u32 = 60;

/// The file names in an attempt's directory.
pub mod files {
    /// Section 3.32 rule 22's first record.
    pub const INTENT: &str = "intent.json";
    /// The spawn confirmation.
    pub const LAUNCHED: &str = "launched.json";
    /// The startup decision.
    pub const ADMISSION: &str = "admission.json";
    /// The completion.
    pub const RECORD: &str = "record.json";
    /// The admission gate script.
    pub const GATE: &str = "admission-gate";
    /// The gate's consultations, appended by the gate.
    pub const GATE_LOG: &str = "gate.log";
}

/// The admission gate, written once into an attempt's directory before its
/// spawn (section 3.32 rules 25 and 26). Launcher content, not harness
/// content: no revision's digest covers it.
///
/// It reads and discards the tool call, waits for `admission.json`, and lets
/// the call run only when the decision is `admitted`. Exit 2 is the provider's
/// documented blocking code for a `PreToolUse` hook.
pub const GATE_SCRIPT: &str = r#"#!/bin/sh
# Statecraft admission gate for one run attempt: spec 002 section 3.32 rule 26.
# Written once by the launcher into the attempt's directory. Not harness
# content. Every tool call waits here for the launcher's startup decision and
# runs only if that decision is `admitted`.
dir=$(CDPATH='' cd -- "$(dirname -- "$0")" 2>/dev/null && pwd -P) || {
  echo "statecraft: the admission gate cannot find its attempt directory; tool use withheld" >&2
  exit 2
}
# The first argument is how long to wait for a decision, in tenths of a
# second; the launcher registers the command with it.
limit=${1:-300}
cat > /dev/null 2>&1
waited=0
while [ ! -f "$dir/admission.json" ]; do
  if [ "$waited" -ge "$limit" ]; then
    printf 'withheld: no decision within %s tenths of a second\n' "$limit" >> "$dir/gate.log" 2>/dev/null
    echo "statecraft: no startup decision within the gate's wait; tool use withheld" >&2
    exit 2
  fi
  sleep 0.1
  waited=$((waited + 1))
done
if grep -q '"decision":"admitted"' "$dir/admission.json" 2>/dev/null; then
  printf 'admitted\n' >> "$dir/gate.log" 2>/dev/null
  exit 0
fi
printf 'refused\n' >> "$dir/gate.log" 2>/dev/null
echo "statecraft: the startup decision refused this session's tool use; see $dir/admission.json" >&2
exit 2
"#;

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
        self.directory(root).join(files::INTENT)
    }

    /// Where the spawn confirmation is.
    pub fn launched_path(&self, root: &Path) -> PathBuf {
        self.directory(root).join(files::LAUNCHED)
    }

    /// Where the startup decision is.
    pub fn admission_path(&self, root: &Path) -> PathBuf {
        self.directory(root).join(files::ADMISSION)
    }

    /// Where the record is.
    pub fn record_path(&self, root: &Path) -> PathBuf {
        self.directory(root).join(files::RECORD)
    }

    /// Where the admission gate is.
    pub fn gate_path(&self, root: &Path) -> PathBuf {
        self.directory(root).join(files::GATE)
    }

    /// Where the gate appends its consultations.
    pub fn gate_log_path(&self, root: &Path) -> PathBuf {
        self.directory(root).join(files::GATE_LOG)
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

/// The settings document a session is given, by identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadIdentity {
    /// SHA-256 of the exact bytes.
    pub digest: String,
    /// Their length.
    pub bytes: u64,
    /// The argument that carries them.
    pub argument: String,
    /// SHA-256 of the deny floor alone, as `session payload` prints it. Equal
    /// to `digest` only when the document is the floor alone. Absent from a
    /// record written before section 3.32, whose document was the floor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_digest: Option<String>,
}

impl PayloadIdentity {
    /// This build's floor payload, alone.
    pub fn of_this_build() -> Self {
        Self::of_document(&crate::session::payload_json())
    }

    /// The identity of one exact document.
    pub fn of_document(document: &str) -> Self {
        Self {
            digest: digest_bytes(document.as_bytes()),
            bytes: document.len() as u64,
            argument: crate::session::SETTINGS_ARGUMENT.to_string(),
            floor_digest: Some(crate::startup::payload_identity()),
        }
    }
}

/// One hook this product registered for a session, in its settings document
/// (section 3.32 rule 25).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registration {
    /// The provider's hook event, `SessionStart` or `PreToolUse`.
    pub event: String,
    /// Its matcher.
    pub matcher: String,
    /// The command string, exactly as the document carries it.
    pub command: String,
    /// The script the command names, absolute.
    pub script: String,
    /// SHA-256 of that script's bytes when the intent was written.
    pub script_digest: String,
    /// The hook timeout, in seconds, where one is registered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u32>,
}

/// What a run records before it attempts the spawn.
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
    /// The settings document the session is to be given, by identity.
    pub payload: PayloadIdentity,
    /// The binding nonce, fresh for this attempt.
    pub nonce: String,
    /// The exact settings document, where the intent is version 3 or later.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_document: Option<String>,
    /// The hooks registered in it, in the order the document carries them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub registrations: Vec<Registration>,
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

    /// The provider's `hooks` settings value for these registrations, or
    /// `None` when nothing is registered.
    pub fn hooks(&self) -> Option<serde_json::Value> {
        hooks_value(&self.registrations)
    }

    /// Whether this attempt's tool calls are held by an admission gate.
    pub fn gated(&self) -> bool {
        self.registrations.iter().any(|r| r.event == "PreToolUse")
    }
}

/// The `hooks` value for a set of registrations.
fn hooks_value(registrations: &[Registration]) -> Option<serde_json::Value> {
    if registrations.is_empty() {
        return None;
    }
    let mut hooks = serde_json::Map::new();
    for r in registrations {
        let mut entry = serde_json::json!({ "type": "command", "command": r.command });
        if let Some(timeout) = r.timeout {
            entry["timeout"] = serde_json::json!(timeout);
        }
        let groups = hooks
            .entry(r.event.clone())
            .or_insert_with(|| serde_json::json!([]));
        groups
            .as_array_mut()
            .expect("created as an array")
            .push(serde_json::json!({ "matcher": r.matcher, "hooks": [entry] }));
    }
    Some(serde_json::Value::Object(hooks))
}

/// Quote a path for the shell a provider runs a hook command with.
fn shell_quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', r"'\''"))
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

impl Prepared {
    /// The exact settings document the session is to be given.
    pub fn settings_document(&self) -> String {
        self.intent
            .settings_document
            .clone()
            .unwrap_or_else(crate::session::payload_json)
    }
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
    /// The attempt already has evidence. It is not replaced.
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

/// Write an attempt's gate, where it is gated, and then its intent. Nothing
/// may be launched unless this succeeds.
pub fn prepare(p: &Preparation<'_>) -> Result<Prepared, NotPrepared> {
    if let Some(why) = p.attempt.invalid() {
        return Err(NotPrepared::Identity(why));
    }
    let standing = crate::required::evaluate(p.layout, p.manifest, None);
    if let Some(why) = standing.refuses_a_run() {
        return Err(NotPrepared::Standing(why));
    }
    let io = |e: std::io::Error| NotPrepared::Io(e.to_string());
    let exists = |path: &Path| NotPrepared::Exists {
        run: p.attempt.run_id.clone(),
        attempt: p.attempt.attempt,
        path: path.display().to_string(),
    };
    let intent_path = p.attempt.intent_path(p.root);
    for existing in [
        intent_path.clone(),
        p.attempt.launched_path(p.root),
        p.attempt.admission_path(p.root),
        p.attempt.record_path(p.root),
        p.attempt.gate_path(p.root),
    ] {
        if existing.exists() {
            return Err(exists(&existing));
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

    // Section 3.32 rule 25: a selected revision's startup hook and this
    // attempt's gate are registered per invocation. The gate is written first,
    // so the intent can name the bytes it registers.
    let mut registrations = Vec::new();
    if let Some(selected) = &selected {
        let hook = Path::new(&selected.root).join("hooks/statecraft-session-start.sh");
        let hook_bytes = std::fs::read(&hook).map_err(|e| {
            NotPrepared::Io(format!(
                "the selected revision's startup hook {} could not be read: {e}",
                hook.display()
            ))
        })?;
        let gate = p.attempt.gate_path(p.root);
        write_once(&gate, GATE_SCRIPT).map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => exists(&gate),
            _ => NotPrepared::Io(e.to_string()),
        })?;
        make_executable(&gate).map_err(io)?;
        let gate = std::fs::canonicalize(&gate).map_err(io)?;
        let hook = hook.display().to_string();
        let gate = gate.display().to_string();
        registrations.push(Registration {
            event: "SessionStart".into(),
            matcher: "startup".into(),
            command: shell_quote(&hook),
            script: hook,
            script_digest: digest_bytes(&hook_bytes),
            timeout: None,
        });
        registrations.push(Registration {
            event: "PreToolUse".into(),
            matcher: "*".into(),
            command: format!("{} {GATE_WAIT_TENTHS}", shell_quote(&gate)),
            script: gate,
            script_digest: digest_bytes(GATE_SCRIPT.as_bytes()),
            timeout: Some(GATE_HOOK_TIMEOUT_SECONDS),
        });
    }
    let document = match hooks_value(&registrations) {
        Some(hooks) => crate::session::managed_payload_json(hooks),
        None => crate::session::payload_json(),
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
        payload: PayloadIdentity::of_document(&document),
        nonce: nonce().map_err(io)?,
        settings_document: Some(document),
        registrations,
    };
    let mut json =
        serde_json::to_string_pretty(&intent).map_err(|e| NotPrepared::Io(e.to_string()))?;
    json.push('\n');
    write_once(&intent_path, &json).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => exists(&intent_path),
        _ => NotPrepared::Io(e.to_string()),
    })?;
    Ok(Prepared {
        digest: digest_bytes(json.as_bytes()),
        intent,
        path: intent_path,
    })
}

#[cfg(unix)]
fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn make_executable(_: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Sixteen bytes from the operating system's generator, as hex.
fn nonce() -> std::io::Result<String> {
    use std::io::Read;
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Create a file that must not already exist, whole or not at all.
///
/// The bytes go to a private temporary file in the same directory, which is
/// then hard-linked to the final name. The link is the exclusive step: it
/// fails if the name exists, and a reader never sees half a file. A rename
/// would replace an existing file, which is the one thing this must not do.
fn write_once(path: &Path, contents: &str) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "a record needs a parent")
    })?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".statecraft-partial-{}-{}",
        std::process::id(),
        nonce()?
    ));
    let written = (|| {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        std::fs::hard_link(&temporary, path)
    })();
    // The temporary name goes whatever happened; the linked name stays.
    let _ = std::fs::remove_file(&temporary);
    written
}

// ---------------------------------------------------------------------------
// The observation.
// ---------------------------------------------------------------------------

/// Why an acknowledgment did not correlate which revision answered.
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

/// Which revision directory this attempt's acknowledgment named, and the grade
/// of that evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "grade")]
pub enum HarnessObservation {
    /// Section 3.32 rule 27: one acknowledgment, in a `SessionStart` response
    /// of this attempt's stream, carried this attempt's binding and named this
    /// revision directory. Correlated, not authenticated: nothing says which
    /// process printed it. Records written before section 3.32 call this grade
    /// `acknowledged`, and are read as this one.
    #[serde(alias = "acknowledged")]
    Correlated {
        /// The revision directory the line named.
        root: String,
        /// Its display identifier.
        display: String,
        /// The full digest its files had when the record was written.
        digest: String,
        /// The session the acknowledgment arrived in.
        session_id: String,
        /// The hook's name as the provider reported it.
        hook_name: Option<String>,
        /// The nonce it echoed, which is this attempt's.
        nonce: String,
    },
    /// Nothing correlated which revision answered.
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
            HarnessObservation::Correlated { digest, .. } => Some(digest),
            HarnessObservation::Unverified { .. } => None,
        }
    }

    /// One line.
    pub fn describe(&self) -> String {
        match self {
            HarnessObservation::Correlated {
                display,
                session_id,
                root,
                ..
            } => format!(
                "{display}, correlated: a SessionStart response in session {session_id} carried \
                 this attempt's acknowledgment naming {root}; which process printed it is not \
                 established"
            ),
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
/// one binding acknowledgment correlates the revision; anything else is
/// unverified and says which way.
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
    HarnessObservation::Correlated {
        root: first,
        display: crate::harness::display_id(&revision.digest),
        digest: revision.digest,
        session_id: session_id.unwrap_or_default().to_string(),
        hook_name: roots[0].1.clone(),
        nonce: intent.nonce.clone(),
    }
}

// ---------------------------------------------------------------------------
// The spawn confirmation and the startup decision.
// ---------------------------------------------------------------------------

/// `launched.json`: a process was created for this attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Launched {
    /// Schema version.
    pub version: u32,
    /// Which attempt.
    pub attempt: AttemptIdentity,
    /// The process id the spawn call returned.
    pub pid: u32,
    /// When the confirmation was written, RFC 3339 UTC.
    pub confirmed_at: String,
    /// SHA-256 of the `intent.json` bytes this launch followed.
    pub intent_digest: String,
}

/// The startup decision's word (section 3.32 rule 26).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Decision {
    /// Governed work is released.
    Admitted,
    /// Refused: a correlated acknowledgment named another revision.
    Mismatched,
    /// Refused: no acknowledgment was admitted.
    NotEstablished,
    /// The project commits no requirement, so no decision gates the work.
    NotGated,
}

impl Decision {
    /// How an operator reads it.
    pub fn describe(self) -> &'static str {
        match self {
            Decision::Admitted => "admitted",
            Decision::Mismatched => "refused: mismatched",
            Decision::NotEstablished => "refused: not-established",
            Decision::NotGated => "not gated: the project commits no requirement",
        }
    }

    /// Whether it refuses governed work.
    pub fn refuses(self) -> bool {
        matches!(self, Decision::Mismatched | Decision::NotEstablished)
    }
}

/// `admission.json`: the startup decision, written once, one line of JSON so
/// the gate can read it without a parser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Admission {
    /// Schema version.
    pub version: u32,
    /// Which attempt.
    pub attempt: AttemptIdentity,
    /// The decision.
    pub decision: Decision,
    /// Why, in one sentence.
    pub reason: String,
    /// When it was made, RFC 3339 UTC.
    pub decided_at: String,
    /// The stream line the decision was made at, or `None` when the stream
    /// ended before the decision point and it was made at its end.
    pub at_line: Option<usize>,
    /// The observation it was made from.
    pub observation: HarnessObservation,
    /// SHA-256 of the `intent.json` bytes it decides for.
    pub intent_digest: String,
}

/// Watches one managed launch: confirms the spawn, collects the startup hook
/// responses, and makes the startup decision at the first event after them.
///
/// Spec 002 section 3.32 rules 22 and 26, through the supervisor's watch of
/// spec 004 section 5, 2026-09-22.
pub struct LaunchWatch<'a> {
    root: &'a Path,
    layout: &'a Layout,
    manifest: &'a Manifest,
    prepared: &'a Prepared,
    now: &'a dyn Fn() -> String,
    /// The process id the spawn call returned, whether or not its
    /// confirmation was persisted.
    pub spawned: Option<u32>,
    /// Why the confirmation could not be persisted, where it could not.
    pub confirmation_error: Option<String>,
    responses: Vec<HookResponse>,
    session: Option<String>,
    /// The decision, once made.
    pub admission: Option<Admission>,
    /// Why the decision could not be persisted, where it could not.
    pub admission_error: Option<String>,
}

impl<'a> LaunchWatch<'a> {
    /// A watch for one prepared attempt. `now` gives RFC 3339 UTC.
    pub fn new(
        root: &'a Path,
        layout: &'a Layout,
        manifest: &'a Manifest,
        prepared: &'a Prepared,
        now: &'a dyn Fn() -> String,
    ) -> Self {
        Self {
            root,
            layout,
            manifest,
            prepared,
            now,
            spawned: None,
            confirmation_error: None,
            responses: Vec::new(),
            session: None,
            admission: None,
            admission_error: None,
        }
    }

    /// Decide now, from what has been read, if nothing has decided yet: the
    /// stream ended before its decision point. Returns why governed work is
    /// refused, where it is.
    pub fn decide_at_end(&mut self) -> Option<String> {
        if self.admission.is_none() && self.admission_error.is_none() && self.spawned.is_some() {
            return self.decide(None);
        }
        None
    }

    /// Make and persist the decision. Returns the reason to stop the process,
    /// where governed work is refused or the decision could not be persisted.
    fn decide(&mut self, at_line: Option<usize>) -> Option<String> {
        let intent = &self.prepared.intent;
        let (decision, reason, observation) = if intent.selected.is_none() {
            (
                Decision::NotGated,
                "the project commits no harness requirement, so nothing is selected and no \
                 startup decision gates the session's tool calls"
                    .to_string(),
                observe(
                    intent,
                    &self.responses,
                    self.session.as_deref(),
                    self.layout,
                ),
            )
        } else {
            let observation = observe(
                intent,
                &self.responses,
                self.session.as_deref(),
                self.layout,
            );
            let standing =
                crate::required::evaluate(self.layout, self.manifest, observation.digest());
            match (&observation, &standing) {
                (
                    HarnessObservation::Correlated { .. },
                    Standing::Exact {
                        resolved: Some(_), ..
                    },
                ) => (
                    Decision::Admitted,
                    "one correlated acknowledgment named the required revision, and the \
                     standing against it is exact"
                        .to_string(),
                    observation,
                ),
                (HarnessObservation::Correlated { .. }, Standing::Mismatched { .. }) => (
                    Decision::Mismatched,
                    standing.refusal().unwrap_or_else(|| {
                        "the correlated revision is not the required one".to_string()
                    }),
                    observation,
                ),
                (HarnessObservation::Correlated { .. }, other) => (
                    Decision::NotEstablished,
                    format!(
                        "the correlated revision leaves the standing {}: {}",
                        other.word(),
                        other.refusal().unwrap_or_default()
                    ),
                    observation,
                ),
                (HarnessObservation::Unverified { .. }, _) => (
                    Decision::NotEstablished,
                    format!(
                        "no acknowledgment was admitted before the session's first turn: {}",
                        observation.describe()
                    ),
                    observation,
                ),
            }
        };
        let admission = Admission {
            version: LAUNCH_VERSION,
            attempt: intent.attempt.clone(),
            decision,
            reason: reason.clone(),
            decided_at: (self.now)(),
            at_line,
            observation,
            intent_digest: self.prepared.digest.clone(),
        };
        let written = serde_json::to_string(&admission)
            .map_err(|e| std::io::Error::other(e.to_string()))
            .and_then(|json| {
                write_once(
                    &intent.attempt.admission_path(self.root),
                    &format!("{json}\n"),
                )
            });
        let refuses = decision.refuses();
        self.admission = Some(admission);
        match written {
            Ok(()) if refuses => Some(format!(
                "the startup decision refused governed work ({}): {reason}",
                decision.describe()
            )),
            Ok(()) => None,
            Err(e) => {
                let why = format!("the startup decision could not be persisted: {e}");
                self.admission_error = Some(why.clone());
                // An unpersisted decision is one the gate cannot read, so it
                // holds every tool call; a gated session is stopped rather
                // than left waiting on it.
                (intent.gated() || refuses).then_some(why)
            }
        }
    }

    /// What the watch established, for [`finalize`].
    pub fn watched(&self) -> Watched {
        Watched {
            spawned: self.spawned,
            confirmation_error: self.confirmation_error.clone(),
            admission: self.admission.clone(),
            admission_error: self.admission_error.clone(),
        }
    }
}

impl Watch<(usize, ProviderEvent)> for LaunchWatch<'_> {
    fn spawned(&mut self, pid: u32) -> Result<(), String> {
        self.spawned = Some(pid);
        let intent = &self.prepared.intent;
        let launched = Launched {
            version: LAUNCH_VERSION,
            attempt: intent.attempt.clone(),
            pid,
            confirmed_at: (self.now)(),
            intent_digest: self.prepared.digest.clone(),
        };
        let written = serde_json::to_string_pretty(&launched)
            .map_err(|e| std::io::Error::other(e.to_string()))
            .and_then(|json| {
                write_once(
                    &intent.attempt.launched_path(self.root),
                    &format!("{json}\n"),
                )
            });
        written.map_err(|e| {
            let why = format!(
                "the spawn confirmation could not be persisted, so the process was stopped \
                 before its prompt was delivered: {e}"
            );
            self.confirmation_error = Some(why.clone());
            why
        })
    }

    fn event(&mut self, (line, event): &(usize, ProviderEvent)) -> Control {
        if self.admission.is_some() || self.admission_error.is_some() {
            return Control::Continue;
        }
        if statecraft_adapter_claude_code::execution::is_session_start_hook(event) {
            self.responses.extend(HookResponse::of(event));
            return Control::Continue;
        }
        if let Some(session) = statecraft_adapter_claude_code::execution::init_session(event) {
            self.session = Some(session.to_string());
        }
        match self.decide(Some(*line)) {
            Some(why) => Control::Stop(why),
            None => Control::Continue,
        }
    }
}

/// What a [`LaunchWatch`] established, handed to [`finalize`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Watched {
    /// The process id the spawn call returned.
    pub spawned: Option<u32>,
    /// Why the confirmation could not be persisted.
    pub confirmation_error: Option<String>,
    /// The decision.
    pub admission: Option<Admission>,
    /// Why the decision could not be persisted.
    pub admission_error: Option<String>,
}

// ---------------------------------------------------------------------------
// The record.
// ---------------------------------------------------------------------------

/// How the process ended, as the supervisor decided it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessEnd {
    /// Whether a process was created. In a version 3 record, from the spawn
    /// call's own answer; in a version 2 record, from whether supervision
    /// returned.
    pub launched: bool,
    /// The observed outcome word, `completed`, `interrupted` and so on, or
    /// `None` where the outcome is not known.
    pub outcome: Option<String>,
    /// A transport diagnostic, where there was one.
    pub stream_error: Option<String>,
    /// A survivor report, where there was one.
    pub surviving_processes: Option<String>,
    /// Why the launch or its supervision failed, where it did.
    pub failure: Option<String>,
    /// The process id, where the spawn call returned one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// Whether `launched.json` was persisted. Absent from a version 2 record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed: Option<bool>,
    /// Why this product stopped the process, where it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped: Option<String>,
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

/// Section 3.31's and 3.32's additions to a run attempt's section 3.26 record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchEvidence {
    /// SHA-256 of the `intent.json` bytes this record finalizes.
    pub intent_digest: String,
    /// The workspace the session started in.
    pub workspace: String,
    /// The revision selected before launch.
    pub selected: Option<Selected>,
    /// The settings document the session was to be given.
    pub payload: PayloadIdentity,
    /// The settings bytes the adapter wrote, where it wrote any.
    pub settings_written: Option<Written>,
    /// The provider's own session id, from its init event.
    pub provider_session: Option<String>,
    /// The provider's version, from its init event.
    pub provider_version: Option<String>,
    /// How the process ended.
    pub process: ProcessEnd,
    /// Which revision directory the acknowledgment named, and the grade.
    pub harness: HarnessObservation,
    /// The startup decision, where one was made. Absent from a version 2
    /// record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admission: Option<Admission>,
    /// Why the decision could not be persisted, where it could not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admission_error: Option<String>,
    /// The gate's consultations, one word each, as `gate.log` held them when
    /// the record was written. Absent where the attempt was not gated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<Vec<String>>,
}

/// What the launch produced, as the run hands it over.
#[derive(Debug, Clone)]
pub enum Launch<'a> {
    /// Supervision returned.
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
        /// Why this product stopped the process, where it did.
        stopped: Option<String>,
    },
    /// Supervision returned an error: before a spawn, or after one, which the
    /// watch's `spawned` says.
    Failed {
        /// Why.
        reason: String,
    },
}

/// Read `gate.log`, one word per consultation. `None` when there is none.
fn read_gate_log(path: &Path) -> Option<Vec<String>> {
    std::fs::read_to_string(path)
        .ok()
        .map(|t| t.lines().map(str::to_string).collect())
}

/// Assemble and write an attempt's record. The only place `record.json` is
/// written, and it is written once.
pub fn finalize(
    root: &Path,
    layout: &Layout,
    manifest: &Manifest,
    prepared: &Prepared,
    launch: &Launch<'_>,
    watched: &Watched,
    recorded_at: &str,
) -> std::io::Result<(StartupRecord, PathBuf)> {
    let intent = &prepared.intent;
    let launched = watched.spawned.is_some();
    let confirmed = launched.then_some(watched.confirmation_error.is_none());
    let (harness, process, settings_written, provider_session, provider_version) = match launch {
        Launch::Failed { reason } => (
            unverified(
                Unverified::NotLaunched,
                if launched {
                    format!("supervision failed after the spawn, so no stream was read: {reason}")
                } else {
                    reason.clone()
                },
            ),
            ProcessEnd {
                launched,
                outcome: None,
                stream_error: None,
                surviving_processes: None,
                failure: Some(reason.clone()),
                pid: watched.spawned,
                confirmed,
                stopped: None,
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
            stopped,
        } => (
            observe(intent, hook_responses, *session_id, layout),
            ProcessEnd {
                launched: true,
                outcome: Some((*outcome).to_string()),
                stream_error: stream_error.clone(),
                surviving_processes: surviving_processes.clone(),
                failure: None,
                pid: watched.spawned,
                confirmed,
                stopped: stopped.clone(),
            },
            Some(Written {
                digest: digest_bytes(settings_written),
                bytes: settings_written.len() as u64,
            }),
            session_id.map(str::to_string),
            provider_version.map(str::to_string),
        ),
    };

    // Rule 16: supply is what the launch handed over, and nothing else. A
    // spawn whose confirmation could not be persisted was stopped before its
    // prompt, and supplied its settings only.
    let supply = match (&process.launched, &settings_written) {
        (false, _) => Supply::Failed {
            reason: format!(
                "no process was created, so nothing was handed to a session: {}",
                process.failure.clone().unwrap_or_default()
            ),
            partial: Vec::new(),
        },
        (true, None) => Supply::Failed {
            reason: "the adapter reported no settings bytes".to_string(),
            partial: intent.instructions.clone(),
        },
        (true, _) if intent.load_chain.is_empty() => Supply::NotAttempted {
            reason: "the documented load rule does not reach the managed file in the workspace, \
                     so no instruction chain was supplied"
                .to_string(),
        },
        (true, Some(w)) if w.digest != intent.payload.digest => Supply::Failed {
            reason: format!(
                "the settings file held bytes digesting to {} and the document is {}",
                w.digest, intent.payload.digest
            ),
            partial: intent.instructions.clone(),
        },
        (true, Some(_)) => Supply::Supplied {
            files: intent.instructions.clone(),
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
            admission: watched.admission.clone(),
            admission_error: watched.admission_error.clone(),
            gate: if intent.gated() {
                Some(read_gate_log(&intent.attempt.gate_log_path(root)).unwrap_or_default())
            } else {
                None
            },
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

/// Whether this record's startup decision refuses the attempt, and under which
/// guard, with why (section 3.32 rule 26; section 3.31 rule 19 for a record
/// written before it).
pub fn refuses_the_attempt(record: &StartupRecord) -> Option<(&'static str, String)> {
    let launch = record.launch.as_ref()?;
    match (&launch.admission, &launch.admission_error) {
        (Some(a), _) if a.decision == Decision::Mismatched => {
            Some((HARNESS_IDENTITY_GUARD, a.reason.clone()))
        }
        (Some(a), _) if a.decision == Decision::NotEstablished => {
            Some((STARTUP_ADMISSION_GUARD, a.reason.clone()))
        }
        (_, Some(why)) if launch.gate.is_some() => Some((STARTUP_ADMISSION_GUARD, why.clone())),
        (Some(_), _) => None,
        (None, _) => match &record.standing {
            Standing::Mismatched { .. } if launch.process.pid.is_none() => record
                .standing
                .refusal()
                .map(|why| (HARNESS_IDENTITY_GUARD, why)),
            _ => None,
        },
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
    /// The startup decision, in one line, where one was made.
    pub admission: Option<String>,
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
            admission: None,
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
        if let Some(admission) = &self.admission {
            out.push_str(&format!("  admission: {admission}\n"));
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

/// A run attempt's verdict (section 3.31 rule 21, section 3.32 rules 23 and
/// 26).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// No intent: this product attempted no spawn for it.
    NotLaunched,
    /// An intent and nothing after it: whether a process was created is
    /// unknown.
    LaunchUnknown,
    /// A confirmed spawn and no completion, or a supervision that failed after
    /// the spawn: the outcome is unknown.
    OutcomeUnknown,
    /// The spawn call reported that no process was created.
    SpawnFailed,
    /// A process that did not end by itself as one readable session.
    Interrupted,
    /// The startup decision refused: a correlated revision other than the
    /// required one.
    Mismatched,
    /// The startup decision refused: nothing established the revision.
    NotAdmitted,
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
            Verdict::LaunchUnknown => "launch-unknown",
            Verdict::OutcomeUnknown => "outcome-unknown",
            Verdict::SpawnFailed => "spawn-failed",
            Verdict::Interrupted => "interrupted",
            Verdict::Mismatched => "mismatched",
            Verdict::NotAdmitted => "not-admitted",
            Verdict::Unverified => "unverified",
            Verdict::Qualified => "qualified",
        }
    }

    /// Whether the verdict leaves the attempt's outcome unknown, so it must not
    /// be retried until it is reconciled (section 3.32 rule 24).
    pub fn uncertain(self) -> bool {
        matches!(self, Verdict::LaunchUnknown | Verdict::OutcomeUnknown)
    }
}

/// What a spawn attempt is known to have done, from the records alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Spawn {
    /// No intent: this product attempted no spawn.
    NotAttempted,
    /// An intent and no confirmation: whether a process exists is unknown.
    Unknown,
    /// A confirmation: a process was created.
    Confirmed,
    /// Created, and its confirmation could not be persisted.
    Unconfirmed,
    /// The spawn call reported that no process was created.
    Failed,
}

impl Spawn {
    /// The word.
    pub fn word(self) -> &'static str {
        match self {
            Spawn::NotAttempted => "not-attempted",
            Spawn::Unknown => "unknown",
            Spawn::Confirmed => "confirmed",
            Spawn::Unconfirmed => "created, confirmation not persisted",
            Spawn::Failed => "failed",
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
    /// Where the spawn confirmation is.
    pub launched_path: String,
    /// Where the startup decision is.
    pub admission_path: String,
    /// Where the record is, and whether it is there.
    pub record_path: String,
    /// What the spawn is known to have done.
    pub spawn: Spawn,
    /// The intent, where there is one.
    pub intent: Option<Box<Intent>>,
    /// The spawn confirmation, where there is one.
    pub launched: Option<Launched>,
    /// The startup decision, where there is one.
    pub admission: Option<Box<Admission>>,
    /// The gate's consultations as `gate.log` holds them now, where it exists.
    pub gate: Option<Vec<String>>,
    /// The record, where there is one.
    pub record: Option<Box<StartupRecord>>,
    /// The verdict.
    pub verdict: Verdict,
    /// Every reason for it, in order.
    pub reasons: Vec<String>,
    /// What the operator should do next, where the records leave something to
    /// do.
    pub next: Option<String>,
}

impl AttemptStartup {
    /// Whether the verdict is `qualified`.
    pub fn qualified(&self) -> bool {
        self.verdict == Verdict::Qualified
    }

    /// The human rendering: what an operator asks, in order.
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
            "spawn     {}{}\n",
            self.spawn.word(),
            self.launched.as_ref().map_or(String::new(), |l| format!(
                ": process {} at {}",
                l.pid, l.confirmed_at
            ))
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
                "settings  {} ({} bytes, via {}){}\n",
                intent.payload.digest,
                intent.payload.bytes,
                intent.payload.argument,
                match &intent.payload.floor_digest {
                    Some(floor) if *floor != intent.payload.digest =>
                        format!("; the deny floor alone is {floor}"),
                    _ => String::new(),
                }
            ));
            for r in &intent.registrations {
                out.push_str(&format!(
                    "hook      {} {} -> {} ({})\n",
                    r.event, r.matcher, r.script, r.script_digest
                ));
            }
        }
        match &self.admission {
            Some(a) => out.push_str(&format!(
                "admission {} at {}: {}\n",
                a.decision.describe(),
                a.decided_at,
                a.reason
            )),
            None => out.push_str("admission no decision recorded\n"),
        }
        if let Some(gate) = &self.gate {
            out.push_str(&format!(
                "gate      consulted {} time(s){}\n",
                gate.len(),
                if gate.is_empty() {
                    String::new()
                } else {
                    format!(": {}", gate.join(", "))
                }
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
        let mark = |path: &str| {
            if Path::new(path).exists() {
                path.to_string()
            } else {
                format!("{path} (absent)")
            }
        };
        out.push_str(&format!(
            "evidence  {}\n          {}\n          {}\n          {}\n",
            mark(&self.intent_path),
            mark(&self.launched_path),
            mark(&self.admission_path),
            mark(&self.record_path),
        ));
        out.push_str(&format!("verdict   {}\n", self.verdict.word()));
        for reason in &self.reasons {
            out.push_str(&format!("  - {reason}\n"));
        }
        if let Some(next) = &self.next {
            out.push_str(&format!("next      {next}\n"));
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

/// Read an attempt's records and judge them, from their bytes alone.
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
    let launched_path = identity.launched_path(root);
    let admission_path = identity.admission_path(root);
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
    fn parse_as<T: serde::de::DeserializeOwned>(
        path: &Path,
        bytes: &Option<Vec<u8>>,
    ) -> Result<Option<T>, NotRead> {
        match bytes {
            Some(b) => serde_json::from_slice(b)
                .map(Some)
                .map_err(|e| NotRead::Unreadable {
                    path: path.display().to_string(),
                    detail: e.to_string(),
                }),
            None => Ok(None),
        }
    }
    let intent_bytes = read(&intent_path)?;
    let launched_bytes = read(&launched_path)?;
    let admission_bytes = read(&admission_path)?;
    let record_bytes = read(&record_path)?;
    let intent: Option<Intent> = parse_as(&intent_path, &intent_bytes)?;
    let launched: Option<Launched> = parse_as(&launched_path, &launched_bytes)?;
    let admission: Option<Admission> = parse_as(&admission_path, &admission_bytes)?;
    let record: Option<StartupRecord> = parse_as(&record_path, &record_bytes)?;
    let gate = read_gate_log(&identity.gate_log_path(root));

    let read_back = ReadBack {
        identity: &identity,
        intent: intent.as_ref(),
        intent_bytes: intent_bytes.as_deref(),
        launched: launched.as_ref(),
        admission: admission.as_ref(),
        gate: gate.as_deref(),
        record: record.as_ref(),
        outcome: fact.outcome.as_deref(),
    };
    let spawn = read_back.spawn();
    let (verdict, reasons) = judge(&read_back);
    let next = next_action(&read_back, verdict);
    Ok(AttemptStartup {
        root: root.display().to_string(),
        attempt: identity,
        outcome: fact.outcome.clone(),
        intent_path: intent_path.display().to_string(),
        launched_path: launched_path.display().to_string(),
        admission_path: admission_path.display().to_string(),
        record_path: record_path.display().to_string(),
        spawn,
        intent: intent.map(Box::new),
        launched,
        admission: admission.map(Box::new),
        gate,
        record: record.map(Box::new),
        verdict,
        reasons,
        next,
    })
}

/// Everything read for one attempt.
struct ReadBack<'a> {
    identity: &'a AttemptIdentity,
    intent: Option<&'a Intent>,
    intent_bytes: Option<&'a [u8]>,
    launched: Option<&'a Launched>,
    admission: Option<&'a Admission>,
    gate: Option<&'a [String]>,
    record: Option<&'a StartupRecord>,
    outcome: Option<&'a str>,
}

impl ReadBack<'_> {
    fn spawn(&self) -> Spawn {
        if self.intent.is_none() {
            return Spawn::NotAttempted;
        }
        if self.launched.is_some() {
            return Spawn::Confirmed;
        }
        match self.record.and_then(|r| r.launch.as_ref()) {
            Some(l) if !l.process.launched => Spawn::Failed,
            Some(l) if l.process.confirmed == Some(false) => Spawn::Unconfirmed,
            // A version 2 record: supervision returned, which is all it says.
            Some(l) if l.process.confirmed.is_none() => Spawn::Confirmed,
            _ => Spawn::Unknown,
        }
    }
}

/// What the records leave the operator to do, where they leave anything.
fn next_action(r: &ReadBack<'_>, verdict: Verdict) -> Option<String> {
    let intent = r.intent?;
    let run = &r.identity.run_id;
    let n = r.identity.attempt;
    match verdict {
        Verdict::LaunchUnknown => Some(format!(
            "whether a provider process was created is unknown: look for one started in {} and \
             inspect that workspace for effects; the attempt stays live, so `run` refuses run \
             {run} until attempt {n} is reconciled (spec 003 section 3.6), and this build has \
             no verb that reconciles it",
            intent.workspace
        )),
        Verdict::OutcomeUnknown => Some(format!(
            "process {} was created{}; confirm it is no longer running, and inspect {} for \
             effects; the attempt stays live, so `run` refuses run {run} until attempt {n} is \
             reconciled (spec 003 section 3.6), and this build has no verb that reconciles it",
            r.launched
                .map_or("(id not persisted)".to_string(), |l| l.pid.to_string()),
            r.launched
                .map_or(String::new(), |l| format!(" at {}", l.confirmed_at)),
            intent.workspace
        )),
        Verdict::Mismatched | Verdict::NotAdmitted => Some(format!(
            "inspect {} for effects before the stop, which are not excluded; `harness show` \
             names what is required and installed, and a retry is a new attempt of run {run}",
            intent.workspace
        )),
        _ => None,
    }
}

/// The judgement, recomputed from the records every time they are read.
fn judge(r: &ReadBack<'_>) -> (Verdict, Vec<String>) {
    let (intent, intent_bytes) = match (r.intent, r.intent_bytes) {
        (Some(i), Some(b)) => (i, b),
        _ => {
            let mut reasons = vec![match r.outcome {
                Some(word) => format!(
                    "the attempt ended {word} and no intent was recorded; this product attempts \
                     no spawn before its intent is persisted, so it created no provider process \
                     for this attempt"
                ),
                None => "no intent was recorded for this attempt, so this product attempted no \
                         spawn for it"
                    .to_string(),
            }];
            if r.record.is_some() || r.launched.is_some() || r.admission.is_some() {
                reasons.push(
                    "records are present without their intent, and a record read without its \
                     intent is not evidence of a launch"
                        .to_string(),
                );
            }
            return (Verdict::NotLaunched, reasons);
        }
    };
    let intent_digest = digest_bytes(intent_bytes);

    // The confirmation and the decision must belong to this intent before
    // either is believed.
    let mut unbound = Vec::new();
    if intent.attempt != *r.identity {
        unbound.push(format!(
            "the intent names run {} attempt {}",
            intent.attempt.run_id, intent.attempt.attempt
        ));
    }
    if let Some(l) = r.launched {
        if l.attempt != *r.identity || l.intent_digest != intent_digest {
            unbound.push("the spawn confirmation is not this intent's".to_string());
        }
    }
    if let Some(a) = r.admission {
        if a.attempt != *r.identity || a.intent_digest != intent_digest {
            unbound.push("the startup decision is not this intent's".to_string());
        }
        if a.decision == Decision::Admitted
            && !matches!(a.observation, HarnessObservation::Correlated { .. })
        {
            unbound.push("the decision admits without a correlated acknowledgment".to_string());
        }
    }

    let Some(record) = r.record else {
        if !unbound.is_empty() {
            let mut reasons = vec!["the records do not agree with themselves".to_string()];
            reasons.extend(unbound);
            return (Verdict::Unverified, reasons);
        }
        let decision = r
            .admission
            .map_or("no startup decision was recorded".to_string(), |a| {
                format!(
                    "the startup decision was {} at {}",
                    a.decision.describe(),
                    a.decided_at
                )
            });
        return match r.launched {
            Some(l) => {
                let mut reasons = vec![
                    format!(
                        "a process ({}) was created at {} and given its prompt, and no \
                         completion was recorded: this product stopped before it could record \
                         one, so the outcome is unknown and effects are possible",
                        l.pid, l.confirmed_at
                    ),
                    decision,
                ];
                if let Some(gate) = r.gate {
                    reasons.push(format!(
                        "the admission gate has been consulted {} time(s){}",
                        gate.len(),
                        if gate.is_empty() {
                            String::new()
                        } else {
                            format!(": {}", gate.join(", "))
                        }
                    ));
                }
                (Verdict::OutcomeUnknown, reasons)
            }
            None => (
                Verdict::LaunchUnknown,
                vec![format!(
                    "an intent was persisted at {} and no spawn confirmation was: this product \
                     stopped between the two, so whether a process was created is unknown",
                    intent.recorded_at
                )],
            ),
        };
    };

    if record.attempt.as_ref() != Some(r.identity) {
        unbound.push("the record names another attempt, or none".to_string());
    }
    let Some(launch) = record.launch.as_ref() else {
        return (
            Verdict::Unverified,
            vec!["the record carries no launch evidence, so it is not a run's record".into()],
        );
    };
    if launch.intent_digest != intent_digest {
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
        unbound
            .push("the resolved identity is not the one the acknowledgment correlated".to_string());
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
    if let HarnessObservation::Correlated { nonce, .. } = &launch.harness {
        if *nonce != intent.nonce {
            unbound.push("the acknowledgment's nonce is not this attempt's".to_string());
        }
    }
    if record.observation.observed() {
        unbound.push(format!(
            "the record carries an observation, and a run attempt carries none: {RUN_OBSERVATION}"
        ));
    }
    // A version 3 record says which earlier records it followed; they must be
    // the ones on disk.
    if let Some(confirmed) = launch.process.confirmed {
        let on_disk = r.launched.map(|l| l.pid);
        match (confirmed, on_disk) {
            (true, Some(pid)) if Some(pid) == launch.process.pid => {}
            (false, None) => {}
            _ => unbound
                .push("the record and the spawn confirmation on disk do not agree".to_string()),
        }
    }
    if launch.admission.as_ref() != r.admission {
        // A decision the record names and the disk lacks, or the reverse, is
        // tolerated only when the record says the decision was not persisted.
        let unpersisted = launch.admission_error.is_some() && r.admission.is_none();
        if !unpersisted {
            unbound.push("the record and the startup decision on disk do not agree".to_string());
        }
    }
    if !unbound.is_empty() {
        let mut reasons = vec!["the records do not agree with themselves".to_string()];
        reasons.extend(unbound);
        return (Verdict::Unverified, reasons);
    }

    if !launch.process.launched {
        return (
            Verdict::SpawnFailed,
            vec![format!(
                "the spawn call failed, and reported that no process was created: {}",
                launch.process.failure.clone().unwrap_or_default()
            )],
        );
    }
    if launch.process.confirmed == Some(false) {
        return (
            Verdict::Interrupted,
            vec![format!(
                "process {} was created and stopped before its prompt was delivered: {}",
                launch
                    .process
                    .pid
                    .map_or("(unknown)".to_string(), |p| p.to_string()),
                launch.process.stopped.clone().unwrap_or_default()
            )],
        );
    }
    if launch.process.outcome.is_none() {
        return (
            Verdict::OutcomeUnknown,
            vec![format!(
                "a process was created and its supervision failed, so its outcome is unknown and \
                 effects are possible: {}",
                launch.process.failure.clone().unwrap_or_default()
            )],
        );
    }

    let refused = launch
        .admission
        .as_ref()
        .filter(|a| a.decision.refuses())
        .map(|a| a.decision);
    if refused.is_some() || (launch.admission_error.is_some() && launch.gate.is_some()) {
        let mut reasons = vec![match &launch.admission {
            Some(a) => format!(
                "the startup decision was {} at {}{}: {}",
                a.decision.describe(),
                a.decided_at,
                a.at_line
                    .map_or(" at the end of the stream".to_string(), |l| format!(
                        ", at stream line {l}"
                    )),
                a.reason
            ),
            None => format!(
                "the startup decision could not be persisted, so the gate held every tool call: {}",
                launch.admission_error.clone().unwrap_or_default()
            ),
        }];
        reasons.extend(boundary(launch));
        let verdict = if refused == Some(Decision::Mismatched) {
            Verdict::Mismatched
        } else {
            Verdict::NotAdmitted
        };
        return (verdict, reasons);
    }
    if launch.process.outcome.as_deref() == Some("interrupted") {
        let mut reasons = vec!["the process did not end by itself as one readable session".into()];
        reasons.extend(launch.process.stopped.clone());
        reasons.extend(launch.process.stream_error.clone());
        reasons.extend(launch.process.surviving_processes.clone());
        return (Verdict::Interrupted, reasons);
    }
    // A version 2 record: no decision, and a mismatch refused after the fact.
    if launch.admission.is_none() {
        if let Standing::Mismatched { .. } = record.standing {
            return (
                Verdict::Mismatched,
                vec![
                    record.standing.refusal().unwrap_or_default(),
                    "written before section 3.32: the refusal came after the session ended, and \
                     nothing gated its work"
                        .to_string(),
                ],
            );
        }
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

/// What a refusal at the startup decision establishes about the boundary, and
/// what it does not (section 3.32 rule 26).
fn boundary(launch: &LaunchEvidence) -> Vec<String> {
    let mut out = vec![match &launch.process.stopped {
        Some(_) => "the launcher stopped the process group at the decision".to_string(),
        None => "the process ended by itself before the launcher stopped it".to_string(),
    }];
    match launch.gate.as_deref() {
        Some([]) | None => out.push(
            "the admission gate was never consulted, so whether the provider routes tool calls \
             through it is not established: effects before termination are not excluded, and \
             this refusal is retrospective for anything the provider did without the gate"
                .to_string(),
        ),
        Some(consultations) => out.push(format!(
            "the admission gate was consulted {} time(s) and answered {}; a tool call routed \
             through it did not run",
            consultations.len(),
            consultations.join(", ")
        )),
    }
    out.push(
        "stopping the process does not prove that nothing happened outside a tool call, or \
         that anything was undone"
            .to_string(),
    );
    out
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

    fn now() -> String {
        "2026-09-22T00:00:02Z".to_string()
    }

    /// A native `hook_response` event carrying `r`.
    fn hook_event(r: &HookResponse) -> ProviderEvent {
        serde_json::from_value(serde_json::json!({
            "type": "system", "subtype": "hook_response",
            "hook_event": r.hook_event, "hook_name": r.hook_name, "stdout": r.stdout,
            "exit_code": r.exit_code, "outcome": r.outcome, "session_id": r.session_id,
        }))
        .unwrap()
    }

    fn init_event(session: &str) -> ProviderEvent {
        serde_json::from_value(serde_json::json!({
            "type": "system", "subtype": "init", "session_id": session,
            "claude_code_version": "2.1.267",
        }))
        .unwrap()
    }

    /// What the run does, without a process: confirm a spawn, show the watch
    /// the startup hook responses and then the init event, and finalize with
    /// the settings bytes `written`. Returns the record, its path, what the
    /// watch established, and why it stopped the process, if it did.
    fn launch_through(
        w: &World,
        p: &Prepared,
        responses: &[HookResponse],
        written: &[u8],
    ) -> std::io::Result<(StartupRecord, PathBuf, Watched, Option<String>)> {
        let now = now;
        let mut watch = LaunchWatch::new(&w.root, &w.layout, &w.manifest, p, &now);
        watch.spawned(4242).unwrap();
        let mut stop = None;
        for (i, event) in responses
            .iter()
            .map(hook_event)
            .chain([init_event("s-1")])
            .enumerate()
        {
            if let Control::Stop(why) = watch.event(&(i + 1, event)) {
                stop = Some(why);
                break;
            }
        }
        let watched = watch.watched();
        let launch = Launch::Spawned {
            hook_responses: responses,
            session_id: Some("s-1"),
            provider_version: Some("2.1.267"),
            settings_written: written,
            outcome: if stop.is_some() {
                "interrupted"
            } else {
                "completed"
            },
            stream_error: None,
            surviving_processes: None,
            stopped: stop.clone(),
        };
        let (record, path) = finalize(
            &w.root,
            &w.layout,
            &w.manifest,
            p,
            &launch,
            &watched,
            "2026-09-22T00:00:03Z",
        )?;
        Ok((record, path, watched, stop))
    }

    fn document(p: &Prepared) -> Vec<u8> {
        p.settings_document().into_bytes()
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
        let (record, _, watched, stop) = launch_through(&w, &p, &responses, &document(&p)).unwrap();
        assert_eq!(stop, None);
        let admission = watched.admission.unwrap();
        assert_eq!(admission.decision, Decision::Admitted);
        assert_eq!(admission.at_line, Some(2), "decided at the init event");
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
            HarnessObservation::Correlated { .. }
        ));
    }

    #[test]
    fn a_corrupt_answering_revision_reads_as_what_it_now_is() {
        let w = world(true);
        let p = prepared(&w, 1);
        let root = required_root(&w);
        let line = ack(&p.intent, &root);
        std::fs::write(root.join("rules/statecraft-governed-work.md"), "changed\n").unwrap();
        let (record, _, watched, stop) =
            launch_through(&w, &p, &[response(&line)], &document(&p)).unwrap();
        // Section 3.32 rule 26: a revision that no longer digests to the
        // requirement establishes nothing, so governed work is withheld.
        assert_eq!(
            watched.admission.unwrap().decision,
            Decision::NotEstablished
        );
        assert!(stop.is_some());
        // Observed is what the directory digests to now, and the required
        // directory itself no longer digests to the requirement.
        assert_ne!(record.resolved_harness.as_deref(), Some(w.digest.as_str()));
        assert_eq!(record.standing.word(), "corrupt");
        assert!(!record.qualifies());
        let shown = inspect(&w.root, "003-x", Some(1), &facts(1)).unwrap();
        assert_eq!(shown.verdict, Verdict::NotAdmitted, "{:?}", shown.reasons);
    }

    #[test]
    fn a_mismatched_revision_refuses_the_attempt() {
        let w = world(true);
        let p = prepared(&w, 1);
        let other = w.layout.harness_revision_dir("h-bbbbbbbbbbbb");
        std::fs::create_dir_all(other.join("hooks")).unwrap();
        std::fs::write(other.join("hooks/statecraft-session-start.sh"), "older").unwrap();
        let (record, _, watched, stop) =
            launch_through(&w, &p, &[response(&ack(&p.intent, &other))], &document(&p)).unwrap();
        assert_eq!(watched.admission.unwrap().decision, Decision::Mismatched);
        assert!(stop.unwrap().contains("refused governed work"));
        assert_eq!(record.standing.word(), "mismatched");
        assert_eq!(
            refuses_the_attempt(&record).map(|(guard, _)| guard),
            Some(HARNESS_IDENTITY_GUARD)
        );
        let shown = inspect(&w.root, "003-x", None, &facts(1)).unwrap();
        assert_eq!(shown.verdict, Verdict::Mismatched);
        // No gate was consulted here, and the answer does not pretend one was.
        assert!(
            shown.reasons.iter().any(|r| r.contains("never consulted")),
            "{:?}",
            shown.reasons
        );
        assert!(shown.next.is_some());
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
        // Nothing is registered: the document is the floor alone, and no
        // gate exists to hold anything.
        assert!(p.intent.registrations.is_empty());
        assert_eq!(p.settings_document(), crate::session::payload_json());
        assert!(!p.intent.attempt.gate_path(&w.root).exists());
        let line = ack(&p.intent, &required_root(&w));
        let (record, _, watched, stop) =
            launch_through(&w, &p, &[response(&line)], &document(&p)).unwrap();
        assert_eq!(watched.admission.unwrap().decision, Decision::NotGated);
        assert_eq!(stop, None);
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
        launch_through(
            &w,
            &first,
            &[response(&ack(&first.intent, &required_root(&w)))],
            &document(&first),
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
                &Watched::default(),
                "t"
            )
            .is_err()
        );

        // A second attempt that replays the first attempt's acknowledgment
        // does not inherit its observation, and the first record is intact.
        let second = prepared(&w, 2);
        assert_ne!(second.intent.nonce, first.intent.nonce);
        let replay = ack(&first.intent, &required_root(&w));
        let (record, _, _, stop) =
            launch_through(&w, &second, &[response(&replay)], &document(&second)).unwrap();
        assert!(stop.is_some(), "a replayed acknowledgment released work");
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
        assert_eq!(latest.verdict, Verdict::NotAdmitted);
        assert!(
            latest.reasons.iter().any(|r| r.contains("replayed")),
            "{:?}",
            latest.reasons
        );
    }

    #[test]
    fn an_intent_alone_is_launch_unknown_and_nothing_fabricates_a_record() {
        let w = world(true);
        let p = prepared(&w, 1);
        let shown = inspect(
            &w.root,
            "003-x",
            None,
            &[AttemptFact {
                number: 1,
                outcome: None,
            }],
        )
        .unwrap();
        assert_eq!(shown.spawn, Spawn::Unknown);
        assert_eq!(shown.verdict, Verdict::LaunchUnknown, "{:?}", shown.reasons);
        assert!(shown.verdict.uncertain());
        assert!(
            !shown.reasons.iter().any(|r| r.contains("interrupted")),
            "an intent was read as an interruption: {:?}",
            shown.reasons
        );
        assert!(
            shown
                .next
                .as_deref()
                .unwrap()
                .contains("no verb that reconciles")
        );
        assert!(!p.intent.attempt.record_path(&w.root).exists());
        assert!(!p.intent.attempt.launched_path(&w.root).exists());
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
        assert_eq!(shown.spawn, Spawn::NotAttempted);
        assert_eq!(shown.verdict, Verdict::NotLaunched);
        assert!(matches!(
            inspect(&w.root, "003-x", Some(4), &facts(1)),
            Err(NotRead::NoSuchAttempt(_))
        ));
    }

    #[test]
    fn a_failed_spawn_supplies_nothing_and_is_spawn_failed() {
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
            &Watched::default(),
            "t",
        )
        .unwrap();
        assert_eq!(record.supply.word(), "failed");
        assert_eq!(record.resolved_harness, None);
        let shown = inspect(&w.root, "003-x", None, &facts(1)).unwrap();
        assert_eq!(shown.spawn, Spawn::Failed);
        assert_eq!(shown.verdict, Verdict::SpawnFailed);
    }

    #[test]
    fn other_settings_bytes_are_a_failed_supply() {
        let w = world(true);
        let p = prepared(&w, 1);
        let (record, _, _, _) = launch_through(&w, &p, &[], b"{}").unwrap();
        assert_eq!(record.supply.word(), "failed");
        assert!(record.supply.describe().contains("document"));
    }

    /// Section 3.31 rule 21: the judgement is the bytes', every time, and a
    /// hand edit changes it rather than asserting one.
    #[test]
    fn a_record_read_back_is_judged_again_and_hand_edits_do_not_qualify_it() {
        let w = world(true);
        let p = prepared(&w, 1);
        let (_, path, _, _) = launch_through(
            &w,
            &p,
            &[response(&ack(&p.intent, &required_root(&w)))],
            &document(&p),
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

    // ---- Section 3.32: crash and failure injection around the launch ----

    fn live(n: u32) -> Vec<AttemptFact> {
        let mut f = facts(n - 1);
        f.push(AttemptFact {
            number: n,
            outcome: None,
        });
        f
    }

    /// Put a directory where a record would go, so its exclusive creation
    /// fails deterministically.
    fn block(path: &Path) {
        std::fs::create_dir_all(path).unwrap();
    }

    /// Crash after the confirmation: a process is known to exist, and nothing
    /// says how it ended.
    #[test]
    fn a_confirmed_spawn_with_no_record_is_outcome_unknown_and_names_its_process() {
        let w = world(true);
        let p = prepared(&w, 1);
        let now = now;
        let mut watch = LaunchWatch::new(&w.root, &w.layout, &w.manifest, &p, &now);
        watch.spawned(31337).unwrap();
        // This product stops here.
        let shown = inspect(&w.root, "003-x", None, &live(1)).unwrap();
        assert_eq!(shown.spawn, Spawn::Confirmed);
        assert_eq!(
            shown.verdict,
            Verdict::OutcomeUnknown,
            "{:?}",
            shown.reasons
        );
        assert!(shown.reasons[0].contains("31337"));
        assert!(shown.reasons[0].contains("effects are possible"));
        assert!(
            shown
                .reasons
                .iter()
                .any(|r| r.contains("no startup decision"))
        );
        let next = shown.next.clone().unwrap();
        assert!(
            next.contains("31337") && next.contains(&p.intent.workspace),
            "{next}"
        );
        assert!(shown.describe().contains("outcome-unknown"));
    }

    /// Crash between the spawn and its confirmation: a process exists, and the
    /// records say only that whether one exists is unknown. They neither deny
    /// it nor claim it.
    #[test]
    fn a_process_created_before_its_confirmation_is_persisted_reads_as_launch_unknown() {
        let w = world(true);
        let p = prepared(&w, 1);
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .unwrap();
        // The spawn returned; this product stops before the watch hears of it.
        let shown = inspect(&w.root, "003-x", None, &live(1)).unwrap();
        let alive = child.try_wait().unwrap().is_none();
        let _ = child.kill();
        let _ = child.wait();
        assert!(alive, "the fixture's process ended before it was inspected");
        assert_eq!(shown.spawn, Spawn::Unknown);
        assert_eq!(shown.verdict, Verdict::LaunchUnknown);
        assert!(
            shown.reasons[0].contains("whether a process was created is unknown"),
            "{:?}",
            shown.reasons
        );
        assert!(!p.intent.attempt.launched_path(&w.root).exists());
    }

    /// The confirmation cannot be persisted: the real supervisor stops the
    /// process before its prompt is delivered, and the record says so.
    #[test]
    fn a_confirmation_that_cannot_be_persisted_stops_the_process_before_its_prompt() {
        use statecraft_adapter::environment::{Blueprint, CheckSuiteCommands, construct};
        use std::os::unix::fs::PermissionsExt;

        let w = world(true);
        let p = prepared(&w, 1);
        block(&p.intent.attempt.launched_path(&w.root));
        let bin = tempfile::tempdir().unwrap();
        let program = bin.path().join("provider");
        std::fs::write(
            &program,
            "#!/bin/sh\ncat > got-prompt\necho '{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s-1\"}'\ntouch finished\n",
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let invocation =
            statecraft_adapter_claude_code::Invocation::new(program.to_str().unwrap(), &[], None);
        let request = statecraft_adapter::protocol::Request {
            workspace: w.workspace.clone(),
            base_commit: "0".repeat(40),
            prompt: b"the governed work".to_vec(),
            capabilities: statecraft_adapter::capability::Requested::none(),
            deadline_seconds: 30,
            attempt: statecraft_adapter::protocol::AttemptIdentity {
                run_id: "003-x".into(),
                number: 1,
            },
        };
        let environment = construct(
            &Blueprint::empty().allowing("PATH", "/usr/bin:/bin"),
            &CheckSuiteCommands(vec![]),
        );
        let now = now;
        let mut watch = LaunchWatch::new(&w.root, &w.layout, &w.manifest, &p, &now);
        let execution = statecraft_adapter_claude_code::execution::supervise_with(
            &invocation,
            &request,
            &environment,
            &[],
            &mut watch,
        )
        .unwrap();
        assert!(watch.spawned.is_some());
        assert!(watch.confirmation_error.is_some());
        assert!(execution.supervised.stopped.is_some());
        let got = std::fs::read(w.workspace.join("got-prompt")).unwrap_or_default();
        assert!(got.is_empty(), "the prompt reached an unconfirmed process");
        assert!(!w.workspace.join("finished").exists());

        let watched = watch.watched();
        let (record, _) = finalize(
            &w.root,
            &w.layout,
            &w.manifest,
            &p,
            &Launch::Spawned {
                hook_responses: &execution.hook_responses,
                session_id: execution.session_id.as_deref(),
                provider_version: None,
                settings_written: &execution.settings_written,
                outcome: execution.supervised.outcome.word(),
                stream_error: None,
                surviving_processes: execution.supervised.surviving_processes.clone(),
                stopped: execution.supervised.stopped.clone(),
            },
            &watched,
            "t",
        )
        .unwrap();
        assert_eq!(
            record.launch.as_ref().unwrap().process.confirmed,
            Some(false)
        );
        std::fs::remove_dir(p.intent.attempt.launched_path(&w.root)).unwrap();
        let shown = inspect(&w.root, "003-x", None, &facts(1)).unwrap();
        assert_eq!(shown.spawn, Spawn::Unconfirmed);
        assert_eq!(shown.verdict, Verdict::Interrupted, "{:?}", shown.reasons);
        assert!(shown.reasons[0].contains("before its prompt was delivered"));
    }

    /// The decision cannot be persisted: the gate cannot read one, so it
    /// holds every tool call, and the session is stopped and refused.
    #[test]
    fn a_decision_that_cannot_be_persisted_withholds_work_and_stops_the_session() {
        let w = world(true);
        let p = prepared(&w, 1);
        block(&p.intent.attempt.admission_path(&w.root));
        let (record, _, watched, stop) = launch_through(
            &w,
            &p,
            &[response(&ack(&p.intent, &required_root(&w)))],
            &document(&p),
        )
        .unwrap();
        assert!(watched.admission_error.is_some());
        assert!(stop.unwrap().contains("could not be persisted"));
        let (guard, _) = refuses_the_attempt(&record).unwrap();
        assert_eq!(guard, STARTUP_ADMISSION_GUARD);
        std::fs::remove_dir(p.intent.attempt.admission_path(&w.root)).unwrap();
        let shown = inspect(&w.root, "003-x", None, &facts(1)).unwrap();
        assert_eq!(shown.verdict, Verdict::NotAdmitted, "{:?}", shown.reasons);
    }

    /// The final record cannot be persisted: what is on disk is a confirmed
    /// spawn and a decision, and the outcome is unknown rather than inferred.
    #[test]
    fn a_record_that_cannot_be_persisted_leaves_the_outcome_unknown() {
        let w = world(true);
        let p = prepared(&w, 1);
        block(&p.intent.attempt.record_path(&w.root));
        assert!(
            launch_through(
                &w,
                &p,
                &[response(&ack(&p.intent, &required_root(&w)))],
                &document(&p),
            )
            .is_err()
        );
        std::fs::remove_dir(p.intent.attempt.record_path(&w.root)).unwrap();
        let shown = inspect(&w.root, "003-x", None, &live(1)).unwrap();
        assert_eq!(
            shown.verdict,
            Verdict::OutcomeUnknown,
            "{:?}",
            shown.reasons
        );
        assert_eq!(
            shown.admission.as_ref().unwrap().decision,
            Decision::Admitted
        );
        assert!(
            shown.reasons.iter().any(|r| r.contains("admitted")),
            "{:?}",
            shown.reasons
        );
    }

    /// The decision is made at the first event after the startup hooks, and
    /// not at any hook event before it.
    #[test]
    fn the_decision_is_made_at_the_first_event_after_the_startup_hooks() {
        let w = world(true);
        let p = prepared(&w, 1);
        let now = now;
        let mut watch = LaunchWatch::new(&w.root, &w.layout, &w.manifest, &p, &now);
        watch.spawned(1).unwrap();
        let admission = p.intent.attempt.admission_path(&w.root);
        let started: ProviderEvent = serde_json::from_value(serde_json::json!({
            "type": "system", "subtype": "hook_started", "hook_event": "SessionStart",
            "hook_name": "SessionStart:startup", "session_id": "s-1"
        }))
        .unwrap();
        assert_eq!(watch.event(&(1, started)), Control::Continue);
        assert!(!admission.exists());
        let hook = hook_event(&response(&ack(&p.intent, &required_root(&w))));
        assert_eq!(watch.event(&(2, hook)), Control::Continue);
        assert!(!admission.exists(), "decided before the hooks were done");
        assert_eq!(watch.event(&(3, init_event("s-1"))), Control::Continue);
        let written: Admission =
            serde_json::from_str(&std::fs::read_to_string(&admission).unwrap()).unwrap();
        assert_eq!(written.decision, Decision::Admitted);
        assert_eq!(written.at_line, Some(3));
        // One line of JSON, which is what the gate reads.
        assert_eq!(
            std::fs::read_to_string(&admission).unwrap().lines().count(),
            1
        );
        // Later events change nothing.
        let later = hook_event(&response("statecraft-startup\tv1\tjunk"));
        assert_eq!(watch.event(&(4, later)), Control::Continue);
        assert_eq!(watch.admission.as_ref().unwrap(), &written);
    }

    /// A stream that ends before its decision point is decided at its end.
    #[test]
    fn a_stream_that_ends_before_its_decision_point_is_decided_at_its_end() {
        let w = world(true);
        let p = prepared(&w, 1);
        let now = now;
        let mut watch = LaunchWatch::new(&w.root, &w.layout, &w.manifest, &p, &now);
        watch.spawned(1).unwrap();
        let why = watch.decide_at_end().unwrap();
        assert!(why.contains("not-established"), "{why}");
        assert_eq!(watch.admission.as_ref().unwrap().at_line, None);
        // Deciding again changes nothing.
        assert_eq!(watch.decide_at_end(), None);
    }

    /// Anything already in an attempt's directory refuses preparation, and an
    /// existing record is never replaced.
    #[test]
    fn evidence_already_on_disk_refuses_preparation_and_is_never_replaced() {
        let w = world(true);
        let attempt = AttemptIdentity {
            run_id: "003-x".into(),
            attempt: 1,
        };
        for file in [
            files::LAUNCHED,
            files::ADMISSION,
            files::RECORD,
            files::GATE,
        ] {
            let path = attempt.directory(&w.root).join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "prior evidence").unwrap();
            let err = prepare(&Preparation {
                root: &w.root,
                workspace: &w.workspace,
                base_commit: "x",
                attempt: attempt.clone(),
                recorded_at: "t",
                layout: &w.layout,
                manifest: &w.manifest,
                adapter: adapter(),
                program: "p",
            })
            .unwrap_err();
            assert!(matches!(err, NotPrepared::Exists { .. }), "{file}: {err}");
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "prior evidence");
            assert!(write_once(&path, "new").is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "prior evidence");
            std::fs::remove_file(&path).unwrap();
        }
        // No temporary file is left behind by a refused write.
        let leftovers: Vec<_> = std::fs::read_dir(attempt.directory(&w.root))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains("partial"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    /// Section 3.32 rule 25: the document registers the selected revision's
    /// startup hook and this attempt's gate, beside the floor, and nothing
    /// else; the adapter accepts it byte for byte; and its identity is not the
    /// floor's.
    #[test]
    fn the_settings_document_registers_the_selected_hook_and_the_gate_and_nothing_else() {
        let w = world(true);
        let p = prepared(&w, 1);
        let text = p.settings_document();
        let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
        let mut keys: Vec<_> = doc.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["hooks", "permissions"]);
        assert_eq!(doc["permissions"], crate::session::payload()["permissions"]);
        let hook = required_root(&w).join("hooks/statecraft-session-start.sh");
        assert_eq!(
            doc["hooks"]["SessionStart"][0]["hooks"][0]["command"],
            format!("'{}'", hook.display())
        );
        assert_eq!(doc["hooks"]["SessionStart"][0]["matcher"], "startup");
        let gate = std::fs::canonicalize(p.intent.attempt.gate_path(&w.root)).unwrap();
        assert_eq!(
            doc["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            format!("'{}' {GATE_WAIT_TENTHS}", gate.display())
        );
        assert_eq!(doc["hooks"]["PreToolUse"][0]["matcher"], "*");
        assert_eq!(doc["hooks"].as_object().unwrap().len(), 2);
        assert_eq!(p.intent.payload.digest, digest_bytes(text.as_bytes()));
        assert_eq!(
            p.intent.payload.floor_digest.as_deref(),
            Some(crate::startup::payload_identity().as_str())
        );
        assert_ne!(p.intent.payload.digest, crate::startup::payload_identity());
        assert_eq!(
            p.intent.registrations[0].script_digest,
            digest_bytes(&std::fs::read(&hook).unwrap())
        );
        assert_eq!(
            std::fs::read_to_string(&gate).unwrap(),
            GATE_SCRIPT,
            "the gate on disk is the launcher's"
        );
        let floor: Vec<String> = crate::settings::DENY_FLOOR
            .iter()
            .map(|r| (*r).to_string())
            .collect();
        let invocation = statecraft_adapter_claude_code::Invocation::new("claude", &floor, None)
            .with_hooks(p.intent.hooks().unwrap())
            .with_settings_document(text.clone());
        assert!(invocation.is_ok(), "{invocation:?}");
    }

    /// A record written before section 3.32 is read, its grade read as
    /// correlated, and it is judged without being qualified or rewritten.
    #[test]
    fn a_record_written_before_section_3_32_is_read_and_never_qualified() {
        let w = world(true);
        let p = prepared(&w, 1);
        let (_, record_path, _, _) = launch_through(
            &w,
            &p,
            &[response(&ack(&p.intent, &required_root(&w)))],
            &document(&p),
        )
        .unwrap();
        // Reduce both records to the version 2 shape.
        let mut intent: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&p.path).unwrap()).unwrap();
        let o = intent.as_object_mut().unwrap();
        o.remove("settingsDocument");
        o.remove("registrations");
        o.insert("version".into(), 2.into());
        intent["payload"]
            .as_object_mut()
            .unwrap()
            .remove("floorDigest");
        let intent_bytes = serde_json::to_vec_pretty(&intent).unwrap();
        std::fs::write(&p.path, &intent_bytes).unwrap();
        let mut record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&record_path).unwrap()).unwrap();
        record["version"] = 2.into();
        let launch = record["launch"].as_object_mut().unwrap();
        launch.remove("admission");
        launch.remove("gate");
        launch.insert("intentDigest".into(), digest_bytes(&intent_bytes).into());
        let process = launch["process"].as_object_mut().unwrap();
        process.remove("pid");
        process.remove("confirmed");
        process.remove("stopped");
        launch["harness"]["grade"] = "acknowledged".into();
        let payload = launch["payload"].as_object_mut().unwrap();
        payload.remove("floorDigest");
        let before = serde_json::to_vec_pretty(&record).unwrap();
        std::fs::write(&record_path, &before).unwrap();
        for file in [files::LAUNCHED, files::ADMISSION, files::GATE_LOG] {
            let _ = std::fs::remove_file(p.intent.attempt.directory(&w.root).join(file));
        }

        let shown = inspect(&w.root, "003-x", None, &facts(1)).unwrap();
        assert_eq!(shown.verdict, Verdict::Unverified, "{:?}", shown.reasons);
        assert_eq!(shown.spawn, Spawn::Confirmed);
        assert!(matches!(
            shown
                .record
                .as_ref()
                .unwrap()
                .launch
                .as_ref()
                .unwrap()
                .harness,
            HarnessObservation::Correlated { .. }
        ));
        assert_eq!(
            std::fs::read(&record_path).unwrap(),
            before,
            "a read rewrote it"
        );
    }

    // ---- The admission gate, run as the provider would run it ----

    fn gate_in(dir: &Path) -> PathBuf {
        let gate = dir.join(files::GATE);
        std::fs::write(&gate, GATE_SCRIPT).unwrap();
        make_executable(&gate).unwrap();
        gate
    }

    fn consult(gate: &Path, wait_tenths: u32) -> std::process::Output {
        use std::io::Write;
        let mut child = std::process::Command::new(gate)
            .arg(wait_tenths.to_string())
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(br#"{"tool_name":"Bash","tool_input":{"command":"touch x"}}"#)
            .unwrap();
        child.wait_with_output().unwrap()
    }

    fn decision_line(decision: Decision) -> String {
        let a = Admission {
            version: LAUNCH_VERSION,
            attempt: AttemptIdentity {
                run_id: "r".into(),
                attempt: 1,
            },
            decision,
            reason: "because \"decision\":\"admitted\" is only text here".into(),
            decided_at: "t".into(),
            at_line: Some(1),
            observation: unverified(Unverified::Absent, "x"),
            intent_digest: "d".into(),
        };
        format!("{}\n", serde_json::to_string(&a).unwrap())
    }

    #[test]
    fn the_gate_releases_a_tool_call_only_on_an_admitted_decision() {
        let dir = tempfile::tempdir().unwrap();
        let gate = gate_in(dir.path());
        let admission = dir.path().join(files::ADMISSION);
        let log = dir.path().join(files::GATE_LOG);

        // No decision: withheld after the wait.
        let out = consult(&gate, 2);
        assert_eq!(out.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&out.stderr).contains("withheld"));

        // Refused, including a refusal whose reason mentions admission.
        for refused in [Decision::Mismatched, Decision::NotEstablished] {
            std::fs::write(&admission, decision_line(refused)).unwrap();
            let out = consult(&gate, 2);
            assert_eq!(out.status.code(), Some(2), "{refused:?}");
        }
        std::fs::write(&admission, decision_line(Decision::Admitted)).unwrap();
        assert_eq!(consult(&gate, 2).status.code(), Some(0));
        let consulted = std::fs::read_to_string(&log).unwrap();
        let words: Vec<_> = consulted
            .lines()
            .map(|l| l.split(':').next().unwrap())
            .collect();
        assert_eq!(words, ["withheld", "refused", "refused", "admitted"]);
    }

    /// A tool call that arrives before the decision waits for it: it does not
    /// run until the decision exists, and runs once it says admitted.
    #[test]
    fn a_tool_call_before_the_decision_waits_for_it() {
        let dir = tempfile::tempdir().unwrap();
        let gate = gate_in(dir.path());
        let admission = dir.path().join(files::ADMISSION);
        let started = std::time::Instant::now();
        let waiting = {
            let gate = gate.clone();
            std::thread::spawn(move || consult(&gate, 100))
        };
        std::thread::sleep(std::time::Duration::from_millis(400));
        assert!(
            !waiting.is_finished(),
            "the gate released before any decision"
        );
        std::fs::write(&admission, decision_line(Decision::Admitted)).unwrap();
        let out = waiting.join().unwrap();
        assert_eq!(out.status.code(), Some(0));
        assert!(started.elapsed() >= std::time::Duration::from_millis(400));
    }
}
