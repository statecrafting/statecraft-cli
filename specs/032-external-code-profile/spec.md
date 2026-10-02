---
id: "032-external-code-profile"
title: "Required project-owned code checks under managed governance"
status: draft
implementation: pending
created: "2026-10-02"
summary: >
  Profile revision 14 keeps Rust as its default and admits an explicitly
  selected external code surface. A contained executable local script and a
  tracked reusable workflow remain required. Rust prerequisites apply only to
  the Rust selection. Governance, coupling, AI review, protected owner
  exceptions and the single required aggregate retain their existing rules.
amends:
  - "002-environment-lifecycle"
  - "018-governed-bootstrap-inputs"
  - "023-contained-commit-walk"
  - "031-engine-in-bin"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: corrective }
depends_on:
  - "002-environment-lifecycle"
  - "018-governed-bootstrap-inputs"
  - "024-review-budget-and-ratification"
  - "031-engine-in-bin"
obligations:
  - id: "I-1"
    kind: invariant
    text: "Selecting an external code surface never disables a required job or weakens governance, coupling, AI review, ratification, owner authority, or aggregate failure rules."
    anchor: "3-2-required-code"
  - id: "R-1"
    kind: requirement
    text: "Revision 14 accepts a closed ci.code selection: rust by default, or external with one tracked reusable workflow and one contained executable local script. Rust-only prerequisites are inapplicable for external code."
    anchor: "3-1-explicit-selection"
  - id: "R-2"
    kind: requirement
    text: "The external script belongs to the base-policy authority set in both workflow detection and aggregate recomputation, runs from the trusted base when present, and its inputs and observations are bound by plan identity."
    anchor: "3-3-authority-and-consent"
  - id: "R-3"
    kind: requirement
    text: "Every-commit governance remains required; its code-format companion is cargo fmt for Rust and the declared external local code check for an external project."
    anchor: "3-4-commit-walk"
  - id: "V-1"
    kind: verification
    text: "Adversarial fixtures prove missing or escaping inputs withhold setup, edited inputs invalidate plan identity, a candidate cannot weaken its own trusted script, and an edited script needs the owner exception. Migration tests prove revision-13 convergence and unchanged default Rust commands."
    anchor: "verification"
    inputs:
      - "crates/statecraft-home/tests/external_code.rs"
      - "crates/statecraft-home/tests/setup_workflows.rs"
      - "crates/statecraft-home/tests/setup_upgrade.rs"
---

# 032: Required project-owned code checks

## 1. Purpose

The fleet upgrade exposed an applicability gap. fact-fold and
travel-memory-compare are Node projects, hqgit is currently specification-only,
and butler-ai requires macOS and Windows suites. Revision 13 requires tracked
Rust toolchain and lock files for every project and runs code on Ubuntu.
These repositories can adopt the exact engine pin while retaining their own
CI, but cannot receive managed governance and review with honest code checks.
The owner requested uniform fleet governance. This draft proposes the bounded
producer capability needed to deliver it; that request does not itself ratify
a new contract.

## 2. Territory

This amendment extends spec 002's home crate, profile templates and tests, and
changes one spec 006 CLI test to recognize that the engine location introduced
in revision 13 remains valid in later revisions. Approved specs stay unedited.
No adopter receives dummy Rust inputs or a skipped required code job.

## 3. Behavior

### 3.1 Explicit selection

Absent `ci.code`, or `{"kind":"rust"}`, selects the existing locked Rust
commands and project-owned `rust-toolchain.toml` and `Cargo.lock` prerequisites.
The alternative has exactly three fields:

```json
{"kind":"external","workflow":".github/workflows/project-code.yml","script":"scripts/project-code.sh"}
```

Unknown keys, an unsupported kind, incomplete fields, shell metacharacters,
absolute paths, traversal, or profile-owned destinations refuse planning.
The workflow is a regular tracked `.yml` or `.yaml` file directly under
`.github/workflows/`, owned by the project, with `on: workflow_call` semantics.
The script is a regular tracked executable inside the project, never a link
or an escaping path. Missing prerequisites withhold the whole profile rather
than generating or copying a check. The exact engine pin, Git work tree and
any declared authored-content checker remain prerequisites for both kinds.

### 3.2 Required code

The external selection renders the required `code` job as a local reusable
workflow call with `contents: read`, no inherited secrets, and no additional
permissions. Its workflow must judge every applicable project code suite,
including platform requirements. All required constituent results must
succeed; a skipped or cancelled applicable suite cannot yield success.
The project owns these suites and their applicability claims.

The profile's aggregate still needs `code` on pull requests, pushes and merge
groups. Failed, cancelled or skipped code blocks it. Local `gate.sh code`
runs the declared script and treats a nonzero result as a finding. Rust keeps
its current command vectors. No owner exception can replace a failed code job.

### 3.3 Authority and consent

The effective selection, workflow bytes, script bytes and prerequisite
observations participate in plan identity. A change requires a fresh plan and
consent; declaration and manifest reconciliation remain unchanged.
Every workflow remains authority. The declared external script additionally
joins the trusted base policy's authority set in both the workflow's exception
detection and `ci-gate.sh` recomputation. Editing or deleting it requires the
protected owner exception for the exact run, even if candidate policy or
workflow detection claims otherwise.

The local gate reads the script's executable Git blob at the supplied base
commit and runs it against the candidate tree. A missing base commit or a
non-executable base blob refuses. First adoption may use the candidate's
script when the base carries none, and says so. Local runs without a supplied
base use the working tree script.

### 3.4 Commit walk

`governance.gate_each_commit` still checks every commit's own governance tree
using the contained engine of spec 023. Its Rust format companion remains
`cargo fmt --all --check`. For external code the declared project check runs
instead, with the same trusted-base script selection. Rust-only checks are
explicitly inapplicable, rather than accidentally failing on absent Cargo.

### 3.5 Revision and upgrade

These changed profile bytes and semantics register revision 14 with a new
content identity. Revision 13 remains a captured migration fixture, never a
second live registration. Ordinary managed reconciliation upgrades clean
files and preserves customized files with their intended copies. A second
unchanged plan writes nothing. Repository-local engine location stays `.bin`.

## 4. Out of scope

Additional permissions, secrets or input maps for reusable code and extra
jobs, Windows installer repair, consented engine-pin movement, bridge
planning, and changes to engine governance semantics remain separate work.
Revision 14 does not claim that tailored projects use identical build verbs.

## 5. Decisions awaiting ratification

The producer extension is proposed separately from the original three draft
ratifications. Owner ratification is required before implementation can land
under `governance.require_ratified`. Preserving code evidence and trusted-base
authority takes precedence over making a profile installation appear complete.

## Verification

```verify:cli
cargo test -p statecraft-home --test external_code
cargo test -p statecraft-home --test setup_upgrade
cargo test -p statecraft-home --test setup_workflows
```
