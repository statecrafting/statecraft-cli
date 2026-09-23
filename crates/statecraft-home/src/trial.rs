//! The managed-startup trial: one run attempt that asks whether a provider
//! honors the startup hook and the admission gate a managed run supplies.
//!
//! Spec 002 section 3.33, settled by the owner on 2026-09-22.
//!
//! # What this module adds to a run attempt
//!
//! The trial is a `run` attempt (rule 30). This module supplies the four
//! values it differs in, places the read-only sentinel (rule 31), watches the
//! stream beside [`LaunchWatch`] to keep a timeline of what arrived where
//! (rule 34), and judges the attempt's records into the words of rules 35 to
//! 37. It launches nothing, decides nothing about admission, and writes one
//! file: `trial.json`, once.
//!
//! # Procedure, not provider
//!
//! Everything here is exercised against local fakes, faithful and not. A fake
//! that honors the registration tests this procedure; only a provider session
//! observes a provider, and a synthetic record says which it was on every
//! read.

use crate::launch::{
    ACKNOWLEDGMENT, Admission, AttemptIdentity, AttemptPaths, CHILD_ATTESTED, Decision,
    HarnessObservation, LaunchWatch, Unverified, Verdict,
};
use serde::{Deserialize, Serialize};
use statecraft_adapter::supervisor::{Control, Watch};
use statecraft_adapter_claude_code::execution::HookResponse;
use statecraft_adapter_claude_code::stream::ProviderEvent;
use statecraft_environment::digest::digest_bytes;
use std::path::{Path, PathBuf};

/// The trial's run id: fixed, so one project holds at most one trial (rule 32).
pub const RUN_ID: &str = "statecraft-startup-trial";

/// The sentinel's file name, in the attempt's workspace (rule 31).
pub const SENTINEL: &str = "STATECRAFT-TRIAL-SENTINEL";

/// The sentinel line's first field.
pub const SENTINEL_PREFIX: &str = "statecraft-trial-sentinel";

/// The turn limit (rule 30): a read and a reply, with one turn to spare.
pub const MAX_TURNS: u32 = 3;

/// The session deadline when the operator names none, in seconds.
pub const DEFAULT_DEADLINE_SECONDS: u64 = 120;

/// The longest session deadline the trial accepts, in seconds.
pub const MAX_DEADLINE_SECONDS: u64 = 300;

/// The trial's record, beside section 3.32's four.
pub const FILE: &str = "trial.json";

/// The schema version of `trial.json`.
pub const TRIAL_VERSION: u32 = 1;

/// What every effects word is scoped to (rule 36).
pub const SCOPE: &str = "tool calls only: the provider's own startup, other hooks and anything \
                         outside a tool call are not covered by any word here";

/// Whether a provider ran, as the operator stated it (rule 33).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    /// The operator stated that a provider session runs.
    ProviderSession,
    /// The operator stated that the executable is a local fake.
    Synthetic,
}

impl Origin {
    /// The word.
    pub fn word(self) -> &'static str {
        match self {
            Origin::ProviderSession => "provider-session",
            Origin::Synthetic => "synthetic",
        }
    }
}

/// The prompt the trial delivers (rule 31). Asks for one read and a reply.
pub fn prompt() -> String {
    format!(
        "Use the Read tool exactly once to read the file {SENTINEL} in the current directory. \
         Then reply with its contents and nothing else. Do not use any other tool."
    )
}

/// Why a trial cannot be started, before anything is prepared.
pub fn refuses_deadline(seconds: u64) -> Option<String> {
    (seconds == 0 || seconds > MAX_DEADLINE_SECONDS).then(|| {
        format!("the trial's deadline must be between 1 and {MAX_DEADLINE_SECONDS} seconds, not {seconds}")
    })
}

/// The sentinel, as placed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sentinel {
    /// Where it is.
    pub path: String,
    /// The nonce its line carries, and nothing else carries.
    pub nonce: String,
    /// SHA-256 of its bytes.
    pub digest: String,
}

/// Place the sentinel in `workspace`, refusing a path that already exists.
pub fn place_sentinel(workspace: &Path) -> std::io::Result<Sentinel> {
    use std::io::Write;
    let path = workspace.join(SENTINEL);
    let nonce = crate::launch::fresh_nonce()?;
    let line = format!("{SENTINEL_PREFIX} {nonce}\n");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    file.write_all(line.as_bytes())?;
    file.sync_all()?;
    Ok(Sentinel {
        path: path.display().to_string(),
        nonce,
        digest: digest_bytes(line.as_bytes()),
    })
}

// ---------------------------------------------------------------------------
// The timeline.
// ---------------------------------------------------------------------------

/// One stream event, as the trial keeps it: by line, and only what the
/// judgement or an operator needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Entry {
    /// A hook's start.
    #[serde(rename = "hook-started")]
    HookStarted {
        /// Stream line.
        line: usize,
        /// Which hook event.
        event: Option<String>,
    },
    /// A hook's response.
    #[serde(rename = "hook-response")]
    HookResponse {
        /// Stream line.
        line: usize,
        /// Which hook event.
        event: Option<String>,
        /// Its exit code.
        exit_code: Option<i64>,
        /// Whether its output held a line that begins the acknowledgment.
        acknowledgment: bool,
    },
    /// The init event.
    Init {
        /// Stream line.
        line: usize,
        /// The session it names.
        session_id: Option<String>,
        /// The provider version it reports.
        version: Option<String>,
    },
    /// An assistant's request to run a tool.
    #[serde(rename = "tool-use")]
    ToolUse {
        /// Stream line.
        line: usize,
        /// The tool-use id.
        id: String,
        /// Which tool.
        name: String,
    },
    /// What came back for a tool use.
    #[serde(rename = "tool-result")]
    ToolResult {
        /// Stream line.
        line: usize,
        /// The tool-use id it answers.
        id: String,
        /// Neither an error nor carrying a non-execution note.
        executed: bool,
        /// Whether its text carries the sentinel's nonce.
        nonce: bool,
    },
    /// The terminal event.
    Terminal {
        /// Stream line.
        line: usize,
    },
    /// Anything else, by the kind that names it.
    Other {
        /// Stream line.
        line: usize,
        /// What it was.
        event: String,
    },
}

