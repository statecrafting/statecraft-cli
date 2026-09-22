---
id: "006-command-surface"
title: "The command surface: one binary, every verb the other specs name, and what an exit code means"
status: approved
implementation: complete
created: "2026-09-16"
summary: >
  The binary, and every verb reachable through it. Specs 002 to 005 each
  describe operator verbs and then own only a library crate, so nothing they
  specify is runnable. This spec owns crates/statecraft-cli/ and binds those
  verbs to a process: the command tree, the rule that a command is a thin
  binding and never a second implementation, the closed exit-code vocabulary
  that distinguishes a refusal from a failure from a finding, human and --json
  output as two renderings of one value, and the binary's name being a recorded
  decision rather than a Cargo default. It also carries the work, run and accept
  bindings that were specified separately as 009: discovery as a join of two
  spec-spine reports because one of them does not carry status, inspection as a
  read-only fold of the run record and nothing else, and the rule that a binding
  needing a library entry point gets it from the crate that owns the behavior.
  Section 3.11.1 adds the five verbs 002 sections 3.25 to 3.29 need: inspecting
  and explicitly upgrading the required harness identity, obtaining the
  managed-session payload, and recording startup evidence beside submitting
  qualification evidence for admission.
establishes:
  - { kind: directory, path: "crates/statecraft-cli/" }
extends:
  # Section 3.2 forbids a second implementation, so a binding that needs a
  # library entry point gets it from the crate that owns the behavior. Each edge
  # is additive, and neither changes what 003 or 005 requires. These arrived
  # with the work/run/accept bindings and survive their spec's consolidation
  # into this one: the crates they reach are still other specs' territory.
  - { spec: "003-work-and-run-semantics", unit: { kind: directory, path: "crates/statecraft-run/" }, nature: additive }
  - { spec: "005-acceptance-and-evidence", unit: { kind: directory, path: "crates/statecraft-acceptance/" }, nature: additive }
depends_on:
  - "000-bootstrap"
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
  - "003-work-and-run-semantics"
  - "004-execution-adapter"
  - "005-acceptance-and-evidence"
---

# 006: The command surface

## 1. Purpose

Spec 002 names `project register`, `env plan`, `env apply`, `env upgrade`,
`env remove` and `doctor`. Specs 003 to 005 name more. Every one of those specs
owns a library crate and stops there, which is correct for each of them and
leaves the product in a state where nothing it specifies can be run.

This spec exists so that gap is closed **once, deliberately, by a spec that owns
a binary**, rather than by whichever implementing change first finds the absence
inconvenient. A binary that appears as a side effect of a feature is a binary
nobody designed: its exit codes are whatever that feature needed, and the next
feature inherits them.

The gap was closed in two passes. This spec first bound `002`'s verbs, and the
`work`, `run` and `accept` bindings followed as a separate spec, `009`, because
the behavior behind them had to exist before a verb could reach it. Both passes
are here now. Section 3.1's admission rule is what ordered them, and it still
governs the next verb: a command is added by the change that implements the
behavior behind it, never ahead of it.

## 2. Territory

`crates/statecraft-cli/`.

Not this spec's territory: every behavior the commands expose. Those belong to
`002` to `005`, and section 3.2 is the rule that keeps them there. Where a
binding needed an entry point its owning library did not expose, the entry point
was added in that library under the `extends` edges above, never here; section
3.11 is that rule.

## 3. Behavior

### 3.1 The command tree

The binary exposes the verbs its dependency specs name, grouped as those specs
group them:

| Command | Owning spec | What it does |
|---|---|---|
| `project register <path>` | `002` | Records a target and prints its verdict with reasons. |
| `project list` | `002` | Every registered target, its verdict and whether it is armed. |
| `project arm <path>` / `project disarm <path>` | `002` | Consent to being driven, separately from registration. |
| `env plan` | `002` | Prints what `env apply` would do. Writes nothing. |
| `env apply` | `002` | Performs the plan. |
| `env upgrade` | `002` | Re-plans against a newer product or adapter version. |
| `env remove` | `002` | Removes managed paths whose digest still matches. |
| `doctor` | `002` | Diagnoses. Repairs nothing. |
| `work list` | `003` | The ready set for a registered target, each row naming the report field it came from. |
| `work show <id>` | `003` | One unit of work, and why it is or is not eligible. |
| `run <id>` | `003`, `004` | Prepares the workspace, supervises one attempt through the adapter, records intent and outcome. |
| `run list` | `003` | Every run for a registered target, with its attempts and outcomes. |
| `run show <run>` | `003`, `005` | The reviewable outcome of `005` section 3.9. |
| `accept <run>` | `005` | Judges the candidate independently and records the acceptance or its absence. |
| `harness show <path>` | `002` | The required harness identity, the resolved one, and the standing between them. Reads only. |
| `harness upgrade <path>` | `002` | Commits the shipped revision as the project's required identity, as an explicit act. |
| `session payload` | `002` | The exact managed-session settings bytes; with `--json`, also their identity and the argument that carries them. |
| `startup record <path> <session>` | `002` | Writes the startup record for one session, with the live observation absent. |
| `startup capture <path> <control> <capture-dir>` | `002` | Launches one qualification control and records the launch and everything it produced. |
| `startup qualify <path> <session> <capture-dir>` | `002` | Submits the three captured controls, which are admitted or refused. |

