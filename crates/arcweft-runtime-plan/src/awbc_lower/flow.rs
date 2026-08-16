use crate::awbc_lower::AwbcAudioLowerer;
use crate::awbc_lower::AwbcTraitMethodLowerer;
use crate::awbc_lower::expr::AwbcExprLowerer;
use crate::awbc_lower::frame::FrameBuilder;
use crate::awbc_lower::inventory::{AwbcInventory, AwbcLowerDiagnostic, line_cleanup};
use crate::awbc_lower::line::AwbcLineLowerer;
use crate::awbc_lower::pattern::lower_pattern;
use crate::awbc_lower::{table_index, table_range_len};
use arcweft_core::awbc::schema::{
    AwbcBindMode, AwbcBlock, AwbcBlockId, AwbcChoiceId, AwbcChoiceOption, AwbcDialogueValueBinding,
    AwbcDialogueValueRole, AwbcEffectPlanId, AwbcEffectSetId, AwbcFrameLayoutId, AwbcFunction,
    AwbcFunctionFlags, AwbcFunctionId, AwbcFunctionKind, AwbcInstruction, AwbcIntrinsic,
    AwbcIntrinsicId, AwbcLineCancelHandler, AwbcLineTaskGroup, AwbcLineTaskGroupId,
    AwbcLineTaskNode, AwbcLineTaskNodeId, AwbcLineTaskTrigger, AwbcParallelPolicy, AwbcPatternId,
    AwbcPureHelper, AwbcPureHelperOrigin, AwbcRegisterId, AwbcResumePoint, AwbcResumePointId,
    AwbcSafePointKind, AwbcScopeId, AwbcTableRange, AwbcTerminator, AwbcTrapCode,
};
use arcweft_core::effect::LineEffectRequest;
use arcweft_core::line_task::{
    ChildCancelPolicy, ChildJoinPolicy, LineTaskGroup, LineTaskNode, LineTaskTrigger,
    ParallelPolicy,
};
use arcweft_core::pattern::RuntimePattern;
use arcweft_core::plan::{
    ChoiceRuntimeOption, EntryRuntimeId, FlowOp, FlowRuntimeId, RuntimeDialogueValueRole,
    RuntimeEntrySpec, RuntimeEntryTarget, RuntimeFlow, RuntimeIteratorEvidence,
    RuntimeIteratorWitnessExecutable, RuntimeMatchArm, RuntimePlan, RuntimePureHelper,
    RuntimePureHelperOrigin, RuntimeTraitMethodId,
};
use arcweft_core::value::{RuntimeExpr, RuntimeValue};
use std::collections::{BTreeMap, BTreeSet};

/// Builds one contiguous flow body while allowing host-visible suspension
/// terminators to split the instruction stream into verified resume blocks.
struct FlowBodyBuilder {
    owner: AwbcFunctionId,
    block_start: u32,
    instruction_start: u32,
    resume_points: Vec<AwbcResumePointId>,
    terminated: bool,
    returns_value: bool,
    has_dynamic_target: bool,
}

#[derive(Clone, Copy)]
struct ForLoweringInput<'a> {
    pattern: &'a RuntimePattern,
    source: &'a RuntimeExpr,
    evidence: &'a RuntimeIteratorEvidence,
    ops: &'a [FlowOp],
    path: &'a str,
}

struct BranchJoin {
    fallthroughs: Vec<AwbcBlockId>,
}

struct GuardedCandidate {
    guard_false_jump: AwbcBlockId,
    fallthrough: Option<AwbcBlockId>,
}

impl BranchJoin {
    const fn new() -> Self {
        Self {
            fallthroughs: Vec::new(),
        }
    }

    fn push(&mut self, block: AwbcBlockId) {
        self.fallthroughs.push(block);
    }
}

impl FlowBodyBuilder {
    fn new(inventory: &AwbcInventory, owner: AwbcFunctionId) -> Self {
        Self {
            owner,
            block_start: table_index(inventory.program.blocks.len()),
            instruction_start: table_index(inventory.program.instructions.len()),
            resume_points: Vec::new(),
            terminated: false,
            returns_value: false,
            has_dynamic_target: false,
        }
    }

    fn suspend(
        &mut self,
        inventory: &mut AwbcInventory,
        kind: AwbcSafePointKind,
        terminator: impl FnOnce(AwbcResumePointId) -> AwbcTerminator,
    ) {
        if self.terminated {
            return;
        }
        let current_block = AwbcBlockId(table_index(inventory.program.blocks.len()));
        let next_block = AwbcBlockId(current_block.0.saturating_add(1));
        let resume = inventory.push_resume_point(AwbcResumePoint {
            function: self.owner,
            block: next_block,
            frame_layout: AwbcFrameLayoutId::default(),
            kind,
        });
        let instruction_len =
            table_range_len(self.instruction_start, inventory.program.instructions.len());
        let safe_point = self.current_block_safe_point(inventory, kind);
        inventory.push_block(AwbcBlock {
            owner: self.owner,
            instructions: AwbcTableRange::new(self.instruction_start, instruction_len),
            terminator: terminator(resume),
            safe_point,
            source_map: None,
        });
        self.instruction_start = table_index(inventory.program.instructions.len());
        self.resume_points.push(resume);
    }

    fn close_block(
        &mut self,
        inventory: &mut AwbcInventory,
        terminator: AwbcTerminator,
        safe_point: AwbcSafePointKind,
    ) -> AwbcBlockId {
        let block = AwbcBlockId(table_index(inventory.program.blocks.len()));
        let instruction_len =
            table_range_len(self.instruction_start, inventory.program.instructions.len());
        let safe_point = self.current_block_safe_point(inventory, safe_point);
        inventory.push_block(AwbcBlock {
            owner: self.owner,
            instructions: AwbcTableRange::new(self.instruction_start, instruction_len),
            terminator,
            safe_point,
            source_map: None,
        });
        self.instruction_start = table_index(inventory.program.instructions.len());
        block
    }

    fn reopen_after_terminated_branch(&mut self, inventory: &AwbcInventory) -> AwbcBlockId {
        let block = AwbcBlockId(table_index(inventory.program.blocks.len()));
        self.instruction_start = table_index(inventory.program.instructions.len());
        self.terminated = false;
        block
    }

    fn terminate(
        &mut self,
        inventory: &mut AwbcInventory,
        terminator: AwbcTerminator,
        safe_point: AwbcSafePointKind,
    ) {
        if self.terminated {
            return;
        }
        self.returns_value |= matches!(terminator, AwbcTerminator::Return { value: Some(_) });
        self.has_dynamic_target |= matches!(terminator, AwbcTerminator::GotoDynamic { .. });
        let instruction_len =
            table_range_len(self.instruction_start, inventory.program.instructions.len());
        let safe_point = self.current_block_safe_point(inventory, safe_point);
        inventory.push_block(AwbcBlock {
            owner: self.owner,
            instructions: AwbcTableRange::new(self.instruction_start, instruction_len),
            terminator,
            safe_point,
            source_map: None,
        });
        self.terminated = true;
    }

    fn current_block_safe_point(
        &self,
        inventory: &AwbcInventory,
        safe_point: AwbcSafePointKind,
    ) -> AwbcSafePointKind {
        if table_index(inventory.program.blocks.len()) == self.block_start {
            AwbcSafePointKind::FlowEntry
        } else {
            safe_point
        }
    }

    fn finish(mut self, inventory: &mut AwbcInventory) -> FlowBody {
        if !self.terminated {
            self.terminate(
                inventory,
                AwbcTerminator::Return { value: None },
                AwbcSafePointKind::Return,
            );
        }
        let block_len = table_range_len(self.block_start, inventory.program.blocks.len());
        FlowBody {
            entry_block: AwbcBlockId(self.block_start),
            blocks: AwbcTableRange::new(self.block_start, block_len),
            resume_points: self.resume_points,
            returns_value: self.returns_value,
            has_dynamic_target: self.has_dynamic_target,
        }
    }

    const fn needs_value_fallthrough(&self) -> bool {
        self.returns_value && !self.terminated
    }
}

struct FlowBody {
    entry_block: AwbcBlockId,
    blocks: AwbcTableRange,
    resume_points: Vec<AwbcResumePointId>,
    returns_value: bool,
    has_dynamic_target: bool,
}

/// Lowers all flow entry functions and public entries.
pub struct AwbcFlowLowerer<'inventory, 'plan> {
    inventory: &'inventory mut AwbcInventory,
    plan: &'plan RuntimePlan,
    diagnostics: Vec<AwbcLowerDiagnostic>,
}

impl<'inventory, 'plan> AwbcFlowLowerer<'inventory, 'plan> {
    pub fn new(inventory: &'inventory mut AwbcInventory, plan: &'plan RuntimePlan) -> Self {
        Self {
            inventory,
            plan,
            diagnostics: Vec::new(),
        }
    }

