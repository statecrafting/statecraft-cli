//! Remote desired state (spec 012).
//!
//! A setup profile renders one committed, canonical JSON document that
//! declares what the profile expects of the remote repository: required
//! checks, extra required jobs, code-owner review, merge queue, the
//! review-exception Environment and its reviewers, workflow-token defaults,
//! secret names, repository custom properties, and the profile identity
//! (section 3.1). It declares expectations only: it is not an apply plan,
//! not proof of remote configuration, and not authority to change a
//! repository.
//!
//! `doctor --remote` reads the document and compares each leaf with
//! read-only host operations (sections 3.2 and 3.3), in
//! [`compare`]. Nothing here writes remote state or reads a secret value.

use crate::setup::{Host, HostError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The document's schema version.
pub const SCHEMA: &str = "statecraft/remote-desired-state/1";
/// Where the profile renders it.
pub const PATH: &str = ".statecraft/setup/github-actions-rust.remote.json";
/// The aggregate job that requires each declared extra job.
pub const AGGREGATE: &str = "ci-gate";
/// The visibility classes a required secret may be satisfied from.
pub const SECRET_VISIBILITY: [&str; 2] = ["repository", "organization"];

/// The desired-state document. Unknown fields refuse a parse; an omitted
/// optional field is no claim, never a default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesiredState {
    /// [`SCHEMA`].
    pub schema: String,
    /// The profile that rendered it.
    pub profile: ProfileClaim,
    /// The branch the rules target.
    pub default_branch: String,
    /// Each required status check and the App that must report it.
    pub required_checks: Vec<RequiredCheck>,
    /// Every declared `ci.extra_required_jobs` entry and its aggregation.
    pub extra_required_jobs: ExtraJobs,
    /// Code-owner review and the governed paths `CODEOWNERS` must cover.
    pub code_owner_review: CodeOwnerReview,
    /// The merge queue, when the project declares one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_queue: Option<MergeQueue>,
    /// The review-exception Environment.
    pub review_exception: ReviewException,
    /// The workflow token's defaults.
    pub workflow_token: WorkflowToken,
    /// Required secrets, by name only.
    pub secrets: Vec<SecretRequirement>,
    /// Repository custom properties, when the project declares any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_properties: Option<CustomProperties>,
}

/// The rendering profile's identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileClaim {
    /// Profile id.
    pub id: String,
    /// Profile revision.
    pub revision: u32,
    /// Profile content identity.
    pub identity: String,
}

/// One required status check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequiredCheck {
    /// The check's context name.
    pub name: String,
    /// The GitHub App that must report it.
    pub app_id: u64,
}

/// The declared extra required jobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtraJobs {
    /// The job whose result requires each of them.
    pub aggregate: String,
    /// Each job and the reusable workflow it calls.
    pub jobs: Vec<ExtraJob>,
}

/// One declared extra required job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtraJob {
    /// The job id.
    pub job: String,
    /// The reusable workflow.
    pub workflow: String,
}

/// Code-owner review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodeOwnerReview {
    /// Whether the default branch requires it.
    pub required: bool,
    /// The governed paths `CODEOWNERS` must name owners for.
    pub paths: Vec<String>,
    /// The owners each path must name, when the project declares them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owners: Option<Vec<String>>,
}

/// The merge queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MergeQueue {
    /// Whether the default branch requires one.
    pub required: bool,
    /// `merge`, `squash` or `rebase`, when required.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_method: Option<String>,
    /// The most entries built at once, when required.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_concurrency: Option<u64>,
    /// Whether every queued entry must pass, when required.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_entries_must_pass: Option<bool>,
}

/// The review-exception Environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewException {
    /// The Environment's name.
    pub environment: String,
    /// The required reviewers, when the project declares them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewers: Option<Vec<String>>,
    /// Whether a deployment's author may not approve it.
    pub prevent_self_review: bool,
}

/// The workflow token's defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowToken {
    /// `read` or `write`.
    pub default_permissions: String,
    /// Whether Actions may create or approve pull requests.
    pub can_approve_pull_request_reviews: bool,
}

/// One required secret: satisfied by any one of its names, visible in an
/// allowed class. Never a value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SecretRequirement {
    /// The names, any one of which satisfies it.
    pub any_of: Vec<String>,
    /// The visibility classes that satisfy it.
    pub visibility: Vec<String>,
}

/// Repository custom properties.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CustomProperties {
    /// Who defines the property schema: always `organization` on GitHub.
    pub schema_owner: String,
    /// Each property's exact repository value.
    pub values: BTreeMap<String, String>,
}

impl DesiredState {
    /// Parse a document. Unknown fields and a foreign schema refuse.
    pub fn parse(bytes: &[u8]) -> Result<DesiredState, String> {
        let doc: DesiredState = serde_json::from_slice(bytes)
            .map_err(|e| format!("{PATH} is not a desired-state document: {e}"))?;
        if doc.schema != SCHEMA {
            return Err(format!(
                "{PATH} declares schema `{}`; this product reads `{SCHEMA}`",
                doc.schema
            ));
        }
        Ok(doc)
    }

    /// The canonical bytes: keys sorted, two-space indentation, a final
    /// newline.
    pub fn canonical(&self) -> Vec<u8> {
        let value = serde_json::to_value(self).expect("serializable");
        let mut text = serde_json::to_string_pretty(&value).expect("serializable");
        text.push('\n');
        text.into_bytes()
    }
}

/// What the project declares under `remote.*` (spec 012 section 3.1). Each
/// is optional: an omitted one is no claim.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteParameters {
    /// `remote.merge_queue`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_queue: Option<MergeQueue>,
    /// `remote.exception_reviewers`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exception_reviewers: Option<Vec<String>>,
    /// `remote.custom_properties`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_properties: Option<BTreeMap<String, String>>,
}

fn handle(key: &str, v: &str) -> Result<String, String> {
    let body = v.strip_prefix('@').unwrap_or("");
    let ok = !body.is_empty()
        && body.split('/').count() <= 2
        && body.split('/').all(|p| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        });
    if ok {
        Ok(v.to_string())
    } else {
        Err(format!(
            "{key} entry `{v}` is not an @user or @org/team handle"
        ))
    }
}