impl Entry {
    /// Its stream line.
    pub fn line(&self) -> usize {
        match self {
            Entry::HookStarted { line, .. }
            | Entry::HookResponse { line, .. }
            | Entry::Init { line, .. }
            | Entry::ToolUse { line, .. }
            | Entry::ToolResult { line, .. }
            | Entry::Terminal { line }
            | Entry::Other { line, .. } => *line,
        }
    }

    /// What kind of event it was, in one word, for the decision point.
    pub fn word(&self) -> String {
        match self {
            Entry::HookStarted { event, .. } => {
                format!("hook-started:{}", event.as_deref().unwrap_or("unnamed"))
            }
            Entry::HookResponse { event, .. } => {
                format!("hook-response:{}", event.as_deref().unwrap_or("unnamed"))
            }
            Entry::Init { .. } => "init".to_string(),
            Entry::ToolUse { .. } => "tool-use".to_string(),
            Entry::ToolResult { .. } => "tool-result".to_string(),
            Entry::Terminal { .. } => "result".to_string(),
            Entry::Other { event, .. } => event.clone(),
        }
    }

    fn is_session_start_response(&self) -> bool {
        matches!(self, Entry::HookResponse { event: Some(e), .. } if e == "SessionStart")
    }
}

/// The entries one event contributes.
fn entries_of(line: usize, event: &ProviderEvent, nonce: &str) -> Vec<Entry> {
    match event {
        ProviderEvent::System(s) if s.is_init() => vec![Entry::Init {
            line,
            session_id: s.session_id.clone(),
            version: s.claude_code_version.clone(),
        }],
        ProviderEvent::System(s) if s.is_hook_response() => HookResponse::of(event)
            .into_iter()
            .map(|r| Entry::HookResponse {
                line,
                event: r.hook_event.clone(),
                exit_code: r.exit_code,
                acknowledgment: r
                    .stdout
                    .as_deref()
                    .unwrap_or_default()
                    .lines()
                    .any(|l| l.starts_with(ACKNOWLEDGMENT)),
            })
            .collect(),
        ProviderEvent::System(s) if s.is_hook() => vec![Entry::HookStarted {
            line,
            event: s.hook_event.clone(),
        }],
        ProviderEvent::System(s) => vec![Entry::Other {
            line,
            event: format!("system:{}", s.subtype),
        }],
        ProviderEvent::Assistant(m) => match m.tool_uses() {
            Ok(uses) if !uses.is_empty() => uses
                .into_iter()
                .map(|u| Entry::ToolUse {
                    line,
                    id: u.id,
                    name: u.name,
                })
                .collect(),
            Ok(_) => vec![Entry::Other {
                line,
                event: "assistant".to_string(),
            }],
            Err(e) => vec![Entry::Other {
                line,
                event: format!("assistant: unreadable tool use: {e}"),
            }],
        },
        ProviderEvent::User(m) => match m.tool_results() {
            Ok(results) if !results.is_empty() => results
                .into_iter()
                .map(|r| {
                    let noted = m
                        .tool_result_meta
                        .iter()
                        .any(|n| n.id == r.tool_use_id && n.non_execution_kind.is_some());
                    Entry::ToolResult {
                        line,
                        executed: !r.is_error && !noted,
                        nonce: r.text().is_some_and(|t| t.contains(nonce)),
                        id: r.tool_use_id,
                    }
                })
                .collect(),
            Ok(_) => vec![Entry::Other {
                line,
                event: "user".to_string(),
            }],
            Err(e) => vec![Entry::Other {
                line,
                event: format!("user: unreadable tool result: {e}"),
            }],
        },
        ProviderEvent::Result(_) => vec![Entry::Terminal { line }],
        ProviderEvent::Unknown => vec![Entry::Other {
            line,
            event: "unknown".to_string(),
        }],
    }
}

/// Where the startup decision was made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionPoint {
    /// The stream line, or `None` when the stream ended first.
    pub line: Option<usize>,
    /// The kind of event at that line (premise P5 of rule 29's ordering).
    pub event: String,
}

/// Watches a trial: forwards everything to the attempt's [`LaunchWatch`],
/// unchanged, and keeps the timeline beside it.
pub struct TrialWatch<'w, 'a> {
    inner: &'w mut LaunchWatch<'a>,
    nonce: String,
    /// Every event, as kept.
    pub entries: Vec<Entry>,
    /// Where the inner watch decided, once it has.
    pub decision: Option<DecisionPoint>,
}

impl<'w, 'a> TrialWatch<'w, 'a> {
    /// A trial's watch around the attempt's own.
    pub fn new(inner: &'w mut LaunchWatch<'a>, sentinel: &Sentinel) -> Self {
        Self {
            inner,
            nonce: sentinel.nonce.clone(),
            entries: Vec::new(),
            decision: None,
        }
    }

    fn decided(&self) -> bool {
        self.inner.admission.is_some() || self.inner.admission_error.is_some()
    }

