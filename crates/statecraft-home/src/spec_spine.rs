//! Which `spec-spine` executable this product runs, and which one a managed
//! session's hooks are handed.
//!
//! Spec 002 section 5, 2026-09-25: **one environment variable selects the
//! spec-spine binary**, amending section 3.23 contract 2 rules 2 and 5. The
//! variable is [`ENV`], `STATECRAFT_SPEC_SPINE`, and the precedence is the
//! delivered hooks' own, so a verb this product runs and a hook it delivers
//! choose the same binary from the same inputs:
//!
//! 1. **In a managed session** (the launch named a run in
//!    `STATECRAFT_RUN_ID`), the value is the supervisor's resolved path and the
//!    only candidate. It is not put to the pin: its identity is the
//!    supervisor's resolution, not a version string.
//! 2. **Outside one**, a non-empty value is the operator's override: the only
//!    candidate, put to the pin, and with no fallback when it names no
//!    executable or one the repository does not admit.
//! 3. **Otherwise** the convention candidates, as spec 028 section 3.2 orders
//!    them: the engine spec-spine's own launcher resolves for the repository
//!    (asked with `launcher resolve --json`, never trusted to run it), the
//!    repository-local `.tooling/bin/spec-spine`, the repository's own
//!    `target/release/spec-spine`, and then the first `spec-spine` on `PATH`
//!    that is not the launcher. The first the pin admits is selected, and
//!    each one passed over is named. An unpinned repository takes the first
//!    candidate.
//! 4. **The retired name, [`RETIRED`], is reported and never read**: when it is
//!    set and [`ENV`] is not, a notice says it was ignored and names the new
//!    one.
//!
//! Resolution never acquires (spec 028 section 3.5): it reads the filesystem
//! and runs `--version` and the launcher's resolution query, nothing else.
//! Every selected binary carries the SHA-256 of its bytes, so what is reported
//! is the file that runs.
//!
//! The supervisor's own selection is [`for_supervisor`]: rule 3 alone. It
//! reads neither variable from the operator's environment, so an operator's
//! shell value cannot reach a managed hook, and [`managed_binding`] places the
//! result in the constructed environment over any value already there.

use crate::flow::{Corpus, SpecSpineCommand};
use statecraft_environment::probe::{CheckAnswer, CommandProbe, Unavailability, names_pin_refusal};
use statecraft_environment::qualify::{CorpusState, TargetProbe};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The one variable that selects the binary.
pub const ENV: &str = "STATECRAFT_SPEC_SPINE";

/// The retired name: reported as ignored, never read as a selection.
pub const RETIRED: &str = "SPEC_SPINE_BIN";

/// The variable whose presence makes a session a managed one.
pub const MANAGED: &str = crate::launch::ENV_RUN;

/// The executable's file name.
pub const PROGRAM: &str = "spec-spine";

/// The `tool` spec-spine's launcher names in its envelope.
pub const LAUNCHER: &str = "spec-spine-launcher";

/// The guard a managed launch is refused under when candidates exist and the
/// project admits none of them.
pub const SELECTION_GUARD: &str = "spec-spine-selection";

/// Which rule selected the binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// The supervisor's path, in a managed session.
    Supervisor,
    /// The operator's override, outside one.
    Override,
    /// The engine spec-spine's launcher resolved for the repository.
    Launcher,
    /// The repository-local install, `.tooling/bin/spec-spine`.
    RepositoryLocal,
    /// The repository's own `target/release/spec-spine`.
    RepositoryBuild,
    /// The first on `PATH` that is not the launcher.
    Path,
}

impl Rule {
    /// The word a report carries.
    pub fn word(self) -> &'static str {
        match self {
            Rule::Supervisor => "supervisor",
            Rule::Override => "override",
            Rule::Launcher => "launcher",
            Rule::RepositoryLocal => "repository-local",
            Rule::RepositoryBuild => "repository-build",
            Rule::Path => "path",
        }
    }
}

/// A selected binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selected {
    /// Its path, absolute.
    pub program: PathBuf,
    /// The rule that chose it.
    pub rule: Rule,
    /// What it reported to `--version`, when it answered.
    pub version: Option<String>,
    /// `sha256:<hex>` of its bytes, read when it was selected; `None` only
    /// when the file could not be read.
    pub digest: Option<String>,
}

impl Selected {
    fn new(program: &Path, rule: Rule, version: Option<String>) -> Self {
        Self {
            digest: digest_of(program),
            program: absolute(program),
            rule,
            version,
        }
    }

    /// One line naming the selection: program, rule, version and digest.
    pub fn describe(&self) -> String {
        format!(
            "{} (rule {}, reports {}, {})",
            self.program.display(),
            self.rule.word(),
            self.version.as_deref().unwrap_or("no version"),
            self.digest.as_deref().unwrap_or("digest unreadable")
        )
    }
}

/// Why nothing was selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unselected {
    /// No candidate exists at all.
    Absent,
    /// A candidate exists and none may judge: an override that names no
    /// executable or one the pin refuses, no compatible convention candidate,
    /// or a probe that failed. The sentence names the binary, its version,
    /// the pin and the remedy.
    Refused(String),
}

