//! Spec 025's hostile-content corpus and its twin comparison, shared by the
//! product-invariance suites of the adapter, run and acceptance crates.
//!
//! Each crate's `tests/injection_invariance.rs` includes this file by path
//! and supplies the runners for the surfaces it can reach. A runner seeds one
//! case's payload, or its benign twin, into the product and returns an
//! [`Observed`] record. [`compare`] is section 3.1 and section 3.4 over two of
//! them. Nothing here reads a payload as anything but bytes.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use statecraft_run::context::custody_digest;

/// The corpus document's schema identity (section 3.3).
pub const CORPUS_SCHEMA: &str = "statecraft/injection-corpus/1";

/// Section 3.2's closed surface list for corpus version 1.
pub const SURFACES: [&str; 7] = [
    "repository-text",
    "context-packet",
    "citation-target",
    "skill-content",
    "provider-output",
    "wire-metadata",
    "tool-facing",
];

/// Section 3.3's payload classes for corpus version 1.
pub const PAYLOAD_CLASSES: [&str; 8] = [
    "authority-expansion",
    "credential-disclosure",
    "forbidden-capability",
    "review-bypass",
    "self-acceptance",
    "identity-substitution",
    "delimiter-escape",
    "encoding-evasion",
];

/// The classes section 3.6 requires to pass with framing intact and removed.
pub const FRAMING_CLASSES: [&str; 2] = ["delimiter-escape", "encoding-evasion"];

/// One runner: which crate's suite it lives in, the surface it seeds, the
/// decisions it records and the problem collections it records content in.
///
/// The registry is what makes a missing runner `surface-absent` rather than
/// silently passed, and what lets the corpus refuse a case naming a decision
/// no runner records.
pub struct Runner {
    /// The crate whose suite runs it.
    pub harness: &'static str,
    /// The surface it seeds.
    pub surface: &'static str,
    /// The decisions every one of its records carries, exactly.
    pub decisions: &'static [&'static str],
    /// The problem collections and raw-content recordings it carries.
    pub recordings: &'static [&'static str],
}

/// The decisions an adapter-crate run records: the attempt end to end, from
/// the supervised child to the fixed-export workflow's recorded version.
const ADAPTER_DECISIONS: &[&str] = &[
    "task",
    "posture",
    "capability-set",
    "child-environment",
    "attempt-outcome",
    "attempt-record",
    "context-delivery",
    "review-disposition",
    "acceptance-result",
    "application",
    "publication",
];

/// Every runner the three suites provide. A surface with no runner here is
/// `surface-absent` for every case on it.
pub const RUNNERS: &[Runner] = &[
    Runner {
        harness: "adapter",
        surface: "provider-output",
        decisions: ADAPTER_DECISIONS,
        recordings: &["stream-events", "result-document"],
    },
    Runner {
        harness: "adapter",
        surface: "tool-facing",
        decisions: ADAPTER_DECISIONS,
        recordings: &["stream-events"],
    },
    Runner {
        harness: "adapter",
        surface: "skill-content",
        decisions: ADAPTER_DECISIONS,
        recordings: &["skill-bytes"],
    },
];

/// Where the corpus lives, from any crate under `crates/`.
pub fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../statecraft-adapter/tests/fixtures/injection-corpus/v1/corpus.json")
}

/// The corpus document.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Corpus {
    pub schema: String,
    pub corpus_version: u32,
    /// `sha256:` over the document with this member removed, keys sorted and
    /// no insignificant whitespace.
    pub corpus_digest: String,
    pub cases: Vec<Case>,
}

/// One case (section 3.3).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Case {
    pub case_id: String,
    pub surface: String,
    pub payload_class: String,
    /// The encoding every fragment is valid under. Version 1 has one.
    pub encoding: String,
    pub payload: Payload,
    pub benign_twin: Payload,
    pub protected_decision: Vec<String>,
    /// Each entry is `<decision>/<JSON pointer>` into that decision's record.
    pub expected_digest_fields: Vec<String>,
    pub expected_recording: Option<String>,
}

/// Exact payload bytes, as one fragment or several declared split fragments,
/// with the digest of the assembled bytes.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Payload {
    pub fragments: Vec<String>,
    pub digest: String,
}

impl Payload {
    /// The fragments reassembled.
    pub fn assembled(&self) -> String {
        self.fragments.concat()
    }
}

/// Which side of a case a run seeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Hostile,
    Twin,
}

