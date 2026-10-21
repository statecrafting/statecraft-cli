---
id: "044-supply-chain-profile"
title: "Setup profile revision 18: supply-chain, policy-isolation, workflow-lint, structural-scan and secret-scan jobs"
status: draft
implementation: pending
created: "2026-10-21"
summary: >
  Amends spec 002's setup profile. github-actions-rust revision 18 adds five
  required checks to every target: a supply-chain job (cargo-deny,
  cargo-audit, cargo-vet with exact tool versions), a guard that refuses a
  pull request mixing dependency or policy files with anything else, a
  workflow lint (zizmor and an offline refusal of any action not pinned by
  commit digest), an ast-grep scan where the project declares rules, and a
  digest-verified gitleaks scan. Each is wired into ci-gate.
amends:
  - "002-environment-lifecycle"
# No `extends` yet: the implementing change adds the edge on
# crates/statecraft-home/ (002) with the code, because the gate counts an
# extender as an owner.
depends_on:
  - "002-environment-lifecycle"
  - "024-review-budget-and-ratification"
  - "031-engine-in-bin"
obligations:
  - id: "R-1"
    kind: requirement
    text: "Every tool a new job runs is installed at an exact version and verified by digest, or by a commit-pinned action; no job resolves a floating version."
    anchor: "3-1-pinned-tools"
  - id: "R-2"
    kind: requirement
    text: "A pull request that changes a dependency or policy file and any other file fails; the guard compares the merge base with the head and fails closed, exit 2, when it cannot decide."
    anchor: "3-3-the-policy-isolation-guard"
  - id: "R-3"
    kind: requirement
    text: "A workflow step that names an action by tag or branch instead of a 40-hex commit digest fails, decided offline from the workflow text."
    anchor: "3-4-workflow-lint"
  - id: "R-4"
    kind: requirement
    text: "Each new job is a required need of ci-gate, so no job's failure or absence can leave ci-gate green."
    anchor: "3-7-ci-gate-and-the-revision"
  - id: "V-1"
    kind: verification
    text: "Rendering tests show each job in a revision-18 workflow with its pinned tools, the guard's three exits on fixture histories, the pin check's refusals, and a revision-17 project upgrading in one plan."
    anchor: "verification"
    inputs:
      - "crates/statecraft-home/tests/setup_workflows.rs"
      - "crates/statecraft-home/tests/setup_upgrade.rs"
---

# 044: Supply-chain, policy-isolation, workflow-lint, structural-scan and secret-scan jobs

## 1. Purpose

Revision 17 judges the corpus (`governance`), the workspace (`code`) and the
review (`ai-review`, `review-exception`), and names nothing about where the
code's dependencies come from, whether a pull request smuggled a dependency
change inside a feature, whether the workflows themselves run mutable code,
or whether a secret was committed. The owner ruled on 2026-10-20 that
gitleaks joins the profile in this item, and the integration queue names the
rest. Because the profile is the source of every target's managed files, the
checks reach every enrolled repository through one upgrade.

## 2. Territory

This spec changes, under `crates/statecraft-home/`, the registered profile
revision and its job table in `src/setup.rs`, the profile's workflow and
scripts under `profiles/github-actions-rust/`, and the tests that render them.
It adds two profile scripts, `scripts/statecraft/guard-policy-files.sh` and
`scripts/statecraft/verify-action-pins.sh`. A target's rendered copies remain
managed outputs of spec 002's setup flow and are never edited by hand.

Two scripts adapt material from preloop-ci (MIT License, copyright 2026
Preloop contributors): the guard from `supply-chain/guard-policy-files.sh`
and its tests, and the pin rule from `scripts/verify-action-pins.py`. They
are re-implemented, not copied: the guard keeps the exit contract and the
merge-base rule, and the pin check keeps only the rule that a step is pinned
by digest, decided offline. Each rendered script carries the MIT notice for
the part it adapts.

## 3. Behavior

### 3.1 Pinned tools

Every tool a new job runs is fixed by the profile revision, never by the
runner: a commit-pinned action, or a release archive downloaded at an exact
version and refused unless its SHA-256 equals the digest the profile records.
`cargo install` of a tool uses `--locked` and an exact `--version`. Upgrading
a tool is a profile revision.

### 3.2 The supply-chain job

`supply-chain` checks out with full history and no persisted credentials,
removes any pull-request-supplied `.cargo/config.toml`, and runs, each as its
own step whose failure fails the job:

1. `cargo deny check all` when `deny.toml` exists;
2. `cargo audit`, reading `.cargo/audit.toml` when it exists; and
3. `cargo vet --locked` when `supply-chain/config.toml` exists.

A step whose policy file is absent records `not-configured` in the job
summary and passes; whether `deny.toml` may be absent is open (section 7,
O-1). The job never edits a policy file, never writes an exemption, and
treats an advisory as a failure unless the project's own committed policy
ignores it.

### 3.3 The policy-isolation guard

