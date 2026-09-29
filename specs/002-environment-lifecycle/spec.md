---
id: "002-environment-lifecycle"
title: "Project registration and the Statecraft-managed environment: one global home, one project area, one initialization flow, and where authority comes from"
status: approved
implementation: complete
created: "2026-09-16"
summary: >
  Defines repository registration, initialization, the global and per-project
  environment, ownership classes, manifests, configuration provenance,
  upgrade, removal, transfer, and setup-profile rendering. Harness delivery,
  governance-producer adoption, and managed-session evidence are separated
  into specs 008, 009, and 015.
establishes:
  - { kind: directory, path: "crates/statecraft-environment/" }
  - { kind: directory, path: "crates/statecraft-home/" }
  # The acceptance, as a program rather than as prose. Claimed as a whole file:
  # `scripts/**/*` is in [index] extra_hashed_inputs, so it is witnessed and
  # raises no L-008, and a change to it restamping every shard is correct for a
  # file that decides what this corpus may claim about enforcement.
  - "scripts/acceptance/managed-session.sh"
amends:
  # Section 3.6's "no configuration file may change a rule" gets its precise
  # reading in section 3.16: a layer supplies a value, never a rule, and every
  # value carries the layer that supplied it.
  - "006-command-surface"
extends:
  # Every verb sections 3.11 to 3.21 name is a binding inside the crate 006 owns
  # as one directory unit, which is how 006 section 3.1 admits a verb to the tree.
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
  # The authored-content check skips the compiled artifacts by path, and section
  # 3.19 moves them. The script is 001's unit, so the edge is declared rather
  # than discovered by the coupling gate.
  - { spec: "001-boundaries-and-authority", unit: "scripts/check-authored-content.sh", nature: corrective }
  # One test in 004's crate was repaired while this spec's round was measuring
  # against it: `settings_transport.rs`'s deadline attempt raced its own
  # subject. The repair is to the test's fixture and its assertions only, and
  # 004's required behavior is untouched, which is why the nature is
  # corrective. The edge is declared rather than left for the coupling gate to
  # discover, and the diagnosis is a dated entry in section 5.
  # Section 3.37 rule 2 adds one entry point there, `supervise_with_in`, which
  # writes the settings file into the attempt's exchange directory; additive,
  # and the edge keeps the nature it was first declared with.
  - { spec: "004-execution-adapter", unit: { kind: directory, path: "crates/statecraft-adapter-claude-code/" }, nature: corrective }
  # Section 3.30 rule 12's launch is supervised by the process-group supervisor
  # 004 already owns, so the raw capture it needs is added there rather than
  # written a second time here. Additive: no existing behavior of the
  # supervisor changes. The same section's rule 7 types three stream fields in
  # the claude-code crate above, which is also additive; that edge keeps the
  # nature of the repair it was first declared for.
  - { spec: "004-execution-adapter", unit: { kind: directory, path: "crates/statecraft-adapter/" }, nature: additive }
depends_on:
  - "000-bootstrap"
  - "001-boundaries-and-authority"
# Not `depends_on`: 006, which depends on this spec. The realignment half needs
# 006's command tree and its exit-code vocabulary, and while that half was spec
# `010` it could say so without a cycle. Merged, the `extends` and `amends`
# edges above carry the relationship, and they name the unit and the nature,
# which `depends_on` never did. See the 2026-09-21 entry in section 5.
---

# 002: Project registration and the Statecraft-managed environment

## 1. Purpose

Two jobs that are usually conflated, separated here because they consent to
different things:

- **Registration** records that a repository is a target, with a read-only
  qualification verdict. It changes nothing inside the repository.
- **Environment installation** writes files into the repository so that agent
  sessions and this product's own checks have a working loop.

This product owns installation and ongoing maintenance of the working
environment. That is more than relocating templates: what makes it a product is
knowing which bytes it owns, what to do when one has changed underneath it, what
a pin means, and what removal is obliged to leave.

