//! Ordered collection and Result traversal use the original output type.

use super::{RuntimeProgramTypeError, RuntimeProgramTypes, RuntimeProgramValueType};
use crate::{
    awbc::schema::AwbcRuntimeTypeShape,
    pattern::{RuntimeBuiltinVariantCaseIdentity, RuntimeSemanticTypeId},
    plan::RuntimePlanTypeProjection,
};

#[cfg(test)]
mod tests;

/// The closed relationship between one fanout child and its aggregate.
/// This is a borrowed type decision, not a second task or type inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeFanoutOutputKind {
    Sequence,
    ResultSequence,
}

impl RuntimeProgramTypes<'_> {
    pub(crate) fn fanout_output_kind(
        self,
        aggregate: RuntimeSemanticTypeId,
        child: RuntimeSemanticTypeId,
    ) -> Result<RuntimeFanoutOutputKind, RuntimeProgramTypeError> {
        if self.fanout_sequence_item(aggregate)? == Some(child) {
            return Ok(RuntimeFanoutOutputKind::Sequence);
        }
        if let (Some((collection, aggregate_error)), Some((item, child_error))) = (
            self.fanout_result_items(aggregate)?,
            self.fanout_result_items(child)?,
        ) && aggregate_error == child_error
            && self.fanout_sequence_item(collection)? == Some(item)
        {
            return Ok(RuntimeFanoutOutputKind::ResultSequence);
        }
        Err(RuntimeProgramTypeError::InvalidFanoutOutput { aggregate, child })
    }

    fn fanout_sequence_item(
        self,
        semantic: RuntimeSemanticTypeId,
    ) -> Result<Option<RuntimeSemanticTypeId>, RuntimeProgramTypeError> {
        Ok(match self.value_type(semantic)? {
            RuntimeProgramValueType::Plan { program, ty } => {
                match program
                    .type_table()
                    .get(ty)
                    .ok_or(RuntimeProgramTypeError::Missing {
                        semantic_type: semantic,
                    })?
                    .projection()
                {
                    RuntimePlanTypeProjection::Sequence { item, .. } => program
                        .type_table()
                        .get(*item)
                        .map(crate::plan::RuntimePlanTypeDeclaration::semantic_identity),
                    _ => None,
                }
            }
            RuntimeProgramValueType::Awbc { program, ty } => {
                match program.runtime_types[ty.index()].shape() {
                    AwbcRuntimeTypeShape::Sequence { item, .. } => program
                        .runtime_types
                        .get(item.index())
                        .map(crate::awbc::schema::AwbcRuntimeType::semantic_identity),
                    _ => None,
                }
            }
        })
    }

    fn fanout_result_items(
        self,
        semantic: RuntimeSemanticTypeId,
    ) -> Result<Option<(RuntimeSemanticTypeId, RuntimeSemanticTypeId)>, RuntimeProgramTypeError>
    {
        Ok(match self.value_type(semantic)? {
            RuntimeProgramValueType::Plan { program, ty } => {
                match program
                    .type_table()
                    .get(ty)
                    .ok_or(RuntimeProgramTypeError::Missing {
                        semantic_type: semantic,
                    })?
                    .projection()
                {
                    RuntimePlanTypeProjection::Result { value, error, .. } => program
                        .type_table()
                        .get(*value)
                        .zip(program.type_table().get(*error))
                        .map(|(value, error)| {
                            (value.semantic_identity(), error.semantic_identity())
                        }),
                    _ => None,
                }
            }
            RuntimeProgramValueType::Awbc { program, ty } => {
                let item = |case| {
                    program
                        .builtin_variant_payload_item(ty, case)
                        .and_then(|item| {
                            program
                                .runtime_types
                                .get(item.index())
                                .map(crate::awbc::schema::AwbcRuntimeType::semantic_identity)
                        })
                };
                item(RuntimeBuiltinVariantCaseIdentity::ResultOk)
                    .zip(item(RuntimeBuiltinVariantCaseIdentity::ResultErr))
            }
        })
    }
}
