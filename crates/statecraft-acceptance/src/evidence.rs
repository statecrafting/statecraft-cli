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
//!
//! # Where these types live now
//!
//! [`Reference`], [`Construction`] and [`Embedded`] are defined in
//! `statecraft-envelope` and re-exported here (spec 007). A reference is the
//! most widely exchanged value in the pair of products, and it now has one
//! definition. The field names, their order and the construction spellings are
//! the ones this crate established; the envelope adds three optional fields
//! and one construction, each omitted when absent, so a reference written here
//! is byte for byte what it was.
//!
//! What stays here is this product's judgment: [`integrity`] answers the
//! dimension from a reference and the bytes in hand, under the construction the
//! reference names and no other.

use crate::dimensions::Integrity;

pub use statecraft_envelope::reference::{Construction, Embedded, Reference};

/// Judge integrity of preserved bytes against their reference.
///
/// `unknown` covers two different situations and says so in one word on
/// purpose: there were no bytes to check, or this build cannot evaluate the
/// construction the reference names. Neither is a failure, and neither is a
/// pass.
pub fn integrity(reference: &Reference, content: Option<&[u8]>) -> Integrity {
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
            construction: Construction::CanonicalRecordSha256 {
                canonicalization_version: "1".into(),
            },
            ..Reference::over_file_bytes("record", "1", b"x")
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

    #[test]
    fn a_construction_this_build_does_not_know_is_unknown_and_never_guessed_at() {
        // The envelope preserves an unrecognized construction rather than
        // failing the read; section 3.7's rule is then this product's answer
        // to it, which is the same answer it gives a canonical record.
        let text = r#"{"evidence_type":"x","schema_version":"1","digest":"00","bytes":1,"construction":{"canonical-record-blake3":{"canonicalization_version":"9"}}}"#;
        let r: Reference = serde_json::from_str(text).unwrap();
        assert!(matches!(r.construction, Construction::Unknown(_)));
        assert_eq!(integrity(&r, Some(b"x")), Integrity::Unknown);
    }

    #[test]
    fn the_digest_is_the_sha256_this_product_has_always_written() {
        let r = Reference::over_file_bytes("receipt", "1", b"test");
        assert_eq!(
            r.digest,
            statecraft_environment::digest::digest_bytes(b"test")
        );
        assert_eq!(r.bytes, 4);
    }
}
