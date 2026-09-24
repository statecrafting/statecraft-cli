//! Spec 002 section 3.13 rule 4, the library half: "Removal removes the
//! inserted line and nothing else, and only while the file still begins with
//! it." The binary half is `crates/statecraft-cli/tests/env_remove_bridge.rs`.

use statecraft_environment::apply::{BridgeSite, Outcome, remove_with};
use statecraft_environment::digest::digest_bytes;
use statecraft_environment::manifest::{Manifest, Modification, ModificationKind, Pins};
use statecraft_environment::time::FixedClock;
use std::collections::BTreeMap;
use std::path::Path;

const LINE: &str = "@.statecraft/AGENTS.md";

fn clock() -> FixedClock {
    FixedClock(1_700_000_000)
}

fn sites() -> Vec<BridgeSite> {
    vec![BridgeSite {
        path: "AGENTS.md".into(),
        kind: ModificationKind::ImportBridge,
        line: LINE.into(),
    }]
}

fn manifest_with(target: &Path, before: Option<&str>, after: &str) -> Manifest {
    let mut m = Manifest::new(Pins {
        product: "0.0.0".into(),
        spec_spine: "0.23.0".into(),
        adapters: BTreeMap::new(),
        producer: None,
    });
    m.upsert_modification(Modification {
        path: "AGENTS.md".into(),
        kind: ModificationKind::ImportBridge,
        line: LINE.into(),
        digest_before: before.map(|b| digest_bytes(b.as_bytes())),
        digest_after: digest_bytes(after.as_bytes()),
        written_at: "2026-09-23T00:00:00Z".into(),
    });
    m.write(target).unwrap();
    m
}

fn agents(target: &Path) -> Option<String> {
    std::fs::read_to_string(target.join("AGENTS.md")).ok()
}

#[test]
fn removal_takes_the_line_and_its_separator_and_restores_the_users_bytes() {
    let target = tempfile::tempdir().unwrap();
    let user = "# Mine\n\nrules\n";
    let bridged = format!("{LINE}\n\n{user}");
    std::fs::write(target.path().join("AGENTS.md"), &bridged).unwrap();
    manifest_with(target.path(), Some(user), &bridged);

    let outcome = remove_with(target.path(), &clock(), &sites())
        .unwrap()
        .outcome;

    assert_eq!(outcome.word(), "applied", "{outcome:?}");
    assert_eq!(agents(target.path()).as_deref(), Some(user));
    let after = Manifest::read(target.path()).unwrap().unwrap();
    assert!(
        after.modifications.is_empty(),
        "the record goes with the line"
    );
}

#[test]
fn edits_the_user_made_around_the_bridge_are_kept() {
    let target = tempfile::tempdir().unwrap();
    let user = "# Mine\n\nrules\n";
    let bridged = format!("{LINE}\n\n{user}");
    manifest_with(target.path(), Some(user), &bridged);
    let edited = format!("{LINE}\n\n# Mine, renamed\n\nrules\nand a new one\n");
    std::fs::write(target.path().join("AGENTS.md"), &edited).unwrap();

    let outcome = remove_with(target.path(), &clock(), &sites())
        .unwrap()
        .outcome;

    assert_eq!(outcome.word(), "applied", "{outcome:?}");
    assert_eq!(
        agents(target.path()).as_deref(),
        Some("# Mine, renamed\n\nrules\nand a new one\n")
    );
}

#[test]
fn a_bridge_that_is_no_longer_first_is_left_and_its_record_kept() {
    let target = tempfile::tempdir().unwrap();
    let user = "# Mine\n";
    manifest_with(target.path(), Some(user), &format!("{LINE}\n\n{user}"));
    let moved = format!("# Mine\n\n{LINE}\n");
    std::fs::write(target.path().join("AGENTS.md"), &moved).unwrap();

    match remove_with(target.path(), &clock(), &sites())
        .unwrap()
        .outcome
    {
        Outcome::Partial { withheld, .. } => {
            assert_eq!(withheld[0].path, "AGENTS.md");
            assert!(withheld[0].reason.describe().contains("no longer begins"));
        }
        other => panic!("expected partial, got {other:?}"),
    }
    assert_eq!(agents(target.path()), Some(moved));
    assert_eq!(
        Manifest::read(target.path())
            .unwrap()
            .unwrap()
            .modifications
            .len(),
        1
    );
}

#[test]
fn an_edited_line_is_not_the_recorded_line_and_is_left() {
    let target = tempfile::tempdir().unwrap();
    manifest_with(target.path(), Some("x\n"), &format!("{LINE}\n\nx\n"));
    let edited = "@.statecraft/AGENTS.md see also\n\nx\n";
    std::fs::write(target.path().join("AGENTS.md"), edited).unwrap();

    let outcome = remove_with(target.path(), &clock(), &sites())
        .unwrap()
        .outcome;

    assert_eq!(outcome.word(), "partial");
    assert_eq!(agents(target.path()).as_deref(), Some(edited));
}

