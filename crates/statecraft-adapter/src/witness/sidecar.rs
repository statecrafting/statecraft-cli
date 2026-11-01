//! One supervisor-owned sidecar process, spec 013 sections 3.2 to 3.4.
//!
//! [`start`] spawns the verified producer, writes the one `start` line, and
//! waits a bounded time for a bound `ready` that validates against the
//! reserved endpoint and the attempt's sidecar area. Only then may the child
//! be spawned. [`Running::close`] ends custody by closing the sidecar's stdin
//! after the child has ended, reads to the terminal message within a grace
//! period, and kills a sidecar that outlives it.
//!
//! Stdout is read on its own thread through the bounded [`Reader`]; stderr is
//! discarded, because diagnostics are not protocol evidence and this crate
//! does not redact them. The process handle is retained for supervision from
//! spawn to exit, and a child can neither request nor replace it.
//!
//! The wait for `ready` runs two deadlines in sequence (spec 043 section
//! 3.1): a start deadline from spawn until the producer's own code is
//! observed running, and the ready deadline from that observation. The
//! observation (section 3.3) is that the producer has consumed the start
//! line: the supervisor keeps a duplicate of the read end of the pipe it wrote
//! the line to and asks the kernel how many bytes remain unread. Nothing the
//! operating system does before the producer runs, the confinement wrapper
//! and the loader included, reads that pipe, and the producer sends nothing
//! the supervisor would have to trust. Where the kernel cannot answer, 013's
//! single deadline from spawn stands.

use crate::witness::ledger::VerifiedProducer;
use crate::witness::posture::Observed;
use crate::witness::protocol::{
    Completion, Finding, Integrity, MAX_LINE_BYTES, MAX_MESSAGES, Notice, Reader, ReadyRefusal,
    Start, Termination, ValidatedReady,
};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, PipeReader, PipeWriter, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// The closing reason of a sidecar whose code was not observed running
/// before the start deadline (spec 043 section 3.2).
pub const START_TIMEOUT: &str = "start-timeout";
/// The closing reason of a started sidecar that reported no valid `ready`
/// before the ready deadline.
pub const READY_TIMEOUT: &str = "ready-timeout";

/// How often the start observation is re-read while the start deadline runs.
const START_POLL: Duration = Duration::from_millis(10);

/// Everything needed to start one sidecar.
#[derive(Debug, Clone)]
pub struct Plan {
    /// The verified executable; the only program started.
    pub producer: VerifiedProducer,
    /// The one `start` message.
    pub start: Start,
    /// The confinement prefix the executable runs under, argv form. Empty
    /// means none was applied, which the caller records.
    pub wrapper: Vec<OsString>,
    /// The sidecar's whole environment; nothing is inherited.
    pub environment: BTreeMap<String, String>,
    /// The start deadline: how long after spawn the producer's code must be
    /// observed running (spec 043 section 3.1). Longer than `ready_timeout`.
    pub start_timeout: Duration,
    /// The ready deadline: how long the started producer has to report a
    /// valid `ready`, from the start observation.
    pub ready_timeout: Duration,
    /// How long the sidecar has to finish after its stdin closes.
    pub grace: Duration,
}

/// How the supervisor observed the producer's start (spec 043 section 3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartObservation {
    /// The producer read from the pipe holding the start line.
    StartLineConsumed,
    /// The kernel could not report the pipe's unread bytes, so no observation
    /// was available and 013's single deadline ran from spawn.
    Unavailable,
}

/// When the sidecar started and reported ready, for spec 043 section 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// Which observation applied.
    pub observation: StartObservation,
    /// Spawn to the start observation; `None` when it never came.
    pub started_after: Option<Duration>,
    /// The start observation (spawn, when none was available) to the
    /// validated `ready`; `None` when none came.
    pub ready_after: Option<Duration>,
}

/// Which deadline is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Starting,
    Started,
}

enum Line {
    Bytes(Vec<u8>),
    Eof,
}

/// A sidecar that reported a valid `ready`.
#[derive(Debug)]
pub struct Running {
    child: Child,
    stdin: Option<PipeWriter>,
    lines: Receiver<Line>,
    reader: Reader,
    ready: ValidatedReady,
    area: PathBuf,
    eof: bool,
    grace: Duration,
    timing: Timing,
}

