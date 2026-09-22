//! The required harness identity, and what a managed session does when the
//! required one and the resolved one disagree.
//!
//! Spec 002 section 3.25, settled by the owner on 2026-09-21.
//!
//! # Two records, and they are not the same record
//!
//! The **required** identity is a committed project requirement. It lives in
//! the manifest's project declaration, which is the governed contract for
//! exactly this ("what this project requires, keyed by tool or capability"),
//! it travels with the repository, and it is reviewed like any other committed
//! change. The **resolved** identity is recorded per managed session by
//! [`crate::resolved`], and says which revision actually answered.
//!
//! Keeping them apart is what makes disagreement visible. One record rewritten
//! as it is read cannot disagree with anything, so nothing here writes: every
//! function in this module that inspects takes `&Manifest`, and the only
//! function that changes a requirement is [`apply_upgrade`], which is an
//! explicit reviewed project change and takes `&mut Manifest` to say so.
//!
//! # The full digest is the integrity proof
//!
//! The committed value is the **full** digest over the revision's files, and
//! every comparison in this module is against that. `h-` plus twelve hex
//! characters is what a plan, a verdict and a log print, and it is never on its
//! own what an equality check is performed against: a truncation two revisions
//! could share is not a proof, whatever the odds are.
//!
//! # What refuses, and what stays possible
//!
//! Managed execution refuses on **missing**, **corrupt** and **mismatched**,
//! and on an installed tree this product could not read at all. Four things
//! stay possible under every one of those refusals, because a refusal that
//! prevents diagnosis is worse than the state it refuses: inspection
//! ([`inspect`]), diagnosis ([`Standing::describe`]), planning
//! ([`plan_upgrade`]) and an explicit upgrade ([`apply_upgrade`]).
//!
//! # Existing manifests
//!
//! A manifest written before this section existed carries no harness
//! requirement. That is [`Standing::Unrequired`], it is **not** qualified for
//! managed execution, and it is not repaired by a read. The migration is an
//! operator act: `plan_upgrade` against the revision they mean, reviewed and
//! committed like any other change to the declaration.

use crate::harness::{self, Revision};
use crate::home::Layout;
use serde::{Deserialize, Serialize};
use statecraft_environment::manifest::Manifest;

/// The key the committed harness requirement lives under.
///
/// One key in the manifest's `project.requirements`, which is the governed
/// contract for a committed requirement and already binds every configuration
/// layer: no personal default and no run choice may contradict one
/// (section 3.16, and [`crate::authority`] implements it). A second bespoke
/// field beside it would be a requirement that the requirement machinery does
/// not know about.
pub const REQUIREMENT_KEY: &str = "harness";

/// The committed requirement, verbatim, or `None` when the project declares
/// none.
///
/// A read. It does not consult the home, it does not pick a revision, and it
/// does not populate the key it failed to find.
pub fn required_of(manifest: &Manifest) -> Option<&str> {
    manifest
        .project
        .requirements
        .get(REQUIREMENT_KEY)
        .map(String::as_str)
        .filter(|v| !v.is_empty())
}

/// A committed requirement this build cannot use as an integrity proof.
///
/// A requirement is a full digest. A short display identifier committed in its
/// place would make every comparison a truncation comparison, which section
/// 3.25 refuses by name, so it is rejected rather than accepted and compared
/// leniently.
pub fn malformed_requirement(value: &str) -> Option<String> {
    let full = statecraft_environment::digest::digest_bytes(b"");
    if value.starts_with("h-") {
        return Some(format!(
            "`{value}` is a display identifier; the committed requirement carries the full \
             digest, because an equality check against a truncation is not an integrity proof"
        ));
    }
    if value.len() != full.len() || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(format!(
            "`{value}` is not a {}-character hexadecimal digest",
            full.len()
        ));
    }
    None
}

