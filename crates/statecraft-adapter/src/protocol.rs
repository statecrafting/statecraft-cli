//! The three parts of the seam: a request, an event stream, a result.
//!
//! Spec 004 section 3.1. None of them names a provider. The supervisor owns the
//! event stream and derives spec 003 section 3.5's refusal count from it; **the
//! adapter does not report a count**, which is why there is no field here for
//! one.

use crate::capability::Capability;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// What the supervisor writes to the adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// The prepared workspace the child works in.
    pub workspace: PathBuf,
    /// The base revision, already resolved to a commit by spec 003.
    pub base_commit: String,
    /// The prompt, as bytes on a stream.
    ///
    /// Section 3.1: **never interpolated into a shell command line.** Holding it
    /// as bytes rather than as a string is the same point one layer down: there
    /// is no encoding step here at which quoting could be got wrong.
    #[serde(with = "prompt_bytes")]
    pub prompt: Vec<u8>,
    /// What the run requires and prefers.
    pub capabilities: crate::capability::Requested,
    /// How long the child may take.
    pub deadline_seconds: u64,
    /// Which attempt this is, so a record can be tied to it.
    pub attempt: AttemptIdentity,
}

/// Which attempt a request belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptIdentity {
    /// The run.
    pub run_id: String,
    /// The attempt number within it.
    pub number: u32,
}

mod prompt_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&String::from_utf8_lossy(v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        Ok(String::deserialize(d)?.into_bytes())
    }
}

/// One item on the adapter's typed, ordered event stream.
///
/// No `Eq`: a cost carries an `f64`, and a currency amount is not a thing to
/// compare for exact equality anyway.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "event",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum Event {
    /// What the adapter **actually applied**, beside what was requested.
    ///
    /// Section 3.3: the effective configuration is evidence rather than an
    /// assumption about what the flags meant. An adapter that asked for four
    /// capabilities and applied two says so here, and section 3.5.6 makes the
    /// discrepancy a qualification failure.
    Init {
        /// What the provider actually applied.
        applied: Vec<Capability>,
        /// The adapter's version.
        adapter_version: String,
        /// The provider's version, as the adapter observed it.
        provider_version: String,
    },
    /// Something happened. Carried through, not interpreted.
    Progress {
        /// One line.
        message: String,
    },
    /// A guard refused something.
    ///
    /// The supervisor counts these. The adapter does not.
    Refusal {
        /// What refused.
        guard: String,
        /// What it refused.
        detail: String,
    },
    /// The terminal event.
    Result {
        /// How the adapter classifies its own termination. An input to the
        /// supervisor's decision, never an authority over it.
        classification: Classification,
        /// What the provider charged, if it reports a cost at all.
        ///
        /// `None` is rendered as `unknown`, **never as zero** (section 3.5.5).
        cost: Option<Cost>,
    },
}

/// How an adapter classifies its own termination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Classification {
    /// The session reached its own end.
    Completed,
    /// The session's work did not hold.
    Failed,
    /// The session was stopped.
    Stopped,
}

/// What a session cost, when the provider reports one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cost {
    /// The amount.
    pub amount: f64,
    /// Its unit, as the provider names it.
    pub unit: String,
}

/// A cost as it is reported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", untagged)]
pub enum ReportedCost {
    /// The provider reported one.
    Known(Cost),
    /// It did not. Rendered as the string `unknown`, so no reader can mistake
    /// it for a zero that somebody measured.
    Unknown(String),
}

impl ReportedCost {
    /// From an optional cost.
    pub fn from_option(cost: Option<Cost>) -> Self {
        match cost {
            Some(c) => ReportedCost::Known(c),
            None => ReportedCost::Unknown("unknown".to_string()),
        }
    }

    /// Whether a cost is actually known.
    pub fn is_known(&self) -> bool {
        matches!(self, ReportedCost::Known(_))
    }
}

/// What the supervisor records when the stream ends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterResult {
    /// The adapter's own classification.
    pub classification: Classification,
    /// What the init event said was applied.
    pub applied: Vec<Capability>,
    /// What the negotiation degraded.
    pub degraded: Vec<Capability>,
    /// The cost, known or `unknown`.
    pub cost: ReportedCost,
    /// The adapter's version.
    pub adapter_version: String,
    /// The provider's version.
    pub provider_version: String,
}

/// Why a stream could not be read as a result.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StreamError {
    /// A line was not a well-formed event.
    ///
    /// Section 3.5.4: reported as malformed. **Never read as success, and never
    /// as a clean completion with missing fields.**
    #[error("event stream is malformed at line {line}: {detail}")]
    Malformed {
        /// Which line.
        line: usize,
        /// What went wrong.
        detail: String,
    },
    /// The stream ended with no result event.
    ///
    /// Section 3.8: the attempt is `interrupted`, not `completed`.
    #[error("event stream ended after {events} event(s) with no result event")]
    NoResult {
        /// How many events did arrive.
        events: usize,
    },
    /// The stream had no init event, so nothing says what was applied.
    #[error("event stream carried no init event, so what the provider applied is unknown")]
    NoInit,
}

