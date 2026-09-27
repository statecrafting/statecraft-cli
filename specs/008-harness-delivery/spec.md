---
id: "008-harness-delivery"
title: "Harness delivery, hooks, and skills"
status: approved
implementation: complete
created: "2026-09-26"
summary: >
  Carries the environment adapter, delivered harness, consented settings, required identity, and startup-delivery requirements relocated from spec 002. It keeps repository initialization separate from the harness that a registered adapter delivers.
depends_on:
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
extends:
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-environment/" }, nature: additive }
  - { spec: "002-environment-lifecycle", unit: { kind: directory, path: "crates/statecraft-home/" }, nature: additive }
relocates:
  - { spec: "002-environment-lifecycle", from: "3-9-adapters", to: "3-9-adapters" }
  - { spec: "002-environment-lifecycle", from: "3-14-one-global-harness-delivered-by-adapters-copied-into-no-repository", to: "3-14-one-global-harness-delivered-by-adapters-copied-into-no-repository" }
  - { spec: "002-environment-lifecycle", from: "3-22-the-counterparty-s-state-and-the-order-the-last-of-its-harness-moves-in", to: "3-22-the-counterparty-s-state-and-the-order-the-last-of-its-harness-moves-in" }
  - { spec: "002-environment-lifecycle", from: "3-23-what-delivered-harness-content-must-satisfy", to: "3-23-what-delivered-harness-content-must-satisfy" }
  - { spec: "002-environment-lifecycle", from: "3-24-the-consented-settings-modification", to: "3-24-the-consented-settings-modification" }
  - { spec: "002-environment-lifecycle", from: "3-25-the-required-harness-identity-and-the-resolved-one", to: "3-25-the-required-harness-identity-and-the-resolved-one" }
  - { spec: "002-environment-lifecycle", from: "3-26-startup-delivery-evidence", to: "3-26-startup-delivery-evidence" }
  - { spec: "002-environment-lifecycle", from: "3-27-global-adapter-registration-is-not-managed-session-permission-delivery", to: "3-27-global-adapter-registration-is-not-managed-session-permission-delivery" }
  - { spec: "002-environment-lifecycle", from: "3-28-an-unmarked-registration-is-the-user-s-and-resemblance-is-not-ownership", to: "3-28-an-unmarked-registration-is-the-user-s-and-resemblance-is-not-ownership" }
---

# 008: Harness delivery, hooks, and skills

## 1. Purpose

The product installs one governed harness through registered adapters without copying that harness into target repositories. Delivery is exact, consented where it modifies user-owned settings, and diagnosable from persisted identity and startup evidence.

## 2. Territory

The requirements below govern adapter declarations, the global harness, its hooks and skills, settings consent, required identity, and delivery evidence. Code remains owned by spec 002's crates; this relocation changes no code ownership.

## 3. Behavior

### 3.9 Adapters

An agent-harness adapter declares: the harness it targets; the exact set of paths
it would manage; the facts it cannot express in that harness; and the
prerequisites it needs present. Adapters are additive and independent, and two
adapters may not declare the same path.

An adapter whose harness is absent, or whose prerequisites are missing, **refuses
to claim its paths** and says which prerequisite is absent. It does not write
files for a harness that is not there.

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

**The inventory offered, and adopted on 2026-09-21.** Ten skills (`prime`,
`next`, `build`, `verify`, `ship`, `shepherd`, `spec`, `commit`, `code-review`,
`setup`) and four agents (`architect`, `explorer`, `implementer`, `reviewer`).
They are already **repository-invariant**: every project-specific fact lives in
that project's `AGENTS.md`, which each skill ends by pointing at. That property
was built for a distribution that was then cancelled, and it is what makes
section 3.14's "maintained once, copied into no repository" viable for them
unchanged.

The owner adopted the whole inventory as Statecraft harness content, subject to
the repository-invariant project-layer boundary above. What is delivered is a
**Statecraft-namespaced equivalent** of each, which is section 3.14 rule 2 and
not a new condition. The four event behaviors are adopted with it:
`SessionStart`, `PostToolUse`, `PreToolUse` and `Stop`, including the push gate
and the pull-request gate and the assertions each carries. `Stop` is adopted
under the advisory policy below, not as a gate.