/// Validate `remote.merge_queue`: `{"required": false}`, or `required` true
/// with exactly `merge_method`, `build_concurrency` and
/// `all_entries_must_pass`.
pub fn merge_queue(value: &serde_json::Value) -> Result<MergeQueue, String> {
    const KEY: &str = "remote.merge_queue";
    let obj = value
        .as_object()
        .ok_or_else(|| format!("{KEY} must be an object"))?;
    let required = obj
        .get("required")
        .and_then(|v| v.as_bool())
        .ok_or_else(|| format!("{KEY}.required must be true or false"))?;
    let allowed: &[&str] = if required {
        &[
            "required",
            "merge_method",
            "build_concurrency",
            "all_entries_must_pass",
        ]
    } else {
        &["required"]
    };
    if let Some(other) = obj.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(if required {
            format!("{KEY}: unknown key `{other}`")
        } else {
            format!("{KEY}: `{other}` is declared only when the queue is required")
        });
    }
    if !required {
        return Ok(MergeQueue {
            required,
            merge_method: None,
            build_concurrency: None,
            all_entries_must_pass: None,
        });
    }
    let method = obj
        .get("merge_method")
        .and_then(|v| v.as_str())
        .filter(|m| ["merge", "squash", "rebase"].contains(m))
        .ok_or_else(|| format!("{KEY}.merge_method must be merge, squash or rebase"))?;
    let concurrency = obj
        .get("build_concurrency")
        .and_then(|v| v.as_u64())
        .filter(|n| (1..=100).contains(n))
        .ok_or_else(|| format!("{KEY}.build_concurrency must be an integer in 1..=100"))?;
    let all = obj
        .get("all_entries_must_pass")
        .and_then(|v| v.as_bool())
        .ok_or_else(|| format!("{KEY}.all_entries_must_pass must be true or false"))?;
    Ok(MergeQueue {
        required,
        merge_method: Some(method.to_string()),
        build_concurrency: Some(concurrency),
        all_entries_must_pass: Some(all),
    })
}

/// Validate `remote.exception_reviewers`: a nonempty list of distinct
/// handles.
pub fn exception_reviewers(value: &serde_json::Value) -> Result<Vec<String>, String> {
    const KEY: &str = "remote.exception_reviewers";
    let list = value
        .as_array()
        .filter(|l| !l.is_empty())
        .ok_or_else(|| format!("{KEY} must be a nonempty list of handles"))?;
    let mut out: Vec<String> = Vec::new();
    for item in list {
        let v = item
            .as_str()
            .ok_or_else(|| format!("{KEY} entries must be strings"))?;
        let h = handle(KEY, v)?;
        if out.iter().any(|o| o.eq_ignore_ascii_case(&h)) {
            return Err(format!("{KEY}: `{v}` is declared twice"));
        }
        out.push(h);
    }
    Ok(out)
}

/// Validate `remote.custom_properties`: a nonempty object of property name
/// to exact string value.
pub fn custom_properties(value: &serde_json::Value) -> Result<BTreeMap<String, String>, String> {
    const KEY: &str = "remote.custom_properties";
    let obj = value
        .as_object()
        .filter(|o| !o.is_empty())
        .ok_or_else(|| format!("{KEY} must be a nonempty object of name to string value"))?;
    let mut out = BTreeMap::new();
    for (name, v) in obj {
        let name_ok = (1..=75).contains(&name.len())
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '$' | '#'));
        if !name_ok {
            return Err(format!("{KEY}: `{name}` is not a custom property name"));
        }
        let v = v
            .as_str()
            .ok_or_else(|| format!("{KEY}.{name} must be a string"))?;
        out.insert(name.clone(), v.to_string());
    }
    Ok(out)
}

/// Everything the profile supplies to render the document.
pub struct Rendering<'a> {
    /// The profile's id, revision and identity.
    pub profile: ProfileClaim,
    /// The branch the rules target.
    pub default_branch: &'a str,
    /// The App that must report `ci-gate`.
    pub gate_app_id: u64,
    /// The declared extra jobs, as `(job, workflow)`.
    pub extra_jobs: Vec<(String, String)>,
    /// The governed paths `CODEOWNERS` must cover.
    pub governed_paths: Vec<String>,
    /// `review.code_owners`, empty when undeclared.
    pub code_owners: &'a [String],
    /// The review-exception Environment.
    pub exception_environment: &'a str,
    /// The reviewer credential names, any one of which satisfies the review.
    pub credentials: Vec<String>,
    /// What the project declared under `remote.*`.
    pub remote: &'a RemoteParameters,
}

/// Render the document.
pub fn render(r: &Rendering<'_>) -> DesiredState {
    let mut paths = r.governed_paths.clone();
    paths.sort();
    paths.dedup();
    DesiredState {
        schema: SCHEMA.to_string(),
        profile: r.profile.clone(),
        default_branch: r.default_branch.to_string(),
        required_checks: vec![RequiredCheck {
            name: AGGREGATE.to_string(),
            app_id: r.gate_app_id,
        }],
        extra_required_jobs: ExtraJobs {
            aggregate: AGGREGATE.to_string(),
            jobs: r
                .extra_jobs
                .iter()
                .map(|(job, workflow)| ExtraJob {
                    job: job.clone(),
                    workflow: workflow.clone(),
                })
                .collect(),
        },
        code_owner_review: CodeOwnerReview {
            required: true,
            paths,
            owners: (!r.code_owners.is_empty()).then(|| r.code_owners.to_vec()),
        },
        merge_queue: r.remote.merge_queue.clone(),
        review_exception: ReviewException {
            environment: r.exception_environment.to_string(),
            reviewers: r.remote.exception_reviewers.clone(),
            prevent_self_review: true,
        },
        workflow_token: WorkflowToken {
            default_permissions: "read".to_string(),
            can_approve_pull_request_reviews: false,
        },
        secrets: vec![SecretRequirement {
            any_of: r.credentials.clone(),
            visibility: SECRET_VISIBILITY.iter().map(|s| s.to_string()).collect(),
        }],
        custom_properties: r
            .remote
            .custom_properties
            .clone()
            .map(|values| CustomProperties {
                schema_owner: "organization".to_string(),
                values,
            }),
    }
}

/// The parameter-independent part of the document, which the profile
/// identity commits to: the schema and the fixed expectations.
pub fn static_part(
    gate_app_id: u64,
    exception_environment: &str,
    credentials: &[String],
) -> String {
    serde_json::json!({
        "schema": SCHEMA,
        "path": PATH,
        "requiredChecks": [{"name": AGGREGATE, "appId": gate_app_id}],
        "aggregate": AGGREGATE,
        "codeOwnerReview": {"required": true},
        "reviewException": {"environment": exception_environment, "preventSelfReview": true},
        "workflowToken": {"defaultPermissions": "read", "canApprovePullRequestReviews": false},
        "secrets": [{"anyOf": credentials, "visibility": SECRET_VISIBILITY}],
        "customProperties": {"schemaOwner": "organization"},
    })
    .to_string()
}

/// How one compared leaf stands (section 3.2). Exactly one per leaf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FieldState {
    /// The host answered and the observed value equals the declared one.
    Matching,
    /// The host answered and the observed value differs or is absent.
    Drifted,
    /// The host or resource could not be reached, not by a refusal.
    Unavailable,
    /// The host refused the credential's read authority.
    Unauthorized,
    /// The host, plan or API does not expose or support the capability.
    Unsupported,
    /// No comparison was possible without guessing.
    Unverified,
}

