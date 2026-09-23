//! The register of targets, kept in the product's own home.
//!
//! Spec 002 section 3.1. `project register <path>` records an absolute path
//! **under the product's own home** and evaluates a read-only qualification. The
//! location matters: registration writes nothing inside the target, so a
//! repository can be registered, found unqualified, and never know it happened.
//!
//! Arming is separate from registering. Registering says "this exists"; arming
//! says "you may drive it". Arming still does not consent to an execution
//! posture, which is spec 004's to define.

use crate::qualify::{Qualification, TargetProbe, Verdict, qualify};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One registered target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registration {
    /// The absolute path of the target.
    pub root: PathBuf,
    /// The verdict at the time of the last evaluation.
    pub qualification: Qualification,
    /// Whether the operator has consented to this target being driven.
    #[serde(default)]
    pub armed: bool,
}

impl Registration {
    /// Whether work may be scheduled here.
    ///
    /// Both conditions, and they are independent: a qualified target that was
    /// never armed is not eligible, and arming an unqualified one does not make
    /// it so.
    pub fn eligible(&self) -> bool {
        self.armed && self.qualification.verdict.schedulable()
    }
}

/// The register itself.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registry {
    /// Registered targets, ordered by path so the stored file is stable.
    #[serde(default)]
    pub projects: Vec<Registration>,
}

/// What went wrong reading or writing the register.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    /// The register could not be read or written.
    #[error("registry i/o at {path}: {source}")]
    Io {
        /// The file involved.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The register exists but could not be parsed.
    #[error("registry at {path} is malformed: {source}")]
    Malformed {
        /// The file involved.
        path: PathBuf,
        /// The underlying error.
        source: serde_json::Error,
    },
    /// A relative path was offered for registration.
    #[error("{0} is not absolute; register records absolute paths only")]
    NotAbsolute(PathBuf),
    /// An operation named a target that is not registered.
    #[error("{0} is not registered")]
    NotRegistered(PathBuf),
}

/// The register's file inside a product home.
pub fn registry_path(home: &Path) -> PathBuf {
    home.join("projects.json")
}

