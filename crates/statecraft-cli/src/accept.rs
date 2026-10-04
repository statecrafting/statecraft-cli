//! `accept`: the sequence, and nothing but the sequence.
//!
//! Added by spec `006`'s additive `extends` edge. Every judgement here comes
//! from `statecraft_acceptance`; what this module contributes is the order, the
//! three observations only the caller can make (what the candidate is, what
//! changed, and what the base's authority-set bytes are), and the invocation
//! spec 005 section 3.3.1 deliberately keeps out of the library.
//!
//! # Why the invocations are here and not there
//!
//! Spec 005 section 3.3.1 rule 3: the acceptance library reads the delta
//! report's bytes and does not run it, because spec 001 section 3.5 requires
//! every authority-set member to be read at the **trusted base** and a candidate
//! that chose its own classifier would classify itself. Rule 2 adds that the
//! binary is resolved independently of the candidate. So `spec-spine delta` is
//! run here, in the **target** rather than in the prepared workspace, with the
//! base and the candidate named explicitly.
//!
//! This is the caller side the decision record assigns to the `accept` binding: the reader
//! landed in #20, and the verb that obtains a report is this one.

use statecraft_acceptance::absence::{Absence, Recorded};
use statecraft_acceptance::authority::{self, Declared, DeltaReport, NoDeltaReport};
use statecraft_acceptance::delta::SpecSpineDeltaReport;
use statecraft_acceptance::independence::Check;
use statecraft_acceptance::independence::SuiteResult;
use statecraft_acceptance::judged::{Base, Candidate, Judged, NoAcceptance, Policy};
use statecraft_acceptance::outcome::Acceptance;
use statecraft_acceptance::receipt::{MintContext, mint};
use std::path::Path;

