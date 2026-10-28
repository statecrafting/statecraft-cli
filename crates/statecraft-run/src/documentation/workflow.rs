//! The fixed-export documentation workflow record: spec 022 sections 3.1,
//! 3.2 and 3.4, through its additive `extends` edge on this crate.
//!
//! [`prepare`] admits a documentation attempt to the
//! `fixed-export-documentation/1` profile only from bytes already retained
//! in the run boundary, pinned to immutable revisions, under a `no-spend`
//! task, and writes one `statecraft/fixed-export-documentation/1` record
//! that names every subject by identity. Repeating it with the same inputs
//! reports the existing record; different inputs for the same attempt refuse.
//!
//! [`retain_events`] keeps every adapter event's exact bytes, in order,
//! under custody, before anything parses them, and names what is wrong with
//! the sequence as findings. [`advance`] appends a new version of the record
//! for each state transition; nothing earlier is rewritten.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use statecraft_envelope::hash::sha256_hex;

use super::{CostCeiling, DocumentationTask, Skill};
use crate::context::{self, CustodyError, Manifest};
use crate::record::{Chain, Entry, Identity as RecordIdentity, Kind, RecordError};

/// The profile token.
pub const PROFILE: &str = "fixed-export-documentation/1";
/// The workflow record's closed schema word.
pub const WORKFLOW_SCHEMA: &str = "statecraft/fixed-export-documentation/1";
/// The construction the workflow digest is computed by.
pub const WORKFLOW_DIGEST_CONSTRUCTION: &str = "statecraft/fixed-export-workflow-digest/1";
/// The subject of a workflow record version.
pub const WORKFLOW_SUBJECT: &str = "fixed-export-workflow";

/// One retained packet page, by identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct PacketRef {
    pub repository: String,
    pub revision: String,
    pub tree: String,
    pub dirty_state: String,
    pub page_digest: String,
    pub custody_digest: String,
}

/// An adapter by name and version: at preparation, the one the run policy
/// names, with no capabilities; once executed, the one selected, with the
/// capabilities it declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdapterRef {
    /// The adapter's name.
    pub name: String,
    /// Its version.
    pub version: String,
    /// The capability tokens it declared.
    pub capabilities: Vec<String>,
}

/// One retained adapter event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventRef {
    /// Its position in the stream, from 0.
    pub sequence: u64,
    /// The media type of its bytes.
    pub media_type: String,
    /// Its exact length.
    pub byte_length: u64,
    /// `sha256:` over exactly its bytes.
    pub custody_digest: String,
}

/// Every subject the workflow identifies (section 3.1).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct Subjects {
    pub manifest_digest: String,
    pub packets: Vec<PacketRef>,
    pub task_digest: String,
    /// The pinned skill's digest, or `not-required`.
    pub skill: String,
    pub result_schema_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<AdapterRef>,
    pub events: Vec<EventRef>,
    /// The raw terminal result's custody digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_result: Option<String>,
    pub reviews: Vec<String>,
    pub corrections: Vec<String>,
    pub acceptances: Vec<String>,
}

/// The workflow's states, in the order they can be reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum WorkflowState {
    Prepared,
    Executed,
    Validated,
    Reviewed,
    Corrected,
    Judged,
}

/// One state transition and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Transition {
    /// The state entered.
    pub state: WorkflowState,
    /// A stable reason.
    pub reason: String,
    /// When, `YYYY-MM-DDTHH:MM:SSZ`.
    pub at: String,
}

/// One `statecraft/fixed-export-documentation/1` record version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct Workflow {
    pub schema: String,
    pub profile: String,
    /// The registered project.
    pub project: String,
    pub run_id: String,
    pub attempt: u32,
    /// The prepared workspace's candidate revision.
    pub candidate: String,
    pub subjects: Subjects,
    pub transitions: Vec<Transition>,
    /// Every finding, each with a stable code word, in the order found.
    pub findings: Vec<String>,
    pub digest_construction: String,
    pub workflow_digest: String,
}

impl Workflow {
    /// The digest of every member but `workflowDigest`.
    pub fn computed_digest(&self) -> String {
        let mut unsigned = self.clone();
        unsigned.workflow_digest = String::new();
        let mut bytes = WORKFLOW_DIGEST_CONSTRUCTION.as_bytes().to_vec();
        bytes.push(b'\n');
        bytes.extend(serde_json::to_vec(&unsigned).expect("a workflow serializes"));
        format!("sha256:{}", sha256_hex(&bytes))
    }

    fn seal(mut self) -> Self {
        self.digest_construction = WORKFLOW_DIGEST_CONSTRUCTION.to_string();
        self.workflow_digest = self.computed_digest();
        self
    }

    /// The state the latest transition entered.
    pub fn state(&self) -> Option<WorkflowState> {
        self.transitions.last().map(|t| t.state)
    }
}

