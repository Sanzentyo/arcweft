//! Bounded operational evidence for candidate-specific argument evaluation.

use arcweft_lang_syntax::expr::CallExpr;

use crate::{
    callable::{
        CallableArgumentIndex, CallableArgumentSlotIndex, CallableCandidateId, CallableLimits,
        PRODUCTION_CALLABLE_LIMITS,
    },
    types::TypeKind,
};

use super::{TypeChecker, TypeExpressionId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CandidateEvaluationPass {
    Probe,
    SelectedReplay,
    RejectedRecoveryReplay,
    DirectCommitted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CandidateExpectedType {
    Exact(TypeKind),
    Unchecked,
    Unmapped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PhysicalArgumentEvaluationKind {
    Authored,
    Recovered,
    FixedLiteralSpread,
    TypedRestSpread,
    Unmapped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PhysicalCandidateArgumentEvaluation {
    pub(crate) call_expression: TypeExpressionId,
    pub(crate) candidate: CallableCandidateId,
    pub(crate) pass: CandidateEvaluationPass,
    pub(crate) argument: CallableArgumentIndex,
    pub(crate) slot: CallableArgumentSlotIndex,
    pub(crate) kind: PhysicalArgumentEvaluationKind,
    pub(crate) expected: CandidateExpectedType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CallNodeKey(usize);

impl CallNodeKey {
    fn from_call(call: &CallExpr) -> Self {
        Self(std::ptr::from_ref(call) as usize)
    }
}

struct CandidateEvaluationContext {
    call: CallNodeKey,
    call_expression: TypeExpressionId,
    candidate: CallableCandidateId,
    pass: CandidateEvaluationPass,
    next_slots: Vec<usize>,
}

#[derive(Clone, Copy)]
pub(super) struct CandidateEvaluationScope {
    context_depth: Option<usize>,
}

pub(super) struct PhysicalCandidateEvaluationRecorder {
    contexts: Vec<CandidateEvaluationContext>,
    evaluations: Vec<PhysicalCandidateArgumentEvaluation>,
    evaluation_limit: usize,
    overflowed: bool,
}

impl PhysicalCandidateEvaluationRecorder {
    pub(super) fn new() -> Self {
        Self {
            contexts: Vec::with_capacity(PRODUCTION_CALLABLE_LIMITS.max_nested_calls() + 1),
            evaluations: Vec::new(),
            evaluation_limit: physical_evaluation_limit(PRODUCTION_CALLABLE_LIMITS),
            overflowed: false,
        }
    }

    pub(super) fn is_active(&self) -> bool {
        !self.contexts.is_empty()
    }

    pub(super) fn event_checkpoint(&self) -> usize {
        self.evaluations.len()
    }

    pub(super) fn begin(
        &mut self,
        enabled: bool,
        call: &CallExpr,
        call_expression: TypeExpressionId,
        candidate: &CallableCandidateId,
        pass: CandidateEvaluationPass,
    ) -> CandidateEvaluationScope {
        if !enabled {
            return CandidateEvaluationScope {
                context_depth: None,
            };
        }
        if self.contexts.len()
            >= PRODUCTION_CALLABLE_LIMITS
                .max_nested_calls()
                .saturating_add(1)
        {
            self.overflowed = true;
            return CandidateEvaluationScope {
                context_depth: None,
            };
        }
        let context_depth = self.contexts.len();
        self.contexts.push(CandidateEvaluationContext {
            call: CallNodeKey::from_call(call),
            call_expression,
            candidate: candidate.clone(),
            pass,
            next_slots: vec![0; call.args().len()],
        });
        CandidateEvaluationScope {
            context_depth: Some(context_depth),
        }
    }

    pub(super) fn end(&mut self, scope: CandidateEvaluationScope) {
        let Some(context_depth) = scope.context_depth else {
            return;
        };
        if self.contexts.len() != context_depth + 1 {
            self.overflowed = true;
            self.contexts.truncate(context_depth);
            return;
        }
        self.contexts.pop();
    }

    pub(super) fn record(
        &mut self,
        call: &CallExpr,
        argument: usize,
        kind: PhysicalArgumentEvaluationKind,
        expected: CandidateExpectedType,
    ) {
        let Some(context) = self.contexts.last_mut() else {
            return;
        };
        if context.call != CallNodeKey::from_call(call) {
            return;
        }
        let Ok(argument_id) = CallableArgumentIndex::try_from_usize(argument) else {
            self.overflowed = true;
            return;
        };
        let Some(next_slot) = context.next_slots.get_mut(argument) else {
            self.overflowed = true;
            return;
        };
        let Ok(slot_id) = CallableArgumentSlotIndex::try_from_usize(*next_slot) else {
            self.overflowed = true;
            return;
        };
        *next_slot += 1;
        if self.evaluations.len() >= self.evaluation_limit {
            self.overflowed = true;
            return;
        }
        self.evaluations.push(PhysicalCandidateArgumentEvaluation {
            call_expression: context.call_expression,
            candidate: context.candidate.clone(),
            pass: context.pass,
            argument: argument_id,
            slot: slot_id,
            kind,
            expected,
        });
    }

    pub(super) fn reclassify(
        &mut self,
        checkpoint: usize,
        call_expression: TypeExpressionId,
        candidate: &CallableCandidateId,
        from: CandidateEvaluationPass,
        to: CandidateEvaluationPass,
    ) {
        for evaluation in self.evaluations.iter_mut().skip(checkpoint) {
            if evaluation.call_expression == call_expression
                && &evaluation.candidate == candidate
                && evaluation.pass == from
            {
                evaluation.pass = to;
            }
        }
    }

    pub(super) fn finish(&mut self) -> (Vec<PhysicalCandidateArgumentEvaluation>, bool) {
        self.contexts.clear();
        (
            std::mem::take(&mut self.evaluations),
            std::mem::take(&mut self.overflowed),
        )
    }
}

impl TypeChecker<'_> {
    pub(super) fn begins_physical_candidate_evaluation(
        &self,
        call_span: Option<&arcweft_source::SourceSpan>,
    ) -> bool {
        self.physical_candidate_argument_evaluations.is_active()
            || self.call_target_fact_recorder.wants(call_span)
    }

    pub(super) fn begin_physical_candidate_evaluation(
        &mut self,
        enabled: bool,
        call: &CallExpr,
        call_expression: TypeExpressionId,
        candidate: &CallableCandidateId,
        pass: CandidateEvaluationPass,
    ) -> CandidateEvaluationScope {
        self.physical_candidate_argument_evaluations.begin(
            enabled,
            call,
            call_expression,
            candidate,
            pass,
        )
    }

    pub(super) fn end_physical_candidate_evaluation(&mut self, scope: CandidateEvaluationScope) {
        self.physical_candidate_argument_evaluations.end(scope);
    }

    pub(super) fn physical_candidate_evaluation_checkpoint(&self) -> usize {
        self.physical_candidate_argument_evaluations
            .event_checkpoint()
    }

    pub(super) fn reclassify_physical_candidate_evaluations(
        &mut self,
        checkpoint: usize,
        call_expression: TypeExpressionId,
        candidate: &CallableCandidateId,
        from: CandidateEvaluationPass,
        to: CandidateEvaluationPass,
    ) {
        self.physical_candidate_argument_evaluations.reclassify(
            checkpoint,
            call_expression,
            candidate,
            from,
            to,
        );
    }

    pub(super) fn record_physical_candidate_argument_evaluation(
        &mut self,
        call: &CallExpr,
        argument: usize,
        kind: PhysicalArgumentEvaluationKind,
        expected: CandidateExpectedType,
    ) {
        self.physical_candidate_argument_evaluations
            .record(call, argument, kind, expected);
    }
}

fn physical_evaluation_limit(limits: CallableLimits) -> usize {
    let candidate_passes = limits.max_candidates_per_call().saturating_add(1);
    let slots_per_candidate = limits
        .max_parameters_per_callable()
        .saturating_add(limits.max_recovery_nodes());
    let query_ceiling = usize::try_from(limits.max_query_work()).unwrap_or(usize::MAX);
    let per_depth = candidate_passes
        .saturating_mul(slots_per_candidate)
        .min(query_ceiling);
    per_depth.saturating_mul(limits.max_nested_calls().saturating_add(1))
}
