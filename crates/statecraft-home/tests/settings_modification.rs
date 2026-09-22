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
//!
//! Section 3.24 as the owner revised it on 2026-09-21 identifies an insertion
//! by exact content, structural location **and** recorded provenance, which is
//! established by constructing disagreements between the file and the record:
//!
//! | Requirement | Test |
//! |---|---|
//! | a duplicate key is ambiguous structure | `a_duplicate_json_key_is_ambiguous_structure_and_is_refused` |
//! | ambiguity anywhere, not only on the touched paths | `a_duplicate_key_nested_below_the_touched_paths_is_also_refused` |
//! | the target is a precondition, not only the content | `a_file_changed_between_the_plan_and_the_write_is_not_overwritten` |
//! | a marker is content, and content can be copied | `a_forged_marker_with_no_record_behind_it_is_never_adopted` |
//! | a user's own refusal is never claimed | `a_pre_existing_user_deny_is_never_claimed_and_survives_removal` |
//! | a duplicated value leaves the user's copy | `a_duplicate_deny_value_leaves_the_users_copy_behind` |
//! | multiple valid locations, one recorded modification | `the_insertions_are_not_adjacent_and_neither_is_claimed_by_the_other` |
//! | a record that cannot be read establishes nothing | `a_corrupt_record_is_a_failure_and_the_settings_file_is_untouched` |
//! | removal after a user edit leaves every byte | `removal_after_a_user_edit_reports_and_leaves_every_byte` |
//! | unrelated keys survive reapplication and removal | `reapplying_after_a_user_adds_an_unrelated_key_preserves_it` |

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
    // The four event behaviors the owner adopted on 2026-09-21, each named in
    // the exact lines it would add.
    assert_eq!(plan.adding_hooks.len(), harness::ADOPTED_HOOKS.len());
    for hook in &plan.adding_hooks {
        assert!(plan.render().contains(&hook.event));
        assert!(plan.render().contains(&hook.matcher));
        for line in hook.command.lines() {
            assert!(plan.render().contains(line), "{line} is not in the plan");
        }
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
        .join("hooks/statecraft-session-start.sh");
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

    // Section 3.27: the user's deny list is exactly what it was. Not "their
    // entries kept their place while the floor was appended after them": the
    // floor does not arrive here at all, because a deny entry is evaluated
    // before anything of this product's runs and so carries no project gate.
    assert_eq!(
        deny,
        ["Bash(rm -rf /*)", "Bash(cargo publish*)", "Read(./.env)"],
        "the global deny list was written to"
    );

    // The floor is real and it is delivered per managed session instead.
    let payload = statecraft_home::session::payload();
    let carried: Vec<&str> = payload
        .pointer("/permissions/deny")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    for entry in settings::DENY_FLOOR {
        assert!(
            carried.contains(&entry),
            "{entry} is in neither the file nor the session payload, so the \
             floor was lowered rather than moved"
        );
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
    // Section 3.27: the record describes hook registrations and no deny
    // entry, because no deny entry was placed.
    assert!(record.deny.is_empty(), "{:?}", record.deny);
    assert_eq!(record.hooks.len(), harness::ADOPTED_HOOKS.len());

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
    let edited = written.replace("statecraft-session-start.sh", "my-own-gate.sh");
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

    // The repaired record claims neither half, and the hook is the half worth
    // asserting. Under section 3.24 as revised on 2026-09-21 an insertion is
    // identified by exact content, structural location **and** recorded
    // provenance, conjunctively. The marker is the first of the three. It is
    // also copyable, so a marked registration with no record behind it is
    // content this product cannot prove it placed, and it is reported rather
    // than adopted.
    assert!(
        answer.render().contains("will leave it"),
        "the repair did not report what it declines to claim:\n{}",
        answer.render()
    );
    assert!(
        answer
            .render()
            .contains("a marker is content and content can be copied"),
        "the repair claimed a marked hook it cannot prove it placed:\n{}",
        answer.render()
    );

    suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Remove,
    });
    let left: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path(&sandbox)).unwrap()).unwrap();
    assert!(
        left.get("hooks").is_some(),
        "a marked hook with no recorded provenance was removed on the marker alone"
    );
    assert!(
        left.get("permissions").is_none(),
        "section 3.27: no permissions block should ever have been written here"
    );

    // Nothing was written by the repair, so the file is still the bytes the
    // first apply left. The cost of the lost record is that this product will
    // not take its own content back out, which is the conservative direction
    // and is the one section 3.24 names.
    assert_eq!(
        std::fs::read(settings_path(&sandbox)).unwrap(),
        written,
        "the removal that could claim nothing still changed the file"
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
    assert_eq!(
        groups.len(),
        1,
        "the superseded SessionStart registration accumulated"
    );
    assert!(
        !after.contains("h-000000000000"),
        "the old revision survived"
    );
    assert!(after.contains(&harness::revision_of(&harness::shipped()).id));
    assert!(
        value.get("permissions").is_none(),
        "section 3.27: no deny entry is written into a user's global settings"
    );

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

// ---------------------------------------------------------------------------
// Section 3.24 as the owner revised it on 2026-09-21.
//
// The contract above is satisfied by a property of the file's layout, which a
// reader can see. The revised contract is satisfied by three properties of
// each insertion, of which only the first is visible in the file: exact
// content, structural location, and recorded provenance. A claim of that shape
// is established by exercising the cases where the record and the file
// disagree, so every test below constructs a disagreement and asserts which
// way it is resolved.
// ---------------------------------------------------------------------------

/// The settings file after a consented apply, as bytes.
fn apply_consented(sandbox: &Sandbox, suite: &Suite<'_>) -> Vec<u8> {
    let answer = install_home(sandbox, suite);
    let token = token_from_plan(&answer);
    let answer = suite.harness(sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });
    assert_eq!(
        answer.severity(),
        Severity::Ok,
        "the consented apply did not succeed:\n{}",
        answer.render()
    );
    std::fs::read(settings_path(sandbox)).unwrap()
}

