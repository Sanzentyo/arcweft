//! Module, top-level declaration, and dialogue entry checks.

mod view;

pub(crate) use super::call_target_facts::SignatureFocusedAnalysis;
use super::call_target_facts::{CallResolverControl, CallTargetFactRecorder, CallTargetFactReport};
use super::line_plan::DialogueContentRangeMode;
use super::{
    CallableExecutionMode, CheckedCallableExecution, EffectScope, EntityKind, EnumVariantPayload,
    FunctionKind, FxCatalog, HirModule, HirTopLevelDecl, LifetimeKey, LifetimeScopeKind,
    NominalTypeContext, Pattern, Stmt, StreamGeneratorFacts, TypeCheckEnv, TypeCheckError,
    TypeCheckReport, TypeCheckWarning, TypeChecker, TypeExpressionId, TypeKind,
    TypedLoweringEvidenceKind, YieldContext, choice_output_type, entity_kind_for_decl,
    function_callable_id, function_param_local_type, function_param_local_type_with_generics,
    function_signature_type, function_signature_type_with_nominal_types, ident_pattern_name,
    normalize_choice_type, signature_generic_names, stream_return_types, type_ref_kind,
    type_ref_kind_with_generics, validate_typecheck_ready,
};
#[cfg(test)]
use crate::callable::ResolverWork;
use crate::callable::{CallTargetFactError, CallTargetFactMode, CallTargetFacts};
use crate::canonicalization::{
    CanonicalizationSourceSet, CheckedCanonicalizationInventory, SemanticDataUnavailable,
};
use crate::checker::helpers::{type_kind_label, type_ref_label};
use crate::dialogue_view::{DialogueViewModelRegistry, STANDARD_DIALOGUE_VIEW_RESOURCE};
use crate::effect_model::{
    CallableId, CallableKind, EffectContract, Visibility as EffectVisibility,
};
use crate::effects::EffectSet;
use crate::style::check_view_styles;
use crate::view_part::{ViewPartDiagnostic, check_view_parts};
use arcweft_lang_hir::entry::HirEntryItem;
use arcweft_lang_hir::model::{HirFlow, HirFunction};
use arcweft_lang_hir::project::HirProject;
use arcweft_lang_hir::style::HirStyleDecl;
use arcweft_lang_hir::symbol::CallableDeclarationId;
use arcweft_lang_syntax::ast::common::Visibility;
use arcweft_lang_syntax::ast::flow::AuthoredExpr;
use arcweft_lang_syntax::ast::items::{
    EntityDeclItem, EntityDeclKind, EntryRouteBinding, EntryRouteBindingSource, EnumItem,
    EnumVariant, ExternModItem, ExternModMember, ExternModSource, ImplItem, ImplMember, StructItem,
    TypeAliasItem,
};
use arcweft_lang_syntax::expr::{ComputationBlockKind, Expr};
use arcweft_lang_syntax::types::{FnParam, FnSignature, GenericParam, TypeRef};
use arcweft_source::SourceDocumentId;
use arcweft_source::SourceDocumentIdentity;
use std::collections::{BTreeSet, HashMap, HashSet};
#[cfg(test)]
use std::sync::atomic::AtomicBool;

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

    /// Returns the checked invocation behavior for one canonical declaration.
    pub fn callable_execution(
        &self,
        declaration: &CallableDeclarationId,
    ) -> Option<&CheckedCallableExecution> {
        self.callable_executions
            .iter()
            .find(|fact| fact.declaration() == declaration)
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

    /// Returns committed facts for one checker expression identity.
    ///
    /// Accepted registered-project analysis records ordinary call surfaces. A
    /// report produced with fact recording disabled returns `Ok(None)`.
    pub fn call_target_facts(
        &self,
        expression: TypeExpressionId,
    ) -> Result<Option<&CallTargetFacts>, CallTargetFactError> {
        if let Some(error) = &self.call_target_fact_report.error {
            return Err(error.clone());
        }
        Ok(self.call_target_fact_report.facts.get(&expression))
    }

    /// Returns the sole committed call fact from focused semantic analysis.
    pub fn focused_call_target_facts(&self) -> Result<&CallTargetFacts, CallTargetFactError> {
        let CallTargetFactMode::Focused { call, .. } = &self.call_target_fact_report.mode else {
            return Err(CallTargetFactError::FocusedModeRequired);
        };
        if let Some(error) = &self.call_target_fact_report.error {
            return Err(error.clone());
        }
        self.call_target_fact_report
            .facts
            .values()
            .next()
            .ok_or_else(|| CallTargetFactError::FocusedTargetMissing { call: call.clone() })
    }

    pub(crate) fn retained_call_target_facts(&self) -> impl Iterator<Item = &CallTargetFacts> {
        self.call_target_fact_report.facts.values()
    }
}

