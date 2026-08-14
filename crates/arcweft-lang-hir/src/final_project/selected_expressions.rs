//! Selected expression-owner inventory for one executable final-HIR project.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::dialogue_application::HirPostfixBracketCandidates;
use crate::expr::{HirCallArgument, HirExprKind};
use crate::identity::{ExprId, HirModuleId};
use crate::module::HirModule;

use super::{HirExecutableProjectView, HirRuntimeSemanticOwnerInventory};

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
    #[error(
        "expression {expression:?} was classified as a non-value call carrier but is not a Call"
    )]
    InvalidNonValueCallCarrier { expression: ExprId },
    #[error("non-value call carrier {expression:?} requires a runtime receiver but has none")]
    MissingRuntimeCallReceiver { expression: ExprId },
}

/// Accepted use of a final-HIR call callee in runtime lowering.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeCallCalleeDisposition {
    /// The accepted target is selected statically, so the callee subtree is
    /// not a runtime value operand.
    Static,
    /// The accepted call arguments include the call's value receiver.
    RuntimeReceiver,
}

/// Accepted higher-layer disposition of one HIR expression at the runtime
/// type-fact boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeExpressionTypeDisposition {
    /// The expression produces a runtime value whose accepted type is retained.
    Retain,
    /// A selected call lowers as a non-value carrier. Its authored arguments
    /// remain live, while its callee follows the accepted dispatch use.
    NonValueCallCarrier {
        callee: HirRuntimeCallCalleeDisposition,
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
        selected_postfix: impl FnMut(ExprId) -> Option<ExprId>,
    ) -> Result<BTreeSet<ExprId>, HirSelectedExpressionInventoryError> {
        self.selected_expression_owners_in_domain(
            SelectedExpressionDomain::SemanticAnalysis,
            None,
            selected_postfix,
            |_| HirRuntimeExpressionTypeDisposition::Retain,
        )
    }

    fn selected_expression_owners_in_domain(
        self,
        domain: SelectedExpressionDomain,
        outer_owners: Option<&BTreeSet<ExprId>>,
        mut selected_postfix: impl FnMut(ExprId) -> Option<ExprId>,
        mut expression_disposition: impl FnMut(ExprId) -> HirRuntimeExpressionTypeDisposition,
    ) -> Result<BTreeSet<ExprId>, HirSelectedExpressionInventoryError> {
        let modules = self
            .modules()
            .map(|(_, module)| (module.module_id(), module.as_ref()))
            .collect::<BTreeMap<_, _>>();
        let all = modules
            .values()
            .flat_map(|module| module.expressions().map(|(owner, _)| owner))
            .filter(|owner| outer_owners.is_none_or(|outer| outer.contains(owner)))
            .collect::<BTreeSet<_>>();
        let children = modules
            .values()
            .flat_map(|module| module.expressions())
            .filter(|(owner, _)| outer_owners.is_none_or(|outer| outer.contains(owner)))
            .map(|(_, expression)| expression)
            .flat_map(|expression| expression.kind().direct_expression_children())
            .filter(|owner| outer_owners.is_none_or(|outer| outer.contains(owner)))
            .collect::<BTreeSet<_>>();
        let mut pending = all.difference(&children).copied().collect::<Vec<_>>();
        let excluded_roots = if domain == SelectedExpressionDomain::RuntimeType {
            self.items()
                .flat_map(|item| item.item().kind().effect_expression_roots())
                .collect::<BTreeSet<_>>()
        } else {
            BTreeSet::new()
        };
        let mut visited = BTreeSet::new();
        let mut selected = BTreeSet::new();

        while let Some(owner) = pending.pop() {
            if !visited.insert(owner)
                || excluded_roots.contains(&owner)
                || outer_owners.is_some_and(|outer| !outer.contains(&owner))
            {
                continue;
            }
            let kind = resolve_expression(&modules, owner)?;
            let disposition = if domain == SelectedExpressionDomain::RuntimeType {
                expression_disposition(owner)
            } else {
                HirRuntimeExpressionTypeDisposition::Retain
            };
            if let HirRuntimeExpressionTypeDisposition::NonValueCallCarrier { callee } = disposition
            {
                append_non_value_call_operands(&modules, owner, kind, callee, &mut pending)?;
                continue;
            }
            match kind {
                HirExprKind::PostfixBracket(postfix) => {
                    let candidate = selected_postfix(owner).ok_or(
                        HirSelectedExpressionInventoryError::MissingPostfixSelection {
                            expression: owner,
                        },
                    )?;
                    let selected_index = match postfix.candidates() {
                        HirPostfixBracketCandidates::Ambiguous { index, dialogue }
                            if candidate == *index || candidate == *dialogue =>
                        {
                            candidate == *index
                        }
                        HirPostfixBracketCandidates::Ambiguous { .. }
                        | HirPostfixBracketCandidates::Invalid { .. } => {
                            return Err(
                                HirSelectedExpressionInventoryError::InvalidPostfixSelection {
                                    expression: owner,
                                    candidate,
                                },
                            );
                        }
                    };
                    match domain {
                        SelectedExpressionDomain::SemanticAnalysis => {
                            selected.insert(owner);
                            pending.extend([postfix.target(), candidate]);
                        }
                        SelectedExpressionDomain::RuntimeType => {
                            if selected_index {
                                selected.insert(owner);
                            }
                            pending.extend([postfix.target(), candidate]);
                        }
                    }
                }
                kind @ HirExprKind::DialogueContentApplication(_)
                    if domain == SelectedExpressionDomain::RuntimeType =>
                {
                    pending.extend(kind.direct_expression_children());
                }
                kind => {
                    selected.insert(owner);
                    pending.extend(kind.direct_expression_children());
                }
            }
        }
        Ok(selected)
    }
}

