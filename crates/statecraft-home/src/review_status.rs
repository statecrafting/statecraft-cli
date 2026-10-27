//! Spec 062: one read-only answer about the review of a pull request's head.
use crate::setup::{Host, HostError, repository_slug};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

// github-actions-rust/workflows/statecraft-ci.yml names the caller ai-review;
// workflows/statecraft-ai-review.yml names its called job review.
const REVIEW_JOB: &str = "ai-review / review";

/// Arguments independent of any command-line rendering.
pub struct Request<'a> {
    /// The pull request number.
    pub pull_request: u64,
    /// The requested head, or the current head when omitted.
    pub head: Option<&'a str>,
    /// An explicit owner/name; otherwise read origin.
    pub repository: Option<&'a str>,
    /// The directory whose origin supplies the default repository.
    pub directory: &'a Path,
}

/// The closed outcome vocabulary; skip classes remain forward compatible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// A completed review with no findings.
    NoFindings,
    /// A completed review with findings.
    Findings,
    /// A published skip and its class.
    Skipped(String),
    /// An unfinished review may still publish a verdict.
    Pending,
    /// No counting record and no unfinished review.
    Absent,
    /// The requested head differs from the current head.
    Stale,
}
impl Outcome {
    /// Stable wire spelling.
    pub fn word(&self) -> String {
        match self {
            Self::NoFindings => "no-findings".into(),
            Self::Findings => "findings".into(),
            Self::Skipped(class) => format!("skipped:{class}"),
            Self::Pending => "pending".into(),
            Self::Absent => "absent".into(),
            Self::Stale => "stale".into(),
        }
    }
    /// Spec 006 process exit code for this outcome.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NoFindings => 0,
            Self::Findings | Self::Skipped(_) => 1,
            Self::Pending | Self::Absent | Self::Stale => 2,
        }
    }
}
impl Serialize for Outcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.word())
    }
}

/// The counting check run identity.
#[derive(Debug, Serialize)]
pub struct CheckRun {
    /// GitHub identifier.
    pub id: u64,
    /// The check run URL.
    pub url: String,
}
/// The observed workflow attempt and review job.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    /// GitHub identifier.
    pub id: u64,
    /// The workflow run attempt number.
    pub attempt: u64,
    /// The review job conclusion, absent until ended.
    pub review_job_conclusion: Option<String>,
    /// Elapsed seconds for a completed review job.
    pub review_job_duration_seconds: Option<u64>,
}
/// Both human and JSON output render this value.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// The classified review state.
    pub outcome: Outcome,
    /// The pull request number.
    pub pull_request: u64,
    /// The requested head, or the current head when omitted.
    pub head: String,
    /// The head read from the pull request.
    pub current_head: String,
    /// The counting check run, if any.
    pub check_run: Option<CheckRun>,
    /// An unfinished review attempt, or the newest ended attempt.
    pub run: Option<Run>,
    /// Why the review is absent or stale.
    pub reason: Option<String>,
    /// The exact counting evidence value, with all fields retained.
    pub record: Option<Value>,
}

/// A usage error or a sanitized host read failure.
#[derive(Debug)]
pub enum Error {
    /// Arguments cannot identify a supported read.
    Usage(&'static str),
    /// The host did not supply a usable answer.
    Read(HostError),
}
impl From<HostError> for Error {
    fn from(value: HostError) -> Self {
        Self::Read(value)
    }
}
fn malformed() -> HostError {
    HostError::Unreachable("invalid review status response".into())
}
fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, HostError> {
    value[key].as_str().ok_or_else(malformed)
}
fn number(value: &Value, key: &str) -> Result<u64, HostError> {
    value[key].as_u64().ok_or_else(malformed)
}
fn valid_repo(repo: &str) -> bool {
    let parts: Vec<_> = repo.split('/').collect();
    parts.len() == 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && *p != "."
                && *p != ".."
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
}

/// The most pages one collection read follows: 5,000 rows, far beyond any
/// real head's check runs or workflow runs.
const MAX_PAGES: u32 = 50;

/// Page through a REST collection using GET only. No retry or polling.
/// Stops at a short or empty page or once `total_count` rows are read, and
/// fails rather than reading past `MAX_PAGES`, so a host that keeps
/// answering full pages cannot hold the read open.
fn collection(host: &dyn Host, path: &str, key: &str) -> Result<Vec<Value>, HostError> {
    let mut values = Vec::new();
    for page in 1..=MAX_PAGES {
        let separator = if path.contains('?') { '&' } else { '?' };
        let response = host.get(&format!("{path}{separator}per_page=100&page={page}"))?;
        let rows = response[key].as_array().ok_or_else(malformed)?;
        let count = rows.len();
        values.extend(rows.iter().cloned());
        let total = response["total_count"].as_u64();
        if count < 100 || total.is_some_and(|t| values.len() as u64 >= t) {
            return Ok(values);
        }
    }
    Err(HostError::Unreachable(format!(
        "more than {MAX_PAGES} pages of {key}"
    )))
}

/// Only a fenced JSON record with both subject identifiers can count.
fn record(check: &Value, pr: u64, head: &str) -> Option<Value> {
    if check["name"] != "statecraft-ai-review" {
        return None;
    }
    let text = check["output"]["text"].as_str()?;
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.trim() != "```json" {
            continue;
        }
        let mut block = String::new();
        let mut closed = false;
        for line in lines.by_ref() {
            if line.trim() == "```" {
                closed = true;
                break;
            }
            block.push_str(line);
            block.push('\n');
        }
        if closed
            && let Ok(value) = serde_json::from_str::<Value>(&block)
            && value["subject"]["pullRequest"].as_u64() == Some(pr)
            && value["subject"]["head"].as_str() == Some(head)
        {
            return Some(value);
        }
    }
    None
}
fn counting<'a>(checks: &'a [Value], pr: u64, head: &str) -> Option<(&'a Value, Value)> {
    checks
        .iter()
        .filter_map(|check| Some((check, record(check, pr, head)?, check["id"].as_u64()?)))
        .max_by_key(|(_, _, id)| *id)
        .map(|(check, record, _)| (check, record))
}

