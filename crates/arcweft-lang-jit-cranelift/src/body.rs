//! Native scalar bodies borrow checked FunctionSite control operations.
//! Return values merge at a body-local block, including inside batch loops.

use super::*;
use arcweft_core::pattern::RuntimePatternKind;
use arcweft_core::plan::FlowOp;
use arcweft_core::pure::{
    RuntimePureControlBindings, RuntimePureFunctionBodyRef, RuntimePureFunctionRef,
};
use arcweft_core::scope::{RuntimeScopeExitTarget, RuntimeScopeFrameKind};
use cranelift::codegen::ir::Block;

trait ScalarLowering {
    type Binding: Copy + PartialEq;
    fn ty(&self) -> Type;
    fn binding(&self, value: Value) -> Self::Binding;
    fn expression(
        &self,
        builder: &mut FunctionBuilder<'_>,
        bindings: &BTreeMap<RuntimeLocalDeclarationId, Self::Binding>,
        expr: &RuntimeExpr,
        stats: &mut PureFunctionStats,
    ) -> Result<Value, CraneliftCodegenError>;
    fn condition(
        &self,
        builder: &mut FunctionBuilder<'_>,
        bindings: &BTreeMap<RuntimeLocalDeclarationId, Self::Binding>,
        expr: &RuntimeExpr,
        stats: &mut PureFunctionStats,
    ) -> Result<Value, CraneliftCodegenError>;
}
macro_rules! scalar_lowering {
    ($name:ident, $binding:ident, $expr:ident, $condition:ident, $ty:ident) => {
        struct $name;
        impl ScalarLowering for $name {
            type Binding = $binding;
            fn ty(&self) -> Type {
                types::$ty
            }
            fn binding(&self, value: Value) -> Self::Binding {
                $binding::Value(value)
            }
            fn expression(
                &self,
                builder: &mut FunctionBuilder<'_>,
                bindings: &BTreeMap<RuntimeLocalDeclarationId, Self::Binding>,
                expr: &RuntimeExpr,
                stats: &mut PureFunctionStats,
            ) -> Result<Value, CraneliftCodegenError> {
                lower::$expr(builder, bindings, expr, stats)
            }
            fn condition(
                &self,
                builder: &mut FunctionBuilder<'_>,
                bindings: &BTreeMap<RuntimeLocalDeclarationId, Self::Binding>,
                expr: &RuntimeExpr,
                stats: &mut PureFunctionStats,
            ) -> Result<Value, CraneliftCodegenError> {
                lower::$condition(builder, bindings, expr, stats)
            }
        }
    };
}
scalar_lowering!(I64, LoweredIntBinding, lower_expr, lower_condition, I64);
scalar_lowering!(
    I32,
    LoweredIntBinding,
    lower_i32_expr,
    lower_i32_condition,
    I32
);
scalar_lowering!(
    U32,
    LoweredIntBinding,
    lower_u32_expr,
    lower_u32_condition,
    I32
);
scalar_lowering!(
    U64,
    LoweredIntBinding,
    lower_u64_expr,
    lower_u64_condition,
    I64
);
scalar_lowering!(
    F32,
    LoweredF32Binding,
    lower_f32_expr,
    lower_f32_condition,
    F32
);
scalar_lowering!(
    F64,
    LoweredF64Binding,
    lower_f64_expr,
    lower_f64_condition,
    F64
);
struct Small(SmallIntKind);
impl ScalarLowering for Small {
    type Binding = LoweredSmallIntBinding;
    fn ty(&self) -> Type {
        self.0.cranelift_type()
    }
    fn binding(&self, value: Value) -> Self::Binding {
        LoweredSmallIntBinding::Value(value)
    }
    fn expression(
        &self,
        builder: &mut FunctionBuilder<'_>,
        bindings: &BTreeMap<RuntimeLocalDeclarationId, Self::Binding>,
        expr: &RuntimeExpr,
        stats: &mut PureFunctionStats,
    ) -> Result<Value, CraneliftCodegenError> {
        lower::lower_small_int_expr(builder, bindings, expr, stats, self.0)
    }
    fn condition(
        &self,
        builder: &mut FunctionBuilder<'_>,
        bindings: &BTreeMap<RuntimeLocalDeclarationId, Self::Binding>,
        expr: &RuntimeExpr,
        stats: &mut PureFunctionStats,
    ) -> Result<Value, CraneliftCodegenError> {
        lower::lower_small_int_condition(builder, bindings, expr, stats, self.0)
    }
}

