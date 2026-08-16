use crate::awbc_lower::frame::FrameBuilder;
use crate::awbc_lower::inventory::{AwbcInventory, AwbcLowerDiagnostic, PendingAwbcClosure};
use crate::awbc_lower::pattern::{lower_pattern, plan_type, variant_case_name};
use crate::awbc_lower::{table_index, table_range_len};
use arcweft_core::awbc::schema::{
    AwbcBinaryOp, AwbcBindMode, AwbcBlock, AwbcBlockId, AwbcEffectSetId, AwbcFunction,
    AwbcFunctionFlags, AwbcFunctionKind, AwbcInstruction, AwbcIntrinsic, AwbcIntrinsicId,
    AwbcPatternId, AwbcPureHelperId, AwbcRegisterId, AwbcRuntimeType, AwbcSafePointKind,
    AwbcScopeId, AwbcTableRange, AwbcTerminator, AwbcTraitMethodId, AwbcTrapCode, AwbcUnaryOp,
};
use arcweft_core::pattern::RuntimePattern;
use arcweft_core::plan::{RuntimePlan, RuntimeReceiverMode};
use arcweft_core::value::{
    RuntimeBinaryOp, RuntimeCallTarget, RuntimeExpr, RuntimeExprKind, RuntimeExprMatchArm,
    RuntimeUnaryOp,
};

/// Expression lowerer used by flow/source/stream builders.
pub struct AwbcExprLowerer<'a, 'b, 'plan> {
    pub inventory: &'a mut AwbcInventory,
    pub frame: &'b mut FrameBuilder,
    pub plan: &'plan RuntimePlan,
    pub path: String,
}

