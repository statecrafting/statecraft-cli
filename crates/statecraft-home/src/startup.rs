//! What a managed session records about delivery at its start.
//!
//! Spec 002 section 3.26, settled by the owner on 2026-09-21.
//!
//! # Seven things, and three evidence classes
//!
//! The record carries seven fields: the project identity, the instruction-file
//! identities, the required harness identity, the resolved harness identity,
//! the adapter identity, the load chain and the delivery status. The last of
//! those is section 3.14's three-valued verdict, and it is **unchanged**:
//! `reached`, `not-reached` and `unverified` mean exactly what section 3.14's
//! table says, and redefining an existing verdict would change the meaning of
//! every record already written under it.
//!
//! What is new is carried in added fields, narrowly defined, alongside the
//! verdict. They keep three statements distinct and never substitute one for
//! another:
//!
//! 1. **The documented load chain reaches a file.** [`Delivery`], the section
//!    3.14 verdict. A statement about the tree and the rule.
//! 2. **Its bytes were resolved and supplied.** [`Supply`]. A statement about
//!    what this product did.
//! 3. **A live session demonstrated the expected behavior.** [`Observation`].
//!    A statement about a session, and only a session can produce it.
//!
//! Each is strictly weaker evidence for the next and none implies the one
//! after it, which is why they are three fields rather than one richer word.
//! [`StartupRecord::qualifies`] is the only place all three are read together,
//! and it requires all three.
//!
//! # What no field here claims
//!
//! A digest establishes that bytes are the bytes. An acknowledgement
//! establishes that something emitted an acknowledgement. Neither establishes
//! that a model **read**, **understood** or **complied with** the
//! instructions, and no field in this record makes that claim.
//!
//! # Absent evidence is not success
//!
//! A record is assembled from parts that can each be absent, and an absent
//! part is recorded as an absence rather than omitted. [`StartupRecord::write`]
//! refuses a record whose required fields are not all present, and it writes
//! through a temporary file and a rename, so a reader never sees half a record
//! and mistakes a truncation for a shorter answer.

pub use crate::admission::{
    Capture, Control, Evidence, Invocation, Measurement, NotAdmitted, floor_claims,
};

use crate::delivery::Delivery;
use crate::required::Standing;
use serde::{Deserialize, Serialize};
use statecraft_environment::digest::digest_bytes;
use statecraft_environment::manifest::Manifest;
use std::path::{Path, PathBuf};

/// The schema version of a startup record.
pub const STARTUP_VERSION: u32 = 1;

/// Where startup records live inside a project.
///
/// Under the ignored runtime state, beside the frozen run resolutions: a
/// startup record is about one session on one machine, and committing it would
/// make a per-machine fact part of the governed corpus. The committed half of
/// section 3.25 is the requirement, and only the requirement.
pub const DIRECTORY: &str = "startup";

/// One file, by path and digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileIdentity {
    /// Repository-relative, forward slashes.
    pub path: String,
    /// SHA-256 of the bytes read.
    pub digest: String,
    /// Length in bytes.
    pub bytes: u64,
}

impl FileIdentity {
    /// Read one file's identity from a project tree.
    ///
    /// `Ok(None)` when the file is not there, which is an absence the caller
    /// records rather than an error it hides.
    pub fn read(root: &Path, rel: &str) -> std::io::Result<Option<Self>> {
        let path = statecraft_environment::claimant::resolve(root, rel);
        match statecraft_environment::digest::digest_file(&path)? {
            None => Ok(None),
            Some((digest, bytes)) => Ok(Some(Self {
                path: rel.to_string(),
                digest,
                bytes,
            })),
        }
    }
}

/// Which repository this is, and the manifest that makes it a target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIdentity {
    /// The project root, absolute.
    pub root: String,
    /// The manifest's own digest: the bytes that make this a target.
    pub manifest_digest: String,
    /// The schema version the manifest declares.
    pub manifest_version: u32,
}

impl ProjectIdentity {
    /// Read a project's identity. `Ok(None)` when it holds no manifest, which
    /// is the one state in which it is not a target at all.
    pub fn read(root: &Path) -> std::io::Result<Option<Self>> {
        let path = statecraft_environment::claimant::resolve(
            root,
            statecraft_environment::manifest::MANIFEST_PATH,
        );
        let Some((manifest_digest, _)) = statecraft_environment::digest::digest_file(&path)? else {
            return Ok(None);
        };
        let version = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .and_then(|v| v.get("version").and_then(serde_json::Value::as_u64))
            .unwrap_or(0) as u32;
        Ok(Some(Self {
            root: root.display().to_string(),
            manifest_digest,
            manifest_version: version,
        }))
    }
}

/// Which adapter performed the delivery, and its own identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterIdentity {
    /// The adapter's own name.
    pub name: String,
    /// The harness it targets.
    pub harness: String,
    /// Its version.
    pub version: String,
}

impl AdapterIdentity {
    /// The identity of a declared adapter.
    pub fn of(declaration: &statecraft_environment::adapter::Declaration) -> Self {
        Self {
            name: declaration.name.clone(),
            harness: declaration.harness.clone(),
            version: declaration.version.clone(),
        }
    }
}

