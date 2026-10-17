//! Borrowed complete scalar rows from the original checked project-call ABI.

use super::{
    RuntimeNumericInput, RuntimeNumericProjection, RuntimePureFunctionRef, runtime_value_as_scalar,
};
use crate::pattern::RuntimePattern;
use crate::plan::{
    RuntimeCallableAttachedContract, RuntimeCallableInputSource, RuntimeCallableParameterKind,
    RuntimeCallableTransition, RuntimeFunctionInputSource, RuntimePlan,
    RuntimeProjectCallOrdinaryMaterialization,
};
use crate::runtime_id::RuntimeProjectCallSiteId;
use crate::value::{RuntimeCallArgumentMode, RuntimeExpr, RuntimeExprKind, RuntimeValue};
use std::sync::Arc;

pub(crate) struct RuntimeNumericCallRow<'plan> {
    pub(super) function: RuntimePureFunctionRef<'plan>,
    pub(super) values: Box<[&'plan RuntimeValue]>,
    result: &'plan RuntimePattern,
}

impl<'plan> RuntimeNumericProjection<'plan> {
    pub(crate) fn from_calls(rows: &[RuntimeNumericCallRow<'plan>]) -> Option<Self> {
        let function = rows.first()?.function;
        let arity = function.inputs.len();
        if arity > crate::pure::RuntimeI64Args::MAX
            || rows.iter().any(|row| {
                row.function.id != function.id
                    || !Arc::ptr_eq(row.function.plan(), function.plan())
                    || row.values.len() != arity
            })
        {
            return None;
        }
        Some(Self {
            function,
            inputs: (0..arity).map(RuntimeNumericInput::Column).collect(),
            forwarding_ops: 0,
        })
    }
}

impl<'plan> RuntimeNumericCallRow<'plan> {
    pub(crate) fn project(
        plan: &'plan Arc<RuntimePlan>,
        site: RuntimeProjectCallSiteId,
    ) -> Option<Self> {
        let site = plan.project_call_sites().get(site)?;
        let call = site.plan();
        let RuntimeExprKind::MakeCallable {
            state,
            captures: retained,
        } = call.callee().kind()
        else {
            return None;
        };
        if *state != call.state() || call.attached().is_some() {
            return None;
        }
        let state = plan.callable_states().get(*state)?;
        let RuntimeCallableTransition::Invoke {
            function,
            captures,
            arguments,
        } = &state.transition
        else {
            return None;
        };
        let body = plan.function_sites().get(*function)?;
        if body.inputs().len() > crate::pure::RuntimeI64Args::MAX
            || !body.invocation_effects().is_empty()
            || !body.body().is_effect_free()
            || !matches!(state.attached, RuntimeCallableAttachedContract::None)
            || state.parameters.len() != call.ordinary().len()
            || state.retained.len() != retained.len()
            || state.parameters.iter().any(|parameter| {
                !matches!(parameter.kind, RuntimeCallableParameterKind::Fixed)
                    || parameter.abi_ty != parameter.binding_ty
            })
        {
            return None;
        }
        // Only immutable scalar operands are speculated. Reads, moves and
        // executable defaults retain their ordinary evaluation transactions.
        for operand in call.operands() {
            if operand.mode() != RuntimeCallArgumentMode::Value {
                return None;
            }
            constant(plan, operand.value())?;
        }
        for capture in retained {
            constant(plan, capture)?;
        }
        let source = |source| -> Option<&'plan RuntimeExpr> {
            match source {
                RuntimeCallableInputSource::Retained { position } => {
                    let position = usize::try_from(position).ok()?;
                    let expression = retained.get(position)?;
                    (expression.ty() == state.retained.get(position)?.ty).then_some(expression)
                }
                RuntimeCallableInputSource::Argument { position } => {
                    let position = usize::try_from(position).ok()?;
                    let RuntimeProjectCallOrdinaryMaterialization::Fixed(row) =
                        call.ordinary().get(position)?
                    else {
                        return None;
                    };
                    let parameter = state.parameters.get(position)?;
                    if usize::try_from(row.parameter()).ok()? != position
                        || row.abi_ty() != row.binding_ty()
                        || row.abi_ty() != parameter.abi_ty
                    {
                        return None;
                    }
                    let expression = call
                        .operands()
                        .get(usize::try_from(row.source_index()).ok()?)?
                        .value();
                    (expression.ty() == row.abi_ty()).then_some(expression)
                }
                RuntimeCallableInputSource::Attached => None,
            }
        };
        let mut values = Vec::with_capacity(body.inputs().len());
        for input in body.inputs() {
            let (sources, position) = match input.source() {
                RuntimeFunctionInputSource::Capture { position }
                | RuntimeFunctionInputSource::CapturedParameter { position, .. } => {
                    (captures, position)
                }
                RuntimeFunctionInputSource::Parameter { position, .. } => (arguments, position),
            };
            let expression = source(*sources.get(usize::try_from(position).ok()?)?)?;
            if expression.ty() != plan.local_declarations().get(input.input_local())?.ty()
                || expression.ty() != input.pattern().ty()
            {
                return None;
            }
            values.push(constant(plan, expression)?);
        }
        let function = RuntimePureFunctionRef::resolve(plan, *function).ok()?;
        if !function.scalar_eval_supported
            || !function.supports_scalar_frame()
            || function.result_type() != site.result().ty()
        {
            return None;
        }
        Some(Self {
            function,
            values: values.into_boxed_slice(),
            result: site.result(),
        })
    }

    pub(crate) const fn function(&self) -> RuntimePureFunctionRef<'plan> {
        self.function
    }

    pub(crate) const fn result(&self) -> &'plan RuntimePattern {
        self.result
    }
}

fn constant<'a>(plan: &RuntimePlan, expression: &'a RuntimeExpr) -> Option<&'a RuntimeValue> {
    let RuntimeExprKind::Value(value) = expression.kind() else {
        return None;
    };
    if !value.ownership().permits_copy() {
        return None;
    }
    runtime_value_as_scalar(value)?;
    plan.value_matches_type(expression.ty(), value)
        .ok()?
        .then_some(value)
}
