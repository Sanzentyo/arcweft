# Seq-04.4 Cache Explain Query Evidence Implementation

Date: 2026-06-28

## Goal

Expose persistent compiler query evidence through `arcw cache explain` so users can inspect
safe parse/HIR `.awbo` query records by logical item, artifact key, object digest, or persistent
query key digest.

The output remains deterministic and machine-readable for `--json`. The non-JSON output gains a
stable text rendering of the same adapter-owned evidence without introducing an interactive or
non-deterministic report mode.

## Ownership boundary

The implementation follows the current Arcweft layering:

- `arcweft-project` owns Sans I/O query and persistent-object domain behavior.
- `arcweft-project-loader` owns filesystem cache inspection, object reads, record reads,
  soft-miss evidence, and recovery recommendations.
- `arcweft-cli` only renders the `CacheExplainReport` returned by the loader.

Two small inherent methods are added to Arcweft-owned boundary enums rather than adding ad hoc CLI
helpers:

- `QueryKind::from_cache_namespace(namespace)` parses stable record namespaces.
- `CompilerObjectKind::from_safe_read_through_artifact_kind(artifact_kind)` maps safe parse/HIR
  artifact records back to persistent object kinds.

## Adapter evidence model

`arcweft-project-loader::cache::persistent_query` gains:

- `PersistentQueryExplainEvidence`
- `PersistentQueryKeyInputEvidence`
- `PersistentQueryNamedDigestEvidence`
- `PersistentQueryExplainStatus::{Hit, Miss}`
- `PersistentQueryRecoveryAction::{NoneRequired, RebuildFromSource}`
- `FilesystemCacheStore::explain_persistent_query_record(query, record)`

The explain evidence is intentionally derived in the adapter. It reuses the seq04.2/seq04.3
read-through and write-through evidence:

- `PersistentQueryReadOutcome`
- `PersistentQueryHit`
- `PersistentQueryMiss`
- `PersistentQueryMissReason`
- `PersistentQueryMissReason::invalidation_reason()`
- `CacheRecordStatus`

The CLI does not decode `.awci`, decode `.awbo`, classify soft misses, or map reasons to recovery.

## Report shape

Each `CacheExplainMatch` may now contain an optional `persistent_query` object. JSON output keeps
the existing deterministic report envelope and adds these fields when the matched record is a safe
persistent compiler query record:

```json
{
  "persistent_query": {
    "query": "parse",
    "artifact_key": "...",
    "object_kind": "parsed_syntax",
    "query_key": "...",
    "key_inputs": {
      "query_options_digest": "...",
      "dependency_interface_digests": [{ "name": "dep", "digest": "..." }],
      "dependency_body_digests": [{ "name": "dep", "digest": "..." }],
      "environment_digest": "..."
    },
    "compiler_identity": {
      "package_version": "0.1.0",
      "git_commit": "...",
      "rustc": "...",
      "target": "...",
      "enabled_features": ["..."]
    },
    "source_digest": "...",
    "payload_kind": "parsed_syntax",
    "record_schema_version": 1,
    "object_schema_version": 1,
    "payload_schema_version": 1,
    "object_digest": "...",
    "object_len": 128,
    "record_object_digest": "...",
    "record_object_len": 128,
    "observed_object_digest": "...",
    "observed_object_len": 128,
    "payload_digest": "...",
    "payload_len": 96,
    "status": "hit",
    "cache_record_status": { "kind": "hit" },
    "recovery_action": "none_required"
  }
}
```

For misses, the same object uses:

```json
{
  "status": "miss",
  "cache_record_status": { "kind": "miss", "reason": { "kind": "corrupt_object" } },
  "soft_miss_reason": { "kind": "object_digest_mismatch", "expected": "...", "actual": "..." },
  "recovery_action": "rebuild_from_source"
}
```

## Lookup behavior

`explain_cache(root, query)` still accepts 64-character lowercase BLAKE3 digests. It now checks
three digest families:

1. content-addressed object digests;
2. artifact key / `.awci` record path digests;
3. persistent compiler query key digests recovered from readable safe `.awbo` envelopes.

`explain_cache_by_logical_item(root, logical_item)` keeps its existing behavior and now enriches
safe parse/HIR persistent records with `persistent_query` evidence.

A missing object cannot provide a recoverable query-key digest without changing the record schema,
so a missing-object soft miss is inspectable through the logical item or artifact-key record path.
The implementation intentionally does not add a record schema migration or new cache write policy.

## Validation and recovery flow

For a candidate persistent query record, the adapter:

1. maps record artifact kind to a safe persistent object kind;
2. reads the object path referenced by the record;
3. reports missing/read/length/digest failures as soft misses with `rebuild_from_source`;
4. decodes the `.awbo` envelope when bytes are present and digest-valid;
5. derives the `CompilerObjectKey` from deterministic payload key inputs;
6. calls `FilesystemCacheStore::read_persistent_query` with a normal
   `PersistentQueryReadRequest`;
7. converts the resulting hit/miss evidence into `PersistentQueryExplainEvidence`.

This keeps source-of-truth validation in the persistent query adapter rather than duplicating it in
CLI rendering.

## Tests added

### `arcweft-project-loader`

- `cache_explain_embeds_persistent_query_hit_evidence`
  - writes a parse `.awbo` through `write_persistent_query`;
  - explains by logical item;
  - verifies query key, source digest, payload kind, schema, hit status, and no recovery;
  - explains by persistent query key digest.
- `cache_explain_embeds_persistent_query_soft_miss_evidence`
  - writes a parse `.awbo`;
  - removes the object file;
  - explains by logical item;
  - verifies `PersistentQueryExplainStatus::Miss`, `MissingObject`, and `RebuildFromSource`.

### `arcweft-cli`

- `explain_accepts_persistent_query_key_digest`
  - writes a parse `.awbo` through the loader;
  - invokes the CLI cache explain command with the persistent query key digest and `--json` mode.

## Non-goals retained

- No new cache write policy.
- No record schema migration.
- No runtime-plan reuse.
- No bytecode-unit reuse.
- No link-plan reuse.
- No TUI or non-deterministic pretty report.

## Structure audit notes

The implementation adds no new dependencies and no new crates. It extends one existing adapter
module and one existing CLI module. The new public evidence types live under
`arcweft-project-loader::cache::persistent_query`, which already owns typed read-through and
write-through evidence.

The structural audit was run against parent revision `34d477cba34a` plus this working change. It
reported 0 errors and 113 warnings. Changed Rust file measurements after formatting:

| Path | Bytes | LOC | Classification | Responsibility |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-cli/src/app/cache.rs` | 27,879 | 714 | production + unit tests | CLI cache command rendering and cache command smoke tests |
| `crates/arcweft-project/src/incremental.rs` | 24,712 | 706 | production + unit tests | incremental query identities and build snapshot evidence |
| `crates/arcweft-project/src/persistent_object/schema.rs` | 12,257 | 315 | production | AWBO schema/domain types |
| `crates/arcweft-project-loader/src/cache/inspect.rs` | 44,432 | 1,256 | production + unit tests | filesystem cache stats/verify/explain/prune adapter |
| `crates/arcweft-project-loader/src/cache/persistent_query.rs` | 47,157 | 1,237 | production + external unit-test module | persistent query read/write/explain adapter |

`inspect.rs` and `persistent_query.rs` are above the 1,200 LOC warning threshold but below the
2,500 LOC error threshold. This slice keeps the new logic inside the existing cache inspect and
persistent query responsibility modules rather than creating a new workspace-external fixture tree.
A future split should separate inspect tests/helpers or persistent explain evidence formatting when
those modules take another substantial cache feature.

Largest current Rust files observed during the audit pass are existing/generated or test-heavy
hotspots, led by `crates/arcweft-text-layout/src/vertical_orientation.rs` at 357,456 bytes /
12,394 LOC and `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` at 255,424 bytes / 7,445 LOC.
This seq04.4 slice does not modify those files.

## Validation status

Validated after applying the package to this checkout:

```bash
cargo fmt --all
cargo test -p arcweft-project-loader cache_explain --all-features
cargo test -p arcweft-cli cache --all-features
cargo check -p arcweft-cli -p arcweft-project-loader --all-targets --all-features
cargo clippy -p arcweft-cli -p arcweft-project-loader --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
cargo fmt --all -- --check
just test-workspace
```

Results:

- `arcweft-project-loader cache_explain`: 2 tests passed.
- `arcweft-cli cache`: 14 tests passed.
- `cargo check` and clippy passed for `arcweft-cli` and `arcweft-project-loader`.
- structural audit: 1,805 files scanned, 956 Rust files, 455,929 Rust physical LOC, 0 errors, 113 warnings.
- formatting, whitespace, and workspace fast path checks passed.

## Apply-time adjustments

- The package patch was assembled before the current cache CLI/external fetch code, so
  `inspect.rs` and `cache.rs` were ported manually onto the current main layout.
- `CacheRecord` is passed by reference in `push_record_match`, and several early-return matches
  were expressed with `let else` to satisfy the workspace clippy gate.

No intentional design deviation from seq04.4 is introduced. The only documented limitation is that
query-key lookup for a missing object cannot be recovered without adding the query key to `.awci`
records, which would be a record schema/write-policy change and is outside this slice.
