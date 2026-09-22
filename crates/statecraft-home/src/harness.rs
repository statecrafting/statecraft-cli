//! The one canonical harness source, maintained globally and copied into no
//! repository.
//!
//! Spec 002 section 3.14. Skills, agent definitions, rules, hooks and adapter
//! templates live once, under `harness/<revision>/` in the product home. A
//! revision is **content addressed**: its identity is a digest over its own
//! files, so two homes holding the same bytes hold the same revision, a changed
//! byte is a different revision, and "which harness did this run use" has an
//! answer that does not depend on a version somebody remembered to bump.
//!
//! Two properties of the shipped content are asserted rather than intended:
//! every delivered name is Statecraft-namespaced, and every delivered behavior
//! states the gate that makes it inert outside a Statecraft project.

use crate::home::Layout;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The namespace every harness name carries.
pub const NAMESPACE: &str = "statecraft";

/// The sentence every delivered behavior carries, and the gate it states.
///
/// A behavior that applies everywhere is a behavior that collides with somebody
/// else's repository. The gate is a file test an agent can perform itself, not
/// a promise this product makes on its behalf.
pub const GATE: &str = "Applies only inside a Statecraft project: a repository holding `.statecraft/environment.json`. \
     Outside one, ignore this file entirely.";

/// One file of the canonical harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessFile {
    /// Path relative to the revision directory, forward slashes.
    pub rel_path: String,
    /// The bytes.
    pub contents: String,
    /// True when the file must arrive executable.
    pub executable: bool,
}

/// One file's identity inside a revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileIdentity {
    /// Path relative to the revision directory.
    pub rel_path: String,
    /// SHA-256 of the contents.
    pub digest: String,
    /// Length in bytes.
    pub bytes: u64,
}

/// A content-addressed harness revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Revision {
    /// The revision identity, `h-` followed by twelve hex characters.
    pub id: String,
    /// Every file, ordered by path.
    pub files: Vec<FileIdentity>,
}

/// The identity of a set of harness files.
///
/// Over path and content only, in path order. Not over the mode, not over a
/// timestamp: a revision is what the files say, and a re-installed identical
/// tree is the same revision.
pub fn revision_of(files: &[HarnessFile]) -> Revision {
    let mut identities: Vec<FileIdentity> = files
        .iter()
        .map(|f| FileIdentity {
            rel_path: f.rel_path.clone(),
            digest: statecraft_environment::digest::digest_bytes(f.contents.as_bytes()),
            bytes: f.contents.len() as u64,
        })
        .collect();
    identities.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    let mut material = String::new();
    for i in &identities {
        material.push_str(&i.rel_path);
        material.push('\u{0}');
        material.push_str(&i.digest);
        material.push('\n');
    }
    let full = statecraft_environment::digest::digest_bytes(material.as_bytes());
    Revision {
        id: format!("h-{}", &full[..12]),
        files: identities,
    }
}

/// The harness this build ships.
///
/// Deliberately small. The point of a global harness is that it is one source,
/// not that it is a large one, and every file here has to earn the fact that it
/// is delivered into an operator's agent home.
pub fn shipped() -> Vec<HarnessFile> {
    let mut files = vec![
        HarnessFile {
            rel_path: "rules/statecraft-governed-work.md".to_string(),
            contents: RULE_GOVERNED_WORK.to_string(),
            executable: false,
        },
        HarnessFile {
            rel_path: "skills/statecraft-project/SKILL.md".to_string(),
            contents: SKILL_PROJECT.to_string(),
            executable: false,
        },
        HarnessFile {
            rel_path: "agents/statecraft-acceptance-reader.md".to_string(),
            contents: AGENT_ACCEPTANCE_READER.to_string(),
            executable: false,
        },
        HarnessFile {
            rel_path: "adapters/claude-code.md".to_string(),
            contents: ADAPTER_CLAUDE_CODE.to_string(),
            executable: false,
        },
    ];
    files.extend(ADOPTED_SKILLS.iter().map(|(name, contents)| HarnessFile {
        rel_path: format!("skills/statecraft-{name}/SKILL.md"),
        contents: (*contents).to_string(),
        executable: false,
    }));
    files.extend(ADOPTED_AGENTS.iter().map(|(name, contents)| HarnessFile {
        rel_path: format!("agents/statecraft-{name}.md"),
        contents: (*contents).to_string(),
        executable: false,
    }));
    files.extend(ADOPTED_HOOKS.iter().map(|hook| HarnessFile {
        rel_path: format!("hooks/{}", hook.file),
        contents: hook.contents.to_string(),
        executable: true,
    }));
    files
}

