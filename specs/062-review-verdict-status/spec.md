---
id: "062-review-verdict-status"
title: "Setup profile revision 22 publishes the AI review's verdict as a check run, and review status reads it"
status: draft
implementation: pending
created: "2026-10-27"
summary: >
  Amends spec 002's setup profile, spec 024's review and spec 006's command
  surface. github-actions-rust revision 22 publishes every review result
  (findings, no-findings, skipped:<class>) as a check run on
  the reviewed head, named `statecraft-ai-review`, whose output carries the
  evidence record byte for byte. A new read-only verb, `statecraft-cli review
  status <pr> [--head <sha>] [--repo <owner/name>] [--json]`, reads it under one rule for which
  review counts, the rule ci-gate already uses in the merge queue, and answers
  with a closed outcome and spec 006's exit codes. The `## AI review` comment
  is unchanged. Arming, retrying and waiting are out of scope.
amends:
  - "002-environment-lifecycle"
  - "006-command-surface"
  - "024-review-budget-and-ratification"
# No `extends` yet: the implementing change adds the edges on
# crates/statecraft-home/ (002) and crates/statecraft-cli/ (006) with the code,
# so this draft claims no path while it waits for ratification.
depends_on:
  - "002-environment-lifecycle"
  - "006-command-surface"
  - "012-remote-desired-state"
  - "024-review-budget-and-ratification"
  - "061-review-call-stall"
# `obligations` is spec-spine's own frontmatter field, used by approved specs
# such as 013, 014 and 018; `make gate` lints it with --fail-on-warn.
# Anchors follow spec-spine's slug for a numbered heading: "### 3.1 The
# verdict check run" is `3-1-the-verdict-check-run`, as approved spec 061's
# `3-1-the-stall-watchdog` is; `make gate` resolves every anchor here.
obligations:
  - id: "R-1"
    kind: requirement
    text: "Every review job that ends in a review or a visible skip also creates a check run named `statecraft-ai-review` on the reviewed head, in a step after the evidence record is uploaded; its output carries the result as its title and the evidence record byte for byte."
    anchor: "3-1-the-verdict-check-run"
  - id: "R-2"
    kind: requirement
    text: "The check run's conclusion is `success` for no-findings, `failure` for findings and `neutral` for any skipped result; it is never a required status context, and ci-gate stays the single context branch protection names."
    anchor: "3-1-the-verdict-check-run"
  - id: "R-3"
    kind: requirement
    text: "For a head in this repository, a job that cannot create the check run fails (exit 4), as a review that cannot post its comment does today, so no such result stands without its machine-readable copy. A head in a fork is the one exception: its token cannot create check runs, the step does not run, and review status derives `skipped:fork` from the pull request itself."
    anchor: "3-1-the-verdict-check-run"
  - id: "R-4"
    kind: requirement
    text: "The review that counts for a pull request at a head is the newest `statecraft-ai-review` check run on that head whose embedded record names that pull request and that head; a check run naming another pull request or head is ignored, never counted."
    anchor: "3-2-which-review-counts"
  - id: "R-5"
    kind: requirement
    text: "`review status <pr>` reads the pull request's current head, or checks `--head <sha>` against it, and reports exactly one outcome: no-findings, findings, skipped:<class>, pending, absent or stale; it never writes, waits, retries, arms or comments."
    anchor: "3-3-the-review-status-verb"
  - id: "R-6"
    kind: requirement
    text: "The exit code follows spec 006 section 3.3: 0 for no-findings; 1 for findings or any skipped result; 2 for pending, absent or stale; 3 for usage; 4 when GitHub cannot be read, classified from the `gh` error stream as spec 012 section 3.5 does."
    anchor: "3-4-outcomes-and-exit-codes"
  - id: "R-7"
    kind: requirement
    text: "`absent` names its reason: no statecraft-ci run at the head, a review job that ended cancelled (its duration reported, so a run to the job limit is visible), a review job that failed or refused, or only check runs naming another pull request or head."
    anchor: "3-4-outcomes-and-exit-codes"
  - id: "R-8"
    kind: requirement
    text: "The `## AI review` comment, the skip notices, the evidence artifact and how ci-gate reads a review are unchanged."
    anchor: "3-5-what-does-not-change"
  - id: "V-1"
    kind: verification
    text: "Step tests run ai-review.sh with a stubbed reviewer and a stubbed gh and find one check run per result, carrying the record; rendering tests show the review job's checks permission and a revision-20 project upgrading in one plan; CLI tests run review status against a stub gh and find every outcome and exit code in section 5, including a newer attempt superseding an older one on one head, a record naming another pull request, and a moved head."
    anchor: "verification"
    inputs:
      - "crates/statecraft-home/tests/setup_workflows.rs"
      - "crates/statecraft-home/tests/setup_upgrade.rs"
      - "crates/statecraft-cli/tests/setup_profile.rs"
---

# 062: The AI review's verdict as a check run, read by `review status`

## 1. Purpose