/// The answer, with what the selection passed over and what it ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// The binary, or why there is none.
    pub outcome: Result<Selected, Unselected>,
    /// Every convention candidate passed over, one sentence each.
    pub passed_over: Vec<String>,
    /// The candidates put to the pin and passed over, in order, each with
    /// what it reported: what `doctor` names as observed when nothing was
    /// selected, taken from this resolution and never from a separate probe.
    pub considered: Vec<Selected>,
    /// Notices that change nothing, such as the retired name being ignored.
    pub notices: Vec<String>,
}

impl Selection {
    /// Every sentence the selection has for an operator, in order.
    pub fn remarks(&self) -> Vec<String> {
        self.notices
            .iter()
            .chain(self.passed_over.iter())
            .cloned()
            .collect()
    }

    /// The selected binary, or the sentence a refusal carries: the pin, every
    /// candidate passed over and the preparation command (spec 028 section
    /// 3.5).
    pub fn judge(&self) -> Result<&Selected, String> {
        match &self.outcome {
            Ok(selected) => Ok(selected),
            Err(why) => Err(NotSelected::from(why, self).answer().describe()),
        }
    }
}

/// The selection for an operation that names no repository: nothing is
/// resolved, and every spec-spine question it would ask is answered as
/// unavailable, naming why.
pub fn without_repository() -> Selection {
    Selection {
        outcome: Err(Unselected::Refused(
            "this operation names no repository, so no spec-spine is resolved for it".to_string(),
        )),
        passed_over: Vec::new(),
        considered: Vec::new(),
        notices: Vec::new(),
    }
}

/// Select for an operation this product runs in `root`, reading the
/// environment through `var`.
pub fn select(root: &Path, var: &dyn Fn(&str) -> Option<String>) -> Selection {
    let set = |name: &str| var(name).filter(|v| !v.is_empty());
    let mut notices = Vec::new();
    let chosen = set(ENV);
    if let Some(old) = set(RETIRED)
        && chosen.is_none()
    {
        notices.push(format!(
            "ignored {RETIRED}={}: that name is retired and selects nothing; set {ENV} to \
             choose the binary",
            old.replace(['\n', '\r'], " ")
        ));
    }
    let pin = Pin::of(root);
    if let Some(value) = chosen {
        let path = PathBuf::from(&value);
        let outcome = if set(MANAGED).is_some() {
            if executable(&path) {
                Ok(Selected::new(&path, Rule::Supervisor, version_of(&path)))
            } else {
                Err(Unselected::Refused(format!(
                    "the supervisor's binary {ENV}={value} is not an executable, and no other \
                     binary is consulted in a managed session"
                )))
            }
        } else {
            override_outcome(root, &pin, &value)
        };
        return Selection {
            outcome,
            passed_over: Vec::new(),
            considered: Vec::new(),
            notices,
        };
    }
    let mut selection = conventions(root, &pin, set("PATH").as_deref());
    selection.notices = notices;
    selection
}

/// [`select`] against this process's environment.
pub fn select_here(root: &Path) -> Selection {
    select(root, &|name| std::env::var(name).ok())
}

/// The supervisor's selection for a managed session in `root`: the
/// convention candidates alone, on the `PATH` given. Neither [`ENV`] nor
/// [`RETIRED`] is read.
pub fn for_supervisor(root: &Path, path: Option<&str>) -> Selection {
    conventions(root, &Pin::of(root), path)
}

/// The constructed environment's binding with the supervisor's path in it.
/// Both selection names already in `binding` are removed first, so the
/// supervisor's value is the only one the session can see; with no selected
/// path (no candidate at all) neither name reaches the session, and the hooks
/// keep their absent-binary behavior.
pub fn managed_binding(
    binding: &[(String, String)],
    selected: Option<&Path>,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = binding
        .iter()
        .filter(|(name, _)| name != ENV && name != RETIRED)
        .cloned()
        .collect();
    if let Some(selected) = selected {
        out.push((ENV.to_string(), selected.display().to_string()));
    }
    out
}

fn override_outcome(root: &Path, pin: &Pin, value: &str) -> Result<Selected, Unselected> {
    let remedy = format!(
        "Unset {ENV}, or point it at a binary that satisfies the pin; the pin itself moves only \
         as a D-06 change"
    );
    let path = PathBuf::from(value);
    if !executable(&path) {
        return Err(Unselected::Refused(format!(
            "the override {ENV}={value} names no executable ({}). {remedy}",
            pin.words()
        )));
    }
    let version = version_of(&path);
    let shown = version.as_deref().unwrap_or("no version");
    match pin.admits(&path, root, version.as_deref()) {
        Admits::Yes => Ok(Selected::new(&path, Rule::Override, version)),
        Admits::No => Err(Unselected::Refused(format!(
            "the override {ENV}={value} reports {shown}, which does not satisfy {}. {remedy}",
            pin.words()
        ))),
        Admits::NotPerformed => Err(Unselected::Refused(format!(
            "the override {ENV}={value} (reports {shown}) could not be checked against {}: its \
             configuration probe failed. {remedy}",
            pin.words()
        ))),
    }
}