impl<'a, 'b, 'plan> AwbcExprLowerer<'a, 'b, 'plan> {
    pub fn new(
        inventory: &'a mut AwbcInventory,
        frame: &'b mut FrameBuilder,
        path: impl Into<String>,
        plan: &'plan RuntimePlan,
    ) -> Self {
        Self {
            inventory,
            frame,
            plan,
            path: path.into(),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "This mirrors the RuntimeExpr enum one arm at a time until expression lowering is split by expression family."
    )]
    /// Lowers one admitted runtime expression into the current AWBC frame.
    ///
    /// # Panics
    ///
    /// Panics only when an already-admitted runtime method-call identity cannot
    /// be reconstructed as the typed core callable identity it originated from.
    pub fn lower(&mut self, expr: &RuntimeExpr) -> AwbcRegisterId {
        match expr.kind() {
            RuntimeExprKind::Value(value) => {
                let ty = self.inventory.intern_runtime_value_type(value);
                let dst = self.frame.temp(ty);
                let constant = self.inventory.constant_runtime_value(value);
                self.inventory
                    .push_instruction(AwbcInstruction::LoadConst { dst, constant });
                dst
            }
            RuntimeExprKind::Local(name) => {
                self.frame.register_for_local(*name).unwrap_or_else(|| {
                    self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                        self.path.clone(),
                        format!("local `{name}` is read before it is allocated in AWBC frame"),
                    ));
                    self.frame.temp(self.inventory.dynamic_ty())
                })
            }
            RuntimeExprKind::EntityRef(value) => {
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                let constant = self.inventory.constant_runtime_value(
                    &arcweft_core::value::RuntimeValue::EntityRef(value.runtime_label()),
                );
                self.inventory
                    .push_instruction(AwbcInstruction::LoadConst { dst, constant });
                dst
            }
            RuntimeExprKind::Agent(agent) => {
                let mut operands = Vec::with_capacity(
                    agent.operands().len() + usize::from(agent.choice().is_some()),
                );
                if let Some(choice) = agent.choice() {
                    operands.push(self.load_runtime_const(
                        &arcweft_core::value::RuntimeValue::EntityRef(choice.as_str().to_owned()),
                    ));
                }
                operands.extend(
                    agent
                        .operands()
                        .into_iter()
                        .map(|operand| self.lower(operand)),
                );
                let constructor = agent.constructor();
                let ty = self
                    .inventory
                    .intern_type(AwbcRuntimeType::Agent(constructor.result_type()));
                let dst = self.frame.temp(ty);
                self.inventory.push_instruction(AwbcInstruction::MakeAgent {
                    dst,
                    constructor,
                    operands,
                });
                dst
            }
            RuntimeExprKind::Let {
                binding,
                expr,
                body,
            } => {
                let value = self.lower(expr);
                let local = self
                    .frame
                    .local(*binding, plan_type(self.inventory, self.plan, expr.ty()));
                self.inventory.push_instruction(AwbcInstruction::Move {
                    dst: local,
                    src: value,
                });
                self.lower(body)
            }
            RuntimeExprKind::Tuple(items) => {
                let registers = items.iter().map(|item| self.lower(item)).collect();
                let ty = self.inventory.intern_type(AwbcRuntimeType::Tuple(vec![
                    self.inventory
                        .dynamic_ty();
                    items.len()
                ]));
                let dst = self.frame.temp(ty);
                self.inventory.push_instruction(AwbcInstruction::MakeTuple {
                    dst,
                    items: registers,
                });
                dst
            }
            RuntimeExprKind::BracketSeq(items) => {
                let registers = items.iter().map(|item| self.lower(item)).collect();
                let ty = self
                    .inventory
                    .intern_type(AwbcRuntimeType::Sequence(self.inventory.dynamic_ty()));
                let dst = self.frame.temp(ty);
                self.inventory
                    .push_instruction(AwbcInstruction::MakeSequence {
                        dst,
                        items: registers,
                    });
                dst
            }
            RuntimeExprKind::RepeatSeq { value, len } => {
                let value = self.lower(value);
                let len_reg = self.frame.temp(self.inventory.i64_ty());
                let constant = self
                    .inventory
                    .constant_runtime_value(&arcweft_core::value::RuntimeValue::usize(*len as u64));
                self.inventory.push_instruction(AwbcInstruction::LoadConst {
                    dst: len_reg,
                    constant,
                });
                let ty = self
                    .inventory
                    .intern_type(AwbcRuntimeType::Sequence(self.inventory.dynamic_ty()));
                let dst = self.frame.temp(ty);
                self.inventory
                    .push_instruction(AwbcInstruction::RepeatSequence {
                        dst,
                        value,
                        len: len_reg,
                    });
                dst
            }
            RuntimeExprKind::Range {
                start,
                end,
                inclusive,
            } => {
                let start = self.lower_optional_range_bound(start.as_deref());
                let end = self.lower_optional_range_bound(end.as_deref());
                let inclusive =
                    self.load_runtime_const(&arcweft_core::value::RuntimeValue::Bool(*inclusive));
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                let intrinsic = self.intern_intrinsic("core.range", 3);
                self.inventory
                    .push_instruction(AwbcInstruction::CallIntrinsic {
                        dst: Some(dst),
                        intrinsic,
                        args: vec![start, end, inclusive],
                    });
                dst
            }
            RuntimeExprKind::NominalRecord(record) => {
                let Some(domain) = self.plan.nominal_record_domains().get(expr.ty()) else {
                    self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                        self.path.clone(),
                        format!(
                            "nominal record expression type {} has no RuntimePlan record domain",
                            expr.ty()
                        ),
                    ));
                    return self.frame.temp(self.inventory.dynamic_ty());
                };
                let ty = plan_type(self.inventory, self.plan, expr.ty());
                let mut registers = vec![None; domain.fields().len()];
                for initializer in record.initializers() {
                    let value = self.lower(initializer.value());
                    registers[initializer.field().zero_based() as usize] = Some(value);
                }
                let registers = registers
                    .into_iter()
                    .map(|register| {
                        register.expect(
                            "checked nominal record expression must initialize every layout field",
                        )
                    })
                    .collect();
                let field_names = domain
                    .fields()
                    .iter()
                    .map(|field| self.inventory.intern_string(field.name()))
                    .collect();
                let dst = self.frame.temp(ty);
                self.inventory
                    .push_instruction(AwbcInstruction::MakeRecord {
                        dst,
                        ty,
                        field_names,
                        fields: registers,
                    });
                dst
            }
            RuntimeExprKind::Variant { ordinal, payload } => {
                let ty = plan_type(self.inventory, self.plan, expr.ty());
                let dst = self.frame.temp(ty);
                let payload = payload.as_deref().map(|payload| self.lower(payload));
                let case_name = variant_case_name(self.inventory, self.plan, expr.ty(), *ordinal);
                self.inventory
                    .push_instruction(AwbcInstruction::MakeVariant {
                        dst,
                        ty,
                        case: *ordinal,
                        case_name,
                        payload,
                    });
                dst
            }
            RuntimeExprKind::Field { target, field } => {
                let target = self.lower(target);
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                let field = self.inventory.intern_string(&field.label());
                self.inventory
                    .push_instruction(AwbcInstruction::ProjectField { dst, target, field });
                dst
            }
            RuntimeExprKind::ProjectTuple { target, ordinal } => {
                let target = self.lower(target);
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                self.inventory
                    .push_instruction(AwbcInstruction::ProjectTuple {
                        dst,
                        target,
                        ordinal: table_index(*ordinal),
                    });
                dst
            }
            RuntimeExprKind::ProjectRecord { target, ordinal } => {
                let target = self.lower(target);
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                self.inventory
                    .push_instruction(AwbcInstruction::ProjectRecord {
                        dst,
                        target,
                        ordinal: table_index(*ordinal),
                    });
                dst
            }
            RuntimeExprKind::AssignNominalField {
                base,
                field,
                expr,
                body,
            } => {
                let Some(target) = self.frame.register_for_local(*base) else {
                    self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                        self.path.clone(),
                        format!("field assignment base `{base}` is not in the AWBC frame"),
                    ));
                    return self.lower(body);
                };
                let value = self.lower(expr);
                self.inventory
                    .push_instruction(AwbcInstruction::AssignRecordField {
                        target,
                        field: field.zero_based(),
                        value,
                    });
                self.lower(body)
            }
            RuntimeExprKind::Call { callee, args } => self.lower_call(callee, args),
            RuntimeExprKind::Function(site) => self.lower_function_site(*site),
            RuntimeExprKind::Apply { callee, args } => {
                let callee = self.lower(callee);
                let args = args
                    .iter()
                    .map(|arg| self.lower(arg.value()))
                    .collect::<Vec<_>>();
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                self.inventory
                    .push_instruction(AwbcInstruction::ApplyFunction { dst, callee, args });
                dst
            }
            RuntimeExprKind::TraitCall {
                callable,
                receiver,
                receiver_mode,
                args,
            } => {
                let receiver_register = self.lower(receiver);
                let args = args.iter().map(|arg| self.lower(arg.value())).collect();
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                let receiver_out = (*receiver_mode == RuntimeReceiverMode::MutRef).then(|| {
                    if matches!(receiver.kind(), RuntimeExprKind::Local(_)) {
                        receiver_register
                    } else {
                        self.frame.temp(self.inventory.dynamic_ty())
                    }
                });
                self.inventory
                    .push_instruction(AwbcInstruction::CallTraitMethod {
                        dst,
                        method: AwbcTraitMethodId(table_index(callable.0)),
                        receiver: receiver_register,
                        args,
                        receiver_out,
                    });
                dst
            }
            RuntimeExprKind::PureCall { helper, args } => {
                let args = args.iter().map(|arg| self.lower(arg.value())).collect();
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                self.inventory
                    .push_instruction(AwbcInstruction::CallPureHelper {
                        dst,
                        helper: AwbcPureHelperId(table_index(helper.0)),
                        args,
                    });
                dst
            }
            RuntimeExprKind::Map {
                source,
                param,
                body,
            } => {
                let source = self.lower(source);
                let _ = self
                    .frame
                    .local(*param, plan_type(self.inventory, self.plan, body.ty()));
                let body = self.lower(body);
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                let intrinsic = self.intern_intrinsic("seq.map", 2);
                self.inventory
                    .push_instruction(AwbcInstruction::CallIntrinsic {
                        dst: Some(dst),
                        intrinsic,
                        args: vec![source, body],
                    });
                dst
            }
            RuntimeExprKind::Filter {
                source,
                param,
                body,
            } => {
                let source = self.lower(source);
                let _ = self
                    .frame
                    .local(*param, plan_type(self.inventory, self.plan, body.ty()));
                let body = self.lower(body);
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                let intrinsic = self.intern_intrinsic("seq.filter", 2);
                self.inventory
                    .push_instruction(AwbcInstruction::CallIntrinsic {
                        dst: Some(dst),
                        intrinsic,
                        args: vec![source, body],
                    });
                dst
            }
            RuntimeExprKind::Sum { source } => {
                let source = self.lower(source);
                let dst = self.frame.temp(self.inventory.i64_ty());
                let intrinsic = self.intern_intrinsic("seq.sum", 1);
                self.inventory
                    .push_instruction(AwbcInstruction::CallIntrinsic {
                        dst: Some(dst),
                        intrinsic,
                        args: vec![source],
                    });
                dst
            }
            RuntimeExprKind::Unary { op, expr } => {
                let src = self.lower(expr);
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                self.inventory.push_instruction(AwbcInstruction::Unary {
                    dst,
                    op: unary_op(*op),
                    src,
                });
                dst
            }
            RuntimeExprKind::Binary { lhs, op, rhs } => {
                let lhs = self.lower(lhs);
                let rhs = self.lower(rhs);
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                self.inventory.push_instruction(AwbcInstruction::Binary {
                    dst,
                    op: binary_op(*op),
                    lhs,
                    rhs,
                });
                dst
            }
            RuntimeExprKind::If { .. }
            | RuntimeExprKind::IfLet { .. }
            | RuntimeExprKind::Match { .. } => self.lower_value_control_expr(expr),
            RuntimeExprKind::ReductionUnchanged { state } => {
                let state = self.lower(state);
                let ty = plan_type(self.inventory, self.plan, expr.ty());
                let dst = self.frame.temp(ty);
                self.inventory
                    .push_instruction(AwbcInstruction::MakeReductionUnchanged { dst, ty, state });
                dst
            }
        }
    }

    fn lower_call(
        &mut self,
        callee: &RuntimeCallTarget,
        args: &[arcweft_core::value::RuntimeCallArgument],
    ) -> AwbcRegisterId {
        let args = args
            .iter()
            .map(|arg| self.lower(arg.value()))
            .collect::<Vec<_>>();
        let dst = self.frame.temp(self.inventory.dynamic_ty());
        let intrinsic = self.intern_intrinsic(callee.as_label(), args.len());
        self.inventory
            .push_instruction(AwbcInstruction::CallIntrinsic {
                dst: Some(dst),
                intrinsic,
                args,
            });
        dst
    }

    fn lower_optional_range_bound(&mut self, expr: Option<&RuntimeExpr>) -> AwbcRegisterId {
        if let Some(expr) = expr {
            self.lower(expr)
        } else {
            self.load_runtime_const(&arcweft_core::value::RuntimeValue::Unit)
        }
    }

    fn lower_function_site(
        &mut self,
        site: arcweft_core::runtime_id::RuntimeFunctionSiteId,
    ) -> AwbcRegisterId {
        let Some(function_site) = self.plan.function_sites().get(site) else {
            self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                self.path.clone(),
                format!("function site {site} is absent from the RuntimePlan"),
            ));
            return self.frame.temp(self.inventory.dynamic_ty());
        };
        let already_lowered = self.inventory.function_site_function(site).is_some();
        let function = self.inventory.reserve_function_site_slot(site);
        let captures = function_site
            .captures()
            .iter()
            .filter_map(|local| {
                self.frame
                    .register_for_local(*local)
                    .map(|register| (*local, register))
            })
            .collect::<Vec<_>>();
        if !already_lowered {
            self.inventory.push_pending_closure(PendingAwbcClosure {
                function,
                params: function_site.params().into(),
                captures: function_site.captures().into(),
                body: function_site.body().clone(),
                path: format!("{}.function.{site}", self.path),
            });
        }
        let params = function_site
            .params()
            .iter()
            .map(|local| local_name(self.inventory, *local))
            .collect();
        let capture_names = captures
            .iter()
            .map(|(local, _)| local_name(self.inventory, *local))
            .collect();
        let dst = self.frame.temp(self.inventory.dynamic_ty());
        self.inventory
            .push_instruction(AwbcInstruction::MakeFunction {
                dst,
                function,
                params,
                capture_names,
                captures: captures.iter().map(|(_, register)| *register).collect(),
            });
        dst
    }

    fn lower_value_control_expr(&mut self, expr: &RuntimeExpr) -> AwbcRegisterId {
        let captures = self.frame.capture_slots();
        let function = self.inventory.reserve_function_slot();
        self.inventory.push_pending_closure(PendingAwbcClosure {
            function,
            params: Box::new([]),
            captures: captures.iter().map(|capture| capture.local).collect(),
            body: expr.clone(),
            path: format!("{}.control.{}", self.path, function.0),
        });

        let callee = self.frame.temp(self.inventory.dynamic_ty());
        let capture_names = captures
            .iter()
            .map(|capture| local_name(self.inventory, capture.local))
            .collect();
        self.inventory
            .push_instruction(AwbcInstruction::MakeFunction {
                dst: callee,
                function,
                params: Vec::new(),
                capture_names,
                captures: captures.iter().map(|capture| capture.register).collect(),
            });
        let dst = self.frame.temp(self.inventory.dynamic_ty());
        self.inventory
            .push_instruction(AwbcInstruction::ApplyFunction {
                dst,
                callee,
                args: Vec::new(),
            });
        dst
    }

    fn load_runtime_const(&mut self, value: &arcweft_core::value::RuntimeValue) -> AwbcRegisterId {
        let dst = self.frame.temp(self.inventory.dynamic_ty());
        let constant = self.inventory.constant_runtime_value(value);
        self.inventory
            .push_instruction(AwbcInstruction::LoadConst { dst, constant });
        dst
    }

    fn intern_intrinsic(&mut self, label: &str, arity: usize) -> AwbcIntrinsicId {
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
                            .is_some_and(|signature| signature.params.len() == arity)
                })
        {
            return AwbcIntrinsicId(table_index(index));
        }
        let signature = self.inventory.intern_dynamic_value_signature(arity);
        let id = AwbcIntrinsicId(table_index(self.inventory.program.intrinsics.len()));
        let public_id = self.inventory.intern_string(label);
        self.inventory.program.intrinsics.push(AwbcIntrinsic {
            public_id,
            registry_code: 0,
            signature,
            revision: 1,
        });
        id
    }
}

