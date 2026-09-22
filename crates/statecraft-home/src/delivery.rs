//! Delivery: what actually reaches a session, evaluated rather than assumed.
//!
//! Spec 002 section 3.14. Two different deliveries live here, and conflating
//! them is the mistake this module exists to prevent:
//!
//! - **Native delivery** puts small links into an agent's own home so its
//!   discovery finds the one canonical harness. It copies nothing into a
//!   repository, it never rewrites a user's settings file, and it happens only
//!   under an explicit operator action.
//! - **Instruction delivery** asks whether a session rooted at a project would
//!   actually resolve `.statecraft/AGENTS.md`. A file existing is not delivery,
//!   and neither is a digest match. Each harness has a documented load rule,
//!   and this module evaluates that rule against the actual tree.
//!
//! The import line in a root `AGENTS.md` is a project convention. Claude Code
//! documents expansion of an `@path` import; Codex's documentation does not
//! establish an equivalent, so its rule is not evaluable here and the verdict
//! is `unverified`. Reporting `unverified` is the honest answer; reporting
//! `delivered` because a file is on disk is not.

use crate::harness::HarnessFile;
use crate::project::INSTRUCTIONS;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The environment variable that relocates every native agent home.
///
/// Set to a sandbox by a test, so that checking global integration never
/// requires writing into the operator's real home. Absent, the parent is the
/// operator's home, which is where these directories actually are.
pub const NATIVE_ROOT_ENV: &str = "STATECRAFT_NATIVE_ROOT";

/// The parent directory native agent homes live under.
pub fn native_parent() -> PathBuf {
    if let Ok(explicit) = std::env::var(NATIVE_ROOT_ENV) {
        if !explicit.is_empty() {
            return PathBuf::from(explicit);
        }
    }
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
}

/// A harness's own configuration home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeHome {
    /// The harness this belongs to.
    pub harness: String,
    /// Its root directory.
    pub root: PathBuf,
}

/// The native homes this product knows how to deliver into, under a parent.
///
/// The parent is a parameter rather than a read of the environment, so an
/// operation is explicit about where it would write and a test never has to
/// mutate a process-wide variable to stay out of the operator's real home.
pub fn native_homes_under(parent: &Path) -> Vec<NativeHome> {
    vec![NativeHome {
        harness: "claude-code".to_string(),
        root: parent.join(".claude"),
    }]
}

/// The native homes under the resolved parent.
pub fn native_homes() -> Vec<NativeHome> {
    native_homes_under(&native_parent())
}

/// A file this product will never touch in a native home, whatever happens.
///
/// Section 3.4: an adapter never repoints an agent home, never moves an
/// authentication store, and never changes personal permissions. Stated as
/// data so a test can hold it, rather than as a habit.
pub const NEVER_TOUCHED: [&str; 6] = [
    "settings.json",
    "settings.local.json",
    ".credentials.json",
    "credentials.json",
    "config.json",
    "auth.json",
];

/// What native delivery would do with one path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum NativeAction {
    /// A link from the native home to the canonical source.
    Link {
        /// Where the link goes, absolute.
        at: String,
        /// What it points at, absolute.
        to: String,
        /// True when a link with the same target is already there.
        already: bool,
    },
    /// Nothing is done, and this is why.
    Skipped {
        /// The path not touched.
        at: String,
        /// Why.
        reason: String,
    },
}

impl NativeAction {
    /// Whether performing this changes anything.
    pub fn changes_anything(&self) -> bool {
        matches!(self, NativeAction::Link { already: false, .. })
    }

    /// A one-line rendering.
    pub fn describe(&self) -> String {
        match self {
            NativeAction::Link { at, to, already } => {
                let verb = if *already { "present" } else { "link" };
                format!("{verb} {at} -> {to}")
            }
            NativeAction::Skipped { at, reason } => format!("skip {at}: {reason}"),
        }
    }
}

/// What native delivery would do in one agent home.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativePlan {
    /// The harness.
    pub harness: String,
    /// Its home.
    pub home: String,
    /// One action per delivered name.
    pub actions: Vec<NativeAction>,
}

