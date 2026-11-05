---
id: "052-provider-golden-captures"
title: "Version-pinned provider captures, a quarantine and a divergence ledger gate a new provider version"
status: approved
implementation: pending
created: "2026-10-21"
summary: >
  Turns the Claude Code adapter's recorded streams into a governed corpus of
  golden captures, each pinned to a provider version and a named scenario,
  with a quarantine list for captures that are wrong recordings and a
  divergence ledger for differences a reviewer has accepted. A new provider
  version is compared against the goldens offline; an unreviewed divergence
  refuses that version's qualification. Capturing costs provider usage, so it
  is owner-triggered; comparison runs in CI with no provider and no network.
amends:
  - "004-execution-adapter"
extends:
  - { spec: "004-execution-adapter", unit: { kind: directory, path: "crates/statecraft-adapter-claude-code/" }, nature: additive }
  - { spec: "006-command-surface", unit: { kind: directory, path: "crates/statecraft-cli/" }, nature: additive }
depends_on:
  - "004-execution-adapter"
# `obligations` is spec-spine's own frontmatter field, used by approved specs
# such as 013, 014 and 018; `make gate` lints it with --fail-on-warn.
obligations:
  - id: "I-1"
    kind: invariant
    text: "No provider version is qualified while a scenario's capture for it differs from that scenario's golden in a way neither the golden nor a reviewed divergence-ledger entry accounts for."
    anchor: "3-4-comparing-a-new-version"
  - id: "R-1"
    kind: requirement
    text: "Every golden names its provider version, platform, scenario and capture date, and the comparison refuses a capture whose version or scenario does not match the golden it is compared to."
    anchor: "3-1-the-corpus"
  - id: "R-2"
    kind: requirement
    text: "A quarantined capture is excluded from comparison only with a recorded reason and is kept, never deleted; lifting it requires a fresh capture."
    anchor: "3-2-the-quarantine"
  - id: "R-3"
    kind: requirement
    text: "Comparison runs offline: it spawns no provider, opens no network connection and reads no credential."
    anchor: "3-4-comparing-a-new-version"
  - id: "V-1"
    kind: verification
    text: "A synthetic capture differing from its golden in a mapped field refuses qualification; the same capture with a reviewed ledger entry passes; a quarantined golden is skipped and reported; the comparison runs in CI with networking denied."
    anchor: "verification"
    inputs:
      - "crates/statecraft-adapter-claude-code/testdata/stream/README.md"
---

# 052: Provider golden captures

## 1. Purpose

Spec 004 sections 3.9 to 3.13 rest on streams recorded from Claude Code
2.1.267 (`crates/statecraft-adapter-claude-code/testdata/stream/`), and that
directory's README says plainly that a fixture is evidence of what one version
emitted, not of what the next one emits. The provider ships often (the
installed version on 2026-10-21 is 2.1.307), and the outcome mapping keys on
fields a release can move: a denial carried under `success`, an API error
under `success` with `is_error`. Spec 004 section 3.4 already refuses to carry
a qualification across binary versions; this spec gives a new version
something concrete to be compared against, and makes a difference visible
before it becomes a wrong outcome.

**Drafts this one cites, not yet on main:** spec 055, the Codex adapter (pull request 309). Each is cited by its proposed number so the reference survives ratification.

## 2. Territory

The corpus moves under a governed fixtures directory in the Claude Code
adapter's crate, with the quarantine and the ledger beside it. The
comparison is a library function in that crate, and `statecraft-cli` gains a
capture verb and a compare verb. Spec 004's negative suite and qualification
record are unchanged in shape; qualification gains one precondition.

## 3. Behavior

### 3.1 The corpus

A golden is one scenario's stream, recorded from one provider version on one
platform, with a manifest entry naming the version, platform, scenario,
invocation and capture date. Scenarios are named once and reused across
versions; today's five recordings become the first five scenarios at 2.1.267.
Redactions are listed per capture, as the existing README lists them.

