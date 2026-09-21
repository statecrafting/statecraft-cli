//! The diagnostic surface. Read-only, always.
//!
//! Spec 002 section 3.5. `doctor` reports exactly one state per manifest entry,
//! plus pin mismatches, adapters configured versus available, and absent
//! prerequisites. **It never repairs.** A diagnostic that fixes what it measures
//! cannot be trusted to measure it.

use crate::adapter::{Declaration, HarnessProbe, Readiness, readiness};
use crate::claimant::{Claimant, ForeignClaims, ShadowResolver, resolve};
use crate::digest::digest_file;
use crate::manifest::{Class, Manifest};
use std::path::Path;

/// The state of one manifest entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// On disk, digest matches, and it is the copy a session would resolve.
    Present,
    /// On disk, digest differs. Left alone.
    Drifted {
        /// What the manifest records.
        expected: String,
        /// What is on disk.
        found: String,
    },
    /// In the manifest, absent on disk.
    Missing,
    /// On disk and claimed by another installer. Refused for management.
    Foreign {
        /// Who claims it.
        claimant: Claimant,
    },
    /// Digest matches, but this is not the copy a session would resolve.
    ///
    /// The distinction that makes this state necessary: a digest match is not
    /// evidence that the file is in force.
    Shadowed {
        /// The claimant that wins resolution.
        claimant: Claimant,
    },
}

impl State {
    /// Whether this state should make `doctor` exit non-zero.
    ///
    /// `shadowed` counts: a file that is present, correct, and not what runs is
    /// exactly as broken as one that is missing, and harder to notice.
    pub fn is_finding(&self) -> bool {
        !matches!(self, State::Present)
    }

    /// A one-word rendering, the vocabulary of section 3.5's table.
    pub fn word(&self) -> &'static str {
        match self {
            State::Present => "present",
            State::Drifted { .. } => "drifted",
            State::Missing => "missing",
            State::Foreign { .. } => "foreign",
            State::Shadowed { .. } => "shadowed",
        }
    }
}

/// One entry's diagnosis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryReport {
    /// Repository-relative path.
    pub path: String,
    /// Managed or adopted.
    pub class: Class,
    /// Its state.
    pub state: State,
}

/// A finding that is not about one manifest entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// A recorded pin does not match what is present now.
    PinMismatch {
        /// Which pin.
        pin: String,
        /// What the manifest records.
        recorded: String,
        /// What is present.
        found: String,
    },
    /// A configured adapter is not available: its harness or a prerequisite is
    /// absent.
    AdapterUnavailable {
        /// The adapter's name.
        adapter: String,
        /// What is missing.
        missing: Vec<String>,
    },
    /// A path this product's own adapters write, present on disk, absent from
    /// the manifest.
    ///
    /// Section 3.2 calls this a defect, and it is: the three classes are
    /// disjoint and exhaustive by construction, so a path the product wrote and
    /// did not record is the one thing the model does not admit.
    UnmanagedWrite {
        /// The path.
        path: String,
        /// The adapter whose declaration covers it.
        adapter: String,
    },
    /// A tracked modification is no longer in the file it was made to.
    ///
    /// Spec 002 section 3.13: the modification is one line inside a file this
    /// product does not own, so its absence is a finding and never a repair.
    /// The user removed the line, which is their right; the record says so.
    ModificationLost {
        /// The file.
        path: String,
        /// The line that is gone.
        line: String,
    },
    /// A committed declared value names one machine rather than a requirement.
    ///
    /// Spec 002 section 3.12. Refused at write time, and reported here for a
    /// declaration that arrived some other way, such as a hand edit.
    NonPortableDeclaration {
        /// Which map: `requirements` or `overrides`.
        field: String,
        /// The key.
        key: String,
        /// Why it is not portable.
        reason: String,
    },
}

impl Finding {
    /// A one-line rendering for a report.
    pub fn describe(&self) -> String {
        match self {
            Finding::PinMismatch {
                pin,
                recorded,
                found,
            } => format!("pin {pin}: recorded {recorded}, found {found}"),
            Finding::AdapterUnavailable { adapter, missing } => {
                format!(
                    "adapter {adapter} unavailable, missing {}",
                    missing.join(", ")
                )
            }
            Finding::UnmanagedWrite { path, adapter } => {
                format!("unmanaged-write {path}, declared by adapter {adapter}")
            }
            Finding::ModificationLost { path, line } => {
                format!("modification-lost {path}: `{line}` is no longer in the file")
            }
            Finding::NonPortableDeclaration { field, key, reason } => {
                format!("non-portable-declaration {field}.{key}: {reason}")
            }
        }
    }
}

/// Everything `doctor` found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    /// One line per manifest entry.
    pub entries: Vec<EntryReport>,
    /// Everything else.
    pub findings: Vec<Finding>,
}

impl Report {
    /// True when anything is wrong.
    pub fn has_findings(&self) -> bool {
        !self.findings.is_empty() || self.entries.iter().any(|e| e.state.is_finding())
    }

