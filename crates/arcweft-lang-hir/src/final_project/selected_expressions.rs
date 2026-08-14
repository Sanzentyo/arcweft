//! Selected expression-owner inventory for one executable final-HIR project.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::dialogue_application::HirPostfixBracketCandidates;
use crate::expr::HirExprKind;
use crate::identity::{ExprId, HirModuleId};
use crate::module::HirModule;

use super::HirExecutableProjectView;

/// Failure to resolve the expression owners selected by a higher-layer
/// postfix decision.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirSelectedExpressionInventoryError {
    #[error("selected expression owner references unknown HIR module {module:?}")]
    UnknownModule { module: HirModuleId },
    #[error("selected expression owner references unresolved expression {expression:?}")]
    UnresolvedExpression { expression: ExprId },
    #[error("postfix expression {expression:?} has no selected candidate")]
    MissingPostfixSelection { expression: ExprId },
    #[error(
        "expression {candidate:?} is not one of postfix expression {expression:?}'s candidates"
    )]
    InvalidPostfixSelection {
        expression: ExprId,
        candidate: ExprId,
    },
}

impl HirExecutableProjectView<'_> {
    /// Returns the exact expression owners reachable after bounded postfix
    /// ambiguity has been resolved by the supplied accepted decisions.
    ///
    /// HIR owns graph traversal and candidate membership. The callback remains
    /// the sole higher-layer authority for which candidate semantic analysis
    /// accepted; this method neither infers nor stores that decision.
    pub fn selected_expression_owners(
        self,
        mut selected_postfix: impl FnMut(ExprId) -> Option<ExprId>,
    ) -> Result<BTreeSet<ExprId>, HirSelectedExpressionInventoryError> {
        let modules = self
            .modules()
            .map(|(_, module)| (module.module_id(), module.as_ref()))
            .collect::<BTreeMap<_, _>>();
        let all = modules
            .values()
            .flat_map(|module| module.expressions().map(|(owner, _)| owner))
            .collect::<BTreeSet<_>>();
        let children = modules
            .values()
            .flat_map(|module| module.expressions().map(|(_, expression)| expression))
            .flat_map(|expression| expression.kind().direct_expression_children())
            .collect::<BTreeSet<_>>();
        let mut pending = all.difference(&children).copied().collect::<Vec<_>>();
        let mut selected = BTreeSet::new();

        while let Some(owner) = pending.pop() {
            if !selected.insert(owner) {
                continue;
            }
            match resolve_expression(&modules, owner)? {
                HirExprKind::PostfixBracket(postfix) => {
                    pending.push(postfix.target());
                    let candidate = selected_postfix(owner).ok_or(
                        HirSelectedExpressionInventoryError::MissingPostfixSelection {
                            expression: owner,
                        },
                    )?;
                    let admissible = matches!(
                        postfix.candidates(),
                        HirPostfixBracketCandidates::Ambiguous { index, dialogue }
                            if candidate == *index || candidate == *dialogue
                    );
                    if !admissible {
                        return Err(
                            HirSelectedExpressionInventoryError::InvalidPostfixSelection {
                                expression: owner,
                                candidate,
                            },
                        );
                    }
                    pending.push(candidate);
                }
                kind => pending.extend(kind.direct_expression_children()),
            }
        }
        Ok(selected)
    }
}

fn resolve_expression<'project>(
    modules: &BTreeMap<HirModuleId, &'project HirModule>,
    expression: ExprId,
) -> Result<&'project HirExprKind, HirSelectedExpressionInventoryError> {
    modules
        .get(&expression.module())
        .copied()
        .ok_or(HirSelectedExpressionInventoryError::UnknownModule {
            module: expression.module(),
        })?
        .resolve_expr(expression)
        .map(crate::expr::HirExpr::kind)
        .map_err(|_| HirSelectedExpressionInventoryError::UnresolvedExpression { expression })
}