### 3.2 The quarantine

A capture whose recording is wrong, rather than a provider behaviour that
changed, is quarantined: it stays in the corpus under a quarantine directory,
and a quarantine list names it with a reason and the evidence. The model is
preloop-ci's `.runner-watch/quarantine.toml` and
`docs/contributing/fidelity-gap.md` (MIT), where three contaminated captures
are kept as wire-shape evidence instead of deleted. Lifting a quarantine
entry needs a fresh capture of that scenario.

### 3.3 The divergence ledger

A ledger entry records an accepted difference between a version's capture and
its golden: the scenario, the version, the fields that differ, why it does not
change the outcome mapping (or the spec amendment that changed the mapping),
and the reviewer. The ledger is append-only; a later version that diverges
the same way needs its own entry.

### 3.4 Comparing a new version

The compare verb reads a version's captures and the goldens and reports, per
scenario, `matches`, `diverges-reviewed` (a ledger entry covers it),
`diverges-unreviewed`, `quarantined` or `missing`. Comparison is by meaning
over the fields the adapter maps (spec 004 sections 3.10 to 3.13), not by
bytes: ids, timestamps, costs and session identifiers differ every run and are
named as ignored. A version with any `diverges-unreviewed` or `missing`
scenario is not qualifiable; spec 004 section 3.4's record for it is refused
naming the scenarios. It runs offline.

### 3.5 Capturing

The capture verb runs each scenario against the installed provider and writes
captures for its version. It costs provider usage, so it runs only when an
operator invokes it, never in CI and never as a side effect of another verb.
Captures are reviewed and committed like any authored change.

## 4. Out of scope

- Any provider other than Claude Code; a second adapter brings its own corpus
  (spec 055, the Codex adapter, pull request 309).
- Recording wire traffic: spec 013's witness owns that, and its captures are
  untrusted testimony, not goldens.
- Running a provider in CI.

## 5. Resolved decisions

**2026-10-21: the gate is qualification, not `harness upgrade`.** The work
order named `harness upgrade` as the verb that diffs a new version. In this
corpus `harness upgrade` commits the shipped Statecraft harness revision (spec
006 section 3.1, spec 002), which has nothing to do with the provider's
version. The version gate already exists as spec 004 section 3.4's
per-version qualification, so the comparison is a precondition there and the
two new verbs are the capture and the compare.

**2026-11-01: the corpus stays where it is.** Section 2's governed fixtures
directory is `crates/statecraft-adapter-claude-code/testdata/stream/` itself:
V-1's input names its README and specs 004, 015 and 034 cite the path, so
nothing moves. `corpus.json`, `quarantine.json`, `ledger.json` and
`quarantine/` sit beside the five recordings, and a version's captures are a
directory with a `captures.json` of `corpus.json`'s shape.

**2026-11-01: meaning is ten mapped fields.** `golden::MAPPED_FIELDS`: the
mapped outcome and the provider's claim (3.13), `subtype`, `is_error` and
`terminal_reason`, the denied tools of `permission_denials` and of the
mid-stream notifications (3.11), whether a cost was reported and whether hook
events occurred (3.10), and which of the seven verbatim tool names the init
event applied (3.12). The tool count and every other tool name are ignored,
because the goldens' names are count-preserving placeholders; so are
`num_turns`, `model`, tool inputs and prose, beside the ids, timestamps and
costs section 3.4 names (`golden::IGNORED_FIELDS`).

**2026-11-01: what R-1 matches.** A capture's entry must name the version its
own init event reports, a capture set names one version, and a capture of a
scenario no golden or quarantine entry names is refused. The platform is
recorded and reported, not matched. A scenario is compared with its latest
golden by capture date.

**2026-11-01: quarantine and ledger rules.** An active quarantine entry makes
its scenario `quarantined` in every comparison until `liftedBy` names a golden
of that scenario captured after the quarantine date. A ledger entry covers a
divergence when its scenario and version match and it lists every differing
field; append-only is checked as dates that never decrease.

