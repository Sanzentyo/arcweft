# Proof convergence: permissive typed save reader deletion

Date: 2026-07-27

Status: implementation and cut-specific validation complete

## Context

Proof Stage 7 ultimately requires one strict persisted-payload migration after
the accepted HIR and runtime assertion authority switches. The returned
Proof-concurrency `v6.1.1.4` archive remains the rejected SHA-256
`414f95f8ef4c5f3abcce163f0c9b01f124098f0bac856f174af09b5c1e7d564b`,
so this cut does not infer the missing final leaf schema or migrate assertion
payloads.

An independent provisional reader was nevertheless removable now.
`arcweft-save::decode_typed_json_save` used ordinary Serde decoding, which
silently ignored unknown fields. The runtime-driver already used the strict
reader, but the native player outer session envelope still used the permissive
entry point.

## Deleted authority

- removed the public permissive `decode_typed_json_save` function;
- moved the native player outer session envelope directly to the existing
  `decode_strict_typed_json_save` owner;
- added compile-fail evidence that downstream code cannot import the removed
  reader;
- retained no alias, renamed wrapper, dual reader, predecessor migration,
  source gate, or compatibility shim.

The one strict typed reader now rejects:

- unknown top-level fields;
- unknown fields in nested payloads;
- duplicate fields;
- trailing JSON payload values;
- trailing save-envelope data even when a caller supplies a general envelope
  option that would otherwise allow it;
- mismatched schema IDs and non-JSON codec IDs;
- future versions, and predecessor versions without an explicit migration.

The native player has direct evidence that its outer session envelope rejects
an unknown field before runtime-session import. Runtime-driver save tests retain
their existing atomic validation of nested fields and restored state.

## Validation

- the independently audited working change was Jujutsu change
  `tvoztlxpytvkotkvzzlsnnlwrlwxkssv` over parent `601fb326`;
- `cargo test -p arcweft-save --all-targets`: passed, including 11 typed JSON
  tests, 6 envelope/migration tests, and the compile-fail public API row;
- `cargo test -p arcweft-player-native --lib`: passed, 44 tests;
- `cargo test -p arcweft-runtime-driver --test awbc_product_session save_ --
  --nocapture`: passed, 13 tests;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `just test-tier2`: passed the complete MCP stdio/native capture and exact
  visual-golden suite;
- `just test-workspace`: all preceding workspace and compile-fail suites,
  including the new removed-reader API row, passed; the final
  `arcw_fixtures_check_run` suite retained only the pre-existing two `FsError`
  failures:
  `spec_should_pass/check/010_capability_fs_read.arcw` and
  `spec_should_pass/run/002_file_read_task.arcw`;
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-strict-typed-save-reader-deletion-2026-07-27`:
  scanned 3,699 files, 1,934 Rust files, and 902,299 Rust physical LOC; reported
  0 errors and 144 existing warnings. Exact measurements and dependency evidence
  are retained in the
  [structure audit](structure-audits/proof-strict-typed-save-reader-deletion-2026-07-27/violations.md).

The changed production owners are `arcweft-save/src/lib.rs` at 24,236 bytes /
767 physical LOC and `arcweft-player-native/src/scene_windowed.rs` at 41,283
bytes / 1,132 physical LOC. The latter remains below the 1,200-LOC production
warning threshold and gained only the schema-owned decode boundary plus its
test. The sole manifest change is a test-only `trybuild` dependency for typed
API-removal evidence; no production dependency edge or facade re-export was
added.

Because this public contract deletion spans the save crate and native runtime
consumer, the cut is treated as Tier 2 risk even though no assertion payload is
changed.

## Remaining boundary

This preparatory deletion is not Proof Stage 7 completion. Runtime assertion
inventory, guarded AWBC encoding, bundle/cache propagation, and final
save/checkpoint/replay identity still follow the corrected HIR leaf and public
authority switches. No serialized syntax/HIR/session identity is introduced
here.
