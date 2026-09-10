//! Exact selected-call joins owned by the callable authority.
//!
//! A final semantic consumer may have HIR lookup evidence (for example a
//! typed receiver/method key), but it must not rebuild callable identity or
//! resolve a second catalog.  [`validate_selected_application`] is the sole
//! seam for joining one clean selected call with its prepared application and
//! the current callable authority.

use std::{collections::BTreeMap, sync::Arc};

use arcweft_lang_hir::{
    expr::HirCallArgumentOrdinal,
    symbol::{CallableDeclarationKey, CallableDeclarationOwner},
};
use thiserror::Error;

use crate::{
    effect_row::{EffectRow, EffectRowError, EffectSubstitution},
    final_analysis::{CheckedFunctionExecution, CheckedProjectNominal},
    types::{ArrayLength, TypeKind, constraints::ClosedTypeInstantiation},
};

use super::{
    CallableArgumentSlotIndex, CallableCandidateId, CallableFamily, CallableGroupIndex,
    CallableParameterConsumer, CallableParameterCoordinate, CallableParameterPassing,
    CallableParameterPresence, CallableResultSchema, CallableSignatureSchemaDigest,
    CheckedCallApplication, CheckedCallContinuationDigest, CheckedCallExecutionArgument,
    CheckedCallOperandDestination, CheckedCallResult, CheckedCallableCatalog,
    CheckedCallableDigest, CheckedCallableExecution, CheckedCallableFacts, CheckedCallableId,
    CheckedCallableLookupError, CheckedMethodLookup, ContentCallableIdentity,
    FrozenCallTypeSolution, ResolvedCallable, ResolvedCallableBaseInstantiation,
    ResolvedCallableOrigin, ResolvedCallableState,
};

/// Failure while joining one final call fact with the current callable
/// authority.  Every variant is typed evidence failure; no spelling or
/// source-identity fallback is available.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CheckedCallableJoinError {
    #[error(transparent)]
    GenericScope(#[from] crate::types::GenericScopeError),
    #[error("call target is not a clean selected callable")]
    NotSelected,
    #[error("selected call fact does not belong to the prepared application authority")]
    ApplicationAuthorityMismatch,
    #[error("selected callable group does not match the call fact")]
    SelectedGroupMismatch,
    #[error("selected callable has no current parameter group")]
    CurrentGroupMissing,
    #[error("selected call next group does not match the callable schema")]
    NextGroupMismatch,
    #[error("selected call has no typed result")]
    MissingResult,
    #[error("selected call result type does not match its current/full or partial group")]
    ResultMismatch,
    #[error("call argument ordinal is not source contiguous")]
    ArgumentOrdinalMismatch,
    #[error("call argument slot index is not contiguous")]
    ArgumentSlotMismatch,
    #[error("selected call argument is not mapped to the current group")]
    ArgumentGroupMismatch,
    #[error("selected call argument mapping points outside the schema")]
    ArgumentParameterMissing,
    #[error("selected call generic type observation conflicts")]
    GenericInstantiationMismatch,
    #[error("selected call effects do not match the current group")]
    EffectsMismatch,
    #[error("checked callable ID is missing for a catalog-backed selection")]
    MissingCheckedCallable,
    #[error("checked callable record is missing for a checked selection")]
    MissingCheckedRecord,
    #[error("selected checked callable record does not agree with the catalog row")]
    CatalogRecordMismatch,
    #[error("selected callable signature disagrees with the catalog row")]
    CatalogSignatureMismatch,
    #[error("catalog row effects disagree with the selected callable")]
    CatalogEffectsMismatch,
    #[error("checked callable lookup failed: {0:?}")]
    Catalog(CheckedCallableLookupError),
    #[error("a receiver/method key is required by the selected callable")]
    MissingReceiverKey,
    #[error("receiver/method evidence was supplied for a non-method callable")]
    UnexpectedReceiverKey,
    #[error("receiver type disagrees with the selected callable")]
    ReceiverTypeMismatch,
    #[error("receiver mode disagrees with the selected callable schema")]
    ReceiverModeMismatch,
    #[error("checked method lookup has no accepted candidate")]
    MethodLookupMissing,
    #[error("checked method lookup is ambiguous or inaccessible")]
    MethodLookupAmbiguous,
    #[error("checked method lookup selected a different ID")]
    MethodLookupMismatch,
    #[error("selected callable has no typed intrinsic authority")]
    MissingIntrinsicAuthority,
    #[error("selected callable family disagrees with its typed candidate")]
    IntrinsicFamilyMismatch,
    #[error("intrinsic callable does not own one exact fixed schema effect row")]
    IntrinsicEffectSchemaMismatch,
    #[error("selected callable instantiation transcript cannot be canonically encoded")]
    InstantiationTranscript,
}

/// Exact runtime input lineage of one selected project-function application.
///
/// A direct declaration call has no prior runtime value. A continuation call
/// must consume the lineage issued by the checked prefix application; runtime
/// lowering may not infer that lineage from the callee value's shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProjectContinuationRuntimeAbi {
    lineage: CheckedCallContinuationDigest,
    function_type: TypeKind,
    prefix_types: Box<[TypeKind]>,
}

impl CheckedProjectContinuationRuntimeAbi {
    pub const fn lineage(&self) -> CheckedCallContinuationDigest {
        self.lineage
    }

    pub const fn function_type(&self) -> &TypeKind {
        &self.function_type
    }

    pub const fn prefix_types(&self) -> &[TypeKind] {
        &self.prefix_types
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProjectFunctionParameterMaterialization {
    coordinate: CallableParameterCoordinate,
    passing: CallableParameterPassing,
    abi_type: TypeKind,
    binding_type: TypeKind,
    /// Indices into `CheckedCallApplicationCore::runtime_operands()` in exact
    /// source-evaluation order. These are never ABI destinations.
    operand_indices: Box<[u32]>,
}

impl CheckedProjectFunctionParameterMaterialization {
    pub const fn coordinate(&self) -> CallableParameterCoordinate {
        self.coordinate
    }

    pub const fn passing(&self) -> CallableParameterPassing {
        self.passing
    }

    pub const fn abi_type(&self) -> &TypeKind {
        &self.abi_type
    }

    pub const fn binding_type(&self) -> &TypeKind {
        &self.binding_type
    }

    pub const fn operand_indices(&self) -> &[u32] {
        &self.operand_indices
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedProjectFunctionRuntimeInput {
    Direct,
    Continuation {
        abi: CheckedProjectContinuationRuntimeAbi,
    },
}

/// Checked outcome owned by one project-function call site.
///
/// Non-terminal groups produce another typed continuation. Only a terminal
/// call with no deferred generic parameters may request an executable
/// callable instance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedProjectFunctionRuntimeOutcome {
    Continue {
        abi: CheckedProjectContinuationRuntimeAbi,
        next_group: CallableGroupIndex,
    },
    Invoke {
        result: TypeKind,
    },
}

/// Final-sema runtime selection for one ordinary project function call.
///
/// This is the sole bridge from checked callable/continuation authority to a
/// compiler-produced runtime callable instance. It deliberately retains the
/// frozen substitution rather than exposing a call-site reconstruction API.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProjectFunctionRuntimeSelection {
    declaration: CallableDeclarationKey,
    group: CallableGroupIndex,
    instantiation: CallableInstantiationDigest,
    base_instantiation: ResolvedCallableBaseInstantiation,
    solution: Arc<FrozenCallTypeSolution>,
    function_type: TypeKind,
    current_group_materialization: Box<[CheckedProjectFunctionParameterMaterialization]>,
    effects: crate::effects::EffectSet,
    input: CheckedProjectFunctionRuntimeInput,
    outcome: CheckedProjectFunctionRuntimeOutcome,
}

/// Closed ordinary project-function instance selected by a checked runtime
/// ingress that does not have a source call expression.
///
/// Entry roles are the first consumer. Their checked contracts prove one
/// complete parameter group, no generic inventory, no attached-content ABI,
/// and one closed effect row. This record lets the compiler issue the same
/// instance key and body projection as an ordinary terminal call without
/// fabricating a call solution or adopting types from the runtime ingress.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProjectFunctionRootRuntimeSelection {
    declaration: CallableDeclarationKey,
    group: CallableGroupIndex,
    solution: CheckedProjectFunctionInstanceSolution,
    function_type: TypeKind,
    effects: crate::effects::EffectSet,
}

/// Flat, sealed declaration environment for one closed project-function
/// instance. All right-hand sides have already been closed in the caller's
/// environment; body projection applies these callee keys simultaneously.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProjectFunctionInstanceSolution {
    solution: Arc<ClosedTypeInstantiation>,
    instantiation: CallableInstantiationDigest,
    function_type: TypeKind,
}

