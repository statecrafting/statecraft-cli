//! What admits a live observation, and what refuses one.
//!
//! Spec 002 section 3.29, settled by the owner on 2026-09-21 and recorded
//! before this module was written.
//!
//! # The defect this module exists to close
//!
//! The first admission read the session's prose. It accepted a transcript that
//! contained the command text together with any of `permission`, `blocked`,
//! `refus`, `denied` or `not allowed`. The transcript
//!
//! ```text
//! cargo publish --dry-run: permission granted; command executed successfully
//! ```
//!
//! contains both halves and was admitted as evidence that the command had been
//! **refused**. That is not a gap in a word list. A sentence a model emitted is
//! a statement by the model, and no reading of it establishes what the harness
//! did, because the two are not the same event.
//!
//! So nothing here reads prose for a decision. The admission reads the
//! **structured refusal record** the harness itself emits, which spec 004
//! section 3.3 already owns, parses and tests in
//! [`statecraft_adapter_claude_code`]. Prose may sit in the capture beside it
//! and is carried, never read.
//!
//! # Three controls, enforced here rather than remembered
//!
//! Section 3.29 rule 2 makes the controls conditions on the evidence rather
//! than steps in a procedure. [`admit`] refuses a claim whose controls are
//! absent or whose controls did not behave as the table says:
//!
//! | Control | The invocation | What must be true of the capture |
//! |---|---|---|
//! | [`Control::Refusal`] | the claimed command, with the payload | a structured denial naming that command |
//! | [`Control::Allowed`] | a command no floor entry claims, with the payload | the command was attempted and **not** denied |
//! | [`Control::WithoutPayload`] | the claimed command, with no payload | the command was attempted and **not** denied |
//!
//! Without the second, a refusal is consistent with a payload that refuses
//! everything. Without the third, it is evidence for the operator's own
//! configuration rather than for this payload.
//!
//! # What this module does not establish
//!
//! It establishes what the captured bytes show. It does not establish that the
//! bytes came from a provider: a capture assembled by hand, well formed, with
//! the right version and a denial entry naming the right command, is admitted,
//! because nothing available here can tell it from a recorded one. That
//! residual is why section 3.29 keeps the bytes with the record instead of
//! summarizing them into it. The admission is reviewable because what it was
//! made from can be re-read, not because the check cannot be fooled.

use crate::settings::DENY_FLOOR;
use serde::{Deserialize, Serialize};
use statecraft_adapter_claude_code::{ProviderEvent, ResultEvent, read_jsonl};
use statecraft_environment::digest::digest_bytes;

/// Which measurement a capture is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Control {
    /// The claimed command, run with the managed-session payload.
    Refusal,
    /// A command no floor entry claims, run with the same payload.
    Allowed,
    /// The claimed command, run with no payload at all.
    WithoutPayload,
}

impl Control {
    /// The token as it is written in an error and in a record.
    pub fn word(self) -> &'static str {
        match self {
            Control::Refusal => "refusal",
            Control::Allowed => "allowed-command",
            Control::WithoutPayload => "without-payload",
        }
    }

    /// Whether this control's invocation must carry the settings payload.
    ///
    /// The absent-payload control is the one that must not, and it is the one
    /// whose whole purpose is that it did not.
    pub fn carries_the_payload(self) -> bool {
        !matches!(self, Control::WithoutPayload)
    }
}

/// One invocation, as it was spawned.
///
/// Section 3.29 rule 4: an observation is bound to the invocation that produced
/// it. Recorded verbatim rather than described, so a reader compares arguments
/// rather than a summary of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Invocation {
    /// The program, as spawned.
    pub program: String,
    /// Its arguments, in order.
    pub arguments: Vec<String>,
    /// The working directory it ran in.
    pub working_directory: String,
}

impl Invocation {
    /// Whether the invocation names the settings argument.
    ///
    /// Presence only. What the argument's value *was* is carried separately in
    /// [`Measurement::settings`] as the bytes themselves, because a path
    /// recorded in an argument list says nothing about what the file held at
    /// the moment the harness read it.
    pub fn names_the_settings_argument(&self) -> bool {
        self.arguments
            .iter()
            .any(|a| a == crate::session::SETTINGS_ARGUMENT)
    }

