---
id: "004-execution-adapter"
title: "The execution adapter boundary and the first provider: one protocol, declared capabilities, a qualification suite, a constructed child environment, and what a real stream can witness"
status: approved
implementation: in-progress
created: "2026-09-16"
summary: >
  The single seam through which this product runs an agent, and the first
  provider behind it. Fixes the three-part protocol (a request the supervisor
  writes, an event stream it reads, a result it records), the closed capability
  vocabulary with required tokens refusing before any process starts and
  preferred tokens degrading visibly, the negative suite that qualifies an
  adapter binary version, and the constructed rather than filtered child
  environment whose property is a complete record and explicitly not
  containment of hostile code. Sections 3.9 to 3.16 name Claude Code as the
  first provider and fix what its stream can and cannot witness: which
  capability tokens the manifest may declare, each against a measurement rather
  than a reading of the flags; the finding that a mid-stream denial notification
  accompanies the terminal refusal record and that a denied session still
  classifies itself as a success; which of two denial mechanisms the adapter
  must use, because they have opposite evidence properties; and what a
  qualification record binds to.
establishes:
  - { kind: directory, path: "crates/statecraft-adapter/" }
  - { kind: directory, path: "crates/statecraft-adapter-claude-code/" }
extends:
  # Inspection folds the recorded posture through the reviewable account.
  - { spec: "005-acceptance-and-evidence", unit: { kind: directory, path: "crates/statecraft-acceptance/" }, nature: additive }
  # Keep the provider claim separate from the mapped termination when the
  # run supervisor records both in its existing outcome shape. Section 3.17
  # adds the planning coverage to the intent (`session::begin_with`).
  - { spec: "003-work-and-run-semantics", unit: { kind: directory, path: "crates/statecraft-run/" }, nature: additive }
  # The environment half of this adapter (002 section 3.9) registers a harness
  # adapter, its managed paths and its prerequisites, which is a declaration
  # inside the crate 002 owns as one directory unit. Nothing 002 requires
  # changes: it already specifies what an adapter declares and what it does
  # when its harness is absent. Section 3.17 carries `project.commands` in the
  # project declaration so a rewrite keeps it (002 section 3.16 names it).
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-environment/" }, nature: additive }
  # Section 3.17 removes the vacuous binding in `adapters::child_environment`
  # and binds `run` to the command coverage it compares at planning and at
  # launch, in a new `coverage` module of that crate.
  # `env plan`, `env apply`, `env upgrade`, `env remove` and `doctor` were bound
  # to a refusal whose stated reason was that no spec ratifies an adapter.
  # Naming one falsified the reason, so the binding changed in the same change,
  # inside the crate 006 owns.
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: corrective }
depends_on:
  - "000-bootstrap"
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
  - "003-work-and-run-semantics"
# Not `depends_on`: 005 and 006. Both are reached by the `extends` edges above,
# and both depend on this spec in turn: 005 rests on the seam's posture and 006
# binds the verbs behind it. While the provider half was a separate spec those
# two facts were acyclic, because the provider spec could depend on 005 and 006
# while the seam did not. Merged, declaring them here is a cycle the compiler
# refuses (V-014), and it would be a false one: what the provider half needs
# from 005 and 006 is *their territory*, which `extends` already declares and
# scopes to the units it names. See the 2026-09-21 entry in section 5.
---

# 004: The execution adapter boundary, and the first provider

## 1. Purpose

One seam, so that a second provider is a thin adapter and not a second product.
The supervisor never learns a provider's name; the adapter never learns the run's
policy. This is the boundary that makes constitution XI checkable: a capability
is something an adapter declares and a suite confirms, not something inferred
from a familiar file layout.

The archived predecessor reached this shape across its specs 043, 114, 116, 120
and 124, and the ordering was the expensive part: it packaged members before the
seam existed and had to perform the surgery afterwards. This corpus starts with
the seam.

Sections 3.9 to 3.16 then name the first provider, Claude Code. The seam was
fixed first and deliberately: a vocabulary designed against one provider is a
description of that provider. Naming one turns the capability vocabulary from a
design into a set of claims that can be checked, and the checking found that two
of them are not supportable in the obvious way. Those findings are in sections
3.10 and 3.11, and they are why this half exists.

## 2. Territory

`crates/statecraft-adapter/`, the seam, and
`crates/statecraft-adapter-claude-code/`, the first provider behind it.

**They are two compilation units, and that is the rule rather than an accident
of history.** A provider name appearing in `crates/statecraft-adapter/` is a
defect, and it is a defect a compiler can find: the seam crate does not depend
on the provider crate, names no provider in any type, string or feature, and is
built and tested without it. The rule was enforced by two specs before it was
enforced by two crates; the crates were always the mechanism, and they are
untouched by these two documents becoming one. A second provider is another
crate beside this one, claimed by its own spec, and section 3.8 already requires
that two adapters declaring the same path refuse at plan time.

Not this spec's territory: the supervisor's classification of an attempt
(`003`), and any second provider.

## 3. Behavior

### 3.1 The protocol

An adapter is a separate process the supervisor spawns. Three parts, none of
which names a provider:

- **A request** the supervisor writes: the workspace path, the base revision, the
  prompt bytes, the required and preferred capability tokens, a deadline, and an
  attempt identity. The prompt is delivered on a stream, never interpolated into
  a shell command line.
- **An event stream** the supervisor reads: a typed, ordered sequence carrying at
  minimum an init event (what the adapter actually applied), progress events,
  **refusal events**, and a terminal result event.
- **A result** the supervisor records: the termination classification, the
  capabilities applied and degraded, the cost if the provider reports one, and
  the adapter and provider versions.

The supervisor owns the event stream and derives `003` §3.5's refusal count from
it. The adapter does not report a count.

### 3.2 The capability vocabulary

Closed. Six tokens in the first slice; adding one is an amendment, not a
configuration change:

| Token | What it asserts |
|---|---|
| `tool-allowlist` | The provider can restrict which tools the session may use. |
| `turn-limit` | The provider can cap the number of turns. |
| `workspace-write` | The provider can confine writes to the prepared workspace. |
| `hook-enforcement` | The provider runs the repository's hooks and reports a blocked action as a structured event. |
| `cost-report` | The provider reports a cost for the session. |
| `structured-refusals` | A refusal is a structured event, not text the supervisor would have to parse from a transcript. |

An adapter's manifest lists the tokens it supports. A request lists what the run
**requires** and what it **prefers**.

### 3.3 Required refuses, preferred degrades

- A **required** token the manifest lacks is a **refusal before any process is
  spawned**. The manifest is read first, every time. There is no partial spawn
  and no post-hoc discovery.
