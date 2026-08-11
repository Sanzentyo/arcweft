# FINAL CONTRACT — Lang-01.4.2.1

## Final disposition

This is a repository-specific final contract, not a generation fallback and not a compatibility proposal.

- `FINAL_CONTRACT=true`
- `FALLBACK=false`
- `BLOCKED=false`
- `OPEN_QUESTIONS=0`
- `IMPLEMENTATION_READY=true`
- `contract_agent_validated=true`
- `repository_contract_validation_succeeded=true`
- pinned `main`: `5821a3ca479b5b89ca6ede997b9cf4f42f6280a6`
- production code changed: **no**

The prior Lang-01.4.2 archive remains intake evidence only. Pinned main's intake audit explicitly classifies it as not implementation-ready. This package replaces that unresolved deliverable at the contract layer; it does not modify the repository.

## Authority and scope

Contract decisions are grounded in the pinned blobs listed in `REPOSITORY_EVIDENCE.json`. The repository-wide `AGENTS.md` and the complete uploaded Rust Skill were read before decisions were closed.

This contract governs:

- the first public resource extension-manifest transport and V1 DTOs;
- its sole direct-final decoder and sole canonical encoder;
- conversion to the current immutable `arcweft-resource-model` registry;
- package loading from explicit resolver-owned paths;
- deterministic AWFB publication of canonical extension manifests; and
- source-backed diagnostics, limits, atomicity, and tests.

It does not switch public `res` syntax authority. At the pinned revision, `res` still exists only in the private attached grammar; no final detached resource AST/HIR/sema owner exists. This manifest work remains a prerequisite for that later atomic switch.

## Closed decisions

### 1. Transport and direct-final dispatch

The authored transport is strict UTF-8 JSON. The exact root marker is `"arcweft.resource-type-manifest"`; the sole supported schema integer is `1`.

There is one public reader:

```text
arcweft_resource_manifest::decode_resource_type_manifest
```

It performs a lexical parse once, probes `format` and `schema`, dispatches directly to the V1 lowerer, and returns a source-backed typed result. There is no alias, provisional spelling, V0 reader, compatibility reader, migration shim, fallback branch, generic `Value` boundary, or second semantic parser.

The sole public canonical encoder is:

```text
arcweft_resource_manifest::encode_resource_type_manifest_v1
```

### 2. Exact top-level record

`ResourceTypeManifestFileV1` has exactly six required fields and admits no others:

```json
{
  "format": "arcweft.resource-type-manifest",
  "schema": 1,
  "package": { "id": "org.example.package", "version": "1.2.3" },
  "schemas": [],
  "resource_types": [],
  "codecs": []
}
```

Arrays may be empty because the current registry has a canonical empty publication and independently admits schema, type, and codec candidates. One document may publish any number of resource types. Exactly one selected document is admitted per exact package coordinate.

### 3. Package coordinate and normalization

`PackageCoordinateFile` is the object `{id, version}` and converts directly to current `arcweft_manifest_model::{PackageId, PackageVersion}`.

- `id` must be the current lowercase reverse-domain `PackageId`; it contains at least one dot and lower-kebab segments.
- `version` is parsed by current `semver::Version` through `PackageVersion` and encoded with its canonical `Display` spelling.
- No trimming, case folding, Unicode normalization, package alias, or relative coordinate is performed.
- The loader supplies the expected selected coordinate. The decoded coordinate must equal it exactly after typed construction.
- One `PackageId` may have only one selected `PackageVersion` in an aggregate publication.

### 4. Ownership and multi-document resolution

A new Sans-I/O library crate, `arcweft-resource-manifest`, owns neutral wire DTOs, strict JSON, source maps, diagnostics, canonical encoding, descriptor-claim verification, semantic conversion, and aggregate publication orchestration. It performs no filesystem access and no package discovery.

All manifests are decoded first. Their schemas, descriptors, and codecs are then combined with the supplied immutable base registry and submitted once to `ResourceTypeRegistry::publish`. Same-document and same-package forward references therefore resolve without ordering dependence. Cross-package references resolve only when their target exists in the supplied base registry or an explicitly selected dependency manifest. Failure returns diagnostics and publishes nothing.

### 5. Current closed model is authoritative

The wire covers every current variant, with exact spellings in `WIRE_SCHEMA.md`.

- `ResourceScalarType`: 14 variants.
- `ResourceValueType`: scalar, option, list, non-empty list, ordered map, nominal record, nominal enum, asset ref, exact resource ref, retained identity ref, constrained scalar.
- `ResourceConstValue`: scalar, option, sequence, map, record, enum, asset ref, exact resource ref, retained identity ref.
- retained identity: `character`, `view`, `action`, `layer`, `signal`, `presentation_target`, `scroll_region`.

Character, View, Action, Layer, Signal, presentation target, and scroll region never pass through `ResourceRef`. Presentation targets retain global/view scope plus `PublicId`; scroll regions retain the owning View `EntityId` plus region `PublicId`.

There is no byte scalar or byte constant in the current closed resource model. V1 therefore defines **no byte encoding**. `bytes`, base64, hexadecimal byte arrays, and byte-specialized lists are unknown tags or ordinary lists, never an implicit byte value. Binary payloads remain assets selected by `AssetRef` and `ResourceAssetPayloadKindId`.

