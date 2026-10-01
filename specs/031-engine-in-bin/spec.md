---
id: "031-engine-in-bin"
title: "Setup profile revision 13: the repository-local engine lives in .bin/"
status: approved
implementation: complete
created: "2026-09-30"
summary: >
  The owner decided that neither spec-spine nor Statecraft uses `.tooling/bin`
  and that the repository-local spec-spine is `.bin/spec-spine`. Profile
  github-actions-rust revision 13 installs the pinned engine there, its gate,
  commit walk, CI cache and policy commands read it there, and its ignore
  fragment ignores `.bin/`. The resolver's repository-local candidate, the
  delivered hooks and this repository's own `Makefile` follow. A revision-12
  project converges through the ordinary managed upgrade.
amends:
  # Approved specs stay unedited; registry relationships reports the edges.
  # 001 section 3.13's dependency row names the install location; 002 owns the
  # setup profile; 010's command table and 023's contained copy spell the
  # path; 024 establishes revision 12 as the registered profile revision,
  # which this amendment supersedes with revision 13.
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
  - "010-profile-unresolved-claims"
  - "023-contained-commit-walk"
  - "024-review-budget-and-ratification"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: corrective }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: corrective }
  # C-01's current statement and D-06's rationale name the install location.
  - { spec: "001-boundaries-and-authority", unit: { kind: file, path: "docs/decisions/00-founding-decisions.md" }, nature: corrective }
depends_on:
  - "002-environment-lifecycle"
  - "024-review-budget-and-ratification"
obligations:
  - id: "R-1"
    kind: requirement
    text: "Revision 13 installs the pinned spec-spine at `.bin/spec-spine`, and every rendered file and policy command that runs the repository-local engine names that path."
    anchor: "3-1-one-location"
  - id: "R-2"
    kind: requirement
    text: "No verb, rendered file or delivered hook of this product reads `.tooling/bin`."
    anchor: "3-1-one-location"
  - id: "R-3"
    kind: requirement
    text: "A revision-12 project upgrades to revision 13 through the ordinary managed upgrade, and a second plan of the same selection writes nothing."
    anchor: "3-3-upgrade"
  - id: "V-1"
    kind: verification
    text: "The revision-12 to revision-13 upgrade replaces the three templates that named `.tooling/bin`, and spec-owned resolver and hook fixtures prove selection of `.bin/spec-spine`. The implementation adds the captured revision-12 fixture before this verification runs."
    anchor: "verification"
    inputs:
      - "crates/statecraft-home/tests/setup_upgrade.rs"
      - "crates/statecraft-home/tests/support/profile-r12/"
      - "crates/statecraft-home/src/spec_spine.rs"
      - "crates/statecraft-home/tests/harness_hooks.rs"
      - "crates/statecraft-cli/tests/engine_in_bin.rs"
---

# 031: The repository-local engine lives in .bin/

## 1. Purpose

The owner's decision, 2026-09-30: neither spec-spine nor Statecraft should use
`.tooling/bin`; the repository-local engine belongs in `.bin/`.

Revision 12 installs the pinned engine with `cargo install --root .tooling`,
so the executable is `.tooling/bin/spec-spine`, and names that path in its gate
script, its commit walk's contained copy, its CI cache, its policy commands and
its ignore fragment. The product's repository-local resolution and the four delivered hooks read
it there as the repository-local candidate. spec-spine's launcher (its spec 188)
now reads its project tool directory at `.bin/` only, so the two would disagree
about where a project's own engine is.

## 2. Territory

This spec changes the registered profile revision, the templates of `gate.sh`,
`install-spec-spine.sh` and `statecraft-ci.yml`, the policy's command vectors,
the ignore fragment, the producer's resolver exclusion for the tool directory,
the resolver's repository-local candidate, the four delivered hooks, and tests
under `crates/statecraft-home/` and `crates/statecraft-cli/`. It also moves this
repository's own `Makefile`, `.gitignore`, `spec-spine.toml` exclusion and
rendered profile files to revision 13, and the documents that state the
location. Spec 023 is amended because its requirement names the contained
copy's destination. The implementation unit that renders and tests that copy
remains `crates/statecraft-home/`, owned by spec 002 and named by this spec's
`extends` edge. Spec 010 is amended because its command table names the policy
vectors' executable path. Those vectors are likewise rendered by
`crates/statecraft-home/`; spec 010 owns no separate implementation unit.
Spec 024 is amended because it establishes revision 12 as the registered
profile revision; this spec supersedes that claim with revision 13.

## 3. Behavior

### 3.1 One location

The repository-local engine is `.bin/spec-spine`, a regular executable file in
a directory the profile's ignore fragment ignores. Revision 13's installer
reads the exact pin as revision 12's did, runs `cargo install` into a scratch
root outside the repository, and moves the one executable into `.bin/` through
a temporary name, so no cargo install record is left in the repository. The
gate script, the commit walk's contained copy (spec 023), the CI cache path and
every policy command vector name `.bin/spec-spine`.

The resolver's repository-local candidate and the delivered hooks' candidate
list name `.bin/spec-spine`. `.tooling/bin` is not
a candidate anywhere. The producer's resolver exclusions name `.bin` in place
of `.tooling`.

### 3.2 This repository

`make tools` runs the rendered `scripts/statecraft/install-spec-spine.sh`, so
a local session and CI install the same way. `SPEC_SPINE_LOCAL` is
`.bin/spec-spine`. The rendered files are revision 13's, applied by `init
apply` from the implementation.

### 3.3 Upgrade

The three changed templates are managed files, so a revision-12 project's
upgrade replaces them when their bytes are the ones revision 12 recorded and
leaves a customized one with its intended copy, as every earlier revision.
Every other rendered file changes only in its revision header. The ignore
fragment's `.bin/` line is merged beside the old `.tooling/` line, which is the
user's to remove.

In CI, a pull request that upgrades a project is judged by its base's gate and
installer (revision 12, `.tooling/bin`), which agree with each other; the
head's workflow caches `.bin/spec-spine`, which that run does not write, so the
cache misses once. After the merge both halves are revision 13.

## 4. Out of scope

- spec-spine's own repository, whose gate files are rendered by this profile
  and move when it adopts revision 13.
- Removing a `.tooling/` directory left in a checkout. Nothing reads it; the
  operator deletes it.

## 5. Resolved decisions

**2026-09-30: fixed, not configured.** The earlier review recommended a
`[layout]` key for the tool directory. The owner chose one fixed path, and
spec-spine's launcher dropped its `tool_dir` lock key to match, so the two
tools cannot disagree about it.

**2026-09-30: install through a scratch root.** `cargo install --root .bin`
would place the executable at `.bin/bin/spec-spine` and write cargo's install
record beside it. Installing into a temporary root and moving the executable
keeps `.bin/` to the one file the resolver reads.

**2026-09-30: `make tools` runs the rendered installer.** Revision 12's
`Makefile` spelled its own `cargo install` line, a second definition of the
install that could drift from the one CI runs.

## Verification

The implementation first captures revision 12's changed template bytes under
`tests/support/profile-r12/`, as earlier profile upgrades captured their
predecessors. Spec 024's profile revision 12 is already implemented; its
`implementation: in-progress` state records the separate live permission
experiment and does not withhold those shipped template bytes.

```verify:cli
cargo test -p statecraft-home --test setup_upgrade a_revision_twelve_project_upgrades_to_revision_thirteen
cargo test -p statecraft-home --lib spec_spine
cargo test -p statecraft-home --test harness_hooks contract_2_the_repository_local_engine_is_in_bin
cargo test -p statecraft-cli --test engine_in_bin
```
