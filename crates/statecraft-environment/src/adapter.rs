//! Agent-harness adapters: what one declares, and when it refuses.
//!
//! Spec 002 section 3.9. An adapter declares the harness it targets, the exact
//! set of paths it would manage, the facts it cannot express in that harness,
//! and the prerequisites it needs present. Adapters are additive and
//! independent, and two adapters may not declare the same path.
//!
//! Refusal is the interesting part. An adapter whose harness is absent, or whose
//! prerequisites are missing, refuses to claim its paths and says which
//! prerequisite is absent. It does not write files for a harness that is not
//! there, and it never falls back to appending to a file it does not own
//! (section 3.8, decision D-09).

use serde::{Deserialize, Serialize};

/// A condition an adapter needs before it will claim anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prerequisite {
    /// Stable identifier, for reporting and for a test to assert on.
    pub id: String,
    /// What a reader needs to know to satisfy it.
    pub description: String,
}

impl Prerequisite {
    /// A prerequisite with an id and a description.
    pub fn new(id: &str, description: &str) -> Self {
        Self {
            id: id.to_string(),
            description: description.to_string(),
        }
    }
}

/// A path an adapter would write, and what it would write there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedFile {
    /// Repository-relative, forward slashes.
    pub path: String,
    /// The bytes the adapter would write.
    pub contents: Vec<u8>,
    /// True when this is a pointer file rather than the adapter's own content.
    ///
    /// Section 3.8 allows at most one pointer per adapter, written only where no
    /// file exists at that path. The distinction is recorded because the two
    /// have different collision behavior: a pointer path already holding a file
    /// degrades the adapter, it does not merely withhold a write.
    pub pointer: bool,
    /// What the entry this file becomes is for. `reference` unless the
    /// declaration says otherwise; only a first write records it, because a
    /// recorded role never changes automatically.
    pub role: crate::manifest::Role,
}

impl ManagedFile {
    /// A file the adapter owns outright.
    pub fn owned(path: &str, contents: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.to_string(),
            contents: contents.into(),
            pointer: false,
            role: crate::manifest::Role::Reference,
        }
    }

    /// A file this product seeds for the project to author (spec 002 section
    /// 5, 2026-09-24, provenance item 2): written only when absent, and never
    /// rewritten once recorded as an authored input.
    pub fn authored_input(path: &str, contents: impl Into<Vec<u8>>) -> Self {
        Self {
            role: crate::manifest::Role::AuthoredInput,
            ..Self::owned(path, contents)
        }
    }

    /// A pointer file, written only where nothing exists.
    pub fn pointer(path: &str, contents: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.to_string(),
            contents: contents.into(),
            pointer: true,
            role: crate::manifest::Role::Reference,
        }
    }
}

/// What an adapter declares about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    /// The adapter's own name, unique within a plan.
    pub name: String,
    /// The harness it targets.
    pub harness: String,
    /// Its version, recorded in the manifest pins.
    pub version: String,
    /// Exactly the paths it would manage.
    pub files: Vec<ManagedFile>,
    /// Facts it cannot express in this harness, stated rather than dropped.
    pub unexpressible: Vec<String>,
    /// What must be present before it will claim anything.
    pub prerequisites: Vec<Prerequisite>,
}

impl Declaration {
    /// Every path this adapter declares.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.iter().map(|f| f.path.as_str())
    }
}

/// Whether an adapter will claim its paths in a given target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    /// The harness is present and every prerequisite is satisfied.
    Claiming,
    /// The harness is absent, or a prerequisite is missing. The adapter claims
    /// nothing and names what is absent.
    Refused {
        /// The prerequisite ids that are not satisfied, harness absence first.
        missing: Vec<String>,
    },
    /// The adapter claims its own paths but could not place a pointer, because
    /// a file already exists at the pointer path.
    ///
    /// Section 3.8: the existing file is reported as `foreign`, the adapter
    /// reports itself degraded, and it does not append.
    Degraded {
        /// Why, one line per reason.
        reasons: Vec<String>,
    },
}

impl Readiness {
    /// True when the adapter will write anything at all.
    pub fn claims_paths(&self) -> bool {
        !matches!(self, Readiness::Refused { .. })
    }
}

/// What the environment can observe about a target, for adapters to decide on.
///
/// A trait so a test does not need a real harness installed, and so this crate
/// never infers a harness's presence from a heuristic it invented.
pub trait HarnessProbe {
    /// True when the named harness is present for this target.
    fn harness_present(&self, harness: &str) -> bool;
    /// True when the named prerequisite is satisfied for this target.
    fn prerequisite_satisfied(&self, harness: &str, prerequisite: &str) -> bool;
}

