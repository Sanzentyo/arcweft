//! Materialization of checked callable sources and selected group chains.

use crate::semantic_facts::{
    RuntimeCallableSpecializationFact, RuntimeCallableSpecializationKey,
    RuntimeProjectCallableSourceFact, RuntimeProjectCallableSourceKey,
};
use arcweft_core::plan::{
    RuntimeCallableAttachedContract, RuntimeCallableDefault, RuntimeCallableInputSource,
    RuntimeCallableParameterCoordinate, RuntimeCallableParameterInput,
    RuntimeCallableParameterKind, RuntimeCallablePosition, RuntimeCallableRetainedInput,
    RuntimeCallableRetainedRole, RuntimeCallableSpecializationDefinition,
    RuntimeCallableSpecializationSeedId, RuntimeCallableSpecializationState,
    RuntimeCallableStateDefinition, RuntimeCallableStateSeed, RuntimeCallableStateSeedId,
    RuntimeCallableTransition, RuntimeFunctionSpecializationArguments,
};
use arcweft_lang_sema::callable::{
    CallableParameterPresence, CheckedProjectFunctionCallableSourceDigest,
};

use super::*;

pub(super) type ProjectCallableStates =
    BTreeMap<RuntimeProjectFunctionInstanceKey, Box<[RuntimeCallableStateSeedId]>>;
pub(super) type ProjectCallableSourceStates =
    BTreeMap<RuntimeProjectCallableSourceKey, RuntimeCallableStateSeedId>;
pub(super) type CallableSpecializationSeeds =
    BTreeMap<RuntimeCallableSpecializationKey, RuntimeCallableSpecializationSeedId>;
pub(super) type ProjectCallableApplicationStates =
    BTreeMap<(RuntimeProjectCallableSourceKey, RuntimeSemanticTypeId), RuntimeCallableStateSeedId>;
pub(super) type CallableSpecializationTargetStates = BTreeMap<
    (
        RuntimeCallableSpecializationKey,
        CheckedProjectFunctionCallableSourceDigest,
    ),
    RuntimeCallableStateSeedId,
>;

