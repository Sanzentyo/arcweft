//! Exact accepted lexical identity at the runtime semantic projection boundary.

use arcweft_core::scope::RuntimeScopeIdentity;
use arcweft_lang_hir::expr::HirNamedBlockName;

use super::*;

pub(super) fn validate_expression_scope(
    kind: &HirExprKind,
    expression: ExprId,
    identity: &RuntimeScopeIdentity,
) -> Result<(), RuntimeSemanticFactsError> {
    if matches!((kind, identity),
        (HirExprKind::NamedBlock(block), RuntimeScopeIdentity::Named(name))
            if matches!(block.name(), HirNamedBlockName::Resolved(authored) if authored.as_str() == name.as_str())
    ) {
        Ok(())
    } else {
        Err(RuntimeSemanticFactsError::InvalidExpressionScope { expression })
    }
}

pub(super) fn validate_statement_scope(
    kind: &HirStmtKind,
    statement: StmtId,
    identity: &RuntimeScopeIdentity,
) -> Result<(), RuntimeSemanticFactsError> {
    if matches!(kind, HirStmtKind::Scope(scope)
        if scope.name().map(HirName::as_str) == identity.name().map(arcweft_id::DeclarationName::as_str))
    {
        Ok(())
    } else {
        Err(RuntimeSemanticFactsError::InvalidStatementScope { statement })
    }
}

pub(super) fn validate_global_scopes(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    owners: RuntimeSemanticOwnerSet<'_>,
    instance_expressions: &BTreeSet<ExprId>,
    instance_statements: &BTreeSet<StmtId>,
    expressions: &BTreeMap<ExprId, RuntimeScopeIdentity>,
    statements: &BTreeMap<StmtId, RuntimeScopeIdentity>,
) -> Result<(), RuntimeSemanticFactsError> {
    for (owner, identity) in expressions {
        if instance_expressions.contains(owner) {
            return Err(RuntimeSemanticFactsError::InstanceOwnedGlobalFact {
                family: RuntimeSemanticFactFamily::ExpressionScope,
            });
        }
        require_expr_family(
            modules,
            owners,
            *owner,
            RuntimeSemanticFactFamily::ExpressionScope,
            |kind| matches!(kind, HirExprKind::NamedBlock(_)),
        )?;
        validate_expression_scope(resolve_expr(modules, *owner)?, *owner, identity)?;
    }
    for owner in owners.expressions() {
        if !instance_expressions.contains(&owner)
            && matches!(resolve_expr(modules, owner)?, HirExprKind::NamedBlock(_))
            && !expressions.contains_key(&owner)
        {
            return Err(RuntimeSemanticFactsError::InvalidExpressionScope { expression: owner });
        }
    }
    for (owner, identity) in statements {
        if instance_statements.contains(owner) {
            return Err(RuntimeSemanticFactsError::InstanceOwnedGlobalFact {
                family: RuntimeSemanticFactFamily::StatementScope,
            });
        }
        require_stmt_family(
            modules,
            owners,
            *owner,
            RuntimeSemanticFactFamily::StatementScope,
            |kind| matches!(kind, HirStmtKind::Scope(_)),
        )?;
        validate_statement_scope(resolve_stmt(modules, *owner)?, *owner, identity)?;
    }
    for module in modules.values() {
        for (owner, statement) in module.statements() {
            if owners.contains_statement(owner)
                && !instance_statements.contains(&owner)
                && matches!(statement.kind(), HirStmtKind::Scope(_))
                && !statements.contains_key(&owner)
            {
                return Err(RuntimeSemanticFactsError::InvalidStatementScope { statement: owner });
            }
        }
    }
    Ok(())
}
