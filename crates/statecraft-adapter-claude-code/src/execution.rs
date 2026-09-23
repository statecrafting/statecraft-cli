//! Connect the native stream to the existing mapping without teaching the
//! generic protocol a provider's spelling. Spec 004 sections 3.9 to 3.13.

use crate::Invocation;
use crate::outcome::TerminalReading;
use crate::stream::{MapError, ProviderEvent, ResultEvent, map_stream};
use statecraft_adapter::capability::Capability;
use statecraft_adapter::environment::ChildEnvironment;
use statecraft_adapter::protocol::{Classification, Request, StreamError};
use statecraft_adapter::supervisor::{Supervised, Unwatched, Watch, supervise_watched};
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
    /// Every `hook_response` the stream carried, in the order read.
    ///
    /// Spec 004 section 5, 2026-09-22: handed over for spec 002 section 3.31,
    /// which judges them. Nothing here reads what a hook printed.
    pub hook_responses: Vec<HookResponse>,
    /// The session id the init event reported, where one was read.
    pub session_id: Option<String>,
    /// The exact bytes the settings file held when the process was spawned,
    /// read back from that file after it was written. What this adapter
    /// supplied, as opposed to what it was asked to supply.
    pub settings_written: Vec<u8>,
}

/// One hook's reported response, as the provider streamed it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookResponse {
    /// The session the event names.
    pub session_id: Option<String>,
    /// The hook event that fired it, for example `SessionStart`.
    pub hook_event: Option<String>,
    /// The hook's name as the provider reports it, for example
    /// `SessionStart:startup`.
    pub hook_name: Option<String>,
    /// Its exit code.
    pub exit_code: Option<i64>,
    /// The provider's own outcome word for it.
    pub outcome: Option<String>,
    /// Its standard output, verbatim.
    pub stdout: Option<String>,
}

impl HookResponse {
    /// Read one native event as a hook's reported response, where it is one.
    ///
    /// Spec 004 section 5, 2026-09-22: a caller watching a launch sees native
    /// events, and this is the one reading of the provider's spelling it needs,
    /// kept in the adapter that owns those bytes.
    pub fn of(event: &ProviderEvent) -> Option<Self> {
        match event {
            ProviderEvent::System(s) if s.is_hook_response() => Some(Self {
                session_id: s.session_id.clone(),
                hook_event: s.hook_event.clone(),
                hook_name: s.hook_name.clone(),
                exit_code: s.exit_code,
                outcome: s.outcome.clone(),
                stdout: s.stdout.clone(),
            }),
            _ => None,
        }
    }
}

/// The session id an init event reports, where the event is one.
pub fn init_session(event: &ProviderEvent) -> Option<&str> {
    match event {
        ProviderEvent::System(s) if s.is_init() => s.session_id.as_deref(),
        _ => None,
    }
}

/// Whether the event is the init event.
pub fn is_init(event: &ProviderEvent) -> bool {
    matches!(event, ProviderEvent::System(s) if s.is_init())
}

/// Whether the event is a `SessionStart` hook's start or response: the events
/// that precede a session's first turn in the recorded streams.
pub fn is_session_start_hook(event: &ProviderEvent) -> bool {
    matches!(event, ProviderEvent::System(s)
        if s.is_hook() && s.hook_event.as_deref() == Some("SessionStart"))
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
    supervise_watched_in(
        invocation,
        request,
        environment,
        granted,
        &std::env::temp_dir(),
        &mut Unwatched,
    )
}

/// [`supervise`], with the caller's watch told of the spawn before the prompt
/// is delivered and shown each native event, numbered, as it is read (spec 004
/// section 5, 2026-09-22).
pub fn supervise_with(
    invocation: &Invocation,
    request: &Request,
    environment: &ChildEnvironment,
    granted: &[Capability],
    watch: &mut dyn Watch<(usize, ProviderEvent)>,
) -> std::io::Result<Execution> {
    supervise_watched_in(
        invocation,
        request,
        environment,
        granted,
        &std::env::temp_dir(),
        watch,
    )
}

