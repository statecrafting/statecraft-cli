//! Acceptance of a fixed-export documentation workflow: spec 022 sections
//! 3.6, 3.8 and 3.9, through its additive `extends` edge on this crate
//! (spec 005 owns it).
//!
//! [`judge`] takes spec 021's judgment of the attempt's current subject and
//! the workflow's latest version, and refuses to let either stand in for
//! the other. The workflow must be this profile's, hold its digest, have
//! reached `validated`, and name the same task, manifest and raw result the
//! judgment's subject names. Every workflow finding withholds acceptance; a
//! prohibited effect, or a proposed patch whose paths escape, rejects. The
//! outcome is never better than spec 021's.
//!
//! [`inspect_patch`] reads a proposed patch as data: its declared base, the
//! paths it names, its digest and whether it is binary. Applicability is
//! never checked here, so it is `not-recorded`; nothing is applied.
//!
//! [`record`] appends the judgment and the workflow's `judged` version. An
//! accepted subject stays inert: no application or publication follows.

use serde::Serialize;

use statecraft_envelope::hash::sha256_hex;
use statecraft_run::context::custody_digest;
use statecraft_run::documentation::review::CoreTuple;
use statecraft_run::documentation::workflow::{
    self, PROFILE, WORKFLOW_SCHEMA, Workflow, WorkflowState,
};
use statecraft_run::documentation::{Disposition, DocumentationTask};
use statecraft_run::record::{Chain, Entry, Identity as RecordIdentity, Kind, RecordError};

use crate::documentation_acceptance::{BOUNDARY_NOTICE, Judgment, Outcome};

/// The subject of a recorded workflow judgment.
pub const ACCEPTANCE_SUBJECT: &str = "fixed-export-acceptance";
/// The construction a subject's digest is computed by.
pub const SUBJECT_DIGEST_CONSTRUCTION: &str = "statecraft/fixed-export-subject-digest/1";

/// `sha256:` over the construction word, one LF, and the tuple's JSON. Two
/// generations differ in it whenever one byte of an artifact differs.
pub fn subject_digest(subject: &CoreTuple) -> String {
    let mut bytes = SUBJECT_DIGEST_CONSTRUCTION.as_bytes().to_vec();
    bytes.push(b'\n');
    bytes.extend(serde_json::to_vec(subject).expect("a tuple serializes"));
    format!("sha256:{}", sha256_hex(&bytes))
}

/// A proposed patch, read as data (section 3.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchReport {
    /// The logical path of the artifact that carries it.
    pub artifact: String,
    /// `sha256:` over its exact bytes.
    pub digest: String,
    /// The `base-commit:` it declares, or `not-recorded`.
    pub base: String,
    /// The paths it would change, in first-occurrence order.
    pub paths: Vec<String>,
    /// Whether it carries a binary change.
    pub binary: bool,
    /// Always `not-recorded`: no applicability check runs here.
    pub applicability: &'static str,
    /// Every finding, each with a stable code word.
    pub findings: Vec<String>,
}

fn escapes(path: &str) -> bool {
    path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..")
}

/// Section 3.6: parse `bytes` as a unified diff without applying it. A
/// path that is absolute or climbs out is `path-escape`; bytes that are not
/// text, or name no path, are findings too.
pub fn inspect_patch(artifact: &str, bytes: &[u8]) -> PatchReport {
    let mut report = PatchReport {
        artifact: artifact.to_string(),
        digest: custody_digest(bytes),
        base: "not-recorded".to_string(),
        paths: Vec::new(),
        binary: false,
        applicability: "not-recorded",
        findings: Vec::new(),
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        report
            .findings
            .push("not-text: the patch is not UTF-8".into());
        return report;
    };
    for line in text.lines() {
        if let Some(base) = line.strip_prefix("base-commit: ") {
            report.base = base.trim().to_string();
        } else if line.starts_with("GIT binary patch")
            || (line.starts_with("Binary files ") && line.ends_with(" differ"))
        {
            report.binary = true;
        } else if let Some(rest) = line
            .strip_prefix("--- ")
            .or_else(|| line.strip_prefix("+++ "))
        {
            let path = rest.split('\t').next().unwrap_or(rest).trim();
            if path == "/dev/null" {
                continue;
            }
            let path = path
                .strip_prefix("a/")
                .or_else(|| path.strip_prefix("b/"))
                .unwrap_or(path);
            if escapes(path) {
                report.findings.push(format!("path-escape: {path:?}"));
            }
            if !report.paths.iter().any(|p| p == path) {
                report.paths.push(path.to_string());
            }
        }
    }
    if report.paths.is_empty() {
        report
            .findings
            .push("no-paths: the patch names no path".into());
    }
    report
}

