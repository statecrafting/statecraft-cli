<!-- Historical record only. Not normative. -->
<!-- Moved from specs/005-acceptance-and-evidence/spec.md at base d77e011e39dbc4689e49492c4e528d860be7eb86. -->

# Archived implementation journal: 005-acceptance-and-evidence

This file preserves the chronological implementation record that formerly
occupied section 5 of the active spec. Current requirements live in the
active spec's behavior section, and current rationale lives in its bounded
`Resolved decisions` section. Git remains the authoritative change history.


Dated entries for choices §3 was silent on. None changes what it requires. The
entries concerning the envelope were recorded against `007` while that spec was
separate; they are kept verbatim, because this section is a history and a
history is corrected by appending.

**2026-09-17: the report is read, and what that decided.** The owner authorized
the 088 integration as an authority change on its own, which is the authority the
entry below said this section did not have. Four choices §3 was silent on were
made in the course of it, and each is here rather than in a commit message
because each is a choice a reviewer could reasonably have made differently.

*The class-to-member mapping is fixed in the spec, not in the code.* 088's eleven
classes are structural and are not this product's member names, so somebody had
to say which witnesses which. Leaving it to the implementation would have put an
authority decision in a match arm. §3.3.2 is that mapping, and every member it
names is one `001` §3.5 already enumerates: the integration reads a new answer,
it does not widen the set the answer is about.

*`policy` is read against the path, not the class alone.* The first mapping
written here was path-blind, and it was wrong in a way that would not have shown
up in a test: this repository hashes `README.md`, `AGENTS.md`, `docs/**` and
`.claude/rules/**`, so every one of them arrives classed `policy`, and a blind
reading would have made a README edit an authority change and made the authority
set extendable by editing a list in `spec-spine.toml`. §3.3.2 splits the class:
the base's own configuration is corpus-side, and every other hashed input keeps
its membership where case 2 put it, which is here, by path.

*`requirement` is not a member, and that is the load-bearing half.* Including it
would have been the conservative-looking choice and would have made the
integration inoperative: the coupling gate puts an owning `spec.md` in nearly
every diff, so every candidate would have been an authority change and nothing
would ever rest on its own suite. The candidate that weakens the acceptance
judging it is not lost by the exclusion; it arrives as `verification`, which is a
member. `unknown` is the opposite case and is treated as no answer at all.

*The crate reads the report and does not run spec-spine.* The 2026-09-16 entry
below records that nothing in this crate acts, and that property is kept: the
bytes are handed in. It is not only tidiness. `001` §3.5 rule 1 reads every
authority-set member at the trusted base, and which binary classifies is part of
what must not come from the candidate, so §3.3.1 fixes the invocation contract
for the caller instead of burying a process spawn in the library that judges.

*A report about another change is refused.* §3.3.3 case 4 is not in 088 and is
not a doubt about it: the report answers about the diff it was given, and
checking that it covers the paths being judged is what stops a stale or
misaddressed report from being read as an answer about this candidate.

**2026-09-17: the corrected reason is attributed to this product, and is not a
new capability.** Section 3.3 named spec-spine 088 as carried by no release,
which the move to the `=0.20.0` pin falsified, and the falsehood had reached the
record: the corpus-side note this crate wrote said in so many words that the
installed spec-spine carries no change-classification report. Under the current
pin it does.

Two answers were available. Reading the report is what CLI-08 asks for and is a
capability this change does not add: it is its own change, and `AGENTS.md`
requires an authority change to be separated from the work it would authorize, so
bundling it here is the thing that rule forbids. The other is to say the true
thing about what this product does, which is that it does not read the report.
That is what the note now says, and it carries the installed version beside it so
a reader can see which spec-spine was asked and told nothing.

What did not change: the verdict is still `not-recorded`, acceptance is still
refused on the candidate's own suite, and this crate still builds no classifier
of its own. The prohibition in section 3.3 was never conditional on the report
being unreleased.

