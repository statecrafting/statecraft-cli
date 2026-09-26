---
id: "010-profile-unresolved-claims"
title: "Setup profile revision 9: a project decides whether governance refuses an unresolved claim"
status: draft
implementation: in-progress
created: "2026-09-25"
summary: >
  Amends 002's setup profile (section 5, the setup-profile entry and the
  revision 4 entry). github-actions-rust revision 9 adds one boolean
  parameter, governance.fail_on_unresolved, default true. True renders what
  revision 8 renders: gate.sh governance runs index check with
  --fail-on-unresolved. False omits only that flag: index check still runs,
  names each unresolved claim and fails a stale or invalid index, and every
  other governance step is unchanged. It exists for a corpus that approves a
  spec before building it, where an approved spec's unbuilt claim is
  unresolved by design. Owner decision of 2026-09-25 ("statecraft-cli
  switch"), requested by the travel-memory program for Rahi.
amends:
  # The setup profile's parameter set and its revision. 002 is not edited to
  # record it.
  - "002-environment-lifecycle"
extends:
  # The profile is code in 002's crate (src/setup.rs and the gate.sh
  # template). `amends` does not make this spec an owner of that code (001
  # section 5, the amendment model, Part 1 item 1), so the edge is declared
  # here, in the same change.
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
  # `Parameters` is a grandfathered snake_case document in 006's JSON naming
  # guard, so the new key is named there.
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
  - "006-command-surface"
---

# 010: A project decides whether governance refuses an unresolved claim

## 1. Purpose

Revision 8 of `github-actions-rust` always runs
`spec-spine index check --fail-on-unresolved` in `gate.sh governance`. That is
right for a corpus that builds what it claims in the same pull request, which
is this repository's rule (`AGENTS.md`, "The gate"). It is wrong for a corpus
that approves a spec before building it: Rahi does so by its decision `D-11`,
so each approved but unbuilt spec's `establishes` path is an unresolved claim
by design, and Rahi's adoption pull request (statecrafting/rahi#90) fails
governance on 18 of them and cannot adopt the profile.

The owner decided on 2026-09-25 ("statecraft-cli switch") that the profile
carries a parameter for it. This spec is that parameter. Under the amendment
model (spec `001` section 5, adopted 2026-09-25) a change to what an approved
spec requires is a new spec with an `amends` edge, and this is it.

## 2. Territory

None of its own. The code it changes is `002`'s, reached through the
`extends` edge above: `Parameters`, `parameters()`, `commands_for()`, the
render values and `REVISION` in `crates/statecraft-home/src/setup.rs`; the
`gate.sh` template under `crates/statecraft-home/profiles/github-actions-rust/`;
and the tests in `crates/statecraft-home/`. One line of `006`'s JSON naming
guard (`crates/statecraft-cli/tests/support/json_naming.rs`) names the new key
beside its siblings, because `Parameters` is a grandfathered snake_case
document there.

## 3. Behavior

### 3.1 The parameter

`governance.fail_on_unresolved`, in a project's `project.setup` block, is a
boolean with default `true`. Any other JSON type refuses the plan with
`governance.fail_on_unresolved must be true or false`, as every other boolean
governance parameter does.

### 3.2 What it selects

| Value | `gate.sh governance` runs | Policy `commands.governance[3]` |
|---|---|---|
| `true` (default) | `spec-spine index check --fail-on-unresolved` | `[".tooling/bin/spec-spine", "index", "check", "--fail-on-unresolved"]` |
| `false` | `spec-spine index check` | `[".tooling/bin/spec-spine", "index", "check"]` |

With `false`, only the flag goes:

1. `index check` still runs. A stale or invalid index still fails it, and
   each unresolved claim is still printed (spec-spine's `W-001`), so the
   claim is reported rather than refused.
2. Every other governance step runs exactly as it does with `true`: `check
   --fail-on-warn`, `lint --fail-on-warn`, `index coverage` (with
   `--fail-on-untraced` when `governance.enforce_coverage` is `true`), the
   declared authored-content script and its text mode, the base rule, the
   commit walk (which runs this same `gate.sh governance` at each commit, so
   the parameter applies there too), coupling, and `ci-gate`'s policy.
3. The harness hooks this product delivers (`002` sections 3.22 and 3.23) are
   not the profile and are unchanged: they still read `check
   --fail-on-unresolved` locally.

### 3.3 The revision

The profile becomes revision 9 with a new identity, because the `gate.sh`
template changed. With the default, a re-rendered `gate.sh` behaves as
revision 8's: its bytes differ only in comments, the revision line, the
rendered `FAIL_ON_UNRESOLVED=true` and the branch that reads it. Revision 9
changes no other template; the other rendered files change only where they
print the revision or the identity. It adds no job, no required check and no
remote obligation beyond one operator note naming the parameter. A revision-8 project upgrades by
the managed upgrade `002` already specifies, and a second apply of the same
parameters writes nothing. A re-render is an authority-set change (revision
5), so it needs the owner's exception once, as every re-render does.

### 3.4 Observable negative cases

| Case | Required behavior |
|---|---|
| `governance.fail_on_unresolved` is a string, a number or `null` | The plan is refused with `must be true or false`. |
| `false`, and the index is stale | `gate.sh governance` exits non-zero: `index check` still judges freshness. |
| `false`, and a tracked file breaks the authored-content rules | Refused exactly as with `true`. |
| The parameter is absent | `index check --fail-on-unresolved`, as in revision 8. |

## 4. Out of scope

- This repository's own CI. It keeps the default and is not re-rendered by
  this change; its move to revision 9 is its own authority change.
- Consumer repositories. Rahi re-renders statecrafting/rahi#90 with
  `governance.fail_on_unresolved: false` after this merges; that is Rahi's
  change, not this one.
- The delivered harness hooks (section 3.2 item 3).

## 5. Decisions recorded during implementation

**2026-09-25: the measurement the table in section 3.2 rests on.** spec-spine
0.26.0 (Rahi's pin) and 0.27.0 (this repository's), each run at Rahi main
`bec84199660e` exported into a scratch directory: `check --fail-on-warn`,
`lint --fail-on-warn`, `index coverage` and `index check` each exit 0, and
`index check` prints `index is fresh (18 warning(s), 0 error(s): 18 W-001)`;
`index check --fail-on-unresolved` exits 1 on the same 18. So omitting the
one flag is sufficient for Rahi and no other step needs a parameter. The
tests' spec-spine stub answers `index check` the same way.

**2026-09-25: the parameter is named for the flag it controls.**
`governance.fail_on_unresolved` is the name the owner's request proposed, and
it follows `governance.enforce_coverage`'s pattern of one boolean per refusal.

**2026-09-25: the number `010`.** `007` is taken by the draft in
statecrafting/statecraft-cli#165, and `008` and `009` are named by spec `001`
section 5's proposal for the split of `002`, so this spec takes the next free
number rather than one a pending proposal names.

## Verification

Each line is one command.

```verify:cli
cargo test -p statecraft-home --lib setup::tests::fail_on_unresolved_is_a_boolean_that_defaults_to_true
cargo test -p statecraft-home --test setup_workflows fail_on_unresolved_false_omits_only_that_flag
cargo test -p statecraft-home --test setup_upgrade a_revision_eight_project_upgrades_to_revision_nine
```
