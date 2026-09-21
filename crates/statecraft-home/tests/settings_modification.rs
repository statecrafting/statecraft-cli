//! Acceptance for the consented settings modification.
//!
//! Spec 002 section 3.24. Every test here goes through the typed operation
//! boundary against a temporary product home and a temporary native root, and
//! judges the **file on disk and the exit severity**, never a source string. A
//! test that asserted the code contains a phrase would pass against a build
//! that placed the phrase in a comment and wrote nothing.
//!
//! The rows are the requirements of section 3.24, one test each:
//!
//! | Requirement | Test |
//! |---|---|
//! | the plan names the exact lines, and writes nothing | `the_plan_shows_the_exact_lines_and_writes_nothing` |
//! | the command resolves inside the canonical harness | `the_registered_command_is_the_script_the_harness_installed` |
//! | refused by default | `without_consent_nothing_is_written_and_the_withheld_write_is_a_finding` |
//! | installing skills and agents is not consent | `delivering_skills_and_agents_does_not_touch_the_settings_file` |
//! | consent to one revision is not consent to the next | `a_consent_token_for_other_content_is_refused_and_the_current_one_is_shown` |
//! | never a permission | `no_allow_entry_is_added_and_no_ask_setting_is_downgraded` |
//! | the deny floor only ever grows | `existing_deny_entries_keep_their_place_and_the_floor_is_appended` |
//! | `settings.local.json` is never written | `the_local_override_layer_is_never_written` |
//! | unrelated content preserved | `every_byte_outside_the_region_survives_the_write` |
//! | recorded as a modification | `the_modification_records_the_path_the_content_and_both_digests` |
//! | idempotent | `applying_twice_writes_the_same_bytes_and_records_one_modification` |
//! | removal only while intact | `removal_takes_back_exactly_what_was_recorded` |
//! | an edited region is reported and left | `an_edited_region_is_reported_and_kept` |
//! | a conflict is named, not resolved | `a_users_own_hook_on_the_same_event_survives_and_is_reported` |
//! | a malformed file is refused | `a_malformed_settings_file_is_refused_and_left_exactly_as_it_was` |
//! | an interrupted apply is recoverable | `an_apply_interrupted_before_the_record_is_repaired_by_running_it_again` |
//! | a superseded revision comes out before the new one goes in | `a_changed_revision_replaces_the_region_rather_than_accumulating` |

mod support;

use statecraft_home::service::{Answer, Operation, Severity};
use statecraft_home::settings::{self, Intent, Modification, SettingsOutcome};
use statecraft_home::team::Unreachable;
use statecraft_home::{harness, home};
use support::{FixedClock, Harness, Sandbox, StatedProbe, conforming_producer};

/// The suite's harness, with the parts no settings test varies.
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

/// A sandbox whose native `.claude/` exists, because a home that is not there
/// is a different row (`NotApplicable`) and not this one.
fn sandbox_with_native_home() -> Sandbox {
    let sandbox = Sandbox::new();
    std::fs::create_dir_all(sandbox.native_root().join(".claude/agents")).unwrap();
    sandbox
}

fn settings_path(sandbox: &Sandbox) -> std::path::PathBuf {
    sandbox.native_root().join(".claude/settings.json")
}

struct Suite<'a> {
    producer: statecraft_home::producer::Recorded,
    corpus: support::StatedCorpus,
    probe: StatedProbe,
    authority: Unreachable,
    revisions: statecraft_home::authority::StaticRevision,
    _marker: std::marker::PhantomData<&'a ()>,
}

fn suite<'a>() -> Suite<'a> {
    Suite {
        producer: conforming_producer(),
        corpus: support::StatedCorpus::fine(),
        probe: StatedProbe::default(),
        authority: Unreachable::default(),
        revisions: statecraft_home::authority::StaticRevision::default(),
        _marker: std::marker::PhantomData,
    }
}

impl Suite<'_> {
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