// GitHub REST job timestamps are UTC YYYY-MM-DDTHH:MM:SSZ. Validate the
// calendar before converting to seconds, including jobs spanning midnight.
fn timestamp(text: &str) -> Option<i64> {
    if !text.is_ascii() || text.len() != 20 || !text.ends_with('Z') {
        return None;
    }
    let parts: Vec<_> = text[..19].split(['-', 'T', ':']).collect();
    if parts.len() != 6 {
        return None;
    }
    let n: Vec<i64> = parts
        .iter()
        .map(|s| s.parse().ok())
        .collect::<Option<_>>()?;
    let (year, month, day, hour, minute, second) = (n[0], n[1], n[2], n[3], n[4], n[5]);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
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
    if !(1..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || !(1..=days[(month - 1) as usize]).contains(&day)
        || !(0..24).contains(&hour)
        || !(0..60).contains(&minute)
        || !(0..60).contains(&second)
    {
        return None;
    }
    let y = year - 1;
    let prior = y * 365 + y / 4 - y / 100 + y / 400;
    Some(
        (prior + days[..(month - 1) as usize].iter().sum::<i64>() + day - 1) * 86400
            + hour * 3600
            + minute * 60
            + second,
    )
}
fn duration(job: &Value) -> Option<u64> {
    let start = timestamp(job["started_at"].as_str()?)?;
    let end = timestamp(job["completed_at"].as_str()?)?;
    u64::try_from(end - start).ok()
}

fn runs(
    host: &dyn Host,
    repo: &str,
    head: &str,
    pr: u64,
) -> Result<Option<(Run, bool)>, HostError> {
    let mut runs = collection(
        host,
        &format!("repos/{repo}/actions/workflows/statecraft-ci.yml/runs?head_sha={head}"),
        "workflow_runs",
    )?;
    runs.retain(|run| {
        run["head_sha"] == head
            && run["pull_requests"]
                .as_array()
                .is_none_or(|prs| prs.is_empty() || prs.iter().any(|p| p["number"] == pr))
    });
    runs.sort_by_key(|run| std::cmp::Reverse(run["id"].as_u64().unwrap_or(0)));
    let mut latest = None;
    for run in runs {
        let id = number(&run, "id")?;
        let attempt = number(&run, "run_attempt")?;
        let jobs = collection(
            host,
            &format!("repos/{repo}/actions/runs/{id}/attempts/{attempt}/jobs"),
            "jobs",
        )?;
        let job = jobs
            .iter()
            .filter(|j| j["name"] == REVIEW_JOB)
            .max_by_key(|j| j["id"].as_u64().unwrap_or(0));
        let active = if let Some(job) = job {
            string(job, "status")? != "completed"
        } else {
            string(&run, "status")? != "completed"
        };
        let summary = Run {
            id,
            attempt,
            review_job_conclusion: job.and_then(|j| j["conclusion"].as_str().map(str::to_owned)),
            review_job_duration_seconds: job.and_then(duration),
        };
        // Any unfinished review can still publish a counting record.
        if active {
            return Ok(Some((summary, true)));
        }
        if latest.is_none() {
            latest = Some((summary, false));
        }
    }
    Ok(latest)
}

/// Read exactly one snapshot through the operator's host, without mutations.
pub fn status(host: &dyn Host, request: Request<'_>) -> Result<Status, Error> {
    let repo = request
        .repository
        .map(str::to_owned)
        .or_else(|| repository_slug(request.directory))
        .ok_or(Error::Usage(
            "--repo is required when origin is not a GitHub repository",
        ))?;
    if request.pull_request == 0
        || !valid_repo(&repo)
        || request
            .head
            .is_some_and(|h| h.is_empty() || !h.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(Error::Usage(
            "expected a positive PR number, owner/name repository and hexadecimal head",
        ));
    }
    let pr = host.get(&format!("repos/{repo}/pulls/{}", request.pull_request))?;
    let _state = string(&pr, "state")?;
    let current = string(&pr["head"], "sha")?;
    if current.is_empty() || !current.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(malformed().into());
    }
    let mut answer = Status {
        outcome: Outcome::Absent,
        pull_request: request.pull_request,
        head: request.head.unwrap_or(current).into(),
        current_head: current.into(),
        check_run: None,
        run: None,
        reason: None,
        record: None,
    };
    if request.head.is_some_and(|head| head != current) {
        answer.outcome = Outcome::Stale;
        answer.reason = Some("head-moved".into());
        return Ok(answer);
    }
    if string(&pr["head"]["repo"], "full_name")? != repo {
        answer.outcome = Outcome::Skipped("fork".into());
        return Ok(answer);
    }
    let checks = collection(
        host,
        &format!(
            "repos/{repo}/commits/{current}/check-runs?check_name=statecraft-ai-review&filter=all"
        ),
        "check_runs",
    )?;
    let selected = counting(&checks, request.pull_request, current);
    let run = runs(host, &repo, current, request.pull_request)?;
    let pending = run.as_ref().is_some_and(|(_, active)| *active);
    answer.run = run.map(|(run, _)| run);
    if let Some((check, record)) = selected {
        let result = string(&record, "result")?;
        answer.outcome = match result {
            "no-findings" => Outcome::NoFindings,
            "findings" => Outcome::Findings,
            value
                if value
                    .strip_prefix("skipped:")
                    .is_some_and(|c| !c.is_empty()) =>
            {
                Outcome::Skipped(value[8..].into())
            }
            _ => return Err(malformed().into()),
        };
        answer.check_run = Some(CheckRun {
            id: number(check, "id")?,
            url: string(check, "html_url")?.into(),
        });
        answer.record = Some(record);
    } else if pending {
        answer.outcome = Outcome::Pending;
    } else {
        answer.reason = Some(
            if checks.iter().any(|c| c["name"] == "statecraft-ai-review") {
                "mismatched-record"
            } else if answer
                .run
                .as_ref()
                .is_some_and(|r| r.review_job_conclusion.as_deref() == Some("cancelled"))
            {
                "cancelled"
            } else if answer.run.is_some() {
                "failed"
            } else {
                "no-run"
            }
            .into(),
        );
    }
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn check(id: u64, pr: u64, head: &str) -> Value {
        json!({"id":id,"name":"statecraft-ai-review","output":{"text":format!("```json\n{}\n```", json!({"subject":{"pullRequest":pr,"head":head}}))}})
    }
    struct Endless;
    impl Host for Endless {
        fn get(&self, _path: &str) -> Result<Value, HostError> {
            let rows: Vec<Value> = (0..100).map(|i| json!({"id": i})).collect();
            Ok(json!({"check_runs": rows}))
        }
    }
    struct Counted(std::cell::Cell<u32>);
    impl Host for Counted {
        fn get(&self, _path: &str) -> Result<Value, HostError> {
            self.0.set(self.0.get() + 1);
            let rows: Vec<Value> = (0..100).map(|i| json!({"id": i})).collect();
            Ok(json!({"total_count": 200, "check_runs": rows}))
        }
    }
    #[test]
    fn a_host_answering_full_pages_forever_is_cut_off() {
        let err =
            collection(&Endless, "repos/o/r/commits/abc/check-runs", "check_runs").unwrap_err();
        assert_eq!(
            err,
            HostError::Unreachable("more than 50 pages of check_runs".into())
        );
    }
    #[test]
    fn total_count_ends_the_read_on_a_full_last_page() {
        let host = Counted(std::cell::Cell::new(0));
        let rows = collection(&host, "repos/o/r/commits/abc/check-runs", "check_runs").unwrap();
        assert_eq!((rows.len(), host.0.get()), (200, 2));
    }
    #[test]
    fn highest_matching_id_counts_independent_of_order() {
        let checks = vec![
            check(4, 7, "abc"),
            check(99, 8, "abc"),
            check(100, 7, "def"),
            check(1, 7, "abc"),
        ];
        assert_eq!(counting(&checks, 7, "abc").unwrap().0["id"], 4);
        assert!(counting(&checks, 9, "abc").is_none());
    }
    #[test]
    fn malformed_and_unfenced_records_do_not_count() {
        for text in ["{}", "```json\nnot json\n```", "```json\n{}"] {
            let check = json!({"name":"statecraft-ai-review","output":{"text":text}});
            assert!(record(&check, 1, "abc").is_none());
        }
    }
    #[test]
    fn durations_cross_calendar_boundaries_and_reject_bad_dates() {
        assert_eq!(
            duration(
                &json!({"started_at":"2024-02-29T23:50:00Z","completed_at":"2024-03-01T00:10:00Z"})
            ),
            Some(1200)
        );
        assert!(timestamp("2023-02-29T00:00:00Z").is_none());
        assert!(
            duration(
                &json!({"started_at":"2024-03-01T00:10:00Z","completed_at":"2024-03-01T00:00:00Z"})
            )
            .is_none()
        );
    }
}
