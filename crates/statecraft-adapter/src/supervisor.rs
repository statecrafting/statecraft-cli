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
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::thread::JoinHandle;
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
    /// Set when the caller's [`Watch`] refused the spawn or asked for the
    /// process to be stopped, with the reason it gave. A stopped supervision is
    /// `interrupted`: the process did not end by itself.
    pub stopped: Option<String>,
    /// Whether the deadline ended the process. Only the supervisor observes
    /// this directly: a caller that maps the stream afterwards may add a
    /// stream error of its own (no init event, say), and that error must not
    /// hide that the deadline is what stopped the process.
    pub timed_out: bool,
}

/// What a [`Watch`] asks of the supervisor after it has seen an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    /// Keep reading.
    Continue,
    /// Kill the process group now, as at the deadline, keeping every event
    /// read so far, and report this reason.
    Stop(String),
}

/// A caller's view of one supervised launch (spec 004 section 5, 2026-09-22).
///
/// Both methods run on the supervising thread, so a watch may write files and
/// hold state without synchronizing. Neither is given the prompt.
pub trait Watch<E> {
    /// The spawn call returned a process with this id. Called before the
    /// prompt writer starts: an `Err` kills the process group, no prompt is
    /// written, and the supervision reports the reason as `stopped`.
    fn spawned(&mut self, pid: u32) -> Result<(), String>;

    /// One decoded event, in stream order, before the next is read.
    fn event(&mut self, event: &E) -> Control;
}

/// The watch a caller that supplies none gets: it confirms every spawn and
/// never stops anything, so supervision is exactly what it was without one.
pub struct Unwatched;

impl<E> Watch<E> for Unwatched {
    fn spawned(&mut self, _pid: u32) -> Result<(), String> {
        Ok(())
    }
    fn event(&mut self, _event: &E) -> Control {
        Control::Continue
    }
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
    supervise_watched(
        program,
        args,
        request,
        environment,
        decode,
        is_terminal,
        &mut Unwatched,
    )
}

/// [`supervise_stream`], with a caller's [`Watch`] told of the spawn before the
/// prompt is delivered and shown each event as it is read.
///
/// Spec 004 section 5, 2026-09-22, for spec 002 section 3.32: a launch whose
/// spawn confirmation must be persisted before the process is given work, and
/// whose startup decision may stop the process at an event.
pub fn supervise_watched<E: Send + 'static>(
    program: &Path,
    args: &[&str],
    request: &Request,
    environment: &ChildEnvironment,
    decode: fn(&str, usize) -> Result<E, StreamError>,
    is_terminal: fn(&E) -> bool,
    watch: &mut dyn Watch<E>,
) -> std::io::Result<Supervised<E>> {
    let spawned = match spawn_supervised(program, args, request, environment, &mut |pid| {
        watch.spawned(pid)
    })? {
        Launched::Running(spawned) => spawned,
        Launched::Refused {
            reason,
            surviving_processes,
        } => {
            return Ok(Supervised {
                events: Vec::new(),
                outcome: Outcome::Interrupted,
                stream_error: None,
                surviving_processes,
                stopped: Some(reason),
                timed_out: false,
            });
        }
    };
    read_supervised(
        spawned.child,
        spawned.stdout,
        spawned.writer,
        spawned.deadline,
        Instant::now,
        decode,
        is_terminal,
        watch,
    )
}

/// A spawned adapter, its pipes and the deadline it is already running against.
///
/// Split out from [`read_supervised`] so the reading half can be driven over
/// any reader. Both halves are private: the seam exists so a stdout failure can
/// be injected deterministically in a unit test, not as API.
struct Spawned {
    child: Child,
    stdout: ChildStdout,
    writer: JoinHandle<()>,
    deadline: Instant,
}

/// A spawn the caller confirmed, or one it refused and that was killed before
/// it was given its prompt.
enum Launched {
    Running(Spawned),
    Refused {
        reason: String,
        surviving_processes: Option<String>,
    },
}

/// Spawn the adapter, start prompt delivery, and begin the deadline.
fn spawn_supervised(
    program: &Path,
    args: &[&str],
    request: &Request,
    environment: &ChildEnvironment,
    confirm: &mut dyn FnMut(u32) -> Result<(), String>,
) -> std::io::Result<Launched> {
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

    // The caller confirms the spawn before the process is given any work. A
    // refusal kills the group with the prompt unwritten; dropping the piped
    // stdin with the child closes it.
    if let Err(reason) = confirm(child.id()) {
        let surviving_processes = kill_tree(&mut child);
        return Ok(Launched::Refused {
            reason,
            surviving_processes,
        });
    }

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
    Ok(Launched::Running(Spawned {
        child,
        stdout,
        writer,
        deadline,
    }))
}

/// What the reader thread hands back, in order, ending with exactly one `Ended`.
///
/// The reader reports its own termination rather than letting the channel's
/// disconnection stand for it. Disconnection says the sender was dropped; it
/// does not say the pipe reached end of file, and the two were previously
/// indistinguishable to the caller.
enum Item<E> {
    Event(E),
    Malformed(StreamError),
    /// `None` is a clean end of file. `Some` is a read failure and its detail.
    Ended(Option<String>),
}

