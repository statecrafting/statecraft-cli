//! The fixed-export profile's offline invocation boundary: spec 022 sections
//! 3.3 and 3.4, through its additive `extends` edge on this crate (spec 004
//! owns it).
//!
//! [`select`] picks the adapter the recorded workflow names, exactly, and
//! refuses one that does not declare every [`ProfileCapability`]. [`admit`]
//! verifies the request before any process starts: the workflow is prepared,
//! and the request carries exactly the recorded manifest, pages, task, schema
//! and skill. The provider never selects itself and a real provider is never
//! a fallback.
//!
//! [`run_fake`] is the pilot's deterministic test double. It receives the
//! same [`Request`] and writes the same [`Event`] protocol as any adapter; it
//! reads no clock, draws no random value and makes no network, credential,
//! repository or publication call. A scripted attempt at any of those goes to
//! a [`Denial`], which refuses it, keeps it, and makes the fake emit a
//! structured refusal.
//!
//! [`record`] keeps every event line under custody before parsing it, folds
//! the adapter's delivery testimony, keeps the first terminal result's raw
//! bytes, and appends the `executed` and `validated` versions of the
//! workflow. A prohibited effect, a stated length or digest that is not the
//! bytes', and every sequence finding stay on the workflow as findings.

use std::path::Path;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use statecraft_run::context::{self as run_context, CustodyError, Manifest, custody_digest};
use statecraft_run::documentation::delivery;
use statecraft_run::documentation::workflow::{self, AdapterRef, Workflow, WorkflowState};
use statecraft_run::documentation::{
    self as doc, Binding, CostCeiling, DocumentationTask, Party, RecordResultError, Validation,
};
use statecraft_run::record::{Chain, RecordError};

use crate::capability::Capability;
use crate::documentation::RawResult;
use crate::protocol::{Classification, Event, Request};

/// What an adapter must declare to run under the profile (section 3.3).
///
/// A closed set of its own, apart from spec 004's six provider capability
/// tokens: these say what the adapter does with the request, not what it
/// can ask of a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileCapability {
    /// The task's exact bytes reach the provider.
    ExactTaskBytes,
    /// The packet, skill and schema's exact bytes reach the provider.
    ExactInputBytes,
    /// The input is bounded and local, with no network requirement.
    OfflineInput,
    /// The terminal result is returned as exact bytes with their custody.
    ResultCustody,
    /// Execution is a deterministic fake, for the pilot.
    DeterministicFake,
}

impl ProfileCapability {
    /// The token as it is written.
    pub fn token(self) -> &'static str {
        match self {
            ProfileCapability::ExactTaskBytes => "exact-task-bytes",
            ProfileCapability::ExactInputBytes => "exact-input-bytes",
            ProfileCapability::OfflineInput => "offline-input",
            ProfileCapability::ResultCustody => "result-custody",
            ProfileCapability::DeterministicFake => "deterministic-fake",
        }
    }
}

/// Every capability the profile requires, in canonical order.
pub const REQUIRED: [ProfileCapability; 5] = [
    ProfileCapability::ExactTaskBytes,
    ProfileCapability::ExactInputBytes,
    ProfileCapability::OfflineInput,
    ProfileCapability::ResultCustody,
    ProfileCapability::DeterministicFake,
];

/// What one available adapter declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Declaration {
    /// The adapter's name.
    pub name: String,
    /// Its version.
    pub version: String,
    /// The profile capabilities it declares.
    pub profile: Vec<ProfileCapability>,
    /// Spec 004's provider capabilities it supports.
    pub supports: Vec<Capability>,
}

impl Declaration {
    /// The workflow's reference to it.
    pub fn reference(&self) -> AdapterRef {
        AdapterRef {
            name: self.name.clone(),
            version: self.version.clone(),
            capabilities: self.profile.iter().map(|c| c.token().to_string()).collect(),
        }
    }
}

/// The pilot's fake adapter's name.
pub const FAKE_NAME: &str = "fixed-export-fake";
/// Its version.
pub const FAKE_VERSION: &str = "1";
/// The guard its structured refusals name.
pub const BOUNDARY_GUARD: &str = "fixed-export-boundary";

