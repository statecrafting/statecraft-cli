---
id: "027-observation-and-proposals"
title: "Observation and proposals: noticing that a repository's governed truth changed, and proposing typed work without acting on it"
status: draft
implementation: pending
created: "2026-09-28"
summary: >
  Adds the transition this product lacks between "spec-spine says the
  repository means X" and "the operator is offered the work that follows".
  One observation reads exactly one commit through spec-spine's existing
  structured reports, reduces the answers to three closed projections, and
  compares them with the last observation recorded for that repository. Each
  semantic transition a built-in subscription names becomes a proposal with a
  deterministic identity, recorded in an append-only chain in the product
  home. A proposal is never executed by this spec: the operator admits it,
  admission re-derives its preconditions against the current commit, and an
  admitted proposal enters spec 003's existing run path with its cause
  recorded in the attempt's intent. No watcher, no loop, no automatic
  admission, and no change to spec-spine.
# The crate this spec owns, `crates/statecraft-observe/`, is claimed by the
# change that creates it (section 2): `index check --fail-on-unresolved`
# refuses a claim on a directory that does not exist yet.
extends:
  # Section 3.7: the attempt intent names the proposal that caused it, written
  # once with the intent and before any effect. Additive: an intent without a
  # proposal is exactly today's intent.
  - { spec: "003-work-and-run-semantics", unit: { kind: directory, path: "crates/statecraft-run/" }, nature: additive }
  # Section 3.9: the `observe` and `proposal` verbs, each a binding of one
  # library operation in this spec's crate.
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "001-boundaries-and-authority"
  - "003-work-and-run-semantics"
  # Section 3.9: `proposal show` reports an admitted run's acceptance state as
  # spec 005 records it.
  - "005-acceptance-and-evidence"
  - "006-command-surface"
---

# 027: Observation and proposals

## 1. Purpose

Every transition this product performs today is invocation-driven. An
operator runs `work list`, reads spec-spine's answer as of that moment, and
names a unit of work to `run`. Nothing records what the answer *was* the last
time it was asked, so nothing can say what changed, and nothing can say why a
given piece of work became available.

This spec adds that record and nothing that acts on it:

```
commit ──> spec-spine reports ──> projections ──> transition ──> proposal
 (one)      (existing verbs)      (closed set)    (vs. last      (recorded,
                                                   recorded)      never run)
                                                                     │
                                          operator: proposal admit ──┘
                                                         │
                                          re-derive preconditions
                                                         │
                                          spec 003 run path, unchanged,
                                          intent names the proposal
```

It holds four properties the rest of the product already holds, and states
them here so an implementation cannot trade one away:

1. **spec-spine establishes truth; this spec only compares answers.** No
   projection is computed from a registry or index shard. Every input is a
   structured report spec-spine already emits and spec 003 already consumes.
2. **An observation is of one identified commit.** It is reproducible from its
   recorded inputs, so its result is evidence rather than a sample of a moving
   working tree.
3. **A wake is not a fact.** Nothing in this spec reads a filesystem event, and
   the successor that adds one (section 4) may only cause an observation, never
   supply its content.
4. **Noticing is not acting.** A proposal carries no authority. Admission is an
   operator act that passes through spec 003's lifecycle policy, lock and
   contract binding exactly as `run` does.

## 2. Territory

