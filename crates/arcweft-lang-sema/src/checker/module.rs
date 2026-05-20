//! Module, top-level declaration, and dialogue entry checks.

use super::{
    EffectScope, EntityKind, FlowKind, FunctionKind, HirModule, HirTopLevelDecl, LifetimeKey,
    LifetimeScopeKind, Pattern, TypeCheckError, TypeChecker, TypeKind, YieldContext,
    choice_output_type, entity_kind_for_decl, ident_pattern_name, stream_return_types,
    type_ref_kind, validate_typecheck_ready,
};
use arcweft_lang_syntax::ast::items::EntryItem;
use arcweft_lang_syntax::expr::{ComputationBlockKind, Expr};

impl TypeChecker<'_> {
    pub(super) fn check_module(&mut self, module: &HirModule) {
        if let Err(errors) = validate_typecheck_ready(module) {
            self.errors.extend(
                errors
                    .into_iter()
                    .map(|error| TypeCheckError::new(error.message().to_owned())),
            );
        }

        self.bind_top_level_entity_aliases(module);
        self.bind_top_level_functions(module);
        self.bind_extern_capability_functions(module);

        for flow in module.flows() {
            self.active_borrows.clear();
            self.borrow_local_lifetimes.clear();
            self.locals.clear();
            self.loop_stack.clear();
            self.active_presentation_defaults.clear();
            self.line_mark_stack.clear();
            self.lifetime_guarantees.clear();
            self.dropped_lifetime_keys.clear();
            if let Some(signature) = flow.signature() {
                for group in signature.param_groups() {
                    for param in group.params() {
                        self.bind_function_param(param.pattern(), &type_ref_kind(param.ty()));
                    }
                }
            }
            if let Some(id) = flow.id() {
                match flow.kind() {
                    FlowKind::Flow => self.expect_entity_kind(id, &EntityKind::Flow, "flow id"),
                    FlowKind::Fragment => {
                        self.expect_entity_kind(id, &EntityKind::Fragment, "fragment id");
                    }
                }
            }
            for contract in flow.contracts() {
                self.check_contract_clause(contract);
            }
            let effect_scope = EffectScope::from_contracts(flow.contracts());
            let effect_snapshot = self.apply_effect_scope(&effect_scope);
            self.check_flow_items(flow.body());
            self.effect_capabilities = effect_snapshot;
        }
        for function in module.functions() {
            self.active_borrows.clear();
            self.borrow_local_lifetimes.clear();
            self.locals.clear();
            self.loop_stack.clear();
            self.yield_stack.clear();
            self.active_presentation_defaults.clear();
            for group in function.signature().param_groups() {
                for param in group.params() {
                    self.bind_function_param(param.pattern(), &type_ref_kind(param.ty()));
                }
            }
            for contract in function.contracts() {
                self.check_contract_clause(contract);
            }
            let effect_scope = EffectScope::from_contracts(function.contracts());
            let effect_snapshot = self.apply_effect_scope(&effect_scope);
            if function.kind() == FunctionKind::Stream {
                self.check_stream_function(function);
                self.effect_capabilities = effect_snapshot;
                continue;
            }
            let actual = self.check_block_expr(function.statements(), function.value());
            self.effect_capabilities = effect_snapshot;
            if let (Some(expected), Some(actual)) = (
                function.signature().return_type().map(type_ref_kind),
                actual,
            ) {
                if !types_compatible(&expected, &actual) {
                    self.errors.push(TypeCheckError::new(format!(
                        "function `{}` returns {expected:?}, but body has {actual:?}",
                        function.name()
                    )));
                }
            }
        }
        for declaration in module.declarations() {
            self.check_top_level_decl(declaration);
        }
        self.check_flow_items(module.top_level_items());
    }

    fn bind_top_level_entity_aliases(&mut self, module: &HirModule) {
        for declaration in module.declarations() {
            let HirTopLevelDecl::EntityDecl(item) = declaration else {
                continue;
            };
            let Some(alias) = item.surface_alias() else {
                continue;
            };
            self.global_symbols.insert(
                alias.to_owned(),
                TypeKind::Ref(entity_kind_for_decl(item.kind())),
            );
        }
    }

    fn bind_extern_capability_functions(&mut self, module: &HirModule) {
        for declaration in module.declarations() {
            let HirTopLevelDecl::ExternCapability(item) = declaration else {
                continue;
            };
            for function in item.functions() {
                let return_type = function
                    .signature()
                    .return_type()
                    .map_or(TypeKind::Unit, type_ref_kind);
                let name = format!("{}.{}", item.id(), function.signature().name());
                self.global_functions.insert(name.clone(), return_type);
                self.global_function_effects.insert(
                    name,
                    function
                        .effects()
                        .iter()
                        .filter_map(crate::fact_layer::capability_from_expr)
                        .map(|capability| capability.as_str().to_owned())
                        .collect(),
                );
            }
        }
    }

    fn bind_top_level_functions(&mut self, module: &HirModule) {
        for function in module.functions() {
            let return_type = function
                .signature()
                .return_type()
                .map_or(TypeKind::Unit, type_ref_kind);
            self.global_functions
                .insert(function.name().to_owned(), return_type);
        }
    }

    pub(super) fn check_top_level_decl(&mut self, declaration: &HirTopLevelDecl) {
        match declaration {
            HirTopLevelDecl::Attribute(_)
            | HirTopLevelDecl::DialogueDefaults(_)
            | HirTopLevelDecl::Enum(_)
            | HirTopLevelDecl::Impl(_)
            | HirTopLevelDecl::Proof(_)
            | HirTopLevelDecl::Struct(_)
            | HirTopLevelDecl::Trait(_)
            | HirTopLevelDecl::TrustedAxiom(_)
            | HirTopLevelDecl::ExternMod(_)
            | HirTopLevelDecl::ExternCapability(_) => {}
            HirTopLevelDecl::Entry(item) => {
                self.expect_entity_kind(item.id(), &EntityKind::Entry, "entry id");
                for item in item.items() {
                    self.check_entry_item(item);
                }
            }
            HirTopLevelDecl::Test(item) => {
                if let Some(id) = item.id().as_absolute() {
                    self.expect_entity_kind(id, &EntityKind::Test, "test id");
                }
            }
            HirTopLevelDecl::Bench(item) => {
                if let Some(id) = item.id().as_absolute() {
                    self.expect_entity_kind(id, &EntityKind::Bench, "bench id");
                }
            }
            HirTopLevelDecl::EntityDecl(item) => {
                self.expect_entity_kind(
                    item.id(),
                    &entity_kind_for_decl(item.kind()),
                    "entity declaration id",
                );
            }
            HirTopLevelDecl::Callable(item) => {
                self.active_borrows.clear();
                self.borrow_local_lifetimes.clear();
                self.locals.clear();
                self.loop_stack.clear();
                for contract in item.contracts() {
                    self.check_contract_clause(contract);
                }
            }
            HirTopLevelDecl::State(item) => {
                self.active_borrows.clear();
                self.borrow_local_lifetimes.clear();
                self.locals.clear();
                self.loop_stack.clear();
                for field in item.fields() {
                    self.check_expr(field.default());
                }
            }
            HirTopLevelDecl::TypeAlias(item) => {
                let outer_locals = self.locals.clone();
                self.locals
                    .insert("self".to_owned(), type_ref_kind(item.target()));
                for clause in item.where_clauses() {
                    self.check_expr(clause);
                }
                self.locals = outer_locals;
            }
            HirTopLevelDecl::Hook(item) => {
                self.expect_entity_kind(item.id(), &EntityKind::Hook, "hook id");
                let effect_scope = EffectScope::from_effects(item.effects());
                let effect_snapshot = self.apply_effect_scope(&effect_scope);
                self.check_block_expr(item.body_statements(), None);
                self.effect_capabilities = effect_snapshot;
            }
            HirTopLevelDecl::MemoFn(item) => {
                self.active_borrows.clear();
                self.borrow_local_lifetimes.clear();
                self.locals.clear();
                self.loop_stack.clear();
                self.yield_stack.clear();
                self.check_block_expr(item.body_statements(), item.body_value());
            }
            HirTopLevelDecl::Parser(item) => {
                self.active_borrows.clear();
                self.borrow_local_lifetimes.clear();
                self.locals.clear();
                self.loop_stack.clear();
                self.yield_stack.clear();
                self.check_block_expr(item.body_statements(), item.body_value());
            }
            HirTopLevelDecl::Source(item) => {
                self.active_borrows.clear();
                self.borrow_local_lifetimes.clear();
                self.locals.clear();
                self.loop_stack.clear();
                self.yield_stack.clear();
                if let Some(id) = item.id() {
                    self.expect_entity_kind(id, &EntityKind::Source, "source id");
                }
                self.check_source_item(item);
            }
        }
    }

    fn check_entry_item(&mut self, item: &EntryItem) {
        match item {
            EntryItem::Start(target) | EntryItem::Run(target) => {
                self.expect_entity_kind(target, &EntityKind::Flow, "entry flow target");
            }
            EntryItem::Route { target, .. } => {
                self.expect_entity_kind(target, &EntityKind::Flow, "entry route target");
            }
            EntryItem::Option { value, .. } => {
                self.check_expr(value);
            }
            EntryItem::Raw(raw) => {
                self.errors.push(TypeCheckError::new(format!(
                    "raw entry item is not type-checkable: {raw}"
                )));
            }
        }
    }

    pub(super) fn check_stream_function(
        &mut self,
        function: &arcweft_lang_hir::model::HirFunction,
    ) {
        let Some((item_ty, error_ty)) = function
            .signature()
            .return_type()
            .and_then(stream_return_types)
        else {
            self.errors.push(TypeCheckError::new(format!(
                "`stream fn {}` must declare `-> Stream<T, E>`",
                function.name()
            )));
            self.check_block_expr(function.statements(), function.value());
            return;
        };
        self.yield_stack.push(YieldContext::Stream {
            item_ty,
            error_ty,
            yield_count: 0,
        });
        self.check_block_expr(function.statements(), None);
        let value_is_stream_block = matches!(
            function.value(),
            Some(Expr::ComputationBlock {
                kind: ComputationBlockKind::Stream,
                ..
            })
        );
        if let Some(value) = function.value() {
            self.check_expr(value);
        }
        let Some(YieldContext::Stream { yield_count, .. }) = self.yield_stack.pop() else {
            return;
        };
        if yield_count == 0 && !value_is_stream_block {
            self.errors.push(TypeCheckError::new(format!(
                "`stream fn {}` does not yield any item",
                function.name()
            )));
        }
    }

    pub(super) fn check_choice_binding(
        &mut self,
        pattern: &Pattern,
        choice: &arcweft_lang_hir::model::HirChoice,
    ) {
        self.check_choice(choice);
        if let Some(name) = ident_pattern_name(pattern)
            && let Some(ty) = choice_output_type(choice)
        {
            self.locals.insert(name.to_owned(), ty);
        }
    }

    pub(super) fn check_loop_binding(
        &mut self,
        pattern: &Pattern,
        block: &arcweft_lang_hir::model::HirLoop,
    ) {
        let ty = self.check_loop_block(block, true);
        if let (Some(name), Some(ty)) = (ident_pattern_name(pattern), ty) {
            self.locals.insert(name.to_owned(), ty);
        }
    }

    pub(super) fn check_await_binding(
        &mut self,
        pattern: &Pattern,
        await_with: &arcweft_lang_hir::model::HirAwait,
    ) {
        let ty = self.check_await_item(await_with);
        if let (Some(name), Some(ty)) = (ident_pattern_name(pattern), ty) {
            self.locals.insert(name.to_owned(), ty);
        }
    }

    pub(super) fn check_dialogue_item(&mut self, dialogue: &arcweft_lang_hir::model::HirDialogue) {
        if !self.is_dialogue_callee(dialogue.callee()) {
            self.errors.push(TypeCheckError::new(format!(
                "dialogue callee `{}` must resolve to Ref<Character> or SpeakerPreset",
                dialogue.callee()
            )));
        }
        if let Some(id) = dialogue.id() {
            self.expect_entity_kind(id, &EntityKind::DialogueLine, "dialogue line id");
        }
        if let Some(text_key) = dialogue.text_key() {
            self.expect_entity_kind(text_key, &EntityKind::Text, "dialogue text key");
        }
        if let Some(look) = dialogue.look() {
            self.check_expr(look);
        }
        if let Some(stage) = dialogue.stage() {
            self.check_expr(stage);
        }
        if let Some(portrait) = dialogue.portrait() {
            self.check_expr(portrait);
        }
        let marks = self.check_dialogue_content(dialogue.content().tokens());
        self.with_line_runtime_scope(|checker| {
            if let Some(focus) = dialogue.focus() {
                checker.check_expr(focus);
                checker.lifetime_guarantees.insert(LifetimeKey::new(
                    LifetimeScopeKind::Line,
                    vec!["focus".to_owned()],
                ));
            }
            if let Some(cleanup) = dialogue.cleanup() {
                checker.check_expr(cleanup);
            }
            if let Some(plan) = dialogue.plan() {
                checker.line_out_depth += 1;
                checker
                    .line_label_stack
                    .push(plan.label().map(str::to_owned));
                checker.line_mark_stack.push(marks);
                for item in plan.items() {
                    checker.check_line_plan_item(item);
                }
                checker.line_mark_stack.pop();
                checker.line_label_stack.pop();
                checker.line_out_depth -= 1;
            }
        });
    }
}

fn types_compatible(expected: &TypeKind, actual: &TypeKind) -> bool {
    if expected == actual || matches!(expected, TypeKind::Named(name) if name == "_") {
        return true;
    }
    match (expected, actual) {
        (
            TypeKind::Result {
                ok: expected_ok,
                error: expected_error,
            },
            TypeKind::Result {
                ok: actual_ok,
                error: actual_error,
            },
        ) => {
            types_compatible(expected_ok, actual_ok)
                && (types_compatible(expected_error, actual_error)
                    || matches!(actual_error.as_ref(), TypeKind::Named(name) if name == "_"))
        }
        (TypeKind::Option(expected), TypeKind::Option(actual)) => {
            types_compatible(expected, actual)
                || matches!(actual.as_ref(), TypeKind::Named(name) if name == "_")
        }
        _ => false,
    }
}
