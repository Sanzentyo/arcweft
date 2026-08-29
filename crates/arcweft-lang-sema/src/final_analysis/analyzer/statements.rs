//! Statement roles, effect contracts, scopes, and source evidence.

use super::{
    Analyzer, AssertionContext, BTreeMap, BTreeSet, CallableDeclarationKey, CallableEffectContract,
    CheckedAssertionDisposition, CheckedCallableExecution, CheckedExpression,
    CheckedExpressionResolution, CheckedIteration, CheckedIteratorFamily,
    CheckedSuspensionStatement, CheckedTraitConformance, CheckedTraitIdentity,
    CheckedTypeSelection, CheckedValueResolution, EffectClauseSource, EffectId, EffectItemSource,
    EffectRow, EffectSet, ExprId, FinalSemanticAnalysisError, FinalSemanticAnalysisInput,
    GenericParameterOwnerId, GenericTypeBinding, GenericTypeParameterId, GenericTypeScope,
    HirAssertionMode, HirCallableEffectSourcePart, HirCallableSourceOwner, HirCallableSourceRole,
    HirExprKind, HirExprSourceRole, HirFunctionItem, HirGenericParameter, HirImplMember, HirItem,
    HirItemKind, HirItemSourceRole, HirModule, HirName, HirPatternSourceRole, HirScopeKind,
    HirScopeOwner, HirScopeSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite,
    HirStmtKind, HirTypeKind, ItemId, ModuleSegment, PatternId, PreparedAssignmentStatement,
    PreparedExpressionFact, PreparedStatementPayload, ProjectSymbolTable, ScopeId, SourceSpan,
    TypeId, TypeKind, TypeSourceEvidence,
    calls::checked_project_nominal,
    expression_types::builtin_iteration,
    items::{SourceCallableShell, checked_catalog_error},
};
impl Analyzer<'_, '_, '_> {
    pub(super) fn analyze_statements(
        &mut self,
        input: &mut FinalSemanticAnalysisInput,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let mut iteration_facts = self
            .facts
            .take_iteration_facts_for_statements()
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        for module in self.modules.values() {
            for (owner, statement) in module.statements() {
                if statement.is_poisoned() {
                    return Err(FinalSemanticAnalysisError::RecoveredOwner);
                }
                let fact = self.checked_statement_fact(
                    module,
                    owner,
                    statement.scope(),
                    statement.kind(),
                    &mut iteration_facts,
                )?;
                input.push_prepared_statement(owner, fact);
            }
        }
        if !iteration_facts.is_empty() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(())
    }

    pub(super) fn select_iteration(
        &self,
        source: &TypeKind,
    ) -> Result<CheckedIteration, FinalSemanticAnalysisError> {
        if let Some((family, _, item)) = builtin_iteration(source) {
            return Ok(CheckedIteration::Builtin { family, item });
        }

        if let Some(into_iterator) = self.standard_iterator_impl(
            "IntoIterator",
            source,
            "into_iter",
            &CheckedTraitIdentity::StandardIntoIterator,
        )? {
            let item = into_iterator
                .associated
                .get("Item")
                .cloned()
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            let into_iter = into_iterator
                .associated
                .get("IntoIter")
                .cloned()
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            let iterator = self
                .standard_iterator_impl(
                    "Iterator",
                    &into_iter,
                    "next",
                    &CheckedTraitIdentity::StandardIterator,
                )?
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            if iterator.associated.get("Item") != Some(&item) {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            return Ok(CheckedIteration::Witness {
                source: source.clone(),
                item,
                into_iter,
                into_iterator: into_iterator.conformance,
                iterator: iterator.conformance,
            });
        }

        let iterator = self
            .standard_iterator_impl(
                "Iterator",
                source,
                "next",
                &CheckedTraitIdentity::StandardIterator,
            )?
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let item = iterator
            .associated
            .get("Item")
            .cloned()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        Ok(CheckedIteration::IteratorWitness {
            source: source.clone(),
            item,
            iterator: iterator.conformance,
        })
    }

    fn standard_iterator_impl(
        &self,
        trait_name: &str,
        target: &TypeKind,
        method_name: &str,
        trait_identity: &CheckedTraitIdentity,
    ) -> Result<Option<SelectedStandardIteratorImpl>, FinalSemanticAnalysisError> {
        let mut selected = None;
        for module in self.modules.values() {
            for (owner, item) in module.items() {
                let HirItemKind::Impl(implementation) = item.kind() else {
                    continue;
                };
                let Some(trait_ref) = implementation.trait_ref() else {
                    continue;
                };
                let trait_ref = module
                    .resolve_type(trait_ref)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                let HirTypeKind::Path(path) = trait_ref.kind() else {
                    continue;
                };
                if path.segments().last().and_then(|segment| match segment {
                    arcweft_lang_hir::leaf::HirPathSegment::Identifier(name) => Some(name.as_str()),
                    arcweft_lang_hir::leaf::HirPathSegment::ProjectSymbol(_) => None,
                }) != Some(trait_name)
                    || self.types.get(&implementation.target()) != Some(target)
                {
                    continue;
                }

                let mut associated = BTreeMap::new();
                let mut method = None;
                for (ordinal, member) in implementation.members().iter().enumerate() {
                    match member {
                        HirImplMember::AssociatedType(assignment) => {
                            let Some(name) = assignment.name().resolved() else {
                                return Err(FinalSemanticAnalysisError::RecoveredOwner);
                            };
                            let value = self.types.get(&assignment.target()).cloned().ok_or(
                                FinalSemanticAnalysisError::TypeResolutionFailed {
                                    owner: assignment.target(),
                                },
                            )?;
                            if associated.insert(name.as_str().to_owned(), value).is_some() {
                                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                            }
                        }
                        HirImplMember::Function(function)
                            if function.name().resolved().map(HirName::as_str)
                                == Some(method_name) =>
                        {
                            method = Some(
                                u16::try_from(ordinal)
                                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                            );
                        }
                        HirImplMember::Function(_) => {}
                        HirImplMember::Error => {
                            return Err(FinalSemanticAnalysisError::RecoveredOwner);
                        }
                    }
                }
                let method = method.ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let declaration = self
                    .symbols
                    .callable_symbols()
                    .find_map(|symbol| {
                        if symbol.source_item() != owner
                            || symbol.source_owner()
                                != (HirCallableSourceOwner::ImplFunction { member: method })
                        {
                            return None;
                        }
                        let CallableDeclarationKey::ImplMethod(declaration) = symbol.declaration()
                        else {
                            return None;
                        };
                        Some(declaration.clone())
                    })
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let candidate = SelectedStandardIteratorImpl {
                    conformance: CheckedTraitConformance::new(
                        owner,
                        trait_identity.clone(),
                        method,
                        declaration,
                    ),
                    associated,
                };
                if selected.replace(candidate).is_some() {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
            }
        }
        Ok(selected)
    }

    fn checked_statement_fact(
        &self,
        module: &HirModule,
        owner: super::StmtId,
        scope: ScopeId,
        statement: &HirStmtKind,
        iteration_facts: &mut BTreeMap<ExprId, CheckedIteration>,
    ) -> Result<PreparedStatementPayload, FinalSemanticAnalysisError> {
        let prepared = match statement {
            HirStmtKind::Assertion { mode, conditions } => self
                .checked_assertion_role(module, owner, scope, *mode, conditions)
                .map(PreparedStatementPayload::Assertion),
            HirStmtKind::For(statement) => {
                let source = self.facts.expressions().get(&statement.source()).ok_or(
                    FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: statement.source(),
                    },
                )?;
                let iteration = iteration_facts.remove(&statement.iterator()).ok_or(
                    FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: statement.iterator(),
                    },
                )?;
                let item = iteration_item(&iteration);
                if !iteration_accepts_source(&iteration, source.ty()) {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                let iterator = self.facts.expressions().get(&statement.iterator()).ok_or(
                    FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: statement.iterator(),
                    },
                )?;
                let expected_iterator = iteration_iterator(&iteration);
                let next_value = self
                    .facts
                    .expressions()
                    .get(&statement.next_value())
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: statement.next_value(),
                    })?;
                if iterator.ty() != &expected_iterator || next_value.ty() != item {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                Ok(PreparedStatementPayload::Iteration(Box::new(iteration)))
            }
            HirStmtKind::Assign { target, value } => {
                return self
                    .prepared_assignment_statement(module, *target, *value)
                    .map(PreparedStatementPayload::Assignment);
            }
            HirStmtKind::Expression { expression } => {
                if let Some(effect) =
                    self.prepare_evaluated_effect_expression(module, *expression)?
                {
                    return Ok(PreparedStatementPayload::EvaluatedEffect(effect));
                }
                Ok(PreparedStatementPayload::HirOwned)
            }
            HirStmtKind::Break { value, .. } => {
                self.validate_break_statement(module, owner, *value)?;
                Ok(PreparedStatementPayload::HirOwned)
            }
            HirStmtKind::Yield { .. } => Ok(PreparedStatementPayload::Yield),
            HirStmtKind::Wait { target } => {
                let target_fact = self.facts.expressions().get(target).ok_or(
                    FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: *target },
                )?;
                if target_fact.ty() != &TypeKind::Duration {
                    return Err(FinalSemanticAnalysisError::StatementOperandTypeMismatch {
                        statement: owner,
                        owner: *target,
                        expected: Box::new(TypeKind::Duration),
                        actual: Box::new(target_fact.ty().clone()),
                    });
                }
                Ok(PreparedStatementPayload::Suspension(Box::new(
                    CheckedSuspensionStatement::Wait,
                )))
            }
            HirStmtKind::Let { .. }
            | HirStmtKind::LetElse { .. }
            | HirStmtKind::Return { .. }
            | HirStmtKind::Out { .. }
            | HirStmtKind::Goto { .. }
            | HirStmtKind::Defer { .. }
            | HirStmtKind::Signal { .. }
            | HirStmtKind::LifetimeSet { .. }
            | HirStmtKind::On { .. }
            | HirStmtKind::UnsafeLifetime { .. }
            | HirStmtKind::Choice { .. }
            | HirStmtKind::If(_)
            | HirStmtKind::IfLet(_)
            | HirStmtKind::Match(_)
            | HirStmtKind::While(_)
            | HirStmtKind::WhileLet(_)
            | HirStmtKind::Close { .. }
            | HirStmtKind::Select(_)
            | HirStmtKind::SourceLocale(_)
            | HirStmtKind::Scope(_)
            | HirStmtKind::Include(_)
            | HirStmtKind::Continue { .. }
            | HirStmtKind::ProofCall { .. } => Ok(PreparedStatementPayload::HirOwned),
            HirStmtKind::Error => Err(FinalSemanticAnalysisError::RecoveredOwner),
        }?;
        Ok(prepared)
    }

    fn validate_break_statement(
        &self,
        module: &HirModule,
        owner: super::StmtId,
        value: Option<ExprId>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let target = self
            .topology
            .control_transfer(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if target.kind() != arcweft_lang_hir::project::HirControlTransferKind::Break {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let Some(value) = value else {
            return Ok(());
        };
        match target.target().loop_family() {
            Some(arcweft_lang_hir::project::HirLoopTargetFamily::LoopExpression) => Ok(()),
            Some(
                arcweft_lang_hir::project::HirLoopTargetFamily::WhileStatement
                | arcweft_lang_hir::project::HirLoopTargetFamily::WhileLetStatement
                | arcweft_lang_hir::project::HirLoopTargetFamily::ForStatement,
            ) => {
                let target = target
                    .target()
                    .loop_body_owner()
                    .and_then(arcweft_lang_hir::project::HirSemanticBodyOwner::statement_owner)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                Err(FinalSemanticAnalysisError::BreakValueRequiresLoop {
                    owner,
                    value,
                    target,
                    value_source: expression_span(module, value)?,
                    target_source: source_span(
                        module,
                        HirSourceQuery::Stmt {
                            owner: target,
                            role: arcweft_lang_hir::source_index::HirStmtSourceRole::Whole,
                        },
                    )?,
                })
            }
            None => Err(FinalSemanticAnalysisError::WrongPayloadFamily),
        }
    }

    fn prepared_assignment_statement(
        &self,
        module: &HirModule,
        target: ExprId,
        value: ExprId,
    ) -> Result<PreparedAssignmentStatement, FinalSemanticAnalysisError> {
        let target_expression = module
            .resolve_expr(target)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirExprKind::Select(select) = target_expression.kind() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let base_expression = module
            .resolve_expr(select.target())
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if !matches!(base_expression.kind(), HirExprKind::Path(_)) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let base = self.facts.expressions().get(&select.target()).ok_or(
            FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                owner: select.target(),
            },
        )?;
        let Some(CheckedExpressionResolution::Value(CheckedValueResolution::Local(local))) =
            base.checked_resolution()
        else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let target_fact = self
            .facts
            .expressions()
            .get(&target)
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: target })?;
        let PreparedExpressionFact::ProjectField(prepared_field) = target_fact else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        if base.ty()
            != self
                .facts
                .locals()
                .get(local)
                .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: *local })?
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let TypeKind::ProjectNominal(base_nominal) = base.ty() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let declaration = self
            .symbols
            .nominal(base_nominal.declaration())
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let nominal = checked_project_nominal(declaration, base.ty())?;
        if prepared_field.nominal() != &nominal || prepared_field.field_type() != target_fact.ty() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let value_fact = self
            .facts
            .expressions()
            .get(&value)
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: value })?;
        if target_fact.ty() != value_fact.ty() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(PreparedAssignmentStatement::new(
            *local,
            nominal,
            target,
            value,
            target_fact.ty().clone(),
        ))
    }

    fn checked_assertion_role(
        &self,
        module: &HirModule,
        owner: super::StmtId,
        scope: ScopeId,
        mode: HirAssertionMode,
        conditions: &[ExprId],
    ) -> Result<CheckedAssertionDisposition, FinalSemanticAnalysisError> {
        let mode = match mode {
            HirAssertionMode::Resolved(mode) => mode,
            HirAssertionMode::Recovered => {
                return Err(FinalSemanticAnalysisError::RecoveredOwner);
            }
        };
        let context = assertion_context(module, scope)?;
        if !context.allows(mode) {
            return Err(FinalSemanticAnalysisError::AssertionModeNotAllowed {
                owner,
                mode,
                context,
            });
        }
        for (index, condition) in conditions.iter().copied().enumerate() {
            let checked = self.facts.expressions().get(&condition).ok_or(
                FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: condition },
            )?;
            if checked.ty() != &TypeKind::Bool {
                return Err(FinalSemanticAnalysisError::AssertionConditionNotBool {
                    owner,
                    condition,
                    index,
                    actual: Box::new(checked.ty().clone()),
                });
            }
            if !checked.effects().is_empty() {
                return Err(FinalSemanticAnalysisError::AssertionConditionNotPure {
                    owner,
                    condition,
                    index,
                    effects: checked.effects().clone(),
                });
            }
        }
        let disposition = match mode {
            arcweft_lang_syntax::assertion::AssertionMode::Prove => {
                CheckedAssertionDisposition::PendingProof
            }
            arcweft_lang_syntax::assertion::AssertionMode::Check => {
                CheckedAssertionDisposition::Runtime(
                    crate::assertion::AssertionRuntimePolicy::AlwaysGuard,
                )
            }
            arcweft_lang_syntax::assertion::AssertionMode::Debug
                if self
                    .control
                    .assertion_build_profile()
                    .retains_debug_assertions() =>
            {
                CheckedAssertionDisposition::Runtime(
                    crate::assertion::AssertionRuntimePolicy::DebugGuard,
                )
            }
            arcweft_lang_syntax::assertion::AssertionMode::Debug => {
                CheckedAssertionDisposition::OmittedDebug
            }
        };
        Ok(disposition)
    }
}

