use arcweft_lang_hir::model::HirModule;
use arcweft_lang_sema::{
    registration::RegisteredSemanticWorld,
    signature::{SignatureQuery, SignatureQueryControl},
};
use arcweft_source::identity::SourceSnapshotId;

fn attempt_snapshot_identity_query<'a>(
    world: &'a RegisteredSemanticWorld,
    snapshot: &'a SourceSnapshotId,
    hir: &'a HirModule,
    control: SignatureQueryControl<'a>,
) {
    let _ = SignatureQuery::production(world, snapshot, hir, 0, control);
}

fn main() {}
