//! The documentation task and its structured result in the run record: spec
//! 020, through its additive `extends` edge on this crate (spec 003 owns it;
//! spec 020 owns no code).
//!
//! Two closed documents, kept apart (section 3.1):
//!
//! - [`DocumentationTask`] is `statecraft/documentation-task/1`, the work and
//!   its limits. [`DocumentationTask::decode`] refuses anything but the closed
//!   schema, the exact version 1 operation sets and `publication:
//!   prohibited`, and the session writes it into the attempt's intent under
//!   [`INTENT_KEY`] before any adapter process exists.
//! - [`DocumentationResult`] is `statecraft/documentation-result/1`, what the
//!   provider returned: testimony, never acceptance. [`record_result`] keeps
//!   the raw bytes under their custody digest before decoding them, and a
//!   decoding failure is recorded beside the bytes, never repaired.
//!
//! [`project`] folds both for `run show` from the chain and nothing else.
//!
//! The task half is this module; the result half is [`result`], re-exported
//! here.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use statecraft_envelope::hash::sha256_hex;

use crate::context::{self, Bounds};
use crate::record::{Chain, Kind};
use crate::session::INTENT_SUBJECT;

pub mod result;
pub use result::*;

/// The task's closed schema word.
pub const TASK_SCHEMA: &str = "statecraft/documentation-task/1";
/// The result's closed schema word.
pub const RESULT_SCHEMA: &str = "statecraft/documentation-result/1";
/// The named construction the task digest is computed by.
pub const TASK_DIGEST_CONSTRUCTION: &str = "statecraft/documentation-task-digest/1";
/// Where the task sits in the attempt's intent detail.
pub const INTENT_KEY: &str = "documentationTask";
/// The subject of a recorded result.
pub const RESULT_SUBJECT: &str = "documentation-result";

/// The exact bytes of the result schema version 1 delivers before spawn.
pub const RESULT_SCHEMA_BYTES: &[u8] = include_bytes!("../../schemas/documentation-result-1.json");

/// The digest a task must name for [`RESULT_SCHEMA_BYTES`].
pub fn result_schema_digest() -> String {
    context::custody_digest(RESULT_SCHEMA_BYTES)
}

/// Raw-byte limits on a task, applied before anything decodes it.
pub const TASK_BOUNDS: Bounds = Bounds {
    total_bytes: 1024 * 1024,
    pages: 1,
    members: 65_536,
    depth: 16,
    string_bytes: 64 * 1024,
};

/// Raw-byte limits on a result, applied before anything decodes it.
pub const RESULT_BOUNDS: Bounds = Bounds {
    total_bytes: 16 * 1024 * 1024,
    pages: 1,
    members: 262_144,
    depth: 32,
    string_bytes: 1024 * 1024,
};

/// The longest string any member of either document may hold.
pub const TEXT_BYTES: usize = 16 * 1024;
/// The most elements any collection of either document may hold.
pub const ITEMS: usize = 4096;

/// One operation token. The set is closed: an unknown token refuses decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum Operation {
    ReadContext,
    DraftArtifact,
    ProposePatch,
    ApplyPatch,
    Commit,
    Push,
    OpenPullRequest,
    Publish,
    Release,
    Deploy,
    RemoteAdmin,
    CredentialRead,
    ProviderSelect,
}

/// Version 1's allowed set, exactly (section 3.3).
pub const ALLOWED_V1: [Operation; 3] = [
    Operation::ReadContext,
    Operation::DraftArtifact,
    Operation::ProposePatch,
];

/// Version 1's forbidden set, exactly (section 3.3, invariant I-2).
pub const FORBIDDEN_V1: [Operation; 10] = [
    Operation::ApplyPatch,
    Operation::Commit,
    Operation::Push,
    Operation::OpenPullRequest,
    Operation::Publish,
    Operation::Release,
    Operation::Deploy,
    Operation::RemoteAdmin,
    Operation::CredentialRead,
    Operation::ProviderSelect,
];

/// The two operation sets, both exhaustive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Operations {
    /// What the provider may do.
    pub allowed: Vec<Operation>,
    /// What no task field can grant.
    pub forbidden: Vec<Operation>,
}

impl Operations {
    /// Version 1's sets, in their canonical order.
    pub fn v1() -> Self {
        Self {
            allowed: ALLOWED_V1.to_vec(),
            forbidden: FORBIDDEN_V1.to_vec(),
        }
    }
}

/// The object the requested artifact describes or changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Candidate {
    /// The repository id.
    pub repository: String,
    /// The immutable revision.
    pub revision: String,
    /// The tree.
    pub tree: String,
}

