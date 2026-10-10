//! Spec 012: the remote desired-state document a setup profile renders, and
//! the read-only per-field comparison `doctor --remote` makes of it.
//!
//! The host is a fake that answers by path and records every path asked, so
//! no test reaches GitHub. Each negative case of section 3.6 that the library
//! decides has a test here; the usage refusal and the binary's exit are in
//! `statecraft-cli`'s `setup_profile` suite.

use statecraft_environment::digest::digest_bytes;
use statecraft_environment::manifest::{Manifest, Pins};
use statecraft_home::remote_state::{
    self, Comparison, DesiredState, FieldState, ProfileClaim, RemoteParameters, Rendering,
};
use statecraft_home::setup::{self, Host, HostError, Inputs, Profile, Role};
use statecraft_home::setup_input;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

const TOML: &str = "[meta]\nrequired_version = \"=0.26.0\"\n";
const SLUG: &str = "owner/fixture";

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (rel, text) in [
        ("spec-spine.toml", TOML),
        ("rust-toolchain.toml", "[toolchain]\nchannel = \"1.96.0\"\n"),
        ("Cargo.lock", "version = 4\n"),
    ] {
        std::fs::write(dir.path().join(rel), text).unwrap();
    }
    for args in [
        &["init", "--quiet"][..],
        &["add", "rust-toolchain.toml", "Cargo.lock"],
    ] {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}");
    }
    dir
}

fn manifest() -> Manifest {
    Manifest::new(Pins {
        product: "test".into(),
        spec_spine: "unpinned".into(),
        adapters: Default::default(),
        producer: None,
    })
}

fn plan_with(
    root: &Path,
    manifest: &Manifest,
    block: &BTreeMap<String, serde_json::Value>,
) -> Result<setup::Plan, String> {
    setup::plan(&Inputs {
        root,
        profile: &Profile::registered(),
        block,
        manifest,
        spec_spine_toml: Some(TOML),
        derived_dir: ".statecraft/derived",
        bound: &setup_input::Bound::with_producer("0.26.0"),
    })
}

fn apply(root: &Path, plan: &setup::Plan, manifest: &mut Manifest) {
    setup::apply(root, plan, manifest, "2026-10-10T00:00:00Z", &mut |p, b| {
        std::fs::create_dir_all(p.parent().unwrap())?;
        std::fs::write(p, b)
    })
    .unwrap();
    setup::finish(root).unwrap();
}

fn declared() -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        (
            "remote.merge_queue".to_string(),
            serde_json::json!({"required": true, "merge_method": "merge", "build_concurrency": 5, "all_entries_must_pass": true}),
        ),
        (
            "remote.exception_reviewers".to_string(),
            serde_json::json!(["@owner", "@owner/release"]),
        ),
        (
            "remote.custom_properties".to_string(),
            serde_json::json!({"team": "platform"}),
        ),
        (
            "review.code_owners".to_string(),
            serde_json::json!(["@owner"]),
        ),
    ])
}

// ------------------------------------------------------------- the document

#[test]
fn http_status_classification_skips_non_status_prefixes_and_keeps_only_the_status() {
    use statecraft_home::setup::HostError;
    for code in ["401", "403", "502"] {
        let error = setup::classify_gh_failure(&format!(
            "Sending HTTP request with private context\ngh: private response (HTTP {code})"
        ));
        let expected = format!("HTTP {code}");
        match error {
            HostError::Unauthorized(reason) if code != "502" => assert_eq!(reason, expected),
            HostError::Unreachable(reason) if code == "502" => assert_eq!(reason, expected),
            other => panic!("unexpected classification: {other:?}"),
        }
    }
    assert_eq!(
        setup::classify_gh_failure("Sending HTTP request\ngh: missing (HTTP 404)"),
        HostError::NotFound
    );
}