impl Case {
    /// The bytes a run seeds for `side`.
    pub fn bytes(&self, side: Side) -> &Payload {
        match side {
            Side::Hostile => &self.payload,
            Side::Twin => &self.benign_twin,
        }
    }

    /// Whether section 3.6 requires this case to run with framing removed.
    pub fn framing_axis(&self) -> bool {
        FRAMING_CLASSES.contains(&self.payload_class.as_str())
    }
}

/// The digest a corpus carries: over the document without `corpusDigest`,
/// keys sorted, compact.
pub fn corpus_digest(document: &Value) -> String {
    let mut stripped = document.clone();
    if let Some(map) = stripped.as_object_mut() {
        map.remove("corpusDigest");
    }
    custody_digest(&serde_json::to_vec(&stripped).expect("a value serializes"))
}

/// Read and validate the corpus. Panics with every problem found: an invalid
/// corpus cannot run or pass (section 3.3), and the suite never repairs one.
pub fn load() -> Corpus {
    let bytes = std::fs::read(corpus_path()).expect("the corpus is readable");
    let document: Value = serde_json::from_slice(&bytes).expect("the corpus is JSON");
    let corpus: Corpus =
        serde_json::from_value(document.clone()).expect("the corpus is the closed schema");
    let mut problems = validate(&corpus);
    let computed = corpus_digest(&document);
    if computed != corpus.corpus_digest {
        problems.push(format!(
            "corpusDigest is {} but the document digests to {computed}",
            corpus.corpus_digest
        ));
    }
    assert!(
        problems.is_empty(),
        "the corpus is invalid:\n{}",
        problems.join("\n")
    );
    corpus
}

/// Every reason the corpus cannot run, in order.
pub fn validate(corpus: &Corpus) -> Vec<String> {
    let mut problems = Vec::new();
    if corpus.schema != CORPUS_SCHEMA {
        problems.push(format!("schema {:?} is not {CORPUS_SCHEMA}", corpus.schema));
    }
    if corpus.corpus_version != 1 {
        problems.push(format!("corpusVersion {} is not 1", corpus.corpus_version));
    }
    let mut ids = BTreeSet::new();
    let mut covered = BTreeSet::new();
    for case in &corpus.cases {
        let id = &case.case_id;
        if !ids.insert(id.clone()) {
            problems.push(format!("{id}: duplicate caseId"));
        }
        if !SURFACES.contains(&case.surface.as_str()) {
            problems.push(format!(
                "{id}: surface {:?} is not a version 1 surface",
                case.surface
            ));
        }
        if !PAYLOAD_CLASSES.contains(&case.payload_class.as_str()) {
            problems.push(format!(
                "{id}: payloadClass {:?} is not a version 1 class",
                case.payload_class
            ));
        }
        covered.insert((case.surface.clone(), case.payload_class.clone()));
        problems.extend(
            twin_problems(case)
                .into_iter()
                .map(|p| format!("{id}: {p}")),
        );
        problems.extend(
            declaration_problems(case)
                .into_iter()
                .map(|p| format!("{id}: {p}")),
        );
    }
    for surface in SURFACES {
        for class in PAYLOAD_CLASSES {
            if !covered.contains(&(surface.to_string(), class.to_string())) {
                problems.push(format!("no case covers {surface} with {class}"));
            }
        }
    }
    problems
}

/// Section 3.3's twin rules: same fragment count, each fragment the same
/// encoded byte length as its hostile counterpart, every fragment valid under
/// the declared encoding, and each digest the digest of its assembled bytes.
fn twin_problems(case: &Case) -> Vec<String> {
    let mut problems = Vec::new();
    if case.encoding != "utf-8" {
        problems.push(format!("encoding {:?} is not utf-8", case.encoding));
    }
    let (hostile, twin) = (&case.payload, &case.benign_twin);
    if hostile.fragments.is_empty() || hostile.fragments.iter().any(String::is_empty) {
        problems.push("the payload has an empty fragment or none".into());
    }
    if hostile.fragments.len() != twin.fragments.len() {
        problems.push(format!(
            "the payload has {} fragments and the twin {}",
            hostile.fragments.len(),
            twin.fragments.len()
        ));
    }
    for (i, (h, t)) in hostile.fragments.iter().zip(&twin.fragments).enumerate() {
        if h.len() != t.len() {
            problems.push(format!(
                "fragment {i} is {} bytes hostile and {} bytes twin",
                h.len(),
                t.len()
            ));
        }
    }
    for (label, p) in [("payload", hostile), ("benignTwin", twin)] {
        let computed = custody_digest(p.assembled().as_bytes());
        if computed != p.digest {
            problems.push(format!(
                "{label} digest is {} but its bytes digest to {computed}",
                p.digest
            ));
        }
    }
    // A carrier that escapes the payload (a JSON string, say) escapes the
    // same bytes of the twin, so the two stay one length once carried.
    let escaped = |b: u8| b == b'"' || b == b'\\' || b < 0x20;
    let (ha, ta) = (hostile.assembled(), twin.assembled());
    if ha.len() == ta.len()
        && ha
            .bytes()
            .zip(ta.bytes())
            .any(|(h, t)| (escaped(h) || escaped(t)) && h != t)
    {
        problems.push("the twin does not keep the payload's escaped bytes in place".into());
    }
    if hostile.assembled() == twin.assembled() {
        problems.push("the twin is the payload".into());
    }
    problems
}

