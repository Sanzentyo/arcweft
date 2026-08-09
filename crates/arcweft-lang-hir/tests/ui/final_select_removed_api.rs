use arcweft_lang_hir::expr::HirSelectedMember;
use arcweft_lang_syntax::expressions::ExpressionProjection;

fn standalone_select(_: arcweft_lang_syntax::attachment::AttachedSelectExpr) {}

fn combined_optional_dot(_: arcweft_lang_syntax::expressions::OptionalDot) {}

fn removed_invalid_member(member: HirSelectedMember) {
    if let HirSelectedMember::Invalid(_) = member {}
}

fn removed_optional_flag(projection: ExpressionProjection) {
    if let ExpressionProjection::Select { optional: _, .. } = projection {}
}

fn main() {}
