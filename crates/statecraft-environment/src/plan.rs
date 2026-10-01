//! What `env apply` would do, computed without doing any of it.
//!
//! Spec 002 section 3.4: `env plan` prints what `env apply` would do and writes
//! nothing; `env apply` performs it; `env upgrade` re-plans against a newer
//! product or adapter version. All three share this module, because a plan an
//! operator read and a plan an apply executed being different computations is
//! how a preview stops being one.
//!
//! The rule the whole module turns on: **an upgrade never resolves a conflict by
//! choosing.** A write lands only on a path whose on-disk bytes are the bytes
//! the manifest says this product last wrote. Anything else is withheld and
//! named.

use crate::adapter::{
    Declaration, PathCollision, ProviderPath, Readiness, collisions, provider_paths, readiness,
};
use crate::claimant::{Claimant, ForeignClaims, resolve};
use crate::digest::{digest_bytes, digest_file};
use crate::manifest::{Class, Manifest, Role, SourceKind};
use std::path::Path;

/// Why a planned write will not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Withholding {
    /// Another installer claims the path, or a file is already there and this
    /// product did not put it there.
    Foreign {
        /// Who holds it.
        claimant: Claimant,
    },
    /// A managed file whose on-disk bytes are not the bytes last written.
    Drifted {
        /// The digest the manifest records.
        expected: String,
        /// The digest found on disk.
        found: String,
    },
    /// A pointer path that already holds a file.
    ///
    /// Section 3.8: the adapter reports itself degraded and does not append.
    PointerPathOccupied {
        /// Who holds it, as far as this product can tell.
        claimant: Claimant,
    },
    /// The manifest records this path as adopted, and adopted paths are never
    /// rewritten.
    Adopted,
    /// A tracked modification removal will not take back (spec 002 section
    /// 3.13 rule 4), because its ownership cannot be decided.
    Modification {
        /// Why, in one line.
        why: String,
    },
    /// A file an adapter no longer declares, kept because resolving its path
    /// would cross a symbolic link.
    SymbolicLink,
    /// A file an adapter no longer declares, kept because a file this plan
    /// does not rewrite still imports it (spec 030 section 3.3): removing it
    /// would break the import that delivers it.
    StillImported {
        /// The file that imports it.
        by: String,
    },
}

impl Withholding {
    /// Who holds the path, where the withholding is because someone does.
    pub fn claimant(&self) -> Option<&Claimant> {
        match self {
            Withholding::Foreign { claimant } | Withholding::PointerPathOccupied { claimant } => {
                Some(claimant)
            }
            // A modification is withheld because its ownership cannot be
            // decided, so it names no holder.
            Withholding::Drifted { .. }
            | Withholding::Adopted
            | Withholding::Modification { .. }
            | Withholding::SymbolicLink
            | Withholding::StillImported { .. } => None,
        }
    }

    /// A one-line rendering for a report.
    pub fn describe(&self) -> String {
        match self {
            Withholding::Foreign { claimant } => {
                format!("claimed by {}", claimant.describe())
            }
            Withholding::Drifted { expected, found } => {
                format!("drifted: expected {expected}, found {found}")
            }
            Withholding::PointerPathOccupied { claimant } => {
                format!("pointer path already holds a file, {}", claimant.describe())
            }
            Withholding::Adopted => "adopted, never rewritten".to_string(),
            Withholding::Modification { why } => format!("bridge withheld: {why}"),
            Withholding::SymbolicLink => {
                "reached through a symbolic link; left as it is".to_string()
            }
            Withholding::StillImported { by } => format!(
                "no longer declared, kept: {by} still imports it and this plan does not \
                 rewrite {by}"
            ),
        }
    }
}

/// A write the plan intends to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedWrite {
    /// Repository-relative path.
    pub path: String,
    /// The adapter that owns it.
    pub adapter: String,
    /// The bytes to write.
    pub contents: Vec<u8>,
    /// Their digest, computed once here so apply does not recompute it.
    pub digest: String,
    /// True when the path is already managed and the write replaces it.
    pub replaces_existing: bool,
    /// The role the entry it records carries: the recorded one when the path
    /// is already managed, the declaration's for a first write.
    pub role: Role,
}

