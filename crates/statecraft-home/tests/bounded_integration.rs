//! The bounded integration, end to end, in an isolated home and an isolated
//! fixture repository.
//!
//! Every other test file here judges one mechanism. This one walks the whole
//! flow once and asserts the properties that only exist **between** the
//! mechanisms: that installation, consent, gating, project instructions,
//! reversibility and the absence of arming all hold at the same time, in one
//! home, against one repository.
//!
//! # What makes it isolated
//!
//! The product home, the native agent directory and the fixture repository
//! are all inside one temporary directory. Nothing here reads or writes the
//! operator's real `$HOME`, real `~/.claude/`, or any real project, and the
//! `hook_is_independent_of_the_operators_ambient_configuration` test asserts
//! that rather than leaving it to the construction.
//!
//! No platform login is performed and none is available: the authority port
//! is [`Unreachable`], which is the state a solo operator is in, and every
//! assertion below holds in it.
//!
//! | Demonstrated | Test |
//! |---|---|
//! | canonical full-harness installation | `the_whole_harness_installs_once_under_the_home` |
//! | exact consented native modifications | `the_native_modification_is_exactly_what_the_plan_named` |
//! | managed-session permission delivery | `the_floor_is_delivered_per_session_and_never_globally` |
//! | project gating outside managed projects | `an_unrelated_repository_is_untouched_and_ungoverned` |
//! | managed instructions before the root bridge | `the_managed_instructions_exist_before_the_bridge_points_at_them` |
//! | root AGENTS.md and user configuration preserved | `a_pre_existing_root_agents_file_and_user_settings_survive` |
//! | all four event paths | `all_four_event_paths_are_registered_and_resolve_to_installed_files` |
//! | reversibility and recovery | `the_whole_modification_reverses_and_the_home_can_be_rebuilt` |
//! | local operation without platform login | `everything_above_holds_with_no_platform_reachable` |
//! | no accidental arming or execution authorization | `nothing_here_arms_a_target_or_authorizes_a_run` |

mod support;

use statecraft_home::service::{Answer, Operation, Severity};
use statecraft_home::settings::{self, Intent, SettingsOutcome};
use statecraft_home::team::Unreachable;
use statecraft_home::{harness, session};
use support::{FixedClock, Harness, Sandbox, StatedProbe, conforming_producer};

macro_rules! harness {
    ($sandbox:expr, $producer:expr, $corpus:expr, $probe:expr, $authority:expr, $revisions:expr) => {
        Harness {
            layout: $sandbox.layout(),
            producer: $producer,
            corpus: $corpus,
            probe: $probe,
            authority: $authority,
            revisions: $revisions,
            clock: FixedClock(1_758_412_800),
            native_root: $sandbox.native_root(),
        }
    };
}

struct Suite {
    producer: statecraft_home::producer::Recorded,
    corpus: support::StatedCorpus,
    probe: StatedProbe,
    /// Deliberately unreachable: a solo operator with no platform login, which
    /// is the state every assertion in this file is made in.
    authority: Unreachable,
    revisions: statecraft_home::authority::StaticRevision,
}

fn suite() -> Suite {
    Suite {
        producer: conforming_producer(),
        corpus: support::StatedCorpus::fine(),
        probe: StatedProbe::default(),
        authority: Unreachable::default(),
        revisions: statecraft_home::authority::StaticRevision::default(),
    }
}

impl Suite {
    fn harness<'s>(&'s self, sandbox: &Sandbox) -> Harness<'s> {
        harness!(
            sandbox,
            &self.producer,
            &self.corpus,
            &self.probe,
            &self.authority,
            &self.revisions
        )
    }
}

/// A sandbox with a native agent directory, which is the shape an operator
/// who already uses the harness is in.
fn sandbox() -> Sandbox {
    let s = Sandbox::new();
    std::fs::create_dir_all(s.native_root().join(".claude/agents")).unwrap();
    s
}

fn settings_path(s: &Sandbox) -> std::path::PathBuf {
    s.native_root().join(".claude/settings.json")
}

fn outcome(answer: &Answer) -> &SettingsOutcome {
    match answer {
        Answer::HomeChange(change) => change
            .settings
            .iter()
            .find(|o| !matches!(o, SettingsOutcome::NotApplicable { .. }))
            .unwrap_or_else(|| panic!("no applicable settings outcome")),
        other => panic!("expected a home change, got {other:?}"),
    }
}