/// Read a spawned adapter's stream under its deadline, then clean up.
///
/// Generic over the reader so a stdout failure can be injected without touching
/// a live descriptor, and over the clock the deadline is compared against so an
/// event-order claim can be tested without racing it (spec 004 section 5,
/// 2026-09-22). Production passes [`Instant::now`] and the deadline computed at
/// the spawn, so the deadline's start point is the spawn. Everything else,
/// including child-exit observation, deadline enforcement and descendant
/// handling, is the same in every use.
#[allow(clippy::too_many_arguments)]
fn read_supervised<E: Send + 'static, R: Read + Send + 'static>(
    mut child: Child,
    stdout: R,
    writer: JoinHandle<()>,
    deadline: Instant,
    now: impl Fn() -> Instant,
    decode: fn(&str, usize) -> Result<E, StreamError>,
    is_terminal: fn(&E) -> bool,
    watch: &mut dyn Watch<E>,
) -> std::io::Result<Supervised<E>> {
    let (tx, rx) = mpsc::sync_channel(16);
    let reader = std::thread::spawn(move || {
        let mut stdout = BufReader::new(stdout);
        let mut failure = None;
        'read: {
            for (i, line) in (&mut stdout).lines().enumerate() {
                let line = match line {
                    Ok(line) => line,
                    Err(e) => {
                        // A read that failed is not a stream that ended. Say so
                        // here rather than letting the absence of a result event
                        // describe it later.
                        failure = Some(format!("while reading the event stream: {e}"));
                        break 'read;
                    }
                };
                if line.trim().is_empty() {
                    continue;
                }
                let event = decode(&line, i + 1);
                let stop_parsing = event.as_ref().map_or(true, is_terminal);
                let item = match event {
                    Ok(event) => Item::Event(event),
                    Err(e) => Item::Malformed(e),
                };
                if tx.send(item).is_err() {
                    return;
                }
                if stop_parsing {
                    break;
                }
            }
            // A terminal event or malformed line ends the trusted prefix. Drain
            // raw bytes after it, keeping the channel open until the pipe
            // closes. A failure here is still a failure to read the stream to
            // its end, so it is reported rather than discarded.
            if let Err(e) = std::io::copy(&mut stdout, &mut std::io::sink()) {
                failure = Some(format!(
                    "while draining output after the trusted prefix: {e}"
                ));
            }
        }
        let _ = tx.send(Item::Ended(failure));
    });

    let mut events = Vec::new();
    let mut stream_error = None;
    let mut read_failure: Option<String> = None;
    let mut timed_out = false;
    let mut ended = false;
    let mut stopped: Option<String> = None;

    loop {
        let remaining = deadline.saturating_duration_since(now());
        if remaining.is_zero() {
            timed_out = true;
            // What the reader had already delivered when the deadline fired
            // was read before the deadline, and dropping it with the channel
            // would lose exactly the evidence an interruption must keep: a
            // terminal denial that arrived in the last polling interval.
            if !ended {
                drain_delivered(&rx, &mut events, &mut stream_error);
            }
            break;
        }
        // Neither the end of the stream nor a terminal event says the child has
        // exited. Likewise, child exit does not close a pipe still held by a
        // descendant. Keep all of these observations under the same deadline,
        // without blocking on wait or join. The exit code is not a verdict on
        // the work.
        if ended && child.try_wait()?.is_some() && reader.is_finished() && writer.is_finished() {
            break;
        }
        let interval = remaining.min(Duration::from_millis(10));
        if ended {
            std::thread::sleep(interval);
            continue;
        }
        match rx.recv_timeout(interval) {
            Ok(Item::Event(event)) => {
                let control = watch.event(&event);
                events.push(event);
                if let Control::Stop(reason) = control {
                    // The caller decided at this event. What follows it in
                    // the pipe was not read, and is not waited for.
                    stopped = Some(reason);
                    break;
                }
            }
            Ok(Item::Malformed(e)) => {
                // Keep the diagnostic and preceding evidence through cleanup.
                // First malformed line wins: parsing stops there, so a second
                // would describe bytes the supervisor never trusted.
                if stream_error.is_none() {
                    stream_error = Some(e);
                }
            }
            Ok(Item::Ended(failure)) => {
                read_failure = failure;
                ended = true;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // The reader went away without reporting its own end. That is
                // not evidence of a clean end of file, so it is not recorded as
                // one.
                read_failure = Some(
                    "the stdout reader ended without reporting the end of the stream".to_string(),
                );
                ended = true;
            }
        }
    }

    let surviving = if timed_out || stopped.is_some() {
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

    // Diagnostic precedence, where more than one is observed: a malformed line
    // is a judgement about bytes that arrived and is the most specific, so it
    // is kept. A read failure is next: it establishes that the stream was never
    // read to its end. Only a stream that was read to its end, and carried no
    // result, is reported as having no result.
    let has_result = events.iter().any(is_terminal);
    if stream_error.is_none() {
        if let Some(detail) = read_failure {
            stream_error = Some(StreamError::ReadFailed {
                detail,
                events: events.len(),
            });
        } else if !has_result && !timed_out && stopped.is_none() {
            stream_error = Some(StreamError::NoResult {
                events: events.len(),
            });
        }
    }

    let outcome = if timed_out || stopped.is_some() {
        // Nothing was judged, so this is interrupted and not failed. A process
        // the caller stopped did not end by itself either.
        Outcome::Interrupted
    } else if stream_error.is_some() {
        // A stream that cannot be read is never a completion. It is also not a
        // judgement of the work, so it is interrupted rather than failed. A
        // read failure lands here with the rest: the provider's terminal claim
        // is retained in `events`, and stays separate from this observation.
        Outcome::Interrupted
    } else {
        Outcome::Completed
    };

    Ok(Supervised {
        events,
        outcome,
        stream_error,
        surviving_processes: surviving,
        stopped,
        timed_out,
    })
}

