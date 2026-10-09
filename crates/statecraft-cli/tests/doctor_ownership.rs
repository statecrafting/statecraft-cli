//! Spec 026 through the built binary: `doctor` compares recorded, journaled
//! and rendered ownership, reports a rendering it could not compute, reads a
//! manifest with no journal as having no transfers, and writes nothing.
//!
//! Every run is given a temporary `STATECRAFT_HOME`, `HOME` and a constructed
//! `PATH`. The manifest is edited by hand where a row needs a state no verb
//! leaves behind, which is exactly the case the finding exists to surface.
//! Every table row is exercised against the library in
//! `statecraft-environment`'s `tests/ownership_disagreement.rs`.

#![cfg(unix)]

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A governance contract path the linked producer renders.
const CONTRACT: &str = "standards/spec/contract.md";

struct Sandbox {
    dir: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let s = Self { dir };
        for d in [s.home(), s.bin(), s.project()] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(s.dir.path().join(".claude.json"), b"{}").unwrap();
        statecraft_adapter::fixture::install_script(
            &s.bin().join("claude"),
            "#!/bin/sh\n[ \"$1\" = --version ] && { echo 2.1.267; exit 0; }\nexit 3\n",
            0o755,
        )
        .unwrap();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(s.project())
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}");
        };
        git(&["init", "--quiet", "--initial-branch=main"]);
        git(&["config", "user.email", "test@example.invalid"]);
        git(&["config", "user.name", "test"]);
        git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(s.project().join("README.md"), b"x\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "--quiet", "-m", "one"]);
        let root = s.root();
        assert!(code(&s.run(&["project", "register", &root])) <= 1);
        let applied = s.run(&["env", "apply", &root]);
        assert!(code(&applied) <= 1, "{}", text(&applied));
        assert!(s.manifest_path().exists());
        s
    }

    fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }

    fn bin(&self) -> PathBuf {
        self.dir.path().join("bin")
    }

    fn project(&self) -> PathBuf {
        self.dir.path().join("project")
    }

    fn root(&self) -> String {
        self.project().to_string_lossy().to_string()
    }

    fn manifest_path(&self) -> PathBuf {
        self.project().join(".statecraft/environment.json")
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", self.home())
            .env("HOME", self.dir.path())
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin().display()))
            .env("USER", "fixture-operator")
            .output()
            .expect("the binary runs")
    }

    /// `doctor --json`, with the tree read before and after it.
    fn doctor(&self) -> (i32, Value) {
        let before = tree(&self.project());
        let out = self.run(&["doctor", &self.root(), "--json"]);
        assert_eq!(tree(&self.project()), before, "doctor changed the tree");
        let value: Value =
            serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(&out)));
        (code(&out), value)
    }

    fn edit_manifest(&self, edit: impl FnOnce(&mut Value)) {
        let mut m: Value =
            serde_json::from_slice(&std::fs::read(self.manifest_path()).unwrap()).unwrap();
        edit(&mut m);
        std::fs::write(
            self.manifest_path(),
            format!("{}\n", serde_json::to_string_pretty(&m).unwrap()),
        )
        .unwrap();
    }
}

fn code(out: &Output) -> i32 {
    out.status.code().expect("exited")
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Every file under `root`, `.git` included, by path and bytes.
fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let e = e.unwrap();
            let t = e.file_type().unwrap();
            if t.is_dir() {
                stack.push(e.path());
            } else if t.is_file() {
                out.insert(e.path(), std::fs::read(e.path()).unwrap());
            }
        }
    }
    out
}

fn findings(v: &Value) -> Vec<String> {
    v["report"]["findings"]
        .as_array()
        .unwrap_or_else(|| panic!("{v}"))
        .iter()
        .map(|f| f.as_str().unwrap().to_string())
        .collect()
}

fn ownership(v: &Value) -> Vec<String> {
    findings(v)
        .into_iter()
        .filter(|f| f.starts_with("ownership-disagreement"))
        .collect()
}

fn notes(v: &Value) -> Vec<String> {
    v["report"]["notes"]
        .as_array()
        .map(|a| a.iter().map(|n| n.as_str().unwrap().to_string()).collect())
        .unwrap_or_default()
}

fn record(id: &str, path: &str, from: &str, to: &str) -> Value {
    serde_json::json!({
        "id": id,
        "path": path,
        "from": from,
        "to": to,
        "digest": "0".repeat(64),
        "bytes": 0,
        "producer": "spec-spine-core@0.28.0",
        "operator": "fixture",
        "operator_provenance": "operator-supplied",
        "reason": "fixture",
        "at": "2026-10-08T00:00:00Z",
        "manifest_before": "0".repeat(64),
    })
}

