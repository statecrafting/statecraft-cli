---
id: "067-post-merge-push"
title: "Setup profile revision 24: where the merge queue is required, a push to the default branch runs only the cache saves and ci-gate"
status: approved
implementation: complete
created: "2026-11-02"
summary: >
  Amends specs 002, 032, 044 and 058. Where the policy records a required
  merge queue, the queue has already judged the exact commit a push to the
  default branch carries. github-actions-rust revision 24 skips every job
  that judges on that push, keeps the cache steps, and has ci-gate admit the
  skips by an explicit post-merge rule only when the pushed commit passed
  ci-gate in its merge-queue run. Repo-owned called workflows receive a
  boolean post_merge input. Without a required queue nothing changes.
amends:
  - "002-environment-lifecycle"
  - "032-external-code-profile"
  - "044-supply-chain-profile"
  - "058-ci-throughput-profile"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
depends_on:
  - "012-remote-desired-state"
  - "032-external-code-profile"
  - "044-supply-chain-profile"
  - "058-ci-throughput-profile"
  - "059-job-level-concurrency"
obligations:
  - id: "R-1"
    kind: requirement
    text: "Where remote.merge_queue.required is true, governance, preflight, supply-chain, workflow-lint, sg-scan and secret-scan are skipped on a push, and the policy's push rule for them, code and every declared extra job is post-merge."
    anchor: "3-1-what-a-post-merge-push-runs"
  - id: "R-2"
    kind: requirement
    text: "On a post-merge push, code runs its cache steps: the spec-spine cache, a lookup of the cargo key, and only on a miss the build that fills it and the save."
    anchor: "3-2-the-cache-steps"
  - id: "R-3"
    kind: requirement
    text: "ci-gate runs on the push and admits a post-merge job that succeeded or was skipped only when the pushed commit's latest completed merge-queue run of statecraft-ci has a successful ci-gate job; a failed, cancelled or vanished job, a commit no queue run judged, a failed queue gate and an unreadable record each block."
    anchor: "3-3-ci-gate-s-post-merge-rule"
  - id: "R-4"
    kind: requirement
    text: "With a required queue, every declared extra job and an external code call receive the boolean input post_merge, true on a push, and run where preflight is skipped; the plan refuses a called workflow that does not declare the input."
    anchor: "3-4-called-workflows"
  - id: "R-5"
    kind: requirement
    text: "Without a required queue, the rendered jobs, their conditions and the policy's push rules are those of revision 23."
    anchor: "3-5-without-a-required-queue"
  - id: "V-1"
    kind: verification
    text: "Rendering tests show the post-merge form only under a required queue, the cache steps, the policy rules and the called workflows' input and refusal; ci-gate.sh, run as a program, admits and blocks each post-merge case, and every new blocking branch is noticed when inverted."
    anchor: "verification"
    inputs:
      - "crates/statecraft-home/tests/setup_workflows.rs"
  - id: "V-2"
    kind: verification
    text: "A revision-23 project upgrades to revision 24 in one plan, with and without a required queue, and a second plan writes nothing."
    anchor: "verification"
    inputs:
      - "crates/statecraft-home/tests/setup_upgrade.rs"
---

# 067: A post-merge push runs only the cache saves and the gate

## 1. Purpose

A merge queue judges the exact commit that lands. GitHub builds each entry
on a `gh-readonly-queue/<branch>/pr-<n>-<sha>` branch and, when its required
checks pass, fast-forwards the default branch to that entry's head commit.
The `push` run that follows therefore re-tests a tree the queue has just
passed.

An adopter measured the cost (rustev, 2026-11-02). Push-to-main runs were
about 30% of runner minutes from 10-28 to 11-01. On 11-02 the queue ran 6.9
runs per merge against a usual 1.0 to 1.3, almost all failing on runner
faults, and the push runs competed with it for the same runners.

