---
id: "001-boundaries-and-authority"
title: "The product boundary, the component owners, and the separation of authority"
status: approved
# Records decisions and owns prose. There is no code behind it and never
# will be, so `n-a` keeps it out of the ready set (spec-spine 045).
implementation: n-a
created: "2026-09-16"
summary: >
  What this product is, what it is not, and who owns each capability it
  depends on. Fixes the claim vocabulary (specified / implemented / tested /
  released), the two interaction modes and which one is built first, the rule
  that separates an authority change from an implementation change, and the
  reuse disposition for every shared component this repository would otherwise
  reimplement. Owns the product half of the constitution and the two founding
  records under docs/. It owns no code.
establishes:
  - { kind: section, file: "standards/spec/constitution.md", anchor: "vi-a-declaration-of-completion-is-not-an-acceptance" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "vii-a-candidate-cannot-enlarge-its-own-authority" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "viii-name-the-enforcement-or-do-not-claim-the-protection" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "ix-outcomes-are-recorded-outside-the-child-and-recovery-reconciles-before-it-repeats" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "x-intent-execution-verification-and-acceptance-are-separate-records" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "xi-a-capability-is-declared-and-qualified-never-assumed" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "xii-public-claims-are-graded" }
  - { kind: section, file: "standards/spec/constitution.md", anchor: "xiii-the-local-product-owes-nothing-to-a-hosted-one" }
  - "docs/decisions/00-founding-decisions.md"
  - "scripts/check-authored-content.sh"
depends_on:
  - "000-bootstrap"
---

# 001: The product boundary, the component owners, and the separation of authority

## 1. Purpose

This repository is a local environment for **governed agent work**: it prepares a
repository, selects a unit of work, supervises an agent through it in an isolated
workspace, judges the result independently, and retains a reviewable account of
what happened.

Four things go wrong without a written boundary, and all four went wrong in the
archived predecessor before its specs 119 to 129 corrected them: the product
grows a second spec compiler, it grows a second memory store, it accepts an
agent's own word for its outcome, and it lets the thing being judged edit the
rules it is judged by. This spec fixes the boundary so those are refusals rather
than regressions.

It owns no code. It owns the product half of the constitution and the two
founding records under `docs/`.

## 2. Territory

Constitution principles VI to XIII; `docs/decisions/00-founding-decisions.md`;
sections 3.8 to 3.12;
`scripts/check-authored-content.sh`.

Not this spec's territory: the corpus contract (`000`), the environment
lifecycle (`002`), run semantics (`003`), the execution adapter (`004`),
acceptance and evidence (`005`).

## 3. Behavior

### 3.1 What this product owns

Repository registration and onboarding; the managed working environment inside a
registered repository; workspace preparation; supervision of an execution
adapter; run state and its recovery; independent acceptance; and the reviewable
outcome. It works on one machine, on one repository, with no account.

### 3.2 What this product does not own, and who does

| Capability | Owner | This product's relationship |
|---|---|---|
| Specification semantics, compilation, ownership analysis, freshness, verification contracts | spec-spine | Consumer of its supported commands and **its structured reports**. Never a second compiler, and never an ad-hoc read of the derived tree (`.statecraft/derived/`, formerly `.derived/`). |
| Application knowledge, recall, coordination semantics | aicortex | No dependency in the first slice. `005` places the interface out of scope; the narrow, optional, one-way boundary it would be is described in section 3.12, and implemented by neither side. |
| Service chassis, identity, persistent cell enforcement | Rahi | Not a required local daemon, not a process sandbox, not a desktop framework. A future hosted backend consumes explicit contracts from this product. |
| Ordered pure checks and decision composition | action-gate | Adapted at the boundary: this product supplies the required checks and the deny-by-default ceiling, because the library's own fallthrough is allow. |
| Hash-linked records, signing, verification | attest-ledger | Reused for the record envelope. Durability, crash recovery, and independently supplied issuer trust stay here. |
| Canonical JSON serialization | canonical-keysort-json | Reused. Key sorting is not strict portable-input validation; see `005`. |
| Outcome scoring | trust-window | Deferred, with no consumer. A score never overrides authorization. |
| Scoped temporal facts | fact-fold | Not this product's concern. |
| Legacy factory certificates | tenant-emit, tenant-tail | Assessed, not adopted. |

A shared name is not a shared semantic. Before any new shared mechanism is
designed here, its disposition is recorded in
section 3.8 as one of: **reuse**, **extend the
owner**, **adapt at the boundary**, **recover from archive**, or **new, with a
stated mismatch**.

### 3.3 The claim vocabulary

Four grades, stated separately, never inferred from one another:

- **specified**: an approved spec describes it.
- **implemented**: code exists that a reader can run.
- **tested**: a named check exercises it, including its negative cases.
- **released**: a versioned artifact a third party can install.

Observable rule: a document in this repository that calls a behavior
implemented, tested or released names the evidence in the same sentence. A spec
being `approved` grants no grade above *specified*.

Where this repository stands, restated whenever it changes rather than left to
age:

| Date | Grade |
|---|---|
| 2026-09-16 | Specified only, and not fully: no spec but `000` was approved, and no code existed. |
| 2026-09-16 | `000` to `005` specified. `002` additionally **implemented and tested within its own territory**, the evidence being `crates/statecraft-environment/`, 59 tests, and one integration test per row of `002` section 3.10 named after the row it covers. Nothing released; `F-02` defers publication. |
| 2026-09-16 | `000` to `006` specified. `002` to `005` additionally **implemented and tested within their own territories**: four crates, 221 tests, and one integration test per row of each spec's observable-negative-cases table, named after the row it covers. `006` is ratified and not yet implemented, so the product is still not runnable. Nothing released. |
| 2026-09-17 | `000` to `007` specified and approved. `002` to `007` additionally **implemented and tested within their own territories**: six crates, 320 tests, and 73 of 73 source files specifically claimed. For `002` to `006` the evidence is one integration test per row of each spec's observable-negative-cases table, named after the row it covers; `007` has no such table, and its acceptance is the compatibility suite of its section 3.4, which runs from two crates of this workspace, `statecraft-envelope` and `statecraft-acceptance`, and fails on each independently. That is a check within one implementation and not parity between two, so it claims nothing about a second reader. `006` is now implemented, so the product **is runnable as `statecraft-cli`**; what its verbs do is still bounded by the territories above. Nothing released; `F-02` defers publication and `crates/statecraft-envelope/` stays `publish = false`. |
| 2026-09-21 | Seven specs, `000` to `006`, all specified and approved, after four pairs were consolidated into the spec that held each subject first. `002` to `006` additionally **implemented and tested**: eight crates, 683 tests, 117 of 117 source files specifically claimed, and 27 verbs bound in the binary. Measured with `make status`, `cargo test --workspace`, `spec-spine index coverage` and `statecraft-cli --help` on 2026-09-21. The consolidation changed no requirement and merged no crate, so no grade moves because of it. Nothing released; `F-02` defers publication and `crates/statecraft-envelope/` stays `publish = false`. |
| 2026-09-23 | Seven specs, all approved. `002` is `in-progress`, not implemented: its local obligations are implemented and tested, and of the two live questions its section 3.33 separates, the managed-startup trial is `established` once and the permission experiment has no admitted result (`002` section 5, 2026-09-23). `003` to `006` implemented and tested; the workspace suite and `index coverage` are measured on each merge rather than restated here. Nothing released; `F-02` defers publication. |

Each row is narrower than "the spec is implemented". The commands `002` to `005`
name are not bound to a process by those crates, so the grade they claim is
*implemented for their own territory* and nothing about a command line. `006` is
the binary that changes that, and **until `006` is implemented no document here
may call this product runnable.**

### 3.4 The two interaction modes

They are different products and are not conflated:

- **Mode A, the agent invokes the CLI.** An agent already running in a session
  calls this product's verbs to register a repository, read ready work, prepare
  a workspace, or request an independent acceptance. The product is a tool the
  agent uses. It observes only what the agent chooses to tell it, so a refusal
  the agent does not report is invisible, and nothing in this mode can satisfy
  constitution IX.
- **Mode B, the CLI supervises the agent.** This product launches the agent
  process, holds the event stream, counts refusals itself, owns the child's
  environment, and decides the outcome. Constitution VI, VII, VIII, IX and XI
  are only enforceable here, because only here is there a supervisor.

**Recommendation: build Mode B first**, and expose Mode A as the same verbs over
the same structured output once they exist. Mode A built first produces a tool
that cannot keep its own constitution, and the archived predecessor's specs 119
and 129 are the record of discovering that after the fact. Mode A is not
abandoned: every verb `003` and `005` define emits machine-readable output, so
Mode A follows without a second surface.

This ordering is a **proposed decision** (`D-05`), not an adopted one.

### 3.5 An authority change is not an implementation change

The **authority set** of a registered repository is: its policy, **including the
lifecycle policy `003` section 3.1.1 reads from it**, its check suite, its
verifier, its hooks, the acceptance instructions the product reads, and the
environment manifest that says which files this product manages.

Membership here is a question about *this* product's trust boundary. It is not
the same question as how a base's rules would classify a change to one of these
paths, and `005` section 3.3 fixes which of the two answers each member takes.

Three observable rules:

1. Every member of the authority set is read at the **trusted base revision** of
   a run, never from the candidate. `005` fixes how the base is identified.
2. A candidate whose diff touches the authority set is reported as an
   **authority change**. It is not accepted on the strength of the check suite
   it proposes; it requires a human decision recorded outside the candidate.
3. A run never widens its own permissions. There is no flag whose only effect is
   to remove a guard, and the product ships no default that bypasses permission
   enforcement.

### 3.6 Authored-content rules that hold repository-wide

These are mechanical and checked by `scripts/check-authored-content.sh`:

1. No authored file contains U+2014. Use a colon, semicolon, comma, parentheses
   or two sentences. U+2013 is permitted only for numeric or section ranges.
2. No file in this repository, and no commit message, pull-request body, issue,
   review or release note, contains an agent-session URL or a session-tracking
   trailer, and none substitutes another tracking link.
3. `LICENSE` and any `NOTICE` are preserved. Recovering behavior from an
   archived component does not relicense it: section 3.9
   records each source's license, and an AGPL source's *behavior and fixtures*
   may be reimplemented from a written description, while its code may not be
   copied into this Apache-2.0 tree.

