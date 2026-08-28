//! Expression-family checking outside ordinary-call resolution.

#[path = "dialogue_line_plan.rs"]
mod dialogue_line_plan;
#[path = "expressions/records.rs"]
mod records;

use std::{rc::Rc, sync::Arc};

use super::{
    Analyzer, ArrayLength, BTreeSet, BorrowKind, CallableDeclarationKey,
    CandidateSemanticProjection, CheckedAwait, CheckedAwaitPendingObserver, CheckedChoice,
    CheckedChoiceGoto, CheckedClosure, CheckedDialogueEffectSiteOrdinal,
    CheckedDialogueEffectTrigger, CheckedDialogueMarkHandler, CheckedDialogueMarkOrdinal,
    CheckedExpression, CheckedExpressionResolution, CheckedImplicitCallable, CheckedPipe,
    CheckedProjectItem, CheckedStageLook, CheckedStyleCallee, CheckedTry, CheckedTryBoundary,
    CheckedTryCarrier, CheckedTypeSelection, CheckedValueResolution, CheckedVariantOwner,
    CheckedViewCall, CheckedViewCallee, EffectId, EffectSet, EntityKind, EnumVariantPayload,
    ExprId, FinalSemanticAnalysisError, GenericParameterOwnerId, GenericTypeParameterId,
    HirAwaitBranchKind, HirBinaryOp, HirBorrowKind, HirCallArgument, HirChoiceCompactAction,
    HirChoiceItem, HirComputationBlockKind, HirExpr, HirExprKind, HirIdRef, HirIdRefValue,
    HirIntegerLiteral, HirItemKind, HirLinePlanItem, HirLiteral, HirModule, HirPathRoot,
    HirPathSegment, HirPatternKind, HirPostfixBracket, HirPostfixBracketCandidates, HirRecordField,
    HirRecoveredName, HirScopeKind, HirScopeOwner, HirSelectedMember, HirSourcePresence,
    HirSourceQuery, HirSourceSite, HirStmtKind, HirTriggerPattern, HirTypeSourceRole, HirUnaryOp,
    LocalLookup, PostfixBracketResolution, PreparedDialogueApplication, PreparedDialogueEffectSite,
    PreparedDialogueLinePlan, PreparedExpressionFact, PreparedExpressionShell,
    ProjectHirSymbolLookupError, ProjectNominalBody, ProjectNominalDeclaration, ProjectNominalType,
    ProjectSymbolResolutionError, ProjectTypeTarget, ProjectValueLookup, RegisteredSemanticValueId,
    ResolvedProjectSymbol, RichTextAttributeChecker, ScopeId, SourceSpan, TypeKind,
    TypeParameterSubstitutions,
    calls::{checked_character_dialogue_target, checked_project_nominal, nominal_substitutions},
    expression_types::{
        common_type, expected_item, indexed_item, literal_type, value_resolution_type,
    },
    patterns::{checked_builtin_closed_owner, resolve_closed_variant_path},
    statements::{enclosing_item, expression_span},
};
use crate::callable::{
    CallResolverAuthority, CallResolverRequest, CharacterDialoguePatchContext, DialogueCallableId,
    DialogueCalleeIdentity, PreparedCallCallee, ResolveCallOutcome, ResolvedCallTarget,
    ResolverWork, resolve_call_target,
};
use crate::checked_rich_text::{
    CheckedDialogueHostEvent, CheckedDialogueToken, CheckedRichTextAction,
};
use crate::final_analysis::type_rules::integer_suffix_type;
use crate::registration::RegisteredExternalOwner;
use arcweft_id::PublicId;
use arcweft_lang_hir::expr::HirPlaceholderKind;

use super::expression_error::{AnalyzerExpressionContext, AnalyzerExpressionError};
use super::state::{
    CandidateFactOperationFailure, CandidateFactTransactionAction,
    CandidateFactTransactionAuthority,
};

use super::entities::EntityReferenceResolutionError;

/// Typed contextual expectation carried from a parent lower source.
///
/// `Complete` may constrain a nested call's result. `Parametric` exposes its
/// shape to contextual non-call grammar, but its declaration-owned unbound
/// parameters remain owned by the parent constraint scope and therefore may
/// not enter a child call solver. Projecting the carrier to a child shape
/// intersects the exact sorted unbound inventory; a child with no remaining
/// outer parameters becomes complete.
#[derive(Clone, Debug)]
pub(super) enum AnalyzerExpressionExpectation<'a> {
    Unconstrained,
    Complete(&'a TypeKind),
    EnumConstructorHead(&'a TypeKind),
    Parametric {
        expected: &'a TypeKind,
        unbound: Arc<[crate::types::constraints::ConstraintGenericParameterId]>,
    },
}

impl<'a> AnalyzerExpressionExpectation<'a> {
    pub(super) fn from_complete(expected: Option<&'a TypeKind>) -> Self {
        expected.map_or(Self::Unconstrained, Self::Complete)
    }

    pub(super) const fn enum_constructor_head(expected: &'a TypeKind) -> Self {
        Self::EnumConstructorHead(expected)
    }

    pub(super) fn parametric(
        expected: &'a TypeKind,
        unbound: &[crate::types::constraints::ConstraintGenericParameterId],
    ) -> Option<Self> {
        if unbound.is_empty() || !unbound.windows(2).all(|pair| pair[0] < pair[1]) {
            return None;
        }
        let inventory = crate::types::TypeGenericUseCollector::collect(expected).ok()?;
        if !unbound.iter().all(|parameter| match parameter {
            crate::types::constraints::ConstraintGenericParameterId::Type(parameter) => {
                inventory.types().binary_search(parameter).is_ok()
            }
            crate::types::constraints::ConstraintGenericParameterId::Const(parameter) => {
                inventory.consts().binary_search(parameter).is_ok()
            }
        }) {
            return None;
        }
        Some(Self::Parametric {
            expected,
            unbound: Arc::from(unbound),
        })
    }

    pub(super) fn contextual_shape(&self) -> Option<&'a TypeKind> {
        match self {
            Self::Unconstrained => None,
            Self::Complete(expected) | Self::EnumConstructorHead(expected) => Some(expected),
            Self::Parametric { expected, unbound } if matches!(expected, TypeKind::GenericParam(parameter) if unbound.iter().any(|candidate| matches!(candidate, crate::types::constraints::ConstraintGenericParameterId::Type(candidate) if candidate == parameter))) => {
                None
            }
            Self::Parametric { expected, .. } => Some(expected),
        }
    }

    /// Only a closed expectation may become a child call result equation.
    pub(super) const fn child_call_result(&self) -> Option<&'a TypeKind> {
        match self {
            Self::Complete(expected) | Self::EnumConstructorHead(expected) => Some(expected),
            Self::Unconstrained | Self::Parametric { .. } => None,
        }
    }

    fn accepts_cached(&self, checked: &super::PreparedExpressionFact) -> bool {
        match self {
            Self::Complete(expected) | Self::EnumConstructorHead(expected) => {
                expected.accepts(checked.ty())
            }
            Self::Parametric { expected, .. } => {
                checked.reusable_for_parametric_expectation(expected)
            }
            Self::Unconstrained => true,
        }
    }

    pub(super) fn is_contextual(&self) -> bool {
        !matches!(self, Self::Unconstrained)
    }

    pub(super) const fn is_enum_constructor_head(&self) -> bool {
        matches!(self, Self::EnumConstructorHead(_))
    }

    fn project<'b>(
        &self,
        expected: Option<&'b TypeKind>,
    ) -> Result<AnalyzerExpressionExpectation<'b>, crate::types::TypeGenericUseError> {
        let Some(expected) = expected else {
            return Ok(AnalyzerExpressionExpectation::Unconstrained);
        };
        let Self::Parametric { unbound, .. } = self else {
            return Ok(AnalyzerExpressionExpectation::Complete(expected));
        };
        let inventory = crate::types::TypeGenericUseCollector::collect(expected)?;
        let projected = unbound
            .iter()
            .filter(|parameter| match parameter {
                crate::types::constraints::ConstraintGenericParameterId::Type(parameter) => {
                    inventory.types().binary_search(parameter).is_ok()
                }
                crate::types::constraints::ConstraintGenericParameterId::Const(parameter) => {
                    inventory.consts().binary_search(parameter).is_ok()
                }
            })
            .cloned()
            .collect::<Vec<_>>();
        if projected.is_empty() {
            Ok(AnalyzerExpressionExpectation::Complete(expected))
        } else {
            Ok(AnalyzerExpressionExpectation::Parametric {
                expected,
                unbound: Arc::from(projected),
            })
        }
    }

    pub(super) fn project_checked<'b>(
        &self,
        owner: ExprId,
        expected: Option<&'b TypeKind>,
    ) -> Result<AnalyzerExpressionExpectation<'b>, AnalyzerExpressionError> {
        self.project(expected).map_err(|_| AnalyzerExpressionError::Call {
            owner,
            failure: super::calls::CallAnalysisFailure::Invariant(
                super::calls::CallAnalysisInvariant::Constraint(
                    crate::callable::CallConstraintInvariant::Lower(
                        crate::types::constraints::TypeConstraintInvariant::SourceProtocol(
                            crate::types::constraints::TypeConstraintSourceProtocolInvariant::InvalidEvidence,
                        ),
                    ),
                ),
            ),
        })
    }
}

