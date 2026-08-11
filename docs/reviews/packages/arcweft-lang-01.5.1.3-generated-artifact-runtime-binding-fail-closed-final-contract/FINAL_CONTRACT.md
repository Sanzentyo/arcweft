# Lang-01.5.1.3 — generated artifact runtime-binding fail-closed final contract

## 1. Authority and status

This document is normative for implementation of Lang-01.5.1.3 against Git commit `0c8cb74dd96116a8b987cc419c9a280b6cabe4a4`.

`FINAL_STATUS = READY_FOR_IMPLEMENTATION`  
`OPEN_QUESTIONS = 0`

The current repository already retains accepted `ExternalModuleImportSpec`, source-backed metadata, typed target/package/module/artifact/export facts, selected Activity bindings, and a `SourceSetRevision`. The missing boundary is the runtime binding identity and its fail-closed selection. This contract fills only that boundary.

## 2. Non-negotiable invariants

1. **One accepted authority.** A runtime binding requirement is projected from the already accepted `LoadedExternalModuleMetadata` and selected profile. Metadata is not re-read or re-decoded.
2. **One exact key.** The complete typed `GeneratedArtifactBindingKey` is the sole host claim authority. Names, paths, mounts, profiles, or Activity IDs are never independent lookup keys.
3. **One product-local runtime identity.** `GeneratedArtifactBindingId` is meaningful only with the launch product and topology identity that assigned it.
4. **Complete structural comparison.** Correlation uses typed field equality in the fixed order in `ERRORS_AND_LIFETIME.md`; it does not compare debug strings, serialized bytes, or a newly invented aggregate digest.
5. **Fail before host work.** Every generated function dispatch and generated Activity start resolves the exact binding before callback invocation, task enqueue, scheduler mutation, Activity state allocation, registry insertion, or emitted event.
6. **No fallback.** Any missing, stale, unselected, kind-mismatched, or mismatched binding is terminal for that attempt.
7. **Sans I/O.** The key, product, catalog, and validation code performs no filesystem, dynamic-library, WASM, process, network, Cargo, WIT, clock, thread, or provider operation.
8. **Deletion-driven migration.** Generated exports stop using the current string-only call path once their typed origin is available. No compatibility path remains.

## 3. Owning model

Add a new Sans-I/O library crate, `arcweft-runtime-binding`, as the sole owner of:

- `GeneratedArtifactBindingKey` and its nested identities;
- `GeneratedArtifactBindingLaunchProduct` and requirement validation;
- strict format/schema serialization;
- the host-owned generic catalog builder and immutable catalog;
- structured product, registration, resolution, mismatch, and stale errors.

The product-local `GeneratedArtifactBindingId` belongs in foundational `arcweft-id`, because `arcweft-adapter-context`, `arcweft-core`, `arcweft-runtime-plan`, the compiler, and the new binding crate must share it without making lower runtime layers depend on metadata/topology crates.

`arcweft-runtime-binding` depends downward on `arcweft-adapter-metadata`, `arcweft-id`, `arcweft-manifest-model`, and `arcweft-source`. `arcweft-core` and `arcweft-runtime-plan` depend only on `arcweft-id` for the ID. `arcweft-runtime-host` may depend on `arcweft-runtime-binding`; the reverse dependency is forbidden.

## 4. Exact binding key

`GeneratedArtifactBindingKey` contains these typed groups:

### 4.1 Topology identity

- selected `ProfileId`;
- complete accepted `SourceSetRevision`.

This pair is the serializable runtime revision authority for this split. It ensures a metadata or manifest overlay changes the launch identity even if the new product happens to assign the same numeric requirement ordinal.

### 4.2 Import identity

- `ExternalModuleImportId`;
- `ModuleMountPath`;
- metadata `NormalizedProjectPath`;
- accepted metadata `RawDigest` from the manifest import;
- import `ManifestVisibility`;
- import `DependencyDemand`.

The manifest's expected package/version/module/family/ABI-hash fields are not duplicated as a second authority. Admission has already reconciled them with the accepted metadata. The canonical accepted package/module/target/ABI values appear once in the key, while `SourceSetRevision` and the metadata raw digest bind the exact manifest and metadata bytes that established the reconciliation.

### 4.3 Metadata source identity

- complete `SourceDocumentIdentity` (`SourceDocumentId`, exact `SourceRevision`, and source length);
- metadata `abi_hash`;
- metadata `payload_hash`.

The source identity is retained rather than reconstructed from a path. Generator provenance, requirements, format/schema, and unrelated exports are not copied into every per-export key: the exact metadata raw digest, source identity, payload hash, and source-set revision pin the complete accepted document. Any change to those omitted fields therefore makes an older key stale or mismatched; it cannot remain silently reusable.

### 4.4 Target identity

