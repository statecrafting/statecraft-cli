//! Section 3.4: the gate hook, asked of Codex before the turn.
//!
//! Before the turn the adapter runs `codex app-server`, sends `initialize`,
//! `initialized` and one `hooks/list` for the workspace (no thread, turn or
//! model request), and refuses unless this product's gate is listed as a
//! `PreToolUse` command hook that is synchronous, enabled and `managed`, with
//! an unrestricted matcher. `managed` and not merely trusted is the
//! 2026-10-31 decision in section 5: the adapter never writes a trust entry,
//! and the operator installs the managed policy once.
//!
//! The query runs in the attempt's own constructed environment, through the
//! seam's supervisor: the same per-run `CODEX_HOME`, working directory and
//! executable the turn will use. An environment whose `CODEX_HOME` is not the
//! attempt's refuses before anything is spawned, so the hooks listed are the
//! hooks that turn will run.
//!
//! **The operator's one-time installation.** Codex trusts a hook either by
//! a trust hash it keeps itself or by an administrator-managed policy. The
//! operator installs the managed policy once, outside any run, registering
//! the gate command as a synchronous `PreToolUse` command hook with no
//! matcher; no verb of this product performs it, and the adapter reads only
//! whether Codex then reports the gate as `managed` for the per-run
//! `CODEX_HOME`.
//!
//! **The response's field spelling is synthetic** (`testdata/hooks/README.md`)
//! until the owner records a real `hooks/list` answer. A response this module
//! cannot read refuses; nothing here reads an unreadable answer as admitted.

use crate::home::{CODEX_HOME, RunHome};
use serde::{Deserialize, Serialize};
use statecraft_adapter::environment::ChildEnvironment;
use statecraft_adapter::protocol::{Request, StreamError};
use statecraft_adapter::supervisor::{Unwatched, supervise_watched};
use std::path::Path;

/// The JSON-RPC id of the `hooks/list` request: its answer is the terminal line.
pub const QUERY_ID: u64 = 1;

/// The longest the query may take, whatever the attempt's own deadline.
pub const QUERY_DEADLINE_SECONDS: u64 = 30;

/// One hook as `hooks/list` reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListedHook {
    /// The hook event, `PreToolUse` for the gate.
    pub event_name: String,
    /// The handler type, `command` for the gate.
    pub handler_type: String,
    /// The command it runs.
    #[serde(default)]
    pub command: Option<String>,
    /// The tool matcher; absent, empty or `*` is unrestricted.
    #[serde(default)]
    pub matcher: Option<String>,
    /// Whether Codex runs it without waiting for its answer.
    #[serde(rename = "async")]
    pub asynchronous: bool,
    /// Whether it is enabled.
    pub enabled: bool,
    /// Where its trust comes from; the gate needs `managed`.
    pub trust_status: String,
}

/// The gate hook, admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admitted {
    /// The listing that admitted it.
    pub hook: ListedHook,
}

/// Why the attempt refuses before the turn. Each carries a stable code.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HookRefusal {
    /// The query's environment is not the attempt's.
    #[error("codex-hook-query-environment: {detail}")]
    QueryEnvironment {
        /// What differed.
        detail: String,
    },
    /// The query could not be made or was not answered.
    #[error("codex-hook-query-failed: {detail}")]
    QueryFailed {
        /// Why.
        detail: String,
    },
    /// The answer was not a readable hook list.
    #[error("codex-hook-list-unreadable: {detail}")]
    Unreadable {
        /// Why.
        detail: String,
    },
    /// The gate is not listed as a `PreToolUse` command hook.
    #[error("codex-gate-hook-absent: the gate is not a listed PreToolUse command hook")]
    Absent,
    /// Listed, and disabled.
    #[error("codex-gate-hook-disabled: the gate hook is listed and not enabled")]
    Disabled,
    /// Listed, and asynchronous: Codex would not wait for its answer.
    #[error("codex-gate-hook-asynchronous: the gate hook is not synchronous")]
    Asynchronous,
    /// Listed, and restricted to some tools.
    #[error("codex-gate-hook-matcher-restricted: the gate hook matches only `{matcher}`")]
    MatcherRestricted {
        /// The matcher.
        matcher: String,
    },
    /// Listed, and its trust is not `managed`.
    #[error("codex-gate-hook-not-managed: the gate hook's trust is `{trust}`, not `managed`")]
    NotManaged {
        /// The trust Codex reported.
        trust: String,
    },
}