#[test]
fn the_profile_renders_one_canonical_document_the_manifest_records() {
    let dir = project();
    let root = dir.path();
    let mut m = manifest();
    let plan = plan_with(root, &m, &BTreeMap::new()).unwrap();
    assert_eq!(plan.revision, 16);
    let file = plan
        .files
        .iter()
        .find(|f| f.path == remote_state::PATH)
        .expect("the document is planned");
    assert_eq!(file.role, Role::RemoteState);
    assert!(file.authority_set, "the document is in the authority set");
    apply(root, &plan, &mut m);

    let bytes = std::fs::read(root.join(remote_state::PATH)).unwrap();
    let doc = DesiredState::parse(&bytes).unwrap();
    assert_eq!(doc.canonical(), bytes, "the committed bytes are canonical");
    // Check the emitted ordering independently of canonical() itself.
    let text = std::str::from_utf8(&bytes).unwrap();
    let keys: Vec<_> = text
        .lines()
        .filter_map(|line| {
            line.strip_prefix("  \"")
                .and_then(|rest| rest.split('"').next())
        })
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(
        keys, sorted,
        "the emitted root keys are lexicographically sorted"
    );
    assert!(text.contains("\"profile\": {\n    \"id\":"));
    let obligations = setup::remote_obligations();
    let revision = |n| {
        obligations
            .iter()
            .position(|note| note.starts_with(&format!("revision {n}:")))
            .unwrap()
    };
    for (earlier, later) in [(13, 14), (14, 15), (15, 16)] {
        assert!(
            revision(earlier) < revision(later),
            "revision notes stay chronological"
        );
    }
    assert_eq!(doc.schema, remote_state::SCHEMA);
    assert_eq!(
        doc.profile,
        ProfileClaim {
            id: setup::PROFILE_ID.into(),
            revision: setup::REVISION,
            identity: Profile::registered().identity(),
        }
    );
    // The manifest records its path, digest, profile id, revision and schema.
    let selection = m.project.setup.as_ref().unwrap();
    let record = selection.remote_state.as_ref().unwrap();
    assert_eq!(record.path, remote_state::PATH);
    assert_eq!(record.digest, digest_bytes(&bytes));
    assert_eq!(record.schema, remote_state::SCHEMA);
    assert_eq!(
        (selection.profile.as_str(), selection.revision),
        (setup::PROFILE_ID, 16)
    );
    let entry = m.entry(remote_state::PATH).unwrap();
    assert_eq!(entry.digest, digest_bytes(&bytes));
    assert!(entry.source.identity.ends_with("github-actions-rust@16"));

    // The policy lists it, and CODEOWNERS coverage includes it.
    let policy: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join(setup::POLICY_PATH)).unwrap()).unwrap();
    assert!(
        policy["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["path"] == remote_state::PATH && f["role"] == "remote-state")
    );
    assert!(
        doc.code_owner_review
            .paths
            .contains(&format!("/{}", remote_state::PATH))
    );
    assert!(
        doc.code_owner_review
            .paths
            .contains(&format!("/{}", setup::POLICY_PATH))
    );

    // A repeat plan of the same inputs renders the same bytes.
    let again = plan_with(root, &m, &BTreeMap::new()).unwrap();
    let f = again
        .files
        .iter()
        .find(|f| f.path == remote_state::PATH)
        .unwrap();
    assert_eq!(f.action.word(), "unchanged");
}

#[test]
fn every_field_of_section_3_1_is_present_and_an_omission_is_no_claim() {
    let dir = project();
    let plan = plan_with(dir.path(), &manifest(), &BTreeMap::new()).unwrap();
    let mut m = manifest();
    apply(dir.path(), &plan, &mut m);
    let text = std::fs::read_to_string(dir.path().join(remote_state::PATH)).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    for key in [
        "profile",
        "defaultBranch",
        "requiredChecks",
        "extraRequiredJobs",
        "codeOwnerReview",
        "reviewException",
        "workflowToken",
        "secrets",
    ] {
        assert!(v.get(key).is_some(), "{key} is declared");
    }
    // Undeclared, these three are omitted, never defaulted.
    for key in ["mergeQueue", "customProperties"] {
        assert!(v.get(key).is_none(), "{key} is omitted: {text}");
    }
    assert!(v["reviewException"].get("reviewers").is_none());
    assert_eq!(
        v["requiredChecks"],
        serde_json::json!([{"name": "ci-gate", "appId": setup::GATE_APP_ID}])
    );
    assert_eq!(v["workflowToken"]["defaultPermissions"], "read");
    assert_eq!(
        v["secrets"][0]["anyOf"],
        serde_json::json!([setup::PREFERRED_CREDENTIAL, setup::CREDENTIAL])
    );
    assert!(
        !text.contains("value\""),
        "no secret value is ever declared"
    );

    // Declared, they are rendered exactly.
    let dir = project();
    let plan = plan_with(dir.path(), &manifest(), &declared()).unwrap();
    let mut m = manifest();
    apply(dir.path(), &plan, &mut m);
    let doc =
        DesiredState::parse(&std::fs::read(dir.path().join(remote_state::PATH)).unwrap()).unwrap();
    let q = doc.merge_queue.unwrap();
    assert!(q.required);
    assert_eq!(q.merge_method.as_deref(), Some("merge"));
    assert_eq!(q.build_concurrency, Some(5));
    assert_eq!(q.all_entries_must_pass, Some(true));
    assert_eq!(
        doc.review_exception.reviewers.unwrap(),
        vec!["@owner".to_string(), "@owner/release".to_string()]
    );
    assert_eq!(doc.custom_properties.unwrap().values["team"], "platform");
    assert_eq!(
        doc.code_owner_review.owners.unwrap(),
        vec!["@owner".to_string()]
    );
}

