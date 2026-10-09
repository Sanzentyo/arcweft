//! AOT physical control IR lowered directly from the admitted executable body.
//! The original FunctionSite remains the only signature/body authority.

use super::*;
use crate::pattern::RuntimePatternKind;
use crate::plan::{FlowOp, RuntimeExecutableBody};
use crate::pure::{
    RuntimePureControlBindings, RuntimePureFunctionBodyRef, RuntimePureFunctionRef,
    RuntimePureUnitDestination, RuntimePureUnitSource, RuntimePureUnitValue,
};
use crate::scope::{RuntimeScopeExitTarget, RuntimeScopeFrameKind};

#[derive(Clone, Debug, PartialEq)]
pub(in crate::pure) enum AotControlOp<E, B> {
    Store {
        slot: usize,
        value: E,
    },
    Evaluate(E),
    If {
        condition: B,
        then_ops: Box<[Self]>,
        else_ops: Box<[Self]>,
    },
    Scope {
        identity: RuntimeScopeIdentity,
        kind: RuntimeScopeFrameKind,
        ops: Box<[Self]>,
    },
    Return(E),
    EnterScope {
        identity: RuntimeScopeIdentity,
        kind: RuntimeScopeFrameKind,
    },
    ExitScope,
    Unit {
        source: RuntimePureUnitSource,
        destination: RuntimePureUnitDestination,
    },
    ExitScopeUnit {
        source: RuntimePureUnitSource,
        destination: RuntimePureUnitDestination,
    },
    ExitScopeStore {
        slot: usize,
        value: E,
    },
    ExitScopeEvaluate(E),
}

trait CompileBody {
    type Expr;
    type Bool;
    fn expression(
        name: &str,
        value: &RuntimeExpr,
        context: &mut AotCompileContext,
    ) -> Result<Self::Expr, RuntimeEvalError>;
    fn condition(
        name: &str,
        value: &RuntimeExpr,
        context: &mut AotCompileContext,
    ) -> Result<Self::Bool, RuntimeEvalError>;
}
struct I64;
struct Scalar;
impl CompileBody for I64 {
    type Expr = AotI64Expr;
    type Bool = AotBoolExpr;
    fn expression(
        name: &str,
        value: &RuntimeExpr,
        context: &mut AotCompileContext,
    ) -> Result<Self::Expr, RuntimeEvalError> {
        compile_aot_i64_expr(name, value, context)
    }
    fn condition(
        name: &str,
        value: &RuntimeExpr,
        context: &mut AotCompileContext,
    ) -> Result<Self::Bool, RuntimeEvalError> {
        compile_aot_bool_expr(name, value, context)
    }
}
impl CompileBody for Scalar {
    type Expr = AotScalarExpr;
    type Bool = AotScalarBoolExpr;
    fn expression(
        name: &str,
        value: &RuntimeExpr,
        context: &mut AotCompileContext,
    ) -> Result<Self::Expr, RuntimeEvalError> {
        compile_aot_scalar_expr(name, value, context)
    }
    fn condition(
        name: &str,
        value: &RuntimeExpr,
        context: &mut AotCompileContext,
    ) -> Result<Self::Bool, RuntimeEvalError> {
        compile_aot_scalar_bool_expr(name, value, context)
    }
}

pub(super) fn compile_i64(
    function: RuntimePureFunctionRef<'_>,
    context: &mut AotCompileContext,
) -> Result<AotI64Expr, RuntimeEvalError> {
    let name = function.name;
    match function.body {
        RuntimePureFunctionBodyRef::Expression(value) => compile_aot_i64_expr(name, value, context),
        RuntimePureFunctionBodyRef::Executable(value) => Ok(AotI64Expr::Executable(
            compile_body::<I64>(function, value, context)?,
        )),
    }
}
pub(super) fn compile_scalar(
    function: RuntimePureFunctionRef<'_>,
    context: &mut AotCompileContext,
) -> Result<AotScalarExpr, RuntimeEvalError> {
    let name = function.name;
    match function.body {
        RuntimePureFunctionBodyRef::Expression(value) => {
            compile_aot_scalar_expr(name, value, context)
        }
        RuntimePureFunctionBodyRef::Executable(value) => Ok(AotScalarExpr::Executable(
            compile_body::<Scalar>(function, value, context)?,
        )),
    }
}

