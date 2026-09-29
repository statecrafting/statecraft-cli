---
id: "024-review-budget-and-ratification"
title: "Setup profile revision 12: a token-budgeted AI review and ratification as a merge condition"
status: approved
implementation: in-progress
created: "2026-09-27"
summary: >
  Amends spec 002's setup profile and spec 017's review cap. github-actions-rust
  revision 12 leaves a managed file out of the AI review only when its bytes
  match the digest its policy records, sends deletions as a list under their
  own cap, measures the reviewable change in estimated tokens, and reviews a
  change larger than one call in file groups whose verdicts are merged. It also
  refuses a merge that changes a path a draft spec owns, unless that pull
  request ratifies the spec with the owner's exception. An unreadable base now
  refuses instead of letting the candidate's policy and scripts judge it.
amends:
  - "002-environment-lifecycle"
  - "017-ai-review-diff-cap"
amends_verification:
  - "017-ai-review-diff-cap"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
  - "017-ai-review-diff-cap"
  - "023-contained-commit-walk"
---

# 024: A token-budgeted AI review and ratification as a merge condition

## 1. Purpose

Revision 11's AI review has three defects the owner decided to repair
together, and its merge gate has one gap.

1. **Managed files are reviewed as if they were authored.** A profile upgrade
   re-renders every managed file, and the review spends its budget on bytes
   Statecraft wrote. `review.exclude` removes a path by prefix only, which
   would also hide a hand edit to that path.
2. **Deletions count as much as additions.** A pull request that removes a
   large directory is `oversized` although nothing new needs review.
3. **The cap is in lines.** Lines do not measure what a reviewer call costs or
   whether it fits the model's context, and one call is the only shape: a
   change over the cap is skipped, never split.
4. **An unratified spec's code can merge.** Agents never ratify (AGENTS.md),
   but nothing in `ci-gate` refuses a change to code whose owning spec is still
   `draft`. The rule depended on the merger remembering it.
5. **An unreadable base is read as an adoption.** spec-spine's review of its
   pull request 403 found, by reading the code, that `ci-gate.sh`'s policy read
   and the workflow steps that read `ci-gate.sh` and `ai-review.sh` at the base
   take the adoption branch whenever `git show <base>:<path>` fails, which it
   also does when the base commit itself cannot be resolved. The candidate's
   own policy and scripts then judge it.

## 2. Territory

This spec changes the registered profile revision, the templates of
`ai-review.sh`, `ci-gate.sh`, `gate.sh`, `statecraft-ci.yml` and
`statecraft-ai-review.yml`, the setup parameters and policy, and tests under
`crates/statecraft-home/`. It extends spec 006's `crates/statecraft-cli/` only
to add the four new policy parameter keys to the persisted policy document's
grandfathered snake_case keys in the JSON naming test support. The rendered files remain managed outputs of spec
002's setup flow.

## 3. Behavior

### 3.1 A managed file leaves the review only by digest

For every changed path the policy at the head lists in `files`, the review
compares the sha256 of the path's bytes at the head with the digest that
policy records for it. When they are equal, the file is Statecraft's own
rendering and is left out of the diff sent to the reviewer; the input names it
in a `MANAGED (digest-verified, not reviewed)` section and the evidence record
lists it. When they differ, or the file is deleted, it is reviewed like any
other path. A path is never left out by its name alone; `review.exclude`
keeps its prefix meaning unchanged.

Every managed file and the policy are in the authority set (revision 5), so a
change that leaves the review this way still needs the owner's exception.

### 3.2 Deletions are a list with their own cap

A changed file with no added lines (a deleted file, or one whose change only
removes lines) is not sent as a diff. The reviewer receives a
`DELETIONS (summary, not reviewed as text)` section: one line per such file
with its removed-line count, then the totals. The list is bounded by
`review.deletion_cap` estimated tokens (default 20000, range 1000 through
200000). Past it the list is truncated with a line that says so and keeps the
complete totals, and the evidence record says `truncated: true`. Removed lines
inside a file that also adds lines stay in that file's diff, because the
reviewer needs them to read the change.

### 3.3 The cap is in estimated tokens

A token estimate is the byte count divided by three, rounded up. This
over-counts for English and source text, so a budget it fits is fitted by the
real tokenizer too. Two parameters size the budget:

- `review.context_tokens` (default 200000, range 16000 through 2000000): the
  reviewer model's context window. Half of it is the **call budget**, the
  largest diff text one call carries; the other half is left for the prompt,
  the repository context and the answer.
- `review.max_calls` (default 4, range 1 through 16): the cost budget, as the
  most reviewer calls one pull request may take. The **ceiling** is the call
  budget times `review.max_calls`.

Only added lines count against the ceiling. `review.diff_cap` remains a
backstop in added lines: an explicit value (1 through 20000) is kept as spec
017 3.2 requires, and an absent one renders 20000, the maximum.

### 3.4 A change larger than one call is reviewed in groups

