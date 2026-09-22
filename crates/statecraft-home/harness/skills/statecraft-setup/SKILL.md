---
name: statecraft-setup
description: "One-time contributor setup: install the pinned spec-spine, the stack toolchain AGENTS.md names, fetch the base ref, and verify the governed loop once, so /prime can report lifecycle and structural counts. Applies only inside a Statecraft project: a repository holding `.statecraft/environment.json`. Outside one, ignore this file entirely."
allowed-tools: Bash, Read
---

> Applies only inside a Statecraft project: a repository holding `.statecraft/environment.json`. Outside one, ignore this file entirely.
# /setup

Get a fresh clone operational. After this completes, `/prime` can report
lifecycle and structural counts through `spec-spine`, never by ad-hoc
parsing of `.statecraft/derived/**/*.json` (`AGENTS.md` "Governed artifact reads").

## Process

### 1. Install the pinned spec-spine

`AGENTS.md` pins the version and names the invocation (a binary on `PATH`,
a package wrapper, or an in-tree build). CI runs the pinned one, so a local
pass on another version proves nothing: a different version is a halt.
Any one route:

```sh
cargo install spec-spine-cli --version <pin> --locked   # with a Rust toolchain
npm i -g spec-spine@<pin>                               # prebuilt binary, no toolchain
uvx spec-spine@<pin> --version                          # Python, ephemeral
```

Verify: `spec-spine --version` prints the pin. When `AGENTS.md` says the
binary is built from this checkout, build it as it says instead and use
that path for every command below.

### 2. Stack toolchain

Install whatever `AGENTS.md` (or `CLAUDE.md`) names for the language
gates: a pinned toolchain file, a package manager, optional linters. Report
each as present or absent; a missing optional tool is a note, a missing
required one is a halt. `jq` is a convenience the hooks in
`.claude/settings.json` use; each hook says what it skipped when `jq` is
absent.

### 3. Fetch the base ref

The coupling gate diffs against the default branch on the remote:

```sh
git fetch origin main
```

### 4. Verify the governed loop

Run the gate exactly as `AGENTS.md` "Working the backlog" lists it under
"Run the gate before every commit". The governance floor is:

```sh
# The gate, exactly as this project's AGENTS.md lists it. Which verbs it
# runs, which flags they carry, and whether a build or test suite joins
# them are facts about this project, and AGENTS.md is where they are
# stated. A generic skill that listed them would be right in one
# repository and quietly wrong in every other.
```

then the stack's own build, tests, and lints. On a clean checkout
`compile` and `index` are deterministic no-ops; if
`git status --short -- .statecraft/derived/` shows a diff afterwards, the committed
shards were stale. Say so and leave the diff for the session to commit
(`chore(derived): ...`); do not hide it. Halt on the first failing step
and surface its output verbatim.

Then the reads `/prime` will use:

```sh
spec-spine registry status-report --json --nonzero-only
spec-spine registry plan
spec-spine index coverage
```

### 5. Emit summary

```text
## setup: <project>

**spec-spine:** {<pin> / wrong version <v> / failed at <step>}
**Toolchain:** {<what AGENTS.md names>: present / absent}
**Optional tools:** {name: present/absent, ...}
**Governed loop:**
  - compile: {ok / failed}
  - index: {ok / regenerated, shards left for the session to commit}
  - lint: {clean / N diagnostics}
  - check: {registry fresh / stale, index fresh / stale}
  - couple: {clean / drift surfaced}
  - coverage: {N claimed, M unclaimed / not enforced}
  - stack gate: {ok / failed at <command> / none declared}
**Lifecycle:** {N specs across <statuses>}  (from registry status-report)
**Ready:** {ids / (nothing ready)}  (from registry plan)

Next: run `/prime` to load full session context.
```

Do not invent counts. Only report values that came back from a
`spec-spine` subcommand or a gate command.

## Rules

- Halt on first failure. Do not silently continue past a missing
  prerequisite or a failing gate.
- Never parse `.statecraft/derived/**/*.json` directly; use the `spec-spine`
  subcommands.
- Idempotent: safe to re-run.

## Project layer

Read from `AGENTS.md`: the version pin, the binary invocation, the
toolchain, the gate command list. Nothing here is edited per project.