impl NativePlan {
    /// True when performing this changes nothing.
    pub fn unchanged(&self) -> bool {
        !self.actions.iter().any(NativeAction::changes_anything)
    }
}

/// Which harness directories a delivered kind belongs in.
///
/// Only the two a harness actually discovers from its own home. Hooks are
/// deliberately absent: wiring one would mean rewriting the operator's own
/// settings file, which section 3.4 refuses, so the hook ships in the harness
/// and is installed by the operator if they want it.
fn native_destination(rel_path: &str) -> Option<String> {
    let (kind, rest) = rel_path.split_once('/')?;
    match kind {
        "skills" => {
            // A skill is a directory; the link is to its directory, not to the
            // file inside it.
            let name = rest.split('/').next()?;
            Some(format!("skills/{name}"))
        }
        "agents" => Some(format!("agents/{rest}")),
        _ => None,
    }
}

/// Compute native delivery for one home. Reads; writes nothing.
pub fn native_plan(home: &NativeHome, revision_root: &Path, files: &[HarnessFile]) -> NativePlan {
    let mut destinations: Vec<(String, PathBuf)> = Vec::new();
    for file in files {
        let Some(destination) = native_destination(&file.rel_path) else {
            continue;
        };
        if destinations.iter().any(|(d, _)| *d == destination) {
            continue;
        }
        // The source is the destination's own subtree inside the revision: a
        // skill links to its directory, an agent to its file.
        let source_rel = match destination.split_once('/') {
            Some(("skills", name)) => format!("skills/{name}"),
            _ => file.rel_path.clone(),
        };
        destinations.push((
            destination,
            statecraft_environment::claimant::resolve(revision_root, &source_rel),
        ));
    }
    destinations.sort_by(|a, b| a.0.cmp(&b.0));

    let actions = destinations
        .into_iter()
        .map(|(destination, source)| {
            let at = statecraft_environment::claimant::resolve(&home.root, &destination);
            let at_display = at.display().to_string();
            if NEVER_TOUCHED
                .iter()
                .any(|n| destination.ends_with(n) || at.ends_with(n))
            {
                return NativeAction::Skipped {
                    at: at_display,
                    reason: "this product never writes a harness's own configuration".to_string(),
                };
            }
            match std::fs::symlink_metadata(&at) {
                Err(_) => NativeAction::Link {
                    at: at_display,
                    to: source.display().to_string(),
                    already: false,
                },
                Ok(meta) if meta.file_type().is_symlink() => {
                    let current = std::fs::read_link(&at).unwrap_or_default();
                    NativeAction::Link {
                        at: at_display,
                        to: source.display().to_string(),
                        already: current == source,
                    }
                }
                Ok(_) => NativeAction::Skipped {
                    at: at_display,
                    reason: "a real file or directory is already there, and it is not this \
                             product's to replace"
                        .to_string(),
                },
            }
        })
        .collect();

    NativePlan {
        harness: home.harness.clone(),
        home: home.root.display().to_string(),
        actions,
    }
}

/// Perform a native plan. The one operation that writes outside the product
/// home, and it happens only under `home apply`.
pub fn native_apply(plan: &NativePlan) -> std::io::Result<Vec<String>> {
    let mut performed = Vec::new();
    for action in &plan.actions {
        let NativeAction::Link { at, to, already } = action else {
            continue;
        };
        if *already {
            continue;
        }
        let at = PathBuf::from(at);
        if let Some(parent) = at.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if std::fs::symlink_metadata(&at).is_ok() {
            std::fs::remove_file(&at)?;
        }
        symlink(Path::new(to), &at)?;
        performed.push(at.display().to_string());
    }
    Ok(performed)
}

#[cfg(unix)]
fn symlink(source: &Path, at: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source, at)
}

#[cfg(windows)]
fn symlink(source: &Path, at: &Path) -> std::io::Result<()> {
    if source.is_dir() {
        std::os::windows::fs::symlink_dir(source, at)
    } else {
        std::os::windows::fs::symlink_file(source, at)
    }
}

/// A recorded delivery, kept in the product home.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    /// The harness.
    pub harness: String,
    /// The harness's home.
    pub home: String,
    /// The harness revision delivered.
    pub revision: String,
    /// Each link, as `at -> to`.
    pub links: Vec<String>,
    /// When, RFC 3339 UTC.
    pub recorded_at: String,
}