    /// The inner watch's end-of-stream decision, recorded as made at the end.
    pub fn decide_at_end(&mut self) -> Option<String> {
        let before = self.decided();
        let why = self.inner.decide_at_end();
        if !before && self.decided() {
            self.decision = Some(DecisionPoint {
                line: None,
                event: "end of stream".to_string(),
            });
        }
        why
    }
}

impl Watch<(usize, ProviderEvent)> for TrialWatch<'_, '_> {
    fn spawned(&mut self, pid: u32) -> Result<(), String> {
        self.inner.spawned(pid)
    }

    fn event(&mut self, item: &(usize, ProviderEvent)) -> Control {
        let (line, event) = item;
        let kept = entries_of(*line, event, &self.nonce);
        let before = self.decided();
        let control = self.inner.event(item);
        if !before && self.decided() {
            self.decision = Some(DecisionPoint {
                line: Some(*line),
                event: kept
                    .first()
                    .map_or_else(|| "unknown".to_string(), Entry::word),
            });
        }
        self.entries.extend(kept);
        control
    }
}

// ---------------------------------------------------------------------------
// The facts, and the judgement.
// ---------------------------------------------------------------------------

/// How the process ended, as the supervisor reported it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessEnd {
    /// Whether a process was spawned at all.
    pub spawned: bool,
    /// The supervisor's outcome word.
    pub outcome: Option<String>,
    /// Why the launcher stopped it, where it did.
    pub stopped: Option<String>,
    /// Why the stream could not be read as a completion, where it could not.
    pub stream_error: Option<String>,
    /// A process that outlived the kill, where one did.
    pub surviving: Option<String>,
    /// Why the supervision itself failed, where it did.
    pub failed: Option<String>,
    /// Whether the supervisor's deadline ended the process, as the supervisor
    /// observed it. Absent from records written before 2026-09-23, which read
    /// as `false` and are judged by the older inference below.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub timed_out: bool,
}

impl ProcessEnd {
    /// Whether the deadline, rather than the process or the launcher, ended it
    /// (rule 37: "a process the deadline stopped").
    ///
    /// The supervisor's own observation decides. A session the deadline stopped
    /// before it wrote an `init` event also carries the adapter's "no init"
    /// stream error, and reading that error as a reason the deadline did not
    /// end it judged such a trial `not-established` instead of `uncertain`.
    /// A record written before the flag existed keeps the older inference, so
    /// it is judged on reload exactly as it was when written.
    pub fn deadline(&self, entries: &[Entry]) -> bool {
        if self.timed_out {
            return self.spawned && self.stopped.is_none() && self.failed.is_none();
        }
        self.spawned
            && self.outcome.as_deref() == Some("interrupted")
            && self.stopped.is_none()
            && self.stream_error.is_none()
            && self.failed.is_none()
            && !entries.iter().any(|e| matches!(e, Entry::Terminal { .. }))
    }

    /// Whether it ended by itself, with nothing left behind.
    pub fn ended_by_itself(&self) -> bool {
        self.spawned
            && self.outcome.as_deref() == Some("completed")
            && self.stopped.is_none()
            && self.stream_error.is_none()
            && self.surviving.is_none()
            && self.failed.is_none()
    }
}

/// What the trial observed, as written (rule 34).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Facts {
    /// Schema version.
    pub version: u32,
    /// Provider session or fake.
    pub origin: Origin,
    /// Which attempt.
    pub attempt: AttemptIdentity,
    /// The session deadline, in seconds.
    pub deadline_seconds: u64,
    /// The turn limit.
    pub max_turns: u32,
    /// The prompt delivered.
    pub prompt: String,
    /// The sentinel.
    pub sentinel: Sentinel,
    /// The settings bytes the adapter wrote, verbatim, where any were.
    pub settings_written: Option<String>,
    /// The version the run path's probe reported.
    pub probed_version: Option<String>,
    /// The timeline.
    pub entries: Vec<Entry>,
    /// Where the decision was made, where it was.
    pub decision: Option<DecisionPoint>,
    /// How the process ended.
    pub process: ProcessEnd,
}

/// Rule 35.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "word")]
pub enum HookEvidence {
    /// One acknowledgment correlated before the decision point.
    Correlated,
    /// No `SessionStart` response anywhere in the stream.
    Absent,
    /// A response before the decision point whose acknowledgment did not bind.
    Unbound {
        /// Section 3.31 rule 18's word.
        kind: String,
    },
    /// Acknowledgments only after the decision point.
    Late,
    /// Acknowledgments naming different revisions.
    Conflicting,
    /// Correlated, naming a revision other than the required one.
    Mismatched,
}

impl HookEvidence {
    /// The word, with the kind where there is one.
    pub fn describe(&self) -> String {
        match self {
            HookEvidence::Correlated => "correlated".to_string(),
            HookEvidence::Absent => "absent".to_string(),
            HookEvidence::Unbound { kind } => format!("unbound ({kind})"),
            HookEvidence::Late => "late".to_string(),
            HookEvidence::Conflicting => "conflicting".to_string(),
            HookEvidence::Mismatched => "mismatched".to_string(),
        }
    }
}

/// Rule 36.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Effects {
    /// No tool executed before admission, as far as the gate held them.
    Excluded,
    /// A tool executed where nothing held it.
    DemonstratedPossible,
    /// The records cannot say.
    Unobserved,
}

impl Effects {
    /// The word.
    pub fn word(self) -> &'static str {
        match self {
            Effects::Excluded => "excluded",
            Effects::DemonstratedPossible => "demonstrated-possible",
            Effects::Unobserved => "unobserved",
        }
    }
}

/// Rule 37.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrialVerdict {
    /// Every condition of rule 37 holds.
    Established,
    /// A completed launch in which a condition fails.
    NotEstablished,
    /// The launch itself is uncertain, or the deadline ended it.
    Uncertain,
}

