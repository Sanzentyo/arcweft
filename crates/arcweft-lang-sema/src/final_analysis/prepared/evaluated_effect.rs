use arcweft_lang_hir::identity::ExprId;

use crate::callable::{CallableEvaluatedEffect, CallableSignatureSchemaDigest, CheckedCallSite};

/// Analyzer-owned evaluated-effect metadata awaiting the post-call seal.
///
/// The statement pass records only the terminal checked call site, its schema
/// identity, and callable-owned disposition. Physical operands and policy
/// semantics are projected from the final checked application later.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedEvaluatedEffect {
    root: ExprId,
    site: CheckedCallSite,
    schema: CallableSignatureSchemaDigest,
    disposition: CallableEvaluatedEffect,
}

impl PreparedEvaluatedEffect {
    pub(crate) const fn new(
        root: ExprId,
        site: CheckedCallSite,
        schema: CallableSignatureSchemaDigest,
        disposition: CallableEvaluatedEffect,
    ) -> Self {
        Self {
            root,
            site,
            schema,
            disposition,
        }
    }

    pub(crate) const fn root(&self) -> ExprId {
        self.root
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        ExprId,
        CheckedCallSite,
        CallableSignatureSchemaDigest,
        CallableEvaluatedEffect,
    ) {
        (self.root, self.site, self.schema, self.disposition)
    }
}
