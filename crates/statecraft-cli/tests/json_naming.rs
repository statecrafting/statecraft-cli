//! Spec 006 section 5, 2026-09-25: the JSON naming convention, enforced from
//! the source side.
//!
//! The walker in `support/json_naming.rs` checks what the binary actually
//! prints; this file checks what the workspace declares, so a new type with a
//! snake_case field fails here before any test prints it. Both read the one
//! exemption list, `GRANDFATHERED`, in that module.
//!
//! The scan is textual, over `crates/*/src/**/*.rs`, and deliberately narrow:
//! it looks at items that `#[derive(...Serialize...)]` and computes the name
//! serde would write for each field and variant, from `rename_all`,
//! `rename_all_fields` and a per-member `rename`. A field is checked against
//! the key rule (`^[a-z][a-zA-Z0-9]*$`); a variant's name is checked against
//! the value rule (kebab-case, one word included). A hand-written `Serialize`
//! and a `json!` literal are invisible to it, which is why the walker exists.

#[path = "support/json_naming.rs"]
mod json_naming;

use json_naming::{GRANDFATHERED, is_key, is_value};
use std::path::{Path, PathBuf};

/// One declared name that does not serialize to the convention.
#[derive(Debug)]
struct Finding {
    /// `crate/src/file.rs::Type`, the form the exemption list uses.
    item: String,
    /// The member and the name serde would write for it.
    detail: String,
}

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            sources(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
}

/// The line with string literals and a trailing comment removed, so braces
/// inside a message or a doc line are not counted.
fn code_of(line: &str) -> String {
    let t = line.trim_start();
    if t.starts_with("//") {
        return String::new();
    }
    let mut out = String::new();
    let mut in_str = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_str {
            if c == '\\' {
                chars.next();
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        if c == '"' {
            in_str = true;
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            break;
        }
        out.push(c);
    }
    out
}

/// The value of `key = "..."` inside an attribute text, if present.
fn attr_value(attrs: &str, key: &str) -> Option<String> {
    let mut rest = attrs;
    while let Some(i) = rest.find(key) {
        let before = rest[..i].chars().last();
        let after = &rest[i + key.len()..];
        let boundary = before.is_none_or(|c| !c.is_alphanumeric() && c != '_');
        let trimmed = after.trim_start();
        if boundary && trimmed.starts_with('=') {
            let v = trimmed[1..].trim_start();
            if let Some(v) = v.strip_prefix('"') {
                return v.split('"').next().map(str::to_string);
            }
        }
        rest = after;
    }
    None
}

fn has_word(attrs: &str, word: &str) -> bool {
    attrs
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .any(|w| w == word)
}

/// Split an identifier into lowercase words, from snake_case or PascalCase.
fn words(ident: &str) -> Vec<String> {
    let ident = ident.trim_start_matches("r#");
    let mut out = Vec::new();
    for part in ident.split('_').filter(|p| !p.is_empty()) {
        let mut cur = String::new();
        for c in part.chars() {
            if c.is_uppercase() && !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            cur.extend(c.to_lowercase());
        }
        if !cur.is_empty() {
            out.push(cur);
        }
    }
    out
}

/// The name serde writes for `ident` under a `rename_all` rule. Without a rule
/// a field keeps its snake_case spelling and a variant its PascalCase one.
fn renamed(ident: &str, rule: Option<&str>) -> String {
    let w = words(ident);
    match rule {
        Some("camelCase") => w
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == 0 {
                    s.clone()
                } else {
                    let mut c = s.chars();
                    c.next()
                        .map(|f| f.to_uppercase().chain(c).collect())
                        .unwrap_or_default()
                }
            })
            .collect(),
        Some("kebab-case") => w.join("-"),
        Some("snake_case") => w.join("_"),
        Some("lowercase") => w.concat(),
        _ => ident.trim_start_matches("r#").to_string(),
    }
}

/// Every declared name in one file that does not serialize to the convention.
fn scan(file: &Path, label: &str, findings: &mut Vec<Finding>) {
    let text = std::fs::read_to_string(file).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    // Types in a test module are not the product's output.
    let end = lines
        .iter()
        .position(|l| l.trim() == "#[cfg(test)]")
        .unwrap_or(lines.len());
    for i in 0..end {
        let Some((kind, name)) = item_of(lines[i]) else {
            continue;
        };
        let mut j = i;
        while j > 0 {
            let t = lines[j - 1].trim();
            if t.is_empty() || t.ends_with('}') || t.ends_with(';') || t.ends_with('{') {
                break;
            }
            j -= 1;
        }
        let attrs = lines[j..i]
            .iter()
            .filter(|l| !l.trim_start().starts_with("//"))
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        if !(attrs.contains("derive(") && has_word(&attrs, "Serialize")) {
            continue;
        }
        let rule = attr_value(&attrs, "rename_all");
        let field_rule = attr_value(&attrs, "rename_all_fields");
        let untagged = has_word(&attrs, "untagged");
        let item = format!("{label}::{name}");
        for m in members(&lines[i..end], kind) {
            let serialized = match (&m.rename, &m.variant_of) {
                (Some(r), _) => r.clone(),
                (None, None) => renamed(&m.ident, rule.as_deref()),
                (None, Some(_)) => renamed(&m.ident, field_rule.as_deref()),
            };
            let is_variant = kind == "enum" && m.variant_of.is_none();
            let ok = if is_variant {
                untagged || is_value(&serialized)
            } else {
                is_key(&serialized)
            };
            if !ok {
                let at = match &m.variant_of {
                    Some(v) => format!("{v}.{}", m.ident),
                    None => m.ident.clone(),
                };
                findings.push(Finding {
                    item: item.clone(),
                    detail: format!("{at} -> {serialized:?}"),
                });
            }
        }
    }
}

