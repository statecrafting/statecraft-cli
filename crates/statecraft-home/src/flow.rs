//! The one initialization flow.
//!
//! Spec 002 section 3.17. Seven steps, in order, each reported separately, and
//! then it stops: arming and execution are separate explicit acts and are not
//! steps of initialization.
//!
//! **The bootstrap cycle is avoided by that order, not by an exception.**
//! Qualification is step seven because the corpus it judges is created by step
//! six. Nothing in steps one to six requires the project to be qualified, and
//! `init` never requires a prior qualified state.
//!
//! **An interrupted or repeated initialization is safe.** Each step is
//! idempotent, the last completed step is recorded under the project's runtime
//! state, a re-run re-reconciles and rewrites no authored file, and the outcome
//! is `complete` only when every step reported done.
//!
//! `plan` and `apply` are one function with one flag. A preview computed by
//! one code path and performed by another is not a preview.
//!
//! **Every precondition is decided before the first write** (spec 002 section
//! 5, 2026-09-24). The preflight is shared by both, writes nothing, takes no
//! lock and creates nothing; `plan` runs only the preflight. `apply` takes the
//! manifest lock first, runs the preflight, then performs, and reports every
//! change it made as read from disk. Four outcomes: `complete`, `partial`,
//! `refused` (a precondition, nothing written but the lock) and `failed` (an
//! execution error, whatever was written).

use crate::bridge;
use crate::delivery::{self, Delivery};
use crate::home::{Layout, NOT_RECORDED, Personal, ToolRecord, Tools};
use crate::ignore;
use crate::producer::{self, Conformance, Producer};
use crate::project;
use serde::Serialize;
use statecraft_environment::adapter::{Declaration, ManagedFile, StaticProbe};
use statecraft_environment::claimant::{ForeignClaims, resolve};
use statecraft_environment::digest::{digest_bytes, digest_file};
use statecraft_environment::manifest::{Class, Entry, Manifest, Pins, Source, SourceKind};
use statecraft_environment::plan::Withholding;
use statecraft_environment::probe::{CheckAnswer, Unavailability, check_unavailable, run_check};
use statecraft_environment::qualify::Qualification;
use statecraft_environment::registry::Registry;
use statecraft_environment::time::{Clock, rfc3339_utc};
use std::path::Path;

/// The pseudo-harness the governance files are declared under.
///
/// The governance starter files are not an agent-harness adapter's; they are
/// this product's own templates. The name exists so spec 002's plan, which is
/// where every withholding rule lives, can be reused without restating one of
/// them here.
pub const GOVERNANCE_SOURCE: &str = "statecraft-governance";

/// How `spec-spine` is asked about a corpus.
///
/// A trait because the real implementation runs a process, and because a test
/// must be able to initialize a project without a governance binary installed.
pub trait Corpus {
    /// Compile the corpus.
    fn compile(&self, root: &Path) -> Result<String, String>;
    /// Index the codebase.
    fn index(&self, root: &Path) -> Result<String, String>;
    /// Check that the committed artifacts are fresh and valid.
    fn check(&self, root: &Path) -> Result<String, String>;
    /// The tool's version, when it can be asked.
    fn version(&self) -> Option<String>;

    /// The program this double or command runs, when it names one. What
    /// [`Report::observed_spec_spine`] reports as the executable observed.
    fn program(&self) -> Option<String> {
        None
    }

    /// Whether the tool is there and carries `check`, asked before anything is
    /// run (spec 002 section 3.23, contract 5). `Err` carries the answer that
    /// says why not.
    ///
    /// The default is a tool that is always there, which is what a stated test
    /// double is.
    fn carries_check(&self, _root: &Path) -> Result<(), CheckAnswer> {
        Ok(())
    }

    /// `check`'s answer in spec-spine's own vocabulary, for the translation of
    /// section 3.23. The default reads [`Corpus::check`] as the two answers a
    /// stated double can give: fresh, or a corpus that does not validate.
    fn check_answer(&self, root: &Path) -> CheckAnswer {
        match self.check(root) {
            Ok(_) => CheckAnswer::Fresh,
            Err(detail) => CheckAnswer::DoesNotValidate { detail },
        }
    }

    /// `compile`'s answer, translated as `check`'s is. The default reads
    /// [`Corpus::compile`]'s `Err` as a finding, which is what a stated double
    /// means by it.
    fn compile_answer(&self, root: &Path) -> Ran {
        self.compile(root).map_or_else(Ran::Finding, |_| Ran::Done)
    }

    /// `index`'s answer, the same way.
    fn index_answer(&self, root: &Path) -> Ran {
        self.index(root).map_or_else(Ran::Finding, |_| Ran::Done)
    }
}

/// How a writing verb of the corpus tool ended, in section 3.23's reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ran {
    /// Exit 0.
    Done,
    /// Exit 1 or 2: a finding about the corpus.
    Finding(String),
    /// Any other end, a signal, or a spawn error: the verb did not perform.
    NotPerformed(String),
}

/// The real one: the `spec-spine` binary, asked rather than reimplemented.
#[derive(Debug, Clone)]
pub struct SpecSpineCommand {
    /// The binary to run.
    pub program: String,
}

impl Default for SpecSpineCommand {
    fn default() -> Self {
        Self {
            program: "spec-spine".to_string(),
        }
    }
}

impl SpecSpineCommand {
    fn run(&self, root: &Path, args: &[&str]) -> Result<String, String> {
        let output = std::process::Command::new(&self.program)
            .args(args)
            .current_dir(root)
            .output()
            .map_err(|e| format!("{} {}: {e}", self.program, args.join(" ")))?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if output.status.success() {
            Ok(first_line(&text))
        } else {
            Err(first_line(&text))
        }
    }
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("no output")
        .to_string()
}

