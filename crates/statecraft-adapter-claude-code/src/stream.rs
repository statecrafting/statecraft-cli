//! The provider's event stream, and the mapping onto spec 004 section 3.1.
//!
//! Spec 004 section 3.9 fixes which provider event is which part of the seam:
//!
//! | `004` section 3.1 part | Provider event |
//! |---|---|
//! | The request | Process arguments and a settings document, plus the prompt on stdin. |
//! | The init event | `{"type":"system","subtype":"init"}` |
//! | Progress events | `assistant` and `user` events, and `system` events including `hook_started`, `hook_response` and `permission_denied`. |
//! | Refusal events | Derived from [`ResultEvent::permission_denials`], verbatim. |
//! | The result | `{"type":"result"}` |
//!
//! # The mid-stream denial notification, and why it is not a second refusal
//!
//! The recorded deny-rule stream carries a mid-stream
//! `{"type":"system","subtype":"permission_denied"}` event, with `tool_name`,
//! `tool_use_id`, `decision_reason_type` and a prose `message`, and its
//! tool-use id is the same one the terminal `permission_denials` entry names.
//! Section 3.1's table said there were no refusal events at all; the owner
//! amended it on 2026-09-17 to say what the recording shows, which is that the
//! notification is progress and the terminal entries are the refusal record.
//!
//! What this module does is section 3.3 rule 1 and nothing beyond it, and the
//! amendment did not change it: the refusal record is `permission_denials`,
//! read verbatim off the result event, and the mid-stream event is carried
//! through as a progress event the way section 3.1 classifies every other
//! `system` event. Counting both would count one denied tool use twice.
//! Nothing is counted twice, and nothing is invented.

use crate::capabilities;
use serde::{Deserialize, Serialize};
use statecraft_adapter::capability::Capability;
use statecraft_adapter::protocol::{Cost, Event};

/// One line of the provider's `stream-json` output.
///
/// `Unknown` is not a tolerance for malformed input: a line that is not JSON at
/// all still fails to deserialize, and spec 004 section 3.5 case 4 makes that
/// malformed. This variant is for a well-formed event of a `type` this build
/// does not map, which a provider may add at any time and which is not a defect
/// in the stream.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderEvent {
    /// A `system` event: `init`, `hook_started`, `hook_response` and others.
    ///
    /// Boxed for the same reason `Result` is: the init event carries nine
    /// fields, most of them optional strings, and an unboxed variant would make
    /// every progress event in a stream as large as the one init event.
    System(Box<SystemEvent>),
    /// An assistant turn.
    Assistant(MessageEvent),
    /// A tool result fed back to the model.
    User(MessageEvent),
    /// The terminal event.
    Result(Box<ResultEvent>),
    /// A well-formed event of a type this build does not map.
    #[serde(other)]
    Unknown,
}

/// A `system` event. Which fields are present depends on `subtype`.
///
/// Every field is optional because one struct covers `init`, the hook pair and
/// the rest: a separate type per subtype would refuse to deserialize the next
/// subtype the provider adds, and section 3.1 classifies the unknown ones as
/// progress rather than as errors.
#[derive(Debug, Clone, Deserialize)]
pub struct SystemEvent {
    /// Which system event this is.
    pub subtype: String,
    /// The session this event belongs to. Every recorded event carries one.
    #[serde(default)]
    pub session_id: Option<String>,
    /// The provider's own version, from the init event.
    #[serde(default)]
    pub claude_code_version: Option<String>,
    /// The model the session ran.
    #[serde(default)]
    pub model: Option<String>,
    /// The permission mode in force.
    #[serde(default, rename = "permissionMode")]
    pub permission_mode: Option<String>,
    /// The tool set **as applied**. Section 3.4: this reflects tool-set
    /// removal, and does not reflect an allowlist.
    #[serde(default)]
    pub tools: Option<Vec<String>>,
    /// Where the credential came from. Measured `"none"`, which is section
    /// 3.6's finding: this provider was not authenticated by an environment
    /// variable.
    #[serde(default, rename = "apiKeySource")]
    pub api_key_source: Option<String>,
    /// The working directory the session ran in.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Hook events name which hook ran.
    #[serde(default)]
    pub hook_name: Option<String>,
    /// And which event fired it.
    #[serde(default)]
    pub hook_event: Option<String>,
    /// A `permission_denied` event names the tool.
    #[serde(default)]
    pub tool_name: Option<String>,
    /// And the tool-use id, which ties it to the result's denial entry.
    #[serde(default)]
    pub tool_use_id: Option<String>,
    /// A `permission_denied` event's own classification of why. Recorded as
    /// `subcommandResults` on 2.1.267. Carried for a record, never decisive.
    #[serde(default)]
    pub decision_reason_type: Option<String>,
    /// Prose. Never parsed for a decision: spec 004 section 3.1 requires a
    /// refusal to be a structured event rather than text pulled out of a
    /// transcript, and this field is carried, not read.
    #[serde(default)]
    pub message: Option<String>,
}