#[test]
fn an_invalid_remote_parameter_refuses_the_plan() {
    let dir = project();
    for (key, value, says) in [
        (
            "remote.merge_queue",
            serde_json::json!({"required": true}),
            "merge_method",
        ),
        (
            "remote.merge_queue",
            serde_json::json!({"required": false, "merge_method": "merge"}),
            "only when the queue is required",
        ),
        (
            "remote.merge_queue",
            serde_json::json!({"required": true, "merge_method": "merge", "build_concurrency": 5, "all_entries_must_pass": true, "x": 1}),
            "unknown key `x`",
        ),
        (
            "remote.exception_reviewers",
            serde_json::json!([]),
            "nonempty",
        ),
        (
            "remote.exception_reviewers",
            serde_json::json!(["owner"]),
            "handle",
        ),
        (
            "remote.exception_reviewers",
            serde_json::json!(["@a", "@A"]),
            "twice",
        ),
        (
            "remote.custom_properties",
            serde_json::json!({"team": 1}),
            "string",
        ),
        (
            "remote.custom_properties",
            serde_json::json!({"a b": "x"}),
            "property name",
        ),
        (
            "remote.apply",
            serde_json::json!(true),
            "unknown setup parameter",
        ),
    ] {
        let block = BTreeMap::from([(key.to_string(), value)]);
        let err = plan_with(dir.path(), &manifest(), &block).unwrap_err();
        assert!(err.contains(says), "{key}: {err}");
    }
}

#[test]
fn an_unknown_document_field_or_schema_refuses_the_parse() {
    let doc = render(&RemoteParameters::default());
    let mut v = serde_json::to_value(&doc).unwrap();
    v["applyWith"] = serde_json::json!("terraform");
    let err = DesiredState::parse(v.to_string().as_bytes()).unwrap_err();
    assert!(err.contains("applyWith"), "{err}");
    let mut v = serde_json::to_value(&doc).unwrap();
    v["workflowToken"]["extra"] = serde_json::json!(1);
    assert!(DesiredState::parse(v.to_string().as_bytes()).is_err());
    let mut v = serde_json::to_value(&doc).unwrap();
    v["schema"] = serde_json::json!("statecraft/remote-desired-state/2");
    assert!(
        DesiredState::parse(v.to_string().as_bytes())
            .unwrap_err()
            .contains("schema")
    );
}

// ------------------------------------------------------------ a fake host

/// Answers by exact path; anything unanswered is unreachable. Records each
/// path asked.
struct Fake {
    answers: BTreeMap<String, Result<serde_json::Value, HostError>>,
    asked: RefCell<Vec<String>>,
}

impl Host for Fake {
    fn get(&self, path: &str) -> Result<serde_json::Value, HostError> {
        self.asked.borrow_mut().push(path.to_string());
        self.answers
            .get(path)
            .cloned()
            .unwrap_or_else(|| Err(HostError::Unreachable(format!("no answer for {path}"))))
    }
}