impl CheckedProjectFunctionInstanceSolution {
    pub const fn instantiation(&self) -> CallableInstantiationDigest {
        self.instantiation
    }

    /// Callable ABI closed in the caller's environment during selection.
    pub const fn function_type(&self) -> &TypeKind {
        &self.function_type
    }

    pub fn instantiate_array_length(
        &self,
        length: &ArrayLength,
    ) -> Result<ArrayLength, crate::types::TypeInstantiationError> {
        self.solution.instantiate_array_length(length)
    }

    pub fn instantiate_type(
        &self,
        ty: &TypeKind,
    ) -> Result<TypeKind, crate::types::TypeInstantiationError> {
        self.solution.instantiate_type(ty)
    }

    /// Projects a declaration type through this instance while admitting each
    /// structural occurrence to the caller's compilation work budget.
    pub fn instantiate_type_with_control<C: crate::types::TypeProjectionControl>(
        &self,
        ty: &TypeKind,
        control: &mut C,
    ) -> Result<TypeKind, crate::types::TypeProjectionError<C::Error>> {
        self.solution.instantiate_type_with_control(ty, control)
    }

    pub fn instantiate_array_length_with_control<C: crate::types::TypeProjectionControl>(
        &self,
        length: &ArrayLength,
        control: &mut C,
    ) -> Result<ArrayLength, crate::types::TypeProjectionError<C::Error>> {
        self.solution
            .instantiate_array_length_with_control(length, control)
    }

    pub fn instantiate_effect_row_with_control<C: crate::types::TypeProjectionControl>(
        &self,
        row: &EffectRow,
        control: &mut C,
    ) -> Result<crate::effects::EffectSet, crate::types::TypeProjectionError<C::Error>> {
        self.solution
            .project_effect_row_with_control(row, 1, control)
    }

    /// Closes one checked project nominal under this exact instance solution
    /// while retaining its declaration and HIR owner evidence. Downstream
    /// runtime projection must not reconstruct a nominal from an open
    /// declaration row and call-site arguments.
    pub fn instantiate_project_nominal(
        &self,
        nominal: &CheckedProjectNominal,
    ) -> Result<CheckedProjectNominal, crate::types::TypeInstantiationError> {
        self.instantiate_project_nominal_with_control(
            nominal,
            &mut crate::types::UnmeteredTypeProjection,
        )
        .map_err(crate::types::TypeProjectionError::into_instantiation)
    }

    pub fn instantiate_project_nominal_with_control<C: crate::types::TypeProjectionControl>(
        &self,
        nominal: &CheckedProjectNominal,
        control: &mut C,
    ) -> Result<CheckedProjectNominal, crate::types::TypeProjectionError<C::Error>> {
        let closed = self.instantiate_type_with_control(&nominal.ty(), control)?;
        let identity = closed.semantic_identity_digest_in_scope_with_control(
            &crate::types::GenericScope::default(),
            control,
        )?;
        let TypeKind::ProjectNominal(closed) = closed else {
            unreachable!("type specialization preserves the project nominal constructor");
        };
        Ok(CheckedProjectNominal::new(
            closed.declaration().clone(),
            nominal.owner(),
            identity,
            closed.arguments().to_vec(),
        ))
    }

    /// Closes one executable effect row through the same flat environment
    /// as the instance's value types. Closure/function-site emission
    /// must not infer an effect set from body operations.
    pub fn instantiate_effect_row(
        &self,
        row: &EffectRow,
    ) -> Result<crate::effects::EffectSet, EffectRowError> {
        self.solution.instantiate_effect_row(row)
    }
}

impl CheckedProjectFunctionRuntimeSelection {
    pub const fn declaration(&self) -> &CallableDeclarationKey {
        &self.declaration
    }

    pub const fn group(&self) -> CallableGroupIndex {
        self.group
    }

    pub const fn instantiation(&self) -> CallableInstantiationDigest {
        self.instantiation
    }

    /// Closes this terminal selection's caller-owned binding values once,
    /// producing the flat environment used by the selected declaration body.
    pub fn close_instance(
        &self,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
    ) -> Result<CheckedProjectFunctionInstanceSolution, CheckedProjectFunctionRuntimeSelectionError>
    {
        self.close_instance_with_control(enclosing, &mut crate::types::UnmeteredTypeProjection)
            .map_err(|error| match error {
                CheckedProjectFunctionInstanceProjectionError::Selection(error) => error,
                CheckedProjectFunctionInstanceProjectionError::Projection(error) => {
                    error.into_instantiation().into()
                }
            })
    }

    /// Closes the same invocation under a consumer-owned type projection budget.
    pub fn close_instance_with_control<C: crate::types::TypeProjectionControl>(
        &self,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        control: &mut C,
    ) -> Result<
        CheckedProjectFunctionInstanceSolution,
        CheckedProjectFunctionInstanceProjectionError<C::Error>,
    > {
        control
            .check()
            .map_err(crate::types::TypeProjectionError::Control)?;
        if !matches!(
            self.outcome,
            CheckedProjectFunctionRuntimeOutcome::Invoke { .. }
        ) {
            return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidResult.into());
        }
        let solution = self.solution.close_instantiation_with_control(
            enclosing.map(|row| row.solution.as_ref()),
            control,
        )?;
        let empty = ClosedTypeInstantiation::default();
        let caller = enclosing.map_or(&empty, |row| row.solution.as_ref());
        let function_type = caller.instantiate_type_with_control(&self.function_type, control)?;
        let instantiation = callable_instantiation_digest_from_bindings(
            &self.base_instantiation,
            solution.type_bindings(),
            solution.const_bindings(),
            solution
                .effect_bindings()
                .map(|(variable, value)| (*variable, value)),
            |ty, control| caller.instantiate_type_with_control(ty, control),
            control,
        )
        .map_err(|error| match error {
            CallableInstantiationDigestError::Projection(error) => {
                CheckedProjectFunctionInstanceProjectionError::Projection(error)
            }
            CallableInstantiationDigestError::TranscriptLength => {
                CheckedProjectFunctionRuntimeSelectionError::InstantiationTranscript.into()
            }
        })?;
        Ok(CheckedProjectFunctionInstanceSolution {
            solution: Arc::new(solution),
            instantiation,
            function_type,
        })
    }

    pub const fn solution(&self) -> &Arc<FrozenCallTypeSolution> {
        &self.solution
    }

    pub const fn function_type(&self) -> &TypeKind {
        &self.function_type
    }

    /// Materialized callee-binding ABI for the completed group. Rest
    /// parameters own one binding row regardless of
    /// how many authored execution slots supplied that value.
    pub const fn current_group_materialization(
        &self,
    ) -> &[CheckedProjectFunctionParameterMaterialization] {
        &self.current_group_materialization
    }

    pub const fn effects(&self) -> &crate::effects::EffectSet {
        &self.effects
    }

    pub const fn input(&self) -> &CheckedProjectFunctionRuntimeInput {
        &self.input
    }

    pub const fn outcome(&self) -> &CheckedProjectFunctionRuntimeOutcome {
        &self.outcome
    }
}