### 3.7 Observable negative cases

| Case | Required behavior |
|---|---|
| A document calls a behavior `implemented` with no named evidence | Refused by review; 3.3 is the rule it violates. |
| An authored file contains U+2014 | `scripts/check-authored-content.sh` exits non-zero and names the file and line. |
| An authored file contains an agent-session URL | Same check, same exit, named separately from the U+2014 finding. |
| A design proposes a new shared mechanism with no disposition row | Refused: 3.2 requires the row, including the mismatch that justifies `new`. |
| A candidate's diff touches the authority set and the run accepts it on its own suite | Violates 3.5.2; `005` is where the mechanism lives. |
| A verb is added in Mode A that has no Mode B equivalent | Refused: 3.4 makes Mode B the supervisor, and a Mode-A-only verb has no supervisor to record its outcome. |

### 3.8 The reuse dispositions, the archive, the slice, and the later interfaces

Sections 3.8 to 3.12 are the design record section 3.2 requires, folded in from
`docs/design/00-boundaries-and-reuse.md`, which was prepared 2026-09-16 and is
deleted by the change that folds it. Nothing in it is revised on the way in: a
disposition is a decision and moving one is the owner's act, not a consequence
of a build or of a file moving.

Two of its dated notes are kept as the dated statements they are. As prepared it
read "nothing here is implemented, and no dependency declared below exists in
any manifest, because no manifest exists", which described 2026-09-16 and stopped
being true. Corrected 2026-09-19: a Cargo workspace exists, and one row below has
moved from a proposal to a declared dependency, `attest-ledger-core`, pinned to
`a9c3595` in `crates/statecraft-run/Cargo.toml` and used by `src/record.rs`. The
distinction the tables keep apart is unchanged: a dependency this product would
take on a component that is implemented today, against an interface this product
proposes and neither side has built.

#### Component owners, and what this product's relationship actually is

| Component | State today | This product's relationship | Kind |
|---|---|---|---|
| spec-spine 0.23.0 (CLI; `=0.20.0` until 2026-09-23) | Implemented, released, installed locally at `.tooling/bin` | Invokes its supported commands, parses its structured reports | **actual dependency**, on a released binary |
| `spec-spine-core` 0.23.0 | Implemented, released on crates.io | `scaffold_init_json`, the governance starter set `002` section 3.15 consumes | **actual dependency**, a library pinned `=0.23.0` in `crates/statecraft-home` since 2026-09-23 (`=0.21.0` before, non-conforming), moved independently of the CLI pin; conforming (`002` section 5) |
| `attest-ledger` 0.1.0 | Implemented, Apache-2.0 | Record envelope, chain hashing, verification | **actual dependency** as of 2026-09-19: `attest-ledger-core`, pinned to `a9c3595` in `crates/statecraft-run`. The disposition it was adopted under is the **reuse** row below. |
| `canonical-keysort-json` 0.1.0 | Implemented, Apache-2.0, Rust only | Canonical serialization at the hashing boundary | **proposed reuse** |
| `action-gate` 0.1.0 | Implemented, Apache-2.0 | Check composition only, with required checks and the deny ceiling supplied here | **proposed adaptation at the boundary** |
| Rahi | Implemented locally, not released | None. Not a local daemon, not a sandbox, not a UI framework | **no dependency**; a future hosted backend consumes contracts this product publishes |
| aicortex | Design plus a local bootstrap; no coordination runtime | None in the first slice | **proposed interface**, built by neither side |
| `trust-window` 0.1.0 | Implemented, Apache-2.0 | None | **deferred**, no consumer (F-05) |
| `fact-fold` | Implemented, TypeScript, private | None | **not this product's concern** |
| `tenant-emit`, `tenant-tail` | Implemented, released, tied to the retired factory model | None | **assessed, not adopted** (F-11) |
| Chancery `kernel-addon` | Implemented Rust kernel, Apache-2.0, product parked | None | **donor of lessons only** |
| `governance-native` `portable.rs` | Implemented, **AGPL-3.0** | Behavior and fixtures may be reimplemented from description; code may not be copied here | **recover behavior from archive**, not code |
| Archived predecessor CLI | 76 specs, partly implemented, history preserved | Source of measured failures, contracts and fixtures | **recover from archive**, selectively |

#### The dispositions, stated as section 3.2 requires

Each row below is the record that mechanism needs before anything is designed
here. A shared name is not evidence of a shared semantic, and a mismatch is
stated rather than implied.

