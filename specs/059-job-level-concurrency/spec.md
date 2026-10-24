---
id: "059-job-level-concurrency"
title: "Setup profile revision 18 cancels superseded work job by job, so a run waiting on the owner exception never blocks a newer one"
status: approved
implementation: complete
created: "2026-10-24"
summary: >
  Amends spec 058. Revision 18's single workflow-level concurrency group let
  a run waiting on the owner-exception Environment hold the group: GitHub does
  not cancel such a run, so every newer push to the same pull request stayed
  pending behind it. Every job except review-exception and ci-gate now has a
  concurrency group of its pull request and job instead, and the workflow has
  none. Revision 18 keeps its number: no project had adopted the first form.
amends:
  - "058-ci-throughput-profile"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
depends_on:
  - "058-ci-throughput-profile"
obligations:
  - id: "R-1"
    kind: requirement
    text: "The rendered statecraft-ci workflow declares no workflow-level concurrency group."
    anchor: "3-1-groups-per-job"
  - id: "R-2"
    kind: requirement
    text: "governance, preflight, code, ai-review and every declared extra job (and an external code call) each have a group of their pull request and job, cancelled in progress only for pull_request; a push and a merge_group run's groups are named by their own run id."
    anchor: "3-1-groups-per-job"
  - id: "R-3"
    kind: requirement
    text: "review-exception and ci-gate have no concurrency group, so a run waiting on the owner exception holds nothing a newer run needs."
    anchor: "3-1-groups-per-job"
  - id: "V-1"
    kind: verification
    text: "Rendering tests resolve each job's group and cancel flag per event, show no workflow-level group and none on review-exception or ci-gate, and show the extra and external code jobs carry theirs."
    anchor: "verification"
    inputs:
      - "crates/statecraft-home/tests/setup_workflows.rs"
      - "crates/statecraft-home/tests/setup_upgrade.rs"
---

# 059: Superseded work is cancelled job by job

## 1. Purpose

Spec 058 section 3.1 put every `pull_request` run of a pull request in one
workflow-level concurrency group with `cancel-in-progress`. Its first adopter
showed the flaw before it merged (rustev pull request 221, 2026-10-24). Run
40054810664 was waiting on the `statecraft-review-exception` Environment, and
the group did not cancel it. GitHub does not cancel a run waiting on an
Environment through a concurrency group, any more than through a plain cancel
request. The run kept holding the group, so the next push's run stayed
`pending` behind it, and a third push replaced that pending run, which never
ran. A pull request whose review returns findings needs the owner exception,
and that is the usual moment for a fix push. Under the first form, every such
fix waited on the owner's action on the old run.

## 2. Territory

The registered profile's `statecraft-ci.yml` template, the renderer's
extra-job and external-code text, the revision note, and tests under
`crates/statecraft-home/`. This spec amends spec 058 section 3.1 and its
obligation R-1. Nothing else in spec 058 changes.

## 3. Behavior

### 3.1 Groups per job

The rendered workflow declares no workflow-level `concurrency`. Each of
`governance`, `preflight`, `code`, `ai-review`, every job declared in
`ci.extra_required_jobs`, and an external code selection's `code` call
declares:

```yaml
concurrency:
  group: statecraft-ci-<pr-N, or the run id>-<job>
  cancel-in-progress: <true on pull_request only>
```

The group names the pull request (`pr-<number>`) on `pull_request` and the
run's own id on `push` and `merge_group`. A newer push to a pull request
therefore cancels each job of the older run that is queued or running. A push
to the default branch and a merge-queue entry each keep groups of their own run,
and neither is ever cancelled or queued behind another run.

`review-exception` and `ci-gate` declare no group. They are the jobs that wait
on the owner exception (`ci-gate` waits through its need on it). A run left
waiting there holds no group, so the newer run starts at once. The older run's
wait is left for the owner or for a cleanup that force-cancels superseded
waiting runs. It judges a head that no longer exists, so it cannot make the
pull request's required check green.

### 3.2 The revision

The profile stays revision 18 with a new identity. No project had rendered
the first form into a merged tree (statecraft-cli itself is at revision 17,
and rustev's adoption was held), so no adopter carries the flawed identity. A
revision-17 project upgrades to this form through the ordinary plan and apply.

## 4. Out of scope

- Cancelling a run that is waiting on an Environment, which a workflow token
  that only reads cannot do.
- Any change to what a job judges, or to ci-gate's rules.

## 5. Resolved decisions

**2026-10-24: ratified by the owner.** The implementation's AI review found
no defect in the code; its only finding was this spec's draft status. The
owner ratified it as drafted, and its declared acceptance passes.

**2026-10-24: correct revision 18 rather than take a new number.** The flaw
was found by the first adoption before it merged. Keeping the number avoids a
revision that no project would ever adopt.

## Verification

Each line is one command.

```verify:cli
cargo test -p statecraft-home --test setup_workflows revision_eighteen
cargo test -p statecraft-home --test setup_upgrade a_revision_seventeen_project_upgrades_to_revision_eighteen
```
