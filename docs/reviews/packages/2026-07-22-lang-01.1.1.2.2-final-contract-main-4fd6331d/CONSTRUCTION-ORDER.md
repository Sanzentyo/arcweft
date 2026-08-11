# Construction and Dependency Order

## 1. Dependency direction

The final dependency graph is:

```text
arcweft-rust-abi
    (Sans I/O typed Rust metadata)
        ↓
arcweft-adapter-context
    (typed manifest, mounts, generated source + sema input under `sema`)
        ↓
arcweft-project-loader / arcweft-cli
    (collect source-backed facts)
        ↓
arcweft-lang-sema registration
    (accepted inventory → AcceptedNominalWorld → metadata/callable projection)
        ↓
RegisteredTypeCheckEnv
        ↓
compiler/runtime-plan/verify/tooling/LSP
```

There is no `arcweft-lang-sema → arcweft-adapter-context` dependency. Sema owns neutral registration input types; adapter-context constructs them only under its existing `sema` feature.

## 2. Producer order inside adapter-context

For each selected standard or adapter manifest:

1. Validate the adapter ID and derive `AdapterEnvironmentOwnerId::for_adapter`.
2. Validate all `AdapterNominalDeclaration` values.
3. Validate and freeze `AdapterRustPackageMountTable`.
4. Ingest Rust manifests only after all referenced package mounts exist.
5. Validate Rust package ID/version/hash consistency.
6. Map every Rust package-local type path through its exact mount.
7. Convert every `ArcweftRustTypeRef` recursively to validated `AdapterTypeKind`.
8. Validate that callable effects, groups, parameters, overloads, and host-call links satisfy the existing model.
9. Render the deterministic synthetic source while constructing the typed source map.
10. Compute the canonical manifest digest from typed values, not from `Debug` or rendered text.
11. Produce `SourceBackedAdapterRegistrationParts`.

No semantic `TypeKind` is created in these steps.

## 3. Loader and compiler input order

`arcweft-project-loader` collects every `SourceBackedAdapterRegistrationParts` and passes:

- generated source documents into the project source set;
- external declaration facts into the existing external fact list; and
- `SourceBackedEnvironmentRegistrationInput` values into `ProjectRegistrationFacts::try_new`.

The CLI/compiler path that separately calls `callable_publications(adapter_manifests)` is deleted. A project compilation context receives only `ProjectRegistrationFacts` and the ordinary compiler inputs.

`ProjectRegistrationFacts::try_new` performs these checks before returning:

1. source identity uniqueness;
2. all spans belong to supplied source documents and current revisions;
3. environment owner uniqueness;
4. manifest digest uniqueness per owner;
5. declaration/callable/Rust item IDs are unique within an owner;
6. publication record declaration ordinals are contiguous;
7. package mount claims agree across all selected inputs;
8. all typed input collections are canonicalized by stable key.

It then binds each environment input to the request’s:

- `ProjectSymbolWorldId`;
- `ProjectSymbolRevision`;
- generated source identity; and
- manifest digest.

This produces crate-owned `BoundEnvironmentRegistrationInput` values inside `ProjectRegistrationFacts`.

## 4. Single registered-world transaction

`CharacterRegistrar::register` retains one entry point and one final commit. Its normative order is:

### Phase 1 — validate immutable request identity

- validate prior-world lineage;
- validate project symbol world/revision;
- validate source identities and revisions;
- validate character manifests and existing registration prerequisites;
- validate all bound environment inputs.

Failure returns the existing registration report. Nothing is committed.

### Phase 2 — construct project symbol/link state

- link project modules;
- construct project symbol tables;
- construct external owner registry entries;
- construct character owner entries;
- retain the current project nominal catalog and source-backed semantic index behavior.

No callable publication is constructed.

### Phase 3 — assemble accepted nominal inventory

Start from the request base environment’s accepted nominal catalog. In deterministic owner/item order:

1. add visible adapter-native nominal records:
   - owner `Environment(adapter:<adapter-id>)`;
   - exact world-visible `TypePath`;
   - declared arity;
   - `AcceptedNominalSemantics::Opaque`;
   - source-backed adapter origin.
2. add visible Rust type records:
   - owner `RustPackage(RustPackageId)`;
   - mounted exact `TypePath`;
   - arity from Rust declaration parameters;
   - `AcceptedNominalSemantics::Opaque`;
   - source-backed Rust item origin.
3. add inaccessible declarations only to `AcceptedNominalVisibilityIndex`.
4. retain standard exact/character records already admitted by existing paths.
5. detect all duplicate full paths and owner/package inconsistencies before the world exists.
6. finalize the accepted catalog digest.

No callable data participates in this phase.

### Phase 4 — construct `AcceptedNominalWorld` exactly once

Construct:

```text
AcceptedNominalWorld {
    project symbols,
    accepted base environment with final accepted catalog,
    external owners,
    visibility index,
    world ID,
    project symbol revision
}
```

The resulting stamp is immutable. This is the same world used by:

- authored project callable signature resolution;
- Rust metadata projection;
- environment callable projection;
- tooling snapshots; and
- persistent environment identity.

### Phase 5 — resolve authored project callable schemas

Create `RegisteredCallableCatalogBuilder::for_nominal_world`.

Call `add_project` using the existing source-backed callable signature resolver and the world from Phase 4. This retains:

- project nominal resolution;
- alias expansion;
- generic binder ownership;
- query budget;
- poison accounting; and
- exact `extern` signature semantics.