This spec owns one new crate, `crates/statecraft-observe/`, a library with
no binary (the verbs are spec 006's, joined through the additive edge above).
The `establishes` directory unit is added by the change that creates the
crate, because the gate refuses a claim on a directory that does not exist. It
owns:

- the observation: which commit, which inputs, which reports, which result;
- the three projections of section 3.3 and the transitions between two of
  their values;
- the closed subscription set of section 3.4;
- the proposal, its identity, its preconditions and its dispositions;
- the observation chain in the product home, and its recovery.

It extends spec 003 by one additive member of the attempt intent (section
3.7) and spec 006 by the verbs of section 3.9. It changes nothing spec-spine
does, requires no spec-spine release beyond the pin in force, and reads no
file under `.statecraft/derived/`.

The crate depends on `statecraft-run` for the report reader, the lifecycle
policy, the override journal's current state, the contract binding and the
repository key, so that the eligibility this spec records is the eligibility
`work list` would print for the same inputs. Reimplementing any of them here
would give one question two answers.

## 3. Behavior

### 3.1 An observation is of exactly one commit

An observation names a registered repository and a ref (default: the branch
the operator checkout's `HEAD` names, or its commit id when `HEAD` is
detached) and resolves the ref to a commit id once, before anything else is
read. The ref is recorded as named, beside the commit it resolved to, so that
admission can observe the same ref again (section 3.6). Every later read in that observation is
of that commit's tracked bytes.

1. **The operator's checkout is never read as the observed tree.** The commit
   is materialized read-only in the product home, following the same rule as
   spec 003's workspace: the operator's checkout is never edited and never
   substituted. Uncommitted edits are not observed. This is deliberate: every
   spec-spine answer this spec consumes (plan, list, closure, check) is an
   answer about a ledger that is committed with the tree, and a working tree
   has no identity an observation could record.
2. **The observation's input identity** is the canonical digest of: the
   repository key (spec 003 section 5, 2026-09-23, "one repository key"), the
   commit id, the spec-spine version token the reports carry, the lifecycle policy as read at that
   commit (with whether it was declared or defaulted), and the override
   journal's current state (spec 003 section 3.1.5). Overrides are product
   state that changes eligibility, so an observation that omitted them would
   record two different answers under one identity.
3. **An observation whose input identity equals the last recorded one** writes
   nothing and reports `unchanged`. Recording it again would add an entry that
   no reader can distinguish from the first.
4. **The reports are read through spec 003's reader, from the `spec-spine` on
   `PATH`**, the same resolution `work list` uses. A report missing a field is
   refused naming the field and the version (spec 003 section 3.8, first row);
   no projection is computed from a partial answer.

### 3.2 A refused observation is recorded, not skipped

spec-spine may decline to answer for the observed commit: the committed ledger
is stale, the corpus does not validate, a claim is unresolved, or the pin is
not met. The observation then records `refused` with spec-spine's own exit
class and message, computes no `work-eligibility` or `contract-identity`
projection, and its only possible transition is the `corpus-state` one below.

`corpus-state` is the one projection a refused observation has, and it is
compared with the **immediately preceding** recorded observation, answered or
refused. So the first refusal after an answer is a `corpus-changed` transition
into `refused`, which the `corpus-refusal` subscription (section 3.4) turns
into a notice; a second refusal with the same class is no transition; and the
first answer after a refusal is `corpus-changed` back to `answered`.

For the other two projections, a refused observation does not replace the
last *answered* observation as the comparison base. The next answered
observation is compared with the last answered one, and the transition it
reports spans the refusal. A repository
that is briefly stale therefore never looks as if every spec left eligibility
and re-entered it.

### 3.3 Three closed projections

A projection is a pure function from the observation's reports to a value with
a canonical encoding. The set is closed; a fourth projection is a change to
this spec.

| Projection | Value | Source |
|---|---|---|
| `corpus-state` | one of `answered`, `refused(<class>)` | the observation's own result (section 3.2) |
| `work-eligibility` | for every spec the plan report names as ready: `eligible`, or `excluded` with spec 003's reason | spec 003 section 3.1 join, via `statecraft-run::work::select` |
| `contract-identity` | for every `eligible` row: the context-closure digest spec 003 section 3.1.3 binds an attempt to, or `unavailable` with spec-spine's reason | spec-spine `registry closure`, through spec 003's binding |

A **transition** is computed by comparing a projection's value in two
observations, keyed by spec id where the value is keyed: for
`work-eligibility` and `contract-identity`, the current answered observation
and the last answered one before it; for `corpus-state`, the current
observation and the one immediately before it (section 3.2):

- `entered` (absent or `excluded` before, `eligible` now);
- `exited` (`eligible` before, absent or `excluded` now), carrying the reason;
- `reason-changed` (`excluded` in both, with different reasons);
- `contract-moved` (`eligible` in both, with different `contract-identity`
  values: two different digests, a digest before and `unavailable` now, or
  `unavailable` before and a digest now; two `unavailable` values with
  different reasons are not a move, because no contract was bound under
  either);
- `corpus-changed` (the `corpus-state` value differs).

The transitions of one observation are sorted by projection, then spec id,
then kind, so the same pair of observations always yields the same sequence.

**The first observation of a repository** has no comparison base. It records
its projections and reports every current `eligible` row as `entered`, marked
`initial`, so an operator can tell a backlog that existed before observation
began from work that became eligible while it was being observed.

### 3.4 Subscriptions are built in and closed

A subscription maps one transition kind to one proposal kind. The set is part
of this spec, not of any repository:

| Subscription | Transition | Proposal kind |
|---|---|---|
| `eligible-work` | `work-eligibility` `entered` | `run` for that spec |
| `withdrawn-work` | `work-eligibility` `exited` | `notice`, naming the reason and every open `run` proposal for that spec |
| `moved-contract` | `contract-identity` `contract-moved` | `notice`, naming every open `run` proposal and every live or unaccepted attempt bound to the earlier digest |
| `corpus-refusal` | `corpus-state` `corpus-changed` into `refused` | `notice`, naming spec-spine's class and message |

`reason-changed` and `corpus-changed` back to `answered` are recorded as
transitions and propose nothing.

A `notice` has no effect to admit; it is closed only by `proposal acknowledge`.
Only a `run` proposal can be admitted or dismissed.

**A subscription is not configuration.** A target cannot add, remove or
reshape one. The target already controls what becomes eligible, through the
lifecycle policy spec 003 reads at its base; a subscription file in the target
would be a second, candidate-authorable way to cause work, which constitution
VII refuses (a candidate cannot enlarge its own authority). Configurable
subscriptions are deferred by name in section 4.

### 3.5 A proposal's identity is its cause, not its moment

A proposal's identity is the canonical digest of: the subscription id, the
proposal kind, the scope, and the projection row that justified it (for
`run`: the eligibility row and its closure digest; for a `notice`: the two
values the transition compared). The scope is tagged: `spec` with the spec id
for a keyed projection, or `repository` for `corpus-state`, which is keyed by
nothing, so a `corpus-refusal` notice has a defined identity and no spec id
can collide with it. The observation id, the commit id and the
time are **not** in the identity.

Consequences, each intended:

1. The same semantic fact reached by different commits (a documentation-only
   commit between two observations, a rebase that preserves the corpus) yields
   the same proposal id, and it is not proposed again: the second observation
   writes `recurred` against the proposal already made.
2. A spec that exits and re-enters eligibility under **the same** contract
   yields the same id. If that proposal is still `open`, the recurrence is
   recorded as `recurred` and the proposal stays `open`, so its latest ref is
   the one admission re-observes (section 3.6). If it was `admitted` or `dismissed` (a
   `notice`: `acknowledged`), the earlier disposition stands and nothing new
   is proposed: the same work under the same contract was already decided. If
   it was `stale` or `superseded`, nobody decided the work; the recurrence is
   recorded as `recurred`, the proposal is `open` again, and an open `run`
   proposal for the same spec under another contract is `superseded` by it
   (section 3.8).
3. A spec that re-enters under a **different** contract yields a new id,
   because the work it offers is not the same work.

A proposal records, beside its identity, every observation that produced it,
so its first cause and each later recurrence stay readable.

### 3.6 Preconditions are re-derived at admission

A `run` proposal is admitted only by `proposal admit` (section 3.9), an
operator act naming the operator and a reason. Admission:

1. performs a fresh observation of the ref recorded on the proposal's latest
   `proposed` or `recurred` entry (section 3.8);
2. refuses, writing the disposition `stale` with the differing value, when
   the fresh observation is `refused`, when the spec is not `eligible` in it,
   or when its closure digest differs from the proposal's;
3. otherwise writes `admitting`, then enters spec 003's run path for that spec, at the fresh
   observation's commit, with the lock, workspace, lifecycle policy, override
   and contract binding that `run` applies, none of them bypassed or
   pre-decided by the proposal.

`stale` is a disposition, not a failure: the proposal was right when it was
made and the repository moved. A refusal from spec 003 (a live attempt holds
the lock; a capability token is absent) is reported as that refusal, and the
admission writes `released` with reason `run-refused`, which returns the
proposal to `open`; nothing about the proposal was wrong.

**A retry is `run`, and a dismissal blocks nothing.** Admission is the only
way a proposal causes a run, and it causes at most one: a proposal disposed
`admitted` is never admitted again. Retrying that work, after any outcome, is
spec 003's `run`, which appends an attempt under spec 003's rules and
needs no proposal. `run` does not read the observation chain, so no
disposition here, `dismissed` included, ever blocks, delays or conditions a
`run`; a dismissal only stops this spec proposing the same work under the
same contract again (section 3.5).

### 3.7 The run record names its cause

The attempt intent gains one optional member naming the admitting proposal's
id, written once, with the intent, before any effect (spec 003 section 3.3,
the intent-before-effect rule). An intent without it is exactly today's
intent, and spec 003's readers treat its absence as "admitted by `run`".

This is the whole of the extension to spec 003. It gives `run show` and
`proposal show` one join in each direction, and it is what makes the crash
analysis of section 3.8 exact.

### 3.8 The observation chain

Everything this spec records is one append-only, hash-linked chain per
repository **in the product home**, filed under spec 003's repository key,
using the construction and durability rules of spec 003's run record: fsync
before acknowledgment, a torn tail reported and appended after, earlier
entries never rewritten. It does not live under the target's
`.statecraft/state/`: spec 003 section 5 (2026-09-16) places run history
outside the target so the supervised process cannot reach it, and the chain
that causes runs is held to the same rule.

Entry kinds, closed:

| Kind | Carries |
|---|---|
| `observed` | observation id, input identity (section 3.1), the ref as named and the commit it resolved to, `answered` or `refused`, projection digests, and the transitions |
| `proposed` | proposal id, kind, scope (section 3.5), subscription, justifying row, observation id, ref |
| `recurred` | proposal id, observation id, ref (section 3.5 rule 1) |
| `admitting` | proposal id, fresh observation id, operator, reason |
| `released` | proposal id, the `admitting` entry it ends, and one of `run-refused` (section 3.6) or `admission-interrupted` (crash analysis below); the proposal is `open` again, and it is not a disposition |
| `disposed` | proposal id, one of `admitted` (with run id and attempt), `stale` (with the differing value), `dismissed` (operator, reason), `acknowledged` (operator), `superseded` (by which proposal) |

**There is no mutable checkpoint.** "The last answered observation" is the
last `observed` entry with `answered`, read from the chain. A checkpoint file
beside the chain would be a second copy of a fact the chain already records,
and the two could disagree after a crash.

**Crash analysis.** An admission that enters the run path writes, in order,
`admitting`, then spec 003's intent (section 3.7), then `disposed`. An
admission that ends `stale` never writes `admitting`: it writes `disposed:
stale` directly after the fresh observation, under the same lock hold, and
has no effect to recover. A process that dies after `admitting`:

- with no attempt intent naming the proposal: no effect happened, because
  spec 003 writes its intent before any effect. Recovery writes `released`
  with reason `admission-interrupted`, so the proposal is `open` again and may
  be admitted under a new admission, which re-derives its preconditions as
  any admission does (section 3.6).
- with an attempt intent naming the proposal: recovery writes
  `disposed: admitted` naming that run and attempt. Whether the attempt's own
  effect happened is spec 003's question, answered by spec 003's recovery,
  never by this chain.

So the observation chain holds no unknown state of its own; every ambiguity it
could have is delegated to the record that already owns it.

**Recovery runs at the next chain writer.** An `admitting` entry with no
later `released` or `disposed` for its proposal is orphaned only if its
writer died, because a live admission holds the observation lock until it
returns. So every verb that appends to the chain, once it holds the lock and
before its own work, recovers each orphaned `admitting` as above. A
`proposal admit` naming an orphaned proposal therefore sees it `open` again
(or `admitted`) before section 3.6 begins. The read-only verbs write nothing
and report an orphaned `admitting` as `admission-interrupted, not yet
recovered`.

**Concurrency.** One writer per chain, held by a lock file in the product home
beside the chain, distinct from spec 003's repository lock: an observation
does not wait for a live attempt, and a live attempt does not block
observation. Admission takes spec 003's lock through the run path, in that
order only (observation lock, then repository lock), so the two cannot
deadlock.

The observation lock is not reentrant and is taken once per verb. Every verb
that appends to the chain (`observe`, `proposal admit`, `proposal dismiss` and
`proposal acknowledge`) acquires it at entry and holds it until it returns, and
the read-only verbs take no lock;
the observation itself is one library operation that requires the lock to be
held by its caller and never acquires it. Admission's fresh observation
(section 3.6, step 1) is that same operation called under admission's own
hold, so it neither waits on itself nor runs unlocked, and no other writer
can append between the fresh observation and the disposition.

A `run` proposal still `open` when a newer proposal for the same spec is made
under a different contract is `superseded` by it, so at most one `run`
proposal per spec is open at a time.

### 3.9 Verbs

Each verb takes the target path first, accepts `--json` in the family
envelope (spec 007), calls exactly one library operation, and follows spec
006's exit contract.

| Verb | Does |
|---|---|
| `observe <path> [--ref <ref>]` | One observation (sections 3.1 to 3.5). Prints the observation id, `answered`, `refused` or `unchanged`, the transitions, and each proposal made or recurred. |
| `observe show <path> [--ref <ref>]` | The last answered observation, and whether the ref (resolved with `observe`'s default, section 3.1) now resolves to a commit the chain has no `observed` entry for. Writes nothing. |
| `proposal list <path>` | Open proposals, in the order they were first proposed. Writes nothing. |
| `proposal show <path> <id>` | The causal trace: every observation that produced it, the transition and the two projection values, the subscription, each disposition, and for `admitted` the run and attempt with their outcome and acceptance state as spec 003 and spec 005 report them. For each transition it cites spec-spine's `delta` report between the two compared commits, verbatim, as the explanation of what changed; it never derives one from this spec's projections, and a refused or absent report is shown as `unavailable` with spec-spine's reason. An `entered (initial)` transition has no earlier commit, so it cites no `delta` and is shown as `initial`, naming the one commit observed. Writes nothing. |
| `proposal admit <path> <id> <operator> <reason...>` | Section 3.6. |
| `proposal dismiss <path> <id> <operator> <reason...>` | Closes a `run` proposal without running it. |
| `proposal acknowledge <path> <id> <operator>` | Closes a `notice`. |

Exit codes follow spec 006: `observe` exits 0 for `answered` or `unchanged`
and 1 for `refused` (a finding about the target, not a failure of this
product); `proposal admit` exits as `run` would when admitted, 1 when the
disposition is `stale`, and 2 when spec 003 refuses; `proposal dismiss` and
`proposal acknowledge` exit 0 when they write their disposition, and the
read-only verbs exit 0 when they answer; a verb that finds the
observation lock held is refused, exit 2, and writes nothing; a verb whose
write to the chain fails exits 4.

Exit 2 is spec 006's one refusal outcome, not one condition, and it is
never read alone. The two refusals `proposal admit` can meet are told apart
the way spec 007 tells every refusal apart: a held observation lock is class
`refused`, names the lock, and writes nothing; a spec 003 refusal carries the
class spec 003 gives it, names spec 003's reason, and writes `released` with
reason `run-refused` (section 3.6).

There is no verb that observes repeatedly, admits automatically, or admits more
than one proposal. `proposal admit` is the operator naming one unit of work,
which is the only scheduling policy spec 006 section 4 permits.

### 3.10 Internal seams

The crate's observation code depends on three narrow interfaces, so that the
comparison, matching and proposal logic is a pure function of values and is
tested without git, spec-spine or a product home:

- a **commit source**: resolve a ref, materialize a commit read-only;
- a **report source**: the structured spec-spine answers for a materialized
  tree (production: spec 003's reader over the `spec-spine` on `PATH`);
- an **observation store**: append and read the chain.

These are internal. None is a public extension point, because no second
implementation exists; a successor that adds one (a wake source, a different
semantic provider) makes the interface public in its own change and states its
contract there.

### 3.11 Observable negative cases

| Case | Required behavior |
|---|---|
| The ref does not resolve | Refused, exit 2, naming the ref. Nothing recorded. |
| The target is not registered | Refused, exit 2, as every verb taking a target path refuses. |
| spec-spine reports the observed commit's ledger stale | `refused` recorded with spec-spine's class; no eligibility or contract projection; a `corpus-refusal` notice when the previous observation was answered or refused with another class, and no `run` proposal; exit 1. |
| A report lacks a field a projection needs | Refused naming the field and version; nothing recorded; exit 2. |
| An observation whose input identity equals the last one | `unchanged`, nothing recorded, exit 0. |
| The operator checkout has uncommitted edits to a spec | Not observed. The observation is of the commit, and nothing is recorded about the working tree. |
| A refused observation between two answered ones | The transition spans the refusal; no spec is reported as having exited and re-entered. |
| The same eligibility reached by two commits | One proposal; the second observation writes `recurred`. |
| A spec re-enters eligibility under the same contract after its proposal was dismissed | No new proposal; the dismissal stands and is named. |
| A spec re-enters under a different contract | A new proposal; any open one for that spec is `superseded`. |
| `proposal admit` after the spec left eligibility | `stale`, naming the exclusion reason; exit 1; no run. |
| `proposal admit` after the contract moved | `stale`, naming both digests; exit 1; no run. |
| `proposal admit` when the fresh observation is `refused` | The `observed` entry records the refusal; `stale`, naming spec-spine's class; exit 1; no run. |
| `proposal admit` while an attempt is live | spec 003's refusal, exit 2; the admission writes `released`, reason `run-refused`, and the proposal is `open`. |
| `proposal admit` on a `notice`, or on a disposed proposal | Refused, exit 2, naming its kind or disposition. |
| `proposal dismiss` on a `notice`, or `proposal acknowledge` on a `run` proposal | Refused, exit 2, naming its kind and the verb that closes it. |
| The process dies after `admitting` and before the attempt intent | Recovery writes `released`, reason `admission-interrupted`; the proposal is `open` again; no run is replayed. |
| The process dies after the attempt intent and before `disposed` | Recovery disposes `admitted`, naming the run found by the intent's proposal member. |
| Two `observe` invocations on one repository at once | The second is refused, exit 2, naming the held observation lock. |
| The chain's tail is torn | Read to the last complete entry, the torn tail reported, appended after. |
| A target file declaring subscriptions | Ignored as a subscription source; the set of section 3.4 is the only one. |
| Any file under `.statecraft/derived/` | Never opened by this crate. |

## 4. Out of scope

Each is deferred by name, with the condition that reopens it.

- **Wake sources and a watch loop** (a filesystem watcher, a ref poller, a
  long-running `observe --watch`). The product recovers no standby daemon
  (spec 001 section 3.9). Reopens as its own spec once the owner decides a
  foreground loop is not that daemon; it may only call this spec's `observe`,
  and the correctness of every observation must not depend on a wake arriving.
- **Automatic admission of any proposal.** Deferred under `F-05` (adaptive
  autonomy). Reopens as its own spec that states the bound (per repository, per
  chain of causes, per interval) and records the policy it was admitted under.
- **Any ordering, draining or batching of proposals.** `F-10` defers any work
  queue; open proposals are an unordered set the operator picks from, one at a
  time.
- **Configurable subscriptions, and any query language over projections.**
  Reopens when a fourth built-in subscription is needed and a declaration can be
  read at a trusted base the way spec 003 reads the lifecycle policy.
- **Incremental projection.** Every observation recomputes all three
  projections. Reopens on a measured observation cost, never in anticipation.
- **Observation of a candidate or workspace.** Only the operator's repository
  at a named ref is observed; a candidate is judged by spec 005.
- **Consuming spec-spine's authority snapshot.** `attest --snapshot` writes into
  the target's derived directory, so it is not a read, and linking
  `spec-spine-core` for a runtime answer would change how this product consumes
  spec-spine for a target (spec 003 reads the CLI on `PATH`). Reopens if
  spec-spine offers a non-writing snapshot report.
- Changes to spec-spine of any kind; cross-repository observation; any user
  interface beyond command output (`F-04`).

## 5. Resolved decisions

**2026-09-28: "proposal" and "observe", not "intent" and "reconcile".** The
run record already has an intent (the half of the intent/outcome bracket) and
spec 003 already has reconciliation (of an attempt whose effect is unknown).
Reusing either word for this spec's objects would make every reader of a run
record disambiguate by context.

**2026-09-28: observation is of commits, not working trees.** Every spec-spine
answer consumed here is about a committed ledger, `registry closure` refuses a
stale one, and a working tree has no identity to record. Observing uncommitted
edits would make a proposal's cause unreproducible.

**2026-09-28: the projections compare spec-spine's answers, not its types.**
spec-spine's `AuthorityDelta` is the per-path ownership movement inside the
commit-pair `delta` report (its spec 071), not a difference between two
snapshots, so it is not a transition over repository state and is not used as
one here.

**2026-09-29: an interrupted or refused admission is released, not disposed.**
The first draft disposed an interrupted admission as `stale` and then called
the proposal admissible, which the closed dispositions could not express. A
`released` entry now ends an `admitting` entry without deciding the proposal:
it is `open` again, and the next admission re-derives its preconditions.
`contract-moved` covers a move to or from `unavailable`, so the
`moved-contract` notice fires when a bound contract stops resolving.

**2026-09-29: `corpus-state` compares with the previous observation, and the
ref is recorded.** A refused observation has only the `corpus-state`
projection, compared with the observation immediately before it, so
`corpus-refusal` fires on the first refusal and not again until an answer
intervenes. The ref is not in the input identity, because the same commit
reached by two refs is the same answer, but it is recorded on `observed`,
`proposed` and `recurred`, and admission re-observes the ref of the
proposal's latest cause.

**2026-09-29: a stale or superseded proposal reopens on recurrence, and a held
lock is a refusal.** Only `admitted`, `dismissed` and `acknowledged` are
decisions about the work; a proposal made `stale` or `superseded` by the
repository's movement is `open` again when its identity recurs. A held
observation lock is exit 2, as section 3.11 states, and exit 4 is kept for a
write that failed.

**2026-09-29: a repository-wide notice has a scope, not a spec id, and exit 2
is told apart by class.** `corpus-state` is keyed by nothing, so proposal
identity digests a tagged scope instead of a spec id. Exit 2 stays spec
006's single refusal outcome; which refusal it was is the envelope's class
and message and whether a `released` entry was written, as spec 007 intends,
rather than a new exit code.

**2026-09-29: the observation lock is held once, by the verb.** Admission
contains an observation, so the observation operation requires a held lock
instead of taking one; `observe show` takes the same `--ref` and default as
`observe`, so "the ref" in it is never a guess.

**2026-09-29: every recurrence of an open or reopened proposal writes
`recurred`.** Only a decided proposal (`admitted`, `dismissed`,
`acknowledged`) records nothing on recurrence; an `open` one records the
recurrence, which keeps the ref admission re-observes current.

**2026-09-29: every chain writer holds the lock; explanations are
spec-spine's; a retry is `run`.** `proposal dismiss` and `proposal
acknowledge` append, so they take the observation lock like the other two
writers. `proposal show` explains a transition with spec-spine's `delta`
report rather than a locally derived diff. The chain never gates `run`.

**2026-09-29: `admitting` is written only on the way into the run path, and
recovery runs at the next writer.** A stale admission has no effect, so it
needs no `admitting` entry; an orphaned `admitting` is recovered by whichever
verb next takes the observation lock, which is also the only time the orphan
can be told from a live admission.

**2026-09-29: an initial transition cites no `delta`, and the closing verbs
exit 0.** The first observation has no second commit to compare, so
`proposal show` names it `initial` instead of inventing a base.

## Verification

```verify:cli
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
spec-spine index coverage --fail-on-untraced
test -f crates/statecraft-observe/src/lib.rs
test -f crates/statecraft-observe/tests/negative_cases.rs
cargo test -p statecraft-observe
```