fn item_of(line: &str) -> Option<(&'static str, String)> {
    let mut t = line.trim_start();
    if let Some(rest) = t.strip_prefix("pub") {
        t = rest.trim_start();
        if t.starts_with('(') {
            t = t[t.find(')')? + 1..].trim_start();
        }
    }
    let (kind, rest) = if let Some(r) = t.strip_prefix("struct ") {
        ("struct", r)
    } else if let Some(r) = t.strip_prefix("enum ") {
        ("enum", r)
    } else {
        return None;
    };
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some((kind, name))
}

/// One named member: a field, a variant, or a field of a struct variant.
struct Member {
    ident: String,
    rename: Option<String>,
    variant_of: Option<String>,
}

/// The named members of an item whose first line is `body[0]`.
///
/// A struct's fields are at depth 1. An enum's variants are at depth 1 and a
/// struct variant's fields at depth 2. Members marked `skip` or `flatten` are
/// left out: the first writes nothing and the second writes another type's
/// names, which that type's own declaration answers for.
fn members(body: &[&str], kind: &str) -> Vec<Member> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut pending = String::new();
    // The same attributes with string literals removed, so a `rename = "skip"`
    // is not read as the `skip` marker.
    let mut pending_code = String::new();
    let mut in_attr = false;
    let mut variant: Option<String> = None;
    for (n, raw) in body.iter().enumerate() {
        let code = code_of(raw);
        let t = raw.trim();
        if n == 0 {
            if code.contains(';') && !code.contains('{') {
                return out;
            }
            depth += code.matches('{').count() as i32 - code.matches('}').count() as i32;
            if depth <= 0 {
                return out;
            }
            continue;
        }
        if in_attr || t.starts_with("#[") {
            pending.push_str(raw);
            pending.push(' ');
            pending_code.push_str(&code_of(raw));
            pending_code.push(' ');
            in_attr = !t.ends_with(']');
            continue;
        }
        if t.starts_with("//") || t.is_empty() {
            continue;
        }
        let at = depth;
        let ident: String = t
            .trim_start_matches("pub(crate) ")
            .trim_start_matches("pub ")
            .trim_start_matches("r#")
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let skipped = has_word(&pending_code, "skip")
            || has_word(&pending_code, "flatten")
            || has_word(&pending_code, "skip_serializing");
        let rename = if pending.contains("serde(") {
            attr_value(&pending, "rename")
        } else {
            None
        };
        let rest = t
            .trim_start_matches("pub(crate) ")
            .trim_start_matches("pub ")
            .trim_start_matches("r#");
        let rest = rest[ident.len().min(rest.len())..].trim_start();
        let is_field_line = rest.starts_with(':') && !rest.starts_with("::");
        if !ident.is_empty() {
            if kind == "struct" && at == 1 && is_field_line {
                if !skipped {
                    out.push(Member {
                        ident: ident.clone(),
                        rename,
                        variant_of: None,
                    });
                }
            } else if kind == "enum" && at == 1 {
                variant = Some(ident.clone());
                if !skipped {
                    out.push(Member {
                        ident: ident.clone(),
                        rename,
                        variant_of: None,
                    });
                }
                // A struct variant written on one line: `V { a: T, b: U },`.
                if let (Some(open), Some(close)) = (code.find('{'), code.rfind('}')) {
                    for part in code[open + 1..close.max(open + 1)].split(',') {
                        if let Some((name, _)) = part.split_once(':') {
                            let name = name.trim().trim_start_matches("r#");
                            if !name.is_empty()
                                && name.chars().all(|c| c.is_alphanumeric() || c == '_')
                            {
                                out.push(Member {
                                    ident: name.to_string(),
                                    rename: None,
                                    variant_of: variant.clone(),
                                });
                            }
                        }
                    }
                }
            } else if kind == "enum" && at == 2 && is_field_line && !skipped {
                out.push(Member {
                    ident: ident.clone(),
                    rename,
                    variant_of: variant.clone(),
                });
            }
        }
        pending.clear();
        pending_code.clear();
        depth += code.matches('{').count() as i32 - code.matches('}').count() as i32;
        if depth <= 0 {
            break;
        }
    }
    out
}