/// Why preparation refuses. Each carries a stable code word.
#[derive(Debug, thiserror::Error)]
pub enum PrepareError {
    /// An input is not the attempt's, or names another subject.
    #[error("binding: {0}")]
    Binding(String),
    /// A packet names a revision that is not an immutable object id.
    #[error("moving-input: {0}")]
    MovingInput(String),
    /// A page's bytes are not retained under its custody digest.
    #[error("not-retained: {0}")]
    NotRetained(String),
    /// The task permits spend.
    #[error("spend: the fixed-export profile requires no-spend")]
    Spend,
    /// The skill bytes are absent, unexpected, or not the pinned ones.
    #[error("skill: {0}")]
    Skill(String),
    /// The attempt already has a workflow for other inputs.
    #[error("competing-workflow: run {run} attempt {attempt} is prepared for other inputs")]
    Competing {
        /// The run.
        run: String,
        /// The attempt.
        attempt: u32,
    },
    /// Custody could not be read or written.
    #[error(transparent)]
    Custody(#[from] CustodyError),
    /// The record could not be appended.
    #[error(transparent)]
    Record(#[from] RecordError),
}

/// A revision is immutable only as a full object id.
fn object_id(revision: &str) -> bool {
    matches!(revision.len(), 40 | 64)
        && revision
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Every workflow version recorded for the attempt, in record order.
pub fn versions(chain: &Chain, run_id: &str, attempt: u32) -> Vec<Result<Workflow, String>> {
    chain
        .entries()
        .into_iter()
        .filter(|e| {
            e.kind == Kind::Evidence
                && e.subject == WORKFLOW_SUBJECT
                && e.run_id == run_id
                && e.attempt == attempt
        })
        .map(|e| {
            let w: Workflow = e
                .detail
                .get("workflow")
                .cloned()
                .ok_or_else(|| "the record carries no workflow".to_string())
                .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))?;
            if w.schema != WORKFLOW_SCHEMA || w.computed_digest() != w.workflow_digest {
                return Err("the workflow does not hold its digest".into());
            }
            Ok(w)
        })
        .collect()
}

/// The attempt's current workflow: its latest version.
pub fn current(chain: &Chain, run_id: &str, attempt: u32) -> Option<Result<Workflow, String>> {
    versions(chain, run_id, attempt).pop()
}

fn append(chain: &mut Chain, w: &Workflow, at: &str) -> Result<(), RecordError> {
    let n = chain.entries().len();
    chain.append(
        &format!("{}/{}/{WORKFLOW_SUBJECT}/{n}", w.run_id, w.attempt),
        at,
        &Entry {
            kind: Kind::Evidence,
            run_id: w.run_id.clone(),
            attempt: w.attempt,
            subject: WORKFLOW_SUBJECT.to_string(),
            idempotency_key: None,
            detail: serde_json::json!({ "workflow": w }),
            effect_id: RecordIdentity::Absent,
        },
    )
}

/// What preparation found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    /// The workflow, new or existing.
    pub workflow: Workflow,
    /// Whether it already existed with these inputs.
    pub existing: bool,
}

/// Sections 3.2 and 3.3: the subjects a preparation would record, or why
/// it refuses. It writes nothing to the record, so a caller can refuse
/// before it writes an intent; the pinned skill's bytes are retained.
/// `adapter` is the run policy's adapter, by name and version, which is the
/// only one execution may select.
pub fn subjects(
    home: &std::path::Path,
    manifest: &Manifest,
    task: &DocumentationTask,
    skill: Option<&[u8]>,
    adapter: &AdapterRef,
) -> Result<Subjects, PrepareError> {
    if task.context_manifest_digest != manifest.manifest_digest
        || task.run_id != manifest.run_id
        || task.attempt != manifest.attempt
    {
        return Err(PrepareError::Binding(
            "the task and the manifest name different subjects".into(),
        ));
    }
    if !matches!(task.cost_ceiling, CostCeiling::NoSpend(_)) {
        return Err(PrepareError::Spend);
    }
    let skill = match (&task.skill, skill) {
        (Skill::NotRequired(_), None) => "not-required".to_string(),
        (Skill::NotRequired(_), Some(_)) => {
            return Err(PrepareError::Skill("the task needs no skill".into()));
        }
        (Skill::Pinned(_), None) => return Err(PrepareError::Skill("absent".into())),
        (Skill::Pinned(p), Some(bytes)) => {
            if context::custody_digest(bytes) != p.digest {
                return Err(PrepareError::Skill("not the pinned bytes".into()));
            }
            context::retain(home, bytes)?
        }
    };
    let mut packets = Vec::new();
    for repo in &manifest.repositories {
        if !object_id(&repo.snapshot.revision) {
            return Err(PrepareError::MovingInput(format!(
                "repository {} names revision {:?}",
                repo.id, repo.snapshot.revision
            )));
        }
        for page in &repo.pages {
            if context::read_retained(home, &page.custody_digest)?.is_none() {
                return Err(PrepareError::NotRetained(format!(
                    "repository {} page {}",
                    repo.id, page.page_digest
                )));
            }
            packets.push(PacketRef {
                repository: repo.id.clone(),
                revision: repo.snapshot.revision.clone(),
                tree: repo.snapshot.tree.clone(),
                dirty_state: repo.snapshot.dirty_state.clone(),
                page_digest: page.page_digest.clone(),
                custody_digest: page.custody_digest.clone(),
            });
        }
    }
    Ok(Subjects {
        manifest_digest: manifest.manifest_digest.clone(),
        packets,
        task_digest: task.task_digest.clone(),
        skill,
        result_schema_digest: task.result_schema.digest.clone(),
        adapter: Some(adapter.clone()),
        ..Subjects::default()
    })
}