**Adoption is delivery, and delivery is not authorization.** A delivered skill
may be invoked; it acquires no standing permission by being delivered. Nothing
in this adoption lets a skill publish, merge, release or execute without the
authorization that operation independently requires, and a skill that reads as
though it did is a skill to fix rather than an authority to infer. The deny
floor below and section 3.14 rule 3's project gate both continue to apply to
every adopted name.

**The project layer keeps its facts.** Repository invariance is a condition on
the delivered content, checked rather than assumed: a generic skill does not
hardcode any project's crate layout, gate commands or workflow, and a project's
`AGENTS.md` stays the authority for those. That applies to the counterparty's
own repository as much as to any other, and an adopted file carrying
spec-spine's specifics is an adoption defect.

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

**An end-of-turn hook advises; an operation gate enforces.** Contract 6 above
is a rule about **gates**, and a harness's end-of-turn event is not one. The
owner settled the policy on 2026-09-21 and it has three parts:

1. **Stop is advisory, and reports the result accurately.** It says what the
   check answered, including that the check could not be performed. It does not
   convert that answer into a block.
2. **An enforcing operation gate refuses a failed or unavailable check.** A push
   gate, a pull-request gate and any other gate standing in front of an
   operation refuse on every non-zero code and on a check that did not run,
   which is contract 6 unchanged.
3. **Stop never prevents a useful failure handback.** A session that has
   something to report, including a failure, hands it back. A stale derived tree
   or a read that was not performed is a fact the handback carries, never a
   reason to withhold it. The asymmetry is the point: a gate that wrongly
   refuses costs an operation that can be retried, and an end-of-turn block that
   wrongly fires costs the report of why the work failed, which is the thing
   nobody can reconstruct afterwards.

This changes no hook's repair behavior. Contract 1 stands exactly as written:
read, never repair, with the single sanctioned exception of a `compile` after an
edit to a `spec.md`, where the session is live and can commit the result. No
part of the Stop policy authorizes a hook to write.

**Two exit vocabularies, and no numeric passthrough between them.** Contract 4
reads spec-spine's codes. Spec `006` §3.3 fixes this product's own. They are
different closed vocabularies that share the integers, and three of the four
overlapping values disagree:

| Code | spec-spine `check` | Statecraft command (`006` §3.3) |
|---|---|---|
| 0 | fresh | did what was asked, found nothing wrong |
| 1 | the corpus does not validate | a finding: a diagnostic state, a withheld write |
| 2 | stale, or an unresolved claim | refused: a precondition was not met |
| 3 | the read was not performed | usage error: no such operation |
| 4 | not used | failed |

Propagating a spec-spine code as a Statecraft code would report a stale tree as
a refusal and an unperformed read as a usage error. Where this product runs a
governance verb and answers in its own vocabulary, it **translates**:

| spec-spine `check` answered | this product reports |
|---|---|
| 0 fresh | 0 |
| 1 the corpus does not validate | 1, a finding: the check ran and the corpus is what it found |
| 2 stale, or an unresolved claim | 1, a finding, and the two readings are distinguished in the text, not in the code |
| 3 the read was not performed | 4, a failure: nothing about the corpus was established |
| the binary is absent, or lacks the verb | 2, a refusal: a precondition was not met and nothing was done |

An enforcing gate collapses the same answers to green and not-green, where only
`0` is green. That is contract 6, and it is not a different translation: it is
this one, read by something that has only two outcomes to spend.

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

The harness this build ships under section 3.14 was deliberately small, and the
adoption above does not change the judgment behind it: the point of a global
harness is that it is one source, not that it is a large one. What grew is the
inventory the owner decided to carry, and every added file still pays the same
price, which is the assertion that judges it.

### 3.24 The consented settings modification

Section 3.14 rule 1 admits exactly one write into a harness's own settings file.
It is the narrowest thing that lets a hook the harness ships actually fire, and
everything about it is shaped so that a user who never consents is in the same
position as before this section existed.

**What it may carry, and nothing else.** Two kinds of line:

1. A **hook registration** whose command resolves inside the canonical harness
   under the product home. Never a command assembled from anything else.
