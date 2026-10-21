//! The adopted producer's context-packet reads: spec 019 section 3.3, through
//! its additive `extends` edge on this crate (spec 002 owns it).
//!
//! This is the one crate that links the producer, so it is where a packet is
//! assembled, decoded and validated. Two of the producer's interfaces are
//! used, each where 0.31.0 supplies it:
//!
//! - **The pinned executable** binds a snapshot and assembles a page:
//!   `spec-spine context packet --repo <dir> --request <file> --repository
//!   <id> [--revision <rev>] --json`. Snapshot binding is the producer's CLI
//!   contract; the library never infers one. Its versioned verdict on a
//!   refusal is read for its `error.kind`, never its prose alone.
//! - **The linked library** decodes a page under the exact adopted schema
//!   (`spec_spine_core::types::ContextPacket`, which denies unknown fields) and
//!   recomputes its digest with `spec_spine_core::packet_digest`.
//!
//! Nothing here reimplements a selector, a closure, a digest or a snapshot
//! binding. Every answer is the producer's; the comparisons that judge them
//! are `statecraft-acceptance`'s.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use spec_spine_core::types::{CONTEXT_PACKET_SCHEMA_VERSION, ContextPacket, parse_semver};

use statecraft_run::context::{
    AcceptedPage, BOUNDS, Completeness, Intended, OmissionFact, PageFacts, ProducerBuild,
    Requirement, Snapshot, WarningFact, scan,
};

/// The packet schema the adopted producer serves, which this consumer reads.
pub const SCHEMA_VERSION: &str = CONTEXT_PACKET_SCHEMA_VERSION;

/// The producer operation a retained page records as its source.
pub const SOURCE_OPERATION: &str = "spec-spine context packet";

/// Why a page could not be decoded or validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decoded {
    /// Valid under the adopted schema, its digest reproduced.
    Valid(Box<PageFacts>),
    /// The producer's digest operation does not reproduce the page digest.
    Invalid(String),
    /// Another schema major, or a newer minor than the adopted one serves.
    Unsupported(String),
    /// Beyond the bounds, or not a page of the adopted schema.
    Unreadable(String),
}

fn word<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn requirement(r: spec_spine_core::types::PacketRequirement) -> Requirement {
    match r {
        spec_spine_core::types::PacketRequirement::Required => Requirement::Required,
        spec_spine_core::types::PacketRequirement::Optional => Requirement::Optional,
    }
}

fn packet(bytes: &[u8]) -> Result<ContextPacket, Decoded> {
    scan(&[bytes], &BOUNDS).map_err(|e| Decoded::Unreadable(e.to_string()))?;
    let packet: ContextPacket =
        serde_json::from_slice(bytes).map_err(|e| Decoded::Unreadable(e.to_string()))?;
    let ours = parse_semver(CONTEXT_PACKET_SCHEMA_VERSION).expect("the adopted version parses");
    match parse_semver(&packet.schema_version) {
        Some((major, minor, _)) if major == ours.0 && minor <= ours.1 => Ok(packet),
        _ => Err(Decoded::Unsupported(format!(
            "schema {} is not served by the adopted {CONTEXT_PACKET_SCHEMA_VERSION}",
            packet.schema_version
        ))),
    }
}

/// [`packet`], and the producer's digest operation must reproduce the page
/// digest the bytes carry. Every page this module reads goes through this
/// one: a recorded page before its request, revision or member drives a
/// producer call, and a page the producer just returned before its digest or
/// snapshot is reported.
fn validated(bytes: &[u8]) -> Result<ContextPacket, Decoded> {
    let packet = packet(bytes)?;
    match spec_spine_core::packet_digest(&packet) {
        Ok(d) if d == packet.packet_digest => Ok(packet),
        Ok(d) => Err(Decoded::Invalid(format!(
            "the producer computes {d}, the page carries {}",
            packet.packet_digest
        ))),
        Err(e) => Err(Decoded::Unreadable(e.to_string())),
    }
}

/// Decode and validate one page's exact bytes (section 3.7.1).
pub fn decode(bytes: &[u8]) -> Decoded {
    let packet = match validated(bytes) {
        Ok(p) => p,
        Err(d) => return d,
    };
    Decoded::Valid(Box::new(PageFacts {
        schema_version: packet.schema_version.clone(),
        producer: ProducerBuild {
            package: packet.producer.package.clone(),
            version: packet.producer.version.clone(),
            build: packet.producer.build.clone(),
        },
        snapshot: snapshot(&packet),
        request_digest: packet.request_digest.clone(),
        closure_digest: packet.closure_digest.clone(),
        packet_digest: packet.packet_digest.clone(),
        completeness: match word(&packet.completeness).as_str() {
            "complete" => Completeness::Complete,
            "partial" => Completeness::Partial,
            _ => Completeness::Incomplete,
        },
        continues: packet.continuation.is_some(),
        omissions: packet
            .omissions
            .iter()
            .map(|o| OmissionFact {
                identity: o.identity.clone(),
                requirement: requirement(o.requirement),
                reason: word(&o.reason),
            })
            .collect(),
        warnings: packet
            .warnings
            .iter()
            .map(|w| WarningFact {
                code: word(&w.code),
                identity: w.identity.clone(),
            })
            .collect(),
    }))
}

