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
    plan_and_apply_with(root, profile, manifest, &BTreeMap::new())
}

/// As the flow plans: `block` is the parameters the declaration records.
fn plan_and_apply_with(
    root: &Path,
    profile: &Profile,
    manifest: &mut Manifest,
    block: &BTreeMap<String, serde_json::Value>,
) -> setup::Plan {
    let plan = setup::plan(&Inputs {
        root,
        profile,
        block,
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

const R5_EXCEPTION: &str = "    needs: [governance, ai-review]\n    # The owner exception (S-1): a release candidate whose review was skipped,\n    # (revision 2, R2-1) any pull request whose review returned findings, and\n    # (revision 5, rule 2) any pull request that changes the authority set.\n    if: always() && github.event_name == 'pull_request' && (needs.governance.outputs.authority_change == 'true' || (needs.ai-review.outputs.release_candidate == 'true' && startsWith(needs.ai-review.outputs.result, 'skipped:')) || needs.ai-review.outputs.result == 'findings')";
const R4_EXCEPTION: &str = "    needs: [ai-review]\n    # The owner exception (S-1): a release candidate whose review was skipped,\n    # and (revision 2, R2-1) any pull request whose review returned findings.\n    if: always() && github.event_name == 'pull_request' && ((needs.ai-review.outputs.release_candidate == 'true' && startsWith(needs.ai-review.outputs.result, 'skipped:')) || needs.ai-review.outputs.result == 'findings')";

/// Revision 4 of the registered profile, rebuilt from revision 5 by undoing
/// revision 5's marks in the three templates a managed upgrade compares: the
/// workflow runs the candidate's gate and runs the exception job for no
/// authority change, the commit walk runs each commit's own gate, and
/// `ci-gate.sh` only reports an authority change. Only the templates matter
/// to a managed upgrade.
fn revision_four() -> Profile {
    let mut r4 = Profile::registered();
    assert_eq!(r4.revision, 5, "the registered profile is revision 5");
    r4.revision = 4;
    for t in &mut r4.templates {
        if t.path == ".github/workflows/statecraft-ci.yml" {
            assert!(t.body.contains(R5_EXCEPTION), "revision 5's exception job");
            t.body = t.body.replace(R5_EXCEPTION, R4_EXCEPTION);
            assert!(
                t.body.contains("sh \"${STATECRAFT_GATE:?}\" "),
                "revision 5's gate read at the base"
            );
            t.body = t.body.replace(
                "sh \"${STATECRAFT_GATE:?}\" ",
                "sh scripts/statecraft/gate.sh ",
            );
        }
        if t.path == "scripts/statecraft/gate.sh" {
            assert!(
                t.body.contains("script=\"$SELF\""),
                "revision 5's walk judges with the running gate"
            );
            t.body = t.body.replace(
                "script=\"$SELF\"",
                "script=\"$wt/scripts/statecraft/gate.sh\"",
            );
        }
        if t.path == "scripts/statecraft/ci-gate.sh" {
            assert!(
                t.body.contains("authority=yes"),
                "revision 5's authority rule"
            );
            t.body = t.body.replace("authority=yes", "authority=reported-r4");
        }
    }
    r4
}

/// Revision 3 of the registered profile, rebuilt from revision 4 by undoing
/// revision 4's marks in the two templates a managed upgrade compares: the
/// gate script's per-commit walk and the workflow's step that runs it. Only
/// the templates matter to a managed upgrade.
fn revision_three() -> Profile {
    let mut r3 = revision_four();
    assert_eq!(
        r3.revision, 4,
        "revision 4 is rebuilt from the registered one"
    );
    r3.revision = 3;
    for t in &mut r3.templates {
        if t.path == ".github/workflows/statecraft-ci.yml" {
            assert!(
                t.body.contains("gate.sh commits"),
                "revision 4's commit walk step"
            );
            t.body = t.body.replace("gate.sh commits", "gate.sh commits-r3");
        }
        if t.path == "scripts/statecraft/gate.sh" {
            assert!(t.body.contains("  commits)"), "revision 4's commit walk");
            t.body = t.body.replace("  commits)", "  commits-r3)");
        }
    }
    r3
}

const R3_TRIGGER: &str = "  # Revision 3: a required merge queue judges each queued entry here. The\n  # review is not re-run for it; ci-gate reads the verdict recorded for the\n  # entry's pull request.\n  merge_group:\n";

/// Revision 2 of the registered profile, rebuilt from revision 3 by undoing
/// revision 3's marks in the two templates a managed upgrade compares: the
/// `merge_group` trigger and the gate's `recorded-review` rule. Only the
/// templates matter to a managed upgrade.
fn revision_two() -> Profile {
    let mut r2 = revision_three();
    assert_eq!(
        r2.revision, 3,
        "revision 3 is rebuilt from the registered one"
    );
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

    let r3 = revision_three();
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

fn applied_revision_three(with_script: bool) -> (tempfile::TempDir, Manifest) {
    let dir = project();
    let root = dir.path();
    if with_script {
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        std::fs::write(
            root.join(setup::AUTHORED_CONTENT_SCRIPT),
            "#!/bin/sh\nexit 0\n",
        )
        .unwrap();
    }
    let mut manifest = Manifest::new(Pins {
        product: "0.0.0".into(),
        spec_spine: "unpinned".into(),
        adapters: Default::default(),
        producer: None,
    });
    assert!(plan_and_apply(root, &revision_three(), &mut manifest).whole());
    let selection = manifest.project.setup.as_ref().unwrap();
    assert_eq!(selection.revision, 3);
    // Revision 3 recorded no governance parameter: its gate ran the script
    // whenever it was executable.
    assert!(
        !selection
            .parameters
            .contains_key("governance.authored_content")
    );
    (dir, manifest)
}

/// Revision 4, rule 2 and item 7: the upgrade from revision 3 declares the
/// authored-content script when it exists, so the check a revision-3 project
/// ran is kept, and declares nothing when it does not.
#[test]
fn a_revision_three_project_upgrades_to_revision_four() {
    for with_script in [true, false] {
        let (dir, mut manifest) = applied_revision_three(with_script);
        let root = dir.path();
        let r3 = revision_three();
        let r4 = revision_four();
        assert_ne!(r4.identity(), r3.identity());
        let upgrade = plan_and_apply(root, &r4, &mut manifest);
        let file = |rel: &str| upgrade.files.iter().find(|f| f.path == rel).unwrap();

        // Unchanged since written: rewritten with revision 4's gate.
        assert_eq!(file("scripts/statecraft/gate.sh").action, Action::Replace);
        assert_eq!(
            file(".github/workflows/statecraft-ci.yml").action,
            Action::Replace
        );
        let gate = std::fs::read_to_string(root.join("scripts/statecraft/gate.sh")).unwrap();
        assert!(gate.contains("  commits)"), "{gate}");
        assert!(upgrade.whole());

        let selection = manifest.project.setup.as_ref().unwrap();
        assert_eq!(selection.revision, 4);
        let declared = selection.parameters.get("governance.authored_content");
        if with_script {
            assert_eq!(
                upgrade.parameters.authored_content.as_deref(),
                Some(setup::AUTHORED_CONTENT_SCRIPT)
            );
            assert_eq!(
                declared,
                Some(&serde_json::json!(setup::AUTHORED_CONTENT_SCRIPT))
            );
            assert!(
                gate.contains("AUTHORED_CONTENT='scripts/check-authored-content.sh'"),
                "{gate}"
            );
            let policy: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(root.join(setup::POLICY_PATH)).unwrap(),
            )
            .unwrap();
            assert_eq!(
                policy["parameters"]["authored_content"],
                setup::AUTHORED_CONTENT_SCRIPT
            );
        } else {
            assert_eq!(upgrade.parameters.authored_content, None);
            assert_eq!(declared, None);
            assert!(gate.contains("AUTHORED_CONTENT=''"), "{gate}");
        }
        assert!(upgrade.render().contains(if with_script {
            "authored content: scripts/check-authored-content.sh"
        } else {
            "authored content: none declared"
        }));

        // A re-plan at revision 4, with the parameters the declaration now
        // records (as the flow plans), is stable: it neither adds nor drops
        // the declaration, and writes nothing.
        let recorded = manifest.project.setup.as_ref().unwrap().parameters.clone();
        let again = plan_and_apply_with(root, &r4, &mut manifest, &recorded);
        assert!(again.files.iter().all(|f| !f.action.writes()));
        assert_eq!(
            again.parameters.authored_content,
            upgrade.parameters.authored_content
        );
    }
}

/// Revision 4, item 7: `ci-gate`'s policy is unchanged, and the operator
/// steps name the new refusal and the parameters that keep checks run by
/// hand.
#[test]
fn revision_four_keeps_the_policy_and_names_the_new_refusal() {
    let policy = setup::jobs();
    assert_eq!(policy["governance"]["pull_request"], "required");
    assert_eq!(policy["governance"]["merge_group"], "required");
    assert_eq!(policy["code"]["merge_group"], "required");
    assert_eq!(policy["ai-review"]["merge_group"], "recorded-review");
    assert_eq!(policy.as_object().unwrap().len(), 4);
    let steps = setup::remote_obligations().join("\n");
    assert!(
        steps.contains("a pull request whose base is not the default branch fails governance"),
        "{steps}"
    );
    for parameter in [
        "governance.require_default_base",
        "governance.enforce_coverage",
        "governance.authored_content",
        "governance.authored_content_text",
        "governance.gate_each_commit",
        "governance.require_signed_commits",
    ] {
        assert!(steps.contains(parameter), "{parameter}: {steps}");
    }
}

/// Revision 5, item 4: a revision-4 project upgrades through one plan and
/// apply. The unchanged workflow and scripts are rewritten with revision 5's
/// reading of the gate at the base and its authority rule, and the declared
/// parameters carry forward unchanged.
#[test]
fn a_revision_four_project_upgrades_to_revision_five() {
    let dir = project();
    let root = dir.path();
    std::fs::create_dir_all(root.join("scripts")).unwrap();
    std::fs::write(
        root.join(setup::AUTHORED_CONTENT_SCRIPT),
        "#!/bin/sh\nexit 0\n",
    )
    .unwrap();
    let mut manifest = Manifest::new(Pins {
        product: "0.0.0".into(),
        spec_spine: "unpinned".into(),
        adapters: Default::default(),
        producer: None,
    });
    let mut block = BTreeMap::new();
    for (k, v) in [
        (
            "governance.authored_content",
            serde_json::json!(setup::AUTHORED_CONTENT_SCRIPT),
        ),
        ("governance.gate_each_commit", serde_json::json!(true)),
    ] {
        block.insert(k.to_string(), v);
    }
    let r4 = revision_four();
    assert!(plan_and_apply_with(root, &r4, &mut manifest, &block).whole());
    assert_eq!(manifest.project.setup.as_ref().unwrap().revision, 4);
    let wf = ".github/workflows/statecraft-ci.yml";
    let before = std::fs::read_to_string(root.join(wf)).unwrap();
    assert!(
        before.contains("sh scripts/statecraft/gate.sh governance"),
        "{before}"
    );

    let r5 = Profile::registered();
    assert_ne!(r5.identity(), r4.identity());
    let recorded = manifest.project.setup.as_ref().unwrap().parameters.clone();
    let upgrade = plan_and_apply_with(root, &r5, &mut manifest, &recorded);
    let file = |rel: &str| upgrade.files.iter().find(|f| f.path == rel).unwrap();
    for rel in [
        wf,
        "scripts/statecraft/gate.sh",
        "scripts/statecraft/ci-gate.sh",
    ] {
        assert_eq!(file(rel).action, Action::Replace, "{rel}");
    }
    assert!(upgrade.whole());
    let workflow = std::fs::read_to_string(root.join(wf)).unwrap();
    assert!(
        workflow.contains("sh \"${STATECRAFT_GATE:?}\" governance"),
        "{workflow}"
    );
    assert!(
        workflow.contains("needs.governance.outputs.authority_change == 'true'"),
        "{workflow}"
    );
    let gate = std::fs::read_to_string(root.join("scripts/statecraft/gate.sh")).unwrap();
    assert!(gate.contains("script=\"$SELF\""), "{gate}");
    assert!(
        gate.contains("AUTHORED_CONTENT='scripts/check-authored-content.sh'"),
        "{gate}"
    );
    assert!(gate.contains("GATE_EACH_COMMIT=true"), "{gate}");
    let ci_gate = std::fs::read_to_string(root.join("scripts/statecraft/ci-gate.sh")).unwrap();
    assert!(ci_gate.contains("authority=yes"), "{ci_gate}");

    let selection = manifest.project.setup.as_ref().unwrap();
    assert_eq!(selection.revision, 5);
    assert_eq!(selection.parameters, recorded);
    let policy: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join(setup::POLICY_PATH)).unwrap())
            .unwrap();
    assert_eq!(policy["revision"], 5);
    assert_eq!(
        policy["parameters"]["authored_content"],
        setup::AUTHORED_CONTENT_SCRIPT
    );
    assert!(policy["authority_rule"].is_object(), "{policy}");
}

/// Revision 5, items 3 and 4: the operator steps say that every re-render or
/// authority-set change needs the owner's approval once, and that the upgrade
/// from revision 4 is judged by the base's revision-4 gate, which only
/// reports it; `ci-gate`'s jobs and rules are unchanged.
#[test]
fn revision_five_names_the_owner_approval_in_its_operator_steps() {
    let steps = setup::remote_obligations().join("\n");
    for said in [
        "a candidate never judges itself with its own gate",
        "run as they exist at the base",
        "Every re-render of the profile, and every change to a file of the authority set, therefore needs the owner's approval once",
        "judged by the base's revision-4 ci-gate, which reports the authority change and does not block it",
        "procedural",
        "a pull request that changes the authority set (revision 5)",
    ] {
        assert!(steps.contains(said), "{said}: {steps}");
    }
    let policy = setup::jobs();
    assert_eq!(policy.as_object().unwrap().len(), 4);
    assert_eq!(
        policy["review-exception"]["pull_request"],
        "owner-exception"
    );
    assert_eq!(policy["review-exception"]["merge_group"], "inapplicable");
    let rule = setup::authority_rule();
    assert_eq!(rule["exception_environment"], setup::EXCEPTION_ENVIRONMENT);
}
