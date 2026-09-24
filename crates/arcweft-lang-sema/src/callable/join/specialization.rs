//! The checked function-value and invocation paths share one specialization
//! proof. A proof names one source continuation; it never selects code from a
//! function type or reconstructs a producing expression.

use super::{
    Arc, ArrayLength, CallableGroupIndex, CallableInstantiationDigestError,
    CheckedCallContinuationDigest, CheckedCallableCatalog, CheckedProjectContinuationRuntimeAbi,
    CheckedProjectFunctionInstanceProjectionError, CheckedProjectFunctionInstanceSolution,
    CheckedProjectFunctionRootRuntimeSelection, CheckedProjectFunctionRuntimeInput,
    CheckedProjectFunctionRuntimeOutcome, CheckedProjectFunctionRuntimeSelection,
    CheckedProjectFunctionRuntimeSelectionError, ClosedTypeInstantiation, EffectSubstitution,
    TypeKind, callable_instantiation_digest_from_bindings,
};
use crate::{
    callable::CheckedFunctionSpecialization,
    effects::EffectSet,
    types::{GenericScope, TypeProjectionControl, TypeProjectionError},
};

/// A closed substitution of one program-callable source scheme. Argument
/// slices use the source binder's ordinal order in each namespace, independent
/// of declaration names and caller expression identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProjectFunctionSpecialization {
    lineage: CheckedCallContinuationDigest,
    next_group: CallableGroupIndex,
    source_type: TypeKind,
    specialized_type: TypeKind,
    type_arguments: Box<[TypeKind]>,
    const_arguments: Box<[ArrayLength]>,
    effect_arguments: Box<[EffectSet]>,
    closed_selection: CheckedProjectFunctionRootRuntimeSelection,
}

impl CheckedProjectFunctionSpecialization {
    pub const fn lineage(&self) -> CheckedCallContinuationDigest {
        self.lineage
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
}

struct ProjectSpecializationSeed<'a> {
    source: &'a CheckedProjectContinuationRuntimeAbi,
    next_group: CallableGroupIndex,
    arguments: ClosedTypeInstantiation,
    body: ClosedTypeInstantiation,
}

