---
id: "003-work-and-run-semantics"
title: "Work selection, workspace preparation, the run record, and recovery after interruption"
status: approved
implementation: complete
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
extends:
  # S1 adds one field to the record payload `003` owns. `Entry` has public
  # fields, so every struct literal in the workspace must name the new field,
  # and one of those literals is a test helper in `005`'s crate
  # (`crates/statecraft-acceptance/src/suite.rs`). The edge is additive, the
  # edit there is a single `effect_id: Identity::Absent`, and nothing `005`
  # requires changes.
  - { spec: "005-acceptance-and-evidence", unit: { kind: directory, path: "crates/statecraft-acceptance/" }, nature: additive }
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

### 3.1.2 Two reports that both carry status, read from one state

Recorded on 2026-09-22 before the implementation it authorizes. spec-spine's
spec 102, merged on its `main` at `75a998f7` and present at `3b67b63d`, adds
`status` to each `ready` row of `registry plan --json`, copied from the
registry record. Measured here on 2026-09-22 against a scratch copy of this
corpus recompiled by the CLI built from `3b67b63d`: every `ready` row carries
`id`, `title` and `status`, and no row carries `implementation`. The pinned
0.20.0 carries neither. No released version carries `status` in the plan: the
registry's newest is 0.21.0.

So a producer may answer `status` twice, once in each report this section
joins, and this product must not pick one silently. Five rules:

1. **The join stays.** `registry list --json` is still the only report that
   carries `implementation`, so it is still read, and `status` is still taken
   from it. Nothing here narrows what section 3.1.1 reads.
2. **Where the plan carries `status`, it is compared, not preferred.** For
   every ready row, the plan's `status` must equal the list's `status` for the
   same id. A difference is a **disagreement**, and the read is refused naming
   the id, both values, both reports and the producer version. Nothing is
   scheduled from a report that contradicts itself, and neither value is
   chosen.
3. **A disagreement is only reported from one state.** The reads are
   bracketed: `registry list`, then `registry plan`, then `registry list`
   again, after `check` has said the ledger is fresh. The two `list` answers
   must be byte-identical; if they are not, the ledger moved while it was being
   read, the read is refused as **moved**, and nothing is compared. This
   product still never reads the ledger's files itself (section 3.1).
4. **A plan row without `status` is the released shape, not a gap.** Under a
   producer whose plan carries no `status`, the join proceeds exactly as
   before, and each row records that its `status` came from `registry list`
   alone. A plan that carries `status` on some rows and not others is refused
   as unreadable, because a report that is not one shape is not a report this
   build reads.
5. **Every row says where each field came from.** `status` is recorded with
   its sources: `registry list` alone, or `registry list` agreeing with
   `registry plan`.

A disagreement or a moved ledger is a refusal (exit **2** under spec 006
section 3.3): the precondition that the producer answers consistently from one
state was not met, and nothing was done. It is not a failure of this product,
and it is not a finding about the target.

What this does not establish: that the producer's two answers are correct,
only that they agree; or that the ledger did not move and move back between
the two `list` reads. The second is recorded rather than excluded.

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

### 3.3.1 Effect identity and one-to-one closing

§3.3 brackets every effect between an intent and an outcome, and §3.6 folds the
record to find every intent with no outcome. Neither says how an outcome names
the intent it closes. Pairing them by `(run_id, attempt, subject)` uses a key
the record does not make unique, and pairing on a non-unique key is not pairing:
two effects bracketed in one attempt under one subject are read as one, and a
single outcome closes both.

An effect this section governs therefore carries its own identity.

1. **The identity.** An intent for such an effect carries an `effectId`: a
   non-empty string, unique within its `run_id`, chosen by the writer before the
   intent is made durable. The outcome that closes the effect repeats it
   unchanged, and every record that is evidence for that effect repeats it too.
2. **The correlation.** For records carrying an identity, correlation is by
   `(run_id, effectId)` and by nothing else. `subject` is never a correlation
   key, `attempt` is never a correlation key, and neither is consulted as a
   tie-breaker. Two runs may carry the same identity string; they are different
   effects, and that is not a defect. Every answer the fold gives names both
   halves of the key, so no result is attributable to the wrong run.
