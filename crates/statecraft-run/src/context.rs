//! Context-packet consumption in the run record: spec 019, through its
//! additive `extends` edge on this crate (spec 003 owns it; spec 019 owns no
//! code).
//!
//! Four things live here, and none of them reads a packet's content:
//!
//! - [`scan`] is section 3.4's bounded handling: total bytes, page count,
//!   member count, nesting depth and string length, applied to the bytes
//!   before anything decodes them.
//! - [`Manifest::build`] composes one `statecraft/context-task-manifest/1`
//!   document from producer-reported facts (section 3.1) and refuses facts
//!   that do not repeat consistently (section 3.3). The facts come from the
//!   one crate that links the producer, `statecraft-home`; this crate decodes
//!   no packet and computes no producer digest.
//! - [`retain`] keeps the exact accepted bytes, write-once, under their
//!   custody digest (section 3.4).
//! - [`fold_delivery`] and [`project`] keep intended delivery and
//!   adapter-observed delivery as two collections (section 3.5), reading
//!   nothing but the chain.
//!
//! The manifest is written into the attempt's `prepare-workspace` intent under
//! `contextManifest` (section 3.6). Delivery observations are
//! [`Kind::Evidence`] records under [`DELIVERY_SUBJECT`], which no earlier
//! fold reads.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use statecraft_envelope::hash::sha256_hex;

use crate::record::{Chain, Entry, Identity, Kind, RecordError};
use crate::session::INTENT_SUBJECT;

/// The manifest's closed schema word.
pub const MANIFEST_SCHEMA: &str = "statecraft/context-task-manifest/1";
/// The named construction the manifest digest is computed by.
pub const DIGEST_CONSTRUCTION: &str = "statecraft/context-manifest-digest/1";
/// Where the manifest sits in the attempt's intent detail.
pub const INTENT_KEY: &str = "contextManifest";
/// The subject of an adapter-observed delivery record.
pub const DELIVERY_SUBJECT: &str = "context-delivery";
/// The one delivery channel version 1 supports (section 3.5).
pub const CHANNEL: &str = "adapter-request-bundle";

/// Section 3.4's limits, applied before any decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// Bytes across every page of one manifest.
    pub total_bytes: usize,
    /// Pages across every repository of one manifest.
    pub pages: usize,
    /// Object members and array elements in one page.
    pub members: usize,
    /// Container nesting in one page.
    pub depth: usize,
    /// Raw bytes inside one string literal.
    pub string_bytes: usize,
}

/// The limits this product applies.
pub const BOUNDS: Bounds = Bounds {
    total_bytes: 16 * 1024 * 1024,
    pages: 256,
    members: 262_144,
    depth: 64,
    string_bytes: 4 * 1024 * 1024,
};

/// A page or a page set that exceeds [`Bounds`], or is not text at all.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BoundsError {
    /// Too many bytes across the set.
    #[error("context packets exceed {limit} bytes")]
    TotalBytes {
        /// The limit.
        limit: usize,
    },
    /// Too many pages across the set.
    #[error("context packets exceed {limit} pages")]
    Pages {
        /// The limit.
        limit: usize,
    },
    /// One page has too many members.
    #[error("page {page} exceeds {limit} members")]
    Members {
        /// The page's position.
        page: usize,
        /// The limit.
        limit: usize,
    },
    /// One page nests too deeply.
    #[error("page {page} nests deeper than {limit}")]
    Depth {
        /// The page's position.
        page: usize,
        /// The limit.
        limit: usize,
    },
    /// One page has a string that is too long.
    #[error("page {page} has a string longer than {limit} bytes")]
    StringBytes {
        /// The page's position.
        page: usize,
        /// The limit.
        limit: usize,
    },
    /// One page is not balanced text a decoder could read within the bounds.
    #[error("page {page} is not bounded JSON text")]
    Malformed {
        /// The page's position.
        page: usize,
    },
}

/// Apply `bounds` to every page, before anything decodes one.
///
/// This is a scanner, not a parser: it tracks strings, escapes and container
/// depth, and counts the separators that introduce a member or an element,
/// which bounds both from above. It never builds a value, so a hostile page
/// costs one pass over its bytes and no allocation proportional to them.
pub fn scan(pages: &[&[u8]], bounds: &Bounds) -> Result<(), BoundsError> {
    if pages.len() > bounds.pages {
        return Err(BoundsError::Pages {
            limit: bounds.pages,
        });
    }
    let total: usize = pages.iter().map(|p| p.len()).sum();
    if total > bounds.total_bytes {
        return Err(BoundsError::TotalBytes {
            limit: bounds.total_bytes,
        });
    }
    for (page, bytes) in pages.iter().enumerate() {
        scan_page(page, bytes, bounds)?;
    }
    Ok(())
}

