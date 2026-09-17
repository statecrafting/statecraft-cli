//! Spawning an adapter, holding it to a deadline, and killing its descendants.
//!
//! Spec 004 sections 3.1, 3.3 and 3.5. The supervisor reads the manifest first,
//! **every time**: a required token the manifest lacks is a refusal before any
//! process is spawned, and there is no partial spawn and no post-hoc discovery.
//!
//! A hung child is killed at the deadline **with its descendants**, and the
//! attempt is `interrupted`. A child that leaves a process behind is reported as
//! a residual, never as a clean termination.
//!
//! The child runs **in the request's workspace**. Spec 003 section 3.2 prepares
//! an isolated worktree so the operator's checkout is never edited and no
//! session runs in it, and the spawn is where that holds or does not.

use crate::capability::{Negotiation, Requested, negotiate};
use crate::environment::{ChildEnvironment, EnvironmentState};
use crate::manifest::Manifest;
use crate::protocol::{Event, Request, StreamError, parse_event};
use statecraft_run::attempt::Outcome;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Why nothing was spawned.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpawnRefusal {
    /// A required capability the manifest lacks.
    ///
    /// Names the token **and** the adapter version, because the same token may
    /// be present in the next version and the operator needs to know which.
    #[error(
        "adapter {adapter} {version} does not support required capability `{token}`; \
         refused before spawn, no process created"
    )]
    MissingRequired {
        /// The adapter.
        adapter: String,
        /// Its version.
        version: String,
        /// The token it lacks.
        token: String,
    },
    /// The constructed environment refused.
    #[error("the constructed environment refused: {reasons}")]
    Environment {
        /// Why, joined.
        reasons: String,
    },
}

/// What a supervised run produced.
#[derive(Debug, Clone, PartialEq)]
pub struct Supervised<E = Event> {
    /// Every event read from the stream, in order.
    pub events: Vec<E>,
    /// The outcome the supervisor decided.
    pub outcome: Outcome,
    /// Set when the stream could not be read as a completion.
    pub stream_error: Option<StreamError>,
    /// Set when a process outlived the kill.
    pub surviving_processes: Option<String>,
}

/// Check a request against a manifest and an environment, before spawning.
///
/// Returns the negotiation when a spawn may proceed. The manifest is read here
/// and only here, which is what makes "the manifest is read first, every time"
/// structural rather than a convention.
pub fn preflight(
    manifest: &Manifest,
    requested: &Requested,
    environment: &ChildEnvironment,
) -> Result<Negotiation, SpawnRefusal> {
    let negotiation = negotiate(requested, &manifest.supports);
    if let Some(missing) = negotiation.missing_required.first() {
        return Err(SpawnRefusal::MissingRequired {
            adapter: manifest.adapter.clone(),
            version: manifest.version.clone(),
            token: missing.token().to_string(),
        });
    }
    if let EnvironmentState::Refused { reasons } = &environment.state {
        return Err(SpawnRefusal::Environment {
            reasons: reasons.join("; "),
        });
    }
    Ok(negotiation)
}

/// Spawn an adapter and read its event stream under a deadline.
///
/// The child is put in its own process group so the deadline can kill the whole
/// tree. A deny list of pids would be a list of the children somebody thought
/// of; a process group is the complete set.
///
/// The child's working directory is the request's workspace, and a workspace
/// that is not an existing directory is refused before anything is spawned.
pub fn supervise(
    program: &Path,
    args: &[&str],
    request: &Request,
    environment: &ChildEnvironment,
) -> std::io::Result<Supervised> {
    supervise_stream(program, args, request, environment, parse_event, |event| {
        matches!(event, Event::Result { .. })
    })
}

