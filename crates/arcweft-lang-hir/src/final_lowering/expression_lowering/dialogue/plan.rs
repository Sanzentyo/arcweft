//! Typed Dialogue line-plan lowering through the existing HIR arenas.

use arcweft_lang_syntax::attachment::{AttachedDialogueLinePlan, AttachedDialogueLinePlanItem};
use arcweft_lang_syntax::grammar::SyntaxKind;

use crate::dialogue_application::{HirDialogueContent, HirLinePlan, HirLinePlanItem};
use crate::identity::{ExprId, LocalId, ScopeId};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};

use super::super::super::StagedHirModuleTransaction;

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_dialogue_line_plan(
        &mut self,
        attached: &AttachedDialogueLinePlan,
        owner: ExprId,
        parent_scope: ScopeId,
        content: &HirDialogueContent,
    ) -> Result<HirLinePlan, HirLowerFailure> {
        let scope = self.allocate_expression_owned_block_scope(
            attached.body().syntax(),
            owner,
            parent_scope,
        )?;
        let mut items = Vec::with_capacity(attached.body().items().len());
        let mut locals = Vec::<LocalId>::new();
        for item in attached.body().items() {
            match item {
                AttachedDialogueLinePlanItem::Init(init) => {
                    let init_scope =
                        self.allocate_expression_owned_block_scope(init.body(), owner, scope)?;
                    let mut statements = Vec::with_capacity(init.statements().len());
                    let mut init_locals = Vec::<LocalId>::new();
                    for statement in init.statements() {
                        let lowered = self.lower_attached_dialogue_line_plan_statement(
                            statement, init_scope, content,
                        )?;
                        init_locals.extend_from_slice(&lowered.locals);
                        statements.push(lowered.owner);
                    }
                    self.close_scope_members(init_scope, init_locals.into_boxed_slice())?;
                    items.push(HirLinePlanItem::Init {
                        scope: init_scope,
                        statements: statements.into_boxed_slice(),
                    });
                }
                AttachedDialogueLinePlanItem::Statement(statement) => {
                    let is_cancel_rule =
                        statement.kind() == SyntaxKind::DialogueCancelRuleStatement;
                    let lowered = self
                        .lower_attached_dialogue_line_plan_statement(statement, scope, content)?;
                    locals.extend_from_slice(&lowered.locals);
                    let item = if lowered.poisoned {
                        HirLinePlanItem::Error(lowered.owner)
                    } else if is_cancel_rule {
                        HirLinePlanItem::CancelRule(lowered.owner)
                    } else {
                        HirLinePlanItem::Statement(lowered.owner)
                    };
                    items.push(item);
                }
            }
        }
        self.close_scope_members(scope, locals.into_boxed_slice())?;
        HirLinePlan::try_new(scope, None, items.into_boxed_slice())
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
    }
}
