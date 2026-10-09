//! Native expression entry into the program's callable-state authority.

#[cfg(test)]
#[path = "function/tests.rs"]
mod tests;

use std::sync::Arc;

use crate::pattern::match_runtime_pattern_owned;
use crate::plan::{RuntimeFunctionInputSource, RuntimeFunctionSiteBody};
use crate::plan::{RuntimePureInputType, RuntimePureOutputType};
use crate::pure::{RuntimeFixedArgs, RuntimeI32Args, RuntimeI64Args, RuntimePureFunctionRef};
use crate::runtime_id::{RuntimeCallableStateId, RuntimeFunctionSiteId};
use crate::task::RuntimeProgramOwner;
use crate::value::{
    RuntimeCallArgument, RuntimeCallArgumentMode, RuntimeCallableApplication,
    RuntimeCallableBodyReference, RuntimeCallableValue, RuntimeCallableValueError,
    RuntimeExactInteger, RuntimeValue,
};

use super::{
    Engine, RuntimeCallBackend, RuntimeEvalError, RuntimeExpr, runtime_value_label,
    spread_runtime_values,
};

impl Engine {
    pub(super) fn evaluate_specialize_callable_expr(
        &mut self,
        value: &RuntimeExpr,
        specialization: crate::runtime_id::RuntimeCallableSpecializationId,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr_with_backend(value, backend)?;
        let RuntimeValue::Callable(callable) = value else {
            return Err(RuntimeEvalError::ExpectedFunction(runtime_value_label(
                &value,
            )));
        };
        callable
            .specialize(
                &RuntimeProgramOwner::Plan(Arc::clone(&self.plan)),
                specialization,
            )
            .map(RuntimeValue::Callable)
            .map_err(Into::into)
    }

    pub(in crate::engine) fn evaluate_dialogue_site(
        &mut self,
        site: &crate::plan::RuntimeDialogueValueSite,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let captures = site
            .captures()
            .iter()
            .map(|capture| self.evaluate_expr_with_backend(capture, backend))
            .collect::<Result<Vec<_>, _>>()?;
        self.evaluate_function_site(site.function(), captures, Vec::new(), backend)
    }

