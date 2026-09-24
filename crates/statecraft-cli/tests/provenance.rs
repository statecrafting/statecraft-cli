//! Provenance of what initialization writes: spec 002 section 5, the
//! 2026-09-24 entry that amends sections 3.2, 3.3, 3.5, 3.6 and 3.15, through
//! the built binary on real directories with an isolated `HOME`.
//!
//! The `spec-spine` on `PATH` is a stub whose `--version` answer is a file
//! beside it, so what the executable says and what the project declares can be
//! made to disagree on purpose. The governance files come from the real linked
//! library.

#![cfg(unix)]

use std::path::PathBuf;
use std::process::{Command, Output};

const STUB: &str = r#"#!/bin/sh
here="$(dirname "$0")"
case "$*" in
  --version) echo "spec-spine $(/bin/cat "$here/version")" ;;
  'check --help') exit 0 ;;
  check|compile|index) exit 0 ;;
  *) exit 3 ;;
esac
"#;

const CONFIG: &str = "spec-spine.toml";
const CONSTITUTION: &str = "standards/spec/constitution.md";
const CONTRACT: &str = "standards/spec/contract.md";
const BOOTSTRAP: &str = "specs/000-bootstrap/spec.md";
const TEMPLATE: &str = "standards/spec/templates/spec-template.md";

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new(executable_version: &str) -> Self {
        let f = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir_all(f.bin()).unwrap();
        std::fs::create_dir_all(f.project()).unwrap();
        for args in [
            vec!["init", "--quiet", "--initial-branch=main"],
            vec!["config", "user.email", "fixture@example.invalid"],
            vec!["config", "user.name", "fixture"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["commit", "--quiet", "--allow-empty", "-m", "base"],
        ] {
            let out = Command::new("git")
                .args(&args)
                .current_dir(f.project())
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}");
        }
        statecraft_adapter::fixture::install_script(&f.bin().join("spec-spine"), STUB, 0o755)
            .unwrap();
        f.set_version(executable_version);
        f
    }

    fn set_version(&self, v: &str) {
        std::fs::write(self.bin().join("version"), v).unwrap();
    }

    fn bin(&self) -> PathBuf {
        self.dir.path().join("bin")
    }
    fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }
    fn project(&self) -> PathBuf {
        self.dir.path().join("project")
    }
    fn at(&self, rel: &str) -> PathBuf {
        self.project().join(rel)
    }
    fn root(&self) -> String {
        self.project().display().to_string()
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", self.home())
            .env("STATECRAFT_NATIVE_ROOT", self.dir.path().join("native"))
            .env("HOME", self.dir.path())
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin().display()))
            .env("USER", "fixture-operator")
            .output()
            .unwrap()
    }

    fn json(&self, args: &[&str]) -> (i32, serde_json::Value) {
        let mut args = args.to_vec();
        args.push("--json");
        let out = self.cli(&args);
        let value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
            panic!(
                "{e}: {}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            )
        });
        (out.status.code().expect("exited by itself"), value)
    }

    fn init(&self) -> (i32, serde_json::Value) {
        let (exit, v) = self.json(&["init", "apply", &self.root()]);
        (exit, v["value"]["value"].clone())
    }

    fn doctor(&self) -> (i32, serde_json::Value) {
        let (exit, v) = self.json(&["doctor", &self.root()]);
        (exit, v["value"].clone())
    }

    fn declaration(&self) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(self.at(".statecraft/environment.json")).unwrap())
            .unwrap()
    }

    fn edit(&self, rel: &str, f: impl FnOnce(String) -> String) {
        let text = std::fs::read_to_string(self.at(rel)).unwrap();
        let edited = f(text.clone());
        assert_ne!(text, edited, "the edit changed {rel}");
        std::fs::write(self.at(rel), edited).unwrap();
    }
}