/// What comparing the committed requirement against the home established.
///
/// Exactly one of these is true of a project at a moment. Only [`Exact`]
/// permits managed execution, and the other variants are not ranked: a
/// mismatch is not "closer to qualified" than a missing revision, and none of
/// them is repaired by choosing something else.
///
/// [`Exact`]: Standing::Exact
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "standing")]
pub enum Standing {
    /// The project declares no harness requirement.
    ///
    /// Every manifest written before section 3.25 is in this state. It is not
    /// qualified, and a read does not leave it qualified by filling the key
    /// in: the evidence that the requirement was never committed is the thing
    /// a repair would destroy.
    Unrequired,
    /// A requirement is committed and this build cannot read it as a digest.
    MalformedRequirement {
        /// The committed value, verbatim.
        required: String,
        /// Why it cannot be used as an integrity proof.
        reason: String,
    },
    /// No revision is installed under the required identity.
    Missing {
        /// The required full digest.
        required: String,
        /// Every identity that **is** installed, for a report. Named so an
        /// operator can see them; never selected from (section 3.25).
        installed: Vec<String>,
    },
    /// A revision is installed under the required identity and its files no
    /// longer digest to it.
    Corrupt {
        /// The required full digest.
        required: String,
        /// What the installed files actually digest to.
        computed: String,
        /// Each file whose bytes differ, are missing, or are unexpected.
        findings: Vec<Finding>,
    },
    /// The installed tree under the required identity could not be read.
    ///
    /// Distinct from corrupt on purpose: corrupt is an answer, and this is the
    /// absence of one. Both refuse, and only one of them says what is wrong.
    Unreadable {
        /// The required full digest.
        required: String,
        /// What the read reported.
        reason: String,
    },
    /// A revision resolved for this session and it is not the required one.
    Mismatched {
        /// The required full digest.
        required: String,
        /// The full digest that actually answered.
        resolved: String,
    },
    /// The required revision is installed and intact, and nothing disagrees.
    Exact {
        /// The required full digest.
        required: String,
        /// The full digest that resolved for this session, where one has.
        ///
        /// `None` is the state an inspection or a `doctor` run is in: the
        /// question has not been asked. Nothing **disagrees** in that state,
        /// which is why it is not a finding, and nothing has **resolved**,
        /// which is why it does not permit managed execution. Those are two
        /// different questions and this field is what keeps them apart.
        resolved: Option<String>,
    },
}

/// One file's disagreement with the revision it is installed under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Finding {
    /// A file the revision names is not on disk.
    Absent {
        /// Path relative to the revision directory.
        rel_path: String,
    },
    /// A file on disk the revision does not name.
    Unexpected {
        /// Path relative to the revision directory.
        rel_path: String,
    },
    /// A file whose bytes are not the ones the revision names.
    Changed {
        /// Path relative to the revision directory.
        rel_path: String,
        /// What the revision says.
        expected: String,
        /// What is there.
        found: String,
    },
}

impl Finding {
    /// A one-line rendering.
    pub fn describe(&self) -> String {
        match self {
            Finding::Absent { rel_path } => format!("absent {rel_path}"),
            Finding::Unexpected { rel_path } => format!("unexpected {rel_path}"),
            Finding::Changed { rel_path, .. } => format!("changed {rel_path}"),
        }
    }
}

impl Standing {
    /// The required identity, where one is committed.
    pub fn required(&self) -> Option<&str> {
        match self {
            Standing::Unrequired => None,
            Standing::MalformedRequirement { required, .. }
            | Standing::Missing { required, .. }
            | Standing::Corrupt { required, .. }
            | Standing::Unreadable { required, .. }
            | Standing::Mismatched { required, .. }
            | Standing::Exact { required, .. } => Some(required),
        }
    }

