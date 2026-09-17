//! Connect the native stream to the existing mapping without teaching the
//! generic protocol a provider's spelling. Spec 008 sections 3.1 to 3.5.

use crate::outcome::TerminalReading;
use crate::stream::{MapError, ProviderEvent, ResultEvent, map_stream};
use statecraft_adapter::capability::Capability;
use statecraft_adapter::environment::ChildEnvironment;
use statecraft_adapter::protocol::{Classification, Request, StreamError};
use statecraft_adapter::supervisor::{Supervised, supervise_stream};
use statecraft_run::attempt::Outcome;
use std::path::Path;

/// A native execution mapped onto the generic seam, with its terminal evidence.
#[derive(Debug)]
pub struct Execution {
    /// Mapped events and process diagnostics, including a mapped outcome.
    pub supervised: Supervised,
    /// The existing outcome bridge's reading, separate from the provider claim.
    pub terminal: Option<TerminalReading>,
    /// The terminal fields as read, also retained for an unmapped terminal state.
    pub result: Option<ResultEvent>,
}

impl Execution {
    /// The input to the run supervisor's independent refusal accounting.
    ///
    /// A denied success still claims completion. Replacing the claim with the
    /// corrected refusal would erase that disagreement. Transport errors and
    /// unknown terminal states cannot make the observed outcome completed.
    pub fn termination(&self) -> statecraft_run::session::Termination {
        let adapter_claimed = self.result.as_ref().map_or(Outcome::Interrupted, |result| {
            match crate::outcome::provider_claim(result) {
                Classification::Completed => Outcome::Completed,
                Classification::Failed => Outcome::Failed,
                Classification::Stopped => Outcome::Interrupted,
            }
        });
        statecraft_run::session::Termination {
            observed: self.supervised.outcome,
            adapter_claimed,
        }
    }

    /// Evidence for the outcome record's extensible detail, not a new view of
    /// the command's closed outcome or serialization contract.
    pub fn evidence(&self) -> serde_json::Value {
        let terminal = self.result.as_ref().map(|r| {
            serde_json::json!({
                "subtype": r.subtype,
                "is_error": r.is_error,
                "terminal_reason": r.terminal_reason,
                "stop_reason": r.stop_reason,
                "num_turns": r.num_turns,
                "total_cost_usd": r.total_cost_usd,
                "permission_denials": r.permission_denials,
                "providerClaim": crate::outcome::provider_claim(r),
            })
        });
        serde_json::json!({
            "events": self.supervised.events,
            "providerTerminal": terminal,
            "streamError": self.supervised.stream_error.as_ref().map(ToString::to_string),
            "survivingProcesses": self.supervised.surviving_processes,
        })
    }
}

/// Supervise native JSONL, then use the provider's existing stream and outcome
/// mappings. The generic supervisor alone creates and controls the process.
pub fn supervise(
    program: &Path,
    args: &[&str],
    request: &Request,
    environment: &ChildEnvironment,
    granted: &[Capability],
) -> std::io::Result<Execution> {
    let native = supervise_stream(
        program,
        args,
        request,
        environment,
        |line, number| {
            serde_json::from_str(line)
                .map(|event| (number, event))
                .map_err(|e| StreamError::Malformed {
                    line: number,
                    detail: e.to_string(),
                })
        },
        |(_, event)| matches!(event, ProviderEvent::Result(_)),
    )?;
    let mut supervised = Supervised {
        events: Vec::new(),
        outcome: native.outcome,
        stream_error: native.stream_error,
        surviving_processes: native.surviving_processes,
    };
    let result_line = native.events.last().map(|(line, _)| *line).unwrap_or(1);
    let events: Vec<_> = native.events.into_iter().map(|(_, event)| event).collect();
    let mut result = events.iter().find_map(|event| match event {
        ProviderEvent::Result(result) => Some((**result).clone()),
        _ => None,
    });
    let mut terminal = None;
    match map_stream(&events, granted) {
        Ok(mapped) => {
            supervised.events = mapped.events;
            result = mapped.result;
            if let Some(r) = &result {
                match crate::outcome(r) {
                    Ok(reading) => {
                        if supervised.outcome == Outcome::Completed {
                            supervised.outcome = reading.outcome;
                        }
                        terminal = Some(reading);
                    }
                    Err(e) => {
                        supervised.stream_error = Some(StreamError::Malformed {
                            line: result_line,
                            detail: e.to_string(),
                        });
                    }
                }
            }
        }
        Err(error) => {
            // Preserve a transport diagnostic's exact physical line number.
            if supervised.stream_error.is_none() {
                supervised.stream_error = Some(match error {
                    MapError::NoInit => StreamError::NoInit,
                    MapError::Malformed { line, detail } => StreamError::Malformed { line, detail },
                });
            }
        }
    }
    if supervised.stream_error.is_some() {
        supervised.outcome = Outcome::Interrupted;
    }
    Ok(Execution {
        supervised,
        terminal,
        result,
    })
}
