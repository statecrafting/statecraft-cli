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
    /// What the project requires of the harness, and what the home holds.
    ///
    /// Spec 002 section 3.25's inspection, and spec 006 section 3.11.1's rule
    /// that it activates nothing: no install, no requirement written, no
    /// delivery. Reading the state must not be a way of changing it.
    HarnessShow {
        /// The project.
        root: PathBuf,
    },
    /// Commit the shipped revision as the project's required identity.
    ///
    /// Section 3.25's explicit upgrade, which is the act inspection is not. The
    /// revision is installed under the home first, because a requirement
    /// pointing at a revision the home does not hold cannot stand `exact` on
    /// this machine.
    HarnessUpgrade {
        /// The project.
        root: PathBuf,
    },
    /// The exact managed-session settings bytes, and their identity.
    ///
    /// Section 3.27. No path: the payload is a property of this build and not
    /// of any target.
    SessionPayload,
    /// Write one session's startup record, with the observation absent.
    StartupRecord {
        /// The project.
        root: PathBuf,
        /// The session.
        session_id: String,
    },
    /// Submit captured evidence for admission, and record what it establishes.
    ///
    /// Sections 3.29 and 3.30. The capture directory is read and then judged,
    /// as two steps, so an unreadable capture and a capture that shows no
    /// refusal stay distinguishable: the first is a failure, the second a
    /// refusal (spec 006 section 3.11.2). A refused claim writes nothing.
    StartupQualify {
        /// The project.
        root: PathBuf,
        /// The session.
        session_id: String,
        /// The capture directory the three launches wrote into.
        submission: PathBuf,
    },
    /// Launch one qualification control and record the launch (spec 002
    /// section 3.30 rule 12, spec 006 section 3.11.2).
    StartupCapture {
        /// The project the session runs in.
        root: PathBuf,
        /// Which control.
        control: crate::admission::Control,
        /// Where the record is written.
        directory: PathBuf,
        /// The provider, as named.
        program: String,
        /// The session's deadline.
        deadline_seconds: u64,
        /// Whether the operator stated the program is a local fake.
        synthetic: bool,
        /// The environment the provider runs with, exactly as the caller
        /// supplies it.
        environment: std::collections::BTreeMap<String, String>,
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

/// The upgrade an inspection found, or the reason there is none.
///
/// A named enum rather than a `Result`, because this field is part of spec 006
/// section 3.4's JSON contract and `Result`'s serialization is `Ok`/`Err`,
/// which names nothing a caller would script against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "availability", content = "value")]
pub enum AvailableUpgrade {
    /// What `harness upgrade` would commit.
    Available(Box<crate::required::Upgrade>),
    /// Why it would not.
    Unavailable(Box<crate::required::UpgradeRefusal>),
}

/// What the project requires of the harness, and what this machine holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessStanding {
    /// The project.
    pub root: String,
    /// The inspection: required, installed, shipped, and the standing.
    pub inspection: crate::required::Inspection,
    /// The upgrade this build would offer, or why it would not.
    ///
    /// Carried by the **inspection** because knowing what an upgrade would do
    /// is a read. Performing it is [`Operation::HarnessUpgrade`], and nothing
    /// here performs it.
    pub available_upgrade: AvailableUpgrade,
    /// Whether anything was installed, written or delivered by this call.
    /// Always false: an inspection that activated something would make the
    /// state unreadable without changing it.
    pub activated_anything: bool,
}

/// What an explicit upgrade did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessUpgraded {
    /// The project.
    pub root: String,
    /// What was committed.
    pub upgrade: crate::required::Upgrade,
    /// Harness files this call wrote under the home.
    pub installed: Vec<String>,
    /// Whether the manifest changed on disk.
    pub manifest_written: bool,
    /// The standing that results, read back from what was written.
    pub standing: crate::required::Standing,
}

/// The managed-session settings payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPayload {
    /// The exact bytes the settings argument receives.
    pub payload: String,
    /// Their identity, which an observation is bound to.
    pub digest: String,
    /// The argument that carries them.
    pub argument: String,
    /// Whether this call wrote the payload anywhere. Always false: obtaining
    /// the bytes is a read, and delivering them is a session starting.
    pub delivered: bool,
}

