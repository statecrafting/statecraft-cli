//! Repository setup profiles.
//!
//! Spec 002 section 5, 2026-09-24, the setup-profile entry (adopted with owner
//! decisions S-0 to S-5). A profile is `(id, revision)`, content-addressed: its
//! identity is a digest over its template files and its static policy, so a
//! changed byte is a different revision. One profile is registered,
//! `github-actions-rust`, and it is a closed value of this module: there is no
//! plugin loading and no template language beyond named parameter
//! substitution.
//!
//! A profile renders **committed repository files**. It composes with the
//! governance producer's output and never widens it; the producer still
//! returns governance starter content only.
//!
//! What this module decides, and the flow performs:
//!
//! - the **plan**: each file's intended bytes and digest, its role, whether it
//!   is in the authority set, the three-digest reconciliation against the
//!   manifest and the disk, prerequisites with their observed state, the
//!   command selections, the remote obligations, and a **plan identity**;
//! - the **apply**: a resume record written before the first write, each file
//!   written through the caller's observed writer, `.intended` copies for a
//!   customized file, and the manifest entries, which the flow writes last;
//! - the **six results**, reported separately, and the read-only remote
//!   verification `doctor --remote` asks for.

use serde::{Deserialize, Serialize};
use statecraft_environment::digest::digest_bytes;
use statecraft_environment::manifest::{
    Class, Entry, Manifest, SetupSelection, Source, SourceKind,
};
use std::collections::BTreeMap;
use std::path::Path;

/// The one registered profile.
pub const PROFILE_ID: &str = "github-actions-rust";
/// Its revision.
pub const REVISION: u32 = 9;
/// Where the rendered policy document lives in the target.
pub const POLICY_PATH: &str = ".statecraft/setup/github-actions-rust.json";
/// The resume record, under the project's runtime state.
pub const RESUME_PATH: &str = ".statecraft/state/setup/apply.json";
/// Where a customized file's intended bytes are left for a manual merge.
pub const INTENDED_DIR: &str = ".statecraft/state/setup";
/// The credential the reviewer falls back to, by name. Its value is never read
/// here.
pub const CREDENTIAL: &str = "CLAUDE_CODE_OAUTH_TOKEN";
/// The credential the reviewer prefers when it is set (revision 8), by name.
pub const PREFERRED_CREDENTIAL: &str = "ANTHROPIC_API_KEY";
/// The command an operator runs to set it.
pub const CREDENTIAL_COMMAND: &str = "gh secret set CLAUDE_CODE_OAUTH_TOKEN";
/// The protected Environment an owner exception is approved on (decision S-1;
/// from revision 2 also for a pull request whose review returned `findings`).
pub const EXCEPTION_ENVIRONMENT: &str = "statecraft-review-exception";
/// The GitHub App that must report `ci-gate` (GitHub Actions), so a status any
/// token posts does not satisfy the required check (revision 2, item 3).
pub const GATE_APP_ID: u64 = 15368;
/// The reviewer CLI, pinned by exact version.
pub const REVIEW_TOOL: &str = "claude-code";
/// Its version.
pub const REVIEW_TOOL_VERSION: &str = "2.1.116";
/// The source identity prefix a profile's manifest entries carry.
pub const SOURCE_PREFIX: &str = "statecraft-setup:";
/// The ignore fragment the profile adds to the governance one.
pub const IGNORE_FRAGMENT: &str =
    "# The repository-local spec-spine the setup profile installs.\n.tooling/\n";
/// The visible skip classes, non-blocking for ordinary pull requests (S-2).
pub const SKIP_CLASSES: [&str; 5] = ["draft", "fork", "dependabot", "oversized", "transient"];
/// The places a CODEOWNERS file is read from; one existing anywhere is the
/// user's and none is rendered.
pub const CODEOWNERS_LOCATIONS: [&str; 3] = [".github/CODEOWNERS", "CODEOWNERS", "docs/CODEOWNERS"];

/// The authored-content script a revision-3 project ran when it was
/// executable; the upgrade to revision 4 declares it when it exists.
pub const AUTHORED_CONTENT_SCRIPT: &str = "scripts/check-authored-content.sh";
/// The revision that made the authored-content script a declared parameter.
const AUTHORED_CONTENT_DECLARED_SINCE: u32 = 4;

const DEFAULT_DIFF_CAP: u64 = 2000;
const MAX_DIFF_CAP: u64 = 20000;
const DEFAULT_RELEASE_PATTERN: &str = "release/*";
/// Where a declared extra required job's reusable workflow lives (revision
/// 7): GitHub calls a reusable workflow only from this directory, never from
/// a subdirectory of it.
const WORKFLOW_DIR: &str = ".github/workflows/";
/// The job ids the profile renders, which a declared extra job may not reuse.
const PROFILE_JOB_IDS: [&str; 5] = [
    "governance",
    "code",
    "ai-review",
    "review-exception",
    "ci-gate",
];

/// A rendered file's role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// A GitHub Actions workflow.
    Workflow,
    /// A script CI and the local gate run.
    Script,
    /// The policy document the aggregate gate reads at the base.
    Policy,
    /// A `Makefile`, rendered only when none exists.
    Makefile,
    /// A `CODEOWNERS`, rendered only when none exists (S-3).
    Codeowners,
}

/// One template of a profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    /// The repository-relative path it renders to.
    pub path: String,
    /// Its role.
    pub role: Role,
    /// The template text, with `{{name}}` parameters.
    pub body: String,
    /// Rendered only when no file of its kind exists.
    pub only_when_absent: bool,
}

/// A profile: a closed value, identified by its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// The id.
    pub id: String,
    /// The revision.
    pub revision: u32,
    /// The templates, in render order.
    pub templates: Vec<Template>,
}

macro_rules! template {
    ($path:expr, $role:expr, $file:expr, $absent:expr) => {
        Template {
            path: $path.to_string(),
            role: $role,
            body: include_str!(concat!("../profiles/github-actions-rust/", $file)).to_string(),
            only_when_absent: $absent,
        }
    };
}

impl Profile {
    /// The one registered profile.
    pub fn registered() -> Profile {
        Profile {
            id: PROFILE_ID.to_string(),
            revision: REVISION,
            templates: vec![
                template!(
                    ".github/workflows/statecraft-ci.yml",
                    Role::Workflow,
                    "workflows/statecraft-ci.yml",
                    false
                ),
                template!(
                    ".github/workflows/statecraft-ai-review.yml",
                    Role::Workflow,
                    "workflows/statecraft-ai-review.yml",
                    false
                ),
                template!(
                    "scripts/statecraft/install-spec-spine.sh",
                    Role::Script,
                    "scripts/install-spec-spine.sh",
                    false
                ),
                template!(
                    "scripts/statecraft/gate.sh",
                    Role::Script,
                    "scripts/gate.sh",
                    false
                ),
                template!(
                    "scripts/statecraft/ci-gate.sh",
                    Role::Script,
                    "scripts/ci-gate.sh",
                    false
                ),
                template!(
                    "scripts/statecraft/ai-review.sh",
                    Role::Script,
                    "scripts/ai-review.sh",
                    false
                ),
                template!("Makefile", Role::Makefile, "Makefile", true),
                template!(".github/CODEOWNERS", Role::Codeowners, "CODEOWNERS", true),
            ],
        }
    }

    /// Look a profile up by id.
    pub fn named(id: &str) -> Option<Profile> {
        (id == PROFILE_ID).then(Profile::registered)
    }

    /// The content identity: a digest over every template, the ignore
    /// fragment and the static policy. A changed byte is a different revision.
    pub fn identity(&self) -> String {
        let mut text = format!("profile {} revision {}\n", self.id, self.revision);
        for t in &self.templates {
            text.push_str(&format!(
                "template {} {:?} {} {}\n",
                t.path,
                t.role,
                t.only_when_absent,
                digest_bytes(t.body.as_bytes())
            ));
        }
        text.push_str(&format!(
            "ignore {}\n",
            digest_bytes(IGNORE_FRAGMENT.as_bytes())
        ));
        text.push_str(&format!(
            "policy {}\n",
            digest_bytes(static_policy().to_string().as_bytes())
        ));
        digest_bytes(text.as_bytes())
    }

    /// The source identity its manifest entries carry.
    pub fn source_identity(&self) -> String {
        format!("{SOURCE_PREFIX}{}@{}", self.id, self.revision)
    }
}

/// The part of the policy that does not depend on the project.
fn static_policy() -> serde_json::Value {
    serde_json::json!({
        "commands": commands(),
        "jobs": jobs(),
        "review": {
            "tool": REVIEW_TOOL,
            "tool_version": REVIEW_TOOL_VERSION,
            "credential": CREDENTIAL,
            "skip_classes": SKIP_CLASSES,
            "release_candidate_rule": {
                "requires": "a review (findings or no-findings), or an owner exception",
                "exception_environment": EXCEPTION_ENVIRONMENT,
            },
            "findings_rule": {
                "requires": "a no-findings review, or an owner exception approved for the run",
                "exception_environment": EXCEPTION_ENVIRONMENT,
            },
            "never_an_approval": true,
        },
        "authority_rule": authority_rule(),
        "prerequisites": {
            "local": ["an exact spec-spine pin", "rust-toolchain.toml", "Cargo.lock", "a git work tree"],
        },
        "remote": remote_obligations(),
    })
}

/// The command selections at the default parameters: fixed argument
/// vectors, never a guess.
pub fn commands() -> serde_json::Value {
    commands_for(&Parameters::defaults("main".to_string(), Vec::new()))
}

