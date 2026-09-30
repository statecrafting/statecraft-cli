---
id: "026-ownership-disagreement"
title: "doctor reports a path whose recorded ownership disagrees with the transfer journal or the current rendering"
status: draft
implementation: pending
created: "2026-09-27"
summary: >
  Amends spec 002 section 3.5 with one additive, read-only doctor finding,
  ownership-disagreement. For each path, doctor compares three facts: the
  manifest entry's class, the latest transfer-journal record for the path, and
  the class the current selection's rendering would give it. Each incompatible
  ownership state defined below is reported with both sources named. doctor
  repairs nothing, transfers nothing and never treats a rendering it could not
  compute as agreement. Backlog SC-006.
amends:
  # Section 3.5, the doctor states and findings. Under the approved-spec
  # amendment rule, 002 stays unedited; registry relationships reports the edge.
  - "002-environment-lifecycle"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-environment/" }, nature: additive }
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "002-environment-lifecycle"
  - "006-command-surface"
obligations:
  - id: "R-1"
    kind: requirement
    text: "doctor reports ownership-disagreement whenever a non-none journal target differs from the recorded class; rendered is managed while journaled is user; recorded is adopted while rendered is managed; recorded is managed while rendered is absent; or an existing path has recorded user, journaled none and rendered managed. It names each source and its value."
    anchor: "3-2-the-finding"
  - id: "I-1"
    kind: invariant
    text: "The finding is read-only and deterministic: doctor writes no file, manifest or journal byte, performs no transfer, and reports the same findings for the same inputs."
    anchor: "3-4-read-only-and-deterministic"
  - id: "V-1"
    kind: verification
    text: "Fixture repositories prove every disagreement row, including recorded adopted with rendered managed, plus the unavailable rendering, the legacy manifest without a journal, and byte-identical trees before and after doctor."
    anchor: "verification"
    inputs:
      - "crates/statecraft-environment/tests/ownership_disagreement.rs"
      - "crates/statecraft-cli/tests/doctor_ownership.rs"
---

# 026: doctor reports ownership disagreement

## 1. Purpose

Spec 002 section 3.35, "Per-path ownership transfer, as an operator's act,"
makes ownership transfer per path, explicit and journaled. Its rule 5 says a
manifest whose journal disagrees with its entries is reported by `transfer
plan` and refused by `transfer apply` and `transfer revert`. `doctor`, the
diagnostic surface of section 3.5, does not report it. An operator learns of
the disagreement only by asking to transfer the path.

This spec amends that approved section 3.5 without editing spec 002. The
amendment edge is the navigable record: `spec-spine registry relationships
002-environment-lifecycle` reports spec 026 as an amendment.

The disagreement is not hypothetical. On 2026-09-26 initialization was found to
re-adopt a governance path the operator had transferred to `user`, because a
profile read option still named the path (backlog SC-004; the corrective
change is not yet integrated). That repair prevents the write; nothing surfaces
an existing record of it, or the next variant of it.

This spec adds that report. It changes no class, no transfer rule, and no write
decision. It is a proposal and authorizes no implementation.

## 2. Territory

This spec owns no product code. A later implementation changes only the units
its `extends` edges name: the doctor module in `statecraft-environment`, the
read-only rendering in `statecraft-home` that `env plan` already computes, and
the `doctor` projection in `statecraft-cli`.

## 3. Behavior

### 3.1 Three facts per path

For each path that appears in any of the three sources below, doctor reads:

1. **recorded**: the manifest entry's class, `managed` or `adopted`, or `user`
   when there is no entry (002 section 3.2);
2. **journaled**: the target class of the latest transfer-journal record for
   the path, where a reversal counts as its inverse move, or `none` when the
   journal has no record for it; and
3. **rendered**: the class the current selection's rendering would give the
   path, computed exactly as `env plan` computes it and writing nothing:
   `managed` when this product would write it, `read-only` when a profile or
   adapter only reads it, or absent when the rendering does not name it.

