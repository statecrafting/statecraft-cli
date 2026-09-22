//! The fixture adapter, so the negative suite runs with no real provider.
//!
//! Spec 004 section 3.5: "one table every adapter runs, including a fixture
//! adapter that ships with the suite so it can run with no real provider
//! installed."
//!
//! It is a script this crate writes to a temporary path and spawns, not a
//! function call. A fixture that skipped the process boundary would not exercise
//! the thing the suite exists to check: the stream, the deadline and the kill
//! are all properties of a real child.

use std::io::Write;
use std::path::{Path, PathBuf};

/// How the fixture should behave for one suite row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Behavior {
    /// Init, one refusal, then a completed result.
    RefusalThenCompleted,
    /// Init then a completed result, with no cost reported.
    CompletedWithNoCost,
    /// Init, then a line that is not an event at all.
    MalformedStream,
    /// Init, progress, and then the stream simply ends.
    NoResultEvent,
    /// Init, then sleep past any reasonable deadline.
    HangsForever,
    /// Init declaring less than the manifest promised, then a result.
    AppliesLessThanDeclared,
    /// Init, then a progress event carrying the directory the child is actually
    /// running in, and a marker file written there.
    ///
    /// Two observations rather than one, because they answer different
    /// questions: the progress line says where the child *was*, and the marker
    /// says where its writes *landed*. A supervisor that named a working
    /// directory the child never entered would pass the first and fail the
    /// second.
    ReportsWorkingDirectory,
}

/// The file [`Behavior::ReportsWorkingDirectory`] writes into the child's
/// working directory.
///
/// Interpolated into the script rather than spelled twice, so the suite looks
/// for the file the fixture actually writes.
pub const WORKING_DIRECTORY_MARKER: &str = "child-working-directory.txt";

/// Write the fixture adapter into `dir` and return its path.
///
/// A `sh` script: the suite needs a child process, not a particular language,
/// and every platform this repository's CI runs on has one.
///
/// # Why the script is staged and copied rather than written in place
///
/// The suite's rows run as threads in one test binary, and each of them ends by
/// exec'ing the script this function just wrote. Writing the script directly at
/// the path that is about to be exec'd opens a window: a sibling thread that
/// forks while this file is open for writing hands its child a duplicate of
/// that descriptor, and `execve` refuses a file any process holds open for
/// writing (`ETXTBSY`, "text file busy") until that child reaches its own
/// `exec` and `O_CLOEXEC` clears it. Measured as an intermittent Linux CI
/// failure across several unrelated rows of the suite.
///
/// So this process never opens the executed path for writing at all. It writes
/// a staged file it will not exec, and a **child process** copies that file into
/// place. The only descriptor that was ever open for writing on the executed
/// path belonged to a process that has already exited, so no fork of this one
/// can be holding it. Nothing about the product was involved in the race and
/// nothing about it changes here: the fixture is still a real script, still
/// exec'd through the same supervisor path.
pub fn write(dir: &Path, behavior: Behavior) -> std::io::Result<PathBuf> {
    let path = dir.join("fixture-adapter.sh");
    let body: String = match behavior {
        Behavior::RefusalThenCompleted => {
            r#"
echo '{"event":"init","applied":["turn-limit"],"adapterVersion":"1.0.0","providerVersion":"fixture"}'
echo '{"event":"refusal","guard":"write-outside-workspace","detail":"refused a write above the workspace"}'
echo '{"event":"result","classification":"completed","cost":null}'
"#
            .to_string()
        }
        Behavior::CompletedWithNoCost => {
            r#"
echo '{"event":"init","applied":["turn-limit"],"adapterVersion":"1.0.0","providerVersion":"fixture"}'
echo '{"event":"result","classification":"completed","cost":null}'
"#
            .to_string()
        }
        Behavior::MalformedStream => {
            r#"
echo '{"event":"init","applied":["turn-limit"],"adapterVersion":"1.0.0","providerVersion":"fixture"}'
echo 'this line is not an event'
echo '{"event":"result","classification":"completed","cost":null}'
"#
            .to_string()
        }
        Behavior::NoResultEvent => {
            r#"
echo '{"event":"init","applied":["turn-limit"],"adapterVersion":"1.0.0","providerVersion":"fixture"}'
echo '{"event":"progress","message":"working"}'
"#
            .to_string()
        }
        Behavior::HangsForever => {
            r#"
echo '{"event":"init","applied":["turn-limit"],"adapterVersion":"1.0.0","providerVersion":"fixture"}'
sleep 300 &
sleep 300
"#
            .to_string()
        }
        Behavior::AppliesLessThanDeclared => {
            r#"
echo '{"event":"init","applied":[],"adapterVersion":"1.0.0","providerVersion":"fixture"}'
echo '{"event":"result","classification":"completed","cost":null}'
"#
            .to_string()
        }
        // `pwd` is a shell builtin, so this asks nothing of the constructed
        // environment beyond the `sh` the fixture already needs. The marker is
        // written through a relative path deliberately: an absolute one would
        // prove only that a path can be spelled, not that the child is in it.
        Behavior::ReportsWorkingDirectory => format!(
            r#"
echo '{{"event":"init","applied":["turn-limit"],"adapterVersion":"1.0.0","providerVersion":"fixture"}}'
printf '{{"event":"progress","message":"%s"}}\n' "$(pwd)"
pwd > {WORKING_DIRECTORY_MARKER}
echo '{{"event":"result","classification":"completed","cost":null}}'
"#
        ),
    };

    // Staged, and never exec'd: see this function's note on `ETXTBSY`.
    let staged = dir.join("fixture-adapter.staged");
    let mut file = std::fs::File::create(&staged)?;
    // Read the prompt off stdin and discard it: the point is that the prompt
    // arrives on a stream, and a fixture that never read it would let a
    // regression to a command-line prompt pass unnoticed.
    write!(file, "#!/bin/sh\ncat > /dev/null\n{body}")?;
    file.sync_all()?;
    drop(file);

    copy_through_a_child(&staged, &path)?;
    std::fs::remove_file(&staged)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Changing a mode does not open the file for writing, so this adds no
        // window of its own.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }

    Ok(path)
}

