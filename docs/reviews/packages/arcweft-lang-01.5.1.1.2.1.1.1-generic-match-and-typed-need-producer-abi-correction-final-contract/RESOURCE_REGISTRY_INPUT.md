# Resource type registry input and equality contract

## Existing owner retained

`arcweft-resource-model::registry::ResourceTypeRegistry` is immutable, canonically ordered, and owns `ResourceTypeRegistryDigest`. Its existing `verify_integrity()` recomputes schema and registry digests. This contract does not define another digest type or algorithm.

## Dependency and constructor

Add the acyclic workspace dependency:

```text
arcweft-lang-sema -> arcweft-resource-model
```

`FinalSemanticCatalogs::production(world, resource_types)` becomes a fallible, non-const constructor. It calls `resource_types.verify_integrity()` before returning and freezes `resource_types.digest()` beside the borrow. Tests may use `ResourceTypeRegistry::empty()` or an immutable fixture; they may not construct a digest directly.

## Final analysis publication

`FinalSemanticAnalysis` retains the exact digest used to classify resource-backed semantic types and View expressions. Staged analysis cannot publish if the registry integrity check fails or if the registry borrow does not match the compiler's accepted compilation context.

## Compiler call sites

The production compiler changes its one call to:

```rust
let semantic_catalogs = FinalSemanticCatalogs::production(
    registered_world.as_ref(),
    context.resource_types(),
)?;
```

The same `context.resource_types()` reference is passed to `ViewProjectLowerer::for_project`, which already borrows `ResourceTypeRegistry` and stores its digest in `CompiledViewProduct`.

All direct call sites are updated in the same compile-clean cut: compiler project compilation, sema final-analysis tests, and LSP signature-cache integration rows.

## Equality gate

Before complete checked View catalog publication and again before bundle construction:

```rust
let expected = context.resource_types().digest();
ensure!(analysis.resource_type_registry_digest() == expected);
ensure!(compiled_view.resource_type_registry_digest() == expected);
ensure!(reactive_section.resource_types() == expected);
```

The errors are typed as integrity failure, semantic registry mismatch, View product registry mismatch, or bundle section registry mismatch. They occur before mutation/publication and are never converted to an empty registry.

## Stale input test

A registry object from another publication with a different digest, even if its display names overlap, is stale. Replacing or mutating registry bytes after analysis causes integrity/digest mismatch and rejects View catalog/bundle publication. Metadata timestamps and source spelling do not participate.
