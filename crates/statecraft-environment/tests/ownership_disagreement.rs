//! Spec 026: recorded, journaled and rendered ownership, compared.
//!
//! Each row of section 3.2's table against a fixture repository, the
//! exclusions the section names (`read-only`, the `foreign` overlap, an
//! absent file), section 3.3's unavailable rendering, and section 3.4's
//! order and read-only property. The binary half is `statecraft-cli`'s
//! `tests/doctor_ownership.rs`.

#![cfg(unix)]

use statecraft_environment::doctor::{Finding, Report};
use statecraft_environment::manifest::{Class, Entry, Manifest, Pins, Source, SourceKind};
use statecraft_environment::ownership::{
    Pair, Rendered, Rendering, RenderingFact, disagreements, report,
};
use statecraft_environment::transfer::{OPERATOR_PROVENANCE, Ownership, TransferRecord};
use std::collections::BTreeSet;
use std::path::Path;

const ADAPTER: &str = "adapter test-adapter 1.0.0";

fn manifest() -> Manifest {
    Manifest::new(Pins {
        product: "0.0.0".into(),
        spec_spine: "0.28.0".into(),
        adapters: Default::default(),
        producer: None,
    })
}

fn entry(path: &str, class: Class, kind: SourceKind, identity: &str) -> Entry {
    Entry {
        path: path.into(),
        class,
        source: Source {
            kind,
            identity: identity.into(),
        },
        digest: "0".repeat(64),
        bytes: 0,
        written_at: "2026-10-08T00:00:00Z".into(),
        transfer: None,
        role: Default::default(),
    }
}

fn record(id: &str, path: &str, from: Ownership, to: Ownership) -> TransferRecord {
    TransferRecord {
        id: id.into(),
        path: path.into(),
        from,
        to,
        digest: "0".repeat(64),
        bytes: 0,
        producer: "spec-spine-core@0.28.0".into(),
        operator: "fixture".into(),
        operator_provenance: OPERATOR_PROVENANCE.into(),
        reason: "fixture".into(),
        at: "2026-10-08T00:00:00Z".into(),
        manifest_before: "0".repeat(64),
        reverts: None,
    }
}

fn rendering(paths: &[(&str, Rendered)]) -> Rendering {
    let mut r = Rendering::new();
    for (path, class) in paths {
        r.name(path, *class, ADAPTER);
    }
    r
}

fn repo(files: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for f in files {
        let at = dir.path().join(f);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, b"bytes").unwrap();
    }
    dir
}

fn pairs(root: &Path, m: &Manifest, r: Option<&Rendering>) -> Vec<(String, Pair)> {
    disagreements(root, m, r, &BTreeSet::new())
        .into_iter()
        .map(|d| (d.path, d.pair))
        .collect()
}

#[test]
fn row_1_a_journal_target_that_differs_from_the_recorded_class_names_both_sources() {
    let dir = repo(&["a.md"]);
    let mut m = manifest();
    // Transferred to `user`, then recorded `adopted` again: the SC-004 shape.
    m.upsert(entry(
        "a.md",
        Class::Adopted,
        SourceKind::Template,
        "statecraft-governance",
    ));
    m.transfers
        .push(record("t1", "a.md", Ownership::Adopted, Ownership::User));
    let found = disagreements(dir.path(), &m, None, &BTreeSet::new());
    assert_eq!(found.len(), 1, "{found:?}");
    let d = &found[0];
    assert_eq!(d.pair, Pair::RecordedJournaled);
    assert_eq!(
        (d.left.name, d.left.value.as_str()),
        ("recorded", "adopted")
    );
    assert!(d.left.source.contains("statecraft-governance"), "{d:?}");
    assert_eq!(
        (d.right.name, d.right.value.as_str()),
        ("journaled", "user")
    );
    assert_eq!(d.right.source, "journal record t1");
    assert!(
        d.describe()
            .starts_with("ownership-disagreement a.md recorded/journaled")
    );
    assert!(d.describe().contains("transfer plan"), "{}", d.describe());
}

#[test]
fn row_1_a_reversal_counts_as_its_inverse_move() {
    let dir = repo(&["a.md"]);
    let mut m = manifest();
    m.transfers
        .push(record("t1", "a.md", Ownership::User, Ownership::Adopted));
    let mut reversal = record("t2", "a.md", Ownership::Adopted, Ownership::User);
    reversal.reverts = Some("t1".into());
    m.transfers.push(reversal);
    // The reversal left it `user`, and nothing records it: agreement.
    assert!(pairs(dir.path(), &m, None).is_empty());
    // Recorded `adopted` against the reversal's `user`: disagreement.
    m.upsert(entry("a.md", Class::Adopted, SourceKind::Adapter, "x"));
    assert_eq!(
        pairs(dir.path(), &m, None),
        [("a.md".to_string(), Pair::RecordedJournaled)]
    );
}