| Mechanism | Disposition | Reason, and the concrete mismatch where one exists |
|---|---|---|
| Hash-linked record envelope | **reuse** `attest-ledger` | It owns the construction. Durability (fsync before acknowledge), an O(1) append, crash-torn-tail recovery and independently supplied issuer trust are **not** in it and stay here. Its anchor verifier trusts the key the anchor carries (C-08), which cannot satisfy spec 005 section 3.6, so trust roots are supplied by this product. |
| Canonical JSON bytes | **reuse** `canonical-keysort-json` | It exists to stop `serde_json`'s `preserve_order` from silently changing the bytes a hash is computed over. |
| Strict portable-input validation | **recover behavior from archive**, new code | `canonical-keysort-json` sorts keys; it does not reject duplicate keys, non-integer numeric tokens, out-of-range integers or invalid encoding. `governance-native`'s `portable.rs` does, including rejecting `-0`, and is AGPL-3.0 (C-09). Recover the described behavior and the golden cases; write the code here. |
| Ordered check composition | **adapt at the boundary** `action-gate` | Its evaluator returns the first decision and otherwise **allows**, including over an empty registry (C-07). A deny-by-default acceptance cannot be built by trusting that fallthrough, so this product supplies the required-check set and the permission ceiling, and enforcement stays outside the library. |
| Run state, supervision, outcomes, recovery | **new** | No owner exists. `action-gate` composes pure checks and enforces no effect; Rahi enforces cells, not local child processes. |
| Execution adapter protocol and capability tokens | **new**, shaped by the archive | The predecessor's specs 043, 114, 116, 120 and 124 reached this shape; the code is TypeScript and Rust in a retired tree. Recover the protocol shape and the negative table; write the code here. |
| Acceptance receipt | **new**, semantics inherited | The four dimensions and the byte-reference rule are inherited (C-13, D-07). The receipt binding repository, base, candidate, suite, exit codes and policy digest is this product's. |
| Legacy certificate emission and verification | **assessed, not adopted** | `tenant-emit` and `tenant-tail` are real and independently usable, and their inputs are the retired factory's run-directory and stage layout (C-14). Forcing this product's outcome into that layout to reuse a package would shape the product around a retired factory. Reopened by a mapping from a real outcome, not by their availability. |
| Knowledge, recall, temporal facts | **not ours** | aicortex owns application knowledge; `fact-fold` is a candidate for its graph storage. Building either here would be the second generic memory store I-08 refuses. |
| Autonomy scoring | **deferred** | `trust-window` is available and has no consumer here. Introducing adaptive autonomy in a first slice to justify a dependency is the wrong order. |

### 3.9 What the archive is used for, and what it is not

The predecessor is evidence, not a template. Three specific things are worth
recovering, and were read for this design:

1. **Measured failures** (C-11). A foreign-origin browser POST reached a
   loopback API with no `Origin` or `Host` check and disarmed a fixture project.
   A fabricated token in the supervisor's environment reached the gate suite,
   repository hooks and the publishing path. A driven session could publish
   around the supervisor through a keyring-authenticated `gh` or plain SSH.
   These are why constitution VIII and IX are worded as enforcement rather than
   intent, and why spec 004 constructs the child environment instead of filtering
   it.
2. **Contracts that were paid for.** The adapter seam's three parts, the closed
   capability vocabulary, the negative conformance table, the five outcome names,
   the intent/outcome bracket, and the three names for absence. Each cost a spec
   and a correction in the predecessor.
3. **Ordering lessons.** The predecessor packaged members before the seam existed
   and performed the surgery afterwards (its spec 043 on its spec 042). It drove
   every session with permissions bypassed at one hardcoded call site until its
   spec 032 made posture explicit registry state. It journaled a null model for
   44 sessions before its spec 040 made the model a chosen, recorded fact. This
   corpus puts the seam, the posture and the record first.

What is **not** recovered: the hosted control-plane client and its verbs, the MCP
face, the multi-project scheduler and standby daemon, the web UI, corpus
synthesis and holdback validation, quota and cost machinery, and the export and
attestation bundle formats. Each is either deferred by name in the decision
record or belongs to a hosted decision that has not been made.

Licenses are preserved as found. An AGPL-3.0 source contributes a written
description of behavior and its golden cases to this Apache-2.0 tree, never its
code.

### 3.10 The two interaction modes, drawn out

```
Mode A: the agent invokes this product
  agent session ──calls──> statecraft verbs ──> reads spec-spine, prepares, reports
  The agent is the supervisor of itself. A refusal it does not report is invisible.
  Constitution IX cannot be satisfied: there is no second party holding the record.

Mode B: this product supervises the agent          <-- built first (D-05)
  operator ──> statecraft ──spawns──> adapter ──spawns──> agent process
                    │                    │
                    │<───event stream────┘   refusals counted HERE
                    └── run record, outcome, independent acceptance
  The supervisor owns the environment, the deadline, the event stream and the verdict.
```

Mode A is served afterwards by the same verbs: every verb specs 002 to 005
define emits machine-readable output, so an agent calling them is a consumer of
the Mode B surface rather than a second product. What Mode A can never supply is
a supervisor, which is why it is second and not first.

### 3.11 The bounded first vertical slice

One repository, one work item, one adapter, one isolated workspace, one
independent inspection, one reviewable outcome. **Publication is not part of it**
(F-02).