pub(crate) fn lower_pending_closures(inventory: &mut AwbcInventory, plan: &RuntimePlan) {
    while let Some(closure) = inventory.pop_pending_closure() {
        let mut frame = FrameBuilder::new();
        for local in &closure.captures {
            frame.parameter(*local, plan_type(inventory, plan, local_type(plan, *local)));
        }
        for local in &closure.params {
            frame.parameter(*local, plan_type(inventory, plan, local_type(plan, *local)));
        }

        let mut body = ExprBodyBuilder::new(inventory, closure.function);
        lower_closure_body(
            inventory,
            &mut frame,
            plan,
            &mut body,
            &closure.body,
            &closure.path,
        );
        let layout =
            inventory.intern_frame_layout(format!("{}:frame", closure.path), frame.finish());
        let block = body.block_start;
        let block_len = table_range_len(block.0, inventory.program.blocks.len());
        let params = closure
            .captures
            .iter()
            .chain(closure.params.iter())
            .map(|local| plan_type(inventory, plan, local_type(plan, *local)))
            .collect();
        let result = plan_type(inventory, plan, closure.body.ty());
        let signature = inventory.intern_signature(params, Some(result), AwbcEffectSetId(0));
        inventory.replace_function(
            closure.function,
            AwbcFunction {
                public_id: None,
                kind: AwbcFunctionKind::Synthetic,
                signature,
                frame_layout: layout,
                blocks: AwbcTableRange::new(block.0, block_len),
                entry_block: block,
                flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
            },
        );
    }
}

