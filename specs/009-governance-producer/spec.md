---
id: "009-governance-producer"
title: "Governance producer adoption"
status: approved
implementation: complete
created: "2026-09-26"
summary: >
  Carries the governance producer boundary relocated from spec 002: exact released identity, supported APIs, and the rule that Statecraft consumes spec-spine without becoming a second compiler.
depends_on:
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
relocates:
  - { spec: "002-environment-lifecycle", from: "3-15-the-governance-producer-boundary", to: "3-15-the-governance-producer-boundary" }
---

# 009: Governance producer adoption

## 1. Purpose

Statecraft consumes one exact released spec-spine producer identity and uses only its supported interfaces. Producer adoption is distinct from repository initialization and from harness delivery.

## 2. Territory

The requirement below governs the linked producer, its exact identity, and the boundary between Statecraft and spec-spine. Producer integration code remains in the crates owned by spec 002.

## 3. Behavior

### 3.15 The governance producer boundary

Governance starter files come from the spec-spine **library**, and from nowhere
else. The call is

`spec_spine_core::scaffold_init_json(config_json) -> Result<String, Error>`

returning the existing serialized `Scaffold` files-as-data shape with the
existing `ScaffoldFile` fields. It performs no write and no environment
discovery. The removed `spec-spine init` command is not invoked, no governance
template is vendored into this repository, and there is no fallback installer
built from old kit bytes.

The layout is passed **explicitly**, never defaulted:

```
specs_dir     = "specs"
standards_dir = "standards/spec"
derived_dir   = ".statecraft/derived"
state_dir     = ".statecraft/state"
```

The **contract set** is closed. This product places exactly these, and treats
every other returned path as out of contract:

1. `spec-spine.toml` at the repository root;
2. `<standards_dir>/constitution.md` and `<standards_dir>/contract.md`;
3. `<standards_dir>/templates/**`;
4. `<specs_dir>/000-bootstrap/spec.md`;
5. `.gitignore`, treated as a **fragment**.

An out-of-contract path is **not written**, is named in the report, and makes the
producer's conformance `non-conforming` for that run. That is a finding about the
producer, not a failure of the project: every in-contract path still reconciles.

Reconciliation is `002`'s ownership model and adds nothing to it. A contract path
that does not exist is written and recorded `managed`. A contract path that
already exists is **adopted**: recorded with the digest observed, depended on,
and never rewritten. Existing configuration and authored standards or specs are
not disposable templates.

The `.gitignore` fragment is **merged**: the lines it contributes that are not
already present are appended inside one marked block, and no unrelated entry is
replaced, reordered or removed. Merging is idempotent.

The producer identity (name and exact version) is recorded in the report and in
the declaration's pins. An exact, reproducible dependency is required; a
filesystem path to a sibling checkout is not one, and is never committed.


## 4. Out of scope

- Publishing spec-spine.
- Reimplementing registry, index, lint, coupling, or verification semantics.
- Repository setup-profile rendering, retained by spec 002 and amended by spec 010.

## 5. Resolved decisions

**2026-09-26: producer adoption is one identity.** The CLI pin, linked core, package checksum, and adoption ledger describe one released producer. Development checkouts and unreleased behavior are not admissible producer identities.

## Verification

Each line is one command.

```verify:cli
cargo test -p statecraft-home --test producer_integration
cargo test -p statecraft-cli --test producer_compatibility
```