    pub fn lower_plan(&mut self) {
        let flows = self
            .plan
            .flows()
            .iter()
            .map(|flow| flow.id.clone())
            .collect::<BTreeSet<_>>();
        let entries = self.plan.entries().iter().collect::<Vec<_>>();
        self.lower_plan_selection(&flows, &entries);
    }

    /// Lowers one selected entry directly from its complete owning plan.
    pub fn lower_entry_plan(&mut self, selected: &EntryRuntimeId) {
        let entries = self
            .plan
            .entries()
            .iter()
            .filter(|entry| &entry.id == selected)
            .collect::<Vec<_>>();
        let [entry] = entries.as_slice() else {
            self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                selected.canonical_label(),
                if entries.is_empty() {
                    "selected entry is missing from the accepted RuntimePlan"
                } else {
                    "selected entry is duplicated in the accepted RuntimePlan"
                },
            ));
            return;
        };
        let flows = selected_flow_closure(self.plan, entry);
        self.lower_plan_selection(&flows, &entries);
    }

    fn lower_plan_selection(
        &mut self,
        selected_flows: &BTreeSet<FlowRuntimeId>,
        entries: &[&RuntimeEntrySpec],
    ) {
        for helper in self.plan.pure_helpers() {
            self.lower_pure_helper(helper);
        }
        AwbcTraitMethodLowerer::new(self.inventory, self.plan).lower_plan();
        for group in self.plan.line_task_groups() {
            self.lower_line_task_group(group);
        }

        for flow in self
            .plan
            .flows()
            .iter()
            .filter(|flow| selected_flows.contains(&flow.id))
        {
            self.inventory.reserve_flow_function_slot(&flow.id);
        }
        for flow in self
            .plan
            .flows()
            .iter()
            .filter(|flow| selected_flows.contains(&flow.id))
        {
            self.lower_flow(flow);
        }
        self.inventory.lower_selected_entries(self.plan, entries);
    }

    fn lower_pure_helper(&mut self, helper: &RuntimePureHelper) {
        let expected_index = self.inventory.program.pure_helpers.len();
        if helper.id.0 != expected_index {
            self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                format!("pure.{}", helper.name),
                format!(
                    "pure helper `{}` has id {}, expected contiguous id {}",
                    helper.name, helper.id.0, expected_index
                ),
            ));
            return;
        }

        let owner = self.inventory.reserve_function_slot();
        let mut frame = FrameBuilder::new();
        let dynamic_ty = self.inventory.dynamic_ty();
        let mut parameter_types = Vec::with_capacity(helper.input_locals.len());
        for input in &helper.input_locals {
            let ty = self
                .plan
                .local_declarations()
                .get(*input)
                .map_or(dynamic_ty, |declaration| {
                    crate::awbc_lower::pattern::plan_type(
                        self.inventory,
                        self.plan,
                        declaration.ty(),
                    )
                });
            frame.parameter(*input, ty);
            parameter_types.push(ty);
        }
        let instruction_start = table_index(self.inventory.program.instructions.len());
        let value = AwbcExprLowerer::new(
            self.inventory,
            &mut frame,
            format!("pure.{}", helper.name),
            self.plan,
        )
        .lower(&helper.expr);
        let instruction_len =
            table_range_len(instruction_start, self.inventory.program.instructions.len());
        let layout = self
            .inventory
            .intern_frame_layout(format!("pure.{}:frame", helper.name), frame.finish());
        let block = self.inventory.push_block(AwbcBlock {
            owner,
            instructions: AwbcTableRange::new(instruction_start, instruction_len),
            terminator: AwbcTerminator::Return { value: Some(value) },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        });
        let public_id = self.inventory.intern_string(&helper.name);
        let result_type =
            crate::awbc_lower::pattern::plan_type(self.inventory, self.plan, helper.expr.ty());
        let signature = self.inventory.intern_signature(
            parameter_types,
            Some(result_type),
            arcweft_core::awbc::schema::AwbcEffectSetId(0),
        );
        let function = self.inventory.replace_function(
            owner,
            AwbcFunction {
                public_id: Some(public_id),
                kind: AwbcFunctionKind::PureHelper,
                signature,
                frame_layout: layout,
                blocks: AwbcTableRange::new(block.0, 1),
                entry_block: block,
                flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
            },
        );
        self.inventory.program.pure_helpers.push(AwbcPureHelper {
            public_id,
            signature,
            function,
            scalar_eval_supported: helper.scalar_eval_supported,
            origin: match helper.origin {
                RuntimePureHelperOrigin::Annotated => AwbcPureHelperOrigin::Annotated,
                RuntimePureHelperOrigin::Inferred => AwbcPureHelperOrigin::Inferred,
            },
        });
    }

    fn local_type(
        &mut self,
        local: arcweft_core::runtime_id::RuntimeLocalDeclarationId,
    ) -> arcweft_core::awbc::schema::AwbcTypeId {
        let Some(declaration) = self.plan.local_declarations().get(local) else {
            self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                format!("local.{local}"),
                "RuntimePlan local declaration is absent during AWBC lowering",
            ));
            return self.inventory.dynamic_ty();
        };
        crate::awbc_lower::pattern::plan_type(self.inventory, self.plan, declaration.ty())
    }

    pub fn into_diagnostics(mut self) -> Vec<AwbcLowerDiagnostic> {
        self.diagnostics.extend(self.inventory.take_diagnostics());
        self.diagnostics
    }

    fn lower_line_task_group(&mut self, group: &LineTaskGroup) -> AwbcLineTaskGroupId {
        let node_start = table_index(self.inventory.program.line_task_nodes.len());
        let node_id = |id: arcweft_core::runtime_id::RuntimeLineTaskNodeId| {
            AwbcLineTaskNodeId(
                node_start.saturating_add(u32::try_from(id.index()).unwrap_or(u32::MAX)),
            )
        };
        let captures = group.captures().to_vec();
        self.lower_line_task_nodes(group, node_start, &captures);
        let cancel_handlers = group
            .cancel_rules()
            .iter()
            .enumerate()
            .map(|(index, rule)| AwbcLineCancelHandler {
                trigger: rule.trigger(),
                function: self.lower_line_task_action(
                    &captures,
                    rule.action(),
                    &format!("line_task.cancel.{index}"),
                ),
            })
            .collect();
        let id = AwbcLineTaskGroupId(table_index(self.inventory.program.line_task_groups.len()));
        let completed = group
            .cleanup()
            .actions(arcweft_core::line_task::ScopeExit::Completed);
        let cleanup_completed = (!completed.is_empty()).then(|| {
            self.lower_line_task_action(&captures, completed, "line_task.cleanup.completed")
        });
        let cancelled = group
            .cleanup()
            .actions(arcweft_core::line_task::ScopeExit::Cancelled);
        let cleanup_cancelled = (!cancelled.is_empty()).then(|| {
            self.lower_line_task_action(&captures, cancelled, "line_task.cleanup.cancelled")
        });
        let failed = group
            .cleanup()
            .actions(arcweft_core::line_task::ScopeExit::Failed);
        let cleanup_failed = (!failed.is_empty())
            .then(|| self.lower_line_task_action(&captures, failed, "line_task.cleanup.failed"));
        self.inventory
            .program
            .line_task_groups
            .push(AwbcLineTaskGroup {
                captures,
                root: node_id(group.root()),
                nodes: AwbcTableRange::new(
                    node_start,
                    table_range_len(node_start, self.inventory.program.line_task_nodes.len()),
                ),
                cancel_handlers,
                cleanup_completed,
                cleanup_cancelled,
                cleanup_failed,
                cleanup: line_cleanup(group.cleanup().policy()),
            });
        id
    }

    fn lower_line_task_nodes(
        &mut self,
        group: &LineTaskGroup,
        node_start: u32,
        captures: &[arcweft_core::runtime_id::RuntimeLocalDeclarationId],
    ) {
        let node_id = |id: arcweft_core::runtime_id::RuntimeLineTaskNodeId| {
            AwbcLineTaskNodeId(
                node_start.saturating_add(u32::try_from(id.index()).unwrap_or(u32::MAX)),
            )
        };
        for (index, node) in group.nodes().iter().enumerate() {
            let path = format!("line_task.{index}");
            let lowered = match node {
                LineTaskNode::Sequence(nodes) => {
                    AwbcLineTaskNode::Sequence(nodes.iter().copied().map(node_id).collect())
                }
                LineTaskNode::Start(nodes) => {
                    AwbcLineTaskNode::Start(nodes.iter().copied().map(node_id).collect())
                }
                LineTaskNode::Parallel { policy, children } => AwbcLineTaskNode::Parallel {
                    policy: match policy {
                        ParallelPolicy::JoinAll => AwbcParallelPolicy::JoinAll,
                    },
                    children: children.iter().copied().map(node_id).collect(),
                },
                LineTaskNode::Child {
                    id,
                    key,
                    name,
                    trigger,
                    priority,
                    join_policy,
                    cancel_policy,
                    scope,
                } => AwbcLineTaskNode::Child {
                    id: self.inventory.intern_string(&id.0),
                    key: key.as_ref().map(|key| self.inventory.intern_string(&key.0)),
                    name: name.as_ref().map(|name| self.inventory.intern_string(name)),
                    trigger: match trigger {
                        LineTaskTrigger::Immediate => AwbcLineTaskTrigger::Immediate,
                        LineTaskTrigger::Mark(mark) => AwbcLineTaskTrigger::Mark(*mark),
                        LineTaskTrigger::Delay(duration) => {
                            AwbcLineTaskTrigger::DelayNanos(duration.as_nanos())
                        }
                    },
                    priority: priority.0,
                    join: match join_policy {
                        ChildJoinPolicy::Join => {
                            arcweft_core::awbc::schema::AwbcChildJoinPolicy::Join
                        }
                        ChildJoinPolicy::Detached => {
                            arcweft_core::awbc::schema::AwbcChildJoinPolicy::Detached
                        }
                    },
                    cancel: match cancel_policy {
                        ChildCancelPolicy::CancelAndJoin => {
                            arcweft_core::awbc::schema::AwbcChildCancelPolicy::CancelAndJoin
                        }
                        ChildCancelPolicy::Finish => {
                            arcweft_core::awbc::schema::AwbcChildCancelPolicy::Finish
                        }
                        ChildCancelPolicy::Detach => {
                            arcweft_core::awbc::schema::AwbcChildCancelPolicy::Detach
                        }
                    },
                    scope: node_id(*scope),
                },
                LineTaskNode::Action(ops) => {
                    AwbcLineTaskNode::Action(self.lower_line_task_action(captures, ops, &path))
                }
            };
            self.inventory.program.line_task_nodes.push(lowered);
        }
    }

    fn lower_line_task_action(
        &mut self,
        captures: &[arcweft_core::runtime_id::RuntimeLocalDeclarationId],
        ops: &[FlowOp],
        path: &str,
    ) -> AwbcFunctionId {
        let owner = self.inventory.reserve_function_slot();
        let mut frame = FrameBuilder::new();
        for capture in captures {
            let ty = self.local_type(*capture);
            frame.parameter(*capture, ty);
        }
        let mut body = FlowBodyBuilder::new(self.inventory, owner);
        self.lower_ops(&mut frame, &mut body, ops, path);
        if body.needs_value_fallthrough() {
            self.terminate_value_fallthrough(&mut frame, &mut body);
        }
        let body = body.finish(self.inventory);
        let layout = self
            .inventory
            .intern_frame_layout(format!("{path}:frame"), frame.finish());
        for resume in body.resume_points {
            if let Some(point) = self.inventory.program.resume_points.get_mut(resume.index()) {
                point.frame_layout = layout;
            }
        }
        let params = captures
            .iter()
            .map(|capture| self.local_type(*capture))
            .collect();
        let signature = self.inventory.intern_signature(
            params,
            body.returns_value.then(|| self.inventory.dynamic_ty()),
            AwbcEffectSetId(0),
        );
        let public_id = Some(self.inventory.intern_string(path));
        self.inventory.replace_function(
            owner,
            AwbcFunction {
                public_id,
                kind: AwbcFunctionKind::LineTask,
                signature,
                frame_layout: layout,
                blocks: body.blocks,
                entry_block: body.entry_block,
                flags: AwbcFunctionFlags(
                    AwbcFunctionFlags::MAY_SUSPEND | AwbcFunctionFlags::DETERMINISTIC,
                ),
            },
        )
    }

    fn lower_flow(&mut self, flow: &RuntimeFlow) -> AwbcFunctionId {
        let mut frame = FrameBuilder::new();
        let public_name = flow_public_id(&flow.id);
        let canonical_name = flow.id.canonical_label();
        let owner = self
            .inventory
            .flow_function(&flow.id)
            .unwrap_or_else(|| self.inventory.reserve_flow_function_slot(&flow.id));
        let dynamic_ty = self.inventory.dynamic_ty();
        for parameter in &flow.params {
            let ty =
                self.plan
                    .local_declarations()
                    .get(*parameter)
                    .map_or(dynamic_ty, |declaration| {
                        crate::awbc_lower::pattern::plan_type(
                            self.inventory,
                            self.plan,
                            declaration.ty(),
                        )
                    });
            frame.parameter(*parameter, ty);
        }
        let mut body = FlowBodyBuilder::new(self.inventory, owner);
        self.lower_ops(&mut frame, &mut body, &flow.ops, &public_name);
        if body.needs_value_fallthrough() {
            self.terminate_value_fallthrough(&mut frame, &mut body);
        }
        let body = body.finish(self.inventory);
        let layout = self
            .inventory
            .intern_frame_layout(format!("flow:{canonical_name}"), frame.finish());
        for resume in body.resume_points {
            if let Some(point) = self.inventory.program.resume_points.get_mut(resume.index()) {
                point.frame_layout = layout;
            }
        }
        let params = flow
            .params
            .iter()
            .map(|parameter| self.local_type(*parameter))
            .collect();
        let signature = if body.returns_value {
            self.inventory.intern_signature(
                params,
                Some(self.inventory.dynamic_ty()),
                AwbcEffectSetId(0),
            )
        } else {
            self.inventory
                .intern_signature(params, None, AwbcEffectSetId(0))
        };
        let public_id = self.inventory.intern_string(&public_name);
        let mut flags = AwbcFunctionFlags::MAY_SUSPEND | AwbcFunctionFlags::DETERMINISTIC;
        if body.has_dynamic_target {
            flags |= AwbcFunctionFlags::HAS_DYNAMIC_TARGET;
        }
        let function = self.inventory.replace_flow_function(
            &flow.id,
            owner,
            AwbcFunction {
                public_id: Some(public_id),
                kind: AwbcFunctionKind::Flow,
                signature,
                frame_layout: layout,
                blocks: body.blocks,
                entry_block: body.entry_block,
                flags: AwbcFunctionFlags(flags),
            },
        );
        debug_assert_eq!(function, owner);
        function
    }

    fn terminate_value_fallthrough(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
    ) {
        let unit = self.inventory.constant_runtime_value(&RuntimeValue::Unit);
        let fallback = frame.return_value(self.inventory.dynamic_ty());
        self.inventory.push_instruction(AwbcInstruction::LoadConst {
            dst: fallback,
            constant: unit,
        });
        body.terminate(
            self.inventory,
            AwbcTerminator::Return {
                value: Some(fallback),
            },
            AwbcSafePointKind::Return,
        );
    }

    fn lower_ops(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
        ops: &[FlowOp],
        path: &str,
    ) {
        for (index, op) in ops.iter().enumerate() {
            if body.terminated {
                break;
            }
            self.lower_op(frame, body, op, &format!("{path}.{index}"));
        }
    }

    #[allow(clippy::too_many_lines)]
    fn lower_op(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
        op: &FlowOp,
        path: &str,
    ) {
        match op {
            FlowOp::Bind(bindings) => {
                for binding in bindings {
                    let ty = self.local_type(binding.local);
                    let local = frame.local(binding.local, ty);
                    let constant = self.inventory.constant_runtime_value(&binding.value);
                    self.inventory.push_instruction(AwbcInstruction::LoadConst {
                        dst: local,
                        constant,
                    });
                }
            }
            FlowOp::Let { pattern, expr } | FlowOp::ExitScopeBind { pattern, expr } => {
                let value =
                    AwbcExprLowerer::new(self.inventory, frame, path, self.plan).lower(expr);
                let pattern = lower_pattern(self.inventory, self.plan, frame, pattern);
                self.inventory
                    .push_instruction(AwbcInstruction::BindPattern {
                        pattern,
                        value,
                        mode: AwbcBindMode::Declare,
                    });
            }
            FlowOp::LetElse {
                pattern,
                expr,
                else_ops,
            } => {
                self.lower_let_else(frame, body, pattern, expr, else_ops, path);
            }
            FlowOp::AssignNominalField { base, field, value } => {
                let Some(target) = frame.register_for_local(*base) else {
                    self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                        path,
                        format!("field assignment base `{base}` is not in the AWBC frame"),
                    ));
                    return;
                };
                let value =
                    AwbcExprLowerer::new(self.inventory, frame, path, self.plan).lower(value);
                self.inventory
                    .push_instruction(AwbcInstruction::AssignRecordField {
                        target,
                        field: field.zero_based(),
                        value,
                    });
            }
            FlowOp::Dialogue { content } => {
                let Some(content_plan) = self.plan.dialogue_content().get(*content) else {
                    self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                        path,
                        format!("dialogue content plan {content} is absent from the RuntimePlan"),
                    ));
                    return;
                };
                let group = content_plan
                    .line_task_group()
                    .map(|group| AwbcLineTaskGroupId(table_index(group.index())));
                let content =
                    AwbcLineLowerer::new(self.inventory).content_for_line(content_plan, group);
                let group_captures = group
                    .and_then(|group| self.inventory.program.line_task_groups.get(group.index()))
                    .map(|group| group.captures.clone())
                    .unwrap_or_default();
                let line_task_captures = group_captures
                    .iter()
                    .map(|capture| {
                        let ty = self.local_type(*capture);
                        frame.local(*capture, ty)
                    })
                    .collect();
                let mut values_by_function = BTreeMap::new();
                let mut values = Vec::with_capacity(content_plan.values().len());
                for site in content_plan.values() {
                    let function = site.function();
                    let value = if let Some(value) = values_by_function.get(&function) {
                        *value
                    } else if let Some(function_site) = self.plan.function_sites().get(function) {
                        let value = AwbcExprLowerer::new(
                            self.inventory,
                            frame,
                            format!("{path}.dialogue.{function}"),
                            self.plan,
                        )
                        .lower(function_site.body());
                        values_by_function.insert(function, value);
                        value
                    } else {
                        self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                            path,
                            format!("dialogue value function site {function} is absent from the RuntimePlan"),
                        ));
                        continue;
                    };
                    values.push(AwbcDialogueValueBinding {
                        slot: site.slot(),
                        role: awbc_dialogue_value_role(site.role()),
                        value,
                    });
                }
                body.suspend(self.inventory, AwbcSafePointKind::Dialogue, |resume| {
                    AwbcTerminator::Dialogue {
                        content,
                        values,
                        line_task_captures,
                        resume,
                    }
                });
            }
            FlowOp::Choice { id, options } => {
                let choice = self.lower_choice(id.as_deref(), options);
                let dst = frame.temp(self.inventory.string_ty());
                body.suspend(self.inventory, AwbcSafePointKind::Choice, |resume| {
                    AwbcTerminator::Choice {
                        choice,
                        dst,
                        resume,
                    }
                });
            }
            FlowOp::Await {
                binding,
                target,
                pending,
            } => {
                self.lower_pending_effects(pending);
                let task = self.inventory.intern_host_task_with_outcome(
                    &target.need.0,
                    &target.task.0,
                    &target.request,
                    &target.outcome,
                );
                let args = target
                    .request
                    .args
                    .iter()
                    .map(|arg| {
                        AwbcExprLowerer::new(self.inventory, frame, path, self.plan)
                            .lower(arg.value())
                    })
                    .collect::<Vec<_>>();
                let task_handle = frame.temp(self.inventory.dynamic_ty());
                self.inventory.push_instruction(AwbcInstruction::StartTask {
                    dst: task_handle,
                    plan: task,
                    args,
                });
                let binding = binding
                    .as_ref()
                    .map(|binding| lower_pattern(self.inventory, self.plan, frame, binding));
                body.suspend(self.inventory, AwbcSafePointKind::Await, |resume| {
                    AwbcTerminator::Await {
                        handle: task_handle,
                        binding,
                        resume,
                    }
                });
            }
            FlowOp::AwaitMany {
                binding,
                target,
                pending,
            } => {
                self.lower_pending_effects(pending);
                let source = AwbcExprLowerer::new(self.inventory, frame, path, self.plan)
                    .lower(&target.source);
                let task = self.inventory.intern_host_task_with_outcome(
                    &target.need.0,
                    &target.task.0,
                    &target.request,
                    &target.outcome,
                );
                let item_binding =
                    frame.local(target.item_binding, self.local_type(target.item_binding));
                self.inventory
                    .set_await_many_policy(task, item_binding, target.limit);
                let binding = binding
                    .as_ref()
                    .map(|binding| lower_pattern(self.inventory, self.plan, frame, binding));
                body.suspend(self.inventory, AwbcSafePointKind::AwaitMany, |resume| {
                    AwbcTerminator::AwaitMany {
                        plan: task,
                        source,
                        binding,
                        resume,
                    }
                });
            }
            FlowOp::HostCall { binding, target } => {
                let call = self.inventory.intern_host_call(target);
                let args = target
                    .args
                    .iter()
                    .map(|arg| {
                        AwbcExprLowerer::new(self.inventory, frame, path, self.plan)
                            .lower(arg.value())
                    })
                    .collect::<Vec<_>>();
                // Host calls always have a value result in the admitted runtime
                // signature. Keep that result shape even when the source flow
                // deliberately discards the value; only the pattern binding is
                // optional.
                let dst = frame.temp(self.inventory.dynamic_ty());
                let pattern = binding
                    .as_ref()
                    .map(|binding| lower_pattern(self.inventory, self.plan, frame, binding));
                body.suspend(self.inventory, AwbcSafePointKind::HostCall, |resume| {
                    AwbcTerminator::HostCall {
                        call,
                        args,
                        dst: Some(dst),
                        resume,
                    }
                });
                if let Some(pattern) = pattern {
                    self.inventory
                        .push_instruction(AwbcInstruction::BindPattern {
                            pattern,
                            value: dst,
                            mode: AwbcBindMode::Declare,
                        });
                }
            }
            FlowOp::If {
                condition,
                then_ops,
                else_ops,
            } => {
                self.lower_if(frame, body, condition, then_ops, else_ops, path);
            }
            FlowOp::IfLet {
                pattern,
                expr,
                guard,
                then_ops,
                else_ops,
            } => {
                self.lower_if_let(
                    frame,
                    body,
                    pattern,
                    expr,
                    guard.as_ref(),
                    then_ops,
                    else_ops,
                    path,
                );
            }
            FlowOp::Match { scrutinee, arms } => {
                self.lower_match(frame, body, scrutinee, arms, path);
            }
            FlowOp::Loop { body: ops }
            | FlowOp::LetLoop { body: ops, .. }
            | FlowOp::Thread { body: ops, .. } => {
                let scope = frame.enter_scope();
                self.inventory
                    .push_instruction(AwbcInstruction::EnterScope { scope });
                self.lower_ops(frame, body, ops, &format!("{path}.body"));
                if !body.terminated {
                    body.suspend(self.inventory, AwbcSafePointKind::BudgetYield, |resume| {
                        AwbcTerminator::BudgetYield { resume }
                    });
                    self.inventory
                        .push_instruction(AwbcInstruction::ExitScope { scope });
                }
                frame.exit_scope();
            }
            FlowOp::LoopNext { body: ops }
            | FlowOp::WhileNext { body: ops, .. }
            | FlowOp::WhileLetNext { body: ops, .. }
            | FlowOp::ForNext { body: ops, .. } => {
                self.lower_ops(frame, body, ops.as_ref(), &format!("{path}.next"));
            }
            FlowOp::While {
                condition,
                body: ops,
            } => {
                let _ =
                    AwbcExprLowerer::new(self.inventory, frame, path, self.plan).lower(condition);
                self.lower_ops(frame, body, ops, &format!("{path}.body"));
            }
            FlowOp::WhileLet {
                pattern,
                expr,
                guard,
                body: ops,
            } => {
                let value =
                    AwbcExprLowerer::new(self.inventory, frame, path, self.plan).lower(expr);
                let pattern = lower_pattern(self.inventory, self.plan, frame, pattern);
                let matched = frame.temp(self.inventory.bool_ty());
                self.inventory
                    .push_instruction(AwbcInstruction::TestPattern {
                        dst: matched,
                        pattern,
                        value,
                    });
                if let Some(guard) = guard {
                    let _ =
                        AwbcExprLowerer::new(self.inventory, frame, path, self.plan).lower(guard);
                }
                self.lower_ops(frame, body, ops, &format!("{path}.body"));
            }
            FlowOp::For {
                pattern,
                source,
                evidence,
                body: ops,
            } => {
                self.lower_for(
                    frame,
                    body,
                    ForLoweringInput {
                        pattern,
                        source,
                        evidence,
                        ops,
                        path,
                    },
                );
            }
            FlowOp::Scope(ops) => {
                let scope = frame.enter_scope();
                self.inventory
                    .push_instruction(AwbcInstruction::EnterScope { scope });
                self.lower_ops(frame, body, ops, &format!("{path}.scope"));
                if !body.terminated {
                    self.inventory
                        .push_instruction(AwbcInstruction::ExitScope { scope });
                }
                frame.exit_scope();
            }
            FlowOp::LetScope {
                pattern,
                ops,
                value,
            } => self.lower_let_scope(frame, body, pattern, ops, value, path),
            FlowOp::Break(value) => {
                if let Some(value) = value {
                    let value =
                        AwbcExprLowerer::new(self.inventory, frame, path, self.plan).lower(value);
                    let _ = value;
                }
                self.push_intrinsic_call("flow.break", Vec::new());
            }
            FlowOp::ReturnExpr(value) => {
                let value =
                    AwbcExprLowerer::new(self.inventory, frame, path, self.plan).lower(value);
                let result = frame.return_value(self.inventory.dynamic_ty());
                self.inventory.push_instruction(AwbcInstruction::Move {
                    dst: result,
                    src: value,
                });
                self.close_active_scopes_for_terminator(frame);
                body.terminate(
                    self.inventory,
                    AwbcTerminator::Return {
                        value: Some(result),
                    },
                    AwbcSafePointKind::Return,
                );
            }
            FlowOp::Continue => {
                self.push_intrinsic_call("flow.continue", Vec::new());
            }
            FlowOp::Goto(target) => {
                if let Some(function) = self.inventory.flow_function(target) {
                    self.close_active_scopes_for_terminator(frame);
                    body.terminate(
                        self.inventory,
                        AwbcTerminator::GotoStatic {
                            function,
                            args: Vec::new(),
                        },
                        AwbcSafePointKind::CallableBoundary,
                    );
                } else {
                    let target = target.canonical_label();
                    self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                        path,
                        format!("static goto targets missing accepted Flow `{target}`"),
                    ));
                    let message = self.inventory.intern_string(&format!(
                        "static goto targets missing accepted Flow `{target}`"
                    ));
                    self.close_active_scopes_for_terminator(frame);
                    body.terminate(
                        self.inventory,
                        AwbcTerminator::Trap {
                            code: AwbcTrapCode::MissingDynamicTarget,
                            message: Some(message),
                        },
                        AwbcSafePointKind::CallableBoundary,
                    );
                }
            }
            FlowOp::GotoExpr(expr) => {
                let target =
                    AwbcExprLowerer::new(self.inventory, frame, path, self.plan).lower(expr);
                let stable_target = frame.root_temp(self.inventory.dynamic_ty());
                self.inventory.push_instruction(AwbcInstruction::Move {
                    dst: stable_target,
                    src: target,
                });
                self.close_active_scopes_for_terminator(frame);
                body.terminate(
                    self.inventory,
                    AwbcTerminator::GotoDynamic {
                        target: stable_target,
                        args: Vec::new(),
                    },
                    AwbcSafePointKind::CallableBoundary,
                );
            }
            FlowOp::Return(value) => {
                let value = self.inventory.constant_string(value);
                let dst = frame.return_value(self.inventory.string_ty());
                self.inventory.push_instruction(AwbcInstruction::LoadConst {
                    dst,
                    constant: value,
                });
                self.close_active_scopes_for_terminator(frame);
                body.terminate(
                    self.inventory,
                    AwbcTerminator::Return { value: Some(dst) },
                    AwbcSafePointKind::Return,
                );
            }
            FlowOp::Effect(effect) => {
                let (effect, args) = self.lower_effect_plan(frame, path, effect);
                self.inventory
                    .push_instruction(AwbcInstruction::EmitEffect { effect, args });
            }
            FlowOp::EvaluatedEffect(effect) => {
                let args = effect
                    .argument_exprs()
                    .into_iter()
                    .map(|expr| {
                        AwbcExprLowerer::new(self.inventory, frame, path, self.plan).lower(expr)
                    })
                    .collect();
                let effect = self.inventory.intern_evaluated_effect(effect);
                self.inventory
                    .push_instruction(AwbcInstruction::EmitEffect { effect, args });
            }
            FlowOp::RegisterCleanup { key, effect } => {
                let key = self.inventory.intern_string(key);
                let (effect, args) = self.lower_effect_plan(frame, path, effect);
                self.inventory
                    .push_instruction(AwbcInstruction::RegisterCleanup { key, effect, args });
            }
            FlowOp::CancelCleanup { key } => {
                let key = self.inventory.intern_string(key);
                self.inventory
                    .push_instruction(AwbcInstruction::CancelCleanup { key });
            }
            FlowOp::EnterScope => {
                let scope = frame.enter_scope();
                self.inventory
                    .push_instruction(AwbcInstruction::EnterScope { scope });
            }
            FlowOp::ExitScope => {
                self.inventory.push_instruction(AwbcInstruction::ExitScope {
                    scope: AwbcScopeId(0),
                });
                frame.exit_scope();
            }
            FlowOp::Noop => {
                self.inventory.push_instruction(AwbcInstruction::Nop);
            }
        }
    }

    fn lower_effect_plan(
        &mut self,
        frame: &mut FrameBuilder,
        path: &str,
        effect: &LineEffectRequest,
    ) -> (AwbcEffectPlanId, Vec<AwbcRegisterId>) {
        match effect {
            LineEffectRequest::Audio(command) => {
                let (command, args) =
                    AwbcAudioLowerer::new(self.inventory, frame, path, self.plan).lower(command);
                let effect = self.inventory.intern_audio_effect(command, args.len());
                (effect, args)
            }
            _ => (self.inventory.intern_effect(effect), Vec::new()),
        }
    }

    fn lower_pending_effects(&mut self, pending: &[arcweft_core::effect::LineEffectRequest]) {
        for effect in pending {
            let effect = self.inventory.intern_effect(effect);
            self.inventory
                .push_instruction(AwbcInstruction::EmitEffect {
                    effect,
                    args: Vec::new(),
                });
        }
    }

    fn lower_choice(&mut self, id: Option<&str>, options: &[ChoiceRuntimeOption]) -> AwbcChoiceId {
        let public_id = id.map(|id| self.inventory.intern_string(id));
        let lowered = options
            .iter()
            .map(|option| AwbcChoiceOption {
                public_id: option
                    .id
                    .as_ref()
                    .map(|id| self.inventory.intern_string(id)),
                label: self.inventory.intern_string(&option.label),
                condition: None,
                target: option
                    .target
                    .as_ref()
                    .and_then(|target| self.inventory.flow_function(target)),
                out_effect: option.out.as_ref().map(|out| {
                    self.inventory
                        .intern_effect(&arcweft_core::effect::LineEffectRequest::Out(out.clone()))
                }),
                effects: option
                    .effects
                    .iter()
                    .map(|effect| self.inventory.intern_effect(effect))
                    .collect(),
            })
            .collect::<Vec<_>>();
        self.inventory
            .intern_choice(format!("choice:{id:?}:{options:?}"), public_id, lowered)
    }

    fn lower_let_else(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        else_ops: &[FlowOp],
        path: &str,
    ) {
        let value = AwbcExprLowerer::new(self.inventory, frame, path, self.plan).lower(expr);
        let pattern = lower_pattern(self.inventory, self.plan, frame, pattern);
        let matched = frame.temp(self.inventory.bool_ty());
        self.inventory
            .push_instruction(AwbcInstruction::TestPattern {
                dst: matched,
                pattern,
                value,
            });

        let matched_block = AwbcBlockId(table_index(
            self.inventory.program.blocks.len().saturating_add(1),
        ));
        let branch_block = body.close_block(
            self.inventory,
            AwbcTerminator::Branch {
                condition: matched,
                then_block: matched_block,
                else_block: matched_block,
            },
            AwbcSafePointKind::None,
        );

        self.inventory
            .push_instruction(AwbcInstruction::BindPattern {
                pattern,
                value,
                mode: AwbcBindMode::Declare,
            });
        let mut join = BranchJoin::new();
        join.push(self.close_jump_to_join(body));

        let else_block = AwbcBlockId(table_index(self.inventory.program.blocks.len()));
        patch_branch_else_block(self.inventory, branch_block, else_block);
        self.lower_ops(frame, body, else_ops, &format!("{path}.else"));
        if !body.terminated {
            join.push(self.close_jump_to_join(body));
        }
        self.finish_join(body, join);
    }

    fn lower_if(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
        condition: &RuntimeExpr,
        then_ops: &[FlowOp],
        else_ops: &[FlowOp],
        path: &str,
    ) {
        let condition =
            AwbcExprLowerer::new(self.inventory, frame, path, self.plan).lower(condition);
        let then_block = AwbcBlockId(table_index(
            self.inventory.program.blocks.len().saturating_add(1),
        ));
        let branch_block = body.close_block(
            self.inventory,
            AwbcTerminator::Branch {
                condition,
                then_block,
                else_block: then_block,
            },
            AwbcSafePointKind::None,
        );

        let mut join = BranchJoin::new();
        self.lower_scoped_branch_ops(frame, body, None, then_ops, &format!("{path}.then"));
        if body.terminated {
            let else_block = body.reopen_after_terminated_branch(self.inventory);
            patch_branch_else_block(self.inventory, branch_block, else_block);
        } else {
            join.push(self.close_jump_to_join(body));
            let else_block = AwbcBlockId(table_index(self.inventory.program.blocks.len()));
            patch_branch_else_block(self.inventory, branch_block, else_block);
        }

        self.lower_scoped_branch_ops(frame, body, None, else_ops, &format!("{path}.else"));
        if !body.terminated {
            join.push(self.close_jump_to_join(body));
        }
        self.finish_join(body, join);
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_if_let(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        guard: Option<&RuntimeExpr>,
        then_ops: &[FlowOp],
        else_ops: &[FlowOp],
        path: &str,
    ) {
        let value = AwbcExprLowerer::new(self.inventory, frame, path, self.plan).lower(expr);
        let pattern = self.lower_branch_pattern(frame, pattern);
        let matched = frame.temp(self.inventory.bool_ty());
        self.inventory
            .push_instruction(AwbcInstruction::TestPattern {
                dst: matched,
                pattern,
                value,
            });
        let candidate_block = AwbcBlockId(table_index(
            self.inventory.program.blocks.len().saturating_add(1),
        ));
        let branch_block = body.close_block(
            self.inventory,
            AwbcTerminator::Branch {
                condition: matched,
                then_block: candidate_block,
                else_block: candidate_block,
            },
            AwbcSafePointKind::None,
        );

        let mut join = BranchJoin::new();
        if let Some(guard) = guard {
            let guarded =
                self.lower_guarded_candidate(frame, body, pattern, value, guard, then_ops, path);
            if let Some(fallthrough) = guarded.fallthrough {
                join.push(fallthrough);
            }
            let else_block = AwbcBlockId(table_index(self.inventory.program.blocks.len()));
            patch_branch_else_block(self.inventory, branch_block, else_block);
            patch_jump_target(self.inventory, guarded.guard_false_jump, else_block);
        } else {
            self.lower_scoped_branch_ops(
                frame,
                body,
                Some((pattern, value)),
                then_ops,
                &format!("{path}.then"),
            );
            if body.terminated {
                let else_block = body.reopen_after_terminated_branch(self.inventory);
                patch_branch_else_block(self.inventory, branch_block, else_block);
            } else {
                join.push(self.close_jump_to_join(body));
                let else_block = AwbcBlockId(table_index(self.inventory.program.blocks.len()));
                patch_branch_else_block(self.inventory, branch_block, else_block);
            }
        }

        self.lower_scoped_branch_ops(frame, body, None, else_ops, &format!("{path}.else"));
        if !body.terminated {
            join.push(self.close_jump_to_join(body));
        }
        self.finish_join(body, join);
    }

    fn lower_match(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
        scrutinee: &arcweft_core::value::RuntimeExpr,
        arms: &[RuntimeMatchArm],
        path: &str,
    ) {
        let scrutinee =
            AwbcExprLowerer::new(self.inventory, frame, path, self.plan).lower(scrutinee);
        let mut join = BranchJoin::new();
        for (index, arm) in arms.iter().enumerate() {
            let pattern = self.lower_branch_pattern(frame, &arm.pattern);
            let matched = frame.temp(self.inventory.bool_ty());
            self.inventory
                .push_instruction(AwbcInstruction::TestPattern {
                    dst: matched,
                    pattern,
                    value: scrutinee,
                });
            let candidate_block = AwbcBlockId(table_index(
                self.inventory.program.blocks.len().saturating_add(1),
            ));
            let branch_block = body.close_block(
                self.inventory,
                AwbcTerminator::Branch {
                    condition: matched,
                    then_block: candidate_block,
                    else_block: candidate_block,
                },
                AwbcSafePointKind::None,
            );

            if let Some(guard) = arm.guard.as_ref() {
                let guarded = self.lower_guarded_candidate(
                    frame,
                    body,
                    pattern,
                    scrutinee,
                    guard,
                    &arm.ops,
                    &format!("{path}.arm.{index}"),
                );
                if let Some(fallthrough) = guarded.fallthrough {
                    join.push(fallthrough);
                }
                let next_arm_block = AwbcBlockId(table_index(self.inventory.program.blocks.len()));
                patch_branch_else_block(self.inventory, branch_block, next_arm_block);
                patch_jump_target(self.inventory, guarded.guard_false_jump, next_arm_block);
            } else {
                self.lower_scoped_branch_ops(
                    frame,
                    body,
                    Some((pattern, scrutinee)),
                    &arm.ops,
                    &format!("{path}.arm.{index}"),
                );
                if body.terminated {
                    let next_arm_block = body.reopen_after_terminated_branch(self.inventory);
                    patch_branch_else_block(self.inventory, branch_block, next_arm_block);
                } else {
                    join.push(self.close_jump_to_join(body));
                    let next_arm_block =
                        AwbcBlockId(table_index(self.inventory.program.blocks.len()));
                    patch_branch_else_block(self.inventory, branch_block, next_arm_block);
                }
            }
        }
        self.terminate_pattern_mismatch(body, "match pattern did not match");
        self.finish_join(body, join);
    }

    fn lower_scoped_branch_ops(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
        binding: Option<(AwbcPatternId, AwbcRegisterId)>,
        ops: &[FlowOp],
        path: &str,
    ) {
        let restored_scope_depth = frame.scope_depth();
        let scope = frame.enter_scope();
        self.inventory
            .push_instruction(AwbcInstruction::EnterScope { scope });
        if let Some((pattern, value)) = binding {
            self.inventory
                .push_instruction(AwbcInstruction::BindPattern {
                    pattern,
                    value,
                    mode: AwbcBindMode::Declare,
                });
        }
        self.lower_ops(frame, body, ops, path);
        if !body.terminated {
            self.inventory
                .push_instruction(AwbcInstruction::ExitScope { scope });
            frame.exit_scope();
        }
        frame.restore_scope_depth_after_branch(restored_scope_depth);
    }

    fn lower_let_scope(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
        pattern: &RuntimePattern,
        ops: &[FlowOp],
        value: &RuntimeExpr,
        path: &str,
    ) {
        let scope = frame.enter_scope();
        self.inventory
            .push_instruction(AwbcInstruction::EnterScope { scope });
        self.lower_ops(frame, body, ops, &format!("{path}.let_scope"));
        if body.terminated {
            frame.exit_scope();
            return;
        }

        let scoped_value =
            AwbcExprLowerer::new(self.inventory, frame, path, self.plan).lower(value);
        let value = frame.root_temp(self.inventory.dynamic_ty());
        self.inventory.push_instruction(AwbcInstruction::Move {
            dst: value,
            src: scoped_value,
        });
        self.inventory
            .push_instruction(AwbcInstruction::ExitScope { scope });
        frame.exit_scope();

        let pattern = lower_pattern(self.inventory, self.plan, frame, pattern);
        self.inventory
            .push_instruction(AwbcInstruction::BindPattern {
                pattern,
                value,
                mode: AwbcBindMode::Declare,
            });
    }

    fn lower_branch_pattern(
        &mut self,
        frame: &mut FrameBuilder,
        pattern: &RuntimePattern,
    ) -> AwbcPatternId {
        let restored_scope_depth = frame.scope_depth();
        let _ = frame.enter_scope();
        let pattern = lower_pattern(self.inventory, self.plan, frame, pattern);
        frame.restore_scope_depth_after_branch(restored_scope_depth);
        pattern
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Guarded pattern candidates need the active frame, body, pattern, value, guard, branch ops, and diagnostic path together."
    )]
    fn lower_guarded_candidate(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
        pattern: AwbcPatternId,
        value: AwbcRegisterId,
        guard: &RuntimeExpr,
        ops: &[FlowOp],
        path: &str,
    ) -> GuardedCandidate {
        let restored_scope_depth = frame.scope_depth();
        let scope = frame.enter_scope();
        self.inventory
            .push_instruction(AwbcInstruction::EnterScope { scope });
        self.inventory
            .push_instruction(AwbcInstruction::BindPattern {
                pattern,
                value,
                mode: AwbcBindMode::Declare,
            });
        let guard = AwbcExprLowerer::new(self.inventory, frame, format!("{path}.guard"), self.plan)
            .lower(guard);
        let body_block = AwbcBlockId(table_index(
            self.inventory.program.blocks.len().saturating_add(1),
        ));
        let guard_branch_block = body.close_block(
            self.inventory,
            AwbcTerminator::Branch {
                condition: guard,
                then_block: body_block,
                else_block: body_block,
            },
            AwbcSafePointKind::None,
        );

        self.lower_ops(frame, body, ops, &format!("{path}.then"));
        if !body.terminated {
            self.inventory
                .push_instruction(AwbcInstruction::ExitScope { scope });
            frame.exit_scope();
        }
        frame.restore_scope_depth_after_branch(restored_scope_depth);

        let fallthrough = if body.terminated {
            None
        } else {
            Some(self.close_jump_to_join(body))
        };
        let guard_false_block = if body.terminated {
            body.reopen_after_terminated_branch(self.inventory)
        } else {
            AwbcBlockId(table_index(self.inventory.program.blocks.len()))
        };
        patch_branch_else_block(self.inventory, guard_branch_block, guard_false_block);
        self.inventory
            .push_instruction(AwbcInstruction::ExitScope { scope });
        let guard_false_jump = body.close_block(
            self.inventory,
            AwbcTerminator::Jump {
                target: AwbcBlockId::default(),
            },
            AwbcSafePointKind::None,
        );
        GuardedCandidate {
            guard_false_jump,
            fallthrough,
        }
    }

    fn close_jump_to_join(&mut self, body: &mut FlowBodyBuilder) -> AwbcBlockId {
        body.close_block(
            self.inventory,
            AwbcTerminator::Jump {
                target: AwbcBlockId::default(),
            },
            AwbcSafePointKind::None,
        )
    }

    fn finish_join(&mut self, body: &mut FlowBodyBuilder, join: BranchJoin) {
        if join.fallthroughs.is_empty() {
            return;
        }
        let join_block = if body.terminated {
            body.reopen_after_terminated_branch(self.inventory)
        } else {
            AwbcBlockId(table_index(self.inventory.program.blocks.len()))
        };
        for block in join.fallthroughs {
            patch_jump_target(self.inventory, block, join_block);
        }
    }

    fn terminate_pattern_mismatch(&mut self, body: &mut FlowBodyBuilder, message: &str) {
        let message = self.inventory.intern_string(message);
        body.terminate(
            self.inventory,
            AwbcTerminator::Trap {
                code: AwbcTrapCode::PatternMismatch,
                message: Some(message),
            },
            AwbcSafePointKind::None,
        );
    }

    fn lower_for(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
        input: ForLoweringInput<'_>,
    ) {
        if let RuntimeIteratorEvidence::Witness(witness) = input.evidence {
            match &witness.executable {
                RuntimeIteratorWitnessExecutable::TraitCalls { into_iter, next } => {
                    self.lower_trait_call_for(frame, body, input, *into_iter, *next);
                }
                RuntimeIteratorWitnessExecutable::IdentityIntoIterator { next } => {
                    self.lower_identity_trait_call_for(frame, body, input, *next);
                }
            }
            return;
        }
        self.lower_intrinsic_iterator_for(frame, body, input);
    }

    fn lower_intrinsic_iterator_for(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
        input: ForLoweringInput<'_>,
    ) {
        let Some(evidence_label) = input.evidence.awbc_label() else {
            self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                input.path.to_owned(),
                "witness-backed IntoIterator lowering requires executable trait method bodies",
            ));
            return;
        };
        let source =
            AwbcExprLowerer::new(self.inventory, frame, input.path, self.plan).lower(input.source);
        let evidence = self.lower_iterator_evidence_constant(frame, evidence_label);
        let iterator = frame.runtime_state(self.inventory.dynamic_ty());
        let into_iter = self.intrinsic("core.iter.into_iter", 2, Some(self.inventory.dynamic_ty()));
        self.inventory
            .push_instruction(AwbcInstruction::CallIntrinsic {
                dst: Some(iterator),
                intrinsic: into_iter,
                args: vec![source, evidence],
            });

        let condition_block = AwbcBlockId(table_index(
            self.inventory.program.blocks.len().saturating_add(1),
        ));
        body.close_block(
            self.inventory,
            AwbcTerminator::Jump {
                target: condition_block,
            },
            AwbcSafePointKind::None,
        );

        let next_pair = frame.temp(self.inventory.dynamic_ty());
        let next = self.intrinsic("core.iter.next", 1, Some(self.inventory.dynamic_ty()));
        self.inventory
            .push_instruction(AwbcInstruction::CallIntrinsic {
                dst: Some(next_pair),
                intrinsic: next,
                args: vec![iterator],
            });
        self.inventory
            .push_instruction(AwbcInstruction::ProjectTuple {
                dst: iterator,
                target: next_pair,
                ordinal: 0,
            });
        let next_value = frame.temp(self.inventory.dynamic_ty());
        self.inventory
            .push_instruction(AwbcInstruction::ProjectTuple {
                dst: next_value,
                target: next_pair,
                ordinal: 1,
            });
        let condition_ty = self.inventory.bool_ty();
        let condition = frame.temp(condition_ty);
        let is_some = self.intrinsic("core.option.is_some", 1, Some(condition_ty));
        self.inventory
            .push_instruction(AwbcInstruction::CallIntrinsic {
                dst: Some(condition),
                intrinsic: is_some,
                args: vec![next_value],
            });
        let condition_block = AwbcBlockId(table_index(self.inventory.program.blocks.len()));
        let body_block = AwbcBlockId(condition_block.0.saturating_add(1));
        body.close_block(
            self.inventory,
            AwbcTerminator::Branch {
                condition,
                then_block: body_block,
                else_block: body_block,
            },
            AwbcSafePointKind::LoopBackedge,
        );

        let scope = frame.enter_scope();
        self.inventory
            .push_instruction(AwbcInstruction::EnterScope { scope });
        let value = frame.temp(self.inventory.dynamic_ty());
        let unwrap = self.intrinsic("core.option.unwrap", 1, Some(self.inventory.dynamic_ty()));
        self.inventory
            .push_instruction(AwbcInstruction::CallIntrinsic {
                dst: Some(value),
                intrinsic: unwrap,
                args: vec![next_value],
            });
        let pattern = lower_pattern(self.inventory, self.plan, frame, input.pattern);
        self.inventory
            .push_instruction(AwbcInstruction::BindPattern {
                pattern,
                value,
                mode: AwbcBindMode::Declare,
            });
        self.lower_ops(frame, body, input.ops, &format!("{}.body", input.path));
        if body.terminated {
            frame.exit_scope();
            self.reopen_loop_exit_after_terminated_body(body, condition_block);
            return;
        }
        self.close_iterator_for_iteration(frame, body, scope, condition_block);
    }

    fn lower_trait_call_for(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
        input: ForLoweringInput<'_>,
        into_iter: RuntimeTraitMethodId,
        next: RuntimeTraitMethodId,
    ) {
        let Some(into_iter) = self.inventory.trait_method(into_iter) else {
            self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                input.path.to_owned(),
                "iterator witness refers to an unlowered IntoIterator method",
            ));
            return;
        };
        let source =
            AwbcExprLowerer::new(self.inventory, frame, input.path, self.plan).lower(input.source);
        let iterator = frame.runtime_state(self.inventory.dynamic_ty());
        self.inventory
            .push_instruction(AwbcInstruction::CallTraitMethod {
                dst: iterator,
                method: into_iter,
                receiver: source,
                args: Vec::new(),
                receiver_out: None,
            });
        self.lower_trait_iterator_loop(frame, body, input, iterator, next);
    }

    fn lower_identity_trait_call_for(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
        input: ForLoweringInput<'_>,
        next: RuntimeTraitMethodId,
    ) {
        let source =
            AwbcExprLowerer::new(self.inventory, frame, input.path, self.plan).lower(input.source);
        let iterator = frame.runtime_state(self.inventory.dynamic_ty());
        self.inventory.push_instruction(AwbcInstruction::Move {
            dst: iterator,
            src: source,
        });
        self.lower_trait_iterator_loop(frame, body, input, iterator, next);
    }

    fn lower_trait_iterator_loop(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
        input: ForLoweringInput<'_>,
        iterator: AwbcRegisterId,
        next: RuntimeTraitMethodId,
    ) {
        let Some(next) = self.inventory.trait_method(next) else {
            self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                input.path.to_owned(),
                "iterator witness refers to an unlowered Iterator::next method",
            ));
            return;
        };
        let condition_block = AwbcBlockId(table_index(
            self.inventory.program.blocks.len().saturating_add(1),
        ));
        body.close_block(
            self.inventory,
            AwbcTerminator::Jump {
                target: condition_block,
            },
            AwbcSafePointKind::None,
        );

        let next_value = frame.temp(self.inventory.dynamic_ty());
        self.inventory
            .push_instruction(AwbcInstruction::CallTraitMethod {
                dst: next_value,
                method: next,
                receiver: iterator,
                args: Vec::new(),
                receiver_out: Some(iterator),
            });
        let condition_ty = self.inventory.bool_ty();
        let condition = frame.temp(condition_ty);
        let is_some = self.intrinsic("core.option.is_some", 1, Some(condition_ty));
        self.inventory
            .push_instruction(AwbcInstruction::CallIntrinsic {
                dst: Some(condition),
                intrinsic: is_some,
                args: vec![next_value],
            });
        let condition_block = AwbcBlockId(table_index(self.inventory.program.blocks.len()));
        let body_block = AwbcBlockId(condition_block.0.saturating_add(1));
        body.close_block(
            self.inventory,
            AwbcTerminator::Branch {
                condition,
                then_block: body_block,
                else_block: body_block,
            },
            AwbcSafePointKind::LoopBackedge,
        );

        let scope = frame.enter_scope();
        self.inventory
            .push_instruction(AwbcInstruction::EnterScope { scope });
        let value = frame.temp(self.inventory.dynamic_ty());
        let unwrap = self.intrinsic("core.option.unwrap", 1, Some(self.inventory.dynamic_ty()));
        self.inventory
            .push_instruction(AwbcInstruction::CallIntrinsic {
                dst: Some(value),
                intrinsic: unwrap,
                args: vec![next_value],
            });
        let pattern = lower_pattern(self.inventory, self.plan, frame, input.pattern);
        self.inventory
            .push_instruction(AwbcInstruction::BindPattern {
                pattern,
                value,
                mode: AwbcBindMode::Declare,
            });
        self.lower_ops(frame, body, input.ops, &format!("{}.body", input.path));
        if body.terminated {
            frame.exit_scope();
            self.reopen_loop_exit_after_terminated_body(body, condition_block);
            return;
        }
        self.close_iterator_for_iteration(frame, body, scope, condition_block);
    }

    fn reopen_loop_exit_after_terminated_body(
        &mut self,
        body: &mut FlowBodyBuilder,
        condition_block: AwbcBlockId,
    ) {
        let after_block = body.reopen_after_terminated_branch(self.inventory);
        patch_branch_else_block(self.inventory, condition_block, after_block);
    }

    fn close_active_scopes_for_terminator(&mut self, frame: &mut FrameBuilder) {
        for scope in frame.active_scope_ids_for_exit() {
            self.inventory
                .push_instruction(AwbcInstruction::ExitScope { scope });
        }
        frame.exit_all_scopes();
    }

    fn lower_iterator_evidence_constant(
        &mut self,
        frame: &mut FrameBuilder,
        evidence: &str,
    ) -> AwbcRegisterId {
        let value = RuntimeValue::String(evidence.to_owned());
        let constant = self.inventory.constant_runtime_value(&value);
        let dst = frame.temp(self.inventory.dynamic_ty());
        self.inventory
            .push_instruction(AwbcInstruction::LoadConst { dst, constant });
        dst
    }

    fn close_iterator_for_iteration(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
        scope: AwbcScopeId,
        condition_block: AwbcBlockId,
    ) {
        self.inventory
            .push_instruction(AwbcInstruction::ExitScope { scope });
        frame.exit_scope();
        body.close_block(
            self.inventory,
            AwbcTerminator::Jump {
                target: condition_block,
            },
            AwbcSafePointKind::LoopBackedge,
        );
        let after_block = AwbcBlockId(table_index(self.inventory.program.blocks.len()));
        patch_branch_else_block(self.inventory, condition_block, after_block);
    }

    fn intrinsic(
        &mut self,
        label: &str,
        arity: usize,
        result: Option<arcweft_core::awbc::schema::AwbcTypeId>,
    ) -> AwbcIntrinsicId {
        if let Some((index, _)) =
            self.inventory
                .program
                .intrinsics
                .iter()
                .enumerate()
                .find(|(_, candidate)| {
                    self.inventory.string(candidate.public_id) == label
                        && self
                            .inventory
                            .program
                            .signatures
                            .get(candidate.signature.index())
                            .is_some_and(|signature| {
                                signature.params.len() == arity && signature.result == result
                            })
                })
        {
            return AwbcIntrinsicId(table_index(index));
        }
        let id = AwbcIntrinsicId(table_index(self.inventory.program.intrinsics.len()));
        let public_id = self.inventory.intern_string(label);
        let signature = self.inventory.intern_signature(
            vec![self.inventory.dynamic_ty(); arity],
            result,
            AwbcEffectSetId(0),
        );
        self.inventory.program.intrinsics.push(AwbcIntrinsic {
            public_id,
            registry_code: 0,
            signature,
            revision: 1,
        });
        id
    }

    fn push_intrinsic_call(&mut self, label: &str, args: Vec<AwbcRegisterId>) {
        let intrinsic = self.intrinsic(label, args.len(), None);
        self.inventory
            .push_instruction(AwbcInstruction::CallIntrinsic {
                dst: None,
                intrinsic,
                args,
            });
    }
}

