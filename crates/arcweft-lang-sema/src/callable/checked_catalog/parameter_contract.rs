//! Declaration-owned lexical closure of one parameter group.

use super::CheckedCallableFacts;
use crate::{
    effect_row::EffectRow,
    effects::EffectSet,
    types::{GenericScope, TypeKind},
};

/// The checked parameter schema with declaration generics owned by its binder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallableParameterContract {
    schema: TypeKind,
    scope: GenericScope,
}

#[derive(Debug, thiserror::Error)]
pub enum CheckedCallableParameterContractError {
    #[error("parameter contract has no group {group:?}")]
    MissingGroup {
        group: crate::callable::CallableGroupIndex,
    },
    #[error("parameter contract has no declared type")]
    MissingType,
    #[error(transparent)]
    Instantiation(#[from] crate::types::TypeInstantiationError),
    #[error("parameter contract did not produce a function-shaped binding schema")]
    InvalidSchema,
}

impl CheckedCallableFacts {
    /// Closes the parameter contract independently of its executable result.
    /// The Unit result marks completion of binding, not execution of this callable.
    pub fn parameter_contract(
        &self,
        group: crate::callable::CallableGroupIndex,
    ) -> Result<CheckedCallableParameterContract, CheckedCallableParameterContractError> {
        let schema = self.signature();
        let source = schema
            .project_function_type_from_group(
                group,
                &EffectRow::closed(EffectSet::new()),
                || Ok(TypeKind::Unit),
                |_, parameter| {
                    parameter
                        .declared_type()
                        .cloned()
                        .ok_or(CheckedCallableParameterContractError::MissingType)
                },
            )
            .map_err(|error| match error {
                crate::callable::schema::CallableFunctionTypeProjectionError::MissingGroup {
                    group,
                } => CheckedCallableParameterContractError::MissingGroup { group },
                crate::callable::schema::CallableFunctionTypeProjectionError::Projection(error) => {
                    error
                }
            })?;
        let schema = schema.quantify_function_value(&source)?;
        let TypeKind::Function { binder, .. } = &schema else {
            return Err(CheckedCallableParameterContractError::InvalidSchema);
        };
        let scope = GenericScope::default().with_binder(*binder);
        Ok(CheckedCallableParameterContract { schema, scope })
    }
}

impl CheckedCallableParameterContract {
    pub const fn schema(&self) -> &TypeKind {
        &self.schema
    }
    pub const fn scope(&self) -> &GenericScope {
        &self.scope
    }
    pub fn parameters(&self) -> &[TypeKind] {
        let TypeKind::Function { params, .. } = &self.schema else {
            unreachable!("sealed parameter schema is function-shaped")
        };
        params
    }

    /// Closed subterms use the root identity; scoped terms retain this binder.
    pub fn parameter_identity(
        &self,
        ordinal: usize,
    ) -> Result<crate::types::SemanticTypeDigest, CheckedCallableParameterContractError> {
        let ty = self
            .parameters()
            .get(ordinal)
            .ok_or(CheckedCallableParameterContractError::MissingType)?;
        ty.semantic_identity_digest()
            .or_else(|_| ty.semantic_identity_digest_in_scope(&self.scope))
            .map_err(|error| CheckedCallableParameterContractError::Instantiation(error.into()))
    }
}
