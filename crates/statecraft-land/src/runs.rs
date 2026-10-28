//! Waiting review-exception runs, read in full before any landing act uses them.
use crate::action::{Remote, malformed};
use serde::Deserialize;
// `Host` is the trait that provides `remote.host.get`; without it in scope
// that call does not resolve (E0599).
use statecraft_home::setup::{Host, HostError};
use std::collections::BTreeSet;

// The github-actions-rust profile's workflows/statecraft-ci.yml names this Environment.
const REVIEW_EXCEPTION: &str = "statecraft-review-exception";
#[derive(Deserialize)]
pub(crate) struct WaitingRun {
    pub id: u64,
    pub head_sha: String,
    pub html_url: String,
    pub created_at: String,
    #[serde(skip)]
    pub superseded: bool,
}
pub(crate) fn waiting(
    remote: &Remote,
    repo: &str,
    heads: &BTreeSet<&str>,
) -> Result<Vec<WaitingRun>, HostError> {
    let mut result = Vec::new();
    for page in 1..=100 {
        let value = remote.host.get(&format!(
            "repos/{repo}/actions/runs?status=waiting&per_page=100&page={page}"
        ))?;
        let rows = value["workflow_runs"].as_array().ok_or_else(malformed)?;
        for row in rows {
            let event = row["event"].as_str().ok_or_else(malformed)?;
            if event != "pull_request" {
                continue;
            }
            let mut run: WaitingRun =
                serde_json::from_value(row.clone()).map_err(|_| malformed())?;
            let pending = remote.host.get(&format!(
                "repos/{repo}/actions/runs/{}/pending_deployments",
                run.id
            ))?;
            let deployments = pending.as_array().ok_or_else(malformed)?;
            let mut kept = false;
            for entry in deployments {
                let name = entry["environment"]["name"]
                    .as_str()
                    .ok_or_else(malformed)?;
                kept |= name == REVIEW_EXCEPTION;
            }
            if kept {
                run.superseded = !heads.contains(run.head_sha.as_str());
                result.push(run);
            }
        }
        if rows.len() < 100 {
            return Ok(result);
        }
    }
    Err(malformed())
}
