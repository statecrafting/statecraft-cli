//! The exit statements a shell script states, for the family exit contract
//! (spec 002 section 5, 2026-09-25, profile revision 7).
//!
//! A statement is `exit` or `leave` in command position: at the start of a
//! line, or after `;`, `{`, `(`, `&&`, `||`, `then`, `else`, `do` or a case
//! arm's `)`. A word inside a message ("spec-spine exit 3", "(exit $rc)") is
//! not in command position and is not a statement. Comment lines and awk
//! programs (whose `exit` is awk's) are skipped.

/// One exit statement: its 1-based line, its verb (`exit` or `leave`) and the
/// word after it (empty for a bare `exit`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitStatement {
    pub line: usize,
    pub verb: &'static str,
    pub code: String,
}

/// Every exit statement in `text`.
pub fn exit_statements(text: &str) -> Vec<ExitStatement> {
    let mut out = Vec::new();
    let mut in_awk = false;
    for (i, raw) in text.lines().enumerate() {
        let mut line = raw.to_string();
        if in_awk {
            // The program ends at the line whose first quote closes it.
            match line.find('\'') {
                Some(q) => {
                    line = line[q + 1..].to_string();
                    in_awk = false;
                }
                None => continue,
            }
        }
        // A one-line awk program, and the opening of a multi-line one.
        while let Some(start) = line.find("awk '") {
            let rest = &line[start + 5..];
            match rest.find('\'') {
                Some(end) => line = format!("{}{}", &line[..start], &rest[end + 1..]),
                None => {
                    line.truncate(start);
                    in_awk = true;
                }
            }
        }
        if line.trim_start().starts_with('#') {
            continue;
        }
        let words: Vec<&str> = line
            .split(|c: char| c.is_whitespace())
            .filter(|w| !w.is_empty())
            .collect();
        for (k, w) in words.iter().enumerate() {
            let verb = match w.trim_end_matches(';') {
                "exit" => "exit",
                "leave" => "leave",
                _ => continue,
            };
            if words.get(k + 1).is_some_and(|n| n.starts_with("()")) {
                continue; // the definition `leave() {`
            }
            let prev = if k == 0 { "" } else { words[k - 1] };
            let command_position = k == 0
                || prev.ends_with(';')
                || prev.ends_with(')')
                || matches!(prev, "{" | "(" | "&&" | "||" | "then" | "else" | "do");
            if !command_position {
                continue;
            }
            let code = if w.ends_with(';') {
                String::new()
            } else {
                words
                    .get(k + 1)
                    .map(|n| n.trim_end_matches([';', '\'']).to_string())
                    .filter(|n| !matches!(n.as_str(), "}" | "fi" | ";;" | "esac"))
                    .unwrap_or_default()
            };
            out.push(ExitStatement {
                line: i + 1,
                verb,
                code,
            });
        }
    }
    out
}