/// The command selections a project's parameters make (revision 4): coverage
/// enforced or reported, an unresolved claim refused or reported (revision 9,
/// spec 010), and the declared authored-content script, if any.
pub fn commands_for(p: &Parameters) -> serde_json::Value {
    let mut coverage = vec![".tooling/bin/spec-spine", "index", "coverage"];
    if p.enforce_coverage {
        coverage.push("--fail-on-untraced");
    }
    let mut index_check = vec![".tooling/bin/spec-spine", "index", "check"];
    if p.fail_on_unresolved {
        index_check.push("--fail-on-unresolved");
    }
    let mut governance = vec![
        serde_json::json!([".tooling/bin/spec-spine", "check", "--fail-on-warn"]),
        serde_json::json!([".tooling/bin/spec-spine", "lint", "--fail-on-warn"]),
        serde_json::json!(coverage),
        serde_json::json!(index_check),
    ];
    if let Some(script) = &p.authored_content {
        governance.push(serde_json::json!([script]));
    }
    serde_json::json!({
        "governance": governance,
        "code": [
            ["cargo", "build", "--workspace", "--locked"],
            ["cargo", "test", "--workspace", "--locked"],
            ["cargo", "clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"],
            ["cargo", "fmt", "--all", "--check"],
        ],
    })
}

/// Each CI job, whether it is required, and its rule per event.
pub fn jobs() -> serde_json::Value {
    serde_json::json!({
        "governance": {"required": true, "optional": false, "pull_request": "required", "push": "required", "merge_group": "required"},
        "code": {"required": true, "optional": false, "pull_request": "required", "push": "required", "merge_group": "required"},
        "ai-review": {"required": true, "optional": false, "pull_request": "required-review", "push": "inapplicable", "merge_group": "recorded-review"},
        "review-exception": {"required": true, "optional": false, "pull_request": "owner-exception", "push": "inapplicable", "merge_group": "inapplicable"},
    })
}

/// Revision 5, rule 2: a candidate that changes the authority set needs the
/// owner's exception for its run. The rendered workflow runs the exception
/// job when the base's policy carries this rule, and `ci-gate.sh` enforces it
/// from the base's policy whatever any job reports.
pub fn authority_rule() -> serde_json::Value {
    serde_json::json!({
        "set": [
            "every path in files",
            POLICY_PATH,
            "scripts/statecraft/*",
            "the declared governance.authored_content script",
        ],
        "compared": "the candidate's own changes, base...head",
        "requires": "the owner exception approved for the run; in the merge queue, for the run recorded for the entry's pull request",
        "exception_environment": EXCEPTION_ENVIRONMENT,
        "read_at_the_base": ["gate.sh", "install-spec-spine.sh", "ci-gate.sh", "ai-review.sh", "the declared authored-content script"],
    })
}

/// What the product states and never performs.
pub fn remote_obligations() -> Vec<String> {
    vec![
        "Settings, Actions: Actions enabled; the workflow token read-only by default".to_string(),
        format!("the secret {PREFERRED_CREDENTIAL} or {CREDENTIAL} visible to the repository, set by the operator ({CREDENTIAL_COMMAND}, or gh secret set {PREFERRED_CREDENTIAL}); the review uses {PREFERRED_CREDENTIAL} when it is set and {CREDENTIAL} otherwise (revision 8), and neither value is ever in a log, a file or a message"),
        format!("branch protection on the default branch: require the status check ci-gate from GitHub Actions (app id {GATE_APP_ID}), and branches up to date"),
        "branch protection on the default branch: required approvals 0, and require code-owner review, so ci-gate with the AI review approves ordinary changes and a change to the profile's files needs a review its author cannot give (S-3, R2-2)".to_string(),
        format!("the Environment {EXCEPTION_ENVIRONMENT} with the owner as a required reviewer, for owner exceptions: a release candidate whose review was skipped (S-1), a pull request whose review returned findings (R2-1), and a pull request that changes the authority set (revision 5)"),
        "a merge queue, if the default branch requires one: upgrade to revision 3 first, or merge the upgrade while no queue is required, because ci-gate reads its policy at the base and revision 2 states no merge_group rule; a queue entry is judged by the review recorded for its pull request, never by a second review (revision 3)".to_string(),
        "a new refusal (revision 4): a pull request whose base is not the default branch fails governance, because a stacked pull request merges into another branch and is never judged against the default branch; open each branch off the default branch, or set governance.require_default_base to false".to_string(),
        format!("revision 5: a candidate never judges itself with its own gate. gate.sh, install-spec-spine.sh and the declared authored-content script run as they exist at the base, and a pull request that changes the authority set (the rendered workflows, scripts/statecraft/*, the policy, the declared authored-content script) blocks ci-gate until the owner approves the Environment {EXCEPTION_ENVIRONMENT} for that run. Every re-render of the profile, and every change to a file of the authority set, therefore needs the owner's approval once"),
        "the upgrade from revision 4 to 5 is one pull request judged by the base's revision-4 ci-gate, which reports the authority change and does not block it, so the owner's approval of that pull request is procedural: approve it before merging (revision 5)".to_string(),
        "revision 6: every file under .github/workflows/ is in the authority set, rendered or not, so adding, changing or removing any workflow needs the owner's approval once; a workflow the profile does not render could otherwise report a check named ci-gate".to_string(),
        "revision 7: every rendered script exits in one family contract, 0 ok, 1 finding, 2 refused, 3 usage, 4 failed; a missing spec-spine or a missing declared authored-content script now refuses with 2, a usage error is 3, and a command that broke is 4. A job the project must keep required is declared in ci.extra_required_jobs as a reusable workflow under .github/workflows/ (on: workflow_call); ci-gate needs it and blocks on failed, cancelled and skipped exactly as for its own jobs".to_string(),
        "revision 9: governance.fail_on_unresolved (default true) decides whether gate.sh governance runs index check with --fail-on-unresolved; set it to false only in a corpus that approves specs before it builds them, where an approved spec's unbuilt claim is unresolved by design: index check still runs and reports each such claim, and every other governance check is unchanged (spec 010)".to_string(),
        "a repository that already runs these checks by hand keeps them by setting governance.enforce_coverage (index coverage --fail-on-untraced), governance.authored_content (the script's path; absent or not executable refuses), governance.authored_content_text (the title, the body and every commit message), governance.gate_each_commit (each commit's tree passes the gate and cargo fmt) and governance.require_signed_commits (each commit verified as signed by GitHub) (revision 4)".to_string(),
    ]
}

/// The parameters a project may set, validated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Parameters {
    /// The branch a push is gated on.
    pub default_branch: String,
    /// The largest reviewable diff, in changed lines.
    pub diff_cap: u64,
    /// Repository-relative prefixes excluded from the review diff.
    pub exclude: Vec<String>,
    /// The glob over head refs that makes a pull request a release candidate.
    pub release_branch_pattern: String,
    /// The owners a rendered CODEOWNERS names (empty: none is rendered).
    pub code_owners: Vec<String>,
    /// `governance.enforce_coverage`: coverage refuses an untraced file
    /// rather than reporting it (revision 4, rule 1).
    pub enforce_coverage: bool,
    /// `governance.authored_content`: the authored-content script, required
    /// when declared; unset, no authored-content step runs (rule 2).
    pub authored_content: Option<String>,
    /// `governance.authored_content_text`: the declared script also judges
    /// the pull request's title and body and every commit message (rule 3).
    pub authored_content_text: bool,
    /// `governance.gate_each_commit`: each commit's tree passes the gate and
    /// the format check (rule 4).
    pub gate_each_commit: bool,
    /// `governance.require_signed_commits`: each commit is verified as signed
    /// by GitHub (rule 4).
    pub require_signed_commits: bool,
    /// `governance.require_default_base`: a pull request whose base is not
    /// the default branch fails governance (rule 5).
    pub require_default_base: bool,
    /// `governance.fail_on_unresolved`: `index check` refuses an unresolved
    /// claim rather than reporting it (revision 9, spec 010).
    pub fail_on_unresolved: bool,
    /// `ci.extra_required_jobs`: jobs beyond the profile's that ci-gate
    /// requires, each a call of the project's own reusable workflow
    /// (revision 7).
    pub extra_required_jobs: Vec<ExtraJob>,
}

/// One declared extra required job (revision 7): a job id the rendered
/// workflow defines as a call of the project's reusable workflow, which
/// ci-gate needs and the policy requires on every event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExtraJob {
    /// The job id, in the rendered `statecraft-ci.yml`.
    pub job: String,
    /// The reusable workflow it calls, repository-relative, directly under
    /// `.github/workflows/`.
    pub workflow: String,
}

/// Validate `ci.extra_required_jobs`: a list of `{"job", "workflow"}`
/// objects. A job id is GitHub's job-id shape, unique, and none of the
/// profile's own; a workflow is an existing `.yml` or `.yaml` file directly
/// under `.github/workflows/`, and not one the profile renders.
fn extra_required_jobs(root: &Path, value: &serde_json::Value) -> Result<Vec<ExtraJob>, String> {
    const KEY: &str = "ci.extra_required_jobs";
    let list = value
        .as_array()
        .ok_or_else(|| format!("{KEY} must be a list of {{\"job\", \"workflow\"}} objects"))?;
    let mut out: Vec<ExtraJob> = Vec::new();
    for item in list {
        let obj = item
            .as_object()
            .ok_or_else(|| format!("{KEY} entries must be {{\"job\", \"workflow\"}} objects"))?;
        if let Some(other) = obj.keys().find(|k| *k != "job" && *k != "workflow") {
            return Err(format!("{KEY}: unknown key `{other}`"));
        }
        let field = |name: &str| {
            obj.get(name)
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("{KEY} entries need a string `{name}`"))
        };
        let (job, workflow) = (field("job")?, field("workflow")?);
        let job_ok = job.len() <= 100
            && job
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && job
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'));
        if !job_ok {
            return Err(format!("{KEY}: `{job}` is not a GitHub job id"));
        }
        if PROFILE_JOB_IDS.contains(&job) {
            return Err(format!("{KEY}: `{job}` is a job the profile renders"));
        }
        if out.iter().any(|e| e.job == job) {
            return Err(format!("{KEY}: `{job}` is declared twice"));
        }
        let name = workflow.strip_prefix(WORKFLOW_DIR).unwrap_or("");
        let wf_ok = safe_path_chars(workflow)
            && !name.is_empty()
            && !name.contains('/')
            && !name.starts_with('.')
            && (name.ends_with(".yml") || name.ends_with(".yaml"));
        if !wf_ok {
            return Err(format!(
                "{KEY}: `{workflow}` is not a workflow file directly under {WORKFLOW_DIR}"
            ));
        }
        if Profile::registered()
            .templates
            .iter()
            .any(|t| t.path == workflow)
        {
            return Err(format!(
                "{KEY}: `{workflow}` is a workflow the profile renders"
            ));
        }
        if !root.join(workflow).is_file() {
            return Err(format!(
                "{KEY}: `{workflow}` does not exist; write the reusable workflow (on: workflow_call) first"
            ));
        }
        out.push(ExtraJob {
            job: job.to_string(),
            workflow: workflow.to_string(),
        });
    }
    Ok(out)
}

