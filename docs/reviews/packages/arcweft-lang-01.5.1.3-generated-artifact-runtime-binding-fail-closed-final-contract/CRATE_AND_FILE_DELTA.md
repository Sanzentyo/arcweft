# Crate, dependency, and file delta

This is a design-only change map. It is not a patch or overlay. Paths name the owning files/modules at the inspected baseline; private module splitting may be adjusted mechanically if current main moves code before implementation, but ownership and dependency direction are normative.

## Workspace root

### `Cargo.toml`

- Add workspace member and dependency `arcweft-runtime-binding`.
- Reuse workspace `serde` and `thiserror`; add no third-party runtime/loading dependency.
- Preserve edition 2024, Rust 1.96, and `unsafe_code = "forbid"`.

## New crate: `crates/arcweft-runtime-binding/`

### `Cargo.toml`

Dependencies only:

- `arcweft-adapter-metadata`;
- `arcweft-id`;
- `arcweft-manifest-model`;
- `arcweft-source`;
- workspace `serde`;
- workspace `thiserror`.

No `std::fs`, loader, process, WASM engine, networking, async runtime, provider, Cargo metadata, WIT parser, or runtime-host dependency.

### Source files

- `src/lib.rs` — public re-exports and crate-level Sans-I/O contract.
- `src/key.rs` — nested identities, target conversion, export identity, structural correlation.
- `src/product.rs` — format/schema, canonicalization, contiguous IDs, derived Activity selections, accepted-marker validation, strict serde validation.
- `src/catalog.rs` — generic fixed-slot builder/catalog.
- `src/error.rs` — codes, product/registration/resolve/mismatch/stale errors.
- `tests/product_codec.rs` — strict wire/canonicalization matrix.
- `tests/catalog.rs` — missing/mismatch/stale/unselected/deterministic sentinel matrix.

Do not create `mod.rs` files.

## `crates/arcweft-id`

- Add `src/generated_artifact.rs` with `GeneratedArtifactBindingId` and overflow error.
- Re-export from `src/lib.rs`.
- Use existing serde policy; public wire type implements both directions.

## `crates/arcweft-adapter-metadata`

### `src/model.rs`

- Extend the existing `exact_string!` owner macro with `as_str()`.
- Add inherent `AdapterTarget::family()` and `AdapterTarget::abi_str()`.
- Do not add an extension trait in the runtime-binding crate.
- Do not redesign metadata schema, parsing, hash validation, target variants, or export reconciliation.

## `crates/arcweft-adapter-context`

### `src/manifest.rs`

- Add direct `AdapterFunctionOrigin` field to `AdapterFunction`.
- Existing `with_function_signature` sets `HostAdapter`.
- Add `with_generated_function_signature` that sets the typed ID.
- Add inherent `AdapterFunction::origin()`.
- Use one private insertion path to avoid duplicate construction code.
- Add dependency on `arcweft-id`.

Do not add a path-to-origin map, extension trait, wrapper manifest, or second function record.

## `crates/arcweft-project-loader`

### `src/topology/external.rs`

- Replace `extend_selected_adapter` plus `validate_activity_bindings` with one `project_selected_external_modules` transaction.
- Retain current type-reference parsing, mount projection, visibility, purity/effect validation, and duplicate mounted-identity behavior.
- Build exact function and selected-Activity keys from `LoadedExternalModuleMetadata` and `ResolvedLaunchProfile` without calling metadata decode; retain each `ResolvedActivityBinding::implementation_id()`.
- Canonicalize keys, assign IDs, derive exact Activity selections, then add generated functions with those IDs.

### `src/topology/model.rs`

- Add immutable `Arc<GeneratedArtifactBindingLaunchProduct>` to `LoadedProfileTopology`.
- Add constructor parameter and getter.
- Keep current `external_modules`, adapter, resources, and source revision; the product references accepted facts rather than replacing them.

### Topology assembly owner

- Compute/obtain the complete `SourceSetRevision` before final generated projection.
- Construct one `GeneratedArtifactTopologyIdentity` from selected profile ID and revision.
- Return adapter and binding product atomically.
- Add dependency on `arcweft-runtime-binding`.

## `crates/arcweft-lang-sema`

- At the existing adapter-function ingestion point, copy `AdapterFunctionOrigin` into the existing checked/resolved callable record that already owns path, overload, signature, and effects.
- At call/reference/partial-call analysis, emit ID-bearing callable origin in the existing runtime lowering evidence.
- Preserve the origin through overload resolution and first-class function typing.
- Do not add a global callable-spelling-to-binding table or perform a post-sema adapter search.
- Add dependency on `arcweft-id` only if not already available through a legitimate lower dependency; do not depend on `arcweft-runtime-binding`.