impl SystemEvent {
    /// Whether this is the init event.
    pub fn is_init(&self) -> bool {
        self.subtype == "init"
    }

    /// Whether this is one of the hook events `hook-enforcement` rests on.
    pub fn is_hook(&self) -> bool {
        self.subtype == "hook_started" || self.subtype == "hook_response"
    }

    /// Whether this is the mid-stream denial notification section 3.1 lists as
    /// a progress event rather than as a refusal.
    pub fn is_permission_denied(&self) -> bool {
        self.subtype == "permission_denied"
    }
}

/// An `assistant` or `user` event.
///
/// The body is carried, and interpreted only as far as the two structured
/// blocks a tool call leaves: an assistant's `tool_use` and the `tool_result`
/// fed back in the next user event. Spec 002 section 3.30 rule 7 needs them
/// correlated, and typing them here keeps that reading in the adapter that owns
/// these bytes rather than in a second parser elsewhere. Text blocks are never
/// read for a decision.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageEvent {
    /// The provider's message object.
    #[serde(default)]
    pub message: serde_json::Value,
    /// The session this event belongs to.
    #[serde(default)]
    pub session_id: Option<String>,
    /// The harness's own notes on the tool results this event carries.
    ///
    /// Recorded on 2.1.267 beside a denied tool result as
    /// `{"id": <tool-use id>, "non_execution_kind": "permission-rule"}`, and
    /// absent beside a tool that ran. It is the harness saying a result is not
    /// an execution, which a result's error flag does not say: a command that
    /// ran and failed carries the same flag.
    #[serde(default)]
    pub tool_result_meta: Vec<ToolResultMeta>,
}

/// One entry of a user event's `tool_result_meta`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ToolResultMeta {
    /// The tool-use id the note is about.
    pub id: String,
    /// Why the tool did not execute, when it did not.
    #[serde(default)]
    pub non_execution_kind: Option<String>,
}

/// An assistant's request to run a tool. A request, not an execution.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ToolUse {
    /// The tool-use id every later event about this call names.
    pub id: String,
    /// Which tool.
    pub name: String,
    /// The input, verbatim.
    #[serde(default)]
    pub input: serde_json::Value,
}

impl ToolUse {
    /// The `command` the input names, when it names one.
    pub fn command(&self) -> Option<&str> {
        self.input.get("command").and_then(serde_json::Value::as_str)
    }
}

/// What the harness fed back for one tool use.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ToolResult {
    /// The tool use this answers.
    pub tool_use_id: String,
    /// The harness's error flag. Set for a refusal **and** for a command that
    /// ran and failed, so it does not say which.
    #[serde(default)]
    pub is_error: bool,
    /// The content, a string or a list of text blocks.
    #[serde(default)]
    pub content: serde_json::Value,
}

