//! Ordinary-call probing, accounting, and final semantic selection.

#[path = "calls/semantics.rs"]
mod semantics;

use semantics::{
    call_result_type, physical_evaluation_kind, physical_expected_type, provisional_call_effects,
    provisional_callable_effects, select_candidate_probes,
};
pub(super) use semantics::{
    callable_schema_type, callable_schema_type_with_effects, checked_project_nominal,
    final_call_effects, final_callable_effects, nominal_substitutions,
};

use super::expression_types::value_resolution_type;
use super::preparation::AssociatedReceiverTypeResolution;
use super::statements::{expression_span, scope_is_within, source_span};
use super::{
    Analyzer, BTreeMap, CallArgumentMapping, CallCalleeClassificationFact, CallPoison,
    CallResolverAuthority, CallResolverRequest, CallTargetFacts, CallTargetFactsInput,
    CallableDeclarationKey, CallableDeclarationOwner, CallableGroupIndex, CallableInstantiation,
    CandidateEvaluationPass, CandidateProbe, CandidateScore, CandidateSelection,
    CandidateSemanticProjection, CharacterDialogueCharacterType, CharacterDialogueFieldCoordinate,
    CharacterDialoguePatchContext, CharacterOwnerSource, CheckedCallArgumentFact,
    CheckedCallArgumentSlotFact, CheckedCallArgumentSlotInput, CheckedCallArgumentSlotSource,
    CheckedCallTarget, CheckedCallableDeclaration, CheckedCharacterDialogueFactory,
    CheckedCharacterDialoguePatch, CheckedCharacterDialoguePatchField,
    CheckedCharacterDialogueReconfigure, CheckedCharacterDialogueTarget, CheckedExpression,
    CheckedExpressionResolution, CheckedPatchOperation, CheckedTypeSelection,
    CheckedValueResolution, EffectRow, EffectSet, EvaluatedCallArguments, ExprId,
    FinalCallCalleeFacts, FinalSemanticAnalysisError, HirAssociatedSeparator, HirCallArgument,
    HirCallArgumentSourcePart, HirCallCallee, HirCallExpr, HirExprKind, HirExprSourceRole,
    HirModule, HirPathSegment, HirSelectedMember, HirSourcePresence, HirSourceQuery, HirSourceSite,
    PendingCallAnalysis, PhysicalCandidateArgument, PhysicalCandidateArgumentEvaluation,
    RegisteredSemanticValueId, ResolveCallOutcome, ResolvedCallTarget, ResolvedCallable,
    ResolvedCharacterOwner, ResolverWork, ScopeId, TypeKind, TypeParameterSubstitutions,
    map_call_arguments, map_unmapped_call_arguments, prepare_final_call_callee,
    prepare_language_free_dot_path, resolve_call_target,
};
use crate::callable::{MappedCallArgument, MappedCallArgumentSlot};
use crate::final_analysis::type_rules::compact_numeric_element_type as infer_compact_numeric_element_type;

#[derive(Clone, Copy)]
struct CallSource<'a> {
    module: &'a HirModule,
    owner: ExprId,
    call: &'a HirCallExpr,
    expected: Option<&'a TypeKind>,
}

struct ResolvedCallQuery {
    callee: CallCalleeClassificationFact,
    considered: Vec<ResolvedCallable>,
    function_value_type: Option<TypeKind>,
    current_group: CallableGroupIndex,
    work: ResolverWork,
    argument_count: u64,
    dialogue_context: CharacterDialoguePatchContext,
}

struct AssociatedReceiverRecovery {
    receiver: arcweft_lang_hir::identity::TypeId,
    separator: HirAssociatedSeparator,
    result: TypeKind,
}

struct CandidateProbeBatch {
    probes: Vec<CandidateProbe>,
    singleton_checkpoint: Option<usize>,
}

#[derive(Clone, Copy)]
struct CandidateProbeRequest<'a> {
    module: &'a HirModule,
    owner: ExprId,
    arguments: &'a [HirCallArgument],
    candidate: &'a ResolvedCallable,
    current_group: CallableGroupIndex,
    expected_result: Option<&'a TypeKind>,
    pass: CandidateEvaluationPass,
}

#[derive(Clone, Copy)]
struct ArgumentEvaluationRequest<'a> {
    call: ExprId,
    authored: &'a [HirCallArgument],
    candidate: &'a ResolvedCallable,
    mapping: &'a CallArgumentMapping,
    pass: CandidateEvaluationPass,
    shape_rejected: bool,
}

#[derive(Clone, Copy)]
struct MappedArgumentEvaluationRequest<'a> {
    call: ExprId,
    authored: &'a HirCallArgument,
    mapped: &'a MappedCallArgument,
    candidate: &'a ResolvedCallable,
    pass: CandidateEvaluationPass,
    shape_rejected: bool,
    argument: arcweft_lang_hir::expr::HirCallArgumentOrdinal,
}

struct MappedArgumentEvaluation {
    fact: CheckedCallArgumentFact,
    hard_errors: usize,
    exact_matches: usize,
}

struct MappedSlotEvaluation {
    fact: CheckedCallArgumentSlotFact,
    hard_error: bool,
    exact_match: bool,
    poison: CallPoison,
}

#[derive(Clone, Copy)]
struct CharacterDialoguePatchFieldRequest<'a> {
    source: CallSource<'a>,
    target: &'a CheckedCharacterDialogueTarget,
    context: CharacterDialoguePatchContext,
    index: usize,
    argument: &'a HirCallArgument,
}

struct RecoveryCall<'a> {
    source: CallSource<'a>,
    callee: CallCalleeClassificationFact,
    candidates: &'a [ResolvedCallable],
    considered: &'a [ResolvedCallable],
    current_group: CallableGroupIndex,
    arguments: Vec<CheckedCallArgumentFact>,
    result: TypeKind,
    work: ResolverWork,
    ambiguous: bool,
}

pub(super) fn checked_character_dialogue_target(
    expression: ExprId,
    checked: &CheckedExpression,
) -> Option<CheckedCharacterDialogueTarget> {
    if let CheckedExpressionResolution::Value(CheckedValueResolution::ProjectItem(item)) =
        checked.resolution()
        && item.family() == arcweft_id::DeclarationIdentityFamily::Character
    {
        let character = item.character().map_or(
            CharacterDialogueCharacterType::Any,
            CharacterDialogueCharacterType::Exact,
        );
        return Some(CheckedCharacterDialogueTarget::Character {
            expression,
            item: Some(Box::new(item.clone())),
            character,
        });
    }
    match checked.ty() {
        TypeKind::Ref(entity) if entity.kind() == &crate::types::EntityKind::Character => {
            Some(CheckedCharacterDialogueTarget::Character {
                expression,
                item: match checked.resolution() {
                    CheckedExpressionResolution::Value(CheckedValueResolution::ProjectItem(
                        item,
                    )) => Some(Box::new(item.clone())),
                    _ => None,
                },
                character: CharacterDialogueCharacterType::Any,
            })
        }
        TypeKind::CharacterDialogue(ty) => Some(CheckedCharacterDialogueTarget::Dialogue {
            expression,
            ty: ty.clone(),
        }),
        _ => None,
    }
}

fn character_dialogue_field_type(
    target: &CheckedCharacterDialogueTarget,
    coordinate: &CharacterDialogueFieldCoordinate,
) -> Option<TypeKind> {
    Some(match coordinate {
        CharacterDialogueFieldCoordinate::Voice => TypeKind::Named("DialogueVoice".to_owned()),
        CharacterDialogueFieldCoordinate::Look => match target.character().exact() {
            Some(character) => TypeKind::character_look(character.clone()),
            None => return None,
        },
        CharacterDialogueFieldCoordinate::Stage => TypeKind::Named("DialogueStage".to_owned()),
        CharacterDialogueFieldCoordinate::Portrait => {
            TypeKind::Named("DialoguePortrait".to_owned())
        }
        CharacterDialogueFieldCoordinate::Focus => TypeKind::Named("DialogueFocus".to_owned()),
        CharacterDialogueFieldCoordinate::Cleanup => TypeKind::Named("DialogueCleanup".to_owned()),
        CharacterDialogueFieldCoordinate::View => {
            TypeKind::entity_ref(crate::types::EntityKind::View)
        }
        CharacterDialogueFieldCoordinate::SourceLocale => TypeKind::String,
        CharacterDialogueFieldCoordinate::Hooks => {
            TypeKind::Seq(Box::new(TypeKind::Named("DialogueHook".to_owned())))
        }
        CharacterDialogueFieldCoordinate::Style => TypeKind::Choice(vec![
            TypeKind::entity_ref(crate::types::EntityKind::Style),
            TypeKind::Named("RichTextStyle".to_owned()),
        ]),
        CharacterDialogueFieldCoordinate::RichText => TypeKind::Named("RichTextStyle".to_owned()),
        CharacterDialogueFieldCoordinate::InlineFailure => {
            TypeKind::Named("InlineFailurePolicy".to_owned())
        }
        CharacterDialogueFieldCoordinate::Custom(_) => return None,
    })
}

