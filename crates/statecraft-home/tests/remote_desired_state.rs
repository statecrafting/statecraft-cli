//! Spec 012: the remote desired-state document a setup profile renders, and
//! its parse. The read-only comparison `doctor --remote` makes of it lands
//! with that verb.
//!
//! The tests render through the profile and parse the document; no test
//! reaches a host.

use statecraft_environment::digest::digest_bytes;
use statecraft_environment::manifest::{Manifest, Pins};
use statecraft_home::remote_state::{
    self, DesiredState, ProfileClaim, RemoteParameters, Rendering,
};
use statecraft_home::setup::{self, Inputs, Profile, Role};
use statecraft_home::setup_input;
use std::collections::BTreeMap;
use std::path::Path;

const TOML: &str = "[meta]\nrequired_version = \"=0.26.0\"\n";

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
    assert!(
        revision(13) < revision(16),
        "revision notes stay chronological"
    );
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
