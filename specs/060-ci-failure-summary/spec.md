---
id: "060-ci-failure-summary"
title: "Setup profile revision 19: a blocked ci-gate names each failed job with its first error lines, and the code job's cache leaves out workspace artifacts"
status: approved
implementation: complete
created: "2026-10-24"
summary: >
  Amends spec 002's setup profile. github-actions-rust revision 19 changes
  what a failure tells its reader and what the cache holds, never what
  judges: when ci-gate blocks, it lists every failed job of its run with the
  failed step and at most five error lines from that job's log, in the step
  summary and as annotations, on a best-effort read that never changes the
  verdict or the exit code. Before the code job saves the cargo cache it
  drops the workspace's own artifacts, which Cargo rebuilds after any
  checkout. Spec 044 moves from revision 19 to revision 20.
amends:
  - "002-environment-lifecycle"
  - "044-supply-chain-profile"
  - "058-ci-throughput-profile"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
depends_on:
  - "002-environment-lifecycle"
  - "058-ci-throughput-profile"
  - "059-job-level-concurrency"
obligations:
  - id: "R-1"
    kind: requirement
    text: "When ci-gate blocks, it lists each job of its own run attempt that ended failure, other than ci-gate, with its first failed step and at most five matching log lines, in the step summary and as at most ten error annotations."
    anchor: "3-1-the-failure-summary"
  - id: "R-2"
    kind: requirement
    text: "The failure summary never changes ci-gate's verdict or exit code: a read that fails is reported as unavailable with its reason, and a passing gate reads nothing."
    anchor: "3-1-the-failure-summary"
  - id: "R-3"
    kind: requirement
    text: "Log text from the candidate is written only to the step summary inside a fenced block it cannot close, and to annotations with %, carriage return and newline escaped, so it can issue no workflow command."
    anchor: "3-2-candidate-text-is-data"
  - id: "R-4"
    kind: requirement
    text: "Before the code job saves the cargo cache it removes the workspace members' artifacts (cargo clean -p for each member), so the saved entry holds dependencies only."
    anchor: "3-3-the-code-cache-holds-dependencies-only"
  - id: "R-5"
    kind: requirement
    text: "Spec 044's jobs are delivered as revision 20: wherever spec 044, or spec 058 section 3.6, names revision 19 for them it is read as revision 20, and revision 18 as the revision upgraded from is read as revision 19."
    anchor: "3-5-spec-044-becomes-revision-20"
  - id: "V-1"
    kind: verification
    text: "Step tests run ci-gate.sh against a stubbed gh: a blocked run lists the failed jobs, steps and error lines, escapes annotation text and fences the summary; a failed read reports unavailable with the same exit code; a passing run reads nothing. Rendering tests show the code job's trim step, and a revision-18 project upgrades to revision 19 in one plan."
    anchor: "verification"
    inputs:
      - "crates/statecraft-home/tests/setup_workflows.rs"
      - "crates/statecraft-home/tests/setup_upgrade.rs"
---

# 060: A blocked gate names what failed, and the code cache holds dependencies only

## 1. Purpose

Revision 18's `ci-gate` reports a failure as `BLOCK: required job 'bindings'
ended 'failure'`. The required job is often a reusable workflow of many jobs
(rustev's `bindings` has ten), so the reader opens the run, finds the red
job, opens its log and searches it. At rustev on 2026-10-24 that was the
usual path for each of nine failed runs, among them a merge-queue entry
whose only failure was one test in `bindings / node 20`.

The same adopter measured the cache after revision 18 (25 runs). The `code`
job's entry was 2.7 GB, and the job restored it once in 19 runs. Most of
the entry was the workspace's own crates. Cargo fingerprints a path
dependency by its source files' modification times, which a fresh checkout
resets, so those artifacts are rebuilt in every run. They take space under
the repository's 10 GB limit and evict entries a later run could use.

## 2. Territory

The registered profile's `ci-gate.sh` and `statecraft-ci.yml`, the revision
note in the renderer, and tests under `crates/statecraft-home/`. This spec
amends spec 058 section 3.6 and its obligation R-6 (for the renumbering
only), and spec 044 as section 3.5 says.

## 3. Behavior

### 3.1 The failure summary

When `ci-gate` is about to exit 1 (blocked), it reads the jobs of its own run
attempt (`GITHUB_RUN_ID`, `GITHUB_RUN_ATTEMPT`) with the token the job
already holds (`actions: read`). For each job that ended `failure`, other
than `ci-gate`, in the order the API lists them, it reports:

- the job's name and its first step that ended `failure`;
- at most five lines of that job's log that carry an error marker
  (`##[error]`, `error[`, `error:`, `panicked at`, `FAILED`, `not ok`,
  `BLOCK:`), each without its timestamp and cut to 300 characters.

