//! Spec 030 V-1: a governed project needs no provider-specific directory, and
//! this product writes none.
//!
//! A fresh repository with every provider directory absent, an isolated home
//! and a stub `spec-spine` is driven through initialization, the environment
//! verbs, the governance reads, a run and the diagnostic. Afterwards the tree
//! holds no provider directory, and the delivery evaluation the initialization
//! reports says the managed instructions are reached for the harness whose
//! load rule this product can evaluate. A second case starts from the state an
//! earlier build left, `.claude/statecraft/instructions.md` recorded as
//! managed, and shows a read never touches it; its removal on upgrade is the
//! library's `tests/retirement.rs`, which runs with a claiming adapter.

#![cfg(unix)]

#[path = "support/json_naming.rs"]
mod json_naming;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PROVIDER_DIRECTORIES: [&str; 6] = [
    ".claude", ".codex", ".agents", ".agent", ".cursor", ".gemini",
];

/// A `spec-spine` that reports a compatible version, a fresh corpus and an
/// empty backlog.
const STUB: &str = r#"#!/bin/sh
case "$*" in
  --version) echo 'spec-spine 0.23.0' ;;
  'check --help') exit 0 ;;
  check|compile|index) exit 0 ;;
  'registry plan --json') echo '{"blocked":[],"ready":[],"schemaVersion":"0.6.0"}' ;;
  'registry list --json') echo '{"items":[]}' ;;
  *) exit 3 ;;
esac
"#;

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
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
        f
    }

    fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }
    fn native(&self) -> PathBuf {
        self.dir.path().join("native")
    }
    fn bin(&self) -> PathBuf {
        self.dir.path().join("bin")
    }
    fn project(&self) -> PathBuf {
        self.dir.path().join("project")
    }
    fn root(&self) -> String {
        self.project().display().to_string()
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_statecraft-cli"))
            .args(args)
            .env_clear()
            .env("STATECRAFT_HOME", self.home())
            .env("STATECRAFT_NATIVE_ROOT", self.native())
            .env("HOME", self.dir.path())
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin().display()))
            .env("USER", "fixture-operator")
            .output()
            .unwrap()
    }

    /// Every provider directory anywhere under the project, `.git` aside.
    fn provider_directories(&self) -> Vec<String> {
        let mut found = Vec::new();
        find(&self.project(), &self.project(), &mut found);
        found
    }
}

fn find(root: &Path, at: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(at).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if name == ".git" || !path.is_dir() || path.is_symlink() {
            continue;
        }
        if PROVIDER_DIRECTORIES.contains(&name.as_str()) {
            out.push(path.strip_prefix(root).unwrap().display().to_string());
        }
        find(root, &path, out);
    }
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn a_project_with_no_provider_directory_is_governed_and_gains_none() {
    let f = Fixture::new();
    let root = f.root();
    assert!(f.provider_directories().is_empty());

    let init = f.cli(&["init", "apply", &root, "--json"]);
    assert!(init.status.code().unwrap() <= 1, "{}", text(&init));
    let answer: serde_json::Value =
        json_naming::from_output(&init.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(&init)));
    let delivery = json_naming::payload(&answer)["value"]["delivery"].clone();
    let claude = delivery
        .as_array()
        .unwrap_or_else(|| panic!("{answer}"))
        .iter()
        .find(|d| d["harness"] == "claude-code")
        .unwrap_or_else(|| panic!("{answer}"))
        .clone();
    assert_eq!(claude["verdict"]["verdict"], "reached", "{claude}");
    assert!(
        claude.to_string().contains(".statecraft/AGENTS.md"),
        "{claude}"
    );

    for args in [
        vec!["env", "plan", root.as_str()],
        vec!["env", "apply", root.as_str()],
        vec!["env", "upgrade", root.as_str()],
        vec!["work", "list", root.as_str()],
        vec!["run", "list", root.as_str()],
        vec!["run", root.as_str()],
        vec!["doctor", root.as_str()],
        vec!["home", "apply"],
        vec!["harness", "show"],
    ] {
        let out = f.cli(&args);
        assert!(
            out.status.code().is_some(),
            "{args:?} ended by a signal: {}",
            text(&out)
        );
        assert!(
            f.provider_directories().is_empty(),
            "{args:?} created {:?}: {}",
            f.provider_directories(),
            text(&out)
        );
    }
    assert!(
        !f.project().join(".claude").exists(),
        "no verb wrote inside the target's provider directory"
    );
}

