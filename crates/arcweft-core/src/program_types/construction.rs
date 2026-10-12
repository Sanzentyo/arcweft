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
    #[error("selected type {semantic_type:?} has an invalid construction type origin")]
    InvalidTypeOrigin {
        semantic_type: RuntimeSemanticTypeId,
    },
    #[error("selected variant {semantic_type:?} case {ordinal} has a different snapshot header")]
    VariantSnapshotHeader {
        semantic_type: RuntimeSemanticTypeId,
        ordinal: u32,
    },
}

/// An ephemeral source-ordered case resolved from the selected executable.
/// It is never a second type catalog or a published runtime carrier.
pub(crate) struct ProgramVariantCase {
    owner: RuntimeVariantIdentity,
    ordinal: u32,
    name: String,
    payload: Option<RuntimeSemanticTypeId>,
}

impl ProgramVariantCase {
    pub(crate) fn into_header(self) -> (RuntimeVariantIdentity, u32, String) {
        (self.owner, self.ordinal, self.name)
    }
}

impl RuntimeProgramTypes<'_> {
    /// Retains the source wrapper form when its codec view is unit, tuple or
    /// transparent newtype. A structural record or nominal variant returns None.
    pub fn nominal_record_shape(
        &self,
        semantic_type: RuntimeSemanticTypeId,
    ) -> Result<Option<crate::entry::RuntimeNominalRecordShape>, RuntimeProgramTypeError> {
        Ok(self
            .nominal_record_metadata(semantic_type)?
            .map(|(shape, _)| shape))
    }

    fn nominal_record_metadata(
        &self,
        semantic_type: RuntimeSemanticTypeId,
    ) -> Result<Option<(crate::entry::RuntimeNominalRecordShape, usize)>, RuntimeProgramTypeError>
    {
        self.require_type(semantic_type)?;
        Ok(match self {
            Self::Plan(plan) => plan
                .type_table()
                .id_for_semantic(semantic_type)
                .and_then(|ty| plan.nominal_record_domains().get(ty))
                .map(|domain| (domain.shape(), domain.fields().len())),
            Self::Awbc(program) => {
                match program.runtime_types[self.awbc_type(semantic_type)?.index()].shape() {
                    AwbcRuntimeTypeShape::NominalRecord { shape, fields, .. } => {
                        Some((*shape, fields.len()))
                    }
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
        let value = self.record_candidate(semantic_type, values)?;
        self.validate_live_value(semantic_type, &value, limits)?;
        Ok(value)
    }

    /// Resolves a nominal snapshot header before any descendant is restored.
    pub(crate) fn require_nominal_record_snapshot_header(
        &self,
        semantic_type: RuntimeSemanticTypeId,
        nominal: &RuntimeNominalTypeId,
        layout: TypeLayoutHash,
        fields: usize,
        binding: Option<&super::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), RuntimeProgramValueConstructionError> {
        self.require_nominal(semantic_type, nominal, layout)?;
        let (_, expected) = self.nominal_record_metadata(semantic_type)?.ok_or(
            RuntimeProgramValueConstructionError::TypeShape {
                semantic_type,
                expected: "nominal record",
            },
        )?;
        if expected != fields {
            return Err(RuntimeProgramValueConstructionError::FieldCount {
                semantic_type,
                expected,
                actual: fields,
            });
        }
        if !self.type_origin_is_valid(semantic_type, binding) {
            return Err(RuntimeProgramValueConstructionError::InvalidTypeOrigin { semantic_type });
        }
        Ok(())
    }

    /// Admits a private restored record using the selected program's original
    /// row and snapshot rules, including persistable affine descendants.
    pub(crate) fn try_snapshot_record_value(
        &self,
        semantic_type: RuntimeSemanticTypeId,
        values: Vec<RuntimeValue>,
        binding: Option<std::sync::Arc<super::RuntimeFunctionEffectInstantiation>>,
        limits: RuntimeSchemaLimits,
    ) -> Result<RuntimeNominalRecordValue, RuntimeProgramValueConstructionError> {
        let RuntimeValue::NominalRecord(record) = self.record_candidate(semantic_type, values)?
        else {
            return Err(RuntimeProgramValueConstructionError::TypeShape {
                semantic_type,
                expected: "nominal record",
            });
        };
        let value = RuntimeValue::NominalRecord(record.with_type_instantiation(binding));
        self.validate_snapshot_value(semantic_type, &value, limits)?;
        let RuntimeValue::NominalRecord(record) = value else {
            unreachable!();
        };
        Ok(record)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one descriptor-derived candidate constructor preserves defining field order and header evidence for native and AWBC record families"
    )]
    fn record_candidate(
        &self,
        semantic_type: RuntimeSemanticTypeId,
        values: Vec<RuntimeValue>,
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
        let case = self.variant_case(semantic_type, ordinal)?;
        crate::value::RuntimeVariantValue::try_from_program_case(case, payload, None, |value| {
            self.validate_live_value(semantic_type, value, limits)
                .map_err(Into::into)
        })
    }

    pub(crate) fn require_variant_snapshot_header(
        &self,
        semantic_type: RuntimeSemanticTypeId,
        owner: &RuntimeVariantIdentity,
        ordinal: u32,
        name: &str,
        has_payload: bool,
        binding: Option<&super::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), RuntimeProgramValueConstructionError> {
        let case = self.variant_case(semantic_type, ordinal)?;
        if case.owner != *owner || case.name != name || case.payload.is_some() != has_payload {
            return Err(
                RuntimeProgramValueConstructionError::VariantSnapshotHeader {
                    semantic_type,
                    ordinal,
                },
            );
        }
        if !self.type_origin_is_valid(semantic_type, binding) {
            return Err(RuntimeProgramValueConstructionError::InvalidTypeOrigin { semantic_type });
        }
        Ok(())
    }

    pub(crate) fn try_snapshot_variant_value(
        &self,
        semantic_type: RuntimeSemanticTypeId,
        ordinal: u32,
        payload: Option<RuntimeValue>,
        binding: Option<std::sync::Arc<super::RuntimeFunctionEffectInstantiation>>,
        limits: RuntimeSchemaLimits,
    ) -> Result<RuntimeValue, RuntimeProgramValueConstructionError> {
        let case = self.variant_case(semantic_type, ordinal)?;
        crate::value::RuntimeVariantValue::try_from_program_case(case, payload, binding, |value| {
            self.validate_snapshot_value(semantic_type, value, limits)
                .map_err(Into::into)
        })
    }

    pub(crate) fn variant_case(
        &self,
        semantic_type: RuntimeSemanticTypeId,
        ordinal: u32,
    ) -> Result<ProgramVariantCase, RuntimeProgramValueConstructionError> {
        self.require_type(semantic_type)?;
        Ok(match self {
            Self::Plan(plan) => {
                let ty = plan
                    .type_table()
                    .id_for_semantic(semantic_type)
                    .ok_or(RuntimeProgramTypeError::Missing { semantic_type })?;
                let case = plan.variant_case(ty, ordinal)?;
                ProgramVariantCase {
                    ordinal,
                    owner: case.owner().clone(),
                    name: case.name().to_owned(),
                    payload: case.payload().map(|ty| {
                        plan.type_table()
                            .get(ty)
                            .expect("admitted variant case payload row")
                            .semantic_identity()
                    }),
                }
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
                ProgramVariantCase {
                    ordinal,
                    owner,
                    name: string(case.name, "variant case")?,
                    payload: case
                        .payload
                        .map(|ty| {
                            program
                                .runtime_types
                                .get(ty.index())
                                .map(crate::awbc::schema::AwbcRuntimeType::semantic_identity)
                                .ok_or_else(|| {
                                    RuntimeProgramTypeError::from(
                                        AwbcTypeProjectionError::RuntimeTypeOutOfBounds {
                                            index: ty.0,
                                        },
                                    )
                                })
                        })
                        .transpose()?,
                }
            }
        })
    }
}