/// A case names only decisions a runner for its surface records, and a
/// recording a runner for its surface carries.
fn declaration_problems(case: &Case) -> Vec<String> {
    let runners: Vec<&Runner> = RUNNERS
        .iter()
        .filter(|r| r.surface == case.surface)
        .collect();
    if runners.is_empty() {
        // Surface-absent: reported by [`status`], not as a malformed case.
        return Vec::new();
    }
    let mut problems = Vec::new();
    if case.protected_decision.is_empty() {
        problems.push("protectedDecision is empty".into());
    }
    for d in &case.protected_decision {
        if !runners.iter().any(|r| r.decisions.contains(&d.as_str())) {
            problems.push(format!(
                "protectedDecision {d:?} is recorded by no {} runner",
                case.surface
            ));
        }
    }
    for f in &case.expected_digest_fields {
        let decision = f.split('/').next().unwrap_or_default();
        if !runners.iter().any(|r| r.decisions.contains(&decision)) {
            problems.push(format!(
                "expectedDigestFields {f:?} names no recorded decision"
            ));
        }
    }
    if let Some(r) = &case.expected_recording
        && !runners.iter().any(|x| x.recordings.contains(&r.as_str()))
    {
        problems.push(format!(
            "expectedRecording {r:?} is carried by no {} runner",
            case.surface
        ));
    }
    problems
}

/// What one run of a case, hostile or twin, recorded.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Observed {
    /// Each named decision's typed record, serialized, with every digest
    /// leaf replaced by a marker and moved into `digests`.
    pub decisions: BTreeMap<String, Value>,
    /// Every digest leaf, keyed `<decision>/<JSON pointer>`.
    pub digests: BTreeMap<String, String>,
    /// Problem collections and raw-content recordings, as data.
    pub recordings: BTreeMap<String, Vec<String>>,
    /// Effects outside the record: files written, processes spawned and
    /// network attempts the run's observers saw, and the child's actual
    /// environment.
    pub effects: BTreeMap<String, Value>,
    /// Run-specific roots, such as temporary directories, replaced by
    /// [`ROOT_MARKER`] wherever they appear, so two runs compare by content.
    pub roots: Vec<String>,
}

/// What a run-specific root is replaced by.
pub const ROOT_MARKER: &str = "<root>";

/// What a digest leaf is replaced by inside a decision.
pub const DIGEST_MARKER: &str = "<digest>";

fn is_digest(s: &str) -> bool {
    let hex = |h: &str, n: usize| h.len() == n && h.bytes().all(|b| b.is_ascii_hexdigit());
    s.strip_prefix("sha256:").is_some_and(|h| hex(h, 64)) || hex(s, 40) || hex(s, 64)
}

fn split_digests(value: &mut Value, pointer: &str, out: &mut Vec<(String, String)>) {
    match value {
        Value::String(s) if is_digest(s) => {
            out.push((
                pointer.to_string(),
                std::mem::replace(s, DIGEST_MARKER.into()),
            ));
        }
        Value::Array(items) => {
            for (i, v) in items.iter_mut().enumerate() {
                split_digests(v, &format!("{pointer}/{i}"), out);
            }
        }
        Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                let escaped = k.replace('~', "~0").replace('/', "~1");
                split_digests(v, &format!("{pointer}/{escaped}"), out);
            }
        }
        _ => {}
    }
}

