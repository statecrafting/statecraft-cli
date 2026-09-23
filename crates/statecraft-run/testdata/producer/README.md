# Recorded producer reports

The exact bytes two spec-spine builds answered for `registry plan --json` and
`registry list --json`, read on 2026-09-22 against **one authored corpus**:
this repository at `22dc1559db6d09488e3bbbc423567c3be51f2450`. They are the
measurement behind spec 003 section 3.1.2, and `report.rs`'s tests read them
as the producer wrote them. Nothing here was edited; a test that needs a
contradiction mutates a copy in memory.

| Directory | Producer | Identity | Ledger read |
|---|---|---|---|
| `released-0.20.0/` | spec-spine 0.20.0, the pinned CLI (`spec-spine.toml` `required_version = "=0.20.0"`) | tag `v0.20.0`, commit `4d14cce6dd151073f02c0c657c52c5c5c4d0bca8`; binary SHA-256 `9c18e8816847ffac67551c928c7e22e432ce84260f12692d4f5efd185652d8fa` as installed by `make tools` | the committed ledger at `22dc155` |
| `expansion-3b67b63d/` | spec-spine `main`, **unreleased**, self-reported `0.22.0` | commit `3b67b63d48965fefce6732d1cc865d861ee9928b`, built with `cargo build --release --locked -p spec-spine-cli`; binary SHA-256 `ea0dee7b09bbd08c536db3c1b795b343f3c5a1dcb4dc3aaead6e8fead73fb731` | a scratch copy of `22dc155`, repinned to `=0.22.0` and compiled and indexed **by that build**, so every read came from that producer's own state |

The version string does not identify the expansion build: the producer's own
0.22.0 release candidate, cut from another commit, reports the same string.
The commit and the binary digest do.

Two differences the tests rest on: the expansion build's `ready` rows carry
`status` and the released build's do not, and neither carries
`implementation`, which is why the join with `registry list` stays.
