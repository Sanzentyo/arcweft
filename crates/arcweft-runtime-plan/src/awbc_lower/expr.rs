use crate::awbc_lower::frame::FrameBuilder;
use crate::awbc_lower::inventory::{AwbcInventory, AwbcLowerDiagnostic, PendingAwbcClosure};
use crate::awbc_lower::pattern::{lower_pattern, pattern_binding_names};
use crate::awbc_lower::{table_index, table_range_len};
use arcweft_core::awbc::schema::{
    AwbcBinaryOp, AwbcBindMode, AwbcBlock, AwbcBlockId, AwbcEffectSetId, AwbcFunction,
    AwbcFunctionFlags, AwbcFunctionKind, AwbcInstruction, AwbcIntrinsic, AwbcIntrinsicId,
    AwbcPatternId, AwbcPureHelperId, AwbcRegisterId, AwbcRuntimeType, AwbcSafePointKind,
    AwbcScopeId, AwbcTableRange, AwbcTerminator, AwbcTraitMethodId, AwbcTrapCode, AwbcUnaryOp,
};
use arcweft_core::pattern::RuntimePattern;
use arcweft_core::plan::RuntimeReceiverMode;
use arcweft_core::value::{
    RuntimeBinaryOp, RuntimeCallTarget, RuntimeExpr, RuntimeExprMatchArm, RuntimeUnaryOp,
};
use std::collections::BTreeSet;

/// Expression lowerer used by flow/source/stream builders.
pub struct AwbcExprLowerer<'a, 'b> {
    pub inventory: &'a mut AwbcInventory,
    pub frame: &'b mut FrameBuilder,
    pub path: String,
}

