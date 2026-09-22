//! Spec 004 sections 3.5 and 3.8: deadline enforcement and cleanup, through a
//! real child process.
//!
//! Spec 004 section 5, 2026-09-22, fixes what this file may assert. The
//! product's deadline runs from the spawn; a hung attempt is never ended before
//! it; when it fires the supervisor keeps what was already delivered, kills the
//! process group and returns without being held by anything the child left.
//! It does **not** promise that a child is `execve`d, runs or is read inside
//! any bound, and on a loaded machine one is not. So:
//!
//! - The six **hanging** fixtures assert only what holds whether or not the
//!   child ever ran: supervision returned no earlier than the one-second
//!   deadline and was not held by the child's 300-second hang, the outcome is
//!   `interrupted`, nothing survived, and whatever was retained is, in order,
//!   a prefix of what the child's own trace says it emitted.
//! - The four **completing** fixtures are not about the deadline, so each names
//!   a sixty-second one as a watchdog and keeps its exact assertions.
//! - Which events survive an interruption, and in which order, is established
//!   deterministically in the supervisor's unit tests, where the clock is the
//!   test's rather than the scheduler's.
#![cfg(unix)]

use statecraft_adapter::capability::Requested;
use statecraft_adapter::environment::{Blueprint, CheckSuiteCommands, construct};
use statecraft_adapter::protocol::{
    AttemptIdentity, Classification, Event, Request, StreamError, read_stream, refusals,
};
use statecraft_adapter::supervisor::supervise;
use statecraft_run::attempt::Outcome;
use statecraft_run::refusal::{Accounting, decide};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// The deadline where the deadline is the measurement.
const SHORT: u64 = 1;
/// The deadline where it is not: a watchdog, never a claim.
const WATCHDOG: u64 = 60;
/// The bound that separates "not held by the child's 300-second hang" from the
/// latency of the `kill` the supervisor spawns. A statement that supervision
/// was not held, never a measure of promptness.
const NOT_HELD: Duration = Duration::from_secs(60);
/// How long the outer process waits for a worker before killing it. Longer
/// than every inner bound, so it only ever fires on a supervisor that did not
/// return, and it proves nothing about the product.
const OUTER: Duration = Duration::from_secs(120);

// The outer test process recovers even if supervise blocks forever. Each
// fixture writes its process-group identity before doing anything that hangs.
// Output goes to a file, so the outer harness never waits for a pipe's EOF.
// Cleanup runs on drop, which is before any assertion and also on a panic.
struct Cleanup<'a> {
    worker: Child,
    dir: &'a Path,
}

impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        if let Ok(pid) = std::fs::read_to_string(self.dir.join("group")) {
            let pid: u32 = pid.trim().parse().expect("fixture pid");
            let _ = Command::new("kill")
                .args(["-s", "KILL", "--", &format!("-{pid}")])
                .output();
        }
        if let Ok(pid) = std::fs::read_to_string(self.dir.join("descendant")) {
            let _ = Command::new("kill")
                .args(["-s", "KILL", pid.trim()])
                .output();
        }
        let _ = self.worker.kill();
        let _ = self.worker.wait();
    }
}