The AI review's result reaches a caller outside the run in two ways today:
the `## AI review` comment, written for people, and the evidence artifact
`statecraft-ai-review-<head>`, which only ci-gate reads, and only in the
merge queue. Anything that has to act on a verdict (arming a pull request,
deciding whether to fix, telling a reviewer what is left) parses the comment.

At rustev, the first adopter, that parsing was copied into six scripts across
delivery sessions between 2026-10-16 and 2026-10-27, and two defects recur in
every copy:

1. **A skip reads as a missing or empty verdict.** The `draft` and
   `dependabot` notices carry no head marker, and a fork gets no notice, so a
   caller that looks for a comment naming the head waits forever. A notice
   that does name the head has no fenced verdict block, so a wrapper that
   arms a pull request on `no-findings` read an empty verdict and silently
   did nothing.
2. **Every copy hardcodes what the profile renders**: the repository, the
   workflow file and the job name `ai-review / review`. A revision that
   renames any of them leaves every caller waiting on a verdict that already
   exists.

Both come from reading a document written for people. This spec gives the
result one machine-readable place, owned by the profile that writes it, and
one reader that applies the rule ci-gate already uses.

## 2. Territory

The registered profile's `ai-review.sh` and `statecraft-ai-review.yml`, the
revision note in the renderer, a new `review status` verb in
`crates/statecraft-cli/`, and tests under `crates/statecraft-home/` and
`crates/statecraft-cli/`. This spec amends spec 024's review for what a
review publishes, spec 002 for the revision, and spec 006 for the verb.

## 3. Behavior

### 3.1 The verdict check run

1. Whenever `ai-review.sh` ends in a review or a visible skip (exit 0 with
   an evidence record; a `findings` verdict is exit 0 too, since the
   script's own exit contract says "A findings verdict is a review, exit 0;
   ci-gate reads it"), a new step of the review job, after the step that
   uploads the record, creates one check run on `HEAD_SHA` named
   `statecraft-ai-review`. It is created last, so the comment or skip notice
   and the uploaded record already stand. A refusal or failure (exit 2 or 4)
   creates none; it already fails the job, and section 3.4 reports it.
2. Its output's `title` is the result string exactly as the record holds it.
   Its `summary` is the subject line the comment ends with (base, head, diff
   digest, calls) and the number of findings. Its `text` is the evidence
   record, byte for byte, as a fenced JSON block. A record over the check run
   output's size limit fails the step (exit 4) rather than being truncated.
3. Its conclusion is `success` for `no-findings`, `failure` for `findings`
   and `neutral` for every `skipped:<class>`. The check
   run is information, never a gate: it is not a required status context,
   and branch protection keeps naming `ci-gate` alone.
4. The review job gains `checks: write`. The workflow-level default stays
   `contents: read`.
5. For a head in this repository, a job that cannot create the check run
   fails with exit 4, through the ERR trap, as a review that cannot post its
   comment does. A head in a fork is the one exception: its token cannot
   create check runs, so the step does not run, and `review status` derives
   `skipped:fork` from the pull request itself by the rule `ai-review.sh`
   applies (3.4). It is the only outcome with no check run behind it.
6. Each attempt of the job creates its own check run. Earlier ones are left
   in place; section 3.2 says which counts.

### 3.2 Which review counts

The review that counts for pull request `N` at head `H` is the newest
`statecraft-ai-review` check run on `H` (highest id) whose embedded record
has `subject.pullRequest == N` and `subject.head == H`. This is the rule
ci-gate's `recorded_review` applies to the evidence artifact in the merge
queue (the newest completed statecraft-ci run at the head), stated over the
check run:

- A re-run that ends in a different result supersedes the earlier one; a
  `skipped:transient` followed by a review counts as the review.
- One commit can be the head of two pull requests. A check run naming
  another pull request is ignored for this one.
- A check run whose record names another head (a record that does not
  describe its commit) is ignored and reported as such (3.4).

### 3.3 The `review status` verb

```
statecraft-cli review status <pr> [--head <sha>] [--repo <owner/name>] [--json]
```

1. It reads through the operator's `gh`, as `doctor --remote` does (spec
   012); it holds no token and reads no secret. `--repo` defaults to the
   repository of the current directory's `origin`.
2. It reads the pull request's current head. With `--head`, a different
   current head is `stale` and reports the current one; nothing older is
   looked up on the caller's behalf.
3. It reads the head's check runs named `statecraft-ai-review` and applies
   section 3.2. With no counting check run, it reads the head's statecraft-ci
   runs and their review job, and decides in this order: a review job not
   yet ended is `pending`, whatever non-counting check runs exist, because
   that job may still publish one that counts; otherwise check runs that
   exist but name another pull request or head are `absent` with reason
   `mismatched-record`; otherwise the review job's end decides `cancelled`
   or `failed`, and no run at all is `no-run`.
4. It never writes: no comment, no re-run, no cancel, no arming. It never
   waits: one read, one answer.
