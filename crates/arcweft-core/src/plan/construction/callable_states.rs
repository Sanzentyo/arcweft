//! Issuer-bound construction of the program's reusable callable state graph.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use crate::entry::schema::value_budget::ValidationWork;
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PartialApplication {
    owner: Option<RuntimePlanTypeId>,
    source: RuntimePlanTypeId,
    result: RuntimePlanTypeId,
    arguments: usize,
}

/// Construction owns interning and the finite set of checked partial-arrow
/// demands. The final program contains only the admitted state table.
#[derive(Debug, Default)]
pub(super) struct RuntimeCallableStateBuilder {
    // A lowering step may reserve more rows while borrowing one immutable
    // definition. Snapshots share that actual payload, not a cloned graph.
    pub(super) states: Vec<Option<Arc<RuntimeCallableState>>>,
    initial: BTreeMap<(RuntimeFunctionSiteId, RuntimePlanTypeId), RuntimeCallableStateId>,
    partials: BTreeSet<PartialApplication>,
    source_complete: bool,
}

impl RuntimeCallableStateBuilder {
    /// Call admission reads the final graph after pending partial arrows exist.
    pub(super) fn validate_declared_call(
        &self,
        call: &crate::plan::RuntimeProjectCallPlan,
    ) -> Result<(), RuntimePlanBuildError> {
        let base = self
            .states
            .get(call.state().index())
            .and_then(Option::as_ref)
            .ok_or(RuntimePlanBuildError::UnknownCallableState {
                state: call.state(),
            })?;
        let parameters = call
            .ordinary()
            .iter()
            .map(|row| row.parameter())
            .collect::<Vec<_>>();
        if self.states.iter().flatten().any(|candidate| {
            candidate.origin == base.origin
                && candidate.active_group() == Some(call.completed_group())
                && candidate.function_type == call.callee().ty()
                && candidate.admits_declared_parameters(&parameters)
        }) {
            Ok(())
        } else {
            Err(RuntimePlanBuildError::InvalidProjectCallAbi {
                context: "sealed declared callee family and remaining coordinates",
            })
        }
    }
    fn reserve(&mut self) -> Result<RuntimeCallableStateId, RuntimePlanBuildError> {
        if self.source_complete {
            return Err(RuntimePlanBuildError::CallableSourceGraphClosed);
        }
        let state = RuntimeCallableStateId::for_index(self.states.len())
            .ok_or(RuntimeCallableStateError::IdentityExhausted)?;
        self.states.push(None);
        Ok(state)
    }

    pub(super) fn request_partial(
        &mut self,
        owner: Option<RuntimePlanTypeId>,
        source: RuntimePlanTypeId,
        result: RuntimePlanTypeId,
        arguments: usize,
    ) -> Result<(), RuntimePlanBuildError> {
        if self.source_complete {
            for (index, row) in self.states.iter().enumerate() {
                let row = row
                    .as_ref()
                    .expect("source completion requires every original row");
                if row.function_type != source
                    || arguments == 0
                    || arguments >= row.parameters.len()
                {
                    continue;
                }
                if row
                    .positional_partial(arguments)
                    .and_then(|partial| self.states.get(partial.state.index()))
                    .and_then(Option::as_ref)
                    .is_none_or(|target| target.function_type != result)
                {
                    return Err(RuntimePlanBuildError::MissingSourceCallablePartial {
                        state: RuntimeCallableStateId::for_index(index)
                            .expect("reserved source state"),
                        result,
                        arguments,
                    });
                }
            }
        }
        self.partials.insert(PartialApplication {
            owner,
            source,
            result,
            arguments,
        });
        Ok(())
    }

    pub(in crate::plan::construction) fn source_storage_bytes(rows: usize) -> Option<usize> {
        rows.checked_mul(
            size_of::<Option<Arc<RuntimeCallableState>>>()
                + size_of::<RuntimeCallableStateSeedId>()
                + size_of::<RuntimeCallableStateSeed>()
                + 2 * size_of::<RuntimeCallableState>()
                + 2 * size_of::<usize>(),
        )
    }

    pub(in crate::plan::construction) fn complete_source(
        &mut self,
    ) -> Result<(), RuntimePlanBuildError> {
        let count = self.states.iter().filter(|row| row.is_none()).count();
        if count != 0 {
            return Err(RuntimePlanBuildError::IncompleteCallableStates { count });
        }
        self.source_complete = true;
        Ok(())
    }

