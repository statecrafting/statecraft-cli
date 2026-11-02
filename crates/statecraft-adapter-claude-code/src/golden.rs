//! Version-pinned golden captures, the quarantine and the divergence ledger.
//!
//! Spec 052. The streams under `testdata/stream/` are the measurement spec 004
//! sections 3.9 to 3.13 rest on, recorded from one provider version. This
//! module turns them into a corpus a new provider version is compared against:
//!
//! - `corpus.json` names each golden's scenario, provider version, platform,
//!   invocation, capture date, file and redactions (section 3.1);
//! - `quarantine.json` names each capture that is a wrong recording, with a
//!   reason and the evidence, kept under `quarantine/` and never deleted
//!   (section 3.2);
//! - `ledger.json` names each accepted divergence, append-only (section 3.3).
//!
//! [`compare`] reads by **meaning**, over the fields the adapter maps
//! ([`MAPPED_FIELDS`]), never by bytes: [`IGNORED_FIELDS`] differ every run.
//! It is pure. Nothing here spawns a process, opens a socket or reads a
//! credential (R-3); the two loaders read only the files their manifests name,
//! through a caller's reader or under one directory.

use crate::outcome::{outcome, provider_claim};
use crate::stream::{ProviderEvent, read_jsonl};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

/// The fields a comparison reads, each one a field spec 004 sections 3.10 to
/// 3.13 maps. A difference in any of them is a divergence.
pub const MAPPED_FIELDS: [&str; 10] = [
    "appliedTools",
    "costReported",
    "deniedTools",
    "hookEvents",
    "isError",
    "midStreamDenials",
    "outcome",
    "providerClaim",
    "subtype",
    "terminalReason",
];

/// What differs every run, or what no mapping reads, and is therefore never
/// compared. `claude_code_version` is not compared either: it is checked
/// against the manifest instead (R-1).
pub const IGNORED_FIELDS: [&str; 16] = [
    "session_id",
    "uuid",
    "hook_id",
    "timestamp",
    "cwd",
    "total_cost_usd",
    "usage",
    "modelUsage",
    "duration_ms",
    "duration_api_ms",
    "num_turns",
    "model",
    "message ids and tool-use ids",
    "tool_input",
    "result text and message content",
    "tool names other than the verbatim ones, and the tool count",
];

/// The tool names the recorded fixtures keep verbatim (the stream README);
/// every other name is a count-preserving placeholder, so only these can be
/// compared across a redacted golden and a fresh capture.
pub const VERBATIM_TOOLS: [&str; 7] = ["Bash", "Read", "Edit", "Write", "Glob", "Grep", "Task"];

/// The corpus manifest's file name, under the corpus directory.
pub const CORPUS_MANIFEST: &str = "corpus.json";
/// The quarantine list's file name, under the corpus directory.
pub const QUARANTINE_LIST: &str = "quarantine.json";
/// The divergence ledger's file name, under the corpus directory.
pub const LEDGER: &str = "ledger.json";
/// A capture set's manifest file name, under the capture directory.
pub const CAPTURE_MANIFEST: &str = "captures.json";
/// Where quarantined recordings are kept, under the corpus directory.
pub const QUARANTINE_DIRECTORY: &str = "quarantine/";

/// One recorded stream and what it was recorded from (section 3.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capture {
    /// The named scenario, reused across versions.
    pub scenario: String,
    /// The provider version the stream was recorded from.
    pub provider_version: String,
    /// The platform it was recorded on.
    pub platform: String,
    /// The invocation, as an operator would repeat it.
    pub invocation: String,
    /// The capture date, `YYYY-MM-DD`.
    pub captured: String,
    /// The stream's path, relative to the manifest's directory.
    pub file: String,
    /// What was redacted or dropped from the recording.
    #[serde(default)]
    pub redactions: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    captures: Vec<Capture>,
}

