//! Borrowed type authority for producers serving native and AWBC programs.
//!
//! A context selects one existing program. It owns no type table, schema,
//! generation token, or fallback and does not grant publication authority.

use thiserror::Error;

use crate::{
    awbc::{
        schema::{AwbcProgram, AwbcRuntimeTypeShape, AwbcTypeId, AwbcVariantIdentity},
        type_projection::AwbcTypeProjectionError,
        value_admission::AwbcValueAdmissionError,
    },
    entry::{RuntimeNominalTypeId, RuntimeSchemaLimits, RuntimeValueDigest, TypeLayoutHash},
    pattern::{RuntimeBuiltinVariantIdentity, RuntimeCheckedType, RuntimeSemanticTypeId},
    plan::{
        RuntimePlan, RuntimePlanTypeProjection, RuntimePlanTypeResolutionError,
        RuntimePlanValueAdmissionError,
    },
    value::RuntimeValue,
};

mod construction;
mod data_shapes;
pub use construction::RuntimeProgramValueConstructionError;
pub use data_shapes::{
    RuntimeDataFieldDefaultError, RuntimeDataFieldDefaultRequest, RuntimeProgramDataShapeError,
    RuntimeProgramDataShapes,
};

/// One active executable type authority borrowed for producer-side admission.
#[derive(Clone, Copy, Debug)]
pub enum RuntimeProgramTypes<'a> {
    Plan(&'a RuntimePlan),
    Awbc(&'a AwbcProgram),
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeProgramTypeError {
    #[error("active program has no semantic type {semantic_type:?}")]
    Missing {
        semantic_type: RuntimeSemanticTypeId,
    },
    #[error("active program repeats semantic type {semantic_type:?}")]
    Ambiguous {
        semantic_type: RuntimeSemanticTypeId,
    },
    #[error("active program type index {index} exceeds the executable ID domain")]
    TypeIndexOverflow { index: usize },
    #[error("semantic type {semantic_type:?} has no finite checked predicate")]
    NoCheckedPredicate {
        semantic_type: RuntimeSemanticTypeId,
    },
    #[error("active semantic type {semantic_type:?} is not a parameterized DataShape")]
    NotDataShape {
        semantic_type: RuntimeSemanticTypeId,
    },
    #[error("active semantic type {semantic_type:?} has no structural nominal body")]
    NotNominal {
        semantic_type: RuntimeSemanticTypeId,
    },
    #[error(
        "nominal role {semantic_type:?} expects identity {expected:?}, program declares {actual:?}"
    )]
    NominalIdentity {
        semantic_type: RuntimeSemanticTypeId,
        expected: RuntimeNominalTypeId,
        actual: RuntimeNominalTypeId,
    },
    #[error(
        "nominal role {semantic_type:?} expects layout {expected:?}, program declares {actual:?}"
    )]
    NominalLayout {
        semantic_type: RuntimeSemanticTypeId,
        expected: TypeLayoutHash,
        actual: TypeLayoutHash,
    },
    #[error(transparent)]
    PlanType(Box<RuntimePlanTypeResolutionError>),
    #[error(transparent)]
    AwbcType(Box<AwbcTypeProjectionError>),
    #[error(transparent)]
    PlanValue(Box<RuntimePlanValueAdmissionError>),
    #[error(transparent)]
    AwbcValue(Box<AwbcValueAdmissionError>),
}

impl From<RuntimePlanTypeResolutionError> for RuntimeProgramTypeError {
    fn from(error: RuntimePlanTypeResolutionError) -> Self {
        Self::PlanType(Box::new(error))
    }
}
impl From<AwbcTypeProjectionError> for RuntimeProgramTypeError {
    fn from(error: AwbcTypeProjectionError) -> Self {
        Self::AwbcType(Box::new(error))
    }
}
impl From<RuntimePlanValueAdmissionError> for RuntimeProgramTypeError {
    fn from(error: RuntimePlanValueAdmissionError) -> Self {
        Self::PlanValue(Box::new(error))
    }
}
impl From<AwbcValueAdmissionError> for RuntimeProgramTypeError {
    fn from(error: AwbcValueAdmissionError) -> Self {
        Self::AwbcValue(Box::new(error))
    }
}

