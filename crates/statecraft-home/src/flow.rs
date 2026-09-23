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

impl Corpus for SpecSpineCommand {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Every step reported done.
    Complete,
    /// Something was withheld or skipped. Never reported as complete.
    Partial,
    /// A precondition stopped it before any write.
    Refused,
}

impl Outcome {
    /// A one-word rendering.
    pub fn word(self) -> &'static str {
        match self {
            Outcome::Complete => "complete",
            Outcome::Partial => "partial",
            Outcome::Refused => "refused",
        }
    }
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
    /// Repository-relative paths written, or that would be.
    pub writes: Vec<String>,
    /// Paths not written, each with its reason.
    pub withheld: Vec<String>,
    /// Paths already there and now depended on rather than rewritten.
    pub adopted: Vec<String>,
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
    /// How it ended.
    pub outcome: Outcome,
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
            out.push_str(&format!(
                "{:<11} {:<9} {}\n",
                step.step.word(),
                step.state.word(),
                step.detail
            ));
        }
        for path in &self.adopted {
            out.push_str(&format!("adopt      {path}\n"));
        }
        for path in &self.writes {
            let verb = if self.mode == Mode::Plan {
                "would write"
            } else {
                "write     "
            };
            out.push_str(&format!("{verb} {path}\n"));
        }
        for path in &self.withheld {
            out.push_str(&format!("withhold   {path}\n"));
        }
        for d in &self.delivery {
            out.push_str(&format!(
                "delivery   {}: {}\n",
                d.harness,
                d.verdict.describe()
            ));
        }
        out.push_str(&format!("{}\n", self.outcome.word()));
        out
    }

    fn finish(mut self) -> Self {
        // Section 3.17: `refused` is a precondition that stopped the flow
        // before any write. A step refused after the governance and project
        // steps completed and wrote their files (section 3.23's translation
        // of an absent producer at step 6, say) refused that step and nothing
        // in it, and the initialization is `partial`: files are in place and
        // a step did not run.
        let completed = |step: Step| {
            self.steps.iter().any(|s| {
                s.step == step
                    && !matches!(
                        s.state,
                        StepState::Refused { .. } | StepState::Failed { .. }
                    )
            })
        };
        let wrote =
            self.mode == Mode::Apply && completed(Step::Governance) && completed(Step::Project);
        let failed = self
            .steps
            .iter()
            .any(|s| matches!(s.state, StepState::Failed { .. }));
        let refused = self
            .steps
            .iter()
            .any(|s| matches!(s.state, StepState::Refused { .. }));
        self.outcome = if failed || (refused && !wrote) {
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
}

/// Compute the initialization. Writes nothing.
pub fn plan(ctx: &Context<'_>) -> Report {
    run(ctx, Mode::Plan)
}

/// Perform the initialization.
pub fn apply(ctx: &Context<'_>) -> Report {
    run(ctx, Mode::Apply)
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
        conformance: None,
        bridge: None,
        delivery: Vec::new(),
        qualification: None,
        outcome: Outcome::Partial,
    };

    // 1. home. The product's own home, never a native agent location: writing
    //    into one of those is `home apply` and an explicit operator act.
    let home_state = step_home(ctx, writing, &now);
    let stop = !home_state.state.done();
    report.steps.push(home_state);
    if stop {
        return report.finish();
    }

    // 2. plan. The producer is asked once, here, and its answer carries every
    //    later step.
    let starter = match producer::produce(ctx.producer) {
        Ok(s) => s,
        Err(e) => {
            report.steps.push(StepReport {
                step: Step::Plan,
                state: StepState::Refused {
                    reason: e.to_string(),
                },
                detail: "the governance producer did not answer".to_string(),
            });
            return report.finish();
        }
    };
    report.conformance = Some(starter.conformance.clone());
    report.steps.push(StepReport {
        step: Step::Plan,
        state: StepState::Done,
        detail: format!(
            "{} governance file(s), {}",
            starter.governance.len(),
            starter.conformance.describe()
        ),
    });

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

    // Every write of the declaration below happens under the manifest lock,
    // taken before the read, so a transfer or an `env apply` recorded in
    // between cannot be erased by this run's write (spec 002 section 3.35).
    let _manifest_lock = if writing {
        match statecraft_environment::manifest::lock(
            ctx.root,
            statecraft_environment::manifest::WRITER_WAIT,
        ) {
            Ok(held) => Some(held),
            Err(e) => {
                // Another writer held it past the wait: nothing was written.
                let busy = matches!(
                    e,
                    statecraft_environment::manifest::ManifestError::Busy { .. }
                );
                let reason = e.to_string();
                report.steps.push(StepReport {
                    step: Step::Reconcile,
                    state: if busy {
                        StepState::Refused { reason }
                    } else {
                        StepState::Failed { reason }
                    },
                    detail: "the declaration could not be locked for writing".to_string(),
                });
                return report.finish();
            }
        }
    } else {
        None
    };
    let mut manifest = match Manifest::read(ctx.root) {
        Ok(Some(m)) => m,
        Ok(None) => Manifest::new(Pins {
            product: ctx.product_version.clone(),
            spec_spine: ctx
                .corpus
                .version()
                .unwrap_or_else(|| NOT_RECORDED.to_string()),
            adapters: Default::default(),
        }),
        Err(e) => {
            report.steps.push(StepReport {
                step: Step::Reconcile,
                state: StepState::Failed {
                    reason: e.to_string(),
                },
                detail: "the declaration could not be read".to_string(),
            });
            return report.finish();
        }
    };

    // 3. reconcile. A contract path already on disk is ADOPTED: recorded with
    //    the digest observed, depended on, and never rewritten. That is what
    //    keeps an authored spec or a hand-tuned configuration from being
    //    treated as a disposable template.
    match step_reconcile(ctx, &managed, &mut manifest, &now) {
        Ok(adopted) => {
            report.adopted = adopted.clone();
            report.steps.push(StepReport {
                step: Step::Reconcile,
                state: StepState::Done,
                detail: format!("{} existing file(s) adopted, none rewritten", adopted.len()),
            });
        }
        Err(e) => {
            report.steps.push(StepReport {
                step: Step::Reconcile,
                state: StepState::Failed {
                    reason: e.to_string(),
                },
                detail: "the project could not be read".to_string(),
            });
            return report.finish();
        }
    }

    // 4. governance. Which paths are written and which are withheld is spec
    //    002's plan, asked rather than restated here.
    let declaration = governance_declaration(&managed);
    let probe = StaticProbe::new().with_harness(GOVERNANCE_SOURCE);
    let computed = match statecraft_environment::plan::plan(
        ctx.root,
        Some(&manifest),
        std::slice::from_ref(&declaration),
        &probe,
        &ForeignClaims::none(),
    ) {
        Ok(p) => p,
        Err(e) => {
            report.steps.push(StepReport {
                step: Step::Governance,
                state: StepState::Failed {
                    reason: e.to_string(),
                },
                detail: "the project could not be planned".to_string(),
            });
            return report.finish();
        }
    };

    for write in &computed.writes {
        report.writes.push(write.path.clone());
    }
    for held in &computed.withheld {
        report
            .withheld
            .push(format!("{}: {}", held.path, held.reason.describe()));
    }

    if writing {
        if let Err(e) = perform_writes(ctx.root, &computed, &mut manifest, ctx.producer, &now) {
            report.steps.push(StepReport {
                step: Step::Governance,
                state: StepState::Failed {
                    reason: e.to_string(),
                },
                detail: "a governance file could not be written".to_string(),
            });
            return report.finish();
        }
    }

    // The ignore fragment is merged, never installed as a file.
    let ignore_detail = match merge_ignore(ctx.root, starter.ignore_fragment.as_deref(), writing) {
        Ok(detail) => detail,
        Err(refusal) => {
            report.steps.push(StepReport {
                step: Step::Governance,
                state: StepState::Refused {
                    reason: refusal.describe(),
                },
                detail: "the ignore rules would take the project area out of version control"
                    .to_string(),
            });
            return report.finish();
        }
    };
    report.steps.push(StepReport {
        step: Step::Governance,
        state: if starter.conformance.conforming {
            StepState::Done
        } else {
            StepState::Withheld {
                reason: starter.conformance.describe(),
            }
        },
        detail: format!(
            "{} write(s), {} withheld, {ignore_detail}",
            computed.writes.len(),
            computed.withheld.len()
        ),
    });

    // 5. project. The declaration and the instruction bridge. The bridge is a
    //    tracked modification of a file this product does not own.
    let known = known_generated(&starter);
    let root_text = std::fs::read_to_string(resolve(ctx.root, project::ROOT_INSTRUCTIONS)).ok();
    let bridge_plan = bridge::plan(root_text.as_deref(), &known);
    if bridge_plan.action.changes_the_file() {
        report.writes.push(project::ROOT_INSTRUCTIONS.to_string());
    }
    report.bridge = Some(bridge_plan.clone());

    let project_state = if writing {
        match write_project(ctx.root, &bridge_plan, &mut manifest, &now) {
            Ok(()) => StepState::Done,
            Err(e) => StepState::Failed { reason: e },
        }
    } else {
        StepState::Done
    };
    let project_done = project_state.done();
    report.steps.push(StepReport {
        step: Step::Project,
        state: project_state,
        detail: format!(
            "declaration and instructions; root bridge {}",
            bridge_plan.action.word()
        ),
    });
    if !project_done {
        return report.finish();
    }
    if writing {
        let _ = Progress::write(ctx.root, Step::Project, &now);
    }

    // 6. corpus. Compile, index, then check. `check` last and never replaced by
    //    a writing verb: a read that repairs what it measures cannot measure it.
    let corpus_state = if writing {
        step_corpus(ctx)
    } else {
        StepState::Done
    };
    let corpus_detail = match &corpus_state {
        StepState::Done if writing => "compiled, indexed and checked".to_string(),
        StepState::Done => "would compile, index and check".to_string(),
        StepState::Withheld { reason } | StepState::Refused { reason } => reason.clone(),
        StepState::Failed { reason } => reason.clone(),
    };
    let corpus_done = corpus_state.done();
    report.steps.push(StepReport {
        step: Step::Corpus,
        state: corpus_state,
        detail: corpus_detail,
    });
    if writing && corpus_done {
        let _ = Progress::write(ctx.root, Step::Corpus, &now);
    }

    // 7. register. Registration and qualification, and then it stops: arming
    //    is a separate act and execution is another one again.
    let register_state = match step_register(ctx, writing) {
        Ok((qualification, detail)) => {
            report.qualification = Some(qualification);
            StepReport {
                step: Step::Register,
                state: StepState::Done,
                detail,
            }
        }
        Err(RegisterFailure::Refused(reason)) => StepReport {
            step: Step::Register,
            state: StepState::Refused { reason },
            detail: "the project was not registered".to_string(),
        },
        Err(RegisterFailure::Failed(reason)) => StepReport {
            step: Step::Register,
            state: StepState::Failed { reason },
            detail: "the project could not be registered".to_string(),
        },
    };
    let registered = register_state.state.done();
    report.steps.push(register_state);
    if writing && registered {
        let _ = Progress::write(ctx.root, Step::Register, &now);
    }

    // Delivery is evaluated after the files are in place, because the whole
    // point is to look at the tree rather than at an intention.
    for rule in delivery::load_rules() {
        report.delivery.push(DeliveryReport {
            harness: rule.harness.clone(),
            verdict: delivery::evaluate(ctx.root, &rule),
        });
    }

    report.writes.sort();
    report.writes.dedup();
    report.withheld.sort();
    report.adopted.sort();
    report.finish()
}