#[test]
fn two_copies_of_the_line_are_ambiguous_and_left() {
    let target = tempfile::tempdir().unwrap();
    manifest_with(target.path(), Some("x\n"), &format!("{LINE}\n\nx\n"));
    let two = format!("{LINE}\n\nx\n{LINE}\n");
    std::fs::write(target.path().join("AGENTS.md"), &two).unwrap();

    match remove_with(target.path(), &clock(), &sites())
        .unwrap()
        .outcome
    {
        Outcome::Partial { withheld, .. } => {
            assert!(withheld[0].reason.describe().contains("2 times"))
        }
        other => panic!("expected partial, got {other:?}"),
    }
    assert_eq!(agents(target.path()), Some(two));
}

#[test]
fn two_records_for_one_path_are_ambiguous_and_left() {
    let target = tempfile::tempdir().unwrap();
    let mut m = manifest_with(target.path(), Some("x\n"), &format!("{LINE}\n\nx\n"));
    let copy = m.modifications[0].clone();
    m.modifications.push(copy);
    m.write(target.path()).unwrap();
    let bridged = format!("{LINE}\n\nx\n");
    std::fs::write(target.path().join("AGENTS.md"), &bridged).unwrap();

    match remove_with(target.path(), &clock(), &sites())
        .unwrap()
        .outcome
    {
        Outcome::Partial { withheld, .. } => {
            assert!(withheld[0].reason.describe().contains("2 modifications"))
        }
        other => panic!("expected partial, got {other:?}"),
    }
    assert_eq!(agents(target.path()), Some(bridged));
}

#[test]
fn a_bridge_with_no_record_is_a_note_and_left() {
    let target = tempfile::tempdir().unwrap();
    Manifest::new(Pins {
        product: "0.0.0".into(),
        spec_spine: "0.23.0".into(),
        adapters: BTreeMap::new(),
        producer: None,
    })
    .write(target.path())
    .unwrap();
    let text = format!("{LINE}\n\nmine\n");
    std::fs::write(target.path().join("AGENTS.md"), &text).unwrap();

    let removal = remove_with(target.path(), &clock(), &sites()).unwrap();

    assert_eq!(removal.outcome.word(), "applied", "a note is not a finding");
    assert!(
        removal.notes[0].contains("records no bridge"),
        "{removal:?}"
    );
    assert_eq!(agents(target.path()), Some(text));
}

#[test]
fn a_record_with_no_file_is_reported_and_kept() {
    let target = tempfile::tempdir().unwrap();
    manifest_with(target.path(), Some("x\n"), &format!("{LINE}\n\nx\n"));

    let outcome = remove_with(target.path(), &clock(), &sites())
        .unwrap()
        .outcome;

    assert_eq!(outcome.word(), "partial");
    assert!(agents(target.path()).is_none(), "nothing is created");
    assert_eq!(
        Manifest::read(target.path())
            .unwrap()
            .unwrap()
            .modifications
            .len(),
        1
    );
}

#[test]
fn a_file_this_product_created_is_kept_empty_when_only_the_line_was_in_it() {
    let target = tempfile::tempdir().unwrap();
    let created = format!("{LINE}\n");
    std::fs::write(target.path().join("AGENTS.md"), &created).unwrap();
    manifest_with(target.path(), None, &created);

    let outcome = remove_with(target.path(), &clock(), &sites())
        .unwrap()
        .outcome;

    assert_eq!(outcome.word(), "applied", "{outcome:?}");
    assert_eq!(
        agents(target.path()).as_deref(),
        Some(""),
        "the line is removed, and nothing else: the file stays"
    );
}

#[test]
fn a_created_file_the_user_wrote_in_keeps_their_lines() {
    let target = tempfile::tempdir().unwrap();
    let created = format!("{LINE}\n");
    manifest_with(target.path(), None, &created);
    std::fs::write(
        target.path().join("AGENTS.md"),
        format!("{LINE}\n\nadded later\n"),
    )
    .unwrap();

    remove_with(target.path(), &clock(), &sites()).unwrap();

    assert_eq!(
        agents(target.path()).as_deref(),
        Some("\nadded later\n"),
        "only the line this product created is taken; the user's blank line and text stay"
    );
}

#[test]
fn removing_again_after_success_changes_nothing() {
    let target = tempfile::tempdir().unwrap();
    let user = "# Mine\n";
    let bridged = format!("{LINE}\n\n{user}");
    std::fs::write(target.path().join("AGENTS.md"), &bridged).unwrap();
    manifest_with(target.path(), Some(user), &bridged);
    remove_with(target.path(), &clock(), &sites()).unwrap();

    let again = remove_with(target.path(), &clock(), &sites())
        .unwrap()
        .outcome;

    assert_eq!(again.word(), "applied", "{again:?}");
    assert_eq!(agents(target.path()).as_deref(), Some(user));
}

