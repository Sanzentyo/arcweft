//! Flow-runtime lowering.

mod record_projection;

use self::record_projection::rewrite_known_record_projections_in_op;
use crate::errors::{LinePlanLowerError, RuntimePlanLowerError};
use crate::expr::{
    RuntimePureHelperLookup, lower_runtime_expr, lower_runtime_expr_strict_with_expected_type,
    lower_runtime_expr_strict_with_pure, runtime_call_effect,
};
use crate::host_request::{lower_agent_host_task_request, lower_host_task_request};
use crate::labels::expr_label;
use crate::line_task::{lower_line_plan, lower_line_plan_statements};
use crate::pattern::lower_runtime_pattern;
use crate::pure::{PureHelperCandidate, lower_pure_helper_candidates};
use crate::render_text::{
    DialogueDisplayDefaults, DialogueSpeakerPreset, lower_dialogue_display_with_speaker_presets,
    speaker_preset_from_let,
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
    RuntimeMatchArm, RuntimePlan, RuntimePureHelper, RuntimePureHelperId, RuntimeRouteBinding,
    RuntimeRouteBindingSource, RuntimeRouteSpec, RuntimeTraitMethod,
};
use arcweft_core::step::RuntimeHostCallMode;
use arcweft_core::task::{
    AWAIT_MANY_ITEM_BINDING, AwaitManyTarget, AwaitTarget, HostTaskArgTemplate,
    HostTaskRequestTemplate, NeedId, TaskId,
};
use arcweft_core::value::{RuntimeExpr, RuntimeExprMatchArm, RuntimeSeq, RuntimeValue};
use arcweft_lang_hir::model::{
    HirAgent, HirAwait, HirChoice, HirChoiceOption, HirDialogue, HirFlow, HirFlowItem, HirLoop,
    HirMatch, HirModule, HirScopeExpr, HirThread, HirTopLevelDecl,
};
use arcweft_lang_hir::syntax::ast::{
    choice::ChoiceAction,
    flow::{AwaitBranchKind, FlowItem, ScopeExprBlock, Stmt, StmtMatchArm, ThreadBlock},
    ids::{EntityRef, EntityRefSyntax},
    items::{EntryItem, EntryKind, FunctionKind},
    line_plan::LinePlan,
    pattern::Pattern,
};
use arcweft_lang_hir::syntax::expr::Expr;
use arcweft_lang_hir::syntax::parser::parse_dialogue_content_lossy;
use arcweft_lang_hir::syntax::types::TypeRef;
use arcweft_render_text::LineDisplayCatalog;
use presentation::{
    presentation_create_args, presentation_explicit_mount_handle_id, presentation_handle_call,
    presentation_handle_id, presentation_mount_call,
};
use std::{cell::Cell, collections::BTreeMap, sync::Arc};

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
}

/// Options that select profile/build-context inputs for runtime-plan lowering.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimePlanLowerOptions {
    dialogue_defaults: Option<String>,
    for_iteration_evidence: Vec<RuntimeIteratorEvidence>,
    trait_methods: Vec<RuntimeTraitMethod>,
    typed_lowering_evidence: Vec<RuntimeTypedLoweringEvidence>,
    required_typed_lowering_evidence_len: Option<usize>,
}

impl RuntimePlanLowerOptions {
    /// Creates default source-local lowering options.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            dialogue_defaults: None,
            for_iteration_evidence: Vec::new(),
            trait_methods: Vec::new(),
            typed_lowering_evidence: Vec::new(),
            required_typed_lowering_evidence_len: None,
        }
    }

    /// Selects a dialogue defaults profile by entity ID, for example
    /// `dialogue.defaults.mobile`.
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
    let pure_helpers = runtime_pure_helpers(&pure_candidate_report.candidates, &mut stats);
    let pure_map = pure_helper_map(&pure_helpers);
    let entries = lower_runtime_entries(module);
    let (flows, line_task_groups, line_display_catalog, stream_plans, source_plans) = {
        let typed_expression_cursor = Cell::new(0);
        let pure_lookup = RuntimePureHelperLookup::new(&pure_map, &pure_helpers)
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
            .map(|function| lower_stream_function(function, pure_lookup))
            .collect::<Vec<_>>();
        let source_plans = module
            .declarations()
            .iter()
            .filter_map(|decl| match decl {
                HirTopLevelDecl::Source(source) => Some(lower_source_plan(source, pure_lookup)),
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
            let plan = finalize_runtime_plan(
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
    let pure_helpers = runtime_pure_helpers(&pure_candidate_report.candidates, &mut stats);
    let pure_map = pure_helper_map(&pure_helpers);
    let lowered = {
        let typed_expression_cursor = Cell::new(0);
        let pure_lookup = RuntimePureHelperLookup::new(&pure_map, &pure_helpers)
            .with_typed_lowering_evidence(
                options.typed_lowering_evidence(),
                &typed_expression_cursor,
            );
        lower_agent_controller_flow(module, agent, pure_lookup)?
    };
    let entry_flow = lowered.id.clone();
    let entry_id = EntryRuntimeId::canonical(&entry_flow.canonical_label())
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    stats.pure_helpers = pure_helpers.len();
    RuntimePlan::new(Some(entry_flow.clone()), vec![lowered], Vec::new())
        .map(|plan| {
            let plan = finalize_runtime_plan(
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
            }
        })
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])
}

fn runtime_pure_helpers(
    candidates: &[PureHelperCandidate],
    stats: &mut RuntimePlanLowerStats,
) -> Vec<RuntimePureHelper> {
    candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            stats.pure_expr_cloned_nodes += candidate.shape().expr_weight;
            candidate.to_runtime_helper(RuntimePureHelperId(index))
        })
        .collect()
}

fn pure_helper_map(helpers: &[RuntimePureHelper]) -> BTreeMap<String, RuntimePureHelperId> {
    helpers
        .iter()
        .map(|helper| (helper.name.clone(), helper.id))
        .collect()
}

fn finalize_runtime_plan(mut plan: RuntimePlan, stats: &mut RuntimePlanLowerStats) -> RuntimePlan {
    for flow in &mut plan.flows {
        stats.optimized_flows += 1;
        optimize_flow_ops(&mut flow.ops, stats);
    }
    stats.pure_call_exprs = plan
        .flows
        .iter()
        .map(|flow| count_flow_ops_pure_calls(&flow.ops))
        .sum();
    plan
}

fn optimize_flow_ops(ops: &mut Vec<FlowOp>, stats: &mut RuntimePlanLowerStats) {
    stats.optimized_op_slices += 1;
    for op in ops.iter_mut() {
        optimize_nested_flow_ops(op, stats);
    }
    optimize_known_record_projection_lets(ops);
    optimize_local_map_sum_lets(ops, stats);
}

fn optimize_flow_op_slice(ops: &mut [FlowOp], stats: &mut RuntimePlanLowerStats) {
    stats.optimized_op_slices += 1;
    for op in ops {
        optimize_nested_flow_ops(op, stats);
    }
}