    /// A one-line rendering.
    pub fn describe(&self) -> String {
        format!(
            "{} {} (in {})",
            self.program,
            self.arguments.join(" "),
            self.working_directory
        )
    }
}

/// The captured output of one invocation, and where it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capture {
    /// Where the bytes were read from. Provenance, kept with the bytes.
    pub source: String,
    /// The bytes, verbatim and unnormalized.
    pub bytes: String,
}

impl Capture {
    /// The identity of the captured bytes.
    pub fn digest(&self) -> String {
        digest_bytes(self.bytes.as_bytes())
    }
}

/// One control: what was run, what it was given, and what it produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measurement {
    /// The invocation, as spawned.
    pub invocation: Invocation,
    /// The exact settings bytes the invocation was given, when it was given
    /// any. `None` is a statement, not an omission: the absent-payload control
    /// is the measurement whose point is that there were none.
    pub settings: Option<String>,
    /// What the session produced.
    pub capture: Capture,
}

/// Everything a claim carries.
///
/// Serialized into the record it qualifies, captures and all, because section
/// 3.29 rules 3 and 5 re-judge a record that was read back and neither is
/// decidable from a digest once the original file is gone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    /// The harness version the observation is claimed against.
    pub version: String,
    /// The identity of the payload the session was started with.
    pub payload_digest: String,
    /// The command the floor should have refused.
    pub refused_command: String,
    /// The command no floor entry claims, which the positive control ran.
    pub allowed_command: String,
    /// The refusal itself.
    pub refusal: Measurement,
    /// The positive control.
    pub allowed: Measurement,
    /// The negative control.
    pub without_payload: Measurement,
}

impl Evidence {
    /// The three controls, each with the measurement that carries it.
    pub fn controls(&self) -> [(Control, &Measurement); 3] {
        [
            (Control::Refusal, &self.refusal),
            (Control::Allowed, &self.allowed),
            (Control::WithoutPayload, &self.without_payload),
        ]
    }
}

