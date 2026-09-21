---
id: "002-environment-lifecycle"
title: "Project registration and the Statecraft-managed environment: one global home, one project area, one initialization flow, and where authority comes from"
status: approved
implementation: in-progress
created: "2026-09-16"
summary: >
  How a repository becomes a target this product may work in, and how the
  product installs and maintains the working environment inside it. Fixes the
  three ownership classes (managed, adopted, user), the committed manifest that
  records every managed byte with its source and digest, the pins the
  environment records, what an upgrade does when a managed file has drifted,
  what removal leaves behind, and how an agent-harness adapter declares what it
  owns. Preserving a pre-existing user instruction file is a refusal, not a
  merge. Sections 3.11 to 3.21 carry the realignment that followed spec-spine
  withdrawing its kit and its public initializer, which left this product the
  sole user-facing initializer: one Statecraft-managed global environment under
  the product home and one per-project area under .statecraft/, the reusable
  harness as a single global source delivered by adapters rather than copied
  into every repository, the governance producer boundary as the spec-spine
  library's files-as-data result, the root instruction bridge as a tracked
  managed modification rather than ownership of a user's file, configuration
  authority as four layers with provenance rather than one last-writer-wins
  merge, and a solo and team boundary in which every capability has a local
  implementation and an unreachable platform is an honest unavailable state.
  Sections 3.22 and 3.23 record the counterparty's state at the moment it handed
  its harness over, the order the last of that harness moves in, and what any
  content delivered through the harness mechanism has to satisfy. Section 3.24
  is the one write into a harness's own settings file that section 3.14 rule 1
  admits: narrow, marked, reversible, refused by default, and able to add a
  refusal but never a permission.
establishes:
  - { kind: directory, path: "crates/statecraft-environment/" }
  - { kind: directory, path: "crates/statecraft-home/" }
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

### 3.9 Adapters

An agent-harness adapter declares: the harness it targets; the exact set of paths
it would manage; the facts it cannot express in that harness; and the
prerequisites it needs present. Adapters are additive and independent, and two
adapters may not declare the same path.

An adapter whose harness is absent, or whose prerequisites are missing, **refuses
to claim its paths** and says which prerequisite is absent. It does not write
files for a harness that is not there.

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
| `qualifications.json` | Provider qualification records. | `008`, unchanged |

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

### 3.14 One global harness, delivered by adapters, copied into no repository

Skills, agent definitions, rules, hooks and adapter templates are maintained
once, in `harness/<revision>/` under the home. A revision is **content
addressed**: its identity is a digest over its own files, so two homes holding
the same bytes hold the same revision and a changed byte is a different
revision.

A project receives **no copy**. A supported managed session needs no
project-local generic harness directory, and an initialization that wrote one
would be reintroducing the thing this realignment removes.

Native agent discovery may still need something in a native location such as
`~/.claude/` or `~/.agents/`. Those are **adapters**: small links, generated
files, plugins or launch configuration that point at the one canonical source.
Four rules bound them:

1. An adapter never repoints an agent home and never moves, reads or rewrites an
   authentication store. Unrelated native user configuration is preserved. A
   user's settings file is touched only under the **consented settings
   modification** of section 3.24, which is narrow, marked, reversible and
   refused by default; outside that, a delivery that would have to rewrite it is
   not performed.
2. Every delivered name is Statecraft-namespaced, so it cannot collide with a
   generic user skill of the same purpose.
3. Every delivered behavior is **gated to Statecraft projects**: it applies only
   inside a repository holding `.statecraft/environment.json`, and is inert
   everywhere else. An unrelated repository is unaffected by global integration.
4. Writing into a native location is an **explicit operator action**. It happens
   under `home apply`, never as a side effect of a read, a test or a project
   operation.

**Delivery is evaluated, not assumed.** A file existing, or matching a digest, is
not delivery. Each harness has a documented load rule, and this product evaluates
it against the actual tree:

| Verdict | Meaning |
|---|---|
| `reached` | Following the harness's documented load rule from its entry file arrives at `.statecraft/AGENTS.md`. The chain is named. |
| `not-reached` | The rule was evaluated and does not arrive there. The reason is named. |
| `unverified` | The harness has no documented rule this product can evaluate. Reported as unverified, never as delivered. |