3. **Absence, validity and the three states.** A record is in exactly one of
   three states, and they are never conflated:
   - **absent**: the record carries no `effectId` key. This and only this is a
     record without an identity, folded by the pairing of clause 7.
   - **valid**: the key is present and its value is a string that is not empty.
   - **invalid**: the key is present with any other value, **including JSON
     `null`**, an empty string, a number, a boolean, an array or an object.

   An invalid identity is a defect the fold reports. It is never read as an
   absent identity, the record carrying it is never folded by the pairing of
   clause 7, and the value it carried is retained in the report as the record
   carried it. Presence is therefore decided by the key, never by the value: a
   decoding that maps a present `null` to the same state as a missing key does
   not satisfy this clause. A record is never made undecodable by an invalid
   identity, and is never dropped because of one.
4. **Uniqueness within a run.** At most one intent in a run carries a given
   identity. A second intent carrying an identity an earlier intent of the same
   run already carried is a defect the fold reports, **whether or not the
   earlier one was closed**: closure does not release an identity for reuse, and
   the report says which of the two shapes it found. Neither intent is
   discarded, neither is chosen over the other, and neither overwrites the
   other. The identity is **ambiguous** from the second intent onward: it is not
   reported as one open effect, and no outcome closes it.
5. **One to one.** An intent is closed by at most one outcome. An identity
   already closed is not closed again: a second outcome naming it is a defect
   the fold reports, and it neither replaces the first nor is absorbed by it. An
   outcome naming an identity the fold has no intent for is likewise a defect,
   and so is an outcome naming an ambiguous identity. None of the three closes
   anything.

   Records are folded in chain order. An outcome whose `(run_id, effectId)` has
   no preceding intent is an orphan outcome and closes nothing. A later intent
   does not retroactively match that outcome: it remains open unless a
   subsequent outcome validly closes it. The earlier orphan-outcome defect
   remains reported even if a subsequent outcome closes the intent.
6. **Defects are reported in full, and are the caller's to check.** An
   identity-aware fold result **includes every defect the fold detected, with
   none filtered away, collapsed or summarized out**, each naming the run, the
   identity where there is one, and which condition was found. A defect does not
   change the open or closed status of an unrelated effect. Duplicate intent
   identities follow clause 4: the affected key becomes ambiguous even if an
   earlier outcome closed it. Defects are carried beside the fold's other
   findings rather than in place of them. **A caller that uses a fold result to
   authorize an action must check its defects first**; the fold reports, and
   does not decide. What a defect authorizes or forbids is an activation policy,
   is not fixed here, and has no caller to bind yet.
7. **Records with no identity key are unchanged.** Such a record keeps the
   existing `(run_id, attempt, subject)` pairing exactly, including the intents
   that pairing leaves permanently unmatched. That pairing considers only such
   records. No existing writer is modified, no existing record is reinterpreted,
   and no existing record is rewritten to carry an identity it was written
   without. What the legacy pairing leaves unmatched is a separate finding, with
   its own repair and its own decision, and is not decided here.
8. **Wire form.** The identity is a top-level key of the record payload, spelled
   `effectId`, and the key is **omitted entirely** from a record that has none.
   A build that does not know the key ignores it and reads the record otherwise
   unchanged.
9. **What this section does not do.** It does not generate an identity, does not
   name any subject that uses one, does not allocate an ordinal, does not make
   any verb write one, does not carry an identity into a reconciliation record,
   does not decide what a defect authorizes, and does not change what §3.6 does
   with an unmatched intent once it is found.

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
spec 072 (`two ready specs can collide`) is the collision report for exactly that
question. It landed in spec-spine `v0.20.0` and the pin carries it (`C-16`,
corrected 2026-09-17).

