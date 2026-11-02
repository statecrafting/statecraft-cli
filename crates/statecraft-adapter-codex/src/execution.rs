//! The supervised turn, mapped onto the seam.
//!
//! The seam's supervisor spawns, writes the prompt to standard input, holds
//! the deadline and kills descendants; this module only decodes and maps.
//!
//! **The stream is read to its end.** The supervisor stops decoding at the
//! first line its terminal predicate accepts, which would hide a second
//! terminal turn (section 3.2, obligation R-1). So no line is terminal to the
//! supervisor, a clean end of stream comes back as its `NoResult`, and that
//! one diagnostic is replaced here by section 3.2's own reading of the whole
//! stream. Every other transport diagnostic (malformed line, read failure,
//! deadline, a stopped watch) stands, and the attempt stays `interrupted`.

use crate::spawn::Invocation;
use crate::stream::{CodexEvent, Fault, Reading, parse_line, read, to_seam};
use statecraft_adapter::environment::ChildEnvironment;
use statecraft_adapter::protocol::{Request, StreamError};
use statecraft_adapter::supervisor::{Supervised, Unwatched, supervise_watched};
use statecraft_run::attempt::Outcome;
use std::path::Path;

/// One supervised turn.
#[derive(Debug)]
pub struct Execution {
    /// The seam's events and the decided outcome.
    pub supervised: Supervised,
    /// Section 3.2's reading, when the stream was read to its end.
    pub reading: Option<Reading>,
    /// The native events, in order, as decoded.
    pub native: Vec<CodexEvent>,
}

/// Run `invocation` under the seam's supervisor and map what it emitted.
///
/// `provider_version` is what a probe observed (`codex --version`), because
/// the stream carries none.
///
/// # Errors
///
/// When the process could not be spawned.
pub fn supervise(
    invocation: &Invocation,
    request: &Request,
    environment: &ChildEnvironment,
    provider_version: &str,
) -> std::io::Result<Execution> {
    let native = supervise_watched(
        Path::new(&invocation.program),
        &invocation.args(),
        request,
        environment,
        |line: &str, number: usize| {
            parse_line(line).map_err(|detail| StreamError::Malformed {
                line: number,
                detail,
            })
        },
        |_: &CodexEvent| false,
        &mut Unwatched,
    )?;
    Ok(map(native, provider_version))
}

/// Map what the supervisor read. A function of its input alone, so every
/// branch is testable without a process.
pub fn map(native: Supervised<CodexEvent>, provider_version: &str) -> Execution {
    let read_to_end = matches!(native.stream_error, Some(StreamError::NoResult { .. }));
    let mut supervised = Supervised {
        events: to_seam(&native.events, provider_version),
        outcome: native.outcome,
        stream_error: native.stream_error,
        surviving_processes: native.surviving_processes,
        stopped: native.stopped,
        timed_out: native.timed_out,
    };
    let reading = read_to_end.then(|| read(&native.events));
    if let Some(reading) = &reading {
        supervised.outcome = reading.outcome;
        supervised.stream_error = if reading.faults.contains(&Fault::NoThread) {
            Some(StreamError::NoInit)
        } else if reading.faults.contains(&Fault::TerminalTurns { count: 0 }) {
            Some(StreamError::NoResult {
                events: native.events.len(),
            })
        } else {
            None
        };
    } else {
        supervised.outcome = Outcome::Interrupted;
    }
    Execution {
        supervised,
        reading,
        native: native.events,
    }
}