fn compile_body<C: CompileBody>(
    function: RuntimePureFunctionRef<'_>,
    body: &RuntimeExecutableBody,
    context: &mut AotCompileContext,
) -> Result<Box<[AotControlOp<C::Expr, C::Bool>]>, RuntimeEvalError> {
    let name = function.name;
    if !body.is_effect_free() {
        return Err(unsupported_aot(
            name,
            "executable scalar body has admitted execution effects",
        ));
    }
    admit_ops(name, body.ops(), 0)?;
    let abi_slots = context.slots.clone();
    context.read_policy = AotLocalReadPolicy::ExecutableProjection;
    let mut control = RuntimePureControlBindings::new(function, context.slots.clone());
    let (ops, falls_through) = compile_ops::<C>(name, body.ops(), context, &mut control, 0)?;
    if falls_through {
        return Err(unsupported_aot(
            name,
            "executable scalar body has a path without a value return",
        ));
    }
    context.slots = abi_slots;
    context.read_policy = AotLocalReadPolicy::CopyExpression;
    Ok(ops.into_boxed_slice())
}

/// Every closed FlowOp is supported or explicitly declined. Unreachable
/// operation kinds are checked; reachable expressions retain physical admission.
fn admit_ops(name: &str, ops: &[FlowOp], depth: usize) -> Result<(), RuntimeEvalError> {
    if depth > 128 {
        return Err(unsupported_aot(
            name,
            "executable scalar nesting exceeds the codegen budget",
        ));
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
                admit_ops(name, then_ops, depth + 1)?;
                admit_ops(name, else_ops, depth + 1)?;
                None
            }
            FlowOp::Scope { body, .. } => {
                admit_ops(name, body, depth + 1)?;
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
            return Err(unsupported_aot(
                name,
                format!("executable scalar body does not support {operation}"),
            ));
        }
    }
    // A return unwinds reached scopes. Compilation below requires every scope
    // without an explicit exit to return instead of falling through.
    Ok(())
}