Sections 3.11 to 3.21 carry the realignment that made it the *sole* owner.
spec-spine withdrew its kit and its public initializer, so the transition
contract §3.7 once held has no second installer to coexist with, and this
product became the only user-facing initializer and environment manager. That
half fixes where the environment lives, globally and per project, where its
authority comes from, and what a producer at the boundary is allowed to hand
back.

## 2. Territory

`crates/statecraft-environment/` and `crates/statecraft-home/`.

Two crates and one spec. The split is the one the realignment draws: the
environment crate owns a *target repository's* managed bytes, and the home crate
owns the product's own global environment and the flow that initializes a
project. A change to what this product writes into someone else's repository and
a change to what it keeps in its own home are different blast radii, and keeping
them separate compilation units is what makes that visible.

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

### 3.7 The transition contract with `spec-spine init --with-kit` (withdrawn)

**Withdrawn.** spec-spine removed the kit and the public initializer, so there is
no second installer to coexist with and the machinery would be permanent
scaffolding for an event that finished. Section 3.21 states the withdrawal, and
names the three parts of it that are **retained** because each is general rather
than kit-specific and each still protects a user-owned file: a `foreign` finding
names an owner and not only a path, `shadowed` stays because a digest match is
not evidence that a file is in force, and ownership transfer stays per path,
explicit, operator-initiated, reversible and recorded.

The number is kept rather than reused. Citations to §3.7 elsewhere in the corpus
resolve here and find the withdrawal, which is the answer they need.

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

### 3.10 Observable negative cases

Every row is required behavior. The first set belongs to registration and
the target repository's environment; the rest to the global home, the
project area and initialization.

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
| Fresh solo initialization in a temporary home with no platform credential and no platform reachability | `init apply` completes; every step reports done; nothing asked for an account, a login or a token. |
| `init apply` on an unarmed, unregistered project | The project is registered and qualified; it is **not** armed and nothing is executed. |
| A root `AGENTS.md` that exists and carries the user's own content | Its content is preserved; the import is the first line, appears exactly once, and a second run changes nothing. |
| A supported adapter whose harness load rule is evaluable | Delivery of `.statecraft/AGENTS.md` is reported `reached`, naming the chain. |
| A harness with no documented load rule this product can evaluate | Reported `unverified`, never `delivered`, and nothing is injected on the strength of a file existing. |
| A supported managed session | No project-local generic harness copy is required, and initialization writes none. |
| Existing user-global agent settings present before `home apply` | Byte-identical afterwards. |
| An unrelated repository on the same machine | Unaffected by global integration, and the delivered behavior is inert there. |
| A project override and an explicit run choice | Both recorded in the resolved configuration, each with the layer that supplied it. |
| A global upgrade after a run resolved | The recorded resolution of that run is unchanged. |
| The real spec-spine library and commands against `derived_dir = ".statecraft/derived"` | The scaffold is produced, the corpus compiles and indexes, and `check` passes at the new path. |
| A `.gitignore` that would ignore all of `.statecraft/` | Refused, naming the line: governed project metadata stays governed and only runtime state is excluded. |
| A solo project with a local approval for a subject | Eligible, with the authority recorded as the local operator. |
| A project enrolled into a team | Only that project's declaration changes; a sibling project's authority is untouched. |
| An enrolled project whose platform is unreachable, for a subject that requires shared approval | `pending`, never granted; a local approval does not satisfy it; the project is still enrolled. |
| A candidate diff that weakens a constraint in the declaration | Resolution uses the base revision's answer and reports an authority change. |
| An initialization interrupted after some steps, then repeated | No authored file is destroyed, the completed steps are not redone destructively, and the outcome is `partial` until every step reports done. |
| A producer result carrying a path outside the contract set | Not written; named; the producer's conformance for that run is `non-conforming`. |
| Both `.derived/` and `.statecraft/derived/` present | The relocation refuses and names both. No file is moved. |
| A declared value that is an absolute path or begins with `~` | Refused at write time, with the key named. |

