//! The one spec-spine an operation runs (spec 029).
//!
//! Every verb that asks spec-spine anything asks the executable
//! `statecraft_home::spec_spine::select_here` chose for the target, and asks it
//! once: the selection is made at the first question and every later question
//! in the same process reuses it, so a verb and every step inside it name the
//! same program and the same digest. A process runs one operation, which is
//! what makes a per-process record "once per operation" (section 3.1).
//!
//! The selection rule is the home crate's; this module only keeps its answer
//! and hands it to the types that invoke spec-spine, none of which has a
//! default program any more.

use statecraft_acceptance::suite::SpecSpineVerify;
use statecraft_home::spec_spine::{self, Selection};
use statecraft_run::report::{ReportError, SpecSpineCli};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

type Resolved = Mutex<BTreeMap<PathBuf, Arc<Selection>>>;

/// The selection for `root`, made once per process.
pub fn selection(root: &Path) -> Arc<Selection> {
    static RESOLVED: OnceLock<Resolved> = OnceLock::new();
    let key = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let map = RESOLVED.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut map = map.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    map.entry(key)
        .or_insert_with(|| Arc::new(spec_spine::select_here(root)))
        .clone()
}

/// The selected program, or the refusal naming the pin, every candidate
/// passed over and the preparation command.
pub fn program(root: &Path) -> Result<String, String> {
    selection(root)
        .judge()
        .map(|selected| selected.program.display().to_string())
}

/// The report source for `root`, or the refusal as a report error an answer
/// already knows how to render (spec-spine not runnable: a refusal, exit 2).
pub fn report_source(root: &Path) -> Result<SpecSpineCli, ReportError> {
    program(root)
        .map(|binary| SpecSpineCli { binary })
        .map_err(|detail| ReportError::NotRunnable {
            path: root.display().to_string(),
            detail,
        })
}

/// The suite source for `root`.
pub fn verify_source(root: &Path) -> Result<SpecSpineVerify, String> {
    program(root).map(|binary| SpecSpineVerify { binary })
}

/// The version the resolution observed, for `doctor`'s pin comparison and
/// for what an acceptance names: the selected program's, or, when the pin
/// admitted none, the first candidate the resolution passed over. `None` when
/// the resolution saw no executable that answered `--version`. Never a
/// separate probe.
pub fn observed_version(root: &Path) -> Option<String> {
    let selection = selection(root);
    match selection.judge() {
        Ok(selected) => selected.version.clone(),
        Err(_) => selection
            .considered
            .iter()
            .find_map(|passed| passed.version.clone()),
    }
}

/// What `doctor` reports about the judge (spec 029 section 3.3): the
/// resolution, what it passed over, and a `spec-spine` on `PATH` that differs
/// from it, as information and never as the judge.
pub fn doctor_notes(root: &Path) -> Vec<String> {
    let selection = selection(root);
    let mut notes = Vec::new();
    match selection.judge() {
        Ok(selected) => notes.push(format!("spec-spine judge: {}", selected.describe())),
        Err(why) => notes.push(format!("spec-spine judge: none selected: {why}")),
    }
    for remark in selection.remarks() {
        notes.push(format!("spec-spine resolution: {remark}"));
    }
    if let Some(on_path) = first_on_path()
        && selection
            .judge()
            .map_or(true, |selected| !same_file(&selected.program, &on_path))
    {
        notes.push(format!(
            "spec-spine on PATH: {} is not the judge for this project, and is reported for \
             information only",
            on_path.display()
        ));
    }
    notes
}

fn first_on_path() -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(spec_spine::PROGRAM))
        .find(|candidate| candidate.is_file())
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}