impl Registry {
    /// Read the register from a product home, or an empty one if absent.
    pub fn read(home: &Path) -> Result<Self, RegistryError> {
        let path = registry_path(home);
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|source| RegistryError::Malformed { path, source }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(RegistryError::Io { path, source }),
        }
    }

    /// Write the register into a product home.
    pub fn write(&self, home: &Path) -> Result<(), RegistryError> {
        let path = registry_path(home);
        std::fs::create_dir_all(home).map_err(|source| RegistryError::Io {
            path: path.clone(),
            source,
        })?;
        let mut json =
            serde_json::to_string_pretty(self).map_err(|source| RegistryError::Malformed {
                path: path.clone(),
                source,
            })?;
        json.push('\n');
        std::fs::write(&path, json).map_err(|source| RegistryError::Io { path, source })
    }

    /// The registration for a target, if it has one.
    pub fn get(&self, root: &Path) -> Option<&Registration> {
        self.projects.iter().find(|p| p.root == root)
    }

    /// Register a target, or re-evaluate one already registered.
    ///
    /// Re-registering preserves `armed`: arming is an operator's act, and a
    /// re-evaluation is not a withdrawal of consent. It does replace the
    /// verdict, because a stale verdict is worse than none.
    ///
    /// It also preserves the root **as first stored**. The register compares
    /// paths component-wise, so re-registering `<root>/` finds `<root>`; and
    /// spec 003 keys a repository's run record, override journal and lock by
    /// the stored root (its section 5, 2026-09-23). Replacing the stored
    /// spelling on a re-evaluation would re-key that history.
    pub fn register(
        &mut self,
        root: &Path,
        probe: &dyn TargetProbe,
    ) -> Result<&Registration, RegistryError> {
        if !root.is_absolute() {
            return Err(RegistryError::NotAbsolute(root.to_path_buf()));
        }
        let qualification = qualify(root, probe);
        let existing = self.get(root);
        let armed = existing.map(|r| r.armed).unwrap_or(false);
        let stored = existing.map_or_else(|| root.to_path_buf(), |r| r.root.clone());
        let registration = Registration {
            root: stored,
            qualification,
            armed,
        };
        match self
            .projects
            .binary_search_by(|p| p.root.cmp(&registration.root))
        {
            Ok(i) => self.projects[i] = registration,
            Err(i) => self.projects.insert(i, registration),
        }
        Ok(self.get(root).expect("just inserted"))
    }

    /// Store a registered target's root under another spelling of it.
    ///
    /// The register itself treats the two as one path; this changes only which
    /// spelling it holds. Whether that is safe is not the register's to judge:
    /// spec 003 keys a repository's records by the stored root, and its caller
    /// decides (spec 003 section 5, 2026-09-23). A path the register does not
    /// already hold is refused.
    pub fn respell(&mut self, root: &Path) -> Result<(), RegistryError> {
        let p = self
            .projects
            .iter_mut()
            .find(|p| p.root == root)
            .ok_or_else(|| RegistryError::NotRegistered(root.to_path_buf()))?;
        p.root = root.to_path_buf();
        Ok(())
    }

    /// Arm or disarm a registered target.
    pub fn set_armed(&mut self, root: &Path, armed: bool) -> Result<(), RegistryError> {
        let p = self
            .projects
            .iter_mut()
            .find(|p| p.root == root)
            .ok_or_else(|| RegistryError::NotRegistered(root.to_path_buf()))?;
        p.armed = armed;
        Ok(())
    }

    /// Every target that may be scheduled.
    pub fn eligible(&self) -> impl Iterator<Item = &Registration> {
        self.projects.iter().filter(|p| p.eligible())
    }

    /// Every target, eligible or not. Registration makes a target VISIBLE; that
    /// is the whole point of recording an unqualified one.
    pub fn all(&self) -> impl Iterator<Item = &Registration> {
        self.projects.iter()
    }

    /// Targets with a given verdict.
    pub fn with_verdict(&self, verdict: Verdict) -> impl Iterator<Item = &Registration> {
        self.projects
            .iter()
            .filter(move |p| p.qualification.verdict == verdict)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qualify::CorpusState;

    struct Good;
    impl TargetProbe for Good {
        fn is_git_work_tree(&self, _: &Path) -> bool {
            true
        }
        fn has_base_revision(&self, _: &Path) -> bool {
            true
        }
        fn corpus(&self, _: &Path) -> CorpusState {
            CorpusState::Compiles
        }
    }

    struct NotGit;
    impl TargetProbe for NotGit {
        fn is_git_work_tree(&self, _: &Path) -> bool {
            false
        }
        fn has_base_revision(&self, _: &Path) -> bool {
            false
        }
        fn corpus(&self, _: &Path) -> CorpusState {
            CorpusState::Absent
        }
    }

    #[test]
    fn registering_writes_nothing_inside_the_target() {
        let target = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let before: Vec<_> = std::fs::read_dir(target.path()).unwrap().collect();
        let mut r = Registry::default();
        r.register(target.path(), &Good).unwrap();
        r.write(home.path()).unwrap();
        let after: Vec<_> = std::fs::read_dir(target.path()).unwrap().collect();
        assert_eq!(before.len(), after.len());
        assert_eq!(after.len(), 0);
    }

    #[test]
    fn a_relative_path_is_refused() {
        let mut r = Registry::default();
        assert!(matches!(
            r.register(Path::new("relative/path"), &Good),
            Err(RegistryError::NotAbsolute(_))
        ));
    }

    #[test]
    fn an_unqualified_target_is_still_recorded_and_visible() {
        let target = tempfile::tempdir().unwrap();
        let mut r = Registry::default();
        r.register(target.path(), &NotGit).unwrap();
        assert_eq!(r.all().count(), 1);
        assert_eq!(r.with_verdict(Verdict::Unqualified).count(), 1);
        assert_eq!(r.eligible().count(), 0);
    }

    #[test]
    fn arming_is_separate_from_registering() {
        let target = tempfile::tempdir().unwrap();
        let mut r = Registry::default();
        r.register(target.path(), &Good).unwrap();
        assert_eq!(r.eligible().count(), 0, "qualified is not armed");
        r.set_armed(target.path(), true).unwrap();
        assert_eq!(r.eligible().count(), 1);
    }

    #[test]
    fn arming_an_unqualified_target_does_not_make_it_eligible() {
        let target = tempfile::tempdir().unwrap();
        let mut r = Registry::default();
        r.register(target.path(), &NotGit).unwrap();
        r.set_armed(target.path(), true).unwrap();
        assert_eq!(r.eligible().count(), 0);
    }

    #[test]
    fn re_registering_preserves_arming_and_replaces_the_verdict() {
        let target = tempfile::tempdir().unwrap();
        let mut r = Registry::default();
        r.register(target.path(), &Good).unwrap();
        r.set_armed(target.path(), true).unwrap();
        r.register(target.path(), &NotGit).unwrap();
        let reg = r.get(target.path()).unwrap();
        assert!(reg.armed, "consent survives a re-evaluation");
        assert_eq!(reg.qualification.verdict, Verdict::Unqualified);
        assert!(!reg.eligible());
    }

    #[test]
    fn respelling_changes_only_the_stored_spelling() {
        let target = tempfile::tempdir().unwrap();
        let first = target.path().to_path_buf();
        let mut r = Registry::default();
        r.register(&first, &Good).unwrap();
        r.set_armed(&first, true).unwrap();
        let slash = PathBuf::from(format!("{}/", first.display()));
        r.respell(&slash).unwrap();
        assert_eq!(r.projects.len(), 1);
        let reg = r.get(&first).unwrap();
        assert_eq!(reg.root.as_os_str(), slash.as_os_str());
        assert!(reg.armed);
        assert!(matches!(
            r.respell(&target.path().join("other")),
            Err(RegistryError::NotRegistered(_))
        ));
    }

    #[test]
    fn re_registering_under_another_spelling_keeps_the_stored_root() {
        let target = tempfile::tempdir().unwrap();
        let first = target.path().to_path_buf();
        let mut r = Registry::default();
        r.register(&first, &Good).unwrap();
        for again in [
            PathBuf::from(format!("{}/", first.display())),
            first.join("."),
        ] {
            r.register(&again, &Good).unwrap();
            assert_eq!(r.projects.len(), 1);
            assert_eq!(
                r.get(&again).unwrap().root.as_os_str(),
                first.as_os_str(),
                "{again:?}"
            );
        }
    }

    #[test]
    fn arming_an_unregistered_target_is_an_error_not_a_silent_insert() {
        let mut r = Registry::default();
        assert!(matches!(
            r.set_armed(Path::new("/nowhere"), true),
            Err(RegistryError::NotRegistered(_))
        ));
    }

    #[test]
    fn the_register_round_trips_through_the_product_home() {
        let target = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let mut r = Registry::default();
        r.register(target.path(), &Good).unwrap();
        r.set_armed(target.path(), true).unwrap();
        r.write(home.path()).unwrap();
        assert_eq!(Registry::read(home.path()).unwrap(), r);
    }

    #[test]
    fn an_absent_register_reads_as_empty() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(Registry::read(home.path()).unwrap(), Registry::default());
    }
}
