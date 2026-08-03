//! Direct attached leaf-expression lowering into the final expression arena.
//!
//! This first expression slice consumes only parser-owned typed projections.
//! It never reopens source text, constructs a detached expression, or admits a
//! second leaf payload owner.

mod choice;
mod dialogue;

pub(crate) use dialogue::CandidateCursor;

use std::collections::BTreeSet;

use arcweft_lang_syntax::attachment::node::MissingExpressionKind;
use arcweft_lang_syntax::attachment::{
    AstNode, AttachedExpressionChild, AttachedExpressionNode, AttachedMatchArm,
};
use arcweft_lang_syntax::expressions::{
    ExpressionComponentRole, ExpressionProjection, SyntaxAssociatedCallSyntax,
    SyntaxAwaitPropagation, SyntaxBinaryOperator, SyntaxBorrowKind,
    SyntaxCallArgumentListTerminator, SyntaxCallArgumentProjection, SyntaxCallCalleeProjection,
    SyntaxCallProjection, SyntaxCallTypeApplicationSpelling, SyntaxCallTypeApplicationTerminator,
    SyntaxCallTypeArgumentProjection, SyntaxCallTypeChildRole, SyntaxComputationBlockKind,
    SyntaxExpressionSlot, SyntaxLifetimeRegistryPath, SyntaxLifetimeRegistryScope,
    SyntaxMatchArmPart, SyntaxMatchBodyTerminator, SyntaxMatchProjection, SyntaxNumericSequence,
    SyntaxNumericSequenceRecovery, SyntaxPlaceholderKind, SyntaxPostfixBracketProjection,
    SyntaxRecordField, SyntaxRequiredTokenState, SyntaxSelectedMember, SyntaxTryForm,
    SyntaxUnaryOperator,
};
use arcweft_lang_syntax::name::SyntaxNameIssue;

use crate::diagnostic::{HirRecoveryDiagnostic, HirRecoveryPrimary};
use crate::expr::{
    HirArrayRepeatExpr, HirAssociatedCallSyntax, HirAssociatedReceiver, HirAwaitExpr,
    HirAwaitPropagation, HirBinaryExpr, HirBinaryOp, HirBorrowExpr, HirBorrowKind,
    HirBracketSequenceExpr, HirCallArgument, HirCallArgumentListTerminator, HirCallArgumentOrdinal,
    HirCallBuildError, HirCallCallee, HirCallChildPoison, HirCallChildStates, HirCallExpr,
    HirCallTypeApplication, HirCallTypeApplicationSpelling, HirCallTypeApplicationTerminator,
    HirCallTypeArgument, HirCallTypeArgumentOrdinal, HirCallValue, HirClosureExpr,
    HirClosureParameter, HirComputationBlockExpr, HirComputationBlockKind, HirDereferenceExpr,
    HirExpr, HirExprError, HirExprKind, HirExpressionRecoveryIssue, HirGenericExprIssue, HirIfExpr,
    HirIfLetExpr, HirIndexExpr, HirMatchArm, HirMatchExpr, HirMatchRecoveryIssue,
    HirNamedBlockExpr, HirNamedBlockName, HirPipeExpr, HirPlaceholderKind, HirPoisonState,
    HirRangeExpr, HirRecordExpr, HirRecordField, HirRecordFieldIssue, HirRecordLiteralExpr,
    HirRecoveredName, HirRecoveryIssue, HirRequiredTokenState, HirSelectExpr, HirSelectedMember,
    HirTryExpr, HirTryForm, HirTupleExpr, HirUnaryExpr, HirUnaryOp, literal_recovery_issue,
};
use crate::identity::{ExprId, HirLimit, ScopeId, SyntheticKey, SyntheticOwner, SyntheticRole};
use crate::leaf::{
    HirIntegerLiteral, HirLifetimePathRecovery, HirLifetimePathValue, HirLifetimeRegistryIssue,
    HirLifetimeRegistryPath, HirLifetimeRegistryScope, HirNumericSequence,
    HirNumericSequenceElement, HirNumericSequenceRecovery, HirPathValue, HirShortVariantName,
};
use crate::lower::{HirInvariantFailure, HirLimitError, HirLowerFailure};
use crate::scope::{CaptureAccess, HirPatternBindingPolicy, HirScope, HirScopeKind, HirScopeOwner};
use crate::source_index::{
    HirExprSourceRole, HirMatchArmSourcePart, HirSourceQuery, HirSourceSite,
};
use crate::type_ref::HirTypeResolver;

use super::id_ref_projection::id_ref;
use super::literal_projection::{integer_issue, integer_literal, integer_suffix, literal};
use super::name_projection::{
    attempted_name_bytes, name, name_issue, recovered_name, require_attempted_name_limit,
};
use super::path_projection::{TypedPathProjection, project_attached_path, project_type_path};
use super::statement_lowering::OmittedValueTail;
use super::{StagedHirModuleTransaction, require_limit};

