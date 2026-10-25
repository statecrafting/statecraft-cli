---
id: "061-review-call-stall"
title: "Setup profile revision 20: a stalled AI review call is detected, retried once, and otherwise skipped as transient"
status: approved
implementation: complete
created: "2026-10-25"
summary: >
  Amends spec 002's setup profile and spec 024's review. github-actions-rust
  revision 20 watches each reviewer call: the call streams its events, and a
  call that emits none for 180 seconds is stopped and retried once with the
  same input. A second stall ends the review as a visible
  `skipped:transient`, naming the stall, instead of running into the job's
  limit. The review job's limit becomes 20 minutes so that one retry fits.
  What the review reads, the findings it reports and how they judge are
  unchanged. Spec 044 moves from revision 20 to revision 21.
amends:
  - "002-environment-lifecycle"
  - "024-review-budget-and-ratification"
  - "044-supply-chain-profile"
  - "058-ci-throughput-profile"
  - "060-ci-failure-summary"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
# Spec 044 is amended (renumbered, section 3.5) but not depended on, as in
# spec 060: it is approved and unstarted, and `spec-spine registry plan`
# would report this spec "blocked by 044-supply-chain-profile (pending)".
depends_on:
  - "002-environment-lifecycle"
  - "024-review-budget-and-ratification"
  - "058-ci-throughput-profile"
  - "060-ci-failure-summary"
obligations:
  - id: "R-1"
    kind: requirement
    text: "Each reviewer call runs with streamed output, partial messages included, and is stopped when it emits no event for 180 seconds; a call that keeps emitting events is never stopped by this rule."
    anchor: "3-1-the-stall-watchdog"
  - id: "R-2"
    kind: requirement
    text: "A stopped call is retried once, with the same input and a fresh working directory and home; at most one retry is made per call."
    anchor: "3-2-one-retry"
  - id: "R-3"
    kind: requirement
    text: "A call that stalls on its retry ends the review as `skipped:transient`, posting the skip notice with the stall's reason and the diff digest as every transient skip does; a stall never yields findings, no-findings or a refusal."
    anchor: "3-2-one-retry"
  - id: "R-4"
    kind: requirement
    text: "The review text of a call is the text of its final result event; the prompt, inputs, grouping, budget, refusal and transient classification of spec 024 and the findings format are unchanged."
    anchor: "3-3-what-does-not-change"
  - id: "R-5"
    kind: requirement
    text: "The review job's timeout-minutes is 20."
    anchor: "3-4-the-revision"
  - id: "R-6"
    kind: requirement
    text: "Spec 044's jobs are delivered as revision 21: wherever spec 044, spec 058 section 3.6 or spec 060 section 3.5 names revision 20 for them it is read as revision 21, and the revision upgraded from as revision 20."
    anchor: "3-5-spec-044-becomes-revision-21"
  - id: "V-1"
    kind: verification
    text: "Step tests run ai-review.sh with a stubbed claude: a call that stalls once and then answers is retried once and its review is used; a call that stalls twice ends as skipped:transient with the stall named; a call that streams slowly but steadily past 180 seconds of total time is never stopped. Rendering tests show the review job's 20-minute limit, and a revision-19 project upgrades to revision 20 in one plan."
    anchor: "verification"
    inputs:
      - "crates/statecraft-home/tests/setup_workflows.rs"
      - "crates/statecraft-home/tests/setup_upgrade.rs"
---

# 061: A stalled AI review call is detected, retried once, and otherwise skipped as transient

## 1. Purpose

Revision 19's review job runs each reviewer call as `claude -p` with text
output and no limit of its own; only the job's 15-minute `timeout-minutes`
stops it. At rustev, 131 review jobs that reviewed something ran between
2026-10-24 and 2026-10-25. 119 finished in 10 minutes or less (most in 1 to
7). 12, about 9%, ran to the 15-minute limit and were cancelled. None
finished between 11 and 14 minutes. Each cancelled log goes silent right
after the `AI review credential` line until the cancel, so the job was
waiting inside a reviewer call.

The size of the change does not explain it: one pull request of 1,785
changed lines passed in 7 to 9 minutes on three attempts and hung on three
others. Each hang costs the full 15 minutes, a cancelled owner-exception
wait, and a manual rerun.

## 2. Territory

The registered profile's `ai-review.sh` and `statecraft-ai-review.yml`, the
revision note in the renderer, and tests under `crates/statecraft-home/`.
This spec amends spec 024's review for the call itself, and spec 044, spec
058 section 3.6 and spec 060 section 3.5 for the renumbering only.

## 3. Behavior

### 3.1 The stall watchdog

1. Each reviewer call runs as `claude -p` with `--output-format stream-json`,
   `--verbose` and `--include-partial-messages`, so that the call emits an
   event for each partial message while the model writes.
2. A watchdog reads the stream. When 180 seconds pass with no new event,
   it stops the call (`SIGTERM`, then `SIGKILL` after 10 seconds) and
   records a stall for that call.