/// What one sidecar left behind, whether it reached `ready` or not.
#[derive(Debug, Clone)]
pub struct Ended {
    /// Everything read, findings included.
    pub reader: Reader,
    /// How the process ended.
    pub termination: Termination,
    /// The manifest's integrity, when a terminal message names one.
    pub integrity: Integrity,
    /// Why the child must not be spawned, when it was refused before.
    pub refusal: Option<String>,
    /// When the producer started and reported ready.
    pub timing: Timing,
}

impl Ended {
    /// What was observed, for [`crate::witness::posture::derive`].
    ///
    /// Any protocol finding, a missing terminal message, or a termination
    /// other than `finished` is a failure: testimony from a sidecar that
    /// broke its protocol is not complete capture.
    #[must_use]
    pub fn observed(&self, qualified: bool) -> Observed {
        let failed = !self.reader.findings().is_empty()
            || self.termination != Termination::Finished
            || self.refusal.is_some();
        match self.reader.terminal() {
            Some(_) if failed => Observed::Failed,
            None => Observed::Failed,
            Some(t) => match t.completion {
                Completion::Complete => Observed::Complete {
                    intact: self.integrity == Integrity::Intact,
                    qualified,
                },
                Completion::Incomplete | Completion::Absent | Completion::Unknown => {
                    Observed::Incomplete
                }
            },
        }
    }
}

/// Why no sidecar process could be started at all.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("wire-witness sidecar could not be spawned: {0}")]
pub struct SpawnFailed(pub String);

/// Start the sidecar and wait for a valid `ready`.
///
/// The effect intent must already be durable ([`crate::witness::effect::open`]).
///
/// # Errors
///
/// `Err(Ok(SpawnFailed))` when no process started, which is
/// `witness-unavailable`; `Err(Err(Ended))` when a started sidecar failed or
/// its `ready` refuses child spawn. Either way the child must not be spawned.
pub fn start(plan: &Plan) -> Result<Running, Result<SpawnFailed, Box<Ended>>> {
    let argv = argv(plan);
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]);
    spawn(plan, command)
}

/// [`start`] under the sidecar's own confinement (spec 013 section 3.6.2),
/// admitted by [`crate::boundary::Prepared::prepare_sidecar`] and self-tested
/// by the caller immediately before. The wrapper, if any, runs inside it.
///
/// # Errors
///
/// As [`start`].
pub fn start_confined(
    plan: &Plan,
    boundary: &crate::boundary::Prepared,
) -> Result<Running, Result<SpawnFailed, Box<Ended>>> {
    if boundary.record().sidecar_outbound_ports.is_none()
        || boundary.record().witness_endpoint != Some(plan.start.listen_endpoint)
    {
        return Err(Ok(SpawnFailed(
            "the boundary is not this sidecar's own confinement".into(),
        )));
    }
    let argv = argv(plan);
    let args: Vec<&str> = argv[1..].iter().filter_map(|a| a.to_str()).collect();
    if args.len() + 1 != argv.len() {
        return Err(Ok(SpawnFailed("a non-UTF-8 argument".into())));
    }
    spawn(
        plan,
        boundary.command(std::path::Path::new(&argv[0]), &args),
    )
}

fn argv(plan: &Plan) -> Vec<OsString> {
    let mut argv = plan.wrapper.clone();
    argv.push(plan.producer.path.clone().into_os_string());
    argv.push("sidecar".into());
    argv
}

