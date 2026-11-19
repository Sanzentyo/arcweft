//! Original values are admitted by their source, never a candidate inventory.

use crate::effect_row::prepared::{
    PreparedEffectFailure, PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectRow,
    PreparedEffectTermEdit,
};
use crate::types::PreparedEffects;
use crate::types::projection_control::EffectProjectionControl;
use crate::types::structural_projection::ScopedTypeProjection;
use crate::types::{GenericScope, TypeProjectionControl, TypeProjectionNodeKind};

use std::sync::Arc;

use super::{ConstraintOperand, ConstraintOperandOrigin};
#[cfg(test)]
use crate::effect_row::EffectPredicate;
#[cfg(test)]
use crate::types::constraints::TypeConstraintRejection;
#[cfg(test)]
use crate::types::constraints::solution::{
    map_effects_with_control, map_predicate_with_control, map_term_with_control,
};
use crate::types::constraints::{
    ConstraintDomain, OriginalProducerContext, TypeConstraintError,
    context::{TypeConstraintAccounting, TypeConstraintContext},
    lexical::ConstraintLexicalScope,
    references,
};
use crate::types::{
    ArrayLength, GenericConstReference, GenericTypeReference, TypeKind, TypeProjectionError,
};
use crate::{effect_row::EffectRow, types::GenericEffectReference};

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(in crate::types::constraints) trait OriginalOperandValue:
    Sized
{
    fn admit<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        target: &ConstraintLexicalScope,
        original: &OriginalProducerContext,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError>;
}

#[cfg(test)]
impl<D: ConstraintDomain, T> ConstraintOperand<D, T> {
    /// Admit in the owning source Γ and transport into the requested actual
    /// frames in one controlled fold. The target never reinterprets the input.
    pub(in crate::types::constraints) fn from_original_in<A: TypeConstraintAccounting>(
        value: &T,
        scope: Arc<ConstraintLexicalScope>,
        original: OriginalProducerContext,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError>
    where
        T: OriginalOperandValue,
    {
        let value = value.admit(&scope, &original, context)?;
        Ok(Self {
            value: Arc::new(value),
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Original(original)),
        })
    }
}

impl<D: ConstraintDomain> ConstraintOperand<D, TypeKind<PreparedEffects>> {
    /// Retain the source authority after transporting its prepared type in
    /// the live relation. No completed type or digest is issued here.
    pub(in crate::types::constraints::transaction) fn from_prepared_original_in<
        A: TypeConstraintAccounting,
    >(
        value: &TypeKind<PreparedEffects>,
        scope: Arc<ConstraintLexicalScope>,
        original: OriginalProducerContext,
        phase: &mut ConstraintEffectPhase<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        let value = phase.admit_type(value, &scope, &original, context)?;
        Ok(Self {
            value: Arc::new(value),
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Original(original)),
        })
    }
}

#[cfg(test)]
impl<A: TypeConstraintAccounting, D: ConstraintDomain> TypeConstraintContext<'_, A, D> {
    pub(in crate::types::constraints) fn fixture_prepared_operand_in(
        &mut self,
        original: &OriginalProducerContext,
        value: &TypeKind<PreparedEffects>,
        graph: &mut PreparedEffectGraph,
    ) -> Result<ConstraintOperand<D>, TypeConstraintError> {
        self.check_cancelled()?;
        for _ in original.scope().binders() {
            self.enter_node()?;
        }
        ConstraintOperand::from_prepared_original_in(
            value,
            Arc::new(original.gamma().clone()),
            original.clone(),
            &mut ConstraintEffectPhase::new(graph),
            self,
        )
    }

    pub(in crate::types::constraints) fn fixture_operand_in<T: OriginalOperandValue>(
        &mut self,
        original: &OriginalProducerContext,
        value: &T,
    ) -> Result<ConstraintOperand<D, T>, TypeConstraintError> {
        self.check_cancelled()?;
        for _ in original.scope().binders() {
            self.enter_node()?;
        }
        ConstraintOperand::from_original_in(
            value,
            Arc::new(original.gamma().clone()),
            original.clone(),
            self,
        )
    }
}