/// A harness's documented rule for finding its instructions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadRule {
    /// The harness.
    pub harness: String,
    /// The files it reads first, in order.
    pub entries: Vec<String>,
    /// Whether it expands an `@path` import from an entry file.
    ///
    /// `None` means **not established**, which is not the same as `Some(false)`.
    /// A rule this product cannot evaluate produces `unverified`.
    pub expands_imports: Option<bool>,
    /// What the verdict cites.
    pub basis: String,
}

/// The rules this product knows.
pub fn load_rules() -> Vec<LoadRule> {
    vec![
        LoadRule {
            harness: "claude-code".to_string(),
            entries: vec!["CLAUDE.md".to_string(), "AGENTS.md".to_string()],
            expands_imports: Some(true),
            basis: "Claude Code documents expansion of an `@path` import from its project \
                    instruction file"
                .to_string(),
        },
        LoadRule {
            harness: "codex-cli".to_string(),
            entries: vec!["AGENTS.md".to_string()],
            expands_imports: None,
            basis: "Codex's documentation does not establish native expansion of an `@path` \
                    import, so the chain cannot be evaluated from the file tree"
                .to_string(),
        },
    ]
}

/// Whether a session would actually resolve the managed instructions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "verdict")]
pub enum Delivery {
    /// The documented rule arrives at the managed file. The chain is named.
    Reached {
        /// The files traversed, entry first, managed file last.
        via: Vec<String>,
    },
    /// The rule was evaluated and does not arrive there.
    NotReached {
        /// Why.
        reason: String,
    },
    /// The harness has no documented rule this product can evaluate.
    Unverified {
        /// Why, citing what is and is not established.
        reason: String,
    },
}

impl Delivery {
    /// A one-word rendering.
    pub fn word(&self) -> &'static str {
        match self {
            Delivery::Reached { .. } => "reached",
            Delivery::NotReached { .. } => "not-reached",
            Delivery::Unverified { .. } => "unverified",
        }
    }

    /// True only for an evaluated rule that arrives.
    pub fn reached(&self) -> bool {
        matches!(self, Delivery::Reached { .. })
    }

    /// A one-line rendering for a report.
    pub fn describe(&self) -> String {
        match self {
            Delivery::Reached { via } => format!("reached via {}", via.join(" -> ")),
            Delivery::NotReached { reason } => format!("not-reached: {reason}"),
            Delivery::Unverified { reason } => format!("unverified: {reason}"),
        }
    }
}

/// How deep an import chain is followed before this product stops.
const MAX_IMPORT_DEPTH: usize = 8;

