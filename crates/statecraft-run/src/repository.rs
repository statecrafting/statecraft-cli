//! The one key a repository's records in the product home are filed under
//! (spec 003 section 3.1.4 rules 3, 4 and 7; section 5, 2026-09-23).
//!
//! The run record, the override journal and the repository lock are one file
//! each per repository. The register decides whether a typed path names a
//! registered repository, and it compares paths component-wise, so `/x/p`,
//! `/x/p/` and `/x/p/.` are one registration. A key digested from the typed
//! bytes would give that one registration three lock files, three chains and
//! three journals. The key is therefore the registration's **stored** root, as
//! the register holds it, and a caller resolves the typed path to it before
//! anything is keyed.
//!
//! Two further guards, because a key alone cannot see them:
//!
//! - Two registrations naming one directory (a path and a symbolic link to it,
//!   each registered) are two stored roots for one repository. [`resolve`]
//!   refuses both rather than choose one.
//! - Records written before the key was the stored root were keyed by whatever
//!   spelling the operator typed. [`elsewhere`] finds the ones filed under a
//!   spelling the register equates with this root, so a chain that exists under
//!   another spelling is never read as "no history" and never merged.

use statecraft_environment::digest::digest_bytes;
use statecraft_environment::registry::{Registration, Registry};
use std::path::{Path, PathBuf};

/// The digest a repository's records are filed under.
///
/// Computed exactly as it always was, from the path's lossy UTF-8 bytes, so a
/// record written through the registered spelling keeps its key.
pub fn key(root: &Path) -> String {
    digest_bytes(root.to_string_lossy().as_bytes())
}

/// Why a typed path did not resolve to one registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unresolved {
    /// No registration names it.
    NotRegistered,
    /// More than one registration names the same directory.
    SameDirectory(Vec<PathBuf>),
}

/// The registration a typed path names, whose stored root is the key.
///
/// The register's own comparison decides whether the path is registered; this
/// adds only the refusal of a directory registered under two roots, which would
/// otherwise be two locks for one repository.
pub fn resolve<'r>(registry: &'r Registry, typed: &Path) -> Result<&'r Registration, Unresolved> {
    let registration = registry.get(typed).ok_or(Unresolved::NotRegistered)?;
    let Some(identity) = directory_identity(&registration.root) else {
        return Ok(registration);
    };
    let mut roots: Vec<PathBuf> = registry
        .all()
        .filter(|r| directory_identity(&r.root) == Some(identity))
        .map(|r| r.root.clone())
        .collect();
    if roots.len() > 1 {
        roots.sort();
        return Err(Unresolved::SameDirectory(roots));
    }
    Ok(registration)
}

#[cfg(unix)]
fn directory_identity(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).ok().map(|m| (m.dev(), m.ino()))
}

#[cfg(not(unix))]
fn directory_identity(_: &Path) -> Option<(u64, u64)> {
    None
}

