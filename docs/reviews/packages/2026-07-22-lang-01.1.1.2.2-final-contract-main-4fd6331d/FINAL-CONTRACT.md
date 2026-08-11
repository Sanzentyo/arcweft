# Normative Final Contract

## 1. Scope and normative language

This document is the final contract for Lang-01.1.1.2.2 at `main@4fd6331dc342d30a7f4ac7774852b60801866ef7`. “Must”, “must not”, “is”, and “is not” are normative. The design below is the only accepted implementation design for this request.

The correction covers:

- Rust ABI nominal type carriers;
- adapter manifest nominal type carriers;
- source-backed registration facts;
- accepted-world projection;
- callable publication;
- Rust ADT/enum metadata;
- callable/tooling/persistent identities; and
- atomic registered-world admission.

It does not modify production code in this package.

## 2. Repository-confirmed defect

At the pinned baseline, the adapter/Rust path can publish a nominal as `TypeKind::Named(String)` while the authored `extern` path resolves the same accepted export as `TypeKind::AcceptedNominal(AcceptedNominalType)`. The checked callable schema uses exact `TypeKind` equality, and method keys contain the receiver `TypeKind`; therefore the two paths are semantically unequal even when their display spelling is equal.

The repository also confirms these ordering facts:

1. `AcceptedNominalWorld` is constructed before the callable catalog.
2. `RegisteredCallableCatalogBuilder::add_project` already receives that world for source-backed project signature resolution.
3. environment publications are currently constructed before registration and then admitted later.
4. `arcweft-lang-sema` does not depend on `arcweft-adapter-context`; the optional dependency direction is adapter-context → sema.
5. the registered-world operation already returns a complete new world or an error report, so the correction must remain inside that transaction.

The missing boundary is therefore the conversion of typed manifest references into accepted-world semantic identities after the world exists and before a final environment publication exists.

## 3. Selected identity model

### 3.1 Rust nominal owner

A Rust-exported nominal has exactly this semantic owner:

```rust
AcceptedNominalOwnerId::RustPackage(RustPackageId)
```

The `RustPackageId` is derived from the validated `ArcweftRustPackageId` in the Rust ABI manifest. The selected adapter is:

- the callable provider;
- the source-document owner;
- the Rust package mount authority; and
- part of Rust callable provenance.

The selected adapter is **not** the semantic owner of a Rust nominal. No API may substitute `EnvironmentBindingId`, `AdapterPackageId`, the adapter display name, or the callable provider for the Rust package owner.

### 3.2 Adapter-native nominal owner

An adapter-native or mounted external-module nominal is a different declaration class. Its owner is:

```rust
AcceptedNominalOwnerId::Environment(EnvironmentBindingId)
```

The binding ID is deterministically derived from the manifest owner as `adapter:<adapter-id>`. A reference to this declaration carries that owner explicitly. It cannot alias a Rust nominal even if owner-independent display paths are equal.

This is one typed model: every adapter publication nominal reference carries a complete accepted owner, a complete accepted path, and recursively typed arguments. No owner is guessed from the callable, terminal name, selected package, or registration order.

### 3.3 Exact accepted ID

Before sema projection, the adapter-context boundary constructs:

```rust
AcceptedNominalId {
    owner: AcceptedNominalOwnerId,
    canonical_path: TypePath,
}
```

The ID is only a candidate until validated against the exact `AcceptedNominalWorld`. Projection validates it and then creates the existing `AcceptedNominalType`; it does not run a second name resolver.

## 4. Path and mount model

### 4.1 Rust ABI path

`ArcweftRustTypePath` is a validated, non-empty, package-local sequence of segments. It has no source-language root, `self`, `super`, suffix matching, or terminal-name semantics.

### 4.2 Adapter accepted path

`AdapterNominalPath` is a validated, non-empty, world-visible path under `ModulePathRoot::ImplicitCrate`. It is converted exactly to `TypePath`; no string parser runs after manifest validation.

### 4.3 Rust package mount

Each adapter manifest owns one `AdapterRustPackageMountTable`. It maps every referenced `ArcweftRustPackageId` to an `AdapterNominalPathPrefix`. All mounts must be registered before a Rust manifest is ingested.

For Rust package `P`, mount prefix `M`, and package-local type path `L`, the accepted path is:

```text
M.segments + L.segments
```

An empty prefix is legal and preserves the current one-segment public surface where no collision exists. The resulting path must be non-empty and within the existing nominal path limits.

A package may have only one prefix in one adapter manifest. The same package ID appearing through multiple selected adapters must have the same version and metadata hash and must map to the same accepted path set in one registered world. A conflict is a structured registration failure.

### 4.4 Collision rule

`AcceptedNominalCatalog` remains the exact, global path catalog already present in sema. Consequently:

- equal full accepted paths collide even when owners differ;
- equal terminal segments do not collide when full accepted paths differ;
- package owner equality does not authorize two records at one full path;
- no suffix, leaf, or display-name lookup is added.

Example:

```text
owner RustPackage("alpha"), path vendor.alpha.Rank   // accepted
owner RustPackage("beta"),  path vendor.beta.Rank    // accepted
project declaration          crate::Rank              // separate ProjectNominal identity
```

Mapping both Rust packages to `vendor.shared.Rank` fails as a duplicate accepted export.

## 5. Publication boundary

### 5.1 Input, not final publication

`SourceBackedAdapterRegistrationFacts` must no longer create `EnvironmentCallablePublication`. It emits sema-owned `SourceBackedEnvironmentRegistrationInput` containing:

- the manifest/callable owner;
- the generated source identity and exact type-node spans;
- a canonical manifest digest;
- visible and inaccessible nominal inventory;
- Rust ADT metadata inputs; and
- callable publication record inputs whose type trees contain exact `AcceptedNominalId` candidates.

`ProjectRegistrationFacts::try_new` binds those inputs to one symbol world and revision and validates that every source span belongs to the supplied source set.

### 5.2 Accepted-world projection

After the accepted catalog and external owner registry have been finalized, `CharacterRegistrar` constructs `AcceptedNominalWorld`. It then invokes the world-owned projection operation for every environment registration input.

Projection must cover, recursively and before record construction:

- method receivers;
- every parameter in every parameter group;
- the result type;
- every `Vec`, `Seq`, `Option`, `Result`, and tuple child;
- every accepted nominal argument; and
- every Rust struct field, enum payload field, and newtype inner type.

A successful projection produces only final `TypeKind` values. A failed projection produces no `EnvironmentCallablePublication`, no Rust metadata catalog entry for the batch, and no recovery `TypeKind::Error`.

### 5.3 Final publication stamp

Every final `EnvironmentCallablePublication` carries the `AcceptedNominalWorldStamp` of the world that produced it:

```text
world ID + project symbol revision + accepted nominal catalog digest
```

`RegisteredCallableCatalogBuilder` is created for exactly one stamp and rejects a publication with any other stamp. This is the catalog-revision guard; a publication cannot be cached and admitted into a different accepted world.

### 5.4 Constructor authority

Only `AcceptedNominalWorld::try_project_environment_publication` may create a public final environment publication containing manifest/Rust nominal types. The existing public context-free publication constructors are narrowed to `pub(crate)` and require a stamp. Adapter-context never constructs `TypeKind::AcceptedNominal` directly.

## 6. Accepted record instantiation reuse

The existing authored resolver and the new publication projection must share one inherent operation on the existing accepted record type:

```rust
AcceptedNominalRecord::try_instantiate(arguments)
```

That operation owns:

- arity validation;
- `Exact`, `Opaque`, and `Character` accepted semantics; and
- construction of `AcceptedNominalType`.

The authored source resolver retains source lookup, alias handling, budgets, poison accounting, and diagnostics. Publication projection retains manifest item/source context and fail-closed aggregation. Neither duplicates accepted-record instantiation semantics.

## 7. Context-free conversion decision

`AdapterTypeKind::to_sema_type_kind()` is deleted.

There is no public replacement that returns `TypeKind`. Primitive-only conversion is also not exposed because it would create two publication construction paths and invite partial recursive conversion.

Adapter-context may have a crate-private exhaustive conversion from validated `AdapterTypeKind` to `EnvironmentTypeProjectionNode`. That target is unresolved registration input, not a semantic type, and every nominal node already contains an exact owner/path candidate.

The following are prohibited:

- `From<&AdapterTypeKind> for TypeKind`;
- `TryFrom<&AdapterTypeKind> for TypeKind` without an `AcceptedNominalWorld`;
- `Named` restoration for a failed nominal;
- string equality between `Named` and `AcceptedNominal`;
- suffix or terminal-name lookup;
- parsing `TypeKind::source_label()` back into a type.

## 8. Non-callable Rust metadata decision

Rust ADT metadata migrates in the same atomic cut.

### 8.1 Final identity

Rust metadata is keyed by `AcceptedNominalId`; instantiated queries receive `AcceptedNominalType`. The following string identities are removed from the Rust metadata route:

- `AdapterRustType.package: String`;
- `ArcweftRustTypeDecl.name: String` as semantic identity;
- `RustPackageExports.types: HashSet<String>`;
- Rust enum payload entries keyed by `TypeKind::Named`.

### 8.2 Generic metadata

Rust declaration type parameters become `GenericTypeParameterId` values owned by:

```rust
GenericTypeOwnerId::AcceptedNominal(AcceptedNominalId)
```

