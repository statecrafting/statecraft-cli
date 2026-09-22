//! What admits a live observation, and what refuses one.
//!
//! Spec 002 sections 3.29 and 3.30, each settled by the owner and recorded
//! before the code it authorizes.
//!
//! # The defects this module exists to close
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
//! **refused**. Section 3.29 closed that by reading the structured refusal
//! record the harness emits.
//!
//! The second admission, written under section 3.29, still let a **request**
//! stand in for an execution, a **text match** stand in for a tool-use
//! identity, the **last** init and terminal events stand in for a whole
//! capture, and an argument list **written after the launch** stand in for the
//! launch. Section 3.30 names each, and this module is its repair:
//!
//! - every tool use is correlated by its id with its result, its denial entry,
//!   its mid-stream denial and the harness's own non-execution note, and is
//!   classified [`Use::Refused`], [`Use::Executed`] or unresolved (rule 9);
//! - a capture is read whole: one session, one init, one terminal event, last
//!   (rule 8);
//! - each control's outcome is judged separately, and permission success is
//!   not command success (rule 10);
//! - the process end and terminal state are judged per observation (rule 11);
//! - and the invocation is judged against the [`Launch`] record the launching
//!   operation wrote, by recomputing the exact argument vector this build
//!   constructs (rule 12).
//!
//! Nothing here reads prose for a decision. The one place text is compared is
//! the allowed control's tool result, which is the harness's record of the
//! command's own output, compared exactly against the output the command is
//! chosen to produce.
//!
//! # What this module does not establish
//!
//! A [`Launch`] record is launcher-attested (section 3.30, "The trust
//! boundary"). It says what this product launched and what came back. It does
//! not say that the provider loaded the settings, which the three controls
//! exist to infer from behavior, and it does not authenticate itself: a record
//! edited by hand after the fact and still consistent is admitted, because
//! nothing here signs it. The bytes are kept so the admission can be re-read.

use crate::settings::DENY_FLOOR;
use serde::{Deserialize, Serialize};
use statecraft_adapter_claude_code::{
    PermissionDenial, ProviderEvent, ResultEvent, ToolResult, ToolUse, read_jsonl,
};
use statecraft_environment::digest::digest_bytes;
use std::collections::{BTreeMap, BTreeSet};

/// The tool the floor's entries govern. Every entry is `Bash(...)`.
pub const GOVERNED_TOOL: &str = "Bash";

/// The claimed command the acceptance runs (section 3.30, "The experiment").
///
/// Claimed by `Bash(cargo publish*)`. The explicit manifest path names a file
/// that does not exist, so cargo stops before resolving anything, searches no
/// ancestor directory for a manifest, and contacts no registry. If enforcement
/// fails, this is what runs, and it is chosen to be harmless when it does.
pub const REFUSED_COMMAND: &str =
    "cargo publish --dry-run --manifest-path statecraft-absent/Cargo.toml";

/// The directory [`REFUSED_COMMAND`]'s manifest path would sit in. The
/// launching operation refuses a working directory that holds it.
pub const REFUSED_COMMAND_ABSENT_PATH: &str = "statecraft-absent";

/// The allowed command, which no floor entry claims and which succeeds
/// everywhere a shell does.
pub const ALLOWED_COMMAND: &str = "echo statecraft-allowed-control";

/// What [`ALLOWED_COMMAND`] prints, which its tool result must carry exactly.
pub const ALLOWED_OUTPUT: &str = "statecraft-allowed-control";

/// The turn cap every control runs under (section 3.30 rule 11).
pub const MAX_TURNS: u32 = 1;

/// Which measurement a capture is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Control {
    /// The claimed command, run with the managed-session payload.
    Refusal,
    /// A command no floor entry claims, run with the same payload.
    #[serde(rename = "allowed-command")]
    Allowed,
    /// The claimed command, run with no payload at all.
    WithoutPayload,
}

impl Control {
    /// Every control, in the order the acceptance launches them.
    pub const ALL: [Control; 3] = [Control::Refusal, Control::Allowed, Control::WithoutPayload];

    /// The token as it is written in an error, a record and a file name.
    pub fn word(self) -> &'static str {
        match self {
            Control::Refusal => "refusal",
            Control::Allowed => "allowed-command",
            Control::WithoutPayload => "without-payload",
        }
    }

    /// The control a token names.
    pub fn from_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.word() == word)
    }

    /// Whether this control's invocation must carry the settings payload.
    ///
    /// The absent-payload control is the one that must not, and it is the one
    /// whose whole purpose is that it did not.
    pub fn carries_the_payload(self) -> bool {
        !matches!(self, Control::WithoutPayload)
    }
}

/// The prompt a control's session is given, on standard input.
///
/// Never an argument, so no text in it can be read as an option (section 3.30
/// rule 12).
pub fn prompt(command: &str) -> String {
    format!(
        "Use the {GOVERNED_TOOL} tool to run exactly this command, once, and then stop. Do not \
         run anything else.\n\n{command}\n"
    )
}

/// The exact argument vector this build launches a control with.
///
/// The one definition, used by the launching operation to construct the
/// invocation and by the admission to check a recorded one. `settings` is the
/// path the payload was written to, for a control that carries it.
pub fn arguments(
    control: Control,
    refused: &str,
    allowed: &str,
    settings: Option<&str>,
) -> Vec<String> {
    let mut args: Vec<String> = statecraft_adapter_claude_code::spawn::BASE_ARGS
        .iter()
        .map(|a| (*a).to_string())
        .collect();
    args.extend(["--max-turns".to_string(), MAX_TURNS.to_string()]);
    // One grant, identical in all three launches, naming exactly the two
    // commands. Without it a non-interactive session refuses an ungranted
    // command whatever the payload says, and the controls would measure the
    // missing grant (section 3.30, "One grant, identical in all three").
    args.push("--allowedTools".to_string());
    args.push(format!("{GOVERNED_TOOL}({refused})"));
    args.push(format!("{GOVERNED_TOOL}({allowed})"));
    if control.carries_the_payload() {
        if let Some(path) = settings {
            args.push(crate::session::SETTINGS_ARGUMENT.to_string());
            args.push(path.to_string());
        }
    }
    args
}

/// One invocation, as it was launched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Invocation {
    /// The program, as launched: the resolved path.
    pub program: String,
    /// Its arguments, in order.
    pub arguments: Vec<String>,
    /// The working directory it ran in.
    pub working_directory: String,
}