fn unsupported(operation: &str) -> CraneliftCodegenError {
    CraneliftCodegenError::UnsupportedExpr(format!(
        "executable scalar body does not support {operation}"
    ))
}
fn admit_ops(ops: &[FlowOp], depth: usize) -> Result<(), CraneliftCodegenError> {
    if depth > 128 {
        return Err(unsupported("nesting beyond the codegen budget"));
    }
    for op in ops {
        let declined = match op {
            FlowOp::Let { pattern, .. } => {
                if !matches!(
                    pattern.kind(),
                    RuntimePatternKind::Bind { .. }
                        | RuntimePatternKind::Typed { .. }
                        | RuntimePatternKind::Discard
                ) {
                    Some("binding pattern")
                } else {
                    None
                }
            }
            FlowOp::If {
                then_ops, else_ops, ..
            } => {
                admit_ops(then_ops, depth + 1)?;
                admit_ops(else_ops, depth + 1)?;
                None
            }
            FlowOp::Scope { body, .. } => {
                admit_ops(body, depth + 1)?;
                None
            }
            // Reached scope targets are checked by the shared Core lexical
            // projection, including exits nested under generated branches.
            FlowOp::EnterScope { .. } | FlowOp::ExitScope => None,
            FlowOp::ReturnExpr(_) | FlowOp::Noop => None,
            FlowOp::Bind(_) => Some("prebound values"),
            FlowOp::FormatOperandAttempt { .. } | FlowOp::CompleteFormatOperand { .. } => {
                Some("format attempt")
            }
            FlowOp::LetElse { .. } => Some("refutable binding"),
            FlowOp::Assign { .. } => Some("place assignment"),
            FlowOp::LineOperation { .. }
            | FlowOp::CommitDialogueResult { .. }
            | FlowOp::SelectDialogueResult { .. }
            | FlowOp::Dialogue { .. } => Some("line operation"),
            FlowOp::Choice { .. } => Some("choice"),
            FlowOp::Await { .. }
            | FlowOp::StartNeedProducer { .. }
            | FlowOp::AwaitMany { .. }
            | FlowOp::CompleteAwaitObserver => Some("suspension"),
            FlowOp::HostCall { .. } => Some("host call"),
            FlowOp::ProjectCall { .. } | FlowOp::ApplyGroup { .. } => {
                Some("nested function control transfer")
            }
            FlowOp::IfLet { .. } | FlowOp::Match { .. } => Some("pattern control flow"),
            FlowOp::EnterScheduledScope { .. } | FlowOp::ExitScheduledScope { .. } => {
                Some("native scheduler scope marker")
            }
            FlowOp::Loop { .. }
            | FlowOp::LoopNext { .. }
            | FlowOp::While { .. }
            | FlowOp::WhileNext { .. }
            | FlowOp::WhileLet { .. }
            | FlowOp::WhileLetNext { .. }
            | FlowOp::For { .. }
            | FlowOp::ForNext { .. }
            | FlowOp::Break(_)
            | FlowOp::Continue => Some("loop control"),
            FlowOp::Thread { .. } => Some("thread"),
            FlowOp::LetScope { .. } => Some("scope value binding"),
            FlowOp::ExitScopeBind { pattern, .. } => {
                if matches!(
                    pattern.kind(),
                    RuntimePatternKind::Bind { .. }
                        | RuntimePatternKind::Typed { .. }
                        | RuntimePatternKind::Discard
                ) {
                    None
                } else {
                    Some("scope result pattern")
                }
            }
            FlowOp::Goto(_) | FlowOp::GotoExpr(_) => Some("flow transfer"),
            FlowOp::Return(_) => Some("flow label return"),
            FlowOp::Effect(_)
            | FlowOp::EvaluatedEffect(_)
            | FlowOp::RegisterDefer { .. }
            | FlowOp::RegisterCleanup { .. }
            | FlowOp::CancelCleanup { .. } => Some("effect or cleanup"),
        };
        if let Some(operation) = declined {
            return Err(unsupported(operation));
        }
    }
    // A value return unwinds still-entered scopes in the owning Engine. The
    // compiler below requires that every unterminated scope actually returns.
    Ok(())
}
fn projection_error(error: arcweft_core::value::RuntimeEvalError) -> CraneliftCodegenError {
    CraneliftCodegenError::UnsupportedExpr(error.to_string())
}
fn lower_body<L: ScalarLowering>(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<RuntimeLocalDeclarationId, L::Binding>,
    function: RuntimePureFunctionRef<'_>,
    stats: &mut PureFunctionStats,
    lowering: L,
) -> Result<Value, CraneliftCodegenError> {
    let mut control = RuntimePureControlBindings::new(function, bindings.clone());
    match function.body {
        RuntimePureFunctionBodyRef::Expression(expr) => {
            let value = lowering.expression(builder, control.numeric(), expr, stats)?;
            control
                .consume_numeric_expression(expr)
                .map_err(projection_error)?;
            Ok(value)
        }
        RuntimePureFunctionBodyRef::Executable(body) => {
            if !body.is_effect_free() {
                return Err(unsupported("admitted execution effects"));
            }
            admit_ops(body.ops(), 0)?;
            let result = builder.create_block();
            builder.append_block_param(result, lowering.ty());
            if lower_ops(
                builder,
                &mut control,
                body.ops(),
                stats,
                &lowering,
                result,
                0,
            )? {
                return Err(unsupported("a path without a value return"));
            }
            builder.switch_to_block(result);
            Ok(builder.block_params(result)[0])
        }
    }
}

