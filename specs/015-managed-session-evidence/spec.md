---
id: "015-managed-session-evidence"
title: "Managed-session evidence and admission"
status: approved
implementation: in-progress
created: "2026-09-26"
summary: >
  Carries the live-observation, control, startup-record, launch-state, managed-trial, trailer, and confinement requirements relocated from spec 002. It separates runtime evidence from installation and harness delivery.
depends_on:
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
  - { spec: "004-execution-adapter", unit: { kind: directory, path: "crates/statecraft-adapter-claude-code/" }, nature: additive }
relocates:
  - { spec: "002-environment-lifecycle", from: "3-29-what-admits-a-live-observation", to: "3-29-what-admits-a-live-observation" }
  - { spec: "002-environment-lifecycle", from: "3-30-what-each-control-must-demonstrate-and-what-binds-a-capture-to-its-launch", to: "3-30-what-each-control-must-demonstrate-and-what-binds-a-capture-to-its-launch" }
  - { spec: "002-environment-lifecycle", from: "3-31-the-startup-record-a-run-writes-and-the-harness-revision-that-answered", to: "3-31-the-startup-record-a-run-writes-and-the-harness-revision-that-answered" }
  - { spec: "002-environment-lifecycle", from: "3-32-launch-states-the-correlated-acknowledgment-per-invocation-startup-delivery-and-when-governed-work-is-released", to: "3-32-launch-states-the-correlated-acknowledgment-per-invocation-startup-delivery-and-when-governed-work-is-released" }
  - { spec: "002-environment-lifecycle", from: "3-33-the-managed-startup-trial-and-why-it-is-not-the-permission-experiment", to: "3-33-the-managed-startup-trial-and-why-it-is-not-the-permission-experiment" }
  - { spec: "002-environment-lifecycle", from: "3-34-one-allowlisted-event-after-the-terminal-event", to: "3-34-one-allowlisted-event-after-the-terminal-event" }
  - { spec: "002-environment-lifecycle", from: "3-37-where-an-attempt-s-launch-records-live-and-what-the-child-is-given", to: "3-37-where-an-attempt-s-launch-records-live-and-what-the-child-is-given" }
---

# 015: Managed-session evidence and admission

## 1. Purpose

A managed session is admitted only from correlated launch records and bounded control evidence. Installation or hook presence alone never proves that a particular run received or obeyed the required harness.

## 2. Territory

The requirements below govern live-observation admission, controls, startup and trial records, launch states, one allowlisted trailing event, and the attempt-scoped record boundary. Implementing code remains in crates already owned by specs 002 and 004.

## 3. Behavior

### 3.29 What admits a live observation

A narrowly scoped authority amendment, settled by the owner on 2026-09-21 and
recorded before the implementation it authorizes. Section 3.26 fixed that a
live observation is the third evidence class and that only a session produces
one. Section 3.28 fixed that qualification is measured rather than read off the
configuration. This section fixes **what a claimed observation has to carry**
before it is admitted as one, because the first implementation of that
admission read the provider's prose and a sentence saying the opposite of a
refusal satisfied it.

**The defect this closes, exactly.** The admission accepted a transcript that
contained the command text together with any of the words `permission`,
`blocked`, `refus`, `denied` or `not allowed`. The transcript
`cargo publish --dry-run: permission granted; command executed successfully`
satisfies that predicate, and was admitted as evidence that the command had
been refused. The repair is not a longer word list and not a more elaborate
reading of natural language: a claim about enforcement is admitted from
**structured evidence the harness itself emits**, or it is not admitted.

**Rule 1: provider prose alone cannot establish enforced refusal.** Text a
model emitted is a statement by the model. A refusal that survives as evidence
is a structured record the harness produced, of the kind spec `004` section 3.4
requires a refusal-bearing restriction to produce, naming the tool, the
tool-use id and the tool input verbatim. Where the supported adapter exposes
that record, it is what the admission reads. Prose may accompany it and is
never what is read.

**Rule 2: the controls are part of the evidence, not part of a procedure.**
An observation qualifies only when all three of the following are present and
each one succeeded:

| Control | What it is | Why it is required |
|---|---|---|
| the refusal | the claimed command, run with the managed-session payload | the observation itself |
| the allowed command | a command no floor entry claims, run with the same payload | without it the refusal is consistent with a payload that refuses everything |
| the absent payload | the same claimed command, run with no payload | without it the refusal is evidence for the operator's own configuration rather than for this payload |

These are conditions on the evidence a qualification boundary accepts. A
document that describes them, a checklist that recommends them, or an operator
who remembers them is not what this rule means: the boundary that admits the
observation refuses one whose controls are absent or whose controls did not
behave as the table says.

**Rule 3: missing, contradictory, substituted or mismatched evidence refuses
qualification.** A capture that is empty, that does not parse as the harness's
own structured output, that reaches no terminal event, that carries no refusal
record for the claimed command, or that carries a refusal for the allowed
command, refuses the claim. Two controls presenting the same captured bytes is
substituted evidence and refuses the claim, because one capture cannot be two
measurements.

**Rule 4: evidence for another invocation or settings payload cannot qualify
this one.** The observation is bound to the invocation that produced it: the
program and arguments as spawned, the working directory, the settings payload
by digest, and the harness version the capture itself reports. A payload digest
that is not this build's, a version that disagrees with the capture, a refusal
control whose invocation does not carry the payload, or an absent-payload
control whose invocation does carry one, all refuse the claim.

*Amended by section 3.37 rule 4:* for a confined launch, the program and arguments are the provider's as handed to the confinement; the confinement is bound apart from them, by its mechanism and profile or ruleset digest.

**Rule 5: the same rules govern every route.** Construction, deserialization,
and conversion from any other qualification type reach an admitted observation
only through these rules. A record read back from a file is re-checked against
them before it is treated as qualified, so writing the word into a file by hand
is not a weaker route to the same claim; it is not a route at all.

**Rule 6: unverified is the answer when the evidence cannot decide.** Where the
supported adapter cannot expose evidence sufficient to distinguish a permission
refusal from a model's statement about one, the result stays unverified and the
session is reported as not qualified. Section 3.27's last paragraph already
refuses the two repairs that would hide this. This section adds the third: the
admission is not loosened so that a claim succeeds. A truthful inability to
qualify is the correct outcome, and an invented proof is not an outcome at all.

**What is preserved.** The original captured bytes and their provenance are
kept with the record, not summarized into it. The admission is reviewable
because what it was made from is kept and can be re-read, which is the property
section 3.26 already relies on and which rules 3 and 5 now depend on.

### 3.30 What each control must demonstrate, and what binds a capture to its launch

A narrowly scoped authority amendment, settled by the owner on 2026-09-22 and
recorded before the implementation it authorizes. It sharpens section 3.29
rules 2 to 4. Rules 1, 5 and 6 are unchanged and govern everything below.

