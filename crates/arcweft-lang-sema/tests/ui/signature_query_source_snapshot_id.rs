use arcweft_lang_hir::module::HirModule;
use arcweft_lang_sema::{
    final_analysis::FinalSemanticAnalysis,
    registration::RegisteredSemanticWorld,
    signature::{SignatureQuery, SignatureQueryControl},
};
use arcweft_source::identity::SourceSnapshotId;

fn attempt_snapshot_identity_query<'a>(
    world: &'a RegisteredSemanticWorld,
    snapshot: &'a SourceSnapshotId,
    hir: &'a HirModule,
    analysis: &'a FinalSemanticAnalysis,
    control: SignatureQueryControl<'a>,
) {
    let _ = SignatureQuery::production(world, snapshot, hir, analysis, 0, control);
}

fn main() {}