fn local_type(
    plan: &RuntimePlan,
    local: arcweft_core::runtime_id::RuntimeLocalDeclarationId,
) -> arcweft_core::runtime_id::RuntimePlanTypeId {
    plan.local_declarations().get(local).map_or_else(
        || panic!("admitted RuntimePlan local {local} is absent"),
        arcweft_core::plan::RuntimeLocalDeclaration::ty,
    )
}

fn local_name(
    inventory: &mut AwbcInventory,
    local: arcweft_core::runtime_id::RuntimeLocalDeclarationId,
) -> arcweft_core::awbc::schema::AwbcStringId {
    inventory.intern_string(&format!("local.{local}"))
}

struct ExprBodyBuilder {
    owner: arcweft_core::awbc::schema::AwbcFunctionId,
    block_start: AwbcBlockId,
    instruction_start: u32,
    terminated: bool,
}

impl ExprBodyBuilder {
    fn new(inventory: &AwbcInventory, owner: arcweft_core::awbc::schema::AwbcFunctionId) -> Self {
        let block_start = AwbcBlockId(table_index(inventory.program.blocks.len()));
        Self {
            owner,
            block_start,
            instruction_start: table_index(inventory.program.instructions.len()),
            terminated: false,
        }
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
        self.close_block(inventory, terminator, safe_point);
        self.terminated = true;
    }
}

