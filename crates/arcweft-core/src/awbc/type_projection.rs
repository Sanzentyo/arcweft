//! Projection from verified AWBC type rows to native runtime owners.

use super::schema::{
    AwbcProgram, AwbcRuntimeType, AwbcSignedIntKind, AwbcTypeId, AwbcUnsignedIntKind,
    AwbcVariantIdentity,
};
use crate::entry::{RuntimeIdentityError, RuntimeNominalTypeId, TypeLayoutHash};
use crate::pattern::{
    RuntimeCheckedType, RuntimeCheckedVariantCase, RuntimeOpaqueTypeAdmission,
    RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId,
};
use crate::value::{
    RuntimeNominalRecordLayout, RuntimeNominalRecordLayoutError, RuntimeSignedIntWidth,
    RuntimeUnsignedIntWidth,
};
use std::collections::BTreeSet;
use thiserror::Error;

/// Failure to reify an AWBC runtime type as its native checked owner.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AwbcTypeProjectionError {
    #[error("AWBC runtime type index {index} is out of bounds")]
    RuntimeTypeOutOfBounds { index: u32 },
    #[error("AWBC opaque producer string index {index} is out of bounds")]
    ProducerStringOutOfBounds { index: u32 },
    #[error("AWBC opaque producer at string index {index} is invalid: {source}")]
    InvalidOpaqueProducer {
        index: u32,
        source: RuntimeIdentityError,
    },
    #[error("AWBC string index {index} is out of bounds while projecting {role}")]
    StringOutOfBounds { index: u32, role: &'static str },
    #[error("AWBC nominal identity at string index {index} is invalid: {source}")]
    InvalidNominalIdentity {
        index: u32,
        source: RuntimeIdentityError,
    },
    #[error("AWBC runtime type {index} is not in the closed checked-type image")]
    UnsupportedCheckedType { index: u32 },
    #[error("AWBC checked-type graph contains a cycle at runtime type {index}")]
    CheckedTypeCycle { index: u32 },
    #[error("AWBC checked-type graph exceeds the 64-level nesting limit")]
    CheckedTypeDepth,
    #[error("AWBC built-in variant runtime type {index} has an invalid checked shape")]
    InvalidBuiltinVariant { index: u32 },
    #[error("AWBC nominal-record descriptor is invalid: {source}")]
    InvalidNominalRecordLayout {
        source: RuntimeNominalRecordLayoutError,
    },
}

impl AwbcRuntimeType {
    /// Reifies an opaque row through the same native owner used by checked
    /// execution. Non-opaque rows return `None`.
    pub fn try_opaque_owner(
        &self,
        strings: &[String],
    ) -> Result<Option<RuntimeOpaqueTypeOwner>, AwbcTypeProjectionError> {
        let Self::Opaque {
            producer,
            semantic_identity,
            admission,
        } = self
        else {
            return Ok(None);
        };
        let spelling = strings
            .get(producer.index())
            .ok_or(AwbcTypeProjectionError::ProducerStringOutOfBounds { index: producer.0 })?;
        let producer =
            RuntimeOpaqueTypeProducerId::try_new(spelling.clone()).map_err(|source| {
                AwbcTypeProjectionError::InvalidOpaqueProducer {
                    index: producer.0,
                    source,
                }
            })?;
        let semantic_identity = RuntimeSemanticTypeId::from_bytes(*semantic_identity);
        Ok(Some(match admission {
            RuntimeOpaqueTypeAdmission::ExactIdentity => {
                RuntimeOpaqueTypeOwner::exact(producer, semantic_identity)
            }
            RuntimeOpaqueTypeAdmission::ProducerWide => {
                RuntimeOpaqueTypeOwner::producer_wide(producer, semantic_identity)
            }
        }))
    }
}

impl AwbcProgram {
    /// Reifies one indexed opaque row through its canonical string table.
    pub fn opaque_owner(
        &self,
        ty: AwbcTypeId,
    ) -> Result<Option<RuntimeOpaqueTypeOwner>, AwbcTypeProjectionError> {
        self.runtime_types
            .get(ty.index())
            .ok_or(AwbcTypeProjectionError::RuntimeTypeOutOfBounds { index: ty.0 })?
            .try_opaque_owner(&self.strings)
    }