/// The fake's declaration: every profile capability, and structured refusals.
pub fn fake_declaration() -> Declaration {
    Declaration {
        name: FAKE_NAME.to_string(),
        version: FAKE_VERSION.to_string(),
        profile: REQUIRED.to_vec(),
        supports: vec![Capability::StructuredRefusals],
    }
}

/// Why selection or admission refuses, before any process starts. Each
/// carries a stable code word.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    /// No available adapter is exactly the one the policy names.
    #[error("adapter-not-selected: {0}")]
    NotSelected(String),
    /// The selected adapter does not declare a required profile capability.
    #[error("capability-missing: {0}")]
    CapabilityMissing(String),
    /// The workflow is not prepared for this request.
    #[error("workflow: {0}")]
    Workflow(String),
    /// The request's context bundle is not the recorded manifest's.
    #[error(transparent)]
    Context(#[from] crate::context::BundleError),
    /// The request's documentation bundle is not the recorded task's.
    #[error(transparent)]
    Documentation(#[from] crate::documentation::BundleError),
    /// The task permits spend.
    #[error("spend: the fixed-export profile requires no-spend")]
    Spend,
}

/// Section 3.3: select exactly the adapter `policy` names from `available`.
/// The first exact name and version wins; nothing else is a fallback.
pub fn select<'a>(
    policy: &AdapterRef,
    available: &'a [Declaration],
) -> Result<&'a Declaration, Refusal> {
    let selected = available
        .iter()
        .find(|d| d.name == policy.name && d.version == policy.version)
        .ok_or_else(|| Refusal::NotSelected(format!("{} {}", policy.name, policy.version)))?;
    let missing: Vec<&str> = REQUIRED
        .iter()
        .filter(|c| !selected.profile.contains(c))
        .map(|c| c.token())
        .collect();
    if !missing.is_empty() {
        return Err(Refusal::CapabilityMissing(missing.join(", ")));
    }
    Ok(selected)
}

/// Section 3.3: before spawn, the workflow is prepared for exactly this
/// manifest and task, the request carries their exact bytes, every byte
/// reference agrees with retained custody, and the task spends nothing.
pub fn admit(
    request: &Request,
    workflow: &Workflow,
    manifest: &Manifest,
    task: &DocumentationTask,
    selected: &Declaration,
) -> Result<(), Refusal> {
    if workflow.state() != Some(WorkflowState::Prepared) {
        return Err(Refusal::Workflow(format!(
            "the workflow is {:?}, not prepared",
            workflow.state()
        )));
    }
    if workflow.subjects.manifest_digest != manifest.manifest_digest
        || workflow.subjects.task_digest != task.task_digest
        || request.attempt.run_id != workflow.run_id
        || request.attempt.number != workflow.attempt
    {
        return Err(Refusal::Workflow(
            "the request does not name the prepared subjects".into(),
        ));
    }
    if !matches!(task.cost_ceiling, CostCeiling::NoSpend(_)) {
        return Err(Refusal::Spend);
    }
    crate::context::check(request.context.as_ref(), Some(manifest))?;
    crate::documentation::check(
        request.documentation.as_ref(),
        Some(task),
        &selected.supports,
    )?;
    Ok(())
}

/// One effect the profile prohibits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "effect", content = "target")]
pub enum Effect {
    /// A network request.
    Network(String),
    /// A credential read.
    CredentialRead(String),
    /// A write outside the run record, including the repository.
    RepositoryWrite(String),
    /// A publication.
    Publication(String),
    /// A shell escape.
    Shell(String),
    /// A clock read.
    Clock,
    /// A random choice.
    Random,
}

impl Effect {
    /// One line naming the effect and its target.
    pub fn describe(&self) -> String {
        match self {
            Effect::Network(t) => format!("network {t}"),
            Effect::CredentialRead(t) => format!("credential-read {t}"),
            Effect::RepositoryWrite(t) => format!("repository-write {t}"),
            Effect::Publication(t) => format!("publication {t}"),
            Effect::Shell(t) => format!("shell {t}"),
            Effect::Clock => "clock".to_string(),
            Effect::Random => "random".to_string(),
        }
    }
}