**The defects this closes, exactly.** The admission written under section 3.29
read each capture's assistant turns for tool-use **requests** and treated a
request with no matching denial entry as a control that ran. A request is not
an execution, and the absence of a denial entry is not the presence of a
result. It matched a denial to a command by the command text alone, so a
denial on another tool, or on another tool use, carrying the same text
satisfied it. It kept only the last init event and the last terminal event, so
a capture holding two sessions, or two disagreeing terminal events, was read as
one. And it bound an invocation to its settings by checking that a
`--settings` token appeared somewhere in a caller-authored argument list and,
separately, that caller-supplied bytes digested to the payload. Neither check
relates the argument to the bytes, and the argument list was written by the
same caller after the fact.

**Rule 7: a control is judged from correlated structured events.** Everything
below is read through the supported adapter's own types (spec `004` section
3.9), extended there where a field it carries was not yet typed, and never
through a second parser in this spec's crate. The fields read are the ones the
recorded Claude Code `2.1.267` streams under
`crates/statecraft-adapter-claude-code/testdata/stream/` carry: the session id
on every event; the init event's version and working directory; each assistant
`tool_use` block's id, tool name and input; each `tool_result` block's
tool-use id, error flag and content, with the carrying event's
`tool_result_meta` and its `non_execution_kind`; the mid-stream
`permission_denied` event's tool-use id, tool name and decision reason; and the
terminal event's `permission_denials`. A field the provider does not emit is
not required, and no fixture invents one.

**Rule 8: a capture is one session, read whole.** A capture is admissible only
when it holds exactly one init event and exactly one terminal event, the
terminal event is its last event, no turn precedes the init event, every
system, assistant, user and terminal event names the same session, tool-use ids
are unique, every tool result names a tool use earlier in the same capture and
no tool use has two results, every denial entry and every mid-stream denial
names a tool use in the same capture whose tool name agrees, every denial
entry's input is that tool use's input verbatim, and the init event's working
directory is the directory the launch recorded. Anything else is conflicting,
duplicated, mixed-session or incomplete evidence and refuses the claim.

**Rule 9: each tool use is classified, not inferred.** The governed tool is the
one the floor's entries name, `Bash`, and a command is its input's `command`
string compared exactly. For one tool use:

| Classification | What must be present |
|---|---|
| refused | a terminal denial entry for its id, and its tool result marked as not executed by `non_execution_kind` |
| executed | a tool result for its id, no `non_execution_kind` on it, no denial entry and no mid-stream denial for its id |
| unresolved | anything else: a request with no result, a result marked not executed with no denial behind it, or a denial whose result is not marked |

An unresolved use refuses the claim. A request with the expected command text
under another tool's name is not a use of the governed tool, and a denial on it
proves nothing about the floor.

**Rule 10: what each control proves.** Section 3.29 rule 2's table, stated as
outcomes:

| Control | Required outcome | Not required |
|---|---|---|
| the refusal | at least one governed use of the claimed command, and every such use **refused** | anything about the command's own exit |
| the allowed command | at least one governed use of the allowed command, every such use **executed**, and at least one whose result is not an error and whose content, trailing line breaks removed, is exactly the expected output | |
| the absent payload | at least one governed use of the claimed command, and every such use **executed** | the command succeeding |

A control whose capture carries any other tool use refuses the claim, because
evidence about a session that did something else is not evidence about this
one. **Permission success and command success are different facts.** The
absent-payload control proves that the claimed command reached execution with
no permission refusal, under the same grant and without the payload. The
command is chosen so that it fails harmlessly once it runs, and it is expected
to: `cargo publish --dry-run --manifest-path statecraft-absent/Cargo.toml` names
a manifest path that does not exist, so cargo stops before resolving anything,
searches no ancestor directory, and contacts no registry. A result flagged as an
error there is the command failing, which is what it was chosen to do, and it
is not a refusal unless the harness marks it as one.

A denied command that also shows evidence of executing is not a refusal. Two
uses of the claimed command in the refusal control, one refused and one
executed, refuse the claim.

**Rule 11: terminal and process conditions are judged per observation, and
kept.** A control is measurable only when the process ended by itself inside
its deadline, no signal ended it, nothing in its process group outlived it,
its exit code is `0` or `1`, and that code agrees with the terminal event's
error flag the way every recorded stream agrees (`0` with `is_error: false`,
`1` with `is_error: true`). Its terminal reason is `completed` or `max_turns`.
The turn cap is admitted because the measurement shows the tool result arriving
before the capped terminal event (`max-turns.jsonl`, `--max-turns 1`), so the
cap ends a session whose control has already produced its evidence. An
`api_error`, a terminal state spec `004` section 3.13 does not map, a timeout, a
signal or a survivor leaves the control unmeasured and refuses the claim. The
terminal state and the process end are preserved in the record either way.

**Rule 12: the launch is the evidence of the invocation.** Section 3.29 rule 4's
binding is made by the operation that launches the process, not by a
description written afterwards. This product's own launch (spec `006` section
3.11.2) constructs the argument vector, writes the payload to a settings file,
resolves and digests the executable, reads its version, supervises the process
in its own process group under a deadline, and records together: the executable
as requested and as resolved, with its digest; the version it reported; the
argument vector and working directory; the prompt, which travels on standard
input and never in an argument; the settings path, the exact settings bytes, and
their digest before the launch and after the process ended; standard output and
standard error, separately and verbatim; the exit code, signal, timeout and
survivors; a capture identity; and the control it is.

The admission recomputes the argument vector this build constructs for the
recorded control, commands and settings path, and requires it **exactly**. So
a payload control carries one `--settings` argument naming the recorded file,
and the absent-payload control carries none in either spelling, `--settings
<path>` or `--settings=<path>`; a second settings argument, a substituted path,
a reordered or additional argument, or bytes that changed while the process ran
refuse the claim. Because the prompt is not an argument, no text in it can be
read as an option. Three controls are three launches: their capture
identities, their sessions and their tool-use ids are pairwise distinct, and
two controls naming one session are substituted evidence however differently
their bytes are formatted.

**The trust boundary.** A launch record is **launcher-attested**. It
establishes that this product started this executable with these arguments,
this working directory and these settings bytes, and received these bytes back.
It does not establish that the provider **loaded** the settings: that is
inferred from behavior, which is what the three controls are for. It does not
authenticate its own origin either. A record edited by hand after the fact and
still consistent is admitted, because nothing here signs it, and no field and
no rendering claims cryptographic provenance. What makes it reviewable is that
the bytes are kept, which is section 3.29's closing paragraph.

**Synthetic captures stay synthetic.** A capture launched against a fake
provider is marked `synthetic` by the launching operation, at the operator's
explicit request. Synthetic evidence runs the whole admission, so the path is
testable end to end, and an observation admitted from any synthetic control is
**never** a live observation: the record does not qualify and every rendering
says synthetic. The mark prevents this product's own fixtures from being
presented as earned; it does not detect a forgery, which the paragraph above
already disclaims.

