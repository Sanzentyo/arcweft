//! Flow-runtime lowering.

mod binding;
mod closure_metadata;
mod optimizer;
mod pure_helpers;
mod record_projection;
mod syntax_helpers;
mod value_block;

pub use closure_metadata::{RuntimeClosureCapture, RuntimeClosureCaptureInventory};

pub(crate) use self::syntax_helpers::sanitize_task_id_part;
use self::{
    binding::LoweredLetBinding,
    pure_helpers::runtime_pure_helper_inventory,
    syntax_helpers::{
        agent_task_name, dialogue_call_parts, flow_runtime_id, method_name, parallel_limit,
        selected_call_parts, split_capability_operation, traverse_callee,
    },
    value_block::FlowValueBlock,
};
use crate::errors::{LinePlanLowerError, RuntimePlanLowerError};
use crate::expr::{
    LoweredRuntimeEffect, RuntimePureHelperLookup, lower_runtime_effect_strict_with_pure,
    lower_runtime_expr_strict_with_expected_type, lower_runtime_expr_strict_with_pure,
};
use crate::function_values::{lower_runtime_function_value_candidates, runtime_function_value_map};
use crate::host_request::{lower_agent_host_task_request, lower_host_task_request};
use crate::labels::expr_label;
use crate::line_task::{lower_line_plan, lower_line_plan_statements};
use crate::lowering_context::ExecutableLoweringLocation;
use crate::pattern::lower_runtime_pattern_checked;
use crate::pure::lower_pure_helper_candidates;
use crate::render_text::{
    DialogueDisplayDefaults, DialogueSpeakerPreset, FxCatalog,
    lower_dialogue_display_with_speaker_presets_and_fx, speaker_preset_from_let,
};
use crate::source::lower_source_plan;
use crate::stream::lower_stream_function;
use crate::typed_evidence::RuntimeTypedLoweringEvidence;
use arcweft_core::effect::LineEffectRequest;
use arcweft_core::line_task::{LineOutRequest, LineTaskGroup};
use arcweft_core::pattern::RuntimePattern;
use arcweft_core::plan::{
    ChoiceRuntimeOption, EntryRuntimeId, FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec,
    RuntimeEntryTarget, RuntimeFlow, RuntimeHostCallTarget, RuntimeIteratorEvidence, RuntimeLineId,
    RuntimeMatchArm, RuntimePlan, RuntimeRouteBinding, RuntimeRouteBindingSource, RuntimeRouteSpec,
    RuntimeTraitMethod,
};
use arcweft_core::step::RuntimeHostCallMode;
use arcweft_core::task::{
    AWAIT_MANY_ITEM_BINDING, AwaitManyTarget, AwaitTarget, HostTaskArgTemplate,
    HostTaskRequestTemplate, NeedId, TaskId,
};
use arcweft_core::value::{RuntimeExpr, RuntimeValue};
use arcweft_lang_hir::model::{
    HirAgent, HirAwait, HirChoice, HirChoiceOption, HirDialogue, HirFlow, HirFlowItem, HirFor,
    HirLoop, HirMatch, HirModule, HirScopeExpr, HirThread, HirTopLevelDecl,
};
use arcweft_lang_hir::syntax::ast::{
    choice::ChoiceAction,
    flow::{
        AuthoredExpr, AwaitBranchKind, FlowItem, ScopeExprBlock, Stmt, StmtMatchArm, ThreadBlock,
    },
    ids::EntityRefSyntax,
    items::{EntryItem, EntryKind, FunctionKind},
    pattern::Pattern,
};
use arcweft_lang_hir::syntax::expr::Expr;
use arcweft_lang_hir::syntax::types::TypeRef;
use arcweft_render_text::LineDisplayCatalog;
use presentation::{
    presentation_create_args, presentation_explicit_mount_handle_id, presentation_handle_call,
    presentation_handle_id, presentation_mount_call,
};
use std::{cell::Cell, collections::BTreeMap};

mod presentation;

pub(crate) struct LoweredRuntimeFlows {
    pub(crate) flows: Vec<RuntimeFlow>,
    pub(crate) line_task_groups: Vec<LineTaskGroup>,
    pub(crate) line_display_catalog: LineDisplayCatalog,
}

/// Runtime-plan lowering result plus compiler-side optimization counters.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimePlanLowerReport {
    pub plan: RuntimePlan,
    pub stats: RuntimePlanLowerStats,
    pub line_display_catalog: LineDisplayCatalog,
    pub closure_captures: Vec<RuntimeClosureCaptureInventory>,
}

/// Options that select profile/build-context inputs for runtime-plan lowering.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimePlanLowerOptions {
    package_identity: Option<String>,
    dialogue_defaults: Option<String>,
    for_iteration_evidence: Vec<RuntimeIteratorEvidence>,
    trait_methods: Vec<RuntimeTraitMethod>,
    typed_lowering_evidence: Vec<RuntimeTypedLoweringEvidence>,
    closure_captures: Vec<RuntimeClosureCaptureInventory>,
    required_typed_lowering_evidence_len: Option<usize>,
}

impl RuntimePlanLowerOptions {
    /// Creates default source-local lowering options.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            package_identity: None,
            dialogue_defaults: None,
            for_iteration_evidence: Vec::new(),
            trait_methods: Vec::new(),
            typed_lowering_evidence: Vec::new(),
            closure_captures: Vec::new(),
            required_typed_lowering_evidence_len: None,
        }
    }

    /// Selects the canonical package identity used by compiled cross-section
    /// references such as source-defined Fx applications.
    #[must_use]
    pub fn with_package_identity(mut self, package: impl Into<String>) -> Self {
        self.package_identity = Some(package.into());
        self
    }

    /// Selects a dialogue defaults profile by entity ID, for example
    /// `dialogue.mobile`.
    #[must_use]
    pub fn with_dialogue_defaults(mut self, id: impl Into<String>) -> Self {
        self.dialogue_defaults = Some(id.into());
        self
    }

    #[must_use]
    pub fn with_for_iteration_evidence(
        mut self,
        evidence: impl IntoIterator<Item = RuntimeIteratorEvidence>,
    ) -> Self {
        self.for_iteration_evidence = evidence.into_iter().collect();
        self
    }

    #[must_use]
    pub fn with_trait_methods(
        mut self,
        trait_methods: impl IntoIterator<Item = RuntimeTraitMethod>,
    ) -> Self {
        self.trait_methods = trait_methods.into_iter().collect();
        self
    }

    #[must_use]
    pub fn with_typed_lowering_evidence(
        mut self,
        evidence: impl IntoIterator<Item = RuntimeTypedLoweringEvidence>,
    ) -> Self {
        self.typed_lowering_evidence = evidence.into_iter().collect();
        self
    }

    /// Requires checked-build lowering to receive the exact typed evidence
    /// count exported by semantic analysis.
    #[must_use]
    pub fn with_required_typed_lowering_evidence_len(mut self, len: usize) -> Self {
        self.required_typed_lowering_evidence_len = Some(len);
        self
    }

    /// Selected dialogue defaults profile ID, if supplied by a launch profile.
    #[must_use]
    pub fn dialogue_defaults(&self) -> Option<&str> {
        self.dialogue_defaults.as_deref()
    }

    /// Canonical package identity for source-defined cross-section references.
    ///
    /// Source-local lowering uses `crate`; project and bundle drivers should
    /// supply the selected package identity explicitly.
    #[must_use]
    pub fn package_identity(&self) -> &str {
        self.package_identity.as_deref().unwrap_or("crate")
    }

    pub fn for_iteration_evidence(&self) -> &[RuntimeIteratorEvidence] {
        &self.for_iteration_evidence
    }

    pub fn trait_methods(&self) -> &[RuntimeTraitMethod] {
        &self.trait_methods
    }

    pub fn typed_lowering_evidence(&self) -> &[RuntimeTypedLoweringEvidence] {
        &self.typed_lowering_evidence
    }

    fn validate_typed_lowering_evidence(&self) -> Result<(), RuntimePlanLowerError> {
        let Some(required_len) = self.required_typed_lowering_evidence_len else {
            return Ok(());
        };
        let actual_len = self.typed_lowering_evidence.len();
        if actual_len == required_len {
            return Ok(());
        }
        Err(RuntimePlanLowerError::new(format!(
            "checked runtime lowering expected {required_len} typed lowering evidence record(s), found {actual_len}; pass the TypeCheckReport-derived evidence into RuntimePlanLowerOptions"
        )))
    }
}

