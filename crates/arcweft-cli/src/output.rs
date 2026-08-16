use crate::app::project::{CheckedModule, verify_compiled_project};
use arcweft_core::aot::AotProgramStats;
use arcweft_core::awbc::schema::AwbcProgram;
use arcweft_core::effect::{LineEffectRequest, RuntimeAssertionFailure};
use arcweft_core::engine::{FlowFiber, FlowStatusLabelStyle};
use arcweft_core::line_task::{LineTaskGroup, LineTaskNode, LineTaskTrigger, ScopeExit};
use arcweft_core::plan::FlowEvent;
use arcweft_core::step::{RuntimePureCallStats, RuntimeStepResult, RuntimeStepStats};
use arcweft_core::stream::{RuntimeStreamEvent, StreamEventKind, StreamOp};
use arcweft_core::task::TaskSpec;
use arcweft_core::value::RuntimePayload;
use arcweft_lang_sema::final_analysis::FinalSemanticAnalysis;
use arcweft_lang_syntax::incremental::SyntaxParseStats;
use arcweft_runtime_host::{
    HostSystemInfo, NativeTaskStats, RuntimeExecutorPureCompileStatsSummary,
    RuntimeExecutorPureConfigSummary, RuntimeExecutorStats,
};
use arcweft_runtime_plan::flow::{RuntimePlanLowerReport, RuntimePlanLowerStats};
use arcweft_test::{ScriptBench, ScriptTest};
use arcweft_text_model::DialogueContentSpec;
use arcweft_tooling::runtime_diagnostic::{
    RuntimeAssertionDiagnosticIdentity, project_runtime_assertion_fault,
};
use arcweft_verify::{BackendKind, VerificationMode, VerificationPolicy};
use std::process::ExitCode;

