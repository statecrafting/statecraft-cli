---
id: "073-new-crate-class"
title: "Setup profile revision 29: the policy-isolation guard ignores a new workspace crate that adds no dependency"
status: draft
implementation: pending
created: "2026-11-24"
summary: >
  Amends spec 044 section 3.3, specs 071 and 072, and spec 002's setup
  profile. A workspace whose members are a glob picks up a new crate's
  Cargo.toml the moment it lands, so the manifest cannot land before its
  sources (the build breaks) and cannot land with them (the guard reads
  policy plus other). Every new crate has needed an admin merge. Revision 29
  adds a class, new-crate: a Cargo.toml in a directory that is new at the
  head, whose every dependency inherits from the workspace or is a path to a
  workspace crate, and a Cargo.lock whose only change is that crate's
  sourceless entry and the dependency lists that reference it. Like
  generated paths it is ignored. Any added, removed or changed registry or
  git package keeps the file policy, so a dependency still lands alone.
amends:
  - "002-environment-lifecycle"
  - "044-supply-chain-profile"
  - "071-version-only-cargo-class"
  - "072-self-pin-and-owning-spec"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
depends_on:
  - "002-environment-lifecycle"
  - "044-supply-chain-profile"
  - "071-version-only-cargo-class"
  - "072-self-pin-and-owning-spec"
obligations:
  - id: "R-1"
    kind: requirement
    text: "A Cargo.toml that the merge base does not hold, whose directory holds no path at the merge base, and whose every table and dependency line is one section 3.1 admits, is new-crate; the guard counts it in neither policy nor other."
    anchor: "3-1-the-new-crate-manifest"
  - id: "R-2"
    kind: requirement
    text: "A Cargo.lock that both revisions hold is new-crate when, after revision 27's mask, its header and every block with a source are unchanged, no sourceless block is removed, each existing sourceless block differs at most in its dependencies list, and each added block is sourceless and named by a new-crate manifest of this change."
    anchor: "3-2-the-new-crate-lockfile"
  - id: "R-3"
    kind: requirement
    text: "Anything section 3.1 or 3.2 does not admit keeps the file policy: a registry, git or alternate-registry dependency, a new or changed package with a source, a workspace, patch, replace or profile table, a links key, a dependency written in a shape the guard does not recognise, a reused directory, and a path git quotes."
    anchor: "3-3-what-stays-policy"
  - id: "R-4"
    kind: requirement
    text: "The guard's exits and its merge-base comparison are spec 044's; a new-crate path is excluded from the policy paths an owning spec may name; its summary line also counts the new-crate paths."
    anchor: "3-4-exits-report-and-the-owning-spec"
---

# 073: Setup profile revision 29: the policy-isolation guard ignores a new workspace crate that adds no dependency

## 1. Purpose

Spec 044 section 3.3 classes every `Cargo.toml` and `Cargo.lock` as policy so
that a dependency change is never reviewed inside a feature. Spec 071 excused
a version bump and spec 072 a self-pin and the owning spec; spec 071 section 4
kept "a new crate's `Cargo.toml` beside a source file" at exit 1 on purpose,
because nothing then distinguished a crate from a dependency.

That row is now the defect. A workspace that declares its members as a glob
(`members = ["crates/*"]`, the form both this repository and aicortex use)
makes a new crate's manifest a member as soon as it lands:

- landed alone, the manifest names a crate with no `src/lib.rs`, so the
  workspace no longer builds and every required check fails;
- landed with its sources, the manifest and the lockfile are policy and the
  sources are other, so the guard exits 1.

There is no third order, so every new crate needs an admin merge. aicortex
pull requests #126 (`crates/aicortex-graph`) and #127 (`crates/aicortex-mcp`)
are the observed instances, under revision 28.