fn spawn(plan: &Plan, mut command: Command) -> Result<Running, Result<SpawnFailed, Box<Ended>>> {
    let area = plan.start.output_directory.clone();
    // The supervisor makes the stdin pipe itself and keeps a duplicate of
    // its read end, which is the start observation's whole instrument. Both
    // ends are close-on-exec; the child receives the read end as its stdin
    // only, and an extra read end never withholds EOF: that follows the last
    // write end, which `Running::close` drops.
    let (reader, mut stdin) = std::io::pipe().map_err(|e| Ok(SpawnFailed(e.to_string())))?;
    let probe = reader
        .try_clone()
        .map_err(|e| Ok(SpawnFailed(e.to_string())))?;
    command
        .env_clear()
        .envs(&plan.environment)
        .stdin(Stdio::from(reader))
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|e| Ok(SpawnFailed(e.to_string())))?;
    let spawned = Instant::now();
    drop(command);
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(Ok(SpawnFailed("stdio unavailable".into())));
    };
    let lines = read_lines(stdout);
    let mut running = Running {
        child,
        stdin: None,
        lines,
        reader: Reader::new(plan.start.binding.clone()),
        ready: ValidatedReady {
            endpoint: plan.start.listen_endpoint,
            certificate_path: plan.start.certificate_path.clone(),
        },
        area,
        eof: false,
        grace: plan.grace,
        timing: Timing {
            observation: StartObservation::StartLineConsumed,
            started_after: None,
            ready_after: None,
        },
    };
    let line = plan.start.line();
    if stdin.write_all(&line).and_then(|()| stdin.flush()).is_err() {
        return Err(Err(Box::new(running.abort("start-not-delivered"))));
    }
    running.stdin = Some(stdin);

    // Section 3.1: the start deadline runs until the observation, then the
    // ready deadline runs from it. With no observation available, the ready
    // deadline runs from spawn (section 3.3, last paragraph).
    let mut phase = Phase::Starting;
    let mut deadline = spawned + plan.start_timeout;
    let mut observed_at = spawned;
    let mut probe = Some(probe);
    if unread(probe.as_ref()).is_none() {
        running.timing.observation = StartObservation::Unavailable;
        probe = None;
        phase = Phase::Started;
        deadline = spawned + plan.ready_timeout;
    }
    loop {
        let now = Instant::now();
        if phase == Phase::Starting {
            match unread(probe.as_ref()) {
                Some(remaining) if remaining < line.len() => {
                    observed_at = now;
                    running.timing.started_after = Some(now - spawned);
                    probe = None;
                    phase = Phase::Started;
                    deadline = now + plan.ready_timeout;
                }
                Some(_) => {}
                None => {
                    running.timing.observation = StartObservation::Unavailable;
                    probe = None;
                    phase = Phase::Started;
                    deadline = spawned + plan.ready_timeout;
                }
            }
        }
        let mut wait = deadline.saturating_duration_since(now);
        if phase == Phase::Starting {
            wait = wait.min(START_POLL);
        }
        match running.lines.recv_timeout(wait) {
            Ok(Line::Bytes(bytes)) => match running.reader.read(&bytes) {
                Ok(Notice::Ready(ready)) => {
                    return match ready.validate(
                        plan.start.listen_endpoint,
                        &running.area,
                        &plan.start.certificate_path,
                    ) {
                        Ok(validated) => {
                            running.ready = validated;
                            running.timing.ready_after = Some(observed_at.elapsed());
                            Ok(running)
                        }
                        Err(refusal) => Err(Err(Box::new(running.refuse(&refusal)))),
                    };
                }
                Ok(_) => {}
                Err(finding) if finding.is_custody_violation() => {
                    return Err(Err(Box::new(running.abort("custody-violation"))));
                }
                Err(_) => {}
            },
            Ok(Line::Eof) | Err(RecvTimeoutError::Disconnected) => {
                running.eof = true;
                return Err(Err(Box::new(running.abort("sidecar-exited-before-ready"))));
            }
            Err(RecvTimeoutError::Timeout) if Instant::now() < deadline => {}
            Err(RecvTimeoutError::Timeout) => {
                let why = match phase {
                    Phase::Starting => START_TIMEOUT,
                    Phase::Started => READY_TIMEOUT,
                };
                return Err(Err(Box::new(running.abort(why))));
            }
        }
    }
}