#[derive(serde::Serialize)]
pub(crate) struct RuntimePlanReport {
    pub(crate) lines: Vec<RuntimeLinePlanSummary>,
    pub(crate) dialogue_content_catalog: Vec<DialogueContentSpec>,
    pub(crate) streams: Vec<RuntimeStreamPlanSummary>,
    pub(crate) verifier_diagnostics: usize,
    pub(crate) verifier_obligations: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeStreamPlanSummary {
    pub(crate) id: String,
    pub(crate) item_ty: String,
    pub(crate) error_ty: String,
    pub(crate) ops: usize,
    pub(crate) yields: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeLinePlanSummary {
    pub(crate) child_tasks: usize,
    pub(crate) effects: usize,
    pub(crate) root: RuntimeNodeSummary,
    pub(crate) captures: usize,
    pub(crate) nodes: usize,
    pub(crate) cancel_rules: usize,
    pub(crate) cleanup_actions: usize,
}

#[derive(serde::Serialize)]
struct RuntimeScopeSummary {
    node: Box<RuntimeNodeSummary>,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeNodeSummary {
    pub(crate) kind: String,
    children: Vec<RuntimeNodeSummary>,
    task: Option<Box<RuntimeTaskSummary>>,
    effect: Option<String>,
}

#[derive(serde::Serialize)]
struct RuntimeTaskSummary {
    id: String,
    key: Option<String>,
    name: Option<String>,
    trigger: String,
    priority: i32,
    join_policy: String,
    cancel_policy: String,
    scope: Box<RuntimeScopeSummary>,
}

impl RuntimePlanReport {
    pub(crate) fn from_lowered(
        checked: &CheckedModule,
        runtime_report: &RuntimePlanLowerReport,
    ) -> Result<Self, ExitCode> {
        let verification = verify_compiled_project(
            &checked.compiled,
            VerificationPolicy {
                mode: VerificationMode::Dev,
                backend: BackendKind::Emit,
                allow_trusted_proofs: true,
            },
        )?;
        Ok(Self {
            lines: checked
                .runtime_plan()
                .plan
                .line_task_groups()
                .iter()
                .map(RuntimeLinePlanSummary::from_lowered)
                .collect(),
            dialogue_content_catalog: runtime_report.dialogue_content_catalog.records().to_vec(),
            streams: runtime_report
                .plan
                .stream_plans()
                .iter()
                .map(|stream| RuntimeStreamPlanSummary {
                    id: stream.id().public_label().into_string(),
                    item_ty: stream.item_ty().to_string(),
                    error_ty: stream.error_ty().to_string(),
                    ops: stream.ops().len(),
                    yields: stream.ops().iter().map(count_stream_yields).sum(),
                })
                .collect(),
            verifier_diagnostics: verification.diagnostics.len(),
            verifier_obligations: verification.obligations.len(),
        })
    }
}

fn count_stream_yields(op: &StreamOp) -> usize {
    match op {
        StreamOp::Yield { .. } => 1,
        StreamOp::ForNext { body, .. } => body.iter().map(count_stream_yields).sum(),
        StreamOp::If {
            then_ops, else_ops, ..
        } => then_ops
            .iter()
            .chain(else_ops)
            .map(count_stream_yields)
            .sum(),
        StreamOp::Match { arms, .. } => arms
            .iter()
            .flat_map(|arm| &arm.ops)
            .map(count_stream_yields)
            .sum(),
        StreamOp::Let { .. } | StreamOp::Close { .. } | StreamOp::Return => 0,
    }
}

impl RuntimeLinePlanSummary {
    fn from_lowered(group: &LineTaskGroup) -> Self {
        let root = node_summary(group, group.root());
        Self {
            child_tasks: count_child_tasks(group),
            effects: count_effects(group),
            root,
            captures: group.captures().len(),
            nodes: group.nodes().len(),
            cancel_rules: group.cancel_rules().len(),
            cleanup_actions: [
                ScopeExit::Completed,
                ScopeExit::Cancelled,
                ScopeExit::Failed,
            ]
            .into_iter()
            .map(|exit| group.cleanup().actions(exit).len())
            .sum(),
        }
    }
}

fn scope_summary(
    group: &LineTaskGroup,
    node: arcweft_core::runtime_id::RuntimeLineTaskNodeId,
) -> RuntimeScopeSummary {
    RuntimeScopeSummary {
        node: Box::new(node_summary(group, node)),
    }
}

fn node_summary(
    group: &LineTaskGroup,
    node: arcweft_core::runtime_id::RuntimeLineTaskNodeId,
) -> RuntimeNodeSummary {
    let Some(node) = group.node(node) else {
        return RuntimeNodeSummary {
            kind: "invalid".to_owned(),
            children: Vec::new(),
            task: None,
            effect: None,
        };
    };
    match node {
        LineTaskNode::Sequence(children) => node_children_summary(group, "sequence", children),
        LineTaskNode::Start(children) => node_children_summary(group, "start", children),
        LineTaskNode::Parallel { children, .. } => {
            node_children_summary(group, "parallel", children)
        }
        LineTaskNode::Child {
            id,
            key,
            name,
            trigger,
            priority,
            join_policy,
            cancel_policy,
            scope,
        } => RuntimeNodeSummary {
            kind: "child".to_owned(),
            children: Vec::new(),
            task: Some(Box::new(RuntimeTaskSummary {
                id: id.0.clone(),
                key: key.as_ref().map(|key| key.0.clone()),
                name: name.clone(),
                trigger: trigger_label(trigger),
                priority: priority.0,
                join_policy: format!("{join_policy:?}"),
                cancel_policy: format!("{cancel_policy:?}"),
                scope: Box::new(scope_summary(group, *scope)),
            })),
            effect: None,
        },
        LineTaskNode::Action(ops) => RuntimeNodeSummary {
            kind: "action".to_owned(),
            children: Vec::new(),
            task: None,
            effect: Some(format!("{} flow ops", ops.len())),
        },
    }
}

fn node_children_summary(
    group: &LineTaskGroup,
    kind: &str,
    children: &[arcweft_core::runtime_id::RuntimeLineTaskNodeId],
) -> RuntimeNodeSummary {
    RuntimeNodeSummary {
        kind: kind.to_owned(),
        children: children
            .iter()
            .map(|child| node_summary(group, *child))
            .collect(),
        task: None,
        effect: None,
    }
}

fn trigger_label(trigger: &LineTaskTrigger) -> String {
    match trigger {
        LineTaskTrigger::Immediate => "immediate".to_owned(),
        LineTaskTrigger::Mark(name) => format!("mark {name}"),
        LineTaskTrigger::Delay(duration) => format!("delay {}ns", duration.as_nanos()),
    }
}

fn effect_label(effect: &LineEffectRequest) -> String {
    match effect {
        LineEffectRequest::RegisterHandle { key, .. } => format!("register {key}"),
        LineEffectRequest::DropHandle { key } => format!("drop {key}"),
        LineEffectRequest::Wait(target) => wait_target_label(target),
        LineEffectRequest::Call(call) => format!("call {}", call.callee),
        LineEffectRequest::Log(log) => format!("log.{}", log.level),
        LineEffectRequest::SignalWrite(write) => format!("signal.set {}", write.target),
        LineEffectRequest::MetricWrite(write) => format!("metric.set {}", write.target),
        LineEffectRequest::EmitEvent(event) => format!("event.emit {}", event.event),
        LineEffectRequest::Out(_) => "out".to_owned(),
        LineEffectRequest::Return(_) => "return".to_owned(),
        LineEffectRequest::Goto(_) => "goto".to_owned(),
        LineEffectRequest::Panic(_) => "panic".to_owned(),
        LineEffectRequest::Fail(_) => "fail".to_owned(),
        LineEffectRequest::Bail(_) => "bail".to_owned(),
        LineEffectRequest::Ensure { .. } => "ensure".to_owned(),
        LineEffectRequest::Assert(assertion) => match assertion.profile() {
            arcweft_core::effect::RuntimeAssertionProfile::Always => "assert".to_owned(),
            arcweft_core::effect::RuntimeAssertionProfile::DebugOnly => "debug_assert".to_owned(),
        },
        LineEffectRequest::Close(_) => "close".to_owned(),
        LineEffectRequest::Select(_) => "select".to_owned(),
        LineEffectRequest::Break { .. } => "break".to_owned(),
        LineEffectRequest::Continue { .. } => "continue".to_owned(),
        LineEffectRequest::Audio(command) => format!("audio.{}", command.operation_name()),
    }
}

fn wait_target_label(target: &arcweft_core::effect::RuntimeWaitTarget) -> String {
    match target {
        arcweft_core::effect::RuntimeWaitTarget::Duration(duration) => {
            format!("wait({}ns)", duration.as_nanos())
        }
        arcweft_core::effect::RuntimeWaitTarget::Mark(mark) => format!("wait(mark({mark}))"),
        arcweft_core::effect::RuntimeWaitTarget::Expr(expr) => format!("wait({expr})"),
    }
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeRunReport {
    pub(crate) host_system: HostSystemInfo,
    pub(crate) executor: RuntimeExecutorTier,
    pub(crate) executor_stats: RuntimeExecutorStats,
    pub(crate) native_io: NativeTaskStats,
    pub(crate) steps: Vec<RuntimeStepRunSummary>,
    pub(crate) final_status: String,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeProfileReport {
    pub(crate) source: String,
    pub(crate) syntax_warnings: usize,
    pub(crate) line_task_groups: usize,
    pub(crate) compiler: RuntimeProfileCompiler,
    pub(crate) phases: Vec<RuntimeProfilePhase>,
    pub(crate) runtime: RuntimeProfileRuntime,
}

#[derive(serde::Serialize)]
pub(crate) struct VerifyTypesReport {
    pub(crate) status: String,
    pub(crate) source: String,
    pub(crate) syntax_warnings: usize,
    pub(crate) line_task_groups: usize,
    pub(crate) phases: Vec<RuntimeProfilePhase>,
    pub(crate) semantic: FinalSemanticProfileStats,
    pub(crate) verifier: VerifyTypesVerifierSummary,
    pub(crate) runtime: Option<VerifyTypesRuntimeSelfCheck>,
}

#[derive(serde::Serialize)]
pub(crate) struct VerifyTypesVerifierSummary {
    pub(crate) diagnostics: usize,
    pub(crate) obligations: usize,
    pub(crate) unsafe_audits: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct VerifyTypesRuntimeSelfCheck {
    pub(crate) executor: RuntimeExecutorTier,
    pub(crate) executor_stats: RuntimeExecutorStats,
    pub(crate) native_io: NativeTaskStats,
    pub(crate) steps_run: usize,
    pub(crate) final_status: String,
    pub(crate) diagnostics: usize,
    pub(crate) failed: bool,
    pub(crate) steps: Vec<RuntimeStepRunSummary>,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptBenchRunReport {
    pub(crate) source: String,
    pub(crate) syntax_warnings: usize,
    pub(crate) line_task_groups: usize,
    pub(crate) compiler: RuntimeProfileCompiler,
    pub(crate) phases: Vec<RuntimeProfilePhase>,
    pub(crate) benches: Vec<ScriptBenchRunSummary>,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeProfileCompiler {
    pub(crate) syntax: SyntaxProfileStats,
    pub(crate) semantic: FinalSemanticProfileStats,
    pub(crate) runtime_plan: RuntimePlanProfileStats,
    pub(crate) awbc: AwbcProfileStats,
    pub(crate) aot: AotProfileStats,
}

#[derive(Clone, Copy, serde::Serialize)]
pub(crate) struct RuntimePlanProfileStats {
    pub(crate) pure_helpers: usize,
    pub(crate) pure_candidate_functions_seen: usize,
    pub(crate) pure_candidate_lower_attempts: usize,
    pub(crate) pure_candidate_lower_failures_inferred: usize,
    pub(crate) pure_expr_lowered_nodes: usize,
    pub(crate) pure_expr_cloned_nodes: usize,
    pub(crate) pure_rewrite_expr_visits: usize,
    pub(crate) optimized_flows: usize,
    pub(crate) optimized_op_slices: usize,
    pub(crate) local_use_tail_scans: usize,
    pub(crate) local_use_scan_ops: usize,
    pub(crate) sequence_map_sum_fusions: usize,
    pub(crate) map_sum_fusions: usize,
    pub(crate) sequence_source_inlines: usize,
    pub(crate) pure_call_exprs: usize,
}

impl From<RuntimePlanLowerStats> for RuntimePlanProfileStats {
    fn from(stats: RuntimePlanLowerStats) -> Self {
        Self {
            pure_helpers: stats.pure_helpers,
            pure_candidate_functions_seen: stats.pure_candidate_functions_seen,
            pure_candidate_lower_attempts: stats.pure_candidate_lower_attempts,
            pure_candidate_lower_failures_inferred: stats.pure_candidate_lower_failures_inferred,
            pure_expr_lowered_nodes: stats.pure_expr_lowered_nodes,
            pure_expr_cloned_nodes: stats.pure_expr_cloned_nodes,
            pure_rewrite_expr_visits: stats.pure_rewrite_expr_visits,
            optimized_flows: stats.optimized_flows,
            optimized_op_slices: stats.optimized_op_slices,
            local_use_tail_scans: stats.local_use_tail_scans,
            local_use_scan_ops: stats.local_use_scan_ops,
            sequence_map_sum_fusions: stats.sequence_map_sum_fusions,
            map_sum_fusions: stats.map_sum_fusions,
            sequence_source_inlines: stats.sequence_source_inlines,
            pure_call_exprs: stats.pure_call_exprs,
        }
    }
}

#[derive(Clone, Copy, serde::Serialize)]
pub(crate) struct SyntaxProfileStats {
    pub(crate) accepted_source_bytes: usize,
    pub(crate) lexer_tokens: usize,
    pub(crate) grammar_events: usize,
    pub(crate) top_level_items: usize,
    pub(crate) statements: usize,
    pub(crate) expressions: usize,
    pub(crate) type_nodes: usize,
    pub(crate) pattern_nodes: usize,
    pub(crate) identity_bearing_nodes: usize,
    pub(crate) diagnostic_identities: usize,
}

impl From<SyntaxParseStats> for SyntaxProfileStats {
    fn from(stats: SyntaxParseStats) -> Self {
        Self {
            accepted_source_bytes: stats.accepted_source_bytes(),
            lexer_tokens: stats.lexer_tokens(),
            grammar_events: stats.grammar_events(),
            top_level_items: stats.top_level_items(),
            statements: stats.statements(),
            expressions: stats.expressions(),
            type_nodes: stats.type_nodes(),
            pattern_nodes: stats.pattern_nodes(),
            identity_bearing_nodes: stats.identity_bearing_nodes(),
            diagnostic_identities: stats.diagnostic_identities(),
        }
    }
}

#[derive(Clone, Copy, serde::Serialize)]
pub(crate) struct FinalSemanticProfileStats {
    pub(crate) types: usize,
    pub(crate) locals: usize,
    pub(crate) captures: usize,
    pub(crate) expressions: usize,
    pub(crate) patterns: usize,
    pub(crate) statements: usize,
    pub(crate) items: usize,
    pub(crate) calls: usize,
    pub(crate) call_diagnostics: u64,
    pub(crate) logical_argument_checks: u64,
    pub(crate) resolver_invocations: u64,
    pub(crate) candidate_argument_probes: u64,
    pub(crate) selected_replay_argument_visits: u64,
    pub(crate) retained_argument_fact_publications: u64,
}

impl From<&FinalSemanticAnalysis> for FinalSemanticProfileStats {
    fn from(analysis: &FinalSemanticAnalysis) -> Self {
        let work = analysis.work();
        Self {
            types: analysis.types().len(),
            locals: analysis.locals().len(),
            captures: analysis.captures().len(),
            expressions: analysis.expressions().len(),
            patterns: analysis.patterns().len(),
            statements: analysis.statements().len(),
            items: analysis.items().len(),
            calls: analysis.calls().len(),
            call_diagnostics: work.call_diagnostics(),
            logical_argument_checks: work.logical_argument_checks(),
            resolver_invocations: work.resolver_invocations(),
            candidate_argument_probes: work.candidate_argument_probes(),
            selected_replay_argument_visits: work.selected_replay_argument_visits(),
            retained_argument_fact_publications: work.retained_argument_fact_publications(),
        }
    }
}

#[derive(serde::Serialize)]
pub(crate) struct AwbcProfileStats {
    flows: usize,
    instructions: usize,
    line_task_groups: usize,
    stream_plans: usize,
}

impl From<&AwbcProgram> for AwbcProfileStats {
    fn from(program: &AwbcProgram) -> Self {
        Self {
            flows: program.flow_executables.len(),
            instructions: program.instructions.len(),
            line_task_groups: program.line_task_groups.len(),
            stream_plans: program.stream_plans.len(),
        }
    }
}

#[derive(serde::Serialize)]
pub(crate) struct AotProfileStats {
    flows: usize,
    ops: usize,
    linear_ops: usize,
    branch_ops: usize,
    effect_ops: usize,
    await_ops: usize,
    choice_ops: usize,
    dialogue_ops: usize,
    jump_ops: usize,
    linear_dispatch_flows: usize,
    mixed_dispatch_flows: usize,
}

impl From<&AotProgramStats> for AotProfileStats {
    fn from(stats: &AotProgramStats) -> Self {
        Self {
            flows: stats.flows,
            ops: stats.ops,
            linear_ops: stats.linear_ops,
            branch_ops: stats.branch_ops,
            effect_ops: stats.effect_ops,
            await_ops: stats.await_ops,
            choice_ops: stats.choice_ops,
            dialogue_ops: stats.dialogue_ops,
            jump_ops: stats.jump_ops,
            linear_dispatch_flows: stats.linear_dispatch_flows,
            mixed_dispatch_flows: stats.mixed_dispatch_flows,
        }
    }
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct RuntimeProfilePhase {
    pub(crate) name: &'static str,
    pub(crate) elapsed_ns: u128,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeProfileRuntime {
    pub(crate) host_system: HostSystemInfo,
    pub(crate) executor: RuntimeExecutorTier,
    pub(crate) executor_stats: RuntimeExecutorStats,
    pub(crate) native_io: NativeTaskStats,
    pub(crate) steps: Vec<RuntimeStepRunSummary>,
    pub(crate) final_status: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuntimeExecutorTier {
    AwbcProduct,
    BytecodeVm,
    Aot,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptTestRunReport {
    pub(crate) tests: Vec<ScriptTestRunSummary>,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptTestRunSummary {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) status: ScriptTestStatus,
    pub(crate) steps_run: usize,
    pub(crate) final_status: Option<ScriptTestFinalStatus>,
    pub(crate) diagnostics: Vec<String>,
    pub(crate) steps: Vec<RuntimeStepRunSummary>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ScriptTestStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ScriptTestFinalStatus {
    NotStarted,
    AdapterError,
    Flow(String),
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptBenchRunSummary {
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) sections: Vec<ScriptBenchSectionRunSummary>,
    pub(crate) diagnostics: Vec<String>,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptBenchSectionRunSummary {
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) diagnostics: Vec<String>,
    pub(crate) measurement: Option<ScriptBenchMeasurementSummary>,
    pub(crate) pure_helper: Option<ScriptBenchPureHelperMeasurementSummary>,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptBenchMeasurementSummary {
    pub(crate) host_system: HostSystemInfo,
    pub(crate) executor: RuntimeExecutorTier,
    pub(crate) executor_stats: RuntimeExecutorStats,
    pub(crate) native_io: NativeTaskStats,
    pub(crate) warmup: usize,
    pub(crate) iterations: usize,
    pub(crate) steps: usize,
    pub(crate) per_executed_op_ns: u128,
    pub(crate) elapsed_ns: ScriptBenchElapsedSummary,
    pub(crate) deterministic: ScriptBenchDeterministicSummary,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptBenchElapsedSummary {
    pub(crate) min: u128,
    pub(crate) median: u128,
    pub(crate) max: u128,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptBenchDeterministicSummary {
    pub(crate) executed_ops_median: usize,
    pub(crate) child_fiber_ticks_median: usize,
    pub(crate) max_child_fibers_median: usize,
    pub(crate) line_effects_median: usize,
    pub(crate) task_requests_median: usize,
    pub(crate) task_events_in_median: usize,
    pub(crate) pure_calls_median: usize,
    pub(crate) math_calls_median: usize,
    pub(crate) math_accelerated_calls_median: usize,
    pub(crate) pure_batch_calls_median: usize,
    pub(crate) pure_batch_items_median: usize,
    pub(crate) pure_flat_batch_calls_median: usize,
    pub(crate) pure_flat_batch_items_median: usize,
    pub(crate) pure_flat_batch_bytes_borrowed_median: usize,
    pub(crate) pure_flatten_materializations_median: usize,
    pub(crate) pure_flatten_bytes_copied_median: usize,
    pub(crate) pure_jit_calls_median: usize,
    pub(crate) pure_aot_calls_median: usize,
    pub(crate) pure_vm_calls_median: usize,
    pub(crate) pure_parallel_policy_checks_median: usize,
    pub(crate) pure_parallel_work_units_median: usize,
    pub(crate) pure_parallel_batches_median: usize,
    pub(crate) pure_parallel_skipped_backend_median: usize,
    pub(crate) pure_parallel_skipped_small_median: usize,
    pub(crate) pure_thread_pool_jobs_median: usize,
    pub(crate) pure_thread_pool_build_elapsed_ns_median: u128,
    pub(crate) pure_arg_stack_packs_median: usize,
    pub(crate) pure_arg_vec_allocations_median: usize,
    pub(crate) pure_arg_bytes_copied_median: usize,
    pub(crate) pure_arg_bytes_borrowed_median: usize,
    pub(crate) pure_result_bytes_copied_median: usize,
    pub(crate) pure_fallbacks_median: usize,
    pub(crate) diagnostics: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptBenchPureHelperMeasurementSummary {
    pub(crate) host_system: HostSystemInfo,
    pub(crate) helper: String,
    pub(crate) input_bindings: Vec<String>,
    pub(crate) matches_vm: bool,
    pub(crate) warmup: usize,
    pub(crate) iterations: usize,
    pub(crate) samples: usize,
    pub(crate) timings: ScriptBenchPureHelperTimingSummary,
    pub(crate) jit_batch: ScriptBenchPureHelperBatchSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) runtime_batch: Option<ScriptBenchPureHelperRuntimeBatchSummary>,
    pub(crate) deterministic: ScriptBenchPureHelperDeterministicSummary,
    pub(crate) vm_stats: ScriptBenchPureHelperStatsSummary,
    pub(crate) aot_stats: ScriptBenchPureHelperStatsSummary,
    pub(crate) jit_stats: ScriptBenchPureHelperStatsSummary,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptBenchPureHelperTimingSummary {
    pub(crate) aot_compile_elapsed_ns: u128,
    pub(crate) compile_elapsed_ns: u128,
    pub(crate) aot_elapsed_ns: u128,
    pub(crate) jit_elapsed_ns: u128,
    pub(crate) vm_elapsed_ns: u128,
    pub(crate) aot_per_iteration_ns: u128,
    pub(crate) jit_per_iteration_ns: u128,
    pub(crate) vm_per_iteration_ns: u128,
    pub(crate) aot_speedup_x: String,
    pub(crate) speedup_x: String,
    pub(crate) aot_samples: ScriptBenchPureHelperTimingSamples,
    pub(crate) jit_samples: ScriptBenchPureHelperTimingSamples,
    pub(crate) vm_samples: ScriptBenchPureHelperTimingSamples,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptBenchPureHelperBatchSummary {
    pub(crate) compile_elapsed_ns: u128,
    pub(crate) elapsed_ns: u128,
    pub(crate) per_iteration_ns: u128,
    pub(crate) speedup_x: String,
    pub(crate) jit_call_speedup_x: String,
    pub(crate) samples: ScriptBenchPureHelperTimingSamples,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptBenchPureHelperRuntimeBatchSummary {
    pub(crate) matches_vm: bool,
    pub(crate) accumulator: i64,
    pub(crate) elapsed_ns: u128,
    pub(crate) per_iteration_ns: u128,
    pub(crate) speedup_x: String,
    pub(crate) samples: ScriptBenchPureHelperTimingSamples,
    pub(crate) config: RuntimeExecutorPureConfigSummary,
    pub(crate) compile: RuntimeExecutorPureCompileStatsSummary,
    pub(crate) stats: RuntimePureCallStatsSummary,
}

#[derive(Clone, Copy, serde::Serialize)]
pub(crate) struct ScriptBenchPureHelperTimingSamples {
    pub(crate) min: u128,
    pub(crate) median: u128,
    pub(crate) max: u128,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptBenchPureHelperDeterministicSummary {
    #[serde(rename = "aot_accumulator")]
    pub(crate) aot: i64,
    #[serde(rename = "jit_accumulator")]
    pub(crate) jit: i64,
    #[serde(rename = "jit_batch_accumulator")]
    pub(crate) jit_batch: i64,
    #[serde(rename = "vm_accumulator")]
    pub(crate) vm: i64,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptBenchPureHelperStatsSummary {
    #[serde(rename = "evaluated_exprs")]
    pub(crate) exprs: usize,
    #[serde(rename = "evaluated_calls")]
    pub(crate) calls: usize,
    #[serde(rename = "evaluated_binary_ops")]
    pub(crate) binary_ops: usize,
}

impl ScriptBenchRunSummary {
    pub(crate) fn new(
        bench: &ScriptBench,
        status: impl Into<String>,
        sections: Vec<ScriptBenchSectionRunSummary>,
        diagnostics: Vec<String>,
    ) -> Self {
        Self {
            id: bench.id.clone(),
            status: status.into(),
            sections,
            diagnostics,
        }
    }
}

impl ScriptBenchSectionRunSummary {
    pub(crate) fn new(
        name: impl Into<String>,
        status: impl Into<String>,
        diagnostics: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: status.into(),
            diagnostics,
            measurement: None,
            pure_helper: None,
        }
    }

    pub(crate) fn measured(
        name: impl Into<String>,
        diagnostics: Vec<String>,
        measurement: ScriptBenchMeasurementSummary,
    ) -> Self {
        Self {
            name: name.into(),
            status: "measured".to_owned(),
            diagnostics,
            measurement: Some(measurement),
            pure_helper: None,
        }
    }
}

impl ScriptTestRunSummary {
    pub(crate) fn skipped(test: &ScriptTest, reason: impl Into<String>) -> Self {
        Self {
            id: test.id.clone(),
            kind: test.kind.clone(),
            status: ScriptTestStatus::Skipped,
            steps_run: 0,
            final_status: None,
            diagnostics: vec![reason.into()],
            steps: Vec::new(),
        }
    }

    pub(crate) fn completed(
        test: &ScriptTest,
        passed: bool,
        final_status: ScriptTestFinalStatus,
        diagnostics: Vec<String>,
        steps: Vec<RuntimeStepRunSummary>,
    ) -> Self {
        Self {
            id: test.id.clone(),
            kind: test.kind.clone(),
            status: if passed {
                ScriptTestStatus::Passed
            } else {
                ScriptTestStatus::Failed
            },
            steps_run: steps.len(),
            final_status: Some(final_status),
            diagnostics,
            steps,
        }
    }
}

impl ScriptTestFinalStatus {
    fn as_str(&self) -> &str {
        match self {
            Self::NotStarted => "not_started",
            Self::AdapterError => "adapter_error",
            Self::Flow(status) => status,
        }
    }
}

impl std::fmt::Display for ScriptTestStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        };
        f.write_str(label)
    }
}

impl serde::Serialize for ScriptTestFinalStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeStepRunSummary {
    pub(crate) index: usize,
    pub(crate) stop_reason: String,
    pub(crate) fiber_status: String,
    pub(crate) stats: RuntimeStepStatsSummary,
    pub(crate) diagnostics: Vec<String>,
    /// Typed runtime assertion failures are the sole assertion-presence
    /// authority for CLI expectations and higher-level consumers.
    pub(crate) assertion_failures: Vec<RuntimeAssertionFailure>,
    /// Fresh-session presentation derived from the exact accepted runtime-plan
    /// artifact and its retained assertion inventory.
    pub(crate) assertion_diagnostics: Vec<RuntimeAssertionRunDiagnostic>,
    pub(crate) flow_events: Vec<String>,
    pub(crate) line_effects: Vec<String>,
    pub(crate) task_requests: Vec<String>,
    pub(crate) observations: RuntimeObservationSummary,
    pub(crate) stream_events: Vec<String>,
    pub(crate) stream_states: Vec<RuntimeQueueStateSummary>,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeAssertionRunDiagnostic {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) identity: &'static str,
    pub(crate) mode: &'static str,
    pub(crate) condition_index: u8,
}

impl RuntimeAssertionRunDiagnostic {
    fn from_session_failure(
        context: &arcweft_compiler::runtime_diagnostics::ExecutionDiagnosticContext,
        failure: RuntimeAssertionFailure,
    ) -> Result<Self, arcweft_runtime_plan::assertion_identity::RuntimeAssertionProjectionError>
    {
        let fault = context.project_assertion_failure(failure)?;
        let diagnostic = project_runtime_assertion_fault(&fault);
        let RuntimeAssertionDiagnosticIdentity::Session {
            mode,
            condition_index,
        } = *diagnostic.identity()
        else {
            unreachable!("fresh assertion fault projection always has session identity")
        };
        Ok(Self {
            code: diagnostic.code(),
            message: diagnostic.message().to_owned(),
            identity: "session",
            mode: mode.as_str(),
            condition_index,
        })
    }
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeStepStatsSummary {
    pub(crate) executed_ops: usize,
    pub(crate) pending_ops_before: usize,
    pub(crate) pending_ops_after: usize,
    pub(crate) child_fibers: usize,
    pub(crate) pure: RuntimePureCallStatsSummary,
    pub(crate) task_events_in: usize,
    pub(crate) stream_events_emitted: usize,
    pub(crate) line_effects: usize,
    pub(crate) diagnostics: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimePureCallStatsSummary {
    pub(crate) pure_calls: usize,
    pub(crate) math_calls: usize,
    pub(crate) math_accelerated_calls: usize,
    pub(crate) batch_calls: usize,
    pub(crate) batch_items: usize,
    pub(crate) flat_batch_calls: usize,
    pub(crate) flat_batch_items: usize,
    pub(crate) flat_batch_bytes_borrowed: usize,
    pub(crate) flatten_materializations: usize,
    pub(crate) flatten_bytes_copied: usize,
    pub(crate) jit_calls: usize,
    pub(crate) aot_calls: usize,
    pub(crate) vm_calls: usize,
    pub(crate) arg_stack_packs: usize,
    pub(crate) arg_vec_allocations: usize,
    pub(crate) arg_bytes_copied: usize,
    pub(crate) arg_bytes_borrowed: usize,
    pub(crate) result_bytes_copied: usize,
    pub(crate) parallel_policy_checks: usize,
    pub(crate) parallel_work_units: usize,
    pub(crate) parallel_batches: usize,
    pub(crate) parallel_skipped_backend: usize,
    pub(crate) parallel_skipped_small: usize,
    pub(crate) thread_pool_jobs: usize,
    pub(crate) thread_pool_build_elapsed_ns: u128,
    pub(crate) fallbacks: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeObservationSummary {
    pub(crate) signals: Vec<RuntimeObservedAssignment>,
    pub(crate) metrics: Vec<RuntimeObservedAssignment>,
    pub(crate) logs: Vec<RuntimeObservedLog>,
    pub(crate) events: Vec<RuntimeObservedEvent>,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeObservedAssignment {
    pub(crate) target: String,
    pub(crate) value: String,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeObservedLog {
    pub(crate) level: String,
    pub(crate) message: String,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeObservedEvent {
    pub(crate) event: String,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeQueueStateSummary {
    pub(crate) id: String,
    pub(crate) queue_depth: usize,
    pub(crate) closed: bool,
    pub(crate) overflow_count: u64,
}

impl RuntimeStepRunSummary {
    pub(crate) fn from_result_and_task_requests(
        index: usize,
        result: RuntimeStepResult,
        fiber: &FlowFiber,
        execution_diagnostics: &arcweft_compiler::runtime_diagnostics::ExecutionDiagnosticContext,
    ) -> Result<
        (Self, Vec<TaskSpec>),
        arcweft_runtime_plan::assertion_identity::RuntimeAssertionProjectionError,
    > {
        let RuntimeStepResult {
            mut output,
            fiber_status,
            stop_reason,
            stats,
        } = result;
        let task_requests = std::mem::take(&mut output.requests.tasks);
        let assertion_failures = output
            .effects
            .line
            .iter()
            .filter_map(|effect| match effect {
                LineEffectRequest::Assert(assertion) => {
                    Some(RuntimeAssertionFailure::new(assertion.clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let assertion_diagnostics = assertion_failures
            .iter()
            .cloned()
            .map(|failure| {
                RuntimeAssertionRunDiagnostic::from_session_failure(execution_diagnostics, failure)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let summary = Self {
            index,
            stop_reason: format!("{stop_reason:?}"),
            fiber_status: fiber_status.status_label(FlowStatusLabelStyle::Debug),
            stats: RuntimeStepStatsSummary::from(stats),
            diagnostics: output
                .diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.message)
                .collect(),
            assertion_failures,
            assertion_diagnostics,
            flow_events: output.flow_events.iter().map(flow_event_label).collect(),
            line_effects: output.effects.line.iter().map(effect_label).collect(),
            task_requests: task_requests.iter().map(task_request_label).collect(),
            observations: RuntimeObservationSummary {
                signals: fiber
                    .observations
                    .signals
                    .iter()
                    .map(|(target, value)| RuntimeObservedAssignment {
                        target: target.clone(),
                        value: value.clone(),
                    })
                    .collect(),
                metrics: fiber
                    .observations
                    .metrics
                    .iter()
                    .map(|(target, value)| RuntimeObservedAssignment {
                        target: target.clone(),
                        value: value.clone(),
                    })
                    .collect(),
                logs: fiber
                    .observations
                    .logs
                    .iter()
                    .map(|log| RuntimeObservedLog {
                        level: log.level.clone(),
                        message: log.message.clone(),
                    })
                    .collect(),
                events: fiber
                    .observations
                    .events
                    .iter()
                    .map(|event| RuntimeObservedEvent {
                        event: event.event.clone(),
                    })
                    .collect(),
            },
            stream_events: output
                .effects
                .stream_events
                .iter()
                .map(stream_event_label)
                .collect(),
            stream_states: fiber
                .stream_states
                .values()
                .map(|state| RuntimeQueueStateSummary {
                    id: state.id.public_label().into_string(),
                    queue_depth: state.queue.len(),
                    closed: state.closed,
                    overflow_count: 0,
                })
                .collect(),
        };
        Ok((summary, task_requests))
    }
}

impl From<RuntimeStepStats> for RuntimeStepStatsSummary {
    fn from(stats: RuntimeStepStats) -> Self {
        Self {
            executed_ops: stats.executed_ops,
            pending_ops_before: stats.pending_ops_before,
            pending_ops_after: stats.pending_ops_after,
            child_fibers: stats.child_fibers,
            pure: RuntimePureCallStatsSummary::from(stats.pure),
            task_events_in: stats.task_events_in,
            stream_events_emitted: stats.stream_events_emitted,
            line_effects: stats.line_effects,
            diagnostics: stats.diagnostics,
        }
    }
}

impl From<RuntimePureCallStats> for RuntimePureCallStatsSummary {
    fn from(stats: RuntimePureCallStats) -> Self {
        Self {
            pure_calls: stats.pure_calls,
            math_calls: stats.math_calls,
            math_accelerated_calls: stats.math_accelerated_calls,
            batch_calls: stats.batch_calls,
            batch_items: stats.batch_items,
            flat_batch_calls: stats.flat_batch_calls,
            flat_batch_items: stats.flat_batch_items,
            flat_batch_bytes_borrowed: stats.flat_batch_bytes_borrowed,
            flatten_materializations: stats.flatten_materializations,
            flatten_bytes_copied: stats.flatten_bytes_copied,
            jit_calls: stats.jit_calls,
            aot_calls: stats.aot_calls,
            vm_calls: stats.vm_calls,
            arg_stack_packs: stats.arg_stack_packs,
            arg_vec_allocations: stats.arg_vec_allocations,
            arg_bytes_copied: stats.arg_bytes_copied,
            arg_bytes_borrowed: stats.arg_bytes_borrowed,
            result_bytes_copied: stats.result_bytes_copied,
            parallel_policy_checks: stats.parallel_policy_checks,
            parallel_work_units: stats.parallel_work_units,
            parallel_batches: stats.parallel_batches,
            parallel_skipped_backend: stats.parallel_skipped_backend,
            parallel_skipped_small: stats.parallel_skipped_small,
            thread_pool_jobs: stats.thread_pool_jobs,
            thread_pool_build_elapsed_ns: stats.thread_pool_build_elapsed_ns,
            fallbacks: stats.fallbacks,
        }
    }
}

fn flow_event_label(event: &FlowEvent) -> String {
    match event {
        FlowEvent::DialogueLine { line, .. } => format!("dialogue {}", line.public_label()),
        FlowEvent::LineCancelled { trigger } => format!("line_cancelled {trigger}"),
        FlowEvent::ChoicePresented { id, .. } => {
            format!("choice_presented {}", id.as_deref().unwrap_or("-"))
        }
        FlowEvent::ChoiceSelected { id, option } => {
            format!("choice_selected {} {option}", id.as_deref().unwrap_or("-"))
        }
        FlowEvent::AwaitStarted { need, task } => format!("await_started {} {}", need.0, task.0),
        FlowEvent::AwaitReady { need, value } => {
            format!("await_ready {} {}", need.0, value.label())
        }
        FlowEvent::AwaitProgress { need, progress } => {
            format!("await_progress {} {}", need.0, progress.label())
        }
        FlowEvent::Goto { target } => format!("goto {}", target.public_label()),
        FlowEvent::Return { value } => format!("return {value}"),
        FlowEvent::Done => "done".to_owned(),
    }
}

fn task_request_label(task: &TaskSpec) -> String {
    format!(
        "{} key={} class={:?} request={}",
        task.id.0, task.key.0, task.class, task.debug_label
    )
}

fn stream_event_label(event: &RuntimeStreamEvent) -> String {
    format!(
        "{} {}",
        event.stream.public_label(),
        event_kind_label(&event.kind)
    )
}

fn event_kind_label(kind: &StreamEventKind<RuntimePayload, RuntimePayload>) -> String {
    match kind {
        StreamEventKind::Item(item) => format!("item {}", item.label()),
        StreamEventKind::Error(error) => format!("error {}", error.label()),
        StreamEventKind::End => "end".to_owned(),
    }
}

fn count_child_tasks(group: &LineTaskGroup) -> usize {
    group
        .nodes()
        .iter()
        .filter(|node| matches!(node, LineTaskNode::Child { .. }))
        .count()
}

fn count_effects(group: &LineTaskGroup) -> usize {
    group
        .nodes()
        .iter()
        .filter_map(|node| match node {
            LineTaskNode::Action(ops) => Some(ops.len()),
            _ => None,
        })
        .sum::<usize>()
        + group
            .cancel_rules()
            .iter()
            .map(|rule| rule.action().len())
            .sum::<usize>()
        + [
            ScopeExit::Completed,
            ScopeExit::Cancelled,
            ScopeExit::Failed,
        ]
        .into_iter()
        .map(|exit| group.cleanup().actions(exit).len())
        .sum::<usize>()
}
