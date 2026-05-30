use crate::CheckedModule;
use crate::native_task::NativeTaskStats;
use arcweft_core::aot::AotProgramStats;
use arcweft_core::bytecode::BytecodeStats;
use arcweft_core::effect::LineEffectRequest;
use arcweft_core::engine::{FlowFiber, FlowFiberStatus};
use arcweft_core::line_task::{LineTaskGroup, LineTaskNode, LineTaskScope, LineTaskTrigger};
use arcweft_core::plan::FlowEvent;
use arcweft_core::source::{RuntimeSourceEvent, SourceEventKind, SourcePolicy};
use arcweft_core::step::{RuntimePureCallStats, RuntimeStepResult, RuntimeStepStats};
use arcweft_core::stream::{RuntimeStreamEvent, StreamOp};
use arcweft_core::task::TaskSpec;
use arcweft_core::value::RuntimePayload;
use arcweft_lang_sema::check::{
    TypeCheckReport, TypeCheckStats, TypeJudgment, TypeJudgmentRule, TypeJudgmentSubject,
};
use arcweft_runtime_plan::flow::lower_runtime_plan;
use arcweft_runtime_plan::line_task::LoweredLineTaskGroup;
use arcweft_test::{ScriptBench, ScriptTest};
use arcweft_verify::{
    BackendKind, RuntimeTypeValidationStats, VerificationMode, VerificationPolicy,
    verify_module_with_env,
};

