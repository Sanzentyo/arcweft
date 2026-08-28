//! Statement roles, effect contracts, scopes, and source evidence.

use super::{
    Analyzer, AssertionContext, BTreeMap, BTreeSet, CallableDeclarationKey, CallableEffectContract,
    CheckedAssertionDisposition, CheckedCallableExecution, CheckedCallableId, CheckedClosureId,
    CheckedEvaluatedEffect, CheckedExpression, CheckedExpressionResolution, CheckedIteration,
    CheckedIteratorFamily, CheckedStatement, CheckedStatementRole, CheckedSuspensionStatement,
    CheckedTraitConformance, CheckedTraitIdentity, CheckedTypeSelection, CheckedValueResolution,
    EffectClauseSource, EffectId, EffectItemSource, EffectRow, EffectSet, ExprId,
    FinalSemanticAnalysisError, FinalSemanticAnalysisInput, GenericParameterOwnerId,
    GenericTypeBinding, GenericTypeParameterId, GenericTypeScope, HirAssertionMode,
    HirCallableEffectSourcePart, HirCallableSourceOwner, HirCallableSourceRole, HirExprKind,
    HirExprSourceRole, HirFunctionItem, HirGenericParameter, HirImplMember, HirItem, HirItemKind,
    HirItemSourceRole, HirModule, HirName, HirPatternSourceRole, HirScopeKind, HirScopeOwner,
    HirScopeSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite, HirStmtKind, HirTypeKind,
    ItemId, ModuleSegment, PatternId, PreparedAssignmentStatement, PreparedExpressionFact,
    PreparedStatementFact, ProjectSymbolTable, ScopeId, SourceSpan, TypeId, TypeKind,
    TypeSourceEvidence,
    calls::checked_project_nominal,
    expression_types::builtin_iteration,
    items::{SourceCallableShell, checked_catalog_error},
};
use crate::callable::CheckedCallSite;
use crate::final_analysis::{CheckedDropFade, CheckedDropPolicy};

