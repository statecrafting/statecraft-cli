//! A resolved run is frozen.
//!
//! Spec 002 section 3.16, last rule. A run resolves its harness revision and its
//! tools **once**, and records the requested identity alongside the identity
//! that actually resolved. A later global upgrade changes the home and changes
//! nothing about a run already resolved, which is only true because the
//! resolution is a written record rather than a fresh lookup.
//!
//! The record lives under the project's runtime state, which is ignored: it is
//! about one run on one machine, and committing it would make a per-machine
//! fact part of the governed corpus.

use crate::authority::Resolution;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The schema version of a resolution record.
pub const RESOLVED_VERSION: u32 = 1;

/// What was asked for, and what answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    /// The name.
    pub name: String,
    /// What was asked for, verbatim.
    pub requested: String,
    /// What answered, or the reserved absence word.
    pub resolved: String,
}

/// One run's frozen resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedRun {
    /// Schema version.
    pub version: u32,
    /// The run.
    pub run_id: String,
    /// The project, absolute.
    pub project: String,
    /// When it was resolved, RFC 3339 UTC.
    pub resolved_at: String,
    /// The harness revision this run uses.
    pub harness: Identity,
    /// Every tool, ordered by name.
    pub tools: Vec<Identity>,
    /// The configuration answers, frozen with everything else.
    pub configuration: serde_json::Value,
}

impl ResolvedRun {
    /// Freeze a resolution.
    pub fn freeze(
        run_id: &str,
        project: &Path,
        resolved_at: &str,
        harness: Identity,
        mut tools: Vec<Identity>,
        configuration: &Resolution,
    ) -> Self {
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        Self {
            version: RESOLVED_VERSION,
            run_id: run_id.to_string(),
            project: project.display().to_string(),
            resolved_at: resolved_at.to_string(),
            harness,
            tools,
            configuration: serde_json::to_value(configuration).unwrap_or(serde_json::Value::Null),
        }
    }

    /// Where a run's resolution lives inside a project.
    pub fn path(root: &Path, run_id: &str) -> PathBuf {
        statecraft_environment::claimant::resolve(
            root,
            &format!("{}/resolved/{run_id}.json", crate::project::STATE),
        )
    }

    /// Write the record. Refuses to replace one that exists.
    ///
    /// The refusal is the freezing. A second `freeze` for the same run would be
    /// a re-resolution, which is exactly the thing a global upgrade must not be
    /// able to cause.
    pub fn write(&self, root: &Path) -> std::io::Result<()> {
        let path = Self::path(root, &self.run_id);
        if path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "run {} is already resolved; a resolution is written once",
                    self.run_id
                ),
            ));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut json = serde_json::to_string_pretty(self)?;
        json.push('\n');
        std::fs::write(path, json)
    }

    /// Read a run's resolution, if it has one.
    pub fn read(root: &Path, run_id: &str) -> std::io::Result<Option<Self>> {
        let path = Self::path(root, run_id);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authority::{Source, TeamAnswer, Trusted};
    use crate::home::Personal;

    fn resolution() -> Resolution {
        crate::authority::resolve(
            &Personal::default(),
            &Trusted {
                project: Default::default(),
                source: Source::TrustedBase {
                    revision: "base".into(),
                },
                authority_change: None,
            },
            &TeamAnswer::NotEnrolled,
            &crate::authority::RunChoices::none(),
        )
    }

    fn frozen(harness: &str) -> ResolvedRun {
        ResolvedRun::freeze(
            "003-x",
            Path::new("/p"),
            "1970-01-01T00:00:00Z",
            Identity {
                name: "harness".into(),
                requested: "latest".into(),
                resolved: harness.into(),
            },
            vec![Identity {
                name: "spec-spine".into(),
                requested: "=0.20.0".into(),
                resolved: "0.20.0".into(),
            }],
            &resolution(),
        )
    }

    #[test]
    fn a_resolution_records_both_the_request_and_what_answered() {
        let r = frozen("h-aaaaaaaaaaaa");
        assert_eq!(r.harness.requested, "latest");
        assert_eq!(r.harness.resolved, "h-aaaaaaaaaaaa");
        assert_eq!(r.tools[0].requested, "=0.20.0");
        assert_eq!(r.tools[0].resolved, "0.20.0");
    }

    #[test]
    fn a_global_upgrade_cannot_change_a_run_already_resolved() {
        let dir = tempfile::tempdir().unwrap();
        frozen("h-aaaaaaaaaaaa").write(dir.path()).unwrap();

        // The home upgrades: a new harness revision is now what `latest` means.
        let upgraded = frozen("h-bbbbbbbbbbbb");
        let refused = upgraded.write(dir.path()).unwrap_err();
        assert_eq!(refused.kind(), std::io::ErrorKind::AlreadyExists);

        let read = ResolvedRun::read(dir.path(), "003-x").unwrap().unwrap();
        assert_eq!(read.harness.resolved, "h-aaaaaaaaaaaa");
    }

    #[test]
    fn a_run_with_no_resolution_reads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(ResolvedRun::read(dir.path(), "nope").unwrap().is_none());
    }

    #[test]
    fn the_record_lives_in_ignored_runtime_state() {
        let path = ResolvedRun::path(Path::new("/p"), "003-x");
        assert!(path.to_string_lossy().contains(crate::project::STATE));
    }

    #[test]
    fn tools_are_ordered_so_the_record_is_stable() {
        let r = ResolvedRun::freeze(
            "r",
            Path::new("/p"),
            "1970-01-01T00:00:00Z",
            Identity {
                name: "harness".into(),
                requested: "x".into(),
                resolved: "y".into(),
            },
            vec![
                Identity {
                    name: "z".into(),
                    requested: "1".into(),
                    resolved: "1".into(),
                },
                Identity {
                    name: "a".into(),
                    requested: "1".into(),
                    resolved: "1".into(),
                },
            ],
            &resolution(),
        );
        assert_eq!(
            r.tools.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            ["a", "z"]
        );
    }
}