#[derive(serde::Serialize)]
pub(crate) struct CheckReport {
    pub(crate) status: String,
    pub(crate) flows: usize,
    pub(crate) line_task_groups: usize,
    pub(crate) syntax_warnings: usize,
    pub(crate) typecheck: TypeCheckProfileStats,
    pub(crate) borrow_check: BorrowCheckProfileStats,
    pub(crate) phases: Vec<RuntimeProfilePhase>,
    pub(crate) verifier_diagnostics: usize,
    pub(crate) verifier_obligations: usize,
    pub(crate) unsafe_audits: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimePlanReport {
    pub(crate) lines: Vec<RuntimeLinePlanSummary>,
    pub(crate) streams: Vec<RuntimeStreamPlanSummary>,
    pub(crate) sources: Vec<RuntimeSourcePlanSummary>,
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
pub(crate) struct RuntimeSourcePlanSummary {
    pub(crate) id: String,
    pub(crate) item_ty: String,
    pub(crate) error_ty: String,
    pub(crate) policy: RuntimeSourcePolicySummary,
    pub(crate) handlers: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeSourcePolicySummary {
    pub(crate) backpressure: String,
    pub(crate) replay: String,
    pub(crate) privacy: String,
    pub(crate) max_queue: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeLinePlanSummary {
    pub(crate) flow_id: Option<String>,
    pub(crate) line_id: Option<String>,
    pub(crate) callee: String,
    pub(crate) child_tasks: usize,
    pub(crate) effects: usize,
    pub(crate) root: RuntimeNodeSummary,
    pub(crate) options: usize,
    pub(crate) bindings: usize,
    pub(crate) out: usize,
    pub(crate) cancel_rules: usize,
    pub(crate) memo: usize,
    pub(crate) assertions: usize,
}

#[derive(serde::Serialize)]
struct RuntimeScopeSummary {
    node: Box<RuntimeNodeSummary>,
    defer_count: usize,
    completed_defer_count: usize,
    cancelled_defer_count: usize,
    failed_defer_count: usize,
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

impl CheckReport {
    pub(crate) fn from_checked(
        checked: &CheckedModule,
        verification: &arcweft_verify::VerificationReport,
    ) -> Self {
        Self {
            status: if verification.has_errors() {
                "failed"
            } else {
                "ok"
            }
            .to_owned(),
            flows: checked.hir.flows().len(),
            line_task_groups: checked.line_task_groups.len(),
            syntax_warnings: checked.syntax_warnings,
            typecheck: TypeCheckProfileStats::from(&checked.typecheck_report),
            borrow_check: BorrowCheckProfileStats::from(&checked.typecheck_report.stats),
            phases: checked.phases.clone(),
            verifier_diagnostics: verification.diagnostics.len(),
            verifier_obligations: verification.obligations.len(),
            unsafe_audits: verification.unsafe_audit_count(),
        }
    }
}

impl RuntimePlanReport {
    pub(crate) fn from_checked(checked: &CheckedModule) -> Self {
        let verification = verify_module_with_env(
            &checked.hir,
            &checked.env,
            VerificationPolicy {
                mode: VerificationMode::Dev,
                backend: BackendKind::Emit,
            },
        );
        let runtime_plan = lower_runtime_plan(&checked.hir).ok();
        Self {
            lines: checked
                .line_task_groups
                .iter()
                .map(RuntimeLinePlanSummary::from_lowered)
                .collect(),
            streams: runtime_plan
                .as_ref()
                .map(|plan| {
                    plan.stream_plans
                        .iter()
                        .map(|stream| RuntimeStreamPlanSummary {
                            id: stream.id.0.clone(),
                            item_ty: stream.item_ty.clone(),
                            error_ty: stream.error_ty.clone(),
                            ops: stream.ops.len(),
                            yields: stream.ops.iter().map(count_stream_yields).sum(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            sources: runtime_plan
                .as_ref()
                .map(|plan| {
                    plan.source_plans
                        .iter()
                        .map(|source| RuntimeSourcePlanSummary {
                            id: source.id.0.clone(),
                            item_ty: source.item_ty.clone(),
                            error_ty: source.error_ty.clone(),
                            policy: source_policy_summary(&source.policy),
                            handlers: source.handlers.len(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            verifier_diagnostics: verification.diagnostics.len(),
            verifier_obligations: verification.obligations.len(),
        }
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
        StreamOp::Let { .. } | StreamOp::Close { .. } | StreamOp::Return | StreamOp::Noop => 0,
    }
}

fn source_policy_summary(policy: &SourcePolicy) -> RuntimeSourcePolicySummary {
    RuntimeSourcePolicySummary {
        backpressure: format!("{:?}", policy.backpressure),
        replay: format!("{:?}", policy.replay),
        privacy: format!("{:?}", policy.privacy),
        max_queue: policy.max_queue,
    }
}

impl RuntimeLinePlanSummary {
    fn from_lowered(line: &LoweredLineTaskGroup) -> Self {
        let group = line.group();
        let root = node_summary(&group.root.node);
        Self {
            flow_id: line.flow_id().map(|id| id.body().to_owned()),
            line_id: line.line_id().map(|id| id.body().to_owned()),
            callee: line.callee().to_owned(),
            child_tasks: count_child_tasks(group),
            effects: count_effects(group),
            root,
            options: group.options.len(),
            bindings: group.bindings.len(),
            out: group.out.len(),
            cancel_rules: group.cancel_rules.len(),
            memo: group.memo.len(),
            assertions: group.assertions.len(),
        }
    }
}

fn scope_summary(scope: &LineTaskScope) -> RuntimeScopeSummary {
    RuntimeScopeSummary {
        node: Box::new(node_summary(&scope.node)),
        defer_count: scope.defer_stack.len(),
        completed_defer_count: scope.completed_defer_stack.len(),
        cancelled_defer_count: scope.cancelled_defer_stack.len(),
        failed_defer_count: scope.failed_defer_stack.len(),
    }
}

fn node_summary(node: &LineTaskNode) -> RuntimeNodeSummary {
    match node {
        LineTaskNode::Seq(children) => node_children_summary("seq", children),
        LineTaskNode::Start(children) => node_children_summary("start", children),
        LineTaskNode::Parallel { children, .. } => node_children_summary("parallel", children),
        LineTaskNode::Child(task) => RuntimeNodeSummary {
            kind: "child".to_owned(),
            children: Vec::new(),
            task: Some(Box::new(RuntimeTaskSummary {
                id: task.id.0.clone(),
                key: task.key.as_ref().map(|key| key.0.clone()),
                name: task.name.clone(),
                trigger: trigger_label(&task.trigger),
                priority: task.priority.0,
                join_policy: format!("{:?}", task.join_policy),
                cancel_policy: format!("{:?}", task.cancel_policy),
                scope: Box::new(scope_summary(&task.scope)),
            })),
            effect: None,
        },
        LineTaskNode::Effect(effect) => RuntimeNodeSummary {
            kind: "effect".to_owned(),
            children: Vec::new(),
            task: None,
            effect: Some(effect_label(effect)),
        },
    }
}

fn node_children_summary(kind: &str, children: &[LineTaskNode]) -> RuntimeNodeSummary {
    RuntimeNodeSummary {
        kind: kind.to_owned(),
        children: children.iter().map(node_summary).collect(),
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
        LineEffectRequest::Assert(assertion) => match assertion.profile {
            arcweft_core::effect::RuntimeAssertionProfile::Always => "assert".to_owned(),
            arcweft_core::effect::RuntimeAssertionProfile::DebugOnly => "debug_assert".to_owned(),
        },
        LineEffectRequest::Close(_) => "close".to_owned(),
        LineEffectRequest::Select(_) => "select".to_owned(),
        LineEffectRequest::Break { .. } => "break".to_owned(),
        LineEffectRequest::Continue { .. } => "continue".to_owned(),
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
    pub(crate) typecheck: TypeCheckProfileStats,
    pub(crate) borrow_check: BorrowCheckProfileStats,
    pub(crate) runtime_type_validation: RuntimeTypeValidationReportSummary,
    pub(crate) verifier: VerifyTypesVerifierSummary,
    pub(crate) runtime: Option<VerifyTypesRuntimeSelfCheck>,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeTypeValidationReportSummary {
    pub(crate) diagnostics: usize,
    pub(crate) errors: usize,
    pub(crate) stats: RuntimeTypeValidationProfileStats,
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
    pub(crate) typecheck: TypeCheckProfileStats,
    pub(crate) borrow_check: BorrowCheckProfileStats,
    pub(crate) runtime_type_validation: RuntimeTypeValidationProfileStats,
    pub(crate) bytecode: BytecodeProfileStats,
    pub(crate) aot: AotProfileStats,
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct TypeCheckProfileStats {
    pub(crate) flows: usize,
    pub(crate) functions: usize,
    pub(crate) declarations: usize,
    pub(crate) top_level_items: usize,
    pub(crate) statements: usize,
    pub(crate) expressions: usize,
    pub(crate) warnings: usize,
    pub(crate) warning_samples: Vec<String>,
    pub(crate) judgments: usize,
    judgment_rules: TypeCheckJudgmentRuleStats,
    judgment_samples: Vec<TypeCheckJudgmentSample>,
}

impl From<&TypeCheckReport> for TypeCheckProfileStats {
    fn from(report: &TypeCheckReport) -> Self {
        let stats = &report.stats;
        Self {
            flows: stats.flows,
            functions: stats.functions,
            declarations: stats.declarations,
            top_level_items: stats.top_level_items,
            statements: stats.statements,
            expressions: stats.expressions,
            warnings: report.warnings.len(),
            warning_samples: report
                .warnings
                .iter()
                .take(8)
                .map(|warning| warning.message().to_owned())
                .collect(),
            judgments: stats.judgments,
            judgment_rules: TypeCheckJudgmentRuleStats::from_judgments(&report.judgments),
            judgment_samples: report
                .judgments
                .iter()
                .take(8)
                .map(TypeCheckJudgmentSample::from)
                .collect(),
        }
    }
}

#[derive(Clone, Default, serde::Serialize)]
struct TypeCheckJudgmentRuleStats {
    expr: usize,
    expected: usize,
    let_binding: usize,
    #[serde(rename = "return")]
    return_: usize,
}

impl TypeCheckJudgmentRuleStats {
    fn from_judgments(judgments: &[TypeJudgment]) -> Self {
        let mut stats = Self::default();
        for judgment in judgments {
            match judgment.rule {
                TypeJudgmentRule::Expr => stats.expr += 1,
                TypeJudgmentRule::Expected => stats.expected += 1,
                TypeJudgmentRule::LetBinding => stats.let_binding += 1,
                TypeJudgmentRule::Return => stats.return_ += 1,
            }
        }
        stats
    }
}

#[derive(Clone, serde::Serialize)]
struct TypeCheckJudgmentSample {
    id: usize,
    subject: String,
    rule: &'static str,
    ty: String,
    expected: Option<String>,
}

impl From<&TypeJudgment> for TypeCheckJudgmentSample {
    fn from(judgment: &TypeJudgment) -> Self {
        Self {
            id: judgment.id.index(),
            subject: type_judgment_subject_label(&judgment.subject),
            rule: type_judgment_rule_label(judgment.rule),
            ty: format!("{:?}", judgment.ty),
            expected: judgment
                .expected
                .as_ref()
                .map(|expected| format!("{expected:?}")),
        }
    }
}

fn type_judgment_subject_label(subject: &TypeJudgmentSubject) -> String {
    match subject {
        TypeJudgmentSubject::Expr { kind } => format!("expr:{kind}"),
        TypeJudgmentSubject::LetBinding { pattern } => format!("let:{pattern}"),
        TypeJudgmentSubject::Return { context } => format!("return:{context}"),
        TypeJudgmentSubject::Expected { context } => format!("expected:{context}"),
    }
}

const fn type_judgment_rule_label(rule: TypeJudgmentRule) -> &'static str {
    match rule {
        TypeJudgmentRule::Expr => "expr",
        TypeJudgmentRule::Expected => "expected",
        TypeJudgmentRule::LetBinding => "let_binding",
        TypeJudgmentRule::Return => "return",
    }
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct BorrowCheckProfileStats {
    binding_groups: usize,
    bindings: usize,
    state_snapshots: usize,
    state_restores: usize,
    state_merges: usize,
    state_cloned_bindings: usize,
    boundary_checks: usize,
    escape_checks: usize,
    max_active_borrows: usize,
}

impl From<&TypeCheckStats> for BorrowCheckProfileStats {
    fn from(stats: &TypeCheckStats) -> Self {
        Self {
            binding_groups: stats.borrow_binding_groups,
            bindings: stats.borrow_bindings,
            state_snapshots: stats.borrow_state_snapshots,
            state_restores: stats.borrow_state_restores,
            state_merges: stats.borrow_state_merges,
            state_cloned_bindings: stats.borrow_state_cloned_bindings,
            boundary_checks: stats.borrow_boundary_checks,
            escape_checks: stats.borrow_escape_checks,
            max_active_borrows: stats.max_active_borrows,
        }
    }
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeTypeValidationProfileStats {
    flows: usize,
    ops: usize,
    expressions: usize,
    conditions: usize,
    guards: usize,
    let_bindings: usize,
    returns: usize,
    route_targets: usize,
    choice_targets: usize,
    type_judgments: usize,
}

impl From<&RuntimeTypeValidationStats> for RuntimeTypeValidationProfileStats {
    fn from(stats: &RuntimeTypeValidationStats) -> Self {
        Self {
            flows: stats.flows,
            ops: stats.ops,
            expressions: stats.expressions,
            conditions: stats.conditions,
            guards: stats.guards,
            let_bindings: stats.let_bindings,
            returns: stats.returns,
            route_targets: stats.route_targets,
            choice_targets: stats.choice_targets,
            type_judgments: stats.type_judgments,
        }
    }
}

#[derive(serde::Serialize)]
pub(crate) struct BytecodeProfileStats {
    flows: usize,
    instructions: usize,
    line_task_groups: usize,
    stream_plans: usize,
    source_plans: usize,
}

impl From<&BytecodeStats> for BytecodeProfileStats {
    fn from(stats: &BytecodeStats) -> Self {
        Self {
            flows: stats.flows,
            instructions: stats.instructions,
            line_task_groups: stats.line_task_groups,
            stream_plans: stats.stream_plans,
            source_plans: stats.source_plans,
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
    pub(crate) executor: RuntimeExecutorTier,
    pub(crate) executor_stats: RuntimeExecutorStats,
    pub(crate) native_io: NativeTaskStats,
    pub(crate) steps: Vec<RuntimeStepRunSummary>,
    pub(crate) final_status: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuntimeExecutorTier {
    BytecodeVm,
    Aot,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub(crate) struct RuntimeExecutorStats {
    pub(crate) aot_fast_path_ops: usize,
    pub(crate) pure_config: RuntimeExecutorPureConfigSummary,
    pub(crate) pure_acceleration: RuntimeExecutorPureAccelerationSummary,
    pub(crate) pure_compile: RuntimeExecutorPureCompileStatsSummary,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub(crate) struct RuntimeExecutorPureConfigSummary {
    pub(crate) backend: &'static str,
    pub(crate) workers: RuntimeExecutorPureWorkerSummary,
    pub(crate) resolved_workers: usize,
    pub(crate) batch_min_len: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuntimeExecutorPureWorkerSummary {
    #[default]
    Auto,
    Fixed(usize),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub(crate) struct RuntimeExecutorPureAccelerationSummary {
    pub(crate) annotated: usize,
    pub(crate) inferred: usize,
    pub(crate) jit: usize,
    pub(crate) aot: usize,
    pub(crate) vm: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub(crate) struct RuntimeExecutorPureCompileStatsSummary {
    pub(crate) jit_attempts: usize,
    pub(crate) jit_successes: usize,
    pub(crate) jit_failures: usize,
    pub(crate) aot_attempts: usize,
    pub(crate) aot_successes: usize,
    pub(crate) aot_failures: usize,
    pub(crate) cache_hits: usize,
    pub(crate) cache_misses: usize,
    pub(crate) compile_elapsed_ns: u128,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptTestRunReport {
    pub(crate) tests: Vec<ScriptTestRunSummary>,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptTestRunSummary {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) status: String,
    pub(crate) steps_run: usize,
    pub(crate) final_status: Option<String>,
    pub(crate) diagnostics: Vec<String>,
    pub(crate) steps: Vec<RuntimeStepRunSummary>,
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
    pub(crate) line_effects_median: usize,
    pub(crate) task_requests_median: usize,
    pub(crate) task_events_in_median: usize,
    pub(crate) pure_calls_median: usize,
    pub(crate) pure_batch_items_median: usize,
    pub(crate) pure_thread_pool_jobs_median: usize,
    pub(crate) pure_arg_vec_allocations_median: usize,
    pub(crate) diagnostics: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptBenchPureHelperMeasurementSummary {
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
    #[serde(rename = "evaluated_method_calls")]
    pub(crate) method_calls: usize,
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

    pub(crate) fn measured_pure_helper(
        name: impl Into<String>,
        diagnostics: Vec<String>,
        pure_helper: ScriptBenchPureHelperMeasurementSummary,
    ) -> Self {
        Self {
            name: name.into(),
            status: "measured".to_owned(),
            diagnostics,
            measurement: None,
            pure_helper: Some(pure_helper),
        }
    }
}

impl ScriptTestRunSummary {
    pub(crate) fn skipped(test: &ScriptTest, reason: impl Into<String>) -> Self {
        Self {
            id: test.id.clone(),
            kind: test.kind.clone(),
            status: "skipped".to_owned(),
            steps_run: 0,
            final_status: None,
            diagnostics: vec![reason.into()],
            steps: Vec::new(),
        }
    }

    pub(crate) fn completed(
        test: &ScriptTest,
        passed: bool,
        final_status: String,
        diagnostics: Vec<String>,
        steps: Vec<RuntimeStepRunSummary>,
    ) -> Self {
        Self {
            id: test.id.clone(),
            kind: test.kind.clone(),
            status: if passed { "passed" } else { "failed" }.to_owned(),
            steps_run: steps.len(),
            final_status: Some(final_status),
            diagnostics,
            steps,
        }
    }
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeStepRunSummary {
    pub(crate) index: usize,
    pub(crate) stop_reason: String,
    pub(crate) fiber_status: String,
    pub(crate) stats: RuntimeStepStatsSummary,
    pub(crate) diagnostics: Vec<String>,
    pub(crate) flow_events: Vec<String>,
    pub(crate) line_effects: Vec<String>,
    pub(crate) task_requests: Vec<String>,
    pub(crate) observations: RuntimeObservationSummary,
    pub(crate) source_events: Vec<String>,
    pub(crate) stream_events: Vec<String>,
    pub(crate) source_close_requests: Vec<String>,
    pub(crate) source_states: Vec<RuntimeQueueStateSummary>,
    pub(crate) stream_states: Vec<RuntimeQueueStateSummary>,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeStepStatsSummary {
    pub(crate) executed_ops: usize,
    pub(crate) pending_ops_before: usize,
    pub(crate) pending_ops_after: usize,
    pub(crate) child_fibers: usize,
    pub(crate) pure: RuntimePureCallStatsSummary,
    pub(crate) task_events_in: usize,
    pub(crate) source_events_in: usize,
    pub(crate) source_events_emitted: usize,
    pub(crate) stream_events_emitted: usize,
    pub(crate) line_effects: usize,
    pub(crate) diagnostics: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimePureCallStatsSummary {
    pub(crate) pure_calls: usize,
    pub(crate) batch_calls: usize,
    pub(crate) batch_items: usize,
    pub(crate) jit_calls: usize,
    pub(crate) aot_calls: usize,
    pub(crate) vm_calls: usize,
    pub(crate) arg_stack_packs: usize,
    pub(crate) arg_vec_allocations: usize,
    pub(crate) arg_bytes_copied: usize,
    pub(crate) result_bytes_copied: usize,
    pub(crate) thread_pool_jobs: usize,
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
    pub(crate) fn from_result(index: usize, result: RuntimeStepResult, fiber: &FlowFiber) -> Self {
        let RuntimeStepResult {
            output,
            fiber_status,
            stop_reason,
            stats,
        } = result;
        Self {
            index,
            stop_reason: format!("{stop_reason:?}"),
            fiber_status: flow_status_label(&fiber_status),
            stats: RuntimeStepStatsSummary::from(stats),
            diagnostics: output
                .diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.message)
                .collect(),
            flow_events: output.flow_events.iter().map(flow_event_label).collect(),
            line_effects: output.effects.line.iter().map(effect_label).collect(),
            task_requests: output
                .requests
                .tasks
                .iter()
                .map(task_request_label)
                .collect(),
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
            source_events: output
                .effects
                .source_events
                .iter()
                .map(source_event_label)
                .collect(),
            stream_events: output
                .effects
                .stream_events
                .iter()
                .map(stream_event_label)
                .collect(),
            source_close_requests: output
                .requests
                .source_close
                .iter()
                .map(|source| source.0.clone())
                .collect(),
            source_states: fiber
                .source_states
                .values()
                .map(|state| RuntimeQueueStateSummary {
                    id: state.id.0.clone(),
                    queue_depth: state.queue.len(),
                    closed: state.closed,
                    overflow_count: state.overflow_count,
                })
                .collect(),
            stream_states: fiber
                .stream_states
                .values()
                .map(|state| RuntimeQueueStateSummary {
                    id: state.id.0.clone(),
                    queue_depth: state.queue.len(),
                    closed: state.closed,
                    overflow_count: 0,
                })
                .collect(),
        }
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
            source_events_in: stats.source_events_in,
            source_events_emitted: stats.source_events_emitted,
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
            batch_calls: stats.batch_calls,
            batch_items: stats.batch_items,
            jit_calls: stats.jit_calls,
            aot_calls: stats.aot_calls,
            vm_calls: stats.vm_calls,
            arg_stack_packs: stats.arg_stack_packs,
            arg_vec_allocations: stats.arg_vec_allocations,
            arg_bytes_copied: stats.arg_bytes_copied,
            result_bytes_copied: stats.result_bytes_copied,
            thread_pool_jobs: stats.thread_pool_jobs,
            fallbacks: stats.fallbacks,
        }
    }
}

fn flow_event_label(event: &FlowEvent) -> String {
    match event {
        FlowEvent::DialogueLine { line } => format!("dialogue {}", line.0),
        FlowEvent::LineCancelled { trigger } => format!("line_cancelled {trigger}"),
        FlowEvent::ChoicePresented { id } => {
            format!("choice_presented {}", id.as_deref().unwrap_or("-"))
        }
        FlowEvent::ChoiceSelected { id, option } => {
            format!("choice_selected {} {option}", id.as_deref().unwrap_or("-"))
        }
        FlowEvent::AwaitStarted { need, task } => format!("await_started {} {}", need.0, task.0),
        FlowEvent::AwaitReady { need, value } => format!("await_ready {} {value}", need.0),
        FlowEvent::AwaitProgress { need, progress } => {
            format!("await_progress {} {progress}", need.0)
        }
        FlowEvent::Goto { target } => format!("goto {}", target.0),
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

fn source_event_label(event: &RuntimeSourceEvent) -> String {
    format!("{} {}", event.source.0, event_kind_label(&event.kind))
}

fn stream_event_label(event: &RuntimeStreamEvent) -> String {
    format!("{} {}", event.stream.0, event_kind_label(&event.kind))
}

fn event_kind_label(kind: &SourceEventKind<RuntimePayload, RuntimePayload>) -> String {
    match kind {
        SourceEventKind::Item(item) => format!("item {}", item.label()),
        SourceEventKind::Progress(progress) => format!("progress {progress}"),
        SourceEventKind::Disconnected => "disconnected".to_owned(),
        SourceEventKind::PermissionRevoked => "permission_revoked".to_owned(),
        SourceEventKind::Error(error) => format!("error {}", error.label()),
        SourceEventKind::End => "end".to_owned(),
    }
}

pub(crate) fn flow_status_label(status: &FlowFiberStatus) -> String {
    match status {
        FlowFiberStatus::Running => "running".to_owned(),
        FlowFiberStatus::Waiting(state) => format!("waiting {}", state.target.task.0),
        FlowFiberStatus::WaitingMany(state) => format!(
            "waiting_many {} {}/{}",
            state.target.task.0,
            state.results.iter().filter(|value| value.is_some()).count(),
            state.results.len()
        ),
        FlowFiberStatus::Choice(state) => {
            format!("choice {}", state.id.as_deref().unwrap_or("-"))
        }
        FlowFiberStatus::Done(exit) => format!("done {exit:?}"),
        FlowFiberStatus::Failed(message) => format!("failed {message}"),
    }
}

fn count_child_tasks(group: &LineTaskGroup) -> usize {
    count_child_tasks_in_node(&group.root.node)
}

fn count_child_tasks_in_node(node: &LineTaskNode) -> usize {
    match node {
        LineTaskNode::Seq(children)
        | LineTaskNode::Start(children)
        | LineTaskNode::Parallel { children, .. } => {
            children.iter().map(count_child_tasks_in_node).sum()
        }
        LineTaskNode::Child(task) => 1 + count_child_tasks_in_node(&task.scope.node),
        LineTaskNode::Effect(_) => 0,
    }
}

fn count_effects(group: &LineTaskGroup) -> usize {
    count_effects_in_scope(&group.root)
}

fn count_effects_in_scope(scope: &LineTaskScope) -> usize {
    count_effects_in_node(&scope.node)
        + scope.defer_stack.iter().map(Vec::len).sum::<usize>()
        + scope
            .completed_defer_stack
            .iter()
            .map(Vec::len)
            .sum::<usize>()
        + scope
            .cancelled_defer_stack
            .iter()
            .map(Vec::len)
            .sum::<usize>()
        + scope.failed_defer_stack.iter().map(Vec::len).sum::<usize>()
}

fn count_effects_in_node(node: &LineTaskNode) -> usize {
    match node {
        LineTaskNode::Seq(children) | LineTaskNode::Start(children) => {
            children.iter().map(count_effects_in_node).sum()
        }
        LineTaskNode::Parallel { children, .. } => children.iter().map(count_effects_in_node).sum(),
        LineTaskNode::Child(task) => count_effects_in_scope(&task.scope),
        LineTaskNode::Effect(_) => 1,
    }
}
