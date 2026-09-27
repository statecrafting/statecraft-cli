---
id: "023-contained-commit-walk"
title: "Setup profile revision 11: the commit walk runs a contained spec-spine"
status: approved
implementation: complete
created: "2026-09-27"
summary: >
  Amends spec 002's setup profile. github-actions-rust revision 11 copies the
  spec-spine binary into each temporary commit worktree as a regular file
  instead of linking it from outside, so spec-spine's containment rule can
  read every commit's tree. The walk's verdicts are otherwise unchanged.
amends:
  - "002-environment-lifecycle"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
depends_on:
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
  - "017-ai-review-diff-cap"
---

# 023: The commit walk runs a contained spec-spine

## 1. Purpose

Revision 10's commit walk (`gate.sh commits` with
`governance.gate_each_commit`) judged each commit in a temporary git worktree
and gave that worktree a spec-spine by symbolic link: to the checkout's
`.tooling/bin/spec-spine` when the commit pins the head's release, or to a
binary installed under the runner's temporary directory otherwise. Both
targets are outside the worktree. spec-spine's containment rule (its spec 144,
released in 0.28.0) refuses to read a repository through a link that leaves
it, so under a 0.28.0 pin every walked commit was refused before it was
judged. spec-spine pull request 403, adopting revision 10, measured this: its
governance job refused all four pinned commits with "'.tooling/bin/spec-spine'
is a link to ..., outside it (spec 144)". Revision 11 removes the link.

## 2. Territory

This spec changes the registered profile revision, the commit walk in the
profile's `gate.sh` template, and tests under `crates/statecraft-home/`. The
rendered scripts and policy remain managed outputs of spec 002's setup flow.

## 3. Behavior

### 3.1 Each worktree holds its own binary

For every commit the walk judges, the binary it selects (the checkout's own
for the head's pin, or the one installed for another pin) is copied to
`.tooling/bin/spec-spine` inside that commit's temporary worktree, made
executable, and checked to be a regular file and not a link before the gate
runs there. A copy that cannot be made, or that is not a regular file, fails
that commit exactly as a failing gate does. The worktree, and the copy with
it, is removed after the commit is judged.

### 3.2 Nothing else about the walk changes

Which commits are walked, which pin selects which release, the gate and
format check run at each tree, the running (base) `gate.sh` judging every
commit, the refusal of a commit with no exact pin, and the exit codes are
revision 10's. A walk that passed under revision 10 with a pre-0.28.0 pin
passes under revision 11.

### 3.3 Upgrade behavior

The profile becomes revision 11 with a new identity. Only
`scripts/statecraft/gate.sh` changes in body; every other managed file changes
only in its revision and identity header. A revision-10 project upgrades
through the ordinary plan and apply, and a second plan of the same selection
writes nothing.

## 4. Out of scope

- Changing spec-spine's containment rule or any spec-spine pull request.
- Reordering or rebuilding a branch whose early commits carry no exact pin;
  the walk still refuses such a commit, as revision 4 requires.
- Caching one copy across commits, or hard links: a copy per worktree is the
  simplest arrangement the containment rule accepts.

## 5. Resolved decisions

**2026-09-27: copy, not link.** A link inside the worktree to a file inside
the worktree would also be contained, but the binary has to come from
somewhere outside it, so the file itself is placed there. The copy costs one
file write per walked commit and removes any dependence on how spec-spine
resolves links.

## Verification

Each line is one command.

```verify:cli
cargo test -p statecraft-home --test setup_workflows the_commit_walk_runs_a_contained_spec_spine_in_every_worktree
cargo test -p statecraft-home --test setup_upgrade a_revision_ten_project_upgrades_to_revision_eleven
```