Measured on this repository on 2026-11-02: the default branch's head
`371d4013` carries merge-queue run 41548562019 (`merge_group`, branch
`gh-readonly-queue/main/pr-398-8485a2e4...`, success) at that same SHA.
That run's `ci-gate` check run 125643373126 concluded `success` from app
15368 on `371d4013`, while the push run 41555616880 for the commit was
still queued. The five first-parent commits before it each carry a
successful `merge_group` run at their own SHA. One of them, `8485a2e4`, has
no push run at all, because the queue landed two entries in one push. So the
merge-queue run's checks already sit on the pushed SHA, and a consumer that
reads `ci-gate` check runs on a commit sees that one. rustev's `release.yml`
(its spec 058) accepts any completed `ci-gate` check run that concluded
`success` on the tagged commit (`tools/rustev-release/src/tagged.rs`, `ci_gate`).
This revision still reports a `ci-gate` check run on every push, so the
requirement holds whichever run the consumer reads.

## 2. Territory

The registered profile's `statecraft-ci.yml` template and `ci-gate.sh`, the
revision, the policy's push rules, the renderer's extra-job and external-code
text and its plan refusal, the remote obligation, and tests under
`crates/statecraft-home/`. This spec amends:

- spec 002's setup profile, where `ci-gate` blocks a skipped required job on
  every event;
- spec 032 section 3.2, where `code` is needed on pushes and a skipped `code`
  blocks;
- spec 044 sections 3.6 and 3.7, where `secret-scan` scans a push's range and
  the four jobs are required on every event;
- spec 058 obligations R-2 and R-5, where a skipped `preflight` blocks on
  every event and the cargo cache is saved from a push.

Each holds unchanged for `pull_request` and `merge_group`, and for `push`
where no queue is required.

## 3. Behavior

### 3.1 What a post-merge push runs

A **post-merge push** is a `push` to the default branch in a project whose
setup parameters declare `remote.merge_queue` with `required: true` (spec
012), rendered by revision 24 or later. The rendered workflow knows it from
that parameter. The repository settings are not read at run time.

On a post-merge push, `governance`, `preflight`, `supply-chain`,
`workflow-lint`, `sg-scan` and `secret-scan` carry
`if: github.event_name != 'push'` and are skipped. `ai-review` and
`review-exception` already run only on `pull_request`. `code` and every
declared extra job need `preflight`, and they run there with
`!cancelled() && (needs.preflight.result == 'success' || github.event_name == 'push')`.
On every other event that condition is the default one. `ci-gate` keeps
`if: always()`.

### 3.2 The cache steps

The `code` job keeps every cache step on a post-merge push. It reads the gate
at the base and installs the pinned spec-spine through its cache, so the
default branch's spec-spine entry is still saved on a pin change. It looks
up the cargo key with `lookup-only` and downloads nothing. When the key
missed, it runs `gate.sh code` to fill the entry, drops the workspace's own
artifacts and saves it, as revision 19 does. A key that hit runs nothing
more. A cache filled from nothing judges no correctness the queue did not
already judge, but its build can fail, and that failure blocks like any
other. A missed key restores no partial entry, so the fill builds from a
clean target. Keys change only with `Cargo.lock` and `rust-toolchain.toml`.

### 3.3 ci-gate's post-merge rule

The policy's `push` rule for `governance`, `preflight`, `code`,
`supply-chain`, `workflow-lint`, `sg-scan`, `secret-scan` and every declared
extra job becomes `post-merge`. The policy records the rule under
`post_merge_rule`. `ci-gate.sh` reads it from the base, as every rule:

1. A job under `post-merge` that ended `success` or `skipped` is admitted. A
   job that ended `failure` or `cancelled` blocks, and a vanished job blocks
   as under every rule.
2. The pushed commit must have been judged by the queue. The gate reads the
   completed `merge_group` runs of `statecraft-ci` whose head is the pushed
   commit, takes the latest, and requires its `ci-gate` job to have concluded
   `success`. A push with no such run bypassed the queue and blocks. So do a
   queue run whose `ci-gate` did not succeed and a record the API cannot
   return.

