# Recorded provider streams

Captured from **Claude Code 2.1.267 on darwin, on 2026-09-17**, in a scratch git
work tree, with `claude --print --output-format stream-json --verbose`. These are
the measurement behind spec 008 sections 3.1 to 3.5, committed.

**A fixture is evidence of what the provider emitted on a named version. It is
not evidence that the provider still emits it**, and nothing in this crate claims
otherwise. Spec 008's `## Verification` block says why the acceptance checks the
mapping against these bytes rather than spawning the provider: section 3.6
measured that this provider resolves credentials through the operating-system
keychain, which no check runner has, and spec 005 section 3.2 rule 3 makes a
check that did not run `unknown` and never a pass.

| File | Invocation | What it records |
|---|---|---|
| `success.jsonl` | plain | `subtype: "success"`, `permission_denials: []`, `total_cost_usd: 0.044009`, `num_turns: 1`, exit 0. The `cost-report` basis. |
| `denied.jsonl` | `permissions.deny: ["Bash(echo:*)"]` | One `permission_denials` entry with the tool, the id and the full input, **and** `subtype: "success"`, `is_error: false`, `terminal_reason: "completed"`, exit 0. Section 3.3's load-bearing finding. |
| `max-turns.jsonl` | `--max-turns 1` on a prompt needing three tool calls | `subtype: "error_max_turns"`, `terminal_reason: "max_turns"`, `is_error: true`, exit 1. Section 3.5 maps it `interrupted`, not `failed`. |
| `tool-removed.jsonl` | `--disallowedTools Bash` | `tools` has **89** entries and `Bash` is absent, and `permission_denials` is **empty**. Section 3.4's observable-but-unrecorded mechanism. |
| `api-error.jsonl` | plain, in a constructed environment carrying only `PATH` | `subtype: "success"` with `is_error: true`, `terminal_reason: "api_error"`, `permission_denials: []`, `num_turns: 1`, `total_cost_usd: 0`, `result: "Not logged in"`, exit 1. Section 3.5 maps it `interrupted`, not the completion its subtype claims and not `failed`. |

The two tool counts are the measurement section 3.4 rests on and they are
preserved exactly: 88 entries with `Bash` present under a deny rule, 89 entries
with `Bash` absent under removal. The count going up when a tool is removed is
what the provider did; it is recorded rather than tidied.

`api-error.jsonl` was captured on the same date and the same provider version,
under the environment spec 008 section 3.6 describes: a child given `PATH` alone
cannot reach the operating-system keychain, so the provider terminated
unauthenticated. The authentication is the *cause* and not the finding. What the
fixture records is the **shape** every API-side error of this provider arrives
in, a `success` subtype carrying an error flag, which is why section 3.5 cannot
key its completion row on the subtype alone.

## What was dropped, and what was redacted

These are **selections and redactions of real recordings**, not synthetic JSON,
and both edits are listed here so a reader never has to guess which bytes are
the provider's.

Dropped whole lines, none of which any mapping reads:

- `system` events of subtype `thinking_tokens`, and `rate_limit_event` events;
- `assistant` events whose only content block is `thinking`, because the block
  carries a long opaque signature and nothing else.

Redacted in every line that was kept, because these fixtures are committed to a
public repository and the operator's machine is not part of the measurement:

- `session_id`, `uuid`, `hook_id` and `timestamp` replaced with fixed values;
- `cwd`, `memory_paths` and `messaging_socket_path` replaced with a placeholder
  path, and `plugins` emptied;
- the **names** inside `tools`, `skills`, `agents`, `slash_commands`,
  `terminal_slash_commands` and `mcp_servers` replaced with placeholders that
  **preserve the counts**, except the tool names the measurements rest on
  (`Bash`, `Read`, `Edit`, `Write`, `Glob`, `Grep`, `Task`), which are verbatim.

Everything else is verbatim, including every field spec 008 sections 3.1 to 3.5
rest on: `claude_code_version`, `model`, `permissionMode`, `apiKeySource`,
`subtype`, `is_error`, `terminal_reason`, `stop_reason`, `num_turns`,
`total_cost_usd`, `usage`, `modelUsage` and `permission_denials`.
