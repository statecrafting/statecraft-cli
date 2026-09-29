//! A setup profile's project-owned prerequisites: spec 018 section 3.4.
//!
//! Each prerequisite is observed independently and reported `met` or
//! `unmet` with its owner and the consequence. One unmet prerequisite
//! withholds every profile-owned file, so a repository never receives a gate
//! that cannot run. Nothing here generates, copies, selects, resolves or
//! initializes anything, and no option marks a prerequisite met (I-2): a
//! toolchain, a lock file, a content policy and a work tree are the target
//! project's, and an adopted `spec-spine.toml` is its owner's.

use crate::setup::{Parameters, Prerequisite};
use std::io;
use std::path::Path;
use std::path::PathBuf;

/// The governance producer owns a `spec-spine.toml` it scaffolds.
pub const PRODUCER_OWNER: &str = "governance producer";
/// The target project owns what it builds with.
pub const PROJECT_OWNER: &str = "target project";
/// A declared checker belongs to the project and to the spec that owns it.
pub const CHECKER_OWNER: &str = "target project and its owning spec";

const WITHHELD: &str = "the whole profile is withheld";

/// What the plan read about the governance configuration.
pub struct Pin<'a> {
    /// The bytes `spec-spine.toml` will have: the disk's when it exists,
    /// else the producer's.
    pub text: Option<&'a str>,
    /// Whether the file is on disk and therefore adopted, never rewritten.
    pub adopted: bool,
    /// The exact version of the linked producer, which the pin must admit.
    pub producer_version: &'a str,
}

/// Observe every prerequisite of `github-actions-rust`, in a stable order.
pub fn observe(root: &Path, pin: &Pin<'_>, params: &Parameters) -> Vec<Prerequisite> {
    let mut out = vec![
        exact_pin(pin),
        tracked_file(
            root,
            "rust-toolchain.toml",
            "Statecraft never generates, copies or selects a toolchain: commit the project's rust-toolchain.toml",
        ),
        tracked_file(
            root,
            "Cargo.lock",
            "Statecraft never generates it or runs a dependency resolver, and every cargo verb the gate runs is --locked: commit the project's Cargo.lock",
        ),
    ];
    if let Some(rel) = &params.authored_content {
        out.push(checker(root, rel));
    }
    let work_tree = root.join(".git").exists();
    out.push(prerequisite(
        "a git work tree",
        if work_tree { "present" } else { "absent" },
        work_tree,
        PROJECT_OWNER,
        "Statecraft never initializes Git: initialize the repository first",
    ));
    out
}

fn prerequisite(
    name: &str,
    observed: impl Into<String>,
    met: bool,
    owner: &str,
    remedy: &str,
) -> Prerequisite {
    Prerequisite {
        name: name.to_string(),
        observed: observed.into(),
        met,
        owner: owner.to_string(),
        consequence: if met {
            "none".to_string()
        } else {
            format!("{WITHHELD}; {remedy}")
        },
    }
}

/// Section 3.3: a new file carries the producer's exact pin; an adopted one
/// must already be exact and admit the linked producer, and is never
/// rewritten.
fn exact_pin(pin: &Pin<'_>) -> Prerequisite {
    let owner = if pin.adopted {
        PROJECT_OWNER
    } else {
        PRODUCER_OWNER
    };
    let want = pin.producer_version;
    let read = pin
        .text
        .ok_or_else(|| "spec-spine.toml is absent".to_string())
        .and_then(crate::setup::exact_pin);
    let (observed, met) = match read {
        Ok(v) if v == want => (format!("required_version = \"={v}\""), true),
        Ok(v) => (
            format!("required_version = \"={v}\", which does not admit the linked producer {want}"),
            false,
        ),
        Err(e) => (e, false),
    };
    let remedy = if pin.adopted {
        format!(
            "the adopted spec-spine.toml is never rewritten, and no other binary is selected: set `[meta] required_version = \"={want}\"` in it"
        )
    } else {
        "the producer's scaffold did not carry its own exact pin".to_string()
    };
    prerequisite("an exact spec-spine pin", observed, met, owner, &remedy)
}

