# Diagnostics, source ranges, and resource limits

## Diagnostic surface

```rust
pub struct ResourceManifestDiagnostic {
    pub code: ResourceManifestDiagnosticCode,
    pub message: String,
    pub primary: SourceSpan,
    pub related: Box<[ResourceManifestRelatedSpan]>,
}

pub struct ResourceManifestRelatedSpan {
    pub label: String,
    pub span: SourceSpan,
}

pub struct ResourceManifestReport {
    diagnostics: Box<[ResourceManifestDiagnostic]>,
}
```

Diagnostics are sorted by `(document identity, primary.start, primary.end, code, related ranges)`. Within one decoder result the document identity is constant. Aggregate diagnostics sort package coordinates first, then the per-document key above. Messages are explanatory; code and typed payload are stable authority.

## Exact codes

| Enum | Stable code | Boundary |
| --- | --- | --- |
| `InvalidUtf8` | `resource_manifest.invalid_utf8` | loader byte-to-SourceDocument adapter |
| `BomNotAllowed` | `resource_manifest.bom_not_allowed` | lexical |
| `ByteLimit` | `resource_manifest.byte_limit` | lexical/source |
| `DepthLimit` | `resource_manifest.depth_limit` | lexical |
| `NodeLimit` | `resource_manifest.node_limit` | lexical |
| `StringLimit` | `resource_manifest.string_limit` | lowering |
| `CollectionLimit` | `resource_manifest.collection_limit` | lowering |
| `RecordLimit` | `resource_manifest.record_limit` | lowering/publication |
| `WorkLimit` | `resource_manifest.work_limit` | lowering/canonicalization/publication precharge |
| `InvalidJson` | `resource_manifest.invalid_json` | lexical syntax |
| `DuplicateKey` | `resource_manifest.duplicate_key` | lexical object |
| `RootWrongShape` | `resource_manifest.root_wrong_shape` | dispatch |
| `MissingFormat` | `resource_manifest.missing_format` | dispatch |
| `MalformedFormat` | `resource_manifest.malformed_format` | dispatch |
| `UnsupportedFormat` | `resource_manifest.unsupported_format` | dispatch |
| `MissingSchemaVersion` | `resource_manifest.missing_schema_version` | dispatch |
| `MalformedSchemaVersion` | `resource_manifest.malformed_schema_version` | dispatch |
| `UnsupportedSchemaVersion` | `resource_manifest.unsupported_schema_version` | dispatch |
| `UnknownField` | `resource_manifest.unknown_field` | closed object |
| `MissingField` | `resource_manifest.missing_field` | closed object |
| `NullNotAllowed` | `resource_manifest.null_not_allowed` | every field |
| `WrongShape` | `resource_manifest.wrong_shape` | every typed boundary |
| `UnknownTag` | `resource_manifest.unknown_tag` | closed enum |
| `WrongTagContent` | `resource_manifest.wrong_tag_content` | closed enum |
| `InvalidInteger` | `resource_manifest.invalid_integer` | integer lexical form |
| `IntegerOverflow` | `resource_manifest.integer_overflow` | typed integer range |
| `NonFiniteFloat` | `resource_manifest.non_finite_float` | float bit construction |
| `NonCanonicalFloat` | `resource_manifest.non_canonical_float` | negative zero or noncanonical bit text |
| `InvalidString` | `resource_manifest.invalid_string` | Unicode/char/string boundary |
| `InvalidId` | `resource_manifest.invalid_id` | typed ID constructor |
| `InvalidDigest` | `resource_manifest.invalid_digest` | digest text |
| `DuplicateRecord` | `resource_manifest.duplicate_record` | semantic duplicate key/ID/record |
| `PackageMismatch` | `resource_manifest.package_mismatch` | selected coordinate/provenance |
| `VersionConflict` | `resource_manifest.version_conflict` | aggregate package set |
| `UnresolvedPackage` | `resource_manifest.unresolved_package` | dependency resolver input |
| `DescriptorDigestMismatch` | `resource_manifest.descriptor_digest_mismatch` | typed descriptor conversion |
| `RegistryValidation` | `resource_manifest.registry_validation` | existing registry issue/default/type/reference validation |
| `ArtifactMalformed` | `resource_manifest.artifact_malformed` | AWFB manifest-set framing |
| `ArtifactNonCanonicalManifest` | `resource_manifest.artifact_non_canonical_manifest` | embedded bytes do not regenerate exactly |
| `ArtifactDigestMismatch` | `resource_manifest.artifact_digest_mismatch` | embedded raw digest |
| `RegistryDigestMismatch` | `resource_manifest.registry_digest_mismatch` | reconstructed final registry |