On a pull request, `governance` runs `guard-policy-files.sh <base> <head>`
after fetching both revisions. The guard computes the merge base of the two
and the paths changed between that merge base and the head, never between
the two tips, so a change present on both sides cannot cancel out. A path is
a dependency or policy file when it is `deny.toml`, `.cargo/audit.toml`, any
path under `supply-chain/`, or any `Cargo.toml` or `Cargo.lock` at any depth.

| Exit | Meaning |
|---|---|
| 0 | No dependency or policy file changed, or only such files changed. |
| 1 | Both kinds changed: the pull request must be split. The message lists each kind. |
| 2 | The guard could not decide: bad usage, an unresolvable revision, or no merge base. A failure. |

The guard never fetches, so it cannot hang or half-decide, and it classifies
paths in linear time. A spine adoption under `D-06` moves the `Cargo.toml`
pin with `spec-spine.toml`, the shards and the adoption record in one change;
how that shape passes is open (section 7, O-2).

### 3.4 Workflow lint

`workflow-lint` runs zizmor at an exact version over `.github/workflows/`,
then `verify-action-pins.sh`, which reads each `uses:` line and fails unless
it names a local path (`./`), a `docker://` image by `@sha256:` digest, or
`owner/repo[/path]@<40 lowercase hex>` followed by a `# <ref>` comment. A tag,
a branch, a short digest or a missing comment is a finding naming the file
and line. The check is offline: it does not ask a host whether the comment
matches the digest, which is a different question this revision leaves to
review.

### 3.5 Structural scan

`sg-scan` runs `ast-grep scan` at an exact version when the project commits
an `sgconfig.yml`, and fails on any rule at `error` severity. Without the
file the job records `not-configured` and passes. The rules are the
project's; the profile ships none.

### 3.6 Secret scan

`secret-scan` runs gitleaks from its release archive, verified by digest
(section 3.1), never from an action that needs a license key or network
configuration. On a pull request it scans the commits between the merge base
and the head; on a push, the pushed range. A finding fails the job and the
report names the rule and location, never the matched value. An allowlist is
the project's committed gitleaks configuration; the profile adds none.

### 3.7 ci-gate and the revision

Each of `supply-chain`, `workflow-lint`, `sg-scan` and `secret-scan` is a
required need of `ci-gate`, and the guard is a step of `governance`, which
already is one; none is marked optional, so `jobs.<name>.enabled` refuses for
them as for `code`. The profile becomes revision 18 with a new identity. A
revision-17 project upgrades through the ordinary plan and apply, which adds
the two scripts and rewrites the workflow; a second plan of the same
selection writes nothing.

## 4. Observable negative cases

| Case | Required result |
|---|---|
| A pull request changes `Cargo.lock` and a source file | The guard exits 1, listing both. |
| The base revision was never fetched | The guard exits 2. |
| A change to `deny.toml` exists on both tips | The merge-base diff still sees it. |
| A step uses `actions/checkout@v7` | `workflow-lint` fails, naming the line. |
| A digest pin has no `# <ref>` comment | `workflow-lint` fails. |
| A downloaded gitleaks archive does not match its recorded digest | `secret-scan` fails before running it. |
| A committed advisory is not ignored by the project's policy | `supply-chain` fails. |
| `sgconfig.yml` is absent | `sg-scan` passes and records `not-configured`. |
| A target disables `supply-chain` | The plan refuses, as for any job not marked optional. |

## 5. This repository's own adoption

`statecraft-home` depends on `serde_yaml`, which keeps `unsafe-libyaml` in
this workspace's lockfile, and `cargo audit` reports it (RUSTSEC-2024-0449).
This repository adopts revision 18 only after a separate, spec-first
dependency change moves `statecraft-home` off `serde_yaml`; it does not ignore
the advisory to pass its own check. Its `deny.toml`, `.cargo/audit.toml` and
`supply-chain/` arrive in their own isolated pull request (queue item H3),
each file claimed by its spec.

## 6. Out of scope

- Checking that a pin's `# <ref>` comment matches its digest against a host.
- Dependency update automation, SBOM generation and release provenance (the
  release spec's).
- Any rule an `sgconfig.yml` would contain, and any gitleaks allowlist.
- Writing or relaxing a project's supply-chain policy on its behalf.

## 7. Open questions for the owner

- **O-1.** Whether a target without `deny.toml` passes `supply-chain` with
  `not-configured` (section 3.2), or revision 18 requires one.
- **O-2.** How a `D-06` spine adoption passes the guard (section 3.3): treat
  `spec-spine.toml`, `.statecraft/derived/**` and `docs/adoption/spec-spine.md`
  as policy-class paths, so an adoption is all policy; or split every
  adoption into a `Cargo.toml` pull request and a corpus one, which breaks
  `H-3 (a)`'s one producer identity per change.

## Verification

This spec is `status: draft` and declares no acceptance until it is ratified
and built. Its acceptance will render a revision-18 workflow and assert each
job's pinned tools and its place in `ci-gate`; run the guard against fixture
histories for exits 0, 1 and 2 and the both-tips case; run the pin check
against fixture workflows for each refusal; and upgrade a revision-17 project
in one plan.
