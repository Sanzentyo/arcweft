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

/// Selected declaration authority for a completed value-call group. The
/// structural callee arrow alone cannot describe omitted declaration formals.
#[derive(Clone, Eq, PartialEq)]
pub struct CheckedExternalCallableApplicationProjection {
    origin: CheckedExpressionOrigin,
    catalog: Arc<CheckedCallableCatalog>,
    declaration: CheckedCallableId,
    function_type: SemanticTypeDigest,
    group: crate::callable::CallableGroupIndex,
}

impl std::fmt::Debug for CheckedExternalCallableApplicationProjection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckedExternalCallableApplicationProjection")
            .field("origin", &self.origin)
            .field("declaration", &self.declaration)
            .field("function_type", &self.function_type)
            .field("group", &self.group)
            .finish_non_exhaustive()
    }
}

impl CheckedExternalCallableApplicationProjection {
    pub const fn origin(&self) -> &CheckedExpressionOrigin {
        &self.origin
    }
    pub const fn function_type(&self) -> SemanticTypeDigest {
        self.function_type
    }
    pub const fn group(&self) -> crate::callable::CallableGroupIndex {
        self.group
    }
    /// # Panics
    /// The issuer retains the immutable catalog that admitted this declaration.
    pub fn declaration(&self) -> &CheckedCallableFacts {
        self.catalog
            .callable(&self.declaration)
            .expect("application issuer validates immutable catalog membership")
    }
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
    pub fn external_callable_application_projection(
        &self,
        owner: ExprId,
    ) -> Result<
        Option<CheckedExternalCallableApplicationProjection>,
        CheckedExternalCallableValueProjectionError,
    > {
        use crate::callable::{
            CallableGroupIndex, CallableValidator, CheckedCallCalleeExecution, CheckedCallResult,
        };
        use CheckedExternalCallableValueProjectionError as Error;
        let application = self
            .call(owner)
            .and_then(|call| call.selected_application())
            .ok_or(Error::MissingExpression { owner })?;
        if !matches!(
            application.core().callee(),
            CheckedCallCalleeExecution::Value { .. }
        ) {
            return Ok(None);
        }
        let Some(id) = application.core().candidates().selected().checked() else {
            return Ok(None);
        };
        let checked = self
            .checked_callables()
            .callable(id)
            .map_err(|_| Error::MissingDeclaration { owner })?;
        let schema = checked.signature();
        if checked.host_call_contract().is_some()
            || !matches!(schema.validator(), CallableValidator::Ordinary)
        {
            return Ok(None);
        }
        // Project functions own another checked body/continuation contract.
        if !matches!(
            id.declaration(),
            crate::callable::CheckedCallableDeclaration::Environment(_)
        ) {
            return Ok(None);
        }
        let group = application.core().current_group();
        let complete = match application.result() {
            CheckedCallResult::Continuation(next) => next.next_group().get() == group.get() + 1,
            CheckedCallResult::Value(value) => schema.value_type() == Some(value),
            CheckedCallResult::ContentEmission(_) => false,
        };
        if !complete {
            return Ok(None);
        }
        let effects = schema
            .effects()
            .fixed_row()
            .ok_or(Error::TypeMismatch { owner })?;
        let ty = schema
            .declared_function_type_from_group(CallableGroupIndex::ZERO, effects)
            .map_err(|_| Error::TypeMismatch { owner })?;
        let ty = schema
            .quantify_function_value(&ty)
            .map_err(|_| Error::TypeMismatch { owner })?;
        let function_type = ty
            .semantic_identity_digest()
            .map_err(|_| Error::TypeMismatch { owner })?;
        let origin = self
            .expression_origin(owner)
            .map_err(|_| Error::MissingOrigin { owner })?;
        Ok(Some(CheckedExternalCallableApplicationProjection {
            origin,
            catalog: Arc::clone(self.checked_callables()),
            declaration: id.clone(),
            function_type,
            group,
        }))
    }

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
