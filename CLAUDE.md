# CLAUDE.md: statecraft-cli

**Read [AGENTS.md](AGENTS.md) first.** It is the authority for this repository:
the gate, the backlog protocol, source ownership, approval semantics and the
authored-content rules. This file adds only what is specific to Claude Code and
does not restate it.

## What this repository is right now

A specification-only repository. No code, no binary, no test suite. The product
is a local environment for governed agent work; the boundary is
[README.md](README.md), the reasoning is
[docs/design/00-boundaries-and-reuse.md](docs/design/00-boundaries-and-reuse.md),
and what is proposed versus adopted is
[docs/decisions/00-founding-decisions.md](docs/decisions/00-founding-decisions.md).

`spec-spine registry plan` names `002-environment-lifecycle` ready, and since
2026-09-16 that one **is** dispatchable: the owner ratified `001` and `002`.
`003` to `005` are still `draft`, and `plan` will offer `003` the moment `002`
reports complete. That offer is spec-spine's lifecycle answer (`draft` plus
`pending` is schedulable), **not permission to build it**. Ratification is the
owner's act. See AGENTS.md, "New sessions".

## Commands

```sh
make gate                  # the whole check surface: freshness, lint, authored content
make refresh               # spec-spine compile && spec-spine index, after editing a spec.md
make verify SPEC=001       # one spec's declared acceptance
spec-spine registry plan   # what is schedulable
```

There is no build, no `cargo` target and no test runner, because there is no code.
If a task seems to need one, the missing thing is a spec.

## Conventions that bite

- **`.derived/` is compiler output.** Read it through `spec-spine` subcommands
  only. Never `jq` it, never hand-edit it, and never run a writing `compile` to
  make a freshness check pass.
- **Refresh with the change, not before the check.** After editing any
  `spec.md`, run `make refresh` and commit the shards alongside the edit. Running
  `compile` and then `compile --check` in the same breath passes unconditionally
  and proves nothing.
- **A forward claim is expected.** Specs `002` to `005` claim crates that do not
  exist. Those unresolved units are warnings by design; do not "fix" them by
  narrowing a spec's territory or by adding an empty crate.
- **Do not install the spec-spine kit here.** `spec-spine init --with-kit` writes
  a harness this repository deliberately does not carry: who owns the harness is
  an open question (`D-04`), and spec `002` section 3.7 is the contract that keeps
  two installers from claiming the same files.
- **The pin is exact.** `required_version = "=0.18.0"`. 0.19.0 exists and is not
  adopted; adopting it is its own change with its own re-index.
- **No em dash, no session links.** `make gate` enforces both. This applies to
  commit messages and pull-request bodies too, where the gate cannot see them.
