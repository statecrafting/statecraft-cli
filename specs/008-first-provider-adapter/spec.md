---
id: "008-first-provider-adapter"
title: "The first provider adapter: naming a provider, and what its stream can and cannot witness"
status: approved
implementation: complete
created: "2026-09-17"
summary: >
  Spec 004 fixed the seam and forbade a provider name inside it, so nothing in
  this corpus names one and the environment verbs refuse for want of a ratified
  adapter set. This spec names the first: Claude Code. It fixes which of 004
  section 3.2's six capability tokens the manifest may declare, each against a
  measurement rather than a reading of the flags; it fixes the mapping from the
  provider's event stream onto 004 section 3.1's three parts, including the
  finding that there is no mid-stream refusal event and that a denied session
  still classifies itself as a success; it fixes which denial mechanism the
  adapter must use, because the two available mechanisms have opposite evidence
  properties; and it fixes what a qualification record binds to.
establishes:
  # Claimed by the change that writes it, which is this one. Section 2 says why
  # the claim could not be in the draft: the gate carries
  # `index check --fail-on-unresolved`, so a spec claiming a crate it has not
  # written yet fails.
  - { kind: directory, path: "crates/statecraft-adapter-claude-code/" }
extends:
  # Inspection folds the recorded posture through the reviewable account.
  - { spec: "005-acceptance-and-evidence", unit: { kind: directory, path: "crates/statecraft-acceptance/" }, nature: additive }
  # The native reader needs the existing process supervisor with a typed
  # decoder. The generic protocol stays strict; no provider is named in 004.
  - { spec: "004-execution-adapter", unit: { kind: directory, path: "crates/statecraft-adapter/" }, nature: additive }
  # Keep the provider claim separate from the mapped termination when the
  # run supervisor records both in its existing outcome shape.
  - { spec: "003-work-and-run-semantics", unit: { kind: directory, path: "crates/statecraft-run/" }, nature: additive }
  # The environment half of this adapter (002 section 3.9) registers a harness
  # adapter, its managed paths and its prerequisites, which is a declaration
  # inside the crate 002 owns as one directory unit. Nothing 002 requires
  # changes: it already specifies what an adapter declares and what it does
  # when its harness is absent, and this is the first thing to declare it.
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-environment/" }, nature: additive }
  # `env plan`, `env apply`, `env upgrade`, `env remove` and `doctor` are bound
  # today to a refusal whose stated reason is that no spec ratifies an adapter.
  # Ratifying one falsifies the reason, so the binding changes in the same
  # change, inside the crate 006 owns.
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: corrective }
depends_on:
  - "000-bootstrap"
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
  - "003-work-and-run-semantics"
  - "004-execution-adapter"
  - "005-acceptance-and-evidence"
  # The `extends` edge above changes a binding inside the crate 006 owns, so
  # 006 is a dependency and not only a unit this spec reaches into. 007 and
  # 009 both declare the spec they extend; this one did not, and the omission
  # was an oversight rather than a position.
  - "006-command-surface"
---

# 008: The first provider adapter

## 1. Purpose

Spec `004` fixed one seam and then forbade a provider name inside it, which was
the right order and leaves the product unable to run anything: no spec names a
provider, so no adapter set exists, so `env plan`, `env apply`, `env upgrade`,
`env remove` and `doctor` refuse with exit 2 and say why. That refusal is honest
and is not a gap in `006`. It is this spec's absence.

This spec is the first thing in the corpus permitted to name a provider. Its
value is not that it names one; it is that naming one turns `004`'s capability
vocabulary from a design into a set of claims that can be checked, and the
checking found that two of them are not supportable in the obvious way.

## 2. Territory

`crates/statecraft-adapter-claude-code/`, **claimed by the change that writes
it, not by this draft**. The gate carries `index check --fail-on-unresolved`, so
a spec that claims a crate before writing it fails (`AGENTS.md`, "The gate"). The
claim belongs in the implementing change, which is where the ownership ratchet
wants it anyway.

Not this spec's territory: the seam (`004`), the supervisor's classification of
an attempt (`003`), and any second provider.