/// Runtime selection errors remain distinct from consumer-owned projection
/// aborts; neither loses its structured cause at the instance boundary.
#[derive(Debug, Error)]
pub enum CheckedProjectFunctionInstanceProjectionError<E: std::error::Error + 'static> {
    #[error(transparent)]
    Selection(#[from] CheckedProjectFunctionRuntimeSelectionError),
    #[error(transparent)]
    Projection(#[from] crate::types::TypeProjectionError<E>),
}

impl CheckedProjectFunctionRootRuntimeSelection {
    pub const fn declaration(&self) -> &CallableDeclarationKey {
        &self.declaration
    }

    pub const fn group(&self) -> CallableGroupIndex {
        self.group
    }

    pub const fn solution(&self) -> &CheckedProjectFunctionInstanceSolution {
        &self.solution
    }

    pub const fn function_type(&self) -> &TypeKind {
        &self.function_type
    }

    pub const fn effects(&self) -> &crate::effects::EffectSet {
        &self.effects
    }
}

/// Opaque typed cause of a failed callable-template projection.
/// The lower invariant vocabulary remains private to semantic analysis.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error(transparent)]
pub struct CheckedProjectFunctionProjectionFailure {
    source: Box<super::CallConstraintInvariant>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CheckedProjectFunctionRuntimeSelectionError {
    #[error(transparent)]
    Instantiation(#[from] crate::types::TypeInstantiationError),
    #[error("project-function runtime selection disagrees with the checked callable join")]
    JoinMismatch,
    #[error("project-function runtime selection has no executable body authority")]
    MissingRuntimeExecution,
    #[error("stream-factory project functions do not use an ordinary runtime function site")]
    StreamFactory,
    #[error("terminal project-function runtime selection retains deferred generic parameters")]
    OpenTerminalInstantiation,
    #[error("project-function runtime selection has an invalid callable type or result")]
    InvalidResult,
    #[error("checked project callable lookup failed: {0:?}")]
    Catalog(CheckedCallableLookupError),
    #[error("checked project callable projection failed: {0}")]
    CallableProjection(#[source] CheckedProjectFunctionProjectionFailure),
    #[error("checked project callable effect row is not a closed instantiation: {0}")]
    EffectRow(EffectRowError),
    #[error("checked project continuation has no exact runtime prefix ABI")]
    InvalidContinuationAbi,
    #[error("checked project-function instance transcript cannot be canonically closed")]
    InstantiationTranscript,
    #[error("project-function runtime root is not an ordinary Function declaration")]
    InvalidRootDeclaration,
    #[error("project-function runtime root must own exactly one complete parameter group")]
    InvalidRootGroup,
    #[error("project-function runtime root retains a generic type or const inventory")]
    OpenRootInstantiation,
    #[error("project-function runtime root cannot require an attached-content operand")]
    RootAttachedContent,
}

impl From<super::CallConstraintInvariant> for CheckedProjectFunctionRuntimeSelectionError {
    fn from(error: super::CallConstraintInvariant) -> Self {
        Self::CallableProjection(CheckedProjectFunctionProjectionFailure {
            source: Box::new(error),
        })
    }
}

/// Selects one closed ordinary Function instance for a checked non-call
/// runtime ingress.
///
/// This is deliberately narrower than call selection: an ingress cannot
/// manufacture continuation groups, infer generic substitutions, or supply
/// attached content. The checked Entry contract is expected to establish
/// these preconditions before requesting this projection.
pub fn select_project_function_root_runtime(
    declaration: &CallableDeclarationKey,
    catalog: &CheckedCallableCatalog,
) -> Result<CheckedProjectFunctionRootRuntimeSelection, CheckedProjectFunctionRuntimeSelectionError>
{
    if declaration.owner() != CallableDeclarationOwner::Function {
        return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidRootDeclaration);
    }
    let checked = catalog
        .project_callable(declaration)
        .map_err(CheckedProjectFunctionRuntimeSelectionError::Catalog)?;
    match checked.execution() {
        CheckedCallableExecution::Runtime(CheckedFunctionExecution::DirectFrame) => {}
        CheckedCallableExecution::Runtime(CheckedFunctionExecution::StreamFactory { .. }) => {
            return Err(CheckedProjectFunctionRuntimeSelectionError::StreamFactory);
        }
        CheckedCallableExecution::DispatchContract => {
            return Err(CheckedProjectFunctionRuntimeSelectionError::MissingRuntimeExecution);
        }
    }
    if !checked.signature().generic_inventory().types().is_empty()
        || !checked.signature().generic_inventory().consts().is_empty()
    {
        return Err(CheckedProjectFunctionRuntimeSelectionError::OpenRootInstantiation);
    }
    let [group] = checked.signature().groups() else {
        return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidRootGroup);
    };
    if checked.attached_content().is_some() {
        return Err(CheckedProjectFunctionRuntimeSelectionError::RootAttachedContent);
    }
    let effects = checked
        .exposed_row()
        .resolve(&EffectSubstitution::new())
        .map_err(CheckedProjectFunctionRuntimeSelectionError::EffectRow)?;
    let parameters = group
        .parameters()
        .iter()
        .map(|parameter| parameter.declared_type().cloned())
        .collect::<Option<Vec<_>>>()
        .ok_or(CheckedProjectFunctionRuntimeSelectionError::InvalidResult)?;
    let result = checked
        .signature()
        .value_type()
        .cloned()
        .ok_or(CheckedProjectFunctionRuntimeSelectionError::InvalidResult)?;
    let function_type =
        TypeKind::function_with_effects(parameters, result, checked.exposed_row().clone());
    let instantiation = empty_callable_instantiation_digest()
        .map_err(|_| CheckedProjectFunctionRuntimeSelectionError::InstantiationTranscript)?;
    Ok(CheckedProjectFunctionRootRuntimeSelection {
        declaration: declaration.clone(),
        group: group.index(),
        solution: CheckedProjectFunctionInstanceSolution {
            solution: Arc::new(ClosedTypeInstantiation::default()),
            instantiation,
            function_type: ClosedTypeInstantiation::default().instantiate_type(&function_type)?,
        },
        function_type,
        effects,
    })
}

/// Selects the runtime continuation/instance contract for one checked project
/// function application.
///
/// Project callables outside ordinary Function declarations (extern
/// capabilities, methods, Views, predicates, and proofs) return `Ok(None)`;
/// their existing typed runtime owners remain distinct. A Function selection
/// is either complete or a typed error—there is no non-generic or single-group
/// fallback.
pub fn select_project_function_runtime(
    application: &CheckedCallApplication,
    join: &CheckedCallableJoin,
    catalog: &CheckedCallableCatalog,
) -> Result<
    Option<CheckedProjectFunctionRuntimeSelection>,
    CheckedProjectFunctionRuntimeSelectionError,
> {
    let selected = application.core().candidates().selected();
    let ResolvedCallableOrigin::Project { declaration, .. } = selected.origin() else {
        return Ok(None);
    };
    if declaration.owner() != CallableDeclarationOwner::Function {
        return Ok(None);
    }
    if application.core().current_group() != join.current_group()
        || selected.id() != &CallableCandidateId::Project(declaration.clone())
        || join.checked_id() != selected.checked()
    {
        return Err(CheckedProjectFunctionRuntimeSelectionError::JoinMismatch);
    }
    let checked = catalog
        .project_callable(declaration)
        .map_err(CheckedProjectFunctionRuntimeSelectionError::Catalog)?;
    match checked.execution() {
        CheckedCallableExecution::Runtime(CheckedFunctionExecution::DirectFrame) => {}
        CheckedCallableExecution::Runtime(CheckedFunctionExecution::StreamFactory { .. }) => {
            return Err(CheckedProjectFunctionRuntimeSelectionError::StreamFactory);
        }
        CheckedCallableExecution::DispatchContract => {
            return Err(CheckedProjectFunctionRuntimeSelectionError::MissingRuntimeExecution);
        }
    }

    let solution = Arc::clone(application.core().solution());
    let (input, function_type) = match selected.state() {
        ResolvedCallableState::Base => {
            let ty = selected
                .base()
                .callable_type_with_terminal_effects(checked.exposed_row())
                .map_err(CheckedProjectFunctionRuntimeSelectionError::from)?;
            (
                CheckedProjectFunctionRuntimeInput::Direct,
                solution
                    .instantiate_result(&ty)
                    .map_err(CheckedProjectFunctionRuntimeSelectionError::from)?,
            )
        }
        ResolvedCallableState::Continuation(continuation) => (
            CheckedProjectFunctionRuntimeInput::Continuation {
                abi: checked_project_continuation_runtime_abi(
                    continuation.digest(),
                    continuation.function_type().clone(),
                    checked,
                    selected,
                    &solution,
                    join.current_group().get(),
                )?,
            },
            continuation.function_type().clone(),
        ),
    };
    if !matches!(function_type, TypeKind::Function { .. }) {
        return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidResult);
    }
    let outcome = match application.result() {
        CheckedCallResult::Continuation(continuation)
            if join.next_group() == Some(continuation.next_group()) =>
        {
            CheckedProjectFunctionRuntimeOutcome::Continue {
                abi: checked_project_continuation_runtime_abi(
                    continuation.digest(),
                    continuation.function_type().clone(),
                    checked,
                    selected,
                    &solution,
                    join.current_group().get().checked_add(1).ok_or(
                        CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi,
                    )?,
                )?,
                next_group: continuation.next_group(),
            }
        }
        CheckedCallResult::Value(result) if join.next_group().is_none() => {
            if !solution.is_fully_instantiated() {
                return Err(CheckedProjectFunctionRuntimeSelectionError::OpenTerminalInstantiation);
            }
            CheckedProjectFunctionRuntimeOutcome::Invoke {
                result: result.clone(),
            }
        }
        CheckedCallResult::ContentEmission(_)
        | CheckedCallResult::Continuation(_)
        | CheckedCallResult::Value(_) => {
            return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidResult);
        }
    };
    let effects = solution
        .instantiate_effect_row(join.schema_effects())
        .map_err(CheckedProjectFunctionRuntimeSelectionError::EffectRow)?;
    let current_group_materialization = checked_project_function_parameter_materialization(
        application,
        checked,
        &solution,
        join.current_group(),
    )?;
    Ok(Some(CheckedProjectFunctionRuntimeSelection {
        declaration: declaration.clone(),
        group: join.current_group(),
        instantiation: join.instantiation(),
        base_instantiation: selected.instantiation().clone(),
        solution,
        function_type,
        current_group_materialization,
        effects,
        input,
        outcome,
    }))
}