fn conventions(root: &Path, pin: &Pin, path_var: Option<&str>) -> Selection {
    let mut passed_over = Vec::new();
    let on_path = path_entries(path_var);
    // The first launcher on PATH is asked, and every launcher on PATH is left
    // out of the PATH candidate: a launcher is never itself the judge.
    let mut launcher = None;
    let mut path_engine = None;
    for candidate in on_path {
        match ask_launcher(&candidate, root) {
            Some(answer) => {
                if launcher.is_none() {
                    launcher = Some((candidate, answer));
                }
            }
            None => {
                if path_engine.is_none() {
                    path_engine = Some(candidate);
                }
            }
        }
    }
    let mut candidates = Vec::new();
    if let Some((at, answer)) = launcher {
        match answer {
            LauncherAnswer::Resolved { path, digest } => {
                candidates.push((path.clone(), Rule::Launcher));
                if let Some(said) = digest
                    && executable(&path)
                    && digest_of(&path).as_deref() != Some(said.as_str())
                {
                    // The file changed between the launcher's answer and this
                    // read: neither identity can be recorded as the other.
                    candidates.pop();
                    passed_over.push(format!(
                        "passed over {} (launcher at {}): the launcher reported {said}, and the \
                         file now reads {}",
                        path.display(),
                        at.display(),
                        digest_of(&path).unwrap_or_else(|| "unreadable".to_string())
                    ));
                }
            }
            LauncherAnswer::Unresolved(why) => passed_over.push(format!(
                "passed over the launcher at {}: it resolved no engine ({why})",
                at.display()
            )),
        }
    }
    candidates.push((
        root.join(".tooling/bin").join(PROGRAM),
        Rule::RepositoryLocal,
    ));
    candidates.push((
        root.join("target/release").join(PROGRAM),
        Rule::RepositoryBuild,
    ));
    if let Some(engine) = path_engine {
        candidates.push((engine, Rule::Path));
    }
    let mut found = false;
    let mut considered = Vec::new();
    for (candidate, rule) in candidates {
        if !executable(&candidate) {
            continue;
        }
        found = true;
        let version = version_of(&candidate);
        let shown = version.clone().unwrap_or_else(|| "no version".to_string());
        let what = rule_phrase(rule);
        match pin.admits(&candidate, root, version.as_deref()) {
            Admits::Yes => {
                return Selection {
                    outcome: Ok(Selected::new(&candidate, rule, version)),
                    passed_over,
                    considered,
                    notices: Vec::new(),
                };
            }
            Admits::No => {
                passed_over.push(format!(
                    "passed over {} ({what}, reports {shown}): it does not satisfy {}",
                    candidate.display(),
                    pin.words()
                ));
                considered.push(Selected::new(&candidate, rule, version));
            }
            Admits::NotPerformed => {
                return Selection {
                    outcome: Err(Unselected::Refused(format!(
                        "{} ({what}, reports {shown}) could not be checked against {}: its \
                         configuration probe failed, so no later candidate is tried",
                        candidate.display(),
                        pin.words()
                    ))),
                    passed_over,
                    considered,
                    notices: Vec::new(),
                };
            }
        }
    }
    let outcome = if found || !passed_over.is_empty() {
        Err(Unselected::Refused(format!(
            "no candidate satisfies {}. {PREPARE}",
            pin.words()
        )))
    } else {
        Err(Unselected::Absent)
    };
    Selection {
        outcome,
        passed_over,
        considered,
        notices: Vec::new(),
    }
}

/// The remedy a refusal names (spec 028 section 3.5): resolution never
/// prepares an engine itself.
pub const PREPARE: &str = "Prepare the pinned engine with `make tools` (into .tooling/bin), or \
                           with `spec-spine launcher install` where spec-spine's launcher is \
                           installed; nothing was downloaded";

/// What spec-spine's launcher answered to `launcher resolve --json`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum LauncherAnswer {
    /// The engine it resolved, and the digest it read.
    Resolved {
        path: PathBuf,
        digest: Option<String>,
    },
    /// It is the launcher, and resolved nothing: its summary.
    Unresolved(String),
}

/// Ask `candidate` for the launcher's resolution in `root`. `None` means the
/// candidate is not a launcher: it did not answer a launcher envelope (an
/// engine answers the unknown verb with a usage error). The query writes and
/// downloads nothing (spec-spine spec 188 section 3.7).
fn ask_launcher(candidate: &Path, root: &Path) -> Option<LauncherAnswer> {
    let output = Command::new(candidate)
        .args(["launcher", "resolve", "--json"])
        .current_dir(root)
        .env_remove("SPEC_SPINE_ACQUIRE")
        .env("SPEC_SPINE_FROZEN", "1")
        .output()
        .ok()?;
    read_launcher(&output.stdout)
}