impl Analyzer<'_, '_, '_> {
    /// Root/publication entrypoint.  Recursive expression evaluation must use
    /// [`Self::evaluate_expression`] with the caller-owned context so a
    /// candidate never falls back to the published fact map.
    pub(super) fn check_expression_published(
        &mut self,
        owner: ExprId,
        expected: Option<&TypeKind>,
    ) -> Result<super::PreparedExpressionFact, FinalSemanticAnalysisError> {
        let context = AnalyzerExpressionContext::published(Rc::clone(&self.call_frames));
        self.evaluate_expression(&context, owner, expected)
            .map_err(|error| error.into_public(owner))
    }

    pub(super) fn evaluate_expression(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        owner: ExprId,
        expected: Option<&TypeKind>,
    ) -> Result<super::PreparedExpressionFact, AnalyzerExpressionError> {
        self.evaluate_expression_with_expectation(
            context,
            owner,
            AnalyzerExpressionExpectation::from_complete(expected),
        )
    }

    pub(super) fn evaluate_expression_with_expectation(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        owner: ExprId,
        expectation: AnalyzerExpressionExpectation<'_>,
    ) -> Result<super::PreparedExpressionFact, AnalyzerExpressionError> {
        let cached = match context.authority() {
            super::expression_error::AnalyzerExpressionFactAuthority::Published => {
                self.facts.expressions().get(&owner).cloned()
            }
            super::expression_error::AnalyzerExpressionFactAuthority::Candidate(authority) => self
                .facts
                .candidate_expression(authority, owner)
                .map_err(AnalyzerExpressionError::fact)?,
        };
        if let Some(checked) = cached {
            if expectation.accepts_cached(&checked) {
                if let Some(CheckedExpressionResolution::Value(CheckedValueResolution::Local(
                    local,
                ))) = checked.checked_resolution()
                {
                    self.record_implicit_capture(owner, *local)
                        .map_err(AnalyzerExpressionError::fatal)?;
                }
                return Ok(checked);
            }
            let module = self
                .module(owner.module())
                .map_err(AnalyzerExpressionError::fatal)?;
            let expression = module.resolve_expr(owner).map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
            })?;
            if let (Some(expected), HirExprKind::Literal(literal)) =
                (expectation.contextual_shape(), expression.kind())
                && let Some((ty, selection)) = literal_type(literal, Some(expected))
            {
                let contextual = CheckedExpression::new(
                    ty,
                    selection,
                    checked.effects().clone(),
                    CheckedExpressionResolution::Literal(literal.clone()),
                );
                let outcome = self.run_candidate_fact_transaction::<_, AnalyzerExpressionError>(
                    |this, _authority, _transaction| {
                        this.facts
                            .replace_contextual_expression(owner, contextual.clone())
                            .map_err(|_| {
                                AnalyzerExpressionError::fact(
                                    super::CandidateFactTransactionViolation::ProjectionUnavailable,
                                )
                            })?;
                        Ok(CandidateFactTransactionAction::Commit(contextual))
                    },
                )?;
                return outcome
                    .into_committed()
                    .map(Into::into)
                    .map_err(AnalyzerExpressionError::fact);
            }
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        self.with_expression_guard(owner, |this| {
            let outcome = this.run_candidate_fact_transaction::<_, CandidateFactOperationFailure>(
                |this, _authority, transaction_authority| {
                    this.control.check().map_err(|error| match error {
                        FinalSemanticAnalysisError::Cancelled => AnalyzerExpressionError::Abort(
                            crate::types::constraints::TypeConstraintAbort::Cancelled,
                        ),
                        error => AnalyzerExpressionError::fatal(error),
                    })?;
                    let module = this
                        .module(owner.module())
                        .map_err(AnalyzerExpressionError::fatal)?;
                    let expression = module
                        .resolve_expr(owner)
                        .map_err(|_| {
                            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                        })?
                        .clone();
                    let expression_uses = this
                        .topology
                        .module(owner.module())
                        .filter(|topology| topology.snapshot() == module.snapshot_id())
                        .map(|topology| topology.expression_uses())
                        .ok_or_else(|| {
                            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                        })?;
                    let region = expression_uses
                        .implicit_callable_region(owner, HirPlaceholderKind::PartialApplication)
                        .map_err(|_| {
                            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                        })?;
                    let placeholders = region.placeholders().collect::<Vec<_>>().into_boxed_slice();
                    let inside_implicit_callable =
                        this.implicit_callable_stack.iter().rev().any(|context| {
                            expression_uses
                                .implicit_callable_region(
                                    context.owner,
                                    HirPlaceholderKind::PartialApplication,
                                )
                                .is_ok_and(|region| region.contains_expression(owner))
                        });
                    let checked = if placeholders.is_empty() || inside_implicit_callable {
                        match this.check_prepared_expression_kind(
                            context,
                            module,
                            owner,
                            &expression,
                            &expectation,
                        )? {
                            Some(prepared) => Ok(prepared),
                            None => this
                                .check_expression_kind::<CandidateFactOperationFailure>(
                                    context,
                                    module,
                                    owner,
                                    &expression,
                                    &expectation,
                                    &transaction_authority,
                                )
                                .map(Into::into),
                        }
                    } else {
                        this.check_implicit_callable_expression::<CandidateFactOperationFailure>(
                            context,
                            module,
                            owner,
                            &expression,
                            &expectation,
                            placeholders,
                            &transaction_authority,
                        )
                        .map(Into::into)
                    };
                    let checked = checked?;
                    let checked = this.attach_nested_path_evidence(owner, checked)?;
                    let write = if context.is_candidate()
                        && this.facts.expressions().contains_key(&owner)
                    {
                        this.facts
                            .replace_existing_expression(owner, checked.clone())
                    } else {
                        this.facts.publish_new_expression(owner, checked.clone())
                    };
                    if let Err(error) = write {
                        return Err(match error {
                            super::state::ExpressionFactWriteViolation::AlreadyPublished => {
                                AnalyzerExpressionError::invariant(
                                    FinalSemanticAnalysisError::DuplicateFact {
                                        family: super::SemanticFactFamily::Expression,
                                    },
                                )
                            }
                            super::state::ExpressionFactWriteViolation::MissingPublishedFact => {
                                AnalyzerExpressionError::invariant(
                                    FinalSemanticAnalysisError::WrongPayloadFamily,
                                )
                            }
                            super::state::ExpressionFactWriteViolation::Candidate(violation) => {
                                AnalyzerExpressionError::fact(violation)
                            }
                        }
                        .into());
                    }
                    Ok(CandidateFactTransactionAction::Commit(checked))
                },
            )?;
            outcome
                .into_committed()
                .map_err(AnalyzerExpressionError::fact)
        })
    }

    fn with_expression_guard<T>(
        &mut self,
        owner: ExprId,
        operation: impl FnOnce(&mut Self) -> Result<T, AnalyzerExpressionError>,
    ) -> Result<T, AnalyzerExpressionError> {
        if !self
            .facts
            .begin_expression(owner)
            .map_err(AnalyzerExpressionError::fact)?
        {
            return Err(AnalyzerExpressionError::invariant(
                FinalSemanticAnalysisError::ExpressionCycle { owner },
            ));
        }
        let result = operation(self);
        self.facts.end_expression(owner);
        result
    }

    fn attach_nested_path_evidence(
        &self,
        owner: ExprId,
        checked: super::PreparedExpressionFact,
    ) -> Result<super::PreparedExpressionFact, AnalyzerExpressionError> {
        let super::PreparedExpressionFact::Complete(complete) = checked else {
            return Ok(checked);
        };
        let module = self
            .module(owner.module())
            .map_err(AnalyzerExpressionError::fatal)?;
        let expression = module.resolve_expr(owner).map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
        })?;
        let edges = self
            .topology
            .expression_edges(owner)
            .iter()
            .filter_map(|edge| match edge {
                arcweft_lang_hir::project::HirExpressionEvaluationEdge::Expression {
                    role,
                    ownership: arcweft_lang_hir::expr::HirExpressionChildOwnership::Owning,
                    child,
                } => Some((*child, role.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        let Some(evidence) = crate::final_analysis::match_edges::build_nested_path_evidence(
            expression.kind(),
            &complete,
            &edges,
            self.facts.expressions(),
        ) else {
            return Ok(complete.into());
        };
        Ok(complete.with_nested_path_evidence(evidence).into())
    }

    fn check_implicit_callable_expression<E>(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expectation: &AnalyzerExpressionExpectation<'_>,
        placeholders: Box<[ExprId]>,
        transaction_authority: &CandidateFactTransactionAuthority<'_>,
    ) -> Result<CheckedExpression, E>
    where
        E: From<AnalyzerExpressionError> + From<CandidateFactOperationFailure>,
    {
        let expected = expectation.contextual_shape();
        let contextual = match expected {
            Some(TypeKind::Function {
                params,
                return_type,
                ..
            }) if params.len() == 1 => Some((params[0].clone(), return_type.as_ref().clone())),
            Some(_) => {
                return Err(AnalyzerExpressionError::rejected(owner).into());
            }
            None => None,
        };
        let parameter = contextual
            .as_ref()
            .map(|(parameter, _)| parameter.clone())
            .map_or_else(
                || self.infer_implicit_parameter(context, module, owner, expression, &placeholders),
                Ok,
            )?;
        let expected_result = contextual.as_ref().map(|(_, result)| result);
        let body_expectation = expectation.project_checked(owner, expected_result)?;
        self.implicit_callable_stack
            .push(super::ImplicitCallableContext {
                owner,
                parameter: parameter.clone(),
                result: expected_result.cloned(),
                placeholders: placeholders.clone(),
            });
        let body = self.check_expression_kind::<E>(
            context,
            module,
            owner,
            expression,
            &body_expectation,
            transaction_authority,
        );
        let context = self
            .implicit_callable_stack
            .pop()
            .expect("implicit callable context was just pushed");
        let body = body?;
        let result = if matches!(
            body.resolution(),
            CheckedExpressionResolution::Try(tried)
                if tried.boundary() == CheckedTryBoundary::FunctionSite(owner)
        ) {
            context
                .result
                .clone()
                .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?
        } else {
            body.ty().clone()
        };
        let ty = TypeKind::function_with_effects(
            [parameter.clone()],
            result.clone(),
            crate::effect_row::EffectRow::closed(body.effects().clone()),
        );
        if matches!(expectation, AnalyzerExpressionExpectation::Complete(expected) if !expected.accepts(&ty))
        {
            return Err(AnalyzerExpressionError::rejected(owner).into());
        }
        let uses = self
            .facts
            .take_implicit_capture_uses(owner)
            .map_err(AnalyzerExpressionError::fact)?;
        let callable = CheckedImplicitCallable::seal(
            Arc::clone(&self.topology),
            owner,
            parameter,
            result,
            uses,
            body.resolution().clone(),
        )
        .map_err(|violation| {
            AnalyzerExpressionError::invariant(FinalSemanticAnalysisError::from(violation))
        })?;
        Ok(CheckedExpression::new(
            ty,
            if expectation.is_contextual() {
                CheckedTypeSelection::Expected
            } else {
                CheckedTypeSelection::Inferred
            },
            EffectSet::new(),
            CheckedExpressionResolution::ImplicitCallable(Box::new(callable)),
        ))
    }

    fn infer_implicit_parameter(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        placeholders: &[ExprId],
    ) -> Result<TypeKind, AnalyzerExpressionError> {
        let HirExprKind::Binary(binary) = expression.kind() else {
            return Err(AnalyzerExpressionError::rejected(owner));
        };
        let left_is_placeholder = placeholders.contains(&binary.left());
        let right_is_placeholder = placeholders.contains(&binary.right());
        if left_is_placeholder == right_is_placeholder {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        let concrete_owner = if left_is_placeholder {
            binary.right()
        } else {
            binary.left()
        };
        let contains_nested_placeholder = self
            .topology
            .module(concrete_owner.module())
            .filter(|topology| topology.snapshot() == module.snapshot_id())
            .ok_or_else(|| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
            })?
            .expression_uses()
            .implicit_callable_region(concrete_owner, HirPlaceholderKind::PartialApplication)
            .map_err(|_| AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner))?
            .placeholders()
            .next()
            .is_some();
        if contains_nested_placeholder {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        Ok(self
            .evaluate_expression(context, concrete_owner, None)?
            .ty()
            .clone())
    }

    fn record_implicit_capture(
        &mut self,
        expression: ExprId,
        local: super::LocalId,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let topology = self
            .topology
            .module(expression.module())
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        let expression_uses = topology.expression_uses();
        let binding = topology
            .local_origins()
            .binding(local)
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        let callable_owners = self
            .implicit_callable_stack
            .iter()
            .rev()
            .map(|context| context.owner)
            .collect::<Vec<_>>();
        for callable in callable_owners {
            let region = expression_uses
                .implicit_callable_region(callable, HirPlaceholderKind::PartialApplication)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            if !region.contains_expression(expression) {
                continue;
            }
            if region.contains_binding(binding) {
                continue;
            }
            self.facts
                .record_implicit_capture_use(callable, expression, local)?;
        }
        Ok(())
    }

    /// Produces only rows whose final identity depends on the project-wide C2
    /// seal. Ordinary checked expressions continue through the closed family
    /// dispatcher below; this boundary never projects a runtime nominal.
    fn check_prepared_expression_kind(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expectation: &AnalyzerExpressionExpectation<'_>,
    ) -> Result<Option<PreparedExpressionFact>, AnalyzerExpressionError> {
        let expected = expectation.contextual_shape();
        match expression.kind() {
            HirExprKind::DialogueContentApplication(application) => self
                .prepare_dialogue_content_application(
                    context,
                    module,
                    owner,
                    application,
                    expectation,
                )
                .map(Some),
            HirExprKind::EntityReference(reference)
                if reference
                    .as_resolved()
                    .and_then(|reference| reference.absolute_family())
                    == Some("entry") =>
            {
                let reference = reference.as_resolved().ok_or_else(|| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::RecoveredOwner)
                })?;
                self.prepare_entry_reference(owner, reference, expected)
                    .map(Some)
            }
            HirExprKind::ShortVariant(name)
                if matches!(expected, Some(TypeKind::ProjectNominal(_))) =>
            {
                let name = name.as_resolved().ok_or_else(|| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::RecoveredOwner)
                })?;
                self.prepare_project_variant_expression(
                    owner,
                    expected,
                    name,
                    expectation.is_enum_constructor_head(),
                )
                .map(Some)
            }
            HirExprKind::Record(record) => {
                let declaration = match self
                    .symbols
                    .resolve_hir_type_target(
                        module.key().path(),
                        record.path(),
                        expression_span(module, owner).map_err(AnalyzerExpressionError::fatal)?,
                    )
                    .map_err(|_| AnalyzerExpressionError::rejected(owner))?
                {
                    ProjectTypeTarget::Nominal(declaration) => declaration.clone(),
                    ProjectTypeTarget::External(_) => {
                        return Err(AnalyzerExpressionError::rejected(owner));
                    }
                };
                self.prepare_project_record_expression(
                    context,
                    owner,
                    &declaration,
                    record.fields(),
                    expectation,
                )
                .map(Some)
            }
            HirExprKind::RecordLiteral(record) => {
                let Some(TypeKind::ProjectNominal(expected_nominal)) = expected else {
                    return Err(AnalyzerExpressionError::rejected(owner));
                };
                let declaration = self
                    .symbols
                    .nominal(expected_nominal.declaration())
                    .cloned()
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                self.prepare_project_record_expression(
                    context,
                    owner,
                    &declaration,
                    record.fields(),
                    expectation,
                )
                .map(Some)
            }
            HirExprKind::Select(select) => self
                .check_select_expression(context, owner, select)
                .map(Some),
            _ => Ok(None),
        }
    }

    fn prepare_project_record_expression(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        owner: ExprId,
        declaration: &ProjectNominalDeclaration,
        fields: &[HirRecordField],
        expectation: &AnalyzerExpressionExpectation<'_>,
    ) -> Result<PreparedExpressionFact, AnalyzerExpressionError> {
        let (ty, type_selection, fields) =
            self.check_project_record_fields(context, owner, declaration, fields, expectation)?;
        let nominal =
            checked_project_nominal(declaration, &ty).map_err(AnalyzerExpressionError::fatal)?;
        Ok(PreparedExpressionFact::ProjectRecord(
            crate::final_analysis::PreparedProjectRecordExpression::new(
                super::PreparedExpressionShell::new(ty, type_selection, EffectSet::new()),
                nominal,
                fields,
            ),
        ))
    }

    fn prepare_entry_reference(
        &self,
        owner: ExprId,
        reference: &HirIdRef,
        expected: Option<&TypeKind>,
    ) -> Result<PreparedExpressionFact, AnalyzerExpressionError> {
        let HirIdRef::Absolute(reference) = reference else {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::ValueResolutionFailed { owner },
            ));
        };
        let public_id = arcweft_id::PublicId::try_new(reference.as_str()).map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::ValueResolutionFailed {
                owner,
            })
        })?;
        let mut matches = self.executable.items().filter_map(|item| {
            let HirItemKind::Entry(entry) = item.item().kind() else {
                return None;
            };
            let HirIdRef::Absolute(candidate) = entry.id().value()?.as_resolved()? else {
                return None;
            };
            (candidate == reference).then_some(item.id())
        });
        let lookup_owner = matches.next().ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::ValueResolutionFailed {
                owner,
            })
        })?;
        if matches.next().is_some() {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::ValueResolutionFailed { owner },
            ));
        }
        let type_selection = if expected.is_some() {
            CheckedTypeSelection::Expected
        } else {
            CheckedTypeSelection::Inferred
        };
        let prepared = super::PreparedEntryExpression::new(
            super::PreparedEntryReference::new(public_id, lookup_owner),
            type_selection,
        );
        if expected.is_some_and(|expected| !expected.accepts(prepared.shell().ty())) {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        Ok(PreparedExpressionFact::Entry(prepared))
    }

    fn prepare_project_variant_expression(
        &self,
        owner: ExprId,
        expected: Option<&TypeKind>,
        name: &arcweft_lang_hir::leaf::HirName,
        constructor_head: bool,
    ) -> Result<PreparedExpressionFact, AnalyzerExpressionError> {
        let Some(expected @ TypeKind::ProjectNominal(expected_nominal)) = expected else {
            return Err(AnalyzerExpressionError::rejected(owner));
        };
        let declaration = self
            .symbols
            .nominal(expected_nominal.declaration())
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        let ProjectNominalBody::Enum { variants } = declaration.body() else {
            return Err(AnalyzerExpressionError::rejected(owner));
        };
        let substitutions = nominal_substitutions(declaration, expected_nominal)
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        let mut selected_ordinal = None;
        let mut cases = Vec::with_capacity(variants.len());
        for (ordinal, variant) in variants.iter().enumerate() {
            let ordinal = u32::try_from(ordinal).map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::AccountingOverflow)
            })?;
            let payload = variant
                .payload()
                .map(|payload| {
                    self.types
                        .get(&payload)
                        .map(|payload| substitutions.apply(payload))
                        .ok_or_else(|| {
                            AnalyzerExpressionError::fatal(
                                FinalSemanticAnalysisError::TypeResolutionFailed { owner: payload },
                            )
                        })
                })
                .transpose()?;
            if variant.name().as_str() == name.as_str() {
                if payload.is_some() && !constructor_head {
                    return Err(AnalyzerExpressionError::rejected(owner));
                }
                selected_ordinal = Some(ordinal);
            }
            cases.push(super::PreparedVariantCaseSeed::new(
                ordinal,
                payload,
                Some(variant.name().as_str().to_owned()),
            ));
        }
        let selected_ordinal =
            selected_ordinal.ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        let nominal = checked_project_nominal(declaration, expected)
            .map_err(AnalyzerExpressionError::fatal)?;
        let seed =
            super::PreparedProjectVariantOwnerSeed::try_new(nominal, cases).ok_or_else(|| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidNominalOwner)
            })?;
        let prepared = super::PreparedProjectVariantExpression::try_new(
            super::PreparedExpressionShell::new(
                expected.clone(),
                CheckedTypeSelection::Expected,
                EffectSet::new(),
            ),
            seed,
            selected_ordinal,
        )
        .ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidNominalOwner)
        })?;
        Ok(PreparedExpressionFact::ProjectVariant(prepared))
    }

    fn check_expression_kind<E>(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expectation: &AnalyzerExpressionExpectation<'_>,
        transaction_authority: &CandidateFactTransactionAuthority<'_>,
    ) -> Result<CheckedExpression, E>
    where
        E: From<AnalyzerExpressionError> + From<CandidateFactOperationFailure>,
    {
        let expected = expectation.contextual_shape();
        if expression.is_poisoned() {
            return Err(
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::RecoveredOwner).into(),
            );
        }
        if let Some(checked) =
            self.check_style_expression_kind(context, module, owner, expression)?
        {
            return Ok(checked);
        }
        if let Some(checked) =
            self.check_leaf_expression_kind(module, owner, expression, expected)?
        {
            return Ok(checked);
        }
        if let Some(checked) =
            self.check_sequence_expression_kind(context, owner, expression, expected)?
        {
            return Ok(checked);
        }
        if let Some(checked) =
            self.check_binary_expression_kind(context, module, owner, expression, expected)?
        {
            return Ok(checked);
        }
        if let Some(checked) =
            self.check_unary_expression_kind(context, module, owner, expression, expected)?
        {
            return Ok(checked);
        }
        if let Some(checked) =
            self.check_control_expression_kind(context, module, owner, expression, expected)?
        {
            return Ok(checked);
        }
        if let Some(checked) =
            self.check_closure_expression_kind(context, module, owner, expression, expected)?
        {
            return Ok(checked);
        }
        if let Some(checked) =
            self.check_flow_expression_kind(context, module, owner, expression, expected)?
        {
            return Ok(checked);
        }
        if let Some(checked) =
            self.check_aggregate_expression_kind(context, module, owner, expression, expectation)?
        {
            return Ok(checked);
        }
        if let Some(checked) = self.check_variant_expression_kind(
            context,
            owner,
            expression,
            expected,
            expectation.is_enum_constructor_head(),
        )? {
            return Ok(checked);
        }
        self.check_entity_expression_kind::<E>(
            context,
            module,
            owner,
            expression,
            expectation,
            transaction_authority,
        )?
        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))
        .map_err(E::from)
    }
    fn check_leaf_expression_kind(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, AnalyzerExpressionError> {
        match expression.kind() {
            HirExprKind::Unit => Ok(structural_expression(
                TypeKind::Unit,
                CheckedTypeSelection::Inferred,
            )),
            HirExprKind::Literal(literal) => {
                let (ty, selection) = literal_type(literal, expected)
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                Ok(CheckedExpression::new(
                    ty,
                    selection,
                    EffectSet::new(),
                    CheckedExpressionResolution::Literal(literal.clone()),
                ))
            }
            HirExprKind::Path(path) => {
                let path = path.as_resolved().ok_or_else(|| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::RecoveredOwner)
                })?;
                if let Some(resolution) = self
                    .resolve_path_value(module, owner, expression.scope(), path)
                    .map_err(AnalyzerExpressionError::fatal)?
                {
                    let ty = match &resolution {
                        CheckedValueResolution::Local(local) => {
                            self.facts.locals().get(local).cloned()
                        }
                        _ => value_resolution_type(self.catalogs.world, &resolution),
                    };
                    let ty = ty.ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                    if let CheckedValueResolution::Local(local) = &resolution {
                        self.record_implicit_capture(owner, *local)
                            .map_err(AnalyzerExpressionError::fatal)?;
                    }
                    return Ok(Some(CheckedExpression::new(
                        ty,
                        CheckedTypeSelection::Inferred,
                        EffectSet::new(),
                        CheckedExpressionResolution::Value(resolution),
                    )));
                }
                if let (
                    Some(expected),
                    HirPathRoot::ImplicitCrate,
                    [HirPathSegment::Identifier(name)],
                ) = (expected, path.root(), path.segments())
                {
                    let (variant_owner, ordinal) =
                        self.resolve_short_variant(owner, expected, name, false)?;
                    return Ok(Some(CheckedExpression::new(
                        expected.clone(),
                        CheckedTypeSelection::Expected,
                        EffectSet::new(),
                        CheckedExpressionResolution::Variant(
                            super::CheckedVariantResolution::try_new(variant_owner, ordinal)
                                .ok_or_else(|| {
                                    AnalyzerExpressionError::fatal(
                                        FinalSemanticAnalysisError::WrongPayloadFamily,
                                    )
                                })?,
                        ),
                    )));
                }
                let (ty, variant) = resolve_closed_variant_path(
                    self.catalogs.world.environment().typecheck_env(),
                    path,
                    owner,
                )
                .map_err(AnalyzerExpressionError::fatal)?
                .ok_or_else(|| {
                    AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::ValueResolutionFailed { owner },
                    )
                })?;
                Ok(CheckedExpression::new(
                    ty,
                    CheckedTypeSelection::Inferred,
                    EffectSet::new(),
                    CheckedExpressionResolution::Variant(variant),
                ))
            }
            _ => return Ok(None),
        }
        .map(Some)
    }
    fn check_sequence_expression_kind(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, AnalyzerExpressionError> {
        match expression.kind() {
            HirExprKind::Tuple(tuple) => {
                let children = self.check_expressions(context, tuple.elements(), None)?;
                Ok(structural_expression(
                    TypeKind::Tuple(
                        children
                            .into_iter()
                            .map(|value| value.ty().clone())
                            .collect(),
                    ),
                    CheckedTypeSelection::Inferred,
                ))
            }
            HirExprKind::BracketSequence(sequence) => {
                let children = self.check_expressions(context, sequence.elements(), None)?;
                let item = common_type(
                    children.iter().map(PreparedExpressionFact::ty),
                    expected_item(expected),
                )
                .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                Ok(structural_expression(
                    TypeKind::Vec(Box::new(item)),
                    if expected.is_some() {
                        CheckedTypeSelection::Expected
                    } else {
                        CheckedTypeSelection::Inferred
                    },
                ))
            }
            HirExprKind::NumericBracketSequence(sequence) => {
                let item = integer_suffix_type(sequence.common_suffix())
                    .or_else(|| expected_item(expected).cloned())
                    .unwrap_or(TypeKind::I32);
                Ok(structural_expression(
                    TypeKind::Vec(Box::new(item)),
                    if sequence.common_suffix().is_some() {
                        CheckedTypeSelection::Explicit
                    } else if expected.is_some() {
                        CheckedTypeSelection::Expected
                    } else {
                        CheckedTypeSelection::DefaultNumericFallback
                    },
                ))
            }
            HirExprKind::ArrayRepeat(repeat) => {
                let value =
                    self.evaluate_expression(context, repeat.value(), expected_item(expected))?;
                self.evaluate_expression(context, repeat.length(), Some(&TypeKind::USize))?;
                Ok(structural_expression(
                    TypeKind::Array {
                        item: Box::new(value.ty().clone()),
                        len: ArrayLength::Inferred,
                    },
                    CheckedTypeSelection::Inferred,
                ))
            }
            HirExprKind::Range(range) => {
                let mut bounds = Vec::new();
                if let Some(start) = range.start() {
                    bounds.push(self.evaluate_expression(
                        context,
                        start,
                        expected_item(expected),
                    )?);
                }
                if let Some(end) = range.end() {
                    bounds.push(self.evaluate_expression(context, end, expected_item(expected))?);
                }
                let item = common_type(
                    bounds.iter().map(super::PreparedExpressionFact::ty),
                    expected_item(expected),
                )
                .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                Ok(structural_expression(
                    TypeKind::Range(Box::new(item)),
                    CheckedTypeSelection::Inferred,
                ))
            }
            _ => return Ok(None),
        }
        .map(Some)
    }
    fn check_binary_expression_kind(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        _module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, AnalyzerExpressionError> {
        match expression.kind() {
            HirExprKind::Binary(binary) => {
                let left = self.evaluate_expression(context, binary.left(), None)?;
                let right = self.evaluate_expression(context, binary.right(), Some(left.ty()))?;
                let ty = match binary.operator() {
                    HirBinaryOp::Implies
                    | HirBinaryOp::Or
                    | HirBinaryOp::And
                    | HirBinaryOp::In
                    | HirBinaryOp::Equal
                    | HirBinaryOp::NotEqual
                    | HirBinaryOp::GreaterOrEqual
                    | HirBinaryOp::LessOrEqual
                    | HirBinaryOp::Greater
                    | HirBinaryOp::Less => TypeKind::Bool,
                    HirBinaryOp::Merge
                    | HirBinaryOp::Add
                    | HirBinaryOp::Subtract
                    | HirBinaryOp::Multiply
                    | HirBinaryOp::Divide
                    | HirBinaryOp::Remainder => common_type([left.ty(), right.ty()], expected)
                        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?,
                };
                Ok(structural_expression(ty, CheckedTypeSelection::Inferred))
            }
            _ => return Ok(None),
        }
        .map(Some)
    }

    fn check_unary_expression_kind(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, AnalyzerExpressionError> {
        match expression.kind() {
            HirExprKind::Unary(unary) => {
                let operand = self.evaluate_expression(context, unary.operand(), expected)?;
                let ty = match unary.operator() {
                    HirUnaryOp::Not => TypeKind::Bool,
                    HirUnaryOp::Negate => operand.ty().clone(),
                };
                Ok(structural_expression(ty, CheckedTypeSelection::Inferred))
            }
            HirExprKind::Borrow(borrow) => {
                let operand = self.evaluate_expression(context, borrow.operand(), None)?;
                let kind = match borrow.kind() {
                    HirBorrowKind::Shared => BorrowKind::Shared,
                    HirBorrowKind::Mutable => BorrowKind::Mutable,
                };
                Ok(structural_expression(
                    TypeKind::BorrowRef {
                        kind,
                        lifetime: None,
                        inner: Box::new(operand.ty().clone()),
                    },
                    CheckedTypeSelection::Inferred,
                ))
            }
            HirExprKind::Dereference(dereference) => {
                let operand = self.evaluate_expression(context, dereference.operand(), None)?;
                let TypeKind::BorrowRef { inner, .. } = operand.ty() else {
                    return Err(AnalyzerExpressionError::rejected(owner));
                };
                Ok(structural_expression(
                    inner.as_ref().clone(),
                    CheckedTypeSelection::Inferred,
                ))
            }
            HirExprKind::Index(index) => {
                let target = self.evaluate_expression(context, index.target(), None)?;
                let ty = indexed_item(target.ty())
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                self.evaluate_expression(context, index.index(), None)?;
                Ok(structural_expression(ty, CheckedTypeSelection::Inferred))
            }
            HirExprKind::Try(operation) => {
                let operand = self.evaluate_expression(context, operation.operand(), None)?;
                let carrier = match operand.ty() {
                    TypeKind::Result { ok, error } => CheckedTryCarrier::Result {
                        success: ok.as_ref().clone(),
                        residual: error.clone(),
                    },
                    TypeKind::Option(value) => CheckedTryCarrier::Option {
                        success: value.as_ref().clone(),
                    },
                    _ => {
                        return Err(AnalyzerExpressionError::rejected(owner));
                    }
                };
                let boundary =
                    self.resolve_try_boundary(module, owner, expression.scope(), &carrier)?;
                Ok(CheckedExpression::new(
                    carrier.success().clone(),
                    CheckedTypeSelection::Inferred,
                    operand.effects().clone(),
                    CheckedExpressionResolution::Try(CheckedTry::new(
                        operation.operand(),
                        carrier,
                        boundary,
                    )),
                ))
            }
            HirExprKind::Await(operation) => {
                let operand = self.evaluate_expression(context, operation.operand(), None)?;
                let (ty, resolution) = match operand.ty() {
                    TypeKind::Need(item) => {
                        let observers =
                            self.check_await_pending_observers(module, operation.branches())?;
                        (
                            item.as_ref().clone(),
                            CheckedExpressionResolution::Await(CheckedAwait::new(
                                operation.operand(),
                                observers,
                            )),
                        )
                    }
                    TypeKind::ThreadHandle(value) => (
                        value.as_ref().clone(),
                        CheckedExpressionResolution::Structural,
                    ),
                    _ => {
                        return Err(AnalyzerExpressionError::rejected(owner));
                    }
                };
                Ok(CheckedExpression::new(
                    ty,
                    CheckedTypeSelection::Inferred,
                    EffectSet::from_labels(["control.suspend"])
                        .expect("the language-owned suspension effect is valid"),
                    resolution,
                ))
            }
            _ => return Ok(None),
        }
        .map(Some)
    }

    fn check_await_pending_observers(
        &mut self,
        module: &HirModule,
        branches: &[arcweft_lang_hir::expr::HirAwaitBranch],
    ) -> Result<Vec<CheckedAwaitPendingObserver>, AnalyzerExpressionError> {
        branches
            .iter()
            .map(|branch| {
                if branch.kind() == HirAwaitBranchKind::Recovered {
                    return Err(AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::RecoveredOwner,
                    ));
                }
                if branch.kind() != HirAwaitBranchKind::Pending {
                    return Err(AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::WrongPayloadFamily,
                    ));
                }
                let pattern = branch.pattern().ok_or_else(|| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::RecoveredOwner)
                })?;
                self.seed_contextual_pattern_locals(module, pattern, &TypeKind::Progress)
                    .map_err(AnalyzerExpressionError::fatal)?;
                Ok(CheckedAwaitPendingObserver::new(pattern))
            })
            .collect()
    }

    fn check_control_expression_kind(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, AnalyzerExpressionError> {
        match expression.kind() {
            HirExprKind::Block(block) => {
                self.evaluate_implicit_statement_uses(context, module, block.statements())?;
                let tail = self.evaluate_expression(context, block.tail(), expected)?;
                Ok(structural_expression(
                    tail.ty().clone(),
                    tail.type_selection(),
                ))
            }
            HirExprKind::ComputationBlock(block) => {
                self.infer_nested_expression_bindings(owner)
                    .map_err(AnalyzerExpressionError::fatal)?;
                self.evaluate_implicit_statement_uses(context, module, block.statements())?;
                let expected_success = match (block.kind(), expected) {
                    (HirComputationBlockKind::Result, Some(TypeKind::Result { ok, .. })) => {
                        Some(ok.as_ref())
                    }
                    (HirComputationBlockKind::Option, Some(TypeKind::Option(item))) => {
                        Some(item.as_ref())
                    }
                    _ => None,
                };
                let tail = self.evaluate_expression(context, block.tail(), expected_success)?;
                let ty = match block.kind() {
                    HirComputationBlockKind::Result => {
                        let expected_error = match expected {
                            Some(TypeKind::Result { error, .. }) => Some(error.as_ref()),
                            _ => None,
                        };
                        let residuals = self.try_residuals_for_block(owner);
                        let error = if let Some(expected) = expected_error {
                            if residuals.iter().all(|residual| expected.accepts(residual)) {
                                expected.clone()
                            } else {
                                return Err(AnalyzerExpressionError::rejected(owner));
                            }
                        } else if residuals.is_empty() {
                            TypeKind::Never
                        } else {
                            common_type(residuals.iter().copied(), None)
                                .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?
                        };
                        TypeKind::Result {
                            ok: Box::new(tail.ty().clone()),
                            error: Box::new(error),
                        }
                    }
                    HirComputationBlockKind::Option => {
                        TypeKind::Option(Box::new(tail.ty().clone()))
                    }
                    HirComputationBlockKind::Seq => TypeKind::Seq(Box::new(tail.ty().clone())),
                    HirComputationBlockKind::Stream => TypeKind::Stream {
                        item: Box::new(tail.ty().clone()),
                        error: Box::new(TypeKind::Unit),
                    },
                };
                Ok(structural_expression(ty, CheckedTypeSelection::Inferred))
            }
            HirExprKind::NamedBlock(block) => {
                self.evaluate_implicit_statement_uses(context, module, block.statements())?;
                let tail = self.evaluate_expression(context, block.tail(), expected)?;
                Ok(structural_expression(
                    tail.ty().clone(),
                    tail.type_selection(),
                ))
            }
            HirExprKind::Loop(loop_expression) => {
                self.evaluate_implicit_statement_uses(
                    context,
                    module,
                    loop_expression.statements(),
                )?;
                self.evaluate_expression(context, loop_expression.tail(), None)?;
                let mut exits = Vec::new();
                for (_, statement) in module.statements() {
                    let HirStmtKind::Break { label: None, value } = statement.kind() else {
                        continue;
                    };
                    if !break_targets_loop(module, statement.scope(), owner)
                        .map_err(AnalyzerExpressionError::fatal)?
                    {
                        continue;
                    }
                    if let Some(value) = value {
                        exits.push(self.evaluate_expression(context, *value, expected)?);
                    } else {
                        exits.push(
                            structural_expression(TypeKind::Unit, CheckedTypeSelection::Inferred)
                                .into(),
                        );
                    }
                }
                let (ty, selection) = if exits.is_empty() {
                    (TypeKind::Never, CheckedTypeSelection::Inferred)
                } else {
                    let ty = common_type(
                        exits.iter().map(super::PreparedExpressionFact::ty),
                        expected,
                    )
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                    (ty, CheckedTypeSelection::Inferred)
                };
                Ok(structural_expression(ty, selection))
            }
            HirExprKind::If(conditional) => {
                self.evaluate_expression(context, conditional.condition(), Some(&TypeKind::Bool))?;
                let then_value =
                    self.evaluate_expression(context, conditional.then_branch(), expected)?;
                let else_value = self.evaluate_expression(
                    context,
                    conditional.else_branch(),
                    Some(then_value.ty()),
                )?;
                let ty = common_type([then_value.ty(), else_value.ty()], expected)
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                Ok(structural_expression(ty, CheckedTypeSelection::Inferred))
            }
            HirExprKind::IfLet(conditional) => {
                let scrutinee = self.evaluate_expression(context, conditional.scrutinee(), None)?;
                self.seed_contextual_pattern_locals(module, conditional.pattern(), scrutinee.ty())
                    .map_err(AnalyzerExpressionError::fatal)?;
                if let Some(guard) = conditional.guard() {
                    self.evaluate_expression(context, guard, Some(&TypeKind::Bool))?;
                }
                let then_value =
                    self.evaluate_expression(context, conditional.then_branch(), expected)?;
                let else_value = self.evaluate_expression(
                    context,
                    conditional.else_branch(),
                    Some(then_value.ty()),
                )?;
                let ty = common_type([then_value.ty(), else_value.ty()], expected)
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                Ok(structural_expression(ty, CheckedTypeSelection::Inferred))
            }
            HirExprKind::Match(match_expr) => {
                let scrutinee = self.evaluate_expression(context, match_expr.scrutinee(), None)?;
                let mut values = Vec::new();
                let mut arms = Vec::with_capacity(match_expr.arms().len());
                for arm in match_expr.arms() {
                    self.seed_contextual_pattern_locals(module, arm.pattern(), scrutinee.ty())
                        .map_err(AnalyzerExpressionError::fatal)?;
                    if let Some(guard) = arm.guard() {
                        self.evaluate_expression(context, guard, Some(&TypeKind::Bool))?;
                    }
                    values.push(self.evaluate_expression(context, arm.value(), expected)?);
                    arms.push(crate::final_analysis::CheckedMatchArmFact::new(
                        arm.guard(),
                        arm.value(),
                    ));
                }
                let ty = common_type(
                    values.iter().map(super::PreparedExpressionFact::ty),
                    expected,
                )
                .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                Ok(
                    structural_expression(ty, CheckedTypeSelection::Inferred).with_match_fact(
                        crate::final_analysis::CheckedMatchFact::new(
                            match_expr.scrutinee(),
                            arms.into_boxed_slice(),
                        ),
                    ),
                )
            }
            _ => return Ok(None),
        }
        .map(Some)
    }

    /// Evaluates statement-owned expression uses while an implicit callable
    /// transaction is active. Statement children are reached through HIR's
    /// typed child-edge authority; no closure/capture arena is scanned. Cached
    /// local facts still emit the use-time capture event in
    /// `evaluate_expression`, so target and value uses merge into the same
    /// terminal producer row with `Reassign` priority.
    fn evaluate_implicit_statement_uses(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        statements: &[super::StmtId],
    ) -> Result<(), AnalyzerExpressionError> {
        if self.implicit_callable_stack.is_empty() {
            return Ok(());
        }
        enum Work {
            Statement(super::StmtId),
            Expression(ExprId),
        }
        let mut pending = statements
            .iter()
            .rev()
            .copied()
            .map(Work::Statement)
            .collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        while let Some(work) = pending.pop() {
            match work {
                Work::Expression(expression) => {
                    self.evaluate_expression(context, expression, None)?;
                }
                Work::Statement(statement) => {
                    if !seen.insert(statement) {
                        return Err(AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        ));
                    }
                    let statement = module.resolve_stmt(statement).map_err(|_| {
                        AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                    })?;
                    let edges = statement.kind().try_child_edges().map_err(|_| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::AccountingOverflow,
                        )
                    })?;
                    for edge in edges.into_iter().rev() {
                        match edge.child() {
                            arcweft_lang_hir::stmt::HirStatementChild::Expression(expression) => {
                                pending.push(Work::Expression(expression));
                            }
                            arcweft_lang_hir::stmt::HirStatementChild::Statement(statement) => {
                                pending.push(Work::Statement(statement));
                            }
                            arcweft_lang_hir::stmt::HirStatementChild::Pattern(_)
                            | arcweft_lang_hir::stmt::HirStatementChild::Type(_)
                            | arcweft_lang_hir::stmt::HirStatementChild::Local(_) => {}
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn try_residuals_for_block(&self, owner: ExprId) -> Vec<&TypeKind> {
        self.facts
            .expressions()
            .values()
            .filter_map(|expression| match expression.checked_resolution() {
                Some(CheckedExpressionResolution::Try(tried))
                    if tried.boundary() == CheckedTryBoundary::CarrierBlock(owner) =>
                {
                    tried.carrier().residual()
                }
                _ => None,
            })
            .collect()
    }

    fn resolve_try_boundary(
        &self,
        module: &HirModule,
        owner: ExprId,
        mut scope: ScopeId,
        carrier: &CheckedTryCarrier,
    ) -> Result<CheckedTryBoundary, AnalyzerExpressionError> {
        if matches!(
            carrier,
            CheckedTryCarrier::Result { residual, .. }
                if matches!(residual.as_ref(), TypeKind::Never)
        ) {
            return Ok(CheckedTryBoundary::Infallible);
        }
        loop {
            let current = module.resolve_scope(scope).map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
            })?;
            match current.owner() {
                HirScopeOwner::Expr(expression) => {
                    let expression_kind = module
                        .resolve_expr(*expression)
                        .map_err(|_| {
                            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                        })?
                        .kind();
                    if let HirExprKind::ComputationBlock(block) = expression_kind {
                        let matches = matches!(
                            (block.kind(), carrier),
                            (
                                HirComputationBlockKind::Result,
                                CheckedTryCarrier::Result { .. }
                            ) | (
                                HirComputationBlockKind::Option,
                                CheckedTryCarrier::Option { .. }
                            )
                        );
                        return matches
                            .then_some(CheckedTryBoundary::CarrierBlock(*expression))
                            .ok_or_else(|| AnalyzerExpressionError::rejected(owner));
                    }
                    if current.kind() == HirScopeKind::Closure {
                        let context = self
                            .function_site_stack
                            .iter()
                            .rev()
                            .find(|context| context.owner == *expression)
                            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                        let matches = match (carrier, &context.result) {
                            (
                                CheckedTryCarrier::Result { residual, .. },
                                TypeKind::Result { error, .. },
                            ) => error.accepts(residual),
                            (CheckedTryCarrier::Option { .. }, TypeKind::Option(_)) => true,
                            _ => false,
                        };
                        return matches
                            .then_some(CheckedTryBoundary::FunctionSite(context.owner))
                            .ok_or_else(|| AnalyzerExpressionError::rejected(owner));
                    }
                }
                HirScopeOwner::Item(item) => {
                    let expression_uses = self
                        .topology
                        .module(owner.module())
                        .ok_or_else(|| {
                            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                        })?
                        .expression_uses();
                    if let Some(context) =
                        self.implicit_callable_stack.iter().rev().find(|context| {
                            expression_uses
                                .implicit_callable_region(
                                    context.owner,
                                    HirPlaceholderKind::PartialApplication,
                                )
                                .is_ok_and(|region| region.contains_expression(owner))
                        })
                    {
                        let boundary = context
                            .result
                            .as_ref()
                            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                        let matches = match (carrier, boundary) {
                            (
                                CheckedTryCarrier::Result { residual, .. },
                                TypeKind::Result { error, .. },
                            ) => error.accepts(residual),
                            (CheckedTryCarrier::Option { .. }, TypeKind::Option(_)) => true,
                            _ => false,
                        };
                        return matches
                            .then_some(CheckedTryBoundary::FunctionSite(context.owner))
                            .ok_or_else(|| AnalyzerExpressionError::rejected(owner));
                    }
                    return self.resolve_item_try_boundary(module, owner, *item, carrier);
                }
                HirScopeOwner::Module(_) | HirScopeOwner::Stmt(_) => {}
            }
            let Some(parent) = current.parent() else {
                return Err(AnalyzerExpressionError::rejected(owner));
            };
            scope = parent;
        }
    }

    fn resolve_item_try_boundary(
        &self,
        module: &HirModule,
        owner: ExprId,
        item: super::ItemId,
        carrier: &CheckedTryCarrier,
    ) -> Result<CheckedTryBoundary, AnalyzerExpressionError> {
        let item_kind = module
            .resolve_item(item)
            .map_err(|_| AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner))?
            .kind();
        let return_type = match item_kind {
            HirItemKind::Function(function) => function.return_type(),
            HirItemKind::Flow(flow) => flow.result().authored_type(),
            _ => None,
        }
        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        let boundary = self.types.get(&return_type).ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::TypeResolutionFailed {
                owner: return_type,
            })
        })?;
        let matches = match (carrier, boundary) {
            (CheckedTryCarrier::Result { residual, .. }, TypeKind::Result { error, .. }) => {
                error.accepts(residual)
            }
            (CheckedTryCarrier::Option { .. }, TypeKind::Option(_)) => true,
            _ => false,
        };
        if let (CheckedTryCarrier::Result { residual, .. }, TypeKind::Result { error, .. }) =
            (carrier, boundary)
            && !matches
        {
            return Err(Self::try_error_mismatch(
                module,
                owner,
                return_type,
                residual,
                error,
            )?);
        }
        matches
            .then_some(CheckedTryBoundary::Callable(item))
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))
    }

    fn try_error_mismatch(
        module: &HirModule,
        owner: ExprId,
        return_type: arcweft_lang_hir::identity::TypeId,
        operand_error: &TypeKind,
        return_error: &TypeKind,
    ) -> Result<AnalyzerExpressionError, AnalyzerExpressionError> {
        Ok(AnalyzerExpressionError::fatal(
            FinalSemanticAnalysisError::PropagationErrorMismatch {
                owner,
                operand_error: Box::new(operand_error.clone()),
                return_error: Box::new(return_error.clone()),
                operator_source: source_span_for_role(
                    module,
                    HirSourceQuery::Expr {
                        owner,
                        role: arcweft_lang_hir::source_index::HirExprSourceRole::Operator,
                    },
                )
                .map_err(AnalyzerExpressionError::fatal)?,
                return_source: source_span_for_role(
                    module,
                    HirSourceQuery::Type {
                        owner: return_type,
                        role: HirTypeSourceRole::Whole,
                    },
                )
                .map_err(AnalyzerExpressionError::fatal)?,
            },
        ))
    }
    fn check_closure_expression_kind(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, AnalyzerExpressionError> {
        match expression.kind() {
            HirExprKind::Closure(closure) => {
                let contextual_function = match expected {
                    Some(TypeKind::Function {
                        params,
                        return_type,
                        ..
                    }) if params.len() == closure.parameters().len() => {
                        Some((params.as_slice(), return_type.as_ref()))
                    }
                    Some(TypeKind::Function { .. }) | None => None,
                    Some(_) => {
                        return Err(AnalyzerExpressionError::rejected(owner));
                    }
                };
                if expected.is_some() && contextual_function.is_none() {
                    return Err(AnalyzerExpressionError::rejected(owner));
                }
                let mut parameters = Vec::with_capacity(closure.parameters().len());
                for (index, parameter) in closure.parameters().iter().enumerate() {
                    let annotated = parameter.ty().and_then(|id| self.types.get(&id)).cloned();
                    let contextual = contextual_function.map(|(params, _)| &params[index]);
                    let parameter_ty = match (annotated, contextual) {
                        (Some(annotated), Some(contextual)) if contextual.accepts(&annotated) => {
                            annotated
                        }
                        (Some(_), Some(_)) => {
                            return Err(AnalyzerExpressionError::rejected(owner));
                        }
                        (Some(annotated), None) => annotated,
                        (None, Some(contextual)) => contextual.clone(),
                        (None, None) => self
                            .pattern_type_hint(module, parameter.pattern())
                            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?,
                    };
                    self.seed_contextual_pattern_locals(module, parameter.pattern(), &parameter_ty)
                        .map_err(AnalyzerExpressionError::fatal)?;
                    parameters.push(parameter_ty);
                }
                let declared_result = closure
                    .result_type()
                    .and_then(|id| self.types.get(&id))
                    .cloned();
                let contextual_result = contextual_function.map(|(_, result)| result);
                if let (Some(declared), Some(contextual)) =
                    (declared_result.as_ref(), contextual_result)
                    && !contextual.accepts(declared)
                {
                    return Err(AnalyzerExpressionError::rejected(owner));
                }
                let body_expected = declared_result.as_ref().or(contextual_result);
                if let Some(result) = body_expected {
                    self.function_site_stack.push(super::FunctionSiteContext {
                        owner,
                        result: result.clone(),
                    });
                }
                let body = self.evaluate_expression(context, closure.body(), body_expected);
                if body_expected.is_some() {
                    self.function_site_stack
                        .pop()
                        .expect("function-site context was just pushed");
                }
                let body = body?;
                let result = if matches!(
                    body.checked_resolution(),
                    Some(CheckedExpressionResolution::Try(tried))
                        if tried.boundary() == CheckedTryBoundary::FunctionSite(owner)
                ) {
                    body_expected
                        .cloned()
                        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?
                } else {
                    body.ty().clone()
                };
                let ty = TypeKind::function_with_effects(
                    parameters,
                    result,
                    crate::effect_row::EffectRow::closed(body.effects().clone()),
                );
                if expected.is_some_and(|expected| !expected.accepts(&ty)) {
                    return Err(AnalyzerExpressionError::rejected(owner));
                }
                let checked_closure = CheckedClosure::seal(Arc::clone(&self.topology), owner)
                    .map_err(|violation| {
                        AnalyzerExpressionError::invariant(FinalSemanticAnalysisError::from(
                            violation,
                        ))
                    })?;
                Ok(CheckedExpression::new(
                    ty,
                    if expected.is_some() {
                        CheckedTypeSelection::Expected
                    } else {
                        CheckedTypeSelection::Inferred
                    },
                    EffectSet::new(),
                    CheckedExpressionResolution::Closure(checked_closure),
                ))
            }
            _ => return Ok(None),
        }
        .map(Some)
    }
    fn check_flow_expression_kind(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, AnalyzerExpressionError> {
        match expression.kind() {
            HirExprKind::Pipe(pipe) => {
                let left = self.evaluate_expression(context, pipe.left(), None)?;
                let placeholders = self
                    .topology
                    .module(owner.module())
                    .ok_or_else(|| {
                        AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                    })?
                    .expression_uses()
                    .implicit_callable_region(pipe.right(), HirPlaceholderKind::PipeLeft)
                    .map_err(|_| {
                        AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                    })?
                    .placeholders()
                    .collect::<BTreeSet<_>>();
                self.pipe_stack.push(super::PipeContext {
                    owner,
                    left: pipe.left(),
                    right: pipe.right(),
                    value: left.ty().clone(),
                    placeholders: placeholders.clone(),
                });
                let right = self.evaluate_expression(context, pipe.right(), expected);
                self.pipe_stack.pop().expect("pipe context was just pushed");
                let right = right?;
                let mut effects = left.effects().clone();
                effects.union_with(right.effects());
                Ok(CheckedExpression::new(
                    right.ty().clone(),
                    right.type_selection(),
                    effects,
                    CheckedExpressionResolution::Pipe(CheckedPipe::new(
                        pipe.left(),
                        pipe.right(),
                        placeholders.into_iter().collect(),
                    )),
                ))
            }
            HirExprKind::ForSynthetic(synthetic) => {
                let input = self.evaluate_expression(context, synthetic.input(), expected)?;
                let ty = match synthetic {
                    arcweft_lang_hir::expr::HirForSyntheticExpr::Iterator { .. } => {
                        let iteration = self
                            .select_iteration(input.ty())
                            .map_err(AnalyzerExpressionError::fatal)?;
                        let ty = super::statements::iteration_iterator(&iteration);
                        if self
                            .facts
                            .set_iteration_fact(owner, iteration)
                            .map_err(AnalyzerExpressionError::fact)?
                        {
                            return Err(AnalyzerExpressionError::fatal(
                                FinalSemanticAnalysisError::WrongPayloadFamily,
                            ));
                        }
                        ty
                    }
                    arcweft_lang_hir::expr::HirForSyntheticExpr::NextValue { .. } => {
                        let iteration = self
                            .facts
                            .iteration_facts()
                            .get(&synthetic.input())
                            .ok_or_else(|| AnalyzerExpressionError::rejected(synthetic.input()))?;
                        super::statements::iteration_item(iteration).clone()
                    }
                };
                Ok(structural_expression(ty, CheckedTypeSelection::Inferred))
            }
            HirExprKind::Thread(_) => {
                let mut effects = EffectSet::new();
                effects.insert(
                    EffectId::parse("control.spawn")
                        .expect("the language-owned Thread effect is a valid effect identity"),
                );
                Ok(CheckedExpression::new(
                    TypeKind::ThreadHandle(Box::new(TypeKind::Unit)),
                    CheckedTypeSelection::Inferred,
                    effects,
                    CheckedExpressionResolution::Structural,
                ))
            }
            HirExprKind::Choice(choice) => {
                self.check_choice_expression(context, module, owner, expression, choice, expected)
            }
            _ => return Ok(None),
        }
        .map(Some)
    }

    fn check_choice_expression(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        choice: &arcweft_lang_hir::expr::HirChoiceExpr,
        expected: Option<&TypeKind>,
    ) -> Result<CheckedExpression, AnalyzerExpressionError> {
        let public_id = choice
            .id()
            .map(|id| self.resolve_choice_public_id(module, expression.scope(), id))
            .transpose()?;
        let mut option_ids = Vec::with_capacity(choice.body().items().len());
        let mut gotos = Vec::new();
        let mut outputs = Vec::new();
        let mut effects = EffectSet::new();
        for (arm, item) in choice.body().items().iter().enumerate() {
            let HirChoiceItem::CompactArm(candidate) = item else {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::WrongPayloadFamily,
                ));
            };
            option_ids.push(
                resolve_choice_option_public_id(candidate.id(), public_id.as_ref())
                    .map_err(AnalyzerExpressionError::fatal)?,
            );
            let label =
                self.evaluate_expression(context, candidate.label(), Some(&TypeKind::String))?;
            effects.union_with(label.effects());
            if let Some(condition) = candidate.condition() {
                let condition =
                    self.evaluate_expression(context, condition, Some(&TypeKind::Bool))?;
                effects.union_with(condition.effects());
            }
            match candidate.action() {
                HirChoiceCompactAction::Goto(target) => {
                    let target = target.as_resolved().ok_or_else(|| {
                        AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::RecoveredOwner)
                    })?;
                    let target = self.resolve_choice_goto(module, owner, target)?;
                    gotos.push(CheckedChoiceGoto::new(
                        u32::try_from(arm).map_err(|_| {
                            AnalyzerExpressionError::fatal(
                                FinalSemanticAnalysisError::AccountingOverflow,
                            )
                        })?,
                        target,
                    ));
                }
                HirChoiceCompactAction::Out(value) => {
                    let value = self.evaluate_expression(context, *value, expected)?;
                    effects.union_with(value.effects());
                    outputs.push(value.ty().clone());
                }
                HirChoiceCompactAction::Missing => {
                    return Err(AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::RecoveredOwner,
                    ));
                }
            }
        }
        let ty = if outputs.is_empty() {
            TypeKind::Never
        } else {
            common_type(outputs.iter(), expected)
                .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?
        };
        if expected.is_some_and(|expected| !expected.accepts(&ty)) {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        Ok(CheckedExpression::new(
            ty,
            if expected.is_some() {
                CheckedTypeSelection::Expected
            } else {
                CheckedTypeSelection::Inferred
            },
            effects,
            CheckedExpressionResolution::Choice(CheckedChoice::new(public_id, option_ids, gotos)),
        ))
    }

    fn resolve_choice_goto(
        &self,
        module: &HirModule,
        owner: ExprId,
        target: &HirIdRef,
    ) -> Result<CheckedProjectItem, AnalyzerExpressionError> {
        let target = self
            .resolve_checked_entity_reference(
                module,
                target,
                expression_span(module, owner).map_err(AnalyzerExpressionError::fatal)?,
            )
            .map_err(|error| match error {
                EntityReferenceResolutionError::Lookup => AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::ValueResolutionFailed { owner },
                ),
                EntityReferenceResolutionError::WrongFamily => {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
                }
            })?;
        (target.family() == arcweft_id::DeclarationIdentityFamily::Flow)
            .then_some(target)
            .ok_or_else(|| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
            })
    }

    fn resolve_choice_public_id(
        &self,
        module: &HirModule,
        mut scope: ScopeId,
        value: &arcweft_lang_hir::leaf::HirIdRefValue,
    ) -> Result<arcweft_id::PublicId, AnalyzerExpressionError> {
        let reference = value.as_resolved().ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::RecoveredOwner)
        })?;
        if let HirIdRef::Absolute(value) = reference {
            return checked_choice_public_id(value.as_str())
                .map_err(AnalyzerExpressionError::fatal);
        }

        let mut named_scopes = Vec::new();
        let item = loop {
            let node = module.resolve_scope(scope).map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
            })?;
            if let HirScopeOwner::Item(owner) = node.owner() {
                break *owner;
            }
            if let HirScopeOwner::Stmt(owner) = node.owner()
                && let HirStmtKind::Scope(statement) = module
                    .resolve_stmt(*owner)
                    .map_err(|_| {
                        AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                    })?
                    .kind()
                && let Some(name) = statement.name()
            {
                named_scopes.push(name.as_str());
            }
            let Some(parent) = node.parent() else {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::WrongPayloadFamily,
                ));
            };
            scope = parent;
        };
        named_scopes.reverse();

        let symbol = self.symbols.flow_symbol_for_item(item).ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
        })?;
        let CallableDeclarationKey::Flow(flow) = symbol.declaration() else {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::WrongPayloadFamily,
            ));
        };
        let flow_path = flow
            .public_id()
            .as_str()
            .strip_prefix("flow.")
            .ok_or_else(|| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
            })?;
        let relative = match reference {
            HirIdRef::Relative(relative) => relative,
            HirIdRef::FamilyRelative(relative) if relative.family().as_str() == "choice" => {
                relative.relative()
            }
            HirIdRef::FamilyRelative(_) | HirIdRef::Absolute(_) => {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::WrongPayloadFamily,
                ));
            }
        };
        if relative.parent_depth() > named_scopes.len() {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::WrongPayloadFamily,
            ));
        }
        let retained_scope_count = named_scopes.len() - relative.parent_depth();
        let mut value = String::from("choice.");
        value.push_str(flow_path);
        for name in &named_scopes[..retained_scope_count] {
            value.push('.');
            value.push_str(name);
        }
        value.push('.');
        value.push_str(relative.suffix().as_str());
        checked_choice_public_id(&value).map_err(AnalyzerExpressionError::fatal)
    }
    fn check_aggregate_expression_kind(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expectation: &AnalyzerExpressionExpectation<'_>,
    ) -> Result<Option<CheckedExpression>, AnalyzerExpressionError> {
        let expected = expectation.contextual_shape();
        match expression.kind() {
            HirExprKind::Placeholder(HirPlaceholderKind::PartialApplication) => {
                let context = self
                    .implicit_callable_stack
                    .iter()
                    .rev()
                    .find(|context| context.placeholders.contains(&owner))
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                if expected.is_some_and(|expected| !expected.accepts(&context.parameter)) {
                    return Err(AnalyzerExpressionError::rejected(owner));
                }
                Ok(CheckedExpression::new(
                    context.parameter.clone(),
                    CheckedTypeSelection::Expected,
                    EffectSet::new(),
                    CheckedExpressionResolution::ImplicitParameter {
                        callable: context.owner,
                    },
                ))
            }
            HirExprKind::Placeholder(HirPlaceholderKind::PipeLeft) => {
                let context = self
                    .pipe_stack
                    .iter()
                    .rev()
                    .find(|context| context.placeholders.contains(&owner))
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                if expected.is_some_and(|expected| !expected.accepts(&context.value)) {
                    return Err(AnalyzerExpressionError::rejected(owner));
                }
                Ok(CheckedExpression::new(
                    context.value.clone(),
                    CheckedTypeSelection::Expected,
                    EffectSet::new(),
                    CheckedExpressionResolution::PipeLeft {
                        pipe: context.owner,
                    },
                ))
            }
            HirExprKind::Call(call) => {
                if let Some(checked) =
                    self.check_view_call_expression(context, module, expression, call)?
                {
                    Ok(checked)
                } else {
                    self.check_call_expression_in_context(
                        context,
                        module,
                        owner,
                        call,
                        expectation.child_call_result(),
                        None,
                    )
                }
            }
            _ => return Ok(None),
        }
        .map(Some)
    }

    fn check_select_expression(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        owner: ExprId,
        select: &arcweft_lang_hir::expr::HirSelectExpr,
    ) -> Result<PreparedExpressionFact, AnalyzerExpressionError> {
        let target = self.evaluate_expression(context, select.target(), None)?;
        let HirSelectedMember::Name(name) = select.member() else {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::RecoveredOwner,
            ));
        };
        let (ty, resolution) = if let Some((field, ty)) =
            target.ty().agent_field_type(name.as_str())
        {
            (ty, super::CheckedSelectResolution::AgentField { field })
        } else if let Some((field, ty)) = target.ty().progress_field(name.as_str()) {
            (ty, super::CheckedSelectResolution::ProgressField { field })
        } else {
            match target.ty() {
                TypeKind::ProjectNominal(target_nominal) => {
                    let declaration = self
                        .symbols
                        .nominal(target_nominal.declaration())
                        .cloned()
                        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                    let ProjectNominalBody::Struct { fields } = declaration.body() else {
                        return Err(AnalyzerExpressionError::rejected(owner));
                    };
                    let (ordinal, field) = fields
                        .iter()
                        .enumerate()
                        .find(|(_, field)| field.name().as_str() == name.as_str())
                        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                    let ordinal = u32::try_from(ordinal).map_err(|_| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
                        )
                    })?;
                    let declared_ty = self.types.get(&field.ty()).ok_or_else(|| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::TypeResolutionFailed { owner: field.ty() },
                        )
                    })?;
                    let substitutions = nominal_substitutions(&declaration, &target_nominal)
                        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                    let nominal = checked_project_nominal(&declaration, target.ty())
                        .map_err(AnalyzerExpressionError::fatal)?;
                    let ty = substitutions.apply(declared_ty);
                    return Ok(PreparedExpressionFact::ProjectField(
                        crate::final_analysis::PreparedProjectFieldExpression::new(
                            crate::final_analysis::PreparedExpressionShell::new(
                                ty.clone(),
                                CheckedTypeSelection::Inferred,
                                target.effects().clone(),
                            ),
                            nominal,
                            ordinal,
                            ty,
                            name.clone(),
                        ),
                    ));
                }
                TypeKind::Named(type_name) => {
                    let environment = self.catalogs.world.environment().typecheck_env();
                    let record = environment
                        .environment_record(type_name.as_str())
                        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                    let field = record
                        .field(name.as_str())
                        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                    let ty = field.ty().clone();
                    let selection = crate::final_analysis::CheckedFieldSelection::try_new(
                        record.semantic_type(),
                        crate::record_field::CheckedRecordFieldSemanticId::Environment(
                            field.semantic_id(),
                        ),
                        field.ordinal(),
                        None,
                        field.type_digest(),
                        name.clone(),
                    )
                    .expect("accepted environment record fields have canonical semantic rows");
                    let resolution = if let Some(projection) = environment
                        .dialogue_view_models()
                        .projection(type_name.as_str(), name.as_str())
                    {
                        super::CheckedSelectResolution::DialogueView {
                            projection,
                            field: selection,
                        }
                    } else {
                        super::CheckedSelectResolution::Field(selection)
                    };
                    (ty, resolution)
                }
                _ => return Err(AnalyzerExpressionError::rejected(owner)),
            }
        };
        Ok(CheckedExpression::new(
            ty,
            CheckedTypeSelection::Inferred,
            target.effects().clone(),
            CheckedExpressionResolution::Select(resolution),
        )
        .into())
    }

    fn check_view_call_expression(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        expression: &HirExpr,
        call: &arcweft_lang_hir::expr::HirCallExpr,
    ) -> Result<Option<CheckedExpression>, AnalyzerExpressionError> {
        let Some(item) =
            enclosing_item(module, expression.scope()).map_err(AnalyzerExpressionError::fatal)?
        else {
            return Ok(None);
        };
        let HirItemKind::View(_) = module
            .resolve_item(item)
            .map_err(|_| AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner))?
            .kind()
        else {
            return Ok(None);
        };

        let classification = match call.callee() {
            super::HirCallCallee::Value { value } => {
                let callee_expression = module.resolve_expr(*value).map_err(|_| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                })?;
                if let HirExprKind::Select(select) = callee_expression.kind() {
                    let receiver = self.evaluate_expression(context, select.target(), None)?;
                    if receiver.ty() == &TypeKind::ViewValue {
                        let HirSelectedMember::Name(_) = select.member() else {
                            return Err(AnalyzerExpressionError::fatal(
                                FinalSemanticAnalysisError::RecoveredOwner,
                            ));
                        };
                        return Ok(None);
                    }
                    return Ok(None);
                }
                let Some(callee) = Self::view_direct_callee(module, *value)? else {
                    return Ok(None);
                };
                self.facts
                    .publish_new_expression(
                        *value,
                        CheckedExpression::new(
                            TypeKind::Named("ViewCallable".to_owned()),
                            super::CheckedTypeSelection::Inferred,
                            EffectSet::new(),
                            CheckedExpressionResolution::ViewCallee(callee.clone()),
                        ),
                    )
                    .map_err(|_| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        )
                    })?;
                match callee {
                    CheckedViewCallee::Element(element) => CheckedViewCall::Element(element),
                    CheckedViewCallee::Text => CheckedViewCall::Text,
                    CheckedViewCallee::RichText => CheckedViewCall::RichText,
                }
            }
            super::HirCallCallee::UnresolvedDot {
                value_receiver,
                member,
                ..
            } => {
                let receiver = self.evaluate_expression(context, *value_receiver, None)?;
                if receiver.ty() != &TypeKind::ViewValue {
                    return Ok(None);
                }
                let HirRecoveredName::Valid(_) = member else {
                    return Err(AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::RecoveredOwner,
                    ));
                };
                return Ok(None);
            }
            super::HirCallCallee::Associated { .. } => return Ok(None),
        };

        let mut effects = EffectSet::new();
        for argument in call.arguments() {
            let checked = self.evaluate_expression(context, argument.value(), None)?;
            effects.union_with(checked.effects());
        }
        Ok(Some(CheckedExpression::new(
            TypeKind::ViewValue,
            super::CheckedTypeSelection::Inferred,
            effects,
            CheckedExpressionResolution::ViewCall(classification),
        )))
    }

    fn check_style_expression_kind(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
    ) -> Result<Option<CheckedExpression>, AnalyzerExpressionError> {
        let Some(expected) = self.style_value_kinds.get(&owner).copied() else {
            return Ok(None);
        };
        if expected != arcweft_view::style::ViewStyleValueKind::Color {
            return Ok(None);
        }
        let HirExprKind::Call(call) = expression.kind() else {
            return Ok(None);
        };
        let super::HirCallCallee::Value { value: callee } = call.callee() else {
            return Ok(None);
        };
        if Self::direct_callee_name(module, *callee)?.as_deref() != Some("rgba") {
            return Ok(None);
        }
        let [red, green, blue, alpha] = call.arguments() else {
            return Err(AnalyzerExpressionError::rejected(owner));
        };
        let mut effects = EffectSet::new();
        let channels =
            [red, green, blue, alpha].map(|argument| -> Result<u8, AnalyzerExpressionError> {
                let HirCallArgument::Positional { .. } = argument else {
                    return Err(AnalyzerExpressionError::rejected(owner));
                };
                let checked =
                    self.evaluate_expression(context, argument.value(), Some(&TypeKind::U8))?;
                effects.union_with(checked.effects());
                style_u8_literal(module, argument.value())
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))
            });
        let [red, green, blue, alpha] = channels;
        let color = arcweft_view::style::ViewColorValue::Literal {
            color: arcweft_presentation::appearance::PresentationColor::rgba(
                red?, green?, blue?, alpha?,
            ),
        };
        self.facts
            .publish_new_expression(
                *callee,
                CheckedExpression::new(
                    TypeKind::Named("StyleColorConstructor".to_owned()),
                    CheckedTypeSelection::Inferred,
                    EffectSet::new(),
                    CheckedExpressionResolution::StyleCallee(CheckedStyleCallee::Rgba),
                ),
            )
            .map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
            })?;
        Ok(Some(CheckedExpression::new(
            TypeKind::Named("Color".to_owned()),
            CheckedTypeSelection::Inferred,
            effects,
            CheckedExpressionResolution::StyleValue(
                arcweft_view::style::ViewSpecifiedValue::Color { value: color },
            ),
        )))
    }

    fn direct_callee_name(
        module: &HirModule,
        owner: ExprId,
    ) -> Result<Option<String>, AnalyzerExpressionError> {
        let expression = module.resolve_expr(owner).map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
        })?;
        let HirExprKind::Path(path) = expression.kind() else {
            return Ok(None);
        };
        let Some(path) = path.as_resolved() else {
            return Ok(None);
        };
        if path.root() != super::HirPathRoot::ImplicitCrate || path.segments().len() != 1 {
            return Ok(None);
        }
        let super::HirPathSegment::Identifier(name) = &path.segments()[0] else {
            return Ok(None);
        };
        Ok(Some(name.as_str().to_owned()))
    }

    fn view_direct_callee(
        module: &HirModule,
        owner: ExprId,
    ) -> Result<Option<CheckedViewCallee>, AnalyzerExpressionError> {
        let expression = module.resolve_expr(owner).map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
        })?;
        let HirExprKind::Path(path) = expression.kind() else {
            return Ok(None);
        };
        let Some(path) = path.as_resolved() else {
            return Ok(None);
        };
        if path.root() != super::HirPathRoot::ImplicitCrate || path.segments().len() != 1 {
            return Ok(None);
        }
        let super::HirPathSegment::Identifier(name) = &path.segments()[0] else {
            return Ok(None);
        };
        Ok(Some(match name.as_str() {
            "Text" => CheckedViewCallee::Text,
            "RichText" => CheckedViewCallee::RichText,
            value => match arcweft_view::ViewElementKind::from_source_name(value) {
                Some(element) => CheckedViewCallee::Element(element),
                None => return Ok(None),
            },
        }))
    }
    fn check_variant_expression_kind(
        &self,
        _context: &AnalyzerExpressionContext<'_>,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
        constructor_head: bool,
    ) -> Result<Option<CheckedExpression>, AnalyzerExpressionError> {
        match expression.kind() {
            HirExprKind::ShortVariant(name) => {
                let name = name.as_resolved().ok_or_else(|| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::RecoveredOwner)
                })?;
                let Some(expected) = expected else {
                    return Err(AnalyzerExpressionError::rejected(owner));
                };
                if let TypeKind::CharacterNominal(
                    nominal @ crate::types::CharacterNominalType::Look { character },
                ) = expected
                {
                    let manifest = self
                        .catalogs
                        .world
                        .environment()
                        .character_manifest(character)
                        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                    let look = CheckedStageLook::try_from_registered_manifest(
                        nominal,
                        manifest,
                        name.clone(),
                    )
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                    return Ok(Some(CheckedExpression::new(
                        expected.clone(),
                        CheckedTypeSelection::Expected,
                        EffectSet::new(),
                        CheckedExpressionResolution::StageLook(look),
                    )));
                }
                let (variant_owner, ordinal) =
                    self.resolve_short_variant(owner, expected, name, constructor_head)?;
                Ok(CheckedExpression::new(
                    expected.clone(),
                    CheckedTypeSelection::Expected,
                    EffectSet::new(),
                    CheckedExpressionResolution::Variant(
                        super::CheckedVariantResolution::try_new(variant_owner, ordinal)
                            .ok_or_else(|| {
                                AnalyzerExpressionError::fatal(
                                    FinalSemanticAnalysisError::WrongPayloadFamily,
                                )
                            })?,
                    ),
                ))
            }
            _ => return Ok(None),
        }
        .map(Some)
    }

    fn resolve_short_variant(
        &self,
        owner: ExprId,
        expected: &TypeKind,
        name: &arcweft_lang_hir::leaf::HirName,
        constructor_head: bool,
    ) -> Result<(CheckedVariantOwner, u32), AnalyzerExpressionError> {
        match expected {
            TypeKind::ProjectNominal(expected_nominal) => {
                let declaration = self
                    .symbols
                    .nominal(expected_nominal.declaration())
                    .cloned()
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                let ProjectNominalBody::Enum { variants } = declaration.body() else {
                    return Err(AnalyzerExpressionError::rejected(owner));
                };
                let (_ordinal, variant) = variants
                    .iter()
                    .enumerate()
                    .find(|(_, variant)| variant.name().as_str() == name.as_str())
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                if variant.payload().is_some() && !constructor_head {
                    return Err(AnalyzerExpressionError::rejected(owner));
                }
                // Project cases require the digest-ordered projection seal;
                // source-order checking cannot publish the final owner row.
                Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::InvalidNominalOwner,
                ))
            }
            TypeKind::CharacterNominal(nominal) => {
                let variants = self
                    .catalogs
                    .world
                    .environment()
                    .character_enum_variants(nominal)
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                let ordinal = variants
                    .iter()
                    .position(|variant| variant == name.as_str())
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                let checked_owner = CheckedVariantOwner::try_character_nominal(
                    nominal.clone(),
                    variants.iter().cloned(),
                )
                .ok_or_else(|| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::AccountingOverflow)
                })?;
                Ok((
                    checked_owner,
                    u32::try_from(ordinal).map_err(|_| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::AccountingOverflow,
                        )
                    })?,
                ))
            }
            TypeKind::Option(item) => match name.as_str() {
                "Some" if constructor_head => {
                    Ok((CheckedVariantOwner::option(item.as_ref().clone()), 0))
                }
                "None" => Ok((CheckedVariantOwner::option(item.as_ref().clone()), 1)),
                _ => Err(AnalyzerExpressionError::rejected(owner)),
            },
            TypeKind::Result { ok, error } if constructor_head => match name.as_str() {
                "Ok" => Ok((
                    CheckedVariantOwner::result(ok.as_ref().clone(), error.as_ref().clone()),
                    0,
                )),
                "Err" => Ok((
                    CheckedVariantOwner::result(ok.as_ref().clone(), error.as_ref().clone()),
                    1,
                )),
                _ => Err(AnalyzerExpressionError::rejected(owner)),
            },
            closed_enum_ty => {
                let schema = self
                    .catalogs
                    .world
                    .environment()
                    .typecheck_env()
                    .closed_enum(closed_enum_ty)
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                let (ordinal, selected) = schema
                    .variants()
                    .iter()
                    .enumerate()
                    .find(|(_, variant)| variant.name() == name.as_str())
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                if !constructor_head && !matches!(selected.payload(), EnumVariantPayload::Unit) {
                    return Err(AnalyzerExpressionError::rejected(owner));
                }
                Ok((
                    checked_builtin_closed_owner(schema, closed_enum_ty, owner)
                        .map_err(AnalyzerExpressionError::fatal)?,
                    u32::try_from(ordinal).map_err(|_| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::AccountingOverflow,
                        )
                    })?,
                ))
            }
        }
    }
    fn check_entity_expression_kind<E>(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expectation: &AnalyzerExpressionExpectation<'_>,
        transaction_authority: &CandidateFactTransactionAuthority<'_>,
    ) -> Result<Option<CheckedExpression>, E>
    where
        E: From<AnalyzerExpressionError> + From<CandidateFactOperationFailure>,
    {
        let expected = expectation.contextual_shape();
        match expression.kind() {
            HirExprKind::EntityReference(reference) => {
                let reference = reference.as_resolved().ok_or_else(|| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::RecoveredOwner)
                })?;
                if matches!(
                    expected,
                    Some(TypeKind::Ref(entity))
                        if entity.kind() == &EntityKind::DialogueLine
                ) {
                    return self
                        .check_dialogue_line_reference(owner, reference, expected)
                        .map(Some)
                        .map_err(E::from);
                }
                let source =
                    expression_span(module, owner).map_err(AnalyzerExpressionError::fatal)?;
                let item = self
                    .resolve_checked_entity_reference(module, reference, source)
                    .map_err(|error| match error {
                        EntityReferenceResolutionError::Lookup => AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::ValueResolutionFailed { owner },
                        ),
                        EntityReferenceResolutionError::WrongFamily => {
                            AnalyzerExpressionError::fatal(
                                FinalSemanticAnalysisError::WrongPayloadFamily,
                            )
                        }
                    })?;
                let ty = item.ty();
                if matches!(
                    expectation,
                    AnalyzerExpressionExpectation::Complete(expected)
                        if !expected.accepts(&ty)
                ) {
                    return Err(AnalyzerExpressionError::rejected(owner).into());
                }
                Ok(CheckedExpression::new(
                    ty,
                    CheckedTypeSelection::Inferred,
                    EffectSet::new(),
                    CheckedExpressionResolution::Value(CheckedValueResolution::ProjectItem(item)),
                ))
            }
            HirExprKind::PostfixBracket(postfix) => self
                .check_postfix_bracket(context, owner, postfix, expected, transaction_authority)
                .map_err(E::from),
            HirExprKind::LifetimePath(_) | HirExprKind::Choice(_) => {
                Err(AnalyzerExpressionError::rejected(owner).into())
            }
            HirExprKind::Error(_) => Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::RecoveredOwner,
            )
            .into()),
            _ => return Ok(None),
        }
        .map(Some)
    }

    fn check_dialogue_line_reference(
        &self,
        owner: ExprId,
        reference: &HirIdRef,
        expected: Option<&TypeKind>,
    ) -> Result<CheckedExpression, AnalyzerExpressionError> {
        let HirIdRef::Absolute(reference) = reference else {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::ValueResolutionFailed { owner },
            ));
        };
        let target =
            arcweft_id::dialogue::DialogueLineId::try_new(reference.as_str()).map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::ValueResolutionFailed {
                    owner,
                })
            })?;
        if self.project.dialogue_lines().get(&target).is_none() {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::ValueResolutionFailed { owner },
            ));
        }
        let ty = TypeKind::entity_ref(EntityKind::DialogueLine);
        if expected.is_some_and(|expected| !expected.accepts(&ty)) {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        Ok(CheckedExpression::new(
            ty,
            CheckedTypeSelection::Expected,
            EffectSet::new(),
            CheckedExpressionResolution::DialogueLineReference(target),
        ))
    }

    fn check_postfix_bracket(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        owner: ExprId,
        postfix: &HirPostfixBracket,
        expected: Option<&TypeKind>,
        transaction_authority: &CandidateFactTransactionAuthority<'_>,
    ) -> Result<CheckedExpression, CandidateFactOperationFailure> {
        let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = postfix.candidates()
        else {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::UnresolvedPostfixBracket { owner },
            )
            .into());
        };
        let index_id = *index;
        let dialogue_id = *dialogue;
        let index_probe = self.probe_postfix_candidate(context, index_id, expected);
        let (checked, projection, resolution) = match index_probe {
            Ok((index_checked, index_projection)) => {
                let dialogue_probe = self.probe_postfix_candidate(context, dialogue_id, expected);
                match dialogue_probe {
                    Ok((_dialogue_checked, dialogue_projection)) => {
                        self.facts
                            .discard_candidate_projection(index_projection)
                            .map_err(AnalyzerExpressionError::fact)?;
                        self.facts
                            .discard_candidate_projection(dialogue_projection)
                            .map_err(AnalyzerExpressionError::fact)?;
                        return Err(AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::AmbiguousPostfixBracket { owner },
                        )
                        .into());
                    }
                    Err(dialogue_error)
                        if matches!(&dialogue_error, AnalyzerExpressionError::Rejected(_)) =>
                    {
                        (
                            index_checked,
                            index_projection,
                            PostfixBracketResolution::Index {
                                candidate: index_id,
                            },
                        )
                    }
                    Err(dialogue_error) => {
                        self.facts
                            .discard_candidate_projection(index_projection)
                            .map_err(AnalyzerExpressionError::fact)?;
                        return Err(dialogue_error.into());
                    }
                }
            }
            Err(index_error) if matches!(&index_error, AnalyzerExpressionError::Rejected(_)) => {
                let dialogue_probe = self.probe_postfix_candidate(context, dialogue_id, expected);
                match dialogue_probe {
                    Ok((checked, projection)) => (
                        checked,
                        projection,
                        PostfixBracketResolution::Dialogue {
                            candidate: dialogue_id,
                        },
                    ),
                    Err(dialogue_error)
                        if matches!(&dialogue_error, AnalyzerExpressionError::Rejected(_)) =>
                    {
                        return Err(AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::UnresolvedPostfixBracket { owner },
                        )
                        .into());
                    }
                    Err(dialogue_error) => return Err(dialogue_error.into()),
                }
            }
            Err(index_error) => return Err(index_error.into()),
        };
        self.facts
            .apply_candidate_projection(transaction_authority, projection)
            .map_err(|failure| CandidateFactOperationFailure::Projection(Box::new(failure)))?;
        Ok(CheckedExpression::new(
            checked.ty().clone(),
            checked.type_selection(),
            checked.effects().clone(),
            CheckedExpressionResolution::PostfixBracket(resolution),
        ))
    }

    fn probe_postfix_candidate(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        candidate: ExprId,
        expected: Option<&TypeKind>,
    ) -> Result<(super::PreparedExpressionFact, CandidateSemanticProjection), AnalyzerExpressionError>
    {
        let outcome = self.run_candidate_fact_transaction(|this, authority, _transaction| {
            let child_context = context.child_candidate(authority);
            let result = this.evaluate_expression(&child_context, candidate, expected);
            drop(child_context);
            result.map(CandidateFactTransactionAction::Extract)
        })?;
        outcome
            .into_extracted()
            .map_err(AnalyzerExpressionError::fact)
    }

    fn check_expressions(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        owners: &[ExprId],
        expected: Option<&TypeKind>,
    ) -> Result<Vec<super::PreparedExpressionFact>, AnalyzerExpressionError> {
        owners
            .iter()
            .map(|owner| self.evaluate_expression(context, *owner, expected))
            .collect()
    }

    pub(super) fn resolve_path_value(
        &self,
        module: &HirModule,
        owner: ExprId,
        scope: ScopeId,
        path: &arcweft_lang_hir::leaf::HirPath,
    ) -> Result<Option<CheckedValueResolution>, FinalSemanticAnalysisError> {
        let source = expression_span(module, owner)?;
        match module
            .lookup_path_local(scope, path, &source)
            .map_err(|_| FinalSemanticAnalysisError::ValueResolutionFailed { owner })?
        {
            LocalLookup::Found(local) => {
                return Ok(Some(CheckedValueResolution::Local(local)));
            }
            LocalLookup::AmbiguousPoisoned(_) => {
                return Err(FinalSemanticAnalysisError::ValueResolutionFailed { owner });
            }
            LocalLookup::NotFound => {}
        }
        match self
            .symbols
            .resolve_hir_value_target(module.key().path(), path, source)
            .map_err(|_| FinalSemanticAnalysisError::ValueResolutionFailed { owner })?
        {
            ProjectValueLookup::Present(callable) => Ok(Some(
                CheckedValueResolution::ProjectCallable(super::CheckedProjectCallable::new(
                    callable.declaration().clone(),
                    callable.source_item(),
                )),
            )),
            ProjectValueLookup::Absent => {
                match self.symbols.resolve_hir_symbol_target(
                    module.key().path(),
                    path,
                    expression_span(module, owner)?,
                ) {
                    Ok(ResolvedProjectSymbol::Retained(symbol)) => {
                        let item = CheckedProjectItem::try_new_retained(
                            symbol.public_id().clone(),
                            symbol.family(),
                            symbol.owner(),
                            self.retained_entity_value_type(symbol.owner())
                                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?,
                        )
                        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                        return Ok(Some(CheckedValueResolution::ProjectItem(item)));
                    }
                    Ok(ResolvedProjectSymbol::External(symbol)) => {
                        let owner = self
                            .catalogs
                            .world
                            .environment()
                            .bound_external_owner(self.symbols, symbol.declaration())
                            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                        return Ok(Some(match owner {
                            RegisteredExternalOwner::Character(character) => {
                                CheckedValueResolution::ProjectItem(
                                    CheckedProjectItem::new_external_character(
                                        symbol.declaration(),
                                        character.clone(),
                                    ),
                                )
                            }
                            RegisteredExternalOwner::Environment(environment) => {
                                CheckedValueResolution::Registered(
                                    RegisteredSemanticValueId::for_environment_binding(
                                        environment.value_binding().clone(),
                                    ),
                                )
                            }
                        }));
                    }
                    Err(ProjectHirSymbolLookupError::Symbol(
                        ProjectSymbolResolutionError::Unknown { .. },
                    )) => {}
                    Ok(
                        ResolvedProjectSymbol::Callable(_)
                        | ResolvedProjectSymbol::StructuralCallable(_)
                        | ResolvedProjectSymbol::Nominal(_)
                        | ResolvedProjectSymbol::Module(_),
                    )
                    | Err(_) => {
                        return Err(FinalSemanticAnalysisError::ValueResolutionFailed { owner });
                    }
                }
                if let Some((receiver_path, member)) = path.split_terminal_segment()
                    && let Some(receiver) =
                        self.resolve_path_value(module, owner, scope, &receiver_path)?
                {
                    let receiver_ty = match &receiver {
                        CheckedValueResolution::Local(local) => {
                            self.facts.locals().get(local).cloned()
                        }
                        _ => value_resolution_type(self.catalogs.world, &receiver),
                    };
                    let member = match &member {
                        HirPathSegment::Identifier(name) => name.as_str(),
                        HirPathSegment::ProjectSymbol(name) => name.as_str(),
                    };
                    if receiver_ty.is_some_and(|ty| {
                        matches!(ty, TypeKind::Ref(entity) if entity.kind() == &EntityKind::Character)
                    }) && let Some(character) = receiver.character()
                        && let Some(field) = crate::types::CharacterField::from_name(member)
                    {
                        return Ok(Some(CheckedValueResolution::CharacterField {
                            receiver: Box::new(receiver),
                            character,
                            field,
                        }));
                    }
                }
                let Some(binding) = environment_binding_for_path(path) else {
                    return Ok(None);
                };
                if self
                    .catalogs
                    .world
                    .environment()
                    .environment_binding(&binding)
                    .is_none()
                {
                    return Ok(None);
                }
                Ok(Some(CheckedValueResolution::Registered(
                    RegisteredSemanticValueId::for_environment_binding(binding),
                )))
            }
        }
    }
}