struct SelectedStandardIteratorImpl {
    conformance: CheckedTraitConformance,
    associated: BTreeMap<String, TypeKind>,
}

pub(super) fn iteration_item(iteration: &CheckedIteration) -> &TypeKind {
    match iteration {
        CheckedIteration::Builtin { item, .. }
        | CheckedIteration::Witness { item, .. }
        | CheckedIteration::IteratorWitness { item, .. } => item,
    }
}

pub(super) fn iteration_iterator(iteration: &CheckedIteration) -> TypeKind {
    match iteration {
        CheckedIteration::Builtin { family, item } => TypeKind::IteratorState {
            family: match family {
                CheckedIteratorFamily::Range => crate::types::IteratorStateKind::Range,
                CheckedIteratorFamily::Seq => crate::types::IteratorStateKind::Seq,
                CheckedIteratorFamily::Stream => crate::types::IteratorStateKind::Stream,
                CheckedIteratorFamily::Vec => crate::types::IteratorStateKind::Vec,
                CheckedIteratorFamily::Array => crate::types::IteratorStateKind::Array,
                CheckedIteratorFamily::Slice => crate::types::IteratorStateKind::Slice,
            },
            item: Box::new(item.clone()),
        },
        CheckedIteration::Witness { into_iter, .. } => into_iter.clone(),
        CheckedIteration::IteratorWitness { source, .. } => source.clone(),
    }
}