/// Absorb every item the reader had already delivered, without waiting.
///
/// Called only when the deadline has fired. It takes what is in the channel
/// and nothing more: an item still being read is not an item that was read
/// before the deadline, and waiting for it would be the supervisor being held
/// past its own deadline by the child.
fn drain_delivered<E>(
    rx: &mpsc::Receiver<Item<E>>,
    events: &mut Vec<E>,
    stream_error: &mut Option<StreamError>,
) {
    while let Ok(item) = rx.try_recv() {
        match item {
            Item::Event(event) => events.push(event),
            Item::Malformed(e) => {
                if stream_error.is_none() {
                    *stream_error = Some(e);
                }
            }
            Item::Ended(_) => break,
        }
    }
}

/// What a raw capture read, and how the process ended.
///
/// The bytes are kept exactly as they arrived, standard output and standard
/// error apart. Nothing is decoded here: a capture that is later judged has to
/// be judged from what the process wrote, not from a reading of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captured {
    /// Standard output, verbatim.
    pub stdout: Vec<u8>,
    /// Standard error, verbatim.
    pub stderr: Vec<u8>,
    /// The exit code, when the process exited by itself.
    pub code: Option<i32>,
    /// The signal that ended it, when one did.
    pub signal: Option<i32>,
    /// Whether the deadline ended it.
    pub timed_out: bool,
    /// Set when something in its process group outlived it.
    pub surviving_processes: Option<String>,
}