fn checked_project_continuation_runtime_abi(
    lineage: CheckedCallContinuationDigest,
    function_type: TypeKind,
    checked: &CheckedCallableFacts,
    selected: &ResolvedCallable,
    solution: &FrozenCallTypeSolution,
    completed_group_count: usize,
) -> Result<CheckedProjectContinuationRuntimeAbi, CheckedProjectFunctionRuntimeSelectionError> {
    if !matches!(function_type, TypeKind::Function { .. })
        || completed_group_count == 0
        || completed_group_count >= checked.signature().groups().len()
    {
        return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi);
    }
    let mut prefix_types = Vec::new();
    for group in checked
        .signature()
        .groups()
        .iter()
        .take(completed_group_count)
    {
        prefix_types.extend(checked_project_function_parameter_binding_types(
            selected, group, solution,
        )?);
    }
    Ok(CheckedProjectContinuationRuntimeAbi {
        lineage,
        function_type,
        prefix_types: prefix_types.into_boxed_slice(),
    })
}

fn checked_project_function_parameter_materialization(
    application: &CheckedCallApplication,
    checked: &CheckedCallableFacts,
    solution: &FrozenCallTypeSolution,
    group: CallableGroupIndex,
) -> Result<
    Box<[CheckedProjectFunctionParameterMaterialization]>,
    CheckedProjectFunctionRuntimeSelectionError,
> {
    let schema_group = checked
        .signature()
        .group(group)
        .ok_or(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi)?;
    let mut physical = BTreeMap::<CallableParameterCoordinate, Vec<u32>>::new();
    for (source_index, operand) in application
        .core()
        .runtime_operands()
        .into_vec()
        .into_iter()
        .enumerate()
    {
        let source_index = u32::try_from(source_index)
            .map_err(|_| CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi)?;
        let coordinate = match operand {
            super::CheckedCallRuntimeOperand::Receiver {
                mode:
                    super::CallableReceiverMode::Extension {
                        group, parameter, ..
                    },
                ..
            } => CallableParameterCoordinate::new(*group, *parameter),
            super::CheckedCallRuntimeOperand::Argument { slot, .. } => {
                let CheckedCallOperandDestination::Parameter(coordinate) = slot.destination()
                else {
                    return Err(
                        CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi,
                    );
                };
                *coordinate
            }
            super::CheckedCallRuntimeOperand::Receiver { .. }
            | super::CheckedCallRuntimeOperand::AttachedContent { .. } => continue,
        };
        if coordinate.group() != group {
            return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi);
        }
        physical.entry(coordinate).or_default().push(source_index);
    }
    let mut rows = Vec::with_capacity(schema_group.parameters().len());
    for parameter in schema_group.parameters() {
        if parameter.consumer() != &CallableParameterConsumer::Value {
            return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi);
        }
        let coordinate = CallableParameterCoordinate::new(group, parameter.index());
        let operand_indices = physical.remove(&coordinate).unwrap_or_default();
        if !operand_indices.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi);
        }
        if parameter.passing() == CallableParameterPassing::RestNamed {
            return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi);
        }
        let rest = parameter.passing() == CallableParameterPassing::RestPositional;
        match (rest, parameter.presence(), operand_indices.len()) {
            (true, CallableParameterPresence::Required, _) => {}
            (false, CallableParameterPresence::Required, 1) => {}
            (true, CallableParameterPresence::Defaulted, _)
            | (_, CallableParameterPresence::Optional, _)
            | (false, CallableParameterPresence::Required, _)
            | (false, CallableParameterPresence::Defaulted, _) => {
                return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi);
            }
        }
        let declared = application
            .core()
            .candidates()
            .selected()
            .base()
            .project_parameter_type(coordinate)?;
        let abi_type = solution.instantiate_template(&declared)?;
        let binding_type = if rest {
            TypeKind::Vec(Box::new(abi_type.clone()))
        } else {
            abi_type.clone()
        };
        rows.push(CheckedProjectFunctionParameterMaterialization {
            coordinate,
            passing: parameter.passing(),
            abi_type,
            binding_type,
            operand_indices: operand_indices.into_boxed_slice(),
        });
    }
    if !physical.is_empty() {
        return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi);
    }
    Ok(rows.into_boxed_slice())
}

fn checked_project_function_parameter_binding_types(
    selected: &ResolvedCallable,
    group: &super::CallableParameterGroup,
    solution: &FrozenCallTypeSolution,
) -> Result<Vec<TypeKind>, CheckedProjectFunctionRuntimeSelectionError> {
    group
        .parameters()
        .iter()
        .map(|parameter| {
            if parameter.consumer() != &CallableParameterConsumer::Value {
                return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi);
            }
            let declared =
                selected
                    .base()
                    .project_parameter_type(CallableParameterCoordinate::new(
                        group.index(),
                        parameter.index(),
                    ))?;
            let abi_type = solution.instantiate_template(&declared)?;
            match parameter.passing() {
                CallableParameterPassing::RestPositional => Ok(TypeKind::Vec(Box::new(abi_type))),
                CallableParameterPassing::RestNamed => {
                    Err(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi)
                }
                CallableParameterPassing::PositionalOnly
                | CallableParameterPassing::PositionalOrNamed
                | CallableParameterPassing::NamedOnly => Ok(abi_type),
            }
        })
        .collect()
}

