# CLI fixtures, emitted by the CLI

Each file is JSON **emitted by `statecraft-acceptance`'s own serializers** at
`8f6591f`, the commit before the shared types moved into this crate.
They are not hand-derived. The case list that produced them is
`tests/legacy_cli/mod.rs`, and `tests/cli_compat.rs` asserts that the frozen
transcription of that serializer still writes each of these files byte for
byte, so the provenance is reproducible rather than asserted.

The eight fixtures that came from the platform's staging commit (`d7dc24b`,
hand-derived from the CLI's serde attributes) were compared against the emitted
bytes before they were replaced: all eight were identical. The hand derivation
was right; it is simply no longer the evidence.

Three properties are checked, and they are different properties:

1. **Provenance**: each file is what the CLI's serializer writes.
2. **Byte preservation**: each file deserializes into the shared type and
   re-serializes to identical bytes, member order included.
3. **Semantic interpretation**: the value means what the CLI meant it to mean.
   A fixture that round-trips can still have been read wrong, which is exactly
   what happened to `harness_revision` before the transfer, so every fixture is
   also read for its meaning.

| File | What it pins |
|---|---|
| `reference-file-bytes.json` | a `file-bytes-sha256` reference, no optional field set |
| `reference-canonical-embedded.json` | the versioned canonical construction, plus `embedded` |
| `dimensions-unsigned-today.json` | what this product reports today: `unsigned`, `unknown` |
| `dimensions-all-unknown.json` | every check unperformed |
| `dimensions-all-pass.json` | all four dimensions at `pass` |
| `admission-admit.json` | the admit arm under the `admission` tag |
| `admission-refuse-dimension.json` | `required-dimension-not-passed`, and no `reasons` array |
| `admission-refuse-incomplete.json` | `incomplete-evidence`, naming what was missing |
| `policy-strict.json` | the three CLI booleans, every added field absent |
| `policy-permissive.json` | the same three, all false: absent is not the same as false |
| `recorded-forms.json` | a present value and all three absences |
| `recorded-collision-legacy.json` | the ambiguous bytes a pre-transfer build could write |
| `receipt-not-recorded.json` | a receipt whose `harness_revision` is an absence |
| `receipt-harness-present.json` | a receipt whose `harness_revision` is a value |
| `receipt-legacy-no-authority-paths.json` | a receipt written before a field existed |

Adding a fixture means adding the case that emits it to `tests/legacy_cli`; a
file with no case is refused by `no_fixture_is_committed_without_the_case_that_produced_it`.
