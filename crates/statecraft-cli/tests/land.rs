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

// Write-verb fixtures use real scratch Git repositories and a gh stub. The
// repository being developed, its HEAD and every real remote stay untouched.
const ACTION_GH: &str = r#"#!/bin/sh
here="$(dirname "$0")"
printf 'gh %s\n' "$*" >> "$here/calls"
if [ -f "$here/failure" ]; then cat "$here/failure" >&2; echo 'PRIVATE BODY'; exit 1; fi
case "$1 $2" in
 'pr merge')
   if [ -f "$here/merge-failure" ]; then cat "$here/merge-failure" >&2; echo 'PRIVATE BODY'; exit 1; fi ;;
 'api graphql')
   case "$4" in
     *timelineItems*) cat "$here/timeline.json" ;;
     *statusCheckRollup*)
       case "$4" in *'after:"next"'*) cat "$here/checks-next.json" ;; *) cat "$here/checks.json" ;; esac ;;
     *'signature { isValid }'*) cat "$here/signature.json" ;;
     *viewerDefaultMergeMethod*) cat "$here/method.json" ;;
     *pullRequest*) cat "$here/pr.json" ;;
     *) exit 93 ;;
   esac ;;
 'api -X')
   [ "$3" = GET ] || exit 94
   case "$4" in
     *check-suites*) cat "$here/suites.json" ;;
     *check-runs*) cat "$here/review.json" ;;
     *statecraft-ci.yml/runs*) cat "$here/runs.json" ;;
     *jobs*) echo '{"jobs":[]}' ;;
     *) exit 95 ;;
   esac ;;
 *) exit 96 ;;
