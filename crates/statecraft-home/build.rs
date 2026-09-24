//! Derives the linked producer's version from the build (spec 001 section
//! 3.13, `D-06` as amended on 2026-09-24).
//!
//! The root `Cargo.toml`'s `[workspace.dependencies]` is the one place the
//! version is stated. `Cargo.lock` records what that statement resolved to,
//! which is what this build actually links, so the version a report names is
//! read from there and never spelled a second time in source.

use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let lock = manifest_dir.join("../../Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock.display());
    let text = std::fs::read_to_string(&lock)
        .unwrap_or_else(|e| panic!("reading {}: {e}", lock.display()));
    let version = locked_version(&text, "spec-spine-core")
        .expect("Cargo.lock records exactly one spec-spine-core package");
    println!("cargo:rustc-env=STATECRAFT_PRODUCER_VERSION={version}");
}

/// The version of the single `[[package]]` named `name`, or `None` when there
/// is none or more than one: two linked versions are not one producer.
fn locked_version(lock: &str, name: &str) -> Option<String> {
    let wanted = format!("name = \"{name}\"");
    let mut found = Vec::new();
    for block in lock.split("[[package]]") {
        let mut lines = block.lines().map(str::trim);
        if lines.clone().any(|l| l == wanted) {
            let version = lines
                .find_map(|l| l.strip_prefix("version = \""))
                .and_then(|v| v.strip_suffix('"'))?;
            found.push(version.to_string());
        }
    }
    if found.len() == 1 { found.pop() } else { None }
}