## 3. Behavior

Every measurement below was taken against **Claude Code 2.1.267 on 2026-09-17**,
on `darwin`, in a scratch git work tree, with `claude --print --output-format
stream-json --verbose`. A measurement binds to that version and transfers to no
other (`004` section 3.4).

### 3.1 What is spawned, and the three parts

The adapter spawns `claude --print --output-format stream-json --verbose`. The
prompt is delivered on a stream and never interpolated into a command line
(`004` section 3.1).

| `004` section 3.1 part | Provider event |
|---|---|
| The request | Process arguments and a settings document, plus the prompt on stdin. |
| The init event | `{"type":"system","subtype":"init"}`, carrying `claude_code_version`, `model`, `permissionMode`, `tools`, `mcp_servers`, `skills`, `agents`, `cwd` and `apiKeySource`. |
| Progress events | `assistant` and `user` events, and `system` events including `hook_started` and `hook_response`. |
| Refusal events | **There are none.** See section 3.3. |
| The result | `{"type":"result"}`, carrying `subtype`, `is_error`, `terminal_reason`, `stop_reason`, `num_turns`, `total_cost_usd`, `usage`, `modelUsage` and `permission_denials`. |

### 3.2 The capability tokens, each against a measurement

`004` section 3.2 closed the vocabulary at six tokens. A manifest declaring a
token the adapter does not honor fails qualification (`004` section 3.5, case 6),
so each declaration below names what was measured and what the measurement did
not establish.

| Token | Declared | Measured basis |
|---|---|---|
| `turn-limit` | **Yes** | `--max-turns 1` against a prompt needing three tool calls terminated with `subtype: "error_max_turns"`, `terminal_reason: "max_turns"`, `is_error: true`, and process exit 1. |
| `cost-report` | **Yes** | `total_cost_usd` present and non-zero on the result event (`0.044009` on a one-turn session). Absence is reported as `unknown` and never as zero (`004` section 3.5, case 5). |
| `hook-enforcement` | **Yes** | `system` events with subtypes `hook_started` and `hook_response`, the latter carrying `hook_name`, `hook_event`, `outcome`, `exit_code`, `stdout` and `stderr`. A blocked action is therefore a structured event. |
| `structured-refusals` | **Yes, and only through one mechanism** | See section 3.3. A permission deny rule produced `permission_denials: [{"tool_name":"Bash","tool_use_id":"...","tool_input":{...}}]`. Tool-set removal produced no record at all. |
| `tool-allowlist` | **Yes, with a stated blind spot** | See section 3.4. The restriction is honored; what was applied is only partly observable. |
| `workspace-write` | **Not declared in the first slice** | Not measured. `004` section 3.3 makes an undeclared token a refusal when a run requires it and a recorded degradation when a run prefers it, which is the correct answer for a claim nobody has checked. Declaring it later is an amendment with its own measurement. |

### 3.3 A denied session classifies itself as a success

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

### 3.4 Two denial mechanisms with opposite evidence properties

Measured on the same version, with the same prompt:

| Mechanism | Reflected in the init event | Produces a refusal record |
|---|---|---|
| `--disallowedTools Bash` | **Yes**: `tools` had 89 entries and `Bash` was absent. | **No**: `permission_denials` was empty. The session reported in prose that it had no such tool. |
| A `permissions.deny` rule | **No**: `tools` had 88 entries and `Bash` was present. | **Yes**: one structured entry with the tool, the id and the input. |
| `--allowedTools Read` | **No**: `tools` had 88 entries including `Bash` and `Edit`. | Not measured. |

Neither mechanism alone satisfies `004` section 3.3, which requires the init
event to carry what was **actually applied** beside what was requested, and
`004` section 3.1, which requires a refusal to be a structured event rather than
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
one would satisfy `004` section 3.3's letter and defeat its purpose, which is why
this is written down rather than left to whoever implements it.

### 3.5 The outcome mapping

The adapter does not classify. It reports the provider's terminal fields and the
supervisor maps them onto `003` section 3.4's closed set. The mapping is fixed
here so two adapters cannot disagree about it:

| Provider terminal state | `003` section 3.4 outcome | Why |
|---|---|---|
| `subtype: "success"`, `permission_denials` empty | `completed` | Reached its own end. Says nothing about acceptance. |
| `subtype: "success"`, `permission_denials` non-empty | `refused` | Section 3.3. The completed turns are retained beside the refusal. |
| `terminal_reason: "max_turns"` | `interrupted` | The cap stopped the attempt before anything was judged. It is **not** `failed`: nothing about the work was found not to hold. The provider calls it an error and that reading is not adopted. |
| The deadline passed and the child was killed with its descendants | `interrupted` | `004` section 3.5, case 3. |
| A malformed or truncated stream | Reported as malformed | `004` section 3.5, case 4. Never read as a clean completion with missing fields. |

`cancelled` is never produced by the adapter: it means an operator stopped the
attempt deliberately, which the supervisor knows and the provider does not.

### 3.6 Credentials, and what the constructed environment cannot drop

`004` section 3.6 constructs the child environment rather than filtering it, and
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
   the attempt. This is the residual `004` section 3.6 already declared, named
   here concretely for this provider rather than left general.

### 3.7 The environment half

Per `002` section 3.9 the adapter declares the harness it targets, the exact set
of paths it would manage, the facts it cannot express in that harness, and the
prerequisites it needs present. Two adapters may not declare the same path, and
this is the first, so nothing collides yet and the check still runs.

Its prerequisites are: a `claude` executable resolvable by the constructed
environment, a version it has a qualification record for (section 3.8), and the
credential path of section 3.6. Absent any of them it **refuses to claim its
paths and names which one is absent**. It does not write files for a harness that
is not there.

### 3.8 Qualification binds to a version, and to nothing else

`004` section 3.4: an adapter binary version is qualified only by a recorded pass
of `004` section 3.5's negative suite, and the record names the binary version,
the suite version and the date. An adapter with no qualification record **runs**
and is labelled `unqualified` everywhere it appears.

For this adapter the record names two versions, not one: the adapter's own build
and the **provider** binary it was measured against. Every finding in sections
3.1 to 3.6 is a fact about Claude Code 2.1.267. A provider upgrade invalidates
the qualification even when the adapter is byte-identical, because what was
qualified was the pair.

### 3.9 Observable negative cases

| Case | Required behavior |
|---|---|
| A result with `permission_denials` non-empty and `subtype: "success"` | Attempt outcome `refused`, the completed turns retained, the denial entries recorded verbatim. Never `completed`. |
| A required capability token this manifest does not declare | Refused before any process is spawned, naming the token (`004` section 3.3). No partial spawn. |
| A run prefers `workspace-write` | Runs, and the degradation is recorded on the attempt and carried in the result. Never silent. |
| `terminal_reason: "max_turns"` | Attempt `interrupted`, not `failed`. Acceptance `not-attempted`, reason `attempt-interrupted` (`005` section 3.1.1). |
| The provider exits 0 with a denial recorded | The exit code is not read. Section 3.3. |
| The provider exits 1 on a turn cap | The exit code is not read. Section 3.5. |
| The applied tool allowlist is asked for | `not-recorded`, never the requested list restated as applied. Section 3.4. |
| A tool restriction expressed as tool-set removal where a refusal record is required | Fails qualification: the manifest declared `structured-refusals` and this path produces none. Section 3.4. |
| `claude` is absent from the constructed environment | The environment adapter refuses to claim its paths and names the absent prerequisite. No files written. |
| The provider binary version differs from the qualification record | Labelled `unqualified` in the posture, the attempt record and the outcome. It still runs. |
| A second provider adapter declaring a path this one declares | Refused at plan time, naming both adapters and the path (`002` section 3.10). |

## 4. Out of scope

A second provider, and any shared abstraction extracted from having two. The
interactive mode of this provider. Authentication setup, credential rotation and
anything that would write a credential. Publication and distribution (`F-02`).
Whether the supervisor should ever require `workspace-write`, which needs a
measurement this spec did not take.