fn step_home(ctx: &Context<'_>, writing: bool, now: &str) -> StepReport {
    if writing {
        for dir in ctx.home.directories() {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                return StepReport {
                    step: Step::Home,
                    state: StepState::Failed {
                        reason: format!("{}: {e}", dir.display()),
                    },
                    detail: "the product home could not be created".to_string(),
                };
            }
        }
        let personal = match Personal::read(ctx.home) {
            Ok(p) => p,
            Err(e) => {
                return StepReport {
                    step: Step::Home,
                    state: StepState::Failed {
                        reason: e.to_string(),
                    },
                    detail: "the personal defaults could not be read".to_string(),
                };
            }
        };
        if !ctx.home.personal_file().exists() {
            if let Err(e) = personal.write(ctx.home) {
                return StepReport {
                    step: Step::Home,
                    state: StepState::Failed {
                        reason: e.to_string(),
                    },
                    detail: "the personal defaults could not be written".to_string(),
                };
            }
        }
        let mut tools = match Tools::read(ctx.home) {
            Ok(t) => t,
            Err(e) => {
                return StepReport {
                    step: Step::Home,
                    state: StepState::Failed {
                        reason: e.to_string(),
                    },
                    detail: "the tools record could not be read".to_string(),
                };
            }
        };
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
        if let Err(e) = tools.write(ctx.home) {
            return StepReport {
                step: Step::Home,
                state: StepState::Failed {
                    reason: e.to_string(),
                },
                detail: "the tools record could not be written".to_string(),
            };
        }
        let _ = Progress::write(ctx.root, Step::Home, now);
    }

    let presence = crate::home::presence(ctx.home);
    StepReport {
        step: Step::Home,
        state: StepState::Done,
        detail: format!(
            "{} ({} harness revision(s)); no account, login or token was consulted",
            presence.root.display(),
            crate::harness::installed_revisions(ctx.home).len()
        ),
    }
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
            .map(|(path, contents)| ManagedFile::owned(path, contents.as_bytes().to_vec()))
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
    computed: &statecraft_environment::plan::Plan,
    manifest: &mut Manifest,
    producer: &dyn Producer,
    now: &str,
) -> std::io::Result<()> {
    let identity = producer.identity().describe();
    for write in &computed.writes {
        let target = resolve(root, &write.path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, &write.contents)?;
        manifest.upsert(Entry {
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
        });
    }
    Ok(())
}

