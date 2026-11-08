# Codex capability probes

The recorded probes behind this adapter's spec 047 declaration
(`src/capabilities.rs`, spec 055 section 3.6). Recorded from codex-cli 0.166.0
on macOS, 2026-11-08, on the owner's ChatGPT sign-in.

Each probe ran `codex` with an environment holding only `HOME`, `PATH` and
`CODEX_HOME`. `HOME` was an empty scratch directory, and `CODEX_HOME` was a
per-run directory (mode `0700`) holding the credential (`auth.json`, mode
`0600`), a `hooks.json` and, where named, a `config.toml`. The workspace was a
scratch git work tree. The turn ran with section 3.1's invocation.

| File | Probe | What it shows |
|---|---|---|
| `strict-config.txt` | `codex exec --strict-config` with `config.toml` holding one unknown key | Codex reads the settings document at the path the supervisor wrote, and names that path in its refusal. |
| `per-run-home.jsonl` | Section 3.1's turn, with `config.toml` setting `model_reasoning_effort = "low"` | The turn completes on the per-run directory alone. |
| `hook-block.jsonl` | Section 3.1's turn with `--dangerously-bypass-hook-trust` added, and a `PreToolUse` command hook that exits 2 | The stream holds no event for the blocked action. The two `error` items warn about the bypass flag. |
| `hook-block.stderr.txt` | The same run's standard error, one line | Codex ran the hook and blocked the command. It reported this only on standard error. |
| `gate-input.json` | What that hook read on standard input | `hook_event_name` `PreToolUse`, `tool_name` `Bash`, the command. |

The trust bypass was used for this measurement only. The adapter never passes
it, and section 3.4 still requires the gate hook to be managed.

Outside these files, the probe also found:

- After the per-run turn, the run's thread id appeared under the per-run
  `CODEX_HOME` (`sessions/`, `thread_history_1.sqlite`, `state_5.sqlite-wal`)
  and in no file under the user's `~/.codex`, text or binary.
- The scratch `HOME` was still empty, and the credential was byte-identical
  afterwards.
- The session record carried `reasoning_effort` `low`, the value from the
  supplied `config.toml`.

Redactions: scratch paths become `/scratch/codex-home`, `/scratch/workspace`
and `/scratch/gate`. Thread, turn and tool-use ids become fixed values. Nothing
else was edited.