pub(crate) struct FocusedCallTypeCheckReport {
    #[cfg(test)]
    report: TypeCheckReport,
    call_targets: CallTargetFactReport,
}

impl FocusedCallTypeCheckReport {
    #[cfg(test)]
    pub(crate) const fn report(&self) -> &TypeCheckReport {
        &self.report
    }

    pub(crate) fn focused_call_target_facts(
        &self,
    ) -> Result<&CallTargetFacts, CallTargetFactError> {
        let CallTargetFactMode::Focused { call, .. } = &self.call_targets.mode else {
            return Err(CallTargetFactError::FocusedModeRequired);
        };
        if let Some(error) = &self.call_targets.error {
            return Err(error.clone());
        }
        self.call_targets
            .facts
            .values()
            .next()
            .ok_or_else(|| CallTargetFactError::FocusedTargetMissing { call: call.clone() })
    }
}

/// Analyzes lowered HIR with an explicit symbol/method environment.
pub fn analyze_types(module: &HirModule, env: &TypeCheckEnv) -> TypeCheckReport {
    let (style_catalog, style_diagnostics) = check_view_styles(module);
    let (view_part_catalog, view_part_diagnostics) = check_view_parts(module);
    let mut checker = TypeChecker::new(env, module);
    finish_type_check(
        module,
        style_catalog,
        style_diagnostics,
        view_part_catalog,
        view_part_diagnostics,
        &mut checker,
    )
}

/// Analyzes linked project HIR through the sole registered semantic boundary.
pub fn analyze_registered_project_types(
    module: &HirModule,
    registered: &crate::registration::RegisteredSemanticWorld,
) -> TypeCheckReport {
    let (style_catalog, style_diagnostics) = check_view_styles(module);
    let (view_part_catalog, view_part_diagnostics) = check_view_parts(module);
    let mut checker = TypeChecker::new_with_project(
        registered.environment().typecheck_env(),
        module,
        None,
        Some(registered.symbols()),
        Some(registered),
        CallTargetFactMode::All,
        CallResolverControl::ordinary(),
    );
    finish_type_check(
        module,
        style_catalog,
        style_diagnostics,
        view_part_catalog,
        view_part_diagnostics,
        &mut checker,
    )
}

/// Analyzes one exact accepted call span and retains only its checked call facts.
///
/// This bounded public entry uses the production callable-work limit and a
/// non-cancelled checker control. Interactive signature queries retain their
/// separate caller-owned cancellation and accounting path.
pub fn analyze_registered_project_types_for_focused_call(
    module: &HirModule,
    registered: &crate::registration::RegisteredSemanticWorld,
    call: arcweft_source::SourceSpan,
) -> Result<TypeCheckReport, CallTargetFactError> {
    if !registered
        .symbols()
        .modules()
        .any(|module| registered.symbols().source_identity(module) == Some(call.source()))
    {
        return Err(CallTargetFactError::FocusedSourceUnavailable {
            document: call.source().clone(),
        });
    }
    let (style_catalog, style_diagnostics) = check_view_styles(module);
    let (view_part_catalog, view_part_diagnostics) = check_view_parts(module);
    let mut checker = TypeChecker::new_with_project(
        registered.environment().typecheck_env(),
        module,
        None,
        Some(registered.symbols()),
        Some(registered),
        CallTargetFactMode::Focused {
            call,
            active_argument: None,
            byte_offset: None,
        },
        CallResolverControl::ordinary(),
    );
    let report = finish_type_check(
        module,
        style_catalog,
        style_diagnostics,
        view_part_catalog,
        view_part_diagnostics,
        &mut checker,
    );
    report.focused_call_target_facts()?;
    Ok(report)
}

#[cfg(test)]
pub(crate) fn analyze_registered_project_types_for_call_facts(
    module: &HirModule,
    registered: &crate::registration::RegisteredSemanticWorld,
    call: arcweft_source::SourceSpan,
    cancellation: &AtomicBool,
    work: &mut ResolverWork,
) -> Result<FocusedCallTypeCheckReport, CallTargetFactError> {
    if !registered
        .symbols()
        .modules()
        .any(|module| registered.symbols().source_identity(module) == Some(call.source()))
    {
        return Err(CallTargetFactError::FocusedSourceUnavailable {
            document: call.source().clone(),
        });
    }
    let (style_catalog, style_diagnostics) = check_view_styles(module);
    let (view_part_catalog, view_part_diagnostics) = check_view_parts(module);
    let mut checker = TypeChecker::new_with_project(
        registered.environment().typecheck_env(),
        module,
        None,
        Some(registered.symbols()),
        Some(registered),
        CallTargetFactMode::Focused {
            call,
            active_argument: None,
            byte_offset: None,
        },
        CallResolverControl::caller_owned(cancellation, work, None, None),
    );
    let (report, call_targets) = finish_type_check_with_call_facts(
        module,
        style_catalog,
        style_diagnostics,
        view_part_catalog,
        view_part_diagnostics,
        &mut checker,
    );
    Ok(FocusedCallTypeCheckReport {
        report,
        call_targets,
    })
}