#[test]
fn row_2_a_released_path_the_rendering_would_write() {
    let dir = repo(&[]);
    let mut m = manifest();
    m.transfers
        .push(record("t1", "a.md", Ownership::Managed, Ownership::User));
    let r = rendering(&[("a.md", Rendered::Managed)]);
    let found = disagreements(dir.path(), &m, Some(&r), &BTreeSet::new());
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].pair, Pair::JournaledRendered);
    assert_eq!(found[0].left.value, "user");
    assert_eq!(found[0].right.value, "managed");
    assert_eq!(found[0].right.source, ADAPTER);
}

#[test]
fn row_3_recorded_adopted_and_rendered_managed() {
    let dir = repo(&["a.md"]);
    let mut m = manifest();
    m.upsert(entry(
        "a.md",
        Class::Adopted,
        SourceKind::Adapter,
        "test-adapter",
    ));
    let r = rendering(&[("a.md", Rendered::Managed)]);
    assert_eq!(
        pairs(dir.path(), &m, Some(&r)),
        [("a.md".to_string(), Pair::RecordedRendered)]
    );
    // Read-only or absent cannot rewrite an adopted path.
    let r = rendering(&[("a.md", Rendered::ReadOnly)]);
    assert!(pairs(dir.path(), &m, Some(&r)).is_empty());
    assert!(pairs(dir.path(), &m, Some(&Rendering::new())).is_empty());
}

#[test]
fn row_4_recorded_managed_and_not_rendered() {
    let dir = repo(&["a.md"]);
    let mut m = manifest();
    m.upsert(entry("a.md", Class::Managed, SourceKind::Adapter, "gone"));
    let found = disagreements(dir.path(), &m, Some(&Rendering::new()), &BTreeSet::new());
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].pair, Pair::RecordedRendered);
    assert_eq!(found[0].right.value, "absent");
    // A read-only rendering names it, so it is not absent.
    let r = rendering(&[("a.md", Rendered::ReadOnly)]);
    assert!(pairs(dir.path(), &m, Some(&r)).is_empty());
    let r = rendering(&[("a.md", Rendered::Managed)]);
    assert!(pairs(dir.path(), &m, Some(&r)).is_empty());
}

#[test]
fn row_5_an_existing_unrecorded_path_the_rendering_would_write() {
    let r = rendering(&[("a.md", Rendered::Managed)]);
    let m = manifest();
    let present = repo(&["a.md"]);
    assert_eq!(
        pairs(present.path(), &m, Some(&r)),
        [("a.md".to_string(), Pair::RecordedRendered)]
    );
    // No file: an ordinary first write.
    let absent = repo(&[]);
    assert!(pairs(absent.path(), &m, Some(&r)).is_empty());
    // A path `doctor` reports as `foreign` keeps only that finding.
    let foreign = BTreeSet::from(["a.md".to_string()]);
    assert!(disagreements(present.path(), &m, Some(&r), &foreign).is_empty());
}

#[test]
fn a_read_only_rendering_never_disagrees_with_any_class() {
    let dir = repo(&["a.md", "b.md", "c.md"]);
    let mut m = manifest();
    m.upsert(entry("a.md", Class::Managed, SourceKind::Adapter, "x"));
    m.upsert(entry("b.md", Class::Adopted, SourceKind::Adapter, "x"));
    m.transfers
        .push(record("t1", "c.md", Ownership::Managed, Ownership::User));
    let r = rendering(&[
        ("a.md", Rendered::ReadOnly),
        ("b.md", Rendered::ReadOnly),
        ("c.md", Rendered::ReadOnly),
    ]);
    assert!(pairs(dir.path(), &m, Some(&r)).is_empty());
}