/// The rendered jobs and `needs` entries for the declared extra jobs: empty
/// when none is declared, so a project without any renders what it did.
fn extra_job_text(jobs: &[ExtraJob]) -> (String, String) {
    let defs = jobs
        .iter()
        .map(|e| {
            format!(
                "\n  {job}:\n    name: {job}\n    uses: ./{wf}\n",
                job = e.job,
                wf = e.workflow
            )
        })
        .collect();
    let needs = jobs.iter().map(|e| format!(", {}", e.job)).collect();
    (defs, needs)
}

impl Parameters {
    fn defaults(default_branch: String, exclude: Vec<String>) -> Parameters {
        Parameters {
            default_branch,
            diff_cap: DEFAULT_DIFF_CAP,
            exclude,
            release_branch_pattern: DEFAULT_RELEASE_PATTERN.to_string(),
            code_owners: Vec::new(),
            enforce_coverage: false,
            authored_content: None,
            authored_content_text: false,
            gate_each_commit: false,
            require_signed_commits: false,
            require_default_base: true,
            fail_on_unresolved: true,
            extra_required_jobs: Vec::new(),
        }
    }
}

fn safe_path_chars(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
}

/// Validate a project's `setup` parameters, filling defaults. An unknown key,
/// a value of the wrong type or out of bounds, or `jobs.<name>.enabled` for a
/// job the profile does not mark optional, refuses the plan.
pub fn parameters(
    root: &Path,
    block: &BTreeMap<String, serde_json::Value>,
    derived_dir: &str,
) -> Result<Parameters, String> {
    let mut p = Parameters::defaults(
        remote_head(root).unwrap_or_else(|| "main".to_string()),
        vec![derived_dir.trim_end_matches('/').to_string()],
    );
    let jobs = jobs();
    let flag = |key: &str, value: &serde_json::Value| {
        value
            .as_bool()
            .ok_or_else(|| format!("{key} must be true or false"))
    };
    for (key, value) in block {
        match key.as_str() {
            "governance.enforce_coverage" => p.enforce_coverage = flag(key, value)?,
            "governance.authored_content_text" => p.authored_content_text = flag(key, value)?,
            "governance.gate_each_commit" => p.gate_each_commit = flag(key, value)?,
            "governance.require_signed_commits" => p.require_signed_commits = flag(key, value)?,
            "governance.require_default_base" => p.require_default_base = flag(key, value)?,
            "governance.fail_on_unresolved" => p.fail_on_unresolved = flag(key, value)?,
            "ci.extra_required_jobs" => p.extra_required_jobs = extra_required_jobs(root, value)?,
            "governance.authored_content" => {
                let v = value
                    .as_str()
                    .ok_or("governance.authored_content must be a repository-relative path")?;
                if !safe_path_chars(v)
                    || v.contains("..")
                    || v.starts_with('/')
                    || v.starts_with('-')
                    || v.ends_with('/')
                {
                    return Err(format!(
                        "governance.authored_content `{v}` is not a repository-relative path"
                    ));
                }
                p.authored_content = Some(v.to_string());
            }
            "default_branch" => {
                let v = value.as_str().ok_or("default_branch must be a string")?;
                if !safe_path_chars(v)
                    || v.contains("..")
                    || v.starts_with('-')
                    || v.starts_with('/')
                {
                    return Err(format!("default_branch `{v}` is not a plain ref name"));
                }
                p.default_branch = v.to_string();
            }
            "review.diff_cap" => {
                let v = value
                    .as_u64()
                    .ok_or("review.diff_cap must be a positive integer")?;
                if v == 0 || v > MAX_DIFF_CAP {
                    return Err(format!("review.diff_cap {v} is outside 1..={MAX_DIFF_CAP}"));
                }
                p.diff_cap = v;
            }
            "review.exclude" => {
                let list = value
                    .as_array()
                    .ok_or("review.exclude must be a list of path prefixes")?;
                let mut out = Vec::new();
                for item in list {
                    let v = item
                        .as_str()
                        .ok_or("review.exclude entries must be strings")?;
                    if !safe_path_chars(v) || v.contains("..") || v.starts_with('/') {
                        return Err(format!(
                            "review.exclude entry `{v}` is not a repository-relative prefix"
                        ));
                    }
                    out.push(v.trim_end_matches('/').to_string());
                }
                p.exclude = out;
            }
            "release.branch_pattern" => {
                let v = value
                    .as_str()
                    .ok_or("release.branch_pattern must be a string")?;
                let ok = !v.is_empty()
                    && v.chars().all(|c| {
                        c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | '*' | '?')
                    });
                if !ok {
                    return Err(format!("release.branch_pattern `{v}` is not a ref glob"));
                }
                p.release_branch_pattern = v.to_string();
            }
            "review.code_owners" => {
                let list = value
                    .as_array()
                    .ok_or("review.code_owners must be a list of @handles")?;
                let mut out = Vec::new();
                for item in list {
                    let v = item
                        .as_str()
                        .ok_or("review.code_owners entries must be strings")?;
                    let body = v.strip_prefix('@').unwrap_or("");
                    let ok = !body.is_empty()
                        && body
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/'));
                    if !ok {
                        return Err(format!("review.code_owners entry `{v}` is not an @handle"));
                    }
                    out.push(v.to_string());
                }
                p.code_owners = out;
            }
            other => {
                if let Some(job) = other
                    .strip_prefix("jobs.")
                    .and_then(|r| r.strip_suffix(".enabled"))
                {
                    let optional = jobs[job]["optional"].as_bool();
                    return Err(match optional {
                        None => format!("jobs.{job}.enabled names no job of this profile"),
                        Some(_) => format!(
                            "jobs.{job}.enabled: `{job}` is not optional in revision {REVISION}"
                        ),
                    });
                }
                return Err(format!("unknown setup parameter `{other}`"));
            }
        }
    }
    if p.authored_content_text && p.authored_content.is_none() {
        return Err(
            "governance.authored_content_text needs governance.authored_content: the text is judged by the declared script".to_string(),
        );
    }
    Ok(p)
}

/// The parameters a plan uses: the declared block, plus what an upgrade from
/// before revision 4 carries forward. A revision-3 gate ran
/// `scripts/check-authored-content.sh` whenever it was executable; revision 4
/// runs only a declared script, so the upgrade declares it when it exists and
/// the check a project already ran is kept (revision 4, rule 2).
fn effective_block(
    root: &Path,
    profile: &Profile,
    manifest: &Manifest,
    block: &BTreeMap<String, serde_json::Value>,
) -> BTreeMap<String, serde_json::Value> {
    let mut block = block.clone();
    let upgrading =
        manifest.project.setup.as_ref().is_some_and(|s| {
            s.profile == profile.id && s.revision < AUTHORED_CONTENT_DECLARED_SINCE
        });
    if upgrading
        && profile.revision >= AUTHORED_CONTENT_DECLARED_SINCE
        && !block.contains_key("governance.authored_content")
        && root.join(AUTHORED_CONTENT_SCRIPT).is_file()
    {
        block.insert(
            "governance.authored_content".to_string(),
            serde_json::json!(AUTHORED_CONTENT_SCRIPT),
        );
    }
    block
}

/// What `doctor` says about the authored-content step of a recorded
/// selection (revision 4, rule 2): which script runs, or that none does.
pub fn authored_content_note(selection: &SetupSelection) -> Option<String> {
    if selection.revision < AUTHORED_CONTENT_DECLARED_SINCE {
        return None;
    }
    Some(
        match selection
            .parameters
            .get("governance.authored_content")
            .and_then(|v| v.as_str())
        {
            Some(path) => format!(
                "governance.authored_content is {path}; the gate refuses when it is absent or not executable"
            ),
            None => "governance.authored_content is not declared, so no authored-content step runs"
                .to_string(),
        },
    )
}