2. A **deny entry**, which is a refusal. Where such an entry lands is fixed by
   section 3.27: not in the user's global deny list by default, because a deny
   entry carries no project gate and acquires none from the scripts registered
   beside it.

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

**One recorded modification, several valid locations.** All Statecraft-managed
insertions are tracked by one recorded modification. They may occupy multiple
syntactically valid locations. Each insertion is identified by its exact
content, structural location, and recorded provenance. Unrelated bytes are
preserved, and removal occurs only when Statecraft can establish that the
content is an intact insertion it owns.

That is the contract the owner approved on 2026-09-21, replacing a requirement
that every managed line occupy one physically contiguous marked region. The
replacement is about **representation only**. What the region requirement was
carrying is not physical adjacency but attributability, and attributability is
what the three identifying properties above supply: the same bytes, in the same
place in the document's structure, with a record in the home that says this
product put them there. Adjacency was one way to get it, and in a syntax with
two non-adjacent insertion points it is not an available one. Section 5's
2026-09-21 entries measure why.

The record itself is unchanged: the home records **one modification** (path, the
exact content, the digest before and the digest after) the way section 3.13
records the root instruction bridge, never as a managed entry and never as
ownership of the file. Outside the recorded insertions nothing is rewritten,
reordered or reformatted, and the file's own shape is preserved.

**The file stays strict JSON.** The representation this section permits is the
one the harness's own parser already accepts. It does not extend to JSON5, to
comments outside string values, to a deny entry that refuses nothing and exists
only to mark a boundary, or to a metadata key the harness does not support. A
marker this product needs travels inside content the harness already reads as
content, or it does not travel.

**Reversible, and only while ownership can be established.** Removal removes
exactly the insertions this product can establish it owns, and nothing else. An
insertion whose content no longer matches what was recorded is **reported and
left**: an edited insertion is a user's file again, and this product does not
take it back. Where ownership cannot be established, the insertion stays and
the outcome says which one and why; losing the evidence costs a refusal
nothing. A marker resembling this product's is not by itself proof of
ownership, and a pre-existing user refusal is never claimed as a managed
insertion. Applying the modification twice changes nothing.

**A conflict is named, not resolved.** Where the user already registers a hook on
the same event with a different command, both remain and the situation is
reported. This product does not decide which of two hooks a user wants.

**What this does not become.** It is not a general settings manager, not a
migration, and not a path to any other key. A settings key this section does not
name is not writable by any code path, and adding one is an amendment to this
section rather than a use of it.

### 3.25 The required harness identity and the resolved one

Section 3.14 makes a harness revision content addressed. This section says who
records which revision, and what a managed session does when the two answers
disagree. The owner settled it on 2026-09-21.

**Two records, and they are not the same record.**

- The **required** identity is a **committed project requirement**. It travels
  with the repository, it is reviewed like any other committed change, and it
  is what the project says its managed sessions must run against.
- The **resolved** identity is recorded **per managed session**, separately,
  and says which revision actually answered. Section 3.16's last rule already
  freezes it; this section fixes that the requirement it was resolving against
  is a committed one rather than whatever the home happened to hold.

Keeping them apart is what makes disagreement visible. One record that is
rewritten as it is read cannot disagree with anything, which is precisely the
failure this section refuses.

**The full digest is the integrity proof.** A revision's identity is the digest
over its files, as section 3.14 fixes it. The **full** digest is what the
required record carries and what an integrity comparison uses. A short
identifier derived from it is a **display** convenience: legible in a plan, in a
verdict and in a log, and never on its own the thing an equality check is
performed against. A truncated identifier that two revisions could share is not
a proof, whatever the odds are.

**Managed execution refuses.** A session that would be managed under a required
identity refuses when the required content is **missing** (no such revision is
installed), **corrupt** (a revision is installed under that identity and its
files no longer digest to it), or **mismatched** (the resolved revision is not
the required one). The refusal is a refusal in this product's vocabulary: a
precondition was not met and nothing was done.

**Four things stay possible under that refusal**, because a refusal that
prevents diagnosis is worse than the state it refuses: **inspection** (what is
required, what is installed, what each digests to), **diagnosis** (`doctor`
reporting the disagreement), **planning** (what an apply or an upgrade would
do), and an **explicit upgrade**.

