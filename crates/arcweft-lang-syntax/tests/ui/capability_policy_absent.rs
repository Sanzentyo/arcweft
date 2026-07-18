use arcweft_lang_syntax::ast::items::{CapabilityPolicyDecl, ExternCapabilityItem};

fn reject_policy_accessors(item: &ExternCapabilityItem) {
    let _ = item.policy();
    let _ = item.policies();
}

fn main() {
    let _: Option<CapabilityPolicyDecl> = None;
}