/// A capture that is a wrong recording, kept rather than deleted (section 3.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuarantineEntry {
    /// The scenario whose recording is quarantined.
    pub scenario: String,
    /// The version it was recorded from.
    pub provider_version: String,
    /// The platform it was recorded on.
    pub platform: String,
    /// The kept recording, under [`QUARANTINE_DIRECTORY`].
    pub file: String,
    /// Why the recording is wrong.
    pub reason: String,
    /// What shows it is wrong.
    pub evidence: String,
    /// When it was quarantined, `YYYY-MM-DD`.
    pub quarantined: String,
    /// The fresh golden that lifted it, by its `file`; absent while active.
    #[serde(default)]
    pub lifted_by: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QuarantineList {
    entries: Vec<QuarantineEntry>,
}

/// An accepted difference between a version's capture and its golden
/// (section 3.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LedgerEntry {
    /// The scenario.
    pub scenario: String,
    /// The provider version whose capture diverges.
    pub provider_version: String,
    /// The mapped fields that differ, each one of [`MAPPED_FIELDS`].
    pub fields: Vec<String>,
    /// Why the difference does not change the outcome mapping.
    pub reason: String,
    /// The spec amendment that changed the mapping, when one did.
    #[serde(default)]
    pub amendment: Option<String>,
    /// Who reviewed it.
    pub reviewer: String,
    /// When, `YYYY-MM-DD`.
    pub date: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Ledger {
    entries: Vec<LedgerEntry>,
}

/// Why a corpus or a capture set could not be compared.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GoldenError {
    /// A file the manifests name could not be read.
    #[error("{file} could not be read: {detail}")]
    Read {
        /// The file, relative to its directory.
        file: String,
        /// What the read said.
        detail: String,
    },
    /// A manifest, list or ledger does not parse as its shape.
    #[error("{file} does not parse: {detail}")]
    Schema {
        /// The file.
        file: String,
        /// What the deserializer said.
        detail: String,
    },
    /// The corpus, quarantine or ledger breaks one of spec 052's rules.
    #[error("{0}")]
    Invalid(String),
    /// A capture does not match the golden it would be compared to (R-1).
    #[error("{0}")]
    Refused(String),
}

/// A stream's meaning: each mapped field's value.
pub type Meaning = BTreeMap<String, Value>;

/// Read a stream's provider version and its meaning over [`MAPPED_FIELDS`].
pub fn meaning(text: &str) -> Result<(String, Meaning), String> {
    let events = read_jsonl(text).map_err(|e| e.to_string())?;
    let mut init = None;
    let mut hooks = false;
    let mut mid = Vec::new();
    let mut result = None;
    for event in &events {
        match event {
            ProviderEvent::System(s) if s.is_init() => init = init.or(Some(s)),
            ProviderEvent::System(s) if s.is_hook() => hooks = true,
            ProviderEvent::System(s) if s.is_permission_denied() => {
                mid.push(s.tool_name.clone().unwrap_or_default());
            }
            ProviderEvent::Result(r) => result = Some(r),
            _ => {}
        }
    }
    let init = init.ok_or("the stream carries no init event")?;
    let version = init
        .claude_code_version
        .clone()
        .ok_or("the init event names no claude_code_version")?;
    let tools = init.tools.clone().unwrap_or_default();
    let applied: Vec<&str> = VERBATIM_TOOLS
        .iter()
        .copied()
        .filter(|t| tools.iter().any(|n| n == t))
        .collect();
    mid.sort();
    let mut m = Meaning::new();
    m.insert("appliedTools".into(), json!(applied));
    m.insert("hookEvents".into(), json!(hooks));
    m.insert("midStreamDenials".into(), json!(mid));
    match result {
        None => {
            // Spec 004 section 3.5 case 4: a truncated stream is malformed,
            // and that is its meaning, not a gap in it.
            m.insert("outcome".into(), json!("malformed"));
            for field in [
                "costReported",
                "deniedTools",
                "isError",
                "providerClaim",
                "subtype",
                "terminalReason",
            ] {
                m.insert(field.into(), Value::Null);
            }
        }
        Some(r) => {
            let mut denied: Vec<&str> = r
                .permission_denials
                .iter()
                .map(|d| d.tool_name.as_str())
                .collect();
            denied.sort_unstable();
            let word = outcome(r).map_or("unmapped", |reading| reading.outcome.word());
            m.insert("costReported".into(), json!(r.total_cost_usd.is_some()));
            m.insert("deniedTools".into(), json!(denied));
            m.insert("isError".into(), json!(r.is_error));
            m.insert("outcome".into(), json!(word));
            m.insert(
                "providerClaim".into(),
                serde_json::to_value(provider_claim(r)).unwrap_or(Value::Null),
            );
            m.insert("subtype".into(), json!(r.subtype));
            m.insert("terminalReason".into(), json!(r.terminal_reason));
        }
    }
    Ok((version, m))
}