| Step | Verb (proposed) | Spec | What must be observably true |
|---|---|---|---|
| 1 | `statecraft project register <path>` | 002 | A qualification verdict with reasons is recorded. Nothing is written inside the target. A non-git path is `unqualified`; a corpus-less repository is `ungoverned`. |
| 2 | `statecraft env plan` then `env apply` | 002 | Managed bytes are written and recorded in a committed manifest with source and digest. A pre-existing user instruction file is left untouched and reported `foreign`. A second run is a no-op. |
| 3 | `statecraft work list` | 003 | The ready set comes from spec-spine's structured report, with the field each row came from named. A target whose corpus does not compile refuses, rather than reading the derived tree directly. |
| 4 | `statecraft run start --spec NNN` | 003, 004 | An isolated worktree is prepared from a recorded base commit; the operator's checkout is untouched. One adapter session runs under a constructed environment. Refusals are counted by the supervisor from the event stream. The attempt ends in exactly one of the five outcomes. |
| 5 | `statecraft accept --run <id>` | 005 | The suite runs from instructions read **at the base**, over the candidate sha. A receipt is minted only on a clean tree with an unmoved HEAD. A candidate touching the authority set is reported as an authority change and is not accepted on its own suite. |
| 6 | `statecraft run show <id>` | 005 | One account folded from the records, every value naming its record, the claim beside the independent result, each evidence dimension separately, and absence named as `none`, `not-recorded` or `stale`. |

**Verbs as bound, clarified 2026-09-19.** The column above is headed *proposed*
and stays as written: it is what was proposed on 2026-09-16, and the steps and
their observable requirements are unchanged. What the binary spells is not what
the proposal spelled, so the two are reconciled here rather than by editing the
table. Every verb takes the target path first and accepts `--json`.

| Step | Proposed | Bound today |
|---|---|---|
| 1 | `statecraft project register <path>` | `project register <path>`, unchanged. A consent step joined it: `project arm <path>`, which step 4 now requires. |
| 2 | `statecraft env plan` then `env apply` | `env plan <path>` then `env apply <path>` |
| 3 | `statecraft work list` | `work list <path>` |
| 4 | `statecraft run start --spec NNN` | `run <path> <spec-id>`. No `start` subverb and no `--spec` flag: the spec id is positional, and it is also the run id, because spec `003` section 3.4 makes a retry an appended attempt of the same run. `run` refuses (2) a registered target that is not armed. |
| 5 | `statecraft accept --run <id>` | `accept <path> <run-id>` |
| 6 | `statecraft run show <id>` | `run show <path> <run-id>` |

Spec `006` owns the surface and spec `009`, since folded into `006`, bound steps 3 to 6. Nothing in the
right-hand column revises what the step must make observably true, and the
acceptance below is untouched.

#### The slice's acceptance, stated as refusals

The slice is done when these hold, each demonstrable rather than asserted:

1. An adapter that reports success while the suite fails produces **no receipt**,
   and the report is retained in a field named for a claim.
2. An adapter that emits refusal events and exits **zero** produces an attempt
   whose outcome is `refused`.
3. A required capability the adapter's manifest lacks refuses **before any
   process is spawned**.
4. A candidate whose diff touches the authority set is reported as an authority
   change even when its own suite passes.
5. Killing the supervisor mid-attempt leaves an intent with no outcome; the next
   start reconciles it and reports `unknown` where it cannot tell, and does not
   retry that effect.
6. `env remove` removes every matching managed byte, leaves a drifted one with a
   report, and leaves no other byte changed.
7. Every signature reads `unsigned` and every issuer `unknown`, because nothing is
   signed, and the outcome says so rather than omitting the fields.

#### What the slice deliberately does not prove

That the environment is safe against hostile code. Spec 004 section 3.6 names
three residuals it does not close, and closing them needs an operating-system
mechanism deferred as F-09. The slice proves the record is **complete** through
the supervisor's path, which is a different and smaller claim.

### 3.12 Interfaces this product would publish later

Named so a future consumer has something to consume, and implemented by nothing.

- **To a hosted backend.** The run record's envelope, the receipt, and the four
  evidence dimensions with admission kept apart. A hosted service would admit or
  refuse a receipt under its own policy, supplying its own trust roots. It never
  becomes a precondition for the local product (constitution XIII).
- **To aicortex.** One-way, least-privilege, journal-derived: an outcome summary
  a publisher could hand over after a run closed. It carries no credential, no
  grant and no live claim, and repository files stay readable without it.
- **To an independent verifier.** The preserved evidence bytes, their typed
  digests with named constructions, and a report that separates integrity,
  signature, issuer trust and subject binding. A verifier never executes what the
  evidence carries and never takes the evidence's own anchor as a trust root.

## 4. Out of scope

Hosted platform selection; publication, release and distribution; adaptive
autonomy; a rich user interface; breadth across many providers; and any
aicortex, Rahi or hqgit integration. Each is deferred by name in
`docs/decisions/00-founding-decisions.md` and none is a prerequisite for the
first slice.

This spec also does not choose the language, runtime or packaging. That
recommendation is `D-01` in the decision record, where it can be adopted or
rejected without editing a spec.

## 5. Decisions recorded during implementation

Dated entries for choices §3 was silent on. None changes what §3 requires.

**2026-09-19: the status descriptions corrected in this spec's territory, and
the evidence for each.** Section 3.3 requires evidence beside any claim above
*specified*. Four present-tense claims in the two documents this spec owns had
become false, and each is corrected with the measurement that falsified it. Each
was true when written; this entry records what changed, not a rule for changing
it.

