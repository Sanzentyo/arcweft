//! Exact field-default requests over the selected executable's pure programs.

use arcweft_data::ShapeId;
use arcweft_id::runtime_program::RuntimePureProgramId;
use thiserror::Error;

use super::{RuntimeCodecUse, RuntimeProgramDataShapeError, RuntimeProgramDataShapes};
use crate::{
    entry::RuntimeSchemaLimits,
    pattern::RuntimeSemanticTypeId,
    program_types::{RuntimeProgramTypeError, RuntimeProgramTypes},
    value::RuntimeValue,
};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeDataFieldDefaultError {
    #[error(
        "field {field} of {record_type:?} has a default/skip annotation without an admitted producer"
    )]
    MissingProducer {
        record_type: RuntimeSemanticTypeId,
        field: usize,
    },
    #[error("field {field} of {record_type:?} has a producer without a default/skip annotation")]
    UnexpectedProducer {
        record_type: RuntimeSemanticTypeId,
        field: usize,
    },
    #[error(
        "field {field} of {record_type:?} does not resolve to an exact nullary pure program {program}: {reason}"
    )]
    InvalidProgram {
        record_type: RuntimeSemanticTypeId,
        field: usize,
        program: RuntimePureProgramId,
        reason: &'static str,
    },
}

/// The codec requests a value; the selected pure backend executes it.
/// The request retains the same borrowed program and exact field result type.
/// It contains no closure, I/O capability, inferred value or replacement graph.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeDataFieldDefaultRequest<'program> {
    types: RuntimeProgramTypes<'program>,
    program: RuntimePureProgramId,
    result_type: RuntimeSemanticTypeId,
    result_shape: ShapeId,
}

impl<'program> RuntimeDataFieldDefaultRequest<'program> {
    #[must_use]
    pub const fn program_types(&self) -> RuntimeProgramTypes<'program> {
        self.types
    }

    #[must_use]
    pub const fn program(&self) -> RuntimePureProgramId {
        self.program
    }

    #[must_use]
    pub const fn result_type(&self) -> RuntimeSemanticTypeId {
        self.result_type
    }

    /// Per-use shape coordinate, preserving policies below the field edge.
    #[must_use]
    pub const fn result_shape(&self) -> ShapeId {
        self.result_shape
    }

    /// Checks the generated value before a codec/runtime producer publishes it.
    pub fn validate_result(
        &self,
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<(), RuntimeProgramTypeError> {
        self.types
            .validate_live_value(self.result_type, value, limits)
    }
}

