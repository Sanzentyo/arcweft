# Concrete implementation order

## 1. Neutral wire identities, scalars, limits, and source-backed DTOs

- add workspace crate/member/dependencies for `arcweft-resource-manifest`;
- implement `limits.rs`, `diagnostic.rs`, private `strict_json.rs`, `source_map.rs`, and identity/scalar DTOs in `wire.rs`;
- freeze format/schema probes and exact token enums;
- add no filesystem API and no generic semantic value.

Exit gate: strict lexical/path/limit tests and typed DTO construction tests pass.

## 2. Complete tagged records and canonical examples

- implement every value type, scalar value, constant, retained identity, schema, descriptor, capability, lowering, and codec DTO;
- check in minimal/full fixtures and digest vectors equivalent to this package;
- prove exhaustive variant coverage through match-based typed tests.

Exit gate: schema fixtures cover every current closed variant and no wildcard match hides additions.

## 3. Sole strict decoder and canonical encoder

- expose only `decode_resource_type_manifest` and `encode_resource_type_manifest_v1`;
- use one lexical tree and parallel source map;
- dispatch format/schema strictly;
- canonicalize semantic sets and recompute descriptor claims.

Exit gate: round-trip, byte regeneration, duplicate/null/unknown/wrong-shape, float/integer/string/ID, and determinism tests pass.

## 4. Typed conversion and immutable publication

- add inherent `ResourceTypeDescriptor::semantic_digest()` and typed digest wrapper using the existing transcript;
- add inherent `ResourceTypeRegistry::codecs()` iterator;
- construct current model values directly;
- aggregate all documents/base candidates and call `ResourceTypeRegistry::publish` once;
- exhaustively map registry/default issues to nested source ranges.

Exit gate: all registry positive/negative/tamper/atomic tests pass; existing resource-model contract tests remain unchanged except for additive API coverage.

## 5. Package/build loading and deterministic artifact publication

- add strict root `resource-type-manifest` path;
- add typed explicit topology resource variant/seed and loader transaction;
- hand loader registry to existing compiler context;
- add `BundleSectionKind::ResourceTypeManifests = 22` in the original enum methods;
- implement exact section framing and runtime reconstruction/digest verification.

Exit gate: no-directory-scan integration, required-section behavior, artifact tamper, compiler handoff, and runtime reconstruction tests pass.

## 6. Full gates and structural audit

Run all commands in `TEST_MATRIX.md`, structured dependency checks, and the repository-required structural audit. Record deviations only if a pinned repository invariant has changed; do not introduce aliases or a fallback reader to accommodate implementation convenience.
