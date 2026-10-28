//! Explicit, head-pinned auto-merge with ordered refusals (spec 063 section 3.3).
use crate::{Attempt, UsedPolicy, action::*};
use serde::Serialize;
use serde_json::{Value, json};
use statecraft_home::{
    review_status,
    setup::{Host, HostError},
};
use std::path::Path;

/// One arm answer in either rendering.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    /// Stable result word.
    pub outcome: String,
    /// Requested pull request, if parsed.
    pub pull_request: Option<u64>,
    /// Observed head, if read.
    pub head: Option<String>,
    /// Explanation of a refusal or failure.
    pub reason: Option<String>,
    /// Landing values used, with provenance.
    pub policy: UsedPolicy,
    /// Family process exit code.
    #[serde(skip)]
    pub exit_code: i32,
}
impl Answer {
    fn end(mut self, word: &str, code: i32, reason: impl Into<Option<String>>) -> Self {
        self.outcome = word.into();
        self.exit_code = code;
        self.reason = reason.into();
        self
    }
}
// GitHub timestamps are canonical UTC. Unknown or malformed times fail closed.
fn timestamp(s: &str) -> Option<&str> {
    let b = s.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return None;
    }
    if !b
        .iter()
        .enumerate()
        .all(|(i, b)| [4, 7, 10, 13, 16, 19].contains(&i) || b.is_ascii_digit())
    {
        return None;
    }
    let year: u32 = s[..4].parse().ok()?;
    let month: usize = s[5..7].parse().ok()?;
    let day: u32 = s[8..10].parse().ok()?;
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if year == 0
        || !(1..=12).contains(&month)
        || day == 0
        || day > days[month - 1]
        || &s[11..13] > "23"
        || &s[14..16] > "59"
        || &s[17..19] > "59"
    {
        return None;
    }
    Some(s)
}
fn armed_outcome(enabled: &str, pushed: Option<&str>) -> (&'static str, i32, Option<String>) {
    match (timestamp(enabled), pushed.and_then(timestamp)) {
        (Some(armed), Some(head)) if armed > head => ("already-armed", 0, None),
        (Some(_), Some(_)) => (
            "armed-before-head",
            2,
            Some("arming did not occur after the current head was pushed".into()),
        ),
        _ => (
            "armed-before-head",
            2,
            Some("cannot establish the current head's push time and arming order".into()),
        ),
    }
}
pub(crate) fn next(connection: &Value) -> Result<Option<String>, HostError> {
    match connection["pageInfo"]["hasNextPage"].as_bool() {
        Some(false) => Ok(None),
        Some(true) => Ok(Some(string(&connection["pageInfo"], "endCursor")?.into())),
        None => Err(malformed()),
    }
}
pub(crate) fn after(cursor: &Option<String>) -> String {
    cursor
        .as_ref()
        .map(|c| format!(",after:{}", json!(c)))
        .unwrap_or_default()
}
fn push_time(
    remote: &Remote,
    repo: &str,
    pr: u64,
    head: &str,
) -> Result<Option<String>, HostError> {
    match force_push_time(remote, repo, pr, head) {
        Ok(Some(time)) => Ok(Some(time)),
        Ok(None) | Err(_) => suite_time(remote, repo, head),
    }
}
fn force_push_time(
    remote: &Remote,
    repo: &str,
    pr: u64,
    head: &str,
) -> Result<Option<String>, HostError> {
    let mut cursor = None;
    let mut newest = None;
    for _ in 0..100 {
        let value = remote.query(repo, &format!("pullRequest(number:{pr}) {{ timelineItems(first:100{},itemTypes:[HEAD_REF_FORCE_PUSHED_EVENT]) {{ nodes {{ ... on HeadRefForcePushedEvent {{ createdAt afterCommit {{ oid }} }} }} pageInfo {{ hasNextPage endCursor }} }} }}",after(&cursor)))?;
        let connection = &value["pullRequest"]["timelineItems"];
        for row in connection["nodes"].as_array().ok_or_else(malformed)? {
            if row["afterCommit"]["oid"] == head {
                let time = string(row, "createdAt")?;
                timestamp(time).ok_or_else(malformed)?;
                if newest.as_deref().is_none_or(|n| time > n) {
                    newest = Some(time.to_owned());
                }
            }
        }
        let following = next(connection)?;
        if following.is_none() {
            return Ok(newest);
        }
        if following == cursor {
            return Err(malformed());
        }
        cursor = following;
    }
    Err(malformed())
}
fn suite_time(remote: &Remote, repo: &str, head: &str) -> Result<Option<String>, HostError> {
    let mut earliest: Option<String> = None;
    for page in 1..=100 {
        let value = remote.host.get(&format!(
            "repos/{repo}/commits/{head}/check-suites?per_page=100&page={page}"
        ))?;
        let rows = value["check_suites"].as_array().ok_or_else(malformed)?;
        for row in rows {
            if row["head_sha"] != head {
                continue;
            }
            let time = string(row, "created_at")?;
            timestamp(time).ok_or_else(malformed)?;
            if earliest.as_deref().is_none_or(|e| time < e) {
                earliest = Some(time.into());
            }
        }
        if rows.len() < 100 {
            return Ok(earliest);
        }
    }
    Err(malformed())
}
fn failed_check(row: &Value) -> Result<Option<String>, HostError> {
    if !row["isRequired"].as_bool().ok_or_else(malformed)? {
        return Ok(None);
    }
    let (name, result, failed) = match string(row, "__typename")? {
        "CheckRun" => {
            let status = string(row, "status")?;
            let conclusion = row["conclusion"].as_str().unwrap_or("missing conclusion");
            (
                string(row, "name")?,
                conclusion,
                status == "COMPLETED" && conclusion != "SUCCESS",
            )
        }
        "StatusContext" => {
            let state = string(row, "state")?;
            (
                string(row, "context")?,
                state,
                matches!(state, "FAILURE" | "ERROR"),
            )
        }
        _ => return Err(malformed()),
    };
    Ok(failed.then(|| format!("{name}: {result}")))
}
fn checks(remote: &Remote, repo: &str, pr: u64, head: &str) -> Result<Option<String>, HostError> {
    let mut cursor = None;
    for _ in 0..100 {
        let value = remote.query(repo, &format!("object(expression:{}) {{ ... on Commit {{ statusCheckRollup {{ contexts(first:100{}) {{ nodes {{ __typename ... on CheckRun {{ name status conclusion isRequired(pullRequestNumber:{pr}) }} ... on StatusContext {{ context state isRequired(pullRequestNumber:{pr}) }} }} pageInfo {{ hasNextPage endCursor }} }} }} }} }}", json!(head),after(&cursor)))?;
        let commit = value
            .get("object")
            .filter(|v| v.is_object())
            .ok_or_else(malformed)?;
        if commit.get("statusCheckRollup") == Some(&Value::Null) {
            return Ok(None);
        }
        let connection = &commit["statusCheckRollup"]["contexts"];
        for row in connection["nodes"].as_array().ok_or_else(malformed)? {
            if let Some(reason) = failed_check(row)? {
                return Ok(Some(reason));
            }
        }
        let following = next(connection)?;
        if following.is_none() {
            return Ok(None);
        }
        if following == cursor {
            return Err(malformed());
        }
        cursor = following;
    }
    Err(malformed())
}
/// Arm only after every ordered guard succeeds. The injected attempt is read first.
pub fn run(directory: &Path, args: &[String], attempt: Attempt) -> Answer {
    let mut answer = Answer::default();
    if attempt.inside() {
        return answer.end(
            "inside-run-attempt",
            2,
            Some("landing acts are forbidden inside a run attempt".into()),
        );
    }
    let args = match arguments(args, false) {
        Ok(a) => a,
        Err(e) => return answer.end("usage", 3, Some(e)),
    };
    answer.pull_request = Some(args.pr);
    let repo = match repository(&args, directory) {
        Ok(r) => r,
        Err(e) => return answer.end("usage", 3, Some(e)),
    };
    let (policy, signed) = match declaration(directory) {
        Ok(p) => p,
        Err(e) => return answer.end("refused", 2, Some(e)),
    };
    // `require` answers every key it was asked for, or refuses.
    let default_branch = match policy
        .require(&["default_branch"])
        .map(|mut p| p.remove("default_branch"))
    {
        Ok(Some(supplied)) => supplied,
        Ok(None) => {
            return answer.end(
                "refused",
                2,
                Some("unsupplied landing.default_branch".into()),
            );
        }
        Err(e) => return answer.end("refused", 2, Some(e)),
    };
    let default_branch_value = default_branch.value.clone();
    answer.policy.default_branch = Some(default_branch);
    let remote = Remote::new();
    let pr = match remote.pr(&repo, args.pr) {
        Ok(p) => p,
        Err(e) => return answer.end("failed", 4, Some(e.reason())),
    };
    let head = pr["headRefOid"].as_str().expect("validated head");
    answer.head = Some(head.into());
    if !pr["autoMergeRequest"].is_null() {
        let pushed = push_time(&remote, &repo, args.pr, head).ok().flatten();
        let (word, code, reason) = armed_outcome(
            pr["autoMergeRequest"]["enabledAt"].as_str().unwrap_or(""),
            pushed.as_deref(),
        );
        return answer.end(word, code, reason);
    }
    // Each condition is evaluated only after all earlier conditions passed.
    if pr["state"] != "OPEN" {
        return answer.end("not-open", 2, Some("pull request is not open".into()));
    }
    match pr["isDraft"].as_bool() {
        Some(true) => return answer.end("draft", 2, Some("pull request is a draft".into())),
        Some(false) => {}
        None => return answer.end("failed", 4, Some(malformed().reason())),
    }
    let base = match string(&pr, "baseRefName") {
        Ok(b) => b,
        Err(e) => return answer.end("failed", 4, Some(e.reason())),
    };
    if default_branch_value != base {
        return answer.end("stacked", 2, Some(format!("base is {base}")));
    }
    if args.sha.as_deref().is_some_and(|sha| sha != head) {
        return answer.end("stale", 2, Some(format!("current head is {head}")));
    }
    let snapshot = Snapshot {
        remote: &remote,
        repo: &repo,
        number: args.pr,
        pr: &pr,
    };
    let review = match review_status::status(
        &snapshot,
        review_status::Request {
            pull_request: args.pr,
            head: Some(head),
            repository: Some(&repo),
            directory,
        },
    ) {
        Ok(r) => r.outcome.word(),
        Err(review_status::Error::Read(e)) => return answer.end("failed", 4, Some(e.reason())),
        Err(review_status::Error::Usage(e)) => return answer.end("failed", 4, Some(e.into())),
    };
    if review != "no-findings" {
        return answer.end(
            &format!("review-{review}"),
            2,
            Some(format!("review at {head}: {review}")),
        );
    }
    match checks(&remote, &repo, args.pr, head) {
        Ok(Some(reason)) => return answer.end("check-failed", 2, Some(reason)),
        Ok(None) => {}
        Err(e) => return answer.end("failed", 4, Some(e.reason())),
    }
    if signed {
        let value = match remote.query(
            &repo,
            &format!(
                "object(expression:{}) {{ ... on Commit {{ signature {{ isValid }} }} }}",
                json!(head)
            ),
        ) {
            Ok(v) => v,
            Err(e) => return answer.end("failed", 4, Some(e.reason())),
        };
        if value["object"]["signature"]["isValid"] != true {
            return answer.end(
                "unsigned",
                2,
                Some(format!("head {head} is not verified by GitHub")),
            );
        }
    }
    let methods = match remote.query(
        &repo,
        &format!(
            "mergeQueue(branch:{}) {{ id }} viewerDefaultMergeMethod",
            json!(base)
        ),
    ) {
        Ok(v) => v,
        Err(e) => return answer.end("failed", 4, Some(e.reason())),
    };
    let mut flags = vec!["--auto", "--match-head-commit", head];
    match methods.get("mergeQueue") {
        Some(Value::Null) => match methods["viewerDefaultMergeMethod"].as_str() {
            Some("MERGE") => flags.push("--merge"),
            Some("SQUASH") => flags.push("--squash"),
            Some("REBASE") => flags.push("--rebase"),
            _ => return answer.end("failed", 4, Some(malformed().reason())),
        },
        Some(v) if v.is_object() => {}
        _ => return answer.end("failed", 4, Some(malformed().reason())),
    }
    match remote.merge(&repo, args.pr, &flags) {
        Ok(()) => answer.end("armed", 0, None),
        Err(e) => answer.end("failed", 4, Some(e.reason())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arming_order_fails_closed_and_requires_strictly_later_arming() {
        let before = "2026-10-27T10:00:00Z";
        let after = "2026-10-27T11:00:00Z";
        assert_eq!(armed_outcome(after, Some(before)).0, "already-armed");
        for (armed, pushed) in [
            (before, Some(after)),
            (before, Some(before)),
            (after, None),
            ("bad", Some(before)),
            (after, Some("bad")),
            (after, Some("2026-02-30T10:00:00Z")),
        ] {
            assert_eq!(armed_outcome(armed, pushed).0, "armed-before-head");
        }
    }
    #[test]
    fn every_completed_non_success_refuses_but_pending_and_optional_do_not() {
        for conclusion in [
            "FAILURE",
            "CANCELLED",
            "SKIPPED",
            "TIMED_OUT",
            "ACTION_REQUIRED",
            "STALE",
            "STARTUP_FAILURE",
            "NEUTRAL",
        ] {
            let mut row = json!({"__typename":"CheckRun","isRequired":true,"name":"build","status":"COMPLETED","conclusion":conclusion});
            assert!(failed_check(&row).unwrap().is_some());
            row["isRequired"] = json!(false);
            assert!(failed_check(&row).unwrap().is_none());
        }
        for status in [
            "QUEUED",
            "IN_PROGRESS",
            "PENDING",
            "WAITING",
            "REQUESTED",
            "EXPECTED",
        ] {
            assert!(failed_check(&json!({"__typename":"CheckRun","isRequired":true,"name":"build","status":status,"conclusion":null})).unwrap().is_none());
        }
        for state in ["FAILURE", "ERROR", "PENDING", "SUCCESS", "EXPECTED"] {
            assert_eq!(failed_check(&json!({"__typename":"StatusContext","isRequired":true,"context":"ci","state":state})).unwrap().is_some(),matches!(state,"FAILURE"|"ERROR"));
        }
    }
    #[test]
    fn attempt_precedes_usage_and_all_reads() {
        for attempt in [
            Attempt {
                run_id: true,
                attempt: false,
            },
            Attempt {
                run_id: false,
                attempt: true,
            },
        ] {
            assert_eq!(
                run(Path::new("/does-not-exist"), &[], attempt).outcome,
                "inside-run-attempt"
            );
        }
    }
}
