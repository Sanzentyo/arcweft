use super::{
    HirRuntimeExecutableOwner, HirRuntimeReachabilityEdge, HirRuntimeReachabilityEdgeKind,
    HirRuntimeReachabilityError, HirRuntimeReachabilityRoot, HirRuntimeReachabilityRootKind,
    HirRuntimeReachabilitySite, HirRuntimeSemanticReachabilityInput, execution_roots,
    resolve_item_kind,
};
use crate::{
    expr::HirExprKind,
    identity::{ExprId, ItemId, StmtId},
    item::{HirImplMember, HirItemKind},
    project::HirExecutableProjectView,
    stmt::HirStmtKind,
    symbol::{ImplMethodDeclarationId, ImplMethodKind},
};

pub(super) fn validate_roots_and_edges(
    project: HirExecutableProjectView<'_>,
    input: &HirRuntimeSemanticReachabilityInput,
) -> Result<(), HirRuntimeReachabilityError> {
    for root in &input.roots {
        execution_roots(project, &root.owner).map_err(|_| {
            HirRuntimeReachabilityError::UnknownRoot {
                owner: root.owner.clone(),
            }
        })?;
        if !root_kind_matches(project, root) {
            return Err(HirRuntimeReachabilityError::InvalidRootKind { root: root.clone() });
        }
    }
    for edge in &input.edges {
        validate_site(project, edge.source)?;
        if !edge_source_family_matches(project, edge) {
            return Err(HirRuntimeReachabilityError::InvalidEdgeKind {
                site: edge.source,
                kind: Box::new(edge.kind.clone()),
            });
        }
        execution_roots(project, &edge.target).map_err(|error| match error {
            HirRuntimeReachabilityError::PresentationTarget { .. } => error,
            _ => HirRuntimeReachabilityError::UnknownEdgeTarget {
                target: edge.target.clone(),
            },
        })?;
        if !edge_kind_matches_target(project, edge) {
            return Err(HirRuntimeReachabilityError::InvalidEdgeTarget {
                edge: Box::new(edge.clone()),
            });
        }
    }
    Ok(())
}

fn edge_source_family_matches(
    project: HirExecutableProjectView<'_>,
    edge: &HirRuntimeReachabilityEdge,
) -> bool {
    match (&edge.source, &edge.kind) {
        (
            HirRuntimeReachabilitySite::Expression(owner),
            HirRuntimeReachabilityEdgeKind::CheckedProjectCall { .. }
            | HirRuntimeReachabilityEdgeKind::CheckedTraitMethodCall { .. },
        ) => resolve_expression_kind(project, *owner)
            .is_some_and(|kind| matches!(kind, HirExprKind::Call(_))),
        (
            HirRuntimeReachabilitySite::Statement(owner),
            HirRuntimeReachabilityEdgeKind::CheckedIteratorWitnessMethod { .. },
        ) => resolve_statement_kind(project, *owner)
            .is_some_and(|kind| matches!(kind, HirStmtKind::For(_))),
        (
            HirRuntimeReachabilitySite::Expression(owner),
            HirRuntimeReachabilityEdgeKind::CheckedFlowTransfer { .. },
        ) => resolve_expression_kind(project, *owner)
            .is_some_and(|kind| matches!(kind, HirExprKind::Choice(_))),
        (
            HirRuntimeReachabilitySite::Statement(owner),
            HirRuntimeReachabilityEdgeKind::CheckedFlowTransfer { .. },
        ) => resolve_statement_kind(project, *owner)
            .is_some_and(|kind| matches!(kind, HirStmtKind::Goto { .. })),
        (
            HirRuntimeReachabilitySite::Item(owner),
            HirRuntimeReachabilityEdgeKind::CheckedEntryBinding { .. },
        ) => resolve_item_kind(project, *owner)
            .is_some_and(|kind| matches!(kind, HirItemKind::Entry(_))),
        _ => false,
    }
}

fn resolve_expression_kind(
    project: HirExecutableProjectView<'_>,
    owner: ExprId,
) -> Option<&HirExprKind> {
    project
        .modules()
        .find_map(|(_, module)| (module.module_id() == owner.module()).then_some(module.as_ref()))?
        .resolve_expr(owner)
        .ok()
        .map(crate::expr::HirExpr::kind)
}

fn resolve_statement_kind(
    project: HirExecutableProjectView<'_>,
    owner: StmtId,
) -> Option<&HirStmtKind> {
    project
        .modules()
        .find_map(|(_, module)| (module.module_id() == owner.module()).then_some(module.as_ref()))?
        .resolve_stmt(owner)
        .ok()
        .map(crate::stmt::HirStmt::kind)
}

