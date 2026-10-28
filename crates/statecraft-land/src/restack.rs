//! Replay the host's commit list, preserving authored changes (spec 063 3.5).
use crate::{Attempt, UsedPolicy, action::*, push};
use serde::Serialize;
use serde_json::{Value, json};
use statecraft_home::setup::classify_gh_failure;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    process::Command,
};

/// One restack, or an ordered chain of attempted restacks.
#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    /// Stable outcome word.
    pub outcome: String,
    /// Requested pull request.
    pub pull_request: Option<u64>,
    /// Head before replay, retained even when inspection is required.
    pub original_head: Option<String>,
    /// Current replayed head, once known.
    pub head: Option<String>,
    /// Target branch.
    pub onto: Option<String>,
    /// Commits successfully replayed.
    pub replayed: usize,
    /// Conflicted generated paths that took the base copy.
    pub regenerated: Vec<String>,
    /// Authored conflicted paths.
    pub conflicts: Vec<String>,
    /// Paths with different authored line changes.
    pub differs: Vec<String>,
    /// This invocation disabled auto-merge.
    pub disarmed: bool,
    /// This invocation changed the pull request's base.
    pub retargeted: bool,
    /// Remote head after a lease rejection.
    pub remote_head: Option<String>,
    /// Failure or refusal explanation.
    pub reason: Option<String>,
    /// Committed policy and provenance.
    pub policy: UsedPolicy,
    /// Attempted chain members, in order, never keyed by number.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub restacks: Vec<Answer>,
    /// Family exit code.
    #[serde(skip)]
    pub exit_code: i32,
}
struct Failure {
    word: &'static str,
    code: i32,
    reason: String,
}
impl Failure {
    fn new(word: &'static str, code: i32, reason: impl Into<String>) -> Self {
        Self {
            word,
            code,
            reason: reason.into(),
        }
    }
}
impl From<String> for Failure {
    fn from(reason: String) -> Self {
        Self::new("failed", 4, reason)
    }
}
impl Answer {
    fn fail(&mut self, failure: Failure) {
        self.outcome = failure.word.into();
        self.exit_code = failure.code;
        self.reason = Some(failure.reason);
    }
}
struct Args {
    prs: Vec<u64>,
    onto: Option<String>,
    repo: Option<String>,
    chain: bool,
}
fn parse(args: &[String]) -> Result<Args, String> {
    let mut result = Args {
        prs: vec![],
        onto: None,
        repo: None,
        chain: false,
    };
    let mut words = args.iter();
    let mut json = false;
    while let Some(word) = words.next() {
        match word.as_str() {
            "--json" if !json => json = true,
            "--chain" if !result.chain && result.prs.is_empty() => result.chain = true,
            "--onto" if result.onto.is_none() => {
                result.onto = Some(
                    words
                        .next()
                        .filter(|s| !s.is_empty() && !s.starts_with('-'))
                        .ok_or("missing branch")?
                        .clone(),
                );
            }
            "--repo" if result.repo.is_none() => {
                result.repo = Some(words.next().ok_or("missing repository")?.clone())
            }
            n if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
                result.prs.push(
                    n.parse::<u64>()
                        .ok()
                        .filter(|n| *n > 0)
                        .ok_or("expected positive PR number")?,
                );
            }
            _ => return Err("unexpected or duplicate argument".into()),
        }
    }
    if (result.chain && (result.prs.len() < 2 || result.onto.is_some()))
        || (!result.chain && result.prs.len() != 1)
    {
        return Err("land restack <pr> [--onto <branch>] or land restack --chain <pr> <pr>... [--repo owner/name] [--json]".into());
    }
    // Reuse the action argument validation for repository names and PR numbers.
    let mut common = vec![result.prs[0].to_string()];
    if let Some(repo) = &result.repo {
        common.extend(["--repo".into(), repo.clone()]);
    }
    arguments(&common, false)?;
    Ok(result)
}
fn field<'a>(value: &'a Value, key: &str) -> Result<&'a str, Failure> {
    string(value, key).map_err(|e| Failure::from(e.reason()))
}
fn clean(root: &Path) -> Result<(), Failure> {
    if !git_text(root, &["status", "--porcelain", "--untracked-files=all"])?.is_empty() {
        return Err(Failure::new(
            "dirty",
            2,
            "restack requires a clean worktree",
        ));
    }
    Ok(())
}
fn branch(root: &Path) -> Result<String, Failure> {
    git_text(root, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .map_err(|_| Failure::new("wrong-branch", 2, "no checked-out branch"))
}
fn valid_branch(root: &Path, name: &str) -> Result<(), Failure> {
    if name.starts_with('-')
        || !git(root, &["check-ref-format", &format!("refs/heads/{name}")])?
            .status
            .success()
    {
        return Err(Failure::new("refused", 2, "invalid branch name"));
    }
    Ok(())
}
struct Policy {
    used: UsedPolicy,
    signed: bool,
    patterns: Vec<String>,
    regenerators: Vec<Vec<String>>,
    pre_push: Value,
    default: String,
}
fn policy(root: &Path) -> Result<Policy, Failure> {
    let refused = |e| Failure::new("refused", 2, e);
    let (policy, signed) = declaration(root).map_err(refused)?;
    let mut supplied = policy
        .require(&["default_branch", "generated", "pre_push"])
        .map_err(refused)?;
    let default = supplied
        .get("default_branch")
        .and_then(|s| s.value.as_str())
        .ok_or_else(|| refused("malformed landing.default_branch".into()))?
        .to_owned();
    let pre_push = supplied
        .get("pre_push")
        .ok_or_else(|| refused("unsupplied landing.pre_push".into()))?
        .value
        .clone();
    // Validated here, before any write, with the rule `land push` applies.
    push::vectors(&pre_push).map_err(refused)?;
    let entries = supplied
        .get("generated")
        .and_then(|s| s.value.as_array())
        .ok_or_else(|| refused("malformed landing.generated".into()))?;
    let mut patterns = vec![];
    let mut regenerators = vec![];
    for entry in entries {
        let strings = |key| -> Result<Vec<String>, Failure> {
            entry[key]
                .as_array()
                .and_then(|a| a.iter().map(|s| s.as_str().map(str::to_owned)).collect())
                .ok_or_else(|| refused("malformed landing.generated".into()))
        };
        patterns.extend(strings("paths")?);
        regenerators.push(strings("regenerate")?);
    }
    Ok(Policy {
        used: UsedPolicy {
            default_branch: supplied.remove("default_branch"),
            generated: supplied.remove("generated"),
            pre_push: supplied.remove("pre_push"),
        },
        signed,
        patterns,
        regenerators,
        pre_push,
        default,
    })
}
fn commits(remote: &Remote, repo: &str, number: u64) -> Result<Vec<String>, Failure> {
    let mut result = vec![];
    let mut cursor = None;
    let mut cursors = BTreeSet::new();
    loop {
        let after = cursor
            .as_ref()
            .map(|s| format!(",after:{}", json!(s)))
            .unwrap_or_default();
        let value = remote.query(repo, &format!("pullRequest(number:{number}) {{ commits(first:100{after}) {{ nodes {{ commit {{ oid }} }} pageInfo {{ hasNextPage endCursor }} }} }}")).map_err(|e| e.reason())?;
        let page = &value["pullRequest"]["commits"];
        let nodes = page["nodes"]
            .as_array()
            .ok_or_else(|| "invalid PR commit list".to_owned())?;
        for node in nodes {
            let oid = field(&node["commit"], "oid")?;
            if !matches!(oid.len(), 40 | 64)
                || !oid.bytes().all(|b| b.is_ascii_hexdigit())
                || result.iter().any(|s| s == oid)
            {
                return Err("invalid or repeated PR commit oid".to_owned().into());
            }
            result.push(oid.to_owned());
        }
        match page["pageInfo"]["hasNextPage"].as_bool() {
            Some(false) => break,
            Some(true) => {
                let next = field(&page["pageInfo"], "endCursor")?.to_owned();
                if nodes.is_empty() || !cursors.insert(next.clone()) {
                    return Err("non-advancing PR commit page".to_owned().into());
                }
                cursor = Some(next);
            }
            None => return Err("invalid PR commit page".to_owned().into()),
        }
    }
    Ok(result)
}
fn paths(root: &Path, args: &[&str]) -> Result<Vec<String>, Failure> {
    let output = git(root, args)?;
    if !output.status.success() {
        return Err("git path listing failed".to_owned().into());
    }
    output
        .stdout
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8(s.to_vec()).map_err(|_| "non-UTF-8 git path".to_owned().into()))
        .collect()
}
fn conflicts(root: &Path, patterns: &[String], answer: &mut Answer) -> Result<(), Failure> {
    let all = paths(root, &["diff", "--name-only", "-z", "--diff-filter=U"])?;
    let mut generated = BTreeSet::new();
    for pattern in patterns {
        generated.extend(paths(
            root,
            &[
                "diff",
                "--name-only",
                "-z",
                "--diff-filter=U",
                "--",
                &format!(":(glob){pattern}"),
            ],
        )?);
    }
    answer.conflicts = all
        .iter()
        .filter(|p| !generated.contains(*p))
        .cloned()
        .collect();
    if !answer.conflicts.is_empty() {
        return Err(Failure::new(
            "conflict",
            1,
            format!("authored conflicts: {}", answer.conflicts.join(", ")),
        ));
    }
    for path in generated {
        // If the base deleted this path, its copy is absence.
        let literal = format!(":(literal){path}");
        if paths(root, &["ls-files", "-z", "--stage", "--", &literal])?
            .iter()
            .any(|s| {
                s.split_once('\t')
                    .is_some_and(|(stage, _)| stage.ends_with(" 2"))
            })
        {
            git_text(root, &["checkout", "--ours", "--", &literal])?;
            git_text(root, &["add", "--", &literal])?;
        } else {
            git_text(root, &["rm", "-f", "--", &literal])?;
        }
        if !answer.regenerated.contains(&path) {
            answer.regenerated.push(path);
        }
    }
    Ok(())
}
// Compare multisets of changed lines per literal path. Git handles exclusion
// globs and path names; zero-context hunks keep metadata out of the multiset.
fn authored(
    root: &Path,
    base: &str,
    head: &str,
    patterns: &[String],
) -> Result<BTreeMap<String, Vec<Vec<u8>>>, Failure> {
    let exclusions: Vec<_> = patterns
        .iter()
        .map(|p| format!(":(exclude,glob){p}"))
        .collect();
    let mut args = vec![
        "diff",
        "--no-renames",
        "--name-only",
        "-z",
        base,
        head,
        "--",
        ".",
    ];
    args.extend(exclusions.iter().map(String::as_str));
    let mut result = BTreeMap::new();
    for path in paths(root, &args)? {
        let output = git(
            root,
            &[
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--no-renames",
                "--no-color",
                "--text",
                "-U0",
                base,
                head,
                "--",
                &format!(":(literal){path}"),
            ],
        )?;
        if !output.status.success() {
            return Err("git authored diff failed".to_owned().into());
        }
        let mut hunk = false;
        let mut lines = vec![];
        for line in output.stdout.split(|b| *b == b'\n') {
            if line.starts_with(b"@@ ") {
                hunk = true;
            } else if hunk && matches!(line.first(), Some(b'+' | b'-')) {
                lines.push(line.to_vec());
            }
        }
        lines.sort();
        if !lines.is_empty() {
            result.insert(path, lines);
        }
    }
    Ok(result)
}
// All replay failures use this single restore path. The starting tree was
// clean, so non-ignored untracked files were created during this replay.
fn restore(root: &Path, original_branch: &str, original_head: &str) -> Result<(), String> {
    // --no-commit can leave CHERRY_PICK_HEAD or sequencer state after a conflict.
    git_text(root, &["reset", "--hard", original_head])?;
    git_text(root, &["cherry-pick", "--quit"])?;
    git_text(root, &["clean", "-fd"])?;
    git_text(root, &["switch", "-C", original_branch, original_head])?;
    if git_text(root, &["rev-parse", "HEAD"])? != original_head {
        return Err("restore found the original branch moved".into());
    }
    if !git_text(root, &["status", "--porcelain", "--untracked-files=all"])?.is_empty() {
        return Err("restore left a dirty worktree".into());
    }
    Ok(())
}
struct Replay<'a> {
    root: &'a Path,
    commits: &'a [String],
    policy: &'a Policy,
}
impl Replay<'_> {
    fn run(&self, answer: &mut Answer) -> Result<String, Failure> {
        for oid in self.commits {
            let picked = git(
                self.root,
                &[
                    "-c",
                    "core.editor=true",
                    "-c",
                    "sequence.editor=true",
                    "-c",
                    "rerere.enabled=false",
                    "cherry-pick",
                    "--no-commit",
                    oid,
                ],
            )?;
            let unmerged = paths(self.root, &["diff", "--name-only", "-z", "--diff-filter=U"])?;
            if !unmerged.is_empty() {
                conflicts(self.root, &self.policy.patterns, answer)?;
            } else if !picked.status.success() {
                return Err(format!("cherry-pick failed for {oid}").into());
            }
            for vector in &self.policy.regenerators {
                push::run_vector(
                    self.root,
                    &vector.iter().map(String::as_str).collect::<Vec<_>>(),
                )
                .map_err(|e| Failure::new("failed", 4, format!("regenerator at {oid}: {e}")))?;
            }
            push::run_vectors(self.root, &self.policy.pre_push)
                .map_err(|e| Failure::new("pre-push-failed", 2, format!("commit {oid}: {e}")))?;
            git_text(self.root, &["add", "-A"])?;
            if let Some(reason) = push::markers(self.root, "--cached")? {
                return Err(Failure::new("markers", 2, reason));
            }
            let mut args = vec![
                "-c",
                "core.editor=true",
                "commit",
                "-C",
                oid,
                "--allow-empty",
            ];
            if self.policy.signed {
                args.push("-S");
            }
            git_text(self.root, &args)?;
            answer.replayed += 1;
        }
        Ok(git_text(self.root, &["rev-parse", "HEAD"])?)
    }
}
fn single(
    root: &Path,
    repo: &str,
    number: u64,
    onto: Option<&str>,
    switch: bool,
    answer: &mut Answer,
) -> Result<String, Failure> {
    let policy = policy(root)?;
    // Take the policy before a chain switches branches; each later invocation
    // resolves its policy at that member's HEAD below.
    answer.policy = UsedPolicy {
        default_branch: policy.used.default_branch.clone(),
        generated: policy.used.generated.clone(),
        pre_push: policy.used.pre_push.clone(),
    };
    clean(root)?;
    let remote = Remote::new();
    let pr = remote.pr(repo, number).map_err(|e| e.reason())?;
    let head_branch = field(&pr, "headRefName")?.to_owned();
    valid_branch(root, &head_branch)?;
    if field(&pr["headRepository"], "nameWithOwner")? != repo {
        return Err(Failure::new(
            "wrong-branch",
            2,
            format!("PR head must belong to {repo}"),
        ));
    }
    if switch && branch(root)? != head_branch {
        let reference = format!("refs/heads/{head_branch}");
        if !git(root, &["show-ref", "--verify", "--quiet", &reference])?
            .status
            .success()
        {
            return Err(Failure::new(
                "wrong-branch",
                2,
                format!("no local branch {head_branch}"),
            ));
        }
        git_text(root, &["switch", &head_branch])?;
    }
    if branch(root)? != head_branch {
        return Err(Failure::new(
            "wrong-branch",
            2,
            format!("expected checked-out branch {head_branch}"),
        ));
    }
    let original = git_text(root, &["rev-parse", "HEAD"])?;
    answer.original_head = Some(original.clone());
    answer.head = Some(original.clone());
    if original != field(&pr, "headRefOid")? {
        return Err(Failure::new(
            "stale",
            2,
            format!(
                "local HEAD {original}; pull request head {}",
                field(&pr, "headRefOid")?
            ),
        ));
    }
    let policy = if switch {
        policy_for_member(root, answer)?
    } else {
        policy
    };
    let commits = commits(&remote, repo, number)?;
    let first = commits
        .first()
        .ok_or_else(|| "empty PR commit list".to_owned())?;
    if commits.last() != Some(&original) {
        return Err(Failure::new(
            "stale",
            2,
            "PR commit list does not end at headRefOid",
        ));
    }
    for oid in &commits {
        if !git(root, &["merge-base", "--is-ancestor", oid, &original])?
            .status
            .success()
        {
            return Err(Failure::new(
                "stale",
                2,
                format!("PR commit {oid} is not in local history at {original}"),
            ));
        }
    }
    let old_base = git_text(root, &["rev-parse", &format!("{first}^")])?;
    let base = field(&pr, "baseRefName")?;
    let onto = match onto {
        Some(name) => name.to_owned(),
        None => {
            let value = remote.query(repo, &format!("pullRequests(headRefName:{},states:MERGED,first:1) {{ nodes {{ number }} }}", json!(base))).map_err(|e| e.reason())?;
            let merged = value["pullRequests"]["nodes"]
                .as_array()
                .ok_or_else(|| "invalid merged-base response".to_owned())?;
            if merged.is_empty() {
                base.to_owned()
            } else {
                policy.default.clone()
            }
        }
    };
    valid_branch(root, &onto)?;
    answer.onto = Some(onto.clone());
    git_text(root, &["fetch", "origin", &onto])?;
    let new_base = git_text(root, &["rev-parse", "FETCH_HEAD"])?;
    let lease = push::remote_head(root, &head_branch)?;
    let before = authored(root, &old_base, &original, &policy.patterns)?;
    // From this point, every unsuccessful replay restores through one function.
    let replay = (|| {
        git_text(root, &["switch", "--detach", &new_base])?;
        let head = Replay {
            root,
            commits: &commits,
            policy: &policy,
        }
        .run(answer)?;
        let after = authored(root, &new_base, &head, &policy.patterns)?;
        answer.differs = before
            .keys()
            .chain(after.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|p| before.get(*p) != after.get(*p))
            .cloned()
            .collect();
        Ok::<_, Failure>(head)
    })();
    let head = match replay {
        Ok(h) => h,
        Err(e) => {
            if let Err(restore_error) = restore(root, &head_branch, &original) {
                return Err(format!("{}; restore failed: {restore_error}", e.reason).into());
            }
            answer.head = Some(original);
            return Err(e);
        }
    };
    // Attach the completed replay, including an authored mismatch for inspection.
    if let Err(e) = git_text(root, &["switch", "-C", &head_branch, &head]) {
        if let Err(restore_error) = restore(root, &head_branch, &original) {
            return Err(format!("{e}; restore failed: {restore_error}").into());
        }
        return Err(e.into());
    }
    answer.head = Some(head);
    if !answer.differs.is_empty() {
        return Err(Failure::new(
            "authored-change-differs",
            1,
            format!("authored change differs: {}", answer.differs.join(", ")),
        ));
    }
    // Re-read arming immediately before publishing, since replay can take time.
    let current = remote.pr(repo, number).map_err(|e| e.reason())?;
    push::disarm(&remote, repo, number, &current, &mut answer.disarmed)?;
    if let Err(e) = push::lease_push(root, &head_branch, &lease) {
        answer.remote_head = e.remote_head;
        return Err(Failure::new(e.outcome, e.code, e.reason));
    }
    if base != onto {
        let output = Command::new(&remote.host.program)
            .args([
                "pr",
                "edit",
                &number.to_string(),
                "--repo",
                repo,
                "--base",
                &onto,
            ])
            .output()
            .map_err(|_| "could not execute gh".to_owned())?;
        if !output.status.success() {
            return Err(
                classify_gh_failure(&String::from_utf8_lossy(&output.stderr))
                    .reason()
                    .into(),
            );
        }
        answer.retargeted = true;
    }
    answer.outcome = "restacked".into();
    Ok(head_branch)
}
fn policy_for_member(root: &Path, answer: &mut Answer) -> Result<Policy, Failure> {
    let policy = policy(root)?;
    answer.policy = UsedPolicy {
        default_branch: policy.used.default_branch.clone(),
        generated: policy.used.generated.clone(),
        pre_push: policy.used.pre_push.clone(),
    };
    Ok(policy)
}
/// Restack explicitly requested pull requests, never inside a driven attempt.
pub fn run(directory: &Path, args: &[String], attempt: Attempt) -> Answer {
    let mut answer = Answer::default();
    if attempt.inside() {
        answer.fail(Failure::new(
            "inside-run-attempt",
            2,
            "landing acts are forbidden inside a run attempt",
        ));
        return answer;
    }
    let args = match parse(args) {
        Ok(a) => a,
        Err(e) => {
            answer.fail(Failure::new("usage", 3, e));
            return answer;
        }
    };
    if !args.chain {
        answer.pull_request = Some(args.prs[0]);
    }
    let result = (|| {
        // Missing policy precedes repository discovery or any host read.
        policy(directory)?;
        let repo = repository(
            &Arguments {
                pr: args.prs[0],
                sha: None,
                repo: args.repo,
            },
            directory,
        )
        .map_err(|e| Failure::new("usage", 3, e))?;
        let root = git_text(directory, &["rev-parse", "--show-toplevel"])?;
        let root = Path::new(&root);
        if !args.chain {
            answer.pull_request = Some(args.prs[0]);
            single(
                root,
                &repo,
                args.prs[0],
                args.onto.as_deref(),
                false,
                &mut answer,
            )?;
            return Ok(());
        }
        clean(root)?;
        let start = branch(root)?;
        let mut onto = None;
        for number in args.prs {
            let mut member = Answer {
                pull_request: Some(number),
                ..Answer::default()
            };
            let result = single(root, &repo, number, onto.as_deref(), true, &mut member);
            match result {
                Ok(head_branch) => onto = Some(head_branch),
                Err(e) => member.fail(e),
            }
            let stop = member.exit_code != 0;
            answer.exit_code = member.exit_code;
            answer.outcome = member.outcome.clone();
            answer.reason = member.reason.clone();
            answer.restacks.push(member);
            if stop {
                break;
            }
        }
        // A failed restore can leave replay state that needs operator inspection.
        // Returning to the starting branch is a courtesy after the chain has
        // published: its failure is reported, never the chain's outcome.
        if clean(root).is_ok()
            && branch(root).is_ok()
            && let Err(e) = git_text(root, &["switch", &start])
        {
            let note = format!("could not return to {start}: {e}");
            answer.reason = Some(match answer.reason.take() {
                Some(reason) => format!("{reason}; {note}"),
                None => note,
            });
        }
        Ok::<_, Failure>(())
    })();
    if let Err(e) = result {
        answer.fail(e);
    }
    answer
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attempt_precedes_usage_and_reads() {
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
            let answer = run(Path::new("/does-not-exist"), &[], attempt);
            assert_eq!(answer.outcome, "inside-run-attempt");
            assert_eq!(answer.exit_code, 2);
        }
    }
    #[test]
    fn usage_is_unambiguous() {
        for words in [
            vec![],
            vec!["--chain", "7"],
            vec!["--chain", "7", "8", "--onto", "main"],
            vec!["7", "8"],
            vec!["7", "--onto", "--repo"],
            vec!["7", "--repo", "bad"],
            vec!["0"],
        ] {
            assert!(parse(&words.iter().map(|s| s.to_string()).collect::<Vec<_>>()).is_err());
        }
        assert!(parse(&["--chain", "7", "8"].map(str::to_owned)).is_ok());
    }
}
