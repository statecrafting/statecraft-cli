---
id: "069-code-free-disk"
title: "Setup profile revision 25: the managed code job can free runner disk before it builds"
status: draft
implementation: pending
created: "2026-11-04"
summary: >
  Amends spec 002's setup profile and spec 058's code job. A new boolean
  parameter, ci.code.free_disk, off by default, renders one step in the
  managed code job before the build: it removes the runner's unused
  preinstalled toolchains and reports the free space before and after.
  Off, the rendered workflow is revision 24's. An external code selection
  refuses the parameter, because its job is the project's own.
amends:
  - "002-environment-lifecycle"
  - "058-ci-throughput-profile"
depends_on:
  - "032-external-code-profile"
  - "058-ci-throughput-profile"
  - "067-post-merge-push"
obligations:
  - id: "R-1"
    kind: requirement
    text: "The setup parameter ci.code.free_disk is a boolean, false by default; any other value refuses the plan."
    anchor: "3-1-the-parameter"
  - id: "R-2"
    kind: requirement
    text: "With ci.code.free_disk true, the managed code job runs one step before Build, test, clippy, fmt that prints df -h / and removes exactly the listed preinstalled directories with sudo rm -rf, then prints df -h / again; the step never fails the job."
    anchor: "3-2-the-step"
  - id: "R-3"
    kind: requirement
    text: "On a post-merge push (spec 067) the step carries the build's condition, so it runs only when the cargo key missed and the build fills it."
    anchor: "3-3-on-a-post-merge-push"
  - id: "R-4"
    kind: requirement
    text: "ci.code.free_disk true together with an external code selection refuses the plan, naming both parameters."
    anchor: "3-4-an-external-code-selection"
  - id: "R-5"
    kind: requirement
    text: "With ci.code.free_disk absent or false, the rendered workflow is revision 24's, apart from the revision, the identity and the comment describing this revision."
    anchor: "3-5-off-by-default"
---

# 069: The managed code job can free runner disk before it builds

## 1. Purpose

The managed `code` job builds, tests and lints the whole workspace on one
`ubuntu-latest` runner. An adopter (rustev) measured the runner: 72 GB, 53
GB used before the job starts, so 19 GB free. From 2026-11-03 to 11-04 the
job failed six times on a full disk, with `No space left on device` in
rustc output and `ld terminated with signal 7 [Bus error]`. Its own
repository-owned jobs already remove the preinstalled toolchains they never
use first, which frees about 21 GB. The profile offers no way to do the same
in the job it manages, and a project may not edit a managed workflow.

## 2. Territory

The registered profile's `statecraft-ci.yml` template, the revision, the
parameter's parsing and its refusal, the remote obligation, and tests under
`crates/statecraft-home/`. This spec amends spec 002's setup profile, which
lists the parameters a plan accepts, and spec 058's `code` job, whose steps
it extends by one. Every other job is unchanged.

## 3. Behavior

### 3.1 The parameter

`ci.code.free_disk` is a setup parameter. Its value is `true` or `false`,
and it defaults to `false`. Any other value refuses the plan with exit 2,
as every malformed parameter does.

### 3.2 The step

With the parameter `true`, the managed `code` job renders one step, named
`Free runner disk`, immediately before `Build, test, clippy, fmt`. It runs:

```sh
df -h /
sudo rm -rf /usr/share/dotnet /usr/local/lib/android /opt/ghc /usr/local/.ghcup /opt/hostedtoolcache/CodeQL || true
df -h /
```

The list is fixed by the profile and is the same in every target. Each
directory is a preinstalled toolchain that a Rust build does not read. The
step ends `success` whether or not a directory exists, so a runner image
that drops one of them changes nothing. The two `df` lines are the evidence
of what the step freed.

The step sits after the cache restore, so a restored `target` is already on
disk when the space is measured, and before the build, which is what needs
the space.

### 3.3 On a post-merge push

On a post-merge push (spec 067 section 3.2), the build runs only when the
cargo key missed. The step carries the same condition, so a push whose key
hit removes nothing and spends no time on it.

### 3.4 An external code selection

An external code selection (spec 032) replaces the `code` job with a call of
the project's own workflow, so the managed job and its steps are not
rendered. `ci.code.free_disk: true` beside `ci.code` of kind `external`
refuses the plan with exit 2, naming both parameters, instead of being
silently ignored. That project frees disk in its own workflow.

### 3.5 Off by default

With the parameter absent or `false`, the rendered workflow is revision
24's. The render differs only in its revision, its identity and a comment
that describes this revision. The policy records the parameter's value with
the other parameters.

### 3.6 The revision

The profile becomes revision 25 with a new identity. A revision-24 project
upgrades through the ordinary plan and apply. Every rendered file names
the revision in its header and is rewritten for it; apart from that line,
only `statecraft-ci.yml` changes, and only a project that sets the parameter
gets the new step. The
upgrade pull request is an authority change and needs the owner's
exception once.

## 4. Out of scope

- Freeing disk in any job other than the managed `code` job.
- A project-chosen list of directories; the list is the profile's.
- Reducing what the build writes, for example dependencies without
  debuginfo; that is the project's build configuration.
- Adopting revision 25 in this repository's own CI, which is its own change.

## 5. Resolved decisions

**2026-11-04: requested by the owner.** The owner, on 2026-11-04: "The code
job still has no disk cleanup. The CI setup profile has no setting for it,
so I added it as item E in
travel-memory/handoffs/rustev/m1-2026-11-02/STATECRAFT-CLI-HANDOFF.md for
statecraft-cli." Item E asks for "a profile parameter, for example
`code.free_disk: true`, that renders a step before \"Build, test, clippy,
fmt\"" with the command in section 3.2 and `df -h /` before and after. The
parameter is named `ci.code.free_disk`, beside `ci.code`, because every CI
parameter of the profile is under `ci.`. Drafted for the owner to ratify.

## Verification

Acceptance is declared by the implementing change. It renders the step only
under the parameter, in the right place and with the post-merge condition
on a queued project; refuses a non-boolean value and the external
combination; and upgrades a revision-24 project in one plan, with and
without the parameter, after which a second plan writes nothing.
