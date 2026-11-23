//! Static source admission for lifetime promotion.

use arcweft_lang_hir::{
    expr::{
        HirCallInvocation, HirExprKind, HirLifetimePromotion, HirLifetimePromotionAuthorization,
        HirLifetimePromotionError,
    },
    identity::ExprId,
    module::HirModule,
    scope::{HirScopeKind, HirScopeOwner},
    source_index::{HirSourceQuery, HirStmtSourceRole},
    stmt::{HirStmtKind, HirUnsafeAuditIdentity},
    symbol::{CallableDeclarationOwner, ResolvedProjectSymbol},
};

#[path = "lifetime_promotion/captures.rs"]
mod captures;
#[path = "lifetime_promotion/receiver.rs"]
mod receiver;

use self::receiver::{ReceiverBorrowQuery, TypeStorage};
use super::{
    Analyzer, FinalSemanticAnalysisError,
    expression_error::{AnalyzerExpressionContext, AnalyzerExpressionError},
    statements::{expression_span, source_span},
};
use crate::final_analysis::{
    CheckedExpressionResolution, CheckedTypeSelection,
    model::{
        CheckedLifetimePromotion, CheckedLifetimePromotionAuthorization, LifetimeLexicalActivation,
    },
    prepared::PreparedResolvedExpression,
};

impl Analyzer<'_, '_, '_> {
    pub(super) fn check_lifetime_promotion_expression(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        call: &HirCallInvocation,
    ) -> Result<Option<PreparedResolvedExpression>, AnalyzerExpressionError> {
        let promotion = call.lifetime_promotion(module).map_err(|error| {
            AnalyzerExpressionError::fatal(promotion_operand_error(module, owner, error))
        })?;
        let Some(promotion) = promotion else {
            return Ok(None);
        };
        let authorization = self
            .resolve_promotion_authorization(module, owner, &promotion)
            .map_err(AnalyzerExpressionError::fatal)?;
        let target_source = expression_span(module, promotion.target_source())
            .map_err(AnalyzerExpressionError::fatal)?;
        if LifetimeLexicalActivation::for_expression(&self.topology, owner, promotion.target())
            .map_err(AnalyzerExpressionError::fatal)?
            .is_none()
        {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::LifetimePromotionTargetUnavailable {
                    scope: promotion.target().clone(),
                    source_span: target_source,
                },
            ));
        }
        let receiver = promotion.receiver();
        let receiver_source =
            expression_span(module, receiver).map_err(AnalyzerExpressionError::fatal)?;
        let value = self.evaluate_expression(context, receiver, None)?;
        let ty = value.source_value_type().ok_or_else(|| {
            AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::LifetimePromotionReceiverRequired {
                    source_span: receiver_source.clone(),
                },
            )
        })?;
        if matches!(
            promotion.authorization(),
            HirLifetimePromotionAuthorization::Safe { .. }
        ) {
            let mut borrow = ReceiverBorrowQuery::new(self.symbols, self.control, &receiver_source);
            let borrowed = match borrow
                .storage(ty, &self.types, None)
                .map_err(AnalyzerExpressionError::fatal)?
            {
                TypeStorage::Borrowed => true,
                TypeStorage::Callable => self
                    .value_has_borrowed_captures(context, receiver, &mut borrow)
                    .map_err(AnalyzerExpressionError::fatal)?,
                TypeStorage::Ordinary => false,
            };
            if borrowed {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::LifetimePromotionBorrowedReceiver {
                        source_span: receiver_source,
                    },
                ));
            }
        }
        let request = CheckedLifetimePromotion {
            source: expression_span(module, owner).map_err(AnalyzerExpressionError::fatal)?,
            receiver,
            receiver_source,
            target: promotion.target().clone(),
            target_source,
            authorization,
        };
        Ok(Some(PreparedResolvedExpression::value(
            ty.clone(),
            CheckedTypeSelection::Inferred,
            value.effects().clone(),
            CheckedExpressionResolution::LifetimePromotion(Box::new(request)),
        )))
    }

    fn resolve_promotion_authorization(
        &self,
        module: &HirModule,
        owner: ExprId,
        promotion: &HirLifetimePromotion,
    ) -> Result<CheckedLifetimePromotionAuthorization, FinalSemanticAnalysisError> {
        Ok(match promotion.authorization() {
            HirLifetimePromotionAuthorization::Safe { proof: None } => {
                CheckedLifetimePromotionAuthorization::WithoutProof
            }
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
                let ResolvedProjectSymbol::Callable(symbol) = target else {
                    return Err(FinalSemanticAnalysisError::LifetimePromotionProofFamily {
                        source_span,
                    });
                };
                if symbol.owner() != CallableDeclarationOwner::Proof {
                    return Err(FinalSemanticAnalysisError::LifetimePromotionProofFamily {
                        source_span,
                    });
                }
                CheckedLifetimePromotionAuthorization::Proof {
                    declaration: symbol.declaration().clone(),
                    reference: source_span,
                    declaration_source: symbol.declaration_span().clone(),
                }
            }
            HirLifetimePromotionAuthorization::Audited => {
                let Some(authorization) = enclosing_unsafe_audit(module, owner)? else {
                    return Err(FinalSemanticAnalysisError::LifetimePromotionAuditRequired {
                        source_span: expression_span(module, owner)?,
                    });
                };
                authorization
            }
        })
    }
}

fn promotion_operand_error(
    module: &HirModule,
    owner: ExprId,
    error: HirLifetimePromotionError,
) -> FinalSemanticAnalysisError {
    let operand = match error {
        HirLifetimePromotionError::UnresolvedOperand { .. } => {
            return FinalSemanticAnalysisError::InvalidOwner;
        }
        HirLifetimePromotionError::InvalidArguments => owner,
        HirLifetimePromotionError::InvalidTarget { operand }
        | HirLifetimePromotionError::InvalidProof { operand } => operand,
    };
    match expression_span(module, operand) {
        Ok(source_span) => {
            FinalSemanticAnalysisError::InvalidLifetimePromotionOperands { source_span, error }
        }
        Err(error) => error,
    }
}

fn enclosing_unsafe_audit(
    module: &HirModule,
    expression: ExprId,
) -> Result<Option<CheckedLifetimePromotionAuthorization>, FinalSemanticAnalysisError> {
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
        match *scope.owner() {
            HirScopeOwner::Expr(owner) => {
                let expression = module
                    .resolve_expr(owner)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                if matches!(expression.kind(), HirExprKind::Thread(_)) {
                    break;
                }
            }
            HirScopeOwner::Stmt(owner) => {
                let statement = module
                    .resolve_stmt(owner)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                if let HirStmtKind::UnsafeLifetime { audit, .. } = statement.kind() {
                    let HirUnsafeAuditIdentity::Accepted(id) = audit.identity() else {
                        return Err(FinalSemanticAnalysisError::RecoveredOwner);
                    };
                    return Ok(Some(CheckedLifetimePromotionAuthorization::Audit {
                        owner,
                        id: id.clone(),
                        declaration_source: source_span(
                            module,
                            HirSourceQuery::Stmt {
                                owner,
                                role: HirStmtSourceRole::Whole,
                            },
                        )?,
                    }));
                }
            }
            HirScopeOwner::Module(_) | HirScopeOwner::Item(_) => {}
        }
        current = scope.parent();
    }
    Ok(None)
}