fn edge_kind_matches_target(
    project: HirExecutableProjectView<'_>,
    edge: &HirRuntimeReachabilityEdge,
) -> bool {
    match (&edge.kind, &edge.target) {
        (
            HirRuntimeReachabilityEdgeKind::CheckedTraitMethodCall {
                implementation,
                method,
                ..
            },
            HirRuntimeExecutableOwner::ImplMethod(target),
        ) => {
            method == target
                && impl_method_implementation_owner(project, method) == Some(*implementation)
        }
        (
            HirRuntimeReachabilityEdgeKind::CheckedIteratorWitnessMethod {
                implementation,
                member,
                method,
                ..
            },
            HirRuntimeExecutableOwner::ImplMethod(target),
        ) => {
            method == target
                && iterator_witness_method_matches(project, *implementation, *member, method)
        }
        (
            HirRuntimeReachabilityEdgeKind::CheckedTraitMethodCall { .. }
            | HirRuntimeReachabilityEdgeKind::CheckedIteratorWitnessMethod { .. },
            _,
        ) => false,
        _ => true,
    }
}

fn iterator_witness_method_matches(
    project: HirExecutableProjectView<'_>,
    implementation_owner: ItemId,
    member: u16,
    method: &ImplMethodDeclarationId,
) -> bool {
    if method.kind() != ImplMethodKind::Trait {
        return false;
    }
    let declaration = method.implementation();
    let Some(module) = project
        .modules()
        .find_map(|(path, module)| (path == declaration.module()).then_some(module.as_ref()))
    else {
        return false;
    };
    let Ok(ordinal) = usize::try_from(declaration.source_ordinal()) else {
        return false;
    };
    let Some((actual_owner, implementation)) = module
        .items()
        .filter_map(|(owner, item)| match item.kind() {
            HirItemKind::Impl(implementation) => Some((owner, implementation)),
            _ => None,
        })
        .nth(ordinal)
    else {
        return false;
    };
    if actual_owner != implementation_owner {
        return false;
    }
    let Some(HirImplMember::Function(function)) = implementation.members().get(usize::from(member))
    else {
        return false;
    };
    if function
        .name()
        .resolved()
        .is_none_or(|name| name.as_str() != method.method().as_str())
        || function.body().is_none()
    {
        return false;
    }
    true
}

fn impl_method_implementation_owner(
    project: HirExecutableProjectView<'_>,
    method: &ImplMethodDeclarationId,
) -> Option<ItemId> {
    let implementation = method.implementation();
    let module = project
        .modules()
        .find_map(|(path, module)| (path == implementation.module()).then_some(module.as_ref()))?;
    let ordinal = usize::try_from(implementation.source_ordinal()).ok()?;
    module
        .items()
        .filter(|(_, item)| matches!(item.kind(), HirItemKind::Impl(_)))
        .nth(ordinal)
        .map(|(owner, _)| owner)
}

fn root_kind_matches(
    project: HirExecutableProjectView<'_>,
    root: &HirRuntimeReachabilityRoot,
) -> bool {
    match (root.kind, &root.owner) {
        (HirRuntimeReachabilityRootKind::CheckedFlow, HirRuntimeExecutableOwner::Item(owner)) => {
            resolve_item_kind(project, *owner)
                .is_some_and(|kind| matches!(kind, HirItemKind::Flow(_)))
        }
        (
            HirRuntimeReachabilityRootKind::CheckedEntry
            | HirRuntimeReachabilityRootKind::SelectedEntry,
            HirRuntimeExecutableOwner::Item(owner),
        ) => resolve_item_kind(project, *owner)
            .is_some_and(|kind| matches!(kind, HirItemKind::Entry(_))),
        (
            HirRuntimeReachabilityRootKind::CheckedViewValueProgram,
            HirRuntimeExecutableOwner::Closure(owner),
        ) => resolve_expression_kind(project, *owner)
            .is_some_and(|kind| matches!(kind, HirExprKind::Closure(_))),
        _ => false,
    }
}

fn validate_site(
    project: HirExecutableProjectView<'_>,
    site: HirRuntimeReachabilitySite,
) -> Result<(), HirRuntimeReachabilityError> {
    let resolved = match site {
        HirRuntimeReachabilitySite::Item(owner) => resolve_item_kind(project, owner).is_some(),
        HirRuntimeReachabilitySite::Expression(owner) => project
            .modules()
            .find(|(_, module)| module.module_id() == owner.module())
            .is_some_and(|(_, module)| module.resolve_expr(owner).is_ok()),
        HirRuntimeReachabilitySite::Statement(owner) => project
            .modules()
            .find(|(_, module)| module.module_id() == owner.module())
            .is_some_and(|(_, module)| module.resolve_stmt(owner).is_ok()),
    };
    if resolved {
        Ok(())
    } else {
        Err(HirRuntimeReachabilityError::UnknownEdgeSource { site })
    }
}