/// The remote's own `HEAD`, when the repository records one.
fn remote_head(root: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .args([
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ])
        .current_dir(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let branch = text.strip_prefix("origin/")?.to_string();
    safe_path_chars(&branch).then_some(branch)
}

/// The parameter marker: `{{sc:name}}`. Distinct from a GitHub expression
/// (`${{ ... }}`), which is template text and passes through untouched.
const MARK: &str = "{{sc:";

/// Render one template. Every `{{sc:name}}` must be a known parameter; an
/// unknown or unterminated one is a defect in the template and is reported,
/// never written.
fn render(body: &str, values: &BTreeMap<&str, String>) -> Result<String, String> {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(start) = rest.find(MARK) {
        out.push_str(&rest[..start]);
        let after = &rest[start + MARK.len()..];
        let end = after
            .find("}}")
            .ok_or_else(|| "an unterminated {{sc: in a template".to_string())?;
        let name = &after[..end];
        let value = values
            .get(name)
            .ok_or_else(|| format!("a template names an unknown parameter `{name}`"))?;
        out.push_str(value);
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

/// What the plan does with one path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "action")]
pub enum Action {
    /// Absent: written.
    Write,
    /// Managed and unchanged since it was written: replaced by the new bytes.
    Replace,
    /// Already the intended bytes: recorded only.
    Unchanged,
    /// Written by an interrupted apply of this plan: recorded only.
    Resumed,
    /// Not written, and named.
    Conflict {
        /// Which kind.
        kind: ConflictKind,
    },
    /// Not rendered, by the profile's own rule.
    LeftAlone {
        /// Why, and what an operator may do instead.
        note: String,
    },
    /// Not written because a prerequisite is unmet.
    Withheld,
}

impl Action {
    /// Whether the apply writes the file.
    pub fn writes(&self) -> bool {
        matches!(self, Action::Write | Action::Replace)
    }

    /// Whether the path ends up recorded as the profile's.
    pub fn records(&self) -> bool {
        matches!(
            self,
            Action::Write | Action::Replace | Action::Unchanged | Action::Resumed
        )
    }

    /// A one-word rendering.
    pub fn word(&self) -> String {
        match self {
            Action::Write => "write".into(),
            Action::Replace => "replace".into(),
            Action::Unchanged => "unchanged".into(),
            Action::Resumed => "resumed".into(),
            Action::Conflict { kind } => format!("conflict ({})", kind.word()),
            Action::LeftAlone { .. } => "left-alone".into(),
            Action::Withheld => "withheld".into(),
        }
    }
}

/// Why a path is a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictKind {
    /// It exists and the manifest does not record it: authored, and neither
    /// adopted nor replaced.
    ExistingAuthored,
    /// Managed, and edited since it was written: kept, with the intended bytes
    /// left beside it.
    Customized,
    /// Recorded by another source.
    ForeignManaged,
    /// Written by an interrupted apply, then edited before the re-run.
    EditedAfterInterruption,
}

impl ConflictKind {
    /// A one-word rendering.
    pub fn word(self) -> &'static str {
        match self {
            ConflictKind::ExistingAuthored => "existing-authored",
            ConflictKind::Customized => "customized",
            ConflictKind::ForeignManaged => "foreign-managed",
            ConflictKind::EditedAfterInterruption => "edited-after-interruption",
        }
    }
}

/// One path of the plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePlan {
    /// Repository-relative.
    pub path: String,
    /// Its role.
    pub role: Role,
    /// Whether it is a member of the target's authority set (spec 001 section
    /// 3.5): its check suite and what judges it. Applying it is the
    /// operator's act.
    pub authority_set: bool,
    /// What the plan does.
    #[serde(flatten)]
    pub action: Action,
    /// The intended bytes' digest.
    pub intended: String,
    /// The disk's digest, or `absent`.
    pub current: String,
    /// The manifest's digest, when the profile recorded the path before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
    /// Where the intended bytes are left for a customized file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intended_copy: Option<String>,
    #[serde(skip)]
    bytes: Vec<u8>,
}

/// One prerequisite, with its observed state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prerequisite {
    /// What.
    pub name: String,
    /// What was observed.
    pub observed: String,
    /// Whether it is met.
    pub met: bool,
}

/// How one of the six results stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResultState {
    /// Verified and satisfied.
    Satisfied,
    /// Verified and not satisfied.
    NotSatisfied,
    /// Not asked for.
    NotRun,
    /// Not verified: nothing here can say.
    Unverified,
    /// The profile was withheld.
    Withheld,
}

impl ResultState {
    /// A one-word rendering.
    pub fn word(self) -> &'static str {
        match self {
            ResultState::Satisfied => "satisfied",
            ResultState::NotSatisfied => "not-satisfied",
            ResultState::NotRun => "not-run",
            ResultState::Unverified => "unverified",
            ResultState::Withheld => "withheld",
        }
    }
}

/// One result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Outcome {
    /// How it stands.
    pub state: ResultState,
    /// Why, for a person.
    pub detail: String,
}

impl Outcome {
    fn new(state: ResultState, detail: impl Into<String>) -> Self {
        Self {
            state,
            detail: detail.into(),
        }
    }
}

/// The six results, reported separately. A setup is complete only when all
/// six are satisfied; local initialization can be complete while the remote
/// four are unverified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Results {
    /// Every rendered path present with its intended digest, or named.
    pub files_installed: Outcome,
    /// `gate.sh governance` and `gate.sh code` ran here and passed.
    pub local_checks: Outcome,
    /// The credential exists by name, Actions enabled, token read-only.
    pub remote_prerequisites: Outcome,
    /// Branch protection requires `ci-gate` and code-owner review.
    pub required_checks: Outcome,
    /// A `ci-gate` run exists for the recorded head.
    pub ci_executed: Outcome,
    /// An evidence record exists for that head.
    pub ai_review_produced: Outcome,
}

impl Results {
    /// Whether all six are satisfied.
    pub fn complete(&self) -> bool {
        [
            &self.files_installed,
            &self.local_checks,
            &self.remote_prerequisites,
            &self.required_checks,
            &self.ci_executed,
            &self.ai_review_produced,
        ]
        .iter()
        .all(|o| o.state == ResultState::Satisfied)
    }

    fn local(files: Outcome, local_checks: Outcome) -> Self {
        let unverified = || {
            Outcome::new(
                ResultState::Unverified,
                "local initialization performs no remote read; ask with `doctor --remote`",
            )
        };
        Results {
            files_installed: files,
            local_checks,
            remote_prerequisites: unverified(),
            required_checks: unverified(),
            ci_executed: unverified(),
            ai_review_produced: unverified(),
        }
    }
}

/// The profile plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// The profile id.
    pub profile: String,
    /// Its revision.
    pub revision: u32,
    /// Its content identity.
    pub identity: String,
    /// The plan identity: approve it with `--plan <identity>`.
    pub plan_identity: String,
    /// The validated parameters.
    pub parameters: Parameters,
    /// Every path, in render order, the policy document last.
    pub files: Vec<FilePlan>,
    /// Local prerequisites, observed.
    pub prerequisites: Vec<Prerequisite>,
    /// Set when a prerequisite is unmet and nothing of the profile is written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withheld: Option<String>,
    /// The command selections.
    pub commands: serde_json::Value,
    /// The reviewer's credential, by name only.
    pub credential: String,
    /// How the operator sets it.
    pub credential_command: String,
    /// What the product states and never performs.
    pub remote: Vec<String>,
    /// The ignore patterns the profile adds.
    pub ignore_fragment: String,
    /// The six results.
    pub results: Results,
    /// The declaration's `setup` block as the apply records it.
    #[serde(skip)]
    pub selection: Option<SetupSelection>,
    #[serde(skip)]
    before: BTreeMap<String, String>,
}

impl Plan {
    /// Paths named as conflicts.
    pub fn conflicts(&self) -> Vec<&FilePlan> {
        self.files
            .iter()
            .filter(|f| matches!(f.action, Action::Conflict { .. }))
            .collect()
    }

    /// Whether the profile is fully applied by this plan: not withheld and
    /// no conflict.
    pub fn whole(&self) -> bool {
        self.withheld.is_none() && self.conflicts().is_empty()
    }

    /// Why it is not whole, for a step report.
    pub fn shortfall(&self) -> Option<String> {
        if let Some(why) = &self.withheld {
            return Some(format!("setup profile {} withheld: {why}", self.profile));
        }
        let conflicts: Vec<String> = self
            .conflicts()
            .iter()
            .map(|f| format!("{} ({})", f.path, f.action.word()))
            .collect();
        (!conflicts.is_empty()).then(|| {
            format!(
                "setup profile {}: {} conflict(s): {}",
                self.profile,
                conflicts.len(),
                conflicts.join(", ")
            )
        })
    }

    /// A human-readable rendering.
    pub fn render(&self) -> String {
        let mut out = format!(
            "setup      {} revision {} (identity {}), plan {}\n",
            self.profile,
            self.revision,
            &self.identity[..12],
            self.plan_identity
        );
        for p in &self.prerequisites {
            out.push_str(&format!(
                "setup      prerequisite {}: {} ({})\n",
                p.name,
                if p.met { "met" } else { "UNMET" },
                p.observed
            ));
        }
        if let Some(why) = &self.withheld {
            out.push_str(&format!("setup      withheld: {why}\n"));
        }
        for f in &self.files {
            out.push_str(&format!(
                "setup      {:<11} {}{}\n",
                f.action.word(),
                f.path,
                if f.authority_set {
                    " [authority set]"
                } else {
                    ""
                }
            ));
            if let Action::LeftAlone { note } = &f.action {
                out.push_str(&format!("             {note}\n"));
            }
            if let Action::Conflict { kind } = &f.action {
                out.push_str(&format!(
                    "             {}: previous {}, current {}, intended {}{}\n",
                    kind.word(),
                    f.previous.as_deref().map_or("none", short),
                    short(&f.current),
                    short(&f.intended),
                    f.intended_copy
                        .as_ref()
                        .map(|p| format!("; intended bytes at {p}"))
                        .unwrap_or_default()
                ));
            }
        }
        out.push_str(&format!(
            "setup      authored content: {}\n",
            match &self.parameters.authored_content {
                Some(path) => format!(
                    "{path} (governance.authored_content; absent or not executable refuses)"
                ),
                None =>
                    "none declared (governance.authored_content), so no authored-content step runs"
                        .to_string(),
            }
        ));
        out.push_str(&format!(
            "setup      credential {} (by name only; set it with `{}`)\n",
            self.credential, self.credential_command
        ));
        for r in &self.remote {
            out.push_str(&format!("setup      remote obligation: {r}\n"));
        }
        for (name, o) in results_rows(&self.results) {
            out.push_str(&format!(
                "result     {name:<22} {} ({})\n",
                o.state.word(),
                o.detail
            ));
        }
        out
    }
}

fn short(s: &str) -> &str {
    if s.len() == 64 { &s[..12] } else { s }
}