impl FieldState {
    /// A one-word rendering.
    pub fn word(self) -> &'static str {
        match self {
            FieldState::Matching => "matching",
            FieldState::Drifted => "drifted",
            FieldState::Unavailable => "unavailable",
            FieldState::Unauthorized => "unauthorized",
            FieldState::Unsupported => "unsupported",
            FieldState::Unverified => "unverified",
        }
    }
}

/// One leaf's result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldResult {
    /// The leaf, as `field.leaf`.
    pub field: String,
    /// How it stands.
    pub state: FieldState,
    /// Whether the document declares it; an omitted field is no claim.
    pub declared: bool,
    /// The declared value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<serde_json::Value>,
    /// The observed value, when the host answered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed: Option<serde_json::Value>,
    /// Why, sanitized: never a credential, a secret value or a response body.
    pub reason: String,
}

/// Remote state the host reported that the document does not declare.
/// Reported, never added to the desired state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Extra {
    /// Where it was observed.
    pub field: String,
    /// What was observed.
    pub observed: serde_json::Value,
}

/// The comparison `doctor --remote` reports beside the six results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    /// The document compared.
    pub document: String,
    /// Its schema, when it was read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// A local finding raised before any host was asked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finding: Option<String>,
    /// Why no comparison was made, when that is not a finding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// One result per leaf, in document order.
    pub fields: Vec<FieldResult>,
    /// Observed state the document does not declare.
    pub extras: Vec<Extra>,
    /// How many leaves stand in each state. It never replaces the leaves.
    pub summary: BTreeMap<String, usize>,
}

impl Comparison {
    fn without_fields(finding: Option<String>, note: Option<String>) -> Self {
        Comparison {
            document: PATH.to_string(),
            schema: None,
            finding,
            note,
            fields: Vec::new(),
            extras: Vec::new(),
            summary: BTreeMap::new(),
        }
    }

    /// Whether it is a finding: a local finding, or any declared leaf that
    /// is not `matching`.
    pub fn has_findings(&self) -> bool {
        self.finding.is_some()
            || self
                .fields
                .iter()
                .any(|f| f.declared && f.state != FieldState::Matching)
    }

    /// A human-readable rendering.
    pub fn render(&self) -> String {
        let mut out = String::new();
        if let Some(f) = &self.finding {
            out.push_str(&format!("remote     finding: {f}\n"));
        }
        if let Some(n) = &self.note {
            out.push_str(&format!("remote     {n}\n"));
        }
        for f in &self.fields {
            out.push_str(&format!(
                "remote     {:<44} {} ({})\n",
                f.field,
                f.state.word(),
                f.reason
            ));
        }
        for e in &self.extras {
            out.push_str(&format!(
                "remote     observed, undeclared: {} = {}\n",
                e.field, e.observed
            ));
        }
        out
    }
}

/// What the local side says before any host is asked: the document, or
/// why none is compared. `Err` is a local finding (section 3.6, a
/// desired-state identity that differs from the selected profile).
pub fn read_local(
    root: &std::path::Path,
    manifest: &statecraft_environment::manifest::Manifest,
) -> Result<Option<DesiredState>, String> {
    let Some(selection) = manifest.project.setup.as_ref() else {
        return Ok(None);
    };
    let bytes = match std::fs::read(root.join(PATH)) {
        Ok(b) => b,
        Err(_) if selection.remote_state.is_none() => return Ok(None),
        Err(_) => {
            return Err(format!(
                "{PATH} is absent, and the selection {}@{} rendered it",
                selection.profile, selection.revision
            ));
        }
    };
    let doc = DesiredState::parse(&bytes)?;
    let claim = &doc.profile;
    if claim.id != selection.profile
        || claim.revision != selection.revision
        || claim.identity != selection.identity
    {
        return Err(format!(
            "{PATH} declares profile {}@{} (identity {}), and the selected profile is {}@{} (identity {})",
            claim.id,
            claim.revision,
            short(&claim.identity),
            selection.profile,
            selection.revision,
            short(&selection.identity)
        ));
    }
    if let Some(record) = &selection.remote_state {
        let digest = statecraft_environment::digest::digest_bytes(&bytes);
        if record.digest != digest {
            return Err(format!(
                "{PATH} is not the document the selection rendered (recorded {}, on disk {})",
                short(&record.digest),
                short(&digest)
            ));
        }
    }
    Ok(Some(doc))
}

fn short(s: &str) -> &str {
    if s.len() > 12 { &s[..12] } else { s }
}

/// The comparison, from the local read and the host. The host is asked
/// nothing when the local side is a finding or has no document.
pub fn compare(
    local: Result<Option<DesiredState>, String>,
    slug: Option<&str>,
    host: &dyn Host,
) -> Comparison {
    let doc = match local {
        Err(finding) => return Comparison::without_fields(Some(finding), None),
        Ok(None) => {
            return Comparison::without_fields(
                None,
                Some(format!(
                    "no desired-state document: {PATH} is rendered from profile revision 16"
                )),
            );
        }
        Ok(Some(doc)) => doc,
    };
    let Some(slug) = slug else {
        let mut c = Comparison::without_fields(None, None);
        c.schema = Some(doc.schema.clone());
        c.fields = leaves(&doc)
            .into_iter()
            .map(|(field, declared, expected)| FieldResult {
                field,
                state: FieldState::Unverified,
                declared,
                expected,
                observed: None,
                reason: "the origin remote is not a GitHub repository this product can name".into(),
            })
            .collect();
        c.summary = summary(&c.fields);
        return c;
    };
    let reads = Reads {
        host,
        slug,
        cache: std::cell::RefCell::new(BTreeMap::new()),
    };
    let mut out = Vec::new();
    let mut extras = Vec::new();
    let repo = reads.get("");
    profile_leaf(&doc, &reads, &mut out);
    default_branch_leaf(&doc, &repo, &mut out);
    let rules = Rules::read(&reads, &doc.default_branch);
    required_checks_leaves(&doc, &rules, &mut out, &mut extras);
    extra_job_leaves(&doc, &reads, &mut out);
    code_owner_leaves(&doc, &reads, &rules, &mut out);
    merge_queue_leaves(&doc, &repo, &rules, &mut out);
    exception_leaves(&doc, &reads, &repo, &mut out, &mut extras);
    token_leaves(&doc, &reads, &repo, &mut out);
    secret_leaves(&doc, &reads, &repo, &mut out);
    property_leaves(&doc, &reads, &repo, &mut out, &mut extras);
    Comparison {
        document: PATH.to_string(),
        schema: Some(doc.schema.clone()),
        finding: None,
        note: None,
        summary: summary(&out),
        fields: out,
        extras,
    }
}

fn summary(fields: &[FieldResult]) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for f in fields {
        *m.entry(f.state.word().to_string()).or_insert(0) += 1;
    }
    m
}