fn optimize_nested_flow_ops(op: &mut FlowOp, stats: &mut RuntimePlanLowerStats) {
    match op {
        FlowOp::LetElse { else_ops, .. } => optimize_flow_ops(else_ops, stats),
        FlowOp::If {
            then_ops, else_ops, ..
        }
        | FlowOp::IfLet {
            then_ops, else_ops, ..
        } => {
            optimize_flow_ops(then_ops, stats);
            optimize_flow_ops(else_ops, stats);
        }
        FlowOp::Match { arms, .. } => {
            for arm in arms {
                optimize_flow_ops(&mut arm.ops, stats);
            }
        }
        FlowOp::Loop { body }
        | FlowOp::LetLoop { body, .. }
        | FlowOp::While { body, .. }
        | FlowOp::WhileLet { body, .. }
        | FlowOp::Thread { body, .. }
        | FlowOp::Scope(body)
        | FlowOp::LetScope { ops: body, .. }
        | FlowOp::For { body, .. } => optimize_flow_ops(body, stats),
        FlowOp::LoopNext { body }
        | FlowOp::WhileNext { body, .. }
        | FlowOp::WhileLetNext { body, .. }
        | FlowOp::ForNext { body, .. } => optimize_flow_op_slice(Arc::make_mut(body), stats),
        FlowOp::Bind(_)
        | FlowOp::Let { .. }
        | FlowOp::Dialogue { .. }
        | FlowOp::Choice { .. }
        | FlowOp::Await { .. }
        | FlowOp::AwaitMany { .. }
        | FlowOp::HostCall { .. }
        | FlowOp::Break(_)
        | FlowOp::Continue
        | FlowOp::Goto(_)
        | FlowOp::GotoExpr(_)
        | FlowOp::Return(_)
        | FlowOp::ReturnExpr(_)
        | FlowOp::Effect(_)
        | FlowOp::RegisterCleanup { .. }
        | FlowOp::CancelCleanup { .. }
        | FlowOp::EnterScope
        | FlowOp::ExitScope
        | FlowOp::ExitScopeBind { .. }
        | FlowOp::Noop => {}
    }
}

fn optimize_local_map_sum_lets(ops: &mut Vec<FlowOp>, stats: &mut RuntimePlanLowerStats) {
    let original = std::mem::take(ops);
    let mut index = 0;
    while index < original.len() {
        if let Some(op) = fuse_sequence_map_sum_window(&original, index, stats) {
            stats.sequence_map_sum_fusions += 1;
            ops.push(op);
            index += 3;
            continue;
        }
        if let Some(op) = fuse_map_sum_window(&original, index, stats) {
            stats.map_sum_fusions += 1;
            ops.push(op);
            index += 2;
            continue;
        }
        if let Some(op) = inline_sequence_map_sum_source_window(&original, index, stats) {
            stats.sequence_source_inlines += 1;
            ops.push(op);
            index += 2;
            continue;
        }
        ops.push(original[index].clone());
        index += 1;
    }
}

fn optimize_known_record_projection_lets(ops: &mut [FlowOp]) {
    let mut env = Vec::<(String, Vec<String>)>::new();
    for op in ops {
        rewrite_known_record_projections_in_op(op, &env);
        if let Some(name) = runtime_pattern_binding_name_from_op(op).map(str::to_owned) {
            env.retain(|(candidate, _)| candidate != &name);
            if let FlowOp::Let { expr, .. } = op
                && let Some(fields) = record_projection_fields(expr)
            {
                env.push((name, fields));
            }
        }
    }
}

fn record_projection_fields(expr: &RuntimeExpr) -> Option<Vec<String>> {
    let RuntimeExpr::Value(RuntimeValue::Seq(RuntimeSeq::RecordColumns(records))) = expr else {
        return None;
    };
    Some(
        records
            .fields()
            .iter()
            .map(|field| field.name.clone())
            .collect(),
    )
}

fn fuse_sequence_map_sum_window(
    ops: &[FlowOp],
    index: usize,
    stats: &mut RuntimePlanLowerStats,
) -> Option<FlowOp> {
    let (sequence_name, source_expr) = sequence_let_binding(ops.get(index)?)?;
    let (_, map_expr) = map_let_binding(ops.get(index + 1)?)?;
    let (sum_pattern, sum_source) = local_sum_let_binding(ops.get(index + 2)?)?;
    let (map_source, _) = map_expr_source(map_expr)?;
    if map_source != sequence_name
        || sum_source != runtime_pattern_binding_name_from_op(ops.get(index + 1)?)?
    {
        return None;
    }
    if local_uses_in_op(ops.get(index + 1)?, sequence_name, stats) != 1 {
        return None;
    }
    if local_uses_in_op(ops.get(index + 2)?, sum_source, stats) != 1 {
        return None;
    }
    if !local_is_unused_after_op(ops, index + 1, sequence_name, stats) {
        return None;
    }
    if !local_is_unused_after_op(ops, index + 2, sum_source, stats) {
        return None;
    }
    let mut fused_map = map_expr.clone();
    replace_map_source(&mut fused_map, sequence_name, source_expr)?;
    Some(FlowOp::Let {
        pattern: sum_pattern.clone(),
        expr: RuntimeExpr::Sum {
            source: Box::new(fused_map),
        },
    })
}

fn fuse_map_sum_window(
    ops: &[FlowOp],
    index: usize,
    stats: &mut RuntimePlanLowerStats,
) -> Option<FlowOp> {
    let (sequence_name, map_expr) = map_let_binding(ops.get(index)?)?;
    let (sum_pattern, sum_source) = local_sum_let_binding(ops.get(index + 1)?)?;
    if sequence_name != sum_source
        || !local_is_unused_after_op(ops, index + 1, sequence_name, stats)
    {
        return None;
    }
    if local_uses_in_op(ops.get(index + 1)?, sequence_name, stats) != 1 {
        return None;
    }
    Some(FlowOp::Let {
        pattern: sum_pattern.clone(),
        expr: RuntimeExpr::Sum {
            source: Box::new(map_expr.clone()),
        },
    })
}

fn inline_sequence_map_sum_source_window(
    ops: &[FlowOp],
    index: usize,
    stats: &mut RuntimePlanLowerStats,
) -> Option<FlowOp> {
    let (sequence_name, source_expr) = sequence_let_binding(ops.get(index)?)?;
    if !local_is_unused_after_op(ops, index + 1, sequence_name, stats) {
        return None;
    }
    if local_uses_in_op(ops.get(index + 1)?, sequence_name, stats) != 1 {
        return None;
    }
    let mut next = ops.get(index + 1)?.clone();
    replace_map_sum_source(&mut next, sequence_name, source_expr).then_some(next)
}

fn sequence_let_binding(op: &FlowOp) -> Option<(&str, &RuntimeExpr)> {
    let FlowOp::Let { pattern, expr } = op else {
        return None;
    };
    is_runtime_sequence_expr(expr)
        .then(|| runtime_pattern_binding_name(pattern).map(|name| (name, expr)))?
}