impl Invocation {
    /// A one-line rendering.
    pub fn describe(&self) -> String {
        format!(
            "{} {} (in {})",
            self.program,
            self.arguments.join(" "),
            self.working_directory
        )
    }

    /// Every settings argument, in either spelling, with its value.
    ///
    /// `--settings <path>` and `--settings=<path>` are the provider's two
    /// spellings of one option. Counting both is what makes a second settings
    /// input, or one spelled differently, visible.
    pub fn settings_arguments(&self) -> Vec<(String, Option<String>)> {
        let flag = crate::session::SETTINGS_ARGUMENT;
        let mut found = Vec::new();
        let mut args = self.arguments.iter().peekable();
        while let Some(arg) = args.next() {
            if arg == flag {
                found.push((arg.clone(), args.peek().map(|v| (*v).clone())));
            } else if let Some(value) = arg.strip_prefix(&format!("{flag}=")) {
                found.push((arg.clone(), Some(value.to_string())));
            }
        }
        found
    }
}

/// The captured standard output of one invocation, and where it came from.
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

/// Whether a capture came from the provider or from a local fake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    /// Launched against the executable the operator named as the provider.
    Launched,
    /// Launched against a local fake, at the operator's explicit statement.
    /// Never a live observation (section 3.30, "Synthetic captures stay
    /// synthetic").
    Synthetic,
}

/// How a launched process ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessEnd {
    /// The exit code, when it exited by itself.
    pub code: Option<i32>,
    /// The signal that ended it, when one did.
    pub signal: Option<i32>,
    /// Whether the deadline ended it.
    pub timed_out: bool,
    /// Set when something in its process group outlived it.
    pub surviving_processes: Option<String>,
}

impl ProcessEnd {
    /// Why this end leaves the control unmeasured, if it does.
    ///
    /// Section 3.30 rule 11: ended by itself, inside the deadline, no signal,
    /// no survivor, and an exit code the measured provider ends a session
    /// with.
    pub fn unmeasured(&self) -> Option<String> {
        if self.timed_out {
            return Some("the deadline ended it".to_string());
        }
        if let Some(signal) = self.signal {
            return Some(format!("signal {signal} ended it"));
        }
        if let Some(survivor) = &self.surviving_processes {
            return Some(format!("its process group outlived it: {survivor}"));
        }
        match self.code {
            None => Some("it has no exit code".to_string()),
            Some(0 | 1) => None,
            Some(code) => Some(format!(
                "it exited {code}, which is not a code the measured provider ends a session with"
            )),
        }
    }
}

/// What the launching operation recorded, beside the invocation it launched.
///
/// Section 3.30 rule 12. Written by the same operation that started the
/// process, never reconstructed afterwards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Launch {
    /// This capture's identity.
    pub capture_id: String,
    /// Which control the launch was.
    pub control: Control,
    /// Provider or fake.
    pub origin: Origin,
    /// The command the session was told to run.
    pub command: String,
    /// The prompt, exactly as it was written to standard input.
    pub prompt: String,
    /// The program as the operator named it.
    pub requested_program: String,
    /// The resolved program's digest, when it could be read.
    pub program_digest: Option<String>,
    /// What `--version` printed, verbatim.
    pub probe: String,
    /// The version read from it.
    pub probe_version: Option<String>,
    /// Where the payload was written, for a control that carries it.
    pub settings_path: Option<String>,
    /// The settings file's digest after the process ended.
    pub settings_digest_after: Option<String>,
    /// Standard error, verbatim.
    pub stderr: String,
    /// The digest of standard output's raw bytes.
    pub stdout_digest: String,
    /// The digest of standard error's raw bytes.
    pub stderr_digest: String,
    /// Streams whose bytes were not UTF-8, and so are kept only in the raw
    /// files beside the record and by digest here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub undecodable: Vec<String>,
    /// How the process ended.
    pub process: ProcessEnd,
    /// The deadline the session ran under, in seconds.
    pub deadline_seconds: u64,
}

