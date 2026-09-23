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

`crates/statecraft-run/`, written and resolved.

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

### 3.1.3 The contract an attempt is bound to

Recorded on 2026-09-22 before the implementation it authorizes. spec-spine's
specs 106 and 107, merged on its `main` at `088d6d4b` and `6e123d2e` and
present at `3b67b63d`, resolve qualified obligation references
(`<spec-id>#<obligation-id>`) and **context closures**: a request naming specs,
sections and obligations, answered with every member's identity and one digest
over them that does not depend on order. Resolution is the producer's, and it
is pure: the same request against the same ledger gives the same digest. No
released version carries either; the pinned 0.20.0 answers
`registry closure --help` with its usage code.

An attempt at a unit of work is authorized against a spec, and the operator
later needs to know whether the spec the candidate was built against is still
the spec. So:

1. **What is bound.** Before an attempt's intent is appended, `run` asks the
   producer to resolve one closure: the unit of work's spec, and every
   obligation `registry list` says that spec declares, each as a qualified
   reference. Withdrawn obligations are included, and the producer marks them.
   Nothing is added that the producer did not resolve, and nothing the
   producer resolved is dropped.
2. **Where it is written.** The intent's `detail.contract` holds the request,
   the digest, every member verbatim, and the producer version and command
   that answered. It is written once, with the intent, before any effect, and
   no later record rewrites it.
3. **Every other answer is named, and none stops the run.** The binding is
   evidence, not a gate on starting work:

   | `contract.state` | When |
   |---|---|
   | `bound` | the closure resolved; `digest` and `members` are present |
   | `unsupported` | the installed producer answers `registry closure --help` with anything but success; the record names that producer's version and says this product asked and was not answered. It never asserts what a producer **release** carries (spec 005 section 3.3.3) |
   | `unresolved` | the producer refused a member (its exit 1), naming it |
   | `stale` | the producer refused a stale ledger (its exit 2) |
   | `unreadable` | the answer was not the JSON this build reads |
   | `not-a-unit-of-work` | the attempt is spec 002 section 3.33's trial, which is authorized against no spec |

4. **Resolution stays the producer's.** This product computes no digest, reads
   no ledger file, and does not decide which members a spec's closure should
   have beyond rule 1's request. Whether a bound contract still holds is
   judged where it is used, by spec 005 section 3.18, from a new resolution of
   the same request.
5. **An attempt's contract is a fact about that attempt.** A later attempt of
   the same run binds its own; a comparison never updates the earlier one.

### 3.1.4 The operator override, as an operation this product performs

A narrowly scoped authority amendment, settled by the owner on 2026-09-23 and
recorded before the implementation it authorizes. Section 3.1.1 point 2 fixed
that this product's own state holds only an explicit, recorded override for a
single named spec id per repository, operator-initiated, journaled and surfaced
on every attempt it admits. It did not say how an operator makes one, where it
is kept, how it ends or what an attempt records.

