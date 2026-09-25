@.statecraft/AGENTS.md

# AGENTS.md

The cross-agent authority for this repository, read by Claude Code, Codex CLI and
any other agent through the `AGENTS.md` convention. Edit this file to evolve the
protocol.

This repository holds seven specs, eight crates and one binary. **A crate has
exactly one owning spec, and a spec may own more than one crate**: that is `D-02`
as amended on 2026-09-21, when four pairs of specs were consolidated and no crate
was merged. Do not add code, a crate or a test runner without a spec that claims
it, and the coverage gate refuses an unclaimed source file.

## Where authority lives

1. `specs/000-bootstrap/spec.md`: what a spec is. Its `unamendable` anchors
   cannot be contradicted.
2. `standards/spec/constitution.md`: the durable principles. I to V are
   spec-spine's; VI to XIII are this product's and were **ratified on
   2026-09-16**, three of them frozen as spec `000` anchors.
3. `standards/spec/contract.md`: the normative summary, including the lifecycle
   table that decides what is schedulable.
4. `specs/NNN-slug/spec.md`: ordinary specs.
5. `docs/decisions/00-founding-decisions.md`: what is intent, what is inherited,
   what is proposed. A row of its section 3 binds only once its section 5
   records the adoption; its opening paragraph says which have been.

`.statecraft/derived/` is compiler output. Never hand-edit it, and never parse
it with `jq`, `sed` or `awk`: read it through `spec-spine` subcommands, which
fail at the deserializer instead of silently encoding a stale assumption. It
moved there from `.derived/` with spec `002` section 3.19; the whole of
`.statecraft/` is committed except `.statecraft/state/`.

## New sessions

Read before working. None of these writes.

```sh
make tools                                # install the pinned spec-spine locally
.tooling/bin/spec-spine --version         # must satisfy required_version in spec-spine.toml
make gate                                 # the whole read-only corpus surface
make status                               # version, lifecycle counts, schedulable set
git log --oneline -10
```

Run `spec-spine` through `make`, or as `.tooling/bin/spec-spine`. A bare
`spec-spine` resolves to the shared `~/.cargo/bin` copy, which any project on
this machine replaces, and this repository has been governed by the wrong
version that way more than once. `make` prefers the local binary automatically.

**From release 0.26.0, spec-spine exits in the family contract** (its spec
132; the measurements are in `docs/adoption/spec-spine.md`): 0 ok, 1 a
finding, 2 refused, 3 usage, 4 failed. **A stale tree is exit 1**, beside a
corpus that does not validate and a claim that cannot be resolved; exit 2 is a
refusal to judge at all, a pin not met among them. Under every release from
0.20.0 to 0.25.0, exit 2 meant stale and only stale. An unresolved claim has exited
1 under every pin since 0.20.0: it is a corpus that does not describe its tree
rather than a ledger that has fallen behind, so re-indexing cannot cure it,
and `refresh` against it produces shards that say the same thing.

Read the message, because the code no longer says which. `check` names each
half: `STALE` sends you to `make refresh`; `INVALID`, `UNRESOLVED CLAIM` or
`fresh, but REFUSED` sends you to the spec. Neither says which spec or which
shard, so read the lines that follow.

A genuinely stale tree is reported and then fixed as committed work. Do **not**
substitute a writing `compile` or `index` for a check: a read that repairs the
tree hides the fact that the *committed* copy was stale, so the drift then reads
as a local edit rather than as a defect on the branch.

**`registry plan` offers a `draft` spec as ready, and that is not permission to
build it.** Verified against 0.18.0 on 2026-09-16, and re-verified against
0.20.0 on 2026-09-17 by forcing one spec to `draft` plus `pending` in a scratch
worktree: the lifecycle table makes that combination schedulable, so `plan`
names a spec the owner has not agreed to exactly as it names one the owner has.
Since 0.23.0 each `plan --json` ready row also carries `status` (spec-spine's
102, which keeps readiness as scheduling, not approval), and spec `003` section
3.1.2 compares it with `registry list --json`, which is still the only report
carrying `implementation`, so the join stays. What `draft` withholds is
*ratification*, which is why an unratified spec's unresolved units warn instead
of refusing.

