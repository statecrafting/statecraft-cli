---
id: "070-writable-alias-scan"
title: "A read-only root refuses only a link the child could write"
status: approved
implementation: pending
created: "2026-11-06"
summary: >
  Amends spec 004 section 3.18 rule 2's alias scan. An inaccessible root
  still refuses any multiply linked file. A read-only root (the target's
  working tree, its Git directory, another attempt's workspace) refuses a
  multiply linked file only when another of its names lies where the child
  may write. Cargo's build output in a checkout and Git objects shared with
  a local clone no longer refuse every launch.
amends:
  - "004-execution-adapter"
depends_on:
  - "004-execution-adapter"
obligations:
  - id: "R-1"
    kind: requirement
    text: "A multiply linked file under an inaccessible root refuses the launch at the hardlink step, as before."
    anchor: "3-1-what-the-alias-scan-refuses"
  - id: "R-2"
    kind: requirement
    text: "A multiply linked file under a read-only root refuses the launch at the hardlink step when it shares its device and inode with a file under a writable directory, a writable file, or (macOS) an existing file an adjacent configuration prefix grants; otherwise it does not refuse."
    anchor: "3-1-what-the-alias-scan-refuses"
  - id: "R-3"
    kind: requirement
    text: "When no file the child may write has more than one link, the read-only roots are not walked."
    anchor: "3-2-what-the-child-may-write"
---

# 070: A read-only root refuses only a link the child could write

## 1. Purpose

Spec 004 section 3.18 rule 2 forbids access to the protected set "by any
spelling of a path", a hard link included. The boundary's alias scan enforces
that before launch by refusing any multiply linked file under any protected
root, inaccessible or read-only. Rule 2 makes the target's working tree and
its Git directory readable and not writable, so a second name for one of their
files gives the child nothing it lacks unless the child can write through that
name.

The scan's strictness refuses ordinary checkouts (issue #423). Cargo links its
build output under `target/` many times over (about 380,000 files in this
repository's), and Git shares object files with a local clone (1,681 of 3,180
in this repository's `.git/objects`). `doctor` and `run` refuse on any built
Rust checkout and in any worktree of a locally cloned repository. Spec 064
removed the same refusal for the operator's worktrees under the product home;
the checkout's own build directory and Git directory remained.

## 2. Territory

This spec amends the alias scan of spec 004 section 3.18 rule 2. Its
implementation changes `crates/statecraft-adapter/src/boundary/mod.rs`, which
spec 004 owns, and adds tests to `crates/statecraft-cli/tests/protected_boundary.rs`.

## 3. Behavior

### 3.1 What the alias scan refuses

Under an inaccessible root (the product home's protected entries and the launch
records), a multiply linked file refuses the launch at the `hardlink` step, as
before; reading through any second name would be a read rule 2 forbids.

Under a read-only root, a multiply linked file refuses at the `hardlink` step
when it is the same file, by device and inode, as a file the child may write
(section 3.2). A multiply linked file whose other names the child may only read
refuses nothing: reading it is already allowed.

### 3.2 What the child may write

The files the child may write are those under a writable directory grant, each
writable file grant, and, on macOS, each existing file in a configuration
prefix's parent directory whose name begins with that literal prefix. The scan
gathers the device and inode of every multiply linked one before it walks the
read-only roots. When there are none, no read-only file can share one, and the
read-only roots are not walked.

### 3.3 What does not change

Rule 2's protected set, rule 3's writable roots, the refusal of a writable root
that covers a protected one, and the runtime denial of a new hard link by the
platform mechanism are unchanged. This section changes only which existing
links refuse before launch.

## 4. Out of scope

- Build output under another attempt's workspace when that workspace lies in an
  inaccessible root.
- The cost of walking a writable root that holds a large build directory: it is
  walked once per launch, and the read-only walk follows only when it holds
  multiply linked files.

## 5. Resolved decisions

**2026-11-06: identity, not location.** The scan cannot list a file's other
names, so "another name lies where the child may write" is decided by comparing
device and inode with the multiply linked files under the writable roots.
Exempting a named build directory, as spec 064 did for the operator's
worktrees, would leave Git's shared objects refusing and would fail open on a
link planted inside the exempted directory.

## Verification

Ratified by the owner on 2026-11-08. Acceptance is run through
`make verify SPEC=070`, against fixture trees and the
platform boundary, with no provider.

```verify:cli
cargo test -p statecraft-cli --test protected_boundary
```

These show that cargo-style links inside a read-only root and an object linked
to a sibling clone are admitted while the writable root holds its own linked
files, that a read-only file linked into a writable directory or to a writable
file grant refuses, and that a link under an inaccessible root still refuses.