**The gap, exactly.** `policy::Override` and `work::select` admit a named spec
the policy excludes, and the `Attempt` type has fields for it. Nothing reaches
them: `run` and `work list` always pass `Overrides::none()`, no verb creates,
shows or removes an override, nothing persists one, and no attempt's intent
records one. The row in section 3.8 ("Scheduled, and the attempt records the
override, the operator and the spec id") therefore holds only in a library
test.

**Rule 1: the operation.** An override is created by the operator naming
exactly one registered repository, exactly one spec id, an operator name and a
reason. The repository must be registered with this product; the spec id must
be one the corpus report names; the name and the reason must be non-empty
after trimming. An override for a spec id that already has one in force in
that repository is refused, so one repository holds at most one override per
spec id and a changed reason is a revocation followed by a new grant. An
override is removed by the operator naming the repository, the spec id, an
operator name and a reason; removing one that is not in force is refused.
Nothing else creates or removes one.

**Rule 2: provenance, stated as it is.** The operator name is **supplied by
the operator and not authenticated**. This product has no identity of its own
to check it against, so the record says `operator-supplied` beside the name,
and no rendering calls it verified. Every grant and every revocation records
the time this product wrote it.

**Rule 3: the journal.** Overrides live in this product's home, in one
append-only journal per registered repository beside that repository's run
record, never in the target. Each line is one grant or one revocation, with the
repository, the spec id, the operator, the reason, the time and the digest of
the previous line, so a line edited, reordered or removed from before the last
one reads as a broken journal rather than as a changed set. The overrides in
force are the fold of the journal: granted and not later revoked. A journal
that does not read, or whose links do not verify, is a failure: `run`, `work
list` and `work show` refuse to proceed on it rather than reading it as empty,
because an unreadable journal that read as "no override" would be
indistinguishable from one an operator never wrote. A write that cannot be made
durable is a failure and leaves nothing in force.

*What the links do not detect, named with the mechanism (constitution VIII).*
Removing the last lines, which could delete a revocation, and deleting the
whole file, which reads as no override, leave nothing to verify against. The
journal is in the product home, which spec `004` section 3.6 already names as
reachable by the same operating-system user; closing that is `F-09`'s.

*A torn last line.* A final line with no line break is a write that did not
complete, as a torn tail is for the run record (section 3.8). It is not in
force, `override show` reports it, and the next grant or revocation truncates it
before appending. It is never read as a grant or a revocation.

**Rule 4: scope.** An override admits one spec id, in one repository, past the
lifecycle policy, and nothing else. It does not change the spec's status,
which stays what the corpus says; it never ratifies. It does not make a spec
ready that the report does not name as ready, and it does not bypass arming,
the harness requirement, the one-live-attempt lock, preflight, the contract
binding or any refusal in specs `002`, `004` or `005`. An override in one
repository is not read for any other repository, including one whose corpus
has a spec with the same id.

**Rule 5: what an attempt records.** An attempt's intent in the run record
(section 3.3's intent entry, not a launch's `intent.json`, which is spec
`002`'s) records how the spec was admitted: `policy` with the policy's source (`declared` or `defaulted`)
when the policy admitted it, and `override` with the spec id, the operator as
supplied, the reason, the grant's time and the digest of the grant's journal
line when an override did. `run show` and `run list` render it. An intent
written before this section carries neither, and reads as **not recorded**: it
is never read as `defaulted` and never as admitted by an override.

**Rule 6: the default is unchanged.** Without an override in force, section
3.1.1's default refusal holds exactly: an excluded spec is listed with its
reason and `run` refuses it. A revocation restores that refusal for later
invocations. Attempts already recorded keep the override they recorded.

**Rule 7: the repository lock.** `run` holds an exclusive advisory lock on
one lock file per repository in this product's home, beside the run record,
from before it appends an intent until after its outcome is durable. The
operating system releases the lock when the process holding it ends, however it
ends; nothing infers anything about an effect from that. `override grant` and
`override revoke` take the same lock and refuse, writing nothing, while
another process holds it, so the journal never changes while an attempt is
supervised. Sections 3.5.1 and 3.6.1 use the same lock.

**Acceptance.** Positive, through the binary in an isolated home: a draft is
refused by default; after a grant, `work list` shows it admitted by the named
override and `run` schedules it, and its intent records the override; `override
show` lists it; after a revocation `run` refuses it again and the earlier
attempt still records its override. Negative: a grant with no operator or no
reason, for an unregistered repository or an unknown spec id, or duplicating
one in force, is refused and writes nothing; a revocation of nothing is
refused; an override in repository A does not admit the same id in repository
B; an edited journal line refuses `run` rather than reading as no override; a
spec the report does not name as ready is not scheduled by an override; a
grant while another process holds the repository lock is refused; a torn last
line is reported and not in force; and an intent written before this section
reads as not recorded.

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

### 3.6.1 Reconciling an unresolved attempt, as an operator's act

A narrowly scoped authority amendment, settled by the owner on 2026-09-23 and
recorded before the implementation it authorizes. Section 3.6 fixes that an
intent with no outcome is reconciled before anything is retried, that the
answer is `confirmed`, `absent` or `unknown`, and that `unknown` blocks the
retry. It does not say who reconciles, from what, or what each answer does to
the attempt.

**The gap, exactly.** `recovery::reconcile` and `reconciliation_entry` exist
and nothing calls them. `session::runs` folds only intents and outcomes, so an
attempt whose process died between the two stays live forever: `run` refuses
every later attempt in the repository, names the live one, and spec `002`
section 3.32 rule 24 says "this build has no verb that reconciles an attempt".
The only way out today is editing the record, which section 3.3 forbids.

**What a finding is about.** Whether the attempt's governed work took
effect: any change the session made, in the attempt's workspace or beyond it.
`confirmed` says it did, `absent` says it did not, `unknown` says the operator
cannot tell. "Intent" below is the run record's intent entry (section 3.3); a
launch's `intent.json` is spec `002`'s and is named as such.

**Rule 1: the act names exactly one attempt.** The operator names a registered
repository, a run id, an attempt number, a finding (`confirmed`, `absent` or
`unknown`), the launch state they inspected, an operator name and a reason,
and optionally files offered as supporting evidence. The attempt must be the
repository's live attempt: an intent with no outcome and no conclusive
reconciliation. Naming any other attempt, one already concluded, or one
already reconciled `confirmed` or `absent`, is refused. The reconciliation
takes the repository lock (section 3.1.4 rule 7); if another process holds
it, a `run` is still supervising the attempt, and the reconciliation is
refused and writes nothing.

**Rule 2: the finding is the operator's declaration; the observation is the
product's.** Before writing, this product reads the attempt's launch state the
way `startup show` does (spec `002` section 3.32 rule 23), or `unrecorded` for
a repository with no manifest, and the attempt's `gate.log` where one exists.
It reads; it never launches, signals, replays or re-runs anything to find out
whether an effect happened. The record keeps the two apart: the finding with
`basis: operator-declared`, the operator as supplied and **not
authenticated**, the reason, and each evidence file's path, byte length and
SHA-256 (its content is not copied and not read for a decision); and beside it
the observed launch state, whether the gate log records a released tool call,
the process id `launched.json` confirmed where it did, whether a process with
that id exists at that moment, and the files it read. A finding is
`corroborated` only where this product's own write order establishes it
independently: `absent` against `not-launched` or `spawn-failed`, where section
3.32 rule 22 guarantees no provider process was created for this attempt.
Nothing else is corroborated, and no rendering calls a declaration verified.

*What this does not establish.* A provider process a dead supervisor started
can outlive it; the record says whether a process with the confirmed id exists,
which is an observation and not an identification, and reconciliation stops
nothing. That residual is named wherever the verb is described.

**Rule 3: stale and conflicting reconciliations are refused.** The launch state
the operator names must equal the one this product reads at the moment of
writing; otherwise the reconciliation is stale and refused, and nothing is
written. Against each state:

| Observed launch state | `confirmed` | `absent` | `unknown` |
|---|---|---|---|
| `not-launched` (no `intent.json`) | admitted | admitted, corroborated | admitted |
| `spawn-failed` | admitted | admitted, corroborated | admitted |
| `launch-unknown` | admitted | admitted, declared only | admitted |
| `interrupted` (a process created and stopped before its prompt) | admitted | admitted, declared only | admitted |
| `outcome-unknown` | admitted | admitted, declared only, unless the gate log records a released tool call | admitted |
| a completed launch record (section 3.31 rule 21) | admitted | admitted, declared only, unless the gate log records a released tool call | admitted |
| `unrecorded` | admitted | admitted, declared only | admitted |

Where the gate log records a released tool call, this product's own record
says governed work ran, and `absent` contradicts it: refused as conflicting. The gate writes its log on a best-effort basis and only for a
project that commits a requirement, so an absent or empty log proves nothing:
it neither corroborates `absent` nor refuses it.

**Rule 4: what each finding does.**

| Finding | The attempt | A later `run` in the repository |
|---|---|---|
| `unknown` | Stays live and unresolved. A later reconciliation of the same attempt may replace it with `confirmed` or `absent`, under rules 1 to 3. | Refused, naming the attempt and its `unknown` reconciliation, as today. |
| `absent` | Resolved: its outcome reads `interrupted`, with the reconciliation beside it. | Permitted. |
| `confirmed` | Resolved: its outcome reads `interrupted`, with the reconciliation beside it. | Permitted, and the retained workspace carries the confirmed effect. |

The next attempt appended in the repository, whichever run it belongs to,
names in its intent every attempt reconciled since the previous one, with its
run, number and finding. Nothing is retried automatically after either
answer; a retry is an operator invoking `run`, which section 3.4 already makes
an appended attempt. Reading an attempt through `run show`, `run list` or
`startup show`, or dismissing an answer, writes nothing and releases nothing:
only a reconciliation record changes whether an attempt is live. An `unknown`
is never turned into either answer by the passage of time, a dead process, a
missing file, an empty output or a deadline.

**Rule 5: the record.** A reconciliation is one appended `reconciliation`
record in the repository's run record, naming the run and the attempt, carrying
rule 2's fields, `verdict` equal to the finding (the key the existing reader
renders), `retryAllowed` per rule 4, `idempotentByKey` and the intent's
idempotency key as section 3.6 requires, so that whether a repeat of the effect
is possible is visible, and, when it replaces an `unknown`, the chain position
of the reconciliation it replaces. The intent and every earlier
record are unchanged. A record that cannot be made durable is a failure, and
the attempt stays exactly as it was.

**Rule 6: records written before this section.** A chain with no
reconciliation record folds exactly as before. A `reconciliation` record in the
older shape (`verdict`, `retryAllowed`, `idempotentByKey`, no `basis`), which
no verb ever wrote, is read and rendered, and never releases an attempt,
because it carries no operator, reason or observation.

**Acceptance.** Deterministic, through the binary, with a fake provider and
crash-boundary fixtures that leave an intent with no outcome at each launch
state: `unknown` keeps `run` refused; `absent` against `not-launched` is
corroborated and releases; `absent` against `launch-unknown` and against
`outcome-unknown` with no released tool call is recorded as a declaration and
releases; `absent` against a gate log recording a released tool call is refused
as conflicting; a reconciliation while another process holds the repository
lock is refused; `confirmed` against `outcome-unknown` releases and the next
attempt names it; a stale launch state is refused; a second conclusive
reconciliation is refused; `unknown` then `absent` is accepted and names what
it replaces; a concluded attempt, an unknown attempt number and an empty
operator or reason are refused; `run show` and `startup show` leave a live
attempt live; and the fake's effects are never re-run by any of these.

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

**2026-09-22: section 3.1.2 implemented.** `report::join` is the pure half:
it takes the version, the plan's bytes and the two `list` answers, refuses
`moved` when the two differ, refuses a plan of two shapes as unreadable,
refuses a disagreement naming both values, and otherwise returns the report
with its `StatusSource`. `SpecSpineCli::corpus_report` reads `list`, `plan`,
`list` after `check` and hands the bytes over unchanged. Each `WorkItem`
carries `status_from`. The tests read the two producers' own recorded answers
(`testdata/producer/`, with their provenance), mutate a copy for the
contradiction, and pair two different recorded `list` answers for the move.
`tests/producer_candidate.rs` is ignored by default and runs the real read
against a binary and revision the operator names, on a scratch copy of this
corpus whose ledger that binary compiled. Measured with it on 2026-09-22:
spec-spine 0.20.0 (`v0.20.0`, `4d14cce6`) reads with `status` from `registry
list` alone, and the unreleased `3b67b63d` reads with the plan agreeing.

**2026-09-22: section 3.1.3, recorded before implementation.** `run` binds
each attempt to one closure the producer resolves, the unit of work's spec and
every obligation it declares, and writes it once into the intent. Every answer
other than a resolved closure is a named state, none of which stops the run.
Measured on the unreleased `3b67b63d`: a spec member alone does not carry the
spec's obligations, so the request names them. No code changed with this
entry.

**2026-09-22: section 3.1.3 implemented.** `contract.rs` builds the request
from the list row's declared obligations (`SpecLifecycle` now reads
`obligations`, empty under a producer that reports none), asks
`registry closure --help` before resolving so an unknown subcommand and a
stale ledger are never read from one exit code, and reads the producer's exit
0, 1 and 2 as resolved, unresolved and stale; anything else is unreadable.
`session::begin_bound` writes the binding into the intent's `detail.contract`
with the intent itself; `begin` is unchanged and writes none. `run` binds after
the eligibility check and before `begin_bound`; the trial of spec 002 section
3.33 binds `not-a-unit-of-work`. Measured with the ignored named-producer tests
on 2026-09-22: the unreleased `3b67b63d` binds a closure for
`002-environment-lifecycle` on a scratch copy of this corpus it compiled, and
resolving it again answers the same digest; the pinned 0.20.0 is
`unsupported`, naming itself. A third ignored test replays spec-spine 103's
portable verifier fixtures through a named build's `verify-attestation
--recompute`: `3b67b63d` reproduces all 11 cases from its own revision, and
0.20.0 reproduces 10, refusing the control as a version mismatch, which is
what fixtures bound to the tool that produced them should do. That test is
adoption evidence for a producer build; nothing in this product verifies these
attestations, and no verifier is built here to give the fixtures something to
test.

**2026-09-23: `check`'s exit status is carried into the refusal, not folded
into "does not compile".** Section 3.1.2 rule 3 reads the reports only after
`check` has said the ledger is fresh. The report source treated every non-zero
`check` as the section 3.8 row "a corpus that does not compile". The 2026-09-23
audit measured what that hides. Run with a `spec-spine` on `PATH` that did not
satisfy this repository's `=0.20.0` pin, `work list` said the corpus did not
compile, when the producer had refused the pin with exit 3 and judged nothing.
The section was silent on which answer is which, so this records the reading.
`check` exiting 1 is still `CorpusDoesNotCompile`, and the row is unchanged.
Exiting 2 is `LedgerStale`, the rule's freshness precondition unmet. Any other
end is `ProducerRefused`, naming the version, the exit status and the
producer's first line. Nothing is read in any of the three cases, and the
report never falls back to the derived tree. Which binary is resolved is
unchanged: the operator's `spec-spine`, never one the candidate chose, with
the target's pin enforced by the producer itself.

**2026-09-23: the pinned producer now carries both reports' `status` and
resolves closures.** The CLI pin moved to `=0.23.0` (decisions `D-06`, entry
of this date). Sections 3.1.2 and 3.1.3 were written against a pin that
carried neither, and each says so as a dated measurement, which stays as
written. Measured under the new pin, through the CLI, on this corpus:

- `registry plan --json` is read schema 0.7.0, and its one ready row carries
  `status`, which the join compares with `registry list` and finds agreeing.
- `registry closure` for `002-environment-lifecycle` resolves to one member
  and one digest.

So rule 2's comparison and section 3.1.3's binding are live against the pinned
producer, not only against a named candidate build. No rule changes. Section
3.1.2 rule 4's released shape without `status` stays readable, because a
target may pin an older producer.

**2026-09-23: the published producer's own answers are recorded, and the
fixture replay asserts outcomes, not only exit codes.**
`testdata/producer/released-0.23.0/` holds what the pinned 0.23.0 answered on
this corpus: `registry plan` and `registry list`, and two `registry closure`
answers, one resolved and one refusing a missing member. Until now every
closure this crate read was written by hand in the measured shape. `report.rs`
and `contract.rs` test the join and `interpret` against those bytes. The older
recorded sets stay, as the evidence they were.

The ignored `producer_candidate` fixture test now checks, per case:

- the exit code;
- the verifier's `ok`;
- the recompute outcome its recorded reason names (`match`,
  `contentMismatch`, where `non-canonical-bytes` is reported as a content
  mismatch, or `versionMismatch`);
- that a refusal before the recompute carries a structured error.

The verifier reports only an error kind for that last case, so no message
text is parsed. Run against the published build and the fixture set shipped
inside the published `spec-spine-core` 0.23.0 crate, all eleven cases
reproduce. Run against the 0.20.0 binary with the same set, four do not:
`control-untampered`, `flipped-verdict`, `minor-ahead-content-mismatch` and
`reformatted-same-values`, each read as a version mismatch where the set
expects `match` or `contentMismatch`. The same run binds `unsupported`, because 0.20.0 has no
closure verb, and reads `status` from the list alone. So an older producer is
refused by name where it lacks a capability, and nothing about it is read as
the newer contract.

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
cargo test -p statecraft-run --lib report
cargo test -p statecraft-run --lib contract
```
