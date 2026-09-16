---
id: "003-work-and-run-semantics"
title: "Work selection, workspace preparation, the run record, and recovery after interruption"
status: draft
implementation: pending
created: "2026-09-16"
summary: >
  The core loop's semantics. How a unit of work is identified by reading
  spec-spine's structured report rather than re-deriving it; how an isolated
  workspace is prepared so the operator's checkout is never edited; the
  append-only hash-linked run record with intent written before an effect and
  outcome after it; the closed outcome set (completed, failed, refused,
  interrupted, cancelled) and the rule that a retry appends an attempt rather
  than replacing history; supervisor-owned refusal accounting a supervised
  process cannot erase; and reconciliation of an ambiguous effect before any
  retry, with no exactly-once promise.
establishes:
  - { kind: directory, path: "crates/statecraft-run/" }
depends_on:
  - "000-bootstrap"
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
---

# 003: Work selection, workspace preparation, the run record, and recovery

## 1. Purpose

Everything this product claims rests on a record that survives the process that
wrote it. This spec fixes that record and the state machine over it, before any
adapter (`004`) or acceptance (`005`) can reference either.

Three specific mistakes are refused here by construction. Re-deriving readiness
instead of reading spec-spine's report, which produces a second compiler with a
subtly different answer. Working in the operator's checkout, which makes a
concurrent edit indistinguishable from an agent's change. And trusting the
supervised process's own account of whether it was refused, which is what
constitution IX exists to prevent.

## 2. Territory

`crates/statecraft-run/` (forward claim; unresolved until implemented).

## 3. Behavior

### 3.1 A unit of work

In the first slice, a unit of work is **one spec in a registered repository's
corpus that spec-spine's own report names as ready**.

Readiness is read, never computed here: the product invokes spec-spine's
supported commands and parses their **structured output**. It does not read
`.derived/` directly, does not reimplement dependency resolution, and does not
infer readiness from an exit code when a structured report is available.
`work list` prints the set with the report field each row came from.

If the installed spec-spine's report does not carry a fact the product needs,
that is a **refusal with a named missing field**, not a locally reconstructed
substitute. An upstream gap is reported to spec-spine as a finding; it is not
patched here.

### 3.1.1 Ready is not ratified

spec-spine's readiness answer and this product's scheduling decision are
different questions, and conflating them would let this product build a spec its
owner never agreed to.

Measured against spec-spine 0.18.0 on 2026-09-16: `registry plan` offers a spec
whose `status` is `draft` and whose `implementation` is `pending`, because that
combination is schedulable in the lifecycle table. `draft` withholds
ratification, not schedulability.

A **lifecycle policy** therefore decides which statuses a repository schedules.
Its default admits only `approved`. A ready spec the policy excludes is **listed
with the reason it was excluded**, never silently dropped and never scheduled.

**The policy belongs to the target repository, not to this product's state.**
spec-spine's design note 06 puts repository lifecycle policy in the adopting
repository's column (its section 2 boundary table) and proposes that the shared
build skill **ask the repository** whether a work order is eligible rather than
embed `status == approved` (its section 3.6). If this product kept the answer in
its own registry, two tools would answer "may this draft build" differently, and
the one a human reads would not be the one that acts.

So:

1. The policy is **read from the target repository** when it declares one, as a
   governed, hashed input like any other authority-set member (`001` section 3.5),
   which means a change to it is an authority change.
2. This product's own state holds **only an explicit, recorded override** for a
   single named spec id, per repository. An override is operator-initiated,
   journaled, and surfaced on every attempt it admits. It is never inferred from
   the spec being offered as ready.
3. Where the repository declares nothing, the default in this section applies and
   the attempt records that the policy was defaulted, not declared.

**How the declaration is spelled is not decided here, and must not be invented
here.** Note 06 section 3.6 is explicit that a machine-readable schema is one
candidate among several (a `[policy]` table in `spec-spine.toml`, a documented
convention read from `AGENTS.md`, or a human-named id) and that nothing should be
read as having chosen one. This spec fixes **where the answer comes from and who
may override it**; the format follows spec-spine's filing, and until that exists
the product reads only the override and the default.

### 3.2 Workspace preparation

A run prepares an **isolated git worktree** under `.statecraft/state/`, branched
from a **base revision resolved to a commit at run start and recorded**. The
operator's checkout is never edited, and no session, check or commit runs in it.

Preparation is idempotent by identity: a run has exactly one workspace, and
re-preparing an existing one is a no-op that reports the existing path. Two runs
never share a workspace.

### 3.3 The run record

Append-only, hash-linked, one chain per registered repository, over the
`attest-ledger` record envelope with `canonical-keysort-json` bytes (see
`001` §3.2). Two properties this product owns rather than inherits: **fsync
before acknowledge**, and an in-memory head so an append stays O(1).

Every effect is bracketed: **intent written and durable before the effect,
outcome written after it**. A crash between the two is therefore detectable, and
recovery can tell what was and was not done.

State is recovered by **folding the record**. Memory is never the authority, and
a recovered state is never reconstructed from the filesystem alone.