3. The watchdog limits silence, never total time: a call that keeps
   emitting events runs until it ends or the job's limit is reached.
4. The pinned CLI (`CLAUDE_CLI_VERSION`) must support these flags; the
   implementation confirms that against the pinned version, and raises the
   pin in this revision if it does not.

### 3.2 One retry

1. A stalled call is retried once, with the same input, in a fresh working
   directory and home, as every call already runs.
2. A retry that ends normally is that call's review; the review continues
   with the next call.
3. A retry that stalls ends the whole review as `skipped:transient`, through
   the same skip path, notice and evidence record as a recognized transient
   provider failure, with the reason `the reviewer call stalled twice (no
   event for 180 seconds)`. A stall is never treated as findings,
   no-findings or a refusal.

### 3.3 What does not change

The prompt, the governance context, the grouping into calls, the token
budget and `review.max_calls`, the refusal and transient classification of
a call that exits non-zero, the findings format and how `ci-gate` judges a
review. A call's review text is the `result` field of its final `result`
event, which is the text the call would have written with `--output-format
text`. An exit with no `result` event is the reviewer writing nothing, as
today.

### 3.4 The revision

The review job's `timeout-minutes` becomes 20, so that a call stalled at 180
seconds can be retried and finish within the job (the longest observed
review was 10 minutes). The profile becomes revision 20 with a new
identity. A revision-19 project upgrades through the ordinary plan and
apply; the upgrade pull request is an authority change, judged by the
base's revision-19 `ci-gate`, and it needs the owner's exception once.

### 3.5 Spec 044 becomes revision 21

Spec 044 is approved and not started. Its jobs are delivered as revision
21. Wherever spec 044, spec 058 section 3.6 or spec 060 section 3.5 names
revision 20 for them, it is read as revision 21; wherever one names the
revision upgraded from, it is read as revision 20. Nothing else in those
specs changes, and their text is not edited.

## 4. Out of scope

- Retrying a call that exits non-zero; spec 024's classification stands.
- Splitting a large pull request, or any change to the token budget.
- Rerunning or cancelling a job from outside it.

## 5. Observable negative cases

| Case | Expected |
|---|---|
| A call emits no event for 180 seconds, then its retry answers | One retry; the retry's review is used; the review's verdict is the retry's. |
| A call and its retry both emit no event for 180 seconds | `skipped:transient`, naming the stall; no findings and no refusal. |
| A call streams events steadily for 9 minutes | Never stopped; its review is used. |
| A call ends without a `result` event | Treated as the reviewer writing nothing, as revision 19 does. |
| A call exits non-zero with a provider refusal | Spec 024's refusal, unchanged; not retried. |

## 6. Resolved decisions

**2026-10-25: ratified by the owner.** The owner directed that this spec
be implemented with its ratification in the same pull request; the owner
exception approved on that pull request's run is the ratification of
record. Both decisions are taken as proposed: a stall is 180 seconds with
no event, and the review job's limit is 20 minutes. The rule "when a
review times out, split the pull request rather than raising the limit"
stays for reviews that are only slow.

**2026-10-25: the pinned CLI supports the flags (3.1.4).**
`@anthropic-ai/claude-code@2.1.116`, the pinned `CLAUDE_CLI_VERSION`,
lists `--include-partial-messages` (print mode with stream-json) and
`--output-format stream-json`, so the pin does not change. Whether long
model thinking can pause partial messages for 180 seconds was not
measured live; the longest finished review at the first adopter was 10
minutes of total time, and the watchdog limits silence only.

## Implementation record

**2026-10-25.** `ai-review.sh` runs each call as `claude -p` with
`--output-format stream-json --verbose --include-partial-messages` in the
background, polls its events file once a second, and stops a call whose
file has not grown for 180 seconds (`SIGTERM`, `SIGKILL` after 10 seconds).
A stalled call is retried once in a fresh working directory and home; a
second stall ends the review through the transient skip with the reason
`the reviewer call stalled twice (no event for 180 seconds)`. A call's
review text is the `.result` of its last `result` event, non-JSON lines
ignored; a non-zero exit keeps spec 024's classification, reading the
events in place of the text output. `AI_REVIEW_STALL_SECONDS`, a positive
integer, overrides the limit for tests only; the rendered workflow never
sets it. The review job's `timeout-minutes` is 20, and the profile is
revision 20. Revision 19's two changed templates are kept byte for byte in
`tests/support/profile-r19/` for the upgrade tests. `cargo test -p
statecraft-home` passes (597 tests, 1 ignored), including the five
`revision_twenty` workflow tests and the revision 19 to 20 upgrade.

## Verification

Each line is one command.

```verify:cli
cargo test -p statecraft-home --test setup_workflows revision_twenty
cargo test -p statecraft-home --test setup_upgrade a_revision_nineteen_project_upgrades_to_revision_twenty
```
