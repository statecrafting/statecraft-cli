//! The open pull requests, every page, shared by the landing reads (spec 063 section 3.6).
use crate::{
    action::{Remote, malformed},
    arm::{after, next},
};
use serde::Deserialize;
use statecraft_home::setup::HostError;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequest {
    pub(crate) head_ref_oid: String,
}
pub(crate) fn open(remote: &Remote, repo: &str) -> Result<Vec<PullRequest>, HostError> {
    let mut result = Vec::new();
    let mut cursor = None;
    for _ in 0..100 {
        let v = remote.query(repo, &format!("pullRequests(states:OPEN,first:100{}) {{ nodes {{ number headRefOid }} pageInfo {{ hasNextPage endCursor }} }}", after(&cursor)))?;
        let connection = &v["pullRequests"];
        let rows = connection["nodes"].as_array().ok_or_else(malformed)?;
        for row in rows {
            let pr: PullRequest = serde_json::from_value(row.clone()).map_err(|_| malformed())?;
            if pr.head_ref_oid.is_empty() || row["number"].as_u64().is_none_or(|n| n == 0) {
                return Err(malformed());
            }
            result.push(pr);
        }
        let following = next(connection)?;
        if following.is_none() {
            return Ok(result);
        }
        if following == cursor {
            return Err(malformed());
        }
        cursor = following;
    }
    Err(malformed())
}
