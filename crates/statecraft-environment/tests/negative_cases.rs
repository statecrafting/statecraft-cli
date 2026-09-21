//! Spec 002 section 3.10, the observable negative cases, one test per row.
//!
//! These are the rows a reader can check the implementation against without
//! reading the implementation. Each test names the row it covers in its own
//! name, so a row that stops being covered is visible as a deleted test rather
//! than as a quietly weakened assertion.

use statecraft_environment::adapter::{
    Declaration, ManagedFile, Prerequisite, Readiness, StaticProbe, readiness,
};
use statecraft_environment::apply::{NO_MANIFEST, Outcome, apply, remove};
use statecraft_environment::claimant::{Claimant, ForeignClaims, StaticShadows, UnobservedShadows};
use statecraft_environment::digest::digest_bytes;
use statecraft_environment::doctor::{Finding, Observed, State, doctor};
use statecraft_environment::manifest::{Manifest, Pins};
use statecraft_environment::plan::{Refusal, Withholding, plan};
use statecraft_environment::qualify::{CorpusState, TargetProbe, Verdict, qualify};
use statecraft_environment::registry::Registry;
use statecraft_environment::time::FixedClock;
use std::collections::BTreeMap;
use std::path::Path;

const HARNESS: &str = "test-harness";

fn pins() -> Pins {
    Pins {
        product: "0.0.0".into(),
        spec_spine: "0.18.0".into(),
        adapters: BTreeMap::new(),
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

fn clock() -> FixedClock {
    FixedClock(1_700_000_000)
}

struct Probe {
    git: bool,
    base: bool,
    corpus: CorpusState,
}

impl TargetProbe for Probe {
    fn is_git_work_tree(&self, _: &Path) -> bool {
        self.git
    }
    fn has_base_revision(&self, _: &Path) -> bool {
        self.base
    }
    fn corpus(&self, _: &Path) -> CorpusState {
        self.corpus.clone()
    }
}

// Row 1: `project register` on a path that is not a git work tree.
#[test]
fn register_on_a_non_git_path_is_unqualified_and_writes_nothing_inside_it() {
    let target = tempfile::tempdir().unwrap();
    let probe = Probe {
        git: false,
        base: false,
        corpus: CorpusState::Absent,
    };

    let mut registry = Registry::default();
    let registration = registry.register(target.path(), &probe).unwrap().clone();

    assert_eq!(registration.qualification.verdict, Verdict::Unqualified);
    assert!(!registration.qualification.reasons.is_empty());
    assert_eq!(
        std::fs::read_dir(target.path()).unwrap().count(),
        0,
        "registration must not write inside the target"
    );
}

// Row 2: `project register` on a repository with no corpus.
#[test]
fn register_on_a_repository_with_no_corpus_is_ungoverned_and_visible_but_never_scheduled() {
    let target = tempfile::tempdir().unwrap();
    let probe = Probe {
        git: true,
        base: true,
        corpus: CorpusState::Absent,
    };

    let q = qualify(target.path(), &probe);
    assert_eq!(q.verdict, Verdict::Ungoverned);
    assert!(!q.verdict.schedulable());

    let mut registry = Registry::default();
    registry.register(target.path(), &probe).unwrap();
    registry.set_armed(target.path(), true).unwrap();
    assert_eq!(registry.all().count(), 1, "visible");
    assert_eq!(registry.eligible().count(), 0, "never scheduled");
}

// Row 3: `env apply` where a planned managed path already exists and is not in
// the manifest.
#[test]
fn apply_over_an_unmanifested_existing_file_withholds_it_as_foreign_and_reports_partial() {
    let target = tempfile::tempdir().unwrap();
    std::fs::write(target.path().join("theirs.md"), b"not ours").unwrap();

    let declarations = vec![adapter(
        "a",
        vec![
            ManagedFile::owned("theirs.md", b"ours".to_vec()),
            ManagedFile::owned("fresh.md", b"ours".to_vec()),
        ],
    )];
    let mut manifest = Manifest::new(pins());

    let outcome = apply(
        target.path(),
        &mut manifest,
        &declarations,
        &present(),
        &ForeignClaims::none(),
        &clock(),
    )
    .unwrap();

    match outcome {
        Outcome::Partial { written, withheld } => {
            assert_eq!(written, ["fresh.md"]);
            assert_eq!(withheld.len(), 1);
            assert_eq!(withheld[0].path, "theirs.md");
            assert!(matches!(withheld[0].reason, Withholding::Foreign { .. }));
        }
        other => panic!("expected partial, got {other:?}"),
    }
    assert_eq!(
        std::fs::read(target.path().join("theirs.md")).unwrap(),
        b"not ours",
        "no overwrite"
    );
}

// Row 4: `env upgrade` where a managed file has drifted.
#[test]
fn upgrade_over_a_drifted_managed_file_withholds_it_naming_both_digests() {
    let target = tempfile::tempdir().unwrap();
    let declarations = vec![adapter(
        "a",
        vec![ManagedFile::owned("m.md", b"v1".to_vec())],
    )];
    let mut manifest = Manifest::new(pins());

    let first = apply(
        target.path(),
        &mut manifest,
        &declarations,
        &present(),
        &ForeignClaims::none(),
        &clock(),
    )
    .unwrap();
    assert_eq!(first.word(), "applied");

    // The operator edits the managed file.
    std::fs::write(target.path().join("m.md"), b"hand edited").unwrap();

    let newer = vec![adapter(
        "a",
        vec![ManagedFile::owned("m.md", b"v2".to_vec())],
    )];
    let outcome = apply(
        target.path(),
        &mut manifest,
        &newer,
        &present(),
        &ForeignClaims::none(),
        &clock(),
    )
    .unwrap();

    match outcome {
        Outcome::Partial { written, withheld } => {
            assert!(written.is_empty());
            match &withheld[0].reason {
                Withholding::Drifted { expected, found } => {
                    assert_eq!(expected, &digest_bytes(b"v1"));
                    assert_eq!(found, &digest_bytes(b"hand edited"));
                }
                other => panic!("expected drifted, got {other:?}"),
            }
        }
        other => panic!("expected partial, got {other:?}"),
    }
    assert_eq!(
        std::fs::read(target.path().join("m.md")).unwrap(),
        b"hand edited",
        "an upgrade never resolves a conflict by choosing"
    );
}

// Row 5: `env remove` with no manifest.
#[test]
fn remove_with_no_manifest_refuses_and_deletes_nothing() {
    let target = tempfile::tempdir().unwrap();
    std::fs::write(target.path().join("something.md"), b"user file").unwrap();

    let outcome = remove(target.path(), &clock()).unwrap();

    match outcome {
        Outcome::Refused { reasons } => assert_eq!(reasons, [NO_MANIFEST]),
        other => panic!("expected refusal, got {other:?}"),
    }
    assert!(target.path().join("something.md").exists());
}

// Row 6: `env remove` where a managed file has drifted.
#[test]
fn remove_leaves_a_drifted_managed_file_and_removes_every_matching_one() {
    let target = tempfile::tempdir().unwrap();
    let declarations = vec![adapter(
        "a",
        vec![
            ManagedFile::owned("clean.md", b"c".to_vec()),
            ManagedFile::owned("dirty.md", b"d".to_vec()),
        ],
    )];
    let mut manifest = Manifest::new(pins());
    apply(
        target.path(),
        &mut manifest,
        &declarations,
        &present(),
        &ForeignClaims::none(),
        &clock(),
    )
    .unwrap();

    std::fs::write(target.path().join("dirty.md"), b"changed").unwrap();

    match remove(target.path(), &clock()).unwrap() {
        Outcome::Partial { written, withheld } => {
            assert_eq!(written, ["clean.md"]);
            assert_eq!(withheld[0].path, "dirty.md");
        }
        other => panic!("expected partial, got {other:?}"),
    }
    assert!(!target.path().join("clean.md").exists());
    assert_eq!(
        std::fs::read(target.path().join("dirty.md")).unwrap(),
        b"changed",
        "the operator changed it, so it is theirs now"
    );
}

#[test]
fn remove_never_touches_a_user_path() {
    let target = tempfile::tempdir().unwrap();
    std::fs::write(target.path().join("USER.md"), b"mine").unwrap();
    let declarations = vec![adapter(
        "a",
        vec![ManagedFile::owned("ours.md", b"o".to_vec())],
    )];
    let mut manifest = Manifest::new(pins());
    apply(
        target.path(),
        &mut manifest,
        &declarations,
        &present(),
        &ForeignClaims::none(),
        &clock(),
    )
    .unwrap();

    remove(target.path(), &clock()).unwrap();

    assert!(target.path().join("USER.md").exists());
    assert_eq!(
        std::fs::read(target.path().join("USER.md")).unwrap(),
        b"mine"
    );
}

// Row 7: two adapters declaring the same path.
#[test]
fn two_adapters_declaring_one_path_are_refused_at_plan_time_naming_both() {
    let target = tempfile::tempdir().unwrap();
    let declarations = vec![
        adapter(
            "alpha",
            vec![ManagedFile::owned("shared.md", b"a".to_vec())],
        ),
        adapter("beta", vec![ManagedFile::owned("shared.md", b"b".to_vec())]),
    ];

    let computed = plan(
        target.path(),
        None,
        &declarations,
        &present(),
        &ForeignClaims::none(),
    )
    .unwrap();

    assert!(computed.refused());
    match &computed.refusals[0] {
        Refusal::AdapterPathCollision(c) => {
            assert_eq!(c.path, "shared.md");
            assert_eq!(c.adapters, ("alpha".to_string(), "beta".to_string()));
        }
    }

    let mut manifest = Manifest::new(pins());
    let outcome = apply(
        target.path(),
        &mut manifest,
        &declarations,
        &present(),
        &ForeignClaims::none(),
        &clock(),
    )
    .unwrap();
    assert!(outcome.refused());
    assert!(
        !target.path().join("shared.md").exists(),
        "a refusal writes nothing"
    );
}

// Row 8: an adapter for an absent harness.
#[test]
fn an_adapter_for_an_absent_harness_claims_nothing_and_names_the_absent_prerequisite() {
    let target = tempfile::tempdir().unwrap();
    let declarations = vec![adapter(
        "a",
        vec![ManagedFile::owned("x.md", b"x".to_vec())],
    )];

    let computed = plan(
        target.path(),
        None,
        &declarations,
        &StaticProbe::new(),
        &ForeignClaims::none(),
    )
    .unwrap();

    assert!(computed.writes.is_empty());
    match &computed.adapters[0].readiness {
        Readiness::Refused { missing } => assert_eq!(missing, &["harness-present"]),
        other => panic!("expected refusal, got {other:?}"),
    }
    assert!(!target.path().join("x.md").exists());
}

#[test]
fn an_adapter_with_a_missing_prerequisite_names_that_prerequisite_not_the_harness() {
    let target = tempfile::tempdir().unwrap();
    let mut d = adapter("a", vec![ManagedFile::owned("x.md", b"x".to_vec())]);
    d.prerequisites = vec![Prerequisite::new(
        "loads-pointer",
        "the harness loads a pointer file at the declared path",
    )];

    let computed = plan(
        target.path(),
        None,
        &[d],
        &present(),
        &ForeignClaims::none(),
    )
    .unwrap();

    match &computed.adapters[0].readiness {
        Readiness::Refused { missing } => assert_eq!(missing, &["loads-pointer"]),
        other => panic!("expected refusal, got {other:?}"),
    }
}

// Row 9: a pointer path already holding a user file.
#[test]
fn a_pointer_path_holding_a_user_file_degrades_the_adapter_and_is_not_appended_to() {
    let target = tempfile::tempdir().unwrap();
    std::fs::write(target.path().join("AGENTS.md"), b"the user's instructions").unwrap();

    let declarations = vec![adapter(
        "a",
        vec![
            ManagedFile::owned(".statecraft/adapter/notes.md", b"ours".to_vec()),
            ManagedFile::pointer("AGENTS.md", b"see .statecraft/adapter/notes.md".to_vec()),
        ],
    )];

    let computed = plan(
        target.path(),
        None,
        &declarations,
        &present(),
        &ForeignClaims::none(),
    )
    .unwrap();

    match &computed.adapters[0].readiness {
        Readiness::Degraded { reasons } => {
            assert!(reasons[0].contains("AGENTS.md"));
        }
        other => panic!("expected degraded, got {other:?}"),
    }
    let withheld = computed
        .withheld
        .iter()
        .find(|w| w.path == "AGENTS.md")
        .expect("the pointer path is withheld");
    assert!(matches!(
        withheld.reason,
        Withholding::PointerPathOccupied { .. }
    ));
    assert!(
        computed
            .writes
            .iter()
            .any(|w| w.path == ".statecraft/adapter/notes.md"),
        "the adapter still writes its own paths"
    );

    let mut manifest = Manifest::new(pins());
    apply(
        target.path(),
        &mut manifest,
        &declarations,
        &present(),
        &ForeignClaims::none(),
        &clock(),
    )
    .unwrap();
    assert_eq!(
        std::fs::read(target.path().join("AGENTS.md")).unwrap(),
        b"the user's instructions",
        "never appended to, never replaced"
    );
}

// Row 10: a path written by the product but absent from the manifest.
#[test]
fn doctor_reports_a_declared_path_present_on_disk_and_absent_from_the_manifest_as_unmanaged_write()
{
    let target = tempfile::tempdir().unwrap();
    std::fs::write(target.path().join("stray.md"), b"written and not recorded").unwrap();

    let declarations = vec![adapter(
        "a",
        vec![ManagedFile::owned("stray.md", b"x".to_vec())],
    )];
    let manifest = Manifest::new(pins());

    let report = doctor(
        target.path(),
        &manifest,
        &declarations,
        &present(),
        &ForeignClaims::none(),
        &UnobservedShadows,
        &Observed::default(),
    )
    .unwrap();

    assert!(report.findings.iter().any(|f| matches!(
        f,
        Finding::UnmanagedWrite { path, adapter } if path == "stray.md" && adapter == "a"
    )));
    assert_eq!(report.exit_code(), 1);
}

// Row 11: a managed path whose digest matches but which a session would not
// resolve.
#[test]
fn doctor_reports_a_digest_matching_but_shadowed_path_as_shadowed_and_names_the_claimant() {
    let target = tempfile::tempdir().unwrap();
    let declarations = vec![adapter(
        "a",
        vec![ManagedFile::owned(".harness/skill.md", b"ours".to_vec())],
    )];
    let mut manifest = Manifest::new(pins());
    apply(
        target.path(),
        &mut manifest,
        &declarations,
        &present(),
        &ForeignClaims::none(),
        &clock(),
    )
    .unwrap();

    let shadows = StaticShadows::new().shadowing(
        ".harness/skill.md",
        Claimant::Path {
            path: "~/.harness/skill.md".into(),
        },
    );

    let report = doctor(
        target.path(),
        &manifest,
        &declarations,
        &present(),
        &ForeignClaims::none(),
        &shadows,
        &Observed::default(),
    )
    .unwrap();

    let entry = report
        .entries
        .iter()
        .find(|e| e.path == ".harness/skill.md")
        .unwrap();
    match &entry.state {
        State::Shadowed { claimant } => {
            assert!(claimant.describe().contains("~/.harness/skill.md"));
        }
        other => panic!("expected shadowed, got {other:?}"),
    }
    assert_ne!(entry.state.word(), "present");
    assert_eq!(report.exit_code(), 1);
}

// Row 12: doctor against a drifted environment.
#[test]
fn doctor_reports_every_state_exits_non_zero_and_repairs_nothing() {
    let target = tempfile::tempdir().unwrap();
    let declarations = vec![adapter(
        "a",
        vec![
            ManagedFile::owned("ok.md", b"ok".to_vec()),
            ManagedFile::owned("drift.md", b"v1".to_vec()),
            ManagedFile::owned("gone.md", b"g".to_vec()),
        ],
    )];
    let mut manifest = Manifest::new(pins());
    apply(
        target.path(),
        &mut manifest,
        &declarations,
        &present(),
        &ForeignClaims::none(),
        &clock(),
    )
    .unwrap();

    std::fs::write(target.path().join("drift.md"), b"edited").unwrap();
    std::fs::remove_file(target.path().join("gone.md")).unwrap();

    let report = doctor(
        target.path(),
        &manifest,
        &declarations,
        &present(),
        &ForeignClaims::none(),
        &UnobservedShadows,
        &Observed::default(),
    )
    .unwrap();

    let state_of = |p: &str| {
        report
            .entries
            .iter()
            .find(|e| e.path == p)
            .unwrap()
            .state
            .word()
    };
    assert_eq!(state_of("ok.md"), "present");
    assert_eq!(state_of("drift.md"), "drifted");
    assert_eq!(state_of("gone.md"), "missing");
    assert_eq!(report.exit_code(), 1);

    // Repairs nothing.
    assert_eq!(
        std::fs::read(target.path().join("drift.md")).unwrap(),
        b"edited"
    );
    assert!(!target.path().join("gone.md").exists());
}

// The two rows of spec 004 section 3.8 that are this crate's behavior.
//
// Spec 004's `extends` edge names `crates/statecraft-environment/` as an
// additive unit, and its `## Verification` block runs this suite for these two
// rows: an absent prerequisite and a colliding declared path are things the
// ADAPTER MODEL does, not things a provider's stream does. The declaration under
// test is the real one, reached through a dev-dependency (see `Cargo.toml`), so
// these rows cannot pass against a copy that has drifted from it.

// Spec 004 section 3.8: `claude` is absent from the constructed environment.
#[test]
fn spec_008_an_absent_provider_executable_refuses_to_claim_and_names_the_prerequisite() {
    use statecraft_adapter_claude_code::environment as provider_env;

    let declaration = provider_env::declaration();
    let probe = StaticProbe::new()
        .with_harness(provider_env::HARNESS)
        .with_prerequisite(provider_env::HARNESS, provider_env::QUALIFICATION_RECORD)
        .with_prerequisite(provider_env::HARNESS, provider_env::CREDENTIAL_PATH);

    match readiness(&declaration, &probe) {
        Readiness::Refused { missing } => {
            assert_eq!(missing, [provider_env::PROVIDER_EXECUTABLE]);
        }
        other => panic!("expected a refusal naming the prerequisite, got {other:?}"),
    }

    // "It does not write files for a harness that is not there" is the part that
    // matters, so it is checked as an absence on disk and not only as a verdict.
    let target = tempfile::tempdir().unwrap();
    let computed = plan(
        target.path(),
        None,
        std::slice::from_ref(&declaration),
        &probe,
        &ForeignClaims::none(),
    )
    .unwrap();
    assert!(computed.writes.is_empty());

    let mut manifest = Manifest::new(pins());
    let outcome = apply(
        target.path(),
        &mut manifest,
        std::slice::from_ref(&declaration),
        &probe,
        &ForeignClaims::none(),
        &FixedClock(0),
    )
    .unwrap();
    assert_eq!(outcome, Outcome::Applied { written: vec![] });
    for file in &declaration.files {
        assert!(
            !target.path().join(&file.path).exists(),
            "{} was written for a harness whose prerequisite is absent",
            file.path
        );
    }
    assert!(manifest.entries.is_empty());
}

// Spec 004 section 3.8: a second provider adapter declaring a path this one
// declares. Refused at PLAN time, naming both adapters and the path.
#[test]
fn spec_008_a_second_adapter_declaring_this_ones_path_is_refused_at_plan_time() {
    use statecraft_adapter_claude_code::environment as provider_env;

    let first = provider_env::declaration();
    let contested = first.files[0].path.clone();
    let second = Declaration {
        name: "some-other-provider".into(),
        harness: provider_env::HARNESS.into(),
        version: "1".into(),
        files: vec![ManagedFile::owned(&contested, b"theirs".to_vec())],
        unexpressible: vec![],
        prerequisites: vec![],
    };

    let collisions = statecraft_environment::adapter::collisions(&[first.clone(), second.clone()]);
    assert_eq!(collisions.len(), 1);
    assert_eq!(collisions[0].path, contested);
    assert_eq!(
        collisions[0].adapters,
        (first.name.clone(), second.name.clone())
    );

    // At plan time, before anything is written, and with a probe under which
    // both adapters would otherwise have claimed: a collision is a configuration
    // defect whether or not either could write today.
    let mut probe = StaticProbe::new().with_harness(provider_env::HARNESS);
    for p in &first.prerequisites {
        probe = probe.with_prerequisite(provider_env::HARNESS, &p.id);
    }
    let target = tempfile::tempdir().unwrap();
    let computed = plan(
        target.path(),
        None,
        &[first.clone(), second.clone()],
        &probe,
        &ForeignClaims::none(),
    )
    .unwrap();

    assert!(computed.refused());
    let rendered = computed.render();
    assert!(rendered.contains(&contested));
    assert!(rendered.contains(&first.name));
    assert!(rendered.contains(&second.name));

    // The plan still SHOWS what it would have done, which is what makes it a
    // preview. What the refusal buys is that apply writes nothing, so that is
    // where the absence is checked, on disk.
    let mut manifest = Manifest::new(pins());
    let outcome = apply(
        target.path(),
        &mut manifest,
        &[first.clone(), second],
        &probe,
        &ForeignClaims::none(),
        &FixedClock(0),
    )
    .unwrap();
    assert!(outcome.refused());
    for file in &first.files {
        assert!(
            !target.path().join(&file.path).exists(),
            "{} was written despite a plan-time collision",
            file.path
        );
    }
    assert!(manifest.entries.is_empty());
}
