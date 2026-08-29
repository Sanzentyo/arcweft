use arcweft_id::PublicId;
use arcweft_lang_hir::identity::ExprId;
use arcweft_lang_hir::stmt::{
    HirSelectBranchHead, HirTrigger, HirTriggerPattern, HirUnsafeAudit, HirUnsafeAuditIdentity,
};

fn any<T>() -> T {
    panic!("fixture must not run")
}

fn old_select_field(head: HirSelectBranchHead) {
    match head {
        HirSelectBranchHead::Bind {
            propagates_error, ..
        } => {
            let _ = propagates_error;
        }
        HirSelectBranchHead::Frame { .. }
        | HirSelectBranchHead::Event { .. }
        | HirSelectBranchHead::Recovered => {}
    }
}

fn old_unsafe_label(audit: &HirUnsafeAudit) {
    let _ = audit.id_ref_label();
}

fn untyped_identity_inputs() {
    let _ = HirTrigger::Mark(any::<String>());
    let _ = HirTrigger::Mark(any::<PublicId>());
    let _ = HirTrigger::Mark(any::<ExprId>());
    let _ = HirUnsafeAuditIdentity::Accepted(any::<String>());
    let _ = HirUnsafeAuditIdentity::Accepted(any::<PublicId>());
    let _ = HirUnsafeAuditIdentity::Accepted(any::<ExprId>());
}

fn main() {}
