//! Free declarations are authorized by the corresponding value derivation.

use super::*;
use crate::types::constraints::{ConstraintGenericParameterId, references};

impl<D: ConstraintDomain> ConstraintOperandOrigin<D> {
    fn admits_parameter<A: TypeConstraintAccounting>(
        &self,
        parameter: &ConstraintGenericParameterId,
        field: Option<ConstraintEffectField>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<bool, TypeConstraintError> {
        let mut current = self;
        loop {
            context.enter_node()?;
            current = match current {
                Self::Original(original) => return Ok(original.admits_parameter(parameter)),
                Self::ComputedEffect { support, .. } => {
                    return Ok(support.admits_original_parameter(parameter));
                }
                Self::Parameter(owner) => return Ok(owner.contains(parameter)),
                Self::Template(owner) => return Ok(owner.admits_template_parameter(parameter)),
                Self::Source { value, .. } => match value {
                    ResolvedConstraintSource::Value(owner) => {
                        return Ok(owner.original_context().admits_parameter(parameter));
                    }
                    ResolvedConstraintSource::Result(result) => {
                        return Ok(result.owner.admits_template_parameter(parameter));
                    }
                    #[cfg(test)]
                    ResolvedConstraintSource::Fixture => {
                        let Self::Source { original, .. } = current else {
                            unreachable!()
                        };
                        return Ok(original.admits_parameter(parameter));
                    }
                },
                Self::Result(result) => {
                    return Ok(result.owner.admits_template_parameter(parameter));
                }
                Self::Completed {
                    solution,
                    parameter: binding,
                    ..
                } => {
                    return Ok(solution
                        .binding_captures(binding)
                        .is_some_and(|scope| scope.contains(parameter)));
                }
                Self::Projection { selected, .. } => selected,
                Self::ConstOccurrence(operand) => &operand.origin,
                Self::Rebuilt {
                    header: Some(header),
                    ..
                } if field.is_some() => {
                    return Ok(header.admits(field.expect("checked field"), parameter));
                }
                Self::Rebuilt { source, .. } => source,
                Self::TypeAlias { terminal, .. } => &terminal.origin,
                Self::ConstAlias { terminal, .. } => &terminal.origin,
                Self::FunctionSchemeInstantiation { source, .. } => &source.origin,
                Self::SourceExpectation {
                    value_expected,
                    projection,
                    ..
                } => match projection {
                    CheckedConstraintSourceProjection::Scalar => &value_expected.origin,
                    _ => return Err(TypeConstraintInvariant::OriginalProducer.into()),
                },
            };
        }
    }
}

impl<D: ConstraintDomain> ConstraintOperandView<'_, D> {
    pub(in crate::types::constraints) fn admit_type_reference<A: TypeConstraintAccounting>(
        &self,
        reference: &GenericTypeReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        if matches!(reference, GenericTypeReference::Free(_))
            && !self
                .value_origin
                .admits_parameter(&reference.clone().into(), None, context)?
        {
            return Err(references::type_out_of_scope(reference));
        }
        Ok(())
    }
}

impl<D: ConstraintDomain> ConstraintConstOperandView<'_, D> {
    pub(in crate::types::constraints) fn admit_const_reference<A: TypeConstraintAccounting>(
        &self,
        reference: &GenericConstReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        if matches!(reference, GenericConstReference::Free(_))
            && !self
                .value_origin
                .admits_parameter(&reference.clone().into(), None, context)?
        {
            return Err(references::const_out_of_scope(reference));
        }
        Ok(())
    }
}

impl<D: ConstraintDomain, T> ConstraintOperand<D, T> {
    pub(in crate::types::constraints) fn admits_effect_parameter<A: TypeConstraintAccounting>(
        &self,
        parameter: &ConstraintGenericParameterId,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<bool, TypeConstraintError> {
        self.origin.admits_parameter(parameter, None, context)
    }

    pub(in crate::types::constraints) fn admit_effect_reference<A: TypeConstraintAccounting>(
        &self,
        reference: &crate::types::GenericEffectReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        if let ConstraintOperandOrigin::ComputedEffect { context: owner, .. } = self.origin.as_ref()
            && self.scope != *owner.scope()
        {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        match reference {
            crate::types::GenericEffectReference::Bound(bound) => {
                self.scope
                    .shape()
                    .bound_effect(bound.depth(), bound.slot())?;
            }
            _ if !self
                .origin
                .admits_parameter(&reference.clone().into(), None, context)? =>
            {
                return Err(references::effect_out_of_scope(reference));
            }
            _ => {}
        }
        Ok(())
    }
}

impl<D: ConstraintDomain> ConstraintFunctionHeaderView<'_, D> {
    pub(in crate::types::constraints) fn admits_effect_parameter<A: TypeConstraintAccounting>(
        &self,
        field: ConstraintEffectField,
        parameter: &ConstraintGenericParameterId,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<bool, TypeConstraintError> {
        self.source
            .value_origin
            .admits_parameter(parameter, Some(field), context)
    }

    pub(in crate::types::constraints) fn admit_effect_reference<A: TypeConstraintAccounting>(
        &self,
        field: ConstraintEffectField,
        reference: &crate::types::GenericEffectReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        match reference {
            crate::types::GenericEffectReference::Bound(bound) => {
                self.lexical_scope()
                    .shape()
                    .bound_effect(bound.depth(), bound.slot())?;
            }
            crate::types::GenericEffectReference::Free(_)
                if !self.source.value_origin.admits_parameter(
                    &reference.clone().into(),
                    Some(field),
                    context,
                )? =>
            {
                return Err(references::effect_out_of_scope(reference));
            }
            _ => {}
        }
        Ok(())
    }

    pub(in crate::types::constraints) fn admit<A: TypeConstraintAccounting>(
        &self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        for (field, reference) in self
            .row()
            .formula()
            .into_iter()
            .flat_map(|f| f.variables())
            .map(|r| (ConstraintEffectField::Row, r))
            .chain(
                self.predicate()
                    .variables()
                    .map(|r| (ConstraintEffectField::Predicate, r)),
            )
        {
            context.enter_node()?;
            self.admit_effect_reference(field, reference, context)?;
        }
        Ok(())
    }
}
