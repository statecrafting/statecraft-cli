//! The governance producer boundary.
//!
//! Spec 002 section 3.15. Governance starter files come from the spec-spine
//! **library** and from nowhere else:
//!
//! `spec_spine_core::scaffold_init_json(config_json) -> Result<String, Error>`
//!
//! It returns the existing serialized `Scaffold` files-as-data shape with the
//! existing `ScaffoldFile` fields, performs no write and discovers no
//! environment. This product owns the reconciliation and every filesystem
//! write. The removed `spec-spine init` command is not invoked, no governance
//! template is vendored here, and there is no fallback installer built from old
//! kit bytes.
//!
//! # The contract set is closed
//!
//! Five kinds of path are placed. Anything else the producer returns is **out
//! of contract**: not written, named in the report, and recorded as the
//! producer being non-conforming for that run. That is a finding about the
//! producer, not a failure of the project, and every in-contract path still
//! reconciles.
//!
//! # The wire shape is read with a mirror, not with the producer's own type
//!
//! Deserializing into a local struct without `deny_unknown_fields` is what lets
//! the producer add a field without breaking this consumer, and what keeps the
//! fields this product depends on written down in one place.

use serde::{Deserialize, Serialize};

/// The producer crate this build depends on.
pub const PRODUCER_NAME: &str = "spec-spine-core";

/// The exact version of the linked library, as `Cargo.lock` records it.
///
/// Derived from the build (`build.rs`), never a second literal: `D-06` as
/// amended on 2026-09-24 and spec 002 section 5's provenance entry of the same
/// date. A test still reads the manifest and refuses a drift between the pin
/// and what is linked. A pin recorded in a report that is not the pin in the
/// build is worse than no pin at all.
pub const PRODUCER_VERSION: &str = env!("STATECRAFT_PRODUCER_VERSION");

/// The crates.io checksum `Cargo.lock` records for the linked library.
pub const PRODUCER_CHECKSUM: &str = env!("STATECRAFT_PRODUCER_CHECKSUM");

/// The linked producer as the declaration's pins record it (spec 002 section
/// 5, 2026-09-24, provenance item 3): one identity, fixed by this build.
pub fn linked() -> statecraft_environment::manifest::Producer {
    statecraft_environment::manifest::Producer {
        name: PRODUCER_NAME.to_string(),
        version: PRODUCER_VERSION.to_string(),
        checksum: PRODUCER_CHECKSUM.to_string(),
    }
}

/// The specs directory this product declares.
pub const SPECS_DIR: &str = "specs";
/// The standards directory this product declares.
pub const STANDARDS_DIR: &str = "standards/spec";
/// The derived directory this product declares (spec 002 section 3.12).
pub const DERIVED_DIR: &str = ".statecraft/derived";
/// The state directory this product declares.
pub const STATE_DIR: &str = ".statecraft/state";

/// The configuration this product passes, always explicitly.
///
/// Never defaulted: a producer asked with `{}` would answer for `.derived`,
/// and the whole point of section 3.2 is that the compiled artifacts live in
/// the project area.
pub fn config_json() -> String {
    serde_json::json!({
        "layout": {
            "specs_dir": SPECS_DIR,
            "derived_dir": DERIVED_DIR,
            "standards_dir": STANDARDS_DIR,
            "schemas_dir": "standards/schemas",
            "cargo_workspace": "Cargo.toml",
            "npm_workspaces": ["package.json", "pnpm-workspace.yaml"],
            "standalone_rust_workspaces": [],
            "standalone_npm_packages": [],
            "state_dir": STATE_DIR
        },
        "index": {
            "resolver_exclusions": resolver_exclusions()
        }
    })
    .to_string()
}

/// Whether a governance path is an authored input (spec 002 section 5,
/// 2026-09-24, provenance item 1). The list is closed and stated, never
/// inferred from bytes: the configuration, the bootstrap spec, and the
/// constitution and contract. Everything else this product writes is a
/// reference.
pub fn is_authored_input(rel_path: &str) -> bool {
    rel_path == "spec-spine.toml"
        || rel_path == format!("{SPECS_DIR}/000-bootstrap/spec.md")
        || rel_path == format!("{STANDARDS_DIR}/constitution.md")
        || rel_path == format!("{STANDARDS_DIR}/contract.md")
}

