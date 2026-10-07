//! Program-bound function-frame execution with an explicit Sans-I/O
//! external backend. The backend receives the same selected plan Arc.

use super::*;
use crate::runtime_id::RuntimePlanTypeId;
use arcweft_id::runtime_program::RuntimePureProgramId;
use arcweft_interaction_model::dialogue::CharacterDialoguePatchOperation;

#[cfg(test)]
mod tests;

/// Evaluates a program from borrowed inputs. Every input must have the selected
/// type and a transitively unrestricted value graph. All inputs are checked
/// before materialization or execution; rejection retains the caller's values
/// and does not invoke the backend.
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
    let binding = plan.resolve_pure_program(program).map_err(|reason| {
        error(match reason {
            crate::plan::RuntimePureProgramLookupError::Missing => {
                "pure program is absent from the selected plan"
            }
            crate::plan::RuntimePureProgramLookupError::Ambiguous => {
                "pure program binding is ambiguous"
            }
        })
    })?;
    let site = plan
        .function_sites()
        .get(binding.site())
        .ok_or_else(|| error("pure program function site is absent"))?;
    if site.inputs().len() != binding.input_types().len()
        || plan
            .type_table()
            .get(site.result())
            .map(RuntimePlanTypeDeclaration::semantic_identity)
            != Some(binding.result_type())
    {
        return Err(error(
            "pure program binding disagrees with its function signature",
        ));
    }
    for (input, &expected) in site.inputs().iter().zip(binding.input_types()) {
        let actual = plan
            .local_declarations()
            .get(input.input_local())
            .and_then(|local| plan.type_table().get(local.ty()))
            .map(RuntimePlanTypeDeclaration::semantic_identity);
        if actual != Some(expected) {
            return Err(error(
                "pure program input disagrees with its function local",
            ));
        }
    }
    if args.len() != site.inputs().len() {
        return Err(RuntimeEvalError::TooManyPureArgs {
            helper: program.to_string(),
            max: site.inputs().len(),
            found: args.len(),
        });
    }
    for (input, value) in site.inputs().iter().zip(args) {
        let local = input.input_local();
        let declaration = plan
            .local_declarations()
            .get(local)
            .ok_or(RuntimeEvalError::UnknownLocal(local))?;
        if !plan.value_matches_type(declaration.ty(), value)? {
            return Err(RuntimeEvalError::InvalidExpressionType(declaration.ty()));
        }
    }
    for (input, value) in site.inputs().iter().zip(args) {
        if !value.ownership().permits_copy() {
            return Err(RuntimeEvalError::AffineLocalCopy(input.input_local()));
        }
    }
    let mut engine =
        crate::engine::Engine::for_program_invocation(Arc::clone(plan), program, args.to_vec())
            .map_err(|failure| failure.into_parts().0)?;
    let output = engine.step_with_pure_backend(
        crate::step::RuntimeStepInput::default(),
        crate::step::RuntimeStepOptions {
            mode: crate::step::RuntimeStepMode::Drain,
            budget: crate::step::RuntimeStepBudget { max_ops: 1_000_000 },
            ..crate::step::RuntimeStepOptions::default()
        },
        backend,
    );
    let (_, result) = engine.take_program_result()?.ok_or_else(|| {
        error(&format!(
            "program did not return: {:?}; {:?}",
            output.stop_reason, output.output.diagnostics
        ))
    })?;
    if !plan.value_matches_type(site.result(), &result)? {
        return Err(RuntimeEvalError::InvalidExpressionType(site.result()));
    }
    Ok(result)
}

impl PureEvaluator<'_> {
    pub(super) fn evaluate_character_dialogue_expr(
        &mut self,
        result_type: RuntimePlanTypeId,
        operation: CharacterDialogueOperation,
        target: &RuntimeExpr,
        fields: &[CharacterDialoguePatchField<RuntimeExpr>],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let target = self.evaluate_expr(target)?;
        let mut evaluated = Vec::with_capacity(fields.len());
        for field in fields {
            let operation = match &field.operation {
                CharacterDialoguePatchOperation::Set(expression) => {
                    CharacterDialoguePatchOperation::Set(self.evaluate_expr(expression)?)
                }
                CharacterDialoguePatchOperation::Clear => CharacterDialoguePatchOperation::Clear,
            };
            evaluated.push(CharacterDialoguePatchField {
                coordinate: field.coordinate.clone(),
                operation,
            });
        }
        let semantic_type = self
            .plan
            .type_table()
            .get(result_type)
            .ok_or(RuntimeEvalError::UnknownPlanType(result_type))?
            .semantic_identity();
        let owner = RuntimeProgramOwner::Plan(Arc::clone(self.plan));
        self.external
            .as_deref_mut()
            .ok_or(RuntimeEvalError::CharacterDialogueProducerUnavailable)?
            .produce_character_dialogue(&owner, operation, target, &evaluated, semantic_type)
    }

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
        args: Vec<RuntimeValue>,
        backend: &mut dyn RuntimeExternalCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let helper = resolve_validated_pure_helper(plan, helper)?;
        let bindings = prepare_helper_bindings(plan, helper, args)?;
        self.env.replace_scopes_with_bindings([bindings]);
        let mut evaluator = PureEvaluator::with_env(plan, std::mem::take(&mut self.env))
            .with_format_context(self.format_context.clone());
        evaluator.external = Some(backend);
        let result = validate_helper_result(plan, helper, evaluator.evaluate_expr(&helper.expr));
        self.env = evaluator.into_env();
        result
    }
}