/// Read a launcher envelope. Pure, so the shape is testable without a
/// process.
fn read_launcher(stdout: &[u8]) -> Option<LauncherAnswer> {
    let value: serde_json::Value = serde_json::from_slice(stdout).ok()?;
    // The launcher names itself `spec-spine-launcher` and its verb
    // `launcher.resolve` (spec-spine spec 188, D-12); `spec-spine` is accepted
    // too, since the launcher is installed under that name.
    let tool = value.get("tool")?.as_str()?;
    if !matches!(tool, PROGRAM | LAUNCHER) || !value.get("verb")?.as_str()?.starts_with("launcher")
    {
        return None;
    }
    let text = |v: Option<&serde_json::Value>| v.and_then(|v| v.as_str()).map(str::to_string);
    let report = value.get("report");
    let path = text(report.and_then(|r| r.get("path")));
    match (text(value.get("outcome")).as_deref(), path) {
        (Some("ok"), Some(path)) if Path::new(&path).is_absolute() => {
            Some(LauncherAnswer::Resolved {
                path: PathBuf::from(path),
                digest: text(report.and_then(|r| r.get("digest"))),
            })
        }
        _ => Some(LauncherAnswer::Unresolved(
            text(value.get("summary"))
                .map(|s| s.lines().next().unwrap_or_default().to_string())
                .unwrap_or_else(|| "no summary".to_string()),
        )),
    }
}

fn rule_phrase(rule: Rule) -> &'static str {
    match rule {
        Rule::Launcher => "launcher",
        Rule::RepositoryLocal => "repository-local install",
        Rule::RepositoryBuild => "repository build",
        Rule::Path => "PATH",
        Rule::Supervisor => "supervisor",
        Rule::Override => "override",
    }
}

/// Every executable `spec-spine` on `PATH`, in order, each file once.
fn path_entries(path_var: Option<&str>) -> Vec<PathBuf> {
    let Some(path_var) = path_var else {
        return Vec::new();
    };
    let mut seen = Vec::new();
    for candidate in std::env::split_paths(path_var)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(PROGRAM))
        .filter(|candidate| executable(candidate))
    {
        let key = std::fs::canonicalize(&candidate).unwrap_or_else(|_| candidate.clone());
        if !seen.iter().any(|(k, _): &(PathBuf, PathBuf)| *k == key) {
            seen.push((key, candidate));
        }
    }
    seen.into_iter().map(|(_, c)| c).collect()
}

/// `sha256:<hex>` of a file's bytes.
fn digest_of(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(format!(
        "sha256:{}",
        statecraft_environment::digest::digest_bytes(&bytes)
    ))
}

fn executable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

fn version_of(program: &Path) -> Option<String> {
    let output = Command::new(program).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().next_back())
        .map(str::to_string)
}

#[derive(Debug, PartialEq, Eq)]
enum Admits {
    Yes,
    No,
    NotPerformed,
}

/// The repository's pin: `[meta] required_version` in its `spec-spine.toml`,
/// read as an uncommented line of that table, as the delivered hooks read it.
struct Pin {
    requirement: Option<String>,
    source: PathBuf,
}

impl Pin {
    fn of(root: &Path) -> Self {
        let source = root.join("spec-spine.toml");
        let requirement = std::fs::read_to_string(&source)
            .ok()
            .and_then(|text| required_version(&text));
        Self {
            requirement,
            source,
        }
    }

    fn words(&self) -> String {
        match &self.requirement {
            Some(r) => format!(
                "pin {r} from {} [meta] required_version",
                self.source.display()
            ),
            None => "unpinned".to_string(),
        }
    }

    /// An exact pin is compared with the reported version; any other
    /// requirement is put to the binary itself, which refuses at
    /// configuration load when its version does not satisfy it. Unpinned
    /// admits everything.
    fn admits(&self, program: &Path, root: &Path, version: Option<&str>) -> Admits {
        let Some(requirement) = &self.requirement else {
            return Admits::Yes;
        };
        if let Some(exact) = exact(requirement) {
            return match version {
                None => Admits::NotPerformed,
                Some(v) if v == exact => Admits::Yes,
                Some(_) => Admits::No,
            };
        }
        let Ok(output) = Command::new(program)
            .arg("--repo")
            .arg(root)
            .args(["config", "show"])
            .output()
        else {
            return Admits::NotPerformed;
        };
        match output.status.code() {
            Some(0) => Admits::Yes,
            // A pin not met is 3 below spec-spine 0.26.0 and 2 from it, worded
            // the same under both (spec 002 section 5, 2026-09-25, "both exit
            // tables").
            Some(2 | 3) => {
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                if names_pin_refusal(&text) {
                    Admits::No
                } else {
                    Admits::NotPerformed
                }
            }
            _ => Admits::NotPerformed,
        }
    }
}

/// `=X.Y.Z` with three numeric parts yields `X.Y.Z`.
fn exact(requirement: &str) -> Option<&str> {
    let version = requirement.strip_prefix('=')?.trim();
    let parts: Vec<&str> = version.split('.').collect();
    (parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())))
    .then_some(version)
}