fn scan_page(page: usize, bytes: &[u8], bounds: &Bounds) -> Result<(), BoundsError> {
    let mut depth = 0usize;
    let mut members = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut string_len = 0usize;
    for &b in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
                continue;
            }
            string_len += 1;
            if string_len > bounds.string_bytes {
                return Err(BoundsError::StringBytes {
                    page,
                    limit: bounds.string_bytes,
                });
            }
            continue;
        }
        match b {
            b'"' => {
                in_string = true;
                string_len = 0;
            }
            b'{' | b'[' => {
                depth += 1;
                if depth > bounds.depth {
                    return Err(BoundsError::Depth {
                        page,
                        limit: bounds.depth,
                    });
                }
                members += 1;
            }
            b'}' | b']' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or(BoundsError::Malformed { page })?;
            }
            b',' | b':' => members += 1,
            _ => {}
        }
        if members > bounds.members {
            return Err(BoundsError::Members {
                page,
                limit: bounds.members,
            });
        }
    }
    if in_string || depth != 0 {
        return Err(BoundsError::Malformed { page });
    }
    Ok(())
}

/// `required` or `optional`, for a repository and for an omitted member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Requirement {
    /// The task cannot run without it.
    Required,
    /// The task may run without it, when the policy permits the absence.
    Optional,
}

/// What a page's producer says about its own completeness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Completeness {
    /// Every requested member is present.
    Complete,
    /// Only optional members are absent.
    Partial,
    /// A required member is absent.
    Incomplete,
}

/// The intended delivery of one page (section 3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Intended {
    /// The page must be made visible to the provider.
    Required,
    /// The page may be made visible.
    Optional,
    /// The page is retained and deliberately not delivered.
    Withheld,
}

/// The producer-reported repository snapshot a packet is bound to.
///
/// Every member is the producer's string. None is minted, normalized or
/// shared with another repository here (section 3.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    /// The producer's repository identity.
    pub repository: String,
    /// The revision.
    pub revision: String,
    /// The tree.
    pub tree: String,
    /// The producer's dirty-state word.
    pub dirty_state: String,
}

/// The build a page names as its producer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProducerBuild {
    /// The package.
    pub package: String,
    /// Its version.
    pub version: String,
    /// The build identity the page records, which may be `unverified`.
    pub build: String,
}

/// One omitted member, as its page reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OmissionFact {
    /// The member's canonical identity.
    pub identity: String,
    /// Whether the request required it.
    pub requirement: Requirement,
    /// The producer's reason word.
    pub reason: String,
}

/// One warning, as its page reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WarningFact {
    /// The producer's warning code.
    pub code: String,
    /// The member it names, if it names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,
}

/// What the producer reports about one page, decoded by the caller from the
/// page's exact bytes under the adopted schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageFacts {
    /// The page's schema version.
    pub schema_version: String,
    /// The build the page names.
    pub producer: ProducerBuild,
    /// The snapshot the page is bound to.
    pub snapshot: Snapshot,
    /// The producer's request digest.
    pub request_digest: String,
    /// The producer's closure digest.
    pub closure_digest: String,
    /// The producer's page digest.
    pub packet_digest: String,
    /// The page's completeness.
    pub completeness: Completeness,
    /// Whether the page carries a continuation to a further page.
    pub continues: bool,
    /// The page's omissions, in its order.
    pub omissions: Vec<OmissionFact>,
    /// The page's warnings, in its order.
    pub warnings: Vec<WarningFact>,
}

/// One page as the caller accepted it: its exact bytes and their facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedPage {
    /// The exact response bytes.
    pub bytes: Vec<u8>,
    /// The producer operation that returned them.
    pub source_operation: String,
    /// What the producer reports in them.
    pub facts: PageFacts,
    /// Whether the page is to be delivered.
    pub delivery: Intended,
}

/// One repository's packet as the caller accepted it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedRepository {
    /// The stable local repository id.
    pub id: String,
    /// Its role in the task.
    pub role: String,
    /// Whether the task requires it.
    pub requirement: Requirement,
    /// Its pages, in the producer's order.
    pub pages: Vec<AcceptedPage>,
}

/// The adopted producer and the packet interface it is consumed through.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Producer {
    /// The package.
    pub package: String,
    /// The exact adopted version.
    pub version: String,
    /// The packet schema version the consumer supports.
    pub packet_interface: String,
}

/// The task the manifest belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Task {
    /// No task contract supplied one (section 3.1 member 3).
    NotRecorded,
    /// The task contract's identity.
    Id(String),
}

/// Whether an optional repository's recorded omission may stay eligible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OptionalOmission {
    /// An incomplete optional repository leaves the manifest eligible.
    Permit,
    /// It makes the manifest ineligible.
    Refuse,
}

