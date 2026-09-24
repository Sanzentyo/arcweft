//! Issuer-bound construction of the program's reusable callable state graph.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeCallableAttachedContract, RuntimeCallableInputSource,
    RuntimeCallableParameterCoordinate, RuntimeCallableParameterInput,
    RuntimeCallableParameterKind, RuntimeCallablePartialTransition, RuntimeCallablePosition,
    RuntimeCallableRetainedInput, RuntimeCallableRetainedRole, RuntimeCallableState,
    RuntimeCallableStateDefinition, RuntimeCallableStateError, RuntimeCallableStateTable,
    RuntimeCallableTransition, RuntimeFunctionInputSource,
};
use crate::runtime_id::{RuntimeCallableStateId, RuntimeFunctionSiteId, RuntimePlanTypeId};

use super::seed::RuntimePlanConstructionIssuer;
use super::{RuntimeFunctionSiteSeedId, RuntimePlanBuildError, RuntimePlanBuilder};

/// A reserved callable state. Only its issuing builder can resolve it.
#[derive(Clone)]
pub struct RuntimeCallableStateSeedId {
    issuer: Arc<RuntimePlanConstructionIssuer>,
    state: RuntimeCallableStateId,
}

impl fmt::Debug for RuntimeCallableStateSeedId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RuntimeCallableStateSeedId")
            .field(&self.state)
            .finish()
    }
}

impl PartialEq for RuntimeCallableStateSeedId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.issuer, &other.issuer) && self.state == other.state
    }
}

impl Eq for RuntimeCallableStateSeedId {}

impl RuntimeCallableStateSeedId {
    pub(super) fn resolve(
        &self,
        issuer: &Arc<RuntimePlanConstructionIssuer>,
    ) -> Option<RuntimeCallableStateId> {
        Arc::ptr_eq(&self.issuer, issuer).then_some(self.state)
    }
}

/// One state before semantic and executable references enter the program.
pub type RuntimeCallableStateSeed = RuntimeCallableStateDefinition<
    RuntimeSemanticTypeId,
    RuntimeFunctionSiteSeedId,
    RuntimeCallableStateSeedId,
>;

/// Construction owns interning and the finite set of checked partial-arrow
/// demands. The final program contains only the admitted state table.
#[derive(Debug, Default)]
pub(super) struct RuntimeCallableStateBuilder {
    pub(super) states: Vec<Option<RuntimeCallableState>>,
    initial: BTreeMap<(RuntimeFunctionSiteId, RuntimePlanTypeId), RuntimeCallableStateId>,
    partials: BTreeSet<(RuntimePlanTypeId, RuntimePlanTypeId, usize)>,
}

impl RuntimeCallableStateBuilder {
    fn reserve(&mut self) -> Result<RuntimeCallableStateId, RuntimePlanBuildError> {
        let state = RuntimeCallableStateId::for_index(self.states.len())
            .ok_or(RuntimeCallableStateError::IdentityExhausted)?;
        self.states.push(None);
        Ok(state)
    }

    pub(super) fn request_partial(
        &mut self,
        source: RuntimePlanTypeId,
        result: RuntimePlanTypeId,
        arguments: usize,
    ) {
        self.partials.insert((source, result, arguments));
    }

