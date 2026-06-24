# Request 02: Product Artifact Sections, Patch Fingerprints, and Signing

## Sequence Position

This is the second design request in the integrated execution sequence.

Submit this after `2026-06-24-seq-01-executable-runtime-core.md` has answered
the executable AWBC/runtime-type/entrypoint shape. This design may begin while
Request 01 is being finalized, but it must not finalize bytecode or runtime-type
fingerprints before Request 01 is stable.

## Request

Please design the product artifact resource-section migration and product-grade
patch materialization model as one coherent bundle. This request intentionally
combines:

- compact product resource section codecs;
- patch compatibility fingerprints by section family;
- patch target materialization;
- manifest rewrite;
- signature disposition and regeneration;
- release-manifest interaction.

The design must be concrete enough to turn into small Rust implementation cuts
with focused tests.

## Existing Request Files To Incorporate

Use these existing requests as source material, but answer them together rather
than independently:

- `docs/reviews/requests/2026-06-24-product-resource-section-codecs-design.md`
- `docs/reviews/requests/2026-06-24-patch-target-manifest-signature-design.md`

Also incorporate the implemented contracts recorded in:

- `docs/implementation/integrated-execution-2026-06-24.md`
- `crates/arcweft-bundle/src/resource_codec.rs`
- `crates/arcweft-bundle/src/patch.rs`
- `crates/arcweft-bundle/src/container.rs`
- `crates/arcweft-bundle/src/release.rs`

## Why These Must Be Designed Together

Section codecs, patch fingerprints, manifest rewrite, and signing are one
product-artifact contract:

- section schemas determine descriptor digests and content roots;
- descriptor digests determine patch diffs;
- patch diffs determine compatibility labels;
- compatibility labels determine live apply, generation, or restart behavior;
- manifest rewrite and signing must know which logical resources changed;
- external descriptors and release manifests must use the same typed resource
  identities and digest rules.

Designing resource codecs without patch/signature policy would leave the patch
model stringly and under-specified.

## Current Implementation Evidence

The repository currently has:

- AWFB fixed-header product container validation;
- section descriptors with kind, schema version, residency, placement,
  compression, decoded size, and digests;
- explicit unknown-section handling;
- typed JSON payloads for most non-bytecode product resources;
- AWBC product section carrying the current structured bytecode payload plus
  compact validation sidecar;
- `arcweft-bundle::resource_codec` as a shared section header/string-table/
  public-id-table/budget contract;
- AWFB patch artifacts with patch plans, embedded payload carriers,
  metadata-only external descriptor changes, and target content-root validation;
- signature-policy-aware patch decode/apply entrypoints for patch artifacts.

## Required Design Decisions

Please provide concrete answers for:

1. Which product sections stay JSON for now, and which get compact binary
   codecs first?
2. What are the canonical binary schemas for runtime types, entrypoints,
   adapter requirements, content catalogs, display catalogs, asset catalogs,
   source maps, locale/text resources, audio graphs, shader/UI resources,
   contracts, debug symbols, and graph/entity indexes?
3. How are schema versions, optional fields, enum registries, string tables,
   public-id tables, and cross-section references encoded?
4. What section-specific depth/count/string/byte budgets must decoders enforce?
5. Which cross-section validation belongs in `arcweft-bundle`, and which
   belongs in runtime-driver/player adapters?
6. How do section changes map to `content-only`, `code-compatible`,
   `code-generational`, and `restart-required`?
7. How are patch compatibility fingerprints derived?
8. Which target manifest fields must be rewritten when sections change?
9. How should base signatures be stripped, regenerated, or treated for
   inspection?
10. How should target signatures be produced for local dev, CI, release, and
    offline inspection?
11. How should external payload descriptors interact with `.awfr` release
    manifests and fetch adapters?
12. When are external payload bytes required, and when is metadata-only
    descriptor materialization sufficient?
13. How should release policy validate base release trust, patch release trust,
    target root, and target signature trust?
14. Which crates own each operation?

## Required Implementation Order In The Design

Please propose small compiling cuts in this order or justify a better order:

1. Prioritize section families and freeze section schema ownership.
2. Implement compact codecs for runtime types, entrypoints, and adapter
   requirements.
3. Add deterministic fixtures and decoder budget tests.
4. Add patch compatibility fingerprint rules for migrated section families.
5. Convert content/display/source-map sections incrementally.
6. Add typed manifest rewrite state machine.
7. Add target signature disposition and signing digest rules.
8. Add external payload / release-manifest interaction.
9. Add CLI/runtime smoke paths for local dev and signed release validation.

## Tests To Specify

The design should include focused tests for:

- deterministic compact section bytes;
- unknown required/optional section behavior;
- budget failures;
- JSON-to-binary parity during migration;
- patch classification by section family;
- manifest root and descriptor rewrite correctness;
- base signature stripping on mutation;
- regenerated target signature validation;
- release policy that validates both signed patch and materialized target;
- missing external payload errors;
- metadata-only external descriptor acceptance when allowed;
- failed materialization rollback at adapter boundary.

## Constraints

- Keep `arcweft-bundle` Sans I/O.
- Keep filesystem, network, cache, clocks, and signing-key access in adapters.
- Do not introduce arbitrary codec probing for product bundles.
- Do not imply that a base signature remains valid for changed target bytes.
- Treat release signatures and signer policies as typed data.
- Preserve deterministic content roots and patch diffs.

## Expected Output

Please produce one design document with:

- prioritized resource-section migration plan;
- canonical section schemas;
- patch fingerprint and compatibility rules;
- manifest rewrite state machine;
- signature disposition and release policy;
- external payload/reference behavior;
- crate ownership rules;
- implementation cuts;
- test plan;
- explicit non-goals.