## `crates/arcweft-runtime-plan`

### `src/typed_evidence.rs`

- Add `RuntimeTypedCallableOrigin::{Named, GeneratedArtifact}` and use it directly in existing callable evidence variants.

### `src/expr.rs` and existing strict lowering owners

- Full generated call -> `RuntimeCallTarget::GeneratedArtifact(id)`.
- Generated function reference/partial -> `RuntimeFunctionBody::GeneratedArtifact(id)`.
- Preserve current intrinsic and non-generated named behavior.
- Reject missing typed origin rather than reconstructing from path.

### Plan verification

- Visit both generated call targets and generated function bodies.
- Expose the IDs to compiler cross-product verification.

Depends on `arcweft-id`, not metadata/product crates.

## `crates/arcweft-core`

### `src/value.rs`

- Add generated variants to `RuntimeCallTarget` and `RuntimeFunctionBody`.
- Add owner constructors/accessors.
- Delete `RuntimeCallTarget::as_label() -> &str`; implement honest variant-aware display and `named_label() -> Option<&str>`.
- Update every exhaustive match, codec validation, function apply path, and runtime value digest/identity visitor affected by the new variant.
- Keep `arcweft-core` Sans I/O.

### Runtime execution owners

- Route generated call/apply to a binding-resolution request/context before any host request is emitted.
- Do not put the metadata-rich launch product or catalog in core value types.

## `crates/arcweft-compiler`

### `src/project/registration.rs`

- Add `Arc<GeneratedArtifactBindingLaunchProduct>` to `AcceptedLaunchProfileInput` and its constructor/getter.

### Current `CompiledProject` owner

- Add `Option<Arc<GeneratedArtifactBindingLaunchProduct>>` and a getter.
- Copy `Some(product)` only from the existing optional `AcceptedLaunchProfileInput`; direct/no-profile compilation stores `None` and never synthesizes a profile/topology/product.
- A selected profile with no generated requirements still stores `Some(empty selected product)`.

### Runtime-plan finalization

- After current plan verification, run the cross-product generated-binding verifier.
- Map failures to the existing compile-stage error hierarchy as structured `runtime-binding-product-invalid` evidence.
- Add dependencies on `arcweft-runtime-binding` and `arcweft-id` as required.

## `crates/arcweft-runtime-host`

- Depend on `arcweft-runtime-binding`.
- Add a host assembly module, e.g. `generated_artifact_bindings.rs`, that owns concrete catalog types/leases used by a runner.
- Keep provider/backend construction outside the shared crate.
- Add pre-dispatch gates for generated function call/apply and generated Activity start. Activity start consumes the verified `GeneratedArtifactActivitySelection`, including `ActivityImplementationId`, and resolves the catalog only by its typed binding ID.
- Re-export only stable host-facing catalog types actually needed by host applications.

### `src/activity_host.rs`

- Retain `ActivityHostRegistry` as the concrete instance registry keyed by `InteractionTarget`.
- Do not turn it into a generated artifact resolver.
- Ensure generated Activity resolution occurs before registration/state/event/scheduler mutation.

## `crates/arcweft-runtime-driver`

- Carry the active topology identity and a borrowed/owned host catalog access context through the existing session/step boundary where generated dispatch is attempted.
- The driver remains Sans I/O and does not construct bindings.
- It reports typed resolution failures before emitting host tasks.

## `crates/arcweft-lsp`

### `src/profiles/state.rs`

- `AcceptedProfileEnvironment` atomically owns or is associated with the compiled project's launch product and a generation-scoped catalog lease.
- A replacement never copies the prior catalog.
- Add an inherent generation-correlation method on the owning environment/lease context; do not use an extension trait.
- On generation mismatch, report stale before exposing the catalog.

The `AcceptedEnvironmentGeneration` is not added to serialized key/product schemas.

## Bundle/save/codecs

Every current consumer that serializes the runtime plan or compiled launch product must carry and round-trip the new plan variants, `None`/`Some` product presence, requirements, and Activity selections exactly. There is no dual reader. Catalog bindings themselves are never bundled or saved.

If the bundle currently has a compiled-project sidecar aggregation point, add the launch product there rather than inventing a second standalone file lookup. The product schema remains the same in memory and on wire.