/// Why a claimed observation is not admitted as one.
///
/// Every variant is a refusal of the claim. None of them is a weaker
/// admission, and none of them is recorded as an absence: section 3.29 rule 6
/// is that the answer is unverified, and a record carrying a rejected claim as
/// an absence would lose the fact that a claim was made and refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotAdmitted {
    /// No version was read.
    #[error("no harness version was recorded; the observation is version specific")]
    NoVersion,
    /// The session ran with settings this build did not produce.
    #[error(
        "the session was started with settings digesting to {found}; this build's payload \
         digests to {expected}, so the observation is of some other bytes"
    )]
    PayloadMismatch {
        /// What this build produces.
        expected: String,
        /// What the claim names.
        found: String,
    },
    /// The command is not one the floor claims, so refusing it proves nothing.
    #[error(
        "`{command}` matches no entry in the deny floor, so its refusal is not evidence that \
         the floor was enforced"
    )]
    NotOnTheFloor {
        /// The command claimed.
        command: String,
    },
    /// The positive control ran a command the floor claims.
    #[error(
        "the allowed-command control ran `{command}`, which the deny floor claims; a control \
         that is supposed to demonstrate the payload does not refuse everything has to name \
         something the payload does not refuse"
    )]
    ControlOnTheFloor {
        /// The command the control ran.
        command: String,
    },
    /// Two controls are the same bytes.
    #[error(
        "the {first} and {second} controls present the same captured bytes ({digest}); one \
         capture is one measurement and cannot be two"
    )]
    SubstitutedEvidence {
        /// One control.
        first: &'static str,
        /// The other.
        second: &'static str,
        /// The digest they share.
        digest: String,
    },
    /// A control's invocation does not carry the payload it must.
    #[error(
        "the {control} control's invocation does not carry `{argument}`, so whatever it \
         measured, it did not measure a session started with the payload"
    )]
    PayloadNotInInvocation {
        /// Which control.
        control: &'static str,
        /// The argument that is missing.
        argument: &'static str,
    },
    /// The absent-payload control carried a payload.
    #[error(
        "the without-payload control's invocation carries `{argument}`; a control that exists \
         to show what happens without the payload cannot be run with it"
    )]
    PayloadInTheControl {
        /// The argument that should not be there.
        argument: &'static str,
    },
    /// A control's supplied settings are not the payload it claims.
    #[error(
        "the {control} control was given settings digesting to {found}, and the claim names \
         {expected}; evidence for another settings payload cannot qualify this one"
    )]
    ControlSettingsMismatch {
        /// Which control.
        control: &'static str,
        /// The payload digest the claim names.
        expected: String,
        /// What the control was actually given.
        found: String,
    },
    /// Nothing was captured.
    #[error(
        "the {control} control's capture ({from}) is empty; a step whose output was not \
         captured did not happen"
    )]
    EmptyCapture {
        /// Which control.
        control: &'static str,
        /// Where it was read from. Not named `source`: `thiserror` reads that
        /// name as an error cause, and this is provenance.
        from: String,
    },
    /// The capture is not the harness's structured output.
    #[error(
        "the {control} control's capture ({from}) is not the harness's structured output: \
         {detail}. Prose is not evidence of enforcement, however it reads"
    )]
    Unreadable {
        /// Which control.
        control: &'static str,
        /// Where it was read from.
        from: String,
        /// What the deserializer said.
        detail: String,
    },
    /// The capture never says what the session applied.
    #[error(
        "the {control} control's capture carries no init event, so nothing in it says which \
         harness ran or what it applied"
    )]
    NoInitEvent {
        /// Which control.
        control: &'static str,
    },
    /// The capture reports a version other than the claimed one.
    #[error(
        "the {control} control's capture reports harness version {found} and the claim is \
         made against {expected}; an observation is version specific and these are two \
         different observations"
    )]
    VersionMismatch {
        /// The version claimed.
        expected: String,
        /// The version the capture reports.
        found: String,
        /// Which control.
        control: &'static str,
    },
    /// The session never reached its terminal event.
    #[error(
        "the {control} control's capture reaches no terminal event, so the refusal record it \
         would have carried does not exist; an unfinished session is unverified and not a \
         negative result"
    )]
    NoTerminalResult {
        /// Which control.
        control: &'static str,
    },
    /// No structured refusal for the claimed command.
    #[error(
        "the refusal control's capture carries no structured denial for `{command}`; it \
         carries {found} denial(s) ({named}). A session that says in prose that a command was \
         refused has said so, and nothing more"
    )]
    NoStructuredRefusal {
        /// The command claimed.
        command: String,
        /// How many denials the capture does carry.
        found: usize,
        /// What they name.
        named: String,
    },
    /// A control that must not be refused, was.
    #[error("the {control} control's capture carries a structured denial for `{command}`: {why}")]
    ControlRefused {
        /// Which control.
        control: &'static str,
        /// The command that was denied.
        command: String,
        /// What that means for the claim.
        why: &'static str,
    },
    /// A control whose command was never attempted.
    #[error(
        "the {control} control's capture shows no attempt to run `{command}`, so it measured \
         nothing: {why}"
    )]
    ControlNotAttempted {
        /// Which control.
        control: &'static str,
        /// The command it should have attempted.
        command: String,
        /// Why an unattempted control is not a result.
        why: &'static str,
    },
}

/// Whether a deny-floor entry claims a command.
///
/// The entries are `Bash(<prefix>*)` or `Bash(<exact>)`. Matching them here
/// rather than trusting a caller's word is what makes
/// [`NotAdmitted::NotOnTheFloor`] decidable: an observation of some unrelated
/// command being refused says nothing about this floor.
pub fn floor_claims(command: &str) -> bool {
    DENY_FLOOR.iter().any(|entry| {
        let Some(body) = entry
            .strip_prefix("Bash(")
            .and_then(|b| b.strip_suffix(')'))
        else {
            return false;
        };
        match body.strip_suffix('*') {
            Some(prefix) => command.starts_with(prefix),
            None => command == body,
        }
    })
}

/// What one capture was read as.
struct Read {
    version: Option<String>,
    result: Option<ResultEvent>,
    attempted: Vec<String>,
}

impl Read {
    /// Whether the capture shows a structured denial for this command.
    ///
    /// The denial's `tool_input` is the harness's own record of what it
    /// refused, so the command is compared against that rather than against
    /// anything the session said about it.
    fn denied(&self, command: &str) -> bool {
        self.result.iter().any(|r| {
            r.permission_denials
                .iter()
                .any(|d| command_of(&d.tool_input).as_deref() == Some(command))
        })
    }

