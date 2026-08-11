# Lang-01.1.1.2.1 final contract

**Status:** FINAL — implementation-ready, no unresolved owner-model choice, no fallback-success path.

**Repository:** `Sanzentyo/arcweft`  
**Baseline branch:** `main`  
**Baseline commit:** `4fd6331dc342d30a7f4ac7774852b60801866ef7`  
**Request Git blob:** `5445ff2e48c47a4cb2455b56fb5348784038beb6`  
**Request SHA-256:** `80ec56d3c7afdbbc96550416f0c7a86b4d32649755011052dde4d1f202c6bde5`

## Final decision

`Ref<Entity>` remains canonical authored Arcweft syntax. The existing closed,
language-owned `BuiltinTypeConstructor` is corrected by adding
`BuiltinTypeConstructor::Ref`. `Ref`, `Speaker`, and `SpeakerPreset` are the
closed entity-family-projection subset of that enum. They share one typed
argument-selection and validation rule and differ only in the final inherent
projection:

- `Ref<E>` → `TypeKind::entity_ref(E)` → `TypeKind::Ref(EntityType { kind: E, value: None })`
- `Speaker<E>` → `TypeKind::Speaker(E)`
- `SpeakerPreset<E>` → `TypeKind::SpeakerPreset(E)`

`AcceptedNominalSemantics` remains exactly `Exact | Opaque | Character`.
No accepted-record projection, spelling fallback, compatibility alias, second
resolver, or `Named("Ref<...>")` representation is admitted.

## Package map

- `FINAL_CONTRACT.md` — normative decisions and invariants.
- `API_SHAPES.md` — exact Rust/API changes prescribed for implementation.
- `OWNERSHIP_COLLISIONS.md` — layer ownership and collision rules.
- `DIAGNOSTICS_POISON_SOURCE.md` — diagnostics, poison, source, and work tables.
- `CONSUMERS_TOOLING_PERSISTENCE.md` — checker/callable/entry/index/LSP/runtime/schema contract.
- `CHANGE_SURFACE.md` — repository paths and bounded implementation intent.
- `IMPLEMENTATION_ORDER.md` — mandatory direct-final implementation sequence.
- `TEST_MATRIX.csv` — exhaustive typed behavior matrix.
- `TRACEABILITY.csv` — request-to-contract-to-test mapping.
- `REPOSITORY_EVIDENCE.csv` — inspected repository evidence at the pinned commit.
- `VALIDATION_EVIDENCE.md` — validation performed and its exact boundary.
- `NON_GOALS.md` — explicitly excluded designs and migrations.
- `contract.json` — machine-readable final decisions.
- `REQUEST.md` — exact request that this contract answers.
- `MANIFEST.json` — SHA-256 manifest for every package member except itself.
- `validate_package.py` — offline integrity and contract-completeness validator.
- `VALIDATION_RESULTS.txt` — final package validation result.

This ZIP changes no production source. It is a standalone contract artifact for
a subsequent implementation change.
