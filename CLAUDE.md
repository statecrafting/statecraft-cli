# CLAUDE.md: statecraft-cli

**Read [AGENTS.md](AGENTS.md) first.** It is the authority for this repository:
the gate, the backlog protocol, source ownership, approval semantics and the
authored-content rules. This file adds only what is specific to Claude Code and
does not restate it.

## What this repository is right now

A specified corpus with the code it claims. Seven specs, eight crates and one
binary; the product is a local environment for governed agent work; the boundary
is [README.md](README.md), the reasoning is spec `001` sections 3.8 to 3.12, and
what is proposed versus adopted is
[docs/decisions/00-founding-decisions.md](docs/decisions/00-founding-decisions.md).

All seven specs, `000` to `006`, are `approved`. `registry list` is the
authority on the second half: `000` and `001` carry `implementation: n-a`,
because they own prose and no code, and `002` to `006` carry
`implementation: complete`.

**A spec may own more than one crate.** Four pairs were consolidated on
2026-09-21 and no crate moved, so `002`, `004` and `005` own two crates each.
`D-02` is amended to match, in the decision record's section 5, and each merged
spec's section 2 says what separation its two crates hold. The direction that
still holds in both is the one the gate uses: a crate has exactly one owning
spec.

`draft` plus `pending` is schedulable, so `plan` offers a `draft` as ready and
that is spec-spine's lifecycle answer, not the owner's. **Check the `status`
field, not the plan output.** Ratification is the owner's act; see AGENTS.md,
"New sessions".

## Commands

```sh
make tools                 # install the pinned spec-spine into .tooling/bin
make gate                  # the corpus surface: freshness, lint, authored content
make code                  # the workspace surface: build, test, clippy, fmt
make refresh               # spec-spine compile && spec-spine index, after editing a spec.md
make verify SPEC=001       # one spec's declared acceptance
make status                # version, lifecycle counts, what is schedulable
```

`make code` judges eight crates and 683 tests. Both surfaces are required
through the `ci-gate` status check. The guard that made the cargo verbs skip on
an empty workspace is still there and still correct; it simply no longer fires.

`spec-spine index check --fail-on-unresolved` is now in the gate, so **a new
spec that claims a crate before writing it will fail**. That is deliberate, and
AGENTS.md records what to do if a spec genuinely needs to claim ahead.

## Conventions that bite

- **`.statecraft/derived/` is compiler output.** Read it through `spec-spine`
  subcommands only. Never `jq` it, never hand-edit it, and never run a writing
  `compile` to make a freshness check pass. It moved there from `.derived/` with
  spec `002` section 3.19; `.statecraft/` as a whole is committed, and only
  `.statecraft/state/` is ignored.
- **Refresh with the change, not before the check.** After editing any
  `spec.md`, run `make refresh` and commit the shards alongside the edit. Running
  `compile` and then `compile --check` in the same breath passes unconditionally
  and proves nothing.
- **There are no forward claims left, and the gate now refuses one.** Specs `002`
  to `005` once claimed crates that did not exist; all eight crates are written, and
  `index check --fail-on-unresolved` is in the gate. Under the 0.20.0 pin an
  unresolved claim exits **1**, the validation code, not 2: it is a corpus that
  does not describe its tree, and `make refresh` cannot cure it. Do not "fix" one
  by narrowing a spec's territory or by adding an empty crate; see AGENTS.md,
  which records what a spec that genuinely needs to claim ahead should do.
- **This product is the only installer, and there is no kit to coexist with.**
  spec-spine withdrew its kit and its public initializer, so `D-04`'s transition
  contract is superseded and spec `002` section 3.7 is its withdrawal. Section
  3.21 states what replaced it and what three parts of it are retained. The
  harness is one global source delivered by adapters (section 3.14), copied into
  no repository. Sections 3.22 and 3.23 carry the handoff from that withdrawal:
  what the counterparty still holds, the order the last of it moves in, and
  **seven contracts any delivered hook must satisfy**. If you touch
  `harness::shipped()`, read 3.23 first and run
  `cargo test -p statecraft-home --test harness_hooks`, which extracts the
  shipped body and runs it as a program against each contract.
- **The pin is exact, and the binary is local.** `required_version = "=0.20.0"`,
  installed at the gitignored `.tooling/bin` by `make tools`, which reads the
  version from the pin. Run spec-spine through `make` or as
  `.tooling/bin/spec-spine`; a bare `spec-spine` is the shared `~/.cargo/bin`
  copy that any project on this machine replaces. Adopting a newer spine is its
  own change, with its own re-index and its own bypass-floor review (`D-06`).
- **No em dash, no session links.** `make gate` enforces both. This applies to
  commit messages and pull-request bodies too, where the gate cannot see them.
