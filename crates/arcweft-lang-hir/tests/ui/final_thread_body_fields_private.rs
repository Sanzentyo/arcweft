use arcweft_lang_hir::expr::{HirThreadBody, HirThreadFlowItem};
use arcweft_lang_hir::identity::ScopeId;

fn raw_construct(scope: ScopeId, items: Box<[HirThreadFlowItem]>) -> HirThreadBody {
    HirThreadBody { scope, items }
}

fn main() {}
