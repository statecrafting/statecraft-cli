//! The per-project Statecraft area, and the managed instructions inside it.
//!
//! Spec 002 section 3.12. Exactly four paths, and no others. The existing
//! locations are preserved and not moved: `spec-spine.toml` at the root,
//! `specs/` at the root, `standards/spec/` where it is, and every ordinary
//! project document where it is.

use statecraft_environment::manifest::Manifest;
use std::path::Path;

/// The project area.
pub const AREA: &str = ".statecraft";

/// The committed environment declaration. The same path spec 002 fixed.
pub const DECLARATION: &str = statecraft_environment::manifest::MANIFEST_PATH;

/// The Statecraft-managed project instructions.
pub const INSTRUCTIONS: &str = ".statecraft/AGENTS.md";

/// The committed compiled artifacts.
pub const DERIVED: &str = ".statecraft/derived";

/// The ignored runtime state.
pub const STATE: &str = ".statecraft/state";

/// The user's own cross-agent instruction file, which this product never owns.
pub const ROOT_INSTRUCTIONS: &str = "AGENTS.md";

/// Where the initialization flow records how far it got.
pub const INIT_STATE: &str = ".statecraft/state/init.json";

/// True when this repository is a Statecraft project.
///
/// The predicate every delivered harness behavior gates on (section 3.4), so it
/// is one function rather than a file test spelled slightly differently in five
/// places.
pub fn is_statecraft_project(root: &Path) -> bool {
    statecraft_environment::claimant::resolve(root, DECLARATION).is_file()
}

/// The managed project instructions this product writes.
///
/// Deliberately about **this product's** vocabulary and nothing else. A project
/// that wants to say more says it in its own root file, which this product does
/// not own and does not read for control purposes.
pub fn managed_instructions() -> String {
    MANAGED_INSTRUCTIONS.to_string()
}

const MANAGED_INSTRUCTIONS: &str = r#"# Statecraft: managed project instructions

This file is **managed by Statecraft**. It is rewritten on upgrade and removed
on removal, and its ownership is recorded in `.statecraft/environment.json`.
Write your own instructions in the repository's own `AGENTS.md`, which
Statecraft never owns and never rewrites.

## The area

| Path | Committed | Holds |
|---|---|---|
| `.statecraft/environment.json` | yes | The environment declaration: pins, managed-file ownership, tracked modifications, and the project block. |
| `.statecraft/AGENTS.md` | yes | This file. |
| `.statecraft/derived/` | yes | spec-spine's compiled artifacts. |
| `.statecraft/state/` | no | Runtime state. Ignored. |

`.statecraft/` as a whole is never ignored. Ignoring it would take the
declaration, these instructions and the compiled artifacts out of version
control in one line, and those three are what make the project governed.

## Reading the governed artifacts

Read `.statecraft/derived/` only through `spec-spine` subcommands. A typed read
fails at the deserializer with a clean error; an ad-hoc parse silently encodes a
stale assumption.

## Four acts, never inferred from one another

Registration makes a project visible. Qualification is a read-only verdict.
Arming consents to the project being driven. An execution posture is a separate
consent again. None of them implies the next.

## Where a value came from

`statecraft-cli config show <path>` answers per key, with the layer that
supplied it and every layer that constrained it. A key no layer supplies is
unknown, and unknown is not success: do not substitute a default for it.
"#;

/// Read the declaration, if the project has one.
pub fn declaration(
    root: &Path,
) -> Result<Option<Manifest>, statecraft_environment::manifest::ManifestError> {
    Manifest::read(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_area_holds_exactly_the_four_declared_paths() {
        let four = [DECLARATION, INSTRUCTIONS, DERIVED, STATE];
        for path in four {
            assert!(path.starts_with(AREA), "{path} is outside the area");
        }
        // The root instruction file is deliberately NOT one of them.
        assert!(!ROOT_INSTRUCTIONS.starts_with(AREA));
    }

    #[test]
    fn a_repository_without_a_declaration_is_not_a_statecraft_project() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_statecraft_project(dir.path()));
        let decl = statecraft_environment::claimant::resolve(dir.path(), DECLARATION);
        std::fs::create_dir_all(decl.parent().unwrap()).unwrap();
        std::fs::write(&decl, "{}").unwrap();
        assert!(is_statecraft_project(dir.path()));
    }

    #[test]
    fn the_managed_instructions_say_they_are_managed_and_point_at_the_users_own_file() {
        let text = managed_instructions();
        assert!(text.contains("managed by Statecraft"));
        assert!(text.contains("`AGENTS.md`"));
        assert!(text.contains("never ignored") || text.contains("never be ignored"));
    }
}
