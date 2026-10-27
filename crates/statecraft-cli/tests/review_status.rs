//! Spec 062 CLI contract, entirely through a read-only stub gh.
#[path = "support/json_naming.rs"]
mod json_naming;

use serde_json::{Value, json};
use std::process::Command;

const HEAD: &str = "abcdef0123456789";
const GH: &str = r#"#!/bin/sh
here="$(dirname "$0")"
printf '%s\n' "$*" >> "$here/calls"
[ "$1 $2 $3" = 'api -X GET' ] || exit 97
if [ -f "$here/failure" ]; then
  cat "$here/failure" >&2
  echo 'PRIVATE RESPONSE BODY HTTP 404'
  exit 1
fi
case "$4" in
  repos/o/r/pulls/*) cat "$here/pr.json" ;;
  *check-runs*page=2) cat "$here/checks2.json" ;;
  *check-runs*) cat "$here/checks.json" ;;
  *statecraft-ci.yml/runs*) cat "$here/runs.json" ;;
  */attempts/*/jobs*) cat "$here/jobs.json" ;;
  *) echo 'unexpected stub read' >&2; exit 96 ;;
esac
"#;
struct Fixture(tempfile::TempDir);
impl Fixture {
    fn new() -> Self {
        let f = Self(tempfile::tempdir().unwrap());
        statecraft_adapter::fixture::install_script(&f.0.path().join("gh"), GH, 0o755).unwrap();
        f.write(
            "pr",
            json!({"state":"open","head":{"sha":HEAD,"repo":{"full_name":"o/r"}}}),
        );
        f.checks(vec![]);
        f.write("runs", json!({"workflow_runs":[]}));
        f.write("jobs", json!({"jobs":[]}));
        f
    }
    fn write(&self, name: &str, value: Value) {
        std::fs::write(
            self.0.path().join(format!("{name}.json")),
            value.to_string(),
        )
        .unwrap();
    }
    fn checks(&self, checks: Vec<Value>) {
        self.write("checks", json!({"check_runs":checks}));
    }
    fn job(&self, status: &str, conclusion: Value) {
        self.write("runs", json!({"workflow_runs":[{"id":50,"run_attempt":2,"head_sha":HEAD,"status":status,"pull_requests":[{"number":7}]}]}));
        self.write("jobs", json!({"jobs":[{"id":51,"name":"ai-review / review","status":status,"conclusion":conclusion,"started_at":"2026-10-27T23:50:00Z","completed_at":if status == "completed" {json!("2026-10-28T00:10:00Z")} else {Value::Null}}]}));
    }
    fn invoke(&self, args: &[&str], json_output: bool) -> std::process::Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_statecraft-cli"));
        command.args(["review", "status"]).args(args);
        if json_output {
            command.arg("--json");
        }
        command
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
            .unwrap()
    }
    fn assert(&self, args: &[&str], exit: i32, outcome: &str) -> Value {
        let output = self.invoke(args, true);
        assert_eq!(output.status.code(), Some(exit), "{output:?}");
        let envelope = json_naming::from_output(&output.stdout).unwrap();
        assert_eq!(envelope["verb"], "review.status");
        assert_eq!(envelope["exitCode"], exit);
        if exit >= 2 {
            let kind = match exit {
                2 => "refused",
                3 => "usage",
                4 => "io",
                _ => panic!("unexpected exit code: {exit}"),
            };
            assert_eq!(envelope["error"]["kind"], kind);
        }
        let value = json_naming::payload(&envelope).clone();
        assert_eq!(value["outcome"], outcome);
        assert!(output.stderr.is_empty(), "{output:?}");
        value
    }
    fn calls(&self) -> String {
        std::fs::read_to_string(self.0.path().join("calls")).unwrap_or_default()
    }
}
fn check(id: u64, pr: u64, head: &str, result: &str) -> Value {
    let record = json!({"subject":{"pullRequest":pr,"head":head},"result":result,"findings":[]});
    json!({"id":id,"name":"statecraft-ai-review","html_url":format!("https://github.com/o/r/runs/{id}"),"output":{"text":format!("```json\n{record}\n```\n")}})
}
const ARGS: &[&str] = &["7", "--repo", "o/r"];

#[test]
fn rerun_supersedes_findings_with_no_findings() {
    let f = Fixture::new();
    f.checks(vec![
        check(20, 7, HEAD, "no-findings"),
        check(10, 7, HEAD, "findings"),
    ]);
    let v = f.assert(ARGS, 0, "no-findings");
    assert_eq!(v["checkRun"]["id"], 20);
    assert_eq!(v["record"]["result"], "no-findings");
    assert_eq!(v.as_object().unwrap().len(), 8);
}
#[test]
fn transient_verdict_still_counts_while_rerun_is_running() {
    let f = Fixture::new();
    f.checks(vec![check(10, 7, HEAD, "skipped:transient")]);
    f.job("in_progress", Value::Null);
    let v = f.assert(ARGS, 1, "skipped:transient");
    assert_eq!(v["run"]["attempt"], 2);
    assert!(v["run"]["reviewJobConclusion"].is_null());
}
#[test]
fn draft_has_skip_verdict() {
    let f = Fixture::new();
    f.checks(vec![check(10, 7, HEAD, "skipped:draft")]);
    f.assert(ARGS, 1, "skipped:draft");
}
#[test]
fn fork_needs_only_the_pr_read() {
    let f = Fixture::new();
    f.write(
        "pr",
        json!({"state":"open","head":{"sha":HEAD,"repo":{"full_name":"fork/r"}}}),
    );
    let v = f.assert(ARGS, 1, "skipped:fork");
    assert!(v["checkRun"].is_null());
    assert_eq!(f.calls().lines().count(), 1);
}
#[test]
fn cancelled_at_limit_reports_duration() {
    let f = Fixture::new();
    f.job("completed", json!("cancelled"));
    let v = f.assert(ARGS, 2, "absent");
    assert_eq!(v["reason"], "cancelled");
    assert_eq!(v["run"]["reviewJobDurationSeconds"], 1200);
}
#[test]
fn moved_head_with_expected_head_is_stale_without_older_reads() {
    let f = Fixture::new();
    let v = f.assert(&["7", "--repo", "o/r", "--head", "deadbeef"], 2, "stale");
    assert_eq!(v["head"], "deadbeef");
    assert_eq!(v["currentHead"], HEAD);
    assert_eq!(v["reason"], "head-moved");
    assert_eq!(f.calls().lines().count(), 1);
}
#[test]
fn moved_head_without_expected_head_reads_current_head() {
    let f = Fixture::new();
    f.job("in_progress", Value::Null);
    let v = f.assert(ARGS, 2, "pending");
    assert_eq!(v["head"], HEAD);
    assert!(f.calls().contains(&format!("commits/{HEAD}/check-runs")));
}
#[test]
fn shared_commit_counts_only_the_requested_pull_request() {
    let f = Fixture::new();
    f.checks(vec![
        check(10, 7, HEAD, "findings"),
        check(20, 8, HEAD, "no-findings"),
    ]);
    let v = f.assert(ARGS, 1, "findings");
    assert_eq!(v["checkRun"]["id"], 10);
    let v = f.assert(&["8", "--repo", "o/r"], 0, "no-findings");
    assert_eq!(v["checkRun"]["id"], 20);
}
#[test]
fn mismatched_head_without_running_job_is_absent() {
    let f = Fixture::new();
    f.checks(vec![check(10, 7, "deadbeef", "no-findings")]);
    assert_eq!(f.assert(ARGS, 2, "absent")["reason"], "mismatched-record");
}
#[test]
fn mismatched_head_with_running_job_is_pending() {
    let f = Fixture::new();
    f.checks(vec![check(10, 7, "deadbeef", "no-findings")]);
    f.job("in_progress", Value::Null);
    assert!(f.assert(ARGS, 2, "pending")["reason"].is_null());
}
#[test]
fn no_run_is_explicitly_absent() {
    let f = Fixture::new();
    assert_eq!(f.assert(ARGS, 2, "absent")["reason"], "no-run");
}
#[test]
fn unauthorized_uses_stderr_status_only_and_never_leaks_body_or_headers() {
    let f = Fixture::new();
    std::fs::write(f.0.path().join("failure"), "gh: HTTP 401\nPRIVATE HEADER\n").unwrap();
    let v = f.assert(ARGS, 4, "failed");
    assert_eq!(v["reason"], "unauthorized: HTTP 401");
    assert!(!v.to_string().contains("PRIVATE"));
    let output = f.invoke(ARGS, false);
    assert_eq!(output.status.code(), Some(4));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("PRIVATE"));
}
#[test]
fn failed_review_is_absent() {
    let f = Fixture::new();
    f.job("completed", json!("failure"));
    assert_eq!(f.assert(ARGS, 2, "absent")["reason"], "failed");
}
#[test]
fn counting_search_includes_later_pages_and_all_attempts() {
    let f = Fixture::new();
    f.checks((1..=100).map(|id| check(id, 8, HEAD, "findings")).collect());
    f.write(
        "checks2",
        json!({"check_runs":[check(101,7,HEAD,"no-findings")]}),
    );
    assert_eq!(f.assert(ARGS, 0, "no-findings")["checkRun"]["id"], 101);
    assert!(f.calls().contains("filter=all&per_page=100&page=2"));
}
#[test]
fn queued_run_with_no_job_yet_is_pending() {
    let f = Fixture::new();
    f.job("queued", Value::Null);
    f.write("jobs", json!({"jobs":[]}));
    f.assert(ARGS, 2, "pending");
}
#[test]
fn repository_defaults_to_origin() {
    let f = Fixture::new();
    // The stub git only supports the read used for origin discovery.
    statecraft_adapter::fixture::install_script(
        &f.0.path().join("git"),
        "#!/bin/sh\n[ \"$*\" = 'remote get-url origin' ] || exit 97\necho git@github.com:o/r.git\n",
        0o755,
    )
    .unwrap();
    f.assert(&["7"], 2, "absent");
    assert!(f.calls().contains("repos/o/r/pulls/7"));
}
#[test]
fn usage_is_rejected_without_host_reads() {
    for args in [
        &["0", "--repo", "o/r"][..],
        &["7", "--repo", "o/r?secret"],
        &["7", "--head"],
        &["7", "--repo", "o/r", "--repo", "o/r"],
    ] {
        let f = Fixture::new();
        f.assert(args, 3, "usage");
        assert!(f.calls().is_empty());
    }
}
#[test]
fn human_output_is_the_same_answer_one_line_per_set_field() {
    let f = Fixture::new();
    let expected = f.assert(ARGS, 2, "absent");
    let output = f.invoke(ARGS, false);
    assert_eq!(output.status.code(), Some(2));
    let text = String::from_utf8(output.stdout).unwrap();
    for (key, value) in expected.as_object().unwrap() {
        if !value.is_null() {
            assert!(text.contains(&format!(
                    "{key}: {}",
                    value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string())
                )));
        }
    }
    assert_eq!(text.lines().count(), 5);
}

#[test]
fn rate_limits_are_classified_without_response_details() {
    for error in ["gh: rate limit exceeded (HTTP 403)", "gh: HTTP 429"] {
        let f = Fixture::new();
        std::fs::write(f.0.path().join("failure"), error).unwrap();
        let value = f.assert(ARGS, 4, "failed");
        assert!(
            value["reason"]
                .as_str()
                .unwrap()
                .starts_with("rate limited: HTTP ")
        );
    }
}
#[test]
fn malformed_success_response_is_sanitized() {
    let f = Fixture::new();
    std::fs::write(f.0.path().join("pr.json"), "{\"PRIVATE BODY\": secret}").unwrap();
    let value = f.assert(ARGS, 4, "failed");
    assert_eq!(value["reason"], "unparseable gh answer");
}
