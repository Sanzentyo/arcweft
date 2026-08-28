//! Projection from verified AWBC type rows to native runtime owners.

use super::schema::{
    AwbcAgentTypeShape, AwbcProgram, AwbcRecordField, AwbcRuntimeType, AwbcRuntimeTypeShape,
    AwbcSignedIntKind, AwbcTypeId, AwbcUnsignedIntKind, AwbcVariantCase, AwbcVariantIdentity,
};
use crate::entry::{RuntimeIdentityError, RuntimeNominalTypeId, TypeLayoutHash};
use crate::pattern::{
    RuntimeBuiltinVariantIdentity, RuntimeCheckedRecordTypeError, RuntimeCheckedType,
    RuntimeCheckedVariantCase, RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId,
    RuntimeSemanticTypeId,
};
use crate::value::{
    RuntimeNominalRecordLayout, RuntimeNominalRecordLayoutError, RuntimeRecordFieldId,
    RuntimeSignedIntWidth, RuntimeUnsignedIntWidth,
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
    #[error("AWBC runtime record type {index} has an invalid checked schema: {source}")]
    InvalidRecord {
        index: u32,
        source: RuntimeCheckedRecordTypeError,
    },
}

impl AwbcRuntimeType {
    /// Reifies an opaque row through the same native owner used by checked
    /// execution. Non-opaque rows return `None`.
    pub fn try_opaque_owner(
        &self,
        strings: &[String],
    ) -> Result<Option<RuntimeOpaqueTypeOwner>, AwbcTypeProjectionError> {
        let AwbcRuntimeTypeShape::Opaque {
            producer,
            admission,
            value_class,
            persistence,
            arguments: _,
        } = self.shape()
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
        Ok(Some(RuntimeOpaqueTypeOwner::with_admission(
            producer,
            self.semantic_identity(),
            *admission,
            *value_class,
            *persistence,
        )))
    }
}

struct CheckedVariantProjection<'owner, 'visiting> {
    ty: AwbcTypeId,
    semantic_identity: RuntimeSemanticTypeId,
    owner: &'owner AwbcVariantIdentity,
    arguments: &'owner [AwbcTypeId],
    cases: &'owner [AwbcVariantCase],
    depth: usize,
    visiting: &'visiting mut BTreeSet<AwbcTypeId>,
}

const fn checked_signed_width(kind: AwbcSignedIntKind) -> RuntimeSignedIntWidth {
    match kind {
        AwbcSignedIntKind::I8 => RuntimeSignedIntWidth::I8,
        AwbcSignedIntKind::I16 => RuntimeSignedIntWidth::I16,
        AwbcSignedIntKind::I32 => RuntimeSignedIntWidth::I32,
        AwbcSignedIntKind::I64 => RuntimeSignedIntWidth::I64,
        AwbcSignedIntKind::I128 => RuntimeSignedIntWidth::I128,
        AwbcSignedIntKind::ISize => RuntimeSignedIntWidth::ISize,
    }
}

