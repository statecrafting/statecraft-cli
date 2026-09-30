//! The bootstrap input document: spec 018 sections 3.1, 3.2 and 3.5.
//!
//! A fresh target has no declaration to read setup parameters from, so an
//! operator supplies them in one versioned, read-only document named with
//! `--setup-input <file>`. This module reads it, refuses anything but its
//! closed shape, and states where every effective parameter came from. The
//! parameters themselves are validated by the selected profile's own closed
//! validator ([`crate::setup::parameters`]); nothing here restates a rule.
//!
//! The document is never copied into the target, recorded in the manifest or
//! kept in runtime state: its bytes are bound into the plan identity by
//! digest, and an apply recomputes the plan from the same path.

use crate::setup::Parameters;
use serde::Serialize;
use statecraft_environment::digest::digest_bytes;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The one schema this build reads.
pub const SCHEMA: &str = "statecraft/setup-input/1";

/// The members a version-1 document has, all required.
const MEMBERS: [&str; 3] = ["schema", "profile", "parameters"];

/// A document as read: its profile, its parameters, and its bytes' digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// Where it was read from, as named.
    pub path: String,
    /// The profile it selects.
    pub profile: String,
    /// Its parameters, unvalidated: the profile validates them.
    pub parameters: BTreeMap<String, serde_json::Value>,
    /// The SHA-256 of its exact bytes.
    pub digest: String,
}

/// Read and check a document. `Err` is a planning refusal, and never echoes
/// a member's value: an unknown member may be a credential.
pub fn read(path: &Path) -> Result<Document, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("setup input {}: {e}", path.display()))?;
    parse(&bytes, &path.display().to_string())
}

/// Check a document's bytes.
pub fn parse(bytes: &[u8], path: &str) -> Result<Document, String> {
    let refuse = |why: String| format!("setup input {path}: {why}");
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| refuse(format!("not a JSON document ({e})")))?;
    let serde_json::Value::Object(map) = value else {
        return Err(refuse("the document must be a JSON object".to_string()));
    };
    let unknown: Vec<&str> = map
        .keys()
        .map(String::as_str)
        .filter(|k| !MEMBERS.contains(k))
        .collect();
    if !unknown.is_empty() {
        return Err(refuse(format!(
            "unknown member(s) {}; a setup input carries `schema`, `profile` and `parameters` only, and never a credential, token, secret value, remote observation, consent or plan approval (values not shown)",
            unknown
                .iter()
                .map(|k| format!("`{k}`"))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    for member in MEMBERS {
        if !map.contains_key(member) {
            return Err(refuse(format!("the required member `{member}` is absent")));
        }
    }
    if map["schema"].as_str() != Some(SCHEMA) {
        return Err(refuse(format!(
            "unknown schema; this build reads `{SCHEMA}` only"
        )));
    }
    let Some(profile) = map["profile"].as_str().filter(|p| !p.trim().is_empty()) else {
        return Err(refuse("`profile` must be a profile id".to_string()));
    };
    let serde_json::Value::Object(parameters) = &map["parameters"] else {
        return Err(refuse("`parameters` must be a JSON object".to_string()));
    };
    Ok(Document {
        path: path.to_string(),
        profile: profile.to_string(),
        parameters: parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        digest: digest_bytes(bytes),
    })
}

/// Section 3.1: `--profile` and the document's profile disagree. `Some` is
/// the usage refusal's message; a document that cannot be read is left for
/// the plan to refuse, with its own reason.
pub fn profile_conflict(path: &Path, named: &str) -> Option<String> {
    let doc = read(path).ok()?;
    (doc.profile != named).then(|| {
        format!(
            "--profile {named} and the setup input's profile {} disagree; neither is selected",
            doc.profile
        )
    })
}

/// What the plan reports about its input (section 3.2): the schema and the
/// digest of a supplied document, or that none was supplied. Never the
/// document's bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputReport {
    /// Whether a document was supplied.
    pub supplied: bool,
    /// Its schema.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// The SHA-256 of its bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    /// Where it was read from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// What that means for the parameters, for a person.
    pub detail: String,
}

impl InputReport {
    /// No document: defaults are in effect, and nobody chose them.
    pub fn absent() -> Self {
        Self {
            supplied: false,
            schema: None,
            digest: None,
            path: None,
            detail: "no setup input was supplied, so there is no pre-plan parameter source: a parameter the declaration does not record is the profile's default, not an operator's choice".to_string(),
        }
    }

    /// A supplied document.
    pub fn of(doc: &Document) -> Self {
        Self {
            supplied: true,
            schema: Some(SCHEMA.to_string()),
            digest: Some(doc.digest.clone()),
            path: Some(doc.path.clone()),
            detail: "parameters named by the setup input are its; the rest are recorded or the profile's defaults".to_string(),
        }
    }

    /// The line the plan identity commits to (section 3.5 item 1): the schema
    /// and exact digest, or explicit absence. The path is not bound: the same
    /// bytes read from another place are the same input.
    pub fn identity_line(&self) -> String {
        match (&self.schema, &self.digest) {
            (Some(schema), Some(digest)) if self.supplied => {
                format!("input {schema} {digest}\n")
            }
            _ => "input absent\n".to_string(),
        }
    }
}

/// Where an effective parameter's value came from (section 3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provenance {
    /// Named by the supplied setup input.
    SetupInput,
    /// Recorded in the declaration's setup selection, or carried forward by
    /// a managed upgrade of it.
    Recorded,
    /// The profile's default: nobody chose it.
    Default,
}

impl Provenance {
    /// A one-word rendering.
    pub fn word(self) -> &'static str {
        match self {
            Provenance::SetupInput => "setup-input",
            Provenance::Recorded => "recorded",
            Provenance::Default => "default",
        }
    }
}