/// An authored input this product leaves alone (spec 002 section 5,
/// 2026-09-24, provenance item 2).
///
/// Not a withholding: nothing was held back from the operator. The file is
/// the project's to author, so an upgrade never rewrites it, and a newer seed
/// is reported by its digest rather than written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptAuthored {
    /// Repository-relative path.
    pub path: String,
    /// The seed digest the manifest records.
    pub seed: String,
    /// The digest on disk.
    pub found: String,
    /// The digest of the seed this product would write today, when it
    /// differs from the recorded one.
    pub newer_seed: Option<String>,
}

impl KeptAuthored {
    /// `seeded` while the file is its seed, `customized` otherwise.
    pub fn word(&self) -> &'static str {
        if self.found == self.seed {
            "seeded"
        } else {
            "customized"
        }
    }

    /// A one-line rendering for a report.
    pub fn describe(&self) -> String {
        let mut out = format!(
            "{} {}, authored input, never rewritten",
            self.word(),
            self.path
        );
        if let Some(newer) = &self.newer_seed {
            out.push_str(&format!("; a newer seed is available: {newer}"));
        }
        out
    }
}

/// A write that will not happen, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithheldWrite {
    /// Repository-relative path.
    pub path: String,
    /// The adapter that would have written it.
    pub adapter: String,
    /// The reason.
    pub reason: Withholding,
}

/// A precondition failure that stops the whole plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Two adapters declared the same path.
    ///
    /// Refused at plan time, naming both adapters and the path, rather than
    /// discovered when the second write clobbers the first.
    AdapterPathCollision(PathCollision),
    /// An adapter declared a path inside a provider directory (spec 030
    /// section 3.1), which no adapter may write in a target.
    ProviderDirectory(ProviderPath),
    /// The operator named a path for replacement that this product may not
    /// replace (spec 002 section 3.4, and [`crate::replace`]).
    Replacement {
        /// The path as named.
        path: String,
        /// Why it is refused.
        reason: String,
    },
}

impl Refusal {
    /// A one-line rendering for a report.
    pub fn describe(&self) -> String {
        match self {
            Refusal::AdapterPathCollision(c) => format!(
                "adapters {} and {} both declare {}",
                c.adapters.0, c.adapters.1, c.path
            ),
            Refusal::ProviderDirectory(p) => format!(
                "adapter {} declares {}, inside the provider directory {}/, which no adapter \
                 may write in a target",
                p.adapter, p.path, p.directory
            ),
            Refusal::Replacement { path, reason } => {
                format!("replacement of {path} refused: {reason}")
            }
        }
    }
}

/// The refusals a set of declarations earns before any file is read: two
/// adapters declaring one path, and a path inside a provider directory.
pub fn declaration_refusals(declarations: &[Declaration]) -> Vec<Refusal> {
    collisions(declarations)
        .into_iter()
        .map(Refusal::AdapterPathCollision)
        .chain(
            provider_paths(declarations)
                .into_iter()
                .map(Refusal::ProviderDirectory),
        )
        .collect()
}

/// A managed file an adapter wrote and no longer declares, whose bytes are
/// still the ones recorded: removed on apply (spec 030 section 3.3, under
/// spec 002 section 3.6's removal rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retired {
    /// The path.
    pub path: String,
    /// The adapter that wrote it.
    pub adapter: String,
    /// The recorded digest the file must still carry when it is removed.
    pub digest: String,
}

/// What one adapter will do in this plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterOutcome {
    /// The adapter's name.
    pub name: String,
    /// Whether it claims its paths.
    pub readiness: Readiness,
    /// Facts it told us it cannot express in its harness.
    pub unexpressible: Vec<String>,
}

