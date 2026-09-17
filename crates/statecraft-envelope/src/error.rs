//! One error type for the crate.

/// What can go wrong in a pure function of this crate.
#[derive(Debug, thiserror::Error, PartialEq, Eq, Clone)]
pub enum Error {
    /// A value violates a rule of the format it is being encoded or decoded under.
    #[error("validation: {0}")]
    Validation(String),
    /// Bytes do not decode as canonical CBOR.
    #[error("decode: {0}")]
    Decode(String),
    /// A referenced parent, key or object is absent.
    #[error("not found: {0}")]
    NotFound(String),
    /// A signature or hash did not verify.
    #[error("crypto: {0}")]
    Crypto(String),
}
