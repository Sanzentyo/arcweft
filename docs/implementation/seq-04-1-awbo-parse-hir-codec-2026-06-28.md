# Seq-04.1 AWBO parse/HIR codec implementation note (2026-06-28)

## Boundary

This overlay implements deterministic compiler-private `.awbo` payload codecs for the first safe persistent compiler objects:

- parsed syntax facts;
- HIR-body exact facts.

It deliberately stops before read-through or write-through cache reuse. Corrupt/stale object recovery policy belongs to seq04.2 and later adapter work.

## Source material used

The design follows:

- `docs/reviews/requests/2026-06-24-seq-04-persistent-compiler-query-cache.md`
- `docs/reviews/requests/2026-06-24-persistent-compiler-query-cache-design.md`
- `docs/implementation/integrated-execution-2026-06-24.md`
- `crates/arcweft-project/src/persistent_object.rs`
- `crates/arcweft-project/src/artifact.rs`
- `crates/arcweft-project/src/fingerprint.rs`
- `crates/arcweft-compiler/src/project.rs`
- `crates/arcweft-compiler/src/incremental.rs`
- syntax/HIR source models needed to summarize facts without serializing unstable structs directly.

## Payload schema

`arcweft-project::persistent_object` now separates schema, payload, and codec code.

### Envelope

The `.awbo` envelope keeps the existing identity fields:

- magic bytes;
- cache schema version;
- compiler object kind;
- object stability;
- canonical key digest;
- encoded payload digest;
- encoded payload byte length;
- typed payload.

`AwboEnvelope::encode` emits deterministic binary bytes. `AwboEnvelope::decode(bytes, key)` checks magic, schema, kind, stability, key digest, payload digest, payload length, payload schema version, source digest, compiler identity namespace, stage inputs, spans, and diagnostic summary counts.

### Parsed syntax facts

`ParsedSyntaxObject` contains:

- `schema_version`;
- `compiler_namespace` copied from `CompilerObjectKey`;
- `source_label`;
- exact `source_digest`;
- full-source `StableSourceSpanObject`;
- `SyntaxStatsObject` with parser counters and source dimensions;
- `StableDiagnosticSummaryObject`;
- stable stage inputs copied from the key;
- `ParsedSyntaxEvidenceObject` with root kind, CST shape digest, line-index digest, CST node/token/error counts, typed item counts, and wiki-link count.

The parse evidence is enough for later validation decisions but does not claim typed AST, semantic, or typecheck reuse.

### HIR-body exact facts

`HirBodyObject` contains:

- `schema_version`;
- `compiler_namespace` copied from `CompilerObjectKey`;
- module name;
- exact source digest/span;
- diagnostic summary;
- stable stage inputs copied from the key;
- `body_digest`;
- `HirBodyFactsObject` with top-level counts, nested flow-item counts, symbol digest, and body-shape digest.

The compiler builder projects public HIR accessors into counts and digests. It never serializes `HirModule`, `CompiledProjectModule`, or any unstable Rust internal as a persistent compatibility promise.

## Compiler projection API

`arcweft-compiler::persistent` adds:

- `ParsedSyntaxFactsInput`;
- `HirBodyFactsInput`;
- `PersistentFactsError`;
- `parsed_syntax_object`;
- `parsed_syntax_payload`;
- `hir_body_object`;
- `hir_body_payload`.

These functions are pure and Sans I/O. They perform no filesystem access and no cache store/load operations.

## Enum-owned stable tags

Stable cache tags are added as inherent methods on Arcweft-owned enums:

- syntax: `SyntaxKind`, `FlowKind`, `FunctionKind`, `AwaitBranchKind`;
- HIR: `HirTopLevelDecl`, `HirFlowItem`.

This keeps cache spelling behavior with the enum that owns the variants and avoids scattered helper traits or stringly stage logic.

The `cache_facts` modules are public responsibility modules in `arcweft-lang-syntax` and `arcweft-lang-hir` so downstream compiler fact builders can use the inherent enum methods without broad root-level re-export shims.

## Repository application adjustments

The package overlay was applied as designed, with local compile/clippy adjustments:

- fact builder inputs are borrowed by `parsed_syntax_object`, `parsed_syntax_payload`, `hir_body_object`, and `hir_body_payload` because the builders do not need to consume their input structs;
- `HirBodyCounts` private fields use domain names such as `flows`, `functions`, and `statements`, then map explicitly into the public `HirBodyFactsObject` count fields;
- duplicate HIR flow-item match arms for choice and await variants are combined;
- optional string encoding uses direct `if let` control flow;
- the HIR facts test expects zero `function_count` for the current sample because `pub view current_route()` is represented by current lowered HIR as a declaration, not as an item returned by `HirModule::functions()`.

## Tests included

`arcweft-project::persistent_object` tests cover:

- deterministic encoded bytes;
- parse/HIR round trips;
- unsupported schema rejection;
- compiler identity field rejection;
- malformed diagnostic payload rejection;
- malformed/truncated byte rejection;
- kind mismatch rejection.

`arcweft-compiler::persistent` tests cover:

- deterministic parse fact bytes generated from `ParsedSource`;
- HIR fact round trip generated from lowered HIR without HIR serialization;
- typed rejection of using a HIR key for parse facts.

## Non-goals retained

- No read-through cache lookup.
- No write-through cache persistence.
- No `arcw build` integration.
- No semantic/typecheck/runtime-plan/bytecode/link-plan reuse.
- No public compatibility promise for compiler-private `.awbo` bytes across compiler identities.

## Validation

Run in `D:/git/arcweft` after applying the package:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-project persistent_object --all-features
cargo test -p arcweft-compiler persistent --all-features
cargo check -p arcweft-project -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-project -p arcweft-compiler --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Structural audit result for this cut: `1704` files scanned, `919` Rust files, `440477` Rust physical LOC, `0 error(s), 107 warning(s)`.