fn is_runtime_sequence_expr(expr: &RuntimeExpr) -> bool {
    matches!(
        expr,
        RuntimeExpr::Value(RuntimeValue::Seq(_) | RuntimeValue::Tuple(_))
            | RuntimeExpr::RepeatSeq { .. }
            | RuntimeExpr::BracketSeq(_)
            | RuntimeExpr::Tuple(_)
    )
}

fn replace_map_sum_source(op: &mut FlowOp, sequence_name: &str, source_expr: &RuntimeExpr) -> bool {
    let FlowOp::Let { expr, .. } = op else {
        return false;
    };
    let RuntimeExpr::Sum { source } = expr else {
        return false;
    };
    let RuntimeExpr::Map { source, .. } = source.as_mut() else {
        return false;
    };
    let RuntimeExpr::Local(name) = source.as_ref() else {
        return false;
    };
    if name != sequence_name {
        return false;
    }
    **source = source_expr.clone();
    true
}

fn map_expr_source(expr: &RuntimeExpr) -> Option<(&str, &RuntimeExpr)> {
    let RuntimeExpr::Map { source, .. } = expr else {
        return None;
    };
    let RuntimeExpr::Local(name) = source.as_ref() else {
        return None;
    };
    Some((name.as_str(), source.as_ref()))
}

fn replace_map_source(
    expr: &mut RuntimeExpr,
    sequence_name: &str,
    source_expr: &RuntimeExpr,
) -> Option<()> {
    let RuntimeExpr::Map { source, .. } = expr else {
        return None;
    };
    let RuntimeExpr::Local(name) = source.as_ref() else {
        return None;
    };
    (name == sequence_name).then(|| {
        **source = source_expr.clone();
    })
}

fn map_let_binding(op: &FlowOp) -> Option<(&str, &RuntimeExpr)> {
    let FlowOp::Let { pattern, expr } = op else {
        return None;
    };
    let RuntimeExpr::Map { .. } = expr else {
        return None;
    };
    runtime_pattern_binding_name(pattern).map(|name| (name, expr))
}

fn local_sum_let_binding(op: &FlowOp) -> Option<(&arcweft_core::pattern::RuntimePattern, &str)> {
    let FlowOp::Let { pattern, expr } = op else {
        return None;
    };
    let RuntimeExpr::Sum { source } = expr else {
        return None;
    };
    match source.as_ref() {
        RuntimeExpr::Local(name) => Some((pattern, name.as_str())),
        _ => None,
    }
}