fn awbc_dialogue_value_role(role: RuntimeDialogueValueRole) -> AwbcDialogueValueRole {
    match role {
        RuntimeDialogueValueRole::Interpolation => AwbcDialogueValueRole::Interpolation,
        RuntimeDialogueValueRole::Condition => AwbcDialogueValueRole::Condition,
    }
}

fn patch_branch_else_block(
    inventory: &mut AwbcInventory,
    branch_block: AwbcBlockId,
    else_block: AwbcBlockId,
) {
    let Some(block) = inventory.program.blocks.get_mut(branch_block.index()) else {
        return;
    };
    let AwbcTerminator::Branch {
        else_block: target, ..
    } = &mut block.terminator
    else {
        return;
    };
    *target = else_block;
}

fn patch_jump_target(
    inventory: &mut AwbcInventory,
    jump_block: AwbcBlockId,
    target_block: AwbcBlockId,
) {
    let Some(block) = inventory.program.blocks.get_mut(jump_block.index()) else {
        return;
    };
    let AwbcTerminator::Jump { target } = &mut block.terminator else {
        return;
    };
    *target = target_block;
}

fn entry_target_flows<'a>(
    entries: impl IntoIterator<Item = &'a RuntimeEntrySpec>,
) -> BTreeSet<FlowRuntimeId> {
    let mut targets = BTreeSet::new();
    for entry in entries {
        match &entry.target {
            RuntimeEntryTarget::Flow(flow) | RuntimeEntryTarget::Controller(flow) => {
                targets.insert(flow.clone());
            }
            RuntimeEntryTarget::Routes(routes) => {
                targets.extend(routes.iter().map(|route| route.target.clone()));
            }
        }
    }
    targets
}