#[test]
fn a_duplicate_json_key_is_ambiguous_structure_and_is_refused() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();

    // Valid JSON, and two structural locations with one name. `serde_json`
    // keeps the last occurrence and is what every judgement here is made
    // against; the text walker takes the first and is what every edit is
    // applied to. A build that guessed would judge one value and edit another.
    let ambiguous = br#"{
  "permissions": { "deny": ["Bash(sudo *)"] },
  "permissions": { "deny": ["Bash(curl *)"] }
}
"#;
    std::fs::write(settings_path(&sandbox), ambiguous).unwrap();

    let answer = install_home(&sandbox, &suite);
    assert!(
        matches!(outcome(&answer), SettingsOutcome::Refused { .. }),
        "a duplicate key was not refused: {:?}",
        outcome(&answer)
    );
    assert!(
        answer.render().contains("more than once"),
        "the refusal did not name the ambiguity:\n{}",
        answer.render()
    );
    assert_eq!(answer.severity(), Severity::Refused, "{}", answer.render());
    assert_eq!(
        std::fs::read(settings_path(&sandbox)).unwrap(),
        ambiguous,
        "an ambiguous file was written to"
    );

    // And consent does not force it through: the refusal is about the
    // document's structure, not about the operator's intent.
    let answer = suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented {
            token: "anything".into(),
        },
    });
    assert_eq!(answer.severity(), Severity::Refused, "{}", answer.render());
    assert_eq!(std::fs::read(settings_path(&sandbox)).unwrap(), ambiguous);
}

#[test]
fn a_duplicate_key_nested_below_the_touched_paths_is_also_refused() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();

    // The never-widen checks compare the two parsed documents whole, so an
    // ambiguity anywhere is an ambiguity in the comparison, not only one in
    // the path being spliced.
    let ambiguous = br#"{
  "env": { "A": "1", "A": "2" },
  "permissions": { "deny": [] }
}
"#;
    std::fs::write(settings_path(&sandbox), ambiguous).unwrap();

    let answer = install_home(&sandbox, &suite);
    assert!(
        matches!(outcome(&answer), SettingsOutcome::Refused { .. }),
        "a nested duplicate key was not refused: {:?}",
        outcome(&answer)
    );
    assert!(
        answer.render().contains("env"),
        "the refusal did not name where the ambiguity is:\n{}",
        answer.render()
    );
    assert_eq!(std::fs::read(settings_path(&sandbox)).unwrap(), ambiguous);
}

