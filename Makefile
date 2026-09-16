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
# There is no build, test, fmt or clippy target: this repository holds no code.
# Adding one is a change to whichever spec claims the code it would build.

SPEC_SPINE ?= spec-spine

# The same resolution order the push gate uses: an exported default branch, then
# the remote's own HEAD, then main. An explicit BASE= on the command line wins.
SPEC_SPINE_DEFAULT_BRANCH ?= $(shell git symbolic-ref --short refs/remotes/origin/HEAD 2>/dev/null | sed 's|^origin/||')
BASE ?= origin/$(or $(SPEC_SPINE_DEFAULT_BRANCH),main)

.PHONY: gate refresh verify couple status help

## The whole check surface, read-only throughout. A gate that writes repairs what
## it is meant to judge, so this uses `check` and never `compile`.
##
## Two flags the generic spec-spine kit carries are deliberately absent, because
## on a code-free corpus each refuses this repository's own correct state:
##   index coverage --fail-on-untraced  refuses an empty universe rather than
##       passing vacuously. It joins the gate with the first source file.
##   index check --fail-on-unresolved   refuses a forward claim, which is exactly
##       what specs 002 to 005 are. It joins the gate when this repository builds
##       what it claims within one pull request.
## AGENTS.md carries the same two notes, so neither is a silent omission.
gate:
	$(SPEC_SPINE) check --fail-on-warn
	$(SPEC_SPINE) lint --fail-on-warn
	scripts/check-authored-content.sh

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
	@echo "gate     the read-only check surface: check, lint, authored content"
	@echo "refresh  recompute the committed shard trees"
	@echo "verify   SPEC=<id>, one spec's declared acceptance"
	@echo "couple   the coupling gate against BASE ($(BASE)); CI-only, compares commits"
	@echo "status   version, lifecycle counts, schedulable set"