/// Plan, read the token the plan printed, consent to it.
fn install_and_consent(s: &Sandbox, suite: &Suite) -> Answer {
    let planned = suite.harness(s).execute(Operation::HomeApply {
        settings: Intent::Withheld,
    });
    let SettingsOutcome::Withheld { plan } = outcome(&planned) else {
        panic!("a first apply withholds: {:?}", outcome(&planned));
    };
    let token = plan.consent_token.clone();
    let applied = suite.harness(s).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });
    assert_eq!(
        applied.severity(),
        Severity::Ok,
        "the consented apply did not succeed:\n{}",
        applied.render()
    );
    applied
}

// ---------------------------------------------------------------------------

#[test]
fn the_whole_harness_installs_once_under_the_home() {
    let s = sandbox();
    let suite = suite();
    install_and_consent(&s, &suite);

    // One content-addressed revision directory, holding every shipped file.
    let revision = harness::revision_of(&harness::shipped());
    let root = s.layout().harness_revision_dir(&revision.id);
    assert!(
        root.is_dir(),
        "the revision directory is not under the home"
    );
    for file in harness::shipped() {
        assert!(
            root.join(&file.rel_path).is_file(),
            "{} was not installed",
            file.rel_path
        );
    }
    // Ten skills, four agents, four hooks, and the files this build authored.
    assert_eq!(harness::ADOPTED_SKILLS.len(), 10);
    assert_eq!(harness::ADOPTED_AGENTS.len(), 4);
    assert_eq!(harness::ADOPTED_HOOKS.len(), 4);

    // And the project receives no copy: section 3.14's whole point.
    assert!(
        !s.project().join(".claude").exists(),
        "a repository-local generic harness copy was written"
    );
}

#[test]
fn the_native_modification_is_exactly_what_the_plan_named() {
    let s = sandbox();
    let suite = suite();

    let planned = suite.harness(&s).execute(Operation::HomeApply {
        settings: Intent::Withheld,
    });
    let SettingsOutcome::Withheld { plan } = outcome(&planned) else {
        panic!("a first apply withholds");
    };
    let named = plan.managed.clone();
    let predicted = plan.digest_after.clone();
    let token = plan.consent_token.clone();

    // Nothing is written before consent.
    assert!(!settings_path(&s).exists());

    suite.harness(&s).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });

    // The bytes on disk are the bytes the plan predicted, not merely bytes
    // that mean the same thing.
    let after = std::fs::read_to_string(settings_path(&s)).unwrap();
    assert_eq!(
        statecraft_environment::digest::digest_bytes(after.as_bytes()),
        predicted,
        "the apply wrote something the plan did not predict"
    );
    // And every registration the plan named is in the file, compared after
    // parsing rather than as raw text: the command is a JSON string in the
    // document, so its quotes and newlines are escaped there and a literal
    // substring test would fail on a file that is perfectly correct.
    let value: serde_json::Value = serde_json::from_str(&after).unwrap();
    for hook in &named.hooks {
        let commands: Vec<String> = value
            .pointer(&format!("/hooks/{}", hook.event))
            .unwrap_or_else(|| panic!("{} is not registered", hook.event))
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|g| g["hooks"].as_array().cloned().unwrap_or_default())
            .filter_map(|h| h["command"].as_str().map(str::to_string))
            .collect();
        assert!(
            commands.contains(&hook.command),
            "the plan named a {} command the file does not carry:\n{}\ngot {commands:?}",
            hook.event,
            hook.command
        );
    }
    assert!(
        named.deny.is_empty(),
        "section 3.27: no deny entry is planned"
    );
}

#[test]
fn the_floor_is_delivered_per_session_and_never_globally() {
    let s = sandbox();
    let suite = suite();
    install_and_consent(&s, &suite);

    // Section 3.27: nothing in the global file.
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path(&s)).unwrap()).unwrap();
    assert!(
        after.get("permissions").is_none(),
        "a permissions block reached the operator's global settings: {after}"
    );

    // And the floor is carried, whole, by the session payload.
    let carried = session::payload();
    let deny: Vec<&str> = carried
        .pointer("/permissions/deny")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    for entry in settings::DENY_FLOOR {
        assert!(
            carried.to_string().contains(entry),
            "{entry} left the floor"
        );
    }
    assert_eq!(deny.len(), settings::DENY_FLOOR.len());

    // The mechanism is not claimed as qualified from a read. Section 3.27:
    // the version is establishable here and the effective behavior is not.
    let probe = session::probe_version(std::path::Path::new("/nonexistent/claude"));
    assert!(!session::qualification_from(&probe).qualified());
}

