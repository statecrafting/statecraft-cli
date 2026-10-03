---
id: "034-boundary-read-acceptance"
title: "Protected boundary acceptance preserves read-only checkout access"
status: draft
implementation: pending
created: "2026-10-03"
summary: >
  Corrects the self-test and hostile-fixture acceptance of spec 004 section
  3.18 to preserve rule 2: the operator checkout and other attempt workspaces
  remain readable, and mutations are refused. Product home and launch records
  remain inaccessible except for the existing explicit exceptions.
amends:
  - "004-execution-adapter"
amends_verification:
  - "004-execution-adapter"
depends_on:
  - "004-execution-adapter"
---

# 034: Protected boundary read acceptance

## 1. Purpose

Spec 004 section 3.18 rule 2 grants read-only access to the operator checkout
and other attempt workspaces. Rule 8 and the hostile-fixture acceptance instead
require refused reads there. The owner selected preservation of rule 2 on
2026-10-03 and authorized drafting this correction before implementation.

## 2. Boundaries

This amendment replaces only the conflicting read expectations in rule 8 and
the section 3.18 hostile-fixture acceptance. It neither adds writable roots
nor changes confinement mechanisms, preflight refusal, invocation identity,
posture, protected evidence access, or any open item in rule 12.

## 3. Required behavior

### 3.1 Self-test

The fixed probe runs under the exact launch policy. It must be refused reading
and writing sentinels in the product home and launch records. It must be
allowed to read the operator checkout sentinel and refused writing it.
All other self-test requirements of spec 004 section 3.18 rule 8 remain.

### 3.2 Hostile-fixture acceptance

Reads of the operator checkout and another attempt workspace succeed. Writes,
truncation, rename, deletion and other mutations of those roots are refused,
with protected bytes unchanged. These expectations apply to every path
spelling named in the existing acceptance, including aliases and descendants
that leave the process group or session.

Reads and mutations of the run chain, override journal and state authority,
and launch records remain refused. Every other positive and negative
acceptance requirement of section 3.18 remains unchanged.

## 4. Acceptance criteria

1. A fixed-probe result distinguishes allowed checkout reads from refused
   checkout writes and refused home and launch-record reads and writes.
2. Hostile fixtures assert allowed reads and refused mutations for the
   checkout and other attempt workspaces, including the alias cases.
3. There is no test-only denial of the checkout sentinel used to make an
   otherwise readable checkout appear inaccessible.

## 5. Resolved decisions

Read-only access supports the existing base-policy and shared-object reads.
Correcting acceptance preserves that direction without widening write access.
This draft records the exact replacement text for owner ratification; the
direction to implement does not itself change the lifecycle label.
