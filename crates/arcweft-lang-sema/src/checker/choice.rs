//! Choice-family type checking.

use super::{
    EntityKind, EntityRefSyntax, IdRef, Pattern, TypeCheckError, TypeChecker, TypeKind,
    ident_pattern_name, iter_item_type,
};
use arcweft_lang_syntax::ast::choice::{ChoiceAction, ChoiceItem, ChoiceOption, ChoicePlanItem};

impl TypeChecker<'_> {
    pub(super) fn check_choice(&mut self, choice: &arcweft_lang_hir::model::HirChoice) {
        if let Some(id) = choice.id() {
            self.expect_entity_kind(id, &EntityKind::Choice, "choice id");
        }
        self.check_choice_items(choice.items());
        for option in choice.options() {
            if let Some(id) = option.id() {
                self.expect_entity_kind(id, &EntityKind::ChoiceOption, "choice option id");
            }
            if let Some(target) = option.target() {
                self.expect_entity_kind(target, &EntityKind::Flow, "choice target");
            }
        }
        if let Some(plan) = choice.plan() {
            for item in plan.items() {
                self.check_choice_plan_item(item);
            }
        }
    }

    fn check_choice_items(&mut self, items: &[ChoiceItem]) {
        for item in items {
            self.check_choice_item(item);
        }
    }

    fn check_choice_item(&mut self, item: &ChoiceItem) {
        match item {
            ChoiceItem::Let { pattern, expr } => {
                let value_type = self.check_expr(expr);
                if let (Some(name), Some(ty)) = (ident_pattern_name(pattern), value_type) {
                    self.locals.insert(name.to_owned(), ty);
                }
                if let Pattern::Raw(raw) = pattern {
                    self.errors.push(TypeCheckError::new(format!(
                        "raw choice let pattern is not type-checkable: {raw}"
                    )));
                }
            }
            ChoiceItem::If { condition, items } => {
                self.expect_expr_type(condition, &TypeKind::Bool, "choice if condition");
                let outer_locals = self.locals.clone();
                self.check_choice_items(items);
                self.locals = outer_locals;
            }
            ChoiceItem::For {
                pattern,
                source,
                items,
            } => {
                let source_type = self.check_expr(source);
                let outer_locals = self.locals.clone();
                if let Some(name) = ident_pattern_name(pattern) {
                    self.locals
                        .insert(name.to_owned(), iter_item_type(source_type.as_ref()));
                } else if let Pattern::Raw(raw) = pattern {
                    self.errors.push(TypeCheckError::new(format!(
                        "raw choice for pattern is not type-checkable: {raw}"
                    )));
                }
                self.check_choice_items(items);
                self.locals = outer_locals;
            }
            ChoiceItem::Match { expr, arms } => {
                self.check_expr(expr);
                for arm in arms {
                    let outer_locals = self.locals.clone();
                    if let Pattern::Raw(raw) = arm.pattern() {
                        self.errors.push(TypeCheckError::new(format!(
                            "raw choice match pattern is not type-checkable: {raw}"
                        )));
                    }
                    if let Some(guard) = arm.guard() {
                        self.expect_expr_type(guard, &TypeKind::Bool, "choice match guard");
                    }
                    self.check_choice_items(arm.items());
                    self.locals = outer_locals;
                }
            }
            ChoiceItem::Option(option) => self.check_choice_option(option),
            ChoiceItem::Raw(raw) => self.errors.push(TypeCheckError::new(format!(
                "raw choice item is not type-checkable: {raw}"
            ))),
        }
    }

    fn check_choice_option(&mut self, option: &ChoiceOption) {
        if let Some(IdRef::Absolute(id)) = option.id() {
            self.expect_entity_kind(id, &EntityKind::ChoiceOption, "choice option id");
        }
        if let Some(id_expr) = option.id_expr() {
            self.check_expr(id_expr);
        }
        if let Some(IdRef::Absolute(text_key)) = option.label_text_key() {
            self.expect_entity_kind(text_key, &EntityKind::Text, "choice label text key");
        }
        if let Some(value) = option.value() {
            self.check_expr(value);
        }
        if let Some(enabled) = option.enabled() {
            self.expect_expr_type(enabled, &TypeKind::Bool, "choice enabled");
        }
        if let Some(visible) = option.visible() {
            self.expect_expr_type(visible, &TypeKind::Bool, "choice visible");
        }
        if let Some(order) = option.order() {
            self.expect_expr_type(order, &TypeKind::Int, "choice order");
        }
        if let Some(hotkey) = option.hotkey() {
            self.check_expr(hotkey);
        }
        for field in option.ui_fields() {
            self.check_expr(field.value());
        }
        self.check_choice_action(option.action());
    }

    fn check_choice_action(&mut self, action: &ChoiceAction) {
        match action {
            ChoiceAction::Out(expr) => {
                self.check_expr(expr);
            }
            ChoiceAction::SelectBlock(statements) => {
                let outer_locals = self.locals.clone();
                for stmt in statements {
                    self.check_stmt(stmt);
                }
                self.locals = outer_locals;
            }
            ChoiceAction::Goto(target) => {
                if let EntityRefSyntax::Absolute(target) = target {
                    self.expect_entity_kind(target, &EntityKind::Flow, "choice target");
                }
            }
            ChoiceAction::None => {}
        }
    }

    fn check_choice_plan_item(&mut self, item: &ChoicePlanItem) {
        match item {
            ChoicePlanItem::Option { value, .. } => {
                self.check_expr(value);
            }
            ChoicePlanItem::Timeout { duration, body } => {
                self.expect_expr_type(duration, &TypeKind::Duration, "choice timeout duration");
                for stmt in body {
                    self.check_stmt(stmt);
                }
            }
            ChoicePlanItem::Cancel { body, .. } => {
                for stmt in body {
                    self.check_stmt(stmt);
                }
            }
            ChoicePlanItem::OnSelect { pattern, body } => {
                let outer_locals = self.locals.clone();
                if let Some(name) = ident_pattern_name(pattern) {
                    self.locals
                        .insert(name.to_owned(), TypeKind::Ref(EntityKind::ChoiceOption));
                }
                for stmt in body {
                    self.check_stmt(stmt);
                }
                self.locals = outer_locals;
            }
            ChoicePlanItem::Raw(raw) => self.errors.push(TypeCheckError::new(format!(
                "raw choice-plan item is not type-checkable: {raw}"
            ))),
        }
    }
}
