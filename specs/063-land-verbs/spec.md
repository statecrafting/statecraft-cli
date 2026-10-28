---
id: "063-land-verbs"
title: "The land verbs wait for, arm, push and restack a pull request, and land status orders what is left"
status: approved
# Ratified by the owner on 2026-10-27 (section 7); this frontmatter is the
# status spec-spine and the coupling gate read.
implementation: complete
created: "2026-10-27"
summary: >
  Amends spec 006's command surface; its four write verbs act under the F-02
  narrowing of 2026-10-27 and never inside a run attempt (D-5). Six verbs
  act on a pull request's way to the default branch through the operator's
  gh: `land wait` blocks until a named event, `land arm` enables auto-merge
  only for a head the AI review passed on a pull request based on the
  default branch, `land push` pushes a signed head with a lease after
  disarming, `land restack` moves a stacked pull request onto its new base
  and regenerates only declared generated paths, and `land status` orders
  the open pull requests by their next action, and `land cancel-superseded`
  cancels runs no open pull request needs. The adopter's values (default
  branch, stack caps, generated paths and their regenerators, pre-push
  checks) come from a `landing` block in the project declaration; a value no
  layer supplies is refused, never defaulted. Only `land arm` arms, and
  nothing here approves, merges or re-runs.
amends:
  - "006-command-surface"
establishes:
  - { kind: directory, path: "crates/statecraft-land/" }
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-environment/" }, nature: additive }
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "002-environment-lifecycle"
  - "006-command-surface"
  - "012-remote-desired-state"
  - "062-review-verdict-status"
# `obligations` is spec-spine's own frontmatter field, used by approved specs
# such as 013, 014 and 018 (and 062); `make gate` lints it with --fail-on-warn.
obligations:
  - id: "R-1"
    kind: requirement
    text: "Every land verb reads the adopter's values from the `landing` block of the project declaration through the configuration layers `config show` reports; a value a verb needs that no layer supplies refuses the verb (exit 2) naming the key, and no verb substitutes a default."
    anchor: "3-1-the-landing-policy"
  - id: "R-2"
    kind: requirement
    text: "`land wait` reads each named pull request on its own, never a page of the merge queue, writes nothing, and ends at the first named event, at `--timeout`, or on a read that keeps failing past it."
    anchor: "3-2-land-wait"
  - id: "R-3"
    kind: requirement
    text: "`land arm` first reads whether the pull request is already armed: armed after its current head was pushed is exit 0, `already-armed`, and armed before it is refused (exit 2), `armed-before-head`; neither calls GitHub's arming and both change nothing. Otherwise it enables auto-merge only when the pull request is open and not a draft, its base is the default branch, its head is the one named by `--head` when given, `review status` at that head is `no-findings`, no required check at that head has ended in anything but success (failed, cancelled, skipped, timed out or needing action all refuse, as they fail ci-gate; one still queued or running does not), and the head is signed when the profile requires signed commits; it arms with the head pinned, and refuses (exit 2) naming the first unmet condition."
    anchor: "3-3-land-arm"
  - id: "R-4"
    kind: requirement
    text: "No verb other than `land arm` enables auto-merge, and no land verb approves an Environment, merges, re-runs a job or comments."
    anchor: "3-3-land-arm"
  - id: "R-5"
    kind: requirement
    text: "`land push` pushes only the pull request's own head branch, only a head that is signed when the profile requires it and holds no conflict marker in a tracked file, disarms an armed pull request before the push and reports it, and pushes with a lease on the remote head the caller names; it never re-arms."
    anchor: "3-4-land-push"
  - id: "R-6"
    kind: requirement
    text: "`land restack` replays exactly the pull request's own commits onto the new base, resolves a conflict only on a path the `generated` list declares (by taking the base's copy and running that entry's regenerator), aborts on any other conflicted path without pushing, and refuses to push when the pull request's authored change per file differs after the replay."
    anchor: "3-5-land-restack"
  - id: "R-7"
    kind: requirement
    text: "`land status` writes nothing."
    anchor: "3-6-land-status"
  - id: "R-8"
    kind: requirement
    text: "`land cancel-superseded` force-cancels only pull-request runs waiting on the review-exception Environment whose head no open pull request points at, and nothing else."
    anchor: "3-7-land-cancel-superseded"
  - id: "R-9"
    kind: requirement
    text: "`land arm`, `land push`, `land restack` and `land cancel-superseded` refuse (exit 2), `inside-run-attempt`, and write nothing when `STATECRAFT_RUN_ID` or `STATECRAFT_ATTEMPT` (spec 015 section 3.31 rule 17's attempt binding) is set in their environment."
    anchor: "3-9-never-inside-a-run-attempt"
  - id: "V-1"
    kind: verification
    text: "CLI tests run every land verb against a stub gh and a scratch repository and find every outcome and exit code in sections 3 and 5, including an unsupplied policy key, a queue longer than one page, a skipped review, a stacked base, a moved head, an armed pull request pushed to, a non-generated conflict, a regenerated path, and an authored change altered by a replay."
    anchor: "verification"
    inputs:
      - "crates/statecraft-cli/tests/land.rs"