/// Section 3.2: admit the attempt to the profile from retained, pinned
/// bytes, or refuse before any adapter is selected.
#[allow(clippy::too_many_arguments)]
pub fn prepare(
    home: &std::path::Path,
    chain: &mut Chain,
    project: &str,
    candidate: &str,
    manifest: &Manifest,
    task: &DocumentationTask,
    skill: Option<&[u8]>,
    adapter: &AdapterRef,
    at: &str,
) -> Result<Prepared, PrepareError> {
    let subjects = subjects(home, manifest, task, skill, adapter)?;
    if let Some(found) = current(chain, &task.run_id, task.attempt) {
        let found = found.map_err(PrepareError::Binding)?;
        let same = found.subjects.manifest_digest == subjects.manifest_digest
            && found.subjects.task_digest == subjects.task_digest
            && found.subjects.packets == subjects.packets
            && found.subjects.skill == subjects.skill
            && found.subjects.adapter == subjects.adapter
            && found.candidate == candidate
            && found.project == project;
        return if same {
            Ok(Prepared {
                workflow: found,
                existing: true,
            })
        } else {
            Err(PrepareError::Competing {
                run: task.run_id.clone(),
                attempt: task.attempt,
            })
        };
    }
    let workflow = Workflow {
        schema: WORKFLOW_SCHEMA.to_string(),
        profile: PROFILE.to_string(),
        project: project.to_string(),
        run_id: task.run_id.clone(),
        attempt: task.attempt,
        candidate: candidate.to_string(),
        subjects,
        transitions: vec![Transition {
            state: WorkflowState::Prepared,
            reason: "fixed exports retained and pinned".into(),
            at: at.to_string(),
        }],
        findings: vec![],
        digest_construction: String::new(),
        workflow_digest: String::new(),
    }
    .seal();
    append(chain, &workflow, at)?;
    Ok(Prepared {
        workflow,
        existing: false,
    })
}

/// Append a new version of `workflow` with `change` applied and one more
/// transition. Earlier versions are never rewritten.
pub fn advance(
    chain: &mut Chain,
    workflow: &Workflow,
    state: WorkflowState,
    reason: &str,
    at: &str,
    change: impl FnOnce(&mut Workflow),
) -> Result<Workflow, RecordError> {
    let mut next = workflow.clone();
    change(&mut next);
    next.transitions.push(Transition {
        state,
        reason: reason.to_string(),
        at: at.to_string(),
    });
    let next = next.seal();
    append(chain, &next, at)?;
    Ok(next)
}

/// What the retained events say about the stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedEvents {
    /// Every event, in order.
    pub events: Vec<EventRef>,
    /// The first terminal event's custody digest, if any.
    pub first_terminal: Option<String>,
    /// What is wrong with the sequence: a second terminal, an event after
    /// termination, or no terminal at all.
    pub findings: Vec<String>,
}

/// Section 3.4: keep every event's exact bytes under custody, in order,
/// before parsing any of them. A line counts as terminal when it is a JSON
/// object whose `event` is `result`; a line that is not JSON is still kept, as
/// `application/octet-stream`.
pub fn retain_events(
    home: &std::path::Path,
    lines: &[Vec<u8>],
) -> Result<RetainedEvents, CustodyError> {
    let mut events = Vec::new();
    let mut first_terminal = None;
    let mut findings = Vec::new();
    for (i, bytes) in lines.iter().enumerate() {
        let custody = context::retain(home, bytes)?;
        let parsed = serde_json::from_slice::<Value>(bytes).ok();
        let terminal = parsed
            .as_ref()
            .and_then(|v| {
                v.get("event")
                    .and_then(Value::as_str)
                    .map(|e| e == "result")
            })
            .unwrap_or(false);
        if first_terminal.is_some() {
            findings.push(if terminal {
                format!("second-terminal: event {i}")
            } else {
                format!("after-terminal: event {i}")
            });
        } else if terminal {
            first_terminal = Some(custody.clone());
        }
        events.push(EventRef {
            sequence: i as u64,
            media_type: if parsed.is_some() {
                "application/json"
            } else {
                "application/octet-stream"
            }
            .to_string(),
            byte_length: bytes.len() as u64,
            custody_digest: custody,
        });
    }
    if first_terminal.is_none() {
        findings.push("no-terminal: the stream ended without a result event".into());
    }
    Ok(RetainedEvents {
        events,
        first_terminal,
        findings,
    })
}