/// The ten skills the owner adopted on 2026-09-21.
///
/// Spec 002 §3.23's inventory, delivered under Statecraft-namespaced names,
/// which is §3.14 rule 2. Each file's own front matter carries the
/// Statecraft-project gate of §3.14 rule 3, so a session choosing a skill sees
/// the gate before it reads the body.
///
/// Held as files rather than as string literals in this module: they are 1900
/// lines of authored prose, they are reviewed as prose, and a diff against the
/// source they were adopted from is only legible while they are files.
pub const ADOPTED_SKILLS: [(&str, &str); 10] = [
    (
        "prime",
        include_str!("../harness/skills/statecraft-prime/SKILL.md"),
    ),
    (
        "next",
        include_str!("../harness/skills/statecraft-next/SKILL.md"),
    ),
    (
        "build",
        include_str!("../harness/skills/statecraft-build/SKILL.md"),
    ),
    (
        "verify",
        include_str!("../harness/skills/statecraft-verify/SKILL.md"),
    ),
    (
        "ship",
        include_str!("../harness/skills/statecraft-ship/SKILL.md"),
    ),
    (
        "shepherd",
        include_str!("../harness/skills/statecraft-shepherd/SKILL.md"),
    ),
    (
        "spec",
        include_str!("../harness/skills/statecraft-spec/SKILL.md"),
    ),
    (
        "commit",
        include_str!("../harness/skills/statecraft-commit/SKILL.md"),
    ),
    (
        "code-review",
        include_str!("../harness/skills/statecraft-code-review/SKILL.md"),
    ),
    (
        "setup",
        include_str!("../harness/skills/statecraft-setup/SKILL.md"),
    ),
];

/// The four agents the owner adopted on 2026-09-21.
pub const ADOPTED_AGENTS: [(&str, &str); 4] = [
    (
        "architect",
        include_str!("../harness/agents/statecraft-architect.md"),
    ),
    (
        "explorer",
        include_str!("../harness/agents/statecraft-explorer.md"),
    ),
    (
        "implementer",
        include_str!("../harness/agents/statecraft-implementer.md"),
    ),
    (
        "reviewer",
        include_str!("../harness/agents/statecraft-reviewer.md"),
    ),
];

/// One adopted event behavior: the harness event, the matcher, and the script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdoptedHook {
    /// The harness event it registers on.
    pub event: &'static str,
    /// The matcher the registration carries.
    pub matcher: &'static str,
    /// The file name under `hooks/`.
    pub file: &'static str,
    /// The script.
    pub contents: &'static str,
    /// Whether this event **enforces** or only **advises**.
    ///
    /// Spec 002 §3.23, as the owner settled it on 2026-09-21. An enforcing
    /// operation gate refuses a failed or unavailable check; an end-of-turn
    /// event reports what the check answered and never withholds a handback.
    pub enforcing: bool,
}

/// The four event behaviors the owner adopted on 2026-09-21.
pub const ADOPTED_HOOKS: [AdoptedHook; 4] = [
    AdoptedHook {
        event: "SessionStart",
        matcher: "startup|resume|clear|compact",
        file: "statecraft-session-start.sh",
        contents: include_str!("../harness/hooks/statecraft-session-start.sh"),
        enforcing: false,
    },
    AdoptedHook {
        event: "PostToolUse",
        matcher: "Edit|Write",
        file: "statecraft-post-edit.sh",
        contents: include_str!("../harness/hooks/statecraft-post-edit.sh"),
        enforcing: false,
    },
    AdoptedHook {
        event: "PreToolUse",
        matcher: "Bash",
        file: "statecraft-pre-bash.sh",
        contents: include_str!("../harness/hooks/statecraft-pre-bash.sh"),
        enforcing: true,
    },
    AdoptedHook {
        event: "Stop",
        matcher: "*",
        file: "statecraft-stop.sh",
        contents: include_str!("../harness/hooks/statecraft-stop.sh"),
        enforcing: false,
    },
];

