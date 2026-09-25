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

## 5. Decisions recorded during implementation

Dated entries for choices §3 was silent on. None changes what it requires.
The entries from 2026-09-17 onward that concern the first provider were
recorded against `008` while that spec was separate; they are kept verbatim,
because this section is a history and a history is corrected by appending.

**2026-09-19: a stdout read failure is reported as one, and is never a
completion.** The 2026-09-17 entry below separates terminal parsing from process
completion; it did not say what happens when the read itself fails. The
implementation answered by discarding the failure: the reader thread returned on
an `Err` from `lines()` and ignored the result of the trailing drain, and the
supervisor inferred the end of the stream from the channel disconnecting. That
inference is wrong in both directions. A read failure before a terminal event
was reported as `NoResult`, which claims the supervisor read the stream to its
end and found no result, an observation nobody made. A read failure while
draining after a valid terminal event was reported as nothing at all, and the
attempt was `completed`.

The reader now reports its own termination rather than letting the channel stand
for it, so a clean end of file and a failed read are distinguishable. A read
failure is a new `StreamError::ReadFailed` carrying the failure and the phase it
occurred in, and the attempt is `interrupted`, never `completed`. It is
`interrupted` rather than `failed` for the reason the 2026-09-16 entry below
gives: a stream the supervisor could not read judged nothing, so calling it
`failed` would claim an assessment of the work. The provider's terminal claim,
the trusted event prefix and the retained refusals are unchanged and stay
separate from this observation, which is what §3.1's division between the stream
and the result already requires.

Diagnostic precedence, where more than one is observed: **malformed**, then
**read failure**, then **no result**. A malformed line is a judgement about
bytes that did arrive and is the most specific. A read failure establishes that
the stream was never read to its end. Only a stream read to its end, carrying no
result, is `NoResult`.

Deadline enforcement, child-exit observation, descendant handling and the
no-blocking-join rule are untouched, and no field is added to the supervised
record. Validation is deterministic and does not reproduce an operating-system
race: a line that is not valid UTF-8 makes `lines()` fail in a real child, which
covers the failure before a terminal event through the process boundary; a
drain failure is injected through a private reader seam, since a trailing drain
copies bytes and cannot be made to fail by their content. `cargo test -p
statecraft-adapter --test deadline --locked` passes eleven tests, the ten it
carried plus the unreadable-stdout row, and the deadline, inherited-pipe,
malformed and draining rows are unchanged.

**2026-09-17: terminal parsing and process completion are separate.** The
deadline in §3.5.3 and the no-blocking-reader decision below cover the attempt
through pipe draining and child exit. A terminal event ends the trusted event
prefix, not deadline enforcement; a malformed line likewise ends parsing while
retaining the preceding events and the diagnostic. Cleanup continues under the
same deadline, and expiration interrupts even a child that already claimed
completion. Child exit is observed without blocking, and the remaining output
is drained without interpreting it. EOF alone cannot release
the deadline while the child is alive; child exit alone cannot release it while
a descendant holds the pipe. Exit status supplies no success authority. Prompt
delivery also belongs to that lifetime, so a child that does not read stdin
cannot delay the start of enforcement. These are implementation choices within
§3.1, §3.5.3 and §3.8, with no change to the ratified outcomes or qualification
contract. `cargo test -p statecraft-adapter --test deadline --locked` reproduced
five pre-repair hangs at the six-second outer limit for a one-second attempt:
terminal, malformed, inherited stdout, EOF with a live child, and unread prompt.
The repaired implementation passes that command's ten tests, including inherited
stdin, trailing-output draining, ordinary success, evidence preservation and
refusal precedence. Each fixture runs behind an independent outer timeout that
cleans up its process group and reaps the disposable supervisor process.

**2026-09-17: the fixture adapter is staged and copied into place.** Section 3.5
requires a fixture adapter that ships with the suite, and says nothing about how
it reaches the disk. Writing it directly at the path the suite is about to exec
opened a window: the suite's rows run as threads in one test binary, a sibling
thread that forks while the script is open for writing hands its child a
duplicate of that descriptor, and `execve` refuses a file any process holds open
for writing until that child reaches its own `exec`. It presented as an
intermittent Linux CI failure, `ExecutableFileBusy`, landing on a different row
each time, and it cost a re-run on several unrelated pull requests.

The helper now writes a staged file it never execs and has a **child process**
copy that file into place, so this process never opens the executed path for
writing and no fork of it can be holding a descriptor to it. Nothing about the
supervisor is involved or changed: the fixture is still a real script, still
exec'd through the same path, and every row of the suite asserts exactly what it
did. A new Linux-only reproduction, `cargo test -p statecraft-adapter --test
fixture_exec_race --locked`, forks children that outlive the fork while writer
threads write and exec their own fixtures. Against the original writer on Debian
bookworm with Rust 1.96.1 it refused 2 of 240 execs with `ExecutableFileBusy` on
each of three consecutive runs; after the repair, three consecutive runs refused
none. The same reproduction on darwin refused none either way, which is why it
is bounded to the platform where the failure was observed.

**2026-09-17: the inherited-input fixture saves stdin before backgrounding.**
Linux dash replaces an asynchronous command's stdin with `/dev/null` before
`<&0` can preserve it. The unchanged deadline suite reproduced PR #31's failure
in a disposable Debian bookworm container with Rust 1.96.0: nine tests passed,
but inherited stdin returned `completed` in approximately 11 ms. A bounded pipe
probe observed `/dev/null` at the descendant's fd 0 and a broken prompt pipe.
Saving the parent's stdin with `exec 3<&0`, then starting
`sleep 300 <&3 3<&- >/dev/null &`, retained the same pipe at the descendant's fd 0
after parent exit and kept the writer blocked. The probe confirmed blocking on
macOS sh too. With that fixture correction, the deadline command above passes
all ten tests on both platforms; inherited input additionally asserts that the
deadline elapsed and the descendant marker exists before checking cleanup.
The expected interruption, descendant cleanup and evidence assertions remain;
the supervisor implementation and ratified contract do not change.

**2026-09-16: the closed vocabulary is an enum, not a string.** §3.2 says adding
a token is an amendment and not a configuration change. A string token would let
a caller introduce a seventh; an enum means a seventh requires editing this
crate, which the coupling gate ties to editing this spec. A test asserts the set
has exactly six members.

**2026-09-16: a supervisor is never held past its own deadline, even by a
survivor.** §3.5.3 requires the kill; it does not say what happens if the kill
misses. CI answered that: a backgrounded process in the fixture survived the
group kill on Linux, still held the stdout pipe open, and the supervisor's
reader thread waited out the full 300 seconds after the deadline had correctly
fired at one. So the reader is dropped rather than joined when a deadline
fires. A supervisor that the supervised process can hold past its own deadline
is not one, and the surviving process is reported as a residual (§3.8) rather
than waited for.