fn lower_ops<L: ScalarLowering>(
    builder: &mut FunctionBuilder<'_>,
    control: &mut RuntimePureControlBindings<'_, BTreeMap<RuntimeLocalDeclarationId, L::Binding>>,
    ops: &[FlowOp],
    stats: &mut PureFunctionStats,
    lowering: &L,
    result: Block,
    depth: usize,
) -> Result<bool, CraneliftCodegenError> {
    if depth > 128 {
        return Err(unsupported("nesting beyond the codegen budget"));
    }
    let mut falls_through = true;
    for op in ops {
        if !falls_through {
            break;
        }
        match op {
            FlowOp::Let { pattern, expr } => {
                if let Some(value) = control.evaluate_unit(expr).map_err(projection_error)? {
                    stats.evaluated_exprs += 1;
                    control
                        .bind_unit(pattern, value)
                        .map_err(projection_error)?;
                } else {
                    let value = lowering.expression(builder, control.numeric(), expr, stats)?;
                    control
                        .consume_numeric_expression(expr)
                        .map_err(projection_error)?;
                    match pattern.kind() {
                        RuntimePatternKind::Bind { binding, .. }
                        | RuntimePatternKind::Typed { binding } => {
                            control
                                .numeric_mut()
                                .insert(binding.local(), lowering.binding(value));
                        }
                        RuntimePatternKind::Discard => {}
                        _ => return Err(unsupported("binding pattern")),
                    }
                }
            }
            FlowOp::EnterScope { .. } => {
                control.enter_scope(RuntimeScopeFrameKind::EmittedLexical);
            }
            FlowOp::ExitScope => control
                .exit_scope(RuntimeScopeExitTarget::EmittedLexical)
                .map_err(projection_error)?,
            FlowOp::ExitScopeBind { pattern, expr } => {
                if let Some(value) = control.evaluate_unit(expr).map_err(projection_error)? {
                    stats.evaluated_exprs += 1;
                    control
                        .exit_scope(RuntimeScopeExitTarget::EmittedLexical)
                        .map_err(projection_error)?;
                    control
                        .bind_unit(pattern, value)
                        .map_err(projection_error)?;
                } else {
                    let value = lowering.expression(builder, control.numeric(), expr, stats)?;
                    control
                        .consume_numeric_expression(expr)
                        .map_err(projection_error)?;
                    control
                        .exit_scope(RuntimeScopeExitTarget::EmittedLexical)
                        .map_err(projection_error)?;
                    match pattern.kind() {
                        RuntimePatternKind::Bind { binding, .. }
                        | RuntimePatternKind::Typed { binding } => {
                            control
                                .numeric_mut()
                                .insert(binding.local(), lowering.binding(value));
                        }
                        RuntimePatternKind::Discard => {}
                        _ => return Err(unsupported("scope result pattern")),
                    }
                }
            }
            FlowOp::If {
                condition,
                then_ops,
                else_ops,
            } => {
                let lowered_condition =
                    lowering.condition(builder, control.numeric(), condition, stats)?;
                control
                    .consume_numeric_expression(condition)
                    .map_err(projection_error)?;
                let then_block = builder.create_block();
                let else_block = builder.create_block();
                let next = builder.create_block();
                builder
                    .ins()
                    .brif(lowered_condition, then_block, &[], else_block, &[]);
                let mut branch =
                    |ops: &[FlowOp], block: Block| -> Result<_, CraneliftCodegenError> {
                        builder.switch_to_block(block);
                        let mut selected = control.clone();
                        let scope = (!ops.is_empty())
                            .then(|| selected.enter_scope(RuntimeScopeFrameKind::Control));
                        let falls = lower_ops(
                            builder,
                            &mut selected,
                            ops,
                            stats,
                            lowering,
                            result,
                            depth + 1,
                        )?;
                        if falls {
                            if let Some(scope) = scope {
                                if selected.contains_scope(scope) {
                                    selected
                                        .exit_scope(RuntimeScopeExitTarget::Frame(scope))
                                        .map_err(projection_error)?;
                                }
                            }
                            builder.ins().jump(next, &[]);
                        }
                        Ok((falls, selected))
                    };
                let left = branch(then_ops, then_block)?;
                let right = branch(else_ops, else_block)?;
                falls_through = left.0 || right.0;
                if left.0 && right.0 {
                    if !left.1.compatible_after_branch(&right.1)
                        || left.1.numeric() != right.1.numeric()
                    {
                        return Err(unsupported(
                            "conditional physical place state does not converge",
                        ));
                    }
                    *control = left.1;
                } else if left.0 {
                    *control = left.1;
                } else if right.0 {
                    *control = right.1;
                }
                if falls_through {
                    builder.switch_to_block(next);
                }
            }
            FlowOp::Scope { body, .. } => {
                let scope = control.enter_scope(RuntimeScopeFrameKind::EmittedLexical);
                falls_through =
                    lower_ops(builder, control, body, stats, lowering, result, depth + 1)?;
                if falls_through && control.contains_scope(scope) {
                    control
                        .exit_scope(RuntimeScopeExitTarget::Frame(scope))
                        .map_err(projection_error)?;
                }
            }
            FlowOp::ReturnExpr(expr) => {
                let value = lowering.expression(builder, control.numeric(), expr, stats)?;
                control
                    .consume_numeric_expression(expr)
                    .map_err(projection_error)?;
                builder.ins().jump(result, &[value.into()]);
                falls_through = false;
            }
            FlowOp::Noop => {}
            _ => {
                return Err(unsupported(
                    "operation outside the admitted scalar control subset",
                ));
            }
        }
    }
    Ok(falls_through)
}