#[cfg(test)]
impl<D: ConstraintDomain, T> ConstraintOperand<D, T> {
    /// Deliberately raw terms for lower solver fixtures. Source admission tests
    /// use the production constructor with an explicit source authority.
    pub(in crate::types::constraints) fn fixture_original(
        value: T,
        scope: Arc<ConstraintLexicalScope>,
    ) -> Self {
        let original = OriginalProducerContext::ground_fixture(scope.as_ref().clone());
        Self {
            value: Arc::new(value),
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Original(original)),
        }
    }
}

#[cfg(test)]
impl OriginalOperandValue for TypeKind {
    fn admit<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        target_scope: &ConstraintLexicalScope,
        original: &OriginalProducerContext,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        map_term_with_control(
            self,
            original.scope(),
            target_scope.shape(),
            1,
            context,
            &|reference, source, target, _, context| {
                original
                    .admit_type_reference(target_scope, reference, source, target, context)
                    .map(TypeKind::GenericParam)
            },
            &|length, source, target, _, context| {
                length.admit_in_scopes(source, target, target_scope, original, context)
            },
            &|row, source, target, depth, context| {
                map_effects_with_control(row, depth, context, &|reference, context| {
                    let mapped = original.admit_effect_reference(
                        target_scope,
                        reference,
                        source,
                        target,
                        context,
                    )?;
                    Ok(crate::effect_row::EffectRow::open(
                        crate::effects::EffectSet::new(),
                        mapped,
                    ))
                })
            },
        )
        .map_err(|error| match error {
            TypeProjectionError::Control(error) => error,
            TypeProjectionError::Instantiation(crate::types::TypeInstantiationError::Scope(
                error,
            )) => error.into(),
            TypeProjectionError::Instantiation(_) => TypeConstraintRejection::UnresolvedType.into(),
        })
    }
}

/// A short loan of the actual effect relation. It cannot outlive the source
/// callback or replace the component's original accounting context.
pub(in crate::types::constraints::transaction) struct ConstraintEffectPhase<'graph> {
    graph: &'graph mut PreparedEffectGraph,
}

impl<'graph> ConstraintEffectPhase<'graph> {
    pub(in crate::types::constraints::transaction) fn new(
        graph: &'graph mut PreparedEffectGraph,
    ) -> Self {
        Self { graph }
    }

    pub(in crate::types::constraints::transaction) fn admit_type<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        &mut self,
        value: &TypeKind<PreparedEffects>,
        target: &ConstraintLexicalScope,
        original: &OriginalProducerContext,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind<PreparedEffects>, TypeConstraintError> {
        let mut projection = PreparedOriginalValueProjection {
            edit: self.graph.edit_terms(),
            target,
            original,
            context,
        };
        let value =
            value.project_structure(original.scope(), target.shape(), 1, &mut projection)?;
        projection.edit.commit().map_err(|error| {
            TypeConstraintError::Invariant(
                crate::types::constraints::TypeConstraintInvariant::PreparedEffect(
                    PreparedEffectFailure::Invariant(error),
                ),
            )
        })?;
        Ok(value)
    }
}

struct PreparedOriginalValueProjection<
    'phase,
    'graph,
    'control,
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
> {
    edit: PreparedEffectTermEdit<'graph>,
    target: &'phase ConstraintLexicalScope,
    original: &'phase OriginalProducerContext,
    context: &'phase mut TypeConstraintContext<'control, A, D>,
}

impl<A: TypeConstraintAccounting, D: ConstraintDomain> TypeProjectionControl
    for PreparedOriginalValueProjection<'_, '_, '_, A, D>
{
    type Error = TypeConstraintError;

    fn check(&mut self) -> Result<(), Self::Error> {
        self.context.check_cancelled()
    }

    fn visit_node(&mut self, _: TypeProjectionNodeKind, _: u64) -> Result<(), Self::Error> {
        self.context.enter_node()
    }

    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.context.enter_node()
    }
}

