//! Value ports follow their corresponding derivation, independently of the
//! original observation's result association.

use super::*;
use crate::types::constraints::CheckedConstraintContainerConstructor;

impl<D: ConstraintDomain> ConstraintOperandOrigin<D> {
    pub(super) fn type_child_origin(
        self: &Arc<Self>,
        index: usize,
    ) -> Result<Arc<Self>, TypeConstraintError> {
        let mut current = self;
        loop {
            match current.as_ref() {
                Self::Projection { selected, .. } => current = selected,
                Self::TypeAlias { terminal, .. } => current = &terminal.origin,
                Self::Rebuilt { children, .. } => {
                    return children
                        .get(index)
                        .map(|child| Arc::clone(&child.origin))
                        .ok_or_else(|| TypeConstraintInvariant::OriginalProducer.into());
                }
                Self::SourceExpectation {
                    projection,
                    actual,
                    value_expected,
                } => match projection {
                    CheckedConstraintSourceProjection::Scalar => current = &value_expected.origin,
                    CheckedConstraintSourceProjection::SpreadContainer(
                        CheckedConstraintContainerConstructor::MapValue { .. },
                    ) => match index {
                        0 => current = &actual.origin,
                        1 => return Ok(Arc::clone(&value_expected.origin)),
                        _ => return Err(TypeConstraintInvariant::OriginalProducer.into()),
                    },
                    CheckedConstraintSourceProjection::SpreadContainer(_) if index == 0 => {
                        return Ok(Arc::clone(&value_expected.origin));
                    }
                    _ => return Err(TypeConstraintInvariant::OriginalProducer.into()),
                },
                Self::FunctionSchemeInstantiation { source, .. } => current = &source.origin,
                Self::Original(_)
                | Self::Template(_)
                | Self::Source { .. }
                | Self::Result(_)
                | Self::Completed { .. } => return Ok(Arc::clone(current)),
                Self::Parameter(_) | Self::ConstOccurrence(_) | Self::ConstAlias { .. } => {
                    return Err(TypeConstraintInvariant::OriginalProducer.into());
                }
            }
        }
    }

    pub(super) fn length_origin(self: &Arc<Self>) -> Result<Arc<Self>, TypeConstraintError> {
        let mut current = self;
        loop {
            match current.as_ref() {
                Self::Projection { selected, .. } => current = selected,
                Self::TypeAlias { terminal, .. } => current = &terminal.origin,
                Self::Rebuilt {
                    length: Some(length),
                    ..
                } => return Ok(Arc::clone(&length.origin)),
                Self::SourceExpectation {
                    projection,
                    actual,
                    value_expected,
                } => match projection {
                    CheckedConstraintSourceProjection::Scalar => current = &value_expected.origin,
                    CheckedConstraintSourceProjection::SpreadContainer(
                        CheckedConstraintContainerConstructor::Array { .. },
                    ) => current = &actual.origin,
                    _ => return Err(TypeConstraintInvariant::OriginalProducer.into()),
                },
                Self::Original(_)
                | Self::Template(_)
                | Self::Source { .. }
                | Self::Result(_)
                | Self::Completed { .. } => return Ok(Arc::clone(current)),
                _ => return Err(TypeConstraintInvariant::OriginalProducer.into()),
            }
        }
    }
}
