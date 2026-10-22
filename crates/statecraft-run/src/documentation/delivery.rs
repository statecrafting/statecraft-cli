//! The adapter's delivery observations for a documentation task: spec 020
//! section 3.4.
//!
//! The init event reports what the adapter says it applied. [`fold`] compares
//! that testimony with the recorded task and keeps every missing, mismatched,
//! duplicated or contradictory observation as a finding; any finding makes
//! the later result ineligible for acceptance. Testimony is never proof that
//! a provider received or used anything.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{DocumentationTask, Skill};
use crate::record::{Chain, Entry, Identity as RecordIdentity, Kind, RecordError};

/// The subject of a recorded delivery fold.
pub const DELIVERY_SUBJECT: &str = "documentation-delivery";

/// The word an adapter reports for a task that needs no skill.
pub const SKILL_NOT_REQUIRED: &str = "not-required";

/// What an adapter's init event says it applied for a documentation task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Reported {
    /// The task digest.
    pub task_digest: String,
    /// The context manifest digest.
    pub context_manifest_digest: String,
    /// The skill digest, or exactly [`SKILL_NOT_REQUIRED`].
    pub skill: String,
    /// The result schema digest.
    pub result_schema_digest: String,
    /// The capability tokens the adapter says it applied.
    pub capabilities: Vec<String>,
}

/// One attempt's delivery, as the fold decided it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Delivery {
    /// Every report, in stream order.
    pub reported: Vec<Reported>,
    /// Every finding, each with a stable code word.
    pub findings: Vec<String>,
    /// Whether a later result may be considered for acceptance at all.
    pub eligible: bool,
}

/// Compare the adapter's reports with the task. No report, more than one,
/// or one that does not repeat a member exactly is a finding; nothing is
/// assumed from an absence.
pub fn fold(task: &DocumentationTask, reported: &[Reported]) -> Delivery {
    let mut findings = Vec::new();
    match reported {
        [] => findings.push("not-reported: the init event carried no documentation report".into()),
        [_] => {}
        [first, rest @ ..] => {
            if rest.iter().all(|r| r == first) {
                findings.push(format!("duplicate: {} identical reports", reported.len()));
            } else {
                findings.push(format!(
                    "contradictory: {} reports that differ",
                    reported.len()
                ));
            }
        }
    }
    let skill = match &task.skill {
        Skill::NotRequired(_) => SKILL_NOT_REQUIRED.to_string(),
        Skill::Pinned(s) => s.digest.clone(),
    };
    for r in reported {
        let mut mismatch = |member: &str, ok: bool| {
            if !ok {
                findings.push(format!("mismatch: {member}"));
            }
        };
        mismatch("taskDigest", r.task_digest == task.task_digest);
        mismatch(
            "contextManifestDigest",
            r.context_manifest_digest == task.context_manifest_digest,
        );
        mismatch("skill", r.skill == skill);
        mismatch(
            "resultSchemaDigest",
            r.result_schema_digest == task.result_schema.digest,
        );
        for c in &task.required_capabilities {
            mismatch(
                &format!("capabilities ({c} not applied)"),
                r.capabilities.contains(c),
            );
        }
    }
    findings.sort();
    findings.dedup();
    Delivery {
        reported: reported.to_vec(),
        eligible: findings.is_empty(),
        findings,
    }
}

/// Append the fold, bound to the attempt and its task digest.
pub fn record_delivery(
    chain: &mut Chain,
    task: &DocumentationTask,
    delivery: &Delivery,
    timestamp: &str,
) -> Result<(), RecordError> {
    let n = chain.entries().len();
    chain.append(
        &format!("{}/{}/{DELIVERY_SUBJECT}/{n}", task.run_id, task.attempt),
        timestamp,
        &Entry {
            kind: Kind::Evidence,
            run_id: task.run_id.clone(),
            attempt: task.attempt,
            subject: DELIVERY_SUBJECT.to_string(),
            idempotency_key: None,
            detail: serde_json::json!({
                "binding": {
                    "runId": task.run_id,
                    "attempt": task.attempt,
                    "taskDigest": task.task_digest,
                },
                "delivery": delivery,
            }),
            effect_id: RecordIdentity::Absent,
        },
    )
}

/// The delivery recorded for the task's attempt. `None` when none was;
/// a record that binds but cannot be read, or more than one, is an
/// ineligible delivery with a finding, never absent.
pub fn recorded_delivery(chain: &Chain, task: &DocumentationTask) -> Option<Delivery> {
    let mut found: Vec<Option<Delivery>> = chain
        .entries()
        .into_iter()
        .filter(|e| {
            e.kind == Kind::Evidence
                && e.subject == DELIVERY_SUBJECT
                && e.run_id == task.run_id
                && e.attempt == task.attempt
                && e.detail
                    .get("binding")
                    .and_then(|b| b.get("taskDigest"))
                    .and_then(Value::as_str)
                    == Some(task.task_digest.as_str())
        })
        .map(|e| {
            e.detail
                .get("delivery")
                .cloned()
                .and_then(|v| serde_json::from_value(v).ok())
        })
        .collect();
    let finding = |why: &str| Delivery {
        reported: Vec::new(),
        findings: vec![why.to_string()],
        eligible: false,
    };
    match found.len() {
        0 => None,
        1 => Some(
            found
                .remove(0)
                .unwrap_or_else(|| finding("unreadable: the delivery record")),
        ),
        _ => Some(finding("duplicate: more than one delivery record")),
    }
}