    /// A one-word rendering.
    pub fn word(&self) -> &'static str {
        match self {
            Standing::Unrequired => "unrequired",
            Standing::MalformedRequirement { .. } => "malformed-requirement",
            Standing::Missing { .. } => "missing",
            Standing::Corrupt { .. } => "corrupt",
            Standing::Unreadable { .. } => "unreadable",
            Standing::Mismatched { .. } => "mismatched",
            Standing::Exact { .. } => "exact",
        }
    }

    /// The only state that permits managed execution.
    ///
    /// `Exact` with nothing resolved does **not**: a requirement that nothing
    /// has been resolved against is a healthy project, not a qualified
    /// session, and treating an unasked question as an affirmative answer is
    /// the same defect as treating a configured deny entry as an enforced one
    /// (section 3.28).
    ///
    /// Written as a match with every arm spelled out rather than as a negation,
    /// so that a state added later has to decide rather than inherit `true`.
    pub fn permits_managed_execution(&self) -> bool {
        match self {
            Standing::Exact { resolved, .. } => resolved.is_some(),
            Standing::Unrequired
            | Standing::MalformedRequirement { .. }
            | Standing::Missing { .. }
            | Standing::Corrupt { .. }
            | Standing::Unreadable { .. }
            | Standing::Mismatched { .. } => false,
        }
    }

    /// Why a managed run may not start, or `None` when it may.
    ///
    /// Section 3.25: a session that would be managed under a required identity
    /// refuses when the required content is missing, corrupt or mismatched. A
    /// requirement this build cannot read as a digest, and an installed tree
    /// that cannot be read, are the same condition one step earlier: the
    /// required content cannot be established, and they refuse for that
    /// reason.
    ///
    /// Two states do not refuse a run, and neither is qualified. `Unrequired`
    /// is a project that commits no identity, so no session of it is managed
    /// **under** one; it runs and is recorded as unrequired. `Exact` with
    /// nothing resolved is the state before a session has resolved, and a run
    /// is how one does. [`Standing::refusal`] answers the stricter question of
    /// whether the managed-execution **claim** holds, which neither state
    /// meets.
    pub fn refuses_a_run(&self) -> Option<String> {
        match self {
            Standing::Unrequired | Standing::Exact { .. } => None,
            other => other.refusal(),
        }
    }

    /// Why the required identity and the home or the session **disagree**, or
    /// `None` when nothing does.
    ///
    /// This is the diagnostic question, and it is not the execution question:
    /// a project whose requirement is committed, installed and intact
    /// disagrees with nothing, whether or not a session has resolved against
    /// it yet. [`Standing::refusal`] is the other one.
    pub fn disagreement(&self) -> Option<String> {
        match self {
            Standing::Exact { .. } => None,
            other => other.refusal(),
        }
    }

    /// Why managed execution is refused, or `None` when it is not.
    ///
    /// This product's vocabulary: a precondition was not met and nothing was
    /// done. Each reason names what is required, what was found, and the act
    /// that would change it, because the four things that stay possible under
    /// the refusal are only useful to somebody who knows which one to reach
    /// for.
    pub fn refusal(&self) -> Option<String> {
        let short = |d: &str| harness::display_id(d);
        Some(match self {
            Standing::Exact {
                resolved: Some(_), ..
            } => return None,
            Standing::Exact {
                required,
                resolved: None,
            } => format!(
                "harness {} is required, installed and intact, and no revision has resolved \
                 for this session; a requirement nothing has been resolved against is a \
                 healthy project and not a qualified session",
                short(required)
            ),
            Standing::Unrequired => format!(
                "this project commits no `{REQUIREMENT_KEY}` requirement, so there is nothing a \
                 resolved revision could be checked against; commit one with an explicit upgrade \
                 rather than running against whichever revision this machine happens to hold"
            ),
            Standing::MalformedRequirement { reason, .. } => format!(
                "the committed `{REQUIREMENT_KEY}` requirement cannot be used as an integrity \
                 proof: {reason}"
            ),
            Standing::Missing {
                required,
                installed,
            } => format!(
                "the required harness revision {} is not installed under this home; {} \
                 {} installed, and none of them is a substitute for the one this project \
                 requires",
                short(required),
                if installed.is_empty() {
                    "no revision is".to_string()
                } else {
                    installed.join(", ")
                },
                if installed.len() == 1 { "is" } else { "are" }
            ),
            Standing::Corrupt {
                required,
                computed,
                findings,
            } => format!(
                "the tree installed under {} digests to {} instead: {}",
                short(required),
                short(computed),
                findings
                    .iter()
                    .map(Finding::describe)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Standing::Unreadable { required, reason } => format!(
                "the tree installed under {} could not be read, so nothing about its integrity \
                 is established: {reason}",
                short(required)
            ),
            Standing::Mismatched { required, resolved } => format!(
                "this project requires harness {} and {} resolved; a resolved revision that is \
                 not the required one is a mismatch to report, not a substitute to accept",
                short(required),
                short(resolved)
            ),
        })
    }

    /// A one-line rendering for a report.
    pub fn describe(&self) -> String {
        match self.refusal() {
            None => format!(
                "exact: harness {} is required, installed, intact and resolved",
                harness::display_id(self.required().unwrap_or_default())
            ),
            Some(reason) => format!("{}: {reason}", self.word()),
        }
    }
}

/// Compare the committed requirement with the home, and with what resolved.
///
/// `resolved` is the full digest of the revision that actually answered for
/// this session, where one has. `None` means the question has not been asked
/// yet, which is the state an inspection or a `doctor` run is in: the required
/// revision's presence and integrity are still decidable, and mismatch is not.
///
/// A read. It never installs, never selects and never writes.
pub fn evaluate(layout: &Layout, manifest: &Manifest, resolved: Option<&str>) -> Standing {
    let Some(required) = required_of(manifest) else {
        return Standing::Unrequired;
    };
    if let Some(reason) = malformed_requirement(required) {
        return Standing::MalformedRequirement {
            required: required.to_string(),
            reason,
        };
    }
    let required = required.to_string();
    let id = harness::display_id(&required);

    let files = match harness::read_installed(layout, &id) {
        Err(e) => {
            return Standing::Unreadable {
                required,
                reason: e.to_string(),
            };
        }
        Ok(None) => {
            return Standing::Missing {
                required,
                installed: harness::installed_revisions(layout),
            };
        }
        Ok(Some(files)) => files,
    };

    let computed = harness::revision_of(&files);
    if computed.digest != required {
        return Standing::Corrupt {
            findings: findings_against(&required, &computed),
            required,
            computed: computed.digest,
        };
    }

    match resolved {
        Some(r) if r != required => Standing::Mismatched {
            required,
            resolved: r.to_string(),
        },
        other => Standing::Exact {
            required,
            resolved: other.map(str::to_string),
        },
    }
}

