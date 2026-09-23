use std::collections::HashSet;

use crate::types::TypeKind;

/// One declaration-ordered field, before or after semantic type projection.
/// Data default provenance stays on its owning field through substitution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentRecordField<T = TypeKind> {
    name: String,
    ty: T,
    data_default: Option<super::rust_metadata::RustFieldDefault>,
    skip: bool,
    wire_name: Option<String>,
    bytes_format: Option<arcweft_rust_abi::ArcweftRustBytesFormat>,
}

impl<T> EnvironmentRecordField<T> {
    pub fn new(name: impl Into<String>, ty: T) -> Self {
        Self {
            name: name.into(),
            ty,
            data_default: None,
            skip: false,
            wire_name: None,
            bytes_format: None,
        }
    }

    #[must_use]
    pub fn with_data_default(
        mut self,
        producer: Option<super::rust_metadata::RustFieldDefault>,
        skip: bool,
    ) -> Self {
        self.data_default = producer;
        self.skip = skip;
        self
    }

    pub const fn data_default(&self) -> Option<&super::rust_metadata::RustFieldDefault> {
        self.data_default.as_ref()
    }

    pub fn default_request(&self) -> Option<&super::rust_metadata::RustFieldDefault> {
        self.data_default.as_ref().or_else(|| {
            self.skip
                .then_some(&super::rust_metadata::RustFieldDefault::Trait)
        })
    }

    pub fn with_wire_policy(
        mut self,
        wire_name: Option<String>,
        bytes_format: Option<arcweft_rust_abi::ArcweftRustBytesFormat>,
    ) -> Self {
        self.wire_name = wire_name;
        self.bytes_format = bytes_format;
        self
    }

    pub fn wire_name(&self) -> &str {
        self.wire_name.as_deref().unwrap_or(&self.name)
    }
    pub const fn bytes_format(&self) -> Option<arcweft_rust_abi::ArcweftRustBytesFormat> {
        self.bytes_format
    }

    pub const fn skip(&self) -> bool {
        self.skip
    }

    pub fn try_map_type<U, E>(
        &self,
        map: impl FnOnce(&T) -> Result<U, E>,
    ) -> Result<EnvironmentRecordField<U>, E> {
        Ok(
            EnvironmentRecordField::new(self.name.clone(), map(&self.ty)?)
                .with_data_default(self.data_default.clone(), self.skip)
                .with_wire_policy(self.wire_name.clone(), self.bytes_format),
        )
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn ty(&self) -> &T {
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
    Record(Box<[EnvironmentRecordField]>),
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
            ordered.push(EnvironmentRecordField::new(name, normalize_type_kind(ty)));
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
                    EnvironmentRecordField::new(field.name, normalize_type_kind(field.ty))
                        .with_data_default(field.data_default, field.skip)
                        .with_wire_policy(field.wire_name, field.bytes_format)
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
    }
}