impl SpecSpineCommand {
    fn ran(&self, root: &Path, verb: &str) -> Ran {
        let output = match std::process::Command::new(&self.program)
            .arg(verb)
            .current_dir(root)
            .output()
        {
            Ok(o) => o,
            Err(e) => return Ran::NotPerformed(format!("{} {verb}: {e}", self.program)),
        };
        let text = first_line(&format!(
            "{}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        ));
        match output.status.code() {
            Some(0) => Ran::Done,
            Some(1 | 2) => Ran::Finding(text),
            Some(c) => Ran::NotPerformed(format!("exit {c}: {text}")),
            None => Ran::NotPerformed(format!("signal: {text}")),
        }
    }
}

impl Corpus for SpecSpineCommand {
    fn compile_answer(&self, root: &Path) -> Ran {
        self.ran(root, "compile")
    }
    fn index_answer(&self, root: &Path) -> Ran {
        self.ran(root, "index")
    }
    fn compile(&self, root: &Path) -> Result<String, String> {
        self.run(root, &["compile"])
    }
    fn index(&self, root: &Path) -> Result<String, String> {
        self.run(root, &["index"])
    }
    fn check(&self, root: &Path) -> Result<String, String> {
        self.run(root, &["check"])
    }
    fn carries_check(&self, root: &Path) -> Result<(), CheckAnswer> {
        match check_unavailable(&self.program, root) {
            Some(answer) => Err(answer),
            None => Ok(()),
        }
    }
    fn check_answer(&self, root: &Path) -> CheckAnswer {
        run_check(&self.program, root)
    }
    fn program(&self) -> Option<String> {
        Some(self.program.clone())
    }
    fn version(&self) -> Option<String> {
        let output = std::process::Command::new(&self.program)
            .arg("--version")
            .output()
            .ok()?;
        output.status.success().then(|| {
            String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .next_back()
                .unwrap_or(NOT_RECORDED)
                .to_string()
        })
    }
}

/// A corpus tool that is not installed.
///
/// Reports the absence rather than pretending the corpus compiled. Spec 002
/// section 3.23 translates an absent binary into a refusal: the corpus step
/// does nothing, and says a precondition was not met.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoCorpusTool;

impl Corpus for NoCorpusTool {
    fn compile(&self, _root: &Path) -> Result<String, String> {
        Err("no spec-spine is available to compile the corpus".to_string())
    }
    fn index(&self, _root: &Path) -> Result<String, String> {
        Err("no spec-spine is available to index the codebase".to_string())
    }
    fn check(&self, _root: &Path) -> Result<String, String> {
        Err("no spec-spine is available to check the corpus".to_string())
    }
    fn version(&self) -> Option<String> {
        None
    }
    fn carries_check(&self, _root: &Path) -> Result<(), CheckAnswer> {
        Err(CheckAnswer::Unavailable {
            why: Unavailability::Absent,
            detail: "no spec-spine is available".to_string(),
        })
    }
}

/// The seven steps, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Step {
    /// Resolve and check the global environment.
    Home,
    /// Compute every project change.
    Plan,
    /// Preserve every user file; classify what is already there.
    Reconcile,
    /// Obtain the starter files from the real producer and place them.
    Governance,
    /// Write the declaration and the instruction bridge.
    Project,
    /// Compile, index and check the intended corpus.
    Corpus,
    /// Register the project and evaluate its qualification.
    Register,
}

impl Step {
    /// Every step, in order.
    pub fn all() -> [Step; 7] {
        [
            Step::Home,
            Step::Plan,
            Step::Reconcile,
            Step::Governance,
            Step::Project,
            Step::Corpus,
            Step::Register,
        ]
    }

    /// A one-word rendering.
    pub fn word(self) -> &'static str {
        match self {
            Step::Home => "home",
            Step::Plan => "plan",
            Step::Reconcile => "reconcile",
            Step::Governance => "governance",
            Step::Project => "project",
            Step::Corpus => "corpus",
            Step::Register => "register",
        }
    }
}

/// How one step ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum StepState {
    /// It did what it was for.
    Done,
    /// It did part of it, and named what it withheld.
    Withheld {
        /// Why.
        reason: String,
    },
    /// A precondition stopped it.
    Refused {
        /// Why.
        reason: String,
    },
    /// Something nobody asked for went wrong.
    Failed {
        /// Why.
        reason: String,
    },
}

impl StepState {
    /// A one-word rendering.
    pub fn word(&self) -> &'static str {
        match self {
            StepState::Done => "done",
            StepState::Withheld { .. } => "withheld",
            StepState::Refused { .. } => "refused",
            StepState::Failed { .. } => "failed",
        }
    }

    /// Whether the step completed.
    pub fn done(&self) -> bool {
        matches!(self, StepState::Done)
    }
}

/// When a step refusal was decided (spec 002 section 5, 2026-09-24, rule 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// Before any mutation.
    Preflight,
    /// After mutations, although the preflight passed.
    Late,
}

/// One step's report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepReport {
    /// Which step.
    pub step: Step,
    /// How it ended.
    pub state: StepState,
    /// What it did, for a person.
    pub detail: String,
    /// For a refusal, when it was decided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<Phase>,
}

impl StepReport {
    fn new(step: Step, state: StepState, detail: impl Into<String>) -> Self {
        let phase = matches!(state, StepState::Refused { .. }).then_some(Phase::Preflight);
        Self {
            step,
            state,
            detail: detail.into(),
            phase,
        }
    }

    fn late(mut self) -> Self {
        if self.phase.is_some() {
            self.phase = Some(Phase::Late);
        }
        self
    }
}

/// Whether this was a preview or a performance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// Writes nothing.
    Plan,
    /// Performs it.
    Apply,
}

/// How an initialization ended.
///
/// Spec 002 section 5, 2026-09-24: four outcomes, one exit each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Every step reported done.
    Complete,
    /// Something was withheld, skipped or refused, and no step failed. Never
    /// reported as complete.
    Partial,
    /// A precondition stopped it in the preflight. Nothing was written but
    /// what taking the manifest lock created.
    Refused,
    /// An execution error in some step, whether or not anything was written.
    Failed,
}

impl Outcome {
    /// A one-word rendering.
    pub fn word(self) -> &'static str {
        match self {
            Outcome::Complete => "complete",
            Outcome::Partial => "partial",
            Outcome::Refused => "refused",
            Outcome::Failed => "failed",
        }
    }
}

/// Which side of the boundary a change landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    /// Under the project root, outside `.statecraft/state/`.
    Project,
    /// Under `.statecraft/state/`.
    ProjectState,
    /// The product home.
    Home,
}

impl Category {
    /// A one-word rendering.
    pub fn word(self) -> &'static str {
        match self {
            Category::Project => "project",
            Category::ProjectState => "project-state",
            Category::Home => "home",
        }
    }
}

/// One change an initialization made, read from disk before and after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mutation {
    /// Where.
    pub category: Category,
    /// Which step made it: a step's word, or `lock` for the manifest lock.
    pub step: String,
    /// Repository-relative under the project, absolute in the home.
    pub path: String,
    /// A SHA-256 digest, `absent`, `directory` or `unreadable`.
    pub before: String,
    /// A SHA-256 digest, `removed`, `directory` or `unreadable`.
    pub after: String,
}

/// What an initialization did, or would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// Preview or performance.
    pub mode: Mode,
    /// The project.
    pub root: String,
    /// One per step attempted, in order.
    pub steps: Vec<StepReport>,
    /// Repository-relative paths the plan writes. What was actually changed is
    /// [`Report::mutations`], never this list.
    pub writes: Vec<String>,
    /// Paths not written, each with its reason.
    pub withheld: Vec<String>,
    /// Paths already there and now depended on rather than rewritten.
    pub adopted: Vec<String>,
    /// Authored inputs on disk, left alone: `seeded` or `customized`, with a
    /// newer seed named by its digest (spec 002 section 5, 2026-09-24,
    /// provenance item 2). Information, never a withholding.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub kept: Vec<String>,
    /// Every change made, observed on disk. Always empty for a plan.
    pub mutations: Vec<Mutation>,
    /// What the producer returned, and whether it stayed in contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conformance: Option<Conformance>,
    /// The instruction bridge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge: Option<bridge::Plan>,
    /// Per harness, whether the managed instructions are actually reached.
    pub delivery: Vec<DeliveryReport>,
    /// The qualification, when the project was registered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualification: Option<Qualification>,
    /// The `spec-spine` executable this run observed: an observation, never
    /// a pin (spec 002 section 5, 2026-09-24, provenance item 4). The pin the
    /// project declares is the declaration's `pins.spec_spine`.
    pub observed_spec_spine: ObservedExecutable,
    /// The selected setup profile's plan, and its six results (spec 002
    /// section 5, 2026-09-24, the setup-profile entry).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<crate::setup::Plan>,
    /// How it ended.
    pub outcome: Outcome,
}