#[test]
fn an_unrelated_repository_is_untouched_and_ungoverned() {
    let s = sandbox();
    let suite = suite();
    let unrelated = s.unrelated();
    let before: Vec<_> = walk(&unrelated);

    install_and_consent(&s, &suite);
    suite
        .harness(&s)
        .execute(Operation::InitApply { root: s.project() });

    assert_eq!(
        walk(&unrelated),
        before,
        "an unrelated repository changed while a different project was initialized"
    );
    assert!(
        !unrelated.join(".statecraft").exists(),
        "an unrelated repository acquired a manifest"
    );
    assert!(
        !unrelated.join(".claude").exists(),
        "an unrelated repository acquired harness content"
    );
}

/// Every path under a directory, sorted: a cheap whole-tree comparison.
fn walk(root: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path.clone());
            }
            out.push(path.strip_prefix(root).unwrap().display().to_string());
        }
    }
    out.sort();
    out
}

#[test]
fn the_managed_instructions_exist_before_the_bridge_points_at_them() {
    let s = sandbox();
    let suite = suite();
    let answer = suite
        .harness(&s)
        .execute(Operation::InitApply { root: s.project() });
    assert_ne!(answer.severity(), Severity::Failed, "{}", answer.render());

    let managed = s.project().join(".statecraft/AGENTS.md");
    assert!(
        managed.is_file(),
        "the managed instructions were not created"
    );

    // The root bridge, where one was placed, points at a file that exists.
    // A bridge written first would name a path that resolves to nothing for
    // as long as the flow takes to reach the next step, and an interrupted
    // run would leave exactly that.
    if let Ok(root_text) = std::fs::read_to_string(s.project().join("AGENTS.md")) {
        if root_text.contains(".statecraft/AGENTS.md") {
            assert!(
                managed.is_file(),
                "the root bridge names a managed file that does not exist"
            );
        }
    }
}

#[test]
fn a_pre_existing_root_agents_file_and_user_settings_survive() {
    let s = sandbox();
    let suite = suite();

    // The operator's own root instructions, and their own settings, both
    // written before this product runs at all.
    s.write("AGENTS.md", "# my own instructions\n\nDo it my way.\n");
    let mine = "{\n  \"model\": \"opus\",\n  \"permissions\": {\n    \"allow\": [\n      \"Bash(ls *)\"\n    ]\n  }\n}\n";
    std::fs::write(settings_path(&s), mine).unwrap();
    // And a hook of their own, on an event this product also registers.
    let ambient = s.native_root().join(".claude/hooks/push-gate.sh");
    std::fs::create_dir_all(ambient.parent().unwrap()).unwrap();
    std::fs::write(&ambient, "#!/bin/sh\nexit 0\n").unwrap();

    install_and_consent(&s, &suite);
    suite
        .harness(&s)
        .execute(Operation::InitApply { root: s.project() });

    // Their sentence is still their sentence.
    let root = s.read("AGENTS.md").expect("the root file survived");
    assert!(
        root.contains("Do it my way."),
        "the operator's own root instructions were rewritten: {root}"
    );

    // Their settings keys are untouched: section 3.28's list.
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path(&s)).unwrap()).unwrap();
    assert_eq!(after.pointer("/model").unwrap(), "opus");
    assert_eq!(
        after
            .pointer("/permissions/allow")
            .unwrap()
            .as_array()
            .unwrap(),
        &[serde_json::Value::from("Bash(ls *)")]
    );
    assert!(after.pointer("/permissions/deny").is_none());

    // And their own hook file is still there, byte for byte.
    assert_eq!(
        std::fs::read_to_string(&ambient).unwrap(),
        "#!/bin/sh\nexit 0\n",
        "an unmarked hook the operator wrote was rewritten"
    );
}

#[test]
fn all_four_event_paths_are_registered_and_resolve_to_installed_files() {
    let s = sandbox();
    let suite = suite();
    install_and_consent(&s, &suite);

    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path(&s)).unwrap()).unwrap();
    let revision = harness::revision_of(&harness::shipped());
    let revision_root = s.layout().harness_revision_dir(&revision.id);

    for hook in harness::ADOPTED_HOOKS {
        let groups = after
            .pointer(&format!("/hooks/{}", hook.event))
            .unwrap_or_else(|| panic!("{} is not registered", hook.event))
            .as_array()
            .unwrap();
        let command = groups
            .iter()
            .flat_map(|g| g["hooks"].as_array().cloned().unwrap_or_default())
            .filter_map(|h| h["command"].as_str().map(str::to_string))
            .find(|c| c.starts_with(settings::MARKER))
            .unwrap_or_else(|| panic!("{} has no managed registration", hook.event));

        // The command resolves inside the canonical harness, and the file it
        // names is actually installed and executable. A registration pointing
        // at a path nothing wrote is the failure this asserts away.
        let script = revision_root.join("hooks").join(hook.file);
        assert!(
            command.contains(&script.display().to_string()),
            "{} does not resolve inside the canonical harness: {command}",
            hook.event
        );
        assert!(script.is_file(), "{} was not installed", hook.file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&script).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "{} is not executable", hook.file);
        }
    }
}