- typed `AdapterFamily`;
- validated `GeneratedArtifactAbi` newtype;
- exactly one family-specific detail:
  - Rust: `TargetTriple`;
  - WASM: `WitWorldId`;
  - process: validated `GeneratedArtifactTransport`.

`GeneratedArtifactAbi` and `GeneratedArtifactTransport` remain typed values with private fields and checked constructors. They are not public `String` aliases. The accepted projector obtains their canonical values from inherent behavior on the Arcweft-owned metadata types: `AdapterTarget::family()`, target ABI marker `as_str()`, and `ProcessTransport::as_str()`. The open checked newtypes may represent a provider's syntactically valid but wrong claim so registration can report an exact ABI/transport mismatch; a launch product is valid only when its family/detail/ABI/transport tuple exactly matches the current accepted Arcweft owner markers (`RustAbi`, `WasmAbi`, `ProcessAbi`, and `ProcessTransport`).

### 4.5 Package/module/artifact identity

- complete accepted `AdapterPackage` (`PackageId` and exact `PackageVersion`);
- complete accepted `AdapterModule` (`ExternalModuleId`);
- complete accepted `AdapterArtifact` (`NormalizedProjectPath`, raw `RawDigest`, and `u64` size).

No basename or host filesystem normalization participates.

### 4.6 Export identity

Function requirements contain the complete accepted `AdapterFunctionExport`: name, visibility, ordered parameters with typed names/types, return type, purity, and ordered effect identities.

Activity requirements contain:

- the selected abstract `ActivityId` from the profile binding;
- the selected `ActivityImplementationId` retained by `ResolvedActivityBinding`; and
- the complete accepted `AdapterActivityExport`: export ID, visibility, metadata Activity ID, interface hash, and state hash.

Product construction requires the abstract Activity ID to equal the metadata Activity ID. The abstract and implementation IDs are retained explicitly so the exact manifest selection is visible, round-trippable, and cannot be reconstructed later from an Activity spelling or export name.

## 5. Canonicalization and ID assignment

The product constructor receives valid keys in arbitrary iteration order, validates them, then sorts by a typed canonical anchor:

1. `ExternalModuleImportId`;
2. `ModuleMountPath`;
3. export-kind rank (`Function = 0`, `Activity = 1`);
4. function `FunctionName` or Activity `AdapterExportId`;
5. Activity `ActivityImplementationId`;
6. Activity abstract `ActivityId` as the final Activity tie-breaker.

It then assigns contiguous `GeneratedArtifactBindingId` values equal to canonical vector positions. The ID wire value is `u32`; more than `u32::MAX` requirements is a product error. The product rejects duplicate or conflicting anchors. It never silently selects one duplicate.

Canonical IDs are stable under input iteration permutation, but they are **not** global identities. A new topology may reuse the same ordinal for a different or revised requirement; topology correlation is therefore mandatory before every resolution.

## 6. Launch product

`GeneratedArtifactBindingLaunchProduct` is the single serializable sidecar for one **selected launch profile**:

- exact format marker `arcweft.generated-artifact-bindings`;
- exact schema `1`;
- one `GeneratedArtifactTopologyIdentity`;
- a canonical, contiguous sequence of `GeneratedArtifactBindingRequirement { id, key }`;
- a canonical sequence of `GeneratedArtifactActivitySelection { activity, implementation, binding }` derived from the Activity requirements.

The complete key appears once in this product. Runtime-plan nodes and captured function values carry only `GeneratedArtifactBindingId`. Activity launch assembly carries the exact `GeneratedArtifactActivitySelection`; it never recovers a binding by spelling.

A selected profile with no generated function or selected Activity requirements has `Some(empty selected product)`: it still has the real selected `ProfileId` and `SourceSetRevision`. A compilation context with **no accepted launch profile** has no product at all. `ProjectCompilationContext::accepted_launch_profile: Option<_>` remains the sole optionality; no synthetic profile/topology/product is created.

Deserialization is strict: exact format/schema, unknown-field rejection, validated nested newtypes, current accepted ABI/transport markers, contiguous IDs, canonical order, unique anchors, same topology on every key, canonical Activity selections, and all cross-field invariants. A decoder must reject non-canonical input rather than sorting or repairing it. There is no legacy schema, alias, defaulted field, compatibility reader, or version migration.

## 7. Topology projection transaction

Replace the current split between `extend_selected_adapter` and `validate_activity_bindings` with one transaction that returns:

- the extended `AdapterManifest`; and
- the canonical `GeneratedArtifactBindingLaunchProduct`.

The transaction runs only after the selected profile and complete source-set revision are known. It consumes `LoadedExternalModuleMetadata` directly; `SourceBackedAdapterMetadata::decode` is not called again.

### Function projection

For every admitted non-private generated function export:

1. retain the current mount/type/signature/purity/effect validation;
2. construct one exact function binding key from the same accepted module/export objects;
3. include the key in canonical ID assignment;
4. insert the function with `AdapterFunctionOrigin::GeneratedArtifact(id)` using an inherent `AdapterManifest::with_generated_function_signature` method.

Private function exports receive neither a callable surface nor a binding requirement.

### Activity projection

For every selected `ResolvedLaunchProfile::activity_bindings()` entry:

1. find the already admitted module by `ExternalModuleImportId`;
2. find the accepted Activity export by `AdapterExportId`;
3. retain the exact selected `ResolvedActivityBinding::implementation_id()`;
4. require the metadata Activity ID to equal the selected abstract Activity ID;
5. construct one exact Activity key and requirement. Canonical product construction derives the matching `GeneratedArtifactActivitySelection` from that requirement and assigned ID.

Unselected Activity exports receive no binding requirement. An unselected module or export cannot become callable merely because a host presents a binding.

### Atomicity

No extended adapter is returned without the matching launch product, and no product is returned without its matching adapter origins and Activity selections. Duplicate mounted identities, invalid signatures, Activity reconciliation failures, key/selection invariant failures, or canonicalization failures abort the complete topology transaction.

`LoadedProfileTopology` owns the product and exposes an immutable `Arc<GeneratedArtifactBindingLaunchProduct>` alongside the adapter and `SourceSetRevision`.

## 8. Semantic and runtime-plan identity preservation

Add `AdapterFunctionOrigin` directly to Arcweft-owned `AdapterFunction`:

- `HostAdapter` for the existing constructor;
- `GeneratedArtifact(GeneratedArtifactBindingId)` for generated exports.

The existing semantic callable record that currently consumes `AdapterFunction` must carry this origin as a field. Do not create a path-to-ID side map, extension trait, string prefix, or post-typecheck lookup.

The existing typed lowering evidence carries the generated ID for all three function forms:

1. a direct full call;
2. a top-level generated function used as a first-class value;
3. a partial call that produces a function value.

`RuntimeCallTarget` gains `GeneratedArtifact(GeneratedArtifactBindingId)`. `RuntimeFunctionBody` gains the same generated-artifact variant. A full call lowers to the call-target variant. A function reference or partial call lowers to a `RuntimeFunctionValue` whose body retains the ID; subsequent apply dispatches that ID directly.

Generated paths must no longer pass through `RuntimeCallTarget::from_label`. `RuntimeCallTarget::as_label() -> &str` cannot represent a typed generated target and must be replaced with owner behavior that is honest about the variant, such as `named_label() -> Option<&str>` plus a direct `Display` match. No synthetic generated callable spelling is an execution authority.

After ordinary `RuntimePlan::verify`, the compiler runs one cross-product verifier over `Option<&GeneratedArtifactBindingLaunchProduct>` that proves:

- every generated call target and generated function body references an existing requirement;
- each such requirement is a Function, never Activity;
- every Activity selection points to the exact Activity requirement with the same abstract and implementation IDs;
- every referenced requirement has the same topology as the selected product;
- `None` is valid only when no generated function ID or generated Activity launch selection is present;
- no unreferenced ID can be fabricated by codec input.

`AcceptedLaunchProfileInput` requires the immutable product. `CompiledProject` stores `Option<Arc<GeneratedArtifactBindingLaunchProduct>>`, copied exactly from the existing optional accepted-launch input. A direct/no-profile compile stores `None`; it never fabricates an empty topology.

## 9. Host-owned Sans-I/O catalog

The host constructs `GeneratedArtifactBindingCatalogBuilder<F, A>` from exactly one launch product. The builder creates one fixed slot per canonical requirement. `F` and `A` are host-selected already constructed binding types; the shared crate imposes no loading, execution, `Send`, `Sync`, `Clone`, filesystem, or process contract.

Registration APIs are kind-specific and require:

- the product-local `GeneratedArtifactBindingId`;
- a complete claimed `GeneratedArtifactBindingKey` supplied by the binding provider/host;
- an already constructed function or Activity binding value.

Registration performs, in order:

1. topology correlation (stale on profile/revision mismatch);
2. selected-ID check;
3. requirement-kind check;
4. complete key correlation in the fixed mismatch order;
5. duplicate-slot check;
6. slot mutation.

Therefore a failed registration cannot replace or partially install a slot.

`freeze()` produces an immutable catalog even when some slots are absent. This is intentional: E-19 requires the runtime attempt, not catalog construction, to return `runtime-binding-missing`. There is no resolution API by name, Activity ID, mount, basename, profile, path, digest alone, or adapter ID.

Resolution requires the active `GeneratedArtifactTopologyIdentity` and a typed ID. It checks stale/unselected/kind before slot presence. Missing returns the exact expected requirement in the error and does not invoke the host binding.

