---
id: "055-codex-adapter"
title: "A Codex adapter, built on the capability seam and admitted only by its own pass of spec 004's suite"
status: draft
implementation: pending
created: "2026-10-21"
summary: >
  Proposes statecraft-adapter-codex, the second provider adapter under the
  conditional F-07 lift of 2026-10-20. It mirrors the Claude Code adapter's
  shape: codex exec's JSONL stream mapped to spec 003's closed outcomes, a
  constructed environment, a per-run CODEX_HOME in the spirit of spec 042, and
  harness delivery under spec 008 without touching the user's own Codex
  configuration. It declares its capabilities through the seam proposed as
  spec 047 and is built only after that seam merges. It is qualified only by
  its own pass of spec 004's negative suite on fixtures recorded from a named
  Codex version. An appendix records preloop's facts on other harnesses as
  reference for later adapters; no code.
# Amends 004 only: its section 4 puts "A second provider" out of scope, and
# the conditional F-07 lift of 2026-10-20 (spec 001 section 5) brings Codex
# into scope here. Spec 008 is followed unchanged (its section 3.9 adapters
# are additive), so it is a dependency, not an amendment.
amends:
  - "004-execution-adapter"
# No `extends` and no crate claim yet: the crate is written only after spec
# 047 merges and this spec is ratified, and the implementing change claims it.
depends_on:
  - "004-execution-adapter"
  - "008-harness-delivery"
  - "042-per-run-provider-configuration"
  - "047-capability-seam"
# `obligations` is spec-spine's own frontmatter field, used by approved specs
# such as 013, 014 and 018; `make gate` lints it with --fail-on-warn.
obligations:
  - id: "I-1"
    kind: invariant
    text: "No Codex binary version is qualified except by a recorded pass of spec 004's full negative suite run against this adapter with its own fixtures; a pass by the Claude Code adapter never counts."
    anchor: "3-5-qualification"
  - id: "I-2"
    kind: invariant
    text: "The adapter never reads, writes or rewrites the user's own Codex configuration directory; every file it gives Codex is in a per-run directory the supervisor creates."
    anchor: "3-3-per-run-configuration"
  - id: "R-1"
    kind: requirement
    text: "Exactly one terminal turn event maps to an outcome; zero or more than one, an error event, or a thread id that changes mid-stream maps to interrupted, never completed."
    anchor: "3-2-the-stream-and-the-outcome-mapping"
  - id: "R-2"
    kind: requirement
    text: "A launch whose governing PreToolUse hook Codex does not report as enabled, synchronous and trusted for the workspace refuses before the turn starts."
    anchor: "3-4-the-gate-hook"
  - id: "V-1"
    kind: verification
    text: "Spec 004's negative suite passes against this adapter on recorded Codex fixtures; a source check finds no Codex name outside its crate; a run leaves the user's Codex directory byte-identical."
    anchor: "verification"
    inputs:
      - "crates/statecraft-adapter/tests/no_provider_names.rs"
---

# 055: Codex adapter

## 1. Purpose

Spec 004 built one adapter boundary and one adapter, and the founding record
deferred breadth (`F-07`) until a second provider could be added without
weakening the first. On 2026-10-20 the owner named Codex as that provider,
on three conditions recorded in `docs/decisions/00-founding-decisions.md`
section 5: the crate is built only after the capability-flagged seam merges,
it is admitted only by its own pass of spec 004's full suite, and nothing
weakens the boundary to fit it. This spec states what the adapter does so the
seam (spec 047, a draft) can be checked against a real second declaration
before either is built.

**Drafts this one cites, each on main as a draft:** spec 047, the capability seam (pull request 299), spec 052, golden captures (pull request 304), and spec 053, operator input (pull request 307). Each is cited by its proposed number so the reference survives ratification.

## 2. Territory

A new crate, `statecraft-adapter-codex`, beside
`statecraft-adapter-claude-code`, the only crate allowed to name Codex. It
implements spec 004's adapter protocol and declares its capabilities through
spec 047's declaration. Harness delivery for Codex follows spec 008; spec
008 section 3.28's rule that an unmarked registration is the user's applies
unchanged. No crate is claimed until this spec is ratified and spec 047 has
merged.

## 3. Behavior

### 3.1 What is spawned

`codex exec --json --color never --sandbox workspace-write
--skip-git-repo-check -C <workspace> -c approval_policy="never" -`, the prompt
on standard input. This is the invocation preloop's host runner uses
(`cli/internal/cmd/runner_host_exec_codex.go`, Apache-2.0); it keeps Codex's
own workspace sandbox and its native `PreToolUse` governance and bypasses
neither. This product's confinement (spec 004 section 3.18) wraps it as it
wraps Claude Code. Measured against an installed Codex, the local version on
2026-10-21 is `codex-cli 0.163.0`; preloop's facts are for 0.162.1.

### 3.2 The stream and the outcome mapping

Codex emits JSONL. The events the mapping reads:

| Event | Read as |
|---|---|
| `thread.started` | The provider session id (`thread_id`). A second, different id in one stream is a fault. |
| `turn.completed` | One terminal turn that finished; its `usage` carries input, cached input and output tokens. |
| `turn.failed` | One terminal turn that failed. |
| `error` | A fault. |

