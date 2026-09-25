//! A managed upgrade of the setup profile from revision N to N+1: spec 002
//! section 5, 2026-09-24, the setup-profile entry, section 3 item 4 and
//! acceptance obligation 5.
//!
//! Only one revision is registered, so revision N+1 is the registered
//! profile with two templates changed and its revision advanced: the same
//! type, planned and applied by the same functions a real upgrade uses.

use statecraft_environment::digest::digest_bytes;
use statecraft_environment::manifest::{Manifest, Pins};
use statecraft_home::setup::{self, Action, ConflictKind, Inputs, Profile};
use std::collections::BTreeMap;
use std::path::Path;

const TOML: &str = "[meta]\nrequired_version = \"=0.25.0\"\n";

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (rel, text) in [
        ("spec-spine.toml", TOML),
        ("rust-toolchain.toml", "[toolchain]\nchannel = \"1.96.0\"\n"),
        ("Cargo.lock", "version = 4\n"),
    ] {
        std::fs::write(dir.path().join(rel), text).unwrap();
    }
    std::fs::create_dir_all(dir.path().join(".git")).unwrap();
    dir
}

fn plan_and_apply(root: &Path, profile: &Profile, manifest: &mut Manifest) -> setup::Plan {
    let block = BTreeMap::new();
    let plan = setup::plan(&Inputs {
        root,
        profile,
        block: &block,
        manifest,
        spec_spine_toml: Some(TOML),
        derived_dir: ".statecraft/derived",
    })
    .unwrap();
    setup::apply(
        root,
        &plan,
        manifest,
        "2026-09-24T00:00:00Z",
        &mut |p, b| {
            std::fs::create_dir_all(p.parent().unwrap())?;
            std::fs::write(p, b)
        },
    )
    .unwrap();
    setup::finish(root).unwrap();
    plan
}

fn digest(root: &Path, rel: &str) -> String {
    digest_bytes(&std::fs::read(root.join(rel)).unwrap())
}

#[test]
fn an_unmodified_file_is_replaced_and_a_customized_one_is_kept_with_three_digests() {
    let dir = project();
    let root = dir.path();
    let mut manifest = Manifest::new(Pins {
        product: "0.0.0".into(),
        spec_spine: "unpinned".into(),
        adapters: Default::default(),
        producer: None,
    });
    let n = Profile::registered();
    let first = plan_and_apply(root, &n, &mut manifest);
    assert!(first.whole());

    // The operator customizes one managed file.
    let gate = "scripts/statecraft/gate.sh";
    let written = digest(root, gate);
    let mut customized = std::fs::read_to_string(root.join(gate)).unwrap();
    customized.push_str("# a local addition\n");
    std::fs::write(root.join(gate), &customized).unwrap();

    // Revision N+1 changes that template and another one.
    let mut next = n.clone();
    next.revision = n.revision + 1;
    for t in &mut next.templates {
        if t.path == gate || t.path == "scripts/statecraft/ci-gate.sh" {
            t.body.push_str("# revision N+1\n");
        }
    }
    assert_ne!(next.identity(), n.identity());

    let before_ci_gate = digest(root, "scripts/statecraft/ci-gate.sh");
    let second = plan_and_apply(root, &next, &mut manifest);

    let file = |rel: &str| second.files.iter().find(|f| f.path == rel).unwrap();
    // Unmodified since it was written: replaced by the new bytes.
    let ci_gate = file("scripts/statecraft/ci-gate.sh");
    assert_eq!(ci_gate.action, Action::Replace);
    assert_eq!(ci_gate.previous.as_deref(), Some(before_ci_gate.as_str()));
    assert_eq!(
        digest(root, "scripts/statecraft/ci-gate.sh"),
        ci_gate.intended
    );

    // Customized: left intact; the conflict names all three digests, and the
    // intended bytes are left beside it for a manual merge.
    let g = file(gate);
    assert_eq!(
        g.action,
        Action::Conflict {
            kind: ConflictKind::Customized
        }
    );
    assert_eq!(g.previous.as_deref(), Some(written.as_str()));
    assert_eq!(g.current, digest_bytes(customized.as_bytes()));
    assert_ne!(g.intended, g.current);
    assert_ne!(g.intended, written);
    assert_eq!(
        std::fs::read_to_string(root.join(gate)).unwrap(),
        customized
    );
    let copy = g.intended_copy.as_ref().unwrap();
    assert_eq!(copy, &format!(".statecraft/state/setup/{gate}.intended"));
    assert_eq!(digest(root, copy), g.intended);
    assert!(!second.whole());

    // The manifest records the replaced file at N+1 and keeps the customized
    // file's previous record, so the next upgrade still compares three.
    let entry = manifest.entry("scripts/statecraft/ci-gate.sh").unwrap();
    assert_eq!(entry.source.identity, next.source_identity());
    assert_eq!(entry.digest, ci_gate.intended);
    assert_eq!(manifest.entry(gate).unwrap().digest, written);
    assert_eq!(
        manifest.project.setup.as_ref().unwrap().revision,
        next.revision
    );
}