Metadata field/payload templates contain those typed generic IDs. Instantiation substitutes the checked `AcceptedNominalType.arguments()` through the existing recursive `TypeKind` substitution implementation.

A declaration type parameter may appear only inside metadata owned by that declaration. A free type parameter in a callable receiver, parameter, or result is a hard publication error.

### 8.3 Storage

`RegisteredTypeCheckEnv` stores an immutable `AcceptedRustTypeMetadataCatalog` alongside:

- `AcceptedNominalWorld`;
- the callable catalog; and
- the existing project/character catalogs.

Rust-specific enum/newtype/field lookup moves to this catalog. Existing non-Rust `TypeCheckEnv` enum metadata remains unchanged. No Rust accepted export is inserted into an enum map under `TypeKind::Named`.

## 9. Visibility and unknown-path distinction

The accepted world gains a source-backed `AcceptedNominalVisibilityIndex`. It contains:

- visible external nominal IDs and their declaration sources; and
- known but inaccessible external nominal IDs and their declaration sources.

Only visible records enter `AcceptedNominalCatalog`.

World lookup behavior is exact:

1. catalog record at path, same ID → instantiate;
2. catalog record at path, different owner → `OwnerMismatch`;
3. no catalog record, inaccessible index contains requested ID → `InaccessibleExport`;
4. no catalog record and no inaccessible record → `UnknownPath`.

This index is an admission fact, not a second resolver and not a compatibility lookup.

## 10. Source ownership

`SourceBackedAdapterRegistrationFacts` renders one deterministic synthetic source document while walking the typed manifest. It simultaneously builds a typed source map; it does not scan or reparse the rendered text.

Every type node has a `SourceSpan`, including:

- nominal owner and path;
- each generic argument;
- each composite child;
- method receiver;
- parameter type;
- result type;
- Rust metadata field or variant payload.

Errors use the exact node span as primary source. The containing callable/Rust item and the accepted declaration source, when available, are related sources.

## 11. Determinism and limits

Publication projection reuses:

- `NominalResolutionLimits::PRODUCTION` for per-type nodes, recursive depth, generic arguments, diagnostics, and work;
- `NominalAggregationLimits::PRODUCTION` for document/project reporting;
- `AcceptedNominalCatalogLimits::PRODUCTION` for accepted inventory; and
- the existing `CallableLimits` for records, groups, parameters, effects, and overloads.

The pinned production nominal limits are:

| Resource | Limit |
|---|---:|
| type nodes per reference | 4,096 |
| recursive type depth | 256 |
| generic arguments per application | 256 |
| diagnostics per type reference | 32 |
| work per reference | 65,536 |
| diagnostics per document | 128 |
| diagnostics per project | 512 |
| work per project | 1,048,576 |
| accepted exact records | 4,096 |

Projection reports are sorted by owner, item ID, type site, stable error-code rank, and source range. Bounded omission counts are retained. Hash-map iteration and manifest insertion order cannot affect record order, diagnostics, or digests.

## 12. Schema and compatibility cut

The current contracts are unpublished. Both schema constants remain `1`. Therefore:

- `ARCWEFT_RUST_ABI_SCHEMA_VERSION` remains `1`;
- `ADAPTER_MANIFEST_SCHEMA_VERSION` remains `1`;
- the v1 carrier definitions are replaced in place;
- there is one reader and one writer for each carrier;
- old string nominal shapes are ordinary malformed input under the final v1 shape;
- no aliases, dual readers, migration shim, old-spelling diagnostic, or version bump is added.

All fixtures and producers are updated in the same production commit.

## 13. Atomicity

The registered-world function remains a single transaction. Nominal inventory, metadata projection, callable projection, catalog construction, and environment digest creation are local values until the final `RegisteredTypeCheckEnv` is returned.

On any error:

- the previous registered world remains unchanged;
- no partial nominal record is externally visible;
- no partial Rust metadata catalog is externally visible;
- no partial callable publication is admitted;
- no persistent environment digest is emitted for the failed candidate world.

## 14. Required acceptance state

The implementation is accepted only when all of these are true:

1. A Rust nominal in an adapter/Rust callable and the corresponding authored `extern` signature contain equal `AcceptedNominalType` values.
2. The same accepted ID survives free-function, method receiver, later curried group, result, and nested composite positions.
3. two packages with the same terminal type name remain distinct under distinct mounts; an equal full path is rejected.
4. all projection failures are structured, source-backed where evidence exists, deterministic, bounded, and atomic.
5. Rust non-callable metadata uses accepted IDs and typed generic substitution.
6. callable candidate keys, schema digests, signature help, hover, method lookup, overload matching, and persistent environment keys consume exact semantic identity.
7. no production path constructs a Rust/adapter accepted export as `TypeKind::Named`.
8. all tests in `TEST-MATRIX.csv` pass through typed public or crate-owned APIs.
