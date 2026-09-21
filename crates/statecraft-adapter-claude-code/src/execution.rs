//! Connect the native stream to the existing mapping without teaching the
//! generic protocol a provider's spelling. Spec 004 sections 3.9 to 3.13.

use crate::Invocation;
use crate::outcome::TerminalReading;
use crate::stream::{MapError, ProviderEvent, ResultEvent, map_stream};
use statecraft_adapter::capability::Capability;
use statecraft_adapter::environment::ChildEnvironment;
use statecraft_adapter::protocol::{Classification, Request, StreamError};
use statecraft_adapter::supervisor::{Supervised, supervise_stream};
use statecraft_run::attempt::Outcome;
use std::path::Path;

/// A native execution mapped onto the generic seam, with its terminal evidence.
#[derive(Debug)]
pub struct Execution {
    /// Mapped events and process diagnostics, including a mapped outcome.
    pub supervised: Supervised,
    /// The existing outcome bridge's reading, separate from the provider claim.
    pub terminal: Option<TerminalReading>,
    /// The terminal fields as read, also retained for an unmapped terminal state.
    pub result: Option<ResultEvent>,
    /// A settings cleanup failure, retained beside any terminal evidence.
    pub settings_cleanup_error: Option<String>,
}

impl Execution {
    /// The input to the run supervisor's independent refusal accounting.
    ///
    /// A denied success still claims completion. Replacing the claim with the
    /// corrected refusal would erase that disagreement. Transport errors and
    /// unknown terminal states cannot make the observed outcome completed.
    pub fn termination(&self) -> statecraft_run::session::Termination {
        let adapter_claimed = self.result.as_ref().map_or(Outcome::Interrupted, |result| {
            match crate::outcome::provider_claim(result) {
                Classification::Completed => Outcome::Completed,
                Classification::Failed => Outcome::Failed,
                Classification::Stopped => Outcome::Interrupted,
            }
        });
        statecraft_run::session::Termination {
            observed: self.supervised.outcome,
            adapter_claimed,
        }
    }

    /// Evidence for the outcome record's extensible detail, not a new view of
    /// the command's closed outcome or serialization contract.
    pub fn evidence(&self) -> serde_json::Value {
        let terminal = self.result.as_ref().map(|r| {
            serde_json::json!({
                "subtype": r.subtype,
                "is_error": r.is_error,
                "terminal_reason": r.terminal_reason,
                "stop_reason": r.stop_reason,
                "num_turns": r.num_turns,
                "total_cost_usd": r.total_cost_usd,
                "permission_denials": r.permission_denials,
                "providerClaim": crate::outcome::provider_claim(r),
            })
        });
        serde_json::json!({
            "events": self.supervised.events,
            "providerTerminal": terminal,
            "streamError": self.supervised.stream_error.as_ref().map(ToString::to_string),
            "survivingProcesses": self.supervised.surviving_processes,
            "settingsCleanupError": self.settings_cleanup_error,
        })
    }
}

/// Supervise native JSONL, then use the provider's existing stream and outcome
/// mappings. The generic supervisor alone creates and controls the process.
///
/// The invocation's settings are an additional command-line settings source,
/// using the provider's native precedence and list merging. No settings source
/// or hook is disabled. The private file lives outside the request workspace
/// until supervision returns, and is removed on success, timeout or error.
pub fn supervise(
    invocation: &Invocation,
    request: &Request,
    environment: &ChildEnvironment,
    granted: &[Capability],
) -> std::io::Result<Execution> {
    supervise_in(
        invocation,
        request,
        environment,
        granted,
        &std::env::temp_dir(),
    )
}