**Its availability is not a reason to reopen `F-10`.** That deferral has two
conditions and the report was only one of them; the other is a single-repository
loop that works, which does not exist yet. The bound in this section is
unchanged: one live attempt per registered repository, and the workspace is the
lock. When `F-10` does reopen, it reopens by consuming that report, not by adding
a locally invented footprint declaration. Frame's footprint field is the shape of
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
| Two effects in one attempt share a subject and each carries an `effectId` | Folded as two effects. Each is closed only by the outcome repeating its own identity; the other stays unmatched and keeps its identity. |
| A second outcome names an `effectId` that is already closed | Reported as a defect naming the run and the identity. The first outcome stands; the second neither replaces it nor is absorbed. |
| An outcome names an `effectId` no intent in the fold carries | Reported as a defect naming the run and the identity. It closes nothing and is never dropped silently. |
| A record carries no `effectId` key | Folded by the legacy pairing of §3.3.1 clause 7, unchanged. |
| A record carries an `effectId` that is empty, `null`, or any value that is not a non-empty string | Reported as a defect naming the record and retaining the value as carried. Never read as a record without an identity, never dropped, and never a decode failure. |
| Two intents of one run carry the same `effectId` | Reported as a defect naming both, and saying whether the second followed the first's closure. Both are retained, neither is chosen, and the identity is ambiguous: no outcome closes it. |
| Two different runs carry the same `effectId` string | Two independent effects, each reported under its own run. Not a defect: uniqueness is within a run. |

## 4. Out of scope

Acceptance, receipts and evidence dimensions (`005`); the adapter protocol and
capability negotiation (`004`); any publication effect; quota, cost or spend
control; scheduling across repositories; and human-approval workflow beyond
recording that an approval was required.

## 5. Decisions recorded during implementation

Dated entries for choices §3 was silent on. None changes what it requires.

**2026-09-16: the chain lives in the product home, not in the target.** §3.3
says one chain per registered repository and does not say where. §3.5 does
constrain it: the refusal count is written where the supervised process cannot
reach it, and that process runs inside a worktree under the target's
`.statecraft/state/`. A chain inside the target would be a chain the thing being
judged can edit, so the chain is in the product home, keyed by a digest of the
target's absolute path. An integration test asserts the chain path is under
neither the target nor the workspace.

**2026-09-17: the correction to section 3.7 removes a wait, not a bound.**
Spec-spine 091 was described here as carried by no release, which the move to the
`=0.20.0` pin falsified. Only the sentence was stale: this section rests on 091
as the future answer to `F-10` and never conditioned the concurrency bound on it,
so nothing it requires changed and no behavior did. The bound stays one live
attempt per repository because its reason was never tool support; it was that
cross-repository scheduling is out of scope for the first slice.

**2026-09-16: two reports are joined, because one does not carry status.**
§3.1.1 turns on a spec's `status`, and `registry plan --json` under 0.18.0
carries only `id` and `title`. `registry list --json` carries `status` and
`implementation`. Both are spec-spine's structured output, so joining them is
still reading rather than deriving. A ready spec absent from the lifecycle
report is excluded with "status is unknown" rather than assumed approved.

**2026-09-17: both reports are envelopes, and only one of them was read as
one.** §3.1 requires the product to parse spec-spine's structured output and
§3.1.1 fixes the join, and neither says what the outer shape of either answer
is. Measured against the pinned spec-spine 0.20.0 on 2026-09-17, against this
repository's own corpus and against a scratch one: `registry plan --json`
answers `{"ready": [...], "blocked": [...], "notSchedulable": N,
"schemaVersion": ...}` and `registry list --json` answers `{"items": [...],
"schemaVersion": ...}`. The plan half was already read through its own key; the
lifecycle half was read as a bare array, so every `work`, `run` and `accept`
invocation against a real corpus exited 4 with "expected an array of specs".
No test caught it because every fixture was a hand-built bare array, which is
the shape the parser wanted rather than the shape spec-spine gives it. The
lifecycle half is now read through `items`, a bare array is still accepted
because an older report that is one carries the same rows, and the tests carry
the measured envelope. Nothing §3 requires changed: this is the same read, of
the same two reports, finally performed on the bytes they actually contain.