pub(crate) fn analyze_registered_project_types_for_signature_call(
    analysis: SignatureFocusedAnalysis<'_>,
) -> Result<FocusedCallTypeCheckReport, CallTargetFactError> {
    let SignatureFocusedAnalysis {
        module,
        registered,
        site,
        cancellation,
        work,
        signature_work,
        signature_control,
    } = analysis;
    if !registered
        .symbols()
        .modules()
        .any(|module| registered.symbols().source_identity(module) == Some(site.call().source()))
    {
        return Err(CallTargetFactError::FocusedSourceUnavailable {
            document: site.call().source().clone(),
        });
    }
    let (style_catalog, style_diagnostics) = check_view_styles(module);
    let (view_part_catalog, view_part_diagnostics) = check_view_parts(module);
    let mut checker = TypeChecker::new_with_project(
        registered.environment().typecheck_env(),
        module,
        None,
        Some(registered.symbols()),
        Some(registered),
        CallTargetFactMode::Focused {
            call: site.call().clone(),
            active_argument: site.active_argument(),
            byte_offset: site.byte_offset(),
        },
        CallResolverControl::caller_owned(
            cancellation,
            work,
            Some(signature_work),
            Some(signature_control),
        ),
    );
    let (report, call_targets) = finish_type_check_with_call_facts(
        module,
        style_catalog,
        style_diagnostics,
        view_part_catalog,
        view_part_diagnostics,
        &mut checker,
    );
    #[cfg(not(test))]
    drop(report);
    Ok(FocusedCallTypeCheckReport {
        #[cfg(test)]
        report,
        call_targets,
    })
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
    let (view_part_catalog, view_part_diagnostics) = check_view_parts(&module);
    let mut checker = TypeChecker::new_with_project(
        registered.environment().typecheck_env(),
        &module,
        Some(sources),
        Some(registered.symbols()),
        Some(registered),
        CallTargetFactMode::All,
        CallResolverControl::ordinary(),
    );
    Ok(finish_type_check(
        &module,
        style_catalog,
        style_diagnostics,
        view_part_catalog,
        view_part_diagnostics,
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
    let (view_part_catalog, view_part_diagnostics) = check_view_parts(&module);
    let mut checker = TypeChecker::new_with_project(
        env,
        &module,
        Some(sources),
        Some(&project_symbols),
        None,
        CallTargetFactMode::Disabled,
        CallResolverControl::ordinary(),
    );
    Ok(finish_type_check(
        &module,
        style_catalog,
        style_diagnostics,
        view_part_catalog,
        view_part_diagnostics,
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
    view_part_catalog: crate::view_part::CheckedViewPartCatalog,
    view_part_diagnostics: Vec<ViewPartDiagnostic>,
    checker: &mut TypeChecker<'_>,
) -> TypeCheckReport {
    finish_type_check_with_call_facts(
        module,
        style_catalog,
        style_diagnostics,
        view_part_catalog,
        view_part_diagnostics,
        checker,
    )
    .0
}

fn finish_type_check_with_call_facts(
    module: &HirModule,
    style_catalog: crate::style::CheckedViewStyleCatalog,
    style_diagnostics: Vec<crate::style::StyleDiagnostic>,
    view_part_catalog: crate::view_part::CheckedViewPartCatalog,
    view_part_diagnostics: Vec<ViewPartDiagnostic>,
    checker: &mut TypeChecker<'_>,
) -> (TypeCheckReport, CallTargetFactReport) {
    checker
        .errors
        .extend(style_diagnostics.into_iter().map(TypeCheckError::style));
    checker.errors.extend(
        view_part_diagnostics
            .iter()
            .map(|diagnostic| TypeCheckError::new(diagnostic.message().to_owned())),
    );
    checker.check_module(module);
    checker.apply_pending_higher_order_effect_calls();
    let effects = std::mem::take(&mut checker.effect_collector).finish();
    checker.extend_effect_diagnostics(&effects);
    checker
        .warnings
        .extend(effects.warnings().cloned().map(TypeCheckWarning::effect));
    let canonicalization_inventories = checker.finish_canonicalization_inventories();
    let call_target_fact_recorder = std::mem::replace(
        &mut checker.call_target_fact_recorder,
        CallTargetFactRecorder::new(CallTargetFactMode::Disabled),
    );
    let call_target_fact_report = call_target_fact_recorder.finish();
    let report = TypeCheckReport {
        diagnostics: std::mem::take(&mut checker.errors),
        warnings: std::mem::take(&mut checker.warnings),
        stats: checker.stats.clone(),
        judgments: std::mem::take(&mut checker.judgments),
        typed_lowering_evidence: std::mem::take(&mut checker.typed_lowering_evidence),
        closure_captures: std::mem::take(&mut checker.closure_captures),
        numeric_fallbacks: std::mem::take(&mut checker.numeric_fallbacks),
        callable_executions: std::mem::take(&mut checker.callable_executions),
        effects,
        for_iteration_evidence: std::mem::take(&mut checker.for_iteration_evidence),
        trait_catalog: std::mem::take(&mut checker.trait_catalog),
        style_catalog,
        view_part_catalog,
        view_part_diagnostics,
        canonicalization_inventories,
        project_callable_references: std::mem::take(&mut checker.project_callable_references),
        project_entity_references: std::mem::take(&mut checker.project_entity_references),
        call_target_fact_report: call_target_fact_report.clone(),
    };
    (report, call_target_fact_report)
}

impl TypeChecker<'_> {
    pub(super) fn check_module(&mut self, module: &HirModule) {
        self.stats.flows += module.flows().len();
        self.stats.functions += module.functions().len();
        self.stats.declarations += module.declarations().len();

        if let Err(errors) = validate_typecheck_ready(module) {
            self.errors.extend(
                errors
                    .into_iter()
                    .map(|error| TypeCheckError::new(error.message().to_owned())),
            );
        }

        self.collect_and_store_trait_catalog(module);
        self.action_signatures = view::collect_action_signatures(module, &mut self.errors);
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

        self.with_runtime_for_iteration_evidence(|this| {
            this.check_module_flows(module.flows());
        });
        self.check_module_functions(module.functions());
        for declaration in module.declarations() {
            self.check_top_level_decl(declaration);
        }
        self.locals.clear();
        self.reset_semantic_root_scope(None);
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
                .map(|ty| type_ref_kind(ty.value()));
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
            let higher_order_param_scope =
                self.build_higher_order_param_scope(function, &generic_names);
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
                .map(|ty| type_ref_kind_with_generics(ty.value(), &generic_names));
            let execution = Self::classify_callable_execution(function, expected_return.as_ref());
            for contract in function.contracts() {
                self.check_function_contract_clause(contract, expected_return.as_ref());
            }
            let effect_scope = EffectScope::from_contracts(function.contracts());
            let effect_snapshot = self.apply_effect_scope(&effect_scope);
            let previous_callable = self
                .effect_collector
                .enter(self.effect_callable_id_for_function(function));
            let callable_declaration = self.project_symbols.and_then(|symbols| {
                CallableDeclarationId::for_function(symbols.world().package(), function).ok()
            });
            let typed_lowering_owner =
                callable_declaration
                    .clone()
                    .map(|declaration| super::TypedLoweringOwnerScope {
                        declaration,
                        expression_base: self.stats.expressions,
                    });
            let previous_typed_lowering_owner =
                std::mem::replace(&mut self.typed_lowering_owner, typed_lowering_owner);
            self.higher_order_param_scope_stack
                .push(higher_order_param_scope);
            let predicates = self
                .trait_catalog
                .predicates_for_signature(function.signature());
            self.trait_predicate_stack.push(predicates);
            if function.kind() == FunctionKind::Stream
                || matches!(execution, CallableExecutionMode::StreamFactory { .. })
            {
                self.check_stream_function(function);
                self.trait_predicate_stack.pop();
                self.higher_order_param_scope_stack.pop();
                self.typed_lowering_owner = previous_typed_lowering_owner;
                self.effect_collector.restore(previous_callable);
                self.effect_capabilities = effect_snapshot;
                if let Some(declaration) = callable_declaration {
                    self.callable_executions
                        .push(CheckedCallableExecution::new(declaration, execution));
                }
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
            self.typed_lowering_owner = previous_typed_lowering_owner;
            self.effect_collector.restore(previous_callable);
            self.effect_capabilities = effect_snapshot;
            if let Some(declaration) = callable_declaration {
                self.callable_executions
                    .push(CheckedCallableExecution::new(declaration, execution));
            }
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

    fn classify_callable_execution(
        function: &HirFunction,
        return_type: Option<&TypeKind>,
    ) -> CallableExecutionMode {
        let own_scope_yield_count = Self::own_scope_yield_count(function.statements())
            + function
                .value()
                .map_or(0, |value| Self::expr_own_scope_yield_count(value.expr()));
        match return_type {
            Some(TypeKind::Stream { item, error }) if own_scope_yield_count > 0 => {
                CallableExecutionMode::StreamFactory {
                    item: (**item).clone(),
                    error: (**error).clone(),
                    generator: StreamGeneratorFacts::new(own_scope_yield_count),
                }
            }
            _ => CallableExecutionMode::DirectFrame,
        }
    }

    fn own_scope_yield_count(statements: &[Stmt]) -> usize {
        statements
            .iter()
            .map(Self::stmt_own_scope_yield_count)
            .sum()
    }

    fn stmt_own_scope_yield_count(statement: &Stmt) -> usize {
        match statement {
            Stmt::Let { expr, .. } | Stmt::Return { expr, .. } | Stmt::Expr { expr, .. } => {
                Self::expr_own_scope_yield_count(expr)
            }
            Stmt::Assign { target, expr }
            | Stmt::LifetimeSet { target, expr }
            | Stmt::Signal {
                target,
                value: expr,
            } => {
                Self::expr_own_scope_yield_count(target.expr())
                    + Self::expr_own_scope_yield_count(expr.expr())
            }
            Stmt::LetElse {
                expr, else_body, ..
            } => {
                Self::expr_own_scope_yield_count(expr.expr())
                    + Self::own_scope_yield_count(else_body)
            }
            Stmt::LetScope { scope, .. } => {
                Self::own_scope_yield_count(scope.statements())
                    + scope.value().map_or(0, Self::expr_own_scope_yield_count)
            }
            Stmt::DeferBlock { statements, .. } => Self::own_scope_yield_count(statements),
            Stmt::Defer { expr, .. }
            | Stmt::Out { expr, .. }
            | Stmt::Goto(expr)
            | Stmt::Close(expr)
            | Stmt::Select(expr) => Self::expr_own_scope_yield_count(expr.expr()),
            Stmt::Yield(expr) => 1 + Self::expr_own_scope_yield_count(expr.expr()),
            Stmt::UnsafeLifetime { body, .. } | Stmt::Loop { body } | Stmt::While { body, .. } => {
                Self::own_scope_yield_count(body)
            }
            Stmt::WhileLet {
                expr, guard, body, ..
            } => {
                Self::expr_own_scope_yield_count(expr.expr())
                    + guard
                        .as_ref()
                        .map_or(0, |guard| Self::expr_own_scope_yield_count(guard.expr()))
                    + Self::own_scope_yield_count(body)
            }
            Stmt::For { source, body, .. } => {
                Self::expr_own_scope_yield_count(source.expr()) + Self::own_scope_yield_count(body)
            }
            Stmt::If {
                condition,
                body,
                else_body,
            } => {
                Self::expr_own_scope_yield_count(condition.expr())
                    + Self::own_scope_yield_count(body)
                    + Self::own_scope_yield_count(else_body)
            }
            Stmt::Match { expr, arms } => {
                Self::expr_own_scope_yield_count(expr.expr())
                    + arms
                        .iter()
                        .map(|arm| {
                            arm.guard_authored()
                                .map_or(0, |guard| Self::expr_own_scope_yield_count(guard.expr()))
                                + Self::own_scope_yield_count(arm.body())
                        })
                        .sum::<usize>()
            }
            Stmt::Break { expr, .. } => expr
                .as_ref()
                .map_or(0, |expr| Self::expr_own_scope_yield_count(expr.expr())),
            // Thread and event-handler bodies execute under independent runtime owners. Expressions
            // may contain closures, Seq blocks, or Source values, all of which likewise own their
            // yields and therefore are intentionally not traversed here.
            Stmt::Assertion(_)
            | Stmt::LetChoice { .. }
            | Stmt::LetLoop { .. }
            | Stmt::LetAwait { .. }
            | Stmt::LetActionReceive { .. }
            | Stmt::Thread(_)
            | Stmt::Wait(_)
            | Stmt::On { .. }
            | Stmt::Continue { .. }
            | Stmt::Raw(_) => 0,
        }
    }

    fn expr_own_scope_yield_count(expression: &Expr) -> usize {
        match expression {
            Expr::Tuple(items) | Expr::BracketSeq(items) => {
                items.iter().map(Self::expr_own_scope_yield_count).sum()
            }
            Expr::ArrayRepeat { value, len }
            | Expr::Index {
                target: value,
                index: len,
            }
            | Expr::Pipe {
                lhs: value,
                rhs: len,
            }
            | Expr::Binary {
                lhs: value,
                rhs: len,
                ..
            } => Self::expr_own_scope_yield_count(value) + Self::expr_own_scope_yield_count(len),
            Expr::Call(call) => {
                Self::expr_own_scope_yield_count(call.callee())
                    + call
                        .args()
                        .iter()
                        .map(|argument| Self::expr_own_scope_yield_count(argument.value()))
                        .sum::<usize>()
            }
            Expr::Select(select) => Self::expr_own_scope_yield_count(select.target()),
            Expr::DialogueCall { callee, .. } => Self::expr_own_scope_yield_count(callee),
            Expr::Try(tried) => Self::expr_own_scope_yield_count(tried.operand()),
            Expr::Await(awaited) => Self::expr_own_scope_yield_count(awaited.operand()),
            Expr::Range { start, end, .. } => {
                start.as_deref().map_or(0, Self::expr_own_scope_yield_count)
                    + end.as_deref().map_or(0, Self::expr_own_scope_yield_count)
            }
            Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => fields
                .iter()
                .map(|(_, value)| Self::expr_own_scope_yield_count(value))
                .sum(),
            Expr::Borrow(borrowed) => Self::expr_own_scope_yield_count(borrowed.operand()),
            Expr::Deref(dereferenced) => Self::expr_own_scope_yield_count(dereferenced.operand()),
            Expr::Unary { expr, .. } => Self::expr_own_scope_yield_count(expr),
            Expr::Block { statements, value }
            | Expr::NamedBlock {
                statements, value, ..
            } => {
                Self::own_scope_yield_count(statements)
                    + value.as_deref().map_or(0, Self::expr_own_scope_yield_count)
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                Self::expr_own_scope_yield_count(condition)
                    + Self::expr_own_scope_yield_count(then_branch)
                    + else_branch
                        .as_deref()
                        .map_or(0, Self::expr_own_scope_yield_count)
            }
            Expr::IfLet {
                expr,
                guard,
                then_branch,
                else_branch,
                ..
            } => {
                Self::expr_own_scope_yield_count(expr)
                    + guard.as_deref().map_or(0, Self::expr_own_scope_yield_count)
                    + Self::expr_own_scope_yield_count(then_branch)
                    + else_branch
                        .as_deref()
                        .map_or(0, Self::expr_own_scope_yield_count)
            }
            Expr::Match { scrutinee, arms } => {
                Self::expr_own_scope_yield_count(scrutinee)
                    + arms
                        .iter()
                        .map(|arm| {
                            arm.guard().map_or(0, Self::expr_own_scope_yield_count)
                                + Self::expr_own_scope_yield_count(arm.value())
                        })
                        .sum::<usize>()
            }
            // These variants introduce independent execution/yield owners or contain no child
            // expression capable of owning a statement-level yield.
            Expr::Closure { .. }
            | Expr::ComputationBlock { .. }
            | Expr::Thread { .. }
            | Expr::Literal(_)
            | Expr::EntityRef(_)
            | Expr::LifetimePath { .. }
            | Expr::Path(_)
            | Expr::ShortVariant(_)
            | Expr::Placeholder(_)
            | Expr::NumericBracketSeq(_)
            | Expr::Raw(_) => 0,
        }
    }

    fn build_higher_order_param_scope(
        &self,
        function: &HirFunction,
        generic_names: &HashSet<String>,
    ) -> super::HigherOrderParamScope {
        super::HigherOrderParamScope {
            function_name: function.name().to_owned(),
            callable: self.effect_callable_id_for_function(function),
            param_names: function
                .signature()
                .param_groups()
                .iter()
                .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
                .flat_map(|param| {
                    let ty = function_param_local_type_with_generics(param, generic_names);
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
                if let Some(ty) = param.ty() {
                    self.warn_public_type_ref_anonymous_sum(
                        ty.value(),
                        &format!(
                            "public function `{}` parameter `{}`",
                            function.name(),
                            pattern_public_label(param.pattern())
                        ),
                    );
                }
            }
        }
        if let Some(return_type) = function.signature().return_type() {
            self.warn_public_type_ref_anonymous_sum(
                return_type.value(),
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
                .insert(item.name().to_owned(), type_ref_kind(item.target().value()));
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
                let body_effects = self
                    .effect_collector
                    .inferred_effect_row(&self.effect_callable_id_for_function(function))
                    .unwrap_or_else(|| self.function_effect_row(function.name()));
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
            let callable = self.effect_callable_id_for_function(function);
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

    fn effect_callable_id_for_function(&self, function: &HirFunction) -> CallableId {
        self.project_symbols.map_or_else(
            || function_callable_id(function.name()),
            |symbols| {
                CallableDeclarationId::for_function(symbols.world().package(), function)
                    .map_or_else(
                        |_| function_callable_id(function.name()),
                        |declaration| CallableId::project_function(&declaration),
                    )
            },
        )
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive match keeps every HIR top-level declaration family visible at the checker dispatch boundary"
    )]
    pub(super) fn check_top_level_decl(&mut self, declaration: &HirTopLevelDecl) {
        match declaration {
            HirTopLevelDecl::Enum(_)
            | HirTopLevelDecl::Proof(_)
            | HirTopLevelDecl::Struct(_)
            | HirTopLevelDecl::Trait(_)
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
                self.check_view_declaration(item);
            }
            HirTopLevelDecl::TypeAlias(item) => {
                self.check_type_alias_decl(item);
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

    fn check_extern_mod(&mut self, item: &ExternModItem) {
        if item.abi() != "rust" {
            return;
        }
        let Some(ExternModSource::Crate(package)) = item.source() else {
            self.errors.push(TypeCheckError::new(format!(
                "extern rust module `{}` must declare `from crate \"name\"`",
                item.path()
            )));
            return;
        };
        let catalog = self
            .registered_world
            .map(|world| world.environment().callable_catalog().environment());
        if !catalog.is_some_and(|catalog| catalog.has_rust_package(package)) {
            self.errors
                .push(TypeCheckError::missing_rust_package_metadata(package));
            return;
        }
        let type_exports = self.env.rust_package(package);
        let namespace = item.path().to_string();
        for member in item.members() {
            match member {
                ExternModMember::Type(ty) => {
                    if !type_exports.is_some_and(|exports| exports.has_type(ty.name())) {
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
                    let Ok(export) =
                        crate::callable::CallableName::try_new(function.signature().name())
                    else {
                        self.errors
                            .push(TypeCheckError::missing_rust_export(package, export_name));
                        continue;
                    };
                    let candidates = catalog
                        .into_iter()
                        .flat_map(|catalog| catalog.rust_exports(package, &export))
                        .cloned()
                        .collect::<Vec<_>>();
                    if candidates.is_empty() {
                        self.errors
                            .push(TypeCheckError::missing_rust_export(package, export_name));
                        continue;
                    }
                    if !candidates
                        .iter()
                        .any(|record| record.schema().matches_function_signature(&expected))
                    {
                        self.errors
                            .push(TypeCheckError::rust_export_signature_mismatch(
                                package,
                                export_name,
                                expected.source_label(),
                                candidates[0].schema().source_label(),
                            ));
                    }
                }
                ExternModMember::Activity(activity) => {
                    self.check_type_ref_shape(activity.ty().value());
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
                item.target().value(),
                &format!("public type alias `{}`", item.name()),
            );
        }
        self.check_type_ref_shape(item.target().value());
        self.with_local_mutation_scope(|this| {
            this.bind_local("self".to_owned(), type_ref_kind(item.target().value()));
            for clause in item.where_clauses() {
                this.check_type_ref_shape(clause.subject().value());
                for bound in clause.bounds() {
                    this.check_type_ref_shape(bound.value());
                }
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
                .map(|ty| type_ref_kind_for_impl(ty.value(), &self_ty, &generic_names));
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
            let ty = param.ty().map_or(TypeKind::Unit, |ty| {
                type_ref_kind_for_impl(ty.value(), self_ty, generic_names)
            });
            if param.is_rest() {
                TypeKind::Vec(Box::new(ty))
            } else {
                ty
            }
        };
        self.bind_function_param(param.pattern(), &ty);
    }

    fn check_entry_item(&mut self, item: &HirEntryItem) {
        match item {
            HirEntryItem::StateType { .. }
            | HirEntryItem::Initializer { .. }
            | HirEntryItem::EventType { .. }
            | HirEntryItem::Reducer { .. }
            | HirEntryItem::Controller { .. } => {
                // Project entry binding resolves these typed role references after
                // ordinary nominal and callable catalogs are complete.
            }
            HirEntryItem::Goto(target) => {
                self.expect_entity_kind(target, &EntityKind::Flow, "entry flow target");
            }
            HirEntryItem::Route {
                target,
                path,
                bindings,
                ..
            } => {
                self.expect_entity_kind(target, &EntityKind::Flow, "entry route target");
                self.check_route_bindings(target, path, bindings);
            }
            HirEntryItem::Option { value, .. } => {
                self.check_expr(value);
            }
            HirEntryItem::Raw(raw) => {
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
            .and_then(|ty| stream_return_types(ty.value()))
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
            if let Some(ty) = param.ty() {
                self.check_type_ref_shape(ty.value());
            }
        }
        if let Some(return_type) = signature.return_type() {
            self.check_type_ref_shape(return_type.value());
        }
        for clause in signature.where_clauses() {
            self.check_type_ref_shape(clause.subject().value());
            for bound in clause.bounds() {
                self.check_type_ref_shape(bound.value());
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
                for binding in bound.associated() {
                    self.check_type_ref_shape(binding.value());
                }
            }
            TypeRef::Projection { subject, .. } => self.check_type_ref_shape(subject),
            TypeRef::Reference(reference) => self.check_type_ref_shape(reference.referent()),
            TypeRef::Slice(inner) => self.check_type_ref_shape(inner),
            TypeRef::Recovery(id) => self.errors.push(TypeCheckError::new(format!(
                "recovered type node {} is not type-checkable",
                id.index()
            ))),
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
            TypeKind::BorrowRef {
                kind,
                lifetime,
                inner,
            } => TypeKind::BorrowRef {
                kind: *kind,
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
        Expr::Call(call) => unknown_default_inline_failure_constructor(call.callee(), call.args()),
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
                    .associated()
                    .iter()
                    .any(|binding| type_ref_contains_choice(binding.value()))
        }
        TypeRef::Projection { subject, .. } => type_ref_contains_choice(subject),
        TypeRef::Reference(reference) => type_ref_contains_choice(reference.referent()),
        TypeRef::Slice(inner) => type_ref_contains_choice(inner),
        TypeRef::Never | TypeRef::ConstInt(_) | TypeRef::Path(_) | TypeRef::Recovery(_) => false,
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

fn entry_item_flow_target(
    item: &HirEntryItem,
) -> Option<&arcweft_lang_syntax::ast::ids::EntityRef> {
    match item {
        HirEntryItem::Goto(target) | HirEntryItem::Route { target, .. } => Some(target),
        HirEntryItem::StateType { .. }
        | HirEntryItem::Initializer { .. }
        | HirEntryItem::EventType { .. }
        | HirEntryItem::Reducer { .. }
        | HirEntryItem::Controller { .. }
        | HirEntryItem::Option { .. }
        | HirEntryItem::Raw(_) => None,
    }
}

fn flow_callable_id(name: &str) -> CallableId {
    CallableId::source_flow(name)
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
        .map(|field| (field.name().to_owned(), type_ref_kind(field.ty().value())))
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
    _enum_name: &str,
    variant: &EnumVariant,
    _errors: &mut Vec<TypeCheckError>,
) -> EnumVariantPayload {
    let Some(payload) = variant.payload() else {
        return EnumVariantPayload::Unit;
    };
    enum_variant_tuple_payload_from_type_ref(payload.value())
}

fn enum_variant_tuple_payload_from_type_ref(ty: &TypeRef) -> EnumVariantPayload {
    match ty {
        TypeRef::Tuple(items) => {
            EnumVariantPayload::Tuple(items.iter().map(type_ref_kind).collect())
        }
        ty => EnumVariantPayload::Tuple(vec![type_ref_kind(ty)]),
    }
}

fn impl_target_type(item: &ImplItem) -> TypeKind {
    type_ref_kind_for_impl(
        item.target().value(),
        &TypeKind::Named("Self".to_owned()),
        &HashSet::new(),
    )
}

fn impl_generic_names(generics: &[GenericParam]) -> HashSet<String> {
    generics
        .iter()
        .filter_map(GenericParam::as_type)
        .map(|name| name.as_str().to_owned())
        .collect()
}

fn type_ref_kind_for_impl(
    ty: &TypeRef,
    self_ty: &TypeKind,
    generic_names: &HashSet<String>,
) -> TypeKind {
    match ty {
        TypeRef::Path(path) if crate::types::direct_type_name(path) == Some("Self") => {
            self_ty.clone()
        }
        TypeRef::Generic { base, args }
            if crate::types::direct_type_name(base) == Some("Option") && args.len() == 1 =>
        {
            TypeKind::Option(Box::new(type_ref_kind_for_impl(
                &args[0],
                self_ty,
                generic_names,
            )))
        }
        TypeRef::Generic { base, args }
            if crate::types::direct_type_name(base) == Some("Vec") && args.len() == 1 =>
        {
            TypeKind::Vec(Box::new(type_ref_kind_for_impl(
                &args[0],
                self_ty,
                generic_names,
            )))
        }
        TypeRef::Generic { base, args }
            if crate::types::direct_type_name(base) == Some("Result") && args.len() == 2 =>
        {
            TypeKind::Result {
                ok: Box::new(type_ref_kind_for_impl(&args[0], self_ty, generic_names)),
                error: Box::new(type_ref_kind_for_impl(&args[1], self_ty, generic_names)),
            }
        }
        TypeRef::Reference(reference) => TypeKind::BorrowRef {
            kind: reference.kind(),
            lifetime: reference
                .region()
                .name()
                .map(|lifetime| LifetimeScopeKind::parse(lifetime.name())),
            inner: Box::new(type_ref_kind_for_impl(
                reference.referent(),
                self_ty,
                generic_names,
            )),
        },
        _ => type_ref_kind_with_generics(ty, generic_names),
    }
}
