//! Performing a plan, upgrading, and removing.
//!
//! Spec 002 sections 3.4 and 3.6. Three outcomes, and the middle one is the
//! point: `partial` is a SUCCESS of the upgrade's contract, not a silent one.
//! Every withheld path is named with its reason, and the operator decides what
//! to do per path. Nothing here resolves a conflict by choosing.

use crate::adapter::{Declaration, HarnessProbe};
use crate::claimant::{ForeignClaims, resolve};
use crate::digest::digest_bytes;
use crate::digest::digest_file;
use crate::manifest::{Class, Entry, Manifest, ModificationKind, Source, SourceKind, WRITER_WAIT};
use crate::plan::{Plan, WithheldWrite, plan};
use crate::time::{Clock, rfc3339_utc};
use std::path::Path;

/// How an apply, upgrade or removal ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Every planned write succeeded.
    Applied {
        /// The paths written, in path order.
        written: Vec<String>,
    },
    /// Some writes were withheld. Every withheld path is named with its reason.
    Partial {
        /// The paths written.
        written: Vec<String>,
        /// The paths not written, and why.
        withheld: Vec<WithheldWrite>,
    },
    /// A precondition failed and nothing was written.
    Refused {
        /// Why, one line per reason.
        reasons: Vec<String>,
    },
}

impl Outcome {
    /// True when nothing at all was written.
    pub fn refused(&self) -> bool {
        matches!(self, Outcome::Refused { .. })
    }

    /// A one-word summary: the vocabulary section 3.4 fixes.
    pub fn word(&self) -> &'static str {
        match self {
            Outcome::Applied { .. } => "applied",
            Outcome::Partial { .. } => "partial",
            Outcome::Refused { .. } => "refused",
        }
    }
}

/// What went wrong performing an operation.
#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    /// A filesystem operation failed.
    #[error("i/o at {path}: {source}")]
    Io {
        /// The path involved.
        path: String,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The manifest could not be read or written.
    #[error(transparent)]
    Manifest(#[from] crate::manifest::ManifestError),
}

/// Compute and perform a plan, updating the manifest to match what was written.
///
/// `env upgrade` is this same function against a newer set of declarations: the
/// spec separates the two verbs for the operator, not for the machine, and
/// giving them different code paths is how their conflict rules would drift.
pub fn apply(
    root: &Path,
    manifest: &mut Manifest,
    declarations: &[Declaration],
    probe: &dyn HarnessProbe,
    foreign: &ForeignClaims,
    clock: &dyn Clock,
) -> Result<Outcome, ApplyError> {
    // Under the manifest lock from the plan to the write, and refused before
    // any file is touched when the manifest moved since `manifest` was read:
    // otherwise this apply would erase another writer's recorded change, or a
    // transfer already reported as applied (spec 002 section 3.35).
    let _held = crate::manifest::lock(root, WRITER_WAIT)?;
    manifest.ensure_current(root)?;
    let computed = plan(root, Some(manifest), declarations, probe, foreign).map_err(|source| {
        ApplyError::Io {
            path: root.display().to_string(),
            source,
        }
    })?;
    perform(root, manifest, &computed, declarations, clock)
}

/// [`apply`] against the manifest on disk, read under the manifest lock, or a
/// new one with `pins` when there is none. This is the form a command uses:
/// nothing another writer records between the read and the write can be lost.
pub fn apply_current(
    root: &Path,
    declarations: &[Declaration],
    probe: &dyn HarnessProbe,
    foreign: &ForeignClaims,
    clock: &dyn Clock,
    pins: impl FnOnce() -> crate::manifest::Pins,
) -> Result<Outcome, ApplyError> {
    // A refusal at plan time depends on the declarations alone, and is
    // answered before the lock, so it creates not even runtime state.
    let colliding = crate::adapter::collisions(declarations);
    if !colliding.is_empty() {
        return Ok(Outcome::Refused {
            reasons: colliding
                .into_iter()
                .map(|c| crate::plan::Refusal::AdapterPathCollision(c).describe())
                .collect(),
        });
    }
    let _held = crate::manifest::lock(root, WRITER_WAIT)?;
    let mut manifest = match Manifest::read(root)? {
        Some(m) => m,
        None => {
            let m = Manifest::new(pins());
            m.belongs_to(root, None);
            m
        }
    };
    apply(root, &mut manifest, declarations, probe, foreign, clock)
}

/// Perform an already-computed plan.
///
/// Separate from [`apply`] so an operator can be shown exactly the plan that
/// will run. A preview computed by one function and executed by another is not
/// a preview.
pub fn perform(
    root: &Path,
    manifest: &mut Manifest,
    computed: &Plan,
    declarations: &[Declaration],
    clock: &dyn Clock,
) -> Result<Outcome, ApplyError> {
    let _held = crate::manifest::lock(root, WRITER_WAIT)?;
    manifest.ensure_current(root)?;
    if computed.refused() {
        return Ok(Outcome::Refused {
            reasons: computed.refusals.iter().map(|r| r.describe()).collect(),
        });
    }

    let written_at = rfc3339_utc(clock.now_unix());
    let mut written = Vec::new();

    // Named replacements first (spec 002 section 3.4). Every one is staged and
    // checked before any is renamed into place, so a file that moved since the
    // plan refuses the whole apply with nothing written.
    if let Some(refused) = replace_named(root, manifest, computed, &written_at, &mut written)? {
        return Ok(refused);
    }

    for w in &computed.writes {
        let target = resolve(root, &w.path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ApplyError::Io {
                path: w.path.clone(),
                source,
            })?;
        }
        std::fs::write(&target, &w.contents).map_err(|source| ApplyError::Io {
            path: w.path.clone(),
            source,
        })?;
        manifest.upsert(Entry {
            path: w.path.clone(),
            class: Class::Managed,
            source: Source {
                kind: SourceKind::Adapter,
                identity: w.adapter.clone(),
            },
            digest: w.digest.clone(),
            bytes: w.contents.len() as u64,
            written_at: written_at.clone(),
            transfer: manifest.entry(&w.path).and_then(|e| e.transfer.clone()),
            role: w.role,
        });
        written.push(w.path.clone());
    }

    // Pins record the adapter set that produced this environment. Recorded,
    // never silently satisfied: doctor compares them, nothing repairs them.
    for d in declarations {
        manifest
            .pins
            .adapters
            .insert(d.name.clone(), d.version.clone());
    }

    manifest.write(root)?;

    Ok(if computed.withheld.is_empty() {
        Outcome::Applied { written }
    } else {
        Outcome::Partial {
            written,
            withheld: computed.withheld.clone(),
        }
    })
}