/// The deterministic denial boundary the pilot runs inside (section 3.3).
/// Every attempt is refused and kept; nothing is performed.
#[derive(Debug, Default)]
pub struct Denial {
    attempted: Mutex<Vec<Effect>>,
}

impl Denial {
    /// A boundary with no attempt yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Refuse `effect` and keep it. Always `Err`: the boundary has no
    /// allowance to grant.
    pub fn attempt(&self, effect: Effect) -> Result<(), Effect> {
        self.attempted
            .lock()
            .expect("the denial boundary is not poisoned")
            .push(effect.clone());
        Err(effect)
    }

    /// Every attempted effect, in order.
    pub fn attempted(&self) -> Vec<Effect> {
        self.attempted
            .lock()
            .expect("the denial boundary is not poisoned")
            .clone()
    }
}

/// One step of the fake adapter's fixture script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// An init event testifying to exactly what the request carried.
    Init,
    /// An init event testifying to another task digest.
    InitForTask(String),
    /// A progress event.
    Progress(String),
    /// An attempt at a prohibited effect, made through the denial boundary.
    Attempt(Effect),
    /// A terminal event returning these exact bytes as `application/json`.
    Terminal(Vec<u8>),
    /// A terminal event with a stated media type, length and digest that
    /// may disagree with the bytes.
    TerminalStated(RawResult),
    /// A line written as is.
    Raw(Vec<u8>),
}

/// Section 3.10: run the fake adapter's `script` against `request`, and
/// return the event lines it writes. Deterministic: the same request and
/// script give the same bytes.
pub fn run_fake(
    request: &Request,
    task: &DocumentationTask,
    script: &[Step],
    denial: &Denial,
) -> Vec<Vec<u8>> {
    let applied = vec![Capability::StructuredRefusals];
    let line = |e: &Event| serde_json::to_vec(e).expect("an event serializes");
    let mut out = Vec::new();
    for step in script {
        match step {
            Step::Init | Step::InitForTask(_) => {
                let mut documentation = request
                    .documentation
                    .as_ref()
                    .map(|b| b.delivered(task, &applied));
                if let (Step::InitForTask(digest), Some(d)) = (step, documentation.as_mut()) {
                    d.task_digest = digest.clone();
                }
                out.push(line(&Event::Init {
                    applied: applied.clone(),
                    adapter_version: FAKE_VERSION.to_string(),
                    provider_version: format!("{FAKE_NAME}/{FAKE_VERSION}"),
                    context: request.context.as_ref().map(|b| b.delivered()),
                    documentation,
                }));
            }
            Step::Progress(message) => out.push(line(&Event::Progress {
                message: message.clone(),
            })),
            Step::Attempt(effect) => {
                if let Err(refused) = denial.attempt(effect.clone()) {
                    out.push(line(&Event::Refusal {
                        guard: BOUNDARY_GUARD.to_string(),
                        detail: refused.describe(),
                    }));
                }
            }
            Step::Terminal(bytes) => {
                let text = String::from_utf8_lossy(bytes).into_owned();
                out.push(line(&Event::Result {
                    classification: Classification::Completed,
                    cost: None,
                    documentation: Some(RawResult {
                        media_type: "application/json".to_string(),
                        byte_length: text.len() as u64,
                        custody_digest: custody_digest(text.as_bytes()),
                        bytes: text,
                    }),
                }));
            }
            Step::TerminalStated(raw) => out.push(line(&Event::Result {
                classification: Classification::Completed,
                cost: None,
                documentation: Some(raw.clone()),
            })),
            Step::Raw(bytes) => out.push(bytes.clone()),
        }
    }
    out
}

/// The pilot's adapter and provider parties, as the result must repeat them.
pub fn fake_parties() -> (Party, Party) {
    (
        Party {
            name: FAKE_NAME.to_string(),
            version: FAKE_VERSION.to_string(),
        },
        Party {
            name: format!("{FAKE_NAME}-provider"),
            version: FAKE_VERSION.to_string(),
        },
    )
}