/// What a startup record says, and where it was written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupOutcome {
    /// The project.
    pub root: String,
    /// The session.
    pub session_id: String,
    /// Where the record is.
    pub path: String,
    /// Whether this call wrote it.
    pub written: bool,
    /// The record itself.
    pub record: Box<crate::startup::StartupRecord>,
    /// Whether the session carries the managed-execution claim.
    pub qualified: bool,
}

/// One launched control, as the operation recorded it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureOutcome {
    /// Where the record is.
    pub path: String,
    /// Which control.
    pub control: crate::admission::Control,
    /// Whether the launch completed as one readable session. Says nothing
    /// about the control's outcome, which only the admission judges.
    pub complete: bool,
    /// Why it did not, when it did not.
    pub incomplete: Option<String>,
    /// The record itself.
    pub measurement: Box<crate::admission::Measurement>,
}

/// Why a submitted qualification was not recorded.
///
/// Two reasons, kept apart. A submission that could not be **read** never
/// stated a claim; one that was read and **refused** stated one and was judged.
/// Reporting them the same way would let a typo look like a measured negative,
/// which is exactly the substitution section 3.29 exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum QualificationRefused {
    /// The submission or a file it names could not be read.
    Unread {
        /// What went wrong.
        reason: String,
    },
    /// The claim was judged and did not survive the admission.
    NotAdmitted {
        /// Which rule refused it.
        reason: String,
    },
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
    /// The harness requirement, inspected.
    Harness(Box<HarnessStanding>),
    /// The harness requirement, changed.
    HarnessUpgraded(Box<HarnessUpgraded>),
    /// The managed-session payload.
    SessionPayload(Box<SessionPayload>),
    /// A startup record.
    Startup(Box<StartupOutcome>),
    /// A qualification submission that was not recorded.
    QualificationRefused(Box<QualificationRefused>),
    /// One launched control.
    Captured(Box<CaptureOutcome>),
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
                //
                // A **plan** withholds nothing: writing nothing is what it was
                // asked to do, so naming the modification is success and not a
                // finding. Only an apply that named a modification and did not
                // perform it has withheld a write.
                if change.settings.iter().any(SettingsOutcome::is_failure) {
                    Severity::Failed
                } else if change.settings.iter().any(SettingsOutcome::is_refusal) {
                    Severity::Refused
                } else if change.mode == flow::Mode::Apply
                    && change
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
            // The DIAGNOSTIC question, not the execution one. A project whose
            // requirement is committed, installed and intact disagrees with
            // nothing, and no revision has resolved for it because an
            // inspection is not a session. `permits_managed_execution` would
            // report that healthy project as a finding forever, which is
            // `required::Standing`'s own warning about conflating the two
            // questions. Spec 006 section 3.3: a finding is a diagnostic
            // state, and there is none here.
            Answer::Harness(h) => {
                if h.inspection.standing.disagreement().is_none() {
                    Severity::Ok
                } else {
                    Severity::Finding
                }
            }
            Answer::HarnessUpgraded(u) => {
                if u.standing.disagreement().is_none() {
                    Severity::Ok
                } else {
                    Severity::Finding
                }
            }
            Answer::SessionPayload(_) => Severity::Ok,
            // A record that is not qualified is the ordinary case and it is a
            // finding, not a failure: the session ran, the record says what it
            // establishes, and what it does not establish is the finding.
            Answer::Startup(s) => {
                if s.qualified {
                    Severity::Ok
                } else {
                    Severity::Finding
                }
            }
            // Spec 006 section 3.11.2. A claim the admission refused is a
            // refusal: it was read, judged, and nothing was written. Captures
            // that could not be read are a failure, as an unreadable manifest
            // is: no claim was judged, and reporting it as a refusal would make
            // a missing file look like a measured negative.
            Answer::QualificationRefused(r) => match **r {
                QualificationRefused::NotAdmitted { .. } => Severity::Refused,
                QualificationRefused::Unread { .. } => Severity::Failed,
            },
            // A launch that completed is success whatever the session did; one
            // that did not is recorded and is a finding (spec 006 section
            // 3.11.2).
            Answer::Captured(c) => {
                if c.complete {
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
            Answer::Harness(h) => {
                let mut out = format!("project   {}\n", h.root);
                out.push_str(&format!(
                    "required  {}\n",
                    h.inspection
                        .required_display
                        .as_deref()
                        .unwrap_or("none committed")
                ));
                out.push_str(&format!("shipped   {}\n", h.inspection.shipped));
                for revision in &h.inspection.installed {
                    out.push_str(&format!(
                        "installed {} {}\n",
                        revision.id,
                        match (&revision.unreadable, revision.intact) {
                            (Some(why), _) => format!("unreadable: {why}"),
                            (None, true) => "intact".to_string(),
                            (None, false) => "does not digest to its own name".to_string(),
                        }
                    ));
                }
                out.push_str(&format!("standing  {}\n", h.inspection.standing.describe()));
                out.push_str(&match &h.available_upgrade {
                    AvailableUpgrade::Available(upgrade) => {
                        format!("upgrade   available: {}\n", upgrade.describe())
                    }
                    AvailableUpgrade::Unavailable(why) => {
                        format!("upgrade   unavailable: {why}\n")
                    }
                });
                out.push_str("nothing was installed, written or delivered by this read\n");
                out
            }
            Answer::HarnessUpgraded(u) => {
                let mut out = format!("project   {}\n", u.root);
                out.push_str(&format!("upgrade   {}\n", u.upgrade.describe()));
                out.push_str(&format!(
                    "installed {} harness file(s) under the home\n",
                    u.installed.len()
                ));
                out.push_str(&format!(
                    "manifest  {}\n",
                    if u.manifest_written {
                        "written; commit it, because the requirement is a reviewed project change"
                    } else {
                        "unchanged"
                    }
                ));
                out.push_str(&format!("standing  {}\n", u.standing.describe()));
                out
            }
            // The bytes, and nothing else. This verb exists so an operator can
            // put the payload where the settings argument will read it, and a
            // rendering that appended a digest line would produce a settings
            // file that is not the payload. The digest and the argument are in
            // the `--json` rendering, which is where a caller reads them
            // (spec 006 section 3.4).
            Answer::SessionPayload(p) => p.payload.clone(),
            Answer::Startup(s) => {
                let mut out = format!("session   {}\n", s.session_id);
                out.push_str(&s.record.describe());
                out.push('\n');
                out.push_str(&format!(
                    "{}  {}\n",
                    if s.written { "written " } else { "existing" },
                    s.path
                ));
                out
            }
            Answer::Captured(c) => {
                let m = &c.measurement;
                let mut out = format!("control   {}\n", c.control.word());
                if let Some(l) = &m.launch {
                    if l.origin == crate::admission::Origin::Synthetic {
                        out.push_str(
                            "origin    SYNTHETIC: a local fake, never a live observation\n",
                        );
                    }
                    out.push_str(&format!(
                        "program   {} (probed {})\n",
                        m.invocation.program,
                        l.probe_version.as_deref().unwrap_or("no version")
                    ));
                    out.push_str(&format!(
                        "process   exit {:?}, signal {:?}, timed out {}, survivors {}\n",
                        l.process.code,
                        l.process.signal,
                        l.process.timed_out,
                        l.process.surviving_processes.as_deref().unwrap_or("none")
                    ));
                    out.push_str(&format!(
                        "stdout    {} byte(s)\nstderr    {} byte(s)\n",
                        m.capture.bytes.len(),
                        l.stderr.len()
                    ));
                }
                out.push_str(&match &c.incomplete {
                    None => "session   complete; the admission judges what it shows\n".to_string(),
                    Some(why) => format!("session   incomplete: {why}\n"),
                });
                out.push_str(&format!("record    {}\n", c.path));
                out
            }
            Answer::QualificationRefused(r) => match &**r {
                QualificationRefused::Unread { reason } => format!(
                    "failed: the captures could not be read, so no claim was judged: \
                     {reason}\nnothing was written\n"
                ),
                QualificationRefused::NotAdmitted { reason } => format!(
                    "refused: the claimed observation is not admitted: {reason}\nnothing was \
                     written; the session is unverified, which is the answer and not a \
                     weaker qualification\n"
                ),
            },
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
        Operation::HarnessShow { root } => harness_show(ports, &root),
        Operation::HarnessUpgrade { root } => harness_upgrade(ports, &root),
        Operation::SessionPayload => Answer::SessionPayload(Box::new(SessionPayload {
            payload: crate::session::payload_json(),
            digest: crate::startup::payload_identity(),
            argument: crate::session::SETTINGS_ARGUMENT.to_string(),
            delivered: false,
        })),
        Operation::StartupRecord { root, session_id } => {
            startup_record(ports, &root, &session_id, None)
        }
        Operation::StartupCapture {
            root,
            control,
            directory,
            program,
            deadline_seconds,
            synthetic,
            environment,
        } => startup_capture(
            &root,
            crate::capture::Request {
                root: root.clone(),
                control,
                directory,
                program,
                deadline_seconds,
                origin: if synthetic {
                    crate::admission::Origin::Synthetic
                } else {
                    crate::admission::Origin::Launched
                },
                environment,
            },
        ),
        Operation::StartupQualify {
            root,
            session_id,
            submission,
        } => {
            // A start happens once, and that is a precondition checked before
            // any evidence is read, so a refusal for it is never mistaken for
            // a judgement of the evidence.
            if crate::startup::StartupRecord::path(&root, &session_id).exists() {
                return Answer::Refused {
                    reason: format!(
                        "session {session_id} already has a startup record; a start happens once"
                    ),
                };
            }
            // Two steps, and the failures stay apart. Captures that could not
            // be read never stated a claim; captures that were read and refused
            // stated one and were judged.
            let evidence = match crate::admission::load(&submission) {
                Ok(evidence) => evidence,
                Err(why) => {
                    return Answer::QualificationRefused(Box::new(QualificationRefused::Unread {
                        reason: why.to_string(),
                    }));
                }
            };
            let admitted = crate::admission::admit_in(&evidence, &root)
                .and_then(|()| crate::startup::admit(&evidence));
            match admitted {
                Ok(observation) => startup_record(ports, &root, &session_id, Some(observation)),
                Err(why) => {
                    Answer::QualificationRefused(Box::new(QualificationRefused::NotAdmitted {
                        reason: why.to_string(),
                    }))
                }
            }
        }
    }
}