/// The quoted value of an uncommented `required_version` in `[meta]`.
fn required_version(text: &str) -> Option<String> {
    let mut in_meta = false;
    for raw in text.lines() {
        let line = raw.trim_start();
        if line.starts_with('[') {
            let header: String = line
                .split('#')
                .next()
                .unwrap_or("")
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            in_meta = header == "[meta]";
            continue;
        }
        if !in_meta {
            continue;
        }
        let Some(rest) = line.strip_prefix("required_version") else {
            continue;
        };
        let Some(value) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        let value = value.trim_start().strip_prefix('"')?;
        return value.find('"').map(|end| value[..end].to_string());
    }
    None
}

/// The corpus tool for an operation in `root`: the selected binary, or one
/// that reports why none was selected and runs nothing.
pub fn corpus_for(selection: &Selection) -> Box<dyn Corpus> {
    match &selection.outcome {
        Ok(selected) => Box::new(SpecSpineCommand {
            program: selected.program.display().to_string(),
            found_by: Some(selected.rule.word().to_string()),
        }),
        Err(why) => Box::new(NotSelected::from(why, selection)),
    }
}

/// The target probe for an operation in `root`: `git` as before, and the
/// corpus question asked of the selected binary, or answered as unavailable
/// when none was selected.
pub fn probe_for(selection: &Selection) -> SelectedProbe {
    match &selection.outcome {
        Ok(selected) => SelectedProbe {
            inner: CommandProbe::new(selected.program.display().to_string()),
            refused: None,
        },
        // The inner probe is asked only about git: the corpus question is
        // answered from the refusal and never reaches a program.
        Err(why) => SelectedProbe {
            inner: CommandProbe::new(String::new()),
            refused: Some(NotSelected::from(why, selection).answer()),
        },
    }
}

/// A [`TargetProbe`] whose corpus question goes to the selected binary.
#[derive(Debug, Clone)]
pub struct SelectedProbe {
    inner: CommandProbe,
    refused: Option<CheckAnswer>,
}

impl TargetProbe for SelectedProbe {
    fn is_git_work_tree(&self, path: &Path) -> bool {
        self.inner.is_git_work_tree(path)
    }
    fn has_base_revision(&self, path: &Path) -> bool {
        self.inner.has_base_revision(path)
    }
    fn corpus(&self, path: &Path) -> CorpusState {
        match &self.refused {
            None => self.inner.corpus(path),
            Some(answer) => {
                if !path.join("spec-spine.toml").exists() && !path.join("specs").is_dir() {
                    CorpusState::Absent
                } else {
                    CorpusState::CheckUnavailable(answer.describe())
                }
            }
        }
    }
}

/// The corpus tool when nothing was selected: every verb reports why, and
/// nothing is run.
#[derive(Debug, Clone)]
pub struct NotSelected {
    why: Unavailability,
    detail: String,
}

impl NotSelected {
    fn from(why: &Unselected, selection: &Selection) -> Self {
        match why {
            Unselected::Absent => {
                // The notices come too: an operator who set the retired name
                // and has no binary learns which name to set instead.
                let mut detail = format!(
                    "no spec-spine was found through the launcher, in the repository's \
                     .tooling/bin or target/release, or on PATH. {PREPARE}"
                );
                for remark in selection.remarks() {
                    detail.push_str("; ");
                    detail.push_str(&remark);
                }
                Self {
                    why: Unavailability::Absent,
                    detail,
                }
            }
            Unselected::Refused(sentence) => {
                let mut detail = selection.remarks().join("; ");
                if !detail.is_empty() {
                    detail.push_str("; ");
                }
                detail.push_str(sentence);
                Self {
                    why: Unavailability::NotSelected,
                    detail,
                }
            }
        }
    }

    fn answer(&self) -> CheckAnswer {
        CheckAnswer::Unavailable {
            why: self.why,
            detail: self.detail.clone(),
        }
    }
}

impl Corpus for NotSelected {
    fn compile(&self, _root: &Path) -> Result<String, String> {
        Err(self.answer().describe())
    }
    fn index(&self, _root: &Path) -> Result<String, String> {
        Err(self.answer().describe())
    }
    fn check(&self, _root: &Path) -> Result<String, String> {
        Err(self.answer().describe())
    }
    fn version(&self) -> Option<String> {
        None
    }
    fn carries_check(&self, _root: &Path) -> Result<(), CheckAnswer> {
        Err(self.answer())
    }
    fn check_answer(&self, _root: &Path) -> CheckAnswer {
        self.answer()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn stub(at: &Path, version: &str) {
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 'spec-spine {version}'; fi\nexit 0\n"
        );
        install(at, &script);
    }

