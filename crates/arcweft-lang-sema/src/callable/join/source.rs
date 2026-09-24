//! Program-callable source authority for declaration values and saved groups.

use std::sync::Arc;

use super::specialization::CheckedProjectFunctionSpecialization;
use super::{
    CallableGroupIndex, CallableInstantiationDigestError, CallableParameterConsumer,
    CallableParameterCoordinate, CallableParameterPassing, CallableParameterPresence,
    CheckedCallContinuationDigest, CheckedCallableCatalog, CheckedCallableExecution,
    CheckedCallableFacts, CheckedFunctionExecution, CheckedProjectFunctionInstanceProjectionError,
    CheckedProjectFunctionInstanceSolution, CheckedProjectFunctionRootRuntimeSelection,
    CheckedProjectFunctionRuntimeOutcome, CheckedProjectFunctionRuntimeSelection,
    CheckedProjectFunctionRuntimeSelectionError, ClosedTypeInstantiation, EffectSubstitution,
    FrozenCallTypeSolution, ResolvedCallableBaseInstantiation,
    callable_instantiation_digest_from_bindings,
};
use crate::{
    callable::{CheckedCallableAttachedContentParameter, CheckedFunctionSpecialization},
    types::{
        GenericDeclarationBinder, ScopedType, ScopedTypeView, TypeKind, TypeProjectionControl,
        TypeProjectionError,
    },
};
use arcweft_lang_hir::symbol::{CallableDeclarationKey, CallableDeclarationOwner};

mod identity;
pub use identity::CheckedProjectFunctionCallableSourceDigest;

