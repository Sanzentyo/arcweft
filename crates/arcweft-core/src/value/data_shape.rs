//! Program-bound witnesses for the closed source type `DataShape<T>`.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::{
    awbc::schema::AwbcProgram,
    entry::RuntimeSchemaLimits,
    pattern::RuntimeSemanticTypeId,
    plan::RuntimePlan,
    program_types::{RuntimeProgramTypeError, RuntimeProgramTypes},
    task::RuntimeProgramOwner,
    value::RuntimeValue,
};

/// A witness that retains one exact executable and its `DataShape<T>` row.
///
/// The child `T`, including nominal identities and ordered generic arguments,
/// is always read through that row. Cloning a witness retains its original
/// program even when the host selects a newer executable generation.
#[derive(Clone)]
pub struct RuntimeDataShape {
    owner: RuntimeProgramOwner,
    shape_type: RuntimeSemanticTypeId,
    value_row: ValueRow,
}

#[derive(Clone, Copy)]
enum ValueRow {
    Plan(crate::runtime_id::RuntimePlanTypeId),
    Awbc(crate::awbc::schema::AwbcTypeId),
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeDataShapeError {
    #[error(transparent)]
    ProgramType(#[from] RuntimeProgramTypeError),
    #[error("DataShape witness belongs to a different executable")]
    ProgramMismatch,
    #[error("DataShape witness has type {actual:?}, expected {expected:?}")]
    ShapeType {
        expected: RuntimeSemanticTypeId,
        actual: RuntimeSemanticTypeId,
    },
    #[error("DataShape source type is {actual:?}, expected {expected:?}")]
    ValueType {
        expected: RuntimeSemanticTypeId,
        actual: RuntimeSemanticTypeId,
    },
}

impl RuntimeDataShape {
    /// Binds a witness to an existing parameterized row in the selected program.
    pub fn bind(
        owner: RuntimeProgramOwner,
        shape_type: RuntimeSemanticTypeId,
    ) -> Result<Self, RuntimeDataShapeError> {
        owner.types().data_shape_value_type(shape_type)?;
        let value_row = match &owner {
            RuntimeProgramOwner::Plan(plan) => {
                let row = plan
                    .type_table()
                    .id_for_semantic(shape_type)
                    .and_then(|ty| plan.type_table().get(ty))
                    .expect("binding resolved the exact shape row");
                let crate::plan::RuntimePlanTypeProjection::Agent(
                    crate::plan::RuntimeAgentTypeProjection::DataShape(child),
                ) = row.projection()
                else {
                    unreachable!("binding checked the shape row family")
                };
                ValueRow::Plan(*child)
            }
            RuntimeProgramOwner::Awbc(program) => {
                let row = program
                    .runtime_types
                    .iter()
                    .find(|row| row.semantic_identity() == shape_type)
                    .expect("binding resolved the unique shape row");
                let crate::awbc::schema::AwbcRuntimeTypeShape::Agent(
                    crate::awbc::schema::AwbcAgentTypeShape::DataShape(child),
                ) = row.shape()
                else {
                    unreachable!("binding checked the shape row family")
                };
                ValueRow::Awbc(*child)
            }
        };
        Ok(Self {
            owner,
            shape_type,
            value_row,
        })
    }

    #[must_use]
    pub const fn program_owner(&self) -> &RuntimeProgramOwner {
        &self.owner
    }

    /// Semantic identity of the complete `DataShape<T>` application.
    #[must_use]
    pub const fn shape_type(&self) -> RuntimeSemanticTypeId {
        self.shape_type
    }

    /// Semantic identity of `T`, obtained from the retained executable row.
    #[must_use]
    pub fn value_type(&self) -> RuntimeSemanticTypeId {
        match (&self.owner, self.value_row) {
            (RuntimeProgramOwner::Plan(plan), ValueRow::Plan(ty)) => plan
                .type_table()
                .get(ty)
                .expect("the retained immutable plan admitted this child")
                .semantic_identity(),
            (RuntimeProgramOwner::Awbc(program), ValueRow::Awbc(ty)) => {
                program.runtime_types[ty.index()].semantic_identity()
            }
            _ => unreachable!("the private child coordinate is issued by the retained owner"),
        }
    }

    /// Checks the exact executable and source/result identities at a typed call.
    pub fn validate_for(
        &self,
        owner: &RuntimeProgramOwner,
        shape_type: RuntimeSemanticTypeId,
        value_type: RuntimeSemanticTypeId,
    ) -> Result<(), RuntimeDataShapeError> {
        if !self.owner.same_program(owner) {
            return Err(RuntimeDataShapeError::ProgramMismatch);
        }
        if self.shape_type != shape_type {
            return Err(RuntimeDataShapeError::ShapeType {
                expected: shape_type,
                actual: self.shape_type,
            });
        }
        let actual = self.value_type();
        if actual != value_type {
            return Err(RuntimeDataShapeError::ValueType {
                expected: value_type,
                actual,
            });
        }
        Ok(())
    }

    /// Admits a value as the exact source type retained by this witness.
    pub fn validate_value(
        &self,
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<(), RuntimeDataShapeError> {
        self.owner
            .types()
            .validate_live_value(self.value_type(), value, limits)?;
        Ok(())
    }

    pub(crate) fn matches_plan(
        &self,
        plan: &RuntimePlan,
        shape_type: RuntimeSemanticTypeId,
    ) -> bool {
        matches!(&self.owner, RuntimeProgramOwner::Plan(owner) if std::ptr::eq(owner.as_ref(), plan))
            && self.shape_type == shape_type
    }

    pub(crate) fn matches_awbc(
        &self,
        program: &AwbcProgram,
        shape_type: RuntimeSemanticTypeId,
    ) -> bool {
        matches!(&self.owner, RuntimeProgramOwner::Awbc(owner) if std::ptr::eq(owner.as_ref(), program))
            && self.shape_type == shape_type
    }

    /// Borrows the sole type authority used for codec projection and admission.
    #[must_use]
    pub fn program_types(&self) -> RuntimeProgramTypes<'_> {
        self.owner.types()
    }
}

impl PartialEq for RuntimeDataShape {
    fn eq(&self, other: &Self) -> bool {
        self.shape_type == other.shape_type && self.owner.same_program(&other.owner)
    }
}

impl Eq for RuntimeDataShape {}

impl fmt::Debug for RuntimeDataShape {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeDataShape")
            .field("shape_type", &self.shape_type)
            .field("value_type", &self.value_type())
            .finish_non_exhaustive()
    }
}

impl Serialize for RuntimeDataShape {
    fn serialize<S: Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom(
            "DataShape witnesses require a program-bound snapshot; generic persistence cannot retain their executable owner",
        ))
    }
}

impl<'de> Deserialize<'de> for RuntimeDataShape {
    fn deserialize<D: Deserializer<'de>>(_deserializer: D) -> Result<Self, D::Error> {
        Err(serde::de::Error::custom(
            "DataShape witnesses require program-bound restore; a wire value cannot supply an executable owner",
        ))
    }
}

#[cfg(test)]
mod tests;
