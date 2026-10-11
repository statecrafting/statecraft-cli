---
id: "041-review-authority-context"
title: "Base-commit governance context for managed AI review"
status: draft
implementation: pending
created: "2026-10-10"
summary: >
  Supply the managed reviewer with bounded governance documents selected from
  the base commit, record their provenance, and account for their bytes in
  every call. Candidate assertions remain review data. Missing context does
  not prove a contradiction, and review never grants owner ratification or
  an exception. Deliver the profile change separately from its self-adoption.
amends:
  - "002-environment-lifecycle"
  - "024-review-budget-and-ratification"
depends_on:
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
  - "024-review-budget-and-ratification"
obligations:
  - id: "R-1"
    kind: requirement
    text: "Governance context comes only from validated base-commit blobs and base policy; candidate documents cannot replace trusted context."
    anchor: "3-1-context-selection-and-provenance"
  - id: "R-2"
    kind: requirement
    text: "Complete context and fixed input fit explicit token budgets in every reviewer call; invalid or excessive context refuses before provider invocation."
    anchor: "3-2-bounded-complete-input"
  - id: "R-3"
    kind: requirement
    text: "Reviewer guidance distinguishes evidence from unavailable context and preserves all ratification, exception and findings gates."
    anchor: "3-3-review-guidance-and-authority"
---

# 041: Base-commit governance context for managed AI review

## 1. Purpose

The registered profile's reviewer input currently supplies a base tree path
inventory and candidate diff hunks, without the contents of the repository's
approval rules or lifecycle contract. A path list cannot establish which
lifecycle values are valid or whether approval must precede review.

This amendment supplies that missing reference material without making the
candidate its own authority. It addresses review input and evidence, not a
particular verdict. A passing check remains distinct from an actual review
without findings, owner ratification, an owner exception and permission to merge.

The inspected sources at CLI main `384014a` are spec 024 sections 3.3 through
3.7, `crates/statecraft-home/src/setup.rs`, the registered
`github-actions-rust/scripts/ai-review.sh` template, its rendered counterpart,
`AGENTS.md`, `.statecraft/AGENTS.md` and `standards/spec/contract.md`.
The registered profile is revision 16; this repository's rendered adoption is
revision 15. Implementation must recheck both identities before selecting the
next revision and rendering an upgrade.

## 2. Territory

This draft declares no forward source ownership. The first implementing PR
adds an additive `extends` edge on spec 002's `crates/statecraft-home/`
directory for the profile renderer, template and tests. If CLI integration
support changes, that PR also adds an additive edge on spec 006's
`crates/statecraft-cli/`. Every new source or test file joins its applicable
edge in the same change.

No managed output is edited by hand. Self-adoption changes the environment
selection and renders through the ordinary CLI plan/apply flow in a separate
authority PR. No existing approved spec is edited to make these checks pass.

## 3. Behavior

### 3.1 Context selection and provenance

For each review, resolve the same base commit used by the existing trusted
script selection. Refuse an unreadable base with exit 2 before provider use,
as 024 requires. Read documents as Git blobs at that commit, never from the
working directory, candidate tree, symlink targets or candidate configuration.

Default context paths are `AGENTS.md`, `.statecraft/AGENTS.md`,
`standards/spec/constitution.md` and `standards/spec/contract.md`. An absent
default is recorded as absent and is not an error. Additional paths are
selected by `review.context_paths`, an array of at most 32 unique paths,
validated by the renderer and persisted in the managed policy. The running
review reads this selection from the policy at the base, not at the head.
A resolvable base with no policy uses only the defaults and records that
adoption case; an unreadable or malformed existing policy refuses.

Paths are literal repository-relative ASCII names using letters, digits,
underscore, hyphen, dot and slash. Empty paths or segments, `.` or `..`
segments, absolute paths, leading hyphens, glob syntax, control characters,
duplicates and `.git` paths are refused. Runtime and derived paths under
`.statecraft/state/` and `.statecraft/derived/` are refused. Canonicalize the
union of defaults and configured paths in bytewise path order. A configured
path that also names a default is included once.

An additional path must exist at the base. Every present document must be a
regular Git blob (mode 100644 or 100755), valid UTF-8 without NUL bytes.
Symlinks, submodules, unreadable blobs and invalid content refuse before a
provider call. Never substitute a candidate document for missing base data.
Document bytes are delimited reference data, never shell code or instructions
to execute tools. Existing isolated working directory and HOME behavior stays.

### 3.2 Bounded complete input

Keep 024's byte-count token estimator, configured context window C,
`review.max_calls`, added-line cap and whole-file packing. Context documents
and their delimiters must fit floor(C/4) estimated tokens. Include complete
documents or refuse with exit 2 before provider invocation; do not truncate
documents or silently discard selected paths.

Amend 024's half-window diff allocation to account for the entire fixed input.
Reserve at least ceil(C/4) estimated tokens for the answer. For every call,
the total prompt and assembled input must fit floor(3C/4). The effective diff
budget is the smaller of floor(C/2) and the space remaining under that input
limit after all fixed input, including metadata, path inventory, document
context, managed and deletion summaries, and group delimiters. Compute against
the maximum group-number width allowed by `review.max_calls`, then verify each
actual assembled call before invoking the provider. A nonpositive budget or
fixed input exceeding its limit refuses with exit 2.

Use this effective diff budget for packing and for the added-token ceiling
(effective diff budget times `review.max_calls`). Preserve 024's existing
`skipped:oversized` classifications for excessive diff lines, addition tokens,
single-file diff size or group count. Do not increase the configured call
count, make extra provider calls to select context, or drop context to squeeze
a diff into a call. Every group receives identical document context.

