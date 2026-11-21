//! Free declarations are authorized by the corresponding value derivation.

use super::*;
use crate::effect_row::prepared::{
    PreparedEffectPredicate, PreparedEffectReadView, PreparedEffectRow, SourceEffectParameter,
};
use crate::types::GenericEffectReference;
use crate::types::constraints::{ConstraintGenericParameterId, references};

pub(in crate::types::constraints) struct AdmittedConstraintFunctionHeader<'a, D: ConstraintDomain>(
    ConstraintFunctionHeaderView<'a, D>,
);

impl<'a, D: ConstraintDomain> AdmittedConstraintFunctionHeader<'a, D> {
    pub(in crate::types::constraints) fn view(&self) -> &ConstraintFunctionHeaderView<'a, D> {
        &self.0
    }
}

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
                Self::ClosedScalar => return Ok(false),
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

impl<'a, D: ConstraintDomain> ConstraintFunctionHeaderView<'a, D> {
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

    fn admit_source_parameter<A: TypeConstraintAccounting>(
        &self,
        field: ConstraintEffectField,
        parameter: SourceEffectParameter<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        match parameter {
            SourceEffectParameter::Reference { captured: true, .. } => Ok(()),
            SourceEffectParameter::Reference { reference, .. } => {
                self.admit_effect_reference(field, reference, context)
            }
            SourceEffectParameter::Introduction(reference) => {
                if !self.admits_effect_parameter(field, &reference.clone().into(), context)? {
                    return Err(references::effect_out_of_scope(reference));
                }
                Ok(())
            }
        }
    }

    pub(in crate::types::constraints) fn admit_projection<A: TypeConstraintAccounting>(
        header: Option<Self>,
        row: &PreparedEffectRow,
        predicate: &PreparedEffectPredicate,
        effects: PreparedEffectReadView<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
        mut check_reference: impl FnMut(
            &GenericEffectReference,
            &mut TypeConstraintContext<'_, A, D>,
        ) -> Result<(), TypeConstraintError>,
    ) -> Result<Option<AdmittedConstraintFunctionHeader<'a, D>>, TypeConstraintError> {
        if let Some(header) = &header
            && !(std::ptr::eq(header.row(), row) && std::ptr::eq(header.predicate(), predicate))
        {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }

        // The active branch sees the actual arguments. Source admission also
        // checks the introduction that supplies any inherited arguments.
        let mut admit = |field, parameter: SourceEffectParameter<'_>, context: &mut _| {
            if let SourceEffectParameter::Reference { reference, .. } = parameter {
                check_reference(reference, context)?;
            }
            if let Some(header) = &header {
                header.admit_source_parameter(field, parameter, context)?;
            }
            Ok(())
        };
        effects.visit_predicate_source_parameters(
            predicate,
            context,
            &mut |parameter, context| admit(ConstraintEffectField::Predicate, parameter, context),
        )?;
        if let PreparedEffectPredicate::Canonical(predicate) = predicate
            && predicate.is_impossible()
        {
            return Err(crate::types::constraints::TypeConstraintRejection::Mismatch.into());
        }
        effects.visit_row_source_parameters(row, context, &mut |parameter, context| {
            admit(ConstraintEffectField::Row, parameter, context)
        })?;
        Ok(header.map(AdmittedConstraintFunctionHeader))
    }

    pub(in crate::types) fn admit<A: TypeConstraintAccounting>(
        &self,
        effects: PreparedEffectReadView<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        effects.visit_row_source_parameters(self.row(), context, &mut |parameter, context| {
            self.admit_source_parameter(ConstraintEffectField::Row, parameter, context)
        })?;
        effects.visit_predicate_source_parameters(
            self.predicate(),
            context,
            &mut |parameter, context| {
                self.admit_source_parameter(ConstraintEffectField::Predicate, parameter, context)
            },
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub(in crate::types::constraints) fn admit_for_test<A: TypeConstraintAccounting>(
        self,
        effects: PreparedEffectReadView<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<AdmittedConstraintFunctionHeader<'a, D>, TypeConstraintError> {
        let (row, predicate) = (self.row(), self.predicate());
        Self::admit_projection(Some(self), row, predicate, effects, context, |_, _| Ok(()))?
            .ok_or_else(|| TypeConstraintInvariant::OriginalProducer.into())
    }
}
