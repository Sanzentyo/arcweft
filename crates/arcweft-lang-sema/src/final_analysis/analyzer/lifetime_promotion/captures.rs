//! Borrowed storage evidenced by a function value's checked capture origins.

use std::{collections::BTreeSet, sync::Arc};

use arcweft_lang_hir::{
    expr::HirExprKind, identity::ExprId, module::HirModule, project::HirLocalValueOrigin,
};

use super::{Analyzer, FinalSemanticAnalysisError, ReceiverBorrowQuery};
use crate::{
    callable::{
        CheckedCallSite, PreparedCallGraphIngress, PreparedCallSiteContinuation,
        PreparedFunctionValueOriginProducer, PreparedFunctionValueOriginProgress,
        PreparedFunctionValueOriginQueryError,
        prepare_function_value_origin_query_with_pending_captures,
    },
    final_analysis::{CheckedMatchArmFact, PreparedExpressionFact, PreparedOwnerBoundResolution},
    types::TypeKind,
};

impl Analyzer<'_, '_, '_> {
    /// Looks for borrowed storage using existing function-value evidence. An
    /// opaque producer or an unsealed origin is not evidence of ownership.
    pub(super) fn function_value_has_borrowed_captures(
        &self,
        module: &HirModule,
        root: ExprId,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<bool, FinalSemanticAnalysisError> {
        let captures = self
            .pending_capture_identity_rows()
            .map_err(|error| error.into_public(root))?;
        let local_origins = self
            .topology
            .module(module.module_id())
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
            .local_origins();
        let expressions = self.facts.expressions();
        let mut pending = vec![root];
        let mut visited = BTreeSet::new();
        while let Some(expression) = pending.pop() {
            receiver.charge(1)?;
            if !visited.insert(expression)
                || !expressions
                    .get(&expression)
                    .is_some_and(has_function_origin_fact)
            {
                continue;
            }
            let project_error = |error| {
                self.map_function_value_origin_query_error(expression, error)
                    .into_public(expression)
            };
            let mut progress = prepare_function_value_origin_query_with_pending_captures(
                Arc::clone(&self.topology),
                module,
                expression,
                expressions,
                Arc::clone(&captures),
            );
            let evidence = loop {
                match progress {
                    Ok(PreparedFunctionValueOriginProgress::Ready(evidence)) => {
                        break Some(evidence);
                    }
                    Ok(PreparedFunctionValueOriginProgress::Need(need)) => {
                        receiver.charge(1)?;
                        let requested = need.expression();
                        let Some(fact) = expressions
                            .get(&requested)
                            .filter(|fact| has_function_origin_fact(fact))
                        else {
                            break None;
                        };
                        progress = need.resume(requested, fact, module);
                    }
                    Err(PreparedFunctionValueOriginQueryError::Composite) => break None,
                    Err(error) => return Err(project_error(error)),
                }
            };
            let Some(evidence) = evidence else { continue };
            if let PreparedFunctionValueOriginProducer::Call(site) = evidence.producer()
                && self.continuation_has_borrowed_inputs(*site, receiver, &mut pending)?
            {
                return Ok(true);
            }
            for capture in evidence.captures() {
                receiver.charge(1)?;
                let local = capture.local();
                let ty = self
                    .facts
                    .locals()
                    .get(&local)
                    .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: local })?;
                if receiver.contains_borrow(ty)? {
                    return Ok(true);
                }
                if matches!(ty, TypeKind::Function { .. })
                    && let Some(HirLocalValueOrigin::DirectInitializer(initializer)) =
                        local_origins.origin(local)
                {
                    pending.push(initializer);
                }
            }
            if let PreparedFunctionValueOriginProducer::IndependentExpression { producer } =
                evidence.producer()
            {
                self.append_structural_function_results(module, *producer, receiver, &mut pending)?;
            }
        }
        Ok(false)
    }

    fn continuation_has_borrowed_inputs(
        &self,
        site: CheckedCallSite,
        receiver: &mut ReceiverBorrowQuery<'_>,
        pending: &mut Vec<ExprId>,
    ) -> Result<bool, FinalSemanticAnalysisError> {
        use crate::final_analysis::FinalCallConstraintFailure;
        use crate::final_analysis::analyzer::{CallAnalysisFailure, calls::CallAnalysisInvariant};

        let owner = site.expression();
        let ty = self
            .facts
            .expressions()
            .get(&owner)
            .and_then(PreparedExpressionFact::value_type)
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
        let graph = self.facts.prepared_calls().map_err(|error| {
            crate::final_analysis::analyzer::expression_error::AnalyzerExpressionError::fact(error)
                .into_public(owner)
        })?;
        let classify = |error| {
            FinalSemanticAnalysisError::CallConstraintFailure(FinalCallConstraintFailure::new(
                owner,
                CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error)),
            ))
        };
        if !matches!(
            PreparedCallGraphIngress::new(graph)
                .continuation_at(site, ty)
                .map_err(classify)?,
            PreparedCallSiteContinuation::Prepared(_)
        ) {
            return Ok(false);
        }
        graph
            .project_site_payload(
                site,
                |prefix| {
                    for (source, ty) in prefix.record().value_inputs() {
                        receiver.charge(1)?;
                        if receiver.contains_borrow(ty)? {
                            return Ok(true);
                        }
                        if matches!(ty, TypeKind::Function { .. }) {
                            pending.push(source);
                        }
                    }
                    if let Some(callee) = prefix.record().function_value_origin() {
                        receiver.charge(1)?;
                        pending.push(callee.callee());
                    }
                    Ok(false)
                },
                |_| Err(FinalSemanticAnalysisError::InvalidOwner),
            )
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
    }

    fn append_structural_function_results(
        &self,
        module: &HirModule,
        producer: ExprId,
        receiver: &mut ReceiverBorrowQuery<'_>,
        pending: &mut Vec<ExprId>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let kind = module
            .resolve_expr(producer)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .kind();
        match kind {
            HirExprKind::Block(block) => pending.push(block.tail()),
            HirExprKind::If(branches) => {
                receiver.charge(2)?;
                pending.push(branches.else_branch());
                pending.push(branches.then_branch());
            }
            HirExprKind::IfLet(branches) => {
                receiver.charge(2)?;
                pending.push(branches.else_branch());
                pending.push(branches.then_branch());
            }
            HirExprKind::Match(_) => {
                let matched = self
                    .facts
                    .expressions()
                    .get(&producer)
                    .and_then(PreparedExpressionFact::complete)
                    .and_then(crate::final_analysis::CheckedExpression::match_fact)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                receiver.charge(
                    u64::try_from(matched.arms().len())
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                )?;
                pending.extend(matched.arms().iter().rev().map(CheckedMatchArmFact::value));
            }
            _ => {}
        }
        Ok(())
    }
}

fn has_function_origin_fact(fact: &PreparedExpressionFact) -> bool {
    match fact {
        PreparedExpressionFact::Complete(checked) => {
            matches!(checked.value_type(), Some(TypeKind::Function { .. }))
        }
        PreparedExpressionFact::OwnerBound(prepared) => matches!(
            prepared.resolution(),
            PreparedOwnerBoundResolution::ImplicitCallable(_)
        ),
        _ => false,
    }
}
