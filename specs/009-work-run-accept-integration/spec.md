---
id: "009-work-run-accept-integration"
title: "The integration slice: work, run and accept bound to a process, with discovery and inspection"
status: draft
implementation: pending
created: "2026-09-17"
summary: >
  Specs 003, 004 and 005 each own a library crate and name operator verbs, and
  006 section 3.1 admits a verb to the command tree only through an extends edge
  from the spec owning the behavior. No such edge exists, so work, run and accept
  are specified, implemented inside their own territories, and unreachable. This
  spec is that edge. It binds the three verbs, fixes discovery as a join of two
  spec-spine reports because one of them does not carry status, fixes inspection
  as a read-only fold of the run record and nothing else, places each verb in
  006 section 3.3's closed exit-code vocabulary, and adds no behavior of its own:
  where a binding needs a library entry point that does not exist, the entry
  point is added in the crate that owns the behavior, never re-implemented here.
extends:
  # 006 section 3.1 is explicit that a verb joins the tree by an edge from the
  # spec owning the behavior, and this spec owns no behavior of its own. Every
  # verb it adds is a binding inside the crate 006 owns as one directory unit.
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
  # 006 section 3.2 forbids a second implementation, so a binding that needs a
  # library entry point gets it from the crate that owns the behavior. These two
  # edges are declared rather than discovered mid-build; each is additive, and
  # neither changes what 003 or 005 requires.
  - { spec: "003-work-and-run-semantics", unit: { kind: directory, path: "crates/statecraft-run/" }, nature: additive }
  - { spec: "005-acceptance-and-evidence", unit: { kind: directory, path: "crates/statecraft-acceptance/" }, nature: additive }
depends_on:
  - "000-bootstrap"
  - "001-boundaries-and-authority"
  - "003-work-and-run-semantics"
  - "004-execution-adapter"
  - "005-acceptance-and-evidence"
  - "006-command-surface"
  - "008-first-provider-adapter"
---

# 009: The integration slice

## 1. Purpose

Spec `006` closed the gap where nothing this product specified could be run, and
closed it for `002`'s verbs only. Specs `003`, `004` and `005` are in the same
position `002` was in before `006`: implemented and tested inside their own
territories, with no way to reach any of it from a command line. Spec `001`
section 3.3's grade table is precise about this, and calls the grade
*implemented for their own territory* and "nothing about a command line".

Spec `006` section 3.1 says how that is closed: a verb joins the tree by an
`extends` edge from the spec owning the behavior, and is added by the change that
implements the behavior behind it, never ahead of it. The behavior exists. The
edge does not. This spec is the edge.

It is deliberately one slice and not three. `run` that cannot be inspected is not
usable, and `accept` with no `work` to select from has nothing to judge, so
splitting them would produce two intermediate states in which the product is
runnable and useless.

## 2. Territory

**No new crate.** Every unit this spec touches is owned by `003`, `005` or `006`
and reached by the `extends` edges in its frontmatter. That is the whole shape of
an integration slice: if this spec needed territory of its own, it would be
implementing something, and `006` section 3.2 says a command never does that.

Not this spec's territory: what any verb means. `work` means what `003` section
3.1 says, `run` what `003` sections 3.2 to 3.6 say, `accept` what `005` says, and
where this document and one of those disagree, the owning spec is right and this
one is defective.

## 3. Behavior

### 3.1 The verbs

| Command | Owning spec | What it does |
|---|---|---|
| `work list` | `003` | The ready set for a registered target, each row naming the report field it came from. |
| `work show <id>` | `003` | One unit of work, and why it is or is not eligible. |
| `run <id>` | `003`, `004` | Prepares the workspace, supervises one attempt through the adapter, records intent and outcome. |
| `run list` | `003` | Every run for a registered target, with its attempts and outcomes. |
| `run show <run>` | `003`, `005` | The reviewable outcome of `005` section 3.9. |
| `accept <run>` | `005` | Judges the candidate independently and records the acceptance or its absence. |

There is no `work` verb that schedules, no `run` verb that retries
automatically, and no `accept` verb that publishes. Each of those is refused
somewhere in an owning spec, and a command that offered one would be this spec
adding behavior.

### 3.2 Discovery is a join, because one report does not carry status

`003` section 3.1 reads readiness from spec-spine's structured output and refuses
to reconstruct it locally. `003` section 3.1.1 turns on a spec's `status`.

Measured against spec-spine 0.20.0 on 2026-09-17, in a scratch work tree with one
spec forced to `draft` plus `pending`: `registry plan --json` names that spec
ready and carries **only** `id` and `title`. It does not carry `status`. The same
was true of 0.18.0 on 2026-09-16, so this is not a property the pin change
introduced or removed.

So discovery is the join `003` section 3.1.1 already prescribes: `registry plan
--json` for the ready set, `registry list --json` for `status` and
`implementation`, keyed by `id`. Both are spec-spine's structured output, so the
join is reading rather than deriving. A ready spec absent from the lifecycle
report is excluded with "status is unknown" and is never assumed approved.

Two rules hold this in place:

1. `work list` prints, per row, **which report each field came from**. A reader
   must never have to guess whether `ready` and `status` came from one answer.
2. If a future spec-spine release adds `status` to the plan report, dropping the
   join is a change to this section with its own measurement. It is not something
   an implementation may notice and do quietly, because the join's asymmetric
   failure (a row present in one report and absent from the other) is what makes
   the exclusion visible.

Where a needed field is in neither report, `003` section 3.8 already rules: a
refusal naming the field and the spec-spine version, never a locally derived
substitute.

### 3.3 Inspection is a fold, and reads nothing else

