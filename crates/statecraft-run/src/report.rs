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
use statecraft_environment::probe::{names_refusal, names_stale_only};
use std::path::Path;
use std::process::Command;

/// A spec spec-spine offers as ready.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadySpec {
    /// The spec id.
    pub id: String,
    /// Its title.
    pub title: String,
    /// Its status, where the producer's plan carries one (spec-spine 102;
    /// section 3.1.2). Compared with `registry list`, never preferred to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// Where a row's `status` came from (section 3.1.2 rule 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StatusSource {
    /// `registry list` alone: the plan carries no `status`.
    ListOnly,
    /// `registry list`, and `registry plan` agreed with it.
    ListAgreeingWithPlan,
}

impl StatusSource {
    /// The report fields, as `work list` prints them.
    pub fn describe(self) -> &'static str {
        match self {
            StatusSource::ListOnly => "registry list --json: items[].status",
            StatusSource::ListAgreeingWithPlan => {
                "registry list --json: items[].status, agreeing with registry plan --json: \
                 ready[].status"
            }
        }
    }
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
    /// The obligations the spec declares, where the producer reports them
    /// (spec-spine 106; section 3.1.3). Absent before that producer, and for
    /// a spec that declares none.
    #[serde(default)]
    pub obligations: Vec<DeclaredObligation>,
}

/// One obligation a spec declares, as `registry list` reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclaredObligation {
    /// Its id within the spec, for example `R-1`.
    pub id: String,
    /// Whether it is withdrawn. Still declared, and still bound.
    #[serde(default)]
    pub withdrawn: bool,
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
    /// Where each ready row's `status` came from.
    pub status_source: StatusSource,
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
    /// The ledger in the target is stale: `check` exited 2.
    ///
    /// Section 3.1.2 rule 3 reads only after `check` has said the ledger is
    /// fresh. A stale ledger is a precondition that was not met, not a corpus
    /// that does not compile, and it is cured by recompiling rather than by
    /// editing a spec, so it is named as itself.
    #[error("spec-spine {version} reports the ledger in {path} is stale: {detail}")]
    LedgerStale {
        /// The target.
        path: String,
        /// The producer version.
        version: String,
        /// The first line of the producer's complaint.
        detail: String,
    },
    /// spec-spine ran and refused the target before judging its corpus.
    ///
    /// Measured on 2026-09-23: a spec-spine that does not satisfy the target's
    /// `required_version` refuses with exit 3; from spec-spine 0.26.0 with exit
    /// 2 (measured 2026-09-25). That is not the corpus failing to compile, so
    /// it is not reported as one.
    #[error("spec-spine {version} refused to judge the corpus in {path} ({status}): {detail}")]
    ProducerRefused {
        /// The target.
        path: String,
        /// The producer version.
        version: String,
        /// How `check` ended: `exit N`, or `signal` where it had no code.
        status: String,
        /// The first line of the producer's complaint.
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
    /// The two reports, read from one state, answer `status` differently
    /// (section 3.1.2 rule 2). Neither is chosen.
    #[error(
        "spec-spine {version} contradicts itself about {id}: `registry plan --json` says \
         status `{plan}` and `registry list --json` says `{list}`, read from one state; \
         refusing to schedule from either"
    )]
    Disagreement {
        /// The spec.
        id: String,
        /// What the plan said.
        plan: String,
        /// What the list said.
        list: String,
        /// The producer version.
        version: String,
    },
    /// The ledger moved between the bracketing reads (section 3.1.2 rule 3).
    #[error(
        "the ledger moved while spec-spine {version} was being read: the two `registry list \
         --json` answers around `registry plan --json` differ, so nothing was compared"
    )]
    Moved {
        /// The producer version.
        version: String,
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
        //
        // `check`'s own exit status says which of three things it found, and
        // each is reported as itself: a corpus that does not compile, a stale
        // ledger, and a refusal to judge at all. Below spec-spine 0.26.0 they
        // are 1, 2 and anything else (a pin not met exits 3); from 0.26.0 a
        // stale ledger is 1 and a refusal 2, and the producer's words say
        // which (spec 003 section 5, 2026-09-25).
        let check = self.run(target, &["check"])?;
        if !check.status.success() {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&check.stderr),
                String::from_utf8_lossy(&check.stdout)
            );
            let path = target.display().to_string();
            let detail = text
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("no detail")
                .to_string();
            return Err(check_refusal(
                check.status.code(),
                path,
                version,
                &text,
                detail,
            ));
        }

        // Section 3.1.2 rule 3: `list`, `plan`, `list`, so a disagreement is
        // only ever reported from one state.
        let first = self.run(target, &["registry", "list", "--json"])?;
        let plan = self.run(target, &["registry", "plan", "--json"])?;
        let second = self.run(target, &["registry", "list", "--json"])?;
        join(&version, &plan.stdout, &first.stdout, &second.stdout)
    }
}