/// What differs between the shipped revision bearing an identity and the tree
/// installed under it.
///
/// Only meaningful when the required identity is one this build ships, which
/// is the common case and the one an operator most needs named. When it is
/// not, the digest comparison already refused and this adds no finding rather
/// than inventing one.
fn findings_against(required: &str, computed: &Revision) -> Vec<Finding> {
    let shipped = harness::revision_of(&harness::shipped());
    if shipped.digest != required {
        return Vec::new();
    }
    let mut out = Vec::new();
    for expected in &shipped.files {
        match computed
            .files
            .iter()
            .find(|f| f.rel_path == expected.rel_path)
        {
            None => out.push(Finding::Absent {
                rel_path: expected.rel_path.clone(),
            }),
            Some(found) if found.digest != expected.digest => out.push(Finding::Changed {
                rel_path: expected.rel_path.clone(),
                expected: expected.digest.clone(),
                found: found.digest.clone(),
            }),
            Some(_) => {}
        }
    }
    for found in &computed.files {
        if !shipped.files.iter().any(|f| f.rel_path == found.rel_path) {
            out.push(Finding::Unexpected {
                rel_path: found.rel_path.clone(),
            });
        }
    }
    out.sort_by_key(|f| f.describe());
    out
}

/// The finding `doctor` reports for a standing that refuses, if any.
///
/// Section 3.25's second surviving capability under a refusal. `doctor` cannot
/// perform this comparison itself, because the crate that owns it cannot see a
/// product home; this is the answer, shaped so that the diagnostic surface
/// reports it beside everything else it found rather than in a second report
/// an operator has to know to ask for.
pub fn doctor_finding(standing: &Standing) -> Option<statecraft_environment::doctor::Finding> {
    standing.disagreement().map(|reason| {
        statecraft_environment::doctor::Finding::HarnessRequirement {
            standing: standing.word().to_string(),
            reason,
        }
    })
}

/// One installed revision, as an inspection sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledRevision {
    /// The identity its directory is named for: display form.
    pub id: String,
    /// What its files actually digest to, when they could be read.
    pub digest: Option<String>,
    /// True when the recomputed digest's display form is the directory's name.
    pub intact: bool,
    /// Why the tree could not be read, when it could not.
    pub unreadable: Option<String>,
}

/// What is required, what is installed, and what each digests to.
///
/// Section 3.25's first surviving capability under a refusal. Available in
/// every standing, including the refusing ones, which is the point: an
/// operator diagnosing a mismatch needs exactly this table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspection {
    /// The committed requirement, verbatim, where one is committed.
    pub required: Option<String>,
    /// Its display form, for a report.
    pub required_display: Option<String>,
    /// Every revision installed under the home.
    pub installed: Vec<InstalledRevision>,
    /// The revision this build ships, which is what an upgrade would offer.
    pub shipped: String,
    /// The standing, with no resolved identity asserted.
    pub standing: Standing,
}

/// Inspect the requirement and the home. A read.
pub fn inspect(layout: &Layout, manifest: &Manifest) -> Inspection {
    let required = required_of(manifest).map(str::to_string);
    let installed = harness::installed_revisions(layout)
        .into_iter()
        .map(|id| match harness::read_installed(layout, &id) {
            Err(e) => InstalledRevision {
                id,
                digest: None,
                intact: false,
                unreadable: Some(e.to_string()),
            },
            Ok(None) => InstalledRevision {
                id,
                digest: None,
                intact: false,
                unreadable: Some("the directory disappeared between listing and reading".into()),
            },
            Ok(Some(files)) => {
                let computed = harness::revision_of(&files);
                InstalledRevision {
                    intact: computed.id == id,
                    digest: Some(computed.digest),
                    id,
                    unreadable: None,
                }
            }
        })
        .collect();
    Inspection {
        required_display: required.as_deref().map(harness::display_id),
        required,
        installed,
        shipped: harness::revision_of(&harness::shipped()).digest,
        standing: evaluate(layout, manifest, None),
    }
}

