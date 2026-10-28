//! Admission of the exact checked operand of a suspension statement.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::{
    identity::{HirModuleId, StmtId},
    module::HirModule,
    stmt::HirStmtKind,
};
use arcweft_lang_sema::final_analysis::CheckedSuspensionStatement;

use super::{
    RuntimeExpressionTypeView, RuntimeSemanticFactFamily, RuntimeSemanticFactsError,
    RuntimeSemanticOwnerSet, RuntimeTypeShape, collect_unique, require_stmt_family, resolve_stmt,
};

pub(super) fn admit_global(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    owners: RuntimeSemanticOwnerSet<'_>,
    instance_owned: &BTreeSet<StmtId>,
    expression_types: &RuntimeExpressionTypeView<'_>,
    pending: Vec<(StmtId, CheckedSuspensionStatement)>,
) -> Result<BTreeMap<StmtId, CheckedSuspensionStatement>, RuntimeSemanticFactsError> {
    let suspensions = collect_unique(pending, RuntimeSemanticFactFamily::Suspension)?;
    for (&owner, suspension) in &suspensions {
        if instance_owned.contains(&owner) {
            return Err(RuntimeSemanticFactsError::InstanceOwnedGlobalFact {
                family: RuntimeSemanticFactFamily::Suspension,
            });
        }
        require_stmt_family(
            modules,
            owners,
            owner,
            RuntimeSemanticFactFamily::Suspension,
            |kind| matches!(kind, HirStmtKind::Wait { .. }),
        )?;
        validate(modules, expression_types, owner, suspension)?;
    }
    for module in modules.values() {
        for (owner, statement) in module.statements() {
            if owners.contains_statement(owner)
                && !instance_owned.contains(&owner)
                && matches!(statement.kind(), HirStmtKind::Wait { .. })
                && !suspensions.contains_key(&owner)
            {
                return Err(RuntimeSemanticFactsError::InvalidSuspension { statement: owner });
            }
        }
    }
    Ok(suspensions)
}

pub(super) fn validate(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expression_types: &RuntimeExpressionTypeView<'_>,
    owner: StmtId,
    suspension: &CheckedSuspensionStatement,
) -> Result<(), RuntimeSemanticFactsError> {
    let CheckedSuspensionStatement::Wait { target } = suspension;
    if !matches!(resolve_stmt(modules, owner)?, HirStmtKind::Wait { target: child } if child == target)
        || !expression_types
            .get(target)
            .is_some_and(|ty| matches!(ty.shape(), RuntimeTypeShape::Duration))
    {
        return Err(RuntimeSemanticFactsError::InvalidSuspension { statement: owner });
    }
    Ok(())
}
