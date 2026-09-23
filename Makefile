# The check surface for this repository. Two variables, both overridable:
#
#   SPEC_SPINE  the binary to govern with. It resolves to the repository-local
#               .tooling/bin/spec-spine when that exists, and only otherwise to
#               whatever is on PATH. The pin in spec-spine.toml is checked by
#               the binary itself on every run.
#   BASE        the ref the coupling gate compares against, resolved from the
#               repository rather than assumed to be origin/main.
#
#   make tools           install the pinned spec-spine into .tooling/bin
#   make gate            read-only: everything CI runs, in order
#   make refresh         writing: recompute the committed shard trees
#   make verify SPEC=001 one spec's declared acceptance
#   make couple          the coupling gate against BASE
#
#   make code            build, test, clippy and fmt across the workspace
#
# `gate` and `code` are two surfaces, not one. `gate` judges the corpus and is
# meaningful with no code at all; `code` judges the workspace and is inert until
# a crate exists. CI runs both as separate jobs and requires both through
# `ci-gate`, which is why neither is nested inside the other.

# The pinned version, read from the single place it is authored. Nothing here
# repeats the number: a second spelling is how a pin and its installer drift.
# spec-spine.toml is authored configuration, not compiler output, so reading it
# with sed is not a governed-artifact read.
SPEC_SPINE_VERSION := $(shell sed -n 's/^required_version = "=\(.*\)"/\1/p' spec-spine.toml)

# A repository-local install, not a shared one. `~/.cargo/bin/spec-spine` is a
# single binary every project on this machine shares, so whichever project built
# it last answers for all of them; this repository was measurably governed by the
# wrong version that way. The local copy is gitignored, installed by `make tools`
# at the exact pinned version, and preferred automatically when present.
SPEC_SPINE_LOCAL := .tooling/bin/spec-spine
SPEC_SPINE ?= $(if $(wildcard $(SPEC_SPINE_LOCAL)),$(SPEC_SPINE_LOCAL),spec-spine)

# The same resolution order the push gate uses: an exported default branch, then
# the remote's own HEAD, then main. An explicit BASE= on the command line wins.
SPEC_SPINE_DEFAULT_BRANCH ?= $(shell git symbolic-ref --short refs/remotes/origin/HEAD 2>/dev/null | sed 's|^origin/||')
BASE ?= origin/$(or $(SPEC_SPINE_DEFAULT_BRANCH),main)

.PHONY: tools gate code build test clippy fmt refresh verify couple status help