/// An executable as observed: which program, what it answered, and how it
/// was found. Reported, never recorded as a pin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedExecutable {
    /// The program as invoked, or `not-recorded` when none was named.
    pub program: String,
    /// The version it answered to `--version`, or `not-recorded`.
    pub version: String,
    /// `path` when the program is a bare name resolved through `PATH`,
    /// `explicit` when it names a file, `not-recorded` otherwise.
    pub found_by: String,
}

impl ObservedExecutable {
    fn of(corpus: &dyn Corpus) -> Self {
        let program = corpus.program();
        let found_by = match &program {
            Some(p) if p.contains('/') => "explicit",
            Some(_) => "path",
            None => NOT_RECORDED,
        };
        Self {
            program: program.unwrap_or_else(|| NOT_RECORDED.to_string()),
            version: corpus.version().unwrap_or_else(|| NOT_RECORDED.to_string()),
            found_by: found_by.to_string(),
        }
    }
}

/// One harness's delivery verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryReport {
    /// The harness.
    pub harness: String,
    /// The verdict.
    pub verdict: Delivery,
}

impl Report {
    /// A human-readable rendering.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for step in &self.steps {
            let phase = match step.phase {
                Some(Phase::Preflight) => " (preflight)",
                Some(Phase::Late) => " (late)",
                None => "",
            };
            out.push_str(&format!(
                "{:<11} {:<9} {}{phase}\n",
                step.step.word(),
                step.state.word(),
                step.detail
            ));
        }
        for path in &self.adopted {
            out.push_str(&format!("adopt      {path}\n"));
        }
        if self.mode == Mode::Plan {
            for path in &self.writes {
                out.push_str(&format!("would write {path}\n"));
            }
        }
        for path in &self.withheld {
            out.push_str(&format!("withhold   {path}\n"));
        }
        for m in &self.mutations {
            out.push_str(&format!(
                "changed    {} {} ({}): {} -> {}\n",
                m.category.word(),
                m.path,
                m.step,
                short(&m.before),
                short(&m.after)
            ));
        }
        for d in &self.delivery {
            out.push_str(&format!(
                "delivery   {}: {}\n",
                d.harness,
                d.verdict.describe()
            ));
        }
        if let Some(setup) = &self.setup {
            out.push_str(&setup.render());
        }
        out.push_str(&format!("{}\n", self.outcome.word()));
        out
    }

    /// Spec 002 section 5, 2026-09-24, rule 2. `stopped` is a precondition
    /// that ended the preflight.
    fn finish(mut self, stopped: bool) -> Self {
        let failed = self
            .steps
            .iter()
            .any(|s| matches!(s.state, StepState::Failed { .. }));
        self.outcome = if failed {
            Outcome::Failed
        } else if stopped {
            Outcome::Refused
        } else if self.steps.len() < Step::all().len() || self.steps.iter().any(|s| !s.state.done())
        {
            Outcome::Partial
        } else {
            Outcome::Complete
        };
        self
    }
}

fn short(state: &str) -> &str {
    if state.len() == 64 && state.bytes().all(|b| b.is_ascii_hexdigit()) {
        &state[..12]
    } else {
        state
    }
}

/// Where the flow records how far it got.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// Schema version.
    pub version: u32,
    /// The last step that reported done, if any.
    #[serde(default)]
    pub last_completed: Option<Step>,
    /// When, RFC 3339 UTC.
    pub updated_at: String,
}

impl Progress {
    /// Read the recorded progress, if any.
    pub fn read(root: &Path) -> Option<Self> {
        let bytes = std::fs::read(resolve(root, project::INIT_STATE)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    fn write(root: &Path, step: Step, now: &str) -> std::io::Result<()> {
        let path = resolve(root, project::INIT_STATE);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut json = serde_json::to_string_pretty(&Progress {
            version: 1,
            last_completed: Some(step),
            updated_at: now.to_string(),
        })?;
        json.push('\n');
        std::fs::write(path, json)
    }
}

/// Everything the flow needs, supplied rather than discovered.
pub struct Context<'a> {
    /// The product home.
    pub home: &'a Layout,
    /// The project.
    pub root: &'a Path,
    /// Where governance starter files come from.
    pub producer: &'a dyn Producer,
    /// How the corpus is compiled and checked.
    pub corpus: &'a dyn Corpus,
    /// How the project is qualified.
    pub target_probe: &'a dyn statecraft_environment::qualify::TargetProbe,
    /// The clock.
    pub clock: &'a dyn Clock,
    /// This build's version, for the pins.
    pub product_version: String,
    /// What the operator asked of a setup profile.
    pub setup: SetupRequest,
}

/// What the operator asked of a setup profile: `init plan|apply <path>
/// --profile <id> [--plan <identity>] [--verify-local]`. A project whose
/// declaration already selects a profile is re-planned with it when no
/// profile is named.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetupRequest {
    /// The profile named on the command line.
    pub profile: Option<String>,
    /// The plan identity the operator approved: apply refuses, writing
    /// nothing but the lock, when the recomputed plan differs.
    pub plan: Option<String>,
    /// Run the rendered gate here after applying.
    pub verify_local: bool,
}

/// Compute the initialization: the preflight alone. Writes nothing, takes no
/// lock and creates no file or directory.
pub fn plan(ctx: &Context<'_>) -> Report {
    run(ctx, Mode::Plan)
}

/// Perform the initialization: the manifest lock, the preflight, then the
/// steps.
pub fn apply(ctx: &Context<'_>) -> Report {
    run(ctx, Mode::Apply)
}

/// Observes changes on disk around each write.
///
/// Spec 002 section 5, 2026-09-24, rule 1: a mutation is read from disk before
/// and after the operation, including one that failed part-way, and never
/// computed from the plan.
struct Recorder {
    root: std::path::PathBuf,
    list: Vec<Mutation>,
}

/// A path's state: a digest, `absent`, `directory` or `unreadable`.
fn observe(path: &Path) -> String {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "absent".to_string(),
        Err(_) => "unreadable".to_string(),
        Ok(m) if m.is_dir() => "directory".to_string(),
        Ok(_) => {
            std::fs::read(path).map_or_else(|_| "unreadable".to_string(), |b| digest_bytes(&b))
        }
    }
}

