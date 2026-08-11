# DTO, decoder, source-map, and semantic conversion contract

## New crate and module layout

Add workspace library crate `crates/arcweft-resource-manifest` with no `main.rs`, no filesystem API, and no `mod.rs` files:

```text
src/lib.rs
src/limits.rs
src/wire.rs
src/strict_json.rs
src/source_map.rs
src/diagnostic.rs
src/decode.rs
src/encode.rs
src/convert.rs
src/publication.rs
```

`strict_json.rs` owns a private duplicate-preserving spanned lexical tree. It must not expose `serde_json::Value`. `wire.rs` owns only closed DTO enums/records. `convert.rs` invokes current constructors. `publication.rs` aggregates accepted documents with a supplied base registry and calls the existing registry publisher once.

## Public API

```rust
pub fn decode_resource_type_manifest(
    document: Arc<SourceDocument>,
    expected: &PackageCoordinateFile,
    limits: ResourceManifestDecodeLimits,
) -> Result<SourceBackedResourceTypeManifestV1, ResourceManifestReport>;

pub fn encode_resource_type_manifest_v1(
    manifest: &TypedResourceTypeManifestV1,
) -> Result<Vec<u8>, ResourceManifestEncodeError>;

pub fn publish_resource_type_manifests_v1(
    base: &ResourceTypeRegistry,
    manifests: impl IntoIterator<Item = SourceBackedResourceTypeManifestV1>,
    limits: ResourceManifestPublicationLimits,
) -> Result<PublishedResourceTypeManifestSetV1, ResourceManifestReport>;
```

No public `decode_v1`, legacy reader, untyped parse result, or alternate encoder is exposed. The public decoder performs the strict format/version dispatch. An internal `lower_v1` is not a second reader.

## Core DTO shapes

The following Rust shapes are normative at the field/variant level; implementation may choose boxes/owned strings needed for size without changing semantics.

```rust
pub struct PackageCoordinateFile {
    pub id: PackageId,
    pub version: PackageVersion,
}

pub struct NominalTypeIdFile {
    pub package: PackageId,
    pub module: ResourceModulePath,
    pub name: ResourceTypeName,
}

pub struct ResourceTypeManifestFileV1 {
    pub format: ResourceTypeManifestFormatV1,
    pub schema: ResourceTypeManifestSchemaV1,
    pub package: PackageCoordinateFile,
    pub schemas: Vec<ResourceValueSchemaFileV1>,
    pub resource_types: Vec<ResourceTypeDescriptorFileV1>,
    pub codecs: Vec<ResourceCodecSupportFileV1>,
}
```

Wire enum DTOs use exact adjacent tags and content from `WIRE_SCHEMA.md`; they are never represented as raw label strings after lowering:

```rust
pub enum ResourceValueTypeFileV1 {
    Scalar(ResourceScalarTypeFileV1),
    Option(Box<Self>),
    List(Box<Self>),
    NonEmptyList(Box<Self>),
    OrderedMap { key: Box<Self>, value: Box<Self> },
    Record(ResourceSchemaId),
    Enum(ResourceSchemaId),
    AssetRef { payload_kind: ResourceAssetPayloadKindId },
    ResourceRef { type_id: NominalTypeIdFile },
    RetainedIdentityRef(RetainedIdentityKind),
    ConstrainedScalar(ResourceScalarConstraintFileV1),
}

pub enum ResourceConstValueFileV1 {
    Scalar(ResourceScalarValueFileV1),
    Option(Option<Box<Self>>),
    List(Vec<Self>),
    OrderedMap(Vec<ResourceMapEntryFileV1>),
    Record(ResourceRecordValueFileV1),
    Enum(ResourceEnumValueFileV1),
    AssetRef(ResourceAssetRefValueFileV1),
    ResourceRef(ResourceRefValueFileV1),
    RetainedIdentityRef(ResolvedRetainedIdentityRefFileV1),
}
```

`ResourceScalarTypeFileV1`, `ResourceFieldPresenceFileV1`, `ResourceAgentExposureFileV1`, `ResourceHotReloadClassFileV1`, `ResourceBoundKindFileV1`, `RetainedIdentityKindFileV1`, `PresentationTargetScopeFileV1`, and layout-unit wire enums are exhaustive closed enums. Their `from_wire_token`/`as_wire_token` behavior belongs on those wire enums. Current Arcweft model enum token behavior already present, such as `RetainedIdentityKind::from_manifest_token`, is reused rather than wrapped by string switches.

## Source-backed accepted shape

```rust
pub struct SourceBackedResourceTypeManifestV1 {
    document: Arc<SourceDocument>,
    file: ResourceTypeManifestFileV1,
    typed: TypedResourceTypeManifestV1,
    source_map: ResourceManifestSourceMap,
    canonical_bytes: Arc<[u8]>,
    canonical_digest: RawDigest,
}
```

The accepted product is constructed only after lexical, DTO, semantic, descriptor-digest, and per-document package checks succeed. The aggregate registry may still reject cross-document collisions/references; accepted documents remain immutable inputs to that atomic operation.