- A **preferred** token the manifest lacks runs, and the degradation is recorded
  on the attempt and carried in the result. A degradation is never silent and
  never inferred from its absence.

The init event carries what the provider **actually applied** beside what was
requested, so the effective configuration is evidence rather than an assumption
about what the flags meant.

`structured-refusals` gets no special category. It is an ordinary token, and the
**run** decides whether to require it:

- Required and absent: refused before spawn, like any required token. This is
  the setting for any attempt whose outcome will be relied on, because without
  the token the supervisor cannot satisfy constitution IX.
- Preferred and absent: the attempt runs, the degradation is recorded, and the
  attempt is labelled as carrying an **unverifiable refusal account**.

There are exactly two categories, required and preferred, and this token is in
whichever one the request puts it in.

### 3.4 Qualification

An adapter **binary version** is qualified only by a recorded pass of the
negative suite in §3.5. The record names the binary version, the suite version
and the date.

An adapter with no qualification record **runs**, and is labelled `unqualified`
everywhere it appears: in the posture, in the attempt record and in the outcome.
It is not refused. Refusing an unqualified adapter would make adding a provider
impossible; hiding that it is unqualified would make constitution XI a slogan.

A qualification record for a different binary version does not transfer. Neither
does a suite pass by a sibling adapter, however similar its prompts, skills or
configuration files.

### 3.5 The negative suite

One table every adapter runs, including a fixture adapter that ships with the
suite so it can run with no real provider installed:

1. A refusal is retained beside a completed result.
2. A required capability the manifest lacks is refused **before spawn**, with no
   process created.
3. A hung child is killed at the deadline, **with its descendants**, and the
   attempt is `interrupted`.
4. A malformed event stream is reported as malformed. It is never read as
   success, and never as a clean completion with missing fields.
5. An absent cost is reported as `unknown`. It is never zero.
6. A manifest declaring a token the adapter does not honor fails qualification.
7. Records emitted through the seam are identical whichever adapter
   implementation produced them, for the same request and the same provider
   behavior.
8. **The posture declares every command the run will need.** A constructed
   environment or permission profile that omits a command the repository's check
   suite invokes is refused at plan time, naming the command, rather than
   discovered as a failure mid-attempt. An ambient allowance on the operator's
   machine must not be what makes a run work, because it is absent on the next
   machine. (spec-spine design note 06 section 3.7 records the observed
   instance: a baseline profile allowing git, the GitHub CLI, Bun and spec-spine
   but not the Rust and Make toolchain the family's gate actually needs, and
   assigns the fix to this product. Section 3.17 fixes the two inputs and what
   the comparison between them can and cannot see.)

### 3.6 The child environment is constructed, not filtered

The supervisor builds the child's environment from an allowed set rather than
removing names from its own. A deny list is a list of the paths somebody thought
of; a constructed environment is the complete set of what the child gets.

Publication credentials and approval authority are **not placed in the child**,
and every publishing effect is one the supervisor performs and records.

**What this buys, stated exactly.** The supervisor's path is the only publishing
path this product *provides*. That is a statement about what the product hands
the child, not a statement about what the child can reach: the residuals below
are the ways a capable child defeats it, and the predecessor **measured** two of
them (a keyring-authenticated `gh`, and the remote reached over SSH) before its
spec 125.

So the property is **integrity of the record against an ordinary session**, not
against one that means to get around it, and not containment of hostile code.
This spec claims no isolation. Constitution VIII requires the mechanism and the
residual to be named together, so:

- A reachable absolute path bypasses any path-based redirection.
- The home directory remains readable unless an operating-system mechanism is
  applied, which this spec does not apply. *Amended by section 3.18:* the product
  home and the launch records are now kept from the child by an
  operating-system mechanism, which section 3.18 names with what it does not
  close; the residuals below about credentials and publishing are unchanged.
- A credential held in an OS keychain answers a process that asks for it, and at
  least one supported provider authenticates from that same keychain, so denying
  it breaks the provider.

- Nothing here prevents the child from using a credential it finds by any of the
  above. A publish that goes around the supervisor leaves **no record**, which is
  the exact gap this spec narrows and does not close.

Closing any of these requires an operating-system enforcement mechanism and is
deferred to `F-09`, whose spec must name the mechanism and its own residuals
(constitution VIII). Until then, no document in this repository may say the
child *cannot* publish.

### 3.7 Every attempt reports its posture

The attempt record and the outcome both carry: the adapter name and version, its
qualification state, the capabilities requested, applied and degraded, and
whether the constructed environment was **applied**, **degraded** or **refused**,
with each residual named. An operator never has to ask what posture a run
actually had.

### 3.8 Observable negative cases

Every row is required behavior. The first set belongs to the seam and
holds for any adapter; the rest were measured against the first provider
and are named as such.

| Case | Required behavior |
|---|---|
| A required token the manifest lacks | Refused before spawn, naming the token and the adapter version. No process is created. |
| A preferred token the manifest lacks | The attempt runs; the degradation is in the record and in the result. |
| The adapter's init event reports applying less than it declared | The applied set is recorded as observed; the discrepancy is reported and the adapter fails qualification (§3.5.6). |
| An adapter binary with no qualification record | Runs, labelled `unqualified` in posture, attempt and outcome. |
| A qualification record for a different version of the same adapter | Not honored; the binary is `unqualified`. |
| A malformed event stream | Reported as malformed; the attempt is not `completed`. |
| An event stream that ends without a result event | Attempt `interrupted`, not `completed`. |
| A child that outlives its deadline | Killed with its descendants; attempt `interrupted`. |
| A child that spawns a process surviving the kill | Reported as a residual on the attempt; never reported as a clean termination. |
| An adapter reporting no cost | Cost `unknown`. Never `0`. |
| A provider name appearing in `crates/statecraft-adapter/` | A defect; the seam's purpose is that it cannot. |
| An adapter requesting a credential the constructed environment omits | It fails, visibly, and the failure is recorded. The supervisor does not widen the environment to make it succeed. |
| A posture that omits a command the check suite invokes | Refused at plan time, naming the command. Never discovered mid-attempt, and never covered by an ambient allowance. |
| A document claiming the child cannot publish | Refused by review: section 3.6's residuals contradict it, and constitution VIII forbids the unnamed claim. |
| A result with `permission_denials` non-empty and `subtype: "success"` | Attempt outcome `refused`, the completed turns retained, the denial entries recorded verbatim. Never `completed`. |
| A required capability token this manifest does not declare | Refused before any process is spawned, naming the token (section 3.3). No partial spawn. |
| A run prefers `workspace-write` | Runs, and the degradation is recorded on the attempt and carried in the result. Never silent. |
| `terminal_reason: "max_turns"` | Attempt `interrupted`, not `failed`. Acceptance `not-attempted`, reason `attempt-interrupted` (`005` section 3.1.1). |
| The provider exits 0 with a denial recorded | The exit code is not read. Section 3.11. |
| The provider exits 1 on a turn cap | The exit code is not read. Section 3.13. |
| `subtype: "success"` with `is_error: true` and no denials | Attempt `interrupted`, never `completed`. The provider's `failed` claim is retained beside it. Section 3.13. |
| The applied tool allowlist is asked for | `not-recorded`, never the requested list restated as applied. Section 3.12. |
| A tool restriction expressed as tool-set removal where a refusal record is required | Fails qualification: the manifest declared `structured-refusals` and this path produces none. Section 3.12. |
| `claude` is absent from the constructed environment | The environment adapter refuses to claim its paths and names the absent prerequisite. No files written. |
| The constructed environment carries `PATH` without `USER` | The provider cannot reach the keychain and terminates with section 3.13's `api_error` shape. The environment carries `USER` so that this does not happen. Section 3.14. |
| The provider binary version differs from the qualification record | Labelled `unqualified` in the posture, the attempt record and the outcome. It still runs. |
| A second provider adapter declaring a path this one declares | Refused at plan time, naming both adapters and the path (`002` section 3.10). |