### 3.11 One Statecraft-managed global environment

One global environment, at the product home: `~/.statecraft/`, overridden by
`STATECRAFT_HOME`. The override already exists (`006` section 5) and is
unchanged; what changes is that the home now has a declared shape rather than
two files that happened to be written there.

| Path under the home | Holds | Owner |
|---|---|---|
| `home.json` | The home schema version, the operator's name, and personal defaults. | this spec |
| `tools.json` | Installed tools and harness revisions: for each, what was requested and what actually resolved. | this spec |
| `harness/<revision>/` | The one canonical harness source: skills, agent definitions, rules, hooks and adapter templates. | this spec |
| `delivery.json` | What each native-discovery adapter was asked to deliver, where, and what the delivery check observed. | this spec |
| `approvals/` | Local approval records, one file per project. | this spec |
| `projects.json` | The register of targets. | `002`, unchanged |
| `qualifications.json` | Provider qualification records. | `004` (which absorbed `008`), unchanged |

Three rules the shape has to keep:

1. **Extend, never replace.** This spec reads and writes only the files it owns.
   The register and the qualification records are read through their owners and
   are never rewritten here, so adopting this shape loses no existing record.
2. **An absent home is an empty home, not an error.** Every read-only verb
   answers with no home present, which is what makes a fresh machine usable
   before anything is installed.
3. **No credential lives here.** The home holds no provider credential and no
   platform token. Provider credentials stay where the provider keeps them
   (`004` section 3.14), and this product does not relocate them.

### 3.12 One per-project area

Inside a target repository, exactly four paths, and no others:

| Path | Committed | Holds |
|---|---|---|
| `.statecraft/environment.json` | yes | The environment declaration: pins, managed-file ownership, tracked modifications, and the project block of section 3.16. |
| `.statecraft/AGENTS.md` | yes | The Statecraft-managed project instructions. |
| `.statecraft/derived/` | yes | spec-spine's compiled artifacts. |
| `.statecraft/state/` | no | Runtime state. Ignored. |

The existing locations are preserved and are not moved: `spec-spine.toml` at the
repository root, `specs/` at the repository root, `standards/spec/` where it is,
and every ordinary project document where it is.

**`.statecraft/` as a whole is never ignored.** Ignoring it would take the
committed declaration, the managed instructions and the compiled artifacts out
of version control in one line, and the three of them are exactly what makes a
project governed. A `.gitignore` that ignores the whole directory is a refusal
with that reason, not a warning.

The committed declaration carries **no secret, no machine-specific absolute path
and no personal preference**. A declared value that is an absolute path, or that
begins with `~`, is refused at write time and reported by `doctor`: a remote
worker resolving the same declaration must be able to satisfy it independently,
and a path under one operator's home cannot be satisfied by anyone else.

### 3.13 The root instruction bridge

The root `AGENTS.md` stays the user's file. This product takes one **managed
modification** of it and never ownership of it:

> the first line is exactly `@.statecraft/AGENTS.md`

Observable rules:

1. If no root `AGENTS.md` exists, it is created holding the import line.
2. If one exists, the import is inserted as the first line and **every other
   line is preserved**, except that leading blank lines left behind by the
   insertion are not accumulated.
3. Insertion is **idempotent**: applying it to an already-bridged file changes
   nothing, and a file that carries the import somewhere other than the first
   line ends with exactly one copy of it, first.
4. The modification is recorded in `.statecraft/environment.json` as a
   modification (path, kind, the exact inserted line, the digest before and the
   digest after), not as a managed entry. Removal removes the inserted line and
   nothing else, and only while the file still begins with it.

**Old generated content is classified, never swept.** A root file whose bytes
are byte-identical to a known generated `AGENTS.md` is reported as
`generated-unmodified`, naming the revision it matched. Anything else is
`customized`. Neither is deleted, neither is rewritten, and project policy is
never moved out of a customized file by this product: an ambiguous body is
preserved and the conflict is reported.

