---
id: "029-one-resolved-judge"
title: "One resolved judge: every spec-spine call this product makes runs the executable one resolution selected"
status: approved
implementation: pending
created: "2026-09-30"
summary: >
  Every spec-spine invocation this product makes, in every verb and in every
  hook it delivers, runs the executable that one resolution selected for that
  operation, and records which one. No call site names a bare `spec-spine` and
  lets PATH decide. The resolution's sources are, in order, the supervisor's
  path in a managed session, an operator override, the project's declared
  engine as spec-spine's own launcher resolves it when that launcher is
  present, the repository-local install named by the active setup profile, the repository's own
  build and then PATH; every candidate except the supervisor's already-resolved
  path is put to the pin, and each passed over is named. `doctor` reports the resolved executable, never a separate PATH
  probe. Where the resolved engine carries `check --json`, the check is read
  from its envelope instead of from its wording. Amends spec 008 section 3.23
  contracts 2, 4 and 5.
amends:
  # Section 3.23 contracts 2 (resolution order), 4 (reading check's verdict)
  # and 5 (establishing the verb). Under the approved-spec amendment rule, 008
  # stays unedited; registry relationships reports the edge.
  - "008-harness-delivery"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: corrective }
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-environment/" }, nature: corrective }
  - { spec: "003-work-and-run-semantics", unit: { kind: directory, path: "crates/statecraft-run/" }, nature: corrective }
  - { spec: "005-acceptance-and-evidence", unit: { kind: directory, path: "crates/statecraft-acceptance/" }, nature: corrective }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: corrective }
depends_on:
  - "002-environment-lifecycle"
  - "003-work-and-run-semantics"
  - "005-acceptance-and-evidence"
  - "006-command-surface"
  - "008-harness-delivery"
obligations:
  - id: "R-1"
    kind: requirement
    text: "Every spec-spine invocation this product makes runs an executable chosen by the one resolution of section 3.2 for that operation; no call site constructs a bare `spec-spine` command."
    anchor: "3-1-one-judge-per-operation"
  - id: "R-2"
    kind: requirement
    text: "The resolution consults, in order, the supervisor's path, the operator override, the launcher's resolution, the active setup profile's repository-local install, the repository build and PATH; every candidate except the supervisor's already-resolved path is put to the pin, and every candidate passed over is named."
    anchor: "3-2-the-resolution"
  - id: "R-3"
    kind: requirement
    text: "doctor reports the executable the resolution selected, with its rule, version and digest, and never a separately probed PATH binary as the judge."
    anchor: "3-3-what-is-recorded-and-reported"
  - id: "R-4"
    kind: requirement
    text: "When the resolved engine states that `check` carries `--json`, freshness is read from the envelope's outcome and exit code; wording is read only by a bounded legacy reader for engines that do not."
    anchor: "3-4-reading-check"
  - id: "I-1"
    kind: invariant
    text: "Resolution never acquires, installs, downloads or repairs anything, and a missing pinned engine is a refusal naming the remedy, never a fallback to an engine the pin does not admit."
    anchor: "3-5-resolution-never-acquires"
  - id: "V-1"
    kind: verification
    text: "A fixture with conflicting candidates on PATH, at the profile-declared repository-local path and in `target/release` proves every verb that calls spec-spine runs the same selected executable, and that doctor names it."
    anchor: "verification"
    inputs:
      - "crates/statecraft-cli/tests/one_resolved_judge.rs"
      - "crates/statecraft-environment/tests/check_envelope.rs"
      - "crates/statecraft-home/tests/spec_spine_resolution.rs"
      - "crates/statecraft-home/tests/harness_hooks.rs"
---

# 029: One resolved judge

## 1. Purpose

A governed operation is only as trustworthy as the identity of the executable
that judged it. This product selects that executable today in four different
ways, and which one answers depends on the verb.

Measured on `main` at `c0469a3`:

| Where | What runs |
|---|---|
| `crates/statecraft-home/src/spec_spine.rs` | an ordered, pin-checked selection: supervisor path, override, `target/release/spec-spine`, PATH |
| `init` (`manage.rs`) and the supervisor launch (`main.rs`) | that selection |
| `SpecSpineCli::default()` (run report, work selection, closure, coverage), `SpecSpineVerify::default()` (the acceptance suite), `accept`'s delta report and `doctor`'s observed version | a bare `spec-spine`, whichever PATH finds first |
| the generated managed CI commands (`setup.rs`) and this repository's `Makefile` | `.tooling/bin/spec-spine`, which the selection above never consults |

