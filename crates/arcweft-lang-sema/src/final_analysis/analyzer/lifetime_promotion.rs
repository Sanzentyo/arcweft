//! Static source admission for lifetime promotion.

use arcweft_lang_hir::{
    expr::{HirCallInvocation, HirLifetimePromotionAuthorization, HirLifetimePromotionError},
    identity::ExprId,
    module::HirModule,
    scope::{HirScopeKind, HirScopeOwner},
    stmt::HirStmtKind,
    symbol::{CallableDeclarationOwner, ResolvedProjectSymbol},
};

use super::{Analyzer, FinalSemanticAnalysisError, statements::expression_span};

impl Analyzer<'_, '_, '_> {
    pub(super) fn validate_lifetime_promotion_source(
        &self,
        module: &HirModule,
        owner: ExprId,
        call: &HirCallInvocation,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let promotion = match call.lifetime_promotion(module) {
            Ok(Some(promotion)) => promotion,
            Ok(None) => return Ok(()),
            Err(error) => {
                let operand = match error {
                    HirLifetimePromotionError::UnresolvedOperand { .. } => {
                        return Err(FinalSemanticAnalysisError::InvalidOwner);
                    }
                    HirLifetimePromotionError::InvalidArguments => owner,
                    HirLifetimePromotionError::InvalidTarget { operand }
                    | HirLifetimePromotionError::InvalidProof { operand } => operand,
                };
                return Err(
                    FinalSemanticAnalysisError::InvalidLifetimePromotionOperands {
                        source_span: expression_span(module, operand)?,
                        error,
                    },
                );
            }
        };
        match promotion.authorization() {
            HirLifetimePromotionAuthorization::Safe { proof: None } => Ok(()),
            HirLifetimePromotionAuthorization::Safe {
                proof: Some((proof, reference)),
            } => {
                let source_span = expression_span(module, *proof)?;
                let target = self
                    .symbols
                    .resolve_entity_reference(module.key().path(), reference, source_span.clone())
                    .map_err(
                        |error| FinalSemanticAnalysisError::LifetimePromotionProofLookup {
                            source_span: source_span.clone(),
                            error: Box::new(error),
                        },
                    )?;
                if !matches!(target, ResolvedProjectSymbol::Callable(symbol) if symbol.owner() == CallableDeclarationOwner::Proof)
                {
                    return Err(FinalSemanticAnalysisError::LifetimePromotionProofFamily {
                        source_span,
                    });
                }
                Ok(())
            }
            HirLifetimePromotionAuthorization::Audited => {
                if is_in_unsafe_audit(module, owner)? {
                    Ok(())
                } else {
                    Err(FinalSemanticAnalysisError::LifetimePromotionAuditRequired {
                        source_span: expression_span(module, owner)?,
                    })
                }
            }
        }
    }
}

fn is_in_unsafe_audit(
    module: &HirModule,
    expression: ExprId,
) -> Result<bool, FinalSemanticAnalysisError> {
    let expression = module
        .resolve_expr(expression)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
    let mut current = Some(expression.scope());
    while let Some(scope) = current {
        let scope = module
            .resolve_scope(scope)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if scope.kind() == HirScopeKind::Closure {
            break;
        }
        if let HirScopeOwner::Stmt(owner) = *scope.owner() {
            let statement = module
                .resolve_stmt(owner)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            if matches!(statement.kind(), HirStmtKind::UnsafeLifetime { .. }) {
                return Ok(true);
            }
        }
        current = scope.parent();
    }
    Ok(false)
}