5. `--json` prints one object: `outcome`, `pullRequest`, `head`,
   `currentHead`, `checkRun` (id and URL, or null), `run` (id, attempt and
   review job conclusion and duration, or null), `reason` (for `absent` and
   `stale`) and `record` (the counting evidence record, or null). Without
   `--json` it prints one line per field that is set.

### 3.4 Outcomes and exit codes

| Outcome | When | Exit |
|---|---|---|
| `no-findings` | The counting record's result is `no-findings`. | 0 |
| `findings` | The counting record's result is `findings`; the findings are in `record`. | 1 |
| `skipped:<class>` | The counting record's result is a skip: `draft`, `oversized`, `transient` (spec 061's stall included), or any class a later revision adds. For a head in another repository, `skipped:fork` is reported from the pull request itself, with `checkRun` null, by the rule `ai-review.sh` applies. | 1 |
| `pending` | No counting check run, and a statecraft-ci run at the head is queued or in progress with its review job not yet ended, even when non-counting check runs exist (3.3 item 3). | 2 |
| `absent` | No counting check run, and no run will produce one: no statecraft-ci run at the head (`no-run`), a review job that ended `cancelled` (`cancelled`, with its duration, so a run to the 20-minute limit shows as one), a review job that failed or refused (`failed`), or only check runs naming another pull request or head (`mismatched-record`). | 2 |
| `stale` | `--head` is not the current head. | 2 |

Exit 3 is a usage error. Exit 4 is a read that failed, classified from the
`gh` error stream alone as spec 012 section 3.5 classifies it, never from a
response body.

A skip is exit 1 and never 0: "not reviewed" is a finding about the pull
request, so a caller that treats 0 as reviewed and clean is right without
reading the outcome.

### 3.5 What does not change

The `## AI review` comment and every skip notice, their text and when they
are posted; the evidence record and its artifact; how ci-gate judges a
review on a pull request and reads the recorded one in the merge queue; the
review's prompt, budget, grouping and classification (specs 024 and 061).

### 3.6 The revision

The profile becomes revision 22, with a new identity. Revision 21 is spec
044's. If spec 044 is not delivered first, the implementing change takes the
next free number and says so, as specs 058, 060 and 061 did. A revision-20 or
revision-21 project upgrades through the ordinary plan and apply. The upgrade
pull request changes managed files, so it is an authority change judged by
the base's `ci-gate`, and it needs the owner's exception once.

## 4. Out of scope

Each belongs to a companion draft built on this one, whose `land` verbs
(`land wait`, `land arm`, `land push`, `land restack`, `land status`) share
`review status` as their read:

- arming a pull request on a verdict (checking the base, the verdict for the
  exact head and the required checks);
- re-running a review, or cancelling a run that waits on the owner exception;
- waiting for a verdict, a merge or a queue exit;
- pushing, restacking and landing order. Also out of scope: any
change to what the review finds, and reading reviews made before revision 22,
which have no check run and are `absent` with reason `no-run` or
`mismatched-record` as the case may be.

## 5. Observable negative cases

| Case | Expected |
|---|---|
| A head reviewed `findings`, then re-run and reviewed `no-findings` | Two check runs; `review status` reports `no-findings`, exit 0. |
| A head whose first attempt was `skipped:transient` and whose re-run has not ended | `skipped:transient` is the newest counting record, so the outcome is `skipped:transient`, exit 1, with `run` showing the attempt in progress. |
| A draft pull request | `skipped:draft`, exit 1, never `pending`. |
| A head in a fork | `skipped:fork`, exit 1, `checkRun` null. |
| The review job ran to its 20-minute limit and was cancelled | `absent`, reason `cancelled`, the duration reported, exit 2. |
| The pull request was pushed after the review | Without `--head`: the new head's outcome (usually `pending`). With `--head <old>`: `stale`, naming the current head, exit 2. |
| One commit is the head of two pull requests | Each pull request's outcome comes only from check runs naming it. |
| A `statecraft-ai-review` check run whose record names another head | Ignored; with no other and no review job still running, `absent`, reason `mismatched-record`; with a review job still running, `pending`. |
| A pull request with no statecraft-ci run at its head | `absent`, reason `no-run`, exit 2. |
| `gh` answers 401 | Exit 4, `unauthorized`, no body or header in the output. |
| The review job cannot create the check run | The job fails with exit 4; ci-gate blocks as for any failed review job. |

## 6. Resolved decisions

None yet. Two decisions are for ratification:

1. **The conclusion mapping (3.1.3).** `failure` for findings puts a red
   mark on the pull request even when the owner approves the exception. The
   alternative is `neutral` for every result, with the verdict in the title
   only.
2. **A failed check run fails the job (3.1.5).** The alternative records the
   failure and lets the review stand, at the price of a verdict callers
   cannot read.

## Verification

This spec is `status: draft` and declares no acceptance until it is ratified
and built. Its acceptance will run `ai-review.sh` with a stubbed reviewer and
`gh` and find one check run per result carrying the record, render the
revision and find the review job's `checks: write`, upgrade a revision-20
project in one plan, and run `review status` against a stub `gh` through
every row of section 5.