/// The declaration and application position are held by the surrounding source.
/// A root is a value producer; it has no fabricated call or continuation digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedProjectFunctionCallableOrigin {
    Root,
    Continuation {
        lineage: CheckedCallContinuationDigest,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProjectFunctionCallableParameter {
    coordinate: CallableParameterCoordinate,
    passing: CallableParameterPassing,
    presence: CallableParameterPresence,
    abi_type: ScopedType,
    binding_type: ScopedType,
}

impl CheckedProjectFunctionCallableParameter {
    pub const fn coordinate(&self) -> CallableParameterCoordinate {
        self.coordinate
    }
    pub const fn passing(&self) -> CallableParameterPassing {
        self.passing
    }
    pub const fn presence(&self) -> CallableParameterPresence {
        self.presence
    }
    pub fn abi_type(&self) -> ScopedTypeView<'_> {
        self.abi_type.view()
    }
    pub fn binding_type(&self) -> ScopedTypeView<'_> {
        self.binding_type.view()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProjectFunctionCallableRetainedParameter {
    coordinate: CallableParameterCoordinate,
    binding_type: ScopedType,
}

impl CheckedProjectFunctionCallableRetainedParameter {
    pub const fn coordinate(&self) -> CallableParameterCoordinate {
        self.coordinate
    }
    pub fn binding_type(&self) -> ScopedTypeView<'_> {
        self.binding_type.view()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProjectFunctionCallableAttached {
    parameter: CheckedCallableAttachedContentParameter,
    abi_type: ScopedType,
    binding_type: ScopedType,
}

impl CheckedProjectFunctionCallableAttached {
    /// Original presence/default/role and source binding authority.
    pub const fn parameter(&self) -> &CheckedCallableAttachedContentParameter {
        &self.parameter
    }
    pub fn abi_type(&self) -> ScopedTypeView<'_> {
        self.abi_type.view()
    }
    pub fn binding_type(&self) -> ScopedTypeView<'_> {
        self.binding_type.view()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SourceProjection {
    Declaration(GenericDeclarationBinder),
    Continuation(Arc<FrozenCallTypeSolution>),
}

/// A checked source's function scheme, formal layout and producing environment.
/// The source environment is sealed here; a later witness has its own context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProjectFunctionCallableSource {
    digest: CheckedProjectFunctionCallableSourceDigest,
    declaration: CallableDeclarationKey,
    checked: CheckedCallableFacts,
    origin: CheckedProjectFunctionCallableOrigin,
    group: CallableGroupIndex,
    function_type: TypeKind,
    parameters: Box<[CheckedProjectFunctionCallableParameter]>,
    retained_parameters: Box<[CheckedProjectFunctionCallableRetainedParameter]>,
    attached: Option<CheckedProjectFunctionCallableAttached>,
    projection: SourceProjection,
    base_instantiation: ResolvedCallableBaseInstantiation,
    source_enclosing: Option<CheckedProjectFunctionInstanceSolution>,
    closed_selection: Option<CheckedProjectFunctionRootRuntimeSelection>,
}

impl CheckedProjectFunctionCallableSource {
    pub const fn source_digest(&self) -> CheckedProjectFunctionCallableSourceDigest {
        self.digest
    }
    pub const fn declaration(&self) -> &CallableDeclarationKey {
        &self.declaration
    }
    pub const fn origin(&self) -> CheckedProjectFunctionCallableOrigin {
        self.origin
    }
    pub const fn group(&self) -> CallableGroupIndex {
        self.group
    }
    pub const fn function_type(&self) -> &TypeKind {
        &self.function_type
    }
    pub const fn parameters(&self) -> &[CheckedProjectFunctionCallableParameter] {
        &self.parameters
    }
    pub const fn retained_parameters(&self) -> &[CheckedProjectFunctionCallableRetainedParameter] {
        &self.retained_parameters
    }
    pub const fn attached_source_schema(&self) -> Option<&CheckedProjectFunctionCallableAttached> {
        self.attached.as_ref()
    }
    pub const fn closed_selection(&self) -> Option<&CheckedProjectFunctionRootRuntimeSelection> {
        self.closed_selection.as_ref()
    }

    fn caller(&self) -> Option<&ClosedTypeInstantiation> {
        self.source_enclosing
            .as_ref()
            .map(|row| row.solution.as_ref())
    }

    fn project_template<C: TypeProjectionControl>(
        &self,
        ty: &TypeKind,
        control: &mut C,
    ) -> Result<ScopedType, TypeProjectionError<C::Error>> {
        let projected = match &self.projection {
            SourceProjection::Declaration(binder) => binder.project_with_control(ty, control)?,
            SourceProjection::Continuation(solution) => {
                solution.project_template_with_control(ty, control)?
            }
        };
        self.caller()
            .unwrap_or(&ClosedTypeInstantiation::default())
            .instantiate_scoped_type_with_control(projected.view(), control)
    }

    fn finish<C: TypeProjectionControl>(
        mut self,
        control: &mut C,
    ) -> Result<Self, CheckedProjectFunctionInstanceProjectionError<C::Error>> {
        let signature = self.checked.signature();
        let current = signature
            .group(self.group)
            .ok_or(CheckedProjectFunctionRuntimeSelectionError::InvalidRootGroup)?;
        self.parameters = current
            .parameters()
            .iter()
            .map(|parameter| {
                if parameter.consumer() != &CallableParameterConsumer::Value {
                    return Err(
                        CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi.into(),
                    );
                }
                let declared = parameter
                    .declared_type()
                    .ok_or(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi)?;
                let binding = parameter
                    .passing()
                    .value_binding_type(declared.clone())
                    .ok_or(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi)?;
                Ok(CheckedProjectFunctionCallableParameter {
                    coordinate: CallableParameterCoordinate::new(self.group, parameter.index()),
                    passing: parameter.passing(),
                    presence: parameter.presence(),
                    abi_type: self.project_template(declared, control)?,
                    binding_type: self.project_template(&binding, control)?,
                })
            })
            .collect::<Result<_, CheckedProjectFunctionInstanceProjectionError<C::Error>>>()?;
        let mut retained = Vec::new();
        for group in signature.groups().iter().take(self.group.get()) {
            for parameter in group.parameters() {
                if parameter.consumer() != &CallableParameterConsumer::Value {
                    return Err(
                        CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi.into(),
                    );
                }
                let declared = parameter
                    .declared_type()
                    .ok_or(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi)?;
                let binding = parameter
                    .passing()
                    .value_binding_type(declared.clone())
                    .ok_or(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi)?;
                let projected = self.project_template(&binding, control)?;
                // A retained ABI root may contain local schemes, but no free
                // reference to the callable's incoming residual binder.
                let ty = ClosedTypeInstantiation::default()
                    .instantiate_type_with_control(projected.view().value(), control)?;
                retained.push(CheckedProjectFunctionCallableRetainedParameter {
                    coordinate: CallableParameterCoordinate::new(group.index(), parameter.index()),
                    binding_type: ScopedType::at_root(ty),
                });
            }
        }
        self.retained_parameters = retained.into_boxed_slice();
        self.attached = self
            .checked
            .attached_content()
            .filter(|parameter| parameter.group() == self.group)
            .map(|parameter| {
                Ok::<_, TypeProjectionError<C::Error>>(CheckedProjectFunctionCallableAttached {
                    parameter: parameter.clone(),
                    abi_type: self.project_template(parameter.abi_type(), control)?,
                    binding_type: self.project_template(parameter.binding_type(), control)?,
                })
            })
            .transpose()?;
        let body = match &self.projection {
            SourceProjection::Declaration(binder) if binder.scope().binders().is_empty() => Some(
                ClosedTypeInstantiation::default().for_declaration_with_control(binder, control)?,
            ),
            SourceProjection::Continuation(solution) if solution.is_fully_instantiated() => {
                Some(solution.close_instantiation_with_control(self.caller(), control)?)
            }
            SourceProjection::Declaration(_) | SourceProjection::Continuation(_) => None,
        };
        self.closed_selection = body
            .map(|body| self.close_body(body, control))
            .transpose()?;
        self.digest = self.compute_digest(control)?;
        Ok(self)
    }

    pub fn specialize_callable_value_with_control<C: TypeProjectionControl>(
        &self,
        catalog: &CheckedCallableCatalog,
        witness: &CheckedFunctionSpecialization,
        witness_enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        control: &mut C,
    ) -> Result<
        CheckedProjectFunctionSpecialization,
        CheckedProjectFunctionInstanceProjectionError<C::Error>,
    > {
        self.validate_catalog(catalog)?;
        let empty = ClosedTypeInstantiation::default();
        let caller = witness_enclosing.map_or(&empty, |row| row.solution.as_ref());
        let expected_source =
            caller.instantiate_type_with_control(witness.source_type(), control)?;
        if !super::specialization::same_type(&self.function_type, &expected_source, control)? {
            return Err(
                CheckedProjectFunctionRuntimeSelectionError::SpecializationSourceMismatch.into(),
            );
        }
        let arguments = witness.close_arguments_with_control(Some(caller), control)?;
        let body = match &self.projection {
            SourceProjection::Declaration(binder) => {
                arguments.for_declaration_with_control(binder, control)?
            }
            SourceProjection::Continuation(solution) => {
                solution.close_residual_with_control(&arguments, self.caller(), control)?
            }
        };
        let proof = CheckedProjectFunctionSpecialization::seal(
            self,
            arguments,
            self.close_body(body, control)?,
            control,
        )?;
        let expected = caller.instantiate_type_with_control(witness.specialized_type(), control)?;
        if !super::specialization::same_type(proof.specialized_type(), &expected, control)? {
            return Err(
                CheckedProjectFunctionRuntimeSelectionError::SpecializationResultMismatch.into(),
            );
        }
        Ok(proof)
    }

    pub(super) fn specialize_completed_call<C: TypeProjectionControl>(
        &self,
        body: ClosedTypeInstantiation,
        control: &mut C,
    ) -> Result<
        CheckedProjectFunctionSpecialization,
        CheckedProjectFunctionInstanceProjectionError<C::Error>,
    > {
        let arguments = match &self.projection {
            SourceProjection::Declaration(binder) => {
                body.declaration_arguments_with_control(binder, control)?
            }
            SourceProjection::Continuation(solution) => {
                solution.residual_arguments_with_control(&body, self.caller(), control)?
            }
        };
        CheckedProjectFunctionSpecialization::seal(
            self,
            arguments,
            self.close_body(body, control)?,
            control,
        )
    }

    fn validate_catalog(
        &self,
        catalog: &CheckedCallableCatalog,
    ) -> Result<(), CheckedProjectFunctionRuntimeSelectionError> {
        let checked = catalog
            .project_callable(&self.declaration)
            .map_err(CheckedProjectFunctionRuntimeSelectionError::Catalog)?;
        if checked != &self.checked {
            return Err(CheckedProjectFunctionRuntimeSelectionError::JoinMismatch);
        }
        Ok(())
    }

    fn close_body<C: TypeProjectionControl>(
        &self,
        body: ClosedTypeInstantiation,
        control: &mut C,
    ) -> Result<
        CheckedProjectFunctionRootRuntimeSelection,
        CheckedProjectFunctionInstanceProjectionError<C::Error>,
    > {
        let terminal = self
            .checked
            .signature()
            .groups()
            .last()
            .ok_or(CheckedProjectFunctionRuntimeSelectionError::InvalidRootGroup)?
            .index();
        let callable_type = body.instantiate_type_with_control(
            &declared_function_type(&self.checked, CallableGroupIndex::ZERO)?,
            control,
        )?;
        let function_type = super::specialization::group_type(&callable_type, terminal)?.clone();
        let TypeKind::Function { effects, .. } = &function_type else {
            return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidResult.into());
        };
        let effects = effects
            .resolve(&EffectSubstitution::new())
            .map_err(CheckedProjectFunctionRuntimeSelectionError::EffectRow)?;
        let empty = ClosedTypeInstantiation::default();
        let caller = self.caller().unwrap_or(&empty);
        let instantiation = callable_instantiation_digest_from_bindings(
            &self.base_instantiation,
            body.type_bindings(),
            body.const_bindings(),
            body.effect_bindings(),
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
        Ok(CheckedProjectFunctionRootRuntimeSelection {
            declaration: self.declaration.clone(),
            group: terminal,
            function_type: function_type.clone(),
            effects,
            solution: CheckedProjectFunctionInstanceSolution {
                solution: Arc::new(body),
                instantiation,
                function_type,
                callable_type,
            },
        })
    }
}

pub fn select_project_function_value_runtime<C: TypeProjectionControl>(
    declaration: &CallableDeclarationKey,
    catalog: &CheckedCallableCatalog,
    enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
    control: &mut C,
) -> Result<
    CheckedProjectFunctionCallableSource,
    CheckedProjectFunctionInstanceProjectionError<C::Error>,
> {
    let checked = checked_function(declaration, catalog)?;
    let binder = checked
        .signature()
        .function_value_binder()
        .map_err(TypeProjectionError::from)?;
    let declared = declared_function_type(checked, CallableGroupIndex::ZERO)?;
    let projected = binder.project_with_control(&declared, control)?;
    let empty = ClosedTypeInstantiation::default();
    let caller = enclosing.map_or(&empty, |row| row.solution.as_ref());
    let function_type = caller
        .instantiate_scoped_type_with_control(projected.view(), control)?
        .view()
        .to_quantified_type_with_control(control)?;
    CheckedProjectFunctionCallableSource {
        digest: CheckedProjectFunctionCallableSourceDigest::UNSEALED,
        declaration: declaration.clone(),
        checked: checked.clone(),
        origin: CheckedProjectFunctionCallableOrigin::Root,
        group: CallableGroupIndex::ZERO,
        function_type,
        parameters: Box::new([]),
        retained_parameters: Box::new([]),
        attached: None,
        projection: SourceProjection::Declaration(binder),
        base_instantiation: ResolvedCallableBaseInstantiation::None,
        source_enclosing: root_enclosing(checked, enclosing),
        closed_selection: None,
    }
    .finish(control)
}

impl CheckedProjectFunctionRuntimeSelection {
    /// Exact arrow for this checked group application. The current arguments
    /// have been determined, while a returned continuation keeps its own binder.
    /// This differs from the source scheme before this application is checked.
    pub fn application_function_type_with_control<C: TypeProjectionControl>(
        &self,
        catalog: &CheckedCallableCatalog,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        control: &mut C,
    ) -> Result<TypeKind, CheckedProjectFunctionInstanceProjectionError<C::Error>> {
        let checked = checked_function(&self.declaration, catalog)?;
        let group = checked
            .signature()
            .group(self.group)
            .ok_or(CheckedProjectFunctionRuntimeSelectionError::InvalidRootGroup)?;
        if group.parameters().len() != self.current_group_materialization.len() {
            return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi.into());
        }
        let result = match &self.outcome {
            CheckedProjectFunctionRuntimeOutcome::Continue { abi, .. } => abi.function_type(),
            CheckedProjectFunctionRuntimeOutcome::Invoke { result } => result,
        };
        let arrow = TypeKind::function_with_effects(
            self.current_group_materialization
                .iter()
                .map(|parameter| parameter.abi_type().clone()),
            result.clone(),
            crate::effect_row::EffectRow::closed(self.effects.clone()),
        );
        let empty = ClosedTypeInstantiation::default();
        Ok(enclosing
            .map_or(&empty, |row| row.solution.as_ref())
            .instantiate_type_with_control(&arrow, control)?)
    }

    pub fn callable_value_source_with_control<C: TypeProjectionControl>(
        &self,
        catalog: &CheckedCallableCatalog,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        control: &mut C,
    ) -> Result<
        CheckedProjectFunctionCallableSource,
        CheckedProjectFunctionInstanceProjectionError<C::Error>,
    > {
        let CheckedProjectFunctionRuntimeOutcome::Continue { abi, next_group } = &self.outcome
        else {
            return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi.into());
        };
        self.continuation_source(
            catalog,
            abi,
            *next_group,
            Arc::clone(&self.solution),
            enclosing,
            control,
        )
    }

    pub(super) fn input_callable_source<C: TypeProjectionControl>(
        &self,
        catalog: &CheckedCallableCatalog,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        control: &mut C,
    ) -> Result<
        CheckedProjectFunctionCallableSource,
        CheckedProjectFunctionInstanceProjectionError<C::Error>,
    > {
        match (&self.input, &self.input_continuation) {
            (super::CheckedProjectFunctionRuntimeInput::Direct, None) => {
                select_project_function_value_runtime(
                    &self.declaration,
                    catalog,
                    enclosing,
                    control,
                )
            }
            (
                super::CheckedProjectFunctionRuntimeInput::Continuation { abi },
                Some(continuation),
            ) if abi.lineage() == continuation.digest()
                && self.group == continuation.next_group() =>
            {
                self.continuation_source(
                    catalog,
                    abi,
                    self.group,
                    Arc::clone(continuation.inherited_solution()),
                    enclosing,
                    control,
                )
            }
            _ => Err(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi.into()),
        }
    }

    fn continuation_source<C: TypeProjectionControl>(
        &self,
        catalog: &CheckedCallableCatalog,
        abi: &super::CheckedProjectContinuationRuntimeAbi,
        group: CallableGroupIndex,
        solution: Arc<FrozenCallTypeSolution>,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        control: &mut C,
    ) -> Result<
        CheckedProjectFunctionCallableSource,
        CheckedProjectFunctionInstanceProjectionError<C::Error>,
    > {
        let checked = checked_function(&self.declaration, catalog)?;
        let empty = ClosedTypeInstantiation::default();
        let caller = enclosing.map_or(&empty, |row| row.solution.as_ref());
        let function_type = caller.instantiate_type_with_control(abi.function_type(), control)?;
        CheckedProjectFunctionCallableSource {
            digest: CheckedProjectFunctionCallableSourceDigest::UNSEALED,
            declaration: self.declaration.clone(),
            checked: checked.clone(),
            origin: CheckedProjectFunctionCallableOrigin::Continuation {
                lineage: abi.lineage(),
            },
            group,
            function_type,
            parameters: Box::new([]),
            retained_parameters: Box::new([]),
            attached: None,
            projection: SourceProjection::Continuation(solution),
            base_instantiation: self.base_instantiation.clone(),
            source_enclosing: enclosing.cloned(),
            closed_selection: None,
        }
        .finish(control)
    }
}