esac
"#;
struct ActionFixture {
    dir: tempfile::TempDir,
    head: String,
}
impl ActionFixture {
    fn new() -> Self {
        let mut f = Self {
            dir: tempfile::tempdir().unwrap(),
            head: String::new(),
        };
        std::fs::create_dir(f.path().join("bin")).unwrap();
        std::fs::create_dir(f.path().join(".statecraft")).unwrap();
        f.git(&["init", "-b", "topic"]);
        f.git(&["config", "user.name", "Fixture"]);
        f.git(&["config", "user.email", "fixture@example.invalid"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        f.git(&["init", "--bare", "remote.git"]);
        f.git(&["remote", "add", "origin", "remote.git"]);
        f.declaration(json!({"default_branch":"trunk","pre_push":[]}), false);
        f.record();
        // Seed the bare remote locally; production push is exercised below.
        let remote = f.path().join("remote.git");
        f.git(&["-C", remote.to_str().unwrap(), "fetch", "..", "topic:topic"]);
        statecraft_adapter::fixture::install_script(&f.path().join("bin/gh"), ACTION_GH, 0o755)
            .unwrap();
        // Only log pushes, delegating every Git operation to the real binary.
        let git = Command::new("which").arg("git").output().unwrap();
        let git = String::from_utf8(git.stdout).unwrap();
        let script = format!(
            r#"#!/bin/sh
here="$(dirname "$0")"
printf '%s\n' "$*" >> "$here/git-calls"
case "$1" in push) printf 'git %s\n' "$*" >> "$here/calls" ;; esac
exec '{}' "$@"
"#,
            git.trim()
        );
        statecraft_adapter::fixture::install_script(&f.path().join("bin/git"), &script, 0o755)
            .unwrap();
        f.pr(json!({"state":"OPEN","isDraft":false,"baseRefName":"trunk","headRefName":"topic","headRefOid":f.head,"headRepository":{"nameWithOwner":"o/r"},"autoMergeRequest":null,"mergeQueueEntry":null}));
        f.review("no-findings");
        f.write("runs", json!({"workflow_runs":[]}));
        f.checks(vec![]);
        f.write(
            "signature",
            json!({"data":{"repository":{"object":{"signature":{"isValid":true}}}}}),
        );
        f.write(
            "method",
            json!({"data":{"repository":{"mergeQueue":null,"viewerDefaultMergeMethod":"MERGE"}}}),
        );
        f.write("timeline",json!({"data":{"repository":{"pullRequest":{"timelineItems":{"nodes":[],"pageInfo":{"hasNextPage":false}}}}}}));
        f.write(
            "suites",
            json!({"check_suites":[{"head_sha":f.head,"created_at":"2026-10-27T10:00:00Z"}]}),
        );
        f
    }
    fn path(&self) -> &std::path::Path {
        self.dir.path()
    }
    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.path())
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }
    fn declaration(&self, landing: Value, signed: bool) {
        std::fs::write(self.path().join(".statecraft/environment.json"),json!({"version":2,"pins":{"product":"0.0.0","spec_spine":"unpinned","adapters":{}},"project":{"landing":landing,"setup":{"profile":"github-actions-rust","revision":12,"identity":"fixture","parameters":{"governance.require_signed_commits":signed}}}}).to_string()).unwrap();
    }
    fn record(&mut self) {
        self.git(&["add", ".statecraft"]);
        let tree = self.git(&["write-tree"]);
        let mut args = vec!["commit-tree", tree.as_str(), "-m", "fixture"];
        if !self.head.is_empty() {
            args.extend(["-p", self.head.as_str()]);
        }
        let head = self.git(&args);
        self.git(&["update-ref", "refs/heads/topic", &head]);
        self.head = head;
    }
    fn write(&self, name: &str, value: Value) {
        std::fs::write(
            self.path().join(format!("bin/{name}.json")),
            value.to_string(),
        )
        .unwrap();
    }
    fn pr(&self, value: Value) {
        self.write("pr", json!({"data":{"repository":{"pullRequest":value}}}));
    }
    fn change_pr(&self, field: &str, value: Value) {
        let mut pr: Value =
            serde_json::from_slice(&std::fs::read(self.path().join("bin/pr.json")).unwrap())
                .unwrap();
        pr["data"]["repository"]["pullRequest"][field] = value;
        self.write("pr", pr);
    }
    fn review(&self, result: &str) {
        self.write("review",json!({"check_runs":[{"id":1,"name":"statecraft-ai-review","html_url":"https://example.invalid/check/1","output":{"text":format!("```json\n{}\n```",json!({"subject":{"pullRequest":7,"head":self.head},"result":result}))}}]}));
    }
    fn checks(&self, rows: Vec<Value>) {
        self.write("checks",json!({"data":{"repository":{"object":{"statusCheckRollup":{"contexts":{"nodes":rows,"pageInfo":{"hasNextPage":false}}}}}}}));
    }
    fn command(&self, verb: &str, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_statecraft-cli"));
        cmd.args(["land", verb, "--json"])
            .args(args)
            .current_dir(self.path())
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.path().join("bin").display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("STATECRAFT_HOME", self.path().join("no-home"))
            .env_remove("STATECRAFT_RUN_ID")
            .env_remove("STATECRAFT_ATTEMPT");
        cmd
    }
    fn run(&self, verb: &str, args: &[&str], code: i32, outcome: &str) -> Value {
        let output = self.command(verb, args).output().unwrap();
        self.answer(verb, output, code, outcome)
    }
    fn answer(&self, verb: &str, output: std::process::Output, code: i32, outcome: &str) -> Value {
        assert_eq!(output.status.code(), Some(code), "{output:?}");
        let envelope = json_naming::from_output(&output.stdout).unwrap();
        assert_eq!(envelope["verb"], format!("land.{verb}"));
        assert_eq!(envelope["exitCode"], code);
        let answer = json_naming::payload(&envelope).clone();
        assert_eq!(answer["outcome"], outcome, "{answer}");
        answer
    }
    fn calls(&self) -> String {
        std::fs::read_to_string(self.path().join("bin/calls")).unwrap_or_default()
    }
    // Every git invocation; `calls` records only gh and `git push`.
    fn git_calls(&self) -> String {
        std::fs::read_to_string(self.path().join("bin/git-calls")).unwrap_or_default()
    }
    fn no_writes(&self) {
        let calls = self.calls();
        assert!(
            !calls.contains("pr merge") && !calls.contains("git push"),
            "{calls}"
        );
    }
}
#[test]
fn landing_acts_refuse_attempt_presence_even_empty_before_usage_and_reads() {
    for verb in ["arm", "push"] {
        for key in ["STATECRAFT_RUN_ID", "STATECRAFT_ATTEMPT"] {
            for value in ["", "attempt-1"] {
                let f = ActionFixture::new();
                let output = f
                    .command(verb, &["--invalid"])
                    .env(key, value)
                    .output()
                    .unwrap();
                f.answer(verb, output, 2, "inside-run-attempt");
                assert!(f.calls().is_empty());
                assert!(!f.path().join("bin/git-calls").exists());
            }
        }
    }
}
#[test]
fn landing_acts_usage_precedes_policy_and_missing_default_branch_refuses() {
    for verb in ["arm", "push"] {
        let mut f = ActionFixture::new();
        f.declaration(json!({}), false);
        f.record();
        for args in [
            vec![],
            vec!["0"],
            vec!["7", "--repo", "bad"],
            vec!["7", "--head", "xyz"],
            vec!["7", "--skip"],
        ] {
            f.run(verb, &args, 3, "usage");
        }
        let args = if verb == "arm" {
            vec!["7", "--repo", "o/r"]
        } else {
            vec!["7", "--repo", "o/r", "--lease", "abc"]
        };
        assert!(
            f.run(verb, &args, 2, "refused")["reason"]
                .as_str()
                .unwrap()
                .contains("landing.default_branch")
        );
        assert!(f.calls().is_empty());
    }
}
#[test]
fn arm_refuses_each_condition_in_order() {
    for (field, value, outcome) in [
        ("state", json!("CLOSED"), "not-open"),
        ("isDraft", json!(true), "draft"),
        ("baseRefName", json!("stack-base"), "stacked"),
    ] {
        let f = ActionFixture::new();
        f.change_pr(field, value);
        let answer = f.run(
            "arm",
            &["7", "--repo", "o/r", "--head", "deadbeef"],
            2,
            outcome,
        );
        if outcome == "stacked" {
            assert!(answer["reason"].as_str().unwrap().contains("stack-base"));
        }
        f.no_writes();
    }
    let f = ActionFixture::new();
    let answer = f.run(
        "arm",
        &["7", "--repo", "o/r", "--head", "deadbeef"],
        2,
        "stale",
    );
    assert!(answer["reason"].as_str().unwrap().contains(&f.head));
    f.no_writes();
}
#[test]
fn arm_refuses_review_skipped_draft_and_other_non_passing_reviews() {
    for result in ["skipped:draft", "skipped:transient", "findings"] {
        let f = ActionFixture::new();
        f.review(result);
        f.run(
            "arm",
            &["7", "--repo", "o/r"],
            2,
            &format!("review-{result}"),
        );
        f.no_writes();
    }
    let f = ActionFixture::new();
    f.write("review", json!({"check_runs":[]}));
    f.run("arm", &["7", "--repo", "o/r"], 2, "review-absent");
    f.no_writes();
    f.write("runs", json!({"workflow_runs":[{"id":1,"run_attempt":1,"head_sha":f.head,"status":"in_progress","pull_requests":[{"number":7}]}]}));
    f.run("arm", &["7", "--repo", "o/r"], 2, "review-pending");
    f.no_writes();
}
#[test]
fn arm_refuses_cancelled_and_skipped_required_checks() {
    for conclusion in [
        "CANCELLED",
        "SKIPPED",
        "FAILURE",
        "TIMED_OUT",
        "NEUTRAL",
        "ACTION_REQUIRED",
        "STALE",
        "STARTUP_FAILURE",
    ] {
        let f = ActionFixture::new();
        f.checks(vec![json!({"__typename":"CheckRun","name":"build","isRequired":true,"status":"COMPLETED","conclusion":conclusion})]);
        let value = f.run("arm", &["7", "--repo", "o/r"], 2, "check-failed");
        assert!(value["reason"].as_str().unwrap().contains(conclusion));
        f.no_writes();
    }
}
#[test]
fn arm_pins_head_allows_pending_and_optional_failures_and_uses_repository_method() {
    for (queue, method, flag) in [
        (json!(null), "MERGE", Some("--merge")),
        (json!(null), "SQUASH", Some("--squash")),
        (json!(null), "REBASE", Some("--rebase")),
        (json!({"id":"queue"}), "MERGE", None),
    ] {
        let f = ActionFixture::new();
        f.checks(vec![json!({"__typename":"CheckRun","name":"running","isRequired":true,"status":"IN_PROGRESS","conclusion":null}),json!({"__typename":"CheckRun","name":"optional","isRequired":false,"status":"COMPLETED","conclusion":"FAILURE"})]);
        f.write(
            "method",
            json!({"data":{"repository":{"mergeQueue":queue,"viewerDefaultMergeMethod":method}}}),
        );
        f.run(
            "arm",
            &["7", "--repo", "o/r", "--head", &f.head],
            0,
            "armed",
        );
        let calls = f.calls();
        let last = calls.lines().last().unwrap();
        assert!(last.contains(&format!(
            "pr merge 7 --repo o/r --auto --match-head-commit {}",
            f.head
        )));
        for method in ["--merge", "--squash", "--rebase"] {
            assert_eq!(
                last.split_whitespace().any(|w| w == method),
                flag == Some(method)
            );
        }
    }
}
#[test]
fn arm_already_armed_and_armed_before_head_never_rearm_or_check_conditions() {
    for (enabled, code, outcome) in [
        ("2026-10-27T11:00:00Z", 0, "already-armed"),
        ("2026-10-27T09:00:00Z", 2, "armed-before-head"),
    ] {
        let f = ActionFixture::new();
        f.change_pr("autoMergeRequest", json!({"enabledAt":enabled}));
        f.change_pr("state", json!("CLOSED"));
        f.run(
            "arm",
            &["7", "--repo", "o/r", "--head", "bad"],
            code,
            outcome,
        );
        f.no_writes();
        assert!(!f.calls().contains("check-runs"));
    }
}
#[test]
fn arm_push_time_prefers_latest_matching_force_push_then_earliest_suite_and_refuses_unknown() {
    let f = ActionFixture::new();
    f.change_pr(
        "autoMergeRequest",
        json!({"enabledAt":"2026-10-27T11:00:00Z"}),
    );
    f.write("timeline",json!({"data":{"repository":{"pullRequest":{"timelineItems":{"nodes":[{"createdAt":"2026-10-27T09:00:00Z","afterCommit":{"oid":f.head}},{"createdAt":"2026-10-27T12:00:00Z","afterCommit":{"oid":f.head}},{"createdAt":"2026-10-27T13:00:00Z","afterCommit":{"oid":"other"}}],"pageInfo":{"hasNextPage":false}}}}}}));
    f.run("arm", &["7", "--repo", "o/r"], 2, "armed-before-head");
    f.no_writes();
    assert!(!f.calls().contains("check-suites"));
    let f = ActionFixture::new();
    f.change_pr(
        "autoMergeRequest",
        json!({"enabledAt":"2026-10-27T11:00:00Z"}),
    );
    f.write("suites",json!({"check_suites":[{"head_sha":f.head,"created_at":"2026-10-27T12:00:00Z"},{"head_sha":f.head,"created_at":"2026-10-27T10:00:00Z"}]}));
    f.run("arm", &["7", "--repo", "o/r"], 0, "already-armed");
    f.no_writes();
    f.write("suites", json!({"check_suites":[]}));
    assert!(
        f.run("arm", &["7", "--repo", "o/r"], 2, "armed-before-head")["reason"]
            .as_str()
            .unwrap()
            .contains("cannot establish")
    );
    f.no_writes();
}
#[test]
fn arm_checks_paginate_and_status_context_failures_count() {
    let f = ActionFixture::new();
    f.write("checks",json!({"data":{"repository":{"object":{"statusCheckRollup":{"contexts":{"nodes":[],"pageInfo":{"hasNextPage":true,"endCursor":"next"}}}}}}}));
    f.write("checks-next",json!({"data":{"repository":{"object":{"statusCheckRollup":{"contexts":{"nodes":[{"__typename":"StatusContext","context":"legacy-ci","isRequired":true,"state":"ERROR"}],"pageInfo":{"hasNextPage":false}}}}}}}));
    let value = f.run("arm", &["7", "--repo", "o/r"], 2, "check-failed");
    assert!(
        value["reason"]
            .as_str()
            .unwrap()
            .contains("legacy-ci: ERROR")
    );
    assert!(f.calls().contains("after:\"next\""));
    f.no_writes();
}
#[test]
fn arm_enforces_github_signature_only_when_declared_and_classifies_write_stderr() {
    let mut f = ActionFixture::new();
    f.declaration(json!({"default_branch":"trunk","pre_push":[]}), true);
    f.record();
    f.change_pr("headRefOid", json!(f.head));
    f.review("no-findings");
    f.write(
        "signature",
        json!({"data":{"repository":{"object":{"signature":{"isValid":false}}}}}),
    );
    f.run("arm", &["7", "--repo", "o/r"], 2, "unsigned");
    f.no_writes();
    let f = ActionFixture::new();
    std::fs::write(f.path().join("bin/merge-failure"), "gh: HTTP 403").unwrap();
    let value = f.run("arm", &["7", "--repo", "o/r"], 4, "failed");
    assert!(!value.to_string().contains("PRIVATE"));
    assert!(value["reason"].as_str().unwrap().contains("403"));
}
#[test]
fn push_disarms_before_local_bare_remote_push_and_never_rearms() {
    let mut f = ActionFixture::new();
    let lease = f.head.clone();
    f.declaration(
        json!({"default_branch":"trunk","pre_push":[["printf","preflight\\n"]]}),
        false,
    );
    f.record();
    f.change_pr(
        "autoMergeRequest",
        json!({"enabledAt":"2026-10-27T10:00:00Z"}),
    );
    let output = f
        .command("push", &["7", "--repo", "o/r", "--lease", &lease])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stderr), "preflight\n");
    let value = f.answer("push", output, 0, "pushed");
    assert_eq!(value["disarmed"], true);
    assert_eq!(value["head"], f.head);
    assert_eq!(value["policy"]["prePush"]["layer"], "project-override");
    let calls = f.calls();
    assert!(calls.find("--disable-auto").unwrap() < calls.find("git push").unwrap());
    assert!(!calls.contains("--auto"));
    assert_eq!(
        f.git(&["--git-dir=remote.git", "rev-parse", "topic"]),
        f.head
    );
}
#[test]
fn push_stale_lease_reports_remote_head_after_disarming() {
    let mut f = ActionFixture::new();
    let remote = f.head.clone();
    f.declaration(
        json!({"default_branch":"trunk","pre_push":[["true"]]}),
        false,
    );
    f.record();
    f.change_pr(
        "autoMergeRequest",
        json!({"enabledAt":"2026-10-27T10:00:00Z"}),
    );
    let value = f.run(
        "push",
        &["7", "--repo", "o/r", "--lease", &"0".repeat(40)],
        2,
        "lease-refused",
    );
    assert_eq!(value["remoteHead"], remote);
    assert_eq!(value["disarmed"], true);
    assert!(!f.calls().contains("--auto"));
}
#[test]
fn push_refuses_wrong_branch_and_fork_before_other_guards() {
    for (key, value) in [
        ("headRefName", json!("another")),
        ("headRepository", json!({"nameWithOwner":"fork/r"})),
    ] {
        let f = ActionFixture::new();
        f.change_pr(key, value);
        f.run(
            "push",
            &["7", "--repo", "o/r", "--lease", &f.head],
            2,
            "wrong-branch",
        );
        f.no_writes();
    }
}
#[test]
fn push_refuses_unsigned_committed_head() {
    let mut f = ActionFixture::new();
    f.declaration(json!({"default_branch":"trunk","pre_push":[]}), true);
    f.record();
    f.run(
        "push",
        &["7", "--repo", "o/r", "--lease", &f.head],
        2,
        "unsigned",
    );
    f.no_writes();
}
#[test]
fn push_markers_are_checked_at_head_not_in_uncommitted_text() {
    for marker in ["<<<<<<< branch", "<<<<<<<", ">>>>>>> branch", ">>>>>>>"] {
        let mut f = ActionFixture::new();
        std::fs::write(f.path().join("source.txt"), format!("{marker}\n")).unwrap();
        f.git(&["add", "source.txt"]);
        f.record();
        std::fs::write(f.path().join("source.txt"), "fixed locally\n").unwrap();
        let value = f.run(
            "push",
            &["7", "--repo", "o/r", "--lease", &f.head],
            2,
            "markers",
        );
        assert!(value["reason"].as_str().unwrap().contains("source.txt"));
        f.no_writes();
    }
    let mut f = ActionFixture::new();
    let lease = f.head.clone();
    std::fs::write(f.path().join("source.txt"), "Heading\n=======\n").unwrap();
    f.git(&["add", "source.txt"]);
    f.record();
    std::fs::write(f.path().join("source.txt"), "<<<<<<< uncommitted\n").unwrap();
    f.run(
        "push",
        &["7", "--repo", "o/r", "--lease", &lease],
        0,
        "pushed",
    );
}
#[test]
fn push_requires_pre_push_and_stops_at_first_failed_vector() {
    let mut f = ActionFixture::new();
    f.declaration(json!({"default_branch":"trunk"}), false);
    f.record();
    assert!(
        f.run(
            "push",
            &["7", "--repo", "o/r", "--lease", &f.head],
            2,
            "refused"
        )["reason"]
            .as_str()
            .unwrap()
            .contains("landing.pre_push")
    );
    f.no_writes();
    let mut f = ActionFixture::new();
    f.declaration(
        json!({"default_branch":"trunk","pre_push":[["false"],["touch","should-not-exist"]]}),
        false,
    );
    f.record();
    f.change_pr(
        "autoMergeRequest",
        json!({"enabledAt":"2026-10-27T10:00:00Z"}),
    );
    let value = f.run(
        "push",
        &["7", "--repo", "o/r", "--lease", &f.head],
        2,
        "pre-push-failed",
    );
    assert!(value["reason"].as_str().unwrap().contains("exit 1"));
    assert!(!f.path().join("should-not-exist").exists());
    f.no_writes();
}
#[test]
fn landing_reads_and_disarm_failures_are_failed_without_push() {
    for verb in ["arm", "push"] {
        let f = ActionFixture::new();
        std::fs::write(f.path().join("bin/failure"), "gh: HTTP 401").unwrap();
        let args = if verb == "arm" {
            vec!["7", "--repo", "o/r"]
        } else {
            vec!["7", "--repo", "o/r", "--lease", "abc"]
        };
        let value = f.run(verb, &args, 4, "failed");
        assert!(value["reason"].as_str().unwrap().contains("401"));
        assert!(!value.to_string().contains("PRIVATE"));
        f.no_writes();
    }
    let f = ActionFixture::new();
    f.change_pr(
        "autoMergeRequest",
        json!({"enabledAt":"2026-10-27T10:00:00Z"}),
    );
    std::fs::write(f.path().join("bin/merge-failure"), "gh: HTTP 403").unwrap();
    assert_eq!(
        f.run(
            "push",
            &["7", "--repo", "o/r", "--lease", &f.head],
            4,
            "failed"
        )["disarmed"],
        false
    );
    assert!(!f.calls().contains("git push"));
}