/// The settings outcomes inside a `home` answer.
fn outcomes(answer: &Answer) -> &[SettingsOutcome] {
    match answer {
        Answer::HomeChange(change) => &change.settings,
        other => panic!("expected a home change, got {other:?}"),
    }
}

/// The single settings outcome for the one supported harness.
fn outcome(answer: &Answer) -> &SettingsOutcome {
    outcomes(answer)
        .iter()
        .find(|o| !matches!(o, SettingsOutcome::NotApplicable { .. }))
        .unwrap_or_else(|| panic!("no applicable settings outcome in {answer:?}"))
}

/// The consent token the plan printed, read from the plan itself rather than
/// recomputed: an operator consents to what they were shown.
fn token_from_plan(answer: &Answer) -> String {
    match outcome(answer) {
        SettingsOutcome::Withheld { plan } => plan.consent_token.clone(),
        other => panic!("expected a withheld plan, got {other:?}"),
    }
}

fn install_home(sandbox: &Sandbox, suite: &Suite<'_>) -> Answer {
    suite.harness(sandbox).execute(Operation::HomeApply {
        settings: Intent::Withheld,
    })
}

fn modifications(sandbox: &Sandbox) -> Vec<Modification> {
    settings::read_modifications(&sandbox.layout()).expect("the modification record reads")
}

// ---------------------------------------------------------------------------

#[test]
fn the_plan_shows_the_exact_lines_and_writes_nothing() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    let answer = suite.harness(&sandbox).execute(Operation::HomePlan);

    let SettingsOutcome::Withheld { plan } = outcome(&answer) else {
        panic!(
            "a plan is withheld by definition, got {:?}",
            outcome(&answer)
        );
    };
    // Every line that would be added is in the plan, before anything is written.
    for entry in &plan.adding_deny {
        assert!(plan.render().contains(entry), "{entry} is not in the plan");
    }
    assert_eq!(plan.adding_hooks.len(), 1);
    let hook = &plan.adding_hooks[0];
    assert!(plan.render().contains(&hook.event));
    assert!(plan.render().contains(&hook.matcher));
    for line in hook.command.lines() {
        assert!(plan.render().contains(line), "{line} is not in the plan");
    }

    // And nothing was written: `home plan` does not create the file. A plan
    // withholds nothing, so naming the modification is exit 0 and not a
    // finding; only an apply that named one and did not perform it has
    // withheld a write.
    assert!(!settings_path(&sandbox).exists());
    assert_eq!(answer.severity(), Severity::Ok, "{}", answer.render());
}

#[test]
fn the_registered_command_is_the_script_the_harness_installed() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    let answer = install_home(&sandbox, &suite);
    let token = token_from_plan(&answer);

    let answer = suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });
    assert_eq!(answer.severity(), Severity::Ok, "{}", answer.render());

    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path(&sandbox)).unwrap()).unwrap();
    let command = value
        .pointer("/hooks/SessionStart/0/hooks/0/command")
        .and_then(serde_json::Value::as_str)
        .expect("the registration is there");

    // The command's executable is a file the harness install actually placed,
    // inside the canonical source under the product home, and it is executable.
    let revision = harness::revision_of(&harness::shipped()).id;
    let script = sandbox
        .layout()
        .harness_revision_dir(&revision)
        .join("hooks/statecraft-gate.sh");
    assert!(script.is_file(), "{} was not installed", script.display());
    assert!(
        command.contains(&script.display().to_string()),
        "the registered command does not resolve inside the canonical harness: {command}"
    );
    assert!(
        command.starts_with(settings::MARKER),
        "the registration is not marked: {command}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&script).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o111,
            0o111,
            "the registered script is not executable"
        );
    }
}

#[test]
fn without_consent_nothing_is_written_and_the_withheld_write_is_a_finding() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    let answer = install_home(&sandbox, &suite);

    assert!(
        !settings_path(&sandbox).exists(),
        "a file was created anyway"
    );
    assert_eq!(answer.severity(), Severity::Finding, "{}", answer.render());
    assert!(answer.render().contains("refused by default"));
    assert!(
        modifications(&sandbox).is_empty(),
        "a modification was recorded"
    );
}