fn lower_closure_body(
    inventory: &mut AwbcInventory,
    frame: &mut FrameBuilder,
    plan: &RuntimePlan,
    body: &mut ExprBodyBuilder,
    expr: &RuntimeExpr,
    path: &str,
) {
    match expr.kind() {
        RuntimeExprKind::If {
            condition,
            then_expr,
            else_expr,
        } => lower_if_value_expr(
            inventory,
            frame,
            plan,
            body,
            IfValueExprInput {
                condition,
                then_expr,
                else_expr,
                path,
            },
        ),
        RuntimeExprKind::IfLet {
            pattern,
            expr,
            guard,
            then_expr,
            else_expr,
        } => lower_if_let_value_expr(
            inventory,
            frame,
            plan,
            body,
            IfLetValueExprInput {
                pattern,
                expr,
                guard: guard.as_deref(),
                then_expr,
                else_expr,
                path,
            },
        ),
        RuntimeExprKind::Match { scrutinee, arms } => {
            lower_match_value_expr(inventory, frame, plan, body, scrutinee, arms, path);
        }
        _ => terminate_return_expr(inventory, frame, plan, body, expr, path, None),
    }
}

#[derive(Clone, Copy)]
struct IfValueExprInput<'a> {
    condition: &'a RuntimeExpr,
    then_expr: &'a RuntimeExpr,
    else_expr: &'a RuntimeExpr,
    path: &'a str,
}