/// What an apply carrying per-path consents did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consented {
    /// The outcome, in section 3.4's vocabulary.
    pub outcome: Outcome,
    /// What became of every path the operator named.
    pub named: Vec<crate::replace::Named>,
    /// Staged files an earlier interrupted replacement left under
    /// [`crate::replace::STAGING_DIR`], removed before this one staged
    /// anything.
    pub swept: Vec<String>,
}

/// [`apply_consented`] against the manifest on disk, or a new one with `pins`
/// when there is none. The command reads nothing itself and hands over no
/// manifest of its own. [`apply_consented`] takes the manifest lock once its
/// refusals are decided and refuses there if the manifest moved since this
/// read, so nothing another writer records in between can be lost; [`perform`]
/// writes the manifest.
pub fn apply_consented_current(
    root: &Path,
    declarations: &[Declaration],
    probe: &dyn HarnessProbe,
    foreign: &ForeignClaims,
    clock: &dyn Clock,
    pins: impl FnOnce() -> crate::manifest::Pins,
    consents: &[crate::replace::Consent],
) -> Result<Consented, ApplyError> {
    // A refusal at plan time depends on the declarations alone, and is
    // answered before the lock, so it creates not even runtime state.
    let colliding = crate::adapter::collisions(declarations);
    if !colliding.is_empty() {
        return Ok(Consented {
            outcome: Outcome::Refused {
                reasons: colliding
                    .into_iter()
                    .map(|c| crate::plan::Refusal::AdapterPathCollision(c).describe())
                    .collect(),
            },
            named: Vec::new(),
            swept: Vec::new(),
        });
    }
    let mut manifest = match Manifest::read(root)? {
        Some(m) => m,
        None => {
            let m = Manifest::new(pins());
            m.belongs_to(root, None);
            m
        }
    };
    apply_consented(
        root,
        &mut manifest,
        declarations,
        probe,
        foreign,
        clock,
        consents,
    )
}

