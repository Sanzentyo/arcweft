//! Materialization of the closed callable group chain selected by discovery.

use arcweft_core::plan::{
    RuntimeCallableAttachedContract, RuntimeCallableDefault, RuntimeCallableInputSource,
    RuntimeCallableParameterCoordinate, RuntimeCallableParameterInput,
    RuntimeCallableParameterKind, RuntimeCallablePosition, RuntimeCallableRetainedInput,
    RuntimeCallableRetainedRole, RuntimeCallableStateDefinition, RuntimeCallableStateSeedId,
    RuntimeCallableTransition,
};
use arcweft_lang_sema::callable::CallableParameterPresence;

use super::*;

pub(super) type ProjectCallableStates =
    BTreeMap<RuntimeProjectFunctionInstanceKey, Box<[RuntimeCallableStateSeedId]>>;

pub(super) fn materialize(
    facts: &RuntimePlanSemanticFacts,
    sites: &BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    defaults: &BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> ProjectCallableStates {
    let mut states = BTreeMap::new();
    for instance in facts.project_function_instances() {
        match materialize_instance(instance, sites, defaults, builder) {
            Ok(group_states) => {
                states.insert(instance.key().clone(), group_states);
            }
            Err(error) => errors.push(error),
        }
    }
    states
}

fn materialize_instance(
    instance: &RuntimeProjectFunctionInstanceFact,
    sites: &BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    defaults: &BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    builder: &mut RuntimePlanBuilder,
) -> Result<Box<[RuntimeCallableStateSeedId]>, RuntimePlanLowerError> {
    let error = |message: &str| {
        RuntimePlanLowerError::new(format!(
            "callable state for {:?}: {message}",
            instance.key()
        ))
    };
    let terminal = instance.key().group().get();
    let function = sites
        .get(instance.key())
        .ok_or_else(|| error("body site is absent"))?;
    let states = (0..=terminal)
        .map(|_| {
            builder
                .reserve_callable_state_seed()
                .map_err(|failure| error(&failure.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut ty = instance.callable_type();
    for group in 0..=terminal {
        let RuntimeTypeShape::Function { result, .. } = ty.shape() else {
            return Err(error("closed group chain is incomplete"));
        };
        let retained: Box<[_]> = instance
            .parameters()
            .iter()
            .filter(|parameter| parameter.group().get() < group)
            .map(|parameter| RuntimeCallableRetainedInput {
                role: RuntimeCallableRetainedRole::Parameter(RuntimeCallableParameterCoordinate {
                    group: parameter.group().get() as u32,
                    parameter: parameter.parameter(),
                }),
                ty: parameter.binding_ty().identity(),
            })
            .collect();
        let parameters: Box<[_]> = instance
            .parameters()
            .iter()
            .filter(|parameter| parameter.group().get() == group)
            .map(|parameter| RuntimeCallableParameterInput {
                coordinate: RuntimeCallableParameterCoordinate {
                    group: group as u32,
                    parameter: parameter.parameter(),
                },
                kind: match parameter.kind() {
                    HirParameterKind::Fixed | HirParameterKind::ExtensionReceiver => {
                        RuntimeCallableParameterKind::Fixed
                    }
                    HirParameterKind::RestPositional => RuntimeCallableParameterKind::Rest,
                },
                abi_ty: parameter.abi_ty().identity(),
                binding_ty: parameter.binding_ty().identity(),
            })
            .collect();
        let attached = if group != terminal {
            RuntimeCallableAttachedContract::None
        } else {
            match instance.callable().attached_content_abi() {
                None => RuntimeCallableAttachedContract::None,
                Some(attached) => match attached.presence() {
                    CallableParameterPresence::Required => {
                        RuntimeCallableAttachedContract::Required {
                            ty: attached.binding_ty().identity(),
                        }
                    }
                    CallableParameterPresence::Optional => {
                        let RuntimeTypeShape::Option { item, .. } = attached.binding_ty().shape()
                        else {
                            return Err(error("optional attached binding is not Option"));
                        };
                        RuntimeCallableAttachedContract::Optional {
                            value: item.identity(),
                            binding: attached.binding_ty().identity(),
                        }
                    }
                    CallableParameterPresence::Defaulted => {
                        let default = instance
                            .attached_default()
                            .ok_or_else(|| error("attached default fact is absent"))?;
                        let function = defaults
                            .get(instance.key())
                            .cloned()
                            .ok_or_else(|| error("attached default site is absent"))?;
                        let captures = default
                            .captures()
                            .iter()
                            .map(|capture| match capture.source() {
                                RuntimeProjectFunctionParameterSource::ContinuationPrefix {
                                    position,
                                } => RuntimeCallableInputSource::Retained { position },
                                RuntimeProjectFunctionParameterSource::CurrentGroup {
                                    position,
                                } => RuntimeCallableInputSource::Argument { position },
                            })
                            .collect();
                        RuntimeCallableAttachedContract::Defaulted {
                            ty: attached.binding_ty().identity(),
                            default: RuntimeCallableDefault { function, captures },
                        }
                    }
                },
            }
        };
        let transition = if group == terminal {
            let captures = (0..retained.len())
                .map(|position| RuntimeCallableInputSource::Retained {
                    position: position as u32,
                })
                .collect();
            let arguments = (0..parameters.len())
                .map(|position| RuntimeCallableInputSource::Argument {
                    position: position as u32,
                })
                .chain(
                    (!matches!(attached, RuntimeCallableAttachedContract::None))
                        .then_some(RuntimeCallableInputSource::Attached),
                )
                .collect();
            RuntimeCallableTransition::Invoke {
                function: function.clone(),
                captures,
                arguments,
            }
        } else {
            let values = (0..retained.len())
                .map(|position| RuntimeCallableInputSource::Retained {
                    position: position as u32,
                })
                .chain(
                    (0..parameters.len()).map(|position| RuntimeCallableInputSource::Argument {
                        position: position as u32,
                    }),
                )
                .collect();
            RuntimeCallableTransition::Retain {
                state: states[group + 1].clone(),
                values,
            }
        };
        builder
            .define_callable_state_seed(
                &states[group],
                RuntimeCallableStateDefinition {
                    function_type: ty.identity(),
                    origin: states[0].clone(),
                    position: if group == 0 {
                        RuntimeCallablePosition::Unapplied
                    } else {
                        RuntimeCallablePosition::AfterGroup {
                            completed: group as u32 - 1,
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