pub(super) fn materialize(
    facts: &RuntimePlanSemanticFacts,
    sites: &BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    defaults: &BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> (
    ProjectCallableStates,
    ProjectCallableSourceStates,
    CallableSpecializationSeeds,
    ProjectCallableApplicationStates,
    CallableSpecializationTargetStates,
) {
    let mut states = BTreeMap::new();
    for instance in facts.project_function_instances() {
        match materialize_instance(instance, sites, defaults, builder) {
            Ok(group_states) => {
                states.insert(instance.key().clone(), group_states);
            }
            Err(error) => errors.push(error),
        }
    }
    let mut sources = BTreeMap::new();
    for source in facts.callable_sources() {
        match builder.reserve_callable_state_seed() {
            Ok(state) => {
                sources.insert(source.key().clone(), state);
            }
            Err(error) => errors.push(RuntimePlanLowerError::new(format!(
                "callable source {:?}: {error}",
                source.key()
            ))),
        }
    }
    for source in facts.callable_sources() {
        if let Err(error) = materialize_source(source, &sources, builder) {
            errors.push(error);
        }
    }
    let mut specializations = BTreeMap::new();
    let mut targets = BTreeMap::new();
    for specialization in facts.callable_specializations() {
        match materialize_specialization(specialization, facts, &sources, sites, defaults, builder)
        {
            Ok((seed, rows)) => {
                for (digest, target) in rows {
                    if targets
                        .insert((specialization.key().clone(), digest), target)
                        .is_some()
                    {
                        errors.push(RuntimePlanLowerError::new(format!(
                            "callable specialization {:?} repeats a source digest",
                            specialization.key()
                        )));
                    }
                }
                specializations.insert(specialization.key().clone(), seed);
            }
            Err(error) => errors.push(error),
        }
    }
    let applications = materialize_applications(facts, &sources, builder, errors);
    (states, sources, specializations, applications, targets)
}

fn materialize_applications(
    facts: &RuntimePlanSemanticFacts,
    sources: &ProjectCallableSourceStates,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> ProjectCallableApplicationStates {
    let mut definitions = BTreeMap::<
        (RuntimeProjectCallableSourceKey, RuntimeSemanticTypeId),
        RuntimeCallableStateSeed,
    >::new();
    let mut visit = |_: ExprId, call: &RuntimeResolvedCall| {
        let Some(plan) = call.project_function() else {
            return;
        };
        if let Err(error) = collect_application(plan, facts, sources, &mut definitions) {
            errors.push(error);
        }
    };
    for (owner, call) in facts.calls() {
        visit(owner, call);
    }
    for instance in facts.project_function_instances() {
        instance.visit_calls(&mut visit);
    }
    for closure in facts.root_closures() {
        closure.semantics().visit_calls(&mut visit);
    }
    let mut applications = BTreeMap::new();
    for (key, definition) in definitions {
        let state = match builder.reserve_callable_state_seed() {
            Ok(state) => state,
            Err(failure) => {
                errors.push(RuntimePlanLowerError::new(format!(
                    "callable application {key:?}: {failure}"
                )));
                continue;
            }
        };
        if let Err(failure) = builder.define_callable_state_seed(&state, definition) {
            errors.push(RuntimePlanLowerError::new(format!(
                "callable application {key:?}: {failure}"
            )));
            continue;
        }
        applications.insert(key, state);
    }
    applications
}

fn collect_application(
    plan: &crate::semantic_facts::RuntimeProjectFunctionCallPlan,
    facts: &RuntimePlanSemanticFacts,
    sources: &ProjectCallableSourceStates,
    definitions: &mut BTreeMap<
        (RuntimeProjectCallableSourceKey, RuntimeSemanticTypeId),
        RuntimeCallableStateSeed,
    >,
) -> Result<(), RuntimePlanLowerError> {
    let crate::semantic_facts::RuntimeProjectFunctionCallOutcome::Continue {
        abi,
        next_group,
        target: crate::semantic_facts::RuntimeProjectCallableValueTarget::Source(key),
    } = plan.outcome()
    else {
        return Ok(());
    };
    let error = |message: &str| {
        RuntimePlanLowerError::new(format!("callable application for {key:?}: {message}"))
    };
    let source = facts
        .callable_source(key)
        .ok_or_else(|| error("target source is absent"))?;
    let target = sources
        .get(key)
        .ok_or_else(|| error("target source state is absent"))?;
    let origin = sources
        .get(source.root())
        .ok_or_else(|| error("root source state is absent"))?;
    let group = next_group
        .get()
        .checked_sub(1)
        .and_then(|group| u32::try_from(group).ok())
        .ok_or_else(|| error("current group exceeds state coordinate"))?;
    let RuntimeTypeShape::Function { result, .. } = plan.application_type().shape() else {
        return Err(error("application type is not a function"));
    };
    if result.identity() != abi.function_type().identity() {
        return Err(error(
            "application result is not the checked continuation type",
        ));
    }
    let retained: Box<[_]> = source
        .retained()
        .iter()
        .filter(|input| match input.role {
            RuntimeCallableRetainedRole::Capture { .. } => true,
            RuntimeCallableRetainedRole::Parameter(coordinate) => coordinate.group < group,
        })
        .map(|input| RuntimeCallableRetainedInput {
            role: input.role,
            ty: input.ty.identity(),
        })
        .collect();
    let parameters: Box<[_]> = plan
        .current_group_materialization()
        .iter()
        .map(|input| RuntimeCallableParameterInput {
            coordinate: RuntimeCallableParameterCoordinate {
                group,
                parameter: input.parameter(),
            },
            kind: match input.kind() {
                HirParameterKind::Fixed | HirParameterKind::ExtensionReceiver => {
                    RuntimeCallableParameterKind::Fixed
                }
                HirParameterKind::RestPositional => RuntimeCallableParameterKind::Rest,
            },
            abi_ty: input.abi_ty().identity(),
            binding_ty: input.binding_ty().identity(),
        })
        .collect();
    let values: Box<[_]> = (0..retained.len())
        .map(|position| {
            Ok(RuntimeCallableInputSource::Retained {
                position: u32::try_from(position)
                    .map_err(|_| error("retained input count exceeds state coordinate"))?,
            })
        })
        .chain((0..parameters.len()).map(|position| {
            Ok(RuntimeCallableInputSource::Argument {
                position: u32::try_from(position)
                    .map_err(|_| error("parameter input count exceeds state coordinate"))?,
            })
        }))
        .collect::<Result<_, RuntimePlanLowerError>>()?;
    let definition = RuntimeCallableStateDefinition {
        function_type: plan.application_type().identity(),
        origin: origin.clone(),
        position: if group == 0 {
            RuntimeCallablePosition::Unapplied
        } else {
            RuntimeCallablePosition::AfterGroup {
                completed: group - 1,
            }
        },
        retained,
        parameters,
        result: abi.function_type().identity(),
        attached: RuntimeCallableAttachedContract::None,
        transition: RuntimeCallableTransition::Retain {
            state: target.clone(),
            values,
        },
        partials: Box::new([]),
    };
    let key = (key.clone(), plan.application_type().identity());
    match definitions.entry(key) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(definition);
            Ok(())
        }
        std::collections::btree_map::Entry::Occupied(entry) if entry.get() == &definition => Ok(()),
        std::collections::btree_map::Entry::Occupied(_) => Err(error(
            "same source and application type have different state layouts",
        )),
    }
}

