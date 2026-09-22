//! Shared scaffolding for the managed-environment acceptance suite.
//!
//! Two rules this module exists to keep:
//!
//! - **Nothing touches the operator's real home.** Every test runs against a
//!   temporary product home and a temporary native root, and the helpers here
//!   are the only way a test names either.
//! - **A fixture is labelled as one.** Where a test needs a producer answer,
//!   the answer is derived from the real library and restricted to the
//!   contract set, which is what a conforming producer returns. It is still a
//!   fixture, and no test built on it claims a live integration.

#![allow(dead_code)]

use statecraft_environment::qualify::{CorpusState, TargetProbe};
use statecraft_home::authority::RevisionReader;
use statecraft_home::flow::{Corpus, SpecSpineCommand};
use statecraft_home::home::Layout;
use statecraft_home::producer::{self, Identity, Library, Recorded};
use statecraft_home::service::{self, Answer, Operation, Ports};
use statecraft_home::team::CoordinationAuthority;
use std::path::{Path, PathBuf};

/// The `spec-spine` this repository is governed by.
///
/// The repository-local install first, because a bare `spec-spine` is whichever
/// project on this machine built it last. Absent both, the suite fails and says
/// what to run rather than skipping: a check that quietly does not run is worse
/// than one that fails.
pub fn spec_spine_program() -> String {
    let local = repo_root().join(".tooling/bin/spec-spine");
    if local.is_file() {
        return local.display().to_string();
    }
    if std::process::Command::new("spec-spine")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return "spec-spine".to_string();
    }
    panic!(
        "no spec-spine is available. Run `make tools` to install the pinned version into \
         .tooling/bin; this suite asserts against the real governance tool and will not \
         substitute a stand-in for it."
    );
}

/// This repository's root, from the crate this test belongs to.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name>/ has two ancestors")
        .to_path_buf()
}

/// The real corpus tool.
pub fn corpus_tool() -> SpecSpineCommand {
    SpecSpineCommand {
        program: spec_spine_program(),
    }
}

/// A corpus tool that answers from stated results.
#[derive(Debug, Clone, Default)]
pub struct StatedCorpus {
    /// What every verb answers.
    pub ok: bool,
    /// The version to report.
    pub version: Option<String>,
}

impl Corpus for StatedCorpus {
    fn compile(&self, _root: &Path) -> Result<String, String> {
        self.answer("compile")
    }
    fn index(&self, _root: &Path) -> Result<String, String> {
        self.answer("index")
    }
    fn check(&self, _root: &Path) -> Result<String, String> {
        self.answer("check")
    }
    fn version(&self) -> Option<String> {
        self.version.clone()
    }
}

impl StatedCorpus {
    /// Every verb succeeds.
    pub fn fine() -> Self {
        Self {
            ok: true,
            version: Some("0.0.0-stated".to_string()),
        }
    }

    fn answer(&self, what: &str) -> Result<String, String> {
        if self.ok {
            Ok(format!("{what}: stated"))
        } else {
            Err(format!("{what}: stated failure"))
        }
    }
}

/// A target probe that answers from stated facts.
#[derive(Debug, Clone)]
pub struct StatedProbe {
    /// Whether the path is a git work tree.
    pub git: bool,
    /// Whether a base revision resolves.
    pub base: bool,
    /// What the corpus looks like.
    pub corpus: CorpusState,
}

impl Default for StatedProbe {
    fn default() -> Self {
        Self {
            git: true,
            base: true,
            corpus: CorpusState::Compiles,
        }
    }
}

impl TargetProbe for StatedProbe {
    fn is_git_work_tree(&self, _: &Path) -> bool {
        self.git
    }
    fn has_base_revision(&self, _: &Path) -> bool {
        self.base
    }
    fn corpus(&self, _: &Path) -> CorpusState {
        self.corpus.clone()
    }
}

/// The real library's answer, restricted to the contract set.
///
/// **A fixture, and labelled as one.** It is what a conforming producer
/// returns, built from the real library's own bytes so the corpus it scaffolds
/// is a real one that the real `spec-spine` can compile. It establishes what
/// this product does with a conforming answer, and nothing about whether the
/// library is conforming today: `tests/producer_integration.rs` asks the
/// library directly for that.
pub fn conforming_producer() -> Recorded {
    let real = producer::produce(&Library).expect("the library answers");
    let mut files: Vec<serde_json::Value> = real
        .governance
        .iter()
        .map(|f| serde_json::json!({ "relPath": f.rel_path, "contents": f.contents }))
        .collect();
    if let Some(fragment) = real.ignore_fragment {
        files.push(serde_json::json!({ "relPath": ".gitignore", "contents": fragment }));
    }
    Recorded {
        json: serde_json::json!({ "files": files }).to_string(),
        identity: Identity {
            name: "spec-spine-core (contract set only, a test fixture)".to_string(),
            version: producer::PRODUCER_VERSION.to_string(),
        },
    }
}

