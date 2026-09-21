//! The typed operation boundary.
//!
//! Spec 002 section 3.20. Every operation is a typed request and a typed
//! outcome here. The command surface is **one caller** of it; a local dashboard
//! is a second caller of the same operations, and implements no second settings
//! engine, no second scheduler and no second policy model.
//!
//! No user interface is implemented in this repository. `F-04` stands, and what
//! this module claims is the boundary a dashboard would call and nothing above
//! it.
//!
//! No background loop is added here. Every operation is synchronous, performed
//! by its caller, and one run has one execution owner: the local runner owns
//! local processes, and the platform, when a project is enrolled, coordinates
//! team authority.

use crate::authority::{self, Resolution, RunChoices};
use crate::delivery::{self, NativePlan};
use crate::derived;
use crate::flow::{self, Corpus};
use crate::harness;
use crate::home::{Layout, Personal, Tools};
use crate::project;
use crate::settings::{self, SettingsOutcome};
use crate::team::{self, CoordinationAuthority, Eligibility, LocalApproval, LocalApprovals};
use serde::Serialize;
use statecraft_environment::manifest::{Enrollment, Manifest, Project};
use statecraft_environment::qualify::TargetProbe;
use statecraft_environment::time::{Clock, rfc3339_utc};
use std::path::{Path, PathBuf};

/// What a caller asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// The resolved global environment. Reads only.
    HomeShow,
    /// What `home apply` would write, inside the home and outside it.
    HomePlan,
    /// Create or repair the home, and perform native delivery.
    ///
    /// The settings modification of spec 002 section 3.24 is carried as an
    /// intent rather than performed by default: [`crate::settings::Intent`]'s
    /// default is `Withheld`, so this verb shows the modification and writes
    /// nothing unless the operator consented to that exact content.
    HomeApply {
        /// What the operator asked for, about the settings modification.
        settings: crate::settings::Intent,
    },
    /// Every project change initialization would make.
    InitPlan {
        /// The project.
        root: PathBuf,
    },
    /// Perform it.
    InitApply {
        /// The project.
        root: PathBuf,
    },
    /// The one-time relocation, previewed.
    MigratePlan {
        /// The project.
        root: PathBuf,
    },
    /// The one-time relocation, performed.
    MigrateApply {
        /// The project.
        root: PathBuf,
    },
    /// Record team enrollment in the project declaration.
    Enroll {
        /// The project.
        root: PathBuf,
        /// The team.
        team: String,
    },
    /// Remove it.
    Unenroll {
        /// The project.
        root: PathBuf,
    },
    /// The resolved configuration for a run in that project.
    ConfigShow {
        /// The project.
        root: PathBuf,
        /// The base revision the trusted declaration is read at.
        base_revision: String,
        /// Explicit choices for this invocation.
        choices: RunChoices,
    },
    /// Record a local approval for one subject.
    ApprovalGrant {
        /// The project.
        root: PathBuf,
        /// The subject.
        subject: String,
        /// Who is approving.
        operator: String,
        /// Why.
        reason: String,
    },
    /// The eligibility of one subject, and the authority behind it.
    ApprovalShow {
        /// The project.
        root: PathBuf,
        /// The subject.
        subject: String,
    },
}

/// How an outcome should end a process.
///
/// The command surface maps these onto spec 006 section 3.3's closed
/// vocabulary. Deciding the severity here rather than in the binding is what
/// keeps the binding from carrying a rule an owning spec did not state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// Did what was asked and found nothing wrong.
    Ok,
    /// Ran and reports a finding.
    Finding,
    /// A precondition stopped it.
    Refused,
    /// Something nobody asked for went wrong.
    Failed,
}

