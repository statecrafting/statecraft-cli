---
id: "053-operator-input-record"
title: "Operator input to a live attempt is recorded before delivery, expires, and is evidence on the attempt"
status: approved
implementation: pending
created: "2026-10-21"
summary: >
  Lets an operator steer a live attempt with a note, read by the agent at its
  next governed tool call, or a command, which starts a new turn as a spec 048
  continue. Each input is written to the run record before it is delivered,
  carries its author, reason and expiry, and moves through pending, delivered,
  then acknowledged or expired. The cockpit is view-only by default and grants
  control for a bounded time. Localhost only; no hosted transport and no
  unsigned session-authorized command. An abort command ends the attempt as
  spec 003's cancelled.
amends:
  - "003-work-and-run-semantics"
  - "039-local-cockpit-authority"
# No `extends` yet: the implementing change adds the edges on
# crates/statecraft-run/ (003), crates/statecraft-home/ (002) and
# crates/statecraft-cli/ (006) with the code.
depends_on:
  - "003-work-and-run-semantics"
  - "004-execution-adapter"
  - "039-local-cockpit-authority"
# Spec 048 (run operations) is a draft in pull request 300; the edge to it is
# added when both are ratified.
# `obligations` is spec-spine's own frontmatter field, used by approved specs
# such as 013, 014 and 018; `make gate` lints it with --fail-on-warn.
obligations:
  - id: "I-1"
    kind: invariant
    text: "No operator input reaches a child before its record, naming author, reason, kind and expiry, is durable in the run record."
    anchor: "3-1-the-input-record"
  - id: "R-1"
    kind: requirement
    text: "An input moves only pending to delivered to acknowledged, or pending or delivered to expired; delivered is recorded by the supervisor when it places the input, and acknowledged only from the gate log, labelled child-attested."
    anchor: "3-2-states"
  - id: "R-2"
    kind: requirement
    text: "A cockpit session is view-only until an operator grants it control for a stated duration; an input or operation submitted without a live grant is refused with exit 2."
    anchor: "3-4-view-only-by-default"
  - id: "R-3"
    kind: requirement
    text: "Inputs are accepted only from the local CLI and the loopback cockpit of spec 039; no remote transport and no command authorized by a session alone without an operator's grant."
    anchor: "3-5-local-only"
  - id: "V-1"
    kind: verification
    text: "A note sent during a live attempt is recorded pending, delivered at the next governed call, and expires if no call comes before its expiry; a command is refused without a control grant; the attempt's record carries every input as evidence."
    anchor: "verification"
    inputs:
      - "crates/statecraft-run/src/record.rs"
---

# 053: Operator input record

## 1. Purpose

The only correction an operator has for a running attempt today is to cancel
it and start again. A short instruction ("the fix is in the parser, not the
lexer") is cheaper and keeps the work done so far. It is also a human decision
that changes what the agent does, so it has to be evidence: who sent it, why,
when it reached the agent, and whether the agent saw it. preloop's operator
notes (`docs/guide/operator-notes.md`, `cli/internal/cmd/agents_note_hooks.go`,
`agents_pause.go`, Apache-2.0) solve the delivery problem with a hook that
pulls pending notes at a turn boundary; this spec re-implements that idea over
this product's exchange directory and run record.

**Drafts this one cites:** spec 048, run operations (pull request 300, not yet on main), and spec 050, local halt (on main as a draft since pull request 302). Each is cited by its proposed number so the reference survives ratification.

## 2. Territory

`statecraft-run` gains the input record and its fold. The supervisor in
`statecraft-home` places a delivered input where the gate reads it.
`statecraft-cli` gains `input note` and `input command` verbs and the cockpit's
control grant. Spec 002's gate script learns to return a pending note as
context; its other contracts are unchanged.

## 3. Behavior

### 3.1 The input record

An input is appended to the run record as an intent before anything else
happens: kind (`note` or `command`), body, author, reason, the attempt it is
for and an expiry. Its later states are appended as outcomes. Nothing about an
input lives only in memory.

### 3.2 States

| State | Recorded by | When |
|---|---|---|
| `pending` | the verb | the record is durable |
| `delivered` | the supervisor | it has placed the input where the child reads it |
| `acknowledged` | the supervisor, from the gate log | the gate log says the gate returned it; child-attested (spec 004 section 3.18 rule 5) |
| `expired` | the supervisor | the expiry passed, or the attempt ended, before acknowledgement |

`delivered` is the supervisor's own act and is a fact. `acknowledged` is
testimony from inside the confinement and is rendered as such, never as proof
the agent acted on the input.

### 3.3 Note and command

- A **note** is read at the agent's next governed tool call. The supervisor
  writes it into the attempt's exchange directory, where the child may read
  but not create entries, and the gate returns it beside its decision. An
  attempt that makes no further governed call never reads it, and the note
  expires.
- A **command** starts a turn. In print mode a running turn cannot be
  extended, so a command is a spec 048 `attempt.continue` with the command's
  body as the prompt, under the same lease, key and version rules, after the
  current attempt has an outcome. `abort` is the one command that acts on the
  live attempt: it is spec 048's `attempt.abort` and ends it `cancelled`
  (spec 003 section 3.4), with the input as the recorded reason.

The steering text of every input is recorded on the attempt it was for, as
evidence beside its outcome.

### 3.4 View-only by default

A cockpit session (spec 039) can read runs and inputs and cannot submit
either until an operator, at the CLI, grants that session control for a
stated duration, at most one hour. The grant is recorded with the operator's
name and ends at its expiry or when revoked. The CLI itself needs no grant:
its operator is already the local user.

### 3.5 Local only

Inputs are accepted from the CLI and the loopback cockpit only. Hosted
delivery stays deferred under `F-01`. No session token alone authorizes a
command; every command traces to an operator's grant or the operator's own
CLI invocation.

## 4. Out of scope

- Agent-authored notes (preloop's "a note from an agent").
- Approving a tool call, lifting a halt (spec 050) or granting anything by an
  input: an input steers, it never authorizes.
- Remote or hosted delivery.

## 5. Resolved decisions

**2026-10-21: a command is a continue, not an injection.** preloop injects a
note into the next outbound model request through its gateway. This product
does not sit in the model request path (spec 013's witness reads it as
untrusted testimony and never rewrites it), so a note goes through the gate
the child already executes, and a command becomes a new attempt.

## Verification

Ratified by the owner on 2026-10-31. Acceptance is declared by the
implementing change, which will send a note during a live attempt and see it
recorded pending, then delivered at the next governed call, and expired when
no call comes before its expiry; refuse a cockpit command without a control
grant; and find every input, with author and reason, in the attempt's record.
