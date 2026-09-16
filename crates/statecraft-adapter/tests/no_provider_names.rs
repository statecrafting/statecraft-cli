//! Spec 004 section 3.8: a provider name appearing in `crates/statecraft-adapter/`
//! is a defect.
//!
//! Mechanical rather than remembered. The seam's purpose is that it cannot need
//! a provider's name, so the check is that none appears, not that none is used.

use std::path::Path;

/// Names that must not appear in this crate's territory.
///
/// A list of the ones that exist today, which is the honest scope: it cannot
/// catch a provider nobody has heard of, and it does catch the ones a
/// well-meaning change would reach for.
const PROVIDER_NAMES: [&str; 8] = [
    "claude",
    "anthropic",
    "openai",
    "codex",
    "gemini",
    "copilot",
    "cursor",
    "gpt",
];

fn scan(dir: &Path, findings: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("readable") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            scan(&path, findings);
            continue;
        }
        if path.extension().is_none_or(|e| e != "rs" && e != "toml") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        // This test file names the providers it forbids, which would otherwise
        // be its own first finding.
        if path
            .file_name()
            .is_some_and(|n| n == "no_provider_names.rs")
        {
            continue;
        }
        let lower = text.to_lowercase();
        for name in PROVIDER_NAMES {
            if lower.contains(name) {
                findings.push(format!("{} contains `{name}`", path.display()));
            }
        }
    }
}

#[test]
fn no_provider_name_appears_anywhere_in_this_crate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut findings = Vec::new();
    scan(crate_root, &mut findings);
    assert!(
        findings.is_empty(),
        "a provider name in the seam is a defect (spec 004 section 3.8):\n{}",
        findings.join("\n")
    );
}