Where a harness's rule is evaluable and does not reach the managed file, the
adapter may place **one pointer file, and only where no file exists at that
path**, which is section 3.8 unchanged. Where the rule already reaches the
managed file, **nothing is injected**: a second copy of an import that native
loading already performs is a duplicate, not a belt and braces.

### 3.15 The governance producer boundary

Governance starter files come from the spec-spine **library**, and from nowhere
else. The call is

`spec_spine_core::scaffold_init_json(config_json) -> Result<String, Error>`

returning the existing serialized `Scaffold` files-as-data shape with the
existing `ScaffoldFile` fields. It performs no write and no environment
discovery. The removed `spec-spine init` command is not invoked, no governance
template is vendored into this repository, and there is no fallback installer
built from old kit bytes.

The layout is passed **explicitly**, never defaulted:

```
specs_dir     = "specs"
standards_dir = "standards/spec"
derived_dir   = ".statecraft/derived"
state_dir     = ".statecraft/state"
```

The **contract set** is closed. This product places exactly these, and treats
every other returned path as out of contract:

1. `spec-spine.toml` at the repository root;
2. `<standards_dir>/constitution.md` and `<standards_dir>/contract.md`;
3. `<standards_dir>/templates/**`;
4. `<specs_dir>/000-bootstrap/spec.md`;
5. `.gitignore`, treated as a **fragment**.

An out-of-contract path is **not written**, is named in the report, and makes the
producer's conformance `non-conforming` for that run. That is a finding about the
producer, not a failure of the project: every in-contract path still reconciles.

Reconciliation is `002`'s ownership model and adds nothing to it. A contract path
that does not exist is written and recorded `managed`. A contract path that
already exists is **adopted**: recorded with the digest observed, depended on,
and never rewritten. Existing configuration and authored standards or specs are
not disposable templates.

The `.gitignore` fragment is **merged**: the lines it contributes that are not
already present are appended inside one marked block, and no unrelated entry is
replaced, reordered or removed. Merging is idempotent.

The producer identity (name and exact version) is recorded in the report and in
the declaration's pins. An exact, reproducible dependency is required; a
filesystem path to a sibling checkout is not one, and is never committed.

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

### 3.22 The counterparty's state, and the order the last of its harness moves in

Prepared 2026-09-20 by a spec-spine session and handed to this product as input
to section 3.14. It is **evidence from a counterparty, not an instruction to this
corpus**: every disposition in it belongs to this product's owner, and nothing
here binds beyond what sections 3.13 to 3.15 already required. It is recorded in
the spec it informs rather than filed beside the corpus, because a design note
that lives outside the spec is a second place for a requirement to be written
down and the first place people stop reading.

**What spec-spine no longer does.** It removed the initializer and the kit in
one change:

| Gone | Was |
|---|---|
| `spec-spine init` | the project initializer |
| `--with-kit` | the harness installer |
| `kit/`, `kit_embedded.rs` | the harness, vendored and embedded in the binary |
| `.agents/`, `.codex/` | generated projections of that harness |
| `website/` | the documentation site |

No verb there writes an `AGENTS.md`, a `CLAUDE.md`, a `.claude/` directory, a
skill, an agent brief, a hook, an MCP configuration, a CI workflow or a
`Makefile`. What survived is section 3.15's producer seen from the other side:
`scaffold_init_json` is a pure function of its argument, emits governance starter
content only, as data, and its own tests assert that no emitted path begins with
`.claude/`. So the ownership classes of section 3.2 have no second claimant left
to negotiate with, and an adapter that manages `.claude/**` contends with
nothing.

**Where that repository's own `.claude/` stands**, measured 2026-09-20. It keeps
one for itself, because removing it before a replacement exists would leave it
with no development instruction and no hook enforcement, and it is being
dismantled in the order that never leaves it unprotected:

| Class | State |
|---|---|
| `.claude/rules/` (4 files) | **removed**, folded into that repository's `AGENTS.md` as a `## Rules` section. Never a candidate for a global home: they are its own governance and would bind every project a user opens. |
| the push gate | **installed globally** at `~/.claude/hooks/push-gate.sh`. Repository-agnostic: `git` and `jq`, and no spec-spine. Copied rather than moved, because its tests cannot read `$HOME`. |
| `.claude/skills/` (10), `.claude/agents/` (4) | waiting on this product. |
| `.claude/settings.json` (a PR gate, two session hooks, permissions) | waiting on this product. |

