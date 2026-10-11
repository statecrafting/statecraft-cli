---
id: "040-application-operations"
title: "Callable application operations with unchanged envelopes and chain effects"
status: approved
implementation: pending
created: "2026-10-10"
summary: >
  Extract run, shared attempt launch, acceptance, override recovery, run
  reconciliation and startup trial orchestration from the binary into the
  existing CLI library. Typed operations own admission and effects and return
  render::Answer. Verb adapters parse and render. Four independently green
  delivery slices preserve envelope bytes, record ordering and execution
  ownership, providing the shared boundary a later local cockpit calls.
amends: ["006-command-surface"]
depends_on:
  - "002-environment-lifecycle"
  - "003-work-and-run-semantics"
  - "004-execution-adapter"
  - "005-acceptance-and-evidence"
  - "006-command-surface"
  - "007-family-envelope"
  - "039-local-cockpit-authority"
obligations:
  - id: "R-1"
    kind: requirement
    text: "Every callable operation performs its own authoritative admission, including registration, arming, lock acquisition, journal reads and report selection."
    anchor: "3-1-the-callable-boundary"
  - id: "R-2"
    kind: requirement
    text: "Library and verb yield byte-identical family envelopes and identical ordered chain effects under controlled observations, including refusals and interruptions."
    anchor: "3-2-observable-equivalence"
  - id: "R-3"
    kind: requirement
    text: "Four delivery slices remain independently buildable and verified; no HTTP or async dependency enters the core crates."
    anchor: "3-3-delivery-stack"
---

# 040: Application operations

## 1. Purpose

Spec 002 section 3.20 requires a dashboard to be another caller of the same
operations, with one execution owner and no background work-driving loop.
039 permits the local cockpit but supplies no implementation. Today the binary
owns orchestration that `statecraft_home::service::execute` does not cover.
Exposing a second caller without first moving that orchestration would duplicate
admission or let callers bypass it.

The direction is platform design
`statecraft:8fa73bc:docs/design/07-statecraft-dev-local-cockpit.md`, sections
2, 5, 8, 12 and 13, and P-25 in
`statecraft:8fa73bc:docs/decisions/00-adoption-register.md`. Platform PR 28 was
squash-merged as `8fa73bc`. CLI references were checked against `f98f9f0`:
`crates/statecraft-cli/src/main.rs`, `src/lib.rs`, `src/render.rs` and the
regression suites named below. External design remains proposed; this spec is
the owner-ratified local requirement.

## 2. Territory

This spec declares no forward source ownership. The first implementing change
adds an additive `extends` edge on spec 006's `crates/statecraft-cli/` directory.
Every new application module and integration test stays in that existing crate
and joins that edge in the same change. Spec 006 remains the sole crate owner.
No core crate, root workspace declaration, gate, setup profile or environment
manifest changes are required or authorized.

## 3. Behavior

### 3.1 The callable boundary

Expose typed library operations for run, acceptance, override recovery, run
reconciliation and startup trial, with a shared callable attempt-launch path.
The existing launch helper is shared by run and trial; this extraction adds no
new launch verb. Override recovery is the existing `override recover` operation,
not a renamed run recovery action.

Operations live in the CLI library and return its existing `render::Answer`
with serializable domain reports. They neither print nor choose human versus
JSON formatting, process exit, or parse argv. The binary retains argument
parsing, usage diagnostics, invocation selection, final rendering and exit
mapping. Both callers use the same 007 envelope renderer, preserving verb
identity, schema version, outcome, exit code, summary, report/error distinction,
error kind, key ordering and newline. No new envelope dialect is introduced.

All authoritative admission belongs inside the callable operation. In
particular, registration resolution, arming checks, operation locks, journal
reads, contract and spec-spine report selection currently performed in dispatch
arms or binary helpers move inside. A typed caller supplies operands and the
home/repository context, never a trusted pre-admitted registration, an assertion
that the repository is armed, or an unchecked report chosen to bypass admission.
Internal helpers may share resolved state only after the public operation has
performed the corresponding checks.

Preserve admission order and authoritative effects exactly. An unarmed or
unregistered target still refuses before provider launch, workspace creation
or attempt append wherever the current verb does. Preserve refusals that
legitimately have authoritative effects; no transport-oriented assumption may
turn them into an assertion that nothing happened. Locks cover the same work
and retain the same lifetimes, including cleanup and chain finalization.

Execution stays synchronous and attached to its caller. Interrupt handling,
process-group termination, startup evidence, unknown outcomes and recovery
semantics remain spec 004's and the current implementation's. A provider never
outlives its attempt. No HTTP, async runtime or watcher enters the core crates;
this spec adds no server, daemon, detached task or background scheduler.

### 3.2 Observable equivalence

The extraction changes placement, not behavior. Keep existing regression tests
and acceptance intact. Differential tests compare a verb invocation with the
corresponding typed operation rendered through `render::Answer`, using the
same logical inputs and explicitly controlled clocks, identities, paths,
provider observations and initial journals. Compare full serialized envelope
bytes and the ordered appended records, including their chain linkage. Compare
preexisting chain heads and no-append cases as well as successful appends.