#[test]
fn a_record_this_products_own_operation_followed_is_not_a_disagreement() {
    let dir = repo(&["a.md"]);
    // A move to `managed` whose entry `env remove` then removed.
    let mut m = manifest();
    m.transfers
        .push(record("t1", "a.md", Ownership::User, Ownership::Managed));
    assert!(pairs(dir.path(), &m, None).is_empty());
    // A move to `user` whose file `env apply` then wrote afresh.
    let mut m = manifest();
    m.transfers
        .push(record("t1", "a.md", Ownership::Managed, Ownership::User));
    m.upsert(entry(
        "a.md",
        Class::Managed,
        SourceKind::Adapter,
        "test-adapter",
    ));
    let r = rendering(&[("a.md", Rendered::Managed)]);
    assert!(pairs(dir.path(), &m, Some(&r)).is_empty());
}

#[test]
fn one_path_may_carry_more_than_one_pair_in_the_tables_order() {
    let dir = repo(&["a.md", "b.md"]);
    let mut m = manifest();
    // Recorded adopted, journaled user, rendered managed: all three pairs.
    m.upsert(entry("b.md", Class::Adopted, SourceKind::Adapter, "x"));
    m.transfers
        .push(record("t1", "b.md", Ownership::Adopted, Ownership::User));
    m.upsert(entry("a.md", Class::Managed, SourceKind::Adapter, "x"));
    let r = rendering(&[("b.md", Rendered::Managed)]);
    assert_eq!(
        pairs(dir.path(), &m, Some(&r)),
        [
            ("a.md".to_string(), Pair::RecordedRendered),
            ("b.md".to_string(), Pair::RecordedJournaled),
            ("b.md".to_string(), Pair::JournaledRendered),
            ("b.md".to_string(), Pair::RecordedRendered),
        ]
    );
}

#[test]
fn an_unavailable_rendering_is_reported_once_and_never_read_as_agreement() {
    let dir = repo(&["a.md", "b.md"]);
    let mut m = manifest();
    m.upsert(entry("a.md", Class::Managed, SourceKind::Adapter, "x"));
    m.upsert(entry("b.md", Class::Adopted, SourceKind::Adapter, "x"));
    m.transfers
        .push(record("t1", "b.md", Ownership::Adopted, Ownership::User));
    let mut out = Report::default();
    report(
        dir.path(),
        &m,
        &RenderingFact::Unavailable("the producer did not answer".into()),
        &mut out,
    );
    let notes: Vec<_> = out
        .notes
        .iter()
        .filter(|n| n.starts_with("ownership-rendering-unavailable"))
        .collect();
    assert_eq!(notes.len(), 1, "{:?}", out.notes);
    assert!(notes[0].contains("the producer did not answer"));
    // Recorded and journaled are still compared; nothing needing a rendering
    // is reported as agreement or as a disagreement.
    let found: Vec<_> = out
        .findings
        .iter()
        .map(|f| match f {
            Finding::OwnershipDisagreement(d) => (d.path.as_str(), d.pair),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(found, [("b.md", Pair::RecordedJournaled)]);
    assert_eq!(out.exit_code(), 1);
}

#[test]
fn an_entry_transferred_before_the_journal_existed_is_not_a_disagreement_for_that_alone() {
    let dir = repo(&["a.md"]);
    let mut m = manifest();
    let mut e = entry("a.md", Class::Managed, SourceKind::Adapter, "test-adapter");
    e.transfer = Some(statecraft_environment::manifest::Transfer {
        from: statecraft_environment::claimant::Claimant::User {
            path: "a.md".into(),
        },
        digest_at_transfer: "0".repeat(64),
        evaluated_against: None,
    });
    m.upsert(e);
    let r = rendering(&[("a.md", Rendered::Managed)]);
    assert!(pairs(dir.path(), &m, Some(&r)).is_empty());
}

#[test]
fn the_comparison_writes_nothing_and_answers_the_same_twice() {
    let dir = repo(&["a.md", "b.md"]);
    let mut m = manifest();
    m.upsert(entry("a.md", Class::Managed, SourceKind::Adapter, "x"));
    m.transfers
        .push(record("t1", "b.md", Ownership::Managed, Ownership::User));
    let r = rendering(&[("b.md", Rendered::Managed)]);
    let snapshot = |p: &Path| {
        let mut files: Vec<_> = std::fs::read_dir(p)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (e.file_name(), std::fs::read(e.path()).unwrap())
            })
            .collect();
        files.sort();
        files
    };
    let before = snapshot(dir.path());
    let first = disagreements(dir.path(), &m, Some(&r), &BTreeSet::new());
    let second = disagreements(dir.path(), &m, Some(&r), &BTreeSet::new());
    assert_eq!(first, second);
    assert!(!first.is_empty());
    assert_eq!(snapshot(dir.path()), before);
}