A verb is added by the change that implements the behavior behind it, never
ahead of it: a command that prints "not implemented" is a worse answer than a
command that does not exist, because only one of them is discoverable as absent.
A verb whose behavior lives in a crate this spec does not own joins the tree
through an `extends` edge naming that spec and unit, declared in the frontmatter
above.

There is no `work` verb that schedules, no `run` verb that retries
automatically, and no `accept` verb that publishes. Each of those is refused
somewhere in an owning spec, and a command that offered one would be this spec
adding behavior.

### 3.2 A command is a binding, never a second implementation

A command parses arguments, calls exactly one library operation, renders the
value it returns, and maps it to an exit code. It contains no rule the owning
spec did not state.

This is enforceable rather than aspirational: `crates/statecraft-cli/` is
claimed by this spec, and the coupling gate refuses a change to it that does not
edit this spec. A rule that leaked into a command would therefore have to be
written down here, where it visibly does not belong, instead of accumulating
where nobody looks for it.

### 3.3 The exit-code vocabulary

Closed, and the same for every command:

| Code | Meaning |
|---|---|
| 0 | The operation did what was asked, and found nothing wrong. |
| 1 | The operation ran and reports a **finding**: a diagnostic state, a withheld write, a verdict that is not `qualified`. Nothing failed. |
| 2 | The operation **refused**: a precondition was not met and nothing was done. |
| 3 | Usage error: the arguments do not name an operation this binary has. |
| 4 | The operation **failed**: something went wrong that neither the operator nor the target asked for. |

The distinction between 1, 2 and 4 is the whole point and is the one a caller
scripts against. A `partial` apply is 1: every withheld path was named and the
contract held. A removal with no manifest is 2: it declined. An unreadable
manifest is 4.

`doctor` exits 1 on any `drifted`, `missing`, `foreign`, `shadowed` or
`unmanaged-write`, which is `002` section 3.10's requirement expressed in this
vocabulary rather than restated.

### 3.4 Two renderings of one value

Every command supports `--json`. Human output and JSON output are two renderings
of the **same** returned value, produced from it by this crate, never two code
paths that compute their own answers.

JSON output is a contract: adding a field is compatible, removing or retyping
one is a change to this spec. Human output is not a contract and may be
reshaped freely, which is exactly why a caller is given `--json` to use instead.

### 3.5 The binary's name is recorded, not defaulted

The package is `statecraft-cli`, which is `D-02` as adopted. Cargo would then
name the executable `statecraft-cli` by default, and `D-01` recommends
`statecraft` and **is not adopted for naming**.

So the name is stated here rather than inherited: the executable is
`statecraft-cli`, matching the package, until `D-01`'s naming half is decided.
Adopting `statecraft` later is a change to this section and a rename in one
`[[bin]]` stanza. What this section refuses is the name being settled by a
build-tool default that nobody recorded agreeing to.

### 3.6 What the binary reads, and what it does not

It reads its arguments, the target repository, and the product home. It does not
read a configuration file that could change a rule an owning spec fixed: an
option that would alter behavior is an argument, visible in the invocation that
produced a run record, rather than ambient state.

It never reads `.derived/` directly and never answers a specification question
itself. `spec-spine` is asked, which is `001` section 3.2.

### 3.7 Observable negative cases

Every row is required behavior. The first eight are the environment and
usage verbs, the rest are the work, run and accept bindings.