**The import line is a project convention, not a proven universal mechanism.**
Claude Code documents expansion of an `@path` import; Codex's documentation does
not establish an equivalent. So the line alone is never evidence of delivery,
and section 3.14 is what an adapter has to satisfy instead.

### 3.16 Configuration authority

There is **one** policy and approval model, with explicit provenance. There is no
single last-writer-wins merge.

| Layer | May supply | May not |
|---|---|---|
| Personal defaults (`home.json`) | A default for a key the project does not require and team policy does not constrain. | Anything the project requires; anything a team constraint forbids. |
| Project configuration (committed declaration) | Project behavior, and requirements that bind every layer below and beside it. | Weakening a team constraint. |
| Team policy (platform, when enrolled) | Shared constraints. | Nothing here is overridden locally. |
| Explicit run choice (an argument) | A value inside every applicable constraint. | A value outside one. |

Resolution answers per key with the value, the layer that supplied it, and every
layer that constrained it. Three rules make it honest:

- A run choice outside a constraint is **refused**, naming the constraint and its
  origin. It is never silently clamped to the nearest allowed value.
- A required policy or a required piece of evidence that is missing resolves to
  **unknown**, and unknown is not success.
- **A candidate cannot choose or weaken the policy that judges it.** The trusted
  configuration for a run is read at the trusted base revision, which is `001`
  section 3.5.1. A candidate diff touching the declaration is an authority
  change, which is `001` section 3.5.2, and the resolution reports it as one and
  keeps using the base's answer.

The project block carries one member another spec reads: `commands`, the
command allowance of spec `004` section 3.17, a list of bare program names.
It supplies a value (which programs a posture allows), not a rule; the
comparison is `004`'s.

**This is the precise reading of `006` section 3.6**, which says the binary
reads no configuration file that could change a rule an owning spec fixed. It
still does not. A layer supplies a **value**, never a rule: the rules stay in the
specs that own them, every supplied value carries the layer that supplied it into
the record, and a value no layer is allowed to supply is refused rather than
applied. The declaration and `home.json` are read as the target repository and
the product home, which section 3.16 of `006` already names as two of the three
things the binary reads.

Six postures stay distinct and are never inferred from one another: **approval**,
**eligibility**, **registration**, **qualification**, **arming** and **execution
posture**. Registering makes a target visible; qualification is a read-only
verdict; arming consents to being driven; an approval is about a subject, not
about a repository; eligibility is the join; and the execution posture is `004`'s.

**A resolved run is frozen.** A run resolves its harness revision and its tools
once, and records both the requested identity and the identity that actually
resolved. A later global upgrade changes the home and changes nothing about a run
already resolved.

### 3.17 Initialization, and the verbs

One user-facing initialization flow, spelled the way this binary already spells a
preview and a performance (`env plan` and `env apply`):

| Command | What it does |
|---|---|
| `home show` | The resolved global environment: paths, personal defaults, tools, harness revisions, and each adapter's delivery verdict. Reads only. |
| `home plan` | What `home apply` would write, inside the home and outside it. Writes nothing. |
| `home apply` | Creates or repairs the home and performs adapter delivery. The one operation that writes into a native agent location. |
| `init plan <path>` | Every project change initialization would make. Writes nothing. |
| `init apply <path>` | Performs it. |
| `migrate plan <path>` | The one-time relocation of section 3.19. Writes nothing. |
| `migrate apply <path>` | Performs it. |
| `project enroll <path> <team>` | Records team enrollment in the project declaration. |
| `project unenroll <path>` | Removes it. |
| `config show <path>` | The resolved configuration for a run in that project, per key, with provenance. |
| `approval grant <path> <subject> <operator> <reason...>` | Records a local approval for one subject. |
| `approval show <path> <subject>` | The eligibility of one subject, and the authority behind it. |

