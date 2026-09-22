//! Spec 004 sections 3.9, 3.3 and 3.4 through a disposable settings-reading child.
//! These synthetic denials test transport and evidence, not live qualification.

#![cfg(unix)]

use serde_json::{Value, json};
use statecraft_adapter::capability::Requested;
use statecraft_adapter::environment::{Blueprint, CheckSuiteCommands, construct};
use statecraft_adapter::protocol::{AttemptIdentity, Request, refusals};
use statecraft_adapter_claude_code::{Invocation, execution};
use statecraft_run::attempt::Outcome;
use std::path::{Path, PathBuf};

struct Attempt {
    invocation: Invocation,
    request: Request,
    denial: Value,
}

impl Attempt {
    fn new(workspace: &Path, rule: &str, mode: &str, deadline_seconds: u64) -> Self {
        std::fs::create_dir_all(workspace).unwrap();
        std::fs::create_dir(workspace.join(".claude")).unwrap();
        for name in ["settings.json", "settings.local.json"] {
            std::fs::write(workspace.join(".claude").join(name), EXISTING_SETTINGS).unwrap();
        }
        let child = workspace.join("fixture child ' $ ;.sh");
        statecraft_adapter::fixture::install_script(
            &child,
            r#"#!/bin/sh
set -eu
[ "$#" = 8 ]
[ "$1" = --print ] && [ "$2" = --output-format ]
[ "$3" = stream-json ] && [ "$4" = --verbose ]
[ "$5" = --max-turns ] && [ "$6" = 2 ]
[ "$7" = --settings ]
settings=$8
[ -f "$settings" ] && [ ! -L "$settings" ]
[ "${USER+x}" != x ] && [ "${HOME+x}" != x ]
printf '%s' "$settings" > observed-path
# The deadline path. The child hangs from here on and backgrounds a
# descendant that holds its output pipe, so the group kill is what ends both.
# Nothing the deadline test asserts depends on this branch being reached by any
# particular time: the product promises a deadline measured from spawn, not
# that a child is scheduled inside it. The trace says whether it was reached,
# for a failure message, and is never a precondition.
if [ -f hang ]; then
  echo entered >> trace
  /bin/sleep 300 &
  echo $! > descendant
  exec /bin/sleep 300
fi
/bin/ls -ln "$settings" > observed-mode
/bin/cat "$settings" > observed-settings
/usr/bin/cmp expected-settings observed-settings
printf '%s\n' "$@" > observed-args
if [ -f concurrent ]; then
  : > ready
  # One process per iteration, and this wait can now be long. Bound the rate.
  while [ ! -f ../a/ready ] || [ ! -f ../b/ready ]; do /bin/sleep 0.1; done
  /usr/bin/cmp "$settings" expected-settings
fi
# Ordering requirement: the prompt is consumed here, after the barrier, and
# must not move before it. `cat` returns only at stdin EOF, and EOF needs every
# copy of the write end closed, including one a concurrently spawned child
# inherited. Read before the barrier, each side of the concurrent test could
# wait on the other: one blocked on an EOF it could not reach, so it never
# signalled arrival, and the peer spun in the barrier until both hit the
# deadline. Consuming the prompt after the barrier removes that dependency.
# It does not remove every wait a stray descriptor can cause.
/bin/cat > observed-prompt
/bin/cat stream.jsonl
if [ -f obstruct-cleanup ]; then
  /bin/rm "$settings"
  /bin/mkdir "$settings"
fi
"#,
            0o700,
        )
        .unwrap();
        let invocation = Invocation::new(child.to_str().unwrap(), &[rule.into()], Some(2));
        std::fs::write(
            workspace.join("expected-settings"),
            serde_json::to_vec(&invocation.settings).unwrap(),
        )
        .unwrap();
        if !mode.is_empty() {
            std::fs::write(workspace.join(mode), "").unwrap();
        }
        let denial = json!({"tool_name": "Bash", "tool_use_id": "fixture-denial",
            "tool_input": {"command": rule}});
        let stream = format!(
            "{}\n{}\n",
            json!({"type":"system", "subtype":"init", "claude_code_version":"fixture"}),
            json!({"type":"result", "subtype":"success", "num_turns":2,
                "permission_denials":[denial]})
        );
        std::fs::write(workspace.join("stream.jsonl"), stream).unwrap();
        Self {
            invocation,
            request: Request {
                workspace: workspace.to_path_buf(),
                base_commit: "fixture".into(),
                prompt: b"private prompt: ' \" $(touch injected) `echo no` ;\n\\".to_vec(),
                capabilities: Requested::none(),
                // Named by each test. The deadline is the subject of exactly
                // one of them; for the others it is an incidental bound, and a
                // concurrently spawned child holding an inherited pipe
                // descriptor can delay this child's EOF long enough to exhaust
                // a short one.
                //
                // A generous bound here is **mitigation**: it widens the margin
                // and leaves that behaviour in place. It is not a fix, and it
                // cannot resolve a mutual wait, which is what the ordering
                // requirement above is for. Short only where the deadline is
                // the thing under test, matching the convention
                // `statecraft-adapter`'s own negative suite uses.
                deadline_seconds,
                attempt: AttemptIdentity {
                    run_id: "settings-fixture".into(),
                    number: 1,
                },
            },
            denial,
        }
    }

