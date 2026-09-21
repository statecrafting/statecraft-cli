//! Merging the producer's `.gitignore` fragment into a repository's own.
//!
//! Spec 010 section 3.5: the fragment is **merged**, never installed as a file.
//! The lines it contributes that are not already present are collected inside
//! one marked block, and no unrelated entry is replaced, reordered or removed.
//! Merging is idempotent.
//!
//! Section 3.2 supplies the one refusal: a `.gitignore` that ignores all of
//! `.statecraft/` takes the committed declaration, the managed instructions and
//! the compiled artifacts out of version control in one line. Governed project
//! metadata stays governed and only runtime state is excluded, so that line is
//! refused with its own reason rather than merged around.

use serde::Serialize;

/// The first line of the managed block.
pub const BEGIN: &str = "# >>> statecraft managed ignores >>>";

/// The last line of the managed block.
pub const END: &str = "# <<< statecraft managed ignores <<<";

/// The sentence between the marker and the fragment.
pub const BLOCK_NOTE: &str = "# Managed by Statecraft. Entries above and below this block are yours and are\n\
     # never reordered, rewritten or removed. Delete the whole block to opt out.";

/// Why a merge will not happen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Refusal {
    /// A line ignores the whole project area.
    AreaIgnored {
        /// The offending line, verbatim.
        line: String,
        /// Its one-based number.
        line_number: usize,
    },
}

impl Refusal {
    /// A one-line rendering for a report.
    pub fn describe(&self) -> String {
        match self {
            Refusal::AreaIgnored { line, line_number } => format!(
                ".gitignore line {line_number} `{line}` ignores all of .statecraft/; \
                 the declaration, the managed instructions and the compiled artifacts \
                 are committed, and only .statecraft/state/ is excluded"
            ),
        }
    }
}

/// What a merge produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merge {
    /// The file as this product would leave it.
    pub contents_after: String,
    /// Patterns this merge contributes.
    pub added: Vec<String>,
    /// Patterns the fragment carries that the file already had outside the
    /// managed block. Left exactly where they are.
    pub already_present: Vec<String>,
    /// True when the file is unchanged.
    pub unchanged: bool,
}

/// Merge a fragment into an existing `.gitignore`, or into none.
pub fn merge(existing: Option<&str>, fragment: &str) -> Result<Merge, Refusal> {
    let existing = existing.unwrap_or("");
    if let Some(refusal) = area_ignored(existing) {
        return Err(refusal);
    }

    let (before, after) = split_out_block(existing);
    let outside: Vec<String> = patterns_of(&before)
        .into_iter()
        .chain(patterns_of(&after))
        .collect();

    let mut added = Vec::new();
    let mut already_present = Vec::new();
    let mut block_body = String::new();
    for line in fragment.lines() {
        match pattern_of(line) {
            Some(pattern) => {
                if outside.contains(&pattern) {
                    already_present.push(pattern);
                    continue;
                }
                added.push(pattern);
                block_body.push_str(line);
                block_body.push('\n');
            }
            // A comment or a blank line: carried so the fragment's own guidance
            // survives, which is most of what it is for.
            None => {
                block_body.push_str(line);
                block_body.push('\n');
            }
        }
    }

    let contents_after = if added.is_empty() {
        // Nothing to contribute, so no block is written and no marker is left
        // behind in a file that would otherwise be untouched.
        rejoin(&before, "", &after)
    } else {
        let block = format!(
            "{BEGIN}\n{BLOCK_NOTE}\n{}{END}\n",
            trim_blank_edges(&block_body)
        );
        rejoin(&before, &block, &after)
    };

    let unchanged = contents_after == existing;
    Ok(Merge {
        contents_after,
        added,
        already_present,
        unchanged,
    })
}

/// A line that ignores the whole project area, if there is one.
pub fn area_ignored(text: &str) -> Option<Refusal> {
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with('!') {
            continue;
        }
        let normalized = t.trim_start_matches('/').trim_end_matches('/');
        let normalized = normalized
            .trim_end_matches("**")
            .trim_end_matches('/')
            .trim_end_matches('*')
            .trim_end_matches('/');
        if normalized == crate::project::AREA {
            return Some(Refusal::AreaIgnored {
                line: t.to_string(),
                line_number: i + 1,
            });
        }
    }
    None
}

/// The pattern a line declares, if it declares one.
fn pattern_of(line: &str) -> Option<String> {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return None;
    }
    Some(t.to_string())
}

/// Every pattern in some text.
fn patterns_of(text: &str) -> Vec<String> {
    text.lines().filter_map(pattern_of).collect()
}