/// The closed delivery policy (section 3.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliveryPolicy {
    /// Always [`CHANNEL`].
    pub channel: String,
    /// What an incomplete optional repository does to eligibility.
    pub optional_omission: OptionalOmission,
}

impl DeliveryPolicy {
    /// The one version 1 channel, with the given omission rule.
    pub fn new(optional_omission: OptionalOmission) -> Self {
        Self {
            channel: CHANNEL.to_string(),
            optional_omission,
        }
    }
}

/// One omission, by its position in its retained page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OmissionRef {
    /// The position in the page's `omissions`.
    pub index: usize,
    /// What the page reports there.
    pub fact: OmissionFact,
}

/// One warning, by its position in its retained page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WarningRef {
    /// The position in the page's `warnings`.
    pub index: usize,
    /// What the page reports there.
    pub fact: WarningFact,
}

/// One retained page in the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Page {
    /// The page's position in its repository, from 1.
    pub ordinal: usize,
    /// The producer's page digest, carried, never recomputed here.
    pub page_digest: String,
    /// The exact retained byte length.
    pub byte_length: usize,
    /// Statecraft's custody digest over the retained bytes.
    pub custody_digest: String,
    /// The producer operation that returned the bytes.
    pub source_operation: String,
    /// The build the page names.
    pub producer_build: ProducerBuild,
    /// Its intended delivery.
    pub delivery: Intended,
    /// Its omissions, every one, by reference.
    pub omissions: Vec<OmissionRef>,
    /// Its warnings, every one, by reference.
    pub warnings: Vec<WarningRef>,
}

/// One repository in the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Repository {
    /// The stable local repository id.
    pub id: String,
    /// Its role in the task.
    pub role: String,
    /// Whether the task requires it.
    pub requirement: Requirement,
    /// The producer-reported snapshot, this repository's alone.
    pub snapshot: Snapshot,
    /// The packet schema version.
    pub packet_schema_version: String,
    /// The producer's request digest.
    pub request_digest: String,
    /// The producer's closure digest.
    pub closure_digest: String,
    /// The terminal continuation state; always `terminal` once built.
    pub continuation: String,
    /// The least complete page's completeness.
    pub completeness: Completeness,
    /// The pages, in the producer's order.
    pub pages: Vec<Page>,
}

/// One `statecraft/context-task-manifest/1` document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    /// Always [`MANIFEST_SCHEMA`].
    pub schema: String,
    /// The run.
    pub run_id: String,
    /// The attempt, from 1.
    pub attempt: u32,
    /// The task.
    pub task: Task,
    /// The adopted producer.
    pub producer: Producer,
    /// The repositories, in the caller's order.
    pub repositories: Vec<Repository>,
    /// The delivery policy.
    pub delivery: DeliveryPolicy,
    /// The digest of this composition, by [`DIGEST_CONSTRUCTION`].
    pub manifest_digest: String,
}

/// Why a manifest could not be composed. Each carries a stable code word.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BuildError {
    /// The run id is empty or the attempt is zero.
    #[error("invalid-binding: {0}")]
    Binding(String),
    /// No repository was supplied.
    #[error("empty-manifest: a task manifest names at least one repository")]
    Empty,
    /// A repository id is empty, unsafe or repeated.
    #[error("repository-id: {0}")]
    RepositoryId(String),
    /// A repository supplied no page.
    #[error("no-pages: repository {0} supplied no packet page")]
    NoPages(String),
    /// A page appears twice.
    #[error("duplicate-page: {0} appears more than once")]
    DuplicatePage(String),
    /// A page names another producer than the adopted one.
    #[error("producer-mismatch: repository {0}")]
    ProducerMismatch(String),
    /// Pages of one repository do not repeat their identities.
    #[error("inconsistent-page: repository {repository}, page {ordinal}: {field}")]
    Inconsistent {
        /// The repository.
        repository: String,
        /// The page.
        ordinal: usize,
        /// The identity that differed.
        field: &'static str,
    },
    /// The page chain does not end at a terminal page.
    #[error("not-terminal: repository {0}")]
    NotTerminal(String),
    /// The bytes exceed the bounds.
    #[error(transparent)]
    Bounds(#[from] BoundsError),
}

/// Why a built manifest may not execute (section 3.2).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{reason}: repository {repository}")]
pub struct Ineligible {
    /// The repository.
    pub repository: String,
    /// `required-incomplete` or `optional-incomplete`.
    pub reason: &'static str,
}

/// Custody: `sha256:` over exactly the retained bytes.
pub fn custody_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", sha256_hex(bytes))
}

fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

