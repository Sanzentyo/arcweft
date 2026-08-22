use arcweft_lang_hir::identity::{ExprId, HirSnapshotId};
use arcweft_lang_sema::final_analysis::CheckedMatchRef;

fn forge(snapshot: HirSnapshotId, expression: ExprId) -> CheckedMatchRef {
    CheckedMatchRef {
        snapshot,
        expression,
    }
}

fn main() {}