All seven specs, `000` to `006`, are ratified, so what `plan` offers is a real
work order. Measured 2026-09-23 with `make status`: 7 specs, 1 ready, 0 blocked.
The ready one is `002`, `approved` with `implementation: in-progress`, because
its live permission experiment has no admitted result, which no local
implementation work can supply (spec `002` section 5, 2026-09-23). The next `draft` written here will be offered as ready
anyway; check the `status` field, not the plan output.

spec-spine does not enforce the difference, so this repository does. Until the
owner ratifies a spec (see Approval semantics), `plan` naming it is a reading
suggestion, not a work order. Do not open an implementation branch for a
`draft` without the owner saying so in the request at hand.

## The gate

Two surfaces, not one. `make gate` judges the **corpus** and is meaningful with
no code at all. `make code` judges the **workspace**, which is eight crates
today. CI runs them as separate jobs and requires both through `ci-gate`, the
single status context branch protection names. Both targets run
`scripts/statecraft/gate.sh`, the same script CI runs, so the local and the CI
definition cannot drift (see "Continuous integration" below).

```sh
make gate
spec-spine check --fail-on-warn
spec-spine lint --fail-on-warn
spec-spine index coverage --fail-on-untraced
spec-spine index check --fail-on-unresolved
scripts/check-authored-content.sh

make code
cargo build  --workspace --locked
cargo test   --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt    --all --check
```

CI caches `~/.cargo/registry`, `~/.cargo/git` and `target/` for the `code` job,
keyed on `rust-toolchain.toml` plus `Cargo.lock`: those two are exactly what
invalidates a build. A dependency or toolchain bump misses the cache and is
meant to; an ordinary source edit hits it and recompiles only this workspace's
crates. Nothing in the governance job is cached beyond the spec-spine binary,
because `check` must judge the committed tree and not a restored one.

Each `make code` target is guarded on `crates/*/Cargo.toml` existing, because
every `cargo --workspace` verb refuses a virtual manifest with no members. The
guard is a file test rather than a flag, so the job went live with the first
crate and nobody had to remember to enable it. It no longer fires, and it stays:
the reason it was written, deciding the check surface before the work it judges,
is what Approval semantics requires.

**Both flags the generic kit carries are now in the gate**, and each arrived on
the condition recorded here when it was left out, not on a whim:

- `index coverage --fail-on-untraced` joined with the first source file. On a
  code-free tree it refused an empty universe rather than passing vacuously.
- `index check --fail-on-unresolved` joined when `006` built the last forward
  claim. Until then it would have refused specs that were correctly claiming
  crates they had not written yet.

The second has a cost worth knowing before you pay it: **a new spec that claims
a crate it has not written yet now fails the gate.** That is intended. This
corpus builds what it claims within one pull request, so an unresolved unit
reaching the default branch is a claim nobody wrote. A spec that genuinely needs
to claim ahead of its implementation is the case for removing the flag again,
deliberately, as its own change.

## What a green gate means

The invariant: **the exact integration candidate must pass every required
governance check before it lands.** Two properties are needed for that, and they
are independent. Conflating them is easy and was done here once already.

**Freshness**: the checks ran against a candidate reconciled with the current
`main`. Enforced by branch protection, which requires a branch to be up to date
before merging. How a branch becomes up to date is not prescribed: rebase,
merge, or the forge's own update button all satisfy it, and the one to prefer is
whichever keeps history legible for the change at hand.

**Comparison scope**: the check evaluated *this* candidate's changes and no
others. Enforced by which two commits the check is given, not by freshness.
Re-running a stale check against a moved base makes it fresh and still wrong.

The coupling gate is where scope bites. `spec-spine couple` takes a **three-dot**
diff, so its merge base is derived from the two endpoints it is handed:

- `base.sha...head.sha`, both frozen event SHAs, has this pull request's own fork
  point as its merge base. It evaluates exactly this pull request's changes and
  is stable across re-runs. **This is what CI uses.**