| Corrected claim | Where it stood | The measurement |
|---|---|---|
| "no dependency declared below exists in any manifest, because no manifest exists" | the design record now folded in as section 3.8, status line | A Cargo workspace with seven member crates exists, and `crates/statecraft-run/Cargo.toml` declares `attest-ledger-core` pinned to `a9c3595`, used by `src/record.rs`. |
| `attest-ledger` listed as **proposed reuse** | same file, section 1 table | Same measurement: it is an actual dependency. The **reuse** disposition it was adopted under is unchanged. |
| "specs `008` and `009` are drafts", supporting deferral `F-10` | `docs/decisions/00-founding-decisions.md`, section 4 | `spec-spine registry list` reports both `approved` with `implementation: complete`. `F-10` stays deferred: whether the single-repository loop works is the owner's judgment, not a status field. |
| "one row of section 3 is adopted in part" | same file, adoption-status line | Section 5 of that file already records `D-03` adopted in full and `D-01` and `D-02` adopted for language and layout. The line is restated to match section 5, which stays the only place a row becomes binding. |

The original wording of each replaced claim is quoted in its replacement, and
the dated 2026-09-16 statements in both documents are left as written. Nothing
in this change adopts a `D-` row, lifts a deferral, or revises a disposition, an
acceptance requirement or a policy.

**2026-09-19: this spec's Verification preamble is stale, and the correction is
deferred.** The sentence introducing the block below reads "They assert nothing
about product behavior, because none is implemented." Measured on this date:
`cargo test --workspace` passes 515 tests across seven crates, and
`cargo run -p statecraft-cli -- --help` prints fifteen bound verbs, so the
clause after the comma is false. The commands themselves are untouched, they
still assert the authored foundation this spec owns, and `make verify SPEC=001`
passes unchanged. The owner has acknowledged the staleness and deferred the
correction, so the measurement is recorded here and the preamble is left as it
stands.

**2026-09-21: a borrowed ordinal is corrected in this spec's territory, and the
rule for reading one is recorded.** Section 3.8's tables and the decision record
this spec owns cite other corpora by ordinal. spec-spine collapsed its corpus and
renumbered contiguously on 2026-09-20, which makes an old ordinal worse than a
dangling one: it resolves, to a different document. Three citations in
`docs/decisions/00-founding-decisions.md` were corrected against
`docs/corpus-map.md` in that repository (097 to 078, 091 to 072, and `C-18`'s 102
to 081), and seven more elsewhere in the corpus.

What is **not** corrected is the point of this entry. `D-01`'s reasons cite the
archived predecessor's own spec 102, section 3.9 cites its 032, 040, 042 and 043,
and `crates/statecraft-envelope/PROVENANCE.md` cites hqgit's. Those belong to
other corpora, spec-spine did not renumber them, and remapping one would be the
same defect pointing the other way. The rule this spec now holds: an ordinal is
read together with the sentence that says whose it is, and a spec-spine ordinal
is trusted only from `registry list` or the corpus map. AGENTS.md carries the
operational form.

**2026-09-23: stale references reconciled, and nothing required changed.** The
2026-09-23 audit found references that were true when written and are not now.
The fixes in this change are:

- `.derived/` named where `002` section 3.19 moved the tree;
- `009` named as a live spec;
- the library dependency `spec-spine-core` missing from the component table;
- the grade table stopping at a row that called `002` implemented;
- the decision record's pre-renumbering spec-spine ordinals and its "no verb
  runs `spec-spine delta`".

