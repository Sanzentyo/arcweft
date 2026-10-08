//! Source-ordered Flow operand attempts for checked formatter occurrences.

use super::*;
use crate::semantic_facts::RuntimeFormatTemplateKey;
use arcweft_core::plan::{RuntimeFormatAttemptDeclarationSeed, RuntimeFormatAttemptOperandSeed};

pub(super) fn reserve(
    context: &FinalLoweringContext<'_, '_>,
    builder: &mut RuntimePlanBuilder,
) -> Result<
    (
        BTreeMap<RuntimeFormatTemplateKey, RuntimeFormatAttemptSeedId>,
        BTreeMap<(RuntimeFormatTemplateKey, RuntimeFmtParameterId), RuntimeLocalSeedId>,
    ),
    RuntimePlanLowerError,
> {
    let mut manifests = BTreeMap::new();
    let mut source_local_specs = BTreeMap::new();
    let mut failure = None;
    context
        .facts
        .visit_scoped_calls(&mut |scope, expression, call| {
            if failure.is_some() {
                return;
            }
            let RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Format(
                formatted,
            )) = call.dispatch()
            else {
                return;
            };
            let Some(module) = module_by_id(context.project, expression.module()) else {
                failure = Some(RuntimePlanLowerError::new(format!(
                    "formatter call {expression:?} has no HIR module"
                )));
                return;
            };
            let mut operands = Vec::with_capacity(call.operands().len());
            let mut flow_sources = Vec::new();
            for operand in call.operands() {
                let result = (|| {
                    let (parameter, source) = source_expression(operand)?;
                    let needs_flow = source_needs_flow(module, scope, source)?;
                    operands.push(RuntimeFormatAttemptOperandSeed::new(
                        parameter,
                        operand.ty().identity(),
                    ));
                    if needs_flow {
                        let origin = scope.expression_coordinate(expression)
                            .ok_or_else(|| RuntimePlanLowerError::new("formatter call has no accepted coordinate"))?
                            .runtime_generated_local_source(arcweft_lang_sema::semantic_coordinate::CheckedGeneratedLocalRole::FormatOperand { parameter })
                            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
                        flow_sources.push((parameter, RuntimeLocalDeclarationSeed::new(origin, operand.ty().identity())));
                    }
                    Ok::<_, RuntimePlanLowerError>(())
                })();
                if let Err(error) = result {
                    failure = Some(error);
                    return;
                }
            }
            if flow_sources.is_empty() {
                return;
            }
            let key = RuntimeFormatTemplateKey::for_call(scope.scope(), formatted);
            if manifests.insert(key.clone(), operands).is_some() {
                failure = Some(RuntimePlanLowerError::new(format!(
                    "formatter call {expression:?} repeats a scoped attempt"
                )));
                return;
            }
            for (parameter, ty) in flow_sources {
                if source_local_specs
                    .insert((key.clone(), parameter), ty)
                    .is_some()
                {
                    failure = Some(RuntimePlanLowerError::new(format!(
                        "formatter call {expression:?} repeats a Flow operand"
                    )));
                    return;
                }
            }
        });
    if let Some(error) = failure {
        return Err(error);
    }
    context
        .facts
        .visit_dialogue_content_fragments(&mut |_, fragment| {
            for value in fragment.values() {
                let Some(project) = value.project_display() else {
                    continue;
                };
                if manifests
                    .insert(
                        project.template().clone(),
                        vec![RuntimeFormatAttemptOperandSeed::new(
                            RuntimeFmtParameterId::Value,
                            project.source_type().identity(),
                        )],
                    )
                    .is_some()
                {
                    failure = Some(RuntimePlanLowerError::new(format!(
                        "project interpolation {:?} repeats a scoped attempt",
                        value.expression()
                    )));
                }
            }
        });
    if let Some(error) = failure {
        return Err(error);
    }
    let mut attempts = BTreeMap::new();
    for (key, operands) in manifests {
        let template = context.facts.format_template(&key).ok_or_else(|| {
            RuntimePlanLowerError::new("formatter attempt has no checked template")
        })?;
        let attempt = builder
            .reserve_format_attempt_seed(RuntimeFormatAttemptDeclarationSeed {
                template: template.template().id(),
                operands: operands.into_boxed_slice(),
            })
            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
        attempts.insert(key, attempt);
    }
    let admission = builder
        .admit_type_batch([], source_local_specs.values().copied())
        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
    let mut source_locals = BTreeMap::new();
    for ((key, _), local) in source_local_specs.iter().zip(admission.local_ids()) {
        source_locals.insert(key.clone(), local.clone());
    }
    if source_locals.len() != source_local_specs.len() {
        return Err(RuntimePlanLowerError::new(
            "formatter Flow operand local admission is incomplete",
        ));
    }
    Ok((attempts, source_locals))
}

