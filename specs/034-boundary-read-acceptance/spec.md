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
  - "006-command-surface"
  - "015-managed-session-evidence"
amends_verification:
  - "004-execution-adapter"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: corrective }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: corrective }
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

This amendment corrects the conflicting read expectations in rule 8 and the
section 3.18 hostile-fixture acceptance. It also changes capture project
binding under spec 006 section 3.11.2 and spec 015 sections 3.29 and 3.30.
It adds no writable roots and preserves confinement mechanisms, preflight
refusal, protected evidence access and rule 12 open items.

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

### 3.3 Confined capture project binding

Authorized by the owner on 2026-10-03. A confined `startup capture` executes
in its own writable workspace exported from the trusted source commit, while
its source project remains read-only. The launcher records the canonical
source-project root, source commit and tree separately from its actual working
directory and confinement policy. The provider init event must still report
that exact actual working directory, which must equal the policy workspace.

All three controls must carry source bindings naming the same project, commit
and tree. Qualification requires the source root to match the requested
canonical operator project and its current trusted commit and tree. Different
workspaces are expected; missing, mixed, substituted or mismatched source or
workspace bindings refuse qualification. Existing program, arguments, payload,
version, distinct-session and structured-event requirements remain unchanged.

Legacy captures without confinement or source binding retain their strict
working-directory comparisons. A source binding is never synthesized while
reading an old record. A confined capture without a source binding is refused.

## 4. Acceptance criteria

1. A fixed-probe result distinguishes allowed checkout reads from refused
   checkout writes and refused home and launch-record reads and writes.
2. Hostile fixtures assert allowed reads and refused mutations for the
   checkout and other attempt workspaces, including the alias cases.
3. There is no test-only denial of the checkout sentinel used to make an
   otherwise readable checkout appear inaccessible.

4. Three distinct confined workspaces qualify only against one matching trusted
   source identity; changed source roots, commits, trees or policy workspaces
   refuse before an observation record is written.
5. Legacy directory checks remain strict, and mixed legacy/confined controls
   refuse.

## 5. Resolved decisions

Read-only access supports the existing base-policy and shared-object reads.
Correcting acceptance preserves that direction without widening write access.
This draft records the exact replacement text for owner ratification; the
direction to implement does not itself change the lifecycle label.

## Verification

These commands exercise the provider-neutral fixed probe and the real product
run and acceptance paths with synthetic providers. They require no provider
credentials. Platform-specific probes run only on their matching platform;
passing one platform does not qualify the other.

```verify:cli
cargo test -p statecraft-cli --test protected_boundary --locked
cargo test -p statecraft-cli --test run_startup --locked
cargo test -p statecraft-cli --test contract_binding --locked
cargo test -p statecraft-cli --test qualification_workflow --locked
cargo test -p statecraft-cli --test acceptance_script --locked
```