So the executable `make tools` installs at the exact pin is invisible to the
selection, and most verbs never ask the selection at all. Because pins are
exact and spec-spine refuses a pin it does not meet, the usual consequence is a
refusal rather than a wrong verdict. The consequences that remain are real: a
correctly installed engine is refused because PATH named another; `doctor`
reports a binary the hooks do not use; and `accept` folds a refusal envelope
into "no delta report", losing the reason.

The freshness probe (`crates/statecraft-environment/src/probe.rs`) also reads
`check` by exit code plus wording, across two exit tables, although the pinned
engine emits a structured envelope under `check --json`.

This spec makes one resolution the only path to an executable and records what
it chose.

## 2. Territory

This spec owns no product code. Its implementation changes only the units its
`extends` edges name: the selection in `statecraft-home`, the check probe
in `statecraft-environment`, the report source in `statecraft-run`, the suite
source in `statecraft-acceptance`, and the call sites and `doctor` projection
in `statecraft-cli`.

## 3. Behavior

### 3.1 One judge per operation

An operation resolves its executable once, before its first spec-spine call,
and every call it makes uses that absolute path. The types that invoke
spec-spine (the report source, the suite source, the command used by
initialization and the check probe) have no default that names a program: a
caller cannot construct one without a resolved executable. A hook this product
delivers resolves by the same order (section 3.2), so a verb and a hook given
the same inputs choose the same file.

### 3.2 The resolution

The candidates, in order. The first that exists and is admitted answers; a
later one is never consulted once an earlier one answers or refuses.

1. **Supervisor.** In a managed session, the supervisor's resolved path
   (`STATECRAFT_SPEC_SPINE`) is the only candidate. It is not put to the pin:
   its identity is the supervisor's resolution. A path that is not executable
   refuses.
2. **Operator override.** Outside a managed session, a non-empty
   `STATECRAFT_SPEC_SPINE` is the only candidate, put to the pin, with no
   fallback when it is absent or refused.
3. **Launcher.** When spec-spine's own launcher is on PATH and answers its
   resolution query, its answer is the candidate: the engine the project
   declares, as the launcher resolved it, with the rule it used and the
   artifact digest. It is put to the pin like any other. The launcher is
   asked to resolve only; this product then runs the absolute engine path
   itself, so the recorded identity is the executed one.
4. **Repository-local install.** The one install path established by the active
   setup-profile authority. The resolver consumes that declaration rather than
   restating a path, so an engine-location authority change moves the candidate
   without creating a second answer.
5. **Repository build.** The repository's own `target/release/spec-spine`.
6. **PATH.** The first `spec-spine` on PATH that is not the launcher.

Candidate 2, when present outside a managed session, is put to the pin without
fallback. Candidates 3 to 6 are put to the pin in order; the first admitted is
selected, and every one passed over is named with its version and the pin. An
unpinned repository takes the first candidate that exists and is reported
unpinned.

### 3.3 What is recorded and reported

Each resolution produces `{program, rule, version, digest, passedOver}`, where
`digest` is the SHA-256 of the selected file's bytes. Every report this product
writes about a judged operation (the run record, the acceptance receipt, the
initialization report) names that record. `doctor` reports the resolution for
the project, and a `spec-spine` on PATH that differs from it only as
information, never as the judge.

### 3.4 Reading `check`

