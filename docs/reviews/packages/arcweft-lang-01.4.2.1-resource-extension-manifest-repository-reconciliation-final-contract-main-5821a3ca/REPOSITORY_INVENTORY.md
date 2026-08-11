# Repository inventory at `5821a3ca479b5b89ca6ede997b9cf4f42f6280a6`

## Policy and architecture

The applicable root `AGENTS.md` is repository-wide. No more-specific `AGENTS.md` was found for the inspected crate paths. Its controlling rules for this contract are:

- core/model/data-format layers remain Sans I/O;
- filesystem and package discovery stay in adapters/loaders;
- unreleased/internal formats are replaced directly, not preserved with aliases or dual readers;
- missing behavior on an Arcweft-owned type is added as an inherent method on that original type;
- deterministic typed APIs are preferred over string switches and generic values;
- architecture tests use typed APIs or `cargo metadata`, never source/path scans;
- workspace dependencies are centralized;
- ZIP intake records must be reconciled before work resumes.

The complete uploaded Rust Skill additionally requires idiomatic Rust, cautious visibility/macros/allow attributes, no unsafe/`Box::leak`/`forget`, library-first APIs, no `mod.rs` layout, newtypes for domain identities, serde symmetry where public DTOs use serde, `cargo clippy --all-targets`, and `cargo fmt` at implementation completion.

## Current `res` path

| Layer | Current repository fact | Contract consequence |
| --- | --- | --- |
| syntax kind | `SyntaxKind::ResourceDeclarationItem`, `ResourceBody`, and `ResourceFieldInitializer` are private attached grammar vocabulary | Do not claim a public resource syntax owner |
| parser | `resource_grammar.rs` recognizes `res`, optional absolute public ID, local name, `:`, nominal type path, and `{ field = expression }` | Manifest schema does not redesign source syntax |
| detached AST/HIR/sema | no final public resource declaration owner exists; readiness notes still mark the public switch blocked | Manifest work ends at typed registry publication and remains a prerequisite |
| compiler | compilation context already carries `Arc<ResourceTypeRegistry>` | Loader publishes exactly one immutable registry for existing consumers |

Current syntax diagnostics include `syntax.resource.relative_declaration_id`, `missing_name`, `missing_colon`, `missing_type`, `invalid_type_head`, `missing_body`, `malformed_field`, and `missing_initializer`. They are unrelated to the new JSON decoder and must not be reused as wire diagnostics.

## `arcweft-resource-model` identity inventory

`identity.rs` owns:

- `ResourceIdentityClass`: `ModulePath`, `TypeName`, `SchemaId`, `FieldId`, `FieldName`, `VariantId`, `VariantName`, `AssetPayloadKindId`, `CodecId`, `Family`, `FamilyGroupId`, `BundleSectionId`, `RuntimeHandleKindId`, `DescriptorSourceId`, `SchemaVersion`, `CodecVersion`, `BundleSectionVersion`;
- `ResourceIdentityErrorKind`: `Empty`, `NonCanonical`, `Zero`;
- `ResourceIdentityError`;
- `ResourceModulePath`;
- `ResourceTypeName`;
- `NominalTypeId { package: PackageId, module: ResourceModulePath, name: ResourceTypeName }`;
- `ResourceTypeId(NominalTypeId)`;
- stable text IDs: `ResourceSchemaId`, `ResourceFieldName`, `ResourceVariantName`, `ResourceAssetPayloadKindId`, `ResourceCodecId`, `ResourceFamilyGroupId`, `ResourceBundleSectionId`, `ResourceRuntimeHandleKindId`, `ResourceDescriptorSourceId`;
- `ResourcePublicIdFamily`;
- nonzero-u32 IDs: `ResourceFieldId`, `ResourceVariantId`, `ResourceSchemaVersion`, `ResourceCodecVersion`, `ResourceBundleSectionVersion`;
- `ResourceDeclarationIdentity { EntityId, PublicId, ResourceTypeId }`.

Exact construction rules:

- module paths are dot-separated Arcweft identifiers;
- type, field, and variant names begin with `_` or a Unicode alphabetic scalar; remaining scalars are `_`, Unicode alphabetic, or ASCII digits;
- stable dotted IDs consist of lowercase ASCII segments that begin with a letter, end with a letter/digit, and otherwise admit lowercase letters, digits, `_`, and `-`;
- a public-ID family is one stable segment and contains no dot;
- descriptor source text is nonempty, trim-stable, and control-free;
- numeric IDs and versions reject zero.

