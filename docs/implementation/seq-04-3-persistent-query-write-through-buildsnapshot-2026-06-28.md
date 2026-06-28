# Seq-04.3 persistent query write-through and BuildSnapshot evidence (2026-06-28)

## Boundary

This overlay extends the seq04.1/seq04.2 persistent compiler query substrate with adapter-owned write-through for the two safe compiler-private `.awbo` object families that already have deterministic facts:

- parsed syntax facts (`CompilerObjectKind::ParsedSyntax`, `QueryKind::Parse`);
- HIR-body facts (`CompilerObjectKind::HirBody`, `QueryKind::HirBody`).

Write-through is performed by the filesystem cache adapter. The Sans I/O project data crates continue to own stable types, keys, and snapshot evidence, but do not perform filesystem reads or writes.

## Existing substrate consumed

The overlay assumes the following existing pieces are present:

- `AwboEnvelope` deterministic encode/decode and payload validation;
- `CompilerObjectKey` and `CompilerBuildIdentity` for exact compiler-private identity;
- `CompilerObjectKind::safe_read_through_query_kind` and `safe_read_through_artifact_kind` for the safe parse/HIR object families;
- `FilesystemCacheStore::{store_artifact_with_logical_item, read_persistent_query}`;
- compiler pure fact builders `parsed_syntax_payload` and `hir_body_payload`;
- in-memory project compile-unit cache used by `arcw build --watch`.

## Design

### 1. Adapter-owned write-through API

`arcweft-project-loader::cache::persistent_query` gains:

- `PersistentQueryWriteRequest`;
- `PersistentQueryWriteReceipt`;
- `PersistentQueryWriteError`;
- `FilesystemCacheStore::write_persistent_query`.

The write API validates that the requested query is the safe query family for the object kind, that the payload kind matches the object key, and that the object kind is one of the parse/HIR safe object kinds. It then constructs an `AwboEnvelope`, encodes deterministic bytes, stores the bytes through the existing content-addressed object store, and writes the key-addressed record with the logical item label.

This keeps write-through adapter-owned. The compiler produces pure payloads; the adapter owns object/record persistence and filesystem repair semantics.

### 2. Verified immutable write repair

`FilesystemCacheStore` changes immutable writes from "skip if path exists" to "skip only if existing bytes verify." If an existing object or record path exists but does not match the bytes being written, the store replaces the file atomically through a temp file and rename path.

This is necessary for local-cache recovery tests: a corrupt local object or record must remain a soft miss during read-through and must be repaired after a successful rebuild/write-through instead of being left permanently poisoned by path existence.

### 3. Stable BuildSnapshot evidence model

`arcweft-project::incremental` extends stable cache evidence:

- `InvalidationReason::CorruptObject` distinguishes object-byte/envelope corruption from record corruption;
- `InvalidationReason::ConservativeInvalidation { policy }` records intentional rebuild despite a valid safe object;
- `CacheRecordStatus::HitThenRebuilt { reason }` records valid disk facts that were observed but not used to reconstruct compiler IR;
- `CacheRecordStatus::Rebuilt { reason }` records source rebuild after miss/stale/corrupt evidence.

`CacheRecordStatus` also gains inherent methods:

- `as_str()` for CLI/cache reports;
- `is_rebuilt()` for tests and explainers;
- `rebuild_reason()` for evidence consumers.

`BuildSnapshot::with_additional_queries` appends query evidence and reuses the snapshot's deterministic query ordering. This avoids transient progress notes in stable design docs while giving future `arcw cache explain` work stable field names.

### 4. Compiler snapshot status mapping

`arcweft-compiler::incremental::snapshot_compiled_project` now records compile-unit source rebuilds as `CacheRecordStatus::Rebuilt` with reasons instead of plain `Miss`:

- in-process cache miss: `MissingRecord`;
- incremental disabled: `OptionsChanged`.

This keeps the existing compile-unit HIR-body summary useful while persistent query evidence records finer-grained parse/HIR object write-through outcomes.

### 5. CLI integration and watch precedence

`arcw build` now builds persistent query payloads after a successful project compile and before writing the final snapshot JSON. For each source module and each safe object kind:

