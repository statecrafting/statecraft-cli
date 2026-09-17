# statecraft-cli

A local environment for **governed agent work**: register a repository, prepare
an isolated workspace, supervise an agent through one unit of work, judge the
result independently, and keep a reviewable account of what happened.

It runs on one machine, on one repository, with no account and no hosted service.

## Status: specified and implemented, not released

`cargo run -p statecraft-cli -- project register <path>` records a repository and
prints its verdict with reasons. The four `project` verbs work. The `env` verbs
and `doctor` are bound and **refuse with exit 2**, because they need a configured
adapter set and no spec ratifies a provider adapter yet. The `work`, `run` and
`accept` verbs **do not exist**: spec `006` section 3.1 admits a verb only by the
change that implements the behavior behind it, through an `extends` edge from the
spec that owns it, so `003`, `004` and `005` are libraries with no command
surface.

Nothing is installed or released: `F-02` defers publication, so the way to run
it is from a checkout.

`000` to `007` are approved, and the constitution's product principles VI to
XIII were ratified on 2026-09-16, three of them frozen as spec 000 anchors.
`007-shared-evidence-envelope` was ratified on 2026-09-17. Every spec that
claims code has built it: six crates, no forward claim outstanding, and
`spec-spine registry plan` reports nothing schedulable.

The language and the layout are decided: Rust, one Cargo workspace, crates
matching the spec boundaries. The packaging, the distribution and the first
interaction mode are still **recommendations awaiting the owner's decision**,
recorded with what has been adopted in
[docs/decisions/00-founding-decisions.md](docs/decisions/00-founding-decisions.md).

This repository distinguishes four claims and makes them separately: *specified*,
*implemented*, *tested*, *released*. Today `000` to `007` are specified, and
`002` to `007` are additionally implemented and tested: six crates, 320 tests,
and every row of every spec's observable-negative-cases table covered by one
test named after the row. **Nothing is released**, and `F-02` defers publication.

## The idea

An agent's report that it finished is a **claim**. This product exists to turn
claims into judgments:

- **Acceptance is independent.** The suite runs here, over identified candidate
  bytes, from instructions read at a trusted base revision. A zero exit code is
  not read as success where a structured report exists, and a check that did not
  run is `unknown`, never a pass.
- **A candidate cannot enlarge its own authority.** Policy, verifier, hooks and
  acceptance instructions are read at the base, never from the candidate. A
  change to them is an authority change, decided by a human.
- **Refusals outlive the process that was refused.** The supervisor counts them
  from the adapter's event stream, where the supervised process cannot reach them.
  A refusal followed by a completed turn is evidence, not silence.
- **Protection claims name their mechanism.** Prompts and worktrees are not
  security boundaries against hostile code, and nothing here says otherwise. Each
  claim names what enforces it and what it leaves open.

The full set is the constitution's principles VI to XIII:
[standards/spec/constitution.md](standards/spec/constitution.md).

## Boundaries

This product owns registration and onboarding, the managed working environment,
workspace preparation, supervision, run state and recovery, independent
acceptance, and the reviewable outcome.

It does not own, and will not reimplement:

| Capability | Owner |
|---|---|
| Specification semantics, compilation, ownership, freshness | [spec-spine](https://github.com/statecrafting/spec-spine); this product consumes its commands and reads its structured reports |
| Application knowledge, recall, coordination | aicortex; no dependency in the first slice, and repository files stay readable without it |
| Service chassis, identity, cell enforcement | Rahi; not a local daemon, not a sandbox, not a UI framework |
| Hash-linked records, canonical bytes, check composition | `attest-ledger`, `canonical-keysort-json`, `action-gate` |

The reasoning, the actual-versus-proposed dependency split and the reuse
dispositions are in
[docs/design/00-boundaries-and-reuse.md](docs/design/00-boundaries-and-reuse.md).

Deliberately deferred, by name rather than by omission: hosted platform
selection, publication of any kind, signing and key custody, any user interface,
adaptive autonomy, cost and quota control, breadth across providers, and
scheduling across repositories.

## The first proposed workflow

One repository, one work item, one adapter, one workspace, one independent
inspection, one reviewable outcome. Publication is not part of it.

```
statecraft project register <path>    # a qualification verdict, nothing written inside
statecraft env apply                 # managed bytes, recorded in a committed manifest
statecraft work list                 # the ready set, read from spec-spine's report
statecraft run start --spec NNN       # isolated worktree, supervised session, counted refusals
statecraft accept --run <id>          # the suite at the trusted base; a receipt, or none
statecraft run show <id>              # one account, every value naming its record
```

Two of these verbs are bound today: `project register` works, and `env apply`
refuses with exit 2 until a provider adapter is ratified. The other four do not
exist. They are the slice specs `002` to `005` describe and spec `006` binds,
with its acceptance stated as refusals in
[docs/design/00-boundaries-and-reuse.md](docs/design/00-boundaries-and-reuse.md#4-the-bounded-first-vertical-slice).

## Governance

Governed by [spec-spine](https://github.com/statecrafting/spec-spine), pinned to
**0.20.0** exactly in `spec-spine.toml`. Specs are the source of truth; the
derived shards under `.derived/` are compiler output, committed, and read only
through `spec-spine` subcommands.

The binary is installed **into this repository**, at the gitignored
`.tooling/bin`, and `make` prefers it over anything on `PATH`. A shared
`~/.cargo/bin/spec-spine` is one binary for every project on the machine, so
whichever project built it last governs all of them; a local copy cannot be
replaced by another project's work. `make tools` reads the exact version from
`required_version`, so the pin is the only place the number is written.

```sh
make tools       # install the pinned spec-spine into .tooling/bin
make gate        # read-only: freshness, lint, coverage, the authored-content rules
make code        # read-only: build, test, clippy, fmt across the six crates
make refresh     # writing: recompute the committed shard trees
make verify SPEC=001
```

`make gate` judges the corpus and `make code` judges the workspace; CI requires
both through the single `ci-gate` status check. See [AGENTS.md](AGENTS.md) for
the working protocol.

## The predecessor

An earlier `statecraft-cli` reached 76 specs and a partly implemented product. Its
implementation and history are preserved elsewhere and are **not** restored here:
this repository recovers specific measured failures, contracts and fixtures from
it, and leaves its hosted client, its scheduler and its user interface behind.
What was recovered and what was not is recorded in
[docs/design/00-boundaries-and-reuse.md](docs/design/00-boundaries-and-reuse.md#2-what-the-archive-is-used-for-and-what-it-is-not).

## License

Apache-2.0 (see [LICENSE](LICENSE)).