impl CheckedCallableJoinError {
    pub(crate) fn visit_types<E>(
        &self,
        _visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::GenericScope(_)
            | Self::NotSelected
            | Self::ApplicationAuthorityMismatch
            | Self::SelectedGroupMismatch
            | Self::CurrentGroupMissing
            | Self::NextGroupMismatch
            | Self::MissingResult
            | Self::ResultMismatch
            | Self::ArgumentOrdinalMismatch
            | Self::ArgumentSlotMismatch
            | Self::ArgumentGroupMismatch
            | Self::ArgumentParameterMissing
            | Self::GenericInstantiationMismatch
            | Self::EffectsMismatch
            | Self::MissingCheckedCallable
            | Self::MissingCheckedRecord
            | Self::CatalogRecordMismatch
            | Self::CatalogSignatureMismatch
            | Self::CatalogEffectsMismatch
            | Self::Catalog(_)
            | Self::MissingReceiverKey
            | Self::UnexpectedReceiverKey
            | Self::ReceiverTypeMismatch
            | Self::ReceiverModeMismatch
            | Self::MethodLookupMissing
            | Self::MethodLookupAmbiguous
            | Self::MethodLookupMismatch
            | Self::MissingIntrinsicAuthority
            | Self::IntrinsicFamilyMismatch
            | Self::IntrinsicEffectSchemaMismatch
            | Self::InstantiationTranscript => Ok(()),
        }
    }
}

/// Closed intrinsic candidate family tag retained by a checked join.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IntrinsicCallableCandidateTag {
    FxConstructor,
    EnumVariant,
    Result,
    Option,
    Builtin,
    Agent,
    Presentation,
    Dialogue,
    Content,
    Environment,
    Local,
    FunctionValue,
    CollectionMethod,
    PresentationHandleMethod,
    IntegerMethod,
    DomainMethod,
    CapacityMethod,
    StageMethod,
    LineContextMethod,
    LineSchedule,
    Drop,
    Promotion,
}

impl IntrinsicCallableCandidateTag {
    pub const fn semantic_tag(self) -> u16 {
        match self {
            Self::FxConstructor => 0,
            Self::EnumVariant => 1,
            Self::Result => 2,
            Self::Option => 3,
            Self::Builtin => 4,
            Self::Agent => 5,
            Self::Presentation => 6,
            Self::Dialogue => 7,
            Self::Content => 22,
            Self::Environment => 8,
            Self::Local => 9,
            Self::FunctionValue => 10,
            Self::CollectionMethod => 12,
            Self::PresentationHandleMethod => 13,
            Self::IntegerMethod => 14,
            Self::DomainMethod => 15,
            Self::CapacityMethod => 16,
            Self::StageMethod => 17,
            Self::LineContextMethod => 18,
            Self::LineSchedule => 19,
            Self::Drop => 20,
            Self::Promotion => 21,
        }
    }

    fn from_candidate(candidate: &CallableCandidateId) -> Option<Self> {
        Some(match candidate {
            CallableCandidateId::FxConstructor(_) => Self::FxConstructor,
            CallableCandidateId::EnumVariant(_) => Self::EnumVariant,
            CallableCandidateId::Result(_) => Self::Result,
            CallableCandidateId::Option(_) => Self::Option,
            CallableCandidateId::Builtin(_) => Self::Builtin,
            CallableCandidateId::Agent(_) => Self::Agent,
            CallableCandidateId::Presentation(_) => Self::Presentation,
            CallableCandidateId::Dialogue(_) => Self::Dialogue,
            CallableCandidateId::Content(_) => Self::Content,
            CallableCandidateId::Environment(_) => Self::Environment,
            CallableCandidateId::Local(_) => Self::Local,
            CallableCandidateId::FunctionValue(_) => Self::FunctionValue,
            CallableCandidateId::CollectionMethod(_) => Self::CollectionMethod,
            CallableCandidateId::PresentationHandleMethod(_) => Self::PresentationHandleMethod,
            CallableCandidateId::IntegerMethod(_) => Self::IntegerMethod,
            CallableCandidateId::DomainMethod(_) => Self::DomainMethod,
            CallableCandidateId::CapacityMethod(_) => Self::CapacityMethod,
            CallableCandidateId::StageMethod(_) => Self::StageMethod,
            CallableCandidateId::LineContextMethod(_) => Self::LineContextMethod,
            CallableCandidateId::LineSchedule(_) => Self::LineSchedule,
            CallableCandidateId::Drop(_) => Self::Drop,
            CallableCandidateId::Promotion(_) => Self::Promotion,
            CallableCandidateId::Project(_)
            | CallableCandidateId::Detached(_)
            | CallableCandidateId::Standard(_) => return None,
        })
    }
}

/// Stable digest of the selected callable's typed instantiation.
///
/// This commits both the selected base instantiation (receiver/extension
/// family) and the resolver-owned frozen type/const/effect solution. It is
/// independent of the call-site coordinate and may therefore key one runtime
/// callable instance shared by equal checked applications.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableInstantiationDigest([u8; 32]);

impl CallableInstantiationDigest {
    pub const fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable semantic digest of one fully checked callable-owner join.
///
/// The bytes can only be produced by [`CheckedCallableJoin::semantic_digest`];
/// consumers may borrow them for a parent transcript but cannot mint a second
/// callable authority from raw bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedCallableJoinDigest([u8; 32]);

impl CheckedCallableJoinDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Semantic receiver mode proven by the selected callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableReceiverMode {
    None,
    Value {
        receiver: TypeKind,
    },
    Type {
        receiver: TypeKind,
    },
    Extension {
        receiver: TypeKind,
        group: CallableGroupIndex,
        parameter: super::CallableParameterIndex,
    },
}

impl CallableReceiverMode {
    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::None => Ok(()),
            Self::Value { receiver }
            | Self::Type { receiver }
            | Self::Extension { receiver, .. } => visitor(receiver),
        }
    }
}

/// One source-order argument slot after exact schema validation.
///
/// Only argument/slot ordinals, accepted coordinates, and typed semantic
/// digests are retained.  The originating `ExprId` is deliberately absent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallableArgumentSlot {
    slot: CallableArgumentSlotIndex,
    mapped: Option<CallableParameterCoordinate>,
    inferred: Option<[u8; 32]>,
    expected: Option<[u8; 32]>,
}

impl CheckedCallableArgumentSlot {
    pub const fn slot(&self) -> CallableArgumentSlotIndex {
        self.slot
    }

    pub const fn mapped(&self) -> Option<CallableParameterCoordinate> {
        self.mapped
    }

    pub const fn inferred(&self) -> Option<[u8; 32]> {
        self.inferred
    }

    pub const fn expected(&self) -> Option<[u8; 32]> {
        self.expected
    }
}

/// One source-order argument after exact schema validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallableArgument {
    argument: HirCallArgumentOrdinal,
    slots: Box<[CheckedCallableArgumentSlot]>,
}

impl CheckedCallableArgument {
    pub const fn argument(&self) -> HirCallArgumentOrdinal {
        self.argument
    }

    pub fn slots(&self) -> &[CheckedCallableArgumentSlot] {
        &self.slots
    }
}

/// Complete typed result of the callable-owner join.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCallableJoin {
    Catalog {
        id: Box<CheckedCallableId>,
        digest: CheckedCallableDigest,
        signature: CallableSignatureSchemaDigest,
        catalog_effects: EffectRow,
        effects: EffectRow,
        result: CallableResultSchema,
        current_group: CallableGroupIndex,
        next_group: Option<CallableGroupIndex>,
        arguments: Box<[CheckedCallableArgument]>,
        receiver: CallableReceiverMode,
        instantiation: CallableInstantiationDigest,
    },
    Intrinsic {
        candidate: IntrinsicCallableCandidateTag,
        family: CallableFamily,
        signature: CallableSignatureSchemaDigest,
        schema_effects: EffectRow,
        effects: EffectRow,
        result: CallableResultSchema,
        current_group: CallableGroupIndex,
        next_group: Option<CallableGroupIndex>,
        arguments: Box<[CheckedCallableArgument]>,
        receiver: CallableReceiverMode,
        instantiation: CallableInstantiationDigest,
    },
}