- `base.sha...HEAD`, where `HEAD` is the checked-out `refs/pull/N/merge`, has
  `base.sha` as its merge base and folds in everything merged after the event
  fired. Measured here: 15 changed paths where the pull request changed 1.

The second form is not merely noisy. A waiver is scoped to the diff the gate
evaluated, so a `Spec-Drift-Waiver:` in the body would have covered all 15.

## Continuous integration

This repository's CI is **rendered from Statecraft's own setup profile**,
`github-actions-rust` revision 8 (S-5, owner decision of 2026-09-24): the
product governs itself with what it gives adopters. The rendered files are
managed, and their ownership is recorded in `.statecraft/environment.json`:
`.github/workflows/statecraft-ci.yml`, `.github/workflows/statecraft-ai-review.yml`,
`scripts/statecraft/*.sh` and the policy `.statecraft/setup/github-actions-rust.json`.
**Do not edit them by hand**: change a parameter in the `project.setup` block of
`.statecraft/environment.json` and re-render with `statecraft-cli init plan`
then `init apply --plan <identity>`, or change the profile itself under spec
`002`. A hand edit is drift that `doctor` reports.

The parameters this repository sets keep every check the hand-written
`govern.yml` had: coverage enforced, `scripts/check-authored-content.sh`
required and applied to titles, bodies and commit messages, every commit gated
and signed, and a base other than `main` refused. What the profile adds is the
**AI review** on every pull request. From revision 8 it uses the
`ANTHROPIC_API_KEY` secret when the repository can see one (Console credits)
and the `CLAUDE_CODE_OAUTH_TOKEN` secret otherwise (the subscription). Both are
organization secrets with selected visibility, so the owner switches billing
by changing which repositories see the key; a repository-level secret of the
same name takes precedence. The job log and the evidence record name the
class used (`api-key` or `oauth`), never the value. A `findings` verdict blocks
`ci-gate` unless the owner approves the `statecraft-review-exception`
Environment for that run. In the merge queue the review is not re-run: `ci-gate`
reads the verdict recorded for the entry's pull-request head.

**A pull request never judges itself with its own gate** (revisions 5 and 6):
`gate.sh`, `install-spec-spine.sh`, `ci-gate.sh` and the authored-content
script run as they exist on `main`. **Any change to the CI's own files (every
file under `.github/workflows/`, `scripts/statecraft/*`, the policy,
`scripts/check-authored-content.sh`) blocks `ci-gate` until the owner approves
that run's `statecraft-review-exception` Environment.** A re-render is such a
change, so plan it as a pull request that waits for the owner.

Every rendered script exits in one contract (revision 7): 0 ok, 1 finding,
2 refused, 3 usage, 4 failed. A missing spec-spine exits 2, and a stale
tree exits 1 from `gate.sh` under either spec-spine table: `gate.sh` reads
spec-spine's codes by the release `spec-spine.toml` pins.

## The merge queue

The queue is enabled on `main` (2026-09-24, merge method: merge commit, up to
five entries built, all green required). It preserves the invariant above
because the three properties it needs were made true in the same authority
change that documented them:

1. **Coupling is evaluated against the queued integration candidate.** On a
   `merge_group` event the `governance` job runs `spec-spine couple` over the
   group's `base_sha...head_sha`, which is the entry's change applied on the
   speculative base it will land on, not only the pull request in isolation.
2. **A waiver is bound to its change and to the authority that approved it.**
   A `merge_group` event carries no body, so the step reads the entry's own
   pull-request body through the API, and honours a `Spec-Drift-Waiver:` only
   when the group changes no path that pull request does not change. A group
   that folds in other changes is judged with no waiver: it fails closed
   rather than widening the scope the owner granted.
3. **`ci-gate` refuses success when a required job did not succeed**, on every
   event: failed, cancelled and **skipped** all fail it. An event-gated check
   lives inside a job as a step (both coupling steps do), never as a job that
   can be skipped into a green gate.