/// Compute a plan naming the operator's consented replacements, and perform it
/// only if every consent's plan identity is still the one the plan computes.
///
/// Spec 002 section 3.4. A consent whose identity differs is a stale plan: the
/// drifted file, the manifest entry or the replacement changed since `env plan`
/// reported it. That refuses the whole apply, and nothing is written. A path
/// already holding the replacement is `already-satisfied` whatever identity is
/// given, which is what makes repeating a successful request safe.
pub fn apply_consented(
    root: &Path,
    manifest: &mut Manifest,
    declarations: &[Declaration],
    probe: &dyn HarnessProbe,
    foreign: &ForeignClaims,
    clock: &dyn Clock,
    consents: &[crate::replace::Consent],
) -> Result<Consented, ApplyError> {
    let paths: Vec<String> = consents.iter().map(|c| c.path.clone()).collect();
    let computed =
        crate::plan::plan_naming(root, Some(manifest), declarations, probe, foreign, &paths)
            .map_err(|source| ApplyError::Io {
                path: root.display().to_string(),
                source,
            })?;
    let mut stale = Vec::new();
    for n in &computed.named {
        if let crate::replace::Named::Replace(r) = n {
            let given = consents
                .iter()
                .find(|c| c.path == r.path)
                .map(|c| c.plan_id.as_str())
                .unwrap_or("");
            if given != r.plan_id {
                stale.push(format!(
                    "stale plan for {}: consented to {given}, the plan now is {} (recorded {}, found {}, replacement {}); nothing written",
                    r.path, r.plan_id, r.recorded, r.found, r.replacement
                ));
            }
        }
    }
    if computed.refused() || !stale.is_empty() {
        let mut reasons: Vec<String> = computed.refusals.iter().map(|r| r.describe()).collect();
        reasons.extend(stale);
        return Ok(Consented {
            outcome: Outcome::Refused { reasons },
            named: computed.named,
            swept: Vec::new(),
        });
    }
    // A refusal above wrote nothing, not even the lock file. From here to the
    // write the manifest lock is held, so no other writer's staging is swept,
    // and the manifest is refused if it moved since `manifest` was read, so no
    // change another writer recorded is erased. Files that moved since the
    // plan are caught when each replacement is staged and checked again.
    let _held = crate::manifest::lock(root, WRITER_WAIT)?;
    manifest.ensure_current(root)?;
    let swept = crate::replace::sweep(root).map_err(|source| ApplyError::Io {
        path: crate::replace::STAGING_DIR.to_string(),
        source,
    })?;
    let outcome = perform(root, manifest, &computed, declarations, clock)?;
    Ok(Consented {
        outcome,
        named: computed.named,
        swept,
    })
}