/// Install an executable script at `path` without this process ever holding
/// `path` open for writing.
///
/// The race [`write`]'s note describes is not particular to the fixture
/// adapter: any test that writes a script and then execs it, while sibling
/// test threads fork, can hit `ETXTBSY`. This is the same staging and child
/// copy, for any script a test needs. The staged file sits beside `path` and
/// is removed once the copy is in place.
pub fn install_script(path: &Path, contents: impl AsRef<[u8]>, mode: u32) -> std::io::Result<()> {
    let mut staged_name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("a script path needs a file name"))?
        .to_os_string();
    staged_name.push(".staged");
    let staged = path.with_file_name(staged_name);
    let mut file = std::fs::File::create(&staged)?;
    file.write_all(contents.as_ref())?;
    file.sync_all()?;
    drop(file);
    copy_through_a_child(&staged, path)?;
    std::fs::remove_file(&staged)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = mode;
    Ok(())
}

/// Copy `from` to `to` in a child process, so this process never holds a
/// descriptor open for writing on `to`.
///
/// `std::fs::copy` would defeat the purpose: it opens the destination here.
#[cfg(unix)]
fn copy_through_a_child(from: &Path, to: &Path) -> std::io::Result<()> {
    let status = std::process::Command::new("/bin/cp")
        .arg(from)
        .arg(to)
        .status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "/bin/cp {} {} exited {status}",
            from.display(),
            to.display()
        )));
    }
    Ok(())
}

/// Off Unix there is no `ETXTBSY` to avoid and no `/bin/cp` to avoid it with.
#[cfg(not(unix))]
fn copy_through_a_child(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::copy(from, to).map(|_| ())
}

/// The manifest the fixture adapter declares.
pub fn manifest() -> crate::manifest::Manifest {
    crate::manifest::Manifest {
        adapter: "fixture".into(),
        version: "1.0.0".into(),
        supports: vec![
            crate::capability::Capability::TurnLimit,
            crate::capability::Capability::StructuredRefusals,
        ],
        requires_commands: vec!["sh".into()],
    }
}
