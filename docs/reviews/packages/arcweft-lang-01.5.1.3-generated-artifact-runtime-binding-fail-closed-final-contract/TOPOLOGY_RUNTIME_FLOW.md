# Topology-to-runtime projection and fail-closed flow

## 1. Data-flow overview

```text
manifest + selected profile + retained metadata SourceDocument
        |
        | existing strict admission/hash/expectation checks
        v
LoadedExternalModuleMetadata[] + ResolvedLaunchProfile + SourceSetRevision
        |
        | project_selected_external_modules (single transaction)
        v
AdapterManifest with AdapterFunctionOrigin IDs
        +
GeneratedArtifactBindingLaunchProduct (full exact keys + Activity selections)
        |
        +------------------------------+
        |                              |
        v                              v
sema/typed lowering              host catalog builder
(ID preserved)                   (exact claims registered)
        |                              |
        v                              v
RuntimePlan / RuntimeFunctionValue    immutable fixed slots
(ID only)                             (bindings only)
        |                              |
        +---------------+--------------+
                        |
                        | active topology + typed ID
                        v
               exact resolve before host work
```

## 2. Loader transaction in detail

1. Complete existing selected external-module admission and retain `LoadedExternalModuleMetadata`.
2. Compute the complete source-document revision set through the existing `SourceSetRevision` authority.
3. Construct `GeneratedArtifactTopologyIdentity(selected_profile_id, source_set_revision)`.
4. Walk selected modules to retain current type and callable projection facts.
5. Build key candidates for every non-private generated function.
6. Walk selected profile Activity bindings and build key candidates only for selected implementations, retaining each `ActivityImplementationId`.
7. Canonicalize all candidates, assign IDs, and derive canonical `GeneratedArtifactActivitySelection` values.
8. Insert generated functions into the adapter with the assigned origin IDs.
9. Return the adapter and launch product together.
10. Store both in `LoadedProfileTopology`.

The builder may stage function projection facts until IDs are assigned. It must not first publish a string-only generated function and later patch an origin side table.

## 3. Accepted compiler input

`AcceptedLaunchProfileInput` carries the exact product obtained from topology loading. The compiler must not rebuild it from the adapter manifest, selected profile, runtime-plan strings, or file paths. `CompiledProject` retains it as `Some(Arc<_>)`.

The existing `ProjectCompilationContext::accepted_launch_profile: Option<_>` is authoritative:

- `Some(input)` requires `input.generated_artifact_bindings()` and produces `CompiledProject::generated_artifact_bindings() == Some(...)`;
- `None` produces `CompiledProject::generated_artifact_bindings() == None`;
- no-profile mode must not synthesize a `ProfileId`, topology identity, empty product, or catalog;
- a selected profile with no generated requirements is `Some(real empty selected product)`, not `None`.

## 4. Semantic propagation

At adapter registration, the existing semantic callable record receives `AdapterFunctionOrigin`. Overload selection returns the chosen record including origin. Runtime lowering evidence receives the origin immediately at the successful semantic decision.

The following are forbidden:

- looking up a binding ID from the final callable label;
- re-walking the adapter manifest after type checking;
- inferring generated status from the mount prefix;
- storing `Option<GeneratedArtifactBindingId>` in a side map keyed by expression text;
- treating a generated function value as an ordinary named function and recovering identity on apply.

## 5. Runtime-plan forms

### Direct full call

```text
checked origin = GeneratedArtifact(id)
-> RuntimeExpr::Call { callee: RuntimeCallTarget::GeneratedArtifact(id), ... }
```

### Function reference

```text
checked origin = GeneratedArtifact(id)
-> RuntimeValue::Function(RuntimeFunctionValue {
     body: RuntimeFunctionBody::GeneratedArtifact(id),
     params: exact remaining signature params,
     captures: []
   })
```

### Partial call

Passed arguments become deterministic captures in the existing function-value model. The remaining parameter list is retained and the body remains `GeneratedArtifact(id)`. Applying the completed function does not construct or parse a callable name.

## 6. Compiler cross-product verification

The verifier walks all runtime expressions and nested runtime function values that can contain generated IDs. For every ID:

- checked `u32 -> usize` conversion succeeds;
- product requirement exists at that exact position;
- requirement ID equals the position;
- requirement kind is Function;
- requirement topology equals product topology.

The verifier accepts `Option<&GeneratedArtifactBindingLaunchProduct>`. `None` is valid only when no generated function ID or generated Activity launch selection exists. With `Some`, it verifies the product itself before plan correlation. Any failure is `runtime-binding-product-invalid` and prevents a `CompiledProject` from being returned.

An Activity requirement is not encoded as a `RuntimeCallTarget`. Runtime launch assembly obtains the exact `GeneratedArtifactActivitySelection` from the accepted product, verifies its abstract Activity and `ActivityImplementationId`, and stores that typed selection on the generated Activity launch record. The host catalog is still resolved only by `selection.binding()`; there is no catalog API by Activity ID.

## 7. Host catalog assembly

A host receives `Some(launch_product)` and chooses which requirements it can satisfy. No catalog can be constructed for a no-profile `None` context. For each available provider result it registers an already constructed typed binding with the requirement ID and provider-claimed complete key.

A typical exact in-memory test flow is:

```rust
let product = Arc::new(product_fixture());
let requirement = product.requirements()[0].clone();
let mut builder = GeneratedArtifactBindingCatalogBuilder::<FunctionSentinel, ActivitySentinel>::new(product);
builder.register_function(
    requirement.id(),
    requirement.key().clone(),
    FunctionSentinel(7),
)?;
let catalog = builder.freeze();
let selected = catalog.resolve_function(requirement.key().topology(), requirement.id())?;
assert_eq!(selected, &FunctionSentinel(7));
```

This proves exact deterministic selection only. It opens no artifact and executes no generated code.

## 8. Function fail-closed gate

At the point the runtime would otherwise create a host task/callback:

1. inspect the typed generated variant;
2. resolve using active topology and ID;
3. map error to the runtime's structured host-boundary failure;
4. return before output/request queues are mutated;
5. only after success may a higher execution boundary consume the binding.

Tests use counters and pre/post snapshots of host-request/task queues to prove zero work on failure.

## 9. Activity fail-closed gate

At the point a selected generated Activity implementation would otherwise be instantiated:

1. receive the pre-projected `GeneratedArtifactActivitySelection` and verify it belongs to the active product/implementation;
2. use its typed binding ID;
3. resolve the Activity binding;
4. on failure, preserve all state exactly:
   - no Activity state allocation committed;
   - no `ActivityHostRegistry` insertion;
   - no interaction target publication;
   - no start/action/host event;
   - no scheduler or task enqueue;
5. after success, continue to the future Activity implementation construction boundary.

Existing concrete `ActivityHostRegistry::step` behavior is not a substitute for this gate because registry entries exist only after construction.

## 10. LSP replacement

An accepted LSP candidate already groups compiled project, semantic world, project snapshot, and overlays. Add the binding lease/catalog to that same generation-owned publication unit. Replacement order is:

1. build complete new candidate/product;
2. obtain host registrations for the candidate as applicable;
3. publish a new generation atomically;
4. old environment/catalog remains reachable only through old `Arc` holders;
5. any operation presenting an old generation to current state receives stale;
6. no catalog slots are copied into the new generation.

The catalog's own topology check remains mandatory even after the LSP generation check.