    /// Every command the capture's denials name, for an error that says what
    /// was there instead of what was not.
    fn denials_named(&self) -> (usize, String) {
        let denials: Vec<String> = self
            .result
            .iter()
            .flat_map(|r| r.permission_denials.iter())
            .map(|d| {
                format!(
                    "{}: {}",
                    d.tool_name,
                    command_of(&d.tool_input).unwrap_or_else(|| "no command".to_string())
                )
            })
            .collect();
        let rendered = if denials.is_empty() {
            "none".to_string()
        } else {
            denials.join("; ")
        };
        (denials.len(), rendered)
    }
}

/// The `command` a tool input names, when it names one.
fn command_of(input: &serde_json::Value) -> Option<String> {
    input
        .get("command")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

/// Read one capture into the three things the admission asks of it.
///
/// Every tool use the session attempted is collected from the assistant turns,
/// because a control that never ran its command measured nothing and the
/// terminal event alone cannot say whether it ran.
fn read(control: Control, capture: &Capture) -> Result<Read, NotAdmitted> {
    if capture.bytes.trim().is_empty() {
        return Err(NotAdmitted::EmptyCapture {
            control: control.word(),
            from: capture.source.clone(),
        });
    }
    let events = read_jsonl(&capture.bytes).map_err(|e| NotAdmitted::Unreadable {
        control: control.word(),
        from: capture.source.clone(),
        detail: e.to_string(),
    })?;

    let mut version = None;
    let mut result = None;
    let mut attempted = Vec::new();
    for event in events {
        match event {
            ProviderEvent::System(system) if system.is_init() => {
                version = system.claude_code_version.clone();
            }
            ProviderEvent::Assistant(message) => {
                collect_attempts(&message.message, &mut attempted);
            }
            ProviderEvent::Result(terminal) => result = Some(*terminal),
            _ => {}
        }
    }
    Ok(Read {
        version,
        result,
        attempted,
    })
}

/// Every `command` a message's tool uses name.
fn collect_attempts(message: &serde_json::Value, out: &mut Vec<String>) {
    let Some(content) = message.get("content").and_then(serde_json::Value::as_array) else {
        return;
    };
    for block in content {
        if block.get("type").and_then(serde_json::Value::as_str) != Some("tool_use") {
            continue;
        }
        if let Some(command) = block.get("input").and_then(command_of) {
            out.push(command);
        }
    }
}

/// Admit a live-session observation, or say why it is not one.
///
/// Section 3.29's six rules, in the order that refuses earliest. Nothing here
/// returns a weaker admission: a claim is admitted or it is refused, and a
/// refused claim leaves the session unverified.
pub fn admit(evidence: &Evidence) -> Result<(), NotAdmitted> {
    // Rule 4, the parts that need no capture.
    if evidence.version.trim().is_empty() {
        return Err(NotAdmitted::NoVersion);
    }
    let expected = crate::startup::payload_identity();
    if evidence.payload_digest != expected {
        return Err(NotAdmitted::PayloadMismatch {
            expected,
            found: evidence.payload_digest.clone(),
        });
    }

    // Rule 2: the refusal names something the floor claims, and the positive
    // control names something it does not. Both halves, because a control that
    // is itself on the floor demonstrates nothing about a payload that refuses
    // everything.
    if !floor_claims(&evidence.refused_command) {
        return Err(NotAdmitted::NotOnTheFloor {
            command: evidence.refused_command.clone(),
        });
    }
    if floor_claims(&evidence.allowed_command) {
        return Err(NotAdmitted::ControlOnTheFloor {
            command: evidence.allowed_command.clone(),
        });
    }

    // Rule 3: one capture is one measurement.
    let controls = evidence.controls();
    for i in 0..controls.len() {
        for j in (i + 1)..controls.len() {
            let (first, a) = controls[i];
            let (second, b) = controls[j];
            if a.capture.bytes == b.capture.bytes {
                return Err(NotAdmitted::SubstitutedEvidence {
                    first: first.word(),
                    second: second.word(),
                    digest: a.capture.digest(),
                });
            }
        }
    }

    // Rule 4: each control was given what its own purpose requires, and the
    // settings it was given are the payload the claim names.
    for (control, measurement) in controls {
        let argument = crate::session::SETTINGS_ARGUMENT;
        match (
            control.carries_the_payload(),
            measurement.invocation.names_the_settings_argument(),
        ) {
            (true, false) => {
                return Err(NotAdmitted::PayloadNotInInvocation {
                    control: control.word(),
                    argument,
                });
            }
            (false, true) => return Err(NotAdmitted::PayloadInTheControl { argument }),
            _ => {}
        }
        match (control.carries_the_payload(), &measurement.settings) {
            (true, Some(bytes)) => {
                let found = digest_bytes(bytes.as_bytes());
                if found != evidence.payload_digest {
                    return Err(NotAdmitted::ControlSettingsMismatch {
                        control: control.word(),
                        expected: evidence.payload_digest.clone(),
                        found,
                    });
                }
            }
            (true, None) => {
                return Err(NotAdmitted::ControlSettingsMismatch {
                    control: control.word(),
                    expected: evidence.payload_digest.clone(),
                    found: "nothing recorded".to_string(),
                });
            }
            (false, Some(bytes)) => {
                let _ = bytes;
                return Err(NotAdmitted::PayloadInTheControl { argument });
            }
            (false, None) => {}
        }
    }

    // Rules 1 and 3: every capture is read as structured output, reaches a
    // terminal event, and agrees with the claimed version.
    let mut reads = Vec::new();
    for (control, measurement) in controls {
        let parsed = read(control, &measurement.capture)?;
        let Some(found) = parsed.version.clone() else {
            return Err(NotAdmitted::NoInitEvent {
                control: control.word(),
            });
        };
        if found != evidence.version {
            return Err(NotAdmitted::VersionMismatch {
                expected: evidence.version.clone(),
                found,
                control: control.word(),
            });
        }
        if parsed.result.is_none() {
            return Err(NotAdmitted::NoTerminalResult {
                control: control.word(),
            });
        }
        reads.push((control, parsed));
    }
    let refusal = &reads[0].1;
    let allowed = &reads[1].1;
    let without_payload = &reads[2].1;

    // Rule 1: the refusal is a structured denial naming this command.
    if !refusal.denied(&evidence.refused_command) {
        let (found, named) = refusal.denials_named();
        return Err(NotAdmitted::NoStructuredRefusal {
            command: evidence.refused_command.clone(),
            found,
            named,
        });
    }

    // Rule 2, the positive control: it ran, and it was not refused.
    if !allowed
        .attempted
        .iter()
        .any(|c| c == &evidence.allowed_command)
    {
        return Err(NotAdmitted::ControlNotAttempted {
            control: Control::Allowed.word(),
            command: evidence.allowed_command.clone(),
            why: "without an attempt that succeeded, the refusal is still consistent with a \
                  payload that refuses everything",
        });
    }
    if allowed.denied(&evidence.allowed_command) {
        return Err(NotAdmitted::ControlRefused {
            control: Control::Allowed.word(),
            command: evidence.allowed_command.clone(),
            why: "the payload refused a command no floor entry claims, so what it enforces is \
                  not the floor",
        });
    }

    // Rule 2, the negative control: it ran the same command with no payload,
    // and nothing refused it.
    if !without_payload
        .attempted
        .iter()
        .any(|c| c == &evidence.refused_command)
    {
        return Err(NotAdmitted::ControlNotAttempted {
            control: Control::WithoutPayload.word(),
            command: evidence.refused_command.clone(),
            why: "a control that never attempted the command cannot say whether something \
                  other than the payload would have refused it",
        });
    }
    if without_payload.denied(&evidence.refused_command) {
        return Err(NotAdmitted::ControlRefused {
            control: Control::WithoutPayload.word(),
            command: evidence.refused_command.clone(),
            why: "the command is refused without the payload too, so the refusal is evidence \
                  for the operator's own configuration and not for this payload",
        });
    }

    Ok(())
}

/// What an admitted observation says, in one line.
///
/// Built from the evidence rather than from a caller's words, so the sentence
/// in the record cannot describe something the evidence does not show.
pub fn describe(evidence: &Evidence) -> String {
    format!(
        "`{}` was refused by the installed harness through a structured denial; `{}` ran under \
         the same payload and was not refused, and `{}` was not refused without it. Harness {}, \
         payload {}",
        evidence.refused_command,
        evidence.allowed_command,
        evidence.refused_command,
        evidence.version,
        evidence.payload_digest,
    )
}

/// One control, as an operator writes it down.
///
/// The captures are named by **path** rather than pasted inline: they are the
/// provider's own output files, and a submission that required them to be
/// embedded in JSON would be a submission an operator edits by hand. Every path
/// resolves against the submission's own directory, so a capture directory
/// moves as one thing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmittedControl {
    /// The invocation, as spawned.
    pub invocation: Invocation,
    /// The settings file the invocation was given, when it was given one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<String>,
    /// The captured output.
    pub capture: String,
}

