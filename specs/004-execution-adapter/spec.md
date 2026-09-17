---
id: "004-execution-adapter"
title: "The execution adapter boundary: one protocol, declared capabilities, a qualification suite, and a constructed child environment"
status: approved
implementation: complete
created: "2026-09-16"
summary: >
  The single seam through which this product runs an agent, and the only place a
  provider is named. Fixes the three-part protocol (a request the supervisor
  writes, an event stream it reads, a result it records), the closed capability
  vocabulary with required tokens refusing before any process starts and
  preferred tokens degrading visibly, the negative suite that qualifies an
  adapter binary version, and the constructed rather than filtered child
  environment whose property is a complete record and explicitly not
  containment of hostile code.
establishes:
  - { kind: directory, path: "crates/statecraft-adapter/" }
depends_on:
  - "000-bootstrap"
  - "001-boundaries-and-authority"
  - "003-work-and-run-semantics"
---

# 004: The execution adapter boundary

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

## 2. Territory

`crates/statecraft-adapter/` (forward claim; unresolved until implemented).
Provider-specific adapters are separate compilation units claimed by their own
later specs; a provider name appearing in this territory is a defect.

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
   assigns the fix to this product.)

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
  applied, which this spec does not apply.
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

## 4. Out of scope

Any specific provider's adapter; breadth across many providers; operating-system
sandbox profiles; adaptive autonomy or trust scoring; cost ceilings and quota
parking; and model selection policy. Each is deferred by name in the decision
record.

## 5. Decisions recorded during implementation

Dated entries for choices §3 was silent on. None changes what it requires.

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

## Verification

Each line is one command. §3.5's suite is eight tests named `suite_1` to
`suite_8`, and §3.8's remaining rows are tests beside them. They run against the
fixture adapter, so the table runs with no real provider installed, which is
what §3.5 requires of it.

```verify:cli
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
spec-spine index coverage --fail-on-untraced
cargo test -p statecraft-adapter --test no_provider_names
cargo test -p statecraft-adapter --test negative_suite
```