fn state_of(report: &serde_json::Value, path: &str) -> String {
    report["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("{report}"))
        .iter()
        .find(|e| e["path"] == path)
        .unwrap_or_else(|| panic!("{path} not in {report}"))["state"]
        .as_str()
        .unwrap()
        .to_string()
}

fn lines(report: &serde_json::Value, key: &str) -> Vec<String> {
    report[key]
        .as_array()
        .map(|a| a.iter().map(|l| l.as_str().unwrap().to_string()).collect())
        .unwrap_or_default()
}

fn entry(declaration: &serde_json::Value, path: &str) -> serde_json::Value {
    declaration["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["path"] == path)
        .cloned()
        .unwrap_or_else(|| panic!("{path} not recorded"))
}

/// Items 1, 3 and 4 on a fresh project: roles from the closed list, the one
/// producer identity from the build, and a pin that is what the project
/// declares (nothing, today) and never what `PATH` answered.
#[test]
fn a_fresh_initialization_records_roles_the_linked_producer_and_the_declared_pin() {
    let f = Fixture::new("0.23.0");
    let (exit, report) = f.init();
    assert_eq!(exit, 0, "{report}");
    assert_eq!(report["outcome"], "complete", "{report}");

    // The observation is reported, and named as one.
    let observed = &report["observedSpecSpine"];
    assert_eq!(observed["version"], "0.23.0", "{report}");
    assert_eq!(observed["foundBy"], "path");
    assert_eq!(observed["program"], "spec-spine");

    let d = f.declaration();
    // The producer's scaffold writes the pin commented out, so the project
    // declares none; `PATH`'s 0.23.0 is not a pin.
    assert_eq!(d["pins"]["spec_spine"], "unpinned", "{d}");
    let producer = &d["pins"]["producer"];
    assert_eq!(producer["name"], "spec-spine-core");
    assert_eq!(
        producer["version"],
        statecraft_home::producer::PRODUCER_VERSION
    );
    assert_eq!(
        producer["checksum"],
        statecraft_home::producer::PRODUCER_CHECKSUM
    );

    for path in [CONFIG, CONSTITUTION, CONTRACT, BOOTSTRAP] {
        let e = entry(&d, path);
        assert_eq!(e["class"], "managed", "{e}");
        assert_eq!(e["role"], "authored-input", "{e}");
    }
    for path in [TEMPLATE, ".statecraft/AGENTS.md"] {
        let e = entry(&d, path);
        assert!(e.get("role").is_none(), "a reference omits the role: {e}");
    }
}

/// Item 2 and the bundle proposal's case 8: an edited pin line and date in
/// authored inputs read `customized` and are not findings; an edited template
/// reads `drifted` and is one. Re-running the initialization rewrites neither
/// authored input.
#[test]
fn edited_authored_inputs_are_customized_information_and_an_edited_template_is_drifted() {
    let f = Fixture::new("0.25.0");
    assert_eq!(f.init().0, 0);
    let (before_exit, before) = f.doctor();
    assert_eq!(state_of(&before, CONFIG), "seeded", "{before}");

    let pin = format!(
        "required_version = \"={}\"",
        statecraft_home::producer::PRODUCER_VERSION
    );
    // The scaffold leaves the whole `[meta]` table commented, so pinning is
    // adding it.
    f.edit(CONFIG, |t| format!("{t}\n[meta]\n{pin}\n"));
    f.edit(CONSTITUTION, |t| format!("{t}\nAmended 2026-09-24.\n"));
    let config_bytes = std::fs::read(f.at(CONFIG)).unwrap();

    let (exit, report) = f.doctor();
    assert_eq!(exit, before_exit, "{report}");
    assert_eq!(
        lines(&report, "findings"),
        lines(&before, "findings"),
        "customized authored inputs add no finding"
    );
    assert_eq!(state_of(&report, CONFIG), "customized", "{report}");
    assert_eq!(state_of(&report, CONSTITUTION), "customized");
    assert_eq!(state_of(&report, CONTRACT), "seeded");
    let findings = lines(&report, "findings");
    assert!(
        !findings
            .iter()
            .any(|l| l.contains(CONFIG) || l.contains(CONSTITUTION)),
        "{findings:?}"
    );
    assert!(
        lines(&report, "notes")
            .iter()
            .any(|l| l.contains("not") || l.contains("control")),
        "{report}"
    );

    // A re-run keeps both, reports them as kept rather than withheld, and now
    // records the pin the project declares.
    let (exit, rerun) = f.init();
    assert_eq!(exit, 0, "{rerun}");
    assert_eq!(rerun["outcome"], "complete", "{rerun}");
    let kept = lines(&rerun, "kept");
    assert!(
        kept.iter().any(|l| l.starts_with(&format!(
            "customized {CONFIG}, authored input, never rewritten"
        ))),
        "{kept:?}"
    );
    assert!(
        !lines(&rerun, "withheld").iter().any(|l| l.contains(CONFIG)),
        "{rerun}"
    );
    assert_eq!(std::fs::read(f.at(CONFIG)).unwrap(), config_bytes);
    assert_eq!(
        f.declaration()["pins"]["spec_spine"],
        statecraft_home::producer::PRODUCER_VERSION
    );

    // A template is a reference: an edit is drift, and a finding.
    f.edit(TEMPLATE, |t| format!("{t}\nlocal edit\n"));
    let (exit, report) = f.doctor();
    assert_eq!(state_of(&report, TEMPLATE), "drifted", "{report}");
    assert_eq!(exit, 1);
}

/// Item 4 and case 6's rows that need no bundle: the declared pin, the
/// producer identity and the observed executable disagreeing are distinct
/// reports, each naming both values. The executable is a finding; the
/// producer is information, because a project may move its own pin.
#[test]
fn a_declared_pin_the_producer_and_the_executable_disagreeing_are_distinct_reports() {
    let f = Fixture::new("0.23.0");
    assert_eq!(f.init().0, 0);
    f.edit(CONFIG, |t| {
        format!("{t}\n[meta]\nrequired_version = \"=0.24.0\"\n")
    });
    assert_eq!(f.init().0, 0);
    assert_eq!(f.declaration()["pins"]["spec_spine"], "0.24.0");

    let (exit, report) = f.doctor();
    assert_eq!(exit, 1, "{report}");
    let findings = lines(&report, "findings");
    assert!(
        findings
            .contains(&"pin spec-spine: declared 0.24.0, observed executable 0.23.0".to_string()),
        "{findings:?}"
    );
    let producer = format!(
        "pin spec-spine: declared 0.24.0, producer identity spec-spine-core@{}",
        statecraft_home::producer::PRODUCER_VERSION
    );
    assert!(lines(&report, "notes").contains(&producer), "{report}");
    assert!(
        !findings.iter().any(|l| l.contains("producer identity")),
        "{findings:?}"
    );
}

/// Item 3: a declaration written before provenance is read as it is. No
/// producer is guessed, the entries read as references, and the report says
/// the producer was recorded before provenance.
#[test]
fn a_declaration_recorded_before_provenance_is_read_without_guessed_values() {
    let f = Fixture::new("0.20.0");
    std::fs::create_dir_all(f.at(".statecraft")).unwrap();
    std::fs::write(f.at("notes.md"), "n").unwrap();
    let digest = statecraft_environment::digest::digest_bytes(b"n");
    let old = serde_json::json!({
        "version": 2,
        "pins": { "product": "0.0.0", "spec_spine": "0.20.0", "adapters": {} },
        "entries": [{
            "path": "notes.md",
            "class": "managed",
            "source": { "kind": "template", "identity": "t" },
            "digest": digest,
            "bytes": 1,
            "written_at": "2026-09-17T00:00:00Z"
        }]
    });
    let bytes = serde_json::to_vec_pretty(&old).unwrap();
    std::fs::write(f.at(".statecraft/environment.json"), &bytes).unwrap();
    let out = f.cli(&["project", "register", &f.root()]);
    assert!(out.status.code().unwrap() <= 1, "{out:?}");

    let (_, report) = f.doctor();
    assert_eq!(state_of(&report, "notes.md"), "present", "{report}");
    assert!(
        lines(&report, "notes")
            .iter()
            .any(|l| l.contains("recorded before provenance")),
        "{report}"
    );
    assert!(
        !lines(&report, "findings")
            .iter()
            .any(|l| l.contains("producer identity")),
        "{report}"
    );
    // Diagnosis writes nothing.
    assert_eq!(
        std::fs::read(f.at(".statecraft/environment.json")).unwrap(),
        bytes
    );

    std::fs::write(f.at("notes.md"), "edited").unwrap();
    let (_, report) = f.doctor();
    assert_eq!(state_of(&report, "notes.md"), "drifted", "{report}");
}