/// `path` and each ancestor that does not exist yet, so a `create_dir_all`
/// or a write that creates its parents is observed whole.
fn chain(path: &Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![path.to_path_buf()];
    let mut at = path.parent();
    while let Some(dir) = at {
        if dir.as_os_str().is_empty() || dir.exists() {
            break;
        }
        out.push(dir.to_path_buf());
        at = dir.parent();
    }
    out.reverse();
    out
}

/// Every path under `dir`, and `dir`'s missing ancestors. Used where a program
/// this product does not control writes (the corpus tool in step 6).
fn tree(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = chain(dir);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(path.clone());
            }
            out.push(path);
        }
    }
    out.sort();
    out.dedup();
    out
}

impl Recorder {
    fn new(ctx: &Context<'_>) -> Self {
        let absolute = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        Self {
            root: absolute(ctx.root),
            list: Vec::new(),
        }
    }

    /// Where `path` belongs, and how it is named in the report.
    fn classify(&self, path: &Path) -> (Category, String) {
        let path = self.absolute(path);
        if let Ok(rel) = path.strip_prefix(&self.root) {
            let rel = rel.to_string_lossy().to_string();
            let state = Path::new(project::STATE);
            let category = if Path::new(&rel).starts_with(state) {
                Category::ProjectState
            } else {
                Category::Project
            };
            return (category, rel);
        }
        (Category::Home, path.display().to_string())
    }

    /// `path` with its existing prefix resolved, so a project reached through
    /// a link is still recognised as the project.
    fn absolute(&self, path: &Path) -> std::path::PathBuf {
        let mut existing = path.to_path_buf();
        let mut rest = Vec::new();
        while !existing.exists() {
            match (existing.file_name(), existing.parent()) {
                (Some(name), Some(parent)) => {
                    rest.push(name.to_os_string());
                    existing = parent.to_path_buf();
                }
                _ => return path.to_path_buf(),
            }
        }
        let mut out = std::fs::canonicalize(&existing).unwrap_or(existing);
        for name in rest.into_iter().rev() {
            out.push(name);
        }
        out
    }

    /// Run `op`, observing `paths` on either side of it, and record each one
    /// that changed.
    fn around<T>(
        &mut self,
        step: &str,
        paths: Vec<std::path::PathBuf>,
        op: impl FnOnce() -> T,
    ) -> T {
        let before: Vec<(std::path::PathBuf, String)> = paths
            .into_iter()
            .map(|p| {
                let state = observe(&p);
                (p, state)
            })
            .collect();
        let result = op();
        for (path, was) in before {
            let now = observe(&path);
            if now == was {
                continue;
            }
            let after = if now == "absent" {
                "removed".to_string()
            } else {
                now
            };
            let (category, name) = self.classify(&path);
            self.list.push(Mutation {
                category,
                step: step.to_string(),
                path: name,
                before: was,
                after,
            });
        }
        result
    }

    /// As [`Recorder::around`], over a whole directory tree read again after
    /// `op`, so files `op` created are found.
    fn around_tree<T>(&mut self, step: &str, dir: &Path, op: impl FnOnce() -> T) -> T {
        let before: std::collections::BTreeMap<std::path::PathBuf, String> = tree(dir)
            .into_iter()
            .map(|p| (p.clone(), observe(&p)))
            .collect();
        let result = op();
        let mut paths: Vec<std::path::PathBuf> = before.keys().cloned().collect();
        paths.extend(tree(dir));
        paths.sort();
        paths.dedup();
        for path in paths {
            let was = before
                .get(&path)
                .cloned()
                .unwrap_or_else(|| "absent".to_string());
            let now = observe(&path);
            if now == was {
                continue;
            }
            let after = if now == "absent" {
                "removed".to_string()
            } else {
                now
            };
            let (category, name) = self.classify(&path);
            self.list.push(Mutation {
                category,
                step: step.to_string(),
                path: name,
                before: was,
                after,
            });
        }
        result
    }
}

/// Everything the preflight established, and what performing it needs.
struct Prepared {
    personal: Option<Personal>,
    tools: Tools,
    starter: producer::Starter,
    manifest: Manifest,
    adopted: Vec<String>,
    computed: statecraft_environment::plan::Plan,
    ignore: IgnorePlan,
    bridge: bridge::Plan,
    corpus: Result<(), CheckAnswer>,
    setup: Option<crate::setup::Plan>,
}

/// The `.gitignore` merge, decided in the preflight.
struct IgnorePlan {
    /// The bytes to write, when there is a change.
    contents: Option<String>,
    detail: String,
}

