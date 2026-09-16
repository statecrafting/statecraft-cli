---
id: "002-environment-lifecycle"
title: "Project registration and the managed working environment: install, upgrade, drift, removal"
status: approved
implementation: pending
created: "2026-09-16"
summary: >
  How a repository becomes a target this product may work in, and how the
  product installs and maintains the working environment inside it. Fixes the
  three ownership classes (managed, adopted, user), the committed manifest that
  records every managed byte with its source and digest, the pins the
  environment records, what an upgrade does when a managed file has drifted,
  what removal leaves behind, how an agent-harness adapter declares what it
  owns, and the transition contract that keeps this product and
  `spec-spine init --with-kit` from both claiming the same harness files.
  Preserving a pre-existing user instruction file is a refusal, not a merge.
establishes:
  - { kind: directory, path: "crates/statecraft-environment/" }
depends_on:
  - "000-bootstrap"
  - "001-boundaries-and-authority"
---

# 002: Project registration and the managed working environment

## 1. Purpose

Two jobs that are usually conflated, separated here because they consent to
different things:

- **Registration** records that a repository is a target, with a read-only
  qualification verdict. It changes nothing inside the repository.
- **Environment installation** writes files into the repository so that agent
  sessions and this product's own checks have a working loop.

The long-term intent is that this product owns installation and ongoing
maintenance of the working environment. That is more than relocating templates:
what makes it a product is knowing which bytes it owns, what to do when one has
changed underneath it, what a pin means, and what removal is obliged to leave.
`spec-spine init --with-kit` already writes a harness; §3.7 is how the two stop
colliding without this repository editing or deprecating spec-spine's kit.

## 2. Territory

`crates/statecraft-environment/` (forward claim; unresolved until implemented).
The committed manifest at `.statecraft/environment.json` **in a target
repository** is this spec's format, not this repository's territory.

## 3. Behavior

### 3.1 Registration and qualification

`project register <path>` records an absolute path under the product's own home
and evaluates a **read-only** qualification. It writes nothing inside the target.

Qualification reports a verdict and its reasons, never a bare boolean:

| Verdict | Meaning |
|---|---|
| `qualified` | A git work tree, a resolvable base revision, and a spec-spine corpus that compiles. |
| `ungoverned` | A git work tree with no corpus. Visible, with reasons; never scheduled. |
| `unqualified` | Not a git work tree, or a corpus that does not compile. Visible, with reasons; never scheduled. |

A repository is **armed** separately from being registered. Arming consents to
being driven. It does not consent to an execution posture: that is `004`.

### 3.2 The three ownership classes

Every path inside a target is exactly one of:

- **managed**: written by this product, listed in the manifest with its source
  identity and content digest. The product may rewrite it on upgrade and must
  remove it on removal.
- **adopted**: it existed before, and the product depends on it but does not own
  it. Recorded in the manifest as adopted, with the digest observed at adoption.
  Never rewritten.
- **user**: everything else. Never read for control purposes, never moved, never
  rewritten.

The classes are **disjoint and exhaustive by construction**: a path the product
writes that is not in the manifest is a defect, and `doctor` reports it as
`unmanaged-write`.

### 3.3 The manifest

`.statecraft/environment.json`, committed. Runtime state lives under
`.statecraft/state/` and is gitignored; the manifest is not state, it is the
record of what this product owns in this repository.

Each entry carries: path; class (`managed` or `adopted`); the adapter or
template that is its source; the source identity; the SHA-256 digest and byte
length of the content written or observed; and the timestamp of the write.

The manifest header carries the **pins**: this product's version, the spec-spine
version the environment was installed against, and the adapter set. Pins are
recorded, never silently satisfied: a mismatch is a `doctor` finding.

### 3.4 Install, plan, upgrade

`env plan` prints what `env apply` would do and writes nothing. `env apply`
performs it. `env upgrade` re-plans against a newer product or adapter version.

Upgrade writes only a managed file whose on-disk digest **matches** its manifest
entry. For any other managed file the write is withheld and the file is reported.
The overall result is one of:

- `applied`: every planned write succeeded.
- `partial`: some writes were withheld; every withheld path is named with its
  reason. This is a success of the upgrade's contract, not a silent one.
- `refused`: a precondition failed and nothing was written.

An upgrade never resolves a conflict by choosing. Replacing a drifted managed
file requires the operator to say so per path.

### 3.5 Drift, and what `doctor` reports

`doctor` is the diagnostic surface and is read-only. Per manifest entry it
reports exactly one state:

| State | Meaning |
|---|---|
| `present` | On disk, digest matches. |
| `drifted` | On disk, digest differs. Left alone. |
| `missing` | In the manifest, absent on disk. |
| `foreign` | On disk and claimed by another installer, named by package identity where one exists (§3.7). Refused for management. |
| `shadowed` | Digest matches the manifest, but this is not the copy a session would resolve. The shadowing claimant is named (§3.7). |

Beyond entries, `doctor` reports pin mismatches, the adapters configured versus
available, and absent provider prerequisites. It never repairs. A diagnostic
that fixes what it measures cannot be trusted to measure it.

### 3.6 Removal

`env remove` deletes every `managed` path whose digest still matches, and no
other byte. A drifted managed path is reported and left: the operator changed it,
so it is theirs now. Adopted and user paths are never touched, and nothing is
restored to a remembered earlier state, because the product never recorded one.