| Case | Required behavior |
|---|---|
| An unknown verb | Exit 3, naming the verb and listing the ones that exist. Nothing else happens. |
| `env apply` against an unregistered target | Exit 2, refused, naming the path as unregistered. No write. |
| `env apply` that withholds a path | Exit 1, every withheld path named with its reason, and the paths that did apply reported as applied. |
| `env remove` with no manifest | Exit 2, with the reason from `002` section 3.6. No deletion. |
| `doctor` on a clean environment | Exit 0. |
| `doctor` on any finding | Exit 1, every state reported, nothing repaired. |
| A command given `--json` | Identical facts to the human rendering, from the same value. |
| A library operation returning an i/o error | Exit 4, naming the path, distinguishable from a refusal. |
| `registry plan --json` names a spec the lifecycle report does not | Excluded, with "status is unknown". Never assumed approved. |
| `registry plan --json` names a `draft` plus `pending` spec | Listed as excluded with the reason (`003` section 3.8). Never scheduled, and never silently run. |
| A spec-spine report lacks a field a verb needs | Refused, exit 2, naming the field and the spec-spine version. No substitute. |
| `run` invoked twice concurrently on one repository | The second is refused, exit 2, naming the live attempt. |
| `run show` on a run with an intent and no outcome | The reconciliation state is shown, `unknown` as `unknown`. No inferred outcome. |
| An inspection verb that would repair, prune or compact the record | Refused as a defect in review; inspection is read-only in the strong sense (section 3.9). |
| `accept` on an attempt that ended `refused` | `not-attempted`, reason `attempt-refused`, with the refusal count, exit 1. No receipt. |
| The agent claims success and the suite fails | Outcome `failed`, no receipt, the claim retained in a field named for a claim (`005` section 3.10). |
| A verb answering a specification question itself | Refused as a defect: `spec-spine` is asked (`006` section 3.6, `001` section 3.2). |
| A CLI-crate answer an owning crate could have returned | Refused as a defect (section 3.11). The entry point belongs in the owning crate. |

### 3.8 Discovery is a join, because one report does not carry status

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

### 3.9 Inspection is a fold, and reads nothing else

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

### 3.10 Placing the work, run and accept verbs in the vocabulary

section 3.3's five codes are closed and unchanged. This section only says
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

### 3.11 A binding needs an entry point, not an implementation

section 3.2 forbids a second implementation, and the practical form of that
rule during this slice is: when a verb needs something the owning library does
not expose, the entry point is added **in that library**, under the `extends`
edge declared here, and the binding calls it.

What this forbids concretely: computing a ready set in the CLI crate because the
run crate exposes only a scheduling call; formatting an outcome in the CLI crate
by reaching into record fields the acceptance crate does not return; and any
answer the CLI crate derives that an owning crate could have returned.

section 3.4 still holds over everything added here: human and `--json`
output are two renderings of one returned value, and the JSON shape is a
contract.

### 3.11.1 The five verbs spec 002's sections 3.25 to 3.29 need

Added on 2026-09-21, authorized by the owner and recorded here before the
bindings were written. Three things `002` requires were implemented as library
operations and reachable only through examples in that spec's crate, which made
*implemented* and *reachable by an operator* two different claims with only the
first one true. §3.1's table now carries the five verbs that close it, and each
is a binding in the sense of §3.2 and nothing more.

**Inspection activates nothing.** `harness show` reads the manifest, the home
and the installed revisions and reports the standing. It does not install, does
not write a requirement, and does not deliver. The act that changes the
requirement is `harness upgrade`, spelled separately for that reason: an
inspection that upgraded as a side effect would make reading the state
impossible without changing it.

**The payload verb takes no path.** `session payload` is about the bytes this
build delivers to a managed session, which are a property of the build and not
of any target, in the same way the `home` verbs are about the product's own
home. Requiring a project in order to print them would be requiring a target in
order to read a constant.

**The qualification verb admits, it does not assert.** `startup qualify` hands
captured evidence to `002` §3.29's admission and renders what came back. There
is no flag that marks a session qualified, no flag that lowers what the
admission requires, and a refused claim writes nothing. Under §3.3 a refused
claim is exit 2: a precondition was not met and nothing was done.

**The examples stay.** They are how the acts were reachable before these verbs
and they remain runnable, which keeps a second caller of the same library
operations honest about §3.2. They are examples of calling the boundary, not
the operator's route, and `002`'s handoff no longer names them as one.

### 3.11.2 The sixth verb, and what a qualification exit code distinguishes

