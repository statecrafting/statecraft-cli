---
id: "065-declared-fail-closed-refusal"
title: "A fail-closed screen's refusal of hostile content is declared, not a breach of spec 025's invariant"
status: draft
implementation: pending
created: "2026-10-29"
summary: >
  Amends spec 025. Invariant I-1 says no byte of untrusted content changes a
  Statecraft decision, evidence admission included; spec 014's secret screen
  refuses evidence whose content looks like a credential. A corpus case may
  therefore declare one refusal from a closed list, naming the decisions it
  is carried into and the recordings it leaves empty; those may differ from
  the twin's only as that refusal, and every other difference still fails.
  A refusal only withholds; nothing here lets content expand authority.
amends:
  - "025-injection-qualification"
depends_on:
  - "014-wire-evidence-admission"
  - "025-injection-qualification"
obligations:
  - id: "R-1"
    kind: requirement
    text: "A case may carry expectedRefusal naming one refusal from a closed list, the decisions it is carried into and the recordings it leaves empty; on the hostile side each named decision is that refusal and each named recording is empty, on the twin's side none is refused and each named recording holds the twin."
    anchor: "3-2-what-a-declaration-excuses"
  - id: "R-2"
    kind: requirement
    text: "Any refusal a case does not declare, and any difference outside a declaration, fails the case."
    anchor: "3-2-what-a-declaration-excuses"
  - id: "R-3"
    kind: requirement
    text: "The closed list holds only controls that refuse and retain nothing; it is extended only by a change that names the control and the case that needs it."
    anchor: "3-3-the-closed-list"
---

# 065: A declared fail-closed refusal

## 1. Purpose

Spec 025's suite found that a `wire-metadata` payload carrying a base64 run
trips spec 014's secret screen: the hostile wire evidence is refused as
`prohibited-content` and none of it is retained, while the benign twin is
admitted. Read literally, I-1 makes that a failure, because content changed
evidence admission. The screen is a privacy control that only withholds, and
weakening it to pass the suite is not an option. The owner decided on
2026-10-29 that such a refusal is declared rather than treated as a breach.

This spec carries that decision as an amendment. Spec 025's section 5 entry
of the same date recorded it in place, which is not how an approved spec is
amended; that entry is withdrawn when this spec is ratified.

## 2. Territory

This spec amends spec 025 sections 3.1, 3.3 and 3.4. It owns no code: the
implementation is already in spec 025's harness
(`crates/statecraft-adapter/tests/support/injection.rs`), its corpus and the
acceptance suite, under spec 025's own `extends` edges.

## 3. Behavior

### 3.1 The declaration

A corpus case may carry `expectedRefusal` with three members: `refusal`, one
name from the closed list as `<owning spec>/<reason>`; `decisions`, the
decisions the refusal is carried into; and `recordings`, the recordings it
leaves empty because the refused content is not retained. A case that carries
it names no `expectedRecording` among those recordings.

### 3.2 What a declaration excuses

On the hostile side, each named decision is the declared refusal and each
named recording is empty. On the twin's side, none of the named decisions is
refused and each named recording holds the twin, so the payload is shown to
be the cause and the recording to work. Digest fields under a named decision
may differ. Any other refusal, on either side, is undeclared and fails; any
other difference fails as section 3.4 already says.

### 3.3 The closed list

Version 1 of the list holds one control:
`014-wire-evidence-admission/prohibited-content`. A control joins only if it
refuses and retains nothing of the content it refused, and only by a change
that names the control and the case that needs it.

### 3.4 What does not change

A refusal withholds; it never grants. Nothing here permits content to add an
operation, capability, reviewer, acceptance or publication, and section 3.6
still holds: no detector's verdict is a decision.

## 4. Out of scope

- Any change to spec 014's secret screen.
- Declaring a difference that is not a refusal.

## 5. Resolved decisions

**2026-10-29 (owner decision): declare, do not weaken.** Of the three answers
(declare the refusal, redact under spec 014 so content cannot suppress
evidence, or leave the case failing), the owner chose to declare it, held to
exactly its terms by the comparison and a self-test.

## Verification

This spec is `status: draft` and declares no acceptance until it is ratified.
Its acceptance is spec 025's: the acceptance suite's
`wire-metadata/encoding-evasion` case passes only as the declared refusal, and
the adapter suite's self-test fails each departure from a declaration's terms.
