//! Who claims a path, and whether the claimed copy is the one in force.
//!
//! Spec 002 section 3.21, retained parts 1 and 2 (first stated in section 3.7,
//! since withdrawn): a `foreign` finding names an OWNER, not only a path.
//! Once the claimant can be a globally cached package rather than a file
//! somebody copied in, a path alone stops describing the conflict.
//!
//! Section 3.7 named the precedence trap that makes `shadowed` necessary:
//! a personal file can resolve ahead of a project file of the same name, so the
//! governed copy is present, readable, and not what runs. A digest match is not
//! evidence that a file is in force.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The identity of whatever claims a path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Claimant {
    /// A versioned, namespaced package, which is what spec-spine's design note
    /// 06 proposes kit files become.
    Package {
        /// The package name.
        name: String,
        /// The revision the claim was observed at.
        revision: String,
    },
    /// A file on disk, named by path, which is what a copied kit file is today.
    Path {
        /// Absolute or repository-relative, as observed.
        path: String,
    },
    /// The user: a file at `path` that no manifest records and no other
    /// installer claims.
    ///
    /// Section 3.2 makes `user` everything that is neither `managed` nor
    /// `adopted`, and section 3.8 makes a pre-existing file at a pointer path
    /// the user's. With the kit withdrawn (sections 3.21 and 3.22) there is no
    /// second installer left to name, so this is the owner a `foreign` finding
    /// names for an occupied path: the owner, and the path it holds.
    User {
        /// Repository-relative, as declared.
        path: String,
    },
    /// Something claims the path and this product cannot say what.
    ///
    /// Withdrawn section 3.7's closing sentence: neither the package nor the declaration
    /// exists yet, so an unobservable claimant is recorded as not-recorded
    /// rather than guessed at or silently omitted.
    NotRecorded,
}

impl Claimant {
    /// A one-line rendering for a report.
    pub fn describe(&self) -> String {
        match self {
            Claimant::Package { name, revision } => format!("package {name}@{revision}"),
            Claimant::Path { path } => format!("path {path}"),
            Claimant::User { path } => {
                format!("owner user: {path} is in no manifest and no other installer claims it")
            }
            Claimant::NotRecorded => "not-recorded".to_string(),
        }
    }

    /// The owner alone, for a report field that carries it apart from the path.
    ///
    /// Section 3.21 part 1: a `foreign` finding names an owner, not only a
    /// path. A [`Claimant::Path`] names only a path, so its owner is the
    /// reserved absence word rather than the path restated as an owner.
    pub fn owner(&self) -> String {
        match self {
            Claimant::Package { name, revision } => format!("package {name}@{revision}"),
            Claimant::User { .. } => "user".to_string(),
            Claimant::Path { .. } | Claimant::NotRecorded => "not-recorded".to_string(),
        }
    }
}

/// What a session would actually resolve for a managed path.
///
/// A trait because the answer depends on a harness's own precedence rules,
/// which this crate does not model and must not guess. The default
/// implementation observes nothing, which yields [`Claimant::NotRecorded`]
/// rather than a false `present`.
pub trait ShadowResolver {
    /// The claimant that would win for `repo_relative` inside `root`, if this
    /// resolver can observe one.
    ///
    /// `None` means "no shadow observed", which is not the same as "no shadow
    /// exists": see [`UnobservedShadows`].
    fn shadowing_claimant(&self, root: &Path, repo_relative: &str) -> Option<Claimant>;
}

/// The resolver that observes nothing.
///
/// Correct today and deliberately useless: no harness package format exists to
/// interrogate. It reports no shadow, and `doctor` says so as `not-recorded`
/// where it matters rather than claiming the path is in force.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnobservedShadows;

impl ShadowResolver for UnobservedShadows {
    fn shadowing_claimant(&self, _root: &Path, _repo_relative: &str) -> Option<Claimant> {
        None
    }
}

/// A resolver backed by an explicit table, for tests and for a caller that has
/// already resolved precedence by some other means.
#[derive(Debug, Clone, Default)]
pub struct StaticShadows(Vec<(String, Claimant)>);