fn snapshot(packet: &ContextPacket) -> Snapshot {
    Snapshot {
        repository: packet.snapshot.repository.clone(),
        revision: packet.snapshot.revision.clone(),
        tree: packet.snapshot.tree.clone(),
        dirty_state: word(&packet.snapshot.dirty_state),
    }
}

/// A refusal or failure the producer reported, by its verdict's `error.kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// `refused`, `not-found`, `stale`, `io`, `usage`, or `unreadable` when
    /// the answer was not a verdict at all.
    pub kind: String,
    /// The verdict's message.
    pub message: String,
    /// The process exit code, when it exited.
    pub exit: Option<i32>,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

/// Where a packet is read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source<'a> {
    /// The pinned producer executable.
    pub program: &'a Path,
    /// The repository root.
    pub repo: &'a Path,
    /// The opaque repository identity the caller supplies.
    pub repository: &'a str,
    /// The revision to export, or `None` for the clean working tree at HEAD.
    pub revision: Option<&'a str>,
}

/// One page's exact bytes as the producer wrote them.
fn one_page(source: &Source<'_>, request: &Value) -> Result<Vec<u8>, Refusal> {
    let unreadable = |message: String| Refusal {
        kind: "unreadable".into(),
        message,
        exit: None,
    };
    let file = tempfile::Builder::new()
        .prefix("statecraft-context-request-")
        .suffix(".json")
        .tempfile()
        .map_err(|e| unreadable(e.to_string()))?;
    std::fs::write(file.path(), serde_json::to_vec(request).expect("JSON"))
        .map_err(|e| unreadable(e.to_string()))?;
    let mut command = Command::new(source.program);
    command
        .args(["context", "packet", "--repo"])
        .arg(source.repo)
        .arg("--request")
        .arg(file.path())
        .args(["--repository", source.repository, "--json"]);
    if let Some(revision) = source.revision {
        command.args(["--revision", revision]);
    }
    let out = command.output().map_err(|e| unreadable(e.to_string()))?;
    if out.status.success() {
        return Ok(out.stdout);
    }
    let verdict: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    // An incomplete page is a finding (exit 1), and the page is the verdict's
    // `report`. Its bytes are the producer's canonical document, rendered by
    // the producer's own writer, which is what a successful exit prints.
    if verdict.get("outcome").and_then(Value::as_str) == Some("finding")
        && let Some(report) = verdict.get("report")
    {
        let page: ContextPacket = serde_json::from_value(report.clone())
            .map_err(|e| unreadable(format!("the finding's report: {e}")))?;
        return spec_spine_core::context_packet_document(&page)
            .map(String::into_bytes)
            .map_err(|e| unreadable(e.to_string()));
    }
    let text = |path: &[&str]| {
        path.iter()
            .try_fold(&verdict, |v, k| v.get(k))
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    Err(Refusal {
        kind: text(&["error", "kind"]).unwrap_or_else(|| "unreadable".into()),
        message: text(&["error", "message"]).unwrap_or_else(|| {
            let shown = |b: &[u8]| {
                let t = String::from_utf8_lossy(b);
                t.trim().chars().take(200).collect::<String>()
            };
            format!(
                "not a verdict (exit {:?}): stdout {:?}, stderr {:?}",
                out.status.code(),
                shown(&out.stdout),
                shown(&out.stderr)
            )
        }),
        exit: out.status.code(),
    })
}

/// Why a page set could not be assembled.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AssembleError {
    /// The producer refused or failed.
    #[error("context-producer-{}: {}", .0.kind, .0.message)]
    Producer(Refusal),
    /// A page did not decode or validate.
    #[error("context-page-{0}")]
    Page(String),
    /// The page chain would exceed the bounds.
    #[error("context-pages: more than {0} pages")]
    TooManyPages(usize),
}

/// Assemble every page for `request`, following each continuation to the
/// terminal page (section 3.1), and decode each one.
pub fn assemble(
    source: &Source<'_>,
    request: &Value,
    delivery: Intended,
) -> Result<Vec<AcceptedPage>, AssembleError> {
    let mut request = request.clone();
    if let Some(map) = request.as_object_mut() {
        map.remove("continuation");
    }
    let mut pages = Vec::new();
    loop {
        if pages.len() == BOUNDS.pages {
            return Err(AssembleError::TooManyPages(BOUNDS.pages));
        }
        let bytes = one_page(source, &request).map_err(AssembleError::Producer)?;
        let facts = match decode(&bytes) {
            Decoded::Valid(f) => *f,
            Decoded::Invalid(r) => return Err(AssembleError::Page(format!("invalid: {r}"))),
            Decoded::Unsupported(r) => {
                return Err(AssembleError::Page(format!("unsupported: {r}")));
            }
            Decoded::Unreadable(r) => {
                return Err(AssembleError::Page(format!("unreadable: {r}")));
            }
        };
        let token = serde_json::from_slice::<Value>(&bytes)
            .ok()
            .and_then(|v| v.get("continuation").cloned())
            .filter(|v| !v.is_null());
        pages.push(AcceptedPage {
            bytes,
            source_operation: SOURCE_OPERATION.into(),
            facts,
            delivery,
        });
        match token {
            Some(token) => request["continuation"] = token,
            None => return Ok(pages),
        }
    }
}

