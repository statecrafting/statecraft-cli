---
id: "127-pr-review-note"
title: "The review note: what the local cycle found, posted where the pull request can see it"
status: draft
created: "2026-09-10"
implementation: pending
depends_on:
  - "109-governed-harness"
establishes:
  - "scripts/pr-note.py"
  - "scripts/pr-note.test.py"
extends:
  # 109 owns the harness: AGENTS.md "Working the backlog" gains the step that
  # posts the note, and the Makefile a target, the same shape 118 used.
  - { spec: "109-governed-harness", unit: "AGENTS.md", nature: additive }
  - { spec: "109-governed-harness", unit: { kind: section, file: "Makefile", anchor: "pr-note" }, nature: additive }
summary: >
  This repository reviews locally. `/code-review` runs in the session,
  before the push, and its findings never leave the terminal: the pull
  request shows three green checks and no reasoning. The checks UI already
  reports what CI can know, so the gap is not a CI summary but the thing
  only the session holds, namely what the review found, what was fixed and
  what was accepted. This spec adds one sticky comment per pull request,
  updated in place on every push, carrying the head sha, what changed, the
  gate verdicts that actually ran, and the review's findings with their
  disposition. It is posted by the session, it never invents a verdict, and
  failing to post it never blocks a ship.
---

# 127: The review note

## 1. Purpose

The governed loop reviews before it pushes. `/code-review` runs in the
session against the diff, the gate runs locally in the exact chain CI runs,
and a finding is either fixed or consciously accepted. None of that reaches
the pull request. A reviewer opening one sees three green checks, a title
and a body, and has no way to tell a change that was reviewed and had two
findings fixed from one that was pushed without a glance.

The obvious fix is the wrong one. A CI job that comments the gate results
restates what the checks UI already shows, and it cannot see the local
review at all, because the review happened on a machine CI never touches.
What is missing is not a second rendering of `pass`; it is the session's
own knowledge.

So the session posts it. One comment per pull request, updated in place,
written after each push by the process that just did the work.

This spec also settles a question the repository has been carrying
silently: there is a `CLAUDE_CODE_OAUTH_TOKEN` secret and nothing uses it.
D-5 records the decision rather than leaving it as a gap.

## 2. Territory

- `scripts/pr-note.py`: the upsert. Builds the note from inputs it is given
  and posts or updates the single marked comment on a pull request.
- `scripts/pr-note.test.py`: its test, the shape 118's generator uses.
- `AGENTS.md` (extends 109): "Working the backlog" step 7 gains the note,
  so every agent that ships reads the instruction, not only Claude.
- `Makefile` (extends 109), target `pr-note`: the one invocation, so the
  command has a single definition the way `make gate` does.

## 3. Behavior

### B-1. One comment, found by marker, never a thread

The note carries an HTML marker (`<!-- statecraft:pr-note -->`) as its first
line. The script lists the pull request's issue comments, finds the one
whose body contains the marker, and `PATCH`es it; absent one, it `POST`s.

`gh pr comment --edit-last --create-if-none` exists and is deliberately not
used: it edits the *current user's last comment*, which on a pull request
where the session has also answered a review thread is the wrong comment.
D-1 records this. The marker makes the target explicit rather than
positional.

### B-2. What the note carries

In order:

- the head sha the note describes, and the time it was written;
- **what this push changed**, one line, in the same voice as the commit;
- **the gate**, each command that actually ran with its exit code:
  `make gate` always, the cargo half when the diff touched `src/` or
  `tests/`, the bun half when it touched `members/`;
- **the review**: every `/code-review` finding as `file:line`, its severity,
  and its disposition, one of `fixed` (with the sha that fixed it),
  `accepted` (with the reason), or `deferred` (with where it went).

### B-3. Absence is stated, never implied

A section with nothing to report says so. A review that found nothing reads
`no findings`; a review that did not run reads `not run`, and the two are
never collapsed. A gate half that did not apply reads `not applicable`, not
a blank.

This is 125 D-3's rule and kit 082's `not read` versus `none`, applied
here: a missing line must not read as an absence of findings to someone
deciding whether to look closer.

### B-4. The note never invents a verdict

The script reports the exit codes it is handed and the findings it is
handed. It does not run the gate, does not infer a result from a green
check, and has no opinion. A caller that passes nothing gets a note saying
nothing ran, which is the honest output for that input.

### B-5. Posting is not a gate

A failure to post the note (no network, no token, no pull request yet) is
reported and does not fail the ship. Visibility is worth having and is not
worth blocking a correct change over, and a ship stage that fails on a
comment would teach the next session to skip the step.