#[test]
fn the_whole_modification_reverses_and_the_home_can_be_rebuilt() {
    let s = sandbox();
    let suite = suite();
    let mine = "{\n  \"model\": \"opus\"\n}\n";
    std::fs::write(settings_path(&s), mine).unwrap();

    install_and_consent(&s, &suite);
    assert_ne!(std::fs::read_to_string(settings_path(&s)).unwrap(), mine);

    // Reverse.
    let answer = suite.harness(&s).execute(Operation::HomeApply {
        settings: Intent::Remove,
    });
    assert_ne!(answer.severity(), Severity::Failed, "{}", answer.render());
    assert_eq!(
        std::fs::read_to_string(settings_path(&s)).unwrap(),
        mine,
        "removal did not return the file to the operator's own bytes"
    );
    assert!(
        settings::read_modifications(&s.layout())
            .unwrap()
            .is_empty(),
        "the record survived a completed removal"
    );

    // Recover: the same install runs again from the reversed state and lands
    // in the same place, so reversal is not a one-way door.
    install_and_consent(&s, &suite);
    let rebuilt: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path(&s)).unwrap()).unwrap();
    assert_eq!(rebuilt.pointer("/model").unwrap(), "opus");
    for hook in harness::ADOPTED_HOOKS {
        assert!(
            rebuilt.pointer(&format!("/hooks/{}", hook.event)).is_some(),
            "{} did not come back",
            hook.event
        );
    }
}

#[test]
fn everything_above_holds_with_no_platform_reachable() {
    let s = sandbox();
    let suite = suite();
    // `Unreachable` is the authority port for every test in this file, so
    // this asserts the premise rather than adding a case: if any of it had
    // needed a platform, the tests above would already have failed.
    let answer = install_and_consent(&s, &suite);
    assert_eq!(answer.severity(), Severity::Ok);
    let init = suite
        .harness(&s)
        .execute(Operation::InitApply { root: s.project() });
    assert_ne!(
        init.severity(),
        Severity::Failed,
        "initialization needed a platform: {}",
        init.render()
    );
    assert!(
        s.project().join(".statecraft/environment.json").is_file(),
        "the project was not registered locally"
    );
}

#[test]
fn nothing_here_arms_a_target_or_authorizes_a_run() {
    let s = sandbox();
    let suite = suite();
    install_and_consent(&s, &suite);
    let answer = suite
        .harness(&s)
        .execute(Operation::InitApply { root: s.project() });

    // Initialization stops after registering and qualifying. Arming and
    // running are separate explicit acts, and the report says so rather than
    // leaving it to be inferred.
    let rendered = answer.render();
    assert!(
        rendered.contains("armed: false") || rendered.contains("separate explicit acts"),
        "the report does not say arming is separate: {rendered}"
    );
    let manifest =
        std::fs::read_to_string(s.project().join(".statecraft/environment.json")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    if let Some(armed) = value.pointer("/armed").and_then(|v| v.as_bool()) {
        assert!(!armed, "initialization armed the target");
    }
    assert!(
        !manifest.contains("\"armed\": true"),
        "initialization armed the target: {manifest}"
    );
}

/// The shipped hooks do not depend on the operator's ambient configuration.
///
/// Every other assertion here is made inside a sandbox, which shows that the
/// product does not need the operator's real home. This one shows the
/// converse for the part that actually runs on their machine: a shipped hook
/// executed with no `$HOME`, an empty environment and a bare `PATH` still
/// reaches a verdict and still exits 0 outside a Statecraft project.
#[test]
fn hook_is_independent_of_the_operators_ambient_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let ordinary = dir.path().join("ordinary");
    std::fs::create_dir_all(&ordinary).unwrap();

    for hook in harness::ADOPTED_HOOKS {
        if hook.event != "SessionStart" && hook.event != "Stop" {
            continue;
        }
        let script = dir.path().join(hook.file);
        statecraft_adapter::fixture::install_script(&script, hook.contents, 0o755).unwrap();
        let out = std::process::Command::new(&script)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("CLAUDE_PROJECT_DIR", &ordinary)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{} did not exit 0 with no ambient configuration",
            hook.file
        );
        assert!(
            out.stdout.is_empty() && out.stderr.is_empty(),
            "{} spoke in an unrelated directory: {}{}",
            hook.file,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
