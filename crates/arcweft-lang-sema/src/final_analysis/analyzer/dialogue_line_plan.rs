use super::*;

impl Analyzer<'_, '_, '_> {
    pub(super) fn prepare_dialogue_content_application(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        application: &arcweft_lang_hir::dialogue_application::HirDialogueContentApplication,
        expectation: &AnalyzerExpressionExpectation<'_>,
    ) -> Result<PreparedExpressionFact, AnalyzerExpressionError> {
        let expected = expectation.contextual_shape();
        // Immediate id/text_key arguments are compile-time application
        // coordinates. Publish their accepted project identities before the
        // shared parenthesized-call resolver evaluates argument facts.
        let dialogue_application_metadata =
            self.publish_dialogue_coordinates(module, owner, application)?;
        let target_owner = application.target();
        let target_expression = module.resolve_expr(target_owner).map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
        })?;
        let checked_target = match target_expression.kind() {
            HirExprKind::Call(call) => {
                let dialogue_application_metadata =
                    dialogue_application_metadata.as_ref().ok_or_else(|| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        )
                    })?;
                let checked = self.check_call_expression_in_context(
                    context,
                    module,
                    target_owner,
                    call,
                    None,
                    Some(dialogue_application_metadata),
                )?;
                let write = if context.is_candidate()
                    && self.facts.expressions().contains_key(&target_owner)
                {
                    self.facts
                        .replace_existing_expression(target_owner, checked.clone())
                } else {
                    self.facts
                        .publish_new_expression(target_owner, checked.clone())
                };
                write.map_err(|_| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
                })?;
                checked.into()
            }
            _ => self.evaluate_expression(context, target_owner, None)?,
        };
        let target = checked_character_dialogue_target(target_owner, &checked_target)
            .map_err(|error| AnalyzerExpressionError::Call {
                owner,
                failure: super::super::calls::CallAnalysisFailure::Invariant(
                    super::super::calls::CallAnalysisInvariant::Constraint(error),
                ),
            })?
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        let application_patch = match checked_target.checked_resolution() {
            Some(CheckedExpressionResolution::CharacterDialogueFactory(factory)) => {
                Some(factory.patch().clone())
            }
            Some(CheckedExpressionResolution::CharacterDialogueReconfigure(reconfigure)) => {
                Some(reconfigure.patch().clone())
            }
            _ => None,
        };
        let rich_text =
            RichTextAttributeChecker::check(module, application.content()).map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::RichTextSourceQuery {
                    owner,
                })
            })?;
        if !rich_text.is_valid() {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::InvalidRichTextAttributes {
                    owner,
                    report: Box::new(rich_text),
                },
            ));
        }
        let application_children = module
            .resolve_expr(owner)
            .map_err(|_| AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner))?
            .kind()
            .direct_expression_children();
        for child in application_children {
            if child != target_owner {
                self.evaluate_expression(context, child, None)?;
            }
        }

        let line_plan = self.prepared_dialogue_line_plan(module, application.plan(), &rich_text)?;
        let line_result = application.plan().map_or(Ok(TypeKind::Unit), |plan| {
            self.check_dialogue_line_plan_output(context, plan.items())
        })?;
        let call_target = target.clone();
        let ty = self.publish_dialogue_content_application_call(
            module,
            owner,
            &call_target,
            expected,
            &line_result,
            application.plan().is_some(),
        )?;
        let selection = match expected.map(|expected| expected.accepts(&ty)) {
            Some(true) => CheckedTypeSelection::Expected,
            None => CheckedTypeSelection::Inferred,
            Some(false) => {
                return Err(AnalyzerExpressionError::rejected(owner));
            }
        };
        let nested_path_evidence = self.dialogue_nested_path_evidence(owner);
        let shell = PreparedExpressionShell::new(ty, selection, EffectSet::new());
        let prepared = PreparedDialogueApplication::try_new(
            shell,
            target,
            application_patch,
            Box::new(rich_text),
            line_plan,
            line_result,
            nested_path_evidence,
        )
        .ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
        })?;
        Ok(PreparedExpressionFact::DialogueApplication(prepared))
    }

    fn dialogue_nested_path_evidence(
        &self,
        owner: ExprId,
    ) -> Option<
        Result<
            crate::final_analysis::NestedPathEvidence,
            crate::final_analysis::CheckedChildEdgeError,
        >,
    > {
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
        crate::final_analysis::match_edges::build_line_plan_nested_path_evidence(
            &edges,
            self.facts.expressions(),
        )
    }

    fn prepared_dialogue_line_plan(
        &self,
        module: &HirModule,
        plan: Option<&arcweft_lang_hir::dialogue_application::HirLinePlan>,
        rich_text: &crate::checked_rich_text::CheckedRichTextReport,
    ) -> Result<PreparedDialogueLinePlan, AnalyzerExpressionError> {
        let mut marks = Vec::<PublicId>::new();
        let mut mark_ordinals = std::collections::BTreeMap::<PublicId, u32>::new();
        let mut effect_sites = Vec::new();
        for token in rich_text.content().tokens() {
            let CheckedDialogueToken::Open(tag) = token else {
                continue;
            };
            match tag.action() {
                CheckedRichTextAction::Marker(mark) => {
                    let ordinal = u32::try_from(marks.len()).map_err(|_| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::AccountingOverflow,
                        )
                    })?;
                    if mark_ordinals.insert(mark.clone(), ordinal).is_some() {
                        return Err(AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        ));
                    }
                    marks.push(mark.clone());
                }
                CheckedRichTextAction::Host {
                    action:
                        event @ (CheckedDialogueHostEvent::TimedCue { .. }
                        | CheckedDialogueHostEvent::Call { .. }),
                    ..
                } => {
                    let id = CheckedDialogueEffectSiteOrdinal::new(
                        u32::try_from(effect_sites.len()).map_err(|_| {
                            AnalyzerExpressionError::fatal(
                                FinalSemanticAnalysisError::AccountingOverflow,
                            )
                        })?,
                    );
                    let (expression, trigger) = match event {
                        CheckedDialogueHostEvent::TimedCue { at, call } => {
                            (*call, CheckedDialogueEffectTrigger::Delay(*at))
                        }
                        CheckedDialogueHostEvent::Call { call } => {
                            (*call, CheckedDialogueEffectTrigger::Content)
                        }
                        _ => unreachable!("the grouped checked RichText effect event is closed"),
                    };
                    let effect = self
                        .prepare_evaluated_effect_expression(module, expression)
                        .map_err(AnalyzerExpressionError::fatal)?
                        .ok_or_else(|| {
                            AnalyzerExpressionError::fatal(
                                FinalSemanticAnalysisError::WrongPayloadFamily,
                            )
                        })?;
                    effect_sites.push(PreparedDialogueEffectSite::new(
                        id, trigger, expression, effect,
                    ));
                }
                CheckedRichTextAction::DirectStyle { .. }
                | CheckedRichTextAction::Control { .. }
                | CheckedRichTextAction::Style { .. }
                | CheckedRichTextAction::Layout { .. }
                | CheckedRichTextAction::Transform { .. }
                | CheckedRichTextAction::Object { .. }
                | CheckedRichTextAction::BuiltinFx { .. }
                | CheckedRichTextAction::Host { .. } => {}
            }
        }
        let mut handlers = Vec::new();
        if let Some(plan) = plan {
            self.collect_dialogue_mark_handlers(
                module,
                plan.items(),
                &mark_ordinals,
                &mut handlers,
            )?;
        }
        Ok(PreparedDialogueLinePlan::new(marks, handlers, effect_sites))
    }

    fn collect_dialogue_mark_handlers(
        &self,
        module: &HirModule,
        items: &[HirLinePlanItem],
        marks: &std::collections::BTreeMap<PublicId, u32>,
        handlers: &mut Vec<CheckedDialogueMarkHandler>,
    ) -> Result<(), AnalyzerExpressionError> {
        for item in items {
            match item {
                HirLinePlanItem::Statement(statement) | HirLinePlanItem::On(statement) => {
                    let statement_payload = module.resolve_stmt(*statement).map_err(|_| {
                        AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                    })?;
                    let HirStmtKind::On {
                        trigger: HirTriggerPattern::Mark(pattern),
                        ..
                    } = statement_payload.kind()
                    else {
                        continue;
                    };
                    let pattern = module.resolve_pattern(*pattern).map_err(|_| {
                        AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
                    })?;
                    let HirPatternKind::EntityReference(HirIdRefValue::Resolved(
                        HirIdRef::Relative(mark),
                    )) = pattern.kind()
                    else {
                        return Err(AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        ));
                    };
                    if mark.parent_depth() != 0 || mark.suffix().as_str().contains('.') {
                        return Err(AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        ));
                    }
                    let mark = PublicId::try_new(mark.suffix().as_str()).map_err(|_| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        )
                    })?;
                    let ordinal = marks.get(&mark).copied().ok_or_else(|| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        )
                    })?;
                    if handlers
                        .iter()
                        .any(|handler| handler.statement() == *statement)
                    {
                        return Err(AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        ));
                    }
                    handlers.push(CheckedDialogueMarkHandler::new(
                        *statement,
                        CheckedDialogueMarkOrdinal::new(ordinal),
                    ));
                }
                HirLinePlanItem::StartGroup(children)
                | HirLinePlanItem::TogetherGroup(children) => {
                    self.collect_dialogue_mark_handlers(module, children, marks, handlers)?
                }
                HirLinePlanItem::Init(_)
                | HirLinePlanItem::Thread(_)
                | HirLinePlanItem::Option { .. }
                | HirLinePlanItem::Let { .. }
                | HirLinePlanItem::Out { .. }
                | HirLinePlanItem::CancelRule(_)
                | HirLinePlanItem::TimelineAssert { .. }
                | HirLinePlanItem::Expression(_)
                | HirLinePlanItem::Error(_) => {}
            }
        }
        Ok(())
    }

    fn publish_dialogue_content_application_call(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        target: &super::super::CheckedCharacterDialogueTarget,
        expected: Option<&TypeKind>,
        line_result: &TypeKind,
        has_line_plan: bool,
    ) -> Result<TypeKind, AnalyzerExpressionError> {
        let callee = match target {
            super::super::CheckedCharacterDialogueTarget::Character { character, .. } => {
                DialogueCalleeIdentity::Character {
                    character: character.clone(),
                }
            }
            super::super::CheckedCharacterDialogueTarget::Dialogue { ty, .. } => {
                DialogueCalleeIdentity::CharacterDialogue {
                    character: ty.character().clone(),
                }
            }
        };
        let prepared = PreparedCallCallee::Dialogue {
            id: DialogueCallableId::ContentApplication,
            callee: &callee,
            patch_context: CharacterDialoguePatchContext::ImmediateContentApplication,
            result: crate::callable::DialogueCallableResultContext::ContentApplication {
                line_result,
            },
        };
        let authority = CallResolverAuthority::accepted(
            self.project,
            module,
            self.symbols,
            self.catalogs.world,
        );
        let staged = self.staged_callables.as_ref().ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CheckedCallableCatalog)
        })?;
        let mut work = ResolverWork::new(self.catalogs.callable_limits.max_query_work());
        let request = CallResolverRequest::try_new_dialogue_application(
            prepared,
            &super::super::CallResolverContext {
                authority,
                checked: (&staged.builder).into(),
                presentation_character_owner: None,
                expression: owner,
                cancellation: self.control.cancellation(),
                prepared_continuations: self
                    .facts
                    .prepared_calls()
                    .map_err(AnalyzerExpressionError::fact)?,
                limits: &self.catalogs.callable_limits,
                implicit_extension_receiver: None,
            },
            &mut work,
        )
        .map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CallResolutionFailed {
                owner,
            })
        })?;
        let outcome = resolve_call_target(request);
        let candidates = match outcome {
            ResolveCallOutcome::Resolved(ResolvedCallTarget::Candidates(candidates)) => candidates,
            ResolveCallOutcome::Invariant(error) => {
                return Err(AnalyzerExpressionError::Call {
                    owner,
                    failure: crate::final_analysis::analyzer::calls::CallAnalysisFailure::Invariant(
                        crate::final_analysis::analyzer::calls::CallAnalysisInvariant::Constraint(
                            error,
                        ),
                    ),
                });
            }
            _ => {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::CallResolutionFailed { owner },
                ));
            }
        };
        let considered = candidates.into_shared().map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CallResolutionFailed {
                owner,
            })
        })?;
        let selected = considered.first().cloned().ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CallResolutionFailed {
                owner,
            })
        })?;
        if selected.id()
            != &crate::callable::CallableCandidateId::Dialogue(
                DialogueCallableId::ContentApplication,
            )
            || !matches!(selected.schema().result(), TypeKind::DialogueLine(_))
        {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::CallResolutionFailed { owner },
            ));
        }
        self.publish_resolved_dialogue_application(
            module,
            owner,
            expected,
            target.expression(),
            target.ty(),
            has_line_plan,
            considered,
            work,
        )
    }

    fn check_dialogue_line_plan_output(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        items: &[arcweft_lang_hir::dialogue_application::HirLinePlanItem],
    ) -> Result<TypeKind, AnalyzerExpressionError> {
        use arcweft_lang_hir::dialogue_application::HirLinePlanItem;

        let mut output: Option<TypeKind> = None;
        let mut output_statements = BTreeSet::new();
        let mut pending = vec![items];
        while let Some(items) = pending.pop() {
            for item in items {
                match item {
                    HirLinePlanItem::Out { value, statement } => {
                        if !output_statements.insert(*statement) {
                            return Err(AnalyzerExpressionError::fatal(
                                FinalSemanticAnalysisError::WrongPayloadFamily,
                            ));
                        }
                        let checked = self.evaluate_expression(context, *value, output.as_ref())?;
                        match &output {
                            Some(expected) if !expected.accepts(checked.ty()) => {
                                return Err(AnalyzerExpressionError::rejected(*value));
                            }
                            Some(_) => {}
                            None => output = Some(checked.ty().clone()),
                        }
                    }
                    HirLinePlanItem::StartGroup(items) | HirLinePlanItem::TogetherGroup(items) => {
                        pending.push(items)
                    }
                    _ => {}
                }
            }
        }
        Ok(output.unwrap_or(TypeKind::Unit))
    }

    fn publish_dialogue_coordinates(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        application: &arcweft_lang_hir::dialogue_application::HirDialogueContentApplication,
    ) -> Result<
        Option<crate::callable::PreparedDialogueApplicationMetadataInventory>,
        AnalyzerExpressionError,
    > {
        let target = module.resolve_expr(application.target()).map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
        })?;
        if !matches!(target.kind(), HirExprKind::Call(_)) {
            return application
                .coordinates()
                .is_empty()
                .then_some(None)
                .ok_or_else(|| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
                });
        }
        let projection = module
            .dialogue_application_metadata_projection(owner)
            .map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
            })?;
        let accepted = if application.coordinates().is_empty() {
            None
        } else {
            Some(
                self.project
                    .dialogue_lines()
                    .for_expr(owner)
                    .ok_or_else(|| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        )
                    })?,
            )
        };
        let mut prepared = Vec::with_capacity(projection.coordinates().len());
        for coordinate in projection.coordinates() {
            let accepted = accepted.ok_or_else(|| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
            })?;
            let (metadata_coordinate, ty, evidence, resolution) = match coordinate.kind() {
                arcweft_lang_hir::dialogue_application::HirDialogueCoordinateKind::Id => (
                    crate::callable::DialogueApplicationMetadataCoordinate::Id,
                    TypeKind::entity_ref(EntityKind::DialogueLine),
                    crate::callable::PreparedDialogueApplicationMetadataEvidence::Id(
                        accepted.id().clone(),
                    ),
                    CheckedExpressionResolution::DialogueLineCoordinate(accepted.id().clone()),
                ),
                arcweft_lang_hir::dialogue_application::HirDialogueCoordinateKind::TextKey => (
                    crate::callable::DialogueApplicationMetadataCoordinate::TextKey,
                    TypeKind::entity_ref(EntityKind::Text),
                    crate::callable::PreparedDialogueApplicationMetadataEvidence::TextKey(
                        accepted.text_key().clone(),
                    ),
                    CheckedExpressionResolution::DialogueTextKeyCoordinate(
                        accepted.text_key().clone(),
                    ),
                ),
            };
            prepared.push(
                crate::callable::PreparedDialogueApplicationMetadataArgument::seal(
                    coordinate.argument(),
                    coordinate.value(),
                    metadata_coordinate,
                    ty.clone(),
                    evidence,
                )
                .map_err(|_| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
                })?,
            );
            self.facts
                .publish_new_expression(
                    coordinate.value(),
                    CheckedExpression::new(
                        ty,
                        CheckedTypeSelection::Inferred,
                        EffectSet::new(),
                        resolution,
                    ),
                )
                .map_err(|_| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
                })?;
        }
        crate::callable::PreparedDialogueApplicationMetadataInventory::seal(
            &projection,
            prepared.into_boxed_slice(),
        )
        .map(Some)
        .map_err(|_| AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily))
    }
}