`init apply` performs these steps, in this order, and reports each separately:

1. **home**: resolve and check the global environment. No platform login, no
   network, no account.
2. **plan**: compute every project change.
3. **reconcile**: preserve every user file; write only managed content.
4. **governance**: obtain the starter files through the real spec-spine library
   (section 3.15).
5. **project**: write the declaration and the instruction bridge.
6. **corpus**: compile, index and check the intended corpus through spec-spine.
7. **register**: register the project and evaluate its qualification.

Then it stops. **Arming and execution are separate explicit acts** and are not
steps of initialization.

**The bootstrap cycle is avoided by that order, not by an exception.**
Qualification is step 7 because the corpus it judges is created by step 6.
Nothing in steps 1 to 6 requires the project to be qualified, and `init` never
requires a prior qualified state.

**An interrupted or repeated initialization is safe.** Each step is idempotent;
the last completed step is recorded under `.statecraft/state/`; a re-run
re-reconciles and rewrites no authored file. The outcome is `complete` only when
every step reported done, `partial` when any step was withheld or skipped, and
`refused` when a precondition stopped it before any write. Partial work is never
reported as complete.

`init plan` computes the same plan `init apply` performs, from the same code, so
the preview is a preview.

### 3.18 Solo and team

**The complete local capability set requires no platform.** Local
initialization, project management, execution, approvals, eligibility, policy,
evidence, recovery, dashboard operation and governance verification all have
local implementations, and none of them requires a Statecraft platform account,
a platform login, a hosted connection, a paid plan or a platform-issued
authorization token. This is constitution XIII, made operational. Model-provider
credentials are a separate matter and are required only by the provider actually
selected.

**Enrollment is explicit and project-scoped.** It is recorded in that project's
committed declaration and nowhere else, so enrolling one project changes no other
project's authority. The default is solo.

For an enrolled project the platform is the coordination authority for shared
approvals, eligibility and policy. When it cannot be reached:

- the project does **not** become solo-owned;
- work that does not require remote authority proceeds under the project's
  recorded policy;
- a required shared approval is `pending` or `refused`, never granted, and
  **a local approval never satisfies one**.

The hosted platform is **not implemented here**. What is implemented is the local
boundary, the seam a platform would satisfy, and the honest unavailable states.
The shipped implementation of that seam reaches nothing and reports
`unavailable`; no fixture is presented as a live integration, and no document
here claims one.

### 3.19 The one-time relocation of the compiled artifacts

For this repository and for bounded fixtures, `.derived/` moves to
`.statecraft/derived/`, and the configuration, ignore rules, check surface, CI
and documents that name the old path move with it in the same change.

- If a repository holds **both** a non-empty `.derived/` and a non-empty
  `.statecraft/derived/`, the move is **refused**, naming both. It is never
  resolved by choosing one.
- The operation touches exactly one repository root. It sweeps no sibling
  repository and touches no real home directory.
- It is exposed as its own operation and is not a step of `init`.

### 3.20 The operation boundary a dashboard uses

Every operation in section 3.17 is a typed request and a typed outcome in this
spec's crate. The command surface is one caller of it. A local dashboard is a
second caller of the **same** operations: it implements no second settings
engine, no second scheduler and no second policy model.

A hosted platform and a local dashboard share operation semantics. The local
runner owns local processes; the platform coordinates team authority. **One run
has one execution owner**, and no background loop is added by this spec.

No user interface is implemented here. `F-04` stands, and the grade this spec may
claim about a dashboard is the typed boundary and nothing above it.

### 3.21 What replaces the kit transition contract

Section 3.7 is withdrawn as a transition contract: with the kit and the
public initializer removed, there is no second installer to coexist with, and
keeping the machinery would be permanent scaffolding for an event that finished.

Three parts of it are **retained**, because each is general rather than
kit-specific, and each still protects a user-owned file:

1. A `foreign` finding names an **owner**, not only a path.
2. `shadowed` stays: a digest match is not evidence that a file is in force.
3. Ownership transfer stays per path, explicit, operator-initiated, reversible,
   and recorded with the digest observed at the moment of transfer and with the
   producer revision it was evaluated against.

There is no deprecation program, no dual installer, no compatibility shim and no
generalized legacy-migration framework. The replaced model has no backward
compatibility requirement.

### 3.35 Per-path ownership transfer, as an operator's act

A narrowly scoped authority amendment, settled by the owner on 2026-09-23 and
recorded before the implementation it authorizes. Section 3.21 retains, from
the withdrawn section 3.7, that ownership transfer is "per path, explicit,
operator-initiated, reversible, and recorded with the digest observed at the
moment of transfer and with the producer revision it was evaluated against".
This section is the operation.

**The gap, exactly.** The manifest can carry a `transfer` on an entry, and
`env plan` and `doctor` read one, but nothing creates one and nothing reverses
one. A path this product would manage and finds occupied is withheld as
`foreign` on every apply, and the only way to change that is to edit the
committed manifest by hand.

**Rule 1: three classes, four moves, one path.** The classes are section 3.2's.
A transfer names exactly one repository-relative path, its current class and
the class it is to take, and is one of:

| From | To | What changes |
|---|---|---|
| `user` | `adopted` | An `adopted` entry is recorded with the digest observed now. Nothing is written to the file. |
| `user` | `managed` | A `managed` entry is recorded with the digest observed now, and a `transfer` naming the prior claimant. Only for a path this product would itself write: one an installed adapter declares, with that adapter as its source. A later `env apply` or `env upgrade` may then rewrite it; the bytes it held before are not kept by this product. |
| `adopted` | `user` | The entry is removed. |
| `managed` | `user` | The entry is removed. The file stays, byte for byte; `env remove` no longer deletes it. |

`adopted` to `managed` and back is two transfers through `user`, each recorded.
A path this product has no source for cannot become `managed`: management
means upgrades rewrite it, and there would be nothing to rewrite it with.

**Rule 2: never inferred.** A transfer happens only when an operator asks for
it by path. Nothing moves a path between classes because its bytes resemble
something this product generates, because a producer returns it, or because a
directory holds it. Initialization and upgrade never transfer. In particular a
user instruction file is `user` class under section 3.8 and **cannot** be
transferred to `adopted` or `managed` by this operation. The list is closed, by
file name at any depth: `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.cursorrules`,
`.windsurfrules`, and `.github/copilot-instructions.md`. A pointer file this
product wrote under section 3.8 is already `managed` and may be released to
`user`. The root instruction bridge of section 3.13 is a modification, not an
entry, and is not transferable either.

**Rule 3: what may be named.** The path is relative, uses forward slashes, has
no empty, `.` or `..` component, and names a regular file that exists. No
component may be a symbolic link. A directory is refused, so nothing is adopted
wholesale. Anything under `.statecraft/` and anything under a `.git`
component are refused. Every refusal names the
rule and writes nothing.

**Rule 4: plan, then apply.** `transfer plan` reports, and writes nothing: the
path, its current class as the manifest and the file say, its digest and length
now, the resulting entry, the producer revision, and a **plan identity**, the
SHA-256 of the path, both classes, the file's digest and the manifest's digest.
`transfer apply` takes the same path and classes, that plan identity, an
operator name and a reason, recomputes the plan, and refuses unless the
identity is equal: a file edited, a manifest changed or a class that is no
longer the one named in between is a stale plan, refused, with what changed
named. The operator name is recorded as supplied and not authenticated.

**Rule 5: the record.** An applied transfer changes the manifest entry as rule
1 says and appends one record to the transfer journal, a `transfers` list
inside `.statecraft/environment.json` itself, so that section 3.12's four
paths stay four: its
identity, the path, both classes, the digest and length observed, the producer
revision it was evaluated against (the `spec-spine-core` version this build
links), the operator as supplied, the reason, the time, and the manifest's
digest before. Nothing else in the manifest and no byte of any file changes.
The journal is append-only. A manifest whose journal disagrees with its
entries is reported by `transfer plan`, and `transfer apply` and `transfer
revert` refuse on it.

