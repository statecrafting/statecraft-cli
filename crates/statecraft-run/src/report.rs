//! spec-spine's structured reports, read as typed values.
//!
//! Spec 003 section 3.1. Readiness is **read, never computed here**. This module
//! invokes spec-spine's supported commands and parses their structured output;
//! it does not read `.derived/`, does not reimplement dependency resolution, and
//! does not infer readiness from an exit code when a structured report exists.
//!
//! When a report lacks a field the product needs, that is a **refusal naming the
//! field and the spec-spine version**, never a locally reconstructed substitute.
//! An upstream gap is reported upstream; it is not patched here.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

/// A spec spec-spine offers as ready.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadySpec {
    /// The spec id.
    pub id: String,
    /// Its title.
    pub title: String,
}

/// A spec's lifecycle facts, from `registry list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecLifecycle {
    /// The spec id.
    pub id: String,
    /// `status`: draft, approved, superseded, retired.
    pub status: String,
    /// `implementation`, absent for a spec that declares none.
    #[serde(default)]
    pub implementation: Option<String>,
}

/// The two reports this product joins, and the version that produced them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusReport {
    /// The spec-spine version string, recorded so a refusal can name it.
    pub spec_spine_version: String,
    /// What `registry plan` offered as ready.
    pub ready: Vec<ReadySpec>,
    /// What `registry list` says about every spec's lifecycle.
    pub lifecycle: Vec<SpecLifecycle>,
}

impl CorpusReport {
    /// The lifecycle facts for a spec id, if the report carries them.
    pub fn lifecycle_of(&self, id: &str) -> Option<&SpecLifecycle> {
        self.lifecycle.iter().find(|l| l.id == id)
    }
}

/// Why the product could not read what it needs.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReportError {
    /// A report did not carry a field the product needs.
    ///
    /// The refusal names the field AND the version, because the same product
    /// build will work against a later spec-spine and the operator needs to
    /// know which half to change.
    #[error(
        "spec-spine {version} report `{command}` lacks the field `{field}`; \
         refusing rather than deriving it locally"
    )]
    MissingField {
        /// The command whose report fell short.
        command: String,
        /// The field that was needed.
        field: String,
        /// The spec-spine version that produced the report.
        version: String,
    },
    /// The corpus in the target does not compile.
    ///
    /// Reported as itself, never worked around by reading `.derived/`.
    #[error("the corpus in {path} does not compile: {detail}")]
    CorpusDoesNotCompile {
        /// The target.
        path: String,
        /// The first line of the compiler's complaint.
        detail: String,
    },
    /// spec-spine could not be run at all.
    #[error("could not run spec-spine in {path}: {detail}")]
    NotRunnable {
        /// The target.
        path: String,
        /// What went wrong.
        detail: String,
    },
    /// A report was not the JSON this build understands.
    #[error("spec-spine {version} report `{command}` is not readable: {detail}")]
    Unreadable {
        /// The command.
        command: String,
        /// The version.
        version: String,
        /// What went wrong.
        detail: String,
    },
}

/// Where a corpus report comes from.
///
/// A trait so tests do not need a corpus on disk, and so the one real
/// implementation is the only place a command line is spelled.
pub trait ReportSource {
    /// Read both reports for a target.
    fn corpus_report(&self, target: &Path) -> Result<CorpusReport, ReportError>;
}

/// The real source: `spec-spine` in the target, asked for JSON.
#[derive(Debug, Clone)]
pub struct SpecSpineCli {
    /// The binary to run.
    pub binary: String,
}

impl Default for SpecSpineCli {
    fn default() -> Self {
        Self {
            binary: "spec-spine".into(),
        }
    }
}

impl SpecSpineCli {
    fn run(&self, target: &Path, args: &[&str]) -> Result<std::process::Output, ReportError> {
        Command::new(&self.binary)
            .args(args)
            .current_dir(target)
            .output()
            .map_err(|e| ReportError::NotRunnable {
                path: target.display().to_string(),
                detail: e.to_string(),
            })
    }

    fn version(&self, target: &Path) -> Result<String, ReportError> {
        let out = self.run(target, &["--version"])?;
        // `spec-spine --version` prints `spec-spine 0.20.0`: the version is the
        // last whitespace-separated token. Keeping the whole line made every
        // refusal read `spec-spine spec-spine 0.20.0 report ...` and put a
        // program name inside a field spec 003 section 3.1 requires to name a
        // version.
        Ok(version_token(&String::from_utf8_lossy(&out.stdout)))
    }
}

/// The version out of `spec-spine --version`'s line.
///
/// Separate and testable without a process, because it is what every refusal in
/// [`ReportError`] names and spec 003 section 3.1 requires that to be a version.
pub fn version_token(stdout: &str) -> String {
    stdout
        .split_whitespace()
        .next_back()
        .unwrap_or_default()
        .to_string()
}