fn lower_if_value_expr(
    inventory: &mut AwbcInventory,
    frame: &mut FrameBuilder,
    plan: &RuntimePlan,
    body: &mut ExprBodyBuilder,
    input: IfValueExprInput<'_>,
) {
    let condition = AwbcExprLowerer::new(inventory, frame, input.path, plan).lower(input.condition);
    let then_block = AwbcBlockId(table_index(
        inventory.program.blocks.len().saturating_add(1),
    ));
    let branch_block = body.close_block(
        inventory,
        AwbcTerminator::Branch {
            condition,
            then_block,
            else_block: then_block,
        },
        AwbcSafePointKind::CallableBoundary,
    );
    terminate_return_expr(
        inventory,
        frame,
        plan,
        body,
        input.then_expr,
        &format!("{}.then", input.path),
        None,
    );
    let else_block = body.reopen_after_terminated_branch(inventory);
    patch_branch_else_block(inventory, branch_block, else_block);
    terminate_return_expr(
        inventory,
        frame,
        plan,
        body,
        input.else_expr,
        &format!("{}.else", input.path),
        None,
    );
}

#[derive(Clone, Copy)]
struct IfLetValueExprInput<'a> {
    pattern: &'a RuntimePattern,
    expr: &'a RuntimeExpr,
    guard: Option<&'a RuntimeExpr>,
    then_expr: &'a RuntimeExpr,
    else_expr: &'a RuntimeExpr,
    path: &'a str,
}

