//! The environment half: what this adapter declares, and when it refuses.
//!
//! Spec 008 section 3.7, under spec 002 section 3.9. An adapter declares the
//! harness it targets, the exact set of paths it would manage, the facts it
//! cannot express in that harness, and the prerequisites it needs present.
//!
//! Its prerequisites are the three section 3.7 names:
//!
//! 1. [`PROVIDER_EXECUTABLE`]: a `claude` executable resolvable by the
//!    constructed environment;
//! 2. [`QUALIFICATION_RECORD`]: a provider version this adapter has a record
//!    for (section 3.8);
//! 3. [`CREDENTIAL_PATH`]: the credential path of section 3.6.
//!
//! Absent any of them it **refuses to claim its paths and names which one is
//! absent**. It does not write files for a harness that is not there, and it
//! never appends to a file it does not own (spec 002 section 3.8, `D-09`).
//!
//! # The pointer, and why the existing file wins
//!
//! Spec 002 section 3.8 allows the product its own file at its own path plus
//! **at most one** pointer, written only where no file exists at that path. The
//! pointer here is `CLAUDE.md` carrying an `@` import, which is the mechanism
//! this harness actually loads. An existing `CLAUDE.md` is `user` class: it is
//! reported `foreign`, the adapter reports itself degraded, and nothing is
//! appended or merged.

use statecraft_environment::adapter::{Declaration, ManagedFile, Prerequisite};

/// The harness this adapter targets, as spec 002 section 3.9 names one.
pub const HARNESS: &str = "claude-code";

/// A `claude` executable resolvable by the constructed environment.
pub const PROVIDER_EXECUTABLE: &str = "provider-executable";

/// A provider version this adapter has a qualification record for.
pub const QUALIFICATION_RECORD: &str = "qualification-record";

/// The credential path of spec 008 section 3.6.
pub const CREDENTIAL_PATH: &str = "credential-path";

/// The adapter's own file, which it owns outright.
pub const OWNED_INSTRUCTIONS: &str = ".claude/statecraft/instructions.md";

/// The single pointer path, written only where nothing exists.
pub const POINTER: &str = "CLAUDE.md";

/// Facts this adapter cannot express in this harness.
///
/// Spec 002 section 3.9 requires them stated rather than dropped, and each of
/// these is a measurement from spec 008 rather than a caution.
pub const UNEXPRESSIBLE: [&str; 3] = [
    "the applied tool allowlist: the init event does not reflect an allowlist, so what was \
     applied is recorded `not-recorded` and never the request restated (section 3.4)",
    "that a refused session is not a completion: the provider's own terminal classification \
     calls a denied session `success` and the process exits 0, so the harness cannot be told \
     to report it otherwise and the correction is the supervisor's (section 3.3)",
    "confinement of writes to the prepared workspace: `workspace-write` is not declared, \
     because nobody measured it (section 3.2)",
];

/// The pointer file's contents.
///
/// An `@` import rather than a copy: the pointer names the adapter's own file
/// instead of duplicating it, so there is exactly one place the instructions
/// live and no second copy to drift.
pub fn pointer_contents() -> Vec<u8> {
    format!("@{OWNED_INSTRUCTIONS}\n").into_bytes()
}

/// The adapter's own instructions file.
///
/// Deliberately short. What belongs in a target's instructions is not this
/// spec's question, and a file that said more would be this adapter deciding it.
pub fn instructions_contents() -> Vec<u8> {
    let mut out = String::new();
    out.push_str("# statecraft: managed harness instructions\n\n");
    out.push_str(
        "This file is managed by statecraft's `claude-code` adapter and is recorded in\n\
         `.statecraft/environment.json`. Edit it and `doctor` reports the path as\n\
         `drifted`; it is never repaired silently.\n\n",
    );
    out.push_str("Facts this harness cannot express, stated rather than dropped:\n\n");
    for fact in UNEXPRESSIBLE {
        out.push_str(&format!("- {fact}\n"));
    }
    out.into_bytes()
}

