//! Read-only landing priorities and stacks (spec 063 section 3.6).
use crate::{
    UsedPolicy,
    action::*,
    arm::{Checks, read_checks, timestamp},
    pulls::{PullRequest, open},
    runs,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use statecraft_home::{
    review_status,
    setup::{Host, HostError},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

/// One status result in either rendering.
#[derive(Serialize)]
pub struct Answer {
    /// Family exit code, also present in the report.
    #[serde(skip)]
    pub exit_code: i32,
    #[serde(flatten)]
    report: Value,
}
impl Answer {
    fn end(mut self, outcome: &str, code: i32, reason: Option<String>) -> Self {
        self.exit_code = code;
        self.report["exitCode"] = json!(code);
        self.report["outcome"] = json!(outcome);
        self.report["reason"] = json!(reason);
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
                let repo = words
                    .next()
                    .filter(|r| valid_repo(r))
                    .ok_or("expected --repo owner/name")?;
                result.repo = Some(repo.clone());
            }
            _ => return Err("land status [--repo owner/name] [--json]".into()),
        }
    }
    Ok(result)
}
fn review(
    remote: &Remote,
    repo: &str,
    directory: &Path,
    pr: &PullRequest,
) -> Result<String, HostError> {
    // Snapshot supplies only the fields the review reader needs, pinned to this head.
    let value =
        json!({"state":"OPEN","headRefOid":pr.head_ref_oid,"headRepository":pr.head_repository});
    let snapshot = Snapshot {
        remote,
        repo,
        number: pr.number,
        pr: &value,
    };
    review_status::status(
        &snapshot,
        review_status::Request {
            pull_request: pr.number,
            head: Some(&pr.head_ref_oid),
            repository: Some(repo),
            directory,
        },
    )
    .map(|r| r.outcome.word())
    .map_err(|e| match e {
        review_status::Error::Read(e) => e,
        review_status::Error::Usage(_) => malformed(),
    })
}
fn stack(pr: &PullRequest, prs: &[PullRequest], default: &str) -> (usize, u64) {
    let mut current = pr;
    let mut seen = BTreeSet::from([pr.number]);
    let mut depth = 0;
    while current.base_ref_name != default {
        let Some(parent) = prs
            .iter()
            .find(|p| p.head_ref_name == current.base_ref_name)
        else {
            break;
        };
        if !seen.insert(parent.number) {
            break;
        }
        current = parent;
        depth += 1;
    }
    (depth, current.number)
}
fn spec(title: &str) -> Option<&str> {
    title
        .as_bytes()
        .windows(4)
        .position(|w| w[0] == b'(' && w[1..].iter().all(u8::is_ascii_digit))
        .map(|i| &title[i + 1..i + 4])
}
const RANK: [&str; 10] = [
    "retarget", "approve", "fix", "update", "arm", "hazard", "running", "merging", "stacked",
    "draft",
];
// None means classification has reached the review-dependent portion.
fn classify(
    pr: &PullRequest,
    prs: &[PullRequest],
    default: &str,
    checks: &Checks,
    waiting: bool,
    review: Option<&str>,
) -> Result<Option<(&'static str, String)>, HostError> {
    let armed = pr.auto_merge_request.is_some();
    let base = &pr.base_ref_name;
    let result = if base != default {
        match prs.iter().find(|p| &p.head_ref_name == base) {
            None => (
                "retarget",
                format!("base `{base}` has no open pull request: retarget to {default}"),
            ),
            Some(p) if armed => (
                "hazard",
                format!(
                    "armed while stacked on #{}: it would merge into `{base}`",
                    p.number
                ),
            ),
            Some(p) => ("stacked", format!("waiting for #{}", p.number)),
        }
    } else if let Some(queue) = &pr.merge_queue_entry {
        let state = string(queue, "state")?;
        let position = queue["position"].as_u64().ok_or_else(malformed)?;
        (
            if state == "UNMERGEABLE" {
                "fix"
            } else {
                "merging"
            },
            format!("merge queue position {position}, {state}"),
        )
    } else if !checks.failed.is_empty() {
        let mut detail = format!(
            "failed: {}",
            checks
                .failed
                .iter()
                .take(4)
                .map(|s| s
                    .split_once(": ")
                    .map(|(n, c)| format!("{n} ({c})"))
                    .unwrap_or_else(|| s.clone()))
                .collect::<Vec<_>>()
                .join(", ")
        );
        if checks.failed.len() > 4 {
            detail.push_str(&format!(" and {} more", checks.failed.len() - 4));
        }
        if waiting {
            detail.push_str("; an owner approval is also waiting");
        }
        ("fix", detail)
    } else if waiting {
        ("approve", "owner approval waiting".into())
    } else if pr.is_draft {
        ("draft", "draft".into())
    } else if checks.pending > 0 {
        (
            "running",
            format!("{} required check(s) running", checks.pending),
        )
    } else {
        let Some(word) = review else { return Ok(None) };
        // Earlier draft/check/pending classifications retain precedence. At a settled
        // review, an armed head without a clean verdict is specifically a hazard.
        if word == "pending" {
            ("running", "review pending".into())
        } else if word != "no-findings" {
            (
                if armed { "hazard" } else { "fix" },
                format!("review {word}"),
            )
        } else if armed {
            ("merging", "auto-merge armed".into())
        } else if pr.merge_state_status == "BEHIND" {
            ("update", format!("behind {default}"))
        } else {
            ("arm", "review no-findings: enable auto-merge".into())
        }
    };
    Ok(Some(result))
}
fn approval_reason(
    remote: &Remote,
    repo: &str,
    pr: u64,
    review: &str,
) -> Result<String, HostError> {
    let mut reasons = Vec::new();
    if review != "no-findings" {
        reasons.push(format!("review {review}: resolve it before approving"));
    }
    let (mut authority, mut spec) = (false, false);
    for page in 1..=100 {
        let v = remote.host.get(&format!(
            "repos/{repo}/pulls/{pr}/files?per_page=100&page={page}"
        ))?;
        let rows = v.as_array().ok_or_else(malformed)?;
        for row in rows {
            let path = string(row, "filename")?;
            authority |= [
                "scripts/statecraft/",
                ".github/workflows/",
                ".statecraft/setup/",
            ]
            .iter()
            .any(|p| path.starts_with(p));
            spec |= path.ends_with("spec.md");
        }
        if rows.len() < 100 {
            if authority {
                reasons.push("authority change".into());
            }
            if spec {
                reasons.push("possible ratification (a spec.md changed)".into());
            }
            return Ok(if reasons.is_empty() {
                "see the run".into()
            } else {
                reasons.join(", ")
            });
        }
    }
    Err(malformed())
}
#[derive(Deserialize)]
struct Plan {
    blocked: Vec<Blocked>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Blocked {
    id: String,
    blocked_by: Vec<Dependency>,
}
#[derive(Deserialize)]
struct Dependency {
    id: String,
}
fn unlocks(plan: Plan) -> BTreeMap<String, usize> {
    let mut direct: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for b in plan.blocked {
        for dep in b.blocked_by {
            direct
                .entry(dep.id.chars().take(3).collect())
                .or_default()
                .insert(b.id.chars().take(3).collect());
        }
    }
    direct
        .keys()
        .map(|id| {
            let mut seen = BTreeSet::new();
            let mut todo = vec![id.clone()];
            while let Some(id) = todo.pop() {
                if let Some(children) = direct.get(&id) {
                    for child in children {
                        if seen.insert(child.clone()) {
                            todo.push(child.clone());
                        }
                    }
                }
            }
            (id.clone(), seen.len())
        })
        .collect()
}
fn plan(directory: &Path) -> Result<BTreeMap<String, usize>, HostError> {
    let root =
        git_text(directory, &["rev-parse", "--show-toplevel"]).map_err(HostError::Unreachable)?;
    let local = Path::new(&root).join(".bin/spec-spine");
    #[cfg(unix)]
    let executable = {
        use std::os::unix::fs::PermissionsExt;
        local
            .metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    };
    #[cfg(not(unix))]
    let executable = local.is_file();
    let output = Command::new(if executable {
        local.as_os_str()
    } else {
        std::ffi::OsStr::new("spec-spine")
    })
    .args(["registry", "plan", "--json"])
    .current_dir(root)
    .output()
    .map_err(|_| HostError::Unreachable("could not execute spec-spine".into()))?;
    if !output.status.success() {
        return Err(HostError::Unreachable(
            "spec-spine registry plan failed".into(),
        ));
    }
    let plan = serde_json::from_slice(&output.stdout).map_err(|_| malformed())?;
    Ok(unlocks(plan))
}
fn generated_reviewed(generated: &Value, excludes: &[String]) -> Vec<String> {
    generated
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|g| g["paths"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .filter(|glob| {
            let prefix = glob.split(['*', '?', '[']).next().unwrap_or("");
            !excludes.iter().any(|e| prefix.starts_with(e))
        })
        .map(str::to_owned)
        .collect()
}
// Canonical GitHub UTC timestamps, validated by the arm reader before arithmetic.
fn leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
fn months(year: i64) -> [i64; 12] {
    [
        31,
        if leap(year) { 29 } else { 28 },
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
    ]
}
fn seconds(stamp: &str) -> Result<i64, HostError> {
    timestamp(stamp).ok_or_else(malformed)?;
    let n = |a, b| stamp[a..b].parse::<i64>().map_err(|_| malformed());
    let year = n(0, 4)?;
    let days = if year >= 1970 {
        (1970..year)
            .map(|y| if leap(y) { 366 } else { 365 })
            .sum::<i64>()
    } else {
        -(year..1970)
            .map(|y| if leap(y) { 366 } else { 365 })
            .sum::<i64>()
    };
    let days = days
        + months(year)
            .iter()
            .take((n(5, 7)? - 1) as usize)
            .sum::<i64>()
        + n(8, 10)?
        - 1;
    Ok(days * 86400 + n(11, 13)? * 3600 + n(14, 16)? * 60 + n(17, 19)?)
}
fn round(n: f64) -> f64 {
    (n * 10.0).round() / 10.0
}
fn hours(now: i64, stamp: &str) -> Result<f64, HostError> {
    Ok((now - seconds(stamp)?) as f64 / 3600.0)
}
fn rank(actions: &mut [Value]) {
    actions.sort_by_key(|a| {
        (
            RANK.iter()
                .position(|r| a["action"] == *r)
                .unwrap_or(RANK.len()),
            std::cmp::Reverse(a["unlocks"].as_u64().unwrap_or(0)),
            a["created"].as_str().unwrap_or("").to_owned(),
            a["pullRequest"].as_u64().unwrap_or(0),
        )
    });
}
fn cap(spec: Option<&str>, caps: &Value, default: u64) -> u64 {
    spec.and_then(|s| caps[s].as_u64()).unwrap_or(default)
}
/// The validated values the report uses, read once from the supplied policy.
struct Limits<'a> {
    default: &'a str,
    depth_cap: u64,
    caps: &'a Value,
    max_stacked: u64,
    generated: Value,
}
impl<'a> Limits<'a> {
    fn from(policy: &'a UsedPolicy) -> Option<Self> {
        Some(Self {
            default: policy.default_branch.as_ref()?.value.as_str()?,
            depth_cap: policy.stack_depth.as_ref()?.value.as_u64()?,
            caps: &policy.stack_depth_by_spec.as_ref()?.value,
            max_stacked: policy.stack_max_stacked.as_ref()?.value.as_u64()?,
            generated: policy
                .generated
                .as_ref()
                .map(|g| g.value.clone())
                .unwrap_or(Value::Null),
        })
    }
}
fn collect(
    report: &mut Value,
    directory: &Path,
    repo: &str,
    limits: &Limits,
    excludes: &[String],
    now: i64,
) -> Result<(), HostError> {
    let Limits {
        default,
        depth_cap,
        caps,
        max_stacked,
        ..
    } = *limits;
    let remote = Remote::new();
    let prs = open(&remote, repo)?;
    let heads = prs.iter().map(|p| p.head_ref_oid.as_str()).collect();
    let waiting = runs::waiting(&remote, repo, &heads)?;
    for run in &waiting {
        seconds(&run.created_at)?;
    }
    let unlocks = plan(directory)?;
    let mut actions = Vec::new();
    for pr in &prs {
        let checks = read_checks(&remote, repo, pr.number, &pr.head_ref_oid, false)?;
        // Multiple attempts at one head: show the newest one, measure every wait.
        let wait = waiting
            .iter()
            .filter(|r| r.head_sha == pr.head_ref_oid)
            .max_by_key(|r| (&r.created_at, r.id));
        let preliminary = classify(pr, &prs, default, &checks, wait.is_some(), None)?;
        let verdict = if preliminary.as_ref().is_none_or(|(a, _)| *a == "approve") {
            Some(review(&remote, repo, directory, pr)?)
        } else {
            None
        };
        let (action, detail) = match preliminary {
            Some(a) => a,
            None => classify(
                pr,
                &prs,
                default,
                &checks,
                wait.is_some(),
                verdict.as_deref(),
            )?
            .ok_or_else(malformed)?,
        };
        let (depth, bottom) = stack(pr, &prs, default);
        let spec = spec(&pr.title);
        let cap = cap(spec, caps, depth_cap);
        let mut item = json!({"pullRequest":pr.number,"title":pr.title,"url":pr.url,"spec":spec,"unlocks":spec.and_then(|s| unlocks.get(s)).copied().unwrap_or(0),"base":pr.base_ref_name,"depth":depth,"bottom":bottom,"cap":cap,"landFirst":if depth as u64 > cap {Some(bottom)} else {None},"action":action,"detail":detail,"review":verdict,"created":pr.created_at});
        if action == "approve" {
            let run = wait.ok_or_else(malformed)?;
            item["reason"] = json!(approval_reason(
                &remote,
                repo,
                pr.number,
                verdict.as_deref().ok_or_else(malformed)?
            )?);
            item["run"] = json!(run.html_url);
            item["waitHours"] = json!(round(hours(now, &run.created_at)?));
        }
        actions.push(item);
    }
    rank(&mut actions);
    let stacked = actions
        .iter()
        .filter(|a| a["depth"].as_u64().unwrap_or(0) > 0)
        .count();
    let over = stacked as u64 > max_stacked;
    let bottoms: BTreeSet<_> = actions
        .iter()
        .filter(|a| !a["landFirst"].is_null() || (over && a["depth"].as_u64().unwrap_or(0) > 0))
        .map(|a| a["bottom"].as_u64().unwrap_or(0))
        .collect();
    report["stack"] = json!({"stacked":stacked,"maxStacked":max_stacked,"deepest":actions.iter().map(|a| a["depth"].as_u64().unwrap_or(0)).max().unwrap_or(0),"landFirst":bottoms,"overStacked":over});
    report["supersededRuns"] = json!(
        waiting
            .iter()
            .filter(|r| r.superseded)
            .map(|r| json!({"id":r.id,"headSha":r.head_sha,"url":r.html_url}))
            .collect::<Vec<_>>()
    );
    report["generatedReviewed"] = json!(generated_reviewed(&limits.generated, excludes));
    report["actions"] = json!(actions);
    Ok(())
}
/// Read status at an injected instant. No attempt binding is consulted and no write is performed.
pub fn run(directory: &Path, args: &[String], now: SystemTime) -> Answer {
    let mut answer = Answer {
        exit_code: 0,
        report: json!({"outcome":"read","exitCode":0,"reason":null,"policy":{"generated":null},"actions":[],"stack":null,"supersededRuns":[],"generatedReviewed":[]}),
    };
    let args = match arguments(args) {
        Ok(a) => a,
        Err(e) => return answer.end("usage", 3, Some(e)),
    };
    // As arm and push: no committed declaration supplies no key, so it refuses.
    let manifest = match committed_declaration(directory) {
        Ok(v) => v,
        Err(e) => return answer.end("refused", 2, Some(e)),
    };
    let (policy, _) = match declared_policy(&manifest) {
        Ok(p) => p,
        Err(e) => return answer.end("refused", 2, Some(e)),
    };
    let mut required = match policy.require(&[
        "default_branch",
        "stack.depth",
        "stack.depth_by_spec",
        "stack.max_stacked",
    ]) {
        Ok(p) => p,
        Err(e) => return answer.end("refused", 2, Some(e)),
    };
    let used = UsedPolicy {
        default_branch: required.remove("default_branch"),
        stack_depth: required.remove("stack.depth"),
        stack_depth_by_spec: required.remove("stack.depth_by_spec"),
        stack_max_stacked: required.remove("stack.max_stacked"),
        generated: policy
            .require(&["generated"])
            .ok()
            .and_then(|mut p| p.remove("generated")),
        ..UsedPolicy::default()
    };
    answer.report["policy"] = serde_json::to_value(&used).expect("serializable policy");
    if used.generated.is_none() {
        answer.report["policy"]["generated"] = Value::Null;
    }
    let excludes = if used.generated.is_some() {
        match manifest
            .project
            .setup
            .as_ref()
            .and_then(|s| s.parameters.get("review.exclude"))
        {
            None => vec![],
            Some(v) => match serde_json::from_value::<Vec<String>>(v.clone()) {
                Ok(v) => v,
                Err(_) => return answer.end("failed", 4, Some("malformed review.exclude".into())),
            },
        }
    } else {
        vec![]
    };
    let repo = match repository(&args, directory) {
        Ok(r) => r,
        Err(e) => return answer.end("usage", 3, Some(e)),
    };
    let now = match now
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs()).ok())
    {
        Some(n) => n,
        None => return answer.end("failed", 4, Some("invalid clock".into())),
    };
    let Some(limits) = Limits::from(&used) else {
        return answer.end("refused", 2, Some("malformed landing policy".into()));
    };
    match collect(
        &mut answer.report,
        directory,
        &repo,
        &limits,
        &excludes,
        now,
    ) {
        Err(e) => answer.end("failed", 4, Some(e.reason())),
        Ok(()) => {
            let finding = answer.report["actions"].as_array().is_some_and(|a| {
                a.iter()
                    .any(|a| matches!(a["action"].as_str(), Some("fix" | "hazard")))
            }) || answer.report["generatedReviewed"]
                .as_array()
                .is_some_and(|g| !g.is_empty());
            answer.end(
                if finding { "finding" } else { "read" },
                i32::from(finding),
                None,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pr(n: u64, base: &str) -> PullRequest {
        serde_json::from_value(json!({"number":n,"title":"spec(041): delivery","url":"url","createdAt":"2026-10-20T00:00:00Z","isDraft":false,"baseRefName":base,"headRefName":format!("p{n}"),"headRefOid":format!("h{n}"),"headRepository":{"nameWithOwner":"o/r"},"mergeStateStatus":"CLEAN","autoMergeRequest":null,"mergeQueueEntry":null})).unwrap()
    }
    #[test]
    fn every_class_and_precedence() {
        let parent = pr(1, "main");
        let parents = vec![parent];
        let mut p = pr(2, "gone");
        let mut checks = Checks {
            failed: vec!["ci: FAILURE".into(); 6],
            pending: 1,
        };
        let action = |p: &PullRequest, c: &Checks, waiting, review| {
            classify(p, &parents, "main", c, waiting, review)
                .unwrap()
                .unwrap()
                .0
        };
        assert_eq!(action(&p, &checks, true, None), "retarget");
        p.base_ref_name = "p1".into();
        assert_eq!(action(&p, &checks, true, None), "stacked");
        p.auto_merge_request = Some(json!({}));
        assert_eq!(action(&p, &checks, true, None), "hazard");
        p.base_ref_name = "main".into();
        p.merge_queue_entry = Some(json!({"state":"QUEUED","position":25}));
        assert_eq!(action(&p, &checks, true, None), "merging");
        p.merge_queue_entry = Some(json!({"state":"UNMERGEABLE","position":25}));
        assert_eq!(action(&p, &checks, true, None), "fix");
        p.merge_queue_entry = None;
        let detail = classify(&p, &parents, "main", &checks, true, None)
            .unwrap()
            .unwrap()
            .1;
        assert!(detail.contains("ci (FAILURE)"));
        assert!(detail.ends_with("and 2 more; an owner approval is also waiting"));
        checks.failed.clear();
        p.is_draft = true;
        assert_eq!(action(&p, &checks, true, None), "approve");
        assert_eq!(action(&p, &checks, false, None), "draft");
        p.is_draft = false;
        assert_eq!(action(&p, &checks, false, None), "running");
        checks.pending = 0;
        assert_eq!(action(&p, &checks, false, Some("pending")), "running");
        for word in ["findings", "skipped:transient", "absent", "stale"] {
            assert_eq!(action(&p, &checks, false, Some(word)), "hazard");
            p.auto_merge_request = None;
            assert_eq!(action(&p, &checks, false, Some(word)), "fix");
            p.auto_merge_request = Some(json!({}));
        }
        p.merge_state_status = "BEHIND".into();
        assert_eq!(action(&p, &checks, false, Some("no-findings")), "merging");
        p.auto_merge_request = None;
        assert_eq!(action(&p, &checks, false, Some("no-findings")), "update");
        p.merge_state_status = "CLEAN".into();
        assert_eq!(action(&p, &checks, false, Some("no-findings")), "arm");
    }
    #[test]
    fn depth_cycles_spec_and_generated_prefixes() {
        let mut prs = vec![pr(1, "trunk"), pr(2, "p1"), pr(3, "p2")];
        assert_eq!(stack(&prs[2], &prs, "trunk"), (2, 1));
        assert_eq!(spec(&prs[2].title), Some("041"));
        assert_eq!(spec("plain title"), None);
        prs[0].base_ref_name = "p3".into();
        assert_eq!(stack(&prs[2], &prs, "trunk").0, 2);
        let g = json!([{"paths":["derived/**","src/?.rs","[ab]/x","other/file"]}]);
        assert_eq!(
            generated_reviewed(&g, &["derived/".into(), "src/".into()]),
            vec!["[ab]/x", "other/file"]
        );
    }
    #[test]
    fn ranking_and_per_spec_caps() {
        assert_eq!(cap(Some("041"), &json!({}), 1), 1);
        assert_eq!(cap(Some("041"), &json!({"041":3}), 1), 3);
        assert_eq!(cap(None, &json!({"041":3}), 1), 1);
        let mut actions: Vec<_> = RANK.iter().rev().map(|a| json!({"action":a,"unlocks":0,"created":"2026-10-20T00:00:00Z","pullRequest":1})).collect();
        rank(&mut actions);
        assert_eq!(
            actions
                .iter()
                .map(|a| a["action"].as_str().unwrap())
                .collect::<Vec<_>>(),
            RANK
        );
        let mut actions = vec![
            json!({"action":"arm","unlocks":1,"created":"b","pullRequest":1}),
            json!({"action":"arm","unlocks":2,"created":"z","pullRequest":2}),
            json!({"action":"arm","unlocks":1,"created":"a","pullRequest":4}),
            json!({"action":"arm","unlocks":1,"created":"a","pullRequest":3}),
        ];
        rank(&mut actions);
        assert_eq!(
            actions
                .iter()
                .map(|a| a["pullRequest"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            vec![2, 3, 4, 1]
        );
    }
    #[test]
    fn transitive_unlocks() {
        let weights = unlocks(serde_json::from_value(json!({"blocked":[{"id":"002-b","blockedBy":[{"id":"001-a"}]},{"id":"003-c","blockedBy":[{"id":"002-b"},{"id":"001-a"}]}]})).unwrap());
        assert_eq!(weights["001"], 2);
    }
}
