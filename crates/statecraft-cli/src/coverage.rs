//! Where the command coverage of spec 004 section 3.17 reads its two inputs.
//!
//! The comparison, the reading of a command and the verdicts are
//! `statecraft_adapter::coverage`'s; this module only says **which tree** each
//! input is read from, which is rule 4 of that section:
//!
//! - **at planning**, before the attempt is appended, from the target's working
//!   tree as the operator invoked `run`;
//! - **at launch**, after the base commit is resolved and before the spawn,
//!   from that commit: the declaration as `git show <base>:<path>` returns it,
//!   and the suite planned against an export of the base commit's tree. The
//!   reused workspace is never read for this, because a previous session may
//!   have edited it.
//!
//! The requirement is asked of `spec-spine verify <spec> --plan --json`, run
//! with the `spec-spine` this product invokes for work selection
//! ([`SpecSpineCli`]'s binary). The allowance is the adapter manifest's own
//! commands plus the declaration's. Neither is derived from the other (rule 6).

use statecraft_adapter::coverage::{
    Allowance, Coverage, DECLARATION_PATH, SuitePlan, declared_commands, read_plan,
};
use statecraft_environment::digest::digest_bytes;
use statecraft_run::report::SpecSpineCli;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The allowance from its two sources: the adapter manifest's commands and
/// the declaration's bytes (`None` when the file does not exist).
fn allowance(file: Option<&[u8]>) -> Result<Allowance, String> {
    let declared = declared_commands(file)?;
    let digest = declared.canonical().map(|b| digest_bytes(&b));
    let manifest = statecraft_adapter_claude_code::manifest();
    Ok(Allowance::new(
        &manifest.requires_commands,
        &declared,
        digest,
    ))
}

fn compare(spec: &str, base: Option<&str>, plan: &SuitePlan, allowance: &Allowance) -> Coverage {
    Coverage::compare(
        spec,
        base,
        plan,
        digest_bytes(&plan.canonical()),
        allowance,
        digest_bytes(&allowance.canonical()),
    )
}

/// The spec-spine binary work selection uses.
fn reader() -> String {
    SpecSpineCli::default().binary
}

/// Rule 4, at planning: the working tree as the operator invoked `run`.
///
/// `Err` is a refusal naming why: an unreadable plan or a malformed
/// declaration. A `refused` verdict is returned as a coverage, and the caller
/// refuses on [`Coverage::refusal`].
pub fn at_planning(root: &Path, spec: &str) -> Result<Coverage, String> {
    let file = match std::fs::read(root.join(DECLARATION_PATH)) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("{DECLARATION_PATH} could not be read: {e}")),
    };
    let allowance = allowance(file.as_deref())?;
    let plan = read_plan(&reader(), root, spec).map_err(|e| e.to_string())?;
    Ok(compare(spec, None, &plan, &allowance))
}

/// What the intent records of the planning reading (rule 4): the verdict and
/// the digests of the plan and the allowance it read.
pub fn planning_record(planned: &Coverage) -> serde_json::Value {
    serde_json::json!({
        "phase": "planning",
        "spec": planned.spec,
        "verdict": planned.verdict,
        "planDigest": planned.plan_digest,
        "allowanceDigest": planned.allowance_digest,
    })
}

/// The declaration at `base`, as `git show` returns it; `None` when the base
/// commit holds no such file.
fn declaration_at(root: &Path, base: &str) -> Result<Option<Vec<u8>>, String> {
    let listed = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-tree", "--name-only", base, "--", DECLARATION_PATH])
        .output()
        .map_err(|e| format!("git ls-tree could not run: {e}"))?;
    if !listed.status.success() {
        return Err(format!(
            "git ls-tree {base} failed: {}",
            String::from_utf8_lossy(&listed.stderr).trim()
        ));
    }
    if listed.stdout.iter().all(u8::is_ascii_whitespace) {
        return Ok(None);
    }
    let shown = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["show", &format!("{base}:{DECLARATION_PATH}")])
        .output()
        .map_err(|e| format!("git show could not run: {e}"))?;
    if !shown.status.success() {
        return Err(format!(
            "git show {base}:{DECLARATION_PATH} failed: {}",
            String::from_utf8_lossy(&shown.stderr).trim()
        ));
    }
    Ok(Some(shown.stdout))
}

/// An export of one commit's tree, removed when dropped.
struct Export(PathBuf);

impl Drop for Export {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Export `base`'s tree into a fresh directory outside the target and outside
/// the workspace: `git archive` piped into `tar`.
fn export(root: &Path, base: &str) -> Result<Export, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!(
        "statecraft-base-export-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let export = Export(dir);
    let mut archive = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["archive", "--format=tar", base])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git archive could not run: {e}"))?;
    let tar_in = archive
        .stdout
        .take()
        .ok_or_else(|| "git archive gave no output".to_string())?;
    let tar = Command::new("tar")
        .args(["-x", "-f", "-", "-C"])
        .arg(&export.0)
        .stdin(Stdio::from(tar_in))
        .output()
        .map_err(|e| format!("tar could not run: {e}"))?;
    let archived = archive
        .wait_with_output()
        .map_err(|e| format!("git archive: {e}"))?;
    if !archived.status.success() {
        return Err(format!(
            "git archive {base} failed: {}",
            String::from_utf8_lossy(&archived.stderr).trim()
        ));
    }
    if !tar.status.success() {
        return Err(format!(
            "the export of {base} could not be unpacked: {}",
            String::from_utf8_lossy(&tar.stderr).trim()
        ));
    }
    Ok(export)
}

/// Rule 4, at launch: the base commit. The launch comparison is the one
/// recorded. A digest that differs from planning's is named in
/// [`Coverage::drift`], which refuses the run whatever the verdict.
pub fn at_base(
    root: &Path,
    base: &str,
    spec: &str,
    planned: &Coverage,
) -> Result<Coverage, String> {
    let allowance = allowance(declaration_at(root, base)?.as_deref())?;
    let tree = export(root, base)?;
    let plan = read_plan(&reader(), &tree.0, spec).map_err(|e| e.to_string())?;
    drop(tree);
    let mut coverage = compare(spec, Some(base), &plan, &allowance);
    if coverage.allowance_digest != planned.allowance_digest {
        coverage.drift.push("allowance".to_string());
    }
    if coverage.plan_digest != planned.plan_digest {
        coverage.drift.push("suite plan".to_string());
    }
    Ok(coverage)
}

/// An attempt with no spec (the managed-startup trial): nothing is compared.
/// The allowance is still read at the base, so a malformed declaration is
/// named here as it is for a run (rule 1).
pub fn not_applicable(root: &Path, base: &str) -> Result<Coverage, String> {
    let allowance = allowance(declaration_at(root, base)?.as_deref())?;
    let mut coverage = Coverage::not_applicable(&allowance);
    coverage.base_commit = Some(base.to_string());
    Ok(coverage)
}

/// The answer a planning refusal gives: nothing was appended.
pub fn planning_refused_answer(
    reason: &str,
    coverage: Option<&Coverage>,
) -> crate::render::Answer<serde_json::Value> {
    let mut summary = format!(
        "refused ({}): {reason}\nnothing was appended and no process was created\n",
        statecraft_adapter::coverage::GUARD
    );
    if let Some(c) = coverage {
        summary.push_str(&c.render());
    }
    crate::render::Answer::new(
        serde_json::json!({
            "guard": statecraft_adapter::coverage::GUARD,
            "phase": "planning",
            "reason": reason,
            "coverage": coverage,
        }),
        crate::exit::Exit::Refused,
        summary,
    )
}