/// The bytes written to `codex app-server`'s standard input.
pub fn query_input(workspace: &Path) -> Vec<u8> {
    let lines = [
        serde_json::json!({"id": 0, "method": "initialize", "params": {
            "clientInfo": {"name": "statecraft-cli", "version": env!("CARGO_PKG_VERSION")}}}),
        serde_json::json!({"method": "initialized"}),
        serde_json::json!({"id": QUERY_ID, "method": "hooks/list",
            "params": {"cwd": workspace.to_string_lossy()}}),
    ];
    let mut input = Vec::new();
    for line in lines {
        input.extend_from_slice(line.to_string().as_bytes());
        input.push(b'\n');
    }
    input
}

fn check(hook: &ListedHook) -> Result<(), HookRefusal> {
    if !hook.enabled {
        return Err(HookRefusal::Disabled);
    }
    if hook.asynchronous {
        return Err(HookRefusal::Asynchronous);
    }
    if let Some(m) = hook
        .matcher
        .as_deref()
        .filter(|m| !m.is_empty() && *m != "*")
    {
        return Err(HookRefusal::MatcherRestricted {
            matcher: m.to_string(),
        });
    }
    if hook.trust_status != "managed" {
        return Err(HookRefusal::NotManaged {
            trust: hook.trust_status.clone(),
        });
    }
    Ok(())
}

/// Judge one `hooks/list` answer for `gate_command`.
///
/// # Errors
///
/// A [`HookRefusal`]; when several listings name the gate and none admits it,
/// the first listing's reason.
pub fn judge(response: &serde_json::Value, gate_command: &str) -> Result<Admitted, HookRefusal> {
    if let Some(error) = response.get("error") {
        return Err(HookRefusal::QueryFailed {
            detail: error.to_string(),
        });
    }
    let hooks: Vec<ListedHook> = response
        .pointer("/result/hooks")
        .cloned()
        .ok_or_else(|| HookRefusal::Unreadable {
            detail: "no `result.hooks`".into(),
        })
        .and_then(|v| {
            serde_json::from_value(v).map_err(|e| HookRefusal::Unreadable {
                detail: e.to_string(),
            })
        })?;
    let mut first = None;
    for hook in hooks.into_iter().filter(|h| {
        h.event_name == "PreToolUse"
            && h.handler_type == "command"
            && h.command.as_deref() == Some(gate_command)
    }) {
        match check(&hook) {
            Ok(()) => return Ok(Admitted { hook }),
            Err(refusal) => {
                first.get_or_insert(refusal);
            }
        }
    }
    Err(first.unwrap_or(HookRefusal::Absent))
}

/// Ask Codex for its effective hooks in the attempt's own environment, and
/// judge the answer.
///
/// # Errors
///
/// A [`HookRefusal`]. One for the environment is returned before any spawn.
pub fn query(
    program: &Path,
    home: &RunHome,
    environment: &ChildEnvironment,
    attempt: &Request,
    gate_command: &str,
) -> Result<Admitted, HookRefusal> {
    let expected = home.path().to_string_lossy();
    if environment.variables.get(CODEX_HOME).map(String::as_str) != Some(&*expected) {
        return Err(HookRefusal::QueryEnvironment {
            detail: format!("{CODEX_HOME} is not the attempt's per-run directory {expected}"),
        });
    }
    let request = Request {
        prompt: query_input(&attempt.workspace),
        context: None,
        documentation: None,
        deadline_seconds: attempt.deadline_seconds.min(QUERY_DEADLINE_SECONDS),
        ..attempt.clone()
    };
    let supervised = supervise_watched(
        program,
        &["app-server"],
        &request,
        environment,
        |line: &str, number: usize| {
            serde_json::from_str::<serde_json::Value>(line).map_err(|e| StreamError::Malformed {
                line: number,
                detail: e.to_string(),
            })
        },
        |v: &serde_json::Value| v.get("id").and_then(serde_json::Value::as_u64) == Some(QUERY_ID),
        &mut Unwatched,
    )
    .map_err(|e| HookRefusal::QueryFailed {
        detail: e.to_string(),
    })?;
    let answer = supervised
        .events
        .iter()
        .find(|v| v.get("id").and_then(serde_json::Value::as_u64) == Some(QUERY_ID));
    match answer {
        Some(answer) if !supervised.timed_out => judge(answer, gate_command),
        _ => Err(HookRefusal::QueryFailed {
            detail: match supervised.stream_error {
                Some(e) => e.to_string(),
                None => "no `hooks/list` answer before the deadline".into(),
            },
        }),
    }
}