#[test]
fn a_file_an_earlier_build_left_is_never_touched_by_a_read() {
    let f = Fixture::new();
    let root = f.root();
    let init = f.cli(&["init", "apply", &root, "--json"]);
    assert!(init.status.code().unwrap() <= 1, "{}", text(&init));

    // The state an earlier build's `env apply` left: the file, and its record.
    let retired = statecraft_adapter_claude_code::environment::RETIRED_INSTRUCTIONS;
    let bytes = b"# statecraft: managed harness instructions\n";
    let at = f.project().join(retired);
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(&at, bytes).unwrap();
    use statecraft_environment::manifest::{Class, Entry, Manifest, Role, Source, SourceKind};
    let mut manifest = Manifest::read(&f.project())
        .unwrap()
        .unwrap_or_else(|| panic!("init wrote no manifest: {}", text(&init)));
    let entry = Entry {
        path: retired.to_string(),
        class: Class::Managed,
        source: Source {
            kind: SourceKind::Adapter,
            identity: "claude-code".to_string(),
        },
        digest: statecraft_environment::digest::digest_bytes(bytes),
        bytes: bytes.len() as u64,
        written_at: "2026-09-29T00:00:00Z".to_string(),
        transfer: None,
        role: Role::Reference,
    };
    manifest.upsert(entry);
    manifest.write(&f.project()).unwrap();

    for args in [
        vec!["work", "list", root.as_str()],
        vec!["run", "list", root.as_str()],
        vec!["doctor", root.as_str()],
        vec!["env", "plan", root.as_str()],
    ] {
        let _ = f.cli(&args);
        assert_eq!(std::fs::read(&at).unwrap(), bytes, "{args:?} touched it");
    }

    // Where the adapter does not claim its paths (every platform but macOS,
    // spec 004 section 3.14), an upgrade leaves it too: it could not rewrite a
    // pointer importing the file, so it removes nothing the pointer names.
    #[cfg(not(target_os = "macos"))]
    {
        let _ = f.cli(&["env", "upgrade", &root]);
        assert_eq!(std::fs::read(&at).unwrap(), bytes);
    }
}

/// Spec 030 section 3.2: the pointer is the chain's last link, declared only
/// where nothing earlier reaches the managed instructions, and kept declared
/// where this adapter already wrote one, so convergence rewrites it.
#[test]
fn the_pointer_is_declared_only_where_the_delivery_chain_needs_it() {
    use statecraft_adapter_claude_code::environment::{HARNESS, POINTER, pointer_contents};
    use statecraft_cli::adapters::declarations_for;
    use statecraft_environment::manifest::{Class, Entry, Manifest, Role, Source, SourceKind};

    let pointer_paths = |root: &Path| -> Vec<String> {
        declarations_for(root)
            .iter()
            .flat_map(|d| d.paths().map(str::to_string).collect::<Vec<_>>())
            .collect()
    };

    // A bare repository: nothing reaches the instructions, so the pointer is
    // the link that would.
    let bare = tempfile::tempdir().unwrap();
    assert_eq!(pointer_paths(bare.path()), vec![POINTER.to_string()]);

    // Initialized: root AGENTS.md carries the bridge, which this harness's
    // documented load rule reads, so no pointer is declared and none written.
    let f = Fixture::new();
    let root = f.root();
    let init = f.cli(&["init", "apply", &root, "--json"]);
    assert!(init.status.code().unwrap() <= 1, "{}", text(&init));
    assert!(pointer_paths(&f.project()).is_empty());
    let _ = f.cli(&["env", "apply", &root]);
    assert!(!f.project().join(POINTER).exists());

    // A pointer this adapter recorded stays declared, whatever else reaches.
    let mut manifest = Manifest::read(&f.project()).unwrap().unwrap();
    let bytes = pointer_contents();
    manifest.upsert(Entry {
        path: POINTER.to_string(),
        class: Class::Managed,
        source: Source {
            kind: SourceKind::Adapter,
            identity: HARNESS.to_string(),
        },
        digest: statecraft_environment::digest::digest_bytes(&bytes),
        bytes: bytes.len() as u64,
        written_at: "2026-09-29T00:00:00Z".to_string(),
        transfer: None,
        role: Role::Reference,
    });
    manifest.write(&f.project()).unwrap();
    assert_eq!(pointer_paths(&f.project()), vec![POINTER.to_string()]);
}