**The ordering is fixed, and it is the reason this section exists.** This product
delivers a global harness with a Claude Code adapter; spec-spine confirms that a
session there still has its loop and its hooks; **then** its `.claude/` goes,
with its governing spec superseded in the same change. Doing it in the other
order is the failure the removal of the kit was written to avoid, and no schedule
pressure converts one order into the other.

**The instruction bridge waits on this side by design.** spec-spine will not add
`@.statecraft/AGENTS.md` to its root `AGENTS.md` until this product's initializer
actually writes that file, because an import of a file that does not exist is a
broken instruction rather than an early one. Section 3.13 is the rule and the
initialization flow of section 3.17 writes the file, so the condition is
satisfiable today; what remains is telling the counterparty, which is one line on
its side and nothing on this one.

### 3.23 What delivered harness content must satisfy

Section 3.14 fixes the **mechanism**: one content-addressed source under the
home, adapters that point at it, delivery evaluated rather than assumed. This
section fixes what any **content** delivered through that mechanism has to
satisfy, whether this product authors it or adopts it from the counterparty.
Adopting the inventory below is the owner's act; what adoption costs is stated
here so the decision is not made by discovering the cost afterwards.

**The inventory offered.** Ten skills (`prime`, `next`, `build`, `verify`,
`ship`, `shepherd`, `spec`, `commit`, `code-review`, `setup`) and four agents
(`architect`, `explorer`, `implementer`, `reviewer`). They are already
**repository-invariant**: every project-specific fact lives in that project's
`AGENTS.md`, which each skill ends by pointing at. That property was built for a
distribution that was then cancelled, and it is what makes section 3.14's
"maintained once, copied into no repository" viable for them unchanged.

**Three assertions hold for a delivered skill**, wherever the file lands:

1. No skill names a gate flag its project's `AGENTS.md` omits.
2. A read-only skill never invokes a writing verb.
3. Each skill wraps the tool verbs it exists for, rather than restating them.

**Seven contracts hold for a delivered hook.** Each was written after a measured
failure, and the contract is worth more than the shell that carries it:

1. **Read, never repair.** No hook may invoke a writing subcommand. A hook fires
   where it cannot commit what it regenerated, so a writing hook leaves the
   derived tree dirty; an orchestrator that refuses to start on a dirty tree then
   never starts, and one adopter's pipeline stalled eleven hours on dirt it had
   produced itself. The single sanctioned exception is a `compile` after an edit
   to a `spec.md`, where the session is live and can commit the result.
2. **Resolve the binary in order**: `$SPEC_SPINE_BIN`, then the target
   repository's own `target/release/spec-spine`, then `PATH`. A repository that
   builds its own binary must be governed by the one it builds; the `PATH`
   fallback keeps an adopter on the published CLI working. A bare name resolved
   from `PATH` alone is whichever copy the last unrelated project installed.
3. **Resolve the target repository from the command, not from the session.** A
   multi-repository session pushes and edits in whichever tree the command names.
4. **Read the verdict; never guess it.** `check` has four answers and they are
   not interchangeable: `0` fresh, `1` a corpus that does not validate, `2` stale
   or an unresolved claim, `3` a read that was not performed. Only one of the
   four is repaired by regenerating.
5. **Establish the verb before reading its exit code.** `clap` also spends `2` on
   an unknown subcommand, so a hook confirms the binary carries the verb
   (`check --help`) first. Without that, a binary older than the verb reports a
   fresh tree as stale and sends the session to regenerate shards that were
   already correct.
6. **A gate whose check did not run is not green.** Every non-zero code refuses.
7. **A branch gate resolves the protected branch rather than assuming `main`**:
   `$SPEC_SPINE_DEFAULT_BRANCH`, then the remote's own `HEAD`, then `main` as a
   floor. It refuses only a push that would actually update that branch, so a tag
   push from the default branch is allowed, and it is anchored on the command
   that invokes the verb, so a `grep` or a heredoc merely containing the text
   still runs. A pull-request gate runs the coupling gate before the create verb
   and refuses without a human-written waiver line in the body.

**A deny list is a safety floor, not an adapter's optional extra.** Where a
delivery carries permissions at all, the refusals travel with it: no publish
verb, no release verb, no force push, no recursive removal of a corpus or a
derived tree.