/// Read a typed event stream into a result.
///
/// Every failure mode here is a refusal to call something a completion. That is
/// the module's whole job: a stream this function cannot read is never a pass.
pub fn read_stream(
    events: &[Event],
    degraded: &[Capability],
) -> Result<AdapterResult, StreamError> {
    let mut applied = None;
    let mut versions = None;
    let mut terminal = None;

    for event in events {
        match event {
            Event::Init {
                applied: a,
                adapter_version,
                provider_version,
            } => {
                applied = Some(a.clone());
                versions = Some((adapter_version.clone(), provider_version.clone()));
            }
            Event::Result {
                classification,
                cost,
            } => terminal = Some((*classification, cost.clone())),
            Event::Progress { .. } | Event::Refusal { .. } => {}
        }
    }

    let (applied, (adapter_version, provider_version)) = match (applied, versions) {
        (Some(a), Some(v)) => (a, v),
        _ => return Err(StreamError::NoInit),
    };
    let (classification, cost) = terminal.ok_or(StreamError::NoResult {
        events: events.len(),
    })?;

    Ok(AdapterResult {
        classification,
        applied,
        degraded: degraded.to_vec(),
        cost: ReportedCost::from_option(cost),
        adapter_version,
        provider_version,
    })
}

/// Parse one JSON line as an event, naming the line when it is not one.
pub fn parse_event(line: &str, number: usize) -> Result<Event, StreamError> {
    serde_json::from_str(line).map_err(|e| StreamError::Malformed {
        line: number,
        detail: e.to_string(),
    })
}

/// Every refusal on a stream, for the supervisor to count.
pub fn refusals(events: &[Event]) -> Vec<statecraft_run::refusal::RefusalEvent> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Refusal { guard, detail } => Some(statecraft_run::refusal::RefusalEvent {
                guard: guard.clone(),
                detail: detail.clone(),
            }),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init() -> Event {
        Event::Init {
            applied: vec![Capability::TurnLimit],
            adapter_version: "1.0.0".into(),
            provider_version: "9.9".into(),
        }
    }

    #[test]
    fn a_stream_with_an_init_and_a_result_reads() {
        let r = read_stream(
            &[
                init(),
                Event::Result {
                    classification: Classification::Completed,
                    cost: None,
                },
            ],
            &[],
        )
        .unwrap();
        assert_eq!(r.classification, Classification::Completed);
        assert_eq!(r.applied, [Capability::TurnLimit]);
    }

    #[test]
    fn an_absent_cost_is_unknown_and_never_zero() {
        let r = read_stream(
            &[
                init(),
                Event::Result {
                    classification: Classification::Completed,
                    cost: None,
                },
            ],
            &[],
        )
        .unwrap();
        assert!(!r.cost.is_known());
        assert_eq!(r.cost, ReportedCost::Unknown("unknown".into()));
        let json = serde_json::to_string(&r.cost).unwrap();
        assert_eq!(json, "\"unknown\"");
        assert!(!json.contains('0'));
    }

    #[test]
    fn a_stream_with_no_result_event_is_not_a_completion() {
        match read_stream(
            &[
                init(),
                Event::Progress {
                    message: "x".into(),
                },
            ],
            &[],
        ) {
            Err(StreamError::NoResult { events }) => assert_eq!(events, 2),
            other => panic!("expected NoResult, got {other:?}"),
        }
    }

    #[test]
    fn a_stream_with_no_init_event_cannot_say_what_was_applied() {
        match read_stream(
            &[Event::Result {
                classification: Classification::Completed,
                cost: None,
            }],
            &[],
        ) {
            Err(StreamError::NoInit) => {}
            other => panic!("expected NoInit, got {other:?}"),
        }
    }

    #[test]
    fn a_malformed_line_names_the_line_and_is_never_a_success() {
        match parse_event("{not json", 3) {
            Err(StreamError::Malformed { line, .. }) => assert_eq!(line, 3),
            other => panic!("expected Malformed, got {other:?}"),
        }
    }

    #[test]
    fn refusals_are_collected_for_the_supervisor_to_count() {
        let events = vec![
            init(),
            Event::Refusal {
                guard: "g".into(),
                detail: "d".into(),
            },
            Event::Result {
                classification: Classification::Completed,
                cost: None,
            },
        ];
        assert_eq!(refusals(&events).len(), 1);
    }

    #[test]
    fn a_prompt_is_carried_as_bytes_not_spliced_into_a_command_line() {
        let r = Request {
            workspace: "/w".into(),
            base_commit: "abc".into(),
            prompt: b"rm -rf / ; echo \"quoted\"".to_vec(),
            capabilities: crate::capability::Requested::none(),
            deadline_seconds: 60,
            attempt: AttemptIdentity {
                run_id: "r".into(),
                number: 1,
            },
        };
        // The round trip proves the bytes survive intact; nothing here builds a
        // shell string out of them, and there is no API that would.
        let back: Request = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(back.prompt, r.prompt);
    }
}