### 6. Numeric and text determinism

- signed integer: canonical JSON integer in `i64` range;
- unsigned integer and duration: canonical JSON integer in `u64` range;
- nonzero field/variant/schema/codec/section versions: JSON integer in `1..=u32::MAX`;
- finite float: exact string `0x` plus 16 lowercase hexadecimal digits holding IEEE-754 binary64 bits;
- NaN and infinities: rejected;
- negative zero bits `0x8000000000000000`: rejected as non-canonical; positive zero is `0x0000000000000000`;
- strings: decoded Unicode scalar strings, no general normalization or trimming;
- char: exactly one Unicode scalar value;
- locale: current `LocaleId` validation/canonicalization, including canonical language/script/region casing;
- JSON null and JSON floating-point number tokens: rejected everywhere.

### 7. Strict errors

Duplicate object keys are rejected before DTO lowering with the duplicate key as primary range and the first key as related range. Duplicate semantic IDs/records are rejected even when their bodies are equal. Every object rejects unknown fields. Missing, explicit null, malformed, unsupported, wrong shape, unknown tag, and wrong tag/content are distinct diagnostics. Unsupported format/schema stops before V1 lowering; it never attempts a permissive decode.

### 8. Canonical bytes and digests

Canonical manifest bytes use current `arcweft_manifest_model::canonical_json_bytes` rules:

- compact UTF-8 JSON;
- object keys sorted by raw UTF-8 bytes;
- no whitespace or trailing newline;
- no null or floating number;
- semantic sets sorted by stable typed keys;
- authored list order retained only when order is semantic;
- ordered-map constants sorted by canonical wire bytes of the normalized key;
- empty documentation and absent optional values omitted.

Signatures consume these exact bytes. `RawDigest::for_bytes(canonical_bytes)` is the manifest content digest and cache-key digest. The package cache key is `(PackageId, PackageVersion, RawDigest)`.

Every descriptor carries a required `descriptor_digest`. Conversion constructs the current `ResourceTypeDescriptor`, recomputes an exact `ResourceTypeDescriptorDigest` using derive-key context `arcweft-resource-type-descriptor-v1` and the existing descriptor transcript, and rejects mismatch. The canonical encoder always writes the recomputed claim. Provenance and docs remain excluded, matching the current registry digest invariant.

The minimal model extension is an inherent `ResourceTypeDescriptor::semantic_digest()` plus typed `ResourceTypeDescriptorDigest`; the existing private descriptor encoder is reused. No duplicate digest helper, extension trait, or field-name switch is introduced.

### 9. Source ownership, limits, and atomicity

Accepted output stores one `Arc<SourceDocument>`, one typed manifest, and one source map. All decode and registry diagnostics use `arcweft_source::{SourceRange, SourceSpan}` tied to that exact document revision. Nested defaults and reference paths retain nested value ranges; duplicate and mismatch diagnostics retain related ranges.

Production limits are exact and inclusive:

| Counter | Maximum |
| --- | ---: |
| UTF-8 source bytes | 8,388,608 |
| lexical object/array depth, root = 1 | 64 |
| lexical nodes, including object keys | 65,536 |
| decoded UTF-8 bytes in one string | 1,048,576 |
| items in one array | 16,384 |
| members in one object | 4,096 |
| typed semantic records in one document | 16,384 |
| deterministic work units per document | 1,048,576 |

Limits are checked before or at the allocation/operation they bound. A failure returns no accepted document. Aggregate registry publication is all-or-nothing.

### 10. Package loading and bundle publication

The current strict project manifest gains one optional top-level field `resource-type-manifest` of `NormalizedProjectPath`. Dependency resolvers supply exact resource-manifest seeds with expected package id/version; no directory enumeration is allowed.

Canonical manifests are published in a required AWFB section whenever extensions are present:

- `BundleSectionKind::ResourceTypeManifests = 22`;
- section schema version `1`;
- startup residency, non-executable, content-only patch compatibility;
- exact binary framing in `PACKAGE_AND_ARTIFACT_PUBLICATION.md`.

Older readers that do not know required section code 22 reject the bundle instead of silently skipping extension types. The section reader frames bytes only; every embedded authored manifest goes through the same direct-final manifest reader and canonical encoder.

## Required implementation sequence

1. Add `arcweft-resource-manifest` with limits, wire identities/scalars, source map, DTOs, and diagnostics.
2. Implement every tagged record and freeze minimal/full canonical examples and digest vectors.
3. Implement the sole strict decoder and sole canonical encoder in that Sans-I/O crate.
4. Add the descriptor inherent digest API and convert all documents into one existing `ResourceRegistryPublication`; publish atomically.
5. Add explicit project/dependency loading and deterministic AWFB `ResourceTypeManifests` publication.
6. Complete all positive, negative, round-trip, tamper, determinism, budget, and package/bundle integration tests in `TEST_MATRIX.md`.

The public `res` syntax/HIR/sema switch remains later work. No production implementation may invent a second manifest spelling while waiting for that switch.

## Completion rule

Implementation is complete only when all focused and workspace gates in `TEST_MATRIX.md` pass, the structural dependency audit confirms the documented Sans-I/O edges, every current enum variant is exhaustively covered through typed APIs, and no source-scanning architecture test exists.
