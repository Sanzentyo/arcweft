//! Free declarations are authorized by the corresponding value derivation.

use super::*;
use crate::types::constraints::{ConstraintGenericParameterId, references};

impl<D: ConstraintDomain> ConstraintOperandOrigin<D> {
    fn admits_free<A: TypeConstraintAccounting>(
        &self,
        parameter: &ConstraintGenericParameterId,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<bool, TypeConstraintError> {
        let mut current = self;
        loop {
            context.enter_node()?;
            current = match current {
                Self::Original(original) => return Ok(original.admits_parameter(parameter)),
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
                .admits_free(&reference.clone().into(), context)?
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
                .admits_free(&reference.clone().into(), context)?
        {
            return Err(references::const_out_of_scope(reference));
        }
        Ok(())
    }
}