fn environment_binding_for_path(
    path: &arcweft_lang_hir::leaf::HirPath,
) -> Option<crate::env::identity::EnvironmentBindingId> {
    if path.root() != HirPathRoot::ImplicitCrate {
        return None;
    }
    let mut canonical = String::new();
    for (index, segment) in path.segments().iter().enumerate() {
        if index != 0 {
            canonical.push('.');
        }
        canonical.push_str(match segment {
            HirPathSegment::Identifier(name) => name.as_str(),
            HirPathSegment::ProjectSymbol(name) => name.as_str(),
        });
    }
    crate::env::identity::EnvironmentBindingId::try_new(canonical).ok()
}

fn structural_expression(ty: TypeKind, selection: CheckedTypeSelection) -> CheckedExpression {
    CheckedExpression::new(
        ty,
        selection,
        EffectSet::new(),
        CheckedExpressionResolution::Structural,
    )
}

fn checked_choice_public_id(
    value: &str,
) -> Result<arcweft_id::PublicId, FinalSemanticAnalysisError> {
    let public_id = arcweft_id::PublicId::try_new(value.to_owned())
        .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
    (public_id.as_str().split('.').next() == Some("choice"))
        .then_some(public_id)
        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
}

fn resolve_choice_option_public_id(
    value: &arcweft_lang_hir::leaf::HirIdRefValue,
    choice: Option<&arcweft_id::PublicId>,
) -> Result<arcweft_id::PublicId, FinalSemanticAnalysisError> {
    let reference = value
        .as_resolved()
        .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
    if let HirIdRef::Absolute(value) = reference {
        return checked_choice_public_id(value.as_str());
    }
    let relative = match reference {
        HirIdRef::Relative(relative) => relative,
        HirIdRef::FamilyRelative(relative) if relative.family().as_str() == "choice" => {
            relative.relative()
        }
        HirIdRef::FamilyRelative(_) | HirIdRef::Absolute(_) => {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
    };
    let choice = choice.ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
    let mut base = choice.as_str().split('.').collect::<Vec<_>>();
    if relative.parent_depth() >= base.len() {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    base.truncate(base.len() - relative.parent_depth());
    base.extend(relative.suffix().as_str().split('.'));
    checked_choice_public_id(&base.join("."))
}

fn break_targets_loop(
    module: &HirModule,
    mut scope: ScopeId,
    target: ExprId,
) -> Result<bool, FinalSemanticAnalysisError> {
    loop {
        let current = module
            .resolve_scope(scope)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        match current.owner() {
            HirScopeOwner::Expr(owner)
                if matches!(
                    module
                        .resolve_expr(*owner)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                        .kind(),
                    HirExprKind::Loop(_)
                ) =>
            {
                return Ok(*owner == target);
            }
            HirScopeOwner::Stmt(owner)
                if matches!(
                    module
                        .resolve_stmt(*owner)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                        .kind(),
                    HirStmtKind::While(_) | HirStmtKind::WhileLet(_) | HirStmtKind::For(_)
                ) =>
            {
                return Ok(false);
            }
            _ => {}
        }
        let Some(parent) = current.parent() else {
            return Ok(false);
        };
        scope = parent;
    }
}

fn style_u8_literal(module: &HirModule, owner: ExprId) -> Option<u8> {
    let expression = module.resolve_expr(owner).ok()?;
    let HirExprKind::Literal(HirLiteral::Integer(HirIntegerLiteral::Value {
        magnitude,
        suffix: None,
        ..
    })) = expression.kind()
    else {
        return None;
    };
    match magnitude.limbs_le() {
        [] => Some(0),
        [value] => u8::try_from(*value).ok(),
        _ => None,
    }
}

fn source_span_for_role(
    module: &HirModule,
    query: HirSourceQuery,
) -> Result<SourceSpan, FinalSemanticAnalysisError> {
    let lookup = module
        .source_site(module.provenance().source_identity(), query)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
    match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(span)) => Ok(span.clone()),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => Err(FinalSemanticAnalysisError::RecoveredOwner),
    }
}

