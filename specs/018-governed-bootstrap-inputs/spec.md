---
id: "018-governed-bootstrap-inputs"
title: "Governed inputs for a complete first bootstrap"
status: approved
implementation: complete
created: "2026-09-27"
summary: >
  Amends 002's initialization contract and 009's producer call so a fresh
  repository can supply setup-profile parameters through one explicit,
  plan-bound input document, receives the exact governance-producer pin from
  the producer itself, and remains honestly partial when project-owned
  prerequisites are absent.
amends:
  - "002-environment-lifecycle"
  - "009-governance-producer"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
  # The acceptance script's stub spec-spine answers as the release a fresh
  # scaffold now pins exactly (section 3.3); the owner chose on 2026-09-28 to
  # carry that change here rather than in its own pull request.
  - { spec: "002-environment-lifecycle", unit: { kind: file, path: "scripts/acceptance/managed-session.sh" }, nature: corrective }
depends_on:
  - "002-environment-lifecycle"
  - "006-command-surface"
  - "009-governance-producer"
  - "010-profile-unresolved-claims"
  - "011-init-bootstrap-spec"
obligations:
  - id: "I-1"
    kind: invariant
    text: "Planning and application resolve the same setup input bytes, effective parameters, producer identity, and project prerequisites, all bound by the plan identity."
    anchor: "3-5-plan-identity-application-and-convergence"
  - id: "I-2"
    kind: invariant
    text: "Statecraft never invents, copies, or silently weakens a project-owned prerequisite to make setup appear complete."
    anchor: "3-4-project-owned-prerequisites"
  - id: "R-1"
    kind: requirement
    text: "A fresh repository may supply a closed setup-profile parameter document before any project declaration exists."
    anchor: "3-1-the-bootstrap-input-document"
  - id: "R-2"
    kind: requirement
    text: "A newly scaffolded spec-spine.toml carries the exact version of the linked governance producer through its supported pinned-scaffold option."
    anchor: "3-3-the-governance-pin"
---

# 018: Governed inputs for a complete first bootstrap

## 1. Purpose

The setup profile already validates a closed parameter set, but a fresh target
has no `.statecraft/environment.json` from which initialization can read those
parameters. The command accepts a profile name and plan identity only, so the
first plan necessarily uses defaults. An operator cannot request code owners,
coverage enforcement, authored-content checks, per-commit gating, or signed
commit enforcement without hand-editing managed state after initialization.

The profile also requires an exact spec-spine pin, while Statecraft currently
asks the linked producer for its backward-compatible unpinned scaffold. The
producer already supplies a supported pinned-scaffold option. Statecraft does
not need to edit producer bytes or become a second scaffold author.

This amendment supplies the missing pre-plan input and selects that producer
option. It does not make Statecraft the author of a target's Rust workspace or
content policy, and it does not turn a partial initialization into success.

## 2. Territory and measured gap

This spec owns no product code. A later implementation reaches spec 002's home
flow and spec 006's existing `init plan` and `init apply` commands through the
`extends` edges above, and spec 002's acceptance script's stub
`spec-spine` through the third, because an exact pin no longer admits the
stub's old version. It amends spec 009 only to select an existing supported
producer call; the contract path set and byte-for-byte reconciliation remain
unchanged.

The motivating reproduction is the doc-manus first bootstrap against
Statecraft commit `d77e011e39dbc4689e49492c4e528d860be7eb86`. Its plan exposed
only setup defaults, generated an unpinned `spec-spine.toml`, and withheld the
profile because `rust-toolchain.toml`, `Cargo.lock`, and a declared
authored-content checker were absent. Current `main` at
`c95b7eee37a55ae83fd57ac1fca241637c42763b` still accepts no source for fresh
setup parameters and still calls `scaffold_init_json` rather than the
producer's pinned option.

Repeated-plan reconciliation defects observed in that reproduction are not a
new requirement. Spec 002 already requires safe repetition, one plan shared by
preview and apply, authored-file protection, and convergence. They remain
corrective implementation work under spec 002.

## 3. Behavior

### 3.1 The bootstrap input document

`init plan <path>` and `init apply <path>` accept
`--setup-input <file>`. The file is an operator-supplied, read-only input. It is
not copied into the target, added to the environment manifest, or treated as a
secret store. Its complete version-1 shape is:

```json
{
  "schema": "statecraft/setup-input/1",
  "profile": "github-actions-rust",
  "parameters": {
    "review.code_owners": ["@owner"],
    "governance.enforce_coverage": true,
    "governance.authored_content": "scripts/check-authored-content.sh",
    "governance.authored_content_text": true,
    "governance.gate_each_commit": true,
    "governance.require_signed_commits": true,
    "governance.fail_on_unresolved": true
  }
}
```