#[cfg(test)]
fn supervise_in(
    invocation: &Invocation,
    request: &Request,
    environment: &ChildEnvironment,
    granted: &[Capability],
    temporary_root: &Path,
) -> std::io::Result<Execution> {
    supervise_watched_in(
        invocation,
        request,
        environment,
        granted,
        temporary_root,
        &mut Unwatched,
    )
}

fn supervise_watched_in(
    invocation: &Invocation,
    request: &Request,
    environment: &ChildEnvironment,
    granted: &[Capability],
    temporary_root: &Path,
    watch: &mut dyn Watch<(usize, ProviderEvent)>,
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
    match &invocation.settings_document {
        Some(document) => {
            use std::io::Write;
            settings.as_file_mut().write_all(document.as_bytes())?;
        }
        None => serde_json::to_writer(settings.as_file_mut(), &invocation.settings)?,
    }
    settings.as_file_mut().sync_all()?;
    // Read back rather than remembered: what the spawned process can read is
    // what is in the file, and that is what the record says was supplied.
    let settings_written = std::fs::read(settings.path())?;
    let settings_path = settings.path().to_str().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "temporary settings path is not UTF-8",
        )
    })?;
    let mut args = invocation.args();
    args.extend(["--settings", settings_path]);
    let native = supervise_watched(
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
        watch,
    )?;
    // Cleanup must not discard a readable terminal denial. Keep a failure as
    // a residual alongside the execution evidence rather than returning early.
    // An already removed file needs no further cleanup.
    let settings_cleanup_error = settings
        .close()
        .err()
        .filter(|error| error.kind() != std::io::ErrorKind::NotFound)
        .map(|error| error.to_string());
    let mut execution = map_native(native, granted, settings_cleanup_error);
    execution.settings_written = settings_written;
    Ok(execution)
}

