//! Performing a plan, upgrading, and removing.
//!
//! Spec 002 sections 3.4 and 3.6. Three outcomes, and the middle one is the
//! point: `partial` is a SUCCESS of the upgrade's contract, not a silent one.
//! Every withheld path is named with its reason, and the operator decides what
//! to do per path. Nothing here resolves a conflict by choosing.

use crate::adapter::{Declaration, HarnessProbe};
use crate::claimant::{ForeignClaims, resolve};
use crate::digest::digest_file;
use crate::manifest::{Class, Entry, Manifest, Source, SourceKind};
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
    let computed = plan(root, Some(manifest), declarations, probe, foreign).map_err(|source| {
        ApplyError::Io {
            path: root.display().to_string(),
            source,
        }
    })?;
    perform(root, manifest, &computed, declarations, clock)
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
    if computed.refused() {
        return Ok(Outcome::Refused {
            reasons: computed.refusals.iter().map(|r| r.describe()).collect(),
        });
    }

    let written_at = rfc3339_utc(clock.now_unix());
    let mut written = Vec::new();

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
pub fn remove(root: &Path, clock: &dyn Clock) -> Result<Outcome, ApplyError> {
    let _ = clock;
    let Some(mut manifest) = Manifest::read(root)? else {
        return Ok(Outcome::Refused {
            reasons: vec![NO_MANIFEST.to_string()],
        });
    };

    let mut removed = Vec::new();
    let mut withheld = Vec::new();

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

    manifest.write(root)?;

    Ok(if withheld.is_empty() {
        Outcome::Applied { written: removed }
    } else {
        Outcome::Partial {
            written: removed,
            withheld,
        }
    })
}
