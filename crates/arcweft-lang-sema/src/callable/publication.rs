//! Validated environment callable publication records.

use super::{
    CallableDocumentation, CallableLimits, CallableLookupKey, CallablePublicationError,
    CallableSignatureSchema, CallableSource, CallableValidator, EnvironmentCallableKind,
    EnvironmentCallableOwner, EnvironmentDeclarationOrdinal, RustCallableProvenance,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentCallablePublication {
    owner: EnvironmentCallableOwner,
    records: Vec<EnvironmentCallablePublicationRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentCallablePublicationRecord {
    kind: EnvironmentCallableKind,
    key: CallableLookupKey,
    overload: super::CallableOverloadIndex,
    schema: CallableSignatureSchema,
    documentation: CallableDocumentation,
    source: Option<CallableSource>,
    rust: Option<RustCallableProvenance>,
    declaration_order: EnvironmentDeclarationOrdinal,
}

impl EnvironmentCallablePublication {
    pub fn try_new(
        owner: EnvironmentCallableOwner,
        records: Vec<EnvironmentCallablePublicationRecord>,
        limits: &CallableLimits,
    ) -> Result<Self, CallablePublicationError> {
        if records.len() > limits.max_catalog_records() {
            return Err(super::CallableBuildLimitError::Records {
                actual: records.len(),
                limit: limits.max_catalog_records(),
            }
            .into());
        }
        Ok(Self { owner, records })
    }
    pub const fn owner(&self) -> &EnvironmentCallableOwner {
        &self.owner
    }
    pub fn records(&self) -> &[EnvironmentCallablePublicationRecord] {
        &self.records
    }
}

impl EnvironmentCallablePublicationRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        kind: EnvironmentCallableKind,
        key: CallableLookupKey,
        overload: super::CallableOverloadIndex,
        schema: CallableSignatureSchema,
        documentation: CallableDocumentation,
        source: Option<CallableSource>,
        rust: Option<RustCallableProvenance>,
        declaration_order: EnvironmentDeclarationOrdinal,
    ) -> Result<Self, CallablePublicationError> {
        if kind == EnvironmentCallableKind::RustFunction && rust.is_none() {
            return Err(super::CallableCatalogError::MissingRustProvenance.into());
        }
        if kind == EnvironmentCallableKind::UntypedMethodFallback
            && (!matches!(key, CallableLookupKey::Method(_))
                || !matches!(schema.validator(), CallableValidator::Untyped))
        {
            return Err(CallablePublicationError::InvalidOverload);
        }
        Ok(Self {
            kind,
            key,
            overload,
            schema,
            documentation,
            source,
            rust,
            declaration_order,
        })
    }
    pub const fn kind(&self) -> EnvironmentCallableKind {
        self.kind
    }
    pub const fn key(&self) -> &CallableLookupKey {
        &self.key
    }
    pub const fn overload(&self) -> super::CallableOverloadIndex {
        self.overload
    }
    pub const fn schema(&self) -> &CallableSignatureSchema {
        &self.schema
    }
    pub const fn documentation(&self) -> &CallableDocumentation {
        &self.documentation
    }
    pub const fn source(&self) -> Option<&CallableSource> {
        self.source.as_ref()
    }
    pub const fn rust(&self) -> Option<&RustCallableProvenance> {
        self.rust.as_ref()
    }
    pub const fn declaration_order(&self) -> EnvironmentDeclarationOrdinal {
        self.declaration_order
    }
}