    fn run(&self) -> execution::Execution {
        let environment = construct(&Blueprint::empty(), &CheckSuiteCommands::default());
        execution::supervise(&self.invocation, &self.request, &environment, &[]).unwrap()
    }

    fn check(&self, execution: &execution::Execution) -> PathBuf {
        let root = &self.request.workspace;
        for name in ["settings.json", "settings.local.json"] {
            assert_eq!(
                std::fs::read_to_string(root.join(".claude").join(name)).unwrap(),
                EXISTING_SETTINGS
            );
        }
        assert!(
            std::fs::read_to_string(root.join("observed-mode"))
                .unwrap()
                .starts_with("-rw-------")
        );
        let settings: Value = serde_json::from_slice(
            &std::fs::read(root.join("observed-settings"))
                .expect("child must read the supplied settings"),
        )
        .unwrap();
        assert_eq!(settings, self.invocation.settings);
        assert!(
            !settings["permissions"]["deny"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            std::fs::read(root.join("observed-prompt")).unwrap(),
            self.request.prompt
        );
        let path = PathBuf::from(std::fs::read_to_string(root.join("observed-path")).unwrap());
        assert!(path.is_absolute());
        assert!(!path.starts_with(root.canonicalize().unwrap()));
        assert!(
            !path.exists(),
            "settings must be cleaned up after supervision"
        );
        let args = std::fs::read_to_string(root.join("observed-args")).unwrap();
        assert!(!args.contains("private prompt"));
        assert!(!args.contains(&self.invocation.deny_rules()[0]));
        assert!(!root.join("injected").exists());
        let refusals = refusals(&execution.supervised.events);
        assert_eq!(refusals.len(), 1);
        assert_eq!(
            serde_json::from_str::<Value>(&refusals[0].detail).unwrap(),
            self.denial
        );
        assert_eq!(
            execution.evidence()["providerTerminal"]["permission_denials"],
            json!([self.denial])
        );
        assert_eq!(execution.termination().adapter_claimed, Outcome::Completed);
        assert_eq!(execution.result.as_ref().unwrap().num_turns, 2);
        path
    }
}

const EXISTING_SETTINGS: &str = r#"{"permissions":{"deny":["Read(existing-secret)"]},"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"existing-hook"}]}]}}"#;

#[test]
fn declared_denials_reach_the_child_and_survive_as_structured_evidence() {
    let root = tempfile::tempdir().unwrap();
    let attempt = Attempt::new(
        root.path(),
        "Bash(echo 'quote' \"double\" \\ $HOME $(touch injected) `id`;\n雪:*)",
        "",
        30,
    );
    let execution = attempt.run();
    attempt.check(&execution);
    assert_eq!(execution.supervised.outcome, Outcome::Refused);
    assert!(execution.supervised.stream_error.is_none());
    assert!(execution.settings_cleanup_error.is_none());
}

#[test]
fn concurrent_attempts_keep_independent_settings_until_both_children_read_them() {
    let root = tempfile::tempdir().unwrap();
    let a = Attempt::new(&root.path().join("a"), "Bash(first:*)", "concurrent", 30);
    let b = Attempt::new(&root.path().join("b"), "Bash(second:*)", "concurrent", 30);
    let (first, second) = std::thread::scope(|scope| {
        let first = scope.spawn(|| a.run());
        let second = scope.spawn(|| b.run());
        (first.join().unwrap(), second.join().unwrap())
    });
    assert_ne!(a.check(&first), b.check(&second));
    assert_eq!(first.supervised.outcome, Outcome::Refused);
    assert_eq!(second.supervised.outcome, Outcome::Refused);
}

/// The deadline, the kill and the cleanup, through this crate's execution path.
///
/// Spec 004 section 3.5 case 3, with the contract spec 002's 2026-09-22 entry
/// records. The deadline is five seconds and runs from `spawn`, as the product's
/// does. What is asserted holds whether or not the child was ever scheduled
/// inside those five seconds, because the product does not promise that it
/// would be:
///
/// 1. supervision ended **no earlier** than the deadline, so a child that
///    exited early fails rather than passing as a timeout;
/// 2. it ended well before the child's own 300-second hang, so it was not held
///    by the process it supervises. The 60-second bound separates that defect
///    from the latency of the `kill` the supervisor spawns;
/// 3. the attempt is `interrupted`, nothing in the group survived, and a
///    descendant the child did start is dead by its process id;
/// 4. the workspace's own settings are untouched, and the supplied settings
///    file, when the child lived to name it, is gone. The unconditional form of
///    that last check is `execution`'s own
///    `settings_are_removed_when_supervision_times_out`, where the temporary
///    root is the test's.
///
/// Retention of a terminal denial across the interruption is measured where it
/// is decided, without a race: `execution`'s
/// `an_interruption_keeps_the_terminal_denial_the_supervisor_read` and the
/// supervisor's `events_delivered_before_the_deadline_survive_it`.
#[test]
fn a_hung_child_is_interrupted_at_the_deadline_with_its_group_and_settings_cleaned() {
    use std::time::{Duration, Instant};

    let root = tempfile::tempdir().unwrap();
    let attempt = Attempt::new(root.path(), "Bash(hang:*)", "hang", 5);
    let started = Instant::now();
    let execution = attempt.run();
    let elapsed = started.elapsed();
    let trace = std::fs::read_to_string(root.path().join("trace")).unwrap_or_default();
    let context = format!(
        "after {elapsed:?}: outcome {:?}, stream error {:?}, survivors {:?}; the child's trace \
         is {trace:?} (empty means it was never scheduled, which the product does not \
         promise and nothing here requires)",
        execution.supervised.outcome,
        execution.supervised.stream_error,
        execution.supervised.surviving_processes,
    );

    assert!(
        elapsed >= Duration::from_secs(5),
        "supervision ended before its deadline, so this is not a timeout: {context}"
    );
    assert!(
        elapsed < Duration::from_secs(60),
        "supervision was held far past its deadline by the child it supervises: {context}"
    );
    assert_eq!(
        execution.supervised.outcome,
        Outcome::Interrupted,
        "{context}"
    );
    assert!(
        execution.supervised.surviving_processes.is_none(),
        "the group outlived the kill: {context}"
    );
    if let Ok(pid) = std::fs::read_to_string(root.path().join("descendant")) {
        let alive = std::process::Command::new("kill")
            .args(["-s", "0", pid.trim()])
            .output()
            .unwrap()
            .status
            .success();
        assert!(
            !alive,
            "descendant {} escaped supervision: {context}",
            pid.trim()
        );
    }
    if let Ok(path) = std::fs::read_to_string(root.path().join("observed-path")) {
        assert!(
            !Path::new(&path).exists(),
            "the settings file survived an interrupted supervision: {context}"
        );
    }
    for name in ["settings.json", "settings.local.json"] {
        assert_eq!(
            std::fs::read_to_string(root.path().join(".claude").join(name)).unwrap(),
            EXISTING_SETTINGS
        );
    }
    assert!(execution.settings_cleanup_error.is_none(), "{context}");
}

#[test]
fn a_settings_cleanup_failure_cannot_erase_the_terminal_denial() {
    let root = tempfile::tempdir().unwrap();
    let attempt = Attempt::new(root.path(), "Bash(cleanup:*)", "obstruct-cleanup", 30);
    let execution = attempt.run();
    let path = std::fs::read_to_string(root.path().join("observed-path")).unwrap();
    // Remove only the empty directory the fixture put at its own settings path.
    std::fs::remove_dir(&path).unwrap();
    attempt.check(&execution);
    assert_eq!(execution.supervised.outcome, Outcome::Refused);
    assert!(execution.settings_cleanup_error.is_some());
    assert!(execution.evidence()["settingsCleanupError"].is_string());
}

#[test]
fn malformed_stream_cleanup_removes_the_settings_file() {
    let root = tempfile::tempdir().unwrap();
    let attempt = Attempt::new(root.path(), "Bash(malformed:*)", "", 30);
    std::fs::write(root.path().join("stream.jsonl"), "not json\n").unwrap();
    let execution = attempt.run();
    assert_eq!(execution.supervised.outcome, Outcome::Interrupted);
    assert!(execution.supervised.stream_error.is_some());
    assert!(execution.settings_cleanup_error.is_none());
    let path = std::fs::read_to_string(root.path().join("observed-path")).unwrap();
    assert!(!Path::new(&path).exists());
}