Removal with no manifest present **refuses**. The alternative is guessing which
files were the product's, and a wrong guess deletes a user's work.

### 3.7 The transition contract with `spec-spine init --with-kit`

spec-spine owns its kit. This repository does not edit, vendor or deprecate it,
and standalone spec-spine use stays fully viable.

1. This product **never writes a path that spec-spine's kit owns** unless the
   manifest records an explicit ownership transfer for that path.
2. On `env plan`, a kit file present but absent from the manifest is classed
   `foreign` and reported. The plan proceeds for every other path.
3. Ownership transfer is per path, explicit, operator-initiated, and recorded in
   the manifest with the digest observed at the moment of transfer.
4. Transfer is reversible: releasing a path returns it to `foreign` and leaves
   the bytes in place.
5. The product's own version pin records which spec-spine kit revision the
   transfer was evaluated against, so a kit that moves afterwards is a `doctor`
   finding rather than a silent divergence.

**`foreign` names an owner, not only a path.** spec-spine's design note 06
proposes replacing copied kit files with a versioned, namespaced package,
globally cached per version and pinned by a repository declaration (its section
3.2). Two consequences for this spec, both additive:

- A `foreign` finding carries the **identity of whatever claims the path**: a
  package name and revision where one exists, a path where it does not. A path
  alone stops being enough to describe the conflict once the claimant is a
  cached package rather than a file somebody copied in.
- The claimant may sit **outside the repository entirely**. Note 06 names the
  precedence trap: a personal skill resolves before a project skill of the same
  name, so the governed copy can be present, readable, and not the one that runs.
  `doctor` therefore reports a **shadowed** state for a managed path whose bytes
  match the manifest but which is not what a session would resolve, and the
  shadowing claimant is named. A digest match is not evidence that the file is in
  force.

Neither the package nor the declaration exists yet, so `shadowed` is reported only
where the product can actually observe the shadow, and `not-recorded` otherwise.

Proposed and not adopted: whether this product eventually installs the harness at
all, or only ever adapts around one spec-spine installs, is `D-04` in the
decision record.

### 3.8 User instructions are preserved

A pre-existing `AGENTS.md`, `CLAUDE.md`, `.cursorrules` or equivalent is **user**
class. It is never read for control purposes, never moved, never rewritten, and
never merged into.

Where an adapter needs a session-time entry point, the product writes its own
file at its own path and, at most, one pointer file, and only where no file
exists at that path. An existing file at a pointer path is reported as `foreign`
and the adapter reports itself degraded; it does not append.

This is the containment model Frame uses, and it is chosen **against** the
managed-section model (a tool owning a delimited region inside a file the user
also edits) that swamp uses. The choice, its reason and the prerequisite it
carries are `D-09` in the decision record: a pointer works only if the target
harness loads it, so "this harness loads a pointer at this path" is one of the
declared prerequisites in §3.9, and an adapter whose harness cannot **refuses to
claim its paths** rather than falling back to appending.

### 3.9 Adapters

An agent-harness adapter declares: the harness it targets; the exact set of paths
it would manage; the facts it cannot express in that harness; and the
prerequisites it needs present. Adapters are additive and independent, and two
adapters may not declare the same path.

An adapter whose harness is absent, or whose prerequisites are missing, **refuses
to claim its paths** and says which prerequisite is absent. It does not write
files for a harness that is not there.

### 3.10 Observable negative cases

| Case | Required behavior |
|---|---|
| `project register` on a path that is not a git work tree | Verdict `unqualified` with the reason; nothing written inside the path. |
| `project register` on a repository with no corpus | Verdict `ungoverned` with reasons; visible, never scheduled. |
| `env apply` where a planned managed path already exists and is not in the manifest | Path classed `foreign`, withheld, named; result `partial`. No overwrite. |
| `env upgrade` where a managed file has drifted | Write withheld, path named with digest expected and found; result `partial`. |
| `env remove` with no manifest | Refused, with the reason. No deletion. |
| `env remove` where a managed file has drifted | That file is reported and left; every matching file is removed. |
| Two adapters declaring the same path | Refused at plan time, naming both adapters and the path. |
| An adapter for an absent harness | Refuses to claim its paths, names the absent prerequisite, and does not write. |
| A pointer path already holding a user file | Classed `foreign`; the adapter reports degraded; the file is not appended to or replaced. |
| A path written by the product but absent from the manifest | `doctor` reports `unmanaged-write`. |
| A managed path whose digest matches but which a session would not resolve | `doctor` reports `shadowed` and names the claimant. A digest match alone is never reported as `present`. |
| `doctor` run against a drifted environment | Reports every state; exits non-zero on any `drifted`, `missing`, `foreign` or `unmanaged-write`; repairs nothing. |

## 4. Out of scope

Installing the product itself; provider authentication; hosted registration;
multi-machine environment sync; and any write to a target outside the manifested
set. Publication, release and distribution are deferred by `F-02`; how this
product is itself packaged is recommendation `D-01`.

## Verification

Declared by the change that implements this spec. None of §3 is implemented, so
this spec carries no `verify:cli` block: an acceptance block here today would
either fail or assert something other than the behavior above.
