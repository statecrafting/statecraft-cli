---
id: "045-one-yaml-reader"
title: "statecraft-home reads YAML through the adopted producer's one reader"
status: approved
implementation: pending
created: "2026-10-21"
summary: >
  Amends specs 002, 012 and 032 so every YAML document statecraft-home
  parses (a project's reusable code workflow, and the default branch's CI
  workflow the remote-state comparison reads) is read by spec-spine's one
  YAML reader, which this product already links, and removes the direct
  serde_yaml dependency. The workspace then carries no unsafe-libyaml, and
  RUSTSEC-2024-0449 no longer applies here.
amends:
  - "002-environment-lifecycle"
  - "012-remote-desired-state"
  - "032-external-code-profile"
# No `extends` yet: the implementing change adds the edge on
# crates/statecraft-home/ (002) with the code, because the gate counts an
# extender as an owner.
depends_on:
  - "002-environment-lifecycle"
  - "009-governance-producer"
  - "012-remote-desired-state"
  - "032-external-code-profile"
obligations:
  - id: "R-1"
    kind: requirement
    text: "statecraft-home parses YAML only through spec_spine_core::types::yaml and depends on no other YAML parser."
    anchor: "3-1-one-reader"
  - id: "R-2"
    kind: requirement
    text: "Every structural verdict the two readers reached under serde_yaml (workflow_call present, jobs nonempty, a needs entry naming a job) is unchanged for every workflow those specs' tests read."
    anchor: "3-2-unchanged-verdicts"
  - id: "V-1"
    kind: verification
    text: "The existing setup and external-code suites pass unchanged in their assertions, and a parity test reads each fixture workflow through the new reader with the verdicts the old one gave."
    anchor: "verification"
    inputs:
      - "crates/statecraft-home/tests/setup_workflows.rs"
      - "crates/statecraft-home/tests/external_code.rs"
---

# 045: statecraft-home reads YAML through the adopted producer's one reader

## 1. Purpose

`statecraft-home` depends on `serde_yaml` 0.9 directly, for two reads: spec
032's structural check of a project's reusable code workflow
(`setup.rs`, `ci.code.workflow` must declare `workflow_call` and nonempty
`jobs`), and spec 012's remote-state comparison, which reads whether the
default branch's aggregate job `needs` a required job (`remote_state.rs`).
`serde_yaml` is deprecated and keeps `unsafe-libyaml` in this workspace, which
RUSTSEC-2024-0449 covers. Spec-spine 0.31.0, adopted under spec 009, moved its
own reading to one YAML reader (its spec 196) written in safe Rust and
configured to read as `serde_yaml` 0.9 did wherever a corpus can tell. This
product already links it through `spec-spine-core`. Using it removes the
dependency and the advisory without adding a crate, and it is the queue's
prerequisite for this repository adopting spec 044's supply-chain job.

## 2. Territory

`crates/statecraft-home/src/setup.rs`, `crates/statecraft-home/src/remote_state.rs`,
the tests that parse workflows under `crates/statecraft-home/tests/`, and
`crates/statecraft-home/Cargo.toml`. No other crate parses YAML.

## 3. Behavior

### 3.1 One reader

Both reads call `spec_spine_core::types::yaml::parse` and inspect its
`YamlValue`. `statecraft-home` declares no YAML parser of its own, so the
reader is the one the adopted producer identity carries, and it moves with
that identity under `D-06`.

### 3.2 Unchanged verdicts

The reader keeps YAML 1.2 booleans, so a workflow's `on:` key stays the
string `on`; it refuses a duplicate key, a second document, an unknown tag
and a non-finite float, each of which `serde_yaml` also refused or never
appeared in a workflow these reads accept. A `workflow_call` written as a
string, a sequence entry or a mapping key is recognized exactly as before; a
`needs` written as a string or a sequence is read exactly as before. A
document the new reader refuses is reported as the same `invalid YAML`
refusal, with the reader's message.

### 3.3 Delivery order

Three changes, each its own pull request: this spec; the code change that
switches both reads and the tests to the producer's reader, adding no
dependency; and a dependency-only change removing `serde_yaml` from
`crates/statecraft-home/Cargo.toml` and the lockfile, which carries nothing
else.

## 4. Out of scope

- Any change to what spec 032 or spec 012 judge.
- Choosing or pinning a YAML parser independently of the adopted producer.
- The supply-chain job itself, which is spec 044's.

## 5. Resolved decisions

**2026-10-21: the producer's reader, not a new direct dependency.**
`serde-saphyr` is already in the lockfile through the producer, but adding it
directly would let this product's YAML reading drift from the producer's
configuration at the next adoption. Reading through
`spec_spine_core::types::yaml` keeps one configuration and adds no edge.

## Verification

Ratified by the owner on 2026-10-31. Acceptance is declared by the
implementing change, which will run the setup and external-code suites with
their assertions unchanged, plus a parity test over each fixture workflow
those suites read, under the new reader.