/// Compiler-side counters for runtime-plan pure and flow optimization work.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RuntimePlanLowerStats {
    pub pure_helpers: usize,
    pub pure_candidate_functions_seen: usize,
    pub pure_candidate_lower_attempts: usize,
    pub pure_candidate_lower_failures_inferred: usize,
    pub pure_expr_lowered_nodes: usize,
    pub pure_expr_cloned_nodes: usize,
    pub pure_rewrite_expr_visits: usize,
    pub optimized_flows: usize,
    pub optimized_op_slices: usize,
    pub local_use_tail_scans: usize,
    pub local_use_scan_ops: usize,
    pub sequence_map_sum_fusions: usize,
    pub map_sum_fusions: usize,
    pub sequence_source_inlines: usize,
    pub pure_call_exprs: usize,
}

/// Lowers checked HIR flows to the Sans I/O core runtime program.
///
/// This pass is intentionally stricter than line-task-only lowering: it must
/// not silently skip flow syntax because the engine would otherwise execute a
/// different story than the source describes.
pub fn lower_runtime_plan(module: &HirModule) -> Result<RuntimePlan, Vec<RuntimePlanLowerError>> {
    lower_runtime_plan_with_stats(module).map(|report| report.plan)
}

/// Lowers checked HIR with explicit profile/build-context options.
pub fn lower_runtime_plan_with_options(
    module: &HirModule,
    options: &RuntimePlanLowerOptions,
) -> Result<RuntimePlan, Vec<RuntimePlanLowerError>> {
    lower_runtime_plan_with_stats_and_options(module, options).map(|report| report.plan)
}

/// Lowers checked HIR to a runtime plan and records lowering-time counters.
pub fn lower_runtime_plan_with_stats(
    module: &HirModule,
) -> Result<RuntimePlanLowerReport, Vec<RuntimePlanLowerError>> {
    lower_runtime_plan_with_stats_and_options(module, &RuntimePlanLowerOptions::default())
}

/// Lowers checked HIR with explicit profile/build-context options and records
/// lowering-time counters.
pub fn lower_runtime_plan_with_stats_and_options(
    module: &HirModule,
    options: &RuntimePlanLowerOptions,
) -> Result<RuntimePlanLowerReport, Vec<RuntimePlanLowerError>> {
    options
        .validate_typed_lowering_evidence()
        .map_err(|error| vec![error])?;
    let pure_candidate_report = lower_pure_helper_candidates(module).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| RuntimePlanLowerError::new(error.to_string()))
            .collect::<Vec<_>>()
    })?;
    let mut stats = RuntimePlanLowerStats {
        pure_candidate_functions_seen: pure_candidate_report.stats.functions_seen,
        pure_candidate_lower_attempts: pure_candidate_report.stats.lower_attempts,
        pure_candidate_lower_failures_inferred: pure_candidate_report.stats.lower_failures_inferred,
        pure_expr_lowered_nodes: pure_candidate_report.stats.expr_lowered_nodes,
        ..RuntimePlanLowerStats::default()
    };
    let (pure_helpers, pure_map) =
        runtime_pure_helper_inventory(&pure_candidate_report.candidates, &mut stats);
    let pure_lookup = RuntimePureHelperLookup::new(&pure_map, &pure_helpers);
    let function_value_candidates = lower_runtime_function_value_candidates(module, pure_lookup);
    let function_values = runtime_function_value_map(&function_value_candidates);
    let entries = lower_runtime_entries(module);
    let (flows, line_task_groups, line_display_catalog, stream_plans, source_plans) = {
        let typed_expression_cursor = Cell::new(0);
        let pure_lookup = pure_lookup
            .with_runtime_function_values(&function_values)
            .with_typed_lowering_evidence(
                options.typed_lowering_evidence(),
                &typed_expression_cursor,
            );
        let lowered_flows = lower_runtime_flows(module, pure_lookup, options)?;
        let LoweredRuntimeFlows {
            flows,
            line_task_groups,
            line_display_catalog,
        } = lowered_flows;
        let stream_plans = module
            .functions()
            .iter()
            .filter(|function| function.kind() == FunctionKind::Stream)
            .map(|function| lower_stream_function(module, function, pure_lookup))
            .collect::<Result<Vec<_>, _>>()?;
        let source_plans = module
            .declarations()
            .iter()
            .filter_map(|decl| match decl {
                HirTopLevelDecl::Source(source) => {
                    Some(lower_source_plan(module, source, pure_lookup))
                }
                _ => None,
            })
            .collect::<Result<Vec<_>, _>>()?;
        (
            flows,
            line_task_groups,
            line_display_catalog,
            stream_plans,
            source_plans,
        )
    };
    let entry = implicit_entry_flow(&entries, &flows);
    stats.pure_helpers = pure_helpers.len();
    RuntimePlan::new(entry, flows, line_task_groups)
        .map(|plan| {
            let plan = optimizer::finalize_runtime_plan(
                plan.with_entries(entries)
                    .with_generation_plans(stream_plans, source_plans)
                    .with_pure_helpers(pure_helpers)
                    .with_trait_methods(options.trait_methods.clone()),
                &mut stats,
            );
            RuntimePlanLowerReport {
                plan,
                stats,
                line_display_catalog,
                closure_captures: options.closure_captures.clone(),
            }
        })
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])
}

/// Lowers a checked Agent controller body to the same runtime-plan/bytecode
/// shape used by ordinary flows.
pub fn lower_agent_controller_plan_with_stats(
    module: &HirModule,
    agent: &HirAgent,
) -> Result<RuntimePlanLowerReport, Vec<RuntimePlanLowerError>> {
    lower_agent_controller_plan_with_stats_and_options(
        module,
        agent,
        &RuntimePlanLowerOptions::default(),
    )
}

pub fn lower_agent_controller_plan_with_stats_and_options(
    module: &HirModule,
    agent: &HirAgent,
    options: &RuntimePlanLowerOptions,
) -> Result<RuntimePlanLowerReport, Vec<RuntimePlanLowerError>> {
    options
        .validate_typed_lowering_evidence()
        .map_err(|error| vec![error])?;
    let pure_candidate_report = lower_pure_helper_candidates(module).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| RuntimePlanLowerError::new(error.to_string()))
            .collect::<Vec<_>>()
    })?;
    let mut stats = RuntimePlanLowerStats {
        pure_candidate_functions_seen: pure_candidate_report.stats.functions_seen,
        pure_candidate_lower_attempts: pure_candidate_report.stats.lower_attempts,
        pure_candidate_lower_failures_inferred: pure_candidate_report.stats.lower_failures_inferred,
        pure_expr_lowered_nodes: pure_candidate_report.stats.expr_lowered_nodes,
        ..RuntimePlanLowerStats::default()
    };
    let (pure_helpers, pure_map) =
        runtime_pure_helper_inventory(&pure_candidate_report.candidates, &mut stats);
    let pure_lookup = RuntimePureHelperLookup::new(&pure_map, &pure_helpers);
    let function_value_candidates = lower_runtime_function_value_candidates(module, pure_lookup);
    let function_values = runtime_function_value_map(&function_value_candidates);
    let lowered = {
        let typed_expression_cursor = Cell::new(0);
        let pure_lookup = pure_lookup
            .with_runtime_function_values(&function_values)
            .with_typed_lowering_evidence(
                options.typed_lowering_evidence(),
                &typed_expression_cursor,
            );
        lower_agent_controller_flow(module, agent, pure_lookup, options)?
    };
    let entry_flow = lowered.id.clone();
    let entry_id = EntryRuntimeId::canonical(&entry_flow.canonical_label())
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    stats.pure_helpers = pure_helpers.len();
    RuntimePlan::new(Some(entry_flow.clone()), vec![lowered], Vec::new())
        .map(|plan| {
            let plan = optimizer::finalize_runtime_plan(
                plan.with_entries(vec![RuntimeEntrySpec {
                    id: entry_id,
                    kind: RuntimeEntryKind::Custom("agent_controller".to_owned()),
                    target: RuntimeEntryTarget::Flow(entry_flow),
                }])
                .with_pure_helpers(pure_helpers),
                &mut stats,
            );
            RuntimePlanLowerReport {
                plan,
                stats,
                line_display_catalog: LineDisplayCatalog::default(),
                closure_captures: options.closure_captures.clone(),
            }
        })
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])
}