The sections below name the first provider. Every measurement in them was
taken against **Claude Code 2.1.267 on 2026-09-17**, on `darwin`, in a scratch
git work tree, with `claude --print --output-format stream-json --verbose`. A
measurement binds to that version and transfers to no other (section 3.4).

### 3.9 What is spawned, and the three parts

The adapter spawns `claude --print --output-format stream-json --verbose`. The
prompt is delivered on a stream and never interpolated into a command line
(section 3.1).

| section 3.1 part | Provider event |
|---|---|
| The request | Process arguments and a settings document, plus the prompt on stdin. |
| The init event | `{"type":"system","subtype":"init"}`, carrying `claude_code_version`, `model`, `permissionMode`, `tools`, `mcp_servers`, `skills`, `agents`, `cwd` and `apiKeySource`. |
| Progress events | `assistant` and `user` events, and `system` events including `hook_started`, `hook_response` and `permission_denied`. |
| Refusal events | Derived from the result event's `permission_denials`, verbatim, as section 3.11 requires. The mid-stream `system/permission_denied` notification is carried as progress and is not counted as an additional refusal. |
| The result | `{"type":"result"}`, carrying `subtype`, `is_error`, `terminal_reason`, `stop_reason`, `num_turns`, `total_cost_usd`, `usage`, `modelUsage` and `permission_denials`. |

The recorded
`crates/statecraft-adapter-claude-code/testdata/stream/denied.jsonl` contains a
mid-stream `system/permission_denied` notification with `tool_name`,
`tool_use_id`, `decision_reason_type` and `message`, and its tool-use id also
appears in the terminal `permission_denials` entry. The original claim that
there were no refusal events was therefore incorrect, and this table now says
what the recording shows.

The notification is observable progress; section 3.11's terminal entries remain
the refusal record. Counting both as separate refusals would count the same
denied tool use twice in this very recording, which is why the distinction is
between the two forms and not between two refusals. It makes no claim that every
denial on every provider version has both forms, and it does not change section
3.13's treatment of a malformed or truncated stream.

### 3.10 The capability tokens, each against a measurement

Section 3.2 closed the vocabulary at six tokens. A manifest declaring a
token the adapter does not honor fails qualification (section 3.5, case 6),
so each declaration below names what was measured and what the measurement did
not establish.

| Token | Declared | Measured basis |
|---|---|---|
| `turn-limit` | **Yes** | `--max-turns 1` against a prompt needing three tool calls terminated with `subtype: "error_max_turns"`, `terminal_reason: "max_turns"`, `is_error: true`, and process exit 1. |
| `cost-report` | **Yes** | `total_cost_usd` present and non-zero on the result event (`0.044009` on a one-turn session). Absence is reported as `unknown` and never as zero (section 3.5, case 5). |
| `hook-enforcement` | **Yes** | `system` events with subtypes `hook_started` and `hook_response`, the latter carrying `hook_name`, `hook_event`, `outcome`, `exit_code`, `stdout` and `stderr`. A blocked action is therefore a structured event. |
| `structured-refusals` | **Yes, and only through one mechanism** | See section 3.11. A permission deny rule produced `permission_denials: [{"tool_name":"Bash","tool_use_id":"...","tool_input":{...}}]`. Tool-set removal produced no record at all. |
| `tool-allowlist` | **Yes, with a stated blind spot** | See section 3.12. The restriction is honored; what was applied is only partly observable. |
| `workspace-write` | **Not declared in the first slice** | Not measured. Section 3.3 makes an undeclared token a refusal when a run requires it and a recorded degradation when a run prefers it, which is the correct answer for a claim nobody has checked. Declaring it later is an amendment with its own measurement. |

### 3.11 A denied session classifies itself as a success

This is the load-bearing finding, and it is measured, not inferred.

A permission deny rule (`{"permissions":{"deny":["Bash(echo:*)"]}}`) against a
prompt that calls the denied tool produced, on one result event:

- `permission_denials` with one entry naming the tool, the tool-use id and the
  full tool input;
- `subtype: "success"`, `is_error: false`, `terminal_reason: "completed"`;
- process exit **0**.

So the provider's own terminal classification calls a refused session completed,
and the process exit code agrees with it. Spec `003` section 3.4 already ruled
the other way: a refusal fails the attempt even when the underlying command exits
zero. Spec `003` section 3.5 already assigned the count to the supervisor,
derived from the event stream independently of the adapter's classification and
of the exit code.

Three requirements follow, and none of them is new policy. They are `003`'s
existing rules, now with a measurement showing what they were protecting against:

1. The adapter reports `permission_denials` **verbatim**, as the refusal record,
   and performs no classification of its own from it.
2. The adapter's result carries the provider's terminal classification as the
   provider's claim, in a field named for a claim. It is never promoted to the
   attempt's outcome.
3. A result whose `permission_denials` is non-empty and whose `subtype` is
   `success` is a well-formed result, not a malformed stream. The supervisor
   makes the attempt `refused`.

**The exit code is unreliable in both directions.** A refused session exited 0
and an unjudged one (`max_turns`) exited 1. Neither may be read as an outcome.

### 3.12 Two denial mechanisms with opposite evidence properties

Measured on the same version, with the same prompt:

| Mechanism | Reflected in the init event | Produces a refusal record |
|---|---|---|
| `--disallowedTools Bash` | **Yes**: `tools` had 89 entries and `Bash` was absent. | **No**: `permission_denials` was empty. The session reported in prose that it had no such tool. |
| A `permissions.deny` rule | **No**: `tools` had 88 entries and `Bash` was present. | **Yes**: one structured entry with the tool, the id and the input. |
| `--allowedTools Read` | **No**: `tools` had 88 entries including `Bash` and `Edit`. | Not measured. |

Neither mechanism alone satisfies section 3.3, which requires the init
event to carry what was **actually applied** beside what was requested, and
Section 3.1, which requires a refusal to be a structured event rather than
text parsed out of a transcript. One mechanism is observable and unrecorded; the
other is recorded and unobservable.

So the adapter **must use both, for different purposes**, and this is a
requirement rather than an implementation note:

- Anything whose refusal must survive as evidence is expressed as a
  **permissions deny rule**. Tool-set removal must never be used for it, because
  a removal that the model then wants is invisible to the record and reaches the
  supervisor only as prose.
- The **applied tool set** is read from the init event's `tools`, which reflects
  removal. The applied allowlist is **not** observable, so the adapter records
  the requested allowlist as requested and the applied set as `not-recorded`
  rather than assuming the two agree. `005` section 3.8's three names for
  absence are the vocabulary; this is the `not-recorded` one.

An adapter that reported the requested allowlist as though it were the applied
one would satisfy section 3.3's letter and defeat its purpose, which is why
this is written down rather than left to whoever implements it.

### 3.13 The outcome mapping

The adapter does not classify. It reports the provider's terminal fields and the
supervisor maps them onto `003` section 3.4's closed set. The mapping is fixed
here so two adapters cannot disagree about it:

| Provider terminal state | `003` section 3.4 outcome | Why |
|---|---|---|
| `subtype: "success"`, `is_error: false`, `permission_denials` empty | `completed` | Reached its own end. Says nothing about acceptance. |
| `subtype: "success"`, `permission_denials` non-empty | `refused` | Section 3.3. The completed turns are retained beside the refusal. Read before `is_error`, because a denial entry is evidence and an error flag is a claim. |
| `terminal_reason: "max_turns"` | `interrupted` | The cap stopped the attempt before anything was judged. It is **not** `failed`: nothing about the work was found not to hold. The provider calls it an error and that reading is not adopted. |
| `subtype: "success"`, `is_error: true`, `permission_denials` empty | `interrupted` | The provider stopped on its own error before anything about the work was judged. Same reasoning as the row above, and for the same reason it is not `failed`. |
| The deadline passed and the child was killed with its descendants | `interrupted` | section 3.5, case 3. |
| A malformed or truncated stream | Reported as malformed | section 3.5, case 4. Never read as a clean completion with missing fields. |

`cancelled` is never produced by the adapter: it means an operator stopped the
attempt deliberately, which the supervisor knows and the provider does not.

**A provider error is a success subtype, and it is not a completion.** Measured
on 2026-09-17 against Claude Code 2.1.267: an attempt that could not
authenticate terminated with `subtype: "success"`, `is_error: true`,
`terminal_reason: "api_error"`, `permission_denials` empty and a `result` of
`Not logged in`. This table's first version keyed its success row on the subtype
alone, so that state mapped to `completed`, and a run in which nothing happened
was recorded as an attempt that ran to its own end. `003` section 3.4 reserves
`completed` for an attempt that did run to its own end, so the mapping and the
closed set it maps onto disagreed on a state the provider really produces.

The owner resolved it on 2026-09-17 in favour of `interrupted`, by the reasoning
this table already applies to `max_turns`: the provider stopped before anything
about the work was judged, so nothing was found not to hold, and `failed` stays
what `003` says it is, a judgement about the work. The provider's own `is_error`
reading is still carried as its claim, so the disagreement stays visible in the
record instead of being resolved silently in a match arm. This is not confined
to authentication: every API-side error this provider reports arrives in the
same shape, so before this amendment a transient one would have been recorded as
a completed attempt.

### 3.14 Credentials, and what the constructed environment cannot drop

Section 3.6 constructs the child environment rather than filtering it, and
says plainly that the property bought is a complete record and not containment of
hostile code.

Measured: `apiKeySource` read `"none"` on a working session, so this provider was
not authenticated by an environment variable. On `darwin` it resolves credentials
through the operating system keychain, which a constructed environment does not
remove and cannot record. Two consequences, both stated rather than fixed:

1. The keychain is a **prerequisite**, in the sense `002` section 3.9 gives the
   word. An adapter whose prerequisite is absent refuses to claim its paths and
   names what is absent. A constructed environment that a future change narrows
   far enough to break keychain access breaks authentication, and the failure
   will present as an unrelated provider error unless this is checked first.
2. The credential is reachable by the child and its descendants for the life of
   the attempt. This is the residual section 3.6 already declared, named
   here concretely for this provider rather than left general.

**Rule 1 fired, and `USER` is what it turns on.** A constructed environment
carrying `PATH` alone was measured on 2026-09-17 to break exactly as rule 1
anticipated: the provider terminated with the `api_error` shape section 3.13
records, whose `result` read `Not logged in`. Bisected on the same date, against
the same provider version: adding `USER` to `PATH` completes the attempt
cleanly; `HOME` and `SECURITYSESSIONID` do not, and `USER` set to an account
name that is not the operator's fails the same way as omitting it. So the
keychain lookup is keyed by the account name and `USER` is the name it reads.

The constructed environment therefore carries `USER`, with the operator's own
value, and that carriage is part of the credential path this section names. The
environment is still constructed and not filtered: the allowed set is now two
names rather than one, and everything else in the supervising process's
environment is still dropped, `HOME` among them. This widens no residual that
Section 3.6 has not already declared. Its third residual is exactly this
one, stated there as a general fact about keychains and made concrete here: the
product hands the child the name, the operating system hands it the credential,
and a publish that goes around the supervisor still leaves no record.

`USER` is not itself a credential and carries none: it is an account name the
child could read from the operating system by other means. What the product is
choosing here is to let the provider authenticate at all, and the alternative
measured on the same date is that it cannot.

### 3.15 The environment half

Per `002` section 3.9 the adapter declares the harness it targets, the exact set
of paths it would manage, the facts it cannot express in that harness, and the
prerequisites it needs present. Two adapters may not declare the same path, and
this is the first, so nothing collides yet and the check still runs.

Its prerequisites are: a `claude` executable resolvable by the constructed
environment, a version it has a qualification record for (section 3.16), and the
credential path of section 3.14, whose `USER` carriage that section fixes. Absent
any of them it **refuses to claim its paths and names which one is absent**. It
does not write files for a harness that is not there.

### 3.16 Qualification binds to a version, and to nothing else