/// The preflight: every read and computation `init apply` depends on, and
/// every precondition it can refuse on. Writes nothing.
fn preflight(ctx: &Context<'_>, now: &str, report: &mut Report) -> Result<Prepared, StepReport> {
    let failed = |step: Step, reason: String, detail: &str| {
        StepReport::new(step, StepState::Failed { reason }, detail)
    };

    // 1. home: what step 1 would write, from what the home holds now.
    let personal = Personal::read(ctx.home).map_err(|e| {
        failed(
            Step::Home,
            e.to_string(),
            "the personal defaults could not be read",
        )
    })?;
    let personal = (!ctx.home.personal_file().exists()).then_some(personal);
    let mut tools = Tools::read(ctx.home).map_err(|e| {
        failed(
            Step::Home,
            e.to_string(),
            "the tools record could not be read",
        )
    })?;
    tools.upsert(ToolRecord {
        name: "spec-spine".to_string(),
        requested: "any".to_string(),
        resolved: ctx
            .corpus
            .version()
            .unwrap_or_else(|| NOT_RECORDED.to_string()),
        observed_from: "path".to_string(),
        recorded_at: now.to_string(),
    });
    for revision in crate::harness::installed_revisions(ctx.home) {
        tools.record_revision(&revision);
    }

    // 2. plan. The producer is asked once, here, and its answer carries every
    //    later step.
    let starter = producer::produce(ctx.producer).map_err(|e| {
        StepReport::new(
            Step::Plan,
            StepState::Refused {
                reason: e.to_string(),
            },
            "the governance producer did not answer",
        )
    })?;
    report.conformance = Some(starter.conformance.clone());

    // The managed files this product places: the producer's in-contract set,
    // plus this product's own managed instructions.
    let mut managed: Vec<(String, String)> = starter
        .governance
        .iter()
        .map(|f| (f.rel_path.clone(), f.contents.clone()))
        .collect();
    managed.push((
        project::INSTRUCTIONS.to_string(),
        project::managed_instructions(),
    ));

    // 3. reconcile, in memory. A contract path already on disk is ADOPTED:
    //    recorded with the digest observed, depended on, and never rewritten.
    let mut manifest = match Manifest::read(ctx.root) {
        Ok(Some(m)) => m,
        // The declared pin is read after the governance step, below; until
        // then nothing is declared. Never the version found on `PATH`.
        Ok(None) => Manifest::new(Pins {
            product: ctx.product_version.clone(),
            spec_spine: statecraft_environment::manifest::UNPINNED.to_string(),
            adapters: Default::default(),
            producer: None,
        }),
        Err(e) => {
            return Err(failed(
                Step::Reconcile,
                e.to_string(),
                "the declaration could not be read",
            ));
        }
    };
    let adopted = step_reconcile(ctx, &managed, &mut manifest, now).map_err(|e| {
        failed(
            Step::Reconcile,
            e.to_string(),
            "the project could not be read",
        )
    })?;

    // 4. governance. Which paths are written and which are withheld is spec
    //    002's plan, asked rather than restated here.
    let declaration = governance_declaration(&managed);
    let probe = StaticProbe::new().with_harness(GOVERNANCE_SOURCE);
    let computed = statecraft_environment::plan::plan(
        ctx.root,
        Some(&manifest),
        std::slice::from_ref(&declaration),
        &probe,
        &ForeignClaims::none(),
    )
    .map_err(|e| {
        failed(
            Step::Governance,
            e.to_string(),
            "the project could not be planned",
        )
    })?;
    for write in &computed.writes {
        report.writes.push(write.path.clone());
    }
    for kept in &computed.kept {
        report.kept.push(kept.describe());
    }
    for held in &computed.withheld {
        report
            .withheld
            .push(format!("{}: {}", held.path, held.reason.describe()));
    }

    // The setup profile, planned beside the governance plan and from the
    // same reconciliation. A parameter it refuses, or an approved plan that
    // is not the plan now, is a precondition: nothing is written.
    let setup = plan_setup(ctx, &starter, &manifest)?;
    if let Some(plan) = &setup {
        for f in plan.files.iter().filter(|f| f.action.writes()) {
            report.writes.push(f.path.clone());
        }
    }
    let fragment = match (&setup, starter.ignore_fragment.as_deref()) {
        (Some(plan), governance) if plan.withheld.is_none() => Some(format!(
            "{}{}{}",
            governance.unwrap_or(""),
            if governance.is_some_and(|g| !g.ends_with('\n')) {
                "\n"
            } else {
                ""
            },
            plan.ignore_fragment
        )),
        (_, governance) => governance.map(str::to_string),
    };

    // The ignore fragment is merged, never installed as a file, and its
    // refusal is a precondition decided here, before anything is written.
    let ignore = plan_ignore(ctx.root, fragment.as_deref()).map_err(|e| match e {
        IgnoreStop::Refused(refusal) => StepReport::new(
            Step::Governance,
            StepState::Refused {
                reason: refusal.describe(),
            },
            "the ignore rules would take the project area out of version control",
        ),
        IgnoreStop::Unreadable(reason) => failed(
            Step::Governance,
            reason,
            "the ignore rules could not be read",
        ),
    })?;

    // 5. project. The instruction bridge is a tracked modification of a file
    //    this product does not own.
    let known = known_generated(&starter);
    let root_text = match std::fs::read_to_string(resolve(ctx.root, project::ROOT_INSTRUCTIONS)) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(failed(
                Step::Project,
                format!("{}: {e}", project::ROOT_INSTRUCTIONS),
                "the root instructions could not be read",
            ));
        }
    };
    let bridge_plan = bridge::plan(root_text.as_deref(), &known);
    if bridge_plan.action.changes_the_file() {
        report.writes.push(project::ROOT_INSTRUCTIONS.to_string());
    }
    report.bridge = Some(bridge_plan.clone());

    // 6. corpus. Degradable: whether the tool is there and carries `check` is
    //    decided now and reported, and it stops nothing but step 6.
    let corpus = ctx.corpus.carries_check(ctx.root);

    Ok(Prepared {
        personal,
        tools,
        starter,
        manifest,
        adopted,
        computed,
        ignore,
        bridge: bridge_plan,
        corpus,
        setup,
    })
}

/// The setup profile's plan, when one is selected.
fn plan_setup(
    ctx: &Context<'_>,
    starter: &producer::Starter,
    manifest: &Manifest,
) -> Result<Option<crate::setup::Plan>, StepReport> {
    let refused = |reason: String| {
        StepReport::new(
            Step::Plan,
            StepState::Refused { reason },
            "the setup profile could not be planned",
        )
    };
    let recorded = manifest.project.setup.as_ref();
    let Some(id) = ctx
        .setup
        .profile
        .clone()
        .or_else(|| recorded.map(|s| s.profile.clone()))
    else {
        if ctx.setup.plan.is_some() {
            return Err(refused(
                "--plan names a setup plan, and no setup profile is selected".to_string(),
            ));
        }
        return Ok(None);
    };
    let Some(profile) = crate::setup::Profile::named(&id) else {
        return Err(refused(format!(
            "unknown setup profile `{id}`; this build knows {}",
            crate::setup::PROFILE_ID
        )));
    };
    let block = recorded
        .filter(|s| s.profile == id)
        .map(|s| s.parameters.clone())
        .unwrap_or_default();
    let toml = match std::fs::read_to_string(resolve(ctx.root, "spec-spine.toml")) {
        Ok(text) => Some(text),
        Err(_) => starter
            .governance
            .iter()
            .find(|f| f.rel_path == "spec-spine.toml")
            .map(|f| f.contents.clone()),
    };
    let plan = crate::setup::plan(&crate::setup::Inputs {
        root: ctx.root,
        profile: &profile,
        block: &block,
        manifest,
        spec_spine_toml: toml.as_deref(),
        derived_dir: project::DERIVED,
    })
    .map_err(refused)?;
    if let Some(approved) = &ctx.setup.plan
        && *approved != plan.plan_identity
    {
        return Err(refused(format!(
            "the approved setup plan {approved} is not the plan now, {}: an input changed after it was planned",
            plan.plan_identity
        )));
    }
    Ok(Some(plan))
}

