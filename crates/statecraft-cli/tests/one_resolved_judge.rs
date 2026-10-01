//! Spec 029 V-1: one resolved judge.
//!
//! A target pins `=0.23.0` and finds three candidate engines: an incompatible
//! one first on `PATH`, a compatible one at the setup profile's
//! repository-local install and an incompatible repository build. Every verb driven here asks spec-spine
//! through the one resolution, so every governance call lands on the same
//! file, and `doctor` names that file, its rule, its version and its digest.
//! A launcher on `PATH` is asked for its resolution and never run as the
//! judge; an override and an empty candidate set refuse with nothing falling
//! back.

#![cfg(unix)]

#[path = "support/json_naming.rs"]
mod json_naming;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PIN: &str = "0.23.0";
const PLAN: &str = r#"{"blocked":[],"ready":[{"id":"003-approved","status":"approved","title":"approved"}],"schemaVersion":"0.6.0"}"#;
const LIST: &str =
    r#"{"items":[{"id":"003-approved","status":"approved","implementation":"pending"}]}"#;
const FRESH: &str = r#"{"exitCode":0,"outcome":"ok","report":{"index":{"diagnostics":{"byCode":{},"errors":0,"warnings":0},"fresh":true,"unwitnessed":{"allowed":0,"total":0}},"registry":{"fresh":true,"validationPassed":true,"warnings":0}},"schemaVersion":"1.1.0","summary":"check: ok","tool":"spec-spine","verb":"check"}"#;

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let f = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir_all(f.target()).unwrap();
        std::fs::create_dir_all(f.path_bin()).unwrap();
        for args in [
            &["init", "--quiet"][..],
            &["config", "user.name", "fixture"],
            &["config", "user.email", "fixture@example.com"],
        ] {
            f.git(args);
        }
        std::fs::write(
            f.target().join("spec-spine.toml"),
            format!("[meta]\nrequired_version = \"={PIN}\"\n"),
        )
        .unwrap();
        std::fs::create_dir_all(f.target().join("specs")).unwrap();
        f.git(&["add", "spec-spine.toml"]);
        f.git(&["commit", "--quiet", "-m", "base"]);
        f
    }

    fn target(&self) -> PathBuf {
        self.dir.path().join("target-repo")
    }
    fn path_bin(&self) -> PathBuf {
        self.dir.path().join("bin")
    }
    fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }
    fn calls(&self) -> PathBuf {
        self.dir.path().join("calls")
    }
    fn local(&self) -> PathBuf {
        self.target().join(statecraft_home::setup::ENGINE)
    }
    fn build(&self) -> PathBuf {
        self.target().join("target/release/spec-spine")
    }
    fn on_path(&self) -> PathBuf {
        self.path_bin().join("spec-spine")
    }

    fn git(&self, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(self.target())
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}");
    }

    /// A stub engine reporting `version`, whose `check` takes `--json`, and
    /// which records every invocation as `<its path>|<its arguments>`.
    fn engine(&self, at: &Path, version: &str) {
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        let script = format!(
            "#!/bin/sh\nprintf '%s|%s\\n' \"$0\" \"$*\" >> '{calls}'\ncase \"$*\" in\n  \
             --version) echo 'spec-spine {version}' ;;\n  \
             'check --help') echo 'Usage: spec-spine check [OPTIONS]'; echo '      --json  Emit the verdict' ;;\n  \
             'check --json') echo '{FRESH}' ;;\n  \
             'registry plan --json') echo '{PLAN}' ;;\n  \
             'registry list --json') echo '{LIST}' ;;\n  \
             *) echo \"unknown: $*\" >&2; exit 3 ;;\nesac\n",
            calls = self.calls().display()
        );
        statecraft_adapter::fixture::install_script(at, &script, 0o755).unwrap();
    }

    /// A stub launcher: it answers `launcher resolve --json` with `engine`,
    /// records every invocation, and refuses anything else.
    fn launcher(&self, at: &Path, engine: &Path) {
        let envelope = format!(
            r#"{{"exitCode":0,"outcome":"ok","report":{{"digest":null,"lock":"absent","path":"{}","release":"{PIN}","repo":"{}","rule":"store","target":"x86_64-unknown-linux-gnu","trust":"published-digest"}},"schemaVersion":"0.1.0","summary":"engine 0.23.0 resolved by store","tool":"spec-spine-launcher","verb":"launcher.resolve"}}"#,
            engine.display(),
            self.target().display()
        );
        let script = format!(
            "#!/bin/sh\nprintf '%s|%s\\n' \"$0\" \"$*\" >> '{calls}'\ncase \"$*\" in\n  \
             'launcher resolve --json') echo '{envelope}' ;;\n  \
             *) echo 'the launcher is not the judge' >&2; exit 4 ;;\nesac\n",
            calls = self.calls().display()
        );
        statecraft_adapter::fixture::install_script(at, &script, 0o755).unwrap();
    }

    fn cli(&self, args: &[&str], extra: &[(&str, &str)]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", self.home())
            .env("HOME", self.dir.path())
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.path_bin().display()),
            )
            .envs(extra.iter().copied())
            .output()
            .unwrap()
    }

    fn root(&self) -> String {
        self.target().display().to_string()
    }

    /// Register the target, and forget what registration asked.
    fn register(&self) {
        let out = self.cli(&["project", "register", &self.root()], &[]);
        assert!(out.status.code().unwrap() <= 1, "{}", text(&out));
        let _ = std::fs::remove_file(self.calls());
    }

    /// Every recorded invocation as `(program, arguments)`.
    fn invocations(&self) -> Vec<(String, String)> {
        std::fs::read_to_string(self.calls())
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.split_once('|'))
            .map(|(p, a)| (p.to_string(), a.to_string()))
            .collect()
    }

    /// The programs that were asked a governance question: anything but
    /// `--version`, the launcher's resolution query and `check --help`.
    fn judges(&self) -> Vec<String> {
        let mut judges: Vec<String> = self
            .invocations()
            .into_iter()
            .filter(|(_, a)| a != "--version" && !a.starts_with("launcher ") && a != "check --help")
            .map(|(p, _)| p)
            .collect();
        judges.sort();
        judges.dedup();
        judges
    }
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn digest(path: &Path) -> String {
    format!(
        "sha256:{}",
        statecraft_environment::digest::digest_bytes(&std::fs::read(path).unwrap())
    )
}

