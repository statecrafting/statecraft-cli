---
id: "012-remote-desired-state"
title: "A setup profile declares remote desired state and doctor compares it without writing"
status: approved
implementation: in-progress
created: "2026-09-26"
summary: >
  Amends 002's setup-profile and remote-doctor contract. A profile renders a
  versioned desired remote-state document covering required checks, extra jobs,
  code-owner review, merge queue, the review-exception environment and its
  reviewers, workflow-token defaults, secret names, repository custom
  properties, and profile identity. doctor --remote compares every field
  independently and never writes remote state or reads a secret value.
amends:
  - "002-environment-lifecycle"
extends:
  - { spec: "001-boundaries-and-authority", unit: { kind: file, path: "docs/decisions/00-founding-decisions.md" }, nature: additive }
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-environment/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
  - "006-command-surface"
---

# 012: Remote desired state

## 1. Purpose

The `github-actions-rust` profile already states remote obligations in prose and
`doctor --remote` reads a subset through GitHub. That is not a versioned desired
state: it cannot identify every expected remote field, distinguish lack of host
support from lack of authority, or compare the profile's merge-queue and
custom-property expectations.

This amendment makes the declaration exact and the comparison complete while
preserving the boundary in spec 002: Statecraft performs no remote write. An
external, owner-operated applier may consume the declaration only after its own
authorization.

## 2. Territory

This spec owns no product code. A later implementation reaches spec 002's
profile, manifest, and diagnostic code through the `extends` edges above, and
reaches spec 006 only to bind the additive report through the existing
`doctor --remote` command. This draft also extends spec 001's decision-record
file only to preserve the proposed roadmap. It does not adopt any decision.

## 3. Behavior

### 3.1 The desired-state document

Each setup profile renders a committed, canonical JSON desired-state document.
Its identity is included in the profile identity, and the manifest records its
path, digest, profile id, profile revision, and schema version. The document has
these independently comparable fields:

| Field | Required content |
|---|---|
| `profile` | Profile id, revision, and identity. |
| `defaultBranch` | The exact branch the rules target. |
| `requiredChecks` | Each required check name and required GitHub App identity, including `ci-gate`. |
| `extraRequiredJobs` | Every enabled `ci.extra_required_jobs` entry and the required aggregate result. |
| `codeOwnerReview` | Whether review is required and the governed path set whose owners must be represented by the rendered `CODEOWNERS`. |
| `mergeQueue` | Required or not required, merge method, build concurrency, and whether all queued entries must pass. |
| `reviewException` | Environment name, required reviewer identities, and prevention of self-review. |
| `workflowToken` | Default permission mode and whether Actions may create or approve pull requests. |
| `secrets` | Required secret names and allowed visibility classes, never a value. |
| `customProperties` | Expected repository property names and exact values, with the organization schema named separately. |

Order is deterministic. An omitted field means the profile makes no claim
about it; omission is not a default and `doctor --remote` reports it
`unverified`. Unknown document fields refuse planning rather than being
silently ignored.

The document declares expectations only. It is not a Terraform state file, an
apply plan, proof of remote configuration, or authority to change a repository.

### 3.2 Read-only comparison

`doctor --remote` reads the desired-state document and compares each leaf field
with read-only host operations. It emits one result per leaf and a summary that
does not replace those results. Each result has exactly one state:

| State | Meaning |
|---|---|
| `matching` | The host answered and the observed value equals the declared value. |
| `drifted` | The host answered and the observed value differs or is absent. |
| `unavailable` | The host or resource could not be reached for a reason that is not an authorization refusal. |
| `unauthorized` | The host refused the credential's read authority. |
| `unsupported` | The host, repository plan, or API does not expose or support the declared capability. |
| `unverified` | No comparison was possible without guessing, including an omitted desired value. |

One unavailable or unauthorized field does not suppress comparison of another.
The report retains the host's field identity and a sanitized reason. It never
includes a credential, secret value, authorization header, or response body
that can contain one. Secret comparison asks only whether each declared name is
visible and, where the host exposes it, its visibility class.

Plan mode and local `doctor` perform no remote read. `doctor --remote` performs
no remote write, never changes the manifest, and never repairs drift. Its exit
is a finding when any declared field is not `matching`, while the per-field
state remains the authoritative explanation.

### 3.3 Comparison rules

1. Required checks compare the name and App identity. A context of the right
   name from an unbound source is `drifted`.
2. Extra required jobs compare both their declaration and their required
   aggregation by `ci-gate`; a workflow file alone is insufficient.
