use arcweft_lang_hir::identity::{
    CaptureId, ExprId, HirDatabaseId, HirModuleId, HirRevision, HirSnapshotId, ItemId,
    LocalGeneration, LocalId, PatternId, RawHirIdView, ScopeId, StmtId, SyntheticOwner, TypeId,
};
use arcweft_lang_hir::symbol::ProofArtifactId;
use arcweft_source::identity::{SourceGeneration, SourceSnapshotId};

fn requires_serialize<T: serde::Serialize>() {}
fn requires_deserialize<T: serde::de::DeserializeOwned>() {}

macro_rules! require_serde {
    ($($identity:ty),+ $(,)?) => {
        $(
            requires_serialize::<$identity>();
            requires_deserialize::<$identity>();
        )+
    };
}

fn main() {
    require_serde!(
        HirDatabaseId,
        HirModuleId,
        HirRevision,
        HirSnapshotId,
        ItemId,
        ScopeId,
        LocalId,
        ExprId,
        StmtId,
        TypeId,
        PatternId,
        CaptureId,
        RawHirIdView,
        LocalGeneration,
        SourceGeneration,
        SourceSnapshotId,
        SyntheticOwner,
        ProofArtifactId,
    );
}
