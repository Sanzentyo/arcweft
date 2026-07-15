//! Module, top-level declaration, and dialogue entry checks.

use super::line_plan::DialogueContentRangeMode;
use super::{
    ActionParam, ActionSignature, EffectScope, EntityKind, EnumVariantPayload, FunctionKind,
    FunctionSignature, FxCatalog, HirModule, HirTopLevelDecl, LifetimeKey, LifetimeScopeKind,
    NominalTypeContext, Pattern, Stmt, TypeCheckEnv, TypeCheckError, TypeCheckReport,
    TypeCheckWarning, TypeChecker, TypeExpressionId, TypeKind, TypedLoweringEvidenceKind,
    YieldContext, choice_output_type, entity_kind_for_decl, entity_syntax_kind,
    function_callable_id, function_param_local_type, function_param_local_type_with_generics,
    function_signature_type, function_signature_type_with_nominal_types, ident_pattern_name,
    normalize_choice_type, signature_generic_names, stream_return_types, type_ref_kind,
    type_ref_kind_with_generics, validate_typecheck_ready,
};
use crate::canonicalization::{
    CanonicalizationSourceSet, CheckedCanonicalizationInventory, SemanticDataUnavailable,
};
use crate::checker::helpers::{type_kind_label, type_ref_label};
use crate::dialogue_view::{
    DialogueViewModelRegistry, DialogueViewProjection, STANDARD_DIALOGUE_VIEW_RESOURCE,
};
use crate::effect_model::{
    CallableId, CallableKind, EffectContract, Visibility as EffectVisibility,
};
use crate::effects::EffectSet;
use crate::style::check_view_styles;
use arcweft_lang_hir::model::{HirAgent, HirFlow, HirFunction};
use arcweft_lang_hir::project::HirProject;
use arcweft_lang_hir::style::HirStyleDecl;
use arcweft_lang_hir::symbol::CallableDeclarationId;
use arcweft_lang_syntax::ast::common::Visibility;
use arcweft_lang_syntax::ast::flow::AuthoredExpr;
use arcweft_lang_syntax::ast::items::{
    EntityDeclItem, EntityDeclKind, EntryItem, EntryRouteBinding, EntryRouteBindingSource,
    EnumItem, EnumVariant, ExternModItem, ExternModMember, ImplItem, ImplMember, StructItem,
    TypeAliasItem,
};
use arcweft_lang_syntax::ast::view::{ViewActionInvokeAction, ViewActionPayload};
use arcweft_lang_syntax::expr::{ComputationBlockKind, Expr};
use arcweft_lang_syntax::types::{
    FnParam, FnSignature, TypeRef, parse_fn_signature, parse_type_ref,
};
use arcweft_source::SourceDocumentId;
use arcweft_source::SourceDocumentIdentity;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

impl TypeCheckReport {
    pub fn into_result(self) -> Result<(), Vec<TypeCheckError>> {
        if self.diagnostics.is_empty() {
            Ok(())
        } else {
            Err(self.diagnostics)
        }
    }

    /// Returns the effect-analysis callable owned by a function-valued expression.
    pub fn function_effect_callable_for_expression(
        &self,
        expression_id: TypeExpressionId,
    ) -> Option<&CallableId> {
        self.typed_lowering_evidence.iter().find_map(|evidence| {
            if evidence.expression_id != expression_id {
                return None;
            }
            let TypedLoweringEvidenceKind::FunctionEffectCallable { callable } = &evidence.kind
            else {
                return None;
            };
            Some(callable)
        })
    }

    /// Resolves inferred effect-row variables throughout a semantic type.
    pub fn resolved_type(
        &self,
        ty: &TypeKind,
    ) -> Result<TypeKind, crate::effect_row::EffectRowError> {
        ty.resolve_effect_rows_with(&mut |row| {
            if row.tail() == crate::effect_row::EffectRowTail::Unknown {
                Ok(row.clone())
            } else {
                self.effects
                    .resolve_effect_row(row)
                    .map(crate::effect_row::EffectRow::closed)
            }
        })
    }

    /// Returns canonicalization evidence for one exact registered source identity.
    pub fn canonicalization_inventory(
        &self,
        module: &arcweft_lang_syntax::ast::module_path::CanonicalModulePath,
        source: &SourceDocumentIdentity,
    ) -> Option<&CheckedCanonicalizationInventory> {
        self.canonicalization_inventories
            .iter()
            .find(|inventory| inventory.module() == module && inventory.source() == source)
    }
}

/// Analyzes lowered HIR with an explicit symbol/method environment.
pub fn analyze_types(module: &HirModule, env: &TypeCheckEnv) -> TypeCheckReport {
    let (style_catalog, style_diagnostics) = check_view_styles(module);
    let mut checker = TypeChecker::new(env);
    finish_type_check(module, style_catalog, style_diagnostics, &mut checker)
}

/// Analyzes linked project HIR through the sole registered semantic boundary.
pub fn analyze_registered_project_types(
    module: &HirModule,
    registered: &crate::registration::RegisteredSemanticWorld,
) -> TypeCheckReport {
    let (style_catalog, style_diagnostics) = check_view_styles(module);
    let mut checker = TypeChecker::new_with_project(
        registered.environment().base(),
        None,
        Some(registered.symbols()),
        Some(registered.environment()),
    );
    finish_type_check(module, style_catalog, style_diagnostics, &mut checker)
}

/// Analyzes a registered project while retaining exact-source speaker-line evidence.
pub fn analyze_registered_project_types_for_canonicalization(
    project: &HirProject,
    registered: &crate::registration::RegisteredSemanticWorld,
    sources: &CanonicalizationSourceSet,
) -> Result<TypeCheckReport, SemanticDataUnavailable> {
    validate_canonicalization_sources(project, sources)?;
    let module = project.linked_module();
    let (style_catalog, style_diagnostics) = check_view_styles(&module);
    let mut checker = TypeChecker::new_with_project(
        registered.environment().base(),
        Some(sources),
        Some(registered.symbols()),
        Some(registered.environment()),
    );
    Ok(finish_type_check(
        &module,
        style_catalog,
        style_diagnostics,
        &mut checker,
    ))
}

