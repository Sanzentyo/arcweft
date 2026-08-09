use arcweft_lang_hir::expr::{HirSelectExpr, HirSelectedMember};
use arcweft_lang_hir::identity::ExprId;
use arcweft_lang_hir::leaf::HirName;

fn raw_construct(target: ExprId, member: HirSelectedMember) -> HirSelectExpr {
    HirSelectExpr { target, member }
}

fn old_constructor(target: ExprId, member: HirName) -> HirSelectExpr {
    HirSelectExpr::new(target, member)
}

fn old_member_accessor(select: &HirSelectExpr) -> &HirName {
    select.member()
}

fn main() {}