/// One control: what was launched, what it was given, and what it produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measurement {
    /// The invocation, as launched.
    pub invocation: Invocation,
    /// The exact settings bytes the invocation was given, when it was given
    /// any. `None` is a statement, not an omission: the absent-payload control
    /// is the measurement whose point is that there were none.
    pub settings: Option<String>,
    /// Standard output.
    pub capture: Capture,
    /// The launch record. Absent from every record written before section
    /// 3.30, which is why it defaults: such a record still reads, and the
    /// admission refuses it for this field rather than the reader refusing the
    /// file (section 3.30, "Records written before this section").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch: Option<Launch>,
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
    /// What the allowed command prints. Empty in records written before
    /// section 3.30, which the admission refuses for their missing launch
    /// before it reaches this field.
    #[serde(default)]
    pub allowed_output: String,
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

    /// The command a control was supposed to run.
    pub fn command_of(&self, control: Control) -> &str {
        match control {
            Control::Allowed => &self.allowed_command,
            Control::Refusal | Control::WithoutPayload => &self.refused_command,
        }
    }

    /// Whether any control was launched against a fake.
    ///
    /// A record with no launch at all is not synthetic by this test; it is
    /// refused by the admission for the missing launch instead.
    pub fn synthetic(&self) -> bool {
        self.controls().iter().any(|(_, m)| {
            m.launch
                .as_ref()
                .is_some_and(|l| l.origin == Origin::Synthetic)
        })
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
    /// The positive control names no output to check.
    #[error(
        "the claim names no expected output for the allowed command, so its execution cannot \
         be told from a result that merely exists"
    )]
    NoExpectedOutput,
    /// A record written before section 3.30, or one assembled without a launch.
    #[error(
        "the {control} control carries no launch record, so nothing binds its invocation and \
         settings to what was launched; a record written before section 3.30 reads as \
         unverified, and its bytes are kept as they were"
    )]
    NoLaunchRecord {
        /// Which control.
        control: &'static str,
    },
    /// The launch record says it was a different control.
    #[error("the {control} control's launch record says it launched the {recorded} control")]
    ControlMislabelled {
        /// Which control it is filed as.
        control: &'static str,
        /// What the launch says it was.
        recorded: &'static str,
    },
    /// The launch ran another command than the control's.
    #[error(
        "the {control} control's launch told the session to run `{found}`, and the claim's \
         command for it is `{expected}`"
    )]
    CommandMismatch {
        /// Which control.
        control: &'static str,
        /// What the claim needs.
        expected: String,
        /// What was launched.
        found: String,
    },
    /// The prompt is not the one this build writes.
    #[error("the {control} control's prompt is not the one this build writes for `{command}`")]
    PromptMismatch {
        /// Which control.
        control: &'static str,
        /// The command.
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
    /// Two controls share an identity that one launch or one session has.
    #[error(
        "the {first} and {second} controls share {what} `{value}`; three controls are three \
         launches of three sessions, and one presented twice is substituted evidence however \
         its bytes are formatted"
    )]
    SharedIdentity {
        /// One control.
        first: &'static str,
        /// The other.
        second: &'static str,
        /// Which identity.
        what: &'static str,
        /// Its value.
        value: String,
    },
    /// The controls ran in different places.
    #[error(
        "the {first} control ran in {a} and the {second} control in {b}; the controls are \
         measurements of one project"
    )]
    DifferentWorkingDirectories {
        /// One control.
        first: &'static str,
        /// Where.
        a: String,
        /// The other.
        second: &'static str,
        /// Where.
        b: String,
    },
    /// A payload control's settings argument is missing, doubled, respelled or
    /// names another file.
    #[error("the {control} control's settings argument does not bind it to its settings: {detail}")]
    SettingsArgument {
        /// Which control.
        control: &'static str,
        /// What is wrong.
        detail: String,
    },
    /// The absent-payload control carried a payload.
    #[error(
        "the without-payload control's invocation carries `{argument}`; a control that exists \
         to show what happens without the payload cannot be run with it"
    )]
    PayloadInTheControl {
        /// The argument that should not be there.
        argument: String,
    },
    /// The recorded argument vector is not the one this build launches.
    #[error(
        "the {control} control's arguments are not the ones this build launches it with: \
         expected [{expected}], recorded [{found}]"
    )]
    InvocationMismatch {
        /// Which control.
        control: &'static str,
        /// What this build constructs.
        expected: String,
        /// What was recorded.
        found: String,
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
    /// The settings file changed while the session ran.
    #[error(
        "the {control} control's settings file digested to {before} at launch and {after} \
         after; bytes that changed while the session ran are not the bytes it was given"
    )]
    SettingsChangedDuringRun {
        /// Which control.
        control: &'static str,
        /// At launch.
        before: String,
        /// After.
        after: String,
    },
    /// The process did not end in a way that lets its output be judged.
    #[error(
        "the {control} control is unmeasured: {why}. An interrupted or incomplete session is \
         unverified and not a negative result"
    )]
    Unmeasured {
        /// Which control.
        control: &'static str,
        /// What ended it, or what is missing.
        why: String,
    },
    /// The exit code and the terminal event disagree.
    #[error(
        "the {control} control exited {code} and its terminal event says is_error {is_error}; \
         every recorded session pairs 0 with false and 1 with true, so these disagree"
    )]
    ExitDisagrees {
        /// Which control.
        control: &'static str,
        /// The exit code.
        code: i32,
        /// The terminal event's flag.
        is_error: bool,
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
    /// The init event lacks a field the admission binds to.
    #[error("the {control} control's init event carries no {field}")]
    IncompleteInit {
        /// Which control.
        control: &'static str,
        /// Which field.
        field: &'static str,
    },
    /// The capture reports a version other than the claimed one.
    #[error(
        "the {control} control's {source_of_it} reports harness version {found} and the claim \
         is made against {expected}; an observation is version specific and these are two \
         different observations"
    )]
    VersionMismatch {
        /// The version claimed.
        expected: String,
        /// The version found.
        found: String,
        /// Which control.
        control: &'static str,
        /// The init event or the launch's version probe.
        source_of_it: &'static str,
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
    /// An event the capture may carry once, twice.
    #[error(
        "the {control} control's capture carries more than one {event} event; a capture is one \
         session, and two of these are two sessions or a conflict"
    )]
    DuplicateEvent {
        /// Which control.
        control: &'static str,
        /// Which event.
        event: &'static str,
    },
    /// Events out of the order one session produces.
    #[error("the {control} control's capture is out of order: {detail}")]
    OutOfOrder {
        /// Which control.
        control: &'static str,
        /// What is out of order.
        detail: String,
    },
    /// An event with no session, or events from more than one.
    #[error("the {control} control's capture is not one session: {detail}")]
    MixedSession {
        /// Which control.
        control: &'static str,
        /// What was found.
        detail: String,
    },
    /// Correlation between a request, its result and its refusal failed.
    #[error("the {control} control's capture does not correlate: {detail}")]
    Uncorrelated {
        /// Which control.
        control: &'static str,
        /// What does not line up.
        detail: String,
    },
    /// A governed tool use that is neither refused nor executed.
    #[error(
        "the {control} control's tool use {id} is neither refused nor executed: {why}. A \
         request is not an execution, and the absence of a denial is not a result"
    )]
    Unresolved {
        /// Which control.
        control: &'static str,
        /// The tool-use id.
        id: String,
        /// What is missing or contradictory.
        why: &'static str,
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
    /// The refusal control executed the command it claims was refused.
    #[error(
        "the refusal control's capture shows `{command}` refused and also executed; a command \
         that ran was not prevented, whatever else was denied"
    )]
    ExecutedDespiteRefusal {
        /// The command.
        command: String,
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
    /// A control whose command was never used through the governed tool.
    #[error(
        "the {control} control's capture shows no {GOVERNED_TOOL} use of `{command}`{other}, so \
         it measured nothing: {why}"
    )]
    ControlNotAttempted {
        /// Which control.
        control: &'static str,
        /// The command it should have attempted.
        command: String,
        /// A note naming uses of the command under another tool, if any.
        other: String,
        /// Why an unattempted control is not a result.
        why: &'static str,
    },
    /// A control's session also did something else.
    #[error(
        "the {control} control's capture carries a use of {tool} ({what}) besides its own \
         command; evidence about a session that did something else is not evidence about this \
         one"
    )]
    UnexpectedToolUse {
        /// Which control.
        control: &'static str,
        /// Which tool.
        tool: String,
        /// Its command, or its input.
        what: String,
    },
    /// The controls ran in another project than the one being qualified.
    #[error(
        "the controls ran in {found}, and the session being qualified is in {expected}; \
         evidence about another project cannot qualify this one"
    )]
    AnotherProject {
        /// The project being qualified.
        expected: String,
        /// Where the controls ran.
        found: String,
    },
    /// The allowed command executed and did not produce its output.
    #[error(
        "the allowed-command control's `{command}` executed and its result is not the expected \
         output `{expected}`: {found}"
    )]
    WrongOutput {
        /// The command.
        command: String,
        /// What it prints.
        expected: String,
        /// What was found instead.
        found: String,
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
            .strip_prefix(&format!("{GOVERNED_TOOL}("))
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

/// How one tool use ended (section 3.30 rule 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Use {
    /// A terminal denial entry, and a result the harness marked not executed.
    Refused,
    /// A result, no non-execution mark, no denial of either kind.
    Executed,
}