**Whoever owns the files owns the assertions.** In spec-spine the three skill
assertions and the seven hook contracts are enforced by
`crates/spec-spine-core/tests/harness_hooks.rs` (1124 lines, which extracts each
hook body and runs it as a program over a matrix of command spellings and branch
names) and `harness_skills.rs` (about 800). A hermetic test cannot read `$HOME`,
so neither file survives the move on its own: they are reimplemented where the
files land, or the requirements become unenforced. That cost is small and it is
not optional, and it is the second reason section 3.22's ordering is not
negotiable.

The harness this build ships under section 3.14 is deliberately small, and
adopting the inventory above would not change that judgment by itself: the point
of a global harness is that it is one source, not that it is a large one.

### 3.24 The consented settings modification

Section 3.14 rule 1 admits exactly one write into a harness's own settings file.
It is the narrowest thing that lets a hook the harness ships actually fire, and
everything about it is shaped so that a user who never consents is in the same
position as before this section existed.

**What it may carry, and nothing else.** Two kinds of line:

1. A **hook registration** whose command resolves inside the canonical harness
   under the product home. Never a command assembled from anything else.
2. A **deny entry**, which is a refusal.

A merge may add a refusal. It may never add or widen a permission: no allow
entry, no `ask` downgraded, no existing deny removed, weakened or reordered. The
deny list travels as a floor (section 3.23), and a floor that a delivery can
lower is not one. `settings.local.json` is the user's own override layer and is
never written at all.

**Consent is a separate act from installation.** The modification is named in the
`home apply` plan before anything is written, in the exact lines it would add,
and is **refused by default**: installing skills and agents does not perform it,
and neither does any read, test or project operation. It is performed only when
the operator consents to that modification specifically. Consent to one revision
is not consent to the next: a modification whose lines have changed is presented
again.

**One marked region, recorded as a modification.** Every managed line lives
inside a single marked region in the file. Outside that region nothing is
rewritten, reordered or reformatted, and the file's own shape is preserved. The
region is recorded in the home the way section 3.13 records the root instruction
bridge: as a **modification** (path, the exact lines, the digest before and the
digest after), never as a managed entry and never as ownership of the file.

**Reversible, and only while it is intact.** Removal removes exactly the marked
region and nothing else, and only while the region is present and byte-identical
to what was recorded. A region a user has edited is **reported and left**: an
edited region is a user's file again, and this product does not take it back.
Applying the modification twice changes nothing.

**A conflict is named, not resolved.** Where the user already registers a hook on
the same event with a different command, both remain and the situation is
reported. This product does not decide which of two hooks a user wants.

**What this does not become.** It is not a general settings manager, not a
migration, and not a path to any other key. A settings key this section does not
name is not writable by any code path, and adding one is an amendment to this
section rather than a use of it.

## 4. Out of scope

Installing the product itself; provider authentication; hosted registration;
multi-machine environment sync; and any write to a target outside the manifested
set. Publication, release and distribution are deferred by `F-02`; how this
product is itself packaged is recommendation `D-01`.

Implementing a hosted platform, a platform protocol or a platform client; any
user interface, which `F-04` defers; publication, release and distribution,
which `F-02` defers; multi-machine environment sync; a package registry or a
plugin marketplace; migrating sibling repositories or any real home directory;
adding a provider adapter, which is `008`'s boundary and `F-07`'s deferral; and
any new work, run or acceptance ledger, because `003` and `005` already own
those semantics and a second one would be the failure this product exists to
avoid.

## 5. Decisions recorded during implementation

Dated entries for choices §3 was silent on. None changes what it requires.
The entries from 2026-09-20 concern the realignment and were recorded
against `010` while that spec was separate; they are kept verbatim, because
this section is a history and a history is corrected by appending.

**2026-09-16: the command surface is not in this crate.** §3 names
`project register`, `env plan`, `env apply`, `env upgrade`, `env remove` and
`doctor`. Those are the operator's vocabulary; this spec's territory is
`crates/statecraft-environment/`, and D-02 gives the binary its own crate
(`statecraft-cli`), which no ratified spec owns. So every behavior above is
implemented here as a library operation, and binding it to a process waits for
the spec that owns a binary. The grade this spec claims is therefore
*implemented* for its own territory, and nothing about a command line.

**2026-09-16: timestamps are formatted in-crate.** The manifest is committed and
read by people, so `written_at` is RFC 3339 rather than a count of seconds. A
calendar dependency for one field was not worth it in a crate whose job is to
own as few bytes as possible, so `time::rfc3339_utc` does the conversion and is
tested against the Gregorian century rules that a naive implementation gets
wrong.