fn character_dialogue_field_accepts(
    coordinate: &CharacterDialogueFieldCoordinate,
    expected: Option<&TypeKind>,
    actual: &TypeKind,
) -> bool {
    if let Some(expected) = expected {
        return expected.accepts(actual);
    }
    matches!(
        (coordinate, actual),
        (
            CharacterDialogueFieldCoordinate::Look,
            TypeKind::CharacterNominal(crate::types::CharacterNominalType::Look { .. })
        )
    )
}

fn is_none_patch_value(
    module: &HirModule,
    owner: ExprId,
) -> Result<bool, FinalSemanticAnalysisError> {
    let expression = module
        .resolve_expr(owner)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
    Ok(match expression.kind() {
        HirExprKind::ShortVariant(name) => name
            .as_resolved()
            .is_some_and(|name| name.as_str() == "None"),
        HirExprKind::Path(path) => path.as_resolved().is_some_and(|path| {
            matches!(
                path.segments().last(),
                Some(HirPathSegment::Identifier(name))
                    if name.as_str() == "None"
            )
        }),
        _ => false,
    })
}

fn call_argument_span(
    module: &HirModule,
    owner: ExprId,
    index: usize,
) -> Result<arcweft_source::SourceSpan, FinalSemanticAnalysisError> {
    call_argument_part_span(module, owner, index, HirCallArgumentSourcePart::Whole)
}

fn call_argument_part_span(
    module: &HirModule,
    owner: ExprId,
    index: usize,
    part: HirCallArgumentSourcePart,
) -> Result<arcweft_source::SourceSpan, FinalSemanticAnalysisError> {
    let argument = arcweft_lang_hir::expr::HirCallArgumentOrdinal::try_from_usize(index)
        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
    source_span(
        module,
        HirSourceQuery::Expr {
            owner,
            role: HirExprSourceRole::CallArgument { argument, part },
        },
    )
}