    /// Makes all checked partial requests concrete before publication. No body
    /// target is discovered here: every state refers to a pre-reserved site.
    pub(super) fn seal(
        &mut self,
        builder: &mut RuntimePlanBuilder,
        executable_rows: &mut u32,
        maximum_rows: u32,
    ) -> Result<(), RuntimePlanBuildError> {
        if self.partials.is_empty() {
            return Ok(());
        }
        let demands = self.partials.iter().copied().collect::<Vec<_>>();
        let mut work = ValidationWork::new(crate::entry::RuntimeSchemaLimits::engine_default());
        let mut index = 0;
        while index < self.states.len() {
            let Some(source) = self.states[index].clone() else {
                index += 1;
                continue;
            };
            let mut partials = Vec::new();
            let mut sealed = BTreeMap::new();
            for &demand in &demands {
                let count = demand.arguments;
                work.charge(0)
                    .map_err(RuntimePlanBuildError::CallableOperandType)?;
                let arrow_arity = source.parameters.len()
                    + usize::from(!matches!(
                        source.attached,
                        RuntimeCallableAttachedContract::None
                    ));
                if count == 0
                    || count >= arrow_arity
                    || count > source.parameters.len()
                    || !builder.partial_arrow_accepts(
                        demand.owner,
                        demand.source,
                        source.function_type,
                        &work,
                    )?
                {
                    continue;
                }
                let declared = source
                    .positional_partial(count)
                    .map(|partial| {
                        self.states
                            .get(partial.state.index())
                            .and_then(Option::as_ref)
                            .map(|target| target.function_type)
                            .ok_or(RuntimePlanBuildError::UnknownCallableState {
                                state: partial.state,
                            })
                    })
                    .transpose()?;
                if let Some(target_type) = declared.or_else(|| sealed.get(&count).copied()) {
                    if !builder.partial_arrow_accepts(
                        demand.owner,
                        demand.result,
                        target_type,
                        &work,
                    )? {
                        return Err(RuntimePlanBuildError::InvalidProjectCallAbi {
                            context: "partial application target arrow",
                        });
                    }
                    continue;
                }
                if self.source_complete {
                    return Err(RuntimePlanBuildError::MissingSourceCallablePartial {
                        state: RuntimeCallableStateId::for_index(index)
                            .expect("reserved callable state"),
                        result: demand.result,
                        arguments: count,
                    });
                }
                let function_type = if source.function_type == demand.source {
                    demand.result
                } else {
                    builder.intern_partial_arrow(
                        source.function_type,
                        count,
                        executable_rows,
                        maximum_rows,
                        &mut work,
                    )?
                };
                if !builder.partial_arrow_accepts(
                    demand.owner,
                    demand.result,
                    function_type,
                    &work,
                )? {
                    return Err(RuntimePlanBuildError::InvalidProjectCallAbi {
                        context: "partial application target arrow",
                    });
                }
                sealed.insert(count, function_type);
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
                let definition = RuntimeCallableStateDefinition {
                    function_type,
                    origin: source.origin,
                    position,
                    retained: retained.into_boxed_slice(),
                    parameters: source.parameters[count..].into(),
                    result: source.result,
                    attached,
                    transition,
                    partials: Box::new([]),
                };
                let target = self.states.iter().enumerate().find_map(|(index, state)| {
                    state
                        .as_ref()
                        .filter(|state| state.same_application_layout(&definition))
                        .and_then(|_| RuntimeCallableStateId::for_index(index))
                });
                let target = if let Some(target) = target {
                    target
                } else {
                    let target = self.reserve()?;
                    self.states[target.index()] = Some(Arc::new(definition));
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
            drop(source);
            let current = self.states[index]
                .as_mut()
                .expect("source state was present");
            let current = Arc::get_mut(current)
                .expect("lowering snapshots end before final graph completion");
            current.partials = std::mem::take(&mut current.partials)
                .into_vec()
                .into_iter()
                .chain(partials)
                .collect();
            index += 1;
        }
        Ok(())
    }

    pub(super) fn finish(self) -> RuntimeCallableStateTable {
        RuntimeCallableStateTable::from_admitted(
            self.states
                .into_iter()
                .map(|state| {
                    Arc::try_unwrap(state.expect("all states were defined before publication"))
                        .expect("construction snapshots never escape into the final inventory")
                })
                .collect(),
        )
    }
}

impl RuntimePlanBuilder {
    fn partial_arrow_accepts(
        &self,
        owner: Option<RuntimePlanTypeId>,
        expected: RuntimePlanTypeId,
        actual: RuntimePlanTypeId,
        work: &ValidationWork,
    ) -> Result<bool, RuntimePlanBuildError> {
        if expected == actual {
            return Ok(true);
        }
        let authority = crate::plan::PlanValueAuthority::Building {
            types: &self.types,
            records: &self.nominal_record_domains,
            variants: &self.variant_domains,
        };
        crate::program_types::callable_operand_accepts(&authority, owner, expected, actual, work)
            .map_err(RuntimePlanBuildError::CallableOperandType)
    }

    fn intern_partial_arrow(
        &mut self,
        source: RuntimePlanTypeId,
        arguments: usize,
        executable_rows: &mut u32,
        maximum_rows: u32,
        work: &mut ValidationWork,
    ) -> Result<RuntimePlanTypeId, RuntimePlanBuildError> {
        use crate::plan::{RuntimePlanTypeProjection::Function, RuntimePlanTypeSeed};
        let invalid = || RuntimePlanBuildError::InvalidProjectCallAbi {
            context: "partial application source arrow",
        };
        let source = self.types.get(source).ok_or_else(invalid)?;
        let Function {
            contract,
            parameters,
            result,
        } = source.projection()
        else {
            return Err(invalid());
        };
        let remaining = parameters.get(arguments..).ok_or_else(invalid)?;
        let types = u64::try_from(self.types.len())
            .map_err(|_| RuntimePlanBuildError::ExecutableInventoryArithmeticOverflow)?;
        work.charge_atoms(types, 0)
            .map_err(RuntimePlanBuildError::CallableOperandType)?;
        if let Some(existing) =
            self.types
                .find_function_projection(source.scope(), contract, remaining, *result)
        {
            return Ok(existing);
        }
        let actual = executable_rows
            .checked_add(1)
            .ok_or(RuntimePlanBuildError::ExecutableInventoryArithmeticOverflow)?;
        if actual > maximum_rows {
            return Err(RuntimePlanBuildError::ExecutableInventoryRowsLimit {
                actual,
                maximum: maximum_rows,
            });
        }
        let semantic = |id| {
            self.types
                .get(id)
                .map(crate::plan::RuntimePlanTypeDeclaration::semantic_identity)
                .ok_or_else(invalid)
        };
        let remaining = remaining
            .iter()
            .copied()
            .map(semantic)
            .collect::<Result<Box<[_]>, _>>()?;
        let result = semantic(*result)?;
        let mut identity = blake3::Hasher::new();
        identity.update(b"arcweft.runtime-partial-arrow.v1\0");
        identity.update(source.semantic_identity().as_bytes());
        identity.update(
            &u32::try_from(arguments)
                .map_err(|_| invalid())?
                .to_le_bytes(),
        );
        let seed = RuntimePlanTypeSeed::new(
            RuntimeSemanticTypeId::from_bytes(*identity.finalize().as_bytes()),
            Function {
                contract: contract.clone(),
                parameters: remaining,
                result,
            },
        )
        .with_scope(source.scope().clone());
        let batch = self.types.prepare_batch([seed])?;
        let admitted = self.types.commit_batch(batch);
        let [admitted] = admitted.as_ref() else {
            return Err(invalid());
        };
        *executable_rows = actual;
        Ok(*admitted)
    }

    /// Builds an authoring state from this builder's actual function ABI.
    /// Source replay instead supplies its pre-reserved original state handle.
    pub fn function_callable_state_seed(
        &self,
        function_type: RuntimeSemanticTypeId,
        function: &RuntimeFunctionSiteSeedId,
    ) -> Result<RuntimeCallableStateSeedId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let function_type = self.types.id_for_semantic(function_type).ok_or(
            RuntimePlanBuildError::UnknownSemanticType {
                semantic_identity: function_type,
            },
        )?;
        let state = self.function_callable_state(function_type, function)?;
        Ok(RuntimeCallableStateSeedId {
            issuer: Arc::clone(&self.issuer),
            state,
        })
    }

    pub(super) fn function_callable_state(
        &self,
        function_type: RuntimePlanTypeId,
        function: &RuntimeFunctionSiteSeedId,
    ) -> Result<RuntimeCallableStateId, RuntimePlanBuildError> {
        let (function, sources, types, result, ..) = function
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
        match self.projection(function_type)? {
            crate::plan::RuntimePlanTypeProjection::Function {
                parameters,
                result: expected,
                ..
            } if parameters
                .iter()
                .copied()
                .eq(sources.iter().zip(types).filter_map(|(source, ty)| {
                    matches!(source, RuntimeFunctionInputSource::Parameter { .. }).then_some(*ty)
                }))
                && *expected == result => {}
            _ => {
                return Err(RuntimePlanBuildError::InvalidTypeProjection {
                    context: "function expression",
                    ty: function_type,
                });
            }
        }
        self.intern_function_callable(function_type, function, sources, types, result)
    }

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
        if self.callable_states.borrow().source_complete {
            return Err(RuntimePlanBuildError::CallableSourceGraphClosed);
        }
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
        self.callable_states.borrow_mut().states[state.index()] = Some(Arc::new(definition));
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
        if states.source_complete {
            return Err(RuntimePlanBuildError::CallableSourceGraphClosed);
        }
        if let Some(state) = states.initial.get(&(function, function_type)) {
            return Ok(*state);
        }
        let state = states.reserve()?;
        let retained: Box<[_]> = input_sources
            .iter()
            .zip(input_types)
            .filter_map(|(source, ty)| match source {
                RuntimeFunctionInputSource::Capture { position }
                | RuntimeFunctionInputSource::CapturedParameter { position, .. } => {
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
                RuntimeFunctionInputSource::Parameter { position, .. } => {
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
                RuntimeFunctionInputSource::Capture { .. }
                | RuntimeFunctionInputSource::CapturedParameter { .. } => None,
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
        states.states[state.index()] = Some(Arc::new(RuntimeCallableStateDefinition {
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
        }));
        states.initial.insert((function, function_type), state);
        Ok(state)
    }
}
