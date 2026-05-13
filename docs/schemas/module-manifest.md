# Module Manifest Schema Sketch

```rust
pub struct ModuleSummary {
    pub path: ModulePath,
    pub source_hash: Hash,
    pub exports: Vec<ExportedItem>,
    pub imports: Vec<ImportSummary>,
    pub lazy_policy: LazyPolicy,
}

pub struct ExportedItem {
    pub entity_id: EntityId,
    pub public_id: PublicId,
    pub name: Symbol,
    pub kind: ModuleItemKind,
    pub visibility: Visibility,
    pub type_signature: TypeSignature,
    pub purity: Purity,
    pub phase: EvalPhase,
    pub contracts: Vec<Contract>,
}
```