/// Supervise a typed stream using its owning adapter's decoder.
///
/// Only decoding and terminal recognition vary. The process, cwd, constructed
/// environment, deadline and descendant handling are the same as [`supervise`].
/// The caller maps the returned events and terminal fields at its own boundary;
/// this function's outcome describes transport completion, not the work's result.
pub fn supervise_stream<E: Send + 'static>(
    program: &Path,
    args: &[&str],
    request: &Request,
    environment: &ChildEnvironment,
    decode: fn(&str, usize) -> Result<E, StreamError>,
    is_terminal: fn(&E) -> bool,
) -> std::io::Result<Supervised<E>> {
    let workspace = workspace_to_enter(&request.workspace)?;

    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(workspace)
        .env_clear()
        .envs(&environment.variables)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Its own process group, so the deadline kills descendants too.
        command.process_group(0);
    }

    let mut child = command.spawn()?;
    let deadline = Instant::now() + Duration::from_secs(request.deadline_seconds);

    // The prompt goes on a stream. It is never interpolated into a command line,
    // which is why `args` above carries no prompt and cannot. A child that does
    // not read it must not hold the supervisor outside its deadline loop.
    let mut stdin = child.stdin.take().expect("stdin was piped");
    let prompt = request.prompt.clone();
    let writer = std::thread::spawn(move || {
        use std::io::Write;
        let _ = stdin.write_all(&prompt);
        // Dropping closes it, which is how the child learns the prompt ended.
    });

    let stdout = child.stdout.take().expect("stdout was piped");
    let (tx, rx) = mpsc::sync_channel(16);
    let reader = std::thread::spawn(move || {
        let mut stdout = BufReader::new(stdout);
        for (i, line) in (&mut stdout).lines().enumerate() {
            let Ok(line) = line else { return };
            if line.trim().is_empty() {
                continue;
            }
            let event = decode(&line, i + 1);
            let stop_parsing = event.as_ref().map_or(true, is_terminal);
            if tx.send(event).is_err() {
                return;
            }
            if stop_parsing {
                break;
            }
        }
        // A terminal event or malformed line ends the trusted prefix. Drain
        // raw bytes after it, keeping the channel open until the pipe closes.
        let _ = std::io::copy(&mut stdout, &mut std::io::sink());
    });

    let mut events = Vec::new();
    let mut stream_error = None;
    let mut timed_out = false;
    let mut eof = false;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            timed_out = true;
            break;
        }
        // Neither EOF nor a terminal event says the child has exited. Likewise,
        // child exit does not close a pipe still held by a descendant. Keep all
        // of these observations under the same deadline, without blocking on
        // wait or join. The exit code is not a verdict on the work.
        if eof && child.try_wait()?.is_some() && reader.is_finished() && writer.is_finished() {
            break;
        }
        let interval = remaining.min(Duration::from_millis(10));
        if eof {
            std::thread::sleep(interval);
            continue;
        }
        match rx.recv_timeout(interval) {
            Ok(Ok(event)) => {
                events.push(event);
            }
            Ok(Err(e)) => {
                // Keep the diagnostic and preceding evidence through cleanup.
                stream_error = Some(e);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => eof = true,
        }
    }

    let surviving = if timed_out {
        let residual = kill_tree(&mut child);
        // The reader is NOT joined here, deliberately. It is blocked on a pipe
        // whose writers include anything the child left behind, so joining it
        // would wait exactly as long as the deadline was supposed to prevent.
        // CI found this the expensive way: a backgrounded `sleep 300` in the
        // fixture survived the group kill on Linux, and the join then waited out
        // all 300 seconds after the deadline had correctly fired at one.
        //
        // A supervisor that can be held past its own deadline by the thing it is
        // supervising is not one, so the thread is dropped and the survivor is
        // reported as a residual instead.
        drop(reader);
        drop(writer);
        residual
    } else {
        // All three finished inside the deadline; neither join can now block
        // on a pipe. try_wait already reaped the child.
        let _ = reader.join();
        let _ = writer.join();
        None
    };

    let has_result = events.iter().any(is_terminal);
    if stream_error.is_none() && !has_result && !timed_out {
        stream_error = Some(StreamError::NoResult {
            events: events.len(),
        });
    }

    let outcome = if timed_out {
        // Nothing was judged, so this is interrupted and not failed.
        Outcome::Interrupted
    } else if stream_error.is_some() {
        // A stream that cannot be read is never a completion. It is also not a
        // judgement of the work, so it is interrupted rather than failed.
        Outcome::Interrupted
    } else {
        Outcome::Completed
    };

    Ok(Supervised {
        events,
        outcome,
        stream_error,
        surviving_processes: surviving,
    })
}

/// The directory the child is spawned in, or why it is not one.
///
/// Spec 003 section 3.2 prepares an isolated worktree and says the operator's
/// checkout is never edited and no session runs in it; section 3.1 of this
/// spec puts that workspace in the request. Without this the request carries
/// the path and the child inherits the supervisor's own directory instead,
/// which is the operator's checkout whenever the command was started there.
/// A live provider run measured exactly that: the caller's directory in the
/// child's init event.
///
/// The check is here rather than left to the spawn because the platform's own
/// answer is a bare `ENOENT` raised in the forked child, which reads the same
/// as an adapter binary that is not there. Naming the path is the shape
/// section 3.3 already uses for a required capability, and no process is
/// created in either case. A workspace removed between this check and the
/// spawn still fails at the spawn, which is the honest answer for a directory
/// that stopped existing.
fn workspace_to_enter(workspace: &Path) -> std::io::Result<&Path> {
    if !workspace.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "the request's workspace {} does not exist; refused before spawn, \
                 no process created",
                workspace.display()
            ),
        ));
    }
    if !workspace.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            format!(
                "the request's workspace {} is not a directory; refused before spawn, \
                 no process created",
                workspace.display()
            ),
        ));
    }
    Ok(workspace)
}

/// Kill the child and everything in its process group.
///
/// Returns a description when something survived, which becomes a residual on
/// the attempt rather than a footnote nobody reads.
#[cfg(unix)]
fn kill_tree(child: &mut std::process::Child) -> Option<String> {
    let pid = child.id();
    // Negative pid means the process group. `kill` is shelled out to rather than
    // linking libc for one signal: the dependency would be larger than the need.
    //
    // `-s KILL -- -PID`, not `-KILL -PID`. The `--` is load-bearing: without it
    // a negative pid is ambiguous with an option, and the two `kill`
    // implementations this runs on disagree about which it is. The BSD one on a
    // developer's machine killed the group; the procps one on the Linux runner
    // did not, and the difference was invisible until a backgrounded process
    // outlived the deadline in CI.
    let group = Command::new("kill")
        .args(["-s", "KILL", "--", &format!("-{pid}")])
        .output();
    let _ = child.kill();
    let _ = child.wait();

    if let Err(e) = group {
        return Some(format!("could not kill process group {pid}: {e}"));
    }

    // Ask whether the group still answers. Signal 0 delivers nothing and
    // succeeds only if something is there to receive it, so this checks that the
    // kill took, rather than that the command was well formed. A `kill` that
    // exits zero has said nothing about whether the descendants are gone.
    let still_there = Command::new("kill")
        .args(["-s", "0", "--", &format!("-{pid}")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if still_there {
        Some(format!(
            "process group {pid} still answers after SIGKILL; at least one descendant \
             outlived the deadline"
        ))
    } else {
        None
    }
}

#[cfg(not(unix))]
fn kill_tree(child: &mut std::process::Child) -> Option<String> {
    let pid = child.id();
    let _ = child.kill();
    let _ = child.wait();
    Some(format!(
        "descendants of {pid} are not killed on this platform; \
         a surviving process is possible and is reported rather than assumed away"
    ))
}
