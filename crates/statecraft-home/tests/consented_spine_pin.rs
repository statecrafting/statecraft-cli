//! Spec 033: consent over the full initialization, atomic pin effects and recovery.
#![cfg(unix)]
mod support;

use statecraft_environment::{digest::digest_bytes, judge::JudgeRecord, manifest::Manifest};
use statecraft_home::{
    flow::{self, Context, Corpus, Outcome, SetupRequest},
    producer,
};
use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};
use support::{FixedClock, Sandbox, StatedProbe};

/// A labelled corpus fixture with real readable executable bytes. Failures
/// happen after the authored pin and manifest, without calling a provider.
struct Judge {
    path: PathBuf,
    fail_compile: bool,
}
impl Judge {
    fn new(s: &Sandbox) -> Self {
        let path = s.dir.path().join("judge");
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\necho 'spec-spine {}'\n",
                producer::PRODUCER_VERSION
            ),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            path,
            fail_compile: false,
        }
    }
}
impl Corpus for Judge {
    fn compile(&self, _: &Path) -> Result<String, String> {
        if self.fail_compile {
            Err("stated compile failure".into())
        } else {
            Ok("stated compile".into())
        }
    }
    fn index(&self, _: &Path) -> Result<String, String> {
        Ok("stated index".into())
    }
    fn check(&self, _: &Path) -> Result<String, String> {
        Ok("stated check".into())
    }
    fn version(&self) -> Option<String> {
        Some(producer::PRODUCER_VERSION.into())
    }
    fn judge(&self) -> Option<JudgeRecord> {
        Some(JudgeRecord {
            program: self.path.display().to_string(),
            rule: "stated-fixture".into(),
            version: Some(producer::PRODUCER_VERSION.into()),
            digest: Some(format!(
                "sha256:{}",
                digest_bytes(&std::fs::read(&self.path).unwrap())
            )),
            passed_over: vec![],
        })
    }
}
fn request() -> SetupRequest {
    SetupRequest {
        profile: Some("github-actions-rust".into()),
        spine: Some(format!("={}", producer::PRODUCER_VERSION)),
        ..Default::default()
    }
}
fn run(s: &Sandbox, judge: &dyn Corpus, setup: SetupRequest, apply: bool) -> flow::Report {
    run_with_producer(s, judge, &producer::Library, setup, apply)
}
fn run_with_producer(
    s: &Sandbox,
    judge: &dyn Corpus,
    producer: &dyn producer::Producer,
    setup: SetupRequest,
    apply: bool,
) -> flow::Report {
    let root = s.project();
    let home = s.layout();
    let probe = StatedProbe::default();
    let ctx = Context {
        home: &home,
        root: &root,
        producer,
        corpus: judge,
        target_probe: &probe,
        clock: &FixedClock(1_760_000_000),
        product_version: "test".into(),
        setup,
    };
    if apply {
        flow::apply(&ctx)
    } else {
        flow::plan(&ctx)
    }
}

#[test]
fn changed_producer_or_selected_judge_path_refuses_before_effects() {
    use producer::Producer;
    let s = Sandbox::new();
    let before = source(&s);
    let j = Judge::new(&s);
    let (req, _) = approved(&s, &j);
    let mut changed = producer::Recorded {
        json: producer::Library
            .scaffold(&producer::config_json())
            .unwrap(),
        identity: producer::Library.identity(),
    };
    changed.identity.name = "changed-producer".into();
    unchanged(
        &s,
        &before,
        &run_with_producer(&s, &j, &changed, req.clone(), true),
    );
    let different_path = s.dir.path().join("other-judge");
    std::fs::copy(&j.path, &different_path).unwrap();
    let other = Judge {
        path: different_path,
        fail_compile: false,
    };
    unchanged(&s, &before, &run(&s, &other, req, true));
}

