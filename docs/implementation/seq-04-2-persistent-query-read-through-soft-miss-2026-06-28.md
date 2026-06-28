# Seq-04.2 Persistent Query Read-Through Soft-Miss Implementation

Date: 2026-06-28

## Scope

This package implements the second persistent compiler query cache slice: adapter-owned read-through validation for safe parse/HIR `.awbo` compiler objects. It assumes seq04.1 has already added deterministic `.awbo` codecs for parsed syntax facts and HIR-body facts.

The implementation does not persist new compiler query records. It reads existing records/objects only and returns typed hit/miss evidence so a later build-snapshot slice can record exactly why a query was reused or rebuilt.

## Ownership boundary

The read-through API is owned by `arcweft-project-loader::cache::persistent_query` because it performs filesystem IO over cache records and content-addressed object files. `arcweft-project` remains Sans I/O and only receives two owner methods on `CompilerObjectKind` to state which object kinds are safe for this read-through slice.

The compiler crate remains Sans I/O. Its added test only proves that a persistent object mismatch is recoverable by rebuilding parse/HIR from source; it does not call filesystem cache APIs.

## New module

`crates/arcweft-project-loader/src/cache/persistent_query.rs` defines:

- `PersistentQueryReadRequest`
- `PersistentQueryReadOutcome::{Hit, Miss}`
- `PersistentQueryHit`
- `PersistentQueryHitPayload::{ParsedSyntax, HirBody}`
- `PersistentQueryMiss`
- `PersistentQueryMissReason`
- `PersistentQueryIoKind`
- `FilesystemCacheStore::read_persistent_query(&PersistentQueryReadRequest)`

The hit payload enum intentionally contains only parsed syntax and HIR-body facts. Unsupported object kinds return `PersistentQueryMissReason::UnsupportedObjectKind`.

## Existing-file changes

The patch `overlay/patches/seq-04-2-existing-files.patch` makes these small existing-file edits:

1. `arcweft-project::persistent_object::CompilerObjectKind`
   - adds `safe_read_through_query_kind()`;
   - adds `safe_read_through_artifact_kind()`.

2. `AwboEnvelope`
   - adds `decode_detached(bytes)` for adapter validation that first validates the envelope shape without applying a caller key;
   - keeps existing `decode(bytes, key)` behavior by delegating to `decode_detached` and then calling `validate(key)`.

3. `arcweft-project-loader::cache`
   - exposes `pub mod persistent_query`;
   - makes `FilesystemCacheStore::object_path` and `record_path` `pub(crate)` so sibling cache adapter code and same-crate tests can use the canonical layout without duplicating path construction.

4. `arcweft-compiler::persistent` tests
   - adds `persistent_query_soft_miss_does_not_block_source_rebuild`.

## Read-through flow

`FilesystemCacheStore::read_persistent_query` performs validation in this order:

1. Confirm the requested `CompilerObjectKind` is enabled for safe read-through.
2. Confirm the supplied `QueryKind` matches the object kind.
3. Resolve the expected artifact kind from the object kind.
4. Read and decode the `.awci` record.
5. Validate record schema and artifact key through existing `CacheRecord::from_slice_for_key`.
6. Validate record artifact kind.
7. Read the content-addressed object referenced by the record.
8. Validate object length against the record.
9. Validate object digest against the record.
10. Decode `.awbo` with `AwboEnvelope::decode_detached`.
11. Validate envelope object kind and stability.
12. Validate payload kind.
13. Validate payload schema version.
14. Validate compiler namespace and compiler identity.
15. Validate source digest.
16. Validate query options digest, environment digest, dependency interface digests, and dependency body digests.
17. Validate envelope key digest.
18. Return a typed hit payload for parsed syntax or HIR-body facts.

Every failure returns `PersistentQueryReadOutcome::Miss` with a structured `PersistentQueryMissReason` and observed digest/length evidence when available.

## Soft-miss classes

The implementation covers the requested recoverable classes:

- missing record;
- corrupt record;
- record schema mismatch;
- record key mismatch;
- artifact kind mismatch;
- missing object;
- object read failure;
- object digest mismatch;
- object length mismatch;
- corrupt object;
- object schema mismatch;
- object kind/stability mismatch;
- payload kind mismatch;
- payload schema mismatch;
- payload digest mismatch;
- payload length mismatch;
- compiler identity mismatch;
- source digest mismatch;
- query options mismatch;
- environment mismatch;
- dependency interface mismatch;
- dependency body mismatch;
- envelope key digest mismatch;
- unsupported object kind;
- query kind mismatch.

The API also exposes `cache_record_status()` on `PersistentQueryReadOutcome` to produce a conservative `CacheRecordStatus` for future `BuildSnapshot` integration while preserving richer typed evidence in the read outcome.

## Tests

`arcweft-project-loader` adds focused `persistent_query_*` tests for the requested hit/miss classes. The tests construct deterministic `.awbo` parse/HIR envelopes using seq04.1 payload types and use the filesystem cache store's existing immutable record/object layout.

`arcweft-compiler` adds one focused test with the same `persistent_query` filter. It intentionally exercises existing Sans I/O parse/HIR rebuild after a changed compiler identity makes an existing persistent object invalid.

## Non-goals

This slice intentionally does not add:

- write-through cache persistence;
- compiler-driver use of read-through results;
- CLI cache explain UI changes;
- typecheck reuse;
- runtime-plan reuse;
- bytecode-unit reuse;
- link-plan reuse;
- hard-error policy for corrupt local cache outside explicit cache verification.

## Structure audit notes

New Rust files in this package:

| Path | Owning crate | Role | Bytes | LOC | Notes |
| --- | --- | --- | ---: | ---: | --- |
| `crates/arcweft-project-loader/src/cache/persistent_query.rs` | `arcweft-project-loader` | Production adapter module | 27,425 | 821 | Within the ordinary responsibility-module target range; owns read-through evidence and validation. |
| `crates/arcweft-project-loader/src/cache/persistent_query/tests.rs` | `arcweft-project-loader` | Unit-test submodule | 18,817 | 561 | Focused soft-miss coverage; kept out of production file through `#[cfg(test)] mod tests;`. |

The production module stays under the 1,200 LOC warning threshold. It touches cache adapter orchestration and filesystem IO but does not add new crate dependencies or move IO into Sans I/O crates.

## Validation status

Executed in the Arcweft checkout after applying:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-project-loader persistent_query --all-features -- --nocapture
cargo test -p arcweft-compiler persistent_query --all-features -- --nocapture
cargo check -p arcweft-project -p arcweft-project-loader -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-project -p arcweft-project-loader -p arcweft-compiler --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
just test-workspace
```

Results:

- Focused project-loader persistent query tests passed: 16 tests.
- Focused compiler persistent query rebuild test passed: 1 test.
- Targeted check and clippy passed.
- Structural audit passed with `0 error(s), 107 warning(s)`.
- `just test-workspace` passed.

The applied implementation intentionally boxes `PersistentQueryReadOutcome`
payload variants to keep the public hit/miss enum and internal `Result` errors
small under the workspace clippy configuration. The package zip's generated
patch was malformed for `git apply`, so the same existing-file edits were
applied manually against the current checkout.