Added on 2026-09-22, authorized by the owner and recorded here before the
binding was written. `002` section 3.30 rule 12 binds a qualification control's
invocation to its settings by the operation that **launches** it. A binding
cannot supply that operation from a shell: an argument list a script wrote down
beside the command it ran is exactly the after-the-fact description the rule
refuses. So the launch is a library operation in `002`'s crate, and this verb is
its binding.

**`startup capture <path> <control> <capture-dir>`** launches one control, where
`<control>` is `refusal`, `allowed-command` or `without-payload`, in `<path>`,
and writes one capture record into `<capture-dir>`. Two options:
`--program <executable>` names the provider (default `claude`), and
`--deadline <seconds>` bounds the session (default 300). A third,
`--synthetic`, is the operator stating that the executable is a local fake: it
marks the capture synthetic, which `002` section 3.30 makes permanently
non-qualifying. There is no option that names the arguments, the prompt, the
commands or the settings: the verb constructs them, so a caller cannot describe
an invocation that did not happen.

Its exit codes, in §3.3's vocabulary:

| Code | Meaning for `startup capture` |
|---|---|
| 0 | The control was launched, ended by itself inside its deadline, and its output reads as one complete session. The record is written. |
| 1 | The record is written and the launch did not complete: a timeout, a signal, a survivor, or output that is not one complete session. A finding: it is recorded, and the next control should not be launched. |
| 2 | Refused before launch: an unknown control, a capture record that already exists, a capture directory inside the project, or an executable that cannot be resolved. Nothing was launched. |
| 4 | The launch or the record failed for a reason nobody asked for. |

A 0 is a statement about the launch, never about the control's outcome: a
refusal control that executed its command still exits 0 here, and it is the
admission that refuses the claim.

**`startup qualify` now takes the capture directory** the three launches wrote
into, rather than a submission file naming captures by path. The three records
are read by their fixed names. And §3.11.1's single exit 2 is split where it
conflated two different facts:

| Code | Meaning for `startup qualify` |
|---|---|
| 0 | The observation was admitted and the record qualifies. Not reachable through this verb today, because it records no supply (`002` section 3.26), and that is stated rather than hidden. |
| 1 | The observation was admitted and recorded; the record does not qualify. It says which evidence class is missing. |
| 2 | The claim was read and **refused** by the admission, or a record for the session already exists. Nothing was written, and the session is unverified. |
| 4 | The captures could not be **read**: a record is missing, unreadable or not a capture record. No claim was judged and nothing was written. |

An unreadable capture is 4 for the reason an unreadable manifest is 4 in §3.3:
it is not a precondition the operator declined, it is evidence that is not
there. Reporting it as 2 made a missing file indistinguishable from a measured
negative, which is the substitution `002` section 3.29 exists to prevent.

### 3.12 What the command surface does not unlock

Stated because an integration slice is exactly where scope grows quietly:

- **No publication.** `005` section 3.9 is explicit that no verb in this corpus
  publishes, and adding `accept` does not add one.
- **No parallelism.** `003` section 3.7 keeps one live attempt per registered
  repository, with the workspace as the lock. `F-10` reopens it by consuming
  spec-spine's collision report, which is a separate change.
- **No automatic retry.** A retry is an operator invoking `run` again, which
  appends a new attempt (`003` section 3.4).
- **No second provider.** `008` is the first and its out-of-scope section holds.

## 4. Out of scope

Installing the binary; publishing it; a shell installer; per-target
configuration files; an interactive mode; and any read-only observation surface
(`F-04`). Distribution is deferred by `F-02`, and `D-01`'s packaging half is not
adopted.

Everything in section 3.12. The shape of a hosted or multi-machine surface. Any
scheduling policy beyond "the operator names a unit of work". Whether `work`
should ever accept a unit that is not a spec-spine-ready spec, which would be a
change to `003` section 3.1 and not to this spec.

## 5. Decisions recorded during implementation

Dated entries for choices §3 was silent on. None changes what it requires. The
entries below 2026-09-17 that concern `work`, `run` and `accept` were recorded
against `009` while that spec was separate; they are kept verbatim, because this
section is a history and a history is corrected by appending.

**2026-09-16: the JSON contract has its own view types.** §3.4 makes this
crate's JSON a contract. The library types behind the verbs are not that
contract, and deriving `Serialize` onto spec 002's `Outcome` and `Report` from
here would have been a change to 002's territory made by 006's change, which the
coupling gate refuses and should. So the wire shapes are declared here, where
the contract is, and the bindings map onto them. The two reasons point the same
way, which is usually the sign a boundary is in the right place.