fn bounded(case: &str) {
    let dir = tempfile::tempdir().unwrap();
    let output = std::fs::File::create(dir.path().join("output")).unwrap();
    let worker = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "deadline_worker", "--nocapture"])
        .env("DEADLINE_TEST_CASE", case)
        .env("DEADLINE_TEST_DIR", dir.path())
        .stdin(Stdio::null())
        .stdout(output.try_clone().unwrap())
        .stderr(output)
        .spawn()
        .unwrap();
    let mut cleanup = Cleanup {
        worker,
        dir: dir.path(),
    };
    let started = Instant::now();
    let status = loop {
        if let Some(status) = cleanup.worker.try_wait().unwrap() {
            break Some(status);
        }
        if started.elapsed() >= OUTER {
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    // Cleanup precedes every assertion, including the watchdog's.
    drop(cleanup);
    let output = std::fs::read_to_string(dir.path().join("output")).unwrap();
    let trace = std::fs::read_to_string(dir.path().join("trace")).unwrap_or_default();
    assert!(
        status.is_some_and(|status| status.success()),
        "{case}: worker status {status:?} after {:?}; the outer watchdog is {OUTER:?}\n\
         child trace:\n{trace}\nworker output:\n{output}",
        started.elapsed()
    );
}

// ---- Hanging children: deadline and cleanup, unconditionally ----

#[test]
fn terminal_then_hang() {
    bounded("terminal-hang");
}

#[test]
fn malformed_then_hang() {
    bounded("malformed-hang");
}

#[test]
fn terminal_then_exit_with_inherited_pipe() {
    bounded("inherited-pipe");
}

#[test]
fn eof_while_child_remains_alive() {
    bounded("eof-hang");
}

#[test]
fn blocked_prompt_delivery() {
    bounded("unread-prompt");
}

#[test]
fn terminal_then_exit_with_inherited_input_pipe() {
    bounded("inherited-input");
}

// A child that has not reached its first event when the deadline fires. The
// assertions above must hold for it too, which is what makes them independent
// of scheduling; before 2026-09-22 this shape failed the suite with the
// supervisor behaving correctly.
#[test]
fn a_child_that_has_not_started_by_the_deadline() {
    bounded("late-start");
}

// ---- Completing children: exact assertions, a watchdog deadline ----

// A line that is not valid UTF-8 makes `lines()` fail. That is a read failure,
// not a stream that ended, and it arrives through a real child rather than
// through a seam.
#[test]
fn unreadable_stdout_before_a_terminal_event() {
    bounded("unreadable");
}

#[test]
fn normal_successful_child() {
    bounded("success");
}

#[test]
fn terminal_output_is_drained_without_reinterpreting_it() {
    bounded("drain");
}

#[test]
fn successful_exit_without_result_is_interrupted() {
    bounded("no-result");
}

/// What a fixture does once it has emitted its stream.
struct Fixture {
    /// The lines it emits, in order, each followed by a trace entry.
    lines: Vec<&'static str>,
    /// Shell run after them.
    tail: String,
    /// Whether it reads its prompt first.
    consumes_prompt: bool,
    /// Shell run before anything else, after the group marker.
    before: &'static str,
    /// Whether the deadline is what the case measures.
    hangs: bool,
}

const INIT: &str =
    r#"{"event":"init","applied":[],"adapterVersion":"1.0","providerVersion":"fixture"}"#;
const REFUSAL: &str = r#"{"event":"refusal","guard":"fixture","detail":"blocked"}"#;
const RESULT: &str =
    r#"{"event":"result","classification":"completed","cost":{"amount":0.25,"unit":"fixture"}}"#;

fn fixture(case: &str) -> Fixture {
    let hang = |lines: Vec<&'static str>, tail: &str, consumes_prompt: bool| Fixture {
        lines,
        tail: tail.into(),
        consumes_prompt,
        before: "",
        hangs: true,
    };
    let complete = |lines: Vec<&'static str>, tail: &str| Fixture {
        lines,
        tail: tail.into(),
        consumes_prompt: true,
        before: "",
        hangs: false,
    };
    match case {
        "terminal-hang" => hang(vec![INIT, REFUSAL, RESULT], "exec sleep 300", true),
        "malformed-hang" => hang(
            vec![INIT, REFUSAL, "malformed", RESULT],
            "exec sleep 300",
            true,
        ),
        "inherited-pipe" => hang(
            vec![INIT, REFUSAL, RESULT],
            "sleep 300 &\necho $! > descendant\nexit 0",
            true,
        ),
        "eof-hang" => hang(vec![INIT, REFUSAL], "exec 1>&-\nexec sleep 300", true),
        "unread-prompt" => hang(vec![INIT, REFUSAL], "exec sleep 300", false),
        // dash replaces an asynchronous command's stdin with /dev/null before
        // applying <&0. Save the pipe before backgrounding so the descendant
        // holds it open without reading on both dash and sh.
        "inherited-input" => hang(
            vec![INIT, REFUSAL, RESULT],
            "exec 3<&0\nsleep 300 <&3 3<&- >/dev/null &\necho $! > descendant\nexit 0",
            false,
        ),
        "late-start" => Fixture {
            before: "sleep 3",
            ..hang(vec![INIT, REFUSAL, RESULT], "exec sleep 300", true)
        },
        // Raw bytes that are not valid UTF-8, so reading the line fails.
        "unreadable" => complete(vec![INIT, REFUSAL], "printf 'x\\377\\376y\\n'\nexit 0"),
        "success" => complete(vec![INIT, RESULT], "exit 0"),
        "drain" => complete(
            vec![INIT, REFUSAL, RESULT],
            "i=0\nwhile [ $i -lt 10000 ]; do echo 'discard this trailing output'; i=$((i+1)); done",
        ),
        "no-result" => complete(vec![INIT, REFUSAL], "exit 0"),
        _ => panic!("unknown fixture {case}"),
    }
}