/// Evidence class 2: whether the bytes were resolved and supplied.
///
/// Strictly stronger than the section 3.14 verdict and strictly weaker than a
/// live observation. A `reached` verdict says a rule arrives at a file; this
/// says the file was read, it digests to what is recorded, and its content was
/// handed to the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "supply")]
pub enum Supply {
    /// Every file in the chain was read and handed over.
    Supplied {
        /// Each file supplied, by path and digest.
        files: Vec<FileIdentity>,
    },
    /// Delivery was attempted and did not complete.
    ///
    /// A failed delivery, which is a fact about what this product did. Not the
    /// same as never having tried.
    Failed {
        /// Why.
        reason: String,
        /// What had been supplied when it failed, so a partial delivery is
        /// visible as a partial one rather than as either extreme.
        partial: Vec<FileIdentity>,
    },
    /// Delivery was not attempted, so nothing about it is established.
    NotAttempted {
        /// Why, citing what is and is not established.
        reason: String,
    },
}

impl Supply {
    /// True only when every file was read and handed over.
    pub fn supplied(&self) -> bool {
        matches!(self, Supply::Supplied { .. })
    }

    /// A one-word rendering.
    pub fn word(&self) -> &'static str {
        match self {
            Supply::Supplied { .. } => "supplied",
            Supply::Failed { .. } => "failed",
            Supply::NotAttempted { .. } => "not-attempted",
        }
    }

    /// A one-line rendering.
    pub fn describe(&self) -> String {
        match self {
            Supply::Supplied { files } => format!("supplied {} file(s)", files.len()),
            Supply::Failed { reason, partial } => {
                format!("failed after {} file(s): {reason}", partial.len())
            }
            Supply::NotAttempted { reason } => format!("not attempted: {reason}"),
        }
    }
}

/// Evidence class 3: whether a live session demonstrated the expected behavior.
///
/// Only a session produces this, and section 3.29 fixes what a claimed one has
/// to carry. [`Observation::Observed`] carries the evidence it was admitted
/// from, which makes two things true that were not true of the first version
/// of this type:
///
/// 1. **Every route runs the same admission.** [`admit`] is the constructor,
///    [`Observation::from_qualification`] carries what a qualification already
///    carries, and a value that arrives by **deserialization** is re-judged by
///    [`Observation::admitted`] before anything treats it as qualified.
///    Section 3.29 rule 5: writing the word into a file by hand is not a weaker
///    route to the claim, it is not a route at all.
/// 2. **The bytes are kept.** The captures are in the record, not beside it,
///    because rules 3 and 5 are not decidable from a digest once the original
///    file is gone.
///
/// Section 3.28 still holds underneath: a deny entry present in a settings file
/// establishes that the entry is configured, and configured is not enforced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "observation")]
pub enum Observation {
    /// A running session was observed behaving as the instructions require.
    Observed {
        /// The harness version the observation was made against.
        version: String,
        /// What was observed, in one line.
        observed: String,
        /// What it was admitted from, captures and all.
        evidence: Box<crate::admission::Evidence>,
    },
    /// No live observation. What is established, and what is not.
    NotObserved {
        /// Why.
        reason: String,
    },
}

impl Observation {
    /// Derive the observation from a session qualification.
    ///
    /// A probe produces [`crate::session::Qualification::NotQualified`], and
    /// this maps it to [`Observation::NotObserved`], which is what makes "a
    /// probe alone cannot qualify a session" a property of the type rather
    /// than a habit. The qualified arm carries the same evidence this type
    /// does, so the conversion moves evidence and never manufactures it.
    pub fn from_qualification(q: &crate::session::Qualification) -> Self {
        match q {
            crate::session::Qualification::Qualified {
                version,
                observed,
                evidence,
            } => Observation::Observed {
                version: version.clone(),
                observed: observed.clone(),
                evidence: evidence.clone(),
            },
            crate::session::Qualification::NotQualified { reason, .. } => {
                Observation::NotObserved {
                    reason: reason.clone(),
                }
            }
        }
    }

    /// True for the shape of a live observation.
    ///
    /// Structural only. It says the value is an `Observed`, not that the
    /// evidence inside it still stands, which is [`Observation::admitted`].
    /// The two are separate because a record read back from a file has the
    /// shape without having been judged.
    pub fn observed(&self) -> bool {
        matches!(self, Observation::Observed { .. })
    }

    /// Whether this observation's own evidence still admits it.
    ///
    /// Section 3.29 rule 5. Re-run rather than trusted, so a hand-written
    /// record faces exactly the admission a live claim faces.
    pub fn admitted(&self) -> Result<(), crate::admission::NotAdmitted> {
        match self {
            Observation::Observed { evidence, .. } => crate::admission::admit(evidence),
            Observation::NotObserved { .. } => Ok(()),
        }
    }

    /// Whether the evidence was launched against a local fake.
    ///
    /// Spec 002 section 3.30: synthetic evidence runs the whole admission and
    /// is never a live observation, so a record carrying it never qualifies.
    pub fn synthetic(&self) -> bool {
        match self {
            Observation::Observed { evidence, .. } => evidence.synthetic(),
            Observation::NotObserved { .. } => false,
        }
    }