/// The mapped fields whose values differ between two meanings.
pub fn differing_fields(golden: &Meaning, capture: &Meaning) -> Vec<String> {
    let keys: BTreeSet<&String> = golden.keys().chain(capture.keys()).collect();
    keys.into_iter()
        .filter(|k| golden.get(*k) != capture.get(*k))
        .cloned()
        .collect()
}

/// A recorded stream, its manifest entry and its meaning.
#[derive(Debug, Clone)]
pub struct Recorded {
    /// The manifest entry.
    pub capture: Capture,
    /// The stream's meaning.
    pub meaning: Meaning,
}

/// The goldens, the quarantine and the ledger, validated.
#[derive(Debug, Clone)]
pub struct Corpus {
    /// Every golden, in manifest order.
    pub goldens: Vec<Recorded>,
    /// Every quarantine entry, active and lifted.
    pub quarantine: Vec<QuarantineEntry>,
    /// Every ledger entry, in the order it was appended.
    pub ledger: Vec<LedgerEntry>,
}

/// One version's captures, validated.
#[derive(Debug, Clone)]
pub struct CaptureSet {
    /// The one provider version every capture names.
    pub provider_version: String,
    /// The captures, one per scenario.
    pub captures: Vec<Recorded>,
}

fn parse<T: serde::de::DeserializeOwned>(file: &str, text: &str) -> Result<T, GoldenError> {
    serde_json::from_str(text).map_err(|e| GoldenError::Schema {
        file: file.to_string(),
        detail: e.to_string(),
    })
}

fn date(field: &str, value: &str) -> Result<(), GoldenError> {
    let b = value.as_bytes();
    let shaped = b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
    if shaped {
        Ok(())
    } else {
        Err(GoldenError::Invalid(format!(
            "{field} `{value}` is not a YYYY-MM-DD date"
        )))
    }
}

fn required(what: &str, value: &str) -> Result<(), GoldenError> {
    if value.trim().is_empty() {
        Err(GoldenError::Invalid(format!("{what} is empty")))
    } else {
        Ok(())
    }
}

/// Read every entry's stream and check it against its entry (R-1).
fn recorded(
    captures: Vec<Capture>,
    read: &dyn Fn(&str) -> Result<String, String>,
) -> Result<Vec<Recorded>, GoldenError> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for capture in captures {
        required("a capture's scenario", &capture.scenario)?;
        required("a capture's platform", &capture.platform)?;
        required("a capture's invocation", &capture.invocation)?;
        date("a capture's date", &capture.captured)?;
        let key = (
            capture.scenario.clone(),
            capture.provider_version.clone(),
            capture.platform.clone(),
        );
        if !seen.insert(key) {
            return Err(GoldenError::Invalid(format!(
                "scenario `{}` is recorded twice at {} on {}",
                capture.scenario, capture.provider_version, capture.platform
            )));
        }
        let text = read(&capture.file).map_err(|detail| GoldenError::Read {
            file: capture.file.clone(),
            detail,
        })?;
        let (version, meaning) = meaning(&text).map_err(|detail| {
            GoldenError::Refused(format!(
                "{} is not a provider stream: {detail}",
                capture.file
            ))
        })?;
        if version != capture.provider_version {
            return Err(GoldenError::Refused(format!(
                "{} was recorded from {version}, and its entry names {}",
                capture.file, capture.provider_version
            )));
        }
        out.push(Recorded { capture, meaning });
    }
    Ok(out)
}

