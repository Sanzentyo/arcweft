//! Flow-item type checking and flow-local scope handling.

use super::helpers::let_else_bindings;
use super::{
    EntityKind, HirFlowItem, Pattern, SelectBranchHead, TypeCheckError, TypeChecker, TypeKind,
    entity_kind, ident_pattern_name, type_ref_kind, typed_pattern_binding,
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
                if !matches!(kind, Some(EntityKind::Fragment | EntityKind::Flow)) {
                    self.errors.push(TypeCheckError::new(format!(
                        "include target `{}` must be a flow or fragment reference",
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
        self.expect_expr_type(block.condition(), &TypeKind::Bool, "if condition");
        let borrow_snapshot = self.snapshot_borrow_state();
        self.check_flow_items(block.body());
        let then_state = self.snapshot_borrow_state();
        self.restore_borrow_state(borrow_snapshot.clone());
        self.check_flow_items(block.else_body());
        let else_state = self.snapshot_borrow_state();
        self.merge_borrow_state_from_paths(
            &borrow_snapshot,
            &[&borrow_snapshot, &then_state, &else_state],
        );
    }

    fn check_flow_match_block(&mut self, block: &arcweft_lang_hir::model::HirMatch) {
        let expr_type = self.check_expr(block.expr());
        let base_borrow_snapshot = self.snapshot_borrow_state();
        let mut arm_states = Vec::new();
        for arm in block.arms() {
            self.restore_borrow_state(base_borrow_snapshot.clone());
            let outer_locals = self.locals.clone();
            for (name, ty) in let_else_bindings(arm.pattern(), expr_type.as_ref()) {
                self.locals.insert(name, ty);
            }
            if let Some(guard) = arm.guard() {
                self.expect_expr_type(guard, &TypeKind::Bool, "match arm guard");
            }
            self.check_flow_items(arm.body());
            arm_states.push(self.snapshot_borrow_state());
            self.locals = outer_locals;
        }
        self.check_choice_match_exhaustive(
            expr_type.as_ref(),
            block
                .arms()
                .iter()
                .map(arcweft_lang_hir::model::HirMatchArm::pattern),
        );
        if arm_states.is_empty() {
            self.restore_borrow_state(base_borrow_snapshot);
        } else {
            let arm_state_refs = arm_states.iter().collect::<Vec<_>>();
            self.merge_borrow_state_from_paths(&base_borrow_snapshot, &arm_state_refs);
        }
    }

    fn check_scoped_flow_items(&mut self, items: &[HirFlowItem]) {
        let outer_locals = self.locals.clone();
        let outer_presentation_defaults = self.active_presentation_defaults.clone();
        self.check_flow_items(items);
        self.locals = outer_locals;
        self.active_presentation_defaults = outer_presentation_defaults;
    }

    fn check_scope_expr_binding(
        &mut self,
        pattern: &Pattern,
        scope: &arcweft_lang_hir::model::HirScopeExpr,
    ) {
        let outer_locals = self.locals.clone();
        for stmt in scope.statements() {
            self.check_stmt(stmt);
        }
        let value_type = scope.value().and_then(|value| self.check_expr(value));
        self.locals = outer_locals;
        if let (Some(name), Some(ty)) = (ident_pattern_name(pattern), value_type) {
            self.locals.insert(name.to_owned(), ty);
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
        let borrow_snapshot = self.snapshot_borrow_state();
        let locals_start = self.locals.clone();
        if let Some((name, ty)) = typed_pattern_binding(block.binding()) {
            let ty = type_ref_kind(ty);
            self.locals.insert(name.to_owned(), ty.clone());
            self.register_borrow_bindings(block.binding(), &ty);
        }
        for item in block.body() {
            self.check_flow_item(item);
        }
        self.restore_borrow_state(borrow_snapshot);
        self.locals = locals_start;
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