#[test]
fn a_record_of_no_insertion_gives_no_authority() {
    let target = tempfile::tempdir().unwrap();
    // What an init over a file that already began with the line recorded
    // before the fix: the same digest before and after.
    let text = format!("{LINE}\n\n# Mine\n");
    std::fs::write(target.path().join("AGENTS.md"), &text).unwrap();
    manifest_with(target.path(), Some(&text), &text);

    let outcome = remove_with(target.path(), &clock(), &sites())
        .unwrap()
        .outcome;

    assert_eq!(outcome.word(), "partial", "{outcome:?}");
    assert_eq!(agents(target.path()), Some(text));
}

#[test]
fn a_record_naming_a_path_outside_the_repository_is_never_acted_on() {
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("repo");
    std::fs::create_dir_all(&target).unwrap();
    let outside = format!("{LINE}\n\nnot yours\n");
    std::fs::write(parent.path().join("outside.md"), &outside).unwrap();
    let mut m = manifest_with(&target, Some("x"), &outside);
    m.modifications[0].path = "../outside.md".into();
    m.write(&target).unwrap();

    let outcome = remove_with(&target, &clock(), &sites()).unwrap().outcome;

    assert_eq!(outcome.word(), "partial", "{outcome:?}");
    assert_eq!(
        std::fs::read_to_string(parent.path().join("outside.md")).unwrap(),
        outside
    );
}

#[test]
fn a_record_that_matches_no_bridge_site_is_left() {
    let target = tempfile::tempdir().unwrap();
    let text = format!("{LINE}\n\nnotes\n");
    std::fs::write(target.path().join("NOTES.md"), &text).unwrap();
    let mut m = manifest_with(target.path(), Some("notes\n"), &text);
    m.modifications[0].path = "NOTES.md".into();
    m.write(target.path()).unwrap();

    let outcome = remove_with(target.path(), &clock(), &sites())
        .unwrap()
        .outcome;

    assert_eq!(outcome.word(), "partial", "{outcome:?}");
    assert_eq!(
        std::fs::read_to_string(target.path().join("NOTES.md")).unwrap(),
        text
    );
}

#[test]
fn an_interrupted_removal_is_finished_by_repeating_it() {
    let target = tempfile::tempdir().unwrap();
    let user = "# Mine\n";
    manifest_with(target.path(), Some(user), &format!("{LINE}\n\n{user}"));
    // The file was rewritten, the manifest was not.
    std::fs::write(target.path().join("AGENTS.md"), user).unwrap();

    let removal = remove_with(target.path(), &clock(), &sites()).unwrap();

    assert_eq!(removal.outcome.word(), "applied", "{removal:?}");
    assert_eq!(agents(target.path()).as_deref(), Some(user));
    assert!(
        Manifest::read(target.path())
            .unwrap()
            .unwrap()
            .modifications
            .is_empty()
    );
}

#[test]
fn an_interrupted_removal_of_a_created_bridge_is_finished_too() {
    for leftover in [None, Some("")] {
        let target = tempfile::tempdir().unwrap();
        manifest_with(target.path(), None, &format!("{LINE}\n"));
        if let Some(text) = leftover {
            std::fs::write(target.path().join("AGENTS.md"), text).unwrap();
        }

        let outcome = remove_with(target.path(), &clock(), &sites())
            .unwrap()
            .outcome;

        assert_eq!(outcome.word(), "applied", "{leftover:?}: {outcome:?}");
        assert_eq!(agents(target.path()).as_deref(), leftover);
    }
}

#[test]
fn no_separator_is_taken_when_the_insertion_added_none() {
    let target = tempfile::tempdir().unwrap();
    // The file was empty, so the insertion wrote the line alone; the blank
    // lines after it are the user's.
    manifest_with(target.path(), Some(""), &format!("{LINE}\n"));
    std::fs::write(
        target.path().join("AGENTS.md"),
        format!("{LINE}\n\n\nfoo\n"),
    )
    .unwrap();

    remove_with(target.path(), &clock(), &sites()).unwrap();

    assert_eq!(agents(target.path()).as_deref(), Some("\n\nfoo\n"));
}

#[test]
fn the_separator_the_insertion_added_is_taken_and_only_it() {
    let target = tempfile::tempdir().unwrap();
    manifest_with(target.path(), Some("foo\n"), &format!("{LINE}\n\nfoo\n"));
    std::fs::write(
        target.path().join("AGENTS.md"),
        format!("{LINE}\n\n\nfoo\n"),
    )
    .unwrap();

    remove_with(target.path(), &clock(), &sites()).unwrap();

    assert_eq!(agents(target.path()).as_deref(), Some("\nfoo\n"));
}

#[cfg(unix)]
#[test]
fn taking_the_line_back_keeps_the_files_permissions() {
    use std::os::unix::fs::PermissionsExt as _;
    let target = tempfile::tempdir().unwrap();
    let bridged = format!("{LINE}\n\nx\n");
    std::fs::write(target.path().join("AGENTS.md"), &bridged).unwrap();
    std::fs::set_permissions(
        target.path().join("AGENTS.md"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    manifest_with(target.path(), Some("x\n"), &bridged);

    remove_with(target.path(), &clock(), &sites()).unwrap();

    let mode = std::fs::metadata(target.path().join("AGENTS.md"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    assert_eq!(agents(target.path()).as_deref(), Some("x\n"));
}