impl RuntimeProgramTypes<'_> {
    /// Resolves the exact source type retained by a `DataShape<T>` row.
    /// The child is read from the selected executable; it is never inferred
    /// from a value or reconstructed from a diagnostic checked projection.
    pub fn data_shape_value_type(
        &self,
        semantic_type: RuntimeSemanticTypeId,
    ) -> Result<RuntimeSemanticTypeId, RuntimeProgramTypeError> {
        self.require_type(semantic_type)?;
        match self {
            Self::Plan(plan) => {
                let ty = plan
                    .type_table()
                    .id_for_semantic(semantic_type)
                    .ok_or(RuntimeProgramTypeError::Missing { semantic_type })?;
                let declaration = plan
                    .type_table()
                    .get(ty)
                    .expect("semantic lookup resolves an admitted declaration");
                let RuntimePlanTypeProjection::Agent(
                    crate::plan::RuntimeAgentTypeProjection::DataShape(child),
                ) = declaration.projection()
                else {
                    return Err(RuntimeProgramTypeError::NotDataShape { semantic_type });
                };
                plan.type_table()
                    .get(*child)
                    .map(crate::plan::RuntimePlanTypeDeclaration::semantic_identity)
                    .ok_or_else(|| {
                        RuntimePlanTypeResolutionError::UnknownType { ty: *child }.into()
                    })
            }
            Self::Awbc(program) => {
                let ty = self.awbc_type(semantic_type)?;
                let AwbcRuntimeTypeShape::Agent(
                    crate::awbc::schema::AwbcAgentTypeShape::DataShape(child),
                ) = program.runtime_types[ty.index()].shape()
                else {
                    return Err(RuntimeProgramTypeError::NotDataShape { semantic_type });
                };
                program
                    .runtime_types
                    .get(child.index())
                    .map(crate::awbc::schema::AwbcRuntimeType::semantic_identity)
                    .ok_or_else(|| {
                        AwbcTypeProjectionError::RuntimeTypeOutOfBounds { index: child.0 }.into()
                    })
            }
        }
    }

    /// Checks every descendant of a live value through the selected program.
    /// Finite checked predicates remain diagnostic views, not admission proof.
    pub fn validate_live_value(
        &self,
        semantic_type: RuntimeSemanticTypeId,
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<(), RuntimeProgramTypeError> {
        match self {
            Self::Plan(plan) => {
                let ty = plan
                    .type_table()
                    .id_for_semantic(semantic_type)
                    .ok_or(RuntimeProgramTypeError::Missing { semantic_type })?;
                Ok(plan.validate_live_value(ty, value, limits)?)
            }
            Self::Awbc(program) => {
                Ok(program.validate_live_value(self.awbc_type(semantic_type)?, value, limits)?)
            }
        }
    }

    /// Checks a restored private value against the active program before swap.
    pub fn validate_snapshot_value(
        &self,
        semantic_type: RuntimeSemanticTypeId,
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<(), RuntimeProgramTypeError> {
        match self {
            Self::Plan(plan) => {
                let ty = plan
                    .type_table()
                    .id_for_semantic(semantic_type)
                    .ok_or(RuntimeProgramTypeError::Missing { semantic_type })?;
                Ok(plan.validate_snapshot_value(ty, value, limits)?)
            }
            Self::Awbc(program) => Ok(program.validate_snapshot_value(
                self.awbc_type(semantic_type)?,
                value,
                limits,
            )?),
        }
    }

    /// Correlates nominal metadata with the original program row without
    /// expanding a recursive nominal body into a finite checked predicate.
    pub(crate) fn require_nominal(
        &self,
        semantic_type: RuntimeSemanticTypeId,
        expected_nominal: &RuntimeNominalTypeId,
        expected_layout: TypeLayoutHash,
    ) -> Result<(), RuntimeProgramTypeError> {
        let (nominal, layout) = match self {
            Self::Plan(plan) => {
                let ty = plan
                    .type_table()
                    .id_for_semantic(semantic_type)
                    .ok_or(RuntimeProgramTypeError::Missing { semantic_type })?;
                let declaration = plan
                    .type_table()
                    .get(ty)
                    .expect("semantic lookup returns a registered row");
                let RuntimePlanTypeProjection::Nominal {
                    nominal, layout, ..
                } = declaration.projection()
                else {
                    return Err(RuntimeProgramTypeError::NotNominal { semantic_type });
                };
                if plan.nominal_record_domains().get(ty).is_none()
                    && plan.variant_domains().get(ty).is_none()
                {
                    return Err(RuntimeProgramTypeError::NotNominal { semantic_type });
                }
                (nominal.clone(), *layout)
            }
            Self::Awbc(program) => {
                let ty = self.awbc_type(semantic_type)?;
                let (public_id, layout) = match program.runtime_types[ty.index()].shape() {
                    AwbcRuntimeTypeShape::NominalRecord {
                        public_id, layout, ..
                    }
                    | AwbcRuntimeTypeShape::Variant {
                        owner: AwbcVariantIdentity::Nominal { public_id, layout },
                        ..
                    } => (*public_id, *layout),
                    _ => return Err(RuntimeProgramTypeError::NotNominal { semantic_type }),
                };
                let text = program.strings.get(public_id.index()).ok_or(
                    AwbcTypeProjectionError::StringOutOfBounds {
                        index: public_id.0,
                        role: "nominal role identity",
                    },
                )?;
                let nominal = RuntimeNominalTypeId::try_new(text.clone()).map_err(|source| {
                    AwbcTypeProjectionError::InvalidNominalIdentity {
                        index: public_id.0,
                        source,
                    }
                })?;
                (nominal, TypeLayoutHash::from_bytes(layout))
            }
        };
        if &nominal != expected_nominal {
            return Err(RuntimeProgramTypeError::NominalIdentity {
                semantic_type,
                expected: expected_nominal.clone(),
                actual: nominal,
            });
        }
        if layout != expected_layout {
            return Err(RuntimeProgramTypeError::NominalLayout {
                semantic_type,
                expected: expected_layout,
                actual: layout,
            });
        }
        Ok(())
    }

    /// Resolves a source identity without expanding a possibly recursive type.
    pub fn require_type(
        &self,
        semantic_type: RuntimeSemanticTypeId,
    ) -> Result<(), RuntimeProgramTypeError> {
        match self {
            Self::Plan(plan) => {
                plan.type_table()
                    .id_for_semantic(semantic_type)
                    .ok_or(RuntimeProgramTypeError::Missing { semantic_type })?;
            }
            Self::Awbc(program) => {
                self.awbc_type(semantic_type)?;
                program.validate_type_graph()?;
            }
        }
        Ok(())
    }

    /// Checks only the selected root row's shape, without projecting children.
    pub fn is_result_type(
        &self,
        semantic_type: RuntimeSemanticTypeId,
    ) -> Result<bool, RuntimeProgramTypeError> {
        match self {
            Self::Plan(plan) => {
                let ty = plan
                    .type_table()
                    .id_for_semantic(semantic_type)
                    .ok_or(RuntimeProgramTypeError::Missing { semantic_type })?;
                let declaration = plan.type_table().get(ty).ok_or_else(|| {
                    RuntimeProgramTypeError::PlanType(Box::new(
                        RuntimePlanTypeResolutionError::UnknownType { ty },
                    ))
                })?;
                Ok(matches!(
                    declaration.projection(),
                    RuntimePlanTypeProjection::Result { .. }
                ))
            }
            Self::Awbc(program) => {
                let ty = self.awbc_type(semantic_type)?;
                let row = program.runtime_types.get(ty.index()).ok_or_else(|| {
                    RuntimeProgramTypeError::AwbcType(Box::new(
                        AwbcTypeProjectionError::RuntimeTypeOutOfBounds { index: ty.0 },
                    ))
                })?;
                Ok(matches!(
                    row.shape(),
                    AwbcRuntimeTypeShape::Variant {
                        owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Result),
                        ..
                    }
                ))
            }
        }
    }

    /// Reads the source identities of the selected Result's Ok and Err items.
    /// The canonical one-item case tuples are retained in executable storage.
    pub fn result_types(
        &self,
        semantic_type: RuntimeSemanticTypeId,
    ) -> Result<Option<(RuntimeSemanticTypeId, RuntimeSemanticTypeId)>, RuntimeProgramTypeError>
    {
        self.require_type(semantic_type)?;
        match self {
            Self::Plan(plan) => {
                let ty = plan
                    .type_table()
                    .id_for_semantic(semantic_type)
                    .ok_or(RuntimeProgramTypeError::Missing { semantic_type })?;
                let row = plan
                    .type_table()
                    .get(ty)
                    .expect("semantic lookup returns an admitted row");
                let RuntimePlanTypeProjection::Result { value, error, .. } = row.projection()
                else {
                    return Ok(None);
                };
                let child = |ty| {
                    plan.type_table()
                        .get(ty)
                        .map(crate::plan::RuntimePlanTypeDeclaration::semantic_identity)
                        .ok_or_else(|| {
                            RuntimeProgramTypeError::from(
                                RuntimePlanTypeResolutionError::UnknownType { ty },
                            )
                        })
                };
                Ok(Some((child(*value)?, child(*error)?)))
            }
            Self::Awbc(program) => {
                let ty = self.awbc_type(semantic_type)?;
                let AwbcRuntimeTypeShape::Variant {
                    owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Result),
                    arguments,
                    cases,
                } = program.runtime_types[ty.index()].shape()
                else {
                    return Ok(None);
                };
                let invalid = || {
                    RuntimeProgramTypeError::from(AwbcTypeProjectionError::InvalidBuiltinVariant {
                        index: ty.0,
                    })
                };
                if !arguments.is_empty() || cases.len() != 2 {
                    return Err(invalid());
                }
                let child = |ordinal: usize, expected: &str| {
                    let case = &cases[ordinal];
                    if program.strings.get(case.name.index()).map(String::as_str) != Some(expected)
                    {
                        return Err(invalid());
                    }
                    let payload = case.payload.ok_or_else(invalid)?;
                    let payload = program
                        .runtime_types
                        .get(payload.index())
                        .ok_or_else(invalid)?;
                    let AwbcRuntimeTypeShape::Tuple(items) = payload.shape() else {
                        return Err(invalid());
                    };
                    let [item] = items.as_slice() else {
                        return Err(invalid());
                    };
                    program
                        .runtime_types
                        .get(item.index())
                        .map(crate::awbc::schema::AwbcRuntimeType::semantic_identity)
                        .ok_or_else(invalid)
                };
                Ok(Some((child(0, "Ok")?, child(1, "Err")?)))
            }
        }
    }

    /// Projects only a Result row's Err child as a finite diagnostic type.
    /// Nominal and opaque children retain the exact owner supplied by this
    /// program, while recursive structure outside the Err child is not walked.
    pub fn result_error_checked(
        &self,
        semantic_type: RuntimeSemanticTypeId,
    ) -> Result<Option<RuntimeCheckedType>, RuntimeProgramTypeError> {
        self.result_types(semantic_type)?
            .map(|(_, error)| self.checked_type(error))
            .transpose()
    }
    /// Projects a finite diagnostic predicate, for example an exact opaque role.
    /// Recursive value validation uses `accepts_value` directly instead.
    pub fn checked_type(
        &self,
        semantic_type: RuntimeSemanticTypeId,
    ) -> Result<RuntimeCheckedType, RuntimeProgramTypeError> {
        match self {
            Self::Plan(plan) => {
                let ty = plan
                    .type_table()
                    .id_for_semantic(semantic_type)
                    .ok_or(RuntimeProgramTypeError::Missing { semantic_type })?;
                plan.checked_type(ty)?
                    .ok_or(RuntimeProgramTypeError::NoCheckedPredicate { semantic_type })
            }
            Self::Awbc(program) => Ok(program.checked_type(self.awbc_type(semantic_type)?)?),
        }
    }

    /// Admits a persistent value against the selected program's own tables.
    pub fn accepts_value(
        &self,
        semantic_type: RuntimeSemanticTypeId,
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<RuntimeValueDigest, RuntimeProgramTypeError> {
        match self {
            Self::Plan(plan) => {
                let ty = plan
                    .type_table()
                    .id_for_semantic(semantic_type)
                    .ok_or(RuntimeProgramTypeError::Missing { semantic_type })?;
                Ok(plan.accepts_value(ty, value, limits)?)
            }
            Self::Awbc(program) => {
                Ok(program.accepts_value(self.awbc_type(semantic_type)?, value, limits)?)
            }
        }
    }

    fn awbc_type(
        &self,
        semantic_type: RuntimeSemanticTypeId,
    ) -> Result<AwbcTypeId, RuntimeProgramTypeError> {
        let Self::Awbc(program) = self else {
            unreachable!("AWBC lookup uses the selected AWBC context")
        };
        let mut rows = program
            .runtime_types
            .iter()
            .enumerate()
            .filter(|(_, row)| row.semantic_identity() == semantic_type);
        let (index, _) = rows
            .next()
            .ok_or(RuntimeProgramTypeError::Missing { semantic_type })?;
        if rows.next().is_some() {
            return Err(RuntimeProgramTypeError::Ambiguous { semantic_type });
        }
        u32::try_from(index)
            .map(AwbcTypeId)
            .map_err(|_| RuntimeProgramTypeError::TypeIndexOverflow { index })
    }
}

#[cfg(test)]
mod tests;