    /// Whether this is an admitted, live observation: the shape, the evidence
    /// re-judged, and not synthetic. The one predicate a qualification reads.
    pub fn live(&self) -> bool {
        self.observed() && self.admitted().is_ok() && !self.synthetic()
    }

    /// A one-word rendering.
    pub fn word(&self) -> &'static str {
        match self {
            Observation::Observed { .. } if self.synthetic() => "observed-synthetic",
            Observation::Observed { .. } => "observed",
            Observation::NotObserved { .. } => "not-observed",
        }
    }
}

/// What a managed session records at its start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupRecord {
    /// Schema version.
    pub version: u32,
    /// The session this record is about.
    pub session_id: String,
    /// When, RFC 3339 UTC.
    pub recorded_at: String,

    // The seven fields of section 3.26's table.
    /// 1. Which repository, and the manifest that makes it a target.
    pub project: ProjectIdentity,
    /// 2. Each instruction file reached, by path and digest.
    pub instructions: Vec<FileIdentity>,
    /// 3. The committed requirement of section 3.25, full digest.
    ///
    /// `None` when the project commits none, which is not qualified.
    pub required_harness: Option<String>,
    /// 4. What actually answered, full digest.
    ///
    /// `None` when nothing resolved, which is not qualified.
    pub resolved_harness: Option<String>,
    /// 5. Which adapter performed the delivery, and its own identity.
    pub adapter: AdapterIdentity,
    /// 6. The files traversed, entry first, managed file last.
    pub load_chain: Vec<String>,
    /// 7. The section 3.14 verdict, unchanged.
    pub delivery: Delivery,

    // The added evidence fields, narrowly defined, alongside the verdict.
    /// How the required and resolved identities stand against each other.
    pub standing: Standing,
    /// Evidence class 2: were the bytes resolved and supplied.
    pub supply: Supply,
    /// Evidence class 3: did a live session demonstrate the behavior.
    pub observation: Observation,
}

/// Why a record is not written.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Incomplete {
    /// A field a record cannot be written without.
    #[error(
        "the startup record is missing {field}, and an absent field is not a weaker answer: {why}"
    )]
    MissingField {
        /// Which one.
        field: &'static str,
        /// Why it cannot be defaulted.
        why: &'static str,
    },
    /// A recorded chain that does not match the verdict it is filed under.
    #[error("the record says {verdict} and carries a load chain of {chain} entries: {why}")]
    ChainDisagreesWithVerdict {
        /// The verdict word.
        verdict: &'static str,
        /// How many entries the chain has.
        chain: usize,
        /// Why that cannot be written.
        why: &'static str,
    },
    /// A recorded observation whose own evidence does not admit it.
    #[error(
        "the record carries an observation that its own evidence does not admit: {why}. \
         Section 3.29 rule 5: every route to an admitted observation runs the same \
         admission, deserialization included"
    )]
    ObservationNotAdmitted {
        /// What the admission said.
        why: String,
    },
}

impl StartupRecord {
    /// Every reason this record may not be written.
    ///
    /// A record with an absent required field is not a shorter record: it is a
    /// record that does not establish what it claims to. Checked before the
    /// write rather than tolerated by the reader, so nothing downstream has to
    /// decide what a half-filled record means.
    pub fn incomplete(&self) -> Option<Incomplete> {
        if self.session_id.trim().is_empty() {
            return Some(Incomplete::MissingField {
                field: "sessionId",
                why: "a record that does not say which session it is about cannot be read \
                      back as evidence for one",
            });
        }
        if self.recorded_at.trim().is_empty() {
            return Some(Incomplete::MissingField {
                field: "recordedAt",
                why: "an undated record cannot be placed relative to the state it describes",
            });
        }
        if self.project.manifest_digest.trim().is_empty() {
            return Some(Incomplete::MissingField {
                field: "project.manifestDigest",
                why: "the manifest is what makes this repository a target, so a record with \
                      no manifest identity is about no target",
            });
        }
        if self.adapter.name.trim().is_empty() {
            return Some(Incomplete::MissingField {
                field: "adapter.name",
                why: "a delivery nobody is recorded as having performed is not a delivery \
                      this record can attribute",
            });
        }
        // The verdict and the chain are one statement. `reached` names the
        // chain it arrived by (section 3.14), and a `reached` with no chain is
        // the overclaim the verdict exists to prevent.
        match (&self.delivery, self.load_chain.len()) {
            (Delivery::Reached { .. }, 0) => {
                return Some(Incomplete::ChainDisagreesWithVerdict {
                    verdict: self.delivery.word(),
                    chain: 0,
                    why: "`reached` is the verdict that names its chain; with none named \
                          there is nothing that arrived",
                });
            }
            (Delivery::NotReached { .. } | Delivery::Unverified { .. }, n) if n > 0 => {
                return Some(Incomplete::ChainDisagreesWithVerdict {
                    verdict: self.delivery.word(),
                    chain: n,
                    why: "a verdict that did not arrive cannot carry the chain it would \
                          have arrived by",
                });
            }
            _ => {}
        }
        // Section 3.29 rule 5, at the write. A record whose observation no
        // longer admits itself is not written at all: it would sit in the tree
        // carrying the word `observed` and read as evidence to anything that
        // looked at the field rather than at the claim.
        if let Err(why) = self.observation.admitted() {
            return Some(Incomplete::ObservationNotAdmitted {
                why: why.to_string(),
            });
        }
        None
    }

