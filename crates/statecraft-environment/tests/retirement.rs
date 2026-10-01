//! Spec 030 sections 3.1 and 3.3: no adapter path in a provider directory, and
//! a file an adapter no longer declares leaves with its recorded bytes.
//!
//! Retirement is `env apply`'s and `env upgrade`'s alike, since the two share
//! one library operation. A retired file is removed only while it carries the
//! digest the manifest recorded; one that drifted is withheld and left with its
//! record. Only directories the removal emptied are removed.

use statecraft_environment::adapter::{Declaration, ManagedFile, StaticProbe};
use statecraft_environment::apply::{Outcome, apply};
use statecraft_environment::claimant::ForeignClaims;
use statecraft_environment::manifest::{Class, Manifest, Pins};
use statecraft_environment::plan::{Withholding, plan};
use statecraft_environment::time::FixedClock;
use std::collections::BTreeMap;
use std::path::Path;

const HARNESS: &str = "test-harness";
const OLD: &str = "old/deep/instructions.md";

fn pins() -> Pins {
    Pins {
        product: "0.0.0".into(),
        spec_spine: "0.18.0".into(),
        adapters: BTreeMap::new(),
        producer: None,
    }
}

fn adapter(name: &str, files: Vec<ManagedFile>) -> Declaration {
    Declaration {
        name: name.into(),
        harness: HARNESS.into(),
        version: "1".into(),
        files,
        unexpressible: vec![],
        prerequisites: vec![],
    }
}

fn present() -> StaticProbe {
    StaticProbe::new().with_harness(HARNESS)
}

fn run(root: &Path, manifest: &mut Manifest, declarations: &[Declaration]) -> Outcome {
    apply(
        root,
        manifest,
        declarations,
        &present(),
        &ForeignClaims::none(),
        &FixedClock(1_700_000_000),
    )
    .unwrap()
}

/// Installed under a declaration of `OLD` and `kept.md`, then redeclared with
/// `kept.md` alone.
fn installed() -> (tempfile::TempDir, Manifest, Vec<Declaration>) {
    let target = tempfile::tempdir().unwrap();
    let mut manifest = Manifest::new(pins());
    let before = [adapter(
        "a",
        vec![
            ManagedFile::owned(OLD, b"old".to_vec()),
            ManagedFile::owned("kept.md", b"kept".to_vec()),
        ],
    )];
    assert!(matches!(
        run(target.path(), &mut manifest, &before),
        Outcome::Applied { .. }
    ));
    let after = vec![adapter(
        "a",
        vec![ManagedFile::owned("kept.md", b"kept".to_vec())],
    )];
    (target, manifest, after)
}

#[test]
fn an_undeclared_unchanged_file_is_retired_and_its_emptied_directories_pruned() {
    let (target, mut manifest, after) = installed();

    let planned = plan(
        target.path(),
        Some(&manifest),
        &after,
        &present(),
        &ForeignClaims::none(),
    )
    .unwrap();
    assert_eq!(planned.retired.len(), 1, "{planned:?}");
    assert_eq!(planned.retired[0].path, OLD);
    assert!(planned.render().contains("retire old/deep/instructions.md"));
    assert!(target.path().join(OLD).is_file(), "a plan removes nothing");

    let outcome = run(target.path(), &mut manifest, &after);

    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert!(!target.path().join(OLD).exists());
    assert!(!target.path().join("old").exists(), "emptied, so pruned");
    assert!(manifest.entry(OLD).is_none());
    assert!(manifest.entry("kept.md").is_some());
    assert!(target.path().join("kept.md").is_file());
}

#[test]
fn a_directory_holding_anything_else_is_left() {
    let (target, mut manifest, after) = installed();
    std::fs::write(target.path().join("old/user.md"), b"mine").unwrap();

    run(target.path(), &mut manifest, &after);

    assert!(!target.path().join(OLD).exists());
    assert!(!target.path().join("old/deep").exists());
    assert_eq!(
        std::fs::read(target.path().join("old/user.md")).unwrap(),
        b"mine"
    );
}

