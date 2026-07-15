use std::collections::BTreeMap;

use crate::types::TypeKind;

use super::base::normalize_type_kind;

/// Payload contract for one enum variant known to the semantic environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnumVariantPayload {
    Unit,
    Tuple(Vec<TypeKind>),
    Record(BTreeMap<String, TypeKind>),
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
    pub fn record(fields: impl IntoIterator<Item = (impl Into<String>, TypeKind)>) -> Self {
        Self::Record(
            fields
                .into_iter()
                .map(|(name, ty)| (name.into(), normalize_type_kind(ty)))
                .collect(),
        )
    }

    pub(crate) fn single_type(&self) -> Option<TypeKind> {
        match self {
            Self::Tuple(items) => match items.as_slice() {
                [item] => Some(item.clone()),
                items if !items.is_empty() => Some(TypeKind::Tuple(items.to_vec())),
                _ => None,
            },
            Self::Unit | Self::Record(_) => None,
        }
    }

    pub(crate) fn tuple_items(&self) -> Option<Vec<TypeKind>> {
        match self {
            Self::Tuple(items) => Some(items.clone()),
            Self::Unit | Self::Record(_) => None,
        }
    }

    pub(crate) fn record_field_type(&self, field: &str) -> Option<TypeKind> {
        match self {
            Self::Record(fields) => fields.get(field).cloned(),
            Self::Unit | Self::Tuple(_) => None,
        }
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
                .map(|(name, ty)| (name, normalize_type_kind(ty)))
                .collect(),
        ),
    }
}