Each is corrected in place where it was a current claim, or by an appended,
dated note where it records what was read at the time. The same pass corrected
cross-references in `002` to `006` and in `AGENTS.md`, and spec `000`'s own
stale descriptions ("contains no product code", `.derived/`, "none is
implemented"). Those are editorial: spec `000`'s section 5 says an anchor
forbids contradiction and leaves ordinary editorial amendment available, and
no anchored principle changes.

**2026-09-23: the CLI pin moves to `=0.23.0`, with its `D-06` record.** The
record is `D-06`'s dated entry in the decision record, and editing that record
is a change to this spec's territory, so it is noted here. The component table
names the new pin. The library row moved in its own, later change under spec
`002`.

**2026-09-24: PROPOSED, NOT ADOPTED. The corpus moves to spec-spine's amendment
model, and `002` is split along its seams by relocation only (owner item S).**
Prepared at the owner's request of 2026-09-24 for the owner's ratification
decisions; nothing below binds until the owner adopts it, and each decision is
in the table at the end. It is recorded here because spec `001` owns the
component boundaries and `D-02`, which the split touches, and because a
proposal filed beside the corpus would be a second place for a requirement to
live (`AGENTS.md`, Source ownership). Measurements are in the evidence
repository under `2026-09-24/session10/S/`, taken in disposable clones of `main`
at `17dbdb6` with the pinned spec-spine (the release `spec-spine.toml` names);
nothing was pushed from them.

*Part 1: the amendment model.* From adoption on, a change to what a spec
requires is a **new spec** with an `amends` edge to the spec it changes, not a
dated section 5 entry and not an edit to the amended spec's section 3. This is
spec-spine's rule (its specs 037, 082 and 083): the amended `spec.md` is not
edited to record the amendment, and `registry relationships <id>` reports
`amended_by (incoming)`. Section 5 of every spec keeps only genuine
implementation decisions: a choice the spec was silent on, with its reason.
Two measured consequences shape the rule:

1. **`amends` does not make the amending spec an owner of the amended spec's
   code.** The gate widens ownership through `amends` only for the amended
   `spec.md` itself. Measured (S3): a new spec `amends: [002]`, then a commit
   changing `crates/statecraft-home/src/flow.rs` with an authoring edit to the
   amending spec only: `couple` exit 1, `C-001` naming `002` alone. With an
   added `extends: { spec: 002, unit: crates/statecraft-home/, nature:
   corrective }`, the same commit couples (exit 0). So an amending spec that
   changes behavior in code **declares `extends` on each unit it changes**, in
   the same change that introduces it. That amends nobody's text and needs no
   waiver.
2. **An amendment that replaces acceptance says so** with
   `amends_verification` (spec-spine 082), so `verify <amended>` runs the
   replacement and prints the substitution.

*Part 2: the split of `002`, relocation only.* Four seams, as the owner named
them. `002` keeps initialization and lifecycle; two new specs take the harness
and the producer seams; distribution goes to `007` when `007` is created.

| Destination | Section 3 of `002` today | Section 5 entries of `002` today (by date and first words) |
|---|---|---|
| `002` initialization and lifecycle (stays) | 3.1 to 3.8, 3.10, 3.11, 3.12, 3.13, 3.16 to 3.21, 3.35, 3.36 | 2026-09-16 (all four); 2026-09-20 native root, relocation rewrites, manifest v2, two references left, ratified; 2026-09-21 dependency on 006, derived tree's three states; 2026-09-23 project block commands, 3.35 implemented, replacing a drifted file, env remove takes back the bridge, foreign finding, 3.16 frozen resolution; 2026-09-24 what initialization reports, outcomes implemented, withheld means partial (authority and implementation), setup profile (authority and implementation), provenance (authority and implementation) |
| `008` harness, hooks and skills (new) | 3.9, 3.14, 3.22 to 3.34, 3.37 | 2026-09-20 hooks ship in the harness, delivery verdict; 2026-09-21 every entry from the handoff fold through the deadline-attempt synchronisation except the three producer entries in the `009` row and the derived-tree coverage entry in the `002` row; 2026-09-22 all; 2026-09-23 the two live experiments, non-turn event, trial deadline, trailing-event decision, 3.34 implemented, permission experiment second campaign, launch-state answer, two reads for reconciliation, 3.34 trailer, 3.37 implemented, translation of `check`, gate log planted decision; 2026-09-24 contract 2 (authority and implementation), contract 4 (authority and implementation) |
| `009` producer adoption (new) | 3.15 | 2026-09-20 exact crates.io pin, trimmed producer tested, governance files written by this crate, contract path adopted; 2026-09-21 published 0.21.0 versus source, version literal, `.crate` digest; 2026-09-23 core 0.23.0; 2026-09-24 core 0.25.0, exact pin in new projects |
| `007` distribution (created after spec-spine 0.26.0) | none | 2026-09-23, adopted 2026-09-24: the release bundle contract (Part 9 step 1) |

Three rows are judgement calls and are named as such: 3.9 (adapters declare
what they own, and the code is `crates/statecraft-environment/src/adapter.rs`)
goes with the harness because an adapter is how a harness is delivered; 3.29
to 3.34 and 3.37 (admission, launch and startup records) could be a fifth seam,
"managed-session delivery and admission", and are kept with the harness only
because the owner named four; 3.36, the completion rule, cannot move whole
because it describes all of `002`, so it stays and each new spec gets its own
completion rule as an ordinary amendment after the split (Part 1), which is not
a relocation.

**Code ownership under `D-02`.** The code does not split along these seams:
`crates/statecraft-home` holds `flow.rs` (initialization), `harness.rs` and
`harness/` (the harness), `producer.rs` (the producer) and `launch.rs`
(startup). `D-02` as amended says a crate has exactly one owning spec. Three
ways to hold that:

- (a) **New specs own no code.** `008` and `009` carry requirements and declare
  `extends` on `002`'s crates; `002` keeps both crates. `D-02` holds unchanged,
  and a harness change couples by editing `002` or `008` (measured, S1b: any
  owner's `spec.md` clears the path).
- (b) **Sub-crate units.** `002` replaces its directory claims with file and
  directory units, and `008` and `009` establish theirs. The tool accepts
  overlapping claims silently (measured, S1: `002` owning
  `crates/statecraft-home/` and a new spec establishing
  `crates/statecraft-home/harness/` gives `lint`, `check --fail-on-unresolved`,
  `index coverage --fail-on-untraced` and `index check --fail-on-unresolved`
  all exit 0, and `index owner` lists both), so exclusivity would rest on
  authoring discipline. It amends `D-02`.
- (c) **Split the crates** (a harness crate, a producer crate). Code moves, so
  it is not relocation-only and is its own implementation change later.

*Part 3: how a relocation PR proves it changed no requirement.* Each
relocation PR moves whole sections, keeps each heading's text (the number may
change), and edits nothing inside a moved section. Its body carries the output
of a relocation proof run over its own base and head: for every `spec.md`,
split the body at every heading, key each section by its heading text with the
number removed, hash its body, and require every base section to appear at
head with the same digest (in any spec). New sections are allowed only as
scaffolding (a new spec's purpose, territory, out-of-scope and section 5
headers, and a one-line pointer where a section left). The script is in the
evidence (`relocation-proof.py`); measured on a relocation of 3.1 to 3.6 into
a new spec it reports 7 sections moved unchanged, 0 changed, exit 0, and after
one inserted word in a moved section it reports that section, exit 1. The
registry's own `sectionDigests` cannot serve: each digest is salted with the
spec's path (`<spec_path>#<anchor>`), so a verbatim move changes every digest
(measured: all six moved sections differ). If the owner adopts the proof, the
script becomes a claimed file under `scripts/` in the first relocation PR.

Each relocation PR also: moves the ownership edges that name a moved unit
(measured, S2: `003` and `004` declare `extends` naming `002` for
`crates/statecraft-environment/`; if that unit moves, the edges keep resolving
by path and nothing reports that they name a spec which no longer claims it,
so the PR retargets them); updates citations. There are 327 citations of the
form "`002` section 3.x" outside `002` (code comments in six crates and one
line in each of `003` to `006`) and 447 section references inside `002`.
Keeping each moved section's number in its new spec (so 3.14 stays 3.14 in
`008`) would leave every in-spec reference true and turn each outside citation
into a mechanical "`002` to `008`" rewrite that the proof script cannot see but
a citation map can; renumbering would make every one a semantic edit.

