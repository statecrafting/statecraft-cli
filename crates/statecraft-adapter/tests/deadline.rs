//! Spec 004 sections 3.5 and 3.8: deadline enforcement through pipe cleanup.
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

// The outer test process can recover even if supervise blocks forever. Each
// fixture writes its process-group identity before doing anything that hangs.
// Output goes to a file, so the outer harness never waits for a pipe's EOF.
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
        if started.elapsed() >= Duration::from_secs(6) {
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    // Cleanup precedes every assertion, including the negative control timeout.
    drop(cleanup);
    let output = std::fs::read_to_string(dir.path().join("output")).unwrap();
    assert!(
        status.is_some_and(|status| status.success()),
        "{case}: worker status {status:?} after {:?}; outer timeout is 6s\n{output}",
        started.elapsed()
    );
}

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

// Re-entered only by bounded(), in a disposable process rather than a thread
// the broken supervisor could hold indefinitely. No provider is involved.
#[test]
fn deadline_worker() {
    let Ok(case) = std::env::var("DEADLINE_TEST_CASE") else {
        return;
    };
    let dir = std::env::var_os("DEADLINE_TEST_DIR").unwrap();
    let dir = Path::new(&dir);
    let init = r#"echo '{"event":"init","applied":[],"adapterVersion":"1.0","providerVersion":"fixture"}'"#;
    let refusal = r#"echo '{"event":"refusal","guard":"fixture","detail":"blocked"}'"#;
    let terminal = r#"echo '{"event":"result","classification":"completed","cost":{"amount":0.25,"unit":"fixture"}}'"#;
    let tail = match case.as_str() {
        "terminal-hang" => format!("{terminal}\nexec sleep 300"),
        "malformed-hang" => format!("echo 'malformed'\n{terminal}\nexec sleep 300"),
        "inherited-pipe" => format!("{terminal}\nsleep 300 &\necho $! > descendant\nexit 0"),
        "eof-hang" => "exec 1>&-\nexec sleep 300".into(),
        "unread-prompt" => "exec sleep 300".into(),
        "inherited-input" => {
            format!("{terminal}\nsleep 300 <&0 >/dev/null &\necho $! > descendant\nexit 0")
        }
        "success" => terminal.into(),
        "drain" => format!(
            "{terminal}\ni=0\nwhile [ $i -lt 10000 ]; do echo 'discard this trailing output'; i=$((i+1)); done"
        ),
        "no-result" => "exit 0".into(),
        _ => panic!("unknown fixture"),
    };
    let consume = if case == "unread-prompt" || case == "inherited-input" {
        ""
    } else {
        "cat > /dev/null"
    };
    let refusal = if case == "success" { "" } else { refusal };
    std::fs::write(
        dir.join("adapter.sh"),
        format!("echo $$ > group\n{consume}\n{init}\n{refusal}\n{tail}\n"),
    )
    .unwrap();
    let request = Request {
        workspace: dir.to_path_buf(),
        base_commit: "0".repeat(40),
        prompt: if case == "unread-prompt" || case == "inherited-input" {
            vec![b'x'; 1024 * 1024]
        } else {
            b"fixture prompt".to_vec()
        },
        capabilities: Requested::none(),
        deadline_seconds: 1,
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
    assert!(started.elapsed() < Duration::from_secs(4));
    let completed = case == "success" || case == "drain";
    assert_eq!(
        run.outcome,
        if completed {
            Outcome::Completed
        } else {
            Outcome::Interrupted
        }
    );
    assert!(matches!(run.events.first(), Some(Event::Init { .. })));
    let mut accounting = Accounting::default();
    for refusal in refusals(&run.events) {
        accounting.observe(refusal);
    }
    assert_eq!(accounting.count, u32::from(case != "success"));
    assert_eq!(
        decide(run.outcome, &accounting),
        if case == "success" {
            Outcome::Completed
        } else {
            Outcome::Refused
        }
    );
    if case == "malformed-hang" {
        assert!(matches!(
            run.stream_error,
            Some(StreamError::Malformed { line: 3, .. })
        ));
        assert_eq!(run.events.len(), 2);
    } else if case == "no-result" {
        assert_eq!(run.stream_error, Some(StreamError::NoResult { events: 2 }));
    } else {
        assert_eq!(run.stream_error, None);
    }
    if completed || case == "terminal-hang" || case.starts_with("inherited-") {
        let result = read_stream(&run.events, &[]).unwrap();
        assert_eq!(result.classification, Classification::Completed);
        assert!(result.cost.is_known());
        assert_eq!(
            run.events.len(),
            if case == "success" { 2 } else { 3 },
            "retain the trusted prefix only"
        );
    }
    // Observe the real child and descendant before the outer safety cleanup.
    // A zombie has stopped executing and cannot hold a pipe open.
    for marker in ["group", "descendant"] {
        if let Ok(pid) = std::fs::read_to_string(dir.join(marker)) {
            let output = Command::new("ps")
                .args(["-o", "stat=", "-p", pid.trim()])
                .output()
                .unwrap();
            let stat = String::from_utf8(output.stdout).unwrap();
            assert!(
                stat.trim().is_empty() || stat.trim().starts_with('Z'),
                "surviving {marker}: {stat}"
            );
        }
    }
}
