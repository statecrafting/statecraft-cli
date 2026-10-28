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
     *'pullRequests(states:OPEN'*)
       case "$4" in *'after:"next"'*) cat "$here/open-next.json" ;; *) cat "$here/open.json" ;; esac ;;
     *timelineItems*) cat "$here/timeline.json" ;;
     *statusCheckRollup*)
       case "$4" in *'after:"next"'*) cat "$here/checks-next.json" ;; *) cat "$here/checks.json" ;; esac ;;
     *'signature { isValid }'*) cat "$here/signature.json" ;;
     *viewerDefaultMergeMethod*) cat "$here/method.json" ;;
     *pullRequest*) cat "$here/pr.json" ;;
     *) exit 93 ;;
   esac ;;
 'api -X')
   if [ "$3" = POST ]; then
     case "$4" in
       */force-cancel)
         id="${4%/force-cancel}"; id="${id##*/}"
         if [ -f "$here/cancel-failure-$id" ]; then
           cat "$here/cancel-failure-$id" >&2; echo 'PRIVATE BODY HTTP 404'; exit 1
         fi
         exit 0 ;;
       *) exit 94 ;;
     esac
   fi
   [ "$3" = GET ] || exit 94
   case "$4" in
     *pending_deployments*)
       id="${4%/pending_deployments}"; id="${id##*/}"
       if [ -f "$here/pending-failure-$id" ]; then cat "$here/pending-failure-$id" >&2; exit 1; fi
       if [ -f "$here/pending-$id.json" ]; then cat "$here/pending-$id.json"; else cat "$here/pending.json"; fi ;;
     *'actions/runs?status=waiting'*) cat "$here/waiting.json" ;;
     *'/files?'*) cat "$here/files.json" ;;
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
    for verb in ["arm", "push", "restack", "cancel-superseded"] {
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
    for verb in ["arm", "push", "restack", "cancel-superseded"] {
        let output = Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(["land", verb, "--help"])
            .env("STATECRAFT_RUN_ID", "run-1")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "land {verb} --help");
        assert!(String::from_utf8_lossy(&output.stdout).contains(&format!("land {verb}")));
    }
}

const RESTACK_GH: &str = r#"#!/bin/sh
here="$(dirname "$0")"
printf 'gh %s\n' "$*" >> "$here/calls"
case "$1 $2" in
 'pr merge')
   [ "$5" = 'o/r' ] || exit 91
   [ "$6" = '--disable-auto' ] || exit 92
   touch "$here/disarmed" ;;
 'pr edit')
   [ ! -f "$here/edit-failure" ] || { echo 'gh: HTTP 403' >&2; exit 1; } ;;
 'api graphql')
   case "$4" in
     *'pullRequests(headRefName:"lower"'*) cat "$here/merged.json" ;;
     *'pullRequests(headRefName:'*) echo '{"data":{"repository":{"pullRequests":{"nodes":[]}}}}' ;;
     *'pullRequest(number:6)'*)
       case "$4" in *'commits(first:100'*) cat "$here/6-commits.json" ;; *) cat "$here/6-pr.json" ;; esac ;;
     *'pullRequest(number:7)'*)
       case "$4" in
         *'commits(first:100,after:"next"'*) cat "$here/7-next.json" ;;
         *'commits(first:100'*) cat "$here/7-commits.json" ;;
         *) cat "$here/7-pr.json" ;;
       esac ;;
     *) exit 93 ;;
   esac ;;
 *) exit 94 ;;