fn lower_if_let_value_expr(
    inventory: &mut AwbcInventory,
    frame: &mut FrameBuilder,
    plan: &RuntimePlan,
    body: &mut ExprBodyBuilder,
    input: IfLetValueExprInput<'_>,
) {
    let value = AwbcExprLowerer::new(inventory, frame, input.path, plan).lower(input.expr);
    let pattern = lower_branch_pattern(inventory, plan, frame, input.pattern);
    let matched = frame.temp(inventory.bool_ty());
    inventory.push_instruction(AwbcInstruction::TestPattern {
        dst: matched,
        pattern,
        value,
    });
    let candidate_block = AwbcBlockId(table_index(
        inventory.program.blocks.len().saturating_add(1),
    ));
    let branch_block = body.close_block(
        inventory,
        AwbcTerminator::Branch {
            condition: matched,
            then_block: candidate_block,
            else_block: candidate_block,
        },
        AwbcSafePointKind::CallableBoundary,
    );

    if let Some(guard) = input.guard {
        let scope = enter_pattern_scope(inventory, frame, pattern, value);
        let guard = AwbcExprLowerer::new(inventory, frame, format!("{}.guard", input.path), plan)
            .lower(guard);
        let then_block = AwbcBlockId(table_index(
            inventory.program.blocks.len().saturating_add(1),
        ));
        let guard_branch_block = body.close_block(
            inventory,
            AwbcTerminator::Branch {
                condition: guard,
                then_block,
                else_block: then_block,
            },
            AwbcSafePointKind::CallableBoundary,
        );
        terminate_return_expr(
            inventory,
            frame,
            plan,
            body,
            input.then_expr,
            &format!("{}.then", input.path),
            Some(scope),
        );
        let guard_false_block = body.reopen_after_terminated_branch(inventory);
        patch_branch_else_block(inventory, guard_branch_block, guard_false_block);
        inventory.push_instruction(AwbcInstruction::ExitScope { scope });
        let guard_false_jump = body.close_block(
            inventory,
            AwbcTerminator::Jump {
                target: AwbcBlockId::default(),
            },
            AwbcSafePointKind::CallableBoundary,
        );
        let else_block = AwbcBlockId(table_index(inventory.program.blocks.len()));
        patch_branch_else_block(inventory, branch_block, else_block);
        patch_jump_target(inventory, guard_false_jump, else_block);
        terminate_return_expr(
            inventory,
            frame,
            plan,
            body,
            input.else_expr,
            &format!("{}.else", input.path),
            None,
        );
    } else {
        let scope = enter_pattern_scope(inventory, frame, pattern, value);
        terminate_return_expr(
            inventory,
            frame,
            plan,
            body,
            input.then_expr,
            &format!("{}.then", input.path),
            Some(scope),
        );
        let else_block = body.reopen_after_terminated_branch(inventory);
        patch_branch_else_block(inventory, branch_block, else_block);
        terminate_return_expr(
            inventory,
            frame,
            plan,
            body,
            input.else_expr,
            &format!("{}.else", input.path),
            None,
        );
    }
}

fn lower_match_value_expr(
    inventory: &mut AwbcInventory,
    frame: &mut FrameBuilder,
    plan: &RuntimePlan,
    body: &mut ExprBodyBuilder,
    scrutinee: &RuntimeExpr,
    arms: &[RuntimeExprMatchArm],
    path: &str,
) {
    let scrutinee = AwbcExprLowerer::new(inventory, frame, path, plan).lower(scrutinee);
    for (index, arm) in arms.iter().enumerate() {
        let pattern = lower_branch_pattern(inventory, plan, frame, arm.pattern());
        let matched = frame.temp(inventory.bool_ty());
        inventory.push_instruction(AwbcInstruction::TestPattern {
            dst: matched,
            pattern,
            value: scrutinee,
        });
        let candidate_block = AwbcBlockId(table_index(
            inventory.program.blocks.len().saturating_add(1),
        ));
        let branch_block = body.close_block(
            inventory,
            AwbcTerminator::Branch {
                condition: matched,
                then_block: candidate_block,
                else_block: candidate_block,
            },
            AwbcSafePointKind::CallableBoundary,
        );

        if let Some(guard) = arm.guard() {
            let scope = enter_pattern_scope(inventory, frame, pattern, scrutinee);
            let guard =
                AwbcExprLowerer::new(inventory, frame, format!("{path}.arm.{index}.guard"), plan)
                    .lower(guard);
            let body_block = AwbcBlockId(table_index(
                inventory.program.blocks.len().saturating_add(1),
            ));
            let guard_branch_block = body.close_block(
                inventory,
                AwbcTerminator::Branch {
                    condition: guard,
                    then_block: body_block,
                    else_block: body_block,
                },
                AwbcSafePointKind::CallableBoundary,
            );
            terminate_return_expr(
                inventory,
                frame,
                plan,
                body,
                arm.value(),
                &format!("{path}.arm.{index}.value"),
                Some(scope),
            );
            let guard_false_block = body.reopen_after_terminated_branch(inventory);
            patch_branch_else_block(inventory, guard_branch_block, guard_false_block);
            inventory.push_instruction(AwbcInstruction::ExitScope { scope });
            let guard_false_jump = body.close_block(
                inventory,
                AwbcTerminator::Jump {
                    target: AwbcBlockId::default(),
                },
                AwbcSafePointKind::CallableBoundary,
            );
            let next_arm_block = AwbcBlockId(table_index(inventory.program.blocks.len()));
            patch_branch_else_block(inventory, branch_block, next_arm_block);
            patch_jump_target(inventory, guard_false_jump, next_arm_block);
        } else {
            let scope = enter_pattern_scope(inventory, frame, pattern, scrutinee);
            terminate_return_expr(
                inventory,
                frame,
                plan,
                body,
                arm.value(),
                &format!("{path}.arm.{index}.value"),
                Some(scope),
            );
            let next_arm_block = body.reopen_after_terminated_branch(inventory);
            patch_branch_else_block(inventory, branch_block, next_arm_block);
        }
    }
    let message = inventory.intern_string("match pattern did not match");
    body.terminate(
        inventory,
        AwbcTerminator::Trap {
            code: AwbcTrapCode::PatternMismatch,
            message: Some(message),
        },
        AwbcSafePointKind::CallableBoundary,
    );
}

