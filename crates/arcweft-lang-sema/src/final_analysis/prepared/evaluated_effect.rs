use crate::callable::{CallableEvaluatedEffect, CallableSignatureSchemaDigest, CheckedCallSite};

/// Analyzer-owned evaluated-effect metadata awaiting the post-call seal.
///
/// The statement pass records only the terminal checked call site, its schema
/// identity, and callable-owned disposition. Physical operands and policy
/// semantics are projected from the final checked application later.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedEvaluatedEffect {
    site: CheckedCallSite,
    schema: CallableSignatureSchemaDigest,
    disposition: CallableEvaluatedEffect,
}

impl PreparedEvaluatedEffect {
    pub(crate) const fn new(
        site: CheckedCallSite,
        schema: CallableSignatureSchemaDigest,
        disposition: CallableEvaluatedEffect,
    ) -> Self {
        Self {
            site,
            schema,
            disposition,
        }
    }

    pub(crate) const fn site(&self) -> CheckedCallSite {
        self.site
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        CheckedCallSite,
        CallableSignatureSchemaDigest,
        CallableEvaluatedEffect,
    ) {
        (self.site, self.schema, self.disposition)
    }
}
