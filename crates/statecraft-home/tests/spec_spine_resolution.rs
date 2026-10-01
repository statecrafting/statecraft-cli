//! Spec 029 sections 3.2, 3.3 and 3.5: the one resolution, driven through its
//! public entry with stub engines at every candidate location.
//!
//! Each case is a row of section 3.6 that the resolution itself decides: which
//! candidate answers, which are passed over and named, when nothing falls back,
//! and what a refusal names. The record every report writes (section 3.3) is
//! read from the same selection.

#![cfg(unix)]

use statecraft_home::spec_spine::{self, PREPARE, Rule, Selection, Unselected};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new(pin: Option<&str>) -> Self {
        let f = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir_all(f.root()).unwrap();
        std::fs::create_dir_all(f.path_dir()).unwrap();
        let toml = match pin {
            Some(p) => format!("[meta]\nrequired_version = \"{p}\"\n"),
            None => "# [meta]\n# required_version = \"=0.23.0\"\n".to_string(),
        };
        std::fs::write(f.root().join("spec-spine.toml"), toml).unwrap();
        f
    }

    fn root(&self) -> PathBuf {
        self.dir.path().join("repo")
    }
    fn path_dir(&self) -> PathBuf {
        self.dir.path().join("bin")
    }
    fn local(&self) -> PathBuf {
        self.root().join(statecraft_home::setup::ENGINE)
    }
    fn build(&self) -> PathBuf {
        self.root().join("target/release/spec-spine")
    }
    fn on_path(&self) -> PathBuf {
        self.path_dir().join("spec-spine")
    }

    /// An engine stub answering `--version` with `version`.
    fn engine(&self, at: &Path, version: &str) {
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 'spec-spine {version}'; fi\nexit 0\n"
        );
        statecraft_adapter::fixture::install_script(at, &script, 0o755).unwrap();
    }

    /// A launcher on `PATH` resolving to `engine`.
    fn launcher(&self, engine: &Path) {
        let envelope = format!(
            r#"{{"exitCode":0,"outcome":"ok","report":{{"path":"{}"}},"summary":"ok","tool":"spec-spine-launcher","verb":"launcher.resolve"}}"#,
            engine.display()
        );
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = launcher ]; then echo '{envelope}'; exit 0; fi\nexit 4\n"
        );
        statecraft_adapter::fixture::install_script(&self.on_path(), &script, 0o755).unwrap();
    }

    fn select(&self, extra: &[(&str, &str)]) -> Selection {
        let mut map: BTreeMap<String, String> = extra
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        map.entry("PATH".to_string())
            .or_insert_with(|| self.path_dir().display().to_string());
        spec_spine::select(&self.root(), &move |name| map.get(name).cloned())
    }
}

fn sha256_of(path: &Path) -> String {
    format!(
        "sha256:{}",
        statecraft_environment::digest::digest_bytes(&std::fs::read(path).unwrap())
    )
}

#[test]
fn the_profile_declared_install_answers_and_an_incompatible_path_binary_is_named() {
    let f = Fixture::new(Some("=0.23.0"));
    f.engine(&f.on_path(), "0.99.0");
    f.engine(&f.local(), "0.23.0");

    let s = f.select(&[]);
    let record = s.record().unwrap_or_else(|| panic!("{s:?}"));
    assert_eq!(record.rule, "repository-local");
    assert_eq!(record.program, f.local().display().to_string());
    assert_eq!(record.version.as_deref(), Some("0.23.0"));
    assert_eq!(record.digest, Some(sha256_of(&f.local())));
    // The repository-local install answered, so PATH was never reached.
    assert!(record.passed_over.is_empty(), "{record:?}");

    // With the local install refused, the build and PATH are tried in order,
    // and each one passed over is named with its version and the pin.
    f.engine(&f.local(), "0.22.0");
    f.engine(&f.build(), "0.23.0");
    let record = f.select(&[]).record().unwrap();
    assert_eq!(record.rule, "repository-build");
    assert_eq!(record.passed_over.len(), 1, "{record:?}");
    assert!(
        record.passed_over[0].contains("reports 0.22.0"),
        "{record:?}"
    );
    assert!(record.passed_over[0].contains("=0.23.0"), "{record:?}");
}

