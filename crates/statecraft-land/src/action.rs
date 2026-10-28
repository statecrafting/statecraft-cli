//! Shared inputs and host reads for the explicit landing acts.
use crate::policy::{Policy, Supplied};
use serde::Serialize;
use serde_json::{Value, json};
use statecraft_environment::manifest::Manifest;
use statecraft_home::setup::{GhHost, Host, HostError, classify_gh_failure, repository_slug};
use std::{
    path::Path,
    process::{Command, Output},
};

/// Presence, including an empty value, of the run attempt bindings.
#[derive(Default, Clone, Copy)]
pub struct Attempt {
    /// STATECRAFT_RUN_ID is present.
    pub run_id: bool,
    /// STATECRAFT_ATTEMPT is present.
    pub attempt: bool,
}
impl Attempt {
    pub(crate) fn inside(self) -> bool {
        self.run_id || self.attempt
    }
}
/// Fixed policy member names for the action answers.
#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsedPolicy {
    /// The branch and its supplying layer, once resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_branch: Option<Supplied>,
    /// Stack depth cap and supplying layer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack_depth: Option<Supplied>,
    /// Per-spec stack depth caps and supplying layer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack_depth_by_spec: Option<Supplied>,
    /// Total stacked pull request cap and supplying layer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack_max_stacked: Option<Supplied>,
    /// The pre-push vectors and their supplying layer, once used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_push: Option<Supplied>,
    /// Generated paths and regenerators used by restack.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated: Option<Supplied>,
}
pub(crate) struct Arguments {
    pub pr: u64,
    pub sha: Option<String>,
    pub repo: Option<String>,
}
pub(crate) fn arguments(args: &[String], push: bool) -> Result<Arguments, String> {
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
                result.repo = Some(words.next().ok_or("missing repository")?.clone())
            }
            flag if flag == if push { "--lease" } else { "--head" } && result.sha.is_none() => {
                result.sha = Some(words.next().ok_or("missing SHA")?.clone());
            }
            n if result.pr == 0 && !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
                result.pr = n
                    .parse()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or("expected positive PR number")?;
            }
            _ => return Err("unexpected or duplicate argument".into()),
        }
    }
    if result.pr == 0
        || (push && result.sha.is_none())
        || result
            .sha
            .as_ref()
            .is_some_and(|s| s.is_empty() || !s.bytes().all(|b| b.is_ascii_hexdigit()))
        || result.repo.as_ref().is_some_and(|r| !valid_repo(r))
    {
        return Err("expected a positive PR, hexadecimal SHA and owner/name repository; push requires --lease".into());
    }
    Ok(result)
}
pub(crate) fn valid_repo(repo: &str) -> bool {
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
pub(crate) fn repository(args: &Arguments, dir: &Path) -> Result<String, String> {
    args.repo
        .clone()
        .or_else(|| repository_slug(dir))
        .filter(|r| valid_repo(r))
        .ok_or_else(|| "expected --repo owner/name or a GitHub origin".into())
}
pub(crate) fn declaration(dir: &Path) -> Result<(Policy, bool), String> {
    declared_policy(&committed_declaration(dir)?)
}
pub(crate) fn committed_declaration(dir: &Path) -> Result<Manifest, String> {
    let output = git(dir, &["show", "HEAD:.statecraft/environment.json"])?;
    if !output.status.success() {
        return Err("cannot read committed declaration: landing.default_branch unavailable".into());
    }
    Manifest::parse(&output.stdout).map_err(|_| "malformed landing declaration".into())
}
pub(crate) fn declared_policy(manifest: &Manifest) -> Result<(Policy, bool), String> {
    let signed = match manifest
        .project
        .setup
        .as_ref()
        .and_then(|s| s.parameters.get("governance.require_signed_commits"))
    {
        None => false,
        Some(v) => v
            .as_bool()
            .ok_or("malformed governance.require_signed_commits")?,
    };
    Ok((Policy::from_project(&manifest.project)?, signed))
}
pub(crate) fn git(dir: &Path, args: &[&str]) -> Result<Output, String> {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|_| "could not execute git".into())
}
pub(crate) fn git_text(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = git(dir, args)?;
    if !output.status.success() {
        return Err(format!("git {} failed", args[0]));
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim().to_owned())
        .map_err(|_| "invalid git response".into())
}
pub(crate) fn malformed() -> HostError {
    HostError::Unreachable("invalid landing response".into())
}
pub(crate) fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, HostError> {
    v[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(malformed)
}
pub(crate) struct Remote {
    pub host: GhHost,
}
impl Remote {
    pub fn new() -> Self {
        Self {
            host: GhHost::default(),
        }
    }
    pub fn query(&self, repo: &str, fields: &str) -> Result<Value, HostError> {
        let (owner, name) = repo.split_once('/').ok_or_else(malformed)?;
        let query = format!(
            "query {{ repository(owner:{},name:{}) {{ {fields} }} }}",
            json!(owner),
            json!(name)
        );
        let value = self.graphql(&query)?;
        value["repository"]
            .as_object()
            .map(|_| value["repository"].clone())
            .ok_or_else(malformed)
    }
    pub fn graphql(&self, query: &str) -> Result<Value, HostError> {
        let output = Command::new(&self.host.program)
            .args(["api", "graphql", "-f", &format!("query={query}")])
            .output()
            .map_err(|_| HostError::Unreachable("could not execute gh".into()))?;
        if !output.status.success() {
            return Err(classify_gh_failure(&String::from_utf8_lossy(
                &output.stderr,
            )));
        }
        let value: Value = serde_json::from_slice(&output.stdout).map_err(|_| malformed())?;
        if value
            .get("errors")
            .is_some_and(|v| v.as_array().is_none_or(|a| !a.is_empty()))
        {
            return Err(malformed());
        }
        value["data"]
            .as_object()
            .map(|_| value["data"].clone())
            .ok_or_else(malformed)
    }
    pub fn pr(&self, repo: &str, pr: u64) -> Result<Value, HostError> {
        let value = self.query(repo, &format!("pullRequest(number:{pr}) {{ state isDraft baseRefName headRefName headRefOid headRepository {{ nameWithOwner }} autoMergeRequest {{ enabledAt }} mergeQueueEntry {{ state position }} }}"))?;
        let pr = &value["pullRequest"];
        if !pr.is_object()
            || !pr
                .get("autoMergeRequest")
                .is_some_and(|v| v.is_null() || v.is_object())
        {
            return Err(malformed());
        }
        string(pr, "headRefOid")?;
        Ok(pr.clone())
    }
    pub fn post(&self, path: &str) -> Result<(), HostError> {
        let output = Command::new(&self.host.program)
            .args(["api", "-X", "POST", path])
            .output()
            .map_err(|_| HostError::Unreachable("could not execute gh".into()))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(classify_gh_failure(&String::from_utf8_lossy(
                &output.stderr,
            )))
        }
    }
    pub fn merge(&self, repo: &str, pr: u64, flags: &[&str]) -> Result<(), HostError> {
        let output = Command::new(&self.host.program)
            .args(["pr", "merge", &pr.to_string(), "--repo", repo])
            .args(flags)
            .output()
            .map_err(|_| HostError::Unreachable("could not execute gh".into()))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(classify_gh_failure(&String::from_utf8_lossy(
                &output.stderr,
            )))
        }
    }
}
/// Supply the reviewed PR snapshot so review status judges the observed head.
pub(crate) struct Snapshot<'a> {
    pub remote: &'a Remote,
    pub repo: &'a str,
    pub number: u64,
    pub pr: &'a Value,
}
impl Host for Snapshot<'_> {
    fn get(&self, path: &str) -> Result<Value, HostError> {
        if path == format!("repos/{}/pulls/{}", self.repo, self.number) {
            Ok(
                json!({"state":string(self.pr,"state")?.to_ascii_lowercase(),"head":{"sha":string(self.pr,"headRefOid")?,"repo":{"full_name":string(&self.pr["headRepository"],"nameWithOwner")?}}}),
            )
        } else {
            self.remote.host.get(path)
        }
    }
}