## Primary and related ranges

| Failure | Primary | Related |
| --- | --- | --- |
| duplicate JSON key | duplicate key token | first key token |
| unknown field | unknown key token | enclosing object only if needed for context |
| missing field | zero-width range at enclosing object's closing delimiter | object opening/name range |
| explicit null | null value | field key |
| wrong shape/tag/content | offending value or tag/content token | expected field/tag when useful |
| duplicate schema/type/codec | duplicate identity token | first identity token |
| duplicate field/variant ID/name | duplicate token | first token |
| duplicate map key/record field | duplicate nested key/field | first nested key/field |
| descriptor digest mismatch | claimed digest text | descriptor `type_id` range |
| package mismatch | document package field | expected resolver coordinate source/seed range when source-backed; otherwise typed related note |
| body/default/reference registry issue | narrowest offending nested value/type/reference | owning schema field/type/default range |
| family collision | second family token | first descriptor family token, potentially in another document |
| budget | token/range that crosses the limit, or whole source for byte limit | none |

Registry issue mapping is exhaustive over current `ResourceRegistryIssue` and `ResourceDefaultValidationError`. New issue variants must make the mapping match non-exhaustive compilation fail; no string parsing of `Display` messages is permitted.

## Production decode limits

```rust
pub struct ResourceManifestDecodeLimits {
    bytes: usize,
    nesting_depth: usize,
    lexical_nodes: usize,
    string_bytes: usize,
    collection_items: usize,
    object_members: usize,
    semantic_records: usize,
    work_units: u64,
}

pub const PRODUCTION: Self = Self::new(
    8_388_608,
    64,
    65_536,
    1_048_576,
    16_384,
    4_096,
    16_384,
    1_048_576,
);
```

Limits are inclusive. Exact measurement:

- `bytes`: exact input bytes before UTF-8 decoding; matches `MAX_REGISTRATION_SOURCE_BYTES`;
- `nesting_depth`: maximum simultaneously open `{`/`[` containers, with root object at depth 1;
- `lexical_nodes`: each object, array, scalar value, and object key charges one before semantic allocation;
- `string_bytes`: decoded UTF-8 byte length of each individual key or string value;
- `collection_items`: elements in each individual array;
- `object_members`: key/value pairs in each individual object, before duplicate collapse;
- `semantic_records`: every lowered package/schema/field/variant/descriptor/capabilities/lowering/codec/map-entry/record-field/tag-content record;
- `work_units`: deterministic precharged work described below.

The existing model's own 64-level value-type/default validation remains an additional semantic guard.

## Deterministic work charging

Work is charged by input structure, not wall time or actual comparator calls:

- 1 per lexical node revisited during typed lowering;
- 1 per typed ID/scalar/enum-tag construction;
- 1 per semantic record and collection element;
- 1 per value-type/default/reference edge submitted to registry validation;
- `n * ceil(log2(max(n, 2)))` for every semantically unordered collection before sorting;
- `ceil(encoded_bytes / 64)` for canonical JSON emission;
- `ceil(transcript_bytes / 64)` per descriptor digest;
- `ceil(section_bytes / 64)` for manifest-set artifact framing/verification.

Each charge is checked with `checked_add` before the operation. Overflow is `WorkLimit`. The registry's loops are precharged from the bounded candidate graph; no mutable registry is exposed during validation.

## Inclusive/one-over test rule

Every counter has a focused test at exactly the configured maximum and one unit over. Large production-byte tests may use generated buffers; smaller custom limit instances test structure cheaply. In all one-over cases:

- the exact limit code is returned;
- the primary range identifies the crossing token when one exists;
- no accepted document is returned;
- aggregate publication leaves the prior registry `Arc` unchanged;
- no bundle section or cache entry is emitted.
