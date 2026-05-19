//! Choice-family type checking.

use super::{
    EntityKind, EntityRefSyntax, IdRef, Pattern, TypeCheckError, TypeChecker, TypeKind,
    ident_pattern_name, iter_item_type,
};

impl TypeChecker<'_> {
    pub(super) fn check_choice(&mut self, choice: &arcweft_lang_hir::HirChoice) {
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

    fn check_choice_items(&mut self, items: &[arcweft_lang_syntax::ChoiceItem]) {
        for item in items {
            self.check_choice_item(item);
        }
    }

    fn check_choice_item(&mut self, item: &arcweft_lang_syntax::ChoiceItem) {
        match item {
            arcweft_lang_syntax::ChoiceItem::Let { pattern, expr } => {
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
            arcweft_lang_syntax::ChoiceItem::If { condition, items } => {
                self.expect_expr_type(condition, &TypeKind::Bool, "choice if condition");
                let outer_locals = self.locals.clone();
                self.check_choice_items(items);
                self.locals = outer_locals;
            }
            arcweft_lang_syntax::ChoiceItem::For {
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
            arcweft_lang_syntax::ChoiceItem::Match { expr, arms } => {
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
            arcweft_lang_syntax::ChoiceItem::Option(option) => self.check_choice_option(option),
            arcweft_lang_syntax::ChoiceItem::Raw(raw) => self.errors.push(TypeCheckError::new(
                format!("raw choice item is not type-checkable: {raw}"),
            )),
        }
    }

    fn check_choice_option(&mut self, option: &arcweft_lang_syntax::ChoiceOption) {
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

    fn check_choice_action(&mut self, action: &arcweft_lang_syntax::ChoiceAction) {
        match action {
            arcweft_lang_syntax::ChoiceAction::Out(expr) => {
                self.check_expr(expr);
            }
            arcweft_lang_syntax::ChoiceAction::SelectBlock(statements) => {
                let outer_locals = self.locals.clone();
                for stmt in statements {
                    self.check_stmt(stmt);
                }
                self.locals = outer_locals;
            }
            arcweft_lang_syntax::ChoiceAction::Goto(target) => {
                if let EntityRefSyntax::Absolute(target) = target {
                    self.expect_entity_kind(target, &EntityKind::Flow, "choice target");
                }
            }
            arcweft_lang_syntax::ChoiceAction::None => {}
        }
    }

    fn check_choice_plan_item(&mut self, item: &arcweft_lang_syntax::ChoicePlanItem) {
        match item {
            arcweft_lang_syntax::ChoicePlanItem::Option { value, .. } => {
                self.check_expr(value);
            }
            arcweft_lang_syntax::ChoicePlanItem::Timeout { duration, body } => {
                self.expect_expr_type(duration, &TypeKind::Duration, "choice timeout duration");
                for stmt in body {
                    self.check_stmt(stmt);
                }
            }
            arcweft_lang_syntax::ChoicePlanItem::Cancel { body, .. } => {
                for stmt in body {
                    self.check_stmt(stmt);
                }
            }
            arcweft_lang_syntax::ChoicePlanItem::OnSelect { pattern, body } => {
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
            arcweft_lang_syntax::ChoicePlanItem::Raw(raw) => self.errors.push(TypeCheckError::new(
                format!("raw choice-plan item is not type-checkable: {raw}"),
            )),
        }
    }
}
