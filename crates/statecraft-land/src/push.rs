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
    let markers = match git(
        directory,
        &[
            "grep",
            "-l",
            "-z",
            "-a",
            "-E",
            "^(<<<<<<<( |$)|>>>>>>>( |$))",
            "HEAD",
            "--",
        ],
    ) {
        Ok(o) => o,
        Err(e) => return answer.end("failed", 4, Some(e)),
    };
    match markers.status.code() {
        Some(0) => {
            let first = markers.stdout.split(|b| *b == 0).next().unwrap_or_default();
            return answer.end(
                "markers",
                2,
                Some(format!(
                    "conflict marker in {}",
                    String::from_utf8_lossy(first)
                )),
            );
        }
        Some(1) => {}
        _ => return answer.end("failed", 4, Some("git grep failed".into())),
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
    // The policy validated each vector as a non-empty list of non-empty
    // strings; a shape that does not hold is refused, never run.
    let vectors: Option<Vec<(Value, Vec<String>)>> = pre_push.value.as_array().and_then(|all| {
        all.iter()
            .map(|vector| {
                let argv = vector
                    .as_array()?
                    .iter()
                    .map(|word| word.as_str().map(str::to_owned))
                    .collect::<Option<Vec<_>>>()?;
                (!argv.is_empty()).then(|| (vector.clone(), argv))
            })
            .collect()
    });
    answer.policy.pre_push = Some(pre_push);
    let Some(vectors) = vectors else {
        return answer.end("refused", 2, Some("malformed landing.pre_push".into()));
    };
    let root = match git_text(directory, &["rev-parse", "--show-toplevel"]) {
        Ok(r) => r,
        Err(e) => return answer.end("failed", 4, Some(e)),
    };
    for (vector, argv) in vectors {
        let status = Command::new(&argv[0])
            .args(&argv[1..])
            .current_dir(&root)
            .stdin(Stdio::inherit())
            .stdout(Stdio::from(std::io::stderr()))
            .stderr(Stdio::inherit())
            .status();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                return answer.end(
                    "pre-push-failed",
                    2,
                    Some(format!(
                        "{vector}: exit {}",
                        s.code()
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "signal".into())
                    )),
                );
            }
            Err(_) => {
                return answer.end(
                    "pre-push-failed",
                    2,
                    Some(format!("{vector}: could not execute (no exit code)")),
                );
            }
        }
    }
    if !pr["autoMergeRequest"].is_null() {
        if let Err(e) = remote.merge(&repo, args.pr, &["--disable-auto"]) {
            return answer.end("failed", 4, Some(e.reason()));
        }
        answer.disarmed = true;
    }
    let lease = format!(
        "--force-with-lease={branch}:{}",
        args.sha.as_deref().expect("required lease")
    );
    let output = match git(directory, &["push", &lease, "origin", &branch]) {
        Ok(o) => o,
        Err(e) => return answer.end("failed", 4, Some(e)),
    };
    if output.status.success() {
        return answer.end("pushed", 0, None);
    }
    if lease_rejected(&output.stderr) {
        let reference = format!("refs/heads/{branch}");
        answer.remote_head = git_text(directory, &["ls-remote", "origin", &reference])
            .ok()
            .and_then(|s| s.split_whitespace().next().map(str::to_owned));
        let reason = if answer.remote_head.is_some() {
            "push lease was rejected"
        } else {
            "push lease was rejected; remote head could not be read"
        };
        return answer.end("lease-refused", 2, Some(reason.into()));
    }
    answer.end("failed", 4, Some("git push failed".into()))
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