/// Every leaf the document has, declared or not, with its declared value.
fn leaves(doc: &DesiredState) -> Vec<(String, bool, Option<serde_json::Value>)> {
    use serde_json::json;
    let mut v = vec![
        ("profile".to_string(), true, Some(json!(doc.profile))),
        (
            "defaultBranch".to_string(),
            true,
            Some(json!(doc.default_branch)),
        ),
    ];
    for c in &doc.required_checks {
        v.push((format!("requiredChecks.{}", c.name), true, Some(json!(c))));
    }
    for j in &doc.extra_required_jobs.jobs {
        v.push((format!("extraRequiredJobs.{}", j.job), true, Some(json!(j))));
    }
    v.push((
        "codeOwnerReview.required".into(),
        true,
        Some(json!(doc.code_owner_review.required)),
    ));
    v.push((
        "codeOwnerReview.paths".into(),
        true,
        Some(json!(doc.code_owner_review.paths)),
    ));
    match &doc.merge_queue {
        None => v.push(("mergeQueue".into(), false, None)),
        Some(q) => {
            v.push(("mergeQueue.required".into(), true, Some(json!(q.required))));
            if q.required {
                v.push((
                    "mergeQueue.mergeMethod".into(),
                    true,
                    Some(json!(q.merge_method)),
                ));
                v.push((
                    "mergeQueue.buildConcurrency".into(),
                    true,
                    Some(json!(q.build_concurrency)),
                ));
                v.push((
                    "mergeQueue.allEntriesMustPass".into(),
                    true,
                    Some(json!(q.all_entries_must_pass)),
                ));
            }
        }
    }
    let ex = &doc.review_exception;
    v.push((
        "reviewException.environment".into(),
        true,
        Some(json!(ex.environment)),
    ));
    match &ex.reviewers {
        None => v.push(("reviewException.reviewers".into(), false, None)),
        Some(list) => {
            for r in list {
                v.push((
                    format!("reviewException.reviewers.{r}"),
                    true,
                    Some(json!(r)),
                ));
            }
        }
    }
    v.push((
        "reviewException.preventSelfReview".into(),
        true,
        Some(json!(ex.prevent_self_review)),
    ));
    v.push((
        "workflowToken.defaultPermissions".into(),
        true,
        Some(json!(doc.workflow_token.default_permissions)),
    ));
    v.push((
        "workflowToken.canApprovePullRequestReviews".into(),
        true,
        Some(json!(doc.workflow_token.can_approve_pull_request_reviews)),
    ));
    for s in &doc.secrets {
        v.push((secret_field(s), true, Some(json!(s))));
    }
    match &doc.custom_properties {
        None => v.push(("customProperties".into(), false, None)),
        Some(p) => {
            for (k, val) in &p.values {
                v.push((format!("customProperties.{k}"), true, Some(json!(val))));
            }
        }
    }
    v
}

fn secret_field(s: &SecretRequirement) -> String {
    format!("secrets.{}", s.any_of.join("|"))
}

/// Memoized read-only host reads, scoped to one repository.
struct Reads<'a> {
    host: &'a dyn Host,
    slug: &'a str,
    cache: std::cell::RefCell<BTreeMap<String, Result<serde_json::Value, HostError>>>,
}

impl Reads<'_> {
    /// `repos/<slug><suffix>`.
    fn get(&self, suffix: &str) -> Result<serde_json::Value, HostError> {
        self.raw(&format!("repos/{}{suffix}", self.slug))
    }

    fn raw(&self, path: &str) -> Result<serde_json::Value, HostError> {
        if let Some(hit) = self.cache.borrow().get(path) {
            return hit.clone();
        }
        let answer = self.host.get(path);
        self.cache
            .borrow_mut()
            .insert(path.to_string(), answer.clone());
        answer
    }

    /// A file's text at a ref, through the contents API.
    fn file(&self, path: &str, at: &str) -> Result<String, HostError> {
        let v = self.get(&format!("/contents/{path}?ref={at}"))?;
        let encoded = v["content"]
            .as_str()
            .ok_or_else(|| HostError::Unreachable("the contents answer has no content".into()))?;
        let bytes = base64_decode(encoded)
            .ok_or_else(|| HostError::Unreachable("the contents answer is not base64".into()))?;
        String::from_utf8(bytes).map_err(|_| HostError::Unreachable("the file is not UTF-8".into()))
    }
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\n' | b'\r' => continue,
            _ => return None,
        };
        buf = (buf << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// A host error as a leaf's state and sanitized reason. `NotFound` is the
/// caller's to interpret; here it means the resource is not visible.
fn failed(what: &str, e: &HostError) -> (FieldState, String) {
    match e {
        HostError::NotFound => (FieldState::Drifted, format!("{what}: not found")),
        HostError::Unreachable(why) => (
            FieldState::Unavailable,
            format!("{what}: the host did not answer ({why})"),
        ),
        HostError::Unauthorized(why) => (
            FieldState::Unauthorized,
            format!("{what}: the credential may not read it ({why})"),
        ),
        HostError::Unsupported(why) => (
            FieldState::Unsupported,
            format!("{what}: the host or plan does not support it ({why})"),
        ),
    }
}

fn result(
    field: impl Into<String>,
    state: FieldState,
    expected: Option<serde_json::Value>,
    observed: Option<serde_json::Value>,
    reason: impl Into<String>,
) -> FieldResult {
    FieldResult {
        field: field.into(),
        state,
        declared: true,
        expected,
        observed,
        reason: reason.into(),
    }
}

fn from_error(
    field: impl Into<String>,
    expected: Option<serde_json::Value>,
    what: &str,
    e: &HostError,
) -> FieldResult {
    let (state, reason) = failed(what, e);
    result(field, state, expected, None, reason)
}

fn equal(
    field: &str,
    expected: serde_json::Value,
    observed: serde_json::Value,
    what: &str,
) -> FieldResult {
    if expected == observed {
        result(
            field,
            FieldState::Matching,
            Some(expected),
            Some(observed),
            format!("{what} as declared"),
        )
    } else {
        let reason = format!("{what} is {observed}, declared {expected}");
        result(
            field,
            FieldState::Drifted,
            Some(expected),
            Some(observed),
            reason,
        )
    }
}

fn owner_is_user(repo: &Result<serde_json::Value, HostError>) -> Option<bool> {
    repo.as_ref().ok().map(|r| r["owner"]["type"] == "User")
}

fn owner_login(repo: &Result<serde_json::Value, HostError>) -> Option<String> {
    repo.as_ref()
        .ok()
        .and_then(|r| r["owner"]["login"].as_str().map(str::to_string))
}

fn profile_leaf(doc: &DesiredState, reads: &Reads<'_>, out: &mut Vec<FieldResult>) {
    use serde_json::json;
    let expected = json!(doc.profile);
    let policy = reads.file(crate::setup::POLICY_PATH, &doc.default_branch);
    out.push(match policy {
        Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(v) => equal(
                "profile",
                expected,
                json!({"id": v["id"], "revision": v["revision"], "identity": v["identity"]}),
                &format!("the policy on {}", doc.default_branch),
            ),
            Err(_) => result(
                "profile",
                FieldState::Drifted,
                Some(expected),
                None,
                format!("the policy on {} is not JSON", doc.default_branch),
            ),
        },
        Err(HostError::NotFound) => result(
            "profile",
            FieldState::Drifted,
            Some(expected),
            None,
            format!(
                "{} is absent on {}",
                crate::setup::POLICY_PATH,
                doc.default_branch
            ),
        ),
        Err(e) => from_error("profile", Some(expected), "the default branch's policy", &e),
    });
}

