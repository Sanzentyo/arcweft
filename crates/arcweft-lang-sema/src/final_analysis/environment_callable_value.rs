//! An accepted declaration value and its complete checked execution interface.

use std::sync::Arc;

use super::{
    CheckedExpressionResolution, CheckedValueResolution, ExprId, FinalSemanticAnalysis,
    SemanticTypeDigest, TypeKind,
};
use crate::{
    callable::{CheckedCallableCatalog, CheckedCallableFacts, CheckedCallableId},
    semantic_coordinate::CheckedExpressionOrigin,
};

/// An expression-origin coordinate alone cannot authorize a callable payload.
/// This projection retains the original complete checked declaration, including
/// its groups, defaults, effects and provider, rather than another signature.
#[derive(Clone, Eq, PartialEq)]
pub struct CheckedExternalCallableValueProjection {
    origin: CheckedExpressionOrigin,
    catalog: Arc<CheckedCallableCatalog>,
    declaration: CheckedCallableId,
    ty: TypeKind,
    type_identity: SemanticTypeDigest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CheckedExternalCallableValueProjectionError {
    #[error("expression {owner:?} has no accepted semantic value")]
    MissingExpression { owner: ExprId },
    #[error("expression {owner:?} is not an admitted external callable value")]
    NotExternalCallable { owner: ExprId },
    #[error("expression {owner:?} has no accepted checked callable declaration")]
    MissingDeclaration { owner: ExprId },
    #[error("expression {owner:?} has no complete callable value type")]
    TypeMismatch { owner: ExprId },
    #[error("expression {owner:?} has no accepted source-coordinate authority")]
    MissingOrigin { owner: ExprId },
}

impl FinalSemanticAnalysis {
    pub fn external_callable_value_projection(
        &self,
        owner: ExprId,
    ) -> Result<CheckedExternalCallableValueProjection, CheckedExternalCallableValueProjectionError>
    {
        use CheckedExternalCallableValueProjectionError as Error;

        let expression = self
            .expression(owner)
            .ok_or(Error::MissingExpression { owner })?;
        let id = match expression.resolution() {
            CheckedExpressionResolution::Value(CheckedValueResolution::EnvironmentCallable(id)) => {
                id.clone()
            }
            CheckedExpressionResolution::Value(CheckedValueResolution::ProjectCallable(
                callable,
            )) if callable.declaration().owner()
                == arcweft_lang_hir::symbol::CallableDeclarationOwner::ExternCapability =>
            {
                let declaration = self
                    .checked_callables()
                    .project_callable(callable.declaration())
                    .map_err(|_| Error::MissingDeclaration { owner })?;
                if declaration.host_call_contract().is_none() {
                    return Err(Error::MissingDeclaration { owner });
                }
                declaration.id().clone()
            }
            _ => return Err(Error::NotExternalCallable { owner }),
        };
        self.checked_callables()
            .callable(&id)
            .map_err(|_| Error::MissingDeclaration { owner })?;
        let Some(ty @ TypeKind::Function { .. }) = expression.source_value_type() else {
            return Err(Error::TypeMismatch { owner });
        };
        let type_identity = ty
            .semantic_identity_digest()
            .map_err(|_| Error::TypeMismatch { owner })?;
        let origin = self
            .expression_origin(owner)
            .map_err(|_| Error::MissingOrigin { owner })?;
        Ok(CheckedExternalCallableValueProjection {
            origin,
            catalog: Arc::clone(self.checked_callables()),
            declaration: id,
            ty: ty.clone(),
            type_identity,
        })
    }
}

impl CheckedExternalCallableValueProjection {
    pub const fn origin(&self) -> &CheckedExpressionOrigin {
        &self.origin
    }

    /// Borrows the same immutable catalog row validated before issuance.
    ///
    /// # Panics
    ///
    /// Panics if the private issuance invariant is violated: the retained
    /// declaration must remain in the immutable catalog checked by the issuer.
    pub fn declaration(&self) -> &CheckedCallableFacts {
        self.catalog
            .callable(&self.declaration)
            .expect("the private issuer validates this immutable catalog join")
    }

    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }

    pub const fn type_identity(&self) -> SemanticTypeDigest {
        self.type_identity
    }
}

impl std::fmt::Debug for CheckedExternalCallableValueProjection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckedExternalCallableValueProjection")
            .field("origin", &self.origin)
            .field("declaration", &self.declaration)
            .field("type_identity", &self.type_identity)
            .finish_non_exhaustive()
    }
}
