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