fn default_branch_leaf(
    doc: &DesiredState,
    repo: &Result<serde_json::Value, HostError>,
    out: &mut Vec<FieldResult>,
) {
    let expected = serde_json::json!(doc.default_branch);
    out.push(match repo {
        Ok(r) => equal(
            "defaultBranch",
            expected,
            r["default_branch"].clone(),
            "the repository's default branch",
        ),
        Err(e) => from_error("defaultBranch", Some(expected), "the repository", e),
    });
}

/// The default branch's rules, from classic branch protection and from the
/// rulesets that apply to it. Either may be absent.
struct Rules {
    protection: Result<Option<serde_json::Value>, HostError>,
    rulesets: Result<Vec<serde_json::Value>, HostError>,
}

impl Rules {
    fn read(reads: &Reads<'_>, branch: &str) -> Rules {
        let protection = match reads.get(&format!("/branches/{branch}/protection")) {
            Ok(v) => Ok(Some(v)),
            Err(HostError::NotFound) => Ok(None),
            Err(e) => Err(e),
        };
        let rulesets = match reads.get(&format!("/rules/branches/{branch}")) {
            Ok(v) => Ok(v.as_array().cloned().unwrap_or_default()),
            Err(HostError::NotFound) => Ok(Vec::new()),
            Err(e) => Err(e),
        };
        Rules {
            protection,
            rulesets,
        }
    }

    /// The first error, when either source failed.
    fn any_failed(&self) -> Option<&HostError> {
        self.protection
            .as_ref()
            .err()
            .or(self.rulesets.as_ref().err())
    }

    fn rules_of(&self, kind: &str) -> Vec<&serde_json::Value> {
        self.rulesets
            .as_ref()
            .map(|r| r.iter().filter(|x| x["type"] == kind).collect())
            .unwrap_or_default()
    }

    /// Every required check, as `(context, app id)`.
    fn checks(&self) -> Vec<(String, Option<u64>)> {
        let mut out = Vec::new();
        if let Ok(Some(p)) = &self.protection {
            let rsc = &p["required_status_checks"];
            for c in rsc["checks"].as_array().into_iter().flatten() {
                if let Some(ctx) = c["context"].as_str() {
                    out.push((ctx.to_string(), c["app_id"].as_u64()));
                }
            }
            for c in rsc["contexts"].as_array().into_iter().flatten() {
                if let Some(ctx) = c.as_str()
                    && !out.iter().any(|(n, _)| n == ctx)
                {
                    out.push((ctx.to_string(), None));
                }
            }
        }
        for rule in self.rules_of("required_status_checks") {
            for c in rule["parameters"]["required_status_checks"]
                .as_array()
                .into_iter()
                .flatten()
            {
                if let Some(ctx) = c["context"].as_str() {
                    out.push((ctx.to_string(), c["integration_id"].as_u64()));
                }
            }
        }
        out
    }
}

fn required_checks_leaves(
    doc: &DesiredState,
    rules: &Rules,
    out: &mut Vec<FieldResult>,
    extras: &mut Vec<Extra>,
) {
    use serde_json::json;
    let observed = rules.checks();
    for c in &doc.required_checks {
        let field = format!("requiredChecks.{}", c.name);
        let expected = json!(c);
        let same: Vec<&(String, Option<u64>)> =
            observed.iter().filter(|(n, _)| *n == c.name).collect();
        let seen = json!(
            same.iter()
                .map(|(n, a)| json!({"name": n, "appId": a}))
                .collect::<Vec<_>>()
        );
        if same.iter().any(|(_, a)| *a == Some(c.app_id)) {
            out.push(result(
                field,
                FieldState::Matching,
                Some(expected),
                Some(seen),
                format!("{} is required from app id {}", c.name, c.app_id),
            ));
        } else if let Some(e) = rules.any_failed() {
            // One source could not say, and the other did not match.
            out.push(from_error(
                field,
                Some(expected),
                "the default branch's rules",
                e,
            ));
        } else if same.is_empty() {
            out.push(result(
                field,
                FieldState::Drifted,
                Some(expected),
                None,
                format!("{} is not a required check", c.name),
            ));
        } else {
            // Section 3.3 rule 1: the right name from an unbound source.
            out.push(result(
                field,
                FieldState::Drifted,
                Some(expected),
                Some(seen),
                format!(
                    "{} is required, but not bound to app id {}",
                    c.name, c.app_id
                ),
            ));
        }
    }
    let mut undeclared: BTreeSet<String> = BTreeSet::new();
    for (n, a) in observed {
        if !doc.required_checks.iter().any(|c| c.name == n) {
            undeclared.insert(json!({"name": n, "appId": a}).to_string());
        }
    }
    for x in undeclared {
        extras.push(Extra {
            field: "requiredChecks".into(),
            observed: serde_json::from_str(&x).expect("own json"),
        });
    }
}