fn b64(bytes: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.len();
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let v = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= n {
                out.push(A[((v >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
        // GitHub wraps its base64 at 60 columns.
        if out.len() % 61 == 60 {
            out.push('\n');
        }
    }
    out
}

fn contents(text: &str) -> serde_json::Value {
    serde_json::json!({"encoding": "base64", "content": b64(text.as_bytes())})
}

fn render(remote: &RemoteParameters) -> DesiredState {
    remote_state::render(&Rendering {
        profile: ProfileClaim {
            id: setup::PROFILE_ID.into(),
            revision: setup::REVISION,
            identity: "f".repeat(64),
        },
        default_branch: "main",
        gate_app_id: setup::GATE_APP_ID,
        extra_jobs: vec![("platform".into(), ".github/workflows/platform.yml".into())],
        governed_paths: vec![
            "/.github/workflows/statecraft-ci.yml".into(),
            format!("/{}", setup::POLICY_PATH),
        ],
        code_owners: &["@owner".to_string()],
        exception_environment: setup::EXCEPTION_ENVIRONMENT,
        credentials: setup::credentials(),
        remote,
    })
}

fn full_remote() -> RemoteParameters {
    RemoteParameters {
        merge_queue: Some(
            remote_state::merge_queue(&serde_json::json!({"required": true, "merge_method": "squash", "build_concurrency": 5, "all_entries_must_pass": true}))
                .unwrap(),
        ),
        exception_reviewers: Some(vec!["@owner".into(), "@owner/release".into()]),
        custom_properties: Some(BTreeMap::from([("team".into(), "platform".into())])),
    }
}

fn r(suffix: &str) -> String {
    format!("repos/{SLUG}{suffix}")
}

/// A host on which every declared field matches, for an organization-owned
/// repository.
fn matching(doc: &DesiredState) -> Fake {
    use serde_json::json;
    let policy = json!({
        "id": doc.profile.id, "revision": doc.profile.revision, "identity": doc.profile.identity,
        "jobs": {"platform": {"required": true, "workflow": ".github/workflows/platform.yml"}},
    });
    let workflow = "jobs:\n  platform:\n    uses: ./.github/workflows/platform.yml\n  ci-gate:\n    needs: [governance, code, platform]\n";
    let codeowners =
        "# governed\n/.github/workflows/statecraft-ci.yml @owner\n/.statecraft/setup/ @owner\n";
    let answers = BTreeMap::from([
        (
            r(""),
            Ok(
                json!({"default_branch": "main", "private": false, "owner": {"login": "owner", "type": "Organization"}}),
            ),
        ),
        (
            r(&format!("/contents/{}?ref=main", setup::POLICY_PATH)),
            Ok(contents(&policy.to_string())),
        ),
        (
            r("/contents/.github/workflows/statecraft-ci.yml?ref=main"),
            Ok(contents(workflow)),
        ),
        (
            r("/contents/.github/CODEOWNERS?ref=main"),
            Ok(contents(codeowners)),
        ),
        (
            r("/branches/main/protection"),
            Ok(json!({
                "required_status_checks": {"contexts": ["ci-gate"], "checks": [{"context": "ci-gate", "app_id": setup::GATE_APP_ID}]},
                "required_pull_request_reviews": {"require_code_owner_reviews": true},
            })),
        ),
        (
            r("/rules/branches/main"),
            Ok(
                json!([{"type": "merge_queue", "parameters": {"merge_method": "SQUASH", "max_entries_to_build": 5, "grouping_strategy": "ALLGREEN"}}]),
            ),
        ),
        (
            r(&format!("/environments/{}", setup::EXCEPTION_ENVIRONMENT)),
            Ok(
                json!({"protection_rules": [{"type": "required_reviewers", "prevent_self_review": true, "reviewers": [
                    {"type": "User", "reviewer": {"login": "owner"}},
                    {"type": "Team", "reviewer": {"slug": "release"}},
                ]}]}),
            ),
        ),
        (
            r("/actions/permissions/workflow"),
            Ok(
                json!({"default_workflow_permissions": "read", "can_approve_pull_request_reviews": false}),
            ),
        ),
        (
            "orgs/owner/actions/permissions/workflow".to_string(),
            Ok(
                json!({"default_workflow_permissions": "read", "can_approve_pull_request_reviews": false}),
            ),
        ),
        (
            r(&format!("/actions/secrets/{}", setup::PREFERRED_CREDENTIAL)),
            Err(HostError::NotFound),
        ),
        (
            r(&format!("/actions/secrets/{}", setup::CREDENTIAL)),
            Ok(json!({"name": setup::CREDENTIAL, "created_at": "2026-10-10T00:00:00Z"})),
        ),
        (
            r("/actions/organization-secrets?per_page=100"),
            Ok(json!({"total_count": 0, "secrets": []})),
        ),
        (
            r("/properties/values"),
            Ok(json!([{"property_name": "team", "value": "platform"}])),
        ),
        (
            "orgs/owner/properties/schema".to_string(),
            Ok(json!([{"property_name": "team", "value_type": "string"}])),
        ),
    ]);
    Fake {
        answers,
        asked: RefCell::new(Vec::new()),
    }
}

fn state(c: &Comparison, field: &str) -> FieldState {
    c.fields
        .iter()
        .find(|f| f.field == field)
        .unwrap_or_else(|| panic!("no leaf {field}: {c:#?}"))
        .state
}

fn reason<'a>(c: &'a Comparison, field: &str) -> &'a str {
    &c.fields.iter().find(|f| f.field == field).unwrap().reason
}

// ------------------------------------------------------------ the comparison

#[test]
fn every_declared_leaf_matches_on_a_conforming_host_and_every_call_is_a_read() {
    let doc = render(&full_remote());
    let host = matching(&doc);
    let c = remote_state::compare(Ok(Some(doc)), Some(SLUG), &host);
    for f in &c.fields {
        assert_eq!(f.state, FieldState::Matching, "{}: {}", f.field, f.reason);
        assert!(f.declared);
    }
    assert!(!c.has_findings());
    assert_eq!(c.summary["matching"], c.fields.len());
    for leaf in [
        "profile",
        "defaultBranch",
        "requiredChecks.ci-gate",
        "extraRequiredJobs.platform",
        "codeOwnerReview.required",
        "codeOwnerReview.paths",
        "mergeQueue.required",
        "mergeQueue.mergeMethod",
        "mergeQueue.buildConcurrency",
        "mergeQueue.allEntriesMustPass",
        "reviewException.environment",
        "reviewException.reviewers.@owner",
        "reviewException.reviewers.@owner/release",
        "reviewException.preventSelfReview",
        "workflowToken.defaultPermissions",
        "workflowToken.canApprovePullRequestReviews",
        "secrets.ANTHROPIC_API_KEY|CLAUDE_CODE_OAUTH_TOKEN",
        "customProperties.team",
    ] {
        assert_eq!(state(&c, leaf), FieldState::Matching, "{leaf}");
    }
    assert!(reason(&c, "workflowToken.defaultPermissions").contains("organization policy"));
    // Read-only: the only reads are of this repository and its organization,
    // and no secret's value is ever asked for: a secret is read by name.
    for path in host.asked.borrow().iter() {
        assert!(
            path.starts_with(&format!("repos/{SLUG}")) || path.starts_with("orgs/owner/"),
            "{path}"
        );
        if path.contains("secrets") {
            assert!(
                path.ends_with(setup::CREDENTIAL)
                    || path.ends_with(setup::PREFERRED_CREDENTIAL)
                    || path.ends_with("organization-secrets?per_page=100"),
                "{path}"
            );
        }
    }
}

#[test]
fn an_omitted_field_is_unverified_and_not_a_finding() {
    let doc = render(&RemoteParameters::default());
    let host = matching(&doc);
    let c = remote_state::compare(Ok(Some(doc)), Some(SLUG), &host);
    for leaf in [
        "mergeQueue",
        "reviewException.reviewers",
        "customProperties",
    ] {
        let f = c.fields.iter().find(|f| f.field == leaf).unwrap();
        assert_eq!(f.state, FieldState::Unverified, "{leaf}");
        assert!(!f.declared, "{leaf}");
        assert!(f.reason.contains("no claim"), "{leaf}: {}", f.reason);
    }
    assert!(!c.has_findings(), "{c:#?}");
    // The undeclared reviewers and property are reported as observed extra
    // state, never added to the desired state.
    assert!(
        c.extras
            .iter()
            .any(|e| e.field == "reviewException.reviewers" && e.observed == "@owner")
    );
    assert!(c.extras.iter().any(|e| e.field == "customProperties.team"));
}

#[test]
fn an_unreachable_host_makes_every_field_unavailable() {
    let doc = render(&full_remote());
    let host = Fake {
        answers: BTreeMap::new(),
        asked: RefCell::new(Vec::new()),
    };
    let c = remote_state::compare(Ok(Some(doc)), Some(SLUG), &host);
    assert!(!c.fields.is_empty());
    for f in &c.fields {
        assert_eq!(
            f.state,
            FieldState::Unavailable,
            "{}: {}",
            f.field,
            f.reason
        );
    }
    assert!(c.has_findings());
}

#[test]
fn one_unauthorized_read_leaves_the_others_compared() {
    let doc = render(&full_remote());
    let mut host = matching(&doc);
    host.answers.insert(
        r("/actions/permissions/workflow"),
        Err(HostError::Unauthorized("HTTP 403".into())),
    );
    let c = remote_state::compare(Ok(Some(doc)), Some(SLUG), &host);
    for leaf in [
        "workflowToken.defaultPermissions",
        "workflowToken.canApprovePullRequestReviews",
    ] {
        assert_eq!(state(&c, leaf), FieldState::Unauthorized, "{leaf}");
    }
    let others = c
        .fields
        .iter()
        .filter(|f| !f.field.starts_with("workflowToken"));
    for f in others {
        assert_eq!(f.state, FieldState::Matching, "{}: {}", f.field, f.reason);
    }
}

#[test]
fn an_absent_secret_name_is_drifted_and_no_value_is_requested() {
    let doc = render(&full_remote());
    let mut host = matching(&doc);
    host.answers.insert(
        r(&format!("/actions/secrets/{}", setup::CREDENTIAL)),
        Err(HostError::NotFound),
    );
    let c = remote_state::compare(Ok(Some(doc)), Some(SLUG), &host);
    let leaf = "secrets.ANTHROPIC_API_KEY|CLAUDE_CODE_OAUTH_TOKEN";
    assert_eq!(state(&c, leaf), FieldState::Drifted);
    assert!(reason(&c, leaf).contains("visible"));

    // An organization secret of an allowed name satisfies it.
    host.answers.insert(
        r("/actions/organization-secrets?per_page=100"),
        Ok(serde_json::json!({"secrets": [{"name": setup::PREFERRED_CREDENTIAL}]})),
    );
    let c = remote_state::compare(Ok(Some(render(&full_remote()))), Some(SLUG), &host);
    assert_eq!(state(&c, leaf), FieldState::Matching);
}

#[test]
fn organization_secret_names_are_compared_across_pages_and_partial_reads_are_unverified() {
    let doc = render(&full_remote());
    let mut host = matching(&doc);
    host.answers.insert(
        r(&format!("/actions/secrets/{}", setup::CREDENTIAL)),
        Err(HostError::NotFound),
    );
    let names: Vec<_> = (0..100)
        .map(|i| serde_json::json!({"name": format!("OTHER_{i}")}))
        .collect();
    host.answers.insert(
        r("/actions/organization-secrets?per_page=100"),
        Ok(serde_json::json!({"total_count": 101, "secrets": names})),
    );
    let next = r("/actions/organization-secrets?per_page=100&page=2");
    host.answers.insert(
        next.clone(),
        Ok(serde_json::json!({
            "total_count": 101, "secrets": [{"name": setup::PREFERRED_CREDENTIAL}]
        })),
    );
    let leaf = "secrets.ANTHROPIC_API_KEY|CLAUDE_CODE_OAUTH_TOKEN";
    let c = remote_state::compare(Ok(Some(doc.clone())), Some(SLUG), &host);
    assert_eq!(state(&c, leaf), FieldState::Matching);
    assert!(host.asked.borrow().contains(&next));

    host.answers
        .insert(next, Err(HostError::Unauthorized("HTTP 403".into())));
    let c = remote_state::compare(Ok(Some(doc)), Some(SLUG), &host);
    assert_eq!(state(&c, leaf), FieldState::Unauthorized);
}

#[test]
fn malformed_unicode_identities_are_findings_instead_of_panics() {
    let dir = project();
    let mut m = manifest();
    let plan = plan_with(dir.path(), &m, &BTreeMap::new()).unwrap();
    apply(dir.path(), &plan, &mut m);
    let path = dir.path().join(remote_state::PATH);
    let mut doc = DesiredState::parse(&std::fs::read(&path).unwrap()).unwrap();
    doc.profile.identity = "a".repeat(11) + "é";
    std::fs::write(&path, doc.canonical()).unwrap();
    assert!(
        remote_state::read_local(dir.path(), &m)
            .unwrap_err()
            .contains("selected profile")
    );
}

#[test]
fn team_reviewers_preserve_an_unreadable_owner_without_hiding_independent_fields() {
    for (answer, expected) in [
        (
            Err(HostError::Unreachable("HTTP 502".into())),
            FieldState::Unavailable,
        ),
        (
            Err(HostError::Unauthorized("HTTP 403".into())),
            FieldState::Unauthorized,
        ),
        (
            Ok(serde_json::json!({"private": false, "owner": {"type": "Organization"}})),
            FieldState::Unverified,
        ),
    ] {
        let doc = render(&full_remote());
        let mut host = matching(&doc);
        host.answers.insert(r(""), answer);
        let c = remote_state::compare(Ok(Some(doc)), Some(SLUG), &host);
        assert_eq!(
            state(&c, "reviewException.reviewers.@owner/release"),
            expected
        );
        assert_eq!(
            state(&c, "reviewException.reviewers.@owner"),
            FieldState::Matching
        );
        assert_eq!(
            state(&c, "reviewException.preventSelfReview"),
            FieldState::Matching
        );
        assert!(
            !c.extras
                .iter()
                .any(|extra| extra.observed == serde_json::json!("@/release"))
        );
    }
}

#[test]
fn the_contents_fixture_wraps_every_base64_line_at_sixty_columns() {
    let encoded = b64(&[0; 180]);
    assert_eq!(
        encoded.lines().map(str::len).collect::<Vec<_>>(),
        vec![60; 4]
    );
}

#[test]
fn a_feature_absent_from_the_plan_is_unsupported_and_names_the_limitation() {
    let doc = render(&full_remote());
    let mut host = matching(&doc);
    // A personal private repository: no merge queue, no Environment
    // reviewers, no custom properties.
    host.answers.insert(
        r(""),
        Ok(serde_json::json!({"default_branch": "main", "private": true, "owner": {"login": "owner", "type": "User"}})),
    );
    host.answers.insert(
        r(&format!("/environments/{}", setup::EXCEPTION_ENVIRONMENT)),
        Ok(serde_json::json!({"protection_rules": []})),
    );
    let c = remote_state::compare(Ok(Some(doc)), Some(SLUG), &host);
    for leaf in [
        "mergeQueue.required",
        "mergeQueue.mergeMethod",
        "reviewException.reviewers.@owner",
        "reviewException.preventSelfReview",
        "customProperties.team",
    ] {
        assert_eq!(state(&c, leaf), FieldState::Unsupported, "{leaf}");
    }
    assert!(reason(&c, "mergeQueue.required").contains("organization repositories"));
    assert!(reason(&c, "reviewException.reviewers.@owner").contains("public repositories"));
    // Unsupported is neither matching nor drifted, and is still a finding
    // for a declared field.
    assert!(c.has_findings());

    // A ruleset API the plan refuses is unsupported too, not drifted.
    let doc = render(&full_remote());
    let mut host = matching(&doc);
    host.answers.insert(
        r("/rules/branches/main"),
        Err(HostError::Unsupported("HTTP 403, a plan limitation".into())),
    );
    let c = remote_state::compare(Ok(Some(doc)), Some(SLUG), &host);
    assert_eq!(state(&c, "mergeQueue.required"), FieldState::Unsupported);
}

#[test]
fn a_check_of_the_right_name_from_the_wrong_app_is_drifted() {
    let doc = render(&full_remote());
    let mut host = matching(&doc);
    host.answers.insert(
        r("/branches/main/protection"),
        Ok(serde_json::json!({
            "required_status_checks": {"contexts": ["ci-gate", "lint"], "checks": [{"context": "ci-gate", "app_id": 1}, {"context": "lint", "app_id": null}]},
            "required_pull_request_reviews": {"require_code_owner_reviews": true},
        })),
    );
    let c = remote_state::compare(Ok(Some(doc)), Some(SLUG), &host);
    assert_eq!(state(&c, "requiredChecks.ci-gate"), FieldState::Drifted);
    assert!(reason(&c, "requiredChecks.ci-gate").contains("not bound to app id"));
    // The undeclared required check is observed extra state.
    assert!(
        c.extras
            .iter()
            .any(|e| e.field == "requiredChecks" && e.observed["name"] == "lint")
    );
}

#[test]
fn an_extra_job_needs_both_its_declaration_and_its_aggregation() {
    let doc = render(&full_remote());
    let mut host = matching(&doc);
    host.answers.insert(
        r("/contents/.github/workflows/statecraft-ci.yml?ref=main"),
        Ok(contents(
            "jobs:\n  platform:\n    uses: ./.github/workflows/platform.yml\n  ci-gate:\n    needs: [governance, code]\n",
        )),
    );
    let c = remote_state::compare(Ok(Some(doc)), Some(SLUG), &host);
    assert_eq!(state(&c, "extraRequiredJobs.platform"), FieldState::Drifted);
    assert!(reason(&c, "extraRequiredJobs.platform").contains("needed by ci-gate: false"));
}

#[test]
fn each_declared_reviewer_is_compared_separately() {
    let doc = render(&full_remote());
    let mut host = matching(&doc);
    host.answers.insert(
        r(&format!("/environments/{}", setup::EXCEPTION_ENVIRONMENT)),
        Ok(serde_json::json!({"protection_rules": [{"type": "required_reviewers", "prevent_self_review": false, "reviewers": [
            {"type": "User", "reviewer": {"login": "owner"}},
        ]}]})),
    );
    let c = remote_state::compare(Ok(Some(doc)), Some(SLUG), &host);
    assert_eq!(
        state(&c, "reviewException.reviewers.@owner"),
        FieldState::Matching
    );
    assert_eq!(
        state(&c, "reviewException.reviewers.@owner/release"),
        FieldState::Drifted
    );
    assert_eq!(
        state(&c, "reviewException.preventSelfReview"),
        FieldState::Drifted
    );
    assert_eq!(
        state(&c, "reviewException.environment"),
        FieldState::Matching
    );
}

#[test]
fn a_missing_property_schema_is_distinct_from_a_missing_value() {
    let doc = render(&full_remote());
    let mut host = matching(&doc);
    host.answers
        .insert(r("/properties/values"), Ok(serde_json::json!([])));
    let c = remote_state::compare(Ok(Some(doc.clone())), Some(SLUG), &host);
    assert_eq!(state(&c, "customProperties.team"), FieldState::Drifted);
    assert!(reason(&c, "customProperties.team").contains("repository has no value"));

    host.answers.insert(
        "orgs/owner/properties/schema".into(),
        Ok(serde_json::json!([])),
    );
    let c = remote_state::compare(Ok(Some(doc)), Some(SLUG), &host);
    assert!(
        reason(&c, "customProperties.team").contains("organization schema defines no property")
    );
}

#[test]
fn codeowners_literals_respect_root_relative_paths_and_nested_directory_names() {
    for (pattern, path, expected) in [
        ("src/docs", "src/docs/file.md", FieldState::Matching),
        ("src/docs/", "src/docs/file.md", FieldState::Matching),
        ("src/docs", "nested/src/docs/file.md", FieldState::Drifted),
        ("apps/", "nested/apps/src/file.rs", FieldState::Matching),
        ("apps", "nested/apps/src/file.rs", FieldState::Matching),
        ("/apps/", "nested/apps/src/file.rs", FieldState::Drifted),
        ("apps/", "apps", FieldState::Drifted),
        ("apps", "apps", FieldState::Matching),
        ("apps/", "myapps/file.rs", FieldState::Drifted),
        ("/docs/*", "docs/nested/file.md", FieldState::Drifted),
        ("/docs/**", "docs/nested/file.md", FieldState::Matching),
    ] {
        let mut doc = render(&full_remote());
        doc.code_owner_review.paths = vec![path.into()];
        let mut host = matching(&doc);
        host.answers.insert(
            r("/contents/.github/CODEOWNERS?ref=main"),
            Ok(contents(&format!("{pattern} @owner\n"))),
        );
        let c = remote_state::compare(Ok(Some(doc)), Some(SLUG), &host);
        assert_eq!(
            state(&c, "codeOwnerReview.paths"),
            expected,
            "{pattern}: {path}"
        );
    }
}

#[test]
fn codeowners_must_give_every_governed_path_the_declared_owners() {
    let doc = render(&full_remote());
    let mut host = matching(&doc);
    host.answers.insert(
        r("/contents/.github/CODEOWNERS?ref=main"),
        Ok(contents("* @owner\n/.statecraft/ @someone\n")),
    );
    let c = remote_state::compare(Ok(Some(doc.clone())), Some(SLUG), &host);
    assert_eq!(state(&c, "codeOwnerReview.paths"), FieldState::Drifted);
    assert!(reason(&c, "codeOwnerReview.paths").contains(setup::POLICY_PATH));

    // A pattern this reader does not evaluate is unverified, not a guess.
    let host = {
        let mut h = matching(&doc);
        h.answers.insert(
            r("/contents/.github/CODEOWNERS?ref=main"),
            Ok(contents("* @owner\n**/*.json @owner\n")),
        );
        h
    };
    let c = remote_state::compare(Ok(Some(doc.clone())), Some(SLUG), &host);
    assert_eq!(state(&c, "codeOwnerReview.paths"), FieldState::Unverified);

    // No CODEOWNERS anywhere is drifted; the protection setting is compared
    // independently.
    let mut host = matching(&doc);
    host.answers.insert(
        r("/contents/.github/CODEOWNERS?ref=main"),
        Err(HostError::NotFound),
    );
    for loc in ["CODEOWNERS", "docs/CODEOWNERS"] {
        host.answers.insert(
            r(&format!("/contents/{loc}?ref=main")),
            Err(HostError::NotFound),
        );
    }
    let c = remote_state::compare(Ok(Some(doc)), Some(SLUG), &host);
    assert_eq!(state(&c, "codeOwnerReview.paths"), FieldState::Drifted);
    assert_eq!(state(&c, "codeOwnerReview.required"), FieldState::Matching);
}

#[test]
fn a_merge_queue_configured_otherwise_is_drifted_per_leaf() {
    let doc = render(&full_remote());
    let mut host = matching(&doc);
    host.answers.insert(
        r("/rules/branches/main"),
        Ok(serde_json::json!([{"type": "merge_queue", "parameters": {"merge_method": "MERGE", "max_entries_to_build": 5, "grouping_strategy": "HEADGREEN"}}])),
    );
    let c = remote_state::compare(Ok(Some(doc)), Some(SLUG), &host);
    assert_eq!(state(&c, "mergeQueue.required"), FieldState::Matching);
    assert_eq!(
        state(&c, "mergeQueue.buildConcurrency"),
        FieldState::Matching
    );
    assert_eq!(state(&c, "mergeQueue.mergeMethod"), FieldState::Drifted);
    assert_eq!(
        state(&c, "mergeQueue.allEntriesMustPass"),
        FieldState::Drifted
    );
}

#[test]
fn a_document_that_disagrees_with_the_selection_is_a_local_finding_before_any_host() {
    let dir = project();
    let root = dir.path();
    let mut m = manifest();
    let plan = plan_with(root, &m, &BTreeMap::new()).unwrap();
    apply(root, &plan, &mut m);
    assert!(remote_state::read_local(root, &m).unwrap().is_some());

    // The document now claims another profile identity.
    let path = root.join(remote_state::PATH);
    let mut doc = DesiredState::parse(&std::fs::read(&path).unwrap()).unwrap();
    doc.profile.identity = "0".repeat(64);
    std::fs::write(&path, doc.canonical()).unwrap();
    let host = matching(&doc);
    let (results, c) = setup::remote_report(root, &m, &host, None);
    let finding = c.finding.as_deref().expect("a local finding");
    assert!(finding.contains("selected profile"), "{finding}");
    assert!(c.fields.is_empty());
    assert!(c.has_findings());
    assert!(host.asked.borrow().is_empty(), "no host was asked");
    assert_eq!(
        results.required_checks.state,
        setup::ResultState::Unverified
    );

    // Edited bytes the selection did not render are a finding too.
    let mut doc = DesiredState::parse(&std::fs::read(&path).unwrap()).unwrap();
    doc.profile.identity = Profile::registered().identity();
    doc.workflow_token.default_permissions = "write".into();
    std::fs::write(&path, doc.canonical()).unwrap();
    let err = remote_state::read_local(root, &m).unwrap_err();
    assert!(
        err.contains("not the document the selection rendered"),
        "{err}"
    );

    // Absent after the selection rendered it.
    std::fs::remove_file(&path).unwrap();
    assert!(
        remote_state::read_local(root, &m)
            .unwrap_err()
            .contains("absent")
    );
}

#[test]
fn a_selection_from_before_revision_16_has_nothing_to_compare() {
    let dir = project();
    let root = dir.path();
    let mut m = manifest();
    let plan = plan_with(root, &m, &BTreeMap::new()).unwrap();
    apply(root, &plan, &mut m);
    let selection = m.project.setup.as_mut().unwrap();
    selection.revision = 15;
    selection.remote_state = None;
    std::fs::remove_file(root.join(remote_state::PATH)).unwrap();
    assert!(remote_state::read_local(root, &m).unwrap().is_none());
    let host = Fake {
        answers: BTreeMap::new(),
        asked: RefCell::new(Vec::new()),
    };
    let c = remote_state::compare(Ok(None), Some(SLUG), &host);
    assert!(c.note.as_deref().unwrap().contains("revision 16"));
    assert!(!c.has_findings());
    assert!(host.asked.borrow().is_empty());
}

#[test]
fn a_gh_failure_is_classified_from_its_status_and_never_echoes_a_body() {
    use setup::classify_gh_failure as c;
    assert_eq!(c("gh: Not Found (HTTP 404)"), HostError::NotFound);
    assert_eq!(
        c("gh: Resource not accessible by integration (HTTP 403)\n{\"token\":\"ghp_secret\"}"),
        HostError::Unauthorized("HTTP 403".into())
    );
    assert_eq!(
        c(
            "gh: Upgrade to GitHub Pro or make this repository public to enable this feature. (HTTP 403)"
        ),
        HostError::Unsupported("HTTP 403, a plan limitation".into())
    );
    assert_eq!(
        c("gh: Bad credentials (HTTP 401)"),
        HostError::Unauthorized("HTTP 401".into())
    );
    assert_eq!(
        c("gh: Server Error (HTTP 502)"),
        HostError::Unreachable("HTTP 502".into())
    );
    let e = c("error connecting to api.github.com\nAuthorization: token ghp_x");
    assert!(!e.reason().contains("ghp_"), "{e:?}");
}

#[test]
fn without_a_github_origin_every_leaf_is_unverified_and_no_host_is_asked() {
    let doc = render(&full_remote());
    let host = matching(&doc);
    let c = remote_state::compare(Ok(Some(doc)), None, &host);
    assert!(c.fields.iter().all(|f| f.state == FieldState::Unverified));
    assert!(host.asked.borrow().is_empty());
}
