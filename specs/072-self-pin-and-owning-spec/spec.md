---
id: "072-self-pin-and-owning-spec"
title: "Setup profile revision 28: the policy-isolation guard ignores a self-pin and the owning spec"
status: approved
implementation: complete
created: "2026-11-07"
summary: >
  Amends spec 044 section 3.3, spec 071 and spec 002's setup profile. A
  repository that pins spec-spine to its own version, as spec-spine does,
  moves spec-spine.toml with every release, so its release bump stays mixed
  under revision 27; and a policy file that a spec claims needs that spec's
  spec.md beside it under the coupling gate, which the guard calls other.
  Revision 28 treats a self-pin move as version-only and a spec.md whose
  front matter names a changed policy path as an ignored owning spec. It
  also drops "branches up to date" from the remote obligations and the
  gate's comments that assumed merge commits.
amends:
  - "002-environment-lifecycle"
  - "044-supply-chain-profile"
  - "071-version-only-cargo-class"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
depends_on:
  - "002-environment-lifecycle"
  - "044-supply-chain-profile"
  - "071-version-only-cargo-class"
obligations:
  - id: "R-1"
    kind: requirement
    text: "spec-spine.toml is version-only when both the merge base and the head hold it, the two copies are equal once the required_version line is masked, and the head's required_version, less a leading `=`, equals the version the head's root Cargo.toml declares in [workspace.package] or [package]."
    anchor: "3-1-a-repository-that-pins-itself"
  - id: "R-2"
    kind: requirement
    text: "A changed spec.md is the owning spec, ignored like a generated path, when its front matter, the lines between an opening `---` and the next `---`, contains a policy path of this change that is not version-only, in double or single quotes; any other spec.md is other."
    anchor: "3-2-the-owning-spec"
  - id: "R-3"
    kind: requirement
    text: "The remote obligations do not require branches up to date, and the profile's gate and workflow comments do not assume merge commits."
    anchor: "3-3-the-remote-obligations-and-the-comments"
  - id: "R-4"
    kind: requirement
    text: "The guard's exits and its merge-base comparison are spec 044's; its summary line also counts the owning-spec paths."
    anchor: "3-4-exits-and-report"
---

# 072: Setup profile revision 28: the policy-isolation guard ignores a self-pin and the owning spec

## 1. Purpose

Spec-spine's assessment of revision 27 (2026-11-07) found two cases the
policy-isolation guard still splits, both from its own governance, and one
stated requirement that contradicts the owner's branch protection:

- **The self-pin.** spec-spine pins `required_version` in `spec-spine.toml` to
  its own release, and moves it in every release bump. Revision 27 excuses
  the Cargo files of a bump but not `spec-spine.toml`, so the bump stays
  mixed with its changelog and the release's spec.
- **The owning spec.** Where a spec claims a policy file (spec-spine's
  `deny.toml` and `spec-spine.toml`), the coupling gate requires that spec's
  `spec.md` beside a change to it, and the guard counts the `spec.md` as
  other. No change to such a file passes both without a waiver.
- **Branches up to date.** The remote obligations ask for it; the owner's
  protection on 2026-11-07 does not, because the merge queue tests each entry
  on the latest base.

Spec 044's intent is kept: a dependency or policy change is never reviewed
inside a feature. A spec document is not a feature, and only the one that
claims the changed file is excused.

## 2. Territory

This spec establishes no new files outside spec 002's directory. It changes
the profile template `scripts/statecraft/guard-policy-files.sh`, two comments
in `scripts/statecraft/gate.sh` and one in `statecraft-ci.yml`, the profile
revision and its obligations in `crates/statecraft-home/src/setup.rs`, and the
tests and the revision-27 fixture under `crates/statecraft-home/tests/`, all
under the `extends` edge on `crates/statecraft-home/`.

## 3. Behavior

### 3.1 A repository that pins itself

When `spec-spine.toml` at the repository root changed, both revisions hold
it, and the two copies are equal once each line of the form
`required_version = "..."` has its value masked, the guard reads the head's
pin, drops one leading `=`, and reads the head's root `Cargo.toml` for the
first `version = "..."` in `[workspace.package]` or `[package]`. When the two
are equal and not empty, the path is version-only (spec 071 section 3.1).

A consumer that pins another tool's version, a pin that moves while any other
line changes, and a file present at only one revision stay policy.

### 3.2 The owning spec

The guard first collects the policy paths of the change that are not
version-only. For each changed path named `spec.md` at any depth that the head
holds, it reads the front matter: the file's first line must be `---`, and
the front matter ends at the next `---` line. When a line in it contains one
of those policy paths exactly, between double quotes or between single
quotes, the `spec.md` is an **owning spec**, and the guard ignores it as it
ignores generated paths.

Everything else is other: a mention in the body, an unquoted or
differently-written path, a directory claim that covers the file, a front
matter that never closes, and a spec that names a path this change does not
change. A spec that claims its file another way still splits, as under
revision 27, so the rule fails closed.

### 3.3 The remote obligations and the comments

The branch-protection obligation names the required `ci-gate` check and
states that branches up to date are not required. The gate script's `text`
and `commits` comments and the workflow's authored-content comment name the
commit a change lands as, not a merge commit. No behavior changes with them.

### 3.4 Exits and report

The exits, the merge-base comparison and the absence of renames are spec 044
section 3.3's. The summary line becomes
`policy-isolation: P policy, G generated (ignored), O other, V version-only (ignored), S owning spec (ignored)`.

## 4. Observable cases

| Case | Required result |
|---|---|
| The workspace version, the member versions and `spec-spine.toml`'s self-pin move together, beside the release's `spec.md` and `CHANGELOG.md` | Exit 0; four version-only paths. |
| `spec-spine.toml`'s pin moves to a version that is not the workspace's, beside a source file | Exit 1; `spec-spine.toml` is policy. |
| The self-pin moves and another line of `spec-spine.toml` changes | Exit 1. |
| `deny.toml` beside a `spec.md` whose front matter quotes `deny.toml` | Exit 0; one owning spec. |
| The same with a source file | Exit 1. |
| `deny.toml` beside a `spec.md` that names it only in its body, unquoted, in an unclosed front matter, or not at all | Exit 1; the `spec.md` is other. |
| Every revision-27 case of spec 071 section 4 and spec 044 section 4 | Unchanged. |

## 5. Resolved decisions

**2026-11-07: owner approval.** The owner, given spec-spine's revision-27
assessment and the three defects above, approved this revision ("Proceed
with draft profile 28 (I approve it) then proceed to build it"). Its
ratification takes effect only with the owner's approval of this change's
`statecraft-review-exception`, as for any spec filed `approved`. A target
upgrades to revision 28 by `statecraft-cli init plan|apply` as for any
revision; this repository's own adoption is a separate change.

**Textual, not the index.** The guard stays POSIX `sh` and `awk` with no
spec-spine call: it runs at the base before any tool is installed, and a
candidate's own corpus cannot be trusted to say which spec owns a file. A
quoted path in the front matter is how spec-spine writes an `establishes` or
`extends` claim, so the textual test matches the claim, and a spec that
claims some other way fails closed.

**The self-pin reads the head's own version.** Comparing the pin with the
head's workspace version, not with any version, keeps a consumer's pin move a
policy change, which `D-06` requires to be its own adoption.

## Verification

Each line is one command.

```verify:cli
cargo test -p statecraft-home --test setup_workflows the_policy_guard
cargo test -p statecraft-home --test setup_upgrade a_revision_twenty_seven_project_upgrades_to_revision_twenty_eight
```