impl Corpus {
    /// Load and validate a corpus through `read`, which is given a path
    /// relative to the corpus directory.
    pub fn load(read: &dyn Fn(&str) -> Result<String, String>) -> Result<Self, GoldenError> {
        let text = |file: &str| {
            read(file).map_err(|detail| GoldenError::Read {
                file: file.to_string(),
                detail,
            })
        };
        let manifest: Manifest = parse(CORPUS_MANIFEST, &text(CORPUS_MANIFEST)?)?;
        let quarantine: QuarantineList = parse(QUARANTINE_LIST, &text(QUARANTINE_LIST)?)?;
        let ledger: Ledger = parse(LEDGER, &text(LEDGER)?)?;
        if let Some(c) = manifest
            .captures
            .iter()
            .find(|c| c.file.starts_with(QUARANTINE_DIRECTORY))
        {
            return Err(GoldenError::Invalid(format!(
                "golden {} is under {QUARANTINE_DIRECTORY}",
                c.file
            )));
        }
        let goldens = recorded(manifest.captures, read)?;
        for q in &quarantine.entries {
            // R-2: excluded only with a recorded reason, and kept.
            required(&format!("quarantine reason for {}", q.file), &q.reason)?;
            required(&format!("quarantine evidence for {}", q.file), &q.evidence)?;
            date("a quarantine date", &q.quarantined)?;
            if !q.file.starts_with(QUARANTINE_DIRECTORY) {
                return Err(GoldenError::Invalid(format!(
                    "quarantined {} is not kept under {QUARANTINE_DIRECTORY}",
                    q.file
                )));
            }
            text(&q.file)?;
            // R-2: lifting needs a fresh capture of that scenario.
            if let Some(lift) = &q.lifted_by {
                let fresh = goldens.iter().any(|g| {
                    &g.capture.file == lift
                        && g.capture.scenario == q.scenario
                        && g.capture.captured > q.quarantined
                });
                if !fresh {
                    return Err(GoldenError::Invalid(format!(
                        "quarantine of {} is lifted by {lift}, which is not a golden of \
                         `{}` captured after {}",
                        q.file, q.scenario, q.quarantined
                    )));
                }
            }
        }
        let mut last = String::new();
        for l in &ledger.entries {
            required("a ledger reviewer", &l.reviewer)?;
            required("a ledger reason", &l.reason)?;
            date("a ledger date", &l.date)?;
            if l.fields.is_empty() {
                return Err(GoldenError::Invalid(format!(
                    "a ledger entry for `{}` at {} names no field",
                    l.scenario, l.provider_version
                )));
            }
            if let Some(f) = l
                .fields
                .iter()
                .find(|f| !MAPPED_FIELDS.contains(&f.as_str()))
            {
                return Err(GoldenError::Invalid(format!(
                    "ledger field `{f}` is not a mapped field"
                )));
            }
            // Append-only: an entry is never dated before the one above it.
            if l.date < last {
                return Err(GoldenError::Invalid(format!(
                    "ledger entry dated {} follows one dated {last}; the ledger is append-only",
                    l.date
                )));
            }
            last.clone_from(&l.date);
        }
        Ok(Self {
            goldens,
            quarantine: quarantine.entries,
            ledger: ledger.entries,
        })
    }

    /// Load a corpus from a directory.
    pub fn from_dir(dir: &Path) -> Result<Self, GoldenError> {
        Self::load(&|file: &str| read_under(dir, file))
    }