#[test]
fn delivering_skills_and_agents_does_not_touch_the_settings_file() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    let before = b"{\n  \"theme\": \"mine\"\n}\n";
    std::fs::write(settings_path(&sandbox), before).unwrap();

    install_home(&sandbox, &suite);

    // The namespaced agent arrived; the settings file did not move one byte.
    let linked = sandbox
        .native_root()
        .join(".claude/agents/statecraft-acceptance-reader.md");
    assert!(
        std::fs::symlink_metadata(&linked)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false),
        "installing the harness did not deliver the agent"
    );
    assert_eq!(std::fs::read(settings_path(&sandbox)).unwrap(), before);
}

#[test]
fn a_consent_token_for_other_content_is_refused_and_the_current_one_is_shown() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    let answer = install_home(&sandbox, &suite);
    let real = token_from_plan(&answer);

    let answer = suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented {
            token: "a token for content this operator never saw".into(),
        },
    });
    assert!(
        !settings_path(&sandbox).exists(),
        "a stale consent wrote the file"
    );
    let SettingsOutcome::ConsentStale { expected, .. } = outcome(&answer) else {
        panic!("expected a stale consent, got {:?}", outcome(&answer));
    };
    assert_eq!(expected, &real);
    assert_eq!(answer.severity(), Severity::Finding, "{}", answer.render());
}

#[test]
fn no_allow_entry_is_added_and_no_ask_setting_is_downgraded() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    let before = r#"{
  "permissions": {
    "allow": [
      "Bash(ls *)"
    ],
    "ask": [
      "Bash(git push *)"
    ],
    "defaultMode": "acceptEdits"
  }
}
"#;
    std::fs::write(settings_path(&sandbox), before).unwrap();
    let answer = install_home(&sandbox, &suite);
    let token = token_from_plan(&answer);
    suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });

    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path(&sandbox)).unwrap()).unwrap();
    let original: serde_json::Value = serde_json::from_str(before).unwrap();
    for key in ["allow", "ask", "defaultMode"] {
        assert_eq!(
            after.pointer(&format!("/permissions/{key}")),
            original.pointer(&format!("/permissions/{key}")),
            "permissions.{key} changed"
        );
    }
}

#[test]
fn existing_deny_entries_keep_their_place_and_the_floor_is_appended() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    let before = r#"{
  "permissions": {
    "deny": [
      "Bash(rm -rf /*)",
      "Bash(cargo publish*)",
      "Read(./.env)"
    ]
  }
}
"#;
    std::fs::write(settings_path(&sandbox), before).unwrap();
    let answer = install_home(&sandbox, &suite);
    let token = token_from_plan(&answer);
    suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });

    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path(&sandbox)).unwrap()).unwrap();
    let deny: Vec<&str> = after
        .pointer("/permissions/deny")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();

    // The three the user wrote are still the first three, in their own order.
    assert_eq!(
        &deny[..3],
        &["Bash(rm -rf /*)", "Bash(cargo publish*)", "Read(./.env)"]
    );
    // The floor entry the user already refused is not appended a second time.
    assert_eq!(
        deny.iter()
            .filter(|e| **e == "Bash(cargo publish*)")
            .count(),
        1
    );
    // And every floor entry is now refused.
    for entry in settings::DENY_FLOOR {
        assert!(deny.contains(&entry), "{entry} is not refused");
    }
}

#[test]
fn the_local_override_layer_is_never_written() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    let local = sandbox.native_root().join(".claude/settings.local.json");
    let mine = b"{\n  \"permissions\": { \"allow\": [\"Bash(anything)\"] }\n}\n";
    std::fs::write(&local, mine).unwrap();

    let answer = install_home(&sandbox, &suite);
    let token = token_from_plan(&answer);
    suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });
    suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Remove,
    });

    assert_eq!(std::fs::read(&local).unwrap(), mine);
}