/// `startup capture`: launch one control in a target and record it.
fn startup_capture(root: &Path, request: crate::capture::Request) -> Answer {
    if let Err(answer) = manifest_of(root) {
        return answer;
    }
    let control = request.control;
    match crate::capture::launch(&request) {
        Ok(launched) => Answer::Captured(Box::new(CaptureOutcome {
            path: launched.path.display().to_string(),
            control,
            complete: launched.incomplete.is_none(),
            incomplete: launched.incomplete,
            measurement: Box::new(launched.measurement),
        })),
        Err(crate::capture::Failed::Refused(why)) => Answer::Refused {
            reason: why.to_string(),
        },
        Err(e) => Answer::Failed {
            reason: e.to_string(),
        },
    }
}

/// The manifest a project holds, or the answer that says it holds none.
fn manifest_of(root: &Path) -> Result<Manifest, Answer> {
    match Manifest::read(root) {
        Ok(Some(manifest)) => Ok(manifest),
        Ok(None) => Err(Answer::Refused {
            reason: format!(
                "{} holds no {}, so it is not a target and has no harness requirement",
                root.display(),
                statecraft_environment::manifest::MANIFEST_PATH
            ),
        }),
        Err(e) => Err(Answer::Failed {
            reason: e.to_string(),
        }),
    }
}

