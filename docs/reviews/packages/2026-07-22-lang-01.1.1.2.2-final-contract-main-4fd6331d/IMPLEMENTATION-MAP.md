# Ordered Implementation Map

This map is normative for the production implementation. It names the pinned repository files confirmed at `main@4fd6331dc342d30a7f4ac7774852b60801866ef7` and the direct-final responsibility of each change. File splits may follow the repository’s existing size conventions, but behavior stays on the owning type and no extension-trait detour is permitted.

## Cut 1 — Rust ABI carriers and macros

### `crates/arcweft-rust-abi/src/lib.rs`

- add validated package/path/parameter newtypes;
- replace `ArcweftRustPackage.name` with typed `id`;
- replace `ArcweftRustTypeRef::Named` with `Nominal` and `TypeParameter`;
- replace declaration `name` identity with package-local `path`;
- distinguish unit/tuple/record struct and enum payload shapes;
- add manifest graph validation and bounded errors;
- retain schema constant `1`;
- update `Display` to presentation-only typed formatting;
- update JSON round-trip tests.

### `crates/arcweft-rust-abi-macros/src/lib.rs`

- emit package ID and package-local path in `ArcweftType`;
- emit declaration parameter metadata and `TypeParameter` templates;
- support type generics constrained by `ArcweftType`;
- reject lifetime/const generic ADTs with typed compile diagnostics;
- continue rejecting callable generics and Rust method receivers not supported by the callable ABI;
- update trybuild fixtures.

No new macro is introduced.

## Cut 2 — adapter manifest and file carrier

### `crates/arcweft-adapter-context/src/manifest.rs`

- add nominal owner/path/reference/declaration types;
- replace `AdapterTypeKind::Named` with `Nominal`;
- add package mount table and exact Rust manifest ingestion;
- retain typed callable groups/parameters/overloads;
- ensure standard and external adapter nominal declarations are explicit;
- delete `to_sema_type_kind`;
- keep conversion to sema registration input crate-private and exhaustive.

### `crates/arcweft-adapter-context/src/codec.rs`

- replace string type carrier with recursive tagged carrier;
- add package mounts and nominal declarations to file schema;
- retain schema constant `1`;
- delete type-string parser and every nominal catch-all;
- update JSON/TOML round trips for the final shape.

### `crates/arcweft-adapter-context/src/callable.rs`

- retain checked group/parameter scalar model;
- make all type-bearing fields use final `AdapterTypeKind`;
- add typed source site IDs where item constructors need them;
- keep structural errors separate from semantic projection errors.

### `crates/arcweft-adapter-context/src/manifest/registration.rs`

- extend deterministic source generation with exact per-node source map;
- emit `AcceptedNominalInventoryInput`;
- emit `RustTypeMetadataPublicationInput`;
- emit unresolved callable record inputs;
- compute manifest digest;
- return `SourceBackedAdapterRegistrationParts`;
- never create final publication or `TypeKind`.

### `crates/arcweft-adapter-context/src/publication.rs`

- remove manifest-to-final-publication functions;
- retain only adapter-to-sema registration-input construction or delete the module if empty;
- move any behavior that belongs on `AdapterManifest`, `AdapterTypeKind`, or the sema input type to the original inherent `impl`.

### `crates/arcweft-adapter-context/src/standard.rs`

- declare `HttpRequestContext`, `Conv2dApi`, `InferApi`, and `TensorF32`;
- replace every current string nominal use with typed nominal references;
- register package mounts before any embedded Rust manifest;
- update standard registry/source-backed tests.

### `crates/arcweft-adapter-desktop/src/manifest.rs`

- type package identity for `arcweft-adapter-desktop`;
- register the package mount;
- map `WindowMode` and `CursorIcon` to exact Rust accepted paths;
- replace parameter type string references;
- update Rust manifest declarations/payloads.

## Cut 3 — project-loader source facts

### `crates/arcweft-project-loader/src/topology/external.rs`

- emit explicit adapter-native nominal declarations for public mounted external types;
- emit inaccessible inventory facts for private types;
- recursively build typed nominal references for function types;
- delete `mounted_identity(...) -> String` as a semantic type identity;
- retain visibility, purity, mount, and type-reference limits;
- keep all carrier construction before sema registration.