**Two things this product never does.** It never **silently selects the latest
installed revision**: an installed revision that is not the required one is a
mismatch to be reported, not a substitute to be chosen, and "the newest one is
probably right" is how a project loses the ability to say what it ran. And it
never **rewrites the project requirement during a read**: inspection, doctor
and plan are reads, and a read that repairs its own precondition destroys the
evidence that the precondition was unmet, which is the same defect the gate
refuses in `compile` (AGENTS.md, "New sessions").

**An upgrade is an explicit reviewed project change.** Changing the required
identity is a committed change to the repository, proposed and reviewed like
one. It is never a side effect of installing, of running, or of a newer
revision appearing under the home.

**An upgrade renews settings consent when the content changes.** Where the
consented settings modification of section 3.24 embeds the revision, a new
required identity produces different modification content, and different
content is presented for consent again. That is section 3.24's rule
("consent to one revision is not consent to the next") reached from the other
direction, and it is stated here so an upgrade path cannot satisfy itself by
reusing a consent given for other bytes. Where the modification content is
byte-identical across the upgrade, there is nothing new to consent to and
nothing is asked.

### 3.26 Startup delivery evidence

Section 3.14 fixed a three-valued verdict over a harness's documented load
rule. This section fixes what a managed session **records** about delivery, and
what that record is and is not allowed to claim. The owner settled it on
2026-09-21.

**What is recorded.** Seven things, at the start of a managed session:

| Recorded | What it is |
|---|---|
| project identity | which repository, and the manifest that makes it a target |
| instruction-file identities | each instruction file reached, by path and digest |
| required harness identity | the committed requirement of section 3.25, full digest |
| resolved harness identity | what actually answered, full digest |
| adapter identity | which adapter performed the delivery, and its own identity |
| load chain | the files traversed, entry first, managed file last |
| delivery status | the section 3.14 verdict |

**Three statements, kept distinct and never substituted for one another.**

1. **The documented load chain reaches a file.** A rule was evaluated against
   the tree and arrives. This is a statement about the tree and the rule.
2. **Its bytes were resolved and supplied.** The file was read, it digests to
   what is recorded, and its content was handed to the session. This is a
   statement about what this product did.
3. **A live session demonstrated the expected behavior.** A real session was
   observed behaving as the instructions require. This is a statement about a
   session, and only a session can produce it.

Each is strictly weaker evidence for the next, and none of them implies the
one after it. A record that states the first must not be read as the second,
and a record that states the second must not be read as the third.

**What a digest never proves.** A digest establishes that bytes are the bytes.
An acknowledgement establishes that something emitted an acknowledgement.
Neither establishes that a model **read**, **understood** or **complied with**
the instructions, and no field in this record makes that claim. The three
words are listed because each is a distinct overclaim and all three are easy to
write by accident.

**The three verdicts keep their meanings.** `reached`, `not-reached` and
`unverified` mean exactly what section 3.14's table says. The new evidence is
carried in **added fields**, narrowly defined, alongside the verdict. In
particular `reached` is not quietly widened to mean supplied, and is not
narrowed to require a live observation: a rule that arrives is a rule that
arrives, and the fact that this is weaker than what an operator often wants is
the reason it is reported separately rather than merged into a single richer
word. Redefining an existing verdict changes the meaning of every record
already written under it, which is a migration and not an improvement.

### 3.27 Global adapter registration is not managed-session permission delivery

A narrow amendment, settled by the owner on 2026-09-21 and recorded before the
implementation it authorizes. It separates two things this build had joined:
registering an adapter **globally**, and delivering the deny floor to a
**managed session**.

**Section 3.14 rule 3 is unchanged and is the reason.** Every delivered
behavior applies only inside a repository holding
`.statecraft/environment.json`, and is inert everywhere else. An unrelated
project is unaffected by global integration.

**The floor does not go into the user's global deny list by default.** A
refusal written into the user-global `permissions.deny` applies to every
session that user runs, in every repository, managed or not. Hook *scripts* can
be project-gated because a script can test for the manifest and exit; a deny
entry is evaluated by the harness before anything of this product's runs, so it
carries no gate and acquires none from the scripts shipped beside it. Those
entries are therefore **not project-gated merely because the hook scripts
are**, and this product does not install them there by default.