/// The authority set this repository declares, by path.
///
/// Spec 005 section 3.3 case 2: the repository-artifact members are declared
/// **here, by path**, and the delta report's class for such a path is detail
/// rather than a membership answer. These are the four this repository has:
/// the check suite, the verifier, the hooks, and the acceptance instructions
/// that live outside a `spec.md`.
pub fn declared_authority_set() -> Declared {
    Declared::none()
        .declaring("Makefile", "check-suite")
        .declaring(".github/workflows/", "check-suite")
        .declaring("scripts/", "verifier")
        .declaring(".claude/", "hooks")
        .declaring("spec-spine.toml", "acceptance-instructions")
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = statecraft_run::trusted_git::output(dir, args).ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Identify the candidate in a prepared workspace.
///
/// Three facts, each observed rather than assumed: the sha, whether the work
/// tree was clean, and which paths were dirty when it was not. `head_stable` is
/// the caller's to decide from the run record, so it is a parameter.
#[cfg(test)]
pub fn identify_candidate(workspace: &Path, head_stable: bool) -> Result<Candidate, NoAcceptance> {
    let sha = git(workspace, &["rev-parse", "HEAD"]).ok_or_else(|| {
        NoAcceptance::CandidateUnidentified {
            detail: format!("HEAD does not resolve in {}", workspace.display()),
        }
    })?;
    let status = git(workspace, &["status", "--porcelain"]).ok_or_else(|| {
        NoAcceptance::CandidateUnidentified {
            detail: format!("the work tree in {} cannot be read", workspace.display()),
        }
    })?;
    let dirty_paths: Vec<String> = status
        .lines()
        .filter_map(|l| l.get(3..).map(str::to_string))
        .collect();
    Ok(Candidate {
        sha,
        work_tree_clean: dirty_paths.is_empty(),
        head_stable,
        dirty_paths,
    })
}

/// The policy digest, computed over the authority set's bytes **at the base**.
///
/// Read with `git show <base>:<path>`, so the bytes are the base's and not the
/// candidate's. A member the base does not carry contributes nothing rather
/// than an error: a repository that has not yet written a hooks directory has
/// no hooks to digest. What cannot be computed at all is a **refusal** (spec
/// 006 section 3.10), because a digest nobody can compute identifies no policy.
pub fn policy_digest(
    target: &Path,
    base: &str,
    declared: &Declared,
) -> Result<Policy, NoAcceptance> {
    let mut material = Vec::new();
    let mut any = false;
    for member in &declared.members {
        // `git ls-tree -r --name-only <base> -- <prefix>` names every path the
        // base carries under the prefix, so a prefix and a file are handled the
        // same way and neither needs a special case.
        let listed = git(
            target,
            &[
                "ls-tree",
                "-r",
                "--name-only",
                base,
                "--",
                &member.path_prefix,
            ],
        )
        .ok_or_else(|| NoAcceptance::PolicyDigestUncomputable {
            detail: format!("{base} does not resolve in {}", target.display()),
        })?;
        for path in listed.lines().filter(|l| !l.is_empty()) {
            let Some(bytes) = git(target, &["show", &format!("{base}:{path}")]) else {
                continue;
            };
            any = true;
            material.extend_from_slice(member.member.as_bytes());
            material.push(0);
            material.extend_from_slice(path.as_bytes());
            material.push(0);
            material.extend_from_slice(bytes.as_bytes());
            material.push(0);
        }
    }
    if !any {
        return Err(NoAcceptance::PolicyDigestUncomputable {
            detail: format!(
                "no declared authority-set path exists at {base}, so there is no policy to digest"
            ),
        });
    }
    Ok(Policy {
        digest: statecraft_environment::digest::digest_bytes(&material),
    })
}

/// Every path the candidate changed relative to the base.
pub fn changed_paths(workspace: &Path, base: &str, candidate: &str) -> Vec<String> {
    git(
        workspace,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--name-only",
            &format!("{base}..{candidate}"),
        ],
    )
    .map(|t| {
        t.lines()
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default()
}

/// Obtain spec-spine's change classification for this candidate.
///
/// Run in the **target**, against the base and the candidate by name. A report
/// that cannot be obtained or cannot be read becomes [`NoDeltaReport`], whose
/// reason spec 005 section 3.3.3 constrains: it says what was asked for and what
/// came back, names the installed version, and asserts nothing about what a
/// release carries.
pub fn obtain_delta_report(
    target: &Path,
    base: &str,
    candidate: &str,
    spec_spine_version: &str,
) -> Box<dyn DeltaReport> {
    let Ok(program) = crate::judge::program(target) else {
        return Box::new(NoDeltaReport {
            spec_spine_version: spec_spine_version.to_string(),
        });
    };
    let output = std::process::Command::new(program)
        .args(["delta", "--base", base, "--head", candidate, "--json"])
        .current_dir(target)
        .output();

    let Ok(output) = output else {
        return Box::new(NoDeltaReport {
            spec_spine_version: spec_spine_version.to_string(),
        });
    };
    match SpecSpineDeltaReport::from_envelope_json(&output.stdout) {
        Ok(report) => Box::new(report),
        Err(_) => Box::new(NoDeltaReport {
            spec_spine_version: spec_spine_version.to_string(),
        }),
    }
}

/// Everything `accept` needs that is not read from the record.
#[derive(Debug, Clone)]
pub struct Context {
    /// Which repository.
    pub repository: String,
    /// The spec-spine version, so a reason can name what answered.
    pub spec_spine_version: String,
    /// The adapter's version.
    pub adapter_version: String,
    /// The attempt identity.
    pub attempt: String,
    /// The spec whose declared acceptance is the suite.
    pub spec_id: String,
}

/// Judge one completed attempt.
///
/// Every decision is an owning crate's: [`authority::evaluate`] for the
/// authority-set rule, [`SuiteSource::run_suite`] for the suite, [`mint`] for
/// the receipt. What is here is the order they run in.
pub fn judge(
    target: &Path,
    home: &Path,
    workspace: &Path,
    base_commit: &str,
    head_stable: bool,
    context: &Context,
) -> (Acceptance, Option<SuiteResult>, Recorded<String>) {
    let (acceptance, suite, freshness, _) =
        judge_with_diagnostic(target, home, workspace, base_commit, head_stable, context);
    (acceptance, suite, freshness)
}

/// A failed boundary preparation step, carried beside the not-attempted answer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundaryDiagnostic {
    /// Platform that refused launch.
    pub platform: String,
    /// Preparation step that failed.
    pub step: String,
    /// Concrete failure detail.
    pub detail: String,
}

/// Judge with a structured platform and failed-step diagnostic on refusal.
pub fn judge_with_diagnostic(
    target: &Path,
    home: &Path,
    workspace: &Path,
    base_commit: &str,
    head_stable: bool,
    context: &Context,
) -> (
    Acceptance,
    Option<SuiteResult>,
    Recorded<String>,
    Option<BoundaryDiagnostic>,
) {
    let (run_id, number) = match context.attempt.rsplit_once('/') {
        Some((run, number)) => match number.parse::<u32>() {
            Ok(number) => (run, number),
            Err(error) => return boundary_unavailable("attempt-identity", error.to_string()),
        },
        None => return boundary_unavailable("attempt-identity", "preparation refused"),
    };
    let places = statecraft_home::launch::Places::of(home, target);
    let identity = statecraft_home::launch::AttemptIdentity {
        run_id: run_id.into(),
        attempt: number,
    };
    let exchange = identity.exchange_dir(&places);
    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => return boundary_unavailable("current-executable", error.to_string()),
    };
    let path = std::env::var("PATH").unwrap_or_default();
    let admitted = match statecraft_home::confinement::admit(
        statecraft_home::confinement::Roots {
            home,
            target,
            workspace,
            exchange: &exchange,
            records: &places.records,
            gate: None,
            git_writes: vec![],
            provider: false,
        },
        &executable,
        &path,
    ) {
        Ok(admitted) => admitted,
        Err(error) => return boundary_unavailable("admission", error.to_string()),
    };
    let mut variables = std::collections::BTreeMap::from([("PATH".into(), path.clone())]);
    variables.extend(admitted.variables.clone());
    let cache = Path::new(&variables["TMPDIR"]).join("suite-cache");
    if std::fs::create_dir(&cache).is_err() {
        return boundary_unavailable("cache-root", "preparation refused");
    }
    for name in [
        "HOME",
        "CARGO_HOME",
        "XDG_CACHE_HOME",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "npm_config_cache",
        "PIP_CACHE_DIR",
        "GOCACHE",
        "GOMODCACHE",
        "NUGET_PACKAGES",
    ] {
        let directory = cache.join(name);
        if std::fs::create_dir(&directory).is_err() {
            return boundary_unavailable("cache-directory", "preparation refused");
        }
        variables.insert(name.into(), directory.display().to_string());
    }
    if let Some(operator_home) = std::env::var_os("HOME") {
        variables.insert(
            "RUSTUP_HOME".into(),
            Path::new(&operator_home)
                .join(".rustup")
                .display()
                .to_string(),
        );
    }
    variables.insert(
        "GIT_OBJECT_DIRECTORY".into(),
        exchange.join("objects").display().to_string(),
    );
    let common = match statecraft_run::trusted_git::common(target) {
        Ok(path) => path,
        Err(error) => return boundary_unavailable("trusted-git", error.to_string()),
    };
    variables.insert(
        "GIT_ALTERNATE_OBJECT_DIRECTORIES".into(),
        common.join("objects").display().to_string(),
    );
    let imported = match statecraft_home::confinement::import_objects(
        &admitted, target, workspace, run_id, &variables,
    ) {
        Ok(imported) => imported,
        Err(error) => return boundary_unavailable("object-import", error.to_string()),
    };
    let Some(verified_head) = imported["head"].as_str() else {
        return boundary_unavailable("imported-head", "preparation refused");
    };
    variables.remove("GIT_OBJECT_DIRECTORY");
    variables.remove("GIT_ALTERNATE_OBJECT_DIRECTORIES");
    let held_workspace = match statecraft_adapter::boundary::DataRoot::open(workspace) {
        Ok(root) => root,
        Err(error) => return boundary_unavailable("workspace-handle", error.to_string()),
    };
    let base_export = match crate::coverage::export(target, base_commit) {
        Ok(export) => export,
        Err(error) => return boundary_unavailable("base-export", error.to_string()),
    };
    let candidate = match identify_private_candidate(
        target,
        &held_workspace,
        &base_export.tree(),
        run_id,
        verified_head,
        head_stable,
    ) {
        Ok(candidate) => candidate,
        Err(reason) => return (Acceptance::None { reason }, None, absent(), None),
    };
    let declared = declared_authority_set();
    let policy = match policy_digest(target, base_commit, &declared) {
        Ok(p) => p,
        Err(reason) => return (Acceptance::None { reason }, None, absent(), None),
    };

    let program = match crate::judge::program(target) {
        Ok(program) => program,
        Err(error) => return boundary_unavailable("judge-program", error.to_string()),
    };
    if let Err(error) = admitted.boundary.resolve(Path::new(&program), &path) {
        return boundary_unavailable("judge-resolution", error.to_string());
    }
    let plan = match statecraft_adapter::coverage::read_plan(
        &program,
        &base_export.tree(),
        &context.spec_id,
    ) {
        Ok(plan) => plan,
        Err(error) => return boundary_unavailable("suite-plan", error.to_string()),
    };
    let mut checks = Vec::new();
    for command in &plan.commands {
        if let Err(error) = admitted.retest(&executable) {
            return boundary_unavailable("self-test", error.to_string());
        }
        let shell = match admitted.boundary.resolve(Path::new("/bin/sh"), &path) {
            Ok(shell) => shell,
            Err(error) => return boundary_unavailable("shell-resolution", error.to_string()),
        };
        match statecraft_adapter::supervisor::capture_confined(
            &shell,
            &["-c", command],
            workspace,
            &variables,
            b"",
            std::time::Duration::from_secs(600),
            &admitted.boundary,
        ) {
            Ok(output) if !output.timed_out && output.surviving_processes.is_none() => {
                let text = String::from_utf8_lossy(&output.stdout);
                let report: Option<serde_json::Value> =
                    serde_json::from_slice(&output.stdout).ok().or_else(|| {
                        let start = text.find("{\n  \"exitCode\"")?;
                        serde_json::from_str(&text[start..]).ok()
                    });
                let structured = report.as_ref().and_then(|report| {
                    report
                        .get("ok")
                        .and_then(serde_json::Value::as_bool)
                        .or_else(|| {
                            report
                                .get("outcome")
                                .or_else(|| report.pointer("/report/outcome"))
                                .and_then(serde_json::Value::as_str)
                                .map(|word| matches!(word, "ok" | "passed"))
                        })
                });
                if command.contains("spec-spine")
                    && command.contains("--json")
                    && structured.is_none()
                {
                    checks.push(Check::missing_report(command, output.code.unwrap_or(-1)));
                } else {
                    checks.push(Check::ran(command, output.code.unwrap_or(-1), structured));
                }
            }
            Ok(_) => checks.push(Check::did_not_run(
                command,
                "suite deadline or surviving process",
            )),
            Err(error) => return boundary_unavailable("suite-launch", error.to_string()),
        }
    }
    let mut suite = SuiteResult::new(checks, None);
    suite.confinement = Some(admitted.boundary.record().clone());
    // Generated suite files are not part of the candidate. Re-read only the
    // private reference from the trusted common directory after execution.
    let branch = format!("refs/heads/statecraft/{run_id}/work");
    let after_head = statecraft_run::trusted_git::private_reference(target, &branch).ok();
    let mut after = candidate.clone();
    after.head_stable &= after_head.as_deref() == Some(candidate.sha.as_str());
    let judged = Judged {
        candidate: after,
        base: Base {
            sha: base_commit.into(),
        },
        policy,
    };
    if !suite.ran() {
        return (
            Acceptance::None {
                reason: NoAcceptance::SuiteDidNotRun {
                    unrun_checks: suite.unrun_count(),
                },
            },
            Some(suite),
            absent(),
            None,
        );
    }

    let paths = changed_paths(target, base_commit, &candidate.sha);
    let report = obtain_delta_report(
        target,
        base_commit,
        &candidate.sha,
        &context.spec_spine_version,
    );
    let verdict = authority::evaluate(&paths, &declared, report.as_ref());

    let authority_change = if verdict.may_accept_on_own_suite {
        None
    } else {
        Some(verdict.note.clone())
    };

    let mint_context = MintContext {
        repository: context.repository.clone(),
        product_version: env!("CARGO_PKG_VERSION").to_string(),
        spec_spine_version: context.spec_spine_version.clone(),
        adapter_version: context.adapter_version.clone(),
        attempt: context.attempt.clone(),
        authority_paths_touched: verdict.repository_members_touched.clone(),
        // Spec 029 section 3.3: the receipt names the judge the suite ran.
        judge: crate::judge::selection(target).record(),
    };

    match mint(&judged, &suite, &mint_context, authority_change) {
        Ok(receipt) => {
            let freshness = Recorded::Present("current".to_string());
            (
                Acceptance::Accepted {
                    receipt: Box::new(receipt),
                },
                Some(suite),
                freshness,
                None,
            )
        }
        Err(reason) => (Acceptance::Failed { reason }, Some(suite), absent(), None),
    }
}

