use crate::awbc_lower::AwbcAudioLowerer;
use crate::awbc_lower::expr::AwbcExprLowerer;
use crate::awbc_lower::frame::FrameBuilder;
use crate::awbc_lower::inventory::{AwbcInventory, AwbcLowerDiagnostic};
use crate::awbc_lower::line::AwbcLineLowerer;
use crate::awbc_lower::pattern::lower_pattern;
use crate::awbc_lower::{table_index, table_range_len};
use arcweft_core::audio::RuntimeAudioCommand;
use arcweft_core::awbc::schema::{
    AwbcBindMode, AwbcBlock, AwbcBlockId, AwbcChoiceId, AwbcChoiceOption, AwbcEffectSetId,
    AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId, AwbcFunctionKind,
    AwbcInstruction, AwbcIntrinsic, AwbcIntrinsicId, AwbcLineTaskGroupId, AwbcPureHelper,
    AwbcPureHelperOrigin, AwbcRegisterId, AwbcResumePoint, AwbcResumePointId, AwbcSafePointKind,
    AwbcScopeId, AwbcTableRange, AwbcTerminator,
};
use arcweft_core::effect::LineEffectRequest;
use arcweft_core::pattern::RuntimePattern;
use arcweft_core::plan::{
    ChoiceRuntimeOption, FlowOp, RuntimeEntryTarget, RuntimeFlow, RuntimeMatchArm, RuntimePlan,
    RuntimePureHelper, RuntimePureHelperOrigin,
};
use arcweft_core::value::RuntimeExpr;
use std::collections::BTreeSet;

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
}

struct FlowBody {
    entry_block: AwbcBlockId,
    blocks: AwbcTableRange,
    resume_points: Vec<AwbcResumePointId>,
    returns_value: bool,
    has_dynamic_target: bool,
}

/// Lowers all flow entry functions and public entries.
pub struct AwbcFlowLowerer<'a> {
    inventory: &'a mut AwbcInventory,
    diagnostics: Vec<AwbcLowerDiagnostic>,
}

impl<'a> AwbcFlowLowerer<'a> {
    pub fn new(inventory: &'a mut AwbcInventory) -> Self {
        Self {
            inventory,
            diagnostics: Vec::new(),
        }
    }

