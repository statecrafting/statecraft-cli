//! `provider compare <captures-dir> --corpus <corpus-dir>` (spec 052 section
//! 3.4).
//!
//! A binding onto [`statecraft_adapter_claude_code::golden`]: it reads the two
//! directories through that module's loaders, calls [`compare`] once and
//! renders the comparison. It reads no product home and no project, spawns
//! nothing and opens no connection (R-3), so it is dispatched before either
//! is consulted.

use serde_json::{Value, json};
use statecraft_adapter_claude_code::golden::{CaptureSet, Corpus, GoldenError, compare};
use std::path::PathBuf;

use crate::exit::Exit;
use crate::render::{Answer, ErrorKind};

const USAGE: &str = "usage: provider compare <captures-dir> --corpus <corpus-dir>";

fn arguments(rest: &[String]) -> Result<(PathBuf, PathBuf), String> {
    let (mut captures, mut corpus) = (None, None);
    let mut words = rest.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--corpus" if corpus.is_none() => {
                corpus = Some(PathBuf::from(words.next().ok_or(USAGE)?));
            }
            w if !w.starts_with('-') && captures.is_none() => captures = Some(PathBuf::from(w)),
            _ => return Err(USAGE.to_string()),
        }
    }
    match (captures, corpus) {
        (Some(captures), Some(corpus)) => Ok((captures, corpus)),
        _ => Err(USAGE.to_string()),
    }
}

fn refusal(e: &GoldenError) -> Answer<Value> {
    let (exit, kind) = match e {
        GoldenError::Read { .. } => (Exit::Failed, ErrorKind::Io),
        GoldenError::Schema { .. } => (Exit::Refused, ErrorKind::Schema),
        GoldenError::Invalid(_) => (Exit::Refused, ErrorKind::Validation),
        GoldenError::Refused(_) => (Exit::Refused, ErrorKind::Refused),
    };
    Answer::new(json!({}), exit, e.to_string()).with_kind(kind)
}

/// `provider compare`: one version's captures against the goldens.
///
/// Exit 0 when the version is qualifiable, 1 when a scenario diverges
/// unreviewed or is missing, naming them, and 2 when a capture or the corpus
/// cannot be compared at all.
pub fn compare_verb(rest: &[String]) -> Result<Answer<Value>, String> {
    let (captures, corpus) = arguments(rest)?;
    let loaded = Corpus::from_dir(&corpus).and_then(|corpus| {
        let set = CaptureSet::from_dir(&captures)?;
        compare(&corpus, &set)
    });
    let comparison = match loaded {
        Ok(c) => c,
        Err(e) => return Ok(refusal(&e)),
    };
    let mut summary = String::new();
    for row in &comparison.rows {
        let status = serde_json::to_value(row.status).unwrap_or(Value::Null);
        let status = status.as_str().unwrap_or("unknown");
        if row.differing_fields.is_empty() {
            summary.push_str(&format!("{}: {status}\n", row.scenario));
        } else {
            summary.push_str(&format!(
                "{}: {status} ({})\n",
                row.scenario,
                row.differing_fields.join(", ")
            ));
        }
    }
    let exit = if comparison.qualifiable {
        summary.push_str(&format!(
            "provider {} is qualifiable against the goldens",
            comparison.provider_version
        ));
        Exit::Ok
    } else {
        summary.push_str(&format!(
            "provider {} is not qualifiable: {}",
            comparison.provider_version,
            comparison.blocking.join(", ")
        ));
        Exit::Finding
    };
    let value = serde_json::to_value(&comparison).unwrap_or(Value::Null);
    Ok(Answer::new(value, exit, summary))
}