## Lexical and semantic source maps

The lexical source map follows the repository's strict JSON pattern:

```rust
pub struct JsonPath(Box<[JsonPathSegment]>);

pub enum JsonPathSegment {
    Field(Box<str>),
    Index(u32),
}

pub struct JsonTokenRange {
    pub key: Option<SourceRange>,
    pub value: SourceRange,
}

pub struct ResourceManifestSourceMap {
    lexical: BTreeMap<JsonPath, JsonTokenRange>,
    schemas: BTreeMap<ResourceSchemaId, ResourceSchemaSource>,
    resource_types: BTreeMap<ResourceTypeId, ResourceTypeSource>,
    codecs: BTreeMap<ResourceCodecId, ResourceCodecSource>,
}
```

Semantic source records retain the enclosing record range and exact identity/type/default/tag/content ranges. Nested constant source paths use the existing semantic path concepts: option value, sequence index, map key/value index, record field ID, and enum payload. Duplicate records retain the first and duplicate identity ranges separately.

`SourceRange` remains document-relative. Diagnostics create `SourceSpan` by pairing it with the exact accepted `SourceDocument` revision. A related span from another manifest carries that other document's revision; spans are never re-based onto a synthesized aggregate document.

## One-pass decode stages

1. Adapter creates `SourceDocument` from exact UTF-8 bytes. Invalid UTF-8 is reported before the Sans-I/O decoder.
2. `strict_json` checks byte/BOM/lexical depth/node limits and produces one duplicate-preserving spanned tree.
3. Root must be an object. `format` and `schema` are probed without discarding duplicate/range information.
4. Unsupported format/schema returns the corresponding dispatch diagnostic and does not run V1 lowering.
5. V1 lowering checks closed shapes, unknown/missing/null/wrong-tag behavior, per-string/collection/record/work budgets, and constructs typed wire identities/scalars.
6. Conversion constructs current `ResourceValueSchema`, `ResourceTypeDescriptor`, and `ResourceCodecSupport` values. Provenance is derived, never read from JSON.
7. Each descriptor claim is recomputed and checked.
8. The typed file is canonicalized and encoded once. Canonical bytes and `RawDigest` are retained.
9. If any diagnostic exists, no accepted object is returned.

## Exact conversion map

| Wire | Current semantic target |
| --- | --- |
| `PackageCoordinateFile` | `PackageId` + `PackageVersion` selected package coordinate |
| `NominalTypeIdFile` | `NominalTypeId`, wrapped as `ResourceTypeId` where required |
| scalar type/value | current `ResourceScalarType` / `ResourceScalarValue` constructors |
| finite float bits | `ResourceFloat::try_new(f64::from_bits(bits))`, with pre-rejection of negative zero |
| duration | `LogicalDuration::from_nanos` or current exact constructor |
| length | `ResourceLength::new` with current `LayoutUnit` |
| gain/pan | current checked `GainDbMilli` / `PanMilli` constructors |
| locale | `LocaleId::try_new` |
| option/list/map/record/enum | current `ResourceConstValue` containers and checked map/record constructors |
| asset ref | `ResourceAssetRefValue::new` |
| exact resource ref | `ResourceRefValue::new` |
| retained ref | current `ResolvedRetainedIdentityRef` variant; no intermediate `ResourceRef` |
| field/variant/schema/descriptor | current descriptor module constructors |
| codec | `ResourceCodecSupport::new` with a nonempty `BTreeSet` |

## Required minimal resource-model additions

Current registry digest code already owns the exact descriptor transcript but exposes only schema and whole-registry digests. A manifest claim needs a typed per-descriptor API. Add:

```rust
pub struct ResourceTypeDescriptorDigest(SemanticDigest);

impl ResourceTypeDescriptor {
    pub fn semantic_digest(&self) -> ResourceTypeDescriptorDigest;
}
```

Use derive-key context `arcweft-resource-type-descriptor-v1`. Reuse the existing `encode_descriptor` transcript in `registry/digest.rs`; make only the minimum crate-visible path needed by the inherent method. Do not duplicate the transcript in the manifest crate, add an extension trait, or compare selected fields ad hoc.

Add the missing inherent registry iterator:

```rust
impl ResourceTypeRegistry {
    pub fn codecs(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ResourceCodecId, &ResourceCodecSupport)>;
}
```

`schemas()` and `types()` already exist. The loader can therefore clone the supplied base registry candidates, append all manifest candidates, and call `ResourceTypeRegistry::publish` once. No registry redesign or mutable extension API is needed.

## Aggregate publication result

```rust
pub struct PublishedResourceTypeManifestSetV1 {
    manifests: Box<[SourceBackedResourceTypeManifestV1]>,
    registry: Arc<ResourceTypeRegistry>,
    registry_digest: ResourceTypeRegistryDigest,
}
```

Manifests are stored sorted by `(PackageId, PackageVersion)`. Duplicate exact coordinates and multiple versions for one package ID are diagnosed before registry publication. `registry` is constructed only after all selected documents, base entries, and reference/default/capability checks succeed.
