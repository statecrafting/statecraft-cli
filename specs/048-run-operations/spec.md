---
id: "048-run-operations"
title: "Typed run operations under one controller lease, recorded in the run record"
status: draft
implementation: pending
created: "2026-10-21"
summary: >
  Gives a run three typed operations, attempt.retry, attempt.continue and
  attempt.abort, submitted by one controller holding a lease. Each carries an
  idempotency key and the run version it was decided against; each request
  and its outcome append to spec 003's hash-linked run record, durable and
  uncapped, so replaying the record reproduces the run's state. A reconnecting
  controller reads events from a cursor over the record. The one live slot of
  spec 003 section 3.7 is held or released explicitly by each operation.
amends:
  - "003-work-and-run-semantics"
  - "040-application-operations"
# No `extends` yet: the implementing change adds the edges on
# crates/statecraft-run/ (003) and crates/statecraft-cli/ (006) with the code.
depends_on:
  - "003-work-and-run-semantics"
  - "040-application-operations"
# `obligations` is spec-spine's own frontmatter field, used by approved specs
# such as 013, 014 and 018; `make gate` lints it with --fail-on-warn.
obligations:
  - id: "I-1"
    kind: invariant
    text: "A run's state is a fold over its run record alone: replaying the record reproduces every lease, operation outcome and attempt, and no operation state lives only in memory or in a capped table."
    anchor: "3-4-the-record-is-the-state"
  - id: "R-1"
    kind: requirement
    text: "An operation whose idempotency key the record already holds for that run, with the same operation and version, appends nothing and answers the recorded outcome; the same key with a different operation or version is refused as key-conflict."
    anchor: "3-3-idempotency-and-version"
  - id: "R-2"
    kind: requirement
    text: "An operation whose expected version is not the run's current version is refused with exit 2 and the closed reason stale-version, and appends a refusal, never the effect."
    anchor: "3-3-idempotency-and-version"
  - id: "R-3"
    kind: requirement
    text: "An operation submitted without the run's current lease is refused with exit 2 and the closed reason lease-not-held, before any idempotency or version rule of section 3.3, so a caller without the lease learns nothing about keys."
    anchor: "3-2-one-controller-lease"
  - id: "R-4"
    kind: requirement
    text: "An abort records stop-requested when it is accepted and stop-confirmed only when the attempt's cancelled outcome is durable; neither is read as the other."
    anchor: "3-1-the-three-operations"
  - id: "V-1"
    kind: verification
    text: "A record built by a sequence of operations replays to the same state; a repeated key appends nothing; a stale version exits 2 naming stale-version; a reconnecting cursor reads exactly the events after it."
    anchor: "verification"
    inputs:
      - "crates/statecraft-run/src/record.rs"
---

# 048: Typed run operations under one controller lease

## 1. Purpose

Spec 003 says what an attempt is, how it ends and that a retry appends a new
attempt; it leaves the act of retrying to an operator invoking `run` again.
Spec 039's cockpit and any agent acting as a controller need the same acts as
operations they can submit, see the result of, and safely repeat after a lost
connection. Without a key, a controller that times out and resends launches a
second attempt; without a version, two controllers each decide against a state
the other has already changed. This spec adds those two checks, a lease that
names the one controller allowed to act, and a cursor to read what happened,
and puts all of it in the run record that already exists.

**Drafts this one cites:** spec 047, the capability seam (on main as a draft since pull request 299), and spec 053, operator input (on main as a draft since pull request 307). Each is cited by its proposed number so the reference survives ratification.

## 2. Territory

`statecraft-run` gains the lease, operation, idempotency and version fold over
its record (`crates/statecraft-run/src/record.rs`) and the slot rules beside
`crates/statecraft-run/src/reconcile.rs`. `statecraft-cli` gains the typed
operations in its application layer (spec 040) and their verbs. No hosted
transport (`F-01` stays deferred), and no new record envelope.

## 3. Behavior

### 3.1 The three operations

| Operation | Precondition | Effect |
|---|---|---|
| `attempt.retry` | The run's latest attempt has an outcome and the slot is free. | Appends a new attempt from a fresh workspace at the base revision resolved now, as spec 003 section 3.4 already describes for a retry. |
| `attempt.continue` | The run's latest attempt has an outcome, the slot is free, and the adapter can resume a provider session. | Appends a new attempt that resumes the previous attempt's provider session in the same workspace. It is a new attempt with its own number and outcome; the earlier attempt's outcome is never edited. |
| `attempt.abort` | The run has a live attempt. | Requests the supervisor stop it; the attempt ends `cancelled` (spec 003 section 3.4) with the controller and its reason recorded. |

