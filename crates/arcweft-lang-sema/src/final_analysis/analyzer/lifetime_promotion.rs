//! Static source admission for lifetime promotion.

use arcweft_lang_hir::{
    expr::{
        HirCallInvocation, HirExprKind, HirLifetimePromotionAuthorization,
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
    expression_error::AnalyzerExpressionContext,
    statements::{expression_span, source_span},
};
use crate::{
    callable::PreparedCallCalleeConstraintInputs,
    final_analysis::model::{
        CheckedLifetimePromotion, CheckedLifetimePromotionAuthorization, LifetimeLexicalActivation,
    },
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
        let authorization = match promotion.authorization() {
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
        };
        let target_source = expression_span(module, promotion.target_source())?;
        if LifetimeLexicalActivation::for_expression(&self.topology, owner, promotion.target())?
            .is_none()
        {
            return Err(
                FinalSemanticAnalysisError::LifetimePromotionTargetUnavailable {
                    scope: promotion.target().clone(),
                    source_span: target_source,
                },
            );
        }
        let PreparedCallCalleeConstraintInputs::ValueReceiver(receiver) = callee else {
            return Err(
                FinalSemanticAnalysisError::LifetimePromotionReceiverRequired {
                    source_span: expression_span(module, promotion.receiver())?,
                },
            );
        };
        let source = receiver.source();
        if source != promotion.receiver() {
            return Err(FinalSemanticAnalysisError::InvalidOwner);
        }
        let receiver_source = expression_span(module, source)?;
        if matches!(
            promotion.authorization(),
            HirLifetimePromotionAuthorization::Safe { .. }
        ) {
            let mut borrow = ReceiverBorrowQuery::new(self.symbols, self.control, &receiver_source);
            let borrowed = match borrow.storage(receiver.original(), &self.types, None)? {
                TypeStorage::Borrowed => true,
                TypeStorage::Callable => {
                    self.value_has_borrowed_captures(context, source, &mut borrow)?
                }
                TypeStorage::Ordinary => false,
            };
            if borrowed {
                return Err(
                    FinalSemanticAnalysisError::LifetimePromotionBorrowedReceiver {
                        source_span: receiver_source,
                    },
                );
            }
        }
        Err(
            FinalSemanticAnalysisError::LifetimePromotionExecutionUnavailable {
                source_span: expression_span(module, owner)?,
                request: Box::new(CheckedLifetimePromotion {
                    receiver: receiver_source,
                    target: promotion.target().clone(),
                    target_source,
                    authorization,
                }),
            },
        )
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