fn extra_job_leaves(doc: &DesiredState, reads: &Reads<'_>, out: &mut Vec<FieldResult>) {
    use serde_json::json;
    if doc.extra_required_jobs.jobs.is_empty() {
        return;
    }
    let branch = &doc.default_branch;
    let policy = reads
        .file(crate::setup::POLICY_PATH, branch)
        .map(|t| serde_json::from_str::<serde_json::Value>(&t).unwrap_or_default());
    let workflow = reads
        .file(".github/workflows/statecraft-ci.yml", branch)
        .map(|t| serde_yaml::from_str::<serde_yaml::Value>(&t).unwrap_or_default());
    let aggregate = &doc.extra_required_jobs.aggregate;
    for j in &doc.extra_required_jobs.jobs {
        let field = format!("extraRequiredJobs.{}", j.job);
        let expected = json!(j);
        let (policy, workflow) = match (&policy, &workflow) {
            (Err(e), _) | (_, Err(e)) if !matches!(e, HostError::NotFound) => {
                out.push(from_error(
                    field,
                    Some(expected),
                    "the default branch's CI files",
                    e,
                ));
                continue;
            }
            (p, w) => (p.as_ref().ok(), w.as_ref().ok()),
        };
        let declared = policy.is_some_and(|p| {
            p["jobs"][j.job.as_str()]["required"] == json!(true)
                && p["jobs"][j.job.as_str()]["workflow"] == json!(j.workflow)
        });
        let needs = workflow.is_some_and(|w| {
            let n = &w["jobs"][aggregate.as_str()]["needs"];
            match n {
                serde_yaml::Value::String(s) => *s == j.job,
                serde_yaml::Value::Sequence(list) => {
                    list.iter().any(|x| x.as_str() == Some(j.job.as_str()))
                }
                _ => false,
            }
        });
        let observed = json!({"declaredRequired": declared, "aggregatedBy": if needs { Some(aggregate) } else { None }});
        // Section 3.3 rule 2: the declaration and the aggregation, both.
        out.push(if declared && needs {
            result(
                field,
                FieldState::Matching,
                Some(expected),
                Some(observed),
                format!("required by the policy and needed by {aggregate} on {branch}"),
            )
        } else {
            result(
                field,
                FieldState::Drifted,
                Some(expected),
                Some(observed),
                format!(
                    "on {branch}: required by the policy: {declared}; needed by {aggregate}: {needs}"
                ),
            )
        });
    }
}

/// The owners `CODEOWNERS` gives `path`: `Some(owners)` when a rule decides
/// it, `None` when a pattern this reader cannot evaluate might.
fn codeowners_for(text: &str, path: &str) -> Option<Vec<String>> {
    let target = path.trim_start_matches('/');
    for line in text.lines().rev() {
        let line = line.split('#').next().unwrap_or("").trim();
        let mut parts = line.split_whitespace();
        let Some(pattern) = parts.next() else {
            continue;
        };
        let owners: Vec<String> = parts.map(str::to_string).collect();
        let anchored = pattern.trim_start_matches('/');
        let simple = !anchored.contains(['*', '?', '[', '!', '\\']);
        let matches = if pattern == "*" || pattern == "/**" || pattern == "**" {
            true
        } else if simple {
            let dir = anchored.trim_end_matches('/');
            target == anchored
                || (target.starts_with(dir) && target[dir.len()..].starts_with('/'))
                    && (pattern.starts_with('/') || !dir.contains('/'))
        } else if let Some(dir) = anchored.strip_suffix("/**").or(anchored.strip_suffix("/*"))
            && !dir.contains(['*', '?', '[', '!', '\\'])
        {
            let rest = target.strip_prefix(dir).and_then(|r| r.strip_prefix('/'));
            match rest {
                Some(r) => anchored.ends_with("/**") || !r.contains('/'),
                None => false,
            }
        } else {
            return None;
        };
        if matches {
            return Some(owners);
        }
    }
    Some(Vec::new())
}

fn code_owner_leaves(
    doc: &DesiredState,
    reads: &Reads<'_>,
    rules: &Rules,
    out: &mut Vec<FieldResult>,
) {
    use serde_json::json;
    let cor = &doc.code_owner_review;
    // Section 3.3 rule 3: the protection setting and the CODEOWNERS
    // declaration, independently.
    let from_protection = matches!(&rules.protection, Ok(Some(p))
        if p["required_pull_request_reviews"]["require_code_owner_reviews"] == json!(true));
    let from_rules = rules
        .rules_of("pull_request")
        .iter()
        .any(|r| r["parameters"]["require_code_owner_review"] == json!(true));
    let observed = from_protection || from_rules;
    out.push(if observed == cor.required {
        result(
            "codeOwnerReview.required",
            FieldState::Matching,
            Some(json!(cor.required)),
            Some(json!(observed)),
            format!("code-owner review required: {observed}"),
        )
    } else if let Some(e) = if observed { None } else { rules.any_failed() } {
        from_error(
            "codeOwnerReview.required",
            Some(json!(cor.required)),
            "the default branch's rules",
            e,
        )
    } else {
        result(
            "codeOwnerReview.required",
            FieldState::Drifted,
            Some(json!(cor.required)),
            Some(json!(observed)),
            format!(
                "code-owner review required: {observed}, declared {}",
                cor.required
            ),
        )
    });

    let expected = json!(cor.paths);
    let mut file = None;
    let mut error = None;
    for loc in crate::setup::CODEOWNERS_LOCATIONS {
        match reads.file(loc, &doc.default_branch) {
            Ok(text) => {
                file = Some((loc, text));
                break;
            }
            Err(HostError::NotFound) => {}
            Err(e) => {
                error = Some(e);
                break;
            }
        }
    }
    out.push(match (file, error) {
        (_, Some(e)) => from_error("codeOwnerReview.paths", Some(expected), "CODEOWNERS", &e),
        (None, None) => result(
            "codeOwnerReview.paths",
            FieldState::Drifted,
            Some(expected),
            None,
            format!("no CODEOWNERS on {}", doc.default_branch),
        ),
        (Some((loc, text)), None) => {
            let mut uncovered = Vec::new();
            let mut unknown = Vec::new();
            for p in &cor.paths {
                match codeowners_for(&text, p) {
                    None => unknown.push(p.clone()),
                    Some(owners) => {
                        let ok = match &cor.owners {
                            None => !owners.is_empty(),
                            Some(want) => want
                                .iter()
                                .all(|w| owners.iter().any(|o| o.eq_ignore_ascii_case(w))),
                        };
                        if !ok {
                            uncovered.push(p.clone());
                        }
                    }
                }
            }
            if !uncovered.is_empty() {
                result(
                    "codeOwnerReview.paths",
                    FieldState::Drifted,
                    Some(expected),
                    Some(json!(uncovered)),
                    format!(
                        "{loc} does not give the declared owners: {}",
                        uncovered.join(", ")
                    ),
                )
            } else if !unknown.is_empty() {
                result(
                    "codeOwnerReview.paths",
                    FieldState::Unverified,
                    Some(expected),
                    None,
                    format!(
                        "{loc} has patterns this product does not evaluate that may decide: {}",
                        unknown.join(", ")
                    ),
                )
            } else {
                result(
                    "codeOwnerReview.paths",
                    FieldState::Matching,
                    Some(expected.clone()),
                    Some(expected),
                    format!("{loc} gives every governed path its owners"),
                )
            }
        }
    });
}