impl Manifest {
    /// Compose a manifest for one attempt.
    ///
    /// Applies [`BOUNDS`] to every page's bytes, then requires each
    /// repository's pages to repeat their schema, producer build, snapshot,
    /// request digest and closure digest, to name the adopted producer, and to
    /// end at exactly one terminal page. A page digest may appear once in the
    /// whole manifest. Nothing is recomputed: every digest but the custody
    /// digest and the manifest digest is the producer's.
    pub fn build(
        run_id: &str,
        attempt: u32,
        task: Task,
        producer: &Producer,
        repositories: &[AcceptedRepository],
        delivery: DeliveryPolicy,
    ) -> Result<Self, BuildError> {
        if run_id.is_empty() || attempt == 0 {
            return Err(BuildError::Binding(format!(
                "run {run_id:?} attempt {attempt}"
            )));
        }
        if repositories.is_empty() {
            return Err(BuildError::Empty);
        }
        let all: Vec<&[u8]> = repositories
            .iter()
            .flat_map(|r| r.pages.iter().map(|p| p.bytes.as_slice()))
            .collect();
        scan(&all, &BOUNDS)?;

        let mut ids: Vec<&str> = Vec::new();
        let mut digests: Vec<&str> = Vec::new();
        let mut out = Vec::with_capacity(repositories.len());
        for repo in repositories {
            if !safe_id(&repo.id) || ids.contains(&repo.id.as_str()) {
                return Err(BuildError::RepositoryId(repo.id.clone()));
            }
            ids.push(&repo.id);
            let Some(first) = repo.pages.first() else {
                return Err(BuildError::NoPages(repo.id.clone()));
            };
            let head = &first.facts;
            let mut pages = Vec::with_capacity(repo.pages.len());
            for (i, page) in repo.pages.iter().enumerate() {
                let f = &page.facts;
                let ordinal = i + 1;
                let inconsistent = |field| BuildError::Inconsistent {
                    repository: repo.id.clone(),
                    ordinal,
                    field,
                };
                if f.producer.package != producer.package || f.producer.version != producer.version
                {
                    return Err(BuildError::ProducerMismatch(repo.id.clone()));
                }
                if f.schema_version != head.schema_version {
                    return Err(inconsistent("schemaVersion"));
                }
                if f.producer != head.producer {
                    return Err(inconsistent("producer"));
                }
                if f.snapshot != head.snapshot {
                    return Err(inconsistent("snapshot"));
                }
                if f.request_digest != head.request_digest {
                    return Err(inconsistent("requestDigest"));
                }
                if f.closure_digest != head.closure_digest {
                    return Err(inconsistent("closureDigest"));
                }
                let last = ordinal == repo.pages.len();
                if f.continues == last {
                    return Err(BuildError::NotTerminal(repo.id.clone()));
                }
                if digests.contains(&f.packet_digest.as_str()) {
                    return Err(BuildError::DuplicatePage(f.packet_digest.clone()));
                }
                digests.push(&f.packet_digest);
                pages.push(Page {
                    ordinal,
                    page_digest: f.packet_digest.clone(),
                    byte_length: page.bytes.len(),
                    custody_digest: custody_digest(&page.bytes),
                    source_operation: page.source_operation.clone(),
                    producer_build: f.producer.clone(),
                    delivery: page.delivery,
                    omissions: f
                        .omissions
                        .iter()
                        .enumerate()
                        .map(|(index, fact)| OmissionRef {
                            index,
                            fact: fact.clone(),
                        })
                        .collect(),
                    warnings: f
                        .warnings
                        .iter()
                        .enumerate()
                        .map(|(index, fact)| WarningRef {
                            index,
                            fact: fact.clone(),
                        })
                        .collect(),
                });
            }
            let completeness = repo
                .pages
                .iter()
                .map(|p| p.facts.completeness)
                .max()
                .unwrap_or(Completeness::Complete);
            out.push(Repository {
                id: repo.id.clone(),
                role: repo.role.clone(),
                requirement: repo.requirement,
                snapshot: head.snapshot.clone(),
                packet_schema_version: head.schema_version.clone(),
                request_digest: head.request_digest.clone(),
                closure_digest: head.closure_digest.clone(),
                continuation: "terminal".to_string(),
                completeness,
                pages,
            });
        }
        let mut manifest = Manifest {
            schema: MANIFEST_SCHEMA.to_string(),
            run_id: run_id.to_string(),
            attempt,
            task,
            producer: producer.clone(),
            repositories: out,
            delivery,
            manifest_digest: String::new(),
        };
        manifest.manifest_digest = manifest.computed_digest();
        Ok(manifest)
    }