/// The closed set of output formats version 1 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    /// Markdown.
    Markdown,
    /// Plain text.
    PlainText,
}

/// One format token plus its version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutputFormat {
    /// The token.
    pub format: Format,
    /// Its version.
    pub version: String,
}

/// The word `not-required`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotRequired {
    /// The task needs no skill.
    #[serde(rename = "not-required")]
    NotRequired,
}

/// An exact skill identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillIdentity {
    /// The skill's id.
    pub id: String,
    /// Its version.
    pub version: String,
    /// Its content digest.
    pub digest: String,
}

/// The skill a task needs, or the explicit word that it needs none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Skill {
    /// `not-required`.
    NotRequired(NotRequired),
    /// An exact identity.
    Pinned(SkillIdentity),
}

/// The structured-result schema a task names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultSchemaRef {
    /// Always [`RESULT_SCHEMA`].
    pub id: String,
    /// Always [`result_schema_digest`].
    pub digest: String,
}

/// The clock a deadline is enforced by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClockSource {
    /// The supervisor's system clock, read as UTC.
    SupervisorUtc,
}

/// An absolute deadline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Deadline {
    /// `YYYY-MM-DDTHH:MM:SSZ`.
    pub not_after: String,
    /// Who enforces it.
    pub clock: ClockSource,
}

/// The word `no-spend`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoSpend {
    /// The task may spend nothing.
    #[serde(rename = "no-spend")]
    NoSpend,
}

/// How a ceiling is enforced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Enforcement {
    /// The attempt is stopped when the ceiling is reached.
    StopAttempt,
}

/// A spending ceiling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ceiling {
    /// An ISO 4217 code.
    pub currency: String,
    /// The maximum, in the currency's minor units.
    pub maximum_minor_units: u64,
    /// How it is enforced.
    pub enforcement: Enforcement,
}

/// The cost ceiling, or the explicit word that the task spends nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CostCeiling {
    /// `no-spend`.
    NoSpend(NoSpend),
    /// A ceiling.
    Ceiling(Ceiling),
}

/// What an expected artifact is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Disposition {
    /// Result bytes, under `draft-artifact`.
    Draft,
    /// A patch as a result artifact, under `propose-patch`; never applied.
    Patch,
}

/// The context pages one artifact requires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequiredContext {
    /// A repository id of the spec 019 manifest.
    pub repository: String,
    /// Page digests of that repository's packet.
    pub pages: Vec<String>,
}

/// One expected artifact: a logical destination for review, not write
/// authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Artifact {
    /// The logical path.
    pub path: String,
    /// What it is.
    pub disposition: Disposition,
    /// The context it requires.
    pub required_context: Vec<RequiredContext>,
}

/// Whether semantic review is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum SemanticReview {
    Required,
    NotRequired,
}

/// Whether the task candidate may differ from the workspace candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum CandidateDivergence {
    Permitted,
    Prohibited,
}

/// The review policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewPolicy {
    /// Required reviewers, in order.
    pub reviewers: Vec<String>,
    /// Whether semantic review is required.
    pub semantic_review: SemanticReview,
    /// The acceptance route.
    pub acceptance_route: String,
    /// Absent is silent, and a silent policy prohibits divergence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_divergence: Option<CandidateDivergence>,
}

/// The word `prohibited`, the only publication value version 1 has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Publication {
    /// Publication is prohibited.
    #[serde(rename = "prohibited")]
    Prohibited,
}

/// One `statecraft/documentation-task/1` document (section 3.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(missing_docs)]
pub struct DocumentationTask {
    pub schema: String,
    pub task_id: String,
    pub run_id: String,
    pub attempt: u32,
    pub candidate: Candidate,
    pub context_manifest_digest: String,
    pub objective: String,
    pub audiences: Vec<String>,
    pub output_format: OutputFormat,
    pub operations: Operations,
    pub required_capabilities: Vec<String>,
    pub skill: Skill,
    pub result_schema: ResultSchemaRef,
    pub deadline: Deadline,
    pub cost_ceiling: CostCeiling,
    pub artifacts: Vec<Artifact>,
    pub review_policy: ReviewPolicy,
    pub publication: Publication,
    /// Always [`TASK_DIGEST_CONSTRUCTION`].
    pub digest_construction: String,
    /// The digest of every other member, by [`TASK_DIGEST_CONSTRUCTION`].
    pub task_digest: String,
}

