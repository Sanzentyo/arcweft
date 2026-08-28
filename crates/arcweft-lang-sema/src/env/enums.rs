use std::collections::HashSet;

use crate::types::TypeKind;

/// One declaration-ordered field of an accepted environment enum record case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentEnumRecordField {
    name: String,
    ty: TypeKind,
}

impl EnvironmentEnumRecordField {
    pub fn new(name: impl Into<String>, ty: TypeKind) -> Self {
        Self {
            name: name.into(),
            ty: normalize_type_kind(ty),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }
}

/// Invalid construction of one environment enum payload schema.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum EnumVariantPayloadBuildError {
    #[error("environment enum record payload contains duplicate field `{name}`")]
    DuplicateRecordField { name: String },
}

use super::base::normalize_type_kind;

/// Payload contract for one enum variant known to the semantic environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnumVariantPayload {
    Unit,
    Tuple(Vec<TypeKind>),
    Record(Box<[EnvironmentEnumRecordField]>),
}

impl EnumVariantPayload {
    /// Creates a unit variant payload contract.
    pub const fn unit() -> Self {
        Self::Unit
    }

    /// Creates a tuple/newtype variant payload contract.
    pub fn tuple(items: impl IntoIterator<Item = TypeKind>) -> Self {
        Self::Tuple(items.into_iter().map(normalize_type_kind).collect())
    }

    /// Creates a record variant payload contract.
    pub fn record(
        fields: impl IntoIterator<Item = (impl Into<String>, TypeKind)>,
    ) -> Result<Self, EnumVariantPayloadBuildError> {
        let mut names = HashSet::new();
        let mut ordered = Vec::new();
        for (name, ty) in fields {
            let name = name.into();
            if !names.insert(name.clone()) {
                return Err(EnumVariantPayloadBuildError::DuplicateRecordField { name });
            }
            ordered.push(EnvironmentEnumRecordField::new(name, ty));
        }
        Ok(Self::Record(ordered.into_boxed_slice()))
    }
}

pub(super) fn normalize_enum_variant_payload(payload: EnumVariantPayload) -> EnumVariantPayload {
    match payload {
        EnumVariantPayload::Unit => EnumVariantPayload::Unit,
        EnumVariantPayload::Tuple(items) => {
            EnumVariantPayload::Tuple(items.into_iter().map(normalize_type_kind).collect())
        }
        EnumVariantPayload::Record(fields) => EnumVariantPayload::Record(
            fields
                .into_iter()
                .map(|field| {
                    EnvironmentEnumRecordField::new(field.name, normalize_type_kind(field.ty))
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
    }
}
