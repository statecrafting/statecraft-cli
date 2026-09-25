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

Not this spec's territory: the adoption ledger `docs/adoption/spec-spine.md`,
which no spec claims and `D-06` governs (section 3.13); the corpus contract
(`000`), the environment
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
| spec-spine (CLI), at the exact release `required_version` in `spec-spine.toml` names | Implemented, released, installed locally at `.tooling/bin` | Invokes its supported commands, parses its structured reports | **actual dependency**, on a released binary; which release, and since when, is in `docs/adoption/spec-spine.md` (section 3.13) |
| `spec-spine-core`, at the exact version the root `Cargo.toml` states in `[workspace.dependencies]` | Implemented, released on crates.io | `scaffold_init_json`, the governance starter set `002` section 3.15 consumes | **actual dependency**, a library linked by `crates/statecraft-home`; conformance is `002`'s, and each move is recorded in `docs/adoption/spec-spine.md` (section 3.13) |
| `attest-ledger` 0.1.0 | Implemented, Apache-2.0 | Record envelope, chain hashing, verification | **actual dependency** as of 2026-09-19: `attest-ledger-core` in `crates/statecraft-run`, pinned to git `a9c3595` until 2026-09-25 and to the crates.io release `=0.1.0`, the same source, since (spec `003` section 5). The disposition it was adopted under is the **reuse** row below. |
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

### 3.13 The producer pin is stated once, and each adoption is a ledger entry

`D-06` as amended on 2026-09-24 is the decision; this section is what the
corpus requires of it.

1. **One stated source per pin.** The CLI pin is stated only as
   `required_version` in `spec-spine.toml`. The linked library's exact version
   is stated only in `[workspace.dependencies]` of the root `Cargo.toml`. A
   spec, a root document or the decision record refers to those files and does
   not restate the number. A report or test that must name the version derives
   it from the build, never from a second literal.
2. **The decision and its applications are separate files.** The decision
   record keeps `D-06` as a stable decision. Each adopted release is an entry
   in `docs/adoption/spec-spine.md`, which no spec claims and which `D-06`
   governs. An entry follows the ledger's own entry format: identity, the
   bypass-floor and coupling review, exit codes, hook-read text, the re-index,
   evidence kinds kept apart, and what the adoption does not do.
3. **One producer identity.** The CLI pin and the linked library name the
   same spec-spine release and move in the same change; the ledger entry
   records that release as one identity with each artifact's evidence under
   it (bundle decision H-3 (a), owner Addendum 2 of 2026-09-24). A test
   refuses two pins that name different releases.
4. **The consequence this buys.** An adoption that changes no behavior edits
   only files no spec claims (`spec-spine.toml`, the root `Cargo.toml`,
   `Cargo.lock`, the ledger and the regenerated shards), so it couples with no
   spec edit and no waiver. A release that changes behavior this product
   depends on is not only an adoption: the code or requirement it changes
   carries its owning spec's authoring edit as usual, and never a waiver in
   its place.

Negative cases. A root document or a spec that states the *current* pin by
number is a defect to correct, not a second source to keep in step; a dated
record of what was measured under a named release is history, not a
restatement. An adoption entry written into
the decision record instead of the ledger is a defect: it re-creates the `C-001`
the ledger exists to avoid. A CLI pin and a library version naming different
releases do not satisfy rule 3, however each was qualified.

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

**2026-09-24: the authored-content rules also read text that is not a file
(section 3.6; proposed for the owner's merge, which changes the check
suite).** Under merge commits a pull request's title and body become the merge
commit's message, and each branch commit lands on `main` with its own message,
so both are history the rules in section 3.6 govern. `scripts/check-authored-content.sh`
gains a `--text FILE...` mode that applies the same two rules, with the same
patterns and exit codes, to the files it is given; the tree mode is unchanged.
CI uses it on the pull request's title and body and on every commit message in
the change (`.github/workflows/govern.yml`, the owner's request of 2026-09-24).
Nothing required changes: the rules are the ones section 3.6 already states,
applied to text they already govern.

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
test -f docs/adoption/spec-spine.md
grep -qF 'required_version' spec-spine.toml
```