impl Analyzer<'_, '_, '_> {
    /// Returns the exact ordinary Function declaration that lexically owns an
    /// expression. The checked-callable staging transaction already retains
    /// each accepted body scope and checked identity, so call facts do not
    /// reconstruct ownership from source text or maintain a parallel index.
    pub(super) fn enclosing_ordinary_callable(
        &self,
        module: &HirModule,
        expression: ExprId,
    ) -> Result<Option<CallableDeclarationKey>, FinalSemanticAnalysisError> {
        let scope = module
            .resolve_expr(expression)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .scope();
        let staged = self
            .staged_callables
            .as_ref()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let mut enclosing = None;
        for body in &staged.bodies {
            if body.module != module.module_id()
                || body.owner != CallableDeclarationOwner::Function
                || !scope_is_within(module, scope, body.scope)?
            {
                continue;
            }
            let CheckedCallableDeclaration::Project(declaration) = body.id.declaration() else {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            };
            if declaration.owner() != CallableDeclarationOwner::Function
                || enclosing.replace(declaration.clone()).is_some()
            {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            }
        }
        Ok(enclosing)
    }

    pub(super) fn check_call_expression(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        call: &HirCallExpr,
        expected: Option<&TypeKind>,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let is_root = !self.callable_query_depth.is_active();
        self.callable_query_depth
            .try_enter()
            .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed { owner })?;
        if is_root {
            self.physical_candidate_argument_evaluations
                .entry(owner)
                .or_default();
        }
        self.physical_call_stack.push(owner);
        let result = self.check_call_expression_inner(module, owner, call, expected, None);
        let popped = self
            .physical_call_stack
            .pop()
            .expect("Call stack push and pop are paired");
        assert_eq!(popped, owner, "nested Call stack exits LIFO");
        self.callable_query_depth.leave();
        result
    }

    pub(super) fn check_immediate_character_dialogue_call(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        call: &HirCallExpr,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let is_root = !self.callable_query_depth.is_active();
        self.callable_query_depth
            .try_enter()
            .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed { owner })?;
        if is_root {
            self.physical_candidate_argument_evaluations
                .entry(owner)
                .or_default();
        }
        self.physical_call_stack.push(owner);
        let result = self.check_call_expression_inner(
            module,
            owner,
            call,
            None,
            Some(CharacterDialoguePatchContext::ImmediateContentApplication),
        );
        let popped = self
            .physical_call_stack
            .pop()
            .expect("Call stack push and pop are paired");
        assert_eq!(popped, owner, "nested Call stack exits LIFO");
        self.callable_query_depth.leave();
        result
    }

    fn check_call_expression_inner(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        call: &HirCallExpr,
        expected: Option<&TypeKind>,
        dialogue_context: Option<CharacterDialoguePatchContext>,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let source = CallSource {
            module,
            owner,
            call,
            expected,
        };
        let argument_count = u64::try_from(source.call.arguments().len())
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        let mut work = ResolverWork::new(self.catalogs.callable_limits.max_query_work());
        work.record_logical_argument_checks(argument_count)
            .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
                owner: source.owner,
            })?;
        if let Some(recovery) = self.stage_call_callee_children(source.module, source.call)? {
            return self.publish_associated_receiver_recovery(source, recovery, work);
        }
        let dialogue_context =
            dialogue_context.unwrap_or(CharacterDialoguePatchContext::ReusableValue);
        let mut resolution =
            self.resolve_call_query(source, work, argument_count, dialogue_context)?;
        let probes = self.probe_resolved_call(source, &mut resolution)?;
        match select_candidate_probes(&probes.probes) {
            CandidateSelection::Selected(selected) => {
                self.publish_selected_call(source, resolution, probes, selected)
            }
            CandidateSelection::Ambiguous { primary, tied } => {
                self.publish_ambiguous_call(source, resolution, probes, primary, tied)
            }
            CandidateSelection::Rejected { primary } => {
                self.publish_rejected_call(source, resolution, probes, primary)
            }
        }
    }

    fn checked_character_dialogue_resolution(
        &mut self,
        source: CallSource<'_>,
        selected: &ResolvedCallable,
        context: CharacterDialoguePatchContext,
    ) -> Result<Option<CheckedExpressionResolution>, FinalSemanticAnalysisError> {
        let crate::callable::CallableValidator::Dialogue(id) = selected.schema().validator() else {
            return Ok(None);
        };
        if !matches!(
            id,
            crate::callable::DialogueCallableId::CharacterFactory
                | crate::callable::DialogueCallableId::CharacterReconfigure
        ) {
            return Ok(None);
        }
        let HirCallCallee::Value { value } = source.call.callee() else {
            return Ok(None);
        };
        let Some(callee) = self.facts.expressions().get(value).cloned() else {
            return Ok(None);
        };
        let Some(target) = checked_character_dialogue_target(*value, &callee) else {
            return Ok(None);
        };
        let patch = self.checked_character_dialogue_patch(source, &target, context)?;
        let result = target.result_type();
        let resolution = match (&target, id) {
            (
                CheckedCharacterDialogueTarget::Character { .. },
                crate::callable::DialogueCallableId::CharacterFactory,
            ) => CheckedExpressionResolution::CharacterDialogueFactory(
                CheckedCharacterDialogueFactory::new(target, patch),
            ),
            (
                CheckedCharacterDialogueTarget::Dialogue { .. },
                crate::callable::DialogueCallableId::CharacterReconfigure,
            ) => CheckedExpressionResolution::CharacterDialogueReconfigure(
                CheckedCharacterDialogueReconfigure::new(target, patch),
            ),
            _ => {
                return Err(FinalSemanticAnalysisError::InvalidCharacterDialoguePatch {
                    owner: source.owner,
                });
            }
        };
        if selected.schema().result() != &TypeKind::CharacterDialogue(result) {
            return Err(FinalSemanticAnalysisError::InvalidCharacterDialoguePatch {
                owner: source.owner,
            });
        }
        Ok(Some(resolution))
    }

    fn checked_character_dialogue_patch(
        &self,
        source: CallSource<'_>,
        target: &CheckedCharacterDialogueTarget,
        context: CharacterDialoguePatchContext,
    ) -> Result<CheckedCharacterDialoguePatch, FinalSemanticAnalysisError> {
        let mut fields = Vec::with_capacity(source.call.arguments().len());
        let mut coordinates = BTreeMap::new();
        for (index, argument) in source.call.arguments().iter().enumerate() {
            let request = CharacterDialoguePatchFieldRequest {
                source,
                target,
                context,
                index,
                argument,
            };
            let Some((coordinate, field_span)) =
                self.character_dialogue_field_coordinate(request)?
            else {
                continue;
            };
            if let Some(first_span) = coordinates.insert(coordinate.clone(), field_span.clone()) {
                return Err(
                    FinalSemanticAnalysisError::DuplicateCharacterDialogueField {
                        coordinate,
                        first_span,
                        duplicate_span: field_span,
                    },
                );
            }
            fields.push(self.checked_character_dialogue_patch_field(request, coordinate)?);
        }
        Ok(CheckedCharacterDialoguePatch::new(
            context,
            fields,
            expression_span(source.module, source.owner)?,
        ))
    }

    fn character_dialogue_field_coordinate(
        &self,
        request: CharacterDialoguePatchFieldRequest<'_>,
    ) -> Result<
        Option<(CharacterDialogueFieldCoordinate, arcweft_source::SourceSpan)>,
        FinalSemanticAnalysisError,
    > {
        let CharacterDialoguePatchFieldRequest {
            source,
            context,
            index,
            argument,
            ..
        } = request;
        let field_span = match argument {
            HirCallArgument::Named { .. } => call_argument_part_span(
                source.module,
                source.owner,
                index,
                HirCallArgumentSourcePart::Name,
            )?,
            HirCallArgument::Positional { .. } | HirCallArgument::Spread { .. } => {
                call_argument_span(source.module, source.owner, index)?
            }
        };
        let coordinate = match argument {
            HirCallArgument::Positional { .. } if index == 0 => {
                CharacterDialogueFieldCoordinate::Look
            }
            HirCallArgument::Positional { .. } | HirCallArgument::Spread { .. } => {
                return Err(FinalSemanticAnalysisError::InvalidCharacterDialoguePatch {
                    owner: source.owner,
                });
            }
            HirCallArgument::Named { .. } => {
                let name = argument.resolved_name().ok_or(
                    FinalSemanticAnalysisError::InvalidCharacterDialoguePatch {
                        owner: source.owner,
                    },
                )?;
                match name.as_str() {
                    "id" | "text_key"
                        if context
                            == CharacterDialoguePatchContext::ImmediateContentApplication =>
                    {
                        return Ok(None);
                    }
                    "id" | "text_key" => {
                        return Err(
                            FinalSemanticAnalysisError::CharacterDialogueApplicationOnlyField {
                                field: name.as_str().to_owned(),
                                field_span,
                            },
                        );
                    }
                    "character" | "character_id" | "content" => {
                        return Err(FinalSemanticAnalysisError::InvalidCharacterDialoguePatch {
                            owner: source.owner,
                        });
                    }
                    "voice" => CharacterDialogueFieldCoordinate::Voice,
                    "look" => CharacterDialogueFieldCoordinate::Look,
                    "stage" => CharacterDialogueFieldCoordinate::Stage,
                    "portrait" => CharacterDialogueFieldCoordinate::Portrait,
                    "focus" => CharacterDialogueFieldCoordinate::Focus,
                    "cleanup" => CharacterDialogueFieldCoordinate::Cleanup,
                    "view" => CharacterDialogueFieldCoordinate::View,
                    "source_locale" => CharacterDialogueFieldCoordinate::SourceLocale,
                    "hooks" => CharacterDialogueFieldCoordinate::Hooks,
                    "style" => CharacterDialogueFieldCoordinate::Style,
                    "rich_text" => CharacterDialogueFieldCoordinate::RichText,
                    "inline_error" | "inline_error_policy" | "inline_fallback" => {
                        CharacterDialogueFieldCoordinate::InlineFailure
                    }
                    name => {
                        let descriptor = self
                            .catalogs
                            .world
                            .environment()
                            .character_dialogue_fields()
                            .resolve(source.module.key().path(), name)
                            .ok_or_else(|| {
                                FinalSemanticAnalysisError::UnknownCharacterDialogueField {
                                    name: name.to_owned(),
                                    field_span: field_span.clone(),
                                    scope: source.module.key().path().clone(),
                                }
                            })?;
                        CharacterDialogueFieldCoordinate::Custom(descriptor.id().clone())
                    }
                }
            }
        };
        Ok(Some((coordinate, field_span)))
    }

    fn checked_character_dialogue_patch_field(
        &self,
        request: CharacterDialoguePatchFieldRequest<'_>,
        coordinate: CharacterDialogueFieldCoordinate,
    ) -> Result<CheckedCharacterDialoguePatchField, FinalSemanticAnalysisError> {
        let CharacterDialoguePatchFieldRequest {
            source,
            target,
            index,
            argument,
            ..
        } = request;
        let custom_descriptor = match &coordinate {
            CharacterDialogueFieldCoordinate::Custom(id) => self
                .catalogs
                .world
                .environment()
                .character_dialogue_fields()
                .descriptor(id),
            _ => None,
        };
        let expected = custom_descriptor
            .map(|descriptor| descriptor.value_type().clone())
            .or_else(|| character_dialogue_field_type(target, &coordinate));
        let checked = self.facts.expressions().get(&argument.value()).ok_or(
            FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                owner: argument.value(),
            },
        )?;
        let clearing = is_none_patch_value(source.module, argument.value())?;
        if clearing
            && let Some(descriptor) = custom_descriptor.filter(|descriptor| !descriptor.clearable())
        {
            return Err(
                FinalSemanticAnalysisError::CharacterDialogueFieldNotClearable {
                    field: descriptor.id().clone(),
                    field_span: call_argument_part_span(
                        source.module,
                        source.owner,
                        index,
                        HirCallArgumentSourcePart::Name,
                    )?,
                    declaration_span: descriptor.declaration().clone(),
                },
            );
        }
        let operation = match checked.resolution() {
            CheckedExpressionResolution::Variant(variant)
                if matches!(variant.owner(), super::CheckedVariantOwner::Option { .. })
                    && variant.ordinal() == 1 =>
            {
                CheckedPatchOperation::Clear
            }
            _ if clearing => CheckedPatchOperation::Clear,
            _ if character_dialogue_field_accepts(&coordinate, expected.as_ref(), checked.ty()) => {
                CheckedPatchOperation::Set {
                    value: argument.value(),
                    ty: checked.ty().clone(),
                }
            }
            _ => {
                if let Some(descriptor) = custom_descriptor {
                    return Err(
                        FinalSemanticAnalysisError::CharacterDialogueCustomFieldTypeMismatch {
                            field: descriptor.id().clone(),
                            declared: Box::new(descriptor.value_type().clone()),
                            actual: Box::new(checked.ty().clone()),
                            value_span: expression_span(source.module, argument.value())?,
                            declaration_span: descriptor.declaration().clone(),
                        },
                    );
                }
                return Err(FinalSemanticAnalysisError::CharacterDialogueFieldType {
                    owner: argument.value(),
                });
            }
        };
        Ok(CheckedCharacterDialoguePatchField::new(
            coordinate,
            operation,
            call_argument_span(source.module, source.owner, index)?,
        ))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "call-query resolution keeps preparation, charged resolver execution, and checked fact publication atomic"
    )]
    fn resolve_call_query(
        &mut self,
        source: CallSource<'_>,
        mut work: ResolverWork,
        argument_count: u64,
        dialogue_context: CharacterDialoguePatchContext,
    ) -> Result<ResolvedCallQuery, FinalSemanticAnalysisError> {
        let authority = CallResolverAuthority::accepted(
            self.project,
            source.module,
            self.symbols,
            self.catalogs.world,
        );
        let enum_variants = BTreeMap::new();
        let prepared = prepare_final_call_callee(
            authority,
            source.owner,
            FinalCallCalleeFacts::new(
                self.facts.expressions(),
                self.facts.calls(),
                &self.type_reports,
                &enum_variants,
            ),
            dialogue_context,
            &self.catalogs.callable_limits,
        )
        .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
            owner: source.owner,
        })?;

        let staged = self
            .staged_callables
            .as_ref()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let request = CallResolverRequest::try_new(
            prepared.as_borrowed(),
            &super::CallResolverContext {
                authority,
                checked: (&staged.builder).into(),
                expected: source.expected,
                expression: source.owner,
                cancellation: self.control.cancellation(),
                limits: &self.catalogs.callable_limits,
            },
            &mut work,
        )
        .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
            owner: source.owner,
        })?;
        let callee = request.classification();
        let outcome = resolve_call_target(request);
        let (mut considered, function_value_type, current_group) = match outcome {
            ResolveCallOutcome::Resolved(ResolvedCallTarget::Candidates(candidates)) => (
                candidates.as_slice().to_vec(),
                None,
                candidates.first().call_group(),
            ),
            ResolveCallOutcome::Resolved(ResolvedCallTarget::FunctionValue(value)) => (
                vec![value.callable().clone()],
                Some(value.function_type().clone()),
                value.current_group(),
            ),
            ResolveCallOutcome::Missing(target) => {
                let name = target
                    .path()
                    .map(crate::callable::CallablePath::dotted_name)
                    .or_else(|| target.method().map(|method| method.as_str().to_owned()))
                    .unwrap_or_else(|| "<recovered>".to_owned());
                let lookup = source
                    .module
                    .source_site(
                        source.module.provenance().source_identity(),
                        HirSourceQuery::Expr {
                            owner: source.owner,
                            role: HirExprSourceRole::CallCallee,
                        },
                    )
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                let HirSourcePresence::Present(HirSourceSite::Span(span)) = lookup.presence()
                else {
                    return Err(FinalSemanticAnalysisError::RecoveredOwner);
                };
                return Err(FinalSemanticAnalysisError::UnknownCallTarget {
                    owner: source.owner,
                    kind: target.kind(),
                    name,
                    call_source: span.clone(),
                });
            }
            ResolveCallOutcome::Resolved(ResolvedCallTarget::NonCallable(_))
            | ResolveCallOutcome::Rejected(_) => {
                return Err(FinalSemanticAnalysisError::CallResolutionFailed {
                    owner: source.owner,
                });
            }
        };
        self.specialize_presentation_character_candidates(
            source.module,
            source.owner,
            source.call.arguments(),
            &mut considered,
        )?;
        Ok(ResolvedCallQuery {
            callee,
            considered,
            function_value_type,
            current_group,
            work,
            argument_count,
            dialogue_context,
        })
    }

    fn publish_associated_receiver_recovery(
        &mut self,
        source: CallSource<'_>,
        recovery: AssociatedReceiverRecovery,
        mut work: ResolverWork,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let arguments = self.check_candidate_neutral_arguments(source, &mut work)?;
        work.record_retained_argument_fact_publications(
            u64::try_from(arguments.len())
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
        )
        .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
            owner: source.owner,
        })?;
        let callee = CallCalleeClassificationFact::AssociatedType {
            receiver: recovery.receiver,
            separator: recovery.separator,
        };
        self.stage_associated_receiver_recovery_expression(
            source.module,
            source.call,
            &recovery.result,
        )?;
        let checked =
            CheckedCallTarget::associated_receiver_recovery(arguments, recovery.result.clone());
        let enclosing_callable = self.enclosing_ordinary_callable(source.module, source.owner)?;
        let facts = CallTargetFacts::try_new(
            CallTargetFactsInput {
                expression: source.owner,
                enclosing_callable,
                callee: Some(callee),
                checked,
                diagnostics: Vec::new(),
                accounting: work.call_accounting(),
            },
            &self.catalogs.callable_limits,
        )
        .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
            owner: source.owner,
        })?;
        if self.facts.set_call_fact(source.owner, facts) {
            return Err(FinalSemanticAnalysisError::CallResolutionFailed {
                owner: source.owner,
            });
        }
        let selection = if source
            .expected
            .is_some_and(|expected| expected.accepts(&recovery.result))
        {
            CheckedTypeSelection::Expected
        } else {
            CheckedTypeSelection::Inferred
        };
        Ok(CheckedExpression::new(
            recovery.result,
            selection,
            EffectSet::new(),
            CheckedExpressionResolution::Call,
        ))
    }

    fn check_candidate_neutral_arguments(
        &mut self,
        source: CallSource<'_>,
        work: &mut ResolverWork,
    ) -> Result<Vec<CheckedCallArgumentFact>, FinalSemanticAnalysisError> {
        let mapping = map_unmapped_call_arguments(source.call.arguments()).ok_or(
            FinalSemanticAnalysisError::CallResolutionFailed {
                owner: source.owner,
            },
        )?;
        let mut arguments = Vec::with_capacity(source.call.arguments().len());
        for (index, mapped) in mapping.arguments().iter().enumerate() {
            let argument = arcweft_lang_hir::expr::HirCallArgumentOrdinal::try_from_usize(index)
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
            let mut slots = Vec::with_capacity(mapped.slots().len());
            for slot in mapped.slots() {
                self.control.check()?;
                work.charge_type_check(1).map_err(|_| {
                    FinalSemanticAnalysisError::CallResolutionFailed {
                        owner: source.owner,
                    }
                })?;
                let inferred = self.check_call_argument_slot(slot.source(), None)?;
                slots.push(CheckedCallArgumentSlotFact::new(
                    CheckedCallArgumentSlotInput {
                        slot: slot.slot(),
                        source: slot.source(),
                        mapped: None,
                        inferred: Some(inferred),
                        expected: None,
                        poison: CallPoison::Clean,
                    },
                ));
            }
            arguments.push(CheckedCallArgumentFact::new(
                argument,
                slots,
                CallPoison::Clean,
            ));
        }
        Ok(arguments)
    }

    fn stage_associated_receiver_recovery_expression(
        &mut self,
        module: &HirModule,
        call: &HirCallExpr,
        receiver_type: &TypeKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let HirCallCallee::UnresolvedDot { value_receiver, .. } = call.callee() else {
            return Ok(());
        };
        module
            .resolve_expr(*value_receiver)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if !self.facts.expressions().contains_key(value_receiver) {
            self.facts.set_expression(
                *value_receiver,
                CheckedExpression::new(
                    receiver_type.clone(),
                    CheckedTypeSelection::Inferred,
                    EffectSet::new(),
                    CheckedExpressionResolution::Structural,
                ),
            );
        }
        Ok(())
    }

    fn probe_resolved_call(
        &mut self,
        source: CallSource<'_>,
        resolution: &mut ResolvedCallQuery,
    ) -> Result<CandidateProbeBatch, FinalSemanticAnalysisError> {
        let singleton = resolution.considered.len() == 1;
        let mut singleton_checkpoint = None;
        let mut probes = Vec::with_capacity(resolution.considered.len());
        for candidate in &resolution.considered {
            self.control.check()?;
            resolution
                .work
                .record_candidate_argument_probes(resolution.argument_count)
                .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
                    owner: source.owner,
                })?;
            resolution
                .work
                .charge_argument_mapping(resolution.argument_count)
                .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
                    owner: source.owner,
                })?;
            let candidate_group = if resolution.function_value_type.is_some() {
                resolution.current_group
            } else {
                candidate.call_group()
            };
            let checkpoint = self.facts.begin_candidate_transaction();
            let probe = self.probe_call_candidate(
                CandidateProbeRequest {
                    module: source.module,
                    owner: source.owner,
                    arguments: source.call.arguments(),
                    candidate,
                    current_group: candidate_group,
                    expected_result: source.expected,
                    pass: CandidateEvaluationPass::Probe,
                },
                &mut resolution.work,
            );
            let mut probe = match probe {
                Ok(probe) => probe,
                Err(error) => {
                    self.facts.rollback_candidate_transaction(checkpoint);
                    return Err(error);
                }
            };
            probe.projection = self.facts.capture_candidate_projection(checkpoint);
            if singleton {
                singleton_checkpoint = Some(checkpoint);
            } else {
                self.facts.rollback_candidate_transaction(checkpoint);
            }
            probes.push(probe);
        }
        Ok(CandidateProbeBatch {
            probes,
            singleton_checkpoint,
        })
    }

    #[expect(
        clippy::too_many_lines,
        reason = "selected-call publication validates the complete semantic candidate and argument-accounting record"
    )]
    fn publish_selected_call(
        &mut self,
        source: CallSource<'_>,
        mut resolution: ResolvedCallQuery,
        mut batch: CandidateProbeBatch,
        selected_index: usize,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let selected = batch.probes[selected_index].candidate.clone();
        let current_group = batch.probes[selected_index].current_group;
        let arguments = if let Some(checkpoint) = batch.singleton_checkpoint.take() {
            self.facts.commit_candidate_transaction(checkpoint);
            std::mem::take(&mut batch.probes[selected_index].arguments)
        } else {
            resolution
                .work
                .record_selected_replay_argument_visits(resolution.argument_count)
                .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
                    owner: source.owner,
                })?;
            let checkpoint = self.facts.begin_candidate_transaction();
            let replay = self.commit_call_arguments(
                source.owner,
                source.call.arguments(),
                &selected,
                &batch.probes[selected_index].mapping,
                CandidateEvaluationPass::SelectedReplay,
                &mut resolution.work,
            );
            match replay {
                Ok(arguments) => {
                    self.facts.commit_candidate_transaction(checkpoint);
                    arguments
                }
                Err(error) => {
                    self.facts.rollback_candidate_transaction(checkpoint);
                    return Err(error);
                }
            }
        };
        resolution
            .work
            .record_retained_argument_fact_publications(resolution.argument_count)
            .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
                owner: source.owner,
            })?;

        let result = batch.probes[selected_index].result.clone();
        let effects = provisional_call_effects(&selected, current_group)?;
        let callee_expression = self.stage_resolved_callee_expression(
            source.module,
            source.call,
            &selected,
            &result,
            &provisional_callable_effects(&selected),
        )?;
        let expression_resolution = self
            .checked_character_dialogue_resolution(source, &selected, resolution.dialogue_context)?
            .unwrap_or(CheckedExpressionResolution::Call);
        let mut checked_target = CheckedCallTarget::selected(
            &selected,
            &resolution.considered,
            arguments.clone(),
            result.clone(),
            effects.clone(),
            current_group,
            CallPoison::Clean,
        );
        if let Some(function_value_type) = &resolution.function_value_type {
            checked_target = checked_target.with_function_value_type(function_value_type.clone());
        }
        let pending = PendingCallAnalysis {
            expression: source.owner,
            expression_resolution: expression_resolution.clone(),
            callee_expression,
            enclosing_callable: self.enclosing_ordinary_callable(source.module, source.owner)?,
            callee: resolution.callee,
            selected,
            considered: resolution.considered,
            arguments,
            result: result.clone(),
            current_group,
            function_value_type: resolution.function_value_type,
            accounting: resolution.work.call_accounting(),
        };
        let facts = CallTargetFacts::try_new(
            CallTargetFactsInput {
                expression: pending.expression,
                enclosing_callable: pending.enclosing_callable.clone(),
                callee: Some(pending.callee),
                checked: checked_target,
                diagnostics: Vec::new(),
                accounting: pending.accounting,
            },
            &self.catalogs.callable_limits,
        )
        .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
            owner: source.owner,
        })?;
        if self.facts.set_pending_call(source.owner, pending)
            || self.facts.set_call_fact(source.owner, facts)
        {
            return Err(FinalSemanticAnalysisError::CallResolutionFailed {
                owner: source.owner,
            });
        }
        Ok(CheckedExpression::new(
            result,
            if source.expected.is_some() {
                CheckedTypeSelection::Expected
            } else {
                CheckedTypeSelection::Inferred
            },
            effects.concrete().clone(),
            expression_resolution,
        ))
    }

    fn publish_ambiguous_call(
        &mut self,
        source: CallSource<'_>,
        resolution: ResolvedCallQuery,
        mut batch: CandidateProbeBatch,
        primary: usize,
        tied: Vec<usize>,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let ResolvedCallQuery {
            callee,
            considered,
            work,
            ..
        } = resolution;
        if let Some(checkpoint) = batch.singleton_checkpoint.take() {
            self.facts.rollback_candidate_transaction(checkpoint);
        }
        let projection = std::mem::take(&mut batch.probes[primary].projection);
        self.facts.apply_candidate_projection(projection);
        let candidates = tied
            .into_iter()
            .map(|index| batch.probes[index].candidate.clone())
            .collect::<Vec<_>>();
        let arguments = std::mem::take(&mut batch.probes[primary].arguments);
        let result = batch.probes[primary].result.clone();
        self.publish_recovery_call(RecoveryCall {
            source,
            callee,
            candidates: &candidates,
            considered: &considered,
            current_group: batch.probes[primary].current_group,
            arguments,
            result,
            work,
            ambiguous: true,
        })
    }

    fn publish_rejected_call(
        &mut self,
        source: CallSource<'_>,
        mut resolution: ResolvedCallQuery,
        mut batch: CandidateProbeBatch,
        primary: usize,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let primary_candidate = batch.probes[primary].candidate.clone();
        let arguments = if let Some(checkpoint) = batch.singleton_checkpoint.take() {
            self.facts.rollback_candidate_transaction(checkpoint);
            let checkpoint = self.facts.begin_candidate_transaction();
            let replay = self.replay_rejected_call_arguments(
                source.owner,
                source.call.arguments(),
                &batch.probes[primary].candidate,
                &batch.probes[primary].mapping,
                batch.probes[primary].shape_rejected,
                &mut resolution.work,
            );
            match replay {
                Ok(arguments) => {
                    self.facts.commit_candidate_transaction(checkpoint);
                    arguments
                }
                Err(error) => {
                    self.facts.rollback_candidate_transaction(checkpoint);
                    return Err(error);
                }
            }
        } else {
            let projection = std::mem::take(&mut batch.probes[primary].projection);
            self.facts.apply_candidate_projection(projection);
            std::mem::take(&mut batch.probes[primary].arguments)
        };
        let _ = self.checked_character_dialogue_resolution(
            source,
            &primary_candidate,
            resolution.dialogue_context,
        )?;
        let result = batch.probes[primary].result.clone();
        self.publish_recovery_call(RecoveryCall {
            source,
            callee: resolution.callee,
            candidates: &resolution.considered,
            considered: &resolution.considered,
            current_group: batch.probes[primary].current_group,
            arguments,
            result,
            work: resolution.work,
            ambiguous: false,
        })
    }

    fn publish_recovery_call(
        &mut self,
        recovery: RecoveryCall<'_>,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let RecoveryCall {
            source,
            callee,
            candidates,
            considered,
            current_group,
            arguments,
            result,
            mut work,
            ambiguous,
        } = recovery;
        let primary =
            candidates
                .first()
                .ok_or(FinalSemanticAnalysisError::CallResolutionFailed {
                    owner: source.owner,
                })?;
        work.record_retained_argument_fact_publications(
            u64::try_from(arguments.len())
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
        )
        .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
            owner: source.owner,
        })?;
        self.stage_resolved_callee_expression(
            source.module,
            source.call,
            primary,
            &result,
            &provisional_callable_effects(primary),
        )?;
        let checked = if ambiguous {
            CheckedCallTarget::ambiguous(
                candidates,
                considered,
                arguments,
                result.clone(),
                current_group,
            )
        } else {
            CheckedCallTarget::rejected(candidates, arguments, result.clone(), current_group)
        };
        let enclosing_callable = self.enclosing_ordinary_callable(source.module, source.owner)?;
        let facts = CallTargetFacts::try_new(
            CallTargetFactsInput {
                expression: source.owner,
                enclosing_callable,
                callee: Some(callee),
                checked,
                diagnostics: Vec::new(),
                accounting: work.call_accounting(),
            },
            &self.catalogs.callable_limits,
        )
        .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed {
            owner: source.owner,
        })?;
        if self.facts.set_call_fact(source.owner, facts) {
            return Err(FinalSemanticAnalysisError::CallResolutionFailed {
                owner: source.owner,
            });
        }
        let selection = if source
            .expected
            .is_some_and(|expected| expected.accepts(&result))
        {
            CheckedTypeSelection::Expected
        } else {
            CheckedTypeSelection::Inferred
        };
        Ok(CheckedExpression::new(
            result,
            selection,
            EffectSet::new(),
            CheckedExpressionResolution::Call,
        ))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "callee child staging exhaustively follows the final-HIR callee variants and their typed owners"
    )]
    fn stage_call_callee_children(
        &mut self,
        module: &HirModule,
        call: &HirCallExpr,
    ) -> Result<Option<AssociatedReceiverRecovery>, FinalSemanticAnalysisError> {
        match call.callee() {
            HirCallCallee::Value { value } => {
                let expression = module
                    .resolve_expr(*value)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                if let HirExprKind::Select(select) = expression.kind() {
                    self.check_expression(select.target(), None)?;
                } else if !matches!(expression.kind(), HirExprKind::Path(_)) {
                    self.check_expression(*value, None)?;
                } else if let HirExprKind::Path(path) = expression.kind() {
                    let path = path
                        .as_resolved()
                        .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
                    if let Some(resolution) =
                        self.resolve_path_value(module, *value, expression.scope(), path)?
                    {
                        let ty = self.staged_value_resolution_type(&resolution, *value)?;
                        if let Some(ty) = ty
                            && !self.facts.expressions().contains_key(value)
                        {
                            self.facts.set_expression(
                                *value,
                                CheckedExpression::new(
                                    ty,
                                    CheckedTypeSelection::Inferred,
                                    EffectSet::new(),
                                    CheckedExpressionResolution::Value(resolution),
                                ),
                            );
                        }
                    }
                }
            }
            HirCallCallee::UnresolvedDot {
                value_receiver,
                nominal_receiver,
                separator,
                member,
            } => {
                let expression = module
                    .resolve_expr(*value_receiver)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                if !matches!(expression.kind(), HirExprKind::Path(_)) {
                    self.check_expression(*value_receiver, None)?;
                } else if let HirExprKind::Path(path) = expression.kind() {
                    let path = path
                        .as_resolved()
                        .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
                    let member = member
                        .resolved()
                        .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
                    let full_path = path.with_terminal_member(member);
                    let full_resolution = self.resolve_path_value(
                        module,
                        *value_receiver,
                        expression.scope(),
                        &full_path,
                    )?;
                    match full_resolution {
                        Some(resolution) => {
                            if let Some(ty) =
                                self.staged_value_resolution_type(&resolution, *value_receiver)?
                                && !self.facts.expressions().contains_key(value_receiver)
                            {
                                self.facts.set_expression(
                                    *value_receiver,
                                    CheckedExpression::new(
                                        ty,
                                        CheckedTypeSelection::Inferred,
                                        EffectSet::new(),
                                        CheckedExpressionResolution::Value(resolution),
                                    ),
                                );
                            }
                        }
                        None => {
                            match self.resolve_path_value(
                                module,
                                *value_receiver,
                                expression.scope(),
                                path,
                            )? {
                                Some(resolution) => {
                                    if let Some(ty) = self.staged_value_resolution_type(
                                        &resolution,
                                        *value_receiver,
                                    )? && !self.facts.expressions().contains_key(value_receiver)
                                    {
                                        self.facts.set_expression(
                                            *value_receiver,
                                            CheckedExpression::new(
                                                ty,
                                                CheckedTypeSelection::Inferred,
                                                EffectSet::new(),
                                                CheckedExpressionResolution::Value(resolution),
                                            ),
                                        );
                                    }
                                }
                                None => {
                                    let line_context = path.lexical_name() == Some("line")
                                        && scope_is_dialogue_line_plan(module, expression.scope());
                                    if line_context {
                                        self.facts.set_expression(
                                            *value_receiver,
                                            CheckedExpression::new(
                                                TypeKind::Named("LineContext".to_owned()),
                                                CheckedTypeSelection::Inferred,
                                                EffectSet::new(),
                                                CheckedExpressionResolution::Value(
                                                    CheckedValueResolution::LineContext,
                                                ),
                                            ),
                                        );
                                    } else if prepare_language_free_dot_path(
                                        self.catalogs.world.environment().callable_catalog(),
                                        *value_receiver,
                                        expression,
                                        member,
                                        &self.catalogs.callable_limits,
                                    )
                                    .map_err(|_| {
                                        FinalSemanticAnalysisError::CallResolutionFailed {
                                            owner: *value_receiver,
                                        }
                                    })?
                                    .is_none()
                                    {
                                        let receiver = nominal_receiver.type_id().ok_or(
                                            FinalSemanticAnalysisError::CallResolutionFailed {
                                                owner: *value_receiver,
                                            },
                                        )?;
                                        match self.resolve_associated_receiver_type(receiver)? {
                                            AssociatedReceiverTypeResolution::Complete(_) => {}
                                            AssociatedReceiverTypeResolution::WrongArity(
                                                result,
                                            ) => {
                                                return Ok(Some(AssociatedReceiverRecovery {
                                                    receiver,
                                                    separator: *separator,
                                                    result,
                                                }));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            HirCallCallee::Associated {
                receiver,
                separator,
                ..
            } => {
                let receiver = receiver
                    .type_id()
                    .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
                match self.resolve_associated_receiver_type(receiver)? {
                    AssociatedReceiverTypeResolution::Complete(_) => {}
                    AssociatedReceiverTypeResolution::WrongArity(result) => {
                        return Ok(Some(AssociatedReceiverRecovery {
                            receiver,
                            separator: *separator,
                            result,
                        }));
                    }
                }
            }
        }
        Ok(None)
    }

    fn staged_value_resolution_type(
        &self,
        resolution: &CheckedValueResolution,
        expression: ExprId,
    ) -> Result<Option<TypeKind>, FinalSemanticAnalysisError> {
        match resolution {
            CheckedValueResolution::Local(local) => self
                .facts
                .locals()
                .get(local)
                .cloned()
                .map(Some)
                .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: *local }),
            CheckedValueResolution::ProjectCallable(_) => Ok(None),
            _ => value_resolution_type(self.catalogs.world, resolution)
                .map(Some)
                .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: expression }),
        }
    }

    fn specialize_presentation_character_candidates(
        &mut self,
        module: &HirModule,
        call: ExprId,
        arguments: &[HirCallArgument],
        candidates: &mut [ResolvedCallable],
    ) -> Result<(), FinalSemanticAnalysisError> {
        for candidate in candidates {
            let Some(owner) = self.presentation_character_owner(module, arguments, candidate)
            else {
                continue;
            };
            *candidate = candidate
                .try_with_presentation_character_owner(
                    owner,
                    self.catalogs.world.environment(),
                    &self.catalogs.callable_limits,
                )
                .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed { owner: call })?;
        }
        Ok(())
    }

    fn presentation_character_owner(
        &mut self,
        module: &HirModule,
        arguments: &[HirCallArgument],
        candidate: &ResolvedCallable,
    ) -> Option<ResolvedCharacterOwner> {
        let group = candidate.schema().group(candidate.call_group())?;
        let parameter = group.parameters().iter().find(|parameter| {
            parameter
                .name()
                .is_some_and(|name| name.as_str() == "character")
        })?;
        let mapping = map_call_arguments(
            module,
            candidate.schema(),
            candidate.call_group(),
            arguments,
            None,
        )?;
        let argument = mapping
            .arguments()
            .iter()
            .position(|argument| {
                argument.slots().iter().any(|slot| {
                    slot.coordinate().is_some_and(|coordinate| {
                        coordinate.group() == group.index()
                            && coordinate.parameter() == parameter.index()
                    })
                })
            })
            .and_then(|index| arguments.get(index))?;
        let checked = self.check_expression(argument.value(), None).ok()?;
        let CheckedExpressionResolution::Value(CheckedValueResolution::ProjectItem(item)) =
            checked.resolution()
        else {
            return None;
        };
        let character = item.character()?;
        self.catalogs
            .world
            .environment()
            .character_manifest(&character)?;
        Some(ResolvedCharacterOwner::new(
            character,
            CharacterOwnerSource::EntityReference,
        ))
    }

    fn probe_call_candidate(
        &mut self,
        request: CandidateProbeRequest<'_>,
        work: &mut ResolverWork,
    ) -> Result<CandidateProbe, FinalSemanticAnalysisError> {
        let CandidateProbeRequest {
            module,
            owner,
            arguments,
            candidate,
            current_group,
            expected_result,
            pass,
        } = request;
        let implicit = match candidate.instantiation() {
            CallableInstantiation::Extension {
                group, parameter, ..
            } if *group == current_group => Some(*parameter),
            _ => None,
        };
        let mapped = map_call_arguments(
            module,
            candidate.schema(),
            current_group,
            arguments,
            implicit,
        );
        let shape_rejected = mapped.is_none();
        let mapping = mapped
            .or_else(|| map_unmapped_call_arguments(arguments))
            .ok_or(FinalSemanticAnalysisError::CallResolutionFailed { owner })?;
        let mut score = CandidateScore {
            hard_errors: usize::from(shape_rejected),
            exact_matches: 0,
            unchecked_or_open: mapping.unchecked_or_open_slots(),
            omitted_parameters: mapping.omitted_parameters(),
            authority: candidate.authority(),
        };
        let evaluated = self.evaluate_call_arguments(
            ArgumentEvaluationRequest {
                call: owner,
                authored: arguments,
                candidate,
                mapping: &mapping,
                pass,
                shape_rejected,
            },
            work,
        )?;
        score.exact_matches = score
            .exact_matches
            .checked_add(evaluated.exact_matches)
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        score.hard_errors = score
            .hard_errors
            .checked_add(evaluated.hard_errors)
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        let result = evaluated.substitutions.apply(
            &call_result_type(candidate, current_group)
                .ok_or(FinalSemanticAnalysisError::CallResolutionFailed { owner })?,
        );
        let result = inferred_constructor_result(candidate, &evaluated.arguments, result);
        if let Some(expected) = expected_result {
            if expected.accepts(&result) {
                if expected == &result {
                    score.exact_matches = score
                        .exact_matches
                        .checked_add(1)
                        .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
                }
            } else {
                score.hard_errors = score
                    .hard_errors
                    .checked_add(1)
                    .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
            }
        }
        Ok(CandidateProbe {
            candidate: candidate.clone(),
            current_group,
            mapping,
            arguments: evaluated.arguments,
            projection: CandidateSemanticProjection::default(),
            result,
            score,
            shape_rejected,
        })
    }

    fn check_call_argument_slot(
        &mut self,
        source: CheckedCallArgumentSlotSource,
        expected: Option<&TypeKind>,
    ) -> Result<TypeKind, FinalSemanticAnalysisError> {
        match source {
            CheckedCallArgumentSlotSource::Expression(expression) => self
                .check_expression(expression, expected)
                .map(|checked| checked.ty().clone()),
            CheckedCallArgumentSlotSource::CompactNumericElement { sequence, ordinal } => {
                self.compact_numeric_element_type(sequence, ordinal, expected)
            }
        }
    }

    fn evaluate_call_arguments(
        &mut self,
        request: ArgumentEvaluationRequest<'_>,
        work: &mut ResolverWork,
    ) -> Result<EvaluatedCallArguments, FinalSemanticAnalysisError> {
        let ArgumentEvaluationRequest {
            call,
            authored,
            candidate,
            mapping,
            pass,
            shape_rejected,
        } = request;
        if mapping.arguments().len() != authored.len() {
            return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner: call });
        }
        let mut arguments = Vec::with_capacity(authored.len());
        let mut hard_errors = 0usize;
        let mut exact_matches = 0usize;
        let mut substitutions = TypeParameterSubstitutions::default();
        for (argument_index, (authored, mapped)) in
            authored.iter().zip(mapping.arguments()).enumerate()
        {
            let argument =
                arcweft_lang_hir::expr::HirCallArgumentOrdinal::try_from_usize(argument_index)
                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
            let evaluated = self.evaluate_mapped_call_argument(
                MappedArgumentEvaluationRequest {
                    call,
                    authored,
                    mapped,
                    candidate,
                    pass,
                    shape_rejected,
                    argument,
                },
                work,
                &mut substitutions,
            )?;
            hard_errors = hard_errors
                .checked_add(evaluated.hard_errors)
                .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
            exact_matches = exact_matches
                .checked_add(evaluated.exact_matches)
                .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
            arguments.push(evaluated.fact);
        }
        Ok(EvaluatedCallArguments {
            arguments,
            hard_errors,
            exact_matches,
            substitutions,
        })
    }

    fn evaluate_mapped_call_argument(
        &mut self,
        request: MappedArgumentEvaluationRequest<'_>,
        work: &mut ResolverWork,
        substitutions: &mut TypeParameterSubstitutions,
    ) -> Result<MappedArgumentEvaluation, FinalSemanticAnalysisError> {
        let MappedArgumentEvaluationRequest {
            mapped,
            shape_rejected,
            argument,
            ..
        } = request;
        let mut slots = Vec::with_capacity(mapped.slots().len());
        let mut hard_errors = 0usize;
        let mut exact_matches = 0usize;
        let mut argument_poison = if shape_rejected {
            CallPoison::Rejected
        } else {
            CallPoison::Clean
        };
        for slot in mapped.slots() {
            let evaluated = self.evaluate_mapped_call_slot(request, slot, work, substitutions)?;
            if evaluated.hard_error {
                hard_errors = hard_errors
                    .checked_add(1)
                    .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
            } else if evaluated.exact_match {
                exact_matches = exact_matches
                    .checked_add(1)
                    .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
            }
            argument_poison = argument_poison.merge(evaluated.poison);
            slots.push(evaluated.fact);
        }
        Ok(MappedArgumentEvaluation {
            fact: CheckedCallArgumentFact::new(argument, slots, argument_poison),
            hard_errors,
            exact_matches,
        })
    }

    fn evaluate_mapped_call_slot(
        &mut self,
        request: MappedArgumentEvaluationRequest<'_>,
        slot: &MappedCallArgumentSlot,
        work: &mut ResolverWork,
        substitutions: &mut TypeParameterSubstitutions,
    ) -> Result<MappedSlotEvaluation, FinalSemanticAnalysisError> {
        let MappedArgumentEvaluationRequest {
            call,
            authored,
            candidate,
            pass,
            shape_rejected,
            argument,
            ..
        } = request;
        self.control.check_physical_slot_boundary()?;
        work.charge_type_check(1)
            .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed { owner: call })?;
        self.facts.prepare_physical_slot_evaluation(slot.source());
        let is_dialogue_patch_coordinate = matches!(
            candidate.schema().validator(),
            crate::callable::CallableValidator::Dialogue(
                crate::callable::DialogueCallableId::CharacterFactory
                    | crate::callable::DialogueCallableId::CharacterReconfigure
            )
        ) && slot.coordinate().is_some_and(|coordinate| {
            candidate
                .schema()
                .group(coordinate.group())
                .and_then(|group| group.parameter(coordinate.parameter()))
                .and_then(|parameter| parameter.name())
                .is_none_or(|name| !matches!(name.as_str(), "id" | "text_key"))
        });
        let clear_expected = if is_dialogue_patch_coordinate
            && is_none_patch_value(self.module(call.module())?, authored.value())?
        {
            Some(TypeKind::Option(Box::new(
                slot.expected()
                    .cloned()
                    .unwrap_or_else(|| TypeKind::Named("_".to_owned())),
            )))
        } else {
            None
        };
        let declared_expected = clear_expected.as_ref().or_else(|| slot.expected());
        let expected =
            declared_expected.and_then(|expected| substitutions.apply_argument_expected(expected));
        let physical_expected = physical_expected_type(
            expected.as_ref(),
            slot.coordinate().is_some(),
            shape_rejected,
        );
        let kind = physical_evaluation_kind(
            authored,
            slot,
            shape_rejected,
            candidate.schema().argument_policy().spread(),
        );
        self.record_physical_candidate_argument_evaluation(
            PhysicalCandidateArgumentEvaluation::new(
                call,
                candidate.id().clone(),
                pass,
                PhysicalCandidateArgument::new(
                    argument,
                    slot.slot(),
                    slot.source(),
                    kind,
                    physical_expected,
                ),
            ),
        )?;
        let inferred = self.check_call_argument_slot(slot.source(), expected.as_ref())?;
        let substitution_conflict =
            declared_expected.is_some_and(|declared| !substitutions.observe(declared, &inferred));
        let retained_expected = declared_expected.map(|expected| substitutions.apply(expected));
        let mismatch = substitution_conflict
            || retained_expected
                .as_ref()
                .is_some_and(|expected| !expected.accepts(&inferred));
        let poison = if shape_rejected || mismatch {
            CallPoison::Rejected
        } else {
            CallPoison::Clean
        };
        Ok(MappedSlotEvaluation {
            hard_error: mismatch,
            exact_match: retained_expected.as_ref() == Some(&inferred),
            fact: CheckedCallArgumentSlotFact::new(CheckedCallArgumentSlotInput {
                slot: slot.slot(),
                source: slot.source(),
                mapped: slot.coordinate(),
                inferred: Some(inferred),
                expected: retained_expected,
                poison,
            }),
            poison,
        })
    }

    fn compact_numeric_element_type(
        &self,
        owner: ExprId,
        ordinal: u32,
        expected: Option<&TypeKind>,
    ) -> Result<TypeKind, FinalSemanticAnalysisError> {
        let module = self.module(owner.module())?;
        let expression = module
            .resolve_expr(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirExprKind::NumericBracketSequence(sequence) = expression.kind() else {
            return Err(FinalSemanticAnalysisError::InvalidOwner);
        };
        let ordinal =
            usize::try_from(ordinal).map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        sequence
            .elements()
            .get(ordinal)
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        Ok(infer_compact_numeric_element_type(
            sequence.common_suffix(),
            expected,
        ))
    }

    fn commit_call_arguments(
        &mut self,
        call: ExprId,
        arguments: &[HirCallArgument],
        candidate: &ResolvedCallable,
        mapping: &CallArgumentMapping,
        pass: CandidateEvaluationPass,
        work: &mut ResolverWork,
    ) -> Result<Vec<CheckedCallArgumentFact>, FinalSemanticAnalysisError> {
        let evaluated = self.evaluate_call_arguments(
            ArgumentEvaluationRequest {
                call,
                authored: arguments,
                candidate,
                mapping,
                pass,
                shape_rejected: false,
            },
            work,
        )?;
        if evaluated.hard_errors != 0 {
            return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner: call });
        }
        Ok(evaluated.arguments)
    }

    fn replay_rejected_call_arguments(
        &mut self,
        call: ExprId,
        arguments: &[HirCallArgument],
        candidate: &ResolvedCallable,
        mapping: &CallArgumentMapping,
        shape_rejected: bool,
        work: &mut ResolverWork,
    ) -> Result<Vec<CheckedCallArgumentFact>, FinalSemanticAnalysisError> {
        self.evaluate_call_arguments(
            ArgumentEvaluationRequest {
                call,
                authored: arguments,
                candidate,
                mapping,
                pass: CandidateEvaluationPass::RejectedRecoveryReplay,
                shape_rejected,
            },
            work,
        )
        .map(|evaluated| evaluated.arguments)
    }

    fn stage_resolved_callee_expression(
        &mut self,
        module: &HirModule,
        call: &HirCallExpr,
        selected: &ResolvedCallable,
        result: &TypeKind,
        callable_effects: &EffectRow,
    ) -> Result<Option<ExprId>, FinalSemanticAnalysisError> {
        let (value, nominal_receiver) = match call.callee() {
            HirCallCallee::Value { value } => (*value, false),
            HirCallCallee::UnresolvedDot { value_receiver, .. }
                if !self.facts.expressions().contains_key(value_receiver) =>
            {
                (*value_receiver, true)
            }
            HirCallCallee::UnresolvedDot { .. } | HirCallCallee::Associated { .. } => {
                return Ok(None);
            }
        };
        let expression = module
            .resolve_expr(value)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let method_callee = match expression.kind() {
            HirExprKind::Select(select) if !nominal_receiver => {
                let HirSelectedMember::Name(name) = select.member() else {
                    return Err(FinalSemanticAnalysisError::RecoveredOwner);
                };
                Some((select.target(), name.clone()))
            }
            _ => None,
        };
        let retained_resolution = if method_callee.is_some() {
            None
        } else if let Some(existing) = self.facts.expressions().get(&value) {
            if nominal_receiver {
                return Ok(None);
            }
            match existing.resolution() {
                CheckedExpressionResolution::Value(
                    CheckedValueResolution::ProjectCallable(_)
                    | CheckedValueResolution::Registered(_),
                ) => Some(existing.resolution().clone()),
                _ => return Ok(None),
            }
        } else {
            None
        };
        let ty = if nominal_receiver {
            match selected.instantiation() {
                CallableInstantiation::TypeReceiver { receiver } => receiver.receiver().clone(),
                _ => selected.schema().result().clone(),
            }
        } else {
            instantiated_callee_type(selected, result, callable_effects)
                .ok_or(FinalSemanticAnalysisError::CallResolutionFailed { owner: value })?
        };
        let resolution = if let Some((receiver, name)) = &method_callee {
            match selected.instantiation() {
                CallableInstantiation::Receiver {
                    receiver: selected_receiver,
                } if self
                    .facts
                    .expressions()
                    .get(receiver)
                    .is_some_and(|checked| checked.ty() == selected_receiver) => {}
                _ => return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner: value }),
            }
            CheckedExpressionResolution::Select(super::CheckedSelectResolution::Method {
                name: name.clone(),
            })
        } else if let Some(resolution) = retained_resolution {
            resolution
        } else if nominal_receiver {
            CheckedExpressionResolution::Structural
        } else if let crate::callable::CallableCandidateId::Project(declaration) = selected.id() {
            let symbol = self
                .symbols
                .callable(declaration)
                .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
            CheckedExpressionResolution::Value(CheckedValueResolution::ProjectCallable(
                super::CheckedProjectCallable::new(declaration.clone(), symbol.source_item()),
            ))
        } else {
            CheckedExpressionResolution::Value(CheckedValueResolution::Registered(
                RegisteredSemanticValueId::from_bytes(
                    *selected.schema().semantic_digest().as_bytes(),
                ),
            ))
        };
        let effects = method_callee
            .as_ref()
            .and_then(|(receiver, _)| self.facts.expressions().get(receiver))
            .map_or_else(EffectSet::new, |receiver| receiver.effects().clone());
        self.facts.set_expression(
            value,
            CheckedExpression::new(ty, CheckedTypeSelection::Inferred, effects, resolution),
        );
        Ok((!nominal_receiver).then_some(value))
    }
}

