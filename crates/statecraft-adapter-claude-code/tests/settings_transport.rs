//! Spec 008 sections 3.1, 3.3 and 3.4 through a disposable settings-reading child.
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
/bin/ls -ln "$settings" > observed-mode
[ "${USER+x}" != x ] && [ "${HOME+x}" != x ]
/bin/cat "$settings" > observed-settings
/usr/bin/cmp expected-settings observed-settings
printf '%s' "$settings" > observed-path
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
if [ -f hang ]; then
  /bin/sleep 0.1
  /usr/bin/cmp "$settings" expected-settings
  : > settings-after-terminal
  /bin/sleep 30
fi
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

#[test]
fn timeout_after_terminal_denial_cleans_settings_and_retains_evidence() {
    let root = tempfile::tempdir().unwrap();
    // The one test whose subject is the deadline, so the one short budget.
    let attempt = Attempt::new(root.path(), "Bash(hang:*)", "hang", 5);
    let started = std::time::Instant::now();
    let execution = attempt.run();
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    attempt.check(&execution);
    assert!(root.path().join("settings-after-terminal").exists());
    assert_eq!(execution.supervised.outcome, Outcome::Interrupted);
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