impl StagedHirModuleTransaction<'_> {
    /// Lowers one attached E01-E11 expression through the transaction's sole
    /// expression, slot, source, and diagnostic authority.
    pub(crate) fn lower_attached_expression(
        &mut self,
        attached: &AttachedExpressionNode,
        scope: ScopeId,
    ) -> Result<ExprId, HirLowerFailure> {
        let result = self.lower_attached_expression_inner(attached, scope);
        if result.is_err() {
            self.slots.poison();
        }
        result
    }

    fn lower_attached_expression_inner(
        &mut self,
        attached: &AttachedExpressionNode,
        scope: ScopeId,
    ) -> Result<ExprId, HirLowerFailure> {
        self.validate_attached_expression(attached, scope)?;
        self.preflight_expression(attached)?;
        let whole = attached.whole_source_span();
        let reservation = self.arenas.expressions().reserve_source(
            &mut self.slots,
            attached.id(),
            HirSourceSite::Span(whole.clone()),
        )?;
        let owner = reservation.id();
        if !reservation.is_first_touch() {
            return self.validate_reused_expression(owner, scope);
        }

        let (kind, state, parent_diagnostic_required) =
            self.project_expression(attached, owner, scope)?;
        let poisoned = state.is_poisoned();
        let recovery_primary = (poisoned && parent_diagnostic_required)
            .then(|| {
                recovery_diagnostic_primary(
                    self.request.source().document(),
                    attached,
                    &state,
                    whole.clone(),
                )
            })
            .transpose()?;
        let payload = HirExpr::try_new(scope, kind.clone(), state)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;

        self.source_components.stage_attached_expression(
            self.request.source(),
            owner,
            attached,
            &kind,
        )?;
        if let Some((role, site)) = recovery_primary {
            self.stage_recovery_diagnostic(HirRecoveryDiagnostic::new(
                SyntheticOwner::Expr(owner),
                HirRecoveryPrimary::query(HirSourceQuery::Expr { owner, role }),
                site,
            ));
        }

        self.arenas
            .expressions()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(HirLowerFailure::from)
    }

    fn validate_attached_expression(
        &self,
        attached: &AttachedExpressionNode,
        scope: ScopeId,
    ) -> Result<(), HirLowerFailure> {
        if attached.snapshot_id() != self.request.source().snapshot_id() {
            return Err(HirLowerFailure::StaleSource {
                current: self.request.source().snapshot_id().clone(),
                supplied: attached.snapshot_id().clone(),
            });
        }
        let source = attached.whole_source_span();
        if source.source() != self.request.source().document().identity() {
            return Err(HirLowerFailure::SourceIdentityMismatch {
                expected: self.request.source().document().identity().clone(),
                actual: source.source().clone(),
            });
        }
        if !HirTypeResolver::scope_is_live(self, scope) {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        Ok(())
    }

    pub(super) fn validate_reused_expression(
        &mut self,
        owner: ExprId,
        scope: ScopeId,
    ) -> Result<ExprId, HirLowerFailure> {
        let retained = self
            .arenas
            .expressions()
            .resolve_staged(&self.slots, owner)
            .map_err(HirLowerFailure::from)?;
        if retained.scope() == scope {
            Ok(owner)
        } else {
            Err(HirInvariantFailure::InvalidArenaCommit.into())
        }
    }

    fn project_expression(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        scope: ScopeId,
    ) -> Result<(HirExprKind, HirPoisonState, bool), HirLowerFailure> {
        let (kind, recovery) = match attached.projection() {
            ExpressionProjection::Unit => (HirExprKind::Unit, None),
            ExpressionProjection::Literal(value) => {
                let value = literal(value)?;
                let recovery =
                    literal_recovery_issue(&value).map(HirRecoveryIssue::MalformedLiteral);
                (HirExprKind::Literal(value), recovery)
            }
            ExpressionProjection::EntityReference(value) => {
                let value = id_ref(value)?;
                let recovery = value.recovery_issue().map(HirRecoveryIssue::InvalidId);
                (HirExprKind::EntityReference(value), recovery)
            }
            ExpressionProjection::LifetimePath(value) => {
                let value = project_lifetime_path(value)?;
                let recovery = value
                    .recovery()
                    .map(|recovery| HirRecoveryIssue::InvalidLifetimeRegistry(recovery.issue()));
                (HirExprKind::LifetimePath(value), recovery)
            }
            ExpressionProjection::Path => {
                let value = match (attached.path(), attached.nominal_path_type()) {
                    (Some(path), None) => match project_attached_path(path)? {
                        TypedPathProjection::Resolved(projected) => {
                            self.record_attached_path_capture(scope, path, &projected)?;
                            HirPathValue::Resolved(projected)
                        }
                        TypedPathProjection::Recovered(recovery) => {
                            HirPathValue::Recovered(recovery)
                        }
                    },
                    (None, Some(type_ref)) => HirPathValue::Resolved(project_type_path(
                        type_ref
                            .value()
                            .nominal_path()
                            .ok_or(HirInvariantFailure::InvalidArenaCommit)?,
                    )?),
                    _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
                };
                let recovery = value
                    .recovery()
                    .map(|recovery| HirRecoveryIssue::InvalidPath(recovery.issue().clone()));
                (HirExprKind::Path(value), recovery)
            }
            ExpressionProjection::ShortVariant(value) => {
                let value = match value {
                    Ok(value) => HirShortVariantName::Resolved(name(value)?),
                    Err(issue) => {
                        require_attempted_name_limit(issue)?;
                        HirShortVariantName::Recovered(name_issue(issue))
                    }
                };
                let recovery = value.recovery_issue().map(HirRecoveryIssue::InvalidName);
                (HirExprKind::ShortVariant(value), recovery)
            }
            ExpressionProjection::Placeholder(value) => (
                HirExprKind::Placeholder(match value {
                    SyntaxPlaceholderKind::PartialApplication => {
                        HirPlaceholderKind::PartialApplication
                    }
                    SyntaxPlaceholderKind::PipeLeft => HirPlaceholderKind::PipeLeft,
                }),
                None,
            ),
            ExpressionProjection::Tuple(_) => {
                let (elements, recovery) = self.lower_composite_children(attached, owner, scope)?;
                (HirExprKind::Tuple(HirTupleExpr::new(elements)), recovery)
            }
            ExpressionProjection::BracketSequence(_) => {
                let (elements, recovery) = self.lower_composite_children(attached, owner, scope)?;
                (
                    HirExprKind::BracketSequence(HirBracketSequenceExpr::new(elements)),
                    recovery,
                )
            }
            ExpressionProjection::NumericBracketSequence(sequence) => {
                let recovery = sequence
                    .has_recovery()
                    .then_some(HirRecoveryIssue::InvalidNumericSequence);
                (
                    HirExprKind::NumericBracketSequence(project_numeric_sequence(sequence)?),
                    recovery,
                )
            }
            ExpressionProjection::ArrayRepeat(_) => {
                let (children, recovery) = self.lower_composite_children(attached, owner, scope)?;
                let [value, length] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (
                    HirExprKind::ArrayRepeat(HirArrayRepeatExpr::new(*value, *length)),
                    recovery,
                )
            }
            ExpressionProjection::Call(call) => {
                let (call, recovery) = self.lower_call_expression(attached, owner, scope, call)?;
                (HirExprKind::Call(call), recovery)
            }
            ExpressionProjection::Select(member) => {
                let (children, mut recovery) =
                    self.lower_composite_children(attached, owner, scope)?;
                let [target] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                let member = match member {
                    SyntaxSelectedMember::Name(member) => HirSelectedMember::Name(name(member)?),
                    SyntaxSelectedMember::Missing => {
                        if recovery.is_none() {
                            recovery = Some(HirRecoveryIssue::MissingOperand {
                                role: HirExprSourceRole::SelectedMember,
                            });
                        }
                        HirSelectedMember::Missing
                    }
                };
                (
                    HirExprKind::Select(HirSelectExpr::new(*target, member)),
                    recovery,
                )
            }
            ExpressionProjection::Index(_) => {
                let (children, recovery) = self.lower_composite_children(attached, owner, scope)?;
                let [target, index] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (
                    HirExprKind::Index(HirIndexExpr::new(*target, *index)),
                    recovery,
                )
            }
            ExpressionProjection::DialogueContentApplication(application) => {
                let (application, recovery) =
                    self.lower_dialogue_content_application(attached, owner, scope, application)?;
                (
                    HirExprKind::DialogueContentApplication(application),
                    recovery,
                )
            }
            ExpressionProjection::PostfixBracket(postfix) => {
                let (postfix, recovery) =
                    self.lower_postfix_bracket(attached, owner, scope, postfix)?;
                (HirExprKind::PostfixBracket(postfix), recovery)
            }
            ExpressionProjection::Pipe(_) => {
                let (children, recovery) = self.lower_composite_children(attached, owner, scope)?;
                let [left, right] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (HirExprKind::Pipe(HirPipeExpr::new(*left, *right)), recovery)
            }
            ExpressionProjection::Try { form, .. } => {
                let (children, recovery) = self.lower_composite_children(attached, owner, scope)?;
                let [operand] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (
                    HirExprKind::Try(HirTryExpr::new(*operand, try_form(*form))),
                    recovery,
                )
            }
            ExpressionProjection::Await { propagation, .. } => {
                let (children, recovery) = self.lower_composite_children(attached, owner, scope)?;
                let [operand] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (
                    HirExprKind::Await(HirAwaitExpr::new(
                        *operand,
                        await_propagation(*propagation),
                    )),
                    recovery,
                )
            }
            ExpressionProjection::Thread(thread) => {
                let (thread, recovery) =
                    self.lower_attached_thread_expression(attached, owner, scope, thread)?;
                (
                    HirExprKind::Thread(thread),
                    recovery.map(HirRecoveryIssue::InvalidThread),
                )
            }
            ExpressionProjection::Choice => {
                let (choice, recovery) =
                    self.lower_attached_choice_expression(attached, owner, scope)?;
                (HirExprKind::Choice(choice), recovery)
            }
            ExpressionProjection::Range { inclusive, .. } => {
                let (start, end, recovery) = self.lower_range_children(attached, owner, scope)?;
                (
                    HirExprKind::Range(HirRangeExpr::new(start, end, *inclusive)),
                    recovery,
                )
            }
            ExpressionProjection::Record(fields) => {
                let path = attached
                    .path()
                    .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                let TypedPathProjection::Resolved(path) = project_attached_path(path)? else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                let (fields, recovery) =
                    self.lower_record_fields(attached, owner, scope, fields)?;
                (
                    HirExprKind::Record(HirRecordExpr::new(path, fields)),
                    recovery,
                )
            }
            ExpressionProjection::RecordLiteral(fields) => {
                let (fields, recovery) =
                    self.lower_record_fields(attached, owner, scope, fields)?;
                (
                    HirExprKind::RecordLiteral(HirRecordLiteralExpr::new(fields)),
                    recovery,
                )
            }
            ExpressionProjection::Binary { operator, .. } => {
                let (children, recovery) = self.lower_composite_children(attached, owner, scope)?;
                let [left, right] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (
                    HirExprKind::Binary(HirBinaryExpr::new(
                        *left,
                        binary_operator(*operator),
                        *right,
                    )),
                    recovery,
                )
            }
            ExpressionProjection::If { else_branch, .. } => {
                let (children, recovery) = self.lower_composite_children(attached, owner, scope)?;
                let (condition, then_branch, else_branch) = match (children.as_ref(), else_branch) {
                    ([condition, then_branch, else_branch], Some(_)) => {
                        (*condition, *then_branch, *else_branch)
                    }
                    ([condition, then_branch], None) => {
                        let source = attached
                            .component(ExpressionComponentRole::ElseBranch)
                            .ok_or(HirInvariantFailure::InvalidSourceSpan)?;
                        let else_branch = self.lower_implicit_unit_tail(owner, scope, source)?;
                        (*condition, *then_branch, else_branch)
                    }
                    _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
                };
                (
                    HirExprKind::If(HirIfExpr::new(condition, then_branch, else_branch)),
                    recovery,
                )
            }
            ExpressionProjection::IfLet { .. } => {
                let (if_let, recovery) = self.lower_if_let_expression(attached, owner, scope)?;
                (HirExprKind::IfLet(if_let), recovery)
            }
            ExpressionProjection::Match(projection) => {
                let (match_expression, recovery) =
                    self.lower_match_expression(attached, owner, scope, projection)?;
                (HirExprKind::Match(match_expression), recovery)
            }
            ExpressionProjection::Closure(_) => {
                let (closure, recovery) = self.lower_closure_expression(attached, owner, scope)?;
                (HirExprKind::Closure(closure), recovery)
            }
            ExpressionProjection::Borrow { kind, .. } => {
                let (children, recovery) = self.lower_composite_children(attached, owner, scope)?;
                let [operand] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (
                    HirExprKind::Borrow(HirBorrowExpr::new(borrow_kind(*kind), *operand)),
                    recovery,
                )
            }
            ExpressionProjection::Dereference { .. } => {
                let (children, recovery) = self.lower_composite_children(attached, owner, scope)?;
                let [operand] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (
                    HirExprKind::Dereference(HirDereferenceExpr::new(*operand)),
                    recovery,
                )
            }
            ExpressionProjection::Unary { operator, .. } => {
                let (children, recovery) = self.lower_composite_children(attached, owner, scope)?;
                let [operand] = children.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                (
                    HirExprKind::Unary(HirUnaryExpr::new(unary_operator(*operator), *operand)),
                    recovery,
                )
            }
            ExpressionProjection::Block => {
                let (block, recovery) = self.lower_attached_value_block(attached, owner, scope)?;
                (HirExprKind::Block(block), recovery)
            }
            ExpressionProjection::ComputationBlock(kind) => {
                let (kind, omitted_tail) = match kind {
                    SyntaxComputationBlockKind::Result => (
                        HirComputationBlockKind::Result,
                        OmittedValueTail::MissingRequired,
                    ),
                    SyntaxComputationBlockKind::Task => (
                        HirComputationBlockKind::Task,
                        OmittedValueTail::MissingRequired,
                    ),
                    SyntaxComputationBlockKind::Seq => {
                        (HirComputationBlockKind::Seq, OmittedValueTail::ImplicitUnit)
                    }
                    SyntaxComputationBlockKind::Stream => (
                        HirComputationBlockKind::Stream,
                        OmittedValueTail::ImplicitUnit,
                    ),
                };
                let block =
                    self.lower_attached_value_block_parts(attached, owner, scope, omitted_tail)?;
                (
                    HirExprKind::ComputationBlock(HirComputationBlockExpr::new(
                        kind,
                        block.scope,
                        block.statements,
                        block.tail,
                    )),
                    block.recovery,
                )
            }
            ExpressionProjection::NamedBlock(source_name) => {
                let (name, name_recovery) = match source_name {
                    Ok(source_name) => (HirNamedBlockName::Resolved(name(source_name)?), None),
                    Err(SyntaxNameIssue::Missing) => {
                        return Err(HirInvariantFailure::InvalidArenaCommit.into());
                    }
                    Err(issue) => {
                        require_attempted_name_limit(issue)?;
                        let issue = name_issue(issue);
                        (
                            HirNamedBlockName::InvalidPresent(issue),
                            Some(HirRecoveryIssue::InvalidName(issue)),
                        )
                    }
                };
                let block = self.lower_attached_value_block_parts(
                    attached,
                    owner,
                    scope,
                    OmittedValueTail::ImplicitUnit,
                )?;
                (
                    HirExprKind::NamedBlock(HirNamedBlockExpr::new(
                        name,
                        block.scope,
                        block.statements,
                        block.tail,
                    )),
                    name_recovery.or(block.recovery),
                )
            }
            ExpressionProjection::Error => {
                self.lower_error_recovery_prefix(attached, scope)?;
                (
                    HirExprKind::Error(HirExprError::new(HirGenericExprIssue::UnclassifiedSyntax)),
                    Some(HirRecoveryIssue::InvalidExpression(
                        HirExpressionRecoveryIssue::Generic(
                            HirGenericExprIssue::UnclassifiedSyntax,
                        ),
                    )),
                )
            }
        };
        let parent_diagnostic_required = match attached.projection() {
            ExpressionProjection::Select(member) => {
                matches!(member, SyntaxSelectedMember::Missing)
            }
            // E33 represents missing content and a missing closing bracket on
            // the application owner itself. It deliberately does not mint a
            // synthetic content expression, so that owner must publish the
            // recovery diagnostic directly.
            ExpressionProjection::DialogueContentApplication(_) => true,
            _ => !matches!(
                &recovery,
                Some(
                    HirRecoveryIssue::MissingOperand { .. } | HirRecoveryIssue::MissingRequiredTail
                )
            ),
        };
        Ok((
            kind,
            recovery.map_or(HirPoisonState::Clean, HirPoisonState::Poisoned),
            parent_diagnostic_required,
        ))
    }

    fn preflight_expression(
        &self,
        attached: &AttachedExpressionNode,
    ) -> Result<(), HirLowerFailure> {
        for child in attached.children() {
            if child.missing().is_some()
                && !SyntheticRole::RecoveryOperand.accepts_ordinal(child.ordinal())
            {
                return Err(HirInvariantFailure::InvalidSlotCommit.into());
            }
        }
        if let ExpressionProjection::DialogueContentApplication(application) = attached.projection()
        {
            return self.preflight_dialogue_content_application(attached, application);
        }
        if let ExpressionProjection::PostfixBracket(postfix) = attached.projection() {
            return self.preflight_postfix_bracket(attached, postfix);
        }
        if let ExpressionProjection::Call(call) = attached.projection() {
            match call {
                SyntaxCallProjection::Parenthesized(call) => {
                    require_limit(HirLimit::CallArguments, call.arguments().len())?;
                    if let Some(application) = call.explicit_type_application() {
                        require_limit(HirLimit::CallTypeArguments, application.arguments().len())?;
                    }
                }
                SyntaxCallProjection::CallbackBlock(_) => {
                    require_limit(HirLimit::CallArguments, 1)?;
                }
            }
            return Ok(());
        }
        if let ExpressionProjection::Match(expression) = attached.projection() {
            return require_match_arm_scope_limit(expression.arms().len());
        }
        let ExpressionProjection::NumericBracketSequence(sequence) = attached.projection() else {
            return Ok(());
        };
        require_limit(
            HirLimit::NumericSequenceElements,
            sequence.source_element_count(),
        )?;
        for element in sequence.elements() {
            require_limit(HirLimit::NumericDigitsPerLiteral, element.digit_count())?;
        }
        if let SyntaxNumericSequenceRecovery::InvalidElement { digit_count, .. } =
            sequence.recovery()
        {
            require_limit(HirLimit::NumericDigitsPerLiteral, *digit_count)?;
        }
        require_limit(
            HirLimit::NumericSequenceTotalDigits,
            sequence.total_digit_count(),
        )
    }

    fn lower_call_expression(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        scope: ScopeId,
        projection: &SyntaxCallProjection,
    ) -> Result<(HirCallExpr, Option<HirRecoveryIssue>), HirLowerFailure> {
        if let SyntaxCallProjection::CallbackBlock(callback) = projection {
            if !attached.call_type_children().is_empty() {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let callee = self.lower_call_value_receiver(attached, scope)?;
            let callee_state = if self.staged_expression_is_poisoned(callee)? {
                HirCallChildPoison::Poisoned
            } else {
                HirCallChildPoison::Clean
            };
            let callback_child = self.expression_child(attached, 1, callback.callback())?;
            let argument_role = HirExprSourceRole::CallArgument {
                argument: HirCallArgumentOrdinal::try_new(0)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                part: crate::source_index::HirCallArgumentSourcePart::Value,
            };
            let (value, argument_state) = if let Some(semantic) = callback_child
                .authored_semantic()
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            {
                let value = self.lower_attached_expression_inner(&semantic, scope)?;
                let state = if self.staged_expression_is_poisoned(value)? {
                    HirCallChildPoison::Poisoned
                } else {
                    HirCallChildPoison::Clean
                };
                (HirCallValue::Present { value }, state)
            } else {
                let recovery =
                    self.lower_missing_expression(owner, scope, callback_child, argument_role)?;
                (
                    HirCallValue::Missing { recovery },
                    HirCallChildPoison::Poisoned,
                )
            };
            let argument_states = [argument_state];
            let (call, state) = HirCallExpr::try_new(
                HirCallCallee::value(callee),
                HirCallTypeApplication::absent(),
                Box::new([HirCallArgument::Positional { value }]),
                match callback.terminator() {
                    SyntaxCallArgumentListTerminator::Closed => {
                        HirCallArgumentListTerminator::Closed
                    }
                    SyntaxCallArgumentListTerminator::RecoveredMissing => {
                        HirCallArgumentListTerminator::RecoveredMissing
                    }
                },
                HirCallChildStates::new(callee_state, &argument_states, &[]),
                false,
            )
            .map_err(|error| match error {
                HirCallBuildError::LimitExceeded { limit, observed } => HirLowerFailure::Limit(
                    HirLimitError::with_maximum(limit, observed, limit.maximum()),
                ),
                HirCallBuildError::ChildStateShapeMismatch
                | HirCallBuildError::ChildIdentityMismatch => {
                    HirInvariantFailure::InvalidArenaCommit.into()
                }
            })?;
            let recovery = match state {
                HirPoisonState::Clean => None,
                HirPoisonState::Poisoned(issue) => Some(issue),
            };
            return Ok((call, recovery));
        }
        let SyntaxCallProjection::Parenthesized(call) = projection else {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        };
        let (callee, callee_state) = match call.callee() {
            SyntaxCallCalleeProjection::Ordinary => {
                if attached.call_type_children().iter().any(|child| {
                    !matches!(
                        child.role(),
                        SyntaxCallTypeChildRole::ExplicitCallTypeArgument { .. }
                    )
                }) {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                }
                let callee = self.lower_call_value_receiver(attached, scope)?;
                let state = if self.staged_expression_is_poisoned(callee)? {
                    HirCallChildPoison::Poisoned
                } else {
                    HirCallChildPoison::Clean
                };
                (HirCallCallee::value(callee), state)
            }
            SyntaxCallCalleeProjection::UnresolvedDot { member } => {
                let value_receiver = self.lower_call_value_receiver(attached, scope)?;
                let value_state = if self.staged_expression_is_poisoned(value_receiver)? {
                    HirCallChildPoison::Poisoned
                } else {
                    HirCallChildPoison::Clean
                };
                let nominal_receiver = self.lower_call_associated_receiver(
                    attached,
                    scope,
                    SyntaxCallTypeChildRole::DotNominalReceiver,
                )?;
                (
                    HirCallCallee::unresolved_dot(
                        value_receiver,
                        nominal_receiver,
                        recovered_name(member)?,
                    ),
                    value_state,
                )
            }
            SyntaxCallCalleeProjection::Associated { syntax, member } => {
                let receiver = self.lower_call_associated_receiver(
                    attached,
                    scope,
                    SyntaxCallTypeChildRole::AssociatedReceiver,
                )?;
                let syntax = match syntax {
                    SyntaxAssociatedCallSyntax::DotFallback => HirAssociatedCallSyntax::DotFallback,
                    SyntaxAssociatedCallSyntax::ExplicitDoubleColon => {
                        HirAssociatedCallSyntax::ExplicitDoubleColon
                    }
                };
                (
                    HirCallCallee::associated(receiver, recovered_name(member)?, syntax),
                    HirCallChildPoison::Clean,
                )
            }
        };

        let mut type_argument_states = Vec::new();
        let explicit_type_application = match call.explicit_type_application() {
            None => HirCallTypeApplication::absent(),
            Some(application) => {
                let mut arguments = Vec::with_capacity(application.arguments().len());
                for (position, projection) in application.arguments().iter().enumerate() {
                    HirCallTypeArgumentOrdinal::try_new(position)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                    if matches!(projection, SyntaxCallTypeArgumentProjection::Missing) {
                        arguments.push(HirCallTypeArgument::Missing);
                        continue;
                    }
                    let syntax_ordinal = u16::try_from(position)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                    let child = attached
                        .call_type_children()
                        .iter()
                        .find(|child| {
                            child.role()
                                == SyntaxCallTypeChildRole::ExplicitCallTypeArgument {
                                    ordinal: syntax_ordinal,
                                }
                        })
                        .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                    let ty = self.lower_attached_type(child.node(), scope)?;
                    let poisoned = self
                        .arenas
                        .types()
                        .resolve_staged(&self.slots, ty)
                        .map_err(HirLowerFailure::from)?
                        .is_poisoned();
                    let expected_poison =
                        matches!(projection, SyntaxCallTypeArgumentProjection::InvalidPresent);
                    if poisoned != expected_poison {
                        return Err(HirInvariantFailure::InvalidArenaCommit.into());
                    }
                    let state = if poisoned {
                        HirCallChildPoison::Poisoned
                    } else {
                        HirCallChildPoison::Clean
                    };
                    type_argument_states.push(state);
                    arguments.push(if poisoned {
                        HirCallTypeArgument::InvalidPresent { poisoned: ty }
                    } else {
                        HirCallTypeArgument::Resolved { ty }
                    });
                }
                HirCallTypeApplication::present(
                    match application.spelling() {
                        SyntaxCallTypeApplicationSpelling::DirectAngle => {
                            HirCallTypeApplicationSpelling::DirectAngle
                        }
                        SyntaxCallTypeApplicationSpelling::Turbofish => {
                            HirCallTypeApplicationSpelling::Turbofish
                        }
                    },
                    arguments.into_boxed_slice(),
                    match application.terminator() {
                        SyntaxCallTypeApplicationTerminator::Closed => {
                            HirCallTypeApplicationTerminator::Closed
                        }
                        SyntaxCallTypeApplicationTerminator::RecoveredMissing => {
                            HirCallTypeApplicationTerminator::RecoveredMissing
                        }
                        SyntaxCallTypeApplicationTerminator::InvalidPresent => {
                            HirCallTypeApplicationTerminator::InvalidPresent
                        }
                    },
                )
            }
        };

        let mut arguments = Vec::with_capacity(call.arguments().len());
        let mut argument_states = Vec::with_capacity(call.arguments().len());
        for (index, source_argument) in call.arguments().iter().enumerate() {
            let argument = HirCallArgumentOrdinal::try_new(index)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            let child_ordinal = u32::try_from(index)
                .ok()
                .and_then(|index| index.checked_add(1))
                .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
            let child = self.expression_child(attached, child_ordinal, source_argument.value())?;
            let role = HirExprSourceRole::CallArgument {
                argument,
                part: crate::source_index::HirCallArgumentSourcePart::Value,
            };
            let (value, child_state) = if let Some(semantic) = child
                .authored_semantic()
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            {
                let value = self.lower_attached_expression_inner(&semantic, scope)?;
                let state = if self.staged_expression_is_poisoned(value)? {
                    HirCallChildPoison::Poisoned
                } else {
                    HirCallChildPoison::Clean
                };
                (HirCallValue::Present { value }, state)
            } else {
                let recovery = self.lower_missing_expression(owner, scope, child, role)?;
                (
                    HirCallValue::Missing { recovery },
                    HirCallChildPoison::Poisoned,
                )
            };
            argument_states.push(child_state);
            arguments.push(match source_argument {
                SyntaxCallArgumentProjection::Positional { .. } => {
                    HirCallArgument::Positional { value }
                }
                SyntaxCallArgumentProjection::Named {
                    name: source_name,
                    equals,
                    ..
                } => {
                    let call_name = match source_name {
                        Ok(source_name) => HirRecoveredName::Valid(name(source_name)?),
                        Err(SyntaxNameIssue::Missing) => HirRecoveredName::Missing,
                        Err(issue) => {
                            require_attempted_name_limit(issue)?;
                            HirRecoveredName::InvalidPresent
                        }
                    };
                    HirCallArgument::Named {
                        name: call_name,
                        equals: match equals {
                            SyntaxRequiredTokenState::Present => HirRequiredTokenState::Present,
                            SyntaxRequiredTokenState::Missing => HirRequiredTokenState::Missing,
                            SyntaxRequiredTokenState::InvalidPresent => {
                                HirRequiredTokenState::InvalidPresent
                            }
                        },
                        value,
                    }
                }
                SyntaxCallArgumentProjection::Spread { ellipsis, .. } => HirCallArgument::Spread {
                    value,
                    ellipsis: match ellipsis {
                        SyntaxRequiredTokenState::Present => HirRequiredTokenState::Present,
                        SyntaxRequiredTokenState::Missing => HirRequiredTokenState::Missing,
                        SyntaxRequiredTokenState::InvalidPresent => {
                            HirRequiredTokenState::InvalidPresent
                        }
                    },
                },
            });
        }

        let (call, state) = HirCallExpr::try_new(
            callee,
            explicit_type_application,
            arguments.into_boxed_slice(),
            match call.terminator() {
                SyntaxCallArgumentListTerminator::Closed => HirCallArgumentListTerminator::Closed,
                SyntaxCallArgumentListTerminator::RecoveredMissing => {
                    HirCallArgumentListTerminator::RecoveredMissing
                }
            },
            HirCallChildStates::new(callee_state, &argument_states, &type_argument_states),
            false,
        )
        .map_err(|error| match error {
            HirCallBuildError::LimitExceeded { limit, observed } => HirLowerFailure::Limit(
                HirLimitError::with_maximum(limit, observed, limit.maximum()),
            ),
            HirCallBuildError::ChildStateShapeMismatch
            | HirCallBuildError::ChildIdentityMismatch => {
                HirInvariantFailure::InvalidArenaCommit.into()
            }
        })?;
        let recovery = match state {
            HirPoisonState::Clean => None,
            HirPoisonState::Poisoned(issue) => Some(issue),
        };
        Ok((call, recovery))
    }

    fn lower_call_value_receiver(
        &mut self,
        attached: &AttachedExpressionNode,
        scope: ScopeId,
    ) -> Result<ExprId, HirLowerFailure> {
        let callee_child = self.expression_child(attached, 0, SyntaxExpressionSlot::Authored)?;
        let callee = callee_child
            .authored_semantic()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        self.lower_attached_expression_inner(&callee, scope)
    }

    fn lower_call_associated_receiver(
        &mut self,
        attached: &AttachedExpressionNode,
        scope: ScopeId,
        expected_role: SyntaxCallTypeChildRole,
    ) -> Result<HirAssociatedReceiver, HirLowerFailure> {
        let receiver = attached
            .call_type_children()
            .iter()
            .find(|child| child.role() == expected_role)
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let receiver = self.lower_attached_type(receiver.node(), scope)?;
        let poisoned = self
            .arenas
            .types()
            .resolve_staged(&self.slots, receiver)
            .map_err(HirLowerFailure::from)?
            .is_poisoned();
        Ok(if poisoned {
            HirAssociatedReceiver::invalid_present(receiver)
        } else {
            HirAssociatedReceiver::resolved(receiver)
        })
    }

    fn lower_composite_children(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        scope: ScopeId,
    ) -> Result<(Box<[ExprId]>, Option<HirRecoveryIssue>), HirLowerFailure> {
        let mut children = Vec::with_capacity(attached.children().len());
        let mut recovery = None;
        for child in attached.children() {
            let role = composite_child_role(attached.projection(), child.ordinal())
                .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
            if let Some(semantic) = child
                .authored_semantic()
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            {
                let lowered = self.lower_attached_expression_inner(&semantic, scope)?;
                if self.staged_expression_is_poisoned(lowered)? {
                    recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                        HirExpressionRecoveryIssue::RecoveredChild { role },
                    ));
                }
                children.push(lowered);
            } else {
                children.push(self.lower_missing_expression(owner, scope, child, role)?);
                recovery.get_or_insert(HirRecoveryIssue::MissingOperand { role });
            }
        }
        Ok((children.into_boxed_slice(), recovery))
    }

    fn lower_error_recovery_prefix(
        &mut self,
        attached: &AttachedExpressionNode,
        scope: ScopeId,
    ) -> Result<(), HirLowerFailure> {
        match attached.children() {
            [] => Ok(()),
            [prefix] if prefix.ordinal() == 0 && prefix.missing().is_none() => {
                let semantic = prefix
                    .authored_semantic()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                self.lower_attached_expression_inner(&semantic, scope)?;
                Ok(())
            }
            _ => Err(HirInvariantFailure::InvalidArenaCommit.into()),
        }
    }

    fn lower_closure_expression(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        outer_scope: ScopeId,
    ) -> Result<(HirClosureExpr, Option<HirRecoveryIssue>), HirLowerFailure> {
        let ExpressionProjection::Closure(projection) = attached.projection() else {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        };
        if projection.parameters().len() != attached.closure_parameters().len()
            || projection.has_result_type() != attached.closure_result_type().is_some()
        {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }

        let closure_scope =
            self.allocate_expression_scope(attached, owner, outer_scope, HirScopeKind::Closure)?;
        let mut parameters = Vec::with_capacity(projection.parameters().len());
        let mut locals = Vec::new();
        let mut recovery = None;
        for (source, attached_parameter) in projection
            .parameters()
            .iter()
            .zip(attached.closure_parameters())
        {
            if source.has_type() != attached_parameter.ty().is_some() {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let lowered_pattern = self.lower_attached_pattern_binding(
                attached_parameter.pattern(),
                closure_scope,
                HirPatternBindingPolicy::ClosureParameter,
            )?;
            match self
                .arenas
                .patterns()
                .resolve_staged(&self.slots, lowered_pattern.owner)?
                .state()
            {
                HirPoisonState::Clean => {}
                HirPoisonState::Poisoned(issue) => {
                    recovery.get_or_insert_with(|| issue.clone());
                }
            }
            let ty = attached_parameter
                .ty()
                .map(|attached_type| self.lower_attached_type(attached_type, closure_scope))
                .transpose()?;
            if let Some(ty) = ty {
                if let HirPoisonState::Poisoned(issue) =
                    self.arenas.types().resolve_staged(&self.slots, ty)?.state()
                {
                    recovery.get_or_insert_with(|| issue.clone());
                }
            }
            parameters.push(
                HirClosureParameter::try_new(lowered_pattern.owner, ty, closure_scope)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            );
            locals.extend(lowered_pattern.locals);
        }

        let result_type = attached
            .closure_result_type()
            .map(|attached_type| self.lower_attached_type(attached_type, closure_scope))
            .transpose()?;
        if let Some(result_type) = result_type {
            if let HirPoisonState::Poisoned(issue) = self
                .arenas
                .types()
                .resolve_staged(&self.slots, result_type)?
                .state()
            {
                recovery.get_or_insert_with(|| issue.clone());
            }
        }
        self.close_scope_members(closure_scope, locals.into_boxed_slice())?;

        self.begin_closure_captures(owner, closure_scope)?;
        let body_child = self.expression_child(attached, 0, projection.body())?;
        let body = if let Some(semantic) = body_child
            .authored_semantic()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
        {
            let body = self.lower_attached_expression_inner(&semantic, closure_scope)?;
            if self.staged_expression_is_poisoned(body)? {
                recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::RecoveredChild {
                        role: HirExprSourceRole::Body,
                    },
                ));
            }
            body
        } else {
            recovery.get_or_insert(HirRecoveryIssue::MissingRequiredTail);
            self.lower_missing_required_tail(owner, closure_scope, body_child.source_span())?
        };
        let captures = self.finish_closure_captures(owner)?;

        Ok((
            HirClosureExpr::new(
                closure_scope,
                parameters.into_boxed_slice(),
                result_type,
                body,
                captures,
            ),
            recovery,
        ))
    }

    fn lower_if_let_expression(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        outer_scope: ScopeId,
    ) -> Result<(HirIfLetExpr, Option<HirRecoveryIssue>), HirLowerFailure> {
        let ExpressionProjection::IfLet {
            scrutinee,
            guard,
            then_branch,
            else_branch,
        } = attached.projection()
        else {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        };
        let binding_scope = self.allocate_expression_scope(
            attached,
            owner,
            outer_scope,
            HirScopeKind::Conditional,
        )?;
        let attached_pattern = attached
            .pattern()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let lowered_pattern = self.lower_attached_pattern_binding(
            attached_pattern,
            binding_scope,
            HirPatternBindingPolicy::PatternBinding,
        )?;
        let pattern = self
            .arenas
            .patterns()
            .resolve_staged(&self.slots, lowered_pattern.owner)?;
        let mut recovery = match pattern.state() {
            HirPoisonState::Clean => None,
            HirPoisonState::Poisoned(issue) => Some(issue.clone()),
        };

        let scrutinee_child = self.expression_child(attached, 0, *scrutinee)?;
        let scrutinee = self.lower_if_let_operand(
            owner,
            scrutinee_child,
            outer_scope,
            HirExprSourceRole::Scrutinee,
            &mut recovery,
        )?;
        let guard = if let Some(expected) = guard {
            let child = self.expression_child(attached, 1, *expected)?;
            Some(self.lower_if_let_operand(
                owner,
                child,
                binding_scope,
                HirExprSourceRole::Guard,
                &mut recovery,
            )?)
        } else {
            None
        };
        let then_child = self.expression_child(attached, 2, *then_branch)?;
        let then_branch = self.lower_if_let_operand(
            owner,
            then_child,
            binding_scope,
            HirExprSourceRole::ThenBranch,
            &mut recovery,
        )?;
        let else_branch = match else_branch {
            Some(expected) => {
                let child = self.expression_child(attached, 3, *expected)?;
                if let Some(semantic) = child
                    .authored_semantic()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                {
                    let lowered = self.lower_attached_expression_inner(&semantic, outer_scope)?;
                    if self.staged_expression_is_poisoned(lowered)? {
                        recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                            HirExpressionRecoveryIssue::RecoveredChild {
                                role: HirExprSourceRole::ElseBranch,
                            },
                        ));
                    }
                    lowered
                } else {
                    recovery.get_or_insert(HirRecoveryIssue::MissingRequiredTail);
                    self.lower_missing_required_tail(owner, outer_scope, child.source_span())?
                }
            }
            None => {
                let source = attached
                    .component(ExpressionComponentRole::ElseBranch)
                    .ok_or(HirInvariantFailure::InvalidSourceSpan)?;
                recovery.get_or_insert(HirRecoveryIssue::MissingRequiredTail);
                self.lower_missing_required_tail(owner, outer_scope, source)?
            }
        };

        self.close_scope_members(binding_scope, lowered_pattern.locals)?;
        Ok((
            HirIfLetExpr::new(
                binding_scope,
                lowered_pattern.owner,
                scrutinee,
                guard,
                then_branch,
                else_branch,
            ),
            recovery,
        ))
    }

    fn lower_match_expression(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        outer_scope: ScopeId,
        projection: &SyntaxMatchProjection,
    ) -> Result<(HirMatchExpr, Option<HirRecoveryIssue>), HirLowerFailure> {
        if attached.match_arms().len() != projection.arms().len() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }

        let scrutinee_child = self.expression_child(attached, 0, projection.scrutinee())?;
        let mut recovery = None;
        let scrutinee = self.lower_if_let_operand(
            owner,
            scrutinee_child,
            outer_scope,
            HirExprSourceRole::Scrutinee,
            &mut recovery,
        )?;

        let mut arms = Vec::with_capacity(projection.arms().len());
        for (arm_index, (source_arm, attached_arm)) in projection
            .arms()
            .iter()
            .zip(attached.match_arms())
            .enumerate()
        {
            let arm =
                u32::try_from(arm_index).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            let arm_scope = self.allocate_match_arm_scope(attached_arm, owner, outer_scope)?;
            let lowered_pattern = self.lower_attached_pattern_binding(
                attached_arm.pattern(),
                arm_scope,
                HirPatternBindingPolicy::MatchBinding,
            )?;
            if self
                .arenas
                .patterns()
                .resolve_staged(&self.slots, lowered_pattern.owner)?
                .is_poisoned()
            {
                recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::RecoveredChild {
                        role: HirExprSourceRole::MatchArm {
                            arm,
                            part: HirMatchArmSourcePart::Pattern,
                        },
                    },
                ));
            }

            let guard_role = HirExprSourceRole::MatchArm {
                arm,
                part: HirMatchArmSourcePart::Guard,
            };
            let guard = match (source_arm.guard(), attached_arm.guard()) {
                (None, None) => None,
                (Some(SyntaxExpressionSlot::Authored), Some(attached_guard)) => {
                    let semantic = attached_guard
                        .authored_semantic()
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                        .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                    let guard = self.lower_attached_expression_inner(&semantic, arm_scope)?;
                    if self.staged_expression_is_poisoned(guard)? {
                        recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                            HirExpressionRecoveryIssue::RecoveredChild { role: guard_role },
                        ));
                    }
                    Some(guard)
                }
                (Some(SyntaxExpressionSlot::Missing), Some(attached_guard)) => {
                    if attached_guard
                        .authored_semantic()
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                        .is_some()
                        || attached_guard.missing().is_none()
                    {
                        return Err(HirInvariantFailure::InvalidArenaCommit.into());
                    }
                    recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                        HirExpressionRecoveryIssue::RecoveredChild { role: guard_role },
                    ));
                    None
                }
                _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
            };

            if !matches!(source_arm.arrow(), SyntaxRequiredTokenState::Present) {
                recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::RecoveredChild {
                        role: HirExprSourceRole::MatchArm {
                            arm,
                            part: HirMatchArmSourcePart::Arrow,
                        },
                    },
                ));
            }

            let value_role = HirExprSourceRole::MatchArm {
                arm,
                part: HirMatchArmSourcePart::Value,
            };
            let attached_value = attached_arm.value();
            let value = match source_arm.value() {
                SyntaxExpressionSlot::Authored => {
                    let semantic = attached_value
                        .authored_semantic()
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                        .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                    let value = self.lower_attached_expression_inner(&semantic, arm_scope)?;
                    if self.staged_expression_is_poisoned(value)? {
                        recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                            HirExpressionRecoveryIssue::RecoveredChild { role: value_role },
                        ));
                    }
                    value
                }
                SyntaxExpressionSlot::Missing => {
                    if attached_value
                        .authored_semantic()
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                        .is_some()
                        || attached_value.missing().is_none()
                    {
                        return Err(HirInvariantFailure::InvalidArenaCommit.into());
                    }
                    recovery.get_or_insert(HirRecoveryIssue::MissingRequiredTail);
                    self.lower_missing_required_tail_for_scope(
                        arm_scope,
                        attached_value.source_span(),
                    )?
                }
            };

            self.close_scope_members(arm_scope, lowered_pattern.locals.clone())?;
            arms.push(
                HirMatchArm::try_new(
                    arm_scope,
                    lowered_pattern.owner,
                    guard,
                    value,
                    lowered_pattern.locals,
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            );
        }

        match projection.terminator() {
            SyntaxMatchBodyTerminator::Closed => {}
            SyntaxMatchBodyTerminator::MissingBody => {
                recovery.get_or_insert(HirRecoveryIssue::InvalidMatch(
                    HirMatchRecoveryIssue::MissingBody,
                ));
            }
            SyntaxMatchBodyTerminator::RecoveredMissingClose => {
                recovery.get_or_insert(HirRecoveryIssue::InvalidMatch(
                    HirMatchRecoveryIssue::UnclosedBody,
                ));
            }
        }

        let expression = HirMatchExpr::try_new(scrutinee, arms.into_boxed_slice())
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok((expression, recovery))
    }

    fn allocate_match_arm_scope(
        &mut self,
        attached: &AttachedMatchArm,
        owner: ExprId,
        parent: ScopeId,
    ) -> Result<ScopeId, HirLowerFailure> {
        let reservation = self.arenas.scopes().reserve_source(
            &mut self.slots,
            attached.id(),
            HirSourceSite::Span(attached.whole_source_span()),
        )?;
        let scope = reservation.id();
        if reservation.is_first_touch() {
            let payload = HirScope::try_new(
                owner.module(),
                HirScopeKind::MatchArm,
                Some(parent),
                HirScopeOwner::Expr(owner),
                Box::new([]),
                Box::new([]),
            )
            .map_err(|_| HirInvariantFailure::InvalidScopeParent)?;
            self.arenas
                .scopes()
                .finalize(&mut self.slots, reservation, payload)?;
            self.append_scope_child(parent, scope)?;
            return Ok(scope);
        }
        let retained = self.arenas.scopes().resolve_staged(&self.slots, scope)?;
        if retained.kind() == HirScopeKind::MatchArm
            && retained.parent() == Some(parent)
            && retained.owner() == &HirScopeOwner::Expr(owner)
        {
            Ok(scope)
        } else {
            Err(HirInvariantFailure::InvalidScopeParent.into())
        }
    }

    fn allocate_expression_scope(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        parent: ScopeId,
        kind: HirScopeKind,
    ) -> Result<ScopeId, HirLowerFailure> {
        let reservation = self.arenas.scopes().reserve_source(
            &mut self.slots,
            attached.id(),
            HirSourceSite::Span(attached.whole_source_span()),
        )?;
        let scope = reservation.id();
        if reservation.is_first_touch() {
            let payload = HirScope::try_new(
                owner.module(),
                kind,
                Some(parent),
                HirScopeOwner::Expr(owner),
                Box::new([]),
                Box::new([]),
            )
            .map_err(|_| HirInvariantFailure::InvalidScopeParent)?;
            self.arenas
                .scopes()
                .finalize(&mut self.slots, reservation, payload)?;
            self.append_scope_child(parent, scope)?;
            return Ok(scope);
        }
        let retained = self.arenas.scopes().resolve_staged(&self.slots, scope)?;
        if retained.kind() == kind
            && retained.parent() == Some(parent)
            && retained.owner() == &HirScopeOwner::Expr(owner)
        {
            Ok(scope)
        } else {
            Err(HirInvariantFailure::InvalidScopeParent.into())
        }
    }

    fn expression_child<'attached>(
        &self,
        attached: &'attached AttachedExpressionNode,
        ordinal: u32,
        expected: arcweft_lang_syntax::expressions::SyntaxExpressionSlot,
    ) -> Result<&'attached AttachedExpressionChild, HirLowerFailure> {
        let child = attached
            .children()
            .iter()
            .find(|child| child.ordinal() == ordinal)
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        if child.authored().is_some()
            == matches!(
                expected,
                arcweft_lang_syntax::expressions::SyntaxExpressionSlot::Authored
            )
        {
            Ok(child)
        } else {
            Err(HirInvariantFailure::InvalidArenaCommit.into())
        }
    }

    fn lower_if_let_operand(
        &mut self,
        owner: ExprId,
        child: &AttachedExpressionChild,
        scope: ScopeId,
        role: HirExprSourceRole,
        recovery: &mut Option<HirRecoveryIssue>,
    ) -> Result<ExprId, HirLowerFailure> {
        if let Some(semantic) = child
            .authored_semantic()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
        {
            let lowered = self.lower_attached_expression_inner(&semantic, scope)?;
            if self.staged_expression_is_poisoned(lowered)? {
                recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::RecoveredChild { role },
                ));
            }
            Ok(lowered)
        } else {
            let lowered = self.lower_missing_expression(owner, scope, child, role)?;
            recovery.get_or_insert(HirRecoveryIssue::MissingOperand { role });
            Ok(lowered)
        }
    }

    fn lower_range_children(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        scope: ScopeId,
    ) -> Result<(Option<ExprId>, Option<ExprId>, Option<HirRecoveryIssue>), HirLowerFailure> {
        let ExpressionProjection::Range {
            start: expected_start,
            end: expected_end,
            ..
        } = attached.projection()
        else {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        };
        let mut start = None;
        let mut end = None;
        let mut recovery = None;
        for child in attached.children() {
            let role = composite_child_role(attached.projection(), child.ordinal())
                .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
            let lowered = if let Some(semantic) = child
                .authored_semantic()
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            {
                let lowered = self.lower_attached_expression_inner(&semantic, scope)?;
                if self.staged_expression_is_poisoned(lowered)? {
                    recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                        HirExpressionRecoveryIssue::RecoveredChild { role },
                    ));
                }
                lowered
            } else {
                let lowered = self.lower_missing_expression(owner, scope, child, role)?;
                recovery.get_or_insert(HirRecoveryIssue::MissingOperand { role });
                lowered
            };
            let slot = match child.ordinal() {
                0 => &mut start,
                1 => &mut end,
                _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
            };
            if slot.replace(lowered).is_some() {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
        }
        if start.is_some() != expected_start.is_some() || end.is_some() != expected_end.is_some() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        Ok((start, end, recovery))
    }

    fn lower_record_fields(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        scope: ScopeId,
        source_fields: &[SyntaxRecordField],
    ) -> Result<(Box<[HirRecordField]>, Option<HirRecoveryIssue>), HirLowerFailure> {
        let mut fields = Vec::with_capacity(source_fields.len());
        let mut names = BTreeSet::new();
        let mut recovery = None;
        for (field, source) in source_fields.iter().enumerate() {
            let field =
                u32::try_from(field).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            match source {
                SyntaxRecordField::Explicit {
                    name: source_name,
                    value,
                } => {
                    let Some(child) = attached
                        .children()
                        .iter()
                        .find(|child| child.ordinal() == field)
                    else {
                        return Err(HirInvariantFailure::InvalidArenaCommit.into());
                    };
                    let field_name = match source_name {
                        Ok(source_name) => name(source_name)?,
                        Err(issue) => {
                            require_attempted_name_limit(issue)?;
                            recovery
                                .get_or_insert(HirRecoveryIssue::InvalidName(name_issue(issue)));
                            fields.push(HirRecordField::invalid(HirRecordFieldIssue::MissingName));
                            continue;
                        }
                    };
                    if !names.insert(field_name.clone()) {
                        recovery.get_or_insert(HirRecoveryIssue::InvalidName(
                            crate::leaf::HirNameInvariantError::InvalidIdentifier,
                        ));
                        fields.push(HirRecordField::invalid(HirRecordFieldIssue::DuplicateName));
                        continue;
                    }
                    match value {
                        arcweft_lang_syntax::expressions::SyntaxExpressionSlot::Authored => {
                            let semantic = child
                                .authored_semantic()
                                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                                .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                            let value = self.lower_attached_expression_inner(&semantic, scope)?;
                            if self.staged_expression_is_poisoned(value)? {
                                let role = HirExprSourceRole::RecordField {
                                    field,
                                    part: crate::source_index::HirRecordFieldSourcePart::Value,
                                };
                                recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                                    HirExpressionRecoveryIssue::RecoveredChild { role },
                                ));
                            }
                            fields.push(HirRecordField::explicit(field_name, value));
                        }
                        arcweft_lang_syntax::expressions::SyntaxExpressionSlot::Missing => {
                            let role = HirExprSourceRole::RecordField {
                                field,
                                part: crate::source_index::HirRecordFieldSourcePart::Value,
                            };
                            self.lower_missing_expression(owner, scope, child, role)?;
                            recovery.get_or_insert(HirRecoveryIssue::MissingOperand { role });
                            fields.push(HirRecordField::invalid(HirRecordFieldIssue::MissingValue));
                        }
                    }
                }
                SyntaxRecordField::Shorthand { name: source_name } => {
                    let field_name = match source_name {
                        Ok(source_name) => name(source_name)?,
                        Err(issue) => {
                            require_attempted_name_limit(issue)?;
                            recovery
                                .get_or_insert(HirRecoveryIssue::InvalidName(name_issue(issue)));
                            fields.push(HirRecordField::invalid(HirRecordFieldIssue::MissingName));
                            continue;
                        }
                    };
                    if !names.insert(field_name.clone()) {
                        recovery.get_or_insert(HirRecoveryIssue::InvalidName(
                            crate::leaf::HirNameInvariantError::InvalidIdentifier,
                        ));
                        fields.push(HirRecordField::invalid(HirRecordFieldIssue::DuplicateName));
                        continue;
                    }
                    let first_use = attached
                        .component(ExpressionComponentRole::RecordField {
                            field,
                            part: arcweft_lang_syntax::expressions::ExpressionRecordFieldPart::Name,
                        })
                        .ok_or(HirInvariantFailure::InvalidSourceSpan)?;
                    let local = self
                        .visible_local(scope, &field_name, first_use.range().start())?
                        .ok_or(HirInvariantFailure::InvalidLocalTimeline)?;
                    self.record_local_capture(scope, local, first_use, CaptureAccess::Read)?;
                    fields.push(HirRecordField::shorthand(field_name, local));
                }
            }
        }
        Ok((fields.into_boxed_slice(), recovery))
    }

    pub(super) fn staged_expression_is_poisoned(
        &mut self,
        owner: ExprId,
    ) -> Result<bool, HirLowerFailure> {
        self.arenas
            .expressions()
            .resolve_staged(&self.slots, owner)
            .map(HirExpr::is_poisoned)
            .map_err(HirLowerFailure::from)
    }

    /// Lowers a parser-owned missing expression that is itself the semantic
    /// value owner, rather than a synthetic child of another expression.
    pub(super) fn lower_source_missing_expression(
        &mut self,
        attached: &AstNode<MissingExpressionKind>,
        scope: ScopeId,
    ) -> Result<ExprId, HirLowerFailure> {
        if attached.syntax().snapshot_id() != self.request.source().snapshot_id() {
            return Err(HirLowerFailure::StaleSource {
                current: self.request.source().snapshot_id().clone(),
                supplied: attached.syntax().snapshot_id().clone(),
            });
        }
        let span = attached.source_span();
        if span.source() != self.request.source().document().identity() {
            return Err(HirLowerFailure::SourceIdentityMismatch {
                expected: self.request.source().document().identity().clone(),
                actual: span.source().clone(),
            });
        }
        if !HirTypeResolver::scope_is_live(self, scope) {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        let site = HirSourceSite::from_attached_span(self.request.source().document(), &span)
            .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
        if !matches!(site, HirSourceSite::Insertion(_)) {
            return Err(HirInvariantFailure::InvalidSourceSpan.into());
        }
        let reservation = self.arenas.expressions().reserve_source(
            &mut self.slots,
            attached.id(),
            site.clone(),
        )?;
        let owner = reservation.id();
        if !reservation.is_first_touch() {
            return self.validate_reused_expression(owner, scope);
        }
        let payload = HirExpr::try_new(
            scope,
            HirExprKind::Error(HirExprError::new(
                HirGenericExprIssue::TransactionalChildFailure,
            )),
            HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
                role: HirExprSourceRole::Whole,
            }),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.source_components.stage_attached_missing_expression(
            self.request.source(),
            owner,
            attached,
            &payload,
        )?;
        self.stage_recovery_diagnostic(HirRecoveryDiagnostic::new(
            SyntheticOwner::Expr(owner),
            HirRecoveryPrimary::query(HirSourceQuery::Expr {
                owner,
                role: HirExprSourceRole::Whole,
            }),
            site,
        ));
        self.arenas
            .expressions()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(HirLowerFailure::from)
    }

    pub(super) fn lower_missing_owned_expression(
        &mut self,
        parent: SyntheticOwner,
        scope: ScopeId,
        site: HirSourceSite,
        ordinal: u32,
        role: HirExprSourceRole,
    ) -> Result<ExprId, HirLowerFailure> {
        if !matches!(site, HirSourceSite::Insertion(_)) {
            return Err(HirInvariantFailure::InvalidSourceSpan.into());
        }
        let key = SyntheticKey::try_new(parent, SyntheticRole::RecoveryOperand, ordinal)
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let reservation =
            self.arenas
                .expressions()
                .reserve_synthetic(&mut self.slots, key, site.clone())?;
        let expression = reservation.id();
        if !reservation.is_first_touch() {
            return self.validate_reused_expression(expression, scope);
        }
        let payload = HirExpr::try_new(
            scope,
            HirExprKind::Error(HirExprError::new(
                HirGenericExprIssue::TransactionalChildFailure,
            )),
            HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand { role }),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.stage_recovery_diagnostic(HirRecoveryDiagnostic::new(
            SyntheticOwner::Expr(expression),
            HirRecoveryPrimary::owner_whole(SyntheticOwner::Expr(expression)),
            site,
        ));
        self.arenas
            .expressions()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(HirLowerFailure::from)
    }

    fn lower_missing_expression(
        &mut self,
        parent: ExprId,
        scope: ScopeId,
        child: &AttachedExpressionChild,
        role: HirExprSourceRole,
    ) -> Result<ExprId, HirLowerFailure> {
        if child.missing().is_none() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        let site = HirSourceSite::from_attached_span(
            self.request.source().document(),
            &child.source_span(),
        )
        .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
        if !matches!(site, HirSourceSite::Insertion(_)) {
            return Err(HirInvariantFailure::InvalidSourceSpan.into());
        }
        self.lower_missing_owned_expression(
            SyntheticOwner::Expr(parent),
            scope,
            site,
            child.ordinal(),
            role,
        )
    }
}