    /// Whether this session carries the managed-execution claim.
    ///
    /// All three evidence classes, and section 3.25's standing. Deliberately a
    /// conjunction with no shortcut: none of the three implies the next, so
    /// none of them substitutes for another here either.
    pub fn qualifies(&self) -> bool {
        self.standing.permits_managed_execution()
            && self.delivery.reached()
            && self.supply.supplied()
            // Section 3.29 rule 5 and section 3.30. The three conditions above
            // are properties of this record; this one is a property of the
            // evidence the record carries, re-judged rather than believed, so a
            // record assembled by hand reaches this line and is refused by it,
            // and so is one whose evidence came from a local fake.
            && self.observation.live()
    }

    /// What this record establishes, one line per evidence class.
    ///
    /// Three lines rather than one verdict, because collapsing them is exactly
    /// the substitution section 3.26 forbids.
    pub fn describe(&self) -> String {
        format!(
            "standing {}\nreachability {}\nsupply {}\nobservation {}\nqualified {}",
            self.standing.word(),
            self.delivery.describe(),
            self.supply.describe(),
            self.observation.word(),
            self.qualifies()
        )
    }

    /// Where a session's startup record lives inside a project.
    pub fn path(root: &Path, session_id: &str) -> PathBuf {
        statecraft_environment::claimant::resolve(
            root,
            &format!("{}/{DIRECTORY}/{session_id}.json", crate::project::STATE),
        )
    }

    /// Write the record.
    ///
    /// Refuses an incomplete one, and writes through a temporary file and a
    /// rename so no reader sees a partially written record. A second write for
    /// the same session is refused for the same reason
    /// [`crate::resolved::ResolvedRun::write`] refuses one: the record is
    /// evidence about a start, and a start happens once.
    pub fn write(&self, root: &Path) -> std::io::Result<()> {
        if let Some(reason) = self.incomplete() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                reason.to_string(),
            ));
        }
        let path = Self::path(root, &self.session_id);
        if path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "session {} already has a startup record; a start happens once",
                    self.session_id
                ),
            ));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut json = serde_json::to_string_pretty(self)?;
        json.push('\n');
        crate::settings::write_atomically(&path, &json)
    }

    /// Read a session's startup record, if it has one.
    ///
    /// A record that does not deserialize is an error rather than `None`: the
    /// absence of a record and the presence of an unreadable one are different
    /// facts, and reporting the second as the first is how unavailable evidence
    /// becomes a clean slate.
    pub fn read(root: &Path, session_id: &str) -> std::io::Result<Option<Self>> {
        match std::fs::read(Self::path(root, session_id)) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/// Assemble the record a managed session writes at its start.
///
/// Everything it needs is passed in, because every part is a measurement
/// somebody else made: the standing comes from [`crate::required::evaluate`],
/// the verdict from [`crate::delivery::evaluate`], the supply from whatever
/// actually handed bytes over, and the observation from a session. This
/// function reads the two identities it can read for itself, which are the
/// project's and the instruction files' in the chain the verdict names.
#[allow(clippy::too_many_arguments)]
pub fn assemble(
    root: &Path,
    session_id: &str,
    recorded_at: &str,
    manifest: &Manifest,
    adapter: AdapterIdentity,
    delivery: Delivery,
    standing: Standing,
    resolved_harness: Option<String>,
    supply: Supply,
    observation: Observation,
) -> std::io::Result<StartupRecord> {
    let project = ProjectIdentity::read(root)?.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "{} holds no {}, so it is not a target and has no startup to record",
                root.display(),
                statecraft_environment::manifest::MANIFEST_PATH
            ),
        )
    })?;

    let load_chain = match &delivery {
        Delivery::Reached { via } => via.clone(),
        _ => Vec::new(),
    };

    // The identities of the files the chain names, read now. A file the chain
    // names and the tree no longer holds is recorded as absent rather than
    // skipped: a shorter list of identities beside an unchanged chain would
    // read as a shorter chain.
    let mut instructions = Vec::new();
    for rel in &load_chain {
        match FileIdentity::read(root, rel)? {
            Some(identity) => instructions.push(identity),
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "the load chain names {rel} and it is not in the tree; a chain whose \
                         files cannot be identified is not evidence that anything was reached"
                    ),
                ));
            }
        }
    }

    Ok(StartupRecord {
        version: STARTUP_VERSION,
        session_id: session_id.to_string(),
        recorded_at: recorded_at.to_string(),
        project,
        instructions,
        required_harness: crate::required::required_of(manifest).map(str::to_string),
        resolved_harness,
        adapter,
        load_chain,
        delivery,
        standing,
        supply,
        observation,
    })
}