/// The producer's answer to a recorded closure request (section 3.7.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Closure {
    /// Re-resolved at the recorded snapshot: the current digest, and every
    /// closure member present in one producer document and not the other.
    Answered {
        /// The producer's current closure digest.
        digest: String,
        /// The member differences, as canonical member JSON.
        differences: Vec<String>,
    },
    /// The producer refused or failed, or the page could not be read.
    Refused(Refusal),
}

fn members(closure: &Value) -> Vec<String> {
    closure
        .get("members")
        .and_then(Value::as_array)
        .map(|m| m.iter().map(Value::to_string).collect())
        .unwrap_or_default()
}

/// Re-submit the request a page recorded, at the exact revision it recorded,
/// and return the producer's closure answer.
pub fn closure(program: &Path, repo: &Path, page: &[u8]) -> Closure {
    let refused = |kind: &str, message: String| {
        Closure::Refused(Refusal {
            kind: kind.into(),
            message,
            exit: None,
        })
    };
    let recorded = match validated(page) {
        Ok(p) => p,
        Err(Decoded::Unsupported(r)) => {
            return refused("refused", format!("unsupported-schema: {r}"));
        }
        Err(d) => return refused("unreadable", format!("{d:?}")),
    };
    let request = serde_json::to_value(&recorded.request).expect("a request serializes");
    let source = Source {
        program,
        repo,
        repository: &recorded.snapshot.repository,
        revision: Some(&recorded.snapshot.revision),
    };
    match one_page(&source, &request) {
        Err(r) => Closure::Refused(r),
        Ok(bytes) => match validated(&bytes) {
            Err(d) => refused("unreadable", format!("{d:?}")),
            Ok(current) => {
                let before = members(&recorded.closure);
                let after = members(&current.closure);
                let mut differences: Vec<String> = before
                    .iter()
                    .filter(|m| !after.contains(m))
                    .chain(after.iter().filter(|m| !before.contains(m)))
                    .cloned()
                    .collect();
                differences.sort();
                Closure::Answered {
                    digest: current.closure_digest,
                    differences,
                }
            }
        },
    }
}

/// The producer's binding of the current clean working tree at HEAD
/// (section 3.7.3), under the request a page recorded. `Err` names why it is
/// unverified: dirty, absent, unidentifiable or refused.
pub fn current(
    program: &Path,
    repo: &Path,
    repository: &str,
    page: &[u8],
) -> Result<Snapshot, String> {
    let recorded = validated(page).map_err(|d| format!("the recorded page: {d:?}"))?;
    let request = serde_json::to_value(&recorded.request).expect("a request serializes");
    let source = Source {
        program,
        repo,
        repository,
        revision: None,
    };
    let bytes = one_page(&source, &request).map_err(|r| r.to_string())?;
    let current = validated(&bytes).map_err(|d| format!("the current page: {d:?}"))?;
    Ok(snapshot(&current))
}

/// What a page holds under a member identity (section 3.7.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Member {
    /// The page carries it.
    Found {
        /// The member's content digest.
        content_digest: String,
        /// Its first line.
        start_line: usize,
        /// Its last line.
        end_line: usize,
    },
    /// The page omits it.
    Omitted,
    /// The page neither carries nor omits it.
    Absent,
    /// The page did not validate.
    Unreadable(String),
}

/// Look `identity` up in a validated page.
pub fn member(page: &[u8], identity: &str) -> Member {
    let packet = match validated(page) {
        Ok(p) => p,
        Err(d) => return Member::Unreadable(format!("{d:?}")),
    };
    if let Some(m) = packet.members.iter().find(|m| m.identity == identity) {
        return Member::Found {
            content_digest: m.item.digest.clone(),
            start_line: m.item.span.start_line,
            end_line: m.item.span.end_line,
        };
    }
    if packet.omissions.iter().any(|o| o.identity == identity) {
        return Member::Omitted;
    }
    Member::Absent
}

/// A checkout path the operator wrote in their own task or accept file,
/// resolved against that file's directory.
///
/// This is not a containment boundary and never sees producer data: an
/// operator's checkouts may live anywhere, so an absolute path or one with
/// `..` is taken as written. Packet content never reaches it; a packet is
/// never followed to a path (section 3.4).
pub fn operator_path(base: &Path, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    }
}
