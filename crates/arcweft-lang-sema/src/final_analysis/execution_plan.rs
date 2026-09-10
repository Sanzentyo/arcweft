//! Atomic execution-plan publication from checked expression and call facts.
//!
//! Selected applications, evaluated effects and dialogue consumers are joined
//! here before plans are published. Tooling-only call outcomes own no plan.

use std::collections::{BTreeMap, BTreeSet};

use super::{ExprId, FinalSemanticAnalysisError, PreparedExpressionFact, StmtId};
use crate::checked_rich_text::CheckedDialogueToken;

#[cfg(test)]
mod tests;

pub(super) fn seal(
    expressions: BTreeMap<ExprId, PreparedExpressionFact>,
    calls: &BTreeMap<ExprId, crate::callable::CallTargetFacts>,
    statements: &BTreeMap<StmtId, super::PreparedStatementPayload>,
    dialogue_lines: &arcweft_lang_hir::project::AcceptedDialogueLineInventory,
) -> Result<BTreeMap<ExprId, PreparedExpressionFact>, FinalSemanticAnalysisError> {
    let expressions = expressions
        .into_iter()
        .map(|(owner, fact)| {
            fact.into_complete()
                .map(|checked| (owner, checked))
                .map_err(|_| FinalSemanticAnalysisError::UnsealedPreparedC2Owner)
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let mut roles = BTreeMap::<ExprId, BTreeSet<super::CheckedEvaluatedEffectRole>>::new();
    let mut dialogue_consumers = BTreeMap::new();
    for (statement, payload) in statements {
        let super::PreparedStatementPayload::SealedEvaluatedEffect(effect) = payload else {
            continue;
        };
        add_effect_execution_roles(effect, Some(*statement), None, calls, &mut roles)?;
    }
    for (owner, checked) in &expressions {
        let super::CheckedExpressionResolution::DialogueApplication {
            target, rich_text, ..
        } = checked.resolution()
        else {
            continue;
        };
        let line = dialogue_lines
            .for_semantic_expr(*owner)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        if dialogue_consumers
            .insert(target.expression(), (*owner, line.id().clone()))
            .is_some()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        add_rich_text_effect_execution_roles(rich_text, calls, &mut roles)?;
    }

    let replacements = expressions
        .into_iter()
        .map(|(owner, checked)| {
            let plan = execution_plan_for_expression(owner, &checked, calls)?;
            let effect_roles = roles
                .remove(&owner)
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice();
            let dialogue_consumer = dialogue_consumers.remove(&owner);
            let plan = match plan {
                Some(plan) => {
                    let mut plan = plan.with_evaluated_effect_roles(effect_roles);
                    if let Some((dialogue_owner, line)) = dialogue_consumer
                        && plan.call_application().is_some()
                    {
                        plan = plan
                            .with_dialogue_consumer(dialogue_owner, line)
                            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                    }
                    Some(plan)
                }
                None if effect_roles.is_empty() && dialogue_consumer.is_none() => None,
                None => return Err(FinalSemanticAnalysisError::WrongPayloadFamily),
            };
            Ok((
                owner,
                super::PreparedExpressionFact::Complete(checked.with_execution_plan(plan)),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, FinalSemanticAnalysisError>>()?;
    if !roles.is_empty() || !dialogue_consumers.is_empty() {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    Ok(replacements)
}

fn add_rich_text_effect_execution_roles(
    report: &super::CheckedRichTextReport,
    calls: &BTreeMap<ExprId, crate::callable::CallTargetFacts>,
    roles: &mut BTreeMap<ExprId, BTreeSet<super::CheckedEvaluatedEffectRole>>,
) -> Result<(), FinalSemanticAnalysisError> {
    let owner = report.content().id().owner();
    for site in report.effect_plan().effect_sites() {
        add_effect_execution_roles(
            site.effect(),
            None,
            Some((owner, site.root(), site.id())),
            calls,
            roles,
        )?;
    }
    for token in report.content().tokens() {
        if let CheckedDialogueToken::ContentInsert(insertion) = token {
            if let Some(child) = insertion.argument().checked_content() {
                add_rich_text_effect_execution_roles(child, calls, roles)?;
            }
        }
    }
    Ok(())
}

fn add_effect_execution_roles(
    effect: &super::CheckedEvaluatedEffect,
    statement: Option<StmtId>,
    dialogue_site: Option<(ExprId, ExprId, super::CheckedDialogueEffectSiteOrdinal)>,
    calls: &BTreeMap<ExprId, crate::callable::CallTargetFacts>,
    roles: &mut BTreeMap<ExprId, BTreeSet<super::CheckedEvaluatedEffectRole>>,
) -> Result<(), FinalSemanticAnalysisError> {
    let root = effect.site_root();
    if let Some(statement) = statement {
        roles
            .entry(root)
            .or_default()
            .insert(super::CheckedEvaluatedEffectRole::StatementRoot { statement });
    }
    if let Some((owner, site_root, ordinal)) = dialogue_site {
        if site_root != root {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        roles.entry(root).or_default().insert(
            super::CheckedEvaluatedEffectRole::DialogueEffectSite {
                owner,
                root: site_root,
                ordinal,
            },
        );
    }

    let application = effect.application_digest();
    let terminal = effect.application().raw().expression();
    let mut current = Some(terminal);
    let mut visited = BTreeSet::new();
    while let Some(owner) = current {
        if !visited.insert(owner) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let selected = calls
            .get(&owner)
            .and_then(crate::callable::CallTargetFacts::selected_application)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        if owner == terminal && selected.digest() != application {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        roles
            .entry(owner)
            .or_default()
            .insert(super::CheckedEvaluatedEffectRole::Application { application });
        current = match selected.core().candidates().selected().state() {
            crate::callable::ResolvedCallableState::Base => None,
            crate::callable::ResolvedCallableState::Continuation(continuation) => {
                Some(continuation.prefix_call_site().expression())
            }
        };
    }

    if let super::CheckedEvaluatedEffectOperation::Drop {
        invocation: super::CheckedDropInvocation::DropWithPolicy { source, .. },
        ..
    } = effect.operation()
    {
        if let crate::callable::CheckedCallArgumentSlotSource::Expression(owner) =
            source.operand().source().raw()
        {
            roles
                .entry(owner)
                .or_default()
                .insert(super::CheckedEvaluatedEffectRole::DropPolicy { application });
        }
    }
    Ok(())
}

fn execution_plan_for_expression(
    owner: ExprId,
    checked: &super::CheckedExpression,
    calls: &BTreeMap<ExprId, crate::callable::CallTargetFacts>,
) -> Result<Option<super::CheckedExpressionExecutionPlan>, FinalSemanticAnalysisError> {
    use super::{
        CheckedExpressionCallCallee, CheckedExpressionResolution, CheckedRuntimeValueDisposition,
        CheckedStructuralExecutionReason,
    };
    if matches!(
        checked.result(),
        super::CheckedExpressionResult::Unavailable
    ) {
        let facts = calls
            .get(&owner)
            .ok_or(FinalSemanticAnalysisError::CallFactMismatch)?;
        if !matches!(checked.resolution(), CheckedExpressionResolution::Call)
            || facts.outcome().site() != crate::callable::CheckedCallSite::HirCall(owner)
            || facts.selected_application().is_some()
        {
            return Err(FinalSemanticAnalysisError::CallFactMismatch);
        }
        return Ok(None);
    }
    let value = if checked.result().value_type().is_some() {
        CheckedRuntimeValueDisposition::Retain
    } else {
        CheckedRuntimeValueDisposition::Omit
    };
    let call =
        |expected: crate::callable::CheckedCallSite,
         expected_digest: Option<crate::callable::CheckedCallApplicationDigest>| {
            let facts = calls
                .get(&owner)
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            if facts.outcome().site() != expected {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            let Some(application) = facts.selected_application() else {
                return Err(FinalSemanticAnalysisError::CallFactMismatch);
            };
            if expected_digest.is_some_and(|digest| application.digest() != digest) {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            let callee = if matches!(
                application.core().callee(),
                crate::callable::CheckedCallCalleeExecution::Value { .. }
            ) || matches!(
                application.core().execution().receiver(),
                crate::callable::CheckedCallReceiverProjection::Operand { .. }
            ) {
                CheckedExpressionCallCallee::RuntimeReceiver
            } else {
                CheckedExpressionCallCallee::Static
            };
            Ok(super::CheckedExpressionExecutionPlan::call(
                application.digest(),
                value,
                callee,
            ))
        };

    let plan = match checked.resolution() {
        CheckedExpressionResolution::Call
        | CheckedExpressionResolution::CharacterDialogueFactory(_)
        | CheckedExpressionResolution::CharacterDialogueReconfigure(_)
        | CheckedExpressionResolution::ViewFxApplication(_) => {
            call(crate::callable::CheckedCallSite::HirCall(owner), None)
        }
        CheckedExpressionResolution::ContentApplication(application) => {
            match application.as_ref() {
                super::CheckedContentApplication::Value { .. } => {
                    Ok(super::CheckedExpressionExecutionPlan::structural(
                        value,
                        CheckedStructuralExecutionReason::ContentValue,
                    ))
                }
                super::CheckedContentApplication::ContentResultCall { application, .. } => call(
                    crate::callable::CheckedCallSite::AttachedContentApplication {
                        expression: owner,
                        family:
                            crate::callable::CheckedAttachedContentApplicationFamily::ContentCall,
                    },
                    Some(*application),
                ),
                super::CheckedContentApplication::EmissionCall { .. } => {
                    Ok(super::CheckedExpressionExecutionPlan::structural(
                        CheckedRuntimeValueDisposition::Omit,
                        CheckedStructuralExecutionReason::ContentEmission,
                    ))
                }
            }
        }
        CheckedExpressionResolution::DialogueApplication { .. } => {
            Ok(super::CheckedExpressionExecutionPlan::structural(
                CheckedRuntimeValueDisposition::Omit,
                CheckedStructuralExecutionReason::DialogueApplication,
            ))
        }
        CheckedExpressionResolution::CompileTimeCallee(_)
        | CheckedExpressionResolution::TypeValue(_)
        | CheckedExpressionResolution::CompileTimeScalar(_) => {
            Ok(super::CheckedExpressionExecutionPlan::structural(
                CheckedRuntimeValueDisposition::Omit,
                CheckedStructuralExecutionReason::CompileTimeOnly,
            ))
        }
        CheckedExpressionResolution::PostfixBracket(_) => {
            Ok(super::CheckedExpressionExecutionPlan::structural(
                CheckedRuntimeValueDisposition::Omit,
                CheckedStructuralExecutionReason::PostfixBracket,
            ))
        }
        CheckedExpressionResolution::Structural => {
            Ok(super::CheckedExpressionExecutionPlan::structural(
                value,
                CheckedStructuralExecutionReason::Structural,
            ))
        }
        CheckedExpressionResolution::Literal(_) => {
            Ok(super::CheckedExpressionExecutionPlan::structural(
                value,
                CheckedStructuralExecutionReason::Literal,
            ))
        }
        CheckedExpressionResolution::Value(_)
        | CheckedExpressionResolution::Select(_)
        | CheckedExpressionResolution::Nominal(_)
        | CheckedExpressionResolution::Variant(_)
        | CheckedExpressionResolution::CompileTimeEnum(_)
        | CheckedExpressionResolution::DialogueLineReference(_)
        | CheckedExpressionResolution::DialogueLineCoordinate(_)
        | CheckedExpressionResolution::DialogueTextKeyCoordinate(_)
        | CheckedExpressionResolution::StageLook(_)
        | CheckedExpressionResolution::Effect(_)
        | CheckedExpressionResolution::Await(_)
        | CheckedExpressionResolution::Choice(_)
        | CheckedExpressionResolution::Try(_)
        | CheckedExpressionResolution::ImplicitCallable(_)
        | CheckedExpressionResolution::Closure(_)
        | CheckedExpressionResolution::ImplicitParameter(_)
        | CheckedExpressionResolution::Pipe(_)
        | CheckedExpressionResolution::PipeLeft(_)
        | CheckedExpressionResolution::ViewCall(_)
        | CheckedExpressionResolution::StyleValue(_) => {
            Ok(super::CheckedExpressionExecutionPlan::structural(
                value,
                CheckedStructuralExecutionReason::Value,
            ))
        }
    }?;
    Ok(Some(plan))
}