/// The six results as named rows.
pub fn results_rows(r: &Results) -> [(&'static str, &Outcome); 6] {
    [
        ("files-installed", &r.files_installed),
        ("local-checks", &r.local_checks),
        ("remote-prerequisites", &r.remote_prerequisites),
        ("required-checks", &r.required_checks),
        ("ci-executed", &r.ci_executed),
        ("ai-review-produced", &r.ai_review_produced),
    ]
}

/// The resume record: written before the first write, removed after the
/// manifest is written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resume {
    /// Schema version.
    pub version: u32,
    /// The plan identity that started it.
    pub plan_identity: String,
    /// Each path it would write, in order, with its state before (a digest,
    /// or `absent`).
    pub paths: Vec<ResumePath>,
}

/// One path of the resume record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumePath {
    /// Repository-relative.
    pub path: String,
    /// Its state before the interrupted apply.
    pub before: String,
}

impl Resume {
    /// Read the record, if one is there.
    pub fn read(root: &Path) -> Option<Resume> {
        let bytes = std::fs::read(root.join(RESUME_PATH)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }
}

fn observe(path: &Path) -> String {
    match std::fs::read(path) {
        Ok(bytes) => digest_bytes(&bytes),
        Err(_) => "absent".to_string(),
    }
}

/// The exact pin a `spec-spine.toml` states, when it states one: an
/// uncommented `required_version = "=X.Y.Z"` in `[meta]`, read the way the
/// rendered `install-spec-spine.sh` reads it.
pub fn exact_pin(text: &str) -> Result<String, String> {
    let mut section = String::new();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            section = t.chars().filter(|c| !c.is_whitespace()).collect();
            continue;
        }
        if section != "[meta]" {
            continue;
        }
        let Some(rest) = t.strip_prefix("required_version") else {
            continue;
        };
        let Some(value) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        let value = value.trim();
        let Some(inner) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
            return Err(format!("required_version {value} is not a quoted version"));
        };
        let Some(version) = inner.strip_prefix('=') else {
            return Err(format!(
                "required_version \"{inner}\" is not an exact pin (=X.Y.Z)"
            ));
        };
        let parts: Vec<&str> = version.split('.').collect();
        let exact = parts.len() == 3
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
        return if exact {
            Ok(version.to_string())
        } else {
            Err(format!(
                "required_version \"{inner}\" is not an exact pin (=X.Y.Z)"
            ))
        };
    }
    Err("spec-spine.toml [meta] carries no required_version".to_string())
}

/// Everything the plan reads, supplied by the flow.
pub struct Inputs<'a> {
    /// The project.
    pub root: &'a Path,
    /// The profile.
    pub profile: &'a Profile,
    /// The declaration's parameters.
    pub block: &'a BTreeMap<String, serde_json::Value>,
    /// The manifest as the flow holds it.
    pub manifest: &'a Manifest,
    /// The bytes `spec-spine.toml` will have after the governance step: the
    /// disk's when the file exists, else the producer's.
    pub spec_spine_toml: Option<&'a str>,
    /// The project's derived directory.
    pub derived_dir: &'a str,
}

