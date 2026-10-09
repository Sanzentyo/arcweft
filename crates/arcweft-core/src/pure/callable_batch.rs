//! Borrowed physical projection of an admitted callable and its actual call.
//! The original FunctionSite and every whole formal remain the authority.
use super::{RuntimePureFunctionRef, RuntimePureScalar, runtime_value_as_scalar};
use crate::pattern::RuntimePatternKind;
use crate::plan::{
    FlowOp, RuntimeCallableAttachedContract, RuntimeCallableInputSource,
    RuntimeCallableParameterKind, RuntimeCallableTransition, RuntimeFunctionInputSource,
    RuntimeFunctionSiteBody, RuntimePlan, RuntimeProjectCallOrdinaryMaterialization,
};
use crate::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId, RuntimeProjectCallSiteId};
use crate::task::RuntimeProgramOwner;
use crate::value::{
    RuntimeCallArgumentMode, RuntimeCallableValue, RuntimeEvalError, RuntimeExpr, RuntimeExprKind,
};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeNumericMapInput {
    Item,
    Constant(RuntimePureScalar),
}
#[derive(Clone, Copy, Debug)]
struct Input {
    source: RuntimeNumericMapInput,
    ty: RuntimePlanTypeId,
}
pub(crate) struct RuntimeNumericMapProjection<'a> {
    pub function: RuntimePureFunctionRef<'a>,
    pub inputs: Box<[RuntimeNumericMapInput]>,
    /// Control operations executed by the actual forwarding closure. Target
    /// scalar completion is reported separately by its compiled backend.
    pub forwarding_ops: usize,
}
type Inputs = BTreeMap<RuntimeLocalDeclarationId, Input>;

