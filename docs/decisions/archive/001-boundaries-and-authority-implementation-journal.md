<!-- Historical record only. Not normative. -->
<!-- Moved from specs/001-boundaries-and-authority/spec.md at base d77e011e39dbc4689e49492c4e528d860be7eb86. -->

# Archived implementation journal: 001-boundaries-and-authority

This file preserves the chronological implementation record that formerly
occupied section 5 of the active spec. Current requirements live in the
active spec's behavior section, and current rationale lives in its bounded
`Resolved decisions` section. Git remains the authoritative change history.


Dated entries for choices §3 was silent on. None changes what §3 requires.

**2026-09-19: the status descriptions corrected in this spec's territory, and
the evidence for each.** Section 3.3 requires evidence beside any claim above
*specified*. Four present-tense claims in the two documents this spec owns had
become false, and each is corrected with the measurement that falsified it. Each
was true when written; this entry records what changed, not a rule for changing
it.

| Corrected claim | Where it stood | The measurement |
|---|---|---|
| "no dependency declared below exists in any manifest, because no manifest exists" | the design record now folded in as section 3.8, status line | A Cargo workspace with seven member crates exists, and `crates/statecraft-run/Cargo.toml` declares `attest-ledger-core` pinned to `a9c3595`, used by `src/record.rs`. |
| `attest-ledger` listed as **proposed reuse** | same file, section 1 table | Same measurement: it is an actual dependency. The **reuse** disposition it was adopted under is unchanged. |
| "specs `008` and `009` are drafts", supporting deferral `F-10` | `docs/decisions/00-founding-decisions.md`, section 4 | `spec-spine registry list` reports both `approved` with `implementation: complete`. `F-10` stays deferred: whether the single-repository loop works is the owner's judgment, not a status field. |
| "one row of section 3 is adopted in part" | same file, adoption-status line | Section 5 of that file already records `D-03` adopted in full and `D-01` and `D-02` adopted for language and layout. The line is restated to match section 5, which stays the only place a row becomes binding. |

The original wording of each replaced claim is quoted in its replacement, and
the dated 2026-09-16 statements in both documents are left as written. Nothing
in this change adopts a `D-` row, lifts a deferral, or revises a disposition, an
acceptance requirement or a policy.

**2026-09-19: this spec's Verification preamble is stale, and the correction is
deferred.** The sentence introducing the block below reads "They assert nothing
about product behavior, because none is implemented." Measured on this date:
`cargo test --workspace` passes 515 tests across seven crates, and
`cargo run -p statecraft-cli -- --help` prints fifteen bound verbs, so the
clause after the comma is false. The commands themselves are untouched, they
still assert the authored foundation this spec owns, and `make verify SPEC=001`
passes unchanged. The owner has acknowledged the staleness and deferred the
correction, so the measurement is recorded here and the preamble is left as it
stands.

**2026-09-21: a borrowed ordinal is corrected in this spec's territory, and the
rule for reading one is recorded.** Section 3.8's tables and the decision record
this spec owns cite other corpora by ordinal. spec-spine collapsed its corpus and
renumbered contiguously on 2026-09-20, which makes an old ordinal worse than a
dangling one: it resolves, to a different document. Three citations in
`docs/decisions/00-founding-decisions.md` were corrected against
`docs/corpus-map.md` in that repository (097 to 078, 091 to 072, and `C-18`'s 102
to 081), and seven more elsewhere in the corpus.

What is **not** corrected is the point of this entry. `D-01`'s reasons cite the
archived predecessor's own spec 102, section 3.9 cites its 032, 040, 042 and 043,
and `crates/statecraft-envelope/PROVENANCE.md` cites hqgit's. Those belong to
other corpora, spec-spine did not renumber them, and remapping one would be the
same defect pointing the other way. The rule this spec now holds: an ordinal is
read together with the sentence that says whose it is, and a spec-spine ordinal
is trusted only from `registry list` or the corpus map. AGENTS.md carries the
operational form.

**2026-09-23: stale references reconciled, and nothing required changed.** The
2026-09-23 audit found references that were true when written and are not now.
The fixes in this change are:

- `.derived/` named where `002` section 3.19 moved the tree;
- `009` named as a live spec;
- the library dependency `spec-spine-core` missing from the component table;
- the grade table stopping at a row that called `002` implemented;
- the decision record's pre-renumbering spec-spine ordinals and its "no verb
  runs `spec-spine delta`".

Each is corrected in place where it was a current claim, or by an appended,
dated note where it records what was read at the time. The same pass corrected
cross-references in `002` to `006` and in `AGENTS.md`, and spec `000`'s own
stale descriptions ("contains no product code", `.derived/`, "none is
implemented"). Those are editorial: spec `000`'s section 5 says an anchor
forbids contradiction and leaves ordinary editorial amendment available, and
no anchored principle changes.

**2026-09-23: the CLI pin moves to `=0.23.0`, with its `D-06` record.** The
record is `D-06`'s dated entry in the decision record, and editing that record
is a change to this spec's territory, so it is noted here. The component table
names the new pin. The library row moved in its own, later change under spec
`002`.

**2026-09-24: the authored-content rules also read text that is not a file
(section 3.6; proposed for the owner's merge, which changes the check
suite).** Under merge commits a pull request's title and body become the merge
commit's message, and each branch commit lands on `main` with its own message,
so both are history the rules in section 3.6 govern. `scripts/check-authored-content.sh`
gains a `--text FILE...` mode that applies the same two rules, with the same
patterns and exit codes, to the files it is given; the tree mode is unchanged.
CI uses it on the pull request's title and body and on every commit message in
the change (`.github/workflows/govern.yml`, the owner's request of 2026-09-24).
Nothing required changes: the rules are the ones section 3.6 already states,
applied to text they already govern.

**2026-09-25: the session-link patterns cover every historical form
(section 3.6.2; a correction, which changes the check suite and so waits for
the owner's exception).** `scripts/check-authored-content.sh` matched a Claude
session link only in its UUID form, `claude.ai/(chat|code)/` followed by eight
hexadecimal characters or hyphens, so the `claude.ai/code/session_...` form
that cloud sessions append to pull-request bodies passed the title and body
check, and a Codex cloud task link (`chatgpt.com/codex/tasks/task_...`) passed
everywhere. Found by the travel-memory program while five repositories adopted
setup profile revision 7. Two patterns are added, `claude\.ai/(chat|code)/session_`
and `chatgpt\.com/codex/tasks/task_`, each followed by at least eight
identifier characters, so a bare product page (`claude.ai/code`,
`chatgpt.com/codex`) still passes. The script gains `--self-test`, which runs
its own `--text` mode over built samples: each link form and trailer refused,
each near-miss clean. Run against the previous patterns it fails on exactly the
two `session_` samples and the Codex sample. Nothing required changes: section
3.6.2 already refuses every agent-session URL, and this makes the check do what
the rule says. The tree mode found no such link in this repository.

**2026-09-25: rustev is recorded as an optional neighbour (owner,
2026-09-25).** The owner asked for rustev in the boundary tables of sections
3.2 and 3.8, as a spec change for the owner's ratification: an embeddable
decision engine, an optional neighbour, no dependency either way, whose
evidence records overlap with this product's run evidence and with
`attest-ledger`. The two rows above are that change. Measured at rustev
(`https://github.com/statecrafting/rustev`, public) `origin/main` `ed5ff48`
(`ed5ff4878cc6692799f6b31038e4b21e78d18464`): its README names this product among the optional
neighbours it is usable without, and no manifest in either repository depends
on the other. The overlap is the subject of the shared-primitives inventory
kept with the 2026-09-25 session evidence, which proposes one canonical JSON,
one digest format and one ledger format across the family.

**2026-09-24, adopted 2026-09-25: decision identifiers that cannot be
read as a spec-spine diagnostic (owner Addendum 2, item N).** Proposed
2026-09-24; **adopted by the owner on 2026-09-25** as written below. Nothing is
renamed by this entry: the scheme binds new identifiers and new labels from
adoption, and the existing references are renamed by rename-only pull
requests that follow the relocation-only pull requests the amendment-model
entry below sequences, which wait for spec-spine 0.27.0.

*The collision.* The decision record's identifiers are a letter, a dash and two
digits: `C-01` to `C-18`, `D-01` to `D-10`, `I-01` to `I-10`, `F-01` onward,
`G-04`. spec-spine's diagnostics are a letter, a dash and three digits:
`C-001` (coupling drift), `I-004`, `L-001`, `V-014`, `W-001`. The two share a
shape and, for `C` and `I`, a letter, so `C-01` and `C-001` read as the same
kind of thing and differ by one character. Both families appear in the same
documents: `docs/decisions/00-founding-decisions.md`, `AGENTS.md` and spec
`002` cite `C-001` beside `C-01`.

*Measured at main `17dbdb6`* (ripgrep over `specs`, `standards`, `docs`, the
root documents, `crates`, `Makefile`, `scripts` and `.github`, excluding
`target/` and JSON): `C-nn` 58 references, `D-nn` 116, `I-nn` 15, `F-nn` 82,
`G-nn` 10; spec-spine forms `C-nnn` 15, `V-nnn` 8, `L-nnn` 3, `W-nnn` 3,
`I-nnn` 1. The per-file map is kept with the session evidence.

*Proposed scheme.* A permanent decision identifier is `FD-<class><nn>`, the
record's initials, a dash, then the class letter and two digits with no dash
between them: `FD-D06`, `FD-C01`, `FD-I04`, `FD-F02`, `FD-G04`. The pattern
`FD-[A-Z][0-9]{2}` cannot match spec-spine's `[A-Z]-[0-9]{3}`, and a reader
who sees `FD-` knows which record to open. A later record (the adoption ledger
of section 3.13, if it ever needs identifiers) takes its own initials.

*Proposal-local labels stay out of permanent documents.* Labels coined inside
a proposal or a decision sheet (`D1`, `D2`, `H-n`, `Q-n`, `S-n`, `R4d`,
`P-1`, `F13`, `A1` to `A12`, `L1` to `L5`) are scaffolding for one
conversation. Measured: `H-n` 122 references (91 in spec `002`, 2 in `003`,
one comment in each of the four delivered hooks and `harness_hooks.rs`),
`Q-n` 19 (spec `002` and one hook comment), `S-n` 31 (spec `002`,
`setup.rs`, the setup tests and the profile templates), `A-n` 39, `F1n` 10,
`L1` to `L5` 7, `D1`/`D2` 2, `P-1` 2, `R4d` 1, all in spec `002` except as
listed. Rule proposed: when a proposal is adopted, its entry names each choice
by what it decided (for example "one producer identity" rather than "H-3 (a)")
and may cite the label once, in parentheses, as provenance; code comments and
delivered files cite the spec section, never a label. A bundle entry's own
internal part numbering (`P1.1`, `P6.3`) is section structure, not a label,
and stays.

*How the map would be applied.* Not by a sweep in this entry. Renaming 281
decision-record references and roughly 230 label references is a
mechanical change like the relocations the owner's spec-structure item (S)
already sequences, but it cannot ride inside them: Part 3 below admits no edit
inside a moved section, and spec-spine 0.27.0's relocation proof (its spec 142)
ignores only heading numbers and levels, trailing whitespace and blank-line
runs, so a renamed identifier inside a moved section would fail it. The renames
therefore follow the relocations, in rename-only PRs, each demonstrating that
the only change to requirement text is the identifier. (Corrected 2026-09-25
on an AI-review finding: the entry as first written said the renames ride
with the relocation PRs.)

**2026-09-24, direction adopted 2026-09-25: what 1.0 means: a readiness
checklist, a stability policy, and a release pipeline (owner Addendum 2, items
RD and Q).** **The owner adopted the stability policy and the readiness
checklist as the direction on 2026-09-25.** The release pipeline stays
proposed: it waited on the attest-ledger item, and that blocker is removed by
the change that takes `attest-ledger-core` from crates.io at `=0.1.0` (spec
`003` section 5, 2026-09-25, which records the diff), so the pipeline is the next proposal to bring
back. The checklist below cites two spec `006` section 5 entries adopted
2026-09-25: the parser entry (clap for per-verb arguments) and the JSON naming
and strict-input entry. Nothing here lifts `F-02`:
publication stays deferred until the owner
lifts it separately, and no grade above *specified* is claimed for anything
below.

*Stability policy (adopted as the direction, 2026-09-25).* Before 1.0, any surface may change with a
dated spec amendment. From 1.0, these are **stable**, and changing one
incompatibly needs a major version and a migration note:

1. **The command tree**: every verb in `Verb::all()` and its arguments
   (spec `006` sections 3.1 and 3.11). Adding a verb or an optional flag is
   compatible; removing or renaming one is not.
2. **The exit and JSON contract**: the five exit codes of spec `006` section
   3.3, and the family envelope: the family exit and JSON contract that
   draft #118 proposes for spec `006`, adopted by the owner on
   2026-09-25 and landing with the spec-spine 0.26.0 migration. Adding a
   field is compatible;
   removing, retyping or renaming one is not (`006` section 3.4, "Two
   renderings of one value", which already calls an added field compatible).
3. **Documented formats**: the environment manifest, the run record and
   journal, receipts and evidence, and bundle metadata, each with a
   `schemaVersion` and a reader for every version it ever wrote.
4. **Outcome words** (`partial`, `refused`, `withheld`, `drifted`, ...), as
   the shared glossary defines them.

Human output, log text and anything under `.statecraft/state/` are not
stable.

*Readiness checklist (adopted as the direction, 2026-09-25), with the evidence each item needs.*

| Item | Done when | Evidence |
|---|---|---|
| Stable commands | every verb has help, argument docs and a binary test per usage error | `--help` output per verb; tests that a bad flag exits 3 (the spec `006` parser entry) |
| Exit and JSON contract | the family envelope adopted; every verb's `--json` parses as it | a test over every verb; `exitCode` equals the process status |
| Documented formats | each format has a schema document and a `schemaVersion`; strict within a version (the spec `006` JSON naming entry) | schema files; a test reading every version's fixture |
| Conformance tests | producer conformance, adapter conformance and the acceptance suites run in CI against the adopted pins | CI run ids; `make verify` for each spec |
| Acceptance stability | the 002/003 intermittent failures diagnosed and fixed, never retried into green | the acceptance diagnostic recorded in spec `002` section 5 (the fresh-executable first-exec stall), with before and after timings |
| Security posture | `SECURITY.md` with private reporting enabled; spec `004`'s credential-fence residuals stated; a threat model per trust boundary | the files; the repository setting read back |
| Supply chain | `cargo-deny` (advisories, licenses, bans, sources) and a declared-MSRV build in CI; pinned actions | CI job results |
| Release pipeline | the pipeline below, exercised once on a pre-release tag | the release run id; a fresh-consumer verification record |
| Claims | README and specs state each behavior's grade with evidence (section 3.3) | the grade table, re-measured at the tag |

*Release pipeline (proposed), to spec-spine's standard.* spec-spine's
`release.yml` (its specs 019 and 119) is the reference: a tag-gated build of a
per-target archive with a `.sha256` sidecar, a per-target CycloneDX SBOM that
fails closed when empty, a SLSA build-provenance attestation per archive, a
GitHub Release carrying those assets, and idempotent registry publication;
its `determinism.yml` proves the build byte-identical. For this product:

1. **Signed tags**: annotated tags signed with the owner's key, as spec-spine's
   `v0.25.0` is (ED25519); the pipeline refuses an unsigned or unverified tag.
2. **Archives and checksums** per supported target, with `.sha256` sidecars.
3. **SBOM** per archive, CycloneDX JSON, failing closed when it lists no
   components.
4. **Build attestations** for each archive, verifiable with
   `gh attestation verify`.
5. **Fresh-consumer verification**: after publication, a job on a clean
   runner downloads the published assets (not the build tree), verifies the
   signature, checksum and attestation, installs, and runs a smoke suite
   (`--help`, `doctor` on a scratch repository, `init plan`), recording the
   result against the tag, as spec-spine's 119 judges what was built.

*The crates.io blocker, stated.* `crates/statecraft-run` depends on
`attest-ledger-core` by git revision (`a9c3595`). crates.io refuses a crate
with a git dependency, so this product cannot be published there until
`attest-ledger-core` is published to a registry or vendored under its
license. Archive and GitHub Release distribution is not blocked by it.
Workspace crates stay `publish = false` until `F-02` is lifted.

**2026-09-24, Part 1 adopted 2026-09-25: the corpus moves to spec-spine's
amendment model, and `002` is split along its seams by relocation only (owner
item S).** **The owner adopted Part 1, the amendment model, on 2026-09-25**:
from that date a new behavioral amendment to an approved spec is a new spec
with an `amends` edge (a `draft` is still amended in place)
(and `extends` on each unit whose code it changes), and section 5 keeps real
implementation decisions only; a section 5 entry stays the route that clears
`C-001` for a change that alters no requirement, as it was before. **Part 2, the split of `002`, is held** until
spec-spine 0.27.0 provides an exclusive claim transfer and a provable
relocation (friction items 1 and 3 below, sent to spec-spine as its item C
findings); then 0.27.0 is adopted under `D-06` and the split is done through
relocation-only pull requests. Creating `007`, the distribution seam, is not
held with Part 2: it moves no section out of `002` (step 5 below drafts it from
the bundle entry), so it follows the spec-spine 0.26.0 migration with
`planned: true` claims, as the owner decided the same day. The open decisions in the table at the end (code
ownership during the split, section numbers on relocation, a fifth seam, the
relocation proof, and who merges fold PRs) stay open until then; their
labels there are the proposal's own and are not cited elsewhere. The text below is the proposal as prepared.
Prepared at the owner's request of 2026-09-24 for the owner's ratification
decisions; nothing below binds until the owner adopts it, and each decision is
in the table at the end. It is recorded here because spec `001` owns the
component boundaries and `D-02`, which the split touches, and because a
proposal filed beside the corpus would be a second place for a requirement to
live (`AGENTS.md`, Source ownership). Measurements are in the evidence
repository under `2026-09-24/session10/S/`, taken in disposable clones of `main`
at `17dbdb6` with the pinned spec-spine (the release `spec-spine.toml` names);
nothing was pushed from them.

*Part 1: the amendment model.* From adoption on, a change to what a spec
requires is a **new spec** with an `amends` edge to the spec it changes, not a
dated section 5 entry and not an edit to the amended spec's section 3. This is
spec-spine's rule (its specs 037, 082 and 083): the amended `spec.md` is not
edited to record the amendment, and `registry relationships <id>` reports
`amended_by (incoming)`. Section 5 of every spec keeps only genuine
implementation decisions: a choice the spec was silent on, with its reason.
Two measured consequences shape the rule:

1. **`amends` does not make the amending spec an owner of the amended spec's
   code.** The gate widens ownership through `amends` only for the amended
   `spec.md` itself. Measured (S3): a new spec `amends: [002]`, then a commit
   changing `crates/statecraft-home/src/flow.rs` with an authoring edit to the
   amending spec only: `couple` exit 1, `C-001` naming `002` alone. With an
   added `extends: { spec: 002, unit: crates/statecraft-home/, nature:
   corrective }`, the same commit couples (exit 0). So an amending spec that
   changes behavior in code **declares `extends` on each unit it changes**, in
   the same change that introduces it. That amends nobody's text and needs no
   waiver.
2. **An amendment that replaces acceptance says so** with
   `amends_verification` (spec-spine 082), so `verify <amended>` runs the
   replacement and prints the substitution.

*Part 2: the split of `002`, relocation only.* Four seams, as the owner named
them. `002` keeps initialization and lifecycle; two new specs take the harness
and the producer seams; distribution goes to `007` when `007` is created.

| Destination | Section 3 of `002` today | Section 5 entries of `002` today (by date and first words) |
|---|---|---|
| `002` initialization and lifecycle (stays) | 3.1 to 3.8, 3.10, 3.11, 3.12, 3.13, 3.16 to 3.21, 3.35, 3.36 | 2026-09-16 (all four); 2026-09-20 native root, relocation rewrites, manifest v2, two references left, ratified; 2026-09-21 dependency on 006, derived tree's three states; 2026-09-23 project block commands, 3.35 implemented, replacing a drifted file, env remove takes back the bridge, foreign finding, 3.16 frozen resolution; 2026-09-24 what initialization reports, outcomes implemented, withheld means partial (authority and implementation), setup profile (authority and implementation), provenance (authority and implementation) |
| `008` harness, hooks and skills (new) | 3.9, 3.14, 3.22 to 3.34, 3.37 | 2026-09-20 hooks ship in the harness, delivery verdict; 2026-09-21 every entry from the handoff fold through the deadline-attempt synchronisation except the three producer entries in the `009` row and the derived-tree coverage entry in the `002` row; 2026-09-22 all; 2026-09-23 the two live experiments, non-turn event, trial deadline, trailing-event decision, 3.34 implemented, permission experiment second campaign, launch-state answer, two reads for reconciliation, 3.34 trailer, 3.37 implemented, translation of `check`, gate log planted decision; 2026-09-24 contract 2 (authority and implementation), contract 4 (authority and implementation) |
| `009` producer adoption (new) | 3.15 | 2026-09-20 exact crates.io pin, trimmed producer tested, governance files written by this crate, contract path adopted; 2026-09-21 published 0.21.0 versus source, version literal, `.crate` digest; 2026-09-23 core 0.23.0; 2026-09-24 core 0.25.0, exact pin in new projects |
| `007` distribution (created after spec-spine 0.26.0) | none | 2026-09-23, adopted 2026-09-24: the release bundle contract (Part 9 step 1) |

Three rows are judgement calls and are named as such: 3.9 (adapters declare
what they own, and the code is `crates/statecraft-environment/src/adapter.rs`)
goes with the harness because an adapter is how a harness is delivered; 3.29
to 3.34 and 3.37 (admission, launch and startup records) could be a fifth seam,
"managed-session delivery and admission", and are kept with the harness only
because the owner named four; 3.36, the completion rule, cannot move whole
because it describes all of `002`, so it stays and each new spec gets its own
completion rule as an ordinary amendment after the split (Part 1), which is not
a relocation.

**Code ownership under `D-02`.** The code does not split along these seams:
`crates/statecraft-home` holds `flow.rs` (initialization), `harness.rs` and
`harness/` (the harness), `producer.rs` (the producer) and `launch.rs`
(startup). `D-02` as amended says a crate has exactly one owning spec. Three
ways to hold that:

- (a) **New specs own no code.** `008` and `009` carry requirements and declare
  `extends` on `002`'s crates; `002` keeps both crates. `D-02` holds unchanged,
  and a harness change couples by editing `002` or `008` (measured, S1b: any
  owner's `spec.md` clears the path).
- (b) **Sub-crate units.** `002` replaces its directory claims with file and
  directory units, and `008` and `009` establish theirs. The tool accepts
  overlapping claims silently (measured, S1: `002` owning
  `crates/statecraft-home/` and a new spec establishing
  `crates/statecraft-home/harness/` gives `lint`, `check --fail-on-unresolved`,
  `index coverage --fail-on-untraced` and `index check --fail-on-unresolved`
  all exit 0, and `index owner` lists both), so exclusivity would rest on
  authoring discipline. It amends `D-02`.
- (c) **Split the crates** (a harness crate, a producer crate). Code moves, so
  it is not relocation-only and is its own implementation change later.

*Part 3: how a relocation PR proves it changed no requirement.* Each
relocation PR moves whole sections, keeps each heading's text (the number may
change), and edits nothing inside a moved section. Its body carries the output
of a relocation proof run over its own base and head: for every `spec.md`,
split the body at every heading, key each section by its heading text with the
number removed, hash its body, and require every base section to appear at
head with the same digest (in any spec). New sections are allowed only as
scaffolding (a new spec's purpose, territory, out-of-scope and section 5
headers, and a one-line pointer where a section left). The script is in the
evidence (`relocation-proof.py`); measured on a relocation of 3.1 to 3.6 into
a new spec it reports 7 sections moved unchanged, 0 changed, exit 0, and after
one inserted word in a moved section it reports that section, exit 1. The
registry's own `sectionDigests` cannot serve: each digest is salted with the
spec's path (`<spec_path>#<anchor>`), so a verbatim move changes every digest
(measured: all six moved sections differ). If the owner adopts the proof, the
script becomes a claimed file under `scripts/` in the first relocation PR.

Each relocation PR also: moves the ownership edges that name a moved unit
(measured, S2: `003` and `004` declare `extends` naming `002` for
`crates/statecraft-environment/`; if that unit moves, the edges keep resolving
by path and nothing reports that they name a spec which no longer claims it,
so the PR retargets them); updates citations. There are 327 citations of the
form "`002` section 3.x" outside `002` (code comments in six crates and one
line in each of `003` to `006`) and 447 section references inside `002`.
Keeping each moved section's number in its new spec (so 3.14 stays 3.14 in
`008`) would leave every in-spec reference true and turn each outside citation
into a mechanical "`002` to `008`" rewrite that the proof script cannot see but
a citation map can; renumbering would make every one a semantic edit.

*Order.* Each step is its own PR; the relocations fall under the owner
delegation (#111) once this split is adopted.

1. This proposal, adopted or amended by the owner (one authority PR changing
   this entry to adopted, the `D-02` choice, and `AGENTS.md` for the
   amendment model). As adopted on 2026-09-25, that PR adopts Part 1 and
   `AGENTS.md` only; the `D-02` choice (S-B in the table below) stays open
   with Part 2, as the adoption note above says. (Noted 2026-09-25 on an
   AI-review finding.)
2. The 0.26.0 migration (adoption, the exit and JSON contract amendment of
   spec `006`, the one-identity rule) lands before any relocation, so the
   relocations do not race the code it changes.
3. (Held until spec-spine 0.27.0 is adopted, per the adoption note above.)
   Relocation R1: `009` producer adoption (3.15 and its entries), the smallest
   seam, to prove the method.
4. Relocation R2: `008` harness, hooks and skills.
5. `007` is drafted from the bundle entry by relocation (the distribution
   seam), after 0.26.0's planned claims are adopted; the owner ratifies it;
   then it is built.
6. Fold, per spec: each adopted section 5 entry that states a requirement is
   folded into section 3 of the spec that now holds it, one spec per PR,
   `002` last because it is largest. A fold is not a relocation (it rewrites
   requirement text into its final form), so each fold PR is an authority
   change for the owner, and it keeps the folded entry's date and decision
   reference beside the rule. After the fold, section 5 holds implementation
   decisions only, and Part 1 governs every later change.

*Friction measured, for spec-spine.* (1) No exclusive claim transfer: partial
`supersedes` is additive by design (its spec 018 section 4 defers the
owner-stripping operation), and a plain second `establishes` overlapping an
existing directory claim raises no lint, so "exactly one owner" is not
checkable. (2) An `extends` edge naming a spec that no longer claims the unit
resolves silently. (3) `sectionDigests` are path-salted, so they cannot prove
a verbatim move. (4) `amends` never widens code ownership; the gate's
remediation text says so for `extends`, but nothing tells an amending spec it
also needs one. (5) `couple` resolves ownership from the checked-out tree's
committed index, not from `--head`: judging the same base and head gave exit 1
with the head checked out and exit 0 while a later commit that adds an
`extends` edge was checked out. CI checks out the pull request, so CI is
consistent, but a local reproduction depends on the checkout. (6) `compact`
merges specs and rewrites their citations; there is no inverse for a split.
(7) A new approved spec claiming a crate that does not exist yet still exits
1 under `index check --fail-on-unresolved` (`W-001`), which is what 0.26.0's
planned claims are to change for `007`. None of these blocks the split; (1),
(2) and (5) are recorded as authoring discipline the relocation PRs carry.

| Item | Options | Recommended default | Consequence of the default |
|---|---|---|---|
| S-A: amendment model | **decided 2026-09-25: adopted as Part 1** | (decided) | Every later behavioral change is a new spec; section 5 stops growing with requirements |
| S-B: code ownership during the split | (a) new specs own no code / (b) sub-crate units, amending `D-02` / (c) split crates later | (a) | `D-02` unchanged; harness and producer code still couple through `002` or the seam spec |
| S-C: section numbers on relocation | keep numbers / renumber | keep numbers | Outside citations change only their spec number; no in-spec reference breaks |
| S-D: fifth seam for admission and launch (3.29 to 3.34, 3.37) | with `008` / own spec | with `008` | `008` is large; a later split of it is another relocation |
| S-E: the relocation proof | adopt the script as a claimed file / proof in PR bodies only | claimed file under `scripts/` | Each relocation PR carries a mechanical proof CI can rerun |
| S-F: fold PRs | owner merges each / delegated | owner merges each | The fold rewrites requirement text, so it stays the owner's act |