/// `harness show`. A read, and nothing else.
fn harness_show(ports: &Ports<'_>, root: &Path) -> Answer {
    let manifest = match manifest_of(root) {
        Ok(m) => m,
        Err(answer) => return answer,
    };
    let inspection = crate::required::inspect(ports.home, &manifest);
    // What an upgrade WOULD do. Planning is a read; nothing below performs it.
    let available_upgrade =
        match crate::required::plan_upgrade(ports.home, &manifest, &inspection.shipped) {
            Ok(upgrade) => AvailableUpgrade::Available(Box::new(upgrade)),
            Err(why) => AvailableUpgrade::Unavailable(Box::new(why)),
        };
    Answer::Harness(Box::new(HarnessStanding {
        root: root.display().to_string(),
        inspection,
        available_upgrade,
        activated_anything: false,
    }))
}

/// `harness upgrade`. The explicit, reviewed act of section 3.25.
fn harness_upgrade(ports: &Ports<'_>, root: &Path) -> Answer {
    let mut manifest = match manifest_of(root) {
        Ok(m) => m,
        Err(answer) => return answer,
    };
    // The revision is installed first. A requirement pointing at a revision
    // this home does not hold cannot stand `exact` here, and committing one
    // would commit a requirement that is already broken on the machine
    // proposing it.
    let installed = match harness::install(ports.home, &harness::shipped()) {
        Ok(installed) => installed,
        Err(e) => {
            return Answer::Failed {
                reason: e.to_string(),
            };
        }
    };
    let upgrade =
        match crate::required::plan_upgrade(ports.home, &manifest, &installed.revision.digest) {
            Ok(upgrade) => upgrade,
            Err(refusal) => {
                return Answer::Refused {
                    reason: refusal.to_string(),
                };
            }
        };
    let changed = upgrade.from.as_deref() != Some(upgrade.to.as_str());
    crate::required::apply_upgrade(&mut manifest, &upgrade);
    if changed {
        if let Err(e) = manifest.write(root) {
            return Answer::Failed {
                reason: e.to_string(),
            };
        }
    }
    // The standing is read back from what was written, not from what was
    // intended: the two differ exactly when something went wrong quietly.
    let written = match manifest_of(root) {
        Ok(m) => m,
        Err(answer) => return answer,
    };
    let standing =
        crate::required::evaluate(ports.home, &written, crate::required::required_of(&written));
    Answer::HarnessUpgraded(Box::new(HarnessUpgraded {
        root: root.display().to_string(),
        upgrade,
        installed: installed.written,
        manifest_written: changed,
        standing,
    }))
}