**2026-11-01: the precondition, and the order of the verbs.**
`qualification::record_compared` is section 3.4's refusal: it writes the
record for the compared version only when no scenario diverges unreviewed or
is missing, and names those that do. The compare verb is
`provider compare <captures-dir> --corpus <dir>`, exit 1 when not
qualifiable. The capture verb (3.5) follows in its own change, because its
scenario prompts and redaction are the owner's to review before any provider
usage is spent.

**2026-11-01: networking is denied by the test.** The CI workflow is a
rendered, managed file, so the acceptance runs the binary under
`sandbox-exec` with `(deny network*)` on macOS and in a fresh network
namespace on Linux, with an empty environment, and fails when neither starts.

**2026-11-02: the capture verb, its table and its redaction.** The verb is
`provider capture <out-dir>`: it refuses a directory that is not empty and a
`PATH` with no `claude`, runs the five scenarios in `corpus.json`'s order,
each in a fresh scratch git work tree, with the prompt on standard input,
and writes one redacted `<scenario>.jsonl` per scenario and a `captures.json`
that `provider compare` reads as it is. The scenario prompts, flags,
settings and recorded invocation strings are one table,
`capture::SCENARIOS` in the adapter's crate, beside `golden.rs`; spawning
stays in `statecraft-cli`. Redaction is `capture::redact`, a pure function
from raw stream lines to redacted lines implementing the stream README's
list and no other rule, with the goldens' placeholder spellings; the
`timestamp` fixed value is the capture date at midnight, as the goldens'
is theirs. Its tests run a fake `claude`; no verify line runs a provider.

**2026-11-04: the first live capture, and the redaction it needed.** The
owner authorized the live capture run on 2026-11-04. `provider capture` against
Claude Code 2.1.318 on macOS captured all five scenarios, and
`provider compare` found each `matches` against the 2.1.267 goldens: the
version is qualifiable. The captures were not committed, because 2.1.318's
`init` event carries two fields the README's list did not name,
`original_cwd` (the scratch work tree) and `additional_directories` (the
operator's own directories, from the operator's settings), and a tool's
`file_path` names the work tree, which the goldens had hidden by hand. The
README's list gains one rule, which `capture::redact` implements:
`original_cwd` becomes the placeholder path, the work tree becomes it wherever
a string names it, and `additional_directories` is emptied, so a capture
committed for 2.1.318 or later carries none of the operator's paths.

**2026-11-04: where a capture set is committed.** The spec names no place
for a version's captures. They sit under the governed fixtures directory as
`captures/<version>/`, exactly as `provider capture` wrote them, so
`provider compare captures/<version> --corpus .` reruns offline from the
tree. The first is 2.1.318, captured on macOS; every scenario `matches`.
Its review found one more field 2.1.318 adds, `credential_expires_at`, which
describes the operator's credential, so the README's list fixes it to the
capture date's midnight as it does `timestamp`. The provider's request,
message and tool-use ids stay verbatim, as the goldens keep them.

## Verification

Ratified by the owner on 2026-10-31. Acceptance is declared by the
implementing change, as the commands below: they compare a synthetic capture
that differs from its golden in a mapped field and see qualification refused,
add a reviewed ledger entry and see it pass, skip and report a quarantined
golden, and run the comparison in CI with networking denied. Each line is one
command.

```verify:cli
cargo test -p statecraft-adapter-claude-code --locked --test golden_captures
cargo test -p statecraft-adapter-claude-code --locked --lib golden
cargo test -p statecraft-adapter-claude-code --locked --lib qualification
cargo test -p statecraft-cli --locked --test provider_compare
cargo test -p statecraft-adapter-claude-code --locked --lib capture
cargo test -p statecraft-cli --locked --test provider_capture
```