    /// Reifies one executable nominal-record descriptor through its checked
    /// field-type graph. Non-nominal-record rows return `None`.
    pub(crate) fn nominal_record_layout(
        &self,
        ty: AwbcTypeId,
    ) -> Result<Option<RuntimeNominalRecordLayout>, AwbcTypeProjectionError> {
        let row = self
            .runtime_types
            .get(ty.index())
            .ok_or(AwbcTypeProjectionError::RuntimeTypeOutOfBounds { index: ty.0 })?;
        let AwbcRuntimeType::NominalRecord {
            public_id,
            semantic_identity,
            layout,
            fields,
        } = row
        else {
            return Ok(None);
        };
        let nominal = self.nominal_identity(*public_id)?;
        let mut visiting = BTreeSet::new();
        let fields = fields
            .iter()
            .map(|field| {
                let name = self.strings.get(field.name.index()).cloned().ok_or(
                    AwbcTypeProjectionError::StringOutOfBounds {
                        index: field.name.0,
                        role: "nominal record field name",
                    },
                )?;
                self.checked_type(field.ty, 0, &mut visiting)
                    .map(|checked| (name, checked))
            })
            .collect::<Result<Vec<_>, _>>()?;
        RuntimeNominalRecordLayout::try_from_checked_projection(
            nominal,
            RuntimeSemanticTypeId::from_bytes(*semantic_identity),
            TypeLayoutHash::from_bytes(*layout),
            fields,
        )
        .map(Some)
        .map_err(|source| AwbcTypeProjectionError::InvalidNominalRecordLayout { source })
    }