### 3.2 The finding

The available facts imply incompatible ownership, and
`ownership-disagreement` is reported, whenever a non-`none` journal target
differs from the recorded class; rendered is `managed` while journaled is
`user`; recorded is `adopted` while rendered is `managed`; recorded is
`managed` while rendered is absent; or an existing path has recorded `user`,
journaled `none` and rendered `managed`. The following table enumerates those
cases and their operational consequences:

| Pair | Disagreement |
|---|---|
| recorded, journaled | `journaled` is not `none` and differs from `recorded`. |
| journaled, rendered | `journaled` is `user` and `rendered` is `managed`: the next apply would write a file the operator released. |
| recorded, rendered | `recorded` is `adopted` and `rendered` is `managed`: the selection asks to write a path the manifest says is never rewritten. |
| recorded, rendered | `recorded` is `managed` and the rendering does not name the path: the entry has no source to rewrite it with, yet `env remove` would delete it. |
| recorded, rendered | `recorded` is `user`, `journaled` is `none`, `rendered` is `managed`, and the file exists: the path would be written over bytes the product never recorded. |

The last row overlaps `foreign` where a claimant is named. In the final finding
set, a path that satisfies `foreign` has only that finding; the overlapping
`ownership-disagreement` row is omitted regardless of evaluation order. A
`read-only` rendering never disagrees with any class, because reading is not
ownership. An `adopted` recording disagrees with a `managed` rendering under
the recorded-rendered row. It disagrees with the journal only when a
non-`none` journal target differs from `adopted`. A `read-only` or absent
rendering cannot rewrite or delete an adopted path and is not a disagreement.

Each finding names the path, the pair, and both values with their sources: the
manifest entry, the journal record's identity, or the selection and producer
revision the rendering used. One path may carry more than one pair.

The finding makes doctor exit non-zero, as the findings of section 3.5 do,
because each row predicts a wrong write, a wrong deletion or a refused transfer.

### 3.3 When a fact cannot be read

When the rendering cannot be computed (the producer is unavailable, the
selection does not resolve, or the pin mismatches), doctor reports
`ownership-rendering-unavailable` once, with the reason, and compares only
recorded and journaled. It never assumes the rendering agrees.

A manifest written before spec 002 section 3.35, "Per-path ownership transfer,
as an operator's act," has no journal and reads as having no transfers (that
section's Compatibility paragraph). An entry carrying a `transfer` with no
journal record is reported as it is by `transfer plan`, recorded without a
journal, and is not an `ownership-disagreement` on that ground alone.

### 3.4 Read-only and deterministic

doctor writes no file, manifest or journal byte and performs no transfer,
reconciliation or adoption. The finding does not suggest a repair beyond naming
the verb an operator would use (`transfer plan`, or `env plan`). Findings are
ordered by path, then by pair in the table's order, so the same inputs render
the same report in human and JSON output (spec 006).

## 4. Out of scope

- Integrating the SC-004 initialization repair, which is a corrective change
  under spec 002.
- Any automatic repair, transfer or journal rewrite.
- A change to the three classes, the transfer rules, or what `env apply` writes.
- Ownership of paths outside the target repository.

## 5. Resolved decisions

**2026-09-27: the finding is a finding, not information.** Every row predicts
an operation that would act on the wrong owner's file, which is what section
3.5's non-zero exit exists to surface.

**2026-09-27: `read-only` rendering is never a disagreement.** A profile that
reads an operator's file does not claim it; the SC-004 defect was treating a
read as ownership, and this finding must not repeat it.

## Verification

No implementation acceptance is declared while this spec is unratified. The
`V-1` inputs name the fixture surface a later implementation adds: an isolated
home and fixture repositories exercising each table row, the unavailable
rendering, a legacy manifest, and a tree digest taken before and after doctor.