/// Why a task or a result refuses. Each carries a stable code word.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DocumentError {
    /// The bytes exceed the raw bounds.
    #[error("bounds: {0}")]
    Bounds(String),
    /// A string or a collection exceeds the member bounds.
    #[error("member-bounds: {0}")]
    MemberBounds(String),
    /// The bytes are not the closed schema.
    #[error("malformed: {0}")]
    Malformed(String),
    /// The operation sets are not version 1's exactly.
    #[error("operations: {0}")]
    Operations(String),
    /// The result schema is not the one version 1 delivers.
    #[error("result-schema: {0}")]
    ResultSchema(String),
    /// The stated digest is not the computed one.
    #[error("digest: stated {stated}, computed {computed}")]
    Digest {
        /// What the document states.
        stated: String,
        /// What its members compute to.
        computed: String,
    },
    /// An identity, a path or a reference is not acceptable.
    #[error("invalid: {0}")]
    Invalid(String),
    /// An identifier appears twice.
    #[error("duplicate: {0}")]
    Duplicate(String),
    /// A reference names something outside the task or the manifest.
    #[error("outside-task: {0}")]
    OutsideTask(String),
    /// The task and workspace candidates differ under a policy that does
    /// not permit it.
    #[error("candidate-divergence: task {task}, workspace {workspace}")]
    CandidateDivergence {
        /// The task's candidate revision.
        task: String,
        /// The prepared workspace's revision.
        workspace: String,
    },
}

/// Walk a decoded value and refuse any string longer than [`TEXT_BYTES`] or
/// any collection longer than [`ITEMS`], naming where.
fn member_bounds(value: &Value, path: &mut String) -> Result<(), DocumentError> {
    match value {
        Value::String(s) if s.len() > TEXT_BYTES => Err(DocumentError::MemberBounds(format!(
            "{path} exceeds {TEXT_BYTES} bytes"
        ))),
        Value::Array(items) => {
            if items.len() > ITEMS {
                return Err(DocumentError::MemberBounds(format!(
                    "{path} exceeds {ITEMS} items"
                )));
            }
            for (i, item) in items.iter().enumerate() {
                let len = path.len();
                path.push_str(&format!("[{i}]"));
                member_bounds(item, path)?;
                path.truncate(len);
            }
            Ok(())
        }
        Value::Object(members) => {
            for (k, v) in members {
                let len = path.len();
                path.push('.');
                path.push_str(k);
                member_bounds(v, path)?;
                path.truncate(len);
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Bounds first, then the member bounds, then the closed schema.
pub(crate) fn decode_closed<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    bounds: &Bounds,
) -> Result<T, DocumentError> {
    context::scan(&[bytes], bounds).map_err(|e| DocumentError::Bounds(e.to_string()))?;
    let value: Value =
        serde_json::from_slice(bytes).map_err(|e| DocumentError::Malformed(e.to_string()))?;
    member_bounds(&value, &mut String::from("$"))?;
    serde_json::from_value(value).map_err(|e| DocumentError::Malformed(e.to_string()))
}

pub(crate) fn safe_id(what: &str, id: &str) -> Result<(), DocumentError> {
    let ok = !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
    if ok {
        Ok(())
    } else {
        Err(DocumentError::Invalid(format!("{what} {id:?}")))
    }
}

pub(crate) fn digest_form(what: &str, digest: &str) -> Result<(), DocumentError> {
    let ok = digest.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    });
    if ok {
        Ok(())
    } else {
        Err(DocumentError::Invalid(format!("{what} {digest:?}")))
    }
}

pub(crate) fn nonempty(what: &str, s: &str) -> Result<(), DocumentError> {
    if s.trim().is_empty() {
        Err(DocumentError::Invalid(format!("{what} is empty")))
    } else {
        Ok(())
    }
}

/// A logical path: relative, `/`-separated, and never escaping.
pub(crate) fn logical_path(path: &str) -> Result<(), DocumentError> {
    let ok = !path.is_empty()
        && path.len() <= 512
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains('\0')
        && path
            .split('/')
            .all(|seg| !seg.is_empty() && seg != "." && seg != "..");
    if ok {
        Ok(())
    } else {
        Err(DocumentError::Invalid(format!("logical path {path:?}")))
    }
}

pub(crate) fn unique<'a>(
    what: &str,
    items: impl IntoIterator<Item = &'a str>,
) -> Result<(), DocumentError> {
    let mut seen = BTreeSet::new();
    for item in items {
        if !seen.insert(item) {
            return Err(DocumentError::Duplicate(format!("{what} {item:?}")));
        }
    }
    Ok(())
}