    /// Every scenario the corpus names.
    pub fn scenarios(&self) -> BTreeSet<String> {
        self.goldens
            .iter()
            .map(|g| g.capture.scenario.clone())
            .chain(self.quarantine.iter().map(|q| q.scenario.clone()))
            .collect()
    }

    /// The golden a scenario is compared to: its latest capture.
    pub fn golden(&self, scenario: &str) -> Option<&Recorded> {
        self.goldens
            .iter()
            .filter(|g| g.capture.scenario == scenario)
            .max_by(|a, b| {
                (&a.capture.captured, &a.capture.provider_version)
                    .cmp(&(&b.capture.captured, &b.capture.provider_version))
            })
    }

    /// The active quarantine entry for a scenario, when one is not lifted.
    pub fn quarantined(&self, scenario: &str) -> Option<&QuarantineEntry> {
        self.quarantine
            .iter()
            .find(|q| q.scenario == scenario && q.lifted_by.is_none())
    }
}

impl CaptureSet {
    /// Load and validate one version's captures through `read`.
    pub fn load(read: &dyn Fn(&str) -> Result<String, String>) -> Result<Self, GoldenError> {
        let text = read(CAPTURE_MANIFEST).map_err(|detail| GoldenError::Read {
            file: CAPTURE_MANIFEST.to_string(),
            detail,
        })?;
        let manifest: Manifest = parse(CAPTURE_MANIFEST, &text)?;
        let versions: BTreeSet<&str> = manifest
            .captures
            .iter()
            .map(|c| c.provider_version.as_str())
            .collect();
        let provider_version = match versions.into_iter().collect::<Vec<_>>().as_slice() {
            [one] => (*one).to_string(),
            [] => return Err(GoldenError::Invalid("the capture set is empty".into())),
            many => {
                return Err(GoldenError::Refused(format!(
                    "the capture set names more than one provider version: {}",
                    many.join(", ")
                )));
            }
        };
        let mut scenarios = BTreeSet::new();
        if let Some(c) = manifest
            .captures
            .iter()
            .find(|c| !scenarios.insert(c.scenario.clone()))
        {
            return Err(GoldenError::Invalid(format!(
                "scenario `{}` is captured twice",
                c.scenario
            )));
        }
        Ok(Self {
            provider_version,
            captures: recorded(manifest.captures, read)?,
        })
    }

    /// Load a capture set from a directory.
    pub fn from_dir(dir: &Path) -> Result<Self, GoldenError> {
        Self::load(&|file: &str| read_under(dir, file))
    }
}

/// Read a manifest-named file, refusing a path that leaves the directory.
fn read_under(dir: &Path, file: &str) -> Result<String, String> {
    let relative = Path::new(file);
    if !relative
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err("the path is not relative to its directory".into());
    }
    std::fs::read_to_string(dir.join(relative)).map_err(|e| e.to_string())
}

/// What a comparison found for one scenario (section 3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// The capture means what the golden means.
    Matches,
    /// It differs, and a ledger entry covers every differing field.
    DivergesReviewed,
    /// It differs, and no ledger entry covers it.
    DivergesUnreviewed,
    /// The scenario is quarantined, so it is skipped and reported.
    Quarantined,
    /// The version has no capture of the scenario.
    Missing,
}

impl Status {
    /// Whether this status withholds qualification.
    pub fn blocks(self) -> bool {
        matches!(self, Status::DivergesUnreviewed | Status::Missing)
    }
}

/// One scenario's row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    /// The scenario.
    pub scenario: String,
    /// What was found.
    pub status: Status,
    /// The golden's version, when a golden was compared.
    pub golden_version: Option<String>,
    /// The mapped fields that differ.
    pub differing_fields: Vec<String>,
    /// The ledger entry that covers them, by its index.
    pub ledger_entry: Option<usize>,
    /// The quarantine reason, when the scenario is quarantined.
    pub quarantine_reason: Option<String>,
}