const R3_TRIGGER: &str = "  # Revision 3: a required merge queue judges each queued entry here. The\n  # review is not re-run for it; ci-gate reads the verdict recorded for the\n  # entry's pull request.\n  merge_group:\n";

/// Revision 2 of the registered profile, rebuilt from revision 3 by undoing
/// revision 3's marks in the two templates a managed upgrade compares: the
/// `merge_group` trigger and the gate's `recorded-review` rule. Only the
/// templates matter to a managed upgrade.
fn revision_two() -> Profile {
    let mut r2 = Profile::registered();
    assert_eq!(r2.revision, 3, "the registered profile is revision 3");
    r2.revision = 2;
    for t in &mut r2.templates {
        if t.path == ".github/workflows/statecraft-ci.yml" {
            assert!(
                t.body.contains(R3_TRIGGER),
                "revision 3's merge_group trigger"
            );
            t.body = t.body.replace(R3_TRIGGER, "");
        }
        if t.path == "scripts/statecraft/ci-gate.sh" {
            assert!(t.body.contains("recorded-review"), "revision 3's gate rule");
            t.body = t.body.replace("recorded-review", "recorded-review-r2");
        }
    }
    r2
}

/// Revision 1, rebuilt from revision 2 by undoing revision 2's two template
/// changes (the owner exception for a findings verdict, R2-1).
fn revision_one() -> Profile {
    let mut r1 = revision_two();
    r1.revision = 1;
    for t in &mut r1.templates {
        if t.path == ".github/workflows/statecraft-ci.yml" {
            let from = "    # The owner exception (S-1): a release candidate whose review was skipped,\n    # and (revision 2, R2-1) any pull request whose review returned findings.\n    if: always() && github.event_name == 'pull_request' && ((needs.ai-review.outputs.release_candidate == 'true' && startsWith(needs.ai-review.outputs.result, 'skipped:')) || needs.ai-review.outputs.result == 'findings')";
            assert!(t.body.contains(from), "revision 2's exception condition");
            t.body = t.body.replace(
                from,
                "    if: always() && github.event_name == 'pull_request' && needs.ai-review.outputs.release_candidate == 'true' && startsWith(needs.ai-review.outputs.result, 'skipped:')",
            );
        }
        if t.path == "scripts/statecraft/ci-gate.sh" {
            assert!(t.body.contains("owner-exception"), "revision 2's gate rule");
            t.body = t.body.replace("owner-exception", "rc-exception-r1");
        }
    }
    r1
}

#[test]
fn a_revision_one_project_upgrades_to_revision_two() {
    let dir = project();
    let root = dir.path();
    let mut manifest = Manifest::new(Pins {
        product: "0.0.0".into(),
        spec_spine: "unpinned".into(),
        adapters: Default::default(),
        producer: None,
    });
    let r1 = revision_one();
    assert!(plan_and_apply(root, &r1, &mut manifest).whole());
    assert_eq!(manifest.project.setup.as_ref().unwrap().revision, 1);

    // The operator customized the workflow; the gate script is as written.
    let wf = ".github/workflows/statecraft-ci.yml";
    let mut customized = std::fs::read_to_string(root.join(wf)).unwrap();
    customized.push_str("# a local addition\n");
    std::fs::write(root.join(wf), &customized).unwrap();

    let r2 = revision_two();
    assert_ne!(r2.identity(), r1.identity());
    let upgrade = plan_and_apply(root, &r2, &mut manifest);
    let file = |rel: &str| upgrade.files.iter().find(|f| f.path == rel).unwrap();

    // Unchanged since written: rewritten with revision 2's gate.
    assert_eq!(
        file("scripts/statecraft/ci-gate.sh").action,
        Action::Replace
    );
    let gate = std::fs::read_to_string(root.join("scripts/statecraft/ci-gate.sh")).unwrap();
    assert!(gate.contains("owner-exception"), "{gate}");

    // Customized: withheld, bytes intact, revision 2's copy left beside it.
    assert_eq!(
        file(wf).action,
        Action::Conflict {
            kind: ConflictKind::Customized
        }
    );
    assert_eq!(std::fs::read_to_string(root.join(wf)).unwrap(), customized);
    assert!(!upgrade.whole());
    assert_eq!(manifest.project.setup.as_ref().unwrap().revision, 2);
}