/// Analyzes one linked project while retaining exact-source speaker-line evidence.
pub fn analyze_project_types_for_canonicalization(
    project: &HirProject,
    env: &TypeCheckEnv,
    sources: &CanonicalizationSourceSet,
) -> Result<TypeCheckReport, SemanticDataUnavailable> {
    validate_canonicalization_sources(project, sources)?;
    let document = canonicalization_diagnostic_document(project, sources);
    let root_source = project
        .source(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
        .ok_or_else(|| {
            SemanticDataUnavailable::new(
                document.clone(),
                "HIR project has no root source identity",
            )
        })?;
    let world = arcweft_lang_hir::symbol::ProjectSymbolWorldId::try_new(
        project.package().clone(),
        root_source.id().clone(),
        "canonicalization",
    )
    .map_err(|error| SemanticDataUnavailable::new(document.clone(), error.to_string()))?;
    let revision = arcweft_lang_hir::symbol::ProjectSymbolRevision::try_for_documents(
        project
            .modules()
            .filter_map(|(path, _)| project.source(path)),
    )
    .map_err(|error| SemanticDataUnavailable::new(document.clone(), error.to_string()))?;
    let externals =
        arcweft_lang_hir::symbol::ProjectExternalDeclarations::try_new(world, revision, Vec::new())
            .map_err(|error| SemanticDataUnavailable::new(document.clone(), error.to_string()))?;
    let project_symbols = project
        .project_symbols(&externals)
        .map_err(|report| {
            SemanticDataUnavailable::new(
                document,
                report
                    .diagnostics()
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        })?
        .into_table();
    let module = project.linked_module();
    let (style_catalog, style_diagnostics) = check_view_styles(&module);
    let mut checker =
        TypeChecker::new_with_project(env, Some(sources), Some(&project_symbols), None);
    Ok(finish_type_check(
        &module,
        style_catalog,
        style_diagnostics,
        &mut checker,
    ))
}

fn validate_canonicalization_sources(
    project: &HirProject,
    sources: &CanonicalizationSourceSet,
) -> Result<(), SemanticDataUnavailable> {
    let document = canonicalization_diagnostic_document(project, sources);
    if sources.project() != project.package() {
        return Err(SemanticDataUnavailable::new(
            document,
            format!(
                "source project `{}` does not match checked project `{}`",
                sources.project(),
                project.package()
            ),
        ));
    }
    for (module, _) in project.modules() {
        let Some(expected) = project.source(module) else {
            continue;
        };
        let Some(actual) = sources.source(module) else {
            return Err(SemanticDataUnavailable::new(
                document,
                format!("module `{module}` has no canonicalization source identity"),
            ));
        };
        if actual != expected {
            return Err(SemanticDataUnavailable::new(
                actual.id().clone(),
                format!("module `{module}` canonicalization source is stale"),
            ));
        }
    }
    Ok(())
}

fn canonicalization_diagnostic_document(
    project: &HirProject,
    sources: &CanonicalizationSourceSet,
) -> SourceDocumentId {
    sources
        .first_document()
        .cloned()
        .or_else(|| {
            project
                .source(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
                .map(|identity| identity.id().clone())
        })
        .or_else(|| {
            project.modules().find_map(|(module, _)| {
                project.source(module).map(|identity| identity.id().clone())
            })
        })
        .unwrap_or_else(|| SourceDocumentId::try_new("<project>").expect("non-empty document id"))
}

fn finish_type_check(
    module: &HirModule,
    style_catalog: crate::style::CheckedViewStyleCatalog,
    style_diagnostics: Vec<crate::style::StyleDiagnostic>,
    checker: &mut TypeChecker<'_>,
) -> TypeCheckReport {
    checker
        .errors
        .extend(style_diagnostics.into_iter().map(TypeCheckError::style));
    checker.check_module(module);
    checker.apply_pending_higher_order_effect_calls();
    let effects = std::mem::take(&mut checker.effect_collector).finish();
    checker
        .errors
        .extend(effects.errors().cloned().map(TypeCheckError::effect));
    checker
        .warnings
        .extend(effects.warnings().cloned().map(TypeCheckWarning::effect));
    let canonicalization_inventories = checker.finish_canonicalization_inventories();
    TypeCheckReport {
        diagnostics: std::mem::take(&mut checker.errors),
        warnings: std::mem::take(&mut checker.warnings),
        stats: checker.stats.clone(),
        judgments: std::mem::take(&mut checker.judgments),
        typed_lowering_evidence: std::mem::take(&mut checker.typed_lowering_evidence),
        closure_captures: std::mem::take(&mut checker.closure_captures),
        numeric_fallbacks: std::mem::take(&mut checker.numeric_fallbacks),
        effects,
        for_iteration_evidence: std::mem::take(&mut checker.for_iteration_evidence),
        trait_catalog: std::mem::take(&mut checker.trait_catalog),
        style_catalog,
        canonicalization_inventories,
    }
}

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

        self.collect_and_store_trait_catalog(module);
        self.action_signatures = collect_action_signatures(module, &mut self.errors);
        self.bind_top_level_entity_aliases(module);
        self.bind_top_level_type_aliases(module);
        self.bind_top_level_nominal_fields(module);
        self.bind_dialogue_view_models(module);
        self.bind_top_level_nominal_variant_payloads(module);
        self.bind_extern_capability_functions(module);
        self.register_effect_callables(module);
        self.bind_top_level_functions(module);
        self.flow_params = collect_flow_params(module);
        self.fx = FxCatalog::from_module(module, &mut self.errors);

        self.check_module_agents(module.agents());
        self.with_runtime_for_iteration_evidence(|this| {
            this.check_module_flows(module.flows());
        });
        self.check_module_functions(module.functions());
        for declaration in module.declarations() {
            self.check_top_level_decl(declaration);
        }
        self.locals.clear();
        self.reset_semantic_root_scope(None);
        self.check_flow_items(module.top_level_items());
    }

    fn check_module_agents(&mut self, agents: &[HirAgent]) {
        for agent in agents {
            self.clear_borrow_state();
            self.locals.clear();
            self.reset_semantic_root_scope(agent.module_path());
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
                    "agent `{}` returns {}, but body has {}",
                    item.name(),
                    type_kind_label(&expected),
                    type_kind_label(&actual)
                )));
            }
        }
    }

    fn check_module_flows(&mut self, flows: &[HirFlow]) {
        for flow in flows {
            self.clear_borrow_state();
            self.locals.clear();
            self.reset_semantic_root_scope(flow.module_path());
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
                self.expect_entity_kind(id, &EntityKind::Flow, "flow id");
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
            self.reset_semantic_root_scope(function.module_path());
            self.loop_stack.clear();
            self.yield_stack.clear();
            self.active_presentation_defaults.clear();
            self.warn_public_signature_anonymous_sum(function);
            self.check_signature_type_refs(function.signature());
            let generic_names = signature_generic_names(function.signature());
            self.check_function_parameter_defaults(function, &generic_names);
            let higher_order_param_scope = super::HigherOrderParamScope {
                function_name: function.name().to_owned(),
                callable: function_callable_id(function.name()),
                param_names: function
                    .signature()
                    .param_groups()
                    .iter()
                    .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
                    .flat_map(|param| {
                        let ty = function_param_local_type_with_generics(param, &generic_names);
                        super::function_param_higher_order_bindings(
                            param.pattern(),
                            &ty,
                            NominalTypeContext::new(
                                &self.nominal_fields,
                                &self.nominal_variant_payloads,
                                self.env,
                            ),
                        )
                        .into_iter()
                        .map(|binding| binding.name().to_owned())
                    })
                    .collect::<BTreeSet<_>>(),
            };
            for group in function.signature().param_groups() {
                for param in group.params() {
                    self.bind_function_param(
                        param.pattern(),
                        &function_param_local_type_with_generics(param, &generic_names),
                    );
                }
            }
            let expected_return = function
                .signature()
                .return_type()
                .map(|ty| type_ref_kind_with_generics(ty, &generic_names));
            for contract in function.contracts() {
                self.check_function_contract_clause(contract, expected_return.as_ref());
            }
            let effect_scope = EffectScope::from_contracts(function.contracts());
            let effect_snapshot = self.apply_effect_scope(&effect_scope);
            let previous_callable = self
                .effect_collector
                .enter(function_callable_id(function.name()));
            self.higher_order_param_scope_stack
                .push(higher_order_param_scope);
            let predicates = self
                .trait_catalog
                .predicates_for_signature(function.signature());
            self.trait_predicate_stack.push(predicates);
            if function.kind() == FunctionKind::Stream {
                self.check_stream_function(function);
                self.trait_predicate_stack.pop();
                self.higher_order_param_scope_stack.pop();
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
            self.connect_function_return_effect_callable(function.name(), actual.as_ref());
            self.trait_predicate_stack.pop();
            self.higher_order_param_scope_stack.pop();
            self.effect_collector.restore(previous_callable);
            self.effect_capabilities = effect_snapshot;
            if let (Some(expected), Some(actual)) = (expected_return, actual)
                && !self.types_compatible(&expected, &actual)
            {
                self.errors.push(TypeCheckError::new(format!(
                    "function `{}` returns {}, but body has {}",
                    function.name(),
                    type_kind_label(&expected),
                    type_kind_label(&actual)
                )));
            }
        }
    }

    fn check_function_body_expr(
        &mut self,
        statements: &[Stmt],
        value: Option<&arcweft_lang_syntax::ast::flow::AuthoredExpr>,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        if value.is_some() {
            return self.check_authored_block_expr_with_expected(statements, value, expected);
        }
        match statements.split_last() {
            Some((
                Stmt::Return {
                    expr,
                    expr_source,
                    expr_range,
                },
                statements,
            )) => self.check_tail_return_block_expr_with_expected(
                statements,
                expr,
                expr_source.as_deref(),
                *expr_range,
                expected,
            ),
            _ => self.check_authored_block_expr(statements, None),
        }
    }

    fn check_function_parameter_defaults(
        &mut self,
        function: &HirFunction,
        generic_names: &HashSet<String>,
    ) {
        for param in function
            .signature()
            .param_groups()
            .iter()
            .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
        {
            let Some(default) = param.default() else {
                continue;
            };
            if !function.has_attribute("fx") {
                self.errors.push(TypeCheckError::new(format!(
                    "default parameters are currently reserved for `#[fx]` functions; `{}` must make this argument explicit",
                    function.name()
                )));
            }
            let expected = function_param_local_type_with_generics(param, generic_names);
            self.check_expr_with_expected(default, Some(&expected));
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
        self.expected_returns.push(expected.cloned());
        let result = check(self);
        self.expected_returns.pop();
        result
    }

    fn with_runtime_for_iteration_evidence<R>(&mut self, check: impl FnOnce(&mut Self) -> R) -> R {
        let previous = self.record_runtime_for_iteration_evidence;
        self.record_runtime_for_iteration_evidence = true;
        let result = check(self);
        self.record_runtime_for_iteration_evidence = previous;
        result
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

    fn bind_top_level_nominal_fields(&mut self, module: &HirModule) {
        self.nominal_fields = self.env.nominal_records.clone();
        for declaration in module.declarations() {
            let HirTopLevelDecl::Struct(item) = declaration else {
                continue;
            };
            self.nominal_fields
                .insert(item.name().to_owned(), struct_field_types(item));
        }
    }

    fn bind_dialogue_view_models(&mut self, module: &HirModule) {
        match DialogueViewModelRegistry::from_hir(module) {
            Ok(models) => self.dialogue_view_models = models,
            Err(errors) => self.errors.extend(
                errors
                    .into_iter()
                    .map(|error| TypeCheckError::new(error.to_string())),
            ),
        }
    }

    fn bind_top_level_nominal_variant_payloads(&mut self, module: &HirModule) {
        self.nominal_variant_payloads.clear();
        for declaration in module.declarations() {
            let HirTopLevelDecl::Enum(item) = declaration else {
                continue;
            };
            let payloads = enum_variant_payload_types(item, &mut self.errors);
            self.nominal_variant_payloads
                .insert(item.name().to_owned(), payloads);
        }
    }

    fn bind_extern_capability_functions(&mut self, module: &HirModule) {
        for declaration in module.declarations() {
            let HirTopLevelDecl::ExternCapability(item) = declaration else {
                continue;
            };
            for function in item.functions() {
                self.check_signature_type_refs(function.signature());
                let signature_type = function_signature_type_with_nominal_types(
                    function.signature(),
                    NominalTypeContext::new(
                        &self.nominal_fields,
                        &self.nominal_variant_payloads,
                        self.env,
                    ),
                );
                let name = format!("{}.{}", item.id(), function.signature().name());
                self.global_functions
                    .insert(name.clone(), signature_type.return_type().clone());
                self.global_function_signatures
                    .insert(name.clone(), signature_type);
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
            let signature_type = function_signature_type_with_nominal_types(
                function.signature(),
                NominalTypeContext::new(
                    &self.nominal_fields,
                    &self.nominal_variant_payloads,
                    self.env,
                ),
            );
            let signature_type = if function.kind() == FunctionKind::Function {
                let body_effects = self.function_effect_row(function.name());
                signature_type.with_body_effects(&body_effects)
            } else {
                signature_type
            };
            if let Some(symbols) = self.project_symbols {
                let declaration =
                    CallableDeclarationId::for_function(symbols.world().package(), function)
                        .expect(
                            "linked callable functions must retain canonical module provenance",
                        );
                self.project_functions
                    .insert(declaration.clone(), signature_type.return_type().clone());
                self.project_function_signatures
                    .insert(declaration, signature_type.clone());
            }
            self.global_functions.insert(
                function.name().to_owned(),
                signature_type.return_type().clone(),
            );
            if function.kind() == FunctionKind::Function {
                self.register_function_return_effect_callable(
                    function.name(),
                    signature_type.body_return_type(),
                );
            }
            self.global_function_signatures
                .insert(function.name().to_owned(), signature_type);
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
            let _ = self.register_effect_callable(
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
                let _ = self.register_effect_callable(
                    name,
                    flow_callable_id(name),
                    CallableKind::Flow,
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
                function.has_attribute("pure") || function.has_attribute("fx"),
                &mut self.errors,
            );
            if let Some(effects) = contract.upper_bound() {
                self.global_function_effects
                    .insert(function.name().to_owned(), effects.to_labels());
            }
            let callable = function_callable_id(function.name());
            let registered = self.register_effect_callable(
                function.name(),
                callable.clone(),
                CallableKind::Function,
                effect_visibility_from_syntax(function.visibility()),
                contract,
            );
            if registered && function.kind() == FunctionKind::Function {
                self.ordinary_source_functions
                    .insert(function.name().to_owned());
                self.effect_collector.ensure_inferred_effect_row(&callable);
            }
        }
    }

    fn register_effect_callable(
        &mut self,
        source_name: &str,
        id: CallableId,
        kind: CallableKind,
        visibility: EffectVisibility,
        contract: EffectContract,
    ) -> bool {
        if let Err(error) =
            self.effect_collector
                .register_callable(source_name, id, kind, visibility, contract)
        {
            self.errors.push(TypeCheckError::new(error.to_string()));
            return false;
        }
        true
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive match keeps every HIR top-level declaration family visible at the checker dispatch boundary"
    )]
    pub(super) fn check_top_level_decl(&mut self, declaration: &HirTopLevelDecl) {
        match declaration {
            HirTopLevelDecl::DialogueDefaults(item) => self.check_dialogue_defaults(item),
            HirTopLevelDecl::Enum(_)
            | HirTopLevelDecl::Proof(_)
            | HirTopLevelDecl::Struct(_)
            | HirTopLevelDecl::Trait(_)
            | HirTopLevelDecl::TrustedAxiom(_)
            | HirTopLevelDecl::ExternCapability(_) => {}
            HirTopLevelDecl::Impl(item) => self.check_impl_item(item),
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
                if item.kind() == EntityDeclKind::View
                    && item.id().body() == STANDARD_DIALOGUE_VIEW_RESOURCE
                {
                    self.errors.push(TypeCheckError::new(format!(
                        "View resource `{STANDARD_DIALOGUE_VIEW_RESOURCE}` is reserved by the standard dialogue runtime"
                    )));
                }
                self.expect_entity_kind(
                    item.id(),
                    &entity_kind_for_decl(item.kind()),
                    "entity declaration id",
                );
                self.check_view_action_invokes(item);
                self.check_view_fx_applications(item);
                self.check_view_dialogue_text_sources(item);
            }
            HirTopLevelDecl::Callable(item) => {
                self.clear_borrow_state();
                self.locals.clear();
                self.reset_semantic_root_scope(None);
                self.loop_stack.clear();
                for contract in item.contracts() {
                    self.check_contract_clause(contract);
                }
            }
            HirTopLevelDecl::State(item) => {
                self.clear_borrow_state();
                self.locals.clear();
                self.reset_semantic_root_scope(None);
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
                self.reset_semantic_root_scope(None);
                self.loop_stack.clear();
                self.yield_stack.clear();
                self.check_block_expr(item.body_statements(), item.body_value());
            }
            HirTopLevelDecl::Parser(item) => {
                self.clear_borrow_state();
                self.locals.clear();
                self.reset_semantic_root_scope(None);
                self.loop_stack.clear();
                self.yield_stack.clear();
                self.check_block_expr(item.body_statements(), item.body_value());
            }
            HirTopLevelDecl::Source(source) => {
                let item = source.item();
                self.clear_borrow_state();
                self.locals.clear();
                self.reset_semantic_root_scope(None);
                self.loop_stack.clear();
                self.yield_stack.clear();
                if let Some(id) = item.id() {
                    self.expect_entity_kind(id, &EntityKind::Source, "source id");
                }
                self.check_source_item(item);
            }
            HirTopLevelDecl::Style(item) => self.check_style_decl(item),
        }
    }

    fn check_style_decl(&mut self, item: &HirStyleDecl) {
        self.expect_entity_kind(item.id(), &EntityKind::Style, "style id");
    }

    fn check_dialogue_defaults(
        &mut self,
        item: &arcweft_lang_syntax::ast::dialogue::DialogueDefaultsItem,
    ) {
        for assignment in item
            .assignments()
            .iter()
            .filter(|assignment| assignment.path().dotted() == "view")
        {
            if !matches!(
                assignment.value(),
                Expr::EntityRef(reference)
                    if entity_syntax_kind(reference) == Some(EntityKind::View)
            ) {
                self.errors.push(TypeCheckError::new(
                    "dialogue defaults `view` must be a typed View reference".to_owned(),
                ));
            }
        }
    }

    fn check_view_dialogue_text_sources(&mut self, item: &EntityDeclItem) {
        let Some(view) = item.view_body().and_then(|body| body.view()) else {
            return;
        };
        let signature = match parse_fn_signature(&format!("fn view{}", item.signature_tail())) {
            Ok(signature) => signature,
            Err(error) => {
                self.errors.push(TypeCheckError::new(format!(
                    "View `{}` has an invalid parameter signature: {error}",
                    item.id().body()
                )));
                return;
            }
        };
        let parameters = signature
            .param_groups()
            .iter()
            .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
            .filter_map(|parameter| {
                let name = parameter.pattern().simple_binding_name()?;
                let TypeRef::Path(type_name) = parameter.ty() else {
                    return None;
                };
                Some((name, type_name.as_str()))
            })
            .collect::<HashMap<_, _>>();

        for text in view.text_nodes() {
            let Some(label) = text.source().dotted_selector_label() else {
                continue;
            };
            let Some((parameter, field)) = label.split_once('.') else {
                continue;
            };
            let Some(type_name) = parameters.get(parameter) else {
                continue;
            };
            let Some(model) = self.dialogue_view_models.model(type_name) else {
                continue;
            };
            let Some(projection) = model.projection(field) else {
                self.errors.push(TypeCheckError::new(format!(
                    "dialogue View parameter `{parameter}` has no runtime projection `{field}`"
                )));
                continue;
            };
            let rich = text.rich_surface().is_some();
            match projection {
                DialogueViewProjection::Content if !rich => {
                    self.errors.push(TypeCheckError::new(format!(
                        "dialogue content projection `{label}` must be emitted by `RichText(...)`"
                    )));
                }
                DialogueViewProjection::Speaker if rich => {
                    self.errors.push(TypeCheckError::new(format!(
                        "dialogue speaker projection `{label}` must be emitted by `Text(...)`"
                    )));
                }
                DialogueViewProjection::Speaker | DialogueViewProjection::Content => {}
                DialogueViewProjection::Occurrence
                | DialogueViewProjection::Stage
                | DialogueViewProjection::Reveal
                | DialogueViewProjection::PrimaryAction => {
                    self.errors.push(TypeCheckError::new(format!(
                        "dialogue lifecycle projection `{label}` is not text content"
                    )));
                }
            }
        }

        for action in view.action_projections() {
            let Some(label) = action.dotted_selector_label() else {
                continue;
            };
            let Some((parameter, field)) = label.split_once('.') else {
                continue;
            };
            let Some(type_name) = parameters.get(parameter) else {
                self.errors.push(TypeCheckError::new(format!(
                    "View action projection `{label}` has no matching parameter"
                )));
                continue;
            };
            let Some(model) = self.dialogue_view_models.model(type_name) else {
                self.errors.push(TypeCheckError::new(format!(
                    "View action projection `{label}` does not come from a dialogue View model"
                )));
                continue;
            };
            if model.projection(field) != Some(DialogueViewProjection::PrimaryAction) {
                self.errors.push(TypeCheckError::new(format!(
                    "View action projection `{label}` must select `primary_action`"
                )));
            }
        }
    }

    fn check_view_action_invokes(&mut self, item: &EntityDeclItem) {
        let Some(view) = item.view_body().and_then(|body| body.view()) else {
            return;
        };
        for action in view.action_invokes() {
            self.check_view_action_invoke(&action);
        }
    }

    fn check_view_fx_applications(&mut self, item: &EntityDeclItem) {
        let Some(view) = item.view_body().and_then(|body| body.view()) else {
            return;
        };
        for application in view.fx_applications() {
            self.fx
                .validate_view_application(application, &mut self.errors);
        }
    }

    fn check_view_action_invoke(&mut self, action: &ViewActionInvokeAction) {
        if entity_syntax_kind(action.action()) != Some(EntityKind::Action) {
            self.errors.push(TypeCheckError::new(format!(
                "action.invoke target `{}` must be an Action reference",
                action.action().canonical_body()
            )));
            return;
        }

        let action_id = action.action().canonical_body();
        let Some(signature) = self.action_signatures.get(&action_id).cloned() else {
            self.errors.push(TypeCheckError::new(format!(
                "action.invoke target `{action_id}` is not declared"
            )));
            return;
        };

        self.check_action_invoke_payload(&action_id, &signature, action);
    }

    fn check_action_invoke_payload(
        &mut self,
        action_id: &str,
        signature: &ActionSignature,
        action: &ViewActionInvokeAction,
    ) {
        let Some(payload) = action.payload() else {
            for param in signature
                .params()
                .iter()
                .filter(|param| !param.has_default())
            {
                self.errors.push(TypeCheckError::new(format!(
                    "action.invoke for `{action_id}` is missing payload `{}`",
                    action_param_label(param)
                )));
            }
            return;
        };

        let Some(payload_name) = action.payload_name() else {
            self.errors.push(TypeCheckError::new(format!(
                "action.invoke for `{action_id}` must name its payload"
            )));
            return;
        };

        let Some(param) = signature.param(payload_name) else {
            self.errors.push(TypeCheckError::new(format!(
                "action `{action_id}` does not declare payload `{payload_name}`"
            )));
            return;
        };

        let actual = action_payload_type(payload);
        if !self.types_compatible(param.ty(), &actual) {
            self.errors.push(TypeCheckError::new(format!(
                "action.invoke payload `{payload_name}` for `{action_id}` expects {}, but View payload has {}",
                type_kind_label(param.ty()),
                type_kind_label(&actual)
            )));
        }

        for missing in signature
            .params()
            .iter()
            .filter(|param| !param.has_default() && param.name() != payload_name)
        {
            self.errors.push(TypeCheckError::new(format!(
                "action.invoke for `{action_id}` is missing payload `{}`",
                action_param_label(missing)
            )));
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

    fn check_impl_item(&mut self, item: &ImplItem) {
        let self_ty = impl_target_type(item);
        let generic_names = impl_generic_names(item.generics());
        for member in item.members() {
            let ImplMember::Function {
                signature,
                body_statements,
                body_value,
                ..
            } = member
            else {
                continue;
            };
            self.clear_borrow_state();
            self.locals.clear();
            self.reset_semantic_root_scope(None);
            self.loop_stack.clear();
            self.yield_stack.clear();
            self.check_signature_type_refs(signature);
            for group in signature.param_groups() {
                for param in group.params() {
                    self.bind_impl_method_param(param, &self_ty, &generic_names);
                }
            }
            let expected_return = signature
                .return_type()
                .map(|ty| type_ref_kind_for_impl(ty, &self_ty, &generic_names));
            let predicates = self.trait_catalog.predicates_for_signature(signature);
            self.trait_predicate_stack.push(predicates);
            let actual = self.with_expected_return(expected_return.as_ref(), |this| {
                this.check_function_body_expr(
                    body_statements,
                    body_value.as_deref(),
                    expected_return.as_ref(),
                )
            });
            self.trait_predicate_stack.pop();
            if let (Some(expected), Some(actual)) = (expected_return, actual)
                && !self.types_compatible(&expected, &actual)
            {
                self.errors.push(TypeCheckError::new(format!(
                    "impl method `{}` returns {}, but body has {}",
                    signature.name(),
                    type_kind_label(&expected),
                    type_kind_label(&actual)
                )));
            }
        }
        self.clear_borrow_state();
        self.locals.clear();
        self.reset_semantic_root_scope(None);
        self.loop_stack.clear();
        self.yield_stack.clear();
        self.active_presentation_defaults.clear();
    }

    fn bind_impl_method_param(
        &mut self,
        param: &FnParam,
        self_ty: &TypeKind,
        generic_names: &HashSet<String>,
    ) {
        let Some(name) = ident_pattern_name(param.pattern()) else {
            return;
        };
        let ty = if name == "self" {
            self_ty.clone()
        } else {
            let ty = type_ref_kind_for_impl(param.ty(), self_ty, generic_names);
            if param.is_rest() {
                TypeKind::Vec(Box::new(ty))
            } else {
                ty
            }
        };
        self.bind_function_param(param.pattern(), &ty);
    }

    fn check_entry_item(&mut self, item: &EntryItem) {
        match item {
            EntryItem::Goto(target) => {
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
            self.check_authored_block_expr(function.statements(), function.value());
            return;
        };
        self.yield_stack.push(YieldContext::Stream {
            item_ty,
            error_ty,
            yield_count: 0,
        });
        self.check_block_expr(function.statements(), None);
        let value_is_stream_block = matches!(
            function.value().map(AuthoredExpr::expr),
            Some(Expr::ComputationBlock {
                kind: ComputationBlockKind::Stream,
                ..
            })
        );
        if let Some(value) = function.value() {
            self.check_authored_expr(value);
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
        self.record_checked_speaker_line(dialogue);
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
        if let Some(view) = dialogue.view() {
            self.expect_entity_kind(view, &EntityKind::View, "dialogue View");
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
            dialogue.content(),
            dialogue_has_default_inline_failure_policy(dialogue),
            DialogueContentRangeMode::ContentSourceMap,
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
            TypeRef::Tuple(items) => {
                for item in items {
                    self.check_type_ref_shape(item);
                }
            }
            TypeRef::Function {
                params,
                return_type,
                effects,
            } => {
                for param in params {
                    self.check_type_ref_shape(param);
                }
                self.check_type_ref_shape(return_type);
                if let Some(effects) = effects
                    && let Err(error) = EffectSet::from_labels(effects.effects())
                {
                    self.errors.push(TypeCheckError::new(format!(
                        "invalid function type effect row: {error}"
                    )));
                }
            }
            TypeRef::Generic { args, .. } => {
                for arg in args {
                    self.check_type_ref_shape(arg);
                }
            }
            TypeRef::TraitBound(bound) => {
                for arg in bound.args() {
                    self.check_type_ref_shape(arg);
                }
                for binding in bound.assoc_bindings() {
                    self.check_type_ref_shape(binding.value());
                }
            }
            TypeRef::Projection { subject, .. } => self.check_type_ref_shape(subject),
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
            TypeKind::Function {
                params,
                return_type,
                effects,
            } => {
                let params = params
                    .iter()
                    .map(|param| self.erase_aliases_with_seen(param, seen))
                    .collect::<Vec<_>>();
                let return_type = self.erase_aliases_with_seen(return_type, seen);
                TypeKind::function_with_effects(params, return_type, effects.clone())
            }
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
        Expr::ShortVariant(name) => unknown_default_inline_failure_atom(&format!(".{name}")),
        Expr::Select(select) => match select.target() {
            Expr::Path(namespace) => {
                unknown_default_inline_failure_field(namespace.as_label(), select.member().as_str())
            }
            _ => None,
        },
        Expr::Call { callee, args } => unknown_default_inline_failure_constructor(callee, args),
        _ => None,
    }
}

fn unknown_default_inline_failure_constructor(
    callee: &Expr,
    args: &[arcweft_lang_syntax::expr::CallArg],
) -> Option<String> {
    let constructor = match callee {
        Expr::Path(path) if path == "fallback" => "fallback",
        Expr::Select(select) if matches!(select.target(), Expr::Path(namespace) if namespace == "InlineFailure") => {
            select.member().as_str()
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

fn unknown_default_inline_fallback_value(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => unknown_default_inline_fallback_atom(path),
        Expr::ShortVariant(name) => unknown_default_inline_fallback_atom(&format!(".{name}")),
        Expr::Select(select) => match select.target() {
            Expr::Path(namespace) => unknown_default_inline_fallback_field(
                namespace.as_label(),
                select.member().as_str(),
            ),
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
        Expr::Path(path) => path.as_label().to_owned(),
        Expr::ShortVariant(name) => format!(".{name}"),
        Expr::Select(select) => format!(
            "{}.{}",
            default_inline_policy_label(select.target()),
            select.member().as_str()
        ),
        _ => format!("{expr:?}"),
    }
}

fn type_ref_contains_choice(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Choice(_) => true,
        TypeRef::Tuple(items) => items.iter().any(type_ref_contains_choice),
        TypeRef::Function {
            params,
            return_type,
            ..
        } => params.iter().any(type_ref_contains_choice) || type_ref_contains_choice(return_type),
        TypeRef::Generic { args, .. } => args.iter().any(type_ref_contains_choice),
        TypeRef::TraitBound(bound) => {
            bound.args().iter().any(type_ref_contains_choice)
                || bound
                    .assoc_bindings()
                    .iter()
                    .any(|binding| type_ref_contains_choice(binding.value()))
        }
        TypeRef::Projection { subject, .. } => type_ref_contains_choice(subject),
        TypeRef::Ref { inner, .. } | TypeRef::Slice(inner) => type_ref_contains_choice(inner),
        TypeRef::Never | TypeRef::ConstInt(_) | TypeRef::Path(_) => false,
    }
}

fn collect_action_signatures(
    module: &HirModule,
    errors: &mut Vec<TypeCheckError>,
) -> HashMap<String, ActionSignature> {
    module
        .declarations()
        .iter()
        .filter_map(|declaration| match declaration {
            HirTopLevelDecl::EntityDecl(item) if item.kind() == EntityDeclKind::Action => {
                Some(item)
            }
            _ => None,
        })
        .filter_map(|item| match action_signature_from_decl(item) {
            Ok(signature) => Some((item.id().body().to_owned(), signature)),
            Err(message) => {
                errors.push(TypeCheckError::new(format!(
                    "invalid action signature for `{}`: {message}",
                    item.id().body()
                )));
                None
            }
        })
        .collect()
}

fn action_signature_from_decl(item: &EntityDeclItem) -> Result<ActionSignature, String> {
    let signature_tail = item.signature_tail().trim();
    if signature_tail.is_empty() {
        return Ok(ActionSignature::new([]));
    }

    let signature = parse_fn_signature(&format!("fn action{signature_tail}"))
        .map_err(|error| error.to_string())?;
    if signature.return_type().is_some() {
        return Err("action declarations do not return values".to_owned());
    }

    let params = signature
        .param_groups()
        .iter()
        .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
        .map(action_param_from_fn_param)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ActionSignature::new(params))
}

fn action_param_from_fn_param(param: &FnParam) -> Result<ActionParam, String> {
    if param.is_rest() {
        return Err("action payload parameters cannot be rest parameters".to_owned());
    }
    if param.receiver_kind().is_some() {
        return Err("action payload parameters cannot include a receiver".to_owned());
    }
    let Some(name) = ident_pattern_name(param.pattern()) else {
        return Err("action payload parameters must use identifier patterns".to_owned());
    };
    Ok(ActionParam::new(
        name,
        type_ref_kind(param.ty()),
        param.default().is_some(),
    ))
}

fn action_payload_type(payload: &ViewActionPayload) -> TypeKind {
    match payload {
        ViewActionPayload::LiteralString(_) | ViewActionPayload::TextControlProjection { .. } => {
            TypeKind::String
        }
    }
}

fn action_param_label(param: &ActionParam) -> String {
    format!("{}: {}", param.name(), type_kind_label(param.ty()))
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
        EntryItem::Goto(target) | EntryItem::Route { target, .. } => Some(target),
        EntryItem::Option { .. } | EntryItem::Raw(_) => None,
    }
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

fn struct_field_types(item: &StructItem) -> HashMap<String, TypeKind> {
    item.fields()
        .iter()
        .map(|field| (field.name().to_owned(), type_ref_kind(field.ty())))
        .collect()
}

fn enum_variant_payload_types(
    item: &EnumItem,
    errors: &mut Vec<TypeCheckError>,
) -> HashMap<String, EnumVariantPayload> {
    item.variants()
        .iter()
        .map(|variant| {
            let payload = enum_variant_payload_type(item.name(), variant, errors);
            (variant.name().to_owned(), payload)
        })
        .collect()
}

fn enum_variant_payload_type(
    enum_name: &str,
    variant: &EnumVariant,
    errors: &mut Vec<TypeCheckError>,
) -> EnumVariantPayload {
    let Some(payload) = variant.payload() else {
        return EnumVariantPayload::Unit;
    };
    parse_enum_variant_payload(payload).unwrap_or_else(|message| {
        errors.push(TypeCheckError::new(format!(
            "enum `{enum_name}` variant `{}` has invalid payload type: {message}",
            variant.name()
        )));
        EnumVariantPayload::Unit
    })
}

fn parse_enum_variant_payload(payload: &str) -> Result<EnumVariantPayload, String> {
    let payload = payload.trim();
    if payload.is_empty() {
        return Ok(EnumVariantPayload::Unit);
    }
    if let Some(record) = payload
        .strip_prefix('{')
        .and_then(|inner| inner.strip_suffix('}'))
    {
        return parse_enum_variant_record_payload(record);
    }
    parse_type_ref(payload)
        .map(enum_variant_tuple_payload_from_type_ref)
        .map_err(|error| error.to_string())
}

fn enum_variant_tuple_payload_from_type_ref(ty: TypeRef) -> EnumVariantPayload {
    match ty {
        TypeRef::Tuple(items) => {
            EnumVariantPayload::Tuple(items.iter().map(type_ref_kind).collect())
        }
        ty => EnumVariantPayload::Tuple(vec![type_ref_kind(&ty)]),
    }
}

fn parse_enum_variant_record_payload(record: &str) -> Result<EnumVariantPayload, String> {
    let mut fields = BTreeMap::new();
    for field in split_top_level_commas(record) {
        let field = field.trim();
        if field.is_empty() {
            continue;
        }
        let (name, ty) = split_top_level_colon(field)
            .ok_or_else(|| format!("record payload field `{field}` must use `name: Type`"))?;
        let ty = parse_type_ref(ty.trim()).map_err(|error| error.to_string())?;
        fields.insert(name.trim().to_owned(), type_ref_kind(&ty));
    }
    Ok(EnumVariantPayload::Record(fields))
}

fn split_top_level_commas(source: &str) -> Vec<&str> {
    split_top_level_char(source, ',')
}

fn split_top_level_colon(source: &str) -> Option<(&str, &str)> {
    let index = top_level_char_index(source, ':')?;
    Some((&source[..index], &source[index + ':'.len_utf8()..]))
}

fn split_top_level_char(source: &str, delimiter: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = DelimiterDepth::default();
    for (index, ch) in source.char_indices() {
        if ch == delimiter && depth.is_top_level() {
            parts.push(&source[start..index]);
            start = index + ch.len_utf8();
            continue;
        }
        depth.update(ch);
    }
    parts.push(&source[start..]);
    parts
}

fn top_level_char_index(source: &str, delimiter: char) -> Option<usize> {
    let mut depth = DelimiterDepth::default();
    source.char_indices().find_map(|(index, ch)| {
        if ch == delimiter && depth.is_top_level() {
            return Some(index);
        }
        depth.update(ch);
        None
    })
}

#[derive(Clone, Copy, Debug, Default)]
struct DelimiterDepth {
    paren: i32,
    brace: i32,
    bracket: i32,
    angle: i32,
}

impl DelimiterDepth {
    fn update(&mut self, ch: char) {
        match ch {
            '(' => self.paren += 1,
            ')' => self.paren -= 1,
            '{' => self.brace += 1,
            '}' => self.brace -= 1,
            '[' => self.bracket += 1,
            ']' => self.bracket -= 1,
            '<' => self.angle += 1,
            '>' if self.angle > 0 => self.angle -= 1,
            _ => {}
        }
    }

    const fn is_top_level(self) -> bool {
        self.paren == 0 && self.brace == 0 && self.bracket == 0 && self.angle == 0
    }
}

fn impl_target_type(item: &ImplItem) -> TypeKind {
    parse_type_ref(item.target()).map_or_else(
        |_| TypeKind::Named(item.target().to_owned()),
        |ty| type_ref_kind_for_impl(&ty, &TypeKind::Named("Self".to_owned()), &HashSet::new()),
    )
}

fn impl_generic_names(generics: Option<&str>) -> HashSet<String> {
    generics
        .into_iter()
        .flat_map(|source| source.split(','))
        .filter_map(|item| {
            let name = item
                .trim()
                .trim_start_matches('<')
                .trim_end_matches('>')
                .split(':')
                .next()
                .unwrap_or_default()
                .trim();
            (!name.is_empty()).then_some(name.to_owned())
        })
        .collect()
}

fn type_ref_kind_for_impl(
    ty: &TypeRef,
    self_ty: &TypeKind,
    generic_names: &HashSet<String>,
) -> TypeKind {
    match ty {
        TypeRef::Path(path) if path == "Self" => self_ty.clone(),
        TypeRef::Generic { base, args } if base == "Option" && args.len() == 1 => TypeKind::Option(
            Box::new(type_ref_kind_for_impl(&args[0], self_ty, generic_names)),
        ),
        TypeRef::Generic { base, args } if base == "Vec" && args.len() == 1 => TypeKind::Vec(
            Box::new(type_ref_kind_for_impl(&args[0], self_ty, generic_names)),
        ),
        TypeRef::Generic { base, args } if base == "Result" && args.len() == 2 => {
            TypeKind::Result {
                ok: Box::new(type_ref_kind_for_impl(&args[0], self_ty, generic_names)),
                error: Box::new(type_ref_kind_for_impl(&args[1], self_ty, generic_names)),
            }
        }
        TypeRef::Ref { lifetime, inner } => TypeKind::BorrowRef {
            lifetime: lifetime
                .as_ref()
                .map(|lifetime| LifetimeScopeKind::parse(lifetime.name())),
            inner: Box::new(type_ref_kind_for_impl(inner, self_ty, generic_names)),
        },
        _ => type_ref_kind_with_generics(ty, generic_names),
    }
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
                || type_kind_label(param.ty()),
                |name| format!("{name}: {}", type_kind_label(param.ty())),
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "fn({params}) -> {}",
        type_kind_label(signature.return_type())
    )
}