fn merge_queue_leaves(
    doc: &DesiredState,
    repo: &Result<serde_json::Value, HostError>,
    rules: &Rules,
    out: &mut Vec<FieldResult>,
) {
    use serde_json::json;
    let Some(q) = &doc.merge_queue else {
        out.push(FieldResult {
            field: "mergeQueue".into(),
            state: FieldState::Unverified,
            declared: false,
            expected: None,
            observed: None,
            reason: "the profile makes no claim: remote.merge_queue is not declared".into(),
        });
        return;
    };
    let mut expected: Vec<(&str, serde_json::Value)> =
        vec![("mergeQueue.required", json!(q.required))];
    if q.required {
        expected.push(("mergeQueue.mergeMethod", json!(q.merge_method)));
        expected.push(("mergeQueue.buildConcurrency", json!(q.build_concurrency)));
        expected.push((
            "mergeQueue.allEntriesMustPass",
            json!(q.all_entries_must_pass),
        ));
    }
    let unsupported_reason = match (owner_is_user(repo), &rules.rulesets) {
        (Some(true), _) => Some(
            "merge queues are available only to organization repositories (public, or private on GitHub Enterprise Cloud)".to_string(),
        ),
        (_, Err(HostError::Unsupported(why))) => Some(format!("rulesets: {why}")),
        _ => None,
    };
    if let Some(why) = unsupported_reason {
        // Section 3.3 rule 4: neither matching nor drifted.
        for (field, e) in expected {
            out.push(result(
                field,
                FieldState::Unsupported,
                Some(e),
                None,
                why.clone(),
            ));
        }
        return;
    }
    if let Err(e) = &rules.rulesets {
        for (field, x) in expected {
            out.push(from_error(
                field,
                Some(x),
                "the default branch's rulesets",
                e,
            ));
        }
        return;
    }
    let rule = rules.rules_of("merge_queue").into_iter().next();
    out.push(equal(
        "mergeQueue.required",
        json!(q.required),
        json!(rule.is_some()),
        "a merge queue required",
    ));
    if !q.required {
        return;
    }
    let params = rule.map(|r| r["parameters"].clone());
    let observed = |f: &dyn Fn(&serde_json::Value) -> serde_json::Value| params.as_ref().map(f);
    for (field, x) in expected.into_iter().skip(1) {
        let seen = match field {
            "mergeQueue.mergeMethod" => observed(&|p| {
                p["merge_method"]
                    .as_str()
                    .map_or(serde_json::Value::Null, |m| json!(m.to_ascii_lowercase()))
            }),
            "mergeQueue.buildConcurrency" => observed(&|p| p["max_entries_to_build"].clone()),
            _ => observed(&|p| json!(p["grouping_strategy"] == "ALLGREEN")),
        };
        out.push(match seen {
            Some(v) => equal(field, x, v, field.trim_start_matches("mergeQueue.")),
            None => result(
                field,
                FieldState::Drifted,
                Some(x),
                None,
                "no merge queue is configured",
            ),
        });
    }
}

fn exception_leaves(
    doc: &DesiredState,
    reads: &Reads<'_>,
    repo: &Result<serde_json::Value, HostError>,
    out: &mut Vec<FieldResult>,
    extras: &mut Vec<Extra>,
) {
    use serde_json::json;
    let ex = &doc.review_exception;
    let env = reads.get(&format!("/environments/{}", ex.environment));
    let mut reviewer_fields: Vec<(String, serde_json::Value)> = Vec::new();
    match &ex.reviewers {
        Some(list) => {
            for r in list {
                reviewer_fields.push((format!("reviewException.reviewers.{r}"), json!(r)));
            }
        }
        None => out.push(FieldResult {
            field: "reviewException.reviewers".into(),
            state: FieldState::Unverified,
            declared: false,
            expected: None,
            observed: None,
            reason: "the profile makes no claim: remote.exception_reviewers is not declared".into(),
        }),
    }
    let self_review = (
        "reviewException.preventSelfReview".to_string(),
        json!(ex.prevent_self_review),
    );
    let env = match env {
        Ok(v) => {
            out.push(result(
                "reviewException.environment",
                FieldState::Matching,
                Some(json!(ex.environment)),
                Some(json!(ex.environment)),
                format!("the Environment {} exists", ex.environment),
            ));
            v
        }
        Err(e) => {
            let (state, reason) = match &e {
                HostError::NotFound => (
                    FieldState::Drifted,
                    format!("the Environment {} does not exist", ex.environment),
                ),
                other => failed("the exception Environment", other),
            };
            out.push(result(
                "reviewException.environment",
                state,
                Some(json!(ex.environment)),
                None,
                reason.clone(),
            ));
            for (field, x) in reviewer_fields.into_iter().chain([self_review]) {
                out.push(result(field, state, Some(x), None, reason.clone()));
            }
            return;
        }
    };
    let rule = env["protection_rules"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|r| r["type"] == "required_reviewers")
        .cloned();
    let Some(rule) = rule else {
        let private = repo.as_ref().ok().map(|r| r["private"] == json!(true));
        let (state, reason) = match (private, owner_is_user(repo)) {
            (Some(true), Some(true)) => (
                FieldState::Unsupported,
                "Environment required reviewers on GitHub Free, Pro or Team are available only for public repositories".to_string(),
            ),
            (Some(true), _) => (
                FieldState::Unverified,
                "no required reviewers, and the organization's plan, which decides whether a private repository may have them, is not visible".to_string(),
            ),
            (Some(false), _) => (
                FieldState::Drifted,
                format!("the Environment {} has no required reviewers", ex.environment),
            ),
            (None, _) => match repo {
                Err(e) => failed("the repository", e),
                Ok(_) => (FieldState::Unverified, "the repository's visibility is unknown".into()),
            },
        };
        for (field, x) in reviewer_fields.into_iter().chain([self_review]) {
            out.push(result(field, state, Some(x), None, reason.clone()));
        }
        return;
    };
    let configured: Vec<String> = rule["reviewers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| match r["type"].as_str() {
            Some("User") => r["reviewer"]["login"].as_str().map(|l| format!("@{l}")),
            Some("Team") => {
                let org = owner_login(repo).unwrap_or_default();
                r["reviewer"]["slug"]
                    .as_str()
                    .map(|s| format!("@{org}/{s}"))
            }
            _ => None,
        })
        .collect();
    // Section 3.3 rule 5: each declared reviewer separately.
    for (field, x) in reviewer_fields {
        let want = x.as_str().unwrap_or_default();
        let found = configured.iter().any(|c| c.eq_ignore_ascii_case(want));
        out.push(result(
            field,
            if found {
                FieldState::Matching
            } else {
                FieldState::Drifted
            },
            Some(x.clone()),
            Some(json!(configured)),
            if found {
                format!("{want} is a required reviewer")
            } else {
                format!("{want} is not a required reviewer")
            },
        ));
    }
    let declared: Vec<&String> = ex.reviewers.iter().flatten().collect();
    for c in &configured {
        if !declared.iter().any(|d| d.eq_ignore_ascii_case(c)) {
            extras.push(Extra {
                field: "reviewException.reviewers".into(),
                observed: json!(c),
            });
        }
    }
    out.push(equal(
        &self_review.0,
        self_review.1,
        json!(rule["prevent_self_review"] == json!(true)),
        "self-review prevention",
    ));
}