# Every `cargo --workspace` verb refuses a virtual manifest with no members, so
# the Rust targets are guarded on a crate existing rather than simply run. The
# guard is a wildcard over the workspace's own member glob, so it goes live with
# the first crate and needs no edit to do it.
CRATE_MANIFESTS := $(wildcard crates/*/Cargo.toml)
SKIP_NOTE := no crate exists yet, so the workspace has no members and cargo has nothing to judge

## The whole check surface, read-only throughout. A gate that writes repairs what
## it is meant to judge, so this uses `check` and never `compile`.
##
## `index coverage --fail-on-untraced` joined this list with the first source
## file, which is the condition AGENTS.md recorded for it. On a code-free tree it
## refused an empty universe rather than passing vacuously; now it defends
## every claimed file instead of reporting a number.
##
## `index check --fail-on-unresolved` joined this list when 006 built the last
## forward claim. Both flags the generic spec-spine kit carries are now present,
## and each arrived on the condition recorded for it rather than on a whim:
## coverage with the first source file, unresolved with the last unbuilt claim.
## A new spec claiming a crate it has not written yet will now fail the gate,
## which is the intended cost of having none outstanding.
## Install the pinned spec-spine into .tooling/bin. Idempotent: `cargo install`
## is a no-op when the same version is already there, so CI and a local session
## run the same line. --locked builds spec-spine's own lockfile rather than a
## freshly resolved one, so two installs of one version are the same binary.
tools:
	@test -n "$(SPEC_SPINE_VERSION)" || { echo "no required_version in spec-spine.toml"; exit 3; }
	cargo install spec-spine-cli --version $(SPEC_SPINE_VERSION) --locked --root .tooling
	$(SPEC_SPINE_LOCAL) --version

gate:
	@echo "governing with: $(SPEC_SPINE) (pin =$(SPEC_SPINE_VERSION))"
	$(SPEC_SPINE) check --fail-on-warn
	$(SPEC_SPINE) lint --fail-on-warn
	$(SPEC_SPINE) index coverage --fail-on-untraced
	$(SPEC_SPINE) index check --fail-on-unresolved
	scripts/check-authored-content.sh

## The Rust half of the check surface, in the order that fails fastest.
##
## The guard is one shell per recipe LINE, so it has to be one `if` rather than
## a `test ... || exit 0` followed by the command: the early exit would end only
## its own line and make would run the next one anyway. That bug is why the
## first draft of these targets ran cargo against a workspace with no members.
code: build test clippy fmt

build:
	@if [ -z "$(CRATE_MANIFESTS)" ]; then echo "$(SKIP_NOTE)"; else set -x; cargo build --workspace --locked; fi

test:
	@if [ -z "$(CRATE_MANIFESTS)" ]; then echo "$(SKIP_NOTE)"; else set -x; cargo test --workspace --locked; fi

clippy:
	@if [ -z "$(CRATE_MANIFESTS)" ]; then echo "$(SKIP_NOTE)"; else set -x; cargo clippy --workspace --all-targets --locked -- -D warnings; fi

fmt:
	@if [ -z "$(CRATE_MANIFESTS)" ]; then echo "$(SKIP_NOTE)"; else set -x; cargo fmt --all --check; fi

## The writing half, for a session that has edited a spec and can commit the
## regenerated shards with the change that made them stale. Never run this
## immediately before `gate` to make it pass: that proves nothing about what the
## branch committed.
refresh:
	$(SPEC_SPINE) compile
	$(SPEC_SPINE) index

## One spec's declared acceptance (spec-spine 043). Deliberately not part of
## `gate`, because it runs code the corpus declares, and not run in CI.
##
## A declared command that names `spec-spine` resolves it on PATH, the same way
## any command does. So the repository-local directory is put first on PATH for
## the whole run: without it, `make verify` judged a corpus pinned =0.20.0 with
## the shared ~/.cargo/bin copy, which refuses the pin with exit 3. Measured on
## 2026-09-23: specs 000, 002, 003, 005 and 006 failed that way and passed with
## the local copy first.
verify:
	@test -n "$(SPEC)" || { echo "usage: make verify SPEC=<id>"; exit 3; }
	PATH="$(CURDIR)/$(dir $(SPEC_SPINE_LOCAL)):$$PATH" $(SPEC_SPINE) verify $(SPEC)

## The coupling gate, over two COMMITS. **Commit first.** Run against BASE while
## HEAD is still BASE and the diff is empty: the gate reports "0 path(s)
## checked, no drift" and exits 0, which reads exactly like a pass and proves
## nothing. Measured here on 2026-09-17, on a staged-but-uncommitted tree that
## CI then refused for two real C-001 violations.
##
## This target reproduces a verdict locally;
## the authoritative one is what CI recorded against the pull request's own
## frozen endpoints, which a later run on merged main cannot reconstruct.
##
## 0.20.0 adds `--include-uncommitted` (spec 081), which unions `git diff HEAD`
## into the range so a pre-commit run judges the change being committed. It is
## off here and off in CI, deliberately: CI judges a pushed range, where the
## working tree is irrelevant and must stay so. Wiring it into a commit-boundary
## hook is its own change, and this repository has adopted no hook.
couple:
	$(SPEC_SPINE) couple --base $(BASE) --head HEAD

## What the corpus currently holds, and what is schedulable.
status:
	$(SPEC_SPINE) --version
	$(SPEC_SPINE) registry status-report --nonzero-only
	$(SPEC_SPINE) registry plan

help:
	@echo "tools    install the pinned spec-spine ($(SPEC_SPINE_VERSION)) into .tooling/bin"
	@echo "gate     the corpus check surface: check, lint, authored content"
	@echo "code     the workspace check surface: build, test, clippy, fmt"
	@echo "refresh  recompute the committed shard trees"
	@echo "verify   SPEC=<id>, one spec's declared acceptance"
	@echo "couple   the coupling gate against BASE ($(BASE)); compares two commits"
	@echo "status   version, lifecycle counts, schedulable set"
