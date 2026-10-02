//! Spec 032: applicability and plan consent for required external code.
#![cfg(unix)]

use statecraft_environment::manifest::{Manifest, Pins};
use statecraft_home::setup::{self, Inputs, Profile};
use statecraft_home::setup_input::Bound;
use std::collections::BTreeMap;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;
use std::process::Command;

const TOML: &str = "[meta]\nrequired_version = \"=0.28.0\"\n";
const WORKFLOW: &str = ".github/workflows/project-code.yml";
const SCRIPT: &str = "scripts/project-code.sh";

fn git(root: &Path, args: &[&str]) {
    let result = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "--quiet"]);
    std::fs::create_dir_all(root.join(".github/workflows")).unwrap();
    std::fs::create_dir_all(root.join("scripts")).unwrap();
    std::fs::write(root.join(WORKFLOW), "on:\n  workflow_call:\njobs:\n  check:\n    runs-on: ubuntu-latest\n    steps:\n      - run: scripts/project-code.sh\n").unwrap();
    std::fs::write(root.join(SCRIPT), "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(root.join(SCRIPT), std::fs::Permissions::from_mode(0o755)).unwrap();
    git(root, &["add", WORKFLOW, SCRIPT]);
    dir
}

fn block() -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([(
        "ci.code".into(),
        serde_json::json!({"kind":"external", "workflow":WORKFLOW, "script":SCRIPT}),
    )])
}

fn manifest() -> Manifest {
    Manifest::new(Pins {
        product: "0.0.0".into(),
        spec_spine: "unpinned".into(),
        adapters: Default::default(),
        producer: None,
    })
}

fn plan(root: &Path, params: &BTreeMap<String, serde_json::Value>) -> Result<setup::Plan, String> {
    setup::plan(&Inputs {
        root,
        profile: &Profile::registered(),
        block: params,
        manifest: &manifest(),
        spec_spine_toml: Some(TOML),
        derived_dir: ".statecraft/derived",
        bound: &Bound::with_producer("0.28.0"),
    })
}

#[test]
fn an_external_project_needs_real_checks_and_no_invented_rust_inputs() {
    let dir = project();
    let root = dir.path();
    let mut params = block();
    params.insert(
        "review.code_owners".into(),
        serde_json::json!(["@bartekus"]),
    );
    let plan = plan(root, &params).unwrap();
    assert!(plan.withheld.is_none(), "{:?}", plan.prerequisites);
    assert!(!root.join("Cargo.lock").exists());
    assert!(!root.join("rust-toolchain.toml").exists());
    assert_eq!(plan.commands["code"], serde_json::json!([[SCRIPT]]));
    let mut manifest = manifest();
    setup::apply(
        root,
        &plan,
        &mut manifest,
        "2026-10-02T00:00:00Z",
        &mut |p, b| {
            std::fs::create_dir_all(p.parent().unwrap())?;
            std::fs::write(p, b)
        },
    )
    .unwrap();
    setup::finish(root).unwrap();
    let owners = std::fs::read_to_string(root.join(".github/CODEOWNERS")).unwrap();
    assert!(
        owners.contains(&format!("/{WORKFLOW} @bartekus")),
        "{owners}"
    );
    assert!(owners.contains(&format!("/{SCRIPT} @bartekus")), "{owners}");
    let wf = std::fs::read(root.join(".github/workflows/statecraft-ci.yml")).unwrap();
    let parsed: serde_yaml::Value = serde_yaml::from_slice(&wf).unwrap();
    assert_eq!(parsed["jobs"]["code"]["uses"], format!("./{WORKFLOW}"));
    assert!(parsed["jobs"]["code"]["if"].is_null());
    assert!(parsed["jobs"]["code"]["secrets"].is_null());
    let policy: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join(setup::POLICY_PATH)).unwrap()).unwrap();
    for event in ["pull_request", "push", "merge_group"] {
        assert_eq!(policy["jobs"]["code"][event], "required");
    }
    assert_eq!(policy["jobs"]["code"]["required"], true);
    assert!(
        parsed["jobs"]["ci-gate"]["needs"]
            .as_sequence()
            .unwrap()
            .contains(&serde_yaml::Value::String("code".into()))
    );
}