fn merge_ignore(
    root: &Path,
    fragment: Option<&str>,
    writing: bool,
) -> Result<String, ignore::Refusal> {
    let Some(fragment) = fragment else {
        return Ok("the producer returned no ignore fragment".to_string());
    };
    let path = root.join(".gitignore");
    let existing = std::fs::read_to_string(&path).ok();
    let merged = ignore::merge(existing.as_deref(), fragment)?;
    if merged.unchanged {
        return Ok("ignore rules already carry every pattern".to_string());
    }
    if writing {
        std::fs::write(&path, &merged.contents_after).map_err(|_| {
            ignore::Refusal::AreaIgnored {
                line: ".gitignore".to_string(),
                line_number: 0,
            }
        })?;
    }
    Ok(format!("{} ignore pattern(s) merged", merged.added.len()))
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
    bridge_plan: &bridge::Plan,
    manifest: &mut Manifest,
    now: &str,
) -> Result<(), String> {
    if bridge_plan.action.changes_the_file() {
        std::fs::write(
            resolve(root, project::ROOT_INSTRUCTIONS),
            &bridge_plan.contents_after,
        )
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
    manifest.write(root).map(|_| ()).map_err(|e| e.to_string())
}

fn step_corpus(ctx: &Context<'_>) -> StepState {
    // Spec 002 section 3.23, contract 5: the tool and its `check` are
    // established before anything runs, so a binary that is absent or lacks
    // the verb is a refusal and nothing in this step was done.
    if let Err(answer) = ctx.corpus.carries_check(ctx.root) {
        return StepState::Refused {
            reason: answer.describe(),
        };
    }
    for (what, result) in [
        ("compile", ctx.corpus.compile(ctx.root)),
        ("index", ctx.corpus.index(ctx.root)),
    ] {
        if let Err(reason) = result {
            return StepState::Withheld {
                reason: format!("{what}: {reason}"),
            };
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
        registry.write(ctx.home.root()).map_err(failed)?;
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