/// Spawn a program in its own process group, feed it `stdin`, and keep both
/// output streams verbatim, under a deadline.
///
/// The same process-group supervision [`supervise`] uses, for a caller that has
/// to keep the raw bytes rather than a decoded event stream. The deadline runs
/// from the spawn. At the deadline the whole group is killed. A child that
/// exits while a descendant is still in its group has the group killed too, and
/// the survivor is reported, because a descendant left running is not a
/// process that ended.
///
/// Readers are joined only once the child has exited and both pipes have
/// closed, or not at all: a reader blocked on a pipe a survivor holds would
/// otherwise hold the supervisor past its deadline.
pub fn capture(
    program: &Path,
    args: &[&str],
    workspace: &Path,
    environment: &std::collections::BTreeMap<String, String>,
    stdin: &[u8],
    deadline: Duration,
) -> std::io::Result<Captured> {
    let workspace = workspace_to_enter(workspace)?;
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(workspace)
        .env_clear()
        .envs(environment)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn()?;
    let deadline = Instant::now() + deadline;

    let mut input = child.stdin.take().expect("stdin was piped");
    let bytes = stdin.to_vec();
    let writer = std::thread::spawn(move || {
        use std::io::Write;
        let _ = input.write_all(&bytes);
    });
    let pipe = |mut from: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut out = Vec::new();
            let _ = from.read_to_end(&mut out);
            out
        })
    };
    let out = pipe(Box::new(child.stdout.take().expect("stdout was piped")));
    let err = pipe(Box::new(child.stderr.take().expect("stderr was piped")));

    let mut status = None;
    let mut timed_out = false;
    loop {
        if status.is_none() {
            status = child.try_wait()?;
        }
        if status.is_some() && out.is_finished() && err.is_finished() {
            break;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let surviving = if timed_out {
        let residual = kill_tree(&mut child);
        if status.is_none() {
            status = child.try_wait()?;
        }
        // The group is dead, so its pipes close and the readers finish with
        // what was written before the deadline, which is the evidence a
        // timeout keeps. Bounded: a writer that escaped the group is a
        // survivor, and it does not get to hold this function either.
        let settle = Instant::now() + Duration::from_secs(2);
        while !(out.is_finished() && err.is_finished()) && Instant::now() < settle {
            std::thread::sleep(Duration::from_millis(10));
        }
        residual
    } else {
        // The child is gone. Anything still answering in its group is a
        // descendant it left behind, which is killed and reported.
        group_survivor(&mut child)
    };

    let (stdout, stderr) = if out.is_finished() && err.is_finished() {
        let _ = writer.join();
        (
            out.join().unwrap_or_default(),
            err.join().unwrap_or_default(),
        )
    } else {
        // A survivor may hold a pipe. The readers are dropped rather than
        // joined, and what they had read is lost with them; the timeout and
        // the survivor say why.
        drop(writer);
        (Vec::new(), Vec::new())
    };

    let (code, signal) = match status {
        Some(s) => (s.code(), exit_signal(&s)),
        None => (None, None),
    };
    Ok(Captured {
        stdout,
        stderr,
        code,
        signal,
        timed_out,
        surviving_processes: surviving,
    })
}

#[cfg(unix)]
fn exit_signal(status: &std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn exit_signal(_: &std::process::ExitStatus) -> Option<i32> {
    None
}

/// After a child exited by itself, kill and report anything left in its group.
#[cfg(unix)]
fn group_survivor(child: &mut std::process::Child) -> Option<String> {
    let pid = child.id();
    let answers = Command::new("kill")
        .args(["-s", "0", "--", &format!("-{pid}")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !answers {
        return None;
    }
    let residual = kill_tree(child);
    Some(residual.unwrap_or_else(|| {
        format!(
            "process group {pid} still had members after its leader exited; they were \
             killed, and a process that left descendants running did not end cleanly"
        )
    }))
}

#[cfg(not(unix))]
fn group_survivor(_: &mut std::process::Child) -> Option<String> {
    None
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
    //
    // It is asked for a bounded time, not once. A descendant the kill ended is
    // a zombie until whoever inherited it reaps it, and signal 0 succeeds on a
    // zombie. Asked immediately, the probe raced that reaping and reported a
    // process the kill had ended as one that outlived it: measured on Linux CI
    // on 2026-09-22, where a fixture's foreground `sleep` was reparented and
    // not yet reaped. A process that survived `SIGKILL` still answers when the
    // bound runs out, and is reported.
    let answers = || {
        Command::new("kill")
            .args(["-s", "0", "--", &format!("-{pid}")])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };
    let settle = Instant::now() + Duration::from_secs(2);
    let mut still_there = answers();
    while still_there && Instant::now() < settle {
        std::thread::sleep(Duration::from_millis(20));
        still_there = answers();
    }

    if still_there {
        Some(format!(
            "process group {pid} still answers two seconds after SIGKILL; at least one \
             descendant outlived the kill, or is a zombie its reaper has not collected"
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// An event the reader delivered before the deadline fired is kept.
    ///
    /// Deterministic: the channel is filled before the drain runs, which is
    /// the state the deadline branch finds when the reader delivered in the
    /// last polling interval. Before the drain existed, those items were
    /// dropped with the channel.
    #[test]
    fn events_delivered_before_the_deadline_survive_it() {
        let (tx, rx) = mpsc::sync_channel(16);
        tx.send(Item::Event(1)).unwrap();
        tx.send(Item::Event(2)).unwrap();
        tx.send(Item::Malformed(StreamError::Malformed {
            line: 3,
            detail: "x".into(),
        }))
        .unwrap();
        tx.send(Item::Event(4)).unwrap();
        let mut events = Vec::new();
        let mut error = None;
        drain_delivered(&rx, &mut events, &mut error);
        assert_eq!(events, [1, 2, 4]);
        assert!(matches!(
            error,
            Some(StreamError::Malformed { line: 3, .. })
        ));
        // Nothing more was delivered, and the drain did not wait for more.
        drop(tx);
        drain_delivered(&rx, &mut events, &mut error);
        assert_eq!(events.len(), 3);
    }

    fn sh(script: &str, stdin: &[u8], deadline: Duration) -> (Captured, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let env: std::collections::BTreeMap<String, String> =
            [("PATH".to_string(), "/usr/bin:/bin".to_string())].into();
        let captured = capture(
            Path::new("/bin/sh"),
            &["-c", script],
            dir.path(),
            &env,
            stdin,
            deadline,
        )
        .unwrap();
        (captured, dir)
    }

    #[test]
    fn a_capture_keeps_both_streams_apart_and_verbatim() {
        let (c, _dir) = sh(
            "cat; printf 'to stderr\\377\\n' >&2; printf 'no newline'; exit 3",
            b"in\n",
            Duration::from_secs(30),
        );
        assert_eq!(c.stdout, b"in\nno newline");
        assert_eq!(c.stderr, b"to stderr\xff\n");
        assert_eq!((c.code, c.signal, c.timed_out), (Some(3), None, false));
        assert!(c.surviving_processes.is_none());
    }

    #[test]
    fn a_capture_says_which_signal_ended_the_process() {
        let (c, _dir) = sh("kill -s TERM $$", b"", Duration::from_secs(30));
        assert_eq!((c.code, c.signal, c.timed_out), (None, Some(15), false));
    }

    #[test]
    fn a_capture_kills_the_whole_group_at_the_deadline_and_keeps_what_was_written() {
        let started = Instant::now();
        let (c, dir) = sh(
            "sleep 300 & echo $! > descendant; echo before; exec sleep 300",
            b"",
            Duration::from_secs(1),
        );
        let elapsed = started.elapsed();
        assert!(c.timed_out);
        assert!(elapsed >= Duration::from_secs(1), "{elapsed:?}");
        // Held by its own child it would take 300 seconds; this separates that
        // defect from the latency of the `kill` it spawns.
        assert!(elapsed < Duration::from_secs(60), "{elapsed:?}");
        assert_eq!(c.stdout, b"before\n");
        assert!(
            c.surviving_processes.is_none(),
            "{:?}",
            c.surviving_processes
        );
        let pid = std::fs::read_to_string(dir.path().join("descendant")).unwrap();
        let alive = Command::new("kill")
            .args(["-s", "0", pid.trim()])
            .output()
            .unwrap()
            .status
            .success();
        assert!(!alive, "descendant {pid} outlived the deadline");
    }

    #[test]
    fn a_descendant_left_running_after_the_child_exits_is_killed_and_reported() {
        // The descendant closes the pipes, so nothing holds the capture open:
        // only the group check can see it.
        let (c, dir) = sh(
            "sleep 300 </dev/null >/dev/null 2>&1 & echo $! > descendant; exit 0",
            b"",
            Duration::from_secs(30),
        );
        assert!(!c.timed_out);
        assert_eq!(c.code, Some(0));
        assert!(c.surviving_processes.is_some());
        let pid = std::fs::read_to_string(dir.path().join("descendant")).unwrap();
        let alive = Command::new("kill")
            .args(["-s", "0", pid.trim()])
            .output()
            .unwrap()
            .status
            .success();
        assert!(!alive, "descendant {pid} was reported and left running");
    }
    use crate::capability::Requested;
    use crate::environment::{Blueprint, CheckSuiteCommands, construct};
    use crate::protocol::{AttemptIdentity, Classification, read_stream};
    use std::io::Write;

    /// A reader that passes `budget` bytes through and then fails every read.
    ///
    /// Deterministic fault injection: no pipe race, no descriptor manipulation,
    /// and the failure lands at a byte offset the test chooses. Sized to the
    /// trusted prefix, it fails exactly once that prefix has been delivered,
    /// which is the drain.
    struct FailAfter<R> {
        inner: R,
        budget: usize,
    }

    impl<R: Read> Read for FailAfter<R> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.budget == 0 {
                return Err(std::io::Error::other("injected stdout read failure"));
            }
            let cap = buf.len().min(self.budget);
            let read = self.inner.read(&mut buf[..cap])?;
            self.budget -= read;
            Ok(read)
        }
    }

    // A valid terminal event followed by a failure to drain the trailing bytes
    // is interrupted, and the terminal claim and the refusal survive it.
    #[test]
    fn a_drain_failure_after_a_terminal_event_is_not_a_completion() {
        let dir = tempfile::tempdir().unwrap();
        let stream = concat!(
            r#"{"event":"init","applied":[],"adapterVersion":"1.0","providerVersion":"fixture"}"#,
            "\n",
            r#"{"event":"refusal","guard":"fixture","detail":"blocked"}"#,
            "\n",
            r#"{"event":"result","classification":"completed","cost":{"amount":0.25,"unit":"fixture"}}"#,
            "\n",
        );
        std::fs::write(dir.path().join("stream.jsonl"), stream).unwrap();
        let script = dir.path().join("adapter.sh");
        let mut file = std::fs::File::create(&script).unwrap();
        file.write_all(b"cat > /dev/null\ncat stream.jsonl\n")
            .unwrap();
        drop(file);

        let request = Request {
            workspace: dir.path().to_path_buf(),
            base_commit: "0".repeat(40),
            prompt: b"fixture prompt".to_vec(),
            capabilities: Requested::none(),
            deadline_seconds: 30,
            attempt: AttemptIdentity {
                run_id: "drain-failure".into(),
                number: 1,
            },
        };
        let environment = construct(
            &Blueprint::empty().allowing("PATH", "/usr/bin:/bin"),
            &CheckSuiteCommands(vec![]),
        );
        let Launched::Running(spawned) = spawn_supervised(
            Path::new("/bin/sh"),
            &["adapter.sh"],
            &request,
            &environment,
            &mut |_| Ok(()),
        )
        .unwrap() else {
            panic!("an unrefused spawn runs");
        };
        let stdout = FailAfter {
            inner: spawned.stdout,
            budget: stream.len(),
        };
        let run: Supervised = read_supervised(
            spawned.child,
            stdout,
            spawned.writer,
            spawned.deadline,
            Instant::now,
            parse_event,
            |event| matches!(event, Event::Result { .. }),
            &mut Unwatched,
        )
        .unwrap();

        match &run.stream_error {
            Some(StreamError::ReadFailed { detail, events }) => {
                assert!(
                    detail.contains("while draining output after the trusted prefix"),
                    "the phase belongs in the diagnostic, got {detail}"
                );
                assert_eq!(*events, 3);
            }
            other => panic!("expected a read failure, got {other:?}"),
        }
        assert_eq!(run.outcome, Outcome::Interrupted, "never read as success");
        assert_eq!(run.events.len(), 3, "the trusted prefix is retained");
        assert!(matches!(run.events.first(), Some(Event::Init { .. })));
        assert_eq!(crate::protocol::refusals(&run.events).len(), 1);
        // The provider's own claim survives the supervisor's observation.
        let claimed = read_stream(&run.events, &[]).unwrap();
        assert_eq!(claimed.classification, Classification::Completed);
        assert_eq!(run.surviving_processes, None);
    }

    // ---- Event order and retention at the deadline, deterministically ----
    //
    // Spec 004 section 5, 2026-09-22. The real-process suite in
    // `tests/deadline.rs` can only assert what holds whether or not a child
    // ran. What the supervisor retains, and in which order, is decided here,
    // through the reading seam with a clock the test controls: the scripted
    // stream is delivered, and the deadline passes only once the reader has
    // asked for bytes beyond it, so every scripted line is already in the
    // channel when the deadline branch runs. The child is a real process in its
    // own group, so the kill is real and its death is checked by process id.

    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    const INIT: &str =
        r#"{"event":"init","applied":[],"adapterVersion":"1.0","providerVersion":"fixture"}"#;
    const REFUSAL: &str = r#"{"event":"refusal","guard":"fixture","detail":"blocked"}"#;
    const RESULT: &str = r#"{"event":"result","classification":"completed","cost":{"amount":0.25,"unit":"fixture"}}"#;

    /// What the scripted reader does once its bytes are delivered.
    enum Then {
        /// Blocks, as a pipe held open by a live writer does, until the test
        /// drops the sender.
        Hang(mpsc::Receiver<()>),
        /// End of file.
        Eof,
    }

    /// A stdout that delivers `bytes`, raises `beyond` the first time it is
    /// asked for more, and then hangs or ends.
    struct Scripted {
        bytes: Vec<u8>,
        offset: usize,
        then: Then,
        beyond: Arc<AtomicBool>,
    }

    impl Read for Scripted {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let rest = &self.bytes[self.offset..];
            let n = rest.len().min(buf.len());
            if n > 0 {
                buf[..n].copy_from_slice(&rest[..n]);
                self.offset += n;
                return Ok(n);
            }
            // Every complete line before this call has been decoded and sent:
            // the reader asks for more bytes only once its buffer is spent.
            self.beyond.store(true, Ordering::SeqCst);
            // Bounded, so a supervisor that wrongly joined this reader fails
            // its test by outcome rather than holding it forever.
            if let Then::Hang(rx) = &self.then {
                let _ = rx.recv_timeout(Duration::from_secs(60));
            }
            Ok(0)
        }
    }

    /// A clock frozen before the deadline until `beyond` is raised, and past it
    /// `polls` readings after that. `None` never passes it.
    fn clock(deadline: Instant, beyond: Option<(Arc<AtomicBool>, usize)>) -> impl Fn() -> Instant {
        let base = Instant::now();
        let seen = AtomicUsize::new(0);
        move || match &beyond {
            Some((flag, polls)) if flag.load(Ordering::SeqCst) => {
                if seen.fetch_add(1, Ordering::SeqCst) >= *polls {
                    deadline + Duration::from_millis(1)
                } else {
                    base
                }
            }
            _ => base,
        }
    }

    /// A real child in its own process group, and its descendant, both
    /// running before supervision starts. The marker is the synchronization
    /// that establishes the prerequisite; the wait for it is bounded.
    fn group(script: &str) -> (Child, tempfile::TempDir) {
        use std::os::unix::process::CommandExt;
        let dir = tempfile::tempdir().unwrap();
        let child = Command::new("/bin/sh")
            .args(["-c", script])
            .current_dir(dir.path())
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let started = Instant::now();
        while !dir.path().join("ready").exists() {
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "the fixture child never started"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        (child, dir)
    }

    fn dead(pid: &str) -> bool {
        !Command::new("kill")
            .args(["-s", "0", pid.trim()])
            .output()
            .unwrap()
            .status
            .success()
    }

    struct Seam {
        run: Supervised,
        child_pid: u32,
        dir: tempfile::TempDir,
    }

    /// Supervise `lines` through the seam, with `child_script` as the real
    /// child and the writer either finished or blocked.
    fn seam(
        lines: &[&str],
        then_hang: bool,
        child_script: &str,
        writer_blocked: bool,
        advance: Option<usize>,
    ) -> Seam {
        let (child, dir) = group(child_script);
        let child_pid = child.id();
        let beyond = Arc::new(AtomicBool::new(false));
        let mut bytes = lines.join("\n").into_bytes();
        bytes.push(b'\n');
        let (hold, held) = mpsc::channel::<()>();
        let stdout = Scripted {
            bytes,
            offset: 0,
            then: if then_hang {
                Then::Hang(held)
            } else {
                Then::Eof
            },
            beyond: beyond.clone(),
        };
        let (block, blocked) = mpsc::channel::<()>();
        let writer = std::thread::spawn(move || {
            // Bounded for the same reason as the scripted reader's hang.
            if writer_blocked {
                let _ = blocked.recv_timeout(Duration::from_secs(60));
            }
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        let run = read_supervised(
            child,
            stdout,
            writer,
            deadline,
            clock(deadline, advance.map(|polls| (beyond, polls))),
            parse_event,
            |event| matches!(event, Event::Result { .. }),
            &mut Unwatched,
        )
        .unwrap();
        drop(hold);
        drop(block);
        Seam {
            run,
            child_pid,
            dir,
        }
    }

    const HANGS: &str = "sleep 300 & echo $! > descendant; : > ready; exec sleep 300";
    const EXITS: &str = ": > ready; exit 0";

    #[test]
    fn a_terminal_event_then_a_hang_is_retained_whole_and_interrupted() {
        let s = seam(&[INIT, REFUSAL, RESULT], true, HANGS, false, Some(0));
        assert_eq!(s.run.outcome, Outcome::Interrupted);
        assert_eq!(s.run.events.len(), 3, "{:?}", s.run.events);
        assert!(matches!(s.run.events[0], Event::Init { .. }));
        assert_eq!(crate::protocol::refusals(&s.run.events).len(), 1);
        // The provider's claim is kept beside the supervisor's observation.
        let claimed = read_stream(&s.run.events, &[]).unwrap();
        assert_eq!(claimed.classification, Classification::Completed);
        assert_eq!(s.run.stream_error, None);
        assert_eq!(s.run.surviving_processes, None);
        assert!(
            dead(&s.child_pid.to_string()),
            "the child outlived the kill"
        );
        let descendant = std::fs::read_to_string(s.dir.path().join("descendant")).unwrap();
        assert!(
            dead(&descendant),
            "descendant {descendant} outlived the kill"
        );
    }

    #[test]
    fn a_malformed_line_keeps_the_events_before_it_and_its_diagnostic() {
        let s = seam(
            &[INIT, REFUSAL, "malformed", RESULT],
            true,
            HANGS,
            false,
            Some(0),
        );
        assert_eq!(s.run.outcome, Outcome::Interrupted);
        assert_eq!(s.run.events.len(), 2, "{:?}", s.run.events);
        assert!(matches!(
            s.run.stream_error,
            Some(StreamError::Malformed { line: 3, .. })
        ));
        assert_eq!(crate::protocol::refusals(&s.run.events).len(), 1);
        assert_eq!(s.run.surviving_processes, None);
    }

    #[test]
    fn end_of_file_with_a_live_child_holds_until_the_deadline_and_no_longer() {
        // The clock passes the deadline only several polls after end of file,
        // so a supervisor released by end of file alone would have returned
        // un-timed-out before then, and would not read as interrupted.
        let s = seam(&[INIT, REFUSAL], false, HANGS, false, Some(5));
        assert_eq!(s.run.outcome, Outcome::Interrupted);
        assert_eq!(s.run.events.len(), 2);
        assert_eq!(
            s.run.stream_error, None,
            "a timeout is not a missing result"
        );
        assert!(dead(&s.child_pid.to_string()));
        assert!(s.run.timed_out, "the deadline is what ended it");
    }

    #[test]
    fn a_blocked_prompt_writer_holds_until_the_deadline_and_no_longer() {
        // The child has exited and the stream has ended; only the writer is
        // still blocked. It must not release the supervisor early, and it must
        // not hold it past the deadline either: it is dropped, not joined.
        let s = seam(&[INIT, REFUSAL, RESULT], false, EXITS, true, Some(5));
        assert_eq!(s.run.outcome, Outcome::Interrupted);
        assert_eq!(s.run.events.len(), 3);
        assert_eq!(s.run.surviving_processes, None);
    }

    #[test]
    fn an_inherited_output_pipe_after_exit_is_interrupted_with_its_evidence() {
        // The child exited; its pipe is still held, so the stream never ends.
        let s = seam(&[INIT, REFUSAL, RESULT], true, EXITS, false, Some(0));
        assert_eq!(s.run.outcome, Outcome::Interrupted);
        assert_eq!(s.run.events.len(), 3);
        let claimed = read_stream(&s.run.events, &[]).unwrap();
        assert_eq!(claimed.classification, Classification::Completed);
    }

    #[test]
    fn a_normal_completion_is_completed_with_its_whole_stream() {
        // The clock never passes the deadline, so this cannot be a timeout.
        let s = seam(&[INIT, REFUSAL, RESULT], false, EXITS, false, None);
        assert_eq!(s.run.outcome, Outcome::Completed);
        assert_eq!(s.run.events.len(), 3);
        assert_eq!(s.run.stream_error, None);
        assert!(!s.run.timed_out);
    }

    #[test]
    fn output_after_the_terminal_event_is_drained_and_not_read() {
        let trailing = ["discard this trailing output"; 64];
        let mut lines = vec![INIT, RESULT];
        lines.extend(trailing);
        let s = seam(&lines, false, EXITS, false, None);
        assert_eq!(s.run.outcome, Outcome::Completed);
        assert_eq!(s.run.events.len(), 2, "the trusted prefix only");
    }

    #[test]
    fn an_exit_without_a_terminal_event_is_interrupted_with_no_result() {
        let s = seam(&[INIT, REFUSAL], false, EXITS, false, None);
        assert_eq!(s.run.outcome, Outcome::Interrupted);
        assert_eq!(
            s.run.stream_error,
            Some(StreamError::NoResult { events: 2 })
        );
        assert!(!s.run.timed_out, "an exit with no result is not a deadline");
    }

    // ---- The watch (spec 004 section 5, 2026-09-22) ----

    /// Records what the supervisor told it, and answers as configured.
    struct Recording {
        refuse_spawn: Option<String>,
        stop_at: Option<usize>,
        spawned: Vec<u32>,
        seen: usize,
        at_spawn: Option<Box<dyn FnMut()>>,
    }

    impl Watch<Event> for Recording {
        fn spawned(&mut self, pid: u32) -> Result<(), String> {
            self.spawned.push(pid);
            if let Some(check) = self.at_spawn.as_mut() {
                check();
            }
            match &self.refuse_spawn {
                Some(reason) => Err(reason.clone()),
                None => Ok(()),
            }
        }
        fn event(&mut self, _event: &Event) -> Control {
            self.seen += 1;
            match self.stop_at {
                Some(n) if n == self.seen => Control::Stop(format!("stopped at event {n}")),
                _ => Control::Continue,
            }
        }
    }

    fn watched(script: &str, watch: &mut Recording) -> (Supervised, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let run = watched_in(dir.path(), script, watch);
        (run, dir)
    }

    fn watched_in(dir: &Path, script: &str, watch: &mut Recording) -> Supervised {
        std::fs::write(dir.join("child.sh"), script).unwrap();
        let request = Request {
            workspace: dir.to_path_buf(),
            base_commit: "0".repeat(40),
            prompt: b"fixture prompt".to_vec(),
            capabilities: Requested::none(),
            deadline_seconds: 30,
            attempt: AttemptIdentity {
                run_id: "watched".into(),
                number: 1,
            },
        };
        let environment = construct(
            &Blueprint::empty().allowing("PATH", "/usr/bin:/bin"),
            &CheckSuiteCommands(vec![]),
        );
        supervise_watched(
            Path::new("/bin/sh"),
            &["child.sh"],
            &request,
            &environment,
            parse_event,
            |event| matches!(event, Event::Result { .. }),
            watch,
        )
        .unwrap()
    }

    fn recording() -> Recording {
        Recording {
            refuse_spawn: None,
            stop_at: None,
            spawned: Vec::new(),
            seen: 0,
            at_spawn: None,
        }
    }

    /// The prompt writer starts after the watch confirmed the spawn, not
    /// before: while the confirmation is held, the child has been given
    /// nothing, and once it is released the child gets the whole prompt.
    #[test]
    fn the_prompt_is_delivered_only_after_the_spawn_is_confirmed() {
        let dir = tempfile::tempdir().unwrap();
        let script = format!("cat > got-prompt\nprintf '%s\\n' '{INIT}' '{RESULT}'\n");
        let at_confirmation = Arc::new(std::sync::Mutex::new(None::<Vec<u8>>));
        let slot = at_confirmation.clone();
        let input = dir.path().join("got-prompt");
        let mut watch = recording();
        watch.at_spawn = Some(Box::new(move || {
            // Hold the confirmation long enough for a writer, had one
            // started, to have delivered the prompt. The redirection creates
            // the file before `cat` reads, so absent and empty both mean
            // nothing was delivered.
            std::thread::sleep(Duration::from_millis(300));
            *slot.lock().unwrap() = Some(std::fs::read(&input).unwrap_or_default());
        }));
        let run = watched_in(dir.path(), &script, &mut watch);
        assert_eq!(
            at_confirmation.lock().unwrap().as_deref(),
            Some(&b""[..]),
            "the child read its prompt before the spawn was confirmed"
        );
        assert_eq!(watch.spawned.len(), 1);
        assert_eq!(run.outcome, Outcome::Completed, "{run:?}");
        assert_eq!(run.stopped, None);
        assert_eq!(
            std::fs::read(dir.path().join("got-prompt")).unwrap(),
            b"fixture prompt"
        );
    }

    /// A refused confirmation kills the process with its prompt unwritten, and
    /// nothing after the refusal runs.
    #[test]
    fn a_refused_confirmation_kills_the_process_and_delivers_no_prompt() {
        let script = "echo $$ > pid\ncat > got-prompt\ntouch finished\n";
        let mut watch = recording();
        watch.refuse_spawn = Some("the confirmation could not be persisted".into());
        let (run, dir) = watched(script, &mut watch);
        assert_eq!(run.outcome, Outcome::Interrupted);
        assert_eq!(
            run.stopped.as_deref(),
            Some("the confirmation could not be persisted")
        );
        assert!(run.events.is_empty());
        assert_eq!(run.stream_error, None);
        let got = std::fs::read(dir.path().join("got-prompt")).unwrap_or_default();
        assert!(got.is_empty(), "a prompt reached a refused spawn: {got:?}");
        assert!(!dir.path().join("finished").exists());
        if let Ok(pid) = std::fs::read_to_string(dir.path().join("pid")) {
            assert!(dead(&pid), "the refused process is still running");
        }
    }

    /// A stop at an event kills the group before the child's next effect, keeps
    /// the events read up to and including that one, and reads no more.
    #[test]
    fn a_watch_that_stops_at_an_event_kills_the_group_before_the_next_effect() {
        let script = format!(
            "cat > /dev/null\necho $$ > pid\nprintf '%s\\n' '{INIT}'\n\
             while [ ! -f go ]; do sleep 0.05; done\ntouch effect\nprintf '%s\\n' '{RESULT}'\n"
        );
        let mut watch = recording();
        watch.stop_at = Some(1);
        let (run, dir) = watched(&script, &mut watch);
        assert_eq!(run.outcome, Outcome::Interrupted);
        assert_eq!(run.stopped.as_deref(), Some("stopped at event 1"));
        assert_eq!(run.events.len(), 1);
        assert!(matches!(run.events[0], Event::Init { .. }));
        assert_eq!(run.stream_error, None, "a stop is not a transport failure");
        // Release the child's next effect. A child still running would take
        // it within one poll.
        std::fs::write(dir.path().join("go"), "").unwrap();
        std::thread::sleep(Duration::from_millis(500));
        assert!(
            !dir.path().join("effect").exists(),
            "the effect ran after the stop"
        );
        let pid = std::fs::read_to_string(dir.path().join("pid")).unwrap();
        assert!(dead(&pid));
    }

    /// Without a watch, supervision is unchanged: the watch that is supplied
    /// by default confirms and never stops.
    #[test]
    fn an_unwatched_supervision_is_what_it_was() {
        let mut watch = recording();
        let script = format!("cat > /dev/null\nprintf '%s\\n' '{INIT}' '{REFUSAL}' '{RESULT}'\n");
        let (run, _dir) = watched(&script, &mut watch);
        assert_eq!(run.outcome, Outcome::Completed);
        assert_eq!(run.events.len(), 3);
        assert_eq!(watch.seen, 3);
        assert_eq!(run.stopped, None);
    }
}