fn inferred_constructor_result(
    candidate: &ResolvedCallable,
    arguments: &[CheckedCallArgumentFact],
    declared: TypeKind,
) -> TypeKind {
    if !matches!(
        candidate.instantiation(),
        CallableInstantiation::Option { expected: None }
    ) {
        return declared;
    }
    let [argument] = arguments else {
        return declared;
    };
    let [slot] = argument.slots() else {
        return declared;
    };
    slot.inferred()
        .cloned()
        .map(|item| TypeKind::Option(Box::new(item)))
        .unwrap_or(declared)
}

pub(super) fn instantiated_callee_type(
    selected: &ResolvedCallable,
    result: &TypeKind,
    effects: &EffectRow,
) -> Option<TypeKind> {
    if matches!(
        selected.instantiation(),
        CallableInstantiation::Option { expected: None }
    ) && let TypeKind::Option(item) = result
    {
        return Some(TypeKind::function_with_effects(
            [item.as_ref().clone()],
            result.clone(),
            effects.clone(),
        ));
    }
    callable_schema_type_with_effects(selected.schema(), effects)
}

fn scope_is_dialogue_line_plan(module: &HirModule, mut scope: ScopeId) -> bool {
    loop {
        if module.expressions().any(|(_, expression)| {
            matches!(
                expression.kind(),
                HirExprKind::DialogueContentApplication(application)
                    if application.plan().is_some_and(|plan| plan.root_scope() == scope)
            )
        }) {
            return true;
        }
        let Ok(current) = module.resolve_scope(scope) else {
            return false;
        };
        let Some(parent) = current.parent() else {
            return false;
        };
        scope = parent;
    }
}