fn token_leaves(
    doc: &DesiredState,
    reads: &Reads<'_>,
    repo: &Result<serde_json::Value, HostError>,
    out: &mut Vec<FieldResult>,
) {
    use serde_json::json;
    let wt = &doc.workflow_token;
    let fields = [
        (
            "workflowToken.defaultPermissions",
            json!(wt.default_permissions),
            "default_workflow_permissions",
        ),
        (
            "workflowToken.canApprovePullRequestReviews",
            json!(wt.can_approve_pull_request_reviews),
            "can_approve_pull_request_reviews",
        ),
    ];
    let observed = reads.get("/actions/permissions/workflow");
    // Section 3.3 rule 6: a stricter organization policy, where visible.
    let org = match (owner_is_user(repo), owner_login(repo)) {
        (Some(false), Some(login)) => {
            Some(reads.raw(&format!("orgs/{login}/actions/permissions/workflow")))
        }
        _ => None,
    };
    for (field, expected, key) in fields {
        out.push(match &observed {
            Ok(v) => {
                let mut r = equal(field, expected, v[key].clone(), "the repository's value");
                match &org {
                    Some(Ok(o)) if o[key] == v[key] => {
                        r.reason.push_str(&format!(
                            "; the organization policy is the same ({}), so it may be inherited",
                            o[key]
                        ));
                    }
                    Some(Ok(o)) => r
                        .reason
                        .push_str(&format!("; the organization policy is {}", o[key])),
                    Some(Err(e)) => r.reason.push_str(&format!(
                        "; the organization policy was not read ({})",
                        failed("organization", e).1
                    )),
                    None => {}
                }
                r
            }
            Err(e) => from_error(field, Some(expected), "the workflow token policy", e),
        });
    }
}

fn secret_leaves(
    doc: &DesiredState,
    reads: &Reads<'_>,
    repo: &Result<serde_json::Value, HostError>,
    out: &mut Vec<FieldResult>,
) {
    use serde_json::json;
    for s in &doc.secrets {
        let field = secret_field(s);
        let expected = json!(s);
        let mut seen: Vec<serde_json::Value> = Vec::new();
        let mut errors: Vec<HostError> = Vec::new();
        // Section 3.3 rule 7: the name and its visible scope, never a value.
        if s.visibility.iter().any(|v| v == "repository") {
            for name in &s.any_of {
                match reads.get(&format!("/actions/secrets/{name}")) {
                    Ok(_) => seen.push(json!({"name": name, "visibility": "repository"})),
                    Err(HostError::NotFound) => {}
                    Err(e) => errors.push(e),
                }
            }
        }
        if s.visibility.iter().any(|v| v == "organization") && owner_is_user(repo) != Some(true) {
            match reads.get("/actions/organization-secrets?per_page=100") {
                Ok(v) => {
                    for x in v["secrets"].as_array().into_iter().flatten() {
                        if let Some(n) = x["name"].as_str()
                            && s.any_of.iter().any(|a| a == n)
                        {
                            seen.push(json!({"name": n, "visibility": "organization"}));
                        }
                    }
                }
                Err(HostError::NotFound) => {}
                Err(e) => errors.push(e),
            }
        }
        out.push(if !seen.is_empty() {
            result(
                field,
                FieldState::Matching,
                Some(expected),
                Some(json!(seen)),
                "a declared name is visible in an allowed scope",
            )
        } else if let Some(e) = errors.first() {
            from_error(field, Some(expected), "the secret names", e)
        } else {
            result(
                field,
                FieldState::Drifted,
                Some(expected),
                None,
                format!(
                    "none of {} is visible to the repository",
                    s.any_of.join(", ")
                ),
            )
        });
    }
}

fn property_leaves(
    doc: &DesiredState,
    reads: &Reads<'_>,
    repo: &Result<serde_json::Value, HostError>,
    out: &mut Vec<FieldResult>,
    extras: &mut Vec<Extra>,
) {
    use serde_json::json;
    let Some(props) = &doc.custom_properties else {
        out.push(FieldResult {
            field: "customProperties".into(),
            state: FieldState::Unverified,
            declared: false,
            expected: None,
            observed: None,
            reason: "the profile makes no claim: remote.custom_properties is not declared".into(),
        });
        // Still read, so a value the host holds is reported as extra state.
        if owner_is_user(repo) == Some(false)
            && let Ok(v) = reads.get("/properties/values")
        {
            for x in v.as_array().into_iter().flatten() {
                if let Some(k) = x["property_name"].as_str()
                    && !x["value"].is_null()
                {
                    extras.push(Extra {
                        field: format!("customProperties.{k}"),
                        observed: x["value"].clone(),
                    });
                }
            }
        }
        return;
    };
    if owner_is_user(repo) == Some(true) {
        for (k, v) in &props.values {
            out.push(result(
                format!("customProperties.{k}"),
                FieldState::Unsupported,
                Some(json!(v)),
                None,
                "custom properties exist only for organization repositories",
            ));
        }
        return;
    }
    let values = reads.get("/properties/values");
    let schema = owner_login(repo).map(|o| reads.raw(&format!("orgs/{o}/properties/schema")));
    let observed: BTreeMap<String, serde_json::Value> = values
        .as_ref()
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|x| Some((x["property_name"].as_str()?.to_string(), x["value"].clone())))
        .collect();
    for (k, v) in &props.values {
        let field = format!("customProperties.{k}");
        let expected = json!(v);
        if let Err(e) = &values {
            out.push(from_error(
                field,
                Some(expected),
                "the repository's custom properties",
                e,
            ));
            continue;
        }
        let seen = observed.get(k).cloned().filter(|x| !x.is_null());
        if seen.as_ref() == Some(&expected) {
            out.push(result(
                field,
                FieldState::Matching,
                Some(expected.clone()),
                Some(expected),
                "the repository value as declared",
            ));
            continue;
        }
        // Section 3.3 rule 8: a missing schema is distinct from a missing
        // repository value.
        let in_schema = match &schema {
            Some(Ok(s)) => Some(
                s.as_array()
                    .into_iter()
                    .flatten()
                    .any(|p| p["property_name"] == json!(k)),
            ),
            _ => None,
        };
        let reason = match (in_schema, &seen) {
            (Some(false), _) => format!("the organization schema defines no property {k}"),
            (_, None) => format!("the repository has no value for {k}"),
            (_, Some(x)) => format!("the repository value of {k} is {x}, declared {expected}"),
        };
        out.push(result(
            field,
            FieldState::Drifted,
            Some(expected),
            seen,
            reason,
        ));
    }
    for (k, v) in observed {
        if !v.is_null() && !props.values.contains_key(&k) {
            extras.push(Extra {
                field: format!("customProperties.{k}"),
                observed: v,
            });
        }
    }
}