Files that add lines are packed in path order into groups whose diff text each
fits the call budget. Each group is one reviewer call carrying the same
repository context, the full list of changed paths, the managed and deletion
sections, and that group's diff. Every call must end with a verdict naming the
head, citing only changed paths, exactly as a single call must. The merged
verdict is `findings` when any group has findings, and its findings are the
union; otherwise it is `no-findings`. One comment carries every group's
answer. A call that fails is classified as a single call's failure is, and
ends the review with that class.

The review is a visible `skipped:oversized` when the added lines exceed
`review.diff_cap`, when the added-line tokens exceed the ceiling, when one
file's diff text exceeds the call budget, or when packing needs more than
`review.max_calls` groups. The evidence record carries `calls`, the addition
tokens, the call budget, the ceiling, the managed list and the deletion
totals.

### 3.5 Ratification is a merge condition

With `governance.require_ratified` (default true), `gate.sh couple` and
`gate.sh couple-group` finish by asking the pinned spec-spine, at the
candidate's tree, which specs own each changed path (`index owner`). A path
owned by a spec whose status at the candidate is `draft` fails the step with
exit 1 and names the path and the spec. The check lives in the coupling steps
because they already run on exactly the two events a merge passes through, a
pull request and a merge-queue entry, and because a base whose `gate.sh`
predates revision 12 runs them without it, so the upgrade to revision 12 is not
judged by a rule its base lacks.

A pull request may move such a spec to `approved`. That is a
**ratification**: a changed `spec.md` whose frontmatter `status` is `approved`
at the head and anything else, or absent, at the base. The governance job's
exception step and `ci-gate.sh` each detect it from the two trees, without
spec-spine and without reading any job's claim, and a ratification requires
the owner exception exactly as an authority change does: on a pull request the
`review-exception` job must succeed for the run; in the merge queue the
exception recorded for the entry's pull request must be `success`; on push it
is reported. Setting `governance.require_ratified` to false removes the
draft-owner refusal only; a ratification still needs the owner exception.

### 3.6 An unreadable base refuses

Only a base that resolves to a commit in the clone and does not carry the file
is the adoption. `ci-gate.sh`, before it reads the policy, and the workflow
steps that read `ci-gate.sh` and `ai-review.sh` at the base each refuse with
exit 2 when a non-empty, non-zero base does not resolve to a commit, on every
event, before any file is read from the candidate. This is the rule the
`governance` and `code` jobs' "Read the gate at the base" steps already apply.

### 3.7 Upgrade behavior

The profile becomes revision 12 with a new identity. A revision-11 project
upgrades through the ordinary plan and apply, and a second plan of the same
selection writes nothing. An explicit `review.diff_cap` is preserved; the new
parameters take their defaults unless declared.

## 4. Out of scope

- Rendering a profile in CI to prove a digest; the recorded digest is the
  evidence, and the owner exception covers the policy that records it.
- The real tokenizer or a provider token-count call.
- A merge-queue-only required job and the acceptance sweep spec-spine's draft
  spec 157 asks for; that is a later revision, after the owner decides who
  selects the acceptance a change can break.
- Ratifying this repository's own drafts; see 5.

## 5. Resolved decisions

**2026-09-27: the owner's decisions this spec encodes.** Digest-verified
managed exclusion, a separate deletion cap, a token cap sized from the model's
context and a cost budget, split review with an `oversized` ceiling, and
ratification as a merge condition with the ratifying pull request and the
owner exception as the only exception. The owner chose one revision for both
changes.

**2026-09-27: the rollout order.** Revision 12's refusal applies from the pull
request after the one that adopts it, because each run reads `gate.sh` at the
base. Spec 007 is `draft` and extends `crates/statecraft-cli/`, so once
revision 12 is on `main` every change to that crate is refused until the owner
ratifies 007. Ratify 007 before, or in, the next pull request that touches
that crate.

## Verification

Each line is one command.

```verify:cli
cargo test -p statecraft-home --lib setup::tests::review_budget_parameters_are_validated_and_defaulted
cargo test -p statecraft-home --test setup_upgrade a_revision_nine_project_upgrades_to_revision_ten
cargo test -p statecraft-home --test setup_upgrade a_revision_eleven_project_upgrades_to_revision_twelve
cargo test -p statecraft-home --test setup_workflows ai_review_classifies_every_case
cargo test -p statecraft-home --test setup_workflows a_managed_file_leaves_the_review_only_by_digest
cargo test -p statecraft-home --test setup_workflows deletions_are_a_list_under_their_own_cap
cargo test -p statecraft-home --test setup_workflows a_change_larger_than_one_call_is_reviewed_in_groups
cargo test -p statecraft-home --test setup_workflows a_draft_owner_refuses_and_a_ratification_needs_the_owner_exception
cargo test -p statecraft-home --test setup_workflows an_unreadable_base_refuses_and_never_reads_the_candidate
cargo test -p statecraft-home --test setup_workflows inverting_a_blocking_branch_of_ai_review_is_noticed
cargo test -p statecraft-home --test setup_workflows inverting_a_blocking_branch_of_ci_gate_is_noticed
```