#[test]
fn missing_untracked_nonexecutable_and_symlink_checks_withhold_every_profile_file() {
    for mutation in [
        "missing",
        "untracked",
        "nonexecutable",
        "symlink",
        "workflow-link",
        "workflow-missing",
    ] {
        let dir = project();
        let root = dir.path();
        match mutation {
            "workflow-missing" => std::fs::remove_file(root.join(WORKFLOW)).unwrap(),
            "missing" => std::fs::remove_file(root.join(SCRIPT)).unwrap(),
            "untracked" => git(root, &["rm", "--cached", SCRIPT]),
            "nonexecutable" => {
                std::fs::set_permissions(root.join(SCRIPT), std::fs::Permissions::from_mode(0o644))
                    .unwrap()
            }
            "symlink" => {
                std::fs::remove_file(root.join(SCRIPT)).unwrap();
                symlink("/bin/true", root.join(SCRIPT)).unwrap();
            }
            "workflow-link" => {
                let old = std::fs::read(root.join(WORKFLOW)).unwrap();
                let other = tempfile::NamedTempFile::new().unwrap();
                std::fs::write(other.path(), old).unwrap();
                std::fs::remove_file(root.join(WORKFLOW)).unwrap();
                symlink(other.path(), root.join(WORKFLOW)).unwrap();
                let p = plan(root, &block()).unwrap();
                assert!(p.withheld.is_some());
                continue;
            }
            _ => unreachable!(),
        }
        let p = plan(root, &block()).unwrap();
        assert!(p.withheld.is_some(), "{mutation}");
        assert!(p.files.iter().all(|f| !f.action.writes()), "{mutation}");
    }
}

#[test]
fn escaping_or_incomplete_selections_refuse_planning() {
    let dir = project();
    for value in [
        serde_json::json!({"kind":"external"}),
        serde_json::json!({"kind":"external","workflow":WORKFLOW,"script":"../outside.sh"}),
        serde_json::json!({"kind":"external","workflow":WORKFLOW,"script":"scripts/a';true"}),
        serde_json::json!({"kind":"external","workflow":WORKFLOW,"script":"scripts/statecraft/gate.sh"}),
        serde_json::json!({"kind":"rust","script":SCRIPT}),
        serde_json::json!({"kind":"none"}),
    ] {
        assert!(
            plan(
                dir.path(),
                &BTreeMap::from([("ci.code".into(), value.clone())])
            )
            .is_err(),
            "{value}"
        );
    }
}

#[test]
fn changed_external_input_bytes_invalidate_a_previously_consented_plan() {
    let dir = project();
    let before = plan(dir.path(), &block()).unwrap().plan_identity;
    std::fs::write(dir.path().join(SCRIPT), "#!/bin/sh\nexit 1\n").unwrap();
    let changed_script = plan(dir.path(), &block()).unwrap().plan_identity;
    assert_ne!(before, changed_script);
    std::fs::write(dir.path().join(WORKFLOW), "on: workflow_call\njobs:\n  code:\n    runs-on: ubuntu-latest\n    steps:\n      - run: false\n").unwrap();
    let changed_workflow = plan(dir.path(), &block()).unwrap().plan_identity;
    assert_ne!(changed_script, changed_workflow);
}

#[test]
fn workflow_call_comments_and_string_literals_are_not_reusable_events() {
    let dir = project();
    for text in [
        "# on: workflow_call\non: push\njobs: {}\n",
        "on: push\ncomment: 'on: workflow_call'\njobs: {}\n",
        "on: workflow_call\njobs: {}\n",
        "not: [valid: yaml",
    ] {
        std::fs::write(dir.path().join(WORKFLOW), text).unwrap();
        assert!(plan(dir.path(), &block()).is_err(), "{text}");
    }
}
