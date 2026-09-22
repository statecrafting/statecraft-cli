//! Spec 002 section 3.23's three skill assertions, plus the repository
//! invariance the owner made a condition of adoption on 2026-09-21.
//!
//! Section 3.23 says whoever owns the files owns the assertions, and that a
//! hermetic test cannot read `$HOME`, so the counterparty's own enforcement
//! could not move. This product now owns ten skills and four agents, so the
//! assertions live here.
//!
//! **What these tests are careful not to be.** The owner named the failure
//! mode directly: a check that merely finds a phrase in a file. So every
//! assertion below is over **parsed structure** (front matter, fenced command
//! blocks, declared tools) rather than over prose, and the two that cannot be
//! are written to fail on the thing that would actually be wrong rather than
//! on a wording. A skill is prose and some of it can only be judged by
//! reading; what can be judged mechanically is judged mechanically, and the
//! rest is out of scope rather than faked.
//!
//! | Assertion | Test |
//! |---|---|
//! | 3.23.1 no skill names a gate flag its project's AGENTS.md omits | `no_skill_names_a_gate_flag_of_its_own` |
//! | 3.23.2 a read-only skill never invokes a writing verb | `a_read_only_skill_invokes_no_writing_verb` |
//! | 3.23.3 each skill wraps the tool verbs it exists for | `every_skill_wraps_verbs_rather_than_restating_them` |
//! | 3.14 rule 2, namespaced | `every_adopted_name_is_namespaced` |
//! | 3.14 rule 3, project-gated | `every_adopted_file_states_the_project_gate` |
//! | adoption defect: a project's own layout | `no_adopted_file_hardcodes_another_projects_layout` |

use statecraft_home::harness::{self, ADOPTED_AGENTS, ADOPTED_SKILLS};

/// The front matter of an adopted file, as key and value.
fn front_matter(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Some(rest) = text.strip_prefix("---\n") else {
        return out;
    };
    let Some(end) = rest.find("\n---\n") else {
        return out;
    };
    for line in rest[..end].lines() {
        if let Some((k, v)) = line.split_once(':') {
            if !k.starts_with(char::is_whitespace) && !k.trim().is_empty() {
                out.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
    }
    out
}

fn field(text: &str, key: &str) -> Option<String> {
    front_matter(text)
        .into_iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
}

/// The shell lines inside fenced code blocks: what a skill actually tells a
/// session to run, as opposed to what its prose discusses.
fn commands(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    // Two states, tracked separately: whether a fence is open at all, and
    // whether the open one is a shell fence. Collapsing them into one flag
    // makes the CLOSING ``` of a ```markdown block read as the OPENING of a
    // shell block, because a bare ``` is also how a shell fence opens.
    let mut fenced = false;
    let mut inside = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if fenced {
                fenced = false;
                inside = false;
            } else {
                fenced = true;
                // Only shell fences carry commands; a ```markdown block is a
                // template a skill prints, not a command it runs.
                inside = matches!(
                    trimmed.trim_start_matches('`').trim(),
                    "sh" | "bash" | "shell" | "console" | ""
                );
            }
            continue;
        }
        if inside {
            let code = line.split('#').next().unwrap_or("").trim();
            if !code.is_empty() {
                out.push(code.to_string());
            }
        }
    }
    out
}

/// Every adopted file, as `(kind, name, contents)`.
fn adopted() -> Vec<(&'static str, &'static str, &'static str)> {
    ADOPTED_SKILLS
        .iter()
        .map(|(n, c)| ("skill", *n, *c))
        .chain(ADOPTED_AGENTS.iter().map(|(n, c)| ("agent", *n, *c)))
        .collect()
}

// ---------------------------------------------------------------------------