### 3.4 The closed outcome set

An attempt ends in exactly one of five outcomes. None is a synonym for another,
and none may be widened later without an amendment.

| Outcome | Meaning |
|---|---|
| `completed` | The attempt ran to its own end. **This says nothing about acceptance**, which is `005`. |
| `failed` | The attempt ran and its work did not hold: a check failed, or the adapter reported a failure result. |
| `refused` | A guard refused. Counted by the supervisor per §3.5. A refusal fails the attempt even when the underlying command exits zero. |
| `interrupted` | The attempt stopped without reaching an outcome: a crash, a kill, a lost supervisor, a base revision that moved. Distinct from `failed`, because nothing was judged. |
| `cancelled` | An operator stopped it deliberately. Distinct from `interrupted`, because the reason is recorded and is not a fault. |

A **retry appends a new attempt** to the run, with its own number, base revision
and outcome. It never edits, deletes or supersedes an earlier attempt's records.
History is the point of the record.

### 3.5 Refusal accounting belongs to the supervisor

The count and a bounded sample of refusals are derived by the supervisor from the
adapter's **structured event stream** (`004`), independently of:

- the adapter's classification of its own termination,
- the supervised process's exit code,
- anything the supervised process says about itself.

The count is written to a location the supervised process cannot reach. A
refusal followed by a `completed` turn is therefore evidence and not silence,
which is the concrete failure the archived predecessor measured before its spec
129.

`refused` is not the only outcome a refusal can produce: a refusal recorded
beside an otherwise completed turn still makes the attempt's outcome `refused`,
and the completed turn is retained beside it.

### 3.6 Recovery and reconciliation

On start, the product folds the record and finds every **intent with no
outcome**. Each is reconciled before anything is retried:

1. Observe the world for the effect the intent describes.
2. Write a reconciliation outcome: `confirmed`, `absent`, or `unknown`.
3. `unknown` **blocks** the retry of that effect and is reported to the operator.
   It is never resolved by assuming either answer.

This product makes **no exactly-once promise** for an external effect. Where an
effect is idempotent by a key, the key is recorded in the intent and named in the
record. Where it is not, the record says so, and a repeat is possible and visible
rather than impossible and claimed.

### 3.7 Concurrency bounds for the first slice

One live attempt per registered repository, and the workspace is the lock. A
second attempt on a repository with a live attempt is refused, naming the live
attempt. Cross-repository scheduling, parallelism, queueing and any resource
budget are out of scope.

**When parallelism reopens, it consumes an upstream report rather than inventing
a format.** Two ready specs can claim overlapping territory, and spec-spine's
spec 091 (`two ready specs can collide`) is the collision report for exactly that
question; it is approved and complete on spec-spine main and in no release
(`C-16`). `F-10` reopens parallelism by consuming that report, not by adding a
locally invented footprint declaration. Frame's footprint field is the shape of
the idea, not a format to copy: Frame computes it from its own spec files, and
this product's specs are spec-spine's.

### 3.8 Observable negative cases

| Case | Required behavior |
|---|---|
| spec-spine's report lacks a field the product needs | Refused, naming the field and the spec-spine version. No locally derived substitute. |
| spec-spine reports a `draft` spec as ready under the default policy | Listed as excluded with the reason. Never scheduled. |
| A named draft is admitted by an operator override | Scheduled, and the attempt records the override, the operator and the spec id. |
| The target declares a lifecycle policy and the product's state disagrees | The repository's declaration wins; the disagreement is reported. The product never prefers its own copy. |
| The target declares no policy | The default (`approved` only) applies, and the attempt records that the policy was defaulted rather than declared. |
| A corpus that does not compile in the target | `work list` refuses for that repository and reports the compile failure; it does not fall back to reading `.derived/`. |
| The base revision moves during an attempt | Outcome `interrupted`; acceptance is refused for that attempt (`005`). |
| The workspace path is occupied by a foreign directory | Preparation refuses, naming the path. No deletion, no reuse. |
| A second attempt starts while one is live | Refused, naming the live attempt. |
| The process dies between intent and outcome | Recovery finds the unmatched intent and reconciles it before any retry. |
| Reconciliation cannot determine whether the effect happened | Written as `unknown`; that retry is blocked and reported; no assumption either way. |
| The record's tail is torn by a crash mid-append | Recovery reads to the last complete record, reports the torn tail, and appends after it. Earlier records are never rewritten. |
| The adapter reports refusals and exits zero | Attempt outcome `refused`; the count comes from the event stream, not the exit code. |
| The supervised process attempts to alter the refusal count | Impossible by placement, and the attempted write is itself recorded. |
| A retry is requested for a `completed` attempt | A new attempt is appended; the earlier attempt's records are unchanged and still readable. |

## 4. Out of scope

Acceptance, receipts and evidence dimensions (`005`); the adapter protocol and
capability negotiation (`004`); any publication effect; quota, cost or spend
control; scheduling across repositories; and human-approval workflow beyond
recording that an approval was required.

## Verification

Declared by the change that implements this spec. None of §3 is implemented, so
this spec carries no `verify:cli` block.
