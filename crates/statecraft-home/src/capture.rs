//! Launching one qualification control, and recording the launch.
//!
//! Spec 002 section 3.30 rule 12, and spec 006 section 3.11.2's
//! `startup capture`. The invocation is bound to its settings by **this**
//! operation: it constructs the argument vector with
//! [`admission::arguments`], writes the payload it passes, resolves and
//! digests the executable, reads its version, launches it under the existing
//! process-group supervisor, and writes one record holding all of it. Nothing
//! a caller supplies becomes part of the invocation except the program, the
//! deadline and the working directory, so no caller can describe an invocation
//! that did not happen.
//!
//! What the record establishes is launcher-attested: that this product started
//! this executable with these arguments and these settings bytes and received
//! these bytes back. It does not establish that the provider loaded the
//! settings, which is what the three controls infer from behavior.

use crate::admission::{
    self, ALLOWED_COMMAND, Capture, Control, Invocation, Launch, Measurement, Origin, ProcessEnd,
    REFUSED_COMMAND, REFUSED_COMMAND_ABSENT_PATH,
};
use statecraft_environment::digest::{digest_bytes, digest_file};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long the version probe may take. A probe is not a session.
pub const PROBE_DEADLINE: Duration = Duration::from_secs(30);

/// The default deadline for one control's session, in seconds.
pub const DEFAULT_DEADLINE_SECONDS: u64 = 300;

/// What a launch needs, all of it stated by the caller.
#[derive(Debug, Clone)]
pub struct Request {
    /// The project the session runs in.
    pub root: PathBuf,
    /// Which control.
    pub control: Control,
    /// Where the record and the raw streams are written.
    pub directory: PathBuf,
    /// The provider, as the operator names it: a name looked up on the
    /// environment's `PATH`, or a path.
    pub program: String,
    /// The session's deadline.
    pub deadline_seconds: u64,
    /// Provider or fake, as the operator states it.
    pub origin: Origin,
    /// The environment the provider runs with, exactly.
    pub environment: BTreeMap<String, String>,
}

/// Why nothing was launched.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// The working directory is not one.
    #[error("{path} is not a directory the session can run in: {detail}")]
    Root {
        /// Where.
        path: String,
        /// Why.
        detail: String,
    },
    /// The refused command's manifest path exists, so it would not fail
    /// harmlessly.
    #[error(
        "{path} exists, and the refused command names a manifest inside it; it has to be absent \
         so that the command fails harmlessly if enforcement does not hold"
    )]
    AbsentPathPresent {
        /// The path.
        path: String,
    },
    /// The capture directory is inside the project.
    #[error(
        "the capture directory {directory} is inside the project {root}; a session could write \
         over its own evidence and its settings, so captures are kept outside it"
    )]
    InsideProject {
        /// The capture directory.
        directory: String,
        /// The project.
        root: String,
    },
    /// A capture happens once.
    #[error("{path} already holds a capture of this control; a launch happens once")]
    Exists {
        /// The record.
        path: String,
    },
    /// The executable could not be resolved.
    #[error("the program `{program}` could not be resolved to an executable: {detail}")]
    Program {
        /// As named.
        program: String,
        /// Why.
        detail: String,
    },
}

