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
//! Nothing here writes remote state or reads a secret value.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