impl ReportSource for SpecSpineCli {
    fn corpus_report(&self, target: &Path) -> Result<CorpusReport, ReportError> {
        let version = self.version(target)?;

        // A corpus that does not compile is reported as itself. Falling back to
        // `.derived/` here is the exact move spec 001 forbids, and it would also
        // be reading a tree the compiler has just said is stale.
        let check = self.run(target, &["check"])?;
        if !check.status.success() {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&check.stderr),
                String::from_utf8_lossy(&check.stdout)
            );
            return Err(ReportError::CorpusDoesNotCompile {
                path: target.display().to_string(),
                detail: text
                    .lines()
                    .map(str::trim)
                    .find(|l| !l.is_empty())
                    .unwrap_or("no detail")
                    .to_string(),
            });
        }

        let plan = self.run(target, &["registry", "plan", "--json"])?;
        let plan_json: serde_json::Value =
            serde_json::from_slice(&plan.stdout).map_err(|e| ReportError::Unreadable {
                command: "registry plan --json".into(),
                version: version.clone(),
                detail: e.to_string(),
            })?;
        let ready_raw = plan_json
            .get("ready")
            .ok_or_else(|| ReportError::MissingField {
                command: "registry plan --json".into(),
                field: "ready".into(),
                version: version.clone(),
            })?;
        let ready: Vec<ReadySpec> =
            serde_json::from_value(ready_raw.clone()).map_err(|e| ReportError::Unreadable {
                command: "registry plan --json".into(),
                version: version.clone(),
                detail: e.to_string(),
            })?;

        let list = self.run(target, &["registry", "list", "--json"])?;
        let list_json: serde_json::Value =
            serde_json::from_slice(&list.stdout).map_err(|e| ReportError::Unreadable {
                command: "registry list --json".into(),
                version: version.clone(),
                detail: e.to_string(),
            })?;
        let lifecycle = parse_lifecycle(&list_json, &version)?;

        Ok(CorpusReport {
            spec_spine_version: version,
            ready,
            lifecycle,
        })
    }
}

/// Parse `registry list --json` into lifecycle facts.
///
/// Separate and public to the crate so the missing-field refusal is testable
/// without a corpus: the field this product most depends on, `status`, is the
/// one `registry plan` does not carry, which is precisely why two reports are
/// joined rather than one being stretched.
pub fn parse_lifecycle(
    value: &serde_json::Value,
    version: &str,
) -> Result<Vec<SpecLifecycle>, ReportError> {
    // Both reports are envelopes, and only one of them was read as such.
    // Measured against the pinned spec-spine 0.20.0 on 2026-09-17:
    // `registry list --json` answers `{"items": [...], "schemaVersion": ...}`,
    // the way `registry plan --json` answers `{"ready": [...], ...}` and is
    // already read through its own key above. Reading this one as a bare array
    // made every `work`, `run` and `accept` invocation exit 4 against a real
    // corpus. A bare array is still accepted, because an older report that is
    // one carries the same rows.
    let rows = value
        .get("items")
        .and_then(serde_json::Value::as_array)
        .or_else(|| value.as_array())
        .ok_or_else(|| ReportError::Unreadable {
            command: "registry list --json".into(),
            version: version.to_string(),
            detail: "expected `items` to be an array of specs".into(),
        })?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        for field in ["id", "status"] {
            if row.get(field).is_none() {
                return Err(ReportError::MissingField {
                    command: "registry list --json".into(),
                    field: field.into(),
                    version: version.to_string(),
                });
            }
        }
        out.push(
            serde_json::from_value(row.clone()).map_err(|e| ReportError::Unreadable {
                command: "registry list --json".into(),
                version: version.to_string(),
                detail: e.to_string(),
            })?,
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_row_without_status_refuses_naming_the_field_and_the_version() {
        let v = serde_json::json!([{"id": "001-x", "title": "x"}]);
        match parse_lifecycle(&v, "0.18.0") {
            Err(ReportError::MissingField {
                command,
                field,
                version,
            }) => {
                assert_eq!(field, "status");
                assert_eq!(version, "0.18.0");
                assert!(command.contains("registry list"));
            }
            other => panic!("expected a missing-field refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_complete_list_row_parses() {
        let v = serde_json::json!([
            {"id": "001-x", "status": "approved", "implementation": "pending"}
        ]);
        let rows = parse_lifecycle(&v, "0.18.0").unwrap();
        assert_eq!(rows[0].status, "approved");
        assert_eq!(rows[0].implementation.as_deref(), Some("pending"));
    }

    #[test]
    fn an_absent_implementation_is_none_rather_than_an_error() {
        let v = serde_json::json!([{"id": "001-x", "status": "approved"}]);
        let rows = parse_lifecycle(&v, "0.18.0").unwrap();
        assert_eq!(rows[0].implementation, None);
    }

    #[test]
    fn a_report_that_is_not_an_array_is_unreadable_not_missing_a_field() {
        let v = serde_json::json!({"specs": []});
        assert!(matches!(
            parse_lifecycle(&v, "0.18.0"),
            Err(ReportError::Unreadable { .. })
        ));
    }

    // The shape the pinned spec-spine actually emits, measured on 2026-09-17.
    // The hand-built bare arrays above are what let this go unnoticed: they are
    // the shape this parser wanted rather than the shape it is given.
    #[test]
    fn the_pinned_reports_envelope_is_read_through_its_items_key() {
        let v = serde_json::json!({
            "items": [
                {"id": "000-bootstrap", "status": "approved", "implementation": "n-a"},
                {"id": "001-x", "status": "approved", "implementation": "pending"}
            ],
            "schemaVersion": "0.1.0"
        });
        let rows = parse_lifecycle(&v, "0.20.0").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].id, "001-x");
        assert_eq!(rows[1].status, "approved");
    }

    #[test]
    fn a_row_inside_the_envelope_without_status_still_refuses_naming_the_field() {
        let v = serde_json::json!({"items": [{"id": "001-x", "title": "x"}]});
        match parse_lifecycle(&v, "0.20.0") {
            Err(ReportError::MissingField { field, version, .. }) => {
                assert_eq!(field, "status");
                assert_eq!(version, "0.20.0");
            }
            other => panic!("expected a missing-field refusal, got {other:?}"),
        }
    }

    #[test]
    fn the_version_a_refusal_names_is_the_version_and_not_the_program_name() {
        assert_eq!(version_token("spec-spine 0.20.0\n"), "0.20.0");
        assert_eq!(version_token(""), "");
    }
}