impl CheckedCallableJoin {
    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Catalog {
                result, receiver, ..
            }
            | Self::Intrinsic {
                result, receiver, ..
            } => {
                result.visit_types(visitor)?;
                receiver.visit_types(visitor)
            }
        }
    }

    pub const fn checked_id(&self) -> Option<&CheckedCallableId> {
        match self {
            Self::Catalog { id, .. } => Some(id),
            Self::Intrinsic { .. } => None,
        }
    }

    pub const fn digest(&self) -> Option<CheckedCallableDigest> {
        match self {
            Self::Catalog { digest, .. } => Some(*digest),
            Self::Intrinsic { .. } => None,
        }
    }

    pub const fn signature(&self) -> CallableSignatureSchemaDigest {
        match self {
            Self::Catalog { signature, .. } | Self::Intrinsic { signature, .. } => *signature,
        }
    }

    pub const fn instantiation(&self) -> CallableInstantiationDigest {
        match self {
            Self::Catalog { instantiation, .. } | Self::Intrinsic { instantiation, .. } => {
                *instantiation
            }
        }
    }

    pub const fn current_group(&self) -> CallableGroupIndex {
        match self {
            Self::Catalog { current_group, .. } | Self::Intrinsic { current_group, .. } => {
                *current_group
            }
        }
    }

    pub const fn next_group(&self) -> Option<CallableGroupIndex> {
        match self {
            Self::Catalog { next_group, .. } | Self::Intrinsic { next_group, .. } => *next_group,
        }
    }

    pub const fn result(&self) -> &CallableResultSchema {
        match self {
            Self::Catalog { result, .. } | Self::Intrinsic { result, .. } => result,
        }
    }

    pub const fn effects(&self) -> &EffectRow {
        match self {
            Self::Catalog { effects, .. } | Self::Intrinsic { effects, .. } => effects,
        }
    }

    /// Callable-schema effect row whose concrete closed application is the
    /// runtime function/callback ABI. Catalog-backed calls retain this row
    /// separately from the application execution-fold row because evaluated
    /// effect roles suppress ordinary runtime-call execution.
    pub const fn schema_effects(&self) -> &EffectRow {
        match self {
            Self::Catalog {
                catalog_effects, ..
            } => catalog_effects,
            Self::Intrinsic { schema_effects, .. } => schema_effects,
        }
    }

    pub fn arguments(&self) -> &[CheckedCallableArgument] {
        match self {
            Self::Catalog { arguments, .. } | Self::Intrinsic { arguments, .. } => arguments,
        }
    }

    pub const fn receiver(&self) -> &CallableReceiverMode {
        match self {
            Self::Catalog { receiver, .. } | Self::Intrinsic { receiver, .. } => receiver,
        }
    }

    /// Stable semantic transcript for the fully checked join.
    pub fn semantic_digest(
        &self,
    ) -> Result<CheckedCallableJoinDigest, crate::types::GenericScopeError> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.lang.checked-callable-authority-join.v1\0");
        match self {
            Self::Catalog {
                id,
                digest,
                signature,
                catalog_effects,
                effects,
                result,
                current_group,
                next_group,
                arguments,
                receiver,
                instantiation,
            } => {
                hasher.update(&[0]);
                hasher.update(id.semantic_digest().as_bytes());
                hasher.update(digest.as_bytes());
                hasher.update(signature.as_bytes());
                write_effect(&mut hasher, catalog_effects);
                write_effect(&mut hasher, effects);
                write_result_schema(&mut hasher, result)?;
                write_group(&mut hasher, *current_group);
                write_optional_group(&mut hasher, *next_group);
                write_arguments(&mut hasher, arguments);
                write_receiver(&mut hasher, receiver)?;
                hasher.update(instantiation.bytes());
            }
            Self::Intrinsic {
                candidate,
                family,
                signature,
                schema_effects,
                effects,
                result,
                current_group,
                next_group,
                arguments,
                receiver,
                instantiation,
            } => {
                hasher.update(&[1]);
                hasher.update(&candidate.semantic_tag().to_le_bytes());
                hasher.update(&[callable_family_tag(*family)]);
                hasher.update(signature.as_bytes());
                write_effect(&mut hasher, schema_effects);
                write_effect(&mut hasher, effects);
                write_result_schema(&mut hasher, result)?;
                write_group(&mut hasher, *current_group);
                write_optional_group(&mut hasher, *next_group);
                write_arguments(&mut hasher, arguments);
                write_receiver(&mut hasher, receiver)?;
                hasher.update(instantiation.bytes());
            }
        }
        Ok(CheckedCallableJoinDigest(*hasher.finalize().as_bytes()))
    }
}

/// The one callable-owner validation seam for final semantic consumers.
///
/// Every execution, result, group, effect, receiver, and instantiation row is
/// projected from the already sealed application.  The join performs only the
/// remaining checked-catalog identity lookup; it never observes HIR receiver
/// spelling or reruns lower substitution.
pub(crate) fn validate_selected_application(
    application: &CheckedCallApplication,
    catalog: &CheckedCallableCatalog,
) -> Result<CheckedCallableJoin, CheckedCallableJoinError> {
    let core = application.core();
    let selected = core.candidates().selected();
    let current_group = core.current_group();
    let next_group = match application.result() {
        CheckedCallResult::Value(_) => None,
        CheckedCallResult::ContentEmission(_) => None,
        CheckedCallResult::Continuation(continuation) => Some(continuation.next_group()),
    };
    let arguments = checked_join_arguments(selected, current_group, core.execution().arguments())?;
    let receiver = checked_receiver_mode(selected)?;
    let signature = selected.schema().semantic_digest();
    let instantiation = callable_instantiation_digest(selected.instantiation(), core.solution())
        .map_err(|_| CheckedCallableJoinError::InstantiationTranscript)?;
    let result = match application.result() {
        CheckedCallResult::Value(value) => CallableResultSchema::Value(value.clone()),
        CheckedCallResult::ContentEmission(operation) => {
            CallableResultSchema::ContentEmission(*operation)
        }
        CheckedCallResult::Continuation(continuation) => {
            CallableResultSchema::Value(continuation.function_type().clone())
        }
    };
    let effects = core.effects().clone();

    match selected.checked() {
        Some(id) => {
            let row = catalog
                .callable(id)
                .map_err(CheckedCallableJoinError::Catalog)?;
            if row.id() != id
                || row.signature().semantic_digest() != signature
                || row.record().id() != selected.id()
            {
                return Err(CheckedCallableJoinError::CatalogRecordMismatch);
            }
            if let Some(schema_key) = row.record().receiver_method_key() {
                let key = match selected.instantiation() {
                    ResolvedCallableBaseInstantiation::Extension { receiver, .. } => {
                        super::ReceiverMethodKey::new(
                            receiver.clone(),
                            row.record()
                                .extension_method_name()
                                .ok_or(CheckedCallableJoinError::MethodLookupMismatch)?
                                .clone(),
                        )
                    }
                    _ => schema_key,
                };
                match catalog.method(&key) {
                    CheckedMethodLookup::Candidates(candidates)
                        if candidates.iter().any(|candidate| candidate == id) => {}
                    CheckedMethodLookup::Candidates(_) => {
                        return Err(CheckedCallableJoinError::MethodLookupMismatch);
                    }
                    CheckedMethodLookup::Absent => {
                        return Err(CheckedCallableJoinError::MethodLookupMissing);
                    }
                    CheckedMethodLookup::Inaccessible(_) => {
                        return Err(CheckedCallableJoinError::MethodLookupAmbiguous);
                    }
                }
            }
            let catalog_effects = row.exposed_row().clone();
            Ok(CheckedCallableJoin::Catalog {
                id: Box::new(id.clone()),
                digest: id.semantic_digest(),
                signature,
                catalog_effects,
                effects,
                result,
                current_group,
                next_group,
                arguments,
                receiver,
                instantiation,
            })
        }
        None => {
            let candidate = IntrinsicCallableCandidateTag::from_candidate(selected.id())
                .ok_or(CheckedCallableJoinError::MissingIntrinsicAuthority)?;
            if selected.family() != selected.id().intrinsic_family() {
                return Err(CheckedCallableJoinError::IntrinsicFamilyMismatch);
            }
            let schema_effects = selected
                .schema()
                .effects()
                .fixed_row()
                .cloned()
                .ok_or(CheckedCallableJoinError::IntrinsicEffectSchemaMismatch)?;
            Ok(CheckedCallableJoin::Intrinsic {
                candidate,
                family: selected.family(),
                signature,
                schema_effects,
                effects,
                result,
                current_group,
                next_group,
                arguments,
                receiver,
                instantiation,
            })
        }
    }
}