/// Perform the named replacements of a plan. `Some` is a refusal, returned
/// before any file is renamed into place.
fn replace_named(
    root: &Path,
    manifest: &mut Manifest,
    computed: &Plan,
    written_at: &str,
    written: &mut Vec<String>,
) -> Result<Option<Outcome>, ApplyError> {
    use crate::replace::Named;
    let replacements: Vec<&crate::replace::Replacement> = computed
        .named
        .iter()
        .filter_map(|n| match n {
            Named::Replace(r) => Some(r),
            _ => None,
        })
        .collect();

    let mut staged = Vec::new();
    let cleanup = |staged: &[std::path::PathBuf]| {
        for p in staged {
            let _ = std::fs::remove_file(p);
        }
    };
    for (i, r) in replacements.iter().enumerate() {
        match crate::replace::stage(root, &r.contents, i, Some(&resolve(root, &r.path))) {
            Ok(p) => staged.push(p),
            Err(source) => {
                cleanup(&staged);
                return Err(ApplyError::Io {
                    path: crate::replace::STAGING_DIR.to_string(),
                    source,
                });
            }
        }
    }
    // A last look before any rename: every file must still be a regular file,
    // reached through no link, holding the drift the plan saw. Any change
    // refuses the whole replacement with nothing renamed.
    let mut moved = Vec::new();
    for r in &replacements {
        if let Err(e) = crate::replace::check_target(root, &r.path, false) {
            moved.push(format!("{}: {e}; nothing written", r.path));
            continue;
        }
        let now = digest_file(&resolve(root, &r.path)).map_err(|source| ApplyError::Io {
            path: r.path.clone(),
            source,
        })?;
        if now.as_ref().map(|(d, _)| d) != Some(&r.found) {
            moved.push(format!(
                "{} changed while applying: the plan saw {}, it now holds {}; nothing written",
                r.path,
                r.found,
                now.map(|(d, _)| d).unwrap_or_else(|| "nothing".to_string())
            ));
        }
    }
    if !moved.is_empty() {
        cleanup(&staged);
        return Ok(Some(Outcome::Refused { reasons: moved }));
    }
    for (r, from) in replacements.iter().zip(&staged) {
        // Checked again immediately before this rename, not only before the
        // first: an earlier rename in this loop takes time. A failure here is
        // an i/o failure (4); files already renamed hold their replacement and
        // a repeated request reports them `already-satisfied`.
        let renamed = crate::replace::check_target(root, &r.path, false)
            .and_then(|()| std::fs::rename(from, resolve(root, &r.path)));
        if let Err(source) = renamed {
            cleanup(&staged);
            return Err(ApplyError::Io {
                path: r.path.clone(),
                source,
            });
        }
        record_managed(
            manifest,
            &r.path,
            &r.adapter,
            &r.replacement,
            r.replacement_bytes,
            written_at,
        );
        written.push(r.path.clone());
    }
    for n in &computed.named {
        if let Named::AlreadySatisfied {
            path,
            adapter,
            digest,
            records: true,
        } = n
        {
            let bytes = digest_file(&resolve(root, path))
                .map_err(|source| ApplyError::Io {
                    path: path.clone(),
                    source,
                })?
                .map(|(_, n)| n)
                .unwrap_or(0);
            record_managed(manifest, path, adapter, digest, bytes, written_at);
        }
    }
    Ok(None)
}

fn record_managed(
    manifest: &mut Manifest,
    path: &str,
    adapter: &str,
    digest: &str,
    bytes: u64,
    written_at: &str,
) {
    let transfer = manifest.entry(path).and_then(|e| e.transfer.clone());
    // A replacement keeps the role the entry was recorded with: a role never
    // changes as a side effect of rewriting bytes.
    let role = manifest.entry(path).map(|e| e.role).unwrap_or_default();
    manifest.upsert(Entry {
        path: path.to_string(),
        class: Class::Managed,
        source: Source {
            kind: SourceKind::Adapter,
            identity: adapter.to_string(),
        },
        digest: digest.to_string(),
        bytes,
        written_at: written_at.to_string(),
        transfer,
        role,
    });
}

/// Why a removal refused.
pub const NO_MANIFEST: &str =
    "no manifest at .statecraft/environment.json; removal will not guess which files were ours";

/// Remove every managed path whose digest still matches, and no other byte.
///
/// Section 3.6. A drifted managed path is reported and left: the operator
/// changed it, so it is theirs now. Adopted and user paths are never touched,
/// and nothing is restored to a remembered earlier state, because this product
/// never recorded one.
///
/// Removal with **no manifest present refuses**. The alternative is guessing,
/// and a wrong guess deletes a user's work.
///
/// With no bridge sites named, a recorded modification matches none, so it is
/// withheld rather than taken back; [`remove_with`] names the sites.
pub fn remove(root: &Path, clock: &dyn Clock) -> Result<Outcome, ApplyError> {
    remove_with(root, clock, &[]).map(|r| r.outcome)
}

/// Where this product places a tracked modification, its kind, and the exact
/// line it places. A record is acted on only when all three match a site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeSite {
    /// Repository-relative path.
    pub path: String,
    /// The kind of modification.
    pub kind: ModificationKind,
    /// The exact line.
    pub line: String,
}

/// What a removal did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removal {
    /// The outcome, in section 3.4's vocabulary.
    pub outcome: Outcome,
    /// Observations that are not findings: a bridge line this product has no
    /// record of inserting (it is the user's, and is left), and a record
    /// dropped because an earlier removal had already taken its line back.
    pub notes: Vec<String>,
}

