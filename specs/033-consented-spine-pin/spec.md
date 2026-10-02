---
id: "033-consented-spine-pin"
title: "A reviewed initialization plan may explicitly move the adopted spec-spine pin"
status: approved
implementation: pending
created: "2026-10-02"
summary: >
  Adds an explicit exact --spine request to init plan and init apply. The
  source and destination pins, producer identity, original configuration
  digest and minimal authored edit are part of the consented plan identity.
  Apply changes only the existing required_version value in spec-spine.toml
  after revalidating the plan, then uses the ordinary managed upgrade rules.
  No implicit pin move, dependency rewrite, ratification or branch protection
  change is authorized by initialization.
amends:
  - "002-environment-lifecycle"
  - "006-command-surface"
  - "018-governed-bootstrap-inputs"
  - "029-one-resolved-judge"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-environment/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "002-environment-lifecycle"
  - "006-command-surface"
  - "018-governed-bootstrap-inputs"
  - "029-one-resolved-judge"
  - "032-external-code-profile"
obligations:
  - id: "R-1"
    kind: requirement
    text: "An exact --spine request is explicit consent input, never an inferred or default pin move."
    anchor: "3-1-exact-request"
  - id: "R-2"
    kind: requirement
    text: "The plan identity binds the original configuration digest, source and destination pins, producer identity, authored edit and all managed upgrade actions."
    anchor: "3-2-reviewed-plan"
  - id: "R-3"
    kind: requirement
    text: "Apply revalidates the complete plan before writing and changes only the existing required_version value, preserving all other authored configuration bytes."
    anchor: "3-3-minimal-apply"
  - id: "R-4"
    kind: requirement
    text: "Pin consent neither ratifies specs nor permits an incompatible base gate, secret transmission, dependency rewrite or branch protection change."
    anchor: "3-4-authority-boundary"
---

# 033: Consented spec-spine pin

## 1. Purpose

A fleet upgrade currently requires an operator to edit the adopted pin before
initialization can render the new profile. The operator cannot review the pin
move and its managed consequences as one plan. This draft proposes that explicit
pin request as an initialization input. It does not claim implementation,
acceptance, release or owner ratification.

## 2. Territory

This spec owns no new product unit. Its extends edges name the initialization
planner and applier in statecraft-home, manifest records in
statecraft-environment, and the command surface in statecraft-cli. It amends
spec 002's authored-file protection for this one consented value edit, spec
006's argument surface, spec 018 sections 3.3 to 3.5's adopted-pin rule, and
spec 029's resolution only as section 3.2 below specifies. It builds on spec
032's merged profile revision 14, including its external code selection. The
existing managed-file drift rules and project authority remain in force.

## 3. Contract

### 3.1 Exact request

`init plan <path> --spine '=X.Y.Z'` requests one exact release. The syntax is
`=` followed by three decimal components, each either zero or a nonzero digit
followed by digits. Whitespace, leading zeros, prerelease and build suffixes,
ranges, wildcards and missing components refuse. Both `--spine <value>` and
`--spine=<value>` are accepted on these two verbs only; repeated or missing
options are usage errors (exit 3). An invalid or unsupported request is a
precondition refusal (exit 2).

The request must equal the linked producer's spec-spine release. Rendering an
older profile, bridge mode and changing the producer dependency are separate
proposals. Without --spine existing behavior remains unchanged: an incompatible
adopted pin withholds the profile, and an unavailable judge is reported rather
than silently replaced. A profile must be selected, by the existing declaration,
--profile or --setup-input, for an explicit --spine request; otherwise it refuses.

The target must already have a regular, contained `spec-spine.toml` with one
unambiguous string-valued `[meta].required_version` key. The source value must
itself be an exact release in the syntax above. Missing keys or files, invalid
TOML, duplicate definitions, non-UTF-8 input, dotted or inline-table definitions,
escaped, literal or multiline value strings refuse with the key and reason.
Supported input is a single-line double-quoted value under an explicit `[meta]`
table; indentation, spacing, inline comments on the table or value and LF or
CRLF are preserved. Every pin consumer needed by this flow, including the
rendered installer, must read that supported representation consistently and
compare exact identities. The command does not recreate an adopted
configuration or replace its other keys. A request already equal to the
adopted pin is an explicit no-op for that key.

### 3.2 Reviewed plan

Planning writes nothing. Its human and JSON reports name the current pin,
requested pin, linked producer release and identity, original configuration
digest, exact authored value edit, rendered profile identity, managed writes
and withheld paths. The plan identity binds all of these inputs and actions,
including the explicit --spine request, its presence even for a same-pin request,
setup input bytes or absence, effective parameters, prerequisite observations,
manifest bytes or absence, observed target bytes and the ordered initialization
actions outside the setup profile. The identity also binds the canonical target
root and the selected judge's absolute path, version and executable digest.
No profile-only identity may authorize an additional initialization action.

The planner evaluates initialization against the proposed configuration bytes
without requiring the operator to edit the file first. It reports remote
settings obligations, required owner decisions and any inability to qualify a
base-gate transition; it never presents a bridge requirement as a passing gate.
A request outside the producer's supported release refuses a plan rather than
producing an identity whose apply would fail after writing the pin. Unrelated
unmet prerequisites still withhold the profile and keep initialization partial;
the explicit pin edit remains visible as an independently consented action.

For this request alone, resolution uses the proposed exact pin as an in-memory
input. It retains spec 029's candidate order and exclusive supervisor or
operator override, requires a readable executable digest and the requested
version even for a supervisor selection, and never downloads, installs or
rewrites launcher state. No admitted executable means refusal before target
writes, with the requested version and preparation remedy. The same selected
absolute executable is used throughout apply, with its identity rechecked
before effects. Planning does not compile or index, temporarily rewrite the
configuration, or claim that the actual target already passed a corpus gate.

