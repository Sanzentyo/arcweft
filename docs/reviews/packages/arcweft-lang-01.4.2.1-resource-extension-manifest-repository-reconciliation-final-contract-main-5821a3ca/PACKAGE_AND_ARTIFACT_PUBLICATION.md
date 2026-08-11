# Package loading and deterministic artifact publication

## Root package declaration

Extend the current strict `ArcweftManifestDocument` with:

```rust
#[serde(default)]
pub resource_type_manifest: Option<NormalizedProjectPath>,
```

With the root manifest's kebab-case wire policy, the exact TOML key is:

```toml
resource-type-manifest = "resource-types.json"
```

It is singular. One selected package coordinate has at most one resource extension-manifest document. The field is optional because packages need not publish extension types.

## Explicit dependency seeds

Add a typed topology kind on the existing enum, not a side-channel label:

```rust
ProfileTopologyResourceKind::ResourceTypeManifest {
    package_id: PackageId,
    package_version: PackageVersion,
}
```

Workspace input derives the expected coordinate from the already accepted root `PackageSpec`. Dependency resolvers provide exact owner-qualified `ProfileDependencyResourceSeed` resources with the selected package id/version and normalized logical path. The loader never scans a directory for `*.json` or guesses a conventional filename.

A dependency seed with absent bytes/path is `UnresolvedPackage`; an admitted document whose coordinate differs is `PackageMismatch`. Two versions of one package ID in the selected graph are `VersionConflict` before registry publication.

## Loader transaction

Extend `ProfileTopologyLoadRequest` with an immutable engine/base `Arc<ResourceTypeRegistry>`, defaulting explicitly to `ResourceTypeRegistry::empty()` only at call sites that truly have no built-ins.

Loader sequence:

1. accept the strict root project manifest;
2. resolve its explicit resource-manifest path with existing containment/normalized-path rules;
3. accept dependency resources supplied by the resolver;
4. create one `SourceDocument` per exact UTF-8 source;
5. decode every document through `decode_resource_type_manifest` with its expected coordinate;
6. sort accepted documents by `(PackageId, PackageVersion)`;
7. combine base registry candidates and every document candidate;
8. call `ResourceTypeRegistry::publish` once;
9. return the accepted manifest set and `Arc<ResourceTypeRegistry>` as part of the immutable loaded topology.

Failure at any step returns no new loaded topology transaction. The caller's previous base registry remains untouched.

## Compiler handoff

`AcceptedLaunchProfileInput` and `ProjectCompilationContext` already carry `Arc<ResourceTypeRegistry>`. Replace affected construction sites with the loader-published `Arc`; do not add a parallel manifest lookup to syntax/HIR/sema or a global singleton. Existing compiler/view code continues consuming only the typed registry and its digest.

## AWFB section kind

Add the enum variant to the original `BundleSectionKind` implementation:

```rust
ResourceTypeManifests // encoded u32 22
```

Update all exhaustive inherent methods in the same owner:

- `encoded() -> 22`;
- `from_encoded(22)`;
- `is_executable() -> false`;
- `default_residency() -> Startup`;
- `patch_default_compatibility() -> ContentOnly`;
- not a universally required Program section;
- when emitted, its section descriptor is **required**, so an older runtime rejects unknown code 22 rather than skipping it.

Patch bundles do not directly contain this section. Program, AgentController, and ContentPack may contain it when their selected extension set is nonempty.

## `ResourceTypeManifests` section V1 bytes

The section descriptor schema version is exactly `1`. Decoded section bytes are:

| Offset/sequence | Encoding |
| --- | --- |
| 0 | 8-byte magic `41 57 52 4d 0d 0a 1a 0a` (`AWRM\r\n\x1a\n`) |
| 8 | `u32` little-endian internal schema version `1` |
| 12 | `u32` little-endian manifest count |
| 16 | 32 raw bytes of final `ResourceTypeRegistryDigest` semantic digest |
| then, per entry | `u64` little-endian canonical manifest byte length |
| | 32 raw bytes of `RawDigest::for_bytes(canonical_manifest)` |
| | exact canonical manifest UTF-8 bytes |
| end | no padding and no trailing bytes |

Entries are strictly sorted by decoded `(PackageId, PackageVersion)`. Duplicate coordinates and multiple versions of one package ID are invalid. Count is bounded by the semantic-record limit; each embedded manifest is bounded by 8,388,608 bytes; total section bytes remain subject to existing AWFB read budget and section decoded-size checks.

## Section encoding

1. Take the exact accepted source-backed manifest set, already sorted.
2. Re-run the sole canonical encoder from typed values; never reuse unverified authored bytes.
3. Compute each `RawDigest` over those exact bytes.
4. Write header and entries without padding.
5. Write final registry semantic digest from the already published immutable registry.
6. Let existing AWFB section content/stored digests and signatures cover the entire section.

The section is omitted when no selected extension manifest exists. A bundle that references an extension type without the section fails runtime registry/type validation; this is not a fallback to an empty registry.

## Section decoding

1. Existing AWFB reader validates bounds, required-section semantics, storage transform, and section digest.
2. The manifest-set reader checks magic, both schema-version carriers, count, lengths, and no trailing bytes.
3. Check each entry raw digest before JSON decode.
4. Decode through the sole direct-final manifest reader using the coordinate in the document as the expected artifact coordinate.
5. Canonically encode and require byte-for-byte equality with embedded bytes.
6. Require strict coordinate ordering and uniqueness.
7. Aggregate with the runtime's engine base registry and publish once.
8. Compare the reconstructed final registry digest with the 32-byte header digest.
9. Expose the registry only after every check succeeds.

This wrapper is a binary collection framing, not an alternate authored manifest reader and not a compatibility path.

## Cache and signature identity

- authored manifest canonical bytes are the signature message supplied to the signing layer;
- per-manifest cache key is `(PackageId, PackageVersion, RawDigest)`;
- descriptor claim is `ResourceTypeDescriptorDigest` with its own derive-key context;
- final semantic identity is current `ResourceTypeRegistryDigest`;
- bundle storage/content identity remains existing `BundleDigest` over exact section bytes.

These identities are not interchangeable and use their current typed wrappers.