### B-6. The note carries no credential

The script writes only its given inputs and never echoes an environment
variable, a token, a remote URL with userinfo, or the contents of a file it
was not given. `pr-note.test.py` asserts it on a payload seeded with
token-shaped strings.

## 4. Functional requirements

- **FR-001.** Upsert: two runs against one pull request leave exactly one
  marked comment, the second body replacing the first.
- **FR-002.** Marker: a comment by the same user without the marker is
  never touched.
- **FR-003.** Absence: each of `no findings`, `not run` and `not applicable`
  renders distinctly (B-3).
- **FR-004.** Honesty: given no gate results, the note says nothing ran
  rather than omitting the section (B-4).
- **FR-005.** Non-blocking: a failing post exits non-zero from the script
  but `make pr-note` reports and returns success (B-5).
- **FR-006.** Redaction: token-shaped inputs do not appear in the body.

## 5. Acceptance

- `python3 scripts/pr-note.test.py` exits 0.
- `make gate` exits 0.
- A live round on this repository: two pushes to one pull request produce
  exactly **one** comment, whose final body names the second head sha and
  the findings of the review that ran before it.
- A round where the review found nothing shows `no findings`, and a round
  where it was skipped shows `not run`.
- With posting forced to fail, the ship stage still completes and the
  failure is reported in the session.

## Verification

```sh
python3 scripts/pr-note.test.py
make gate
```

## 6. Out of scope

- **A CI-side AI review.** See D-5.
- **Reviewing.** This spec transports a review's result; it does not review,
  and it changes nothing about `/code-review`.
- **The checks UI.** The note does not restate per-job pass and fail, which
  GitHub already renders better than a comment can.
- **Review threads.** Answering a reviewer stays `/shepherd`'s job on its
  own threads; the note is a one-way record and never a reply.
- **Any skill file.** 109 §3.1 requires every `SKILL.md` byte-identical to
  the kit, so the instruction lives in `AGENTS.md`, which is where this
  repository's project layer belongs.

## 7. Resolved decisions

D-1 (2026-09-10). Marker-based upsert, not `gh pr comment --edit-last`.
`--edit-last` (available in gh 2.73.0, with `--create-if-none`) targets the
current user's most recent comment, which is only the note when the session
has posted nothing else. A session that also comments on a pull request
would silently overwrite that instead. A marker names the target rather
than counting backwards from the end.

D-2 (2026-09-10). The session posts, not CI. CI cannot see the local
review, which is the whole content worth adding; a CI comment would restate
the checks UI. The cost is that the step can be skipped, which is why it
lives in `AGENTS.md` "Working the backlog", read by every agent, rather
than in a skill that only one of them runs.

D-3 (2026-09-10). Posting failures are reported, not fatal (B-5). The
alternative teaches the next session that the step is a hazard, and a
skipped step provides no visibility at all.

D-4 (2026-09-10). The note is not a substitute for the checks UI, and B-2
deliberately omits per-job status. Two renderings of the same fact drift,
and the one in the comment is the one that goes stale.

D-5 (2026-09-10). No CI-side AI review, and the decision is recorded here
because until now it existed only as a gap. This repository carries a
`CLAUDE_CODE_OAUTH_TOKEN` secret, added 2026-07-23, referenced by no
workflow: no `anthropics/claude-*` action appears anywhere under
`.github/`, and no spec or design document mentions one. The local cycle
reviews before the push, inside the loop that can act on a finding, and it
costs no CI minutes and no round trip. A CI review would arrive after the
push, restate what the session already knew, and lengthen every iteration.
The token is therefore left unused deliberately rather than by oversight.
Removing the secret is a separate operator action and is not required by
this spec; a later session that wants a CI review amends this decision
rather than discovering an unexplained secret and guessing.

## Status (2026-09-10)

Authored `draft`, `implementation: pending`. It comes from an observation
made while shipping specs 109 and 126: the pull requests for both carried
three green checks and no trace of the review that had already happened
locally.

The facts D-5 rests on were measured on 2026-09-10: `gh secret list` shows
`CLAUDE_CODE_OAUTH_TOKEN` dated 2026-07-23; a repository-wide search finds
no reference to it and no `anthropics/claude-*` action; `.github/workflows/`
holds `ci.yml`, `members.yml`, `release.yml` and `spec-spine.yml`; and no
file under `specs/`, `docs/` or `standards/` mentions a CI-side AI review.

Approval is a human flip.
