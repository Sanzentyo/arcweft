//! Source field defaults joined to the existing registered callable authority.

use std::{collections::BTreeMap, sync::Arc};

use arcweft_core::{entry::RuntimeCallableId, pattern::RuntimeSemanticTypeId};
use arcweft_id::runtime_program::RuntimePureProgramId;
use arcweft_rust_abi::ArcweftRustCallableRole;

use super::{CallableRecord, CanonicalEncoder, RegisteredCallableCatalog, encode_record};
use crate::{
    callable::{
        CallableEffectSchema, CheckedCallableCatalog, CheckedCallableId, RustCallablePurity,
    },
    env::rust_metadata::RustFieldDefault,
    types::TypeKind,
};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RustFieldDefaultBindingError {
    #[error("the field default has no registered Rust callable")]
    Missing,
    #[error("the field default selects more than one registered Rust callable")]
    Ambiguous,
    #[error("the registered default is not declared pure")]
    NotPure,
    #[error("the registered default is not nullary")]
    HasParameters,
    #[error("the registered default has effects or an unresolved effect row")]
    HasEffects,
    #[error("the registered default result differs from the exact instantiated field type")]
    ResultMismatch,
    #[error("the field default belongs to a different checked callable generation")]
    StaleGeneration,
    #[error(transparent)]
    GenericScope(#[from] crate::types::GenericScopeError),
}

/// A lease on the original accepted callable allocation. The program digest
/// commits that complete declaration, its source, role, signature and result;
/// it is never manufactured from a function's path spelling alone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredRustFieldDefaultProgram {
    program: RuntimePureProgramId,
    record: Arc<CallableRecord>,
    result: RuntimeSemanticTypeId,
}

/// The same registered program after the checked candidate-to-declaration join.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRustFieldDefaultProgram {
    registered: RegisteredRustFieldDefaultProgram,
    checked: CheckedCallableId,
}

/// Indexes into the existing immutable catalog, never copied declarations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RustFieldDefaultIndex {
    paths: BTreeMap<crate::callable::RustItemPath, Vec<crate::callable::EnvironmentCallableId>>,
    results:
        BTreeMap<crate::types::SemanticTypeDigest, Vec<crate::callable::EnvironmentCallableId>>,
}

impl RustFieldDefaultIndex {
    pub(super) fn new(
        catalog: &super::EnvironmentCallableCatalog,
    ) -> Result<Self, crate::types::GenericScopeError> {
        let mut result = Self {
            paths: BTreeMap::new(),
            results: BTreeMap::new(),
        };
        for (id, record) in &catalog.by_id {
            let Some(rust) = record.rust() else { continue };
            result
                .paths
                .entry(rust.rust_path().clone())
                .or_default()
                .push(id.clone());
            if rust.role() == ArcweftRustCallableRole::DefaultConstructor
                && let Some(ty) = record.schema().value_type()
            {
                result
                    .results
                    .entry(ty.semantic_identity_digest()?)
                    .or_default()
                    .push(id.clone());
            }
        }
        Ok(result)
    }
}

impl RegisteredCallableCatalog {
    pub(crate) fn rust_field_default(
        &self,
        source: &RustFieldDefault,
        result: &TypeKind,
    ) -> Result<RegisteredRustFieldDefaultProgram, RustFieldDefaultBindingError> {
        let candidates = match source {
            RustFieldDefault::Function(path) => self.rust_defaults.paths.get(path),
            RustFieldDefault::Trait => self
                .rust_defaults
                .results
                .get(&result.semantic_identity_digest()?),
        }
        .ok_or(RustFieldDefaultBindingError::Missing)?;
        let [id] = candidates.as_slice() else {
            return Err(RustFieldDefaultBindingError::Ambiguous);
        };
        let record = self
            .environment
            .by_id
            .get(id)
            .ok_or(RustFieldDefaultBindingError::StaleGeneration)?;
        if record
            .rust()
            .is_none_or(|rust| rust.purity() != RustCallablePurity::Pure)
        {
            return Err(RustFieldDefaultBindingError::NotPure);
        }
        if record
            .schema()
            .groups()
            .iter()
            .any(|group| !group.parameters().is_empty())
            || record.schema().groups().len() > 1
        {
            return Err(RustFieldDefaultBindingError::HasParameters);
        }
        if !matches!(record.schema().effects(), CallableEffectSchema::Fixed(row) if row.is_empty())
        {
            return Err(RustFieldDefaultBindingError::HasEffects);
        }
        if record.schema().value_type() != Some(result) {
            return Err(RustFieldDefaultBindingError::ResultMismatch);
        }
        let result = RuntimeSemanticTypeId::from(result.semantic_identity_digest()?);
        let mut encoder = CanonicalEncoder::default();
        encode_record(&mut encoder, record);
        encoder.bytes(result.as_bytes());
        let program = RuntimePureProgramId::from_checked_digest(
            encoder.finish(b"arcweft.rust-field-default-program.v1\0")?,
        );
        Ok(RegisteredRustFieldDefaultProgram {
            program,
            record: Arc::clone(record),
            result,
        })
    }
}

impl RegisteredRustFieldDefaultProgram {
    pub const fn program(&self) -> RuntimePureProgramId {
        self.program
    }

    pub(crate) fn bind(
        self,
        catalog: &CheckedCallableCatalog,
    ) -> Result<CheckedRustFieldDefaultProgram, RustFieldDefaultBindingError> {
        let checked = catalog
            .checked_for_candidate(self.record.id())
            .map_err(|_| RustFieldDefaultBindingError::StaleGeneration)?;
        let facts = catalog
            .callable(checked)
            .map_err(|_| RustFieldDefaultBindingError::StaleGeneration)?;
        if !Arc::ptr_eq(facts.record(), &self.record) {
            return Err(RustFieldDefaultBindingError::StaleGeneration);
        }
        Ok(CheckedRustFieldDefaultProgram {
            registered: self,
            checked: checked.clone(),
        })
    }
}

impl CheckedRustFieldDefaultProgram {
    pub const fn program(&self) -> RuntimePureProgramId {
        self.registered.program
    }
    pub const fn result_type(&self) -> RuntimeSemanticTypeId {
        self.registered.result
    }
    pub fn target(&self) -> RuntimeCallableId {
        RuntimeCallableId::from_checked_digest(self.checked.semantic_digest().into_bytes())
    }
    pub const fn checked(&self) -> &CheckedCallableId {
        &self.checked
    }
}