**Records written before this section.** They carry no launch record. They
still deserialize, their bytes and provenance stay in the file untouched, and
the admission refuses them for the missing launch, so they read as not
qualified. No record is rewritten, migrated or deleted.

**The experiment these rules judge.** The acceptance script is the operator's
route, and its contract is part of this amendment:

- **Commands.** Refused: `cargo publish --dry-run --manifest-path
  statecraft-absent/Cargo.toml`, which the floor's `Bash(cargo publish*)`
  claims. Allowed: `echo statecraft-allowed-control`, whose expected output is
  `statecraft-allowed-control` and which no floor entry claims.
- **One grant, identical in all three.** Every launch carries
  `--allowedTools` naming exactly those two commands. Without a grant a
  non-interactive session refuses an unapproved command whatever the payload
  says, so the absent-payload control could never show execution and the
  refusal could not be attributed to the payload. With the grant identical,
  the payload is the only difference between the refusal and the absent-payload
  launches. That the provider's deny entry prevails over the grant is a premise
  the refusal control **tests**: if it does not, the refusal control records an
  execution and the claim is refused. The payload itself still carries no allow
  entry, which is section 3.24's and section 3.28's rule and is untouched.
- **Turns.** `--max-turns 1`, for rule 11's measured reason.
- **Sessions.** At most **three** provider sessions: the refusal, the allowed
  command and the absent payload, in that order. The first control whose launch
  does not complete ends the experiment, and no later session is started. There
  is no retry, no replay after an uncertain outcome, and no conditional extra
  session. Each launch also runs the provider's `--version` once, which is a
  version probe and not a session.
- **Bounds.** Each session has its own deadline (default 300 seconds), each
  version probe 30 seconds, so the whole stage is bounded by three sessions and
  three probes.
- **Approvals.** The provider stage refuses unless
  `APPROVED_PROVIDER_SESSION=yes`; the real-home coexistence stage refuses unless
  `APPROVED_REAL_HOME_COEXISTENCE=yes`. Neither implies the other.
- **The local test route.** `SC_ACCEPTANCE_FAKE_PROVIDER=<path>` runs the
  provider stage's whole control flow against a local executable instead of the
  provider. It never reads the provider approval, refuses to run when that
  approval is also set, marks every capture synthetic, and reports its result as
  synthetic.

*Amended by section 3.37 rule 4:* for a confined launch, the argument vector recomputed and required exactly is the provider's as handed to the confinement; the wrapper's own arguments are not part of it, and the confinement is recorded and bound apart.

### 3.31 The startup record a run writes, and the harness revision that answered

A narrowly scoped authority amendment, settled by the owner on 2026-09-22 and
recorded before the implementation it authorizes. Section 3.26 fixed what a
managed session records at its start, and section 3.25 fixed that the resolved
identity is recorded per session and says which revision actually answered.
Neither was true of `run`: it wrote no startup record at all, and the
`startup record` and `startup qualify` verbs filled the resolved identity in
with the **required** one, so a revision nobody measured read as `exact` and
resolved. This section fixes when a run's record is written, what binds it to
the attempt, what measures the answering revision, and what each grade of that
measurement establishes.

**The defect this closes, exactly.** A required revision is not an observed
one, and a verified directory on disk is not proof that the launched session
used it. Recording the requirement as the resolution is the substitution
section 3.26 forbids between its first and third statements, applied to the
harness instead of to the instructions.

**Rule 13: a run's startup evidence is two write-once records per attempt,
under the attempt's identity.** Both live under the project's ignored runtime
state, at `.statecraft/state/startup/runs/<run>/<attempt>/`, where `<run>` and
`<attempt>` are the run id and attempt number spec `003` section 3.4 assigned.
*Amended by section 3.37:* the directory is now in the product home, and the
gate's files are in a separate exchange directory.

| Record | Written | Holds |
|---|---|---|
| `intent.json` | after the attempt's intent is appended and every preflight has passed, **before** the process is created | the attempt identity; the project identity; the workspace the session starts in; the load chain and instruction-file identities evaluated **in that workspace**; the required identity; the standing before launch; the **selected** revision; the adapter identity; the resolved program; the payload's digest, length and argument; and a fresh binding nonce |
| `record.json` | after the process ended, or after the launch failed | the section 3.26 record: its seven fields and the added evidence, plus the attempt identity, the digest of the `intent.json` bytes it finalizes, the provider's session id and version as its stream reported them, the process outcome, the settings bytes the adapter actually wrote, and the **observed** revision with the grade of its evidence |

Neither is ever rewritten. A record already present under the attempt's
identity refuses the write, so a previous attempt's evidence is never
overwritten and never adopted by a later attempt. `record.json` names the
digest of the intent it finalizes, so the pair is judged together and an intent
changed after the fact no longer binds.

