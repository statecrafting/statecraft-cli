---
id: "071-version-only-cargo-class"
title: "Setup profile revision 27: the policy-isolation guard ignores a workspace version bump"
status: approved
implementation: complete
created: "2026-11-07"
summary: >
  Amends spec 044 section 3.3 and spec 002's setup profile. The
  policy-isolation guard classes any Cargo.toml or Cargo.lock as policy, so
  a release bump, which moves the workspace version in both beside its
  changelog and re-blessed fixtures, always reads as mixed and has needed an
  admin merge. Revision 27 adds a fourth class, version-only: a Cargo file
  present at both revisions that is unchanged once every workspace package's
  version is masked. Like generated paths it is ignored. A dependency,
  source, lockfile-format or new-file change keeps the file policy.
amends:
  - "002-environment-lifecycle"
  - "044-supply-chain-profile"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
depends_on:
  - "002-environment-lifecycle"
  - "044-supply-chain-profile"
  - "068-finding-severity"
obligations:
  - id: "R-1"
    kind: requirement
    text: "A Cargo.toml or Cargo.lock that both the merge base and the head hold, and whose two copies are equal once every workspace package's version is masked, is version-only; the guard counts it in neither policy nor other."
    anchor: "3-1-the-version-only-class"
  - id: "R-2"
    kind: requirement
    text: "Any other change to a Cargo.toml or Cargo.lock keeps it policy: a registry dependency's version, a source, the lockfile format, an added or deleted file, or a path git quotes."
    anchor: "3-2-what-stays-policy"
  - id: "R-3"
    kind: requirement
    text: "The guard's exits and its merge-base comparison are spec 044's; its summary line also counts the version-only paths."
    anchor: "3-3-exits-and-report"
---

# 071: Setup profile revision 27: the policy-isolation guard ignores a workspace version bump

## 1. Purpose

Spec 044 section 3.3 puts every `Cargo.toml` and `Cargo.lock` in the policy
class, so that a dependency change is never reviewed inside a feature. A
release bump moves no dependency: it changes the workspace version in
`Cargo.toml` and the workspace packages' versions in `Cargo.lock`, beside a
changelog, `package.json` versions and re-blessed fixtures. Under revision 26
that pull request is always mixed (exit 1), so each release needed an admin
merge or a split into two pull requests that each fail their own checks. A
rustev release was held about five hours on it.

The fix keeps 044's intent exactly: a file is excused only when a mechanical
comparison proves the bump is its whole change.

## 2. Territory

This spec establishes no new files outside spec 002's directory. It changes
the profile template `scripts/statecraft/guard-policy-files.sh`, the profile
revision and its obligations in `crates/statecraft-home/src/setup.rs`, and
the tests and the revision-26 fixture under `crates/statecraft-home/tests/`,
all under the `extends` edge on `crates/statecraft-home/`.

## 3. Behavior

### 3.1 The version-only class

For each changed path named `Cargo.toml` or `Cargo.lock` at any depth, the
guard reads the file at the merge base and at the head and masks the
version of every workspace package:

- in `Cargo.toml`, the `version = "..."` line of a `[package]` or
  `[workspace.package]` table, and the `version = "..."` of an inline table
  that also names a `path` (a workspace or path dependency);
- in `Cargo.lock`, the `version = "..."` line of a `[[package]]` block that
  has no `source` line (a workspace member).

When the two masked copies are equal, the path is **version-only**. The
guard ignores it, as it ignores generated paths: policy plus version-only
passes, other plus version-only passes, and only policy plus other fails.

### 3.2 What stays policy

A registry dependency's version (an inline table without `path`, or a
`[[package]]` with a `source`), any other line, the lockfile's top-level
`version`, a file present at only one of the two revisions, and a path git
had to quote all keep the file in the policy class. The comparison is
textual and deterministic; it never runs cargo and never fetches.

### 3.3 Exits and report

The exits, the merge-base comparison and the absence of renames are spec
044 section 3.3's. The summary line becomes
`policy-isolation: P policy, G generated (ignored), O other, V version-only (ignored)`.

## 4. Observable cases

| Case | Required result |
|---|---|
| The workspace version moves in `Cargo.toml`, a member `Cargo.toml` and `Cargo.lock`, beside `CHANGELOG.md` | Exit 0, three version-only paths. |
| The same, with a registry dependency's version moved too | Exit 1; `Cargo.toml` and `Cargo.lock` are policy. |
| `Cargo.lock`'s format `version` changes beside a source file | Exit 1. |
| A new crate's `Cargo.toml` beside a source file | Exit 1. |
| Every revision-26 case of spec 044 section 4 | Unchanged. |

## 5. Resolved decisions

**2026-11-07: owner direction.** The owner directed that a version bump no
longer need an admin merge, that a release cycle bump the version when it
opens, and pre-approved the spec that amends spec 044 to do so ("I also
pre-approve the draft of spec 044"); this spec is that amendment. Its
ratification takes effect only with the owner's approval of this change's
`statecraft-review-exception`, as for any spec filed `approved`. A target
upgrades to revision 27 by `statecraft-cli init plan|apply` as for any
revision; this repository's own adoption is a separate change.

**Masking, not parsing.** The guard stays POSIX `sh` and `awk`, linear in the
file. It recognises only the three line shapes in 3.1; anything it does not
recognise stays policy, so an unusual layout fails closed into the split
that revision 26 already required.

## Verification

Each line is one command.

```verify:cli
cargo test -p statecraft-home --test setup_workflows the_policy_guard
cargo test -p statecraft-home --test setup_upgrade a_revision_twenty_six_project_upgrades_to_revision_twenty_seven
```