/// Compute the profile plan. Reads, never writes. `Err` is a refusal: a
/// parameter the profile does not accept, or a template defect.
pub fn plan(inputs: &Inputs<'_>) -> Result<Plan, String> {
    let root = inputs.root;
    let profile = inputs.profile;
    let block = effective_block(root, profile, inputs.manifest, inputs.block);
    let params = parameters(root, &block, inputs.derived_dir)?;
    let identity = profile.identity();
    let source = profile.source_identity();

    // Prerequisites, observed (S-4: the exact pin is required).
    let pin = inputs
        .spec_spine_toml
        .ok_or_else(|| "spec-spine.toml is absent".to_string())
        .and_then(exact_pin);
    let exists = |rel: &str| root.join(rel).exists();
    let prerequisites = vec![
        Prerequisite {
            name: "an exact spec-spine pin".into(),
            observed: match &pin {
                Ok(v) => format!("required_version = \"={v}\""),
                Err(e) => e.clone(),
            },
            met: pin.is_ok(),
        },
        Prerequisite {
            name: "rust-toolchain.toml".into(),
            observed: if exists("rust-toolchain.toml") {
                "present"
            } else {
                "absent; the profile requires it and never writes it"
            }
            .into(),
            met: exists("rust-toolchain.toml"),
        },
        Prerequisite {
            name: "Cargo.lock".into(),
            observed: if exists("Cargo.lock") {
                "present"
            } else {
                "absent; every cargo verb runs --locked"
            }
            .into(),
            met: exists("Cargo.lock"),
        },
        Prerequisite {
            name: "a git work tree".into(),
            observed: if exists(".git") { "present" } else { "absent" }.into(),
            met: exists(".git"),
        },
    ];
    let unmet: Vec<String> = prerequisites
        .iter()
        .filter(|p| !p.met)
        .map(|p| format!("{}: {}", p.name, p.observed))
        .collect();
    let withheld =
        (!unmet.is_empty()).then(|| format!("unmet prerequisite(s): {}", unmet.join("; ")));

    let mut values: BTreeMap<&str, String> = BTreeMap::new();
    values.insert("profile.revision", profile.revision.to_string());
    values.insert("profile.identity", identity.clone());
    values.insert("default_branch", params.default_branch.clone());
    values.insert("review.diff_cap", params.diff_cap.to_string());
    values.insert("review.exclude", params.exclude.join(" "));
    values.insert(
        "release.branch_pattern",
        params.release_branch_pattern.clone(),
    );
    values.insert("review.tool_version", REVIEW_TOOL_VERSION.to_string());
    for (name, on) in [
        ("governance.enforce_coverage", params.enforce_coverage),
        (
            "governance.authored_content_text",
            params.authored_content_text,
        ),
        ("governance.gate_each_commit", params.gate_each_commit),
        (
            "governance.require_signed_commits",
            params.require_signed_commits,
        ),
        (
            "governance.require_default_base",
            params.require_default_base,
        ),
        ("governance.fail_on_unresolved", params.fail_on_unresolved),
    ] {
        values.insert(name, on.to_string());
    }
    values.insert(
        "governance.authored_content",
        params.authored_content.clone().unwrap_or_default(),
    );
    let (extra_jobs, extra_needs) = extra_job_text(&params.extra_required_jobs);
    values.insert("ci.extra_jobs", extra_jobs);
    values.insert("ci.extra_needs", extra_needs);
    let owned_paths: Vec<&str> = profile
        .templates
        .iter()
        .filter(|t| t.role != Role::Codeowners)
        .map(|t| t.path.as_str())
        .chain(std::iter::once(POLICY_PATH))
        .collect();
    values.insert(
        "codeowners.lines",
        owned_paths
            .iter()
            .map(|p| format!("/{p} {}", params.code_owners.join(" ")))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    let resume = Resume::read(root);
    let recorded = |rel: &str| {
        resume
            .as_ref()
            .and_then(|r| r.paths.iter().find(|p| p.path == rel))
            .map(|p| p.before.clone())
    };

    let mut files = Vec::new();
    let mut before = BTreeMap::new();
    let classify = |rel: &str, bytes: &[u8]| -> (Action, String, Option<String>) {
        let intended = digest_bytes(bytes);
        let current = observe(&root.join(rel));
        let entry = inputs.manifest.entry(rel);
        let previous = entry
            .filter(|e| e.class == Class::Managed && e.source.identity.starts_with(SOURCE_PREFIX))
            .map(|e| e.digest.clone());
        let action = if withheld.is_some() {
            Action::Withheld
        } else if let Some(prev) = &previous {
            if current == intended {
                Action::Unchanged
            } else if current == *prev || current == "absent" {
                Action::Replace
            } else {
                Action::Conflict {
                    kind: ConflictKind::Customized,
                }
            }
        } else if entry.is_some() {
            Action::Conflict {
                kind: ConflictKind::ForeignManaged,
            }
        } else if current == "absent" {
            Action::Write
        } else if let Some(was) = recorded(rel) {
            if current == intended {
                Action::Resumed
            } else if current == was {
                Action::Replace
            } else {
                Action::Conflict {
                    kind: ConflictKind::EditedAfterInterruption,
                }
            }
        } else {
            // Authored, even when its bytes happen to equal the intended ones:
            // adopting it would claim the operator's CI is the profile's.
            Action::Conflict {
                kind: ConflictKind::ExistingAuthored,
            }
        };
        (action, current, previous)
    };

    for t in &profile.templates {
        let bytes = render(&t.body, &values)?.into_bytes();
        let managed_before = inputs
            .manifest
            .entry(&t.path)
            .is_some_and(|e| e.source.identity.starts_with(SOURCE_PREFIX));
        let (mut action, current, previous) = classify(&t.path, &bytes);
        let mut path = t.path.clone();
        if t.only_when_absent && !managed_before {
            match t.role {
                Role::Makefile if current != "absent" => {
                    action = Action::LeftAlone {
                        note: "a Makefile exists and is yours; it is never edited. To run the same gate, add: `gate: ; sh scripts/statecraft/gate.sh governance` and `code: ; sh scripts/statecraft/gate.sh code`".into(),
                    };
                }
                Role::Codeowners => {
                    let existing = CODEOWNERS_LOCATIONS.iter().find(|p| root.join(p).exists());
                    if let Some(found) = existing {
                        path = (*found).to_string();
                        action = Action::LeftAlone {
                            note: format!(
                                "{found} exists and is yours; make it name owners for the profile's files so code-owner review covers them (S-3)"
                            ),
                        };
                    } else if params.code_owners.is_empty() {
                        action = Action::LeftAlone {
                            note: "no review.code_owners declared, so no CODEOWNERS is rendered; code-owner review of the profile's files stays a remote obligation (S-3)".into(),
                        };
                    }
                }
                _ => {}
            }
        }
        let intended = digest_bytes(&bytes);
        let intended_copy = matches!(
            action,
            Action::Conflict {
                kind: ConflictKind::Customized | ConflictKind::EditedAfterInterruption
            }
        )
        .then(|| format!("{INTENDED_DIR}/{path}.intended"));
        before.insert(path.clone(), current.clone());
        files.push(FilePlan {
            authority_set: matches!(
                t.role,
                Role::Workflow | Role::Script | Role::Policy | Role::Codeowners
            ),
            path,
            role: t.role,
            action,
            intended,
            current,
            previous,
            intended_copy,
            bytes,
        });
    }

    // The policy document, rendered last because it lists the others.
    let mut policy = static_policy();
    policy["id"] = serde_json::json!(profile.id);
    policy["revision"] = serde_json::json!(profile.revision);
    policy["identity"] = serde_json::json!(identity);
    policy["parameters"] = serde_json::to_value(&params).expect("serializable");
    policy["review"]["release_branch_pattern"] = serde_json::json!(params.release_branch_pattern);
    let commands = commands_for(&params);
    policy["commands"] = commands.clone();
    // Revision 7: a declared extra job is required on every event, by the
    // same rule as the profile's own required jobs.
    for e in &params.extra_required_jobs {
        policy["jobs"][e.job.as_str()] = serde_json::json!({
            "required": true,
            "optional": false,
            "declared": "ci.extra_required_jobs",
            "workflow": e.workflow,
            "pull_request": "required",
            "push": "required",
            "merge_group": "required",
        });
    }
    policy["files"] = serde_json::json!(
        files
            .iter()
            .filter(|f| f.role != Role::Makefile && !matches!(f.action, Action::LeftAlone { .. }))
            .map(|f| serde_json::json!({"path": f.path, "digest": f.intended, "role": f.role}))
            .collect::<Vec<_>>()
    );
    let mut policy_bytes = serde_json::to_string_pretty(&policy).expect("serializable");
    policy_bytes.push('\n');
    let policy_bytes = policy_bytes.into_bytes();
    let (action, current, previous) = classify(POLICY_PATH, &policy_bytes);
    before.insert(POLICY_PATH.to_string(), current.clone());
    let intended = digest_bytes(&policy_bytes);
    files.push(FilePlan {
        path: POLICY_PATH.to_string(),
        role: Role::Policy,
        authority_set: true,
        intended_copy: matches!(
            action,
            Action::Conflict {
                kind: ConflictKind::Customized | ConflictKind::EditedAfterInterruption
            }
        )
        .then(|| format!("{INTENDED_DIR}/{POLICY_PATH}.intended")),
        action,
        intended,
        current,
        previous,
        bytes: policy_bytes,
    });

    // The plan identity: the profile, the declared block, and every path the
    // plan reads or would write, observed and intended.
    let mut id_text = format!("profile {identity}\nsource {source}\n");
    id_text.push_str(&format!(
        "block {}\n",
        serde_json::to_string(&block).expect("serializable")
    ));
    id_text.push_str(&format!(
        "parameters {}\n",
        serde_json::to_string(&params).expect("serializable")
    ));
    for f in &files {
        id_text.push_str(&format!(
            "file {} {} {} {}\n",
            f.path,
            f.current,
            f.intended,
            f.action.word()
        ));
    }
    for rel in [
        "spec-spine.toml",
        "rust-toolchain.toml",
        "Cargo.lock",
        ".gitignore",
    ] {
        id_text.push_str(&format!("read {rel} {}\n", observe(&root.join(rel))));
    }
    if let Some(text) = inputs.spec_spine_toml {
        id_text.push_str(&format!("pin-source {}\n", digest_bytes(text.as_bytes())));
    }
    let plan_identity = digest_bytes(id_text.as_bytes());

    let files_outcome = if withheld.is_some() {
        Outcome::new(ResultState::Withheld, "the profile was withheld")
    } else {
        Outcome::new(ResultState::NotRun, "planned, not applied")
    };
    let results = Results::local(
        files_outcome,
        Outcome::new(
            ResultState::NotRun,
            "local checks run only with `--verify-local`",
        ),
    );

    Ok(Plan {
        profile: profile.id.clone(),
        revision: profile.revision,
        identity: identity.clone(),
        plan_identity,
        parameters: params,
        files,
        prerequisites,
        withheld,
        commands,
        credential: CREDENTIAL.to_string(),
        credential_command: CREDENTIAL_COMMAND.to_string(),
        remote: remote_obligations(),
        ignore_fragment: IGNORE_FRAGMENT.to_string(),
        results,
        selection: Some(SetupSelection {
            profile: profile.id.clone(),
            revision: profile.revision,
            identity,
            parameters: block,
        }),
        before,
    })
}

/// Perform the plan's writes. `write` performs one write and is where the
/// flow observes it; a failure stops the apply where it is, which an
/// interrupted process does too, and the resume record makes the re-run safe.
///
/// Order: the resume record, then each file in plan order, then each
/// `.intended` copy. Manifest entries are recorded in memory; the flow writes
/// the manifest last, then calls [`finish`].
pub fn apply(
    root: &Path,
    plan: &Plan,
    manifest: &mut Manifest,
    now: &str,
    write: &mut dyn FnMut(&Path, &[u8]) -> std::io::Result<()>,
) -> std::io::Result<()> {
    if plan.withheld.is_some() {
        return Ok(());
    }
    let resume = Resume {
        version: 1,
        plan_identity: plan.plan_identity.clone(),
        paths: plan
            .files
            .iter()
            .filter(|f| f.action.writes())
            .map(|f| ResumePath {
                path: f.path.clone(),
                before: plan
                    .before
                    .get(&f.path)
                    .cloned()
                    .unwrap_or_else(|| "absent".into()),
            })
            .collect(),
    };
    let mut record = serde_json::to_string_pretty(&resume).expect("serializable");
    record.push('\n');
    write(&root.join(RESUME_PATH), record.as_bytes())?;
    for f in plan.files.iter().filter(|f| f.action.writes()) {
        write(&root.join(&f.path), &f.bytes)?;
    }
    for f in &plan.files {
        if let Some(copy) = &f.intended_copy {
            write(&root.join(copy), &f.bytes)?;
        }
    }
    let source = format!("{SOURCE_PREFIX}{}@{}", plan.profile, plan.revision);
    for f in plan.files.iter().filter(|f| f.action.records()) {
        // An entry that already records these bytes from this source is left
        // as it is, so a repeat apply rewrites nothing, not even a date.
        let current = manifest.entry(&f.path).is_some_and(|e| {
            e.class == Class::Managed && e.source.identity == source && e.digest == f.intended
        });
        if current {
            continue;
        }
        manifest.upsert(Entry {
            path: f.path.clone(),
            class: Class::Managed,
            source: Source {
                kind: SourceKind::Template,
                identity: source.clone(),
            },
            digest: f.intended.clone(),
            bytes: f.bytes.len() as u64,
            written_at: now.to_string(),
            transfer: manifest.entry(&f.path).and_then(|e| e.transfer.clone()),
            // A rendered CI file is this product's reference content: an
            // edit to it is drift, never an expected authored input.
            role: statecraft_environment::manifest::Role::Reference,
        });
    }
    if let Some(selection) = &plan.selection {
        manifest.project.setup = Some(selection.clone());
    }
    Ok(())
}

/// Remove the resume record once the manifest is written.
pub fn finish(root: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(root.join(RESUME_PATH)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// `files-installed`, read from the disk after an apply.
pub fn files_installed(root: &Path, plan: &Plan) -> Outcome {
    if plan.withheld.is_some() {
        return Outcome::new(ResultState::Withheld, "the profile was withheld");
    }
    let mut wrong = Vec::new();
    for f in &plan.files {
        match &f.action {
            Action::Conflict { .. } | Action::LeftAlone { .. } | Action::Withheld => {}
            _ => {
                if observe(&root.join(&f.path)) != f.intended {
                    wrong.push(f.path.clone());
                }
            }
        }
    }
    if wrong.is_empty() {
        let named = plan.conflicts().len();
        Outcome::new(
            ResultState::Satisfied,
            if named == 0 {
                "every rendered path has its intended digest".to_string()
            } else {
                format!(
                    "every rendered path has its intended digest, and {named} conflict(s) are named"
                )
            },
        )
    } else {
        Outcome::new(
            ResultState::NotSatisfied,
            format!("not as intended: {}", wrong.join(", ")),
        )
    }
}

/// `local-checks`: run the rendered gate here, both halves.
pub fn local_checks(root: &Path) -> Outcome {
    for half in ["governance", "code"] {
        let out = std::process::Command::new("sh")
            .args(["scripts/statecraft/gate.sh", half])
            .current_dir(root)
            .output();
        match out {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                return Outcome::new(
                    ResultState::NotSatisfied,
                    format!(
                        "gate.sh {half} exited {}",
                        o.status
                            .code()
                            .map_or_else(|| "by a signal".to_string(), |c| c.to_string())
                    ),
                );
            }
            Err(e) => {
                return Outcome::new(ResultState::NotSatisfied, format!("gate.sh {half}: {e}"));
            }
        }
    }
    Outcome::new(
        ResultState::Satisfied,
        "gate.sh governance and gate.sh code ran here and passed",
    )
}

/// The results after an apply.
pub fn applied_results(root: &Path, plan: &Plan, verify_local: bool) -> Results {
    let files = files_installed(root, plan);
    let local = if verify_local && plan.withheld.is_none() {
        local_checks(root)
    } else {
        Outcome::new(
            ResultState::NotRun,
            "local checks run only with `--verify-local`",
        )
    };
    Results::local(files, local)
}

/// A read-only host the remote results are asked of.
pub trait Host {
    /// A GitHub REST read, by path (`repos/<owner>/<repo>/...`).
    fn get(&self, path: &str) -> Result<serde_json::Value, HostError>;
}

/// Why a host read did not answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostError {
    /// The host answered that the resource does not exist.
    NotFound,
    /// The host could not be asked, or answered something else.
    Unreachable(String),
}

/// The real host: `gh api`, read-only, with whatever credential `gh` holds.
/// No secret value is ever requested: a secret read returns its name and
/// dates only.
#[derive(Debug, Clone)]
pub struct GhHost {
    /// The program.
    pub program: String,
}

impl Default for GhHost {
    fn default() -> Self {
        Self {
            program: "gh".to_string(),
        }
    }
}

impl Host for GhHost {
    fn get(&self, path: &str) -> Result<serde_json::Value, HostError> {
        let out = std::process::Command::new(&self.program)
            .args(["api", "-X", "GET", path])
            .output()
            .map_err(|e| HostError::Unreachable(format!("{}: {e}", self.program)))?;
        if out.status.success() {
            return serde_json::from_slice(&out.stdout)
                .map_err(|e| HostError::Unreachable(format!("unparseable answer: {e}")));
        }
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout)
        );
        if text.contains("HTTP 404") || text.contains("Not Found") {
            Err(HostError::NotFound)
        } else {
            Err(HostError::Unreachable(
                text.lines().next().unwrap_or("no output").to_string(),
            ))
        }
    }
}

