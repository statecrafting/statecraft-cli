//! What this adapter declares: its manifest and its spec 047 declaration.
//!
//! Spec 055 section 3.6: the declaration is measured, not copied from the
//! first adapter's, and a field nobody has measured is declared `false`.
//! The 2026-11-08 probes (`testdata/probe/`) measure the fields; the
//! 2026-11-02 recordings measure the stream and the hook listing. No token is
//! measured, so every token is still absent:
//!
//! | Field | Declared | Why |
//! |---|---|---|
//! | `stream_witness` | `false` | Spec 013 names only the first adapter's authority (section 4). |
//! | `print_mode_hooks` | `false` | `codex exec` runs a `PreToolUse` hook and honours its block, but its stream reports no event for the blocked action (`hook-block.jsonl`). |
//! | `wire_sidecar` | `false` | Nobody routed Codex through a reserved endpoint. |
//! | `per_run_config_dir` | `true` | A turn on a per-run `CODEX_HOME` completes and writes nothing to the user's `~/.codex` (`per-run-home.jsonl`). |
//! | `settings_file_transport` | `true` | Codex reads the `config.toml` the supervisor writes at a path it chooses (`strict-config.txt`). |
//! | `version_field` | none | `thread.started` carries no version. |
//!
//! Spec 047 makes `run` require `print_mode_hooks` on a managed launch, so a
//! managed `run` still refuses this adapter with `capability-undeclared`.

use statecraft_adapter::capabilities::AdapterCapabilities;
use statecraft_adapter::capability::Capability;
use statecraft_adapter::manifest::Manifest;

/// This adapter's name. Not a harness name and not a binary name.
pub const ADAPTER_NAME: &str = "codex";

/// The provider version section 3.1 measured the invocation against.
pub const MEASURED_PROVIDER_VERSION: &str = "0.166.0";

/// The commands the constructed environment must carry (spec 004 section
/// 3.5.8): `codex` is this adapter's own; `git` is spec 003's workspace.
pub const REQUIRES_COMMANDS: [&str; 2] = ["codex", "git"];

/// The tokens this adapter declares: none, because none is measured. Spec
/// 004 section 3.5 case 6 makes a declared-but-unapplied token a
/// qualification failure, so an unmeasured one stays out.
pub fn supported() -> Vec<Capability> {
    Vec::new()
}

/// This adapter's manifest, as the seam reads it.
pub fn manifest() -> Manifest {
    Manifest {
        adapter: ADAPTER_NAME.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        supports: supported(),
        requires_commands: REQUIRES_COMMANDS.iter().map(|c| (*c).to_string()).collect(),
    }
}

/// This adapter's spec 047 declaration. A struct literal, so a field left out
/// does not compile.
pub fn declared() -> AdapterCapabilities {
    AdapterCapabilities {
        stream_witness: false,
        print_mode_hooks: false,
        wire_sidecar: false,
        per_run_config_dir: true,
        settings_file_transport: true,
        version_field: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use statecraft_adapter::capabilities::{Field, register};

    #[test]
    fn the_declaration_sets_every_field_and_declares_only_what_a_probe_measured() {
        let value = serde_json::to_value(declared()).unwrap();
        assert_eq!(register(&value), Ok(declared()));
        let declared_fields: Vec<Field> = Field::ALL
            .iter()
            .copied()
            .filter(|f| declared().declares(*f))
            .collect();
        assert_eq!(
            declared_fields,
            [Field::PerRunConfigDir, Field::SettingsFileTransport]
        );
    }

    fn probe(name: &str) -> String {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/probe/");
        std::fs::read_to_string(format!("{path}{name}")).unwrap()
    }

    #[test]
    fn the_recorded_probes_support_each_declared_value() {
        // settings_file_transport: the supplied document is read and named.
        assert!(probe("strict-config.txt").contains("/scratch/codex-home/config.toml"));
        // per_run_config_dir: the turn completes on the per-run directory.
        let per_run = probe("per-run-home.jsonl");
        assert!(
            per_run
                .lines()
                .any(|l| l.contains(r#""type":"turn.completed""#))
        );
        // print_mode_hooks: the hook ran and blocked, and the stream has no
        // event naming the block.
        assert!(probe("hook-block.stderr.txt").contains("blocked by PreToolUse hook"));
        assert!(probe("gate-input.json").contains(r#""hook_event_name":"PreToolUse""#));
        let hook = probe("hook-block.jsonl");
        assert!(!hook.contains("PreToolUse") && !hook.contains("statecraft probe gate"));
    }

    #[test]
    fn no_token_is_declared_without_a_measurement() {
        assert!(supported().is_empty());
        assert_eq!(manifest().adapter, ADAPTER_NAME);
        assert!(manifest().requires_commands.contains(&"codex".to_string()));
    }
}