    /// The digest [`DIGEST_CONSTRUCTION`] names: `sha256:` over the
    /// construction word, one LF, and the document's JSON with
    /// `manifestDigest` empty. Every collection in the document is ordered
    /// and every object is a struct, so the bytes are fixed by the value.
    pub fn computed_digest(&self) -> String {
        let mut unsigned = self.clone();
        unsigned.manifest_digest = String::new();
        let mut bytes = DIGEST_CONSTRUCTION.as_bytes().to_vec();
        bytes.push(b'\n');
        bytes.extend(serde_json::to_vec(&unsigned).expect("a manifest serializes"));
        format!("sha256:{}", sha256_hex(&bytes))
    }

    /// The manifest's exact bytes, as the adapter request carries them.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a manifest serializes")
    }

    /// Section 3.2: whether this manifest may execute.
    ///
    /// A required repository that is not complete never may. An incomplete
    /// optional repository may only when the policy permits that omission.
    pub fn eligibility(&self) -> Result<(), Ineligible> {
        for repo in &self.repositories {
            let complete = repo.completeness == Completeness::Complete;
            match repo.requirement {
                Requirement::Required if !complete => {
                    return Err(Ineligible {
                        repository: repo.id.clone(),
                        reason: "required-incomplete",
                    });
                }
                Requirement::Optional
                    if !complete && self.delivery.optional_omission == OptionalOmission::Refuse =>
                {
                    return Err(Ineligible {
                        repository: repo.id.clone(),
                        reason: "optional-incomplete",
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Every page, with its repository id, in manifest order.
    pub fn pages(&self) -> impl Iterator<Item = (&str, &Page)> {
        self.repositories
            .iter()
            .flat_map(|r| r.pages.iter().map(move |p| (r.id.as_str(), p)))
    }
}

/// Why an adapter request does not carry the manifest its intent recorded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RequestMismatch {
    /// The request's bytes do not decode as a manifest.
    #[error("context-request-unreadable: {0}")]
    Unreadable(String),
    /// The request's stated digest is not the recorded one.
    #[error("context-request-mismatch: digest")]
    Digest,
    /// The request's manifest is not the recorded one.
    #[error("context-request-mismatch: manifest")]
    Manifest,
}

/// Section 3.6: the request must carry exactly the recorded manifest, under
/// exactly its digest, or the attempt refuses before spawn.
pub fn verify_request(
    recorded: &Manifest,
    bytes: &[u8],
    digest: &str,
) -> Result<(), RequestMismatch> {
    if digest != recorded.manifest_digest {
        return Err(RequestMismatch::Digest);
    }
    let carried: Manifest =
        serde_json::from_slice(bytes).map_err(|e| RequestMismatch::Unreadable(e.to_string()))?;
    if &carried != recorded || carried.computed_digest() != recorded.manifest_digest {
        return Err(RequestMismatch::Manifest);
    }
    Ok(())
}

/// Why retained bytes could not be kept or read back.
#[derive(Debug, thiserror::Error)]
pub enum CustodyError {
    /// A filesystem operation failed.
    #[error("context custody i/o at {path}: {source}")]
    Io {
        /// Where.
        path: PathBuf,
        /// Why.
        source: std::io::Error,
    },
    /// A file under a custody digest holds other bytes.
    #[error("context custody mismatch at {0}")]
    Mismatch(PathBuf),
}

/// Where the bytes under `custody` are kept in the product home.
pub fn custody_path(home: &Path, custody: &str) -> PathBuf {
    let hex = custody.strip_prefix("sha256:").unwrap_or(custody);
    home.join("context").join(format!("{hex}.json"))
}

/// Keep `bytes` under their custody digest, write-once.
///
/// An existing file is accepted only when it holds the same bytes, so a
/// retry that reuses a page names the bytes it already retained and never
/// replaces them.
pub fn retain(home: &Path, bytes: &[u8]) -> Result<String, CustodyError> {
    use std::io::Write;
    let custody = custody_digest(bytes);
    let path = custody_path(home, &custody);
    let io = |source| CustodyError::Io {
        path: path.clone(),
        source,
    };
    let parent = path.parent().expect("custody path has a parent");
    std::fs::create_dir_all(parent).map_err(io)?;
    if path.exists() {
        return match read_retained(home, &custody)? {
            Some(_) => Ok(custody),
            None => Err(CustodyError::Mismatch(path)),
        };
    }
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let temporary = parent.join(format!(
        ".statecraft-partial-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let written = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        use rustix::fs::{CWD, RenameFlags, renameat_with};
        match renameat_with(CWD, &temporary, CWD, &path, RenameFlags::NOREPLACE) {
            Ok(()) => Ok(()),
            Err(rustix::io::Errno::EXIST) => Ok(()),
            Err(rustix::io::Errno::INVAL | rustix::io::Errno::NOSYS) => {
                match std::fs::hard_link(&temporary, &path) {
                    // A racing writer published first; the read-back below
                    // decides whether it published these bytes.
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
                    other => other,
                }
            }
            Err(e) => Err(std::io::Error::from(e)),
        }
    })();
    let _ = std::fs::remove_file(&temporary);
    written.map_err(io)?;
    match read_retained(home, &custody)? {
        Some(_) => Ok(custody),
        None => Err(CustodyError::Mismatch(path)),
    }
}

/// The bytes kept under `custody`: `None` when they are absent or are not
/// the bytes the digest names.
pub fn read_retained(home: &Path, custody: &str) -> Result<Option<Vec<u8>>, CustodyError> {
    let path = custody_path(home, custody);
    match std::fs::read(&path) {
        Ok(bytes) if custody_digest(&bytes) == custody => Ok(Some(bytes)),
        Ok(_) => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(CustodyError::Io { path, source }),
    }
}

/// The manifest recorded in an attempt's intent, if its intent carries one.
///
/// `Err` when the intent carries a value that is not a manifest, or a
/// manifest for another binding or under another digest: such a value binds
/// nothing and is never read as absent.
pub fn recorded_manifest(
    chain: &Chain,
    run_id: &str,
    attempt: u32,
) -> Result<Option<Manifest>, String> {
    let entries = chain.entries();
    let intent = entries.iter().find(|e| {
        e.kind == Kind::Intent
            && e.subject == INTENT_SUBJECT
            && e.run_id == run_id
            && e.attempt == attempt
    });
    let Some(value) = intent.and_then(|e| e.detail.get(INTENT_KEY)) else {
        return Ok(None);
    };
    let manifest: Manifest =
        serde_json::from_value(value.clone()).map_err(|e| format!("unreadable manifest: {e}"))?;
    if manifest.run_id != run_id
        || manifest.attempt != attempt
        || manifest.schema != MANIFEST_SCHEMA
        || manifest.computed_digest() != manifest.manifest_digest
    {
        return Err("the recorded manifest does not bind this attempt".to_string());
    }
    Ok(Some(manifest))
}

/// One page the adapter reports it withheld.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Withheld {
    /// The page.
    pub page_digest: String,
    /// The adapter's reason.
    pub reason: String,
}

/// What an adapter's init event says about context delivery: its testimony,
/// not proof that a provider received, retained, parsed or used anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Reported {
    /// The manifest digest the adapter says it applied.
    pub manifest_digest: String,
    /// The pages it says it made provider-visible.
    pub visible: Vec<String>,
    /// The pages it says it withheld.
    pub withheld: Vec<Withheld>,
}

/// One delivery observation the supervisor recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "state", content = "report")]
pub enum Observation {
    /// A trusted init event carried this report.
    Reported(Reported),
    /// The stream held no context report.
    NotReported,
    /// The stream could not be decoded far enough to decide.
    Unreadable(String),
}

/// One page's provider-visible delivery state (section 3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    /// The adapter reports the exact page as made visible.
    ReportedVisible,
    /// The adapter reports that it withheld the page.
    ReportedWithheld,
    /// No trusted event accounts for the page.
    NotReported,
    /// Events report incompatible states or another digest.
    Contradictory,
    /// The stream could not be decoded far enough to decide.
    Unreadable,
}

/// One intended page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntendedPage {
    /// The repository.
    pub repository: String,
    /// The page.
    pub page_digest: String,
    /// What was intended.
    pub delivery: Intended,
}