#[test]
fn every_byte_outside_the_region_survives_the_write() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    // Four-space indentation, a comment-free but idiosyncratic shape, and keys
    // this product knows nothing about.
    let before = "{\n    \"$schema\": \"https://json.schemastore.org/claude-code-settings.json\",\n    \"model\": \"opus\",\n    \"statusLine\": { \"type\": \"command\", \"command\": \"mine.sh\" },\n    \"permissions\": {\n        \"allow\": [\n            \"Bash(ls *)\"\n        ]\n    },\n    \"env\": { \"MY_VAR\": \"1\" }\n}\n";
    std::fs::write(settings_path(&sandbox), before).unwrap();

    let answer = install_home(&sandbox, &suite);
    let token = token_from_plan(&answer);
    suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });
    let after = std::fs::read_to_string(settings_path(&sandbox)).unwrap();

    // Not "logically equal": the same bytes, line for line, for every line the
    // author wrote.
    for line in before.lines() {
        if line == "}" {
            continue;
        }
        assert!(
            after.contains(line),
            "the author's line `{line}` did not survive:\n{after}"
        );
    }
    // The author's own key order is intact.
    let keys: Vec<usize> = ["$schema", "model", "statusLine", "permissions", "env"]
        .iter()
        .map(|k| after.find(&format!("\"{k}\"")).expect("the key survives"))
        .collect();
    assert!(keys.windows(2).all(|w| w[0] < w[1]), "keys were reordered");
}

#[test]
fn the_modification_records_the_path_the_content_and_both_digests() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    let before = "{\n  \"model\": \"opus\"\n}\n";
    std::fs::write(settings_path(&sandbox), before).unwrap();

    let answer = install_home(&sandbox, &suite);
    let token = token_from_plan(&answer);
    suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });

    let recorded = modifications(&sandbox);
    assert_eq!(recorded.len(), 1);
    let record = &recorded[0];
    assert_eq!(record.path, settings_path(&sandbox).display().to_string());
    assert_eq!(record.kind, settings::KIND);
    assert_eq!(record.harness, "claude-code");
    assert_eq!(
        record.revision,
        harness::revision_of(&harness::shipped()).id
    );
    assert_eq!(record.deny.len(), settings::DENY_FLOOR.len());
    assert_eq!(record.hooks.len(), 1);

    // The digests are of the actual bytes either side, not of an intention.
    let after = std::fs::read_to_string(settings_path(&sandbox)).unwrap();
    assert_eq!(
        record.digest_before.as_deref(),
        Some(statecraft_environment::digest::digest_bytes(before.as_bytes()).as_str())
    );
    assert_eq!(
        record.digest_after,
        statecraft_environment::digest::digest_bytes(after.as_bytes())
    );

    // A modification, never a managed entry: the file is not claimed as owned.
    assert!(
        !home::Layout::new(sandbox.home())
            .owned_files()
            .iter()
            .any(|f| f == &settings_path(&sandbox))
    );
}

#[test]
fn applying_twice_writes_the_same_bytes_and_records_one_modification() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    let answer = install_home(&sandbox, &suite);
    let token = token_from_plan(&answer);

    suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented {
            token: token.clone(),
        },
    });
    let first = std::fs::read(settings_path(&sandbox)).unwrap();

    let answer = suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });
    let second = std::fs::read(settings_path(&sandbox)).unwrap();

    assert_eq!(first, second, "a second apply changed the file");
    assert_eq!(modifications(&sandbox).len(), 1);
    assert_eq!(answer.severity(), Severity::Ok, "{}", answer.render());
}