/// Why execution could not be recorded.
#[derive(Debug, thiserror::Error)]
pub enum RecordExecutionError {
    /// The event bytes could not be kept.
    #[error(transparent)]
    Custody(#[from] CustodyError),
    /// The result could not be kept or recorded.
    #[error(transparent)]
    Result(#[from] RecordResultError),
    /// A record could not be appended.
    #[error(transparent)]
    Record(#[from] RecordError),
}

/// Sections 3.4 and 3.5: keep every event line, fold the delivery
/// testimony, keep the first terminal result's raw bytes, and append the
/// workflow's `executed` and `validated` versions. Returns the latest.
#[allow(clippy::too_many_arguments)]
pub fn record(
    home: &Path,
    chain: &mut Chain,
    prepared: &Workflow,
    manifest: &Manifest,
    task: &DocumentationTask,
    selected: &Declaration,
    binding: &Binding,
    lines: &[Vec<u8>],
    denial: &Denial,
    at: &str,
) -> Result<Workflow, RecordExecutionError> {
    let retained = workflow::retain_events(home, lines)?;
    let mut findings = retained.findings.clone();
    let mut events = Vec::new();
    for (i, bytes) in lines.iter().enumerate() {
        match serde_json::from_slice::<Event>(bytes) {
            Ok(e) => events.push(e),
            Err(_) => findings.push(format!("unreadable-event: event {i}")),
        }
    }
    for observation in crate::context::observation(&events, None) {
        run_context::record_delivery(chain, manifest, &observation, at)?;
    }
    let fold = delivery::fold(task, &crate::documentation::reported(&events));
    findings.extend(fold.findings.iter().map(|f| format!("delivery {f}")));
    delivery::record_delivery(chain, task, &fold, at)?;
    for effect in denial.attempted() {
        findings.push(format!("prohibited-effect: {}", effect.describe()));
    }
    // The first terminal event is the result; a later one is already a
    // sequence finding and is kept, never chosen.
    let first = events.iter().find_map(|e| match e {
        Event::Result { documentation, .. } => Some(documentation.clone()),
        _ => None,
    });
    let mut raw_result = None;
    let validation = match first {
        Some(Some(raw)) => {
            if raw.byte_length != raw.bytes.len() as u64 {
                findings.push("raw-length-mismatch: the stated length is not the bytes'".into());
            }
            if raw.custody_digest != custody_digest(raw.bytes.as_bytes()) {
                findings.push("raw-digest-mismatch: the stated digest is not the bytes'".into());
            }
            if raw.media_type != "application/json" {
                findings.push(format!("media-type: {}", raw.media_type));
            }
            let v = doc::record_result(
                home,
                chain,
                task,
                binding,
                &raw.media_type,
                raw.bytes.as_bytes(),
                at,
            )?;
            raw_result = Some(custody_digest(raw.bytes.as_bytes()));
            Some(v)
        }
        Some(None) => {
            findings.push("no-result: the terminal event carried no documentation result".into());
            None
        }
        None => None,
    };
    let executed = workflow::advance(
        chain,
        prepared,
        WorkflowState::Executed,
        &format!("{} events retained", retained.events.len()),
        at,
        |w| {
            w.subjects.adapter = Some(selected.reference());
            w.subjects.events = retained.events.clone();
            w.subjects.raw_result = raw_result.clone();
            w.findings.extend(findings.iter().cloned());
        },
    )?;
    let reason = match &validation {
        None => "not-recorded: no raw result".to_string(),
        Some(Validation::Bound { .. }) => "bound".to_string(),
        Some(Validation::SchemaInvalid { reason }) => format!("schema-invalid: {reason}"),
        Some(Validation::Unbound { field }) => format!("unbound: {field}"),
    };
    Ok(workflow::advance(
        chain,
        &executed,
        WorkflowState::Validated,
        &reason,
        at,
        |_| {},
    )?)
}
