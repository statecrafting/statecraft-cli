//! Revision 13's repository-local engine location across the setup profile.

use statecraft_home::{producer, setup};

#[test]
fn revision_thirteen_uses_bin_for_every_managed_engine_path() {
    let profile = setup::Profile::registered();
    assert!(profile.revision >= 13);
    assert_eq!(producer::TOOL_DIR, ".bin");
    assert!(setup::IGNORE_FRAGMENT.contains(".bin/"));
    // Spec 031 retains the old ignore line without reading or executing that directory.
    assert!(
        setup::IGNORE_FRAGMENT
            .lines()
            .any(|line| line == ".tooling/")
    );

    let template = |path: &str| {
        profile
            .templates
            .iter()
            .find(|template| template.path == path)
            .unwrap_or_else(|| panic!("missing template {path}"))
            .body
            .as_str()
    };
    let installer = template("scripts/statecraft/install-spec-spine.sh");
    assert!(installer.contains("mktemp -d"), "{installer}");
    assert!(installer.contains("--root \"$scratch\""), "{installer}");
    assert!(installer.contains(".bin/spec-spine"), "{installer}");
    let gate = template("scripts/statecraft/gate.sh");
    assert!(gate.contains(".bin/spec-spine"), "{gate}");
    let workflow = template(".github/workflows/statecraft-ci.yml");
    assert!(workflow.contains(".bin/spec-spine"), "{workflow}");

    let rendered = profile
        .templates
        .iter()
        .map(|template| template.body.as_str())
        .chain(std::iter::once(setup::IGNORE_FRAGMENT))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!rendered.contains(".tooling/bin"), "{rendered}");

    let commands = setup::commands().to_string();
    assert!(commands.contains(".bin/spec-spine"), "{commands}");
    assert!(!commands.contains(".tooling/bin"), "{commands}");
}