fn boundary_unavailable(
    step: &str,
    detail: impl ToString,
) -> (
    Acceptance,
    Option<SuiteResult>,
    Recorded<String>,
    Option<BoundaryDiagnostic>,
) {
    (
        Acceptance::NotAttempted {
            reason: statecraft_acceptance::judged::NotAttemptedReason::BoundaryUnavailable,
            refusal_count: None,
        },
        None,
        absent(),
        Some(BoundaryDiagnostic {
            platform: std::env::consts::OS.into(),
            step: step.into(),
            detail: detail.to_string(),
        }),
    )
}

fn identify_private_candidate(
    target: &Path,
    workspace: &statecraft_adapter::boundary::DataRoot,
    base_tree: &Path,
    run: &str,
    verified_head: &str,
    head_stable: bool,
) -> Result<Candidate, NoAcceptance> {
    let identify = |detail: String| NoAcceptance::CandidateUnidentified { detail };
    let branch = format!("refs/heads/statecraft/{run}/work");
    let reference = statecraft_run::trusted_git::private_reference(target, &branch)
        .ok()
        .ok_or_else(|| identify("private branch has no object identity".into()))?;
    if reference != verified_head {
        return Err(identify(
            "private branch changed after verified import".into(),
        ));
    }
    let sha = verified_head.to_string();
    let export = crate::coverage::export(target, &sha).map_err(identify)?;
    let expected = statecraft_adapter::boundary::DataRoot::open(&export.tree())
        .and_then(|root| root.snapshot())
        .map_err(|e| identify(e.to_string()))?;
    let actual = workspace.snapshot().map_err(|e| identify(e.to_string()))?;
    let mut dirty = Vec::new();
    for (path, entry) in &expected {
        if actual.get(path) != Some(entry) {
            dirty.push(path.display().to_string());
        }
    }
    for path in actual.keys().filter(|path| !expected.contains_key(*path)) {
        // Ignore rules are interpreted only from the trusted base export.
        let mut command =
            statecraft_run::trusted_git::command(target).map_err(|e| identify(e.to_string()))?;
        let ignored = command
            .arg("--work-tree")
            .arg(base_tree)
            .args([
                "-c",
                "core.excludesFile=/dev/null",
                "check-ignore",
                "--no-index",
                "--quiet",
                "--",
            ])
            .arg(path)
            .current_dir(base_tree)
            .status()
            .map_err(|e| identify(e.to_string()))?;
        if ignored.code() != Some(0) {
            if ignored.code() != Some(1) {
                return Err(identify("trusted ignore comparison refused".into()));
            }
            dirty.push(path.display().to_string());
        }
    }
    dirty.sort();
    Ok(Candidate {
        sha,
        work_tree_clean: dirty.is_empty(),
        head_stable,
        dirty_paths: dirty,
    })
}