esac
"#;
struct RestackFixture {
    action: ActionFixture,
    original: String,
    lower: String,
    commits: Vec<String>,
}
impl RestackFixture {
    fn new(source_conflict: bool, signed: bool, merged: bool, second: bool) -> Self {
        let f = ActionFixture::new();
        std::fs::write(
            f.path().join(".git/info/exclude"),
            "bin/\nremote.git/\nno-home/\n",
        )
        .unwrap();
        f.declaration(json!({"default_branch":"trunk","generated":[{"paths":[".statecraft/derived/**"],"regenerate":["./bin/regenerate"]}],"pre_push":[["./bin/preflight"]]}), signed);
        std::fs::create_dir_all(f.path().join(".statecraft/derived")).unwrap();
        std::fs::write(
            f.path().join(".statecraft/derived/schema"),
            "original schema\n",
        )
        .unwrap();
        std::fs::write(f.path().join("source.txt"), "original source\n").unwrap();
        f.git(&["add", "-A"]);
        f.git(&["commit", "-m", "base"]);
        f.git(&["branch", "trunk"]);
        f.git(&["switch", "-c", "lower"]);
        std::fs::write(f.path().join("lower.txt"), "lower change\n").unwrap();
        f.git(&["add", "lower.txt"]);
        f.git(&["commit", "-m", "lower"]);
        let lower = f.git(&["rev-parse", "HEAD"]);
        f.git(&["switch", "-C", "topic", "lower"]);
        std::fs::write(
            f.path().join(".statecraft/derived/schema"),
            "upper schema\n",
        )
        .unwrap();
        std::fs::write(f.path().join("source.txt"), "upper source\n").unwrap();
        f.git(&["add", "-A"]);
        f.git(&[
            "commit",
            "-m",
            "upper one",
            "--author",
            "Original Author <author@example.invalid>",
        ]);
        let mut commits = vec![f.git(&["rev-parse", "HEAD"])];
        if second {
            std::fs::write(f.path().join("second.txt"), "second authored change\n").unwrap();
            f.git(&["add", "second.txt"]);
            f.git(&["commit", "-m", "upper two"]);
            commits.push(f.git(&["rev-parse", "HEAD"]));
        }
        let original = f.git(&["rev-parse", "HEAD"]);
        f.git(&["switch", "trunk"]);
        if merged {
            f.git(&["merge", "--no-ff", "lower", "-m", "merge lower"]);
        }
        std::fs::write(f.path().join(".statecraft/derived/schema"), "base schema\n").unwrap();
        if source_conflict {
            std::fs::write(f.path().join("source.txt"), "base source\n").unwrap();
        }
        f.git(&["add", "-A"]);
        f.git(&["commit", "-m", "advance base"]);
        // Seed all remote refs without networking, then restore the caller branch.
        f.git(&[
            "--git-dir=remote.git",
            "fetch",
            ".",
            "+refs/heads/*:refs/heads/*",
        ]);
        f.git(&["switch", "topic"]);
        statecraft_adapter::fixture::install_script(&f.path().join("bin/gh"), RESTACK_GH, 0o755)
            .unwrap();
        statecraft_adapter::fixture::install_script(
            &f.path().join("bin/regenerate"),
            r#"#!/bin/sh
cat .statecraft/derived/schema >> bin/base-copies
printf 'deterministic schema\n' > .statecraft/derived/schema
printf 'regenerate\n' >> bin/order
"#,
            0o755,
        )
        .unwrap();
        statecraft_adapter::fixture::install_script(
            &f.path().join("bin/preflight"),
            r#"#!/bin/sh
printf 'check\n' >> bin/order
[ ! -f bin/fail-second ] || [ ! -f second.txt ] || exit 17
"#,
            0o755,
        )
        .unwrap();
        let real_git =
            String::from_utf8(Command::new("which").arg("git").output().unwrap().stdout).unwrap();
        let shim = format!(
            r#"#!/bin/sh
here="$(dirname "$0")"
printf '%s\n' "$*" >> "$here/git-calls"
case "$1" in
 push)
   printf 'git %s\n' "$*" >> "$here/calls"
   if [ -f "$here/reject-lease" ]; then
     '{}' --git-dir=remote.git update-ref refs/heads/topic refs/heads/trunk || exit 95
   fi ;;
esac
# Log the required signing argument, but never use an operator's signing key.
case "$*" in
 '-c core.editor=true commit -C '*' --allow-empty -S')
   exec '{}' -c core.editor=true -c commit.gpgsign=false commit -C "$5" --allow-empty ;;
esac
exec '{}' "$@"
"#,
            real_git.trim(),
            real_git.trim(),
            real_git.trim()
        );
        statecraft_adapter::fixture::install_script(&f.path().join("bin/git"), &shim, 0o755)
            .unwrap();
        let fixture = Self {
            action: f,
            original,
            lower,
            commits,
        };
        fixture.pr(7, "topic", "lower", &fixture.original);
        fixture.pr(6, "lower", "trunk", &fixture.lower);
        fixture.list(7, &fixture.commits, false);
        fixture.list(6, std::slice::from_ref(&fixture.lower), false);
        fixture.action.write("merged", json!({"data":{"repository":{"pullRequests":{"nodes":if merged { vec![json!({"number":6})] } else { vec![] }}}}}));
        assert!(fixture.action.git(&["status", "--porcelain"]).is_empty());
        fixture
    }
    fn pr(&self, number: u64, branch: &str, base: &str, head: &str) {
        self.action.write(&format!("{number}-pr"), json!({"data":{"repository":{"pullRequest":{"state":"OPEN","isDraft":false,"baseRefName":base,"headRefName":branch,"headRefOid":head,"headRepository":{"nameWithOwner":"o/r"},"autoMergeRequest":{"enabledAt":"2026-10-27T10:00:00Z"},"mergeQueueEntry":null}}}}));
    }
    fn list(&self, number: u64, commits: &[String], more: bool) {
        self.action.write(&format!("{number}-commits"), json!({"data":{"repository":{"pullRequest":{"commits":{"nodes":commits.iter().map(|oid| json!({"commit":{"oid":oid}})).collect::<Vec<_>>(),"pageInfo":{"hasNextPage":more,"endCursor":if more { json!("next") } else { Value::Null }}}}}}}));
    }
    fn run(&self, code: i32, outcome: &str) -> Value {
        self.action
            .run("restack", &["7", "--repo", "o/r"], code, outcome)
    }
    fn restored(&self) {
        assert_eq!(self.action.git(&["branch", "--show-current"]), "topic");
        assert_eq!(self.action.git(&["rev-parse", "HEAD"]), self.original);
        assert_eq!(
            self.action
                .git(&["--git-dir=remote.git", "rev-parse", "topic"]),
            self.original
        );
        assert!(self.action.git(&["status", "--porcelain"]).is_empty());
        assert!(!self.action.calls().contains("git push"));
        assert!(!self.action.calls().contains("pr merge"));
    }
    fn script(&self, name: &str, body: &str) {
        statecraft_adapter::fixture::install_script(
            &self.action.path().join(format!("bin/{name}")),
            body,
            0o755,
        )
        .unwrap();
    }
}
#[test]
fn restack_mixed_conflicts_name_authored_path_and_restore_without_push() {
    let f = RestackFixture::new(true, false, true, false);
    let answer = f.run(1, "conflict");
    assert_eq!(answer["conflicts"], json!(["source.txt"]));
    f.restored();
    assert!(!f.action.path().join("bin/order").exists());
}
#[test]
fn restack_generated_conflict_regenerates_signs_pushes_with_lease_and_retargets() {
    let f = RestackFixture::new(false, true, true, false);
    let answer = f.run(0, "restacked");
    assert_eq!(answer["originalHead"], f.original);
    assert_eq!(answer["onto"], "trunk");
    assert_eq!(answer["replayed"], 1);
    assert_eq!(answer["regenerated"], json!([".statecraft/derived/schema"]));
    assert_eq!(answer["disarmed"], true);
    assert_eq!(answer["retargeted"], true);
    assert_eq!(
        std::fs::read_to_string(f.action.path().join("bin/base-copies")).unwrap(),
        "base schema\n"
    );
    assert_eq!(
        std::fs::read_to_string(f.action.path().join("bin/order")).unwrap(),
        "regenerate\ncheck\n"
    );
    assert_eq!(
        f.action.git(&["show", "HEAD:.statecraft/derived/schema"]),
        "deterministic schema"
    );
    assert_eq!(
        f.action.git(&["log", "-1", "--format=%an <%ae>%n%B"]),
        "Original Author <author@example.invalid>\nupper one"
    );
    let calls = f.action.calls();
    assert!(calls.contains(&format!("--force-with-lease=topic:{}", f.original)));
    assert!(calls.contains("pr edit 7 --repo o/r --base trunk"));
    assert!(calls.find("--disable-auto").unwrap() < calls.find("git push").unwrap());
    assert!(!calls.contains("--auto"));
    assert!(
        f.action
            .git_calls()
            .contains(&format!("commit -C {} --allow-empty -S", f.original))
    );
    assert_eq!(
        f.action
            .git(&["--git-dir=remote.git", "rev-parse", "topic"]),
        answer["head"].as_str().unwrap()
    );
    assert!(f.action.git(&["status", "--porcelain"]).is_empty());
}
#[test]
fn restack_authored_regenerator_edit_stays_local_for_inspection() {
    let f = RestackFixture::new(false, false, true, false);
    f.script("regenerate", "#!/bin/sh\nprintf 'regenerated\n' > .statecraft/derived/schema\nprintf 'unwanted authored edit\n' >> source.txt\n");
    let answer = f.run(1, "authored-change-differs");
    assert_eq!(answer["differs"], json!(["source.txt"]));
    assert_eq!(answer["originalHead"], f.original);
    assert_ne!(f.action.git(&["rev-parse", "HEAD"]), f.original);
    assert_eq!(f.action.git(&["branch", "--show-current"]), "topic");
    assert_eq!(
        f.action
            .git(&["--git-dir=remote.git", "rev-parse", "topic"]),
        f.original
    );
    assert!(!f.action.calls().contains("git push"));
    assert!(!f.action.calls().contains("pr merge"));
}
#[test]
fn restack_preflight_at_each_paginated_commit_restores_on_second_failure() {
    let f = RestackFixture::new(false, false, true, true);
    f.list(7, &f.commits[..1], true);
    f.action.write("7-next", json!({"data":{"repository":{"pullRequest":{"commits":{"nodes":[{"commit":{"oid":f.commits[1]}}],"pageInfo":{"hasNextPage":false}}}}}}));
    std::fs::write(f.action.path().join("bin/fail-second"), "").unwrap();
    let answer = f.run(2, "pre-push-failed");
    assert_eq!(answer["replayed"], 1);
    assert!(answer["reason"].as_str().unwrap().contains(&f.commits[1]));
    assert!(answer["reason"].as_str().unwrap().contains("exit 17"));
    assert!(
        answer["reason"]
            .as_str()
            .unwrap()
            .contains("./bin/preflight")
    );
    assert_eq!(
        std::fs::read_to_string(f.action.path().join("bin/order")).unwrap(),
        "regenerate\ncheck\nregenerate\ncheck\n"
    );
    assert!(f.action.calls().contains("after:\"next\""));
    f.restored();
}
#[test]
fn restack_failure_and_markers_restore_including_new_untracked_files() {
    for (script, code, outcome) in [
        ("#!/bin/sh\ntouch stray-file\nexit 19\n", 4, "failed"),
        (
            "#!/bin/sh\nprintf '<<<<<<< unresolved\n' > .statecraft/derived/schema\n",
            2,
            "markers",
        ),
    ] {
        let f = RestackFixture::new(false, false, true, false);
        f.script("regenerate", script);
        let answer = f.run(code, outcome);
        if code == 4 {
            assert!(answer["reason"].as_str().unwrap().contains("regenerator"));
        }
        f.restored();
        assert!(!f.action.path().join("stray-file").exists());
    }
}
#[test]
fn restack_local_preconditions_and_missing_policy_never_write() {
    for case in [
        "dirty",
        "tracked",
        "wrong-branch",
        "fork",
        "stale",
        "generated",
        "pre_push",
        "malformed-pre_push",
    ] {
        let f = RestackFixture::new(false, false, true, false);
        let mut expected = case;
        match case {
            "dirty" => std::fs::write(f.action.path().join("untracked"), "dirty").unwrap(),
            "tracked" => {
                std::fs::write(f.action.path().join("source.txt"), "dirty").unwrap();
                expected = "dirty";
            }
            "wrong-branch" => {
                f.action.git(&["switch", "lower"]);
            }
            "fork" => {
                let path = f.action.path().join("bin/7-pr.json");
                let mut value: Value =
                    serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                value["data"]["repository"]["pullRequest"]["headRepository"]["nameWithOwner"] =
                    json!("fork/r");
                std::fs::write(path, value.to_string()).unwrap();
                expected = "wrong-branch";
            }
            "stale" => {
                f.action.git(&[
                    "commit",
                    "--allow-empty",
                    "-m",
                    "local extra not listed by GitHub",
                ]);
            }
            "malformed-pre_push" => {
                let landing = json!({"default_branch":"trunk","generated":[],"pre_push":[[]]});
                f.action.declaration(landing, false);
                f.action.git(&["add", ".statecraft/environment.json"]);
                f.action.git(&["commit", "-m", "malformed policy"]);
                expected = "refused";
            }
            key => {
                let mut landing = json!({"default_branch":"trunk","generated":[],"pre_push":[]});
                landing.as_object_mut().unwrap().remove(key);
                f.action.declaration(landing, false);
                f.action.git(&["add", ".statecraft/environment.json"]);
                f.action.git(&["commit", "-m", "missing policy"]);
                expected = "refused";
            }
        }
        let answer = f.run(2, expected);
        if case == "stale" {
            assert!(answer["reason"].as_str().unwrap().contains(&f.original));
            assert!(
                answer["reason"]
                    .as_str()
                    .unwrap()
                    .contains(&f.action.git(&["rev-parse", "HEAD"]))
            );
        }
        if expected == "refused" {
            assert!(answer["reason"].as_str().unwrap().contains(&format!(
                "landing.{}",
                case.trim_start_matches("malformed-")
            )));
        }
        f.action.no_writes();
        let calls = f.action.git_calls();
        for write in ["fetch", "cherry-pick", "switch", "reset", "commit", "push"] {
            assert!(
                !calls.lines().any(|line| line.starts_with(write)),
                "{calls}"
            );
        }
    }
}
#[test]
fn restack_lease_refusal_disarms_and_reports_moved_remote_head() {
    let f = RestackFixture::new(false, false, true, false);
    std::fs::write(f.action.path().join("bin/reject-lease"), "").unwrap();
    let answer = f.run(2, "lease-refused");
    assert_eq!(answer["remoteHead"], f.action.git(&["rev-parse", "trunk"]));
    assert_eq!(answer["disarmed"], true);
    assert_eq!(answer["retargeted"], false);
    assert!(f.action.path().join("bin/disarmed").exists());
    assert!(!f.action.calls().contains("--auto"));
    assert!(!f.action.calls().contains("pr edit"));
}
#[test]
fn restack_chain_is_bottom_first_returns_to_start_and_stops_on_failure() {
    for fail in [false, true] {
        let f = RestackFixture::new(false, false, false, false);
        if fail {
            f.script("preflight", "#!/bin/sh\nexit 21\n");
        }
        let answer = f.action.run(
            "restack",
            &["--chain", "6", "7", "--repo", "o/r"],
            if fail { 2 } else { 0 },
            if fail { "pre-push-failed" } else { "restacked" },
        );
        let members = answer["restacks"].as_array().unwrap();
        assert_eq!(members.len(), if fail { 1 } else { 2 });
        assert_eq!(members[0]["pullRequest"], 6);
        assert_eq!(members[0]["onto"], "trunk");
        if fail {
            assert!(!f.action.calls().contains("pullRequest(number:7)"));
            assert!(!f.action.calls().contains("git push"));
        } else {
            assert_eq!(members[1]["pullRequest"], 7);
            assert_eq!(members[1]["onto"], "lower");
            assert_eq!(members[1]["retargeted"], false);
            let calls = f.action.calls();
            assert!(
                calls.find("--force-with-lease=lower:").unwrap()
                    < calls.find("--force-with-lease=topic:").unwrap()
            );
        }
        assert_eq!(f.action.git(&["branch", "--show-current"]), "topic");
        assert!(f.action.git(&["status", "--porcelain"]).is_empty());
    }
}
#[test]
fn restack_explicit_onto_and_unmerged_base_do_not_use_default_branch() {
    for explicit in [false, true] {
        let f = RestackFixture::new(false, false, !explicit, false);
        let args = if explicit {
            vec!["7", "--repo", "o/r", "--onto", "lower"]
        } else {
            vec!["7", "--repo", "o/r"]
        };
        if !explicit {
            f.action.write(
                "merged",
                json!({"data":{"repository":{"pullRequests":{"nodes":[]}}}}),
            );
        }
        let answer = f.action.run("restack", &args, 0, "restacked");
        assert_eq!(answer["onto"], "lower");
        assert_eq!(answer["retargeted"], false);
        assert!(!f.action.git_calls().contains("fetch origin trunk"));
    }
}