/// A version's comparison against the goldens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    /// The version compared.
    pub provider_version: String,
    /// One row per scenario, in scenario order.
    pub rows: Vec<Row>,
    /// Whether the version is qualifiable (I-1).
    pub qualifiable: bool,
    /// The scenarios that withhold qualification.
    pub blocking: Vec<String>,
    /// What the comparison never reads.
    pub ignored: Vec<String>,
}

/// Compare one version's captures with the goldens (section 3.4). Offline:
/// both arguments are already in memory.
pub fn compare(corpus: &Corpus, set: &CaptureSet) -> Result<Comparison, GoldenError> {
    let scenarios = corpus.scenarios();
    if let Some(c) = set
        .captures
        .iter()
        .find(|c| !scenarios.contains(&c.capture.scenario))
    {
        return Err(GoldenError::Refused(format!(
            "capture {} names scenario `{}`, which no golden names",
            c.capture.file, c.capture.scenario
        )));
    }
    let mut rows = Vec::new();
    for scenario in scenarios {
        let mut row = Row {
            scenario: scenario.clone(),
            status: Status::Missing,
            golden_version: None,
            differing_fields: Vec::new(),
            ledger_entry: None,
            quarantine_reason: None,
        };
        if let Some(q) = corpus.quarantined(&scenario) {
            row.status = Status::Quarantined;
            row.quarantine_reason = Some(q.reason.clone());
            rows.push(row);
            continue;
        }
        // A scenario named only by a lifted entry and no golden cannot be,
        // since lifting names a golden; so a golden exists here.
        let Some(golden) = corpus.golden(&scenario) else {
            return Err(GoldenError::Invalid(format!(
                "scenario `{scenario}` has neither a golden nor an active quarantine"
            )));
        };
        row.golden_version = Some(golden.capture.provider_version.clone());
        if let Some(capture) = set.captures.iter().find(|c| c.capture.scenario == scenario) {
            let differing = differing_fields(&golden.meaning, &capture.meaning);
            row.status = if differing.is_empty() {
                Status::Matches
            } else if let Some(index) = corpus.ledger.iter().position(|l| {
                l.scenario == scenario
                    && l.provider_version == set.provider_version
                    && differing.iter().all(|f| l.fields.contains(f))
            }) {
                row.ledger_entry = Some(index);
                Status::DivergesReviewed
            } else {
                Status::DivergesUnreviewed
            };
            row.differing_fields = differing;
        }
        rows.push(row);
    }
    let blocking: Vec<String> = rows
        .iter()
        .filter(|r| r.status.blocks())
        .map(|r| r.scenario.clone())
        .collect();
    Ok(Comparison {
        provider_version: set.provider_version.clone(),
        qualifiable: blocking.is_empty(),
        blocking,
        rows,
        ignored: IGNORED_FIELDS.iter().map(|s| s.to_string()).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_meaning_of_the_denied_golden_is_refused_with_its_denial() {
        let (version, m) = meaning(include_str!("../testdata/stream/denied.jsonl")).unwrap();
        assert_eq!(version, "2.1.267");
        assert_eq!(m["outcome"], "refused");
        assert_eq!(m["deniedTools"], json!(["Bash"]));
        assert_eq!(m["midStreamDenials"], json!(["Bash"]));
        assert_eq!(m["subtype"], "success");
        assert_eq!(m.len(), MAPPED_FIELDS.len());
    }

    #[test]
    fn a_truncated_stream_means_malformed() {
        let text = include_str!("../testdata/stream/success.jsonl");
        let truncated: String = text
            .lines()
            .filter(|l| !l.contains("\"type\":\"result\""))
            .map(|l| format!("{l}\n"))
            .collect();
        let (_, m) = meaning(&truncated).unwrap();
        assert_eq!(m["outcome"], "malformed");
    }

    #[test]
    fn a_path_leaving_its_directory_is_not_read() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_under(dir.path(), "../corpus.json").is_err());
        assert!(read_under(dir.path(), "/etc/hosts").is_err());
    }
}