/// The state of the global environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeStatus {
    /// The home's root.
    pub root: String,
    /// True when nothing is missing.
    pub complete: bool,
    /// What is missing, if anything.
    pub missing: Vec<String>,
    /// The operator's defaults.
    pub personal: Personal,
    /// Installed tools and recorded harness revisions.
    pub tools: Tools,
    /// Harness revisions actually present under the home.
    pub harness_revisions: Vec<String>,
    /// The revision this build would install.
    pub shipped_revision: String,
    /// What native delivery looks like right now, per agent home.
    pub delivery: Vec<NativePlan>,
    /// Whether anything about the platform was consulted. Always false.
    pub platform_consulted: bool,
}

/// What `home apply` did, or would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeChange {
    /// Preview or performance.
    pub mode: flow::Mode,
    /// The home's root.
    pub root: String,
    /// The harness revision installed, or that would be.
    pub revision: String,
    /// Harness files written by this call.
    pub written: Vec<String>,
    /// Harness files already present with the same bytes.
    pub unchanged: Vec<String>,
    /// Native delivery, per agent home.
    pub delivery: Vec<NativePlan>,
    /// Links actually created.
    pub linked: Vec<String>,
    /// Paths deliberately left alone, each with its reason.
    pub preserved: Vec<String>,
    /// The consented settings modification, per native home.
    ///
    /// Section 3.24. Present in a plan as well as in an apply, because the
    /// modification has to be named in the plan before anything is written.
    pub settings: Vec<SettingsOutcome>,
}

/// What an enrollment change did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrollmentChange {
    /// The project.
    pub root: String,
    /// What it is now.
    pub enrollment: Enrollment,
    /// True when this call changed it.
    pub changed: bool,
}

/// What an approval call answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalOutcome {
    /// The project.
    pub root: String,
    /// The subject.
    pub subject: String,
    /// Whether a local record was written by this call.
    pub recorded: bool,
    /// Why no local record was written, when that is the answer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refused: Option<String>,
    /// The eligibility, and the authority behind it.
    pub eligibility: Eligibility,
    /// What answered for the team, in words.
    pub authority: String,
}

/// What an operation answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "operation", content = "value")]
pub enum Answer {
    /// The global environment.
    Home(Box<HomeStatus>),
    /// A change to it.
    HomeChange(Box<HomeChange>),
    /// An initialization.
    Init(Box<flow::Report>),
    /// A relocation.
    Migrate(Box<derived::Outcome>),
    /// An enrollment change.
    Enrollment(Box<EnrollmentChange>),
    /// A resolved configuration.
    Configuration(Box<Resolution>),
    /// An approval or an eligibility.
    Approval(Box<ApprovalOutcome>),
    /// A precondition stopped the operation.
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

impl Answer {
    /// How this should end a process.
    pub fn severity(&self) -> Severity {
        match self {
            Answer::Refused { .. } => Severity::Refused,
            Answer::Failed { .. } => Severity::Failed,
            Answer::Home(status) => {
                if status.complete {
                    Severity::Ok
                } else {
                    Severity::Finding
                }
            }
            Answer::HomeChange(change) => {
                // Spec 006 section 3.3: a withheld write is a finding, a
                // precondition that stopped something is a refusal, and
                // something nobody asked for is a failure. The settings
                // modification is the one part of this verb that can be any of
                // the three, and saying so is what makes it scriptable.
                if change.settings.iter().any(SettingsOutcome::is_failure) {
                    Severity::Failed
                } else if change.settings.iter().any(SettingsOutcome::is_refusal) {
                    Severity::Refused
                } else if change
                    .settings
                    .iter()
                    .any(SettingsOutcome::is_withheld_write)
                {
                    Severity::Finding
                } else {
                    Severity::Ok
                }
            }
            Answer::Init(report) => match report.outcome {
                flow::Outcome::Complete => Severity::Ok,
                flow::Outcome::Partial => Severity::Finding,
                flow::Outcome::Refused => Severity::Refused,
            },
            Answer::Migrate(outcome) => match outcome.verdict {
                derived::Verdict::Refused { .. } => Severity::Refused,
                derived::Verdict::NotNeeded { .. } => Severity::Ok,
                derived::Verdict::Planned { .. } => Severity::Ok,
            },
            Answer::Enrollment(_) => Severity::Ok,
            Answer::Configuration(resolution) => {
                if resolution.has_findings() {
                    Severity::Finding
                } else {
                    Severity::Ok
                }
            }
            Answer::Approval(outcome) => {
                if outcome.refused.is_some() {
                    Severity::Refused
                } else if outcome.eligibility.eligible() {
                    Severity::Ok
                } else {
                    Severity::Finding
                }
            }
        }
    }