/// One page as the adapter observed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedPage {
    /// The repository.
    pub repository: String,
    /// The page.
    pub page_digest: String,
    /// Its state.
    pub state: Visibility,
    /// The adapter's withholding reason, when it gave one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Intended and observed delivery, as two collections, plus every finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Delivery {
    /// What the manifest intended.
    pub intended: Vec<IntendedPage>,
    /// What the adapter testified to.
    pub observed: Vec<ObservedPage>,
    /// What the observations got wrong: duplicates, contradictions, another
    /// manifest digest, a page the manifest does not name.
    pub findings: Vec<String>,
    /// The independent transport fact; `not-recorded` until spec 014 admits
    /// a wire artifact for the attempt.
    pub transport: String,
}

fn page_state(report: &Reported, digest: &str) -> (Visibility, Option<String>) {
    let visible = report.visible.iter().filter(|d| *d == digest).count();
    let withheld: Vec<&Withheld> = report
        .withheld
        .iter()
        .filter(|w| w.page_digest == digest)
        .collect();
    match (visible, withheld.as_slice()) {
        (0, []) => (Visibility::NotReported, None),
        (1, []) => (Visibility::ReportedVisible, None),
        (0, [w]) => (Visibility::ReportedWithheld, Some(w.reason.clone())),
        _ => (Visibility::Contradictory, None),
    }
}