    /// Makes all checked partial requests concrete before publication. No body
    /// target is discovered here: every state refers to a pre-reserved site.
    pub(super) fn seal(&mut self) -> Result<(), RuntimePlanBuildError> {
        let demands = self.partials.iter().copied().collect::<Vec<_>>();
        let mut index = 0;
        while index < self.states.len() {
            let Some(source) = self.states[index].clone() else {
                index += 1;
                continue;
            };
            let mut partials = Vec::new();
            for &(_, function_type, count) in demands
                .iter()
                .filter(|(ty, _, _)| *ty == source.function_type)
            {
                let arrow_arity = source.parameters.len()
                    + usize::from(!matches!(
                        source.attached,
                        RuntimeCallableAttachedContract::None
                    ));
                if count == 0 || count >= arrow_arity || count > source.parameters.len() {
                    continue;
                }
                let retained_count = source.retained.len();
                let remap = |input| match input {
                    RuntimeCallableInputSource::Argument { position }
                        if (position as usize) < count =>
                    {
                        RuntimeCallableInputSource::Retained {
                            position: (retained_count + position as usize) as u32,
                        }
                    }
                    RuntimeCallableInputSource::Argument { position } => {
                        RuntimeCallableInputSource::Argument {
                            position: position - count as u32,
                        }
                    }
                    input => input,
                };
                let map_sources = |sources: &[RuntimeCallableInputSource]| {
                    sources.iter().copied().map(remap).collect()
                };
                let parameters: Box<[_]> = source.parameters[..count]
                    .iter()
                    .map(|input| input.coordinate)
                    .collect();
                let (group, mut bound) = match &source.position {
                    RuntimeCallablePosition::Unapplied => (0, Vec::new()),
                    RuntimeCallablePosition::WithinGroup { group, bound } => {
                        (*group, bound.to_vec())
                    }
                    RuntimeCallablePosition::AfterGroup { completed } => {
                        (completed + 1, Vec::new())
                    }
                };
                bound.extend_from_slice(&parameters);
                bound.sort_unstable();
                let position = RuntimeCallablePosition::WithinGroup {
                    group,
                    bound: bound.into_boxed_slice(),
                };
                let target = self.states.iter().enumerate().find_map(|(index, state)| {
                    state
                        .as_ref()
                        .filter(|state| {
                            state.origin == source.origin
                                && state.position == position
                                && state.function_type == function_type
                        })
                        .and_then(|_| RuntimeCallableStateId::for_index(index))
                });
                let target = if let Some(target) = target {
                    target
                } else {
                    let target = self.reserve()?;
                    let mut retained = source.retained.to_vec();
                    retained.extend(source.parameters[..count].iter().map(|input| {
                        RuntimeCallableRetainedInput {
                            role: RuntimeCallableRetainedRole::Parameter(input.coordinate),
                            ty: input.binding_ty,
                        }
                    }));
                    let attached = match source.attached.clone() {
                        RuntimeCallableAttachedContract::Defaulted { ty, mut default } => {
                            if let crate::plan::RuntimeCallableDefault::Body { captures, .. } =
                                &mut default
                            {
                                *captures = map_sources(captures);
                            }
                            RuntimeCallableAttachedContract::Defaulted { ty, default }
                        }
                        attached => attached,
                    };
                    let transition = match &source.transition {
                        RuntimeCallableTransition::RequiresSpecialization => {
                            RuntimeCallableTransition::RequiresSpecialization
                        }
                        RuntimeCallableTransition::Retain { state, values } => {
                            RuntimeCallableTransition::Retain {
                                state: *state,
                                values: map_sources(values),
                            }
                        }
                        RuntimeCallableTransition::Invoke {
                            function,
                            captures,
                            arguments,
                        } => RuntimeCallableTransition::Invoke {
                            function: *function,
                            captures: map_sources(captures),
                            arguments: map_sources(arguments),
                        },
                    };
                    self.states[target.index()] = Some(RuntimeCallableStateDefinition {
                        function_type,
                        origin: source.origin,
                        position,
                        retained: retained.into_boxed_slice(),
                        parameters: source.parameters[count..].into(),
                        result: source.result,
                        attached,
                        transition,
                        partials: Box::new([]),
                    });
                    target
                };
                let values = (0..source.retained.len())
                    .map(|position| RuntimeCallableInputSource::Retained {
                        position: position as u32,
                    })
                    .chain(
                        (0..count).map(|position| RuntimeCallableInputSource::Argument {
                            position: position as u32,
                        }),
                    )
                    .collect();
                partials.push(RuntimeCallablePartialTransition {
                    parameters,
                    state: target,
                    values,
                });
            }
            let current = self.states[index]
                .as_mut()
                .expect("source state was present");
            current.partials = current.partials.iter().cloned().chain(partials).collect();
            index += 1;
        }
        Ok(())
    }

    pub(super) fn finish(self) -> RuntimeCallableStateTable {
        RuntimeCallableStateTable::from_admitted(
            self.states
                .into_iter()
                .map(|state| state.expect("all states were defined before publication"))
                .collect(),
        )
    }
}

