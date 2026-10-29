//! Spec 020's command projections: the documentation task, the adapter's
//! delivery testimony and the result under `run show`, and the inputs a
//! later acceptance contract reads under `accept` (section 3.9).
//!
//! Neither command validates, reviews, applies a patch, writes an expected
//! artifact path, selects a provider, retries or publishes. `run show` reads
//! `statecraft-run::documentation`'s fold of the record; `accept` reads
//! `statecraft-acceptance::documentation`'s inputs, whose acceptance is
//! `not-declared`.

use serde::Serialize;

use statecraft_run::documentation::{Projection, ResultProjection, Validation};
use statecraft_run::record::Chain;

/// One attempt's documentation, as `run show` renders it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptDocumentationView {
    /// The attempt.
    pub attempt: u32,
    /// The record's projection: `not-recorded` for a historical attempt.
    #[serde(flatten)]
    pub projection: Projection,
    /// Spec 021 section 3.10: the review and correction identities, listed
    /// without judging them.
    pub records: Vec<statecraft_run::documentation::review::Listed>,
    /// Spec 022 section 3.11: the fixed-export workflow's latest version,
    /// `not-recorded` for an attempt outside the profile.
    pub fixed_export: crate::fixed_export::WorkflowView,
}

/// Every attempt's documentation for `run show`, folded from the chain alone.
pub fn of_run(chain: &Chain, run: &statecraft_run::Run) -> Vec<AttemptDocumentationView> {
    run.attempts
        .iter()
        .map(|a| AttemptDocumentationView {
            attempt: a.number,
            projection: statecraft_run::documentation::project(chain, &run.id, a.number),
            records: statecraft_run::documentation::review::identities(chain, &run.id, a.number),
            fixed_export: crate::fixed_export::view(chain, &run.id, a.number),
        })
        .collect()
}

/// The human lines for each attempt's documentation.
pub fn describe(views: &[AttemptDocumentationView]) -> String {
    let mut out = String::new();
    for v in views {
        match &v.projection {
            Projection::NotRecorded => {}
            Projection::Unreadable { reason } => {
                out.push_str(&format!(
                    "attempt {} documentation task unreadable: {reason}\n",
                    v.attempt
                ));
            }
            Projection::Recorded {
                task,
                delivery,
                result,
            } => {
                out.push_str(&format!(
                    "attempt {} documentation task {} {}\n",
                    v.attempt, task.task_id, task.task_digest
                ));
                match delivery {
                    None => out.push_str("  delivery not-recorded\n"),
                    Some(d) if d.eligible => out.push_str("  delivery reported\n"),
                    Some(d) => {
                        for f in &d.findings {
                            out.push_str(&format!("  delivery finding {f}\n"));
                        }
                    }
                }
                out.push_str(&describe_result(result));
            }
        }
        for r in &v.records {
            use statecraft_run::documentation::review::Listed;
            out.push_str(&match r {
                Listed::Review {
                    digest,
                    reviewer,
                    recommendation,
                } => format!(
                    "  review {digest} by {reviewer}, recommends {}\n",
                    serde_json::to_value(recommendation)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_string))
                        .unwrap_or_default()
                ),
                Listed::Correction { digest, supersedes } => {
                    format!("  correction {digest} supersedes {supersedes}\n")
                }
                Listed::Unreadable { reason } => format!("  record unreadable: {reason}\n"),
            });
        }
        out.push_str(&crate::fixed_export::describe(v.attempt, &v.fixed_export));
    }
    out
}

fn describe_result(result: &ResultProjection) -> String {
    match result {
        ResultProjection::NotRecorded => "  result not-recorded\n".to_string(),
        ResultProjection::Unreadable { reason } => format!("  result unreadable: {reason}\n"),
        ResultProjection::Duplicate { custody } => format!(
            "  result duplicate: {} terminal results, none chosen\n",
            custody.len()
        ),
        ResultProjection::Recorded(r) => {
            let head = format!("  result {} {}", r.raw.custody_digest, r.raw.media_type);
            match &r.validation {
                Validation::SchemaInvalid { reason } => {
                    format!("{head} schema-invalid: {reason}\n")
                }
                Validation::Unbound { field } => format!("{head} unbound: {field}\n"),
                Validation::Bound { summary } => format!(
                    "{head} provider {} (the provider's claim), {} artifact(s), {} claim(s), \
                     {} unknown, {} contradiction(s), {} unsupported, {} omission(s), \
                     {} citation failure(s), {} unexplained\n",
                    serde_json::to_value(summary.provider_completion)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_string))
                        .unwrap_or_default(),
                    summary.artifacts.len(),
                    summary.claims,
                    summary.unknowns,
                    summary.contradictions,
                    summary.unsupported_claims,
                    summary.provider_omissions,
                    summary.statecraft_citation_failures,
                    summary.unexplained_claims,
                ),
            }
        }
    }
}

