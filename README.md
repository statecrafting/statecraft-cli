# statecraft-cli

A local environment for **governed agent work**: register a repository, prepare
an isolated workspace, supervise an agent through one unit of work, judge the
result independently, and keep a reviewable account of what happened.

It runs on one machine, on one repository, with no account and no hosted service.

## Status: specified, implemented and tested, not released

Every verb the specs name is bound, and `cargo run -p statecraft-cli -- --help`
prints each one beside the spec it answers to:

```
project register   project list   project arm   project disarm
project enroll   project unenroll
env plan   env apply   env upgrade   env remove   doctor
work list   work show   run   run list   run show   accept
home show   home plan   home apply
init plan   init apply   migrate plan   migrate apply
config show   approval grant   approval show
harness show   harness upgrade   session payload
startup record   startup capture   startup qualify   startup show   startup trial
```

Seven specs, `000` to `006`, all `approved`; eight crates and one binary.
`make code` runs the whole workspace suite and `make gate` requires every
source file to be specifically claimed by a spec, so neither a test count nor a
coverage figure is restated here: run them. Every spec that claims code has
built it, and no forward claim is outstanding.

Seven specs and eight crates, because **a spec may own more than one crate**.
The corpus was eleven specs until 2026-09-21, when four pairs that each
described one subject across two documents were consolidated: the work, run and
accept bindings into `006`, the shared evidence envelope into `005`, the first
provider adapter into `004`, and the managed environment into `002`. No crate
was merged, renamed or deleted and no requirement changed; `D-02` is amended to
match, and a crate still has exactly one owning spec.

`000` to `006` were ratified between 2026-09-16 and 2026-09-21, and the
constitution's product principles VI to XIII were ratified on 2026-09-16, three
of them frozen as spec `000` anchors.

Nothing is installed or released: the workspace is at version `0.0.0` with
`publish = false`, and `F-02` defers publication of any kind. The way to run it
is from a checkout.

The language and the layout are decided: Rust, one Cargo workspace, one owning
spec per crate. The packaging, the distribution and the binary's
name are still **recommendations awaiting the owner's decision**, recorded with
what has been adopted in
[docs/decisions/00-founding-decisions.md](docs/decisions/00-founding-decisions.md).

This repository distinguishes four claims and makes them separately: *specified*,
*implemented*, *tested*, *released*. Today `000` to `006` are specified and
approved; `002` to `006` are additionally implemented and tested by
`cargo test --workspace`, except what only a live provider session can
establish, and `registry list` is the authority on each spec's implementation
state. The machine-checkable rows of each spec's
observable-negative-cases table are carried by tests named after them; the rows
those tables state as refused in review are review obligations, and no test is
claimed for them. **No managed session has been live-qualified**: every
qualification path is exercised against local fakes, which the records mark
synthetic. **Nothing is released**, and `F-02` defers publication.

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
[spec 001](specs/001-boundaries-and-authority/spec.md), sections 3.8 to 3.12.

Deliberately deferred, by name rather than by omission: hosted platform
selection, publication of any kind, signing and key custody, any user interface,
adaptive autonomy, cost and quota control, breadth across providers, and
scheduling across repositories.

## The first workflow, as it is actually invoked

One repository, one work item, one adapter, one workspace, one independent
inspection, one reviewable outcome. Publication is not part of it (`F-02`).

Every verb below takes the **target path** as its first argument, and every verb
accepts `--json`. Run from a checkout, because nothing is installed:

```sh
cargo run -p statecraft-cli -- project register <path>   # a verdict with reasons; nothing written inside
cargo run -p statecraft-cli -- project arm      <path>   # consent to the target being driven
cargo run -p statecraft-cli -- env plan         <path>   # what would be managed, and what is withheld
cargo run -p statecraft-cli -- env apply        <path>   # managed bytes, plus the pins, in a committed manifest
cargo run -p statecraft-cli -- work list        <path>   # the ready set, read from spec-spine's report
cargo run -p statecraft-cli -- run              <path> <spec-id>  # isolated worktree, supervised session, counted refusals
cargo run -p statecraft-cli -- accept           <path> <run-id>   # the suite at the trusted base; a receipt, or none
cargo run -p statecraft-cli -- run show         <path> <run-id>   # one account, every value naming its record
```

`doctor`, `project list`, `project disarm`, `work show`, `run list`,
`env upgrade` and `env remove` complete the surface.

### What each step needs before it will do anything

| Prerequisite | Which verbs | What happens without it |
|---|---|---|
| A **registered** target | every verb except `project register` and `project list` | Refused (2), naming the path. |
| An **armed** target | `run`, and only `run` | Refused (2), naming `project arm <path>`. Discovery and inspection read an unarmed target, which is what registering one is for. |
| A bare `spec-spine` resolvable on this process's `PATH`, and a corpus in the target that compiles | `work list`, `work show`, `run` | A finding (1) naming the target and what spec-spine said. Readiness is read from `registry plan` and `registry list`, never computed here and never read from `.statecraft/derived/`. The binary invoked is whatever `spec-spine` resolves to, not this repository's pinned `.tooling/bin` copy. |
| The provider adapter's three prerequisites: a resolvable `claude` executable, the credential path, and a **qualification record** for the pair (this adapter's build, that provider version) under `<product home>/qualifications.json` | `env plan`, `env apply`, `env upgrade`, `doctor` | The adapter **refuses to claim its paths and names which one is absent**, and `doctor` reports the finding. `env apply` still runs: it reports `applied: 0 path(s) written` and exits 0, writing **no managed byte** while still creating the manifest at `.statecraft/environment.json` with the pins and an empty `entries` list. The withheld managed paths and the recorded pins are two different writes, and only the first is withheld. A missing record does not stop `run` either: an unqualified adapter still runs, and is labelled `unqualified` in the posture, the attempt record and the outcome. |