/// A launch that could not be carried out or recorded.
#[derive(Debug, thiserror::Error)]
pub enum Failed {
    /// Refused before anything was launched.
    #[error(transparent)]
    Refused(#[from] Refused),
    /// Something nobody asked for went wrong.
    #[error("the launch could not be carried out or recorded: {0}")]
    Io(#[from] std::io::Error),
}

/// What a launch produced.
#[derive(Debug, Clone)]
pub struct Launched {
    /// Where the record was written.
    pub path: PathBuf,
    /// The record.
    pub measurement: Measurement,
    /// Why the launch did not complete as one readable session, when it did
    /// not. `None` says nothing about the control's outcome: a refusal control
    /// that executed its command completed as a launch, and the admission is
    /// what refuses the claim.
    pub incomplete: Option<String>,
}

/// Resolve a program the way a shell would, against the given environment.
fn resolve(program: &str, environment: &BTreeMap<String, String>) -> Result<PathBuf, Refused> {
    let refused = |detail: String| Refused::Program {
        program: program.to_string(),
        detail,
    };
    let candidate = if program.contains('/') {
        PathBuf::from(program)
    } else {
        let path = environment
            .get("PATH")
            .ok_or_else(|| refused("the environment has no PATH to look it up on".into()))?;
        std::env::split_paths(path)
            .map(|dir| dir.join(program))
            .find(|p| p.is_file())
            .ok_or_else(|| refused("not found on PATH".into()))?
    };
    let resolved = candidate
        .canonicalize()
        .map_err(|e| refused(e.to_string()))?;
    if !resolved.is_file() {
        return Err(refused("not a file".into()));
    }
    Ok(resolved)
}

/// Where a path that may not exist yet will resolve: its nearest existing
/// ancestor, canonicalized, with the rest appended.
fn resolved_ahead(path: &Path) -> std::io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut existing = absolute.as_path();
    let mut rest = Vec::new();
    while existing.symlink_metadata().is_err() {
        let Some(parent) = existing.parent() else {
            break;
        };
        if let Some(name) = existing.file_name() {
            rest.push(name.to_os_string());
        }
        existing = parent;
    }
    let mut resolved = existing.canonicalize()?;
    for name in rest.into_iter().rev() {
        resolved.push(name);
    }
    Ok(resolved)
}

/// The version a `--version` probe printed: the first token that starts with
/// a digit, which for the measured provider is `2.1.267` out of
/// `2.1.267 (Claude Code)`.
pub fn version_of(probe: &str) -> Option<String> {
    probe
        .split_whitespace()
        .find(|t| t.starts_with(|c: char| c.is_ascii_digit()))
        .map(str::to_string)
}

fn utf8(bytes: Vec<u8>, name: &str, undecodable: &mut Vec<String>) -> String {
    match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => {
            undecodable.push(name.to_string());
            String::new()
        }
    }
}