fn compile_ops<C: CompileBody>(
    name: &str,
    ops: &[FlowOp],
    context: &mut AotCompileContext,
    control: &mut RuntimePureControlBindings<'_, BTreeMap<RuntimeLocalDeclarationId, usize>>,
    depth: usize,
) -> Result<(Vec<AotControlOp<C::Expr, C::Bool>>, bool), RuntimeEvalError> {
    if depth > 128 {
        return Err(unsupported_aot(
            name,
            "executable scalar nesting exceeds the codegen budget",
        ));
    }
    let mut compiled = Vec::new();
    let mut falls_through = true;
    for op in ops {
        if !falls_through {
            break;
        }
        context.slots = control.numeric().clone();
        match op {
            FlowOp::Let { pattern, expr } => {
                if let Some(source) = control.unit_source(expr)? {
                    let value = control.read_unit(source)?;
                    let destination = control.bind_unit(pattern, value)?;
                    compiled.push(AotControlOp::Unit {
                        source,
                        destination,
                    });
                } else {
                    let value = C::expression(name, expr, context)?;
                    control.consume_numeric_expression(expr)?;
                    match pattern.kind() {
                        RuntimePatternKind::Bind { binding, .. }
                        | RuntimePatternKind::Typed { binding } => {
                            let slot = context.next_slot;
                            context.next_slot += 1;
                            control.numeric_mut().insert(binding.local(), slot);
                            compiled.push(AotControlOp::Store { slot, value });
                        }
                        RuntimePatternKind::Discard => compiled.push(AotControlOp::Evaluate(value)),
                        _ => {
                            return Err(unsupported_aot(name, "executable scalar binding pattern"));
                        }
                    }
                }
            }
            FlowOp::EnterScope { identity } => {
                control.enter_scope(RuntimeScopeFrameKind::EmittedLexical);
                compiled.push(AotControlOp::EnterScope {
                    identity: identity.clone(),
                    kind: RuntimeScopeFrameKind::EmittedLexical,
                });
            }
            FlowOp::ExitScope => {
                control.exit_scope(RuntimeScopeExitTarget::EmittedLexical)?;
                compiled.push(AotControlOp::ExitScope);
            }
            FlowOp::ExitScopeBind { pattern, expr } => {
                if let Some(source) = control.unit_source(expr)? {
                    let value = control.read_unit(source)?;
                    control.exit_scope(RuntimeScopeExitTarget::EmittedLexical)?;
                    let destination = control.bind_unit(pattern, value)?;
                    compiled.push(AotControlOp::ExitScopeUnit {
                        source,
                        destination,
                    });
                } else {
                    let value = C::expression(name, expr, context)?;
                    control.consume_numeric_expression(expr)?;
                    control.exit_scope(RuntimeScopeExitTarget::EmittedLexical)?;
                    match pattern.kind() {
                        RuntimePatternKind::Bind { binding, .. }
                        | RuntimePatternKind::Typed { binding } => {
                            let slot = context.next_slot;
                            context.next_slot += 1;
                            control.numeric_mut().insert(binding.local(), slot);
                            compiled.push(AotControlOp::ExitScopeStore { slot, value });
                        }
                        RuntimePatternKind::Discard => {
                            compiled.push(AotControlOp::ExitScopeEvaluate(value))
                        }
                        _ => return Err(unsupported_aot(name, "executable scope result pattern")),
                    }
                }
            }
            FlowOp::If {
                condition,
                then_ops,
                else_ops,
            } => {
                let compiled_condition = C::condition(name, condition, context)?;
                control.consume_numeric_expression(condition)?;
                let mut branch = |ops: &[FlowOp]| -> Result<_, RuntimeEvalError> {
                    let mut selected = control.clone();
                    let scope = (!ops.is_empty())
                        .then(|| selected.enter_scope(RuntimeScopeFrameKind::Control));
                    let (mut body, falls) =
                        compile_ops::<C>(name, ops, context, &mut selected, depth + 1)?;
                    if let Some(scope) = scope {
                        if falls && selected.contains_scope(scope) {
                            selected.exit_scope(RuntimeScopeExitTarget::Frame(scope))?;
                        }
                        body = vec![AotControlOp::Scope {
                            identity: RuntimeScopeIdentity::Anonymous,
                            kind: RuntimeScopeFrameKind::Control,
                            ops: body.into_boxed_slice(),
                        }];
                    }
                    Ok((body, falls, selected))
                };
                let left = branch(then_ops)?;
                let right = branch(else_ops)?;
                falls_through = left.1 || right.1;
                if left.1 && right.1 {
                    if !left.2.compatible_after_branch(&right.2)
                        || left.2.numeric() != right.2.numeric()
                    {
                        return Err(unsupported_aot(
                            name,
                            "conditional physical place state does not converge",
                        ));
                    }
                    *control = left.2;
                } else if left.1 {
                    *control = left.2;
                } else if right.1 {
                    *control = right.2;
                }
                compiled.push(AotControlOp::If {
                    condition: compiled_condition,
                    then_ops: left.0.into_boxed_slice(),
                    else_ops: right.0.into_boxed_slice(),
                });
            }
            FlowOp::Scope { identity, body } => {
                let mut selected = control.clone();
                let scope = selected.enter_scope(RuntimeScopeFrameKind::EmittedLexical);
                let (body, falls) =
                    compile_ops::<C>(name, body, context, &mut selected, depth + 1)?;
                if falls && selected.contains_scope(scope) {
                    selected.exit_scope(RuntimeScopeExitTarget::Frame(scope))?;
                }
                if falls {
                    *control = selected;
                }
                compiled.push(AotControlOp::Scope {
                    identity: identity.clone(),
                    kind: RuntimeScopeFrameKind::EmittedLexical,
                    ops: body.into_boxed_slice(),
                });
                falls_through = falls;
            }
            FlowOp::ReturnExpr(value) => {
                let compiled_value = C::expression(name, value, context)?;
                control.consume_numeric_expression(value)?;
                compiled.push(AotControlOp::Return(compiled_value));
                falls_through = false;
            }
            FlowOp::Noop => {}
            _ => {
                return Err(unsupported_aot(
                    name,
                    "operation outside the admitted scalar control subset",
                ));
            }
        }
    }
    context.slots = control.numeric().clone();
    Ok((compiled, falls_through))
}