**The security argument.** The guard exists so that a dependency, the thing
cargo-deny, cargo-audit and a human dependency review judge, is reviewed on
its own and never buried in a feature diff. A crate whose every dependency is
inherited from `[workspace.dependencies]` or is a path to another workspace
crate introduces no dependency: every version requirement it uses was
reviewed when the workspace declared it, and the lockfile proves the
resolution by adding no package with a source. The crate's own code is other
and is reviewed as code. The class excuses exactly that, by a mechanical
comparison, and nothing wider; a new crate that needs a new third-party
package still lands its dependency in its own policy pull request first.

## 2. Territory

This spec establishes no new files outside spec 002's directory. It changes
the profile template
`crates/statecraft-home/profiles/github-actions-rust/scripts/guard-policy-files.sh`
(rendered into a target as `scripts/statecraft/guard-policy-files.sh`), the
profile revision and its obligations in `crates/statecraft-home/src/setup.rs`,
and the tests and a revision-28 fixture under `crates/statecraft-home/tests/`,
all under the `extends` edge on `crates/statecraft-home/`.

## 3. Behavior

### 3.1 The new-crate manifest

For each changed path named `Cargo.toml` at any depth other than the
repository root, that git did not quote, and that revision 27 did not already
class version-only, the guard tests three things.

1. **The directory is new.** The merge base holds no `Cargo.toml` at that
   path, and `git ls-tree` at the merge base lists no path under the
   manifest's directory. A crate made out of an existing directory, a
   vendored tree included, does not qualify.
2. **Every table is admitted.** A table header, after spaces and a trailing
   comment are removed, is one of: `[package]`, `[package.metadata]` or any
   `[package.metadata.*]`, `[lib]`, `[[bin]]`, `[[test]]`, `[[bench]]`,
   `[[example]]`, `[features]`, `[lints]`, `[badges]`, a dependency table
   (`[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`), or a
   dependency table under `[target.<spec>.]`. Inside `[package]`, a `links`
   key is not admitted.
3. **Every dependency line is admitted.** Inside a dependency table each
   non-blank, non-comment line is one of these single-line shapes:
   - `name.workspace = true`;
   - `name = { workspace = true, ... }` whose other keys are only `features`,
     `optional` and `default-features`;
   - `name = { path = "<relative>", ... }` whose other keys are only
     `version`, `features`, `optional`, `default-features` and `package`, with
     no `git`, `branch`, `tag`, `rev`, `registry` or `registry-index` key,
     whose path, resolved against the manifest's directory and normalised,
     stays inside the repository, and whose target directory holds, at the
     head, a `Cargo.toml` that is either unchanged since the merge base or
     itself a candidate of this section.

When all three hold, the manifest is **new-crate**. A path dependency on
another new manifest needs no fixed point: if that manifest fails its own
test it is policy, and the change splits anyway.

### 3.2 The new-crate lockfile

For each changed path named `Cargo.lock` that both revisions hold, that git
did not quote, and that revision 27 did not already class version-only, the
guard applies revision 27's mask (spec 071 section 3.1) to both copies, so a
new crate inside a version bump is still one change, and splits each into its
header (everything before the first `[[package]]`) and its `[[package]]`
blocks. The lockfile is **new-crate** when:

- the two headers are equal, so the lockfile format and any `[metadata]` are
  untouched;
- the blocks that have a `source` line are the same set, textually, at both
  revisions: no registry or git package is added, removed, re-versioned or
  re-checksummed;
- every sourceless block of the merge base is present at the head with the
  same `name`, and differs at most in its `dependencies = [...]` list; and
- every block the head adds is sourceless, and its `name` is the `[package]`
  `name` of a manifest this change classes new-crate.

A dependency list may then reference only packages the lockfile already
locked or the new workspace crates. In particular, a new crate that inherits
a `[workspace.dependencies]` entry no member used before makes cargo lock a
new registry package, and the lockfile stays policy: inheriting a declaration
is not the same as already depending on it, and the guard follows the lock.

### 3.3 What stays policy

The class fails closed. Each of these keeps its file policy, as under
revision 28:

- a dependency written as a version string, a multi-line inline table, a
  `[dependencies.<name>]` subtable, or any other shape 3.1 does not list;