enum ForwardedCall<'a> {
    Project {
        site: RuntimeProjectCallSiteId,
        control_ops: usize,
        inputs: Inputs,
    },
    Expression {
        callee: &'a RuntimeExpr,
        args: &'a [crate::value::RuntimeCallArgument],
    },
}
pub(crate) fn project_numeric_map<'a>(
    plan: &'a Arc<RuntimePlan>,
    mapping: &RuntimeCallableValue,
) -> Result<Option<RuntimeNumericMapProjection<'a>>, RuntimeEvalError> {
    mapping.validate_for_owner(&RuntimeProgramOwner::Plan(Arc::clone(plan)))?;
    let Some(state) = plan.callable_states().get(mapping.state()) else {
        return Ok(None);
    };
    let [parameter] = state.parameters.as_ref() else {
        return Ok(None);
    };
    if !matches!(parameter.kind, RuntimeCallableParameterKind::Fixed)
        || parameter.abi_ty != parameter.binding_ty
        || !matches!(state.attached, RuntimeCallableAttachedContract::None)
    {
        return Ok(None);
    }
    let RuntimeCallableTransition::Invoke {
        function,
        captures,
        arguments,
    } = &state.transition
    else {
        return Ok(None);
    };
    let Some(body) = plan.function_sites().get(*function) else {
        return Ok(None);
    };
    if !body.invocation_effects().is_empty() || !body.body().is_effect_free() {
        return Ok(None);
    }
    let source = |source: RuntimeCallableInputSource| match source {
        RuntimeCallableInputSource::Argument { position: 0 } => Some(Input {
            source: RuntimeNumericMapInput::Item,
            ty: parameter.binding_ty,
        }),
        RuntimeCallableInputSource::Retained { position } => {
            let ty = state.retained.get(position as usize)?.ty;
            let value = mapping.retained().get(position as usize)?;
            value.ownership().permits_copy().then_some(())?;
            Some(Input {
                source: RuntimeNumericMapInput::Constant(runtime_value_as_scalar(value)?),
                ty,
            })
        }
        RuntimeCallableInputSource::Argument { .. } | RuntimeCallableInputSource::Attached => None,
    };
    let mut locals = Inputs::new();
    let mut full = Vec::new();
    for input in body.inputs() {
        let (sources, position) = match input.source() {
            RuntimeFunctionInputSource::Capture { position }
            | RuntimeFunctionInputSource::CapturedParameter { position, .. } => {
                (captures, position)
            }
            RuntimeFunctionInputSource::Parameter { position, .. } => (arguments, position),
        };
        let Some(value) = sources.get(position as usize).copied().and_then(&source) else {
            return Ok(None);
        };
        let Some(declaration) = plan.local_declarations().get(input.input_local()) else {
            return Ok(None);
        };
        if value.ty != declaration.ty() || value.ty != input.pattern().ty() {
            return Ok(None);
        }
        full.push(value);
        match input.pattern().kind() {
            RuntimePatternKind::Bind { binding, .. } | RuntimePatternKind::Typed { binding } => {
                locals.insert(binding.local(), value);
            }
            RuntimePatternKind::Discard => {}
            _ => return Ok(None),
        }
    }
    if body.is_eager_pure_candidate() {
        return selected_projection(plan, *function, full, 0);
    }
    let forwarding = RuntimePureFunctionRef::resolve(plan, *function)?;
    let Some(call) = forwarded_call(plan, forwarding, &locals) else {
        return Ok(None);
    };
    let (callee, args, forwarding_ops) = match call {
        ForwardedCall::Project {
            site,
            control_ops,
            inputs,
        } => {
            let Some(site) = plan.project_call_sites().get(site) else {
                return Ok(None);
            };
            let request = site.plan();
            if request.attached().is_some() {
                return Ok(None);
            }
            let mut args = Vec::new();
            for (position, row) in request.ordinary().iter().enumerate() {
                let RuntimeProjectCallOrdinaryMaterialization::Fixed(row) = row else {
                    return Ok(None);
                };
                if usize::try_from(row.parameter()).ok() != Some(position)
                    || row.abi_ty() != row.binding_ty()
                {
                    return Ok(None);
                }
                let Some(operand) = request.operands().get(row.source_index() as usize) else {
                    return Ok(None);
                };
                if operand.mode() != RuntimeCallArgumentMode::Value {
                    return Ok(None);
                }
                let Some(value) = expression_input(operand.value(), &inputs) else {
                    return Ok(None);
                };
                if value.ty != row.abi_ty() {
                    return Ok(None);
                }
                args.push(value);
            }
            if request.operands().len() != args.len() {
                return Ok(None);
            }
            let RuntimeExprKind::MakeCallable { state, .. } = request.callee().kind() else {
                return Ok(None);
            };
            if *state != request.state() {
                return Ok(None);
            }
            locals = inputs;
            (request.callee(), args, control_ops)
        }
        ForwardedCall::Expression { callee, args } => {
            let mut ordered = args
                .iter()
                .map(|arg| {
                    (
                        arg.abi_position(),
                        (arg.mode() == RuntimeCallArgumentMode::Value)
                            .then(|| expression_input(arg.value(), &locals))
                            .flatten(),
                    )
                })
                .collect::<Vec<_>>();
            ordered.sort_by_key(|(position, _)| *position);
            if ordered
                .iter()
                .enumerate()
                .any(|(index, (position, _))| *position as usize != index)
            {
                return Ok(None);
            }
            let Some(args) = ordered
                .into_iter()
                .map(|(_, value)| value)
                .collect::<Option<Vec<_>>>()
            else {
                return Ok(None);
            };
            (callee, args, 0)
        }
    };
    let RuntimeExprKind::MakeCallable {
        state,
        captures: expressions,
    } = callee.kind()
    else {
        return Ok(None);
    };
    let Some(state) = plan.callable_states().get(*state) else {
        return Ok(None);
    };
    if !matches!(state.attached, RuntimeCallableAttachedContract::None)
        || state.parameters.iter().any(|row| {
            !matches!(row.kind, RuntimeCallableParameterKind::Fixed) || row.abi_ty != row.binding_ty
        })
        || state.parameters.len() != args.len()
        || state.retained.len() != expressions.len()
        || state
            .parameters
            .iter()
            .zip(&args)
            .any(|(row, input)| row.abi_ty != input.ty)
    {
        return Ok(None);
    }
    let RuntimeCallableTransition::Invoke {
        function,
        captures,
        arguments,
    } = &state.transition
    else {
        return Ok(None);
    };
    let Some(target) = plan.function_sites().get(*function) else {
        return Ok(None);
    };
    if matches!(
        forwarding.body,
        super::RuntimePureFunctionBodyRef::Expression(_)
    ) && matches!(target.body(), RuntimeFunctionSiteBody::Executable(_))
    {
        return Ok(None);
    }
    let source = |source: RuntimeCallableInputSource| match source {
        RuntimeCallableInputSource::Retained { position } => {
            let value = expression_input(expressions.get(position as usize)?, &locals)?;
            (value.ty == state.retained.get(position as usize)?.ty).then_some(value)
        }
        RuntimeCallableInputSource::Argument { position } => args.get(position as usize).copied(),
        RuntimeCallableInputSource::Attached => None,
    };
    let mut full = Vec::new();
    for input in target.inputs() {
        let (sources, position) = match input.source() {
            RuntimeFunctionInputSource::Capture { position }
            | RuntimeFunctionInputSource::CapturedParameter { position, .. } => {
                (captures, position)
            }
            RuntimeFunctionInputSource::Parameter { position, .. } => (arguments, position),
        };
        let Some(value) = sources.get(position as usize).copied().and_then(&source) else {
            return Ok(None);
        };
        if !plan
            .local_declarations()
            .get(input.input_local())
            .is_some_and(|row| row.ty() == value.ty)
            || input.pattern().ty() != value.ty
        {
            return Ok(None);
        }
        full.push(value);
    }
    selected_projection(plan, *function, full, forwarding_ops)
}
fn selected_projection<'a>(
    plan: &'a Arc<RuntimePlan>,
    site: crate::runtime_id::RuntimeFunctionSiteId,
    inputs: Vec<Input>,
    forwarding_ops: usize,
) -> Result<Option<RuntimeNumericMapProjection<'a>>, RuntimeEvalError> {
    let Some(body) = plan.function_sites().get(site) else {
        return Ok(None);
    };
    if !body.is_eager_pure_candidate() {
        return Ok(None);
    }
    let function = RuntimePureFunctionRef::resolve(plan, site)?;
    if !function.scalar_eval_supported
        || !function.supports_scalar_frame()
        || function.inputs.len() != inputs.len()
        || inputs.len() > crate::pure::RuntimeI64Args::MAX
    {
        return Ok(None);
    }
    Ok(Some(RuntimeNumericMapProjection {
        function,
        inputs: inputs.into_iter().map(|row| row.source).collect(),
        forwarding_ops,
    }))
}
fn expression_input(expression: &RuntimeExpr, locals: &Inputs) -> Option<Input> {
    expression_input_at_depth(expression, locals, 0)
}
fn expression_input_at_depth(
    expression: &RuntimeExpr,
    locals: &Inputs,
    depth: usize,
) -> Option<Input> {
    if depth > 128 {
        return None;
    }
    let source = match expression.kind() {
        RuntimeExprKind::Scope { body, .. } => {
            let input = expression_input_at_depth(body, locals, depth + 1)?;
            (expression.ty() == input.ty).then_some(input.source)?
        }
        RuntimeExprKind::Value(value) if value.ownership().permits_copy() => {
            RuntimeNumericMapInput::Constant(runtime_value_as_scalar(value)?)
        }
        // Numeric Copy values may be moved by the admitted call. The projection
        // neither clones affine locals nor changes the actual transfer contract.
        RuntimeExprKind::Local(read) if read.fields().is_empty() => {
            let input = locals.get(&read.local())?;
            (input.ty == expression.ty()).then_some(input.source)?
        }
        _ => return None,
    };
    Some(Input {
        source,
        ty: expression.ty(),
    })
}
/// Only a symbolic view of original scalar Copy places is retained here.
/// RuntimePureControlBindings still owns Move availability and exact lexical
/// exit semantics; RuntimePureFunctionRef owns the complete scheduled cost.
#[derive(Clone, Copy)]
enum ForwardingValue {
    Input(Input),
    CallResult {
        site: RuntimeProjectCallSiteId,
        ty: RuntimePlanTypeId,
    },
}
impl ForwardingValue {
    fn ty(self) -> RuntimePlanTypeId {
        match self {
            Self::Input(input) => input.ty,
            Self::CallResult { ty, .. } => ty,
        }
    }
}
type ForwardingBindings = BTreeMap<RuntimeLocalDeclarationId, ForwardingValue>;
struct ForwardingCall {
    site: RuntimeProjectCallSiteId,
    inputs: Inputs,
}
fn forwarding_value(
    expression: &RuntimeExpr,
    control: &super::RuntimePureControlBindings<'_, ForwardingBindings>,
    depth: usize,
) -> Option<ForwardingValue> {
    if depth > 128 {
        return None;
    }
    let value = match expression.kind() {
        RuntimeExprKind::Scope { body, .. } => forwarding_value(body, control, depth + 1)?,
        RuntimeExprKind::Local(read) if read.fields().is_empty() => {
            *control.numeric().get(&read.local())?
        }
        RuntimeExprKind::Value(value) if value.ownership().permits_copy() => {
            ForwardingValue::Input(Input {
                source: RuntimeNumericMapInput::Constant(runtime_value_as_scalar(value)?),
                ty: expression.ty(),
            })
        }
        _ => return None,
    };
    (value.ty() == expression.ty()).then_some(value)
}
fn bind_forwarding_value(
    pattern: &crate::pattern::RuntimePattern,
    value: ForwardingValue,
    control: &mut super::RuntimePureControlBindings<'_, ForwardingBindings>,
) -> Option<()> {
    if pattern.ty() != value.ty() {
        return None;
    }
    match pattern.kind() {
        RuntimePatternKind::Bind { binding, .. } | RuntimePatternKind::Typed { binding } => {
            control.numeric_mut().insert(binding.local(), value);
        }
        RuntimePatternKind::Discard => {}
        _ => return None,
    }
    Some(())
}
fn forwarded_call<'a>(
    plan: &RuntimePlan,
    function: RuntimePureFunctionRef<'a>,
    inputs: &Inputs,
) -> Option<ForwardedCall<'a>> {
    match function.body {
        super::RuntimePureFunctionBodyRef::Expression(expression) => {
            forwarded_expression(expression, 0)
        }
        super::RuntimePureFunctionBodyRef::Executable(body) => {
            fn collect(
                plan: &RuntimePlan,
                ops: &[FlowOp],
                control: &mut super::RuntimePureControlBindings<'_, ForwardingBindings>,
                call: &mut Option<ForwardingCall>,
                depth: usize,
            ) -> Option<bool> {
                use crate::scope::{RuntimeScopeExitTarget, RuntimeScopeFrameKind};
                if depth > 128 {
                    return None;
                }
                for op in ops {
                    match op {
                        FlowOp::Scope { body, .. } => {
                            if !body.is_empty() {
                                let scope =
                                    control.enter_scope(RuntimeScopeFrameKind::EmittedLexical);
                                if collect(plan, body, control, call, depth + 1)? {
                                    return Some(true);
                                }
                                if control.contains_scope(scope) {
                                    control
                                        .exit_scope(RuntimeScopeExitTarget::Frame(scope))
                                        .ok()?;
                                }
                            }
                        }
                        FlowOp::EnterScope { .. } => {
                            control.enter_scope(RuntimeScopeFrameKind::EmittedLexical);
                        }
                        FlowOp::ExitScope => {
                            control
                                .exit_scope(RuntimeScopeExitTarget::EmittedLexical)
                                .ok()?;
                        }
                        FlowOp::Let { pattern, expr } => {
                            if let Some(value) = control.evaluate_unit(expr).ok()? {
                                control.bind_unit(pattern, value).ok()?;
                            } else {
                                let value = forwarding_value(expr, control, 0)?;
                                control.consume_numeric_expression(expr).ok()?;
                                bind_forwarding_value(pattern, value, control)?;
                            }
                        }
                        FlowOp::ExitScopeBind { pattern, expr } => {
                            let unit = control.evaluate_unit(expr).ok()?;
                            let numeric = if unit.is_none() {
                                let value = forwarding_value(expr, control, 0)?;
                                control.consume_numeric_expression(expr).ok()?;
                                Some(value)
                            } else {
                                None
                            };
                            control
                                .exit_scope(RuntimeScopeExitTarget::EmittedLexical)
                                .ok()?;
                            if let Some(value) = unit {
                                control.bind_unit(pattern, value).ok()?;
                            } else {
                                bind_forwarding_value(pattern, numeric?, control)?;
                            }
                        }
                        FlowOp::ProjectCall { site } => {
                            // Exactly one original call may forward the mapping.
                            // Hidden calls and result-dependent next calls decline.
                            if call.is_some() {
                                return None;
                            }
                            let row = plan.project_call_sites().get(*site)?;
                            let request = row.plan();
                            let RuntimeExprKind::MakeCallable { captures, .. } =
                                request.callee().kind()
                            else {
                                return None;
                            };
                            let mut aliases = Inputs::new();
                            for expression in captures
                                .iter()
                                .chain(request.operands().iter().map(|operand| operand.value()))
                            {
                                let ForwardingValue::Input(_) =
                                    forwarding_value(expression, control, 0)?
                                else {
                                    return None;
                                };
                            }
                            for (local, value) in control.numeric() {
                                if let ForwardingValue::Input(input) = value {
                                    aliases.insert(*local, *input);
                                }
                            }
                            for expression in captures
                                .iter()
                                .chain(request.operands().iter().map(|operand| operand.value()))
                            {
                                control.consume_numeric_expression(expression).ok()?;
                            }
                            bind_forwarding_value(
                                row.result(),
                                ForwardingValue::CallResult {
                                    site: *site,
                                    ty: row.result().ty(),
                                },
                                control,
                            )?;
                            *call = Some(ForwardingCall {
                                site: *site,
                                inputs: aliases,
                            });
                        }
                        FlowOp::ReturnExpr(result) => {
                            let ForwardingValue::CallResult { site, .. } =
                                forwarding_value(result, control, 0)?
                            else {
                                return None;
                            };
                            if call.as_ref()?.site != site {
                                return None;
                            }
                            control.consume_numeric_expression(result).ok()?;
                            return Some(true);
                        }
                        FlowOp::Noop => {}
                        _ => return None,
                    }
                }
                Some(false)
            }
            let bindings = inputs
                .iter()
                .map(|(local, input)| (*local, ForwardingValue::Input(*input)))
                .collect();
            let mut control = super::RuntimePureControlBindings::new(function, bindings);
            let mut call = None;
            if !collect(plan, body.ops(), &mut control, &mut call, 0)? {
                return None;
            }
            let ForwardingCall { site, inputs } = call?;
            let control_ops = function.exact_forwarding_control_ops(site)?;
            Some(ForwardedCall::Project {
                site,
                control_ops,
                inputs,
            })
        }
    }
}
fn forwarded_expression(expression: &RuntimeExpr, depth: usize) -> Option<ForwardedCall<'_>> {
    if depth > 128 {
        return None;
    }
    match expression.kind() {
        RuntimeExprKind::Scope { body, .. } => forwarded_expression(body, depth + 1),
        RuntimeExprKind::ApplyGroup { callee, args } => {
            Some(ForwardedCall::Expression { callee, args })
        }
        _ => None,
    }
}

