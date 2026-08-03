use arcweft_lang_hir::identity::{ExprId, HirModuleId, RawHirId, SyntheticOwner};

fn raw_owner(owner: SyntheticOwner) -> RawHirId {
    owner.raw_for_fingerprint()
}

fn numeric_slots(owner: SyntheticOwner, module: HirModuleId, expression: ExprId) {
    let _ = owner.slot();
    let _ = module.slot();
    let _ = expression.slot();
}

fn main() {}