/// [`remove`], also taking back every tracked modification the manifest records.
///
/// Spec 002 section 3.13 rule 4: "Removal removes the inserted line and nothing
/// else, and only while the file still begins with it." The record is the
/// authority, and only a record whose path, kind and line match one of `sites`
/// and whose path is well formed is acted on. It locates the line; everything
/// else in the file is the user's and is kept byte for byte, including edits
/// made since the bridge was inserted, and the file itself is never deleted.
/// A modification is withheld, named, and its record kept when it matches no
/// site, when it records no insertion (its digest before equals its digest
/// after), or when ownership cannot be decided: the file is absent, reached
/// through a symbolic link, not text, does not begin with the recorded line,
/// or carries it more than once; or two records name one path.
pub fn remove_with(
    root: &Path,
    clock: &dyn Clock,
    sites: &[BridgeSite],
) -> Result<Removal, ApplyError> {
    let _ = clock;
    // No manifest is a refusal with nothing written, not even the lock file.
    if Manifest::read_bytes(root)?.is_none() {
        return Ok(Removal {
            outcome: Outcome::Refused {
                reasons: vec![NO_MANIFEST.to_string()],
            },
            notes: Vec::new(),
        });
    }
    let _held = crate::manifest::lock(root, WRITER_WAIT)?;
    let Some(mut manifest) = Manifest::read(root)? else {
        return Ok(Removal {
            outcome: Outcome::Refused {
                reasons: vec![NO_MANIFEST.to_string()],
            },
            notes: Vec::new(),
        });
    };

    let mut removed = Vec::new();
    let mut withheld = Vec::new();
    let mut notes = Vec::new();

    let managed: Vec<Entry> = manifest.managed().cloned().collect();
    for entry in managed {
        let target = resolve(root, &entry.path);
        match digest_file(&target).map_err(|source| ApplyError::Io {
            path: entry.path.clone(),
            source,
        })? {
            Some((found, _)) if found == entry.digest => {
                std::fs::remove_file(&target).map_err(|source| ApplyError::Io {
                    path: entry.path.clone(),
                    source,
                })?;
                manifest.remove(&entry.path);
                removed.push(entry.path.clone());
            }
            Some((found, _)) => {
                withheld.push(WithheldWrite {
                    path: entry.path.clone(),
                    adapter: entry.source.identity.clone(),
                    reason: crate::plan::Withholding::Drifted {
                        expected: entry.digest.clone(),
                        found,
                    },
                });
            }
            None => {
                // Already gone. Removing it from the manifest is the whole job;
                // an absent file is not a failure of a removal.
                manifest.remove(&entry.path);
                removed.push(entry.path.clone());
            }
        }
    }

    remove_modifications(
        root,
        &mut manifest,
        sites,
        &mut removed,
        &mut withheld,
        &mut notes,
    )?;

    manifest.write(root)?;

    Ok(Removal {
        outcome: if withheld.is_empty() {
            Outcome::Applied { written: removed }
        } else {
            Outcome::Partial {
                written: removed,
                withheld,
            }
        },
        notes,
    })
}

fn modification_withheld(path: &str, why: String) -> WithheldWrite {
    WithheldWrite {
        path: path.to_string(),
        adapter: "import-bridge".to_string(),
        reason: crate::plan::Withholding::Modification { why },
    }
}

/// Whether a line of text is the recorded line, ignoring only its terminator.
fn is_line(candidate: &str, line: &str) -> bool {
    candidate.trim_end_matches(['\n', '\r']).trim_end() == line
}

