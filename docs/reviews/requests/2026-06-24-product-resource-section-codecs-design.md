# Request: Product Resource Section Codecs Design

## Request

Please design the implementation path for replacing the remaining typed JSON
product resource sections with compact, deterministic AWFB section codecs.

The design should be concrete enough to turn into small Rust implementation
cuts with focused tests.

## Why this needs a decision

The incremental hot-swap bundle work now has:

- AWFB v1 as the fixed-header product container.
- Section descriptors with explicit kind, schema version, residency,
  placement, compression, decoded size, and digests.
- Bounded section reads, zstd output limits, external descriptors, unknown
  optional-section skipping, and unknown required-section rejection.
- Product `ProgramBytecode` encoded as an `AWBC` binary envelope.
- Runtime types, entrypoints, adapter requirements, content catalog, display
  catalog, normalized source, patch plans, and release metadata still encoded
  as section-specific typed JSON payloads.

The current bundle/reference material defines the AWFB container shape and the
section families, but it does not define compact binary schemas for graph
indexes, entity tables, asset catalogs, display catalogs, source maps,
contracts, shaders, UI, audio, text resources, locale data, or debug symbols.

This means the repository has the container and safe section boundaries, but
does not yet have authoritative binary resource codecs that can replace the
JSON resource payloads.

## Design questions

Please propose concrete answers for:

1. Which v1 product sections should remain JSON, and which should get compact
   binary codecs first?
2. What are the canonical binary schemas for runtime types, entrypoints,
   adapter requirements, content catalog, display catalog, asset catalog,
   source maps, locale/text resources, audio graphs, shader/UI resources, debug
   symbols, contracts, and graph/entity indexes?
3. How are schema versions, optional fields, enum registries, string tables,
   public-id tables, and cross-section references encoded?
4. What depth/count/string/byte budgets must each section-specific decoder
   enforce before exposing typed values?
5. Which cross-section validation belongs in `arcweft-bundle`, and which
   validation belongs in runtime-driver/player adapters?
6. How should external section descriptors and release manifests reference
   resource sections without forcing embedded payloads?
7. How should patch diffing classify compact resource section changes as
   content-only, code-compatible, code-generational, or restart-required?
8. How should inspection/export formats present compact sections without
   making JSON an alternate product codec?
9. What golden fixtures prove deterministic bytes, bounds checking, unknown
   section handling, and JSON-to-binary parity during migration?

## Constraints

- Keep `arcweft-bundle` Sans I/O.
- Keep `.awfb` as the only product container path.
- Do not introduce arbitrary codec probing for product bundles.
- Keep JSON/TOML/YAML/MessagePack/CBOR/Avro as explicit inspection/export or
  interoperability paths only.
- Preserve deterministic content roots and patch diffs.
- Prefer typed section-owner APIs over ad hoc per-call JSON maps.
- Do not block the current product path on all resource codecs existing at
  once; the migration should be section-by-section.

## Expected output

Please provide:

- the prioritized section-codec migration plan;
- affected crates/modules;
- new or changed public/private types;
- canonical binary schema definitions;
- decoder budgets and validation rules;
- inspection/export behavior;
- patch compatibility rules for compact resource changes;
- step-by-step implementation order;
- focused unit tests, golden fixtures, and CLI smoke commands.

## Current goal boundary

Until this design is answered, the current incremental hot-swap goal should not
implement:

- compact binary codecs for every non-bytecode product resource section;
- a one-off binary schema invented from the current JSON tests;
- codec probing or JSON fallback for product `.awfb` sections;
- patch compatibility rules for compact resource sections beyond the currently
  implemented typed JSON payloads.

The current goal may keep:

- AWFB v1 fixed-header container validation;
- typed JSON payloads for non-bytecode product sections;
- `AWBC` compact validation sidecar plus structured bytecode payload;
- explicit inspection/export formats separate from `.awfb`;
- section-by-section replacement points documented for future codecs.

## Useful current evidence

Start with these files:

- `crates/arcweft-bundle/src/container.rs`
- `crates/arcweft-bundle/src/product.rs`
- `crates/arcweft-bundle/src/patch.rs`
- `docs/05-build-and-security/packaging.md`
- `docs/implementation/incremental-hot-swap-bundle-2026-06-23.md`