fn runtime_pattern_binding_name(pattern: &arcweft_core::pattern::RuntimePattern) -> Option<&str> {
    match pattern {
        arcweft_core::pattern::RuntimePattern::Ident(name)
        | arcweft_core::pattern::RuntimePattern::MutIdent(name)
        | arcweft_core::pattern::RuntimePattern::Typed { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

fn runtime_pattern_binding_name_from_op(op: &FlowOp) -> Option<&str> {
    let FlowOp::Let { pattern, .. } = op else {
        return None;
    };
    runtime_pattern_binding_name(pattern)
}

fn local_is_unused_after_op(
    ops: &[FlowOp],
    op_index: usize,
    name: &str,
    stats: &mut RuntimePlanLowerStats,
) -> bool {
    stats.local_use_tail_scans += 1;
    ops.iter().skip(op_index + 1).all(|op| {
        stats.local_use_scan_ops += 1;
        count_flow_op_local_uses_by_name(op, name) == 0
    })
}

fn local_uses_in_op(op: &FlowOp, name: &str, stats: &mut RuntimePlanLowerStats) -> usize {
    stats.local_use_scan_ops += 1;
    count_flow_op_local_uses_by_name(op, name)
}

fn count_flow_ops_pure_calls(ops: &[FlowOp]) -> usize {
    ops.iter().map(count_flow_op_pure_calls).sum()
}

fn count_flow_op_pure_calls(op: &FlowOp) -> usize {
    match op {
        FlowOp::LetElse { expr, else_ops, .. } => {
            count_runtime_expr_pure_calls(expr) + count_flow_ops_pure_calls(else_ops)
        }
        FlowOp::If {
            condition,
            then_ops,
            else_ops,
        } => {
            count_runtime_expr_pure_calls(condition)
                + count_flow_ops_pure_calls(then_ops)
                + count_flow_ops_pure_calls(else_ops)
        }
        FlowOp::IfLet {
            expr,
            guard,
            then_ops,
            else_ops,
            ..
        } => {
            count_runtime_expr_pure_calls(expr)
                + guard.as_ref().map_or(0, count_runtime_expr_pure_calls)
                + count_flow_ops_pure_calls(then_ops)
                + count_flow_ops_pure_calls(else_ops)
        }
        FlowOp::Match { scrutinee, arms } => {
            count_runtime_expr_pure_calls(scrutinee)
                + arms
                    .iter()
                    .map(|arm| {
                        arm.guard.as_ref().map_or(0, count_runtime_expr_pure_calls)
                            + count_flow_ops_pure_calls(&arm.ops)
                    })
                    .sum::<usize>()
        }
        FlowOp::Loop { body }
        | FlowOp::LetLoop { body, .. }
        | FlowOp::Thread { body, .. }
        | FlowOp::Scope(body) => count_flow_ops_pure_calls(body),
        FlowOp::LoopNext { body } | FlowOp::ForNext { body, .. } => count_flow_ops_pure_calls(body),
        FlowOp::While { condition, body } => {
            count_runtime_expr_pure_calls(condition) + count_flow_ops_pure_calls(body)
        }
        FlowOp::WhileNext { condition, body } => {
            count_runtime_expr_pure_calls(condition) + count_flow_ops_pure_calls(body)
        }
        FlowOp::WhileLet {
            expr, guard, body, ..
        } => {
            count_runtime_expr_pure_calls(expr)
                + guard.as_ref().map_or(0, count_runtime_expr_pure_calls)
                + count_flow_ops_pure_calls(body)
        }
        FlowOp::WhileLetNext {
            expr, guard, body, ..
        } => {
            count_runtime_expr_pure_calls(expr)
                + guard.as_ref().map_or(0, count_runtime_expr_pure_calls)
                + count_flow_ops_pure_calls(body)
        }
        FlowOp::For { source, body, .. } => {
            count_runtime_expr_pure_calls(source) + count_flow_ops_pure_calls(body)
        }
        FlowOp::AwaitMany { target, .. } => count_runtime_expr_pure_calls(&target.source),
        FlowOp::HostCall { target, .. } => {
            target.args.iter().map(count_runtime_expr_pure_calls).sum()
        }
        FlowOp::LetScope { ops, value, .. } => {
            count_flow_ops_pure_calls(ops) + count_runtime_expr_pure_calls(value)
        }
        FlowOp::Let { expr, .. }
        | FlowOp::Break(Some(expr))
        | FlowOp::GotoExpr(expr)
        | FlowOp::ReturnExpr(expr)
        | FlowOp::ExitScopeBind { expr, .. } => count_runtime_expr_pure_calls(expr),
        FlowOp::Bind(_)
        | FlowOp::Dialogue { .. }
        | FlowOp::Choice { .. }
        | FlowOp::Await { .. }
        | FlowOp::Effect(_)
        | FlowOp::RegisterCleanup { .. }
        | FlowOp::CancelCleanup { .. }
        | FlowOp::EnterScope
        | FlowOp::ExitScope
        | FlowOp::Break(None)
        | FlowOp::Continue
        | FlowOp::Goto(_)
        | FlowOp::Return(_)
        | FlowOp::Noop => 0,
    }
}

fn count_runtime_expr_pure_calls(expr: &RuntimeExpr) -> usize {
    match expr {
        RuntimeExpr::PureCall { args, .. } => {
            1 + args
                .iter()
                .map(count_runtime_expr_pure_calls)
                .sum::<usize>()
        }
        RuntimeExpr::Let { expr, body, .. } => {
            count_runtime_expr_pure_calls(expr) + count_runtime_expr_pure_calls(body)
        }
        RuntimeExpr::AssignField {
            target, expr, body, ..
        } => {
            count_runtime_expr_pure_calls(target)
                + count_runtime_expr_pure_calls(expr)
                + count_runtime_expr_pure_calls(body)
        }
        RuntimeExpr::Tuple(items) | RuntimeExpr::BracketSeq(items) => {
            items.iter().map(count_runtime_expr_pure_calls).sum()
        }
        RuntimeExpr::RepeatSeq { value, .. } => count_runtime_expr_pure_calls(value),
        RuntimeExpr::Range { start, end, .. } => {
            start.as_deref().map_or(0, count_runtime_expr_pure_calls)
                + end.as_deref().map_or(0, count_runtime_expr_pure_calls)
        }
        RuntimeExpr::Record(fields) => fields
            .iter()
            .map(|field| count_runtime_expr_pure_calls(&field.value))
            .sum(),
        RuntimeExpr::Variant { payload, .. } => {
            payload.as_deref().map_or(0, count_runtime_expr_pure_calls)
        }
        RuntimeExpr::Field { target, .. }
        | RuntimeExpr::ProjectTuple { target, .. }
        | RuntimeExpr::ProjectRecord { target, .. }
        | RuntimeExpr::SpreadArg(target) => count_runtime_expr_pure_calls(target),
        RuntimeExpr::Call { args, .. } => args.iter().map(count_runtime_expr_pure_calls).sum(),
        RuntimeExpr::Function { body, .. } => count_runtime_expr_pure_calls(body),
        RuntimeExpr::Apply { callee, args } => {
            count_runtime_expr_pure_calls(callee)
                + args
                    .iter()
                    .map(count_runtime_expr_pure_calls)
                    .sum::<usize>()
        }
        RuntimeExpr::MethodCall { receiver, args, .. }
        | RuntimeExpr::TraitCall { receiver, args, .. } => {
            count_runtime_expr_pure_calls(receiver)
                + args
                    .iter()
                    .map(count_runtime_expr_pure_calls)
                    .sum::<usize>()
        }
        RuntimeExpr::Map { source, body, .. } | RuntimeExpr::Filter { source, body, .. } => {
            count_runtime_expr_pure_calls(source) + count_runtime_expr_pure_calls(body)
        }
        RuntimeExpr::Sum { source } | RuntimeExpr::Unary { expr: source, .. } => {
            count_runtime_expr_pure_calls(source)
        }
        RuntimeExpr::Binary { lhs, rhs, .. } => {
            count_runtime_expr_pure_calls(lhs) + count_runtime_expr_pure_calls(rhs)
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            count_runtime_expr_pure_calls(condition)
                + count_runtime_expr_pure_calls(then_expr)
                + count_runtime_expr_pure_calls(else_expr)
        }
        RuntimeExpr::IfLet {
            expr,
            guard,
            then_expr,
            else_expr,
            ..
        } => {
            count_runtime_expr_pure_calls(expr)
                + guard.as_deref().map_or(0, count_runtime_expr_pure_calls)
                + count_runtime_expr_pure_calls(then_expr)
                + count_runtime_expr_pure_calls(else_expr)
        }
        RuntimeExpr::Match { scrutinee, arms } => {
            count_runtime_expr_pure_calls(scrutinee)
                + arms
                    .iter()
                    .map(|arm| {
                        arm.guard.as_ref().map_or(0, count_runtime_expr_pure_calls)
                            + count_runtime_expr_pure_calls(&arm.value)
                    })
                    .sum::<usize>()
        }
        RuntimeExpr::Value(_) | RuntimeExpr::Local(_) | RuntimeExpr::EntityRef(_) => 0,
    }
}

fn count_flow_ops_local_uses_by_name(ops: &[FlowOp], name: &str) -> usize {
    ops.iter()
        .map(|op| count_flow_op_local_uses_by_name(op, name))
        .sum()
}

fn count_flow_op_local_uses_by_name(op: &FlowOp, name: &str) -> usize {
    match op {
        FlowOp::LetElse { expr, else_ops, .. } => {
            count_runtime_expr_local_uses_by_name(expr, name)
                + count_flow_ops_local_uses_by_name(else_ops, name)
        }
        FlowOp::If {
            condition,
            then_ops,
            else_ops,
        } => {
            count_runtime_expr_local_uses_by_name(condition, name)
                + count_flow_ops_local_uses_by_name(then_ops, name)
                + count_flow_ops_local_uses_by_name(else_ops, name)
        }
        FlowOp::IfLet {
            expr,
            guard,
            then_ops,
            else_ops,
            ..
        } => {
            count_runtime_expr_local_uses_by_name(expr, name)
                + count_optional_runtime_expr_local_uses_by_name(guard.as_ref(), name)
                + count_flow_ops_local_uses_by_name(then_ops, name)
                + count_flow_ops_local_uses_by_name(else_ops, name)
        }
        FlowOp::Match { scrutinee, arms } => {
            count_runtime_expr_local_uses_by_name(scrutinee, name)
                + count_match_arms_local_uses_by_name(arms, name)
        }
        FlowOp::Loop { body }
        | FlowOp::LetLoop { body, .. }
        | FlowOp::Thread { body, .. }
        | FlowOp::Scope(body) => count_flow_ops_local_uses_by_name(body, name),
        FlowOp::LoopNext { body } | FlowOp::ForNext { body, .. } => {
            count_flow_ops_local_uses_by_name(body, name)
        }
        FlowOp::While { condition, body } => {
            count_runtime_expr_local_uses_by_name(condition, name)
                + count_flow_ops_local_uses_by_name(body, name)
        }
        FlowOp::WhileNext { condition, body } => {
            count_runtime_expr_local_uses_by_name(condition, name)
                + count_flow_ops_local_uses_by_name(body, name)
        }
        FlowOp::WhileLet {
            expr, guard, body, ..
        } => {
            count_runtime_expr_local_uses_by_name(expr, name)
                + count_optional_runtime_expr_local_uses_by_name(guard.as_ref(), name)
                + count_flow_ops_local_uses_by_name(body, name)
        }
        FlowOp::WhileLetNext {
            expr, guard, body, ..
        } => {
            count_runtime_expr_local_uses_by_name(expr, name)
                + count_optional_runtime_expr_local_uses_by_name(guard.as_ref(), name)
                + count_flow_ops_local_uses_by_name(body, name)
        }
        FlowOp::For { source, body, .. } => {
            count_runtime_expr_local_uses_by_name(source, name)
                + count_flow_ops_local_uses_by_name(body, name)
        }
        FlowOp::AwaitMany { target, .. } => {
            count_runtime_expr_local_uses_by_name(&target.source, name)
        }
        FlowOp::HostCall { target, .. } => target
            .args
            .iter()
            .map(|arg| count_runtime_expr_local_uses_by_name(arg, name))
            .sum(),
        FlowOp::LetScope { ops, value, .. } => {
            count_flow_ops_local_uses_by_name(ops, name)
                + count_runtime_expr_local_uses_by_name(value, name)
        }
        FlowOp::Let { expr, .. }
        | FlowOp::Break(Some(expr))
        | FlowOp::GotoExpr(expr)
        | FlowOp::ReturnExpr(expr)
        | FlowOp::ExitScopeBind { expr, .. } => count_runtime_expr_local_uses_by_name(expr, name),
        FlowOp::Bind(_)
        | FlowOp::Dialogue { .. }
        | FlowOp::Choice { .. }
        | FlowOp::Await { .. }
        | FlowOp::Effect(_)
        | FlowOp::RegisterCleanup { .. }
        | FlowOp::CancelCleanup { .. }
        | FlowOp::EnterScope
        | FlowOp::ExitScope
        | FlowOp::Break(None)
        | FlowOp::Continue
        | FlowOp::Goto(_)
        | FlowOp::Return(_)
        | FlowOp::Noop => 0,
    }
}

fn count_match_arms_local_uses_by_name(arms: &[RuntimeMatchArm], name: &str) -> usize {
    arms.iter()
        .map(|arm| {
            count_optional_runtime_expr_local_uses_by_name(arm.guard.as_ref(), name)
                + count_flow_ops_local_uses_by_name(&arm.ops, name)
        })
        .sum()
}

fn count_runtime_expr_local_uses_by_name(expr: &RuntimeExpr, name: &str) -> usize {
    match expr {
        RuntimeExpr::Local(local) => usize::from(local == name),
        RuntimeExpr::Let { expr, body, .. } => {
            count_runtime_expr_local_uses_by_name(expr, name)
                + count_runtime_expr_local_uses_by_name(body, name)
        }
        RuntimeExpr::AssignField {
            target, expr, body, ..
        } => {
            count_runtime_expr_local_uses_by_name(target, name)
                + count_runtime_expr_local_uses_by_name(expr, name)
                + count_runtime_expr_local_uses_by_name(body, name)
        }
        RuntimeExpr::Tuple(items) | RuntimeExpr::BracketSeq(items) => items
            .iter()
            .map(|item| count_runtime_expr_local_uses_by_name(item, name))
            .sum(),
        RuntimeExpr::RepeatSeq { value, .. } => count_runtime_expr_local_uses_by_name(value, name),
        RuntimeExpr::Range { start, end, .. } => {
            count_optional_runtime_expr_local_uses_by_name(start.as_deref(), name)
                + count_optional_runtime_expr_local_uses_by_name(end.as_deref(), name)
        }
        RuntimeExpr::Record(fields) => fields
            .iter()
            .map(|field| count_runtime_expr_local_uses_by_name(&field.value, name))
            .sum(),
        RuntimeExpr::Variant { payload, .. } => payload.as_deref().map_or(0, |payload| {
            count_runtime_expr_local_uses_by_name(payload, name)
        }),
        RuntimeExpr::Field { target, .. }
        | RuntimeExpr::ProjectTuple { target, .. }
        | RuntimeExpr::ProjectRecord { target, .. }
        | RuntimeExpr::SpreadArg(target) => count_runtime_expr_local_uses_by_name(target, name),
        RuntimeExpr::Call { args, .. } | RuntimeExpr::PureCall { args, .. } => args
            .iter()
            .map(|arg| count_runtime_expr_local_uses_by_name(arg, name))
            .sum(),
        RuntimeExpr::Function { params, body } => {
            count_runtime_function_local_uses_by_name(params, body, name)
        }
        RuntimeExpr::Apply { callee, args } => {
            count_runtime_apply_local_uses_by_name(callee, args, name)
        }
        RuntimeExpr::MethodCall { receiver, args, .. }
        | RuntimeExpr::TraitCall { receiver, args, .. } => {
            count_runtime_expr_local_uses_by_name(receiver, name)
                + args
                    .iter()
                    .map(|arg| count_runtime_expr_local_uses_by_name(arg, name))
                    .sum::<usize>()
        }
        RuntimeExpr::Map {
            source,
            param,
            body,
        }
        | RuntimeExpr::Filter {
            source,
            param,
            body,
        } => count_runtime_scoped_body_local_uses_by_name(source, param, body, name),
        RuntimeExpr::Sum { source } | RuntimeExpr::Unary { expr: source, .. } => {
            count_runtime_expr_local_uses_by_name(source, name)
        }
        RuntimeExpr::Binary { lhs, rhs, .. } => {
            count_runtime_expr_local_uses_by_name(lhs, name)
                + count_runtime_expr_local_uses_by_name(rhs, name)
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            count_runtime_expr_local_uses_by_name(condition, name)
                + count_runtime_expr_local_uses_by_name(then_expr, name)
                + count_runtime_expr_local_uses_by_name(else_expr, name)
        }
        RuntimeExpr::IfLet {
            expr,
            guard,
            then_expr,
            else_expr,
            ..
        } => count_runtime_if_let_local_uses_by_name(
            expr,
            guard.as_deref(),
            then_expr,
            else_expr,
            name,
        ),
        RuntimeExpr::Match { scrutinee, arms } => {
            count_runtime_match_local_uses_by_name(scrutinee, arms, name)
        }
        RuntimeExpr::Value(_) | RuntimeExpr::EntityRef(_) => 0,
    }
}

fn count_runtime_function_local_uses_by_name(
    params: &[String],
    body: &RuntimeExpr,
    name: &str,
) -> usize {
    if params.iter().any(|param| param == name) {
        0
    } else {
        count_runtime_expr_local_uses_by_name(body, name)
    }
}

fn count_runtime_apply_local_uses_by_name(
    callee: &RuntimeExpr,
    args: &[RuntimeExpr],
    name: &str,
) -> usize {
    count_runtime_expr_local_uses_by_name(callee, name)
        + args
            .iter()
            .map(|arg| count_runtime_expr_local_uses_by_name(arg, name))
            .sum::<usize>()
}

fn count_runtime_scoped_body_local_uses_by_name(
    source: &RuntimeExpr,
    param: &str,
    body: &RuntimeExpr,
    name: &str,
) -> usize {
    count_runtime_expr_local_uses_by_name(source, name)
        + if param == name {
            0
        } else {
            count_runtime_expr_local_uses_by_name(body, name)
        }
}

fn count_runtime_if_let_local_uses_by_name(
    expr: &RuntimeExpr,
    guard: Option<&RuntimeExpr>,
    then_expr: &RuntimeExpr,
    else_expr: &RuntimeExpr,
    name: &str,
) -> usize {
    count_runtime_expr_local_uses_by_name(expr, name)
        + count_optional_runtime_expr_local_uses_by_name(guard, name)
        + count_runtime_expr_local_uses_by_name(then_expr, name)
        + count_runtime_expr_local_uses_by_name(else_expr, name)
}

fn count_runtime_match_local_uses_by_name(
    scrutinee: &RuntimeExpr,
    arms: &[RuntimeExprMatchArm],
    name: &str,
) -> usize {
    count_runtime_expr_local_uses_by_name(scrutinee, name)
        + arms
            .iter()
            .map(|arm| {
                count_optional_runtime_expr_local_uses_by_name(arm.guard.as_ref(), name)
                    + count_runtime_expr_local_uses_by_name(&arm.value, name)
            })
            .sum::<usize>()
}

fn count_optional_runtime_expr_local_uses_by_name(expr: Option<&RuntimeExpr>, name: &str) -> usize {
    expr.map_or(0, |expr| count_runtime_expr_local_uses_by_name(expr, name))
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
    pure_helpers: RuntimePureHelperLookup<'_, 'static>,
    options: &RuntimePlanLowerOptions,
) -> Result<LoweredRuntimeFlows, Vec<RuntimePlanLowerError>> {
    let display_defaults = DialogueDisplayDefaults::try_from_module_with_selection(
        module,
        options.dialogue_defaults(),
    )
    .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    let mut lowerer = FlowRuntimeLowerer {
        agent_controller: false,
        line_task_groups: Vec::new(),
        line_display_catalog: LineDisplayCatalog::default(),
        display_defaults,
        speaker_preset_scopes: Vec::new(),
        presentation_handle_scopes: Vec::new(),
        function_local_scopes: Vec::new(),
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
    pure_helpers: RuntimePureHelperLookup<'_, 'static>,
) -> Result<RuntimeFlow, Vec<RuntimePlanLowerError>> {
    let display_defaults = DialogueDisplayDefaults::try_from_module_with_selection(module, None)
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    let mut lowerer = FlowRuntimeLowerer {
        agent_controller: true,
        line_task_groups: Vec::new(),
        line_display_catalog: LineDisplayCatalog::default(),
        display_defaults,
        speaker_preset_scopes: Vec::new(),
        presentation_handle_scopes: Vec::new(),
        function_local_scopes: Vec::new(),
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
    let mut ops = lowerer.lower_flow_stmt_list(&id, 0, agent.item().body_statements());
    if let Some(value) = agent.item().body_value() {
        if let Some(mut host_ops) = lowerer.lower_agent_host_call_expr(value) {
            ops.append(&mut host_ops);
        } else {
            ops.push(FlowOp::ReturnExpr(lowerer.lower_runtime_expr(value)));
        }
    }
    if lowerer.errors.is_empty() {
        Ok(RuntimeFlow { id, ops })
    } else {
        Err(lowerer.errors)
    }
}

struct FlowRuntimeLowerer<'a> {
    agent_controller: bool,
    line_task_groups: Vec<LineTaskGroup>,
    line_display_catalog: LineDisplayCatalog,
    display_defaults: DialogueDisplayDefaults,
    speaker_preset_scopes: Vec<BTreeMap<String, DialogueSpeakerPreset>>,
    presentation_handle_scopes: Vec<BTreeMap<String, PresentationHandleBinding>>,
    function_local_scopes: Vec<BTreeMap<String, usize>>,
    errors: Vec<RuntimePlanLowerError>,
    pure_helpers: RuntimePureHelperLookup<'a, 'static>,
    for_iteration_evidence: &'a [RuntimeIteratorEvidence],
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

impl FlowRuntimeLowerer<'_> {
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
        let ops = self.lower_flow_items(&id, flow.body(), index);
        RuntimeFlow { id, ops }
    }

    fn lower_flow_items(
        &mut self,
        flow_id: &FlowRuntimeId,
        items: &[HirFlowItem],
        flow_index: usize,
    ) -> Vec<FlowOp> {
        self.speaker_preset_scopes.push(BTreeMap::new());
        self.presentation_handle_scopes.push(BTreeMap::new());
        self.function_local_scopes.push(BTreeMap::new());
        let ops = self.lower_flow_items_in_scope(flow_id, items, flow_index);
        self.function_local_scopes.pop();
        self.presentation_handle_scopes.pop();
        self.speaker_preset_scopes.pop();
        ops
    }

    fn lower_flow_items_in_scope(
        &mut self,
        flow_id: &FlowRuntimeId,
        items: &[HirFlowItem],
        flow_index: usize,
    ) -> Vec<FlowOp> {
        let mut ops = Vec::new();
        for item in items {
            match item {
                HirFlowItem::Dialogue(dialogue) => {
                    ops.push(self.lower_runtime_dialogue(flow_id, flow_index, dialogue));
                }
                HirFlowItem::Choice(choice) | HirFlowItem::LetChoice { choice, .. } => {
                    ops.push(self.lower_choice(choice));
                }
                HirFlowItem::Await(await_with) => {
                    ops.push(self.lower_await(None, await_with));
                }
                HirFlowItem::LetAwait {
                    pattern,
                    await_with,
                    ..
                } => {
                    ops.push(self.lower_await(Some(pattern), await_with));
                }
                HirFlowItem::Stmt(stmt) => {
                    self.register_speaker_preset(stmt);
                    ops.extend(self.lower_flow_stmt(flow_id, flow_index, stmt));
                }
                HirFlowItem::Thread(thread) => {
                    ops.extend(self.lower_hir_thread(thread, flow_id, flow_index));
                }
                HirFlowItem::Scope(scope) => {
                    ops.push(FlowOp::Scope(self.lower_flow_items(
                        flow_id,
                        scope.body(),
                        flow_index,
                    )));
                }
                HirFlowItem::LetScope { pattern, scope } => {
                    ops.push(self.lower_scope_expr(flow_id, flow_index, pattern, scope));
                }
                HirFlowItem::If(block) => {
                    ops.push(FlowOp::If {
                        condition: self.lower_runtime_expr(block.condition()),
                        then_ops: self.lower_flow_items(flow_id, block.body(), flow_index),
                        else_ops: self.lower_flow_items(flow_id, block.else_body(), flow_index),
                    });
                }
                HirFlowItem::IfLet(block) => {
                    ops.push(FlowOp::IfLet {
                        pattern: lower_runtime_pattern(block.pattern()),
                        expr: self.lower_runtime_expr(block.expr()),
                        guard: self.lower_optional_runtime_expr(block.guard()),
                        then_ops: self.lower_flow_items(flow_id, block.body(), flow_index),
                        else_ops: self.lower_flow_items(flow_id, block.else_body(), flow_index),
                    });
                }
                HirFlowItem::Match(block) => {
                    ops.push(self.lower_match_block(flow_id, block, flow_index));
                }
                HirFlowItem::Loop(block) => {
                    ops.push(FlowOp::Loop {
                        body: self.lower_flow_items(flow_id, block.body(), flow_index),
                    });
                }
                HirFlowItem::LetLoop { pattern, block } => {
                    ops.push(self.lower_loop_expr(flow_id, pattern, block, flow_index));
                }
                HirFlowItem::While(block) => {
                    ops.push(FlowOp::While {
                        condition: self.lower_runtime_expr(block.condition()),
                        body: self.lower_flow_items(flow_id, block.body(), flow_index),
                    });
                }
                HirFlowItem::WhileLet(block) => {
                    ops.push(FlowOp::WhileLet {
                        pattern: lower_runtime_pattern(block.pattern()),
                        expr: self.lower_runtime_expr(block.expr()),
                        guard: self.lower_optional_runtime_expr(block.guard()),
                        body: self.lower_flow_items(flow_id, block.body(), flow_index),
                    });
                }
                HirFlowItem::For(block) => {
                    if let Some(evidence) = self.next_for_iteration_evidence() {
                        ops.push(FlowOp::For {
                            pattern: lower_runtime_pattern(block.pattern()),
                            source: self.lower_runtime_expr(block.source()),
                            evidence,
                            body: self.lower_flow_items(flow_id, block.body(), flow_index),
                        });
                    } else {
                        self.errors.push(RuntimePlanLowerError::new(
                            "missing trait-resolved IntoIterator evidence for `for` source",
                        ));
                    }
                }
                other => {
                    self.errors.push(RuntimePlanLowerError::new(format!(
                        "unsupported flow item for runtime lowering: {other:?}"
                    )));
                }
            }
        }
        ops
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
        FlowOp::LetScope {
            pattern: lower_runtime_pattern(pattern),
            ops: self.lower_flow_stmt_list(flow_id, flow_index, scope.statements()),
            value: scope
                .value()
                .map_or(RuntimeExpr::Value(RuntimeValue::Unit), |value| {
                    self.lower_runtime_expr(value)
                }),
        }
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
                    pattern: lower_runtime_pattern(arm.pattern()),
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
            pattern: lower_runtime_pattern(pattern),
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
                    "{}.line.{task_group}",
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
        self.line_display_catalog
            .push(lower_dialogue_display_with_speaker_presets(
                line.clone(),
                dialogue,
                &self.display_defaults,
                &active_speaker_presets,
            ));
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
        let pending = await_with
            .branches()
            .iter()
            .filter(|branch| branch.kind() == AwaitBranchKind::Pending)
            .flat_map(|branch| self.lower_pending_flow_items(branch.body()))
            .collect();
        match self.lower_await_many_target(await_with.expr(), &task_name) {
            Ok(Some(target)) => {
                return FlowOp::AwaitMany {
                    binding: binding.map(lower_runtime_pattern),
                    target,
                    pending,
                };
            }
            Ok(None) => {}
            Err(message) => {
                self.errors.push(RuntimePlanLowerError::new(message));
                return FlowOp::Noop;
            }
        }
        FlowOp::Await {
            binding: binding.map(lower_runtime_pattern),
            target: AwaitTarget::new(
                NeedId(format!("need.await.{task_name}")),
                TaskId(format!("task.await.{task_name}")),
                lower_host_task_request(await_with.expr()),
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
        match stmt {
            Stmt::Let {
                pattern, ty, expr, ..
            } => self.lower_let_stmt(flow_id, flow_index, pattern, ty.as_ref(), expr),
            Stmt::LetScope { pattern, scope } => {
                self.lower_let_scope_stmt(flow_id, flow_index, pattern, scope)
            }
            Stmt::LetLoop { pattern, block } => vec![FlowOp::LetLoop {
                pattern: lower_runtime_pattern(pattern),
                body: self.lower_syntax_flow_items(flow_id, flow_index, block.body()),
            }],
            Stmt::LetActionReceive { pattern, action } => {
                self.lower_action_receive_stmt(pattern, action)
            }
            Stmt::LetElse {
                pattern,
                ty,
                expr,
                else_body,
            } => vec![FlowOp::LetElse {
                pattern: lower_runtime_pattern(pattern),
                expr: self.lower_runtime_expr_with_expected_type(ty.as_ref(), expr),
                else_ops: self.lower_flow_stmt_list(flow_id, flow_index, else_body),
            }],
            Stmt::Goto(expr) => vec![FlowOp::GotoExpr(self.lower_runtime_expr(expr))],
            Stmt::Return(expr) => vec![FlowOp::ReturnExpr(
                self.lower_runtime_expr_result(expr)
                    .unwrap_or_else(|_| lower_runtime_expr(expr)),
            )],
            Stmt::Assign { target, expr } => {
                self.lower_assignment_stmt(target, expr)
                    .map_or_else(Vec::new, |expr| {
                        vec![FlowOp::Let {
                            pattern: RuntimePattern::Discard,
                            expr,
                        }]
                    })
            }
            Stmt::Expr(expr) => self
                .lower_presentation_handle_method(expr)
                .or_else(|| Self::lower_explicit_presentation_mount(flow_id, expr))
                .or_else(|| self.lower_agent_host_call_expr(expr))
                .unwrap_or_else(|| vec![FlowOp::Effect(runtime_call_effect(expr))]),
            Stmt::Out { label, expr } => {
                vec![FlowOp::Effect(LineEffectRequest::Out(LineOutRequest {
                    label: label.clone(),
                    value: expr_label(expr),
                }))]
            }
            Stmt::If {
                condition,
                body,
                else_body,
            } => self.lower_if_stmt(flow_id, flow_index, condition, body, else_body),
            Stmt::Loop { body } => vec![FlowOp::Loop {
                body: self.lower_flow_stmt_list(flow_id, flow_index, body),
            }],
            Stmt::While { condition, body } => vec![FlowOp::While {
                condition: self.lower_runtime_expr(condition),
                body: self.lower_flow_stmt_list(flow_id, flow_index, body),
            }],
            Stmt::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => vec![FlowOp::WhileLet {
                pattern: lower_runtime_pattern(pattern),
                expr: self.lower_runtime_expr(expr),
                guard: self.lower_optional_runtime_expr(guard.as_ref()),
                body: self.lower_flow_stmt_list(flow_id, flow_index, body),
            }],
            Stmt::For {
                pattern,
                source,
                body,
            } => self.lower_for_stmt(flow_id, flow_index, pattern, source, body),
            Stmt::Thread(thread) => self.lower_thread_stmt(flow_id, flow_index, thread),
            Stmt::Match { expr, arms } => vec![FlowOp::Match {
                scrutinee: self.lower_runtime_expr(expr),
                arms: self.lower_stmt_match_arms(flow_id, flow_index, arms),
            }],
            Stmt::Break { expr, .. } => {
                vec![FlowOp::Break(
                    self.lower_optional_runtime_expr(expr.as_ref()),
                )]
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

    fn lower_let_scope_stmt(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        pattern: &Pattern,
        scope: &ScopeExprBlock,
    ) -> Vec<FlowOp> {
        vec![FlowOp::LetScope {
            pattern: lower_runtime_pattern(pattern),
            ops: self.lower_flow_stmt_list(flow_id, flow_index, scope.statements()),
            value: scope
                .value()
                .map_or(RuntimeExpr::Value(RuntimeValue::Unit), |value| {
                    self.lower_runtime_expr(value)
                }),
        }]
    }

    fn lower_action_receive_stmt(&mut self, pattern: &Pattern, action: &Expr) -> Vec<FlowOp> {
        vec![FlowOp::HostCall {
            binding: Some(lower_runtime_pattern(pattern)),
            target: RuntimeHostCallTarget::new(
                "ui.action.await",
                "ui.action",
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
            pattern: lower_runtime_pattern(pattern),
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
        vec![FlowOp::For {
            pattern: lower_runtime_pattern(pattern),
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
    ) -> Vec<FlowOp> {
        if let Some(ops) = self.lower_dialogue_result_let(flow_id, flow_index, pattern, expr) {
            self.record_function_local_binding(pattern, None);
            ops
        } else if let Some(op) = self.lower_agent_host_call_let(pattern, expr) {
            self.record_function_local_binding(pattern, None);
            vec![op]
        } else if let Some(ops) = self.lower_presentation_handle_let(flow_id, pattern, expr) {
            self.record_function_local_binding(pattern, None);
            ops
        } else {
            let expr = self.lower_runtime_expr_with_expected_type(ty, expr);
            let arity = self.runtime_expr_function_arity(&expr);
            self.record_function_local_binding(pattern, arity);
            vec![FlowOp::Let {
                pattern: lower_runtime_pattern(pattern),
                expr,
            }]
        }
    }

    fn lower_stmt_match_arms(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        arms: &[StmtMatchArm],
    ) -> Vec<RuntimeMatchArm> {
        arms.iter()
            .map(|arm| RuntimeMatchArm {
                pattern: lower_runtime_pattern(arm.pattern()),
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
        self.presentation_handle_scopes.push(BTreeMap::new());
        self.function_local_scopes.push(BTreeMap::new());
        let ops = statements
            .iter()
            .flat_map(|statement| self.lower_flow_stmt(flow_id, flow_index, statement))
            .collect();
        self.function_local_scopes.pop();
        self.presentation_handle_scopes.pop();
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
        let dialogue = HirDialogue::expression_call(
            expr_label(callee),
            parse_dialogue_content_lossy(content.to_owned()),
            plan.cloned(),
        );
        Some(vec![
            self.lower_runtime_dialogue(flow_id, flow_index, &dialogue),
            FlowOp::Let {
                pattern: lower_runtime_pattern(pattern),
                expr: self.lower_runtime_expr(expr),
            },
        ])
    }

    fn lower_agent_host_call_let(&mut self, pattern: &Pattern, expr: &Expr) -> Option<FlowOp> {
        if !self.agent_controller {
            return None;
        }
        let request = lower_agent_host_task_request(expr)?;
        let task_name = agent_task_name(expr);
        Some(FlowOp::Await {
            binding: Some(lower_runtime_pattern(pattern)),
            target: AwaitTarget::new(
                NeedId(format!("need.agent.{task_name}")),
                TaskId(format!("task.agent.{task_name}")),
                request,
            ),
            pending: Vec::new(),
        })
    }

    fn lower_agent_host_call_expr(&mut self, expr: &Expr) -> Option<Vec<FlowOp>> {
        if !self.agent_controller {
            return None;
        }
        let request = lower_agent_host_task_request(expr)?;
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
        let ops = items
            .iter()
            .flat_map(|item| match item {
                FlowItem::Stmt(statement) => self.lower_flow_stmt(flow_id, flow_index, statement),
                other => {
                    self.errors.push(RuntimePlanLowerError::new(format!(
                        "unsupported nested flow item for runtime lowering: {other:?}"
                    )));
                    Vec::new()
                }
            })
            .collect();
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

fn dialogue_call_parts(expr: &Expr) -> Option<(&Expr, &str, Option<&LinePlan>)> {
    match expr {
        Expr::DialogueCall {
            callee,
            content,
            plan,
        } => Some((callee.as_ref(), content.as_str(), plan.as_ref())),
        Expr::Try { expr } => dialogue_call_parts(expr),
        _ => None,
    }
}

pub(crate) fn sanitize_task_id_part(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn agent_task_name(expr: &Expr) -> String {
    expr_label(expr)
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '.'
            }
        })
        .collect::<String>()
        .trim_matches('.')
        .to_owned()
}

fn flow_runtime_id(id: &EntityRef) -> FlowRuntimeId {
    FlowRuntimeId::from_runtime_target_value(id.body()).expect("HIR flow ID should be valid")
}

fn method_name(method: &str) -> &str {
    method.split_once('<').map_or(method, |(name, _)| name)
}

fn selected_call_parts(
    expr: &Expr,
) -> Option<(&Expr, &str, &[arcweft_lang_hir::syntax::expr::CallArg])> {
    let Expr::Call { callee, args } = expr else {
        return None;
    };
    let Expr::Select(select) = callee.as_ref() else {
        return None;
    };
    Some((select.target(), select.member().as_str(), args.as_slice()))
}

fn traverse_callee(args: &[arcweft_lang_hir::syntax::expr::CallArg]) -> Result<&Expr, String> {
    let [arg] = args else {
        return Err("traverse(...) requires exactly one positional task function".to_owned());
    };
    if arg.name().is_some() || arg.is_spread() {
        return Err("traverse(...) task function must be a positional argument".to_owned());
    }
    Ok(arg.value())
}

fn split_capability_operation(name: &str) -> Result<(String, String), String> {
    name.rsplit_once('.').map_or_else(
        || {
            Err(format!(
                "traverse task function `{name}` must be capability-qualified"
            ))
        },
        |(capability, operation)| Ok((capability.to_owned(), operation.to_owned())),
    )
}

fn parallel_limit(args: &[arcweft_lang_hir::syntax::expr::CallArg]) -> Result<usize, String> {
    let [arg] = args else {
        return Err("parallel(...) requires exactly `limit = N`".to_owned());
    };
    if arg.name() != Some("limit") || arg.is_spread() {
        return Err("parallel(...) requires a named `limit = N` argument".to_owned());
    }
    let Expr::Literal(arcweft_lang_hir::syntax::expr::Literal::Int { value, .. }) = arg.value()
    else {
        return Err("parallel limit must be an integer literal".to_owned());
    };
    usize::try_from(*value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| "parallel limit must be greater than zero".to_owned())
}

#[cfg(test)]
mod tests;