/// Assemble and write one session's startup record.
///
/// The observation is passed in, admitted or absent, because this function
/// records and never judges: section 3.29's admission is the judge and it runs
/// before anything reaches here.
fn startup_record(
    ports: &Ports<'_>,
    root: &Path,
    session_id: &str,
    observation: Option<crate::startup::Observation>,
) -> Answer {
    let manifest = match manifest_of(root) {
        Ok(m) => m,
        Err(answer) => return answer,
    };
    let Some(rule) = delivery::load_rules()
        .into_iter()
        .find(|r| r.harness == crate::session::SUPPORTED_HARNESS)
    else {
        return Answer::Failed {
            reason: format!(
                "no documented load rule for {}",
                crate::session::SUPPORTED_HARNESS
            ),
        };
    };
    let verdict = delivery::evaluate(root, &rule);
    let required_digest = crate::required::required_of(&manifest).map(str::to_string);
    let standing = crate::required::evaluate(ports.home, &manifest, required_digest.as_deref());
    let resolved = standing
        .permits_managed_execution()
        .then(|| required_digest.clone())
        .flatten();

    let record = crate::startup::assemble(
        root,
        session_id,
        &rfc3339_utc(ports.clock.now_unix()),
        &manifest,
        crate::startup::AdapterIdentity {
            name: crate::session::SUPPORTED_HARNESS.to_string(),
            harness: crate::session::SUPPORTED_HARNESS.to_string(),
            version: ports.product_version.clone(),
        },
        verdict,
        standing,
        resolved,
        // This verb records; it delivers nothing, and says so rather than
        // implying a supply it did not perform.
        crate::startup::Supply::NotAttempted {
            reason: "this operation reads and records; it performs no delivery, so nothing \
                     about one is established here"
                .to_string(),
        },
        observation.unwrap_or(crate::startup::Observation::NotObserved {
            reason: "no live session was observed for this start".to_string(),
        }),
    );
    let record = match record {
        Ok(record) => record,
        Err(e) => {
            return Answer::Failed {
                reason: e.to_string(),
            };
        }
    };
    let path = crate::startup::StartupRecord::path(root, session_id);
    match record.write(root) {
        Ok(()) => Answer::Startup(Box::new(StartupOutcome {
            root: root.display().to_string(),
            session_id: session_id.to_string(),
            path: path.display().to_string(),
            written: true,
            qualified: record.qualifies(),
            record: Box::new(record),
        })),
        // A start happens once, so a second record for the same session is a
        // precondition that stopped the operation rather than a failure.
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Answer::Refused {
            reason: e.to_string(),
        },
        Err(e) => Answer::Failed {
            reason: e.to_string(),
        },
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