fn run(ctx: &Context<'_>, mode: Mode) -> Report {
    let writing = mode == Mode::Apply;
    let now = rfc3339_utc(ctx.clock.now_unix());
    let mut report = Report {
        mode,
        root: ctx.root.display().to_string(),
        steps: Vec::new(),
        writes: Vec::new(),
        withheld: Vec::new(),
        adopted: Vec::new(),
        kept: Vec::new(),
        mutations: Vec::new(),
        conformance: None,
        bridge: None,
        delivery: Vec::new(),
        qualification: None,
        observed_spec_spine: ObservedExecutable::of(ctx.corpus),
        setup: None,
        outcome: Outcome::Partial,
    };
    let mut rec = Recorder::new(ctx);

    // The manifest lock, first and only when performing: every write of the
    // declaration below happens under it, taken before the read, so a transfer
    // or an `env apply` recorded in between cannot be erased by this run's
    // write (spec 002 section 3.35). What taking it created is reported, also
    // when it is where the initialization stops.
    let _manifest_lock = if writing {
        let lock_paths = chain(&ctx.root.join(statecraft_environment::manifest::LOCK_PATH));
        let taken = rec.around("lock", lock_paths, || {
            statecraft_environment::manifest::lock(
                ctx.root,
                statecraft_environment::manifest::WRITER_WAIT,
            )
        });
        match taken {
            Ok(held) => Some(held),
            Err(e) => {
                // Another writer held it past the wait: a precondition.
                let busy = matches!(
                    e,
                    statecraft_environment::manifest::ManifestError::Busy { .. }
                );
                let reason = e.to_string();
                report.steps.push(StepReport::new(
                    Step::Home,
                    if busy {
                        StepState::Refused { reason }
                    } else {
                        StepState::Failed { reason }
                    },
                    "the declaration could not be locked for writing",
                ));
                report.mutations = rec.list;
                return report.finish(busy);
            }
        }
    } else {
        None
    };

    let prepared = match preflight(ctx, &now, &mut report) {
        Ok(p) => p,
        Err(stop) => {
            let refused = matches!(stop.state, StepState::Refused { .. });
            report.steps.push(stop);
            report.mutations = rec.list;
            return report.finish(refused);
        }
    };
    let Prepared {
        personal,
        tools,
        starter,
        mut manifest,
        adopted,
        computed,
        ignore,
        bridge: bridge_plan,
        corpus: corpus_ready,
        setup,
    } = prepared;

    macro_rules! stop_failed {
        ($step:expr, $reason:expr, $detail:expr) => {{
            report.steps.push(StepReport::new(
                $step,
                StepState::Failed { reason: $reason },
                $detail,
            ));
            report.mutations = rec.list;
            return report.finish(false);
        }};
    }

    // 1. home. The product's own home, never a native agent location: writing
    //    into one of those is `home apply` and an explicit operator act.
    if writing {
        if let Err(reason) = step_home(ctx, &mut rec, personal.as_ref(), &tools) {
            stop_failed!(Step::Home, reason, "the product home could not be written");
        }
        if let Err(e) = progress(&mut rec, ctx.root, Step::Home, &now) {
            stop_failed!(
                Step::Home,
                e,
                "the initialization's progress could not be recorded"
            );
        }
    }
    let presence = crate::home::presence(ctx.home);
    report.steps.push(StepReport::new(
        Step::Home,
        StepState::Done,
        format!(
            "{} ({} harness revision(s)); no account, login or token was consulted",
            presence.root.display(),
            crate::harness::installed_revisions(ctx.home).len()
        ),
    ));

    // 2. plan.
    report.steps.push(StepReport::new(
        Step::Plan,
        StepState::Done,
        format!(
            "{} governance file(s), {}",
            starter.governance.len(),
            starter.conformance.describe()
        ),
    ));

    // 3. reconcile.
    report.adopted = adopted.clone();
    report.steps.push(StepReport::new(
        Step::Reconcile,
        StepState::Done,
        format!("{} existing file(s) adopted, none rewritten", adopted.len()),
    ));

    // 4. governance.
    if writing {
        if let Err(e) = perform_writes(
            ctx.root,
            &mut rec,
            &computed,
            &mut manifest,
            ctx.producer,
            &now,
        ) {
            stop_failed!(
                Step::Governance,
                e.to_string(),
                "a governance file could not be written"
            );
        }
        if let Some(contents) = &ignore.contents {
            let path = ctx.root.join(".gitignore");
            let written = rec.around(Step::Governance.word(), chain(&path), || {
                std::fs::write(&path, contents)
            });
            if let Err(e) = written {
                stop_failed!(
                    Step::Governance,
                    format!(".gitignore: {e}"),
                    "the ignore rules could not be written"
                );
            }
        }
        if let Some(plan) = &setup {
            let root = ctx.root;
            let performed =
                crate::setup::apply(root, plan, &mut manifest, &now, &mut |path, bytes| {
                    rec.around(Step::Governance.word(), chain(path), || {
                        if let Some(parent) = path.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        std::fs::write(path, bytes)
                    })
                });
            if let Err(e) = performed {
                report.setup = Some(plan.clone());
                stop_failed!(
                    Step::Governance,
                    format!("setup profile: {e}"),
                    "a setup profile file could not be written"
                );
            }
        }
    }
    // Spec 002 section 5, 2026-09-24 (I-3): a path withheld for any reason
    // but `adopted` leaves this step withheld, so the initialization is
    // `partial` and never `complete`. An adopted path is reconcile's intended
    // result and is reported under `adopted` instead.
    let conflicts: Vec<&str> = computed
        .withheld
        .iter()
        .filter(|held| !matches!(held.reason, Withholding::Adopted))
        .map(|held| held.path.as_str())
        .collect();
    let setup_shortfall = setup.as_ref().and_then(crate::setup::Plan::shortfall);
    report.steps.push(StepReport::new(
        Step::Governance,
        if !starter.conformance.conforming {
            StepState::Withheld {
                reason: starter.conformance.describe(),
            }
        } else if !conflicts.is_empty() || setup_shortfall.is_some() {
            // Both are named when both hold: the governance paths withheld,
            // then the setup profile's shortfall.
            let governance = (!conflicts.is_empty()).then(|| {
                format!(
                    "{} path(s) withheld: {}",
                    conflicts.len(),
                    conflicts.join(", ")
                )
            });
            StepState::Withheld {
                reason: governance
                    .into_iter()
                    .chain(setup_shortfall)
                    .collect::<Vec<_>>()
                    .join("; "),
            }
        } else {
            StepState::Done
        },
        format!(
            "{} write(s), {} withheld, {}",
            computed.writes.len(),
            computed.withheld.len(),
            ignore.detail
        ),
    ));

    // Spec 002 section 5, 2026-09-24, provenance items 3 and 4: the pins
    // record the one producer identity this build links, and the pin the
    // project now declares, read after the governance step wrote or adopted
    // `spec-spine.toml`. The executable observed is in the report, not here.
    if writing {
        manifest.pins.spec_spine = statecraft_environment::manifest::declared_pin(ctx.root);
        manifest.pins.producer = Some(producer::linked());
    }

    // 5. project. The declaration and the instruction bridge.
    if writing {
        if let Err(e) = write_project(ctx.root, &mut rec, &bridge_plan, &mut manifest, &now) {
            stop_failed!(
                Step::Project,
                e,
                format!(
                    "declaration and instructions; root bridge {}",
                    bridge_plan.action.word()
                )
            );
        }
        // The manifest is written, so the setup's resume record has done its
        // work: an interrupted run before this point re-plans from it.
        if setup.is_some() {
            let path = ctx.root.join(crate::setup::RESUME_PATH);
            if let Err(e) = rec.around(Step::Project.word(), chain(&path), || {
                crate::setup::finish(ctx.root)
            }) {
                stop_failed!(
                    Step::Project,
                    format!("{}: {e}", crate::setup::RESUME_PATH),
                    "the setup resume record could not be removed"
                );
            }
        }
        if let Err(e) = progress(&mut rec, ctx.root, Step::Project, &now) {
            stop_failed!(
                Step::Project,
                e,
                "the initialization's progress could not be recorded"
            );
        }
    }
    report.steps.push(StepReport::new(
        Step::Project,
        StepState::Done,
        format!(
            "declaration and instructions; root bridge {}",
            bridge_plan.action.word()
        ),
    ));

    // 6. corpus. Compile, index, then check. `check` last and never replaced by
    //    a writing verb: a read that repairs what it measures cannot measure it.
    let corpus_refused_early = corpus_ready.is_err();
    let corpus_report = match corpus_ready {
        Err(answer) => StepReport::new(
            Step::Corpus,
            StepState::Refused {
                reason: answer.describe(),
            },
            answer.describe(),
        ),
        Ok(()) if !writing => StepReport::new(
            Step::Corpus,
            StepState::Done,
            "would compile, index and check",
        ),
        Ok(()) => {
            let derived = ctx.root.join(project::DERIVED);
            let state = rec.around_tree(Step::Corpus.word(), &derived, || step_corpus(ctx));
            let late = matches!(state, StepState::Refused { .. });
            let detail = match &state {
                StepState::Done => "compiled, indexed and checked".to_string(),
                StepState::Withheld { reason }
                | StepState::Refused { reason }
                | StepState::Failed { reason } => reason.clone(),
            };
            let r = StepReport::new(Step::Corpus, state, detail);
            if late { r.late() } else { r }
        }
    };
    let corpus_done = corpus_report.state.done();
    let corpus_failed = matches!(corpus_report.state, StepState::Failed { .. });
    report.steps.push(corpus_report);
    if corpus_failed {
        report.mutations = rec.list;
        return report.finish(false);
    }
    if writing
        && corpus_done
        && let Err(e) = progress(&mut rec, ctx.root, Step::Corpus, &now)
    {
        stop_failed!(
            Step::Corpus,
            e,
            "the initialization's progress could not be recorded"
        );
    }

    // 7. register. Registration and qualification, and then it stops: arming
    //    is a separate act and execution is another one again.
    let register_report = match step_register(ctx, &mut rec, writing) {
        Ok((qualification, detail)) => {
            report.qualification = Some(qualification);
            StepReport::new(Step::Register, StepState::Done, detail)
        }
        Err(RegisterFailure::Refused(reason)) => {
            let r = StepReport::new(
                Step::Register,
                StepState::Refused { reason },
                "the project was not registered",
            );
            // Decided in the preflight when step 6 already found the tool
            // unavailable; otherwise a producer refused after mutations.
            if corpus_refused_early || !writing {
                r
            } else {
                r.late()
            }
        }
        Err(RegisterFailure::Failed(reason)) => StepReport::new(
            Step::Register,
            StepState::Failed { reason },
            "the project could not be registered",
        ),
    };
    let registered = register_report.state.done();
    report.steps.push(register_report);
    if writing
        && registered
        && let Err(e) = progress(&mut rec, ctx.root, Step::Register, &now)
    {
        stop_failed!(
            Step::Register,
            e,
            "the initialization's progress could not be recorded"
        );
    }

    // Delivery is evaluated after the files are in place, because the whole
    // point is to look at the tree rather than at an intention.
    for rule in delivery::load_rules() {
        report.delivery.push(DeliveryReport {
            harness: rule.harness.clone(),
            verdict: delivery::evaluate(ctx.root, &rule),
        });
    }

    // The six results, after everything else, because `local-checks` runs the
    // rendered gate over the finished tree.
    if let Some(mut plan) = setup {
        if writing {
            plan.results = crate::setup::applied_results(ctx.root, &plan, ctx.setup.verify_local);
        }
        report.setup = Some(plan);
    }

    report.writes.sort();
    report.writes.dedup();
    report.withheld.sort();
    report.adopted.sort();
    report.kept.sort();
    report.mutations = rec.list;
    report.finish(false)
}

