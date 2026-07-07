use crate::awbc_lower::frame::FrameBuilder;
use crate::awbc_lower::inventory::{AwbcInventory, AwbcLowerDiagnostic};
use crate::awbc_lower::pattern::lower_pattern;
use crate::awbc_lower::table_index;
use arcweft_core::awbc::schema::{
    AwbcBinaryOp, AwbcInstruction, AwbcIntrinsic, AwbcIntrinsicId, AwbcPureHelperId,
    AwbcRegisterId, AwbcTraitMethodId, AwbcUnaryOp,
};
use arcweft_core::plan::RuntimeReceiverMode;
use arcweft_core::value::{RuntimeBinaryOp, RuntimeCallTarget, RuntimeExpr, RuntimeUnaryOp};

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
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                self.inventory.push_instruction(AwbcInstruction::MakeTuple {
                    dst,
                    items: registers,
                });
                dst
            }
            RuntimeExpr::BracketSeq(items) => {
                let registers = items.iter().map(|item| self.lower(item)).collect();
                let dst = self.frame.temp(self.inventory.dynamic_ty());
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
                let dst = self.frame.temp(self.inventory.dynamic_ty());
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
            RuntimeExpr::Variant { name, payload, .. } => {
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                let payload = payload.as_deref().map(|payload| self.lower(payload));
                let ty = self.inventory.dynamic_ty();
                let case_name = self.inventory.intern_string(name);
                self.inventory
                    .push_instruction(AwbcInstruction::MakeVariant {
                        dst,
                        ty,
                        case: stable_case(name),
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
                let target = RuntimeCallTarget::Named(method.clone());
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
            RuntimeExpr::If {
                condition,
                then_expr,
                else_expr,
            } => {
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                let condition = self.lower(condition);
                let then_value = self.lower(then_expr);
                let else_value = self.lower(else_expr);
                let intrinsic = self.intern_intrinsic("select.bool", 3);
                self.inventory
                    .push_instruction(AwbcInstruction::CallIntrinsic {
                        dst: Some(dst),
                        intrinsic,
                        args: vec![condition, then_value, else_value],
                    });
                dst
            }
            RuntimeExpr::IfLet {
                pattern,
                expr,
                guard,
                then_expr,
                else_expr,
            } => {
                let value = self.lower(expr);
                let matched = self.frame.temp(self.inventory.bool_ty());
                let pattern = lower_pattern(self.inventory, self.frame, pattern);
                self.inventory
                    .push_instruction(AwbcInstruction::TestPattern {
                        dst: matched,
                        pattern,
                        value,
                    });
                let matched = if let Some(guard) = guard {
                    let guard = self.lower(guard);
                    let both = self.frame.temp(self.inventory.bool_ty());
                    self.inventory.push_instruction(AwbcInstruction::Binary {
                        dst: both,
                        op: AwbcBinaryOp::And,
                        lhs: matched,
                        rhs: guard,
                    });
                    both
                } else {
                    matched
                };
                let then_value = self.lower(then_expr);
                let else_value = self.lower(else_expr);
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                let intrinsic = self.intern_intrinsic("select.bool", 3);
                self.inventory
                    .push_instruction(AwbcInstruction::CallIntrinsic {
                        dst: Some(dst),
                        intrinsic,
                        args: vec![matched, then_value, else_value],
                    });
                dst
            }
            RuntimeExpr::Match { scrutinee, arms } => {
                let scrutinee = self.lower(scrutinee);
                let dst = self.frame.temp(self.inventory.dynamic_ty());
                let mut args = vec![scrutinee];
                for arm in arms {
                    args.push(self.lower(&arm.value));
                    if let Some(guard) = &arm.guard {
                        args.push(self.lower(guard));
                    }
                }
                let intrinsic = self.intern_intrinsic("match.value", args.len());
                self.inventory
                    .push_instruction(AwbcInstruction::CallIntrinsic {
                        dst: Some(dst),
                        intrinsic,
                        args,
                    });
                dst
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
            registry_code: stable_case(label),
            signature,
            revision: 1,
        });
        id
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

fn stable_case(value: &str) -> u32 {
    value.bytes().fold(2_166_136_261_u32, |acc, byte| {
        acc.wrapping_mul(16_777_619) ^ u32::from(byte)
    })
}
