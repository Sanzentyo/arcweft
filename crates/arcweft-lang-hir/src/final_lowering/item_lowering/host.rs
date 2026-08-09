//! Attached Test and Bench lowering into their final statement-only HIR owner.

use arcweft_lang_syntax::attachment::node::{BenchItemKind, TestItemKind};
use arcweft_lang_syntax::attachment::{
    AstNode, AttachedPlanBody, AttachedPlanId, AttachedTestKind,
};

use crate::identity::{ItemId, ScopeId};
use crate::item::{
    HirBenchItem, HirItem, HirItemIssue, HirItemKind, HirTestItem, HirTestKind, HirTestKindIssue,
};
use crate::leaf::{HirIdRefIssue, HirIdRefRecovery, HirIdRefShape, HirIdRefValue};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::scope::HirScopeKind;

use super::super::{StagedHirModuleTransaction, id_ref_projection, name_projection};
use super::{LoweredItemProjection, item_state};

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_test_declaration(
        &mut self,
        owner: ItemId,
        parent_scope: ScopeId,
        syntax: &AstNode<TestItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = syntax
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let prefix = self.lower_item_prefix(attached.prefix(), parent_scope)?;
        let id = lower_plan_id(attached.id())?;
        let kind = lower_test_kind(attached.kind())?;
        let body = self.lower_plan_body(owner, parent_scope, attached.body())?;

        let mut issue = prefix.issue;
        if matches!(attached.id(), AttachedPlanId::Missing(_)) {
            issue.get_or_insert(HirItemIssue::MissingId);
        } else if id.recovery_issue().is_some() {
            issue.get_or_insert(HirItemIssue::Recovery);
        }
        if matches!(kind, HirTestKind::Recovered(_)) {
            issue.get_or_insert(HirItemIssue::MissingKind);
        }
        if matches!(attached.body(), AttachedPlanBody::Missing(_)) {
            issue.get_or_insert(HirItemIssue::MissingBody);
        } else if attached.body().has_recovery() || body.poisoned {
            issue.get_or_insert(HirItemIssue::Recovery);
        }
        if !attached.trailing_recoveries().is_empty() {
            issue.get_or_insert(HirItemIssue::MalformedHeader);
        }

        let kind = HirItemKind::Test(HirTestItem::new(id, kind, body.scope, body.statements));
        Ok(LoweredItemProjection {
            item: HirItem::try_new_with_state(
                owner,
                parent_scope,
                prefix.value,
                kind,
                Box::new([]),
                item_state(issue),
            )
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            members: None,
        })
    }

    pub(super) fn lower_bench_declaration(
        &mut self,
        owner: ItemId,
        parent_scope: ScopeId,
        syntax: &AstNode<BenchItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = syntax
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let prefix = self.lower_item_prefix(attached.prefix(), parent_scope)?;
        let id = lower_plan_id(attached.id())?;
        let body = self.lower_plan_body(owner, parent_scope, attached.body())?;

        let mut issue = prefix.issue;
        if matches!(attached.id(), AttachedPlanId::Missing(_)) {
            issue.get_or_insert(HirItemIssue::MissingId);
        } else if id.recovery_issue().is_some() {
            issue.get_or_insert(HirItemIssue::Recovery);
        }
        if matches!(attached.body(), AttachedPlanBody::Missing(_)) {
            issue.get_or_insert(HirItemIssue::MissingBody);
        } else if attached.body().has_recovery() || body.poisoned {
            issue.get_or_insert(HirItemIssue::Recovery);
        }
        if !attached.trailing_recoveries().is_empty() {
            issue.get_or_insert(HirItemIssue::MalformedHeader);
        }

        let kind = HirItemKind::Bench(HirBenchItem::new(id, body.scope, body.statements));
        Ok(LoweredItemProjection {
            item: HirItem::try_new_with_state(
                owner,
                parent_scope,
                prefix.value,
                kind,
                Box::new([]),
                item_state(issue),
            )
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            members: None,
        })
    }

    fn lower_plan_body(
        &mut self,
        owner: ItemId,
        parent_scope: ScopeId,
        attached: &AttachedPlanBody,
    ) -> Result<LoweredPlanBody, HirLowerFailure> {
        let scope = self.allocate_item_body_scope_from_syntax(
            &attached.syntax(),
            owner,
            parent_scope,
            HirScopeKind::Block,
        )?;
        let Some(block) = attached.block() else {
            self.close_scope_members(scope, Box::new([]))?;
            return Ok(LoweredPlanBody {
                scope,
                statements: Box::new([]),
                poisoned: true,
            });
        };
        let lowered = self.lower_attached_statement_only_block(block, scope)?;
        Ok(LoweredPlanBody {
            scope,
            statements: lowered.statements,
            poisoned: lowered.poisoned,
        })
    }
}

struct LoweredPlanBody {
    scope: ScopeId,
    statements: Box<[crate::identity::StmtId]>,
    poisoned: bool,
}

fn lower_plan_id(attached: &AttachedPlanId) -> Result<HirIdRefValue, HirLowerFailure> {
    match attached {
        AttachedPlanId::Authored(_) => id_ref_projection::id_ref(
            attached
                .value()
                .ok_or(HirInvariantFailure::InvalidArenaCommit)?,
        ),
        AttachedPlanId::Missing(_) => Ok(HirIdRefValue::Recovered(HirIdRefRecovery::new(
            HirIdRefShape::Missing,
            HirIdRefIssue::Missing,
        ))),
    }
}

fn lower_test_kind(attached: &AttachedTestKind) -> Result<HirTestKind, HirLowerFailure> {
    Ok(match attached {
        AttachedTestKind::Scenario(_) => HirTestKind::Scenario,
        AttachedTestKind::Visual(_) => HirTestKind::Visual,
        AttachedTestKind::Audio(_) => HirTestKind::Audio,
        AttachedTestKind::Fixture(_) => HirTestKind::Fixture,
        AttachedTestKind::Custom { value, .. } => {
            HirTestKind::Custom(name_projection::name(value)?)
        }
        AttachedTestKind::Missing(_) => HirTestKind::Recovered(HirTestKindIssue::Missing),
    })
}