#[test]
fn removal_takes_back_exactly_what_was_recorded() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    let before = "{\n  \"model\": \"opus\",\n  \"permissions\": {\n    \"allow\": [\n      \"Bash(ls *)\"\n    ],\n    \"deny\": [\n      \"Read(./.env)\"\n    ]\n  }\n}\n";
    std::fs::write(settings_path(&sandbox), before).unwrap();

    let answer = install_home(&sandbox, &suite);
    let token = token_from_plan(&answer);
    suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });
    assert_ne!(
        std::fs::read_to_string(settings_path(&sandbox)).unwrap(),
        before
    );

    let answer = suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Remove,
    });
    assert_eq!(
        std::fs::read_to_string(settings_path(&sandbox)).unwrap(),
        before,
        "removal did not restore the file byte for byte"
    );
    assert!(modifications(&sandbox).is_empty());
    assert_eq!(answer.severity(), Severity::Ok, "{}", answer.render());

    // Removing again is not a failure: there is nothing recorded to remove.
    let answer = suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Remove,
    });
    let SettingsOutcome::Withdrawn { removal, .. } = outcome(&answer) else {
        panic!("expected a withdrawal, got {:?}", outcome(&answer));
    };
    assert!(matches!(
        removal,
        statecraft_home::settings::Removal::NotPresent { .. }
    ));
}

#[test]
fn an_edited_region_is_reported_and_kept() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    let answer = install_home(&sandbox, &suite);
    let token = token_from_plan(&answer);
    suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });

    // The operator edits the command this product placed. It is theirs now.
    let written = std::fs::read_to_string(settings_path(&sandbox)).unwrap();
    let edited = written.replace("statecraft-gate.sh", "my-own-gate.sh");
    std::fs::write(settings_path(&sandbox), &edited).unwrap();

    let answer = suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Remove,
    });
    assert_eq!(
        std::fs::read_to_string(settings_path(&sandbox)).unwrap(),
        edited,
        "an edited region was taken back"
    );
    let SettingsOutcome::Withdrawn { removal, .. } = outcome(&answer) else {
        panic!("expected a withdrawal, got {:?}", outcome(&answer));
    };
    assert!(
        matches!(removal, statecraft_home::settings::Removal::Edited { .. }),
        "{removal:?}"
    );
    assert!(answer.render().contains("yours now"), "{}", answer.render());
    // The record survives, because it is the only thing that still says what
    // this product put there.
    assert_eq!(modifications(&sandbox).len(), 1);
}

#[test]
fn a_users_own_hook_on_the_same_event_survives_and_is_reported() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    let before = r#"{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "startup",
        "hooks": [
          { "type": "command", "command": "echo my own session hook" }
        ]
      }
    ]
  }
}
"#;
    std::fs::write(settings_path(&sandbox), before).unwrap();

    let answer = install_home(&sandbox, &suite);
    let token = token_from_plan(&answer);
    let answer2 = suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });

    let after = std::fs::read_to_string(settings_path(&sandbox)).unwrap();
    assert!(after.contains("echo my own session hook"), "{after}");
    let value: serde_json::Value = serde_json::from_str(&after).unwrap();
    assert_eq!(
        value
            .pointer("/hooks/SessionStart")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2,
        "both registrations should remain"
    );
    // Both the plan and the applied outcome name the conflict rather than
    // resolving it.
    assert!(
        answer
            .render()
            .contains("does not decide which of two hooks")
    );
    assert!(
        answer2
            .render()
            .contains("does not decide which of two hooks")
    );
}

#[test]
fn a_malformed_settings_file_is_refused_and_left_exactly_as_it_was() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    let broken = b"{ \"permissions\": { \"deny\": [ }\n";
    std::fs::write(settings_path(&sandbox), broken).unwrap();

    let answer = install_home(&sandbox, &suite);
    assert_eq!(std::fs::read(settings_path(&sandbox)).unwrap(), broken);
    assert!(matches!(outcome(&answer), SettingsOutcome::Refused { .. }));
    assert_eq!(answer.severity(), Severity::Refused, "{}", answer.render());

    // And consenting does not force it through: the refusal is about the file,
    // not about the consent.
    let answer = suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented {
            token: "anything".into(),
        },
    });
    assert_eq!(std::fs::read(settings_path(&sandbox)).unwrap(), broken);
    assert_eq!(answer.severity(), Severity::Refused, "{}", answer.render());
}

