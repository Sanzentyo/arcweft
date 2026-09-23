//! Program-bound deterministic helper execution with an explicit Sans-I/O
//! external backend. The backend receives the same selected plan Arc.

use super::*;
use crate::runtime_id::RuntimePlanTypeId;
use arcweft_id::runtime_program::RuntimePureProgramId;

#[cfg(test)]
mod tests;

pub fn evaluate_pure_program_with_backend(
    plan: &Arc<RuntimePlan>,
    program: RuntimePureProgramId,
    args: &[RuntimeValue],
    backend: &mut impl RuntimeCallBackend,
) -> Result<RuntimeValue, RuntimeEvalError> {
    let error = |reason: &str| RuntimeEvalError::UnsupportedPure {
        name: program.to_string(),
        reason: reason.to_owned(),
    };
    let mut bindings = plan
        .pure_programs()
        .iter()
        .filter(|binding| binding.program() == program);
    let binding = bindings
        .next()
        .ok_or_else(|| error("pure program is absent from the selected plan"))?;
    if bindings.next().is_some() {
        return Err(error("pure program binding is ambiguous"));
    }
    let helper = resolve_validated_pure_helper(plan, binding.helper())?;
    if helper.input_locals.len() != binding.input_types().len()
        || plan
            .type_table()
            .get(helper.expr.ty())
            .map(RuntimePlanTypeDeclaration::semantic_identity)
            != Some(binding.result_type())
    {
        return Err(error(
            "pure program binding disagrees with its helper signature",
        ));
    }
    for (&local, &expected) in helper.input_locals.iter().zip(binding.input_types()) {
        let actual = plan
            .local_declarations()
            .get(local)
            .and_then(|local| plan.type_table().get(local.ty()))
            .map(RuntimePlanTypeDeclaration::semantic_identity);
        if actual != Some(expected) {
            return Err(error("pure program input disagrees with its helper local"));
        }
    }
    let bindings = prepare_helper_bindings(plan, helper, args.iter().cloned())?;
    let mut evaluator = PureEvaluator::new_ref(plan, &bindings);
    evaluator.external = Some(backend);
    validate_helper_result(plan, helper, evaluator.evaluate_expr(&helper.expr))
}

impl PureEvaluator<'_> {
    pub(super) fn evaluate_external_call_expr(
        &mut self,
        callee: &RuntimeCallTarget,
        args: &[RuntimeCallArgument],
        result_type: RuntimePlanTypeId,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.stats.evaluated_calls += 1;
        let error = |reason: &str| RuntimeEvalError::UnsupportedPure {
            name: callee.as_label().to_owned(),
            reason: reason.to_owned(),
        };
        let mut materialized = Vec::with_capacity(args.len());
        for argument in args {
            let value = self.evaluate_expr(argument.value())?;
            let ty = argument.value().ty();
            let (values, types) = match argument.mode() {
                RuntimeCallArgumentMode::Value => (vec![value], vec![ty]),
                RuntimeCallArgumentMode::Spread => {
                    let values = spread_runtime_values(value)?;
                    let types = match self
                        .plan
                        .type_table()
                        .get(ty)
                        .map(RuntimePlanTypeDeclaration::projection)
                    {
                        Some(
                            RuntimePlanTypeProjection::Sequence { item, .. }
                            | RuntimePlanTypeProjection::Array { item, .. },
                        ) => vec![*item; values.len()],
                        Some(RuntimePlanTypeProjection::Tuple(items))
                            if items.len() == values.len() =>
                        {
                            items.to_vec()
                        }
                        _ => {
                            return Err(error(
                                "pure external spread disagrees with its selected type row",
                            ));
                        }
                    };
                    (values, types)
                }
            };
            materialized.push((argument.abi_position(), values, types));
        }
        materialized.sort_by_key(|(position, _, _)| *position);
        let mut values = Vec::new();
        let mut semantic_types = Vec::new();
        for (_, items, types) in materialized {
            values.extend(items);
            for ty in types {
                semantic_types.push(
                    self.plan
                        .type_table()
                        .get(ty)
                        .ok_or_else(|| error("pure external argument type is missing"))?
                        .semantic_identity(),
                );
            }
        }
        let result = self
            .plan
            .type_table()
            .get(result_type)
            .ok_or_else(|| error("pure external result type is missing"))?
            .semantic_identity();
        let context = RuntimeExternalCallContext::for_program(
            RuntimeProgramOwner::Plan(Arc::clone(self.plan)),
            semantic_types,
            result,
            RuntimeSchemaLimits::engine_default(),
        )
        .map_err(|failure| error(&failure.to_string()))?;
        self.external
            .as_mut()
            .and_then(|backend| backend.call_external(&context, callee, &values))
            .ok_or_else(|| error("selected pure external callable has no backend"))?
    }
}

impl VmPureFunctionScratch {
    pub(super) fn evaluate_values_with_external(
        &mut self,
        plan: &Arc<RuntimePlan>,
        helper: RuntimePureHelperId,
        args: &[RuntimeValue],
        backend: &mut dyn RuntimeExternalCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let helper = resolve_validated_pure_helper(plan, helper)?;
        let bindings = prepare_helper_bindings(plan, helper, args.iter().cloned())?;
        self.env.replace_scopes_with_bindings([bindings]);
        let mut evaluator = PureEvaluator::with_env(plan, std::mem::take(&mut self.env));
        evaluator.external = Some(backend);
        let result = validate_helper_result(plan, helper, evaluator.evaluate_expr(&helper.expr));
        self.env = evaluator.into_env();
        result
    }
}