Section 3.4: an adapter binary version is qualified only by a recorded pass
of section 3.5's negative suite, and the record names the binary version,
the suite version and the date. An adapter with no qualification record **runs**
and is labelled `unqualified` everywhere it appears.

For this adapter the record names two versions, not one: the adapter's own build
and the **provider** binary it was measured against. Every finding in sections
3.9 to 3.14 is a fact about Claude Code 2.1.267. A provider upgrade invalidates
the qualification even when the adapter is byte-identical, because what was
qualified was the pair.

### 3.17 The command allowance, the suite's programs, and the comparison between them

A narrowly scoped authority amendment, settled by the owner on 2026-09-23 and
recorded before the implementation it authorizes. It gives section 3.5 row 8
and section 3.8's matching row two independent inputs, where the product
binding had one.

**The gap, exactly.** `adapters::child_environment` supplies the adapter
manifest's `requires_commands` (`claude`, `git`) as both the posture's
declared commands and the check suite's required commands, so the two are
equal by construction and the refusal cannot fire (section 5, the 2026-09-20
qualification entry, row 8). A trial whose acceptance needed `python3` ran
with nothing refused, because the child's `PATH` is the operator's.

**Rule 1: the allowance is declared, separately from the suite.** The
commands a posture allows are the adapter's own `requires_commands` plus the
commands the repository declares in the project block of its committed
`.statecraft/environment.json` (spec `002` sections 3.12 and 3.16), under one
member:

```json
"project": { "commands": ["cargo", "make"] }
```

`commands` is a list of bare program names: non-empty, no `/`, no whitespace,
no duplicates. A malformed entry refuses the run and names it. An absent
member declares nothing, and the allowance is then the adapter's own commands
alone. The declaration is already a member of the authority set (spec `005`
section 3.3 case 3), so a candidate that widens it is an authority change. This
product never adds a program to the allowance on its own account. Only the
committed project layer supplies `commands`: no personal default, team value
or run choice of spec `002` section 3.16 may, because a machine-local allowance
is exactly what section 3.5 row 8 forbids. A shell builtin that also exists as
a program, such as `test`, is compared by name like any other: a suite that
uses it declares it.

**This changes behavior, deliberately.** A target that declares no allowance
and whose suite names a program beyond the adapter's own is refused on its next
`run`, naming each program to declare. Nothing is grandfathered: a run that
worked only because the operator's `PATH` supplied a program is the case row 8
exists to refuse.

**Rule 2: the requirement comes from the suite, independently.** The programs
a run requires are read from the check suite the attempt's spec actually
declares: `spec-spine verify <spec> --plan --json`, run with the `spec-spine`
this product invokes for work selection, whose `commands` are the spec's
`## Verification` commands, without running them. A command is **simple** when,
outside single-quoted spans (POSIX: every character between two `'` is
literal), it contains none of `|`, `&`, `;`, `<`, `>`, `(`, `)`, `$`, a
backquote, a backslash or a line break, every single and double quote is
closed, and its first word is a bare program name (letters, digits and
`_ . + -`, not a `NAME=value` assignment) that is not a **wrapper**; its
program is that first word. The wrappers are a closed list of programs that
run another program named in their arguments: `env`, `exec`, `command`,
`builtin`, `xargs`, `timeout`, `nice`, `nohup`, `time`, `sudo`, `doas`, `eval`,
`source`, `.`, `sh`, `bash`, `dash`, `zsh` and `ksh`. A simple command whose
first word contains `/` names a file rather than a program `PATH` resolves; it
is listed as `path` and requires no program directly. Any other command is not
parsed: it is listed as `unparsed`, by its text. `skipped` blocks are listed
as skipped and require nothing. A plan that cannot be read (the command fails,
exits non-zero, or answers something that does not parse as the plan) refuses
the run, naming why: nothing is assumed about a suite that could not be read,
and an empty plan is only one that says it is empty.

**Rule 3: what the comparison can and cannot claim.** A program required and
absent from the allowance is **missing**. The coverage verdict is one of:

| Verdict | When | The run |
|---|---|---|
| `refused` | any program is missing | refused, naming each missing program and the commands that name it |
| `partial` | nothing is missing, and at least one command is unparsed | proceeds; the unparsed commands are named as not checked |
| `direct` | nothing is missing, and every command was parsed | proceeds |
| `not-applicable` | the attempt has no spec, as the managed-startup trial of spec `002` section 3.33 does not | proceeds; nothing was compared |

No verdict says `complete`. `direct` means every program the suite's commands
name directly is allowed; a program one of them runs in turn (`make` running
`cargo`, `cargo` running `rustc`) is not seen by this reading, and every
rendering of the verdict says so. The allowance is compared, not enforced: the
child's `PATH` still resolves through the operator's, which section 3.6's
residuals already name, and the record says the allowance is checked and not
enforced.

**Rule 4: read at the base, planned and then confirmed.** The comparison is
made twice, and neither reading comes from a workspace a session may have
edited:

- **At planning**, before the attempt is appended, from the target's working
  tree as the operator invoked `run`. A `refused` verdict here refuses the run
  with nothing appended.
- **At launch**, after the attempt's base commit is resolved and before the
  spawn, from that commit: the declaration as `git show
  <base>:.statecraft/environment.json` returns it, and the suite planned
  against an export of the base commit's tree. The workspace is not read for
  this, because section 3.2 of spec `003` reuses it across attempts and a
  previous session may have edited it.

The launch comparison is the one recorded, and section 3.7's attempt record
carries it too: the intent records the planning verdict and the digests of the
plan and the allowance it read. The run is refused at launch if its
verdict is `refused`, or if the allowance or the attempt's spec's suite plan
differs from planning by digest, naming which. So an uncommitted change to the
allowance or to that spec's verification commands refuses the run; an
uncommitted change to anything else does not. A launch refusal comes after the
intent, so the attempt is concluded `refused` under the guard
`posture-coverage`, the way a preflight refusal after the intent is concluded.

**Rule 5: what is recorded.** The attempt's outcome detail carries, under
`posture.coverage`: the spec, the base commit, the digest of the suite's plan,
each command with its program, `path` or `unparsed`, the skipped blocks, the
allowance with each entry's source (`adapter` or `declared`, with the
declaration's digest at the base, or `absent`), the missing programs, the
verdict, and the two limits of rule 3 as words. `run
show` renders it. An attempt written before this section has no
`posture.coverage` and reads as **not checked**, never as covered.

**Rule 6: the vacuous binding is removed.** Nothing supplies the allowance as
the requirement, or the reverse.