*Order.* Each step is its own PR; the relocations fall under the owner
delegation (#111) once this split is adopted.

1. This proposal, adopted or amended by the owner (one authority PR changing
   this entry to adopted, the `D-02` choice, and `AGENTS.md` for the
   amendment model).
2. The 0.26.0 migration (adoption, the exit and JSON contract amendment of
   spec `006`, the one-identity rule) lands before any relocation, so the
   relocations do not race the code it changes.
3. Relocation R1: `009` producer adoption (3.15 and its entries), the smallest
   seam, to prove the method.
4. Relocation R2: `008` harness, hooks and skills.
5. `007` is drafted from the bundle entry by relocation (the distribution
   seam), after 0.26.0's planned claims are adopted; the owner ratifies it;
   then it is built.
6. Fold, per spec: each adopted section 5 entry that states a requirement is
   folded into section 3 of the spec that now holds it, one spec per PR,
   `002` last because it is largest. A fold is not a relocation (it rewrites
   requirement text into its final form), so each fold PR is an authority
   change for the owner, and it keeps the folded entry's date and decision
   reference beside the rule. After the fold, section 5 holds implementation
   decisions only, and Part 1 governs every later change.

*Friction measured, for spec-spine.* (1) No exclusive claim transfer: partial
`supersedes` is additive by design (its spec 018 section 4 defers the
owner-stripping operation), and a plain second `establishes` overlapping an
existing directory claim raises no lint, so "exactly one owner" is not
checkable. (2) An `extends` edge naming a spec that no longer claims the unit
resolves silently. (3) `sectionDigests` are path-salted, so they cannot prove
a verbatim move. (4) `amends` never widens code ownership; the gate's
remediation text says so for `extends`, but nothing tells an amending spec it
also needs one. (5) `couple` resolves ownership from the checked-out tree's
committed index, not from `--head`: judging the same base and head gave exit 1
with the head checked out and exit 0 while a later commit that adds an
`extends` edge was checked out. CI checks out the pull request, so CI is
consistent, but a local reproduction depends on the checkout. (6) `compact`
merges specs and rewrites their citations; there is no inverse for a split.
(7) A new approved spec claiming a crate that does not exist yet still exits
1 under `index check --fail-on-unresolved` (`W-001`), which is what 0.26.0's
planned claims are to change for `007`. None of these blocks the split; (1),
(2) and (5) are recorded as authoring discipline the relocation PRs carry.

| Item | Options | Recommended default | Consequence of the default |
|---|---|---|---|
| S-A: amendment model | adopt as Part 1 / keep section 5 amendments | adopt, with the `extends` rule | Every later behavioral change is a new spec; section 5 stops growing with requirements |
| S-B: code ownership during the split | (a) new specs own no code / (b) sub-crate units, amending `D-02` / (c) split crates later | (a) | `D-02` unchanged; harness and producer code still couple through `002` or the seam spec |
| S-C: section numbers on relocation | keep numbers / renumber | keep numbers | Outside citations change only their spec number; no in-spec reference breaks |
| S-D: fifth seam for admission and launch (3.29 to 3.34, 3.37) | with `008` / own spec | with `008` | `008` is large; a later split of it is another relocation |
| S-E: the relocation proof | adopt the script as a claimed file / proof in PR bodies only | claimed file under `scripts/` | Each relocation PR carries a mechanical proof CI can rerun |
| S-F: fold PRs | owner merges each / delegated | owner merges each | The fold rewrites requirement text, so it stays the owner's act |

## Verification

Each line below is one command. These assert the authored foundation, which
exists today. They assert nothing about product behavior, because none is
implemented.

```verify:cli
test -f docs/decisions/00-founding-decisions.md
test -x scripts/check-authored-content.sh
scripts/check-authored-content.sh
grep -qF 'XII. Public claims are graded' standards/spec/constitution.md
grep -qiF 'specified' standards/spec/constitution.md
grep -qF 'Frozen by spec 000 as `independent-acceptance`' standards/spec/constitution.md
grep -qF 'D-01' docs/decisions/00-founding-decisions.md
grep -qF 'D-05' docs/decisions/00-founding-decisions.md
```
