//! The one canonical harness source, maintained globally and copied into no
//! repository.
//!
//! Spec 010 section 3.4. Skills, agent definitions, rules, hooks and adapter
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
    vec![
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
            rel_path: "hooks/statecraft-gate.sh".to_string(),
            contents: HOOK_GATE.to_string(),
            executable: true,
        },
        HarnessFile {
            rel_path: "adapters/claude-code.md".to_string(),
            contents: ADAPTER_CLAUDE_CODE.to_string(),
            executable: false,
        },
    ]
}

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

const HOOK_GATE: &str = r#"#!/bin/sh
# Statecraft: the project gate, as a hook an operator may wire up themselves.
#
# Applies only inside a Statecraft project: a repository holding
# `.statecraft/environment.json`. Outside one this exits 0 and does nothing,
# which is what makes it safe to place on a path shared with other work.
#
# This product does NOT wire this hook into an agent's settings. Doing so would
# mean rewriting a user's own configuration file, and spec 010 section 3.4
# refuses that. Install it yourself if you want it.
set -eu

root=$(git rev-parse --show-toplevel 2>/dev/null || true)
[ -n "$root" ] || exit 0
[ -f "$root/.statecraft/environment.json" ] || exit 0

cd "$root"
spec-spine check --fail-on-warn
spec-spine lint --fail-on-warn
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
    fn a_script_arrives_executable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        let installed = install(&layout, &shipped()).unwrap();
        let script = installed.root.join("hooks/statecraft-gate.sh");
        let mode = std::fs::metadata(&script).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111);
    }
}
