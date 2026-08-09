use arcweft_lang_hir::identity::{
    CaptureId, ExprId, HirDatabaseId, HirModuleId, HirRevision, HirSnapshotId, ItemId,
    LocalGeneration, LocalId, PatternId, ScopeId, StmtId, TypeId,
};
use arcweft_source::identity::SourceGeneration;
use core::num::{NonZeroU32, NonZeroU64};

fn forge_module(database: HirDatabaseId, slot: NonZeroU32) -> HirModuleId {
    HirModuleId { database, slot }
}

fn forge_snapshot(module: HirModuleId, revision: HirRevision) -> HirSnapshotId {
    HirSnapshotId { module, revision }
}

fn main() {
    let _ = HirDatabaseId(NonZeroU64::MIN);
    let _ = HirRevision(NonZeroU32::MIN);
    let _ = LocalGeneration(NonZeroU32::MIN);
    let _ = SourceGeneration(NonZeroU32::MIN);

    let _ = ItemId;
    let _ = ScopeId;
    let _ = LocalId;
    let _ = ExprId;
    let _ = StmtId;
    let _ = TypeId;
    let _ = PatternId;
    let _ = CaptureId;
}
