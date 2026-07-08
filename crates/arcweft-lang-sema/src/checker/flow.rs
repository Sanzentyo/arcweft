//! Flow-item type checking and flow-local scope handling.

use super::helpers::let_else_bindings;
use super::{
    BorrowStateDelta, EntityKind, HirFlowItem, Pattern, SelectBranchHead, SuspensionBoundary,
    TypeCheckError, TypeChecker, TypeKind, entity_kind, ident_pattern_name, type_ref_kind,
    typed_pattern_binding,
};

impl TypeChecker<'_> {
    pub(super) fn check_flow_items(&mut self, items: &[HirFlowItem]) {
        for item in items {
            self.check_flow_item(item);
        }
    }

    fn check_flow_item(&mut self, item: &HirFlowItem) {
        match item {
            HirFlowItem::Stmt(stmt) => self.check_stmt(stmt),
            HirFlowItem::Dialogue(dialogue) => {
                self.check_dialogue_item(dialogue);
            }
            HirFlowItem::Choice(choice) => {
                self.check_choice(choice);
            }
            HirFlowItem::LetChoice { pattern, choice } => {
                self.check_choice_binding(pattern, choice);
            }
            HirFlowItem::LetScope { pattern, scope } => {
                self.check_scope_expr_binding(pattern, scope);
            }
            HirFlowItem::LetLoop { pattern, block } => {
                self.check_loop_binding(pattern, block);
            }
            HirFlowItem::LetAwait {
                pattern,
                await_with,
            } => {
                self.check_await_binding(pattern, await_with);
            }
            HirFlowItem::Thread(thread) => {
                self.reject_active_borrows(SuspensionBoundary::ThreadSuspension);
                self.check_flow_items(thread.body());
            }
            HirFlowItem::If(block) => {
                self.check_flow_if_block(block);
            }
            HirFlowItem::IfLet(block) => {
                self.check_if_let_block(block);
            }
            HirFlowItem::Match(block) => {
                self.check_flow_match_block(block);
            }
            HirFlowItem::Loop(block) => {
                self.check_loop_block(block, true);
            }
            HirFlowItem::While(block) => {
                self.check_while_block(block);
            }
            HirFlowItem::WhileLet(block) => {
                self.check_while_let_block(block);
            }
            HirFlowItem::For(block) => {
                self.check_for_block(block);
            }
            HirFlowItem::Select(block) => {
                self.check_select_block(block);
            }
            HirFlowItem::Borrow(block) => {
                self.check_borrow_block(block);
            }
            HirFlowItem::SourceLocale(block) => {
                self.check_flow_items(block.body());
            }
            HirFlowItem::Scope(block) => {
                self.check_scoped_flow_items(block.body());
            }
            HirFlowItem::Include(entity) => {
                let kind = entity_kind(entity);
                if !matches!(kind, Some(EntityKind::Flow)) {
                    self.errors.push(TypeCheckError::new(format!(
                        "include target `{}` must be a flow reference",
                        entity.body()
                    )));
                }
            }
            HirFlowItem::Await(await_with) => {
                self.check_await_item(await_with);
            }
        }
    }

    fn check_flow_if_block(&mut self, block: &arcweft_lang_hir::model::HirIf) {
        self.expect_authored_expr_type(block.condition_authored(), &TypeKind::Bool, "if condition");
        let borrow_checkpoint = self.checkpoint_borrow_state();
        self.check_flow_items(block.body());
        let then_state = self.capture_borrow_state_delta(borrow_checkpoint);
        self.restore_borrow_state(borrow_checkpoint);
        self.check_flow_items(block.else_body());
        let else_state = self.capture_borrow_state_delta(borrow_checkpoint);
        let unchanged_state = BorrowStateDelta::default();
        self.merge_borrow_state_from_deltas(
            borrow_checkpoint,
            &[&unchanged_state, &then_state, &else_state],
        );
    }

    fn check_flow_match_block(&mut self, block: &arcweft_lang_hir::model::HirMatch) {
        let expr_type = self.check_authored_expr(block.expr_authored());
        let base_borrow_checkpoint = self.checkpoint_borrow_state();
        let mut arm_states = Vec::new();
        for arm in block.arms() {
            self.restore_borrow_state(base_borrow_checkpoint);
            let local_snapshot =
                self.insert_scoped_locals(let_else_bindings(arm.pattern(), expr_type.as_ref()));
            if let Some(guard) = arm.guard_authored() {
                self.expect_authored_expr_type(guard, &TypeKind::Bool, "match arm guard");
            }
            self.check_flow_items(arm.body());
            arm_states.push(self.capture_borrow_state_delta(base_borrow_checkpoint));
            self.restore_scoped_locals(local_snapshot);
        }
        self.check_choice_match_exhaustive(
            expr_type.as_ref(),
            block
                .arms()
                .iter()
                .map(arcweft_lang_hir::model::HirMatchArm::pattern),
        );
        if arm_states.is_empty() {
            self.restore_borrow_state(base_borrow_checkpoint);
        } else {
            let arm_state_refs = arm_states.iter().collect::<Vec<_>>();
            self.merge_borrow_state_from_deltas(base_borrow_checkpoint, &arm_state_refs);
        }
    }

    fn check_scoped_flow_items(&mut self, items: &[HirFlowItem]) {
        let outer_presentation_defaults = self.active_presentation_defaults.clone();
        self.with_local_mutation_scope(|this| this.check_flow_items(items));
        self.active_presentation_defaults = outer_presentation_defaults;
    }

    fn check_scope_expr_binding(
        &mut self,
        pattern: &Pattern,
        scope: &arcweft_lang_hir::model::HirScopeExpr,
    ) {
        let value_type = self.with_local_mutation_scope(|this| {
            for stmt in scope.statements() {
                this.check_stmt(stmt);
            }
            scope.value().and_then(|value| this.check_expr(value))
        });
        if let (Some(name), Some(ty)) = (ident_pattern_name(pattern), value_type) {
            self.bind_local(name.to_owned(), ty);
        }
    }

    fn check_select_block(&mut self, block: &arcweft_lang_hir::model::HirSelect) {
        if block.branches().is_empty() {
            self.errors.push(TypeCheckError::new(
                "select block must define at least one branch".to_owned(),
            ));
        }
        for branch in block.branches() {
            self.check_select_head(branch.head());
            for item in branch.body() {
                self.check_flow_item(item);
            }
        }
    }

    fn check_borrow_block(&mut self, block: &arcweft_lang_hir::model::HirBorrow) {
        self.check_expr(block.source());
        let borrow_checkpoint = self.checkpoint_borrow_state();
        self.with_local_mutation_scope(|this| {
            if let Some((name, ty)) = typed_pattern_binding(block.binding()) {
                let ty = type_ref_kind(ty);
                this.bind_local(name.to_owned(), ty.clone());
                this.register_borrow_bindings(block.binding(), &ty);
            }
            for item in block.body() {
                this.check_flow_item(item);
            }
        });
        self.restore_borrow_state(borrow_checkpoint);
    }

    fn check_select_head(&mut self, head: &SelectBranchHead) {
        match head {
            SelectBranchHead::Bind { source, .. } => {
                self.check_expr(source);
            }
            SelectBranchHead::Frame(pattern) | SelectBranchHead::Event(pattern) => {
                if let Pattern::Raw(raw) = pattern {
                    self.errors.push(TypeCheckError::new(format!(
                        "raw select branch pattern is not type-checkable: {raw}"
                    )));
                }
            }
            SelectBranchHead::Raw(raw) => {
                self.errors.push(TypeCheckError::new(format!(
                    "raw select branch head is not type-checkable: {raw}"
                )));
            }
        }
    }
}