/// A probe backed by explicit facts, for tests and for a caller that has
/// already done the detection.
#[derive(Debug, Clone, Default)]
pub struct StaticProbe {
    harnesses: Vec<String>,
    prerequisites: Vec<(String, String)>,
}

impl StaticProbe {
    /// Nothing present.
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare a harness present.
    #[must_use]
    pub fn with_harness(mut self, harness: &str) -> Self {
        self.harnesses.push(harness.to_string());
        self
    }

    /// Declare a prerequisite satisfied for a harness.
    #[must_use]
    pub fn with_prerequisite(mut self, harness: &str, prerequisite: &str) -> Self {
        self.prerequisites
            .push((harness.to_string(), prerequisite.to_string()));
        self
    }
}

impl HarnessProbe for StaticProbe {
    fn harness_present(&self, harness: &str) -> bool {
        self.harnesses.iter().any(|h| h == harness)
    }

    fn prerequisite_satisfied(&self, harness: &str, prerequisite: &str) -> bool {
        self.prerequisites
            .iter()
            .any(|(h, p)| h == harness && p == prerequisite)
    }
}

/// Decide whether an adapter claims its paths.
///
/// Harness absence is reported as the pseudo-prerequisite `harness-present`, and
/// it comes first: an adapter whose harness is missing has every prerequisite
/// missing too, and listing them all would bury the one that matters.
pub fn readiness(declaration: &Declaration, probe: &dyn HarnessProbe) -> Readiness {
    if !probe.harness_present(&declaration.harness) {
        return Readiness::Refused {
            missing: vec!["harness-present".to_string()],
        };
    }
    let missing: Vec<String> = declaration
        .prerequisites
        .iter()
        .filter(|p| !probe.prerequisite_satisfied(&declaration.harness, &p.id))
        .map(|p| p.id.clone())
        .collect();
    if missing.is_empty() {
        Readiness::Claiming
    } else {
        Readiness::Refused { missing }
    }
}

/// A path two adapters both declare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathCollision {
    /// The contested path.
    pub path: String,
    /// The two adapters, in declaration order.
    pub adapters: (String, String),
}

/// Every path more than one adapter declares.
///
/// Section 3.10 requires this to be refused at PLAN time, naming both adapters
/// and the path, which is why it is computed over declarations rather than
/// discovered when the second write fails.
pub fn collisions(declarations: &[Declaration]) -> Vec<PathCollision> {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    let mut out = Vec::new();
    for d in declarations {
        for path in d.paths() {
            if let Some((_, first)) = seen.iter().find(|(p, _)| *p == path) {
                out.push(PathCollision {
                    path: path.to_string(),
                    adapters: ((*first).to_string(), d.name.clone()),
                });
            } else {
                seen.push((path, &d.name));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declaration(name: &str, paths: &[&str]) -> Declaration {
        Declaration {
            name: name.into(),
            harness: "claude-code".into(),
            version: "1".into(),
            files: paths
                .iter()
                .map(|p| ManagedFile::owned(p, b"x".to_vec()))
                .collect(),
            unexpressible: vec![],
            prerequisites: vec![],
        }
    }

    #[test]
    fn an_adapter_for_an_absent_harness_refuses_and_names_the_absence() {
        let d = declaration("a", &["p"]);
        match readiness(&d, &StaticProbe::new()) {
            Readiness::Refused { missing } => assert_eq!(missing, ["harness-present"]),
            other => panic!("expected refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_missing_prerequisite_refuses_and_names_that_prerequisite() {
        let mut d = declaration("a", &["p"]);
        d.prerequisites = vec![Prerequisite::new(
            "loads-pointer",
            "the harness loads a pointer file at the declared path",
        )];
        let probe = StaticProbe::new().with_harness("claude-code");
        match readiness(&d, &probe) {
            Readiness::Refused { missing } => assert_eq!(missing, ["loads-pointer"]),
            other => panic!("expected refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_satisfied_adapter_claims() {
        let mut d = declaration("a", &["p"]);
        d.prerequisites = vec![Prerequisite::new("loads-pointer", "x")];
        let probe = StaticProbe::new()
            .with_harness("claude-code")
            .with_prerequisite("claude-code", "loads-pointer");
        assert_eq!(readiness(&d, &probe), Readiness::Claiming);
    }

    #[test]
    fn two_adapters_declaring_one_path_collide_and_both_are_named() {
        let a = declaration("alpha", &["shared", "a-only"]);
        let b = declaration("beta", &["shared"]);
        let c = collisions(&[a, b]);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].path, "shared");
        assert_eq!(c[0].adapters, ("alpha".to_string(), "beta".to_string()));
    }

    #[test]
    fn independent_adapters_do_not_collide() {
        let a = declaration("alpha", &["a"]);
        let b = declaration("beta", &["b"]);
        assert!(collisions(&[a, b]).is_empty());
    }
}