/// Spellings the register treats as `root` that key differently from it.
///
/// Not every such spelling: the set is unbounded (`/x//p`, `/x/./p`). These are
/// the ones an operator produces by typing the path with a trailing separator
/// or `.`, and the ones making a relative `.` or `./` absolute produces.
pub fn other_spellings(root: &Path) -> Vec<String> {
    let own = root.to_string_lossy().to_string();
    let normal: PathBuf = root.components().collect();
    let normal = normal.to_string_lossy().to_string();
    let base = normal.trim_end_matches('/');
    let mut out: Vec<String> = Vec::new();
    for s in [
        normal.clone(),
        format!("{base}/"),
        format!("{base}/."),
        format!("{base}/./"),
        format!("{base}//"),
    ] {
        if s != own && !s.is_empty() && !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// A record file for this repository filed under another spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Elsewhere {
    /// The spelling it is keyed by.
    pub spelling: String,
    /// The file.
    pub file: PathBuf,
}

/// Record files named `<key><suffix>` in `records` that belong to `root` under
/// another spelling.
pub fn elsewhere(records: &Path, root: &Path, suffix: &str) -> Vec<Elsewhere> {
    other_spellings(root)
        .into_iter()
        .map(|spelling| {
            let file = records.join(format!("{}{suffix}", key(Path::new(&spelling))));
            Elsewhere { spelling, file }
        })
        .filter(|e| e.file.exists())
        .collect()
}

/// The sentence a refusal to read records filed elsewhere carries.
pub fn elsewhere_detail(found: &[Elsewhere]) -> String {
    let listed: Vec<String> = found
        .iter()
        .map(|e| format!("{} (typed as {})", e.file.display(), e.spelling))
        .collect();
    format!(
        "has records filed under another spelling of the same registered path: {}. They were \
         neither read nor merged, and nothing was written, because an empty record here would \
         otherwise read as no history",
        listed.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_is_unchanged_for_the_registered_spelling() {
        // The digest every home was already keyed by for this spelling.
        assert_eq!(
            key(Path::new("/fixture/a")),
            digest_bytes("/fixture/a".as_bytes())
        );
    }

    #[test]
    fn other_spellings_are_the_ones_the_register_equates() {
        let root = Path::new("/x/p");
        let others = other_spellings(root);
        assert_eq!(others, vec!["/x/p/", "/x/p/.", "/x/p/./", "/x/p//"]);
        for s in &others {
            assert_eq!(Path::new(s), root, "{s}");
        }
        let dotted = other_spellings(Path::new("/x/p/."));
        assert!(dotted.contains(&"/x/p".to_string()));
        assert!(!dotted.contains(&"/x/p/.".to_string()));
    }

    struct Probe;
    impl statecraft_environment::qualify::TargetProbe for Probe {
        fn is_git_work_tree(&self, _: &Path) -> bool {
            true
        }
        fn has_base_revision(&self, _: &Path) -> bool {
            true
        }
        fn corpus(&self, _: &Path) -> statecraft_environment::qualify::CorpusState {
            statecraft_environment::qualify::CorpusState::Compiles
        }
    }

    #[test]
    fn a_typed_spelling_resolves_to_the_stored_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("p");
        std::fs::create_dir_all(&root).unwrap();
        let mut registry = Registry::default();
        registry.register(&root, &Probe).unwrap();
        for typed in [
            root.clone(),
            PathBuf::from(format!("{}/", root.display())),
            root.join("."),
        ] {
            let r = resolve(&registry, &typed).unwrap();
            assert_eq!(
                r.root.to_string_lossy(),
                root.to_string_lossy(),
                "{typed:?}"
            );
        }
        assert_eq!(
            resolve(&registry, &dir.path().join("q")),
            Err(Unresolved::NotRegistered)
        );
    }

    #[cfg(unix)]
    #[test]
    fn one_directory_registered_under_two_roots_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("p");
        std::fs::create_dir_all(&root).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&root, &link).unwrap();
        let mut registry = Registry::default();
        registry.register(&root, &Probe).unwrap();
        // A symbolic link that is not itself registered is not registered.
        assert_eq!(resolve(&registry, &link), Err(Unresolved::NotRegistered));
        assert!(resolve(&registry, &root).is_ok());
        registry.register(&link, &Probe).unwrap();
        for typed in [&root, &link] {
            let Err(Unresolved::SameDirectory(roots)) = resolve(&registry, typed) else {
                panic!("{typed:?} resolved");
            };
            assert_eq!(roots.len(), 2);
        }
    }

    #[test]
    fn records_under_another_spelling_are_found() {
        let home = tempfile::tempdir().unwrap();
        let records = home.path().join("records");
        std::fs::create_dir_all(&records).unwrap();
        assert!(elsewhere(&records, Path::new("/x/p"), ".jsonl").is_empty());
        let stray = records.join(format!("{}.jsonl", key(Path::new("/x/p/"))));
        std::fs::write(&stray, "").unwrap();
        let found = elsewhere(&records, Path::new("/x/p"), ".jsonl");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, stray);
        assert!(elsewhere(&records, Path::new("/x/p/"), ".jsonl").is_empty());
        assert!(elsewhere(&records, Path::new("/x/p"), ".overrides.jsonl").is_empty());
    }
}