/// Admit a live-session observation, or say why it is not one.
///
/// Section 3.29's admission, and the only constructor of
/// [`Observation::Observed`] that is not a deserialization. The rules and the
/// reasoning live in [`crate::admission`]; this is where they meet the record,
/// and it does one thing the module below cannot: it builds the observation's
/// one-line sentence **from the evidence**, so the sentence in a record cannot
/// describe something the evidence does not show.
///
/// A refused claim returns the refusal. It does not return a weaker
/// observation, and a caller that records one anyway is recording an absence,
/// which is what section 3.29 rule 6 requires: unverified is the answer, and a
/// truthful inability to qualify is preferable to an invented proof.
pub fn admit(evidence: &crate::admission::Evidence) -> Result<Observation, NotAdmitted> {
    crate::admission::admit(evidence)?;
    Ok(Observation::Observed {
        version: evidence.version.clone(),
        observed: crate::admission::describe(evidence),
        evidence: Box::new(evidence.clone()),
    })
}

/// The identity of the settings payload a session was started with.
///
/// Recorded beside the rest because section 3.27's mechanism is the thing a
/// live observation is made against: an observation of a session started with
/// other bytes is an observation of something else.
pub fn payload_identity() -> String {
    digest_bytes(crate::session::payload_json().as_bytes())
}

#[cfg(test)]
mod tests {
    // The one fixture, shared with this crate's integration tests. A test that
    // weakens a control is then visibly weakening the same evidence every
    // other test passes against.
    //
    // `include!` rather than `#[path]`: a nested module's path is resolved
    // against `src/startup/`, a directory this crate does not have, so the
    // traversal fails before it reaches the file. `include!` resolves against
    // the file that invokes it, which exists by construction.
    #[allow(dead_code)]
    mod evidence {
        use crate as statecraft_home;
        include!("../tests/support/evidence.rs");
    }

    use super::*;
    use crate::harness;
    use crate::home::Layout;
    use crate::required;
    use statecraft_environment::manifest::Pins;

    fn adapter() -> AdapterIdentity {
        AdapterIdentity {
            name: "claude-code".into(),
            harness: "claude-code".into(),
            version: "0.1.0".into(),
        }
    }

    fn manifest(requirement: Option<&str>) -> Manifest {
        let mut m = Manifest::new(Pins {
            product: "0.1.0".into(),
            spec_spine: "0.20.0".into(),
            adapters: Default::default(),
        });
        if let Some(r) = requirement {
            m.project
                .requirements
                .insert(required::REQUIREMENT_KEY.into(), r.into());
        }
        m
    }