#[test]
fn missing_non_utf8_and_nonregular_configuration_and_missing_judge_refuse_read_only() {
    for kind in ["missing", "non-utf8", "directory", "judge"] {
        let s = Sandbox::new();
        source(&s);
        let j = Judge::new(&s);
        let path = s.project().join("spec-spine.toml");
        match kind {
            "missing" => std::fs::remove_file(&path).unwrap(),
            "non-utf8" => std::fs::write(&path, [0xff]).unwrap(),
            "directory" => {
                std::fs::remove_file(&path).unwrap();
                std::fs::create_dir(&path).unwrap();
            }
            "judge" => std::fs::remove_file(&j.path).unwrap(),
            _ => unreachable!(),
        }
        // A missing executable is represented by the real resolver, rather
        // than the labelled fixture's deliberately readable judge record.
        let selection = statecraft_home::spec_spine::select_exact(
            &s.project(),
            request().spine.as_ref().unwrap(),
            &|name| (name == "STATECRAFT_SPEC_SPINE").then(|| j.path.display().to_string()),
        );
        let tool = statecraft_home::spec_spine::corpus_for(&selection);
        let report = run(&s, tool.as_ref(), request(), false);
        assert_eq!(
            report.outcome,
            Outcome::Refused,
            "{kind}: {}",
            report.render()
        );
        assert!(report.mutations.is_empty());
        assert!(!s.exists(".statecraft"));
        assert!(!s.home().exists());
    }
}