1. Re-parse the module source to build parse facts.
2. Use the already compiled module HIR to build HIR-body facts.
3. Derive deterministic `CompilerObjectKey` and `ArtifactKey` from the build snapshot, source digest, dependency interface digests, compiler identity, and query options digest.
4. If project incremental mode is disabled, record a rebuild reason without writing.
5. If the in-memory compile-unit cache hit, record `Hit` and do not touch disk. This is the watch-mode precedence rule.
6. Otherwise read the disk record for typed evidence, write the `.awbo` object/record after successful rebuild, and record one of:
   - `HitThenRebuilt { ConservativeInvalidation { policy: "safe_awbo_facts_do_not_reconstruct_compiler_ir" } }`;
   - `Rebuilt { MissingRecord | CacheSchemaChanged | CompilerChanged | SourceChanged | InterfaceChanged | BodyChanged | EnvironmentChanged | OptionsChanged | CorruptRecord | CorruptObject }`.

The final `ProjectBuildArtifacts` carries the snapshot actually written to disk. The watch loop compares this final snapshot, including persistent-query evidence and AWFB content root, when reporting invalidations.

## Determinism

The implementation preserves deterministic AWFB content roots by keeping `.awbo` write-through outside bundle byte generation. The bundle is still compiled or read from the existing bundle cache before persistent query objects are written. The snapshot's `content_root` is computed from the generated AWFB bytes and appended query evidence does not feed back into bundle content.

`BuildSnapshot::with_additional_queries` sorts by query kind and artifact key using the existing stable ordering rule, so repeated clean and cached builds produce stable snapshot query ordering.

## Tests added

### `arcweft-project`

- snapshot status helpers report rebuild reasons;
- appended query evidence is deterministic.

### `arcweft-compiler`

- project snapshot records persistent-query rebuild status and reason for compile-unit miss evidence.

### `arcweft-project-loader`

- write-through stores a parse `.awbo` object that read-through can load;
- write-through rejects payload/key kind mismatches;
- corrupt object read-through maps to `InvalidationReason::CorruptObject`;
- object/record store repairs corrupt existing immutable paths after a valid write.

### `arcweft-cli`

- clean build writes parse/HIR persistent query evidence and repeated builds preserve the AWFB content root;
- watch-style in-memory compile-unit hits take precedence over corrupt disk records;
- clean rebuild after corrupt persistent objects records a corrupt-object rebuild reason.

## Non-goals retained

- No interface summary reuse until the seq04.5 schema exists.
- No semantic, typecheck, runtime-plan, bytecode, or link-plan reuse.
- No public compatibility promise for compiler-private `.awbo` payloads across compiler identities.
- No change to the bundle format or AWFB content-root derivation.

## Structural note

No new dependencies are introduced. New behavior is added as inherent methods on Arcweft-owned enum/boundary types where the behavior belongs to the type (`CacheRecordStatus` and `BuildSnapshot`). Filesystem persistence remains in `arcweft-project-loader`; compiler fact projection remains pure in `arcweft-compiler`.

## Validation

Applied and validated in the Arcweft checkout on 2026-06-28:

```bash
cargo fmt --all
cargo fmt --all -- --check
cargo check -p arcweft-project-loader -p arcweft-compiler -p arcweft-cli --all-targets --all-features
cargo clippy -p arcweft-project-loader -p arcweft-compiler -p arcweft-cli --all-targets --all-features -- -D warnings
cargo test -p arcweft-project incremental --all-features
cargo test -p arcweft-project-loader persistent_query --all-features
cargo test -p arcweft-compiler persistent_query --all-features
cargo test -p arcweft-cli cache --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
just test-workspace
```

## Application Notes

The package patch was assembled against an older connector-observed revision, so
two hunks drifted in `persistent_query.rs` and `project_commands.rs`. The
intended changes were applied against the current main shape:

- `PersistentQueryWriteError` and `PersistentQueryWriteRequest::new` were added
  to the adapter-owned persistent query module.
- CLI persistent-query write-through imports and watch-loop snapshot threading
  were reconciled with the current `ProjectBuildArtifacts` layout.
- The CLI write-through body was split into source/item/commit helpers to keep
  the implementation under the active clippy line-count gate without changing
  the package boundary.

No intentional design deviation from the seq04.3 package was introduced.