impl CheckedProjectFunctionRuntimeSelection {
    /// Join a value-use witness to this exact prefix's latent body. The
    /// witness supplies only checked substitution; this selection supplies
    /// declaration, continuation lineage and retained-prefix authority.
    /// The source and value use may belong to different enclosing instances;
    /// each owns its free references and is closed exactly once.
    pub fn specialize_callable_value_with_control<C: TypeProjectionControl>(
        &self,
        catalog: &CheckedCallableCatalog,
        witness: &CheckedFunctionSpecialization,
        source_enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        witness_enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        control: &mut C,
    ) -> Result<
        CheckedProjectFunctionSpecialization,
        CheckedProjectFunctionInstanceProjectionError<C::Error>,
    > {
        control.check().map_err(TypeProjectionError::Control)?;
        let CheckedProjectFunctionRuntimeOutcome::Continue { abi, next_group } = &self.outcome
        else {
            return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi.into());
        };
        let empty = ClosedTypeInstantiation::default();
        let source_caller = source_enclosing.map_or(&empty, |row| row.solution.as_ref());
        let witness_caller = witness_enclosing.map_or(&empty, |row| row.solution.as_ref());
        let source = source_caller.instantiate_type_with_control(abi.function_type(), control)?;
        let witness_source =
            witness_caller.instantiate_type_with_control(witness.source_type(), control)?;
        if !same_type(&source, &witness_source, control)? {
            return Err(
                CheckedProjectFunctionRuntimeSelectionError::SpecializationSourceMismatch.into(),
            );
        }
        let arguments = witness.close_arguments_with_control(Some(witness_caller), control)?;
        let body =
            self.solution
                .close_residual_with_control(&arguments, Some(source_caller), control)?;
        let proof = self.seal_specialization(
            ProjectSpecializationSeed {
                source: abi,
                next_group: *next_group,
                arguments,
                body,
            },
            catalog,
            source_enclosing,
            control,
        )?;
        let expected =
            witness_caller.instantiate_type_with_control(witness.specialized_type(), control)?;
        if !same_type(proof.specialized_type(), &expected, control)? {
            return Err(
                CheckedProjectFunctionRuntimeSelectionError::SpecializationResultMismatch.into(),
            );
        }
        Ok(proof)
    }

    /// Issue the same proof from a terminal checked call. Its completed
    /// solution supplies the arguments for the input continuation's residual
    /// slots; the inherited prefix is verified by forward composition.
    pub fn specialize_continuation_with_control<C: TypeProjectionControl>(
        &self,
        catalog: &CheckedCallableCatalog,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        control: &mut C,
    ) -> Result<
        CheckedProjectFunctionSpecialization,
        CheckedProjectFunctionInstanceProjectionError<C::Error>,
    > {
        control.check().map_err(TypeProjectionError::Control)?;
        let (CheckedProjectFunctionRuntimeInput::Continuation { abi }, Some(continuation)) =
            (&self.input, &self.input_continuation)
        else {
            return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi.into());
        };
        if !matches!(
            self.outcome,
            CheckedProjectFunctionRuntimeOutcome::Invoke { .. }
        ) || abi.lineage() != continuation.digest()
            || self.group != continuation.next_group()
        {
            return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidContinuationAbi.into());
        }
        let caller = enclosing.map(|row| row.solution.as_ref());
        let body = self
            .solution
            .close_instantiation_with_control(caller, control)?;
        let arguments = continuation
            .inherited_solution()
            .residual_arguments_with_control(&body, caller, control)?;
        self.seal_specialization(
            ProjectSpecializationSeed {
                source: abi,
                next_group: self.group,
                arguments,
                body,
            },
            catalog,
            enclosing,
            control,
        )
    }

    fn seal_specialization<C: TypeProjectionControl>(
        &self,
        seed: ProjectSpecializationSeed<'_>,
        catalog: &CheckedCallableCatalog,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        control: &mut C,
    ) -> Result<
        CheckedProjectFunctionSpecialization,
        CheckedProjectFunctionInstanceProjectionError<C::Error>,
    > {
        let empty = ClosedTypeInstantiation::default();
        let caller = enclosing.map_or(&empty, |row| row.solution.as_ref());
        let source_type =
            caller.instantiate_type_with_control(seed.source.function_type(), control)?;
        let specialized_type = seed.arguments.specialize_function_with_control(
            seed.source.function_type(),
            Some(caller),
            control,
        )?;
        let checked = catalog
            .project_callable(&self.declaration)
            .map_err(CheckedProjectFunctionRuntimeSelectionError::Catalog)?;
        let terminal = checked
            .signature()
            .groups()
            .last()
            .ok_or(CheckedProjectFunctionRuntimeSelectionError::InvalidRootGroup)?
            .index();
        let declared = checked
            .signature()
            .declared_function_type_from_group(CallableGroupIndex::ZERO, checked.exposed_row())
            .map_err(CheckedProjectFunctionRuntimeSelectionError::from)?;
        let callable_type = seed
            .body
            .instantiate_type_with_control(&declared, control)?;
        if !same_type(
            group_type(&callable_type, seed.next_group)?,
            &specialized_type,
            control,
        )? {
            return Err(
                CheckedProjectFunctionRuntimeSelectionError::SpecializationResultMismatch.into(),
            );
        }
        let function_type = group_type(&callable_type, terminal)?.clone();
        let TypeKind::Function { effects, .. } = &function_type else {
            return Err(CheckedProjectFunctionRuntimeSelectionError::InvalidResult.into());
        };
        let effects = effects
            .resolve(&EffectSubstitution::new())
            .map_err(CheckedProjectFunctionRuntimeSelectionError::EffectRow)?;
        let instantiation = callable_instantiation_digest_from_bindings(
            &self.base_instantiation,
            seed.body.type_bindings(),
            seed.body.const_bindings(),
            seed.body.effect_bindings(),
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
        let type_arguments = seed
            .arguments
            .type_bindings()
            .map(|(_, value)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                // The arguments have already been closed in their caller. A root
                // projection charges their structure without reapplying callee keys.
                empty.instantiate_type_with_control(value.value(), control)
            })
            .collect::<Result<Box<[_]>, TypeProjectionError<C::Error>>>()?;
        let const_arguments = seed
            .arguments
            .const_bindings()
            .map(|(_, value)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                empty.instantiate_array_length_with_control(value.value(), control)
            })
            .collect::<Result<Box<[_]>, TypeProjectionError<C::Error>>>()?;
        let effect_arguments = seed
            .arguments
            .effect_bindings()
            .map(|(_, value)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                empty.project_effect_row_with_control(value.value(), 1, control)
            })
            .collect::<Result<Box<[_]>, TypeProjectionError<C::Error>>>()?;
        Ok(CheckedProjectFunctionSpecialization {
            lineage: seed.source.lineage(),
            next_group: seed.next_group,
            source_type,
            specialized_type,
            type_arguments,
            const_arguments,
            effect_arguments,
            closed_selection: CheckedProjectFunctionRootRuntimeSelection {
                declaration: self.declaration.clone(),
                group: terminal,
                function_type: function_type.clone(),
                effects,
                solution: CheckedProjectFunctionInstanceSolution {
                    solution: Arc::new(seed.body),
                    instantiation,
                    function_type,
                    callable_type,
                },
            },
        })
    }
}

fn group_type(
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

fn same_type<C: TypeProjectionControl>(
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
