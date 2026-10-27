//! Per-PR GraphQL observation, with the existing review reader on that snapshot.
use crate::{Event, Observation, Queue, Request};
use serde_json::{Value, json};
use statecraft_home::{
    review_status,
    setup::{GhHost, Host, HostError, classify_gh_failure},
};

/// REST reads plus one pull request's GraphQL fields, never a queue page.
pub trait WaitHost: Host {
    /// Read state, head, arming and mergeQueueEntry for exactly this PR.
    fn pull_request(&self, repository: &str, number: u64) -> Result<Value, HostError>;
}
/// Production host. Authentication stays entirely in gh.
#[derive(Default)]
pub struct Gh(GhHost);
impl Host for Gh {
    fn get(&self, path: &str) -> Result<Value, HostError> {
        self.0.get(path)
    }
}
impl WaitHost for Gh {
    fn pull_request(&self, repository: &str, number: u64) -> Result<Value, HostError> {
        let (owner, name) = repository.split_once('/').ok_or_else(malformed)?;
        let query = format!(
            "query {{ repository(owner:{},name:{}) {{ pullRequest(number:{number}) {{ state headRefOid headRepository {{ nameWithOwner }} autoMergeRequest {{ enabledAt }} mergeQueueEntry {{ state position }} }} }} }}",
            json!(owner),
            json!(name)
        );
        let output = std::process::Command::new(&self.0.program)
            .args(["api", "graphql", "-f", &format!("query={query}")])
            .output()
            .map_err(|_| HostError::Unreachable("could not execute gh".into()))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let error = classify_gh_failure(&stderr);
            // GraphQL can report a missing PR without an HTTP error status.
            if error == HostError::Unreachable("gh exited without an HTTP status".into())
                && stderr.contains("Could not resolve to a PullRequest with the number of")
            {
                return Err(HostError::NotFound);
            }
            return Err(error);
        }
        let value: Value = serde_json::from_slice(&output.stdout).map_err(|_| malformed())?;
        // Never classify failure from response-body error text.
        if value
            .get("errors")
            .is_some_and(|v| v.as_array().is_none_or(|a| !a.is_empty()))
        {
            return Err(malformed());
        }
        let repository = value
            .get("data")
            .and_then(|d| d.get("repository"))
            .ok_or_else(malformed)?;
        if repository.is_null() {
            return Err(HostError::NotFound);
        }
        let pr = repository.get("pullRequest").ok_or_else(malformed)?;
        if pr.is_null() {
            return Err(HostError::NotFound);
        }
        Ok(pr.clone())
    }
}
fn malformed() -> HostError {
    HostError::Unreachable("invalid land wait response".into())
}
fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, HostError> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(malformed)
}
struct Snapshot<'a> {
    host: &'a dyn WaitHost,
    path: String,
    pr: Value,
}
impl Host for Snapshot<'_> {
    fn get(&self, path: &str) -> Result<Value, HostError> {
        if path == self.path {
            Ok(self.pr.clone())
        } else {
            self.host.get(path).map_err(|e| {
                if e == HostError::NotFound {
                    HostError::Unreachable("review resource not found".into())
                } else {
                    e
                }
            })
        }
    }
}
pub(crate) fn read(
    host: &dyn WaitHost,
    repo: &str,
    pr: u64,
    request: &Request<'_>,
) -> Result<Observation, HostError> {
    let value = host.pull_request(repo, pr)?;
    let state = match string(&value, "state")? {
        "OPEN" => "open",
        "CLOSED" => "closed",
        "MERGED" => "merged",
        _ => return Err(malformed()),
    }
    .to_owned();
    let head = string(&value, "headRefOid")?.to_owned();
    if !head.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(malformed());
    }
    let armed = match value.get("autoMergeRequest") {
        Some(Value::Null) => false,
        Some(v) if v.is_object() => {
            string(v, "enabledAt")?;
            true
        }
        _ => return Err(malformed()),
    };
    let queue = match value.get("mergeQueueEntry") {
        Some(Value::Null) => None,
        Some(v) => Some(Queue {
            state: string(v, "state")?.into(),
            position: v["position"].as_u64().ok_or_else(malformed)?,
        }),
        None => return Err(malformed()),
    };
    let review = if matches!(request.event, Event::Reviewed | Event::Changed) {
        let head_repo = string(&value["headRepository"], "nameWithOwner")?;
        let snapshot = Snapshot {
            host,
            path: format!("repos/{repo}/pulls/{pr}"),
            pr: json!({"state":state,"head":{"sha":head,"repo":{"full_name":head_repo}}}),
        };
        Some(
            review_status::status(
                &snapshot,
                review_status::Request {
                    pull_request: pr,
                    head: Some(&head),
                    repository: Some(repo),
                    directory: request.directory,
                },
            )
            .map_err(|e| match e {
                review_status::Error::Read(e) => e,
                review_status::Error::Usage(_) => malformed(),
            })?
            .outcome
            .word(),
        )
    } else {
        None
    };
    Ok(Observation {
        state,
        head,
        armed,
        queue,
        review,
    })
}