/// Launch one control and record it.
///
/// The record is written whether or not the session completed: a timeout, a
/// signal or unreadable output is recorded, and [`Launched::incomplete`] says
/// which. Only a refusal before launch writes nothing.
pub fn launch(request: &Request) -> Result<Launched, Failed> {
    let control = request.control;
    let root = request.root.canonicalize().map_err(|e| Refused::Root {
        path: request.root.display().to_string(),
        detail: e.to_string(),
    })?;
    if !root.is_dir() {
        return Err(Refused::Root {
            path: root.display().to_string(),
            detail: "not a directory".into(),
        }
        .into());
    }
    let absent = root.join(REFUSED_COMMAND_ABSENT_PATH);
    if absent.symlink_metadata().is_ok() {
        return Err(Refused::AbsentPathPresent {
            path: absent.display().to_string(),
        }
        .into());
    }
    // Judged before anything is created: a refusal leaves nothing behind,
    // including an empty directory inside the project it refused to write in.
    let intended = resolved_ahead(&request.directory)?;
    if intended.starts_with(&root) {
        return Err(Refused::InsideProject {
            directory: intended.display().to_string(),
            root: root.display().to_string(),
        }
        .into());
    }
    std::fs::create_dir_all(&request.directory)?;
    let directory = request.directory.canonicalize()?;
    let record = directory.join(admission::record_name(control));
    let stdout_path = directory.join(format!("{}.stdout", control.word()));
    let stderr_path = directory.join(format!("{}.stderr", control.word()));
    let settings_path = directory.join(format!("{}.settings.json", control.word()));
    for path in [&record, &stdout_path, &stderr_path, &settings_path] {
        if path.symlink_metadata().is_ok() {
            return Err(Refused::Exists {
                path: path.display().to_string(),
            }
            .into());
        }
    }
    let program = resolve(&request.program, &request.environment)?;
    let program_digest = digest_file(&program)?.map(|(d, _)| d);

    // The probe. A version read beside the launch, by the same resolved
    // executable, so the record says which binary answered.
    let probed = statecraft_adapter::supervisor::capture(
        &program,
        &["--version"],
        &root,
        &request.environment,
        b"",
        PROBE_DEADLINE,
    )?;
    let probe = String::from_utf8_lossy(&probed.stdout).to_string();
    let probe_version = if probed.code == Some(0) {
        version_of(&probe)
    } else {
        None
    };

    let command = match control {
        Control::Allowed => ALLOWED_COMMAND,
        Control::Refusal | Control::WithoutPayload => REFUSED_COMMAND,
    };
    let payload = crate::session::payload_json();
    let settings_arg = if control.carries_the_payload() {
        crate::settings::write_atomically(&settings_path, &payload)?;
        Some(settings_path.display().to_string())
    } else {
        None
    };
    let arguments = admission::arguments(
        control,
        REFUSED_COMMAND,
        ALLOWED_COMMAND,
        settings_arg.as_deref(),
    );
    let prompt = admission::prompt(command);
    let args: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let captured = statecraft_adapter::supervisor::capture(
        &program,
        &args,
        &root,
        &request.environment,
        prompt.as_bytes(),
        Duration::from_secs(request.deadline_seconds),
    )?;

    let settings_digest_after = settings_arg.as_ref().map(|_| {
        std::fs::read(&settings_path)
            .map(|bytes| digest_bytes(&bytes))
            .unwrap_or_else(|e| format!("unreadable: {e}"))
    });
    std::fs::write(&stdout_path, &captured.stdout)?;
    std::fs::write(&stderr_path, &captured.stderr)?;
    let stdout_digest = digest_bytes(&captured.stdout);
    let stderr_digest = digest_bytes(&captured.stderr);
    let mut undecodable = Vec::new();
    let stdout = utf8(captured.stdout, "stdout", &mut undecodable);
    let stderr = utf8(captured.stderr, "stderr", &mut undecodable);
    let process = ProcessEnd {
        code: captured.code,
        signal: captured.signal,
        timed_out: captured.timed_out,
        surviving_processes: captured.surviving_processes,
    };
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let capture_id = digest_bytes(
        format!(
            "{}|{}|{nanos}|{}|{stdout_digest}",
            control.word(),
            std::process::id(),
            directory.display()
        )
        .as_bytes(),
    );

    let capture = Capture {
        source: stdout_path.display().to_string(),
        bytes: stdout,
    };
    let incomplete = process.unmeasured().or_else(|| {
        if !undecodable.is_empty() {
            return Some(format!("{} was not UTF-8", undecodable.join(" and ")));
        }
        admission::one_session(control, &capture)
            .err()
            .map(|e| e.to_string())
    });
    let measurement = Measurement {
        invocation: Invocation {
            program: program.display().to_string(),
            arguments,
            working_directory: root.display().to_string(),
        },
        settings: settings_arg.as_ref().map(|_| payload.clone()),
        capture,
        launch: Some(Launch {
            capture_id,
            control,
            origin: request.origin,
            command: command.to_string(),
            prompt,
            requested_program: request.program.clone(),
            program_digest,
            probe,
            probe_version,
            settings_path: settings_arg,
            settings_digest_after,
            stderr,
            stdout_digest,
            stderr_digest,
            undecodable,
            process,
            deadline_seconds: request.deadline_seconds,
        }),
    };
    let mut json = serde_json::to_string_pretty(&measurement).map_err(std::io::Error::other)?;
    json.push('\n');
    crate::settings::write_atomically(&record, &json)?;
    Ok(Launched {
        path: record,
        measurement,
        incomplete,
    })
}

/// Where a path would be written, for a caller that has to say so before it
/// launches anything.
pub fn record_path(directory: &Path, control: Control) -> PathBuf {
    directory.join(admission::record_name(control))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_is_the_first_numeric_token_of_the_probe() {
        assert_eq!(
            version_of("2.1.267 (Claude Code)\n").as_deref(),
            Some("2.1.267")
        );
        assert_eq!(version_of("claude 3.0.1").as_deref(), Some("3.0.1"));
        assert_eq!(version_of("no version here"), None);
    }
}