/// `owner/repo` from the `origin` remote, when it is a GitHub URL.
pub fn repository_slug(root: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(root)
        .output()
        .ok()?;
    let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let rest = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("git@github.com:"))
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))?;
    let rest = rest.trim_end_matches('/').trim_end_matches(".git");
    let mut parts = rest.split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    (parts.next().is_none() && !owner.is_empty() && !repo.is_empty())
        .then(|| format!("{owner}/{repo}"))
}

fn head_of(root: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The six results, with the remote four asked of `host`, read-only.
/// `head` is the commit the CI and review results are asked about; absent,
/// the work tree's `HEAD`.
pub fn remote_results(
    root: &Path,
    manifest: &Manifest,
    host: &dyn Host,
    head: Option<&str>,
) -> Results {
    let files = installed_from_manifest(root, manifest);
    let local = Outcome::new(
        ResultState::NotRun,
        match manifest
            .project
            .setup
            .as_ref()
            .and_then(authored_content_note)
        {
            Some(note) => format!("`doctor --remote` does not run the local gate; {note}"),
            None => "`doctor --remote` does not run the local gate".to_string(),
        },
    );
    let Some(slug) = repository_slug(root) else {
        let why = "the origin remote is not a GitHub repository this product can name";
        return Results {
            files_installed: files,
            local_checks: local,
            remote_prerequisites: Outcome::new(ResultState::Unverified, why),
            required_checks: Outcome::new(ResultState::Unverified, why),
            ci_executed: Outcome::new(ResultState::Unverified, why),
            ai_review_produced: Outcome::new(ResultState::Unverified, why),
        };
    };
    let branch = manifest
        .project
        .setup
        .as_ref()
        .and_then(|s| s.parameters.get("default_branch"))
        .and_then(|v| v.as_str().map(str::to_string))
        .or_else(|| remote_head(root))
        .unwrap_or_else(|| "main".to_string());
    let head = head
        .map(str::to_string)
        .or_else(|| head_of(root))
        .unwrap_or_default();

    let unreachable = |what: &str, e: &str| {
        Outcome::new(
            ResultState::Unverified,
            format!("{what}: the host did not answer ({e})"),
        )
    };

    // remote-prerequisites
    let prerequisites = (|| {
        let mut missing = Vec::new();
        match host.get(&format!("repos/{slug}/actions/secrets/{CREDENTIAL}")) {
            Ok(_) => {}
            Err(HostError::NotFound) => missing.push(format!("the secret {CREDENTIAL} is not set")),
            Err(HostError::Unreachable(e)) => return unreachable("the secret", &e),
        }
        match host.get(&format!("repos/{slug}/actions/permissions")) {
            Ok(v) if v["enabled"] == serde_json::json!(true) => {}
            Ok(_) => missing.push("Actions is not enabled".to_string()),
            Err(HostError::NotFound) => missing.push("Actions is not enabled".to_string()),
            Err(HostError::Unreachable(e)) => return unreachable("Actions permissions", &e),
        }
        match host.get(&format!("repos/{slug}/actions/permissions/workflow")) {
            Ok(v) if v["default_workflow_permissions"] == "read" => {}
            Ok(v) => missing.push(format!(
                "the workflow token default is {}",
                v["default_workflow_permissions"]
            )),
            Err(HostError::NotFound) => {
                missing.push("the workflow token default is unknown".into())
            }
            Err(HostError::Unreachable(e)) => return unreachable("workflow permissions", &e),
        }
        match host.get(&format!(
            "repos/{slug}/environments/{EXCEPTION_ENVIRONMENT}"
        )) {
            Ok(v) => {
                let reviewers = v["protection_rules"]
                    .as_array()
                    .is_some_and(|rules| rules.iter().any(|r| r["type"] == "required_reviewers"));
                if !reviewers {
                    missing.push(format!(
                        "the Environment {EXCEPTION_ENVIRONMENT} has no required reviewers"
                    ));
                }
            }
            Err(HostError::NotFound) => missing.push(format!(
                "the Environment {EXCEPTION_ENVIRONMENT} does not exist"
            )),
            Err(HostError::Unreachable(e)) => return unreachable("the exception Environment", &e),
        }
        if missing.is_empty() {
            Outcome::new(
                ResultState::Satisfied,
                format!(
                    "{CREDENTIAL} exists by name, Actions enabled, token read-only, {EXCEPTION_ENVIRONMENT} protected"
                ),
            )
        } else {
            Outcome::new(ResultState::NotSatisfied, missing.join("; "))
        }
    })();

    // required-checks
    let required = match host.get(&format!("repos/{slug}/branches/{branch}/protection")) {
        Ok(v) => {
            let contexts: Vec<String> = v["required_status_checks"]["contexts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c.as_str().map(str::to_string))
                .chain(
                    v["required_status_checks"]["checks"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|c| c["context"].as_str().map(str::to_string)),
                )
                .collect();
            let gate = contexts.iter().any(|c| c == "ci-gate");
            // Revision 2, item 3: the required check names its source. A
            // context without an app, or with another app, is satisfied by a
            // status any token with write access can post.
            let bound = v["required_status_checks"]["checks"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|c| {
                    c["context"] == "ci-gate" && c["app_id"] == serde_json::json!(GATE_APP_ID)
                });
            let owners = v["required_pull_request_reviews"]["require_code_owner_reviews"]
                == serde_json::json!(true);
            match (gate, bound, owners) {
                (true, true, true) => Outcome::new(
                    ResultState::Satisfied,
                    format!("{branch} requires ci-gate from GitHub Actions and code-owner review"),
                ),
                _ => Outcome::new(
                    ResultState::NotSatisfied,
                    format!(
                        "{branch}: ci-gate required: {gate}; ci-gate bound to GitHub Actions (app id {GATE_APP_ID}): {bound}; code-owner review required: {owners}"
                    ),
                ),
            }
        }
        Err(HostError::NotFound) => Outcome::new(
            ResultState::NotSatisfied,
            format!("{branch} has no branch protection"),
        ),
        Err(HostError::Unreachable(e)) => unreachable("branch protection", &e),
    };

    // ci-executed
    let ci = match host.get(&format!(
        "repos/{slug}/commits/{head}/check-runs?check_name=ci-gate"
    )) {
        Ok(v) => {
            let runs = v["check_runs"].as_array().cloned().unwrap_or_default();
            let success = runs.iter().any(|r| r["conclusion"] == "success");
            if success {
                Outcome::new(
                    ResultState::Satisfied,
                    format!("a ci-gate run succeeded for {head}"),
                )
            } else if runs.is_empty() {
                Outcome::new(
                    ResultState::NotSatisfied,
                    format!("no ci-gate run exists for {head}"),
                )
            } else {
                let seen: Vec<String> = runs
                    .iter()
                    .map(|r| {
                        r["conclusion"]
                            .as_str()
                            .or(r["status"].as_str())
                            .unwrap_or("unknown")
                            .to_string()
                    })
                    .collect();
                Outcome::new(
                    ResultState::NotSatisfied,
                    format!("ci-gate for {head} ended: {}", seen.join(", ")),
                )
            }
        }
        Err(HostError::NotFound) => Outcome::new(
            ResultState::NotSatisfied,
            format!("{head} is not known to the host"),
        ),
        Err(HostError::Unreachable(e)) => unreachable("check runs", &e),
    };

    // ai-review-produced
    let review = match host.get(&format!(
        "repos/{slug}/actions/artifacts?name=statecraft-ai-review-{head}"
    )) {
        Ok(v) => {
            let count = v["total_count"].as_u64().unwrap_or(0);
            if count > 0 {
                let id = v["artifacts"][0]["id"].clone();
                Outcome::new(
                    ResultState::Satisfied,
                    format!(
                        "evidence artifact statecraft-ai-review-{head} exists (id {id}); its result is in the record and the pull-request comment"
                    ),
                )
            } else {
                Outcome::new(
                    ResultState::NotSatisfied,
                    format!("no evidence artifact for {head}"),
                )
            }
        }
        Err(HostError::NotFound) => Outcome::new(
            ResultState::NotSatisfied,
            format!("no evidence artifact for {head}"),
        ),
        Err(HostError::Unreachable(e)) => unreachable("artifacts", &e),
    };

    Results {
        files_installed: files,
        local_checks: local,
        remote_prerequisites: prerequisites,
        required_checks: required,
        ci_executed: ci,
        ai_review_produced: review,
    }
}

/// `files-installed`, from the manifest's profile entries and the disk.
pub fn installed_from_manifest(root: &Path, manifest: &Manifest) -> Outcome {
    if manifest.project.setup.is_none() {
        return Outcome::new(ResultState::NotSatisfied, "no setup profile is selected");
    }
    let mut wrong = Vec::new();
    let mut count = 0;
    for e in manifest
        .entries
        .iter()
        .filter(|e| e.source.identity.starts_with(SOURCE_PREFIX))
    {
        count += 1;
        if observe(&root.join(&e.path)) != e.digest {
            wrong.push(e.path.clone());
        }
    }
    if count == 0 {
        Outcome::new(ResultState::NotSatisfied, "no profile file is recorded")
    } else if wrong.is_empty() {
        Outcome::new(
            ResultState::Satisfied,
            format!("{count} recorded profile file(s) have their recorded digest"),
        )
    } else {
        Outcome::new(
            ResultState::NotSatisfied,
            format!("differs from the record: {}", wrong.join(", ")),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_parameter_is_known_and_every_one_renders() {
        let profile = Profile::registered();
        let dir = tempfile::tempdir().unwrap();
        let manifest = Manifest::new(statecraft_environment::manifest::Pins {
            product: "0".into(),
            spec_spine: "0".into(),
            adapters: Default::default(),
            producer: None,
        });
        let block = BTreeMap::new();
        let plan = plan(&Inputs {
            root: dir.path(),
            profile: &profile,
            block: &block,
            manifest: &manifest,
            spec_spine_toml: Some("[meta]\nrequired_version = \"=0.25.0\"\n"),
            derived_dir: ".statecraft/derived",
        })
        .unwrap();
        for f in &plan.files {
            let text = String::from_utf8(f.bytes.clone()).unwrap();
            assert!(!text.contains(MARK), "{} keeps a parameter", f.path);
        }
    }

    #[test]
    fn the_identity_moves_with_a_byte() {
        let a = Profile::registered();
        let mut b = a.clone();
        b.templates[0].body.push(' ');
        assert_ne!(a.identity(), b.identity());
        assert_eq!(a.identity(), Profile::registered().identity());
    }

    #[test]
    fn an_exact_pin_is_read_as_the_install_script_reads_it() {
        assert_eq!(
            exact_pin("[meta]\nrequired_version = \"=0.25.0\"\n").unwrap(),
            "0.25.0"
        );
        assert!(exact_pin("[meta]\n# required_version = \"=0.25.0\"\n").is_err());
        assert!(exact_pin("[meta]\nrequired_version = \"0.25.0\"\n").is_err());
        assert!(exact_pin("[meta]\nrequired_version = \"=0.25\"\n").is_err());
        assert!(exact_pin("[other]\nrequired_version = \"=0.25.0\"\n").is_err());
    }

    #[test]
    fn unknown_and_unoptional_parameters_refuse() {
        let dir = tempfile::tempdir().unwrap();
        let one = |k: &str, v: serde_json::Value| {
            let mut m = BTreeMap::new();
            m.insert(k.to_string(), v);
            parameters(dir.path(), &m, ".statecraft/derived")
        };
        assert!(one("command", serde_json::json!("make")).is_err());
        assert!(one("jobs.code.enabled", serde_json::json!(false)).is_err());
        assert!(one("review.diff_cap", serde_json::json!(0)).is_err());
        assert!(one("review.exclude", serde_json::json!(["../x"])).is_err());
        assert!(one("default_branch", serde_json::json!("main; rm")).is_err());
        assert_eq!(
            one("review.diff_cap", serde_json::json!(150))
                .unwrap()
                .diff_cap,
            150
        );
    }

    #[test]
    fn revision_four_parameters_are_validated_and_default_to_revision_three() {
        let dir = tempfile::tempdir().unwrap();
        let with = |pairs: &[(&str, serde_json::Value)]| {
            let m: BTreeMap<String, serde_json::Value> = pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect();
            parameters(dir.path(), &m, ".statecraft/derived")
        };
        let d = with(&[]).unwrap();
        assert!(!d.enforce_coverage && !d.authored_content_text);
        assert!(!d.gate_each_commit && !d.require_signed_commits);
        assert!(
            d.require_default_base,
            "the one new refusal is on by default"
        );
        assert_eq!(d.authored_content, None);
        assert_eq!(commands(), commands_for(&d));

        for key in [
            "governance.enforce_coverage",
            "governance.authored_content_text",
            "governance.gate_each_commit",
            "governance.require_signed_commits",
            "governance.require_default_base",
            "governance.fail_on_unresolved",
        ] {
            assert!(with(&[(key, serde_json::json!("yes"))]).is_err(), "{key}");
        }
        for bad in ["../x.sh", "/abs.sh", "-x.sh", "a b.sh", "dir/", ""] {
            assert!(
                with(&[("governance.authored_content", serde_json::json!(bad))]).is_err(),
                "{bad}"
            );
        }
        // The text mode is the declared script's, so it needs one.
        let err =
            with(&[("governance.authored_content_text", serde_json::json!(true))]).unwrap_err();
        assert!(err.contains("needs governance.authored_content"), "{err}");
        let p = with(&[
            (
                "governance.authored_content",
                serde_json::json!("scripts/check.sh"),
            ),
            ("governance.authored_content_text", serde_json::json!(true)),
            ("governance.enforce_coverage", serde_json::json!(true)),
        ])
        .unwrap();
        let c = commands_for(&p);
        assert_eq!(
            c["governance"][2],
            serde_json::json!([
                ".tooling/bin/spec-spine",
                "index",
                "coverage",
                "--fail-on-untraced"
            ])
        );
        assert_eq!(c["governance"][4], serde_json::json!(["scripts/check.sh"]));
    }

    /// Spec 010: `governance.fail_on_unresolved` defaults to `true`, and
    /// `false` drops only `--fail-on-unresolved` from the policy's commands.
    #[test]
    fn fail_on_unresolved_is_a_boolean_that_defaults_to_true() {
        let dir = tempfile::tempdir().unwrap();
        let with = |value: Option<serde_json::Value>| {
            let m: BTreeMap<String, serde_json::Value> = value
                .map(|v| ("governance.fail_on_unresolved".to_string(), v))
                .into_iter()
                .collect();
            parameters(dir.path(), &m, ".statecraft/derived")
        };
        let d = with(None).unwrap();
        assert!(d.fail_on_unresolved);
        assert_eq!(with(Some(serde_json::json!(true))).unwrap(), d);
        for bad in [
            serde_json::json!("false"),
            serde_json::json!(0),
            serde_json::Value::Null,
        ] {
            let err = with(Some(bad)).unwrap_err();
            assert!(err.contains("must be true or false"), "{err}");
        }
        let off = with(Some(serde_json::json!(false))).unwrap();
        assert!(!off.fail_on_unresolved);
        let (on, off) = (commands_for(&d), commands_for(&off));
        assert_eq!(
            on["governance"][3],
            serde_json::json!([
                ".tooling/bin/spec-spine",
                "index",
                "check",
                "--fail-on-unresolved"
            ])
        );
        assert_eq!(
            off["governance"][3],
            serde_json::json!([".tooling/bin/spec-spine", "index", "check"])
        );
        let mut rest = on.clone();
        rest["governance"][3] = off["governance"][3].clone();
        assert_eq!(rest, off, "nothing else in the selections moves");
    }

    #[test]
    fn extra_required_jobs_are_validated() {
        let dir = tempfile::tempdir().unwrap();
        let wf = dir.path().join(".github/workflows");
        std::fs::create_dir_all(&wf).unwrap();
        std::fs::write(wf.join("deny.yml"), "on:\n  workflow_call:\n").unwrap();
        let with = |v: serde_json::Value| {
            let mut m = BTreeMap::new();
            m.insert("ci.extra_required_jobs".to_string(), v);
            parameters(dir.path(), &m, ".statecraft/derived")
        };
        let job = |j: &str, w: &str| serde_json::json!([{"job": j, "workflow": w}]);
        let ok = with(job("deny", ".github/workflows/deny.yml")).unwrap();
        assert_eq!(
            ok.extra_required_jobs,
            vec![ExtraJob {
                job: "deny".into(),
                workflow: ".github/workflows/deny.yml".into()
            }]
        );
        assert!(
            with(serde_json::json!([]))
                .unwrap()
                .extra_required_jobs
                .is_empty()
        );
        for (bad, why) in [
            (serde_json::json!("deny"), "must be a list"),
            (serde_json::json!(["deny"]), "objects"),
            (serde_json::json!([{"job": "deny"}]), "a string `workflow`"),
            (
                serde_json::json!([{"job": "deny", "workflow": ".github/workflows/deny.yml", "if": "x"}]),
                "unknown key",
            ),
            (
                job("1deny", ".github/workflows/deny.yml"),
                "not a GitHub job id",
            ),
            (
                job("de ny", ".github/workflows/deny.yml"),
                "not a GitHub job id",
            ),
            (
                job("ci-gate", ".github/workflows/deny.yml"),
                "the profile renders",
            ),
            (
                job("code", ".github/workflows/deny.yml"),
                "the profile renders",
            ),
            (job("deny", "deny.yml"), "directly under"),
            (
                job("deny", ".github/workflows/sub/deny.yml"),
                "directly under",
            ),
            (job("deny", ".github/workflows/../x.yml"), "directly under"),
            (job("deny", ".github/workflows/deny.txt"), "directly under"),
            (
                job("deny", ".github/workflows/statecraft-ci.yml"),
                "the profile renders",
            ),
            (
                job("deny", ".github/workflows/absent.yml"),
                "does not exist",
            ),
            (
                serde_json::json!([
                    {"job": "deny", "workflow": ".github/workflows/deny.yml"},
                    {"job": "deny", "workflow": ".github/workflows/deny.yml"}
                ]),
                "declared twice",
            ),
        ] {
            let err = with(bad.clone()).unwrap_err();
            assert!(err.contains(why), "{bad}: {err}");
        }
    }

    #[test]
    fn doctor_names_the_authored_content_step() {
        let mut s = SetupSelection {
            profile: PROFILE_ID.into(),
            revision: 3,
            identity: String::new(),
            parameters: BTreeMap::new(),
        };
        assert_eq!(authored_content_note(&s), None);
        s.revision = REVISION;
        assert!(
            authored_content_note(&s)
                .unwrap()
                .contains("not declared, so no authored-content step runs")
        );
        s.parameters.insert(
            "governance.authored_content".into(),
            serde_json::json!(AUTHORED_CONTENT_SCRIPT),
        );
        assert!(
            authored_content_note(&s)
                .unwrap()
                .contains("refuses when it is absent or not executable")
        );
    }
}