    /// A human-readable rendering.
    pub fn render(&self) -> String {
        match self {
            Answer::Home(s) => {
                let mut out = format!(
                    "home {}\n{} harness revision(s) present; this build ships {}\n",
                    s.root,
                    s.harness_revisions.len(),
                    s.shipped_revision
                );
                for missing in &s.missing {
                    out.push_str(&format!("missing {missing}\n"));
                }
                for plan in &s.delivery {
                    out.push_str(&format!("{} {}\n", plan.harness, plan.home));
                    for action in &plan.actions {
                        out.push_str(&format!("  {}\n", action.describe()));
                    }
                }
                out.push_str("no account, login, token or hosted connection was consulted\n");
                out
            }
            Answer::HomeChange(c) => {
                let mut out = format!("home {}\nharness {}\n", c.root, c.revision);
                for path in &c.written {
                    out.push_str(&format!("write {path}\n"));
                }
                for path in &c.linked {
                    out.push_str(&format!("link {path}\n"));
                }
                for note in &c.preserved {
                    out.push_str(&format!("preserve {note}\n"));
                }
                for outcome in &c.settings {
                    out.push_str(&outcome.render());
                }
                out
            }
            Answer::Init(r) => r.render(),
            Answer::Migrate(m) => {
                let mut out = m.verdict.render();
                for path in &m.moved {
                    out.push_str(&format!("moved {path}\n"));
                }
                for file in &m.rewritten {
                    out.push_str(&format!("rewrote {file}\n"));
                }
                out
            }
            Answer::Enrollment(e) => match &e.enrollment {
                Enrollment::Solo => format!("{}: solo\n", e.root),
                Enrollment::Team { team } => format!("{}: enrolled in {team}\n", e.root),
            },
            Answer::Configuration(r) => r.render(),
            Answer::Approval(a) => {
                let mut out = format!("{}: {}\n", a.subject, a.eligibility.describe());
                if let Some(reason) = &a.refused {
                    out.push_str(&format!("refused: {reason}\n"));
                }
                out.push_str(&format!("authority: {}\n", a.authority));
                out
            }
            Answer::Refused { reason } => format!("refused: {reason}\n"),
            Answer::Failed { reason } => format!("failed: {reason}\n"),
        }
    }
}

/// Everything an operation needs, supplied rather than discovered.
pub struct Ports<'a> {
    /// The product home.
    pub home: &'a Layout,
    /// Where governance starter files come from.
    pub producer: &'a dyn crate::producer::Producer,
    /// How the corpus is compiled and checked.
    pub corpus: &'a dyn Corpus,
    /// How a project is qualified.
    pub target_probe: &'a dyn TargetProbe,
    /// The coordination authority a team project defers to.
    pub authority: &'a dyn CoordinationAuthority,
    /// How a file is read at a revision.
    pub revisions: &'a dyn authority::RevisionReader,
    /// The clock.
    pub clock: &'a dyn Clock,
    /// The parent directory native agent homes hang off.
    ///
    /// Explicit rather than read from the environment inside the operation, so
    /// a caller always states where delivery would write and a test stays out
    /// of the operator's real home without mutating a process-wide variable.
    pub native_root: PathBuf,
    /// This build's version.
    pub product_version: String,
}