**2026-09-16: the environment verbs refuse, and that is not a stub.** §3.1 says a
verb joins the tree as the behavior behind it lands, and `env plan`, `env
apply`, `env upgrade`, `env remove` and `doctor` all need a configured adapter
set. No spec ratifies a provider adapter, so there is nothing for them to plan
against. They exit 2 with the reason, which is the correct answer to a
precondition that is not met, rather than exit 0 having done nothing or a
"not implemented" message that reads like a defect. The verbs that need no
adapter, the four `project` verbs, work.

**2026-09-17: the entry above is superseded by spec 008.** It is kept rather than
rewritten, because it records what was true on its own date and this section is
a history. What has changed is its premise: spec `008` ratified Claude Code as
the first provider adapter and declares the corrective edge on this spec's
command crate. The configured adapter set is `declarations` in
`crates/statecraft-cli/src/adapters.rs`, and the environment verbs are bound to
it in `environment_verb` in `crates/statecraft-cli/src/main.rs`. So the
unconditional refusal for want of a ratified adapter no longer applies. A
prerequisite that is genuinely absent still refuses, under `002` section 3.9 and
`004` section 3.14, and that is the same correct answer to an unmet precondition
the entry above describes.

**2026-09-16: the product home is overridable by `STATECRAFT_HOME`.** §3.6 says
the binary reads its arguments, the target and the product home, and that no
configuration file may change a rule. An environment variable naming *where the
product's own state lives* changes no rule: it relocates the register. It earns
its place by making the end-to-end tests possible at all, since they must not
write into the developer's real home to check that registration writes nothing
into a target.

**2026-09-16: a relative path is resolved, not refused.** Spec 002 refuses a
relative path, and that refusal is about what the register stores. An operator
typing `project register .` has not made an error, so the binding makes the path
absolute against the working directory first. It does not canonicalize:
resolving symlinks would record a path the operator did not name.

**2026-09-16: the last gate flag joined with this change.**
`index check --fail-on-unresolved` was withheld while specs claimed crates they
had not written. `006` built the last one, so it is now enforced in `make gate`
and in CI. The cost is recorded in AGENTS.md: a new spec claiming a crate ahead
of its implementation now fails the gate, and removing the flag again would be
its own deliberate change.

**2026-09-17: every verb takes the target path as its first argument.** §3.1
names the verbs and not their arguments, and §3.10's last row makes arguments
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
attempt could not prepare, which §3.12's "no automatic retry" rule depends on
working. The workspace is retained and the outcome record says so. When a run
ends is a question §3 does not answer and this entry does not answer either.

**2026-09-17: the slice's JSON carries an owning crate's own type where that
crate already derives it.** §3.4 makes this crate's JSON a contract and
§5 records why the environment verbs got view types: deriving `Serialize`
onto spec 002's `Outcome` from here would have been a change to 002's territory.
That reason does not apply to `Eligibility`, `ReviewableOutcome` and
`Acceptance`: each is already serialisable in its owning crate and each is the
shape its own spec fixed, and `005` §3.9's account in particular must not have a
second shape. So only wire shapes this crate had to invent get a view type
here. The visible consequence is that those three serialise their fields in
snake case while this crate's own views use camel case, which is recorded rather
than hidden; unifying it is a change to §3.4.

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

**2026-09-18: `run` refuses an unarmed target, and only `run`.** `002` §3.1
already requires arming as the consent to being driven and
`Registration::eligible` already reads it; §3.10 placed `run`'s other
preconditions in §3.3's vocabulary and was silent on this one, so the
binding drove a registered target that had consented to nothing. It is a
precondition, so a refusal (**2**) naming the path and the act that would
consent, and it is evaluated where the registration is already read, which puts
it ahead of the corpus report and therefore ahead of every effect: no workspace,
no appended attempt, no spawned provider. Only `run` drives, so only `run` is
gated: `work list`, `work show`, `run list`, `run show` and `accept` read, and
a registered target staying readable while unarmed is what `002` §3.1 records a
target for. The gate is on consent alone and not on `eligible`, which also folds
in qualification: those are `002`'s two independent conditions and conflating
them here would answer a question this change did not ask. Disarming withdraws
consent for the next invocation; it does not cancel a live attempt, which §3
does not provide and this entry does not add.

**2026-09-17: the policy digest is computed over the base's bytes, read with
`git show`.** `005` §3.3 requires every authority-set member to be read at the
trusted base. A base that carries no declared authority-set path at all is a
**refusal**, because a digest nobody can compute identifies no policy. Which
paths are members is `005` §3.3 case 2's declaration, by path, and the five this
repository declares are listed in the binding.