#[test]
fn a_launcher_resolved_path_that_is_not_executable_is_named() {
    let f = Fixture::new(Some("=0.23.0"));
    let missing = f.dir.path().join("gone/spec-spine");
    f.launcher(&missing);

    let s = f.select(&[]);
    assert!(matches!(s.outcome, Err(Unselected::Refused(_))), "{s:?}");
    assert_eq!(s.passed_over.len(), 1, "{s:?}");
    assert!(
        s.passed_over[0].contains(&missing.display().to_string()),
        "{s:?}"
    );
    assert!(s.passed_over[0].contains("not executable"), "{s:?}");
}

#[test]
fn an_incompatible_override_is_refused_and_nothing_falls_back() {
    let f = Fixture::new(Some("=0.23.0"));
    f.engine(&f.on_path(), "0.23.0");
    f.engine(&f.local(), "0.23.0");
    let wrong = f.dir.path().join("elsewhere/spec-spine");
    f.engine(&wrong, "0.21.0");
    let wrong_s = wrong.display().to_string();

    let s = f.select(&[(spec_spine::ENV, &wrong_s)]);
    assert!(matches!(s.outcome, Err(Unselected::Refused(_))), "{s:?}");
    assert!(s.record().is_none());
    assert!(
        s.passed_over.is_empty(),
        "no convention was consulted: {s:?}"
    );

    // A compatible override is the only candidate, and its rule says so.
    f.engine(&wrong, "0.23.0");
    let record = f.select(&[(spec_spine::ENV, &wrong_s)]).record().unwrap();
    assert_eq!(record.rule, "override");
}

#[test]
fn a_managed_session_whose_supervisor_path_is_not_executable_is_refused() {
    let f = Fixture::new(Some("=0.23.0"));
    f.engine(&f.local(), "0.23.0");
    let missing = f.dir.path().join("gone/spec-spine").display().to_string();

    let s = f.select(&[(spec_spine::ENV, &missing), (spec_spine::MANAGED, "run-1")]);
    let Err(Unselected::Refused(why)) = &s.outcome else {
        panic!("{s:?}");
    };
    assert!(why.contains("no other binary is consulted"), "{why}");

    // The supervisor's path is not put to the pin: its identity is the
    // supervisor's resolution.
    let other = f.dir.path().join("supervisor/spec-spine");
    f.engine(&other, "0.1.0");
    let other_s = other.display().to_string();
    let s = f.select(&[(spec_spine::ENV, &other_s), (spec_spine::MANAGED, "run-1")]);
    assert_eq!(s.outcome.unwrap().rule, Rule::Supervisor);
}

#[test]
fn a_launcher_answer_the_pin_does_not_admit_is_passed_over_and_named() {
    let f = Fixture::new(Some("=0.23.0"));
    let store = f.dir.path().join("store/spec-spine");
    f.engine(&store, "0.24.0");
    f.launcher(&store);
    f.engine(&f.local(), "0.23.0");

    let record = f.select(&[]).record().unwrap();
    assert_eq!(record.rule, "repository-local");
    assert!(
        record.passed_over[0].contains("(launcher, reports 0.24.0)"),
        "{record:?}"
    );

    // Admitted, the launcher's engine answers, and the launcher itself is
    // never the program recorded.
    f.engine(&store, "0.23.0");
    let record = f.select(&[]).record().unwrap();
    assert_eq!(record.rule, "launcher");
    assert_eq!(record.program, store.display().to_string());
}

#[test]
fn with_no_candidate_admitted_the_refusal_names_the_pin_the_candidates_and_the_remedy() {
    let f = Fixture::new(Some("=0.23.0"));
    f.engine(&f.local(), "0.20.0");
    f.engine(&f.on_path(), "0.21.0");

    let s = f.select(&[]);
    let why = s.judge().unwrap_err();
    assert!(why.contains("=0.23.0"), "{why}");
    assert!(why.contains(&f.local().display().to_string()), "{why}");
    assert!(why.contains(&f.on_path().display().to_string()), "{why}");
    assert!(why.contains(PREPARE), "{why}");
    // Resolution never acquires: nothing was written anywhere it looked.
    assert!(!f.build().exists());
}

#[test]
fn an_unpinned_repository_takes_the_first_candidate_and_says_so() {
    let f = Fixture::new(None);
    f.engine(&f.local(), "0.1.0");
    f.engine(&f.on_path(), "0.99.0");

    let s = f.select(&[]);
    assert_eq!(s.record().unwrap().rule, "repository-local");
    assert!(s.notices.iter().any(|n| n.contains("is unpinned")), "{s:?}");
}
