//! The adoption ledger, spec 013 section 3.1.
//!
//! An adopted producer identity is seven facts: the exact release and its
//! artifact digest, the interface identity, the binding and terminal schema
//! versions, the interface digest, the per-platform qualification record, the
//! Statecraft release range that adopts it, and the rollback identity. They are
//! written once, here, as constants, because an upgrade is a reviewed
//! dependency change and never a runtime negotiation.
//!
//! [`verify`] is the only way to obtain a [`VerifiedProducer`]. It reads the
//! one path it is given and refuses an absent, unsupported, oversized or
//! digest-mismatched executable. It never searches for another one.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

/// The largest executable [`verify`] will hash. The adopted 0.1.0 builds are a
/// few megabytes; anything near this bound is not the adopted artifact.
pub const MAX_EXECUTABLE_BYTES: u64 = 256 * 1024 * 1024;

/// One supported platform of an adopted release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platform {
    /// The Rust target triple the artifact was built for.
    pub target: &'static str,
    /// `sha256:` digest of the qualified executable.
    pub executable_digest: &'static str,
    /// `sha256:` digest of the release archive that carries it.
    pub archive_digest: &'static str,
    /// The producer-corpus path of the platform's qualification record.
    pub qualification: &'static str,
}

/// What restores service if this adoption is withdrawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rollback {
    /// No earlier release claims this interface major. Rolling back removes
    /// the adoption and every requested attempt records `witness-unavailable`.
    None,
    /// The prior exact release, which stays supported until the upgrade is
    /// accepted.
    Release(&'static str),
}

/// One adopted producer, section 3.1's seven facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Adoption {
    /// The producer's name.
    pub name: &'static str,
    /// Fact 1: the exact immutable release version.
    pub version: &'static str,
    /// Fact 1: the commit every adopted executable was built from.
    pub source_commit: &'static str,
    /// Fact 2: the interface identity.
    pub interface: &'static str,
    /// Fact 3: the `AttemptBinding` schema version.
    pub binding_schema: u64,
    /// Fact 3: the terminal-message schema version.
    pub terminal_schema: u64,
    /// Fact 4: the interface sections, anchor and digest, verified against
    /// the producer corpus and equal to spec 013's `interface_references`.
    pub interface_sections: &'static [(&'static str, &'static str)],
    /// Facts 1 and 5: each supported platform's artifact and qualification.
    pub platforms: &'static [Platform],
    /// Fact 6: the Statecraft workspace versions that adopt it.
    pub statecraft_range: &'static str,
    /// Fact 7: the rollback identity.
    pub rollback: Rollback,
}

/// The one adopted producer: wire-witness 0.1.0, released 2026-10-15.
///
/// Every value is copied from the producer's release record,
/// `qualification/0.1.0/release-notes.md` at wire-witness commit
/// `3a65a1f`, and spec 013's interface pins.
pub const ADOPTED: Adoption = Adoption {
    name: "wire-witness",
    version: "0.1.0",
    source_commit: "4a7c603b8c26eed66955b5b814af3cd576a5a3ef",
    interface: "wire-witness.sidecar/1",
    binding_schema: 1,
    terminal_schema: 1,
    interface_sections: &[
        (
            "3-1-attemptbinding",
            "sha256:68aafeb66bf494eda066bff13eb506fce5f9a8b3777505a74d98ec09fa8e11ca",
        ),
        (
            "3-2-versioned-stdio",
            "sha256:d64434f79125e34a81f99f23670275a9d7946a02df05d34b8deee56e10fca8ec",
        ),
        (
            "3-3-bracketed-lifetime",
            "sha256:b40040b08f584f275ea4f52b4281378c23fd365c783e888fb957acf81cd9c5f1",
        ),
    ],
    platforms: &[
        Platform {
            target: "aarch64-apple-darwin",
            executable_digest: "sha256:649a2c099c6df14d9fe2a2906241032f382661389f3a30d3ffa71daed1eeb58c",
            archive_digest: "sha256:64ccc41968420b581d58f9f638027d9a79886760aaa83b8a1f0ec02dab6dfd0e",
            qualification: "qualification/0.1.0/aarch64-apple-darwin.md",
        },
        Platform {
            target: "aarch64-unknown-linux-gnu",
            executable_digest: "sha256:468a8bac6b6536b64aa86f78251ff35e10842d3f98ea29bd085ca9b7b70d2c08",
            archive_digest: "sha256:a6b493089e2ab5d74ac641dd2ce8fd6b03ad19d5fd7ccade2c84ad50787100f7",
            qualification: "qualification/0.1.0/aarch64-unknown-linux-gnu.md",
        },
        Platform {
            target: "x86_64-unknown-linux-gnu",
            executable_digest: "sha256:27a629a69afd0980acce778a7bbaaee7d10ab541e5adeaeb790b5d2e4126cc70",
            archive_digest: "sha256:f0c225ae7109c4cce84742567842bdec11d9c20b6fba1de399a9424581820f4a",
            qualification: "qualification/0.1.0/x86_64-unknown-linux-gnu.md",
        },
    ],
    statecraft_range: "=0.0.0",
    rollback: Rollback::None,
};

