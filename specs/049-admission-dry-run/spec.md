---
id: "049-admission-dry-run"
title: "An admission dry-run evaluates a policy through the closed evaluator and writes nothing"
status: approved
implementation: complete
created: "2026-10-21"
summary: >
  Adds a verb that evaluates a revision's recorded verdicts and decisions
  against an admission policy through spec 037's closed evaluator and answers
  what admission would decide, with the same refusal codes in the same order,
  without writing an attestation, a record or any state. Beside the decision
  it reports every check's own result, so an operator previewing a policy
  change sees which requirements allowed and were overruled by a deny. The
  policy-eval/v1 claim is unchanged; whether that trace should ever enter the
  claim is an owner question.
amends:
  - "005-acceptance-and-evidence"
  - "006-command-surface"
# A new module inside an extended crate is reached through that crate's
# edge, as specs 036, 040 and 043 do; a file claim there is in no content
# hash and lint L-008 refuses it.
extends:
  - { spec: "005-acceptance-and-evidence", unit: { kind: directory, path: "crates/statecraft-envelope/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "005-acceptance-and-evidence"
  - "006-command-surface"
  - "037-closed-admission-gate"
# `obligations` is spec-spine's own frontmatter field, used by approved specs
# such as 013, 014 and 018; `make gate` lints it with --fail-on-warn.
obligations:
  - id: "I-1"
    kind: invariant
    text: "For every policy and input, the dry-run's decision, refusal codes and their order equal admission::evaluate's on the same input; the dry-run calls that function rather than a copy of it."
    anchor: "3-2-same-evaluator-same-codes"
  - id: "I-2"
    kind: invariant
    text: "A dry-run writes nothing: no attestation, no run record, no journal entry and no file under the product home or the target."
    anchor: "3-3-no-side-effects"
  - id: "R-1"
    kind: requirement
    text: "The answer names the policy it evaluated by its digest and says whether that digest is the one the bundle's recorded admission names, a different one, or none."
    anchor: "3-1-the-verb"
  - id: "R-2"
    kind: requirement
    text: "The answer lists every registered check with its own result in registration order, and marks an allowing check as overruled when the decision is a refusal."
    anchor: "3-4-the-check-trace"
  - id: "V-1"
    kind: verification
    text: "A differential test over the admission corpus finds the dry-run's decision and codes equal to evaluate's; a run with a read-only home and target succeeds and leaves both byte-identical."
    anchor: "verification"
    inputs:
      - "crates/statecraft-envelope/tests/admission_differential.rs"
---

# 049: Admission dry-run

## 1. Purpose

An operator changing an admission policy today learns what it does by
submitting under it. Spec 037 made the evaluator a pure function with a closed
set of refusal codes, so the question "what would this policy decide for this
revision" can be answered without any effect. preloop's policy simulation
(`backend/preloop/services/policy_simulation.py`, Apache-2.0) answers the same
question for its rule engine and reports the rules that matched but did not
decide; this spec re-implements the idea for a closed gate, where every
requirement is checked and none outranks another.

## 2. Territory

`statecraft-envelope` gains a function that returns `admission::evaluate`'s
`Admission` together with each check's own result (`admission::evaluate_traced`)
and the bundle reading `verify` makes, offered without a pinned root set
(`bundle::admission_inputs`); `statecraft-cli` gains the verb that reads its
inputs, calls it, and renders the answer through the family envelope (spec
007), in the module `crates/statecraft-cli/src/admission.rs` with its suite
`crates/statecraft-cli/tests/admission_dry_run.rs`, both in spec 006's
directory, extended by this spec. No new crate and no new dependency.

## 3. Behavior

### 3.1 The verb

`admission dry-run --bundle <path> --policy <path>` reads a decision bundle
(spec 005) and an admission policy from files and evaluates the bundle's
revision's verdicts and decisions under that policy, collected exactly as
`bundle::verify` collects them to reproduce an admission
(`crates/statecraft-envelope/src/bundle.rs`). There is no stored policy to
default to: `hosted decision --policy` takes the pinned policy from the
caller the same way. The answer names the evaluated policy by its digest and
states whether it is the policy the bundle's recorded admission names:
`recorded`, `different` (a proposed policy being previewed) or `none` (the
bundle carries no admission). It reads files only; no network, no hosted
cell.

### 3.2 Same evaluator, same codes

The decision is `admission::evaluate`'s, called on the same
`AdmissionInput`, so the refusal codes, their order and their serialized form
are exactly admission's. The verb exits 0 when the evaluation admits and with
the finding code when it refuses, as `hosted decision` does for a diverged
bundle; a dry-run that refuses is a successful answer, not a failure of the
verb. An input it cannot read or parse is refused with the existing schema and
not-found kinds.

### 3.3 No side effects

The verb writes no `statecraft/policy-eval/v1` attestation, appends nothing to
any run record or journal, and creates no file. It does not take the
repository lock, because it touches nothing the lock guards.

### 3.4 The check trace

Beside the decision, the answer lists each check the closed gate registered,
in registration order, with its own result: `allow`, `deny` with its refusal
code, or `undecided`. When the decision is a refusal, every allowing check is
marked `overruled`. This is the closed gate's counterpart of preloop's
"also matched" list: no rule wins on priority here, so what an operator needs
to see is which requirements a policy satisfied and which one denied.

The trace is part of the dry-run's answer only. The `statecraft/policy-eval/v1`
claim keeps the shape spec 037 section 3.3 froze: policy, inputs, decision,
reasons.

## 4. Out of scope

- Writing or signing any attestation from a dry-run.
- Evaluating anything but admission: run-time tool policy and confinement
  admission (spec 002) are not this evaluator.
- Changing `policy-eval/v1`; the trace is never recorded in a real admission
  (section 5).

## 5. Resolved decisions

**2026-10-21: the trace stays out of the claim in this draft.** The work order
asked that rules which matched but lost be recorded in the policy-eval/v1
claim. Spec 037 R-2 requires that claim's bytes to equal the pre-037
evaluator's for every input, and a real admission's claim gaining a field is
a change to an attestation shape every existing bundle carries. A dry-run
writes no claim at all, so the trace has nowhere in the claim to go without an
amendment to 037 and 005. This draft keeps the trace in the answer and leaves
the claim to the owner's answer below.

**2026-10-31: the trace stays out of real admissions.** Spec 037 R-2's
byte-equal claim and spec 005's claim shape are kept. Recording overruled
checks in an admission would be a new amending spec with a `policy-eval/v2`
predicate beside v1, proposed on its own if an operator ever needs it. This
spec stands as drafted: the trace is the dry-run's answer and nothing else.

**2026-10-31: the bundle is read, not verified.** The verb takes a bundle and
a policy and no pinned root set, so it resolves the bundle as `bundle::verify`
does (object hashes, the entry chain, every claim) and checks no signature;
`hosted decision` remains the verification. A bundle that does not resolve is
refused with the `validation` kind, a file that does not parse with `schema`,
and a path that names nothing with `not-found`. The trace renders each check
as `{id, result, code?}` with the result words `allow`, `overruled`, `deny`
and `undecided`; a `deny`'s `code` and the answer's `reasons` are the refusal
codes in admission's own serialized form, so their kebab-case keys join the
spec 005 entry of the JSON naming exemption list beside `incomplete-evidence`.

## Verification

Ratified by the owner on 2026-10-31. Acceptance is run through
`make verify SPEC=049`, with no provider, no cell and no network.

```verify:cli
cargo test -p statecraft-envelope --test admission_differential
cargo test -p statecraft-envelope --test decision_bundle
cargo test -p statecraft-cli --test admission_dry_run
cargo test -p statecraft-cli --test family_envelope
```

The differential suite evaluates every case of the admission corpus through
`evaluate` and `evaluate_traced` and requires the same decision, the same codes
in the same order and serialized form, and `overruled` on an allowing check
exactly when the decision refuses (I-1, R-2). The bundle suite shows
`bundle::verify` unchanged by the shared reading. The binary suite runs the
verb against assembled bundles: the digest and its relation to the recorded
admission (R-1), the trace through the envelope (R-2), the codes against
`evaluate`'s (I-1), a run against a read-only home and target that leaves both
byte-identical (I-2, V-1), the refusal kinds, and the usage answers. The
envelope suite shows the verb answers the family envelope under its dotted
name.
