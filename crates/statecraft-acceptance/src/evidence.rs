//! Preserved bytes, digests, and the construction that produced them.
//!
//! Spec 005 section 3.7. A reference to evidence carries: type, schema version,
//! the SHA-256 digest and byte length of the **bytes**, the named hash
//! construction, and where relevant the container and selector for an embedded
//! record.
//!
//! A canonical record hash **never substitutes** for a file-byte digest, and the
//! construction is part of the reference's identity. Any new construction is
//! versioned; historical records are never rewritten to satisfy a newer rule.

use serde::{Deserialize, Serialize};

/// How a digest was produced.
///
/// Part of a reference's identity, not a detail of it. Two references to the
/// same bytes under different constructions are different references, which is
/// what keeps a historical record verifiable under the rule it was written
/// under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Construction {
    /// SHA-256 over the file's bytes exactly as stored.
    FileBytesSha256,
    /// SHA-256 over a canonicalized record.
    ///
    /// **Never a substitute for [`Construction::FileBytesSha256`]**: it answers
    /// a different question, and the two agreeing is a coincidence of the input.
    CanonicalRecordSha256 {
        /// The canonicalization rule's version.
        canonicalization_version: String,
    },
}

impl Construction {
    /// The name this construction is written under.
    pub fn name(&self) -> String {
        match self {
            Construction::FileBytesSha256 => "file-bytes-sha256".to_string(),
            Construction::CanonicalRecordSha256 {
                canonicalization_version,
            } => format!("canonical-record-sha256/{canonicalization_version}"),
        }
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

/// A reference to preserved evidence bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    /// What kind of evidence this is.
    pub evidence_type: String,
    /// The schema version it was written under.
    pub schema_version: String,
    /// SHA-256 of the bytes.
    pub digest: String,
    /// Their length.
    pub bytes: u64,
    /// The construction that produced the digest.
    pub construction: Construction,
    /// Set when the evidence is embedded in a container.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedded: Option<Embedded>,
}

impl Reference {
    /// A reference over raw file bytes.
    pub fn over_file_bytes(evidence_type: &str, schema_version: &str, content: &[u8]) -> Self {
        Self {
            evidence_type: evidence_type.to_string(),
            schema_version: schema_version.to_string(),
            digest: statecraft_environment::digest::digest_bytes(content),
            bytes: content.len() as u64,
            construction: Construction::FileBytesSha256,
            embedded: None,
        }
    }

    /// Whether some bytes still match this reference.
    ///
    /// The only way this crate answers the integrity question: by rehashing
    /// under the construction the reference names, never under the current
    /// favourite.
    pub fn matches(&self, content: &[u8]) -> bool {
        match &self.construction {
            Construction::FileBytesSha256 => {
                statecraft_environment::digest::digest_bytes(content) == self.digest
                    && content.len() as u64 == self.bytes
            }
            // A canonical construction needs the canonicalizer that produced it.
            // Answering with a file-byte hash here is exactly the substitution
            // section 3.7 forbids, so this reports that it cannot answer.
            Construction::CanonicalRecordSha256 { .. } => false,
        }
    }

    /// Whether this reference can be checked by this build.
    pub fn checkable_here(&self) -> bool {
        matches!(self.construction, Construction::FileBytesSha256)
    }
}

/// Judge integrity of preserved bytes against their reference.
pub fn integrity(reference: &Reference, content: Option<&[u8]>) -> crate::dimensions::Integrity {
    use crate::dimensions::Integrity;
    match content {
        None => Integrity::Unknown,
        Some(_) if !reference.checkable_here() => Integrity::Unknown,
        Some(bytes) => {
            if reference.matches(bytes) {
                Integrity::Pass
            } else {
                // Never repaired, never silently re-canonicalized.
                Integrity::Fail
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dimensions::Integrity;

    #[test]
    fn intact_bytes_pass_integrity() {
        let r = Reference::over_file_bytes("receipt", "1", b"the evidence");
        assert_eq!(integrity(&r, Some(b"the evidence")), Integrity::Pass);
    }

    #[test]
    fn a_byte_level_mutation_fails_and_is_never_repaired() {
        let r = Reference::over_file_bytes("receipt", "1", b"the evidence");
        assert_eq!(integrity(&r, Some(b"the evidencf")), Integrity::Fail);
        // The reference is unchanged: nothing re-canonicalized it to match.
        assert_eq!(
            r.digest,
            Reference::over_file_bytes("receipt", "1", b"the evidence").digest
        );
    }

    #[test]
    fn absent_bytes_are_unknown_not_a_failure() {
        let r = Reference::over_file_bytes("receipt", "1", b"x");
        assert_eq!(integrity(&r, None), Integrity::Unknown);
    }

    #[test]
    fn a_canonical_construction_is_not_answered_with_a_file_byte_hash() {
        let r = Reference {
            evidence_type: "record".into(),
            schema_version: "1".into(),
            digest: statecraft_environment::digest::digest_bytes(b"x"),
            bytes: 1,
            construction: Construction::CanonicalRecordSha256 {
                canonicalization_version: "1".into(),
            },
            embedded: None,
        };
        assert!(!r.checkable_here());
        assert_eq!(
            integrity(&r, Some(b"x")),
            Integrity::Unknown,
            "a canonical record hash never substitutes for a file-byte digest"
        );
    }

    #[test]
    fn the_construction_is_part_of_the_references_identity() {
        let a = Reference::over_file_bytes("r", "1", b"x");
        let mut b = a.clone();
        b.construction = Construction::CanonicalRecordSha256 {
            canonicalization_version: "1".into(),
        };
        assert_ne!(a, b);
        assert_ne!(a.construction.name(), b.construction.name());
    }

    #[test]
    fn a_versioned_construction_names_its_version() {
        let c = Construction::CanonicalRecordSha256 {
            canonicalization_version: "2".into(),
        };
        assert_eq!(c.name(), "canonical-record-sha256/2");
    }
}
