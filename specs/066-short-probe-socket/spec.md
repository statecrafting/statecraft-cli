---
id: "066-short-probe-socket"
title: "The boundary self-test's probe socket lives under a short fixed directory, so a long home path cannot refuse a launch"
status: approved
implementation: pending
created: "2026-10-31"
summary: >
  Amends spec 064 section 3.3. The self-test's listening Unix socket moves
  from a directory under the product home's boundary entry to a private
  directory under /tmp, so its path no longer grows with the home's path
  and stays within the platform's socket path limit (104 bytes on macOS,
  108 on Linux). The sentinel stays in boundary/, the protected roots and
  the confinement record are unchanged, and rule 6 of spec 004 section
  3.18 still decides whether the probe's connect is refused.
amends:
  - "064-protected-home-entries"
# No `extends` yet: the implementing change adds the edge on
# crates/statecraft-home/ (002) and crates/statecraft-cli/ (006) with the
# code, because the gate counts an extender as an owner.
depends_on:
  - "004-execution-adapter"
  - "064-protected-home-entries"
obligations:
  - id: "R-1"
    kind: requirement
    text: "The launch confinement and the witness sidecar each bind the self-test's probe socket in a fresh directory created under /tmp with mode 0700, whose path does not depend on the product home's path, and remove it when the self-test's owner is dropped."
    anchor: "3-1-where-the-socket-lives"
  - id: "R-2"
    kind: requirement
    text: "The home sentinel stays inside the boundary entry, and the protected roots a confinement record names, and its digest, are the same as under spec 064."
    anchor: "3-2-what-does-not-move"
  - id: "R-3"
    kind: requirement
    text: "A launch whose product home path is longer than the platform's socket path limit allows under spec 064 is no longer refused at the self-test's socket bind."
    anchor: "3-3-what-a-long-home-now-does"
  - id: "V-1"
    kind: verification
    text: "A launch with a product home whose path is over 100 bytes passes the self-test on macOS and Linux, the probe still reports unixDenied, and a confinement record's protected roots and digest equal the ones spec 064's tests record."
    anchor: "verification"
    inputs:
      - "crates/statecraft-cli/tests/protected_boundary.rs"
---

# 066: A short probe socket

## 1. Purpose

Spec 064 section 3.3 moved the self-test's sentinel and its probe socket
directory into the product home's fixed `boundary/` entry. The socket's path is
then `<home>/boundary/probe-XXXXXX/socket`, 29 bytes longer than the home's own
path. A Unix socket's path must fit `sun_path`: 104 bytes on macOS, including
the terminating zero, and 108 on Linux. So on macOS a home longer than 74 bytes
refuses every launch with `boundary unavailable: path must be shorter than
SUN_LEN`, before any rule is judged. It fails closed, and it was found when a
test's home under the macOS `/var/folders` temporary directory reached 110
bytes (spec 064 section 5, 2026-10-31).

The owner decided on 2026-10-31 that a home's path length should not refuse a
launch.

## 2. Territory

This spec amends spec 064 section 3.3. Its implementation changes the launch
confinement (`crates/statecraft-home/src/confinement.rs`) and the witness
sidecar (`crates/statecraft-home/src/witness.rs`) in `statecraft-home`, spec
002's crate, and adds tests in `statecraft-cli`.

## 3. Behavior

### 3.1 Where the socket lives

The self-test's listening Unix socket is bound as `socket` inside a fresh
directory created under `/tmp` with the prefix `statecraft-probe-` and mode
`0700`. On macOS `/tmp` is `/private/tmp`; the socket's path is under 40
bytes on both platforms whatever the home's path. The directory is removed
when the value owning the self-test is dropped, as the directory under
`boundary/` was.

`/tmp` is used rather than the platform temporary directory, because on macOS
that is a per-user path under `/var/folders` whose length the product does not
control.

### 3.2 What does not move

The home sentinel stays inside `boundary/`. The protected roots a confinement
record names, and its digest, are exactly those of spec 064 section 3.3,
because the probe socket's path is an input to the self-test and is not
recorded. Nothing in spec 004 section 3.18 changes: rule 6 refuses the probe's
connect to any Unix-domain socket but the platform's name resolution, wherever
the socket lives, and the self-test still requires `unixDenied`.

### 3.3 What a long home now does

A launch whose home path is too long for spec 064's socket path is no longer
refused at the bind. A path too long for any other use (an open or a create)
still refuses where it occurs, as before.

## 4. Out of scope

- Any other socket or path-length limit; this product binds no other Unix
  socket.
- Moving the sentinel, the records or any protected root.
- The test-only change of spec 064 section 5 (2026-10-31), which stays.

## 5. Resolved decisions

**2026-10-31 (owner decision): a long home path must not refuse a launch.**
Asked whether the product should use a short socket location so longer homes
work, the owner answered yes.

**2026-10-31: `/tmp`, not a relative bind.** Binding by a relative path after
changing the working directory would keep the socket inside `boundary/`, but
the working directory is process-wide and the launch path is not
single-threaded. A private directory under `/tmp` needs no process-wide
state, and a confined child cannot remove it: `/tmp` is in none of rule 3's
writable roots.

## Verification

Ratified by the owner on 2026-10-31. Acceptance is declared by the
implementing change, which will run a launch with a product home whose path is
over 100 bytes on each platform, show the self-test passing with `unixDenied`,
and compare a confinement record's protected roots and digest with the ones
spec 064's tests record.