impl Analyzer<'_, '_, '_> {
    pub(super) fn analyze_statements(
        &self,
        input: &mut FinalSemanticAnalysisInput,
    ) -> Result<(), FinalSemanticAnalysisError> {
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
                )?;
                input.push_prepared_statement(owner, fact);
            }
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
    ) -> Result<PreparedStatementFact, FinalSemanticAnalysisError> {
        let role = match statement {
            HirStmtKind::Assertion { mode, conditions } => {
                self.checked_assertion_role(module, owner, scope, *mode, conditions)
            }
            HirStmtKind::For(statement) => {
                let source = self.facts.expressions().get(&statement.source()).ok_or(
                    FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: statement.source(),
                    },
                )?;
                let iteration = self
                    .facts
                    .iteration_facts()
                    .get(&statement.iterator())
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: statement.iterator(),
                    })?;
                let item = iteration_item(iteration);
                if !iteration_accepts_source(iteration, source.ty()) {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                let iterator = self.facts.expressions().get(&statement.iterator()).ok_or(
                    FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: statement.iterator(),
                    },
                )?;
                let expected_iterator = iteration_iterator(iteration);
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
                Ok(CheckedStatementRole::Iteration(Box::new(iteration.clone())))
            }
            HirStmtKind::Assign { target, value } => {
                return self
                    .prepared_assignment_statement(module, *target, *value)
                    .map(PreparedStatementFact::Assignment);
            }
            HirStmtKind::Expression { expression } => {
                self.checked_expression_statement_role(module, *expression)
            }
            HirStmtKind::Break { label, value } => {
                self.checked_break_role(module, owner, scope, label.as_ref(), *value)
            }
            HirStmtKind::Yield { .. } => Ok(CheckedStatementRole::Yield),
            HirStmtKind::UnsafeLifetime { .. } => Ok(CheckedStatementRole::UnsafeAudit),
            HirStmtKind::Wait { .. } => Ok(CheckedStatementRole::Suspension(Box::new(
                CheckedSuspensionStatement::Wait,
            ))),
            HirStmtKind::Error => Err(FinalSemanticAnalysisError::RecoveredOwner),
            _ => Ok(CheckedStatementRole::Ordinary),
        }?;
        Ok(CheckedStatement::new(statement_role_effects(&role), role).into())
    }

    fn checked_break_role(
        &self,
        module: &HirModule,
        owner: super::StmtId,
        mut scope: ScopeId,
        label: Option<&HirName>,
        value: Option<ExprId>,
    ) -> Result<CheckedStatementRole, FinalSemanticAnalysisError> {
        if label.is_some() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let Some(value) = value else {
            return Ok(CheckedStatementRole::Ordinary);
        };
        loop {
            let current = module
                .resolve_scope(scope)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            if let HirScopeOwner::Stmt(target) = current.owner() {
                let statement = module
                    .resolve_stmt(*target)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                match statement.kind() {
                    HirStmtKind::While(_) | HirStmtKind::WhileLet(_) | HirStmtKind::For(_) => {
                        return Err(FinalSemanticAnalysisError::BreakValueRequiresLoop {
                            owner,
                            value,
                            target: *target,
                            value_source: expression_span(module, value)?,
                            target_source: source_span(
                                module,
                                HirSourceQuery::Stmt {
                                    owner: *target,
                                    role: arcweft_lang_hir::source_index::HirStmtSourceRole::Whole,
                                },
                            )?,
                        });
                    }
                    _ => {}
                }
            }
            if let HirScopeOwner::Expr(target) = current.owner()
                && matches!(
                    module
                        .resolve_expr(*target)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                        .kind(),
                    HirExprKind::Loop(_)
                )
            {
                return Ok(CheckedStatementRole::Ordinary);
            }
            let Some(parent) = current.parent() else {
                return Err(FinalSemanticAnalysisError::InvalidOwner);
            };
            scope = parent;
        }
    }

    fn checked_expression_statement_role(
        &self,
        module: &HirModule,
        expression: ExprId,
    ) -> Result<CheckedStatementRole, FinalSemanticAnalysisError> {
        Ok(self
            .checked_evaluated_effect_expression(module, expression)?
            .map_or(CheckedStatementRole::Ordinary, |effect| {
                CheckedStatementRole::EvaluatedEffect(Box::new(effect))
            }))
    }

    pub(super) fn checked_evaluated_effect_expression(
        &self,
        module: &HirModule,
        expression: ExprId,
    ) -> Result<Option<CheckedEvaluatedEffect>, FinalSemanticAnalysisError> {
        let authored = module
            .resolve_expr(expression)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let (call_owner, pipeline_target) = match authored.kind() {
            HirExprKind::Call(_) => (expression, None),
            HirExprKind::Pipe(authored_pipe) => {
                let Some(CheckedExpressionResolution::Pipe(checked_pipe)) = self
                    .facts
                    .expressions()
                    .get(&expression)
                    .and_then(PreparedExpressionFact::checked_resolution)
                else {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                };
                if checked_pipe.left() != authored_pipe.left()
                    || checked_pipe.right() != authored_pipe.right()
                {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                (checked_pipe.right(), Some(checked_pipe.left()))
            }
            _ => return Ok(None),
        };
        let Some(node) = self
            .facts
            .prepared_calls()
            .map_err(FinalSemanticAnalysisError::from)?
            .selected_nodes()
            .find(|node| node.site() == CheckedCallSite::HirCall(call_owner))
        else {
            return Ok(None);
        };
        let application = node.prefix().application();
        let Some(effect) = application.selected().schema().evaluated_effect() else {
            return Ok(None);
        };
        if application
            .selected()
            .next_group_for(application.completed_group())
            .is_some()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let call_expression = module
            .resolve_expr(call_owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirExprKind::Call(call) = call_expression.kind() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let effect = match effect {
            crate::callable::CallableEvaluatedEffect::Drop(operation) => self.checked_drop_effect(
                module,
                call_owner,
                call,
                application,
                operation,
                pipeline_target,
            )?,
            _ if pipeline_target.is_some() => {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            effect => {
                let mapping = node
                    .prefix()
                    .record()
                    .input_projection()
                    .authored()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let head = effect
                    .accepts_open_fields()
                    .then(|| {
                        mapping.arguments().iter().find_map(|argument| {
                            argument.slots().iter().find_map(|slot| {
                                slot.coordinate()
                                    .filter(|coordinate| effect.operand_role(*coordinate).is_some())
                                    .map(|_| argument.source())
                            })
                        })
                    })
                    .flatten();
                let open_arguments = mapping.arguments().iter().map(|argument| {
                    let [slot] = argument.slots() else {
                        return None;
                    };
                    let source = match slot.source() {
                        crate::callable::CheckedCallArgumentSlotSource::Expression(source) => {
                            source
                        }
                        crate::callable::CheckedCallArgumentSlotSource::CompactNumericElement {
                            ..
                        } => return None,
                    };
                    Some((source, slot.open_argument()?))
                });
                CheckedEvaluatedEffect::try_from_call(
                    effect,
                    call.arguments(),
                    application.selected().schema().semantic_digest(),
                    head,
                    open_arguments,
                )
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?
            }
        };
        Ok(Some(effect))
    }

    fn checked_drop_effect(
        &self,
        module: &HirModule,
        owner: ExprId,
        call: &arcweft_lang_hir::expr::HirCallExpr,
        application: &crate::callable::PreparedCallableApplication,
        operation: crate::callable::DropCallableId,
        pipeline_target: Option<ExprId>,
    ) -> Result<CheckedEvaluatedEffect, FinalSemanticAnalysisError> {
        if operation == crate::callable::DropCallableId::OnDrop {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let selected = application.selected();
        let receiver = match (selected.instantiation(), call.callee(), pipeline_target) {
            (
                crate::callable::CallableInstantiation::Receiver { .. },
                arcweft_lang_hir::expr::HirCallCallee::UnresolvedDot { value_receiver, .. },
                None,
            ) => Some(*value_receiver),
            (
                crate::callable::CallableInstantiation::Extension {
                    group, parameter, ..
                },
                arcweft_lang_hir::expr::HirCallCallee::UnresolvedDot { value_receiver, .. },
                None,
            ) if selected
                .schema()
                .extension_receiver()
                .is_some_and(|receiver| {
                    receiver.group() == *group
                        && receiver.parameter() == *parameter
                        && application
                            .completed_group()
                            .get()
                            .checked_add(1)
                            .is_some_and(|next| next == group.get())
                }) =>
            {
                Some(*value_receiver)
            }
            (
                crate::callable::CallableInstantiation::Extension {
                    group, parameter, ..
                },
                _,
                Some(target),
            ) if selected
                .schema()
                .extension_receiver()
                .is_some_and(|receiver| {
                    receiver.group() == *group
                        && receiver.parameter() == *parameter
                        && application
                            .completed_group()
                            .get()
                            .checked_add(1)
                            .is_some_and(|next| next == group.get())
                }) =>
            {
                Some(target)
            }
            (
                crate::callable::CallableInstantiation::Receiver { .. }
                | crate::callable::CallableInstantiation::Extension { .. },
                _,
                _,
            ) => {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            (_, _, Some(_)) => return Err(FinalSemanticAnalysisError::WrongPayloadFamily),
            (_, _, None) => None,
        };
        let target = receiver
            .or_else(|| single_call_argument(call.arguments()))
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let (policy_source, policy) = match operation {
            crate::callable::DropCallableId::Drop
            | crate::callable::DropCallableId::DropOptional => (None, CheckedDropPolicy::Default),
            crate::callable::DropCallableId::DropWithPolicy => {
                let source = if receiver.is_some() {
                    single_call_argument(call.arguments())
                } else {
                    let reference = selected
                        .prepared_continuation()
                        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                    let prefix = self
                        .facts
                        .prepared_calls()
                        .map_err(FinalSemanticAnalysisError::from)?
                        .continuation_prefix(reference)
                        .map_err(|failure| {
                            FinalSemanticAnalysisError::CallSeal(
                                crate::final_analysis::FinalCallSealFailure::new(
                                    crate::final_analysis::FinalCallSealLocation::Site(
                                        CheckedCallSite::HirCall(owner),
                                    ),
                                    failure,
                                ),
                            )
                        })?;
                    let CheckedCallSite::HirCall(owner) = prefix.site() else {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                    };
                    let expression = module
                        .resolve_expr(owner)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                    let HirExprKind::Call(prefix_call) = expression.kind() else {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                    };
                    single_call_argument(prefix_call.arguments())
                }
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                (Some(source), self.checked_drop_policy(module, source)?)
            }
            crate::callable::DropCallableId::OnDrop => {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        };
        Ok(CheckedEvaluatedEffect::Drop {
            operation,
            target,
            policy_source,
            policy,
        })
    }

    fn checked_drop_policy(
        &self,
        module: &HirModule,
        expression: ExprId,
    ) -> Result<CheckedDropPolicy, FinalSemanticAnalysisError> {
        let checked =
            self.facts.expressions().get(&expression).ok_or(
                FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: expression },
            )?;
        match checked.checked_resolution() {
            Some(CheckedExpressionResolution::Value(CheckedValueResolution::Registered(value))) => {
                let binding = value
                    .environment_binding()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                match self
                    .catalogs
                    .world
                    .environment()
                    .typecheck_env()
                    .standard_environment_value(binding)
                {
                    Some(crate::env::StandardEnvironmentValue::DropPolicy(
                        crate::env::StandardDropPolicyValue::Stop { fade_nanos },
                    )) => Ok(CheckedDropPolicy::Stop {
                        fade: CheckedDropFade::ConstantNanos(fade_nanos),
                    }),
                    None => Err(FinalSemanticAnalysisError::WrongPayloadFamily),
                }
            }
            Some(CheckedExpressionResolution::Variant(variant)) => {
                let super::CheckedVariantOwner::BuiltinClosed { nominal, .. } = variant.owner()
                else {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                };
                let policy = self
                    .catalogs
                    .world
                    .environment()
                    .typecheck_env()
                    .standard_drop_policy_case(nominal, variant.ordinal())
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                Ok(match policy {
                    crate::env::StandardDropPolicyCase::Cancel => CheckedDropPolicy::Cancel,
                    crate::env::StandardDropPolicyCase::Stop => {
                        let source = module
                            .resolve_expr(expression)
                            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                        let HirExprKind::Call(call) = source.kind() else {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                        };
                        let fade = single_call_argument(call.arguments())
                            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                        CheckedDropPolicy::Stop {
                            fade: CheckedDropFade::Expression(fade),
                        }
                    }
                    crate::env::StandardDropPolicyCase::Finish => CheckedDropPolicy::Finish,
                    crate::env::StandardDropPolicyCase::Release => CheckedDropPolicy::Release,
                    crate::env::StandardDropPolicyCase::Detach => CheckedDropPolicy::Detach,
                })
            }
            _ => Err(FinalSemanticAnalysisError::WrongPayloadFamily),
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
            EffectSet::new(),
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
    ) -> Result<CheckedStatementRole, FinalSemanticAnalysisError> {
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
        Ok(CheckedStatementRole::Assertion(disposition))
    }
}

fn single_call_argument(arguments: &[arcweft_lang_hir::expr::HirCallArgument]) -> Option<ExprId> {
    let [argument] = arguments else {
        return None;
    };
    (!matches!(
        argument,
        arcweft_lang_hir::expr::HirCallArgument::Spread { .. }
    ))
    .then(|| argument.value())
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

fn statement_role_effects(role: &CheckedStatementRole) -> EffectSet {
    let mut effects = EffectSet::new();
    if matches!(
        role,
        CheckedStatementRole::Suspension(_) | CheckedStatementRole::Yield
    ) {
        effects.insert(
            EffectId::parse("control.suspend")
                .expect("the language-owned suspension effect is a valid effect identity"),
        );
    }
    effects
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

pub(super) fn closure_effect_rows(
    module: &HirModule,
    body_scope: ScopeId,
    owner: &CheckedCallableId,
    input: &FinalSemanticAnalysisInput,
) -> Result<Vec<(CheckedClosureId, ScopeId, EffectRow)>, FinalSemanticAnalysisError> {
    let mut rows = Vec::new();
    for (expression_id, expression) in module.expressions() {
        let HirExprKind::Closure(closure) = expression.kind() else {
            continue;
        };
        if !scope_is_within(module, expression.scope(), body_scope)? {
            continue;
        }
        let id = CheckedClosureId::from_checked_expression(
            owner.clone(),
            expression_span(module, expression_id)?,
        )
        .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        rows.push((
            id,
            closure.scope(),
            EffectRow::closed(execution_effects(module, closure.scope(), input)?),
        ));
    }
    Ok(rows)
}

/// Collects effects performed by one execution body. A nested closure body is
/// a latent callable row, not an eager effect of creating the closure value.
/// The closure literal expression itself remains in the enclosing scope and is
/// therefore still observed if value creation ever gains an intrinsic effect.
pub(super) fn execution_effects(
    module: &HirModule,
    body_scope: ScopeId,
    input: &FinalSemanticAnalysisInput,
) -> Result<EffectSet, FinalSemanticAnalysisError> {
    let mut effects = EffectSet::new();
    for (owner, checked) in &input.expressions {
        if owner.module() == module.module_id()
            && scope_executes_within(
                module,
                module
                    .resolve_expr(*owner)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                    .scope(),
                body_scope,
            )?
        {
            effects.union_with(checked.effects());
        }
    }
    for (owner, checked) in &input.statements {
        if owner.module() == module.module_id()
            && scope_executes_within(
                module,
                module
                    .resolve_stmt(*owner)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                    .scope(),
                body_scope,
            )?
        {
            effects.union_with(checked.effects());
        }
    }
    Ok(effects)
}

pub(super) fn scope_executes_within(
    module: &HirModule,
    mut candidate: ScopeId,
    root: ScopeId,
) -> Result<bool, FinalSemanticAnalysisError> {
    loop {
        if candidate == root {
            return Ok(true);
        }
        let scope = module
            .resolve_scope(candidate)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if scope.kind() == HirScopeKind::Closure
            || scope_starts_independent_execution(module, scope.owner())?
        {
            return Ok(false);
        }
        let Some(parent) = scope.parent() else {
            return Ok(false);
        };
        candidate = parent;
    }
}

fn scope_starts_independent_execution(
    module: &HirModule,
    owner: &HirScopeOwner,
) -> Result<bool, FinalSemanticAnalysisError> {
    match owner {
        HirScopeOwner::Expr(owner) => {
            let expression = module
                .resolve_expr(*owner)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            Ok(matches!(
                expression.kind(),
                HirExprKind::Closure(_)
                    | HirExprKind::ComputationBlock(_)
                    | HirExprKind::Thread(_)
                    | HirExprKind::Choice(_)
                    | HirExprKind::DialogueContentApplication(_)
                    | HirExprKind::PostfixBracket(_)
            ))
        }
        HirScopeOwner::Stmt(owner) => {
            let statement = module
                .resolve_stmt(*owner)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            Ok(matches!(statement.kind(), HirStmtKind::On { .. }))
        }
        HirScopeOwner::Module(_) | HirScopeOwner::Item(_) => Ok(false),
    }
}

pub(super) fn scope_is_within(
    module: &HirModule,
    mut candidate: ScopeId,
    root: ScopeId,
) -> Result<bool, FinalSemanticAnalysisError> {
    loop {
        if candidate == root {
            return Ok(true);
        }
        let scope = module
            .resolve_scope(candidate)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let Some(parent) = scope.parent() else {
            return Ok(false);
        };
        candidate = parent;
    }
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
