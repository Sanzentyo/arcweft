//! Value uses and calls share one proof from a checked source to a body instance.
//! The source owns declaration identity and the producing environment; a type
//! match never chooses a declaration or reconstructs its producing expression.

use super::{
    ArrayLength, CallableGroupIndex, CheckedCallableCatalog,
    CheckedProjectFunctionInstanceProjectionError, CheckedProjectFunctionInstanceSolution,
    CheckedProjectFunctionRootRuntimeSelection, CheckedProjectFunctionRuntimeOutcome,
    CheckedProjectFunctionRuntimeSelection, CheckedProjectFunctionRuntimeSelectionError,
    ClosedTypeInstantiation, TypeKind,
    source::{
        CheckedProjectFunctionCallableOrigin, CheckedProjectFunctionCallableSource,
        CheckedProjectFunctionCallableSourceDigest,
    },
};
use crate::{
    effects::EffectSet,
    types::{GenericScope, TypeProjectionControl, TypeProjectionError},
};

/// A closed substitution of one program-callable source scheme. Argument
/// slices use the source binder's ordinal order in each namespace, independent
/// of declaration names and caller expression identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProjectFunctionSpecialization {
    source: CheckedProjectFunctionCallableOrigin,
    source_digest: CheckedProjectFunctionCallableSourceDigest,
    next_group: CallableGroupIndex,
    source_type: TypeKind,
    specialized_type: TypeKind,
    type_arguments: Box<[TypeKind]>,
    const_arguments: Box<[ArrayLength]>,
    effect_arguments: Box<[EffectSet]>,
    closed_selection: CheckedProjectFunctionRootRuntimeSelection,
}

impl CheckedProjectFunctionSpecialization {
    pub const fn source(&self) -> CheckedProjectFunctionCallableOrigin {
        self.source
    }
    pub const fn source_digest(&self) -> CheckedProjectFunctionCallableSourceDigest {
        self.source_digest
    }
    pub const fn next_group(&self) -> CallableGroupIndex {
        self.next_group
    }
    pub const fn source_type(&self) -> &TypeKind {
        &self.source_type
    }
    pub const fn specialized_type(&self) -> &TypeKind {
        &self.specialized_type
    }
    pub const fn type_arguments(&self) -> &[TypeKind] {
        &self.type_arguments
    }
    pub const fn const_arguments(&self) -> &[ArrayLength] {
        &self.const_arguments
    }
    pub const fn effect_arguments(&self) -> &[EffectSet] {
        &self.effect_arguments
    }
    pub const fn closed_selection(&self) -> &CheckedProjectFunctionRootRuntimeSelection {
        &self.closed_selection
    }

    pub(super) fn seal<C: TypeProjectionControl>(
        source: &CheckedProjectFunctionCallableSource,
        arguments: ClosedTypeInstantiation,
        closed_selection: CheckedProjectFunctionRootRuntimeSelection,
        control: &mut C,
    ) -> Result<Self, CheckedProjectFunctionInstanceProjectionError<C::Error>> {
        let specialized_type =
            arguments.specialize_function_with_control(source.function_type(), None, control)?;
        if !same_type(
            group_type(closed_selection.solution().callable_type(), source.group())?,
            &specialized_type,
            control,
        )? {
            return Err(
                CheckedProjectFunctionRuntimeSelectionError::SpecializationResultMismatch.into(),
            );
        }
        let empty = ClosedTypeInstantiation::default();
        let type_arguments = arguments
            .type_bindings()
            .map(|(_, value)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                // The RHS has already been closed in its owning environment.
                // A root projection charges structure without applying callee keys.
                empty.instantiate_type_with_control(value.value(), control)
            })
            .collect::<Result<Box<[_]>, TypeProjectionError<C::Error>>>()?;
        let const_arguments = arguments
            .const_bindings()
            .map(|(_, value)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                empty.instantiate_array_length_with_control(value.value(), control)
            })
            .collect::<Result<Box<[_]>, TypeProjectionError<C::Error>>>()?;
        let effect_arguments = arguments
            .effect_bindings()
            .map(|(_, value)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                empty.project_effect_row_with_control(value.value(), 1, control)
            })
            .collect::<Result<Box<[_]>, TypeProjectionError<C::Error>>>()?;
        Ok(Self {
            source: source.origin(),
            source_digest: source.source_digest(),
            next_group: source.group(),
            source_type: empty.instantiate_type_with_control(source.function_type(), control)?,
            specialized_type,
            type_arguments,
            const_arguments,
            effect_arguments,
            closed_selection,
        })
    }
}

impl CheckedProjectFunctionRuntimeSelection {
    /// The terminal call's frozen solution supplies source-binder arguments.
    /// Declaration roots and saved continuations use the same source authority;
    /// both verify the inverse mapping by forward composition.
    pub fn specialize_input_callable_with_control<C: TypeProjectionControl>(
        &self,
        catalog: &CheckedCallableCatalog,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        control: &mut C,
    ) -> Result<
        CheckedProjectFunctionSpecialization,
        CheckedProjectFunctionInstanceProjectionError<C::Error>,
    > {
        control.check().map_err(TypeProjectionError::Control)?;
        if !matches!(
            self.outcome,
            CheckedProjectFunctionRuntimeOutcome::Invoke { .. }
        ) {
            return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi.into());
        }
        let source = self.input_callable_source(catalog, enclosing, control)?;
        let body = self.solution.close_instantiation_with_control(
            enclosing.map(|row| row.solution.as_ref()),
            control,
        )?;
        source.specialize_completed_call(body, control)
    }
}

pub(super) fn group_type(
    mut ty: &TypeKind,
    group: CallableGroupIndex,
) -> Result<&TypeKind, CheckedProjectFunctionRuntimeSelectionError> {
    for _ in 0..group.get() {
        let TypeKind::Function { return_type, .. } = ty else {
            return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidResult);
        };
        ty = return_type;
    }
    if !matches!(ty, TypeKind::Function { .. }) {
        return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidResult);
    }
    Ok(ty)
}

pub(super) fn same_type<C: TypeProjectionControl>(
    left: &TypeKind,
    right: &TypeKind,
    control: &mut C,
) -> Result<bool, TypeProjectionError<C::Error>> {
    let root = GenericScope::default();
    Ok(
        left.semantic_identity_digest_in_scope_with_control(&root, control)?
            == right.semantic_identity_digest_in_scope_with_control(&root, control)?,
    )
}
