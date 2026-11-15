# Codex streams

`codex exec --json` streams for the section 3.1 invocation, the prompt on
standard input.

| File | Provenance | Shape | Mapped |
|---|---|---|---|
| `completed.jsonl` | **Recorded** from codex-cli 0.166.0 on macOS, 2026-11-02 | a message, one command item, a message, one `turn.completed` with usage (including `cache_write_input_tokens` and `reasoning_output_tokens`) | `completed`, cost `unknown`, usage provenance `provider` |
| `error.jsonl` | **Recorded** from codex-cli 0.166.0 on macOS, 2026-11-02, with `-c model="statecraft-no-such-model"` added | an `error` item, then an `error` event and `turn.failed` | `interrupted` |
| `turn-failed.jsonl` | **Synthetic**, written by hand | one `turn.failed` with no `error` event | `interrupted`: no recording yet shows a judged `turn.failed` |

`corpus.json` is this table in a form the suite reads: each recording's
scenario, Codex version, platform, invocation, date and redactions, in spec
052's capture shape, and `turn-failed.jsonl` named apart as a constructed
hostile input. The suite refuses a stream named in neither list.

Recordings were made in a scratch git work tree with a per-run `CODEX_HOME`
holding only the credential. Redactions: `thread_id` is replaced with a fixed
value. No field the mapping reads was edited. A `turn.failed` shape that is a
judged result is named here before the mapping may read it as `failed`.