**2026-09-16: `unmanaged-write` is decided by declaration, not by memory.** §3.2
calls a path the product wrote but did not record a defect, and this product
keeps no second record to compare against. `doctor` therefore reports
`unmanaged-write` for a path that is present on disk, declared by one of the
configured adapters, absent from the manifest, and claimed by no other
installer. The last clause is what keeps it distinct from `foreign`.

**2026-09-16: `shadowed` needs a resolver this product does not have.** §3.7
says a shadow is reported only where the product can observe one. `ShadowResolver`
is the seam, and the shipped implementation observes nothing, because no harness
package format exists to interrogate. A digest match therefore still reports
`present` today, and the state is reachable and tested through an injected
resolver rather than dead code.

**2026-09-20: the producer dependency is an exact crates.io pin, and the
boundary it names is not yet satisfied.** Section 3.5 requires an exact,
reproducible dependency and forbids a filesystem path to a sibling checkout.
This build depends on `spec-spine-core = "=0.21.0"` with `default-features =
false`, which drops the `symbol-resolution` feature and its pinned tree-sitter
grammars; the scaffolder needs none of them. That release **still returns
`AGENTS.md` and three `.claude/rules/` files** alongside the contract set,
because trimming the producer is another repository's change and has not
shipped. This product places neither and reports the producer `non-conforming`,
which makes an initialization against the real library `partial`.
`crates/statecraft-home/tests/producer_integration.rs` asserts exactly that, and
`the_producer_is_not_yet_conforming` is the single test that changes when the
trimmed producer releases. No fixture substitutes for the boundary: where a
conforming answer is needed, the suite derives one from the real library's own
bytes and labels it a fixture.

**2026-09-20: the trimmed producer was tested before it was released, under a
temporary override that is not committed.** Section 3.5 permits a local source
override for development verification and forbids committing one. Measured on
this date against the sibling spec-spine working tree at
`eb37615f34c5c5327d05e87189f08b4c4af38c0c`, branch
`120-the-engine-ships-governance-not-an-environment`, three paths dirty,
reached through a `[patch.crates-io]` entry added to this workspace's manifest
and reverted immediately afterwards: `scaffold_init_json` returns only the
contract set, the conformance reads `conforming`, and the end-to-end
initialization reports **`complete`** where the released `0.21.0` reports
`partial`. The three tests that assert today's non-conformance fail against
that tree, which is exactly what they are for. **Nothing about a release is
claimed**: the committed dependency stays `=0.21.0`, and adopting the trimmed
producer is a pin bump plus those three inversions, as its own change.

**2026-09-20: the governance files are written by this crate, and every rule
about whether to write them is still 002's.** Section 3.5 reconciles through
spec 002's ownership model. `statecraft_environment::plan::plan` computes what
is written and what is withheld, exactly as it does for an adapter, against a
declaration this crate synthesizes for the pseudo-harness
`statecraft-governance`. The write loop is here rather than
`apply::perform`, because a governance starter file's source is a **template**
and not an adapter, and adding a source-kind field to `Declaration` would have
broken every construction site in spec 008's crate for a field spec 008 does not
need. The decisions stay in one place; only the write does not.

**2026-09-20: a contract path already on disk is adopted, and adoption happens
before planning.** Section 3.5 says an existing path is depended on and never
rewritten. Spec 002's plan reaches that outcome for a path the manifest records
as `adopted`, so the reconcile step records the adoption first and the
governance step then withholds the write with 002's own reason. Nothing new
decides it.

**2026-09-20: the native root is a parameter, never a read of the process
environment.** `STATECRAFT_NATIVE_ROOT` exists and is read at the command
surface only. Every operation takes the parent directory explicitly, so an
operation always states where it would write, and a test stays out of the
operator's real home without mutating a process-wide variable that its
neighbours in the same test binary would also see.

**2026-09-20: hooks ship in the harness and are delivered by nobody.** Section
3.4 refuses to rewrite a user's own settings file, and every mechanism for
wiring a hook into the harnesses this product knows about goes through one. So
`hooks/statecraft-gate.sh` is part of the canonical harness, is content
addressed with the rest of it, and is installed by the operator if they want it.
Native delivery links skills and agent definitions and names no hook, and a test
holds that.