impl ToolResult {
    /// The content as text: the string itself, or the text blocks joined.
    ///
    /// `None` when the content is neither, which is a shape this build has not
    /// seen and does not guess at.
    pub fn text(&self) -> Option<String> {
        match &self.content {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Array(blocks) => blocks
                .iter()
                .map(|b| match b.get("type").and_then(serde_json::Value::as_str) {
                    Some("text") => b.get("text").and_then(serde_json::Value::as_str),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()
                .map(|parts| parts.concat()),
            _ => None,
        }
    }
}

impl MessageEvent {
    /// The message's content blocks of one type, typed.
    ///
    /// A block of that type that does not deserialize is an error rather than
    /// skipped: a tool use without an id cannot be correlated with anything,
    /// and silently dropping it would make a capture look like it held fewer
    /// calls than it did.
    fn blocks<T: serde::de::DeserializeOwned>(&self, kind: &str) -> Result<Vec<T>, String> {
        let Some(content) = self
            .message
            .get("content")
            .and_then(serde_json::Value::as_array)
        else {
            return Ok(Vec::new());
        };
        content
            .iter()
            .filter(|b| b.get("type").and_then(serde_json::Value::as_str) == Some(kind))
            .map(|b| serde_json::from_value(b.clone()).map_err(|e| format!("a {kind} block: {e}")))
            .collect()
    }

    /// Every tool use this message requests.
    pub fn tool_uses(&self) -> Result<Vec<ToolUse>, String> {
        self.blocks("tool_use")
    }

    /// Every tool result this message carries.
    pub fn tool_results(&self) -> Result<Vec<ToolResult>, String> {
        self.blocks("tool_result")
    }
}

/// One entry of the result event's refusal record.
///
/// Section 3.3 rule 1: reported **verbatim**. `tool_input` is held as a
/// [`serde_json::Value`] for that reason: a typed projection of it would be
/// this adapter deciding which parts of a refusal are worth keeping, which is a
/// classification section 3.3 forbids it from performing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionDenial {
    /// Which tool was denied.
    pub tool_name: String,
    /// The tool-use id.
    pub tool_use_id: String,
    /// The full tool input, verbatim.
    #[serde(default)]
    pub tool_input: serde_json::Value,
}

/// The provider's terminal event.
#[derive(Debug, Clone, Deserialize)]
pub struct ResultEvent {
    /// The provider's own terminal subtype: measured `"success"` and
    /// `"error_max_turns"`.
    pub subtype: String,
    /// The session this event ends.
    #[serde(default)]
    pub session_id: Option<String>,
    /// The provider's own error flag. **A claim, never an outcome.**
    #[serde(default)]
    pub is_error: bool,
    /// The provider's own terminal reason: measured `"completed"` and
    /// `"max_turns"`.
    #[serde(default)]
    pub terminal_reason: Option<String>,
    /// Why the model stopped.
    #[serde(default)]
    pub stop_reason: Option<String>,
    /// How many turns ran.
    #[serde(default)]
    pub num_turns: u32,
    /// What the session cost. Absent is `unknown`, never zero (spec 004
    /// section 3.5 case 5).
    #[serde(default)]
    pub total_cost_usd: Option<f64>,
    /// The refusal record, verbatim.
    #[serde(default)]
    pub permission_denials: Vec<PermissionDenial>,
}

impl ResultEvent {
    /// Whether the provider recorded any refusal.
    pub fn refused_anything(&self) -> bool {
        !self.permission_denials.is_empty()
    }

    /// Section 3.3 rule 1's refusal record, independent of initialization.
    /// The run supervisor counts these events, never an adapter-supplied count.
    pub(crate) fn refusal_events(&self) -> impl Iterator<Item = Event> + '_ {
        self.permission_denials.iter().map(|denial| Event::Refusal {
            guard: format!("permission-deny-rule/{}", denial.tool_name),
            detail: serde_json::to_string(denial).unwrap_or_else(|_| denial.tool_use_id.clone()),
        })
    }
}

/// Why a stream could not be mapped.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MapError {
    /// A line was not a well-formed event.
    ///
    /// Spec 004 section 3.5 case 4: reported as malformed, never read as a
    /// clean completion with missing fields.
    #[error("provider stream is malformed at line {line}: {detail}")]
    Malformed {
        /// Which line, one-based.
        line: usize,
        /// What the deserializer said.
        detail: String,
    },
    /// No init event, so nothing says what the provider applied.
    #[error("provider stream carried no init event, so what was applied is unknown")]
    NoInit,
}

/// Read a recorded or live `stream-json` output into provider events.
///
/// Blank lines are skipped; anything else that does not deserialize is
/// [`MapError::Malformed`] naming its line.
pub fn read_jsonl(text: &str) -> Result<Vec<ProviderEvent>, MapError> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event: ProviderEvent = serde_json::from_str(line).map_err(|e| MapError::Malformed {
            line: i + 1,
            detail: e.to_string(),
        })?;
        out.push(event);
    }
    Ok(out)
}

/// What a mapped stream carries beside the seam's own events.
#[derive(Debug, Clone)]
pub struct Mapped {
    /// The seam's typed events, in order.
    pub events: Vec<Event>,
    /// The result event, when the stream reached one.
    pub result: Option<ResultEvent>,
    /// The provider version the init event reported.
    pub provider_version: String,
    /// The tool set the init event reported as applied.
    pub applied_tools: Vec<String>,
    /// Mid-stream `permission_denied` events. Recorded because they were
    /// measured, and **not** counted: the refusal record is
    /// [`ResultEvent::permission_denials`] (section 3.3 rule 1).
    pub mid_stream_denials: Vec<String>,
}