### 3.3 Minimal apply

`init apply <path> --spine '=X.Y.Z' --plan <identity>` must recompute the same
plan and require the same request and identity, including for a same-pin request.
Omitting or changing the request, changing the configuration bytes, producer,
judge, manifest, rendering inputs or action set refuses before target writes.
The explicit-request identity cannot be reused on an invocation without --spine.
No pin move occurs without both the explicit request and its reviewed identity.
The existing manifest lock remains mandatory. Lock and runtime staging effects
are reported and grant no consent to target or product-home writes; a stale
plan must refuse before those writes.

The authored operation replaces only the existing required_version string
value. Comments, spacing outside that value, key order, unrelated values and
newline convention remain byte-identical. An edit that cannot preserve those
bytes refuses. The replacement respects containment and refuses symlinked
configuration and linked ancestors below the canonical root. It is staged on
the target filesystem and renamed atomically after revalidating containment,
file type, original bytes and the reviewed action set under the manifest lock.
A same-pin request neither rewrites nor reformats the configuration.

The environment records the adopted pin and consented plan identity only after
the corresponding write succeeds, or after verifying the no-op value. The
manifest records the plan identity with the pin disposition, not merely in
runtime progress state. The normal managed-file matching and drift rules still
decide every subsequent write. An interrupted or partial apply reports which
operations completed, never claims full convergence, and can be replanned from
the actual tree; a prior identity must not authorize a different remaining plan.

A failed pin write stops subsequent initialization writes and cannot advance
the manifest's pin or consent record. If the pin write succeeds but recording
fails, the report names both outcomes; the old manifest is not evidence that
the file reverted. A new same-pin plan may reconcile that actual state without
rewriting the pin. After any completed operation changes the reviewed inputs,
reuse of the old identity refuses, including on a resumed or already-satisfied
request. Reports distinguish withheld operations from failed operations under
the existing exit contract. This adds no cross-file rollback promise.

This amendment adds no `init --replace` flag. Drifted managed files remain
withheld, and any existing per-path replacement consent is exercised through
the separately reviewed environment operation. That operation changes the
tree and therefore requires replanning initialization. Adopted, foreign and
user files remain protected; the pin value edit is the only authored exception.

### 3.4 Authority boundary

A pin request consents only to the reported authored value edit and ordinary
initialization actions in that plan. It grants no spec approval, waiver,
execution posture, signing authority, secret transmission, remote settings
mutation, Cargo or npm dependency rewrite, or branch-protection bypass.
Repositories whose trusted base gate cannot run the proposed engine still
require a separately reviewed bridge before the pin move can land.

## 4. Acceptance criteria

- A mismatched adopted pin retains existing withheld-profile behavior without
  --spine and yields a read-only plan with an exact supported request and judge.
- Range, wildcard, incomplete, duplicate-key, unsupported release, absent-key
  and symlink cases refuse with no target mutation.
- Two configurations differing only in an unrelated comment have different
  consent identities; neither identity authorizes the other configuration.
- Omitting or changing --spine, setup inputs, manifest, producer or judge
  identity, configuration bytes or another planned action refuses before
  target and product-home writes. Runtime lock effects are reported separately.
- A successful move preserves every configuration byte outside the value span,
  including comments and newline convention, and records the new pin and consent.
- A same-pin request performs no authored write; an interrupted apply reports
  actual completion and requires a newly reviewed plan for remaining actions.
- Managed drift remains withheld and a separate replacement invalidates consent;
  no dependency, spec lifecycle, secret or remote protection is changed.
- Missing profile or judge, wrong-version exclusive overrides or supervisor
  selections, and changed executable bytes refuse without acquisition. Both
  supported option forms work; duplicate or missing options report usage.
- Unsupported TOML forms refuse; supported whitespace and inline comments are
  read consistently by planner, resolver, manifest pin reader and installer.
- Failure before rename leaves the old pin; failure after rename before manifest
  recording reports the new file and old record. Failure in a later operation
  reports completed actions, leaves the new pin, and requires a newly reviewed
  plan. Replanning recovery never rewrites a same-pin configuration.
- Rust and external code profiles retain their required checks and withholding
  rules after the move; genuine corpus or local-check failure stays visible.

## 5. Resolved decisions

Exact producer equality keeps this proposal independent of older profile
rendering and bridge mode. Preserving the authored configuration by a single
value edit makes the consent reviewable and avoids reformatting an adopted
user's file. A separate environment replacement keeps this amendment focused
on one authored value rather than introducing another drift consent surface.
Effective-pin resolution enables a read-only preview while retaining one judge,
exclusive overrides and the prohibition on acquisition. Acceptance criteria
are proposed behavior, not evidence that the implementation exists.

## Verification

The implementation adds the two acceptance suites below. Each exercises the
real plan/apply boundary; the binary suite also checks human and JSON reports
and exit classes. Failure injection covers the atomic pin write, manifest
recording and a subsequent initialization operation. Existing suites guard
ordinary initialization, exact resolution and revision 14 external code.
These commands are the proposed acceptance, not passing evidence on this draft.

```verify:cli
cargo test -p statecraft-home --test consented_spine_pin
cargo test -p statecraft-cli --test consented_spine_pin
cargo test -p statecraft-home --test spec_spine_resolution
cargo test -p statecraft-home --test bootstrap_inputs
cargo test -p statecraft-cli --test bootstrap_inputs
cargo test -p statecraft-home --test external_code
cargo test -p statecraft-home --test setup_upgrade
```
