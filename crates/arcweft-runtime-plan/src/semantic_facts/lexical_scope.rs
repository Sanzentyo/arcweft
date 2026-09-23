//! Exact accepted lexical identity at the runtime semantic projection boundary.

use arcweft_core::scope::RuntimeScopeIdentity;
use arcweft_lang_hir::expr::HirNamedBlockName;
use arcweft_lang_hir::identity::ScopeId;
use arcweft_lang_hir::scope::HirScopeKind;

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
    expressions: &BTreeMap<ExprId, RuntimeScopeFact>,
    statements: &BTreeMap<StmtId, RuntimeScopeFact>,
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
        validate_expression_scope(resolve_expr(modules, *owner)?, *owner, identity.identity())?;
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
        validate_statement_scope(resolve_stmt(modules, *owner)?, *owner, identity.identity())?;
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

/// A continuation is admitted only when it names every outward Try in this
/// lexical execution and preserves the exact enclosing residual contract.
pub(super) fn validate_continuation<'a>(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    owner: RuntimeScopeOwner,
    fact: &RuntimeScopeFact,
    value_type: Option<&RuntimeNormalizedType>,
    tries: impl Iterator<Item = (ExprId, &'a RuntimeTryFact)>,
) -> Result<(), RuntimeSemanticFactsError> {
    let invalid = || RuntimeSemanticFactsError::InvalidScopeContinuation { owner };
    let (module_id, scope) = match owner {
        RuntimeScopeOwner::Expression(expression) => {
            let HirExprKind::NamedBlock(block) = resolve_expr(modules, expression)? else {
                return Err(invalid());
            };
            (expression.module(), block.scope())
        }
        RuntimeScopeOwner::Statement(statement) => {
            let HirStmtKind::Scope(block) = resolve_stmt(modules, statement)? else {
                return Err(invalid());
            };
            (statement.module(), block.body().scope())
        }
    };
    let module = module_for(modules, module_id)?;
    let mut exits = Vec::new();
    for (expression, tried) in tries {
        if expression.module() != module_id
            || tried.boundary() == RuntimeTryBoundaryOwner::Infallible
        {
            continue;
        }
        let hir = module.resolve_expr(expression).map_err(|_| invalid())?;
        if !contains_execution_scope(module, hir.scope(), scope).ok_or_else(invalid)? {
            continue;
        }
        let target = match tried.boundary() {
            RuntimeTryBoundaryOwner::CarrierBlock(owner)
            | RuntimeTryBoundaryOwner::ExplicitFunctionSite(owner)
            | RuntimeTryBoundaryOwner::ImplicitFunctionSite(owner) => Some(owner),
            RuntimeTryBoundaryOwner::Callable(_) | RuntimeTryBoundaryOwner::Infallible => None,
        };
        if let Some(target) = target {
            let target = module.resolve_expr(target).map_err(|_| invalid())?;
            if contains_execution_scope(module, target.scope(), scope).ok_or_else(invalid)? {
                continue;
            }
        }
        let continuation = fact.continuation().ok_or_else(invalid)?;
        if continuation.boundary() != tried.boundary()
            || continuation.boundary_type() != tried.boundary_type()
        {
            return Err(invalid());
        }
        exits.push(expression);
    }
    let Some(continuation) = fact.continuation() else {
        return if exits.is_empty() {
            Ok(())
        } else {
            Err(invalid())
        };
    };
    exits.sort_unstable();
    if exits.as_slice() != continuation.exits() {
        return Err(invalid());
    }
    match owner {
        RuntimeScopeOwner::Expression(_) if value_type != Some(continuation.value_type()) => {
            return Err(invalid());
        }
        RuntimeScopeOwner::Statement(_)
            if !matches!(continuation.value_type().shape(), RuntimeTypeShape::Unit) =>
        {
            return Err(invalid());
        }
        _ => {}
    }
    validate_normalized_type(modules, continuation.carrier_type())?;
    validate_normalized_type(modules, continuation.boundary_type())?;
    Ok(())
}

fn contains_execution_scope(
    module: &HirModule,
    mut scope: ScopeId,
    ancestor: ScopeId,
) -> Option<bool> {
    loop {
        if scope == ancestor {
            return Some(true);
        }
        let current = module.resolve_scope(scope).ok()?;
        if matches!(
            current.kind(),
            HirScopeKind::Callable | HirScopeKind::Flow | HirScopeKind::Closure
        ) {
            return Some(false);
        }
        let Some(parent) = current.parent() else {
            return Some(false);
        };
        scope = parent;
    }
}