**2026-09-16: the vocabulary rules are types, not conventions.** §3.5 says
`unsigned` belongs only to `signature` and `not-applicable` only to
`subjectBinding`. Each dimension is therefore its own enum, so neither value is
constructible in the wrong place. A single shared value enum with a rule in
prose would have made both mistakes reachable, and the rules are described as
part of the vocabulary rather than as commentary on it.

**2026-09-16: `admission` is a separate type from the dimensions.** §3.5 keeps
it apart, and the reason is worth stating: a dimension says what a check found,
admission says what a policy decided about those findings. One type carrying
both would make "this evidence is intact" and "this evidence is acceptable" the
same sentence, which is exactly what §3.10's two-policy row exists to
distinguish.

**2026-09-16: `Recorded<T>` carries an absence rather than an `Option`.** §3.8
fixes three names for absence and says none reads as success. An `Option::None`
carries no name, so every optional field in a reported outcome is
`Recorded<T>`: present, or absent as one of the three. That is what makes "a
field a future contract will add reads `not-recorded`, never `none`, never
omitted" a property of the serialization rather than a rule somebody remembers.

**2026-09-16: the authority verdict refuses on `not-recorded`.** §3.3 says the
verdict reads `not-recorded` and that acceptance is still refused on the
candidate's own suite. Those are two statements, and the implementation makes
the second follow from the first: `may_accept_on_own_suite` is false whenever
the corpus-side answer is absent, not only when a member was touched. Refusing
without the report is available; classifying without it is not.

**2026-09-16: integrity is answered only under the construction the reference
names.** §3.7 forbids a canonical record hash substituting for a file-byte
digest. A reference under a construction this build cannot evaluate reports
`unknown` rather than falling back to a file-byte hash, because the fallback
would answer a different question and look like an answer to this one.

**2026-09-16: nothing in this crate acts.** §3.9 says publication is not part of
this spec and that no verb in this corpus publishes. The crate has no function
with an external effect: a receipt is a value, `mint` returns one, and there is
nothing to call that would do anything with it. That is the implementation of
"a receipt is not a permission".

**2026-09-16: re-export, not an adapter.** Section 3.1 says one owner and does
not say how the other side reaches it. A compatibility adapter between two
definitions was considered and rejected: it keeps both definitions alive, so it
keeps the divergence possible, and it has to be written in a direction. The cost
of the re-export is that `Reference`'s struct literal now needs three more
fields at its construction sites; the two in this repository use functional
update from `over_file_bytes` instead.

**2026-09-16: a refusal from `admit` names one reason and writes no `reasons`
array.** The envelope's `Admission::Refuse` gained a `reasons` list, skipped when
empty. This product's `admit` returns on the first hard refusal, so it fills
`reason` and leaves `reasons` empty, which is why its output is byte for byte
what it was. `Admission::reasons()` reads either shape back as the list it is, so
no reader has to know which producer wrote the value.

**2026-09-16: the frozen encoder is a test fixture, not a second implementation.**
`tests/legacy_cli/` looks like the duplication this spec exists to remove. It is
the opposite: the fixtures are only evidence if something that is not the shared
types can still produce them, and after the re-export the acceptance crate no
longer can. It is never edited to make a test pass.

**2026-09-16: the reserved-word check is a trait, not a serialization probe.**
Deciding whether a present value collides could be done by serializing it to a
JSON value and looking. A `Recordable` trait with a defaulted method does it at
compile time instead, costs nothing at run time, and does not quietly make the
type JSON-only. The price is a one-line impl per payload type; `Statement` has
one.

**2026-09-16: the eight hand-derived fixtures were correct.** The platform's
staging commit derived eight fixtures by hand from this product's serde
attributes and labelled them as such. Emitting the same cases from the
serializer produced byte-identical output for all eight. The replacement is
about what the fixtures are evidence of, not about a defect in them.

**Open, for the owner, not decided here:**

- Whether `trust::RootSet` and `roots::RootSet` should converge, and in which
  direction. They answer different questions today and neither product reads the
  other's.
- Whether the shared structs should carry an extras map so an unknown field
  survives a round trip. It is a wire-compatibility improvement with an API cost,
  and it is not needed until a producer writes a field this build has not seen.