/// Fold every observation of one attempt against its manifest.
///
/// The intended set is the manifest's and is never copied into the observed
/// set. A report under another manifest digest binds no page and makes every
/// page `contradictory`. Two reports that disagree on a page make it
/// `contradictory`; two that agree are still a duplicate finding. A report
/// beside an unreadable observation makes every page `contradictory` and is a
/// finding of its own. The more favorable observation is never chosen.
pub fn fold_delivery(manifest: &Manifest, observations: &[Observation]) -> Delivery {
    let mut findings = Vec::new();
    let reports: Vec<&Reported> = observations
        .iter()
        .filter_map(|o| match o {
            Observation::Reported(r) => Some(r),
            _ => None,
        })
        .collect();
    let unreadable = observations
        .iter()
        .any(|o| matches!(o, Observation::Unreadable(_)));
    let foreign = reports
        .iter()
        .any(|r| r.manifest_digest != manifest.manifest_digest);
    if foreign {
        findings.push("manifest-digest-mismatch".to_string());
    }
    if reports.len() > 1 {
        findings.push(format!("duplicate-observation: {}", reports.len()));
    }
    if unreadable && !reports.is_empty() {
        findings
            .push("contradictory-observation: a report beside an unreadable stream".to_string());
    }
    for report in &reports {
        let named = report
            .visible
            .iter()
            .chain(report.withheld.iter().map(|w| &w.page_digest));
        for digest in named {
            if !manifest.pages().any(|(_, p)| &p.page_digest == digest) {
                findings.push(format!("unexpected-page: {digest}"));
            }
        }
    }
    let intended = manifest
        .pages()
        .map(|(repo, p)| IntendedPage {
            repository: repo.to_string(),
            page_digest: p.page_digest.clone(),
            delivery: p.delivery,
        })
        .collect();
    let observed = manifest
        .pages()
        .map(|(repo, p)| {
            let (state, reason) = if foreign {
                (Visibility::Contradictory, None)
            } else if reports.is_empty() {
                if unreadable {
                    (Visibility::Unreadable, None)
                } else {
                    (Visibility::NotReported, None)
                }
            } else {
                let states: Vec<(Visibility, Option<String>)> = reports
                    .iter()
                    .map(|r| page_state(r, &p.page_digest))
                    .collect();
                if unreadable || states.windows(2).any(|w| w[0] != w[1]) {
                    (Visibility::Contradictory, None)
                } else {
                    states[0].clone()
                }
            };
            ObservedPage {
                repository: repo.to_string(),
                page_digest: p.page_digest.clone(),
                state,
                reason,
            }
        })
        .collect();
    Delivery {
        intended,
        observed,
        findings,
        transport: "not-recorded".to_string(),
    }
}

/// Append one delivery observation, bound to the attempt and its manifest.
pub fn record_delivery(
    chain: &mut Chain,
    manifest: &Manifest,
    observation: &Observation,
    timestamp: &str,
) -> Result<(), RecordError> {
    let n = chain.entries().len();
    chain.append(
        &format!(
            "{}/{}/{DELIVERY_SUBJECT}/{n}",
            manifest.run_id, manifest.attempt
        ),
        timestamp,
        &Entry {
            kind: Kind::Evidence,
            run_id: manifest.run_id.clone(),
            attempt: manifest.attempt,
            subject: DELIVERY_SUBJECT.to_string(),
            idempotency_key: None,
            detail: serde_json::json!({
                "binding": {
                    "runId": manifest.run_id,
                    "attempt": manifest.attempt,
                    "manifestDigest": manifest.manifest_digest,
                },
                "observation": observation,
            }),
            effect_id: Identity::Absent,
        },
    )
}

/// Every observation recorded for the manifest's attempt.
///
/// A record whose binding names another run, attempt or manifest digest does
/// not bind and is not returned. A record that binds but whose observation
/// cannot be read is returned as [`Observation::Unreadable`], never dropped.
pub fn recorded_deliveries(chain: &Chain, manifest: &Manifest) -> Vec<Observation> {
    chain
        .entries()
        .into_iter()
        .filter(|e| {
            e.kind == Kind::Evidence
                && e.subject == DELIVERY_SUBJECT
                && e.run_id == manifest.run_id
                && e.attempt == manifest.attempt
        })
        .filter(|e| {
            let b = e.detail.get("binding");
            b.and_then(|b| b.get("runId")).and_then(Value::as_str) == Some(&manifest.run_id)
                && b.and_then(|b| b.get("attempt")).and_then(Value::as_u64)
                    == Some(u64::from(manifest.attempt))
                && b.and_then(|b| b.get("manifestDigest"))
                    .and_then(Value::as_str)
                    == Some(&manifest.manifest_digest)
        })
        .map(|e| {
            e.detail
                .get("observation")
                .cloned()
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_else(|| Observation::Unreadable("unreadable record".to_string()))
        })
        .collect()
}

