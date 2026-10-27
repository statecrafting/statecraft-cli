//! Spec 063 process contract, using local stubs only.
#[path = "support/json_naming.rs"]
mod json_naming;

use serde_json::{Value, json};
use std::process::Command;
const GH: &str = r#"#!/bin/sh
here="$(dirname "$0")"
printf '%s\n' "$*" >> "$here/calls"
if [ -f "$here/failure" ]; then
  cat "$here/failure" >&2
  echo 'PRIVATE RESPONSE BODY HTTP 404'
  exit 1
fi
case "$1 $2" in
  'api graphql')
    case "$4" in *'pullRequest(number:7)'*'mergeQueueEntry { state position }'*) ;; *) exit 97 ;; esac
    cat "$here/pr.json"
    if [ -f "$here/next.json" ]; then mv "$here/next.json" "$here/pr.json"; fi ;;
  'api -X')
    [ "$3" = GET ] || exit 96
    case "$4" in
      *check-runs*) echo '{"check_runs":[]}' ;;
      *statecraft-ci.yml/runs*) echo '{"workflow_runs":[]}' ;;
      *) exit 95 ;;
    esac ;;
  *) exit 94 ;;
esac
"#;
struct Fixture(tempfile::TempDir);
impl Fixture {
    fn new(state: &str, queue: Value) -> Self {
        let f = Self(tempfile::tempdir().unwrap());
        statecraft_adapter::fixture::install_script(&f.0.path().join("gh"), GH, 0o755).unwrap();
        statecraft_adapter::fixture::install_script(
            &f.0.path().join("git"),
            r#"#!/bin/sh
here="$(dirname "$0")"
case "$*" in
 'show HEAD:.statecraft/environment.json') cat "$here/declaration.json" ;;
 'remote get-url origin') echo git@github.com:o/r.git ;;
 *) exit 98 ;;
esac
"#,
            0o755,
        )
        .unwrap();
        f.write(
            "declaration",
            json!({"project":{"landing":{"default_branch":"trunk"}}}),
        );
        f.write("pr", json!({"data":{"repository":{"pullRequest":{"state":state,"headRefOid":"abc","headRepository":{"nameWithOwner":"o/r"},"autoMergeRequest":null,"mergeQueueEntry":queue}}}}));
        f
    }
    fn write(&self, name: &str, value: Value) {
        let mut value = value;
        if name == "declaration" {
            value["version"] = json!(2);
            value["pins"] = json!({"product":"0.0.0","spec_spine":"unpinned","adapters":{}});
        }
        std::fs::write(
            self.0.path().join(format!("{name}.json")),
            value.to_string(),
        )
        .unwrap();
    }
    fn run(&self, args: &[&str], exit: i32, outcome: &str) -> Value {
        let output = Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(["land", "wait", "7", "--json"])
            .args(args)
            .current_dir(self.0.path())
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.0.path().display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("STATECRAFT_HOME", self.0.path().join("no-home"))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(exit), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        let envelope = json_naming::from_output(&output.stdout).unwrap();
        assert_eq!(envelope["verb"], "land.wait");
        assert_eq!(envelope["exitCode"], exit);
        if exit >= 2 {
            assert_eq!(
                envelope["error"]["kind"],
                match exit {
                    2 => "refused",
                    3 => "usage",
                    4 => "io",
                    _ => unreachable!(),
                }
            );
        }
        let value = json_naming::payload(&envelope).clone();
        assert_eq!(value["outcome"], outcome);
        value
    }
    fn wait(&self, event: &str, exit: i32, outcome: &str) -> Value {
        self.run(
            &["--until", event, "--timeout", "0s", "--repo", "o/r"],
            exit,
            outcome,
        )
    }
}
#[test]
fn every_event_has_the_specified_json_and_exit() {
    for (event, state, outcome, exit) in [
        ("merged", "MERGED", "merged", 0),
        ("merged", "CLOSED", "closed-unmerged", 1),
        ("landed", "OPEN", "dropped", 0),
        ("landed", "CLOSED", "closed", 0),
        ("queue-exit", "OPEN", "open", 0),
        ("reviewed", "OPEN", "absent", 0),
        ("changed", "OPEN", "timed-out", 1),
    ] {
        let f = Fixture::new(state, Value::Null);
        let value = f.wait(event, exit, outcome);
        assert_eq!(value["event"], event);
        assert_eq!(value["observations"][0]["pullRequest"], 7);
        assert_eq!(
            value["policy"]["default_branch"],
            json!({"value":"trunk","layer":"project-override"})
        );
        assert_eq!(value["observations"][0]["head"], "abc");
        assert_eq!(
            value["pullRequest"],
            if outcome == "timed-out" {
                Value::Null
            } else {
                json!(7)
            }
        );
    }
}
#[test]
fn queue_position_twenty_five_is_still_waiting() {
    let f = Fixture::new("OPEN", json!({"state":"QUEUED","position":25}));
    let value = f.wait("landed", 1, "timed-out");
    assert_eq!(value["observations"][0]["queue"]["position"], 25);
    let calls = std::fs::read_to_string(f.0.path().join("calls")).unwrap();
    assert_eq!(calls.lines().count(), 1);
    assert!(!calls.contains("first:"));
}
#[test]
fn refusal_usage_and_stderr_classification() {
    let f = Fixture::new("OPEN", Value::Null);
    f.write("declaration", json!({"project":{"landing":{}}}));
    assert!(
        f.wait("merged", 2, "refused")["reason"]
            .as_str()
            .unwrap()
            .contains("landing.default_branch")
    );
    assert!(!f.0.path().join("calls").exists());
    f.write(
        "declaration",
        json!({"project":{"landing":{"default_branch":42}}}),
    );
    assert!(
        f.wait("merged", 2, "refused")["reason"]
            .as_str()
            .unwrap()
            .contains("malformed landing.default_branch")
    );
    f.run(&["--until", "merged"], 3, "usage");
    for (stderr, code, outcome, reason) in [
        (
            "gh: HTTP 401\nPRIVATE HEADER",
            4,
            "failed",
            "unauthorized: HTTP 401",
        ),
        ("gh: HTTP 429", 4, "failed", "rate limited: HTTP 429"),
        ("gh: HTTP 404", 2, "refused", "does not exist"),
        (
            "GraphQL: Could not resolve to a PullRequest with the number of 7.",
            2,
            "refused",
            "does not exist",
        ),
    ] {
        let f = Fixture::new("OPEN", Value::Null);
        std::fs::write(f.0.path().join("failure"), stderr).unwrap();
        let value = f.wait("merged", code, outcome);
        assert!(value["reason"].as_str().unwrap().contains(reason));
        assert!(!value.to_string().contains("PRIVATE"));
    }
}
#[test]
fn changed_uses_the_production_minute_interval() {
    let f = Fixture::new("OPEN", Value::Null);
    let mut next: Value =
        serde_json::from_slice(&std::fs::read(f.0.path().join("pr.json")).unwrap()).unwrap();
    next["data"]["repository"]["pullRequest"]["headRefOid"] = json!("def");
    f.write("next", next);
    let value = f.run(&["--until", "changed", "--timeout", "90s"], 0, "changed");
    assert_eq!(value["observations"][0]["head"], "def");
}