fn require_match_arm_scope_limit(arm_count: usize) -> Result<(), HirLowerFailure> {
    require_limit(HirLimit::Scopes, arm_count)
}

fn recovery_diagnostic_primary(
    document: &arcweft_source::SourceDocument,
    attached: &AttachedExpressionNode,
    state: &HirPoisonState,
    whole: arcweft_source::SourceSpan,
) -> Result<(HirExprSourceRole, HirSourceSite), HirLowerFailure> {
    let role = match attached.projection() {
        ExpressionProjection::Select(SyntaxSelectedMember::Missing) => {
            HirExprSourceRole::SelectedMember
        }
        // Choice owns its complete interior source graph through the attached
        // Choice relation rather than generic expression components. Its
        // outer poison therefore anchors at the source-backed Choice owner.
        ExpressionProjection::Choice => HirExprSourceRole::Whole,
        ExpressionProjection::PostfixBracket(SyntaxPostfixBracketProjection::Invalid {
            ..
        }) => HirExprSourceRole::Content,
        _ => match state {
            HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand { role }) => *role,
            HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::Generic(_),
            )) => HirExprSourceRole::Recovery,
            HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::RecoveredChild { role },
            )) => *role,
            _ => HirExprSourceRole::Whole,
        },
    };
    let source = match role {
        HirExprSourceRole::Whole => whole,
        HirExprSourceRole::Recovery => attached
            .component(ExpressionComponentRole::Recovery)
            .ok_or(HirInvariantFailure::InvalidSourceSpan)?,
        HirExprSourceRole::SelectedMember => attached
            .component(ExpressionComponentRole::SelectedMember)
            .ok_or(HirInvariantFailure::InvalidSourceSpan)?,
        HirExprSourceRole::Content => attached
            .component(ExpressionComponentRole::Content)
            .ok_or(HirInvariantFailure::InvalidSourceSpan)?,
        HirExprSourceRole::CloseBracket => attached
            .component(ExpressionComponentRole::CloseBracket)
            .ok_or(HirInvariantFailure::InvalidSourceSpan)?,
        HirExprSourceRole::Statement { ordinal } => attached
            .component(ExpressionComponentRole::Statement { ordinal })
            .ok_or(HirInvariantFailure::InvalidSourceSpan)?,
        HirExprSourceRole::Tail => attached
            .component(ExpressionComponentRole::Tail)
            .ok_or(HirInvariantFailure::InvalidSourceSpan)?,
        HirExprSourceRole::MatchArm { arm, part } => attached
            .match_arms()
            .get(usize::try_from(arm).map_err(|_| HirInvariantFailure::InvalidSourceSpan)?)
            .and_then(|source_arm| {
                source_arm.component(match part {
                    HirMatchArmSourcePart::Whole => SyntaxMatchArmPart::Whole,
                    HirMatchArmSourcePart::Pattern => SyntaxMatchArmPart::Pattern,
                    HirMatchArmSourcePart::Guard => SyntaxMatchArmPart::Guard,
                    HirMatchArmSourcePart::Arrow => SyntaxMatchArmPart::Arrow,
                    HirMatchArmSourcePart::Value => SyntaxMatchArmPart::Value,
                })
            })
            .ok_or(HirInvariantFailure::InvalidSourceSpan)?,
        _ => attached
            .children()
            .iter()
            .find(|child| {
                composite_child_role(attached.projection(), child.ordinal()) == Some(role)
            })
            .map(AttachedExpressionChild::source_span)
            .ok_or(HirInvariantFailure::InvalidSourceSpan)?,
    };
    let site = HirSourceSite::from_attached_span(document, &source)
        .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
    Ok((role, site))
}