- Whether `VerifierRecord` (section 3.6) and the envelope's
  `EvidenceVerdict` verifier identity should be one type. They overlap in intent
  and not in shape.

**2026-09-22: section 3.18, recorded before implementation.** An attempt's
intent will carry the contract spec 003 section 3.1.3 binds, and `accept`
compares it with the producer's resolution now. A contract that changed, lost a
member or withdrew an obligation is no acceptance, reason `contract-moved`; a
stale ledger refuses before anything is judged; an unbound or unresolvable
contract reads `not-recorded` in the answer, and the receipt's bytes do not
change. No code changed with this entry.

**2026-09-22: section 3.18 implemented.** `contract::compare` is pure over
the binding and a resolution: it keys members as `spec:`, `section:` and
`obligation:` and quotes the identity the producer gave each, the content hash,
the section digest, or the obligation's own fields other than its key, so a
change is named rather than inferred. `contract::check` is the one operation
`accept` calls: it finds the attempt's intent, reads its binding, asks the
producer to resolve the same request only where the binding is `bound`, and
compares. `NoAcceptance::ContractMoved` carries the comparison. The receipt is
untouched, as rule 4 requires.

**2026-09-25: let chains collapsed with the measured rust-version floor.**
The workspace floor moved from 1.85, which never built, to 1.88 (evidence in
spec `002` section 5, same date). Clippy's `collapsible_if` then applies let
chains, and the nested `if` blocks it named in this spec's crates were
collapsed mechanically by `cargo clippy --fix` and `cargo fmt`. No behavior
changed.

**2026-09-25: the delta report reads both `--json` envelopes (owner,
2026-09-25: adopt 0.26.0).** spec-spine 0.26.0's verdict envelope (schema
1.0.0, its spec 132) removes `ok` and carries `outcome`; the delta report
inside it is unchanged at schema 0.1.0. `Envelope` required `ok`, so every
0.26.0 delta would have read as not an envelope. `Envelope::ok` and the new
`Envelope::outcome` are both optional, and a verb answered only when it exits
0 and its envelope says so in either form; one that says neither did not
answer. `VerbDidNotAnswer` names what the envelope said (`ok=` or `outcome=`)
in place of a boolean. Section 3.18's comparison table names the producer's
stale code as 2, which is 0.25.0's; from 0.26.0 a stale ledger is 1, read by its words as
spec `003` section 5 records on this date. Tested against a verbatim 0.26.0
delta (`testdata/delta/spec-spine-0.26.0-pin-move.json`) and in
`both_envelopes_are_read_and_one_that_says_neither_is_refused`.

**2026-09-25: the delta report reads schema 0.2 (owner, 2026-09-25: adopt
0.27.0).** spec-spine 0.27.0 moves the delta report to schema 0.2.0 (its spec
142): a twelfth class, `relocation`, for a section moved between specs with a
proven `relocates` edge, and a `relocations` list. Both are additive, but on a
`0.x` line the minor is the breaking position, so this build read only `0.1.z`
and every 0.27.0 report would have been an absence under section 3.3.3 case 2.
`READS_DELTA_SCHEMAS` is now `0.1` and `0.2`. The `relocations` list is not
read. `relocation` is not in section 3.3.2's table, and this entry does not add
it: the table is a membership answer and is ratified text. A report that uses
it is therefore an absence under case 3, which is the refusal the section
already prescribes for a class this build cannot place. Placing it (a
relocation moves requirement text that `delta` has proven unchanged, so reading
it as `requirement`, no member, is the proposal) is the owner's decision. An
unresolved claim at the merge base is a `validation` error envelope with no
report under 0.27.0 (spec-spine's 145), where 0.26.0 gave a `stale` one; either
way it is not a report and the answer is an absence. Tested against a verbatim
0.27.0 delta (`testdata/delta/spec-spine-0.27.0-readme-edit.json`) in
`the_delta_report_reads_the_0_27_0_schema`,
`a_relocation_class_is_an_absence_until_the_table_places_it` and
`a_validation_error_envelope_is_not_a_report`.
