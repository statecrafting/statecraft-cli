# Codex `hooks/list` answers: SYNTHETIC

**Nothing in this directory is a recording.** Each file is one JSON-RPC answer
to the `hooks/list` request `src/hooks.rs` sends to `codex app-server`
(spec 055 section 3.4), written by hand. The field names (`eventName`,
`handlerType`, `command`, `matcher`, `async`, `enabled`, `trustStatus`) are an
assumption about the shape, not a measurement.

| File | Shape | Judged |
|---|---|---|
| `managed.json` | the gate as a synchronous, enabled, `managed` PreToolUse command hook with no matcher, beside an unrelated hook | admitted |
| `untrusted.json` | the gate listed with trust `untrusted` | refused, `codex-gate-hook-not-managed` |

The owner replaces both with answers recorded from a named Codex version: one
with the managed policy of section 5 (2026-10-31) installed and one without.
If the recorded field names differ, `ListedHook` follows the recording; until
then an answer it cannot read refuses with `codex-hook-list-unreadable`.