/// What a `check` that did not pass reports, read under either of
/// spec-spine's exit tables (spec 003 section 5, 2026-09-25).
fn check_refusal(
    code: Option<i32>,
    path: String,
    version: String,
    text: &str,
    detail: String,
) -> ReportError {
    let stale = match code {
        Some(1) => names_stale_only(text),
        Some(2) => !names_refusal(text),
        _ => false,
    };
    match code {
        _ if stale => ReportError::LedgerStale {
            path,
            version,
            detail,
        },
        Some(1) => ReportError::CorpusDoesNotCompile { path, detail },
        code => ReportError::ProducerRefused {
            path,
            version,
            status: code.map_or_else(|| "signal".to_string(), |c| format!("exit {c}")),
            detail,
        },
    }
}

/// Join one bracketed read into a report (spec 003 sections 3.1.1 and 3.1.2).
///
/// `list_first` and `list_second` are the two `registry list --json` answers
/// read around `plan`. Pure, so every rule is testable against recorded
/// producer output without a producer.
pub fn join(
    version: &str,
    plan: &[u8],
    list_first: &[u8],
    list_second: &[u8],
) -> Result<CorpusReport, ReportError> {
    if list_first != list_second {
        return Err(ReportError::Moved {
            version: version.to_string(),
        });
    }
    let unreadable = |command: &str, detail: String| ReportError::Unreadable {
        command: command.into(),
        version: version.to_string(),
        detail,
    };
    let plan_json: serde_json::Value = serde_json::from_slice(plan)
        .map_err(|e| unreadable("registry plan --json", e.to_string()))?;
    let ready_raw = plan_json
        .get("ready")
        .ok_or_else(|| ReportError::MissingField {
            command: "registry plan --json".into(),
            field: "ready".into(),
            version: version.to_string(),
        })?;
    let ready: Vec<ReadySpec> = serde_json::from_value(ready_raw.clone())
        .map_err(|e| unreadable("registry plan --json", e.to_string()))?;
    let list_json: serde_json::Value = serde_json::from_slice(list_first)
        .map_err(|e| unreadable("registry list --json", e.to_string()))?;
    let lifecycle = parse_lifecycle(&list_json, version)?;

    // Rule 4: all rows carry `status`, or none do.
    let carrying = ready.iter().filter(|r| r.status.is_some()).count();
    let status_source = if carrying == 0 {
        StatusSource::ListOnly
    } else if carrying == ready.len() {
        StatusSource::ListAgreeingWithPlan
    } else {
        return Err(unreadable(
            "registry plan --json",
            format!(
                "{carrying} of {} ready row(s) carry `status`; a report that is not one shape \
                 is not one this build reads",
                ready.len()
            ),
        ));
    };
    // Rule 2: compared, never preferred. A ready row the list does not carry
    // is left to section 3.1.1, which excludes it as unknown.
    for row in &ready {
        if let (Some(plan), Some(list)) = (
            row.status.as_deref(),
            lifecycle.iter().find(|l| l.id == row.id),
        ) && plan != list.status
        {
            return Err(ReportError::Disagreement {
                id: row.id.clone(),
                plan: plan.to_string(),
                list: list.status.clone(),
                version: version.to_string(),
            });
        }
    }
    Ok(CorpusReport {
        spec_spine_version: version.to_string(),
        ready,
        lifecycle,
        status_source,
    })
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

    // Recorded 2026-09-25 from the published 0.25.0 and 0.26.0.
    #[test]
    fn a_check_refusal_is_read_under_both_exit_tables() {
        let read =
            |code, text: &str| check_refusal(Some(code), "p".into(), "v".into(), text, "d".into());
        let stale = "spec-registry: STALE\n1 stale shard(s):\ncodebase-index: STALE (run `spec-spine index`)";
        assert!(matches!(read(2, stale), ReportError::LedgerStale { .. }));
        assert!(matches!(read(1, stale), ReportError::LedgerStale { .. }));
        assert!(matches!(
            read(1, "spec-registry: INVALID"),
            ReportError::CorpusDoesNotCompile { .. }
        ));
        for (code, pin) in [
            (
                3,
                "spec-spine: config error: this repository requires spec-spine =0.1.0",
            ),
            (
                2,
                "spec-spine: refused: this repository requires spec-spine =0.1.0",
            ),
        ] {
            assert!(
                matches!(read(code, pin), ReportError::ProducerRefused { .. }),
                "exit {code}"
            );
        }
        assert!(matches!(
            read(4, "spec-spine: internal error: x"),
            ReportError::ProducerRefused { .. }
        ));
    }

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

    fn recorded(producer: &str, report: &str) -> Vec<u8> {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("testdata/producer")
                .join(producer)
                .join(report),
        )
        .unwrap()
    }

    // Section 3.1.2 rule 4, against the pinned producer's own output.
    #[test]
    fn the_released_producer_joins_as_before_and_says_status_came_from_list_alone() {
        let list = recorded("released-0.20.0", "list.json");
        let report = join(
            "0.20.0",
            &recorded("released-0.20.0", "plan.json"),
            &list,
            &list,
        )
        .unwrap();
        assert_eq!(report.status_source, StatusSource::ListOnly);
        assert!(report.ready.iter().all(|r| r.status.is_none()));
        assert_eq!(report.ready[0].id, "002-environment-lifecycle");
        assert_eq!(
            report
                .lifecycle_of("002-environment-lifecycle")
                .unwrap()
                .status,
            "approved"
        );
    }

    // Rules 1 and 2, against the expansion producer's own output.
    #[test]
    fn the_expansion_producer_status_is_compared_and_agrees() {
        let list = recorded("expansion-3b67b63d", "list.json");
        let report = join(
            "0.22.0",
            &recorded("expansion-3b67b63d", "plan.json"),
            &list,
            &list,
        )
        .unwrap();
        assert_eq!(report.status_source, StatusSource::ListAgreeingWithPlan);
        assert_eq!(report.ready[0].status.as_deref(), Some("approved"));
        // `implementation` still comes from the list, the only report with it.
        assert_eq!(
            report
                .lifecycle_of("002-environment-lifecycle")
                .unwrap()
                .implementation
                .as_deref(),
            Some("in-progress")
        );
    }

    // Rules 1 and 2, against the published 0.23.0's own output: the pinned
    // producer since 2026-09-23 (spec 003 section 5).
    #[test]
    fn the_published_producer_status_is_compared_and_agrees() {
        let list = recorded("released-0.23.0", "list.json");
        let report = join(
            "0.23.0",
            &recorded("released-0.23.0", "plan.json"),
            &list,
            &list,
        )
        .unwrap();
        assert_eq!(report.status_source, StatusSource::ListAgreeingWithPlan);
        assert_eq!(report.ready[0].id, "002-environment-lifecycle");
        assert_eq!(report.ready[0].status.as_deref(), Some("approved"));
        assert_eq!(
            report
                .lifecycle_of("002-environment-lifecycle")
                .unwrap()
                .implementation
                .as_deref(),
            Some("in-progress")
        );
    }

    // Rule 2: one flipped value in the expansion producer's own plan.
    #[test]
    fn a_plan_status_that_contradicts_the_list_is_refused_naming_both() {
        let list = recorded("expansion-3b67b63d", "list.json");
        let plan = String::from_utf8(recorded("expansion-3b67b63d", "plan.json"))
            .unwrap()
            .replacen("\"status\": \"approved\"", "\"status\": \"draft\"", 1);
        match join("0.22.0", plan.as_bytes(), &list, &list) {
            Err(e @ ReportError::Disagreement { .. }) => {
                let text = e.to_string();
                assert!(
                    text.contains("`draft`") && text.contains("`approved`"),
                    "{text}"
                );
                assert!(text.contains("002-environment-lifecycle"), "{text}");
            }
            other => panic!("expected a disagreement, got {other:?}"),
        }
    }

    // Rule 3: the ledger moved between the bracketing reads.
    #[test]
    fn a_ledger_that_moved_between_reads_is_refused_and_nothing_is_compared() {
        let first = recorded("expansion-3b67b63d", "list.json");
        let second = recorded("released-0.20.0", "list.json");
        // The plan would disagree with the second list; it is never compared.
        assert!(matches!(
            join(
                "0.22.0",
                &recorded("expansion-3b67b63d", "plan.json"),
                &first,
                &second
            ),
            Err(ReportError::Moved { .. })
        ));
    }

    // Rule 4: a plan of two shapes.
    #[test]
    fn a_plan_with_status_on_some_rows_only_is_unreadable() {
        let list = recorded("expansion-3b67b63d", "list.json");
        let plan = serde_json::json!({"ready": [
            {"id": "002-environment-lifecycle", "title": "x", "status": "approved"},
            {"id": "003-work-and-run-semantics", "title": "y"}
        ]});
        assert!(matches!(
            join("0.22.0", plan.to_string().as_bytes(), &list, &list),
            Err(ReportError::Unreadable { .. })
        ));
    }
}
