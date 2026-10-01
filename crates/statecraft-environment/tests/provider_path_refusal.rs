//! Spec 030 sections 3.1 and 3.4: an adapter declaration naming a path inside
//! a provider-specific directory is refused at plan time, naming the adapter
//! and the path, and the refused apply writes nothing.

use statecraft_environment::adapter::{
    Declaration, ManagedFile, PROVIDER_DIRECTORIES, StaticProbe, provider_directory,
};
use statecraft_environment::apply::{Outcome, apply};
use statecraft_environment::claimant::ForeignClaims;
use statecraft_environment::manifest::{Manifest, Pins};
use statecraft_environment::plan::{Refusal, plan};
use statecraft_environment::time::FixedClock;
use std::collections::BTreeMap;
use std::path::Path;

const HARNESS: &str = "test-harness";

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

#[test]
fn a_declared_path_inside_a_provider_directory_is_refused_and_nothing_is_written() {
    for directory in PROVIDER_DIRECTORIES {
        let target = tempfile::tempdir().unwrap();
        let path = format!("{directory}/statecraft/instructions.md");
        let declarations = [adapter(
            "a",
            vec![
                ManagedFile::owned(&path, b"x".to_vec()),
                ManagedFile::owned("fine.md", b"y".to_vec()),
            ],
        )];

        let planned = plan(
            target.path(),
            None,
            &declarations,
            &present(),
            &ForeignClaims::none(),
        )
        .unwrap();
        assert!(
            planned
                .refusals
                .iter()
                .any(|r| matches!(r, Refusal::ProviderDirectory(p) if p.path == path)),
            "{planned:?}"
        );
        assert!(
            planned.refusals[0]
                .describe()
                .contains("adapter a declares")
        );

        let mut manifest = Manifest::new(pins());
        let outcome = run(target.path(), &mut manifest, &declarations);
        assert!(matches!(outcome, Outcome::Refused { .. }), "{outcome:?}");
        assert!(!target.path().join(directory).exists(), "{directory}");
        assert!(
            !target.path().join("fine.md").exists(),
            "{directory}: a refusal writes no declared file"
        );
    }
}

#[test]
fn a_provider_directory_is_matched_by_its_first_component_only() {
    for directory in PROVIDER_DIRECTORIES {
        assert_eq!(
            provider_directory(&format!("{directory}/x")),
            Some(directory)
        );
        assert_eq!(
            provider_directory(&format!("./{directory}/x")),
            Some(directory)
        );
    }
    // Neither a root pointer file nor a provider name deeper in the tree is a
    // provider directory in the target.
    for path in [
        "CLAUDE.md",
        "AGENTS.md",
        ".github/workflows/ci.yml",
        "docs/.claude/x",
        ".claudex/y",
    ] {
        assert_eq!(provider_directory(path), None, "{path}");
    }
}

#[test]
fn a_root_pointer_file_is_not_refused() {
    let target = tempfile::tempdir().unwrap();
    let declarations = [adapter(
        "a",
        vec![ManagedFile::pointer(
            "CLAUDE.md",
            b"@.statecraft/AGENTS.md\n".to_vec(),
        )],
    )];
    let planned = plan(
        target.path(),
        None,
        &declarations,
        &present(),
        &ForeignClaims::none(),
    )
    .unwrap();
    assert!(planned.refusals.is_empty(), "{planned:?}");
}