**Rule 6: reversal.** `transfer revert` names a recorded transfer, an operator
and a reason, and applies the inverse move when, and only when, that transfer
is the latest for its path, the path's class is still the one it produced, and
the file's digest is what this product's record says it should be: the digest
the manifest entry records now, where the transfer produced an entry (so a
rewrite this product made and recorded, such as an `env apply` after a move
to `managed`, is not an intervening change), and the digest the transfer
observed, where it removed one. An unrecorded edit, a later transfer or a
missing file refuses the reversal and names which. Reversal changes ownership,
never content: reversing a move to `managed` does not bring back bytes an
apply replaced. A reversal is a new journal record that names the one it
reverses; the reversed record is kept.

**Rule 7: repeating a satisfied request.** Checked before the plan identity:
applying a transfer whose latest journal record already moved the same path
between the same classes, with the path still in that class and the file's
digest what rule 6 says it should be, reports `already-satisfied` and writes
nothing, whatever plan identity was given. Any other request whose named
current class is not the path's class is refused.

**Rule 8: the producer boundary is unchanged.** Adopting a conforming producer
(section 3.15) transfers nothing. A user's pre-existing instruction files stay
`user`, and section 3.13's recognition of a generated, unmodified root
`AGENTS.md` is not a transfer.

**The command surface.** Spec `006` section 3.11.7 adds `transfer plan`,
`transfer apply` and `transfer revert`.

**Compatibility.** A manifest written before this section has no journal and
reads as having no transfers. An entry carrying a `transfer` with no journal
record is read as before, reported by `transfer plan` as recorded without a
journal, and never rewritten. A build that predates this section and rewrites
the manifest does not carry the `transfers` list; none is distributed
(`F-02`), and a manifest that lost its journal reads as above.

**Acceptance.** In an isolated home and fixture repositories, through the
binary: `user` to `adopted` and back; `user` to `managed` for an adapter path
that `env apply` withheld as `foreign`, after which `env apply` writes it, and
the reversal, after which it is withheld again; `managed` to `user`, after
which `env remove` leaves the file. Negative: a stale plan after the file, the
manifest or the class changed; a named class that is not the path's; `user` to
`managed` for a path no adapter declares; a root `AGENTS.md`; a symbolic link,
a `..` path, a directory, `.statecraft/environment.json` and a `.git` path;
a reversal after an intervening edit or a later transfer; and a repeated
request reported `already-satisfied` with nothing written. Every refusal
leaves every byte of the repository as it was.

## 4. Out of scope

Installing the product itself; provider authentication; hosted registration;
multi-machine environment sync; and any write to a target outside the manifested
set. Publication, release and distribution are deferred by `F-02`; how this
product is itself packaged is recommendation `D-01`.

Implementing a hosted platform, a platform protocol or a platform client; any
user interface, which `F-04` defers; publication, release and distribution,
which `F-02` defers; multi-machine environment sync; a package registry or a
plugin marketplace; migrating sibling repositories or any real home directory;
adding a provider adapter, which is `004`'s boundary (it absorbed `008`) and `F-07`'s deferral; and
any new work, run or acceptance ledger, because `003` and `005` already own
those semantics and a second one would be the failure this product exists to
avoid.

## 5. Resolved decisions

Legacy dated citations to this section refer to the preserved historical
journal at `docs/decisions/archive/002-environment-lifecycle-implementation-journal.md`.
They are provenance, not current requirements.

**2026-09-16: lifecycle state is declared, not remembered.** Ownership class,
drift, and replacement are determined from the committed environment manifest
and inspected bytes. An absent declaration is never treated as prior management.

