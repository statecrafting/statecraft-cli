---
id: "017-ai-review-diff-cap"
title: "Setup profile revision 10: AI review defaults to a 3000-line diff cap"
status: approved
implementation: complete
created: "2026-09-27"
summary: >
  Amends spec 002's setup profile. github-actions-rust revision 10 raises the
  inherited AI review diff cap from 2000 to 3000 changed lines so ordinary
  substantial pull requests remain reviewable. Explicit operator selections
  remain authoritative and are preserved on upgrade.
amends:
  - "002-environment-lifecycle"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
depends_on:
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
---

# 017: AI review defaults to a 3000-line diff cap

## 1. Purpose

The `github-actions-rust` profile's inherited limit of 2000 changed lines
skips pull requests that are still practical and valuable to review. A
2431-line implementation demonstrated the failure mode: every required check
could be green while the AI reviewer visibly reported `oversized` and did not
review the change. Revision 10 raises the default to 3000 without weakening
the rule that a skipped review is never represented as a completed review.

## 2. Territory

This spec changes the registered profile revision, the default value of
`review.diff_cap`, its upgrade behavior, and tests under
`crates/statecraft-home/`. The rendered workflow and policy remain managed
outputs of spec 002's setup flow.

## 3. Behavior

### 3.1 Default and explicit values

When `review.diff_cap` is absent from a project's recorded setup parameters,
the profile renders `DIFF_CAP: '3000'` and records `diff_cap: 3000` in the
policy. The accepted explicit range remains 1 through 20000. An explicit value,
including 2000, remains an operator choice and is not rewritten.

### 3.2 Upgrade behavior

The profile becomes revision 10 with a new identity. A revision-9 project that
inherited the old default and therefore has no `review.diff_cap` parameter
upgrades to 3000. A project that explicitly recorded 2000 stays at 2000. This
distinction lets the product repair its default across adopters without
silently overriding local policy.

Statecraft CLI itself has no explicit cap in its recorded setup parameters, so
its managed installation upgrades to revision 10 and renders 3000 through the
same plan and apply path adopters use.

### 3.3 Review semantics do not change

A diff above the selected cap remains a visible `oversized` skip. The wrapper
job may complete successfully, but its evidence remains `skipped`, never
`reviewed`, and a green aggregate gate does not assert that substantive review
occurred. Release-candidate and owner-exception rules are unchanged.

## 4. Out of scope

- Raising the maximum accepted cap above 20000.
- Splitting or otherwise reducing an oversized pull request.
- Rewriting an adopter's explicit `review.diff_cap` selection.
- Claiming that a pull request above 3000 lines received substantive review.

## 5. Resolved decisions

**2026-09-27: 3000 is the family default, not a repository-only override.** The
owner selected one value across the setup profile and Statecraft CLI's own
managed installation. Encoding it as revision 10 makes future initialization
and managed upgrades converge on the same contract.

**2026-09-27: inherited and explicit values remain distinguishable.** The
setup selection stores only declared parameters. An absent key therefore means
the profile default may evolve, while a present key records operator intent and
survives an upgrade.

## Verification

Each line is one command.

```verify:cli
cargo test -p statecraft-home --lib setup::tests::diff_cap_defaults_to_three_thousand
cargo test -p statecraft-home --test setup_upgrade a_revision_nine_project_upgrades_to_revision_ten
```