/// A regular file the git index tracks: what CI checks out is what the gate
/// runs with.
fn tracked_file(root: &Path, rel: &str, remedy: &str) -> Prerequisite {
    let observed = match std::fs::symlink_metadata(root.join(rel)) {
        Err(_) => Err("absent"),
        Ok(m) if !m.file_type().is_file() => Err("present, and not a regular file"),
        Ok(_) if !tracked(root, rel) => {
            Err("present, and not tracked by git (a checkout would not have it)")
        }
        Ok(_) => Ok("a regular tracked file"),
    };
    let met = observed.is_ok();
    let observed = observed.unwrap_or_else(|e| e);
    prerequisite(rel, observed, met, PROJECT_OWNER, remedy)
}

/// Whether the git index at `root` tracks `rel`. No work tree, or no git,
/// is not tracked.
fn tracked(root: &Path, rel: &str) -> bool {
    std::process::Command::new("git")
        .args(["ls-files", "--error-unmatch", "--", rel])
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// The declared authored-content checker: exactly `rel`, inside the target,
/// a regular file, executable.
fn checker(root: &Path, rel: &str) -> Prerequisite {
    let path = root.join(rel);
    let failed = match std::fs::symlink_metadata(&path) {
        Err(_) => Some("absent"),
        Ok(m) => {
            let resolved_path = path.canonicalize();
            let location = canonical_location(&resolved_path, &root.canonicalize());
            if matches!(location, Ok(false)) {
                Some("outside the target")
            } else if location.is_err() && (!m.file_type().is_symlink() || resolved_path.is_ok()) {
                Some("canonical path unreadable")
            } else if m.file_type().is_symlink() {
                Some("a symbolic link, not a regular file")
            } else if !m.file_type().is_file() {
                Some("not a regular file")
            } else if !executable(&m) {
                Some("not executable")
            } else {
                None
            }
        }
    };
    let observed = match failed {
        Some(why) => format!("{rel}: {why}"),
        None => format!("{rel}: a regular executable file"),
    };
    prerequisite(
        "the declared authored-content checker",
        observed,
        failed.is_none(),
        CHECKER_OWNER,
        &format!(
            "Statecraft never invents or copies a content policy: add the executable checker at {rel}, or leave governance.authored_content unset"
        ),
    )
}

/// Whether two resolved paths put the checker inside the target. A failure to
/// resolve either side is not evidence that the checker is outside it.
fn canonical_location(path: &io::Result<PathBuf>, root: &io::Result<PathBuf>) -> io::Result<bool> {
    match (path, root) {
        (Ok(path), Ok(root)) => Ok(path.starts_with(root)),
        (Err(error), _) | (_, Err(error)) => Err(io::Error::new(error.kind(), error.to_string())),
    }
}

#[cfg(unix)]
fn executable(m: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    m.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable(_: &std::fs::Metadata) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn git(dir: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}");
    }

    fn params(root: &Path, checker: Option<&str>) -> Parameters {
        let mut block = BTreeMap::new();
        if let Some(rel) = checker {
            block.insert(
                "governance.authored_content".to_string(),
                serde_json::json!(rel),
            );
        }
        crate::setup::parameters(root, &block, ".statecraft/derived").unwrap()
    }

    fn find<'a>(all: &'a [Prerequisite], name: &str) -> &'a Prerequisite {
        all.iter().find(|p| p.name.contains(name)).unwrap()
    }

    const PINNED: &str = "[meta]\nrequired_version = \"=1.2.3\"\n";

    #[test]
    fn a_rust_prerequisite_must_be_a_regular_tracked_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "--quiet"]);
        let pin = Pin {
            text: Some(PINNED),
            adopted: false,
            producer_version: "1.2.3",
        };
        let all = observe(root, &pin, &params(root, None));
        let lock = find(&all, "Cargo.lock");
        assert!(!lock.met);
        assert_eq!(lock.observed, "absent");
        assert_eq!(lock.owner, PROJECT_OWNER);
        assert!(
            lock.consequence.starts_with(WITHHELD),
            "{}",
            lock.consequence
        );

        std::fs::write(root.join("Cargo.lock"), "version = 4\n").unwrap();
        std::fs::create_dir(root.join("rust-toolchain.toml")).unwrap();
        let all = observe(root, &pin, &params(root, None));
        assert!(find(&all, "Cargo.lock").observed.contains("not tracked"));
        assert!(
            find(&all, "rust-toolchain")
                .observed
                .contains("not a regular file")
        );

        git(root, &["add", "Cargo.lock"]);
        let all = observe(root, &pin, &params(root, None));
        let lock = find(&all, "Cargo.lock");
        assert!(lock.met, "{lock:?}");
        assert_eq!(lock.consequence, "none");
        assert!(find(&all, "git work tree").met);
        assert!(
            !root.join("rust-toolchain.toml").is_file(),
            "nothing is created"
        );
    }

    #[test]
    fn an_adopted_pin_must_be_exact_and_admit_the_linked_producer() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let p = params(root, None);
        let at = |text: Option<&str>, adopted: bool| {
            let pin = Pin {
                text,
                adopted,
                producer_version: "1.2.3",
            };
            find(&observe(root, &pin, &p), "spec-spine pin").clone()
        };
        let new = at(Some(PINNED), false);
        assert!(new.met);
        assert_eq!(new.owner, PRODUCER_OWNER);
        let adopted = at(Some(PINNED), true);
        assert!(adopted.met);
        assert_eq!(adopted.owner, PROJECT_OWNER);
        for (text, needle) in [
            (
                Some("[meta]\nrequired_version = \"=1.2.4\"\n"),
                "does not admit",
            ),
            (
                Some("[meta]\nrequired_version = \"1.2.3\"\n"),
                "not an exact pin",
            ),
            (
                Some("[meta]\n# required_version = \"=1.2.3\"\n"),
                "no required_version",
            ),
            (
                Some("[meta]\nrequired_version = 7\n"),
                "not a quoted version",
            ),
            (None, "absent"),
        ] {
            let p = at(text, true);
            assert!(!p.met, "{text:?}");
            assert!(p.observed.contains(needle), "{needle}: {}", p.observed);
            assert!(
                p.consequence.contains("never rewritten"),
                "{}",
                p.consequence
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_declared_checker_names_the_exact_path_and_the_failed_property() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = dir.path();
        let rel = "scripts/check.sh";
        let pin = Pin {
            text: Some(PINNED),
            adopted: false,
            producer_version: "1.2.3",
        };
        let at = || {
            find(
                &observe(root, &pin, &params(root, Some(rel))),
                "authored-content checker",
            )
            .clone()
        };
        let observed = |p: &Prerequisite| (p.met, p.observed.clone());
        assert_eq!(observed(&at()), (false, format!("{rel}: absent")));
        std::fs::create_dir_all(root.join("scripts")).unwrap();

        std::fs::create_dir(root.join(rel)).unwrap();
        assert_eq!(
            observed(&at()),
            (false, format!("{rel}: not a regular file"))
        );
        std::fs::remove_dir(root.join(rel)).unwrap();

        let target = outside.path().join("check.sh");
        std::fs::write(&target, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::os::unix::fs::symlink(&target, root.join(rel)).unwrap();
        assert_eq!(
            observed(&at()),
            (false, format!("{rel}: outside the target"))
        );
        std::fs::remove_file(root.join(rel)).unwrap();

        std::os::unix::fs::symlink(root.join("scripts/missing.sh"), root.join(rel)).unwrap();
        assert_eq!(
            observed(&at()),
            (false, format!("{rel}: a symbolic link, not a regular file"))
        );
        std::fs::remove_file(root.join(rel)).unwrap();

        let inside = root.join("scripts/real.sh");
        std::fs::write(&inside, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&inside, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::os::unix::fs::symlink(&inside, root.join(rel)).unwrap();
        assert_eq!(
            observed(&at()),
            (false, format!("{rel}: a symbolic link, not a regular file"))
        );
        std::fs::remove_file(root.join(rel)).unwrap();

        std::fs::write(root.join(rel), "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(root.join(rel), std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(observed(&at()), (false, format!("{rel}: not executable")));

        std::fs::set_permissions(root.join(rel), std::fs::Permissions::from_mode(0o755)).unwrap();
        let p = at();
        assert!(p.met, "{p:?}");
        assert_eq!(p.owner, CHECKER_OWNER);

        let none = observe(root, &pin, &params(root, None));
        assert!(
            !none.iter().any(|p| p.name.contains("checker")),
            "no checker is required when none is declared"
        );
    }

    #[test]
    fn an_unreadable_canonical_root_is_not_reported_as_outside() {
        let path = Ok(PathBuf::from("/target/scripts/check.sh"));
        let root = Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "root cannot be resolved",
        ));
        let error = canonical_location(&path, &root).expect_err("the root did not resolve");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
}