An abort has two recorded points: `stop-requested` when the operation is
accepted, and `stop-confirmed` when the attempt's `cancelled` outcome is
durable. A reader never reports a stop as done from the first.

Whether an adapter can resume is a capability, not an adapter name: if spec
047 is ratified, `attempt.continue` reads its declaration; until then it is
refused for every adapter with `capability-undeclared`.

### 3.2 One controller lease

A controller acquires the run's lease by appending an `Accounting` record
naming itself and the run version it observed. One lease per run; acquiring
while another controller holds it is refused with `lease-held`, naming the
holder. A lease ends when its holder releases it, when the run has no live
attempt and the holder's process has ended, or when an operator breaks it,
which is recorded with the operator's name. Reading (the cursor, `runs`) needs
no lease. An operation from a controller that does not hold the current lease
is refused with exit 2 and `lease-not-held`, first of all (section 3.3, rule
0).

### 3.3 Idempotency and version

Every operation carries a controller-chosen idempotency key and the run
version it was decided against. The run version is the count of records about
that run in the chain.

The rules are checked in this order, and the first that applies decides:

0. An operation without the run's current lease is refused with
   `lease-not-held` before any rule below (section 3.2); a caller without the
   lease learns nothing about keys.
1. A key the record already holds for that run, submitted with a different
   operation or version, is refused with `key-conflict`.
2. A key the record already holds for that run, submitted with the same
   operation and version, is a no-op: nothing is appended, and the answer is
   the outcome recorded under that key.
3. A new key whose version is not current is refused with exit 2 and reason
   `stale-version`, naming both versions. The refusal is recorded; the effect
   is not.

The new refusal reasons join spec 003's closed set by this amendment; no other
reason is added.

### 3.4 The record is the state

Each accepted operation is bracketed in the record as every effect already is
(spec 003 section 3.3): an intent naming the key, version, controller and
operation, durable before the effect, and an outcome after it. Nothing is
capped and nothing is evicted: preloop-ci keeps its idempotency table and
audit trail in memory, bounded at 256 and 512 entries
(`crates/preloop-runner-server/src/debug_sessions.rs`, MIT), and a replay past
the bound re-executes. Here the table is the record, so a key is remembered
for as long as the run is.

### 3.5 The live slot

An operation that launches (`retry`, `continue`) takes the repository lock and
the one live slot of spec 003 section 3.7 for the attempt's duration, exactly
as `run` does, and refuses naming the live attempt when the slot is taken. An
abort releases the slot only through the `cancelled` outcome; an attempt whose
stop is requested but never confirmed stays live and goes to spec 003
section 3.6.1 reconciliation like any other unresolved attempt.

### 3.6 The event cursor

A controller reads events after a cursor, the position of a record in the
chain, and receives the run's records after it in order with the next
cursor. A reconnecting controller resumes from the last cursor it holds and
misses nothing, because the chain is the source and is never truncated.

## 4. Out of scope

- Any remote transport for operations; spec 039's localhost cockpit is the
  only caller beyond the CLI.
- Operator input during an attempt (notes and commands), which the draft spec 053 (operator input record) proposes.
- Retrying from an earlier step: an attempt has no steps here.

## 5. Resolved decisions

**2026-10-21: the record is the idempotency table.** The reference design
bounds its table and accepts that a late duplicate re-executes, relying on the
version check to refuse it. Here a late duplicate is answered from the record,
and the version check stays as a second, independent guard.

**2026-10-21: `continue` is a new attempt.** The reference design continues a
paused job in place. An attempt here has no paused state, and spec 003 forbids
editing a concluded attempt, so continuing is a new attempt that resumes the
provider session.

## Verification

This spec is `status: draft` and declares no acceptance until it is ratified
and built. Its acceptance will build a record from a sequence of operations
and replay it to the same state, repeat a key and find nothing appended,
submit a stale version and see exit 2 naming `stale-version`, and resume a
cursor after a dropped connection with no event missed or repeated.