/// Build directories no project's resolver should walk.
pub const BUILD_DIRS: [&str; 5] = ["target", "node_modules", "dist", "build", ".next"];
/// The repository-local tool directory (`make tools` installs into it).
pub const TOOL_DIR: &str = ".tooling";

/// `[index] resolver_exclusions`, derived from the declared layout (spec 002
/// section 5, 2026-09-24, provenance item 5): the derived and state roots this
/// product declares, the build directories, and the local tool directory. It
/// names no other derived directory, so a producer default that spells one is
/// never what a project receives.
pub fn resolver_exclusions() -> Vec<&'static str> {
    let mut out = BUILD_DIRS.to_vec();
    out.extend([DERIVED_DIR, STATE_DIR, TOOL_DIR]);
    out
}

/// One file the producer returned.
///
/// The mirror of `ScaffoldFile`. Unknown fields are ignored on purpose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaffoldFile {
    /// Repository-relative path.
    pub rel_path: String,
    /// The contents.
    pub contents: String,
    /// Whether the producer would overwrite an existing file. This product
    /// never does, whatever this says: reconciliation is spec 002's ownership
    /// model, and an existing file is adopted.
    #[serde(default)]
    pub overwrite: bool,
    /// Whether the file must arrive executable.
    #[serde(default)]
    pub executable: bool,
    /// Whether the producer intends the contents to be appended.
    #[serde(default)]
    pub append: bool,
    /// The substring whose presence means an appended block is already there.
    #[serde(default)]
    pub append_marker: Option<String>,
}

/// The producer's whole answer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Scaffold {
    files: Vec<ScaffoldFile>,
}

/// Where a returned path belongs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Placement {
    /// A governance starter file this product places.
    Governance,
    /// The ignore fragment, merged rather than installed.
    IgnoreFragment,
    /// Outside the contract set. Not written.
    OutOfContract,
}

/// Where a returned path belongs, by the closed contract set of section 3.5.
pub fn classify(rel_path: &str) -> Placement {
    if rel_path == "spec-spine.toml" {
        return Placement::Governance;
    }
    if rel_path == ".gitignore" {
        return Placement::IgnoreFragment;
    }
    if rel_path == format!("{STANDARDS_DIR}/constitution.md")
        || rel_path == format!("{STANDARDS_DIR}/contract.md")
        || rel_path.starts_with(&format!("{STANDARDS_DIR}/templates/"))
    {
        return Placement::Governance;
    }
    if rel_path == format!("{SPECS_DIR}/000-bootstrap/spec.md") {
        return Placement::Governance;
    }
    Placement::OutOfContract
}

/// Who answered, and as what version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    /// The producer's name.
    pub name: String,
    /// Its exact version.
    pub version: String,
}

impl Identity {
    /// A one-line rendering.
    pub fn describe(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }
}

/// Whether the producer stayed inside the contract set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Conformance {
    /// Who answered.
    pub producer: Identity,
    /// True when every returned path is in the contract set.
    pub conforming: bool,
    /// Every returned path outside it, in the order returned.
    pub out_of_contract: Vec<String>,
}

impl Conformance {
    /// A one-line rendering for a report.
    pub fn describe(&self) -> String {
        if self.conforming {
            format!("producer {} conforming", self.producer.describe())
        } else {
            format!(
                "producer {} non-conforming: {} path(s) outside the contract set ({})",
                self.producer.describe(),
                self.out_of_contract.len(),
                self.out_of_contract.join(", ")
            )
        }
    }
}

/// What went wrong asking the producer.
#[derive(Debug, thiserror::Error)]
pub enum ProducerError {
    /// The producer refused.
    #[error("the governance producer refused: {0}")]
    Refused(String),
    /// The producer answered with something this consumer cannot read.
    #[error("the governance producer's answer is unreadable: {0}")]
    Unreadable(String),
}

/// A source of governance starter files.
///
/// A trait so the call site is one line and the boundary is one type. There is
/// exactly one implementation that talks to spec-spine, and it is
/// [`Library`].
pub trait Producer {
    /// The producer's files-as-data answer, as JSON.
    fn scaffold(&self, config_json: &str) -> Result<String, ProducerError>;
    /// Who this is.
    fn identity(&self) -> Identity;
}

/// The real producer: the spec-spine library, called in process.
#[derive(Debug, Clone, Copy, Default)]
pub struct Library;

impl Producer for Library {
    fn scaffold(&self, config_json: &str) -> Result<String, ProducerError> {
        spec_spine_core::scaffold_init_json(config_json)
            .map_err(|e| ProducerError::Refused(format!("{e:?}")))
    }

