//! Guarded, disarm-before-push operation (spec 063 section 3.4).
use crate::{Attempt, UsedPolicy, action::*};
use serde::Serialize;
use serde_json::Value;
use std::{
    path::Path,
    process::{Command, Stdio},
};

/// One push answer, including disarming even when the subsequent push fails.
#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    /// Stable result word.
    pub outcome: String,
    /// Requested PR, if parsed.
    pub pull_request: Option<u64>,
    /// Local head to push, once known.
    pub head: Option<String>,
    /// Remote head after a lease refusal, when readable.
    pub remote_head: Option<String>,
    /// Auto-merge was disabled by this invocation.
    pub disarmed: bool,
    /// Refusal or failure explanation.
    pub reason: Option<String>,
    /// Used landing values with provenance.
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
// git's `%G?` for the local head. No signature (N), a bad one (B) and one made
// by a revoked key (R) are known not to be valid and refuse. A good signature
// (G, U), one whose key or signature has expired (X, Y) and one git cannot check
// here for want of the key (E, the usual case for SSH signing without an
// allowed-signers file) are signed: GitHub's verification, which `land arm`
// reads, is the verdict on them.
fn local_signature_refused(status: &str) -> bool {
    !matches!(status, "G" | "U" | "X" | "Y" | "E")
}
// Only the lease's own rejection: a hook or branch-protection rejection is a
// failed push, not a moved remote head.
fn lease_rejected(stderr: &[u8]) -> bool {
    String::from_utf8_lossy(stderr)
        .to_ascii_lowercase()
        .contains("stale info")
}
/// Push the worktree branch with the explicit lease. Never re-arm it.
pub fn run(directory: &Path, args: &[String], attempt: Attempt) -> Answer {
    let mut answer = Answer::default();
    if attempt.inside() {
        return answer.end(
            "inside-run-attempt",
            2,
            Some("landing acts are forbidden inside a run attempt".into()),
        );
    }
    let args = match arguments(args, true) {
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
    answer.policy.default_branch = match policy.require(&["default_branch"]) {
        Ok(mut p) => p.remove("default_branch"),
        Err(e) => return answer.end("refused", 2, Some(e)),
    };
    let remote = Remote::new();
    let pr = match remote.pr(&repo, args.pr) {
        Ok(p) => p,
        Err(e) => return answer.end("failed", 4, Some(e.reason())),
    };
    let branch = match git_text(directory, &["symbolic-ref", "--quiet", "--short", "HEAD"]) {
        Ok(b) => b,
        Err(_) => return answer.end("wrong-branch", 2, Some("no checked-out branch".into())),
    };
    if pr["headRefName"] != branch || pr["headRepository"]["nameWithOwner"] != repo {
        return answer.end(
            "wrong-branch",
            2,
            Some(format!(
                "checked-out branch {branch} must match the PR head in {repo}"
            )),
        );
    }
    answer.head = match git_text(directory, &["rev-parse", "HEAD"]) {
        Ok(h) => Some(h),
        Err(e) => return answer.end("failed", 4, Some(e)),
    };
    if signed {
        match git_text(directory, &["log", "-1", "--format=%G?", "HEAD"]) {
            Ok(s) if local_signature_refused(&s) => {
                return answer.end(
                    "unsigned",
                    2,
                    Some(format!("local head signature status {s}")),
                );
            }
            Ok(_) => {}
            Err(e) => return answer.end("failed", 4, Some(e)),
        }
    }
    match markers(directory, "HEAD") {
        Ok(Some(reason)) => return answer.end("markers", 2, Some(reason)),
        Ok(None) => {}
        Err(e) => return answer.end("failed", 4, Some(e)),
    }
    // `require` answers every key it was asked for, or refuses.
    let pre_push = match policy
        .require(&["pre_push"])
        .map(|mut p| p.remove("pre_push"))
    {
        Ok(Some(supplied)) => supplied,
        Ok(None) => {
            return answer.end("refused", 2, Some("unsupplied landing.pre_push".into()));
        }
        Err(e) => return answer.end("refused", 2, Some(e)),
    };
    answer.policy.pre_push = Some(pre_push.clone());
    if let Err(e) = vectors(&pre_push.value) {
        return answer.end("refused", 2, Some(e));
    }
    let root = match git_text(directory, &["rev-parse", "--show-toplevel"]) {
        Ok(r) => r,
        Err(e) => return answer.end("failed", 4, Some(e)),
    };
    if let Err(e) = run_vectors(Path::new(&root), &pre_push.value) {
        return answer.end("pre-push-failed", 2, Some(e));
    }
    let Some(lease) = args.sha else {
        return answer.end("usage", 3, Some("missing lease".into()));
    };
    if let Err(e) = disarm(&remote, &repo, args.pr, &pr, &mut answer.disarmed) {
        return answer.end("failed", 4, Some(e));
    }
    match lease_push(directory, &branch, &lease) {
        Ok(()) => answer.end("pushed", 0, None),
        Err(e) => {
            answer.remote_head = e.remote_head;
            answer.end(e.outcome, e.code, Some(e.reason))
        }
    }
}

// Shared by push (committed HEAD) and restack (staged replay tree).
pub(crate) fn markers(directory: &Path, tree: &str) -> Result<Option<String>, String> {
    let output = git(
        directory,
        &[
            "grep",
            "-l",
            "-z",
            "-a",
            "-E",
            "-e",
            "^(<<<<<<<( |$)|>>>>>>>( |$))",
            tree,
            "--",
        ],
    )?;
    match output.status.code() {
        Some(0) => Ok(Some(format!(
            "conflict marker in {}",
            String::from_utf8_lossy(output.stdout.split(|b| *b == 0).next().unwrap_or_default())
        ))),
        Some(1) => Ok(None),
        _ => Err("git grep failed".into()),
    }
}
pub(crate) fn vectors(value: &Value) -> Result<Vec<Vec<&str>>, String> {
    value
        .as_array()
        .and_then(|all| {
            all.iter()
                .map(|v| {
                    let argv = v
                        .as_array()?
                        .iter()
                        .map(Value::as_str)
                        .collect::<Option<Vec<_>>>()?;
                    (!argv.is_empty() && argv.iter().all(|s| !s.is_empty())).then_some(argv)
                })
                .collect()
        })
        .ok_or_else(|| "malformed landing.pre_push".into())
}
pub(crate) fn run_vectors(root: &Path, value: &Value) -> Result<(), String> {
    for argv in vectors(value)? {
        run_vector(root, &argv)?;
    }
    Ok(())
}
pub(crate) fn run_vector(root: &Path, argv: &[&str]) -> Result<(), String> {
    let Some(program) = argv.first() else {
        return Err("empty command vector".into());
    };
    let vector = serde_json::json!(argv);
    let status = Command::new(program)
        .args(&argv[1..])
        .current_dir(root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::from(std::io::stderr()))
        .stderr(Stdio::inherit())
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!(
            "{vector}: exit {}",
            s.code()
                .map(|n| n.to_string())
                .unwrap_or_else(|| "signal".into())
        )),
        Err(_) => Err(format!("{vector}: could not execute (no exit code)")),
    }
}
pub(crate) fn disarm(
    remote: &Remote,
    repo: &str,
    number: u64,
    pr: &Value,
    disarmed: &mut bool,
) -> Result<(), String> {
    if !pr["autoMergeRequest"].is_null() {
        remote
            .merge(repo, number, &["--disable-auto"])
            .map_err(|e| e.reason())?;
        *disarmed = true;
    }
    Ok(())
}
pub(crate) struct PushFailure {
    pub outcome: &'static str,
    pub code: i32,
    pub reason: String,
    pub remote_head: Option<String>,
}
pub(crate) fn remote_head(directory: &Path, branch: &str) -> Result<String, String> {
    let reference = format!("refs/heads/{branch}");
    git_text(directory, &["ls-remote", "origin", &reference])?
        .split_whitespace()
        .next()
        .map(str::to_owned)
        .ok_or_else(|| "remote head could not be read".into())
}
pub(crate) fn lease_push(directory: &Path, branch: &str, lease: &str) -> Result<(), PushFailure> {
    let failed = |reason| PushFailure {
        outcome: "failed",
        code: 4,
        reason,
        remote_head: None,
    };
    let lease = format!("--force-with-lease={branch}:{lease}");
    let output = git(directory, &["push", &lease, "origin", branch]).map_err(failed)?;
    if output.status.success() {
        return Ok(());
    }
    if lease_rejected(&output.stderr) {
        let remote_head = remote_head(directory, branch).ok();
        let reason = if remote_head.is_some() {
            "push lease was rejected"
        } else {
            "push lease was rejected; remote head could not be read"
        };
        return Err(PushFailure {
            outcome: "lease-refused",
            code: 2,
            reason: reason.into(),
            remote_head,
        });
    }
    Err(failed("git push failed".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_invalid_or_absent_local_signatures_refuse() {
        for status in ["N", "B", "R", "", "?"] {
            assert!(local_signature_refused(status), "{status:?}");
        }
        for status in ["G", "U", "X", "Y", "E"] {
            assert!(!local_signature_refused(status), "{status}");
        }
    }
    #[test]
    fn lease_errors_are_distinct_from_transport_failure() {
        assert!(lease_rejected(b"[rejected] branch (stale info)"));
        assert!(!lease_rejected(b"Could not resolve host"));
        assert!(!lease_rejected(
            b"! [remote rejected] branch -> branch (protected branch hook declined)"
        ));
    }
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
            assert_eq!(
                run(Path::new("/does-not-exist"), &[], attempt).outcome,
                "inside-run-attempt"
            );
        }
    }
}