fn iteration_accepts_source(iteration: &CheckedIteration, source: &TypeKind) -> bool {
    match iteration {
        CheckedIteration::Builtin { family, item } => {
            builtin_iteration(source).is_some_and(|(actual_family, _, actual_item)| {
                actual_family == *family && actual_item == *item
            })
        }
        CheckedIteration::Witness {
            source: expected, ..
        }
        | CheckedIteration::IteratorWitness {
            source: expected, ..
        } => expected == source,
    }
}

pub(super) fn function_effect_contract(
    module: &HirModule,
    owner: ItemId,
    function: &HirFunctionItem,
    scope: ScopeId,
    execution: CheckedCallableExecution,
) -> Result<SourceCallableShell, FinalSemanticAnalysisError> {
    if function.effect_clauses().is_empty() {
        let contract = CallableEffectContract::body_inference(
            scope_span(module, scope)?,
            EffectSet::new(),
            Box::new([]),
        )
        .map_err(checked_catalog_error)?;
        return Ok(SourceCallableShell::Body {
            scope,
            execution,
            contract: Box::new(contract),
        });
    }

    let mut permitted = EffectSet::new();
    let mut clause_sources = Vec::with_capacity(function.effect_clauses().len());
    for (position, clause) in function.effect_clauses().iter().enumerate() {
        let clause_ordinal = u16::try_from(position)
            .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let role = |part| HirSourceQuery::Item {
            owner,
            role: HirItemSourceRole::Callable(HirCallableSourceRole::EffectClause {
                owner: HirCallableSourceOwner::Item,
                clause: clause_ordinal,
                part,
            }),
        };
        let whole = source_span(module, role(HirCallableEffectSourcePart::Whole))?;
        let keyword = source_span(module, role(HirCallableEffectSourcePart::Keyword))?;
        let mut item_sources = Vec::with_capacity(clause.operands().len());
        for &expression in clause.operands() {
            let effect = checked_effect_identity(module, expression)?;
            permitted.insert(effect.clone());
            item_sources.push(EffectItemSource::new(
                effect.clone(),
                expression_span(module, expression)?,
            ));
        }
        clause_sources.push(
            EffectClauseSource::try_new(whole, keyword, item_sources.into_boxed_slice())
                .map_err(checked_catalog_error)?,
        );
    }
    let contract = CallableEffectContract::authored(
        EffectRow::closed(permitted),
        clause_sources.into_boxed_slice(),
        None,
        EffectSet::new(),
        Box::new([]),
    )
    .map_err(checked_catalog_error)?;
    Ok(SourceCallableShell::Body {
        scope,
        execution,
        contract: Box::new(contract),
    })
}