    fn nominal_identity(
        &self,
        public_id: super::schema::AwbcStringId,
    ) -> Result<RuntimeNominalTypeId, AwbcTypeProjectionError> {
        let spelling = self.strings.get(public_id.index()).cloned().ok_or(
            AwbcTypeProjectionError::StringOutOfBounds {
                index: public_id.0,
                role: "nominal identity",
            },
        )?;
        RuntimeNominalTypeId::try_new(spelling).map_err(|source| {
            AwbcTypeProjectionError::InvalidNominalIdentity {
                index: public_id.0,
                source,
            }
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the reverse projection is the exhaustive inverse of the closed checked-type family"
    )]
    fn checked_type(
        &self,
        ty: AwbcTypeId,
        depth: usize,
        visiting: &mut BTreeSet<AwbcTypeId>,
    ) -> Result<RuntimeCheckedType, AwbcTypeProjectionError> {
        if depth > 64 {
            return Err(AwbcTypeProjectionError::CheckedTypeDepth);
        }
        let row = self
            .runtime_types
            .get(ty.index())
            .ok_or(AwbcTypeProjectionError::RuntimeTypeOutOfBounds { index: ty.0 })?;
        if !visiting.insert(ty) {
            return Err(AwbcTypeProjectionError::CheckedTypeCycle { index: ty.0 });
        }
        let result = match row {
            AwbcRuntimeType::Never => Ok(RuntimeCheckedType::Never),
            AwbcRuntimeType::Unit => Ok(RuntimeCheckedType::Unit),
            AwbcRuntimeType::Bool => Ok(RuntimeCheckedType::Bool),
            AwbcRuntimeType::Int(kind) => Ok(RuntimeCheckedType::Signed(match kind {
                AwbcSignedIntKind::I8 => RuntimeSignedIntWidth::I8,
                AwbcSignedIntKind::I16 => RuntimeSignedIntWidth::I16,
                AwbcSignedIntKind::I32 => RuntimeSignedIntWidth::I32,
                AwbcSignedIntKind::I64 => RuntimeSignedIntWidth::I64,
                AwbcSignedIntKind::I128 => RuntimeSignedIntWidth::I128,
                AwbcSignedIntKind::ISize => RuntimeSignedIntWidth::ISize,
            })),
            AwbcRuntimeType::UInt(kind) => Ok(RuntimeCheckedType::Unsigned(match kind {
                AwbcUnsignedIntKind::U8 => RuntimeUnsignedIntWidth::U8,
                AwbcUnsignedIntKind::U16 => RuntimeUnsignedIntWidth::U16,
                AwbcUnsignedIntKind::U32 => RuntimeUnsignedIntWidth::U32,
                AwbcUnsignedIntKind::U64 => RuntimeUnsignedIntWidth::U64,
                AwbcUnsignedIntKind::U128 => RuntimeUnsignedIntWidth::U128,
                AwbcUnsignedIntKind::USize => RuntimeUnsignedIntWidth::USize,
            })),
            AwbcRuntimeType::F32 => Ok(RuntimeCheckedType::F32),
            AwbcRuntimeType::F64 => Ok(RuntimeCheckedType::F64),
            AwbcRuntimeType::String => Ok(RuntimeCheckedType::String),
            AwbcRuntimeType::Char => Ok(RuntimeCheckedType::Char),
            AwbcRuntimeType::Duration => Ok(RuntimeCheckedType::Duration),
            AwbcRuntimeType::EntityRef => Ok(RuntimeCheckedType::EntityReference),
            AwbcRuntimeType::Bytes => Ok(RuntimeCheckedType::Bytes),
            AwbcRuntimeType::Sequence(item) => self
                .checked_type(*item, depth + 1, visiting)
                .map(Box::new)
                .map(RuntimeCheckedType::Sequence),
            AwbcRuntimeType::Tuple(items) => items
                .iter()
                .map(|item| self.checked_type(*item, depth + 1, visiting))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeCheckedType::Tuple),
            AwbcRuntimeType::Choice(alternatives) => alternatives
                .iter()
                .map(|item| self.checked_type(*item, depth + 1, visiting))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeCheckedType::Choice),
            AwbcRuntimeType::Nominal {
                public_id,
                semantic_identity,
                layout,
            } => Ok(RuntimeCheckedType::Nominal {
                nominal: self.nominal_identity(*public_id)?,
                semantic_identity: RuntimeSemanticTypeId::from_bytes(*semantic_identity),
                layout: TypeLayoutHash::from_bytes(*layout),
            }),
            AwbcRuntimeType::Opaque { .. } => self
                .opaque_owner(ty)?
                .map(|owner| RuntimeCheckedType::Opaque { owner })
                .ok_or(AwbcTypeProjectionError::UnsupportedCheckedType { index: ty.0 }),
            AwbcRuntimeType::Variant { owner, cases } => {
                let projected = cases
                    .iter()
                    .map(|case| {
                        let name = self.strings.get(case.name.index()).cloned().ok_or(
                            AwbcTypeProjectionError::StringOutOfBounds {
                                index: case.name.0,
                                role: "variant case name",
                            },
                        )?;
                        let payload = case
                            .payload
                            .map(|payload| self.checked_type(payload, depth + 1, visiting))
                            .transpose()?
                            .map(Box::new);
                        Ok(RuntimeCheckedVariantCase { name, payload })
                    })
                    .collect::<Result<Vec<_>, AwbcTypeProjectionError>>()?;
                match owner {
                    AwbcVariantIdentity::Nominal {
                        public_id,
                        semantic_identity,
                    } => Ok(RuntimeCheckedType::Variant {
                        nominal: self.nominal_identity(*public_id)?,
                        semantic_identity: RuntimeSemanticTypeId::from_bytes(*semantic_identity),
                        cases: projected,
                    }),
                    AwbcVariantIdentity::Result => match projected.as_slice() {
                        [
                            RuntimeCheckedVariantCase {
                                name: ok_name,
                                payload: Some(ok),
                            },
                            RuntimeCheckedVariantCase {
                                name: error_name,
                                payload: Some(error),
                            },
                        ] if ok_name == "Ok" && error_name == "Err" => {
                            Ok(RuntimeCheckedType::Result {
                                ok: ok.clone(),
                                error: error.clone(),
                            })
                        }
                        _ => Err(AwbcTypeProjectionError::InvalidBuiltinVariant { index: ty.0 }),
                    },
                    AwbcVariantIdentity::Option => match projected.as_slice() {
                        [
                            RuntimeCheckedVariantCase {
                                name: some_name,
                                payload: Some(item),
                            },
                            RuntimeCheckedVariantCase {
                                name: none_name,
                                payload: None,
                            },
                        ] if some_name == "Some" && none_name == "None" => {
                            Ok(RuntimeCheckedType::Option(item.clone()))
                        }
                        _ => Err(AwbcTypeProjectionError::InvalidBuiltinVariant { index: ty.0 }),
                    },
                }
            }
            AwbcRuntimeType::Record { .. }
            | AwbcRuntimeType::NominalRecord { .. }
            | AwbcRuntimeType::MatrixF32
            | AwbcRuntimeType::MatrixF64
            | AwbcRuntimeType::TensorF32
            | AwbcRuntimeType::TensorF64
            | AwbcRuntimeType::TaskHandle
            | AwbcRuntimeType::NeedHandle
            | AwbcRuntimeType::Dynamic => {
                Err(AwbcTypeProjectionError::UnsupportedCheckedType { index: ty.0 })
            }
        };
        visiting.remove(&ty);
        result
    }
}