/// Perform one operation.
pub fn execute(ports: &Ports<'_>, operation: Operation) -> Answer {
    match operation {
        Operation::HomeShow => home_show(ports),
        Operation::HomePlan => home_change(ports, flow::Mode::Plan, settings::Intent::Withheld),
        Operation::HomeApply { settings } => home_change(ports, flow::Mode::Apply, settings),
        Operation::InitPlan { root } => init(ports, &root, flow::Mode::Plan),
        Operation::InitApply { root } => init(ports, &root, flow::Mode::Apply),
        Operation::MigratePlan { root } => match derived::plan(&root) {
            Ok(verdict) => Answer::Migrate(Box::new(derived::Outcome {
                verdict,
                moved: Vec::new(),
                rewritten: Vec::new(),
            })),
            Err(e) => Answer::Failed {
                reason: e.to_string(),
            },
        },
        Operation::MigrateApply { root } => match derived::apply(&root) {
            Ok(outcome) => Answer::Migrate(Box::new(outcome)),
            Err(e) => Answer::Failed {
                reason: e.to_string(),
            },
        },
        Operation::Enroll { root, team } => enrollment(ports, &root, Enrollment::Team { team }),
        Operation::Unenroll { root } => enrollment(ports, &root, Enrollment::Solo),
        Operation::ConfigShow {
            root,
            base_revision,
            choices,
        } => config_show(ports, &root, &base_revision, &choices),
        Operation::ApprovalGrant {
            root,
            subject,
            operator,
            reason,
        } => approval_grant(ports, &root, &subject, &operator, &reason),
        Operation::ApprovalShow { root, subject } => approval_show(ports, &root, &subject),
    }
}

fn home_show(ports: &Ports<'_>) -> Answer {
    let presence = crate::home::presence(ports.home);
    let personal = match Personal::read(ports.home) {
        Ok(p) => p,
        Err(e) => {
            return Answer::Failed {
                reason: e.to_string(),
            };
        }
    };
    let tools = match Tools::read(ports.home) {
        Ok(t) => t,
        Err(e) => {
            return Answer::Failed {
                reason: e.to_string(),
            };
        }
    };
    let shipped = harness::revision_of(&harness::shipped());
    let revision_root = ports.home.harness_revision_dir(&shipped.id);
    let delivery = delivery::native_homes_under(&ports.native_root)
        .iter()
        .map(|home| delivery::native_plan(home, &revision_root, &harness::shipped()))
        .collect();

    let mut missing = presence.missing_directories.clone();
    missing.extend(presence.missing_files.clone());
    Answer::Home(Box::new(HomeStatus {
        root: presence.root.display().to_string(),
        complete: presence.complete(),
        missing,
        personal,
        tools,
        harness_revisions: harness::installed_revisions(ports.home),
        shipped_revision: shipped.id,
        delivery,
        // Nothing above asked a platform anything, and this field is here so
        // that a report says so rather than leaving a reader to infer it.
        platform_consulted: false,
    }))
}