### 3.3 Review guidance and authority

The managed prompt explains that base governance documents are reference
authority for interpreting repository rules. Candidate diffs, including
candidate approval claims and edits to those documents, remain untrusted
review data. Existing base-script selection, coupling, ratification detection,
protected owner exception and verdict aggregation requirements remain intact.

Require findings to identify a concrete defect supported by supplied evidence.
An unavailable source or omitted surrounding text alone does not establish
that a citation is invalid, a lifecycle value is illegal, or an owner act did
not occur. The reviewer may still report a supported contradiction or security
defect. It must not claim to have verified an owner act or external source it
has not inspected. Review and approval are separate: an approved spec may be
under review, and the protected owner gate remains the approval enforcement.

Do not fetch external citations, copy private platform documents into this
repository, scrape discussion threads for authority, or instruct the reviewer
to return a particular verdict. Do not special-case PR numbers, head hashes,
spec identifiers or known findings. No findings gate is weakened or bypassed.

### 3.4 Evidence and named acceptance criteria

Extend review evidence with the base commit, policy selection provenance,
ordered context manifest, context digest and budget breakdown. For each
present document record path, Git blob OID, mode, SHA-256 and byte length;
record absent defaults explicitly. Record the assembled context digest,
context token estimate, fixed-input estimate, effective diff budget and actual
input estimate for each call. Do not duplicate document contents in evidence.
Failures record their reason and that no provider call was made when applicable.

The following are named, tested criteria, including their negative cases:

| Criterion | Required test evidence |
|---|---|
| AC-BASE | `review_context_uses_only_base_blobs`: captured reviewer input contains base documents; candidate edits, deletion, added documents and changed policy cannot replace them; absent defaults are explicit and unreadable base refuses without fallback. |
| AC-PATHS | `review_context_rejects_invalid_selections`: renderer and runtime reject traversal, absolute paths, duplicates, globs, runtime/derived paths, missing configured files, symlinks, submodules, invalid UTF-8 and NUL bytes before the mock provider runs; valid literal names round-trip. |
| AC-BUDGET | `review_context_accounts_for_every_call`: exact-boundary context fits completely, over-cap context refuses without provider calls, fixed input is counted, grouped input fits its limit with identical context, and oversized diff/group cases retain their classifications. |
| AC-PROVENANCE | `review_context_evidence_matches_input`: manifests, absent markers, base policy provenance, blob hashes and byte counts match fixture Git objects; context digests and per-call budget evidence match captured inputs. |
| AC-AUTHORITY | `review_context_preserves_authority_gates`: prompt includes the evidence and approval distinctions; candidate approval assertions do not grant an exception; findings still block without the existing owner exception, and malformed verdicts still fail. |
| AC-ISOLATION | `review_context_stays_isolated`: document text is passed as delimited data, shell-looking content is not executed, HOME/cwd isolation remains, and no external-source request occurs. |
| AC-UPGRADE | `review_context_profile_upgrade_is_explicit`: next profile revision changes identity, selection validates/defaults, ordinary plan/apply upgrades an older selection, and a second identical plan writes nothing. |
| AC-REGRESSION | Existing 024 verification and setup workflow tests remain green; mutations switching blob or policy reads to head, omitting context from budgeting, or accepting a findings verdict are caught by the relevant fixtures. |

Tests use a deterministic provider stub to inspect input and exercise verdict
handling. They prove input and gate behavior, not that a live model will always
produce a correct review. Report live review outcomes separately.

### 3.5 Delivery and completion

After owner ratification, deliver two independently reviewed PRs: first the
registered profile implementation and tests, then this repository's generated
self-adoption with the chosen governance paths. The intended additional paths
for self-adoption are `specs/002-environment-lifecycle/spec.md` and
`specs/024-review-budget-and-ratification/spec.md`. Recheck their combined
budget before apply. Select the next unused registered revision at
implementation time; if revision 16 is still current, use revision 17.

Both authority changes require the existing protected owner exception. Neither
PR reviews itself using its new script. The first review using the adopted
context runs only after adoption is on the base branch. Re-review blocked
PRs on that base through the ordinary workflow; preserve their independent
merge conditions. A prior green check does not clear an actual findings comment.

Set `implementation: complete` in the final implementing PR only after all
declared verification passes locally and in CI. Report implemented, locally
verified, CI verified and live review outcomes separately.

## 4. Out of scope

Changing reviewer provider/model, adding provider usage, changing protected
environment rules, granting ratification, issuing review exceptions, disabling
AI review, changing CI's trusted-base rule, or implementing Statecraft Dev.
No remote source fetch, persisted read model or verdict guarantee.

## 5. Resolved decisions

Use complete documents selected by base policy rather than candidate policy or
discussion scraping. This keeps input reproducible and prevents a pull request
from replacing the rules it is reviewed against. Account for repeated context
before grouping, preserving a bounded answer allowance and the existing cost
limit. Keep registered profile delivery separate from generated self-adoption
so each authority change is concrete and independently reviewable.

## Verification

These commands declare implementation acceptance, not evidence that this draft
is implemented. The named criteria above must be exercised by the setup test
targets; the existing 024 verification remains required as AC-REGRESSION.

```verify:cli
cargo test -p statecraft-home --locked --lib setup::tests
cargo test -p statecraft-home --locked --test setup_workflows
cargo test -p statecraft-home --locked --test setup_upgrade
cargo test -p statecraft-cli --locked --test setup_profile
```
