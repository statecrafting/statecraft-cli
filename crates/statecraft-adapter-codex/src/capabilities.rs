//! What this adapter declares: its manifest and its spec 047 declaration.
//!
//! Spec 055 section 3.6: the declaration is measured, not copied from the
//! first adapter's, and a field nobody has measured is declared `false`.
//! Nothing below has a recorded measurement yet (the fixtures are synthetic),
//! so every token is absent and every field is `false`:
//!
//! | Field | Declared | Why |
//! |---|---|---|
//! | `stream_witness` | `false` | Spec 013 names only the first adapter's authority (section 4). |
//! | `print_mode_hooks` | `false` | `PreToolUse` in `codex exec` is unmeasured; the hook query is not a run. |
//! | `wire_sidecar` | `false` | Nobody routed Codex through a reserved endpoint. |
//! | `per_run_config_dir` | `false` | [`crate::home`] builds one; no capture shows Codex honouring it. |
//! | `settings_file_transport` | `false` | Codex reads `CODEX_HOME`, not a settings path on the command line. |
//! | `version_field` | none | `thread.started` carries no version. |
//!
//! Spec 047 makes `run` require the fourth and fifth on every launch, so `run`
//! refuses this adapter with `capability-undeclared` until a measurement
//! flips them. That is the conservative answer, and it is the spec's.

use statecraft_adapter::capabilities::AdapterCapabilities;
use statecraft_adapter::capability::Capability;
use statecraft_adapter::manifest::Manifest;

/// This adapter's name. Not a harness name and not a binary name.
pub const ADAPTER_NAME: &str = "codex";

/// The provider version section 3.1 measured the invocation against.
pub const MEASURED_PROVIDER_VERSION: &str = "0.163.0";

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
        per_run_config_dir: false,
        settings_file_transport: false,
        version_field: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use statecraft_adapter::capabilities::{Field, register};

    #[test]
    fn the_declaration_sets_every_field_and_declares_nothing_unmeasured() {
        let value = serde_json::to_value(declared()).unwrap();
        assert_eq!(register(&value), Ok(declared()));
        assert!(Field::ALL.iter().all(|f| !declared().declares(*f)));
    }

    #[test]
    fn no_token_is_declared_without_a_measurement() {
        assert!(supported().is_empty());
        assert_eq!(manifest().adapter, ADAPTER_NAME);
        assert!(manifest().requires_commands.contains(&"codex".to_string()));
    }
}
