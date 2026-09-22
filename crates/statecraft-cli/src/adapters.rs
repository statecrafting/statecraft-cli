//! The configured adapter set, and the child environment it is probed against.
//!
//! Spec 006 section 3.2 says a command is a binding and never a second
//! implementation. Selecting *which* adapters are configured is not a rule about
//! their behavior: it is the input spec 002's operations take, and until spec 008
//! there was no ratified adapter to supply, which is why these verbs refused.
//!
//! Spec 008 ratifies the first one, so this module names it. Everything about
//! what that adapter declares, which prerequisites it needs and how they are
//! detected lives in the adapter's own crate, where spec 008 owns it. Nothing
//! here decides any of it.
//!
//! # Where the qualification records come from
//!
//! Spec 004 section 3.16 makes a qualification an act performed by an operator
//! against a named provider version and **recorded**. A record is therefore read
//! rather than derived: `<product home>/qualifications.json`, which spec 006
//! section 3.6 permits because the product home is one of the three things the
//! binary reads. An absent or unreadable file is no records, which makes the
//! adapter `unqualified`: spec 004 section 3.4 says such an adapter still runs,
//! and spec 004 section 3.15 says it does not claim a target's paths.

use statecraft_adapter::environment::{Blueprint, CheckSuiteCommands, ChildEnvironment, construct};
use statecraft_adapter_claude_code as provider;
use statecraft_adapter_claude_code::qualification::PairedRecord;
use statecraft_environment::adapter::Declaration;
use std::path::Path;

/// Where a recorded qualification lives under the product home.
pub const QUALIFICATIONS_FILE: &str = "qualifications.json";

/// The ratified adapter set.
///
/// One entry today. A second provider is spec 008 section 4's out of scope, and
/// adding one here without a spec that names it would be this binary ratifying
/// an adapter, which `AGENTS.md` makes the owner's act.
pub fn declarations() -> Vec<Declaration> {
    vec![provider::environment::declaration()]
}

/// The child environment the adapter's prerequisites are probed against.
///
/// **Constructed, not filtered** (spec 004 section 3.6): the child gets exactly
/// what the blueprint allows. Everything else in this process's environment is
/// dropped, including the credential paths spec 004 section 3.6 refuses to place
/// in a child at all, and including `HOME`.
///
/// Two names are allowed, and each is a prerequisite spec 004 section 3.15 names:
///
/// - `PATH`, because an adapter that cannot resolve its own executable has
///   nothing to probe.
/// - `USER`, because spec 004 section 3.14's credential path runs through the
///   operating-system keychain and the lookup is keyed by the account name.
///   Measured: `PATH` alone terminates the provider with section 3.5's
///   `api_error` shape, reading `Not logged in`. This is not a credential and
///   carries none; it is the name under which the operating system answers one.
pub fn child_environment() -> ChildEnvironment {
    child_environment_with(&[])
}

/// The same constructed environment, plus the names a run's attempt binding
/// carries (spec 002 section 3.31 rule 17, spec 004 section 5 of 2026-09-22).
/// The names and values are the library's; this only places them.
pub fn child_environment_with(binding: &[(String, String)]) -> ChildEnvironment {
    let manifest = provider::manifest();
    let mut blueprint = Blueprint::empty();
    for (name, value) in binding {
        blueprint = blueprint.allowing(name, value);
    }
    if let Ok(path) = std::env::var("PATH") {
        blueprint = blueprint.allowing("PATH", &path);
    }
    if let Ok(user) = std::env::var("USER") {
        blueprint = blueprint.allowing("USER", &user);
    }
    for command in &manifest.requires_commands {
        blueprint = blueprint.needing_command(command);
    }
    // The suite's commands are the manifest's own, so a posture that omitted one
    // would be refused here rather than discovered mid-attempt (spec 004
    // section 3.5.8).
    construct(
        &blueprint,
        &CheckSuiteCommands(manifest.requires_commands.clone()),
    )
}