    pub fn lower_plan(&mut self, plan: &RuntimePlan) {
        for helper in &plan.pure_helpers {
            self.lower_pure_helper(helper);
        }
        for (index, group) in plan.line_task_groups.iter().enumerate() {
            let group_id = self.inventory.lower_line_task_group(group);
            let public_id = format!("line_task_group.{index}");
            self.inventory
                .intern_content_unit(&public_id, Some(group_id));
        }

        let function_start = table_index(self.inventory.program.functions.len());
        for (index, flow) in plan.flows.iter().enumerate() {
            let offset = u32::try_from(index).unwrap_or(u32::MAX);
            self.inventory.reserve_function_name(
                &flow.id.0,
                AwbcFunctionId(function_start.saturating_add(offset)),
            );
        }
        let entry_targets = entry_target_flow_names(plan);
        for flow in &plan.flows {
            let entry_parameters = if entry_targets.contains(flow.id.0.as_str()) {
                infer_entry_parameter_names(&flow.ops)
            } else {
                Vec::new()
            };
            self.lower_flow(flow, &entry_parameters);
        }
        self.inventory.lower_entries(plan);
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

        let owner = AwbcFunctionId(table_index(self.inventory.program.functions.len()));
        let mut frame = FrameBuilder::new();
        let dynamic_ty = self.inventory.dynamic_ty();
        for input in &helper.input_names {
            let name = self.inventory.intern_string(input);
            frame.parameter(input, name, dynamic_ty);
        }
        let instruction_start = table_index(self.inventory.program.instructions.len());
        let value =
            AwbcExprLowerer::new(self.inventory, &mut frame, format!("pure.{}", helper.name))
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
        let signature = self
            .inventory
            .intern_dynamic_value_signature(helper.input_names.len());
        let function = self.inventory.push_function(
            Some(&helper.name),
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

    pub fn into_diagnostics(mut self) -> Vec<AwbcLowerDiagnostic> {
        self.diagnostics.extend(self.inventory.take_diagnostics());
        self.diagnostics
    }

    fn lower_flow(&mut self, flow: &RuntimeFlow, entry_parameters: &[String]) -> AwbcFunctionId {
        let mut frame = FrameBuilder::new();
        let owner = self
            .inventory
            .function_by_name(&flow.id.0)
            .unwrap_or_else(|| AwbcFunctionId(table_index(self.inventory.program.functions.len())));
        let dynamic_ty = self.inventory.dynamic_ty();
        for parameter in entry_parameters {
            let name = self.inventory.intern_string(parameter);
            frame.parameter(parameter, name, dynamic_ty);
        }
        let mut body = FlowBodyBuilder::new(self.inventory, owner);
        self.lower_ops(&mut frame, &mut body, &flow.ops, &flow.id.0);
        let body = body.finish(self.inventory);
        let layout = self
            .inventory
            .intern_frame_layout(format!("flow:{}", flow.id.0), frame.finish());
        for resume in body.resume_points {
            if let Some(point) = self.inventory.program.resume_points.get_mut(resume.index()) {
                point.frame_layout = layout;
            }
        }
        let params = vec![self.inventory.dynamic_ty(); entry_parameters.len()];
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
        let public_id = self.inventory.intern_string(&flow.id.0);
        let mut flags = AwbcFunctionFlags::MAY_SUSPEND | AwbcFunctionFlags::DETERMINISTIC;
        if body.has_dynamic_target {
            flags |= AwbcFunctionFlags::HAS_DYNAMIC_TARGET;
        }
        let function = self.inventory.push_function(
            Some(flow.id.0.as_str()),
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
                    let name_id = self.inventory.intern_string(&binding.name);
                    let local = frame.local(&binding.name, name_id, self.inventory.dynamic_ty());
                    let constant = self.inventory.constant_runtime_value(&binding.value);
                    self.inventory.push_instruction(AwbcInstruction::LoadConst {
                        dst: local,
                        constant,
                    });
                }
            }
            FlowOp::Let { pattern, expr } | FlowOp::ExitScopeBind { pattern, expr } => {
                let value = AwbcExprLowerer::new(self.inventory, frame, path).lower(expr);
                let pattern = lower_pattern(self.inventory, frame, pattern);
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
                let value = AwbcExprLowerer::new(self.inventory, frame, path).lower(expr);
                let pattern_id = lower_pattern(self.inventory, frame, pattern);
                let matched = frame.temp(self.inventory.bool_ty());
                self.inventory
                    .push_instruction(AwbcInstruction::TestPattern {
                        dst: matched,
                        pattern: pattern_id,
                        value,
                    });
                self.lower_ops(frame, body, else_ops, &format!("{path}.else"));
            }
            FlowOp::Dialogue { line, task_group } => {
                let group = AwbcLineTaskGroupId(table_index(*task_group));
                let content = AwbcLineLowerer::new(self.inventory).content_for_line(line, group);
                body.suspend(self.inventory, AwbcSafePointKind::Dialogue, |resume| {
                    AwbcTerminator::Dialogue {
                        content,
                        line_task_group: group,
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
                let task = self.inventory.intern_host_task(
                    &target.need.0,
                    &target.task.0,
                    &target.request,
                );
                let args = target
                    .request
                    .args
                    .iter()
                    .map(|arg| AwbcExprLowerer::new(self.inventory, frame, path).lower(arg.value()))
                    .collect::<Vec<_>>();
                let task_handle = frame.temp(self.inventory.dynamic_ty());
                self.inventory.push_instruction(AwbcInstruction::StartTask {
                    dst: task_handle,
                    plan: task,
                    args,
                });
                let binding = binding
                    .as_ref()
                    .map(|binding| lower_pattern(self.inventory, frame, binding));
                body.suspend(self.inventory, AwbcSafePointKind::Await, |resume| {
                    AwbcTerminator::Await {
                        task: task_handle,
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
                let source =
                    AwbcExprLowerer::new(self.inventory, frame, path).lower(&target.source);
                let task = self.inventory.intern_host_task(
                    &target.need.0,
                    &target.task.0,
                    &target.request,
                );
                let item_name = self.inventory.intern_string(&target.item_binding);
                let item_binding =
                    frame.local(&target.item_binding, item_name, self.inventory.dynamic_ty());
                self.inventory
                    .set_await_many_policy(task, item_binding, target.limit);
                let binding = binding
                    .as_ref()
                    .map(|binding| lower_pattern(self.inventory, frame, binding));
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
                    .map(|arg| AwbcExprLowerer::new(self.inventory, frame, path).lower(arg))
                    .collect::<Vec<_>>();
                let dst = binding
                    .as_ref()
                    .map(|_| frame.temp(self.inventory.dynamic_ty()));
                let pattern = binding
                    .as_ref()
                    .map(|binding| lower_pattern(self.inventory, frame, binding));
                body.suspend(self.inventory, AwbcSafePointKind::HostCall, |resume| {
                    AwbcTerminator::HostCall {
                        call,
                        args,
                        dst,
                        resume,
                    }
                });
                if let (Some(pattern), Some(value)) = (pattern, dst) {
                    self.inventory
                        .push_instruction(AwbcInstruction::BindPattern {
                            pattern,
                            value,
                            mode: AwbcBindMode::Declare,
                        });
                }
            }
            FlowOp::If {
                condition,
                then_ops,
                else_ops,
            } => {
                let _ = AwbcExprLowerer::new(self.inventory, frame, path).lower(condition);
                let scope = frame.enter_scope();
                self.inventory
                    .push_instruction(AwbcInstruction::EnterScope { scope });
                self.lower_ops(frame, body, then_ops, &format!("{path}.then"));
                self.lower_ops(frame, body, else_ops, &format!("{path}.else"));
                if !body.terminated {
                    self.inventory
                        .push_instruction(AwbcInstruction::ExitScope { scope });
                }
                frame.exit_scope();
            }
            FlowOp::IfLet {
                pattern,
                expr,
                guard,
                then_ops,
                else_ops,
            } => {
                let value = AwbcExprLowerer::new(self.inventory, frame, path).lower(expr);
                let pattern = lower_pattern(self.inventory, frame, pattern);
                let matched = frame.temp(self.inventory.bool_ty());
                self.inventory
                    .push_instruction(AwbcInstruction::TestPattern {
                        dst: matched,
                        pattern,
                        value,
                    });
                if let Some(guard) = guard {
                    let _ = AwbcExprLowerer::new(self.inventory, frame, path).lower(guard);
                }
                self.lower_ops(frame, body, then_ops, &format!("{path}.then"));
                self.lower_ops(frame, body, else_ops, &format!("{path}.else"));
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
                self.lower_ops(frame, body, ops, &format!("{path}.next"));
            }
            FlowOp::While {
                condition,
                body: ops,
            } => {
                let _ = AwbcExprLowerer::new(self.inventory, frame, path).lower(condition);
                self.lower_ops(frame, body, ops, &format!("{path}.body"));
            }
            FlowOp::WhileLet {
                pattern,
                expr,
                guard,
                body: ops,
            } => {
                let value = AwbcExprLowerer::new(self.inventory, frame, path).lower(expr);
                let pattern = lower_pattern(self.inventory, frame, pattern);
                let matched = frame.temp(self.inventory.bool_ty());
                self.inventory
                    .push_instruction(AwbcInstruction::TestPattern {
                        dst: matched,
                        pattern,
                        value,
                    });
                if let Some(guard) = guard {
                    let _ = AwbcExprLowerer::new(self.inventory, frame, path).lower(guard);
                }
                self.lower_ops(frame, body, ops, &format!("{path}.body"));
            }
            FlowOp::For {
                pattern,
                source,
                body: ops,
            } => {
                let source = AwbcExprLowerer::new(self.inventory, frame, path).lower(source);
                let pattern = lower_pattern(self.inventory, frame, pattern);
                self.push_intrinsic_call("flow.for.iter", vec![source]);
                let _ = pattern;
                self.lower_ops(frame, body, ops, &format!("{path}.body"));
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
            } => {
                self.lower_ops(frame, body, ops, &format!("{path}.let_scope"));
                if !body.terminated {
                    let value = AwbcExprLowerer::new(self.inventory, frame, path).lower(value);
                    let pattern = lower_pattern(self.inventory, frame, pattern);
                    self.inventory
                        .push_instruction(AwbcInstruction::BindPattern {
                            pattern,
                            value,
                            mode: AwbcBindMode::Declare,
                        });
                }
            }
            FlowOp::Break(value) => {
                if let Some(value) = value {
                    let value = AwbcExprLowerer::new(self.inventory, frame, path).lower(value);
                    let _ = value;
                }
                self.push_intrinsic_call("flow.break", Vec::new());
            }
            FlowOp::ReturnExpr(value) => {
                let value = AwbcExprLowerer::new(self.inventory, frame, path).lower(value);
                body.terminate(
                    self.inventory,
                    AwbcTerminator::Return { value: Some(value) },
                    AwbcSafePointKind::Return,
                );
            }
            FlowOp::Continue => {
                self.push_intrinsic_call("flow.continue", Vec::new());
            }
            FlowOp::Goto(target) => {
                if let Some(function) = self.inventory.function_by_name(&target.0) {
                    body.terminate(
                        self.inventory,
                        AwbcTerminator::GotoStatic {
                            function,
                            args: Vec::new(),
                        },
                        AwbcSafePointKind::CallableBoundary,
                    );
                } else {
                    self.push_intrinsic_call(&format!("goto.static:{}", target.0), Vec::new());
                }
            }
            FlowOp::GotoExpr(expr) => {
                let target = AwbcExprLowerer::new(self.inventory, frame, path).lower(expr);
                body.terminate(
                    self.inventory,
                    AwbcTerminator::GotoDynamic {
                        target,
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
                body.terminate(
                    self.inventory,
                    AwbcTerminator::Return { value: Some(dst) },
                    AwbcSafePointKind::Return,
                );
            }
            FlowOp::Effect(effect) => {
                let (effect, args) = match effect {
                    LineEffectRequest::Audio(command) => {
                        let (command, args) =
                            AwbcAudioLowerer::new(self.inventory, frame, path).lower(command);
                        let effect = self.inventory.intern_audio_effect(command, args.len());
                        (effect, args)
                    }
                    _ => (self.inventory.intern_effect(effect), Vec::new()),
                };
                self.inventory
                    .push_instruction(AwbcInstruction::EmitEffect { effect, args });
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
                    .and_then(|target| self.inventory.function_by_name(&target.0)),
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

    fn lower_match(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
        scrutinee: &arcweft_core::value::RuntimeExpr,
        arms: &[RuntimeMatchArm],
        path: &str,
    ) {
        let scrutinee = AwbcExprLowerer::new(self.inventory, frame, path).lower(scrutinee);
        for arm in arms {
            let pattern = lower_pattern(self.inventory, frame, &arm.pattern);
            let matched = frame.temp(self.inventory.bool_ty());
            self.inventory
                .push_instruction(AwbcInstruction::TestPattern {
                    dst: matched,
                    pattern,
                    value: scrutinee,
                });
            if let Some(guard) = &arm.guard {
                let _ = AwbcExprLowerer::new(self.inventory, frame, path).lower(guard);
            }
            self.lower_ops(frame, body, &arm.ops, &format!("{path}.arm"));
            if body.terminated {
                break;
            }
        }
    }

    fn intrinsic(&mut self, label: &str) -> AwbcIntrinsicId {
        if let Some((index, _)) = self
            .inventory
            .program
            .intrinsics
            .iter()
            .enumerate()
            .find(|(_, candidate)| self.inventory.string(candidate.public_id) == label)
        {
            return AwbcIntrinsicId(table_index(index));
        }
        let id = AwbcIntrinsicId(table_index(self.inventory.program.intrinsics.len()));
        let public_id = self.inventory.intern_string(label);
        let signature = self.inventory.intern_unit_signature();
        self.inventory.program.intrinsics.push(AwbcIntrinsic {
            public_id,
            registry_code: 0,
            signature,
            revision: 1,
        });
        id
    }

    fn push_intrinsic_call(&mut self, label: &str, args: Vec<AwbcRegisterId>) {
        let intrinsic = self.intrinsic(label);
        self.inventory
            .push_instruction(AwbcInstruction::CallIntrinsic {
                dst: None,
                intrinsic,
                args,
            });
    }
}

fn entry_target_flow_names(plan: &RuntimePlan) -> BTreeSet<&str> {
    let mut targets = BTreeSet::new();
    if let Some(entry_flow) = plan.entry_flow.as_ref() {
        targets.insert(entry_flow.0.as_str());
    }
    for entry in &plan.entries {
        match &entry.target {
            RuntimeEntryTarget::Flow(flow) => {
                targets.insert(flow.0.as_str());
            }
            RuntimeEntryTarget::Routes(routes) => {
                targets.extend(routes.iter().map(|route| route.target.0.as_str()));
            }
        }
    }
    targets
}

fn infer_entry_parameter_names(ops: &[FlowOp]) -> Vec<String> {
    let mut collector = EntryParameterCollector::default();
    collector.collect_ops(ops);
    collector.parameters
}

#[derive(Default)]
struct EntryParameterCollector {
    declared: BTreeSet<String>,
    seen_parameters: BTreeSet<String>,
    parameters: Vec<String>,
}

impl EntryParameterCollector {
    fn collect_ops(&mut self, ops: &[FlowOp]) {
        for op in ops {
            self.collect_op(op);
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "FlowOp free-local discovery mirrors the enum so entry parameter inference stays in AWBC lowering."
    )]
    fn collect_op(&mut self, op: &FlowOp) {
        match op {
            FlowOp::Bind(bindings) => {
                self.declared
                    .extend(bindings.iter().map(|binding| binding.name.clone()));
            }
            FlowOp::Let { pattern, expr } | FlowOp::ExitScopeBind { pattern, expr } => {
                self.collect_expr(expr);
                self.declare_pattern(pattern);
            }
            FlowOp::LetElse {
                pattern,
                expr,
                else_ops,
            } => {
                self.collect_expr(expr);
                self.collect_scoped_ops(else_ops);
                self.declare_pattern(pattern);
            }
            FlowOp::Await {
                binding, target, ..
            } => {
                target
                    .request
                    .args
                    .iter()
                    .for_each(|arg| self.collect_expr(arg.value()));
                if let Some(binding) = binding {
                    self.declare_pattern(binding);
                }
            }
            FlowOp::AwaitMany {
                binding, target, ..
            } => {
                self.collect_expr(&target.source);
                self.collect_with_declared(std::slice::from_ref(&target.item_binding), |this| {
                    target
                        .request
                        .args
                        .iter()
                        .for_each(|arg| this.collect_expr(arg.value()));
                });
                if let Some(binding) = binding {
                    self.declare_pattern(binding);
                }
            }
            FlowOp::HostCall { binding, target } => {
                target.args.iter().for_each(|arg| self.collect_expr(arg));
                if let Some(binding) = binding {
                    self.declare_pattern(binding);
                }
            }
            FlowOp::If {
                condition,
                then_ops,
                else_ops,
            } => {
                self.collect_expr(condition);
                self.collect_scoped_ops(then_ops);
                self.collect_scoped_ops(else_ops);
            }
            FlowOp::IfLet {
                pattern,
                expr,
                guard,
                then_ops,
                else_ops,
            } => {
                self.collect_expr(expr);
                self.collect_optional_expr(guard.as_ref());
                let names = pattern_names(pattern);
                self.collect_with_declared(&names, |this| this.collect_ops(then_ops));
                self.collect_scoped_ops(else_ops);
            }
            FlowOp::Match { scrutinee, arms } => {
                self.collect_expr(scrutinee);
                for arm in arms {
                    let names = pattern_names(&arm.pattern);
                    self.collect_with_declared(&names, |this| {
                        this.collect_optional_expr(arm.guard.as_ref());
                        this.collect_ops(&arm.ops);
                    });
                }
            }
            FlowOp::Loop { body }
            | FlowOp::LetLoop { body, .. }
            | FlowOp::Thread { body, .. }
            | FlowOp::Scope(body) => self.collect_scoped_ops(body),
            FlowOp::LoopNext { body }
            | FlowOp::WhileNext { body, .. }
            | FlowOp::WhileLetNext { body, .. }
            | FlowOp::ForNext { body, .. } => self.collect_scoped_ops(body),
            FlowOp::While { condition, body } => {
                self.collect_expr(condition);
                self.collect_scoped_ops(body);
            }
            FlowOp::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => {
                self.collect_expr(expr);
                self.collect_optional_expr(guard.as_ref());
                let names = pattern_names(pattern);
                self.collect_with_declared(&names, |this| this.collect_ops(body));
            }
            FlowOp::For {
                pattern,
                source,
                body,
            } => {
                self.collect_expr(source);
                let names = pattern_names(pattern);
                self.collect_with_declared(&names, |this| this.collect_ops(body));
            }
            FlowOp::LetScope {
                pattern,
                ops,
                value,
            } => {
                self.collect_scoped_ops(ops);
                self.collect_expr(value);
                self.declare_pattern(pattern);
            }
            FlowOp::Break(Some(value)) | FlowOp::GotoExpr(value) | FlowOp::ReturnExpr(value) => {
                self.collect_expr(value);
            }
            FlowOp::Effect(effect) => self.collect_effect(effect),
            FlowOp::Dialogue { .. }
            | FlowOp::Choice { .. }
            | FlowOp::Break(None)
            | FlowOp::Continue
            | FlowOp::Goto(_)
            | FlowOp::Return(_)
            | FlowOp::EnterScope
            | FlowOp::ExitScope
            | FlowOp::Noop => {}
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Entry parameter discovery must enumerate every audio command expression field."
    )]
    fn collect_effect(&mut self, effect: &LineEffectRequest) {
        let LineEffectRequest::Audio(command) = effect else {
            return;
        };
        match command.as_ref() {
            RuntimeAudioCommand::Play {
                voice,
                resource,
                bus,
                gain_db_milli,
                pan_milli,
                start_frame,
                fade_in_millis,
                ..
            } => {
                self.collect_expr(voice);
                self.collect_expr(resource);
                self.collect_expr(bus);
                self.collect_expr(gain_db_milli);
                self.collect_expr(pan_milli);
                self.collect_expr(start_frame);
                self.collect_expr(fade_in_millis);
            }
            RuntimeAudioCommand::Stop {
                voice,
                fade_out_millis,
            } => {
                self.collect_expr(voice);
                self.collect_expr(fade_out_millis);
            }
            RuntimeAudioCommand::StopAll { fade_out_millis } => {
                self.collect_expr(fade_out_millis);
            }
            RuntimeAudioCommand::SetVoiceGain {
                voice,
                gain_db_milli,
                transition_millis,
            } => {
                self.collect_expr(voice);
                self.collect_expr(gain_db_milli);
                self.collect_expr(transition_millis);
            }
            RuntimeAudioCommand::SetVoicePan {
                voice,
                pan_milli,
                transition_millis,
            } => {
                self.collect_expr(voice);
                self.collect_expr(pan_milli);
                self.collect_expr(transition_millis);
            }
            RuntimeAudioCommand::SetBusGain {
                bus,
                gain_db_milli,
                transition_millis,
            } => {
                self.collect_expr(bus);
                self.collect_expr(gain_db_milli);
                self.collect_expr(transition_millis);
            }
            RuntimeAudioCommand::SetBusMute { bus, muted } => {
                self.collect_expr(bus);
                self.collect_expr(muted);
            }
            RuntimeAudioCommand::SetEffectEnabled {
                bus,
                effect,
                enabled,
            } => {
                self.collect_expr(bus);
                self.collect_expr(effect);
                self.collect_expr(enabled);
            }
            RuntimeAudioCommand::SetEffectParameter {
                bus,
                effect,
                value,
                transition_millis,
                ..
            } => {
                self.collect_expr(bus);
                self.collect_expr(effect);
                self.collect_expr(value);
                self.collect_expr(transition_millis);
            }
            RuntimeAudioCommand::ApplySnapshot {
                snapshot,
                transition_millis,
            } => {
                self.collect_expr(snapshot);
                self.collect_expr(transition_millis);
            }
            RuntimeAudioCommand::RequestMicrophone { capture, .. }
            | RuntimeAudioCommand::StopMicrophone { capture } => {
                self.collect_expr(capture);
            }
            RuntimeAudioCommand::SetCaptureMonitor {
                capture,
                bus,
                gain_db_milli,
            } => {
                self.collect_expr(capture);
                self.collect_optional_expr(bus.as_ref());
                self.collect_expr(gain_db_milli);
            }
        }
    }

    fn collect_expr(&mut self, expr: &RuntimeExpr) {
        match expr {
            RuntimeExpr::Local(name) => self.parameter(name),
            RuntimeExpr::Value(_) | RuntimeExpr::EntityRef(_) => {}
            RuntimeExpr::Let { name, expr, body } => {
                self.collect_expr(expr);
                self.collect_with_declared(std::slice::from_ref(name), |this| {
                    this.collect_expr(body);
                });
            }
            RuntimeExpr::Tuple(items) | RuntimeExpr::BracketSeq(items) => {
                for item in items {
                    self.collect_expr(item);
                }
            }
            RuntimeExpr::RepeatSeq { value, .. }
            | RuntimeExpr::Field { target: value, .. }
            | RuntimeExpr::ProjectTuple { target: value, .. }
            | RuntimeExpr::ProjectRecord { target: value, .. }
            | RuntimeExpr::SpreadArg(value)
            | RuntimeExpr::Sum { source: value }
            | RuntimeExpr::Unary { expr: value, .. } => self.collect_expr(value),
            RuntimeExpr::Range { start, end, .. } => {
                self.collect_optional_expr(start.as_deref());
                self.collect_optional_expr(end.as_deref());
            }
            RuntimeExpr::Record(fields) => {
                for field in fields {
                    self.collect_expr(&field.value);
                }
            }
            RuntimeExpr::Variant { payload, .. } => {
                if let Some(payload) = payload {
                    self.collect_expr(payload);
                }
            }
            RuntimeExpr::Call { args, .. } | RuntimeExpr::PureCall { args, .. } => {
                for arg in args {
                    self.collect_expr(arg);
                }
            }
            RuntimeExpr::MethodCall { receiver, args, .. } => {
                self.collect_expr(receiver);
                for arg in args {
                    self.collect_expr(arg);
                }
            }
            RuntimeExpr::Map {
                source,
                param,
                body,
            } => {
                self.collect_expr(source);
                self.collect_with_declared(std::slice::from_ref(param), |this| {
                    this.collect_expr(body);
                });
            }
            RuntimeExpr::Binary { lhs, rhs, .. } => {
                self.collect_expr(lhs);
                self.collect_expr(rhs);
            }
            RuntimeExpr::If {
                condition,
                then_expr,
                else_expr,
            } => {
                self.collect_expr(condition);
                self.collect_expr(then_expr);
                self.collect_expr(else_expr);
            }
            RuntimeExpr::IfLet {
                pattern,
                expr,
                guard,
                then_expr,
                else_expr,
            } => {
                self.collect_expr(expr);
                self.collect_optional_expr(guard.as_deref());
                let names = pattern_names(pattern);
                self.collect_with_declared(&names, |this| this.collect_expr(then_expr));
                self.collect_expr(else_expr);
            }
            RuntimeExpr::Match { scrutinee, arms } => {
                self.collect_expr(scrutinee);
                for arm in arms {
                    let names = pattern_names(&arm.pattern);
                    self.collect_with_declared(&names, |this| {
                        this.collect_optional_expr(arm.guard.as_ref());
                        this.collect_expr(&arm.value);
                    });
                }
            }
        }
    }

    fn collect_optional_expr(&mut self, expr: Option<&RuntimeExpr>) {
        if let Some(expr) = expr {
            self.collect_expr(expr);
        }
    }

    fn collect_scoped_ops(&mut self, ops: &[FlowOp]) {
        let declared = self.declared.clone();
        self.collect_ops(ops);
        self.declared = declared;
    }

    fn collect_with_declared(&mut self, names: &[String], f: impl FnOnce(&mut Self)) {
        let declared = self.declared.clone();
        self.declared.extend(names.iter().cloned());
        f(self);
        self.declared = declared;
    }

    fn declare_pattern(&mut self, pattern: &RuntimePattern) {
        self.declared.extend(pattern_names(pattern));
    }

    fn parameter(&mut self, name: &str) {
        if !self.declared.contains(name) && self.seen_parameters.insert(name.to_owned()) {
            self.parameters.push(name.to_owned());
        }
    }
}

fn pattern_names(pattern: &RuntimePattern) -> Vec<String> {
    match pattern {
        RuntimePattern::Ident(name)
        | RuntimePattern::MutIdent(name)
        | RuntimePattern::Typed { name, .. } => vec![name.clone()],
        RuntimePattern::Whole { name, pattern } => {
            let mut names = vec![name.clone()];
            names.extend(pattern_names(pattern));
            names
        }
        RuntimePattern::Tuple(patterns) => patterns.iter().flat_map(pattern_names).collect(),
        RuntimePattern::Record { fields, .. } => fields
            .iter()
            .flat_map(|field| pattern_names(&field.pattern))
            .collect(),
        RuntimePattern::BracketSeq { items, rest } => {
            let mut names = items.iter().flat_map(pattern_names).collect::<Vec<_>>();
            if let Some(rest) = rest {
                names.push(rest.clone());
            }
            names
        }
        RuntimePattern::Variant { payload, .. } => {
            payload.as_deref().map_or_else(Vec::new, pattern_names)
        }
        RuntimePattern::Discard | RuntimePattern::Literal(_) | RuntimePattern::Entity(_) => {
            Vec::new()
        }
    }
}