**2026-09-20: filesystem roots are explicit inputs.** Planning and application
receive the native root as a parameter and do not infer authority from process
globals.

**2026-09-23: ownership transfer is an operator act.** Per-path transfer is
planned, consented, journaled, and applied under the manifest lock. Similarity
or an existing marker is not authority.

**2026-09-24: initialization is plan-first and outcome-complete.** Preconditions
are resolved before the first write, withheld paths produce a partial outcome,
and provenance records the exact managed or adopted source used for each write.

**2026-09-26: profile revision 9 is this repository's rendered authority set.**
The repository keeps `governance.fail_on_unresolved` at its default `true`, so
the rendered governance gate continues to refuse unresolved claims.

**2026-09-26: initialization reconciles the transfer journal before adopting
a governance path, and repairs only its own inferred legacy adoption.**
Sections 3.2, 3.15, 3.17 and 3.35 already require initialization to preserve
user ownership, never infer a transfer, and keep entries consistent with the
append-only journal. The implementation choices they left open are these.

- A present, unrecorded governance path whose latest transfer ends in `user`
  stays user. Reconciliation records no entry, changes no file or journal
  byte, and reports `user-transfer-preserved` under `kept`, separately from
  ordinary `adopted` paths.
- The exact state this implementation previously inferred is recognized by
  all of: latest transfer to `user`; current entry `adopted`; source kind
  `template`; source identity `statecraft-governance`; and no entry-level
  transfer. Reconciliation removes only that in-memory entry and reports the
  path as `inferred-adoption-recovered` under `kept`. Plan mode writes nothing.
  Apply performs the same
  change while holding the manifest lock already taken by initialization,
  then persists it through the ordinary manifest write. The file and journal
  are left byte-identical.
- A released governance path that is absent is still eligible for section
  3.15's fresh write. The new entry is `managed`, uses template identity
  `statecraft-governance`, and carries no entry-level transfer. Rule 5's
  consistency check recognizes exactly that product rewrite, beside the
  existing adapter rewrite case. A present released file never qualifies.
- Every other journal disagreement refuses reconciliation before project or
  home writes. `transfer apply` and `transfer revert` keep their existing
  fail-closed ordering. Reversal to `managed` recognizes the closed governance
  contract set as a real template source, so a valid governance release can
  still be reversed without inventing an adapter.

Tests compose initialization, transfer, plan, apply, reversal, exact legacy
repair, deleted-path rewrite, unrelated disagreement refusal, current-revision
profile rendering, repeated planning and repeated apply. Existing interruption
and resume cases exercise the same shared preflight and manifest-last write
order.

## Verification

`--fail-on-untraced` joined the corpus gate with this spec's first
implementation, which is the condition AGENTS.md recorded for it; it defends
every claimed file rather than reporting a number (13 then, 149 on
2026-09-23).

Each line is one command. Every row of section 3.10 is one integration test
named after the row it covers, so a row that stops being covered shows up as a
deleted test rather than as a quietly weakened assertion.

```verify:cli
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
spec-spine index coverage --fail-on-untraced
test -f crates/statecraft-environment/src/manifest.rs
test -f crates/statecraft-environment/tests/negative_cases.rs
spec-spine index check --fail-on-unresolved
test -f crates/statecraft-home/src/lib.rs
cargo test -p statecraft-home --test negative_cases
cargo test -p statecraft-home --lib setup
cargo test -p statecraft-home --test setup_workflows
cargo test -p statecraft-home --test setup_upgrade
cargo test -p statecraft-cli --test setup_profile
test -f crates/statecraft-environment/src/transfer.rs
cargo test -p statecraft-environment --test transfer
cargo test -p statecraft-cli --test ownership_transfer
cargo test -p statecraft-environment --test replace_per_path
cargo test -p statecraft-cli --test env_replace
cargo test -p statecraft-environment --test bridge_removal
cargo test -p statecraft-cli --test env_remove_bridge
```