/// Record how far the flow got. A record that could not be written is an
/// execution error: section 3.17's "the last completed step is recorded" is
/// part of the contract.
fn progress(rec: &mut Recorder, root: &Path, step: Step, now: &str) -> Result<(), String> {
    let path = resolve(root, project::INIT_STATE);
    rec.around(step.word(), chain(&path), || {
        Progress::write(root, step, now)
    })
    .map_err(|e| format!("{}: {e}", project::INIT_STATE))
}

fn step_home(
    ctx: &Context<'_>,
    rec: &mut Recorder,
    personal: Option<&Personal>,
    tools: &Tools,
) -> Result<(), String> {
    let step = Step::Home.word();
    for dir in ctx.home.directories() {
        rec.around(step, chain(&dir), || std::fs::create_dir_all(&dir))
            .map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    if let Some(personal) = personal {
        let path = ctx.home.personal_file();
        rec.around(step, chain(&path), || personal.write(ctx.home))
            .map_err(|e| e.to_string())?;
    }
    let path = ctx.home.tools_file();
    rec.around(step, chain(&path), || tools.write(ctx.home))
        .map_err(|e| e.to_string())
}

fn step_reconcile(
    ctx: &Context<'_>,
    managed: &[(String, String)],
    manifest: &mut Manifest,
    now: &str,
) -> std::io::Result<Vec<String>> {
    let mut adopted = Vec::new();
    for (path, _) in managed {
        if manifest.records(path) {
            continue;
        }
        let Some((digest, bytes)) = digest_file(&resolve(ctx.root, path))? else {
            continue;
        };
        manifest.upsert(Entry {
            path: path.clone(),
            class: Class::Adopted,
            source: Source {
                kind: SourceKind::Template,
                identity: GOVERNANCE_SOURCE.to_string(),
            },
            digest,
            bytes,
            written_at: now.to_string(),
            transfer: None,
            role: Default::default(),
        });
        adopted.push(path.clone());
    }
    Ok(adopted)
}

fn governance_declaration(managed: &[(String, String)]) -> Declaration {
    Declaration {
        name: GOVERNANCE_SOURCE.to_string(),
        harness: GOVERNANCE_SOURCE.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        files: managed
            .iter()
            .map(|(path, contents)| {
                if producer::is_authored_input(path) {
                    ManagedFile::authored_input(path, contents.as_bytes().to_vec())
                } else {
                    ManagedFile::owned(path, contents.as_bytes().to_vec())
                }
            })
            .collect(),
        unexpressible: Vec::new(),
        prerequisites: Vec::new(),
    }
}

/// Write the planned governance files.
///
/// Every decision about WHAT to write and what to withhold came from spec
/// 002's plan above. What is here is the write itself and the entry it records,
/// and the entry's source is a **template**, because these files are this
/// product's templates and not an agent-harness adapter's.
fn perform_writes(
    root: &Path,
    rec: &mut Recorder,
    computed: &statecraft_environment::plan::Plan,
    manifest: &mut Manifest,
    producer: &dyn Producer,
    now: &str,
) -> std::io::Result<()> {
    let identity = producer.identity().describe();
    for write in &computed.writes {
        let target = resolve(root, &write.path);
        let entry = Entry {
            path: write.path.clone(),
            class: Class::Managed,
            source: Source {
                kind: SourceKind::Template,
                identity: if write.path == project::INSTRUCTIONS {
                    GOVERNANCE_SOURCE.to_string()
                } else {
                    identity.clone()
                },
            },
            digest: write.digest.clone(),
            bytes: write.contents.len() as u64,
            written_at: now.to_string(),
            transfer: manifest.entry(&write.path).and_then(|e| e.transfer.clone()),
            role: write.role,
        };
        // A write whose bytes are already on disk, over an entry that records
        // exactly what this one would, changes nothing: the file is left
        // alone and the entry keeps the time it was really written. Without
        // this a repeated `init apply` re-dated every such entry, so the
        // committed manifest changed on each run although no file did.
        let on_disk = digest_file(&target)?.map(|(d, _)| d);
        let unchanged = on_disk.as_deref() == Some(write.digest.as_str())
            && manifest.entry(&write.path).is_some_and(|recorded| {
                Entry {
                    written_at: recorded.written_at.clone(),
                    ..entry.clone()
                } == *recorded
            });
        if unchanged {
            continue;
        }
        rec.around(Step::Governance.word(), chain(&target), || {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&target, &write.contents)
        })?;
        manifest.upsert(entry);
    }
    Ok(())
}

