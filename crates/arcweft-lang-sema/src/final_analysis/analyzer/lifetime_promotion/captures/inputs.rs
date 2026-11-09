//! The checked argument mapper and the lexical frame of each supplied group.

use std::collections::BTreeSet;

use crate::{
    callable::{CallableGroupIndex, CallableParameterCoordinate},
    final_analysis::analyzer::calls::AnalyzerPreparedCallPrefix,
};

use super::{
    Analyzer, FinalSemanticAnalysisError, ReceiverBorrowQuery, call_storage_error,
    worklist::{
        BorrowDependencies, CaptureWorklist, Invocation, InvocationId, ParameterDemand,
        StorageFrameId, ValueOrigin,
    },
};

pub(super) struct InvocationInputs {
    pub(super) prefix: Invocation,
    pub(super) requester: Option<InvocationId>,
    pub(super) suffix: Vec<SuffixInput>,
}

impl InvocationInputs {
    pub(super) fn selected(call: Invocation, requester: Option<InvocationId>) -> Self {
        Self {
            prefix: call,
            requester,
            suffix: Vec::new(),
        }
    }
}

pub(super) struct SuffixInput {
    pub(super) call: Invocation,
    pub(super) requester: InvocationId,
    pub(super) group: CallableGroupIndex,
}

impl Analyzer<'_, '_, '_> {
    pub(super) fn substitute_call_parameters(
        &self,
        (frame, inputs): (StorageFrameId, InvocationInputs),
        origin: ValueOrigin,
        parameters: BTreeSet<ParameterDemand>,
        work: &mut CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        let mut mapped = Vec::new();
        let mut external = BTreeSet::new();
        for parameter in parameters {
            receiver.charge(1)?;
            if parameter.frame == frame && parameter.invocation_caller.is_none() {
                mapped.push(parameter);
            } else {
                external.insert(parameter);
            }
        }
        let mut dependencies = BorrowDependencies::Parameters(external);
        if mapped.is_empty() {
            return Ok(dependencies);
        }
        let graph = self.facts.prepared_calls()?;
        graph
            .project_site_payload(
                inputs.prefix.site,
                |prefix| {
                    let enclosing = prefix.record().enclosing_callable();
                    let mut input = prefix;
                    loop {
                        receiver.charge(1)?;
                        if input.record().enclosing_callable() == enclosing {
                            let group = input.application().completed_group();
                            Self::bind_input_group(
                                input,
                                (group, inputs.prefix.caller, inputs.requester),
                                &mapped,
                                origin,
                                (&mut dependencies, work),
                                receiver,
                            )?;
                            if dependencies.is_borrowed() {
                                break;
                            }
                        }
                        let Some(previous) = input.application().selected().prepared_continuation()
                        else {
                            break;
                        };
                        input = graph
                            .continuation_prefix(previous)
                            .map_err(|error| call_storage_error(inputs.prefix.site, error))?;
                    }
                    Ok(())
                },
                |_| Err(FinalSemanticAnalysisError::InvalidOwner),
            )
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)??;
        for suffix in inputs.suffix {
            if dependencies.is_borrowed() {
                break;
            }
            receiver.charge(1)?;
            graph
                .project_site_payload(
                    suffix.call.site,
                    |prefix| {
                        Self::bind_input_group(
                            prefix,
                            (suffix.group, suffix.call.caller, Some(suffix.requester)),
                            &mapped,
                            origin,
                            (&mut dependencies, work),
                            receiver,
                        )
                    },
                    |_| Err(FinalSemanticAnalysisError::InvalidOwner),
                )
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)??;
        }
        Ok(dependencies)
    }

    fn bind_input_group(
        input: &AnalyzerPreparedCallPrefix,
        (declared_group, caller, requester): (
            CallableGroupIndex,
            StorageFrameId,
            Option<InvocationId>,
        ),
        demands: &[ParameterDemand],
        origin: ValueOrigin,
        (dependencies, work): (&mut BorrowDependencies, &mut CaptureWorklist),
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let supplied_group = input.application().completed_group();
        for demand in demands {
            receiver.charge(1)?;
            if demand.coordinate.group() != declared_group {
                continue;
            }
            let coordinate =
                CallableParameterCoordinate::new(supplied_group, demand.coordinate.parameter());
            if let Some(expression) = input.parameter_value_source(coordinate) {
                let mut supplied = work.demand(
                    ValueOrigin {
                        expression,
                        frame: caller,
                        observation: demand.observation,
                    },
                    origin,
                    receiver,
                )?;
                if let Some(requester) = requester {
                    supplied = supplied.map_parameters(|mut demand| {
                        if demand.invocation_caller.is_none() {
                            demand.invocation_caller = Some(requester);
                        }
                        demand
                    });
                }
                dependencies.include(supplied);
                if dependencies.is_borrowed() {
                    break;
                }
            }
        }
        Ok(())
    }
}