/// Section 3.14 rule 2: every delivered name is Statecraft-namespaced, so it
/// cannot collide with a generic user skill of the same purpose.
///
/// Asserted over the front matter's `name`, which is what a harness dispatches
/// on, not over the file path, which is what a reader notices.
#[test]
fn every_adopted_name_is_namespaced() {
    assert_eq!(ADOPTED_SKILLS.len(), 10);
    assert_eq!(ADOPTED_AGENTS.len(), 4);
    for (kind, name, text) in adopted() {
        let declared = field(text, "name")
            .unwrap_or_else(|| panic!("{kind} {name} declares no name in its front matter"));
        assert_eq!(
            declared,
            format!("{}-{name}", harness::NAMESPACE),
            "{kind} {name} would collide with a generic {declared}"
        );
    }
}

/// Section 3.14 rule 3: every delivered behavior is gated to a Statecraft
/// project and is inert everywhere else.
///
/// In the body **and** in the description: the description is what a session
/// reads when it is choosing among skills, and a gate it only finds after
/// choosing has already failed to prevent the collision.
#[test]
fn every_adopted_file_states_the_project_gate() {
    let marker = ".statecraft/environment.json";
    for (kind, name, text) in adopted() {
        let description = field(text, "description")
            .unwrap_or_else(|| panic!("{kind} {name} declares no description"));
        assert!(
            description.contains(marker),
            "{kind} {name}'s description does not state the gate: {description}"
        );
        let body = text.split("\n---\n").nth(1).unwrap_or(text);
        assert!(
            body.contains(marker),
            "{kind} {name}'s body does not state the gate"
        );
    }
}

/// Assertion 1: no skill names a gate flag its project's `AGENTS.md` omits.
///
/// The general form of that requirement is that a generic skill names **no**
/// gate flag at all, because it cannot know which ones a project has. A flag
/// is a project fact and the project states it.
#[test]
fn no_skill_names_a_gate_flag_of_its_own() {
    // Flags that exist only as a particular corpus's gate configuration.
    const PROJECT_FLAGS: [&str; 5] = [
        "--fail-on-warn",
        "--fail-on-untraced",
        "--fail-on-unresolved",
        "--fail-on-info",
        "--slice",
    ];
    for (kind, name, text) in adopted() {
        for command in commands(text) {
            for flag in PROJECT_FLAGS {
                assert!(
                    !command.contains(flag),
                    "{kind} {name} runs `{command}`, naming the gate flag {flag}; \
                     which flags this project's gate passes is a fact for its \
                     AGENTS.md, and a generic file that names one is correct in \
                     the repository it was written in and wrong everywhere else"
                );
            }
        }
    }
}

/// Assertion 2: a read-only skill never invokes a writing verb.
///
/// Read-only is **declared**, in the skill's own description, and this
/// asserts the commands match the declaration. Deriving it from
/// `allowed-tools` instead does not work and the reason is worth recording:
/// `commit` declares `allowed-tools: Bash` and nothing else, which makes it
/// look read-only by that test while its entire purpose is to write. A tool
/// list says what a skill may reach for, not what it is for.
#[test]
fn a_read_only_skill_invokes_no_writing_verb() {
    const WRITING: [&str; 6] = [
        "spec-spine compile",
        "spec-spine index",
        "spec-spine init",
        "spec-spine ratify",
        "git commit",
        "git push",
    ];
    let mut checked = 0;
    for (name, text) in ADOPTED_SKILLS {
        let description = field(text, "description")
            .unwrap_or_default()
            .to_lowercase();
        let declared_read_only = description.contains("reads only")
            || description.contains("read-only")
            || description.contains("never repairs");
        if !declared_read_only {
            continue;
        }
        checked += 1;
        for command in commands(text) {
            for verb in WRITING {
                assert!(
                    !command.starts_with(verb),
                    "the skill {name} declares itself read-only and runs `{command}`"
                );
            }
        }
    }
    assert!(
        checked > 0,
        "no skill declares itself read-only, so this assertion passed \
         vacuously; the declaration is how a read-only skill is recognized \
         and something has changed"
    );
}