fn lower_runtime_entries(module: &HirModule) -> Vec<RuntimeEntrySpec> {
    module
        .declarations()
        .iter()
        .filter_map(|decl| match decl {
            HirTopLevelDecl::Entry(entry) => Some(RuntimeEntrySpec {
                id: EntryRuntimeId::from_source_entity_body(entry.id().body())
                    .expect("HIR entry ID should be a valid runtime entry ID"),
                kind: lower_entry_kind(entry.kind()),
                target: lower_entry_target(entry.items()),
            }),
            _ => None,
        })
        .collect()
}

fn lower_entry_kind(kind: &EntryKind) -> RuntimeEntryKind {
    match kind {
        EntryKind::Game => RuntimeEntryKind::Game,
        EntryKind::Cli => RuntimeEntryKind::Cli,
        EntryKind::Server => RuntimeEntryKind::Server,
        EntryKind::Activity => RuntimeEntryKind::Activity,
        EntryKind::Test => RuntimeEntryKind::Test,
        EntryKind::Bench => RuntimeEntryKind::Bench,
        EntryKind::Custom(value) => RuntimeEntryKind::Custom(value.clone()),
    }
}

fn lower_entry_target(items: &[EntryItem]) -> RuntimeEntryTarget {
    let routes = items
        .iter()
        .filter_map(|item| match item {
            EntryItem::Route {
                method,
                path,
                target,
                bindings,
            } => Some(RuntimeRouteSpec {
                method: method.clone(),
                path: path.clone(),
                target: flow_runtime_id(target),
                bindings: bindings
                    .iter()
                    .map(|binding| RuntimeRouteBinding {
                        name: binding.name().to_owned(),
                        source: match binding.source() {
                            arcweft_lang_hir::syntax::ast::items::EntryRouteBindingSource::PathParam(name) => {
                                RuntimeRouteBindingSource::PathParam(name.clone())
                            }
                        },
                    })
                    .collect(),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !routes.is_empty() {
        return RuntimeEntryTarget::Routes(routes);
    }
    items
        .iter()
        .find_map(|item| match item {
            EntryItem::Goto(target) => Some(RuntimeEntryTarget::Flow(flow_runtime_id(target))),
            EntryItem::Route { .. } | EntryItem::Option { .. } | EntryItem::Raw(_) => None,
        })
        .unwrap_or_else(|| RuntimeEntryTarget::Routes(Vec::new()))
}

fn implicit_entry_flow(
    entries: &[RuntimeEntrySpec],
    flows: &[RuntimeFlow],
) -> Option<FlowRuntimeId> {
    if entries.len() == 1
        && let Some(flow) = match &entries[0].target {
            RuntimeEntryTarget::Flow(flow) => Some(flow),
            RuntimeEntryTarget::Routes(routes) => routes.first().map(|route| &route.target),
        }
    {
        return Some(flow.clone());
    }
    if entries.is_empty() {
        return flows.first().map(|flow| flow.id.clone());
    }
    None
}

/// Lowers HIR flow bodies into executable Sans I/O flow operations.
pub(crate) fn lower_runtime_flows(
    module: &HirModule,
    pure_helpers: RuntimePureHelperLookup<'_, '_, 'static>,
    options: &RuntimePlanLowerOptions,
) -> Result<LoweredRuntimeFlows, Vec<RuntimePlanLowerError>> {
    let fx = FxCatalog::try_from_module_for_package(module, options.package_identity())
        .map_err(|error| vec![error])?;
    let display_defaults = DialogueDisplayDefaults::try_from_module_with_selection(
        module,
        options.dialogue_defaults(),
    )
    .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    let mut lowerer = FlowRuntimeLowerer {
        module,
        agent_controller: false,
        line_task_groups: Vec::new(),
        line_display_catalog: LineDisplayCatalog::default(),
        display_defaults,
        fx,
        speaker_preset_scopes: Vec::new(),
        presentation_handle_scopes: Vec::new(),
        function_local_scopes: Vec::new(),
        current_location: ExecutableLoweringLocation::in_module("flow `<pending>`", module, None),
        errors: Vec::new(),
        pure_helpers,
        for_iteration_evidence: options.for_iteration_evidence(),
        for_iteration_cursor: 0,
    };
    let flows = module
        .flows()
        .iter()
        .enumerate()
        .map(|(index, flow)| lowerer.lower_flow(index, flow))
        .collect::<Vec<_>>();
    if !module.top_level_items().is_empty() {
        lowerer.errors.push(RuntimePlanLowerError::new(
            "top-level flow items are not executable by the flow runtime yet",
        ));
    }
    if lowerer.errors.is_empty() {
        Ok(LoweredRuntimeFlows {
            flows,
            line_task_groups: lowerer.line_task_groups,
            line_display_catalog: lowerer.line_display_catalog,
        })
    } else {
        Err(lowerer.errors)
    }
}

fn lower_agent_controller_flow(
    module: &HirModule,
    agent: &HirAgent,
    pure_helpers: RuntimePureHelperLookup<'_, '_, 'static>,
    options: &RuntimePlanLowerOptions,
) -> Result<RuntimeFlow, Vec<RuntimePlanLowerError>> {
    let fx = FxCatalog::try_from_module_for_package(module, options.package_identity())
        .map_err(|error| vec![error])?;
    let display_defaults = DialogueDisplayDefaults::try_from_module_with_selection(
        module,
        options.dialogue_defaults(),
    )
    .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    let mut lowerer = FlowRuntimeLowerer {
        module,
        agent_controller: true,
        line_task_groups: Vec::new(),
        line_display_catalog: LineDisplayCatalog::default(),
        display_defaults,
        fx,
        speaker_preset_scopes: Vec::new(),
        presentation_handle_scopes: Vec::new(),
        function_local_scopes: Vec::new(),
        current_location: ExecutableLoweringLocation::in_module(
            "agent `<pending>`",
            module,
            agent.module_path(),
        ),
        errors: Vec::new(),
        pure_helpers,
        for_iteration_evidence: &[],
        for_iteration_cursor: 0,
    };
    let id = agent.item().id().map_or_else(
        || {
            FlowRuntimeId::canonical(&format!("agent.{}", agent.item().name()))
                .expect("generated agent flow ID is valid")
        },
        flow_runtime_id,
    );
    lowerer.current_location = ExecutableLoweringLocation::in_module(
        format!("agent flow `{}`", id.canonical_label()),
        module,
        agent.module_path(),
    );
    let mut ops = lowerer.lower_flow_stmt_list(&id, 0, agent.item().body_statements());
    if let Some(value) = agent.item().body_value() {
        if let Some(mut host_ops) = lowerer.lower_agent_host_call_expr(value.expr(), value.range())
        {
            ops.append(&mut host_ops);
        } else {
            ops.push(FlowOp::ReturnExpr(lowerer.lower_runtime_expr(value.expr())));
        }
    }
    if lowerer.errors.is_empty() {
        Ok(RuntimeFlow { id, ops })
    } else {
        Err(lowerer.errors)
    }
}

struct FlowRuntimeLowerer<'hir, 'helpers, 'functions, 'evidence> {
    module: &'hir HirModule,
    agent_controller: bool,
    line_task_groups: Vec<LineTaskGroup>,
    line_display_catalog: LineDisplayCatalog,
    display_defaults: DialogueDisplayDefaults,
    fx: FxCatalog,
    speaker_preset_scopes: Vec<BTreeMap<String, DialogueSpeakerPreset>>,
    presentation_handle_scopes: Vec<BTreeMap<String, PresentationHandleBinding>>,
    function_local_scopes: Vec<BTreeMap<String, usize>>,
    current_location: ExecutableLoweringLocation<'hir>,
    errors: Vec<RuntimePlanLowerError>,
    pure_helpers: RuntimePureHelperLookup<'helpers, 'functions, 'static>,
    for_iteration_evidence: &'evidence [RuntimeIteratorEvidence],
    for_iteration_cursor: usize,
}

#[derive(Clone)]
struct PresentationHandleBinding {
    handle_id: String,
    kind: &'static str,
}

fn runtime_expr_function_arity(
    expr: &RuntimeExpr,
    function_locals: &BTreeMap<String, usize>,
) -> Option<usize> {
    match expr {
        RuntimeExpr::Function { params, .. } => Some(params.len()),
        RuntimeExpr::Local(name) => function_locals.get(name).copied(),
        RuntimeExpr::Apply { callee, args } => {
            let arity = runtime_expr_function_arity(callee, function_locals)?;
            arity
                .checked_sub(args.len())
                .filter(|remaining| *remaining > 0)
        }
        _ => None,
    }
}

impl FlowRuntimeLowerer<'_, '_, '_, '_> {
    fn lower_runtime_pattern(
        &mut self,
        pattern: &Pattern,
        statement_kind: &'static str,
        role: &'static str,
        source_range: Option<arcweft_lang_hir::syntax::ast::common::TextRange>,
    ) -> RuntimePattern {
        match lower_runtime_pattern_checked(pattern) {
            Ok(pattern) => pattern,
            Err(reason) => {
                self.errors.push(self.current_location.named_pattern_error(
                    statement_kind,
                    role,
                    source_range,
                    reason,
                ));
                RuntimePattern::Discard
            }
        }
    }

    fn lower_stmt_pattern(
        &mut self,
        statement: &Stmt,
        pattern: &Pattern,
        role: &'static str,
    ) -> RuntimePattern {
        match lower_runtime_pattern_checked(pattern) {
            Ok(pattern) => pattern,
            Err(reason) => {
                self.errors
                    .push(self.current_location.pattern_error(statement, role, reason));
                RuntimePattern::Discard
            }
        }
    }

    fn lower_runtime_expr_result(&self, expr: &Expr) -> Result<RuntimeExpr, String> {
        let function_locals = self.active_function_locals();
        let context = self.pure_helpers.with_function_locals(&function_locals);
        lower_runtime_expr_strict_with_pure(expr, context)
    }

    fn lower_runtime_expr(&mut self, expr: &Expr) -> RuntimeExpr {
        match self.lower_runtime_expr_result(expr) {
            Ok(expr) => expr,
            Err(message) => {
                self.errors.push(RuntimePlanLowerError::new(message));
                RuntimeExpr::Value(RuntimeValue::Unit)
            }
        }
    }

    fn lower_runtime_expr_with_expected_type(
        &mut self,
        expected_ty: Option<&TypeRef>,
        expr: &Expr,
    ) -> RuntimeExpr {
        let function_locals = self.active_function_locals();
        let context = self.pure_helpers.with_function_locals(&function_locals);
        match lower_runtime_expr_strict_with_expected_type(expr, expected_ty, context) {
            Ok(expr) => expr,
            Err(message) => {
                self.errors.push(RuntimePlanLowerError::new(message));
                RuntimeExpr::Value(RuntimeValue::Unit)
            }
        }
    }

    fn lower_optional_runtime_expr(&mut self, expr: Option<&Expr>) -> Option<RuntimeExpr> {
        expr.map(|expr| self.lower_runtime_expr(expr))
    }

    fn active_function_locals(&self) -> BTreeMap<String, usize> {
        self.function_local_scopes
            .iter()
            .flat_map(|scope| scope.iter().map(|(name, arity)| (name.clone(), *arity)))
            .collect()
    }

    fn record_function_local_binding(&mut self, pattern: &Pattern, arity: Option<usize>) {
        let Some(name) = pattern.simple_binding_name() else {
            return;
        };
        let Some(scope) = self.function_local_scopes.last_mut() else {
            return;
        };
        if let Some(arity) = arity {
            scope.insert(name.to_owned(), arity);
        } else {
            scope.remove(name);
        }
    }

    fn runtime_expr_function_arity(&self, expr: &RuntimeExpr) -> Option<usize> {
        let function_locals = self.active_function_locals();
        runtime_expr_function_arity(expr, &function_locals)
    }

    fn next_for_iteration_evidence(&mut self) -> Option<RuntimeIteratorEvidence> {
        let evidence = self
            .for_iteration_evidence
            .get(self.for_iteration_cursor)
            .cloned();
        self.for_iteration_cursor = self.for_iteration_cursor.saturating_add(1);
        evidence
    }

    fn lower_flow(&mut self, index: usize, flow: &HirFlow) -> RuntimeFlow {
        let id = flow.id().map_or_else(
            || {
                FlowRuntimeId::canonical(flow.name().unwrap_or("anonymous"))
                    .expect("generated flow ID is valid")
            },
            flow_runtime_id,
        );
        self.current_location = ExecutableLoweringLocation::in_module(
            format!("flow `{}`", id.canonical_label()),
            self.module,
            flow.module_path(),
        );
        let ops = self.lower_flow_items(&id, flow.body(), index);
        RuntimeFlow { id, ops }
    }

    fn lower_flow_items(
        &mut self,
        flow_id: &FlowRuntimeId,
        items: &[HirFlowItem],
        flow_index: usize,
    ) -> Vec<FlowOp> {
        let parent_location = self.current_location.clone();
        self.speaker_preset_scopes.push(BTreeMap::new());
        self.presentation_handle_scopes.push(BTreeMap::new());
        self.function_local_scopes.push(BTreeMap::new());
        let ops = self.lower_flow_items_in_scope(flow_id, items, flow_index);
        self.function_local_scopes.pop();
        self.presentation_handle_scopes.pop();
        self.speaker_preset_scopes.pop();
        self.current_location = parent_location;
        ops
    }

    fn lower_flow_items_in_scope(
        &mut self,
        flow_id: &FlowRuntimeId,
        items: &[HirFlowItem],
        flow_index: usize,
    ) -> Vec<FlowOp> {
        let mut ops = Vec::new();
        let parent_location = self.current_location.clone();
        for (index, item) in items.iter().enumerate() {
            self.current_location = parent_location.statement(index);
            ops.extend(self.lower_flow_item(flow_id, item, flow_index));
        }
        self.current_location = parent_location;
        ops
    }

    fn lower_flow_item(
        &mut self,
        flow_id: &FlowRuntimeId,
        item: &HirFlowItem,
        flow_index: usize,
    ) -> Vec<FlowOp> {
        match item {
            HirFlowItem::Dialogue(dialogue) => {
                vec![self.lower_runtime_dialogue(flow_id, flow_index, dialogue)]
            }
            HirFlowItem::Choice(choice) | HirFlowItem::LetChoice { choice, .. } => {
                vec![self.lower_choice(choice)]
            }
            HirFlowItem::Await(await_with) => vec![self.lower_await(None, await_with)],
            HirFlowItem::LetAwait {
                pattern,
                await_with,
                ..
            } => vec![self.lower_await(Some(pattern), await_with)],
            HirFlowItem::Stmt(stmt) => {
                self.register_speaker_preset(stmt);
                self.lower_flow_stmt(flow_id, flow_index, stmt)
            }
            HirFlowItem::Thread(thread) => self.lower_hir_thread(thread, flow_id, flow_index),
            HirFlowItem::Scope(scope) => vec![FlowOp::Scope(self.lower_flow_items(
                flow_id,
                scope.body(),
                flow_index,
            ))],
            HirFlowItem::LetScope { pattern, scope } => {
                vec![self.lower_scope_expr(flow_id, flow_index, pattern, scope)]
            }
            other => self.lower_control_flow_item(flow_id, other, flow_index),
        }
    }

    fn lower_control_flow_item(
        &mut self,
        flow_id: &FlowRuntimeId,
        item: &HirFlowItem,
        flow_index: usize,
    ) -> Vec<FlowOp> {
        match item {
            HirFlowItem::If(block) => vec![FlowOp::If {
                condition: self.lower_runtime_expr(block.condition()),
                then_ops: self.lower_flow_items(flow_id, block.body(), flow_index),
                else_ops: self.lower_flow_items(flow_id, block.else_body(), flow_index),
            }],
            HirFlowItem::IfLet(block) => {
                let pattern = self.lower_runtime_pattern(
                    block.pattern(),
                    "if-let",
                    "binding",
                    block.expr_authored().range(),
                );
                vec![FlowOp::IfLet {
                    pattern,
                    expr: self.lower_runtime_expr(block.expr()),
                    guard: self.lower_optional_runtime_expr(block.guard()),
                    then_ops: self.lower_flow_items(flow_id, block.body(), flow_index),
                    else_ops: self.lower_flow_items(flow_id, block.else_body(), flow_index),
                }]
            }
            HirFlowItem::Match(block) => {
                vec![self.lower_match_block(flow_id, block, flow_index)]
            }
            HirFlowItem::Loop(block) => vec![FlowOp::Loop {
                body: self.lower_flow_items(flow_id, block.body(), flow_index),
            }],
            HirFlowItem::LetLoop { pattern, block } => {
                vec![self.lower_loop_expr(flow_id, pattern, block, flow_index)]
            }
            HirFlowItem::While(block) => vec![FlowOp::While {
                condition: self.lower_runtime_expr(block.condition()),
                body: self.lower_flow_items(flow_id, block.body(), flow_index),
            }],
            HirFlowItem::WhileLet(block) => {
                let pattern = self.lower_runtime_pattern(
                    block.pattern(),
                    "while-let",
                    "binding",
                    block.expr_authored().range(),
                );
                vec![FlowOp::WhileLet {
                    pattern,
                    expr: self.lower_runtime_expr(block.expr()),
                    guard: self.lower_optional_runtime_expr(block.guard()),
                    body: self.lower_flow_items(flow_id, block.body(), flow_index),
                }]
            }
            HirFlowItem::For(block) => self.lower_for_flow_item(flow_id, block, flow_index),
            other => {
                self.errors.push(RuntimePlanLowerError::new(format!(
                    "unsupported flow item for runtime lowering: {other:?}"
                )));
                Vec::new()
            }
        }
    }

    fn lower_for_flow_item(
        &mut self,
        flow_id: &FlowRuntimeId,
        block: &HirFor,
        flow_index: usize,
    ) -> Vec<FlowOp> {
        let Some(evidence) = self.next_for_iteration_evidence() else {
            self.errors.push(RuntimePlanLowerError::new(
                "missing trait-resolved IntoIterator evidence for `for` source",
            ));
            return Vec::new();
        };
        let pattern = self.lower_runtime_pattern(
            block.pattern(),
            "for",
            "binding",
            block.source_authored().range(),
        );
        vec![FlowOp::For {
            pattern,
            source: self.lower_runtime_expr(block.source()),
            evidence,
            body: self.lower_flow_items(flow_id, block.body(), flow_index),
        }]
    }

    fn register_speaker_preset(&mut self, stmt: &Stmt) {
        let Stmt::Let {
            pattern,
            expr,
            expr_source,
            expr_range,
            ..
        } = stmt
        else {
            return;
        };
        let Some((name, preset)) =
            speaker_preset_from_let(pattern, expr, expr_source.as_deref(), expr_range.as_ref())
        else {
            return;
        };
        if let Some(scope) = self.speaker_preset_scopes.last_mut() {
            scope.insert(name, preset);
        }
    }

    fn active_speaker_presets(&self) -> Vec<DialogueSpeakerPreset> {
        self.speaker_preset_scopes
            .iter()
            .flat_map(|scope| scope.values().cloned())
            .collect()
    }

    fn lower_scope_expr(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        pattern: &Pattern,
        scope: &HirScopeExpr,
    ) -> FlowOp {
        let (op, arity) = self.lower_value_scope_op(
            flow_id,
            flow_index,
            pattern,
            None,
            FlowValueBlock::new(scope.statements(), scope.value()),
        );
        self.record_function_local_binding(pattern, arity);
        op
    }

    fn lower_match_block(
        &mut self,
        flow_id: &FlowRuntimeId,
        block: &HirMatch,
        flow_index: usize,
    ) -> FlowOp {
        FlowOp::Match {
            scrutinee: self.lower_runtime_expr(block.expr()),
            arms: block
                .arms()
                .iter()
                .map(|arm| RuntimeMatchArm {
                    pattern: self.lower_runtime_pattern(
                        arm.pattern(),
                        "match",
                        "arm",
                        block.expr_authored().range(),
                    ),
                    guard: self.lower_optional_runtime_expr(arm.guard()),
                    ops: self.lower_flow_items(flow_id, arm.body(), flow_index),
                })
                .collect(),
        }
    }

    fn lower_loop_expr(
        &mut self,
        flow_id: &FlowRuntimeId,
        pattern: &Pattern,
        block: &HirLoop,
        flow_index: usize,
    ) -> FlowOp {
        FlowOp::LetLoop {
            pattern: self.lower_runtime_pattern(pattern, "let-loop", "binding", None),
            body: self.lower_flow_items(flow_id, block.body(), flow_index),
        }
    }

    fn lower_runtime_dialogue(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        dialogue: &HirDialogue,
    ) -> FlowOp {
        let group = if let Some(plan) = dialogue.plan() {
            match lower_line_plan(plan) {
                Ok(group) => group,
                Err(errors) => {
                    self.push_line_errors(errors);
                    LineTaskGroup::default()
                }
            }
        } else {
            LineTaskGroup::default()
        };
        let task_group = self.line_task_groups.len();
        self.line_task_groups.push(group);
        let line = dialogue.id().map_or_else(
            || {
                RuntimeLineId::canonical(&format!(
                    "{}.dialogue.{task_group}",
                    flow_id.canonical_label()
                ))
                .expect("generated dialogue line ID is valid")
            },
            |id| {
                RuntimeLineId::from_runtime_line_value(id.body())
                    .expect("HIR dialogue line ID should be valid")
            },
        );
        let active_speaker_presets = self.active_speaker_presets();
        match lower_dialogue_display_with_speaker_presets_and_fx(
            line.clone(),
            dialogue,
            &self.display_defaults,
            &active_speaker_presets,
            &self.fx,
        ) {
            Ok(display) => self.line_display_catalog.push(display),
            Err(error) => self.errors.push(error),
        }
        let _ = flow_index;
        FlowOp::Dialogue { line, task_group }
    }

    fn lower_choice(&mut self, choice: &HirChoice) -> FlowOp {
        FlowOp::Choice {
            id: choice.id().map(|id| id.body().to_owned()),
            options: choice
                .options()
                .iter()
                .map(|option| self.lower_choice_option(option))
                .collect(),
        }
    }

    fn lower_choice_option(&mut self, option: &HirChoiceOption) -> ChoiceRuntimeOption {
        let mut effects = Vec::new();
        let mut out = None;
        let mut target = option.target().map(flow_runtime_id);
        match option.action() {
            ChoiceAction::Goto(target_ref) => {
                if let EntityRefSyntax::Absolute(target_ref) = target_ref {
                    target = Some(flow_runtime_id(target_ref));
                }
            }
            ChoiceAction::Out(expr) => {
                out = Some(LineOutRequest {
                    label: None,
                    value: expr_label(expr),
                });
            }
            ChoiceAction::SelectBlock(statements) => {
                effects.extend(self.lower_flow_statements(statements));
            }
            ChoiceAction::None => {}
        }
        ChoiceRuntimeOption {
            id: option.id().map(|id| id.body().to_owned()),
            label: option.label().to_owned(),
            target,
            out,
            effects,
        }
    }

    fn lower_await(&mut self, binding: Option<&Pattern>, await_with: &HirAwait) -> FlowOp {
        let label = expr_label(await_with.expr());
        let task_name = sanitize_task_id_part(&label);
        let source_range = await_with.expr_authored().range();
        let binding = binding
            .map(|pattern| self.lower_runtime_pattern(pattern, "await", "binding", source_range));
        let pending = await_with
            .branches()
            .iter()
            .filter(|branch| branch.kind() == AwaitBranchKind::Pending)
            .flat_map(|branch| self.lower_pending_flow_items(branch.body()))
            .collect();
        match self.lower_await_many_target(await_with.expr(), &task_name) {
            Ok(Some(target)) => {
                return FlowOp::AwaitMany {
                    binding,
                    target,
                    pending,
                };
            }
            Ok(None) => {}
            Err(message) => {
                self.errors
                    .push(self.current_location.named_expression_error(
                        "await",
                        "target",
                        source_range,
                        message,
                    ));
                return FlowOp::Noop;
            }
        }
        let request = match lower_host_task_request(await_with.expr()) {
            Ok(request) => request,
            Err(error) => {
                let error = error.into_runtime_error(
                    self.current_location.owner(),
                    self.current_location.path().to_vec(),
                    source_range,
                );
                self.errors.push(self.current_location.bind_error(error));
                return FlowOp::Noop;
            }
        };
        FlowOp::Await {
            binding,
            target: AwaitTarget::new(
                NeedId(format!("need.await.{task_name}")),
                TaskId(format!("task.await.{task_name}")),
                request,
            ),
            pending,
        }
    }

    fn lower_await_many_target(
        &mut self,
        expr: &Expr,
        task_name: &str,
    ) -> Result<Option<AwaitManyTarget>, String> {
        let Some((parallel_receiver, method, parallel_args)) = selected_call_parts(expr) else {
            return Ok(None);
        };
        if method_name(method) != "parallel" {
            return Ok(None);
        }
        let Some((source, traverse_method, traverse_args)) = selected_call_parts(parallel_receiver)
        else {
            return Err(
                "parallel await requires `source.traverse(call).parallel(limit = N)`".to_owned(),
            );
        };
        if method_name(traverse_method) != "traverse" {
            return Err(
                "parallel await requires `traverse(...)` before `.parallel(...)`".to_owned(),
            );
        }
        let callee = traverse_callee(traverse_args)?;
        let (capability, operation) = split_capability_operation(&expr_label(callee))?;
        let limit = parallel_limit(parallel_args)?;
        Ok(Some(AwaitManyTarget::new(
            NeedId(format!("need.await_many.{task_name}")),
            TaskId(format!("task.await_many.{task_name}")),
            self.lower_runtime_expr(source),
            AWAIT_MANY_ITEM_BINDING,
            limit,
            HostTaskRequestTemplate::new(
                capability,
                operation,
                [HostTaskArgTemplate::positional(RuntimeExpr::Local(
                    AWAIT_MANY_ITEM_BINDING.to_owned(),
                ))],
            ),
        )))
    }

    fn lower_pending_flow_items(&mut self, items: &[HirFlowItem]) -> Vec<LineEffectRequest> {
        items
            .iter()
            .flat_map(|item| match item {
                HirFlowItem::Stmt(stmt) => self.lower_flow_statements(std::slice::from_ref(stmt)),
                other => {
                    self.errors.push(RuntimePlanLowerError::new(format!(
                        "unsupported await pending item for runtime lowering: {other:?}"
                    )));
                    Vec::new()
                }
            })
            .collect()
    }

    fn lower_flow_stmt(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        stmt: &Stmt,
    ) -> Vec<FlowOp> {
        if let Some(binding) = self.lower_binding_flow_stmt(flow_id, flow_index, stmt) {
            return binding;
        }
        match stmt {
            Stmt::Goto(expr) => vec![FlowOp::GotoExpr(self.lower_runtime_expr(expr.expr()))],
            Stmt::Return { expr, .. } => {
                vec![FlowOp::ReturnExpr(self.lower_runtime_expr(expr))]
            }
            Stmt::Assign { target, expr } => self
                .lower_assignment_stmt(target.expr(), expr.expr())
                .map_or_else(Vec::new, |expr| {
                    vec![FlowOp::Let {
                        pattern: RuntimePattern::Discard,
                        expr,
                    }]
                }),
            Stmt::Expr {
                expr, expr_range, ..
            } => self.lower_effect_statement(flow_id, stmt, expr, *expr_range),
            Stmt::Out { label, expr } => {
                vec![FlowOp::Effect(LineEffectRequest::Out(LineOutRequest {
                    label: label.clone(),
                    value: expr_label(expr.expr()),
                }))]
            }
            Stmt::If {
                condition,
                body,
                else_body,
            } => self.lower_if_stmt(flow_id, flow_index, condition.expr(), body, else_body),
            Stmt::Loop { body } => vec![FlowOp::Loop {
                body: self.lower_flow_stmt_list(flow_id, flow_index, body),
            }],
            Stmt::While { condition, body } => vec![FlowOp::While {
                condition: self.lower_runtime_expr(condition.expr()),
                body: self.lower_flow_stmt_list(flow_id, flow_index, body),
            }],
            Stmt::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => {
                let pattern = self.lower_stmt_pattern(stmt, pattern, "binding");
                vec![FlowOp::WhileLet {
                    pattern,
                    expr: self.lower_runtime_expr(expr.expr()),
                    guard: self.lower_optional_runtime_expr(guard.as_ref().map(AuthoredExpr::expr)),
                    body: self.lower_flow_stmt_list(flow_id, flow_index, body),
                }]
            }
            Stmt::For {
                pattern,
                source,
                body,
            } => self.lower_for_stmt(flow_id, flow_index, stmt, pattern, source.expr(), body),
            Stmt::Thread(thread) => self.lower_thread_stmt(flow_id, flow_index, thread),
            Stmt::Match { expr, arms } => vec![FlowOp::Match {
                scrutinee: self.lower_runtime_expr(expr.expr()),
                arms: self.lower_stmt_match_arms(flow_id, flow_index, arms),
            }],
            Stmt::Break { expr, .. } => {
                vec![FlowOp::Break(self.lower_optional_runtime_expr(
                    expr.as_ref().map(AuthoredExpr::expr),
                ))]
            }
            Stmt::Continue { .. } => vec![FlowOp::Continue],
            other => {
                self.errors.push(RuntimePlanLowerError::new(format!(
                    "unsupported flow statement for runtime lowering: {other:?}"
                )));
                Vec::new()
            }
        }
    }

    fn lower_binding_flow_stmt(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        stmt: &Stmt,
    ) -> Option<Vec<FlowOp>> {
        match stmt {
            Stmt::Let {
                pattern,
                ty,
                expr,
                expr_range,
                ..
            } => Some(self.lower_let_stmt(
                flow_id,
                flow_index,
                pattern,
                ty.as_ref(),
                expr,
                *expr_range,
            )),
            Stmt::LetScope { pattern, scope } => {
                Some(self.lower_let_scope_stmt(flow_id, flow_index, pattern, scope))
            }
            Stmt::LetLoop { pattern, block } => {
                let pattern = self.lower_stmt_pattern(stmt, pattern, "binding");
                Some(vec![FlowOp::LetLoop {
                    pattern,
                    body: self.lower_syntax_flow_items(flow_id, flow_index, block.body()),
                }])
            }
            Stmt::LetActionReceive { pattern, action } => {
                let pattern = self.lower_stmt_pattern(stmt, pattern, "binding");
                Some(self.lower_action_receive_stmt(pattern, action.expr()))
            }
            Stmt::LetElse {
                pattern,
                ty,
                expr,
                else_body,
            } => {
                let pattern = self.lower_stmt_pattern(stmt, pattern, "binding");
                Some(vec![FlowOp::LetElse {
                    pattern,
                    expr: self.lower_runtime_expr_with_expected_type(ty.as_ref(), expr.expr()),
                    else_ops: self.lower_flow_stmt_list(flow_id, flow_index, else_body),
                }])
            }
            _ => None,
        }
    }

    fn lower_let_scope_stmt(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        pattern: &Pattern,
        scope: &ScopeExprBlock,
    ) -> Vec<FlowOp> {
        let (op, arity) = self.lower_value_scope_op(
            flow_id,
            flow_index,
            pattern,
            None,
            FlowValueBlock::new(scope.statements(), scope.value()),
        );
        self.record_function_local_binding(pattern, arity);
        vec![op]
    }

    fn lower_effect_statement(
        &mut self,
        flow_id: &FlowRuntimeId,
        statement: &Stmt,
        expr: &Expr,
        source_range: Option<arcweft_lang_hir::syntax::ast::common::TextRange>,
    ) -> Vec<FlowOp> {
        if let Some(ops) = self.lower_presentation_handle_method(expr) {
            return ops;
        }
        if let Some(ops) = Self::lower_explicit_presentation_mount(flow_id, expr) {
            return ops;
        }
        if let Some(ops) = self.lower_agent_host_call_expr(expr, source_range) {
            return ops;
        }
        let function_locals = self.active_function_locals();
        let helpers = self.pure_helpers.with_function_locals(&function_locals);
        match lower_runtime_effect_strict_with_pure(expr, helpers) {
            Ok(LoweredRuntimeEffect::Static(effect)) => vec![FlowOp::Effect(effect)],
            Ok(LoweredRuntimeEffect::Evaluated(effect)) => vec![FlowOp::EvaluatedEffect(effect)],
            Err(reason) => {
                self.errors.push(self.current_location.expression_error(
                    statement,
                    "effect",
                    source_range,
                    reason,
                ));
                Vec::new()
            }
        }
    }

    fn lower_action_receive_stmt(&mut self, pattern: RuntimePattern, action: &Expr) -> Vec<FlowOp> {
        vec![FlowOp::HostCall {
            binding: Some(pattern),
            target: RuntimeHostCallTarget::new(
                "view.action.await",
                "view.action",
                "await",
                [self.lower_runtime_expr(action)],
                RuntimeHostCallMode::Suspend,
                true,
            ),
        }]
    }

    fn lower_presentation_handle_let(
        &mut self,
        flow_id: &FlowRuntimeId,
        pattern: &Pattern,
        expr: &Expr,
    ) -> Option<Vec<FlowOp>> {
        let mount = presentation_mount_call(expr)?;
        let Some(binding) = pattern.simple_binding_name() else {
            self.errors.push(RuntimePlanLowerError::new(
                "value-position presentation handles require a simple binding pattern",
            ));
            return Some(Vec::new());
        };
        let handle_id = presentation_handle_id(flow_id, binding);
        self.register_presentation_handle_binding(binding, handle_id.clone(), mount.kind);
        let create = presentation_handle_call(
            "create",
            presentation_create_args(&handle_id, flow_id, mount.kind, mount.resource, mount.args),
        );
        let cleanup = presentation_handle_call("dispose", vec![format!("handle = @{handle_id}")]);
        let mut ops = vec![FlowOp::Effect(LineEffectRequest::Call(create))];
        if mount.register_scope_cleanup {
            ops.push(FlowOp::RegisterCleanup {
                key: handle_id.clone(),
                effect: LineEffectRequest::Call(cleanup),
            });
        }
        ops.push(FlowOp::Let {
            pattern: self.lower_runtime_pattern(pattern, "let", "binding", None),
            expr: RuntimeExpr::Value(RuntimeValue::String(handle_id)),
        });
        Some(ops)
    }

    fn lower_explicit_presentation_mount(
        flow_id: &FlowRuntimeId,
        expr: &Expr,
    ) -> Option<Vec<FlowOp>> {
        let mount = presentation_mount_call(expr)?;
        let handle_id = presentation_explicit_mount_handle_id(flow_id, mount.kind, mount.resource);
        let create = presentation_handle_call(
            "create",
            presentation_create_args(&handle_id, flow_id, mount.kind, mount.resource, mount.args),
        );
        let mut ops = vec![FlowOp::Effect(LineEffectRequest::Call(create))];
        if mount.register_scope_cleanup {
            ops.push(FlowOp::RegisterCleanup {
                key: handle_id.clone(),
                effect: LineEffectRequest::Call(presentation_handle_call(
                    "dispose",
                    vec![format!("handle = @{handle_id}")],
                )),
            });
        }
        Some(ops)
    }

    fn lower_presentation_handle_method(&mut self, expr: &Expr) -> Option<Vec<FlowOp>> {
        let (receiver, method, args) = selected_call_parts(expr)?;
        if !args.is_empty() {
            return None;
        }
        let binding = self.presentation_handle_binding_for_receiver(receiver)?;
        let operation = match method {
            "show" => "show",
            "hide" => "hide",
            "unmount" => "unmount",
            "release" => "release",
            "destroy" => "destroy",
            "pop" if binding.kind == "overlay" => "dispose",
            _ => return None,
        };
        let mut ops = vec![FlowOp::Effect(LineEffectRequest::Call(
            presentation_handle_call(operation, vec![format!("handle = @{}", binding.handle_id)]),
        ))];
        if matches!(operation, "dispose" | "release" | "destroy") {
            ops.push(FlowOp::CancelCleanup {
                key: binding.handle_id,
            });
        }
        Some(ops)
    }

    fn register_presentation_handle_binding(
        &mut self,
        binding: &str,
        handle_id: String,
        kind: &'static str,
    ) {
        if let Some(scope) = self.presentation_handle_scopes.last_mut() {
            scope.insert(
                binding.to_owned(),
                PresentationHandleBinding { handle_id, kind },
            );
        }
    }

    fn presentation_handle_binding_for_receiver(
        &self,
        receiver: &Expr,
    ) -> Option<PresentationHandleBinding> {
        let Expr::Path(path) = receiver else {
            return None;
        };
        let [segment] = path.segments() else {
            return None;
        };
        let binding = segment.as_str();
        self.presentation_handle_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(binding).cloned())
    }

    fn lower_assignment_stmt(&mut self, target: &Expr, expr: &Expr) -> Option<RuntimeExpr> {
        let Expr::Select(select) = target else {
            self.errors.push(RuntimePlanLowerError::new(format!(
                "unsupported flow assignment target `{}`: only direct record fields are executable",
                expr_label(target)
            )));
            return None;
        };
        let receiver = match self.lower_runtime_expr_result(select.target()) {
            Ok(RuntimeExpr::Local(name)) => RuntimeExpr::Local(name),
            Ok(other) => {
                self.errors.push(RuntimePlanLowerError::new(format!(
                    "unsupported flow assignment receiver `{other}`: assignment requires a local record value"
                )));
                return None;
            }
            Err(reason) => {
                self.errors.push(RuntimePlanLowerError::new(reason));
                return None;
            }
        };
        let expr = match self.lower_runtime_expr_result(expr) {
            Ok(expr) => expr,
            Err(reason) => {
                self.errors.push(RuntimePlanLowerError::new(reason));
                return None;
            }
        };
        Some(RuntimeExpr::AssignField {
            target: Box::new(receiver),
            field: select.member().as_str().to_owned(),
            expr: Box::new(expr),
            body: Box::new(RuntimeExpr::Value(RuntimeValue::Unit)),
        })
    }

    fn lower_if_stmt(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        condition: &Expr,
        body: &[Stmt],
        else_body: &[Stmt],
    ) -> Vec<FlowOp> {
        vec![FlowOp::If {
            condition: self.lower_runtime_expr(condition),
            then_ops: self.lower_flow_stmt_list(flow_id, flow_index, body),
            else_ops: self.lower_flow_stmt_list(flow_id, flow_index, else_body),
        }]
    }

    fn lower_for_stmt(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        statement: &Stmt,
        pattern: &Pattern,
        source: &Expr,
        body: &[Stmt],
    ) -> Vec<FlowOp> {
        let Some(evidence) = self.next_for_iteration_evidence() else {
            self.errors.push(RuntimePlanLowerError::new(
                "missing trait-resolved IntoIterator evidence for `for` source",
            ));
            return Vec::new();
        };
        let pattern = self.lower_stmt_pattern(statement, pattern, "binding");
        vec![FlowOp::For {
            pattern,
            source: self.lower_runtime_expr(source),
            evidence,
            body: self.lower_flow_stmt_list(flow_id, flow_index, body),
        }]
    }

    fn lower_let_stmt(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        pattern: &Pattern,
        ty: Option<&TypeRef>,
        expr: &Expr,
        source_range: Option<arcweft_lang_hir::syntax::ast::common::TextRange>,
    ) -> Vec<FlowOp> {
        let binding = self.lower_let_binding(flow_id, flow_index, pattern, ty, expr, source_range);
        self.record_function_local_binding(pattern, binding.function_arity());
        binding.into_ops()
    }

    fn lower_let_binding(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        pattern: &Pattern,
        ty: Option<&TypeRef>,
        expr: &Expr,
        source_range: Option<arcweft_lang_hir::syntax::ast::common::TextRange>,
    ) -> LoweredLetBinding {
        if let Some(ops) = self.lower_dialogue_result_let(flow_id, flow_index, pattern, expr) {
            return LoweredLetBinding::non_function(ops);
        }
        if let Some(op) = self.lower_agent_host_call_let(pattern, expr, source_range) {
            return LoweredLetBinding::non_function(vec![op]);
        }
        if let Some(ops) = self.lower_presentation_handle_let(flow_id, pattern, expr) {
            return LoweredLetBinding::non_function(ops);
        }
        if let Some(block) = FlowValueBlock::from_expr(expr) {
            return self.lower_value_scope_binding(flow_id, flow_index, pattern, ty, block);
        }
        let expr = self.lower_runtime_expr_with_expected_type(ty, expr);
        let arity = self.runtime_expr_function_arity(&expr);
        LoweredLetBinding::new(
            vec![FlowOp::Let {
                pattern: self.lower_runtime_pattern(pattern, "let", "binding", None),
                expr,
            }],
            arity,
        )
    }

    fn lower_value_scope_binding(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        pattern: &Pattern,
        expected_ty: Option<&TypeRef>,
        block: FlowValueBlock<'_>,
    ) -> LoweredLetBinding {
        let (op, arity) =
            self.lower_value_scope_op(flow_id, flow_index, pattern, expected_ty, block);
        LoweredLetBinding::new(vec![op], arity)
    }

    fn lower_value_scope_op(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        pattern: &Pattern,
        expected_ty: Option<&TypeRef>,
        block: FlowValueBlock<'_>,
    ) -> (FlowOp, Option<usize>) {
        let (ops, value, arity) = self.lower_scoped_statement_value(
            flow_id,
            flow_index,
            block.statements(),
            expected_ty,
            block.value(),
        );
        (
            FlowOp::LetScope {
                pattern: self.lower_runtime_pattern(pattern, "let-scope", "binding", None),
                ops,
                value,
            },
            arity,
        )
    }

    fn lower_scoped_statement_value(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        statements: &[Stmt],
        expected_ty: Option<&TypeRef>,
        value: Option<&Expr>,
    ) -> (Vec<FlowOp>, RuntimeExpr, Option<usize>) {
        self.presentation_handle_scopes.push(BTreeMap::new());
        self.function_local_scopes.push(BTreeMap::new());
        let parent_location = self.current_location.clone();
        let mut ops = Vec::new();
        for (index, statement) in statements.iter().enumerate() {
            self.current_location = parent_location.statement(index);
            ops.extend(self.lower_flow_stmt(flow_id, flow_index, statement));
        }
        self.current_location = parent_location;
        let value = value.map_or(RuntimeExpr::Value(RuntimeValue::Unit), |value| {
            self.lower_runtime_expr_with_expected_type(expected_ty, value)
        });
        let arity = self.runtime_expr_function_arity(&value);
        self.function_local_scopes.pop();
        self.presentation_handle_scopes.pop();
        (ops, value, arity)
    }

    fn lower_stmt_match_arms(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        arms: &[StmtMatchArm],
    ) -> Vec<RuntimeMatchArm> {
        arms.iter()
            .map(|arm| RuntimeMatchArm {
                pattern: self.lower_runtime_pattern(arm.pattern(), "match", "arm", None),
                guard: self.lower_optional_runtime_expr(arm.guard()),
                ops: self.lower_flow_stmt_list(flow_id, flow_index, arm.body()),
            })
            .collect()
    }

    fn lower_thread_stmt(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        thread: &ThreadBlock,
    ) -> Vec<FlowOp> {
        if thread.is_detached() {
            self.errors.push(RuntimePlanLowerError::new(
                "detached flow thread runtime lowering requires a checked detach contract"
                    .to_owned(),
            ));
            Vec::new()
        } else {
            vec![FlowOp::Thread {
                name: thread.name().map(str::to_owned),
                body: self.lower_syntax_flow_items(flow_id, flow_index, thread.body()),
            }]
        }
    }

    fn lower_hir_thread(
        &mut self,
        thread: &HirThread,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
    ) -> Vec<FlowOp> {
        if thread.is_detached() {
            self.errors.push(RuntimePlanLowerError::new(
                "detached flow thread runtime lowering requires a checked detach contract"
                    .to_owned(),
            ));
            Vec::new()
        } else {
            vec![FlowOp::Thread {
                name: thread.name().map(str::to_owned),
                body: self.lower_flow_items(flow_id, thread.body(), flow_index),
            }]
        }
    }

    fn lower_flow_stmt_list(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        statements: &[Stmt],
    ) -> Vec<FlowOp> {
        let (ops, _, _) =
            self.lower_scoped_statement_value(flow_id, flow_index, statements, None, None);
        ops
    }

    fn lower_dialogue_result_let(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        pattern: &Pattern,
        expr: &Expr,
    ) -> Option<Vec<FlowOp>> {
        let (callee, content, plan) = dialogue_call_parts(expr)?;
        let dialogue =
            HirDialogue::expression_call(expr_label(callee), content.clone(), plan.cloned());
        Some(vec![
            self.lower_runtime_dialogue(flow_id, flow_index, &dialogue),
            FlowOp::Let {
                pattern: self.lower_runtime_pattern(pattern, "let", "binding", None),
                expr: self.lower_runtime_expr(expr),
            },
        ])
    }

    fn lower_agent_host_call_let(
        &mut self,
        pattern: &Pattern,
        expr: &Expr,
        source_range: Option<arcweft_lang_hir::syntax::ast::common::TextRange>,
    ) -> Option<FlowOp> {
        if !self.agent_controller {
            return None;
        }
        let request = match lower_agent_host_task_request(expr) {
            Ok(Some(request)) => request,
            Ok(None) => return None,
            Err(error) => {
                let error = error.into_runtime_error(
                    self.current_location.owner(),
                    self.current_location.path().to_vec(),
                    source_range,
                );
                self.errors.push(self.current_location.bind_error(error));
                return Some(FlowOp::Noop);
            }
        };
        let task_name = agent_task_name(expr);
        Some(FlowOp::Await {
            binding: Some(self.lower_runtime_pattern(pattern, "let", "binding", None)),
            target: AwaitTarget::new(
                NeedId(format!("need.agent.{task_name}")),
                TaskId(format!("task.agent.{task_name}")),
                request,
            ),
            pending: Vec::new(),
        })
    }

    fn lower_agent_host_call_expr(
        &mut self,
        expr: &Expr,
        source_range: Option<arcweft_lang_hir::syntax::ast::common::TextRange>,
    ) -> Option<Vec<FlowOp>> {
        if !self.agent_controller {
            return None;
        }
        let request = match lower_agent_host_task_request(expr) {
            Ok(Some(request)) => request,
            Ok(None) => return None,
            Err(error) => {
                let error = error.into_runtime_error(
                    self.current_location.owner(),
                    self.current_location.path().to_vec(),
                    source_range,
                );
                self.errors.push(self.current_location.bind_error(error));
                return Some(vec![FlowOp::Noop]);
            }
        };
        let task_name = agent_task_name(expr);
        Some(vec![FlowOp::Await {
            binding: None,
            target: AwaitTarget::new(
                NeedId(format!("need.agent.{task_name}")),
                TaskId(format!("task.agent.{task_name}")),
                request,
            ),
            pending: Vec::new(),
        }])
    }

    fn lower_syntax_flow_items(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        items: &[FlowItem],
    ) -> Vec<FlowOp> {
        self.presentation_handle_scopes.push(BTreeMap::new());
        self.function_local_scopes.push(BTreeMap::new());
        let parent_location = self.current_location.clone();
        let mut ops = Vec::new();
        for (index, item) in items.iter().enumerate() {
            self.current_location = parent_location.statement(index);
            ops.extend(match item {
                FlowItem::Stmt(statement) => self.lower_flow_stmt(flow_id, flow_index, statement),
                other => {
                    self.errors.push(RuntimePlanLowerError::new(format!(
                        "unsupported nested flow item for runtime lowering: {other:?}"
                    )));
                    Vec::new()
                }
            });
        }
        self.current_location = parent_location;
        self.function_local_scopes.pop();
        self.presentation_handle_scopes.pop();
        ops
    }

    fn lower_flow_statements(&mut self, statements: &[Stmt]) -> Vec<LineEffectRequest> {
        let (effects, errors) = lower_line_plan_statements(statements);
        self.push_line_errors(errors);
        effects
    }

    fn push_line_errors(&mut self, errors: Vec<LinePlanLowerError>) {
        self.errors.extend(
            errors
                .into_iter()
                .map(|error| RuntimePlanLowerError::new(error.message().to_owned())),
        );
    }
}

#[cfg(test)]
mod tests;
