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
}

/// Every attempt's documentation for `run show`, folded from the chain alone.
pub fn of_run(chain: &Chain, run: &statecraft_run::Run) -> Vec<AttemptDocumentationView> {
    run.attempts
        .iter()
        .map(|a| AttemptDocumentationView {
            attempt: a.number,
            projection: statecraft_run::documentation::project(chain, &run.id, a.number),
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