fn home_change(ports: &Ports<'_>, mode: flow::Mode, intent: settings::Intent) -> Answer {
    let writing = mode == flow::Mode::Apply;
    let shipped = harness::shipped();
    let revision = harness::revision_of(&shipped);
    let revision_root = ports.home.harness_revision_dir(&revision.id);

    let (written, unchanged) = if writing {
        for dir in ports.home.directories() {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                return Answer::Failed {
                    reason: format!("{}: {e}", dir.display()),
                };
            }
        }
        match harness::install(ports.home, &shipped) {
            Ok(installed) => (installed.written, installed.unchanged),
            Err(e) => {
                return Answer::Failed {
                    reason: e.to_string(),
                };
            }
        }
    } else {
        (
            shipped.iter().map(|f| f.rel_path.clone()).collect(),
            Vec::new(),
        )
    };

    if writing {
        let mut tools = match Tools::read(ports.home) {
            Ok(t) => t,
            Err(e) => {
                return Answer::Failed {
                    reason: e.to_string(),
                };
            }
        };
        tools.record_revision(&revision.id);
        if let Err(e) = tools.write(ports.home) {
            return Answer::Failed {
                reason: e.to_string(),
            };
        }
        if !ports.home.personal_file().exists() {
            if let Err(e) = Personal::default().write(ports.home) {
                return Answer::Failed {
                    reason: e.to_string(),
                };
            }
        }
        for file in [ports.home.delivery_file(), ports.home.modifications_file()] {
            if !file.exists() {
                if let Err(e) = std::fs::write(&file, "[]\n") {
                    return Answer::Failed {
                        reason: e.to_string(),
                    };
                }
            }
        }
    }

    let mut plans = Vec::new();
    let mut linked = Vec::new();
    let mut preserved = Vec::new();
    for home in delivery::native_homes_under(&ports.native_root) {
        // A native home this operator does not have is not created: placing a
        // `.claude/` beside a harness that is not installed would be this
        // product inventing an agent configuration.
        if !home.root.is_dir() {
            preserved.push(format!(
                "{}: no {} directory, so nothing is delivered there",
                home.harness,
                home.root.display()
            ));
            continue;
        }
        let plan = delivery::native_plan(&home, &revision_root, &shipped);
        for action in &plan.actions {
            if let delivery::NativeAction::Skipped { at, reason } = action {
                preserved.push(format!("{at}: {reason}"));
            }
        }
        if writing {
            match delivery::native_apply(&plan) {
                Ok(mut done) => linked.append(&mut done),
                Err(e) => {
                    return Answer::Failed {
                        reason: e.to_string(),
                    };
                }
            }
        }
        plans.push(plan);
    }

    // Section 3.24, and section 3.14 rule 4: the one write into a harness's own
    // settings file happens under this verb and nowhere else, and only when the
    // operator consented to that exact content. A plan computes it and writes
    // nothing, which is what "named in the plan before anything is written"
    // requires.
    let settings_outcomes: Vec<SettingsOutcome> = delivery::native_homes_under(&ports.native_root)
        .iter()
        .map(|home| {
            let intent = if writing {
                intent.clone()
            } else {
                settings::Intent::Withheld
            };
            settings::perform(
                ports.home,
                home,
                &intent,
                &rfc3339_utc(ports.clock.now_unix()),
            )
        })
        .collect();

    if writing {
        let record = delivery::Record {
            harness: plans
                .iter()
                .map(|p| p.harness.clone())
                .collect::<Vec<_>>()
                .join(","),
            home: plans
                .iter()
                .map(|p| p.home.clone())
                .collect::<Vec<_>>()
                .join(","),
            revision: revision.id.clone(),
            links: linked.clone(),
            recorded_at: rfc3339_utc(ports.clock.now_unix()),
        };
        let json = serde_json::to_string_pretty(&vec![record]).unwrap_or_else(|_| "[]".into());
        if let Err(e) = std::fs::write(ports.home.delivery_file(), format!("{json}\n")) {
            return Answer::Failed {
                reason: e.to_string(),
            };
        }
    }

    Answer::HomeChange(Box::new(HomeChange {
        mode,
        root: ports.home.root().display().to_string(),
        revision: revision.id,
        written,
        unchanged,
        delivery: plans,
        linked,
        preserved,
        settings: settings_outcomes,
    }))
}

fn init(ports: &Ports<'_>, root: &Path, mode: flow::Mode) -> Answer {
    let ctx = flow::Context {
        home: ports.home,
        root,
        producer: ports.producer,
        corpus: ports.corpus,
        target_probe: ports.target_probe,
        clock: ports.clock,
        product_version: ports.product_version.clone(),
    };
    let report = match mode {
        flow::Mode::Plan => flow::plan(&ctx),
        flow::Mode::Apply => flow::apply(&ctx),
    };
    Answer::Init(Box::new(report))
}

