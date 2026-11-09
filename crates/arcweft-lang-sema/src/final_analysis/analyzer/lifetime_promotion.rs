//! Static source admission for lifetime promotion.

use arcweft_lang_hir::{
    expr::{
        HirCallInvocation, HirExprKind, HirLifetimePromotionAuthorization,
        HirLifetimePromotionError,
    },
    identity::ExprId,
    module::HirModule,
    scope::{HirScopeKind, HirScopeOwner},
    stmt::HirStmtKind,
    symbol::{CallableDeclarationOwner, ResolvedProjectSymbol},
};

#[path = "lifetime_promotion/captures.rs"]
mod captures;
#[path = "lifetime_promotion/receiver.rs"]
mod receiver;

use self::receiver::ReceiverBorrowQuery;
use super::{
    Analyzer, FinalSemanticAnalysisError, expression_error::AnalyzerExpressionContext,
    statements::expression_span,
};
use crate::{
    callable::PreparedCallCalleeConstraintInputs, final_analysis::model::LifetimeLexicalActivation,
    types::TypeKind,
};

impl Analyzer<'_, '_, '_> {
    pub(super) fn validate_lifetime_promotion_source(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        call: &HirCallInvocation,
        callee: &PreparedCallCalleeConstraintInputs,
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
            HirLifetimePromotionAuthorization::Safe { proof: None } => {}
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
            }
            HirLifetimePromotionAuthorization::Audited => {
                if !is_in_unsafe_audit(module, owner)? {
                    return Err(FinalSemanticAnalysisError::LifetimePromotionAuditRequired {
                        source_span: expression_span(module, owner)?,
                    });
                }
            }
        }
        if LifetimeLexicalActivation::for_expression(&self.topology, owner, promotion.target())?
            .is_none()
        {
            return Err(
                FinalSemanticAnalysisError::LifetimePromotionTargetUnavailable {
                    scope: promotion.target().clone(),
                    source_span: expression_span(module, promotion.target_source())?,
                },
            );
        }
        let PreparedCallCalleeConstraintInputs::ValueReceiver { source, actual } = callee else {
            return Err(
                FinalSemanticAnalysisError::LifetimePromotionReceiverRequired {
                    source_span: expression_span(module, promotion.receiver())?,
                },
            );
        };
        if *source != promotion.receiver() {
            return Err(FinalSemanticAnalysisError::InvalidOwner);
        }
        if matches!(
            promotion.authorization(),
            HirLifetimePromotionAuthorization::Safe { .. }
        ) {
            let source_span = expression_span(module, *source)?;
            let mut receiver = ReceiverBorrowQuery::new(self.symbols, self.control, &source_span);
            if receiver.contains_borrow(actual, &self.types)?
                || (matches!(actual, TypeKind::Function { .. })
                    && self.function_value_has_borrowed_captures(
                        context,
                        *source,
                        &mut receiver,
                    )?)
            {
                return Err(
                    FinalSemanticAnalysisError::LifetimePromotionBorrowedReceiver { source_span },
                );
            }
        }
        Err(
            FinalSemanticAnalysisError::LifetimePromotionExecutionUnavailable {
                source_span: expression_span(module, owner)?,
            },
        )
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
                if matches!(statement.kind(), HirStmtKind::UnsafeLifetime { .. }) {
                    return Ok(true);
                }
            }
            HirScopeOwner::Module(_) | HirScopeOwner::Item(_) => {}
        }
        current = scope.parent();
    }
    Ok(false)
}