fn checked_effect_identity(
    module: &HirModule,
    owner: ExprId,
) -> Result<EffectId, FinalSemanticAnalysisError> {
    EffectId::try_from_hir_expression(module, owner)
        .map(|(effect, _)| effect)
        .map_err(checked_catalog_error)
}

pub(super) fn checked_effect_expression(
    module: &HirModule,
    owner: ExprId,
) -> Result<(EffectId, Vec<(ExprId, CheckedExpression)>), FinalSemanticAnalysisError> {
    let (effect, owners) =
        EffectId::try_from_hir_expression(module, owner).map_err(checked_catalog_error)?;
    let facts = owners
        .into_iter()
        .map(|owner| {
            (
                owner,
                CheckedExpression::new(
                    TypeKind::Named("EffectCapability".to_owned()),
                    CheckedTypeSelection::Explicit,
                    EffectSet::new(),
                    CheckedExpressionResolution::Effect(effect.clone()),
                ),
            )
        })
        .collect();
    Ok((effect, facts))
}

pub(super) fn generic_scope(
    module: &HirModule,
    scope: ScopeId,
    symbols: &ProjectSymbolTable,
    owner: TypeId,
) -> Result<GenericTypeScope, FinalSemanticAnalysisError> {
    let item_id =
        enclosing_item(module, scope)?.or(nominal_declaration_item_for_type(module, owner)?);
    let Some(item_id) = item_id else {
        return Ok(GenericTypeScope::empty());
    };
    let item = module
        .resolve_item(item_id)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
    let parameters = item_generic_parameters(item);
    if parameters.is_empty() {
        return Ok(GenericTypeScope::empty());
    }
    let source = scope_span(module, scope)?;
    let generic_owner = symbols
        .callable_symbols()
        .find(|callable| callable.source_item() == item_id)
        .map(|callable| GenericParameterOwnerId::Callable(callable.declaration().clone()))
        .or_else(|| {
            symbols
                .nominal_symbols()
                .find(|nominal| nominal.owner() == item_id)
                .map(|nominal| GenericParameterOwnerId::Nominal(nominal.id().clone()))
        })
        .unwrap_or_else(|| GenericParameterOwnerId::AcceptedSource(source.clone()));
    let mut ordinal = 0_u16;
    let mut bindings = Vec::new();
    for parameter in parameters {
        let HirGenericParameter::Type { name, .. } = parameter else {
            continue;
        };
        let name = name
            .resolved()
            .ok_or(FinalSemanticAnalysisError::GenericScope { owner })?;
        let segment = ModuleSegment::new(name.as_str())
            .map_err(|_| FinalSemanticAnalysisError::GenericScope { owner })?;
        let id = GenericTypeParameterId::new(generic_owner.clone(), ordinal);
        ordinal = ordinal
            .checked_add(1)
            .ok_or(FinalSemanticAnalysisError::GenericScope { owner })?;
        bindings.push(GenericTypeBinding::new(
            id,
            segment,
            TypeSourceEvidence::accepted(source.range(), source.clone()),
        ));
    }
    GenericTypeScope::try_new(bindings)
        .map_err(|_| FinalSemanticAnalysisError::GenericScope { owner })
}

