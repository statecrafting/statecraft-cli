---
id: "009-work-run-accept-integration"
title: "The integration slice: work, run and accept bound to a process, with discovery and inspection"
status: approved
implementation: complete
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

## 5. Decisions recorded during implementation

Dated entries for choices §3 was silent on. None changes what §3 requires.

**2026-09-17: every verb takes the target path as its first argument.** §3.1
names the verbs and not their arguments, and §3.4's last row makes arguments
naming no operation a usage error. The product works in *registered* targets, so
every verb needs to know which one; taking it as an argument rather than
inferring it from the working directory keeps it visible in the invocation that
produced a run record, which is §006 3.6's rule about ambient state.

**2026-09-17: `list` and `show` are reserved after `run`.** `run <id>` and `run
list` are ambiguous, so a run id may not be spelled either word. Stated here
rather than discovered: the alternative is an id that silently becomes a
subcommand.

**2026-09-17: the run id is the spec id.** §3.1 says the operator names a unit
of work, and `003` §3.4 makes a retry an appended attempt of the same run. A
fresh id per invocation would turn every retry into a new run and defeat that.

**2026-09-17: `--help` is answered before a verb is resolved.** `work --help`
has to work, and `work` alone is a group rather than a verb, so a help request
is recognised first and its topic may be a group. Exit 0: the question was asked
and answered. Help is not in the command tree's own list, because a usage error
listing it would offer help as a thing to do.

**2026-09-17: concluding an attempt does not release the workspace.**
`003`'s `workspace::release` says "used when a run ends; never during one", and
concluding an *attempt* is not a run ending. Measured while implementing: the
release removed the worktree and left the branch it had created, so the next
attempt could not prepare, which §3.6's "no automatic retry" rule depends on
working. The workspace is retained and the outcome record says so. When a run
ends is a question §3 does not answer and this entry does not answer either.

**2026-09-17: the slice's JSON carries an owning crate's own type where that
crate already derives it.** `006` §3.4 makes this crate's JSON a contract and
`006` §5 records why the environment verbs got view types: deriving `Serialize`
onto spec 002's `Outcome` from here would have been a change to 002's territory.
That reason does not apply to `Eligibility`, `ReviewableOutcome` and
`Acceptance`: each is already serialisable in its owning crate and each is the
shape its own spec fixed, and `005` §3.9's account in particular must not have a
second shape. So only wire shapes this crate had to invent get a view type
here. The visible consequence is that those three serialise their fields in
snake case while this crate's own views use camel case, which is recorded rather
than hidden; unifying it is a change to `006` §3.4.

**2026-09-17: a preflight refusal concludes the attempt rather than leaving it
live.** `004` §3.3 refuses before any process is created, and the intent is
already durable by then (`003` §3.6). An intent with no outcome would send the
next run to reconciliation for an attempt that never started, so the refusal is
recorded as the attempt's outcome with the guard named.

**2026-09-17: the delta report is obtained in the target, not in the
workspace.** `005` §3.3.1 rule 2 requires the classifying binary to be resolved
independently of the candidate, and rule 3 keeps the invocation out of the
acceptance library. This is the caller side the decision record assigns to this
spec: the reader landed in #20 and `accept` is the verb that obtains a report.
Base and candidate are named explicitly so the report is about this change.

**2026-09-17: the policy digest is computed over the base's bytes, read with
`git show`.** `005` §3.3 requires every authority-set member to be read at the
trusted base. A base that carries no declared authority-set path at all is a
**refusal**, because a digest nobody can compute identifies no policy. Which
paths are members is `005` §3.3 case 2's declaration, by path, and the five this
repository declares are listed in the binding.

## Verification

Each line is one command. §3.7's ten rows are integration tests named after the
rows they cover, in `crates/statecraft-cli/tests/integration_slice.rs`. A
separate file from that crate's existing `negative_cases.rs`, which covers
`006`'s own rows: two specs' rows in one file would make a deleted row look like
a refactor.

**Three of the ten rows are review obligations and not tests, and saying so is
part of declaring the acceptance.** Rows 6, 9 and 10 require that an inspection
verb which repaired the record, a verb that answered a specification question
itself, and a CLI-crate answer an owning crate could have returned are each
"refused as a defect". A defect that is refused in review is refused by a
reader, and a test asserting the absence of code nobody wrote passes for the
wrong reason. `006` §3.2 already makes the structural half mechanical: the
coupling gate refuses a change to `crates/statecraft-cli/` that does not edit an
owning spec, so a rule that leaked into a command has to be written down where
it visibly does not belong. The other seven rows are tests.

The three `--help` commands are the only ones that check what this spec is
*for*. Every verb it names is implemented already, inside a territory no command
line reaches; the slice is the edge that makes them reachable. A verb that is
absent from the tree exits `3` under `006` §3.3, so these three commands fail
loudly on exactly the defect this spec exists to remove, and they do it without
running anything, spawning a provider or touching a target.

They check reachability and nothing else. `run --help` says the verb is bound;
it says nothing about whether an attempt would succeed, which is §3.4's own
warning that `run` exiting 0 carries no acceptance claim whatever.

```verify:cli
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
spec-spine index coverage --fail-on-untraced
cargo test -p statecraft-cli --test integration_slice
test -f crates/statecraft-cli/tests/integration_slice.rs
cargo run -q -p statecraft-cli -- work --help
cargo run -q -p statecraft-cli -- run --help
cargo run -q -p statecraft-cli -- accept --help
```