fn composite_child_role(
    projection: &ExpressionProjection,
    ordinal: u32,
) -> Option<HirExprSourceRole> {
    match projection {
        ExpressionProjection::Tuple(_) | ExpressionProjection::BracketSequence(_) => {
            Some(HirExprSourceRole::Element { ordinal })
        }
        ExpressionProjection::ArrayRepeat(_) if ordinal == 0 => {
            Some(HirExprSourceRole::RepeatValue)
        }
        ExpressionProjection::ArrayRepeat(_) if ordinal == 1 => {
            Some(HirExprSourceRole::RepeatLength)
        }
        ExpressionProjection::Select(_) if ordinal == 0 => Some(HirExprSourceRole::Target),
        ExpressionProjection::Index(_) if ordinal == 0 => Some(HirExprSourceRole::Target),
        ExpressionProjection::Index(_) if ordinal == 1 => Some(HirExprSourceRole::Index),
        ExpressionProjection::DialogueContentApplication(_) if ordinal == 0 => {
            Some(HirExprSourceRole::Target)
        }
        ExpressionProjection::PostfixBracket(_) if ordinal == 0 => Some(HirExprSourceRole::Target),
        ExpressionProjection::Pipe(_) if ordinal == 0 => Some(HirExprSourceRole::LeftOperand),
        ExpressionProjection::Pipe(_) if ordinal == 1 => Some(HirExprSourceRole::RightOperand),
        ExpressionProjection::Range { .. } if ordinal == 0 => Some(HirExprSourceRole::RangeStart),
        ExpressionProjection::Range { .. } if ordinal == 1 => Some(HirExprSourceRole::RangeEnd),
        ExpressionProjection::Binary { .. } if ordinal == 0 => Some(HirExprSourceRole::LeftOperand),
        ExpressionProjection::Binary { .. } if ordinal == 1 => {
            Some(HirExprSourceRole::RightOperand)
        }
        ExpressionProjection::If { .. } if ordinal == 0 => Some(HirExprSourceRole::Condition),
        ExpressionProjection::If { .. } if ordinal == 1 => Some(HirExprSourceRole::ThenBranch),
        ExpressionProjection::If { .. } if ordinal == 2 => Some(HirExprSourceRole::ElseBranch),
        ExpressionProjection::IfLet { .. } if ordinal == 0 => Some(HirExprSourceRole::Scrutinee),
        ExpressionProjection::IfLet { .. } if ordinal == 1 => Some(HirExprSourceRole::Guard),
        ExpressionProjection::IfLet { .. } if ordinal == 2 => Some(HirExprSourceRole::ThenBranch),
        ExpressionProjection::IfLet { .. } if ordinal == 3 => Some(HirExprSourceRole::ElseBranch),
        ExpressionProjection::Match(_) if ordinal == 0 => Some(HirExprSourceRole::Scrutinee),
        ExpressionProjection::Closure(_) if ordinal == 0 => Some(HirExprSourceRole::Body),
        ExpressionProjection::Record(_) | ExpressionProjection::RecordLiteral(_) => {
            Some(HirExprSourceRole::RecordField {
                field: ordinal,
                part: crate::source_index::HirRecordFieldSourcePart::Value,
            })
        }
        ExpressionProjection::Try { .. }
        | ExpressionProjection::Await { .. }
        | ExpressionProjection::Borrow { .. }
        | ExpressionProjection::Dereference { .. }
        | ExpressionProjection::Unary { .. }
            if ordinal == 0 =>
        {
            Some(HirExprSourceRole::Operand)
        }
        _ => None,
    }
}