fn findings() -> Vec<Finding> {
    let root = crates_dir();
    let mut files = Vec::new();
    for krate in std::fs::read_dir(&root).unwrap() {
        let src = krate.unwrap().path().join("src");
        if src.is_dir() {
            sources(&src, &mut files);
        }
    }
    files.sort();
    let mut out = Vec::new();
    for f in files {
        let label = f
            .strip_prefix(&root)
            .unwrap()
            .display()
            .to_string()
            .replace('\\', "/");
        scan(&f, &label, &mut out);
    }
    out
}

#[test]
fn every_serialized_name_follows_the_convention_or_is_grandfathered() {
    let exempt: Vec<&str> = GRANDFATHERED
        .iter()
        .flat_map(|g| g.types.iter().copied())
        .collect();
    let offending: Vec<String> = findings()
        .into_iter()
        .filter(|f| !exempt.contains(&f.item.as_str()))
        .map(|f| format!("{}: {}", f.item, f.detail))
        .collect();
    assert!(
        offending.is_empty(),
        "these serialized names break spec 006's JSON naming convention (keys \
         camelCase, enum values kebab-case). Add `#[serde(rename_all = ...)]`, \
         or, for a persisted document that cannot move without a schema bump, \
         a GRANDFATHERED entry in tests/support/json_naming.rs naming why:\n{}",
        offending.join("\n")
    );
}

#[test]
fn every_grandfathered_type_still_needs_its_exemption() {
    // The list only shrinks: an entry whose type now conforms, or no longer
    // exists, is removed rather than left to excuse a future regression.
    let found: Vec<String> = findings().into_iter().map(|f| f.item).collect();
    let stale: Vec<&str> = GRANDFATHERED
        .iter()
        .flat_map(|g| g.types.iter().copied())
        .filter(|t| !found.iter().any(|f| f == t))
        .collect();
    assert!(
        stale.is_empty(),
        "grandfathered types that no longer need it; remove them: {stale:?}"
    );
}

#[test]
fn the_scan_sees_what_serde_would_write() {
    assert_eq!(renamed("recorded_at", Some("camelCase")), "recordedAt");
    assert_eq!(
        renamed("IncompleteEvidence", Some("kebab-case")),
        "incomplete-evidence"
    );
    assert_eq!(renamed("ToolUse", Some("camelCase")), "toolUse");
    assert_eq!(renamed("spec_spine", None), "spec_spine");
    assert_eq!(renamed("Pass", None), "Pass");
    let body = [
        "pub enum E {",
        "    /// A doc line with {braces}.",
        "    #[serde(rename = \"hook-started\")]",
        "    HookStarted { exit_code: Option<i64> },",
        "    Plain,",
        "}",
    ];
    let m = members(&body, "enum");
    let names: Vec<_> = m
        .iter()
        .map(|m| {
            (
                m.ident.as_str(),
                m.rename.as_deref(),
                m.variant_of.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        names,
        [
            ("HookStarted", Some("hook-started"), None),
            ("exit_code", None, Some("HookStarted")),
            ("Plain", None, None),
        ]
    );
}

#[test]
fn the_walker_flags_a_snake_case_key_at_any_depth_and_excuses_only_the_list() {
    let value = serde_json::json!({
        "fineKey": [{ "nested_key": 1 }],
        "spec_spine": "grandfathered",
        "Upper": { "alsoFine": true },
    });
    // Sorted, so the assertion does not depend on the map's iteration order.
    let mut found = json_naming::violations(&value);
    found.sort();
    assert_eq!(
        found,
        [
            "$.fineKey[]: \"nested_key\"".to_string(),
            "$: \"Upper\"".to_string(),
        ]
    );
}

#[test]
fn the_rules_read_as_the_convention_says() {
    assert!(is_key("observedSpecSpine") && is_key("root") && is_key("v2"));
    assert!(!is_key("spec_spine") && !is_key("Root") && !is_key("files-installed"));
    assert!(is_value("not-recorded") && is_value("qualified") && is_value("h1"));
    assert!(!is_value("NotRecorded") && !is_value("not_recorded") && !is_value(""));
}

#[test]
fn a_rename_spelled_skip_is_not_read_as_the_skip_marker() {
    let body = [
        "pub struct S {",
        "    #[serde(rename = \"skip\")]",
        "    odd_one: u8,",
        "    #[serde(skip)]",
        "    gone_one: u8,",
        "}",
    ];
    let m = members(&body, "struct");
    let names: Vec<_> = m
        .iter()
        .map(|m| (m.ident.as_str(), m.rename.as_deref()))
        .collect();
    assert_eq!(names, [("odd_one", Some("skip"))]);
}
