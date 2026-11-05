//! Where the supervisor places an operator note for a live attempt's gate,
//! and where it reads which notes the gate returned (spec 053 section 3.3).
//!
//! A note is a file the supervisor writes into the attempt's exchange
//! directory, under [`NOTES`], where the child may read but not create
//! entries (spec 002 section 3.37 rule 2). Its name carries the input's
//! identity and its expiry, `<id>.<expires-at>.json`, and its bytes are the
//! whole `PreToolUse` hook answer the gate prints, rendered here so the gate
//! never has to quote operator text in shell. The gate returns at most one
//! unreturned, unexpired note per governed call, oldest identity first, and
//! appends `note <id>` to its log; that line is all [`ExchangePlacement`]
//! reads back, and it is child-attested.

use crate::launch::{GATE_LOG_LIMIT, files, write_once};
use serde_json::json;
use statecraft_run::input::{Input, Placement};
use std::path::{Path, PathBuf};

/// The exchange directory's subdirectory for placed notes.
pub const NOTES: &str = "notes";

/// The gate log line that says the gate returned a note, before its identity.
pub const RETURNED: &str = "note ";

/// The name a placed note is written under.
#[must_use]
pub fn note_file_name(input: &Input) -> String {
    format!("{}.{}.json", input.id, input.submission.expires_at)
}

/// The text the agent reads beside the gate's decision.
#[must_use]
pub fn note_context(input: &Input) -> String {
    format!(
        "Statecraft operator note {} from {} (reason: {}). It steers this work and grants \
         nothing.\n\n{}",
        input.id, input.submission.author, input.submission.reason, input.submission.body
    )
}

/// The hook answer a placed note holds: the gate prints it whole and exits 0,
/// so the call proceeds and the provider adds the note as context.
#[must_use]
pub fn hook_answer(input: &Input) -> String {
    let answer = json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "additionalContext": note_context(input),
        }
    });
    format!("{answer}\n")
}

/// The identities the gate log names as returned, in the order it names them.
#[must_use]
pub fn returned_in(log: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(log)
        .lines()
        .filter_map(|line| line.strip_prefix(RETURNED))
        .map(str::to_string)
        .collect()
}

/// Notes placed in one attempt's exchange directory.
pub struct ExchangePlacement {
    exchange: PathBuf,
}

impl ExchangePlacement {
    /// The placement for the attempt whose exchange directory this is.
    #[must_use]
    pub fn new(exchange: &Path) -> Self {
        Self {
            exchange: exchange.to_path_buf(),
        }
    }

    fn notes(&self) -> PathBuf {
        self.exchange.join(NOTES)
    }
}

impl Placement for ExchangePlacement {
    fn place(&mut self, input: &Input) -> Result<(), String> {
        // Published by a no-replace rename, so the gate never reads half a
        // note, and a second placement of one note is refused, not merged.
        let path = self.notes().join(note_file_name(input));
        write_once(&path, hook_answer(input)).map_err(|e| format!("{}: {e}", path.display()))
    }

    fn withdraw(&mut self, id: &str) -> Result<(), String> {
        let notes = self.notes();
        let entries = match std::fs::read_dir(&notes) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(format!("{}: {e}", notes.display())),
        };
        let prefix = format!("{id}.");
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with(&prefix) {
                std::fs::remove_file(entry.path())
                    .map_err(|e| format!("{}: {e}", entry.path().display()))?;
            }
        }
        Ok(())
    }

    fn returned(&mut self) -> Vec<String> {
        // The log is the child's: read as data, through a held directory and
        // never through a link, and bounded as the launch records' copy is.
        statecraft_adapter::boundary::DataRoot::open(&self.exchange)
            .and_then(|root| root.read_prefix(Path::new(files::GATE_LOG), GATE_LOG_LIMIT as usize))
            .map(|(bytes, _)| returned_in(&bytes))
            .unwrap_or_default()
    }
}