/// A sandbox: a temporary product home, a temporary native root, and a
/// temporary repository.
pub struct Sandbox {
    /// Keeps the temporary tree alive.
    pub dir: tempfile::TempDir,
}

impl Sandbox {
    /// A new sandbox with a git repository at `project/`.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let sandbox = Self { dir };
        sandbox.init_repo(sandbox.project());
        sandbox
    }

    /// The product home.
    pub fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }

    /// The product home's layout.
    pub fn layout(&self) -> Layout {
        Layout::new(self.home())
    }

    /// The native root every agent home hangs off.
    pub fn native_root(&self) -> PathBuf {
        self.dir.path().join("native")
    }

    /// The project.
    pub fn project(&self) -> PathBuf {
        self.dir.path().join("project")
    }

    /// A second, unrelated repository on the same machine.
    pub fn unrelated(&self) -> PathBuf {
        let path = self.dir.path().join("unrelated");
        if !path.is_dir() {
            self.init_repo(path.clone());
        }
        path
    }

    /// Make a git repository with one commit, so a base revision resolves.
    pub fn init_repo(&self, at: PathBuf) {
        std::fs::create_dir_all(&at).expect("the repository directory");
        git(&at, &["init", "--quiet"]);
        std::fs::write(at.join("README.md"), "# a repository\n").expect("a file to commit");
        git(&at, &["add", "."]);
        git(
            &at,
            &[
                "-c",
                "user.email=suite@example.invalid",
                "-c",
                "user.name=suite",
                "commit",
                "--quiet",
                "-m",
                "initial",
            ],
        );
    }

    /// Write a file inside the project.
    pub fn write(&self, rel: &str, contents: &str) {
        let at = statecraft_environment::claimant::resolve(&self.project(), rel);
        std::fs::create_dir_all(at.parent().expect("a parent")).expect("the directory");
        std::fs::write(at, contents).expect("the file");
    }

    /// Read a file inside the project.
    pub fn read(&self, rel: &str) -> Option<String> {
        std::fs::read_to_string(statecraft_environment::claimant::resolve(
            &self.project(),
            rel,
        ))
        .ok()
    }

    /// Whether a path exists inside the project.
    pub fn exists(&self, rel: &str) -> bool {
        statecraft_environment::claimant::resolve(&self.project(), rel).exists()
    }

    /// Commit whatever is in the project, returning the revision.
    pub fn commit(&self, message: &str) -> String {
        let at = self.project();
        git(&at, &["add", "-A"]);
        git(
            &at,
            &[
                "-c",
                "user.email=suite@example.invalid",
                "-c",
                "user.name=suite",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                message,
            ],
        );
        let out = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&at)
            .output()
            .expect("git rev-parse");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }
}

/// Run a git command in a directory, failing loudly.
pub fn git(at: &Path, args: &[&str]) {
    // No automatic maintenance. A commit can start `git maintenance run
    // --auto` detached, and its lock file then appears in, and vanishes from,
    // a repository a test is snapshotting: measured on Linux CI on
    // 2026-09-22 as `.git/objects/maintenance.lock` in one snapshot of the
    // unrelated repository and not the next. That is git changing its own
    // directory, not this product changing the repository.
    let out = std::process::Command::new("git")
        .args(["-c", "maintenance.auto=false", "-c", "gc.auto=0"])
        .args(args)
        .current_dir(at)
        .output()
        .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A fixed clock, so a record's timestamp is not a moving target.
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub i64);

impl statecraft_environment::time::Clock for FixedClock {
    fn now_unix(&self) -> i64 {
        self.0
    }
}

/// Everything an operation needs, with each part chosen by the caller.
pub struct Harness<'a> {
    /// The product home.
    pub layout: Layout,
    /// Where governance starter files come from.
    pub producer: &'a dyn producer::Producer,
    /// How the corpus is compiled and checked.
    pub corpus: &'a dyn Corpus,
    /// How a project is qualified.
    pub probe: &'a dyn TargetProbe,
    /// The coordination authority.
    pub authority: &'a dyn CoordinationAuthority,
    /// How a file is read at a revision.
    pub revisions: &'a dyn RevisionReader,
    /// The clock.
    pub clock: FixedClock,
    /// Where native agent homes hang off.
    pub native_root: PathBuf,
}

impl<'a> Harness<'a> {
    /// Perform one operation.
    pub fn execute(&self, operation: Operation) -> Answer {
        let ports = Ports {
            home: &self.layout,
            producer: self.producer,
            corpus: self.corpus,
            target_probe: self.probe,
            authority: self.authority,
            revisions: self.revisions,
            clock: &self.clock,
            native_root: self.native_root.clone(),
            product_version: "0.0.0-suite".to_string(),
        };
        service::execute(&ports, operation)
    }
}

/// The initialization report inside an answer, or a panic naming what came
/// back instead.
pub fn init_report(answer: &Answer) -> &statecraft_home::flow::Report {
    match answer {
        Answer::Init(report) => report,
        other => panic!("expected an initialization report, got {other:?}"),
    }
}