/// The target triple this binary was built for, where the ledger can name one.
#[must_use]
pub fn current_target() -> Option<&'static str> {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("aarch64", "macos") => Some("aarch64-apple-darwin"),
        ("x86_64", "macos") => Some("x86_64-apple-darwin"),
        ("aarch64", "linux") => Some("aarch64-unknown-linux-gnu"),
        ("x86_64", "linux") => Some("x86_64-unknown-linux-gnu"),
        _ => None,
    }
}

impl Adoption {
    /// The adopted artifact for `target`, if this release qualified it.
    #[must_use]
    pub fn platform(&self, target: &str) -> Option<&'static Platform> {
        self.platforms.iter().find(|p| p.target == target)
    }
}

/// Why a producer was refused. Each maps to `witness-unavailable` before
/// spawn (section 3.7); none selects another executable.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProducerRefusal {
    /// This platform has no qualified artifact in the adopted release.
    #[error("wire-witness {version} is not qualified for {target}")]
    UnsupportedPlatform {
        /// The adopted version.
        version: &'static str,
        /// The platform asked about.
        target: String,
    },
    /// The path is not an absolute path to a regular file.
    #[error("wire-witness executable unavailable at {path}: {why}")]
    Unavailable {
        /// The one path that was read.
        path: PathBuf,
        /// What was wrong with it.
        why: String,
    },
    /// The file is not the adopted artifact.
    #[error("wire-witness executable at {path} is {actual}, not the adopted {expected}")]
    DigestMismatch {
        /// The one path that was read.
        path: PathBuf,
        /// The ledger's digest for this platform.
        expected: &'static str,
        /// What the file hashed to.
        actual: String,
    },
}

/// An executable proved to be the adopted artifact for this platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedProducer {
    /// The producer's name.
    pub name: String,
    /// The exact release.
    pub version: String,
    /// The interface identity.
    pub interface: String,
    /// The platform the artifact was qualified on.
    pub target: String,
    /// The executable's digest, equal to the ledger's.
    pub executable_digest: String,
    /// The qualification record that admits it.
    pub qualification: String,
    /// The path that was verified and is the only one spawned.
    pub path: PathBuf,
}

/// Verify `executable` against `adoption` for `target`.
///
/// # Errors
///
/// A [`ProducerRefusal`] naming what was wrong. There is no fallback.
pub fn verify(
    adoption: &Adoption,
    target: &str,
    executable: &Path,
) -> Result<VerifiedProducer, ProducerRefusal> {
    let platform =
        adoption
            .platform(target)
            .ok_or_else(|| ProducerRefusal::UnsupportedPlatform {
                version: adoption.version,
                target: target.to_string(),
            })?;
    let unavailable = |why: &str| ProducerRefusal::Unavailable {
        path: executable.to_path_buf(),
        why: why.to_string(),
    };
    if !executable.is_absolute() {
        return Err(unavailable("not an absolute path"));
    }
    let metadata =
        std::fs::symlink_metadata(executable).map_err(|e| unavailable(&e.to_string()))?;
    if !metadata.file_type().is_file() {
        return Err(unavailable("not a regular file"));
    }
    if metadata.len() > MAX_EXECUTABLE_BYTES {
        return Err(unavailable("larger than any adopted artifact"));
    }
    let actual = digest_file(executable).map_err(|e| unavailable(&e.to_string()))?;
    if actual != platform.executable_digest {
        return Err(ProducerRefusal::DigestMismatch {
            path: executable.to_path_buf(),
            expected: platform.executable_digest,
            actual,
        });
    }
    Ok(VerifiedProducer {
        name: adoption.name.to_string(),
        version: adoption.version.to_string(),
        interface: adoption.interface.to_string(),
        target: platform.target.to_string(),
        executable_digest: actual,
        qualification: platform.qualification.to_string(),
        path: executable.to_path_buf(),
    })
}

