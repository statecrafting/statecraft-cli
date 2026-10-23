---
id: "058-ci-throughput-profile"
title: "Setup profile revision 18: superseded pull-request runs are cancelled, a preflight orders the expensive jobs, and the cargo cache is saved from the default branch only"
status: draft
implementation: in-progress
created: "2026-10-23"
summary: >
  Amends spec 002's setup profile. github-actions-rust revision 18 changes
  when work runs, never what judges it: a newer push to a pull request
  cancels that pull request's older run; a preflight job (the base, the
  committed artifacts' freshness, formatting) runs before code and every
  declared extra job, so a stacked pull request skips them unless it asks
  for them by label; and the code job restores the cargo cache on every event
  but saves it only from the default branch. Every check keeps running where
  it ran; ci-gate's rules are unchanged except that preflight is required.
amends:
  - "002-environment-lifecycle"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
depends_on:
  - "002-environment-lifecycle"
  - "032-external-code-profile"
obligations:
  - id: "R-1"
    kind: requirement
    text: "A pull_request run is in a concurrency group of its pull request and cancels an older run in it; a push and a merge_group run each have a group of their own and cancel nothing."
    anchor: "3-1-superseded-runs-are-cancelled"
  - id: "R-2"
    kind: requirement
    text: "code and every declared extra job need preflight; ci-gate needs preflight and the policy requires it on every event, so its failure, cancellation or skip blocks."
    anchor: "3-2-preflight"
  - id: "R-3"
    kind: requirement
    text: "Every check preflight runs also runs in governance or code; a base gate.sh without the preflight verb makes the step pass and say so, and governance and code still judge everything."
    anchor: "3-2-preflight"
  - id: "R-4"
    kind: requirement
    text: "On a pull_request whose base is not the default branch, preflight fails unless the pull request carries the label statecraft:stacked-ci; the label never changes governance's base refusal."
    anchor: "3-3-stacked-pull-requests"
  - id: "R-5"
    kind: requirement
    text: "The code job restores the cargo cache on every event and saves it only on a push, which the workflow receives only for the default branch."
    anchor: "3-4-the-cargo-cache"
  - id: "V-1"
    kind: verification
    text: "Rendering and step tests show the concurrency group per event, the needs on preflight, preflight in the policy and in ci-gate, the stacked refusal and its label, the adoption pass, the cache split, and a revision-17 project upgrading in one plan."
    anchor: "verification"
    inputs:
      - "crates/statecraft-home/tests/setup_workflows.rs"
      - "crates/statecraft-home/tests/setup_upgrade.rs"
---

# 058: CI throughput: cancelled superseded runs, a preflight, and a default-branch cache

## 1. Purpose

An adopter measured where its pull requests spend their time (rustev,
2026-10-23, 19 runs of `statecraft-ci`). Three costs came from the profile and
not from the project:

- **Superseded runs kept running and kept waiting.** Every push started a full
  run; nothing ended the previous one. 588 runs of superseded heads were
  waiting on the owner-exception Environment at once, which buried the few
  approvals that mattered.
- **Stacked pull requests paid for the whole matrix.** A pull request whose
  base is not the default branch fails `governance` by design (revision 4,
  rule 5), yet `code` and every declared extra job ran to completion for it.
- **The cargo cache cost more than it saved.** Each pull request saved its own
  multi-gigabyte entry, readable by no other pull request. Those saves evicted
  the default branch's entry under the repository's 10 GB limit, so restores
  missed, and the save step took a median of 138 seconds (at most 268) in
  `code` on every run.

Revision 18 changes when and where work runs. It removes no check, and it
changes no rule about what makes `ci-gate` green, except that the new
`preflight` job is required.

## 2. Territory

The registered profile's `statecraft-ci.yml` template and `gate.sh`, its
revision and policy job table, the renderer's extra-job and external-code
text, and tests under `crates/statecraft-home/`. Rendered files remain
managed outputs of spec 002's setup flow.

## 3. Behavior

### 3.1 Superseded runs are cancelled