### `crates/arcweft-project-loader/src/environment.rs`

- collect `SourceBackedAdapterRegistrationParts`;
- add generated documents and external facts as before;
- pass environment registration inputs into `ProjectRegistrationFacts`;
- stop discarding publication/metadata inputs.

### `crates/arcweft-project-loader/src/topology/tests.rs`

- replace string nominal expectations with exact typed owner/path values;
- add public/private inventory and same-terminal mount tests;
- avoid source-text scanning.

## Cut 4 — accepted nominal inherent behavior

### `crates/arcweft-lang-sema/src/env/nominal.rs`

- add `AcceptedNominalWorldStamp`;
- add visibility index;
- add exact world lookup by `AcceptedNominalId`;
- add `AcceptedNominalRecord::try_instantiate`;
- enrich duplicate errors with first/duplicate source;
- retain global exact `TypePath` collision semantics;
- retain catalog digest authority.

### `crates/arcweft-lang-sema/src/types/nominal.rs`

- add `GenericTypeOwnerId::AcceptedNominal`;
- update typed source-label and digest behavior;
- retain crate-owned `AcceptedNominalType::new`;
- add tests for owner-distinct generic parameter identity.

### `crates/arcweft-lang-sema/src/nominal/resolver/engine/resolution.rs`

- replace local accepted-record instantiation logic with the new inherent record operation;
- retain source lookup, aliases, limits, poison allocation, and reporting unchanged;
- do not route manifest inputs through authored `TypeRef`.

### `crates/arcweft-lang-sema/src/nominal/limits.rs`

- reuse existing production limits;
- no new independent limit constants;
- expose only accessors needed by projection.

## Cut 5 — sema registration input and Rust metadata

### New `crates/arcweft-lang-sema/src/registration/environment_input.rs`

- add sema-owned neutral registration input types;
- validate source spans/snapshots;
- canonicalize typed item order;
- keep constructors public only as required by adapter-context’s `sema` feature;
- keep world-bound form crate-owned.

### `crates/arcweft-lang-sema/src/registration/model.rs`

- add environment inputs to `ProjectRegistrationFacts`;
- remove final callable publications from `CharacterRegistrationRequest`;
- bind world/revision/source snapshots in `ProjectRegistrationFacts::try_new`;
- retain prior-world and project symbol contracts.

### New `crates/arcweft-lang-sema/src/env/rust_metadata.rs`

- add publication inputs/final catalog;
- project generic templates to typed generic parameter IDs;
- add typed enum/struct/newtype lookup and instantiation;
- calculate deterministic metadata digest.

### `crates/arcweft-lang-sema/src/env/base.rs`

- remove string-based Rust package export set;
- retain non-Rust base environment behavior;
- route Rust metadata access through registered environment/catalog;
- ensure no Rust accepted record is keyed as `TypeKind::Named`.

### `crates/arcweft-lang-sema/src/env/enums.rs`

- retain `EnumVariantPayload` as the typed payload model;
- expose inherent substitution/instantiation behavior needed by accepted Rust metadata;
- do not add an extension trait or display-name lookup.

## Cut 6 — projection and callable publication

### New `crates/arcweft-lang-sema/src/callable/projection.rs`

- recursively project every type node through one accepted world;
- use `AcceptedNominalWorld::accepted_record` and record instantiation;
- apply existing nominal/aggregation budgets;
- construct deterministic structured diagnostics;
- reject free callable type parameters;
- produce only stamped final publications.

### `crates/arcweft-lang-sema/src/callable/publication.rs`

- add world stamp and publication digest;
- narrow unstamped constructors;
- retain checked publication record invariants.

### `crates/arcweft-lang-sema/src/callable/schema.rs`

- add stable schema digest;
- retain exact semantic equality;
- ensure accepted nominal arguments are recursively encoded.

### `crates/arcweft-lang-sema/src/callable/identity.rs`

- retain existing candidate and receiver key shapes;
- add canonical digest encoding on original types;
- no display-string identity.

### `crates/arcweft-lang-sema/src/callable/builder.rs`

- bind builder to one nominal world stamp;
- reject mismatched publications before record admission;
- preserve project signature resolution and alias matching;
- calculate registered callable catalog digest at finish.

