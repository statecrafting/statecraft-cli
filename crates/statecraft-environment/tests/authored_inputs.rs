//! Spec 002 section 5, the 2026-09-24 provenance entry, item 2: an authored
//! input is written only when absent, never rewritten, reported `seeded` or
//! `customized` as information, and removed only while it is its seed.

use statecraft_environment::adapter::{Declaration, ManagedFile, StaticProbe};
use statecraft_environment::apply::{Outcome, apply, remove};
use statecraft_environment::claimant::{ForeignClaims, UnobservedShadows};
use statecraft_environment::digest::digest_bytes;
use statecraft_environment::doctor::{Observed, State, doctor};
use statecraft_environment::manifest::{Manifest, Pins, Role};
use statecraft_environment::plan::plan;
use statecraft_environment::time::FixedClock;
use std::collections::BTreeMap;

const HARNESS: &str = "test-harness";

fn declaration(seed: &str) -> Declaration {
    Declaration {
        name: "governance".into(),
        harness: HARNESS.into(),
        version: "1".into(),
        files: vec![
            ManagedFile::authored_input("spec-spine.toml", seed.as_bytes().to_vec()),
            ManagedFile::owned("templates/t.md", b"template".to_vec()),
        ],
        unexpressible: vec![],
        prerequisites: vec![],
    }
}

fn manifest() -> Manifest {
    Manifest::new(Pins {
        product: "0.0.0".into(),
        spec_spine: "unpinned".into(),
        adapters: BTreeMap::new(),
        producer: None,
    })
}

fn state(root: &std::path::Path, m: &Manifest, path: &str) -> State {
    let report = doctor(
        root,
        m,
        &[],
        &StaticProbe::new().with_harness(HARNESS),
        &ForeignClaims::none(),
        &UnobservedShadows,
        &Observed::default(),
    )
    .unwrap();
    assert!(
        !report.findings.iter().any(|f| f.describe().contains(path)),
        "{:?}",
        report.findings
    );
    report
        .entries
        .into_iter()
        .find(|e| e.path == path)
        .unwrap()
        .state
}

#[test]
fn an_authored_input_is_seeded_then_customized_never_rewritten_and_removed_only_as_its_seed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let probe = StaticProbe::new().with_harness(HARNESS);
    let clock = FixedClock(1_700_000_000);
    let mut m = manifest();

    let outcome = apply(
        root,
        &mut m,
        &[declaration("seed-1")],
        &probe,
        &ForeignClaims::none(),
        &clock,
    )
    .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let entry = m.entry("spec-spine.toml").unwrap();
    assert_eq!(entry.role, Role::AuthoredInput);
    assert_eq!(entry.digest, digest_bytes(b"seed-1"));
    assert_eq!(m.entry("templates/t.md").unwrap().role, Role::Reference);
    assert_eq!(state(root, &m, "spec-spine.toml"), State::Seeded);

    // A newer seed is reported by its digest and not written, even while the
    // file is still the old seed.
    let p = plan(
        root,
        Some(&m),
        &[declaration("seed-2")],
        &probe,
        &ForeignClaims::none(),
    )
    .unwrap();
    assert!(!p.writes.iter().any(|w| w.path == "spec-spine.toml"));
    assert!(p.withheld.is_empty(), "{:?}", p.withheld);
    let kept = &p.kept[0];
    assert_eq!(kept.word(), "seeded");
    assert_eq!(
        kept.newer_seed.as_deref(),
        Some(digest_bytes(b"seed-2").as_str())
    );

    // Edited: customized, information only, and still never rewritten.
    std::fs::write(root.join("spec-spine.toml"), "edited").unwrap();
    assert!(matches!(
        state(root, &m, "spec-spine.toml"),
        State::Customized { .. }
    ));
    let outcome = apply(
        root,
        &mut m,
        &[declaration("seed-2")],
        &probe,
        &ForeignClaims::none(),
        &clock,
    )
    .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert_eq!(
        std::fs::read(root.join("spec-spine.toml")).unwrap(),
        b"edited"
    );

    // Removal keeps and names the customized input, and removes the rest.
    let removed = remove(root, &clock).unwrap();
    assert!(root.join("spec-spine.toml").exists(), "{removed:?}");
    assert!(!root.join("templates/t.md").exists());
}
