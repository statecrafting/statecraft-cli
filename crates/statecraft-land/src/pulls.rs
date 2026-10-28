//! The open pull requests, every page, shared by the landing reads (spec 063 section 3.6).
use crate::{
    action::{Remote, malformed},
    arm::{after, next, timestamp},
};
use serde::Deserialize;
use serde_json::Value;
use statecraft_home::setup::HostError;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequest {
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) created_at: String,
    pub(crate) is_draft: bool,
    pub(crate) base_ref_name: String,
    pub(crate) head_ref_name: String,
    pub(crate) head_ref_oid: String,
    pub(crate) merge_state_status: String,
    pub(crate) head_repository: Value,
    pub(crate) auto_merge_request: Option<Value>,
    pub(crate) merge_queue_entry: Option<Value>,
}
pub(crate) fn open(remote: &Remote, repo: &str) -> Result<Vec<PullRequest>, HostError> {
    let mut result = Vec::new();
    let mut cursor = None;
    for _ in 0..100 {
        let v = remote.query(repo, &format!("pullRequests(states:OPEN,first:100{}) {{ nodes {{ number title url createdAt isDraft baseRefName headRefName headRefOid headRepository {{ nameWithOwner }} mergeStateStatus autoMergeRequest {{ enabledAt }} mergeQueueEntry {{ state position }} }} pageInfo {{ hasNextPage endCursor }} }}", after(&cursor)))?;
        let connection = &v["pullRequests"];
        let rows = connection["nodes"].as_array().ok_or_else(malformed)?;
        for row in rows {
            let pr: PullRequest = serde_json::from_value(row.clone()).map_err(|_| malformed())?;
            if pr.number == 0
                || timestamp(&pr.created_at).is_none()
                || pr.head_ref_oid.is_empty()
                || pr.head_ref_name.is_empty()
                || pr.base_ref_name.is_empty()
                || row.get("autoMergeRequest").is_none()
                || row.get("mergeQueueEntry").is_none()
            {
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