fn exact_set(what: &str, got: &[Operation], want: &[Operation]) -> Result<(), DocumentError> {
    let set: BTreeSet<_> = got.iter().collect();
    if set.len() != got.len() {
        return Err(DocumentError::Operations(format!("{what} repeats a token")));
    }
    let want: BTreeSet<_> = want.iter().collect();
    if set != want {
        let extra: Vec<_> = set.difference(&want).collect();
        let missing: Vec<_> = want.difference(&set).collect();
        return Err(DocumentError::Operations(format!(
            "{what} is not version 1's set: extra {extra:?}, missing {missing:?}"
        )));
    }
    Ok(())
}

fn timestamp(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 20
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            19 => *c == b'Z',
            _ => c.is_ascii_digit(),
        })
}

impl DocumentationTask {
    /// The digest [`TASK_DIGEST_CONSTRUCTION`] names: `sha256:` over the
    /// construction word, one LF, and the document's JSON with `taskDigest`
    /// empty. Every collection is ordered and every object is a struct, so
    /// the bytes are fixed by the value.
    pub fn computed_digest(&self) -> String {
        let mut unsigned = self.clone();
        unsigned.task_digest = String::new();
        let mut bytes = TASK_DIGEST_CONSTRUCTION.as_bytes().to_vec();
        bytes.push(b'\n');
        bytes.extend(serde_json::to_vec(&unsigned).expect("a task serializes"));
        format!("sha256:{}", sha256_hex(&bytes))
    }

    /// Set the construction and the digest from the other members.
    pub fn seal(mut self) -> Self {
        self.digest_construction = TASK_DIGEST_CONSTRUCTION.to_string();
        self.task_digest = self.computed_digest();
        self
    }