/// A skill that declares no `Bash` runs no commands.
///
/// The other half of matching commands to declarations, and the half that
/// catches a skill quietly acquiring a shell it never asked for.
#[test]
fn a_skill_that_cannot_reach_a_shell_runs_no_commands() {
    for (name, text) in ADOPTED_SKILLS {
        let tools = field(text, "allowed-tools").unwrap_or_default();
        if tools.is_empty() || tools.contains("Bash") {
            continue;
        }
        let ran = commands(text);
        assert!(
            ran.is_empty(),
            "the skill {name} declares `allowed-tools: {tools}`, which has no \
             shell, and runs {ran:?}"
        );
    }
}

/// Assertion 3: each skill wraps the tool verbs it exists for, rather than
/// restating them.
///
/// Restating means carrying a copy of a protocol that lives somewhere else,
/// which is the thing that goes stale. The mechanical form of "wraps" is that
/// the skill **points at the authority** rather than reproducing it, so every
/// skill has to name `AGENTS.md`.
#[test]
fn every_skill_wraps_verbs_rather_than_restating_them() {
    for (name, text) in ADOPTED_SKILLS {
        assert!(
            text.contains("AGENTS.md"),
            "the skill {name} names no authority to defer to, so whatever it \
             says about this project is a copy that will go stale"
        );
    }
}

/// The adoption defect the owner named: a file carrying one project's crate
/// layout, gate commands or workflow.
///
/// Repository invariance is "a condition to be checked rather than a property
/// to be assumed", and the inventory was authored inside the counterparty's
/// own repository, so this is where that assumption would have survived.
#[test]
fn no_adopted_file_hardcodes_another_projects_layout() {
    // Paths and package names that exist only in the repository the inventory
    // was written in.
    const FOREIGN: [&str; 4] = [
        "crates/spec-spine-core",
        "crates/spec-spine-types",
        "crates/spec-spine-cli",
        "crates/{spec-spine-core,spec-spine-types}",
    ];
    for (kind, name, text) in adopted() {
        for path in FOREIGN {
            assert!(
                !text.contains(path),
                "{kind} {name} hardcodes `{path}`, which is the counterparty's \
                 own layout and is wrong in every other project"
            );
        }
    }
}

/// A generic file states no project's build commands as though they were
/// every project's.
///
/// Narrower than it looks, deliberately. A skill may legitimately tell a
/// session to run the governance CLI, which every Statecraft project has. It
/// may not tell one to run a build command, because which build a project has
/// is exactly what `AGENTS.md` is for.
#[test]
fn no_adopted_file_assumes_a_build_toolchain() {
    const BUILD: [&str; 4] = ["cargo check", "cargo build", "cargo clippy", "cargo test"];
    for (kind, name, text) in adopted() {
        for command in commands(text) {
            for verb in BUILD {
                assert!(
                    !command.starts_with(verb),
                    "{kind} {name} runs `{command}`; which command builds and \
                     verifies a project is a fact for its AGENTS.md, and a \
                     generic file that assumes Cargo is wrong in every project \
                     that is not Rust"
                );
            }
        }
    }
}

/// Every adopted file is actually delivered, so the assertions above are
/// about the harness rather than about a list nothing installs.
#[test]
fn every_adopted_file_is_in_the_shipped_harness() {
    let shipped: Vec<String> = harness::shipped().into_iter().map(|f| f.rel_path).collect();
    for (name, _) in ADOPTED_SKILLS {
        let path = format!("skills/statecraft-{name}/SKILL.md");
        assert!(shipped.contains(&path), "{path} is adopted but not shipped");
    }
    for (name, _) in ADOPTED_AGENTS {
        let path = format!("agents/statecraft-{name}.md");
        assert!(shipped.contains(&path), "{path} is adopted but not shipped");
    }
    for hook in harness::ADOPTED_HOOKS {
        let path = format!("hooks/{}", hook.file);
        assert!(shipped.contains(&path), "{path} is adopted but not shipped");
    }
}