/// One workflow judgment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowJudgment {
    /// Always [`PROFILE`].
    pub profile: &'static str,
    /// The workflow version judged.
    pub workflow_digest: Option<String>,
    /// The judged subject's digest; absent when spec 021 identified none.
    pub subject_digest: Option<String>,
    /// Spec 021's judgment of that subject, unchanged.
    pub documentation: Judgment,
    /// Every proposed patch in the subject, read as data.
    pub patches: Vec<PatchReport>,
    /// What the workflow adds, each with a stable code word.
    pub reasons: Vec<String>,
    /// The overall result: never better than spec 021's.
    pub outcome: Outcome,
    /// Section 3.9's boundary, repeated.
    pub notice: &'static str,
}

/// Sections 3.8 and 3.9: judge the workflow's current subject. `retained`
/// reads artifact bytes by digest.
pub fn judge(
    workflow: Result<&Workflow, &str>,
    task: &DocumentationTask,
    documentation: Judgment,
    retained: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> WorkflowJudgment {
    let mut reasons = Vec::new();
    let mut rejects = false;
    let mut digest = None;
    match workflow {
        Err(why) => reasons.push(format!("workflow-unreadable: {why}")),
        Ok(w) => {
            digest = Some(w.workflow_digest.clone());
            if w.schema != WORKFLOW_SCHEMA || w.profile != PROFILE {
                reasons.push(format!("workflow-profile: {}", w.profile));
            }
            if w.computed_digest() != w.workflow_digest {
                reasons.push("workflow-digest: the workflow does not hold its digest".into());
            }
            if !w
                .transitions
                .iter()
                .any(|t| t.state == WorkflowState::Validated)
            {
                reasons.push(format!("workflow-not-validated: {:?}", w.state()));
            }
            if w.subjects.adapter.is_none() {
                reasons.push("workflow-adapter: no adapter was recorded".into());
            }
            if w.subjects.task_digest != task.task_digest {
                reasons.push("workflow-subject: task".into());
            }
            if let Some(s) = &documentation.subject {
                if w.subjects.manifest_digest != s.context_manifest_digest {
                    reasons.push("workflow-subject: manifest".into());
                }
                if w.subjects.raw_result.as_deref() != Some(s.result_custody_digest.as_str()) {
                    reasons.push("workflow-subject: raw result".into());
                }
                if w.run_id != s.run_id || w.attempt != s.attempt {
                    reasons.push("workflow-subject: attempt".into());
                }
            }
            for f in &w.findings {
                rejects |= f.starts_with("prohibited-effect");
                reasons.push(format!("workflow-finding: {f}"));
            }
        }
    }
    let mut patches = Vec::new();
    if let Some(s) = &documentation.subject {
        for a in &s.artifacts {
            let is_patch = task
                .artifacts
                .iter()
                .any(|t| t.path == a.logical_path && t.disposition == Disposition::Patch);
            if !is_patch {
                continue;
            }
            match retained(&a.digest) {
                Some(bytes) => {
                    let report = inspect_patch(&a.logical_path, &bytes);
                    for f in &report.findings {
                        rejects |= f.starts_with("path-escape");
                        reasons.push(format!("patch {}: {f}", a.logical_path));
                    }
                    patches.push(report);
                }
                None => reasons.push(format!("patch {}: not-retained", a.logical_path)),
            }
        }
    }
    let outcome = match documentation.outcome {
        Outcome::Rejected => Outcome::Rejected,
        _ if rejects => Outcome::Rejected,
        Outcome::Accepted if reasons.is_empty() => Outcome::Accepted,
        _ => Outcome::NoAcceptance,
    };
    WorkflowJudgment {
        profile: PROFILE,
        workflow_digest: digest,
        subject_digest: documentation.subject.as_ref().map(subject_digest),
        documentation,
        patches,
        reasons,
        outcome,
        notice: BOUNDARY_NOTICE,
    }
}

impl WorkflowJudgment {
    /// `sha256:` over the judgment's JSON.
    pub fn digest(&self) -> String {
        custody_digest(&serde_json::to_vec(self).expect("a judgment serializes"))
    }
}

/// Append the judgment, then the workflow's `judged` version naming it.
/// Nothing earlier is rewritten, and nothing is applied or published.
pub fn record(
    chain: &mut Chain,
    workflow: &Workflow,
    judgment: &WorkflowJudgment,
    at: &str,
) -> Result<Workflow, RecordError> {
    let digest = judgment.digest();
    let n = chain.entries().len();
    chain.append(
        &format!(
            "{}/{}/{ACCEPTANCE_SUBJECT}/{n}",
            workflow.run_id, workflow.attempt
        ),
        at,
        &Entry {
            kind: Kind::Evidence,
            run_id: workflow.run_id.clone(),
            attempt: workflow.attempt,
            subject: ACCEPTANCE_SUBJECT.to_string(),
            idempotency_key: None,
            detail: serde_json::json!({ "judgmentDigest": digest, "judgment": judgment }),
            effect_id: RecordIdentity::Absent,
        },
    )?;
    let outcome = serde_json::to_value(judgment.outcome)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    workflow::advance(chain, workflow, WorkflowState::Judged, &outcome, at, |w| {
        w.subjects.acceptances.push(digest.clone())
    })
}
