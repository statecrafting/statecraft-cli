//! `env remove` and the root instruction bridge, through the built binary.
//!
//! Spec 002 section 3.13 rule 4: "Removal removes the inserted line and nothing
//! else, and only while the file still begins with it." Spec 002 section 3.6:
//! `env remove` deletes every managed path whose digest still matches, reports
//! and leaves a drifted one, refuses with no manifest, and never touches an
//! adopted or user path. Exit codes are spec 006 section 3.3's.
//!
//! Every run gets a temporary `STATECRAFT_HOME`, `STATECRAFT_NATIVE_ROOT` and
//! `HOME`, so nothing here reads or writes the operator's own home.

#[path = "support/json_naming.rs"]
mod json_naming;

use statecraft_environment::digest::digest_bytes;
use statecraft_environment::manifest::{
    Class, Entry, Manifest, Modification, ModificationKind, Pins, Source, SourceKind,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const LINE: &str = "@.statecraft/AGENTS.md";

struct Sandbox {
    dir: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let sandbox = Self { dir };
        let at = sandbox.project();
        std::fs::create_dir_all(&at).unwrap();
        for args in [
            vec!["init", "--quiet", "--initial-branch=main"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["config", "user.name", "test"],
            vec!["config", "commit.gpgsign", "false"],
        ] {
            assert!(
                Command::new("git")
                    .args(&args)
                    .current_dir(&at)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        std::fs::write(at.join("README.md"), b"readme").unwrap();
        for args in [vec!["add", "."], vec!["commit", "--quiet", "-m", "one"]] {
            assert!(
                Command::new("git")
                    .args(&args)
                    .current_dir(&at)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        sandbox
    }

    fn project(&self) -> PathBuf {
        self.dir.path().join("project")
    }

    fn root(&self) -> String {
        self.project().to_str().unwrap().to_string()
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env("STATECRAFT_HOME", self.dir.path().join("home"))
            .env("STATECRAFT_NATIVE_ROOT", self.dir.path().join("native"))
            .env("HOME", self.dir.path())
            .env("PATH", stub_path(self.dir.path()))
            .output()
            .expect("the binary runs")
    }

    fn register(&self) {
        let out = self.run(&["project", "register", &self.root()]);
        assert!(code(&out) <= 1, "{out:?}");
    }

    fn remove(&self) -> Output {
        self.run(&["env", "remove", &self.root(), "--json"])
    }

    fn agents(&self) -> Option<String> {
        std::fs::read_to_string(self.project().join("AGENTS.md")).ok()
    }

    fn manifest(&self) -> Option<Manifest> {
        Manifest::read(&self.project()).unwrap()
    }

    /// A manifest written the way `init apply` records a bridge.
    fn bridged_by_record(&self, before: Option<&str>, after: &str) -> Manifest {
        let mut m = Manifest::new(pins());
        m.upsert_modification(Modification {
            path: "AGENTS.md".into(),
            kind: ModificationKind::ImportBridge,
            line: LINE.into(),
            digest_before: before.map(|b| digest_bytes(b.as_bytes())),
            digest_after: digest_bytes(after.as_bytes()),
            written_at: "2026-09-23T00:00:00Z".into(),
        });
        m.write(&self.project()).unwrap();
        m
    }
}

fn pins() -> Pins {
    Pins {
        product: "0.0.0".into(),
        spec_spine: "0.23.0".into(),
        adapters: BTreeMap::new(),
        producer: None,
    }
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("the process exited normally")
}

fn json(o: &Output) -> serde_json::Value {
    json_naming::from_output(&o.stdout).unwrap_or_else(|e| panic!("{e}: {o:?}"))
}

fn withheld_reasons(o: &Output) -> Vec<String> {
    json(o)["report"]["withheld"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|w| format!("{} {}", w["path"], w["reason"]))
                .collect()
        })
        .unwrap_or_default()
}

fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(dir).unwrap().display().to_string();
                out.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

#[test]
fn env_remove_takes_back_the_bridge_init_apply_recorded_and_keeps_the_users_file() {
    let sandbox = Sandbox::new();
    let user = "# My project\n\nOur own rules.\n";
    std::fs::write(sandbox.project().join("AGENTS.md"), user).unwrap();
    let init = sandbox.run(&["init", "apply", &sandbox.root()]);
    assert!(code(&init) <= 1, "{init:?}");
    let bridged = sandbox.agents().unwrap();
    assert!(bridged.starts_with(LINE), "{bridged}");
    let recorded = sandbox.manifest().unwrap();
    assert_eq!(recorded.modifications.len(), 1, "init records the bridge");

    let out = sandbox.remove();

    assert!(
        !serde_json::to_string(&json(&out)["report"]["withheld"])
            .unwrap()
            .contains("AGENTS.md"),
        "{out:?}"
    );
    assert_eq!(
        sandbox.agents().as_deref(),
        Some(user),
        "the user's bytes, exactly"
    );
    assert!(sandbox.manifest().unwrap().modifications.is_empty());
    assert!(
        json(&out)["report"]["written"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p == "AGENTS.md")
    );
}

#[test]
fn env_remove_keeps_every_edit_the_user_made_around_the_bridge() {
    let sandbox = Sandbox::new();
    sandbox.register();
    let user = "# Mine\n\nrules\n";
    sandbox.bridged_by_record(Some(user), &format!("{LINE}\n\n{user}"));
    std::fs::write(
        sandbox.project().join("AGENTS.md"),
        format!("{LINE}\n\n# Mine, renamed\n\nrules\nplus one\n"),
    )
    .unwrap();

    let out = sandbox.remove();

    assert_eq!(code(&out), 0, "{out:?}");
    assert_eq!(
        sandbox.agents().as_deref(),
        Some("# Mine, renamed\n\nrules\nplus one\n")
    );
}

#[test]
fn env_remove_refuses_a_bridge_that_no_longer_begins_the_file_and_says_so() {
    let sandbox = Sandbox::new();
    sandbox.register();
    sandbox.bridged_by_record(Some("# Mine\n"), &format!("{LINE}\n\n# Mine\n"));
    let moved = format!("# Mine\n\n{LINE}\n");
    std::fs::write(sandbox.project().join("AGENTS.md"), &moved).unwrap();

    let out = sandbox.remove();

    assert_eq!(code(&out), 1, "a withheld removal is a finding: {out:?}");
    assert!(
        withheld_reasons(&out)
            .iter()
            .any(|r| r.contains("AGENTS.md") && r.contains("no longer begins")),
        "{out:?}"
    );
    assert_eq!(sandbox.agents(), Some(moved));
    assert_eq!(sandbox.manifest().unwrap().modifications.len(), 1);
}

#[test]
fn env_remove_refuses_two_candidate_bridges() {
    let sandbox = Sandbox::new();
    sandbox.register();
    sandbox.bridged_by_record(Some("x\n"), &format!("{LINE}\n\nx\n"));
    let two = format!("{LINE}\n\nx\n{LINE}\n");
    std::fs::write(sandbox.project().join("AGENTS.md"), &two).unwrap();

    let out = sandbox.remove();

    assert_eq!(code(&out), 1, "{out:?}");
    assert!(
        withheld_reasons(&out).iter().any(|r| r.contains("2 times")),
        "{out:?}"
    );
    assert_eq!(sandbox.agents(), Some(two));
}

#[test]
fn env_remove_refuses_two_records_for_one_bridge() {
    let sandbox = Sandbox::new();
    sandbox.register();
    let mut m = sandbox.bridged_by_record(Some("x\n"), &format!("{LINE}\n\nx\n"));
    let copy = m.modifications[0].clone();
    m.modifications.push(copy);
    m.write(&sandbox.project()).unwrap();
    let bridged = format!("{LINE}\n\nx\n");
    std::fs::write(sandbox.project().join("AGENTS.md"), &bridged).unwrap();

    let out = sandbox.remove();

    assert_eq!(code(&out), 1, "{out:?}");
    assert!(
        withheld_reasons(&out)
            .iter()
            .any(|r| r.contains("2 modifications")),
        "{out:?}"
    );
    assert_eq!(sandbox.agents(), Some(bridged));
}

#[test]
fn env_remove_leaves_a_bridge_it_has_no_record_of_and_notes_it() {
    let sandbox = Sandbox::new();
    sandbox.register();
    Manifest::new(pins()).write(&sandbox.project()).unwrap();
    let text = format!("{LINE}\n\nthe user wrote this line themselves\n");
    std::fs::write(sandbox.project().join("AGENTS.md"), &text).unwrap();

    let out = sandbox.remove();

    assert_eq!(code(&out), 0, "a note is not a finding: {out:?}");
    assert!(
        json(&out)["report"]["notes"][0]
            .as_str()
            .unwrap()
            .contains("records no bridge"),
        "{out:?}"
    );
    assert!(withheld_reasons(&out).is_empty());
    assert_eq!(sandbox.agents(), Some(text));
}

#[test]
fn env_remove_reports_a_record_whose_bridge_is_gone_and_keeps_the_record() {
    let sandbox = Sandbox::new();
    sandbox.register();
    sandbox.bridged_by_record(Some("x\n"), &format!("{LINE}\n\nx\n"));

    let out = sandbox.remove();

    assert_eq!(code(&out), 1, "{out:?}");
    assert!(sandbox.agents().is_none(), "nothing is created");
    assert_eq!(sandbox.manifest().unwrap().modifications.len(), 1);
}

#[test]
fn env_remove_keeps_a_bridge_file_it_created_leaving_it_empty() {
    let sandbox = Sandbox::new();
    sandbox.register();
    let created = format!("{LINE}\n");
    std::fs::write(sandbox.project().join("AGENTS.md"), &created).unwrap();
    sandbox.bridged_by_record(None, &created);

    let out = sandbox.remove();

    assert_eq!(code(&out), 0, "{out:?}");
    assert_eq!(
        sandbox.agents().as_deref(),
        Some(""),
        "the line goes, the file stays"
    );
    let again = sandbox.remove();
    assert_eq!(code(&again), 0, "repeating a finished removal: {again:?}");
    assert_eq!(sandbox.agents().as_deref(), Some(""));
}

#[test]
fn env_remove_never_takes_a_line_init_found_already_first() {
    // H1: the user's file already began with the import line, so `init apply`
    // inserted nothing, and removal must take nothing.
    let sandbox = Sandbox::new();
    let user = format!("{LINE}\n\n# Mine\n");
    std::fs::write(sandbox.project().join("AGENTS.md"), &user).unwrap();
    let init = sandbox.run(&["init", "apply", &sandbox.root()]);
    assert!(code(&init) <= 1, "{init:?}");
    assert!(
        sandbox.manifest().unwrap().modifications.is_empty(),
        "no insertion, no record"
    );

    let out = sandbox.remove();

    assert_eq!(sandbox.agents(), Some(user), "{out:?}");
    assert!(
        json(&out)["report"]["notes"][0]
            .as_str()
            .unwrap()
            .contains("records no bridge"),
        "{out:?}"
    );
}

#[test]
fn env_remove_after_init_ran_twice_on_a_file_init_created() {
    // H1's second sequence: the second `init apply` finds the line first and
    // must keep the first run's record, which says the file did not exist.
    let sandbox = Sandbox::new();
    for _ in 0..2 {
        let init = sandbox.run(&["init", "apply", &sandbox.root()]);
        assert!(code(&init) <= 1, "{init:?}");
    }
    let record = sandbox.manifest().unwrap().modifications[0].clone();
    assert!(record.digest_before.is_none(), "{record:?}");
    std::fs::write(
        sandbox.project().join("AGENTS.md"),
        format!("{LINE}\n\nwritten after init\n"),
    )
    .unwrap();

    let out = sandbox.remove();

    assert_eq!(code(&out), 0, "{out:?}");
    assert_eq!(
        sandbox.agents().as_deref(),
        Some("\nwritten after init\n"),
        "only the line this product created"
    );
}

#[test]
fn env_remove_refuses_a_record_that_names_a_path_outside_the_repository() {
    let sandbox = Sandbox::new();
    sandbox.register();
    let outside_path = sandbox.dir.path().join("outside.md");
    let outside = format!("{LINE}\n\nnot the product's\n");
    std::fs::write(&outside_path, &outside).unwrap();
    let mut m = sandbox.bridged_by_record(Some("x"), &outside);
    m.modifications[0].path = "../outside.md".into();
    m.write(&sandbox.project()).unwrap();

    let out = sandbox.remove();

    assert_eq!(code(&out), 1, "{out:?}");
    assert!(
        withheld_reasons(&out)
            .iter()
            .any(|r| r.contains("../outside.md") && r.contains("refused")),
        "{out:?}"
    );
    assert_eq!(std::fs::read_to_string(&outside_path).unwrap(), outside);
}

#[test]
fn env_remove_finishes_a_removal_that_was_interrupted_before_the_manifest() {
    let sandbox = Sandbox::new();
    sandbox.register();
    let user = "# Mine\n";
    sandbox.bridged_by_record(Some(user), &format!("{LINE}\n\n{user}"));
    std::fs::write(sandbox.project().join("AGENTS.md"), user).unwrap();

    let out = sandbox.remove();

    assert_eq!(code(&out), 0, "{out:?}");
    assert_eq!(sandbox.agents().as_deref(), Some(user));
    assert!(sandbox.manifest().unwrap().modifications.is_empty());
}

#[test]
fn env_remove_takes_no_separator_the_insertion_did_not_add() {
    let sandbox = Sandbox::new();
    sandbox.register();
    sandbox.bridged_by_record(Some(""), &format!("{LINE}\n"));
    std::fs::write(
        sandbox.project().join("AGENTS.md"),
        format!("{LINE}\n\n\nfoo\n"),
    )
    .unwrap();

    let out = sandbox.remove();

    assert_eq!(code(&out), 0, "{out:?}");
    assert_eq!(sandbox.agents().as_deref(), Some("\n\nfoo\n"));
}

#[test]
fn env_remove_withholds_a_drifted_managed_path_removes_the_rest_and_writes_the_manifest() {
    let sandbox = Sandbox::new();
    sandbox.register();
    let root = sandbox.project();
    let mut m = Manifest::new(pins());
    for (path, bytes) in [("clean.md", "c"), ("dirty.md", "d")] {
        std::fs::write(root.join(path), bytes).unwrap();
        m.upsert(Entry {
            path: path.into(),
            class: Class::Managed,
            source: Source {
                kind: SourceKind::Adapter,
                identity: "claude-code".into(),
            },
            digest: digest_bytes(bytes.as_bytes()),
            bytes: 1,
            written_at: "2026-09-23T00:00:00Z".into(),
            transfer: None,
            role: Default::default(),
        });
    }
    std::fs::write(root.join("adopted.md"), "a").unwrap();
    m.upsert(Entry {
        path: "adopted.md".into(),
        class: Class::Adopted,
        source: Source {
            kind: SourceKind::Adapter,
            identity: "claude-code".into(),
        },
        digest: digest_bytes(b"a"),
        bytes: 1,
        written_at: "2026-09-23T00:00:00Z".into(),
        transfer: None,
        role: Default::default(),
    });
    m.write(&root).unwrap();
    std::fs::write(root.join("dirty.md"), "the operator's now").unwrap();

    let out = sandbox.remove();

    assert_eq!(code(&out), 1, "{out:?}");
    let value = &json(&out)["report"];
    assert_eq!(value["outcome"], "partial");
    assert_eq!(value["written"], serde_json::json!(["clean.md"]));
    assert_eq!(value["withheld"][0]["path"], "dirty.md");
    assert!(!root.join("clean.md").exists());
    assert_eq!(
        std::fs::read_to_string(root.join("dirty.md")).unwrap(),
        "the operator's now"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("adopted.md")).unwrap(),
        "a"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("README.md")).unwrap(),
        "readme"
    );
    // The manifest is written, not deleted: it still records what is left.
    let after = sandbox.manifest().expect("the manifest is still there");
    let paths: Vec<&str> = after.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, ["adopted.md", "dirty.md"]);
}