impl<'a, 'b> AwbcExprLowerer<'a, 'b> {
    pub fn new(
        inventory: &'a mut AwbcInventory,
        frame: &'b mut FrameBuilder,
        path: impl Into<String>,
    ) -> Self {
        Self {
            inventory,
            frame,
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
        match expr {
            RuntimeExpr::Value(value) => {
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                let constant = self.inventory.constant_runtime_value(value);
                self.inventory
                    .push_instruction(AwbcInstruction::LoadConst { dst, constant });
                dst
            }
            RuntimeExpr::Local(name) => self.frame.register_for_local(name).unwrap_or_else(|| {
                self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                    self.path.clone(),
                    format!("local `{name}` is read before it is allocated in AWBC frame"),
                ));
                self.frame.temp(self.inventory.dynamic_ty())
            }),
            RuntimeExpr::EntityRef(value) => {
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                let constant = self.inventory.constant_runtime_value(
                    &arcweft_core::value::RuntimeValue::EntityRef(value.clone()),
                );
                self.inventory
                    .push_instruction(AwbcInstruction::LoadConst { dst, constant });
                dst
            }
            RuntimeExpr::Let { name, expr, body } => {
                let value = self.lower(expr);
                let name_id = self.inventory.intern_string(name);
                let local = self.frame.local(name, name_id, self.inventory.dynamic_ty());
                self.inventory.push_instruction(AwbcInstruction::Move {
                    dst: local,
                    src: value,
                });
                self.lower(body)
            }
            RuntimeExpr::Tuple(items) => {
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
            RuntimeExpr::BracketSeq(items) => {
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
            RuntimeExpr::RepeatSeq { value, len } => {
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
            RuntimeExpr::Range {
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
            RuntimeExpr::Record(fields) => {
                let field_names = fields
                    .iter()
                    .map(|field| self.inventory.intern_string(&field.name))
                    .collect();
                let registers = fields
                    .iter()
                    .map(|field| self.lower(&field.value))
                    .collect();
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                self.inventory
                    .push_instruction(AwbcInstruction::MakeRecord {
                        dst,
                        ty: self.inventory.dynamic_ty(),
                        field_names,
                        fields: registers,
                    });
                dst
            }
            RuntimeExpr::Variant {
                owner,
                ordinal,
                name,
                payload,
            } => {
                assert!(
                    owner
                        .variant_case(*ordinal)
                        .is_some_and(|case| case.name == *name),
                    "checked runtime variant case must match its typed owner and ordinal"
                );
                let ty = crate::awbc_lower::pattern::intern_runtime_type(self.inventory, owner);
                let dst = self.frame.temp(ty);
                let payload = payload.as_deref().map(|payload| self.lower(payload));
                let case_name = self.inventory.intern_string(name);
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
            RuntimeExpr::Field { target, field } => {
                let target = self.lower(target);
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                let field = self.inventory.intern_string(field);
                self.inventory
                    .push_instruction(AwbcInstruction::ProjectField { dst, target, field });
                dst
            }
            RuntimeExpr::ProjectTuple { target, ordinal } => {
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
            RuntimeExpr::ProjectRecord { target, ordinal } => {
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
            RuntimeExpr::AssignField {
                target,
                field,
                expr,
                body,
            } => {
                let target = match target.as_ref() {
                    RuntimeExpr::Local(name) => {
                        self.frame.register_for_local(name).unwrap_or_else(|| {
                            self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                                self.path.clone(),
                                format!(
                                    "field assignment target `{name}` is not in the AWBC frame"
                                ),
                            ));
                            self.frame.temp(self.inventory.dynamic_ty())
                        })
                    }
                    other => {
                        let _ = self.lower(other);
                        self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                            self.path.clone(),
                            format!("field assignment target `{other}` is not a local receiver"),
                        ));
                        self.frame.temp(self.inventory.dynamic_ty())
                    }
                };
                let value = self.lower(expr);
                let field = self.inventory.intern_string(field);
                self.inventory
                    .push_instruction(AwbcInstruction::AssignField {
                        target,
                        field,
                        value,
                    });
                self.lower(body)
            }
            RuntimeExpr::Call { callee, args } => self.lower_call(callee, args),
            RuntimeExpr::Function { params, body } => self.lower_function(params, body),
            RuntimeExpr::Apply { callee, args } => {
                let callee = self.lower(callee);
                let args = args.iter().map(|arg| self.lower(arg)).collect::<Vec<_>>();
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                self.inventory
                    .push_instruction(AwbcInstruction::ApplyFunction { dst, callee, args });
                dst
            }
            RuntimeExpr::TraitCall {
                callable,
                receiver,
                receiver_mode,
                args,
            } => {
                let receiver_register = self.lower(receiver);
                let args = args.iter().map(|arg| self.lower(arg)).collect();
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                let receiver_out = (*receiver_mode == RuntimeReceiverMode::MutRef).then(|| {
                    if matches!(receiver.as_ref(), RuntimeExpr::Local(_)) {
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
            RuntimeExpr::PureCall { helper, args } => {
                let args = args.iter().map(|arg| self.lower(arg)).collect();
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                self.inventory
                    .push_instruction(AwbcInstruction::CallPureHelper {
                        dst,
                        helper: AwbcPureHelperId(table_index(helper.0)),
                        args,
                    });
                dst
            }
            RuntimeExpr::SpreadArg(expr) => self.lower(expr),
            RuntimeExpr::MethodCall {
                receiver,
                method,
                args,
            } => {
                let _ = self.lower(receiver);
                for arg in args {
                    let _ = self.lower(arg);
                }
                let target = RuntimeCallTarget::callable(
                    arcweft_core::plan::RuntimeCallableId::try_new(method.clone())
                        .expect("RuntimeExpr method call carries an admitted callable identity"),
                );
                self.lower_call(&target, &[]).tap(|_| {
                    self.inventory.diagnostic(AwbcLowerDiagnostic::warning(
                        self.path.clone(),
                        "method-call receiver is lowered as first intrinsic argument; VM host must resolve method dispatch",
                    ));
                })
            }
            RuntimeExpr::Map {
                source,
                param,
                body,
            } => {
                let source = self.lower(source);
                let _ = self.frame.local(
                    param,
                    self.inventory.intern_string(param),
                    self.inventory.dynamic_ty(),
                );
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
            RuntimeExpr::Filter {
                source,
                param,
                body,
            } => {
                let source = self.lower(source);
                let _ = self.frame.local(
                    param,
                    self.inventory.intern_string(param),
                    self.inventory.dynamic_ty(),
                );
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
            RuntimeExpr::Sum { source } => {
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
            RuntimeExpr::Unary { op, expr } => {
                let src = self.lower(expr);
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                self.inventory.push_instruction(AwbcInstruction::Unary {
                    dst,
                    op: unary_op(*op),
                    src,
                });
                dst
            }
            RuntimeExpr::Binary { lhs, op, rhs } => {
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
            RuntimeExpr::If { .. } | RuntimeExpr::IfLet { .. } | RuntimeExpr::Match { .. } => {
                self.lower_value_control_expr(expr)
            }
        }
    }

    fn lower_call(&mut self, callee: &RuntimeCallTarget, args: &[RuntimeExpr]) -> AwbcRegisterId {
        let args = args.iter().map(|arg| self.lower(arg)).collect::<Vec<_>>();
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

    fn lower_function(&mut self, params: &[String], body: &RuntimeExpr) -> AwbcRegisterId {
        let param_names = params.iter().map(String::as_str).collect::<BTreeSet<_>>();
        let free_names = runtime_expr_free_local_names(body);
        let captures = self
            .frame
            .capture_slots()
            .into_iter()
            .filter(|capture| {
                !param_names.contains(capture.name.as_str()) && free_names.contains(&capture.name)
            })
            .collect::<Vec<_>>();
        let function = self.inventory.reserve_function_slot();
        let params = params
            .iter()
            .map(|param| {
                let name = self.inventory.intern_string(param);
                (param.clone(), name)
            })
            .collect::<Vec<_>>();
        self.inventory.push_pending_closure(PendingAwbcClosure {
            function,
            params: params.clone(),
            captures: captures
                .iter()
                .map(|capture| (capture.name.clone(), capture.name_id))
                .collect(),
            body: body.clone(),
            path: format!("{}.closure.{}", self.path, function.0),
        });

        let dst = self.frame.temp(self.inventory.dynamic_ty());
        self.inventory
            .push_instruction(AwbcInstruction::MakeFunction {
                dst,
                function,
                params: params.into_iter().map(|(_, name)| name).collect(),
                capture_names: captures.iter().map(|capture| capture.name_id).collect(),
                captures: captures.iter().map(|capture| capture.register).collect(),
            });
        dst
    }

    fn lower_value_control_expr(&mut self, expr: &RuntimeExpr) -> AwbcRegisterId {
        let free_names = runtime_expr_free_local_names(expr);
        let captures = self
            .frame
            .capture_slots()
            .into_iter()
            .filter(|capture| free_names.contains(&capture.name))
            .collect::<Vec<_>>();
        let function = self.inventory.reserve_function_slot();
        self.inventory.push_pending_closure(PendingAwbcClosure {
            function,
            params: Vec::new(),
            captures: captures
                .iter()
                .map(|capture| (capture.name.clone(), capture.name_id))
                .collect(),
            body: expr.clone(),
            path: format!("{}.control.{}", self.path, function.0),
        });

        let callee = self.frame.temp(self.inventory.dynamic_ty());
        self.inventory
            .push_instruction(AwbcInstruction::MakeFunction {
                dst: callee,
                function,
                params: Vec::new(),
                capture_names: captures.iter().map(|capture| capture.name_id).collect(),
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

pub(crate) fn lower_pending_closures(inventory: &mut AwbcInventory) {
    while let Some(closure) = inventory.pop_pending_closure() {
        let dynamic_ty = inventory.dynamic_ty();
        let mut frame = FrameBuilder::new();
        for (name, name_id) in &closure.captures {
            frame.parameter(name, *name_id, dynamic_ty);
        }
        for (name, name_id) in &closure.params {
            frame.parameter(name, *name_id, dynamic_ty);
        }

        let mut body = ExprBodyBuilder::new(inventory, closure.function);
        lower_closure_body(
            inventory,
            &mut frame,
            &mut body,
            &closure.body,
            &closure.path,
        );
        let layout =
            inventory.intern_frame_layout(format!("{}:frame", closure.path), frame.finish());
        let block = body.block_start;
        let block_len = table_range_len(block.0, inventory.program.blocks.len());
        let signature = inventory.intern_signature(
            vec![dynamic_ty; closure.captures.len().saturating_add(closure.params.len())],
            Some(dynamic_ty),
            AwbcEffectSetId(0),
        );
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
    body: &mut ExprBodyBuilder,
    expr: &RuntimeExpr,
    path: &str,
) {
    match expr {
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => lower_if_value_expr(
            inventory, frame, body, condition, then_expr, else_expr, path,
        ),
        RuntimeExpr::IfLet {
            pattern,
            expr,
            guard,
            then_expr,
            else_expr,
        } => lower_if_let_value_expr(
            inventory,
            frame,
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
        RuntimeExpr::Match { scrutinee, arms } => {
            lower_match_value_expr(inventory, frame, body, scrutinee, arms, path);
        }
        other => terminate_return_expr(inventory, frame, body, other, path, None),
    }
}

fn lower_if_value_expr(
    inventory: &mut AwbcInventory,
    frame: &mut FrameBuilder,
    body: &mut ExprBodyBuilder,
    condition: &RuntimeExpr,
    then_expr: &RuntimeExpr,
    else_expr: &RuntimeExpr,
    path: &str,
) {
    let condition = AwbcExprLowerer::new(inventory, frame, path).lower(condition);
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
        body,
        then_expr,
        &format!("{path}.then"),
        None,
    );
    let else_block = body.reopen_after_terminated_branch(inventory);
    patch_branch_else_block(inventory, branch_block, else_block);
    terminate_return_expr(
        inventory,
        frame,
        body,
        else_expr,
        &format!("{path}.else"),
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
    body: &mut ExprBodyBuilder,
    input: IfLetValueExprInput<'_>,
) {
    let value = AwbcExprLowerer::new(inventory, frame, input.path).lower(input.expr);
    let pattern = lower_branch_pattern(inventory, frame, input.pattern);
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
        let guard =
            AwbcExprLowerer::new(inventory, frame, format!("{}.guard", input.path)).lower(guard);
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
    body: &mut ExprBodyBuilder,
    scrutinee: &RuntimeExpr,
    arms: &[RuntimeExprMatchArm],
    path: &str,
) {
    let scrutinee = AwbcExprLowerer::new(inventory, frame, path).lower(scrutinee);
    for (index, arm) in arms.iter().enumerate() {
        let pattern = lower_branch_pattern(inventory, frame, &arm.pattern);
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

        if let Some(guard) = arm.guard.as_ref() {
            let scope = enter_pattern_scope(inventory, frame, pattern, scrutinee);
            let guard = AwbcExprLowerer::new(inventory, frame, format!("{path}.arm.{index}.guard"))
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
                body,
                &arm.value,
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
                body,
                &arm.value,
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
    body: &mut ExprBodyBuilder,
    expr: &RuntimeExpr,
    path: &str,
    exit_scope: Option<AwbcScopeId>,
) {
    let mut value = AwbcExprLowerer::new(inventory, frame, path).lower(expr);
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
    frame: &mut FrameBuilder,
    pattern: &RuntimePattern,
) -> AwbcPatternId {
    let restored_scope_depth = frame.scope_depth();
    let _ = frame.enter_scope();
    let pattern = lower_pattern(inventory, frame, pattern);
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

fn runtime_expr_free_local_names(expr: &RuntimeExpr) -> BTreeSet<String> {
    let mut collector = RuntimeExprFreeLocalCollector::default();
    collector.collect_expr(expr);
    collector.names
}

#[derive(Default)]
struct RuntimeExprFreeLocalCollector {
    declared: BTreeSet<String>,
    names: BTreeSet<String>,
}

impl RuntimeExprFreeLocalCollector {
    #[allow(
        clippy::too_many_lines,
        reason = "RuntimeExpr free-local collection mirrors the enum so closure capture stays precise."
    )]
    fn collect_expr(&mut self, expr: &RuntimeExpr) {
        match expr {
            RuntimeExpr::Local(name) => {
                if !self.declared.contains(name) {
                    self.names.insert(name.clone());
                }
            }
            RuntimeExpr::Let { name, expr, body } => {
                self.collect_expr(expr);
                self.collect_with_declared(std::slice::from_ref(name), |this| {
                    this.collect_expr(body);
                });
            }
            RuntimeExpr::AssignField {
                target, expr, body, ..
            } => {
                self.collect_expr(target);
                self.collect_expr(expr);
                self.collect_expr(body);
            }
            RuntimeExpr::Tuple(items) | RuntimeExpr::BracketSeq(items) => {
                for item in items {
                    self.collect_expr(item);
                }
            }
            RuntimeExpr::RepeatSeq { value, .. } => self.collect_expr(value),
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
            RuntimeExpr::Field { target, .. }
            | RuntimeExpr::ProjectTuple { target, .. }
            | RuntimeExpr::ProjectRecord { target, .. }
            | RuntimeExpr::SpreadArg(target)
            | RuntimeExpr::Sum { source: target }
            | RuntimeExpr::Unary { expr: target, .. } => self.collect_expr(target),
            RuntimeExpr::Call { args, .. } | RuntimeExpr::PureCall { args, .. } => {
                self.collect_exprs(args);
            }
            RuntimeExpr::Function { params, body } => {
                self.collect_with_declared(params, |this| this.collect_expr(body));
            }
            RuntimeExpr::Apply { callee, args } => {
                self.collect_expr(callee);
                self.collect_exprs(args);
            }
            RuntimeExpr::MethodCall { receiver, args, .. }
            | RuntimeExpr::TraitCall { receiver, args, .. } => {
                self.collect_expr(receiver);
                self.collect_exprs(args);
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
                let names = pattern_binding_names(pattern);
                self.collect_with_declared(&names, |this| {
                    this.collect_optional_expr(guard.as_deref());
                    this.collect_expr(then_expr);
                });
                self.collect_expr(else_expr);
            }
            RuntimeExpr::Match { scrutinee, arms } => {
                self.collect_expr(scrutinee);
                for arm in arms {
                    let names = pattern_binding_names(&arm.pattern);
                    self.collect_with_declared(&names, |this| {
                        this.collect_optional_expr(arm.guard.as_ref());
                        this.collect_expr(&arm.value);
                    });
                }
            }
            RuntimeExpr::Value(_) | RuntimeExpr::EntityRef(_) => {}
        }
    }

    fn collect_exprs(&mut self, exprs: &[RuntimeExpr]) {
        for expr in exprs {
            self.collect_expr(expr);
        }
    }

    fn collect_optional_expr(&mut self, expr: Option<&RuntimeExpr>) {
        if let Some(expr) = expr {
            self.collect_expr(expr);
        }
    }

    fn collect_with_declared(&mut self, names: &[String], f: impl FnOnce(&mut Self)) {
        let declared = self.declared.clone();
        self.declared.extend(names.iter().cloned());
        f(self);
        self.declared = declared;
    }
}

trait Tap: Sized {
    fn tap(self, f: impl FnOnce(&Self)) -> Self {
        f(&self);
        self
    }
}
impl<T> Tap for T {}

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
