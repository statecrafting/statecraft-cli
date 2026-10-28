//! Cancel only superseded waiting review-exception runs (spec 063 section 3.7).
use crate::{Attempt, UsedPolicy, action::*, pulls, runs};
use serde::Serialize;
use std::{collections::BTreeSet, path::Path};

const USAGE: &str = "land cancel-superseded [--repo <owner/name>] [--json]";

/// A successfully cancelled run.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cancelled {
    /// GitHub run identity.
    pub id: u64,
    /// Head observed before cancellation.
    pub head_sha: String,
    /// GitHub run URL.
    pub url: String,
}
/// A run whose cancellation failed.
#[derive(Debug, Serialize)]
pub struct NotCancelled {
    #[serde(flatten)]
    run: Cancelled,
    /// Classified failure, without raw response content.
    pub reason: String,
}
/// Result of one explicit cancellation invocation.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    /// Stable outcome word.
    pub outcome: String,
    /// Family exit code.
    pub exit_code: i32,
    /// Refusal or failure explanation.
    pub reason: Option<String>,
    /// Used policy and supplying layer.
    pub policy: UsedPolicy,
    /// Successful force-cancels, ordered by run ID.
    pub cancelled: Vec<Cancelled>,
    /// Failed force-cancels, ordered by run ID.
    pub not_cancelled: Vec<NotCancelled>,
    /// Waiting review-exception runs still needed by an open pull request.
    pub kept: usize,
}
impl Answer {
    fn end(mut self, outcome: &str, code: i32, reason: Option<String>) -> Self {
        self.outcome = outcome.into();
        self.exit_code = code;
        self.reason = reason;
        self
    }
}
fn arguments(args: &[String]) -> Result<Arguments, String> {
    let mut result = Arguments {
        pr: 0,
        sha: None,
        repo: None,
    };
    let mut words = args.iter();
    let mut json = false;
    while let Some(word) = words.next() {
        match word.as_str() {
            "--json" if !json => json = true,
            "--repo" if result.repo.is_none() => {
                result.repo = Some(words.next().filter(|r| valid_repo(r)).ok_or(USAGE)?.clone());
            }
            _ => return Err(USAGE.into()),
        }
    }
    Ok(result)
}

/// Check the attempt first, finish all reads, then attempt each cancellation once.
pub fn run(directory: &Path, args: &[String], attempt: Attempt) -> Answer {
    let mut answer = Answer::default();
    if attempt.inside() {
        return answer.end("inside-run-attempt", 2, Some("inside-run-attempt".into()));
    }
    let args = match arguments(args) {
        Ok(v) => v,
        Err(e) => return answer.end("usage", 3, Some(e)),
    };
    let (policy, _) = match declaration(directory) {
        Ok(v) => v,
        Err(e) => return answer.end("refused", 2, Some(e)),
    };
    answer.policy.default_branch = match policy.require(&["default_branch"]) {
        Ok(mut v) => v.remove("default_branch"),
        Err(e) => return answer.end("refused", 2, Some(e)),
    };
    let repo = match repository(&args, directory) {
        Ok(v) => v,
        Err(e) => return answer.end("usage", 3, Some(e)),
    };
    let remote = Remote::new();
    let prs = match pulls::open(&remote, &repo) {
        Ok(v) => v,
        Err(e) => return answer.end("failed", 4, Some(e.reason())),
    };
    let heads: BTreeSet<_> = prs.iter().map(|pr| pr.head_ref_oid.as_str()).collect();
    let mut waiting = match runs::waiting(&remote, &repo, &heads) {
        Ok(v) => v,
        Err(e) => return answer.end("failed", 4, Some(e.reason())),
    };
    waiting.sort_by_key(|run| run.id);
    // Pagination may repeat a row while the remote list changes.
    waiting.dedup_by_key(|run| run.id);
    for run in waiting {
        if !run.superseded {
            answer.kept += 1;
            continue;
        }
        let path = format!("repos/{repo}/actions/runs/{}/force-cancel", run.id);
        let run = Cancelled {
            id: run.id,
            head_sha: run.head_sha,
            url: run.html_url,
        };
        match remote.post(&path) {
            Ok(()) => answer.cancelled.push(run),
            Err(e) => answer.not_cancelled.push(NotCancelled {
                run,
                reason: e.reason(),
            }),
        }
    }
    if answer.not_cancelled.is_empty() {
        answer.end("cancelled", 0, None)
    } else {
        let reason = format!(
            "{} run(s) could not be cancelled",
            answer.not_cancelled.len()
        );
        answer.end("failed", 4, Some(reason))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_rejects_duplicates_and_invalid_repository() {
        for args in [
            vec!["--json", "--json"],
            vec!["--repo", "o/r", "--repo", "o/r"],
            vec!["--repo", "../r"],
            vec!["--repo"],
            vec!["7"],
        ] {
            let args: Vec<_> = args.into_iter().map(str::to_owned).collect();
            assert!(arguments(&args).is_err());
        }
        assert!(arguments(&[]).is_ok());
        assert!(arguments(&["--json".into(), "--repo".into(), "o/r".into()]).is_ok());
    }
}