/// Revision 2's operator steps: no human approval for ordinary changes,
/// code-owner review for the profile's files (R2-2), `ci-gate` bound to
/// GitHub Actions, and the exception Environment for a findings verdict.
#[test]
fn revision_two_states_the_approver_model_in_its_operator_steps() {
    let steps = setup::remote_obligations().join("\n");
    assert!(steps.contains("required approvals 0"), "{steps}");
    assert!(steps.contains("require code-owner review"), "{steps}");
    assert!(
        steps.contains("ci-gate from GitHub Actions (app id 15368)"),
        "{steps}"
    );
    assert!(
        steps.contains("a pull request whose review returned findings"),
        "{steps}"
    );
    let policy = setup::jobs();
    assert_eq!(
        policy["review-exception"]["pull_request"],
        "owner-exception"
    );
}

#[test]
fn a_revision_two_project_upgrades_to_revision_three() {
    let dir = project();
    let root = dir.path();
    let mut manifest = Manifest::new(Pins {
        product: "0.0.0".into(),
        spec_spine: "unpinned".into(),
        adapters: Default::default(),
        producer: None,
    });
    let r2 = revision_two();
    assert!(plan_and_apply(root, &r2, &mut manifest).whole());
    assert_eq!(manifest.project.setup.as_ref().unwrap().revision, 2);

    // The operator customized the workflow; the gate script is as written.
    let wf = ".github/workflows/statecraft-ci.yml";
    let mut customized = std::fs::read_to_string(root.join(wf)).unwrap();
    customized.push_str("# a local addition\n");
    std::fs::write(root.join(wf), &customized).unwrap();

    let r3 = Profile::registered();
    assert_ne!(r3.identity(), r2.identity());
    let upgrade = plan_and_apply(root, &r3, &mut manifest);
    let file = |rel: &str| upgrade.files.iter().find(|f| f.path == rel).unwrap();

    // Unchanged since written: rewritten with revision 3's gate.
    assert_eq!(
        file("scripts/statecraft/ci-gate.sh").action,
        Action::Replace
    );
    let gate = std::fs::read_to_string(root.join("scripts/statecraft/ci-gate.sh")).unwrap();
    assert!(gate.contains("recorded-review)"), "{gate}");

    // Customized: withheld, bytes intact; the operator merges the trigger in.
    assert_eq!(
        file(wf).action,
        Action::Conflict {
            kind: ConflictKind::Customized
        }
    );
    assert_eq!(std::fs::read_to_string(root.join(wf)).unwrap(), customized);
    assert!(!upgrade.whole());
    assert_eq!(manifest.project.setup.as_ref().unwrap().revision, 3);
}

/// Revision 3's merge-queue rules: every event the rendered CI triggers on has
/// a rule for every required job, and the operator steps state the upgrade
/// order a base-read gate imposes.
#[test]
fn revision_three_states_a_rule_for_the_queue_and_the_upgrade_order() {
    let policy = setup::jobs();
    for job in ["governance", "code", "ai-review", "review-exception"] {
        for event in ["pull_request", "push", "merge_group"] {
            assert!(
                policy[job][event].is_string(),
                "{job} has no rule for {event}"
            );
        }
    }
    assert_eq!(policy["governance"]["merge_group"], "required");
    assert_eq!(policy["code"]["merge_group"], "required");
    assert_eq!(policy["ai-review"]["merge_group"], "recorded-review");
    assert_eq!(policy["review-exception"]["merge_group"], "inapplicable");
    let steps = setup::remote_obligations().join("\n");
    assert!(steps.contains("upgrade to revision 3 first"), "{steps}");
    assert!(steps.contains("never by a second review"), "{steps}");
}