/// Why the ignore merge stopped the preflight.
enum IgnoreStop {
    /// A precondition: the rules would ignore the project area.
    Refused(ignore::Refusal),
    /// `.gitignore` is there and could not be read.
    Unreadable(String),
}

fn plan_ignore(root: &Path, fragment: Option<&str>) -> Result<IgnorePlan, IgnoreStop> {
    let Some(fragment) = fragment else {
        return Ok(IgnorePlan {
            contents: None,
            detail: "the producer returned no ignore fragment".to_string(),
        });
    };
    let path = root.join(".gitignore");
    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(IgnoreStop::Unreadable(format!(".gitignore: {e}"))),
    };
    let merged = ignore::merge(existing.as_deref(), fragment).map_err(IgnoreStop::Refused)?;
    if merged.unchanged {
        return Ok(IgnorePlan {
            contents: None,
            detail: "ignore rules already carry every pattern".to_string(),
        });
    }
    Ok(IgnorePlan {
        detail: format!("{} ignore pattern(s) merged", merged.added.len()),
        contents: Some(merged.contents_after),
    })
}

/// The generated files this product can recognize by their exact bytes.
///
/// Only what the producer actually returned out of contract. Nothing is
/// vendored, and nothing is recognized on a hardcoded digest.
fn known_generated(starter: &producer::Starter) -> Vec<bridge::KnownGenerated> {
    starter
        .out_of_contract
        .iter()
        .filter(|f| f.rel_path == project::ROOT_INSTRUCTIONS)
        .map(|f| bridge::KnownGenerated {
            label: format!("generated by {}", starter.conformance.producer.describe()),
            contents: f.contents.clone(),
        })
        .collect()
}

fn write_project(
    root: &Path,
    rec: &mut Recorder,
    bridge_plan: &bridge::Plan,
    manifest: &mut Manifest,
    now: &str,
) -> Result<(), String> {
    let step = Step::Project.word();
    if bridge_plan.action.changes_the_file() {
        let path = resolve(root, project::ROOT_INSTRUCTIONS);
        rec.around(step, chain(&path), || {
            std::fs::write(&path, &bridge_plan.contents_after)
        })
        .map_err(|e| format!("{}: {e}", project::ROOT_INSTRUCTIONS))?;
    }
    // Recorded only when this run inserted or moved the line, because the
    // record is the authority removal acts on (spec 002 section 3.13 rule 4).
    // A file that already begins with the line gets no new record: if an
    // earlier run recorded the insertion, that record, with its digest before,
    // is kept as it is; if none did, the line is not this product's, and a
    // record now would let removal take away a line the user wrote.
    if bridge_plan.action.changes_the_file() {
        manifest.upsert_modification(bridge::record(bridge_plan, now));
    }
    let path = resolve(root, project::DECLARATION);
    rec.around(step, chain(&path), || manifest.write(root))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn step_corpus(ctx: &Context<'_>) -> StepState {
    // Spec 002 section 3.23, contract 5: the tool and its `check` are
    // established again before anything runs. The preflight found them; a
    // tool gone since is a late refusal, and nothing in this step was done.
    if let Err(answer) = ctx.corpus.carries_check(ctx.root) {
        return StepState::Refused {
            reason: answer.describe(),
        };
    }
    // `compile` and `index` translated as `check` is: a finding about the
    // corpus is withheld, a verb that did not perform is a failure.
    for (what, ran) in [
        ("compile", ctx.corpus.compile_answer(ctx.root)),
        ("index", ctx.corpus.index_answer(ctx.root)),
    ] {
        match ran {
            Ran::Done => {}
            Ran::Finding(reason) => {
                return StepState::Withheld {
                    reason: format!("{what}: {reason}"),
                };
            }
            Ran::NotPerformed(reason) => {
                return StepState::Failed {
                    reason: format!("{what}: {reason}"),
                };
            }
        }
    }
    // `check`'s answer, translated rather than passed through: 1 and 2 are
    // findings about the corpus (withheld, with the readings named in the
    // text), 3 is a read not performed (failed), and absence is a refusal.
    let answer = ctx.corpus.check_answer(ctx.root);
    match answer {
        CheckAnswer::Fresh => StepState::Done,
        CheckAnswer::DoesNotValidate { .. } | CheckAnswer::Stale { .. } => StepState::Withheld {
            reason: format!("check: {}", answer.describe()),
        },
        CheckAnswer::NotPerformed { .. } => StepState::Failed {
            reason: format!("check: {}", answer.describe()),
        },
        CheckAnswer::Unavailable { .. } => StepState::Refused {
            reason: format!("check: {}", answer.describe()),
        },
    }
}

/// Why step 7 did not register the project.
enum RegisterFailure {
    /// A precondition: spec-spine is absent or lacks `check`.
    Refused(String),
    /// Anything else, including a `check` that did not perform its read.
    Failed(String),
}

fn step_register(
    ctx: &Context<'_>,
    rec: &mut Recorder,
    writing: bool,
) -> Result<(Qualification, String), RegisterFailure> {
    let failed = |e: statecraft_environment::registry::RegistryError| match e {
        e @ statecraft_environment::registry::RegistryError::CorpusCheckUnavailable { .. } => {
            RegisterFailure::Refused(e.to_string())
        }
        e => RegisterFailure::Failed(e.to_string()),
    };
    let mut registry = Registry::read(ctx.home.root()).map_err(failed)?;
    let registration = registry
        .register(ctx.root, ctx.target_probe)
        .map_err(failed)?
        .clone();
    if writing {
        let path = ctx.home.registry_file();
        rec.around(Step::Register.word(), chain(&path), || {
            registry.write(ctx.home.root())
        })
        .map_err(failed)?;
    }
    let detail = format!(
        "{}; armed: {}. Arming and execution are separate explicit acts.",
        registration.qualification.verdict.word(),
        registration.armed
    );
    Ok((registration.qualification, detail))
}

/// The bytes a report would write for a path, for a caller that wants to show
/// a diff. Reads nothing.
pub fn digest_of(contents: &str) -> String {
    digest_bytes(contents.as_bytes())
}
