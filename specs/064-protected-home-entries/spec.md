---
id: "064-protected-home-entries"
title: "The protected set is the product home's entries, not the operator's worktrees"
status: approved
implementation: complete
created: "2026-10-29"
summary: >
  Amends spec 004 section 3.18 rule 2. The protected roots a confinement
  takes from the product home are the home's top-level entries, every one
  except a closed operator list (worktrees and Finder's .DS_Store), rather
  than the home as one root. An unknown or later entry is still protected.
  The self-test's sentinel and socket directory live in one fixed protected
  entry. A multiply linked file in a protected entry still refuses.
amends:
  - "004-execution-adapter"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "004-execution-adapter"
obligations:
  - id: "R-1"
    kind: requirement
    text: "The protected roots taken from the product home are its top-level entries except worktrees and .DS_Store; every other entry, named by this product or not, is protected."
    anchor: "3-1-the-protected-roots"
  - id: "R-2"
    kind: requirement
    text: "A multiply linked file inside a protected entry refuses the launch at the hardlink step, as before; a multiply linked file under the operator's worktrees does not."
    anchor: "3-2-what-the-alias-scan-still-refuses"
  - id: "R-3"
    kind: requirement
    text: "The confinement's self-test sentinel and probe socket directory are created inside one fixed protected entry, boundary/, so the roots a confinement record names do not vary by launch."
    anchor: "3-3-a-stable-record"
---

# 064: The protected set is the product home's entries

## 1. Purpose

Spec 004 section 3.18 rule 2 protects "the product home" as one root, and the
boundary's alias scan walks every protected root and refuses any file with
more than one link. Operators keep task worktrees under the product home at
`~/.statecraft/worktrees/`, and the cargo build directories there hold files
cargo links many times over: about 2.6 million on the machine that reported
it. Every launch there refuses at `hardlink`, which blocked a downstream
repository's profile upgrade.

The owner decided on 2026-10-29 to narrow the protected roots to the
product's own entries and to fail closed.

## 2. Territory

This spec amends spec 004 section 3.18 rule 2. Its implementation changes the
launch confinement and the witness sidecar in `statecraft-home` (spec 002's
crate, which spec 004 already extends) and adds tests in `statecraft-cli`.

## 3. Behavior

### 3.1 The protected roots

The product home contributes, as protected roots, each of its top-level
entries except a closed operator list: `worktrees` and `.DS_Store`. An entry
this product never wrote, or adds later, is protected without being listed.
The harness store and the exchange directory remain readable carve-outs as
rule 2 states. Nothing else in rule 2 changes.

### 3.2 What the alias scan still refuses

A multiply linked file inside a protected entry refuses the launch at the
`hardlink` step, as before. A multiply linked file under `worktrees/` is not
scanned and refuses nothing.

### 3.3 A stable record

The self-test's sentinel and its probe socket directory are created inside one
fixed protected entry, `boundary/`, rather than at the top of the home, so the
protected roots a confinement record names, and its digest, do not vary from
one launch to the next.

### 3.4 What is accepted

A confined child may read the operator's worktrees. They hold no evidence and
no authority of this product; the target's working tree and the attempt's
workspaces keep their own rule 2 protection wherever they live.

## 4. Out of scope

- Moving task worktrees out of the product home.
- Any change to the writable roots of rule 3 or to the hard-link rule for
  protected bytes.

## 5. Resolved decisions

**2026-10-29 (owner decision): protect every entry but the operator's.** A
list of the product's entries would fail open on an entry it missed, such as
`credentials/` or `sealed/`; refusing on any unlisted entry would refuse every
launch on a home that holds entries no current code writes (`archive/`,
`evidence/`, `releases/`, `runners/`). Protecting everything but a closed
operator list fails closed without either cost.

## Verification

Ratified by the owner on 2026-10-29. Acceptance is run through
`make verify SPEC=064`, against fixture homes and the platform boundary,
with no provider.

```verify:cli
cargo test -p statecraft-home --lib home::tests::every_home_entry_but_the_operators_is_protected
cargo test -p statecraft-cli --test protected_boundary
cargo test -p statecraft-cli --test application_equivalence
```

These show that an unknown entry is protected and the operator's are not,
that the whole home refuses on cargo's links while its entries do not, that a
link inside a protected entry still refuses, and that a launch's confinement
record is identical across launches.
