//! AWBC's checked callable-specialization relation.

use super::{AwbcVerifyBudget, AwbcVerifyError};
use crate::awbc::schema::{
    AwbcAgentTypeShape, AwbcProgram, AwbcRuntimeTypeShape, AwbcTypeId, AwbcVariantIdentity,
};
use crate::entry::{RuntimeNominalTypeId, RuntimeSchemaLimits, TypeLayoutHash};
use crate::plan::{
    RuntimeAgentTypeProjection, RuntimeCallableSpecializationContext,
    RuntimeCallableSpecializationError, RuntimePlanRecordField, RuntimePlanTypeProjection,
};
use crate::runtime_id::RuntimeCallableStateId;

pub(super) fn verify(
    program: &AwbcProgram,
    budget: AwbcVerifyBudget,
) -> Result<(), AwbcVerifyError> {
    let count = program.callable_specializations.len();
    if count > budget.callable_specializations {
        return Err(AwbcVerifyError::BudgetExceeded {
            budget: "callable_specializations",
        });
    }
    if count == 0 {
        return Ok(());
    }

    let mut remaining_work = budget.specialization_validation_work;
    for (index, specialization) in program.callable_specializations.iter().enumerate() {
        if remaining_work == 0 {
            return Err(AwbcVerifyError::BudgetExceeded {
                budget: "specialization_validation_work",
            });
        }
        let maximum_work =
            remaining_work.min(RuntimeSchemaLimits::engine_default().max_validation_work);
        let used = specialization
            .validate_counted(program, maximum_work)
            .map_err(|error| match error {
                RuntimeCallableSpecializationError::WorkLimit => AwbcVerifyError::BudgetExceeded {
                    budget: "specialization_validation_work",
                },
                error => AwbcVerifyError::InvalidInvariant {
                    at: format!("callable specialization {index}"),
                    message: error.to_string(),
                },
            })?;
        remaining_work -= used;
    }
    Ok(())
}

impl RuntimeCallableSpecializationContext for AwbcProgram {
    type Type = AwbcTypeId;
    type Function = crate::awbc::schema::AwbcFunctionId;