/// One effective parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Effective {
    /// The parameter's name, as a declaration or an input spells it.
    pub name: String,
    /// Its effective value; `null` for an unset optional one.
    pub value: serde_json::Value,
    /// Where the value came from.
    pub provenance: Provenance,
}

/// Every parameter the profile knows, by name, with its effective value.
/// The same names [`crate::setup::parameters`] accepts; a test holds the two
/// to each other.
pub fn keyed(p: &Parameters) -> BTreeMap<&'static str, serde_json::Value> {
    use serde_json::json;
    BTreeMap::from([
        ("default_branch", json!(p.default_branch)),
        ("review.diff_cap", json!(p.diff_cap)),
        ("review.context_tokens", json!(p.context_tokens)),
        ("review.max_calls", json!(p.max_calls)),
        ("review.deletion_cap", json!(p.deletion_cap)),
        ("review.exclude", json!(p.exclude)),
        ("release.branch_pattern", json!(p.release_branch_pattern)),
        ("review.code_owners", json!(p.code_owners)),
        ("governance.enforce_coverage", json!(p.enforce_coverage)),
        ("governance.authored_content", json!(p.authored_content)),
        (
            "governance.authored_content_text",
            json!(p.authored_content_text),
        ),
        ("governance.gate_each_commit", json!(p.gate_each_commit)),
        (
            "governance.require_signed_commits",
            json!(p.require_signed_commits),
        ),
        (
            "governance.require_default_base",
            json!(p.require_default_base),
        ),
        ("governance.fail_on_unresolved", json!(p.fail_on_unresolved)),
        ("governance.require_ratified", json!(p.require_ratified)),
        (
            "ci.extra_required_jobs",
            json!(
                p.extra_required_jobs
                    .iter()
                    .map(|e| json!({ "job": e.job, "workflow": e.workflow }))
                    .collect::<Vec<_>>()
            ),
        ),
    ])
}

/// Every effective parameter with its provenance: named by the input, else
/// present in the declared block the plan used, else the default.
pub fn effective(
    params: &Parameters,
    supplied: &BTreeSet<String>,
    block: &BTreeMap<String, serde_json::Value>,
) -> Vec<Effective> {
    keyed(params)
        .into_iter()
        .map(|(name, value)| Effective {
            name: name.to_string(),
            value,
            provenance: if supplied.contains(name) {
                Provenance::SetupInput
            } else if block.contains_key(name) {
                Provenance::Recorded
            } else {
                Provenance::Default
            },
        })
        .collect()
}

/// What the flow binds into a setup plan beyond the profile's own reads:
/// spec 018's input, the parameters it named, and the linked producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    /// The input, or its absence.
    pub input: InputReport,
    /// The parameter names the input supplied.
    pub supplied: BTreeSet<String>,
    /// The exact version of the producer that scaffolds a new
    /// `spec-spine.toml`, which an adopted exact pin must also admit
    /// (section 3.3).
    pub producer_version: String,
    /// Further facts the plan identity commits to (section 3.5 item 3): the
    /// producer's identity and the digest of its pinned scaffold.
    pub facts: Vec<String>,
}

impl Bound {
    /// No input, the linked producer, and no further facts: what a caller
    /// planning a recorded selection outside initialization binds.
    pub fn recorded() -> Self {
        Self {
            input: InputReport::absent(),
            supplied: BTreeSet::new(),
            producer_version: crate::producer::PRODUCER_VERSION.to_string(),
            facts: Vec::new(),
        }
    }

