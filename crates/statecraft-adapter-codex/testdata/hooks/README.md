# Codex `hooks/list` answers

Each file is one JSON-RPC answer to the `hooks/list` request `src/hooks.rs`
sends to `codex app-server` (spec 055 section 3.4).

| File | Provenance | Shape | Judged |
|---|---|---|---|
| `untrusted.json` | **Recorded** from codex-cli 0.166.0 on macOS, 2026-11-02 | the gate as a synchronous, enabled `preToolUse` command hook with no matcher, from a per-run `CODEX_HOME`'s `hooks.json`, `isManaged: false`, `trustStatus: untrusted` | refused, `codex-gate-hook-not-managed` |
| `managed.json` | **Derived**: `untrusted.json` with the gate's `isManaged` set to `true`, beside an unrelated asynchronous `postToolUse` hook | the gate as managed | admitted |

The recording was made with the per-run `CODEX_HOME` holding only the hooks
document and the credential, in a scratch git work tree. Redactions: the
scratch paths are replaced with `/scratch/workspace` and
`/scratch/codex-home`, and the gate's path with
`/statecraft/home/harness/hooks/gate`; `currentHash` is left as recorded, so
it is the hash of the unredacted command. No field `src/hooks.rs` reads was
edited.

`managed.json` stays derived until the operator's one-time managed policy is
installed and an answer is recorded with it (section 5, 2026-10-31): that
installation is an administrator act on the machine, which no verb performs.
Until then `isManaged` is the field the recording shows Codex reports, and
nothing else about a managed answer is assumed.