#[test]
fn a_file_changed_between_the_plan_and_the_write_is_not_overwritten() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    std::fs::write(
        settings_path(&sandbox),
        b"{\n  \"permissions\": {\n    \"deny\": [\n      \"Bash(sudo *)\"\n    ]\n  }\n}\n",
    )
    .unwrap();

    let answer = install_home(&sandbox, &suite);
    let token = token_from_plan(&answer);

    // The operator reads the plan, and edits their own settings before
    // repeating the token back. Consent names the content to place; it says
    // nothing about the file it is placed into, so the target is a
    // precondition in its own right and is validated before the write.
    let edited = b"{\n  \"permissions\": {\n    \"deny\": [\n      \"Bash(sudo *)\",\n      \"Bash(nc *)\"\n    ]\n  }\n}\n";
    std::fs::write(settings_path(&sandbox), edited).unwrap();
    let before_write = std::fs::read(settings_path(&sandbox)).unwrap();

    let answer = suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });

    assert!(
        matches!(outcome(&answer), SettingsOutcome::ConsentStale { .. }),
        "a consent given against other bytes was honoured: {:?}",
        outcome(&answer)
    );
    assert!(
        answer.render().contains("the file it goes into"),
        "the staleness did not say what the token covers:\n{}",
        answer.render()
    );
    assert_eq!(answer.severity(), Severity::Finding, "{}", answer.render());
    assert_eq!(
        std::fs::read(settings_path(&sandbox)).unwrap(),
        before_write,
        "a file edited after it was inspected was overwritten:\n{}",
        answer.render()
    );

    // The next apply against the file as it now is succeeds and keeps the
    // user's new entry, so the refusal costs an operator one retry and never
    // costs them a line they wrote.

    let answer = install_home(&sandbox, &suite);
    let token = token_from_plan(&answer);
    let answer = suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });
    assert_eq!(answer.severity(), Severity::Ok, "{}", answer.render());
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path(&sandbox)).unwrap()).unwrap();
    let deny = after
        .pointer("/permissions/deny")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(
        deny.iter().any(|v| v == "Bash(nc *)"),
        "the entry the user added between the plan and the write was lost"
    );
    assert!(
        deny.iter().any(|v| v == "Bash(sudo *)"),
        "an entry the user already had was lost"
    );
}

#[test]
fn a_forged_marker_with_no_record_behind_it_is_never_adopted() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();

    // A user copies a marked registration out of somebody's shipped harness,
    // byte for byte, and writes it into their own settings. The content
    // property of section 3.24's three is satisfied; the recorded provenance
    // is not, and the three are conjunctive.
    let revision = harness::revision_of(&harness::shipped());
    let script = sandbox
        .layout()
        .harness_revision_dir(&revision.id)
        .join("hooks/statecraft-session-start.sh");
    let forged = format!(
        "{}\n{{\n  \"hooks\": {{\n    \"{}\": [\n      {{\n        \"matcher\": \"{}\",\n        \"hooks\": [\n          {{\n            \"type\": \"command\",\n            \"command\": {}\n          }}\n        ]\n      }}\n    ]\n  }}\n}}\n",
        "",
        settings::SHIPPED_EVENT,
        settings::SHIPPED_MATCHER,
        serde_json::Value::String(format!(
            "{} {}\n\"{}\" \"${{CLAUDE_PROJECT_DIR:-.}}\"",
            settings::MARKER,
            revision.id,
            script.display()
        ))
    );
    let forged = forged.trim_start().to_string();
    std::fs::write(settings_path(&sandbox), &forged).unwrap();

    let answer = install_home(&sandbox, &suite);
    let token = token_from_plan(&answer);
    let answer = suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });
    assert!(
        answer
            .render()
            .contains("a marker is content and content can be copied"),
        "a registration carrying the marker was adopted on the marker alone:\n{}",
        answer.render()
    );

    let recorded = modifications(&sandbox);
    assert_eq!(recorded.len(), 1);
    assert!(
        !recorded[0]
            .hooks
            .iter()
            .any(|h| h.event == settings::SHIPPED_EVENT),
        "the record claims the forged registration this product did not place: {:?}",
        recorded[0].hooks
    );
    // The three that were genuinely added ARE claimed, so the assertion above
    // is about provenance and not about the record being empty.
    assert_eq!(recorded[0].hooks.len(), harness::ADOPTED_HOOKS.len() - 1);

    // So removal leaves it. The user wrote those bytes and they are the
    // user's, however much they look like this product's.
    suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Remove,
    });
    let left: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path(&sandbox)).unwrap()).unwrap();
    assert!(
        left.pointer(&format!("/hooks/{}", settings::SHIPPED_EVENT))
            .is_some(),
        "a registration this product never placed was removed"
    );
}