#[derive(Debug)]
pub(crate) enum RuntimeNumericMapBatchData {
    I8(Vec<i8>),
    I16(Vec<i16>),
    I32(Vec<i32>),
    I64(Vec<i64>),
    I128(Vec<i128>),
    ISize(Vec<crate::value::RuntimeISizeValue>),
    U8(Vec<u8>),
    U16(Vec<u16>),
    U32(Vec<u32>),
    U64(Vec<u64>),
    U128(Vec<u128>),
    USize(Vec<crate::value::RuntimeUSizeValue>),
    F32(Vec<f32>),
    F64(Vec<f64>),
}
pub(crate) enum RuntimeNumericMapBatchResult {
    Values(Vec<crate::value::RuntimeValue>),
    Sum(i64),
}
impl RuntimeNumericMapProjection<'_> {
    /// These output ABIs always fit Core's exact i64 Sum member. Wide
    /// values retain full Map execution before their checked narrowing.
    pub(crate) fn supports_total_sum(&self) -> bool {
        use crate::plan::RuntimePureOutputType as O;
        matches!(
            self.function.output_type,
            O::I8 | O::I16 | O::I32 | O::I64 | O::ISize | O::U8 | O::U16 | O::U32
        )
    }
    pub(crate) fn flat_inputs<'value>(
        &self,
        items: impl ExactSizeIterator<Item = &'value crate::value::RuntimeValue>,
    ) -> Option<RuntimeNumericMapBatchData> {
        use crate::plan::{RuntimePureInputType as I, RuntimePureOutputType as O};
        let input_ty = self.function.inputs.first()?.abi();
        if self
            .function
            .inputs
            .iter()
            .any(|input| input.abi() != input_ty)
        {
            return None;
        }
        macro_rules! build {
            ($variant:ident, $ty:ty, $convert:expr) => {{
                let mut flat =
                    Vec::<$ty>::with_capacity(items.len().checked_mul(self.inputs.len())?);
                for item in items {
                    if !item.ownership().permits_copy() {
                        return None;
                    }
                    let item = runtime_value_as_scalar(item)?;
                    for input in &self.inputs {
                        let value = match input {
                            RuntimeNumericMapInput::Item => item,
                            RuntimeNumericMapInput::Constant(value) => *value,
                        };
                        let RuntimePureScalar::$variant(value) = value else {
                            return None;
                        };
                        flat.push(($convert)(value));
                    }
                }
                Some(RuntimeNumericMapBatchData::$variant(flat))
            }};
        }
        match (input_ty, self.function.output_type) {
            (I::I8, O::I8) => build!(I8, i8, |v| v),
            (I::I16, O::I16) => build!(I16, i16, |v| v),
            (I::I32, O::I32) => build!(I32, i32, |v| v),
            (I::I64, O::I64) => build!(I64, i64, |v| v),
            (I::I128, O::I128) => build!(I128, i128, |v| v),
            (I::ISize, O::ISize) => build!(
                ISize,
                crate::value::RuntimeISizeValue,
                crate::value::RuntimeISizeValue::new
            ),
            (I::U8, O::U8) => build!(U8, u8, |v| v),
            (I::U16, O::U16) => build!(U16, u16, |v| v),
            (I::U32, O::U32) => build!(U32, u32, |v| v),
            (I::U64, O::U64) => build!(U64, u64, |v| v),
            (I::U128, O::U128) => build!(U128, u128, |v| v),
            (I::USize, O::USize) => build!(
                USize,
                crate::value::RuntimeUSizeValue,
                crate::value::RuntimeUSizeValue::new
            ),
            (I::F32, O::F32) => build!(F32, f32, |v| v),
            (I::F64, O::F64) => build!(F64, f64, |v| v),
            _ => None,
        }
    }
}
impl RuntimeNumericMapBatchData {
    pub(crate) fn execute(
        self,
        function: RuntimePureFunctionRef<'_>,
        rows: usize,
        fused: bool,
        backend: &mut impl super::RuntimePureCallBackend,
    ) -> Result<RuntimeNumericMapBatchResult, RuntimeEvalError> {
        use crate::value::{RuntimeExactInteger, RuntimeValue};
        let arity = function.inputs.len();
        macro_rules! integers {
            ($inputs:expr, $ty:ty, $batch:ident, $sum:ident, $convert:expr) => {{
                if fused {
                    backend
                        .$sum(function, &$inputs, arity, rows)
                        .map(RuntimeNumericMapBatchResult::Sum)
                } else {
                    let mut output = vec![<$ty>::default(); rows];
                    backend.$batch(function, &$inputs, arity, &mut output)?;
                    Ok(RuntimeNumericMapBatchResult::Values(
                        output.into_iter().map($convert).collect(),
                    ))
                }
            }};
        }
        macro_rules! float {
            ($inputs:expr, $ty:ty, $batch:ident, $variant:ident) => {{
                let mut output = vec![0 as $ty; rows];
                backend.$batch(function, &$inputs, arity, &mut output)?;
                Ok(RuntimeNumericMapBatchResult::Values(
                    output.into_iter().map(RuntimeValue::$variant).collect(),
                ))
            }};
        }
        match self {
            Self::I8(inputs) => integers!(
                inputs,
                i8,
                call_i8_flat_batch,
                call_i8_flat_batch_sum,
                RuntimeValue::i8
            ),
            Self::I16(inputs) => integers!(
                inputs,
                i16,
                call_i16_flat_batch,
                call_i16_flat_batch_sum,
                RuntimeValue::i16
            ),
            Self::I32(inputs) => integers!(
                inputs,
                i32,
                call_i32_flat_batch,
                call_i32_flat_batch_sum,
                RuntimeValue::i32
            ),
            Self::I64(inputs)
                if fused
                    && arity != 0
                    && inputs
                        .chunks_exact(arity)
                        .all(|row| row == &inputs[..arity]) =>
            {
                backend
                    .call_i64_repeated_flat_batch_sum(function, &inputs[..arity], rows)
                    .map(RuntimeNumericMapBatchResult::Sum)
            }
            Self::I64(inputs) => integers!(
                inputs,
                i64,
                call_i64_flat_batch,
                call_i64_flat_batch_sum,
                RuntimeValue::i64
            ),
            Self::I128(inputs) => integers!(
                inputs,
                i128,
                call_i128_flat_batch,
                call_i128_flat_batch_sum,
                RuntimeValue::i128
            ),
            Self::ISize(inputs) => integers!(
                inputs,
                crate::value::RuntimeISizeValue,
                call_exact_int_flat_batch,
                call_exact_int_flat_batch_sum,
                RuntimeExactInteger::into_runtime_value
            ),
            Self::U8(inputs) => integers!(
                inputs,
                u8,
                call_u8_flat_batch,
                call_u8_flat_batch_sum,
                RuntimeValue::u8
            ),
            Self::U16(inputs) => integers!(
                inputs,
                u16,
                call_u16_flat_batch,
                call_u16_flat_batch_sum,
                RuntimeValue::u16
            ),
            Self::U32(inputs) => integers!(
                inputs,
                u32,
                call_u32_flat_batch,
                call_u32_flat_batch_sum,
                RuntimeValue::u32
            ),
            Self::U64(inputs) => integers!(
                inputs,
                u64,
                call_u64_flat_batch,
                call_u64_flat_batch_sum,
                RuntimeValue::u64
            ),
            Self::U128(inputs) => integers!(
                inputs,
                u128,
                call_u128_flat_batch,
                call_u128_flat_batch_sum,
                RuntimeValue::u128
            ),
            Self::USize(inputs) => integers!(
                inputs,
                crate::value::RuntimeUSizeValue,
                call_exact_int_flat_batch,
                call_exact_int_flat_batch_sum,
                RuntimeExactInteger::into_runtime_value
            ),
            Self::F32(inputs) => float!(inputs, f32, call_f32_flat_batch, F32),
            Self::F64(inputs) => float!(inputs, f64, call_f64_flat_batch, F64),
        }
    }
}