A project callable resolution failure aborts the transaction.

### Phase 6 — project Rust ADT metadata

For each bound environment input, in stable owner/item order:

1. look up its exact Rust `AcceptedNominalId` in the Phase 4 world;
2. create declaration-owned `GenericTypeParameterId` values;
3. recursively project every field/payload/newtype node;
4. reject a type parameter outside its exact metadata binder;
5. build one immutable `AcceptedRustTypeMetadataCatalog`;
6. calculate its canonical digest.

No base environment mutation occurs. The catalog remains a local sibling of the accepted world until commit.

Any metadata error aborts the transaction. No callable input from that or any other batch is admitted.

### Phase 7 — project environment callables

For each bound environment input, in stable owner/item order:

1. compare the input’s bound world/revision/source snapshot with the Phase 4 world;
2. project each method receiver;
3. project every parameter in every group, preserving group and parameter indices;
4. project the result;
5. validate nested type-node and work limits;
6. build the existing checked `CallableSignatureSchema`;
7. create `EnvironmentCallablePublicationRecord`;
8. create one stamped `EnvironmentCallablePublication`.

Only after the complete input has projected successfully is it passed to `RegisteredCallableCatalogBuilder::add_environment`.

No record-level partial addition is allowed.

### Phase 8 — finish callable catalog

The builder:

- verifies every publication stamp;
- applies existing authority/provider validation;
- performs exact schema equivalence;
- constructs candidate IDs;
- constructs method receiver indexes from exact `TypeKind`;
- enforces overload and record limits;
- canonicalizes order; and
- computes the registered callable catalog digest.

A failure discards the builder and every preceding local value.

### Phase 9 — derive registered environment identity

Compute:

```text
RegisteredEnvironmentDigest(
    accepted nominal catalog digest,
    accepted Rust metadata digest,
    registered callable catalog digest,
    ordered selected manifest digests,
    world ID,
    project symbol revision
)
```

The exact byte encoding is in `SCHEMA-TOOLING-PERSISTENCE.md`.

### Phase 10 — commit

Return a new `RegisteredTypeCheckEnv` containing:

- the accepted nominal world;
- project/character catalogs;
- accepted Rust metadata catalog;
- registered callable catalog;
- source-backed semantic indexes;
- environment digest.

The caller swaps the previous world only after this return. There is no mutation or externally visible intermediate state.

## 5. Why there is no registration-order cycle

The dependency edges are acyclic:

```text
typed manifests
    → nominal inventory
    → accepted catalog + external owners
    → AcceptedNominalWorld
    → {project signatures, Rust metadata, environment callables}
    → callable catalog + metadata catalog
    → RegisteredTypeCheckEnv
```

The accepted world does not depend on final callable schemas or Rust metadata payloads. It depends only on declaration inventory, owner registry, project symbols, and base exact semantics.

Rust metadata and callables may refer to accepted IDs, but they cannot create accepted IDs. Thus they are downstream of the world.

## 6. Source-backed fact grammar and source map

The synthetic source renderer emits deterministic lines in this order:

1. manifest header;
2. package mounts by package ID;
3. nominal declarations by accepted path;
4. Rust type declarations by package + local path;
5. Rust metadata members by declaration order;
6. adapter symbols by path;
7. methods by receiver digest + method + overload;
8. functions by path + overload;
9. Rust functions by package + Rust item + callable path + overload;
10. effects and host calls;
11. tooling documentation.

Within a type expression, children render in semantic order. The renderer returns each byte range as it writes; it never searches the resulting string.

The source map key is:

```text
EnvironmentPublicationItemId + EnvironmentTypeSite
```

and maps to exactly one `SourceSpan`. Duplicate keys are a producer error.

## 7. Standard and desktop manifest migration

In the same production commit:

- standard `HttpRequestContext`, `Conv2dApi`, `InferApi`, and `TensorF32` become explicit opaque adapter nominal declarations;
- all standard symbol/method/signature references use `AdapterTypeKind::Nominal`;
- desktop `WindowMode` and `CursorIcon` remain Rust-owned types under package `arcweft-adapter-desktop`;
- desktop registers a package mount before ingesting its Rust manifest;
- every external-module public type becomes an adapter-native nominal declaration at its mounted path;
- every private external-module type contributes an inaccessible inventory fact;
- no current manifest producer retains a string nominal constructor.

## 8. Removal of the split CLI path

The following flow is removed:

```text
adapter_manifests
  → try_callable_publication
  → Vec<EnvironmentCallablePublication>
  → ProjectCompilationContext
  → CharacterRegistrationRequest::with_callable_publication
```

The sole flow becomes:

```text
adapter_manifests
  → SourceBackedAdapterRegistrationFacts
  → ProjectRegistrationFacts
  → CharacterRegistrar
  → AcceptedNominalWorld projection
  → final publication
```

This removal is required in the same cut; leaving both flows would permit unstamped or context-free publication.

## 9. Parallelism

After Phase 4, project signature resolution, per-batch Rust metadata projection, and per-batch callable projection may be computed in parallel only if:

- each task borrows the same immutable world;
- each task has an independent bounded work counter;
- results are collected and sorted by the normative stable key before admission;
- any failure cancels final admission, not merely the failing batch;
- diagnostics remain byte-for-byte deterministic relative to serial execution.

Parallel execution is an implementation detail; serial behavior is the contract oracle.