#[test]
fn a_legacy_manifest_with_no_journal_has_no_ownership_finding_and_doctor_writes_nothing() {
    let s = Sandbox::new();
    let m: Value = serde_json::from_slice(&std::fs::read(s.manifest_path()).unwrap()).unwrap();
    assert!(m.get("transfers").is_none(), "{m}");
    let (_, v) = s.doctor();
    assert!(ownership(&v).is_empty(), "{v}");
    assert!(
        !notes(&v)
            .iter()
            .any(|n| n.starts_with("ownership-rendering-unavailable")),
        "{v}"
    );
    // The same inputs, the same report.
    let (_, again) = s.doctor();
    assert_eq!(findings(&again), findings(&v));
}

#[test]
fn a_released_contract_path_the_rendering_would_write_again_is_reported() {
    let s = Sandbox::new();
    // The operator released the path; its file is gone, so initialization
    // would write it afresh.
    s.edit_manifest(|m| {
        m["transfers"] = serde_json::json!([record("t1", CONTRACT, "managed", "user")]);
    });
    let (exit, v) = s.doctor();
    let found = ownership(&v);
    assert_eq!(found.len(), 1, "{v}");
    assert!(
        found[0].starts_with(&format!(
            "ownership-disagreement {CONTRACT} journaled/rendered: journaled user (journal record t1), rendered managed (governance producer"
        )),
        "{}",
        found[0]
    );
    assert_eq!(exit, 1);
}

#[test]
fn recorded_against_journaled_and_a_managed_entry_nothing_renders_are_each_named() {
    let s = Sandbox::new();
    s.edit_manifest(|m| {
        let entries = m["entries"].as_array_mut().unwrap();
        entries.push(serde_json::json!({
            "path": "docs/orphan.md",
            "class": "managed",
            "source": { "kind": "adapter", "identity": "retired-adapter" },
            "digest": "0".repeat(64),
            "bytes": 0,
            "written_at": "2026-10-08T00:00:00Z",
        }));
        entries.push(serde_json::json!({
            "path": "notes/kept.md",
            "class": "adopted",
            "source": { "kind": "template", "identity": "statecraft-governance" },
            "digest": "0".repeat(64),
            "bytes": 0,
            "written_at": "2026-10-08T00:00:00Z",
        }));
        entries.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
        m["transfers"] = serde_json::json!([record("t9", "notes/kept.md", "adopted", "user")]);
    });
    let (exit, v) = s.doctor();
    let found = ownership(&v);
    assert_eq!(found.len(), 2, "{v}");
    assert!(
        found[0].starts_with(
            "ownership-disagreement docs/orphan.md recorded/rendered: recorded managed (manifest entry, source adapter retired-adapter), rendered absent"
        ),
        "{}",
        found[0]
    );
    assert!(
        found[1].starts_with(
            "ownership-disagreement notes/kept.md recorded/journaled: recorded adopted (manifest entry, source template statecraft-governance), journaled user (journal record t9)"
        ),
        "{}",
        found[1]
    );
    assert_eq!(exit, 1);
}

#[test]
fn a_rendering_that_cannot_be_computed_is_reported_once_and_recorded_and_journaled_are_still_compared()
 {
    let s = Sandbox::new();
    s.edit_manifest(|m| {
        m["project"]["setup"] = serde_json::json!({
            "profile": "no-such-profile",
            "revision": 1,
            "identity": "fixture",
        });
        let entries = m["entries"].as_array_mut().unwrap();
        entries.push(serde_json::json!({
            "path": "docs/orphan.md",
            "class": "managed",
            "source": { "kind": "adapter", "identity": "retired-adapter" },
            "digest": "0".repeat(64),
            "bytes": 0,
            "written_at": "2026-10-08T00:00:00Z",
        }));
        m["transfers"] = serde_json::json!([record("t1", "notes/gone.md", "managed", "user")]);
    });
    std::fs::create_dir_all(s.project().join("notes")).unwrap();
    s.edit_manifest(|m| {
        let entries = m["entries"].as_array_mut().unwrap();
        entries.push(serde_json::json!({
            "path": "notes/gone.md",
            "class": "managed",
            "source": { "kind": "template", "identity": "other" },
            "digest": "0".repeat(64),
            "bytes": 0,
            "written_at": "2026-10-08T00:00:00Z",
        }));
        entries.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    });
    let (exit, v) = s.doctor();
    let unavailable: Vec<_> = notes(&v)
        .into_iter()
        .filter(|n| n.starts_with("ownership-rendering-unavailable"))
        .collect();
    assert_eq!(unavailable.len(), 1, "{v}");
    assert!(
        unavailable[0].contains("no-such-profile"),
        "{}",
        unavailable[0]
    );
    // The orphan needs a rendering to be judged, so it is not reported; the
    // journal still disagrees with the recorded class, so that is.
    let found = ownership(&v);
    assert_eq!(found.len(), 1, "{v}");
    assert!(
        found[0].starts_with("ownership-disagreement notes/gone.md recorded/journaled"),
        "{}",
        found[0]
    );
    assert_eq!(exit, 1);
}