### `crates/arcweft-lang-sema/src/callable/catalog.rs`

- retain deterministic authority/provider/kind/overload/declaration ordering;
- expose canonical iteration needed for digest;
- retain exact receiver/candidate identity.

## Cut 7 — single registration transaction

### `crates/arcweft-lang-sema/src/registration/registrar.rs`

Implement the exact order in `CONSTRUCTION-ORDER.md`:

1. request/source checks;
2. project links/owners;
3. accepted inventory + visibility;
4. one accepted world;
5. project callable schemas;
6. Rust metadata projection;
7. environment callable projection;
8. callable catalog finish;
9. environment digest;
10. final registered world.

Convert projection reports into the existing registration report without changing poison/query budget behavior. Do not commit any intermediate state.

### `crates/arcweft-compiler/src/project/registration.rs`

- pass only project registration facts;
- remove prebuilt environment publication parameter plumbing;
- expose registered environment digest to compile stages.

### `crates/arcweft-cli/src/app/project.rs`

- delete separate `callable_publications(adapter_manifests)` path;
- construct source-backed adapter facts once;
- pass environment inputs through project facts;
- update profile registration error rendering for structured sources.

## Cut 8 — digests and persistent keys

### `crates/arcweft-lang-sema/src/types.rs` or new `types/digest.rs`

- add inherent exhaustive `TypeKind::semantic_identity_digest`;
- assign explicit stable tags to every current variant;
- add compile-exhaustive tests.

### `crates/arcweft-compiler/src/persistent.rs`

- use registered environment digest in compiler object keys;
- preserve existing object key fields and AWBO schema;
- update read/write-through tests with non-zero environment digest.

### `crates/arcweft-compiler/src/incremental.rs`

- replace `BuildDigest::ZERO` only in paths that possess a complete registered environment;
- keep syntax-only/pre-registration keys unchanged;
- update snapshot tests.

### `crates/arcweft-project/src/persistent_object/schema.rs`
### `crates/arcweft-project/src/persistent_object/codec.rs`

- no field or version change;
- add round-trip/key-difference tests for non-zero environment digest;
- retain exact compiler identity and payload validation.

### `crates/arcweft-project-loader/src/cache/persistent_query.rs`

- include the registered environment digest in semantic query keys;
- ensure stale accepted world/profile generations miss deterministically.

## Cut 9 — tooling/LSP

### `crates/arcweft-lang-sema/src/signature/project.rs`

- retain exact `TypeKind` in semantic signatures;
- add assertions/accessors for accepted nominal ID where current tests only compare labels;
- include registered environment identity in cache key path.

### `crates/arcweft-lsp/src/features/hover.rs`

- obtain callable/type hover input from typed sema records;
- retain accepted ID until final Markdown serialization;
- do not classify Rust accepted exports through the unrelated internal `Named` presentation branch.

### `crates/arcweft-lsp/src/features/nominal_types.rs`

- add accepted-environment nominal hover/navigation through exact `AcceptedNominalId` and source-backed records;
- preserve project nominal navigation as implemented;
- display package/path/arity without reparsing labels.

### LSP/profile cache files

- keep accepted generation cancellation;
- include registered environment digest/world stamp where signature/hover caches are keyed;
- add stale-world tests.

## Cut 10 — tests and cleanup in the same commit

- implement every row in `TEST-MATRIX.csv`;
- delete old constructors/call sites immediately;
- update all standard/desktop/external fixtures;
- remove dead imports and obsolete helpers;
- run the acceptance commands below;
- update the existing Lang-01.1.1.2 implementation intake with the completed correction and exact test evidence.

## Production acceptance commands

These commands are prescribed for the implementation commit:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p arcweft-rust-abi
cargo test -p arcweft-rust-abi-macros
cargo test -p arcweft-adapter-context --features sema
cargo test -p arcweft-adapter-desktop
cargo test -p arcweft-lang-sema
cargo test -p arcweft-project-loader
cargo test -p arcweft-compiler
cargo test -p arcweft-project
cargo test -p arcweft-lsp
cargo test --workspace --all-features
```

If workspace-wide environmental tests require unavailable platform services, the implementation report must name each command/test and its exact environmental reason. No contract test may be replaced by a source-text scan or implementation-path gate.
