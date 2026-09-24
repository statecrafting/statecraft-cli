//! Spec 002 section 3.4: "Replacing a drifted managed file requires the
//! operator to say so per path." The library half, on every platform. The
//! binary half, through `env plan` and `env apply`, is
//! `crates/statecraft-cli/tests/env_replace.rs`.
//!
//! Every test works in its own temporary directory with a static probe, so
//! nothing here reads a real harness or a real home.

use statecraft_environment::adapter::{Declaration, ManagedFile, StaticProbe};
use statecraft_environment::apply::{Outcome, apply, apply_consented};
use statecraft_environment::claimant::{Claimant, ForeignClaims};
use statecraft_environment::digest::digest_bytes;
use statecraft_environment::manifest::{Class, Manifest, Pins};
use statecraft_environment::plan::{Refusal, plan_naming};
use statecraft_environment::replace::{Consent, Named, STAGING_DIR, plan_id};
use statecraft_environment::time::FixedClock;
use std::collections::BTreeMap;
use std::path::Path;

const HARNESS: &str = "test-harness";

fn pins() -> Pins {
    Pins {
        product: "0.0.0".into(),
        spec_spine: "0.23.0".into(),
        adapters: BTreeMap::new(),
        producer: None,
    }
}

fn declarations(version: &[u8]) -> Vec<Declaration> {
    vec![Declaration {
        name: "a".into(),
        harness: HARNESS.into(),
        version: "1".into(),
        files: vec![
            ManagedFile::owned("one.md", version.to_vec()),
            ManagedFile::owned("two.md", version.to_vec()),
            ManagedFile::pointer("POINTER.md", b"see one.md".to_vec()),
        ],
        unexpressible: vec![],
        prerequisites: vec![],
    }]
}

fn present() -> StaticProbe {
    StaticProbe::new().with_harness(HARNESS)
}

fn clock() -> FixedClock {
    FixedClock(1_700_000_000)
}

/// A target with every managed file installed at v1, then both owned files
/// edited by the operator. Returns the manifest as installed.
fn drifted(target: &Path) -> Manifest {
    let mut manifest = Manifest::new(pins());
    let first = apply(
        target,
        &mut manifest,
        &declarations(b"v1"),
        &present(),
        &ForeignClaims::none(),
        &clock(),
    )
    .unwrap();
    assert!(!first.refused(), "{first:?}");
    std::fs::write(target.join("one.md"), b"edited one").unwrap();
    std::fs::write(target.join("two.md"), b"edited two").unwrap();
    std::fs::write(target.join("USER.md"), b"the user's own").unwrap();
    manifest
}

fn planned(target: &Path, manifest: &Manifest, named: &[&str]) -> Vec<Named> {
    let named: Vec<String> = named.iter().map(|s| s.to_string()).collect();
    plan_naming(
        target,
        Some(manifest),
        &declarations(b"v2"),
        &present(),
        &ForeignClaims::none(),
        &named,
    )
    .unwrap()
    .named
}

