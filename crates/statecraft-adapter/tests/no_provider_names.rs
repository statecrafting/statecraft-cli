//! Spec 004 section 3.8: a provider name appearing in `crates/statecraft-adapter/`
//! is a defect.
//!
//! Mechanical rather than remembered. The seam's purpose is that it cannot need
//! a provider's name, so the check is that none appears, not that none is used.
//!
//! Spec 047 section 3.2 extends the check to the callers: outside an adapter's
//! own crate, no decision compares an adapter's name. A caller reads a field
//! of the adapter's capability declaration instead.

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

/// Whether a line compares something against an adapter's name: a comparison
/// operator, and either a string literal opening with a provider name or an
/// adapter-name constant.
fn compares_an_adapter_name(line: &str) -> bool {
    let compares = line.contains("==") || line.contains("!=") || line.contains("matches!");
    if !compares {
        return false;
    }
    let lower = line.to_lowercase();
    lower.contains("adapter_name")
        || PROVIDER_NAMES
            .iter()
            .any(|name| lower.contains(&format!("\"{name}")))
}

/// Scan one caller crate's `src/`, up to each file's unit-test module: a
/// test may name the adapter it exercises, and a decision may not.
fn scan_callers(dir: &Path, findings: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("readable") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            scan_callers(&path, findings);
            continue;
        }
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (number, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("#[cfg(test)]") {
                break;
            }
            if compares_an_adapter_name(line) {
                findings.push(format!(
                    "{}:{}: {}",
                    path.display(),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }
}

// Spec 047 I-1 and V-1. Every crate in the workspace but the adapters'
// own: the seam is held to the stricter rule above, and an adapter's own
// crate is where its name is allowed.
#[test]
fn no_caller_outside_an_adapter_crate_compares_an_adapter_name() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crates directory");
    let mut findings = Vec::new();
    let mut scanned = 0;
    for entry in std::fs::read_dir(crates).expect("readable") {
        let path = entry.expect("entry").path();
        let own = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("statecraft-adapter"));
        let src = path.join("src");
        if own || !src.is_dir() {
            continue;
        }
        scanned += 1;
        scan_callers(&src, &mut findings);
    }
    assert!(scanned > 0, "no caller crate was scanned");
    assert!(
        findings.is_empty(),
        "a decision outside an adapter crate compares an adapter name; read a field \
         of its capability declaration instead (spec 047 section 3.2):\n{}",
        findings.join("\n")
    );
}

#[test]
fn the_caller_check_recognises_a_comparison() {
    assert!(compares_an_adapter_name(r#"if adapter == "codex" {"#));
    assert!(compares_an_adapter_name("if m.adapter != ADAPTER_NAME {"));
    assert!(!compares_an_adapter_name(r#"let name = "codex";"#));
    assert!(!compares_an_adapter_name("if a.harness == b.harness {"));
}

/// Every `.rs` and `.toml` file under `dir` that contains one of `names`.
fn naming(dir: &Path, names: &[&str], findings: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("readable") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            naming(&path, names, findings);
            continue;
        }
        if path.extension().is_none_or(|e| e != "rs" && e != "toml") {
            continue;
        }
        let lower = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .to_lowercase();
        for name in names.iter().filter(|n| lower.contains(*n)) {
            findings.push(format!("{} contains `{name}`", path.display()));
        }
    }
}

// Spec 055 V-1: the Codex adapter's crate is the only adapter crate that
// names Codex, and it names no other provider. The caller check above
// already covers every crate that is not an adapter's.
#[test]
fn each_adapter_crate_names_only_its_own_provider() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crates directory");
    let mut findings = Vec::new();
    naming(
        &crates.join("statecraft-adapter-codex"),
        &["claude", "anthropic", "gemini", "copilot", "cursor"],
        &mut findings,
    );
    naming(
        &crates.join("statecraft-adapter-claude-code"),
        &["codex"],
        &mut findings,
    );
    assert!(
        findings.is_empty(),
        "an adapter crate names another adapter's provider (spec 055 section 2):\n{}",
        findings.join("\n")
    );
}