`schema`, `profile`, and `parameters` are required. Unknown top-level members,
an unknown schema, a non-object `parameters`, or a parameter the selected
profile does not know refuses planning. Parameter values use the profile's
existing validation and defaults. The document contains policy choices only:
credentials, tokens, secret values, remote observations, consent, and plan
approval are invalid members.

The file may be outside the target so an empty repository need not contain a
temporary control file. Its path is resolved explicitly and no relative path
inside it is resolved against the input file's directory. Repository-relative
parameter values remain relative to the target root.

`--profile <id>` remains supported without an input document and retains its
existing default behavior. When both options are present, their profile ids
must be equal or planning refuses. `--setup-input` without `--profile` selects
the document's profile. A recorded setup selection and a supplied input must
name the same profile; the supplied parameters are a requested upgrade under
spec 002's existing managed reconciliation rules.

### 3.2 Effective parameters are visible before writing

The plan reports the input schema, input byte digest, selected profile,
profile revision and identity, every effective parameter including defaults,
and parameter provenance as `setup-input`, `recorded`, or `default`. Human and
JSON output project the same value. The report does not reproduce the full
input bytes.

An absent input on a fresh repository is not an error. The plan explicitly
reports that defaults are in effect and that no pre-plan parameter source was
supplied. It never describes default code owners, disabled checks, or an absent
authored-content policy as the operator's choice.

### 3.3 The governance pin

When initialization will create `spec-spine.toml`, Statecraft calls
`spec_spine_core::scaffold_init_opts_json` with the same explicit layout spec
009 requires and `{"pinExactVersion":true}`. The returned file carries
`[meta] required_version = "=<linked producer version>"`. Statecraft writes
those returned bytes unchanged, records the same exact producer identity in
the declaration, and verifies the two identities agree before any write.

An existing `spec-spine.toml` remains adopted and is never rewritten. Its pin
must already be exact and must admit the linked producer when the selected
profile requires an exact pin. An absent, ranged, malformed, or mismatched pin
is one unmet prerequisite and names the observed value. Selecting another
binary from `PATH`, rewriting an adopted file, or replacing the linked
producer is not a repair.

The unpinned producer call remains available to consumers that did not request
this Statecraft bootstrap behavior. This amendment changes Statecraft's call,
not the producer's default.

### 3.4 Project-owned prerequisites

Each selected profile declares prerequisites together with an ownership
disposition. For `github-actions-rust`:

| Prerequisite | Owner | Initialization behavior |
|---|---|---|
| exact `spec-spine.toml` pin | governance producer for a new file; target owner for an existing adopted file | Request the producer's exact pin for a new scaffold; inspect but never rewrite an adopted file. |
| `rust-toolchain.toml` | target project | Require a regular tracked file; never generate, copy, or select a toolchain. |
| `Cargo.lock` | target project | Require a regular tracked file; never generate it or run a dependency resolver. |
| declared authored-content checker | target project and its owning spec | Require the exact repository-relative regular executable file when the parameter is set; never invent or copy a policy. |
| Git work tree | target project | Require it; never initialize Git. |

Planning inspects every prerequisite and reports each independently as `met`
or `unmet`, with its owner and the consequence. One unmet prerequisite
withholds all profile-owned files so the repository never receives a gate that
cannot run. Governance scaffold, instruction bridge, corpus compilation, and
registration may still complete as spec 002 permits, but the overall init
outcome is `partial` and the profile step is `withheld`.

The report retains the requested effective parameters even while the profile
is withheld. It does not record a setup selection in the target manifest until
the profile files are applied, because an unapplied request is not installed
state. A retry therefore needs the same input document or another explicitly
reviewed document; Statecraft does not hide pending authority in runtime state.

No option skips, downgrades, or marks a prerequisite met. In particular,
`governance.fail_on_unresolved` keeps its `true` default and changes only when
the input explicitly supplies the profile's already governed parameter.

### 3.5 Plan identity, application, and convergence

The setup plan identity commits to:

1. the input schema and exact byte digest, or explicit absence;
2. selected profile id, revision, identity, and effective parameters;
3. linked governance-producer identity and pinned scaffold bytes;
4. every prerequisite observation;
5. every target file observation used by reconciliation; and
6. the complete ordered action set.

`init apply` with `--setup-input` requires `--plan <identity>`, recomputes the
plan from the same input path, and refuses before writing when any committed
fact differs. Supplying `--setup-input` to apply without an approved plan is a
usage refusal. An apply without a setup input retains spec 002's existing
behavior.

