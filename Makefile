# The check surface for this repository. Two variables, both overridable:
#
#   SPEC_SPINE  the binary to govern with. The pin in spec-spine.toml
#               (=0.18.0) is checked by the binary itself on every run.
#   BASE        the ref the coupling gate compares against, resolved from the
#               repository rather than assumed to be origin/main.
#
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

SPEC_SPINE ?= spec-spine

# The same resolution order the push gate uses: an exported default branch, then
# the remote's own HEAD, then main. An explicit BASE= on the command line wins.
SPEC_SPINE_DEFAULT_BRANCH ?= $(shell git symbolic-ref --short refs/remotes/origin/HEAD 2>/dev/null | sed 's|^origin/||')
BASE ?= origin/$(or $(SPEC_SPINE_DEFAULT_BRANCH),main)

.PHONY: gate code build test clippy fmt refresh verify couple status help

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
## 13/13 specifically claimed instead of reporting the number.
##
## One flag the generic spec-spine kit carries is still deliberately absent:
##   index check --fail-on-unresolved   refuses a forward claim. One is left:
##       006 claims crates/statecraft-cli/, which is what a draft proposal is
##       for. The flag joins the gate when that last claim is built.
## AGENTS.md carries the same note, so it is not a silent omission.
gate:
	$(SPEC_SPINE) check --fail-on-warn
	$(SPEC_SPINE) lint --fail-on-warn
	$(SPEC_SPINE) index coverage --fail-on-untraced
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

## One spec's declared acceptance (spec-spine 049). Deliberately not part of
## `gate`, because it runs code the corpus declares. Specs 002 to 005 declare
## none, which is honest for an unimplemented spec.
verify:
	@test -n "$(SPEC)" || { echo "usage: make verify SPEC=<id>"; exit 3; }
	$(SPEC_SPINE) verify $(SPEC)

## The coupling gate. CI-only by design: it compares two COMMITS, so it cannot see
## a change being staged and is useless as a pre-commit check. This target is for
## reproducing a CI verdict locally, against a commit. Meaningless today (no code
## to drift), correct from the first crate.
couple:
	$(SPEC_SPINE) couple --base $(BASE) --head HEAD

## What the corpus currently holds, and what is schedulable.
status:
	$(SPEC_SPINE) --version
	$(SPEC_SPINE) registry status-report --nonzero-only
	$(SPEC_SPINE) registry plan

help:
	@echo "gate     the corpus check surface: check, lint, authored content"
	@echo "code     the workspace check surface: build, test, clippy, fmt"
	@echo "refresh  recompute the committed shard trees"
	@echo "verify   SPEC=<id>, one spec's declared acceptance"
	@echo "couple   the coupling gate against BASE ($(BASE)); CI-only, compares commits"
	@echo "status   version, lifecycle counts, schedulable set"