**2026-09-20: the delivery verdict is evaluated from the file tree, and no
verdict comes from a provider run.** Section 3.4 requires delivery to be
evaluated rather than assumed, and a provider run costs credit and proves one
session rather than a rule. So a harness's documented load rule is followed from
its entry file through `@path` imports against the real tree. `claude-code`
reports `reached` with the chain; `codex-cli` reports `unverified`, because its
documentation does not establish the expansion. Neither verdict claims anything
about a live session, and the suite spends nothing.

**2026-09-20: what the relocation rewrites, and what it leaves.** Section 3.9
moves the artifacts and the configuration together. The operation rewrites a
**quoted string literal** in `spec-spine.toml` and an ignore pattern that starts
with the old directory, and nothing else. Prose in a comment that mentions the
old location is left alone: an operation that edited English would be guessing
at meaning. The documents are updated by the change that performs the move,
which is this one.

**2026-09-20: the manifest goes to version 2 with no migration.** Section 3.2
adds the project block and the tracked modifications. Nothing is released and no
version-1 file exists outside a test, so a migration would be machinery
maintained for nobody. A version-1 file is refused by version, which is the
existing behavior and names the number it found.

**2026-09-20: two references to the old location are left for their owners.**
`crates/statecraft-run/tests/negative_cases.rs` builds a `.derived/` directory as
a temptation the product must not read, and `docs/decisions/00-founding-decisions.md`
mentions the old path in prose.
Both are other specs' territory (`003` and `001`), the negative control still
holds against any shard directory, and widening this change to reach them would
be editing a spec's territory for a cosmetic improvement. They are recorded here
as a follow-up rather than swept.

**2026-09-20: ratified, and the draft exception it needed is retired.** This
spec was written and built as a `draft`, which AGENTS.md records as the one
exception this corpus has granted. The owner ratified it on the date above, so
`status` is `approved` and `implementation` is `complete` against the evidence
this file's Verification block declares. The exception was for this realignment
and nothing else: the passages in AGENTS.md and CLAUDE.md that carried it are
removed by this change, and the general rule they qualified is unchanged. No
agent ratified anything.


**2026-09-21: the dependency on `006` becomes `extends` and `amends` alone.**
While the realignment was spec `010`, it could name `006` in `depends_on`: this
spec did not, so the graph stayed acyclic. Merged, `002` both depends on `006`
and is depended on by it, and `compile` refuses the cycle (`V-014`). The two
edges that carry the real relationship stay and say more than the dropped line
did: the `extends` edge names the exact crate the verbs are bound in, and the
`amends` edge names what §3.16 reads differently in `006` §3.6. The same
resolution was taken in `004` on the same date and for the same reason.

**2026-09-21: the spec-spine harness handoff is folded in here, and two contracts
the shipped hook does not yet meet.** The handoff arrived as
`docs/design/01-spec-spine-harness-handoff-2026-09.md` and is now §3.22 and
§3.23. It was filed rather than folded by mistake: a design note beside the
corpus is a second place for a requirement to live, and AGENTS.md's source
ownership table has recorded since the founding record moved that this repository
keeps no design directory. Nothing was dropped in the move; the half reporting
that §3.7 still named a live counterparty was already discharged by §3.7 and
§3.21 before the note landed, and the half about spec-spine's renumbered ordinals
is AGENTS.md's "Citing another corpus", written in this same change.

Folding it in made two gaps measurable that a loose file had left unmeasured.
The harness `hooks/statecraft-gate.sh` that `harness::shipped()` carries predates
§3.23 and does not satisfy contract 2 or contract 5: it invokes a bare
`spec-spine` resolved from `PATH` with no `$SPEC_SPINE_BIN` and no repository-local
`target/release/spec-spine` ahead of it, and under `set -eu` it reads the exit
code without first establishing that the binary carries the verb, so `clap`
spending `2` on an unknown subcommand is indistinguishable from a stale tree.
Both are real against the contracts and neither is repaired here: a change to a
hook is an authority change under AGENTS.md "Approval semantics", decided on its
own and never bundled with the change that would authorize it. The gap is
recorded, named precisely, and left for that decision. Contracts 1, 3, 4, 6 and 7
the shipped hook already meets, and its project gate is the test §3.14 rule 3
requires, which is what makes it inert outside a Statecraft project.

**2026-09-21: the two hook contracts are repaired, and the assertions now live
here.** The entry above recorded `hooks/statecraft-gate.sh` failing §3.23
contracts 2 and 5 and left the repair for its own decision, because a change to
a hook is an authority change. The owner asked for it, so this is that change
and nothing else rides along with it.