**Rule 14: preflight refusal and a launch are different facts.** A run refused
before its attempt is appended (section 3.25's refusals) writes neither record.
An attempt concluded `refused` before a process was created (the adapter's
preflight, an unresolvable executable) writes neither record, and its refusal
stays in the attempt record where spec `003` puts it. `intent.json` exists if
and only if this product was about to create the process. A record read without
its intent is not evidence of a launch.

**Rule 15: a recording failure stops or reports, never proceeds silently.** If
`intent.json` cannot be written, nothing is launched: the attempt is concluded
`refused` with the guard `startup-record` and the reason. If `record.json`
cannot be written after the process ended, the attempt is concluded with its
real outcome, its detail says the record was not stored and why, and `run`
exits 4, spec `006` section 3.10's failed row. If this product's own process
ends between the two writes, the intent stays and no record is fabricated, and
reconciliation (spec `003` section 3.6) owns the attempt record. *Amended by
section 3.32 rules 22 and 23:* this rule first said such an attempt reads as
launched and interrupted, and an intent does not establish either.

**Rule 16: supply is recorded by the launch that performs it.** In a run, the
bytes this product hands to the session are two things, and each is recorded
by the operation that hands it. The **instruction chain** is in the workspace
this product prepared and starts the session in: the launch reads each file the
load rule reaches in that workspace immediately before the spawn and records
its digest. The **settings payload** is the document the adapter writes: the
adapter reports the exact bytes it wrote to the settings file it passes, and the
record carries their digest. The supply is `supplied` only when the process was
created, the chain reached the managed file, and the written bytes digest to
the payload this build records; a spawn that failed is `failed`; a chain that
does not arrive is `not-attempted`. Supply established this way says the bytes
were in the session's working tree and on its command line at spawn. It does
not say the provider read them, which is section 3.26's first and third
statements, unchanged.

**Rule 17: the selected revision is the launch configuration, and the observed
revision is measured.** The run **selects** the required revision after the
standing has established that it is installed and intact, and never any other
(section 3.25: no latest, no substitute). A project that commits no
requirement selects nothing. The selection is carried to the session in its
constructed environment, beside the attempt binding: `STATECRAFT_RUN_ID`,
`STATECRAFT_ATTEMPT`, `STATECRAFT_STARTUP_NONCE` and
`STATECRAFT_HARNESS_SELECTED`. None of them is a credential and none carries
one.

What **answered** is measured by the shipped `SessionStart` hook. When the
nonce is present in its environment and the manifest gate of section 3.14 rule
3 passes, it prints one acknowledgment line on its standard output, and writes
nothing:

```text
statecraft-startup<TAB>v1<TAB>nonce=<n><TAB>run=<id><TAB>attempt=<k><TAB>selected=<digest-or-none><TAB>project=<dir><TAB>root=<revision-dir>
```

`root` is the revision directory the executing script is in, resolved by the
script from its own path. Claude Code `2.1.267` reports each hook's standard
output in its stream as a `hook_response` event carrying the session id, the
hook event, the exit code and the output verbatim, which the recorded streams
under `crates/statecraft-adapter-claude-code/testdata/stream/` show. The
adapter types those fields (spec `004` section 3.9) and hands the responses to
the run; this spec's crate judges them.

**Rule 18: what an acknowledgment must satisfy, and what each failure is.** The
observed revision is admitted only from exactly one acknowledgment, in a
`hook_response` event for `SessionStart` that exited `0`, in this attempt's own
stream, whose session id is the session id of that stream's init event, and
whose nonce, run, attempt, selected revision and project all equal what
`intent.json` recorded (the project compared as the canonical workspace path).
The revision directory it names is then read and digested, and that full digest
is the **observed** identity. Anything else is **unverified**, and the record
names which:

| Evidence | Recorded as |
|---|---|
| no acknowledgment in any `SessionStart` response | `unverified: absent` |
| a line that begins the acknowledgment and does not parse | `unverified: malformed` |
| another attempt's nonce | `unverified: replayed` |
| another run or attempt number, or another selection | `unverified: wrong-attempt` |
| another project directory | `unverified: wrong-project` |
| a session id that is not the stream's own | `unverified: wrong-session` |
| a hook that exited non-zero | `unverified: hook-failed` |
| two acknowledgments naming different directories | `unverified: conflicting` |
| a revision directory that is not directly inside this home's harness store | `unverified: foreign-revision` |
| a revision directory that cannot be read and digested | `unverified: unreadable-revision` |

An unverified observation leaves the resolved identity absent, and absent is
not a match: the standing stays `exact` with nothing resolved, which section
3.25 already makes not qualified. An admitted observation becomes the resolved
identity and the standing is evaluated against it. Its directory's current
bytes are what the digest is over, so a revision that was altered after it
answered reads as what it now is.

**Rule 19: a mismatch observed after launch refuses the attempt.** Section 3.25
refuses managed execution under a mismatch, and a run cannot detect one before
the session starts, because the acknowledgment is emitted by the session. So a
mismatched standing, measured from an admitted acknowledgment, is counted as a
refusal under the guard `harness-identity` when the attempt is concluded, the
attempt is `refused` (spec `003` section 3.5), and `accept` treats it as spec
`005` section 3.1.1 treats any refused attempt. *Amended by section 3.32 rule
26:* this rule first let the session run to its end and refused it afterwards,
which section 3.25's "nothing was done" does not admit. An unverified
observation is not a mismatch; for a project that commits a requirement it
now withholds governed work under section 3.32 rule 26, and it never qualifies
the attempt. A project that commits no requirement records
whatever was observed and is `unrequired`, which never qualifies.

**Rule 20: what each grade establishes, and does not.**

| Grade | Establishes | Does not establish |
|---|---|---|
| installed integrity | the required directory's files digest to the committed full digest, before launch, and again when the record is written | that anything used them |
| launch configuration | this product selected that revision and named it, with the attempt binding, in the environment it constructed for the process it created | that the provider or any hook read the environment |
| correlated acknowledgment | see section 3.32 rule 27, which replaces this row | see section 3.32 rule 27 |

*Amended by section 3.32 rule 27.* This row first said the acknowledgment
establishes that a hook script located in the named revision directory
executed. It does not: the line's origin is not authenticated, the provider's
response does not name the command that printed it, and the directory digest
is taken when the record is written, not when anything ran. The grade is the
**correlated acknowledgment**, and rule 27 fixes what it does and does not
establish. The acknowledgment remains **launcher-attested**, like section
3.30's launch record. Section 3.32 rule 25 replaces the operator's global
registration with a per-invocation one, so a managed run no longer depends on
ambient registration for its startup path.

**Rule 21: a run attempt's judgement, and why `qualified` is not reachable
through a run.** The judgement read back from the two records is one of:

| Verdict | When |
|---|---|
| `not-launched` | the attempt exists and has no `intent.json` |
| `launch-unknown`, `outcome-unknown`, `spawn-failed` | section 3.32 rule 23; an intent with no record is one of the first two and is never `interrupted` |
| `interrupted` | a record whose process was created and did not end by itself as one readable session |
| `mismatched` | an admitted acknowledgment naming a revision other than the required one |
| `not-admitted` | section 3.32 rule 26: a requirement is committed and no correlated acknowledgment was admitted at the startup decision |
| `unverified` | launched and recorded, and not qualified for any other reason, each one named |
| `qualified` | the record's `qualifies()` conjunction holds |

A run session is not one of section 3.29's three controls, and rule 4 of that
section refuses evidence for another invocation as evidence for this one. So a
run attempt's observation is always `not-observed`, and `qualified` is not
reachable through a run: a completed run with a matching acknowledgment is
`unverified`, and it says the live observation is the missing class. The
judgement is recomputed on every read from the bytes in the two records,
including section 3.29 rule 5's re-admission, so a field edited by hand changes
the judgement rather than asserting one, and one attempt's records are never
read as another's.

### 3.32 Launch states, the correlated acknowledgment, per-invocation startup delivery, and when governed work is released

A narrowly scoped authority correction to section 3.31, settled by the owner on
2026-09-22 and recorded before the implementation it authorizes. It closes four
defects, each a claim the evidence did not support.

1. **An intent was read as a launch.** Section 3.31 wrote `intent.json` before
   the process was created and read an intent with no record as launched and
   interrupted. This product can stop after writing the intent and before the
   spawn, and after the spawn and before anything else is written; neither
   inference holds.
2. **An acknowledgment was read as execution provenance.** Section 3.31 rule 20
   said the acknowledgment establishes that a hook script in the named
   directory executed, and in the same row that nobody authenticates who
   printed the line. The second is right, so the first cannot be.
3. **The managed startup path was ambient.** A run delivered only the deny
   floor, and the `SessionStart` hook that acknowledges a start reached the
   session only if the operator had registered it globally. A managed run's
   startup evidence depended on the operator's home rather than on what the
   run supplied.
4. **A mismatch was refused after the fact.** Section 3.25 says a refused
   managed execution is one where "nothing was done". Section 3.31 rule 19 let
   the session run to its end and then concluded it refused.

**Rule 22: a launch is four write-once records, each a separate fact.** Every
file lives in the attempt's directory of section 3.31 rule 13 and is created
exclusively: an existing file refuses the write, and nothing is ever replaced.

| Record | Written | Establishes | Does not establish |
|---|---|---|---|
| `intent.json` | after every preflight has passed, before the spawn is attempted | this product was about to attempt a spawn, with this configuration | that a process was created |
| `launched.json` | immediately after the spawn call returned a process, **before** the prompt is delivered to it | a process with this id was created for this attempt | anything the process did |
| `admission.json` | at the startup decision of rule 26 | the decision, its reason, and when it was made | that the provider honored it |
| `record.json` | after the process ended, after the launch failed, or after the confirmation of a spawn could not be persisted | the completion, as section 3.31 rule 13 describes it, and which of the earlier records this product wrote | anything the earlier records do not |

The order is fixed: intent, spawn, confirmation, prompt, decision, record. The
prompt is written to the process only after `launched.json` is persisted, so a
confirmation that cannot be persisted stops the process before it has been
given any work, and the record says so. A spawn and a file write are not one
atomic transaction, and nothing here pretends they are: between the spawn
returning and the confirmation being on disk there is a window in which a
process exists and no record says so, and rule 23 names what that window reads
as.

*Amended by section 3.37:* the four records live in the product home, and the gate's files in a separate exchange directory.

**Rule 23: the launch states, read back, and what each does not prove.**

| What is on disk | State | What it means | What it does not mean |
|---|---|---|---|
| no intent | `not-launched` | this product attempts no spawn before the intent is persisted, so it created no provider process for this attempt | that nothing else happened: the workspace was prepared |
| an intent, nothing after it | `launch-unknown` | intent persisted; this product stopped before confirming a spawn | that no process exists, or that one does |
| an intent and a confirmation, no record | `outcome-unknown` | a process with the recorded id was created and given its prompt; its outcome is unknown | that it was interrupted, or that it had no effect |
| a record whose spawn call failed | `spawn-failed` | the operating system reported that no process was created | anything about the workspace beyond what the preparation did |
| a record whose confirmation could not be persisted | `interrupted` | a process was created, and stopped before its prompt was delivered | that stopping it undid anything |
| a record of a completed launch | section 3.31 rule 21, with `not-admitted` added by rule 26 | as there | as there |

Absence of a final record never proves an interruption, and absence of a
record never proves that no side effect occurred. `launch-unknown` and
`outcome-unknown` are the honest words for a crash, and each names the files it
read.

**Rule 24: an uncertain launch is never replayed automatically.** The run
record's attempt stays live when this product stops mid-launch, and spec `003`
section 3.6 blocks a retry of an intent whose outcome is unknown. `run` refuses
the next attempt, names the live attempt and its launch state, and gives the
operator the inspection to perform: `startup show <path> <run-id> --attempt
<n>`, the process id where one was confirmed, and the workspace where effects
may have landed. Nothing here infers an outcome to free the lock. The one way
to free it is an operator's reconciliation (spec `003` section 3.6.1, the
`run reconcile` verb of spec `006` section 3.11.6), which the answer names;
until that section, this build had no such verb and the answer said so.

