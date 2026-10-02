---
id: "033-consented-spine-pin"
title: "A reviewed initialization plan may explicitly move the adopted spec-spine pin"
status: draft
implementation: pending
created: "2026-10-02"
summary: >
  Adds an explicit exact --spine request to init plan and init apply. The
  source and destination pins, producer identity, original configuration
  digest and minimal authored edit are part of the consented plan identity.
  Apply changes only the existing required_version value in spec-spine.toml
  after revalidating the plan, then uses the ordinary managed upgrade rules.
  No implicit pin move, dependency rewrite, ratification or branch protection
  change is authorized by initialization.
amends:
  - "002-environment-lifecycle"
  - "006-command-surface"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-environment/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "002-environment-lifecycle"
  - "006-command-surface"
obligations:
  - id: "R-1"
    kind: requirement
    text: "An exact --spine request is explicit consent input, never an inferred or default pin move."
    anchor: "3-1-exact-request"
  - id: "R-2"
    kind: requirement
    text: "The plan identity binds the original configuration digest, source and destination pins, producer identity, authored edit and all managed upgrade actions."
    anchor: "3-2-reviewed-plan"
  - id: "R-3"
    kind: requirement
    text: "Apply revalidates the complete plan before writing and changes only the existing required_version value, preserving all other authored configuration bytes."
    anchor: "3-3-minimal-apply"
  - id: "R-4"
    kind: requirement
    text: "Pin consent neither ratifies specs nor permits an incompatible base gate, secret transmission, dependency rewrite or branch protection change."
    anchor: "3-4-authority-boundary"
---

# 033: Consented spec-spine pin

## 1. Purpose

A fleet upgrade currently requires an operator to edit the adopted pin before
initialization can render the new profile. The operator cannot review the pin
move and its managed consequences as one plan. This draft proposes that explicit
pin request as an initialization input. It does not claim implementation,
acceptance, release or owner ratification.

## 2. Territory

This spec owns no new product unit. Its extends edges name the existing
initialization planner and applier, and the command surface that accepts and
reports their inputs. It amends the install and upgrade contract of spec 002
and the argument surface of spec 006. The existing managed-file drift rules,
project authority and plan consent remain in force.

## 3. Contract

### 3.1 Exact request

`init plan <path> --spine '=X.Y.Z'` requests one exact release. The accepted
syntax has a leading equals sign and one complete semantic version; ranges,
wildcards, missing components and ambiguous values refuse before any write.
The request must equal the linked producer's spec-spine release. Rendering an
older profile, bridge mode and changing the producer dependency are separate
proposals. Without --spine the existing mismatch refusal remains unchanged.

The target must already have a regular, contained `spec-spine.toml` with one
unambiguous string-valued `[meta].required_version` key. Missing, duplicate or
unsupported representations refuse with the key and reason. The command does
not recreate an adopted configuration or replace its other keys. A request
already equal to the adopted pin is an explicit no-op for that key.

### 3.2 Reviewed plan

Planning writes nothing. Its human and JSON reports name the current pin,
requested pin, linked producer release and identity, original configuration
digest, exact authored value edit, rendered profile identity, managed writes
and withheld paths. The plan identity binds all of these inputs and actions,
including the explicit --spine request and ordinary --replace consents.

The planner evaluates initialization against the proposed configuration bytes
without requiring the operator to edit the file first. It reports remote
settings obligations, required owner decisions and any inability to qualify a
base-gate transition; it never presents a bridge requirement as a passing gate.
A request outside the producer's supported release refuses a plan rather than
producing an identity whose apply would fail after writing the pin.

### 3.3 Minimal apply

`init apply <path> --spine '=X.Y.Z' --plan <identity>` must recompute the same
plan and require the same request and identity. Omitting or changing the
request, changing the configuration bytes, producer, rendering inputs or
replacement consents refuses before writing. No pin move occurs without both
the explicit request and its reviewed identity.

The authored operation replaces only the existing required_version string
value. Comments, spacing outside that value, key order, unrelated values and
newline convention remain byte-identical. An edit that cannot preserve those
bytes refuses. The replacement respects containment and refuses symlinked
configuration. It is written atomically after revalidating the original bytes.

The environment records the adopted pin and consented plan identity only after
the corresponding write succeeds. The normal managed-file matching and drift
rules still decide every subsequent write. An interrupted or partial apply
reports which operations completed, never claims full convergence, and can be
replanned from the actual tree; a prior identity must not authorize a different
remaining plan. This contract adds no cross-file rollback promise.

### 3.4 Authority boundary

A pin request consents only to the reported authored value edit and ordinary
initialization actions in that plan. It grants no spec approval, waiver,
execution posture, signing authority, secret transmission, remote settings
mutation, Cargo or npm dependency rewrite, or branch-protection bypass.
Repositories whose trusted base gate cannot run the proposed engine still
require a separately reviewed bridge before the pin move can land.

## 4. Acceptance criteria

- A mismatched adopted pin refuses without --spine and yields a read-only plan
  with an exact supported request.
- Range, wildcard, incomplete, duplicate-key, unsupported release, absent-key
  and symlink cases refuse with no mutation.
- Two configurations differing only in an unrelated comment have different
  consent identities; neither identity authorizes the other configuration.
- Omitting or changing --spine, --replace, producer identity or configuration
  bytes between plan and apply refuses before writes.
- A successful move preserves every configuration byte outside the value span,
  including comments and newline convention, and records the new pin.
- A same-pin request performs no authored write; an interrupted apply reports
  actual completion and requires a newly reviewed plan for remaining actions.
- Managed drift remains withheld without its explicit replacement consent;
  no dependency, spec lifecycle, secret or remote protection is changed.

## 5. Resolved decisions

Exact producer equality deliberately keeps this proposal independent of older
profile rendering and bridge mode. Preserving the authored configuration by a
single value edit makes the consent reviewable and avoids reformatting an
adopted user's file. Acceptance criteria are proposed behavior, not evidence
that the implementation exists.