    fn type_scope(&self, ty: Self::Type) -> Option<&crate::plan::RuntimeTypeScope> {
        self.runtime_types
            .get(ty.index())
            .map(crate::awbc::schema::AwbcRuntimeType::scope)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "AWBC and RuntimePlan type algebras need one exhaustive projection boundary."
    )]
    fn type_projection(&self, ty: Self::Type) -> Option<RuntimePlanTypeProjection<Self::Type>> {
        let row = self.runtime_types.get(ty.index())?;
        let projection = match row.shape() {
            AwbcRuntimeTypeShape::BoundType(reference) => {
                RuntimePlanTypeProjection::BoundType(*reference)
            }
            AwbcRuntimeTypeShape::Never => RuntimePlanTypeProjection::Never,
            AwbcRuntimeTypeShape::Unit => RuntimePlanTypeProjection::Unit,
            AwbcRuntimeTypeShape::Bool => RuntimePlanTypeProjection::Bool,
            AwbcRuntimeTypeShape::Int(kind) => RuntimePlanTypeProjection::Signed((*kind).into()),
            AwbcRuntimeTypeShape::UInt(kind) => RuntimePlanTypeProjection::Unsigned((*kind).into()),
            AwbcRuntimeTypeShape::F32 => RuntimePlanTypeProjection::F32,
            AwbcRuntimeTypeShape::F64 => RuntimePlanTypeProjection::F64,
            AwbcRuntimeTypeShape::String => RuntimePlanTypeProjection::String,
            AwbcRuntimeTypeShape::Char => RuntimePlanTypeProjection::Char,
            AwbcRuntimeTypeShape::Bytes => RuntimePlanTypeProjection::Bytes,
            AwbcRuntimeTypeShape::Duration => RuntimePlanTypeProjection::Duration,
            AwbcRuntimeTypeShape::Progress => RuntimePlanTypeProjection::Progress,
            AwbcRuntimeTypeShape::EntityRef => RuntimePlanTypeProjection::EntityReference,
            AwbcRuntimeTypeShape::AgentValue => RuntimePlanTypeProjection::AgentValue,
            AwbcRuntimeTypeShape::Range(item) => RuntimePlanTypeProjection::Range(*item),
            AwbcRuntimeTypeShape::Iterator(item) => RuntimePlanTypeProjection::Iterator(*item),
            AwbcRuntimeTypeShape::Sequence { kind, item } => RuntimePlanTypeProjection::Sequence {
                kind: *kind,
                item: *item,
            },
            AwbcRuntimeTypeShape::Array { item, length } => RuntimePlanTypeProjection::Array {
                item: *item,
                length: *length,
            },
            AwbcRuntimeTypeShape::Map { kind, key, value } => RuntimePlanTypeProjection::Map {
                kind: *kind,
                key: *key,
                value: *value,
            },
            AwbcRuntimeTypeShape::Need(item) => RuntimePlanTypeProjection::Need(*item),
            AwbcRuntimeTypeShape::Task(item) => RuntimePlanTypeProjection::ThreadHandle(*item),
            AwbcRuntimeTypeShape::Stream { item, error } => RuntimePlanTypeProjection::Stream {
                item: *item,
                error: *error,
            },
            AwbcRuntimeTypeShape::Shared(item) => RuntimePlanTypeProjection::Shared(*item),
            AwbcRuntimeTypeShape::Reference(item) => RuntimePlanTypeProjection::Reference(*item),
            AwbcRuntimeTypeShape::Function {
                contract,
                parameters,
                result,
            } => RuntimePlanTypeProjection::Function {
                contract: contract.clone(),
                parameters: parameters.clone().into_boxed_slice(),
                result: *result,
            },
            AwbcRuntimeTypeShape::Tuple(items) => {
                RuntimePlanTypeProjection::Tuple(items.clone().into_boxed_slice())
            }
            AwbcRuntimeTypeShape::Record { fields, .. } => {
                let fields = fields
                    .iter()
                    .map(|field| {
                        Some(RuntimePlanRecordField::new(
                            self.strings.get(field.name?.index())?.clone(),
                            field.ty,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?;
                RuntimePlanTypeProjection::Record(fields.into_boxed_slice())
            }
            AwbcRuntimeTypeShape::Choice(alternatives) => {
                RuntimePlanTypeProjection::Choice(alternatives.clone().into_boxed_slice())
            }
            AwbcRuntimeTypeShape::Nominal {
                public_id,
                layout,
                arguments,
            }
            | AwbcRuntimeTypeShape::NominalRecord {
                public_id,
                layout,
                arguments,
                ..
            }
            | AwbcRuntimeTypeShape::Variant {
                owner: AwbcVariantIdentity::Nominal { public_id, layout },
                arguments,
                ..
            } => RuntimePlanTypeProjection::Nominal {
                nominal: RuntimeNominalTypeId::try_new(
                    self.strings.get(public_id.index())?.clone(),
                )
                .ok()?,
                layout: TypeLayoutHash::from_bytes(*layout),
                arguments: arguments.clone().into_boxed_slice(),
            },
            AwbcRuntimeTypeShape::Variant {
                owner: AwbcVariantIdentity::Builtin(owner),
                cases,
                ..
            } => RuntimePlanTypeProjection::BuiltinVariant {
                owner: *owner,
                cases: cases
                    .iter()
                    .map(|case| case.payload)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            },
            AwbcRuntimeTypeShape::Opaque {
                admission,
                value_class,
                persistence,
                arguments,
                ..
            } => {
                let owner = self.opaque_owner(ty).ok()??;
                RuntimePlanTypeProjection::Opaque {
                    producer: owner.producer().clone(),
                    admission: *admission,
                    value_class: *value_class,
                    persistence: *persistence,
                    arguments: arguments.clone().into_boxed_slice(),
                }
            }
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(kind)) => {
                RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::try_leaf(*kind)?)
            }
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Probe(item)) => {
                RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::Probe(*item))
            }
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::DataShape(item)) => {
                RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::DataShape(*item))
            }
            // These backend-only or dynamically typed rows have no faithful
            // source runtime-plan projection. They cannot authorize a type
            // substitution in the shared callable contract.
            AwbcRuntimeTypeShape::MatrixF32
            | AwbcRuntimeTypeShape::MatrixF64
            | AwbcRuntimeTypeShape::TensorF32
            | AwbcRuntimeTypeShape::TensorF64
            | AwbcRuntimeTypeShape::Dynamic => return None,
        };
        Some(projection)
    }

    fn callable_state(
        &self,
        id: RuntimeCallableStateId,
    ) -> Option<&crate::plan::RuntimeCallableStateDefinition<Self::Type, Self::Function>> {
        self.callable_states.get(id.index())
    }
}