/// Map what the supervisor read onto an execution.
///
/// A function of the supervised events and outcome and nothing else, so what
/// an interruption keeps is decided here and is testable without racing a
/// deadline: whatever the supervisor had read when it stopped is mapped, and an
/// interrupted supervision stays interrupted whatever the provider claimed.
fn map_native(
    native: Supervised<(usize, ProviderEvent)>,
    granted: &[Capability],
    settings_cleanup_error: Option<String>,
) -> Execution {
    let mut supervised = Supervised {
        events: Vec::new(),
        outcome: native.outcome,
        stream_error: native.stream_error,
        surviving_processes: native.surviving_processes,
        stopped: native.stopped,
        timed_out: native.timed_out,
    };
    let result_line = native.events.last().map(|(line, _)| *line).unwrap_or(1);
    let events: Vec<_> = native.events.into_iter().map(|(_, event)| event).collect();
    let mut hook_responses = Vec::new();
    let mut session_id = None;
    for event in &events {
        if session_id.is_none() {
            session_id = init_session(event).map(str::to_string);
        }
        hook_responses.extend(HookResponse::of(event));
    }
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
    Execution {
        supervised,
        terminal,
        result,
        settings_cleanup_error,
        hook_responses,
        session_id,
        settings_written: Vec::new(),
    }
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

    /// A terminal denial the supervisor read before its deadline survives the
    /// interruption as structured evidence, and the interruption is not
    /// overwritten by the provider's claim of completion.
    ///
    /// This is the retention half of spec 004 section 3.5 case 3, measured
    /// where it is decided. The supervision is constructed rather than timed,
    /// so the test does not depend on a child being scheduled inside a
    /// deadline, which the product does not promise.
    #[test]
    fn an_interruption_keeps_the_terminal_denial_the_supervisor_read() {
        let denial = serde_json::json!({"tool_name": "Bash", "tool_use_id": "t1",
            "tool_input": {"command": "hang"}});
        let lines = [
            serde_json::json!({"type":"system","subtype":"init","claude_code_version":"fixture"}),
            serde_json::json!({"type":"result","subtype":"success","num_turns":2,
                "permission_denials":[denial]}),
        ];
        let native = Supervised {
            events: lines
                .iter()
                .enumerate()
                .map(|(i, l)| (i + 1, serde_json::from_value(l.clone()).unwrap()))
                .collect(),
            outcome: Outcome::Interrupted,
            stream_error: None,
            surviving_processes: None,
            stopped: None,
            timed_out: false,
        };
        let execution = map_native(native, &[], None);
        assert_eq!(execution.supervised.outcome, Outcome::Interrupted);
        let refusals = statecraft_adapter::protocol::refusals(&execution.supervised.events);
        assert_eq!(refusals.len(), 1);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&refusals[0].detail).unwrap(),
            denial
        );
        assert_eq!(
            execution.evidence()["providerTerminal"]["permission_denials"],
            serde_json::json!([denial])
        );
        assert_eq!(execution.termination().adapter_claimed, Outcome::Completed);
        assert_eq!(execution.termination().observed, Outcome::Interrupted);
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
        let workspace = tempfile::tempdir().unwrap();
        let temporary_root = tempfile::tempdir().unwrap();
        let child = workspace.path().join("fixture.sh");
        statecraft_adapter::fixture::install_script(
            &child,
            concat!(
                "#!/bin/sh\n",
                "/bin/cat > /dev/null\n",
                r#"echo '{"type":"system","subtype":"init","claude_code_version":"fixture"}'"#,
                "\n",
                "printf 'x\\377\\376y\\n'\n",
                "exit 0\n",
            ),
            0o700,
        )
        .unwrap();

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

    /// The settings file is removed when supervision ends at its deadline.
    ///
    /// The temporary root is the test's own, so the check needs nothing from
    /// the child: whether or not it was scheduled before the deadline, the file
    /// this crate wrote is gone afterwards.
    #[cfg(unix)]
    #[test]
    fn settings_are_removed_when_supervision_times_out() {
        let workspace = tempfile::tempdir().unwrap();
        let temporary_root = tempfile::tempdir().unwrap();
        let child = workspace.path().join("hang.sh");
        statecraft_adapter::fixture::install_script(
            &child,
            "#!/bin/sh\nexec /bin/sleep 300\n",
            0o700,
        )
        .unwrap();
        let invocation = Invocation::new(child.to_str().unwrap(), &["Bash(x:*)".into()], None);
        let environment = construct(&Blueprint::empty(), &CheckSuiteCommands::default());
        let execution = supervise_in(
            &invocation,
            &request(workspace.path()),
            &environment,
            &[],
            temporary_root.path(),
        )
        .unwrap();
        assert_eq!(execution.supervised.outcome, Outcome::Interrupted);
        assert!(execution.settings_cleanup_error.is_none());
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

    /// Spec 004 section 5, 2026-09-22: the recorded 2.1.267 stream's hook
    /// response reaches the caller with its output, exit code and session, and
    /// the init event's session id beside it. Nothing about the mapping moves.
    #[test]
    fn hook_responses_and_the_session_id_are_handed_over_unread() {
        let text = include_str!("../testdata/stream/success.jsonl");
        let native = Supervised {
            events: crate::stream::read_jsonl(text)
                .unwrap()
                .into_iter()
                .enumerate()
                .map(|(i, e)| (i + 1, e))
                .collect(),
            outcome: Outcome::Completed,
            stream_error: None,
            surviving_processes: None,
            stopped: None,
            timed_out: false,
        };
        let execution = map_native(native, &[], None);
        assert_eq!(
            execution.session_id.as_deref(),
            Some("11111111-1111-1111-1111-111111111111")
        );
        assert_eq!(execution.hook_responses.len(), 1);
        let hook = &execution.hook_responses[0];
        assert_eq!(hook.hook_event.as_deref(), Some("SessionStart"));
        assert_eq!(hook.hook_name.as_deref(), Some("SessionStart:startup"));
        assert_eq!(hook.exit_code, Some(0));
        assert_eq!(hook.outcome.as_deref(), Some("success"));
        assert_eq!(hook.session_id, execution.session_id);
        assert!(hook.stdout.as_deref().unwrap().contains("sessionTitle"));
        assert_eq!(execution.supervised.outcome, Outcome::Completed);
    }

    /// What the settings file held at the spawn is what the execution says was
    /// written, byte for byte, including formatting a parse would lose.
    #[test]
    fn the_settings_bytes_written_are_read_back_and_handed_over() {
        let workspace = tempfile::tempdir().unwrap();
        let temporary_root = tempfile::tempdir().unwrap();
        let child = workspace.path().join("fixture.sh");
        statecraft_adapter::fixture::install_script(
            &child,
            concat!(
                "#!/bin/sh\n",
                "/bin/cat > /dev/null\n",
                r#"echo '{"type":"system","subtype":"init","claude_code_version":"fixture","session_id":"s"}'"#,
                "\n",
                r#"echo '{"type":"result","subtype":"success","is_error":false,"num_turns":1,"permission_denials":[],"session_id":"s"}'"#,
                "\n",
            ),
            0o700,
        )
        .unwrap();
        let rules = ["Bash(cargo publish*)".to_string()];
        let invocation = Invocation::new(child.to_str().unwrap(), &rules, None);
        let document = format!(
            "{}\n",
            serde_json::to_string_pretty(&invocation.settings).unwrap()
        );
        let invocation = invocation.with_settings_document(document.clone()).unwrap();
        let environment = construct(&Blueprint::empty(), &CheckSuiteCommands::default());
        let mut request = request(workspace.path());
        request.deadline_seconds = 60;
        let execution = supervise_in(
            &invocation,
            &request,
            &environment,
            &[],
            temporary_root.path(),
        )
        .unwrap();
        assert_eq!(execution.settings_written, document.as_bytes());
        assert_eq!(execution.session_id.as_deref(), Some("s"));
        assert!(execution.hook_responses.is_empty());
    }

    /// A watch sees native events in order, can stop at the init event, and
    /// the execution keeps the startup hook's response and says why it
    /// stopped. Nothing after the stop is read or mapped as a completion.
    #[test]
    fn a_watch_stops_at_init_and_the_startup_evidence_before_it_survives() {
        use statecraft_adapter::supervisor::Control;
        struct AtInit(Vec<bool>);
        impl Watch<(usize, ProviderEvent)> for AtInit {
            fn spawned(&mut self, _pid: u32) -> Result<(), String> {
                Ok(())
            }
            fn event(&mut self, (_, event): &(usize, ProviderEvent)) -> Control {
                self.0.push(is_session_start_hook(event));
                if is_init(event) {
                    Control::Stop("decided at init".into())
                } else {
                    Control::Continue
                }
            }
        }

        let workspace = tempfile::tempdir().unwrap();
        let temporary_root = tempfile::tempdir().unwrap();
        let child = workspace.path().join("fixture.sh");
        statecraft_adapter::fixture::install_script(
            &child,
            concat!(
                "#!/bin/sh\n",
                "/bin/cat > /dev/null\n",
                r#"echo '{"type":"system","subtype":"hook_response","hook_event":"SessionStart","hook_name":"SessionStart:startup","stdout":"ack","exit_code":0,"session_id":"s"}'"#,
                "\n",
                r#"echo '{"type":"system","subtype":"init","claude_code_version":"fixture","session_id":"s"}'"#,
                "\n",
                "sleep 30\n",
                r#"echo '{"type":"result","subtype":"success","is_error":false,"num_turns":1,"permission_denials":[],"session_id":"s"}'"#,
                "\n",
            ),
            0o700,
        )
        .unwrap();
        let invocation = Invocation::new(child.to_str().unwrap(), &[], None);
        let environment = construct(&Blueprint::empty(), &CheckSuiteCommands::default());
        let mut request = request(workspace.path());
        request.deadline_seconds = 60;
        let mut watch = AtInit(Vec::new());
        let started = std::time::Instant::now();
        let execution = supervise_watched_in(
            &invocation,
            &request,
            &environment,
            &[],
            temporary_root.path(),
            &mut watch,
        )
        .unwrap();
        assert!(started.elapsed() < std::time::Duration::from_secs(20));
        assert_eq!(watch.0, [true, false]);
        assert_eq!(
            execution.supervised.stopped.as_deref(),
            Some("decided at init")
        );
        assert_eq!(execution.supervised.outcome, Outcome::Interrupted);
        assert!(execution.result.is_none());
        assert_eq!(execution.session_id.as_deref(), Some("s"));
        assert_eq!(execution.hook_responses.len(), 1);
        assert_eq!(execution.hook_responses[0].stdout.as_deref(), Some("ack"));
    }
}