/// The events the child emitted, in order, up to the end of the trusted
/// prefix: a terminal event ends it, and so does a malformed line.
fn trusted_prefix(lines: &[&str], emitted: usize) -> Vec<&'static str> {
    let mut out = Vec::new();
    for line in lines.iter().take(emitted) {
        match *line {
            l if l == INIT => out.push(INIT),
            l if l == REFUSAL => out.push(REFUSAL),
            l if l == RESULT => {
                out.push(RESULT);
                break;
            }
            _ => break,
        }
    }
    out
}

fn matches_line(event: &Event, line: &str) -> bool {
    match line {
        l if l == INIT => matches!(event, Event::Init { .. }),
        l if l == REFUSAL => matches!(event, Event::Refusal { .. }),
        l if l == RESULT => matches!(event, Event::Result { .. }),
        _ => false,
    }
}

fn dead(pid: &str) -> bool {
    let output = Command::new("ps")
        .args(["-o", "stat=", "-p", pid.trim()])
        .output()
        .unwrap();
    let stat = String::from_utf8(output.stdout).unwrap();
    // A zombie has stopped executing and cannot hold a pipe open.
    stat.trim().is_empty() || stat.trim().starts_with('Z')
}

// Re-entered only by bounded(), in a disposable process rather than a thread
// the broken supervisor could hold indefinitely. No provider is involved.
#[test]
fn deadline_worker() {
    let Ok(case) = std::env::var("DEADLINE_TEST_CASE") else {
        return;
    };
    let dir = std::env::var_os("DEADLINE_TEST_DIR").unwrap();
    let dir = Path::new(&dir);
    let f = fixture(&case);

    // The trace is the child's own account of how far it got, written after
    // each line reached its stdout, so a failure carries its evidence and a
    // retained event can be checked against a line the child actually wrote.
    let mut script = format!("echo $$ > group\n{}\n", f.before);
    if f.consumes_prompt {
        script.push_str("cat > /dev/null\n");
    }
    for (i, line) in f.lines.iter().enumerate() {
        script.push_str(&format!("echo '{line}'\necho {} >> trace\n", i + 1));
    }
    script.push_str(&f.tail);
    script.push('\n');
    std::fs::write(dir.join("adapter.sh"), script).unwrap();

    let deadline_seconds = if f.hangs { SHORT } else { WATCHDOG };
    let request = Request {
        workspace: dir.to_path_buf(),
        base_commit: "0".repeat(40),
        prompt: if f.consumes_prompt {
            b"fixture prompt".to_vec()
        } else {
            vec![b'x'; 1024 * 1024]
        },
        capabilities: Requested::none(),
        deadline_seconds,
        attempt: AttemptIdentity {
            run_id: "deadline-fixture".into(),
            number: 1,
        },
    };
    let environment = construct(
        &Blueprint::empty().allowing("PATH", "/usr/bin:/bin"),
        &CheckSuiteCommands(vec![]),
    );
    let started = Instant::now();
    let run = supervise(
        Path::new("/bin/sh"),
        &["adapter.sh"],
        &request,
        &environment,
    )
    .unwrap();
    let elapsed = started.elapsed();
    let emitted = std::fs::read_to_string(dir.join("trace"))
        .unwrap_or_default()
        .lines()
        .count();

    // Every retained event, in order, is one the child wrote, inside the
    // trusted prefix. This holds whether or not the child ran.
    let prefix = trusted_prefix(&f.lines, emitted);
    assert!(
        run.events.len() <= prefix.len(),
        "{case}: retained {} events and the child wrote a prefix of {}",
        run.events.len(),
        prefix.len()
    );
    for (event, line) in run.events.iter().zip(&prefix) {
        assert!(matches_line(event, line), "{case}: {event:?} is not {line}");
    }

    if f.hangs {
        assert!(
            elapsed >= Duration::from_secs(SHORT),
            "{case}: supervision ended before its deadline, after {elapsed:?}"
        );
        assert!(
            elapsed < NOT_HELD,
            "{case}: supervision was held by the child, {elapsed:?}"
        );
        assert_eq!(run.outcome, Outcome::Interrupted, "{case}");
        assert_eq!(run.surviving_processes, None, "{case}");
        match &run.stream_error {
            None => {}
            // Only a child that reached its malformed line can report it.
            Some(StreamError::Malformed { line: 3, .. })
                if case == "malformed-hang" && emitted >= 3 => {}
            other => panic!("{case}: a timeout reported {other:?}"),
        }
        for marker in ["group", "descendant"] {
            if let Ok(pid) = std::fs::read_to_string(dir.join(marker)) {
                assert!(dead(&pid), "{case}: surviving {marker} {pid}");
            }
        }
        return;
    }

    // A completing child: the watchdog did not fire, and every assertion is
    // exact because the deadline was not what it measured.
    assert!(
        elapsed < Duration::from_secs(WATCHDOG),
        "{case}: the watchdog deadline fired after {elapsed:?}, so the fixture did not \
         complete and nothing below is about the product; trace has {emitted} line(s)"
    );
    let completed = case == "success" || case == "drain";
    assert_eq!(
        run.outcome,
        if completed {
            Outcome::Completed
        } else {
            Outcome::Interrupted
        },
        "{case}"
    );
    assert_eq!(run.events.len(), prefix.len(), "{case}: the whole prefix");
    let mut accounting = Accounting::default();
    for refusal in refusals(&run.events) {
        accounting.observe(refusal);
    }
    assert_eq!(accounting.count, u32::from(case != "success"), "{case}");
    assert_eq!(
        decide(run.outcome, &accounting),
        if case == "success" {
            Outcome::Completed
        } else {
            Outcome::Refused
        },
        "{case}"
    );
    match case.as_str() {
        "no-result" => {
            assert_eq!(run.stream_error, Some(StreamError::NoResult { events: 2 }));
        }
        "unreadable" => match &run.stream_error {
            Some(StreamError::ReadFailed { detail, events }) => {
                assert!(
                    detail.contains("while reading the event stream"),
                    "the phase belongs in the diagnostic, got {detail}"
                );
                assert_eq!(*events, 2, "the preceding evidence is retained");
            }
            other => panic!("expected a read failure, not a stream that ended: {other:?}"),
        },
        _ => {
            assert_eq!(run.stream_error, None, "{case}");
            let result = read_stream(&run.events, &[]).unwrap();
            assert_eq!(result.classification, Classification::Completed);
            assert!(result.cost.is_known());
        }
    }
    if let Ok(pid) = std::fs::read_to_string(dir.join("group")) {
        assert!(dead(&pid), "{case}: surviving child {pid}");
    }
}
