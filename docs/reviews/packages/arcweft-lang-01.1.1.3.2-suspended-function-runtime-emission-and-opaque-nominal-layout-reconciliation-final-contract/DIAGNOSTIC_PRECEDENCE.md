# Diagnostic and validation precedence

## 1. Compile-time precedence

The first failing category wins in this exact order:

1. final-HIR/symbol/checked-catalog generation mismatch;
2. invalid or missing selected Entry;
3. invalid reachability root/source/target ID;
4. duplicate or conflicting root/edge;
5. missing or mismatched checked project edge for a reached source;
6. reached ordinary-function emission classification;
7. reached runtime type projection;
8. project nominal schema projection, including typed opaque leaf;
9. schema canonical encoding / `TypeLayoutHash` generation;
10. nominal-record descriptor/fact admission;
11. runtime semantic fact admission;
12. RuntimePlan construction/verification;
13. AWBC/native lowering and product verification.

This order is normative. In particular, a reached suspending function returning an opaque-containing nominal reports category 6, not category 8.

## 2. Unsupported ordinary-function reasons

Stable reason enum:

```text
PureDirectFrame                       admitted
EffectfulDirectFrameUnsupported       error
SuspendingDirectFrameUnsupported      error
StreamFactoryUnsupported              error
```

Suggested diagnostic codes:

- `compiler.runtime_emission.effectful_function_unsupported`
- `compiler.runtime_emission.suspending_function_unsupported`
- `compiler.runtime_emission.stream_factory_unsupported`

## 3. Suspension diagnostic payload

The typed error retains:

- exact function `ItemId`;
- exact callable declaration identity;
- `CheckedFunctionExecution`;
- `CheckedSuspensionRole`;
- exact effect set/row;
- deterministic first reachability path;
- first direct checked `Await`/suspension expression, if present.

Rendering:

- primary label: function declaration;
- secondary label: first suspension site;
- secondary labels: each source-backed edge in the first path, capped by existing diagnostic limits;
- note: authored ordinary-function suspension is semantically valid but current runtime lowering is not admitted;
- no suggestion to rename the function or wrap it in a synthetic Flow.

## 4. Opaque project nominal diagnostic

Suggested code:

`compiler.runtime_nominal.opaque_leaf_has_no_schema_layout`

Payload:

- project nominal declaration ID;
- semantic identity;
- typed `NominalSchemaPath`;
- accepted opaque producer ID;
- opaque semantic identity.

Rendering states that the opaque leaf is valid, but the enclosing project nominal requires a closed schema-derived layout. It never says “unknown type”.

## 5. Reachability graph errors

Suggested codes:

- `compiler.runtime_reachability.stale_generation`
- `compiler.runtime_reachability.invalid_root`
- `compiler.runtime_reachability.invalid_edge`
- `compiler.runtime_reachability.duplicate_edge`
- `compiler.runtime_reachability.missing_checked_edge`
- `compiler.runtime_reachability.mismatched_checked_edge`
- `compiler.runtime_reachability.presentation_target`
- `compiler.runtime_reachability.limit_exceeded`

Errors retain typed IDs and source queries. They are not collapsed to `reason: String` at crate boundaries.

## 6. Runtime/save precedence

For opaque values:

1. active artifact/generation/ABI;
2. AWBC type-table reference;
3. producer;
4. semantic identity;
5. opaque admission rule;
6. recursive payload checked type;
7. publication.

For project nominal values:

1. active artifact/catalog descriptor;
2. nominal identity;
3. semantic identity where applicable;
4. schema-derived layout hash;
5. field/case count;
6. defining-order identities;
7. first child checked-type failure;
8. publication.

A raw serde decode or AWBC snapshot DTO is quarantined until this sequence succeeds.

## 7. Deterministic multi-path choice

If a function is reached by multiple roots/paths, diagnostics use the lexicographically first shortest path. Adding an unrelated later-sorted root cannot change the selected path. Reordering insertion into hash maps cannot change output.