fn materialize_source(
    source: &RuntimeProjectCallableSourceFact,
    sources: &ProjectCallableSourceStates,
    builder: &mut RuntimePlanBuilder,
) -> Result<(), RuntimePlanLowerError> {
    let error = |message: &str| {
        RuntimePlanLowerError::new(format!("callable source {:?}: {message}", source.key()))
    };
    let state = sources
        .get(source.key())
        .ok_or_else(|| error("state is absent"))?;
    let origin = sources
        .get(source.root())
        .ok_or_else(|| error("root state is absent"))?;
    let group = source.group().get();
    let group = u32::try_from(group).map_err(|_| error("group exceeds state coordinate"))?;
    let RuntimeTypeShape::Function { result, .. } = source.function_type().shape() else {
        return Err(error("function type is absent"));
    };
    let retained = source
        .retained()
        .iter()
        .map(|input| RuntimeCallableRetainedInput {
            role: input.role,
            ty: input.ty.identity(),
        })
        .collect();
    let parameters = source
        .parameters()
        .iter()
        .map(|input| RuntimeCallableParameterInput {
            coordinate: input.coordinate,
            kind: input.kind,
            abi_ty: input.abi_ty.identity(),
            binding_ty: input.binding_ty.identity(),
        })
        .collect();
    let attached = match source.attached() {
        RuntimeCallableAttachedContract::None => RuntimeCallableAttachedContract::None,
        RuntimeCallableAttachedContract::Required { ty } => {
            RuntimeCallableAttachedContract::Required { ty: ty.identity() }
        }
        RuntimeCallableAttachedContract::Optional { value, binding } => {
            RuntimeCallableAttachedContract::Optional {
                value: value.identity(),
                binding: binding.identity(),
            }
        }
        RuntimeCallableAttachedContract::Defaulted { ty, .. } => {
            RuntimeCallableAttachedContract::Defaulted {
                ty: ty.identity(),
                default: RuntimeCallableDefault::RequiresSpecialization,
            }
        }
    };
    builder
        .define_callable_state_seed(
            state,
            RuntimeCallableStateDefinition {
                function_type: source.function_type().identity(),
                origin: origin.clone(),
                position: if group == 0 {
                    RuntimeCallablePosition::Unapplied
                } else {
                    RuntimeCallablePosition::AfterGroup {
                        completed: group - 1,
                    }
                },
                retained,
                parameters,
                result: result.identity(),
                attached,
                transition: RuntimeCallableTransition::RequiresSpecialization,
                partials: Box::new([]),
            },
        )
        .map_err(|failure| error(&failure.to_string()))
}