fn id_of(named: &[Named], path: &str) -> String {
    named
        .iter()
        .find_map(|n| match n {
            Named::Replace(r) if r.path == path => Some(r.plan_id.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{path} is replaceable in {named:?}"))
}

fn consent(path: &str, plan_id: &str) -> Consent {
    Consent {
        path: path.into(),
        plan_id: plan_id.into(),
    }
}

fn run(target: &Path, manifest: &mut Manifest, consents: &[Consent]) -> (Outcome, Vec<Named>) {
    let c = apply_consented(
        target,
        manifest,
        &declarations(b"v2"),
        &present(),
        &ForeignClaims::none(),
        &clock(),
        consents,
    )
    .unwrap();
    (c.outcome, c.named)
}

fn snapshot(target: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for entry in walk(target) {
        let rel = entry
            .strip_prefix(target)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        out.insert(rel, std::fs::read(&entry).unwrap());
    }
    out
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out
}

#[test]
fn the_plan_names_the_drift_it_saw_and_binds_both_digests() {
    let target = tempfile::tempdir().unwrap();
    let manifest = drifted(target.path());
    let named = planned(target.path(), &manifest, &["one.md"]);
    match &named[..] {
        [Named::Replace(r)] => {
            assert_eq!(r.recorded, digest_bytes(b"v1"));
            assert_eq!(r.found, digest_bytes(b"edited one"));
            assert_eq!(r.replacement, digest_bytes(b"v2"));
            assert_eq!(
                r.plan_id,
                plan_id("one.md", "a", &r.recorded, &r.found, &r.replacement)
            );
        }
        other => panic!("expected one replacement, got {other:?}"),
    }
}

#[test]
fn only_the_named_path_is_replaced_and_every_other_byte_is_kept() {
    let target = tempfile::tempdir().unwrap();
    let mut manifest = drifted(target.path());
    let id = id_of(&planned(target.path(), &manifest, &["one.md"]), "one.md");

    let (outcome, _) = run(target.path(), &mut manifest, &[consent("one.md", &id)]);

    match outcome {
        Outcome::Partial { written, withheld } => {
            assert!(written.contains(&"one.md".to_string()));
            assert_eq!(withheld.len(), 1);
            assert_eq!(withheld[0].path, "two.md", "the unnamed drift is withheld");
        }
        other => panic!("expected partial, got {other:?}"),
    }
    assert_eq!(std::fs::read(target.path().join("one.md")).unwrap(), b"v2");
    assert_eq!(
        std::fs::read(target.path().join("two.md")).unwrap(),
        b"edited two",
        "nothing is replaced wholesale or implicitly"
    );
    assert_eq!(
        std::fs::read(target.path().join("USER.md")).unwrap(),
        b"the user's own"
    );
    let entry = manifest.entry("one.md").unwrap();
    assert_eq!(entry.digest, digest_bytes(b"v2"));
    assert_eq!(entry.class, Class::Managed);
    assert!(
        !target
            .path()
            .join(STAGING_DIR)
            .read_dir()
            .unwrap()
            .any(|_| true),
        "nothing staged is left behind"
    );
}

#[test]
fn a_stale_plan_refuses_and_writes_nothing() {
    let target = tempfile::tempdir().unwrap();
    let mut manifest = drifted(target.path());
    let id = id_of(&planned(target.path(), &manifest, &["one.md"]), "one.md");
    std::fs::write(target.path().join("one.md"), b"edited again").unwrap();
    let before = snapshot(target.path());
    let recorded = manifest.clone();

    let (outcome, _) = run(target.path(), &mut manifest, &[consent("one.md", &id)]);

    match outcome {
        Outcome::Refused { reasons } => assert!(reasons[0].contains("stale plan"), "{reasons:?}"),
        other => panic!("expected refused, got {other:?}"),
    }
    assert_eq!(snapshot(target.path()), before, "not one byte written");
    assert_eq!(manifest, recorded);
}

#[test]
fn an_identity_planned_for_another_drift_is_refused() {
    let target = tempfile::tempdir().unwrap();
    let mut manifest = drifted(target.path());
    let named = planned(target.path(), &manifest, &["one.md", "two.md"]);
    let before = snapshot(target.path());

    let (outcome, _) = run(
        target.path(),
        &mut manifest,
        &[consent("one.md", &id_of(&named, "two.md"))],
    );

    assert!(outcome.refused(), "{outcome:?}");
    assert_eq!(snapshot(target.path()), before);
}

#[test]
fn a_changed_replacement_makes_the_plan_stale() {
    let target = tempfile::tempdir().unwrap();
    let mut manifest = drifted(target.path());
    let id = id_of(&planned(target.path(), &manifest, &["one.md"]), "one.md");
    let before = snapshot(target.path());

    let outcome = apply_consented(
        target.path(),
        &mut manifest,
        &declarations(b"v3"),
        &present(),
        &ForeignClaims::none(),
        &clock(),
        &[consent("one.md", &id)],
    )
    .unwrap()
    .outcome;

    assert!(outcome.refused(), "{outcome:?}");
    assert_eq!(snapshot(target.path()), before);
}

#[test]
fn adopted_foreign_user_and_undeclared_paths_are_never_replaced() {
    let target = tempfile::tempdir().unwrap();
    // The pointer path already holds a user file, so it is withheld as occupied.
    std::fs::write(target.path().join("POINTER.md"), b"user pointer").unwrap();
    let mut manifest = drifted(target.path());
    // `two.md` becomes adopted: depended on, never rewritten.
    let mut adopted = manifest.entry("two.md").unwrap().clone();
    adopted.class = Class::Adopted;
    manifest.upsert(adopted);
    let before = snapshot(target.path());

    for path in [
        "two.md",
        "POINTER.md",
        "USER.md",
        "missing.md",
        ".statecraft/environment.json",
        "../escape.md",
        "/etc/passwd",
    ] {
        let named = plan_naming(
            target.path(),
            Some(&manifest),
            &declarations(b"v2"),
            &present(),
            &ForeignClaims::none(),
            &[path.to_string()],
        )
        .unwrap();
        assert!(
            named.refusals.iter().any(|r| matches!(
                r,
                Refusal::Replacement { path: p, .. } if p == path
            )),
            "{path}: {:?}",
            named.refusals
        );
        let (outcome, _) = run(
            target.path(),
            &mut manifest,
            &[consent(path, &"0".repeat(64))],
        );
        assert!(outcome.refused(), "{path}: {outcome:?}");
    }
    assert_eq!(snapshot(target.path()), before);
}

#[test]
fn a_path_another_installer_claims_is_never_replaced() {
    let target = tempfile::tempdir().unwrap();
    let manifest = drifted(target.path());
    let foreign = ForeignClaims::none().claiming(
        "one.md",
        Claimant::Path {
            path: "one.md".into(),
        },
    );
    let named = plan_naming(
        target.path(),
        Some(&manifest),
        &declarations(b"v2"),
        &present(),
        &foreign,
        &["one.md".to_string()],
    )
    .unwrap();
    assert!(named.refused(), "{:?}", named.refusals);
}

#[test]
fn repeating_a_successful_replacement_is_already_satisfied_and_writes_nothing_new() {
    let target = tempfile::tempdir().unwrap();
    let mut manifest = drifted(target.path());
    let id = id_of(&planned(target.path(), &manifest, &["one.md"]), "one.md");
    run(target.path(), &mut manifest, &[consent("one.md", &id)]);
    let digest = manifest.entry("one.md").unwrap().digest.clone();

    let (outcome, named) = run(target.path(), &mut manifest, &[consent("one.md", &id)]);

    assert!(!outcome.refused(), "{outcome:?}");
    assert!(
        matches!(&named[..], [Named::AlreadySatisfied { records: false, .. }]),
        "{named:?}"
    );
    assert_eq!(std::fs::read(target.path().join("one.md")).unwrap(), b"v2");
    assert_eq!(manifest.entry("one.md").unwrap().digest, digest);
}

#[test]
fn a_failed_staging_writes_nothing_and_the_same_request_then_succeeds() {
    let target = tempfile::tempdir().unwrap();
    let mut manifest = drifted(target.path());
    let id = id_of(&planned(target.path(), &manifest, &["one.md"]), "one.md");
    // Block the staging directory with a file, so staging fails.
    std::fs::create_dir_all(target.path().join(".statecraft/state")).unwrap();
    std::fs::write(target.path().join(STAGING_DIR), b"in the way").unwrap();
    let recorded = manifest.clone();

    let failed = apply_consented(
        target.path(),
        &mut manifest,
        &declarations(b"v2"),
        &present(),
        &ForeignClaims::none(),
        &clock(),
        &[consent("one.md", &id)],
    );
    assert!(failed.is_err(), "an i/o failure is a failure");
    assert_eq!(
        std::fs::read(target.path().join("one.md")).unwrap(),
        b"edited one",
        "the old bytes, whole"
    );
    assert_eq!(manifest, recorded);

    std::fs::remove_file(target.path().join(STAGING_DIR)).unwrap();
    let (outcome, _) = run(target.path(), &mut manifest, &[consent("one.md", &id)]);
    assert!(!outcome.refused(), "{outcome:?}");
    assert_eq!(std::fs::read(target.path().join("one.md")).unwrap(), b"v2");
}

#[test]
fn a_rename_that_landed_before_the_manifest_did_is_recovered_by_repeating_the_request() {
    let target = tempfile::tempdir().unwrap();
    let mut manifest = drifted(target.path());
    let id = id_of(&planned(target.path(), &manifest, &["one.md"]), "one.md");
    // The torn state an interruption between the rename and the manifest write
    // leaves: the file holds the replacement, the manifest still says v1.
    std::fs::write(target.path().join("one.md"), b"v2").unwrap();

    let (outcome, named) = run(target.path(), &mut manifest, &[consent("one.md", &id)]);

    assert!(!outcome.refused(), "{outcome:?}");
    assert!(
        matches!(&named[..], [Named::AlreadySatisfied { records: true, .. }]),
        "{named:?}"
    );
    assert_eq!(
        manifest.entry("one.md").unwrap().digest,
        digest_bytes(b"v2")
    );
}

#[cfg(unix)]
#[test]
fn a_managed_path_that_became_a_symbolic_link_is_not_replaced() {
    let target = tempfile::tempdir().unwrap();
    let manifest = drifted(target.path());
    std::fs::write(target.path().join("elsewhere.md"), b"linked").unwrap();
    std::fs::remove_file(target.path().join("one.md")).unwrap();
    std::os::unix::fs::symlink("elsewhere.md", target.path().join("one.md")).unwrap();

    let named = plan_naming(
        target.path(),
        Some(&manifest),
        &declarations(b"v2"),
        &present(),
        &ForeignClaims::none(),
        &["one.md".to_string()],
    )
    .unwrap();

    assert!(named.refused(), "{:?}", named.refusals);
    assert_eq!(
        std::fs::read(target.path().join("elsewhere.md")).unwrap(),
        b"linked"
    );
}

#[test]
fn a_path_that_is_not_drifted_is_refused_rather_than_consented_to() {
    let target = tempfile::tempdir().unwrap();
    let manifest = drifted(target.path());
    std::fs::write(target.path().join("one.md"), b"v1").unwrap();
    let named = plan_naming(
        target.path(),
        Some(&manifest),
        &declarations(b"v2"),
        &present(),
        &ForeignClaims::none(),
        &["one.md".to_string()],
    )
    .unwrap();
    assert!(named.refused(), "{:?}", named.refusals);
}

#[test]
fn leftovers_an_interrupted_staging_left_are_swept_and_reported() {
    let target = tempfile::tempdir().unwrap();
    let mut manifest = drifted(target.path());
    let id = id_of(&planned(target.path(), &manifest, &["one.md"]), "one.md");
    std::fs::create_dir_all(target.path().join(STAGING_DIR)).unwrap();
    std::fs::write(target.path().join(STAGING_DIR).join("99-0.tmp"), b"half").unwrap();

    let c = apply_consented(
        target.path(),
        &mut manifest,
        &declarations(b"v2"),
        &present(),
        &ForeignClaims::none(),
        &clock(),
        &[consent("one.md", &id)],
    )
    .unwrap();

    assert_eq!(c.swept, [format!("{STAGING_DIR}/99-0.tmp")]);
    assert!(!target.path().join(STAGING_DIR).join("99-0.tmp").exists());
    assert_eq!(std::fs::read(target.path().join("one.md")).unwrap(), b"v2");
}

#[cfg(unix)]
#[test]
fn a_replacement_keeps_the_files_permissions() {
    use std::os::unix::fs::PermissionsExt as _;
    let target = tempfile::tempdir().unwrap();
    let mut manifest = drifted(target.path());
    std::fs::set_permissions(
        target.path().join("one.md"),
        std::fs::Permissions::from_mode(0o640),
    )
    .unwrap();
    let id = id_of(&planned(target.path(), &manifest, &["one.md"]), "one.md");

    run(target.path(), &mut manifest, &[consent("one.md", &id)]);

    let mode = std::fs::metadata(target.path().join("one.md"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o640);
    assert_eq!(std::fs::read(target.path().join("one.md")).unwrap(), b"v2");
}

#[cfg(unix)]
#[test]
fn a_linked_staging_directory_is_refused_and_nothing_is_written() {
    let target = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let mut manifest = drifted(target.path());
    let id = id_of(&planned(target.path(), &manifest, &["one.md"]), "one.md");
    // The fixture's manifest write took the manifest lock, which lives in the
    // state directory; replace that directory with the link.
    let state = target.path().join(".statecraft/state");
    if state.exists() {
        std::fs::remove_dir_all(&state).unwrap();
    }
    std::os::unix::fs::symlink(elsewhere.path(), &state).unwrap();

    let failed = apply_consented(
        target.path(),
        &mut manifest,
        &declarations(b"v2"),
        &present(),
        &ForeignClaims::none(),
        &clock(),
        &[consent("one.md", &id)],
    );

    assert!(failed.is_err());
    assert_eq!(
        std::fs::read(target.path().join("one.md")).unwrap(),
        b"edited one"
    );
    assert!(
        std::fs::read_dir(elsewhere.path())
            .unwrap()
            .next()
            .is_none()
    );
}