`run list` and `run show` fold the run record (`003` section 3.3) and read
**nothing else**. Not the filesystem, not the prepared worktree, not the
adapter's transcript, not memory carried from an earlier command in the same
process.

`005` section 3.9 already fixed what the account shows: every value names the
record it came from, what was requested beside what was applied, the claim beside
the independent result, each dimension separately, the receipt or its absence by
name, and every refusal. This spec adds only that those are two commands and that
they are read-only in the strong sense: an inspection verb that repaired, pruned
or compacted anything would make the record it reports a thing its own reader
edits.

An inspection of a run whose intent has no outcome reports the reconciliation
state of `003` section 3.6 (`confirmed`, `absent` or `unknown`) rather than an
inferred outcome. `unknown` is shown as `unknown`.

### 3.4 Placing the verbs in the exit-code vocabulary

`006` section 3.3's five codes are closed and unchanged. This section only says
which of them each verb reaches, because getting this wrong is how a caller comes
to script against the wrong distinction.

| Situation | Code | Why |
|---|---|---|
| `work list` on a target with an empty ready set | 0 | An empty answer to a question that was asked and answered. Not a finding. |
| `work list` where spec-spine's report lacks a needed field | 2 | Refused, naming the field and the version (`003` section 3.8). Nothing was done. |
| `work show` on a spec excluded by `003` section 3.1.1 | 1 | A finding: the reason is the answer, and it is not a failure. |
| `run` on a target with a live attempt | 2 | Refused, naming the live attempt (`003` section 3.7). |
| `run` where a required capability token is absent | 2 | Refused before spawn, naming the token (`004` section 3.3). No process created. |
| `run` whose attempt ends `completed` | 0 | The operation did what was asked. This says nothing about acceptance. |
| `run` whose attempt ends `failed`, `refused` or `interrupted` | 1 | A finding. The operation ran and reports an outcome that is not clean. Nothing about the product failed. |
| `run` whose supervisor could not write the record | 4 | Failed: neither the operator nor the target asked for this. |
| `accept` where the attempt outcome is not `completed` | 1 | `not-attempted` with the reason named (`005` section 3.1.1). A finding, never a silent zero. |
| `accept` where the suite fails | 1 | A finding with no receipt. |
| `accept` where the suite never ran | 1 | No acceptance recorded and the unrun checks counted. Not a pass and not a fail. |
| `accept` where the policy digest cannot be computed at the base | 2 | Refused; the reason is recorded. |
| Any of these with arguments naming no operation | 3 | Usage. |

`run` reaching 0 on a `completed` attempt is the line most likely to be misread,
so it is stated twice: **0 means the attempt reached its own end, and carries no
acceptance claim whatever.** A caller that wants an acceptance runs `accept` and
reads its code.

### 3.5 A binding needs an entry point, not an implementation

`006` section 3.2 forbids a second implementation, and the practical form of that
rule during this slice is: when a verb needs something the owning library does
not expose, the entry point is added **in that library**, under the `extends`
edge declared here, and the binding calls it.

What this forbids concretely: computing a ready set in the CLI crate because the
run crate exposes only a scheduling call; formatting an outcome in the CLI crate
by reaching into record fields the acceptance crate does not return; and any
answer the CLI crate derives that an owning crate could have returned.

`006` section 3.4 still holds over everything added here: human and `--json`
output are two renderings of one returned value, and the JSON shape is a
contract.

### 3.6 What this slice does not unlock

Stated because an integration slice is exactly where scope grows quietly:

- **No publication.** `005` section 3.9 is explicit that no verb in this corpus
  publishes, and adding `accept` does not add one.
- **No parallelism.** `003` section 3.7 keeps one live attempt per registered
  repository, with the workspace as the lock. `F-10` reopens it by consuming
  spec-spine's collision report, which is a separate change.
- **No automatic retry.** A retry is an operator invoking `run` again, which
  appends a new attempt (`003` section 3.4).
- **No second provider.** `008` is the first and its out-of-scope section holds.

### 3.7 Observable negative cases

| Case | Required behavior |
|---|---|
| `registry plan --json` names a spec the lifecycle report does not | Excluded, with "status is unknown". Never assumed approved. |
| `registry plan --json` names a `draft` plus `pending` spec | Listed as excluded with the reason (`003` section 3.8). Never scheduled, and never silently run. |
| A spec-spine report lacks a field a verb needs | Refused, exit 2, naming the field and the spec-spine version. No substitute. |
| `run` invoked twice concurrently on one repository | The second is refused, exit 2, naming the live attempt. |
| `run show` on a run with an intent and no outcome | The reconciliation state is shown, `unknown` as `unknown`. No inferred outcome. |
| An inspection verb that would repair, prune or compact the record | Refused as a defect in review; inspection is read-only in the strong sense (section 3.3). |
| `accept` on an attempt that ended `refused` | `not-attempted`, reason `attempt-refused`, with the refusal count, exit 1. No receipt. |
| The agent claims success and the suite fails | Outcome `failed`, no receipt, the claim retained in a field named for a claim (`005` section 3.10). |
| A verb answering a specification question itself | Refused as a defect: `spec-spine` is asked (`006` section 3.6, `001` section 3.2). |
| A CLI-crate answer an owning crate could have returned | Refused as a defect (section 3.5). The entry point belongs in the owning crate. |

## 4. Out of scope

Everything in section 3.6. The shape of a hosted or multi-machine surface.
Any scheduling policy beyond "the operator names a unit of work". Release and
distribution (`F-02`). Whether `work` should ever accept a unit that is not a
spec-spine-ready spec, which would be a change to `003` section 3.1 and not to
this spec.