A successful exact-plan apply records the normalized effective parameters in
the manifest, runs local profile verification only when `--verify-local` was
supplied, and leaves no pending action. A second plan with the same input and
unchanged target has the same identity, reports every managed file unchanged,
and proposes zero target writes. A partial apply may change completed
governance paths; its next plan reports only actions still required and never
rewrites an authored or already reconciled file.

### 3.6 Observable negative cases

| Case | Required result |
|---|---|
| Input changes after review | Apply refuses the stale plan before every write and names the input digest change. |
| Input and `--profile` disagree | Usage refusal; neither profile is selected. |
| Fresh input contains an unknown parameter | Plan refusal from the selected profile's closed validator. |
| Input carries a credential or unknown top-level member | Plan refusal; the value is not echoed. |
| Existing adopted config has no exact pin | Profile withheld and init partial; the adopted file is unchanged. |
| Producer returns a pin different from its linked identity | Initialization refuses before every write. |
| Authored-content checker is absent, non-regular, outside the target, or not executable | Profile withheld with the exact path and failed property named. |
| Rust prerequisite is absent | Profile withheld; Statecraft creates neither file and runs no resolver. |
| Setup is withheld after governance succeeds | No setup selection is recorded; requested parameters remain visible only in the report. |
| Successful apply is planned again | Same plan identity, zero target writes, every managed file unchanged. |

## 4. Out of scope

- Choosing a target's code owners, toolchain, dependencies, or content policy.
- Generating `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, or an
  authored-content checker.
- Repairing the already governed reconciliation defect by changing this
  amendment's requirements.
- Ratifying the generated bootstrap spec or any other draft.
- Applying remote settings, creating credentials, reading secret values,
  arming, enrolling, invoking a provider, or publishing a release.
- Changing spec-spine's default scaffold behavior or accepting a filesystem
  dependency on an unreleased producer.

## 5. Resolved decisions

**2026-09-27: the input is a document, not repeated key-value flags.** One
versioned document has one reviewable digest, uses the profile's existing
closed validator, and lets plan and apply bind the same bytes. Repeated flags
would need their own ordering, duplicate-key, JSON-value, and shell-quoting
contract.

**2026-09-27: withheld intent is reported, not persisted.** The environment
manifest records installed state. Persisting requested authority before the
profile exists would make a partial apply indistinguishable from an installed
selection and would let a retry act on a choice not present in its command.

**2026-09-28: a supplied input's parameters are laid over the recorded
ones.** Section 3.1 calls them a requested upgrade and is silent on a recorded
parameter the input omits. It keeps its value and provenance `recorded`, so an
input can change a governed parameter but never silently drops one; a default
is restored by stating it.

**2026-09-28: the normalized effective parameters are the named ones.** The
manifest records the validated parameters the input or the declaration named,
in canonical order, never a default: a recorded default would freeze today's
value across a later revision and turn `default` into `recorded` with nobody
choosing it.

**2026-09-28: the input's bytes are bound, its path is not.** The identity
commits to the schema and byte digest; the report names the path read. Since
the identity is one-way, a stale-plan refusal names the input's current digest
for the operator to compare with the digest the reviewed plan reported.

**2026-09-28: tracked means in the git index.** A Rust prerequisite is met by
a regular file, not a link, that `git ls-files --error-unmatch` names; a staged
file counts, since the next commit carries it. A declared checker is judged by
its canonical path, so a link leaving the target is outside it.

**2026-09-28: the pin check belongs to every initialization that creates the
file.** Section 3.3 binds a new file's pin to the producer's identity whether
or not a profile is selected, so the refusal is in the plan step's preflight.

**2026-09-28: a preview lists only the writes an apply performs.** The
governance plan offers a managed file whose bytes and entry already match, and
apply has always left it alone. The preview now asks the same question, so a
converged plan proposes zero target writes (section 3.5). This corrects the
report under spec 002's one-plan rule and changes no write.

## Verification

Each line is one command. The binary suite is the acceptance anchor: section
2's fresh repository with no toolchain files, planned then applied (3.1 to
3.5), then completed and planned again (3.5), and one test per row of section
3.6. The library tests hold the document's closed shape (3.1), provenance
(3.2), the producer's pin (3.3) and each prerequisite's properties (3.4).

```verify:cli
cargo test -p statecraft-home --lib setup_input::
cargo test -p statecraft-home --lib prerequisite::
cargo test -p statecraft-home --lib producer::
cargo test -p statecraft-home --test bootstrap_inputs
cargo test -p statecraft-cli --test bootstrap_inputs
```
