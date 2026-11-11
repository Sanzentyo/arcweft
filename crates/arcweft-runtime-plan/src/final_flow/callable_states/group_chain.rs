//! One group-state construction law for admitted declaration bodies.

use crate::{
    flow::RuntimePlanLowerError,
    semantic_facts::{RuntimeNormalizedType, RuntimeTypeShape},
};
use arcweft_core::{
    pattern::RuntimeSemanticTypeId,
    plan::{
        RuntimeCallableAttachedContract, RuntimeCallableDefault, RuntimeCallableInputSource,
        RuntimeCallableParameterCoordinate, RuntimeCallableParameterInput, RuntimeCallablePosition,
        RuntimeCallableRetainedInput, RuntimeCallableRetainedRole, RuntimeCallableStateDefinition,
        RuntimeCallableStateSeedId, RuntimeCallableTransition, RuntimeFunctionSiteSeedId,
        RuntimePlanBuilder,
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
        start: RuntimeCallableParameterCoordinate,
        initial_type: &RuntimeNormalizedType,
        origin: Option<RuntimeCallableStateSeedId>,
        initial_state: Option<RuntimeCallableStateSeedId>,
        following_groups: Option<&[RuntimeCallableStateSeedId]>,
        builder: &mut RuntimePlanBuilder,
    ) -> Result<Box<[RuntimeCallableStateSeedId]>, RuntimePlanLowerError> {
        let error = |message: &str| RuntimePlanLowerError::new(message);
        let start_group = usize::try_from(start.group)
            .map_err(|_| error("callable first group exceeds its coordinate"))?;
        let first_parameter = usize::try_from(start.parameter)
            .map_err(|_| error("callable first parameter exceeds its coordinate"))?;
        let first_group_arity = self
            .parameters
            .iter()
            .filter(|row| row.coordinate.group == start.group)
            .count();
        if start_group > self.terminal
            || following_groups.is_some_and(|states| states.len() != self.terminal - start_group)
            || (first_parameter > 0 && first_parameter >= first_group_arity)
            || ((start_group > 0 || first_parameter > 0) && origin.is_none())
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
        let first = start.group;
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
        let mut ty = self.function_type;
        for _ in 0..start_group {
            let RuntimeTypeShape::Function { result, .. } = ty.shape() else {
                unreachable!("the complete declaration chain was checked")
            };
            ty = result;
        }
        if !initial_type.is_function_suffix_of(ty, first_parameter) {
            return Err(error(
                "callable remaining type differs from its declaration position",
            ));
        }
        let states = (start_group..=self.terminal)
            .map(|group| {
                if group > start_group
                    && let Some(states) = following_groups
                {
                    return Ok(states[group - start_group - 1].clone());
                }
                if group == start_group
                    && let Some(state) = &initial_state
                {
                    return Ok(state.clone());
                }
                builder
                    .reserve_callable_state_seed()
                    .map_err(|failure| error(&failure.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let origin = origin.unwrap_or_else(|| states[0].clone());
        for (group, coordinate) in (start_group..=self.terminal).zip(first..=last) {
            let index = group - start_group;
            let RuntimeTypeShape::Function { result, .. } = ty.shape() else {
                unreachable!("the complete declaration chain was checked")
            };
            if group > start_group && following_groups.is_some() {
                ty = result;
                continue;
            }
            let position = if group == start_group {
                start
            } else {
                RuntimeCallableParameterCoordinate {
                    group: coordinate,
                    parameter: 0,
                }
            };
            let current_type = if group == start_group {
                initial_type
            } else {
                ty
            };
            let retained: Box<[_]> = self
                .parameters
                .iter()
                .filter(|row| row.coordinate < position)
                .map(|row| RuntimeCallableRetainedInput {
                    role: RuntimeCallableRetainedRole::Parameter(row.coordinate),
                    ty: row.binding_ty,
                })
                .collect();
            let parameters: Box<[_]> = self
                .parameters
                .iter()
                .filter(|row| row.coordinate.group == coordinate && row.coordinate >= position)
                .cloned()
                .collect();
            let input_for = |formal: RuntimeCallableParameterCoordinate| {
                if formal < position {
                    let index = retained
                        .iter()
                        .position(|input| {
                            input.role == RuntimeCallableRetainedRole::Parameter(formal)
                        })
                        .ok_or_else(|| error("callable retained formal is absent"))?;
                    Ok(RuntimeCallableInputSource::Retained {
                        position: u32::try_from(index)
                            .map_err(|_| error("retained ordinal exceeds its coordinate"))?,
                    })
                } else {
                    let index = parameters
                        .iter()
                        .position(|input| input.coordinate == formal)
                        .ok_or_else(|| error("callable argument formal is absent"))?;
                    Ok(RuntimeCallableInputSource::Argument {
                        position: u32::try_from(index)
                            .map_err(|_| error("argument ordinal exceeds its coordinate"))?,
                    })
                }
            };
            let mut attached = if group == self.terminal {
                self.attached.clone()
            } else {
                RuntimeCallableAttachedContract::None
            };
            if let RuntimeCallableAttachedContract::Defaulted {
                default: RuntimeCallableDefault::Body { captures, .. },
                ..
            } = &mut attached
            {
                for capture in captures {
                    if let RuntimeCallableInputSource::Argument { position: ordinal } = *capture {
                        let formal = self
                            .parameters
                            .iter()
                            .filter(|row| row.coordinate.group == coordinate)
                            .nth(ordinal as usize)
                            .ok_or_else(|| error("attached default formal is absent"))?;
                        *capture = input_for(formal.coordinate)?;
                    }
                }
            }
            let captures: Box<[_]> = retained
                .iter()
                .zip(0_u32..)
                .filter(|(input, _)| matches!(input.role, RuntimeCallableRetainedRole::Parameter(formal) if formal.group < coordinate))
                .map(|(_, position)| RuntimeCallableInputSource::Retained { position })
                .collect();
            let arguments = self
                .parameters
                .iter()
                .filter(|input| input.coordinate.group == coordinate)
                .map(|input| input_for(input.coordinate))
                .collect::<Result<Vec<_>, RuntimePlanLowerError>>()?;
            let transition =
                if group == self.terminal {
                    RuntimeCallableTransition::Invoke {
                        function: self.function.clone(),
                        captures,
                        arguments: arguments
                            .into_iter()
                            .chain(
                                (!matches!(attached, RuntimeCallableAttachedContract::None))
                                    .then_some(RuntimeCallableInputSource::Attached),
                            )
                            .collect(),
                    }
                } else {
                    RuntimeCallableTransition::Retain {
                        state: states[index + 1].clone(),
                        values: (0_u32..)
                            .zip(retained.iter())
                            .map(|(position, _)| RuntimeCallableInputSource::Retained { position })
                            .chain((0_u32..).zip(parameters.iter()).map(|(position, _)| {
                                RuntimeCallableInputSource::Argument { position }
                            }))
                            .collect(),
                    }
                };
            builder
                .define_callable_state_seed(
                    &states[index],
                    RuntimeCallableStateDefinition {
                        function_type: current_type.identity(),
                        origin: origin.clone(),
                        position: RuntimeCallablePosition::from_retained_parameters(
                            coordinate, &retained,
                        ),
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