#[test]
fn push_preflight_uses_committed_policy_at_worktree_root_without_a_shell() {
    let mut f = ActionFixture::new();
    let lease = f.head.clone();
    f.declaration(json!({"default_branch":"trunk","pre_push":[["pwd"],["printf","%s\n","$(touch shell-was-used)"]]}),false);
    f.record();
    // An uncommitted edit cannot remove the committed guards.
    f.declaration(json!({"default_branch":"trunk","pre_push":[]}), false);
    std::fs::create_dir(f.path().join("nested")).unwrap();
    let output = f
        .command("push", &["7", "--repo", "o/r", "--lease", &lease])
        .current_dir(f.path().join("nested"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(f.git(&["rev-parse", "--show-toplevel"]).as_str()),
        "{stderr}"
    );
    assert!(stderr.contains("$(touch shell-was-used)"));
    assert!(!f.path().join("shell-was-used").exists());
    f.answer("push", output, 0, "pushed");
}
#[test]
fn push_transport_failure_is_not_a_lease_refusal() {
    let f = ActionFixture::new();
    f.git(&["remote", "set-url", "origin", "missing-remote.git"]);
    let value = f.run(
        "push",
        &["7", "--repo", "o/r", "--lease", &f.head],
        4,
        "failed",
    );
    assert_eq!(value["disarmed"], false);
    let git = f.git_calls();
    assert!(git.contains("push"), "{git}");
    assert!(!git.contains("ls-remote"), "{git}");
}
#[test]
fn arm_uses_suite_time_when_force_push_history_cannot_be_read() {
    let f = ActionFixture::new();
    f.change_pr(
        "autoMergeRequest",
        json!({"enabledAt":"2026-10-27T11:00:00Z"}),
    );
    f.write("timeline", json!({"errors":[{"message":"PRIVATE BODY"}]}));
    f.run("arm", &["7", "--repo", "o/r"], 0, "already-armed");
    f.write("suites", json!({}));
    let value = f.run("arm", &["7", "--repo", "o/r"], 2, "armed-before-head");
    assert!(
        value["reason"]
            .as_str()
            .unwrap()
            .contains("cannot establish")
    );
    assert!(!value.to_string().contains("PRIVATE"));
    f.no_writes();
}
#[test]
fn landing_acts_answer_help_like_every_other_verb() {
    for verb in ["arm", "push"] {
        let output = Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(["land", verb, "--help"])
            .env("STATECRAFT_RUN_ID", "run-1")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "land {verb} --help");
        assert!(String::from_utf8_lossy(&output.stdout).contains(&format!("land {verb}")));
    }
}