**Rule 25: a managed run supplies its startup hook and its gate explicitly.**
Where the project commits a requirement, the run's settings document is the
deny floor of section 3.27 plus two hook registrations, and nothing else:

- `SessionStart`, matcher `startup`: the selected revision's
  `hooks/statecraft-session-start.sh`, by absolute path in the installed
  revision directory whose integrity section 3.25 has just checked.
- `PreToolUse`, matcher `*`: the attempt's **admission gate**, a script this
  product writes once into the attempt's directory before the spawn. It is
  launcher content, not harness content, so it is in no revision's digest and
  no harness upgrade changes it.

The mechanism is the one section 3.27 already relies on: Claude Code documents
`--settings <file-or-json>` as an additional settings source, and documents
`hooks` as a settings key. Whether `2.1.267` honors hooks supplied that way, in
`--print` mode, and passes the session's environment to them, is
**unobserved** in a live run. Nothing here writes the operator's home, and a
managed run no longer needs section 3.24's global registration to be
acknowledged. The intent records the exact document by digest and length, each
registration by event, matcher, command and the digest of the script it names,
and, separately, the digest of the deny floor alone. The run's document is not
the floor's bytes, so under section 3.29 rule 4 no qualification of the floor
payload is evidence for a run's document, and the record never reads one as
the other. Where the project commits no requirement, nothing is selected, the
document is the floor alone, no gate is written, and the record says work was
not gated by a startup decision.

*Amended by section 3.37:* the gate script is written into the attempt's exchange directory, not beside the launch records.

**Rule 26: startup identity is an admission prerequisite, and governed work is
released only by the decision.** Section 3.25 promises prevention, so the
decision is made before governed work is released, not after the session ends.
*Governed work* is every tool call the session makes: the channel through which
a session changes anything. The gate refuses every tool call until
`admission.json` records `admitted`, waits a bounded time for a decision that
has not yet been written, and refuses when that time passes.

The launcher decides at the first event in the attempt's stream that is not a
`SessionStart` hook event: in the recorded `2.1.267` streams, the init event,
which follows every `SessionStart` `hook_response`. At that point it judges the
acknowledgments it has read under section 3.31 rule 18, and writes:

| Decision | When |
|---|---|
| `admitted` | exactly one correlated acknowledgment is admitted, and the standing evaluated against it is `exact` |
| `refused: mismatched` | an admitted acknowledgment names a revision other than the required one |
| `refused: not-established` | no acknowledgment is admitted, for any reason rule 18 names |

On a refusal the launcher stops the process group at once and concludes the
attempt `refused`, under the guard `harness-identity` for a mismatch and
`startup-admission` otherwise. A stream that ends before the decision point is
decided at its end, the same way.

What this boundary establishes, and what it does not:

- **Establishes:** a tool call that the provider routed through the gate did
  not run before `admitted` was written, and did not run after a refusal. The
  gate appends each consultation to the attempt's `gate.log`, so the record can
  say whether the gate was consulted at all.