fn terminate_return_expr(
    inventory: &mut AwbcInventory,
    frame: &mut FrameBuilder,
    plan: &RuntimePlan,
    body: &mut ExprBodyBuilder,
    expr: &RuntimeExpr,
    path: &str,
    exit_scope: Option<AwbcScopeId>,
) {
    let mut value = AwbcExprLowerer::new(inventory, frame, path, plan).lower(expr);
    if let Some(scope) = exit_scope {
        let scoped_value = value;
        value = frame.root_temp(inventory.dynamic_ty());
        inventory.push_instruction(AwbcInstruction::Move {
            dst: value,
            src: scoped_value,
        });
        inventory.push_instruction(AwbcInstruction::ExitScope { scope });
        frame.exit_scope();
    }
    body.terminate(
        inventory,
        AwbcTerminator::Return { value: Some(value) },
        AwbcSafePointKind::CallableBoundary,
    );
}

fn lower_branch_pattern(
    inventory: &mut AwbcInventory,
    plan: &RuntimePlan,
    frame: &mut FrameBuilder,
    pattern: &RuntimePattern,
) -> AwbcPatternId {
    let restored_scope_depth = frame.scope_depth();
    let _ = frame.enter_scope();
    let pattern = lower_pattern(inventory, plan, frame, pattern);
    frame.restore_scope_depth_after_branch(restored_scope_depth);
    pattern
}

fn enter_pattern_scope(
    inventory: &mut AwbcInventory,
    frame: &mut FrameBuilder,
    pattern: AwbcPatternId,
    value: AwbcRegisterId,
) -> AwbcScopeId {
    let scope = frame.enter_scope();
    inventory.push_instruction(AwbcInstruction::EnterScope { scope });
    inventory.push_instruction(AwbcInstruction::BindPattern {
        pattern,
        value,
        mode: AwbcBindMode::Declare,
    });
    scope
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

fn unary_op(op: RuntimeUnaryOp) -> AwbcUnaryOp {
    match op {
        RuntimeUnaryOp::Not => AwbcUnaryOp::Not,
        RuntimeUnaryOp::Neg => AwbcUnaryOp::Neg,
    }
}

fn binary_op(op: RuntimeBinaryOp) -> AwbcBinaryOp {
    match op {
        RuntimeBinaryOp::Eq => AwbcBinaryOp::Eq,
        RuntimeBinaryOp::Ne => AwbcBinaryOp::Ne,
        RuntimeBinaryOp::Lt => AwbcBinaryOp::Lt,
        RuntimeBinaryOp::Le => AwbcBinaryOp::Le,
        RuntimeBinaryOp::Gt => AwbcBinaryOp::Gt,
        RuntimeBinaryOp::Ge => AwbcBinaryOp::Ge,
        RuntimeBinaryOp::Add => AwbcBinaryOp::Add,
        RuntimeBinaryOp::Sub => AwbcBinaryOp::Sub,
        RuntimeBinaryOp::Mul => AwbcBinaryOp::Mul,
        RuntimeBinaryOp::Div => AwbcBinaryOp::Div,
        RuntimeBinaryOp::And => AwbcBinaryOp::And,
        RuntimeBinaryOp::Or => AwbcBinaryOp::Or,
    }
}