/// `sha256:` digest of a file, read with a bound.
fn digest_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?.take(MAX_EXECUTABLE_BYTES + 1);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("sha256:{}", hex(&hasher.finalize())))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adoption_for(digest: &'static str) -> Adoption {
        Adoption {
            platforms: Box::leak(Box::new([Platform {
                target: "test-target",
                executable_digest: digest,
                archive_digest: "sha256:unused",
                qualification: "qualification/test.md",
            }])),
            ..ADOPTED
        }
    }

    // A well-formed digest no test file hashes to.
    const OTHER_DIGEST: &str =
        "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    fn digest_of(bytes: &[u8]) -> String {
        format!("sha256:{}", hex(&Sha256::digest(bytes)))
    }

    #[test]
    fn the_adopted_artifact_verifies_and_names_its_qualification() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wire-witness");
        std::fs::write(&path, b"adopted").unwrap();
        let digest: &'static str = Box::leak(digest_of(b"adopted").into_boxed_str());
        let verified = verify(&adoption_for(digest), "test-target", &path).unwrap();
        assert_eq!(verified.executable_digest, digest);
        assert_eq!(verified.qualification, "qualification/test.md");
        assert_eq!(verified.path, path);
    }

    #[test]
    fn a_different_executable_is_refused_and_nothing_else_is_tried() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wire-witness");
        std::fs::write(&path, b"something else").unwrap();
        let refusal = verify(&adoption_for(OTHER_DIGEST), "test-target", &path)
            .expect_err("a mismatched digest refuses");
        assert!(matches!(refusal, ProducerRefusal::DigestMismatch { .. }));
    }

    #[test]
    fn an_unqualified_platform_absent_path_relative_path_and_symlink_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wire-witness");
        std::fs::write(&path, b"adopted").unwrap();
        let adoption = adoption_for(OTHER_DIGEST);
        assert!(matches!(
            verify(&adoption, "riscv64gc-unknown-linux-gnu", &path),
            Err(ProducerRefusal::UnsupportedPlatform { .. })
        ));
        assert!(matches!(
            verify(&adoption, "test-target", &dir.path().join("absent")),
            Err(ProducerRefusal::Unavailable { .. })
        ));
        assert!(matches!(
            verify(&adoption, "test-target", Path::new("wire-witness")),
            Err(ProducerRefusal::Unavailable { .. })
        ));
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(matches!(
            verify(&adoption, "test-target", &link),
            Err(ProducerRefusal::Unavailable { .. })
        ));
    }

    #[test]
    fn the_ledger_names_three_qualified_platforms_and_no_rollback() {
        let targets: Vec<_> = ADOPTED.platforms.iter().map(|p| p.target).collect();
        assert_eq!(
            targets,
            [
                "aarch64-apple-darwin",
                "aarch64-unknown-linux-gnu",
                "x86_64-unknown-linux-gnu"
            ]
        );
        assert_eq!(ADOPTED.rollback, Rollback::None);
        assert!(ADOPTED.platform("x86_64-apple-darwin").is_none());
        for platform in ADOPTED.platforms {
            assert!(platform.executable_digest.starts_with("sha256:"));
            assert_eq!(platform.executable_digest.len(), 71);
        }
    }

    #[test]
    fn the_ledger_interface_sections_are_the_ones_spec_013_pins() {
        let spec = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../specs/013-supervised-wire-witness/spec.md"
        ))
        .unwrap();
        for (anchor, digest) in ADOPTED.interface_sections {
            let pinned = format!("- anchor: \"{anchor}\"\n        digest: \"{digest}\"");
            assert!(
                spec.contains(&pinned),
                "spec 013 does not pin {anchor} at {digest}"
            );
        }
    }
}