/// One capture, read whole.
struct Stream {
    session: String,
    version: String,
    cwd: String,
    terminal: ResultEvent,
    uses: Vec<ToolUse>,
    results: BTreeMap<String, ToolResult>,
    not_executed: BTreeMap<String, String>,
    denials: Vec<PermissionDenial>,
    mid_stream_denials: BTreeSet<String>,
}

impl Stream {
    /// Classify one tool use, or say why it cannot be.
    fn classify(&self, id: &str) -> Result<Use, &'static str> {
        let denied = self.denials.iter().any(|d| d.tool_use_id == id);
        let result = self.results.contains_key(id);
        let marked = self.not_executed.contains_key(id);
        let mid = self.mid_stream_denials.contains(id);
        match (denied, result, marked, mid) {
            (_, false, _, _) => Err("the request has no result"),
            (true, true, true, _) => Ok(Use::Refused),
            (false, true, false, false) => Ok(Use::Executed),
            (true, true, false, _) => {
                Err("it is denied and its result is not marked as not executed")
            }
            (false, true, true, _) => {
                Err("its result is marked as not executed with no denial entry behind it")
            }
            (false, true, false, true) => {
                Err("a mid-stream denial names it and no terminal entry does")
            }
        }
    }

    fn denials_named(&self) -> (usize, String) {
        let named: Vec<String> = self
            .denials
            .iter()
            .map(|d| {
                format!(
                    "{}: {}",
                    d.tool_name,
                    d.tool_input
                        .get("command")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("no command")
                )
            })
            .collect();
        let rendered = if named.is_empty() {
            "none".to_string()
        } else {
            named.join("; ")
        };
        (named.len(), rendered)
    }
}