**The floor is delivered through a supported managed-session settings
mechanism**, carrying the canonical Statecraft content. Claude Code documents a
`--settings` argument for exactly this shape of need. Documentation is not
evidence: the **installed version** and the **effective behavior** are verified
before this product relies on the mechanism, and an unverified mechanism is an
unavailable one.

**Three conditions on whatever global registration remains.** A global hook
registration, if used at all, is **inert outside a Statecraft project**.
Delivered skills and agents stay **namespaced and project-gated**, which is
section 3.14 rules 2 and 3. And consent asked for a write into a global native
settings file **describes the actual scope of that write**: a modification that
takes effect in every repository is presented as one, whatever the scripts it
registers do afterwards.

**The floor is never weakened to solve a delivery problem.** Section 3.23's
refusals are a floor, and a floor a delivery can lower is not one. Where an
ordinary unmanaged session cannot receive the scoped floor, the answer is to
**report that session as not qualified** for the managed-execution claim.
Two repairs are specifically refused: lowering the floor so that delivery
succeeds, and writing a repository-local generic harness copy so the
limitation stops being visible. The second would also reintroduce exactly what
section 3.14 removes.

### 3.28 An unmarked registration is the user's, and resemblance is not ownership

Settled by the owner on 2026-09-21. Section 3.24 already says that a conflict
is named rather than resolved; this section says what "named rather than
resolved" means for a registration that already exists, and closes the one
route by which this product could take one over by accident.

**Existing registrations are preserved.** The user's own hook registrations
stay, including the global push gate §3.22 records at
`~/.claude/hooks/push-gate.sh`. Coexistence is the behavior to exercise:
this product's delivery runs alongside them and is tested doing so. Where a
later replacement is wanted, it is prepared as an **exact plan** and performed
as its own reviewed act, never folded into a delivery.

**Resemblance is never ownership.** This product does not delete, take
ownership of, or rewrite an **unmarked** registration, and it does so least of
all when the registration looks like content this product ships. A user who
copied a shipped hook, or wrote one that converged on the same commands, owns
what they wrote. Ownership is established the way section 3.24 establishes it,
by exact content, structural location and recorded provenance together; a
marker resembling this product's satisfies one of the three and proves
nothing.

**The keys this product does not touch.** `defaultMode`, `allow` entries, `ask`
entries, model selection, and every other user setting section 3.24 does not
name. That section's closing rule is the general form: a key it does not name
is not writable by any code path. This section names the ones an integration
is most tempted by, because each of them would make a delivery smoother and
each is the user's decision.

**Qualification is measured, not read off the configuration.** Where this
product judges whether a session is qualified, it inspects **effective
behavior**. A deny entry present in a settings file establishes that the entry
is configured; it does not establish that it was enforced, in this version, in
this session, for this command. The two are different claims and only the
second qualifies a session.


## 4. Out of scope

- Provider execution and transport semantics, owned by spec 004.
- Admission of live observations and managed-session trial evidence, owned by spec 015.
- Governance producer selection, owned by spec 009.
- Repository lifecycle and ownership transfer, retained by spec 002.

## 5. Resolved decisions

**2026-09-26: delivery is a separate responsibility from lifecycle.** A repository may be initialized without duplicating the global harness. The relocation preserves the previous requirements verbatim and leaves both environment crates owned by spec 002.

## Verification

Each line is one command.

```verify:cli
cargo test -p statecraft-home --test harness_hooks
test -f crates/statecraft-home/src/settings.rs
cargo test -p statecraft-home --test settings_modification
test -f crates/statecraft-home/src/session.rs
cargo test -p statecraft-home --test harness_skills
cargo test -p statecraft-home --test bounded_integration
test -f crates/statecraft-home/src/required.rs
test -f crates/statecraft-home/src/startup.rs
cargo test -p statecraft-home --lib required
cargo test -p statecraft-home --lib startup
cargo test -p statecraft-home --lib admission
cargo test -p statecraft-home --test qualification_admission
cargo build -p statecraft-home --example require-harness
cargo test -p statecraft-adapter-claude-code --test settings_transport
```