The workflow declares one concurrency group per pull request for
`pull_request` runs (`statecraft-ci-pr-<number>`), with `cancel-in-progress`
true for that event only. A newer push cancels the older run, including one
waiting on the owner-exception Environment. Cancelling an older run is
harmless to ci-gate's rules: it judges the head, and the cancelled run judged a
head that is gone.

A `push` run and a `merge_group` run each get a group named by their own run
id. Neither is cancelled or queued behind another, so the default branch
and every queue entry keep their own run.

### 3.2 Preflight

A `preflight` job runs `gate.sh preflight`, read at the base like every other
gate verb (revision 5, rule 1). It judges:

1. on `pull_request`, the base, as `gate.sh base` does (section 3.3);
2. the committed artifacts' freshness, `spec-spine check --fail-on-warn`;
3. for a Rust code selection with a `Cargo.toml`, `cargo fmt --all --check`.

`code` and every job declared in `ci.extra_required_jobs` need `preflight`.
An external code selection's `code` job needs it too. `governance` and the AI
review do not wait for it. `ci-gate` needs `preflight`, and the policy requires
it on every event. A failed, cancelled or skipped `preflight` blocks, and so
does the skip it causes in the jobs that need it.

Preflight orders the checks; it does not own them. `governance` still runs
the base rule and `spec-spine check`, and `code` still runs `cargo fmt`. A
base whose `gate.sh` has no `preflight` verb is the adoption of this revision:
the step says so and passes, and the full judgment still happens in
`governance` and `code`. The policy's `commands` record the preflight
selection beside `governance` and `code`.

### 3.3 Stacked pull requests

On a `pull_request` whose base is not the default branch, `preflight` fails,
so `code` and the extra jobs are skipped. A stacked pull request that wants
the full matrix anyway, for early feedback, carries the label
`statecraft:stacked-ci`. Preflight then reports the request and judges
freshness and formatting only. The label is a scheduling request, never a
judgment: `governance` still refuses the base, so a stacked pull request
still cannot merge into the default branch with a green `ci-gate`. A label
added after a push takes effect on the next run.

With `governance.require_default_base` false, the base is not judged anywhere,
and preflight does not judge it either.

### 3.4 The cargo cache

The `code` job restores `~/.cargo/registry`, `~/.cargo/git` and `target` under
the existing key on every event (`actions/cache/restore`). It saves under that
key (`actions/cache/save`) only on a `push`, which the workflow receives only
for the default branch, and only when the key missed. Pull requests then read
the default branch's entry instead of writing their own. The job sets
`CARGO_INCREMENTAL=0`, because a fresh runner never reuses incremental state,
and `CARGO_PROFILE_DEV_DEBUG=line-tables-only`, which keeps file and line in a
panic at a fraction of the debuginfo. No command the policy records changes.

### 3.5 The revision

The profile becomes revision 18 with a new identity. A revision-17 project
upgrades through the ordinary plan and apply. The rendered workflow and
`gate.sh` are replaced, and `preflight` enters the policy's job table and
`commands`. The upgrade pull request is an authority change, judged by the
base's revision-17 `ci-gate`, and it needs the owner's exception once.
Section 3.2's adoption rule keeps its own preflight from refusing it.

## 4. Out of scope

- Splitting or rebalancing a project's own acceptance jobs.
- Removing a check from `governance` or `code` because preflight ran it first.
- Enabling a merge queue, which stays a remote setting (`remote.merge_queue`,
  spec 012).
- Caching anything for the declared extra jobs, which their own workflows own.

## 5. Resolved decisions

**2026-10-23: this spec takes revision 18, and spec 044 moves to revision 19.**
The owner chose this numbering. Spec 044 was approved and not yet started, and
this change is small and measured. Spec 044's text was renumbered in the same
change, with no other edit.

**2026-10-23: stacked validation is an explicit request.** The owner agreed
that a stacked pull request's full matrix should be opt-in by label rather
than automatic. Early feedback stays available, and runner time is no longer
spent on a head that cannot merge.

## Verification

Each line is one command.

```verify:cli
cargo test -p statecraft-home --test setup_workflows revision_eighteen
cargo test -p statecraft-home --test setup_upgrade a_revision_seventeen_project_upgrades_to_revision_eighteen
```
