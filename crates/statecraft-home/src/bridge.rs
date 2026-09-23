//! The root instruction bridge: one line of a file this product does not own.
//!
//! Spec 002 section 3.13. The root `AGENTS.md` stays the user's. This product
//! takes exactly one **managed modification** of it:
//!
//! > the first line is exactly `@.statecraft/AGENTS.md`
//!
//! The import line is a project convention and not a proven universal
//! mechanism. Claude Code documents expansion of an `@path` import; Codex's
//! documentation does not establish an equivalent. So this module places the
//! line and records it, and it claims nothing about delivery: that is
//! [`crate::delivery`], which evaluates a harness's documented load rule
//! against the actual tree.

use crate::project::{INSTRUCTIONS, ROOT_INSTRUCTIONS};
use serde::Serialize;
use statecraft_environment::digest::digest_bytes;

/// The line inserted, exactly.
pub const IMPORT_LINE: &str = "@.statecraft/AGENTS.md";

/// What the existing root file is, before anything is inserted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Classification {
    /// There is no root instruction file.
    Absent,
    /// The file is byte-identical to a generated one this product recognizes.
    ///
    /// Reported, never swept: the operator decides whether generated content
    /// they did not write should stay.
    GeneratedUnmodified {
        /// The label of whatever it matched.
        matched: String,
    },
    /// Anything else: the user has written in it, or it came from somewhere
    /// this product cannot recognize. Preserved, and any conflict reported.
    Customized,
}

/// What inserting the line will do, or did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    /// The file does not exist and will be created holding the line.
    Create,
    /// The file exists without the line; it is inserted first.
    Insert,
    /// The file carries the line somewhere other than first; it moves.
    MoveToFirst,
    /// The line is already the first line. Nothing to do.
    AlreadyFirst,
}

impl Action {
    /// Whether performing this action changes the file.
    pub fn changes_the_file(self) -> bool {
        !matches!(self, Action::AlreadyFirst)
    }

    /// A one-word rendering.
    pub fn word(self) -> &'static str {
        match self {
            Action::Create => "create",
            Action::Insert => "insert",
            Action::MoveToFirst => "move-to-first",
            Action::AlreadyFirst => "already-first",
        }
    }
}

/// The bridge, computed without writing anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// The file this is about.
    pub path: String,
    /// What the existing file is.
    pub classification: Classification,
    /// What will happen.
    pub action: Action,
    /// The digest of the file before, when it exists.
    pub digest_before: Option<String>,
    /// The digest of the file as this product would leave it.
    pub digest_after: String,
    /// The bytes this product would leave. Computed here so the apply writes
    /// exactly what the plan showed.
    #[serde(skip)]
    pub contents_after: String,
    /// How many copies of the line the existing file carried. More than one is
    /// what "must not accumulate duplicates" is about.
    pub existing_copies: usize,
}

/// A generated file this product recognizes by its exact bytes.
///
/// Supplied by the caller rather than vendored: the only bytes this product can
/// honestly recognize are the ones a producer it calls actually returned, and
/// hardcoding a digest table would be carrying kit bytes by another name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownGenerated {
    /// What to call it in a report.
    pub label: String,
    /// The exact bytes.
    pub contents: String,
}

/// Compute the bridge. Pure: no filesystem, no clock.
///
/// `existing` is the current root file's text, or `None` when there is none.
pub fn plan(existing: Option<&str>, known: &[KnownGenerated]) -> Plan {
    let classification = match existing {
        None => Classification::Absent,
        Some(text) => match known.iter().find(|k| k.contents == text) {
            Some(k) => Classification::GeneratedUnmodified {
                matched: k.label.clone(),
            },
            None => Classification::Customized,
        },
    };

    let existing_copies = existing
        .map(|t| t.lines().filter(|l| l.trim_end() == IMPORT_LINE).count())
        .unwrap_or(0);

    let action = match existing {
        None => Action::Create,
        Some(text) => {
            let first_is_import = text.lines().next().map(str::trim_end) == Some(IMPORT_LINE);
            if first_is_import && existing_copies == 1 {
                Action::AlreadyFirst
            } else if existing_copies > 0 {
                Action::MoveToFirst
            } else {
                Action::Insert
            }
        }
    };

    let contents_after = bridged(existing.unwrap_or(""));
    Plan {
        path: ROOT_INSTRUCTIONS.to_string(),
        classification,
        action,
        digest_before: existing.map(|t| digest_bytes(t.as_bytes())),
        digest_after: digest_bytes(contents_after.as_bytes()),
        contents_after,
        existing_copies,
    }
}

/// The bridged form of some text.
///
/// Every line equal to the import is removed, then one is prepended. That is
/// what makes the operation idempotent and what stops a second run from
/// accumulating a second copy. Leading blank lines left behind by the removal
/// are not accumulated; nothing else is touched, and no other line is
/// reordered, rewritten or dropped.
pub fn bridged(existing: &str) -> String {
    let remainder: String = existing
        .split_inclusive('\n')
        .filter(|line| line.trim_end_matches(['\n', '\r']).trim_end() != IMPORT_LINE)
        .collect();
    let remainder = remainder.trim_start_matches(['\n', '\r']);
    if remainder.is_empty() {
        format!("{IMPORT_LINE}\n")
    } else {
        format!("{IMPORT_LINE}\n\n{remainder}")
    }
}