const fn checked_unsigned_width(kind: AwbcUnsignedIntKind) -> RuntimeUnsignedIntWidth {
    match kind {
        AwbcUnsignedIntKind::U8 => RuntimeUnsignedIntWidth::U8,
        AwbcUnsignedIntKind::U16 => RuntimeUnsignedIntWidth::U16,
        AwbcUnsignedIntKind::U32 => RuntimeUnsignedIntWidth::U32,
        AwbcUnsignedIntKind::U64 => RuntimeUnsignedIntWidth::U64,
        AwbcUnsignedIntKind::U128 => RuntimeUnsignedIntWidth::U128,
        AwbcUnsignedIntKind::USize => RuntimeUnsignedIntWidth::USize,
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
        let AwbcRuntimeTypeShape::NominalRecord {
            public_id,
            layout,
            arguments,
            fields,
        } = row.shape()
        else {
            return Ok(None);
        };
        let nominal = self.nominal_identity(*public_id)?;
        let mut visiting = BTreeSet::new();
        let arguments = arguments
            .iter()
            .map(|argument| self.checked_type_at_depth(*argument, 0, &mut visiting))
            .collect::<Result<Vec<_>, _>>()?;
        let fields = fields
            .iter()
            .map(|field| {
                let name = self.strings.get(field.name.index()).cloned().ok_or(
                    AwbcTypeProjectionError::StringOutOfBounds {
                        index: field.name.0,
                        role: "nominal record field name",
                    },
                )?;
                self.checked_type_at_depth(field.ty, 0, &mut visiting)
                    .map(|checked| (name, checked))
            })
            .collect::<Result<Vec<_>, _>>()?;
        RuntimeNominalRecordLayout::try_from_checked_projection(
            nominal,
            row.semantic_identity(),
            TypeLayoutHash::from_bytes(*layout),
            arguments,
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

    /// Reifies one indexed AWBC runtime type as the shared checked-type owner.
    pub fn checked_type(
        &self,
        ty: AwbcTypeId,
    ) -> Result<RuntimeCheckedType, AwbcTypeProjectionError> {
        self.checked_type_at_depth(ty, 0, &mut BTreeSet::new())
    }

    fn checked_type_at_depth(
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
        let result = match row.shape() {
            AwbcRuntimeTypeShape::Never => Ok(RuntimeCheckedType::Never),
            AwbcRuntimeTypeShape::Unit => Ok(RuntimeCheckedType::Unit),
            AwbcRuntimeTypeShape::Bool => Ok(RuntimeCheckedType::Bool),
            AwbcRuntimeTypeShape::Int(kind) => {
                Ok(RuntimeCheckedType::Signed(checked_signed_width(*kind)))
            }
            AwbcRuntimeTypeShape::UInt(kind) => {
                Ok(RuntimeCheckedType::Unsigned(checked_unsigned_width(*kind)))
            }
            AwbcRuntimeTypeShape::F32 => Ok(RuntimeCheckedType::F32),
            AwbcRuntimeTypeShape::F64 => Ok(RuntimeCheckedType::F64),
            AwbcRuntimeTypeShape::String => Ok(RuntimeCheckedType::String),
            AwbcRuntimeTypeShape::Char => Ok(RuntimeCheckedType::Char),
            AwbcRuntimeTypeShape::Duration => Ok(RuntimeCheckedType::Duration),
            AwbcRuntimeTypeShape::Progress => Ok(RuntimeCheckedType::Progress),
            AwbcRuntimeTypeShape::EntityRef => Ok(RuntimeCheckedType::EntityReference),
            AwbcRuntimeTypeShape::AgentValue => Ok(RuntimeCheckedType::AgentValue),
            AwbcRuntimeTypeShape::Bytes => Ok(RuntimeCheckedType::Bytes),
            AwbcRuntimeTypeShape::Sequence(item) => self
                .checked_type_at_depth(*item, depth + 1, visiting)
                .map(Box::new)
                .map(RuntimeCheckedType::Sequence),
            AwbcRuntimeTypeShape::Tuple(items) => items
                .iter()
                .map(|item| self.checked_type_at_depth(*item, depth + 1, visiting))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeCheckedType::Tuple),
            AwbcRuntimeTypeShape::Record { fields, .. } => {
                self.checked_record_type(ty, fields, depth, visiting)
            }
            AwbcRuntimeTypeShape::Choice(alternatives) => alternatives
                .iter()
                .map(|item| self.checked_type_at_depth(*item, depth + 1, visiting))
                .collect::<Result<Vec<_>, _>>()
                .map(RuntimeCheckedType::Choice),
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
            } => self.checked_nominal_type(
                row.semantic_identity(),
                *public_id,
                *layout,
                arguments,
                depth,
                visiting,
            ),
            AwbcRuntimeTypeShape::Opaque { .. } => self
                .opaque_owner(ty)?
                .map(|owner| RuntimeCheckedType::Opaque { owner })
                .ok_or(AwbcTypeProjectionError::UnsupportedCheckedType { index: ty.0 }),
            AwbcRuntimeTypeShape::Variant { .. } => {
                self.checked_variant_row(ty, row, depth, visiting)
            }
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(agent)) => {
                Ok(RuntimeCheckedType::Agent(*agent))
            }
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Probe(_))
            | AwbcRuntimeTypeShape::MatrixF32
            | AwbcRuntimeTypeShape::MatrixF64
            | AwbcRuntimeTypeShape::TensorF32
            | AwbcRuntimeTypeShape::TensorF64
            | AwbcRuntimeTypeShape::Range(_)
            | AwbcRuntimeTypeShape::Iterator(_)
            | AwbcRuntimeTypeShape::Array { .. }
            | AwbcRuntimeTypeShape::Map { .. }
            | AwbcRuntimeTypeShape::Need(_)
            | AwbcRuntimeTypeShape::Task(_)
            | AwbcRuntimeTypeShape::Stream { .. }
            | AwbcRuntimeTypeShape::Shared(_)
            | AwbcRuntimeTypeShape::Reference(_)
            | AwbcRuntimeTypeShape::Function { .. }
            | AwbcRuntimeTypeShape::Dynamic => {
                Err(AwbcTypeProjectionError::UnsupportedCheckedType { index: ty.0 })
            }
        };
        visiting.remove(&ty);
        result
    }

    fn checked_record_type(
        &self,
        ty: AwbcTypeId,
        fields: &[AwbcRecordField],
        depth: usize,
        visiting: &mut BTreeSet<AwbcTypeId>,
    ) -> Result<RuntimeCheckedType, AwbcTypeProjectionError> {
        let mut checked = Vec::with_capacity(fields.len());
        for (ordinal, field) in fields.iter().enumerate() {
            let diagnostic_name = self.strings.get(field.name.index()).cloned().ok_or(
                AwbcTypeProjectionError::StringOutOfBounds {
                    index: field.name.0,
                    role: "structural record field name",
                },
            )?;
            let field_id =
                RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).map_err(|_| {
                    AwbcTypeProjectionError::InvalidRecord {
                        index: ty.0,
                        source: RuntimeCheckedRecordTypeError::FieldOrdinalOverflow,
                    }
                })?;
            let field_ty = self.checked_type_at_depth(field.ty, depth + 1, visiting)?;
            checked.push((field_id, diagnostic_name, field_ty));
        }
        RuntimeCheckedType::try_record(checked).map_err(|source| {
            AwbcTypeProjectionError::InvalidRecord {
                index: ty.0,
                source,
            }
        })
    }

    fn checked_nominal_type(
        &self,
        semantic_identity: RuntimeSemanticTypeId,
        public_id: super::schema::AwbcStringId,
        layout: [u8; 32],
        arguments: &[AwbcTypeId],
        depth: usize,
        visiting: &mut BTreeSet<AwbcTypeId>,
    ) -> Result<RuntimeCheckedType, AwbcTypeProjectionError> {
        Ok(RuntimeCheckedType::Nominal {
            nominal: self.nominal_identity(public_id)?,
            semantic_identity,
            layout: TypeLayoutHash::from_bytes(layout),
            arguments: self.checked_children(arguments, depth, visiting)?,
        })
    }

    fn checked_variant_row(
        &self,
        ty: AwbcTypeId,
        row: &AwbcRuntimeType,
        depth: usize,
        visiting: &mut BTreeSet<AwbcTypeId>,
    ) -> Result<RuntimeCheckedType, AwbcTypeProjectionError> {
        let AwbcRuntimeTypeShape::Variant {
            owner,
            arguments,
            cases,
        } = row.shape()
        else {
            unreachable!("checked_variant_row called for a non-variant AWBC row");
        };
        self.checked_variant_type(CheckedVariantProjection {
            ty,
            semantic_identity: row.semantic_identity(),
            owner,
            arguments,
            cases,
            depth,
            visiting,
        })
    }

    fn checked_variant_type(
        &self,
        projection: CheckedVariantProjection<'_, '_>,
    ) -> Result<RuntimeCheckedType, AwbcTypeProjectionError> {
        let CheckedVariantProjection {
            ty,
            semantic_identity,
            owner,
            arguments,
            cases,
            depth,
            visiting,
        } = projection;
        let projected_arguments = self.checked_children(arguments, depth, visiting)?;
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
                    .map(|payload| self.checked_type_at_depth(payload, depth + 1, visiting))
                    .transpose()?
                    .map(Box::new);
                Ok(RuntimeCheckedVariantCase { name, payload })
            })
            .collect::<Result<Vec<_>, AwbcTypeProjectionError>>()?;
        match owner {
            AwbcVariantIdentity::Nominal { public_id } => Ok(RuntimeCheckedType::Variant {
                owner: crate::pattern::RuntimeVariantIdentity::Nominal {
                    nominal: self.nominal_identity(*public_id)?,
                    semantic_identity,
                },
                arguments: projected_arguments,
                cases: projected,
            }),
            AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Result)
                if projected_arguments.is_empty() =>
            {
                match projected.as_slice() {
                    [
                        RuntimeCheckedVariantCase {
                            name: ok_name,
                            payload: Some(ok),
                        },
                        RuntimeCheckedVariantCase {
                            name: error_name,
                            payload: Some(error),
                        },
                    ] if ok_name == "Ok" && error_name == "Err" => Ok(RuntimeCheckedType::Result {
                        ok: unwrap_single_field_tuple(ok).ok_or(
                            AwbcTypeProjectionError::InvalidBuiltinVariant { index: ty.0 },
                        )?,
                        error: unwrap_single_field_tuple(error).ok_or(
                            AwbcTypeProjectionError::InvalidBuiltinVariant { index: ty.0 },
                        )?,
                    }),
                    _ => Err(AwbcTypeProjectionError::InvalidBuiltinVariant { index: ty.0 }),
                }
            }
            AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Option)
                if projected_arguments.is_empty() =>
            {
                match projected.as_slice() {
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
                        unwrap_single_field_tuple(item)
                            .map(RuntimeCheckedType::Option)
                            .ok_or(AwbcTypeProjectionError::InvalidBuiltinVariant { index: ty.0 })
                    }
                    _ => Err(AwbcTypeProjectionError::InvalidBuiltinVariant { index: ty.0 }),
                }
            }
            AwbcVariantIdentity::Builtin(owner) if projected_arguments.is_empty() => {
                Ok(RuntimeCheckedType::Variant {
                    owner: crate::pattern::RuntimeVariantIdentity::Builtin(*owner),
                    arguments: projected_arguments,
                    cases: projected,
                })
            }
            AwbcVariantIdentity::Builtin(_) => {
                Err(AwbcTypeProjectionError::InvalidBuiltinVariant { index: ty.0 })
            }
        }
    }

    fn checked_children(
        &self,
        children: &[AwbcTypeId],
        depth: usize,
        visiting: &mut BTreeSet<AwbcTypeId>,
    ) -> Result<Vec<RuntimeCheckedType>, AwbcTypeProjectionError> {
        children
            .iter()
            .map(|child| self.checked_type_at_depth(*child, depth + 1, visiting))
            .collect()
    }
}

fn unwrap_single_field_tuple(payload: &RuntimeCheckedType) -> Option<Box<RuntimeCheckedType>> {
    let RuntimeCheckedType::Tuple(fields) = payload else {
        return None;
    };
    let [field] = fields.as_slice() else {
        return None;
    };
    Some(Box::new(field.clone()))
}
