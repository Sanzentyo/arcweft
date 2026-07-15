//! Proof obligations derived from typed assertion statements.

use super::{ObligationCollector, ProofDischarge, ProofObligationKind, span_from_range};
use arcweft_lang_hir::syntax::assertion::{AssertionMode, AssertionStmt};

impl ObligationCollector {
    pub(super) fn collect_assertion(&mut self, assertion: &AssertionStmt) {
        for (index, condition) in assertion.conditions().iter().enumerate() {
            self.collect_expr(condition);
            if assertion.mode() == AssertionMode::Prove {
                self.add_assertion_proof_obligation(assertion, index);
            }
        }
    }

    fn add_assertion_proof_obligation(&mut self, assertion: &AssertionStmt, index: usize) {
        self.record_obligation(
            ProofObligationKind::AssertionProof,
            format!("assert.prove condition {index} requires compile-time discharge"),
            Some(format!("condition.{index}")),
            &ProofDischarge::Missing,
            None,
            Some(span_from_range(&assertion.range())),
            Some("verify.proof.unresolved"),
        );
    }
}