impl StaticShadows {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare that `repo_relative` is shadowed by `claimant`.
    #[must_use]
    pub fn shadowing(mut self, repo_relative: &str, claimant: Claimant) -> Self {
        self.0.push((repo_relative.to_string(), claimant));
        self
    }
}

impl ShadowResolver for StaticShadows {
    fn shadowing_claimant(&self, _root: &Path, repo_relative: &str) -> Option<Claimant> {
        self.0
            .iter()
            .find(|(p, _)| p == repo_relative)
            .map(|(_, c)| c.clone())
    }
}

/// Paths another installer owns, which this product must not write.
///
/// Spec 002 section 3.21 part 3 (first stated in withdrawn section 3.7.1): the
/// product never writes a path another installer owns unless the manifest
/// records an explicit transfer. This is that set,
/// supplied by the caller rather than hardcoded, because spec-spine owns its
/// kit and this repository does not vendor a copy of its file list.
#[derive(Debug, Clone, Default)]
pub struct ForeignClaims {
    claims: Vec<(String, Claimant)>,
}

impl ForeignClaims {
    /// No foreign claims.
    pub fn none() -> Self {
        Self::default()
    }

    /// Record that `repo_relative` is claimed by `claimant`.
    #[must_use]
    pub fn claiming(mut self, repo_relative: &str, claimant: Claimant) -> Self {
        self.claims.push((repo_relative.to_string(), claimant));
        self
    }

    /// The claimant of `repo_relative`, if any.
    pub fn claimant_of(&self, repo_relative: &str) -> Option<&Claimant> {
        self.claims
            .iter()
            .find(|(p, _)| p == repo_relative)
            .map(|(_, c)| c)
    }

    /// Every claimed path.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.claims.iter().map(|(p, _)| p.as_str())
    }
}

/// Join a repository root and a repository-relative path.
///
/// Repository-relative paths are stored with forward slashes so a manifest
/// written on one platform reads on another.
pub fn resolve(root: &Path, repo_relative: &str) -> PathBuf {
    let mut out = root.to_path_buf();
    for segment in repo_relative.split('/').filter(|s| !s.is_empty()) {
        out.push(segment);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claimant_describes_itself_with_its_identity_not_only_its_path() {
        let p = Claimant::Package {
            name: "spec-spine-kit".into(),
            revision: "0.18.0".into(),
        };
        assert_eq!(p.describe(), "package spec-spine-kit@0.18.0");
        assert_eq!(p.owner(), "package spec-spine-kit@0.18.0");
        assert_eq!(Claimant::NotRecorded.describe(), "not-recorded");
    }

    #[test]
    fn a_user_claimant_names_its_owner_and_the_path_it_holds() {
        let u = Claimant::User {
            path: "CLAUDE.md".into(),
        };
        assert_eq!(u.owner(), "user");
        assert!(u.describe().starts_with("owner user: CLAUDE.md"));
        // A bare path is not an owner, and is not dressed up as one.
        let p = Claimant::Path {
            path: "CLAUDE.md".into(),
        };
        assert_eq!(p.owner(), "not-recorded");
    }

    #[test]
    fn the_default_resolver_observes_no_shadow() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            UnobservedShadows
                .shadowing_claimant(dir.path(), "AGENTS.md")
                .is_none()
        );
    }

    #[test]
    fn a_static_resolver_answers_only_for_the_paths_it_was_given() {
        let dir = tempfile::tempdir().unwrap();
        let r = StaticShadows::new().shadowing(
            ".claude/skills/ship.md",
            Claimant::Path {
                path: "~/.claude/skills/ship.md".into(),
            },
        );
        assert!(
            r.shadowing_claimant(dir.path(), ".claude/skills/ship.md")
                .is_some()
        );
        assert!(r.shadowing_claimant(dir.path(), "other.md").is_none());
    }

    #[test]
    fn resolve_joins_forward_slash_paths_platform_correctly() {
        let root = Path::new("/tmp/repo");
        assert_eq!(
            resolve(root, ".statecraft/environment.json"),
            Path::new("/tmp/repo/.statecraft/environment.json")
        );
    }
}