const fn try_form(form: SyntaxTryForm) -> HirTryForm {
    match form {
        SyntaxTryForm::PrefixTry => HirTryForm::PrefixTry,
        SyntaxTryForm::PostfixQuestion => HirTryForm::PostfixQuestion,
    }
}

const fn await_propagation(propagation: SyntaxAwaitPropagation) -> HirAwaitPropagation {
    match propagation {
        SyntaxAwaitPropagation::PreserveResult => HirAwaitPropagation::PreserveResult,
        SyntaxAwaitPropagation::PropagateError => HirAwaitPropagation::PropagateError,
    }
}

const fn binary_operator(operator: SyntaxBinaryOperator) -> HirBinaryOp {
    match operator {
        SyntaxBinaryOperator::Implies => HirBinaryOp::Implies,
        SyntaxBinaryOperator::Or => HirBinaryOp::Or,
        SyntaxBinaryOperator::And => HirBinaryOp::And,
        SyntaxBinaryOperator::In => HirBinaryOp::In,
        SyntaxBinaryOperator::Equal => HirBinaryOp::Equal,
        SyntaxBinaryOperator::NotEqual => HirBinaryOp::NotEqual,
        SyntaxBinaryOperator::GreaterOrEqual => HirBinaryOp::GreaterOrEqual,
        SyntaxBinaryOperator::LessOrEqual => HirBinaryOp::LessOrEqual,
        SyntaxBinaryOperator::Greater => HirBinaryOp::Greater,
        SyntaxBinaryOperator::Less => HirBinaryOp::Less,
        SyntaxBinaryOperator::Merge => HirBinaryOp::Merge,
        SyntaxBinaryOperator::Add => HirBinaryOp::Add,
        SyntaxBinaryOperator::Subtract => HirBinaryOp::Subtract,
        SyntaxBinaryOperator::Multiply => HirBinaryOp::Multiply,
        SyntaxBinaryOperator::Divide => HirBinaryOp::Divide,
        SyntaxBinaryOperator::Remainder => HirBinaryOp::Remainder,
    }
}