#[test]
fn env_remove_with_no_manifest_refuses_and_changes_nothing() {
    let sandbox = Sandbox::new();
    sandbox.register();
    let text = format!("{LINE}\n\nmine\n");
    std::fs::write(sandbox.project().join("AGENTS.md"), &text).unwrap();
    let before = snapshot(&sandbox.project());

    let out = sandbox.remove();

    assert_eq!(code(&out), 2, "{out:?}");
    assert_eq!(snapshot(&sandbox.project()), before);
}

#[test]
fn env_remove_takes_no_option_and_no_stray_argument() {
    let sandbox = Sandbox::new();
    sandbox.register();
    let root = sandbox.root();
    for args in [
        vec!["env", "remove", root.as_str(), "--replace", "AGENTS.md"],
        vec!["env", "remove", root.as_str(), "extra"],
        vec!["doctor", root.as_str(), "extra"],
    ] {
        let out = sandbox.run(&args);
        assert_eq!(code(&out), 3, "{args:?}: {out:?}");
    }
}

/// A `spec-spine` that answers every verb these suites' flows ask, first on
/// `PATH`, so they do not depend on whichever one the machine has. Spec 002
/// section 3.23: an absent producer is a refused corpus step and an
/// unregistered project, which is not what these suites are about.
const STUB_SPEC_SPINE: &str =
    "#!/bin/sh\ncase \"$1\" in\n  --version) echo 'spec-spine 0.23.0' ;;\n  *) exit 0 ;;\nesac\n";

fn stub_path(dir: &std::path::Path) -> String {
    let bin = dir.join("stub-bin");
    let spine = bin.join("spec-spine");
    if !spine.exists() {
        std::fs::create_dir_all(&bin).expect("the stub directory");
        statecraft_adapter::fixture::install_script(&spine, STUB_SPEC_SPINE, 0o755)
            .expect("the stub");
    }
    format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}