**2026-09-16: descendants are killed with a process group, and `kill` is shelled
out to.** §3.5.3 requires the kill to reach descendants. The child is spawned
into its own process group and the group is signalled, which is the same
argument §3.6 makes about environments: a list of the pids somebody thought of
is not the set. Sending the signal shells out to `kill` rather than linking a
libc binding, because the dependency would be larger than the need. The form is
`kill -s KILL -- -PID`: the `--` is load-bearing, because a bare negative pid is
ambiguous with an option and the BSD and procps implementations disagree about
which it is. That disagreement is what made the failure above invisible on a
developer's machine and real on the runner. After the kill the group is probed
with signal 0, so a residual is observed rather than assumed absent. On a
non-Unix platform the descendants are not killed and the attempt says so, rather
than reporting a clean termination it did not achieve.

**2026-09-16: the fixture adapter is a real child process.** §3.5 requires a
fixture that ships with the suite. It is a `sh` script this crate writes and
spawns, not a function the suite calls: the stream, the deadline and the kill
are properties of a process, and a fixture that skipped the boundary would let
all three regress unnoticed. The fixture reads and discards the prompt from
stdin, so a regression to a command-line prompt fails a test.

**2026-09-16: a malformed stream and a missing result event are both
`interrupted`.** §3.8 gives the second explicitly and says only that the first
is "not `completed`". They land on the same outcome for the same reason: spec
003 §3.4 defines `failed` as the work having been judged and not held, and a
stream the supervisor cannot read judged nothing. Calling either `failed` would
claim an assessment nobody made.

**2026-09-16: the withheld-credential list is a refusal on construction, not a
filter.** §3.6 says publication credentials are not placed in the child. The
child inherits nothing, so the only route in is a blueprint, and that is what is
refused, with the withholding recorded as a degradation. The list names the
credential families that exist today; it is a backstop against a well-meaning
blueprint, not a containment boundary, and §3.6's residuals still stand.

**2026-09-17: the request's workspace is the child's working directory, and a
workspace that is not one is refused before spawn.** §3.1 puts the prepared
workspace in the request, and spec `003` §3.2 says the operator's checkout is
never edited and that no session runs in it. Neither says which line makes that
true of a spawned process, and nothing did: the supervisor built the child's
program, arguments, constructed environment and pipes, and never set a working
directory, so the child inherited the supervisor's own. That is the operator's
checkout whenever the command was started there, and a live provider run
measured exactly it, reporting the caller's directory in the child's init
event. The path was carried through the protocol and dropped at the boundary,
which is worse than never carrying it: every reader of the request had reason
to believe it was honored. The child is now spawned with the request's
workspace as its working directory. A workspace that does not exist, or that
exists and is not a directory, is refused before anything is spawned, naming
the path, on the error channel the supervisor already returns and in the shape
§3.3 uses for a required capability: the platform's own answer is an `ENOENT`
raised after the fork, which reads the same as an adapter binary that is not
there. Nothing §3 requires changed. This is §3.1's workspace and `003` §3.2's
isolation, enforced where a process actually acquires a directory. The suite
covers it with a fixture that reports the directory it is running in and writes
a marker there through a relative path, so a caller in one directory and a
workspace in another are separated by observation rather than by argument.

**2026-09-16: the provider-name rule is a test that greps this crate.** §3.8
makes a provider name in this territory a defect. A rule nobody can run is a
rule that decays, so `tests/no_provider_names.rs` scans the crate's own sources
for the provider names that exist today. It cannot catch a provider nobody has
heard of, and it does catch the one a future change would reach for.

**2026-09-17: the execution boundary owns the invocation's settings transport.**
Section 3.1 declares a settings document but leaves its transport unspecified.
Installed Claude Code 2.1.267's `--help` accepts `--settings <file-or-json>` as
additional settings. The adapter supplies the complete `Invocation` to its
execution boundary, including its program identity, and adds one `--settings`
argument naming an absolute, randomly named private temporary JSON file outside
the request workspace. The file is created exclusively (0600 on Unix), written
before spawn and held by the library until supervision returns, including an
interruption or spawn error. It is then removed. Temporary storage resolving
inside the workspace is refused. Forced termination of the supervisor itself
can leave the file behind; this is scoped cleanup, not hostile-child isolation.
A cleanup error after execution is retained as `settingsCleanupError` in the
execution evidence, alongside the existing terminal and refusal records. It
does not erase them or change the provider's claim or the outcome mapping.