/// The three prerequisites section 3.7 names.
pub fn prerequisites() -> Vec<Prerequisite> {
    vec![
        Prerequisite::new(
            PROVIDER_EXECUTABLE,
            "a `claude` executable resolvable by the constructed environment; the environment \
             is constructed from an allowed set rather than inherited, so an executable on the \
             operator's own PATH is not evidence it is on the child's",
        ),
        Prerequisite::new(
            QUALIFICATION_RECORD,
            "a recorded suite pass naming this adapter build AND the provider version in front \
             of it; a record for another provider version does not transfer (section 3.8)",
        ),
        Prerequisite::new(
            CREDENTIAL_PATH,
            "the credential path of section 3.6: on darwin this provider resolves credentials \
             through the operating-system keychain, which a constructed environment does not \
             remove and cannot record. This prerequisite is the presence of the MECHANISM, not \
             of a working credential: checking a credential means spending one",
        ),
    ]
}

/// This adapter's declaration, as spec 002 section 3.9 reads one.
pub fn declaration() -> Declaration {
    Declaration {
        name: HARNESS.to_string(),
        harness: HARNESS.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        files: vec![
            ManagedFile::owned(OWNED_INSTRUCTIONS, instructions_contents()),
            ManagedFile::pointer(POINTER, pointer_contents()),
        ],
        unexpressible: UNEXPRESSIBLE.iter().map(|f| (*f).to_string()).collect(),
        prerequisites: prerequisites(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use statecraft_environment::adapter::{Readiness, StaticProbe, collisions, readiness};

    fn satisfied() -> StaticProbe {
        let mut probe = StaticProbe::new().with_harness(HARNESS);
        for p in prerequisites() {
            probe = probe.with_prerequisite(HARNESS, &p.id);
        }
        probe
    }

    #[test]
    fn the_declaration_names_exactly_the_three_prerequisites_section_3_7_lists() {
        let ids: Vec<String> = declaration()
            .prerequisites
            .iter()
            .map(|p| p.id.clone())
            .collect();
        assert_eq!(
            ids,
            [PROVIDER_EXECUTABLE, QUALIFICATION_RECORD, CREDENTIAL_PATH]
        );
    }

    #[test]
    fn there_is_at_most_one_pointer() {
        let pointers = declaration().files.iter().filter(|f| f.pointer).count();
        assert_eq!(pointers, 1);
    }

    #[test]
    fn a_satisfied_adapter_claims_its_paths() {
        assert_eq!(readiness(&declaration(), &satisfied()), Readiness::Claiming);
    }

    #[test]
    fn an_absent_credential_path_refuses_and_names_that_prerequisite() {
        let mut probe = StaticProbe::new().with_harness(HARNESS);
        probe = probe
            .with_prerequisite(HARNESS, PROVIDER_EXECUTABLE)
            .with_prerequisite(HARNESS, QUALIFICATION_RECORD);
        match readiness(&declaration(), &probe) {
            Readiness::Refused { missing } => assert_eq!(missing, [CREDENTIAL_PATH]),
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn this_is_the_first_adapter_so_nothing_collides_yet_and_the_check_still_runs() {
        assert!(collisions(&[declaration()]).is_empty());
    }

    #[test]
    fn the_pointer_imports_the_owned_file_rather_than_copying_it() {
        let text = String::from_utf8(pointer_contents()).unwrap();
        assert_eq!(text.trim(), format!("@{OWNED_INSTRUCTIONS}"));
    }

    #[test]
    fn the_unexpressible_facts_are_stated_rather_than_dropped() {
        let d = declaration();
        assert_eq!(d.unexpressible.len(), 3);
        assert!(d.unexpressible.iter().any(|f| f.contains("not-recorded")));
        assert!(
            d.unexpressible
                .iter()
                .any(|f| f.contains("workspace-write"))
        );
    }
}
