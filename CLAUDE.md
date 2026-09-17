# CLAUDE.md: statecraft-cli

**Read [AGENTS.md](AGENTS.md) first.** It is the authority for this repository:
the gate, the backlog protocol, source ownership, approval semantics and the
authored-content rules. This file adds only what is specific to Claude Code and
does not restate it.

## What this repository is right now

A specified corpus with the code it claims. Six crates and one binary; the
product is a local environment for governed agent work; the boundary is
[README.md](README.md), the reasoning is
[docs/design/00-boundaries-and-reuse.md](docs/design/00-boundaries-and-reuse.md),
and what is proposed versus adopted is
[docs/decisions/00-founding-decisions.md](docs/decisions/00-founding-decisions.md).

`000` to `006` are ratified and implemented. `007-shared-evidence-envelope` is
`draft` and implemented, which `plan` offers as ready and which is **not**
permission to treat it as ratified: `draft` plus `pending` is schedulable, and
that is spec-spine's lifecycle answer, not the owner's. Check the `status`
field, not the plan output. Ratification is the owner's act; see AGENTS.md,
"New sessions".

## Commands

```sh
make gate                  # the corpus surface: freshness, lint, authored content
make code                  # the workspace surface: build, test, clippy, fmt
make refresh               # spec-spine compile && spec-spine index, after editing a spec.md
make verify SPEC=001       # one spec's declared acceptance
spec-spine registry plan   # what is schedulable
```

`make code` judges six crates. Both surfaces are required through the `ci-gate`
status check. The guard that made the cargo verbs skip on an empty workspace is
still there and still correct; it simply no longer fires.

`spec-spine index check --fail-on-unresolved` is now in the gate, so **a new
spec that claims a crate before writing it will fail**. That is deliberate, and
AGENTS.md records what to do if a spec genuinely needs to claim ahead.

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