    /// The task's exact bytes, as the intent and the adapter request carry
    /// them.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a task serializes")
    }

    /// Decode raw task bytes under the closed schema and validate them
    /// (sections 3.2 and 3.3). Nothing is repaired: any refusal is final.
    pub fn decode(bytes: &[u8]) -> Result<Self, DocumentError> {
        let task: Self = decode_closed(bytes, &TASK_BOUNDS)?;
        task.validate()?;
        Ok(task)
    }

    /// Every rule a decoded task must meet.
    pub fn validate(&self) -> Result<(), DocumentError> {
        if self.schema != TASK_SCHEMA {
            return Err(DocumentError::Malformed(format!(
                "schema {:?} is not {TASK_SCHEMA}",
                self.schema
            )));
        }
        if self.digest_construction != TASK_DIGEST_CONSTRUCTION {
            return Err(DocumentError::Malformed(format!(
                "digest construction {:?} is not {TASK_DIGEST_CONSTRUCTION}",
                self.digest_construction
            )));
        }
        safe_id("task id", &self.task_id)?;
        safe_id("run id", &self.run_id)?;
        if self.attempt == 0 {
            return Err(DocumentError::Invalid("attempt 0".into()));
        }
        safe_id("candidate repository", &self.candidate.repository)?;
        nonempty("candidate revision", &self.candidate.revision)?;
        nonempty("candidate tree", &self.candidate.tree)?;
        digest_form("context manifest digest", &self.context_manifest_digest)?;
        nonempty("objective", &self.objective)?;
        if self.audiences.is_empty() {
            return Err(DocumentError::Invalid("audiences is empty".into()));
        }
        for a in &self.audiences {
            nonempty("audience", a)?;
        }
        unique("audience", self.audiences.iter().map(String::as_str))?;
        nonempty("output format version", &self.output_format.version)?;
        exact_set("allowed", &self.operations.allowed, &ALLOWED_V1)?;
        exact_set("forbidden", &self.operations.forbidden, &FORBIDDEN_V1)?;
        for c in &self.required_capabilities {
            safe_id("capability", c)?;
        }
        unique(
            "capability",
            self.required_capabilities.iter().map(String::as_str),
        )?;
        if let Skill::Pinned(s) = &self.skill {
            safe_id("skill id", &s.id)?;
            nonempty("skill version", &s.version)?;
            digest_form("skill digest", &s.digest)?;
        }
        if self.result_schema.id != RESULT_SCHEMA {
            return Err(DocumentError::ResultSchema(format!(
                "{:?} is not {RESULT_SCHEMA}",
                self.result_schema.id
            )));
        }
        if self.result_schema.digest != result_schema_digest() {
            return Err(DocumentError::ResultSchema(format!(
                "digest {:?} is not the delivered schema's {}",
                self.result_schema.digest,
                result_schema_digest()
            )));
        }
        if !timestamp(&self.deadline.not_after) {
            return Err(DocumentError::Invalid(format!(
                "deadline {:?}",
                self.deadline.not_after
            )));
        }
        if let CostCeiling::Ceiling(c) = &self.cost_ceiling
            && !(c.currency.len() == 3 && c.currency.bytes().all(|b| b.is_ascii_uppercase()))
        {
            return Err(DocumentError::Invalid(format!("currency {:?}", c.currency)));
        }
        for a in &self.artifacts {
            logical_path(&a.path)?;
            for rc in &a.required_context {
                safe_id("context repository", &rc.repository)?;
                for p in &rc.pages {
                    digest_form("context page", p)?;
                }
                unique("context page", rc.pages.iter().map(String::as_str))?;
            }
            unique(
                "context repository",
                a.required_context.iter().map(|r| r.repository.as_str()),
            )?;
        }
        unique(
            "artifact path",
            self.artifacts.iter().map(|a| a.path.as_str()),
        )?;
        if self.review_policy.reviewers.is_empty() {
            return Err(DocumentError::Invalid("reviewers is empty".into()));
        }
        for r in &self.review_policy.reviewers {
            nonempty("reviewer", r)?;
        }
        unique(
            "reviewer",
            self.review_policy.reviewers.iter().map(String::as_str),
        )?;
        nonempty("acceptance route", &self.review_policy.acceptance_route)?;
        let computed = self.computed_digest();
        if computed != self.task_digest {
            return Err(DocumentError::Digest {
                stated: self.task_digest.clone(),
                computed,
            });
        }
        Ok(())
    }

    /// Section 3.3: the task may narrow the manifest's delivery but never
    /// add to it. Every page an artifact requires must be a page of that
    /// repository in the manifest, and the manifest must be this task's.
    pub fn check_manifest(&self, manifest: &context::Manifest) -> Result<(), DocumentError> {
        if manifest.manifest_digest != self.context_manifest_digest {
            return Err(DocumentError::OutsideTask(format!(
                "the task names manifest {}, the attempt records {}",
                self.context_manifest_digest, manifest.manifest_digest
            )));
        }
        if manifest.run_id != self.run_id || manifest.attempt != self.attempt {
            return Err(DocumentError::OutsideTask(
                "the manifest binds another attempt".into(),
            ));
        }
        if manifest.task != context::Task::Id(self.task_id.clone()) {
            return Err(DocumentError::OutsideTask(format!(
                "the manifest does not name task {}",
                self.task_id
            )));
        }
        for a in &self.artifacts {
            for rc in &a.required_context {
                let Some(repo) = manifest.repositories.iter().find(|r| r.id == rc.repository)
                else {
                    return Err(DocumentError::OutsideTask(format!(
                        "artifact {} requires repository {}, not in the manifest",
                        a.path, rc.repository
                    )));
                };
                for p in &rc.pages {
                    if !repo.pages.iter().any(|page| &page.page_digest == p) {
                        return Err(DocumentError::OutsideTask(format!(
                            "artifact {} requires page {p} of {}, not in the manifest",
                            a.path, rc.repository
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// Section 3.2: a task candidate that differs from the prepared
    /// workspace's revision is refused unless the review policy explicitly
    /// permits it. Both identities stay visible in the refusal.
    pub fn check_candidate(&self, workspace_revision: &str) -> Result<(), DocumentError> {
        if self.candidate.revision == workspace_revision {
            return Ok(());
        }
        match self.review_policy.candidate_divergence {
            Some(CandidateDivergence::Permitted) => Ok(()),
            _ => Err(DocumentError::CandidateDivergence {
                task: self.candidate.revision.clone(),
                workspace: workspace_revision.to_string(),
            }),
        }
    }
}

/// The task recorded in an attempt's intent, if its intent carries one.
///
/// `Err` when the intent carries a value that is not a valid task, or a task
/// for another binding: such a value binds nothing and is never read as
/// absent.
pub fn recorded_task(
    chain: &Chain,
    run_id: &str,
    attempt: u32,
) -> Result<Option<DocumentationTask>, String> {
    let entries = chain.entries();
    let intent = entries.iter().find(|e| {
        e.kind == Kind::Intent
            && e.subject == INTENT_SUBJECT
            && e.run_id == run_id
            && e.attempt == attempt
    });
    let Some(value) = intent.and_then(|e| e.detail.get(INTENT_KEY)) else {
        return Ok(None);
    };
    let task: DocumentationTask =
        serde_json::from_value(value.clone()).map_err(|e| format!("unreadable task: {e}"))?;
    task.validate().map_err(|e| e.to_string())?;
    if task.run_id != run_id || task.attempt != attempt {
        return Err("the recorded task does not bind this attempt".to_string());
    }
    Ok(Some(task))
}
