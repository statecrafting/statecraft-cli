---
id: "050-local-halt"
title: "A local halt record that run consults before launch, and denies when it cannot read"
status: draft
implementation: pending
created: "2026-10-21"
summary: >
  Adds an operator-written halt record in the product home, per registered
  repository, with three independently halted scopes: run admission, tool
  effects and dispatch. run reads it before launch and refuses a halted scope
  with a closed reason; a halt record it cannot read or verify denies with its
  own reason rather than admitting. Halting with a live attempt records stop
  requested; stop confirmed is recorded only when that attempt's cancelled
  outcome is durable. The hosted tenant halt fact is not defined here; it is
  consumed once its own spec merges.
amends:
  - "003-work-and-run-semantics"
  - "006-command-surface"
# No `extends` yet: the implementing change adds the edges on
# crates/statecraft-run/ (003) and crates/statecraft-cli/ (006) with the code.
depends_on:
  - "003-work-and-run-semantics"
  - "006-command-surface"
# `obligations` is spec-spine's own frontmatter field, used by approved specs
# such as 013, 014 and 018; `make gate` lints it with --fail-on-warn.
obligations:
  - id: "I-1"
    kind: invariant
    text: "No attempt launches while the run-admission scope is halted for its repository, and none launches when the halt record cannot be read or does not verify."
    anchor: "3-3-run-consults-before-launch"
  - id: "R-1"
    kind: requirement
    text: "A halted scope refuses with exit 2 and the closed reason halted naming the scope, the operator and the reason; an unreadable or unverifiable record refuses with halt-state-unreadable."
    anchor: "3-3-run-consults-before-launch"
  - id: "R-2"
    kind: requirement
    text: "Halting with a live attempt records stop-requested at once and stop-confirmed only when that attempt's cancelled outcome is durable; no reader reports the halt as effective on the attempt before then."
    anchor: "3-4-stop-requested-and-stop-confirmed"
  - id: "R-3"
    kind: requirement
    text: "The halt record lives in the product home, outside any target and outside the confinement, and every change to it appends to the repository's run record."
    anchor: "3-1-the-halt-record"
  - id: "V-1"
    kind: verification
    text: "A halted run-admission scope refuses run with exit 2 naming halted; a truncated, unparsable or chain-mismatched record refuses with halt-state-unreadable; a halt during a live attempt shows stop-requested until the cancelled outcome lands."
    anchor: "verification"
    inputs:
      - "crates/statecraft-run/src/record.rs"
---

# 050: Local halt

## 1. Purpose

An operator who wants every agent on a repository to stop has no switch
today: they can cancel the one live attempt, and the next `run` starts
another. A halt is a standing refusal an operator sets and lifts. It must
fail closed: a halt nobody can read is not permission to proceed. And it must
not overstate what it achieved: asking a live attempt to stop and that
attempt having stopped are different facts.

A hosted counterpart, a signed `tenant.halted` fact with the same three
scopes, is proposed for the hosted platform as `P-28`
(statecrafting/statecraft pull request 45) and is not adopted. This spec does
not define it. When its spec merges, a later amendment cites it by revision
and makes this record also consult it.

**Drafts this one cites, not yet on main:** spec 048, run operations (pull request 300). Each is cited by its proposed number so the reference survives ratification.

## 2. Territory

`statecraft-run` gains the halt record, its fold and the pre-launch check;
`statecraft-cli` gains `halt` and `resume` verbs and renders the state in
`runs`. The tool-effects scope reaches a live attempt through the admission
gate the child already executes (spec 002 section 3.32 rule 26) and the
exchange directory it reads (spec 004 section 3.18 rule 5); the gate script's
contracts are unchanged.

## 3. Behavior

### 3.1 The halt record

The halt state for a registered repository is a fold over halt and resume
entries appended to that repository's run record (spec 003 section 3.3),
in the product home beside the chain and the lock. Each entry names the
scope, the operator, a reason and the time. Nothing in the target, and
nothing the supervised process can reach, says whether a repository is
halted.

### 3.2 Scopes

| Scope | What it stops |
|---|---|
| `run-admission` | Launching any attempt: `run`, a retry, and a spec 048 operation that launches. |
| `tool-effects` | Releasing a governed tool call during a live attempt: the supervisor writes a halted decision into the attempt's exchange directory, which the child may read but not create entries in, so the gate refuses the next governed call. What the gate log then says is child-attested, never the proof of the halt; the halt record is. |
| `dispatch` | Handing work to another process or host. There is no local dispatcher yet, so this scope is recorded and refuses nothing until one exists. |

Each scope is halted and resumed independently. `halt` with no scope halts
all three.

### 3.3 Run consults before launch

Before it appends an intent, `run` takes the repository lock and folds the
halt state from the chain it has just opened. A halted `run-admission` scope
refuses with `Exit::Refused` (2), error kind `refused`, and the closed reason
`halted`, naming the scope, the operator and the reason. If the chain cannot be
read, or a halt entry does not parse or does not verify in the chain,
`run` refuses with `halt-state-unreadable`. There is no cached answer to fall
back to: the fold is read under the lock every time.

### 3.4 Stop requested and stop confirmed

Halting `run-admission` or `tool-effects` while an attempt is live records
`stop-requested` and asks the supervisor to cancel the attempt as an operator
cancellation (spec 003 section 3.4). `stop-confirmed` is recorded only when
that attempt's `cancelled` outcome is durable. Until then `runs` shows the
attempt live with the stop requested. An attempt that never reaches an
outcome goes to reconciliation (spec 003 section 3.6.1) like any other, and
the stop stays requested, never confirmed.

### 3.5 Resume

`resume` appends an entry lifting one scope or all three. It does not
restart anything; a later `run` is an operator's new act.

## 4. Out of scope

- Defining or reading the hosted `tenant.halted` fact (see section 1).
- Halting across repositories: a halt is per registered repository.
- Deadlines that pause while halted.

## 5. Resolved decisions

**2026-10-21: the halt state is in the run record, not a separate file.** A
separate file would need its own integrity story; the run record is already
hash-linked, fsynced before acknowledge and out of the supervised process's
reach (spec 003 section 3.3), so an edited or truncated halt is detected the
same way a tampered record is.

**2026-10-21: reason words follow the hosted proposal where they overlap.**
`halt-state-unreadable` is the word `P-28` proposes, so the local and hosted
refusals read alike if both are adopted. The local refusal is `halted`, not
`tenant-halted`, because there is no tenant here.

## Verification

This spec is `status: draft` and declares no acceptance until it is ratified
and built. Its acceptance will refuse `run` under a halted `run-admission`
scope, refuse it with `halt-state-unreadable` against a truncated, unparsable
and a chain-mismatched record, admit it after `resume`, and show
`stop-requested` then `stop-confirmed` across a halt during a live attempt.
