//! Module, top-level declaration, and dialogue entry checks.

use super::{
    EffectScope, EntityKind, FlowKind, FunctionKind, HirModule, HirTopLevelDecl, LifetimeKey,
    LifetimeScopeKind, Pattern, Stmt, TypeCheckError, TypeChecker, TypeKind, YieldContext,
    choice_output_type, entity_kind_for_decl, function_param_local_type, function_signature_type,
    ident_pattern_name, normalize_choice_type, stream_return_types, type_ref_kind,
    types_compatible, validate_typecheck_ready,
};
use crate::checker::helpers::type_kind_label;
use arcweft_lang_hir::model::{HirFlow, HirFunction};
use arcweft_lang_syntax::ast::items::{EntryItem, EntryRouteBinding, EntryRouteBindingSource};
use arcweft_lang_syntax::expr::{ComputationBlockKind, Expr};
use arcweft_lang_syntax::types::{FnSignature, TypeRef};
use std::collections::{HashMap, HashSet};

impl TypeChecker<'_> {
    pub(super) fn check_module(&mut self, module: &HirModule) {
        self.stats.flows += module.flows().len();
        self.stats.functions += module.functions().len();
        self.stats.declarations += module.declarations().len();
        self.stats.top_level_items += module.top_level_items().len();

        if let Err(errors) = validate_typecheck_ready(module) {
            self.errors.extend(
                errors
                    .into_iter()
                    .map(|error| TypeCheckError::new(error.message().to_owned())),
            );
        }

        self.bind_top_level_entity_aliases(module);
        self.bind_top_level_type_aliases(module);
        self.bind_top_level_functions(module);
        self.bind_extern_capability_functions(module);
        self.flow_params = collect_flow_params(module);

        self.check_module_flows(module.flows());
        self.check_module_functions(module.functions());
        for declaration in module.declarations() {
            self.check_top_level_decl(declaration);
        }
        self.check_flow_items(module.top_level_items());
    }

    fn check_module_flows(&mut self, flows: &[HirFlow]) {
        for flow in flows {
            self.active_borrows.clear();
            self.borrow_local_lifetimes.clear();
            self.locals.clear();
            self.loop_stack.clear();
            self.active_presentation_defaults.clear();
            self.line_mark_stack.clear();
            self.lifetime_guarantees.clear();
            self.dropped_lifetime_keys.clear();
            if let Some(signature) = flow.signature() {
                self.check_signature_type_refs(signature);
                for group in signature.param_groups() {
                    for param in group.params() {
                        self.bind_function_param(
                            param.pattern(),
                            &function_param_local_type(param),
                        );
                    }
                }
            }
            let expected_return = flow
                .signature()
                .and_then(|signature| signature.return_type())
                .map(type_ref_kind);
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
            self.with_expected_return(expected_return.as_ref(), |this| {
                this.check_flow_items(flow.body());
            });
            self.effect_capabilities = effect_snapshot;
        }
    }

    fn check_module_functions(&mut self, functions: &[HirFunction]) {
        for function in functions {
            self.active_borrows.clear();
            self.borrow_local_lifetimes.clear();
            self.locals.clear();
            self.loop_stack.clear();
            self.yield_stack.clear();
            self.active_presentation_defaults.clear();
            self.check_signature_type_refs(function.signature());
            for group in function.signature().param_groups() {
                for param in group.params() {
                    self.bind_function_param(param.pattern(), &function_param_local_type(param));
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
            let expected_return = function.signature().return_type().map(type_ref_kind);
            let actual = self.with_expected_return(expected_return.as_ref(), |this| {
                this.check_function_body_expr(
                    function.statements(),
                    function.value(),
                    expected_return.as_ref(),
                )
            });
            self.effect_capabilities = effect_snapshot;
            if let (Some(expected), Some(actual)) = (expected_return, actual)
                && !types_compatible(&expected, &actual)
            {
                self.errors.push(TypeCheckError::new(format!(
                    "function `{}` returns {expected:?}, but body has {actual:?}",
                    function.name()
                )));
            }
        }
    }

    fn check_function_body_expr(
        &mut self,
        statements: &[Stmt],
        value: Option<&Expr>,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        if value.is_some() {
            return self.check_block_expr_with_expected(statements, value, expected);
        }
        match statements.split_last() {
            Some((Stmt::Return(expr), statements)) => {
                self.check_tail_return_block_expr_with_expected(statements, expr, expected)
            }
            _ => self.check_block_expr(statements, None),
        }
    }

    fn with_expected_return<R>(
        &mut self,
        expected: Option<&TypeKind>,
        check: impl FnOnce(&mut Self) -> R,
    ) -> R {
        if let Some(expected) = expected {
            self.expected_returns.push(expected.clone());
            let result = check(self);
            self.expected_returns.pop();
            result
        } else {
            check(self)
        }
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

    fn bind_top_level_type_aliases(&mut self, module: &HirModule) {
        for declaration in module.declarations() {
            let HirTopLevelDecl::TypeAlias(item) = declaration else {
                continue;
            };
            self.global_type_aliases
                .insert(item.name().to_owned(), type_ref_kind(item.target()));
        }
    }

    fn bind_extern_capability_functions(&mut self, module: &HirModule) {
        for declaration in module.declarations() {
            let HirTopLevelDecl::ExternCapability(item) = declaration else {
                continue;
            };
            for function in item.functions() {
                self.check_signature_type_refs(function.signature());
                let return_type = function
                    .signature()
                    .return_type()
                    .map_or(TypeKind::Unit, type_ref_kind);
                let name = format!("{}.{}", item.id(), function.signature().name());
                self.global_functions.insert(name.clone(), return_type);
                self.global_function_signatures
                    .insert(name.clone(), function_signature_type(function.signature()));
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
            self.check_signature_type_refs(function.signature());
            let return_type = function
                .signature()
                .return_type()
                .map_or(TypeKind::Unit, type_ref_kind);
            self.global_functions
                .insert(function.name().to_owned(), return_type);
            self.global_function_signatures.insert(
                function.name().to_owned(),
                function_signature_type(function.signature()),
            );
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
                self.check_type_ref_shape(item.target());
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
            EntryItem::Route {
                target,
                path,
                bindings,
                ..
            } => {
                self.expect_entity_kind(target, &EntityKind::Flow, "entry route target");
                self.check_route_bindings(target, path, bindings);
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

    fn check_route_bindings(
        &mut self,
        target: &arcweft_lang_syntax::ast::ids::EntityRef,
        path: &str,
        bindings: &[EntryRouteBinding],
    ) {
        let path_params = route_path_params(path);
        for binding in bindings {
            match binding.source() {
                EntryRouteBindingSource::PathParam(param) if !path_params.contains(param) => {
                    self.errors.push(TypeCheckError::new(format!(
                        "route binding `{}` references missing path parameter `:{param}`",
                        binding.name()
                    )));
                }
                EntryRouteBindingSource::PathParam(_) => {}
            }
        }

        let Some(flow_params) = self.flow_params.get(target.body()) else {
            return;
        };
        for binding in bindings {
            if !flow_params.contains(binding.name()) {
                self.errors.push(TypeCheckError::new(format!(
                    "route target `{}` has no flow parameter named `{}`",
                    target.body(),
                    binding.name()
                )));
            }
        }
        for param in flow_params {
            if !bindings.iter().any(|binding| binding.name() == param) {
                self.errors.push(TypeCheckError::new(format!(
                    "route target `{}` requires explicit binding for flow parameter `{param}`",
                    target.body()
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

    fn check_signature_type_refs(&mut self, signature: &FnSignature) {
        for param in signature
            .param_groups()
            .iter()
            .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
        {
            self.check_type_ref_shape(param.ty());
        }
        if let Some(return_type) = signature.return_type() {
            self.check_type_ref_shape(return_type);
        }
        for clause in signature.where_clauses() {
            self.check_type_ref_shape(clause.subject());
            for bound in clause.bounds() {
                self.check_type_ref_shape(bound);
            }
        }
    }

    fn check_type_ref_shape(&mut self, ty: &TypeRef) {
        match ty {
            TypeRef::Choice(alternatives) => {
                let mut erased = HashMap::<String, String>::new();
                for alternative in alternatives {
                    self.check_type_ref_shape(alternative);
                    let source_label = crate::checker::helpers::type_ref_label(alternative);
                    let erased_label =
                        type_kind_label(&self.erase_aliases(&type_ref_kind(alternative)));
                    if let Some(previous) =
                        erased.insert(erased_label.clone(), source_label.clone())
                    {
                        self.errors.push(TypeCheckError::new(format!(
                            "anonymous sum alternatives `{previous}` and `{source_label}` erase to the same type `{erased_label}`"
                        )));
                    }
                }
            }
            TypeRef::Generic { args, .. } => {
                for arg in args {
                    self.check_type_ref_shape(arg);
                }
            }
            TypeRef::Ref { inner, .. } | TypeRef::Slice(inner) => self.check_type_ref_shape(inner),
            TypeRef::Never | TypeRef::ConstInt(_) | TypeRef::Path(_) => {}
        }
    }

    fn erase_aliases(&self, ty: &TypeKind) -> TypeKind {
        self.erase_aliases_with_seen(ty, &mut HashSet::new())
    }

    fn erase_aliases_with_seen(&self, ty: &TypeKind, seen: &mut HashSet<String>) -> TypeKind {
        match ty {
            TypeKind::Named(name) => {
                if !seen.insert(name.clone()) {
                    return ty.clone();
                }
                self.global_type_aliases.get(name).map_or_else(
                    || ty.clone(),
                    |aliased| self.erase_aliases_with_seen(aliased, seen),
                )
            }
            TypeKind::Vec(inner) => {
                TypeKind::Vec(Box::new(self.erase_aliases_with_seen(inner, seen)))
            }
            TypeKind::Array { item, len } => TypeKind::Array {
                item: Box::new(self.erase_aliases_with_seen(item, seen)),
                len: len.clone(),
            },
            TypeKind::Slice(inner) => {
                TypeKind::Slice(Box::new(self.erase_aliases_with_seen(inner, seen)))
            }
            TypeKind::Seq(inner) => {
                TypeKind::Seq(Box::new(self.erase_aliases_with_seen(inner, seen)))
            }
            TypeKind::Map { kind, key, value } => TypeKind::Map {
                kind: *kind,
                key: Box::new(self.erase_aliases_with_seen(key, seen)),
                value: Box::new(self.erase_aliases_with_seen(value, seen)),
            },
            TypeKind::BorrowRef { lifetime, inner } => TypeKind::BorrowRef {
                lifetime: lifetime.clone(),
                inner: Box::new(self.erase_aliases_with_seen(inner, seen)),
            },
            TypeKind::Need { ready, error } => TypeKind::Need {
                ready: Box::new(self.erase_aliases_with_seen(ready, seen)),
                error: Box::new(self.erase_aliases_with_seen(error, seen)),
            },
            TypeKind::Stream { item, error } => TypeKind::Stream {
                item: Box::new(self.erase_aliases_with_seen(item, seen)),
                error: Box::new(self.erase_aliases_with_seen(error, seen)),
            },
            TypeKind::Source { item, error } => TypeKind::Source {
                item: Box::new(self.erase_aliases_with_seen(item, seen)),
                error: Box::new(self.erase_aliases_with_seen(error, seen)),
            },
            TypeKind::Result { ok, error } => TypeKind::Result {
                ok: Box::new(self.erase_aliases_with_seen(ok, seen)),
                error: Box::new(self.erase_aliases_with_seen(error, seen)),
            },
            TypeKind::Option(inner) => {
                TypeKind::Option(Box::new(self.erase_aliases_with_seen(inner, seen)))
            }
            TypeKind::ThreadHandle(inner) => {
                TypeKind::ThreadHandle(Box::new(self.erase_aliases_with_seen(inner, seen)))
            }
            TypeKind::Shared(inner) => {
                TypeKind::Shared(Box::new(self.erase_aliases_with_seen(inner, seen)))
            }
            TypeKind::Function { return_type } => TypeKind::Function {
                return_type: Box::new(self.erase_aliases_with_seen(return_type, seen)),
            },
            TypeKind::Tuple(items) => TypeKind::Tuple(
                items
                    .iter()
                    .map(|item| self.erase_aliases_with_seen(item, seen))
                    .collect(),
            ),
            TypeKind::Choice(alternatives) => normalize_choice_type(
                alternatives
                    .iter()
                    .map(|alternative| self.erase_aliases_with_seen(alternative, seen))
                    .collect(),
            ),
            _ => ty.clone(),
        }
    }
}

fn collect_flow_params(module: &HirModule) -> std::collections::HashMap<String, HashSet<String>> {
    module
        .flows()
        .iter()
        .filter_map(|flow| {
            Some((
                flow.id()?.body().to_owned(),
                flow.signature()
                    .map(flow_signature_params)
                    .unwrap_or_default(),
            ))
        })
        .collect()
}

fn flow_signature_params(signature: &FnSignature) -> HashSet<String> {
    signature
        .param_groups()
        .iter()
        .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
        .filter_map(|param| route_bindable_pattern_name(param.pattern()))
        .collect()
}

fn route_bindable_pattern_name(pattern: &Pattern) -> Option<String> {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
            Some(name.clone())
        }
        _ => None,
    }
}

fn route_path_params(path: &str) -> HashSet<String> {
    path.trim_matches('/')
        .split('/')
        .filter_map(|segment| segment.strip_prefix(':'))
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}
