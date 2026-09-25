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
//! 3. **Otherwise** the convention candidates, the repository's own
//!    `target/release/spec-spine` and then `PATH`; the first the pin admits is
//!    selected, and each one passed over is named. An unpinned repository
//!    takes the first candidate.
//! 4. **The retired name, [`RETIRED`], is reported and never read**: when it is
//!    set and [`ENV`] is not, a notice says it was ignored and names the new
//!    one.
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
    /// The repository's own `target/release/spec-spine`.
    RepositoryBuild,
    /// The first on `PATH`.
    Path,
}

impl Rule {
    /// The word a report carries.
    pub fn word(self) -> &'static str {
        match self {
            Rule::Supervisor => "supervisor",
            Rule::Override => "override",
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
                Ok(Selected {
                    version: version_of(&path),
                    program: absolute(&path),
                    rule: Rule::Supervisor,
                })
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
        Admits::Yes => Ok(Selected {
            program: absolute(&path),
            rule: Rule::Override,
            version,
        }),
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
    let candidates = [
        (
            Some(root.join("target/release").join(PROGRAM)),
            Rule::RepositoryBuild,
        ),
        (on_path(path_var), Rule::Path),
    ];
    let mut found = false;
    for (candidate, rule) in candidates {
        let Some(candidate) = candidate.filter(|c| executable(c)) else {
            continue;
        };
        found = true;
        let version = version_of(&candidate);
        let shown = version.clone().unwrap_or_else(|| "no version".to_string());
        let what = rule_phrase(rule);
        match pin.admits(&candidate, root, version.as_deref()) {
            Admits::Yes => {
                return Selection {
                    outcome: Ok(Selected {
                        program: absolute(&candidate),
                        rule,
                        version,
                    }),
                    passed_over,
                    notices: Vec::new(),
                };
            }
            Admits::No => passed_over.push(format!(
                "passed over {} ({what}, reports {shown}): it does not satisfy {}",
                candidate.display(),
                pin.words()
            )),
            Admits::NotPerformed => {
                return Selection {
                    outcome: Err(Unselected::Refused(format!(
                        "{} ({what}, reports {shown}) could not be checked against {}: its \
                         configuration probe failed, so no later candidate is tried",
                        candidate.display(),
                        pin.words()
                    ))),
                    passed_over,
                    notices: Vec::new(),
                };
            }
        }
    }
    let outcome = if found {
        Err(Unselected::Refused(format!(
            "no candidate satisfies {}",
            pin.words()
        )))
    } else {
        Err(Unselected::Absent)
    };
    Selection {
        outcome,
        passed_over,
        notices: Vec::new(),
    }
}

fn rule_phrase(rule: Rule) -> &'static str {
    match rule {
        Rule::RepositoryBuild => "repository build",
        Rule::Path => "PATH",
        Rule::Supervisor => "supervisor",
        Rule::Override => "override",
    }
}

fn on_path(path_var: Option<&str>) -> Option<PathBuf> {
    std::env::split_paths(path_var?)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(PROGRAM))
        .find(|candidate| executable(candidate))
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
            inner: CommandProbe {
                spec_spine: selected.program.display().to_string(),
                ..CommandProbe::default()
            },
            refused: None,
        },
        Err(why) => SelectedProbe {
            inner: CommandProbe::default(),
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
                let mut detail =
                    "no spec-spine was found in the repository's target/release or on PATH"
                        .to_string();
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
    fn install(at: &Path, script: &str) {
        use std::os::unix::fs::PermissionsExt;
        let staged = at.with_extension("staged");
        std::fs::write(&staged, script).unwrap();
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::rename(&staged, at).unwrap();
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
