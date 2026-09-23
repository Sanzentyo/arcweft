//! Value construction from the selected executable's original descriptors.

use thiserror::Error;

use super::{RuntimeProgramTypeError, RuntimeProgramTypes};
use crate::{
    awbc::{
        schema::{AwbcRuntimeTypeShape, AwbcVariantIdentity},
        type_projection::AwbcTypeProjectionError,
    },
    entry::{RuntimeNominalTypeId, RuntimeSchemaLimits, TypeLayoutHash},
    pattern::{RuntimeSemanticTypeId, RuntimeVariantIdentity},
    plan::{RuntimePlanTypeProjection, RuntimePlanVariantCaseError},
    value::{
        RuntimeFieldValue, RuntimeNominalRecordValue, RuntimeRecordAdmissionError,
        RuntimeRecordValue, RuntimeValue,
    },
};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeProgramValueConstructionError {
    #[error(transparent)]
    ProgramType(#[from] RuntimeProgramTypeError),
    #[error(transparent)]
    PlanCase(#[from] RuntimePlanVariantCaseError),
    #[error(transparent)]
    Record(#[from] RuntimeRecordAdmissionError),
    #[error("selected type {semantic_type:?} is not a {expected}")]
    TypeShape {
        semantic_type: RuntimeSemanticTypeId,
        expected: &'static str,
    },
    #[error("selected record {semantic_type:?} requires {expected} fields, received {actual}")]
    FieldCount {
        semantic_type: RuntimeSemanticTypeId,
        expected: usize,
        actual: usize,
    },
    #[error("selected variant {semantic_type:?} has no case {ordinal}")]
    UnknownCase {
        semantic_type: RuntimeSemanticTypeId,
        ordinal: u32,
    },
}

impl RuntimeProgramTypes<'_> {
    /// Retains the source wrapper form when its codec view is unit, tuple or
    /// transparent newtype. A structural record or nominal variant returns None.
    pub fn nominal_record_shape(
        &self,
        semantic_type: RuntimeSemanticTypeId,
    ) -> Result<Option<crate::entry::RuntimeNominalRecordShape>, RuntimeProgramTypeError> {
        self.require_type(semantic_type)?;
        Ok(match self {
            Self::Plan(plan) => plan
                .type_table()
                .id_for_semantic(semantic_type)
                .and_then(|ty| plan.nominal_record_domains().get(ty))
                .map(|domain| domain.shape()),
            Self::Awbc(program) => {
                match program.runtime_types[self.awbc_type(semantic_type)?.index()].shape() {
                    AwbcRuntimeTypeShape::NominalRecord { shape, .. } => Some(*shape),
                    _ => None,
                }
            }
        })
    }

    /// Constructs a structural or nominal record in declared field order and
    /// admits the complete result before returning it to a producer.
    pub fn try_record_value(
        &self,
        semantic_type: RuntimeSemanticTypeId,
        values: Vec<RuntimeValue>,
        limits: RuntimeSchemaLimits,
    ) -> Result<RuntimeValue, RuntimeProgramValueConstructionError> {
        self.require_type(semantic_type)?;
        let check_count = |expected| {
            if values.len() == expected {
                Ok(())
            } else {
                Err(RuntimeProgramValueConstructionError::FieldCount {
                    semantic_type,
                    expected,
                    actual: values.len(),
                })
            }
        };
        let value = match self {
            Self::Plan(plan) => {
                let ty = plan
                    .type_table()
                    .id_for_semantic(semantic_type)
                    .ok_or(RuntimeProgramTypeError::Missing { semantic_type })?;
                let row = plan
                    .type_table()
                    .get(ty)
                    .expect("semantic lookup resolves an admitted row");
                match row.projection() {
                    RuntimePlanTypeProjection::Nominal {
                        nominal, layout, ..
                    } => {
                        let domain = plan.nominal_record_domains().get(ty).ok_or(
                            RuntimeProgramValueConstructionError::TypeShape {
                                semantic_type,
                                expected: "record",
                            },
                        )?;
                        check_count(domain.fields().len())?;
                        RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
                            nominal.clone(),
                            semantic_type,
                            *layout,
                            values,
                        ))
                    }
                    RuntimePlanTypeProjection::Record(fields) => {
                        check_count(fields.len())?;
                        RuntimeValue::try_record(
                            fields
                                .iter()
                                .zip(values)
                                .map(|(field, value)| (field.diagnostic_name().to_owned(), value))
                                .collect(),
                        )?
                    }
                    _ => {
                        return Err(RuntimeProgramValueConstructionError::TypeShape {
                            semantic_type,
                            expected: "record",
                        });
                    }
                }
            }
            Self::Awbc(program) => {
                let ty = self.awbc_type(semantic_type)?;
                match program.runtime_types[ty.index()].shape() {
                    AwbcRuntimeTypeShape::NominalRecord {
                        public_id,
                        layout,
                        fields,
                        ..
                    } => {
                        check_count(fields.len())?;
                        let text = program.strings.get(public_id.index()).ok_or_else(|| {
                            RuntimeProgramTypeError::from(
                                AwbcTypeProjectionError::StringOutOfBounds {
                                    index: public_id.0,
                                    role: "nominal record identity",
                                },
                            )
                        })?;
                        let nominal =
                            RuntimeNominalTypeId::try_new(text.clone()).map_err(|source| {
                                RuntimeProgramTypeError::from(
                                    AwbcTypeProjectionError::InvalidNominalIdentity {
                                        index: public_id.0,
                                        source,
                                    },
                                )
                            })?;
                        RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
                            nominal,
                            semantic_type,
                            TypeLayoutHash::from_bytes(*layout),
                            values,
                        ))
                    }
                    AwbcRuntimeTypeShape::Record { fields, .. } => {
                        check_count(fields.len())?;
                        let fields = fields
                            .iter()
                            .zip(values)
                            .map(|(field, value)| {
                                let name = field
                                    .name
                                    .and_then(|name| program.strings.get(name.index()))
                                    .ok_or(RuntimeProgramValueConstructionError::TypeShape {
                                        semantic_type,
                                        expected: "named-field record",
                                    })?;
                                Ok(RuntimeFieldValue::new_accepted(
                                    field.field,
                                    name.clone(),
                                    value,
                                ))
                            })
                            .collect::<Result<Vec<_>, RuntimeProgramValueConstructionError>>()?;
                        RuntimeValue::Record(RuntimeRecordValue::try_from_fields(fields)?)
                    }
                    _ => {
                        return Err(RuntimeProgramValueConstructionError::TypeShape {
                            semantic_type,
                            expected: "record",
                        });
                    }
                }
            }
        };
        self.validate_live_value(semantic_type, &value, limits)?;
        Ok(value)
    }

    /// Selects an exact source-ordered case, retains its full nominal owner,
    /// and admits its payload under the selected program before publication.
    pub fn try_variant_value(
        &self,
        semantic_type: RuntimeSemanticTypeId,
        ordinal: u32,
        payload: Option<RuntimeValue>,
        limits: RuntimeSchemaLimits,
    ) -> Result<RuntimeValue, RuntimeProgramValueConstructionError> {
        self.require_type(semantic_type)?;
        let (owner, name) = match self {
            Self::Plan(plan) => {
                let ty = plan
                    .type_table()
                    .id_for_semantic(semantic_type)
                    .ok_or(RuntimeProgramTypeError::Missing { semantic_type })?;
                let case = plan.variant_case(ty, ordinal)?;
                (case.owner().clone(), case.name().to_owned())
            }
            Self::Awbc(program) => {
                let ty = self.awbc_type(semantic_type)?;
                let AwbcRuntimeTypeShape::Variant { owner, cases, .. } =
                    program.runtime_types[ty.index()].shape()
                else {
                    return Err(RuntimeProgramValueConstructionError::TypeShape {
                        semantic_type,
                        expected: "variant",
                    });
                };
                let case = usize::try_from(ordinal)
                    .ok()
                    .and_then(|index| cases.get(index))
                    .ok_or(RuntimeProgramValueConstructionError::UnknownCase {
                        semantic_type,
                        ordinal,
                    })?;
                let string = |id: crate::awbc::schema::AwbcStringId, role| {
                    program.strings.get(id.index()).cloned().ok_or_else(|| {
                        RuntimeProgramTypeError::from(AwbcTypeProjectionError::StringOutOfBounds {
                            index: id.0,
                            role,
                        })
                    })
                };
                let owner = match owner {
                    AwbcVariantIdentity::Builtin(owner) => RuntimeVariantIdentity::Builtin(*owner),
                    AwbcVariantIdentity::Nominal { public_id, layout } => {
                        RuntimeVariantIdentity::Nominal {
                            nominal: RuntimeNominalTypeId::try_new(string(
                                *public_id,
                                "nominal variant identity",
                            )?)
                            .map_err(|source| {
                                RuntimeProgramTypeError::from(
                                    AwbcTypeProjectionError::InvalidNominalIdentity {
                                        index: public_id.0,
                                        source,
                                    },
                                )
                            })?,
                            semantic_identity: semantic_type,
                            layout: TypeLayoutHash::from_bytes(*layout),
                        }
                    }
                };
                (owner, string(case.name, "variant case")?)
            }
        };
        let value = RuntimeValue::Variant {
            owner,
            ordinal,
            name,
            payload: payload.map(Box::new),
        };
        self.validate_live_value(semantic_type, &value, limits)?;
        Ok(value)
    }
}
