//! Section 3.2: Codex's JSONL stream and the outcome mapping.
//!
//! | Event | Read as |
//! |---|---|
//! | `thread.started` | The provider session id (`thread_id`). A second, different id is a fault. |
//! | `turn.completed` | One terminal turn that finished; `usage` carries input, cached input and output tokens. |
//! | `turn.failed` | One terminal turn that failed. |
//! | `error` | A fault. |
//!
//! Every other event is progress: carried through by its type alone, never by
//! its content, so nothing the model wrote reaches a decision (suite row 9).
//!
//! Exactly one terminal turn and no fault is `completed`. Anything else is
//! `interrupted`: no terminal turn, two, an `error` event, a changing thread
//! id, and every `turn.failed`, because no fixture yet shows a `turn.failed`
//! shape that is a judged result (section 3.2: until a fixture shows the
//! shape, it maps to `interrupted`). `failed` is therefore never produced
//! here yet, and `cancelled` never is: only the supervisor knows an operator
//! stopped an attempt. The exit code is read nowhere.

use serde::{Deserialize, Serialize};
use statecraft_adapter::protocol::{Classification, Event};
use statecraft_run::attempt::Outcome;

/// One line of `codex exec --json`, as this adapter reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexEvent {
    /// `thread.started`: the provider session id.
    ThreadStarted {
        /// `thread_id`.
        thread_id: String,
    },
    /// `turn.completed`.
    TurnCompleted {
        /// Its token usage, when it carried one.
        usage: Option<Usage>,
    },
    /// `turn.failed`, with the provider's error object kept as evidence.
    TurnFailed {
        /// `error`, verbatim. Read by no decision.
        error: serde_json::Value,
    },
    /// `error`: a fault.
    Error,
    /// Anything else: its `type`, and its `item.type` when it has one.
    Progress {
        /// `type`.
        kind: String,
        /// `item.type`, for `item.*` events.
        item: Option<String>,
    },
}

/// Who reported a usage figure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provenance {
    /// The provider's own stream.
    Provider,
}

/// Token usage from `turn.completed`, recorded with provenance `provider`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// `input_tokens`.
    pub input_tokens: u64,
    /// `cached_input_tokens`.
    pub cached_input_tokens: u64,
    /// `output_tokens`.
    pub output_tokens: u64,
    /// Always [`Provenance::Provider`].
    pub provenance: Provenance,
}

#[derive(Deserialize)]
struct RawUsage {
    input_tokens: u64,
    #[serde(default)]
    cached_input_tokens: u64,
    output_tokens: u64,
}

/// Parse one line. An object without a string `type`, a `thread.started`
/// without a string `thread_id`, or an unreadable `usage` is malformed.
///
/// # Errors
///
/// What was wrong, for the seam's [`statecraft_adapter::StreamError::Malformed`].
pub fn parse_line(line: &str) -> Result<CodexEvent, String> {
    let value: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    let kind = value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .ok_or("an event without a string `type`")?;
    Ok(match kind {
        "thread.started" => CodexEvent::ThreadStarted {
            thread_id: value
                .get("thread_id")
                .and_then(serde_json::Value::as_str)
                .ok_or("`thread.started` without a string `thread_id`")?
                .to_string(),
        },
        "turn.completed" => CodexEvent::TurnCompleted {
            usage: match value.get("usage") {
                None | Some(serde_json::Value::Null) => None,
                Some(u) => {
                    let raw: RawUsage = serde_json::from_value(u.clone())
                        .map_err(|e| format!("`turn.completed` usage: {e}"))?;
                    Some(Usage {
                        input_tokens: raw.input_tokens,
                        cached_input_tokens: raw.cached_input_tokens,
                        output_tokens: raw.output_tokens,
                        provenance: Provenance::Provider,
                    })
                }
            },
        },
        "turn.failed" => CodexEvent::TurnFailed {
            error: value.get("error").cloned().unwrap_or_default(),
        },
        "error" => CodexEvent::Error,
        other => CodexEvent::Progress {
            kind: other.to_string(),
            item: value
                .pointer("/item/type")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
        },
    })
}

/// Parse a whole recorded stream, skipping blank lines.
///
/// # Errors
///
/// The 1-based physical line and what was wrong with it.
pub fn read_jsonl(text: &str) -> Result<Vec<CodexEvent>, (usize, String)> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| parse_line(l).map_err(|e| (i + 1, e)))
        .collect()
}

/// Why a stream is not a completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "fault")]
pub enum Fault {
    /// No `thread.started`, so no session id.
    NoThread,
    /// A second, different `thread_id`.
    ThreadChanged {
        /// The first id read.
        first: String,
        /// The id that differed.
        second: String,
    },
    /// An `error` event.
    ErrorEvent,
    /// Not exactly one terminal turn.
    TerminalTurns {
        /// How many there were.
        count: usize,
    },
    /// A `turn.failed` whose shape no fixture has shown to be judged.
    UnjudgedTurnFailure,
}

/// What section 3.2's mapping read from a stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    /// `completed` when there is no fault, `interrupted` otherwise.
    pub outcome: Outcome,
    /// The provider's own terminal classification, kept as a claim.
    pub provider_claim: Option<Classification>,
    /// The first `thread_id`.
    pub thread_id: Option<String>,
    /// The terminal turn's usage.
    pub usage: Option<Usage>,
    /// Every reason this is not a completion, in the order found.
    pub faults: Vec<Fault>,
}