const fn borrow_kind(kind: SyntaxBorrowKind) -> HirBorrowKind {
    match kind {
        SyntaxBorrowKind::Shared => HirBorrowKind::Shared,
        SyntaxBorrowKind::Mutable => HirBorrowKind::Mutable,
    }
}

const fn unary_operator(operator: SyntaxUnaryOperator) -> HirUnaryOp {
    match operator {
        SyntaxUnaryOperator::Not => HirUnaryOp::Not,
        SyntaxUnaryOperator::Negate => HirUnaryOp::Negate,
    }
}

fn project_numeric_sequence(
    sequence: &SyntaxNumericSequence,
) -> Result<HirNumericSequence, HirLowerFailure> {
    let elements = sequence
        .elements()
        .iter()
        .map(|element| match integer_literal(element.integer())? {
            HirIntegerLiteral::Value {
                magnitude, radix, ..
            } => Ok(HirNumericSequenceElement::new(magnitude, radix)),
            HirIntegerLiteral::Invalid(_) => Err(HirInvariantFailure::InvalidArenaCommit.into()),
        })
        .collect::<Result<Vec<_>, HirLowerFailure>>()?
        .into_boxed_slice();
    let recovery = match sequence.recovery() {
        SyntaxNumericSequenceRecovery::Complete => HirNumericSequenceRecovery::Complete,
        SyntaxNumericSequenceRecovery::MissingFinalElement { ordinal } => {
            HirNumericSequenceRecovery::MissingFinalElement { ordinal: *ordinal }
        }
        SyntaxNumericSequenceRecovery::InvalidElement { ordinal, issue, .. } => {
            HirNumericSequenceRecovery::InvalidElement {
                ordinal: *ordinal,
                issue: integer_issue(issue),
            }
        }
        SyntaxNumericSequenceRecovery::ConflictingSuffix {
            ordinal,
            first,
            conflicting,
        } => HirNumericSequenceRecovery::ConflictingSuffix {
            ordinal: *ordinal,
            first: integer_suffix(*first),
            conflicting: integer_suffix(*conflicting),
        },
    };
    HirNumericSequence::try_new(
        elements,
        sequence.common_suffix().map(integer_suffix),
        recovery,
    )
    .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
}

