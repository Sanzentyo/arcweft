//! A returned application retains each completed group's lexical inputs.

use crate::callable::{CallableCandidateId, PreparedResolvedCallable};

use super::{
    Analyzer, AnalyzerExpressionContext, CaptureEvaluation, FinalSemanticAnalysisError,
    ReceiverBorrowQuery, TypeStorage,
    inputs::SuffixInput,
    invocation::SourceInvocation,
    localization::RetainedBodyProjection,
    worklist::{BorrowDependencies, Invocation, Observation, ValueOrigin},
};

impl Analyzer<'_, '_, '_> {
    pub(super) fn invoke_returned_prefix(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        origin: ValueOrigin,
        prefix: Invocation,
        selected: &PreparedResolvedCallable,
        query: &mut CaptureEvaluation,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        let CallableCandidateId::Project(declaration) = selected.id() else {
            return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
        };
        let mut group = selected
            .next_group_for(selected.call_group())
            .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
        let creator = query.work.frame(prefix.caller)?;
        let mut projection = RetainedBodyProjection::new(
            prefix.site,
            self.localize_application(prefix.site, &creator, receiver)?,
        );
        let mut suffix = Vec::new();
        let mut result = prefix.result;
        while let Observation::Invoke(invoked) = result {
            receiver.charge(1)?;
            let call = query.work.invocation(invoked.id)?;
            let expected = selected
                .schema()
                .group(group)
                .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
            self.with_selected_application(call.site, |application| {
                let callee = application.selected();
                let supplied = callee
                    .schema()
                    .group(application.completed_group())
                    .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
                if !callee.requires_value_callee()
                    || supplied.parameters().len() != expected.parameters().len()
                {
                    return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
                }
                Ok(())
            })?;
            if let Some(specialization) = invoked.specialization {
                let lexical = query.work.frame(specialization.frame)?;
                projection.advance(self.localize_specialization(
                    specialization.owner,
                    &lexical,
                    receiver,
                )?)?;
            }
            let caller = query.work.frame(call.caller)?;
            projection.advance(self.localize_application(call.site, &caller, receiver)?)?;
            suffix.push(SuffixInput {
                call,
                requester: invoked.id,
                group,
            });
            let Some(next) = selected.next_group_for(group) else {
                return self.source_result_dependencies(
                    context,
                    origin,
                    SourceInvocation::retained(
                        prefix,
                        suffix,
                        call,
                        declaration.clone(),
                        projection,
                    ),
                    query,
                    receiver,
                );
            };
            group = next;
            result = call.result;
        }
        self.returned_prefix_storage(origin, prefix, &suffix, query, receiver)
    }

    fn returned_prefix_storage(
        &self,
        origin: ValueOrigin,
        prefix: Invocation,
        suffix: &[SuffixInput],
        query: &mut CaptureEvaluation,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        let mut dependencies = query.work.demand(
            ValueOrigin {
                expression: prefix.site.expression(),
                frame: prefix.caller,
                observation: Observation::Stored,
            },
            origin,
            receiver,
        )?;
        let graph = self.facts.prepared_calls()?;
        for input in suffix {
            if dependencies.is_borrowed() {
                break;
            }
            let frame = query.work.frame(input.call.caller)?;
            graph
                .project_site_payload(
                    input.call.site,
                    |site| {
                        for (expression, ty) in site.record().value_inputs() {
                            receiver.charge(1)?;
                            match self.inspect_value_storage(ty, &frame.projections, receiver)? {
                                TypeStorage::Borrowed => {
                                    dependencies = BorrowDependencies::Borrowed
                                }
                                TypeStorage::Ordinary => {}
                                TypeStorage::Callable => {
                                    let supplied = query
                                        .work
                                        .demand(
                                            ValueOrigin {
                                                expression,
                                                frame: input.call.caller,
                                                observation: Observation::Stored,
                                            },
                                            origin,
                                            receiver,
                                        )?
                                        .map_parameters(|mut demand| {
                                            if demand.invocation_caller.is_none() {
                                                demand.invocation_caller = Some(input.requester);
                                            }
                                            demand
                                        });
                                    dependencies.include(supplied);
                                }
                            }
                            if dependencies.is_borrowed() {
                                break;
                            }
                        }
                        Ok(())
                    },
                    |_| Err(FinalSemanticAnalysisError::InvalidOwner),
                )
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)??;
        }
        Ok(dependencies)
    }
}