---

# 063: The land verbs

## 1. Purpose

Spec 062 gives the AI review's result one machine-readable place and one
reader. What an operator does with that result (wait for it, arm the pull
request, push a fix, move a stacked pull request after its base merges,
decide what to do next) has no verb, so every adopter writes it.

At rustev, the first adopter, those steps were written as shell scripts
across delivery sessions between 2026-10-16 and 2026-10-27: about two dozen
helpers and 48 copies of one restack function, kept in session scratchpads
and a handoff directory. A second machine joining the work had none of them.
The copies drifted, and each defect recurred in some of them:

1. A wait read the first 20 merge-queue entries, so a queued pull request in
   a longer queue read as dropped.
2. The arming wrapper never checked the base, so it would arm a stacked pull
   request, against the adopter's rule.
3. One restack regenerated schemas through a build lock that skipped the
   clean the shared build target needs, and served another worktree's
   artifacts; another resolved conflicts by a blind union and did not check
   the commit signature or leftover markers.
4. The landing report hardcoded one stack cap after the owner had set
   different caps for different chains, so it flagged nothing it should have.

A verb with one owning spec fixes each of these once, for every adopter and
every machine, and states what is policy (the adopter's values) apart from
what is rule (this spec's).

## 2. Territory

Six verbs under `land` in `crates/statecraft-cli/` (spec 006's command
tree, amended here), bound to library operations in a new crate,
`crates/statecraft-land/`, which this spec establishes and owns (decision
D-4), and tests in both.

## 3. Behavior

Every verb reads GitHub through the operator's `gh`, as `doctor --remote`
does (spec 012), and holds no token. `--repo <owner/name>` defaults to the
repository of the current directory's `origin`. Every verb supports
`--json` (spec 006 section 3.4), and its JSON carries a `policy` object with
each `landing` value it used and the layer that supplied it.

### 3.1 The landing policy

1. The adopter's values live in a `landing` block of the project
   declaration, `.statecraft/environment.json`, beside `setup`. They are
   read through the same layers `config show` reports (spec 002), so
   `config show landing.<key>` answers where each came from.
2. The keys:

   | Key | Used by | Holds |
   |---|---|---|
   | `default_branch` | all | The branch pull requests land on. |
   | `stack.depth` | `status` | The cap on unmerged pull requests beneath one pull request. |
   | `stack.depth_by_spec` | `status` | Per-spec caps that differ from `stack.depth`, keyed by spec id. |
   | `stack.max_stacked` | `status` | The cap on stacked pull requests in total. |
   | `generated` | `restack` | A list of `{paths, regenerate}`: path globs and the argument vector that regenerates them. |
   | `pre_push` | `push`, `restack` | A list of argument vectors run in the worktree before a push. |

3. A verb that needs a key no layer supplies is refused with exit 2, naming
   the key. Nothing defaults: an empty `pre_push` is a stated value, an
   absent one is not.
4. These are values, not rules. Which conditions arm a pull request, which
   conflicts may be resolved and what a cap means are fixed here; the
   adopter supplies only the numbers, paths and commands.
5. A `generated` path should also be excluded from the AI review
   (`review.exclude`), since it is checked against its source instead.
   `land status` reports a `generated` path the review still reads as a
   finding.

### 3.2 `land wait`

```
statecraft-cli land wait <pr>... --until <event> --timeout <duration> [--json]
```

1. Events:

   | Event | Met when a named pull request |
   |---|---|
   | `merged` | merged (exit 0) or closed unmerged (exit 1). |
   | `landed` | merged, closed, or is open, not armed and not in the merge queue (`dropped`). |
   | `queue-exit` | is no longer in the merge queue, with its state. |
   | `reviewed` | has a `review status` (spec 062) other than `pending` at its current head. |
   | `changed` | differs from its state at the start in any of: state, head, armed, queue state, review outcome. |

2. It reads each pull request on its own, the merge-queue entry through the
   pull request (`mergeQueueEntry`), never a page of the queue.
3. It reads at most once a minute per pull request, and backs off on a
   rate-limit answer.
4. It ends at the first pull request that meets the event and reports that
   pull request's observation. `--timeout` is required; reaching it is exit 1,
   `timed-out`, with the last observation of each pull request.
5. It writes nothing.

### 3.3 `land arm`

```
statecraft-cli land arm <pr> [--head <sha>] [--json]
```

1. It first reads whether the pull request is already armed, before any
   other check and before any write. Armed after its current head was
   pushed is exit 0, `already-armed`. Armed before its current head was
   pushed is refused, exit 2, `armed-before-head`: auto-merge stays enabled
   across a push, so that arming did not see this head, and `land push`
   disarms first to prevent it. In both cases GitHub's arming is not called
   and nothing changes.
2. Otherwise it checks, in this order, and refuses (exit 2) at the first
   that fails, naming it:

   | Condition | Refusal |
   |---|---|
   | The pull request is open and not a draft. | `not-open`, `draft` |
   | Its base is `landing.default_branch`. | `stacked`, naming the base |
   | Its head is `--head`, when given. | `stale`, naming the current head |
   | `review status` at that head is `no-findings`. | `review-<outcome>`, for example `review-skipped:transient` |
   | No required check at that head has ended other than in success: failed, cancelled, skipped, timed out and action required all count, as each fails ci-gate. A check still queued or in progress does not refuse; auto-merge waits for it. | `check-failed`, naming the check and its conclusion |
   | The head is signed, when `governance.require_signed_commits` is set. | `unsigned` |

3. Only when every condition holds does it enable auto-merge, with the head
   pinned (`--match-head-commit`), by the repository's merge method; with a
   merge queue, the queue's.
4. It is the only verb that enables auto-merge, and only when called. No
   land verb approves an Environment, merges, re-runs a job or comments.

### 3.4 `land push`

```
statecraft-cli land push <pr> --lease <sha> [--json]
```

Run in a worktree whose branch is the pull request's head branch.

1. It refuses (exit 2): another branch checked out (`wrong-branch`); an
   unsigned head when the profile requires signed commits (`unsigned`); a
   conflict marker in a tracked file at the head (`markers`).
2. It runs each `landing.pre_push` vector in order and refuses at the first
   that fails (`pre-push-failed`, naming it, with its exit code). There is
   no flag to skip them: an adopter that wants none declares
   `pre_push: []`, a committed value `config show` reports (constitution
   VIII: no flag whose only effect is to remove a guard).
3. If the pull request is armed it disarms it first and reports `disarmed`.
4. It pushes the head with a lease on `--lease`. A refused lease is exit 2,
   `lease-refused`, with the remote head.
5. It never re-arms. After a push the head needs a new review (spec 062),
   and arming it is a separate `land arm`.

### 3.5 `land restack`

```
statecraft-cli land restack <pr> [--onto <branch>] [--json]
statecraft-cli land restack --chain <pr>... [--json]
```

Run in a clean worktree whose branch is the pull request's head branch.

1. The commits replayed are the pull request's own, as GitHub lists them
   for the pull request, never a local range guess. `--onto` defaults to the
   pull request's base, or to `landing.default_branch` when that base has
   merged.
2. A conflict on a path that a `generated` entry declares takes the base's
   copy. The entry's `regenerate` vector then runs once the commit is
   replayed. A conflict on any other path aborts the replay, restores the
   branch, pushes nothing and is exit 1, `conflict`, listing each path. The
   verb never merges authored text.
3. Every replayed commit is regenerated and checked at its own tree, and the
   regenerated output is amended into that commit, signed when the profile
   requires signed commits (`governance.require_signed_commits`), as
   `land push` and `land arm` require it. A repository that
   gates every commit (`governance.gate_each_commit`) stays green commit by
   commit.
4. The pull request's authored change (lines added and removed per path
   outside `generated`) must be the same after the replay as before. A
   difference is exit 1, `authored-change-differs`, naming each path; the
   result stays local for inspection and the original head is reported.
   This is the rule of this repository's `scripts/resolve-shard-conflicts.sh`,
   which takes the incoming shards and then refuses a merge that changed the
   branch's authored lines.
5. Then it pushes as `land push` does (pre-push checks, disarm, lease on the
   remote head read before the replay) and retargets the pull request to
   `--onto` if its base differs.
6. `--chain` restacks the listed pull requests bottom first, each onto the
   head of the one before, and stops at the first that does not exit 0.
7. Derived artifacts (`.statecraft/derived/`) are an ordinary `generated`
   entry whose regenerator is the repository's refresh. When the pinned
   spec-spine provides a derived-conflict verb, the adopter names it as that
   entry's regenerator; nothing here depends on it.

### 3.6 `land status`

```
statecraft-cli land status [--json]
```

1. It reads the open pull requests, their stacks, each head's required
   checks, `review status` (spec 062), runs waiting on the
   `statecraft-review-exception` Environment and `spec-spine registry plan`,
   and lists one next action per pull request, most urgent class first:
   `retarget`, `approve` (the owner's, with the reason), `fix`, `update`,
   `arm`, `hazard`, `running`, `merging`, `stacked`, `draft`.
2. A pull request's depth is the number of unmerged pull requests beneath
   it, 0 when it targets the default branch. Its cap is
   `stack.depth_by_spec` for its spec, else `stack.depth`. Past either cap it
   names the bottom of the stack to land first.
3. It reports the delivery measures rustev's report carries: approval wait,
   oldest blocked pull request, and per depth the time to land after the
   base merged and the reviews per pull request.
4. It writes nothing. Cancelling the runs it reports as superseded is
   `land cancel-superseded` (3.7).

### 3.7 `land cancel-superseded`

```
statecraft-cli land cancel-superseded [--json]
```

It force-cancels the pull-request runs waiting on the
`statecraft-review-exception` Environment whose head no open pull request
points at (a plain cancel is accepted and does nothing for a waiting run),
and reports each run cancelled and each it could not cancel. A run of
another event, such as a release dispatch waiting on its own Environment, is
never cancelled. It is a separate verb so that `land status` stays a read
(decision D-3).

### 3.8 Exit codes

Spec 006 section 3.3's vocabulary. 0: the verb did what was asked (a wait's
event met, armed, pushed, restacked, a status read, the superseded runs
cancelled). 1: a finding, nothing
failed (closed unmerged, `timed-out`, `conflict`,
`authored-change-differs`, a status with any `fix` or `hazard`). 2: refused,
nothing done (an unsupplied policy key and every refusal named above).
3: usage. 4: failed, including a `gh` read classified as spec 012 section 3.5
classifies it, from the error stream alone.

### 3.9 Never inside a run attempt

`land arm`, `land push`, `land restack` and `land cancel-superseded` act on
GitHub, which founding deferral `F-02` withholds from every product verb
except these four, lifted on 2026-10-27 (decision D-5). Each reads its
environment before any other read or write: when `STATECRAFT_RUN_ID` or
`STATECRAFT_ATTEMPT` is set (the attempt binding of spec 015 section 3.31
rule 17, which a driven session carries), it refuses, exit 2,
`inside-run-attempt`, and writes nothing. `land wait` and `land status`
publish nothing and do not check it. The check is not a security boundary
(constitution VIII): a driven session that holds the operator's `gh` can
call it directly; the founding decision record names that residual.

## 4. Out of scope

- Re-running a review or a failed job, and deciding that a cancellation was
  a hang. A later spec may add a `review rerun`; spec 061 already makes a
  stalled call a visible `skipped:transient`.
- Approving the review-exception Environment, which is the owner's act.
- Merging, and choosing a merge method other than the repository's.
- Running a coding agent, and the build lanes such an agent uses (spec 055
  and later specs).
- Any write to GitHub not named in section 3.

## 5. Observable negative cases

| Case | Expected |
|---|---|
| `landing.default_branch` supplied by no layer | Every verb refused, exit 2, naming the key. |
| `land wait --until landed` on a pull request at position 25 of the queue | Still waiting; never `dropped`. |
| `land arm` on a pull request whose base is another pull request's branch | Refused, `stacked`, exit 2; not armed. |
| `land arm` when the head's review is `skipped:draft` | Refused, `review-skipped:draft`, exit 2. |
| `land arm` when a required check at the head ended `cancelled` or `skipped` | Refused, `check-failed`, naming the check and its conclusion, exit 2. |
| `land arm` on a pull request armed before its current head was pushed | Refused, `armed-before-head`, exit 2. |
| `land push` on an armed pull request | Disarmed, then pushed; output names both; still disarmed. |
| `land push` with an unsigned head where signed commits are required | Refused, `unsigned`, exit 2; nothing pushed. |
| `land restack` with a conflict in a generated schema and in a source file | Aborted, `conflict` naming the source file, exit 1; branch restored; nothing pushed. |
| `land restack` with a conflict only in `.statecraft/derived/` | Base copy taken, refresh run, amended signed, pushed with lease, retargeted, exit 0. |
| A replay that drops one of the pull request's authored lines | `authored-change-differs`, exit 1, naming the path; nothing pushed. |
| `land status` with a 041 chain two deep, `stack.depth` 1 and no 041 entry in `stack.depth_by_spec` | The chain's bottom named to land first. |
| `land cancel-superseded` with a waiting release dispatch run | Not cancelled. |
| `land status` with superseded runs waiting | Reported; nothing cancelled. |

## 6. Delivery

Four implementing pull requests, each near 1,000 reviewed lines (the AI
review's 20-minute limit): (1) the policy block and `land wait`; (2) `land
arm` and `land push`; (3) `land restack`; (4) `land status` and `land
cancel-superseded`, ported from rustev's `scripts/landing/status.py`. The
first establishes `crates/statecraft-land/`. Each claims its paths and adds its own
acceptance. (1) depends on spec 062 for `--until reviewed` only, and can
ship before 062 without that event.

## 7. Resolved decisions

**2026-10-27: ratified by the owner.** The owner decided all four below and
directed ratification in this pull request; the owner exception approved on
its run is the ratification of record.

**D-1: the policy lives in a `landing` block (3.1),** beside `setup` in the
project declaration, not in new setup-profile parameters. The rendered CI
never reads it, so changing a cap or a regenerator is an ordinary commit,
with no re-render and no owner exception.

**D-2: `land push` runs `pre_push` (3.4).** The adopter decides what it
holds; it is meant for the seconds-long governance checks, never the slow
build and test, so pushes still do not wait on local builds. No flag skips
it (constitution VIII); an empty list is the adopter's committed choice.

**D-3: cancelling superseded runs is its own verb (3.7),** not a flag on
`land status`, so the report stays strictly a read and every write is an
explicit act.

**D-4: the operations live in a new crate, `crates/statecraft-land/` (2),**
which this spec establishes and owns, one owning spec per crate; spec 006's
`crates/statecraft-cli/` only binds the verbs.

**D-5 (2026-10-27): the four write verbs are lifted from `F-02` and never
run inside a run attempt.** The founding decision record (section 5,
2026-10-27) narrows `F-02`, which defers every publishing product verb, for
exactly `land arm`, `land push`, `land restack` and `land cancel-superseded`.
Each checks the attempt binding of spec 015 section 3.31 rule 17 before any
read or write and refuses, exit 2, `inside-run-attempt`, when
`STATECRAFT_RUN_ID` or `STATECRAFT_ATTEMPT` is set (R-9). The check is not a
security boundary (constitution VIII): a driven session holding the
operator's `gh` can call it directly, as the record names. The read verbs
need no lift. The implementing change of `land push` updates
`integration_slice`'s guard against publishing verbs to name exactly this
lift, rather than removing the guard.

## Implementation record

**2026-10-27, increment 1: the policy and `land wait`.** The operations live
in `statecraft-land` (D-4); `land wait` is a binding (006 3.2) to
`statecraft_land::wait`, which writes nothing.

- **`landing` on `Project`.** The project declaration's `Project` gains a
  `landing` member, held as an opaque JSON value and written back as read.
  Without it, every rewrite of `environment.json` (init apply, a transfer)
  would drop the block, since the declaration is rewritten from its typed
  form. `statecraft-land` validates it; `statecraft-environment` cannot
  depend on that crate.
- **`config show` reads it.** `statecraft-home`'s resolver projects each key
  (`landing.default_branch`, `landing.stack.depth` and the others of 3.1) as a
  project-layer value, strings as text and structured values as compact JSON,
  so `config show landing.<key>` answers its value and its layer (3.1.1).
  The same projection feeds the policy reader, so the two cannot disagree on
  a supplied key. The values are the project declaration's alone (D-1): a
  personal default is not landing policy, and the reader takes the committed
  declaration at `HEAD`, so an uncommitted edit does not change what a verb
  does.
- **Observations are an array.** The answer's `observations` lists one
  entry per requested pull request in the order given, each carrying its
  `pullRequest` number, never an object keyed by number: spec 007's family
  envelope keeps every member name a fixed camelCase identifier.
- **The clock is injected.** Tests drive the minute interval and the
  rate-limit backoff through a clock the library takes; the binary holds the
  system clock, and no flag shortens a wait (constitution VIII).
- **A `0s` timeout reads once.** Each pull request is observed one time, and
  the answer is the event if it is already met, otherwise `timed-out`.

**2026-10-27, increment 2: `land arm` and `land push`.** Both are bindings
(006 3.2) to `statecraft_land::arm` and `statecraft_land::push`, dispatched
after the command tree parses, so `--help` answers as for every verb.

- **Order.** Each reads the attempt binding first, present even when empty
  (3.9, R-9), then its arguments, then `landing.default_branch`, which every
  verb requires (section 5's first row), and only then GitHub.
- **When the current head was pushed (3.3.1).** The `createdAt` of the
  newest force-push event whose after-commit is the head, otherwise the
  earliest check suite GitHub created at the head. Arming strictly later is
  `already-armed`; arming at the same second, or a push time that cannot be
  read, is `armed-before-head`, so an unknown order refuses and never arms.
- **Required checks (3.3.2).** A check run or status context the head's
  rollup marks required for this pull request; a completed check run with
  any conclusion but success refuses, neutral, stale and startup failure
  among them, and a status context in failure or error.
- **Signed heads.** Read from the committed declaration's
  `governance.require_signed_commits`. `land arm` asks GitHub whether the
  head's signature is valid; `land push` asks git, since GitHub has not seen
  the head yet, and refuses a head with no signature, a bad one or one made
  by a revoked key; a signature git cannot check locally (no key, or an
  expired one) passes, and GitHub's verification decides at `land arm`.
- **Merge method (3.3.3).** None passed when the base has a merge queue (the
  queue's), otherwise the viewer's default merge method for the repository.
- **`land push` refusals (3.4.1).** `wrong-branch` also covers a pull
  request whose head is in another repository, since the push goes to
  `origin`. A conflict marker is a line opening with seven `<` or seven `>`;
  a lone `=======` is a Markdown underline. `pre_push` output goes to
  standard error, so the answer on standard output stays whole. Only git's
  `stale info` is `lease-refused`; a hook or branch-protection rejection is
  a failed push, exit 4.
- **The publishing guards.** `integration_slice` (over the command tree)
  and `negative_cases` (over the binary's verb listing) exempt exactly the
  spelling `land push` from their guards against publishing verbs, citing
  the `F-02` narrowing (D-5); every other verb keeps them.

**2026-10-28, increment 3: `land restack`.** A binding (006 3.2) to
`statecraft_land::restack`, sharing `land push`'s pre-push runner, disarm
and lease push, which moved into functions both verbs call.

- **Order and preconditions.** The attempt binding, then usage, then
  `default_branch`, `generated` and `pre_push`, each required. Before any
  write the worktree must be clean (`dirty` otherwise, exit 2; 3.5 runs in
  a clean worktree) and its `HEAD` must be the pull request's head (`stale`
  otherwise, naming both), so the commits replayed are GitHub's list and
  nothing local beside it.
- **Generated paths.** Matched with git's own glob pathspecs, never a
  matcher of this product's; a conflicted path matching an entry takes the
  base's copy, or its absence when the base deleted it.
- **Each replayed commit** is cherry-picked without committing, every
  `generated` entry's regenerator runs in order, every `pre_push` vector
  runs at that commit's tree (3.5.3: checked at its own tree), the result is
  staged, a conflict marker refuses (`markers`), and the commit is made
  with the original message and author, signed when required. `pre_push`
  is therefore not run again before the push.
- **Authored change (3.5.4).** Per path outside every `generated` pattern,
  the sorted added and removed lines of a zero-context diff, from the
  parent of the first listed commit to the original head and from the new
  base to the new head, as `scripts/resolve-shard-conflicts.sh` compares
  them.
- **Restore.** One function serves every failure during the replay: the
  branch returns to its original head with a clean worktree. After the
  replay completes the branch points at the result, so `authored-change-differs`,
  a refused lease or a failed retarget leaves it there, local and reported
  beside `originalHead`.
- **`--onto`.** The pull request's base, or `default_branch` when a merged
  pull request has that base as its head branch.
- **`--chain`** switches the clean worktree to each pull request's local
  head branch in turn, bottom first, each onto the previous one's head
  branch, stops at the first that does not exit 0, and returns to the
  starting branch when the worktree is clean. Its answer lists each
  attempted pull request in `restacks`.
- **A merge commit** among the pull request's commits is not replayed: its
  cherry-pick fails, the branch is restored, and the answer is exit 4.

**2026-10-28, increment 4a: `land cancel-superseded`.** A binding (006
3.2) to `statecraft_land::cancel`.

- **Order.** The attempt binding (3.9), then usage, then a committed
  `default_branch`; then the open pull requests and the waiting runs are
  read in full before the first write, so a failed read cancels nothing.
- **Which runs.** Only `pull_request` runs waiting on
  `statecraft-review-exception`, by their pending deployment's Environment
  name, whose head no open pull request points at; each is force-cancelled
  once, in ascending id. A run an open pull request still points at is
  counted in `kept` and left alone. `land status` reads the same runs
  through the same reader, so the two agree on which are superseded.
- **A failed cancel** does not stop the rest; each is listed in
  `notCancelled` with its classified reason, and the answer is exit 4
  (3.8), since something asked for was not done.

**2026-10-28, increment 4b: `land status`.** A binding (006 3.2) to
`statecraft_land::status`, which writes nothing: no git write, no GitHub
write, no file. It does not read the attempt binding (3.9).

- **Policy.** `default_branch`, `stack.depth`, `stack.depth_by_spec` and
  `stack.max_stacked` are required (an empty `depth_by_spec` is a stated
  value), as is a committed declaration. `generated` is read only for rule
  3.1.5: absent, that check is not made and `policy.generated` is null.
- **Reads.** The open pull requests, every page, through the reader `land
  cancel-superseded` uses; each head's required checks through `land arm`'s
  reader; `review status` (062) only where the class depends on it; the
  waiting runs through the reader `land cancel-superseded` uses; and
  `spec-spine registry plan --json`, run with the repository's
  `.bin/spec-spine` when it is executable, else the one on `PATH`. Any read
  that fails is exit 4.
- **Classes, in order.** A base that is not the default branch: `retarget`
  when no open pull request has it as its head, `hazard` when armed, else
  `stacked`. Then the merge queue (`fix` when unmergeable, else `merging`),
  a failed required check (`fix`), a waiting run at the head (`approve`),
  `draft`, a required check not completed (`running`), the review (`pending`
  is `running`; any other word but `no-findings` is `fix`, or `hazard` when
  armed), armed (`merging`), `BEHIND` (`update`), else `arm`. Within a class:
  most specs unblocked (the plan's transitive count) first, then oldest,
  then lowest number.
- **The approval reason** names a review word other than `no-findings`, a
  changed path under `scripts/statecraft/`, `.github/workflows/` or
  `.statecraft/setup/` (an authority change), and a changed `spec.md` (a
  possible ratification).
- **A pull request's spec** is the first `(` and three digits of its title,
  the `spec(063): ...` convention; none, and its cap is `stack.depth`.
- **Stacks (3.6.2).** Past its cap an entry names its stack's bottom in
  `landFirst`; more stacked pull requests than `stack.max_stacked` names
  every stacked bottom.
- **Rule 3.1.5.** A `generated` glob is excluded from review when its
  literal prefix, the text before its first `*`, `?` or `[`, starts with a
  `review.exclude` prefix; every other glob is listed in
  `generatedReviewed`, a finding.
- **Superseded runs** are listed in `supersededRuns`, never cancelled
  (section 5).
- **Exit.** 1 when any entry is `fix` or `hazard` or `generatedReviewed` is
  not empty; else 0.

**2026-10-28, increment 4c: `land status`'s measures (3.6.3).** With them
every verb of section 3 is built, and the implementation is complete.

- **Reads.** The pull requests merged in the last seven days, every page,
  with their base changes and force pushes. One with more of those than a
  page holds fails the read (exit 4), naming it, rather than being measured
  from a truncated history.
- **Measures.** The approval wait over waiting runs whose head targets the
  default branch; the oldest pull request not `merging`, `running` or
  `draft`; the count of superseded runs; and per depth over the merged pull
  requests: the depth is its number of base changes, the time to land is
  the median from its last base change to its merge, and the reviews per
  pull request are its force pushes plus one, since the review runs once
  per pushed head and this workflow amends.

**2026-10-28, an unset `review.exclude` (3.1 item 5).** The section was
silent on a project that sets no `review.exclude`. The rendered review then
excludes the profile's default, the derived directory, so `land status`
reads the effective value through the setup validator that renders the
review rather than the recorded key alone. Reading the key alone
reported `.statecraft/derived/**` as reviewed in this repository although
the review skips it. A setup block the validator refuses fails the read
(exit 4), naming the refusal.

## Verification

Each line is one command. The acceptance grew with each increment of
section 6.

```verify:cli
cargo test -p statecraft-land --locked
cargo test -p statecraft-environment --locked --lib landing
cargo test -p statecraft-home --locked --lib landing
cargo test -p statecraft-cli --locked --test land
cargo test -p statecraft-cli --locked --test integration_slice the_grown_command_tree_still_has_no_verb_that_publishes
cargo test -p statecraft-cli --locked --test negative_cases no_verb_the_binary_offers_publishes_anything
cargo test -p statecraft-cli --locked --test land restack
cargo test -p statecraft-cli --locked --test land cancel
cargo test -p statecraft-cli --locked --test land status
```