/// An explicit, reviewed change to the committed requirement.
///
/// Section 3.25: changing the required identity is a committed change to the
/// repository, never a side effect of installing, of running, or of a newer
/// revision appearing under the home. This type is the proposal; nothing is
/// changed until [`apply_upgrade`] is called with it, and nothing is committed
/// until the caller writes the manifest and the operator commits it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Upgrade {
    /// The requirement being replaced, where there is one.
    pub from: Option<String>,
    /// The requirement being committed, full digest.
    pub to: String,
    /// The display form of `to`.
    pub to_display: String,
    /// Whether that revision is installed under this home.
    ///
    /// `false` is allowed: a requirement is a statement about what the project
    /// requires, and a repository may commit one before a given machine holds
    /// it. It is reported because until the revision is installed here, this
    /// machine's standing is [`Standing::Missing`] and managed execution
    /// refuses.
    pub installed_here: bool,
    /// Whether the settings modification's content changes across this upgrade.
    ///
    /// Section 3.25's last rule. Where it changes, section 3.24's consent is
    /// asked again for the new bytes; where it is byte-identical there is
    /// nothing new to consent to and nothing is asked.
    pub renews_settings_consent: bool,
}

impl Upgrade {
    /// A one-line rendering.
    pub fn describe(&self) -> String {
        format!(
            "require harness {} (was {}); {} here; settings consent {}",
            self.to_display,
            self.from
                .as_deref()
                .map(harness::display_id)
                .unwrap_or_else(|| "unrequired".into()),
            if self.installed_here {
                "installed"
            } else {
                "not installed"
            },
            if self.renews_settings_consent {
                "is asked again"
            } else {
                "is unchanged, so nothing is asked"
            }
        )
    }
}

/// Why an upgrade will not be proposed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum UpgradeRefusal {
    /// The target is not a full digest.
    #[error("{reason}")]
    Malformed {
        /// Why.
        reason: String,
    },
    /// The target is installed here and its files do not digest to it.
    ///
    /// Refused rather than committed: pointing a project's requirement at a
    /// directory whose bytes already disagree with its name commits a
    /// requirement that is corrupt on the machine proposing it.
    #[error(
        "the tree installed under {id} digests to something else, so it is not the revision {id} names"
    )]
    TargetCorrupt {
        /// The display identity.
        id: String,
    },
}

/// Propose an upgrade. A read: nothing is changed by planning one.
pub fn plan_upgrade(
    layout: &Layout,
    manifest: &Manifest,
    to: &str,
) -> Result<Upgrade, UpgradeRefusal> {
    if let Some(reason) = malformed_requirement(to) {
        return Err(UpgradeRefusal::Malformed { reason });
    }
    let id = harness::display_id(to);
    let installed_here = match harness::read_installed(layout, &id) {
        Ok(Some(files)) => {
            if harness::revision_of(&files).digest != to {
                return Err(UpgradeRefusal::TargetCorrupt { id });
            }
            true
        }
        Ok(None) => false,
        // An unreadable tree is not an intact one, and it is not a reason to
        // claim the revision is absent either. Reported as not installed here,
        // which is the conservative half: the upgrade may still be committed,
        // and this machine's standing stays refusing until the tree reads.
        Err(_) => false,
    };
    let from = required_of(manifest).map(str::to_string);
    Ok(Upgrade {
        renews_settings_consent: settings_token_for(layout, from.as_deref())
            != settings_token_for(layout, Some(to)),
        from,
        to_display: id,
        to: to.to_string(),
        installed_here,
    })
}

/// The consent token the settings modification would carry for a requirement.
///
/// `None` for an unrequired project, which has no revision to register hooks
/// from and therefore no content to have consented to.
fn settings_token_for(layout: &Layout, digest: Option<&str>) -> Option<String> {
    let digest = digest?;
    let id = harness::display_id(digest);
    let root = layout.harness_revision_dir(&id);
    Some(crate::settings::managed(&id, &root).token())
}

