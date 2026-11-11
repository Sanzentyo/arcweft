//! Materialization of checked callable sources and selected group chains.

#[path = "callable_states/closed_sources.rs"]
mod closed_sources;
#[path = "callable_states/group_chain.rs"]
mod group_chain;
pub(super) use group_chain::CallableGroupChain;

use crate::semantic_facts::{
    RuntimeCallableSpecializationFact, RuntimeCallableSpecializationKey,
    RuntimeProjectCallableSourceFact, RuntimeProjectCallableSourceKey,
    RuntimeProjectCallableValueTarget,
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
        if source.body().is_some() && source.key() != source.root() {
            continue;
        }
        if source.position().group().get() == 0
            && source.position().parameter().get() == 0
            && let Some(state) = source
                .body()
                .and_then(|body| states.get(body))
                .and_then(|states| states.first())
        {
            sources.insert(source.key().clone(), state.clone());
            continue;
        }
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
    let closed =
        match closed_sources::materialize(facts, &states, &mut sources, sites, defaults, builder) {
            Ok(closed) => closed,
            Err(error) => {
                errors.push(error);
                BTreeMap::new()
            }
        };
    for source in facts.callable_sources() {
        if let Err(error) = materialize_source(source, &sources, builder) {
            errors.push(error);
        }
    }
    let mut specializations = BTreeMap::new();
    let mut targets = BTreeMap::new();
    for specialization in facts.callable_specializations() {
        match materialize_specialization(specialization, facts, &sources, &closed, builder) {
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
    let group = u32::try_from(plan.position().group().get())
        .map_err(|_| error("current group exceeds state coordinate"))?;
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
    let definition = retaining_state(
        group,
        plan.application_type().identity(),
        abi.function_type().identity(),
        origin.clone(),
        target.clone(),
        retained,
        parameters,
    )?;
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

fn retaining_state(
    group: u32,
    function_type: RuntimeSemanticTypeId,
    result: RuntimeSemanticTypeId,
    origin: RuntimeCallableStateSeedId,
    next: RuntimeCallableStateSeedId,
    retained: Box<[RuntimeCallableRetainedInput<RuntimeSemanticTypeId>]>,
    parameters: Box<[RuntimeCallableParameterInput<RuntimeSemanticTypeId>]>,
) -> Result<RuntimeCallableStateSeed, RuntimePlanLowerError> {
    let error = RuntimePlanLowerError::new;
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
    Ok(RuntimeCallableStateDefinition {
        function_type,
        origin,
        position: RuntimeCallablePosition::from_retained_parameters(group, &retained),
        retained,
        parameters,
        result,
        attached: RuntimeCallableAttachedContract::None,
        transition: RuntimeCallableTransition::Retain {
            state: next,
            values,
        },
        partials: Box::new([]),
    })
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
    if source.body().is_some() {
        return Ok(());
    }
    let group = source.group().get();
    let group = u32::try_from(group).map_err(|_| error("group exceeds state coordinate"))?;
    let RuntimeTypeShape::Function { result, .. } = source.function_type().shape() else {
        return Err(error("function type is absent"));
    };
    let retained: Box<[_]> = source
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
                position: RuntimeCallablePosition::from_retained_parameters(group, &retained),
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
    closed: &closed_sources::ClosedCallableStates,
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
    for (source_key, selection) in specialization.selections() {
        let source = facts
            .callable_source(source_key)
            .ok_or_else(|| error("source is absent"))?;
        let source_state = sources
            .get(source_key)
            .ok_or_else(|| error("source state is absent"))?;
        let origin = sources
            .get(source.root())
            .ok_or_else(|| error("source root is absent"))?;
        let target = match selection {
            RuntimeProjectCallableValueTarget::Closed {
                instance: instance_key,
                position,
            } => closed
                .get(&(source.root().clone(), instance_key.clone(), *position))
                .cloned()
                .ok_or_else(|| error("closed source layout is absent"))?,
            RuntimeProjectCallableValueTarget::Source(next_key) => {
                let next = facts
                    .callable_source(next_key)
                    .ok_or_else(|| error("retained source is absent"))?;
                let next_state = sources
                    .get(next_key)
                    .ok_or_else(|| error("retained source state is absent"))?;
                let definition = retained_specialization_state(
                    source,
                    next,
                    specialization.target(),
                    origin.clone(),
                    next_state.clone(),
                )?;
                let state = builder
                    .reserve_callable_state_seed()
                    .map_err(|failure| error(&failure.to_string()))?;
                builder
                    .define_callable_state_seed(&state, definition)
                    .map_err(|failure| error(&failure.to_string()))?;
                state
            }
        };
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

fn retained_specialization_state(
    source: &RuntimeProjectCallableSourceFact,
    next: &RuntimeProjectCallableSourceFact,
    specialized: &RuntimeNormalizedType,
    origin: RuntimeCallableStateSeedId,
    next_state: RuntimeCallableStateSeedId,
) -> Result<RuntimeCallableStateSeed, RuntimePlanLowerError> {
    let error = |message: &str| {
        RuntimePlanLowerError::new(format!(
            "retained callable specialization {:?}: {message}",
            source.key(),
        ))
    };
    let RuntimeTypeShape::Function {
        parameters, result, ..
    } = specialized.shape()
    else {
        return Err(error("specialized input type is not a function"));
    };
    let current = next
        .retained()
        .strip_prefix(source.retained())
        .ok_or_else(|| error("next source changed the retained input prefix"))?;
    if parameters.len() != current.len() || parameters.len() != source.parameters().len() {
        return Err(error("next source does not retain every current formal"));
    }
    let parameters = source
        .parameters()
        .iter()
        .zip(parameters.iter().zip(current))
        .map(|(formal, (abi, binding))| {
            if binding.role != RuntimeCallableRetainedRole::Parameter(formal.coordinate) {
                return Err(error(
                    "next source retained a different parameter coordinate",
                ));
            }
            Ok(RuntimeCallableParameterInput {
                coordinate: formal.coordinate,
                kind: formal.kind,
                abi_ty: abi.identity(),
                binding_ty: binding.ty.identity(),
            })
        })
        .collect::<Result<_, _>>()?;
    let group =
        u32::try_from(source.group().get()).map_err(|_| error("group exceeds state coordinate"))?;
    retaining_state(
        group,
        specialized.identity(),
        result.identity(),
        origin,
        next_state,
        source
            .retained()
            .iter()
            .map(|input| RuntimeCallableRetainedInput {
                role: input.role,
                ty: input.ty.identity(),
            })
            .collect(),
        parameters,
    )
}

fn materialize_instance(
    instance: &RuntimeProjectFunctionInstanceFact,
    sites: &BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    defaults: &BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    builder: &mut RuntimePlanBuilder,
) -> Result<Box<[RuntimeCallableStateSeedId]>, RuntimePlanLowerError> {
    materialize_instance_from(
        instance,
        RuntimeCallableParameterCoordinate {
            group: 0,
            parameter: 0,
        },
        instance.callable_type(),
        None,
        None,
        None,
        sites,
        defaults,
        builder,
    )
}

fn materialize_instance_from(
    instance: &RuntimeProjectFunctionInstanceFact,
    start: RuntimeCallableParameterCoordinate,
    initial_type: &RuntimeNormalizedType,
    origin: Option<RuntimeCallableStateSeedId>,
    initial_state: Option<RuntimeCallableStateSeedId>,
    following_groups: Option<&[RuntimeCallableStateSeedId]>,
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
    let function = sites
        .get(instance.key())
        .ok_or_else(|| error("body site is absent"))?;
    let parameters = instance
        .parameters()
        .iter()
        .map(|parameter| {
            Ok(RuntimeCallableParameterInput {
                coordinate: RuntimeCallableParameterCoordinate {
                    group: u32::try_from(parameter.group().get())
                        .map_err(|_| error("parameter group exceeds its coordinate"))?,
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
        })
        .collect::<Result<Vec<_>, RuntimePlanLowerError>>()?;
    let attached = match instance.callable().attached_content_abi() {
        None => RuntimeCallableAttachedContract::None,
        Some(attached) => match attached.presence() {
            CallableParameterPresence::Required => RuntimeCallableAttachedContract::Required {
                ty: attached.binding_ty().identity(),
            },
            CallableParameterPresence::Optional => {
                let RuntimeTypeShape::Option { item, .. } = attached.binding_ty().shape() else {
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
                        RuntimeProjectFunctionParameterSource::ContinuationPrefix { position } => {
                            RuntimeCallableInputSource::Retained { position }
                        }
                        RuntimeProjectFunctionParameterSource::CurrentGroup { position } => {
                            RuntimeCallableInputSource::Argument { position }
                        }
                    })
                    .collect();
                RuntimeCallableAttachedContract::Defaulted {
                    ty: attached.binding_ty().identity(),
                    default: RuntimeCallableDefault::Body { function, captures },
                }
            }
        },
    };
    CallableGroupChain::new(
        instance.callable_type(),
        &parameters,
        instance.key().group().get(),
        function,
        attached,
    )
    .materialize_from(
        start,
        initial_type,
        origin,
        initial_state,
        following_groups,
        builder,
    )
    .map_err(|failure| error(&failure.to_string()))
}