#[test]
fn a_pre_existing_user_deny_is_never_claimed_and_survives_removal() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();

    // The user already refuses two of the floor entries, for their own
    // reasons. Both are indistinguishable from the managed copy, which is
    // exactly why neither may be claimed.
    let theirs = [settings::DENY_FLOOR[0], settings::DENY_FLOOR[3]];
    let existing = format!(
        "{{\n  \"permissions\": {{\n    \"deny\": [\n      {},\n      {}\n    ]\n  }}\n}}\n",
        serde_json::Value::String(theirs[0].into()),
        serde_json::Value::String(theirs[1].into())
    );
    std::fs::write(settings_path(&sandbox), &existing).unwrap();

    apply_consented(&sandbox, &suite);

    let recorded = modifications(&sandbox);
    for entry in theirs {
        assert!(
            !recorded[0].deny.contains(&entry.to_string()),
            "{entry} was the user's and was claimed as a managed insertion"
        );
    }

    suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Remove,
    });
    let left: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path(&sandbox)).unwrap()).unwrap();
    let deny = left
        .pointer("/permissions/deny")
        .unwrap()
        .as_array()
        .unwrap();
    for entry in theirs {
        assert!(
            deny.iter().any(|v| v == entry),
            "{entry} was the user's refusal and removal took it out"
        );
    }
    assert_eq!(
        deny.len(),
        theirs.len(),
        "removal left more than the user's own refusals: {deny:?}"
    );
}

#[test]
fn the_deny_floor_never_reaches_a_users_global_settings() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    apply_consented(&sandbox, &suite);

    // Section 3.27. The case this test replaced exercised a deny entry this
    // product had placed being duplicated by the user, and it can no longer
    // arise, because no deny entry is placed here at all. What is asserted
    // instead is the reason: a deny entry is evaluated before anything of
    // this product's runs, so it carries no project gate and would apply in
    // every repository the user opens.
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path(&sandbox)).unwrap()).unwrap();
    assert!(
        after.get("permissions").is_none(),
        "a permissions block reached a user's global settings: {after}"
    );

    // And the floor was not lowered to achieve that: it is carried, whole, by
    // the managed-session payload instead.
    let payload = statecraft_home::session::payload();
    let carried: Vec<&str> = payload
        .pointer("/permissions/deny")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(carried.len(), settings::DENY_FLOOR.len());
    for entry in settings::DENY_FLOOR {
        assert!(carried.contains(&entry), "{entry} left the floor entirely");
    }
}

#[test]
fn a_duplicate_hook_registration_is_ambiguous_and_only_the_recorded_one_leaves() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    apply_consented(&sandbox, &suite);

    // The user copies this product's own `Stop` registration and adds a
    // second, byte-identical, group of their own. Two identical registrations
    // now sit on one event and only one of them is this product's.
    let text = std::fs::read_to_string(settings_path(&sandbox)).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let groups = value
        .pointer_mut("/hooks/Stop")
        .unwrap()
        .as_array_mut()
        .unwrap();
    let theirs = groups[0].clone();
    groups.push(theirs);
    std::fs::write(
        settings_path(&sandbox),
        format!("{}\n", serde_json::to_string_pretty(&value).unwrap()),
    )
    .unwrap();

    suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Remove,
    });
    let left: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path(&sandbox)).unwrap()).unwrap();
    let remaining = left
        .pointer("/hooks/Stop")
        .map(|g| g.as_array().unwrap().len())
        .unwrap_or(0);
    assert_eq!(
        remaining, 1,
        "removal took both copies of an ambiguous registration, or neither: \
         the record describes one and exactly one may leave"
    );
}