- **Does not establish:** that the provider honors the registration. A provider
  that ignores it runs neither the acknowledgment nor the gate; the decision is
  then `refused: not-established`, the process is stopped, and the refusal is
  **retrospective**: the record says effects before termination are not
  excluded. Nor does it establish that nothing happened outside tool calls (the
  provider's own startup, other hooks), or that stopping the process undid
  anything. Prompt termination is not proof of no effect.

*Amended by section 3.37:* the gate reads its copy of the decision, and writes its log, in the exchange directory; the log is child-attested.

**Rule 27: the correlated acknowledgment.** The grade section 3.31 called
`acknowledged` is **`correlated`**, in the record, in the API and on the
command line. An admitted correlated acknowledgment establishes, and no more:

- a `hook_response` for `SessionStart` in this attempt's stream, exit `0`,
  carrying the stream's own init session id, held one line whose nonce, run,
  attempt, selection and project equal the intent;
- the directory that line names is directly inside this home's harness store,
  and its files, read when the record is written, digest to the recorded
  identity;
- the per-invocation registration of rule 25 named the script in the selected
  revision directory.

It does **not** establish which process printed the line: the provider's
response does not name the command, and the nonce is in the session's
environment, so any hook or process that runs there can print it. It does not
establish that the script at that path is the one that ran, that its bytes when
it ran equal the bytes digested, or that any other file of the revision was
loaded. "Executed" is never claimed. A record written before this section
carries the grade `acknowledged`; it is read as `correlated` with the same
fields and judged the same, and nothing is rewritten.

**Rule 28: the evidence words do not collapse.** Each names one fact and none
implies another: **installed** (a revision directory whose files digest to its
name), **selected** (named in the launch configuration), **supplied** (bytes
in the work tree and on the command line at spawn), **correlated** (rule 27),
**admitted** (rule 26's decision released governed work), and **qualified**
(section 3.29's live observation, unreachable through a run). A record whose
evidence is insufficient under these rules, including one written before them,
stays inspectable and is never qualified by being read.

### 3.33 The managed-startup trial, and why it is not the permission experiment

Settled by the owner on 2026-09-22 and recorded before the implementation it
authorizes. It corrects a reading, adds one bounded experiment, and states what
the ordering in section 3.32 rule 26 assumes of the provider.

**The correction.** Section 3.30's permission experiment launches its three
controls through `startup capture`. Each carries the deny floor's payload and
nothing else: no `SessionStart` registration, no admission gate, no `run`, no
intent and no decision. Its result says whether the floor's deny rule was
enforced against one command on one version. It says nothing about whether a
provider runs hooks supplied through `--settings`, reports them before `init`,
consults a `PreToolUse` gate or waits for it, which are the premises section
3.32 rules 25 and 26 rest on. The handoff's statement of what is outstanding
put that uncertainty beside the permission stage's approval as though running
the stage would retire it; it would not, and the handoff is corrected. The
two experiments answer two questions, their results are reported separately,
and neither result, nor both together, is a qualification: section 3.29's
observation is unreachable through a run (rule 28), and the trial adds no
route to it.

**Rule 29: the trial asks one question.** On the installed provider version,
when a managed run supplies its startup hook and its admission gate through
`--settings` in `--print --output-format stream-json --verbose` mode:

1. does the startup hook's response arrive in the attempt's own stream,
   carrying this attempt's binding;
2. does it arrive before the decision point;
3. is the gate consulted before a requested tool executes; and
4. does a tool execute only after the decision admitted the attempt?

**Rule 30: the trial is a run attempt, not a second launcher.** It goes
through the same preparation (intent, gate, settings document, sentinel of
rule 31), the same supervisor and launch watch, the same finalization and the
same run record as `run`. It differs from `run` in four values and in nothing
else: the run id is the fixed `statecraft-startup-trial`; the prompt is rule
31's instruction; the turn limit is `--max-turns 3`; and the deadline is the
operator's, 120 seconds by default and never more than 300. It consults no
scheduler, because the trial is not a unit of work, and it refuses in a
project that commits no harness requirement, because an ungated run cannot
answer rule 29.

**Rule 31: a harmless sentinel.** Before the intent is written, the launcher
writes `STATECRAFT-TRIAL-SENTINEL` into the attempt's workspace: one line
carrying a fresh nonce. The prompt asks the session to read that file once
with the `Read` tool and to reply with its contents. `Read` changes nothing,
and the workspace is the run's own disposable one. A tool result for a `Read`
request that carries the nonce, with no non-execution note from the harness,
is the evidence that the tool executed; the nonce exists nowhere else, so a
reply cannot carry it without the read. This is the provider's report of the
execution, not a disk observation, and the trial says so.

**Rule 32: the budget is one session and one version probe, and it is spent
once.** The run path's own version probe is the only probe. The trial verb
refuses when the run `statecraft-startup-trial` holds any attempt, live or
concluded, so an uncertain launch is never replayed and a completed trial is
never repeated: a second trial needs a fresh project. The supervisor enforces
the deadline and stops the process group at it; the acceptance script bounds
the whole verb separately. There is no retry of any kind.

**Rule 33: provider execution is a stated act.** The verb refuses unless the
operator names what it is doing: `--provider-session`, which runs the provider,
or `--synthetic`, which states that the executable is a local fake, marks every
trial record synthetic, and can never be read as a provider observation. The
acceptance script's `managed-startup` stage refuses unless
`APPROVED_MANAGED_STARTUP_SESSION=yes`. That approval is not
`APPROVED_PROVIDER_SESSION`, neither satisfies the other, and the local test
route refuses when either is set.

**Rule 34: what the trial keeps.** `trial.json`, written once in the attempt's
directory beside section 3.32's four records, carries: the origin
(`provider-session` or `synthetic`); the sentinel's path, nonce and digest; the
settings bytes the adapter wrote, verbatim; the provider version the probe and
the init event reported; a timeline of the stream by line (each `SessionStart`
`hook_started` and `hook_response`, whether the response carried an
acknowledgment line, the init event, each tool request by id and name, each
tool result by id with whether it executed and whether it carried the nonce,
and the terminal event); the line and the event kind the decision was made at;
the gate's log; the process end (exit or signal, whether the supervisor stopped
it, and surviving processes); and the judgement of rules 35 to 37. Nothing is
removed afterwards. After writing, the verb reads the attempt's records back
from disk and judges them again; a reloaded judgement that differs from the
one written is a failure, not a result. `startup show` renders the trial's
section for the trial's run.

**Rule 35: the hook evidence has a name for each shape.**

| Hook evidence | When |
|---|---|
| `correlated` | exactly the acknowledgment rule 27 describes, read before the decision point |
| `absent` | no `SessionStart` response anywhere in the stream |
| `unbound` | a `SessionStart` response before the decision point, whose acknowledgment did not bind; the kind is section 3.31 rule 18's word |
| `late` | acknowledgments only after the decision point, which rule 26 never waits for |
| `conflicting` | acknowledgments naming different revisions |
| `mismatched` | correlated, naming a revision other than the required one |

**Rule 36: effects before admission get one of three words, never more.**

| Word | When |
|---|---|
| `excluded` | the decision was `admitted`, every tool request in the stream has a gate consultation, every consultation released the call only on `admitted`, and every executed tool result follows the decision line |
| `demonstrated-possible` | a tool executed with no consultation to account for it, or executed before the decision line, or executed while the decision refused, or after the gate withheld or refused it |
| `unobserved` | neither: nothing executed, or the stream is incomplete, so the records cannot say |

Each word is scoped to tool calls. None covers the provider's own startup,
other hooks, or anything outside a tool call, and the record says so beside the
word (rule 26).

*Amended by section 3.37 rule 3:* under confinement the gate consultations these rules read are child-attested; a trial recorded before section 3.37 is read as it was.

**Rule 37: the verdict.**

| Verdict | When |
|---|---|
| `established` | hook evidence `correlated`, the decision `admitted`, at least one tool request, every request consulted, the sentinel executed carrying its nonce after the decision line, effects `excluded`, and a process that ended by itself with no survivor |
| `not-established` | a completed launch in which any of those fails; every failing condition is a named reason |
| `uncertain` | a launch state of `launch-unknown`, `outcome-unknown`, `spawn-failed` or `interrupted`, or a process the deadline stopped |

A synthetic trial reaches these words with its origin beside them, and a
synthetic `established` is a statement about the procedure, never about a
provider.

**What an established trial does not establish.** Which process printed the
acknowledgment (rule 27); that the hook's bytes when it ran equal the bytes
digested; that a refused decision blocks a tool call, since an admitted trial
never refuses and a refusal would be a second session; anything about another
version; anything about the deny floor, which is section 3.30's question; or
qualification.

**The ordering the gate implements, and what it assumes.** The gate opens
exactly when `admission.json` exists holding `"decision":"admitted"`, and that
file is written when the launcher reads the first stream event that is not a
`SessionStart` `hook_started` or `hook_response` (the 2026-09-22 entry's fourth
choice). A tool call requested before that event reaches a gate that polls for
the file every tenth of a second for thirty seconds: an admitted decision
releases it, a refusal refuses it with the provider's blocking code `2`, and no
decision within the wait refuses it the same way and logs `withheld`. The gate
never releases on a timeout. Five provider behaviors are premises, and the
trial observes each or shows that it failed:

- **P1.** A `SessionStart` hook supplied through `--settings` runs in
  `--print` mode and is reported as a `hook_response` before `init`. Recorded
  only for a globally registered hook (the committed 2.1.267 streams); through
  `--settings`, unobserved. If it arrives after `init`, it is `late`.
- **P2.** That hook receives the session's environment, which carries the
  binding. Unobserved. If not, the evidence is `unbound`.
- **P3.** A `PreToolUse` hook supplied through `--settings` runs before the
  tool executes, the provider waits for it up to the registration's
  `timeout` (sixty seconds, which is also the documented default), and exit `2`
  blocks the call. Documented; unobserved here. If the provider does not wait,
  or runs the tool anyway, the effects word is `demonstrated-possible`.
- **P4.** `Read` needs no approval inside the working directory in the
  default permission mode. Documented. If it is denied, the sentinel does not
  execute and the verdict names that.
- **P5.** No event other than a `SessionStart` hook event precedes `init`. If
  one does, the decision is made there with no init session read, the
  acknowledgment is judged `wrong-session`, and the attempt is refused. That is
  fail-closed, and the trial records the event kind the decision was made at
  so the cause is visible rather than inferred.

None of these is weakened to make the trial pass: not the gate's wait, not the
decision point, not the refusal on a missing session.

### 3.34 One allowlisted event after the terminal event

A narrowly scoped authority amendment, settled by the owner on 2026-09-23 and
recorded before the implementation it authorizes. It amends section 3.30 rule 8
in one clause, "the terminal event is its last event", and nothing else in
sections 3.29 and 3.30. Rules 1 to 7 and 9 to 12 are unchanged and govern
everything below.

**The gap this closes, exactly.** The one authorized permission experiment
(section 5, the 2026-09-23 entry on the two live experiments) stopped at its
first session because Claude Code `2.1.267` wrote a `system` event of subtype
`task_summary` after its `result` event, and rule 8 refuses any event after the
terminal one. The measured trailer, event 12 of the refusal control's capture
(stream `cc9b6e59…ea11`), is exactly:

```json
{"type":"system","subtype":"task_summary","detail":null,"uuid":"…","session_id":"…"}
```

with the capture's own session id. The same capture carries a second
`task_summary`, before the terminal event, whose `detail` is a string. The
recorded streams under `crates/statecraft-adapter-claude-code/testdata/stream/`
carry neither. Nothing in this amendment treats a `system` event as harmless
because of its type, and nothing treats a summary as proof that no further work
happened: the trailer is admitted as an event that carries nothing the
admission reads, and refused whenever it could carry more.

**Rule 13: at most one trailer, and only this one.** A capture may carry, after
its terminal event, at most one further event, the **trailer**, and only when
every condition below holds. Otherwise rule 8 applies as written and the
capture is out of order.

1. **A complete terminal event precedes it.** The capture already holds exactly
   one init event and exactly one terminal event, in that order, and the
   terminal event deserializes as the adapter's result type. A trailer never
   completes a capture that has no terminal event, and a trailer before the
   terminal event is not a trailer (see "Before the terminal event").
2. **It is the last event.** Nothing follows the trailer. A second trailer, a
   second terminal event, or any other event after it refuses the capture.
3. **Its shape is closed.** Its line is one JSON object whose members are
   exactly `type`, `subtype`, `detail`, `uuid` and `session_id`: none missing,
   none added. `type` is `system`. `subtype` is on the closed list, which has
   one entry, `task_summary`, and whose provenance is the capture named above.
   `detail` is JSON `null` or a string. `uuid` is a non-empty string.
   `session_id` is a non-empty string.
4. **It names the capture's session.** `session_id` equals the session every
   other event in the capture names. A trailer naming another session is
   mixed-session evidence under rule 8; one naming none, or an empty one, is
   not the closed shape of condition 3 and is refused as out of order.
5. **The shape is read by the adapter.** Rule 7 applies: the closed shape is a
   type in the supported adapter's crate (spec `004` section 3.9), and this
   spec's crate does not parse the line a second way.

Anything else after the terminal event refuses the capture, including, for
avoidance of doubt: an `assistant` or `user` turn; a tool request or tool
result; a `permission_denied` or hook event; a `rate_limit_event` or any event
of a type the adapter does not map; a `system` event of any other subtype; a
`task_summary` with any additional member such as a tool-use id; and a second
`result`.

**Rule 14: the trailer is not evidence.** No field of the trailer is read for
any decision: not the classification of a tool use (rule 9), not a control's
outcome (rule 10), not the terminal or process judgement (rule 11), not the
version or the binding (rules 4 and 12). `detail` is never read, so no prose in
it can qualify, refuse, supply a denial, supply a result, or stand in for a
control. The testable form of this rule: **the admission's judgement of a
capture that carries an admitted trailer is identical to its judgement of the
same capture with the trailer's line removed.** A trailer never turns a
refused capture into an admitted one or the reverse.

**Before the terminal event.** Unchanged. A `task_summary` before the terminal
event is a `system` event naming the session, counted for rule 8's session
check and carrying nothing read, as every `system` event of a subtype the
admission does not name already is. A `rate_limit_event` before the terminal
event is an event of a type the adapter does not map, which spec `004` section
3.1 classifies as progress; its position already counts. This amendment
accounts for both deliberately, and it admits neither after the terminal event
except the one trailer rule 13 describes.

**Bounds.** The trailer is read in the same single pass over the capture that
rule 8 already makes, and the admission reports the trailers it admitted from
that pass rather than reading a capture again. The line the trailer is read
from is the line the adapter read the event from: a capture whose events and
non-blank lines do not correspond one to one is unreadable. The product sets no byte limit on a capture today and
this amendment adds none; the launch's deadline (section 3.30, "Bounds") bounds
what a session can write, and rule 13 bounds what may follow the terminal event
to one line of a closed shape.

**Bytes and order are preserved.** The capture's bytes are kept whole,
trailer included, in the order the provider wrote them. Nothing strips,
rewrites or reorders the trailer before the admission reads it or after.

**Where it applies.** Everywhere rule 8 applies through the admission's reading
of a capture: the permission experiment's admission, and `startup capture`'s
reading of whether a launch completed as one session. It does not change the
managed-startup trial (section 3.33) or a run's startup records (sections 3.31
and 3.32), which do not read a capture through rule 8.

**Historical records.** The refusal control's record from the 2026-09-23
experiment keeps its verdict, `incomplete` under rule 8 as then written, and its
archive is not rewritten. Replaying its bytes through the amended reading is
**offline regression evidence**: it is labeled a replay, it is not a live
observation, and it neither counts toward nor spends any provider session. A
record read back later is re-judged under the rules in force (section 3.29
rule 5), and that single launch record cannot qualify anything, because an
admitted observation needs all three controls.

**The command surface is unchanged.** No verb, flag or exit code is added.
`startup capture` and `startup qualify` report the trailer when one was
admitted, as one line naming its event number and subtype and saying it was not
read.

**Acceptance.** Positive: the measured trailer shape, with `detail: null` and
with a string `detail`, after a complete terminal event, admitted; the
trailer-removed identity of rule 14, over both an admitted and a refused
capture; and the archived capture's bytes, replayed offline, reading as one
complete session with its trailer reported. Negative, each refusing the
capture: a second trailer; any other `system` subtype after the terminal
event; a `rate_limit_event` after it; an assistant or user turn after it; a
second terminal event after the trailer; a trailer with an added member, a
missing member, a non-string non-null `detail`, an empty `uuid`, or a
`session_id` that is absent or names another session; and a trailer with no
terminal event before it.

### 3.37 Where an attempt's launch records live, and what the child is given

An authority amendment, settled by the owner on 2026-09-23 with spec `004`
section 3.18 and recorded before the implementation it authorizes. Sections
3.31 and 3.32 put an attempt's launch directory at
`.statecraft/state/startup/runs/<run>/<attempt>/` inside the target, beside the
workspace the child works in, and put the gate's script and log in the same
directory as the intent, the launch record, the admission decision and the
record. A child can therefore read and rewrite the files its own startup
judgement is recomputed from, and can write the admission decision before the
supervisor does. Constitution IX does not admit that. Section 3.36's accounting
covers this section's requirements as it covers those of sections 3.1 to 3.35.

**Rule 1: the launch records move to the product home.** An attempt's
`intent.json`, `launched.json`, `admission.json`, `record.json`, and a trial's
`trial.json`, live in the product home under the repository's records, one
directory per run and attempt, keyed as the run record is keyed. Their names,
contents, write-once rules and judgement are unchanged; only the directory
moves. The child can neither read nor write them (spec `004` section 3.18 rule
2).

**Rule 2: the exchange directory.** The files the child is given live in a
separate per-attempt exchange directory in the product home (spec `004` section
3.18 rule 5): the admission gate script, the settings document the provider is
given, a copy of the admission decision, and the gate log, created empty before
launch. The gate reads the decision and writes its log there. The supervisor
makes `admission.json` durable in the launch records first and then writes the
exchange copy by creating a new file and renaming it into place; the launch
records' copy is the evidence of what was admitted, and the exchange copy is
only what the gate reads. The settings document's digest is taken before launch
and again when the record is written, as before, and a difference is recorded
as before. The gate log is **child-attested**: the supervisor copies it into the
launch records when it writes `record.json`, bounded in size, read without
following a link, and labelled as written by a process inside the confinement.

**Rule 3: what rests on the gate log.** Section 3.32 rule 26's gate withholds
every tool call until the decision admits; that withholding is enforced by the
gate reading a file the child cannot write, and is unchanged. What the log
reports afterwards (each consultation, and section 3.33's `established` and
`excluded`, which rest on every tool request having a gate consultation) is
child-attested evidence under spec `004` section 3.18's confinement. A trial
recorded before this section, including the one established observation of
section 5's 2026-09-23 entry, was recorded without confinement and is read as it
was; a trial recorded after it states that its consultations are
child-attested. Section 3.36 rule 5 relies on an established trial and inherits
that statement.

**Rule 4: `startup trial` and `startup capture` are confined too.** Both start
a provider and refuse, as a preflight with nothing launched, when the boundary
cannot be established (spec `004` section 3.18 rule 9). A capture writes the
settings file it hands the provider into that capture's exchange directory, not
into the directory the operator names; the operator's directory receives the
capture records after the provider has exited, and a directory inside the
project stays refused, as before. Section 3.29 rule 4 and section 3.30 rule 12
bind the provider's program and arguments as handed to the confinement; the
confinement is bound apart from them by its mechanism and its profile or
ruleset digest, and the wrapper's own arguments are not part of the invocation
(spec `004` section 3.18 rule 10). A confined observation and an unconfined one
are not the same invocation, and no earlier observation is re-judged.

**Rule 5: records written before this section.** Launch records already in a
target's `.statecraft/state/startup/runs/` are read where they are, judged as
before, and labelled as written where the child could reach them. They are not
moved, rewritten or re-judged as protected, and no attempt that wrote them is
reported as confined. A confined child cannot write that tree (spec `004`
section 3.18 rule 2), so no record can be added to it after this section.


## 4. Out of scope

- Installing or upgrading the harness, owned by spec 008.
- Generic provider protocol and process supervision, owned by spec 004.
- Acceptance policy over admitted evidence, owned by spec 005.
- Repository initialization and ownership transfer, retained by spec 002.

## 5. Resolved decisions

**2026-09-26: runtime evidence is not installation state.** Admission binds records to one launch and one harness identity. The relocation preserves the previous requirements verbatim and creates no new live-provider claim.

## Verification

Each line is one command.

```verify:cli
cargo test -p statecraft-home --lib capture
cargo test -p statecraft-cli --test qualification_workflow
cargo test -p statecraft-cli --test acceptance_script
cargo test -p statecraft-cli --test native_stream
cargo test -p statecraft-home --lib launch
cargo test -p statecraft-cli --test run_startup
cargo test -p statecraft-home --lib trial
cargo test -p statecraft-cli --test startup_trial
sh -n scripts/acceptance/managed-session.sh
```
