//! The linked governance producer's identity, fixed at build time.
//!
//! Spec 002 section 5, 2026-09-24 (provenance item 3), and `D-06` as amended
//! the same day: a report that names the producer version derives it from the
//! build, never from a second literal. `Cargo.lock` is what the build links,
//! so it is read here, and the version and the crates.io checksum it records
//! for `spec-spine-core` become compile-time constants.

use std::path::PathBuf;

const PRODUCER: &str = "spec-spine-core";

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let lock = manifest_dir.join("../../Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock.display());
    let text = std::fs::read_to_string(&lock).unwrap_or_else(|e| {
        panic!(
            "{}: {e}; the producer identity needs the lock file",
            lock.display()
        )
    });
    let (version, checksum) = locked(&text, PRODUCER)
        .unwrap_or_else(|| panic!("{PRODUCER} with a checksum is not in {}", lock.display()));
    println!("cargo:rustc-env=STATECRAFT_PRODUCER_VERSION={version}");
    println!("cargo:rustc-env=STATECRAFT_PRODUCER_CHECKSUM={checksum}");
}

/// The version and checksum `Cargo.lock` records for one registry package.
fn locked(text: &str, name: &str) -> Option<(String, String)> {
    for block in text.split("[[package]]") {
        let field = |key: &str| {
            block.lines().find_map(|l| {
                l.strip_prefix(key)
                    .and_then(|r| r.trim().strip_prefix('='))
                    .map(|v| v.trim().trim_matches('"').to_string())
            })
        };
        if field("name ").as_deref() == Some(name) {
            return Some((field("version ")?, field("checksum ")?));
        }
    }
    None
}