/// Apply section 3.2's mapping to a whole stream.
pub fn read(events: &[CodexEvent]) -> Reading {
    let mut faults = Vec::new();
    let mut thread_id: Option<String> = None;
    let mut terminals = 0;
    let mut provider_claim = None;
    let mut usage = None;
    let mut unjudged = false;
    for event in events {
        match event {
            CodexEvent::ThreadStarted { thread_id: id } => match &thread_id {
                None => thread_id = Some(id.clone()),
                Some(first) if first != id => faults.push(Fault::ThreadChanged {
                    first: first.clone(),
                    second: id.clone(),
                }),
                Some(_) => {}
            },
            CodexEvent::TurnCompleted { usage: u } => {
                terminals += 1;
                provider_claim.get_or_insert(Classification::Completed);
                if usage.is_none() {
                    usage.clone_from(u);
                }
            }
            CodexEvent::TurnFailed { .. } => {
                terminals += 1;
                provider_claim.get_or_insert(Classification::Failed);
                unjudged = true;
            }
            CodexEvent::Error => faults.push(Fault::ErrorEvent),
            CodexEvent::Progress { .. } => {}
        }
    }
    if thread_id.is_none() {
        faults.insert(0, Fault::NoThread);
    }
    if terminals != 1 {
        faults.push(Fault::TerminalTurns { count: terminals });
    }
    if unjudged {
        faults.push(Fault::UnjudgedTurnFailure);
    }
    Reading {
        outcome: if faults.is_empty() {
            Outcome::Completed
        } else {
            Outcome::Interrupted
        },
        provider_claim,
        thread_id,
        usage,
        faults,
    }
}

/// The seam's events for a stream. `thread.started` is the init: it applies
/// nothing, because no token is measured, and names the provider version the
/// caller observed, because the stream carries none. Cost is `null` (section
/// 3.2: unless the stream proves one, and this one never has).
pub fn to_seam(events: &[CodexEvent], provider_version: &str) -> Vec<Event> {
    let mut seen_init = false;
    events
        .iter()
        .map(|event| match event {
            CodexEvent::ThreadStarted { .. } if !seen_init => {
                seen_init = true;
                Event::Init {
                    applied: Vec::new(),
                    adapter_version: env!("CARGO_PKG_VERSION").to_string(),
                    provider_version: provider_version.to_string(),
                    context: None,
                    documentation: None,
                }
            }
            CodexEvent::ThreadStarted { .. } => Event::Progress {
                message: "thread.started".into(),
            },
            CodexEvent::TurnCompleted { .. } => Event::Result {
                classification: Classification::Completed,
                cost: None,
                documentation: None,
            },
            CodexEvent::TurnFailed { .. } => Event::Result {
                classification: Classification::Failed,
                cost: None,
                documentation: None,
            },
            CodexEvent::Error => Event::Progress {
                message: "error".into(),
            },
            CodexEvent::Progress { kind, item } => Event::Progress {
                message: match item {
                    Some(item) => format!("{kind}/{item}"),
                    None => kind.clone(),
                },
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(lines: &[&str]) -> Vec<CodexEvent> {
        read_jsonl(&lines.join("\n")).unwrap()
    }

    const STARTED: &str = r#"{"type":"thread.started","thread_id":"t-1"}"#;
    const DONE: &str = r#"{"type":"turn.completed","usage":{"input_tokens":3,"cached_input_tokens":1,"output_tokens":2}}"#;

    #[test]
    fn one_completed_turn_is_completed_with_provider_usage() {
        let r = read(&stream(&[STARTED, DONE]));
        assert_eq!(r.outcome, Outcome::Completed);
        assert_eq!(r.usage.unwrap().provenance, Provenance::Provider);
        assert_eq!(r.thread_id.as_deref(), Some("t-1"));
    }

    #[test]
    fn every_other_shape_is_interrupted() {
        let failed = r#"{"type":"turn.failed","error":{"message":"x"}}"#;
        let error = r#"{"type":"error","message":"x"}"#;
        let other = r#"{"type":"thread.started","thread_id":"t-2"}"#;
        for (lines, fault) in [
            (vec![STARTED], Fault::TerminalTurns { count: 0 }),
            (vec![STARTED, DONE, DONE], Fault::TerminalTurns { count: 2 }),
            (vec![STARTED, error, DONE], Fault::ErrorEvent),
            (vec![STARTED, failed], Fault::UnjudgedTurnFailure),
            (vec![DONE], Fault::NoThread),
            (
                vec![STARTED, other, DONE],
                Fault::ThreadChanged {
                    first: "t-1".into(),
                    second: "t-2".into(),
                },
            ),
        ] {
            let r = read(&stream(&lines));
            assert_eq!(r.outcome, Outcome::Interrupted, "{lines:?}");
            assert!(r.faults.contains(&fault), "{lines:?}: {:?}", r.faults);
        }
    }

    #[test]
    fn a_repeated_identical_thread_id_is_not_a_change() {
        assert_eq!(read(&stream(&[STARTED, STARTED, DONE])).faults, []);
    }

    #[test]
    fn progress_carries_the_type_and_never_the_content() {
        let item =
            r#"{"type":"item.completed","item":{"type":"agent_message","text":"secret words"}}"#;
        let events = to_seam(&stream(&[STARTED, item, DONE]), "0.163.0");
        let text = serde_json::to_string(&events).unwrap();
        assert!(text.contains("item.completed/agent_message"));
        assert!(!text.contains("secret words"));
    }
}