/// How many bytes written to the pipe `probe` reads from are still unread,
/// or `None` where the kernel does not answer. The query reads the pipe's
/// state only; it consumes nothing and the producer takes no part in it.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(unsafe_code)]
fn unread(probe: Option<&PipeReader>) -> Option<usize> {
    use std::os::fd::AsRawFd;
    let probe = probe?;
    let mut count: libc::c_int = 0;
    // SAFETY: FIONREAD writes one `c_int` through the pointer it is given,
    // and `probe` owns the descriptor for the duration of the call.
    let rc = unsafe { libc::ioctl(probe.as_raw_fd(), libc::FIONREAD, &raw mut count) };
    (rc == 0).then(|| usize::try_from(count).unwrap_or(0))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn unread(_probe: Option<&PipeReader>) -> Option<usize> {
    None
}

impl Running {
    /// The one endpoint and certificate the child receives.
    #[must_use]
    pub fn ready(&self) -> &ValidatedReady {
        &self.ready
    }

    /// When the producer started and reported ready.
    #[must_use]
    pub fn timing(&self) -> Timing {
        self.timing
    }

    /// Read whatever the sidecar has sent without blocking. Returns the
    /// custody violation if one arrived: the caller must stop the child and
    /// then [`Running::abort`] the sidecar.
    pub fn poll(&mut self) -> Option<Finding> {
        while let Ok(line) = self.lines.try_recv() {
            match line {
                Line::Bytes(bytes) => {
                    if let Err(finding) = self.reader.read(&bytes)
                        && finding.is_custody_violation()
                    {
                        return Some(finding);
                    }
                }
                Line::Eof => self.eof = true,
            }
        }
        None
    }

    /// End custody after the child has ended: close stdin, read to the
    /// terminal message within the grace period, and kill a sidecar that
    /// outlives it.
    #[must_use]
    pub fn close(mut self) -> Ended {
        drop(self.stdin.take());
        let deadline = Instant::now() + self.grace;
        while !self.eof {
            let wait = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(wait) {
                Ok(Line::Bytes(bytes)) => {
                    let _ = self.reader.read(&bytes);
                }
                Ok(Line::Eof) | Err(RecvTimeoutError::Disconnected) => self.eof = true,
                Err(RecvTimeoutError::Timeout) => break,
            }
        }
        let termination = self.reap(deadline);
        let integrity = self
            .reader
            .terminal()
            .map_or(Integrity::Missing, |t| t.integrity(&self.area));
        Ended {
            reader: self.reader,
            termination,
            integrity,
            refusal: None,
            timing: self.timing,
        }
    }

    /// Stop the sidecar now, for a custody violation or a refused start.
    #[must_use]
    pub fn abort(mut self, why: &str) -> Ended {
        drop(self.stdin.take());
        let _ = self.child.kill();
        let termination = match self.child.wait() {
            Ok(status) if status.code().is_some() => Termination::Failed,
            Ok(_) => Termination::Killed,
            Err(_) => Termination::Unknown,
        };
        self.poll();
        Ended {
            reader: self.reader,
            termination,
            integrity: Integrity::Missing,
            refusal: Some(why.to_string()),
            timing: self.timing,
        }
    }

    fn refuse(self, refusal: &ReadyRefusal) -> Ended {
        self.abort(&refusal.to_string())
    }

    /// Wait for the process, whose stdin this supervisor has closed, until
    /// `deadline`, then kill it.
    fn reap(&mut self, deadline: Instant) -> Termination {
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    return match (status.success(), self.reader.terminal().is_some()) {
                        (true, true) => Termination::Finished,
                        // Exited by itself after stdin closed, with no terminal
                        // message: EOF before `finished` is `sidecar-stopped`.
                        (_, false) if status.code().is_some() => Termination::Stopped,
                        _ => Termination::Failed,
                    };
                }
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Ok(None) => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return Termination::Killed;
                }
                Err(_) => return Termination::Unknown,
            }
        }
    }
}

/// Read stdout one bounded line at a time on its own thread. It hands over
/// one line more than [`MAX_MESSAGES`], so the reader records
/// `too-many-messages` for a flooding sidecar, then stops and closes the pipe.
fn read_lines<R: Read + Send + 'static>(stdout: R) -> Receiver<Line> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut stdout = BufReader::new(stdout);
        for _ in 0..=MAX_MESSAGES {
            let mut line = Vec::new();
            let limit = (MAX_LINE_BYTES + 1) as u64;
            match (&mut stdout).take(limit).read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if !line.ends_with(b"\n") && line.len() as u64 == limit {
                        // Oversized: hand the reader its finding, then skip
                        // to the end of the line without retaining it.
                        let _ = skip_line(&mut stdout);
                    }
                    if sender.send(Line::Bytes(line)).is_err() {
                        return;
                    }
                }
            }
        }
        let _ = sender.send(Line::Eof);
    });
    receiver
}

fn skip_line<R: BufRead>(stdout: &mut R) -> std::io::Result<()> {
    loop {
        let buffer = stdout.fill_buf()?;
        if buffer.is_empty() {
            return Ok(());
        }
        if let Some(at) = buffer.iter().position(|b| *b == b'\n') {
            stdout.consume(at + 1);
            return Ok(());
        }
        let len = buffer.len();
        stdout.consume(len);
    }
}