**Acceptance.** Through the binary, against a fixture repository and a fake
provider: a spec whose verification names `cargo` with no declared allowance
is refused before any process is created, naming `cargo`; the same with
`cargo` declared runs, verdict `direct`; a command with a pipe is named
`unparsed` and the verdict is `partial`; a declared entry holding a `/` or
whitespace refuses; an allowance changed between planning and launch (declared
in the working tree, absent at the base) is refused naming the allowance; a
command led by a wrapper is `unparsed`; the trial records `not-applicable`;
and the recorded coverage is what `run show` renders. Unit: the simple-command reading
over each metacharacter and an assignment prefix, and a requirement and an
allowance built from different inputs, so the comparison is no longer
tautological.

### 3.18 The protected evidence boundary

An authority amendment, settled by the owner on 2026-09-23 and recorded before
the implementation it authorizes. Constitution IX, frozen by spec `000`,
requires that refusals, interruptions and results are recorded by the
supervisor "in a place the supervised process cannot reach". Section 3.6 named
the product home as reachable, because the child runs as the same
operating-system user, and spec `003` section 3.5.1 recorded the conflict. The
owner's decision is that the principle is not amended, waived or reinterpreted:
same-user reachability is an implementation gap against it, and this section is
the boundary that closes the direct route and names, as an open gap, the route
it does not close (rule 12). It changes nothing section 3.6 says about
credentials or publishing, whose deferral to `F-09` stands; `F-09` does not
defer this section (spec `002` section 3.36 rule 2a).

**Rule 1: the supervised process.** Every process this product creates to run
a provider, for `run`, `startup trial` and `startup capture`, and every process
descended from one, including one that leaves the process group or the session.
Every execution this product itself makes of content a confined child could
have written (a verification suite, a check or a build run in or over an
attempt's workspace, by `run`, `accept` or any other verb) is confined in the
same way and is a supervised process too.

**Rule 2: the protected set.** Nothing is readable or writable by the
supervised process in:

- the product home, except the harness store (readable and executable) and the
  attempt's exchange directory (rule 5);
- every attempt's launch records (spec `002` section 3.37);
- the target's working tree (the operator's checkout) and every other attempt's
  workspace, which the child may read and not write;
- the target's Git directory, except the writable roots of rule 3, which the
  child may read and not write.

Neither access is available by any spelling of a path: a relative path, a
symbolic link, a hard link, a different case on a case-insensitive volume, a
firmlink or alternate mount of the same volume, or a rename of an ancestor.
"Read and not write" is stated where reading is needed; everywhere else in the
list, neither.

**Rule 3: the writable roots, and nothing else.** The supervised process may
write only:

| Root | Why |
|---|---|
| the attempt's workspace | the work |
| the attempt's own object directory, in the attempt's exchange area of the product home, created empty by the supervisor before launch and named to the child's Git as its object directory, with the target's shared object store as a read-only alternate; the run's own reference directory, `refs/heads/statecraft/<run>/`, holding the attempt's branch `work`, and its reflog directory under `logs/`, both created before launch; the attempt's worktree administrative directory | its own commits, without a grant on the shared object store or on any directory another run's branch lives in |
| the gate log in the exchange directory, opened for writing only (rule 5) | the gate's trace |
| one temporary directory per attempt, created by the supervisor and given to the child as its temporary directory | scratch |
| the provider's configuration directory (for Claude Code, `~/.claude/`), and its configuration file beside it: on macOS the literal names `~/.claude.json` and its temporary siblings; on Linux that one existing file for writing in place only, because granting creation or removal in the home directory would grant it over every file there | the provider cannot run without it; rule 12 names what this leaves open |
| the devices a process needs (`/dev/null`, `/dev/tty` and the like) | ordinary I/O |

Everything else the child may read, except the protected set, and may not
write. For a verification suite, the writable roots are those of spec `005`
section 3.19 rule 1. A workspace created before this section, whose branch
lives in the shared `refs/heads/statecraft/run/` directory, has its branch
renamed into its own run directory by the supervisor, under the repository
lock, before launch, and the rename is recorded (spec `003` section 3.2); a
rename that fails refuses the launch, and the grant is never widened to the
shared directory. A root that cannot be expressed exactly on a platform is not widened to
fit: the launch refuses on that platform (rule 9).

**Rule 4: what the unconfined product reads and runs.** This product, outside
the confinement, invokes no program and reads no configuration from a path a
confined child could have written, except as this rule names and treats as
inert:

- It resolves programs only from absolute `PATH` entries that lie outside every
  writable root; a relative entry, or one inside a writable root, refuses the
  launch. Every program it runs, the provider included, is checked by its real
  path after resolving links: a program, or a directory on its real path, inside
  a writable root refuses the launch. That covers `/usr/bin/sandbox-exec`, any
  shell it runs, and the provider's installed version directory wherever the
  provider keeps it.
- Its own configuration and environment come from the product home and the
  operator's invocation, which the child cannot write; `spec-spine` reads the
  operator's checkout or an export of the base, which the child can read and
  not write.
- It brings an attempt's objects into the shared store only through a transfer
  that re-hashes every object and checks connectivity, as Git's receiving side
  does for a push, and reads an attempt's commits only after that import. The
  side that reads the child's object directory to produce the transfer runs
  inside the confinement and only emits a stream; the unconfined side only
  receives and verifies it, and ignores any alternates list or configuration
  found in the child's directory. At conclusion the supervisor removes any
  reference in the run's reference directory other than `work`, and records
  that it did. The attempt's object directory is kept until the import succeeds
  or the operator reconciles the attempt; until then the run's branch may name
  objects the shared store does not hold, and the operator's own Git operations
  on the target can report them missing.
- It runs `git` against the target's common Git directory named explicitly,
  with system and global configuration disabled, hooks and the file-system
  monitor switched off, and no attribute or filter driver, and reads the
  attempt's result only through the attempt's branch in that directory. It never
  follows the workspace's `.git` file or the worktree administrative directory's
  `gitdir` and `commondir`, which the child can rewrite.
- It reads an attempt's workspace, worktree administrative directory, gate log
  and temporary directory as data only: opened relative to a directory handle taken before launch, never
  following a symbolic link, refusing a device, pipe or socket, and bounded in
  size.
- It reads the provider's configuration (spec `002` section 3.24, `doctor`) as
  data and never selects what it executes or records from it.

**Rule 5: the exchange directory.** The admission gate (spec `002` section 3.32
rule 26) is executed by the child and has to learn the supervisor's decision
and leave a trace. Each attempt has one exchange directory in the product home,
apart from its launch records, holding the gate script, the settings document
the provider is given, a copy of the admission decision and a gate log the
supervisor creates empty before launch. The child may read and execute in it
and may open the gate log for writing; it may not create, rename, link or
remove any entry there, so it cannot write the decision before the supervisor
does. Neither platform prevents it from overwriting or truncating the log it
may write, which is why the log is **child-attested**: evidence of what was
written there, never of what the supervisor decided. The supervisor makes the
decision durable in the launch records first and then writes the exchange copy
by creating a new file and renaming it into place; if the copy cannot be
written, the gate withholds and the attempt is refused with the failure named.