## Descriptor, field, schema, capability, and codec inventory

`descriptor.rs` owns:

- `ResourceFieldPresence::{Required, Optional}`;
- `ResourceFieldDescriptor { id, name, value_type, presence, default, docs }`;
- `ResourceVariantDescriptor { id, name, payload, docs }`;
- `ResourceRecordSchema { id, nominal_type, version, fields }`;
- `ResourceEnumSchema { id, nominal_type, version, variants }`;
- `ResourceValueSchema::{Record, Enum}` and `ResourceValueSchemaKind`;
- `ResourceAgentExposure::{Hidden, Catalog, CatalogAndRuntime}`;
- `ResourceHotReloadClass::{RestartRequired, ReplaceDefinition, UpdateLiveHandle}`;
- `ResourceCapabilities { runtime_handle_kind, agent_exposure, save_definition_reference, hot_reload }`;
- `ResourceLoweringBinding { codec_id, codec_version, section_id, section_version }`;
- `ResourceCodecSupport { codec_id, versions: BTreeSet<ResourceCodecVersion> }`;
- `ResourceTypeDocs { summary }`;
- `ResourceDescriptorProvenance { package, source }`;
- `ResourceTypeDescriptor { provenance, type_id, public_id_family, family_group, body_schema, capabilities, lowering, docs }`;
- `ResourceCapabilityError::{AgentRuntimeWithoutHandle, LiveHotReloadWithoutHandle}`.

Field and variant arrays are canonicalized by numeric ID and then name. Descriptor documentation and provenance are intentionally excluded from semantic registry digests.

## Constraint, value-type, scalar, and constant inventory

`value.rs` owns the closed scalar inventory:

`Unit`, `Bool`, `SignedInteger`, `UnsignedInteger`, `Float`, `String`, `Char`, `Duration`, `Ratio`, `Length`, `Gain`, `Pan`, `Locale`, `PublicId`.

Concrete scalar owners and constraints are:

- `ResourceFloat(u64)`: only finite `f64`; `-0.0` is normalized to `+0.0`;
- `ResourceRatio(u32)`: millionths in `0..=1_000_000`;
- `ResourceLength { milli_units: i64, unit: LayoutUnit }`;
- `LogicalDuration`: `u64` nanoseconds;
- `GainDbMilli`: `i32`, `-120_000..=24_000`;
- `PanMilli`: `i16`, `-1_000..=1_000`;
- `LocaleId`: canonical ASCII BCP-47, at most 64 bytes;
- `PublicId`;
- `ResourceBoundKind::{Inclusive, Exclusive}`;
- `ResourceScalarBound`;
- `ResourceScalarConstraint`;
- `ResourceConstraintError::{BoundTypeMismatch, Inverted, Empty}`.

`ResourceValueType` variants are exactly:

1. `Scalar`
2. `Option`
3. `Vec`
4. `NonEmptyVec`
5. `Map { key, value }`
6. `NominalRecord`
7. `NominalEnum`
8. `AssetRef { payload_kind }`
9. `ResourceRef { type_id }`
10. `RetainedIdentityRef { identity }`
11. `ConstrainedScalar`

Constant containers and values are:

- `ResourceAssetRefValue { public_id, payload_kind }`;
- `ResourceRefValue { entity, public, resource_type }`;
- `ResourceMapValue(BTreeMap<ResourceConstValue, ResourceConstValue>)`;
- `ResourceRecordValue { schema_id, fields: BTreeMap<ResourceFieldId, ResourceConstValue> }`;
- `ResourceEnumValue { schema_id, variant, payload }`;
- `ResourceConstValue::{Scalar, Option, Sequence, Map, Record, Enum, AssetRef, ResourceRef, RetainedIdentityRef}`;
- `ResourceConstValueKind` with the same coarse categories;
- `ResourceConstConstructionError::{DuplicateMapKey, DuplicateRecordField}`;
- `ResourceValueValidationError::{TypeMismatch, EmptyNonEmptyVec, ConstraintViolation, NestingTooDeep, RetainedIdentityKindMismatch, Nested}`;
- `ResourceValidationPathSegment::{OptionValue, SequenceIndex, MapKey, MapValue, RecordField, EnumPayload}`.

There is no byte scalar, byte constant, or byte reference variant. `MAX_RESOURCE_VALUE_NESTING` is 64.

## Retained-reference inventory

`RetainedIdentityKind::ALL` is exactly:

| Rust variant | Manifest token | Resolved payload |
| --- | --- | --- |
| `Character` | `character` | `EntityId` |
| `View` | `view` | `EntityId` |
| `Action` | `action` | `EntityId` |
| `Layer` | `layer` | `EntityId` |
| `Signal` | `signal` | `EntityId` |
| `PresentationTarget` | `presentation_target` | `PresentationTargetScope` plus target `PublicId` |
| `ScrollRegion` | `scroll_region` | owner View `EntityId` plus region `PublicId` |

`PresentationTargetScope` is `Global` or `View { owner_view_entity_id }`. These categories are semantically disjoint from `ResourceRef` and `AssetRef`.

## Registry, digest, limit, and diagnostic inventory

`registry.rs` defines `RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION = 1`, `ResourceRegistryPublication`, `ResourceSchemaDigest`, `ResourceTypeRegistryDigest`, immutable `ResourceTypeRegistry`, `ResourceRegistryPublicationError`, `ResourceRegistryIssue`, `ResourceDefaultValidationError`, and `ResourceRegistryIntegrityError`.

`ResourceRegistryIssue` variants are exactly:

- `UnsupportedManifestSchemaVersion`
- `DuplicateCodec`
- `CodecWithoutVersions`
- `DuplicateType`
- `DuplicateSchema`
- `DuplicateNominalSchema`
- `DuplicateFieldId`
- `DuplicateFieldName`
- `DuplicateVariantId`
- `DuplicateVariantName`
- `RequiredFieldHasDefault`
- `InvalidFieldDefault`
- `UnknownValueSchema`
- `ValueSchemaKindMismatch`
- `UnknownResourceReferenceType`
- `UnknownBodySchema`
- `BodySchemaNotRecord`
- `BodySchemaNominalTypeMismatch`
- `ProvenancePackageMismatch`
- `FamilyCollision`
- `MissingCodec`
- `UnsupportedCodecVersion`
- `InvalidCapabilities`
- `ValueTypeNestingTooDeep`

`ResourceDefaultValidationError` variants are `Structural`, `UnknownRecordField`, `MissingRecordField`, `UnknownEnumVariant`, `EnumPayloadPresence`, `Nested`, and `NestingTooDeep`. Integrity errors are `MissingSchemaDigest`, `UnexpectedSchemaDigest`, `SchemaDigestMismatch`, and `RegistryDigestMismatch`.

Current digest contexts are:

- `arcweft-resource-value-schema-v1`;
- `arcweft-resource-type-registry-v1`.

The current canonical registry transcript uses little-endian fixed-width integers, one-byte enum tags, unsigned LEB128 lengths, raw UTF-8 strings, and canonical key-byte ordering for maps. The descriptor portion includes type ID, public-ID family, family group, body schema, capabilities, and lowering; it excludes docs and provenance.

Publication first canonicalizes and sorts all candidates, accumulates deterministic issues, and returns a registry only after the complete candidate set is valid. This is the atomic publication substrate the manifest layer must use unchanged.

## Existing strict decoder and source-range patterns

- `arcweft-launch` owns one source-backed Taplo syntax-tree decoder and typed source map for the strict project manifest.
- `arcweft-adapter-metadata::strict_json` owns a duplicate-preserving spanned JSON tree, lexical limits, nested paths, and typed lowering. It uses 8,388,608 bytes, depth 64, and 65,536 nodes.
- `arcweft-source` owns `SourceDocument`, `SourceRange`, `SourceSpan`, document revisions, and the 8,388,608-byte registration boundary.
- `arcweft-manifest-model::canonical_json_bytes` emits compact UTF-8 JSON with object keys sorted by UTF-8 bytes and rejects null/floating values.

These are patterns and lower-level owners, not alternate resource-manifest readers.

## Current package/build/bundle path

- `ArcweftManifestDocument` is the strict root project manifest and currently has no resource extension-manifest path.
- `ProfileTopologyLoadRequest` and `ProfileDependencyResourceSeed` load explicitly supplied normalized resources; directory enumeration is not part of the topology contract.
- `LoadedProfileTopologyResource` retains exact text in `Arc<SourceDocument>`.
- compiler registration already accepts `Arc<ResourceTypeRegistry>`.
- `BundleSectionKind` has typed codes 1 through 21; unknown required sections reject and optional unknown sections may be skipped.

Therefore package discovery belongs in `arcweft-project-loader`, registry semantics stay in `arcweft-resource-model`, the new wire codec belongs in a Sans-I/O crate, and deterministic section framing belongs in `arcweft-bundle`.