/// Map a provider stream onto spec 004 section 3.1's three parts.
///
/// `granted` is what the negotiation granted, which is what the init event is
/// checked against. The seam's `Init` event carries what this adapter
/// **applied**, and section 3.4 is why that is not simply `granted`: the applied
/// allowlist is not observable, so [`crate::denial::ToolRestriction`] reports it
/// as `not-recorded` rather than restating the request as the answer.
pub fn map_stream(events: &[ProviderEvent], granted: &[Capability]) -> Result<Mapped, MapError> {
    let mut out = Vec::new();
    let mut result = None;
    let mut provider_version = None;
    let mut applied_tools = Vec::new();
    let mut mid_stream_denials = Vec::new();
    let mut saw_hook = false;

    for event in events {
        match event {
            ProviderEvent::System(s) if s.is_init() => {
                provider_version = s.claude_code_version.clone();
                applied_tools = s.tools.clone().unwrap_or_default();
                out.push(Event::Init {
                    applied: capabilities::applied(granted, s, saw_hook),
                    adapter_version: env!("CARGO_PKG_VERSION").to_string(),
                    provider_version: s.claude_code_version.clone().unwrap_or_else(|| {
                        statecraft_envelope::absence::Absence::NotRecorded
                            .word()
                            .to_string()
                    }),
                });
            }
            ProviderEvent::System(s) => {
                if s.is_hook() {
                    saw_hook = true;
                }
                if s.is_permission_denied() {
                    mid_stream_denials.push(s.tool_name.clone().unwrap_or_default());
                }
                out.push(Event::Progress {
                    message: format!(
                        "system/{}{}",
                        s.subtype,
                        s.hook_name
                            .as_ref()
                            .map(|h| format!(" {h}"))
                            .unwrap_or_default()
                    ),
                });
            }
            ProviderEvent::Assistant(_) => out.push(Event::Progress {
                message: "assistant".to_string(),
            }),
            ProviderEvent::User(_) => out.push(Event::Progress {
                message: "user".to_string(),
            }),
            ProviderEvent::Result(r) => {
                // Section 3.3 rule 1: the denial entries ARE the refusal record.
                // The supervisor derives spec 003 section 3.5's count from the
                // event stream, so they are put on the stream here rather than
                // handed over as a side channel, and they are put there exactly
                // once.
                out.extend(r.refusal_events());
                out.push(Event::Result {
                    // The provider's claim about itself, as an input. Section
                    // 3.5's mapping is [`crate::outcome`] and the supervisor's
                    // answer is spec 003's.
                    classification: crate::outcome::provider_claim(r),
                    cost: r.total_cost_usd.map(|amount| Cost {
                        amount,
                        unit: "USD".to_string(),
                    }),
                });
                result = Some((**r).clone());
            }
            ProviderEvent::Unknown => out.push(Event::Progress {
                message: "unmapped-event".to_string(),
            }),
        }
    }

    let provider_version = provider_version.ok_or(MapError::NoInit)?;
    Ok(Mapped {
        events: out,
        result,
        provider_version,
        applied_tools,
        mid_stream_denials,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_that_is_not_an_event_is_malformed_and_names_its_line() {
        match read_jsonl("{\"type\":\"result\",\"subtype\":\"success\"}\nnot json\n") {
            Err(MapError::Malformed { line, .. }) => assert_eq!(line, 2),
            other => panic!("expected Malformed, got {other:?}"),
        }
    }

    #[test]
    fn a_stream_with_no_init_event_cannot_say_what_was_applied() {
        let events =
            read_jsonl("{\"type\":\"result\",\"subtype\":\"success\",\"num_turns\":1}\n").unwrap();
        assert!(matches!(map_stream(&events, &[]), Err(MapError::NoInit)));
    }

    #[test]
    fn an_event_type_this_build_does_not_map_is_progress_and_not_an_error() {
        let events = read_jsonl("{\"type\":\"rate_limit_event\",\"x\":1}\n").unwrap();
        assert!(matches!(events[0], ProviderEvent::Unknown));
    }

    /// The recorded denied stream: the request, the refusal and the result are
    /// three events about one tool-use id, and the result is marked as not
    /// executed by the harness itself.
    #[test]
    fn a_recorded_denial_correlates_request_result_and_refusal_by_one_id() {
        let events = read_jsonl(include_str!("../testdata/stream/denied.jsonl")).unwrap();
        let mut uses = Vec::new();
        let mut results = Vec::new();
        let mut meta = Vec::new();
        let mut sessions = std::collections::BTreeSet::new();
        let mut mid_stream = Vec::new();
        let mut denials = Vec::new();
        for event in &events {
            match event {
                ProviderEvent::System(s) => {
                    sessions.insert(s.session_id.clone());
                    if s.is_permission_denied() {
                        mid_stream.push((s.tool_use_id.clone(), s.decision_reason_type.clone()));
                    }
                }
                ProviderEvent::Assistant(m) => {
                    sessions.insert(m.session_id.clone());
                    uses.extend(m.tool_uses().unwrap());
                }
                ProviderEvent::User(m) => {
                    sessions.insert(m.session_id.clone());
                    results.extend(m.tool_results().unwrap());
                    meta.extend(m.tool_result_meta.clone());
                }
                ProviderEvent::Result(r) => {
                    sessions.insert(r.session_id.clone());
                    denials.extend(r.permission_denials.clone());
                }
                ProviderEvent::Unknown => {}
            }
        }
        assert_eq!(sessions.len(), 1, "one session: {sessions:?}");
        assert!(sessions.iter().all(Option::is_some));
        assert_eq!(uses.len(), 1);
        let id = &uses[0].id;
        assert_eq!(uses[0].name, "Bash");
        assert_eq!(uses[0].command(), Some("echo hello"));
        assert_eq!(results.len(), 1);
        assert_eq!(&results[0].tool_use_id, id);
        assert!(results[0].is_error);
        assert_eq!(
            meta,
            [ToolResultMeta {
                id: id.clone(),
                non_execution_kind: Some("permission-rule".into())
            }]
        );
        assert_eq!(mid_stream, [(Some(id.clone()), Some("subcommandResults".into()))]);
        assert_eq!(denials.len(), 1);
        assert_eq!(&denials[0].tool_use_id, id);
        assert_eq!(denials[0].tool_input, uses[0].input);
    }

    /// The recorded turn-capped stream: a tool that ran has a result and no
    /// non-execution note, and its result precedes the capped terminal event.
    #[test]
    fn a_recorded_execution_has_a_result_and_no_non_execution_note() {
        let events = read_jsonl(include_str!("../testdata/stream/max-turns.jsonl")).unwrap();
        let mut order = Vec::new();
        for event in &events {
            match event {
                ProviderEvent::Assistant(m) => {
                    for u in m.tool_uses().unwrap() {
                        order.push(format!("use {}", u.id));
                    }
                }
                ProviderEvent::User(m) => {
                    assert!(m.tool_result_meta.is_empty());
                    for r in m.tool_results().unwrap() {
                        assert!(!r.is_error);
                        assert!(r.text().is_some());
                        order.push(format!("result {}", r.tool_use_id));
                    }
                }
                ProviderEvent::Result(r) => {
                    assert_eq!(r.terminal_reason.as_deref(), Some("max_turns"));
                    order.push("terminal".into());
                }
                _ => {}
            }
        }
        assert_eq!(order.len(), 3, "{order:?}");
        assert!(order[0].starts_with("use "));
        assert_eq!(order[1].replacen("result", "use", 1), order[0]);
        assert_eq!(order[2], "terminal");
    }

    #[test]
    fn a_tool_use_block_without_an_id_is_an_error_and_not_skipped() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"x"}}]}}"#;
        let events = read_jsonl(line).unwrap();
        let ProviderEvent::Assistant(m) = &events[0] else {
            panic!("an assistant event")
        };
        assert!(m.tool_uses().is_err());
    }

    #[test]
    fn a_denial_entrys_tool_input_survives_verbatim() {
        let line = r#"{"tool_name":"Bash","tool_use_id":"t1","tool_input":{"command":"echo hello","description":"Run echo hello"}}"#;
        let d: PermissionDenial = serde_json::from_str(line).unwrap();
        assert_eq!(d.tool_input["command"], "echo hello");
        assert_eq!(d.tool_input["description"], "Run echo hello");
    }
}