/// The plan.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    /// Writes that will happen.
    pub writes: Vec<PlannedWrite>,
    /// Writes that will not, each named with its reason.
    pub withheld: Vec<WithheldWrite>,
    /// Precondition failures. A non-empty list means nothing is written at all.
    pub refusals: Vec<Refusal>,
    /// Per-adapter outcomes, including the ones that refused.
    pub adapters: Vec<AdapterOutcome>,
    /// Paths the operator named for replacement, each replaceable or already
    /// satisfied. A named path that is neither is a [`Refusal::Replacement`]
    /// instead, and a replaceable one is no longer listed as withheld: this is
    /// what the apply would do given the operator's consent for it.
    pub named: Vec<crate::replace::Named>,
    /// Authored inputs on disk, left alone. Information, never a withholding.
    pub kept: Vec<KeptAuthored>,
    /// Files an adapter wrote and no longer declares, to be removed. One that
    /// drifted is in `withheld` instead, and is left.
    pub retired: Vec<Retired>,
}

impl Plan {
    /// True when a precondition failed and apply must write nothing.
    pub fn refused(&self) -> bool {
        !self.refusals.is_empty()
    }

    /// A human-readable rendering, which is what `env plan` prints.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for r in &self.refusals {
            out.push_str(&format!("refused: {}\n", r.describe()));
        }
        for a in &self.adapters {
            match &a.readiness {
                Readiness::Claiming => {}
                Readiness::Refused { missing } => out.push_str(&format!(
                    "adapter {}: refuses to claim its paths, missing {}\n",
                    a.name,
                    missing.join(", ")
                )),
                Readiness::Degraded { reasons } => out.push_str(&format!(
                    "adapter {}: degraded, {}\n",
                    a.name,
                    reasons.join("; ")
                )),
            }
            for fact in &a.unexpressible {
                out.push_str(&format!("adapter {}: cannot express {fact}\n", a.name));
            }
        }
        for w in &self.writes {
            let verb = if w.replaces_existing {
                "rewrite"
            } else {
                "write"
            };
            out.push_str(&format!("{verb} {} ({})\n", w.path, w.adapter));
        }
        for n in &self.named {
            out.push_str(&format!("{}\n", n.describe()));
        }
        for w in &self.withheld {
            out.push_str(&format!("withhold {}: {}\n", w.path, w.reason.describe()));
        }
        for k in &self.kept {
            out.push_str(&format!("keep {}\n", k.describe()));
        }
        for r in &self.retired {
            out.push_str(&format!(
                "retire {} ({} no longer declares it)\n",
                r.path, r.adapter
            ));
        }
        out
    }
}