    /// The process exit code this report implies.
    ///
    /// Section 3.10: non-zero on any `drifted`, `missing`, `foreign` or
    /// `unmanaged-write`. `shadowed` joins them for the reason given on
    /// [`State::is_finding`].
    pub fn exit_code(&self) -> i32 {
        i32::from(self.has_findings())
    }

    /// A human-readable rendering.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for e in &self.entries {
            out.push_str(&format!("{:<9} {}", e.state.word(), e.path));
            match &e.state {
                State::Drifted { expected, found } => {
                    out.push_str(&format!(" (expected {expected}, found {found})"));
                }
                State::Foreign { claimant } | State::Shadowed { claimant } => {
                    out.push_str(&format!(" ({})", claimant.describe()));
                }
                _ => {}
            }
            out.push('\n');
        }
        for f in &self.findings {
            out.push_str(&format!("finding   {}\n", f.describe()));
        }
        out
    }
}

/// What is true of the environment right now, for the pins to be compared
/// against. Supplied by the caller: this crate does not shell out to discover
/// its own version.
#[derive(Debug, Clone, Default)]
pub struct Observed {
    /// This product's running version.
    pub product: Option<String>,
    /// The spec-spine version now available.
    pub spec_spine: Option<String>,
}

/// Diagnose a target. Reads only; repairs nothing.
#[allow(clippy::too_many_arguments)]
pub fn doctor(
    root: &Path,
    manifest: &Manifest,
    declarations: &[Declaration],
    probe: &dyn HarnessProbe,
    foreign: &ForeignClaims,
    shadows: &dyn ShadowResolver,
    observed: &Observed,
) -> std::io::Result<Report> {
    let mut report = Report::default();

    for entry in &manifest.entries {
        let on_disk = digest_file(&resolve(root, &entry.path))?;
        let state = match on_disk {
            None => State::Missing,
            Some((found, _)) if found != entry.digest => State::Drifted {
                expected: entry.digest.clone(),
                found,
            },
            Some(_) => {
                // Digest matches. Two things can still be wrong with it.
                let transferred = entry.transfer.is_some();
                match foreign.claimant_of(&entry.path) {
                    Some(claimant) if !transferred => State::Foreign {
                        claimant: claimant.clone(),
                    },
                    _ => match shadows.shadowing_claimant(root, &entry.path) {
                        Some(claimant) => State::Shadowed { claimant },
                        None => State::Present,
                    },
                }
            }
        };
        report.entries.push(EntryReport {
            path: entry.path.clone(),
            class: entry.class,
            state,
        });
    }

    for d in declarations {
        if let Readiness::Refused { missing } = readiness(d, probe) {
            report.findings.push(Finding::AdapterUnavailable {
                adapter: d.name.clone(),
                missing,
            });
        }
        for file in &d.files {
            if manifest.records(&file.path) {
                continue;
            }
            let exists = resolve(root, &file.path).exists();
            if !exists {
                continue;
            }
            // Present, declared by one of our own adapters, not recorded. If
            // another installer claims it, it is that installer's file and the
            // plan already reports it as foreign; anything else is a byte this
            // product is responsible for and did not write down.
            if foreign.claimant_of(&file.path).is_none() {
                report.findings.push(Finding::UnmanagedWrite {
                    path: file.path.clone(),
                    adapter: d.name.clone(),
                });
            }
        }
    }

    // Spec 002 section 3.13: a tracked line that is gone is reported, never
    // reinserted. Reading the file rather than its digest, because the user is
    // free to edit everything else in it and only the line is ours.
    for modification in &manifest.modifications {
        let text = match std::fs::read_to_string(resolve(root, &modification.path)) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        if !text.lines().any(|l| l.trim_end() == modification.line) {
            report.findings.push(Finding::ModificationLost {
                path: modification.path.clone(),
                line: modification.line.clone(),
            });
        }
    }

    for violation in manifest.project.portability_violations() {
        report.findings.push(Finding::NonPortableDeclaration {
            field: violation.field,
            key: violation.key,
            reason: violation.reason,
        });
    }

    if let Some(found) = &observed.product {
        if *found != manifest.pins.product {
            report.findings.push(Finding::PinMismatch {
                pin: "product".into(),
                recorded: manifest.pins.product.clone(),
                found: found.clone(),
            });
        }
    }
    if let Some(found) = &observed.spec_spine {
        if *found != manifest.pins.spec_spine {
            report.findings.push(Finding::PinMismatch {
                pin: "spec-spine".into(),
                recorded: manifest.pins.spec_spine.clone(),
                found: found.clone(),
            });
        }
    }
    for d in declarations {
        if let Some(recorded) = manifest.pins.adapters.get(&d.name) {
            if *recorded != d.version {
                report.findings.push(Finding::PinMismatch {
                    pin: format!("adapter {}", d.name),
                    recorded: recorded.clone(),
                    found: d.version.clone(),
                });
            }
        }
    }

    report.entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(report)
}