impl TrialVerdict {
    /// The word.
    pub fn word(self) -> &'static str {
        match self {
            TrialVerdict::Established => "established",
            TrialVerdict::NotEstablished => "not-established",
            TrialVerdict::Uncertain => "uncertain",
        }
    }
}

/// The judgement of rules 35 to 37.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Judgement {
    /// Provider session or fake, repeated beside the verdict on purpose.
    pub origin: Origin,
    /// The verdict.
    pub verdict: TrialVerdict,
    /// The hook evidence.
    pub hook: HookEvidence,
    /// Effects before admission.
    pub effects: Effects,
    /// What the effects word covers.
    pub scope: String,
    /// The startup decision, as recorded.
    pub decision: Option<Decision>,
    /// Tool requests in the stream.
    pub tool_requests: usize,
    /// Gate consultations, and how many released a call.
    pub consultations: usize,
    /// Consultations that released a call on `admitted`.
    pub released: usize,
    /// Tool results that executed.
    pub executed: usize,
    /// Whether the sentinel was read, carrying its nonce, after the decision.
    pub sentinel_read: bool,
    /// Every reason, in order. Empty only for `established`.
    pub reasons: Vec<String>,
}

/// Judge a trial from its facts and the attempt's records as they are now.
///
/// Pure: everything it reads is an argument. `required` is the intent's
/// required revision; `launch` is section 3.32's verdict read back.
pub fn judge(
    facts: &Facts,
    admission: Option<&Admission>,
    required: Option<&str>,
    gate: &[String],
    launch: Verdict,
) -> Judgement {
    let entries = &facts.entries;
    let decided_at = facts.decision.as_ref().and_then(|d| d.line);
    let before = |line: usize| decided_at.is_none_or(|d| line < d);
    let after = |line: usize| decided_at.is_some_and(|d| line > d);
    let decision = admission.map(|a| a.decision);
    let mut reasons = Vec::new();

    // Rule 35: the hook evidence.
    let responses = entries.iter().filter(|e| e.is_session_start_response());
    let any_response = responses.clone().next().is_some();
    let acks = |pred: &dyn Fn(usize) -> bool| {
        entries
            .iter()
            .filter(|e| {
                e.is_session_start_response()
                    && matches!(
                        e,
                        Entry::HookResponse {
                            acknowledgment: true,
                            ..
                        }
                    )
                    && pred(e.line())
            })
            .count()
    };
    let acks_before = acks(&|l| decided_at.is_none() || before(l));
    let acks_after = acks(&|l| after(l));
    let hook = match admission.map(|a| &a.observation) {
        Some(HarnessObservation::Correlated { digest, .. }) => {
            if required.is_some_and(|r| r != digest) || decision == Some(Decision::Mismatched) {
                HookEvidence::Mismatched
            } else {
                HookEvidence::Correlated
            }
        }
        Some(HarnessObservation::Unverified { kind, .. }) => match kind {
            Unverified::Conflicting => HookEvidence::Conflicting,
            Unverified::Absent | Unverified::NotLaunched if acks_before == 0 && acks_after > 0 => {
                HookEvidence::Late
            }
            Unverified::Absent | Unverified::NotLaunched if !any_response => HookEvidence::Absent,
            other => HookEvidence::Unbound {
                kind: other.word().to_string(),
            },
        },
        None if acks_before == 0 && acks_after > 0 => HookEvidence::Late,
        None if !any_response => HookEvidence::Absent,
        None => HookEvidence::Unbound {
            kind: "undecided".to_string(),
        },
    };
    match &hook {
        HookEvidence::Correlated => {}
        HookEvidence::Absent => reasons.push(
            "no SessionStart hook response arrived anywhere in the stream: the startup hook \
             supplied through --settings was not reported (premise P1)"
                .to_string(),
        ),
        HookEvidence::Unbound { kind } => reasons.push(format!(
            "a SessionStart response arrived before the decision point and its acknowledgment \
             did not bind ({kind}); premise P2 is that the hook receives the session's \
             environment"
        )),
        HookEvidence::Late => reasons.push(format!(
            "the acknowledgment arrived only after the decision point ({acks_after} after, none \
             before), and the decision never waits for it"
        )),
        HookEvidence::Conflicting => {
            reasons.push("acknowledgments named different revisions".to_string())
        }
        HookEvidence::Mismatched => reasons.push(
            "the correlated acknowledgment named a revision other than the required one"
                .to_string(),
        ),
    }
    if matches!(hook, HookEvidence::Correlated) && acks_after > 0 {
        reasons.push(format!(
            "{acks_after} further acknowledgment(s) arrived after the decision point and were not \
             judged"
        ));
    }

    // Rule 36: effects before admission.
    let uses: Vec<&Entry> = entries
        .iter()
        .filter(|e| matches!(e, Entry::ToolUse { .. }))
        .collect();
    let executed: Vec<&Entry> = entries
        .iter()
        .filter(|e| matches!(e, Entry::ToolResult { executed: true, .. }))
        .collect();
    let released = gate.iter().filter(|l| l.as_str() == "admitted").count();
    let held = gate.len() - released;
    let admitted = decision == Some(Decision::Admitted);
    let mut demonstrated = Vec::new();
    if !executed.is_empty() {
        if !admitted {
            demonstrated.push(format!(
                "{} tool result(s) executed and the decision was {}",
                executed.len(),
                decision.map_or("never made", Decision::describe)
            ));
        }
        if let Some(early) = executed
            .iter()
            .find(|e| decided_at.is_none_or(|d| e.line() < d))
        {
            demonstrated.push(format!(
                "a tool result executed at stream line {} before the decision point",
                early.line()
            ));
        }
        if executed.len() > released {
            demonstrated.push(format!(
                "{} tool result(s) executed and the gate released {released}: a call ran that \
                 the gate did not hold",
                executed.len()
            ));
        }
    }
    let effects = if !demonstrated.is_empty() {
        reasons.extend(demonstrated);
        Effects::DemonstratedPossible
    } else if admitted
        && !uses.is_empty()
        && gate.len() >= uses.len()
        && held == 0
        && !executed.is_empty()
        && executed.iter().all(|e| after(e.line()))
    {
        Effects::Excluded
    } else {
        reasons.push(format!(
            "effects before admission are unobserved: {} tool request(s), {} consultation(s), {} \
             released, {} executed",
            uses.len(),
            gate.len(),
            released,
            executed.len()
        ));
        Effects::Unobserved
    };

    // Rule 37: the remaining conditions.
    if !admitted {
        reasons.push(format!(
            "the startup decision was {}",
            decision.map_or("never made", Decision::describe)
        ));
    }
    if uses.is_empty() {
        reasons.push("the session requested no tool, so the gate had nothing to hold".to_string());
    } else if gate.len() < uses.len() {
        reasons.push(format!(
            "{} tool request(s) and {} gate consultation(s): a request did not consult the gate \
             (premise P3)",
            uses.len(),
            gate.len()
        ));
    }
    if held > 0 {
        reasons.push(format!("the gate withheld or refused {held} call(s)"));
    }
    let sentinel_read = entries.iter().any(|e| {
        matches!(
            e,
            Entry::ToolResult {
                executed: true,
                nonce: true,
                ..
            }
        ) && after(e.line())
    }) && uses
        .iter()
        .any(|e| matches!(e, Entry::ToolUse { name, .. } if name == "Read"));
    if !sentinel_read {
        reasons.push(
            "no executed Read result carried the sentinel's nonce after the decision point"
                .to_string(),
        );
    }
    if !facts.process.ended_by_itself() {
        reasons.push(format!(
            "the process did not end by itself with nothing left behind: outcome {}, stopped {}, \
             stream error {}, survivor {}",
            facts.process.outcome.as_deref().unwrap_or("none"),
            facts.process.stopped.as_deref().unwrap_or("no"),
            facts.process.stream_error.as_deref().unwrap_or("none"),
            facts.process.surviving.as_deref().unwrap_or("none"),
        ));
    }

    let uncertain = launch.uncertain()
        || matches!(launch, Verdict::SpawnFailed | Verdict::Interrupted)
        || facts.process.deadline(entries)
        || facts.process.failed.is_some();
    if uncertain {
        reasons.insert(
            0,
            format!(
                "the launch is {}{}: no retry follows, and the trial is spent",
                launch.word(),
                if facts.process.deadline(entries) {
                    ", and the deadline ended the process"
                } else {
                    ""
                }
            ),
        );
    }
    if launch == Verdict::NotLaunched || !facts.process.spawned {
        reasons.insert(0, "no session was started".to_string());
    }
    let verdict = if uncertain {
        TrialVerdict::Uncertain
    } else if reasons.is_empty() && effects == Effects::Excluded && facts.process.spawned {
        TrialVerdict::Established
    } else {
        TrialVerdict::NotEstablished
    };
    Judgement {
        origin: facts.origin,
        verdict,
        hook,
        effects,
        scope: SCOPE.to_string(),
        decision,
        tool_requests: uses.len(),
        consultations: gate.len(),
        released,
        executed: executed.len(),
        sentinel_read,
        reasons,
    }
}

