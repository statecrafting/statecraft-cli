---
id: "016-bounded-active-specs"
title: "Active specifications stay bounded and historical journals stay outside the corpus"
status: approved
implementation: complete
created: "2026-09-26"
summary: >
  Amends spec 001's authoring model so an active spec carries current
  requirements and concise resolved rationale, never an append-only work log.
  A mechanical gate bounds active files and moves historical journals and
  handoffs to a nonnormative archive outside the compiler input.
amends:
  - "001-boundaries-and-authority"
amends_sections:
  - "5"
extends:
  - { spec: "001-boundaries-and-authority", unit: "scripts/check-authored-content.sh", nature: additive }
establishes:
  - "scripts/check-spec-corpus.sh"
depends_on:
  - "001-boundaries-and-authority"
---

# 016: Active specifications stay bounded

## 1. Purpose

An active specification is a current authority document, not a chronological
record of every implementation choice, measurement, review, or abandoned
proposal. This amendment makes that distinction enforceable so corpus growth
cannot silently recreate an append-only journal.

## 2. Territory

This spec establishes `scripts/check-spec-corpus.sh` and extends spec 001's
authored-content gate only to invoke it. It governs the shape and size of files
under `specs/`. It does not change any product requirement carried by those
files.

## 3. Behavior

### 3.1 Active content and historical content are different records

An active `spec.md` contains the current requirement, its verification, and a
bounded `## 5. Resolved decisions` section for rationale still needed to
interpret the current direction. It does not accumulate work logs, review
transcripts, raw measurements, handoffs, superseded alternatives, or a dated
history of how implementation proceeded.

Historical material worth retaining goes under `docs/decisions/archive/` with
an explicit nonnormative label and provenance. Moving it there preserves the
record without placing it in the compiler's active input or presenting it as a
current requirement.

### 3.2 One active file per spec directory

Every file below `specs/<id>/` is the directory's `spec.md`. Handoffs, reports,
and evidence sidecars are refused. A current requirement belongs in `spec.md`;
a historical record belongs in the archive; generated evidence belongs in the
repository's ignored evidence location.

### 3.3 Mechanical budgets

`scripts/check-spec-corpus.sh` enforces all of the following:

| Surface | Maximum or rule |
|---|---|
| one active `spec.md` | 81,920 bytes and 1,200 lines |
| `## 5. Resolved decisions` | 12,288 bytes and 200 lines |
| retired journal heading | `## 5. Decisions recorded during implementation` is refused |
| spec-directory sidecars | refused |

The limits are guardrails, not targets. Approaching one is a signal to split a
responsibility or remove history, not permission to pad to the boundary.

### 3.4 The normal governance gate runs the check

In tree mode, `scripts/check-authored-content.sh` invokes
`scripts/check-spec-corpus.sh` and propagates a finding. Its `--text` mode stays
limited to authored text supplied by CI, and each script retains an independent
self-test.

## 4. Out of scope

- Rewriting historical test fixtures that intentionally capture an older
  compiled corpus.
- Applying active-spec size limits to the nonnormative archive.
- Deleting historical records merely because they are no longer active.
- Changing spec-spine's compiler, format, or lifecycle semantics.

## 5. Resolved decisions

**2026-09-26: limits are enforced at the active-authority boundary.** The
archive remains searchable and reviewable but is not a compiler input. A hard
81,920-byte and 1,200-line ceiling accommodates the largest coherent current
specs after remediation while preventing a return to the former 550 KB file.

## Verification

Each line is one command.

```verify:cli
scripts/check-spec-corpus.sh
scripts/check-spec-corpus.sh --self-test
scripts/check-authored-content.sh
scripts/check-authored-content.sh --self-test
```