**2026-09-21: five verbs for `002`, and the spellings were chosen to match the
tree rather than the library.** `harness` and `startup` are new groups;
`session` is a third. Each groups a noun the way `project`, `env`, `home` and
`config` already do, and each verb after it is the act. The alternatives
considered were folding all five under `env`, which would have put a read of the
harness requirement beside `env apply` and invited the reading that one implies
the other, and folding the two `startup` verbs into one with a flag, which would
have made submitting evidence look like an option on recording rather than the
separate act §3.29 requires. `session payload` takes no path for the reason §3.11.1
states; the other four take one and none of them requires it to be registered,
because reading a requirement and recording a start are both things a target
does before it is driven.

**2026-09-22, authority: §3.11.2, recorded before the binding.** The launch
verb and the split of `startup qualify`'s exit 2. The alternative considered for
the launch was leaving it in the acceptance script and having the script write
its argument list into the submission from the same variables it spawned with.
That is the design `002` section 3.30 found insufficient, because the same
variables can be written twice differently and nothing checks that they were
not. The table row for `session payload` is corrected in the same change: it
named a `--digest` option that never existed, since the identity moved to the
`--json` rendering on 2026-09-21.

## Verification

Each line is one command. §3.7's rows are integration tests that **spawn the
built binary**: an exit code is a property of a process, and a test that called
a function and inspected a returned enum would check the mapping without ever
checking that the binary uses it.

The rows are split across two test files, and the split is deliberate. The
environment and usage rows live in `crates/statecraft-cli/tests/negative_cases.rs`
and the work, run and accept rows in `tests/integration_slice.rs`, which is how
they were filed when the bindings were two specs. Merging the files would make a
deleted row look like a refactor, so they stay apart.

**Three of the work, run and accept rows are review obligations and not tests,
and saying so is part of declaring the acceptance.** The rows requiring that an
inspection verb which repaired the record, a verb that answered a specification
question itself, and a CLI-crate answer an owning crate could have returned are
each "refused as a defect". A defect that is refused in review is refused by a
reader, and a test asserting the absence of code nobody wrote passes for the
wrong reason. §3.2 already makes the structural half mechanical: the coupling
gate refuses a change to `crates/statecraft-cli/` that does not edit this spec,
so a rule that leaked into a command has to be written down here, where it
visibly does not belong.

`crates/statecraft-cli/tests/arming_consent.rs` carries the §5 entry dated
2026-09-18 instead, for the same filing reason: it is not one of §3.7's rows,
and a row's file should hold rows. It asserts the refusal on one target whose
only changing property is its consent, because a target that is unarmed and also
has nothing ready refuses either way and would prove nothing about which
precondition bit.

`crates/statecraft-cli/tests/managed_environment.rs` carries `002`'s
managed-environment rows, which reach this crate through that spec's own
bindings. `crates/statecraft-cli/tests/qualification_workflow.rs` carries
§3.11.1's five verbs the same way, and asserts three things a library test
cannot: that inspection leaves the manifest and the installed revisions
byte-identical, that a refused submission exits 2 and writes no record, and
that an unreadable submission is reported differently from a refused one. Its
captures are synthetic and the file says so at the top; **no test in this
repository spawns a provider or claims a live qualification.**

The three `--help` commands check reachability and nothing else. A verb that is
absent from the tree exits `3` under §3.3, so they fail loudly on exactly the
defect the work, run and accept bindings exist to remove, and they do it without
running anything, spawning a provider or touching a target. `run --help` says the
verb is bound; it says nothing about whether an attempt would succeed, which is
§3.10's own warning that `run` exiting 0 carries no acceptance claim whatever.

```verify:cli
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
spec-spine index coverage --fail-on-untraced
spec-spine index check --fail-on-unresolved
cargo test -p statecraft-cli --test negative_cases
cargo test -p statecraft-cli --test integration_slice
cargo test -p statecraft-cli --test arming_consent
cargo test -p statecraft-cli --test qualification_workflow
cargo run -q -p statecraft-cli -- harness --help
cargo run -q -p statecraft-cli -- startup --help
test -f crates/statecraft-cli/tests/integration_slice.rs
cargo run -q -p statecraft-cli -- work --help
cargo run -q -p statecraft-cli -- run --help
cargo run -q -p statecraft-cli -- accept --help
```
