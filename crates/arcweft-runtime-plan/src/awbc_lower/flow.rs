use crate::awbc_lower::expr::AwbcExprLowerer;
use crate::awbc_lower::frame::FrameBuilder;
use crate::awbc_lower::inventory::{AwbcInventory, AwbcLowerDiagnostic};
use crate::awbc_lower::line::AwbcLineLowerer;
use crate::awbc_lower::pattern::lower_pattern;
use crate::awbc_lower::{table_index, table_range_len};
use arcweft_core::awbc::schema::{
    AwbcBindMode, AwbcBlock, AwbcChoiceId, AwbcChoiceOption, AwbcFunction, AwbcFunctionFlags,
    AwbcFunctionId, AwbcFunctionKind, AwbcInstruction, AwbcIntrinsic, AwbcIntrinsicId,
    AwbcLineTaskGroupId, AwbcRegisterId, AwbcSafePointKind, AwbcScopeId, AwbcTableRange,
    AwbcTerminator,
};
use arcweft_core::plan::{ChoiceRuntimeOption, FlowOp, RuntimeFlow, RuntimeMatchArm, RuntimePlan};

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
        for (index, group) in plan.line_task_groups.iter().enumerate() {
            let group_id = self.inventory.lower_line_task_group(group);
            let public_id = format!("line_task_group.{index}");
            self.inventory
                .intern_content_unit(&public_id, Some(group_id));
        }
        for flow in &plan.flows {
            self.lower_flow(flow);
        }
        self.inventory.lower_entries(plan);
    }

    pub fn into_diagnostics(mut self) -> Vec<AwbcLowerDiagnostic> {
        self.diagnostics.extend(self.inventory.take_diagnostics());
        self.diagnostics
    }

    fn lower_flow(&mut self, flow: &RuntimeFlow) -> AwbcFunctionId {
        let mut frame = FrameBuilder::new();
        let entry_owner = AwbcFunctionId(table_index(self.inventory.program.functions.len()));
        let instruction_start = table_index(self.inventory.program.instructions.len());
        self.lower_ops(&mut frame, &flow.ops, &flow.id.0);
        let instruction_len =
            table_range_len(instruction_start, self.inventory.program.instructions.len());
        let block = self.inventory.push_block(AwbcBlock {
            owner: entry_owner,
            instructions: AwbcTableRange::new(instruction_start, instruction_len),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::Return,
            source_map: None,
        });
        let layout = self
            .inventory
            .intern_frame_layout(format!("flow:{}", flow.id.0), frame.finish());
        let signature = self.inventory.intern_unit_signature();
        let public_id = self.inventory.intern_string(&flow.id.0);
        self.inventory.push_function(
            Some(flow.id.0.as_str()),
            AwbcFunction {
                public_id: Some(public_id),
                kind: AwbcFunctionKind::Flow,
                signature,
                frame_layout: layout,
                blocks: AwbcTableRange::new(block.0, 1),
                entry_block: block,
                flags: AwbcFunctionFlags(
                    AwbcFunctionFlags::MAY_SUSPEND | AwbcFunctionFlags::DETERMINISTIC,
                ),
            },
        )
    }

    fn lower_ops(&mut self, frame: &mut FrameBuilder, ops: &[FlowOp], path: &str) {
        for (index, op) in ops.iter().enumerate() {
            self.lower_op(frame, op, &format!("{path}.{index}"));
        }
    }

    #[allow(clippy::too_many_lines)]
    fn lower_op(&mut self, frame: &mut FrameBuilder, op: &FlowOp, path: &str) {
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
                self.lower_ops(frame, else_ops, &format!("{path}.else"));
            }
            FlowOp::Dialogue { line, task_group } => {
                let group = AwbcLineTaskGroupId(table_index(*task_group));
                let content = AwbcLineLowerer::new(self.inventory).content_for_line(line, group);
                self.inventory
                    .push_instruction(AwbcInstruction::EnsureContent { content });
                self.push_intrinsic_call("flow.dialogue.safe_point", Vec::new());
            }
            FlowOp::Choice { id, options } => {
                let choice = self.lower_choice(id.as_deref(), options);
                self.push_intrinsic_call(&format!("flow.choice#{}", choice.0), Vec::new());
            }
            FlowOp::Await {
                binding,
                target,
                pending,
            } => {
                for effect in pending {
                    let effect = self.inventory.intern_effect(effect);
                    self.inventory
                        .push_instruction(AwbcInstruction::EmitEffect {
                            effect,
                            args: Vec::new(),
                        });
                }
                let task = self
                    .inventory
                    .intern_host_task(&target.task.0, &target.request);
                let dst = frame.temp(self.inventory.dynamic_ty());
                self.inventory.push_instruction(AwbcInstruction::StartTask {
                    dst,
                    plan: task,
                    args: Vec::new(),
                });
                if let Some(binding) = binding {
                    let pattern = lower_pattern(self.inventory, frame, binding);
                    self.inventory
                        .push_instruction(AwbcInstruction::BindPattern {
                            pattern,
                            value: dst,
                            mode: AwbcBindMode::Declare,
                        });
                }
            }
            FlowOp::AwaitMany {
                binding,
                target,
                pending,
            } => {
                for effect in pending {
                    let effect = self.inventory.intern_effect(effect);
                    self.inventory
                        .push_instruction(AwbcInstruction::EmitEffect {
                            effect,
                            args: Vec::new(),
                        });
                }
                let source =
                    AwbcExprLowerer::new(self.inventory, frame, path).lower(&target.source);
                let task = self
                    .inventory
                    .intern_host_task(&target.task.0, &target.request);
                let _binding = binding
                    .as_ref()
                    .map(|binding| lower_pattern(self.inventory, frame, binding));
                self.push_intrinsic_call(&format!("await_many#{}", task.0), vec![source]);
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
                self.lower_ops(frame, then_ops, &format!("{path}.then"));
                self.lower_ops(frame, else_ops, &format!("{path}.else"));
                self.inventory
                    .push_instruction(AwbcInstruction::ExitScope { scope });
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
                self.lower_ops(frame, then_ops, &format!("{path}.then"));
                self.lower_ops(frame, else_ops, &format!("{path}.else"));
            }
            FlowOp::Match { scrutinee, arms } => self.lower_match(frame, scrutinee, arms, path),
            FlowOp::Loop { body } | FlowOp::LetLoop { body, .. } | FlowOp::Thread { body, .. } => {
                let scope = frame.enter_scope();
                self.inventory
                    .push_instruction(AwbcInstruction::EnterScope { scope });
                self.lower_ops(frame, body, &format!("{path}.body"));
                self.push_intrinsic_call("flow.loop.backedge", Vec::new());
                self.inventory
                    .push_instruction(AwbcInstruction::ExitScope { scope });
                frame.exit_scope();
            }
            FlowOp::LoopNext { body }
            | FlowOp::WhileNext { body, .. }
            | FlowOp::WhileLetNext { body, .. }
            | FlowOp::ForNext { body, .. } => self.lower_ops(frame, body, &format!("{path}.next")),
            FlowOp::While { condition, body } => {
                let _ = AwbcExprLowerer::new(self.inventory, frame, path).lower(condition);
                self.lower_ops(frame, body, &format!("{path}.body"));
            }
            FlowOp::WhileLet {
                pattern,
                expr,
                guard,
                body,
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
                self.lower_ops(frame, body, &format!("{path}.body"));
            }
            FlowOp::For {
                pattern,
                source,
                body,
            } => {
                let source = AwbcExprLowerer::new(self.inventory, frame, path).lower(source);
                let pattern = lower_pattern(self.inventory, frame, pattern);
                self.push_intrinsic_call("flow.for.iter", vec![source]);
                let _ = pattern;
                self.lower_ops(frame, body, &format!("{path}.body"));
            }
            FlowOp::Scope(ops) => {
                let scope = frame.enter_scope();
                self.inventory
                    .push_instruction(AwbcInstruction::EnterScope { scope });
                self.lower_ops(frame, ops, &format!("{path}.scope"));
                self.inventory
                    .push_instruction(AwbcInstruction::ExitScope { scope });
                frame.exit_scope();
            }
            FlowOp::LetScope {
                pattern,
                ops,
                value,
            } => {
                self.lower_ops(frame, ops, &format!("{path}.let_scope"));
                let value = AwbcExprLowerer::new(self.inventory, frame, path).lower(value);
                let pattern = lower_pattern(self.inventory, frame, pattern);
                self.inventory
                    .push_instruction(AwbcInstruction::BindPattern {
                        pattern,
                        value,
                        mode: AwbcBindMode::Declare,
                    });
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
                let _ = value;
                self.push_intrinsic_call("flow.return_expr", Vec::new());
            }
            FlowOp::Continue => {
                self.push_intrinsic_call("flow.continue", Vec::new());
            }
            FlowOp::Goto(target) => {
                self.push_intrinsic_call(&format!("goto.static:{}", target.0), Vec::new());
            }
            FlowOp::GotoExpr(expr) => {
                let target = AwbcExprLowerer::new(self.inventory, frame, path).lower(expr);
                self.push_intrinsic_call("goto.dynamic", vec![target]);
            }
            FlowOp::Return(value) => {
                let value = self.inventory.constant_string(value);
                let dst = frame.return_value(self.inventory.string_ty());
                self.inventory.push_instruction(AwbcInstruction::LoadConst {
                    dst,
                    constant: value,
                });
            }
            FlowOp::Effect(effect) => {
                let effect = self.inventory.intern_effect(effect);
                self.inventory
                    .push_instruction(AwbcInstruction::EmitEffect {
                        effect,
                        args: Vec::new(),
                    });
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
            self.lower_ops(frame, &arm.ops, &format!("{path}.arm"));
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