impl HirRuntimeSemanticOwnerInventory<'_> {
    /// Returns the exact retained expression owners whose accepted types enter
    /// runtime lowering after bounded postfix ambiguity has been resolved.
    ///
    /// Effect metadata and non-value dialogue carrier nodes remain in the
    /// semantic inventory but do not publish runtime type facts. Their runtime
    /// operands are still traversed from the same HIR-owned graph authority.
    /// The second callback supplies the accepted use of selected call carriers;
    /// HIR validates that a call-only disposition cannot hide another family.
    pub fn selected_expression_type_owners(
        &self,
        selected_postfix: impl FnMut(ExprId) -> Option<ExprId>,
        expression_disposition: impl FnMut(ExprId) -> HirRuntimeExpressionTypeDisposition,
    ) -> Result<BTreeSet<ExprId>, HirSelectedExpressionInventoryError> {
        self.project.selected_expression_owners_in_domain(
            SelectedExpressionDomain::RuntimeType,
            Some(self.expression_owners()),
            selected_postfix,
            expression_disposition,
        )
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SelectedExpressionDomain {
    SemanticAnalysis,
    RuntimeType,
}

fn append_non_value_call_operands(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    owner: ExprId,
    kind: &HirExprKind,
    callee: HirRuntimeCallCalleeDisposition,
    pending: &mut Vec<ExprId>,
) -> Result<(), HirSelectedExpressionInventoryError> {
    let HirExprKind::Call(call) = kind else {
        return Err(
            HirSelectedExpressionInventoryError::InvalidNonValueCallCarrier { expression: owner },
        );
    };
    pending.extend(call.arguments().iter().map(HirCallArgument::value));
    if callee == HirRuntimeCallCalleeDisposition::Static {
        return Ok(());
    }
    let callee_owner = call.callee().value_expression().ok_or(
        HirSelectedExpressionInventoryError::MissingRuntimeCallReceiver { expression: owner },
    )?;
    let module = modules.get(&callee_owner.module()).copied().ok_or(
        HirSelectedExpressionInventoryError::UnknownModule {
            module: callee_owner.module(),
        },
    )?;
    let receiver = module
        .resolve_call_value_receiver(call)
        .map_err(
            |_| HirSelectedExpressionInventoryError::UnresolvedExpression {
                expression: callee_owner,
            },
        )?
        .ok_or(
            HirSelectedExpressionInventoryError::MissingRuntimeCallReceiver { expression: owner },
        )?;
    pending.push(receiver);
    Ok(())
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