    /// A project whose root `AGENTS.md` imports the managed instructions, so
    /// the documented rule actually arrives.
    fn project(requirement: Option<&str>) -> (tempfile::TempDir, Manifest) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".statecraft")).unwrap();
        let m = manifest(requirement);
        m.write(root).unwrap();
        std::fs::write(
            root.join(crate::project::INSTRUCTIONS),
            crate::project::managed_instructions(),
        )
        .unwrap();
        std::fs::write(
            root.join("AGENTS.md"),
            format!("@{}\n\n# ours\n", crate::project::INSTRUCTIONS),
        )
        .unwrap();
        (dir, m)
    }

    fn claude_rule() -> crate::delivery::LoadRule {
        crate::delivery::load_rules()
            .into_iter()
            .find(|r| r.harness == "claude-code")
            .unwrap()
    }

    /// A home holding the shipped revision, and its full digest.
    fn home() -> (tempfile::TempDir, Layout, String) {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        let digest = harness::install(&layout, &harness::shipped())
            .unwrap()
            .revision
            .digest;
        (dir, layout, digest)
    }

    fn supplied(root: &Path, chain: &[String]) -> Supply {
        Supply::Supplied {
            files: chain
                .iter()
                .map(|rel| FileIdentity::read(root, rel).unwrap().unwrap())
                .collect(),
        }
    }

    fn observed() -> Observation {
        admit(&evidence::admissible()).expect("the fixture is admissible")
    }

    /// The fully favourable record: every evidence class present.
    fn qualifying() -> (tempfile::TempDir, tempfile::TempDir, StartupRecord) {
        let (home_dir, layout, digest) = home();
        let (dir, m) = project(Some(&digest));
        let verdict = crate::delivery::evaluate(dir.path(), &claude_rule());
        assert!(verdict.reached(), "{verdict:?}");
        let chain = match &verdict {
            Delivery::Reached { via } => via.clone(),
            _ => unreachable!(),
        };
        let standing = required::evaluate(&layout, &m, Some(&digest));
        let record = assemble(
            dir.path(),
            "s-1",
            "1970-01-01T00:00:00Z",
            &m,
            adapter(),
            verdict,
            standing,
            Some(digest),
            supplied(dir.path(), &chain),
            observed(),
        )
        .unwrap();
        (home_dir, dir, record)
    }

    #[test]
    fn the_record_carries_the_seven_fields_and_the_added_evidence_beside_them() {
        let (_h, _d, record) = qualifying();
        let json = serde_json::to_value(&record).unwrap();
        for field in [
            "project",
            "instructions",
            "requiredHarness",
            "resolvedHarness",
            "adapter",
            "loadChain",
            "delivery",
        ] {
            assert!(json.get(field).is_some(), "the record omits {field}");
        }
        // The verdict is the section 3.14 word, unchanged and not widened.
        assert_eq!(json["delivery"]["verdict"], "reached");
        // The added evidence is beside it, never folded into it.
        assert_eq!(json["supply"]["supply"], "supplied");
        assert_eq!(json["observation"]["observation"], "observed");
        assert_eq!(json["standing"]["standing"], "exact");
        assert!(record.qualifies());
    }

    #[test]
    fn the_three_evidence_classes_are_never_substituted_for_one_another() {
        let (_h, _d, base) = qualifying();

        // Class 1 alone: a rule arrives, nothing was supplied, nothing was
        // observed. Still `reached`, and not qualified.
        let mut structural = base.clone();
        structural.supply = Supply::NotAttempted {
            reason: "no delivery was performed in this session".into(),
        };
        structural.observation = Observation::NotObserved {
            reason: "no live session was observed".into(),
        };
        assert!(structural.delivery.reached());
        assert!(!structural.qualifies());

        // Classes 1 and 2: bytes were supplied. Still not a live observation.
        let mut supplied_only = base.clone();
        supplied_only.observation = Observation::NotObserved {
            reason: "no live session was observed".into(),
        };
        assert!(supplied_only.supply.supplied());
        assert!(!supplied_only.qualifies());

        // Only all three together.
        assert!(base.qualifies());
    }

    #[test]
    fn a_probe_alone_cannot_produce_a_live_observation() {
        // Every shape a probe can take, including the most favourable one.
        for probe in [
            crate::session::VersionProbe {
                version: None,
                argument_present: false,
            },
            crate::session::VersionProbe {
                version: Some("2.1.267".into()),
                argument_present: true,
            },
        ] {
            let observation =
                Observation::from_qualification(&crate::session::qualification_from(&probe));
            assert!(!observation.observed(), "{observation:?}");

            let (_h, _d, base) = qualifying();
            let mut from_probe = base;
            from_probe.observation = observation;
            assert!(
                !from_probe.qualifies(),
                "a version probe qualified a session"
            );
        }
    }

    #[test]
    fn a_missing_instruction_file_is_refused_rather_than_recorded_as_a_shorter_chain() {
        let (_h, layout, digest) = home();
        let (dir, m) = project(Some(&digest));
        let verdict = crate::delivery::evaluate(dir.path(), &claude_rule());
        let chain = match &verdict {
            Delivery::Reached { via } => via.clone(),
            _ => unreachable!(),
        };
        assert!(chain.len() > 1);
        std::fs::remove_file(dir.path().join(crate::project::INSTRUCTIONS)).unwrap();

        let err = assemble(
            dir.path(),
            "s-1",
            "1970-01-01T00:00:00Z",
            &m,
            adapter(),
            verdict,
            required::evaluate(&layout, &m, Some(&digest)),
            Some(digest),
            Supply::NotAttempted { reason: "x".into() },
            Observation::NotObserved { reason: "x".into() },
        )
        .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        assert!(err.to_string().contains("not evidence"), "{err}");
    }

    #[test]
    fn changed_bytes_are_a_different_recorded_identity() {
        let (_h, _d, first) = qualifying();
        let root = PathBuf::from(&first.project.root);
        std::fs::write(
            root.join(crate::project::INSTRUCTIONS),
            "# something else\n",
        )
        .unwrap();
        let after = FileIdentity::read(&root, crate::project::INSTRUCTIONS)
            .unwrap()
            .unwrap();
        let before = first
            .instructions
            .iter()
            .find(|f| f.path == crate::project::INSTRUCTIONS)
            .unwrap();
        assert_ne!(before.digest, after.digest);
        assert_ne!(before.bytes, after.bytes);
    }

    #[test]
    fn a_changed_manifest_is_a_different_project_identity() {
        let (_h, _d, record) = qualifying();
        let root = PathBuf::from(&record.project.root);
        let mut m = Manifest::read(&root).unwrap().unwrap();
        m.project.overrides.insert("anything".into(), "else".into());
        m.write(&root).unwrap();
        let after = ProjectIdentity::read(&root).unwrap().unwrap();
        assert_ne!(record.project.manifest_digest, after.manifest_digest);
    }

    #[test]
    fn mismatched_identities_are_recorded_and_do_not_qualify() {
        let (_hd, layout, digest) = home();
        let (dir, m) = project(Some(&digest));
        let other = "e".repeat(64);
        let verdict = crate::delivery::evaluate(dir.path(), &claude_rule());
        let chain = match &verdict {
            Delivery::Reached { via } => via.clone(),
            _ => unreachable!(),
        };
        let record = assemble(
            dir.path(),
            "s-1",
            "1970-01-01T00:00:00Z",
            &m,
            adapter(),
            verdict,
            required::evaluate(&layout, &m, Some(&other)),
            Some(other.clone()),
            supplied(dir.path(), &chain),
            observed(),
        )
        .unwrap();

        assert_eq!(record.required_harness.as_deref(), Some(digest.as_str()));
        assert_eq!(record.resolved_harness.as_deref(), Some(other.as_str()));
        assert_eq!(record.standing.word(), "mismatched");
        assert!(
            !record.qualifies(),
            "a mismatch qualified because the other three classes were favourable"
        );
    }

    #[test]
    fn a_failed_delivery_is_distinct_from_one_never_attempted() {
        let (_h, _d, base) = qualifying();

        let mut failed = base.clone();
        failed.supply = Supply::Failed {
            reason: "the write was interrupted".into(),
            partial: base.instructions.clone(),
        };
        assert_eq!(failed.supply.word(), "failed");
        assert!(failed.supply.describe().contains("interrupted"));
        assert!(!failed.qualifies());

        let mut never = base;
        never.supply = Supply::NotAttempted {
            reason: "this session delivered nothing".into(),
        };
        assert_eq!(never.supply.word(), "not-attempted");
        assert!(!never.qualifies());
        assert_ne!(failed.supply, never.supply);
    }

    #[test]
    fn unavailable_evidence_is_recorded_as_unavailable_and_never_as_success() {
        let (_hd, layout, _digest) = home();
        // A manifest with no requirement, a harness that cannot be evaluated,
        // no delivery and no observation: every class absent at once.
        let (dir, m) = project(None);
        let codex = crate::delivery::load_rules()
            .into_iter()
            .find(|r| r.harness == "codex-cli")
            .unwrap();
        let verdict = crate::delivery::evaluate(dir.path(), &codex);
        assert_eq!(verdict.word(), "unverified");

        let record = assemble(
            dir.path(),
            "s-1",
            "1970-01-01T00:00:00Z",
            &m,
            AdapterIdentity {
                name: "codex-cli".into(),
                harness: "codex-cli".into(),
                version: "0.1.0".into(),
            },
            verdict,
            required::evaluate(&layout, &m, None),
            None,
            Supply::NotAttempted {
                reason: "this harness has no rule this product can evaluate, so nothing was \
                         delivered on the strength of one"
                    .into(),
            },
            Observation::NotObserved {
                reason: "no session was run".into(),
            },
        )
        .unwrap();

        assert_eq!(record.required_harness, None);
        assert_eq!(record.resolved_harness, None);
        assert_eq!(record.standing, Standing::Unrequired);
        assert!(record.load_chain.is_empty());
        assert!(!record.qualifies());
        // It is still a writable record: unavailable evidence is evidence
        // about what is unavailable, and losing it is worse than keeping it.
        assert_eq!(record.incomplete(), None);
        record.write(dir.path()).unwrap();
    }

    #[test]
    fn an_incomplete_record_is_refused_rather_than_written() {
        let (_h, dir, base) = qualifying();
        let root = PathBuf::from(&base.project.root);

        let mut no_session = base.clone();
        no_session.session_id = String::new();
        assert!(matches!(
            no_session.incomplete(),
            Some(Incomplete::MissingField {
                field: "sessionId",
                ..
            })
        ));
        let err = no_session.write(&root).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);

        let mut no_manifest = base.clone();
        no_manifest.project.manifest_digest = String::new();
        assert!(no_manifest.incomplete().is_some());
        assert!(no_manifest.write(&root).is_err());

        let mut no_adapter = base.clone();
        no_adapter.adapter.name = String::new();
        assert!(no_adapter.incomplete().is_some());

        // And nothing landed on disk for any of them.
        assert!(StartupRecord::read(&root, "s-1").unwrap().is_none());
        drop(dir);
    }

    #[test]
    fn a_reached_verdict_with_no_chain_is_refused() {
        let (_h, _d, base) = qualifying();
        let mut lying = base;
        lying.load_chain.clear();
        assert!(matches!(
            lying.incomplete(),
            Some(Incomplete::ChainDisagreesWithVerdict { .. })
        ));
    }

    #[test]
    fn a_verdict_that_did_not_arrive_may_not_carry_a_chain() {
        let (_h, _d, base) = qualifying();
        let mut lying = base;
        lying.delivery = Delivery::NotReached {
            reason: "it does not arrive".into(),
        };
        assert!(matches!(
            lying.incomplete(),
            Some(Incomplete::ChainDisagreesWithVerdict { .. })
        ));
    }

    #[test]
    fn a_record_round_trips_and_a_start_is_recorded_once() {
        let (_h, _d, record) = qualifying();
        let root = PathBuf::from(&record.project.root);
        record.write(&root).unwrap();
        let read = StartupRecord::read(&root, "s-1").unwrap().unwrap();
        assert_eq!(read, record);

        let again = record.write(&root).unwrap_err();
        assert_eq!(again.kind(), std::io::ErrorKind::AlreadyExists);
        // And the first record is intact.
        assert_eq!(StartupRecord::read(&root, "s-1").unwrap().unwrap(), record);
    }

    /// Spec 002 sections 3.29 rule 5 and 3.30: a record read back from disk is
    /// judged again, so its qualification is what its evidence supports now,
    /// not what the file says. Each hand edit below changes one thing and the
    /// reload's judgement follows it.
    #[test]
    fn a_record_read_back_is_judged_again() {
        let (_h, _d, record) = qualifying();
        assert!(record.qualifies());
        let root = PathBuf::from(&record.project.root);
        record.write(&root).unwrap();
        let path = StartupRecord::path(&root, "s-1");
        let reload = || StartupRecord::read(&root, "s-1").unwrap().unwrap();
        assert!(
            reload().qualifies(),
            "the same bytes, read back, qualify the same way"
        );

        let original: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        type Edit = (&'static str, fn(&mut serde_json::Value));
        let edits: [Edit; 3] = [
            ("the launch stripped, as a pre-3.30 record", |v| {
                v["observation"]["evidence"]["refusal"]
                    .as_object_mut()
                    .unwrap()
                    .remove("launch");
            }),
            ("the origin flipped to synthetic", |v| {
                v["observation"]["evidence"]["allowed"]["launch"]["origin"] =
                    serde_json::json!("synthetic");
            }),
            ("the process said to have timed out", |v| {
                v["observation"]["evidence"]["withoutPayload"]["launch"]["process"]["timedOut"] =
                    serde_json::json!(true);
            }),
        ];
        for (name, edit) in edits {
            let mut v = original.clone();
            edit(&mut v);
            std::fs::write(&path, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
            let read = reload();
            assert!(read.observation.observed(), "{name}: the shape survives");
            assert!(!read.qualifies(), "{name}: the reload still qualified");
        }
    }

    #[test]
    fn an_unreadable_record_is_an_error_rather_than_an_absence() {
        let (_h, _d, record) = qualifying();
        let root = PathBuf::from(&record.project.root);
        let path = StartupRecord::path(&root, "s-2");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ truncated").unwrap();
        assert!(
            StartupRecord::read(&root, "s-2").is_err(),
            "a truncated record read as no record, which turns unavailable evidence \
             into a clean slate"
        );
    }

    #[test]
    fn the_record_lives_in_ignored_runtime_state() {
        let path = StartupRecord::path(Path::new("/p"), "s-1");
        assert!(path.to_string_lossy().contains(crate::project::STATE));
    }

    #[test]
    fn a_repository_with_no_manifest_has_no_startup_to_record() {
        let dir = tempfile::tempdir().unwrap();
        let err = assemble(
            dir.path(),
            "s-1",
            "1970-01-01T00:00:00Z",
            &manifest(None),
            adapter(),
            Delivery::Unverified { reason: "x".into() },
            Standing::Unrequired,
            None,
            Supply::NotAttempted { reason: "x".into() },
            Observation::NotObserved { reason: "x".into() },
        )
        .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn no_field_claims_that_a_model_read_understood_or_complied() {
        let (_h, _d, record) = qualifying();
        let json = serde_json::to_string(&record).unwrap().to_lowercase();
        for overclaim in ["understood", "complied", "acknowledged"] {
            assert!(
                !json.contains(overclaim),
                "the record uses the word `{overclaim}`, which no digest establishes"
            );
        }
    }

    #[test]
    fn an_admitted_observation_describes_itself_from_its_own_evidence() {
        let evidence = evidence::admissible();
        let observation = admit(&evidence).unwrap();
        assert!(observation.observed());
        assert!(observation.admitted().is_ok());
        let Observation::Observed {
            version, observed, ..
        } = &observation
        else {
            unreachable!()
        };
        assert_eq!(version, evidence::VERSION);
        // The sentence is built from the evidence, so it names the payload the
        // evidence names and the two controls that make the refusal mean
        // something.
        assert!(observed.contains(&payload_identity()));
        assert!(observed.contains(evidence::REFUSED));
        assert!(observed.contains(evidence::ALLOWED));
    }

    #[test]
    fn a_record_carrying_an_unadmitted_observation_is_not_written() {
        let (_h, dir, mut record) = qualifying();
        let Observation::Observed { evidence, .. } = &mut record.observation else {
            unreachable!()
        };
        // The one change: the positive control is replaced by a copy of the
        // refusal. Section 3.29 rule 3, substituted evidence.
        evidence.allowed = evidence.refusal.clone();
        assert!(!record.qualifies());
        assert!(matches!(
            record.incomplete(),
            Some(Incomplete::ObservationNotAdmitted { .. })
        ));
        let err = record.write(dir.path()).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!StartupRecord::path(dir.path(), &record.session_id).exists());
    }

    #[test]
    fn every_floor_entry_is_matchable_and_nothing_else_is() {
        for entry in crate::settings::DENY_FLOOR {
            let body = entry
                .strip_prefix("Bash(")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let command = body.trim_end_matches('*').to_string();
            assert!(
                floor_claims(&command),
                "the floor entry {entry} claims nothing"
            );
        }
        for command in ["ls", "git status", "cargo build", "git push origin topic"] {
            assert!(!floor_claims(command), "the floor claimed `{command}`");
        }
    }

    #[test]
    fn the_payload_identity_is_over_the_exact_bytes_the_argument_receives() {
        assert_eq!(
            payload_identity(),
            digest_bytes(crate::session::payload_json().as_bytes())
        );
    }
}
