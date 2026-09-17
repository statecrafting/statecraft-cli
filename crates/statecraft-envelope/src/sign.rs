//! Domain-separated Ed25519 signing (hqgit 010 B-7's rule, fixed here).
//!
//! The preimage of a signature is `PREFIX || domain || "\n" || bytes` where
//! `PREFIX` is `statecraft-envelope/v1\n`. The domain is part of what is
//! signed, so a signature over an entry can never be presented as a signature
//! over an attestation with the same bytes. This is the one construction of
//! the crate that must never change inside a MAJOR (constitution VIII).

use ed25519_dalek::{
    Signature as DalekSignature, Signer as _, SigningKey, Verifier as _, VerifyingKey,
};

use crate::Error;
use crate::hash::KeyId;

/// The fixed preimage prefix.
pub const PREIMAGE_PREFIX: &[u8] = b"statecraft-envelope/v1\n";

/// A signing domain. Closed to the constants below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignDomain(pub &'static str);

/// The entry domain (hqgit 017 B-3).
pub const DOMAIN_ENTRY: SignDomain = SignDomain("ledger.entry");
/// The attestation domain (hqgit 027 B-2).
pub const DOMAIN_ATTESTATION: SignDomain = SignDomain("attestation");

/// Build the preimage.
pub fn preimage(domain: SignDomain, bytes: &[u8]) -> Vec<u8> {
    let mut p = Vec::with_capacity(PREIMAGE_PREFIX.len() + domain.0.len() + 1 + bytes.len());
    p.extend_from_slice(PREIMAGE_PREFIX);
    p.extend_from_slice(domain.0.as_bytes());
    p.push(b'\n');
    p.extend_from_slice(bytes);
    p
}

/// A 64-byte Ed25519 signature.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Signature(pub [u8; 64]);

impl std::fmt::Debug for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Signature({}..)", hex::encode(&self.0[..8]))
    }
}

impl Signature {
    /// From exactly 64 bytes.
    pub fn from_bytes(b: &[u8]) -> Result<Self, Error> {
        if b.len() != 64 {
            return Err(Error::Validation(format!(
                "signature is {} bytes, not 64",
                b.len()
            )));
        }
        let mut a = [0u8; 64];
        a.copy_from_slice(b);
        Ok(Signature(a))
    }
}

/// A 32-byte Ed25519 public key.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PublicKey(pub [u8; 32]);

impl std::fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PublicKey({}..)", hex::encode(&self.0[..8]))
    }
}

impl PublicKey {
    /// Its identifier.
    pub fn id(&self) -> KeyId {
        KeyId::of(&self.0)
    }

    /// From lowercase hex.
    pub fn from_hex(s: &str) -> Result<Self, Error> {
        let v = hex::decode(s).map_err(|e| Error::Validation(format!("public key hex: {e}")))?;
        if v.len() != 32 {
            return Err(Error::Validation("public key is not 32 bytes".into()));
        }
        let mut a = [0u8; 32];
        a.copy_from_slice(&v);
        Ok(PublicKey(a))
    }

    /// Verify a domain-separated signature over bytes.
    pub fn verify(&self, domain: SignDomain, bytes: &[u8], sig: &Signature) -> Result<(), Error> {
        let vk = VerifyingKey::from_bytes(&self.0)
            .map_err(|e| Error::Crypto(format!("public key: {e}")))?;
        let s = DalekSignature::from_bytes(&sig.0);
        vk.verify(&preimage(domain, bytes), &s)
            .map_err(|_| Error::Crypto("signature does not verify".into()))
    }
}

/// A signer holding an Ed25519 seed. Never serialized, never `Debug`-printed
/// beyond its key id.
pub struct Signer {
    key: SigningKey,
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Signer({:?})", self.public().id())
    }
}

impl Signer {
    /// From a 32-byte seed. Deterministic: the same seed is the same key,
    /// which is what golden vectors need.
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        Signer {
            key: SigningKey::from_bytes(seed),
        }
    }

    /// The public key.
    pub fn public(&self) -> PublicKey {
        PublicKey(self.key.verifying_key().to_bytes())
    }

    /// Sign bytes under a domain.
    pub fn sign(&self, domain: SignDomain, bytes: &[u8]) -> Signature {
        Signature(self.key.sign(&preimage(domain, bytes)).to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signature_is_bound_to_its_domain() {
        let s = Signer::from_seed(&[7u8; 32]);
        let sig = s.sign(DOMAIN_ENTRY, b"x");
        assert!(s.public().verify(DOMAIN_ENTRY, b"x", &sig).is_ok());
        assert!(s.public().verify(DOMAIN_ATTESTATION, b"x", &sig).is_err());
    }
}