    fn identity(&self) -> Identity {
        Identity {
            name: PRODUCER_NAME.to_string(),
            version: PRODUCER_VERSION.to_string(),
        }
    }
}

/// A producer backed by a recorded answer.
///
/// For tests, and for a caller that already has one. **Never evidence of an
/// integration**: an answer replayed from a string establishes what this
/// consumer does with a shape, and nothing about what the library returns.
#[derive(Debug, Clone)]
pub struct Recorded {
    /// The JSON to return.
    pub json: String,
    /// What to call it.
    pub identity: Identity,
}

impl Producer for Recorded {
    fn scaffold(&self, _config_json: &str) -> Result<String, ProducerError> {
        Ok(self.json.clone())
    }
    fn identity(&self) -> Identity {
        self.identity.clone()
    }
}

/// The producer's answer, sorted into what this product does with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Starter {
    /// Governance files to reconcile, in the order returned.
    pub governance: Vec<ScaffoldFile>,
    /// The ignore fragment, when the producer returned one.
    pub ignore_fragment: Option<String>,
    /// Files outside the contract set. Named, never written.
    pub out_of_contract: Vec<ScaffoldFile>,
    /// Whether the producer stayed inside the contract set.
    pub conformance: Conformance,
}

/// Ask a producer and sort its answer. Writes nothing.
pub fn produce(producer: &dyn Producer) -> Result<Starter, ProducerError> {
    let json = producer.scaffold(&config_json())?;
    let scaffold: Scaffold =
        serde_json::from_str(&json).map_err(|e| ProducerError::Unreadable(e.to_string()))?;

    let mut governance = Vec::new();
    let mut ignore_fragment = None;
    let mut out_of_contract = Vec::new();
    for file in scaffold.files {
        match classify(&file.rel_path) {
            Placement::Governance => governance.push(file),
            Placement::IgnoreFragment => ignore_fragment = Some(file.contents),
            Placement::OutOfContract => out_of_contract.push(file),
        }
    }

    let conformance = Conformance {
        producer: producer.identity(),
        conforming: out_of_contract.is_empty(),
        out_of_contract: out_of_contract.iter().map(|f| f.rel_path.clone()).collect(),
    };
    Ok(Starter {
        governance,
        ignore_fragment,
        out_of_contract,
        conformance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact version in a `version = "=X"` or `required_version = "=X"` line.
    fn exact_pin(text: &str, key_line_prefix: &str, what: &str) -> String {
        let line = text
            .lines()
            .find(|l| l.starts_with(key_line_prefix))
            .unwrap_or_else(|| panic!("{what} is declared"));
        line.split("\"=")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .unwrap_or_else(|| panic!("{what} is pinned exactly: {line}"))
            .to_string()
    }

    #[test]
    fn the_recorded_version_is_the_version_the_workspace_pins() {
        // The root manifest states the version once (spec 001 section 3.13);
        // the member inherits it rather than restating it.
        let manifest = include_str!("../../../Cargo.toml");
        let pinned = exact_pin(manifest, "spec-spine-core", "the producer dependency");
        assert_eq!(pinned, PRODUCER_VERSION);
        let member = include_str!("../Cargo.toml");
        assert!(
            member
                .lines()
                .any(|l| l == "spec-spine-core.workspace = true"),
            "the member inherits the workspace's statement instead of restating it"
        );
    }

    #[test]
    fn the_cli_pin_and_the_linked_library_are_one_release() {
        // One producer identity (H-3 (a), spec 001 section 3.13 rule 3): the
        // governing CLI and the linked library name the same release.
        let config = include_str!("../../../spec-spine.toml");
        let cli = exact_pin(config, "required_version", "the CLI pin");
        assert_eq!(cli, PRODUCER_VERSION);
    }

    #[test]
    fn the_linked_identity_is_what_the_lock_file_records() {
        let lock = include_str!("../../../Cargo.lock");
        let block = lock
            .split("[[package]]")
            .find(|b| b.contains(&format!("name = \"{PRODUCER_NAME}\"")))
            .expect("the producer is in the lock file");
        assert!(block.contains(&format!("version = \"{PRODUCER_VERSION}\"")));
        assert!(block.contains(&format!("checksum = \"{PRODUCER_CHECKSUM}\"")));
        assert_eq!(PRODUCER_CHECKSUM.len(), 64);
        let p = linked();
        assert_eq!(
            (p.name.as_str(), p.version.as_str(), p.checksum.as_str()),
            (PRODUCER_NAME, PRODUCER_VERSION, PRODUCER_CHECKSUM)
        );
    }

    #[test]
    fn the_configuration_is_explicit_about_every_layout_key() {
        let cfg: serde_json::Value = serde_json::from_str(&config_json()).unwrap();
        let layout = &cfg["layout"];
        assert_eq!(layout["specs_dir"], SPECS_DIR);
        assert_eq!(layout["standards_dir"], STANDARDS_DIR);
        assert_eq!(layout["derived_dir"], DERIVED_DIR);
        assert_eq!(layout["state_dir"], STATE_DIR);
        // Never the old location, which is the whole reason the layout is
        // passed rather than defaulted.
        assert_ne!(layout["derived_dir"], ".derived");
    }

    #[test]
    fn the_contract_set_is_closed() {
        for path in [
            "spec-spine.toml",
            "standards/spec/constitution.md",
            "standards/spec/contract.md",
            "standards/spec/templates/spec-template.md",
            "standards/spec/templates/constitution-template.md",
            "specs/000-bootstrap/spec.md",
        ] {
            assert_eq!(classify(path), Placement::Governance, "{path}");
        }
        assert_eq!(classify(".gitignore"), Placement::IgnoreFragment);
        for path in [
            "AGENTS.md",
            "CLAUDE.md",
            ".claude/rules/orchestrator-rules.md",
            ".github/workflows/govern.yml",
            "Makefile",
            ".gitattributes",
            ".githooks/enable-merge-driver.sh",
        ] {
            assert_eq!(classify(path), Placement::OutOfContract, "{path}");
        }
    }

    fn recorded(files: serde_json::Value) -> Recorded {
        Recorded {
            json: serde_json::json!({ "files": files }).to_string(),
            identity: Identity {
                name: "recorded".into(),
                version: "0".into(),
            },
        }
    }

    #[test]
    fn a_conforming_answer_reports_conforming_and_places_everything() {
        let p = recorded(serde_json::json!([
            { "relPath": "spec-spine.toml", "contents": "x" },
            { "relPath": ".gitignore", "contents": "y" },
        ]));
        let s = produce(&p).unwrap();
        assert!(s.conformance.conforming);
        assert_eq!(s.governance.len(), 1);
        assert_eq!(s.ignore_fragment.as_deref(), Some("y"));
        assert!(s.out_of_contract.is_empty());
    }

    #[test]
    fn an_out_of_contract_path_is_named_and_never_placed() {
        let p = recorded(serde_json::json!([
            { "relPath": "spec-spine.toml", "contents": "x" },
            { "relPath": "AGENTS.md", "contents": "generated" },
            { "relPath": ".claude/rules/orchestrator-rules.md", "contents": "r" },
        ]));
        let s = produce(&p).unwrap();
        assert!(!s.conformance.conforming);
        assert_eq!(
            s.conformance.out_of_contract,
            ["AGENTS.md", ".claude/rules/orchestrator-rules.md"]
        );
        assert_eq!(s.governance.len(), 1);
        assert!(s.conformance.describe().contains("non-conforming"));
    }

    #[test]
    fn an_unknown_field_in_the_wire_shape_does_not_break_the_read() {
        let p = recorded(serde_json::json!([
            { "relPath": "spec-spine.toml", "contents": "x", "somethingNew": 7 },
        ]));
        assert!(produce(&p).is_ok());
    }

    #[test]
    fn an_unreadable_answer_is_an_error_and_not_an_empty_scaffold() {
        let p = Recorded {
            json: "{not json".into(),
            identity: Identity {
                name: "recorded".into(),
                version: "0".into(),
            },
        };
        assert!(matches!(produce(&p), Err(ProducerError::Unreadable(_))));
    }

    #[test]
    fn the_real_library_answers_for_the_declared_layout() {
        // The actual boundary, called in process. What it returns is asserted
        // in `tests/producer_integration.rs`; this only holds that the call
        // itself is wired and that the layout reaches it.
        let s = produce(&Library).expect("the library answers");
        let toml = s
            .governance
            .iter()
            .find(|f| f.rel_path == "spec-spine.toml")
            .expect("the configuration is a governance file");
        assert!(toml.contents.contains(DERIVED_DIR));
        assert!(toml.contents.contains(STATE_DIR));
    }
}