/// Compute a plan. Reads the target; writes nothing.
///
/// `manifest` is `None` for a target with no environment installed yet, which
/// is the ordinary first-install case and is not an error.
pub fn plan(
    root: &Path,
    manifest: Option<&Manifest>,
    declarations: &[Declaration],
    probe: &dyn crate::adapter::HarnessProbe,
    foreign: &ForeignClaims,
) -> std::io::Result<Plan> {
    let mut out = Plan::default();

    // Collisions are evaluated across every declaration, including adapters
    // that will go on to refuse: two adapters contesting a path is a
    // configuration defect whether or not either could write today, and hiding
    // it behind an absent harness would surface it later as a mystery.
    out.refusals.extend(declaration_refusals(declarations));

    for declaration in declarations {
        let mut readiness = readiness(declaration, probe);
        let mut degraded_reasons: Vec<String> = Vec::new();

        if readiness.claims_paths() {
            for file in &declaration.files {
                let on_disk = digest_file(&resolve(root, &file.path))?;
                let recorded = manifest.and_then(|m| m.entry(&file.path));
                let transferred = recorded.map(|e| e.transfer.is_some()).unwrap_or(false);

                // Section 3.7.1: a path another installer owns is never written
                // unless the manifest records an explicit transfer for it.
                if !transferred && let Some(claimant) = foreign.claimant_of(&file.path) {
                    out.withheld.push(WithheldWrite {
                        path: file.path.clone(),
                        adapter: declaration.name.clone(),
                        reason: Withholding::Foreign {
                            claimant: claimant.clone(),
                        },
                    });
                    continue;
                }

                match (recorded, on_disk) {
                    // Adopted: depended on, never rewritten.
                    (Some(e), _) if e.class == Class::Adopted => {
                        out.withheld.push(WithheldWrite {
                            path: file.path.clone(),
                            adapter: declaration.name.clone(),
                            reason: Withholding::Adopted,
                        });
                    }
                    // An authored input on disk: the project's to author, so
                    // never rewritten, whatever its bytes. Its seed is compared
                    // only to report whether it was edited and whether this
                    // product would seed it differently today.
                    (Some(e), Some((found, _))) if e.role == Role::AuthoredInput => {
                        let offered = digest_bytes(&file.contents);
                        out.kept.push(KeptAuthored {
                            path: file.path.clone(),
                            seed: e.digest.clone(),
                            found,
                            newer_seed: (offered != e.digest).then_some(offered),
                        });
                    }
                    // Managed and on disk: write only if the bytes are the ones
                    // this product last wrote.
                    (Some(e), Some((found, _))) => {
                        if found == e.digest {
                            out.writes.push(planned(declaration, file, true, e.role));
                        } else {
                            out.withheld.push(WithheldWrite {
                                path: file.path.clone(),
                                adapter: declaration.name.clone(),
                                reason: Withholding::Drifted {
                                    expected: e.digest.clone(),
                                    found,
                                },
                            });
                        }
                    }
                    // Managed and absent: restoring a file we own is a write,
                    // not a conflict. Nobody's edit is at risk.
                    (Some(e), None) => out.writes.push(planned(declaration, file, true, e.role)),
                    // Not manifested, and something is already there. This is
                    // the case section 3.10 requires: classed foreign, withheld,
                    // named, no overwrite. Section 3.21 part 1: the finding
                    // names an owner, not only a path. No manifest records the
                    // file and no other installer claims it, so section 3.2
                    // makes it the user's.
                    (None, Some(_)) => {
                        let claimant =
                            foreign
                                .claimant_of(&file.path)
                                .cloned()
                                .unwrap_or(Claimant::User {
                                    path: file.path.clone(),
                                });
                        if file.pointer {
                            degraded_reasons
                                .push(format!("pointer path {} already holds a file", file.path));
                            out.withheld.push(WithheldWrite {
                                path: file.path.clone(),
                                adapter: declaration.name.clone(),
                                reason: Withholding::PointerPathOccupied { claimant },
                            });
                        } else {
                            out.withheld.push(WithheldWrite {
                                path: file.path.clone(),
                                adapter: declaration.name.clone(),
                                reason: Withholding::Foreign { claimant },
                            });
                        }
                    }
                    // Nothing there, nothing recorded: an ordinary first write.
                    (None, None) => out
                        .writes
                        .push(planned(declaration, file, false, file.role)),
                }
            }
        }

        if !degraded_reasons.is_empty() && matches!(readiness, Readiness::Claiming) {
            readiness = Readiness::Degraded {
                reasons: degraded_reasons,
            };
        }

        out.adapters.push(AdapterOutcome {
            name: declaration.name.clone(),
            readiness,
            unexpressible: declaration.unexpressible.clone(),
        });
    }

    // Spec 030 section 3.3: a managed file a configured adapter wrote and no
    // longer declares leaves with its unchanged bytes, and is reported and
    // left when it drifted. An adopted, transferred or authored entry, and one
    // from an adapter no longer configured, is never retired here. Nor is one
    // whose adapter does not claim its paths: the same apply could not rewrite
    // a pointer that imports the file, so removing it would break the import.
    if let Some(manifest) = manifest {
        for e in manifest.managed() {
            if e.source.kind != SourceKind::Adapter
                || e.transfer.is_some()
                || e.role != Role::Reference
            {
                continue;
            }
            let Some(declaration) = declarations
                .iter()
                .find(|d| d.name == e.source.identity && readiness(d, probe).claims_paths())
            else {
                continue;
            };
            if declaration.paths().any(|p| p == e.path) {
                continue;
            }
            // Retirement follows the effective plan: while a path this adapter
            // declares keeps bytes that import the file, because its write is
            // withheld or not planned, the file still delivers what it held.
            if let Some(by) = still_imported_by(root, declaration, &out.writes, &e.path)? {
                out.withheld.push(WithheldWrite {
                    path: e.path.clone(),
                    adapter: declaration.name.clone(),
                    reason: Withholding::StillImported { by },
                });
                continue;
            }
            match digest_file(&resolve(root, &e.path))? {
                Some((found, _)) if found != e.digest => out.withheld.push(WithheldWrite {
                    path: e.path.clone(),
                    adapter: declaration.name.clone(),
                    reason: Withholding::Drifted {
                        expected: e.digest.clone(),
                        found,
                    },
                }),
                _ => out.retired.push(Retired {
                    path: e.path.clone(),
                    adapter: declaration.name.clone(),
                    digest: e.digest.clone(),
                }),
            }
        }
    }

    out.writes.sort_by(|a, b| a.path.cmp(&b.path));
    out.withheld.sort_by(|a, b| a.path.cmp(&b.path));
    out.kept.sort_by(|a, b| a.path.cmp(&b.path));
    out.retired.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// The declared path whose bytes, after this plan, still import `retiring`.
///
/// A declared path the plan writes ends with the declared bytes; any other
/// keeps what is on disk. An import is a whole line `@<path>`, the form the
/// adapters this product ships write.
fn still_imported_by(
    root: &Path,
    declaration: &Declaration,
    writes: &[PlannedWrite],
    retiring: &str,
) -> std::io::Result<Option<String>> {
    let imports = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .lines()
            .filter_map(|l| l.trim().strip_prefix('@'))
            .any(|p| p.trim() == retiring)
    };
    for file in &declaration.files {
        let after = match writes
            .iter()
            .find(|w| w.path == file.path && w.adapter == declaration.name)
        {
            Some(w) => w.contents.clone(),
            None => match std::fs::read(resolve(root, &file.path)) {
                Ok(bytes) => bytes,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e),
            },
        };
        if imports(&after) {
            return Ok(Some(file.path.clone()));
        }
    }
    Ok(None)
}