## 10. Runtime fail-closed gates

### Generated function call and apply

Before any generated function callback, native-task request, provider operation, or host-work enqueue:

1. obtain the ID directly from `RuntimeCallTarget::GeneratedArtifact` or `RuntimeFunctionBody::GeneratedArtifact`;
2. call `catalog.resolve_function(active_topology, id)`;
3. on any error, return the structured runtime binding failure with no host-side mutation;
4. on success, hand the borrowed already constructed binding to the future/host execution boundary.

This split defines selection only. It does not define how `F` executes Rust, WASM, or process artifacts.

### Generated Activity start

Before allocating generated Activity state, creating an `ActiveActivity`, inserting an `ActivityHostRegistry` entry, emitting a start event, or enqueueing scheduler/host work:

1. receive the pre-projected `GeneratedArtifactActivitySelection` whose abstract Activity and `ActivityImplementationId` were verified against the launch product;
2. call `catalog.resolve_activity(active_topology, selection.binding())`;
3. fail without mutation on any error;
4. only then pass the resolved binding to the Activity execution/instance construction boundary.

The existing `ActivityHostRegistry`, keyed by concrete `InteractionTarget`, remains the registry for already constructed Activity instances. It is not a generated artifact resolver and must never query the catalog by `ActivityId` or spelling. The generated Activity launch record carries the product-owned `GeneratedArtifactActivitySelection` directly; no interaction-target/Activity-name-to-binding side map is introduced.

## 11. Revision and LSP generation rules

### Serializable topology correlation

`ProfileId + SourceSetRevision` is the serialized product/catalog correlation. The exact metadata document identity and raw digest add per-module evidence. An overlay changing metadata or manifest bytes creates a new source-set revision and invalidates the old catalog before presence is examined.

This split does not invent or publish a broader project-topology digest. If a later accepted contract adds such an authority, it may replace the topology field through a new explicit schema; it must not be opportunistically dual-read here.

### Process-local LSP lease

`AcceptedEnvironmentGeneration` remains process-local and is not serialized into the key or launch product. `AcceptedProfileEnvironment` atomically owns the generation, compiled project, the compiled project's optional launch product, and any catalog lease. A catalog lease exists only for `Some(product)`. Replacing the accepted environment discards the old catalog even when rebuilt bytes are identical. A catalog must never be carried forward into a new generation.

The LSP/runtime assembly boundary compares the requested generation with the current `AcceptedProfileEnvironment::generation()` before exposing the catalog. A mismatch returns `runtime-binding-stale`. The normal catalog resolve then performs the serialized topology check. This two-level rule prevents both stale-content reuse and same-content cross-generation ownership reuse.

There is no last-known-good catalog acceptance.

## 12. Error contract

Machine-readable codes are exact:

- `runtime-binding-missing`;
- `runtime-binding-stale`;
- `runtime-binding-mismatch`;
- `runtime-binding-unselected`;
- `runtime-binding-kind-mismatch`;
- `runtime-binding-duplicate`;
- `runtime-binding-product-invalid`.

Errors are typed enums with a `code()` method returning a typed code whose `as_str()` produces the exact spelling. Display text is diagnostic only and is never matched for behavior.

Stale topology/generation has precedence over selected-ID, kind, key mismatch, and missing checks. Registration mismatch is reported before slot mutation. Resolution missing is reported only after topology, selection, and kind are valid.

The complete first-mismatch field order is normative in `ERRORS_AND_LIFETIME.md`.

## 13. Required deletion

The implementation is incomplete until it removes:

- the generated function success path through `RuntimeCallTarget::from_label` / `Named(String)`;
- any generated function path or spelling re-lookup after semantic resolution;
- the separate Activity validation-only pass once the unified projection transaction exists;
- any candidate resolver by callable spelling, Activity spelling, mount, basename, profile, adapter ID, filesystem path, or digest alone;
- any catalog carry-forward across LSP generations;
- any compatibility reader, alias, wrapper, or last-known-good branch introduced during the migration.

Ordinary intrinsic and intentionally named non-generated runtime calls may retain their current named/intrinsic behavior. The deletion applies to generated artifacts, not unrelated call families.

## 14. Completion gate

Implementation is complete only when every row in `TEST_MATRIX.md` passes, all mandatory deletions are demonstrated, and validation includes focused crate tests, public codec round trips, `cargo fmt --all -- --check`, `cargo clippy --all-targets` for affected crates, workspace tests, the required tier-2 suite, structure audit, and `git diff --check` as directed by current repository policy.

No successful artifact execution test belongs to this split. The sole positive host test resolves an exact in-memory sentinel binding and proves deterministic selection without opening a file or invoking a backend.