fn checked_function<'a>(
    declaration: &CallableDeclarationKey,
    catalog: &'a CheckedCallableCatalog,
) -> Result<&'a CheckedCallableFacts, CheckedProjectFunctionRuntimeSelectionError> {
    if declaration.owner() != CallableDeclarationOwner::Function {
        return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidRootDeclaration);
    }
    let checked = catalog
        .project_callable(declaration)
        .map_err(CheckedProjectFunctionRuntimeSelectionError::Catalog)?;
    if !matches!(
        checked.execution(),
        CheckedCallableExecution::Runtime(CheckedFunctionExecution::DirectFrame)
    ) {
        return Err(CheckedProjectFunctionRuntimeSelectionError::MissingRuntimeExecution);
    }
    Ok(checked)
}

fn root_enclosing(
    checked: &CheckedCallableFacts,
    enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
) -> Option<CheckedProjectFunctionInstanceSolution> {
    use super::super::CallableSchemaGenericRole::RigidReference;
    let inventory = checked.signature().generic_inventory();
    let uses_enclosing = inventory
        .types()
        .iter()
        .any(|row| row.role() == RigidReference)
        || inventory
            .consts()
            .iter()
            .any(|row| row.role() == RigidReference)
        || inventory
            .effects()
            .iter()
            .any(|row| row.role() == RigidReference);
    // A context-free declaration root is the same source in every caller;
    // unrelated enclosing substitutions are not part of its evidence or key.
    uses_enclosing.then(|| enclosing.cloned()).flatten()
}

fn declared_function_type(
    checked: &CheckedCallableFacts,
    group: CallableGroupIndex,
) -> Result<TypeKind, CheckedProjectFunctionRuntimeSelectionError> {
    checked
        .signature()
        .project_function_type_from_group(
            group,
            checked.exposed_row(),
            || {
                checked
                    .result_schema()
                    .value_type()
                    .cloned()
                    .ok_or(super::super::CallConstraintInvariant::MalformedSchemaInventory)
            },
            |_, parameter| {
                parameter
                    .declared_type()
                    .cloned()
                    .ok_or(super::super::CallConstraintInvariant::MalformedSchemaInventory)
            },
        )
        .map_err(super::super::CallConstraintInvariant::from)
        .map_err(Into::into)
}