/// What an install did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    /// The revision.
    pub revision: Revision,
    /// Where it lives.
    pub root: PathBuf,
    /// Paths written by this call, relative to the revision directory.
    pub written: Vec<String>,
    /// Paths already present with the same bytes.
    pub unchanged: Vec<String>,
}

/// Install a harness revision under the home. Idempotent.
///
/// A revision directory is content addressed, so re-installing the same files
/// writes nothing and an operator running `home apply` twice gets one revision,
/// not two. A file present with different bytes inside a content-addressed
/// directory is a corrupted store rather than a user edit, so it is rewritten.
pub fn install(layout: &Layout, files: &[HarnessFile]) -> std::io::Result<Installed> {
    let revision = revision_of(files);
    let root = layout.harness_revision_dir(&revision.id);
    let mut written = Vec::new();
    let mut unchanged = Vec::new();

    for file in files {
        let target = statecraft_environment::claimant::resolve(&root, &file.rel_path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let same = std::fs::read(&target)
            .map(|b| b == file.contents.as_bytes())
            .unwrap_or(false);
        if same {
            unchanged.push(file.rel_path.clone());
        } else {
            std::fs::write(&target, file.contents.as_bytes())?;
            written.push(file.rel_path.clone());
        }
        set_executable(&target, file.executable)?;
    }

    written.sort();
    unchanged.sort();
    Ok(Installed {
        revision,
        root,
        written,
        unchanged,
    })
}

/// Every revision identity present under the home.
pub fn installed_revisions(layout: &Layout) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(layout.harness_dir()) else {
        return Vec::new();
    };
    let mut out: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("h-"))
        .collect();
    out.sort();
    out
}

#[cfg(unix)]
fn set_executable(path: &std::path::Path, executable: bool) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = if executable { 0o755 } else { 0o644 };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn set_executable(_path: &std::path::Path, _executable: bool) -> std::io::Result<()> {
    // No executable bit to set. Inert rather than an error: the harness is data
    // on every platform, and only one of them cares how a script is marked.
    Ok(())
}

const RULE_GOVERNED_WORK: &str = r#"# Statecraft: governed work

Applies only inside a Statecraft project: a repository holding
`.statecraft/environment.json`. Outside one, ignore this file entirely.

- The project's own instructions are at `.statecraft/AGENTS.md`. Read them; they
  are the authority for this repository and this file is not.
- Compiled governance artifacts live under `.statecraft/derived/`. Read them
  only through `spec-spine` subcommands, never with an ad-hoc parse.
- Runtime state lives under `.statecraft/state/` and is not committed. Nothing
  authored belongs there.
- Registration, qualification, arming and execution are four separate acts. A
  registered project is not an armed one, and an armed one has not consented to
  an execution posture.
"#;

const SKILL_PROJECT: &str = r#"---
name: statecraft-project
description: >
  Read a Statecraft project's state: what it declares, what is resolved for a
  run, and what is eligible. Applies only inside a repository holding
  .statecraft/environment.json.
---

# statecraft-project

Applies only inside a Statecraft project: a repository holding
`.statecraft/environment.json`. Outside one, ignore this file entirely.

Read-only orientation, in this order:

```sh
statecraft-cli project list --json
statecraft-cli config show <path> --json
statecraft-cli work list <path> --json
```

`config show` answers where each value came from. A value with no layer behind
it is not a default to assume; it is unknown, and unknown is not success.

Nothing in this skill arms a project or starts a run. Both are explicit
operator acts.
"#;

const AGENT_ACCEPTANCE_READER: &str = r#"---
name: statecraft-acceptance-reader
description: >
  Read a Statecraft run's recorded account and report what it establishes,
  without re-judging it. Applies only inside a repository holding
  .statecraft/environment.json.
---

Applies only inside a Statecraft project: a repository holding
`.statecraft/environment.json`. Outside one, ignore this file entirely.

Read `statecraft-cli run show <path> <run-id> --json` and report what the record
establishes. Do not infer an outcome the record does not carry, and do not treat
a completion the child declared as an acceptance: those are separate records and
the product keeps them separate on purpose.
"#;

const ADAPTER_CLAUDE_CODE: &str = r#"# Adapter template: claude-code

Applies only inside a Statecraft project: a repository holding
`.statecraft/environment.json`. Outside one, ignore this file entirely.