    /// As [`Bound::recorded`], with the producer version an adopted pin must
    /// admit stated.
    pub fn with_producer(version: &str) -> Self {
        Self {
            producer_version: version.to_string(),
            ..Self::recorded()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(v: serde_json::Value) -> Result<Document, String> {
        parse(&serde_json::to_vec(&v).unwrap(), "input.json")
    }

    #[test]
    fn a_version_one_document_is_read_with_its_digest() {
        let bytes = br#"{"schema":"statecraft/setup-input/1","profile":"github-actions-rust","parameters":{"review.code_owners":["@owner"]}}"#;
        let d = parse(bytes, "i.json").unwrap();
        assert_eq!(d.profile, "github-actions-rust");
        assert_eq!(
            d.parameters["review.code_owners"],
            serde_json::json!(["@owner"])
        );
        assert_eq!(d.digest, digest_bytes(bytes));
    }

    #[test]
    fn the_shape_is_closed_and_a_value_is_never_echoed() {
        let secret = "ghp_not-a-real-token-0123456789";
        let e = doc(serde_json::json!({
            "schema": SCHEMA, "profile": "p", "parameters": {}, "token": secret
        }))
        .unwrap_err();
        assert!(e.contains("`token`"), "{e}");
        assert!(!e.contains(secret), "the value is not echoed: {e}");
        for (v, needle) in [
            (
                serde_json::json!({"profile": "p", "parameters": {}}),
                "`schema`",
            ),
            (
                serde_json::json!({"schema": SCHEMA, "parameters": {}}),
                "`profile`",
            ),
            (
                serde_json::json!({"schema": SCHEMA, "profile": "p"}),
                "`parameters`",
            ),
            (
                serde_json::json!({"schema": "statecraft/setup-input/2", "profile": "p", "parameters": {}}),
                "unknown schema",
            ),
            (
                serde_json::json!({"schema": SCHEMA, "profile": "p", "parameters": []}),
                "must be a JSON object",
            ),
            (
                serde_json::json!(["not", "an", "object"]),
                "must be a JSON object",
            ),
        ] {
            let e = doc(v).unwrap_err();
            assert!(e.contains(needle), "{needle}: {e}");
        }
        assert!(parse(b"{not json", "i.json").is_err());
    }

    #[test]
    fn the_identity_line_binds_the_bytes_or_their_absence() {
        assert_eq!(InputReport::absent().identity_line(), "input absent\n");
        let d =
            doc(serde_json::json!({"schema": SCHEMA, "profile": "p", "parameters": {}})).unwrap();
        assert_eq!(
            InputReport::of(&d).identity_line(),
            format!("input {SCHEMA} {}\n", d.digest)
        );
    }

    /// `keyed` names exactly what the profile's validator accepts: each
    /// name, fed back its own effective value, validates to the same value.
    #[test]
    fn every_keyed_name_is_a_parameter_the_profile_accepts() {
        let dir = tempfile::tempdir().unwrap();
        let empty = BTreeMap::new();
        let defaults = crate::setup::parameters(dir.path(), &empty, ".statecraft/derived").unwrap();
        for (name, value) in keyed(&defaults) {
            if value.is_null() {
                continue;
            }
            let block = BTreeMap::from([(name.to_string(), value.clone())]);
            let p = crate::setup::parameters(dir.path(), &block, ".statecraft/derived")
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(keyed(&p)[name], value, "{name}");
        }
        assert!(
            crate::setup::parameters(
                dir.path(),
                &BTreeMap::from([("credential".to_string(), serde_json::json!("x"))]),
                ".statecraft/derived"
            )
            .is_err(),
            "the validator is closed"
        );
    }

    #[test]
    fn provenance_is_input_then_recorded_then_default() {
        let dir = tempfile::tempdir().unwrap();
        let block = BTreeMap::from([
            ("review.code_owners".to_string(), serde_json::json!(["@a"])),
            ("review.max_calls".to_string(), serde_json::json!(2)),
        ]);
        let p = crate::setup::parameters(dir.path(), &block, ".statecraft/derived").unwrap();
        let supplied = BTreeSet::from(["review.code_owners".to_string()]);
        let e = effective(&p, &supplied, &block);
        let of = |n: &str| e.iter().find(|x| x.name == n).unwrap().provenance;
        assert_eq!(of("review.code_owners"), Provenance::SetupInput);
        assert_eq!(of("review.max_calls"), Provenance::Recorded);
        assert_eq!(of("governance.fail_on_unresolved"), Provenance::Default);
        assert_eq!(e.len(), keyed(&p).len(), "every parameter is reported");
    }
}
