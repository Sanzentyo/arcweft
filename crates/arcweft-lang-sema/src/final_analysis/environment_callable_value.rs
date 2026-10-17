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
pub struct CheckedEnvironmentCallableValueProjection {
    origin: CheckedExpressionOrigin,
    catalog: Arc<CheckedCallableCatalog>,
    declaration: CheckedCallableId,
    ty: TypeKind,
    type_identity: SemanticTypeDigest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CheckedEnvironmentCallableValueProjectionError {
    #[error("expression {owner:?} has no accepted semantic value")]
    MissingExpression { owner: ExprId },
    #[error("expression {owner:?} is not an admitted environment callable value")]
    NotEnvironmentCallable { owner: ExprId },
    #[error("expression {owner:?} has no accepted checked callable declaration")]
    MissingDeclaration { owner: ExprId },
    #[error("expression {owner:?} has no complete callable value type")]
    TypeMismatch { owner: ExprId },
    #[error("expression {owner:?} has no accepted source-coordinate authority")]
    MissingOrigin { owner: ExprId },
}

impl FinalSemanticAnalysis {
    pub fn environment_callable_value_projection(
        &self,
        owner: ExprId,
    ) -> Result<
        CheckedEnvironmentCallableValueProjection,
        CheckedEnvironmentCallableValueProjectionError,
    > {
        use CheckedEnvironmentCallableValueProjectionError as Error;

        let expression = self
            .expression(owner)
            .ok_or(Error::MissingExpression { owner })?;
        let CheckedExpressionResolution::Value(CheckedValueResolution::EnvironmentCallable(id)) =
            expression.resolution()
        else {
            return Err(Error::NotEnvironmentCallable { owner });
        };
        self.checked_callables()
            .callable(id)
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
        Ok(CheckedEnvironmentCallableValueProjection {
            origin,
            catalog: Arc::clone(self.checked_callables()),
            declaration: id.clone(),
            ty: ty.clone(),
            type_identity,
        })
    }
}

impl CheckedEnvironmentCallableValueProjection {
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

impl std::fmt::Debug for CheckedEnvironmentCallableValueProjection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckedEnvironmentCallableValueProjection")
            .field("origin", &self.origin)
            .field("declaration", &self.declaration)
            .field("type_identity", &self.type_identity)
            .finish_non_exhaustive()
    }
}