The managed project instructions are `.statecraft/AGENTS.md`. Delivery is
**evaluated**, not assumed: this harness documents expansion of an `@path`
import from its entry file, so the product follows that rule from the entry file
and reports whether the chain arrives at the managed file.

- `reached`: the chain arrives. Nothing is injected.
- `not-reached`: the rule was evaluated and does not arrive. One pointer file
  may be placed, and only where no file exists at that path.
- `unverified`: no documented rule this product can evaluate. Nothing is
  claimed, and nothing is injected on the strength of a file existing.
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_revision_identity_is_a_function_of_the_bytes() {
        let a = revision_of(&shipped());
        let b = revision_of(&shipped());
        assert_eq!(a, b);
        assert!(a.id.starts_with("h-"));
        assert_eq!(a.id.len(), 14);
    }

    #[test]
    fn one_changed_byte_is_a_different_revision() {
        let mut changed = shipped();
        changed[0].contents.push('x');
        assert_ne!(revision_of(&shipped()).id, revision_of(&changed).id);
    }

    #[test]
    fn a_reordered_input_is_the_same_revision() {
        let mut reordered = shipped();
        reordered.reverse();
        assert_eq!(revision_of(&shipped()).id, revision_of(&reordered).id);
    }

    #[test]
    fn every_discoverable_name_is_statecraft_namespaced() {
        // A skill, an agent definition, a rule or a hook is discovered by NAME
        // in a directory shared with the operator's own, so each carries the
        // namespace. An adapter template is documentation this product reads
        // about itself and is discovered by nobody, so it does not.
        for file in shipped() {
            let Some((kind, rest)) = file.rel_path.split_once('/') else {
                panic!("{} has no kind", file.rel_path);
            };
            if kind == "adapters" {
                continue;
            }
            let name = rest.split('/').next().unwrap_or("");
            assert!(
                name.starts_with(NAMESPACE),
                "{} is discovered by name and is not namespaced",
                file.rel_path
            );
        }
    }

    #[test]
    fn every_shipped_file_states_the_gate_that_makes_it_inert_elsewhere() {
        for file in shipped() {
            assert!(
                file.contents.contains(".statecraft/environment.json"),
                "{} does not state the gate",
                file.rel_path
            );
        }
    }

    #[test]
    fn installing_twice_writes_once() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        let first = install(&layout, &shipped()).unwrap();
        assert!(!first.written.is_empty());
        assert!(first.unchanged.is_empty());
        let second = install(&layout, &shipped()).unwrap();
        assert!(second.written.is_empty());
        assert_eq!(second.unchanged.len(), shipped().len());
        assert_eq!(first.revision, second.revision);
        assert_eq!(installed_revisions(&layout), [first.revision.id]);
    }

    #[test]
    fn an_upgrade_adds_a_revision_and_leaves_the_old_one() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        let old = install(&layout, &shipped()).unwrap();
        let mut next = shipped();
        next[0].contents.push_str("\nnew line\n");
        let new = install(&layout, &next).unwrap();
        assert_ne!(old.revision.id, new.revision.id);
        let present = installed_revisions(&layout);
        assert!(present.contains(&old.revision.id));
        assert!(present.contains(&new.revision.id));
    }

    #[cfg(unix)]
    #[test]
    fn every_script_arrives_executable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        let installed = install(&layout, &shipped()).unwrap();
        assert_eq!(ADOPTED_HOOKS.len(), 4);
        for hook in ADOPTED_HOOKS {
            let script = installed.root.join("hooks").join(hook.file);
            let mode = std::fs::metadata(&script)
                .unwrap_or_else(|e| panic!("{} was not installed: {e}", hook.file))
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111, "{} is not executable", hook.file);
        }
    }
}

#[cfg(test)]
mod revision_report {
    /// Print the shipped revision identity. Not an assertion: a way to read
    /// the identity a handoff has to record, without a second binary.
    #[test]
    #[ignore = "reporting, not an assertion: run with --ignored to print"]
    fn print_shipped_revision() {
        let r = super::revision_of(&super::shipped());
        println!("revision {} over {} files", r.id, r.files.len());
        for f in &r.files {
            println!("  {}  {}  {} bytes", &f.digest[..16], f.rel_path, f.bytes);
        }
    }
}
