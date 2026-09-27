# Archived implementation-completion policy from spec 002

Status: historical and nonnormative.

Source: `specs/002-environment-lifecycle/spec.md` at base commit
`d77e011e39dbc4689e49492c4e528d860be7eb86`. Archived on 2026-09-26 after
responsibilities moved to specs 008, 009, and 015. It is retained for
provenance and does not define current lifecycle or requirements.

### 3.36 When this spec's implementation is complete

A narrowly scoped authority amendment, settled by the owner on 2026-09-23 and
recorded before it is applied. Since section 3.24 moved `implementation` from
`complete` to `in-progress`, no section has said what would move it back, so
the field has been read either as waiting on a live result no local work can
supply or as ready to flip because the product fails closed. This section
fixes the rule. It changes no requirement in sections 3.1 to 3.35.

**Rule 1: the facts, kept apart.** *Implemented* is a property of this
repository's code measured against sections 3.1 to 3.35. The provider evidence
is not one fact but several. Section 3.32 rule 28 keeps installed, selected,
supplied, correlated, admitted (the startup decision of its rule 26) and
qualified apart. This section adds two words of its own: a session was
**observed** (section 3.29's three controls were captured), and an observation
was **observation-admitted** (sections 3.29 and 3.30 accepted it). Neither kind
of admission is a qualification, and neither implies the other. An
observation-admitted observation is not the verdict `qualified` of section 3.31
rule 21: it makes no run, trial or session qualified, and this section treats
it only as evidence under section 3.27. Where section 3.29 says an observation
"qualifies", it speaks of the floor payload for the invocation the observation
is bound to, and this section does not read that as a session's qualification.
*Activation* is this product acting on a real home. *Counterparty completion*
is what section 3.22 leaves to another repository. Only the first is what the
frontmatter's `implementation` field records; the others are reported beside
it, each on its own evidence, and none is inferred from another. None of them grants the
field, and none withholds it except through a requirement of sections 3.1 to
3.35 that mandates its result (rule 5).

**Rule 2: every normative requirement is accounted for.** `implementation:
complete` requires that, on one revision of `main`, every requirement of
sections 3.1 to 3.35 is in exactly one of these classes, and nothing is in
none:

| Class | What it needs |
|---|---|
| implemented | Reachable through this product's command surface where the requirement is about the product's behavior, and exercised by a named test or declared acceptance command. Code no verb reaches is **not** this class. |
| deferred | Deferred by name in section 4, or by an adopted deferral row of the decision record (an `F-` row, not a proposed one), as either stood before this section; or by a scope change the owner approved. That approval names the requirement by section and rule, says why, and is the owner's own act (a review approval, a commit, or a dated statement), cited in the section 5 entry that records it. A dated section 5 entry an agent writes records that approval and never substitutes for it. A requirement is not deferred by being hard. |
| external | Decidable only by a provider session, a real home, a release, or another repository's act, **and** the requirement does not mandate a particular result. Its current disposition is recorded, whatever it is. A requirement that mandates a result and depends on such an act is accounted for only when that result is established; until then it is unresolved and blocks `complete` (rule 5). |

**Rule 2a: a frozen obligation is never deferred or external.** Each
principle spec `000` freezes is accounted for, in this spec's territory, as
implemented, whether or not a section of 3.1 to 3.35 restates it; that
includes Constitution IX's placement of records where the supervised process
cannot reach them. It is not deferred by any route in rule 2, including a
decision-record deferral such as `F-09`'s of operating-system enforcement; it
is not external because a platform or an operator's act would be needed to
meet it; and recording it as a residual does not account for it. Until it is
implemented it is unresolved and blocks `complete`.

**Rule 3: failing closed is not the same as implemented.** A requirement that
the product *do* something is not satisfied by the product reporting that it
cannot. A requirement that the product *refuse* or *report* something is
satisfied by the refusal or the report. A requirement that makes the product's
reliance on a mechanism conditional on evidence is satisfied only by that
evidence or by the product demonstrably not relying on the mechanism; where
another requirement of these sections itself relies on the mechanism, the
second reading is not available.

**Rule 4: the producer and acceptance.** The pinned producer is the published
crate, resolving with no Git or path override, and conforms under section 3.15
on that revision. Every spec's declared acceptance passes there, run serially,
with every ignored, skipped or unavailable check named. A skipped or
unavailable check that is the only evidence for a requirement leaves that
requirement unaccounted for under rule 2.

**Rule 5: experiments need a disposition; a mandated result needs the result.**
Every live experiment the owner authorized has a recorded disposition
(established, unverified, not admitted, or not run with the reason), with its
evidence kept and its original verdict never rewritten. A positive result is
required where a requirement of sections 3.1 to 3.35 mandates the result
itself rather than the procedure that measures it. Section 3.29 rule 6 is not
such a requirement: it makes `unverified` the correct answer when evidence
cannot decide. **Section 3.27 is one.** It requires that "the installed version
and the effective behavior are verified before this product relies on the
mechanism, and an unverified mechanism is an unavailable one", and section
3.32 rule 25 relies on that mechanism to deliver a managed run's settings
document: the deny floor, the startup hook and the admission gate. So section
3.27 is unresolved, and blocks `complete`, until the behavior a run relies on
is established for the installed provider version: the deny floor through an
admitted observation of sections 3.29 and 3.30, and hooks supplied through
`--settings` through an established trial of section 3.33. Evidence for one
version is not evidence for another, and a matching version string is not by
itself the same provider. The evaluation names the installed binary by path
and digest and shows that each piece of evidence recorded that digest; evidence
that does not record the binary's digest cannot be bound under this rule and
does not count, and recording it on the run and trial paths is a change to
sections 3.31 and 3.33. The deny floor a run supplies, digested alone as
section 3.32 rule 25 records it, must equal the payload the observation is
bound to under section 3.29 rule 4, and the trial's startup-hook and gate
registrations must match the run's by event, matcher and script digest.
Section 3.32 rule 25 says a floor-only observation is not evidence for a run's
whole document; whether it is enough for the floor inside that document is the
owner's decision, and until the owner makes it, section 3.27 is unresolved.
What this rule requires is the observation's admission and the established
trial; it does not require that any session be `qualified`, which section 3.31
rule 21 makes unreachable through a run (restated in section 3.32 rule 28). The alternative, a `run` that treats the
mechanism as unavailable for an unverified version, would be a change to
section 3.32 and is the owner's to make; this section does not make it.

**Rule 6: how it is applied.** The evaluation is a dated section 5 entry
naming the revision, the count of implemented requirements, every requirement
that is deferred or external with its class and evidence, each frozen
obligation of rule 2a with its evidence, and a statement that no requirement is
unresolved. The field then moves in its own lifecycle-only change, which is the
owner's act; this section makes it so. A requirement later found
unmet, or a new requirement, moves it back to `in-progress` in the change that
finds or adds it.