3. Code-owner review compares the protection setting and the applicable
   `CODEOWNERS` declaration independently.
4. Merge queue compares its requirement and configuration. A plan that does
   not offer merge queues reports `unsupported`, not `matching` and not
   `drifted`.
5. The exception Environment compares existence, each required reviewer, and
   self-review prevention separately. One reviewer satisfying a deployment at
   runtime does not establish that every declared reviewer is configured.
6. Workflow-token defaults compare repository policy and any stricter
   organization or enterprise policy the host exposes. An inherited value is
   named as inherited.
7. A required secret may be satisfied by repository, organization, or
   enterprise visibility exactly as the declaration permits. Only its name and
   visible scope are read.
8. Custom-property values compare at the repository. A missing organization
   property schema is distinct from a missing repository value.

### 3.4 External application boundary

Statecraft does not contain, invoke, generate credentials for, or vendor a
remote-state applier. Two owner-operated options are compatible with this
document:

| Property | Terraform module | Small `gh api` applier |
|---|---|---|
| Location | Separate owner-selected infrastructure repository. | Separate owner-selected repository or reviewed operator script outside this product. |
| Credentials | GitHub App or token with the exact repository and organization administration permissions required by selected fields. | The same least authority, supplied by the operator at execution time. |
| State | Terraform state in an owner-selected protected backend. | No implicit state; save the reviewed input, normalized pre-state, operations, and normalized post-state as an evidence bundle. |
| Drift | Terraform plan plus `doctor --remote`. | `doctor --remote`, followed by an explicit dry comparison before mutation. |
| Rollback | Reviewed reverse plan from protected prior state; provider limitations remain visible. | Generated from the captured pre-state and reviewed as a second explicit operation set. |
| Auditability | Plan, apply identity, state revision, actor, and GitHub audit events. | Script identity, desired-state digest, exact requests excluding credentials, actor, before and after reads, and GitHub audit events. |
| Consequence | Strong reusable state and review, with backend and provider lifecycle overhead. | Small surface and transparent calls, with custom rollback and convergence logic to maintain. |

The recommended default is Terraform for repeated multi-repository management.
The small applier is appropriate for a bounded pilot only if the owner prefers
its lower setup cost and accepts its explicit state and rollback burden. No
choice is made by this draft.

### 3.5 GitHub capability limits are input facts

The comparison and any external plan must preserve these current GitHub limits
as capability facts rather than converting them into local policy:

- merge queues are available for public organization repositories and for
  private organization repositories on GitHub Enterprise Cloud;
- Environment required reviewers on GitHub Free, Pro, or Team are available
  only for public repositories;
- repository rulesets are available for public repositories on Free plans and
  for public or private repositories on Pro, Team, and Enterprise Cloud, while
  organization-level rulesets require Team or Enterprise;
- organization owners define custom-property schemas, while repository actors
  can set values only when that schema permits it;
- organization or enterprise policy can override repository workflow-token
  defaults.