/// Split an existing file around a managed block, dropping the block itself.
fn split_out_block(text: &str) -> (String, String) {
    let Some(begin) = text.find(BEGIN) else {
        return (text.to_string(), String::new());
    };
    let rest = &text[begin..];
    let end = match rest.find(END) {
        Some(e) => begin + e + END.len(),
        // An unterminated block: everything from the marker on is ours, which
        // is the only reading that cannot destroy a user's later entries by
        // guessing where the block stopped.
        None => text.len(),
    };
    let after = text[end..].trim_start_matches('\n').to_string();
    (text[..begin].to_string(), after)
}

/// Put a file back together from its three parts.
fn rejoin(before: &str, block: &str, after: &str) -> String {
    let mut out = String::new();
    out.push_str(before);
    if !block.is_empty() {
        if !out.is_empty() && !out.ends_with("\n\n") {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
        }
        out.push_str(block);
    }
    if !after.is_empty() {
        if !out.is_empty() && !out.ends_with("\n\n") {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
        }
        out.push_str(after);
    }
    out
}

/// Drop blank lines at both ends of the block body.
fn trim_blank_edges(body: &str) -> String {
    let trimmed = body.trim_matches('\n');
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAGMENT: &str = "# spec-spine metadata\n\
                            .statecraft/derived/**/build-meta.json\n\
                            \n\
                            # the declared state root\n\
                            .statecraft/state/\n";

    #[test]
    fn a_fresh_repository_gets_the_block_and_nothing_else() {
        let m = merge(None, FRAGMENT).unwrap();
        assert!(m.contents_after.starts_with(BEGIN));
        assert!(m.contents_after.trim_end().ends_with(END));
        assert_eq!(m.added.len(), 2);
        assert!(m.contents_after.contains(".statecraft/state/"));
    }

    #[test]
    fn unrelated_entries_are_preserved_exactly() {
        let existing = "target\n**/*.rs.bk\n\n# my own note\nnode_modules/\n";
        let m = merge(Some(existing), FRAGMENT).unwrap();
        for line in existing.lines() {
            assert!(
                m.contents_after.contains(line),
                "lost `{line}` from the user's own file"
            );
        }
        // And in their original order, ahead of the block.
        let block_at = m.contents_after.find(BEGIN).unwrap();
        assert!(m.contents_after[..block_at].contains("node_modules/"));
    }

    #[test]
    fn merging_twice_changes_nothing_the_second_time() {
        let once = merge(Some("target\n"), FRAGMENT).unwrap();
        let twice = merge(Some(&once.contents_after), FRAGMENT).unwrap();
        assert_eq!(once.contents_after, twice.contents_after);
        assert!(twice.unchanged);
    }

    #[test]
    fn a_pattern_the_user_already_has_is_not_duplicated() {
        let existing = ".statecraft/state/\ntarget\n";
        let m = merge(Some(existing), FRAGMENT).unwrap();
        assert_eq!(
            m.contents_after.matches(".statecraft/state/").count(),
            1,
            "the pattern appears once, where the user put it"
        );
        assert!(
            m.already_present
                .contains(&".statecraft/state/".to_string())
        );
    }

    #[test]
    fn a_fragment_that_contributes_nothing_writes_no_block() {
        let existing = ".statecraft/derived/**/build-meta.json\n.statecraft/state/\n";
        let m = merge(Some(existing), FRAGMENT).unwrap();
        assert!(!m.contents_after.contains(BEGIN));
        assert!(m.unchanged);
    }

    #[test]
    fn ignoring_the_whole_area_is_refused_with_the_line_named() {
        for line in [
            ".statecraft/",
            ".statecraft",
            "/.statecraft/",
            ".statecraft/**",
            ".statecraft/*",
        ] {
            let existing = format!("target\n{line}\n");
            match merge(Some(&existing), FRAGMENT) {
                Err(Refusal::AreaIgnored {
                    line: found,
                    line_number,
                }) => {
                    assert_eq!(found, line);
                    assert_eq!(line_number, 2);
                }
                other => panic!("expected a refusal for `{line}`, got {other:?}"),
            }
        }
    }

    #[test]
    fn ignoring_only_the_state_root_is_not_a_refusal() {
        assert!(merge(Some(".statecraft/state/\n"), FRAGMENT).is_ok());
        assert!(merge(Some(".statecraft/derived/**/build-meta.json\n"), FRAGMENT).is_ok());
    }

    #[test]
    fn a_negated_area_line_is_not_read_as_ignoring_it() {
        assert!(merge(Some("!.statecraft/\n"), FRAGMENT).is_ok());
    }

    #[test]
    fn entries_a_user_adds_after_the_block_survive_the_next_merge() {
        let first = merge(Some("target\n"), FRAGMENT).unwrap();
        let edited = format!("{}\n# added later\nmy-own-thing/\n", first.contents_after);
        let second = merge(Some(&edited), FRAGMENT).unwrap();
        assert!(second.contents_after.contains("my-own-thing/"));
        assert!(second.contents_after.contains("# added later"));
        assert!(second.contents_after.contains("target"));
        assert_eq!(second.contents_after.matches(BEGIN).count(), 1);
    }
}