fn materialize_specialization(
    specialization: &RuntimeCallableSpecializationFact,
    facts: &RuntimePlanSemanticFacts,
    sources: &ProjectCallableSourceStates,
    sites: &BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    defaults: &BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    builder: &mut RuntimePlanBuilder,
) -> Result<
    (
        RuntimeCallableSpecializationSeedId,
        Vec<(
            CheckedProjectFunctionCallableSourceDigest,
            RuntimeCallableStateSeedId,
        )>,
    ),
    RuntimePlanLowerError,
> {
    let error = |message: &str| {
        RuntimePlanLowerError::new(format!(
            "callable specialization {:?}: {message}",
            specialization.key()
        ))
    };
    let mut pairs = Vec::new();
    let mut targets = Vec::new();
    for (source_key, instance_key) in specialization.selections() {
        let source = facts
            .callable_source(source_key)
            .ok_or_else(|| error("source is absent"))?;
        let instance = facts
            .project_function_instance(instance_key)
            .ok_or_else(|| error("selected instance is absent"))?;
        let source_state = sources
            .get(source_key)
            .ok_or_else(|| error("source state is absent"))?;
        let origin = sources
            .get(source.root())
            .ok_or_else(|| error("source root is absent"))?;
        let target_states = materialize_instance_from(
            instance,
            source.group().get(),
            Some(origin.clone()),
            sites,
            defaults,
            builder,
        )?;
        let target = target_states
            .first()
            .cloned()
            .ok_or_else(|| error("target chain is empty"))?;
        targets.push((source_key.digest(), target.clone()));
        pairs.push(RuntimeCallableSpecializationState {
            source: source_state.clone(),
            target,
        });
    }
    let arguments = RuntimeFunctionSpecializationArguments {
        types: specialization
            .arguments()
            .types
            .iter()
            .map(RuntimeNormalizedType::identity)
            .collect(),
        const_lengths: specialization.arguments().const_lengths.clone(),
        effects: specialization.arguments().effects.clone(),
    };
    let seed = builder
        .push_callable_specialization_seed(RuntimeCallableSpecializationDefinition {
            source_type: specialization.source().identity(),
            target_type: specialization.target().identity(),
            arguments,
            states: pairs.into_boxed_slice(),
        })
        .map_err(|failure| error(&failure.to_string()))?;
    Ok((seed, targets))
}

fn materialize_instance(
    instance: &RuntimeProjectFunctionInstanceFact,
    sites: &BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    defaults: &BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    builder: &mut RuntimePlanBuilder,
) -> Result<Box<[RuntimeCallableStateSeedId]>, RuntimePlanLowerError> {
    materialize_instance_from(instance, 0, None, sites, defaults, builder)
}

fn materialize_instance_from(
    instance: &RuntimeProjectFunctionInstanceFact,
    start_group: usize,
    origin: Option<RuntimeCallableStateSeedId>,
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
    if start_group > terminal {
        return Err(error("selected group precedes the source"));
    }
    let function = sites
        .get(instance.key())
        .ok_or_else(|| error("body site is absent"))?;
    let states = (start_group..=terminal)
        .map(|_| {
            builder
                .reserve_callable_state_seed()
                .map_err(|failure| error(&failure.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut ty = instance.callable_type();
    for _ in 0..start_group {
        let RuntimeTypeShape::Function { result, .. } = ty.shape() else {
            return Err(error("closed prefix group chain is incomplete"));
        };
        ty = result;
    }
    let origin = origin.unwrap_or_else(|| states[0].clone());
    for group in start_group..=terminal {
        let index = group - start_group;
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
                            default: RuntimeCallableDefault::Body { function, captures },
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
                state: states[index + 1].clone(),
                values,
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