fn nominal_declaration_item_for_type(
    module: &HirModule,
    owner: TypeId,
) -> Result<Option<ItemId>, FinalSemanticAnalysisError> {
    let mut matched = None;
    for (item_id, item) in module.items() {
        let mut contains = false;
        for root in nominal_item_type_roots(item) {
            if type_graph_contains(module, root, owner)? {
                contains = true;
                break;
            }
        }
        if !contains {
            continue;
        }
        if matched.replace(item_id).is_some() {
            return Err(FinalSemanticAnalysisError::GenericScope { owner });
        }
    }
    Ok(matched)
}

fn nominal_item_type_roots(item: &HirItem) -> Vec<TypeId> {
    let mut roots = Vec::new();
    match item.kind() {
        HirItemKind::TypeAlias(alias) => roots.push(alias.target()),
        HirItemKind::Struct(item) => roots.extend(item.fields().iter().map(|field| field.ty())),
        HirItemKind::Enum(item) => roots.extend(
            item.variants()
                .iter()
                .filter_map(|variant| variant.payload()),
        ),
        _ => return roots,
    }
    for parameter in item_generic_parameters(item) {
        roots.extend_from_slice(parameter.bounds());
    }
    let predicates = match item.kind() {
        HirItemKind::TypeAlias(item) => item.where_predicates(),
        HirItemKind::Struct(item) => item.where_predicates(),
        HirItemKind::Enum(item) => item.where_predicates(),
        _ => &[],
    };
    for predicate in predicates {
        roots.push(predicate.subject());
        roots.extend_from_slice(predicate.bounds());
    }
    roots
}