/// Every qualification record recorded under the product home.
pub fn records(home: &Path) -> Vec<PairedRecord> {
    let path = home.join(QUALIFICATIONS_FILE);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// The harness probe the environment operations are given.
///
/// Built from the constructed child environment, so "is the provider resolvable"
/// is asked of what the child would actually get and not of this process.
pub fn probe(home: &Path) -> provider::ConstructedEnvironmentProbe {
    let environment = child_environment();
    let mut probe =
        provider::ConstructedEnvironmentProbe::from_child_environment(&environment.variables);
    if let Some(executable) = probe.resolved_executable() {
        if let Some(version) = provider::observe_provider_version(&executable) {
            probe = probe.observing_provider_version(&version);
        }
    }
    probe.with_records(records(home))
}

/// The pins a first `env apply` records in the manifest.
///
/// Spec 002 section 3.4 makes the manifest carry the product version, the
/// spec-spine version and each adapter's version. The product's is this build's;
/// each adapter's is its own declaration's; spec-spine's is **asked of
/// spec-spine**, because spec 001 section 3.2 forbids this product from
/// answering a specification question itself, and its version is one.
///
/// An unresolvable spec-spine pins `not-recorded`, which is spec 005 section
/// 3.8's word for "no record of this kind exists" rather than a guess or a zero.
pub fn pins() -> statecraft_environment::manifest::Pins {
    statecraft_environment::manifest::Pins {
        product: env!("CARGO_PKG_VERSION").to_string(),
        spec_spine: observed_spec_spine().unwrap_or_else(|| {
            statecraft_envelope::absence::Absence::NotRecorded
                .word()
                .to_string()
        }),
        adapters: declarations()
            .iter()
            .map(|d| (d.name.clone(), d.version.clone()))
            .collect(),
    }
}

/// What is true right now, for `doctor` to compare the pins against.
pub fn observed() -> statecraft_environment::doctor::Observed {
    statecraft_environment::doctor::Observed {
        product: Some(env!("CARGO_PKG_VERSION").to_string()),
        spec_spine: observed_spec_spine(),
    }
}

/// The spec-spine version now on the path, asked of spec-spine.
///
/// `None` when it cannot be asked, which `doctor` reports rather than filling
/// in: a pin compared against an invented version is worse than a pin compared
/// against nothing.
fn observed_spec_spine() -> Option<String> {
    let output = std::process::Command::new("spec-spine")
        .arg("--version")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    // `spec-spine 0.20.0`: the version is the last whitespace-separated token.
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next_back()
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_configured_set_is_the_one_adapter_spec_008_ratifies() {
        let set = declarations();
        assert_eq!(set.len(), 1);
        assert_eq!(set[0].harness, provider::environment::HARNESS);
    }

    #[test]
    fn the_child_environment_is_constructed_and_carries_no_credential() {
        let environment = child_environment();
        for name in statecraft_adapter::environment::WITHHELD_PREFIXES {
            assert!(!environment.variables.contains_key(name));
        }
        // Constructed, so at most the two names the blueprint allowed, and
        // nothing this process happens to be carrying beside them.
        assert!(environment.variables.len() <= 2);
        for name in environment.variables.keys() {
            assert!(name == "PATH" || name == "USER", "unexpected name {name}");
        }
        // Spec 004 section 3.14: the home directory is not among them, so the
        // keychain is reached by the account name and not by a home path.
        assert!(!environment.variables.contains_key("HOME"));
    }

    #[test]
    fn the_account_name_is_carried_with_the_operators_own_value() {
        // Spec 004 section 3.14 measured that the keychain lookup is keyed by the
        // account name, so a placeholder or an empty value is not a substitute.
        let Ok(user) = std::env::var("USER") else {
            return;
        };
        let environment = child_environment();
        assert_eq!(environment.variables.get("USER"), Some(&user));
    }

    #[test]
    fn the_pins_name_every_configured_adapter_and_this_builds_product_version() {
        let pins = pins();
        assert_eq!(pins.product, env!("CARGO_PKG_VERSION"));
        assert_eq!(pins.adapters.len(), declarations().len());
        assert!(pins.adapters.contains_key(provider::environment::HARNESS));
        // Either a version was observed or the absence is the reserved word,
        // never an empty string and never a zero.
        assert!(!pins.spec_spine.is_empty());
    }

    #[test]
    fn what_is_observed_carries_this_builds_product_version() {
        assert_eq!(
            observed().product.as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn an_absent_qualifications_file_is_no_records_and_not_a_failure() {
        let home = tempfile::tempdir().unwrap();
        assert!(records(home.path()).is_empty());
    }

    #[test]
    fn an_unreadable_qualifications_file_is_no_records_rather_than_a_guess() {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(home.path().join(QUALIFICATIONS_FILE), b"{not json").unwrap();
        assert!(records(home.path()).is_empty());
    }

    #[test]
    fn a_recorded_qualification_round_trips_from_the_product_home() {
        let home = tempfile::tempdir().unwrap();
        let record = provider::record("2.1.267", "008.3.9", "2026-09-17T00:00:00Z");
        std::fs::write(
            home.path().join(QUALIFICATIONS_FILE),
            serde_json::to_vec(std::slice::from_ref(&record)).unwrap(),
        )
        .unwrap();
        assert_eq!(records(home.path()), [record]);
    }
}