#[test]
fn every_verb_asks_the_one_resolved_engine_and_doctor_names_it() {
    let f = Fixture::new();
    f.engine(&f.on_path(), "0.24.0");
    f.engine(&f.local(), PIN);
    f.engine(&f.build(), "0.22.0");

    let out = f.cli(&["project", "register", &f.root()], &[]);
    assert!(out.status.code().unwrap() <= 1, "{}", text(&out));
    // Section 3.3: the initialization report names the whole resolution.
    let out = f.cli(&["init", "plan", &f.root(), "--json"], &[]);
    let initialized = text(&out);
    for part in [
        format!("\"program\": \"{}\"", f.local().display()),
        "\"rule\": \"repository-local\"".to_string(),
        format!("\"digest\": \"{}\"", digest(&f.local())),
    ] {
        assert!(initialized.contains(&part), "{part} not in {initialized}");
    }
    let out = f.cli(&["work", "list", &f.root(), "--json"], &[]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let out = f.cli(&["doctor", &f.root(), "--json"], &[]);
    let doctor = text(&out);

    // One judge across every verb: the repository-local install, the first
    // candidate the pin admits. The incompatible PATH engine and repository
    // build answered `--version` at most, and judged nothing.
    assert_eq!(
        f.judges(),
        [f.local().display().to_string()],
        "{:?}",
        f.invocations()
    );

    // doctor names the file, its rule, its version and its digest, and names
    // the PATH engine only as information.
    let named = format!(
        "spec-spine judge: {} (rule repository-local, reports {PIN}, {})",
        f.local().display(),
        digest(&f.local())
    );
    assert!(doctor.contains(&named), "{doctor}");
    assert!(
        doctor.contains(&format!(
            "spec-spine on PATH: {} is not the judge for this project",
            f.on_path().display()
        )),
        "{doctor}"
    );
    assert!(
        !doctor.contains("observed executable 0.24.0"),
        "doctor compared the pin with the PATH engine: {doctor}"
    );
}

#[test]
fn a_launcher_on_path_is_asked_for_its_resolution_and_never_judges() {
    let f = Fixture::new();
    let store = f.dir.path().join("store/0.23.0/spec-spine");
    f.engine(&store, PIN);
    f.launcher(&f.on_path(), &store);
    // A compatible repository-local install exists, and the launcher's answer
    // comes first in the order.
    f.engine(&f.local(), PIN);
    f.register();

    let out = f.cli(&["work", "list", &f.root(), "--json"], &[]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert_eq!(
        f.judges(),
        [store.display().to_string()],
        "{:?}",
        f.invocations()
    );
    let launcher_calls: Vec<String> = f
        .invocations()
        .into_iter()
        .filter(|(p, _)| *p == f.on_path().display().to_string())
        .map(|(_, a)| a)
        .collect();
    assert!(
        launcher_calls
            .iter()
            .all(|a| a == "launcher resolve --json"),
        "{launcher_calls:?}"
    );

    let out = f.cli(&["doctor", &f.root(), "--json"], &[]);
    assert!(
        text(&out).contains("(rule launcher, reports 0.23.0"),
        "{}",
        text(&out)
    );
}

#[test]
fn an_incompatible_override_refuses_and_nothing_falls_back() {
    let f = Fixture::new();
    f.engine(&f.on_path(), PIN);
    let named = f.dir.path().join("named/spec-spine");
    f.engine(&named, "0.24.0");
    let named_s = named.display().to_string();
    f.register();

    let out = f.cli(
        &["work", "list", &f.root(), "--json"],
        &[("STATECRAFT_SPEC_SPINE", named_s.as_str())],
    );
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(
        text(&out).contains(&format!(
            "the override STATECRAFT_SPEC_SPINE={named_s} reports 0.24.0"
        )),
        "{}",
        text(&out)
    );
    assert!(f.judges().is_empty(), "{:?}", f.invocations());
}

#[test]
fn with_no_admitted_candidate_the_refusal_names_the_pin_and_the_preparation() {
    let f = Fixture::new();
    f.engine(&f.on_path(), "0.24.0");
    f.engine(&f.build(), "0.22.0");
    // Registered while a compatible engine existed, which is then removed.
    f.engine(&f.local(), PIN);
    f.register();
    std::fs::remove_file(f.local()).unwrap();

    let out = f.cli(&["work", "list", &f.root(), "--json"], &[]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    let said = text(&out);
    assert!(said.contains("=0.23.0"), "{said}");
    assert!(said.contains("make tools"), "{said}");
    assert!(said.contains("(PATH, reports 0.24.0)"), "{said}");
    assert!(
        said.contains("(repository build, reports 0.22.0)"),
        "{said}"
    );
    assert!(f.judges().is_empty(), "{:?}", f.invocations());
}
