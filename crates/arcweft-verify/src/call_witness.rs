//! Bounded Proof projection over complete checker-owned Call facts.

use std::sync::Arc;

use arcweft_lang_hir::identity::ExprId;
use arcweft_lang_sema::{
    callable::{CallAnalysisOutcome, CallTargetFacts, CallableCandidateId},
    types::TypeKind,
};

const MAX_PROOF_CALL_CANDIDATE_WITNESSES: usize = 2;

/// Session-bound Proof evidence projected for one checked Call.
///
/// The semantic resolver retains the complete candidate set under its
/// production ceiling. Proof keeps only the primary candidate and first
/// distinct conflict candidate while preserving exact truncation accounting.
/// A result is projected only for a selected application; unselected
/// outcomes own candidate evidence but no result type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofCallWitnessProjection {
    expression: ExprId,
    candidate_witnesses: Arc<[CallableCandidateId]>,
    result: Option<TypeKind>,
    considered_count: usize,
    omitted_count: usize,
}

impl ProofCallWitnessProjection {
    pub(crate) fn from_facts(facts: &CallTargetFacts) -> Self {
        let outcome = facts.outcome();
        let primary = outcome.primary_candidate_id();
        let conflicts = outcome.candidate_ids().cloned().collect::<Vec<_>>();
        let considered = outcome
            .considered_candidate_ids()
            .cloned()
            .collect::<Vec<_>>();
        let (candidate_witnesses, omitted_count) =
            retain_candidate_witnesses(primary, &conflicts, &considered);
        let result = match outcome {
            CallAnalysisOutcome::Selected(application) => Some(application.result().ty().clone()),
            CallAnalysisOutcome::Ambiguous(_)
            | CallAnalysisOutcome::Rejected(_)
            | CallAnalysisOutcome::NonCallable(_)
            | CallAnalysisOutcome::Missing(_) => None,
        };
        Self {
            expression: facts.expression(),
            candidate_witnesses,
            result,
            considered_count: considered.len(),
            omitted_count,
        }
    }

    /// Returns the final-HIR Call expression that owns this projection.
    pub const fn expression(&self) -> ExprId {
        self.expression
    }

    /// Returns the primary candidate and optional first distinct conflict.
    pub fn candidate_witnesses(&self) -> &[CallableCandidateId] {
        &self.candidate_witnesses
    }

    /// Returns the selected call result, if this outcome was selected.
    pub const fn result(&self) -> Option<&TypeKind> {
        self.result.as_ref()
    }

    /// Returns the complete semantic candidate count before Proof projection.
    pub const fn considered_count(&self) -> usize {
        self.considered_count
    }

    /// Returns the number of complete semantic candidates omitted from Proof.
    pub const fn omitted_count(&self) -> usize {
        self.omitted_count
    }
}

fn retain_candidate_witnesses(
    primary: Option<&CallableCandidateId>,
    conflicts: &[CallableCandidateId],
    considered: &[CallableCandidateId],
) -> (Arc<[CallableCandidateId]>, usize) {
    let mut retained = Vec::with_capacity(considered.len().min(MAX_PROOF_CALL_CANDIDATE_WITNESSES));
    if let Some(primary) = primary {
        retained.push(primary.clone());
    }
    for candidates in [conflicts, considered] {
        for candidate in candidates {
            if retained.iter().any(|retained| retained == candidate) {
                continue;
            }
            if retained.len() == MAX_PROOF_CALL_CANDIDATE_WITNESSES {
                break;
            }
            retained.push(candidate.clone());
        }
    }
    let omitted_count = considered
        .len()
        .checked_sub(retained.len())
        .expect("primary Call witness belongs to the complete considered set");
    (retained.into(), omitted_count)
}

#[cfg(test)]
mod tests {
    use arcweft_lang_sema::callable::{BuiltinCallableId, CallableCandidateId};

    use super::retain_candidate_witnesses;

    fn candidate(id: BuiltinCallableId) -> CallableCandidateId {
        CallableCandidateId::Builtin(id)
    }

    #[test]
    fn t_lim_12_009_two_candidates_retain_two_witnesses_without_omission() {
        let first = candidate(BuiltinCallableId::Panic);
        let primary = candidate(BuiltinCallableId::Fail);
        let considered = [first.clone(), primary.clone()];

        let (retained, omitted_count) =
            retain_candidate_witnesses(Some(&primary), &considered, &considered);

        assert_eq!(retained.as_ref(), &[primary, first]);
        assert_eq!(considered.len(), 2);
        assert_eq!(omitted_count, 0);
    }

    #[test]
    fn t_lim_12_010_three_candidates_retain_two_witnesses_and_one_omission() {
        let first = candidate(BuiltinCallableId::Panic);
        let primary = candidate(BuiltinCallableId::Fail);
        let omitted = candidate(BuiltinCallableId::Bail);
        let considered = [first.clone(), primary.clone(), omitted];

        let (retained, omitted_count) =
            retain_candidate_witnesses(Some(&primary), &considered, &considered);

        assert_eq!(retained.as_ref(), &[primary, first]);
        assert_eq!(considered.len(), 3);
        assert_eq!(omitted_count, 1);
    }

    #[test]
    fn proof_call_witness_order_is_deterministic_across_retry() {
        let first = candidate(BuiltinCallableId::Panic);
        let primary = candidate(BuiltinCallableId::Fail);
        let third = candidate(BuiltinCallableId::Bail);
        let considered = [first, primary.clone(), third];

        let first_projection = retain_candidate_witnesses(Some(&primary), &considered, &considered);
        let retry_projection = retain_candidate_witnesses(Some(&primary), &considered, &considered);

        assert_eq!(first_projection, retry_projection);
    }

    #[test]
    fn ambiguity_conflict_precedes_an_earlier_rejected_candidate() {
        let rejected = candidate(BuiltinCallableId::Panic);
        let primary = candidate(BuiltinCallableId::Fail);
        let conflict = candidate(BuiltinCallableId::Bail);
        let conflicts = [primary.clone(), conflict.clone()];
        let considered = [rejected, primary.clone(), conflict.clone()];

        let (retained, omitted_count) =
            retain_candidate_witnesses(Some(&primary), &conflicts, &considered);

        assert_eq!(retained.as_ref(), &[primary, conflict]);
        assert_eq!(omitted_count, 1);
    }
}
