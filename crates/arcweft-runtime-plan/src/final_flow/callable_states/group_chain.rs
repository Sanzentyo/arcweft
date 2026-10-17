//! One group-state construction law for admitted declaration bodies.

use crate::{
    flow::RuntimePlanLowerError,
    semantic_facts::{RuntimeNormalizedType, RuntimeTypeShape},
};
use arcweft_core::{
    pattern::RuntimeSemanticTypeId,
    plan::{
        RuntimeCallableAttachedContract, RuntimeCallableInputSource, RuntimeCallableParameterInput,
        RuntimeCallablePosition, RuntimeCallableRetainedInput, RuntimeCallableRetainedRole,
        RuntimeCallableStateDefinition, RuntimeCallableStateSeedId, RuntimeCallableTransition,
        RuntimeFunctionSiteSeedId, RuntimePlanBuilder,
    },
};

pub(in crate::flow) struct CallableGroupChain<'a> {
    function_type: &'a RuntimeNormalizedType,
    parameters: &'a [RuntimeCallableParameterInput<RuntimeSemanticTypeId>],
    terminal: usize,
    function: &'a RuntimeFunctionSiteSeedId,
    attached: RuntimeCallableAttachedContract<RuntimeSemanticTypeId, RuntimeFunctionSiteSeedId>,
}

impl<'a> CallableGroupChain<'a> {
    pub(in crate::flow) fn new(
        function_type: &'a RuntimeNormalizedType,
        parameters: &'a [RuntimeCallableParameterInput<RuntimeSemanticTypeId>],
        terminal: usize,
        function: &'a RuntimeFunctionSiteSeedId,
        attached: RuntimeCallableAttachedContract<RuntimeSemanticTypeId, RuntimeFunctionSiteSeedId>,
    ) -> Self {
        Self {
            function_type,
            parameters,
            terminal,
            function,
            attached,
        }
    }

    pub(in crate::flow) fn materialize_from(
        self,
        start: usize,
        origin: Option<RuntimeCallableStateSeedId>,
        builder: &mut RuntimePlanBuilder,
    ) -> Result<Box<[RuntimeCallableStateSeedId]>, RuntimePlanLowerError> {
        let error = |message: &str| RuntimePlanLowerError::new(message);
        if start > self.terminal
            || u32::try_from(self.parameters.len()).is_err()
            || self
                .parameters
                .iter()
                .any(|row| row.coordinate.group as usize > self.terminal)
            || self
                .parameters
                .windows(2)
                .any(|rows| rows[0].coordinate >= rows[1].coordinate)
        {
            return Err(error(
                "callable group chain has invalid declaration coordinates",
            ));
        }
        let first = u32::try_from(start)
            .map_err(|_| error("callable first group exceeds its coordinate"))?;
        let last = u32::try_from(self.terminal)
            .map_err(|_| error("callable terminal group exceeds its coordinate"))?;
        // Check the complete declared group chain before reserving its states.
        // A function-valued body result is not another declaration group.
        let mut ty = self.function_type;
        for _ in 0..=self.terminal {
            let RuntimeTypeShape::Function { result, .. } = ty.shape() else {
                return Err(error("callable declaration group chain is incomplete"));
            };
            ty = result;
        }
        let states = (start..=self.terminal)
            .map(|_| {
                builder
                    .reserve_callable_state_seed()
                    .map_err(|failure| error(&failure.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let origin = origin.unwrap_or_else(|| states[0].clone());
        let mut ty = self.function_type;
        for _ in 0..start {
            let RuntimeTypeShape::Function { result, .. } = ty.shape() else {
                unreachable!("the complete declaration chain was checked")
            };
            ty = result;
        }
        for (group, coordinate) in (start..=self.terminal).zip(first..=last) {
            let index = group - start;
            let RuntimeTypeShape::Function { result, .. } = ty.shape() else {
                unreachable!("the complete declaration chain was checked")
            };
            let retained: Box<[_]> = self
                .parameters
                .iter()
                .filter(|row| (row.coordinate.group as usize) < group)
                .map(|row| RuntimeCallableRetainedInput {
                    role: RuntimeCallableRetainedRole::Parameter(row.coordinate),
                    ty: row.binding_ty,
                })
                .collect();
            let parameters: Box<[_]> = self
                .parameters
                .iter()
                .filter(|row| row.coordinate.group as usize == group)
                .cloned()
                .collect();
            let attached = if group == self.terminal {
                self.attached.clone()
            } else {
                RuntimeCallableAttachedContract::None
            };
            let captures: Box<[_]> = retained
                .iter()
                .zip(0_u32..)
                .map(|(_, position)| RuntimeCallableInputSource::Retained { position })
                .collect();
            let arguments = parameters
                .iter()
                .zip(0_u32..)
                .map(|(_, position)| RuntimeCallableInputSource::Argument { position });
            let transition = if group == self.terminal {
                RuntimeCallableTransition::Invoke {
                    function: self.function.clone(),
                    captures,
                    arguments: arguments
                        .chain(
                            (!matches!(attached, RuntimeCallableAttachedContract::None))
                                .then_some(RuntimeCallableInputSource::Attached),
                        )
                        .collect(),
                }
            } else {
                RuntimeCallableTransition::Retain {
                    state: states[index + 1].clone(),
                    values: captures.into_vec().into_iter().chain(arguments).collect(),
                }
            };
            builder
                .define_callable_state_seed(
                    &states[index],
                    RuntimeCallableStateDefinition {
                        function_type: ty.identity(),
                        origin: origin.clone(),
                        position: if group == 0 {
                            RuntimeCallablePosition::Unapplied
                        } else {
                            RuntimeCallablePosition::AfterGroup {
                                completed: coordinate - 1,
                            }
                        },
                        retained,
                        parameters,
                        result: result.identity(),
                        attached,
                        transition,
                        partials: Box::new([]),
                    },
                )
                .map_err(|failure| error(&failure.to_string()))?;
            ty = result;
        }
        Ok(states.into_boxed_slice())
    }
}
