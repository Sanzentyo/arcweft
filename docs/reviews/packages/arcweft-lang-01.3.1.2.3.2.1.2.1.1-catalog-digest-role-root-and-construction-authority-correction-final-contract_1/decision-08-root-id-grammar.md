# Decision 08 — root-ID creation and canonical coordinate grammar

## Existing semantic authority

The selected root coordinate is the existing `arcweft_core::pattern::RuntimeSemanticTypeId([u8; 32])`. It is already the exact semantic identity paired with every normalized runtime type. Root creation is a lossless typed projection, not another digest.

```rust
impl RuntimeProjectRootId {
    #[must_use]
    pub const fn from_semantic_type(id: RuntimeSemanticTypeId) -> Self {
        Self::from_bytes(*id.as_bytes())
    }
}

impl RuntimeProducerRootId {
    #[must_use]
    pub const fn from_semantic_type(id: RuntimeSemanticTypeId) -> Self {
        Self::from_bytes(*id.as_bytes())
    }
}
```

If `RuntimeSemanticTypeId::as_bytes` is not currently const, make the owning accessor const rather than adding a helper trait.

## Byte grammar

A root coordinate is exactly 32 bytes:

```text
runtime_project_root_id  = runtime_semantic_type_id[32]
runtime_producer_root_id = runtime_semantic_type_id[32]
```

There is no domain string, hash operation, version field, tag, length, text encoding, or ordinal in either root ID. The distinct Rust newtypes provide domain separation. Root declaration arrays retain the parent canonical ordering by the bytewise `Ord` of the corresponding root newtype.

## Excluded identity inputs

The following never contribute to the 32 bytes: source spelling, accepted nominal display path, `TypeKind::Named`, `RuntimeNominalTypeId`, layout hash, HIR database/item/expression/pattern IDs, `SourceSpan`, `RuntimePlan`/AWBC dense IDs, table insertion order, Debug/Display output, source maps, or registry slots. Those values may locate source evidence or a use row, but cannot reconstruct a semantic root.

Every `RuntimeProjectRootFact` and `RuntimeProducerRootFact` must prove that its `checked_type` is the projection of the same `RuntimeSemanticTypeId` before raw declarations are emitted. Two sites with the same semantic identity share one project root declaration but retain two site-use rows. Two producer coordinates with the same semantic identity may share a traversal result, but both coordinate rows remain in the producer fact so role/custom authorization cannot be substituted.
