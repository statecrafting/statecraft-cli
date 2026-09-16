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
}

/// Write the fixture adapter into `dir` and return its path.
///
/// A `sh` script: the suite needs a child process, not a particular language,
/// and every platform this repository's CI runs on has one.
pub fn write(dir: &Path, behavior: Behavior) -> std::io::Result<PathBuf> {
    let path = dir.join("fixture-adapter.sh");
    let body = match behavior {
        Behavior::RefusalThenCompleted => {
            r#"
echo '{"event":"init","applied":["turn-limit"],"adapterVersion":"1.0.0","providerVersion":"fixture"}'
echo '{"event":"refusal","guard":"write-outside-workspace","detail":"refused a write above the workspace"}'
echo '{"event":"result","classification":"completed","cost":null}'
"#
        }
        Behavior::CompletedWithNoCost => {
            r#"
echo '{"event":"init","applied":["turn-limit"],"adapterVersion":"1.0.0","providerVersion":"fixture"}'
echo '{"event":"result","classification":"completed","cost":null}'
"#
        }
        Behavior::MalformedStream => {
            r#"
echo '{"event":"init","applied":["turn-limit"],"adapterVersion":"1.0.0","providerVersion":"fixture"}'
echo 'this line is not an event'
echo '{"event":"result","classification":"completed","cost":null}'
"#
        }
        Behavior::NoResultEvent => {
            r#"
echo '{"event":"init","applied":["turn-limit"],"adapterVersion":"1.0.0","providerVersion":"fixture"}'
echo '{"event":"progress","message":"working"}'
"#
        }
        Behavior::HangsForever => {
            r#"
echo '{"event":"init","applied":["turn-limit"],"adapterVersion":"1.0.0","providerVersion":"fixture"}'
sleep 300 &
sleep 300
"#
        }
        Behavior::AppliesLessThanDeclared => {
            r#"
echo '{"event":"init","applied":[],"adapterVersion":"1.0.0","providerVersion":"fixture"}'
echo '{"event":"result","classification":"completed","cost":null}'
"#
        }
    };

    let mut file = std::fs::File::create(&path)?;
    // Read the prompt off stdin and discard it: the point is that the prompt
    // arrives on a stream, and a fixture that never read it would let a
    // regression to a command-line prompt pass unnoticed.
    write!(file, "#!/bin/sh\ncat > /dev/null\n{body}")?;
    drop(file);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }

    Ok(path)
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
