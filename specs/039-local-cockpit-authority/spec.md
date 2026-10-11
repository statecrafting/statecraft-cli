---
id: "039-local-cockpit-authority"
title: "Reopen F-04 for a local same-origin cockpit over the existing operations"
status: approved
implementation: n-a
created: "2026-10-10"
summary: >
  Amends 001's interface exclusion and reopens founding deferral F-04 only
  for Statecraft Dev: a one-repository loopback browser cockpit, calling the
  same typed local operations under 002 section 3.20. This is an authority
  decision, separate from the application extraction, server, distribution
  and command implementations. Ratification grants no production signing,
  release, hosted panel, daemon or detached execution authority.
amends:
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
depends_on:
  - "001-boundaries-and-authority"
  - "002-environment-lifecycle"
  - "007-family-envelope"
obligations:
  - id: "R-1"
    kind: requirement
    text: "F-04 is reopened only for the one-repository loopback same-origin cockpit; all other interface scope stays deferred."
    anchor: "3-1-the-narrow-exception"
  - id: "R-2"
    kind: requirement
    text: "The cockpit calls the existing typed operations, preserves the family envelope and authoritative records, and introduces no second execution owner or detached run."
    anchor: "3-2-the-binding-boundary"
  - id: "R-3"
    kind: requirement
    text: "Each implementation slice requires its own ratified spec and named positive and negative acceptance, carrying the server session, origin, request, and path security controls and the signed, rooted, rollback-resistant, atomic, offline distribution requirements in section 3.3; this decision authorizes no implementation by itself."
    anchor: "3-3-separate-implementation-authority"
---

# 039: Local cockpit authority

## 1. Purpose

F-04 in `docs/decisions/00-founding-decisions.md` defers every interface beyond
command output, including a read-only web view. Spec 001 section 3.7 excludes
recovery of the web UI and section 4 excludes a rich user interface. Spec 002
section 3.20 already defines the operation boundary a dashboard would use but
explicitly leaves F-04 standing. A server implementation cannot lift those
exclusions for itself.

The reviewable-outcome need is to inspect the relationships between specs,
work, attempts, recovery and evidence while local execution proceeds, with
each finding linked to its authoritative report or record. This is the narrow
need proposed as the basis for reopening F-04, not a claim that any interface
has been implemented or tested.

The source direction is the proposed platform design record
`statecraft:8fa73bc:docs/design/07-statecraft-dev-local-cockpit.md`, sections
2, 5, 7, 8, 10 and 12. Its CLI references were checked against `f98f9f0`,
which remains the source revision of this proposal. This spec carries the
authority decision locally. The external proposal has no independent normative
authority: section 3.3 locally requires its cited scenarios to be restated as
acceptance criteria in each applicable implementation spec, then ratified there.
Platform PR 28 squash-merged this direction as `8fa73bc`. Platform direction
P-25 is recorded alongside it in
`statecraft:8fa73bc:docs/decisions/00-adoption-register.md`.

## 2. Territory

This spec owns no code and changes no gate, acceptance runner, trust root or
environment manifest. The `amends` edges change 001's interface boundary and
002 section 3.20's statement that F-04 stands, only to the extent below.
The historical founding record and the amended specs remain unedited.

## 3. Behavior

### 3.1 The narrow exception

Upon owner ratification, F-04 is reopened only for `statecraft dev`, a local
browser cockpit serving one repository through one loopback, same-origin
server. The web UI exclusion in 001 section 3.7 and the rich interface exclusion
in section 4 permit exactly this cockpit, subject to section 3.2. Section
3.20 of 002 no longer withholds permission for this bounded interface.
Other interface surfaces remain deferred.

This permits later specs to propose local queries, subscriptions and governed
commands, an embedded dashboard and independently versioned verified bundles.
It grants no exception for a hosted control plane, platform panel, multiple
repositories, interactive terminals, preview channel, persisted read model,
background daemon, detached run or cloud synchronization. F-01, F-02 and F-03
are not lifted by this decision. In particular, describing a distribution
pipeline is separate from authorizing its deployment or production release.

### 3.2 The binding boundary

Spec 002 section 3.20 remains binding: the cockpit is a second caller of the
same typed operations, with no second settings engine, scheduler or policy
model. HTTP domain responses use the unchanged 007 envelope. Queries do not
write. Commands undergo the verb's validation, authorization and admission.
Capability discovery describes support and grants nothing.

Repository access, spec evaluation, execution, acceptance and recovery remain
local. The browser uploads nothing and makes no cross-origin operational call.
Operational authority is the repository's existing hash-linked record chain;
specification authority is spec-spine's structured reports. Derived views and
buffers are reconstructible and never replace either authority. Identifiers
carry repository identity; cursors carry chain position and head hash, kept
separate from specification closure and index digests and from Git revisions.

One run has one execution owner. A terminal-started run is observed only. A
dashboard-started run is owned by the `dev` process, disclosed by the UI, and
interrupting that process follows the verb's supervision and recovery contract.
Closing a tab or subscription changes no durable state. There is no detached
execution or background work-driving loop.

The interface preserves refusal before launch, process end, unknown outcome,
acceptance, qualification and changed contract closure as distinct states.
Absence of evidence never becomes success. Artifact integrity and operation
authorization are separate checks. A provisioned compatible installation works
offline; host unavailability cannot block a verified installed release or an
ordinary verb. CLI, protocol and dashboard versions remain separate.

### 3.3 Separate implementation authority

After this decision, the sequence is separately ratified specs for application
operations, the read-only server and protocol, dashboard distribution, then
governed commands. Each draft carries its requirements locally, including the
applicable invariants and every in-scope positive and negative scenario from
the design record's section 12 as named tested acceptance criteria. A draft
does not grant permission to implement itself.

The server spec must enforce loopback binding, exact Host and Origin checks,
a short-lived single-use fragment launch token, a lifetime-bound HttpOnly
SameSite=Strict session cookie, CSRF for writes, restrictive same-origin CSP,
bounded requests and rates, and no arbitrary-path read. The distribution spec
must define exact-byte signatures, rooted trust, rollback refusal, verified
staging and atomic activation, and the zero-network offline path. Production
keys and signing remain blocked on the separate 035 ceremony performed by the
owner. This decision neither performs that ceremony nor claims a release.

## 4. Out of scope

Implementation, crate additions, dependency changes, frontend copying,
publication infrastructure, production signing, release and deployment.
The platform repository and archived frontend are read-only sources.

## 5. Resolved decisions

Bart ratified this spec as written on 2026-10-10 with the instruction:
"I, Bart, ratify it; set `status: approved`." The approved status records
that owner act. Review and command results do not confer ratification.
Changes to approved requirements require a separately ratified amending spec
under 001 section 5; the corpus checks below do not replace that owner act.

The amendment is its own authority change. Keeping the historical F-04 row
and approved specs unchanged preserves provenance; the amendment graph records
the ratified exception. `implementation: n-a`
means there is no code to implement in this decision and does not assert that
Statecraft Dev exists.

## Verification

These checks validate the proposal's corpus shape and amendment relationships.
They are not behavioral acceptance for a server, browser or distribution path.
Owner ratification is separate from all command results.

```verify:cli
spec-spine check --fail-on-warn
spec-spine lint --fail-on-warn
spec-spine registry relationships 039-local-cockpit-authority
scripts/check-spec-corpus.sh
scripts/check-authored-content.sh
```