fn absent() -> Recorded<String> {
    Recorded::Absent(Absence::NotRecorded)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_run(dir: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git runs");
        assert!(out.status.success(), "git {args:?}: {out:?}");
    }

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        git_run(dir.path(), &["init", "--quiet"]);
        git_run(dir.path(), &["config", "user.email", "t@example.com"]);
        git_run(dir.path(), &["config", "user.name", "t"]);
        std::fs::write(dir.path().join("Makefile"), b"gate:\n\ttrue\n").unwrap();
        git_run(dir.path(), &["add", "."]);
        git_run(dir.path(), &["commit", "--quiet", "-m", "one"]);
        dir
    }

    #[test]
    fn the_declared_authority_set_names_the_four_member_kinds() {
        let d = declared_authority_set();
        let members: Vec<&str> = d.members.iter().map(|m| m.member.as_str()).collect();
        for kind in [
            "check-suite",
            "verifier",
            "hooks",
            "acceptance-instructions",
        ] {
            assert!(members.contains(&kind), "{kind} is not declared");
        }
    }

    #[test]
    fn a_policy_digest_is_computed_over_the_bases_bytes_and_is_stable() {
        let target = repo();
        let base = git(target.path(), &["rev-parse", "HEAD"]).unwrap();
        let first = policy_digest(target.path(), &base, &declared_authority_set()).unwrap();

        // Change the candidate's copy. The digest is the BASE's, so it moves not
        // at all: a candidate that could change the policy it is judged under
        // would be judging itself.
        std::fs::write(target.path().join("Makefile"), b"gate:\n\tfalse\n").unwrap();
        let second = policy_digest(target.path(), &base, &declared_authority_set()).unwrap();
        assert_eq!(first.digest, second.digest);
    }

    #[test]
    fn a_base_that_carries_no_declared_path_cannot_be_digested_and_refuses() {
        let dir = tempfile::tempdir().unwrap();
        git_run(dir.path(), &["init", "--quiet"]);
        git_run(dir.path(), &["config", "user.email", "t@example.com"]);
        git_run(dir.path(), &["config", "user.name", "t"]);
        std::fs::write(dir.path().join("unrelated.txt"), b"x").unwrap();
        git_run(dir.path(), &["add", "."]);
        git_run(dir.path(), &["commit", "--quiet", "-m", "one"]);
        let base = git(dir.path(), &["rev-parse", "HEAD"]).unwrap();

        match policy_digest(dir.path(), &base, &declared_authority_set()) {
            Err(NoAcceptance::PolicyDigestUncomputable { detail }) => {
                assert!(detail.contains("no policy to digest"));
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_dirty_work_tree_is_identified_and_its_paths_are_named() {
        let target = repo();
        std::fs::write(target.path().join("dirty.txt"), b"x").unwrap();
        let candidate = identify_candidate(target.path(), true).unwrap();
        assert!(!candidate.work_tree_clean);
        assert!(
            candidate
                .dirty_paths
                .iter()
                .any(|p| p.contains("dirty.txt"))
        );
    }

    #[test]
    fn an_unobtainable_delta_report_names_this_products_gap_and_not_a_release() {
        let target = repo();
        let report = obtain_delta_report(target.path(), "nope", "also-nope", "0.20.0");
        let answer = report.corpus_answer(&[]);
        assert_eq!(report.version(), "0.20.0");
        match answer {
            statecraft_acceptance::authority::CorpusAnswer::Unavailable { reason } => {
                assert!(reason.contains("this product"));
                // Never a claim about what a release carries (005 section 3.3.3).
                assert!(!reason.contains("release"));
            }
            other => panic!("expected unavailable, got {other:?}"),
        }
    }

    #[test]
    fn a_candidate_whose_head_moved_gets_no_receipt() {
        let target = repo();
        let base = git(target.path(), &["rev-parse", "HEAD"]).unwrap();
        let context = Context {
            repository: target.path().display().to_string(),
            spec_spine_version: "0.20.0".into(),
            adapter_version: "0.0.0".into(),
            attempt: "r1/1".into(),
            spec_id: "000".into(),
        };
        // `head_stable: false` is the observation spec 003 section 3.8 makes.
        let (acceptance, _, freshness) = judge(
            target.path(),
            target.path(),
            target.path(),
            &base,
            false,
            &context,
        );
        assert!(acceptance.receipt().is_none());
        assert_eq!(freshness, absent());
    }
}