**Rule 6: no other process acts for the child.** The supervised process may not
reach a process outside the confinement that would act on its behalf:

- signals to any process outside the confinement are refused;
- a Unix-domain socket may be connected to only where the platform's name
  resolution requires it (on macOS, `/private/var/run/mDNSResponder`); every
  other connection, stream or datagram, including a user service manager, a
  container engine, a terminal multiplexer and an agent, is refused; a pair of
  connected sockets the process creates for itself is allowed;
- a network connection may be opened only to TCP port 443, and name
  resolution. On macOS the profile also refuses loopback and every UDP
  connection, name resolution going through `mDNSResponder`. On Linux, Landlock
  filters by port and not by address, and does not mediate UDP, so the launch
  refuses while any process on the host listens on TCP port 443, or while a
  process of the same user listens on UDP; a listener started after that check
  is not seen, and rule 12 names that;
- on macOS, Apple events, LaunchServices opens and job submission to `launchd`
  are refused.

On macOS these are the routes the profile refuses, each measured; the profile
allows other Mach services by default, and rule 12 says what that leaves
open.

**Rule 7: the mechanism, per platform.** Applied by the operating system before
the provider's first instruction, inherited by every descendant, and not
removable by it. File descriptors beyond the three standard streams are closed
before the program executes, by an explicit step and not only by
close-on-exec.

| Platform | Mechanism | Required |
|---|---|---|
| macOS | a Seatbelt profile applied by `/usr/bin/sandbox-exec`, which then executes the provider with its program and arguments unchanged: reads allowed except rule 2; writes denied except rule 3; the denials of rule 6 | the program exists and the self-test of rule 8 passes |
| Linux | Landlock applied after `fork` and before `exec`, with `no_new_privs`: a ruleset handling every file-system right the kernel's ABI knows, including `TRUNCATE` and `REFER`, built from directory handles opened before `fork`; its network rule allowing TCP connect to port 443 only; its scopes refusing signals and abstract Unix sockets outside the domain; and a seccomp filter that admits `socket(2)` only for the IPv4 and IPv6 families, admits `socketpair(2)` for the Unix domain because child processes' standard streams use it, and refuses every other family, the creation of a user namespace, and `io_uring` setup, whose operations do not pass through the calls the filter sees. Opening a file by handle needs a capability the child does not hold, which the self-test confirms. The supervisor is not dumpable for as long as it holds any descriptor on the protected set. | Landlock ABI 6 or later, the seccomp filter installed, and the self-test passes |
| anything else | none | refused |