fn selected_flow_closure(plan: &RuntimePlan, entry: &RuntimeEntrySpec) -> BTreeSet<FlowRuntimeId> {
    let mut selected = entry_target_flows([entry]);
    loop {
        let mut discovered = BTreeSet::new();
        let mut has_dynamic_target = false;
        for flow in plan
            .flows()
            .iter()
            .filter(|flow| selected.contains(&flow.id))
        {
            collect_flow_dependencies(&flow.ops, &mut discovered, &mut has_dynamic_target);
        }
        if has_dynamic_target {
            return plan.flows().iter().map(|flow| flow.id.clone()).collect();
        }
        let previous_len = selected.len();
        selected.extend(discovered);
        if selected.len() == previous_len {
            return selected;
        }
    }
}

fn collect_flow_dependencies(
    ops: &[FlowOp],
    targets: &mut BTreeSet<FlowRuntimeId>,
    has_dynamic_target: &mut bool,
) {
    for op in ops {
        let terminates = match op {
            FlowOp::Choice { options, .. } => {
                targets.extend(options.iter().filter_map(|option| option.target.clone()));
                false
            }
            FlowOp::Goto(target) => {
                targets.insert(target.clone());
                true
            }
            FlowOp::GotoExpr(_) => {
                *has_dynamic_target = true;
                true
            }
            FlowOp::LetElse { else_ops, .. } => {
                collect_flow_dependencies(else_ops, targets, has_dynamic_target);
                false
            }
            FlowOp::If {
                then_ops, else_ops, ..
            }
            | FlowOp::IfLet {
                then_ops, else_ops, ..
            } => {
                collect_flow_dependencies(then_ops, targets, has_dynamic_target);
                collect_flow_dependencies(else_ops, targets, has_dynamic_target);
                false
            }
            FlowOp::Match { arms, .. } => {
                for arm in arms {
                    collect_flow_dependencies(&arm.ops, targets, has_dynamic_target);
                }
                false
            }
            FlowOp::Loop { body }
            | FlowOp::LetLoop { body, .. }
            | FlowOp::While { body, .. }
            | FlowOp::WhileLet { body, .. }
            | FlowOp::For { body, .. }
            | FlowOp::Thread { body, .. }
            | FlowOp::Scope(body) => {
                collect_flow_dependencies(body, targets, has_dynamic_target);
                false
            }
            FlowOp::LoopNext { body }
            | FlowOp::WhileNext { body, .. }
            | FlowOp::WhileLetNext { body, .. }
            | FlowOp::ForNext { body, .. } => {
                collect_flow_dependencies(body.as_ref(), targets, has_dynamic_target);
                false
            }
            FlowOp::LetScope { ops, .. } => {
                collect_flow_dependencies(ops, targets, has_dynamic_target);
                false
            }
            FlowOp::Return(_) | FlowOp::ReturnExpr(_) => true,
            FlowOp::Bind(_)
            | FlowOp::Let { .. }
            | FlowOp::AssignNominalField { .. }
            | FlowOp::Dialogue { .. }
            | FlowOp::Await { .. }
            | FlowOp::AwaitMany { .. }
            | FlowOp::HostCall { .. }
            | FlowOp::Break(_)
            | FlowOp::Continue
            | FlowOp::Effect(_)
            | FlowOp::EvaluatedEffect(_)
            | FlowOp::RegisterCleanup { .. }
            | FlowOp::CancelCleanup { .. }
            | FlowOp::EnterScope
            | FlowOp::ExitScope
            | FlowOp::ExitScopeBind { .. }
            | FlowOp::Noop => false,
        };
        if terminates {
            break;
        }
    }
}

fn flow_public_id(flow: &FlowRuntimeId) -> String {
    flow.public_label().into_string()
}