Deterministic fixture inputs may control nondeterminism at existing observation
boundaries. Tests must not erase or normalize outcome, evidence, admission,
identity, record order, timestamps or hashes to conceal disagreement. A fixture
adaptation must preserve the existing behavioral assertion. No live paid
provider invocation is required or authorized.

Named criteria, each backed by an integration test in the implementing stack:

| Criterion | Required case and comparison |
|---|---|
| A-EQ-ACCEPT | Acceptance with valid evidence produces equal envelopes and ordered chain effects; missing evidence and moved contract closure retain their current refusal and effects. |
| A-EQ-RECOVER | Each existing override recovery choice produces equal envelopes and effects; malformed journal and inadmissible choice retain their current outcomes. |
| A-EQ-RECONCILE | Existing reconcile choices, including interrupted and unknown attempts, produce equal envelopes and effects without invented success. |
| A-EQ-LAUNCH | Controlled successful, failed, refused-before-launch and interrupted provider observations preserve envelopes, attempt records, startup finalization and cleanup. |
| A-EQ-RUN | Typed and verb run preserve admission and effects for registered/armed input and unregistered, unarmed, busy lock and unresolved judge refusals. |
| A-EQ-TRIAL | Startup trial preserves its conclusion and records, including repeat refusal and incomplete startup evidence. |
| A-ADMISSION | Invoke the public typed boundary directly for every dispatch admission case: registration, arming, busy lock, malformed journal and report/closure selection; none can be bypassed by choosing the library caller. |
| A-ADAPTER | The final binary adapters contain no operation admission or orchestration; format selection cannot change results or effects. |

Design section 12's CLI/HTTP equivalence scenario requires a server and is
carried by spec B for queries and spec D for commands. This spec establishes
its prerequisite library/verb equivalence without claiming HTTP tests.
Unarmed run, changed closure, absent evidence, unknown outcome and interrupted
execution are tested here at the existing operation boundary and must also be
tested through HTTP in the later specs that expose them. Distribution, session,
SSE and browser-disconnect scenarios stay with B, C and D in their scope.

### 3.3 Delivery stack

Section 3.1's prohibition on HTTP and async dependencies in core crates applies
throughout all four independently buildable and verified delivery slices.

Deliver in order as independently green PRs based on main, each naming its
predecessor. Count additions plus deletions toward the roughly 1,000 changed
line target. A logical slice may use more than one bounded PR if necessary;
each intermediate tree compiles and preserves behavior. No partial boundary
is presented as the finished callable operation.

| Slice | Extraction | Regression suites, including negative cases |
|---|---|---|
| 1 | Acceptance, override recovery and run reconciliation; typed operands and shared target resolution | `integration_slice`, `readiness_override`, `contract_binding`, `run_startup` |
| 2 | Attempt plan, startup finalization, observed launch state and conclusion helpers | `run_startup`, `posture_coverage`, `protected_boundary`, `native_stream` |
| 3 | Shared launch body in bounded helper groups, preserving ordering, supervision and held resources | `run_startup`, `qualification`, `qualification_workflow`, `provenance` |
| 4 | Run admission and startup trial orchestration; finish thin adapters and remove format/printing from the library path | `arming_consent`, `startup_trial`, `one_resolved_judge`, `family_envelope` |

Each slice runs its named suites and local corpus/workspace checks before
commit. The delivery plan introduces `application_equivalence` in slice 1 for
acceptance, recovery and reconciliation, then expands that target in slices
2 through 4 alongside the operations each slice extracts. Each slice runs
the target's tests for the operations delivered so far; final verification
runs the accumulated target. New differential tests run as ordinary CLI integration tests in the
existing workspace CI test job. Final completion requires every criterion and
the full declared verification locally and in CI, with results distinguished.
Set `implementation: complete` only in the final implementing PR once that
verification passes. Approved requirements are not rewritten to fit a failed
extraction; an incompatibility returns to the owner.

## 4. Out of scope

`statecraft dev`, HTTP, frontend, protocol schema, generated TypeScript,
watchers, SSE, bundle distribution, new command semantics, production keys,
platform panel, multiple repositories and persisted read models. No core crate
API redesign, new provider usage or changes to acceptance authority.

## 5. Resolved decisions

Use the existing CLI library and `render::Answer` rather than creating another
service crate: orchestration already depends on CLI report and adapter bindings,
and moving it need not change the domain crates. Internal helper extraction
precedes removal of the remaining binary orchestration so every intermediate
candidate stays buildable. Ownership edges arrive with implementation rather
than making this spec an owner of unrelated CLI work.

## Verification

These commands are required for final implementation acceptance. While
implementation is pending they describe future verification and are not evidence of
implementation. The ordinary workspace CI job includes these Cargo test
targets; hosted results and actual AI review verdict are reported separately.

```verify:cli
cargo test -p statecraft-cli --locked --test integration_slice --test readiness_override --test contract_binding --test run_startup
cargo test -p statecraft-cli --locked --test posture_coverage --test protected_boundary --test native_stream
cargo test -p statecraft-cli --locked --test qualification --test qualification_workflow --test provenance
cargo test -p statecraft-cli --locked --test arming_consent --test startup_trial --test one_resolved_judge --test family_envelope
cargo test -p statecraft-cli --locked --test application_equivalence
```