fn type_graph_contains(
    module: &HirModule,
    root: TypeId,
    sought: TypeId,
) -> Result<bool, FinalSemanticAnalysisError> {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(owner) = pending.pop() {
        if owner == sought {
            return Ok(true);
        }
        if !visited.insert(owner) {
            continue;
        }
        let ty = module
            .resolve_type(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        pending.extend(ty.kind().direct_type_children());
    }
    Ok(false)
}

fn item_generic_parameters(item: &HirItem) -> &[HirGenericParameter] {
    match item.kind() {
        HirItemKind::Flow(item) => item.generic_parameters(),
        HirItemKind::Function(item) => item.generic_parameters(),
        HirItemKind::Predicate(item) => item.generic_parameters(),
        HirItemKind::Proof(item) => item.generic_parameters(),
        HirItemKind::Trait(item) => item.generic_parameters(),
        HirItemKind::Impl(item) => item.generic_parameters(),
        HirItemKind::Enum(item) => item.generic_parameters(),
        HirItemKind::Struct(item) => item.generic_parameters(),
        HirItemKind::TypeAlias(item) => item.generic_parameters(),
        _ => &[],
    }
}

pub(super) fn enclosing_item(
    module: &HirModule,
    mut scope: ScopeId,
) -> Result<Option<ItemId>, FinalSemanticAnalysisError> {
    loop {
        let node = module
            .resolve_scope(scope)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if let HirScopeOwner::Item(owner) = node.owner() {
            return Ok(Some(*owner));
        }
        let Some(parent) = node.parent() else {
            return Ok(None);
        };
        scope = parent;
    }
}

fn assertion_context(
    module: &HirModule,
    mut scope: ScopeId,
) -> Result<AssertionContext, FinalSemanticAnalysisError> {
    loop {
        let node = module
            .resolve_scope(scope)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        match node.kind() {
            HirScopeKind::Proof => return Ok(AssertionContext::ProofBody),
            HirScopeKind::Predicate => return Ok(AssertionContext::PredicateBody),
            HirScopeKind::ContractRequires | HirScopeKind::ContractEnsures => {
                return Ok(AssertionContext::ConstOrType);
            }
            HirScopeKind::Module | HirScopeKind::Callable | HirScopeKind::Flow => {
                return Ok(AssertionContext::OrdinaryBody);
            }
            HirScopeKind::Block
            | HirScopeKind::MatchArm
            | HirScopeKind::Conditional
            | HirScopeKind::Closure => {}
        }
        let Some(parent) = node.parent() else {
            return Err(FinalSemanticAnalysisError::InvalidOwner);
        };
        scope = parent;
    }
}

pub(super) fn scope_span(
    module: &HirModule,
    owner: ScopeId,
) -> Result<SourceSpan, FinalSemanticAnalysisError> {
    source_span(
        module,
        HirSourceQuery::Scope {
            owner,
            role: HirScopeSourceRole::Whole,
        },
    )
}

pub(super) fn expression_span(
    module: &HirModule,
    owner: ExprId,
) -> Result<SourceSpan, FinalSemanticAnalysisError> {
    source_span(
        module,
        HirSourceQuery::Expr {
            owner,
            role: HirExprSourceRole::Whole,
        },
    )
}

pub(super) fn pattern_span(
    module: &HirModule,
    owner: PatternId,
) -> Result<SourceSpan, FinalSemanticAnalysisError> {
    source_span(
        module,
        HirSourceQuery::Pattern {
            owner,
            role: HirPatternSourceRole::Whole,
        },
    )
}

pub(super) fn source_span(
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