fn checked_join_arguments(
    selected: &ResolvedCallable,
    current_group: CallableGroupIndex,
    execution: &[CheckedCallExecutionArgument],
) -> Result<Box<[CheckedCallableArgument]>, CheckedCallableJoinError> {
    let mut arguments = Vec::with_capacity(execution.len());
    for (argument_index, argument) in execution.iter().enumerate() {
        let expected = HirCallArgumentOrdinal::try_from_usize(argument_index)
            .map_err(|_| CheckedCallableJoinError::ArgumentOrdinalMismatch)?;
        if argument.argument() != expected {
            return Err(CheckedCallableJoinError::ArgumentOrdinalMismatch);
        }
        let mut slots = Vec::with_capacity(argument.slots().len());
        for (slot_index, slot) in argument.slots().iter().enumerate() {
            let expected_slot = CallableArgumentSlotIndex::try_from_usize(slot_index)
                .map_err(|_| CheckedCallableJoinError::ArgumentSlotMismatch)?;
            if slot.slot() != expected_slot {
                return Err(CheckedCallableJoinError::ArgumentSlotMismatch);
            }
            let mapped = match slot.destination() {
                CheckedCallOperandDestination::Parameter(coordinate) => {
                    if coordinate.group() != current_group {
                        return Err(CheckedCallableJoinError::ArgumentGroupMismatch);
                    }
                    if selected
                        .schema()
                        .group(coordinate.group())
                        .and_then(|group| group.parameters().get(coordinate.parameter().get()))
                        .is_none()
                    {
                        return Err(CheckedCallableJoinError::ArgumentParameterMissing);
                    }
                    Some(*coordinate)
                }
                CheckedCallOperandDestination::Open(_) => None,
            };
            slots.push(CheckedCallableArgumentSlot {
                slot: slot.slot(),
                mapped,
                inferred: Some(*slot.inferred().semantic_identity_digest()?.as_bytes()),
                expected: slot
                    .expected()
                    .map(TypeKind::semantic_identity_digest)
                    .transpose()?
                    .map(|identity| *identity.as_bytes()),
            });
        }
        arguments.push(CheckedCallableArgument {
            argument: argument.argument(),
            slots: slots.into_boxed_slice(),
        });
    }
    Ok(arguments.into_boxed_slice())
}

fn checked_receiver_mode(
    selected: &ResolvedCallable,
) -> Result<CallableReceiverMode, CheckedCallableJoinError> {
    Ok(match selected.instantiation() {
        ResolvedCallableBaseInstantiation::None
        | ResolvedCallableBaseInstantiation::EnumConstructor
        | ResolvedCallableBaseInstantiation::Result { .. }
        | ResolvedCallableBaseInstantiation::Option
        | ResolvedCallableBaseInstantiation::Character { .. } => CallableReceiverMode::None,
        ResolvedCallableBaseInstantiation::Receiver { receiver } => CallableReceiverMode::Value {
            receiver: receiver.clone(),
        },
        ResolvedCallableBaseInstantiation::TypeReceiver { receiver } => {
            CallableReceiverMode::Type {
                receiver: receiver.receiver().clone(),
            }
        }
        ResolvedCallableBaseInstantiation::Extension {
            receiver,
            group,
            parameter,
        } => CallableReceiverMode::Extension {
            receiver: receiver.clone(),
            group: *group,
            parameter: *parameter,
        },
    })
}

#[derive(Debug, Error)]
enum CallableInstantiationDigestError<E: std::error::Error + 'static = std::convert::Infallible> {
    #[error("callable instantiation transcript length exceeds u64")]
    TranscriptLength,
    #[error(transparent)]
    Projection(#[from] crate::types::TypeProjectionError<E>),
}

impl<E: std::error::Error + 'static> From<crate::types::GenericScopeError>
    for CallableInstantiationDigestError<E>
{
    fn from(error: crate::types::GenericScopeError) -> Self {
        Self::Projection(error.into())
    }
}

fn callable_instantiation_digest(
    instantiation: &ResolvedCallableBaseInstantiation,
    solution: &FrozenCallTypeSolution,
) -> Result<CallableInstantiationDigest, CallableInstantiationDigestError> {
    callable_instantiation_digest_from_bindings(
        instantiation,
        solution.type_bindings(),
        solution.const_bindings(),
        solution
            .effect_bindings()
            .iter()
            .map(|row| (row.variable(), row.value())),
        |ty, _| Ok(ty.clone()),
        &mut crate::types::UnmeteredTypeProjection,
    )
}

fn empty_callable_instantiation_digest()
-> Result<CallableInstantiationDigest, CallableInstantiationDigestError> {
    let solution = ClosedTypeInstantiation::default();
    callable_instantiation_digest_from_bindings(
        &ResolvedCallableBaseInstantiation::None,
        solution.type_bindings(),
        solution.const_bindings(),
        solution
            .effect_bindings()
            .map(|(variable, value)| (*variable, value)),
        |ty, control| solution.instantiate_type_with_control(ty, control),
        &mut crate::types::UnmeteredTypeProjection,
    )
}

fn callable_instantiation_digest_from_bindings<'a, C: crate::types::TypeProjectionControl>(
    instantiation: &ResolvedCallableBaseInstantiation,
    types: impl ExactSizeIterator<
        Item = (
            crate::types::ScopedTypeReferenceView<'a>,
            crate::types::ScopedTypeView<'a>,
        ),
    >,
    consts: impl ExactSizeIterator<
        Item = (
            crate::types::ScopedConstReferenceView<'a>,
            crate::types::ScopedArrayLengthView<'a>,
        ),
    >,
    effects: impl ExactSizeIterator<Item = (crate::effect_row::EffectVar, &'a EffectRow)>,
    project_base: impl Fn(
        &TypeKind,
        &mut C,
    ) -> Result<TypeKind, crate::types::TypeProjectionError<C::Error>>,
    control: &mut C,
) -> Result<CallableInstantiationDigest, CallableInstantiationDigestError<C::Error>> {
    control
        .check()
        .map_err(crate::types::TypeProjectionError::Control)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.lang.callable-instantiation.v1\0");
    match instantiation {
        ResolvedCallableBaseInstantiation::None => {
            hasher.update(&[0]);
        }
        ResolvedCallableBaseInstantiation::EnumConstructor => {
            hasher.update(&[1]);
        }
        ResolvedCallableBaseInstantiation::Result { kind } => {
            hasher.update(&[
                2,
                u8::from(matches!(kind, super::ResultConstructorKind::Err)),
            ]);
        }
        ResolvedCallableBaseInstantiation::Option => {
            hasher.update(&[3]);
        }
        ResolvedCallableBaseInstantiation::Character { owner } => {
            hasher.update(&[4]);
            write_checked_bytes(&mut hasher, owner.character().canonical_identity_bytes())?;
        }
        ResolvedCallableBaseInstantiation::Receiver { receiver } => {
            hasher.update(&[5]);
            let ty = project_base(receiver, control)?;
            hasher.update(
                ty.semantic_identity_digest_in_scope_with_control(
                    &crate::types::GenericScope::default(),
                    control,
                )?
                .as_bytes(),
            );
        }
        ResolvedCallableBaseInstantiation::TypeReceiver { receiver } => {
            hasher.update(&[6]);
            let ty = project_base(receiver.receiver(), control)?;
            hasher.update(
                ty.semantic_identity_digest_in_scope_with_control(
                    &crate::types::GenericScope::default(),
                    control,
                )?
                .as_bytes(),
            );
        }
        ResolvedCallableBaseInstantiation::Extension {
            receiver,
            group,
            parameter,
        } => {
            hasher.update(&[7]);
            let ty = project_base(receiver, control)?;
            hasher.update(
                ty.semantic_identity_digest_in_scope_with_control(
                    &crate::types::GenericScope::default(),
                    control,
                )?
                .as_bytes(),
            );
            write_group(&mut hasher, *group);
            hasher.update(
                &u64::try_from(parameter.get())
                    .map_err(|_| CallableInstantiationDigestError::TranscriptLength)?
                    .to_le_bytes(),
            );
        }
    }
    write_checked_len(&mut hasher, types.len())?;
    for (parameter, value) in types {
        control
            .visit_binding()
            .map_err(crate::types::TypeProjectionError::Control)?;
        hasher.update(
            parameter
                .semantic_identity_digest_with_control(control)?
                .as_bytes(),
        );
        hasher.update(
            value
                .semantic_identity_digest_with_control(control)?
                .as_bytes(),
        );
    }
    write_checked_len(&mut hasher, consts.len())?;
    for (parameter, value) in consts {
        control
            .check()
            .map_err(crate::types::TypeProjectionError::Control)?;
        control
            .visit_binding()
            .map_err(crate::types::TypeProjectionError::Control)?;
        let parameter = parameter.canonical_checked_bytes_with_control(control)?;
        let value = value.canonical_checked_bytes_with_control(control)?;
        write_checked_bytes(&mut hasher, &parameter)?;
        write_checked_bytes(&mut hasher, &value)?;
    }
    write_checked_len(&mut hasher, effects.len())?;
    for (variable, value) in effects {
        control
            .check()
            .map_err(crate::types::TypeProjectionError::Control)?;
        control
            .visit_binding()
            .map_err(crate::types::TypeProjectionError::Control)?;
        hasher.update(variable.issuer().as_bytes());
        hasher.update(&variable.index().to_le_bytes());
        hasher.update(
            value
                .semantic_identity_digest_with_control(control)?
                .as_bytes(),
        );
    }
    Ok(CallableInstantiationDigest(*hasher.finalize().as_bytes()))
}
fn write_checked_len<E: std::error::Error + 'static>(
    hasher: &mut blake3::Hasher,
    length: usize,
) -> Result<(), CallableInstantiationDigestError<E>> {
    hasher.update(
        &u64::try_from(length)
            .map_err(|_| CallableInstantiationDigestError::TranscriptLength)?
            .to_le_bytes(),
    );
    Ok(())
}