/// One attempt's context, as `run show` renders it (section 3.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum Projection {
    /// The intent carries no manifest; a historical record renders this.
    NotRecorded,
    /// The intent carries a value that binds nothing.
    Unreadable {
        /// Why.
        reason: String,
    },
    /// The manifest and its delivery.
    Recorded {
        /// The manifest as recorded.
        manifest: Box<Manifest>,
        /// Intended and observed delivery.
        delivery: Delivery,
    },
}

/// Fold one attempt's context from the chain and nothing else.
pub fn project(chain: &Chain, run_id: &str, attempt: u32) -> Projection {
    match recorded_manifest(chain, run_id, attempt) {
        Ok(None) => Projection::NotRecorded,
        Err(reason) => Projection::Unreadable { reason },
        Ok(Some(manifest)) => {
            let observations = recorded_deliveries(chain, &manifest);
            let delivery = fold_delivery(&manifest, &observations);
            Projection::Recorded {
                manifest: Box::new(manifest),
                delivery,
            }
        }
    }
}

/// The acceptance policy's closed schema word.
pub const POLICY_SCHEMA: &str = "statecraft/context-acceptance-policy/1";
/// Where the policy sits in the attempt's intent detail.
pub const POLICY_KEY: &str = "contextPolicy";

/// One of section 3.7's five checks, or delivery (section 3.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Dimension {
    /// Provider-visible delivery of the intended pages.
    Delivery,
    /// Section 3.7.1.
    Integrity,
    /// Section 3.7.2.
    Closure,
    /// Section 3.7.3.
    Freshness,
    /// Section 3.7.4.
    Citations,
    /// Section 3.7.5.
    Evidence,
}

/// Whether a dimension's facts are required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Need {
    /// Anything but the passing state makes acceptance ineligible.
    Required,
    /// A non-passing state stays visible, and coexists with acceptance only
    /// where the policy names that exact absence.
    Optional,
}

/// One exact non-passing state the policy permits for an optional fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermittedAbsence {
    /// The dimension.
    pub dimension: Dimension,
    /// The state word, exactly as the check reports it.
    pub state: String,
}

/// The task's context acceptance policy (section 3.8): recorded in the intent
/// before execution and never replaced by a newer one at acceptance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextPolicy {
    /// Always [`POLICY_SCHEMA`].
    pub schema: String,
    /// Delivery.
    pub delivery: Need,
    /// Packet integrity.
    pub integrity: Need,
    /// Closure.
    pub closure: Need,
    /// Freshness.
    pub freshness: Need,
    /// Citations.
    pub citations: Need,
    /// Evidence.
    pub evidence: Need,
    /// The exact absences an optional fact may carry into acceptance.
    pub permitted_absences: Vec<PermittedAbsence>,
}

impl ContextPolicy {
    /// What `dimension` needs under this policy.
    pub fn need(&self, dimension: Dimension) -> Need {
        match dimension {
            Dimension::Delivery => self.delivery,
            Dimension::Integrity => self.integrity,
            Dimension::Closure => self.closure,
            Dimension::Freshness => self.freshness,
            Dimension::Citations => self.citations,
            Dimension::Evidence => self.evidence,
        }
    }

    /// Whether the policy permits `state` for an optional fact of `dimension`.
    pub fn permits(&self, dimension: Dimension, state: &str) -> bool {
        self.permitted_absences
            .iter()
            .any(|p| p.dimension == dimension && p.state == state)
    }
}

/// The policy recorded in an attempt's intent, if its intent carries one.
///
/// `Err` when the intent carries a value that is not a policy: it binds
/// nothing and is never read as absent, and no newer policy stands in.
pub fn recorded_policy(
    chain: &Chain,
    run_id: &str,
    attempt: u32,
) -> Result<Option<ContextPolicy>, String> {
    let entries = chain.entries();
    let intent = entries.iter().find(|e| {
        e.kind == Kind::Intent
            && e.subject == INTENT_SUBJECT
            && e.run_id == run_id
            && e.attempt == attempt
    });
    let Some(value) = intent.and_then(|e| e.detail.get(POLICY_KEY)) else {
        return Ok(None);
    };
    let policy: ContextPolicy =
        serde_json::from_value(value.clone()).map_err(|e| format!("unreadable policy: {e}"))?;
    if policy.schema != POLICY_SCHEMA {
        return Err(format!("unsupported policy schema {}", policy.schema));
    }
    Ok(Some(policy))
}