#[cfg(test)]
mod expectation_tests {
    use super::*;

    fn generic(owner: u64) -> GenericTypeParameterId {
        GenericTypeParameterId::new(
            GenericParameterOwnerId::Detached(crate::types::DetachedGenericOwnerId::new(owner)),
            0,
        )
    }

    #[test]
    fn parametric_expectation_seals_sorted_owned_unbound_inventory() {
        let owned = generic(1);
        let foreign = generic(2);
        let expected = TypeKind::Option(Box::new(TypeKind::GenericParam(owned.clone())));
        assert!(
            AnalyzerExpressionExpectation::parametric(&expected, &[owned.clone().into()]).is_some()
        );
        assert!(
            AnalyzerExpressionExpectation::parametric(&expected, &[foreign.into()]).is_none(),
            "an unbound identity absent from the shape cannot be admitted"
        );
        assert!(
            AnalyzerExpressionExpectation::parametric(
                &expected,
                &[owned.clone().into(), owned.into()],
            )
            .is_none(),
            "duplicate/unordered unbound rows are not canonical"
        );
    }

    #[test]
    fn parametric_projection_intersects_children_without_dropping_failures() {
        let parameter = generic(3);
        let expected = TypeKind::Tuple(vec![
            TypeKind::GenericParam(parameter.clone()),
            TypeKind::I64,
        ]);
        let expectation =
            AnalyzerExpressionExpectation::parametric(&expected, &[parameter.clone().into()])
                .expect("canonical parametric expectation");
        assert!(matches!(
            expectation
                .project(Some(&TypeKind::GenericParam(parameter)))
                .expect("valid generic child"),
            AnalyzerExpressionExpectation::Parametric { .. }
        ));
        assert!(matches!(
            expectation
                .project(Some(&TypeKind::I64))
                .expect("valid concrete child"),
            AnalyzerExpressionExpectation::Complete(TypeKind::I64)
        ));
    }

    #[test]
    fn expected_dependent_cache_requires_the_same_parametric_semantic_shape() {
        let parameter = generic(4);
        let first = TypeKind::Option(Box::new(TypeKind::GenericParam(parameter.clone())));
        let second = TypeKind::Probe(Box::new(TypeKind::GenericParam(parameter.clone())));
        let checked: super::PreparedExpressionFact = CheckedExpression::new(
            first.clone(),
            CheckedTypeSelection::Expected,
            EffectSet::new(),
            CheckedExpressionResolution::Structural,
        )
        .into();
        let first_expectation =
            AnalyzerExpressionExpectation::parametric(&first, &[parameter.clone().into()])
                .expect("first expectation");
        let second_expectation =
            AnalyzerExpressionExpectation::parametric(&second, &[parameter.into()])
                .expect("second expectation");
        assert!(first_expectation.accepts_cached(&checked));
        assert!(!second_expectation.accepts_cached(&checked));

        let inferred: super::PreparedExpressionFact = CheckedExpression::new(
            TypeKind::Bool,
            CheckedTypeSelection::Inferred,
            EffectSet::new(),
            CheckedExpressionResolution::Structural,
        )
        .into();
        assert!(second_expectation.accepts_cached(&inferred));
    }
}