/// The text with this product's line taken back out.
///
/// Removal takes the line and nothing else, which is the other half of
/// "tracked modification, not ownership". An empty result is an empty file, not
/// a deleted one: this product did not create every file it bridged, and it
/// cannot tell from the text which ones it did.
///
/// A pure text transform. `env remove` does not call it: it takes the bridge
/// back through `statecraft_environment::apply::remove_with`, which locates
/// the line by the manifest's record and refuses when ownership cannot be
/// decided.
pub fn unbridged(existing: &str) -> String {
    existing
        .split_inclusive('\n')
        .filter(|line| line.trim_end_matches(['\n', '\r']).trim_end() != IMPORT_LINE)
        .collect::<String>()
        .trim_start_matches(['\n', '\r'])
        .to_string()
}

/// The modification record for a performed bridge.
pub fn record(plan: &Plan, written_at: &str) -> statecraft_environment::manifest::Modification {
    statecraft_environment::manifest::Modification {
        path: plan.path.clone(),
        kind: statecraft_environment::manifest::ModificationKind::ImportBridge,
        line: IMPORT_LINE.to_string(),
        digest_before: plan.digest_before.clone(),
        digest_after: plan.digest_after.clone(),
        written_at: written_at.to_string(),
    }
}

/// The import line names the managed instructions, and a test holds it.
///
/// Two constants that must agree and live in different modules is exactly the
/// pair that drifts, so the agreement is asserted rather than assumed.
pub fn import_names_the_managed_file() -> bool {
    IMPORT_LINE == format!("@{INSTRUCTIONS}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(text: &str) -> String {
        bridged(text)
    }

    #[test]
    fn the_import_names_the_managed_instructions() {
        assert!(import_names_the_managed_file());
    }

    #[test]
    fn an_absent_file_is_created_holding_only_the_import() {
        let p = plan(None, &[]);
        assert_eq!(p.action, Action::Create);
        assert_eq!(p.classification, Classification::Absent);
        assert_eq!(p.contents_after, "@.statecraft/AGENTS.md\n");
        assert!(p.digest_before.is_none());
    }

    #[test]
    fn an_existing_file_keeps_every_line_and_gains_the_import_first() {
        let user = "# My project\n\nDo the thing.\n";
        let out = apply(user);
        assert!(out.starts_with("@.statecraft/AGENTS.md\n"));
        assert!(out.contains("# My project"));
        assert!(out.contains("Do the thing."));
        assert_eq!(plan(Some(user), &[]).action, Action::Insert);
    }

    #[test]
    fn inserting_is_idempotent() {
        let user = "# My project\n\nDo the thing.\n";
        let once = apply(user);
        let twice = apply(&once);
        assert_eq!(once, twice);
        assert_eq!(plan(Some(&once), &[]).action, Action::AlreadyFirst);
        assert!(!plan(Some(&once), &[]).action.changes_the_file());
    }

    #[test]
    fn duplicates_never_accumulate() {
        let messy = "@.statecraft/AGENTS.md\n# Title\n@.statecraft/AGENTS.md\nbody\n";
        let out = apply(messy);
        assert_eq!(out.matches(IMPORT_LINE).count(), 1);
        assert!(out.starts_with(IMPORT_LINE));
        assert!(out.contains("# Title"));
        assert!(out.contains("body"));
        assert_eq!(plan(Some(messy), &[]).existing_copies, 2);
        assert_eq!(plan(Some(messy), &[]).action, Action::MoveToFirst);
    }

    #[test]
    fn an_import_that_is_not_first_moves_to_first_and_the_rest_is_kept() {
        let text = "# Title\n\n@.statecraft/AGENTS.md\n\nbody\n";
        let out = apply(text);
        assert_eq!(out.lines().next(), Some(IMPORT_LINE));
        assert_eq!(out.matches(IMPORT_LINE).count(), 1);
        assert!(out.contains("# Title"));
        assert!(out.contains("body"));
    }

    #[test]
    fn a_file_with_no_trailing_newline_is_preserved_as_written() {
        let out = apply("no newline at the end");
        assert!(out.ends_with("no newline at the end"));
        assert_eq!(apply(&out), out);
    }

    #[test]
    fn generated_content_is_recognized_and_never_rewritten_away() {
        let generated = "# AGENTS.md\n\ngenerated body\n";
        let known = [KnownGenerated {
            label: "producer spec-spine-core 0.23.0".into(),
            contents: generated.to_string(),
        }];
        let p = plan(Some(generated), &known);
        match &p.classification {
            Classification::GeneratedUnmodified { matched } => {
                assert!(matched.contains("spec-spine-core"))
            }
            other => panic!("expected generated-unmodified, got {other:?}"),
        }
        // Recognized is not removed: the body survives the insertion.
        assert!(p.contents_after.contains("generated body"));
    }

    #[test]
    fn one_edited_byte_makes_it_customized_rather_than_generated() {
        let generated = "# AGENTS.md\n\ngenerated body\n";
        let known = [KnownGenerated {
            label: "producer".into(),
            contents: generated.to_string(),
        }];
        let edited = format!("{generated}my own note\n");
        assert_eq!(
            plan(Some(&edited), &known).classification,
            Classification::Customized
        );
    }

    #[test]
    fn removal_takes_the_line_and_nothing_else() {
        let user = "# My project\n\nDo the thing.\n";
        let bridged_text = apply(user);
        assert_eq!(unbridged(&bridged_text), user);
    }

    #[test]
    fn the_record_is_a_modification_and_names_the_exact_line() {
        let p = plan(Some("# x\n"), &[]);
        let m = record(&p, "1970-01-01T00:00:00Z");
        assert_eq!(m.line, IMPORT_LINE);
        assert_eq!(m.path, "AGENTS.md");
        assert_eq!(m.digest_after, p.digest_after);
        assert!(m.digest_before.is_some());
    }
}