/// A body of evidence as it is submitted, before anything is read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Submission {
    /// The harness version the observation is claimed against.
    pub version: String,
    /// The identity of the payload the session was started with.
    pub payload_digest: String,
    /// The command the floor should have refused.
    pub refused_command: String,
    /// The command no floor entry claims.
    pub allowed_command: String,
    /// The refusal.
    pub refusal: SubmittedControl,
    /// The positive control.
    pub allowed: SubmittedControl,
    /// The negative control.
    pub without_payload: SubmittedControl,
}

/// Why a submission could not be read into evidence.
///
/// Separate from [`NotAdmitted`] on purpose. A submission that cannot be read
/// is not a claim that was judged and refused; it is a claim that was never
/// stated, and reporting the two the same way would let a typo look like a
/// measured negative.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotRead {
    /// The submission file itself.
    #[error("the submission at {path} could not be read: {detail}")]
    Submission {
        /// Where it was looked for.
        path: String,
        /// What went wrong.
        detail: String,
    },
    /// A file the submission names.
    #[error(
        "the {control} control names {kind} at {path} and it could not be read: {detail}. A \
         capture that is not there is not a weaker capture"
    )]
    Referenced {
        /// Which control.
        control: &'static str,
        /// A capture or a settings file.
        kind: &'static str,
        /// Where it was looked for.
        path: String,
        /// What went wrong.
        detail: String,
    },
}