    // A script written and then exec'd by the same process can race `ETXTBSY`
    // on Linux; the stubs are written to a temporary name and renamed.
    /// A stub written through the adapter's fixture, which copies it into
    /// place in a child process: a file this process wrote and then renamed
    /// can still be open for writing in a sibling test's forked child, and
    /// executing it then fails with `ETXTBSY`, which `admits` reads as a read
    /// not performed.
    fn install(at: &Path, script: &str) {
        statecraft_adapter::fixture::install_script(at, script, 0o755).unwrap();
    }

    fn pinned(root: &Path, pin: Option<&str>) {
        let body = match pin {
            Some(p) => format!("[meta]\nrequired_version = \"{p}\"\n"),
            None => "# [meta]\n# required_version = \"=0.23.0\"\n".to_string(),
        };
        std::fs::write(root.join("spec-spine.toml"), body).unwrap();
    }

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |name: &str| map.get(name).cloned()
    }

    // A range pin is put to the binary. Its refusal is 3 below spec-spine
    // 0.26.0 and 2 from it, worded the same (recorded 2026-09-25).
    #[test]
    fn a_range_pin_refusal_is_read_under_both_exit_tables() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        pinned(&root, Some(">=0.1, <0.2"));
        let pin = Pin::of(&root);
        for (code, words) in [
            (
                3,
                "spec-spine: config error: this repository requires spec-spine >=0.1, <0.2",
            ),
            (
                2,
                "spec-spine: refused: this repository requires spec-spine >=0.1, <0.2",
            ),
        ] {
            let at = dir.path().join(format!("t{code}/spec-spine"));
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            install(
                &at,
                &format!("#!/bin/sh\necho '{words}' >&2\nexit {code}\n"),
            );
            assert_eq!(pin.admits(&at, &root, None), Admits::No, "exit {code}");
        }
        // A 2 that names no pin (0.25.0's stale) establishes nothing.
        let at = dir.path().join("stale/spec-spine");
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        install(&at, "#!/bin/sh\necho 'index is stale' >&2\nexit 2\n");
        assert_eq!(pin.admits(&at, &root, None), Admits::NotPerformed);
    }

    #[test]
    fn the_retired_name_is_reported_and_never_selects() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&root).unwrap();
        pinned(&root, Some("=0.23.0"));
        let named = dir.path().join("named/spec-spine");
        stub(&named, "0.23.0");
        stub(&bin.join(PROGRAM), "0.23.0");
        let named_s = named.display().to_string();
        let bin_s = bin.display().to_string();
        let s = select(&root, &env(&[(RETIRED, &named_s), ("PATH", &bin_s)]));
        let selected = s.outcome.clone().unwrap();
        assert_eq!(selected.rule, Rule::Path);
        assert_eq!(selected.program, bin.join(PROGRAM));
        assert_eq!(s.notices.len(), 1, "{s:?}");
        assert!(s.notices[0].contains(&format!("ignored {RETIRED}={named_s}")));
        assert!(s.notices[0].contains(ENV));
    }

    #[test]
    fn with_no_binary_the_retired_name_notice_still_reaches_the_operator() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        pinned(&root, Some("=0.23.0"));
        let empty = dir.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let empty_s = empty.display().to_string();
        let s = select(
            &root,
            &env(&[(RETIRED, "/nowhere/spec-spine"), ("PATH", &empty_s)]),
        );
        assert_eq!(s.outcome, Err(Unselected::Absent), "{s:?}");
        let detail = NotSelected::from(&Unselected::Absent, &s).detail;
        assert!(detail.starts_with("no spec-spine was found"), "{detail}");
        assert!(detail.contains(RETIRED) && detail.contains(ENV), "{detail}");
    }

    #[test]
    fn outside_a_managed_session_the_variable_is_an_override_put_to_the_pin() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&root).unwrap();
        pinned(&root, Some("=0.23.0"));
        let named = dir.path().join("named/spec-spine");
        stub(&named, "0.24.0");
        stub(&bin.join(PROGRAM), "0.23.0");
        let named_s = named.display().to_string();
        let bin_s = bin.display().to_string();

        // Incompatible: refused, and PATH is never the fallback.
        let s = select(&root, &env(&[(ENV, &named_s), ("PATH", &bin_s)]));
        let Err(Unselected::Refused(why)) = &s.outcome else {
            panic!("{s:?}");
        };
        assert!(why.contains(&format!("the override {ENV}={named_s} reports 0.24.0")));
        assert!(why.contains("=0.23.0") && why.contains(&format!("Unset {ENV}")));
        assert!(s.notices.is_empty(), "both set: the old name is silent");

        // Naming no executable: refused, never a silent skip.
        let gone = dir.path().join("gone").display().to_string();
        let s = select(&root, &env(&[(ENV, &gone), ("PATH", &bin_s)]));
        assert!(
            matches!(&s.outcome, Err(Unselected::Refused(w)) if w.contains("names no executable")),
            "{s:?}"
        );

        // Compatible: selected as the override.
        stub(&named, "0.23.0");
        let s = select(&root, &env(&[(ENV, &named_s), ("PATH", &bin_s)]));
        assert_eq!(s.outcome.unwrap().rule, Rule::Override);
    }

    #[test]
    fn in_a_managed_session_the_variable_is_the_supervisors_and_not_put_to_the_pin() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        pinned(&root, Some("=0.23.0"));
        let supervised = dir.path().join("sup/spec-spine");
        stub(&supervised, "0.24.0");
        let sup = supervised.display().to_string();
        let s = select(&root, &env(&[(ENV, &sup), (MANAGED, "003-x")]));
        let selected = s.outcome.unwrap();
        assert_eq!(selected.rule, Rule::Supervisor);
        assert_eq!(selected.program, supervised);
    }

    #[test]
    fn the_conventions_prefer_a_compatible_repository_build_and_name_what_they_pass_over() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&root).unwrap();
        pinned(&root, Some("=0.23.0"));
        stub(&root.join("target/release").join(PROGRAM), "0.24.0");
        stub(&bin.join(PROGRAM), "0.23.0");
        let bin_s = bin.display().to_string();
        let s = select(&root, &env(&[("PATH", &bin_s)]));
        assert_eq!(s.outcome.clone().unwrap().rule, Rule::Path);
        assert_eq!(s.passed_over.len(), 1);
        assert!(s.passed_over[0].contains("repository build, reports 0.24.0"));

        // Unpinned: the first candidate, the repository build.
        pinned(&root, None);
        let s = select(&root, &env(&[("PATH", &bin_s)]));
        assert_eq!(s.outcome.unwrap().rule, Rule::RepositoryBuild);

        // Nothing anywhere: absent.
        let empty = dir.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let s = select(&empty, &env(&[("PATH", &empty.display().to_string())]));
        assert_eq!(s.outcome, Err(Unselected::Absent));
    }

    // Spec 028 section 3.2: the repository-local install answers before the
    // repository build and PATH, and a selection carries its digest.
    #[test]
    fn the_repository_local_install_comes_before_the_build_and_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&root).unwrap();
        pinned(&root, Some("=0.23.0"));
        stub(&root.join(".tooling/bin").join(PROGRAM), "0.23.0");
        stub(&root.join("target/release").join(PROGRAM), "0.23.0");
        stub(&bin.join(PROGRAM), "0.23.0");
        let bin_s = bin.display().to_string();
        let s = select(&root, &env(&[("PATH", &bin_s)]));
        let selected = s.outcome.unwrap();
        assert_eq!(selected.rule, Rule::RepositoryLocal);
        let digest = selected.digest.clone().unwrap();
        assert!(
            digest.starts_with("sha256:") && digest.len() == 71,
            "{digest}"
        );
        assert!(selected.describe().contains("rule repository-local"));

        // Incompatible: passed over, named, and recorded as considered.
        stub(&root.join(".tooling/bin").join(PROGRAM), "0.22.0");
        let s = select(&root, &env(&[("PATH", &bin_s)]));
        assert_eq!(s.outcome.clone().unwrap().rule, Rule::RepositoryBuild);
        assert!(s.passed_over[0].contains("repository-local install, reports 0.22.0"));
        assert_eq!(s.considered[0].version.as_deref(), Some("0.22.0"));
    }

    #[test]
    fn a_launcher_answer_is_read_from_its_envelope_and_anything_else_is_not_a_launcher() {
        let ok = br#"{"exitCode":0,"outcome":"ok","report":{"digest":"sha256:ab","path":"/store/spec-spine","rule":"store"},"summary":"launcher resolve: ok","tool":"spec-spine-launcher","verb":"launcher.resolve"}"#;
        assert_eq!(
            read_launcher(ok),
            Some(LauncherAnswer::Resolved {
                path: PathBuf::from("/store/spec-spine"),
                digest: Some("sha256:ab".into()),
            })
        );
        let missing = br#"{"exitCode":1,"outcome":"finding","summary":"release 0.23.0 is not installed\nmore","tool":"spec-spine-launcher","verb":"launcher.resolve"}"#;
        assert_eq!(
            read_launcher(missing),
            Some(LauncherAnswer::Unresolved(
                "release 0.23.0 is not installed".into()
            ))
        );
        // A relative path is never executed as an answer.
        let relative = br#"{"outcome":"ok","report":{"path":"spec-spine"},"tool":"spec-spine-launcher","verb":"launcher.resolve"}"#;
        assert!(matches!(
            read_launcher(relative),
            Some(LauncherAnswer::Unresolved(_))
        ));
        // An engine's usage error, another tool, or another verb: not a
        // launcher.
        assert_eq!(read_launcher(b"error: unrecognized subcommand"), None);
        assert_eq!(
            read_launcher(br#"{"outcome":"usage","tool":"spec-spine","verb":"check"}"#),
            None
        );
        assert_eq!(
            read_launcher(br#"{"outcome":"ok","tool":"other","verb":"launcher resolve"}"#),
            None
        );
    }

    #[test]
    fn a_launcher_on_path_resolves_and_is_never_the_path_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&root).unwrap();
        pinned(&root, Some("=0.23.0"));
        let engine = dir.path().join("store/spec-spine");
        stub(&engine, "0.23.0");
        let envelope = format!(
            r#"{{"exitCode":0,"outcome":"ok","report":{{"path":"{}"}},"summary":"ok","tool":"spec-spine-launcher","verb":"launcher.resolve"}}"#,
            engine.display()
        );
        std::fs::create_dir_all(&bin).unwrap();
        install(
            &bin.join(PROGRAM),
            &format!(
                "#!/bin/sh\nif [ \"$1\" = launcher ]; then echo '{envelope}'; exit 0; fi\nexit 4\n"
            ),
        );
        let bin_s = bin.display().to_string();
        let s = select(&root, &env(&[("PATH", &bin_s)]));
        let selected = s.outcome.clone().unwrap();
        assert_eq!(selected.rule, Rule::Launcher);
        assert_eq!(selected.program, engine);

        // The launcher resolves an engine the pin does not admit: passed over,
        // and the launcher itself is never tried as PATH.
        stub(&engine, "0.24.0");
        let s = select(&root, &env(&[("PATH", &bin_s)]));
        assert!(matches!(s.outcome, Err(Unselected::Refused(_))), "{s:?}");
        assert!(
            s.passed_over[0].contains("(launcher, reports 0.24.0)"),
            "{s:?}"
        );
        assert_eq!(s.passed_over.len(), 1, "{s:?}");
    }

    #[test]
    fn the_supervisor_reads_neither_variable_and_its_value_replaces_an_inherited_one() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&root).unwrap();
        stub(&bin.join(PROGRAM), "0.23.0");
        let s = for_supervisor(&root, Some(&bin.display().to_string()));
        let selected = s.outcome.unwrap();
        assert_eq!(selected.program, bin.join(PROGRAM));

        let binding = vec![
            (MANAGED.to_string(), "003-x".to_string()),
            (ENV.to_string(), "/inherited/spec-spine".to_string()),
            (RETIRED.to_string(), "/inherited/spec-spine".to_string()),
        ];
        let out = managed_binding(&binding, Some(&selected.program));
        let values: Vec<&str> = out
            .iter()
            .filter(|(n, _)| n == ENV)
            .map(|(_, v)| v.as_str())
            .collect();
        assert_eq!(values, [bin.join(PROGRAM).display().to_string()]);
        assert!(out.contains(&(MANAGED.to_string(), "003-x".to_string())));
        assert!(!out.iter().any(|(n, _)| n == RETIRED), "{out:?}");

        // No candidate at all: neither name reaches the session.
        let out = managed_binding(&binding, None);
        assert!(
            !out.iter().any(|(n, _)| n == ENV || n == RETIRED),
            "{out:?}"
        );
        assert_eq!(out, [(MANAGED.to_string(), "003-x".to_string())]);
    }

    #[test]
    fn a_refusal_is_a_corpus_that_runs_nothing_and_says_why() {
        let selection = Selection {
            outcome: Err(Unselected::Refused(
                "no candidate satisfies pin =0.23.0".into(),
            )),
            passed_over: vec!["passed over /x (PATH, reports 0.22.0)".into()],
            considered: vec![],
            notices: vec![],
        };
        let corpus = corpus_for(&selection);
        let Err(answer) = corpus.carries_check(Path::new("/")) else {
            panic!("a refused selection carried check");
        };
        assert_eq!(answer.exit_code(), 2);
        let text = answer.describe();
        assert!(
            text.contains("passed over /x") && text.contains("=0.23.0"),
            "{text}"
        );
    }

    #[test]
    fn a_version_from_a_failed_version_call_is_no_version() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join(PROGRAM);
        install(&bin, "#!/bin/sh\necho 'spec-spine 0.23.0'\nexit 1\n");
        assert_eq!(version_of(&bin), None);
        stub(&bin, "0.23.0");
        assert_eq!(version_of(&bin).as_deref(), Some("0.23.0"));
    }

    #[test]
    fn the_pin_is_read_as_the_hooks_read_it() {
        assert_eq!(
            required_version("[meta]\nrequired_version = \"=0.23.0\"\n").as_deref(),
            Some("=0.23.0")
        );
        assert_eq!(
            required_version("# [meta]\n# required_version = \"=0.23.0\"\n"),
            None
        );
        assert_eq!(
            required_version("[index]\nrequired_version = \"=1.0.0\"\n[meta]\n"),
            None
        );
        // An inline comment after the value, and one after the table header,
        // as the hooks' awk reads them.
        assert_eq!(
            required_version("[meta] # the pin\nrequired_version = \"=0.23.0\" # note\n")
                .as_deref(),
            Some("=0.23.0")
        );
        assert_eq!(
            required_version("[meta]\n  required_version=\">=0.23, <0.24\"\n").as_deref(),
            Some(">=0.23, <0.24")
        );
        assert_eq!(required_version("[meta]\nrequired_version = 0.23\n"), None);
        assert_eq!(exact("=0.23.0"), Some("0.23.0"));
        assert_eq!(exact("=0.23"), None);
        assert_eq!(exact(">=0.23, <0.24"), None);
    }
}
