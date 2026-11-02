# Codex streams: SYNTHETIC

**Nothing in this directory is a recording.** These lines were written by hand
in the shape spec 055 section 3.2 names (`thread.started`, `turn.completed`
with `usage`, `turn.failed`, `error`), for `codex exec --json` as invoked in
section 3.1, so the mapping and spec 004's suite have bytes to run against
before a capture exists. They are not evidence of what any Codex version
emits, and they qualify nothing (section 3.5).

| File | Shape | Mapped |
|---|---|---|
| `completed.jsonl` | one thread, one command item, one message, one `turn.completed` with usage | `completed`, cost `unknown`, usage provenance `provider` |
| `turn-failed.jsonl` | one `turn.failed` | `interrupted`: no fixture yet shows a judged `turn.failed` |
| `error.jsonl` | an `error` event, then `turn.failed` | `interrupted` |

## Replacing them with a recording

The owner records each file from a named Codex version, in a scratch git work
tree, with the section 3.1 invocation and the prompt on standard input, and
keeps the file names. Redactions, if any, are listed here: `thread_id`
replaced with a fixed value, and paths inside item payloads replaced with a
placeholder. No field the mapping reads is edited. The recorded version is
then written into `MEASURED_PROVIDER_VERSION` and into the qualification
record, and a `turn.failed` shape that is a judged result is named here before
the mapping may read it as `failed`.
