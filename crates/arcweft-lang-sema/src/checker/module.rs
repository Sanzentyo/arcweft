//! Module, top-level declaration, and dialogue entry checks.

use super::{
    EffectScope, EntityKind, FlowKind, FunctionKind, FunctionSignature, HirModule, HirTopLevelDecl,
    LifetimeKey, LifetimeScopeKind, Pattern, Stmt, TypeCheckError, TypeChecker, TypeKind,
    YieldContext, choice_output_type, entity_kind_for_decl, function_param_local_type,
    function_signature_type, ident_pattern_name, normalize_choice_type, stream_return_types,
    type_ref_kind, validate_typecheck_ready,
};
use crate::checker::helpers::{type_kind_label, type_ref_label};
use crate::effect_model::{
    CallableId, CallableKind, EffectContract, Visibility as EffectVisibility,
};
use crate::effects::EffectSet;
use arcweft_lang_hir::model::{HirAgent, HirFlow, HirFunction};
use arcweft_lang_syntax::ast::common::Visibility;
use arcweft_lang_syntax::ast::items::{
    EntityDeclItem, EntryItem, EntryRouteBinding, EntryRouteBindingSource, ExternModItem,
    ExternModMember, TypeAliasItem,
};
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
        self.register_effect_callables(module);
        self.flow_params = collect_flow_params(module);

        self.check_module_agents(module.agents());
        self.check_module_flows(module.flows());
        self.check_module_functions(module.functions());
        for declaration in module.declarations() {
            self.check_top_level_decl(declaration);
        }
        self.check_flow_items(module.top_level_items());
    }

    fn check_module_agents(&mut self, agents: &[HirAgent]) {
        for agent in agents {
            self.clear_borrow_state();
            self.locals.clear();
            self.loop_stack.clear();
            self.yield_stack.clear();
            self.active_presentation_defaults.clear();
            let item = agent.item();
            if let Some(id) = item.id() {
                self.expect_entity_kind(id, &EntityKind::Agent, "agent id");
            }
            let expected_return = item
                .signature()
                .and_then(FnSignature::return_type)
                .map(type_ref_kind);
            if let Some(signature) = item.signature() {
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
            for contract in item.contracts() {
                self.check_contract_clause(contract);
            }
            let effect_scope = EffectScope::from_contracts(item.contracts());
            let effect_snapshot = self.apply_effect_scope(&effect_scope);
            let previous_callable = self
                .effect_collector
                .enter(CallableId::new(format!("agent.{}", item.name())));
            let actual = self.with_expected_return(expected_return.as_ref(), |this| {
                this.check_function_body_expr(
                    item.body_statements(),
                    item.body_value(),
                    expected_return.as_ref(),
                )
            });
            self.effect_collector.restore(previous_callable);
            self.effect_capabilities = effect_snapshot;
            if let (Some(expected), Some(actual)) = (expected_return, actual)
                && !self.types_compatible(&expected, &actual)
            {
                self.errors.push(TypeCheckError::new(format!(
                    "agent `{}` returns {expected:?}, but body has {actual:?}",
                    item.name()
                )));
            }
        }
    }

    fn check_module_flows(&mut self, flows: &[HirFlow]) {
        for flow in flows {
            self.clear_borrow_state();
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
            let previous_callable = flow
                .name()
                .map(flow_callable_id)
                .map(|id| self.effect_collector.enter(id));
            self.with_expected_return(expected_return.as_ref(), |this| {
                this.check_flow_items(flow.body());
            });
            if let Some(previous_callable) = previous_callable {
                self.effect_collector.restore(previous_callable);
            }
            self.effect_capabilities = effect_snapshot;
        }
    }

    fn check_module_functions(&mut self, functions: &[HirFunction]) {
        for function in functions {
            self.clear_borrow_state();
            self.locals.clear();
            self.loop_stack.clear();
            self.yield_stack.clear();
            self.active_presentation_defaults.clear();
            self.warn_public_signature_anonymous_sum(function);
            self.check_signature_type_refs(function.signature());
            for group in function.signature().param_groups() {
                for param in group.params() {
                    self.bind_function_param(param.pattern(), &function_param_local_type(param));
                }
            }
            let expected_return = function.signature().return_type().map(type_ref_kind);
            for contract in function.contracts() {
                self.check_function_contract_clause(contract, expected_return.as_ref());
            }
            let effect_scope = EffectScope::from_contracts(function.contracts());
            let effect_snapshot = self.apply_effect_scope(&effect_scope);
            let previous_callable = self
                .effect_collector
                .enter(function_callable_id(function.name()));
            if function.kind() == FunctionKind::Stream {
                self.check_stream_function(function);
                self.effect_collector.restore(previous_callable);
                self.effect_capabilities = effect_snapshot;
                continue;
            }
            let actual = self.with_expected_return(expected_return.as_ref(), |this| {
                this.check_function_body_expr(
                    function.statements(),
                    function.value(),
                    expected_return.as_ref(),
                )
            });
            self.effect_collector.restore(previous_callable);
            self.effect_capabilities = effect_snapshot;
            if let (Some(expected), Some(actual)) = (expected_return, actual)
                && !self.types_compatible(&expected, &actual)
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

    fn warn_public_signature_anonymous_sum(&mut self, function: &HirFunction) {
        if function.visibility() != Some(Visibility::Public) {
            return;
        }
        for group in function.signature().param_groups() {
            for param in group.params() {
                self.warn_public_type_ref_anonymous_sum(
                    param.ty(),
                    &format!(
                        "public function `{}` parameter `{}`",
                        function.name(),
                        pattern_public_label(param.pattern())
                    ),
                );
            }
        }
        if let Some(return_type) = function.signature().return_type() {
            self.warn_public_type_ref_anonymous_sum(
                return_type,
                &format!("public function `{}` return type", function.name()),
            );
        }
    }

    fn warn_public_type_ref_anonymous_sum(&mut self, ty: &TypeRef, context: &str) {
        if !type_ref_contains_choice(ty) {
            return;
        }
        self.warnings.push(
            crate::diagnostics::TypeCheckWarning::public_abi_anonymous_sum(
                context,
                type_ref_label(ty),
            ),
        );
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
            for alias in entity_symbol_aliases(item) {
                self.global_symbols.insert(
                    alias,
                    TypeKind::entity_ref(entity_kind_for_decl(item.kind())),
                );
            }
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

    fn register_effect_callables(&mut self, module: &HirModule) {
        let entry_flow_names = entry_target_flow_names(module);
        for agent in module.agents() {
            let item = agent.item();
            let contract = effect_contract_from_contracts(
                item.contracts(),
                agent.has_attribute("pure"),
                &mut self.errors,
            );
            let source_name = format!("agent.{}", item.name());
            self.register_effect_callable(
                &source_name,
                CallableId::new(format!("agent.{}", item.name())),
                CallableKind::Agent,
                EffectVisibility::Boundary,
                contract,
            );
        }
        for flow in module.flows() {
            if let Some(name) = flow.name() {
                let entry_boundary = entry_flow_names.contains(name);
                let contract = effect_contract_from_contracts(
                    flow.contracts(),
                    flow.has_attribute("pure"),
                    &mut self.errors,
                );
                self.register_effect_callable(
                    name,
                    flow_callable_id(name),
                    match flow.kind() {
                        FlowKind::Flow => CallableKind::Flow,
                        FlowKind::Fragment => CallableKind::Fragment,
                    },
                    if entry_boundary {
                        EffectVisibility::Boundary
                    } else {
                        EffectVisibility::Private
                    },
                    contract,
                );
            }
        }
        for function in module.functions() {
            let contract = effect_contract_from_contracts(
                function.contracts(),
                function.has_attribute("pure"),
                &mut self.errors,
            );
            self.register_effect_callable(
                function.name(),
                function_callable_id(function.name()),
                CallableKind::Function,
                effect_visibility_from_syntax(function.visibility()),
                contract,
            );
        }
    }

    fn register_effect_callable(
        &mut self,
        source_name: &str,
        id: CallableId,
        kind: CallableKind,
        visibility: EffectVisibility,
        contract: EffectContract,
    ) {
        if let Err(error) =
            self.effect_collector
                .register_callable(source_name, id, kind, visibility, contract)
        {
            self.errors.push(TypeCheckError::new(error.to_string()));
        }
    }

    pub(super) fn check_top_level_decl(&mut self, declaration: &HirTopLevelDecl) {
        match declaration {
            HirTopLevelDecl::DialogueDefaults(_)
            | HirTopLevelDecl::Enum(_)
            | HirTopLevelDecl::Impl(_)
            | HirTopLevelDecl::Proof(_)
            | HirTopLevelDecl::Struct(_)
            | HirTopLevelDecl::Trait(_)
            | HirTopLevelDecl::TrustedAxiom(_)
            | HirTopLevelDecl::ExternCapability(_) => {}
            HirTopLevelDecl::ExternMod(item) => self.check_extern_mod(item),
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
                self.clear_borrow_state();
                self.locals.clear();
                self.loop_stack.clear();
                for contract in item.contracts() {
                    self.check_contract_clause(contract);
                }
            }
            HirTopLevelDecl::State(item) => {
                self.clear_borrow_state();
                self.locals.clear();
                self.loop_stack.clear();
                for field in item.fields() {
                    self.check_expr(field.default());
                }
            }
            HirTopLevelDecl::TypeAlias(item) => {
                self.check_type_alias_decl(item);
            }
            HirTopLevelDecl::Hook(item) => {
                self.expect_entity_kind(item.id(), &EntityKind::Hook, "hook id");
                let effect_scope = EffectScope::from_effects(item.effects());
                let effect_snapshot = self.apply_effect_scope(&effect_scope);
                self.check_block_expr(item.body_statements(), None);
                self.effect_capabilities = effect_snapshot;
            }
            HirTopLevelDecl::MemoFn(item) => {
                self.clear_borrow_state();
                self.locals.clear();
                self.loop_stack.clear();
                self.yield_stack.clear();
                self.check_block_expr(item.body_statements(), item.body_value());
            }
            HirTopLevelDecl::Parser(item) => {
                self.clear_borrow_state();
                self.locals.clear();
                self.loop_stack.clear();
                self.yield_stack.clear();
                self.check_block_expr(item.body_statements(), item.body_value());
            }
            HirTopLevelDecl::Source(item) => {
                self.clear_borrow_state();
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

    fn check_extern_mod(&mut self, item: &ExternModItem) {
        if item.abi() != "rust" {
            return;
        }
        let Some(package) = item.source().and_then(extern_rust_crate_name) else {
            self.errors.push(TypeCheckError::new(format!(
                "extern rust module `{}` must declare `from crate \"name\"`",
                item.path()
            )));
            return;
        };
        let Some(exports) = self.env.rust_package(package) else {
            self.errors
                .push(TypeCheckError::missing_rust_package_metadata(package));
            return;
        };
        let namespace = item.path().replace("::", ".");
        for member in item.members() {
            match member {
                ExternModMember::Type(ty) => {
                    if !exports.has_type(ty.name()) {
                        self.errors.push(TypeCheckError::missing_rust_export(
                            package,
                            format!("{namespace}.{}", ty.name()),
                        ));
                    }
                }
                ExternModMember::Function(function) => {
                    self.check_signature_type_refs(function.signature());
                    let export_name = format!("{namespace}.{}", function.signature().name());
                    let expected = function_signature_type(function.signature());
                    let Some(actual) = exports.function(&export_name) else {
                        self.errors
                            .push(TypeCheckError::missing_rust_export(package, export_name));
                        continue;
                    };
                    if &expected != actual {
                        self.errors
                            .push(TypeCheckError::rust_export_signature_mismatch(
                                package,
                                export_name,
                                function_signature_label(&expected),
                                function_signature_label(actual),
                            ));
                    }
                }
                ExternModMember::Activity(activity) => {
                    self.check_type_ref_shape(activity.ty());
                }
                ExternModMember::Raw(raw) => {
                    self.errors.push(TypeCheckError::new(format!(
                        "raw extern rust member is not type-checkable: {raw}"
                    )));
                }
            }
        }
    }

    fn check_type_alias_decl(&mut self, item: &TypeAliasItem) {
        if item.visibility() == Some(Visibility::Public) {
            self.warn_public_type_ref_anonymous_sum(
                item.target(),
                &format!("public type alias `{}`", item.name()),
            );
        }
        self.check_type_ref_shape(item.target());
        self.with_local_mutation_scope(|this| {
            this.bind_local("self".to_owned(), type_ref_kind(item.target()));
            for clause in item.where_clauses() {
                this.check_expr(clause);
            }
        });
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
            self.bind_local(name.to_owned(), ty);
        }
    }

    pub(super) fn check_loop_binding(
        &mut self,
        pattern: &Pattern,
        block: &arcweft_lang_hir::model::HirLoop,
    ) {
        let ty = self.check_loop_block(block, true);
        if let (Some(name), Some(ty)) = (ident_pattern_name(pattern), ty) {
            self.bind_local(name.to_owned(), ty);
        }
    }

    pub(super) fn check_await_binding(
        &mut self,
        pattern: &Pattern,
        await_with: &arcweft_lang_hir::model::HirAwait,
    ) {
        let ty = self.check_await_item(await_with);
        if let (Some(name), Some(ty)) = (ident_pattern_name(pattern), ty) {
            self.bind_local(name.to_owned(), ty);
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
        self.check_dialogue_default_inline_failure_policy(dialogue);
        let marks = self.check_dialogue_content(
            dialogue.content().tokens(),
            dialogue_has_default_inline_failure_policy(dialogue),
        );
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

    fn check_dialogue_default_inline_failure_policy(
        &mut self,
        dialogue: &arcweft_lang_hir::model::HirDialogue,
    ) {
        let policy_args = dialogue
            .args()
            .iter()
            .filter(|arg| {
                matches!(
                    arg.name(),
                    "inline_fallback" | "inline_error" | "inline_error_policy"
                )
            })
            .collect::<Vec<_>>();
        if policy_args.len() > 1 {
            self.errors
                .push(TypeCheckError::inline_failure_policy_conflict(format!(
                    "{} default inline policy",
                    dialogue.callee()
                )));
        }
        for arg in policy_args {
            if matches!(arg.name(), "inline_error" | "inline_error_policy")
                && let Some(policy) = unknown_default_inline_failure_policy(arg.value())
            {
                self.errors
                    .push(TypeCheckError::unknown_inline_failure_policy(
                        format!("{} default inline policy", dialogue.callee()),
                        policy,
                    ));
            }
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

fn dialogue_has_default_inline_failure_policy(
    dialogue: &arcweft_lang_hir::model::HirDialogue,
) -> bool {
    dialogue.args().iter().any(|arg| {
        matches!(
            arg.name(),
            "inline_fallback" | "inline_error" | "inline_error_policy"
        )
    })
}

fn unknown_default_inline_failure_policy(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => unknown_default_inline_failure_atom(path),
        Expr::Field { target, field } => match target.as_ref() {
            Expr::Path(namespace) => unknown_default_inline_failure_field(namespace, field),
            _ => None,
        },
        Expr::Call { callee, args } => unknown_default_inline_failure_constructor(callee, args),
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => unknown_default_inline_failure_method_constructor(receiver, method, args),
        _ => None,
    }
}

fn unknown_default_inline_failure_constructor(
    callee: &Expr,
    args: &[arcweft_lang_syntax::expr::CallArg],
) -> Option<String> {
    let constructor = match callee {
        Expr::Path(path) if path == "fallback" => "fallback",
        Expr::Field { target, field } if matches!(target.as_ref(), Expr::Path(namespace) if namespace == "InlineFailure") => {
            field
        }
        _ => return None,
    };
    if constructor != "fallback" {
        return Some(default_inline_policy_label(callee));
    }
    args.iter().find_map(|arg| match arg {
        arcweft_lang_syntax::expr::CallArg::Positional(value) => {
            unknown_default_inline_fallback_value(value)
        }
        arcweft_lang_syntax::expr::CallArg::Named { name, value }
            if name == "value" || name == "text" =>
        {
            unknown_default_inline_fallback_value(value)
        }
        arcweft_lang_syntax::expr::CallArg::Named { .. }
        | arcweft_lang_syntax::expr::CallArg::Spread { .. } => None,
    })
}

fn unknown_default_inline_failure_method_constructor(
    receiver: &Expr,
    method: &str,
    args: &[arcweft_lang_syntax::expr::CallArg],
) -> Option<String> {
    if !matches!(receiver, Expr::Path(namespace) if namespace == "InlineFailure") {
        return None;
    }
    if method != "fallback" {
        return Some(format!("InlineFailure.{method}"));
    }
    args.iter().find_map(|arg| match arg {
        arcweft_lang_syntax::expr::CallArg::Positional(value) => {
            unknown_default_inline_fallback_value(value)
        }
        arcweft_lang_syntax::expr::CallArg::Named { name, value }
            if name == "value" || name == "text" =>
        {
            unknown_default_inline_fallback_value(value)
        }
        arcweft_lang_syntax::expr::CallArg::Named { .. }
        | arcweft_lang_syntax::expr::CallArg::Spread { .. } => None,
    })
}

fn unknown_default_inline_fallback_value(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => unknown_default_inline_fallback_atom(path),
        Expr::Field { target, field } => match target.as_ref() {
            Expr::Path(namespace) => unknown_default_inline_fallback_field(namespace, field),
            _ => None,
        },
        _ => None,
    }
}

fn unknown_default_inline_failure_atom(path: &str) -> Option<String> {
    let variant = path.strip_prefix('.')?;
    (!matches!(variant, "fail" | "discard" | "line_error")).then(|| path.to_owned())
}

fn unknown_default_inline_failure_field(namespace: &str, field: &str) -> Option<String> {
    (namespace == "InlineFailure" && !matches!(field, "fail" | "discard" | "line_error"))
        .then(|| format!("{namespace}.{field}"))
}

fn unknown_default_inline_fallback_atom(path: &str) -> Option<String> {
    let variant = path.strip_prefix('.')?;
    (!matches!(variant, "expr_source" | "call_source" | "value_plain")).then(|| path.to_owned())
}

fn unknown_default_inline_fallback_field(namespace: &str, field: &str) -> Option<String> {
    (namespace == "InlineFallback"
        && !matches!(field, "expr_source" | "call_source" | "value_plain"))
    .then(|| format!("{namespace}.{field}"))
}

fn default_inline_policy_label(expr: &Expr) -> String {
    match expr {
        Expr::Path(path) => path.clone(),
        Expr::Field { target, field } => format!("{}.{field}", default_inline_policy_label(target)),
        _ => format!("{expr:?}"),
    }
}

fn type_ref_contains_choice(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Choice(_) => true,
        TypeRef::Generic { args, .. } => args.iter().any(type_ref_contains_choice),
        TypeRef::Ref { inner, .. } | TypeRef::Slice(inner) => type_ref_contains_choice(inner),
        TypeRef::Never | TypeRef::ConstInt(_) | TypeRef::Path(_) => false,
    }
}

fn pattern_public_label(pattern: &Pattern) -> String {
    ident_pattern_name(pattern).map_or_else(|| format!("{pattern:?}"), ToOwned::to_owned)
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

fn entity_symbol_aliases(item: &EntityDeclItem) -> Vec<String> {
    [
        item.surface_alias().map(str::to_owned),
        item.name().map(str::to_owned),
        item.id()
            .body()
            .rsplit_once('.')
            .map(|(_, suffix)| suffix.to_owned()),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn route_path_params(path: &str) -> HashSet<String> {
    path.trim_matches('/')
        .split('/')
        .filter_map(|segment| segment.strip_prefix(':'))
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn entry_target_flow_names(module: &HirModule) -> HashSet<String> {
    let targets = module
        .declarations()
        .iter()
        .filter_map(|declaration| match declaration {
            HirTopLevelDecl::Entry(entry) => Some(entry.items()),
            _ => None,
        })
        .flatten()
        .filter_map(entry_item_flow_target)
        .map(|target| target.body().to_owned())
        .collect::<HashSet<_>>();

    module
        .flows()
        .iter()
        .filter_map(|flow| {
            let name = flow.name()?;
            let matched = flow.id().is_some_and(|id| targets.contains(id.body()))
                || targets
                    .iter()
                    .filter_map(|target| target.rsplit_once('.').map(|(_, suffix)| suffix))
                    .any(|suffix| suffix == name);
            matched.then_some(name.to_owned())
        })
        .collect()
}

fn entry_item_flow_target(item: &EntryItem) -> Option<&arcweft_lang_syntax::ast::ids::EntityRef> {
    match item {
        EntryItem::Start(target) | EntryItem::Run(target) | EntryItem::Route { target, .. } => {
            Some(target)
        }
        EntryItem::Option { .. } | EntryItem::Raw(_) => None,
    }
}

fn function_callable_id(name: &str) -> CallableId {
    CallableId::new(format!("fn.{name}"))
}

fn flow_callable_id(name: &str) -> CallableId {
    CallableId::new(format!("flow.{name}"))
}

fn effect_visibility_from_syntax(visibility: Option<Visibility>) -> EffectVisibility {
    match visibility {
        Some(Visibility::Public) => EffectVisibility::Public,
        Some(Visibility::Crate | Visibility::Super) | None => EffectVisibility::Private,
    }
}

fn effect_contract_from_contracts(
    contracts: &[super::ContractClause],
    pure: bool,
    errors: &mut Vec<TypeCheckError>,
) -> EffectContract {
    let declared = declared_effect_set_from_contracts(contracts, errors);
    let mut forbidden = EffectSet::new();
    for contract in contracts {
        let super::ContractClause::NoEffect(expr) = contract else {
            continue;
        };
        match crate::effect_contract::effect_id_from_expr(expr) {
            Ok(effect) => {
                forbidden.insert(effect);
            }
            Err(error) => errors.push(TypeCheckError::new(error.to_string())),
        }
    }
    if pure && declared.as_ref().is_some_and(|effects| !effects.is_empty()) {
        errors.push(TypeCheckError::new(format!(
            "pure callable cannot declare non-empty effects {}",
            declared
                .as_ref()
                .expect("checked non-empty declared effects")
        )));
    }

    if pure {
        EffectContract::pure()
    } else if let Some(declared) = declared {
        EffectContract::bounded(declared)
    } else {
        EffectContract::inferred()
    }
    .with_forbidden(forbidden)
}

fn declared_effect_set_from_contracts(
    contracts: &[super::ContractClause],
    errors: &mut Vec<TypeCheckError>,
) -> Option<EffectSet> {
    let mut declared = None::<EffectSet>;
    for contract in contracts {
        let super::ContractClause::Effects(items) = contract else {
            continue;
        };
        let effects = declared.get_or_insert_with(EffectSet::new);
        for item in items {
            match crate::effect_contract::effect_id_from_expr(item) {
                Ok(effect) => {
                    effects.insert(effect);
                }
                Err(error) => errors.push(TypeCheckError::new(error.to_string())),
            }
        }
    }
    declared
}

fn extern_rust_crate_name(source: &str) -> Option<&str> {
    let source = source.trim();
    let name = source.strip_prefix("crate")?.trim();
    name.strip_prefix('"')?.strip_suffix('"')
}

fn function_signature_label(signature: &FunctionSignature) -> String {
    let params = signature
        .params()
        .iter()
        .map(|param| {
            param.name().map_or_else(
                || format!("{:?}", param.ty()),
                |name| format!("{name}: {:?}", param.ty()),
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("fn({params}) -> {:?}", signature.return_type())
}