## 5. Decisions recorded during implementation

Dated entries for choices §3 was silent on. None changes what §3 requires.

**2026-09-17: run qualification is persisted posture, not target qualification.**
Sections 3.8 and 3.9 and `004` sections 3.4 and 3.7 already require the labels.
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
3.4 and 3.5 and this spec's section 3.3. Missing initialization without a denial
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
repair discharges. Section 3.1's mid-stream refusal-event contradiction remains
an owner amendment; the existing mapper still carries that event as progress
and takes refusal evidence from the terminal `permission_denials` only.

**2026-09-17: the applied set reports what the invocation put into effect, minus
what the init event contradicts.** Spec 004 §3.3 wants the init event to carry
what was *actually applied*, and §3.5.6 makes a declared-but-unapplied token a
qualification failure. §3.4 measured that this provider's init event witnesses
**one** of the five tokens directly, the tool set, and only under removal. So
neither extreme works: reporting everything granted would make §3.5.6 vacuous,
and reporting only what init proves would fail qualification on every real run.
The adapter reports what it put into effect and drops what init contradicts,
which today is exactly one thing (`hook-enforcement` before any hook event is
seen). The applied **tool set** stays `not-recorded`, which is §3.4's own answer
and is unaffected by this entry.

**2026-09-17: a terminal state §3.5's table does not list is reported, not
mapped.** The table covers `success` and `max_turns`. A subtype nobody measured
(`error_during_execution`, say) is returned as an unmapped terminal state, the
way spec 004 §3.5 case 4 returns a malformed stream. An outcome this adapter
invented would be an outcome nobody measured.

**2026-09-17: `credential-path` is the presence of the mechanism, not of a
credential.** §3.6 measured `apiKeySource: "none"` and concluded the keychain
answers on `darwin`. Checking that a credential *works* means spending one,
which §4 puts out of scope, so the observable fact is the platform. On a
platform where §3 took no measurement the prerequisite reads **absent** rather
than assumed, because every finding in §3.1 to §3.6 is a fact about `darwin`.

**2026-09-17: `D-09`'s pointer prerequisite is not a fourth prerequisite here.**
Spec 002 §3.8 makes "this harness loads a pointer at this path" a declared
prerequisite, and §3.7 of this spec lists three that are not it. For this
harness the mechanism is the `@path` import in `CLAUDE.md`, which this provider
loads, so it is satisfied by construction and cannot be absent. The three §3.7
names are the ones that can be, and the declaration names exactly those.

**2026-09-17: the environment half's two rows reach the real declaration through
a dev-dependency.** §3.9's absent-prerequisite and colliding-path rows are
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

## Verification

Each line is one command. §3.9's eleven rows are integration tests named after
the rows they cover. Nine live in this adapter's own crate, in
`tests/negative_cases.rs`. Two are the environment half (§3.7): the absent
prerequisite and the colliding declared path are behaviors of the environment
adapter, so they live in the crate `002` owns, under the `extends` edge this
spec's frontmatter declares, and the second `cargo test` line is what runs them.

**The provider is not spawned by the acceptance, and that is a decision rather
than a shortfall.** Every finding in §3.1 to §3.6 was measured against a live
Claude Code 2.1.267, and §3.8 binds the qualification to that pair. A command
here that spawned the provider would need a credential, and §3.6 measured that
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

The live measurement stays where §3.8 puts it: a qualification act performed by
an operator against a named binary version and recorded, never re-derived by a
check. What the suite checks is the consequence rather than the act. An adapter
whose qualification record does not name the provider binary in front of it is
labelled `unqualified` everywhere it appears, and still runs.

The last command is `004` §3.8's guard, run from here on purpose. This is the
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
cargo test -p statecraft-adapter-claude-code --test negative_cases
cargo test -p statecraft-environment --test negative_cases
test -f crates/statecraft-adapter-claude-code/src/lib.rs
test -d crates/statecraft-adapter-claude-code/testdata/stream
cargo test -p statecraft-adapter --test no_provider_names
```