/// Compute a plan in which the operator has named paths to replace.
///
/// Spec 002 section 3.4: replacing a drifted managed file requires the operator
/// to say so per path. Every named path is assessed by [`crate::replace::assess`]
/// against the ordinary plan. A replaceable or already-satisfied path moves out
/// of `withheld` into `named`; any other named path refuses the whole plan,
/// so an apply of it writes nothing. With no path named this is [`plan`].
pub fn plan_naming(
    root: &Path,
    manifest: Option<&Manifest>,
    declarations: &[Declaration],
    probe: &dyn crate::adapter::HarnessProbe,
    foreign: &ForeignClaims,
    named: &[String],
) -> std::io::Result<Plan> {
    let mut out = plan(root, manifest, declarations, probe, foreign)?;
    if named.is_empty() {
        return Ok(out);
    }
    for n in crate::replace::assess(root, manifest, &out, declarations, named)? {
        match n {
            crate::replace::Named::Refused { path, reason } => {
                out.refusals.push(Refusal::Replacement { path, reason });
            }
            other => {
                out.withheld.retain(|w| w.path != other.path());
                out.named.push(other);
            }
        }
    }
    Ok(out)
}

fn planned(
    declaration: &Declaration,
    file: &crate::adapter::ManagedFile,
    replaces_existing: bool,
    role: Role,
) -> PlannedWrite {
    PlannedWrite {
        role,
        path: file.path.clone(),
        adapter: declaration.name.clone(),
        digest: digest_bytes(&file.contents),
        contents: file.contents.clone(),
        replaces_existing,
    }
}