/// Record the upgrade in the manifest, in memory.
///
/// The caller writes the manifest and the operator commits it: that write and
/// that commit are the reviewed project change section 3.25 requires, and this
/// function is deliberately not the place where either happens.
pub fn apply_upgrade(manifest: &mut Manifest, upgrade: &Upgrade) {
    manifest
        .project
        .requirements
        .insert(REQUIREMENT_KEY.to_string(), upgrade.to.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    use statecraft_environment::manifest::Pins;

    fn manifest() -> Manifest {
        Manifest::new(Pins {
            product: "0.1.0".into(),
            spec_spine: "0.20.0".into(),
            adapters: Default::default(),
        })
    }

    fn with_requirement(value: &str) -> Manifest {
        let mut m = manifest();
        m.project
            .requirements
            .insert(REQUIREMENT_KEY.to_string(), value.to_string());
        m
    }

    /// A home with the shipped revision installed, and its full digest.
    fn home() -> (tempfile::TempDir, Layout, String) {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        let installed = harness::install(&layout, &harness::shipped()).unwrap();
        let digest = installed.revision.digest.clone();
        (dir, layout, digest)
    }

    #[test]
    fn a_revision_carries_a_full_digest_and_a_display_identifier_derived_from_it() {
        let r = harness::revision_of(&harness::shipped());
        assert!(r.has_full_digest());
        assert_eq!(r.digest.len(), 64);
        assert_eq!(r.id, harness::display_id(&r.digest));
        assert_eq!(r.id.len(), 14);
    }

    #[test]
    fn a_missing_required_identity_refuses_and_is_not_ranked_against_what_is_installed() {
        let (_d, layout, digest) = home();
        // A requirement for a revision this home does not hold, while it does
        // hold one.
        let other = "0".repeat(64);
        let standing = evaluate(&layout, &with_requirement(&other), None);
        assert_eq!(standing.word(), "missing");
        assert!(!standing.permits_managed_execution());
        let Standing::Missing { installed, .. } = &standing else {
            panic!("{standing:?}");
        };
        assert_eq!(installed, &[harness::display_id(&digest)]);
        assert!(standing.refusal().unwrap().contains("not installed"));
    }

    #[test]
    fn missing_installed_content_is_refused_rather_than_substituted() {
        // Section 3.25: it never silently selects the latest installed
        // revision. Two revisions are installed and the requirement names
        // neither; the answer is `missing`, not the newer of the two.
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        let first = harness::install(&layout, &harness::shipped()).unwrap();
        let mut newer = harness::shipped();
        newer[0].contents.push_str("\nlater\n");
        let second = harness::install(&layout, &newer).unwrap();
        assert_ne!(first.revision.digest, second.revision.digest);

        let standing = evaluate(&layout, &with_requirement(&"a".repeat(64)), None);
        assert_eq!(standing.word(), "missing");
        let reason = standing.refusal().unwrap();
        assert!(
            reason.contains("substitute"),
            "the refusal does not say why the installed ones are not used: {reason}"
        );
    }

    #[test]
    fn corrupted_installed_content_is_reported_file_by_file() {
        let (_d, layout, digest) = home();
        let id = harness::display_id(&digest);
        let victim = layout
            .harness_revision_dir(&id)
            .join("rules/statecraft-governed-work.md");
        std::fs::write(&victim, "tampered\n").unwrap();

        let standing = evaluate(&layout, &with_requirement(&digest), None);
        assert_eq!(standing.word(), "corrupt");
        assert!(!standing.permits_managed_execution());
        let Standing::Corrupt {
            computed, findings, ..
        } = &standing
        else {
            panic!("{standing:?}");
        };
        assert_ne!(computed, &digest);
        assert!(
            findings
                .iter()
                .any(|f| matches!(f, Finding::Changed { rel_path, .. }
                        if rel_path == "rules/statecraft-governed-work.md")),
            "{findings:?}"
        );
    }

    #[test]
    fn a_file_added_into_a_revision_directory_is_corruption_too() {
        let (_d, layout, digest) = home();
        let id = harness::display_id(&digest);
        std::fs::write(
            layout.harness_revision_dir(&id).join("rules/extra.md"),
            "not ours\n",
        )
        .unwrap();
        let standing = evaluate(&layout, &with_requirement(&digest), None);
        let Standing::Corrupt { findings, .. } = &standing else {
            panic!("{standing:?}");
        };
        assert!(
            findings.iter().any(
                |f| matches!(f, Finding::Unexpected { rel_path } if rel_path == "rules/extra.md")
            ),
            "{findings:?}"
        );
    }

    #[test]
    fn a_required_resolved_mismatch_refuses_even_though_both_are_installed() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path());
        let required = harness::install(&layout, &harness::shipped()).unwrap();
        let mut other = harness::shipped();
        other[0].contents.push_str("\nother\n");
        let resolved = harness::install(&layout, &other).unwrap();

        let standing = evaluate(
            &layout,
            &with_requirement(&required.revision.digest),
            Some(&resolved.revision.digest),
        );
        assert_eq!(standing.word(), "mismatched");
        assert!(!standing.permits_managed_execution());
        assert!(standing.refusal().unwrap().contains("mismatch"));
    }

    #[test]
    fn a_successful_exact_resolution_is_the_only_state_that_permits_execution() {
        let (_d, layout, digest) = home();
        let standing = evaluate(&layout, &with_requirement(&digest), Some(&digest));
        assert_eq!(
            standing,
            Standing::Exact {
                required: digest.clone(),
                resolved: Some(digest.clone()),
            }
        );
        assert!(standing.permits_managed_execution());
        assert_eq!(standing.refusal(), None);
        assert_eq!(standing.disagreement(), None);

        // The same project, read without asking what resolved. Nothing
        // disagrees, so `doctor` reports nothing; nothing resolved, so nothing
        // may run on the strength of it.
        let unasked = evaluate(&layout, &with_requirement(&digest), None);
        assert_eq!(unasked.word(), "exact");
        assert_eq!(unasked.disagreement(), None);
        assert!(doctor_finding(&unasked).is_none());
        assert!(
            !unasked.permits_managed_execution(),
            "a requirement nothing has been resolved against qualified a session"
        );
        assert!(
            unasked
                .refusal()
                .unwrap()
                .contains("no revision has resolved")
        );
    }

    #[test]
    fn an_older_manifest_with_no_requirement_is_not_defaulted_into_qualified_execution() {
        let (_d, layout, digest) = home();
        // The shipped revision is installed and is the only one. A build that
        // "helpfully" resolved it would qualify this project; nothing does.
        let m = manifest();
        assert_eq!(required_of(&m), None);
        let standing = evaluate(&layout, &m, Some(&digest));
        assert_eq!(standing, Standing::Unrequired);
        assert!(!standing.permits_managed_execution());
        assert!(standing.refusal().unwrap().contains("commits no"));
    }

    #[test]
    fn a_read_never_populates_the_requirement_it_failed_to_find() {
        let (_d, layout, _digest) = home();
        let project = tempfile::tempdir().unwrap();
        manifest().write(project.path()).unwrap();
        let path = statecraft_environment::claimant::resolve(
            project.path(),
            statecraft_environment::manifest::MANIFEST_PATH,
        );
        let before = std::fs::read(&path).unwrap();

        let read = Manifest::read(project.path()).unwrap().unwrap();
        let _ = evaluate(&layout, &read, None);
        let _ = inspect(&layout, &read);
        let _ = plan_upgrade(&layout, &read, &"b".repeat(64)).unwrap();

        assert_eq!(
            before,
            std::fs::read(&path).unwrap(),
            "a read rewrote the manifest, which destroys the evidence that the \
             precondition was unmet"
        );
    }

    #[test]
    fn a_short_identifier_committed_as_the_requirement_is_refused() {
        let (_d, layout, digest) = home();
        let standing = evaluate(
            &layout,
            &with_requirement(&harness::display_id(&digest)),
            None,
        );
        assert_eq!(standing.word(), "malformed-requirement");
        assert!(!standing.permits_managed_execution());
        assert!(standing.refusal().unwrap().contains("integrity proof"));
    }

    #[test]
    fn inspection_and_repair_remain_available_under_every_refusal() {
        let (_d, layout, digest) = home();
        let id = harness::display_id(&digest);
        std::fs::write(
            layout.harness_revision_dir(&id).join("rules/extra.md"),
            "x\n",
        )
        .unwrap();

        for manifest in [
            manifest(),
            with_requirement(&"c".repeat(64)),
            with_requirement(&digest),
            with_requirement("not-a-digest"),
        ] {
            let standing = evaluate(&layout, &manifest, None);
            assert!(
                !standing.permits_managed_execution(),
                "{standing:?} qualified"
            );

            // 1. inspection: what is required, what is installed, what each
            //    digests to.
            let inspection = inspect(&layout, &manifest);
            assert_eq!(inspection.installed.len(), 1);
            assert!(inspection.installed[0].digest.is_some());
            assert!(!inspection.installed[0].intact);
            assert_eq!(inspection.shipped, digest);

            // 2. diagnosis: the standing says what disagrees.
            assert!(!standing.describe().is_empty());
            assert!(standing.refusal().is_some());

            // 3. planning, and 4. an explicit upgrade.
            let plan = plan_upgrade(&layout, &manifest, &digest);
            let refusal = plan.as_ref().err().map(|e| e.to_string());
            assert!(
                refusal.is_some(),
                "the target is installed and corrupt, so the upgrade is refused rather \
                 than committed: {plan:?}"
            );
        }
    }

    #[test]
    fn an_explicit_upgrade_commits_the_requirement_and_nothing_else_does() {
        let (_d, layout, digest) = home();
        let mut m = manifest();
        let upgrade = plan_upgrade(&layout, &m, &digest).unwrap();
        assert_eq!(upgrade.from, None);
        assert!(upgrade.installed_here);
        // Planning changed nothing.
        assert_eq!(required_of(&m), None);

        apply_upgrade(&mut m, &upgrade);
        assert_eq!(required_of(&m), Some(digest.as_str()));
        assert_eq!(
            evaluate(&layout, &m, Some(&digest)),
            Standing::Exact {
                required: digest.clone(),
                resolved: Some(digest),
            }
        );
    }

    #[test]
    fn an_upgrade_to_new_content_renews_settings_consent() {
        let (_d, layout, digest) = home();
        let mut next = harness::shipped();
        next[0].contents.push_str("\nnew\n");
        let other = harness::install(&layout, &next).unwrap().revision.digest;

        let mut m = manifest();
        let first = plan_upgrade(&layout, &m, &digest).unwrap();
        apply_upgrade(&mut m, &first);
        let upgrade = plan_upgrade(&layout, &m, &other).unwrap();
        assert_eq!(upgrade.from.as_deref(), Some(digest.as_str()));
        assert!(
            upgrade.renews_settings_consent,
            "a new required identity produces different modification content, so \
             section 3.24's consent is asked again"
        );
    }

    #[test]
    fn an_upgrade_to_the_same_identity_asks_for_nothing() {
        let (_d, layout, digest) = home();
        let mut m = manifest();
        let first = plan_upgrade(&layout, &m, &digest).unwrap();
        apply_upgrade(&mut m, &first);
        let again = plan_upgrade(&layout, &m, &digest).unwrap();
        assert!(
            !again.renews_settings_consent,
            "the modification content is byte-identical, so there is nothing new to \
             consent to and nothing is asked"
        );
    }

    #[test]
    fn an_upgrade_may_name_a_revision_this_machine_does_not_hold_and_says_so() {
        let (_d, layout, _digest) = home();
        let elsewhere = "d".repeat(64);
        let upgrade = plan_upgrade(&layout, &manifest(), &elsewhere).unwrap();
        assert!(!upgrade.installed_here);
        assert!(upgrade.describe().contains("not installed"));
    }

    #[test]
    fn an_upgrade_to_a_display_identifier_is_refused() {
        let (_d, layout, digest) = home();
        let err = plan_upgrade(&layout, &manifest(), &harness::display_id(&digest)).unwrap_err();
        assert!(matches!(err, UpgradeRefusal::Malformed { .. }));
    }

    #[test]
    fn doctor_reports_the_disagreement_and_says_nothing_when_there_is_none() {
        let (_d, layout, digest) = home();
        assert_eq!(
            doctor_finding(&evaluate(
                &layout,
                &with_requirement(&digest),
                Some(&digest)
            )),
            None,
            "an exact standing is not a finding"
        );
        let finding = doctor_finding(&evaluate(&layout, &manifest(), None)).unwrap();
        let statecraft_environment::doctor::Finding::HarnessRequirement { standing, .. } = &finding
        else {
            panic!("{finding:?}");
        };
        assert_eq!(standing, "unrequired");
        assert!(finding.describe().starts_with("harness-requirement"));
    }

    #[test]
    fn a_requirement_binds_the_configuration_layers_because_it_is_one() {
        // The key lives in the governed requirements map, so a run choice that
        // contradicts it is refused by the existing authority rules rather than
        // by a second mechanism written beside them.
        let (_d, _layout, digest) = home();
        let m = with_requirement(&digest);
        let resolution = crate::authority::resolve(
            &crate::home::Personal::default(),
            &crate::authority::Trusted {
                project: m.project.clone(),
                source: crate::authority::Source::TrustedBase {
                    revision: "base".into(),
                },
                authority_change: None,
            },
            &crate::authority::TeamAnswer::NotEnrolled,
            &crate::authority::RunChoices(
                [(REQUIREMENT_KEY.to_string(), "h-somethingelse".to_string())]
                    .into_iter()
                    .collect(),
            ),
        );
        let answer = &resolution.keys[REQUIREMENT_KEY];
        assert!(
            matches!(answer, crate::authority::Answer::Refused { .. }),
            "a run choice overrode the committed harness requirement: {answer:?}"
        );
    }

    /// Spec 002 section 3.32 rule 28: installed and intact is not resolved,
    /// and the rendering keeps them apart.
    #[test]
    fn exact_with_nothing_resolved_does_not_say_resolved() {
        let required = "a".repeat(64);
        let unresolved = Standing::Exact {
            required: required.clone(),
            resolved: None,
        };
        let text = unresolved.describe();
        assert!(text.contains("no revision has resolved"), "{text}");
        assert!(!text.contains("and resolved"), "{text}");
        let resolved = Standing::Exact {
            required: required.clone(),
            resolved: Some(required),
        };
        assert!(resolved.describe().contains("and resolved"));
    }
}