/// `trial.json`: the facts, the gate log as it was, and the judgement written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrialRecord {
    /// What was observed.
    pub facts: Facts,
    /// The gate's log when the record was written.
    pub gate: Vec<String>,
    /// What the gate consultations the judgement reads are evidence of:
    /// [`CHILD_ATTESTED`] in a trial recorded after spec 002 section 3.37
    /// (its rule 3). Absent from a trial recorded before it, which was
    /// recorded without confinement and is read as it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consultations: Option<String>,
    /// The judgement written.
    pub judgement: Judgement,
}

impl TrialRecord {
    /// A record written now: its consultations are child-attested.
    pub fn new(facts: Facts, gate: Vec<String>, judgement: Judgement) -> Self {
        Self {
            facts,
            gate,
            consultations: Some(CHILD_ATTESTED.to_string()),
            judgement,
        }
    }
}

/// Where a trial's record is, beside the attempt's launch records.
pub fn path(paths: &AttemptPaths) -> PathBuf {
    paths.records.join(FILE)
}

/// Write the record once, into the attempt's launch records.
pub fn write(paths: &AttemptPaths, record: &TrialRecord) -> std::io::Result<PathBuf> {
    let path = path(paths);
    let json = serde_json::to_string_pretty(record).map_err(std::io::Error::other)?;
    crate::launch::write_once(&path, format!("{json}\n"))?;
    Ok(path)
}

/// A trial's record read back, and judged again from the records on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reloaded {
    /// Where it is.
    pub path: String,
    /// What was written.
    pub record: TrialRecord,
    /// The judgement recomputed now.
    pub judgement: Judgement,
    /// Whether the recomputed judgement is the one written.
    pub agrees: bool,
}

