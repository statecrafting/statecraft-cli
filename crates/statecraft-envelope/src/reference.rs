//! The reference: preserved bytes, their digest, and the construction that
//! produced it. Spec 005 section 3.13; statecraft-cli spec 005 section 3.7 is
//! what it must keep saying.
//!
//! Field names, their order and the `Construction` spellings are the ones
//! statecraft-cli established (read out of `ee0fa7d` and unchanged since).
//! Every addition here is optional and omitted when absent, so a reference
//! written by this product serializes exactly as it did before this crate
//! owned the type. `statecraft-acceptance::evidence` re-exports it.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::hash::sha256_hex;

/// How a digest was produced. Part of a reference's identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Construction {
    /// SHA-256 over the file's bytes exactly as stored (`file-bytes-sha256`).
    FileBytesSha256,
    /// SHA-256 over a canonicalized record; never a substitute for the above.
    CanonicalRecordSha256 {
        /// The canonicalization rule's version.
        canonicalization_version: String,
    },
    /// The platform's native copy: BLAKE3-256 over the stored bytes within a scope (added).
    StatecraftObjectBlake3 {
        /// The repository scope the object is kept in.
        scope: String,
    },
    /// A construction this build does not know; preserved and reported `unknown`.
    Unknown(serde_json::Value),
}

impl Construction {
    /// The name this construction is written under.
    pub fn name(&self) -> String {
        match self {
            Construction::FileBytesSha256 => "file-bytes-sha256".to_string(),
            Construction::CanonicalRecordSha256 {
                canonicalization_version,
            } => {
                format!("canonical-record-sha256/{canonicalization_version}")
            }
            Construction::StatecraftObjectBlake3 { scope } => {
                format!("statecraft-object-blake3/{scope}")
            }
            Construction::Unknown(v) => format!("unknown:{v}"),
        }
    }
}

impl Serialize for Construction {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Construction::FileBytesSha256 => s.serialize_str("file-bytes-sha256"),
            Construction::CanonicalRecordSha256 { canonicalization_version } => {
                serde_json::json!({ "canonical-record-sha256": { "canonicalization_version": canonicalization_version } }).serialize(s)
            }
            Construction::StatecraftObjectBlake3 { scope } => {
                serde_json::json!({ "statecraft-object-blake3": { "scope": scope } }).serialize(s)
            }
            Construction::Unknown(v) => v.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for Construction {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        Ok(match &v {
            serde_json::Value::String(s) if s == "file-bytes-sha256" => {
                Construction::FileBytesSha256
            }
            serde_json::Value::Object(o) if o.len() == 1 => {
                if let Some(serde_json::Value::Object(inner)) = o.get("canonical-record-sha256") {
                    match inner
                        .get("canonicalization_version")
                        .and_then(|x| x.as_str())
                    {
                        Some(cv) => Construction::CanonicalRecordSha256 {
                            canonicalization_version: cv.to_string(),
                        },
                        None => Construction::Unknown(v),
                    }
                } else if let Some(serde_json::Value::Object(inner)) =
                    o.get("statecraft-object-blake3")
                {
                    match inner.get("scope").and_then(|x| x.as_str()) {
                        Some(sc) => Construction::StatecraftObjectBlake3 {
                            scope: sc.to_string(),
                        },
                        None => Construction::Unknown(v),
                    }
                } else {
                    Construction::Unknown(v)
                }
            }
            _ => Construction::Unknown(v),
        })
    }
}

/// Where an embedded record sits inside a container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Embedded {
    /// The container's type.
    pub container: String,
    /// How the record is selected from it.
    pub selector: String,
}

/// A producer's own digest, with its construction named (G-07).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProducerDigest {
    /// The construction.
    pub construction: Construction,
    /// The value, lowercase hex.
    pub value: String,
}

/// The platform's kept copy (added).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Native {
    /// The repository scope.
    pub scope: String,
    /// `<codec>:<hex>`.
    pub cid: String,
}

/// A Git object identifier with its format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitOid {
    /// `sha1` or `sha256`.
    pub format: String,
    /// The hex object id.
    pub oid: String,
}

/// A Git subject: repository, commit, tree where known (G-07).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitSubject {
    /// The origin with userinfo removed. A claim, not a proof.
    pub repository: String,
    /// The commit.
    pub commit: GitOid,
    /// The tree, where known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree: Option<GitOid>,
}

/// A reference to preserved evidence bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    /// What kind of evidence this is.
    pub evidence_type: String,
    /// The schema version it was written under.
    pub schema_version: String,
    /// The digest under `construction`, lowercase hex.
    pub digest: String,
    /// Byte length.
    pub bytes: u64,
    /// The construction that produced the digest.
    pub construction: Construction,
    /// Set when the evidence is embedded in a container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedded: Option<Embedded>,
    /// The producer's own digest, if it declares one (added).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer_digest: Option<ProducerDigest>,
    /// The platform's kept copy (added).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native: Option<Native>,
    /// What the record says it is about (added).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<GitSubject>,
}

impl Reference {
    /// A reference over raw file bytes, exactly as the CLI computes it.
    pub fn over_file_bytes(evidence_type: &str, schema_version: &str, content: &[u8]) -> Self {
        Reference {
            evidence_type: evidence_type.to_string(),
            schema_version: schema_version.to_string(),
            digest: sha256_hex(content),
            bytes: content.len() as u64,
            construction: Construction::FileBytesSha256,
            embedded: None,
            producer_digest: None,
            native: None,
            subject: None,
        }
    }

    /// Whether this build can check the construction.
    pub fn checkable_here(&self) -> bool {
        matches!(
            self.construction,
            Construction::FileBytesSha256 | Construction::StatecraftObjectBlake3 { .. }
        )
    }

    /// Whether bytes still match, under the construction named. A canonical
    /// construction is never answered with a file-byte hash.
    pub fn matches(&self, content: &[u8]) -> bool {
        match &self.construction {
            Construction::FileBytesSha256 => {
                sha256_hex(content) == self.digest && content.len() as u64 == self.bytes
            }
            Construction::StatecraftObjectBlake3 { .. } => {
                crate::hash::Hash::of(content).to_hex() == self.digest
                    && content.len() as u64 == self.bytes
            }
            Construction::CanonicalRecordSha256 { .. } | Construction::Unknown(_) => false,
        }
    }

    /// The identity tuple (resolution R-1).
    pub fn identity(&self) -> (String, String, String, String) {
        (
            self.evidence_type.clone(),
            self.schema_version.clone(),
            self.construction.name(),
            self.digest.clone(),
        )
    }
}