pub(super) fn source_expression(
    operand: &crate::semantic_facts::RuntimeResolvedCallOperand,
) -> Result<(RuntimeFmtParameterId, ExprId), RuntimePlanLowerError> {
    let parameter = operand
        .parameter()
        .ok_or_else(|| RuntimePlanLowerError::new("formatter operand has no checked parameter"))?;
    let parameter = (parameter.group() == 0)
        .then(|| usize::try_from(parameter.parameter()).ok())
        .flatten()
        .and_then(RuntimeFmtParameterId::from_index)
        .ok_or_else(|| RuntimePlanLowerError::new("formatter operand parameter is invalid"))?;
    let RuntimeResolvedCallOperandSource::Expression(source) = operand.source() else {
        return Err(RuntimePlanLowerError::new(
            "formatter operand has no expression source",
        ));
    };
    Ok((parameter, source))
}

fn source_needs_flow(
    module: &HirModule,
    scope: RuntimeScopedExecutableSemanticFactView<'_>,
    expression: ExprId,
) -> Result<bool, RuntimePlanLowerError> {
    if let Some(selected) = scope.postfix_candidate(expression) {
        if selected == expression {
            return Err(RuntimePlanLowerError::new(format!(
                "formatter operand {expression:?} selected itself"
            )));
        }
        return source_needs_flow(module, scope, selected);
    }
    if scope.implicit_callable(expression).is_some() {
        return Ok(false);
    }
    let call = scope.call(expression);
    if scope.evaluated_effect(expression).is_some()
        || call.is_some_and(|call| {
            call.project_function().is_some()
                || matches!(call.dispatch(), RuntimeResolvedCallDispatch::Value { .. })
                || call.need_producer().is_some()
                || matches!(
                    call.dispatch(),
                    RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Host(_))
                )
        })
    {
        return Ok(true);
    }
    let hir = module.resolve_expr(expression).map_err(|error| {
        RuntimePlanLowerError::new(format!(
            "formatter operand {expression:?} is absent from HIR: {error}"
        ))
    })?;
    if matches!(
        hir.kind(),
        HirExprKind::AttachedContentApplication(_)
            | HirExprKind::Block(_)
            | HirExprKind::NamedBlock(_)
            | HirExprKind::Await(_)
            | HirExprKind::Choice(_)
            | HirExprKind::Loop(_)
            | HirExprKind::Try(_)
    ) || matches!(
        hir.kind(),
        HirExprKind::ComputationBlock(block)
            if matches!(
                block.kind(),
                arcweft_lang_hir::expr::HirComputationBlockKind::Result
                    | arcweft_lang_hir::expr::HirComputationBlockKind::Option
            )
    ) {
        return Ok(true);
    }
    if let Some(call) = call {
        for operand in call.operands() {
            if let RuntimeResolvedCallOperandSource::Expression(source) = operand.source() {
                if source == expression {
                    return Err(RuntimePlanLowerError::new(format!(
                        "formatter operand {expression:?} selects itself as a call operand"
                    )));
                }
                if source_needs_flow(module, scope, source)? {
                    return Ok(true);
                }
            }
        }
        // The selected call row owns every evaluated operand. A static HIR
        // callee is a selector, not another runtime source expression.
        return Ok(false);
    }
    let children = scope.expression_children(expression).ok_or_else(|| {
        RuntimePlanLowerError::new(format!(
            "formatter operand {expression:?} has no selected children"
        ))
    })?;
    for child in children {
        if source_needs_flow(module, scope, *child)? {
            return Ok(true);
        }
    }
    Ok(false)
}

impl<'a> FinalFlowLowerer<'a> {
    pub(super) fn lower_format_attempt_call(
        &mut self,
        expression: ExprId,
        call: &RuntimeResolvedCall,
        formatted: &crate::semantic_facts::RuntimeResolvedFormatCall,
        continuation: RuntimeFlowValueContinuation,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let key = RuntimeFormatTemplateKey::for_call(self.semantic_facts.scope(), formatted);
        let attempt = self.format_attempts.get(&key).cloned().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "formatter call {expression:?} has no builder-issued operand attempt"
            ))
        })?;
        let mut ops = Vec::with_capacity(call.operands().len() + 1);
        for operand in call.operands() {
            let (parameter, source) = source_expression(operand)?;
            let (body, value) = if source_needs_flow(self.module, self.semantic_facts, source)? {
                let local = self
                    .format_operand_source_locals
                    .get(&(key.clone(), parameter))
                    .cloned()
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "formatter call {expression:?} Flow operand {parameter:?} has no admitted result local"
                        ))
                    })?;
                let body = self.lower_flow_value_with_overrides(
                    source,
                    RuntimeFlowValueContinuation::Bind {
                        pattern: bind_seed(operand.ty(), local.clone()),
                        tail: RuntimeFlowTail::None,
                    },
                    overrides.clone(),
                )?;
                (
                    body,
                    local_seed(operand.ty(), local, RuntimeLocalReadMode::Move),
                )
            } else {
                (
                    Vec::new(),
                    self.expr_lowerer()
                        .with_overrides(overrides.clone())
                        .lower_source(source)
                        .map_err(RuntimePlanLowerError::new)?,
                )
            };
            ops.push(RuntimeFlowOpSeed::FormatOperandAttempt {
                attempt: attempt.clone(),
                parameter,
                body,
                value,
            });
        }
        let content = self
            .expr_lowerer()
            .with_overrides(overrides)
            .lower_source(expression)
            .map_err(RuntimePlanLowerError::new)?;
        ops.extend(self.apply_value_continuation(content, continuation)?);
        Ok(ops)
    }
}