impl<'program> RuntimeProgramDataShapes<'program> {
    /// Resolves a field's declared producer using its exact codec occurrence.
    /// A constant default is an existing nullary pure program with a literal
    /// body; there is no second constant/default catalog.
    pub fn field_default_request(
        &self,
        mut record: ShapeId,
        field: usize,
    ) -> Result<Option<RuntimeDataFieldDefaultRequest<'program>>, RuntimeProgramDataShapeError>
    {
        let mut visited = std::collections::BTreeSet::new();
        loop {
            let semantic_type = self.semantic_type(record).ok_or(
                RuntimeProgramDataShapeError::MissingCoordinate {
                    index: record.index(),
                },
            )?;
            if !visited.insert(record) {
                return Err(RuntimeProgramDataShapeError::PolicyMismatch { semantic_type });
            }
            let occurrence = &self.index()?[record.index()];
            if let Some(error) = &occurrence.error {
                return Err(error.clone());
            }
            match self.policy(occurrence) {
                Some(RuntimeCodecUse::NominalRef) => {
                    record = ShapeId::new(occurrence.row);
                    continue;
                }
                Some(RuntimeCodecUse::Newtype { .. }) => {
                    record = *occurrence
                        .children
                        .first()
                        .ok_or(RuntimeProgramDataShapeError::PolicyMismatch { semantic_type })?;
                    continue;
                }
                _ => {}
            }
            let Some(RuntimeCodecUse::Record { fields, .. }) = self.policy(occurrence) else {
                return Err(RuntimeProgramDataShapeError::PolicyMismatch { semantic_type });
            };
            let policy = fields
                .get(field)
                .ok_or(RuntimeProgramDataShapeError::PolicyMismatch { semantic_type })?;
            let required = policy.has_default || policy.skip;
            let program = match (required, policy.default_program) {
                (false, None) => return Ok(None),
                (true, None) => {
                    return Err(RuntimeDataFieldDefaultError::MissingProducer {
                        record_type: semantic_type,
                        field,
                    }
                    .into());
                }
                (false, Some(_)) => {
                    return Err(RuntimeDataFieldDefaultError::UnexpectedProducer {
                        record_type: semantic_type,
                        field,
                    }
                    .into());
                }
                (true, Some(program)) => program,
            };
            let child = occurrence
                .children
                .get(field)
                .ok_or(RuntimeProgramDataShapeError::PolicyMismatch { semantic_type })?;
            let result_type = self.semantic_type(*child).ok_or(
                RuntimeProgramDataShapeError::MissingCoordinate {
                    index: child.index(),
                },
            )?;
            let invalid = |reason| RuntimeDataFieldDefaultError::InvalidProgram {
                record_type: semantic_type,
                field,
                program,
                reason,
            };
            let signature = match self.types {
                RuntimeProgramTypes::Plan(plan) => {
                    let mut candidates = plan
                        .pure_programs()
                        .iter()
                        .filter(|binding| binding.program() == program);
                    let binding = candidates
                        .next()
                        .ok_or_else(|| invalid("program is absent"))?;
                    if candidates.next().is_some() {
                        return Err(invalid("program identity is ambiguous").into());
                    }
                    let helper = plan
                        .pure_helpers()
                        .iter()
                        .find(|helper| helper.id == binding.helper())
                        .ok_or_else(|| invalid("helper is absent"))?;
                    let actual_result = plan
                        .type_table()
                        .get(helper.expr.ty())
                        .map(|row| row.semantic_identity());
                    if helper.input_locals.len() != binding.input_types().len()
                        || actual_result != Some(binding.result_type())
                    {
                        return Err(invalid("helper signature disagrees with the binding").into());
                    }
                    (binding.input_types().is_empty(), binding.result_type())
                }
                RuntimeProgramTypes::Awbc(awbc) => {
                    let mut candidates = awbc
                        .pure_programs
                        .iter()
                        .filter(|binding| binding.program == program);
                    let binding = candidates
                        .next()
                        .ok_or_else(|| invalid("program is absent"))?;
                    if candidates.next().is_some() {
                        return Err(invalid("program identity is ambiguous").into());
                    }
                    let helper = awbc
                        .pure_helpers
                        .get(binding.helper.index())
                        .ok_or_else(|| invalid("helper is absent"))?;
                    let signature = awbc
                        .signatures
                        .get(helper.signature.index())
                        .ok_or_else(|| invalid("helper signature is absent"))?;
                    let function = awbc
                        .functions
                        .get(helper.function.index())
                        .ok_or_else(|| invalid("helper function is absent"))?;
                    let actual_result = signature
                        .result
                        .and_then(|ty| awbc.runtime_types.get(ty.index()))
                        .map(|row| row.semantic_identity());
                    if function.kind != crate::awbc::schema::AwbcFunctionKind::PureHelper
                        || function.signature != helper.signature
                        || signature.params.len() != binding.input_types.len()
                        || actual_result != Some(binding.result_type)
                    {
                        return Err(invalid("helper signature disagrees with the binding").into());
                    }
                    (binding.input_types.is_empty(), binding.result_type)
                }
            };
            if !signature.0 {
                return Err(invalid("default producer requires inputs").into());
            }
            if signature.1 != result_type {
                return Err(invalid("result does not match the selected field type").into());
            }
            return Ok(Some(RuntimeDataFieldDefaultRequest {
                types: self.types,
                program,
                result_type,
                result_shape: *child,
            }));
        }
    }
}

#[cfg(test)]
mod tests;