#[test]
fn restack_rejects_incomplete_or_unavailable_host_commit_lists_before_replay() {
    for missing in [false, true] {
        let f = RestackFixture::new(false, false, true, false);
        if missing {
            f.list(7, &["f".repeat(40), f.original.clone()], false);
        } else {
            f.list(7, std::slice::from_ref(&f.lower), false);
        }
        f.run(2, "stale");
        f.restored();
        assert!(!f.action.git_calls().contains("cherry-pick"));
    }
}
#[test]
fn restack_chain_missing_local_member_restores_starting_branch() {
    let f = RestackFixture::new(false, false, false, false);
    f.pr(6, "absent-local-branch", "trunk", &f.lower);
    let answer = f.action.run(
        "restack",
        &["--chain", "6", "7", "--repo", "o/r"],
        2,
        "wrong-branch",
    );
    assert_eq!(answer["restacks"].as_array().unwrap().len(), 1);
    f.restored();
}
#[test]
fn restack_usage_precedes_policy_and_reads() {
    let f = ActionFixture::new();
    for args in [
        vec![],
        vec!["--chain", "7"],
        vec!["7", "8"],
        vec!["--chain", "7", "8", "--onto", "trunk"],
        vec!["7", "--repo", "bad"],
    ] {
        f.run("restack", &args, 3, "usage");
    }
    assert!(f.calls().is_empty());
    assert!(f.git_calls().is_empty());
    let answer = f.run("restack", &["7", "--repo", "o/r"], 2, "refused");
    assert!(
        answer["reason"]
            .as_str()
            .unwrap()
            .contains("landing.generated")
    );
    assert_eq!(answer["pullRequest"], 7);
}
#[test]
fn restack_retarget_failure_keeps_published_head_and_disarm_evidence() {
    let f = RestackFixture::new(false, false, true, false);
    std::fs::write(f.action.path().join("bin/edit-failure"), "").unwrap();
    let answer = f.run(4, "failed");
    assert_eq!(answer["disarmed"], true);
    assert_eq!(answer["retargeted"], false);
    assert!(answer["reason"].as_str().unwrap().contains("403"));
    assert_eq!(
        answer["head"],
        f.action
            .git(&["--git-dir=remote.git", "rev-parse", "topic"])
    );
    assert_ne!(answer["head"], f.original);
    assert_eq!(f.action.git(&["branch", "--show-current"]), "topic");
}
#[test]
fn restack_from_nested_directory_uses_root_and_all_regenerators_in_order() {
    let f = RestackFixture::new(false, false, true, true);
    // An ignored existing directory is retained across detached replay.
    std::fs::create_dir(f.action.path().join("bin/nested")).unwrap();
    let output = f
        .action
        .command("restack", &["7", "--repo", "o/r"])
        .current_dir(f.action.path().join("bin/nested"))
        .output()
        .unwrap();
    let answer = f.action.answer("restack", output, 0, "restacked");
    assert_eq!(answer["replayed"], 2);
    assert_eq!(
        std::fs::read_to_string(f.action.path().join("bin/order")).unwrap(),
        "regenerate\ncheck\nregenerate\ncheck\n"
    );
    assert_eq!(
        f.action.git(&["log", "-2", "--format=%s"]),
        "upper two\nupper one"
    );
}