The report goes to the step summary under `Failed jobs`, and as one error
annotation per failed job, at most ten, titled with the job name and carrying
its first matching line. At most twenty jobs are read.

The summary is best effort. A read that fails, or a missing `REPO`,
`GH_TOKEN` or run id, prints `failure summary unavailable:` with the reason,
and `ci-gate` exits as it would have without the summary. A gate that passes
or refuses (exit 0 or 2) reads nothing. The summary judges nothing. It never
names a job as the cause of the verdict that the `BLOCK:` lines did not
already name.

### 3.2 Candidate text is data

A job's log is text the candidate produced. It reaches the step summary only
inside a fenced block whose fence is longer than any run of backticks in the
text. It reaches an annotation only after `%`, carriage return and newline
are written as `%25`, `%0D` and `%0A`, and the title's `:` and `,` as `%3A`
and `%2C`. It is never printed raw to standard output, where the runner would
read workflow commands from it.

### 3.3 The code cache holds dependencies only

The `code` job's save step, which runs only on a push (spec 058 section 3.4),
is preceded on the same condition by a step that runs
`cargo clean -p <member>` for each workspace member named by
`cargo metadata --no-deps`. The restore step and its key are unchanged.

### 3.4 The revision

The profile becomes revision 19 with a new identity. A revision-18 project
upgrades through the ordinary plan and apply. The upgrade pull request is an
authority change, judged by the base's revision-18 `ci-gate`, and it needs the
owner's exception once.

### 3.5 Spec 044 becomes revision 20

Spec 044 is approved and not started. Its jobs are delivered as revision 20.
Wherever spec 044 names revision 18 or 19 for them, or spec 058 section 3.6
names revision 19, it is read as revision 20. Wherever either names the
revision upgraded from, it is read as revision 19. Nothing else in spec 044
or spec 058 changes, and their text is not edited.

## 4. Out of scope

- Retrying, rerunning or cancelling any job.
- A summary for the extra jobs' own workflows, which their projects own.
- Changing which jobs are required or how any rule judges.

## 5. Resolved decisions

**2026-10-24: the trim step conforms to section 3.3 as written.** The
first adopter's review found that an empty member list made `xargs` run a
bare `cargo clean`, which cleans more than "each workspace member" and
empties `target`. The template now passes the list with `xargs -r`. No
requirement of this spec changes, so its text is not amended; it is
`status: approved` and was ratified in pull request 324. Revision 19 keeps
its number, as spec 059 kept 18, because no project has merged it.
statecraft-cli's own rendered workflow is revision 17
(`github-actions-rust@17` in `.statecraft/environment.json`) and has no
trim step, so there is nothing of its own to re-render.

**2026-10-24: ratified by the owner.** The implementation's AI review
returned no findings. The owner directed ratification in this pull request,
and the owner exception approved on its run is the ratification of record.

**2026-10-24: take revision 19 and move spec 044 to 20.** This follows spec
058's precedent for an approved, unstarted spec 044. The owner authorized the
delivery-process improvements on 2026-10-24; ratifying this draft remains the
owner's act.

## Verification

Each line is one command.

```verify:cli
cargo test -p statecraft-home --test setup_workflows revision_nineteen
cargo test -p statecraft-home --test setup_upgrade a_revision_eighteen_project_upgrades_to_revision_nineteen
```