/// Evaluate one harness's load rule against a project tree.
pub fn evaluate(root: &Path, rule: &LoadRule) -> Delivery {
    let Some(expands) = rule.expands_imports else {
        return Delivery::Unverified {
            reason: rule.basis.clone(),
        };
    };

    let present: Vec<&String> = rule
        .entries
        .iter()
        .filter(|e| statecraft_environment::claimant::resolve(root, e).is_file())
        .collect();
    if present.is_empty() {
        return Delivery::NotReached {
            reason: format!(
                "none of this harness's entry files exists: {}",
                rule.entries.join(", ")
            ),
        };
    }

    for entry in &present {
        if let Some(chain) = follow(root, entry, expands, 0, &mut Vec::new()) {
            return Delivery::Reached { via: chain };
        }
    }
    Delivery::NotReached {
        reason: format!(
            "following the documented rule from {} does not arrive at {INSTRUCTIONS}",
            present
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Follow imports from one file, returning the chain that reaches the managed
/// instructions.
fn follow(
    root: &Path,
    rel: &str,
    expands: bool,
    depth: usize,
    seen: &mut Vec<String>,
) -> Option<Vec<String>> {
    if depth > MAX_IMPORT_DEPTH || seen.iter().any(|s| s == rel) {
        return None;
    }
    seen.push(rel.to_string());
    if rel == INSTRUCTIONS {
        return Some(vec![rel.to_string()]);
    }
    let text =
        std::fs::read_to_string(statecraft_environment::claimant::resolve(root, rel)).ok()?;
    if !expands {
        return None;
    }
    for imported in imports_of(&text) {
        if !statecraft_environment::claimant::resolve(root, &imported).is_file() {
            continue;
        }
        if let Some(mut chain) = follow(root, &imported, expands, depth + 1, seen) {
            let mut out = vec![rel.to_string()];
            out.append(&mut chain);
            return Some(out);
        }
    }
    None
}

/// Every `@path` import a file declares.
///
/// A whole line whose first character is `@`. Deliberately strict: an `@` in
/// prose is not an import, and treating one as an import is how a report
/// claims a chain that does not exist.
pub fn imports_of(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter_map(|l| l.strip_prefix('@'))
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty() && !p.contains(char::is_whitespace))
        .collect()
}

/// A pointer file an adapter may place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pointer {
    /// Repository-relative path.
    pub path: String,
    /// The bytes.
    pub contents: String,
}

/// The pointer an adapter would place to make its rule reach the managed file.
///
/// `None` when the rule already reaches it (nothing is injected: a second copy
/// of an import native loading already performs is a duplicate), when the rule
/// cannot be evaluated (nothing is claimed and nothing is written on the
/// strength of a file existing), or when every entry path already holds a file
/// (spec 002 section 3.8: one pointer, and only where no file exists).
pub fn pointer_for(root: &Path, rule: &LoadRule, verdict: &Delivery) -> Option<Pointer> {
    if !matches!(verdict, Delivery::NotReached { .. }) {
        return None;
    }
    let entry = rule
        .entries
        .iter()
        .find(|e| !statecraft_environment::claimant::resolve(root, e).exists())?;
    Some(Pointer {
        path: entry.clone(),
        contents: format!("{}\n", crate::bridge::IMPORT_LINE),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, contents) in files {
            let at = statecraft_environment::claimant::resolve(dir.path(), path);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, contents).unwrap();
        }
        dir
    }

    fn claude() -> LoadRule {
        load_rules()
            .into_iter()
            .find(|r| r.harness == "claude-code")
            .unwrap()
    }

    fn codex() -> LoadRule {
        load_rules()
            .into_iter()
            .find(|r| r.harness == "codex-cli")
            .unwrap()
    }

    #[test]
    fn an_evaluable_rule_that_arrives_reports_the_chain() {
        let dir = project(&[
            ("CLAUDE.md", "@AGENTS.md\n"),
            ("AGENTS.md", "@.statecraft/AGENTS.md\n\n# mine\n"),
            (INSTRUCTIONS, "managed\n"),
        ]);
        match evaluate(dir.path(), &claude()) {
            Delivery::Reached { via } => {
                assert_eq!(via, ["CLAUDE.md", "AGENTS.md", INSTRUCTIONS]);
            }
            other => panic!("expected reached, got {other:?}"),
        }
    }

    #[test]
    fn an_evaluable_rule_that_does_not_arrive_says_so_rather_than_guessing() {
        let dir = project(&[("CLAUDE.md", "# nothing imported\n"), (INSTRUCTIONS, "m\n")]);
        let v = evaluate(dir.path(), &claude());
        assert_eq!(v.word(), "not-reached");
        assert!(!v.reached());
    }

    #[test]
    fn a_harness_with_no_established_rule_is_unverified_and_never_delivered() {
        let dir = project(&[
            ("AGENTS.md", "@.statecraft/AGENTS.md\n"),
            (INSTRUCTIONS, "m\n"),
        ]);
        // The file is there and the import is first, and that is still not
        // evidence: the verdict is unverified, not reached.
        let v = evaluate(dir.path(), &codex());
        assert_eq!(v.word(), "unverified");
        assert!(!v.reached());
        assert!(v.describe().contains("does not establish"));
    }

    #[test]
    fn nothing_is_injected_where_the_rule_already_reaches() {
        let dir = project(&[
            ("CLAUDE.md", "@.statecraft/AGENTS.md\n"),
            (INSTRUCTIONS, "m\n"),
        ]);
        let v = evaluate(dir.path(), &claude());
        assert!(v.reached());
        assert!(pointer_for(dir.path(), &claude(), &v).is_none());
    }

    #[test]
    fn a_pointer_is_offered_only_where_no_file_exists_at_that_path() {
        let dir = project(&[("AGENTS.md", "# mine, no import\n"), (INSTRUCTIONS, "m\n")]);
        let v = evaluate(dir.path(), &claude());
        assert_eq!(v.word(), "not-reached");
        let pointer = pointer_for(dir.path(), &claude(), &v).expect("CLAUDE.md is free");
        assert_eq!(pointer.path, "CLAUDE.md");
        assert_eq!(pointer.contents, "@.statecraft/AGENTS.md\n");

        // With every entry occupied, no pointer is offered and no file is
        // appended to.
        let occupied = project(&[
            ("CLAUDE.md", "# theirs\n"),
            ("AGENTS.md", "# theirs\n"),
            (INSTRUCTIONS, "m\n"),
        ]);
        let v = evaluate(occupied.path(), &claude());
        assert!(pointer_for(occupied.path(), &claude(), &v).is_none());
    }

    #[test]
    fn an_unverified_rule_never_gets_a_pointer_either() {
        let dir = project(&[(INSTRUCTIONS, "m\n")]);
        let v = evaluate(dir.path(), &codex());
        assert!(pointer_for(dir.path(), &codex(), &v).is_none());
    }

    #[test]
    fn an_import_cycle_terminates() {
        let dir = project(&[("CLAUDE.md", "@AGENTS.md\n"), ("AGENTS.md", "@CLAUDE.md\n")]);
        assert_eq!(evaluate(dir.path(), &claude()).word(), "not-reached");
    }

    #[test]
    fn an_at_sign_in_prose_is_not_an_import() {
        assert!(imports_of("write to bart@statecraft.ing for details\n").is_empty());
        assert!(imports_of("@ \n").is_empty());
        assert_eq!(imports_of("@a/b.md\n"), ["a/b.md"]);
    }

    #[test]
    fn native_delivery_links_and_never_names_a_settings_or_credential_file() {
        let sandbox = tempfile::tempdir().unwrap();
        let home = native_homes_under(sandbox.path())
            .into_iter()
            .next()
            .expect("one native home");
        let revision = sandbox.path().join("harness/h-000000000000");
        let plan = native_plan(&home, &revision, &crate::harness::shipped());
        assert!(!plan.actions.is_empty());
        for action in &plan.actions {
            let rendered = action.describe();
            for forbidden in NEVER_TOUCHED {
                assert!(
                    !rendered.contains(forbidden),
                    "the plan names {forbidden}: {rendered}"
                );
            }
        }
        // Hooks are not delivered: wiring one means rewriting a settings file.
        assert!(!plan.actions.iter().any(|a| a.describe().contains("hooks")));
    }

    #[test]
    fn native_delivery_is_idempotent_and_leaves_a_real_file_alone() {
        let sandbox = tempfile::tempdir().unwrap();
        let home = NativeHome {
            harness: "claude-code".into(),
            root: sandbox.path().join(".claude"),
        };
        let revision = sandbox.path().join("harness/h-000000000000");
        std::fs::create_dir_all(revision.join("skills/statecraft-project")).unwrap();
        std::fs::create_dir_all(revision.join("agents")).unwrap();

        let files = crate::harness::shipped();
        let first = native_plan(&home, &revision, &files);
        assert!(!first.unchanged());
        native_apply(&first).unwrap();
        let second = native_plan(&home, &revision, &files);
        assert!(second.unchanged(), "{:?}", second.actions);

        // A real file where a link would go is the operator's, and stays.
        let occupied = sandbox.path().join(".claude/agents");
        std::fs::create_dir_all(&occupied).unwrap();
        let theirs = occupied.join("statecraft-acceptance-reader.md");
        std::fs::remove_file(&theirs).ok();
        std::fs::write(&theirs, b"mine\n").unwrap();
        let third = native_plan(&home, &revision, &files);
        assert!(
            third
                .actions
                .iter()
                .any(|a| matches!(a, NativeAction::Skipped { .. })),
            "{:?}",
            third.actions
        );
        assert_eq!(std::fs::read(&theirs).unwrap(), b"mine\n");
    }
}