fn write_checked_bytes<E: std::error::Error + 'static>(
    hasher: &mut blake3::Hasher,
    bytes: &[u8],
) -> Result<(), CallableInstantiationDigestError<E>> {
    write_checked_len(hasher, bytes.len())?;
    hasher.update(bytes);
    Ok(())
}

fn write_type(
    hasher: &mut blake3::Hasher,
    ty: &TypeKind,
) -> Result<(), crate::types::GenericScopeError> {
    hasher.update(ty.semantic_identity_digest()?.as_bytes());
    Ok(())
}

fn write_result_schema(
    hasher: &mut blake3::Hasher,
    result: &CallableResultSchema,
) -> Result<(), crate::types::GenericScopeError> {
    match result {
        CallableResultSchema::Value(value) => {
            hasher.update(&[0]);
            write_type(hasher, value)?;
        }
        CallableResultSchema::ContentEmission(operation) => {
            hasher.update(&[1, operation.semantic_tag()]);
            match operation {
                ContentCallableIdentity::Language { definition, schema } => {
                    hasher.update(&[0]);
                    hasher.update(&[content_definition_tag(*definition)]);
                    hasher.update(schema.as_bytes());
                }
                ContentCallableIdentity::TextProxyObject { owner, definition } => {
                    hasher.update(&[1]);
                    hasher.update(owner.as_bytes());
                    hasher.update(definition.as_bytes());
                }
            }
        }
    }
    Ok(())
}

fn content_definition_tag(
    definition: arcweft_presentation::rich_text::PresentationContentCallableDefinitionId,
) -> u8 {
    use arcweft_presentation::rich_text::PresentationContentCallableDefinitionId as Id;
    match definition {
        Id::Strong => 0,
        Id::Em => 1,
        Id::Color => 2,
        Id::Font => 3,
        Id::Size => 4,
        Id::Style(_) => 5,
        Id::Layout(_) => 6,
        Id::Transform(_) => 7,
        Id::Fx => 8,
        Id::Ruby => 9,
        Id::Raw => 10,
    }
}

fn write_group(hasher: &mut blake3::Hasher, group: CallableGroupIndex) {
    hasher.update(&u32::try_from(group.get()).unwrap_or(u32::MAX).to_le_bytes());
}

fn write_optional_group(hasher: &mut blake3::Hasher, group: Option<CallableGroupIndex>) {
    match group {
        Some(group) => {
            hasher.update(&[1]);
            write_group(hasher, group);
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

fn write_effect(hasher: &mut blake3::Hasher, effect: &EffectRow) {
    hasher.update(effect.semantic_identity_digest().as_bytes());
}

fn write_arguments(hasher: &mut blake3::Hasher, arguments: &[CheckedCallableArgument]) {
    hasher.update(
        &u32::try_from(arguments.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for argument in arguments {
        hasher.update(&u32::from(argument.argument().get()).to_le_bytes());
        hasher.update(
            &u32::try_from(argument.slots().len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        for slot in argument.slots() {
            hasher.update(
                &u32::try_from(slot.slot().get())
                    .unwrap_or(u32::MAX)
                    .to_le_bytes(),
            );
            match slot.mapped() {
                Some(coordinate) => {
                    hasher.update(&[1]);
                    write_group(hasher, coordinate.group());
                    hasher.update(
                        &u32::try_from(coordinate.parameter().get())
                            .unwrap_or(u32::MAX)
                            .to_le_bytes(),
                    );
                }
                None => {
                    hasher.update(&[0]);
                }
            }
            write_optional_digest(hasher, slot.inferred());
            write_optional_digest(hasher, slot.expected());
        }
    }
}

fn write_optional_digest(hasher: &mut blake3::Hasher, digest: Option<[u8; 32]>) {
    match digest {
        Some(digest) => {
            hasher.update(&[1]);
            hasher.update(&digest);
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

fn write_receiver(
    hasher: &mut blake3::Hasher,
    receiver: &CallableReceiverMode,
) -> Result<(), crate::types::GenericScopeError> {
    match receiver {
        CallableReceiverMode::None => {
            hasher.update(&[0]);
        }
        CallableReceiverMode::Value { receiver } => {
            hasher.update(&[1]);
            write_type(hasher, receiver)?;
        }
        CallableReceiverMode::Type { receiver } => {
            hasher.update(&[2]);
            write_type(hasher, receiver)?;
        }
        CallableReceiverMode::Extension {
            receiver,
            group,
            parameter,
        } => {
            hasher.update(&[3]);
            write_type(hasher, receiver)?;
            write_group(hasher, *group);
            hasher.update(
                &u32::try_from(parameter.get())
                    .unwrap_or(u32::MAX)
                    .to_le_bytes(),
            );
        }
    }
    Ok(())
}

fn callable_family_tag(family: CallableFamily) -> u8 {
    match family {
        CallableFamily::FxConstructor => 0,
        CallableFamily::EnumConstructor => 1,
        CallableFamily::ResultConstructor => 2,
        CallableFamily::OptionConstructor => 3,
        CallableFamily::Builtin => 4,
        CallableFamily::Agent => 5,
        CallableFamily::Presentation => 6,
        CallableFamily::Dialogue => 7,
        CallableFamily::Project => 8,
        CallableFamily::Environment => 9,
        CallableFamily::Lexical => 10,
        CallableFamily::FunctionValue => 11,
        CallableFamily::CollectionMethod => 12,
        CallableFamily::PresentationHandleMethod => 13,
        CallableFamily::IntegerMethod => 14,
        CallableFamily::DomainMethod => 15,
        CallableFamily::TraitMethod => 16,
        CallableFamily::CapacityMethod => 17,
        CallableFamily::StageMethod => 18,
        CallableFamily::LineContextMethod => 19,
        CallableFamily::LineSchedule => 20,
        CallableFamily::Drop => 21,
        CallableFamily::Promotion => 22,
        CallableFamily::Content => 23,
    }
}

#[cfg(test)]
mod tests;