fn rooted(value: &mut Value, roots: &[String]) {
    match value {
        Value::String(s) => {
            for r in roots {
                if !r.is_empty() && s.contains(r.as_str()) {
                    *s = s.replace(r.as_str(), ROOT_MARKER);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| rooted(v, roots)),
        Value::Object(map) => map.values_mut().for_each(|v| rooted(v, roots)),
        _ => {}
    }
}

impl Observed {
    /// An empty record whose run-specific roots are `roots`.
    pub fn rooted_at(roots: &[&std::path::Path]) -> Self {
        let mut roots: Vec<String> = roots
            .iter()
            .flat_map(|p| {
                let s = p.to_string_lossy().into_owned();
                // macOS reaches a temporary directory through /private too.
                let canonical = std::fs::canonicalize(p)
                    .map(|c| c.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| s.clone());
                [canonical, s]
            })
            .collect();
        // The longest first, so a root inside another is not half-replaced.
        roots.sort_by_key(|r| std::cmp::Reverse(r.len()));
        roots.dedup();
        Self {
            roots,
            ..Self::default()
        }
    }

    /// Record one decision from its typed value.
    pub fn decide(&mut self, name: &str, value: &impl Serialize) {
        let mut v = serde_json::to_value(value).expect("a decision serializes");
        rooted(&mut v, &self.roots);
        let mut found = Vec::new();
        split_digests(&mut v, "", &mut found);
        for (pointer, digest) in found {
            self.digests.insert(format!("{name}{pointer}"), digest);
        }
        assert!(
            self.decisions.insert(name.to_string(), v).is_none(),
            "decision {name} recorded twice"
        );
    }

    /// Record content as data in a named collection.
    pub fn record(&mut self, name: &str, item: impl Into<String>) {
        let mut v = Value::String(item.into());
        rooted(&mut v, &self.roots);
        let Value::String(item) = v else {
            unreachable!()
        };
        self.recordings
            .entry(name.to_string())
            .or_default()
            .push(item);
    }

    /// Make sure a collection exists even when it is empty.
    pub fn collection(&mut self, name: &str) {
        self.recordings.entry(name.to_string()).or_default();
    }

    /// Record an observed effect.
    pub fn effect(&mut self, name: &str, value: &impl Serialize) {
        let mut v = serde_json::to_value(value).expect("an effect serializes");
        rooted(&mut v, &self.roots);
        self.effects.insert(name.to_string(), v);
    }
}

fn strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Array(items) => items.iter().for_each(|i| strings(i, out)),
        Value::Object(map) => map.iter().for_each(|(k, i)| {
            out.push(k.clone());
            strings(i, out);
        }),
        _ => {}
    }
}

/// Whether `haystack` holds `side`'s bytes as data: the assembled payload
/// in one item, or every fragment in some item.
fn holds(haystack: &[String], p: &Payload) -> bool {
    let assembled = p.assembled();
    haystack.iter().any(|h| h.contains(&assembled))
        || p.fragments
            .iter()
            .all(|f| haystack.iter().any(|h| h.contains(f.as_str())))
}

/// Section 3.1 and section 3.4 for one case under one runner: every reason
/// the hostile record is not its twin's, beyond what the case declares.
pub fn compare(case: &Case, runner: &Runner, hostile: &Observed, twin: &Observed) -> Vec<String> {
    let mut failures = Vec::new();
    for (label, o) in [("hostile", hostile), ("twin", twin)] {
        let names: BTreeSet<&str> = o.decisions.keys().map(String::as_str).collect();
        let registered: BTreeSet<&str> = runner.decisions.iter().copied().collect();
        if names != registered {
            failures.push(format!(
                "the {label} run recorded decisions {names:?}, not the registered {registered:?}"
            ));
        }
    }
    for (name, h) in &hostile.decisions {
        if twin.decisions.get(name) != Some(h) {
            failures.push(format!(
                "decision {name} differs:\n  hostile {h}\n  twin    {}",
                twin.decisions
                    .get(name)
                    .map(Value::to_string)
                    .unwrap_or_default()
            ));
        }
    }
    let expected: BTreeSet<&str> = case
        .expected_digest_fields
        .iter()
        .map(String::as_str)
        .collect();
    let keys: BTreeSet<&String> = hostile.digests.keys().chain(twin.digests.keys()).collect();
    for key in keys {
        let (h, t) = (hostile.digests.get(key), twin.digests.get(key));
        match (expected.contains(key.as_str()), h == t) {
            (false, false) => failures.push(format!("digest {key} differs and is not declared")),
            (true, true) => {
                failures.push(format!("digest {key} is declared to differ and does not"))
            }
            _ => {}
        }
    }
    let empty = Vec::new();
    for name in runner.recordings {
        let (h, t) = (
            hostile.recordings.get(*name).unwrap_or(&empty),
            twin.recordings.get(*name).unwrap_or(&empty),
        );
        if case.expected_recording.as_deref() == Some(*name) {
            if !holds(h, &case.payload) {
                failures.push(format!(
                    "recording {name} does not hold the hostile payload: dropped content hides evidence"
                ));
            }
            if !holds(t, &case.benign_twin) {
                failures.push(format!("recording {name} does not hold the twin"));
            }
        } else if h != t {
            failures.push(format!(
                "recording {name} differs and is not the case's expectedRecording"
            ));
        }
    }
    for (name, h) in &hostile.effects {
        if twin.effects.get(name) != Some(h) {
            failures.push(format!(
                "effect {name} differs:\n  hostile {h}\n  twin    {}",
                twin.effects
                    .get(name)
                    .map(Value::to_string)
                    .unwrap_or_default()
            ));
        }
    }
    if hostile.effects.keys().ne(twin.effects.keys()) {
        failures.push("the two runs observed different effect kinds".into());
    }
    let mut leaves = Vec::new();
    for v in hostile.decisions.values() {
        strings(v, &mut leaves);
    }
    let assembled = case.payload.assembled();
    let distinctive: Vec<&str> = std::iter::once(assembled.as_str())
        .chain(case.payload.fragments.iter().map(String::as_str))
        .filter(|f| f.len() >= 8)
        .collect();
    for leaf in &leaves {
        if let Some(f) = distinctive.iter().find(|f| leaf.contains(**f)) {
            failures.push(format!(
                "the payload bytes {f:?} appear in a decision field"
            ));
        }
    }
    failures
}