/// Read one capture whole (section 3.30 rule 8).
fn read(control: Control, capture: &Capture) -> Result<Stream, NotAdmitted> {
    let c = control.word();
    if capture.bytes.trim().is_empty() {
        return Err(NotAdmitted::EmptyCapture {
            control: c,
            from: capture.source.clone(),
        });
    }
    let unreadable = |detail: String| NotAdmitted::Unreadable {
        control: c,
        from: capture.source.clone(),
        detail,
    };
    let events = read_jsonl(&capture.bytes).map_err(|e| unreadable(e.to_string()))?;
    let out_of_order = |detail: String| NotAdmitted::OutOfOrder { control: c, detail };

    let mut sessions: Vec<(&'static str, Option<String>)> = Vec::new();
    let mut init: Option<(String, Option<String>, Option<String>)> = None;
    let mut terminal: Option<ResultEvent> = None;
    let mut uses: Vec<ToolUse> = Vec::new();
    let mut results = BTreeMap::new();
    let mut not_executed = BTreeMap::new();
    let mut mid_stream_denials = BTreeSet::new();

    for (index, event) in events.iter().enumerate() {
        let line = index + 1;
        if terminal.is_some() {
            if matches!(event, ProviderEvent::Result(_)) {
                return Err(NotAdmitted::DuplicateEvent {
                    control: c,
                    event: "terminal",
                });
            }
            return Err(out_of_order(format!(
                "event {line} follows the terminal event, which is the last event of a session"
            )));
        }
        match event {
            ProviderEvent::System(system) if system.is_init() => {
                if init.is_some() {
                    return Err(NotAdmitted::DuplicateEvent {
                        control: c,
                        event: "init",
                    });
                }
                sessions.push(("init", system.session_id.clone()));
                init = Some((
                    system.session_id.clone().unwrap_or_default(),
                    system.claude_code_version.clone(),
                    system.cwd.clone(),
                ));
            }
            ProviderEvent::System(system) => {
                sessions.push(("system", system.session_id.clone()));
                if system.is_permission_denied() {
                    let Some(id) = &system.tool_use_id else {
                        return Err(NotAdmitted::Uncorrelated {
                            control: c,
                            detail: format!(
                                "the mid-stream denial at event {line} names no tool use"
                            ),
                        });
                    };
                    let Some(used) = uses.iter().find(|u| &u.id == id) else {
                        return Err(NotAdmitted::Uncorrelated {
                            control: c,
                            detail: format!(
                                "the mid-stream denial at event {line} names {id}, which no earlier \
                                 request in this capture carries"
                            ),
                        });
                    };
                    if system.tool_name.as_deref() != Some(used.name.as_str()) {
                        return Err(NotAdmitted::Uncorrelated {
                            control: c,
                            detail: format!(
                                "the mid-stream denial for {id} names tool {:?} and the request \
                                 was for {}",
                                system.tool_name, used.name
                            ),
                        });
                    }
                    mid_stream_denials.insert(id.clone());
                }
            }
            ProviderEvent::Assistant(message) => {
                if init.is_none() {
                    return Err(out_of_order(format!(
                        "the assistant turn at event {line} precedes the init event"
                    )));
                }
                sessions.push(("assistant", message.session_id.clone()));
                for used in message.tool_uses().map_err(unreadable)? {
                    if uses.iter().any(|u| u.id == used.id) {
                        return Err(NotAdmitted::Uncorrelated {
                            control: c,
                            detail: format!("tool-use id {} is requested twice", used.id),
                        });
                    }
                    uses.push(used);
                }
            }
            ProviderEvent::User(message) => {
                if init.is_none() {
                    return Err(out_of_order(format!(
                        "the user turn at event {line} precedes the init event"
                    )));
                }
                sessions.push(("user", message.session_id.clone()));
                let here = message.tool_results().map_err(unreadable)?;
                for result in &here {
                    if !uses.iter().any(|u| u.id == result.tool_use_id) {
                        return Err(NotAdmitted::Uncorrelated {
                            control: c,
                            detail: format!(
                                "the tool result at event {line} answers {}, which no earlier \
                                 request in this capture carries",
                                result.tool_use_id
                            ),
                        });
                    }
                    if results.contains_key(&result.tool_use_id) {
                        return Err(NotAdmitted::Uncorrelated {
                            control: c,
                            detail: format!("tool use {} has two results", result.tool_use_id),
                        });
                    }
                    results.insert(result.tool_use_id.clone(), result.clone());
                }
                for note in &message.tool_result_meta {
                    if !here.iter().any(|r| r.tool_use_id == note.id) {
                        return Err(NotAdmitted::Uncorrelated {
                            control: c,
                            detail: format!(
                                "a non-execution note at event {line} names {}, and that event \
                                 carries no result for it",
                                note.id
                            ),
                        });
                    }
                    if let Some(kind) = &note.non_execution_kind {
                        not_executed.insert(note.id.clone(), kind.clone());
                    }
                }
            }
            ProviderEvent::Result(result) => {
                if init.is_none() {
                    return Err(out_of_order(format!(
                        "the terminal event at event {line} precedes the init event"
                    )));
                }
                sessions.push(("terminal", result.session_id.clone()));
                terminal = Some((**result).clone());
            }
            // A well-formed event of a type this build does not map carries no
            // field this admission reads, and spec 004 section 3.1 classifies
            // it as progress. Its position still counts, above.
            ProviderEvent::Unknown => {}
        }
    }
    let Some((session, version, cwd)) = init else {
        return Err(NotAdmitted::NoInitEvent { control: c });
    };
    let Some(terminal) = terminal else {
        return Err(NotAdmitted::NoTerminalResult { control: c });
    };
    let version = version.ok_or(NotAdmitted::IncompleteInit {
        control: c,
        field: "claude_code_version",
    })?;
    let cwd = cwd.ok_or(NotAdmitted::IncompleteInit {
        control: c,
        field: "cwd",
    })?;

    // One session, named by every event that can name one.
    if let Some((kind, _)) = sessions.iter().find(|(_, s)| s.is_none()) {
        return Err(NotAdmitted::MixedSession {
            control: c,
            detail: format!("a {kind} event names no session"),
        });
    }
    let distinct: BTreeSet<&str> = sessions.iter().filter_map(|(_, s)| s.as_deref()).collect();
    if distinct.len() > 1 {
        return Err(NotAdmitted::MixedSession {
            control: c,
            detail: format!("its events name {} sessions: {distinct:?}", distinct.len()),
        });
    }

    // Every terminal denial names a request in this capture, for the same
    // tool, with the same input verbatim.
    for denial in &terminal.permission_denials {
        let Some(used) = uses.iter().find(|u| u.id == denial.tool_use_id) else {
            return Err(NotAdmitted::Uncorrelated {
                control: c,
                detail: format!(
                    "a terminal denial names {}, which no request in this capture carries",
                    denial.tool_use_id
                ),
            });
        };
        if used.name != denial.tool_name || used.input != denial.tool_input {
            return Err(NotAdmitted::Uncorrelated {
                control: c,
                detail: format!(
                    "the terminal denial for {} names {} with input {}, and the request was {} \
                     with input {}",
                    denial.tool_use_id, denial.tool_name, denial.tool_input, used.name, used.input
                ),
            });
        }
    }
    let denials = terminal.permission_denials.clone();
    Ok(Stream {
        session,
        version,
        cwd,
        terminal,
        uses,
        results,
        not_executed,
        denials,
        mid_stream_denials,
    })
}

/// Whether a capture reads as one complete session (section 3.30 rule 8).
///
/// A reading, not a judgement: the launching operation uses it to say whether
/// the next control is worth launching, and says nothing about what the
/// session showed.
pub fn one_session(control: Control, capture: &Capture) -> Result<(), NotAdmitted> {
    read(control, capture).map(|_| ())
}

/// The binding checks of section 3.30 rule 12 for one control, and rule 11's
/// process end. Everything here is judged from the launch record, before the
/// capture is read.
fn bound(evidence: &Evidence, control: Control, m: &Measurement) -> Result<(), NotAdmitted> {
    let c = control.word();
    let launch = m
        .launch
        .as_ref()
        .ok_or(NotAdmitted::NoLaunchRecord { control: c })?;
    if launch.control != control {
        return Err(NotAdmitted::ControlMislabelled {
            control: c,
            recorded: launch.control.word(),
        });
    }
    let command = evidence.command_of(control);
    if launch.command != command {
        return Err(NotAdmitted::CommandMismatch {
            control: c,
            expected: command.to_string(),
            found: launch.command.clone(),
        });
    }
    if launch.prompt != prompt(command) {
        return Err(NotAdmitted::PromptMismatch {
            control: c,
            command: command.to_string(),
        });
    }

    // The settings argument, judged before the whole vector so a refusal says
    // which of the specific ways it is wrong.
    let found = m.invocation.settings_arguments();
    if control.carries_the_payload() {
        let Some(path) = &launch.settings_path else {
            return Err(NotAdmitted::SettingsArgument {
                control: c,
                detail: "the launch wrote no settings file".to_string(),
            });
        };
        match found.as_slice() {
            [] => {
                return Err(NotAdmitted::SettingsArgument {
                    control: c,
                    detail: format!("it carries no `{}`", crate::session::SETTINGS_ARGUMENT),
                });
            }
            [(arg, value)] => {
                if arg != crate::session::SETTINGS_ARGUMENT {
                    return Err(NotAdmitted::SettingsArgument {
                        control: c,
                        detail: format!(
                            "it is spelled `{arg}`, which this build never launches with"
                        ),
                    });
                }
                if value.as_deref() != Some(path.as_str()) {
                    return Err(NotAdmitted::SettingsArgument {
                        control: c,
                        detail: format!(
                            "it names {:?}, and the launch wrote the settings to {path}",
                            value.as_deref().unwrap_or("nothing")
                        ),
                    });
                }
            }
            many => {
                return Err(NotAdmitted::SettingsArgument {
                    control: c,
                    detail: format!(
                        "it carries {} settings arguments ({}), and the provider reads one",
                        many.len(),
                        many.iter()
                            .map(|(a, v)| format!("{a} {}", v.as_deref().unwrap_or("")))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                });
            }
        }
    } else {
        if let Some((arg, _)) = found.first() {
            return Err(NotAdmitted::PayloadInTheControl {
                argument: arg.clone(),
            });
        }
        if launch.settings_path.is_some() || m.settings.is_some() {
            return Err(NotAdmitted::PayloadInTheControl {
                argument: "a settings file".to_string(),
            });
        }
    }
    let expected = arguments(
        control,
        &evidence.refused_command,
        &evidence.allowed_command,
        launch.settings_path.as_deref(),
    );
    if m.invocation.arguments != expected {
        return Err(NotAdmitted::InvocationMismatch {
            control: c,
            expected: expected.join(" "),
            found: m.invocation.arguments.join(" "),
        });
    }

    // The settings bytes, at launch and after.
    if control.carries_the_payload() {
        let Some(bytes) = &m.settings else {
            return Err(NotAdmitted::ControlSettingsMismatch {
                control: c,
                expected: evidence.payload_digest.clone(),
                found: "nothing recorded".to_string(),
            });
        };
        let before = digest_bytes(bytes.as_bytes());
        if before != evidence.payload_digest {
            return Err(NotAdmitted::ControlSettingsMismatch {
                control: c,
                expected: evidence.payload_digest.clone(),
                found: before,
            });
        }
        if launch.settings_digest_after.as_deref() != Some(before.as_str()) {
            return Err(NotAdmitted::SettingsChangedDuringRun {
                control: c,
                before,
                after: launch
                    .settings_digest_after
                    .clone()
                    .unwrap_or_else(|| "unreadable".to_string()),
            });
        }
    }

    // The version the launch's own probe read.
    match &launch.probe_version {
        Some(v) if v == &evidence.version => {}
        other => {
            return Err(NotAdmitted::VersionMismatch {
                expected: evidence.version.clone(),
                found: other.clone().unwrap_or_else(|| "nothing".to_string()),
                control: c,
                source_of_it: "launch's version probe",
            });
        }
    }

    // Rule 11, the process half.
    if let Some(why) = launch.process.unmeasured() {
        return Err(NotAdmitted::Unmeasured { control: c, why });
    }
    if !launch.undecodable.is_empty() {
        return Err(NotAdmitted::Unreadable {
            control: c,
            from: m.capture.source.clone(),
            detail: format!(
                "{} was not UTF-8 and is kept only as raw bytes",
                launch.undecodable.join(" and ")
            ),
        });
    }
    if digest_bytes(m.capture.bytes.as_bytes()) != launch.stdout_digest {
        return Err(NotAdmitted::Unreadable {
            control: c,
            from: m.capture.source.clone(),
            detail: "the recorded output does not digest to what the launch read".to_string(),
        });
    }
    Ok(())
}

/// Rule 11's terminal half, and the exit code against it.
fn terminal_measurable(control: Control, m: &Measurement, s: &Stream) -> Result<(), NotAdmitted> {
    let c = control.word();
    let reason = s.terminal.terminal_reason.as_deref();
    if !matches!(reason, Some("completed" | "max_turns")) {
        return Err(NotAdmitted::Unmeasured {
            control: c,
            why: format!(
                "its terminal event (subtype `{}`, terminal_reason {reason:?}) is not a completed \
                 or turn-capped session",
                s.terminal.subtype
            ),
        });
    }
    if statecraft_adapter_claude_code::outcome(&s.terminal).is_err() {
        return Err(NotAdmitted::Unmeasured {
            control: c,
            why: format!(
                "its terminal subtype `{}` is not in spec 004 section 3.13's mapping",
                s.terminal.subtype
            ),
        });
    }
    let code = m.launch.as_ref().and_then(|l| l.process.code).unwrap_or(-1);
    if (code == 0) == s.terminal.is_error {
        return Err(NotAdmitted::ExitDisagrees {
            control: c,
            code,
            is_error: s.terminal.is_error,
        });
    }
    if s.cwd != m.invocation.working_directory {
        return Err(NotAdmitted::Uncorrelated {
            control: c,
            detail: format!(
                "its init event ran in {} and the launch recorded {}",
                s.cwd, m.invocation.working_directory
            ),
        });
    }
    Ok(())
}

/// Rules 9 and 10 for one control.
fn outcome(evidence: &Evidence, control: Control, s: &Stream) -> Result<(), NotAdmitted> {
    let c = control.word();
    let command = evidence.command_of(control);
    let governed: Vec<&ToolUse> = s
        .uses
        .iter()
        .filter(|u| u.name == GOVERNED_TOOL && u.command() == Some(command))
        .collect();
    if governed.is_empty() {
        let elsewhere: Vec<String> = s
            .uses
            .iter()
            .filter(|u| u.command() == Some(command))
            .map(|u| format!("{} {}", u.name, u.id))
            .collect();
        return Err(NotAdmitted::ControlNotAttempted {
            control: c,
            command: command.to_string(),
            other: if elsewhere.is_empty() {
                String::new()
            } else {
                format!(
                    " (the command text appears under another tool: {}, which is not a use \
                     of the governed tool)",
                    elsewhere.join(", ")
                )
            },
            why: match control {
                Control::Refusal => "a refusal of a request that was never made is not a refusal",
                Control::Allowed => {
                    "without an execution that succeeded, the refusal is still consistent with \
                     a payload that refuses everything"
                }
                Control::WithoutPayload => {
                    "a control that never attempted the command cannot say whether something \
                     other than the payload would have refused it"
                }
            },
        });
    }
    if let Some(other) = s
        .uses
        .iter()
        .find(|u| !(u.name == GOVERNED_TOOL && u.command() == Some(command)))
    {
        return Err(NotAdmitted::UnexpectedToolUse {
            control: c,
            tool: other.name.clone(),
            what: other
                .command()
                .map(str::to_string)
                .unwrap_or_else(|| other.input.to_string()),
        });
    }
    let mut refused = 0;
    let mut executed = Vec::new();
    for used in &governed {
        match s.classify(&used.id) {
            Ok(Use::Refused) => refused += 1,
            Ok(Use::Executed) => executed.push(*used),
            Err(why) => {
                return Err(NotAdmitted::Unresolved {
                    control: c,
                    id: used.id.clone(),
                    why,
                });
            }
        }
    }
    match control {
        Control::Refusal => {
            if refused == 0 {
                let (found, named) = s.denials_named();
                return Err(NotAdmitted::NoStructuredRefusal {
                    command: command.to_string(),
                    found,
                    named,
                });
            }
            if !executed.is_empty() {
                return Err(NotAdmitted::ExecutedDespiteRefusal {
                    command: command.to_string(),
                });
            }
        }
        Control::Allowed => {
            if refused > 0 {
                return Err(NotAdmitted::ControlRefused {
                    control: c,
                    command: command.to_string(),
                    why: "the payload refused a command no floor entry claims, so what it \
                          enforces is not the floor",
                });
            }
            let outputs: Vec<(bool, Option<String>)> = executed
                .iter()
                .map(|u| {
                    let r = &s.results[&u.id];
                    (r.is_error, r.text())
                })
                .collect();
            let produced = outputs.iter().any(|(is_error, text)| {
                !is_error
                    && text.as_deref().map(|t| t.trim_end_matches(['\n', '\r']))
                        == Some(evidence.allowed_output.as_str())
            });
            if !produced {
                return Err(NotAdmitted::WrongOutput {
                    command: command.to_string(),
                    expected: evidence.allowed_output.clone(),
                    found: outputs
                        .iter()
                        .map(|(e, t)| format!("is_error {e}, content {t:?}"))
                        .collect::<Vec<_>>()
                        .join("; "),
                });
            }
        }
        Control::WithoutPayload => {
            if refused > 0 {
                return Err(NotAdmitted::ControlRefused {
                    control: c,
                    command: command.to_string(),
                    why: "the command is refused without the payload too, so the refusal is \
                          evidence for the operator's own configuration and not for this payload",
                });
            }
        }
    }
    Ok(())
}

/// Admit a live-session observation, or say why it is not one.
///
/// Sections 3.29 and 3.30, in the order that refuses earliest. Nothing here
/// returns a weaker admission: a claim is admitted or it is refused, and a
/// refused claim leaves the session unverified. Admission does not decide
/// whether the observation is live; [`Evidence::synthetic`] does, and the
/// record that carries it is never qualified when it is.
pub fn admit(evidence: &Evidence) -> Result<(), NotAdmitted> {
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

    let controls = evidence.controls();
    // Rule 12: every launch record, before any capture is read.
    for (control, m) in controls {
        bound(evidence, control, m)?;
    }
    if evidence.allowed_output.is_empty() {
        return Err(NotAdmitted::NoExpectedOutput);
    }

    // Section 3.29 rule 3 and section 3.30 rule 12: three controls are three
    // launches of three sessions in one project.
    let pairs = [(0, 1), (0, 2), (1, 2)];
    for (i, j) in pairs {
        let ((first, a), (second, b)) = (controls[i], controls[j]);
        if a.capture.bytes == b.capture.bytes {
            return Err(NotAdmitted::SubstitutedEvidence {
                first: first.word(),
                second: second.word(),
                digest: a.capture.digest(),
            });
        }
        let (la, lb) = (a.launch.as_ref(), b.launch.as_ref());
        if let (Some(la), Some(lb)) = (la, lb) {
            if la.capture_id == lb.capture_id {
                return Err(NotAdmitted::SharedIdentity {
                    first: first.word(),
                    second: second.word(),
                    what: "the capture identity",
                    value: la.capture_id.clone(),
                });
            }
        }
        if a.invocation.working_directory != b.invocation.working_directory {
            return Err(NotAdmitted::DifferentWorkingDirectories {
                first: first.word(),
                a: a.invocation.working_directory.clone(),
                second: second.word(),
                b: b.invocation.working_directory.clone(),
            });
        }
    }

    // Rules 8 and 11: each capture read whole, and measurable.
    let mut streams = Vec::new();
    for (control, m) in controls {
        let s = read(control, &m.capture)?;
        if s.version != evidence.version {
            return Err(NotAdmitted::VersionMismatch {
                expected: evidence.version.clone(),
                found: s.version.clone(),
                control: control.word(),
                source_of_it: "init event",
            });
        }
        terminal_measurable(control, m, &s)?;
        streams.push((control, s));
    }
    for (i, j) in pairs {
        let ((first, a), (second, b)) = (&streams[i], &streams[j]);
        if a.session == b.session {
            return Err(NotAdmitted::SharedIdentity {
                first: first.word(),
                second: second.word(),
                what: "the session",
                value: a.session.clone(),
            });
        }
        if let Some(shared) = a.uses.iter().find(|u| b.uses.iter().any(|v| v.id == u.id)) {
            return Err(NotAdmitted::SharedIdentity {
                first: first.word(),
                second: second.word(),
                what: "the tool-use id",
                value: shared.id.clone(),
            });
        }
    }

    // Rules 9 and 10: what each control shows.
    for (control, s) in &streams {
        outcome(evidence, *control, s)?;
    }
    Ok(())
}

/// Admit an observation for one project: [`admit`], and the controls ran in
/// that project.
pub fn admit_in(evidence: &Evidence, root: &std::path::Path) -> Result<(), NotAdmitted> {
    admit(evidence)?;
    let expected = root
        .canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .display()
        .to_string();
    let found = &evidence.refusal.invocation.working_directory;
    if found != &expected {
        return Err(NotAdmitted::AnotherProject {
            expected,
            found: found.clone(),
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
        "{}`{}` was refused by the installed harness through a structured denial and did not \
         execute; `{}` executed under the same payload and printed its expected output; and \
         `{}` executed without the payload. Harness {}, payload {}",
        if evidence.synthetic() {
            "SYNTHETIC, not a live observation: "
        } else {
            ""
        },
        evidence.refused_command,
        evidence.allowed_command,
        evidence.refused_command,
        evidence.version,
        evidence.payload_digest,
    )
}

/// The file a control's launch record is written to, inside a capture
/// directory.
pub fn record_name(control: Control) -> String {
    format!("{}.json", control.word())
}

/// Why a capture directory could not be read into evidence.
///
/// Separate from [`NotAdmitted`] on purpose. Captures that cannot be read are
/// not a claim that was judged and refused; they are a claim that was never
/// stated, and reporting the two the same way would let a missing file look
/// like a measured negative.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotRead {
    /// The capture directory itself.
    #[error("the captures at {path} could not be read: {detail}")]
    Directory {
        /// Where they were looked for.
        path: String,
        /// What went wrong.
        detail: String,
    },
    /// One control's launch record.
    #[error(
        "the {control} control's capture record at {path} could not be read: {detail}. A \
         capture that is not there is not a weaker capture"
    )]
    Record {
        /// Which control.
        control: &'static str,
        /// Where it was looked for.
        path: String,
        /// What went wrong.
        detail: String,
    },
}

/// Read a capture directory's three launch records into evidence.
///
/// Reading only. Nothing is admitted here: [`admit`] is still the judge, and
/// captures that read cleanly can still be refused by every rule in sections
/// 3.29 and 3.30. The claim's version is the refusal launch's probed version,
/// and the claim's commands are the ones the launches ran, so the admission
/// checks the three launches against each other rather than against a
/// caller's statement.
pub fn load(directory: &std::path::Path) -> Result<Evidence, NotRead> {
    if !directory.is_dir() {
        return Err(NotRead::Directory {
            path: directory.display().to_string(),
            detail: "not a directory".to_string(),
        });
    }
    let read_one = |control: Control| -> Result<Measurement, NotRead> {
        let path = directory.join(record_name(control));
        let failed = |detail: String| NotRead::Record {
            control: control.word(),
            path: path.display().to_string(),
            detail,
        };
        let bytes = std::fs::read(&path).map_err(|e| failed(e.to_string()))?;
        serde_json::from_slice(&bytes).map_err(|e| failed(e.to_string()))
    };
    let refusal = read_one(Control::Refusal)?;
    let allowed = read_one(Control::Allowed)?;
    let without_payload = read_one(Control::WithoutPayload)?;
    let launch_of = |m: &Measurement| m.launch.clone();
    Ok(Evidence {
        version: launch_of(&refusal)
            .and_then(|l| l.probe_version)
            .unwrap_or_default(),
        payload_digest: crate::startup::payload_identity(),
        refused_command: launch_of(&refusal).map(|l| l.command).unwrap_or_default(),
        allowed_command: launch_of(&allowed).map(|l| l.command).unwrap_or_default(),
        allowed_output: ALLOWED_OUTPUT.to_string(),
        refusal,
        allowed,
        without_payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_experiments_commands_stand_where_the_floor_puts_them() {
        assert!(floor_claims(REFUSED_COMMAND));
        assert!(!floor_claims(ALLOWED_COMMAND));
        assert!(REFUSED_COMMAND.contains(REFUSED_COMMAND_ABSENT_PATH));
        assert!(ALLOWED_COMMAND.ends_with(ALLOWED_OUTPUT));
    }

    #[test]
    fn the_arguments_differ_between_controls_only_by_the_settings() {
        let with = arguments(Control::Refusal, "a", "b", Some("/s.json"));
        let allowed = arguments(Control::Allowed, "a", "b", Some("/s.json"));
        let without = arguments(Control::WithoutPayload, "a", "b", Some("/s.json"));
        assert_eq!(with, allowed);
        assert_eq!(&with[..with.len() - 2], without.as_slice());
        assert_eq!(&with[with.len() - 2..], ["--settings", "/s.json"]);
        // The grant is identical in all three and names exactly two commands.
        let grant = without.iter().position(|a| a == "--allowedTools").unwrap();
        assert_eq!(without[grant + 1..], ["Bash(a)", "Bash(b)"]);
    }

    /// Spec 002 section 5, 2026-09-22: the grant's two rules each contain
    /// spaces, and the installed 2.1.267 build splits `--allowedTools` values
    /// on commas and spaces **outside parentheses only** (its `sd` function,
    /// read statically from the shipped binary; the help text documents
    /// "Comma or space-separated", with `"Bash(git *) Edit"` as the example).
    /// This is a transcription of that splitter, so the test establishes that
    /// the argument construction survives it. It is not the provider parsing
    /// anything, and nothing here observes enforcement.
    #[test]
    fn each_rule_in_the_grant_survives_the_installed_splitter_whole() {
        fn split_like_2_1_267(values: &[String]) -> Vec<String> {
            let mut out = Vec::new();
            for value in values {
                let (mut current, mut inside) = (String::new(), false);
                for c in value.chars() {
                    match c {
                        '(' => {
                            inside = true;
                            current.push(c);
                        }
                        ')' => {
                            inside = false;
                            current.push(c);
                        }
                        ',' | ' ' if !inside => {
                            if !current.trim().is_empty() {
                                out.push(current.trim().to_string());
                            }
                            current.clear();
                        }
                        _ => current.push(c),
                    }
                }
                if !current.trim().is_empty() {
                    out.push(current.trim().to_string());
                }
            }
            out
        }
        for control in [Control::Refusal, Control::Allowed, Control::WithoutPayload] {
            let args = arguments(control, REFUSED_COMMAND, ALLOWED_COMMAND, Some("/s.json"));
            let grant = args.iter().position(|a| a == "--allowedTools").unwrap();
            // The option is variadic: it takes values up to the next option.
            let values: Vec<String> = args[grant + 1..]
                .iter()
                .take_while(|a| !a.starts_with("--"))
                .cloned()
                .collect();
            assert_eq!(
                split_like_2_1_267(&values),
                [
                    format!("{GOVERNED_TOOL}({REFUSED_COMMAND})"),
                    format!("{GOVERNED_TOOL}({ALLOWED_COMMAND})"),
                ]
            );
        }
        // Neither command may contain a parenthesis, which would end the
        // protected span early and split the rule.
        for command in [REFUSED_COMMAND, ALLOWED_COMMAND] {
            assert!(!command.contains(['(', ')']), "{command}");
        }
    }

    #[test]
    fn no_argument_carries_the_prompt() {
        let args = arguments(
            Control::Refusal,
            REFUSED_COMMAND,
            ALLOWED_COMMAND,
            Some("/s"),
        );
        assert!(!args.iter().any(|a| a.contains("Use the")));
        assert!(prompt(REFUSED_COMMAND).contains(REFUSED_COMMAND));
    }

    #[test]
    fn both_spellings_of_the_settings_argument_are_found() {
        let i = Invocation {
            program: "/p".into(),
            arguments: vec![
                "--settings".into(),
                "/a".into(),
                "--settings=/b".into(),
                "--settingsx".into(),
            ],
            working_directory: "/w".into(),
        };
        assert_eq!(
            i.settings_arguments(),
            [
                ("--settings".to_string(), Some("/a".to_string())),
                ("--settings=/b".to_string(), Some("/b".to_string())),
            ]
        );
    }
}