impl<A: TypeConstraintAccounting, D: ConstraintDomain>
    ScopedTypeProjection<PreparedEffects, PreparedEffects>
    for PreparedOriginalValueProjection<'_, '_, '_, A, D>
{
    fn project_type_reference(
        &mut self,
        reference: &GenericTypeReference,
        source: &GenericScope,
        target: &GenericScope,
        _: u64,
    ) -> Result<TypeKind<PreparedEffects>, TypeProjectionError<Self::Error>> {
        self.original
            .admit_type_reference(self.target, reference, source, target, self.context)
            .map(TypeKind::GenericParam)
    }

    fn project_length(
        &mut self,
        length: &ArrayLength,
        source: &GenericScope,
        target: &GenericScope,
        _: u64,
    ) -> Result<ArrayLength, TypeProjectionError<Self::Error>> {
        length.admit_in_scopes(source, target, self.target, self.original, self.context)
    }

    fn project_row(
        &mut self,
        row: &PreparedEffectRow,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<PreparedEffectRow, TypeProjectionError<Self::Error>> {
        let original = self.original;
        let lexical = self.target;
        self.edit
            .map_row_parameters_with_edit(
                row,
                &mut EffectProjectionControl {
                    control: self.context,
                    depth,
                },
                &mut |reference, edit, control| {
                    original.prepare_effect_reference(
                        lexical,
                        reference,
                        source,
                        target,
                        edit,
                        control.control,
                    )
                },
            )
            .map_err(TypeProjectionError::from)
    }

    fn project_predicate(
        &mut self,
        predicate: &PreparedEffectPredicate,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<PreparedEffectPredicate, TypeProjectionError<Self::Error>> {
        let original = self.original;
        let lexical = self.target;
        self.edit
            .map_predicate_parameters_with_edit(
                predicate,
                &mut EffectProjectionControl {
                    control: self.context,
                    depth,
                },
                &mut |reference, edit, control| {
                    original.prepare_effect_reference(
                        lexical,
                        reference,
                        source,
                        target,
                        edit,
                        control.control,
                    )
                },
            )
            .map_err(TypeProjectionError::from)
    }
}

impl ArrayLength {
    fn admit_in_scopes<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        source: &crate::types::GenericScope,
        target: &crate::types::GenericScope,
        target_scope: &ConstraintLexicalScope,
        original: &OriginalProducerContext,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeProjectionError<TypeConstraintError>> {
        context
            .check_cancelled()
            .map_err(TypeProjectionError::Control)?;
        if let Self::Generic(reference) = self {
            if let GenericConstReference::Bound(bound) = reference {
                source.bound_const(bound.depth(), bound.slot())?;
            }
            if !matches!(reference, GenericConstReference::Bound(_))
                && !original.admits_parameter(&reference.clone().into())
            {
                return Err(TypeProjectionError::Control(
                    references::const_out_of_scope(reference),
                ));
            }
            let mapped = target_scope
                .const_reference(original.gamma(), source, target, reference, context)
                .map_err(|error| {
                    TypeProjectionError::Control(
                        TypeConstraintContext::<A, D>::binding_transport_error(
                            error,
                            reference.clone().into(),
                        ),
                    )
                })?;
            return Ok(Self::Generic(mapped));
        }
        Ok(self.clone())
    }
}

#[cfg(test)]
impl OriginalOperandValue for ArrayLength {
    fn admit<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        target: &ConstraintLexicalScope,
        original: &OriginalProducerContext,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        context.enter_node()?;
        self.admit_in_scopes(original.scope(), target.shape(), target, original, context)
            .map_err(|error| match error {
                TypeProjectionError::Control(error) => error,
                TypeProjectionError::Instantiation(
                    crate::types::TypeInstantiationError::Scope(error),
                ) => error.into(),
                TypeProjectionError::Instantiation(_) => {
                    TypeConstraintRejection::UnresolvedType.into()
                }
            })
    }
}

impl OriginalProducerContext {
    /// Source admission precedes preparation. An authorized inference input
    /// denotes the exact original application lent by its introducing owner.
    fn prepare_effect_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        lexical: &ConstraintLexicalScope,
        reference: &GenericEffectReference,
        source: &GenericScope,
        target: &GenericScope,
        edit: &mut PreparedEffectTermEdit<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<
        Option<PreparedEffectRow>,
        PreparedEffectFailure<TypeProjectionError<TypeConstraintError>>,
    > {
        let admitted = self
            .admit_effect_reference(lexical, reference, source, target, context)
            .map_err(PreparedEffectFailure::Control)?;
        if matches!(admitted, GenericEffectReference::Inference(_)) {
            return self
                .rigid_scope()
                .imported
                .and_then(|lease| lease.effect_application(&admitted))
                .ok_or(crate::effect_row::prepared::PreparedEffectInvariant::ParameterContract)?
                .transport_into(lexical, target, edit, context)
                .map(Some)
                .map_err(|error| {
                    PreparedEffectFailure::Control(TypeProjectionError::Control(error))
                });
        }
        Ok((&admitted != reference).then(|| {
            PreparedEffectRow::Canonical(EffectRow::open(
                crate::effects::EffectSet::new(),
                admitted,
            ))
        }))
    }

    fn admit_type_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        target_scope: &ConstraintLexicalScope,
        reference: &GenericTypeReference,
        source: &GenericScope,
        target: &GenericScope,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericTypeReference, TypeProjectionError<TypeConstraintError>> {
        if let GenericTypeReference::Bound(bound) = reference {
            source.bound_type(bound.depth(), bound.slot())?;
        } else if !self.admits_parameter(&reference.clone().into()) {
            return Err(TypeProjectionError::Control(references::type_out_of_scope(
                reference,
            )));
        }
        target_scope
            .type_reference(self.gamma(), source, target, reference, context)
            .map_err(|error| {
                TypeProjectionError::Control(
                    TypeConstraintContext::<A, D>::binding_transport_error(
                        error,
                        reference.clone().into(),
                    ),
                )
            })
    }

    pub(in crate::types::constraints) fn admit_effect_reference<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        &self,
        target_scope: &ConstraintLexicalScope,
        reference: &GenericEffectReference,
        source: &GenericScope,
        target: &GenericScope,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericEffectReference, TypeProjectionError<TypeConstraintError>> {
        if let GenericEffectReference::Bound(bound) = reference {
            source.bound_effect(bound.depth(), bound.slot())?;
        } else if !self.admits_parameter(&reference.clone().into()) {
            return Err(TypeProjectionError::Control(
                references::effect_out_of_scope(reference),
            ));
        }
        target_scope
            .effect_reference(self.gamma(), source, target, reference, context)
            .map_err(|error| {
                TypeProjectionError::Control(
                    TypeConstraintContext::<A, D>::binding_transport_error(
                        error,
                        reference.clone().into(),
                    ),
                )
            })
    }

    #[cfg(test)]
    fn admit_primitive_effect<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        target: &ConstraintLexicalScope,
        reference: &GenericEffectReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<EffectRow, TypeProjectionError<TypeConstraintError>> {
        let mapped =
            self.admit_effect_reference(target, reference, self.scope(), target.shape(), context)?;
        Ok(EffectRow::open(crate::effects::EffectSet::new(), mapped))
    }
}

#[cfg(test)]
impl OriginalOperandValue for EffectRow {
    fn admit<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        target: &ConstraintLexicalScope,
        original: &OriginalProducerContext,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        map_effects_with_control(self, 1, context, &|reference, context| {
            original.admit_primitive_effect(target, reference, context)
        })
        .map_err(|error| match error {
            TypeProjectionError::Control(error) => error,
            TypeProjectionError::Instantiation(crate::types::TypeInstantiationError::Scope(
                error,
            )) => error.into(),
            TypeProjectionError::Instantiation(_) => TypeConstraintRejection::UnresolvedType.into(),
        })
    }
}

#[cfg(test)]
impl OriginalOperandValue for EffectPredicate {
    fn admit<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        target: &ConstraintLexicalScope,
        original: &OriginalProducerContext,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        map_predicate_with_control(self, 1, context, &|reference, context| {
            original.admit_primitive_effect(target, reference, context)
        })
        .map_err(|error| match error {
            TypeProjectionError::Control(error) => error,
            TypeProjectionError::Instantiation(crate::types::TypeInstantiationError::Scope(
                error,
            )) => error.into(),
            TypeProjectionError::Instantiation(_) => TypeConstraintRejection::UnresolvedType.into(),
        })
    }
}