#[test]
fn a_drifted_undeclared_file_is_withheld_and_left_with_its_record() {
    let (target, mut manifest, after) = installed();
    std::fs::write(target.path().join(OLD), b"edited").unwrap();

    let outcome = run(target.path(), &mut manifest, &after);

    match outcome {
        Outcome::Partial { withheld, .. } => {
            assert_eq!(withheld.len(), 1);
            assert_eq!(withheld[0].path, OLD);
            assert!(matches!(withheld[0].reason, Withholding::Drifted { .. }));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(std::fs::read(target.path().join(OLD)).unwrap(), b"edited");
    assert!(manifest.entry(OLD).is_some(), "the record stays");
}

#[test]
fn an_adopted_entry_or_an_unconfigured_adapter_retires_nothing() {
    // Adopted: the operator took it, so it is theirs to remove.
    let (target, mut manifest, after) = installed();
    let mut entry = manifest.entry(OLD).unwrap().clone();
    entry.class = Class::Adopted;
    manifest.upsert(entry);
    let planned = plan(
        target.path(),
        Some(&manifest),
        &after,
        &present(),
        &ForeignClaims::none(),
    )
    .unwrap();
    assert!(planned.retired.is_empty(), "{planned:?}");

    // Unconfigured: an adapter absent from the declarations says nothing
    // about what it no longer declares.
    let (target, mut manifest, _) = installed();
    let other = [adapter("b", vec![])];
    let planned = plan(
        target.path(),
        Some(&manifest),
        &other,
        &present(),
        &ForeignClaims::none(),
    )
    .unwrap();
    assert!(planned.retired.is_empty(), "{planned:?}");
    run(target.path(), &mut manifest, &other);
    assert!(target.path().join(OLD).is_file());
}

#[test]
fn an_adapter_that_does_not_claim_retires_nothing() {
    let (target, mut manifest, after) = installed();
    let absent = StaticProbe::new();
    let planned = plan(
        target.path(),
        Some(&manifest),
        &after,
        &absent,
        &ForeignClaims::none(),
    )
    .unwrap();
    assert!(planned.retired.is_empty(), "{planned:?}");
    let outcome = apply(
        target.path(),
        &mut manifest,
        &after,
        &absent,
        &ForeignClaims::none(),
        &FixedClock(1_700_000_000),
    )
    .unwrap();
    assert!(!matches!(outcome, Outcome::Refused { .. }), "{outcome:?}");
    assert!(target.path().join(OLD).is_file());
    assert!(manifest.entry(OLD).is_some());
}

#[test]
fn a_file_already_gone_loses_only_its_record() {
    let (target, mut manifest, after) = installed();
    std::fs::remove_file(target.path().join(OLD)).unwrap();

    let outcome = run(target.path(), &mut manifest, &after);

    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert!(manifest.entry(OLD).is_none());
}

/// Installed with `OLD` and a pointer importing it, then redeclared with the
/// pointer alone, importing something else.
fn installed_with_pointer() -> (tempfile::TempDir, Manifest, Vec<Declaration>) {
    let target = tempfile::tempdir().unwrap();
    let mut manifest = Manifest::new(pins());
    let before = [adapter(
        "a",
        vec![
            ManagedFile::owned(OLD, b"old".to_vec()),
            ManagedFile::pointer("POINTER.md", format!("@{OLD}\n").into_bytes()),
        ],
    )];
    assert!(matches!(
        run(target.path(), &mut manifest, &before),
        Outcome::Applied { .. }
    ));
    let after = vec![adapter(
        "a",
        vec![ManagedFile::pointer("POINTER.md", b"@new.md\n".to_vec())],
    )];
    (target, manifest, after)
}

#[test]
fn a_file_is_retired_in_the_apply_that_rewrites_the_pointer_importing_it() {
    let (target, mut manifest, after) = installed_with_pointer();

    let planned = plan(
        target.path(),
        Some(&manifest),
        &after,
        &present(),
        &ForeignClaims::none(),
    )
    .unwrap();
    assert!(
        planned.writes.iter().any(|w| w.path == "POINTER.md"),
        "{planned:?}"
    );
    assert_eq!(planned.retired.len(), 1, "{planned:?}");

    run(target.path(), &mut manifest, &after);
    assert!(!target.path().join(OLD).exists());
    assert_eq!(
        std::fs::read(target.path().join("POINTER.md")).unwrap(),
        b"@new.md\n"
    );
}

#[test]
fn a_file_a_withheld_pointer_still_imports_is_kept_and_reported() {
    let (target, mut manifest, after) = installed_with_pointer();
    // The operator edited the pointer: it is drifted, so it is not rewritten,
    // and it still imports the old file.
    let edited = format!("@{OLD}\nmy own line\n");
    std::fs::write(target.path().join("POINTER.md"), &edited).unwrap();

    let planned = plan(
        target.path(),
        Some(&manifest),
        &after,
        &present(),
        &ForeignClaims::none(),
    )
    .unwrap();
    assert!(planned.retired.is_empty(), "{planned:?}");
    let held = planned
        .withheld
        .iter()
        .find(|w| w.path == OLD)
        .unwrap_or_else(|| panic!("{planned:?}"));
    assert_eq!(
        held.reason,
        Withholding::StillImported {
            by: "POINTER.md".into()
        }
    );
    assert!(
        held.reason
            .describe()
            .contains("POINTER.md still imports it")
    );

    run(target.path(), &mut manifest, &after);
    assert_eq!(std::fs::read(target.path().join(OLD)).unwrap(), b"old");
    assert!(
        manifest.entry(OLD).is_some(),
        "the record stays with the file"
    );
    assert_eq!(
        std::fs::read_to_string(target.path().join("POINTER.md")).unwrap(),
        edited
    );
}