The gate only reads, with the token it already holds (`actions: read`). A
skip is never admitted on `pull_request` or `merge_group`, and never on a
push without a required queue: there the rules stay `required`, and a
skipped job is an unexpected skip.

### 3.4 Called workflows

With a required queue, each job declared in `ci.extra_required_jobs` and an
external code selection's `code` call are rendered with
`with: post_merge: ${{ github.event_name == 'push' }}`. The called workflow
declares the input under `on: workflow_call`:

```yaml
on:
  workflow_call:
    inputs:
      post_merge:
        type: boolean
        default: false
```

It skips its expensive steps when `inputs.post_merge` is true, and still
saves any cache it owns. GitHub refuses a call that passes an undeclared
input, so the plan refuses such a workflow instead, naming the parameter, the
path and the declaration. A called job that ended `success` or `skipped` is
admitted by the post-merge rule. Without a required queue no input is passed,
and none needs to be declared. The input is the documented test. Inside a
called workflow, `github.event_name` is the caller's event, but it cannot
tell whether the queue is required.

### 3.5 Without a required queue

With `remote.merge_queue` absent or `required: false`, the rendered jobs,
their conditions and the policy's push rules are those of revision 23. The
render differs only in its revision, its identity and a comment that
describes this revision. A push still runs every job, and ci-gate still
blocks each skip.

### 3.6 The revision

The profile becomes revision 24 with a new identity. A revision-23 project
upgrades through the ordinary plan and apply: the workflow and `ci-gate.sh`
are replaced, and the policy's push rules change only where the queue is
required. The upgrade pull request is an authority change and needs the
owner's exception once. The push of its own merge commit is judged by the
base's revision-23 policy and script, which know no post-merge rule, so
that one push's `ci-gate` blocks on the skipped jobs. The queue's `ci-gate`
check run on the same commit is unaffected. Do not tag a release on that
commit by its push run alone.

## 4. Out of scope

- Reading whether the queue is required from the repository at run time;
  `doctor --remote` already compares the declared value with the remote
  (spec 012).
- Removing the push trigger, or the `ci-gate` check run on a push.
- What a called workflow skips; the project owns its steps.
- Adopting revision 24 in this repository's own CI, which is its own change.

## 5. Resolved decisions

**2026-11-02: approved by the owner.** "rustev CI throughput, 2026-11-02
(owner-approved). Evidence: on 11-02 the merge queue ran 6.9 runs per merge
(normally 1.0-1.3), almost all failing on runner faults. Push-to-main runs
were about 30% of runner minutes from 10-28 to 11-01, re-testing the tree the
queue had just passed. 43% of PR runs needed the owner exception. B. When the
merge queue is required, a push to the default branch runs only the cache
saves plus a ci-gate check-run on the pushed commit. Rustev's release.yml
(spec 058) requires ci-gate success on the tagged main commit: keep that, or
confirm the merge-group run's check already sits on that SHA. Give the
repo-owned extra workflows a post_merge input (or document the event test) so
they can skip their expensive steps." This spec delivers item B. The
merge-group check was confirmed on the pushed SHA (section 1), and the push's
own `ci-gate` check run is kept as well.

**2026-11-02: the push's gate requires the queue's verdict.** Admitting the
skips alone would let a push that bypassed the queue report a green
`ci-gate` with nothing judged. The rule therefore reads the queue run at the
pushed commit and fails closed. A missed cargo key is filled on the push,
because a save of an unbuilt `target` would hit every later restore and
never refresh.

## Verification

Each line is one command.

```verify:cli
cargo test -p statecraft-home --test setup_workflows revision_twenty_four
cargo test -p statecraft-home --test setup_workflows ci_gate_judges_a_post_merge_push_by_the_queue_run_at_its_commit
cargo test -p statecraft-home --test setup_workflows inverting_a_blocking_branch_of_ci_gate_is_noticed
cargo test -p statecraft-home --test setup_upgrade a_revision_twenty_three_project_upgrades_to_revision_twenty_four
```
