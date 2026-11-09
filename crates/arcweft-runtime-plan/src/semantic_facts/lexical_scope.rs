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
    let matches_source = match (kind, identity) {
        (HirExprKind::NamedBlock(block), RuntimeScopeIdentity::Named(name)) => {
            matches!(block.name(), HirNamedBlockName::Resolved(authored) if authored.as_str() == name.as_str())
        }
        (HirExprKind::NamedBlock(_), RuntimeScopeIdentity::Anonymous) => false,
        (kind, RuntimeScopeIdentity::Anonymous) => kind.value_block_scope().is_some(),
        _ => false,
    };
    if matches_source {
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

pub(super) fn validate_semantic_result(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    owner: ExprId,
    fact: &RuntimeScopeFact,
    semantic: bool,
) -> Result<(), RuntimeSemanticFactsError> {
    match (semantic, fact.semantic_unit()) {
        (true, Some(unit)) if matches!(unit.shape(), RuntimeTypeShape::Unit) => {
            validate_normalized_type(modules, unit)
        }
        (false, None) => Ok(()),
        _ => Err(RuntimeSemanticFactsError::InvalidExpressionScope { expression: owner }),
    }
}

pub(super) fn validate_carrier_block(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    owner: ExprId,
    scope: &RuntimeScopeFact,
    callable: Option<&RuntimeImplicitCallableFact>,
    value_type: Option<&RuntimeNormalizedType>,
) -> Result<(), RuntimeSemanticFactsError> {
    use arcweft_lang_hir::expr::HirComputationBlockKind;
    let kind = resolve_expr(modules, owner)?;
    let invalid = || RuntimeSemanticFactsError::InvalidExpressionScope { expression: owner };
    let HirExprKind::ComputationBlock(block) = kind else {
        return if scope.carrier_block().is_none() {
            Ok(())
        } else {
            Err(invalid())
        };
    };
    let carrier = scope.carrier_block().ok_or_else(invalid)?;
    let ty = carrier.carrier_type();
    if scope.semantic_unit().is_some()
        || !matches!(
            (block.kind(), ty.shape()),
            (
                HirComputationBlockKind::Option,
                RuntimeTypeShape::Option { .. }
            ) | (
                HirComputationBlockKind::Result,
                RuntimeTypeShape::Result { .. }
            )
        )
    {
        return Err(invalid());
    }
    if carrier.delivers_value() {
        if callable
            .map(RuntimeImplicitCallableFact::result)
            .or(value_type)
            != Some(ty)
        {
            return Err(invalid());
        }
    } else if callable.is_none()
        && !value_type.is_some_and(|ty| matches!(ty.shape(), RuntimeTypeShape::Never))
    {
        return Err(invalid());
    }
    validate_normalized_type(modules, ty)
}

pub(super) fn validate_global_scopes(
    project: arcweft_lang_hir::project::HirAnalysisProjectView<'_>,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    owners: RuntimeSemanticOwnerSet<'_>,
    instance_expressions: &BTreeSet<ExprId>,
    instance_statements: &BTreeSet<StmtId>,
    expressions: &BTreeMap<ExprId, RuntimeScopeFact>,
    statements: &BTreeMap<StmtId, RuntimeScopeFact>,
) -> Result<(), RuntimeSemanticFactsError> {
    for (owner, identity) in expressions {
        if !identity
            .origin()
            .validate_owner(project, RuntimeScopeOwner::Expression(*owner))
        {
            return Err(RuntimeSemanticFactsError::InvalidScopeOrigin {
                owner: RuntimeScopeOwner::Expression(*owner),
            });
        }
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
            |kind| kind.value_block_scope().is_some(),
        )?;
        validate_expression_scope(resolve_expr(modules, *owner)?, *owner, identity.identity())?;
    }
    for owner in owners.expressions() {
        if !instance_expressions.contains(&owner)
            && resolve_expr(modules, owner)?.value_block_scope().is_some()
            && !expressions.contains_key(&owner)
        {
            return Err(RuntimeSemanticFactsError::InvalidExpressionScope { expression: owner });
        }
    }
    for (owner, identity) in statements {
        if !identity
            .origin()
            .validate_owner(project, RuntimeScopeOwner::Statement(*owner))
        {
            return Err(RuntimeSemanticFactsError::InvalidScopeOrigin {
                owner: RuntimeScopeOwner::Statement(*owner),
            });
        }
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
            let scope = resolve_expr(modules, expression)?
                .value_block_scope()
                .ok_or_else(invalid)?;
            (expression.module(), scope)
        }
        RuntimeScopeOwner::Statement(statement) => {
            if fact.semantic_unit().is_some() || fact.carrier_block().is_some() {
                return Err(invalid());
            }
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
            RuntimeTryBoundaryOwner::CarrierBlock(owner) => Some(
                resolve_expr(modules, owner)?
                    .value_block_scope()
                    .ok_or_else(invalid)?,
            ),
            RuntimeTryBoundaryOwner::Thread(owner)
            | RuntimeTryBoundaryOwner::ExplicitFunctionSite(owner)
            | RuntimeTryBoundaryOwner::ImplicitFunctionSite(owner) => {
                Some(module.resolve_expr(owner).map_err(|_| invalid())?.scope())
            }
            RuntimeTryBoundaryOwner::Callable(_) | RuntimeTryBoundaryOwner::Infallible => None,
        };
        if let Some(target) = target {
            if contains_execution_scope(module, target, scope).ok_or_else(invalid)? {
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
