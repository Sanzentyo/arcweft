# Request: Patch Target Manifest and Signature Preservation Design

## Request

Please design the implementation path for product-grade AWFB patch target
materialization, including manifest mutation, signature preservation, external
payload fetching, and release policy interaction.

The design should be concrete enough to turn into small Rust implementation
cuts with focused tests.

## Why this needs a decision

The incremental hot-swap bundle work now has:

- AWFB patch artifacts with a `PatchPlan` section.
- Add/replace/remove section operations keyed by section id and content digest.
- Embedded changed-section payloads carried through `AssetBlob` sections.
- Metadata-only external section descriptor changes.
- Base/target content-root validation.
- Runtime/session paths that can inspect and apply patch bytes.
- Signature-policy-aware patch decode/apply entry points that reject unsigned
  or incorrectly signed patch artifacts before target materialization.

The current patch materializer is sufficient for local dev and inspection
flows, but the implementation note records that patch materialization keeps
base manifest bytes in this cut and treats external payload fetching,
signature preservation, and manifest mutation as separate concerns.

Product-grade target materialization is larger than merging section
descriptors. A correct design must answer how manifests are rewritten, how
trailing signatures are preserved or invalidated, how external payloads are
fetched or referenced, and how release manifests and trust policies bind the
materialized target.

## Design questions

Please propose concrete answers for:

1. When a patch changes logical sections, which manifest fields must be
   rewritten in the materialized target AWFB?
2. Should patch target materialization preserve, strip, or regenerate the base
   bundle signature block?
3. How should target signatures be produced for local dev, CI, release
   signing, and offline patch inspection?
4. How should external section descriptor changes interact with `.awfr`
   release manifests and host fetch adapters?
5. When must materialization require external payload bytes, and when is
   metadata-only descriptor materialization sufficient?
6. How should release signature policy validate a signed patch artifact versus
   the materialized target bundle?
7. How are base content root, target content root, whole-file digest, signing
   digest, bundle kind, and key epoch bound across patch and target artifacts?
8. What happens when a patch removes or changes sections referenced by the base
   manifest or release manifest?
9. Which crate owns each operation: `arcweft-bundle`, project-loader cache,
   CLI signing/fetch adapters, runtime-driver, or product players?
10. What tests prove manifest mutation correctness, signature invalidation,
    signature regeneration, external descriptor handling, and rollback on
    failed materialization?

## Constraints

- Keep `arcweft-bundle` Sans I/O.
- Keep filesystem, network, cache, clock, and signing-key access in adapters.
- Do not imply that a base signature remains valid for changed target bytes.
- Preserve deterministic content roots and signing digests.
- Treat release signatures and signer policies as typed data, not raw opaque
  passthroughs.
- Keep local dev patch application ergonomic without weakening product release
  verification.

## Expected output

Please provide:

- the product-grade materialization state machine;
- affected crates/modules;
- new or changed public/private types;
- manifest rewrite rules;
- signature preservation/stripping/regeneration rules;
- external payload fetch/reference rules;
- release manifest interaction;
- error and rollback behavior;
- step-by-step implementation order;
- focused unit tests and CLI smoke commands.

## Current goal boundary

Until this design is answered, the current incremental hot-swap goal should not
implement:

- product-grade manifest mutation for materialized patch targets;
- preserving or regenerating target signatures after patch materialization;
- release-manifest mutation for patch target publication;
- automatic external payload fetching as part of Sans I/O patch
  materialization.

The current goal may keep:

- local/dev patch target materialization from embedded patch payloads;
- metadata-only external descriptor changes in materialized AWFB bytes;
- signature-policy checks on patch artifacts before materialization;
- target content-root validation after materialization;
- CLI/runtime inspection and restart fallback behavior.

## Useful current evidence

Start with these files:

- `crates/arcweft-bundle/src/patch.rs`
- `crates/arcweft-bundle/src/container.rs`
- `crates/arcweft-bundle/src/release.rs`
- `crates/arcweft-runtime-driver/src/session.rs`
- `crates/arcweft-player-native/src/patch_endpoint.rs`
- `crates/arcweft-cli/src/app/release_sign.rs`
- `docs/implementation/incremental-hot-swap-bundle-2026-06-23.md`