fn remove_modifications(
    root: &Path,
    manifest: &mut Manifest,
    sites: &[BridgeSite],
    removed: &mut Vec<String>,
    withheld: &mut Vec<WithheldWrite>,
    notes: &mut Vec<String>,
) -> Result<(), ApplyError> {
    let io = |path: &str| {
        let path = path.to_string();
        move |source| ApplyError::Io { path, source }
    };

    // A site that begins with this product's line while no record names it:
    // not this product's, so a note and nothing else.
    for site in sites {
        if manifest.modifications.iter().any(|m| m.path == site.path) {
            continue;
        }
        if crate::replace::protected(&site.path).is_some()
            || crate::replace::link_on_path(root, &site.path).map_err(io(&site.path))?
        {
            continue;
        }
        let text = match std::fs::read(resolve(root, &site.path)) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(io(&site.path)(e)),
        };
        if text
            .split_inclusive('\n')
            .next()
            .is_some_and(|l| is_line(l, &site.line))
        {
            notes.push(format!(
                "{} begins with `{}`, but the manifest records no bridge there; it is not this product's, and it is left",
                site.path, site.line
            ));
        }
    }

    let records = manifest.modifications.clone();
    let mut paths: Vec<&str> = records.iter().map(|m| m.path.as_str()).collect();
    paths.sort_unstable();
    paths.dedup();
    for path in paths {
        let of_path: Vec<&crate::manifest::Modification> =
            records.iter().filter(|m| m.path == path).collect();
        if of_path.len() > 1 {
            withheld.push(modification_withheld(
                path,
                format!(
                    "the manifest records {} modifications of this path; which one is this product's cannot be decided",
                    of_path.len()
                ),
            ));
            continue;
        }
        let record = of_path[0];
        if let Some(why) = crate::replace::protected(path) {
            withheld.push(modification_withheld(
                path,
                format!("the recorded path is refused: {why}"),
            ));
            continue;
        }
        if !sites
            .iter()
            .any(|s| s.path == record.path && s.kind == record.kind && s.line == record.line)
        {
            withheld.push(modification_withheld(
                path,
                format!(
                    "the record ({:?}, `{}`) is not a modification this product places there; left as it is",
                    record.kind, record.line
                ),
            ));
            continue;
        }
        if record.digest_before.as_deref() == Some(record.digest_after.as_str()) {
            withheld.push(modification_withheld(
                path,
                "the record shows no insertion (the file already began with the line), so it gives no authority to remove it".to_string(),
            ));
            continue;
        }
        if crate::replace::link_on_path(root, path).map_err(io(path))? {
            withheld.push(modification_withheld(
                path,
                "reached through a symbolic link; left as it is".to_string(),
            ));
            continue;
        }
        let target = resolve(root, path);
        let bytes = match std::fs::read(&target) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(io(path)(e)),
        };

        // An earlier removal that rewrote the file and stopped before the
        // manifest: the file holds exactly what taking the line back leaves.
        let taken_back = match (&bytes, &record.digest_before) {
            (Some(b), Some(before)) => digest_bytes(b) == *before,
            (Some(b), None) => b.is_empty(),
            (None, None) => true,
            (None, Some(_)) => false,
        };
        if taken_back {
            manifest.remove_modification(path);
            removed.push(path.to_string());
            notes.push(format!(
                "{path}: the line was already taken back, by an earlier removal that did not finish; the record is dropped"
            ));
            continue;
        }
        let Some(bytes) = bytes else {
            withheld.push(modification_withheld(
                path,
                "recorded, but the file is absent: there is no bridge to take back, and the record is kept".to_string(),
            ));
            continue;
        };
        let Ok(text) = String::from_utf8(bytes) else {
            withheld.push(modification_withheld(
                path,
                "not UTF-8 text; left as it is".to_string(),
            ));
            continue;
        };
        let begins = text == record.line || text.starts_with(&format!("{}\n", record.line));
        if !begins {
            withheld.push(modification_withheld(
                path,
                format!(
                    "no longer begins with the recorded line `{}`; left as it is, and the record is kept",
                    record.line
                ),
            ));
            continue;
        }
        let copies = text
            .split_inclusive('\n')
            .filter(|l| is_line(l, &record.line))
            .count();
        if copies > 1 {
            withheld.push(modification_withheld(
                path,
                format!(
                    "carries the recorded line {copies} times; which one this product inserted cannot be decided, so it is left"
                ),
            ));
            continue;
        }

        // The line and its terminator, and the blank separator only where the
        // insertion added one. Section 3.13 rule 2's insertion writes exactly
        // `line\n` when the file had nothing else, and `line\n\n` before the
        // rest otherwise, so the recorded digest after says which it was.
        let mut rest = text[record.line.len()..].strip_prefix('\n').unwrap_or("");
        let separator_added =
            record.digest_after != digest_bytes(format!("{}\n", record.line).as_bytes());
        if separator_added {
            rest = rest.strip_prefix('\n').unwrap_or(rest);
        }
        // The file is kept even when nothing else is in it: the rule removes
        // the line, not the file.
        crate::replace::write_atomically(root, path, rest.as_bytes()).map_err(io(path))?;
        manifest.remove_modification(path);
        removed.push(path.to_string());
    }
    Ok(())
}