/// Read a submission and the files it names into evidence.
///
/// Reading only. Nothing is admitted here: [`admit`] is still the judge, and a
/// submission that reads cleanly can still be refused by every rule in section
/// 3.29. The two are separate calls so a caller can say which of the two
/// happened, which is the difference between "your capture is missing" and
/// "your capture does not show a refusal".
pub fn load(submission: &std::path::Path) -> Result<Evidence, NotRead> {
    let bytes = std::fs::read(submission).map_err(|e| NotRead::Submission {
        path: submission.display().to_string(),
        detail: e.to_string(),
    })?;
    let parsed: Submission = serde_json::from_slice(&bytes).map_err(|e| NotRead::Submission {
        path: submission.display().to_string(),
        detail: e.to_string(),
    })?;
    let base = submission.parent().unwrap_or(std::path::Path::new("."));

    let read_one = |control: Control, c: &SubmittedControl| -> Result<Measurement, NotRead> {
        let capture_path = base.join(&c.capture);
        let bytes = std::fs::read_to_string(&capture_path).map_err(|e| NotRead::Referenced {
            control: control.word(),
            kind: "a capture",
            path: capture_path.display().to_string(),
            detail: e.to_string(),
        })?;
        let settings = match &c.settings {
            None => None,
            Some(rel) => {
                let path = base.join(rel);
                Some(
                    std::fs::read_to_string(&path).map_err(|e| NotRead::Referenced {
                        control: control.word(),
                        kind: "a settings file",
                        path: path.display().to_string(),
                        detail: e.to_string(),
                    })?,
                )
            }
        };
        Ok(Measurement {
            invocation: c.invocation.clone(),
            settings,
            capture: Capture {
                source: capture_path.display().to_string(),
                bytes,
            },
        })
    };

    Ok(Evidence {
        version: parsed.version,
        payload_digest: parsed.payload_digest,
        refused_command: parsed.refused_command,
        allowed_command: parsed.allowed_command,
        refusal: read_one(Control::Refusal, &parsed.refusal)?,
        allowed: read_one(Control::Allowed, &parsed.allowed)?,
        without_payload: read_one(Control::WithoutPayload, &parsed.without_payload)?,
    })
}