#[test]
fn the_insertions_are_not_adjacent_and_neither_is_claimed_by_the_other() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    apply_consented(&sandbox, &suite);

    // Section 3.24 as revised admits multiple syntactically valid locations,
    // which is the whole reason it was revised. This asserts the shape rather
    // than assuming it: the four insertions land at four distinct structural
    // locations, no two of them adjacent in the document, and one recorded
    // modification tracks all four.
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path(&sandbox)).unwrap()).unwrap();
    for hook in harness::ADOPTED_HOOKS {
        assert!(
            after.pointer(&format!("/hooks/{}", hook.event)).is_some(),
            "the {} insertion is not at hooks.{}",
            hook.event,
            hook.event
        );
    }
    assert_eq!(
        after.pointer("/hooks").unwrap().as_object().unwrap().len(),
        harness::ADOPTED_HOOKS.len(),
        "the insertions did not land at one location each"
    );

    let recorded = modifications(&sandbox);
    assert_eq!(
        recorded.len(),
        1,
        "four locations were tracked by more than one recorded modification"
    );
    assert_eq!(recorded[0].hooks.len(), harness::ADOPTED_HOOKS.len());
}

#[test]
fn a_corrupt_record_is_a_failure_and_the_settings_file_is_untouched() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    let written = apply_consented(&sandbox, &suite);

    // The record is the third identifying property. A record this product
    // cannot read establishes nothing about what it owns, and guessing from
    // the file alone is the failure section 3.24 refuses.
    std::fs::write(sandbox.layout().modifications_file(), "{ not json").unwrap();

    let answer = suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Remove,
    });
    assert_eq!(answer.severity(), Severity::Failed, "{}", answer.render());
    assert_eq!(
        std::fs::read(settings_path(&sandbox)).unwrap(),
        written,
        "an unreadable record led to a write"
    );
}

#[test]
fn removal_after_a_user_edit_reports_and_leaves_every_byte() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    apply_consented(&sandbox, &suite);

    // The user edits the managed command. Exact content no longer matches, so
    // ownership cannot be established and nothing comes out, including the
    // deny entries whose own content is still intact: the record describes one
    // modification and it is not intact.
    let text = std::fs::read_to_string(settings_path(&sandbox)).unwrap();
    let edited = text.replace("CLAUDE_PROJECT_DIR:-.", "CLAUDE_PROJECT_DIR:-/tmp");
    assert_ne!(
        edited, text,
        "the fixture did not actually edit the command"
    );
    std::fs::write(settings_path(&sandbox), &edited).unwrap();

    let answer = suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Remove,
    });
    assert_eq!(
        std::fs::read_to_string(settings_path(&sandbox)).unwrap(),
        edited,
        "an edited modification was partly removed"
    );
    assert!(
        answer.render().contains("edited"),
        "the edit was not reported:\n{}",
        answer.render()
    );
    assert_eq!(
        modifications(&sandbox).len(),
        1,
        "the record was dropped, which is the only thing that still says what was placed"
    );
}

#[test]
fn reapplying_after_a_user_adds_an_unrelated_key_preserves_it() {
    let sandbox = sandbox_with_native_home();
    let suite = suite();
    apply_consented(&sandbox, &suite);

    // An unrelated key, with a shape this module never reads, added after the
    // insertions are in place. Reapplication is idempotent and preservation is
    // about bytes, not about keys this module happens to know.
    let text = std::fs::read_to_string(settings_path(&sandbox)).unwrap();
    let with_extra = text.replacen(
        '{',
        "{\n  \"model\": \"opus\",\n  \"env\": { \"K\": \"v\" },",
        1,
    );
    std::fs::write(settings_path(&sandbox), &with_extra).unwrap();

    let answer = install_home(&sandbox, &suite);
    let token = token_from_plan(&answer);
    let answer = suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Consented { token },
    });
    assert_eq!(answer.severity(), Severity::Ok, "{}", answer.render());
    assert_eq!(
        std::fs::read_to_string(settings_path(&sandbox)).unwrap(),
        with_extra,
        "an idempotent reapplication rewrote the file"
    );

    // And removal takes the insertions and leaves the user's keys exactly.
    suite.harness(&sandbox).execute(Operation::HomeApply {
        settings: Intent::Remove,
    });
    let left: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path(&sandbox)).unwrap()).unwrap();
    assert_eq!(left.pointer("/model").unwrap(), "opus");
    assert_eq!(left.pointer("/env/K").unwrap(), "v");
    assert!(left.get("hooks").is_none(), "the hook insertion survived");
}