/// What `accept` shows under `documentationAcceptance` (spec 021).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum AcceptanceView {
    /// The attempt carries no documentation task.
    NotRecorded,
    /// The judgment could not be recorded.
    Unrecorded {
        /// Why.
        reason: String,
    },
    /// The judgment, recorded.
    Judged {
        /// Every dimension and the outcome.
        #[serde(flatten)]
        judgment: Box<statecraft_acceptance::documentation_acceptance::Judgment>,
    },
}

/// The subject of a recorded documentation acceptance.
pub const ACCEPTANCE_SUBJECT: &str = "documentation-acceptance";

pub(crate) fn git_out(dir: &std::path::Path, args: &[&str]) -> Option<Vec<u8>> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

/// `id` when it is a full SHA-1 or SHA-256 object id in lower-case hex.
pub(crate) fn object_id(id: &str) -> Option<&str> {
    (matches!(id.len(), 40 | 64) && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
        .then_some(id)
}

/// The candidate's paths changed against `base`, tracked and untracked;
/// `None` when the workspace cannot answer.
fn changed_paths(workspace: &std::path::Path, base: &str) -> Option<Vec<String>> {
    let tracked = git_out(workspace, &["diff", "--name-only", base, "--"])?;
    let untracked = git_out(workspace, &["ls-files", "--others", "--exclude-standard"])?;
    let mut out: Vec<String> = String::from_utf8_lossy(&tracked)
        .lines()
        .chain(String::from_utf8_lossy(&untracked).lines())
        .map(str::to_string)
        .collect();
    out.sort();
    out.dedup();
    Some(out)
}

/// Spec 021 section 3.10: judge the attempt's documentation when it has a
/// task, and append the judgment to the record. Reads the trusted-base
/// policy at the attempt's base commit, the claims' citations through spec
/// 019's checks, and the retained bytes; applies and publishes nothing.
pub fn accept(
    root: &std::path::Path,
    home: &std::path::Path,
    chain: &Chain,
    attempt: &statecraft_run::Attempt,
    run_id: &str,
    supplied: &crate::context::AcceptFile,
) -> AcceptanceView {
    use statecraft_acceptance::documentation_acceptance::{self as da, ManifestFreshness};
    use statecraft_run::documentation::{self as doc, review};

    let task = match doc::recorded_task(chain, run_id, attempt.number) {
        Ok(Some(t)) => t,
        Ok(None) => return AcceptanceView::NotRecorded,
        Err(reason) => {
            return AcceptanceView::Unrecorded {
                reason: format!("task-unreadable: {reason}"),
            };
        }
    };
    let delivery = doc::delivery::recorded_delivery(chain, &task);
    let (custody, decoded): (Option<String>, Result<doc::DocumentationResult, String>) =
        match doc::project(chain, run_id, attempt.number) {
            Projection::Recorded {
                result: ResultProjection::Recorded(r),
                ..
            } => {
                let custody = r.raw.custody_digest.clone();
                let decoded = match &r.validation {
                    Validation::Bound { .. } => {
                        match statecraft_run::context::read_retained(home, &custody) {
                            Ok(Some(bytes)) => doc::DocumentationResult::decode(&bytes, &task)
                                .map_err(|e| format!("result-schema-invalid: {e}")),
                            _ => Err("result-bytes-not-retained".to_string()),
                        }
                    }
                    Validation::SchemaInvalid { reason } => {
                        Err(format!("result-schema-invalid: {reason}"))
                    }
                    Validation::Unbound { field } => Err(format!("result-unbound: {field}")),
                };
                (Some(custody), decoded)
            }
            Projection::Recorded {
                result: ResultProjection::Duplicate { .. },
                ..
            } => (None, Err("result-duplicate".into())),
            _ => (None, Err("result-not-recorded".into())),
        };
    let citations: Vec<statecraft_acceptance::context::Citation> = decoded
        .as_ref()
        .map(|r| {
            r.claims
                .iter()
                .flat_map(|c| c.citations.iter())
                .map(|c| statecraft_acceptance::context::Citation {
                    repository: c.repository.clone(),
                    page_digest: c.page_digest.clone(),
                    member: c.member.clone(),
                    content_digest: c.content_digest.clone(),
                    span: c.span,
                })
                .collect()
        })
        .unwrap_or_default();
    let context_file = crate::context::AcceptFile {
        checkouts: supplied.checkouts.clone(),
        citations,
        evidence: supplied.evidence.clone(),
    };
    let context =
        match crate::context::judge(root, home, chain, run_id, attempt.number, &context_file) {
            crate::context::Judgment::Judged { result } => Some(*result),
            _ => None,
        };
    // The base reaches git only as a full hexadecimal object id, so no
    // recorded value can be read as an option or a revision expression.
    let base = object_id(&attempt.base_commit);
    let policy_bytes =
        base.and_then(|base| git_out(root, &["show", &format!("{base}:{}", da::POLICY_PATH)]));
    let policy = match policy_bytes {
        Some(b) => da::Policy::decode(&b),
        None if base.is_none() => Err(format!(
            "the trusted base {:?} is not an object id",
            attempt.base_commit
        )),
        None => Err(format!(
            "{} is absent at the trusted base {}",
            da::POLICY_PATH,
            attempt.base_commit
        )),
    };
    let workspace = statecraft_run::workspace::workspace_path(root, run_id);
    // An unread candidate is carried as its reason, never as an empty
    // identity, so the judgment leaves the subject unidentified.
    let candidate = git_out(&workspace, &["rev-parse", "HEAD"])
        .map(|b| String::from_utf8_lossy(&b).trim().to_string())
        .filter(|c| !c.is_empty())
        .ok_or_else(|| format!("no candidate revision in {}", workspace.display()));
    let changed = base.and_then(|base| changed_paths(&workspace, base));
    let reviews = review::recorded_reviews(chain, run_id, attempt.number);
    let corrections = review::recorded_corrections(chain, run_id, attempt.number);
    let retained = |digest: &str| {
        statecraft_run::context::read_retained(home, digest)
            .ok()
            .flatten()
    };
    let now = statecraft_environment::time::rfc3339_utc(
        statecraft_environment::time::Clock::now_unix(&statecraft_environment::time::SystemClock),
    );
    let judgment = da::judge(&da::Inputs {
        repository: &root.display().to_string(),
        trusted_base: &attempt.base_commit,
        candidate: candidate.as_deref().map_err(Clone::clone),
        task: &task,
        delivery: delivery.as_ref(),
        result_custody: custody.as_deref(),
        result: decoded.as_ref().map_err(Clone::clone),
        context: context.as_ref(),
        // No documentation manifest is supplied until spec 022's workflow
        // writes one; its absence is never read as fresh.
        manifest_freshness: ManifestFreshness::NotRecorded,
        retained: &retained,
        reviews: &reviews,
        corrections: &corrections,
        policy: policy
            .as_ref()
            .map(|(p, d)| (p, d.as_str()))
            .map_err(Clone::clone),
        changed_paths: changed.as_deref(),
        now: &now,
    });
    let recorded = Chain::open(home, root)
        .map_err(|e| e.to_string())
        .and_then(|(mut chain, _)| {
            let n = chain.entries().len();
            chain
                .append(
                    &format!("{run_id}/{}/{ACCEPTANCE_SUBJECT}/{n}", attempt.number),
                    &now,
                    &statecraft_run::record::Entry {
                        kind: statecraft_run::record::Kind::Evidence,
                        run_id: run_id.to_string(),
                        attempt: attempt.number,
                        subject: ACCEPTANCE_SUBJECT.to_string(),
                        idempotency_key: None,
                        detail: serde_json::json!({ "judgment": judgment }),
                        effect_id: statecraft_run::record::Identity::Absent,
                    },
                )
                .map_err(|e| e.to_string())
        });
    match recorded {
        Ok(()) => AcceptanceView::Judged {
            judgment: Box::new(judgment),
        },
        Err(reason) => AcceptanceView::Unrecorded { reason },
    }
}

#[cfg(test)]
mod tests {
    use super::object_id;

    #[test]
    fn only_a_full_hex_object_id_reaches_git() {
        let sha1 = "a".repeat(40);
        let sha256 = "0123456789abcdef".repeat(4);
        assert_eq!(object_id(&sha1), Some(sha1.as_str()));
        assert_eq!(object_id(&sha256), Some(sha256.as_str()));
        for bad in [
            "--output=/tmp/x",
            "HEAD",
            "",
            &"A".repeat(40),
            &"a".repeat(39),
        ] {
            assert_eq!(object_id(bad), None, "{bad}");
        }
    }
}