fn project_lifetime_path(
    value: &SyntaxLifetimeRegistryPath,
) -> Result<HirLifetimePathValue, HirLowerFailure> {
    require_limit(HirLimit::RegistrySegments, value.segments().len())?;
    let segment_count = u32::try_from(value.segments().len())
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;

    let (scope, scope_present, mut recovery, mut semantic_bytes) = match value.scope() {
        SyntaxLifetimeRegistryScope::Frame => {
            (Some(HirLifetimeRegistryScope::Frame), true, None, 0)
        }
        SyntaxLifetimeRegistryScope::Tick => (Some(HirLifetimeRegistryScope::Tick), true, None, 0),
        SyntaxLifetimeRegistryScope::Cue => (Some(HirLifetimeRegistryScope::Cue), true, None, 0),
        SyntaxLifetimeRegistryScope::Line => (Some(HirLifetimeRegistryScope::Line), true, None, 0),
        SyntaxLifetimeRegistryScope::Scene => {
            (Some(HirLifetimeRegistryScope::Scene), true, None, 0)
        }
        SyntaxLifetimeRegistryScope::Flow => (Some(HirLifetimeRegistryScope::Flow), true, None, 0),
        SyntaxLifetimeRegistryScope::Session => {
            (Some(HirLifetimeRegistryScope::Session), true, None, 0)
        }
        SyntaxLifetimeRegistryScope::Global => {
            (Some(HirLifetimeRegistryScope::Global), true, None, 0)
        }
        SyntaxLifetimeRegistryScope::Persistent => {
            (Some(HirLifetimeRegistryScope::Persistent), true, None, 0)
        }
        SyntaxLifetimeRegistryScope::Named(scope) => {
            let scope = name(scope)?;
            let bytes = scope.as_str().len();
            (
                Some(HirLifetimeRegistryScope::Named(scope)),
                true,
                None,
                bytes,
            )
        }
        SyntaxLifetimeRegistryScope::Recovered(issue) => {
            require_attempted_name_limit(issue)?;
            let bytes = attempted_name_bytes(issue);
            let (present, issue) = match issue {
                SyntaxNameIssue::Missing => (false, HirLifetimeRegistryIssue::MissingScope),
                SyntaxNameIssue::InvalidStart { .. }
                | SyntaxNameIssue::InvalidContinuation { .. } => {
                    (true, HirLifetimeRegistryIssue::InvalidNamedScope)
                }
            };
            (None, present, Some(issue), bytes)
        }
    };

    let mut segments = Vec::with_capacity(value.segments().len());
    for (position, segment) in value.segments().iter().enumerate() {
        let bytes = match segment {
            Ok(name) => name.as_str().len(),
            Err(issue) => attempted_name_bytes(issue),
        };
        require_limit(HirLimit::NameBytes, bytes)?;
        semantic_bytes = semantic_bytes
            .checked_add(bytes)
            .ok_or_else(|| limit_overflow(HirLimit::RegistrySemanticBytes))?;
        match segment {
            Ok(segment) => segments.push(name(segment)?),
            Err(_) if recovery.is_none() => {
                recovery = Some(HirLifetimeRegistryIssue::InvalidKeySegment {
                    ordinal: u32::try_from(position)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                });
            }
            Err(_) => {}
        }
    }
    require_limit(HirLimit::RegistrySemanticBytes, semantic_bytes)?;

    if let Some(issue) = recovery {
        return Ok(HirLifetimePathValue::Recovered(
            HirLifetimePathRecovery::new(scope_present, segment_count, value.is_optional(), issue),
        ));
    }
    Ok(HirLifetimePathValue::Resolved(
        HirLifetimeRegistryPath::try_new(
            scope.ok_or(HirInvariantFailure::InvalidArenaCommit)?,
            segments.into_boxed_slice(),
            value.is_optional(),
        ),
    ))
}

fn limit_overflow(limit: HirLimit) -> HirLowerFailure {
    HirLimitError::with_maximum(limit, usize::MAX, limit.maximum()).into()
}

#[cfg(test)]
#[path = "expression_lowering/tests.rs"]
mod tests;
