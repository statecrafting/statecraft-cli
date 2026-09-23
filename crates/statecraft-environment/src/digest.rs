//! Content digests.
//!
//! Every manifest entry carries the SHA-256 of the bytes written or observed
//! (spec 002 section 3.3), and every state `doctor` reports is a comparison
//! against one. The digest is over file CONTENT only: not the path, not the
//! mode, not the mtime. A rename is therefore not drift, which is deliberate.

use sha2::{Digest, Sha256};
use std::path::Path;

/// The SHA-256 of some bytes, lowercase hex.
pub fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut acc, b| {
            use std::fmt::Write as _;
            let _ = write!(acc, "{b:02x}");
            acc
        })
}

/// The SHA-256 of everything a reader yields, lowercase hex, and its length in
/// bytes, read in chunks rather than held whole.
pub fn digest_reader(mut reader: impl std::io::Read) -> std::io::Result<(String, u64)> {
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        hasher.update(&buf[..n]);
        total += n as u64;
    }
    let hex = hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut acc, b| {
            use std::fmt::Write as _;
            let _ = write!(acc, "{b:02x}");
            acc
        });
    Ok((hex, total))
}

/// The SHA-256 of a file's contents, and its length in bytes.
///
/// `Ok(None)` when the file is absent, which is a state this crate reports
/// (`missing`) rather than an error.
pub fn digest_file(path: &Path) -> std::io::Result<Option<(String, u64)>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some((digest_bytes(&bytes), bytes.len() as u64))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_streamed_digest_equals_the_whole_read_one() {
        let bytes: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let (hex, len) = digest_reader(&bytes[..]).unwrap();
        assert_eq!(hex, digest_bytes(&bytes));
        assert_eq!(len, bytes.len() as u64);
    }

    #[test]
    fn empty_input_is_the_known_sha256_of_nothing() {
        assert_eq!(
            digest_bytes(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn a_known_vector_matches() {
        assert_eq!(
            digest_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn an_absent_file_is_none_rather_than_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(digest_file(&dir.path().join("nope")).unwrap().is_none());
    }

    #[test]
    fn a_present_file_reports_digest_and_length() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f");
        std::fs::write(&p, b"abc").unwrap();
        let (d, n) = digest_file(&p).unwrap().unwrap();
        assert_eq!(n, 3);
        assert_eq!(d, digest_bytes(b"abc"));
    }
}