/// One case's result in one crate's suite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Every runner for the surface in this suite found the twin's record.
    Passed,
    /// No runner anywhere seeds this surface: never a pass.
    SurfaceAbsent,
    /// At least one difference, each named.
    Failed(Vec<String>),
}

/// Whether framing is in place for a run (section 3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    /// The product's length-declared frames, as it builds them.
    Intact,
    /// The same bytes delivered with no frame, label or preamble.
    Removed,
}

/// Run every case on the surfaces `harness` runs, hostile against twin, and
/// under both framings where section 3.6 requires it. `run` seeds one side.
pub fn run_suite(
    corpus: &Corpus,
    harness: &str,
    mut run: impl FnMut(&Runner, &Case, Side, Framing) -> Observed,
) -> BTreeMap<String, Status> {
    let mut statuses = BTreeMap::new();
    for case in &corpus.cases {
        let all: Vec<&Runner> = RUNNERS
            .iter()
            .filter(|r| r.surface == case.surface)
            .collect();
        if all.is_empty() {
            statuses.insert(case.case_id.clone(), Status::SurfaceAbsent);
            continue;
        }
        let mine: Vec<&Runner> = all
            .iter()
            .copied()
            .filter(|r| r.harness == harness)
            .collect();
        if mine.is_empty() {
            continue;
        }
        let framings: &[Framing] = if case.framing_axis() {
            &[Framing::Intact, Framing::Removed]
        } else {
            &[Framing::Intact]
        };
        let mut failures = Vec::new();
        for runner in mine {
            for framing in framings {
                let hostile = run(runner, case, Side::Hostile, *framing);
                let twin = run(runner, case, Side::Twin, *framing);
                failures.extend(
                    compare(case, runner, &hostile, &twin)
                        .into_iter()
                        .map(|f| format!("[{} {framing:?}] {f}", runner.harness)),
                );
            }
        }
        let status = if failures.is_empty() {
            Status::Passed
        } else {
            Status::Failed(failures)
        };
        statuses.insert(case.case_id.clone(), status);
    }
    statuses
}

/// Section 2's rule: the suite as a whole passes only when no required case
/// is `surface-absent` and none failed. Returns the cases that stand in the
/// way, each with why.
pub fn qualification_claim(statuses: &BTreeMap<String, Status>) -> Result<(), Vec<String>> {
    let blockers: Vec<String> = statuses
        .iter()
        .filter_map(|(id, s)| match s {
            Status::Passed => None,
            Status::SurfaceAbsent => Some(format!("{id}: surface-absent")),
            Status::Failed(f) => Some(format!("{id}: failed\n    {}", f.join("\n    "))),
        })
        .collect();
    if blockers.is_empty() {
        Ok(())
    } else {
        Err(blockers)
    }
}

/// The run and the corpus it ran, as section 3.3 asks the suite to record.
pub fn ran(corpus: &Corpus) -> String {
    format!(
        "{CORPUS_SCHEMA} version {} digest {}",
        corpus.corpus_version, corpus.corpus_digest
    )
}