Two consequences worth knowing. Two pull requests that each regenerate shards
can each be fresh alone and stale together; the queue's `check` refuses the
second one with exit 2. The fix is to merge `main` into the branch, resolve any
conflict in authored text by hand, and then run
`scripts/resolve-shard-conflicts.sh`: it takes the incoming copy of each
conflicted shard, runs `make refresh`, stages the result, and exits 1 if a
conflict marker remains or if the branch's own authored change (the lines it
adds and removes per file) differs after the merge. It never resolves an
authored conflict; a mechanical "take theirs" once deleted 2,700 lines of spec
`002`.
And a waived pull request queued behind another that touches the same paths is
judged without its waiver, by construction; queue it alone.

## How a pull request is merged

Two methods are allowed; the choice is not arbitrary. Rebase merging is disabled
in the repository settings (owner, 2026-09-24): it rewrites SHAs and adds
nothing merge commits do not already give.

- **Merge commit, the default**, and always when a record cites a branch SHA
  (a waiver's head, "tested on X", an evidence commit) or the pull request is
  stacked on another. The judged heads stay in `main`'s history, and
  `git branch --merged` answers correctly.
- **Squash** only when the branch carries fixup or red intermediate commits and
  no record cites their SHAs.

Every commit that reaches `main` is signed and passes `make gate`; with merge
commits the branch's commits land too, so this binds each one, not only the
head. CI enforces it: on `pull_request` and `merge_group` the `governance` job
gates every commit in the change at its own tree (`gate.sh governance` and
`cargo fmt --check`, with the pin that commit names), refuses one GitHub does not verify as signed,
and applies the authored-content rules to each commit message and to the pull
request's title and body. A branch with a red or unsigned commit is therefore
rebuilt before review, not squashed at merge, so squash is left for branches
nobody cites. **A pull request whose base is not `main` fails `governance`**: stack
by opening each branch off `main` with a depends-on note, or wait for GitHub to
retarget the upper one when the lower one merges. The merge commit's message is
the pull-request title and body, so the authored-content rules bind the body as
history. Read history first-parent (`git log --first-parent`, `git bisect
--first-parent`): the first-parent chain is the sequence of integration
candidates the gate judged.

`spec-spine couple` is **CI-only, and deliberately not in `make gate`**. It
compares two commits, so it cannot see a change being staged and is useless as a
pre-commit check (`C-18`). CI runs it against the pull request's merge base. It
became meaningful with the first crate and judges every `crates/**` change
today. `make couple` exists for reproducing a CI verdict locally, against a
commit, not for gating a commit you are about to make.

## Working the backlog

One spec per pull request, then stop.

1. **Pick the spec.** `spec-spine registry plan --next` names a candidate.
   Confirm it is `approved` before working it: a `draft` is offered as ready and is
   not ratified (see New sessions). If nothing is ratified, ask; never ratify.
2. **Branch.** A feature branch named after the spec id. Never commit to `main`.
3. **Re-read the design before coding.** If the design is imprecise, record the
   choice in the spec. If it is wrong, stop and report. Never rewrite an approved
   spec to match code you just wrote.
4. **Implement within the territory.** Claim every new file in the spec's
   ownership edges. Touching a unit another spec owns is an `extends` edge naming
   that spec and unit.
5. **Refresh and gate.** `make refresh` after editing any `spec.md`, and commit
   the regenerated shards with the change that made them stale. `make gate` before
   every commit. The codebase index hashes the authored tree, not only the specs:
   editing a root document such as this one or `README.md` turns `check` stale
   with no structural change, and the fix is the same refresh in the same commit.
   The shards do change, and visibly: each one's `shardHash` moves, because that
   hash covers the global inputs. What does not change is what the shard says
   about the corpus.
   **`make couple` before committing proves nothing.** The gate compares two
   commits, so with `HEAD` still at `BASE` it judges an empty diff and reports
   "0 path(s) checked, no drift", which reads exactly like a pass. Commit, then
   couple. The same reasoning applies after a merge: a verdict on merged `main`
   cannot establish that the pull request was correctly coupled, because the
   endpoints it judged are gone. The verdict that counts is the one CI recorded
   against that pull request's own frozen endpoints.
   **A commit follows a check only on that check's own exit status.** Chain it
   with `&&` (`make gate && make code && git commit ...`), or run the check as
   its own step and read its status before the next one. Never pipe a check
   into `tail`, `head` or `grep` before a dependent step: a pipeline reports
   the status of its last command, so `make gate | tail` exits 0 when the gate
   failed. Never follow a check with `;`, and never leave a required check in
   the background while committing. The targets themselves propagate failure:
   measured on 2026-09-22, an em dash staged in `README.md` made
   `make gate && git commit` exit 2 with no commit, a misformatted source
   made `make fmt && git commit` exit 2 with no commit, and
   `make gate 2>&1 | tail -1` reported 0 for the same failing gate. Earlier
   series on this repository committed after checks that had failed; the
   published commits stay as they are, and each later commit names its fix.
6. **Verify.** `make verify SPEC=<id>` runs the spec's declared acceptance. A spec
   with no `## Verification` block declares none, which is honest for an
   unimplemented spec and is not a passing acceptance.

## Source ownership

| Path | Owner | Rule |
|---|---|---|
| `specs/**/spec.md` | authored | The source of truth. One directory per spec, name equal to the frontmatter `id`. |
| `standards/spec/**` | authored | Constitution, contract, templates. The constitution is changed by an `approved` spec claiming the affected heading as a section unit. |
| `docs/decisions/**` | authored, claimed by spec `001` | Editing it is a change to spec `001`'s territory. **There is no `docs/design/`, and a design or handoff note does not get one.** It is folded into the spec it informs: the founding record is spec `001` sections 3.8 to 3.12, and the 2026-09 spec-spine harness handoff is spec `002` sections 3.22 and 3.23. A note filed beside the corpus is a second place for a requirement to live, and the first place people stop reading. |
| `scripts/**` | authored, claimed by the spec that adds it | `check-authored-content.sh` is spec `001`'s. |
| `.statecraft/derived/**` | **compiler** | Regenerated by `make refresh` only. Committed. Never hand-edited. `build-meta.json` is the one gitignored file. Moved from `.derived/` by spec `002` section 3.19. |
| `.statecraft/state/**` | runtime | Declared as `state_dir`: ungoverned, gitignored, and never claimed by a spec. |
| `spec-spine.toml` | authored | The layout, and the **single stated source of the CLI pin** (`required_version`); no other document restates the number. Changing the pin is its own change with its own re-index and its own ledger entry (`D-06`). |
| `docs/adoption/spec-spine.md` | authored, claimed by no spec, governed by `D-06` | The adoption ledger: one entry per adopted spec-spine release, recorded as one producer identity: the CLI and the linked core are the same release (H-3 (a)). `D-06` in the decision record stays the stable decision. An adoption that changes no behavior edits only this, `spec-spine.toml`, the root `Cargo.toml`, `Cargo.lock` and the shards, so it couples without a spec edit or a waiver (spec `001` section 3.13). |
| `Cargo.toml`, `rust-toolchain.toml`, `Makefile`, `.github/workflows/**` | authored, claimed by no spec | The check surface and the workspace root it needs; the root `Cargo.toml` is also the single stated source of the linked `spec-spine-core` version (`[workspace.dependencies]`, `D-06`). Deliberately unclaimed: they build and judge the whole corpus rather than any one spec's territory, and a spec that owned them would be a spec that owns the rules it is judged by (constitution VII). Changing any of them is an **authority change**, decided by a human on its own. |
| `crates/**` | authored, claimed by exactly one spec | `D-02` as amended: one owning spec per crate, and a spec may own more than one. A crate's files are claimed by that spec's directory unit, which is how the coupling gate holds a real compile unit per claim. Three specs own two crates each: `002`, `004` and `005`, and each one's section 2 says what separation the two crates hold. |
| `LICENSE` | preserved | Never edited. Apache-2.0. |

## Approval semantics

- **An agent never ratifies a spec.** Flipping `status: draft` to `approved` is
  the repository owner's act. An agent may draft, amend a draft, and report that a
  spec is ready to read.
- **An agent never writes a `Spec-Drift-Waiver:` line on its own authority.** It
  needs explicit human approval, and it is cited in the pull-request body.
- **An agent never adopts a row of `docs/decisions/00-founding-decisions.md`.** It
  may add a row marked proposed, with its reason.
- **An authority change is separated from implementation.** A change touching
  policy, the check suite, the verifier, hooks, the acceptance instructions or
  the environment manifest is proposed on its own and decided by a human. It is
  never bundled with the work it would authorize.
- **Publication is not authorized by default.** No push to `main`, pull request,
  merge, release or deploy happens without the owner asking for it in the request
  at hand. Deferral `F-02` in the decision record holds until separately lifted.
  Publication here means an act outside this repository's own review flow;
  what an agent may do within an owner's standing authorization is in "Owner
  delegation" below, and nothing there overrides a rule of this section.

## Owner delegation

Decided by the repository owner on 2026-09-24. It says which decisions an
agent takes on its own and which stay the owner's. It never widens "Approval
semantics" above: where the two could be read differently, the reserved list
wins.

**Agents decide, record and report afterwards:**

- reversible choices inside a direction the owner has adopted;
- relocation-only changes, which move text or code without changing a
  requirement, shown unchanged by a mechanical comparison;
- test and evidence design;
- repository and worktree hygiene;
- unambiguous corrections of internal inconsistencies.

Each such decision is recorded where it takes effect (a dated section 5 entry,
a commit message, a pull-request body) and named in the next handoff.

**Reserved to the owner:**

- ratification;
- waivers;
- publication;
- spending money, or provider usage;
- trust roots and signing;
- changes to the gate, the check suite or the acceptance authority;
- anything visible outside this repository, or affecting another adopter
  (Rahi);
- deleting anything remote.

**Proposals are never left uncommitted.** A proposal is committed and pushed,
as a draft branch or a pull request, the day it is written; a worktree holding
it is removed when its pull request merges. An uncommitted proposal in a
worktree is invisible to the owner and to the next session.

**Every handoff ends with one decision table:** item, options, recommended
default, consequence of the default. Only decisions that are the owner's
appear in it; everything an agent decided is reported above it.

## Authored-content rules

Checked mechanically by `scripts/check-authored-content.sh`, which is part of
`make gate`. Both rules are spec `001` section 3.6.

1. **No U+2014.** Use a colon, semicolon, comma, parentheses or two sentences.
   U+2013 is permitted only for numeric or section ranges.
2. **No agent-session URL and no session-tracking trailer**, anywhere: files,
   commit messages, pull-request bodies, issues, reviews, release notes. Do not
   substitute another tracking link. No agent attribution line in a commit message
   or a pull-request description.

## Citing another corpus

**A spec-spine ordinal written before 2026-09-20 names a different document
today.** spec-spine collapsed its corpus and renumbered contiguously, so an old
ordinal does not dangle: it resolves, to the wrong spec. `088` is the example
that bites, because it used to be change classification and now is the template
spec.

`~/DevWork/spec-spine/docs/corpus-map.md` resolves both directions and is the
authority. Ten ordinals cited here were corrected against it on 2026-09-21:
037 to 034, 039 to 036, 057 to 050, 069 to 058, 088 to 071, 091 to 072, 097 to
078, 098 to 079, 101 to 080, 102 to 081.

**Not every three-digit ordinal in this corpus is spec-spine's.** Spec `001`
section 3.9 cites the archived predecessor's specs (032, 040, 042, 043, 102,
111 to 114), `004` section 1 cites its 043, 114, 116, 120 and 124, and
`crates/statecraft-envelope/PROVENANCE.md` cites hqgit's. Those belong to other
corpora, they were not renumbered by spec-spine, and remapping one would be the
same defect in the other direction. Read the sentence before you trust the
number, and prefer `registry list` over any ordinal you did not measure.

## Claims

Four grades, stated separately and never inferred from one another: **specified**,
**implemented**, **tested**, **released**. A spec being `approved` grants no grade
above *specified*. Any document here that calls a behavior implemented, tested or
released names the evidence in the same sentence.