- a `git`, alternate-registry or out-of-repository path dependency;
- a `[workspace]`, `[patch.*]`, `[replace]` or `[profile.*]` table, or a
  `links` key, in the new manifest;
- a new manifest in a directory the merge base already holds a path in, the
  root manifest, and a deleted or moved manifest;
- a lockfile that adds, removes or changes a block with a source, changes its
  header, removes a sourceless block, or adds a sourceless block that no
  new-crate manifest names;
- a nested `Cargo.lock` that is new at the head; and
- any path git had to quote.

The comparison is textual and deterministic, POSIX `sh` and `awk` linear in
the files, as specs 071 and 072 decided; it never runs cargo and never
fetches.

### 3.4 Exits, report and the owning spec

The exits, the merge-base comparison and the absence of renames are spec 044
section 3.3's. A new-crate path is excluded from the policy paths that spec
072 section 3.2 lets an owning `spec.md` name, as a version-only path is. The
summary line becomes
`policy-isolation: P policy, G generated (ignored), O other, V version-only (ignored), S owning spec (ignored), N new crate (ignored)`.

### 3.5 Observable cases

Each case is a fixture repository built by the test, with a glob-member
workspace root and one existing crate that depends on one registry package.

| Case | Required result |
|---|---|
| A new `crates/b/Cargo.toml` whose dependencies are `workspace = true` and a path to `crates/a`, `crates/b/src/lib.rs`, and `Cargo.lock` adding a sourceless `b` and its dependency list | Exit 0; two new-crate paths. |
| The same crate, with a dependency the workspace already locks reached through `workspace = true` | Exit 0. |
| The same crate, with `serde = "1"` | Exit 1; the manifest and the lockfile are policy. |
| The same crate, with `x = { workspace = true }` where `x` is declared in `[workspace.dependencies]` but no member used it, so `Cargo.lock` adds a registry block | Exit 1; the lockfile is policy, the manifest is new-crate. |
| A `Cargo.lock` that adds a registry block beside a source file, with no new manifest | Exit 1. |
| A new manifest with a `git` dependency, a `[patch.crates-io]` table, a `[dependencies.x]` subtable, or a `links` key | Exit 1. |
| A new manifest in a directory the merge base already holds a file in | Exit 1. |
| A new manifest at a path git quotes | Exit 1; the path stays policy. |
| A new crate inside a release bump (spec 071 section 4, first row) | Exit 0. |
| A new crate beside a `spec.md` whose front matter claims the crate's directory | Exit 0; no policy path remains. |
| Every revision-28 case of spec 072 section 4, spec 071 section 4 except its "new crate" row, and spec 044 section 4 | Unchanged. |

## 4. Out of scope

- A root `Cargo.toml` that lists members explicitly. Adding a member edits the
  root manifest, which stays policy; such a workspace splits the root edit
  from the crate, or moves to a glob. Excusing a `members` edit is a separate
  amendment if an adopter needs it.
- Removing or renaming a crate.
- Adopting revision 29 in this repository or in any target; each adoption is
  its own change, as for every revision.

## 5. Resolved decisions

**2026-11-24: owner direction.** The owner asked for a draft change so that a
new workspace crate lands in one pull request without an admin merge, keeping
dependency review isolated, with aicortex #126 and #127 as the evidence. This
spec is that draft. Ratification is the owner's act; an implementation
follows only once it is approved.

**The lockfile is the backstop, the manifest the first test.** A manifest can
only state requirements; the lockfile states what cargo resolved. Requiring
both means a manifest that slips past 3.1 through an inherited but unused
declaration, a feature that enables a dependency's optional package, or a
path to a crate outside the workspace still fails on the lock, because each
of those adds a block with a source.

## Verification

Each line is one command.

```verify:cli
cargo test -p statecraft-home --test setup_workflows the_policy_guard
cargo test -p statecraft-home --test setup_upgrade a_revision_twenty_eight_project_upgrades_to_revision_twenty_nine
```