fn supervise_in(
    invocation: &Invocation,
    request: &Request,
    environment: &ChildEnvironment,
    granted: &[Capability],
    temporary_root: &Path,
) -> std::io::Result<Execution> {
    // --settings is singular. Do not silently change the precedence of a
    // caller's second document by appending ours or merging it ourselves.
    if invocation
        .args
        .iter()
        .any(|arg| arg == "--settings" || arg.starts_with("--settings="))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invocation.settings owns --settings; a second settings argument is ambiguous",
        ));
    }
    let temporary_root = temporary_root.canonicalize()?;
    if temporary_root.starts_with(request.workspace.canonicalize()?) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "temporary settings storage must be outside the request workspace",
        ));
    }
    // tempfile uses exclusive creation and mode 0600 on Unix. Each attempt
    // owns a distinct file; no path in the target checkout is opened or reused.
    let mut settings = tempfile::Builder::new()
        .prefix("statecraft-settings-")
        .suffix(".json")
        .tempfile_in(temporary_root)?;
    serde_json::to_writer(settings.as_file_mut(), &invocation.settings)?;
    let settings_path = settings.path().to_str().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "temporary settings path is not UTF-8",
        )
    })?;
    let mut args = invocation.args();
    args.extend(["--settings", settings_path]);
    let native = supervise_stream(
        Path::new(&invocation.program),
        &args,
        request,
        environment,
        |line, number| {
            serde_json::from_str(line)
                .map(|event| (number, event))
                .map_err(|e| StreamError::Malformed {
                    line: number,
                    detail: e.to_string(),
                })
        },
        |(_, event)| matches!(event, ProviderEvent::Result(_)),
    )?;
    // Cleanup must not discard a readable terminal denial. Keep a failure as
    // a residual alongside the execution evidence rather than returning early.
    // An already removed file needs no further cleanup.
    let settings_cleanup_error = settings
        .close()
        .err()
        .filter(|error| error.kind() != std::io::ErrorKind::NotFound)
        .map(|error| error.to_string());
    let mut supervised = Supervised {
        events: Vec::new(),
        outcome: native.outcome,
        stream_error: native.stream_error,
        surviving_processes: native.surviving_processes,
    };
    let result_line = native.events.last().map(|(line, _)| *line).unwrap_or(1);
    let events: Vec<_> = native.events.into_iter().map(|(_, event)| event).collect();
    let mut result = events.iter().find_map(|event| match event {
        ProviderEvent::Result(result) => Some((**result).clone()),
        _ => None,
    });
    let mut terminal = None;
    match map_stream(&events, granted) {
        Ok(mapped) => {
            supervised.events = mapped.events;
            result = mapped.result;
            if let Some(r) = &result {
                match crate::outcome(r) {
                    Ok(reading) => {
                        if supervised.outcome == Outcome::Completed {
                            supervised.outcome = reading.outcome;
                        }
                        terminal = Some(reading);
                    }
                    Err(e) => {
                        supervised.stream_error = Some(StreamError::Malformed {
                            line: result_line,
                            detail: e.to_string(),
                        });
                    }
                }
            }
        }
        Err(error) => {
            // Missing initialization cannot erase independently readable
            // terminal denials (004 section 3.11). Preserve their events for
            // the run supervisor's accounting without inventing an init or
            // treating this stream as a completed execution.
            if let Some(result) = &result {
                supervised.events.extend(result.refusal_events());
            }
            // Preserve a transport diagnostic's exact physical line number.
            if supervised.stream_error.is_none() {
                supervised.stream_error = Some(match error {
                    MapError::NoInit => StreamError::NoInit,
                    MapError::Malformed { line, detail } => StreamError::Malformed { line, detail },
                });
            }
        }
    }
    if supervised.stream_error.is_some() {
        supervised.outcome = Outcome::Interrupted;
    }
    Ok(Execution {
        supervised,
        terminal,
        result,
        settings_cleanup_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use statecraft_adapter::capability::Requested;
    use statecraft_adapter::environment::{Blueprint, CheckSuiteCommands, construct};
    use statecraft_adapter::protocol::AttemptIdentity;

    fn request(workspace: &Path) -> Request {
        Request {
            workspace: workspace.to_path_buf(),
            base_commit: "fixture".into(),
            prompt: b"private stdin".to_vec(),
            capabilities: Requested::none(),
            deadline_seconds: 1,
            attempt: AttemptIdentity {
                run_id: "settings-cleanup".into(),
                number: 1,
            },
        }
    }

    #[test]
    fn settings_are_removed_when_the_invocations_program_cannot_spawn() {
        let workspace = tempfile::tempdir().unwrap();
        let temporary_root = tempfile::tempdir().unwrap();
        let invocation = Invocation::new(
            workspace.path().join("absent-program").to_str().unwrap(),
            &["Bash(echo:*)".into()],
            None,
        );
        let environment = construct(&Blueprint::empty(), &CheckSuiteCommands::default());
        let error = supervise_in(
            &invocation,
            &request(workspace.path()),
            &environment,
            &[],
            temporary_root.path(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        assert_eq!(std::fs::read_dir(temporary_root.path()).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(workspace.path()).unwrap().count(), 0);
    }

    #[test]
    fn duplicate_settings_arguments_refuse_instead_of_changing_precedence() {
        let workspace = tempfile::tempdir().unwrap();
        let temporary_root = tempfile::tempdir().unwrap();
        let environment = construct(&Blueprint::empty(), &CheckSuiteCommands::default());
        for args in [
            vec!["--settings", "other.json"],
            vec!["--settings=other.json"],
        ] {
            let mut invocation = Invocation::new("unused", &[], None);
            invocation.args.extend(args.into_iter().map(str::to_string));
            let error = supervise_in(
                &invocation,
                &request(workspace.path()),
                &environment,
                &[],
                temporary_root.path(),
            )
            .unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
            assert!(error.to_string().contains("second settings argument"));
        }
        assert_eq!(std::fs::read_dir(temporary_root.path()).unwrap().count(), 0);
    }

    // A stdout read failure reaches the outcome, the stream error and the
    // evidence through the provider's own mapping, without being reported as a
    // stream that simply ended. The failure is produced by a line that is not
    // valid UTF-8, so a real child raises it and no seam is needed here.
    #[cfg(unix)]
    #[test]
    fn an_unreadable_stream_is_interrupted_and_says_why() {
        use std::os::unix::fs::PermissionsExt;

        let workspace = tempfile::tempdir().unwrap();
        let temporary_root = tempfile::tempdir().unwrap();
        let child = workspace.path().join("fixture.sh");
        std::fs::write(
            &child,
            concat!(
                "#!/bin/sh\n",
                "/bin/cat > /dev/null\n",
                r#"echo '{"type":"system","subtype":"init","claude_code_version":"fixture"}'"#,
                "\n",
                "printf 'x\\377\\376y\\n'\n",
                "exit 0\n",
            ),
        )
        .unwrap();
        std::fs::set_permissions(&child, std::fs::Permissions::from_mode(0o700)).unwrap();

        let invocation = Invocation::new(child.to_str().unwrap(), &[], None);
        let environment = construct(&Blueprint::empty(), &CheckSuiteCommands::default());
        // This row names its own deadline. The deadline is not what is under
        // test here; the fixture has to reach its unreadable line for the read
        // failure to exist at all, and on a loaded machine the shared one
        // second expires first, which reports the absent init instead.
        let mut request = request(workspace.path());
        request.deadline_seconds = 30;
        let execution = supervise_in(
            &invocation,
            &request,
            &environment,
            &[],
            temporary_root.path(),
        )
        .unwrap();

        match &execution.supervised.stream_error {
            Some(StreamError::ReadFailed { detail, .. }) => {
                assert!(
                    detail.contains("while reading the event stream"),
                    "{detail}"
                );
            }
            other => panic!("expected a read failure, not a stream that ended: {other:?}"),
        }
        assert_eq!(execution.supervised.outcome, Outcome::Interrupted);
        // The provider's claim is absent here and is reported as absent, not
        // as a completion the supervisor could not read.
        assert!(execution.result.is_none());
        assert_eq!(execution.termination().observed, Outcome::Interrupted);
        let evidence = execution.evidence();
        assert!(
            evidence["streamError"]
                .as_str()
                .unwrap()
                .contains("could not read the event stream"),
            "{evidence}"
        );
        // Cleanup still runs on the transport failure.
        assert_eq!(std::fs::read_dir(temporary_root.path()).unwrap().count(), 0);
    }

    #[test]
    fn temporary_storage_in_the_workspace_is_refused_before_writing() {
        let workspace = tempfile::tempdir().unwrap();
        let invocation = Invocation::new("unused", &[], None);
        let environment = construct(&Blueprint::empty(), &CheckSuiteCommands::default());
        let error = supervise_in(
            &invocation,
            &request(workspace.path()),
            &environment,
            &[],
            workspace.path(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("outside the request workspace"));
        assert_eq!(std::fs::read_dir(workspace.path()).unwrap().count(), 0);
    }
}
