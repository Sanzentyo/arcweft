//! Admission of the original checked Include statement and its generation.

use super::{
    RuntimeSemanticFactFamily, RuntimeSemanticFactsError, RuntimeSemanticOwnerSet, collect_unique,
    require_stmt_family, resolve_stmt,
};
use arcweft_lang_hir::{
    identity::{HirModuleId, StmtId},
    module::HirModule,
    project::HirAnalysisProjectView,
    stmt::HirStmtKind,
};
use arcweft_lang_sema::final_analysis::{CheckedIncludeFlowTarget, CheckedLocalUseAuthority};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn admit_global(
    project: HirAnalysisProjectView<'_>,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    owners: RuntimeSemanticOwnerSet<'_>,
    instance_owned: &BTreeSet<StmtId>,
    pending: Vec<CheckedIncludeFlowTarget>,
) -> Result<BTreeMap<StmtId, CheckedIncludeFlowTarget>, RuntimeSemanticFactsError> {
    let includes = collect_unique(
        pending
            .into_iter()
            .map(|target| (target.statement(), target)),
        RuntimeSemanticFactFamily::Include,
    )?;
    for (&owner, target) in &includes {
        if !target.origin().validate_owner(project, owner) {
            return Err(RuntimeSemanticFactsError::InvalidIncludeTarget { statement: owner });
        }
        if instance_owned.contains(&owner) {
            return Err(RuntimeSemanticFactsError::InstanceOwnedGlobalFact {
                family: RuntimeSemanticFactFamily::Include,
            });
        }
        require_stmt_family(
            modules,
            owners,
            owner,
            RuntimeSemanticFactFamily::Include,
            |kind| matches!(kind, HirStmtKind::Include(_)),
        )?;
    }
    for module in modules.values() {
        for (owner, statement) in module.statements() {
            if owners.contains_statement(owner)
                && !instance_owned.contains(&owner)
                && matches!(statement.kind(), HirStmtKind::Include(_))
                && !includes.contains_key(&owner)
            {
                return Err(RuntimeSemanticFactsError::MissingIncludeTarget { statement: owner });
            }
        }
    }
    Ok(includes)
}

pub(super) fn validate_instance(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    authority: &CheckedLocalUseAuthority,
    owner: StmtId,
    target: &CheckedIncludeFlowTarget,
) -> Result<(), RuntimeSemanticFactsError> {
    if !target.origin().validate_authority(authority, owner)
        || !matches!(resolve_stmt(modules, owner)?, HirStmtKind::Include(_))
    {
        return Err(RuntimeSemanticFactsError::InvalidIncludeTarget { statement: owner });
    }
    Ok(())
}