**2026-09-17: the version a refusal names is the version.** §3.1 requires a
refusal to name the missing field and the spec-spine version, and
`spec-spine --version` prints `spec-spine 0.20.0`. Keeping the whole line made
every refusal read "spec-spine spec-spine 0.20.0 report ..." and put a program
name inside `specSpineVersion`, which `006` §3.4 makes a contract. The last
whitespace-separated token is taken, which is what this product already does
where it asks spec-spine the same question for the environment manifest's pins.

**2026-09-16: a torn tail is truncated before the next append.** §3.8 requires
recovery to read to the last complete record, report the tear, and append after
it, without rewriting earlier records. The trailing partial bytes are truncated
at the first append after the tear. That is not rewriting a record: those bytes
were never acknowledged, because acknowledgement is what `fsync` and a
terminating newline together mean here. A test asserts the earlier bytes are
identical before and after.

**2026-09-16: an empty chain is not a broken one.** `attest-ledger`'s
`verify_chain` reports `EmptyChain` for an empty slice, correctly for a ledger
that should have an anchor. On a repository's first run there is nothing to link
yet, so verification begins once there is a record. Reported here because it is
a behavior of a reused component this spec depends on, not a local invention.

**2026-09-16: attest-ledger is a pinned git dependency.** It is public and
unpublished, and `001` §3.2 says the record envelope is reused rather than
reimplemented. Vendoring a copy of a hash-linked ledger into the product that
depends on it would defeat the reuse. The workspace is `publish = false` and
`F-02` defers publication, so the usual objection does not apply yet;
un-pinning it, or moving to a released version, is its own change.

**2026-09-19: the identity is a top-level payload key, spelled in camelCase.**
§3.3.1 puts an identity on the record payload, whose existing keys are `kind`,
`run_id`, `attempt`, `subject`, `idempotency_key` and `detail`: Rust field names
serialized as written. Every other JSON this product emits is camelCase,
including the keys inside `detail` (`baseCommit`, `retryAllowed`, `outcome`) and
every CLI view. Two spellings were available: follow the neighbouring payload
keys, or follow the product's JSON contracts. `effectId` is chosen, because this
record is read through the contracts rather than through the struct, and the
serialization is therefore stated explicitly on the field rather than inherited
from the field's Rust name. Placing the identity inside `detail` was rejected:
`detail` is the untyped remainder, and a correlation key the fold depends on is
not a remainder.

**2026-09-19: presence is decided by the key, and an invalid value is a reported
defect rather than a decode failure.** §3.3.1 clause 3 needs three states where
an optional field offers two. An `Option` of the identity type does not give
them: a self-describing format's `null` deserializes to the same `None` as a
missing key, so an explicit `null` would be read as a record that never carried
an identity, which clause 3 forbids. The field is therefore a three-state value
of its own, defaulting to absent when the key is missing and decoding any
present value: a non-empty string is the identity, and anything else is retained
and reported as invalid. Decoding it as a plain string instead would fail
deserialization, and the fold reads payloads through a decoder that today
discards what it cannot decode, so the strictest-looking choice would have been
the one that loses the defect. What this preserves is the **JSON value** an
invalid identity carried, not necessarily the byte sequence that expressed it:
number formatting, string escaping and object key order are the encoder's. That
is sufficient, because a stored record is never rewritten by this product, so
the historical bytes on disk are untouched regardless; and a record emitted with
no identity omits the key and is byte-identical to what this product writes
today. Making the decoder refuse rather than discard is a separate change, in a
separate spec's territory, and is not made here.

**2026-09-22: section 3.1.2, recorded before implementation.** spec-spine
102 puts `status` in the plan report. Section 3.1.2 keeps the join, compares
the two `status` answers rather than preferring one, refuses a disagreement
and refuses a ledger that moved between bracketed reads. No code changed with
this entry.

## Verification

Each line is one command. §3.8's twenty-two rows are integration tests named after
the rows they cover, including the ones that need a real git repository: the
workspace rows build one in a temporary directory rather than mocking git,
because "the operator's checkout is never edited" is not a claim a mock can
support.

```verify:cli
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
spec-spine index coverage --fail-on-untraced
test -f crates/statecraft-run/src/record.rs
test -f crates/statecraft-run/tests/negative_cases.rs
```
