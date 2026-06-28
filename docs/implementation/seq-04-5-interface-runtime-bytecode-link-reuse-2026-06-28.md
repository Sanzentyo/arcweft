# Seq-04.5 Interface / Runtime / Bytecode / Link Reuse Implementation Note

Date: 2026-06-28

## Boundary

This slice implements interface-summary persistent compiler query support and deliberately keeps runtime-plan, bytecode-unit, link-plan, typecheck, and linked-HIR reuse conservative.

The ready subset extends the existing seq04 parse/HIR substrate with:

- stable interface-summary object schema;
- deterministic interface export and import digest roots;
- project-loader read/write-through for interface summaries;
- compiler pure fact builder from parsed source plus HIR;
- CLI build evidence for `QueryKind::Interface` without changing AWFB bytes or content-root derivation.

## Stable Interface Summary Object Schema

`InterfaceSummaryObject` is now a validated `.awbo` payload with these fields:

- `schema_version`;
- `compiler_namespace`;
- `module`;
- `source_digest`;
- `source_span`;
- `diagnostics`;
- `stage_inputs`;
- `exports_digest`;
- `imports_digest`;
- canonical `public_symbols`.

`PublicSymbolObject` uses `PublicSymbolKind` instead of a free-form string. The first ready symbol families are `flow`, `function`, `agent`, and an opaque `declaration` descriptor for syntax/HIR declarations that are not yet module-aware semantic exports.

Interface read-through validates envelope shape, digest/length, payload schema, compiler namespace, source digest, stage inputs, canonical public-symbol order, duplicate descriptors, export digest, and import digest.

## Read/Write-Through Integration

`CompilerObjectKind::InterfaceSummary` is opted into the existing Arcweft-owned mapping methods:

- `safe_read_through_query_kind() -> QueryKind::Interface`;
- `safe_read_through_artifact_kind() -> ArtifactKind::InterfaceSummary`;
- `from_safe_read_through_artifact_kind(ArtifactKind::InterfaceSummary)`.

`PersistentQueryHitPayload` now includes `InterfaceSummary`. Project-loader validation reconstructs object keys from interface payload fields and returns typed soft misses for stale source, options, dependencies, environment, schema, and corruption failures.

## Compiler Fact Builder

`arcweft-compiler::persistent` adds:

- `InterfaceSummaryFactsInput`;
- `interface_summary_object`;
- `interface_summary_payload`.

The builder records facts through public HIR accessors and stable type-signature summarizers. It does not serialize `HirModule`, linked HIR, `CompiledProjectModule`, runtime-plan objects, or bytecode objects.

## CLI Build Behavior

`arcw build` attempts parse/interface/HIR persistent query write-through after successful compilation. A valid disk hit for interface facts is still recorded as a conservative rebuild:

```text
HitThenRebuilt { ConservativeInvalidation { policy: "safe_awbo_facts_do_not_reconstruct_compiler_ir" } }
```

That status is intentional. It records that the disk facts were valid and equivalent as evidence while the compiler still rebuilds source-derived HIR and semantic context.

The existing deterministic AWFB content-root behavior is preserved because `.awbo` persistence happens after bundle bytes are generated and content-root inspection is complete.

## Deferred Families

Typecheck and linked-HIR reuse remain conservative until module-aware sema exists. Interface summaries are not enough to skip typechecking because current semantic checks still operate over linked HIR/project context.

Runtime-plan unit reuse is deferred because current runtime-plan artifacts are whole-build outputs with insufficient stable per-unit identity for persistent query reuse.

Bytecode-unit reuse is deferred because AWBC/codegen identities are not sufficiently stable in the current repository to trust bytecode-unit reuse.

Link-plan reuse is deferred because it requires stable identities for all runtime/bytecode units and a stable link descriptor. Current link/bundle caching remains artifact-level storage only.

## Validation

The implementation adds or extends tests for interface summary codec round-trip and malformed payload rejection, compiler fact-builder round-trip, project-loader interface read/write-through, unsupported later object families, and CLI cache evidence.
