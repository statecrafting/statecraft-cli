//! Spec 004 sections 3.9, 3.3 and 3.4 through a disposable settings-reading child.
//! These synthetic denials test transport and evidence, not live qualification.

#![cfg(unix)]

use serde_json::{Value, json};
use statecraft_adapter::capability::Requested;
use statecraft_adapter::environment::{Blueprint, CheckSuiteCommands, construct};
use statecraft_adapter::protocol::{AttemptIdentity, Request, refusals};
use statecraft_adapter_claude_code::{Invocation, execution};
use statecraft_run::attempt::Outcome;
use std::os::unix::fs::PermissionsExt;
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
        std::fs::write(
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
# The deadline path, and it is deliberately the SHORTEST path through this
# fixture. Everything the full path does below is asserted by the attempts that
# name a generous deadline; repeating it here put five process spawns, a 100ms
# sleep and a blocking read of stdin between this child starting and the point
# the deadline test actually measures, and all of that had to finish inside the
# five seconds that ARE the subject. That is not a bound, it is a race, and a
# loaded machine lost it.
#
# What is left is the ordering the test needs and nothing else: the settings
# arrived intact, the terminal denial is emitted, the child is STILL ALIVE
# after emitting it, and only then does it hang past the deadline. The marker
# is a shell redirect on the line after the emit, so nothing schedulable sits
# between the two.
#
# The hang is far longer than the deadline on purpose: a child that could
# finish sleeping would end supervision by exiting, and the test would be
# measuring an exit rather than a timeout.
if [ -f hang ]; then
  /usr/bin/cmp "$settings" expected-settings
  /bin/cat stream.jsonl
  : > settings-after-terminal
  /bin/sleep 60
  exit 0
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
        )
        .unwrap();
        std::fs::set_permissions(&child, std::fs::Permissions::from_mode(0o700)).unwrap();
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

/// The deadline fires after a terminal denial: the denial survives as evidence
/// and the settings file is still cleaned up.
///
/// Four distinct things, asserted as four rather than through the shared
/// `check`, because only these four are this test's subject and everything
/// else `check` asserts is covered by the attempts that name a generous
/// deadline. Running all of it here is what made a five-second budget carry
/// work it was never sized for.
///
/// 1. **The terminal denial** reached the supervisor and is in the events.
/// 2. **The timeout**, and it is what ended supervision: the elapsed time is
///    bounded below by the deadline as well as above, so a child that exited
///    early would fail rather than pass as an interruption.
/// 3. **The cleanup** removed the supplied settings file, and left the
///    workspace's own two untouched.
/// 4. **The durable evidence**: the denial is still in the structured evidence
///    after the interruption, which is the thing an interruption could
///    plausibly have cost.
///
/// The ordering the test needs is established in the fixture child rather than
/// hoped for here: the marker it writes sits on the line after the terminal
/// emit, so `settings-after-terminal` existing means the child outlived its own
/// terminal event.
#[test]
fn timeout_after_terminal_denial_cleans_settings_and_retains_evidence() {
    use std::time::{Duration, Instant};

    let root = tempfile::tempdir().unwrap();
    // The one test whose subject is the deadline, so the one short budget.
    let attempt = Attempt::new(root.path(), "Bash(hang:*)", "hang", 5);
    let started = Instant::now();
    let execution = attempt.run();
    let elapsed = started.elapsed();

    // 2. The timeout, and that it is what ended supervision.
    assert_eq!(execution.supervised.outcome, Outcome::Interrupted);
    assert!(
        elapsed >= Duration::from_secs(5),
        "supervision ended in {elapsed:?}, before the deadline it was given; an \
         interruption that arrived early is not the timeout this test is about"
    );
    assert!(
        elapsed < Duration::from_secs(10),
        "supervision took {elapsed:?}, so it was held past its own deadline by \
         the child it was supervising"
    );

    // 1. The terminal denial reached the supervisor before the deadline.
    let seen = refusals(&execution.supervised.events);
    assert_eq!(seen.len(), 1, "the terminal denial did not arrive");
    assert_eq!(
        serde_json::from_str::<Value>(&seen[0].detail).unwrap(),
        attempt.denial
    );

    // And the child outlived it, which is what makes this a timeout AFTER a
    // terminal denial rather than one instead of it.
    assert!(
        root.path().join("settings-after-terminal").exists(),
        "the child did not reach the point past its own terminal event"
    );

    // 4. The evidence is durable across the interruption.
    assert_eq!(
        execution.evidence()["providerTerminal"]["permission_denials"],
        json!([attempt.denial])
    );
    assert_eq!(execution.termination().adapter_claimed, Outcome::Completed);

    // 3. The cleanup happened, and touched nothing of the workspace's own.
    let supplied =
        PathBuf::from(std::fs::read_to_string(root.path().join("observed-path")).unwrap());
    assert!(supplied.is_absolute());
    assert!(!supplied.starts_with(root.path().canonicalize().unwrap()));
    assert!(
        !supplied.exists(),
        "the settings file survived an interrupted supervision"
    );
    for name in ["settings.json", "settings.local.json"] {
        assert_eq!(
            std::fs::read_to_string(root.path().join(".claude").join(name)).unwrap(),
            EXISTING_SETTINGS
        );
    }
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