The product's own state lives outside every target, at `$STATECRAFT_HOME` or
`~/.statecraft` by default. That is what makes `project register` write nothing
inside the repository it registers.

What the slice must make observably true, stated as refusals rather than as
assertions, is
[spec 001](specs/001-boundaries-and-authority/spec.md) section 3.11.

### What a managed run records at its start

A **managed** run is one in a project holding `.statecraft/environment.json`.
The route to one, from a checkout:

```sh
cargo run -p statecraft-cli -- home apply                 # the product home and its harness revision
cargo run -p statecraft-cli -- init apply      <path>     # the project area and its manifest
cargo run -p statecraft-cli -- harness upgrade <path>     # commit the required harness revision, explicitly
cargo run -p statecraft-cli -- run             <path> <spec-id>
cargo run -p statecraft-cli -- startup show    <path> <run-id> [--attempt <n>]
```

`run` refuses before any attempt when the required revision is missing,
corrupt or unreadable (spec `002` section 3.25). Otherwise it writes up to four
write-once records per attempt under
`.statecraft/state/startup/runs/<run>/<attempt>/` (spec `002` sections 3.31 and
3.32): `intent.json` before a spawn is attempted, `launched.json` once the spawn
returned a process and before the prompt is delivered, `admission.json` at the
startup decision, and `record.json` at the end. `startup show` reads them back
and answers, from their bytes:

| Question | Where the answer comes from |
|---|---|
| which revision was **required** | the committed manifest, full digest |
| which was **selected** | the run's own choice, the required revision once it is verified intact |
| what was **supplied** | the settings document the run passed with `--settings`, by digest: the deny floor plus, when a revision is selected, that revision's `SessionStart` hook and the attempt's admission gate, registered for this session only; the floor's own digest is recorded beside it |
| which was **correlated** | one acknowledgment in a `SessionStart` response of this attempt's own stream that carries this attempt's binding and names an installed revision; it does not establish which process printed it, so "executed" is never claimed |
| whether work was **admitted** | the startup decision, made at the first event after the startup hooks: tool calls wait at the gate until it says `admitted`, and a refusal stops the process |
| whether a process was **created** | `launched.json`; an intent alone is `launch-unknown`, and a confirmed spawn with no record is `outcome-unknown` |
| why it is **not qualified** | the verdict and its reasons |

Six words stay apart: **installed**, **selected**, **supplied**, **correlated**,
**admitted** and **qualified**, and a run cannot reach the last: a run session
is not one of the three qualification controls. A provider that ignores the
registration runs neither the acknowledgment nor the gate, so the decision
refuses and the process is stopped, and the refusal says effects before the stop
are not excluded. What is still unobserved is whether the live provider honors
hooks passed through `--settings` in `--print` mode; until a live run shows it,
a live managed run with a requirement is expected to be refused as
`not-admitted`. An attempt whose outcome is unknown stays live, and `run`
refuses the next attempt and names what to inspect rather than replaying it.

`startup trial <path> (--provider-session | --synthetic)` is the one verb that
asks that question (spec `002` section 3.33): one run attempt through the same
launch, a read-only sentinel, one session and one version probe, spent once per
project, judged `established`, `not-established` or `uncertain`, with the hook
evidence and the effects before admission each named. The acceptance script's
`managed-startup` stage runs it under its own approval. The permission
experiment does not answer it: that stage starts no run and supplies no hook.

### The contract a run was authorized against

Where the producer resolves context closures (spec-spine's specs 106 and 107,
released in 0.23.0), `run` asks it for one per attempt, the spec and every obligation
the spec declares, and writes the answer into the attempt's intent before any
effect (spec `003` section 3.1.3). `accept` asks for the same closure again and
compares (spec `005` section 3.18): a contract that changed, lost a member or
withdrew an obligation is no acceptance, reason `contract-moved`, naming each
member with both identities. A target whose pinned producer has no closures
(before spec-spine 0.23.0) binds `unsupported` and compares `not-recorded`.
The receipt is unchanged either way.

### The exit codes a caller scripts against

`0` did what was asked. `1` ran and reports a **finding**. `2` **refused**: a
precondition was not met and nothing was done. `3` the arguments name no verb.
`4` a **failure** nobody asked for. Spec `006` section 3.3 owns the vocabulary
and a test keeps the set closed.

## Governance

Governed by [spec-spine](https://github.com/statecrafting/spec-spine), pinned to
**0.23.0** exactly in `spec-spine.toml`. Specs are the source of truth; the
derived shards under `.statecraft/derived/` are compiler output, committed, and read only
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
make code        # read-only: build, test, clippy, fmt across the eight crates
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
[spec 001](specs/001-boundaries-and-authority/spec.md) section 3.9.

## License

Apache-2.0 (see [LICENSE](LICENSE)).