trait EvaluateBody {
    type Expr;
    type Bool;
    type Value: Copy;
    fn expression(&mut self, value: &Self::Expr) -> Result<Self::Value, RuntimeEvalError>;
    fn condition(&mut self, value: &Self::Bool) -> Result<bool, RuntimeEvalError>;
    fn store(&mut self, slot: usize, value: Self::Value);
    fn enter_control_scope(
        &mut self,
        identity: RuntimeScopeIdentity,
        kind: RuntimeScopeFrameKind,
    ) -> usize;
    fn exit_control_scope(
        &mut self,
        target: RuntimeScopeExitTarget<usize>,
    ) -> Result<(), RuntimeEvalError>;
    fn has_control_scope(&self, id: usize) -> bool;
    fn finish_control_return(&mut self);
    fn read_unit(
        &mut self,
        source: RuntimePureUnitSource,
    ) -> Result<RuntimePureUnitValue, RuntimeEvalError>;
    fn publish_unit(
        &mut self,
        destination: RuntimePureUnitDestination,
        value: RuntimePureUnitValue,
    ) -> Result<(), RuntimeEvalError>;
}
impl EvaluateBody for AotI64Evaluator<'_> {
    type Expr = AotI64Expr;
    type Bool = AotBoolExpr;
    type Value = i64;
    fn expression(&mut self, value: &Self::Expr) -> Result<Self::Value, RuntimeEvalError> {
        self.eval_i64(value)
    }
    fn condition(&mut self, value: &Self::Bool) -> Result<bool, RuntimeEvalError> {
        self.eval_bool(value)
    }
    fn store(&mut self, slot: usize, value: Self::Value) {
        self.slots[slot] = value;
    }
    fn enter_control_scope(
        &mut self,
        identity: RuntimeScopeIdentity,
        kind: RuntimeScopeFrameKind,
    ) -> usize {
        self.scope_stack.push(identity);
        self.control.enter_scope(kind)
    }
    fn exit_control_scope(
        &mut self,
        target: RuntimeScopeExitTarget<usize>,
    ) -> Result<(), RuntimeEvalError> {
        let before = self.control.scope_count();
        self.control.exit_scope(target)?;
        for _ in self.control.scope_count()..before {
            self.scope_stack.pop();
        }
        Ok(())
    }
    fn has_control_scope(&self, id: usize) -> bool {
        self.control.contains_scope(id)
    }
    fn finish_control_return(&mut self) {
        let active = self.control.scope_count();
        self.control.finish_return();
        for _ in 0..active {
            self.scope_stack.pop();
        }
    }
    fn read_unit(
        &mut self,
        source: RuntimePureUnitSource,
    ) -> Result<RuntimePureUnitValue, RuntimeEvalError> {
        self.stats.evaluated_exprs += 1;
        self.control.read_unit(source)
    }
    fn publish_unit(
        &mut self,
        destination: RuntimePureUnitDestination,
        value: RuntimePureUnitValue,
    ) -> Result<(), RuntimeEvalError> {
        self.control.publish_unit(destination, value)
    }
}
impl EvaluateBody for AotScalarEvaluator<'_> {
    type Expr = AotScalarExpr;
    type Bool = AotScalarBoolExpr;
    type Value = RuntimePureScalar;
    fn expression(&mut self, value: &Self::Expr) -> Result<Self::Value, RuntimeEvalError> {
        self.eval_scalar(value)
    }
    fn condition(&mut self, value: &Self::Bool) -> Result<bool, RuntimeEvalError> {
        self.eval_bool(value)
    }
    fn store(&mut self, slot: usize, value: Self::Value) {
        self.slots[slot] = value;
    }
    fn enter_control_scope(
        &mut self,
        identity: RuntimeScopeIdentity,
        kind: RuntimeScopeFrameKind,
    ) -> usize {
        self.scope_stack.push(identity);
        self.control.enter_scope(kind)
    }
    fn exit_control_scope(
        &mut self,
        target: RuntimeScopeExitTarget<usize>,
    ) -> Result<(), RuntimeEvalError> {
        let before = self.control.scope_count();
        self.control.exit_scope(target)?;
        for _ in self.control.scope_count()..before {
            self.scope_stack.pop();
        }
        Ok(())
    }
    fn has_control_scope(&self, id: usize) -> bool {
        self.control.contains_scope(id)
    }
    fn finish_control_return(&mut self) {
        let active = self.control.scope_count();
        self.control.finish_return();
        for _ in 0..active {
            self.scope_stack.pop();
        }
    }
    fn read_unit(
        &mut self,
        source: RuntimePureUnitSource,
    ) -> Result<RuntimePureUnitValue, RuntimeEvalError> {
        self.stats.evaluated_exprs += 1;
        self.control.read_unit(source)
    }
    fn publish_unit(
        &mut self,
        destination: RuntimePureUnitDestination,
        value: RuntimePureUnitValue,
    ) -> Result<(), RuntimeEvalError> {
        self.control.publish_unit(destination, value)
    }
}
fn evaluate_ops<V: EvaluateBody>(
    evaluator: &mut V,
    ops: &[AotControlOp<V::Expr, V::Bool>],
) -> Result<Option<V::Value>, RuntimeEvalError> {
    for op in ops {
        let value = match op {
            AotControlOp::Store { slot, value } => {
                let value = evaluator.expression(value)?;
                evaluator.store(*slot, value);
                None
            }
            AotControlOp::Evaluate(value) => {
                evaluator.expression(value)?;
                None
            }
            AotControlOp::If {
                condition,
                then_ops,
                else_ops,
            } => {
                let condition = evaluator.condition(condition)?;
                evaluate_ops(evaluator, if condition { then_ops } else { else_ops })?
            }
            AotControlOp::Scope {
                identity,
                kind,
                ops,
            } => {
                let scope = evaluator.enter_control_scope(identity.clone(), *kind);
                let value = evaluate_ops(evaluator, ops);
                if evaluator.has_control_scope(scope) {
                    evaluator.exit_control_scope(RuntimeScopeExitTarget::Frame(scope))?;
                }
                value?
            }
            AotControlOp::EnterScope { identity, kind } => {
                evaluator.enter_control_scope(identity.clone(), *kind);
                None
            }
            AotControlOp::ExitScope => {
                evaluator.exit_control_scope(RuntimeScopeExitTarget::EmittedLexical)?;
                None
            }
            AotControlOp::Unit {
                source,
                destination,
            } => {
                let value = evaluator.read_unit(*source)?;
                evaluator.publish_unit(*destination, value)?;
                None
            }
            AotControlOp::ExitScopeUnit {
                source,
                destination,
            } => {
                let value = evaluator.read_unit(*source)?;
                evaluator.exit_control_scope(RuntimeScopeExitTarget::EmittedLexical)?;
                evaluator.publish_unit(*destination, value)?;
                None
            }
            AotControlOp::ExitScopeStore { slot, value } => {
                let value = evaluator.expression(value)?;
                evaluator.exit_control_scope(RuntimeScopeExitTarget::EmittedLexical)?;
                evaluator.store(*slot, value);
                None
            }
            AotControlOp::ExitScopeEvaluate(value) => {
                evaluator.expression(value)?;
                evaluator.exit_control_scope(RuntimeScopeExitTarget::EmittedLexical)?;
                None
            }
            AotControlOp::Return(value) => {
                let value = evaluator.expression(value)?;
                evaluator.finish_control_return();
                Some(value)
            }
        };
        if value.is_some() {
            return Ok(value);
        }
    }
    Ok(None)
}
pub(super) fn evaluate_i64(
    evaluator: &mut AotI64Evaluator<'_>,
    ops: &[AotControlOp<AotI64Expr, AotBoolExpr>],
) -> Result<i64, RuntimeEvalError> {
    evaluate_ops(evaluator, ops)?.ok_or_else(|| {
        unsupported_aot(
            "structured.function",
            "admitted compiled body did not return",
        )
    })
}
pub(super) fn evaluate_scalar(
    evaluator: &mut AotScalarEvaluator<'_>,
    ops: &[AotControlOp<AotScalarExpr, AotScalarBoolExpr>],
) -> Result<RuntimePureScalar, RuntimeEvalError> {
    evaluate_ops(evaluator, ops)?
        .ok_or_else(|| unsupported_aot(evaluator.name, "admitted compiled body did not return"))
}