Exactly one terminal turn is required. `turn.completed` with no fault maps to
`completed`; `turn.failed` maps to `failed` only when the failure is a
judged result, and the fixtures decide which `turn.failed` shapes are judged
and which are API-side, as spec 004 sections 3.11 to 3.13 decided for Claude
Code; until a fixture shows the shape, it maps to `interrupted`. No terminal
turn, two terminal turns, an `error` event or a changing thread id map to
`interrupted`. Cost is `null` unless the stream proves one; token usage is
recorded with provenance `provider`.

### 3.3 Per-run configuration

Codex reads its configuration, hooks and credentials from `CODEX_HOME`
(default `~/.codex`). The supervisor gives each attempt its own `CODEX_HOME`
inside the per-run configuration directory of spec 042, holding the hooks
file and a credential placed by the supervisor, and constructs the child
environment with that one name added. The user's `~/.codex` is never read for
configuration and never written. A launch with no credential refuses, as
spec 042 section 3.2 rule 3 does for Claude Code.

### 3.4 The gate hook

Codex runs `PreToolUse` command hooks from `hooks.json` only when the hooks
feature is on and the hook is trusted, and trust hashes belong to Codex.
Before the turn, the adapter asks Codex for its effective hooks for the
workspace (`codex app-server`, `hooks/list`, as preloop's
`runner_host_exec_codex_hooks.go` does, sending no thread, turn or model
request) and refuses unless this product's gate is listed as a `PreToolUse`
command hook, synchronous, enabled, trusted or managed, with an unrestricted
matcher. The query runs in the attempt's own constructed environment: the
same per-run `CODEX_HOME` (section 3.3), working directory and executable
the turn will use, never the operator's `~/.codex` and never an inherited
`CODEX_HOME`, so the hooks it lists are the hooks that turn will run. A query
that cannot be made in that environment refuses the attempt. It never writes
a trust entry itself. How a per-run `CODEX_HOME` obtains trust is the open
question in section 6.

### 3.5 Qualification

Qualification is spec 004 section 3.4's: a recorded pass of the full negative
suite, by this adapter, on a named Codex version, with fixtures recorded from
that version under this crate's own testdata, in the shape the draft spec 052 proposes
for golden captures. A Claude Code pass never counts (spec 004 section 3.4).
Until qualified, the adapter runs labelled `unqualified`.

### 3.6 Declared capabilities

Its spec 047 declaration is measured, not copied from Claude Code's; each
field is set from a fixture or a recorded probe, and a field nobody has
measured is declared `false`.

## 4. Out of scope

- Any provider beyond Codex; each stays deferred under `F-07`.
- Operator notes for Codex (the draft spec 053 delivers through the gate; Codex's
  `PreToolUse` `additionalContext` is the carrier, measured when built).
- Wire witnessing of Codex traffic (spec 013 names only Claude Code's
  authority).

## 5. Resolved decisions

**2026-10-21: the reference appendix lives here, not in `docs/`.** The work
order asked for a reference document of preloop's facts on other harnesses.
`docs/` is claimed by spec 001, and a new file there is an owner act; the
facts belong beside the adapter that will use them first, as an appendix that
specifies nothing.

## 6. Open question for the owner

**O-1.** A per-run `CODEX_HOME` holds a hooks file Codex has never trusted, and
this adapter must not write trust itself. Two routes keep that rule:
administrator-managed hook policy (Codex reports `managed`), installed once by
the operator, or a one-time operator trust of the gate's exact command
carried into each per-run directory by Codex's own mechanism. Which route,
or should the adapter refuse until one is measured?

## Appendix A: other harnesses (reference only)

Recorded from preloop at revision
`a9d9bc3dae887cd447c1ab179a29765060b1c8fe`, `docs/guide/operator-notes.md`
(Apache-2.0), for later adapters. This appendix specifies nothing and none of
these providers is admitted.

| Harness | Hooks it fires | What reaches the model |
|---|---|---|
| Cursor CLI | `sessionStart`, `beforeSubmitPrompt`, `beforeShellExecution`, `beforeMCPExecution`, `preToolUse`, `stop`, `sessionEnd` | `preToolUse` `additional_context`, collected only on allow and deny; `stop` `followup_message`, at most three continuations. |
| OpenCode 1.18.32 | `experimental.chat.system.transform`, `session.deleted`, through a plugin | The plugin appends to the mutable `system` array before each model request. |
| Copilot CLI | `sessionStart`, `sessionEnd`; `preToolUse` carries nothing | No verified model-visible field. |
| Devin CLI | `SessionStart`, `UserPromptSubmit`, permission hooks | Session registration only; no verified carrier. |

## Verification

This spec is `status: draft` and declares no acceptance until it is ratified,
spec 047 has merged, and the crate is built. Its acceptance will run spec
004's full negative suite against this adapter on fixtures recorded from a
named Codex version, extend the provider-name source check to Codex, refuse a
launch whose gate hook Codex does not report trusted, and compare the user's
Codex directory before and after a run.