#[test]
fn an_apply_interrupted_before_the_record_is_repaired_by_running_it_again() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    let answer = install_home(&sandbox, &suite);
    let token = token_from_plan(&answer);
    suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented {
            token: token.clone(),
        },
    });
    let written = std::fs::read(settings_path(&sandbox)).unwrap();

    // The region reached the file and the record did not: the shape an apply
    // interrupted between the two leaves behind.
    std::fs::write(sandbox.layout().modifications_file(), "[]\n").unwrap();

    let answer = suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });
    assert_eq!(
        std::fs::read(settings_path(&sandbox)).unwrap(),
        written,
        "the repair wrote the region a second time"
    );
    assert_eq!(
        modifications(&sandbox).len(),
        1,
        "the record was not repaired"
    );
    assert_eq!(answer.severity(), Severity::Ok, "{}", answer.render());

    // The repaired record claims the hook, which is marked in the file and so
    // provably this product's, and deliberately does **not** claim the
    // refusals, which are not. Removal therefore takes the hook and leaves
    // every deny entry: a lost record costs a refusal nothing.
    assert!(
        answer.render().contains("will leave it"),
        "the repair did not report what it declines to claim:\n{}",
        answer.render()
    );
    suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Remove,
    });
    let left: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path(&sandbox)).unwrap()).unwrap();
    assert!(
        left.get("hooks").is_none(),
        "the marked hook survived removal"
    );
    assert_eq!(
        left.pointer("/permissions/deny")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        settings::DENY_FLOOR.len(),
        "a refusal this product could not prove it placed was removed"
    );
}

#[test]
fn a_changed_revision_replaces_the_region_rather_than_accumulating() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    let path = settings_path(&sandbox).display().to_string();

    // A region this product placed at an earlier revision, and its record.
    let old = settings::managed(
        "h-000000000000",
        &sandbox.layout().harness_revision_dir("h-000000000000"),
    );
    let old_plan = settings::plan(&path, None, &old).unwrap();
    std::fs::write(settings_path(&sandbox), &old_plan.contents_after).unwrap();
    std::fs::create_dir_all(sandbox.home()).unwrap();
    settings::write_modifications(
        &sandbox.layout(),
        &[Modification {
            path: path.clone(),
            kind: settings::KIND.into(),
            harness: "claude-code".into(),
            revision: old.revision.clone(),
            hooks: old_plan.adding_hooks.clone(),
            deny: old_plan.adding_deny.clone(),
            digest_before: None,
            digest_after: old_plan.digest_after.clone(),
            recorded_at: "2026-09-20T00:00:00Z".into(),
        }],
    )
    .unwrap();

    let answer = install_home(&sandbox, &suite);
    let token = token_from_plan(&answer);
    let answer = suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });
    assert_eq!(answer.severity(), Severity::Ok, "{}", answer.render());

    let after = std::fs::read_to_string(settings_path(&sandbox)).unwrap();
    let value: serde_json::Value = serde_json::from_str(&after).unwrap();
    let groups = value
        .pointer("/hooks/SessionStart")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(groups.len(), 1, "the superseded registration accumulated");
    assert!(
        !after.contains("h-000000000000"),
        "the old revision survived"
    );
    assert!(after.contains(&harness::revision_of(&harness::shipped()).id));

    // The deny floor did not double.
    let deny = value
        .pointer("/permissions/deny")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(deny.len(), settings::DENY_FLOOR.len());
    assert_eq!(modifications(&sandbox).len(), 1);
}

#[test]
fn a_native_home_that_does_not_exist_is_not_created_to_hold_a_settings_file() {
    // No `.claude/` here: the sandbox's native root is bare.
    let sandbox = Sandbox::new();
    std::fs::create_dir_all(sandbox.native_root()).unwrap();
    let suite = suite();

    let answer = install_home(&sandbox, &suite);
    assert!(!sandbox.native_root().join(".claude").exists());
    assert!(
        outcomes(&answer)
            .iter()
            .all(|o| matches!(o, SettingsOutcome::NotApplicable { .. })),
        "{:?}",
        outcomes(&answer)
    );
    assert_eq!(answer.severity(), Severity::Ok, "{}", answer.render());
}