The hook now resolves `$SPEC_SPINE_BIN`, then the target repository's own
`target/release/spec-spine`, then `PATH` (contract 2); probes `check --help` and
`lint --help` before reading any exit code, so a binary older than the verb is
refused by name instead of reported as a stale tree (contract 5); reads each of
`check`'s four answers as itself and exits with the code it was given, rather
than letting `set -e` collapse them (contract 4); refuses when no binary can be
executed inside a Statecraft project (contract 6); and takes the target
repository as its argument rather than from the session's working directory
(contract 3). It remains inert outside a Statecraft project, which is §3.14 rule
3 and a different thing from contract 6.

**§3.23's "whoever owns the files owns the assertions" is now discharged for the
hook.** `crates/statecraft-home/tests/harness_hooks.rs` extracts the shipped body
and runs it as a program against each contract, with a stub `spec-spine` that
records which binary was chosen. Asserted by execution rather than by reading the
source for a phrase, because a script that mentions a contract in a comment would
pass the second and fail the first. Measured against the pre-repair body, eight of
its ten tests fail; the two that pass are the properties that body already had.

Two defects in the first draft of those tests are worth recording, because both
produce a green suite that establishes nothing. Its fixture set `PATH` to the
stub directory alone, which removed `git` along with everything else, so the hook
found no repository and exited 0 having run nothing, and three assertions of the
form "every line of the witness is X" passed on an empty witness. The fixture now
puts a `git` shim in that directory and `assert_only_ran` refuses an empty
witness first. Separately, a scan for writing verbs over the whole body tripped on
the word "re-indexing" inside a message; the answer was not to reword the message
but to assert over the binary in command position, since a scan that cannot tell
an executed word from a printed one gets worked around rather than fixed.

**2026-09-21: the owner amended §3.14 rule 1, and this spec is no longer
`complete`.** §3.22 records that spec-spine's `.claude/settings.json` waits on
this product. Measuring the two sides against each other made the contradiction
plain: rule 1 as written refused every write to a user's settings file, and
`delivery::NEVER_TOUCHED` enforced that in code, so the four hook events and the
deny list had no route here and the leanout could not finish. Three ways out were
put to the owner: spec-spine keeps that file as its own governance, rule 1 is
amended, or the operator pastes a wiring snippet. **The owner chose the
amendment**, and §3.24 is it.

What §3.24 deliberately is not: a settings manager. It carries two kinds of line
and no others, it may add a refusal and never a permission, and a key it does not
name is not writable by any code path. It is modelled on §3.13's root instruction
bridge, which is this corpus's existing answer to touching a file it does not own:
a marked region, recorded as a modification with the digest either side, removed
only while intact, and reported rather than reclaimed once a user has edited it.

**This change is the authority change and nothing else.** AGENTS.md separates one
from the implementation it would authorize, so no code moves here.
`delivery::NEVER_TOUCHED` still lists `settings.json`, `native_destination` still
links only `skills/` and `agents/`, and no consent flow exists. §3.24 is therefore
**specified and not implemented**, and `implementation` moves from `complete` to
`in-progress` rather than staying a claim this tree does not support. That also
makes the remaining work schedulable, which is where the next session should find
it.

**Two corrections to §3.22's inventory, measured 2026-09-21 rather than taken from
the note.** A history is corrected by appending, so §3.22 stands and this is the
correction. That `settings.json` carries **four** hook events, not three:
`SessionStart`, `PostToolUse`, `PreToolUse` (which holds the push gate and the
PR gate together) and `Stop`. And there is a fifth class the note did not
mention, `.claude/agent-memory/`, whose disposition nobody has decided. The rest
of §3.22 was checked and holds, including the global push gate at
`~/.claude/hooks/push-gate.sh`, which is installed and registered.

## Verification

Each line is one command. They run the acceptance this spec's behavior declares:
every row of §3.10 is one integration test, named after the row it covers, so a
row that stops being covered shows up as a deleted test rather than as a
quietly weakened assertion.

`--fail-on-untraced` joins the corpus gate with this change, which is the
condition AGENTS.md recorded for it: coverage is 13/13 specifically claimed, so
the flag now defends that number instead of reporting it.

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
cargo test -p statecraft-home --test harness_hooks
```