fn enrollment(ports: &Ports<'_>, root: &Path, next: Enrollment) -> Answer {
    let mut manifest = match Manifest::read(root) {
        Ok(Some(m)) => m,
        Ok(None) => {
            return Answer::Refused {
                reason: format!(
                    "{} has no {}; enrollment is recorded in the project's own declaration",
                    root.display(),
                    project::DECLARATION
                ),
            };
        }
        Err(e) => {
            return Answer::Failed {
                reason: e.to_string(),
            };
        }
    };
    let changed = manifest.project.enrollment != next;
    manifest.project.enrollment = next.clone();
    let _ = ports;
    if changed {
        if let Err(e) = manifest.write(root) {
            return Answer::Failed {
                reason: e.to_string(),
            };
        }
    }
    Answer::Enrollment(Box::new(EnrollmentChange {
        root: root.display().to_string(),
        enrollment: next,
        changed,
    }))
}

fn project_of(root: &Path) -> Result<Project, Answer> {
    match Manifest::read(root) {
        Ok(Some(m)) => Ok(m.project),
        Ok(None) => Err(Answer::Refused {
            reason: format!(
                "{} has no {}; initialize it first",
                root.display(),
                project::DECLARATION
            ),
        }),
        Err(e) => Err(Answer::Failed {
            reason: e.to_string(),
        }),
    }
}

fn config_show(
    ports: &Ports<'_>,
    root: &Path,
    base_revision: &str,
    choices: &RunChoices,
) -> Answer {
    let candidate = match project_of(root) {
        Ok(p) => p,
        Err(answer) => return answer,
    };
    let personal = match Personal::read(ports.home) {
        Ok(p) => p,
        Err(e) => {
            return Answer::Failed {
                reason: e.to_string(),
            };
        }
    };
    let trusted = authority::trusted(root, base_revision, ports.revisions, &candidate);
    let team = team::team_answer(&trusted.project, ports.authority);
    Answer::Configuration(Box::new(authority::resolve(
        &personal, &trusted, &team, choices,
    )))
}

fn approval_grant(
    ports: &Ports<'_>,
    root: &Path,
    subject: &str,
    operator: &str,
    reason: &str,
) -> Answer {
    let project = match project_of(root) {
        Ok(p) => p,
        Err(answer) => return answer,
    };
    if let Err(refused) = team::may_grant_locally(&project, subject) {
        let eligibility = team::eligibility(&project, subject, None, ports.authority);
        return Answer::Approval(Box::new(ApprovalOutcome {
            root: root.display().to_string(),
            subject: subject.to_string(),
            recorded: false,
            refused: Some(refused.reason),
            eligibility,
            authority: ports.authority.describe(),
        }));
    }

    let mut approvals = match LocalApprovals::read(ports.home, root) {
        Ok(a) => a,
        Err(e) => {
            return Answer::Failed {
                reason: e.to_string(),
            };
        }
    };
    approvals.grant(LocalApproval {
        subject: subject.to_string(),
        operator: operator.to_string(),
        reason: reason.to_string(),
        recorded_at: rfc3339_utc(ports.clock.now_unix()),
    });
    if let Err(e) = approvals.write(ports.home, root) {
        return Answer::Failed {
            reason: e.to_string(),
        };
    }
    let eligibility = team::eligibility(&project, subject, approvals.get(subject), ports.authority);
    Answer::Approval(Box::new(ApprovalOutcome {
        root: root.display().to_string(),
        subject: subject.to_string(),
        recorded: true,
        refused: None,
        eligibility,
        authority: ports.authority.describe(),
    }))
}

fn approval_show(ports: &Ports<'_>, root: &Path, subject: &str) -> Answer {
    let project = match project_of(root) {
        Ok(p) => p,
        Err(answer) => return answer,
    };
    let approvals = match LocalApprovals::read(ports.home, root) {
        Ok(a) => a,
        Err(e) => {
            return Answer::Failed {
                reason: e.to_string(),
            };
        }
    };
    let eligibility = team::eligibility(&project, subject, approvals.get(subject), ports.authority);
    Answer::Approval(Box::new(ApprovalOutcome {
        root: root.display().to_string(),
        subject: subject.to_string(),
        recorded: false,
        refused: None,
        eligibility,
        authority: ports.authority.describe(),
    }))
}