    pub(in crate::engine) fn evaluate_callable_expr(
        &mut self,
        state: RuntimeCallableStateId,
        captures: &[RuntimeExpr],
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let captures = captures
            .iter()
            .map(|capture| self.evaluate_expr_with_backend(capture, backend))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RuntimeValue::Callable(RuntimeCallableValue::try_new(
            RuntimeProgramOwner::Plan(Arc::clone(&self.plan)),
            state,
            captures,
        )?))
    }

    pub(super) fn evaluate_apply_expr(
        &mut self,
        callee: &RuntimeExpr,
        args: &[RuntimeCallArgument],
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let callee = self.evaluate_expr_with_backend(callee, backend)?;
        let args = self.evaluate_function_call_args(args, backend)?;
        let RuntimeValue::Callable(callable) = callee else {
            return Err(RuntimeEvalError::ExpectedFunction(runtime_value_label(
                &callee,
            )));
        };
        self.apply_runtime_function(callable, args, backend)
    }

    pub(in crate::engine) fn evaluate_function_call_args(
        &mut self,
        args: &[RuntimeCallArgument],
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
        let mut materialized = Vec::with_capacity(args.len());
        for argument in args {
            let value = self.evaluate_expr_with_backend(argument.value(), backend)?;
            let values = match argument.mode() {
                RuntimeCallArgumentMode::Value => vec![value],
                RuntimeCallArgumentMode::Spread => spread_runtime_values(value)?,
            };
            materialized.push((argument.abi_position(), values));
        }
        materialized.sort_by_key(|(position, _)| *position);
        Ok(materialized
            .into_iter()
            .flat_map(|(_, values)| values)
            .collect())
    }

    pub(crate) fn apply_runtime_function(
        &mut self,
        callable: RuntimeCallableValue,
        args: Vec<RuntimeValue>,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        callable.validate_for_owner(&RuntimeProgramOwner::Plan(Arc::clone(&self.plan)))?;
        if args.len() < callable.remaining_arity()? {
            return Ok(RuntimeValue::Callable(
                callable
                    .try_bind_prefix(args)
                    .map_err(|failure| RuntimeEvalError::Callable(failure.into_parts().0))?,
            ));
        }
        let arguments = callable
            .materialize_arrow_arguments(args)
            .map_err(|failure| RuntimeEvalError::Callable(failure.into_parts().0))?;
        let mut application = callable
            .prepare_group(arguments, None)
            .map_err(|failure| RuntimeEvalError::Callable(failure.into_parts().0))?;
        loop {
            match application {
                RuntimeCallableApplication::Complete(value) => return Ok(value),
                RuntimeCallableApplication::Invoke(invocation) => {
                    let RuntimeCallableBodyReference::Plan(site) = invocation.body else {
                        return Err(RuntimeCallableValueError::ForeignProgram.into());
                    };
                    return self.evaluate_function_site(
                        site,
                        invocation.captures,
                        invocation.arguments,
                        backend,
                    );
                }
                RuntimeCallableApplication::AttachedDefault {
                    invocation,
                    pending,
                } => {
                    let RuntimeCallableBodyReference::Plan(site) = invocation.body else {
                        return Err(RuntimeCallableValueError::ForeignProgram.into());
                    };
                    let value = self.evaluate_function_site(
                        site,
                        invocation.captures,
                        invocation.arguments,
                        backend,
                    )?;
                    application = pending
                        .complete_default(value)
                        .map_err(|failure| RuntimeEvalError::Callable(failure.into_parts().0))?;
                }
            }
        }
    }

    pub(in crate::engine) fn evaluate_function_site(
        &mut self,
        site: RuntimeFunctionSiteId,
        captures: Vec<RuntimeValue>,
        arguments: Vec<RuntimeValue>,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let plan = Arc::clone(&self.plan);
        let declaration = plan.validate_function_site_inputs(site, &captures, &arguments)?;
        let type_instantiation = declaration.type_instantiation.clone();
        let RuntimeFunctionSiteBody::Expression(body) = declaration.body() else {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: "structured.function".to_owned(),
                reason: "an executable runtime function requires function-call control transfer"
                    .to_owned(),
            });
        };
        if declaration.is_eager_pure_candidate()
            && let Ok(function) = RuntimePureFunctionRef::resolve(&plan, site)
            && let Some(value) =
                Self::evaluate_scalar_function_site(function, &captures, &arguments, backend)?
        {
            if !plan.value_matches_type(declaration.result(), &value)? {
                return Err(RuntimeEvalError::InvalidExpressionType(
                    declaration.result(),
                ));
            }
            return Ok(value);
        }
        let mut captures = captures.into_iter().map(Some).collect::<Vec<_>>();
        let mut arguments = arguments.into_iter().map(Some).collect::<Vec<_>>();
        let mut staged = Vec::new();
        for input in declaration.inputs() {
            let (values, position) = match input.source() {
                RuntimeFunctionInputSource::Capture { position }
                | RuntimeFunctionInputSource::CapturedParameter { position, .. } => {
                    (&mut captures, position)
                }
                RuntimeFunctionInputSource::Parameter { position, .. } => {
                    (&mut arguments, position)
                }
            };
            let value = values
                .get_mut(position as usize)
                .and_then(Option::take)
                .ok_or(
                    crate::value::RuntimeFunctionApplyError::InvalidBoundArgumentPrefix { site },
                )?;
            let bindings = match_runtime_pattern_owned(
                &plan,
                input.pattern(),
                value,
                type_instantiation.as_deref(),
            )?
            .ok_or_else(|| {
                RuntimeEvalError::PatternMismatch(format!(
                    "function site {site} input {:?}",
                    input.source()
                ))
            })?;
            staged.extend(bindings);
        }
        self.fiber
            .env
            .push_function_scope(site, staged.len(), type_instantiation.clone());
        self.fiber.env.bind_all(staged);
        let value = self.evaluate_expr_with_backend(body, backend);
        let value = value.and_then(|value| {
            let matches = match &type_instantiation {
                Some(binding) => binding.value_matches(plan.as_ref(), declaration.result(), &value),
                None => plan.value_matches_type(declaration.result(), &value)?,
            };
            if matches {
                Ok(value)
            } else {
                Err(RuntimeEvalError::InvalidExpressionType(
                    declaration.result(),
                ))
            }
        });
        self.fiber.env.pop_scope();
        value
    }

    /// The normal function ingress has already validated the complete packet.
    /// Scalar backends borrow only a fixed physical pack derived from those
    /// exact rows; every declared formal remains present, including discards.
    fn evaluate_scalar_function_site(
        function: RuntimePureFunctionRef<'_>,
        captures: &[RuntimeValue],
        arguments: &[RuntimeValue],
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let arity = function.inputs.len();
        if arity > RuntimeFixedArgs::<i64>::MAX || !function.supports_scalar_frame() {
            return Ok(None);
        }
        let value = |index: usize| -> Result<&RuntimeValue, RuntimeEvalError> {
            let input = function
                .inputs
                .get(index)
                .and_then(crate::pure::RuntimePureFunctionInputRef::function_input)
                .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                    name: function.name.to_owned(),
                    reason: "function input authority is absent".to_owned(),
                })?;
            let supplied = match input.source() {
                RuntimeFunctionInputSource::Capture { position }
                | RuntimeFunctionInputSource::CapturedParameter { position, .. } => {
                    captures.get(position as usize)
                }
                RuntimeFunctionInputSource::Parameter { position, .. } => {
                    arguments.get(position as usize)
                }
            };
            supplied.ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                name: function.name.to_owned(),
                reason: "complete function input packet is absent".to_owned(),
            })
        };
        macro_rules! exact {
            ($ty:ty, $abi:ident) => {{
                if !function
                    .inputs
                    .iter()
                    .all(|input| input.abi() == RuntimePureInputType::$abi)
                {
                    return Ok(None);
                }
                let mut pack = [<$ty>::default(); 4];
                for (index, slot) in pack.iter_mut().take(arity).enumerate() {
                    *slot = <$ty as RuntimeExactInteger>::try_from_runtime_value(
                        function.name,
                        value(index)?.clone(),
                    )?;
                }
                backend
                    .call_exact_int_slice::<$ty>(function, &pack[..arity])
                    .map(|value| value.map(RuntimeExactInteger::into_runtime_value))
            }};
        }
        match function.output_type {
            RuntimePureOutputType::I8 => exact!(i8, I8),
            RuntimePureOutputType::I16 => exact!(i16, I16),
            RuntimePureOutputType::I128 => exact!(i128, I128),
            RuntimePureOutputType::ISize => exact!(crate::value::RuntimeISizeValue, ISize),
            RuntimePureOutputType::U8 => exact!(u8, U8),
            RuntimePureOutputType::U16 => exact!(u16, U16),
            RuntimePureOutputType::U32 => exact!(u32, U32),
            RuntimePureOutputType::U64 => exact!(u64, U64),
            RuntimePureOutputType::U128 => exact!(u128, U128),
            RuntimePureOutputType::USize => exact!(crate::value::RuntimeUSizeValue, USize),
            RuntimePureOutputType::I32 => {
                if !function
                    .inputs
                    .iter()
                    .all(|input| input.abi() == RuntimePureInputType::I32)
                {
                    return Ok(None);
                }
                let mut pack = [0i32; 4];
                for (index, slot) in pack.iter_mut().take(arity).enumerate() {
                    *slot = i32::try_from_runtime_value(function.name, value(index)?.clone())?;
                }
                backend
                    .call_i32(function, RuntimeI32Args::new(pack, arity))
                    .map(|value| value.map(RuntimeValue::i32))
            }
            RuntimePureOutputType::I64 => {
                if !function
                    .inputs
                    .iter()
                    .all(|input| input.abi() == RuntimePureInputType::I64)
                {
                    return Ok(None);
                }
                let mut pack = [0i64; 4];
                for (index, slot) in pack.iter_mut().take(arity).enumerate() {
                    let RuntimeValue::Int(integer) = value(index)? else {
                        return Err(RuntimeEvalError::ExpectedInt(runtime_value_label(value(
                            index,
                        )?)));
                    };
                    *slot = integer
                        .exact_i64()
                        .ok_or_else(|| RuntimeEvalError::ExpectedInt(integer.to_string()))?;
                }
                backend
                    .call_i64(function, RuntimeI64Args::new(pack, arity))
                    .map(|value| value.map(RuntimeValue::i64))
            }
            RuntimePureOutputType::F32 => {
                if !function
                    .inputs
                    .iter()
                    .all(|input| input.abi() == RuntimePureInputType::F32)
                {
                    return Ok(None);
                }
                let mut pack = [0f32; 4];
                for (index, slot) in pack.iter_mut().take(arity).enumerate() {
                    let RuntimeValue::F32(supplied) = value(index)? else {
                        return Ok(None);
                    };
                    *slot = *supplied;
                }
                backend
                    .call_f32_slice(function, &pack[..arity])
                    .map(|value| value.map(RuntimeValue::F32))
            }
            RuntimePureOutputType::F64 => {
                if !function
                    .inputs
                    .iter()
                    .all(|input| input.abi() == RuntimePureInputType::F64)
                {
                    return Ok(None);
                }
                let mut pack = [0f64; 4];
                for (index, slot) in pack.iter_mut().take(arity).enumerate() {
                    let RuntimeValue::F64(supplied) = value(index)? else {
                        return Ok(None);
                    };
                    *slot = *supplied;
                }
                backend
                    .call_f64_slice(function, &pack[..arity])
                    .map(|value| value.map(RuntimeValue::F64))
            }
            RuntimePureOutputType::Bool | RuntimePureOutputType::Value => Ok(None),
        }
    }
}