#[test]
fn rust_and_external_checks_survive_the_explicit_pin_move_and_drift_stays_withheld() {
    for external in [false, true] {
        let s = Sandbox::new();
        source(&s);
        let j = Judge::new(&s);
        let mut req = request();
        if external {
            s.write(".github/workflows/project-code.yml", "on: workflow_call\njobs:\n  code:\n    runs-on: ubuntu-latest\n    steps:\n      - run: scripts/project-code.sh\n");
            s.write("scripts/project-code.sh", "#!/bin/sh\nexit 0\n");
            std::fs::set_permissions(
                s.project().join("scripts/project-code.sh"),
                std::fs::Permissions::from_mode(0o755),
            )
            .unwrap();
            let input = s.dir.path().join("setup.json");
            std::fs::write(&input, serde_json::json!({"schema":statecraft_home::setup_input::SCHEMA, "profile":"github-actions-rust", "parameters":{"ci.code":{"kind":"external", "workflow":".github/workflows/project-code.yml", "script":"scripts/project-code.sh"}}}).to_string()).unwrap();
            req.input = Some(input);
        } else {
            s.write("rust-toolchain.toml", "[toolchain]\nchannel=\"1.96.0\"\n");
            s.write("Cargo.lock", "version = 4\n");
        }
        assert!(
            std::process::Command::new("git")
                .args(["add", "."])
                .current_dir(s.project())
                .status()
                .unwrap()
                .success()
        );
        let plan = run(&s, &j, req.clone(), false);
        assert!(
            plan.setup.as_ref().unwrap().withheld.is_none(),
            "{}",
            plan.render()
        );
        req.plan = Some(plan.spine_pin.unwrap().plan_identity);
        let applied = run(&s, &j, req.clone(), true);
        assert_eq!(applied.spine_pin.unwrap().outcome, "written");
        let policy: serde_json::Value =
            serde_json::from_str(&s.read(statecraft_home::setup::POLICY_PATH).unwrap()).unwrap();
        assert_eq!(policy["jobs"]["code"]["required"], true);
        let workflow = s.read(".github/workflows/statecraft-ci.yml").unwrap();
        assert!(workflow.contains(if external {
            "./.github/workflows/project-code.yml"
        } else {
            "run: sh \"${STATECRAFT_GATE:?}\" code"
        }));
        assert_eq!(s.exists("Cargo.lock"), !external);
        assert_eq!(s.exists("rust-toolchain.toml"), !external);
        s.write("scripts/statecraft/install-spec-spine.sh", "# user drift\n");
        req.plan = None;
        let plan = run(&s, &j, req.clone(), false);
        req.plan = Some(plan.spine_pin.unwrap().plan_identity);
        let drifted = run(&s, &j, req, true);
        assert_eq!(
            s.read("scripts/statecraft/install-spec-spine.sh").unwrap(),
            "# user drift\n"
        );
        assert!(!drifted.setup.unwrap().conflicts().is_empty());
    }
}
fn source(s: &Sandbox) -> String {
    let text =
        "# preserve me\r\n[meta] # adopted\r\n  required_version  =  \"=0.1.0\" # keep me\r\n";
    s.write("spec-spine.toml", text);
    text.into()
}
fn approved(s: &Sandbox, j: &dyn Corpus) -> (SetupRequest, flow::Report) {
    let report = run(s, j, request(), false);
    assert_ne!(report.outcome, Outcome::Refused, "{}", report.render());
    let mut req = request();
    req.plan = Some(report.spine_pin.as_ref().unwrap().plan_identity.clone());
    (req, report)
}
fn unchanged(s: &Sandbox, before: &str, report: &flow::Report) {
    assert_eq!(report.outcome, Outcome::Refused, "{}", report.render());
    assert_eq!(s.read("spec-spine.toml").as_deref(), Some(before));
    assert!(
        !s.layout().root().exists(),
        "a refusal must leave the product home absent"
    );
    assert!(report.mutations.iter().all(|m| m.step == "lock"));
}
#[test]
fn consent_preserves_crlf_comments_and_records_only_after_a_verified_write() {
    let s = Sandbox::new();
    let before = source(&s);
    let j = Judge::new(&s);
    let (req, plan) = approved(&s, &j);
    assert!(plan.mutations.is_empty());
    assert!(!s.exists(".statecraft"));
    assert_eq!(s.read("spec-spine.toml").unwrap(), before);
    assert!(
        plan.setup.as_ref().unwrap().withheld.is_some(),
        "missing Rust inputs stay withheld"
    );
    let report = run(&s, &j, req.clone(), true);
    assert_eq!(report.outcome, Outcome::Partial, "{}", report.render());
    assert_eq!(
        s.read("spec-spine.toml").unwrap(),
        before.replace("=0.1.0", &req.spine.unwrap())
    );
    let manifest = Manifest::read(&s.project()).unwrap().unwrap();
    assert_eq!(manifest.pins.spec_spine, producer::PRODUCER_VERSION);
    assert_eq!(
        manifest.spine_pin.as_ref().unwrap().plan_identity,
        req.plan.unwrap()
    );
    assert_eq!(report.spine_pin.unwrap().outcome, "written");
    assert!(!s.exists("Cargo.toml"));
    assert!(!s.exists(".github/workflows/statecraft-ci.yml"));
}
#[test]
fn omitted_changed_and_profile_only_consents_refuse() {
    let s = Sandbox::new();
    let before = source(&s);
    let j = Judge::new(&s);
    let (req, _) = approved(&s, &j);
    let mut absent = req.clone();
    absent.spine = None;
    unchanged(&s, &before, &run(&s, &j, absent, true));
    let mut changed = req.clone();
    changed.spine = Some("=9.9.9".into());
    unchanged(&s, &before, &run(&s, &j, changed, true));
    let mut missing = req.clone();
    missing.plan = None;
    unchanged(&s, &before, &run(&s, &j, missing, true));
    let normal = run(
        &s,
        &j,
        SetupRequest {
            profile: req.profile.clone(),
            ..Default::default()
        },
        false,
    );
    let mut profile = req;
    profile.plan = Some(normal.setup.unwrap().plan_identity);
    unchanged(&s, &before, &run(&s, &j, profile, true));
}
#[test]
fn unrelated_target_and_home_changes_invalidate_the_full_plan() {
    for changed in ["comment", "bridge", "manifest", "home", "judge", "action"] {
        let s = Sandbox::new();
        let before = source(&s);
        let j = Judge::new(&s);
        let (req, _) = approved(&s, &j);
        match changed {
            "comment" => s.write("spec-spine.toml", &(before.clone() + "# later\r\n")),
            "bridge" => s.write("AGENTS.md", "custom root instructions\n"),
            "manifest" => s.write(".statecraft/environment.json", "{}"),
            "home" => {
                std::fs::create_dir_all(s.layout().root()).unwrap();
                std::fs::write(s.layout().personal_file(), "{}").unwrap();
            }
            "judge" => {
                std::fs::write(&j.path, "changed readable bytes").unwrap();
            }
            "action" => s.write("standards/spec/contract.md", "new adopted contract\n"),
            _ => unreachable!(),
        }
        let actual = s.read("spec-spine.toml").unwrap();
        let report = run(&s, &j, req, true);
        assert_eq!(
            report.outcome,
            Outcome::Refused,
            "{changed}: {}",
            report.render()
        );
        assert_eq!(s.read("spec-spine.toml").unwrap(), actual);
        assert!(report.mutations.iter().all(|m| m.step == "lock"));
    }
}
#[test]
fn unsupported_sources_requests_missing_profile_and_symlinks_refuse_read_only() {
    for text in [
        "[meta]\nrequired_version='=0.1.0'\n",
        "[meta]\nrequired_version=\"=00.1.0\"\n",
        "meta.required_version=\"=0.1.0\"\n",
        "meta={required_version=\"=0.1.0\"}\n",
        "[meta]\nrequired_version=\"=0.1.0\"\nrequired_version=\"=0.1.0\"\n",
        "[meta]\nrequired_version=\"\"\"=0.1.0\"\"\"\n",
        "[meta]\nrequired_version=\"=0.\\u0031.0\"\n",
        "[meta]\n",
        "garbage",
    ] {
        let s = Sandbox::new();
        s.write("spec-spine.toml", text);
        let j = Judge::new(&s);
        unchanged(&s, text, &run(&s, &j, request(), false));
        assert!(!s.exists(".statecraft"));
    }
    let s = Sandbox::new();
    let before = source(&s);
    let j = Judge::new(&s);
    for value in [
        "0.28.0",
        "=01.2.3",
        "=1.2",
        "=1.2.3-beta",
        "=1.2.3+build",
        " =1.2.3",
        "=1.2.3 ",
        "=9.9.9",
    ] {
        let mut req = request();
        req.spine = Some(value.into());
        unchanged(&s, &before, &run(&s, &j, req, false));
    }
    let mut req = request();
    req.profile = None;
    unchanged(&s, &before, &run(&s, &j, req, false));
    std::fs::remove_file(s.project().join("spec-spine.toml")).unwrap();
    std::os::unix::fs::symlink(&j.path, s.project().join("spec-spine.toml")).unwrap();
    let report = run(&s, &j, request(), false);
    assert_eq!(report.outcome, Outcome::Refused);
    assert!(!s.exists(".statecraft"));
}
#[test]
fn same_pin_is_noop_but_still_needs_consent_and_a_new_identity_after_progress() {
    let s = Sandbox::new();
    source(&s);
    let j = Judge::new(&s);
    s.write(
        "spec-spine.toml",
        &format!(
            "[meta]\nrequired_version = \"={}\" # same\n",
            producer::PRODUCER_VERSION
        ),
    );
    let bytes = std::fs::read(s.project().join("spec-spine.toml")).unwrap();
    let (req, plan) = approved(&s, &j);
    assert!(!plan.spine_pin.unwrap().changes);
    let report = run(&s, &j, req.clone(), true);
    assert_eq!(report.spine_pin.unwrap().outcome, "unchanged");
    assert_eq!(
        std::fs::read(s.project().join("spec-spine.toml")).unwrap(),
        bytes
    );
    assert!(!report.mutations.iter().any(|m| m.path == "spec-spine.toml"));
    let stale = run(&s, &j, req, true);
    assert_eq!(stale.outcome, Outcome::Refused, "{}", stale.render());
    let (fresh, _) = approved(&s, &j);
    let again = run(&s, &j, fresh, true);
    assert_eq!(again.spine_pin.unwrap().outcome, "unchanged");
}
#[test]
fn pin_stage_failure_stops_home_and_manifest_writes() {
    let s = Sandbox::new();
    let before = source(&s);
    let j = Judge::new(&s);
    // The manifest lock has its own writable directory; the pin cannot stage
    // in its read-only parent. This is a real filesystem failure, not a hook.
    std::fs::create_dir_all(s.project().join(".statecraft/state")).unwrap();
    let (req, _) = approved(&s, &j);
    std::fs::set_permissions(s.project(), std::fs::Permissions::from_mode(0o555)).unwrap();
    let report = run(&s, &j, req, true);
    std::fs::set_permissions(s.project(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(report.outcome, Outcome::Failed, "{}", report.render());
    assert_eq!(report.spine_pin.unwrap().outcome, "not-written");
    assert_eq!(s.read("spec-spine.toml").unwrap(), before);
    assert!(!s.layout().root().exists());
    assert!(!s.exists(".statecraft/environment.json"));
}
#[test]
fn later_corpus_finding_reports_completed_pin_and_recovers_with_fresh_same_pin_consent() {
    let s = Sandbox::new();
    source(&s);
    let mut j = Judge::new(&s);
    let (req, _) = approved(&s, &j);
    j.fail_compile = true;
    let failed = run(&s, &j, req.clone(), true);
    assert_eq!(failed.outcome, Outcome::Partial, "{}", failed.render());
    assert_eq!(failed.spine_pin.unwrap().outcome, "written");
    assert_eq!(
        Manifest::read(&s.project())
            .unwrap()
            .unwrap()
            .pins
            .spec_spine,
        producer::PRODUCER_VERSION
    );
    let stale = run(&s, &j, req, true);
    assert_eq!(stale.outcome, Outcome::Refused);
    j.fail_compile = false;
    let (fresh, _) = approved(&s, &j);
    let recovered = run(&s, &j, fresh, true);
    assert_eq!(recovered.spine_pin.as_ref().unwrap().outcome, "unchanged");
    assert_ne!(recovered.outcome, Outcome::Failed, "{}", recovered.render());
}

#[test]
fn manifest_recording_failure_keeps_the_written_pin_and_requires_fresh_recovery() {
    let s = Sandbox::new();
    source(&s);
    let j = Judge::new(&s);
    let initial = run(
        &s,
        &j,
        SetupRequest {
            profile: Some("github-actions-rust".into()),
            ..Default::default()
        },
        true,
    );
    assert_ne!(initial.outcome, Outcome::Failed, "{}", initial.render());
    let old = std::fs::read(s.project().join(".statecraft/environment.json")).unwrap();
    let (req, _) = approved(&s, &j);
    std::fs::set_permissions(
        s.project().join(".statecraft"),
        std::fs::Permissions::from_mode(0o555),
    )
    .unwrap();
    let failed = run(&s, &j, req.clone(), true);
    std::fs::set_permissions(
        s.project().join(".statecraft"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    assert_eq!(failed.outcome, Outcome::Failed, "{}", failed.render());
    let pin = failed.spine_pin.as_ref().unwrap();
    assert_eq!(pin.outcome, "written");
    assert!(!pin.recorded);
    assert_eq!(
        std::fs::read(s.project().join(".statecraft/environment.json")).unwrap(),
        old
    );
    assert!(
        s.read("spec-spine.toml")
            .unwrap()
            .contains(&format!("={}", producer::PRODUCER_VERSION))
    );
    assert_eq!(run(&s, &j, req, true).outcome, Outcome::Refused);
    let (fresh, _) = approved(&s, &j);
    let recovered = run(&s, &j, fresh, true);
    assert_ne!(recovered.outcome, Outcome::Failed, "{}", recovered.render());
    assert_eq!(recovered.spine_pin.as_ref().unwrap().outcome, "unchanged");
    assert!(recovered.spine_pin.unwrap().recorded);
    assert_eq!(
        Manifest::read(&s.project())
            .unwrap()
            .unwrap()
            .pins
            .spec_spine,
        producer::PRODUCER_VERSION
    );
}

#[test]
fn changed_setup_input_and_new_action_options_need_new_consent() {
    let s = Sandbox::new();
    let before = source(&s);
    let j = Judge::new(&s);
    let input = s.dir.path().join("input.json");
    let document = serde_json::json!({"schema":"statecraft/setup-input/1", "profile":"github-actions-rust", "parameters":{}});
    std::fs::write(&input, document.to_string()).unwrap();
    let mut req = request();
    req.input = Some(input.clone());
    let planned = run(&s, &j, req.clone(), false);
    req.plan = Some(planned.spine_pin.unwrap().plan_identity);
    std::fs::write(&input, format!("{}\n", document)).unwrap();
    unchanged(&s, &before, &run(&s, &j, req, true));
    let (mut req, _) = approved(&s, &j);
    req.verify_local = true;
    unchanged(&s, &before, &run(&s, &j, req, true));
}
#[test]
fn exact_resolution_keeps_exclusive_override_and_supervisor_and_never_acquires() {
    use statecraft_home::spec_spine::{self, ENV, MANAGED, Rule};
    let s = Sandbox::new();
    source(&s);
    let good = Judge::new(&s);
    let request = request().spine.unwrap();
    let bad = s.dir.path().join("bad");
    std::fs::write(&bad, "#!/bin/sh\necho 'spec-spine 9.9.9'\n").unwrap();
    std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::create_dir(s.project().join(".bin")).unwrap();
    std::fs::copy(&good.path, s.project().join(".bin/spec-spine")).unwrap();
    for managed in [false, true] {
        let selection = spec_spine::select_exact(&s.project(), &request, &|name| match name {
            ENV => Some(bad.display().to_string()),
            MANAGED if managed => Some("fixture".into()),
            _ => None,
        });
        assert!(selection.outcome.is_err());
        assert!(
            selection.considered.is_empty(),
            "exclusive means no fallback"
        );
    }
    let selected = spec_spine::select_exact(&s.project(), &request, &|_| None);
    assert_eq!(selected.judge().unwrap().rule, Rule::RepositoryLocal);
    assert_eq!(s.read("spec-spine.toml").unwrap(), source(&s));
    assert!(!s.exists(".statecraft"));
}
#[test]
fn rendered_installer_reads_supported_comment_crlf_forms_and_refuses_noncanonical_versions() {
    let s = Sandbox::new();
    let j = Judge::new(&s);
    std::fs::create_dir(s.project().join(".bin")).unwrap();
    std::fs::copy(&j.path, s.project().join(".bin/spec-spine")).unwrap();
    let script = s.dir.path().join("installer.sh");
    let profile = statecraft_home::setup::Profile::registered();
    let template = profile
        .templates
        .iter()
        .find(|t| t.path == "scripts/statecraft/install-spec-spine.sh")
        .unwrap();
    std::fs::write(
        &script,
        template
            .body
            .replace("{{sc:profile.revision}}", &profile.revision.to_string()),
    )
    .unwrap();
    for text in [
        format!(
            "[meta] # table\r\n  required_version = \"={}\" # value\r\n",
            producer::PRODUCER_VERSION
        ),
        format!(
            " [ meta ] # table\nrequired_version=\"={}\"\n",
            producer::PRODUCER_VERSION
        ),
    ] {
        s.write("spec-spine.toml", &text);
        let out = std::process::Command::new("sh")
            .arg(&script)
            .current_dir(s.project())
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    for value in [
        "=01.2.3",
        "=1.2.3-beta",
        "=1.2.3+build",
        "=1.2.3.4",
        ">=1.2.3",
        "=1.2.3 ",
    ] {
        s.write(
            "spec-spine.toml",
            &format!("[meta]\nrequired_version=\"{value}\"\n"),
        );
        let out = std::process::Command::new("sh")
            .arg(&script)
            .current_dir(s.project())
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(2),
            "{value}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