macro_rules! body_entry {
    ($name:ident, $binding:ident, $lowering:ident) => {
        pub(super) fn $name(
            builder: &mut FunctionBuilder<'_>,
            bindings: &BTreeMap<RuntimeLocalDeclarationId, $binding>,
            body: RuntimePureFunctionRef<'_>,
            stats: &mut PureFunctionStats,
        ) -> Result<Value, CraneliftCodegenError> {
            lower_body(builder, bindings, body, stats, $lowering)
        }
    };
}
body_entry!(lower_i64_body, LoweredIntBinding, I64);
body_entry!(lower_i32_body, LoweredIntBinding, I32);
body_entry!(lower_u32_body, LoweredIntBinding, U32);
body_entry!(lower_u64_body, LoweredIntBinding, U64);
body_entry!(lower_f32_body, LoweredF32Binding, F32);
body_entry!(lower_f64_body, LoweredF64Binding, F64);
pub(super) fn lower_small_int_body(
    builder: &mut FunctionBuilder<'_>,
    bindings: &BTreeMap<RuntimeLocalDeclarationId, LoweredSmallIntBinding>,
    body: RuntimePureFunctionRef<'_>,
    stats: &mut PureFunctionStats,
    kind: SmallIntKind,
) -> Result<Value, CraneliftCodegenError> {
    lower_body(builder, bindings, body, stats, Small(kind))
}
