//! Source operands of receiver-based lifetime promotion.

use arcweft_id::{DeclarationIdentityFamily, LifetimeScopeKind};
use thiserror::Error;

use super::{
    HirCallArgument, HirCallArgumentListTerminator, HirCallCallee, HirCallInvocation,
    HirCallInvocationForm, HirCallTypeApplication, HirCallValue, HirExprKind,
    HirRequiredTokenState,
};
use crate::{identity::ExprId, leaf::HirIdRef, module::HirModule};

/// Exact receiver and static operands of an authored lifetime promotion.
/// Semantic checking resolves the target selector and proof reference against
/// the enclosing execution context and registered proof declarations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirLifetimePromotion {
    receiver: ExprId,
    target_source: ExprId,
    target: LifetimeScopeKind,
    authorization: HirLifetimePromotionAuthorization,
}

/// The authored authorization request, before semantic checking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirLifetimePromotionAuthorization {
    Safe { proof: Option<(ExprId, HirIdRef)> },
    Audited,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HirLifetimePromotionError {
    #[error("lifetime promotion operand {operand:?} is outside this HIR module")]
    UnresolvedOperand { operand: ExprId },
    #[error(
        "lifetime promotion requires one positional scope and, for promote, an optional named proof"
    )]
    InvalidArguments,
    #[error("lifetime promotion requires a bare, nonoptional scope selector")]
    InvalidTarget { operand: ExprId },
    #[error("lifetime promotion requires a proof declaration reference")]
    InvalidProof { operand: ExprId },
}

impl HirCallInvocation {
    /// Projects the receiver grammar from the typed HIR operands.
    pub fn lifetime_promotion(
        &self,
        module: &HirModule,
    ) -> Result<Option<HirLifetimePromotion>, HirLifetimePromotionError> {
        let HirCallCallee::UnresolvedDot {
            value_receiver,
            member,
            ..
        } = self.callee()
        else {
            return Ok(None);
        };
        let audited = match member.resolved().map(crate::leaf::HirName::as_str) {
            Some("promote") => false,
            Some("promote_unchecked") => true,
            _ => return Ok(None),
        };
        let Some(first) = self.arguments().first() else {
            return Ok(None);
        };
        let first = module.resolve_expr(first.value()).map_err(|_| {
            HirLifetimePromotionError::UnresolvedOperand {
                operand: first.value(),
            }
        })?;
        let HirExprKind::LifetimePath(path) = first.kind() else {
            // Only the static-scope grammar reserves the receiver spelling.
            // Ordinary project methods can use the same name with values.
            return Ok(None);
        };
        if self.form() != HirCallInvocationForm::Parenthesized
            || self.terminator() != HirCallArgumentListTerminator::Closed
            || self.explicit_type_application() != &HirCallTypeApplication::Absent
        {
            return Err(HirLifetimePromotionError::InvalidArguments);
        }
        let [
            HirCallArgument::Positional {
                value:
                    HirCallValue::Present {
                        value: target_source,
                    },
            },
            remaining @ ..,
        ] = self.arguments()
        else {
            return Err(HirLifetimePromotionError::InvalidArguments);
        };
        let target = path.as_resolved().and_then(|path| path.scope_selector());
        let target = target.ok_or(HirLifetimePromotionError::InvalidTarget {
            operand: *target_source,
        })?;
        let authorization = match (audited, remaining) {
            (true, []) => HirLifetimePromotionAuthorization::Audited,
            (false, []) => HirLifetimePromotionAuthorization::Safe { proof: None },
            (
                false,
                [
                    HirCallArgument::Named {
                        name,
                        equals: HirRequiredTokenState::Present,
                        value: HirCallValue::Present { value },
                    },
                ],
            ) if name.resolved().is_some_and(|name| name.as_str() == "proof") => {
                let expression = module.resolve_expr(*value).map_err(|_| {
                    HirLifetimePromotionError::UnresolvedOperand { operand: *value }
                })?;
                let proof = match expression.kind() {
                    HirExprKind::EntityReference(reference) => {
                        reference.as_resolved().filter(|reference| {
                            reference
                                .declaration_public_id(DeclarationIdentityFamily::Proof)
                                .is_some()
                        })
                    }
                    _ => None,
                };
                let proof =
                    proof.ok_or(HirLifetimePromotionError::InvalidProof { operand: *value })?;
                HirLifetimePromotionAuthorization::Safe {
                    proof: Some((*value, proof.clone())),
                }
            }
            _ => return Err(HirLifetimePromotionError::InvalidArguments),
        };
        Ok(Some(HirLifetimePromotion {
            receiver: *value_receiver,
            target_source: *target_source,
            target: target.clone(),
            authorization,
        }))
    }
}

impl HirLifetimePromotion {
    pub const fn receiver(&self) -> ExprId {
        self.receiver
    }

    pub const fn target_source(&self) -> ExprId {
        self.target_source
    }

    pub const fn target(&self) -> &LifetimeScopeKind {
        &self.target
    }

    pub const fn authorization(&self) -> &HirLifetimePromotionAuthorization {
        &self.authorization
    }
}
