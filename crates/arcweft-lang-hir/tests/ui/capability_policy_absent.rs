use arcweft_lang_hir::model::{HirCapabilityPolicy, HirTopLevelDecl};

fn reject_policy_variant(item: HirTopLevelDecl) {
    if let HirTopLevelDecl::CapabilityPolicy(_policy) = item {}
}

fn main() {
    let _: Option<HirCapabilityPolicy> = None;
}