impl Reloaded {
    /// The human lines.
    pub fn describe(&self) -> String {
        let j = &self.judgement;
        let f = &self.record.facts;
        let mut out = format!(
            "trial     {} ({}){}\n",
            j.verdict.word(),
            f.origin.word(),
            if f.origin == Origin::Synthetic {
                ": a local fake; a statement about the procedure, not about a provider"
            } else {
                ""
            }
        );
        out.push_str(&format!("  hook evidence  {}\n", j.hook.describe()));
        out.push_str(&format!(
            "  decision       {} at {}\n",
            j.decision.map_or("none recorded", Decision::describe),
            f.decision
                .as_ref()
                .map_or("no point recorded".to_string(), |d| {
                    format!(
                        "{} ({})",
                        d.line
                            .map_or("the end of the stream".to_string(), |l| format!("line {l}")),
                        d.event
                    )
                })
        ));
        out.push_str(&format!(
            "  tool calls     {} requested, {} consultation(s), {} released, {} executed; sentinel {}\n",
            j.tool_requests,
            j.consultations,
            j.released,
            j.executed,
            if j.sentinel_read { "read after admission" } else { "not read after admission" }
        ));
        out.push_str(&format!(
            "  effects        {} before admission; scope: {}\n",
            j.effects.word(),
            j.scope
        ));
        out.push_str(&format!(
            "  versions       probe {}, init {}\n",
            f.probed_version.as_deref().unwrap_or("not reported"),
            f.entries
                .iter()
                .find_map(|e| match e {
                    Entry::Init { version, .. } => version.clone(),
                    _ => None,
                })
                .as_deref()
                .unwrap_or("not reported")
        ));
        out.push_str(&format!(
            "  consultations  {}\n",
            match self.record.consultations.as_deref() {
                Some(word) => format!(
                    "{word}: the gate log is written by the gate the child runs, so what it says \
                     is the child's (spec 002 section 3.37 rule 3)"
                ),
                None => "recorded before spec 002 section 3.37, without confinement, and read \
                         as it was"
                    .to_string(),
            }
        ));
        out.push_str(&format!("  record         {}\n", self.path));
        if !self.agrees {
            out.push_str(
                "  RELOAD         the judgement recomputed from disk differs from the one written\n",
            );
        }
        for reason in &j.reasons {
            out.push_str(&format!("  - {reason}\n"));
        }
        out
    }
}

/// Read a trial's record, if the attempt holds one, and judge it again.
///
/// `Ok(None)` when there is no `trial.json`. The judgement is recomputed from
/// the facts written and the admission, intent and gate log as they are now.
pub fn reload(
    paths: &AttemptPaths,
    attempt: &AttemptIdentity,
    admission: Option<&Admission>,
    required: Option<&str>,
    launch: Verdict,
) -> Result<Option<Reloaded>, String> {
    let path = path(paths);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{} could not be read: {e}", path.display())),
    };
    let record: TrialRecord = serde_json::from_str(&text)
        .map_err(|e| format!("{} is not a trial record: {e}", path.display()))?;
    if record.facts.attempt != *attempt {
        return Err(format!(
            "{} names run {} attempt {}, not this one",
            path.display(),
            record.facts.attempt.run_id,
            record.facts.attempt.attempt
        ));
    }
    let gate = read_gate(paths);
    let judgement = judge(&record.facts, admission, required, &gate, launch);
    let agrees = judgement == record.judgement && gate == record.gate;
    Ok(Some(Reloaded {
        path: path.display().to_string(),
        record,
        judgement,
        agrees,
    }))
}