When the resolved engine states that it carries `check --json` (through its
capabilities document once spec-spine's spec 170 is implemented, and until
then by `check --help` listing `--json`, which the probe already runs to
establish the verb; 0.18.0 is the first release whose `check` takes it, read
from each tag's `cmd_check.rs`), the probe runs `check --json` and reads the
envelope's outcome and exit code. The verdict classes of spec 008 section 3.23 contract 4 are read
from the envelope, never from wording. The wording reader remains only for an
engine that does not carry `--json`, and is bounded to the exit tables it
already knows.

The envelope is classified as follows. `ok` is fresh. A `finding` whose
registry half reports `validationPassed: false` does not validate, and outranks
staleness because regeneration cannot cure it. A `finding` with a half
reporting `fresh: false` is stale. A `finding` with both halves fresh and an
unresolved-unit error counted in the index half's diagnostics is an unresolved
claim. Any other `finding` is a corpus that does not validate, naming the
summary. `refused`, `usage` and `failed` are reads not performed. An envelope
that does not parse, is not `check`'s, or whose `exitCode` disagrees with the
process's establishes nothing.

### 3.5 Resolution never acquires

Resolution reads the filesystem and runs `--version` and the launcher's
resolution query. It never downloads, installs, builds or rewrites an
executable, and never runs a writing verb. When no candidate is admitted, the
operation refuses, naming the pin, every candidate passed over, and the
command that prepares the pinned engine (`make tools`, or the launcher's own
install verb when present).

### 3.6 Observable negative cases

| Case | Required behavior |
|---|---|
| A compatible engine at the profile-declared repository-local path and an incompatible one first on PATH | The repository-local engine answers; the PATH binary is named as passed over |
| `run`, `work list`, `accept`, `verify`, coverage and `doctor` in the same fixture | Each reports the same program and digest |
| An override naming an incompatible engine, with a compatible one on PATH | Refused; nothing falls back |
| A managed session whose supervisor path is not executable | Refused; no other candidate is consulted |
| The launcher answers with an engine the pin does not admit | That candidate is passed over and named |
| No candidate is admitted | Refused, naming the pin, the candidates and the preparation command; nothing is downloaded |
| `check --json` from a supported engine reports stale | Classified stale from the envelope, whatever its wording |

## 4. Out of scope

- The launcher itself, its store and its acquisition policy, which are
  spec-spine's (its draft spec 188).
- The linked producer used for scaffolding, which stays linked (spec-spine
  spec 170 section 3.1); the target-versus-linked guard is spec 018's.
- Moving the install location and changing this repository's `Makefile`, which
  remain a separate authority change.

## 5. Resolved decisions


**2026-09-30: the repository-local candidate follows its owning authority.**
This spec does not restate the install path. The active setup profile supplies
one repository-local location, and the resolver and delivered hooks consume
it. The separate engine-location authority changes that declaration to
`.bin/spec-spine`; after it does, a `.tooling/bin/spec-spine` left in a checkout
is not a candidate. Reading both would keep the retired location alive as a
second answer.

**2026-09-30: `--json` support is read from the verb's help, not from a
version.** The probe already runs `check --help` to establish the verb (008
section 3.23 contract 5), and the help lists `--json` exactly when the engine
carries it. Reading it there keeps no version table in this product, and an
engine built from an untagged checkout answers correctly.

**2026-09-30: the envelope has one reader.** Section 3.4 fixes the classification
and its precedence. The run report (`statecraft-run`) reads `check` through the
same probe, so there is one reader of `check` in this product.

**2026-09-30: one resolution per process.** The command surface keeps the
first selection it makes for a target and hands the same one to every later
question in the process. A process runs one operation, so this is section
3.1's "once per operation", and it is why a verb's steps (the report, the
contract binding, coverage, the suite, the delta report and `doctor`) name one
program and one digest.

**2026-09-30: the launcher's envelope is recognized by its verb.** spec-spine's
launcher names itself `spec-spine-launcher` with the verb `launcher.resolve`
(its spec 188, D-12). An executable on `PATH` is a launcher when it answers
that query with an envelope whose tool is `spec-spine-launcher` or
`spec-spine` and whose verb begins `launcher`; an engine answers the unknown
verb with a usage error and is not one. The query is made with
`SPEC_SPINE_FROZEN=1`, so asking can never download. Only an absolute `path`
is taken as an answer. The delivered hooks read the same envelope with `sed`
and `grep`, because two of them run without `jq`.

**2026-09-30: what `doctor` observes when nothing is selected.** The pin is
compared with the version the resolution itself recorded: the selected
program's, or when the pin admitted none, the first candidate the resolution
passed over. It is never a separate `PATH` probe. The resolution, everything
it passed over, and a differing `spec-spine` on `PATH` are notes.

**2026-09-30: an operator override now reaches the verbs' own reads.** Before
this spec, only initialization honored `STATECRAFT_SPEC_SPINE` outside a
session, and `run`'s own reads went to `PATH`. Under section 3.2 rule 2 the
override is the only candidate for every verb outside a managed session. The
supervisor's selection for a session is unchanged: it reads neither variable,
and a session's hooks receive the supervisor's path.


**2026-09-30: resolve, then execute the absolute path.** Running the launcher
and trusting whatever it executes would record one identity and run another
whenever the two resolutions differed. Asking for the resolution and executing
the returned path keeps the recorded and executed identities the same.

## Verification

Each line is one command. The fixtures are stub engines answering different
versions at the profile-declared repository-local path, in `target/release` and
on PATH, and a stub launcher.

```verify:cli
cargo test -p statecraft-cli --test one_resolved_judge
cargo test -p statecraft-environment --test check_envelope
cargo test -p statecraft-home --test spec_spine_resolution
cargo test -p statecraft-home --test harness_hooks contract_2
```
