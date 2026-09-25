//! Atomic execution-plan publication from checked expression and call facts.
//!
//! Selected applications, evaluated effects and dialogue targets are joined
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
    let mut statement_roots =
        BTreeMap::<ExprId, (StmtId, super::CheckedEvaluatedEffectReference)>::new();
    for (statement, payload) in statements {
        let super::PreparedStatementPayload::SealedEvaluatedEffectReference(reference) = payload
        else {
            continue;
        };
        if statement_roots
            .insert(reference.site_root(), (*statement, *reference))
            .is_some()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
    }
    for (owner, checked) in &expressions {
        let Some(effect) = checked.evaluated_effect() else {
            continue;
        };
        if effect.site_root() != *owner {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let statement = match statement_roots.remove(owner) {
            Some((statement, reference)) if reference == effect.reference() => Some(statement),
            Some(_) => return Err(FinalSemanticAnalysisError::WrongPayloadFamily),
            None => None,
        };
        add_effect_execution_roles(effect, statement, None, calls, &mut roles)?;
    }
    if !statement_roots.is_empty() {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    for (owner, checked) in &expressions {
        let super::CheckedExpressionResolution::DialogueApplication {
            target, rich_text, ..
        } = checked.resolution()
        else {
            continue;
        };
        dialogue_lines
            .for_semantic_expr(*owner)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        // The target is evaluated as an ordinary value before line content.
        // The application owns line execution, not the target's call/result.
        if expressions
            .get(&target.expression())
            .and_then(|target| target.value_type())
            != Some(&target.ty())
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        add_rich_text_effect_execution_roles(rich_text, calls, &mut roles)?;
    }

    let contextual_receivers = contextual_receiver_sources(&expressions, calls)?;
    let line_schedule_prefixes = fused_line_schedule_prefixes(calls)?;
    for (owner, checked) in &expressions {
        let contextual_kind = match checked.value_type() {
            Some(crate::types::TypeKind::LineContext) => {
                Some(crate::callable::CheckedCallContextualReceiverKind::LineContext)
            }
            Some(crate::types::TypeKind::StageApi(_)) => {
                Some(crate::callable::CheckedCallContextualReceiverKind::CharacterStage)
            }
            _ => None,
        };
        if contextual_kind.is_some_and(|kind| contextual_receivers.get(owner) != Some(&kind)) {
            return Err(
                FinalSemanticAnalysisError::ContextualCapabilityRequiresDirectReceiver {
                    owner: *owner,
                },
            );
        }
    }

    let replacements = expressions
        .into_iter()
        .map(|(owner, checked)| {
            let plan = execution_plan_for_expression(
                owner,
                &checked,
                calls,
                &contextual_receivers,
                &line_schedule_prefixes,
            )?;
            let effect_roles = roles
                .remove(&owner)
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice();
            let plan = match plan {
                Some(plan) => Some(plan.with_evaluated_effect_roles(effect_roles)),
                None if effect_roles.is_empty() => None,
                None => return Err(FinalSemanticAnalysisError::WrongPayloadFamily),
            };
            Ok((
                owner,
                super::PreparedExpressionFact::Complete(checked.with_execution_plan(plan)),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, FinalSemanticAnalysisError>>()?;
    if !roles.is_empty() {
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
    if dialogue_site.is_none() {
        roles
            .entry(root)
            .or_default()
            .insert(super::CheckedEvaluatedEffectRole::ExpressionRoot { root });
    }
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

fn fused_line_schedule_prefixes(
    calls: &BTreeMap<ExprId, crate::callable::CallTargetFacts>,
) -> Result<BTreeSet<ExprId>, FinalSemanticAnalysisError> {
    use crate::callable::{
        CallableCandidateId, CheckedCallCalleeExecution, CheckedCallResult, LineScheduleCallableId,
        ResolvedCallableState,
    };

    let mut prefixes = BTreeSet::new();
    for facts in calls.values() {
        let Some(completion) = facts.selected_application() else {
            continue;
        };
        let selected = completion.core().candidates().selected();
        if selected.id() != &CallableCandidateId::LineSchedule(LineScheduleCallableId::At) {
            continue;
        }
        let ResolvedCallableState::Continuation(continuation) = selected.state() else {
            continue;
        };
        let prefix_owner = continuation.prefix_call_site().expression();
        let CheckedCallCalleeExecution::Value { source } = completion.core().callee() else {
            return Err(FinalSemanticAnalysisError::CallFactMismatch);
        };
        if source.owner() != prefix_owner {
            continue;
        }
        let prefix = calls
            .get(&prefix_owner)
            .and_then(crate::callable::CallTargetFacts::selected_application)
            .ok_or(FinalSemanticAnalysisError::CallFactMismatch)?;
        if prefix.core().digest() != continuation.prefix_application_core()
            || prefix.core().stable_site() != continuation.prefix_application_site()
            || prefix.core().site() != continuation.prefix_call_site()
            || !matches!(prefix.result(), CheckedCallResult::Continuation(_))
        {
            return Err(FinalSemanticAnalysisError::CallFactMismatch);
        }
        prefixes.insert(prefix_owner);
    }
    Ok(prefixes)
}

fn execution_plan_for_expression(
    owner: ExprId,
    checked: &super::CheckedExpression,
    calls: &BTreeMap<ExprId, crate::callable::CallTargetFacts>,
    contextual_receivers: &BTreeMap<ExprId, crate::callable::CheckedCallContextualReceiverKind>,
    line_schedule_prefixes: &BTreeSet<ExprId>,
) -> Result<Option<super::CheckedExpressionExecutionPlan>, FinalSemanticAnalysisError> {
    use super::{
        CheckedExpressionCallCallee, CheckedExpressionResolution, CheckedRuntimeValueDisposition,
        CheckedStructuralExecutionReason, CheckedValueResolution,
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
    if line_schedule_prefixes.contains(&owner) {
        let application = calls
            .get(&owner)
            .and_then(crate::callable::CallTargetFacts::selected_application)
            .ok_or(FinalSemanticAnalysisError::CallFactMismatch)?;
        if !matches!(checked.resolution(), CheckedExpressionResolution::Call) {
            return Err(FinalSemanticAnalysisError::CallFactMismatch);
        }
        return Ok(Some(
            super::CheckedExpressionExecutionPlan::fused_line_schedule_prefix(application.digest()),
        ));
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
            let value_callee = match checked.resolution() {
                CheckedExpressionResolution::CharacterDialogueFactory(factory) => {
                    Some(factory.target().expression())
                }
                CheckedExpressionResolution::CharacterDialogueReconfigure(reconfigure) => {
                    Some(reconfigure.target().expression())
                }
                _ => match application.core().callee() {
                    crate::callable::CheckedCallCalleeExecution::Value { source } => {
                        Some(source.owner())
                    }
                    crate::callable::CheckedCallCalleeExecution::Direct => None,
                },
            };
            let callee = if let Some(expression) = value_callee {
                CheckedExpressionCallCallee::RuntimeValue { expression }
            } else if matches!(
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
        CheckedExpressionResolution::Structural | CheckedExpressionResolution::Scope(_) => {
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
        CheckedExpressionResolution::Value(CheckedValueResolution::LineContext)
            if contextual_receivers.get(&owner)
                == Some(&crate::callable::CheckedCallContextualReceiverKind::LineContext) =>
        {
            Ok(super::CheckedExpressionExecutionPlan::structural(
                CheckedRuntimeValueDisposition::Omit,
                CheckedStructuralExecutionReason::ContextualCapability,
            ))
        }
        CheckedExpressionResolution::Value(CheckedValueResolution::CharacterField {
            field: crate::types::CharacterField::Stage,
            ..
        }) if contextual_receivers.get(&owner)
            == Some(&crate::callable::CheckedCallContextualReceiverKind::CharacterStage) =>
        {
            Ok(super::CheckedExpressionExecutionPlan::structural(
                CheckedRuntimeValueDisposition::Omit,
                CheckedStructuralExecutionReason::ContextualCapability,
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

fn contextual_receiver_sources(
    expressions: &BTreeMap<ExprId, super::CheckedExpression>,
    calls: &BTreeMap<ExprId, crate::callable::CallTargetFacts>,
) -> Result<
    BTreeMap<ExprId, crate::callable::CheckedCallContextualReceiverKind>,
    FinalSemanticAnalysisError,
> {
    use crate::callable::{
        CheckedCallArgumentSlotSource, CheckedCallContextualReceiverKind,
        CheckedCallReceiverProjection, CheckedCallSite,
    };

    let mut contextual = BTreeMap::new();
    for (call_owner, facts) in calls {
        let Some(application) = facts.selected_application() else {
            continue;
        };
        let CheckedCallReceiverProjection::Contextual { kind, source, ty } =
            application.core().execution().receiver()
        else {
            continue;
        };
        if application.core().site() != CheckedCallSite::HirCall(*call_owner)
            || source.raw() != CheckedCallArgumentSlotSource::Expression(source.owner())
            || !matches!(
                source.coordinate(),
                crate::semantic_coordinate::StableCheckedValueCoordinate::Expression(_)
            )
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let checked_source = expressions
            .get(&source.owner())
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let source_matches_kind = match (kind, ty, checked_source.resolution()) {
            (
                CheckedCallContextualReceiverKind::LineContext,
                crate::types::TypeKind::LineContext,
                super::CheckedExpressionResolution::Value(
                    super::CheckedValueResolution::LineContext,
                ),
            ) => checked_source.value_type() == Some(&crate::types::TypeKind::LineContext),
            (
                CheckedCallContextualReceiverKind::CharacterStage,
                crate::types::TypeKind::StageApi(expected_character),
                super::CheckedExpressionResolution::Value(
                    super::CheckedValueResolution::CharacterField {
                        character,
                        field: crate::types::CharacterField::Stage,
                        ..
                    },
                ),
            ) => expected_character == character && checked_source.value_type() == Some(ty),
            _ => false,
        };
        if !source_matches_kind || contextual.insert(source.owner(), *kind).is_some() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
    }
    Ok(contextual)
}