// The reads every landing report and act shares: a committed policy, one
// open pull request at the fixture's head, and no waiting run.
fn landing_fixture() -> ActionFixture {
    let mut f = ActionFixture::new();
    f.declaration(json!({"default_branch":"trunk"}), false);
    f.record();
    f.write("open",json!({"data":{"repository":{"pullRequests":{"nodes":[open_pr(&f,7,"trunk")],"pageInfo":{"hasNextPage":false}}}}}));
    f.write("waiting", json!({"workflow_runs":[]}));
    f.write(
        "pending",
        json!([{"environment":{"name":"statecraft-review-exception"}}]),
    );
    f
}
fn open_pr(f: &ActionFixture, number: u64, base: &str) -> Value {
    json!({"number":number,"title":"spec(041): test","url":"https://example.invalid/pr","createdAt":"2026-10-20T00:00:00Z","isDraft":false,"baseRefName":base,"headRefName":format!("p{number}"),"headRefOid":f.head,"headRepository":{"nameWithOwner":"o/r"},"mergeStateStatus":"CLEAN","autoMergeRequest":null,"mergeQueueEntry":null})
}

fn cancel_run(id: u64, head: &str) -> Value {
    json!({"id":id,"event":"pull_request","head_sha":head,"html_url":format!("https://example.invalid/run/{id}"),"created_at":"2026-10-27T00:00:00Z"})
}
fn assert_cancel_reads_only(f: &ActionFixture, posts: &[u64]) {
    let calls = f.calls();
    let writes: Vec<_> = calls
        .lines()
        .filter(|l| !l.starts_with("gh api graphql ") && !l.starts_with("gh api -X GET "))
        .collect();
    let expected: Vec<_> = posts
        .iter()
        .map(|id| format!("gh api -X POST repos/o/r/actions/runs/{id}/force-cancel"))
        .collect();
    assert_eq!(writes, expected);
    assert_eq!(f.git_calls(), "show HEAD:.statecraft/environment.json\n");
}
#[test]
fn cancel_usage_policy_and_missing_committed_declaration() {
    let mut f = landing_fixture();
    for args in [
        vec!["7"],
        vec!["--unknown"],
        vec!["--repo"],
        vec!["--repo", "bad"],
        vec!["--repo", "o/r", "--repo", "o/r"],
    ] {
        f.run("cancel-superseded", &args, 3, "usage");
        assert!(f.calls().is_empty());
        assert!(f.git_calls().is_empty());
    }
    f.declaration(json!({}), false);
    f.record();
    let a = f.run("cancel-superseded", &["--repo", "o/r"], 2, "refused");
    assert!(a["reason"].as_str().unwrap().contains("default_branch"));
    assert!(f.calls().is_empty());
    // An uncommitted value cannot supply the missing key.
    f.declaration(json!({"default_branch":"trunk"}), false);
    f.run("cancel-superseded", &["--repo", "o/r"], 2, "refused");
    assert!(f.calls().is_empty());
    std::fs::remove_file(f.path().join(".statecraft/environment.json")).unwrap();
    f.record();
    let a = f.run("cancel-superseded", &["--repo", "o/r"], 2, "refused");
    assert!(
        a["reason"]
            .as_str()
            .unwrap()
            .contains("committed declaration")
    );
    assert!(f.calls().is_empty());
}
#[test]
fn cancel_filters_orders_and_writes_only_one_force_cancel_per_id() {
    let f = landing_fixture();
    f.write("open",json!({"data":{"repository":{"pullRequests":{"nodes":[],"pageInfo":{"hasNextPage":true,"endCursor":"next"}}}}}));
    f.write("open-next",json!({"data":{"repository":{"pullRequests":{"nodes":[open_pr(&f,7,"trunk")],"pageInfo":{"hasNextPage":false}}}}}));
    f.write("waiting",json!({"workflow_runs":[cancel_run(9,"old"),json!({"id":1,"event":"workflow_dispatch"}),cancel_run(4,&f.head),cancel_run(3,"old"),cancel_run(2,"old"),cancel_run(9,"old")]}));
    f.write("pending-2", json!([{"environment":{"name":"release"}}]));
    let a = f.run("cancel-superseded", &["--repo", "o/r"], 0, "cancelled");
    assert_eq!(a["exitCode"], 0);
    assert_eq!(a["kept"], 1);
    assert_eq!(
        a["cancelled"],
        json!([
            {"id":3,"headSha":"old","url":"https://example.invalid/run/3"},
            {"id":9,"headSha":"old","url":"https://example.invalid/run/9"}
        ])
    );
    assert_eq!(a["notCancelled"], json!([]));
    assert_eq!(
        a["policy"]["defaultBranch"],
        json!({"value":"trunk","layer":"project-override"})
    );
    let calls = f.calls();
    assert!(!calls.contains("runs/1/pending_deployments"));
    let last_read = calls.rfind("gh api -X GET ").unwrap();
    assert!(last_read < calls.find("gh api -X POST ").unwrap());
    assert_cancel_reads_only(&f, &[3, 9]);
}
#[test]
fn cancel_empty_is_success_and_failed_posts_do_not_stop_the_rest() {
    let f = landing_fixture();
    let a = f.run("cancel-superseded", &["--repo", "o/r"], 0, "cancelled");
    assert_eq!(a["cancelled"], json!([]));
    assert_eq!(a["kept"], 0);
    assert_cancel_reads_only(&f, &[]);
    let f = landing_fixture();
    f.write(
        "waiting",
        json!({"workflow_runs":[cancel_run(9,"old"),cancel_run(2,"old"),cancel_run(5,"old")]}),
    );
    std::fs::write(f.path().join("bin/cancel-failure-2"), "gh: HTTP 403").unwrap();
    let a = f.run("cancel-superseded", &["--repo", "o/r"], 4, "failed");
    assert_eq!(a["exitCode"], 4);
    assert!(a["reason"].as_str().unwrap().contains("1 run(s)"));
    assert_eq!(a["notCancelled"][0]["id"], 2);
    assert!(
        a["notCancelled"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("403")
    );
    assert_eq!(a["cancelled"][0]["id"], 5);
    assert_eq!(a["cancelled"][1]["id"], 9);
    assert!(!a.to_string().contains("PRIVATE"));
    assert_cancel_reads_only(&f, &[2, 5, 9]);
}
#[test]
fn cancel_failed_reads_prevent_all_posts_even_after_an_eligible_run() {
    for failed in ["open", "waiting", "pending"] {
        let f = landing_fixture();
        f.write(
            "waiting",
            json!({"workflow_runs":[cancel_run(2,"old"),cancel_run(9,"old")]}),
        );
        match failed {
            "open" => f.write("open", json!({})),
            "waiting" => f.write("waiting", json!({})),
            _ => std::fs::write(f.path().join("bin/pending-failure-9"), "gh: HTTP 429").unwrap(),
        }
        let a = f.run("cancel-superseded", &["--repo", "o/r"], 4, "failed");
        assert_eq!(a["cancelled"], json!([]));
        assert_eq!(a["notCancelled"], json!([]));
        assert_cancel_reads_only(&f, &[]);
    }
}

fn status_fixture() -> ActionFixture {
    let mut f = landing_fixture();
    f.declaration(
        json!({"default_branch":"trunk","stack":{"depth":1,"depth_by_spec":{},"max_stacked":1}}),
        false,
    );
    f.record();
    // The record moved the head: the open pull request points at the new one.
    f.write("open",json!({"data":{"repository":{"pullRequests":{"nodes":[open_pr(&f,7,"trunk")],"pageInfo":{"hasNextPage":false}}}}}));
    f.review("no-findings");
    std::fs::create_dir(f.path().join(".bin")).unwrap();
    statecraft_adapter::fixture::install_script(&f.path().join(".bin/spec-spine"), "#!/bin/sh\n[ \"$*\" = 'registry plan --json' ] || exit 91\nprintf '%s' '{\"blocked\":[]}'\n",0o755).unwrap();
    // A PATH binary must not shadow the executable repository pin.
    statecraft_adapter::fixture::install_script(
        &f.path().join("bin/spec-spine"),
        "#!/bin/sh\nexit 92\n",
        0o755,
    )
    .unwrap();
    f.write("files", json!([]));
    f
}
#[test]
fn status_clean_read_ignores_attempt_and_never_writes() {
    let f = status_fixture();
    let before = f.git(&["rev-parse", "HEAD"]);
    let output = f
        .command("status", &["--repo", "o/r"])
        .env("STATECRAFT_RUN_ID", "")
        .output()
        .unwrap();
    let a = f.answer("status", output, 0, "read");
    assert_eq!(a["actions"][0]["action"], "arm");
    assert!(a["policy"]["generated"].is_null());
    assert_eq!(a["policy"]["stackDepth"]["value"], 1);
    assert_eq!(before, f.git(&["rev-parse", "HEAD"]));
    let calls = f.calls();
    for write in ["-X POST", "pr merge", "pr edit", "git push"] {
        assert!(!calls.contains(write), "{calls}");
    }
    assert!(!f.path().join("no-home").exists());
    for line in f.git_calls().lines() {
        assert!(
            line.starts_with("show HEAD:") || line == "rev-parse --show-toplevel",
            "{line}"
        );
    }
}
#[test]
fn status_usage_missing_keys_and_failed_reads() {
    let mut f = status_fixture();
    for args in [
        vec!["7"],
        vec!["--repo", "bad"],
        vec!["--cancel-superseded"],
        vec!["--repo"],
    ] {
        f.run("status", &args, 3, "usage");
    }
    assert!(f.calls().is_empty());
    for (landing, key) in [
        (json!({}), "default_branch"),
        (
            json!({"default_branch":"trunk","stack":{"depth_by_spec":{},"max_stacked":1}}),
            "stack.depth",
        ),
    ] {
        f.declaration(landing, false);
        f.record();
        let a = f.run("status", &["--repo", "o/r"], 2, "refused");
        assert!(a["reason"].as_str().unwrap().contains(key));
    }
    let f = status_fixture();
    std::fs::write(f.path().join("bin/failure"), "HTTP 403 forbidden").unwrap();
    let a = f.run("status", &["--repo", "o/r"], 4, "failed");
    assert!(a["reason"].as_str().unwrap().contains("403"));
    assert!(!a.to_string().contains("PRIVATE BODY"));
}
#[test]
fn status_041_chain_caps_pagination_and_superseded_runs() {
    let f = status_fixture();
    f.write("open",json!({"data":{"repository":{"pullRequests":{"nodes":[open_pr(&f,7,"trunk")],"pageInfo":{"hasNextPage":true,"endCursor":"next"}}}}}));
    f.write("open-next",json!({"data":{"repository":{"pullRequests":{"nodes":[open_pr(&f,8,"p7"),open_pr(&f,9,"p8")],"pageInfo":{"hasNextPage":false}}}}}));
    f.write("waiting",json!({"workflow_runs":[{"id":1,"event":"workflow_dispatch"},{"id":2,"event":"pull_request","head_sha":"old","html_url":"https://example.invalid/run/2","created_at":"2026-10-27T00:00:00Z"}]}));
    let a = f.run("status", &["--repo", "o/r"], 0, "read");
    let top = a["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["pullRequest"] == 9)
        .unwrap();
    assert_eq!(top["depth"], 2);
    assert_eq!(top["cap"], 1);
    assert_eq!(top["landFirst"], 7);
    assert_eq!(a["stack"]["landFirst"], json!([7]));
    assert_eq!(a["stack"]["overStacked"], true);
    assert_eq!(
        a["supersededRuns"],
        json!([{"id":2,"headSha":"old","url":"https://example.invalid/run/2"}])
    );
    let calls = f.calls();
    assert!(!calls.contains("runs/1/pending_deployments"));
    for write in ["-X POST", "pr merge", "pr edit"] {
        assert!(!calls.contains(write));
    }
}
#[test]
fn status_approval_reasons_and_findings() {
    let f = status_fixture();
    f.write("waiting",json!({"workflow_runs":[{"id":2,"event":"pull_request","head_sha":f.head,"html_url":"https://example.invalid/run/2","created_at":"2026-10-27T00:00:00Z"}]}));
    f.write(
        "files",
        json!([{"filename":".statecraft/setup/policy.json"},{"filename":"specs/041/spec.md"}]),
    );
    f.review("findings");
    let a = f.run("status", &["--repo", "o/r"], 0, "read");
    assert_eq!(a["actions"][0]["action"], "approve");
    assert_eq!(
        a["actions"][0]["reason"],
        "review findings: resolve it before approving, authority change, possible ratification (a spec.md changed)"
    );
    f.checks(vec![json!({"__typename":"CheckRun","isRequired":true,"name":"ci","status":"COMPLETED","conclusion":"SKIPPED"})]);
    let a = f.run("status", &["--repo", "o/r"], 1, "finding");
    assert_eq!(a["actions"][0]["action"], "fix");
    assert!(
        a["actions"][0]["detail"]
            .as_str()
            .unwrap()
            .contains("owner approval is also waiting")
    );
}
#[test]
fn status_generated_paths_are_findings_and_review_recognizes_forks() {
    let mut f = status_fixture();
    f.declaration(json!({"default_branch":"trunk","stack":{"depth":1,"depth_by_spec":{},"max_stacked":1},"generated":[{"paths":["derived/**","src/?.rs"],"regenerate":["generate"]}]}),false);
    let path = f.path().join(".statecraft/environment.json");
    let mut declaration: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    declaration["project"]["setup"]["parameters"]["review.exclude"] = json!(["derived/"]);
    std::fs::write(path, declaration.to_string()).unwrap();
    f.record();
    f.review("no-findings");
    let mut pr = open_pr(&f, 7, "trunk");
    pr["headRepository"]["nameWithOwner"] = json!("fork/r");
    f.write("open",json!({"data":{"repository":{"pullRequests":{"nodes":[pr],"pageInfo":{"hasNextPage":false}}}}}));
    let a = f.run("status", &["--repo", "o/r"], 1, "finding");
    assert_eq!(a["generatedReviewed"], json!(["src/?.rs"]));
    assert_eq!(a["actions"][0]["review"], "skipped:fork");
    assert_eq!(a["actions"][0]["action"], "fix");
    assert!(!f.calls().contains("check-runs"));
}