Sources reviewed for this draft are GitHub's current documentation for
[merge queues](https://docs.github.com/en/enterprise-cloud@latest/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue),
[environments](https://docs.github.com/en/actions/reference/workflows-and-actions/deployments-and-environments),
[rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets),
[custom properties](https://docs.github.com/en/organizations/managing-organization-settings/managing-custom-properties-for-repositories-in-your-organization),
[code owners](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners),
and [Actions settings](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/enabling-features-for-your-repository/managing-github-actions-settings-for-a-repository).
Documentation is evidence for capability planning, not proof of any target's
current plan or settings.

### 3.6 Observable negative cases

| Case | Required result |
|---|---|
| Host is unreachable | Every field not otherwise answered is `unavailable`; no write is attempted. |
| Credential lacks one read permission | That field is `unauthorized`; other fields are still compared. |
| Secret name is absent | `drifted`; no secret value is requested. |
| Feature is absent from the repository plan | `unsupported`, with the feature and plan limitation named. |
| Check name exists from the wrong App | `drifted`. |
| Desired-state identity differs from the selected profile | Local corpus finding before a host is asked. |
| Host adds an undeclared setting | Reported as observed extra state, never silently added to desired state. |
| Operator asks Statecraft to apply | Usage refusal: no such operation exists. |

## 4. Out of scope

- Implementing or running Terraform or a `gh api` applier.
- Selecting its repository, credential, state backend, or operator.
- Inspecting secret values or proving provider use from secret presence.
- Mutating protections, rulesets, environments, Actions settings, secrets,
  custom properties, or merge queues.
- Re-rendering this repository's profile or changing a consumer.
- Qualifying GitHub availability for a target without a fresh host comparison.

## 5. Resolved decisions

### 2026-10-10: choices the implementation made where this spec is silent

1. **Revision and location.** The document is a new rendered file, so it is
   profile revision 16 of `github-actions-rust`: a changed byte is a different
   revision. It renders at `.statecraft/setup/github-actions-rust.remote.json`
   beside the policy, with schema `statecraft/remote-desired-state/1`, is a
   member of the authority set, is listed in the policy's `files`, and is one
   of the governed paths `CODEOWNERS` covers. The profile identity commits to
   the document's parameter-independent part. The manifest records the path,
   digest and schema in the selection's `remoteState`; the profile id and
   revision are the selection's own fields and the entry's source identity.
   Canonical means sorted keys, two-space indentation and a final newline.
2. **Where declared values come from.** Fields the profile already fixes are
   rendered from it: `ci-gate` from GitHub Actions (app id 15368), the
   declared extra jobs aggregated by `ci-gate`, code-owner review required
   over the profile's owned paths with `review.code_owners` as the required
   owners when declared, the exception Environment with self-review
   prevented, a `read` workflow token that may not approve pull requests, and
   one secret requirement satisfied by `ANTHROPIC_API_KEY` or
   `CLAUDE_CODE_OAUTH_TOKEN` at repository or organization visibility. Three
   setup parameters declare what the profile cannot know:
   `remote.merge_queue`, `remote.exception_reviewers` and
   `remote.custom_properties`. Each is closed (an unknown key refuses the
   plan), and each omitted one is omitted from the document, so `doctor
   --remote` reports it `unverified` and not a finding.
3. **The local finding, and what a pre-16 selection gets.** A document whose
   profile claim differs from the selection, whose bytes differ from the
   recorded digest, that fails to parse, or that is absent after the
   selection rendered it, is a local finding: no host is asked, and the six
   results' four remote rows are `unverified` with that reason. A selection
   from before revision 16 records no document and gets a note, not a
   finding.
4. **Host reads.** Required checks and code-owner review are read from both
   classic branch protection and the rulesets that apply to the default
   branch; a match from either is `matching`, and when neither matches while
   one source failed, the failure's state is reported rather than `drifted`.
   The `profile` leaf compares the policy on the default branch, and an extra
   job compares the policy's required declaration and the rendered workflow's
   `ci-gate` `needs`, both read through the contents API. `CODEOWNERS` is read
   at its three locations; exact paths, directory prefixes, `*`, `/dir/*` and
   `/dir/**` are evaluated with last-match-wins, and any other pattern that
   could decide a governed path makes that leaf `unverified`.
5. **Capability classification.** A personal repository makes merge queue and
   custom-property leaves `unsupported`, and makes Environment reviewers on a
   private repository `unsupported`. A private organization repository
   without required reviewers is `unverified`, because the organization's
   plan is not visible to the read. A `gh` failure is classified from its
   error stream alone: HTTP 404 is not found, 401 or 403 naming a plan
   upgrade is `unsupported`, other 401 or 403 is `unauthorized`, anything else
   is `unavailable`. The reason keeps only the HTTP status, never a response
   body, header or credential.
6. **Exit and surface.** `doctor --remote --json` carries the comparison as
   `remoteState` beside the six results: the document, its schema, any local
   finding or note, one result per leaf with `declared`, the expected and
   observed values and a reason, observed undeclared `extras`, and a
   per-state summary. The exit is a finding when the local diagnostic has
   findings, when the document is a local finding, or when any declared leaf
   is not `matching`. `--apply`, with or without `--remote`, is the usage
   refusal the argument parser already gives.

## Verification

The library suite renders the document through the profile and checks its
fields, canonical bytes, manifest record and closed parameters, and drives
the comparison against a fake host for every leaf and each negative case of
section 3.6: an unreachable host, one unauthorized read, an absent secret, a
plan without the feature, a check from the wrong App, a document that
disagrees with the selection, undeclared extra state, and omitted fields. The
binary suite renders the document through `init apply`, runs `doctor --remote
--json` against a stub `gh` that answers only reads, checks the per-field
report, the finding exit and an unchanged tree, and checks that `--apply` is
a usage refusal and that local `doctor` asks no host.

```verify:cli
cargo test -p statecraft-home --test remote_desired_state
cargo test -p statecraft-cli --test setup_profile
cargo test -p statecraft-home --test setup_upgrade
```
