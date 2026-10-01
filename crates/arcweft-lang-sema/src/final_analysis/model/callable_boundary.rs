//! Shared callable frame identity for Return and Try propagation.

use super::{CallableDeclarationKey, CheckedImplicitCallableIdentity, ExprId};
use crate::semantic_coordinate::{
    AcceptedDeclarationSemanticId, CheckedExpressionCoordinateEvidence, CheckedSemanticPath,
};

/// Generation-bound expression evidence paired with its accepted semantic
/// coordinate. The raw owner is retained only for runtime/CPS validation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedExpressionBoundary {
    lookup_owner: ExprId,
    coordinate: CheckedSemanticPath,
}

impl CheckedExpressionBoundary {
    pub(crate) fn from_evidence(evidence: CheckedExpressionCoordinateEvidence) -> Self {
        Self {
            lookup_owner: evidence.owner(),
            coordinate: evidence.into_coordinate(),
        }
    }

    pub const fn lookup_owner(&self) -> ExprId {
        self.lookup_owner
    }

    pub const fn coordinate(&self) -> &CheckedSemanticPath {
        &self.coordinate
    }
}

/// Function-site invocation boundary. Explicit closure sites carry only their
/// accepted expression boundary; implicit sites additionally join the
/// callable identity issued by the owner-bound seal.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedFunctionSiteBoundary {
    Explicit(CheckedExpressionBoundary),
    Implicit {
        site: CheckedExpressionBoundary,
        callable: CheckedImplicitCallableIdentity,
    },
}

impl CheckedFunctionSiteBoundary {
    pub const fn site(&self) -> &CheckedExpressionBoundary {
        match self {
            Self::Explicit(site) => site,
            Self::Implicit { site, .. } => site,
        }
    }

    pub const fn callable(&self) -> Option<CheckedImplicitCallableIdentity> {
        match self {
            Self::Explicit(_) => None,
            Self::Implicit { callable, .. } => Some(*callable),
        }
    }
}

/// Accepted callable declaration receiving a return value or Try residual. The declaration
/// key remains the typed catalog lookup; the accepted semantic ID is the
/// stable root identity consumed by transcript/runtime authorities.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedCallableDeclarationBoundary {
    declaration: CallableDeclarationKey,
    accepted: AcceptedDeclarationSemanticId,
}

impl CheckedCallableDeclarationBoundary {
    pub(crate) fn new(
        declaration: CallableDeclarationKey,
        accepted: AcceptedDeclarationSemanticId,
    ) -> Self {
        Self {
            declaration,
            accepted,
        }
    }

    pub const fn declaration(&self) -> &CallableDeclarationKey {
        &self.declaration
    }

    pub const fn accepted(&self) -> AcceptedDeclarationSemanticId {
        self.accepted
    }
}

/// One accepted callable frame. Return and Try propagation share this authority;
/// carrier blocks remain a separate Try boundary and never receive Return.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedCallableBoundary {
    FunctionSite(CheckedFunctionSiteBoundary),
    Declaration(CheckedCallableDeclarationBoundary),
}

impl CheckedCallableBoundary {
    /// Selects a Return frame from HIR structural context and the complete
    /// selected callable facts. Implicit bodies use their sealed region;
    /// lexical item ownership alone never turns a value root into a frame.
    pub(in crate::final_analysis) fn for_return(
        module: &arcweft_lang_hir::module::HirModule,
        owner: arcweft_lang_hir::identity::StmtId,
        value: ExprId,
        expressions: &std::collections::BTreeMap<ExprId, super::CheckedExpression>,
        callables: &crate::callable::CheckedCallableCatalog,
        coordinates: &crate::semantic_coordinate::SemanticCoordinateIndex<'_, '_>,
    ) -> Result<(Self, Option<crate::types::TypeKind>), super::super::FinalSemanticAnalysisError>
    {
        use super::super::{CheckedExpressionResolution, FinalSemanticAnalysisError};
        use arcweft_lang_hir::project::{HirReturnContext, HirSemanticPathRoot};
        let unavailable = || FinalSemanticAnalysisError::ReturnBoundaryUnavailable { owner };
        let (context, root) = coordinates
            .return_context(owner)
            .map_err(|_| unavailable())?;
        let implicit = coordinates
            .enclosing_implicit_callable(value, |candidate| {
                matches!(
                    expressions
                        .get(&candidate)
                        .map(super::CheckedExpression::resolution),
                    Some(CheckedExpressionResolution::ImplicitCallable(_))
                )
            })
            .map_err(|_| unavailable())?;
        if let Some(site) = implicit {
            let Some(CheckedExpressionResolution::ImplicitCallable(callable)) = expressions
                .get(&site)
                .map(super::CheckedExpression::resolution)
            else {
                return Err(unavailable());
            };
            return Ok((
                Self::FunctionSite(CheckedFunctionSiteBoundary::Implicit {
                    site: CheckedExpressionBoundary::from_evidence(
                        coordinates
                            .expression_evidence(site)
                            .map_err(|_| unavailable())?,
                    ),
                    callable: callable.identity(),
                }),
                Some(callable.result().clone()),
            ));
        }
        match context {
            HirReturnContext::FunctionSite(site) => {
                let expression = expressions.get(&site).ok_or_else(unavailable)?;
                if !matches!(
                    module.resolve_expr(site).map_err(|_| unavailable())?.kind(),
                    arcweft_lang_hir::expr::HirExprKind::Closure(_)
                ) {
                    return Err(unavailable());
                }
                let result = match expression.result() {
                    super::CheckedExpressionResult::Value(result) => {
                        let crate::types::TypeKind::Function { return_type, .. } = result.ty()
                        else {
                            return Err(unavailable());
                        };
                        Some(return_type.as_ref().clone())
                    }
                    // Rejected closure creation still has a structural Return
                    // frame for tooling. It supplies no result-type proof and
                    // cannot satisfy closed execution-root admission.
                    super::CheckedExpressionResult::Unavailable => None,
                    super::CheckedExpressionResult::NonValue(_) => return Err(unavailable()),
                };
                Ok((
                    Self::FunctionSite(CheckedFunctionSiteBoundary::Explicit(
                        CheckedExpressionBoundary::from_evidence(
                            coordinates
                                .expression_evidence(site)
                                .map_err(|_| unavailable())?,
                        ),
                    )),
                    result,
                ))
            }
            HirReturnContext::Item(item) => {
                // Predicate, Proof and View item containers are not Return
                // frames. A selected implicit callable above can own their body.
                if !matches!(
                    module.resolve_item(item).map_err(|_| unavailable())?.kind(),
                    arcweft_lang_hir::item::HirItemKind::Function(_)
                        | arcweft_lang_hir::item::HirItemKind::Flow(_)
                        | arcweft_lang_hir::item::HirItemKind::Impl(_)
                ) {
                    return Err(unavailable());
                }
                let HirSemanticPathRoot::Declaration(declaration) = root else {
                    return Err(unavailable());
                };
                let facts = callables
                    .project_callable(declaration)
                    .map_err(|_| unavailable())?;
                let result = facts
                    .signature()
                    .value_type()
                    .ok_or_else(unavailable)?
                    .clone();
                let accepted = coordinates
                    .accepted_declaration(declaration)
                    .map_err(|_| unavailable())?;
                Ok((
                    Self::Declaration(CheckedCallableDeclarationBoundary::new(
                        declaration.clone(),
                        accepted,
                    )),
                    Some(result),
                ))
            }
        }
    }
}