/// The gate's log as it is now, the supervisor's copy where it made one;
/// absent or unreadable is empty. Child-attested either way.
pub fn read_gate(paths: &AttemptPaths) -> Vec<String> {
    paths
        .read_gate_log()
        .ok()
        .flatten()
        .map(|g| g.lines)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(entries: Vec<Entry>, decision_line: Option<usize>) -> Facts {
        Facts {
            version: TRIAL_VERSION,
            origin: Origin::Synthetic,
            attempt: AttemptIdentity {
                run_id: RUN_ID.to_string(),
                attempt: 1,
            },
            deadline_seconds: 120,
            max_turns: MAX_TURNS,
            prompt: prompt(),
            sentinel: Sentinel {
                path: "/w/STATECRAFT-TRIAL-SENTINEL".to_string(),
                nonce: "n".to_string(),
                digest: "d".to_string(),
            },
            settings_written: None,
            probed_version: Some("2.1.267".to_string()),
            entries,
            decision: decision_line.map(|l| DecisionPoint {
                line: Some(l),
                event: "init".to_string(),
            }),
            process: ProcessEnd {
                spawned: true,
                outcome: Some("completed".to_string()),
                ..ProcessEnd::default()
            },
        }
    }

    fn admission(decision: Decision, observation: HarnessObservation) -> Admission {
        Admission {
            version: crate::launch::LAUNCH_VERSION,
            attempt: AttemptIdentity {
                run_id: RUN_ID.to_string(),
                attempt: 1,
            },
            decision,
            reason: String::new(),
            decided_at: "t".to_string(),
            at_line: Some(3),
            observation,
            intent_digest: "i".to_string(),
        }
    }

    fn correlated(digest: &str) -> HarnessObservation {
        HarnessObservation::Correlated {
            root: "/h/r".to_string(),
            display: "h-r".to_string(),
            digest: digest.to_string(),
            session_id: "s".to_string(),
            hook_name: None,
            nonce: "x".to_string(),
        }
    }

    fn unverified(kind: Unverified) -> HarnessObservation {
        HarnessObservation::Unverified {
            kind,
            detail: String::new(),
        }
    }

    fn ack(line: usize) -> Entry {
        Entry::HookResponse {
            line,
            event: Some("SessionStart".to_string()),
            exit_code: Some(0),
            acknowledgment: true,
        }
    }

    fn init(line: usize) -> Entry {
        Entry::Init {
            line,
            session_id: Some("s".to_string()),
            version: Some("2.1.267".to_string()),
        }
    }

    fn read(line: usize) -> Entry {
        Entry::ToolUse {
            line,
            id: "t1".to_string(),
            name: "Read".to_string(),
        }
    }

    fn result(line: usize, executed: bool, nonce: bool) -> Entry {
        Entry::ToolResult {
            line,
            id: "t1".to_string(),
            executed,
            nonce,
        }
    }

    fn faithful() -> Vec<Entry> {
        vec![
            Entry::HookStarted {
                line: 1,
                event: Some("SessionStart".to_string()),
            },
            ack(2),
            init(3),
            read(4),
            result(5, true, true),
            Entry::Terminal { line: 6 },
        ]
    }

    #[test]
    fn a_trial_recorded_now_says_its_consultations_are_child_attested() {
        // Spec 002 section 3.37 rule 3.
        let facts = facts(faithful(), Some(3));
        let gate = vec!["admitted".to_string()];
        let j = judge(
            &facts,
            Some(&admission(Decision::Admitted, correlated("r"))),
            Some("r"),
            &gate,
            Verdict::Unverified,
        );
        let now = TrialRecord::new(facts, gate, j.clone());
        let json = serde_json::to_value(&now).unwrap();
        assert_eq!(json["consultations"], CHILD_ATTESTED);
        let reloaded = |record: TrialRecord| Reloaded {
            path: "trial.json".into(),
            record,
            judgement: j.clone(),
            agrees: true,
        };
        assert!(
            reloaded(now.clone())
                .describe()
                .contains("consultations  child-attested")
        );
        // A trial recorded before that section carries no such word, and is
        // read as it was: the judgement is unchanged, and so is the record.
        let mut older = json;
        older.as_object_mut().unwrap().remove("consultations");
        let older: TrialRecord = serde_json::from_value(older.clone()).unwrap();
        assert_eq!(older.consultations, None);
        assert_eq!(older.judgement, now.judgement);
        let text = reloaded(older.clone()).describe();
        assert!(
            text.contains("recorded before spec 002 section 3.37"),
            "{text}"
        );
        let written = serde_json::to_value(&older).unwrap();
        assert!(written.get("consultations").is_none(), "{written}");
    }

    #[test]
    fn a_faithful_trial_is_established_and_its_effects_excluded() {
        let j = judge(
            &facts(faithful(), Some(3)),
            Some(&admission(Decision::Admitted, correlated("r"))),
            Some("r"),
            &["admitted".to_string()],
            Verdict::Unverified,
        );
        assert_eq!(j.verdict, TrialVerdict::Established, "{:?}", j.reasons);
        assert_eq!(j.hook, HookEvidence::Correlated);
        assert_eq!(j.effects, Effects::Excluded);
        assert!(j.sentinel_read);
        assert!(j.reasons.is_empty());
        assert_eq!(j.origin, Origin::Synthetic);
    }

    #[test]
    fn no_hook_response_is_absent_and_an_ungated_execution_is_demonstrated_possible() {
        // A provider that ignores the registration and runs the tool anyway.
        let entries = vec![
            init(1),
            read(2),
            result(3, true, true),
            Entry::Terminal { line: 4 },
        ];
        let j = judge(
            &facts(entries, Some(1)),
            Some(&admission(
                Decision::NotEstablished,
                unverified(Unverified::Absent),
            )),
            Some("r"),
            &[],
            Verdict::NotAdmitted,
        );
        assert_eq!(j.hook, HookEvidence::Absent);
        assert_eq!(j.effects, Effects::DemonstratedPossible);
        assert_eq!(j.verdict, TrialVerdict::NotEstablished);
    }

    #[test]
    fn an_acknowledgment_after_the_decision_point_is_late() {
        let entries = vec![init(1), ack(2), Entry::Terminal { line: 3 }];
        let j = judge(
            &facts(entries, Some(1)),
            Some(&admission(
                Decision::NotEstablished,
                unverified(Unverified::Absent),
            )),
            Some("r"),
            &[],
            Verdict::NotAdmitted,
        );
        assert_eq!(j.hook, HookEvidence::Late);
        assert_eq!(j.effects, Effects::Unobserved);
        assert_eq!(j.verdict, TrialVerdict::NotEstablished);
    }

    #[test]
    fn a_response_that_does_not_bind_is_unbound_by_its_kind() {
        let j = judge(
            &facts(faithful(), Some(3)),
            Some(&admission(
                Decision::NotEstablished,
                unverified(Unverified::Replayed),
            )),
            Some("r"),
            &["refused".to_string()],
            Verdict::NotAdmitted,
        );
        assert_eq!(
            j.hook,
            HookEvidence::Unbound {
                kind: "replayed".to_string()
            }
        );
        // The Read result executed after a refusal: demonstrated, not excluded.
        assert_eq!(j.effects, Effects::DemonstratedPossible);
    }

    #[test]
    fn conflicting_and_mismatched_evidence_are_named() {
        let conflicting = judge(
            &facts(faithful(), Some(3)),
            Some(&admission(
                Decision::NotEstablished,
                unverified(Unverified::Conflicting),
            )),
            Some("r"),
            &[],
            Verdict::NotAdmitted,
        );
        assert_eq!(conflicting.hook, HookEvidence::Conflicting);
        let mismatched = judge(
            &facts(faithful(), Some(3)),
            Some(&admission(Decision::Mismatched, correlated("other"))),
            Some("r"),
            &[],
            Verdict::Mismatched,
        );
        assert_eq!(mismatched.hook, HookEvidence::Mismatched);
        assert_eq!(mismatched.verdict, TrialVerdict::NotEstablished);
    }

    #[test]
    fn a_tool_that_executed_before_the_decision_line_is_demonstrated_possible() {
        let entries = vec![
            ack(1),
            read(2),
            result(3, true, true),
            init(4),
            Entry::Terminal { line: 5 },
        ];
        // Decided at the init event, line 4: the read at line 3 came first.
        let j = judge(
            &facts(entries, Some(4)),
            Some(&admission(Decision::Admitted, correlated("r"))),
            Some("r"),
            &["admitted".to_string()],
            Verdict::Unverified,
        );
        assert_eq!(j.effects, Effects::DemonstratedPossible);
        assert_eq!(j.verdict, TrialVerdict::NotEstablished);
    }

    #[test]
    fn a_request_that_never_consulted_the_gate_is_not_excluded() {
        // Admitted, and the provider ran the tool without the gate.
        let j = judge(
            &facts(faithful(), Some(3)),
            Some(&admission(Decision::Admitted, correlated("r"))),
            Some("r"),
            &[],
            Verdict::Unverified,
        );
        assert_eq!(j.effects, Effects::DemonstratedPossible);
        assert!(
            j.reasons
                .iter()
                .any(|r| r.contains("did not consult the gate"))
        );
    }

    #[test]
    fn a_withheld_call_that_did_not_execute_leaves_effects_unobserved() {
        let entries = vec![
            ack(1),
            init(2),
            read(3),
            result(4, false, false),
            Entry::Terminal { line: 5 },
        ];
        let j = judge(
            &facts(entries, Some(2)),
            Some(&admission(Decision::Admitted, correlated("r"))),
            Some("r"),
            &["withheld: no decision within 300 tenths of a second".to_string()],
            Verdict::Unverified,
        );
        assert_eq!(j.effects, Effects::Unobserved);
        assert_eq!(j.verdict, TrialVerdict::NotEstablished);
        assert!(
            j.reasons
                .iter()
                .any(|r| r.contains("withheld or refused 1"))
        );
    }

    #[test]
    fn an_uncertain_launch_or_a_deadline_is_uncertain_and_never_established() {
        let mut f = facts(vec![ack(1), init(2), read(3)], Some(2));
        f.process = ProcessEnd {
            spawned: true,
            outcome: Some("interrupted".to_string()),
            ..ProcessEnd::default()
        };
        let j = judge(
            &f,
            Some(&admission(Decision::Admitted, correlated("r"))),
            Some("r"),
            &["admitted".to_string()],
            Verdict::Unverified,
        );
        assert_eq!(j.verdict, TrialVerdict::Uncertain);
        assert!(j.reasons[0].contains("deadline"));
        let j = judge(
            &facts(faithful(), Some(3)),
            None,
            Some("r"),
            &[],
            Verdict::OutcomeUnknown,
        );
        assert_eq!(j.verdict, TrialVerdict::Uncertain);
    }

    /// A session the deadline stopped before it wrote any event carries the
    /// adapter's "no init" stream error as well. Rule 37 makes it `uncertain`
    /// all the same, because the deadline is what stopped it; a record
    /// written before the supervisor reported its deadline keeps the older
    /// reading, so reloading it cannot change its verdict.
    #[test]
    fn a_deadline_before_the_first_event_is_uncertain_despite_the_missing_init() {
        let mut f = facts(Vec::new(), None);
        f.process = ProcessEnd {
            spawned: true,
            outcome: Some("interrupted".to_string()),
            stream_error: Some(
                "event stream carried no init event, so what the provider applied is unknown"
                    .to_string(),
            ),
            timed_out: true,
            ..ProcessEnd::default()
        };
        let j = judge(&f, None, Some("r"), &[], Verdict::Unverified);
        assert_eq!(j.verdict, TrialVerdict::Uncertain, "{:?}", j.reasons);
        assert!(j.reasons[0].contains("deadline"), "{:?}", j.reasons);

        f.process.timed_out = false;
        let j = judge(&f, None, Some("r"), &[], Verdict::Unverified);
        assert_eq!(j.verdict, TrialVerdict::NotEstablished);
    }

    #[test]
    fn a_record_without_the_deadline_flag_reads_and_writes_as_it_did() {
        let end = ProcessEnd {
            spawned: true,
            outcome: Some("completed".to_string()),
            ..ProcessEnd::default()
        };
        let text = serde_json::to_string(&end).unwrap();
        assert!(!text.contains("timedOut"), "{text}");
        let read: ProcessEnd = serde_json::from_str(&text).unwrap();
        assert!(!read.timed_out);
    }

    #[test]
    fn the_scope_is_carried_beside_every_effects_word() {
        let j = judge(
            &facts(faithful(), Some(3)),
            Some(&admission(Decision::Admitted, correlated("r"))),
            Some("r"),
            &["admitted".to_string()],
            Verdict::Unverified,
        );
        assert_eq!(j.scope, SCOPE);
        assert!(SCOPE.contains("tool calls only"));
    }

    #[test]
    fn the_prompt_names_the_sentinel_and_one_read() {
        assert!(prompt().contains(SENTINEL));
        assert!(prompt().contains("Read tool exactly once"));
        assert!(refuses_deadline(0).is_some());
        assert!(refuses_deadline(301).is_some());
        assert!(refuses_deadline(300).is_none());
    }

    #[test]
    fn the_sentinel_is_placed_once_and_never_over_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let s = place_sentinel(dir.path()).unwrap();
        let text = std::fs::read_to_string(&s.path).unwrap();
        assert!(text.contains(&s.nonce));
        assert_eq!(s.digest, digest_bytes(text.as_bytes()));
        assert!(place_sentinel(dir.path()).is_err());
    }
}