On Linux, Landlock is an allowlist: rule 2's read denials are made by granting
read to the siblings of every protected path's ancestors, and a directory
created after the ruleset is built is not readable. A path rule 3 needs whose
parent is also the parent of a protected path, so that it cannot be granted
without granting the protected path, is not expressible, and the launch refuses
(rule 3's last sentence).

*Measured on 2026-09-23.* On macOS 26.5.1, a Seatbelt-confined child was
refused a protected read by a relative path, a symbolic link, an upper-case
spelling and the `/System/Volumes/Data` firmlink; refused a hard link to and a
rename of the protected directory; refused `launchctl submit`, `launchctl
bootstrap`, the setuid `at` and `crontab`, a LaunchServices open, an Apple event
and a signal to its parent; refused a connection to the container engine's Unix
socket and to loopback TCP; and allowed name resolution through
`mDNSResponder`, HTTPS to a remote host, a `socketpair`, and an append to a
file it was granted. On the CI runner (Ubuntu 24.04, kernel 6.17, Landlock ABI
7, Yama 1, unprivileged user namespaces refused), a Landlock-confined child was
refused a protected read and a signal to its parent, and **was not refused a
connection to the session bus, through which `systemd-run --user` ran an
unconfined process that read the protected file**. That measurement is why rule
6 refuses Unix-domain stream sockets on Linux by seccomp, and why Landlock alone
does not meet this section.

**Rule 8: the self-test.** Before each launch, a fixed probe program runs under
the exact profile or ruleset of that launch and must be refused reading and
writing a sentinel in the product home, in the launch records and in the
target's working tree, refused a Unix-domain connection and a loopback
connection, refused a socket of another family and `io_uring` setup (Linux),
and allowed a write in the workspace and to the gate log. On Linux it
must also be refused opening, through `/proc`, a descriptor the supervisor holds
on the protected set. Any other result refuses the launch.

**Rule 9: refusal.** `run`, `startup trial`, `startup capture`, and every verb
that would execute workspace content under rule 1, refuse with exit code 2,
nothing launched and nothing appended as an attempt, and a reason naming the
platform and the step that failed, when the boundary cannot be established.
The refusal is a preflight, like the other preflights of these verbs. There is
no option, environment variable or configuration that launches without it.
Nothing launched before this section is re-judged.

**Rule 10: what binds an invocation.** The confinement is recorded apart from
the invocation: spec `002` section 3.29 rule 4 and section 3.30 rule 12 bind the
provider's program and arguments as handed to the confinement, and the
confinement is bound by its mechanism and its profile or ruleset digest. The
wrapper's own arguments are not part of the invocation. An observation made
without confinement and one made with it are different invocations.

**Rule 11: the record says so.** The attempt's posture (section 3.7) records
the platform, the mechanism, the profile or ruleset digest, the protected set
and the writable roots as resolved, the self-test's result, and each open item
of rule 12 as open, so that no attempt's record reads as meeting IX while rule
12 leaves a route open. An attempt without that record was not confined.

**Rule 12: what this does not close (constitution VIII).**

- **The route through a later unconfined process is open, and IX is not met on
  it.** The child can write files that a same-user process outside the
  confinement later loads: the provider's configuration (rule 3), which the
  operator's own later provider sessions load and act on, so an instruction, a
  setting or a hook placed there is executed by a process that can reach the
  protected set. This is not a scope
  exclusion; it is a gap against a frozen principle, it is reported wherever the
  boundary is described, and it keeps every spec that accounts for IX from
  `complete` (spec `002` section 3.36 rule 2a). Closing it needs an act this
  section does not authorize: a separate operating-system principal for the
  child, or a per-run provider configuration directory whose effect on the
  provider's authentication has been measured live. Both are the owner's.
- Content the operator deliberately executes outside the confinement (the
  attempt's work, reviewed and run by the operator) is the operator's act, not
  the supervised process's, and this section does not claim to protect the
  record from it. Content this product itself executes is confined (rule 1), so
  the product never runs it outside the confinement.
- **On macOS, Mach services other than those rule 6 names are not measured.**
  Whether one of them would start or instruct a process outside the
  confinement is unknown, so IX's direct route on macOS is claimed only for the
  routes measured. Closing that is a Mach-service allowlist measured against the
  provider, which needs a confined provider session (the activation below). It
  is an open item against IX in the same way as the first.
- The host administrator and the operator are outside the supervised process
  and can edit the home. Spec `003` section 3.1.5 says what the product detects
  of that and what it does not.
- The gate log is child-attested (rule 5).
- Whether the provider runs correctly inside the confinement has not been
  measured on either platform: every measurement above used a fixture. The
  first confined provider session is an activation the owner authorizes
  separately (on Linux, for instance, the socket allowlist also refuses the
  netlink family, which some runtimes use to list network interfaces); until then a confined run's failure is reported as such and never
  as a pass.
- On Linux, a same-user UDP listener or TCP port 443 listener started after
  the launch's check is not refused by the mechanism; the check narrows that
  to what starts during the attempt.
- It is not containment of hostile code in general and claims nothing about
  credentials or publishing (section 3.6).

**Acceptance.** Hostile, deterministic child fixtures run through the product's
real launch path in an isolated home and target, each asserting both the
refusal and that the protected bytes are unchanged:

- read, write, truncate, rename and delete the run chain, the override journal
  and its state authority, the launch records, the operator's checkout and
  another attempt's workspace, by absolute and relative path, through a
  symbolic link, a hard link and a case variant, and (macOS) the firmlink;
- create the admission decision before the supervisor;
- write the target's Git configuration and hooks, `objects/info/alternates`,
  another branch, the workspace's `.git` file and the administrative
  `commondir`, then show that the supervisor's next Git operation ignores all
  of them;
- plant a program on a `PATH` entry, and a relative `PATH` entry;
- signal the supervisor; connect to a Unix-domain socket, to loopback, and
  (Linux, where a user service manager runs) `systemd-run --user`;
- start a descendant in a new session and repeat the reads from it;
- hold no descriptor beyond the three standard streams;
- replace a workspace path the supervisor reads at conclusion with a symbolic
  link into the protected set.

Positive: the same fixtures' workspace writes, commits and gate-log write
succeed, and a governed run through the fake provider concludes as before.
Negative platform cases: an unavailable mechanism, a Landlock ABI below 6, a
missing seccomp filter and a failed self-test each refuse with nothing
launched. The Linux fixtures run in CI. The macOS fixtures run in the local
acceptance on macOS, whose result is kept with the other acceptance evidence
outside the repository, and are named as not run in CI; where the suite itself
runs inside another sandbox that prevents nesting, they are named as not run,
never as failed and never as passed.

## 4. Out of scope

Any provider's adapter other than the Claude Code adapter this spec absorbed from `008` (sections 3.9 to 3.16); breadth across many providers; operating-system
sandbox profiles; adaptive autonomy or trust scoring; cost ceilings and quota
parking; and model selection policy. Each is deferred by name in the decision
record.

A second provider, and any shared abstraction extracted from having two. The
interactive mode of this provider. Authentication setup, credential rotation and
anything that would write a credential. Publication and distribution (`F-02`).
Whether the supervisor should ever require `workspace-write`, which needs a
measurement this spec did not take.

## 5. Resolved decisions

Legacy dated citations to this section refer to the preserved historical
journal at `docs/decisions/archive/004-execution-adapter-implementation-journal.md`.
They are provenance, not current requirements.

**2026-09-16: capabilities are typed and measured.** Capability names are a
closed vocabulary, and qualification is based on observed behavior rather than
provider branding.

**2026-09-16: supervision owns the deadline.** A child and its descendants are
bounded by the supervisor, malformed or missing terminal output is reported,
and no provider process may outlive its attempt.

**2026-09-17: the child environment is constructed.** Required inputs are
allowlisted into a new environment; credentials are withheld or passed only by
the declared mechanism.

**2026-09-23: launch evidence is returned to the caller.** Hook responses,
deadline state, and process completion remain separate observations so a later
admission cannot infer one from another.

## Verification

Each line is one command. §3.5's suite is eight tests named `suite_1` to
`suite_8`, and §3.8's remaining rows are tests beside them. They run against the
fixture adapter, so the table runs with no real provider installed, which is
what §3.5 requires of it.

The rows §3.8 absorbed from `008` are integration tests named after the rows
they cover. Most live in the Claude Code adapter's crate, in
`tests/negative_cases.rs`. Two are the environment half (§3.15): the absent
prerequisite and the colliding declared path are behaviors of the environment
adapter, so they live in the crate `002` owns, under the `extends` edge this
spec's frontmatter declares, and the `statecraft-environment` line runs them.
(This paragraph once counted eleven rows; the table has grown since, and a
restated count is wrong after the next row lands.)

**The provider is not spawned by the acceptance, and that is a decision rather
than a shortfall.** Every finding in §3.9 to §3.14 was measured against a live
Claude Code 2.1.267, and §3.16 binds the qualification to that pair. A command
here that spawned the provider would need a credential, and §3.14 measured that
this provider resolves credentials through the operating system keychain on
`darwin`, which no check runner has. Such a command would not run, and `005`
§3.2 rule 3 says a check that did not run is `unknown` and never a pass. An
acceptance whose central commands are structurally `unknown` is worse than one
that says plainly what it covers.

So the stream mapping is checked against **recorded** provider streams, captured
from the measured version and committed under `testdata/stream/`, the way `005`
checks its delta reader against bytes the pinned spec-spine wrote. A fixture is
evidence of what the provider emitted on a named version. It is not evidence
that the provider still emits it, and nothing here claims otherwise.

The live measurement stays where §3.16 puts it: a qualification act performed by
an operator against a named binary version and recorded, never re-derived by a
check. What the suite checks is the consequence rather than the act. An adapter
whose qualification record does not name the provider binary in front of it is
labelled `unqualified` everywhere it appears, and still runs.

The last command is §3.8's guard, run from here on purpose. This is the
first spec in the corpus permitted to name a provider, and the way that goes
wrong is not that the name appears here. It is that the name leaks back into the
seam. Naming a provider and re-checking that the seam still names none belong in
one acceptance.

```verify:cli
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
spec-spine index coverage --fail-on-untraced
cargo test -p statecraft-adapter --test no_provider_names
cargo test -p statecraft-adapter --test negative_suite
cargo test -p statecraft-adapter-claude-code --test negative_cases
cargo test -p statecraft-environment --test negative_cases
test -f crates/statecraft-adapter-claude-code/src/lib.rs
test -d crates/statecraft-adapter-claude-code/testdata/stream
```
