<!-- Spec: specs/001-boundaries-and-authority/spec.md -->

# Boundaries, reuse, and the first vertical slice

Prepared 2026-09-16. Owned by spec `001-boundaries-and-authority`.

**Status: design record, written before any code existed.** As prepared, it read
"nothing here is implemented, and no dependency declared below exists in any
manifest, because no manifest exists". That sentence described 2026-09-16 and no
longer describes the tree; it is replaced rather than deleted, because what the
tables keep apart has not changed. The distinction is between a dependency this
product would take on a component that is implemented today, and an interface
this product proposes and neither side has built.

**Corrected 2026-09-19.** A Cargo workspace exists with seven member crates, and
one row below has moved from a proposal to a declared dependency:
`attest-ledger-core` is in `crates/statecraft-run/Cargo.toml` as a git
dependency pinned to `a9c3595`, used by `crates/statecraft-run/src/record.rs`.
Every other row's state and disposition is unchanged, and no disposition in
either table is revised here: a disposition is a decision, and moving one is the
owner's act, not a consequence of a build.

## 1. Component owners, and what this product's relationship actually is

| Component | State today | This product's relationship | Kind |
|---|---|---|---|
| spec-spine 0.20.0 | Implemented, released, installed locally at `.tooling/bin` | Invokes its supported commands, parses its structured reports | **actual dependency**, on a released binary |
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

### The dispositions, stated as spec 001 section 3.2 requires

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

## 2. What the archive is used for, and what it is not

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

## 3. The two interaction modes, drawn out

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

## 4. The bounded first vertical slice

One repository, one work item, one adapter, one isolated workspace, one
independent inspection, one reviewable outcome. **Publication is not part of it**
(F-02).

| Step | Verb (proposed) | Spec | What must be observably true |
|---|---|---|---|
| 1 | `statecraft project register <path>` | 002 | A qualification verdict with reasons is recorded. Nothing is written inside the target. A non-git path is `unqualified`; a corpus-less repository is `ungoverned`. |
| 2 | `statecraft env plan` then `env apply` | 002 | Managed bytes are written and recorded in a committed manifest with source and digest. A pre-existing user instruction file is left untouched and reported `foreign`. A second run is a no-op. |
| 3 | `statecraft work list` | 003 | The ready set comes from spec-spine's structured report, with the field each row came from named. A target whose corpus does not compile refuses, rather than reading `.derived/` directly. |
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

Spec `006` owns the surface and spec `009` bound steps 3 to 6. Nothing in the
right-hand column revises what the step must make observably true, and the
acceptance below is untouched.

### The slice's acceptance, stated as refusals

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

### What the slice deliberately does not prove

That the environment is safe against hostile code. Spec 004 section 3.6 names
three residuals it does not close, and closing them needs an operating-system
mechanism deferred as F-09. The slice proves the record is **complete** through
the supervisor's path, which is a different and smaller claim.

## 5. Interfaces this product would publish later

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