impl RuntimePlanBuilder {
    /// Reserves an identity before defining mutually recursive callable origins.
    pub fn reserve_callable_state_seed(
        &mut self,
    ) -> Result<RuntimeCallableStateSeedId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let state = self.callable_states.borrow_mut().reserve()?;
        Ok(RuntimeCallableStateSeedId {
            issuer: Arc::clone(&self.issuer),
            state,
        })
    }

    /// Defines a reservation atomically. Invalid references leave it undefined.
    /// Whole-graph transition and body-layout validation happens at `finish`.
    pub fn define_callable_state_seed(
        &mut self,
        handle: &RuntimeCallableStateSeedId,
        definition: RuntimeCallableStateSeed,
    ) -> Result<(), RuntimePlanBuildError> {
        self.ensure_usable()?;
        let state = self.resolve_callable_state_seed(handle)?;
        if self.callable_states.borrow().states[state.index()].is_some() {
            return Err(RuntimePlanBuildError::DuplicateCallableState { state });
        }
        let definition = definition.try_map(
            |semantic_identity| {
                self.types
                    .id_for_semantic(semantic_identity)
                    .ok_or(RuntimePlanBuildError::UnknownSemanticType { semantic_identity })
            },
            |handle| {
                let (site, ..) = handle
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
                Ok(site)
            },
            |handle| self.resolve_callable_state_seed(&handle),
        )?;
        self.callable_states.borrow_mut().states[state.index()] = Some(definition);
        Ok(())
    }

    pub(super) fn resolve_callable_state_seed(
        &self,
        handle: &RuntimeCallableStateSeedId,
    ) -> Result<RuntimeCallableStateId, RuntimePlanBuildError> {
        let state = handle
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignCallableStateSeed)?;
        if self
            .callable_states
            .borrow()
            .states
            .get(state.index())
            .is_none()
        {
            return Err(RuntimePlanBuildError::UnknownCallableState { state });
        }
        Ok(state)
    }

    /// Admits the ordinary closure expression's checked arrow and FunctionSite
    /// ABI as one program-owned initial state. This construction map is never
    /// retained by a value or used to resolve a runtime target.
    pub(super) fn intern_function_callable(
        &self,
        function_type: RuntimePlanTypeId,
        function: RuntimeFunctionSiteId,
        input_sources: &[RuntimeFunctionInputSource],
        input_types: &[RuntimePlanTypeId],
        result: RuntimePlanTypeId,
    ) -> Result<RuntimeCallableStateId, RuntimePlanBuildError> {
        let mut states = self.callable_states.borrow_mut();
        if let Some(state) = states.initial.get(&(function, function_type)) {
            return Ok(*state);
        }
        let state = states.reserve()?;
        let retained: Box<[_]> = input_sources
            .iter()
            .zip(input_types)
            .filter_map(|(source, ty)| match source {
                RuntimeFunctionInputSource::Capture { position } => {
                    Some(RuntimeCallableRetainedInput {
                        role: RuntimeCallableRetainedRole::Capture {
                            position: *position,
                        },
                        ty: *ty,
                    })
                }
                RuntimeFunctionInputSource::Parameter { .. } => None,
            })
            .collect();
        let parameters: Box<[_]> = input_sources
            .iter()
            .zip(input_types)
            .filter_map(|(source, ty)| match source {
                RuntimeFunctionInputSource::Parameter { position } => {
                    Some(RuntimeCallableParameterInput {
                        coordinate: RuntimeCallableParameterCoordinate {
                            group: 0,
                            parameter: *position,
                        },
                        kind: RuntimeCallableParameterKind::Fixed,
                        abi_ty: *ty,
                        binding_ty: *ty,
                    })
                }
                RuntimeFunctionInputSource::Capture { .. } => None,
            })
            .collect();
        let captures = (0..retained.len())
            .map(|position| RuntimeCallableInputSource::Retained {
                position: position as u32,
            })
            .collect();
        let arguments = (0..parameters.len())
            .map(|position| RuntimeCallableInputSource::Argument {
                position: position as u32,
            })
            .collect();
        states.states[state.index()] = Some(RuntimeCallableStateDefinition {
            function_type,
            origin: state,
            position: RuntimeCallablePosition::Unapplied,
            retained,
            parameters,
            result,
            attached: RuntimeCallableAttachedContract::None,
            transition: RuntimeCallableTransition::Invoke {
                function,
                captures,
                arguments,
            },
            partials: Box::new([]),
        });
        states.initial.insert((function, function_type), state);
        Ok(state)
    }
}
