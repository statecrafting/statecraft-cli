//! The provider's event stream, and the mapping onto spec 004 section 3.1.
//!
//! Spec 008 section 3.1 fixes which provider event is which part of the seam:
//!
//! | `004` section 3.1 part | Provider event |
//! |---|---|
//! | The request | Process arguments and a settings document, plus the prompt on stdin. |
//! | The init event | `{"type":"system","subtype":"init"}` |
//! | Progress events | `assistant` and `user` events, and `system` events including `hook_started` and `hook_response`. |
//! | Refusal events | There are none; see [`ResultEvent::permission_denials`]. |
//! | The result | `{"type":"result"}` |
//!
//! # A measured event section 3.1's table does not account for
//!
//! The recorded deny-rule stream carries a mid-stream
//! `{"type":"system","subtype":"permission_denied"}` event, with `tool_name`,
//! `tool_use_id`, `decision_reason_type` and a prose `message`. Section 3.1's
//! table says there are no refusal events, and
//! `tests/negative_cases.rs` records the contradiction as a fact rather than
//! resolving it: **deciding what section 3.1 should say is the owner's, not an
//! implementation's** (`.claude/rules/adversarial-prompt-refusal.md`).
//!
//! What this module does meanwhile is exactly what section 3.3 rule 1 requires
//! and nothing beyond it: the refusal record is `permission_denials`, read
//! verbatim off the result event, and the mid-stream event is carried through
//! as a progress event the way section 3.1 classifies every other `system`
//! event. Nothing is counted twice, and nothing is invented.

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

    /// Whether this is the mid-stream denial event section 3.1 does not list.
    pub fn is_permission_denied(&self) -> bool {
        self.subtype == "permission_denied"
    }
}

/// An `assistant` or `user` event. The body is carried, never interpreted.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageEvent {
    /// The provider's message object, opaque here.
    #[serde(default)]
    pub message: serde_json::Value,
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
                for denial in &r.permission_denials {
                    out.push(Event::Refusal {
                        guard: format!("permission-deny-rule/{}", denial.tool_name),
                        detail: serde_json::to_string(&denial)
                            .unwrap_or_else(|_| denial.tool_use_id.clone()),
                    });
                }
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

    #[test]
    fn a_denial_entrys_tool_input_survives_verbatim() {
        let line = r#"{"tool_name":"Bash","tool_use_id":"t1","tool_input":{"command":"echo hello","description":"Run echo hello"}}"#;
        let d: PermissionDenial = serde_json::from_str(line).unwrap();
        assert_eq!(d.tool_input["command"], "echo hello");
        assert_eq!(d.tool_input["description"], "Run echo hello");
    }
}