The document is serialized as declared, never interpolated into shell text or
put in the environment. A second `--settings` in the invocation's argument
vector is rejected rather than silently selecting one document over the other.
The provider's native command-line precedence applies: managed settings remain
above this source, omitted keys retain their lower-source values, and permission
lists merge across scopes, per the provider's
[settings documentation](https://code.claude.com/docs/en/settings#settings-precedence)
read on this date. The constructed document contains only `permissions.deny`;
no hook, settings-source selector or environment entry is added. An empty deny
list follows the same transport and adds no denial policy. This discharges the
missing settings wiring recorded below, without changing qualification or the
mid-stream event wording, which was still unresolved on that date and which
Section 3.9 now settles.

Before repair, `cargo test -p statecraft-adapter-claude-code --test
settings_transport --locked` failed all three settings-reading child cases:
the real execution boundary passed no settings argument, so the fixture could
not read its declared document. After repair, the same command passes five
cases: special-character denial delivery, concurrent attempts, timeout after
a terminal denial, malformed-stream cleanup, and retention of denial evidence
when settings cleanup itself fails. These are synthetic transport checks, not
a provider qualification measurement.

**2026-09-17: run qualification is persisted posture, not target qualification.**
Sections 3.16 and 3.8, and sections 3.4 and 3.7, already require the labels.
The run binding reads the provider probe's paired qualification answer and uses
`004`'s `Posture`, including observed capabilities and process residuals. That
posture is stored in the attempt outcome's extensible detail. It is not the
environment's target verdict, and no qualification record is created by running.

The immediate outcome and `run show` expose the same recorded posture with its
source record, through an additive `posture` field. `006` section 3.4 explicitly
permits additive JSON fields; the existing seven run fields, their types, exit
codes and closed outcome words stay unchanged. The previous native-stream test's
exact field count describes that repair's scope, not a ratified prohibition on
additions. The acceptance library owns the read-only fold under the additive
edge above, reusing the seam's posture type and rendering. Historical attempts
with no posture report `not-recorded`; inspection never requalifies them from
current files. `cargo test -p statecraft-cli --test qualification --locked`
reproduced six missing-label failures before repair: all fixture runs completed,
but their JSON posture qualification was absent. The same six tests pass after
repair, exercising absent and mismatched records and matching synthetic evidence,
human and JSON output, retries and read-only inspection after removing the
provider and changing the qualification file. They confer no live qualification.

**2026-09-17: missing initialization does not erase a readable terminal denial.**
The native execution bridge's `map_stream` error path discarded the mapped
events when initialization was missing, even with a parsed terminal result
carrying denials. The result survived in detail while the supervisor counted
zero refusals. The repair shares the terminal denial-to-event mapping between
the successful and error paths. The error path retains only those independently
readable refusal events; it invents no initialization and keeps `NoInit`, an
`interrupted` execution and the provider's unchanged completed claim. The run
supervisor then counts the events and records `refused` under `003` sections
3.4 and 3.5 and this spec's section 3.11. Missing initialization without a denial
remains `interrupted`, as `004` section 5's malformed-stream decision requires.

This is implemented in `execution.rs` and `stream.rs` and tested by
`native_denied_success_without_init_keeps_refusals_and_the_stream_error` in the
provider negative suite and
`run_without_init_persists_denial_accounting_claim_and_diagnostic` in the CLI
native-stream suite. Both replay the recorded denied terminal event through a
real child with initialization removed. The CLI test reopens the persisted
accounting and outcome, checks the verbatim denial and diagnostic, and verifies
that acceptance does not run. Its undenied missing-init control stays interrupted.
No ratified requirement, ownership edge or acceptance command changes.

**Separate pre-existing finding, not repaired here:** the generic supervisor
leaves its deadline loop on a result or malformed line before calling
`child.wait()` and joining its reader. Either wait can therefore outlive the
deadline. This requires a separate supervisor repair under `004`, not an
extension of this refusal-accounting remediation.

**2026-09-17: native stream decoding belongs beside the provider mapping.**
The `run` binding sent native JSONL straight to `004`'s `event`-tagged parser,
although this provider emits `type`-tagged lines. The existing `map_stream` and
`outcome` bridge therefore never received a running child's output. The repair
uses `004`'s same process supervisor with a typed reader and terminal predicate,
then calls both existing mappings in this crate. The generic entry point still
uses its strict parser. Process creation, workspace cwd, environment, deadline
and descendant handling remain in `004`; the binding calls this provider's
entry point under the existing corrective edge on `006`.

The provider claim remains separate from refusal accounting: a denied success
is passed to `003` with its completed claim, mapped termination and refusal
events, so the supervisor counts the denials once and records `refused`.
An additive conclusion entry point separates that claim from the mapped
termination without changing the existing record fields. A turn cap supplies
`interrupted`. Unmapped terminal states and malformed or truncated streams
retain a diagnostic and cannot produce a completed outcome. Mapped events,
terminal fields and process diagnostics are retained in the outcome record's
existing extensible detail.
The closed outcomes and the command's serialized view are unchanged.

This wiring does not forward `Invocation.settings`, supply missing
`unqualified` labels, or widen the constructed environment with `USER`. Those
are separate findings and dependencies for live qualification, not claims this
repair discharges. At the time of this repair, section 3.9's mid-stream
refusal-event contradiction remained an owner amendment. Section 3.1 now
distinguishes the mid-stream notification from the terminal refusal record;
the mapper carries the former as progress and takes refusal evidence from
terminal `permission_denials` only.

**2026-09-17: the applied set reports what the invocation put into effect, minus
what the init event contradicts.** Spec §3.3 wants the init event to carry
what was *actually applied*, and §3.13.6 makes a declared-but-unapplied token a
qualification failure. §3.12 measured that this provider's init event witnesses
**one** of the five tokens directly, the tool set, and only under removal. So
neither extreme works: reporting everything granted would make §3.13.6 vacuous,
and reporting only what init proves would fail qualification on every real run.
The adapter reports what it put into effect and drops what init contradicts,
which today is exactly one thing (`hook-enforcement` before any hook event is
seen). The applied **tool set** stays `not-recorded`, which is §3.12's own answer
and is unaffected by this entry.

**2026-09-17: a terminal state §3.13's table does not list is reported, not
mapped.** The table covers `success` and `max_turns`. A subtype nobody measured
(`error_during_execution`, say) is returned as an unmapped terminal state, the
way spec §3.5 case 4 returns a malformed stream. An outcome this adapter
invented would be an outcome nobody measured.

**2026-09-17: `credential-path` is the presence of the mechanism, not of a
credential.** §3.14 measured `apiKeySource: "none"` and concluded the keychain
answers on `darwin`. Checking that a credential *works* means spending one,
which §4 puts out of scope, so the observable fact is the platform. On a
platform where §3 took no measurement the prerequisite reads **absent** rather
than assumed, because every finding in §3.9 to §3.14 is a fact about `darwin`.

**2026-09-17: `D-09`'s pointer prerequisite is not a fourth prerequisite here.**
Spec 002 §3.8 makes "this harness loads a pointer at this path" a declared
prerequisite, and §3.15 of this spec lists three that are not it. For this
harness the mechanism is the `@path` import in `CLAUDE.md`, which this provider
loads, so it is satisfied by construction and cannot be absent. The three §3.15
names are the ones that can be, and the declaration names exactly those.

**2026-09-17: the environment half's two rows reach the real declaration through
a dev-dependency.** §3.8's absent-prerequisite and colliding-path rows are
behaviors of the adapter model spec 002 owns, so the `## Verification` block runs
them in that crate's suite. Testing them against a declaration restated in the
test file would test a copy, and the copy is what drifts, so
`statecraft-environment` dev-depends on this adapter's crate. Cargo permits a
cycle through dev-dependencies; the library's own dependency graph is unchanged
and nothing in its `src/` may name the adapter.

**2026-09-17: the five verbs this spec unbinds take the exit the operation
returns.** The `extends` edge on spec 006's crate changes a refusal whose reason
this spec falsified, and 006 §3.7 fixes the exits for three of the five cases.
For the rest the exit is derived rather than chosen: `env plan` takes the exit
the apply it previews would take (a plan-level refusal is 2, a withheld path is
1, otherwise 0), because a preview whose exit disagrees with the operation it
previews is the one thing the verb exists to prevent. `env upgrade` is `env
apply` against the current declarations, which is spec 002 §3.6's own position.

**2026-09-17: a target must be registered before any environment verb runs.**
006 §3.7 requires `env apply` against an unregistered target to refuse naming the
path, and registration is the precondition all five verbs share. The binding
applies it to all five rather than to apply alone, which adds no rule: it applies
one 002 already has to the four verbs whose row 006 did not spell out.

**2026-09-19: the transport read failure is covered where this adapter maps
it.** Spec `004`'s entry of the same date adds `StreamError::ReadFailed` and
makes such a failure `interrupted`. §3.9's transport and §3.8's negative cases
are silent on which crate demonstrates that the new diagnostic survives this
adapter's own mapping, and the mapping is this spec's. The coverage is a unit
test beside the existing ones in this crate's execution module: a real child
emits a valid init event and then a line that is not valid UTF-8, and the test
asserts the observed outcome is `interrupted`, the stream error is the read
failure, the absent provider claim is reported as absent, the evidence carries
the diagnostic, and the settings file is still removed. No production behavior
in this crate changes: `stream_error` already forces `interrupted` and already
reaches the evidence as a string, and this records that both now hold for a
diagnostic that did not previously exist. No §3.8 row is added or altered.

The row names its own deadline, as the 2026-09-18 entry below requires of every
fixture here. The deadline is not what this row measures: the fixture has to
reach its unreadable line for a read failure to exist at all, and on a loaded
machine the one second the other rows share expires first, which reports the
absent init rather than the read failure. Measured: under six concurrent test
processes the shared deadline produced the absent init in 29 of 30 runs, and a
named 30-second deadline produced the read failure in 30 of 30.

**2026-09-18: the settings fixture consumes its prompt after the concurrency
barrier, and each test names its own deadline.** Both are properties of the
`## Verification` fixture, which §3 is silent on, and neither changes what
§3.9's transport or §3.8's negative cases require. The fixture's `cat` of the
prompt returns only at stdin EOF, and EOF needs every copy of the write end
closed, including one a concurrently spawned child inherited. Read before the
barrier, the two children of the concurrent test could each wait on the other:
one blocked on an EOF it could not reach, so it never signalled arrival, and the
peer spun in the barrier until both hit the deadline. The ordering requirement
removes that mutual wait. It does not remove every wait a stray descriptor can
cause, and the fixture's comment says so at the line it constrains.

The deadline becomes a per-test argument, short only where the deadline is the
subject and generous where it is incidental, matching the convention
`statecraft-adapter`'s negative suite already uses. That is mitigation, not a
fix: it widens the margin against a delayed EOF and leaves the behavior in
place. The timeout test keeps its 5 second budget and its 10 second bound, so
§3.8's bounded-cleanup row is asserted against the same numbers as before.
Nothing is disabled, retried or suppressed, and the four substantive guarantees
are unchanged. The residual is an EOF an inherited descriptor can still delay;
no finite clean run establishes that it cannot recur. Whether `supervise_stream`
gating completion on stdout EOF is itself a defect is a contract question for
spec `004` and is untouched here.

**2026-09-20: the first recorded qualification of this pair, and the live run it
admitted.** §3.16 makes a qualification an act an operator performs against a
named pair and records, and the `## Verification` note below says the act is
never re-derived by a check. No such record was found in the scope searched: this
repository's history, the default product home, and the two trial homes retained
from the 2026-09-17 measurements. Within that scope this is the first
performance of it. Broader provenance was not searched and is not claimed.
Nothing §3 requires changes; the measurements are §3.15's and §3.16's, taken
rather than described.

*What was measured, and against what.* Product source `94362ae` on `main`, a
clean tree, the binary built from it at
`sha256:726bb1208310786155abb1c3ee4b247c69bbbdcd35e7c1727071c7a8bc442578`.
Adapter `claude-code` build `0.0.0`. Provider `/Users/bart/.local/bin/claude`,
answering `2.1.267 (Claude Code)`, which is
`capabilities::MEASURED_PROVIDER_VERSION` and the version the committed streams
under `testdata/stream/` were captured from. Platform `darwin`, macOS 25.5.0.
The commands: `cargo test -p statecraft-adapter --test negative_suite --locked`,
18 passed with `suite_1` to `suite_8` all present; this spec's ten declared
commands, `PATH="$PWD/.tooling/bin:$PATH" make verify SPEC=008`, passed;
`004`'s seven, `make verify SPEC=004`, passed; and `make code` over the same
tree, 537 tests, build, clippy with warnings denied, formatting.

*Which adapter each §3.13 row was actually established against.* §3.5's
own table runs against the **fixture adapter**, which is what lets it run with
no real provider installed, and §3.4 says a suite pass by a sibling
adapter does not transfer. So `suite_1` to `suite_8` passing establishes the
seam, not this adapter. The evidence that does bear on this pair is the
provider-specific replay of the streams committed under `testdata/stream/`,
captured from 2.1.267 and driven through a real child and this crate's own
`execution::supervise`. Row by row:

| §3.5 row | Evidence bearing on **this** adapter | What the child was | Established |
|---|---|---|---|
| 1, a refusal retained beside a completed result | `negative_cases::native_denied_success_keeps_progress_claim_turns_and_exactly_one_refusal`; `a_denied_session_that_calls_itself_a_success_is_refused_and_never_completed` | recorded 2.1.267 stream replayed through a real child | yes |
| 2, a required capability the manifest lacks refused before spawn | `a_required_token_this_manifest_does_not_declare_refuses_before_any_spawn`, against this adapter's own manifest | none spawned, which is the assertion | yes |
| 3, a hung child killed at the deadline **with its descendants** | `settings_transport::timeout_after_terminal_denial_cleans_settings_and_retains_evidence` reaches the deadline and `interrupted` through this crate's execution path | a fixture shell child, not the provider | deadline and `interrupted` yes; **descendants no**: the descendant assertion exists only in `suite_3`, against the fixture adapter, and no hung real provider was ever measured |
| 4, a malformed stream reported as malformed | `a_malformed_stream_is_reported_as_malformed_and_never_a_clean_completion`; `native_malformed_and_truncated_streams_retain_progress_and_report_the_error` | recorded streams, mutated and truncated | yes |
| 5, an absent cost `unknown` and never zero | `a_reported_cost_is_carried_and_an_absent_one_is_unknown_and_never_zero` | recorded streams | yes |
| 6, a declared token the adapter does not honor fails qualification | `a_refusal_that_must_be_evidence_may_not_be_expressed_as_tool_set_removal` asserts `fails qualification` against this adapter's own denial mechanism and `tool-removed.jsonl` | recorded stream | this adapter's mechanism yes; the **runtime declared-not-applied discrepancy** path is asserted only in `suite_6`, against the fixture adapter |
| 7, records identical whichever implementation produced them | `native_recorded_success_crosses_the_process_seam` shows this adapter's records equal its own mapping and read through the generic seam | recorded stream replayed through a real child | **no**: `suite_7` compares two **fixture** implementations, and no comparison involving this adapter and a second implementation exists |
| 8, the posture declares every command the run will need | mechanism asserted in `statecraft-adapter`'s `a_posture_omitting_a_check_suite_command_is_refused_naming_the_command` | none | **no, and the product binding makes it vacuous**: `adapters::child_environment` supplies `manifest.requires_commands` as both the declared command set and the check-suite command set, so the two are equal by construction and the guard cannot fire |

Row 8's gap is not a reading of the code alone. The trial below declared
`python3 tests/check_wordcount.py` as its acceptance, `REQUIRES_COMMANDS` is
`["claude", "git"]`, and nothing refused at plan time: the environment reported
`applied` and the command resolved only because the blueprint copies the
operator's whole `PATH`. That is the case §3.13 row 8 exists to refuse, reaching
an attempt undetected.

*The record, and that it is the record and not a label.* Written to
`<product home>/qualifications.json` as `adapters.rs` reads one: adapter
`claude-code`, binary version `0.0.0`, suite version `008.3.9`, date
`2026-09-20T17:55:18Z`, provider version `2.1.267`. Its effect was measured on
both sides of writing it. Before: `env plan` reported the adapter `refused`,
`missing: ["qualification-record"]`, and planned zero writes. After: `claiming`,
and two writes, `.claude/statecraft/instructions.md` and `CLAUDE.md`. `env
apply` then wrote both and the manifest recorded both with their digests. That
is §3.15's refusal and its release, observed rather than asserted, and it is also
why an `env apply` exit code is not evidence that any path was installed.

Four facts, which this entry keeps apart because collapsing them is how a label
comes to stand for evidence it does not have. **A record was written**, naming
the pair. **The product honored it**, measured on both sides of writing it.
**The live run persisted the label**, readable from a fresh process. **Whether
the record rests on sufficient qualification evidence is not established**: the
table above leaves row 7 unestablished for this adapter, row 3's descendant
clause unestablished for this adapter, row 6 partial, and row 8 both
unestablished and vacuous in the product binding. A reader of the `qualified`
label should read it as "a record naming this pair exists and the product found
it", which is what the code computes, and not as "every §3.13 row has been
established against this adapter", which it is not.

*The live run.* A disposable corpus outside this repository, registered, armed,
and based at its own commit `30665d5`: one approved spec declaring one Python
module and a fixed acceptance suite present at that base. `statecraft-cli run
<corpus> 001-word-count-tool --json`, started `2026-09-20T17:56:15Z`, 57
seconds, exit 0. Outcome `completed`, provider claim `completed`, zero refusals,
base unmoved, capabilities `structured-refusals`, `turn-limit` and `cost-report`
all applied with none degraded, and the posture recorded `qualification:
"qualified"`. The provider implemented the module and committed it, leaving the
prepared work tree clean at candidate `52b51dd`.

*The acceptance.* `statecraft-cli accept <corpus> 001-word-count-tool --json`
answered `accepted` and minted a receipt naming base `30665d5`, candidate
`52b51dd`, one check (`spec-spine verify 001-word-count-tool --json`, exit 0),
policy digest `22b8c544...`, product `0.0.0`, spec-spine `0.20.0`, adapter
`0.0.0`, harness revision `not-recorded`, attempt `001-word-count-tool/1`, and
no authority path touched. No earlier `accepted` answer was found in the scope
searched. The historical run reported in PR 35 is not one: its trial spec
declared no suite, so `accept` correctly answered `no-acceptance`, and that
result stands as recorded.

*The receipt does not bind the suite §3.12 says it binds.* The plan declared at
the trusted base is one command, `python3 tests/check_wordcount.py`, which
`spec-spine verify 001-word-count-tool --plan` prints at `30665d5`. The receipt
carries one entry, and it is `spec-spine verify 001-word-count-tool --json` with
exit 0. §3.12 requires the ordered suite with **each command and each exit
code**; what is bound is the verifier invocation and the verifier's exit code,
so neither the declared command nor its exit code appears anywhere in the
receipt. At one declared command the difference is already total, and it grows
with the plan: an aggregate verdict cannot say which command failed, which never
ran, or in what order they went. `005` §3.4 is where that requirement lives and
this spec does not amend it; the measurement is recorded here because it is what
this trial produced.

*What the posture makes durable, and what an unmet obligation leaves undone.* A
fresh `run show` after the accept reports the posture, including
`qualification: "qualified"`, sourced to `attempt#001-word-count-tool/1`. It
reports the acceptance as `none` with reason `suite-did-not-run`, the receipt
freshness as `not-recorded`, and an authority note reading "no acceptance has
been run for this run". The run record carries `intent`, `accounting` and
`outcome` and nothing else: `accept` wrote nothing. So the run's qualification
posture is durable and the acceptance is not.

That is an **unmet obligation, not a design choice**, and this entry corrects an
earlier reading of it. `006` §3.1 gives `accept` as the verb that "judges the
candidate independently and **records the acceptance or its absence**". `005`
§3.1 requires candidate, base and policy to be "all named in the record", and
makes an unidentifiable one "**no acceptance**, recorded as such". §3.9.1
requires `not-attempted` to be "**recorded** rather than left blank", and says a
reader "must never have to infer from a missing receipt whether the suite ran
and failed, or never ran at all". The account produced here is exactly that
inference, and it resolves it wrongly: it asserts that no acceptance has been
run for a run that had just been accepted.

`005`'s 2026-09-16 entry, "nothing in this crate acts", is not an exemption from
this. It scopes to the acceptance **library**: `mint` returns a value and the
crate performs no external effect, which is how "a receipt is not a permission"
is implemented. The recording obligation falls on the verb's binding, which
`009` owns in the crate `006` owns. Repairing it belongs to `003`, `005` and
`009` together and not to this spec, which is why nothing here is amended; what
is recorded here is that the obligation is unmet and how it presents.

*Two bounded checks taken without the provider, because they bear on what a
receipt means here.* A candidate committed into the same prepared workspace that
rewrote its own `## Verification` block to a marker-writing command and appended
a line to `.claude/` was refused: `acceptance: failed`, reason
`authority-change`, members `acceptance-instructions` and `hooks`, no receipt,
exit 1. The marker file existed afterwards, so that candidate's own acceptance
instructions ran before the refusal. Two distinct things follow, and they are
not the same kind of thing. That the instructions came from the candidate is an
**unmet obligation**: `005` §3.2 already says the suite is run "from the
instructions read **at the base**", and `SpecSpineVerify::run_suite` runs
`spec-spine verify` with the prepared workspace as its working directory, whose
HEAD is the candidate. That the plan executed **before** the authority verdict
was computed is a **sequencing question §3 does not settle** either way, and
turning it into a stated rule needs its own amendment. Both live in `005` and
neither is amended here. A second candidate carrying the base's unimplemented stub under the base's
own acceptance was refused `suite-did-not-pass` with zero unrun checks and no
receipt.

*What this does not establish.* One run is not repeatability. The PR 35 run's
qualification posture is not established from retained evidence: no
`qualifications.json` exists at the default product home or in either retained
trial home from that date, and the run records retained there are `interrupted`
attempts carrying no posture field at all, so it is recorded here as unknown
rather than as unqualified, and its `no-acceptance` result is left exactly as it
was reported. Nor does this establish a conforming independent acceptance path:
trusted-base instruction loading, command-level receipt evidence and durable
recording are each unmet above, and a receipt produced while all three are
outstanding evidences that a verifier exited zero over named bytes, which is
less than §3.12 asks a receipt to bind.

*An ambiguity in the contract, named rather than resolved.* §3.4 defines
the act: "An adapter binary version is qualified only by a recorded pass of the
negative suite in §3.13", and §3.13 makes that table runnable with no real
provider installed. The `## Verification` note below says instead that "the live
measurement stays where §3.16 puts it: a qualification act performed by an
operator against a named binary version and recorded", and that "what the suite
checks is the consequence rather than the act". These do not name the same act.
Under the first, what was done today is the act, completely. Under the second,
the act is a live measurement that no spec defines: nothing states what it must
cover, what would fail it, or how its result enters the record, and
`qualification::record` takes a provider version, a suite version and a date
with no live-measurement input at all. This entry does not choose between the
two readings. It records that the first is satisfied to the extent the row table
above allows, that the second has no definition to satisfy, and that resolving
which one §3.16 means is the owner's.

*Where this evidence lives.* Under
`~/DevWork/statecraft-cli-evidence/2026-09-20-live-qualification/`, outside this
repository and outside any temporary directory: 46 files with an `INVENTORY.md`,
a `SHA256SUMS` verified after copying, the command logs, the isolated product
home's register, qualification record and run-record chain, the environment
manifest, the binary under test, and a git bundle carrying the trial corpus's
complete history including the accepted candidate `52b51dd`, the hostile
candidate `1eecdf8` and the failing-suite candidate `44451e1` under
`refs/evidence/`. The originals under `/private/tmp/statecraft-trial-2026-09-20/`
are preserved unchanged. That archive is an evidence archive and is **not**
receipt persistence: the product still records no acceptance, which is the
defect the archive documents.

*The declared acceptance did not pass on every attempt, and that is retained.*
The first `make verify SPEC=004` of the day failed at command 2, in
`settings_transport::timeout_after_terminal_denial_cleans_settings_and_retains_evidence`,
and so did the first `make verify SPEC=008` of the branch that carries this
entry. Both passed on a later attempt against the same tree, and the same test
also passed a full `make code`, ten consecutive isolated runs of the single
test, and six consecutive runs of its whole test binary. Under a temporary local
instrumentation of the fixture, not committed, every observed failure had the
same shape: the spawned child produced no event, no terminal record and none of
the files it writes as its first actions, including one written to an absolute
path outside the workspace, and the attempt ran to its 5 second deadline. The
2026-09-18 entry above records a residual for this fixture, a delayed end of
file on an inherited descriptor. A child that never wrote anything is a
different signature and is not claimed to be that residual recurring. No retry,
timeout or assertion was changed to obtain any pass recorded above. The
consequence for this spec is stated rather than softened: its declared
acceptance is not yet reliably repeatable on this machine, and the cause is an
open question in this fixture's own territory.

**2026-09-21: the suite label `008.3.9` is not renamed by this consolidation.**
Section 3.16 makes a qualification bind to a named binary version and a named
suite, and the suite this adapter was qualified against was recorded as
`008.3.9` while the provider sections were spec `008`. A record binds to the
label that was recorded. Renaming it here would leave every existing
qualification record naming a suite that no longer exists, which is the one
thing section 3.16 exists to prevent, so the label stands and the section it
was minted from is now section 3.8. A future suite revision mints a new label;
it does not retitle this one.

**2026-09-21: the two back-edges become `extends` alone, not `depends_on`.**
While the provider half was spec `008`, its frontmatter could name `005` and
`006` as dependencies: the seam did not, so the graph stayed acyclic. Merging
the two documents makes one spec that both rests on `005` and `006` and is
rested on by them, and `compile` refuses the cycle (`V-014`).

The resolution is not to weaken either claim. Both `extends` edges stay, and
they are the edges that carry the real relationship: they name the exact unit
reached and the nature of the reach, which `depends_on` never did. What is
dropped is only the coarser second statement of the same fact. The direction
that survives in `depends_on` is the one that was always true of the seam: `005`
depends on this spec, and so does `006`.

This is a cost of the consolidation and is recorded as one. A reader who wants
to know what the provider half needs from acceptance and from the command
surface reads the `extends` edges, which say more than the dropped lines did.

**2026-09-22, authority: the adapter hands its caller the hook responses, the
session id and the settings bytes it wrote.** Spec `002` section 3.31 measures
which harness revision answered a run from a `SessionStart` hook's output, and
records supply from the operation that performs it. Both readings are this
adapter's: §3.9 owns the native stream and the settings file. So the `system`
event's typing gains the three `hook_response` fields the recorded `2.1.267`
streams carry (`stdout`, `exit_code`, `outcome`), and an execution carries,
beside what it already carried, every `hook_response` in the order read with its
session id, the init event's session id, and the exact bytes written to the
settings file, read back from that file before the spawn. Additive: no mapping,
outcome, classification or existing field changes, and the generic seam gains
nothing. A field the provider does not emit stays absent, and no fixture
invents one.

**2026-09-22, authority: a run's constructed environment carries four
attempt-binding names.** Section 3.6 constructs the child environment from an
allowed set, and §3.14 names the two that set held. Spec `002` section 3.31
rule 17 adds `STATECRAFT_RUN_ID`, `STATECRAFT_ATTEMPT`,
`STATECRAFT_STARTUP_NONCE` and `STATECRAFT_HARNESS_SELECTED` for a run, so that
a hook in the session can acknowledge which attempt and which selected revision
it is running under. None is a credential, none carries one, each is set from a
value this product generated or recorded, and the environment stays
constructed: what the child receives is still the complete, recorded set.

**2026-09-22, authority: the deadline suite's contract is corrected before the
tests are.** `crates/statecraft-adapter/tests/deadline.rs` runs ten fixtures
with a one-second deadline starting at `spawn` and asserts, for every one, that
the child's init and refusal events were retained, that supervision returned
inside four seconds, and, behind that, a six-second outer limit. The first
assertion needs a precondition §3.5 case 3 does not promise, that the child is
`execve`d, runs and is read inside that second; the 2026-09-21 measurement in
spec `002` section 5 shows a loaded machine spending a whole deadline in
`execve`. The four-second bound is scheduler-sensitive in the same way: after
the deadline the supervisor spawns `kill` twice, and nothing bounds how long a
spawn takes. Neither assertion can fail only when the product is wrong.

**What the product promises about time, exactly.** The deadline runs from the
spawn. A hung attempt is never ended before it. When it fires, the supervisor
stops waiting on the child, on its pipes and on the prompt writer, keeps what
the reader had already delivered, kills the process group, probes it, and
returns without joining a reader a survivor could hold. The latency after the
deadline is that kill sequence's, and no bound on it is promised.

The corrected suite keeps every property and gives each the measurement that
can establish it:

1. **Deadline and cleanup, through a real process**, for the six fixtures that
   hang: a terminal event then a hang, a malformed line then a hang, exit with
   an inherited output pipe, end of file while the child lives, a prompt the
   child never reads, and exit with an inherited input pipe. The deadline stays
   one second from `spawn`. Unconditionally: supervision returns no earlier
   than the deadline; returns within sixty seconds, a bound chosen to separate
   the defect it exists to catch, a supervisor held by the child's 300-second
   hang, from `kill` latency, and which is therefore a statement that the
   supervisor was not held and not a measure of promptness; the outcome is
   `interrupted`; no survivor is reported; the child and any descendant it
   started are dead by process id. What was retained is checked against the
   child's own trace, written after each line it emitted: every retained event
   is, in order, one the child emitted, within the trusted prefix. Full
   retention is not asserted here, because it needs the child to have run.
2. **Event order and retention, deterministically**, in the supervisor's unit
   tests, through the private reading seam with an injected clock: the scripted
   stream is delivered, the clock is advanced past the deadline only once the
   reader has asked for bytes beyond it, and the child is a real process in its
   own group so the kill is real. That establishes, with no race, that a
   terminal event followed by a hang is retained whole and ends `interrupted`
   with the provider's claim kept, that a malformed line keeps the events
   before it and its diagnostic, that end of file with a live child and a
   blocked prompt writer both hold the supervisor until the deadline and no
   longer, and that evidence delivered before the interruption survives it.
   Production passes the real clock; nothing else about the seam changes.
3. **Completion, through a real process**, for the four fixtures that end by
   themselves: success, trailing output drained, exit with no result, and
   unreadable output. The deadline is not what they measure, so each names a
   sixty-second one as a watchdog, which the 2026-09-18 entry's convention
   already requires, and they keep their exact assertions on events, outcome
   and diagnostic. A fixture that has not started within sixty seconds reports
   a timeout, which these assertions distinguish from the property under test.

The outer limit on each disposable worker process becomes 120 seconds. It is a
watchdog that kills and reaps a worker the supervisor failed to release, it runs
its cleanup before any assertion, and it is not evidence of anything the product
promises. Nothing is retried, serialized or repeated to obtain a pass.

**2026-09-22: the deadline suite, measured before and after.** Before: with the
fixture's first line delayed two seconds, a stand-in for the `execve`
starvation measured on 2026-09-21, `terminal_then_hang` failed on
`matches!(run.events.first(), Some(Event::Init { .. }))` while the supervisor
behaved correctly, returning `interrupted` at 1.03 seconds against its
one-second deadline. That is the defect the entry above names: a test that
fails with the product right. After: the suite carries that shape as an
eleventh real-process row, `a_child_that_has_not_started_by_the_deadline`, and
passes all twelve tests, the worker included. The supervisor's eight new seam
tests pass, and two deliberate mutations, applied and reverted, show what they
catch: removing the post-deadline drain fails the terminal-then-hang test, and
letting end of file release the supervisor fails the end-of-file and
blocked-writer tests. The seam's fixtures bound their own blocking at sixty
seconds, so a supervisor that wrongly waits on them fails by outcome rather than
holding the test run.

**2026-09-22, authority: a caller may watch a supervised launch, confirm the
spawn before the prompt is delivered, and stop the process at an event.** Spec
`002` section 3.32 needs two things only this supervisor can give it. First, a
point between the spawn and the prompt: rule 22 persists a spawn confirmation
and delivers the prompt only after it, so a confirmation that cannot be
persisted leaves a process that was never given work. Second, a decision at an
event: rule 26 decides at the first non-startup event and stops the process
group on a refusal. So the supervisor gains an optional **watch** the caller
supplies. It is told the process id immediately after the spawn call returns a
process and before the prompt writer starts, and may refuse, in which case the
process group is killed, no prompt is written, and the supervision reports the
refusal and an interrupted outcome. It is shown each decoded event, in order,
on the supervising thread, and may ask for the process to be stopped, in which
case the group is killed as at the deadline, the events read so far are kept,
and the supervision reports why it was stopped. A supervision that stopped this
way is `interrupted`: the process did not end by itself. Nothing changes for a
caller that supplies no watch: its spawn, prompt timing, deadline, descendant
handling and outcome are what they were, and the deadline suite is unchanged.
The generic seam still names no provider; the claude-code adapter passes its
native events to the watch unmapped, and adds a reading of one native event as
a hook response or an init session id so the caller need not re-derive the
provider's spelling. The same rule 25 supplies a managed run's hooks per
invocation, so an invocation may carry a `hooks` value in its settings beside
the deny rules; the exact-bytes rule of the settings document is unchanged and
now covers both, and an invocation that registers no hooks writes what it
wrote before.

**2026-09-22: a test that writes a script and execs it is the `ETXTBSY` race,
wherever it is.** CI on this change's pull request failed one row of the
negative suite, `the_prompt_reaches_the_child_on_a_stream_and_no_argument_carries_it`,
with `ExecutableFileBusy`. The test wrote its own script in place and exec'd it,
so a sibling thread's fork could hold that file open for writing at the moment
of the exec: the race `fixture::write` already documents and avoids, at a site
that bypassed it. The product was not involved. The staging is now a public
helper, `fixture::install_script`, and every test in this spec's two crates that
execs a script it wrote goes through it: the failing row, four execution tests,
the settings-transport fixture, and three probe tests, the last being the
second site the 2026-09-19 repair left.

**2026-09-22: the survivor probe waits for reaping, for a bounded time.** CI on
a documentation-only pull request, with code identical to a `main` that had
passed, failed the deadline suite's `late-start` row: supervision reported
"process group still answers after SIGKILL". The fixture's shell runs `sleep`
in the foreground before it emits anything; at the deadline the group is
killed, the shell is reaped by the supervisor, and the `sleep` is reparented
and remains a zombie until its new parent collects it. Signal 0 succeeds on a
zombie, and the probe asked once, immediately, so it raced that reaping and
reported a process the kill had ended as a survivor. The probe now repeats for
up to two seconds and reports only what still answers then, naming the
possibility of an uncollected zombie. A process that genuinely survives
`SIGKILL` is still reported, two seconds later than before, and a supervision
with no survivor pays nothing. The suite's assertion that no survivor is
reported is unchanged.

**2026-09-23: the supervisor reports that its deadline fired.** `Supervised`
gains `timed_out`, set from the supervisor's own observation and carried
unchanged through the Claude Code adapter's mapping. Before this, a caller had
to infer a deadline from an interrupted outcome and the absence of a stream
error. That inference fails when the mapping adds an error of its own: a stream
stopped before `init` also carries "no init event", and spec `002` section
3.33's trial judged such a session a failed launch rather than an uncertain one.
Outcomes, stream errors and every existing field are unchanged. The seam tests
assert the flag for a deadline, a normal completion and an exit with no result.

**2026-09-23: section 3.17 implemented, and the choices it was silent on.**
Nothing §3.17 requires changed. The reading, the comparison and the verdicts
are `statecraft-adapter`'s new `coverage` module; which tree each input is read
from is `statecraft-cli`'s `coverage` module; the attempt record is written
through the edges this spec already declares on `003`'s and `006`'s crates, and
`002`'s crate carries the declared list (its own section 5 of this date).

*The two inputs, and how they stay independent.* The requirement is
`spec-spine verify <spec> --plan --json`, run with `SpecSpineCli`'s binary, the
one work selection runs, in the tree being read; each `commands` entry is read
by `coverage::classify`. The allowance is the adapter manifest's
`requires_commands` plus `project.commands` read from the declaration's bytes.
`Coverage::compare` takes the plan and the allowance as separate arguments and
nothing derives one from the other. `adapters::child_environment_with` now
gives `construct` the allowance as the posture's commands and the plan's
programs as the suite's, so `environment` reports `refused` for the same
missing program; without a suite (the environment verbs) it compares nothing,
and never passes the manifest's list as both.

*The plan's reading.* The envelope must name verb `verify`, say `ok: true` with
exit code 0, and carry a report whose `specId`, `commands` and `skipped` are
present; a missing `commands` is unreadable, not empty. `specId` is recorded
and not compared with the asked id, because spec-spine resolves a short form to
the directory name. A skipped block is `{tag, count}`, as spec-spine emits it.

*The simple-command reading.* Within a double-quoted span a `'` is literal and
opens no single-quoted span, as POSIX has it; the metacharacters are still
refused there, since the rule exempts single-quoted spans only. A line break is
`\n` or `\r`. The first word ends at the first space or tab. A first word
holding `/` is `path` only when it holds no `=` and no quote; otherwise it is
`unparsed`.

*Digests.* SHA-256, lowercase hex, over the serialization of what was read:
the parsed plan (`specId`, `commands`, `skipped`, `acceptanceFrom` when
present), the allowance (its entries with their sources, and the declaration
record), and the declared list. So whitespace or member order in spec-spine's
output does not move a digest, and a change to either input does.

*What planning refuses, and how.* A malformed declaration, an unreadable plan
and a `refused` verdict each return exit 2 with `guard: posture-coverage`,
`phase: planning`, the reason and, where one was computed, the coverage.
Nothing is appended and no process is created. The intent records the planning
reading under `postureCoverage`: `phase`, `spec`, `verdict`, `planDigest` and
`allowanceDigest`.

*What launch refuses, and how.* The base's declaration is read with `git
ls-tree` (no entry is absent) and `git show`; the base's tree is exported with
`git archive` into a fresh directory under the system temporary directory,
which is removed once the plan is read. A digest that differs from planning's is
named in the coverage's `drift` (`allowance`, `suite plan`), which refuses
whatever the verdict. The attempt concludes `refused` under `posture-coverage`
with `postureCoverageRefusal` beside the posture, which the existing mapping
reports as exit 1, a finding. A read that fails at launch refuses the same way
and records no `posture.coverage`, so it reads as not checked, with the reason
in the refusal.

*An absent coverage.* `posture.coverage` is `not-checked` on an attempt that
has none, and deserializes that way from a record written before this date.
The managed-startup trial reads the declaration at its base too, so a malformed
one refuses it as it would a run (rule 1); its verdict is `not-applicable` and
it reads no plan.

*Fixtures.* Every fake `spec-spine` in the binary's existing tests now answers
`verify <spec> --plan --json` with an empty plan, the only change to them. The
new suite, `statecraft-cli`'s `posture_coverage`, drives the binary with a fake
spec-spine that answers the plan from the directory it runs in and logs that
directory, so planning and launch are observed reading different trees.

**2026-09-23: the launch reading, corrected after review.** The entry above
is kept as written; four of its implementation choices were wrong or silent,
and nothing §3.17 requires changes.

*The base is the intent's.* The launch reading took
`session.workspace.base_commit`, which for a reused workspace is that
worktree's `HEAD`: the first attempt's base, or a commit a session made there.
Rule 4 names the attempt's base commit, and the intent records it as
`baseCommit`. `Session` now carries that value (`base_commit`, written beside
the workspace's own), and the launch reading uses it. So a second attempt after
a committed change reads the new commit, and a commit made inside the
workspace is never read as the base. The launch reading also takes the spec
from the planning coverage rather than the run id.

*The export is a temporary index, not an archive.* `git archive` honors
`export-ignore` and `export-subst`, so an attribute could hide or rewrite the
suite. The base's tree is now read into an index file of its own (`read-tree`)
and every entry checked out into a private `tempfile` directory
(`checkout-index --all`), with system and global git configuration disabled,
hooks and fsmonitor off; only the target's own repository configuration still
applies. Gitlinks are not populated, as before.

*The plan read has a deadline.* The reader runs under the supervisor's
`capture`, in its own process group, with 60 seconds to answer
(`PLAN_DEADLINE`). At the deadline the group is killed and the read is
refused as unreadable, naming the deadline.

*A launch reading that could not be made is recorded as such.*
`posture.coverage` holds `{"unread": <reason>}` for it, and `run show` renders
that reason beside the coverage line, so it no longer reads as a bare
`not checked`.

**2026-09-23: the base export's Git invocation, per the second review.** Every
inherited `GIT_*` variable is removed before system and global configuration
are switched off, so none can name another index, directory, object store or
injected configuration; the registered target is named as `safe.directory`,
because switching global configuration off also drops an operator's own
`safe.directory`. Filters the target's own `.git/config` defines still run
during `checkout-index`: that file is the operator's, not committed content,
and spec `004` section 3.18 rule 3 keeps it out of the child's writable roots.

**2026-09-25: spec `006`'s JSON naming convention, this spec's half (adopted by
the owner on 2026-09-25; spec `006` section 5 of that date).** `Negotiation`
(`missingRequired`), the adapter `Manifest` (`requiresCommands`) and the Claude
Code `Invocation` (`toolRestriction`, `settingsDocument`) now serialize in
camelCase. None of the three is written to disk, printed by a verb, or read by
another repository today; spec `006`'s source scan found them. The protocol
(`Request`, `AttemptIdentity`, `AdapterResult`), the posture recorded with every
attempt, the qualification records in `qualifications.json` and the provider
stream mirror are grandfathered: the first is a wire contract, the next two are
persisted, the last is the provider's own spelling. An adapter manifest is not
made strict: it has no schema version (its `version` is the binary's) and is
never read from a file; `qualifications.json` has no schema version either.

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
