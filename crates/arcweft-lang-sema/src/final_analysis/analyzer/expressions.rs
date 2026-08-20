//! Expression-family checking outside ordinary-call resolution.

#[path = "expressions/records.rs"]
mod records;

use super::{
    Analyzer, ArrayLength, BTreeSet, BorrowKind, CandidateSemanticProjection, CheckedAwait,
    CheckedAwaitPendingObserver, CheckedEntryReference, CheckedExpression,
    CheckedExpressionResolution, CheckedImplicitCallable, CheckedPipe, CheckedProjectItem,
    CheckedStyleCallee, CheckedTry, CheckedTryBoundary, CheckedTryCarrier, CheckedTypeSelection,
    CheckedValueResolution, CheckedVariantOwner, CheckedViewCall, CheckedViewCallee, EffectId,
    EffectSet, EntityKind, EnumVariantPayload, ExprId, FinalSemanticAnalysisError,
    GenericTypeOwnerId, GenericTypeParameterId, HirAwaitBranchKind, HirBinaryOp, HirBorrowKind,
    HirCallArgument, HirComputationBlockKind, HirExpr, HirExprKind, HirIdRef, HirIntegerLiteral,
    HirItemKind, HirLiteral, HirModule, HirPathRoot, HirPathSegment, HirPostfixBracket,
    HirPostfixBracketCandidates, HirRecordField, HirRecoveredName, HirScopeKind, HirScopeOwner,
    HirSelectedMember, HirSourcePresence, HirSourceQuery, HirSourceSite, HirStmtKind,
    HirTypeSourceRole, HirUnaryOp, LocalLookup, PostfixBracketResolution,
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
    CallPoison, CallResolverAuthority, CallResolverRequest, CallTargetFacts, CallTargetFactsInput,
    CallableGroupIndex, CharacterDialoguePatchContext, CheckedCallTarget, DialogueCallableId,
    DialogueCalleeIdentity, PreparedCallCallee, ResolveCallOutcome, ResolvedCallTarget,
    ResolverWork, resolve_call_target,
};
use crate::final_analysis::type_rules::integer_suffix_type;
use crate::registration::RegisteredExternalOwner;
use arcweft_lang_hir::expr::HirPlaceholderKind;

use super::entities::EntityReferenceResolutionError;

impl Analyzer<'_, '_, '_> {
    pub(super) fn check_expression(
        &mut self,
        owner: ExprId,
        expected: Option<&TypeKind>,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        if let Some(checked) = self.facts.expressions().get(&owner).cloned() {
            if expected.is_none_or(|expected| expected.accepts(checked.ty())) {
                return Ok(checked);
            }
            let module = self.module(owner.module())?;
            let expression = module
                .resolve_expr(owner)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            if let (Some(expected), HirExprKind::Literal(literal)) = (expected, expression.kind())
                && let Some((ty, selection)) = literal_type(literal, Some(expected))
            {
                let contextual = CheckedExpression::new(
                    ty,
                    selection,
                    checked.effects().clone(),
                    CheckedExpressionResolution::Literal(literal.clone()),
                );
                self.facts.set_expression(owner, contextual.clone());
                return Ok(contextual);
            }
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        }
        if !self.facts.begin_expression(owner) {
            return Err(FinalSemanticAnalysisError::ExpressionCycle { owner });
        }
        let checked = (|| {
            self.control.check()?;
            let module = self.module(owner.module())?;
            let expression = module
                .resolve_expr(owner)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                .clone();
            let (members, placeholders) = expression_placeholder_members(
                module,
                owner,
                HirPlaceholderKind::PartialApplication,
            )?;
            let inside_implicit_callable = self
                .implicit_callable_stack
                .iter()
                .rev()
                .any(|context| context.members.contains(&owner));
            if placeholders.is_empty() || inside_implicit_callable {
                self.check_expression_kind(module, owner, &expression, expected)
            } else {
                self.check_implicit_callable_expression(
                    module,
                    owner,
                    &expression,
                    expected,
                    &members,
                    placeholders,
                )
            }
        })();
        self.facts.end_expression(owner);
        let checked = checked?;
        self.facts.set_expression(owner, checked.clone());
        Ok(checked)
    }

    fn check_implicit_callable_expression(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
        members: &BTreeSet<ExprId>,
        placeholders: BTreeSet<ExprId>,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let contextual = match expected {
            Some(TypeKind::Function {
                params,
                return_type,
                ..
            }) if params.len() == 1 => Some((params[0].clone(), return_type.as_ref().clone())),
            Some(_) => {
                return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
            }
            None => None,
        };
        let parameter = contextual
            .as_ref()
            .map(|(parameter, _)| parameter.clone())
            .map_or_else(
                || self.infer_implicit_parameter(module, owner, expression, &placeholders),
                Ok,
            )?;
        let expected_result = contextual.as_ref().map(|(_, result)| result);
        self.implicit_callable_stack
            .push(super::ImplicitCallableContext {
                owner,
                parameter: parameter.clone(),
                result: expected_result.cloned(),
                members: members.clone(),
                placeholders: placeholders.clone(),
            });
        let body = self.check_expression_kind(module, owner, expression, expected_result);
        let context = self
            .implicit_callable_stack
            .pop()
            .expect("implicit callable context was just pushed");
        let body = body?;
        let captures = self.implicit_callable_captures(module, &context.members)?;
        let result = if matches!(
            body.resolution(),
            CheckedExpressionResolution::Try(tried)
                if tried.boundary() == CheckedTryBoundary::FunctionSite(owner)
        ) {
            context
                .result
                .clone()
                .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?
        } else {
            body.ty().clone()
        };
        let ty = TypeKind::function([parameter.clone()], result.clone());
        if expected.is_some_and(|expected| !expected.accepts(&ty)) {
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        }
        Ok(CheckedExpression::new(
            ty,
            if expected.is_some() {
                CheckedTypeSelection::Expected
            } else {
                CheckedTypeSelection::Inferred
            },
            EffectSet::new(),
            CheckedExpressionResolution::ImplicitCallable(Box::new(CheckedImplicitCallable::new(
                parameter,
                result,
                placeholders.into_iter().collect(),
                captures,
                body.resolution().clone(),
            ))),
        ))
    }

    fn infer_implicit_parameter(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        placeholders: &BTreeSet<ExprId>,
    ) -> Result<TypeKind, FinalSemanticAnalysisError> {
        let HirExprKind::Binary(binary) = expression.kind() else {
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        };
        let left_is_placeholder = placeholders.contains(&binary.left());
        let right_is_placeholder = placeholders.contains(&binary.right());
        if left_is_placeholder == right_is_placeholder {
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        }
        let concrete_owner = if left_is_placeholder {
            binary.right()
        } else {
            binary.left()
        };
        if !expression_placeholder_members(
            module,
            concrete_owner,
            HirPlaceholderKind::PartialApplication,
        )?
        .1
        .is_empty()
        {
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        }
        Ok(self.check_expression(concrete_owner, None)?.ty().clone())
    }

    fn implicit_callable_captures(
        &self,
        module: &HirModule,
        members: &BTreeSet<ExprId>,
    ) -> Result<Box<[super::LocalId]>, FinalSemanticAnalysisError> {
        let mut captures = Vec::new();
        for member in members {
            let Some(CheckedExpressionResolution::Value(CheckedValueResolution::Local(local))) =
                self.facts
                    .expressions()
                    .get(member)
                    .map(CheckedExpression::resolution)
            else {
                continue;
            };
            if !local_is_owned_by_expression_members(module, *local, members)?
                && !captures.contains(local)
            {
                captures.push(*local);
            }
        }
        Ok(captures.into_boxed_slice())
    }

    fn check_expression_kind(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        if expression.is_poisoned() {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        }
        if let Some(checked) = self.check_style_expression_kind(module, owner, expression)? {
            return Ok(checked);
        }
        if let Some(checked) =
            self.check_leaf_expression_kind(module, owner, expression, expected)?
        {
            return Ok(checked);
        }
        if let Some(checked) = self.check_sequence_expression_kind(owner, expression, expected)? {
            return Ok(checked);
        }
        if let Some(checked) =
            self.check_binary_expression_kind(module, owner, expression, expected)?
        {
            return Ok(checked);
        }
        if let Some(checked) =
            self.check_unary_expression_kind(module, owner, expression, expected)?
        {
            return Ok(checked);
        }
        if let Some(checked) =
            self.check_control_expression_kind(module, owner, expression, expected)?
        {
            return Ok(checked);
        }
        if let Some(checked) =
            self.check_closure_expression_kind(module, owner, expression, expected)?
        {
            return Ok(checked);
        }
        if let Some(checked) = self.check_flow_expression_kind(owner, expression, expected)? {
            return Ok(checked);
        }
        if let Some(checked) =
            self.check_aggregate_expression_kind(module, owner, expression, expected)?
        {
            return Ok(checked);
        }
        if let Some(checked) = self.check_variant_expression_kind(owner, expression, expected)? {
            return Ok(checked);
        }
        self.check_entity_expression_kind(module, owner, expression, expected)?
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })
    }
    fn check_leaf_expression_kind(
        &self,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, FinalSemanticAnalysisError> {
        match expression.kind() {
            HirExprKind::Unit => Ok(structural_expression(
                TypeKind::Unit,
                CheckedTypeSelection::Inferred,
            )),
            HirExprKind::Literal(literal) => {
                let (ty, selection) = literal_type(literal, expected)
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                Ok(CheckedExpression::new(
                    ty,
                    selection,
                    EffectSet::new(),
                    CheckedExpressionResolution::Literal(literal.clone()),
                ))
            }
            HirExprKind::Path(path) => {
                let path = path
                    .as_resolved()
                    .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
                if let Some(resolution) =
                    self.resolve_path_value(module, owner, expression.scope(), path)?
                {
                    let ty = match &resolution {
                        CheckedValueResolution::Local(local) => {
                            self.facts.locals().get(local).cloned()
                        }
                        _ => value_resolution_type(self.catalogs.world, &resolution),
                    };
                    let ty =
                        ty.ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
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
                        self.resolve_short_variant(owner, expected, name)?;
                    return Ok(Some(CheckedExpression::new(
                        expected.clone(),
                        CheckedTypeSelection::Expected,
                        EffectSet::new(),
                        CheckedExpressionResolution::Variant(super::CheckedVariantResolution::new(
                            variant_owner,
                            ordinal,
                            name.clone(),
                        )),
                    )));
                }
                let (ty, variant) = resolve_closed_variant_path(
                    self.catalogs.world.environment().typecheck_env(),
                    path,
                    owner,
                )?
                .ok_or(FinalSemanticAnalysisError::ValueResolutionFailed { owner })?;
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
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, FinalSemanticAnalysisError> {
        match expression.kind() {
            HirExprKind::Tuple(tuple) => {
                let children = self.check_expressions(tuple.elements(), None)?;
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
                let children = self.check_expressions(sequence.elements(), None)?;
                let item = common_type(
                    children.iter().map(CheckedExpression::ty),
                    expected_item(expected),
                )
                .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
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
                let value = self.check_expression(repeat.value(), expected_item(expected))?;
                self.check_expression(repeat.length(), Some(&TypeKind::USize))?;
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
                    bounds.push(self.check_expression(start, expected_item(expected))?);
                }
                if let Some(end) = range.end() {
                    bounds.push(self.check_expression(end, expected_item(expected))?);
                }
                let item = common_type(
                    bounds.iter().map(CheckedExpression::ty),
                    expected_item(expected),
                )
                .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
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
        _module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, FinalSemanticAnalysisError> {
        match expression.kind() {
            HirExprKind::Binary(binary) => {
                let left = self.check_expression(binary.left(), None)?;
                let right = self.check_expression(binary.right(), Some(left.ty()))?;
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
                        .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?,
                };
                Ok(structural_expression(ty, CheckedTypeSelection::Inferred))
            }
            _ => return Ok(None),
        }
        .map(Some)
    }

    fn check_unary_expression_kind(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, FinalSemanticAnalysisError> {
        match expression.kind() {
            HirExprKind::Unary(unary) => {
                let operand = self.check_expression(unary.operand(), expected)?;
                let ty = match unary.operator() {
                    HirUnaryOp::Not => TypeKind::Bool,
                    HirUnaryOp::Negate => operand.ty().clone(),
                };
                Ok(structural_expression(ty, CheckedTypeSelection::Inferred))
            }
            HirExprKind::Borrow(borrow) => {
                let operand = self.check_expression(borrow.operand(), None)?;
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
                let operand = self.check_expression(dereference.operand(), None)?;
                let TypeKind::BorrowRef { inner, .. } = operand.ty() else {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
                };
                Ok(structural_expression(
                    (**inner).clone(),
                    CheckedTypeSelection::Inferred,
                ))
            }
            HirExprKind::Index(index) => {
                let target = self.check_expression(index.target(), None)?;
                self.check_expression(index.index(), None)?;
                let ty = indexed_item(target.ty())
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                Ok(structural_expression(ty, CheckedTypeSelection::Inferred))
            }
            HirExprKind::Try(operation) => {
                let operand = self.check_expression(operation.operand(), None)?;
                let carrier = match operand.ty() {
                    TypeKind::Result { ok, error } => CheckedTryCarrier::Result {
                        success: ok.as_ref().clone(),
                        residual: error.clone(),
                    },
                    TypeKind::Option(value) => CheckedTryCarrier::Option {
                        success: value.as_ref().clone(),
                    },
                    _ => {
                        return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                            owner,
                        });
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
                let operand = self.check_expression(operation.operand(), None)?;
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
                    TypeKind::ThreadHandle(value) => {
                        ((**value).clone(), CheckedExpressionResolution::Structural)
                    }
                    _ => {
                        return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                            owner,
                        });
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
    ) -> Result<Vec<CheckedAwaitPendingObserver>, FinalSemanticAnalysisError> {
        branches
            .iter()
            .map(|branch| {
                if branch.kind() == HirAwaitBranchKind::Recovered {
                    return Err(FinalSemanticAnalysisError::RecoveredOwner);
                }
                if branch.kind() != HirAwaitBranchKind::Pending {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                let pattern = branch
                    .pattern()
                    .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
                self.seed_contextual_pattern_locals(module, pattern, &TypeKind::Progress)?;
                Ok(CheckedAwaitPendingObserver::new(pattern))
            })
            .collect()
    }

    fn check_control_expression_kind(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, FinalSemanticAnalysisError> {
        match expression.kind() {
            HirExprKind::Block(block) => {
                let tail = self.check_expression(block.tail(), expected)?;
                Ok(structural_expression(
                    tail.ty().clone(),
                    tail.type_selection(),
                ))
            }
            HirExprKind::ComputationBlock(block) => {
                self.infer_nested_expression_bindings(owner)?;
                let expected_success = match (block.kind(), expected) {
                    (HirComputationBlockKind::Result, Some(TypeKind::Result { ok, .. })) => {
                        Some(ok.as_ref())
                    }
                    (HirComputationBlockKind::Option, Some(TypeKind::Option(item))) => {
                        Some(item.as_ref())
                    }
                    _ => None,
                };
                let tail = self.check_expression(block.tail(), expected_success)?;
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
                                return Err(
                                    FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
                                );
                            }
                        } else if residuals.is_empty() {
                            TypeKind::Never
                        } else {
                            common_type(residuals.iter().copied(), None).ok_or(
                                FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
                            )?
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
                let tail = self.check_expression(block.tail(), expected)?;
                Ok(structural_expression(
                    tail.ty().clone(),
                    tail.type_selection(),
                ))
            }
            HirExprKind::Loop(loop_expression) => {
                self.check_expression(loop_expression.tail(), None)?;
                let mut exits = Vec::new();
                for (_, statement) in module.statements() {
                    let HirStmtKind::Break { label: None, value } = statement.kind() else {
                        continue;
                    };
                    if !break_targets_loop(module, statement.scope(), owner)? {
                        continue;
                    }
                    if let Some(value) = value {
                        exits.push(self.check_expression(*value, expected)?);
                    } else {
                        exits.push(structural_expression(
                            TypeKind::Unit,
                            CheckedTypeSelection::Inferred,
                        ));
                    }
                }
                let (ty, selection) = if exits.is_empty() {
                    (TypeKind::Never, CheckedTypeSelection::Inferred)
                } else {
                    let ty = common_type(exits.iter().map(CheckedExpression::ty), expected)
                        .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                    (ty, CheckedTypeSelection::Inferred)
                };
                Ok(structural_expression(ty, selection))
            }
            HirExprKind::If(conditional) => {
                self.check_expression(conditional.condition(), Some(&TypeKind::Bool))?;
                let then_value = self.check_expression(conditional.then_branch(), expected)?;
                let else_value =
                    self.check_expression(conditional.else_branch(), Some(then_value.ty()))?;
                let ty = common_type([then_value.ty(), else_value.ty()], expected)
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                Ok(structural_expression(ty, CheckedTypeSelection::Inferred))
            }
            HirExprKind::IfLet(conditional) => {
                let scrutinee = self.check_expression(conditional.scrutinee(), None)?;
                self.seed_contextual_pattern_locals(module, conditional.pattern(), scrutinee.ty())?;
                if let Some(guard) = conditional.guard() {
                    self.check_expression(guard, Some(&TypeKind::Bool))?;
                }
                let then_value = self.check_expression(conditional.then_branch(), expected)?;
                let else_value =
                    self.check_expression(conditional.else_branch(), Some(then_value.ty()))?;
                let ty = common_type([then_value.ty(), else_value.ty()], expected)
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                Ok(structural_expression(ty, CheckedTypeSelection::Inferred))
            }
            HirExprKind::Match(match_expr) => {
                let scrutinee = self.check_expression(match_expr.scrutinee(), None)?;
                let mut values = Vec::new();
                for arm in match_expr.arms() {
                    self.seed_contextual_pattern_locals(module, arm.pattern(), scrutinee.ty())?;
                    if let Some(guard) = arm.guard() {
                        self.check_expression(guard, Some(&TypeKind::Bool))?;
                    }
                    values.push(self.check_expression(arm.value(), expected)?);
                }
                let ty = common_type(values.iter().map(CheckedExpression::ty), expected)
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                Ok(structural_expression(ty, CheckedTypeSelection::Inferred))
            }
            _ => return Ok(None),
        }
        .map(Some)
    }

    fn try_residuals_for_block(&self, owner: ExprId) -> Vec<&TypeKind> {
        self.facts
            .expressions()
            .values()
            .filter_map(|expression| match expression.resolution() {
                CheckedExpressionResolution::Try(tried)
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
    ) -> Result<CheckedTryBoundary, FinalSemanticAnalysisError> {
        if matches!(
            carrier,
            CheckedTryCarrier::Result { residual, .. }
                if matches!(residual.as_ref(), TypeKind::Never)
        ) {
            return Ok(CheckedTryBoundary::Infallible);
        }
        loop {
            let current = module
                .resolve_scope(scope)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            match current.owner() {
                HirScopeOwner::Expr(expression) => {
                    let expression_kind = module
                        .resolve_expr(*expression)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
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
                            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                                owner,
                            });
                    }
                    if current.kind() == HirScopeKind::Closure {
                        let context = self
                            .function_site_stack
                            .iter()
                            .rev()
                            .find(|context| context.owner == *expression)
                            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                                owner,
                            })?;
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
                            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                                owner,
                            });
                    }
                }
                HirScopeOwner::Item(item) => {
                    if let Some(context) = self
                        .implicit_callable_stack
                        .iter()
                        .rev()
                        .find(|context| context.members.contains(&owner))
                    {
                        let boundary = context.result.as_ref().ok_or(
                            FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
                        )?;
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
                            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                                owner,
                            });
                    }
                    return self.resolve_item_try_boundary(module, owner, *item, carrier);
                }
                HirScopeOwner::Module(_) | HirScopeOwner::Stmt(_) => {}
            }
            let Some(parent) = current.parent() else {
                return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
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
    ) -> Result<CheckedTryBoundary, FinalSemanticAnalysisError> {
        let item_kind = module
            .resolve_item(item)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .kind();
        let return_type = match item_kind {
            HirItemKind::Function(function) => function.return_type(),
            HirItemKind::Flow(flow) => flow.result().authored_type(),
            _ => None,
        }
        .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
        let boundary = self
            .types
            .get(&return_type)
            .ok_or(FinalSemanticAnalysisError::TypeResolutionFailed { owner: return_type })?;
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
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })
    }

    fn try_error_mismatch(
        module: &HirModule,
        owner: ExprId,
        return_type: arcweft_lang_hir::identity::TypeId,
        operand_error: &TypeKind,
        return_error: &TypeKind,
    ) -> Result<FinalSemanticAnalysisError, FinalSemanticAnalysisError> {
        Ok(FinalSemanticAnalysisError::PropagationErrorMismatch {
            owner,
            operand_error: Box::new(operand_error.clone()),
            return_error: Box::new(return_error.clone()),
            operator_source: source_span_for_role(
                module,
                HirSourceQuery::Expr {
                    owner,
                    role: arcweft_lang_hir::source_index::HirExprSourceRole::Operator,
                },
            )?,
            return_source: source_span_for_role(
                module,
                HirSourceQuery::Type {
                    owner: return_type,
                    role: HirTypeSourceRole::Whole,
                },
            )?,
        })
    }
    fn check_closure_expression_kind(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, FinalSemanticAnalysisError> {
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
                        return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                            owner,
                        });
                    }
                };
                if expected.is_some() && contextual_function.is_none() {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
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
                            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                                owner,
                            });
                        }
                        (Some(annotated), None) => annotated,
                        (None, Some(contextual)) => contextual.clone(),
                        (None, None) => self.pattern_type_hint(module, parameter.pattern()).ok_or(
                            FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
                        )?,
                    };
                    self.seed_contextual_pattern_locals(
                        module,
                        parameter.pattern(),
                        &parameter_ty,
                    )?;
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
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
                }
                let body_expected = declared_result.as_ref().or(contextual_result);
                if let Some(result) = body_expected {
                    self.function_site_stack.push(super::FunctionSiteContext {
                        owner,
                        result: result.clone(),
                    });
                }
                let body = self.check_expression(closure.body(), body_expected);
                if body_expected.is_some() {
                    self.function_site_stack
                        .pop()
                        .expect("function-site context was just pushed");
                }
                let body = body?;
                let result = if matches!(
                    body.resolution(),
                    CheckedExpressionResolution::Try(tried)
                        if tried.boundary() == CheckedTryBoundary::FunctionSite(owner)
                ) {
                    body_expected
                        .cloned()
                        .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?
                } else {
                    body.ty().clone()
                };
                let ty = TypeKind::function(parameters, result);
                if expected.is_some_and(|expected| !expected.accepts(&ty)) {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
                }
                Ok(structural_expression(
                    ty,
                    if expected.is_some() {
                        CheckedTypeSelection::Expected
                    } else {
                        CheckedTypeSelection::Inferred
                    },
                ))
            }
            _ => return Ok(None),
        }
        .map(Some)
    }
    fn check_flow_expression_kind(
        &mut self,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, FinalSemanticAnalysisError> {
        match expression.kind() {
            HirExprKind::Pipe(pipe) => {
                let left = self.check_expression(pipe.left(), None)?;
                let (_, placeholders) = expression_placeholder_members(
                    self.module(owner.module())?,
                    pipe.right(),
                    HirPlaceholderKind::PipeLeft,
                )?;
                self.pipe_stack.push(super::PipeContext {
                    owner,
                    value: left.ty().clone(),
                    placeholders: placeholders.clone(),
                });
                let right = self.check_expression(pipe.right(), expected);
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
                let input = self.check_expression(synthetic.input(), expected)?;
                let ty = match synthetic {
                    arcweft_lang_hir::expr::HirForSyntheticExpr::Iterator { .. } => {
                        let iteration = self.select_iteration(input.ty())?;
                        let ty = super::statements::iteration_iterator(&iteration);
                        if self.iteration_facts.insert(owner, iteration).is_some() {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                        }
                        ty
                    }
                    arcweft_lang_hir::expr::HirForSyntheticExpr::NextValue { .. } => {
                        let iteration = self.iteration_facts.get(&synthetic.input()).ok_or(
                            FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                                owner: synthetic.input(),
                            },
                        )?;
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
            _ => return Ok(None),
        }
        .map(Some)
    }
    fn check_aggregate_expression_kind(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, FinalSemanticAnalysisError> {
        match expression.kind() {
            HirExprKind::Placeholder(HirPlaceholderKind::PartialApplication) => {
                let context = self
                    .implicit_callable_stack
                    .iter()
                    .rev()
                    .find(|context| context.placeholders.contains(&owner))
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                if expected.is_some_and(|expected| !expected.accepts(&context.parameter)) {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
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
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                if expected.is_some_and(|expected| !expected.accepts(&context.value)) {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
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
                if let Some(checked) = self.check_view_call_expression(module, expression, call)? {
                    Ok(checked)
                } else {
                    self.check_call_expression(module, owner, call, expected)
                }
            }
            HirExprKind::Record(record) => {
                let declaration = match self
                    .symbols
                    .resolve_hir_type_target(
                        module.key().path(),
                        record.path(),
                        expression_span(module, owner)?,
                    )
                    .map_err(|_| FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?
                {
                    ProjectTypeTarget::Nominal(declaration) => declaration.clone(),
                    ProjectTypeTarget::External(_) => {
                        return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                            owner,
                        });
                    }
                };
                let (ty, selection) = self.check_project_record_fields(
                    owner,
                    &declaration,
                    record.fields(),
                    expected,
                )?;
                let nominal = checked_project_nominal(&declaration, &ty)?;
                Ok(CheckedExpression::new(
                    ty,
                    selection,
                    EffectSet::new(),
                    CheckedExpressionResolution::Nominal(nominal),
                ))
            }
            HirExprKind::RecordLiteral(record) => {
                let Some(TypeKind::ProjectNominal(expected_nominal)) = expected else {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
                };
                let declaration = self
                    .symbols
                    .nominal(expected_nominal.declaration())
                    .cloned()
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                let (ty, _) = self.check_project_record_fields(
                    owner,
                    &declaration,
                    record.fields(),
                    expected,
                )?;
                let nominal = checked_project_nominal(&declaration, &ty)?;
                Ok(CheckedExpression::new(
                    ty,
                    CheckedTypeSelection::Expected,
                    EffectSet::new(),
                    CheckedExpressionResolution::Nominal(nominal),
                ))
            }
            HirExprKind::Select(select) => self.check_select_expression(owner, select),
            _ => return Ok(None),
        }
        .map(Some)
    }

    fn check_select_expression(
        &mut self,
        owner: ExprId,
        select: &arcweft_lang_hir::expr::HirSelectExpr,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let target = self.check_expression(select.target(), None)?;
        let HirSelectedMember::Name(name) = select.member() else {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
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
                        .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                    let ProjectNominalBody::Struct { fields } = declaration.body() else {
                        return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                            owner,
                        });
                    };
                    let (ordinal, field) = fields
                        .iter()
                        .enumerate()
                        .find(|(_, field)| field.name().as_str() == name.as_str())
                        .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                    let ordinal = u32::try_from(ordinal).map_err(|_| {
                        FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner }
                    })?;
                    let declared_ty = self.types.get(&field.ty()).ok_or(
                        FinalSemanticAnalysisError::TypeResolutionFailed { owner: field.ty() },
                    )?;
                    let substitutions = nominal_substitutions(&declaration, target_nominal)
                        .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                    let nominal = checked_project_nominal(&declaration, target.ty())?;
                    (
                        substitutions.apply(declared_ty),
                        super::CheckedSelectResolution::Field {
                            nominal: Some(nominal),
                            ordinal: Some(ordinal),
                            name: name.clone(),
                        },
                    )
                }
                TypeKind::Named(type_name) => {
                    let environment = self.catalogs.world.environment().typecheck_env();
                    let ty = environment
                        .nominal_records()
                        .get(type_name)
                        .and_then(|fields| fields.get(name.as_str()))
                        .cloned()
                        .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                    let resolution = environment
                        .dialogue_view_models()
                        .projection(type_name, name.as_str())
                        .map_or_else(
                            || super::CheckedSelectResolution::Field {
                                nominal: None,
                                ordinal: None,
                                name: name.clone(),
                            },
                            |projection| super::CheckedSelectResolution::DialogueView {
                                projection,
                                name: name.clone(),
                            },
                        );
                    (ty, resolution)
                }
                _ => return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner }),
            }
        };
        Ok(CheckedExpression::new(
            ty,
            CheckedTypeSelection::Inferred,
            target.effects().clone(),
            CheckedExpressionResolution::Select(resolution),
        ))
    }

    fn check_view_call_expression(
        &mut self,
        module: &HirModule,
        expression: &HirExpr,
        call: &arcweft_lang_hir::expr::HirCallExpr,
    ) -> Result<Option<CheckedExpression>, FinalSemanticAnalysisError> {
        let Some(item) = enclosing_item(module, expression.scope())? else {
            return Ok(None);
        };
        let HirItemKind::View(_) = module
            .resolve_item(item)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .kind()
        else {
            return Ok(None);
        };

        let classification = match call.callee() {
            super::HirCallCallee::Value { value } => {
                let Some(callee) = Self::view_direct_callee(module, *value)? else {
                    return Ok(None);
                };
                if self.facts.set_expression(
                    *value,
                    CheckedExpression::new(
                        TypeKind::Named("ViewCallable".to_owned()),
                        super::CheckedTypeSelection::Inferred,
                        EffectSet::new(),
                        CheckedExpressionResolution::ViewCallee(callee.clone()),
                    ),
                ) {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
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
                let receiver = self.check_expression(*value_receiver, None)?;
                if receiver.ty() != &TypeKind::ViewValue {
                    return Ok(None);
                }
                let HirRecoveredName::Valid(member) = member else {
                    return Err(FinalSemanticAnalysisError::RecoveredOwner);
                };
                CheckedViewCall::Modifier {
                    member: member.clone(),
                }
            }
            super::HirCallCallee::Associated { .. } => return Ok(None),
        };

        let mut effects = EffectSet::new();
        for argument in call.arguments() {
            let checked = self.check_expression(argument.value(), None)?;
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
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
    ) -> Result<Option<CheckedExpression>, FinalSemanticAnalysisError> {
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
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        };
        let mut effects = EffectSet::new();
        let channels =
            [red, green, blue, alpha].map(|argument| -> Result<u8, FinalSemanticAnalysisError> {
                let HirCallArgument::Positional { .. } = argument else {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
                };
                let checked = self.check_expression(argument.value(), Some(&TypeKind::U8))?;
                effects.union_with(checked.effects());
                style_u8_literal(module, argument.value())
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })
            });
        let [red, green, blue, alpha] = channels;
        let color = arcweft_view::style::ViewColorValue::Literal {
            color: arcweft_presentation::appearance::PresentationColor::rgba(
                red?, green?, blue?, alpha?,
            ),
        };
        if self.facts.set_expression(
            *callee,
            CheckedExpression::new(
                TypeKind::Named("StyleColorConstructor".to_owned()),
                CheckedTypeSelection::Inferred,
                EffectSet::new(),
                CheckedExpressionResolution::StyleCallee(CheckedStyleCallee::Rgba),
            ),
        ) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
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
    ) -> Result<Option<String>, FinalSemanticAnalysisError> {
        let expression = module
            .resolve_expr(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
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
    ) -> Result<Option<CheckedViewCallee>, FinalSemanticAnalysisError> {
        let expression = module
            .resolve_expr(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
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
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, FinalSemanticAnalysisError> {
        match expression.kind() {
            HirExprKind::ShortVariant(name) => {
                let name = name
                    .as_resolved()
                    .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
                let Some(expected) = expected else {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
                };
                let (variant_owner, ordinal) = self.resolve_short_variant(owner, expected, name)?;
                Ok(CheckedExpression::new(
                    expected.clone(),
                    CheckedTypeSelection::Expected,
                    EffectSet::new(),
                    CheckedExpressionResolution::Variant(super::CheckedVariantResolution::new(
                        variant_owner,
                        ordinal,
                        name.clone(),
                    )),
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
    ) -> Result<(CheckedVariantOwner, u32), FinalSemanticAnalysisError> {
        match expected {
            TypeKind::ProjectNominal(expected_nominal) => {
                let declaration = self
                    .symbols
                    .nominal(expected_nominal.declaration())
                    .cloned()
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                let ProjectNominalBody::Enum { variants } = declaration.body() else {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
                };
                let (ordinal, variant) = variants
                    .iter()
                    .enumerate()
                    .find(|(_, variant)| variant.name().as_str() == name.as_str())
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                if variant.payload().is_some() {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
                }
                Ok((
                    CheckedVariantOwner::Project(checked_project_nominal(&declaration, expected)?),
                    u32::try_from(ordinal)
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                ))
            }
            TypeKind::CharacterNominal(nominal) => {
                let variants = self
                    .catalogs
                    .world
                    .environment()
                    .character_enum_variants(nominal)
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                let ordinal = variants
                    .iter()
                    .position(|variant| variant == name.as_str())
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                Ok((
                    CheckedVariantOwner::CharacterNominal {
                        nominal: nominal.clone(),
                        cases: variants.to_vec().into_boxed_slice(),
                    },
                    u32::try_from(ordinal)
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                ))
            }
            TypeKind::Option(item) if name.as_str() == "None" => Ok((
                CheckedVariantOwner::Option {
                    item: item.as_ref().clone(),
                },
                1,
            )),
            closed_enum_ty => {
                let schema = self
                    .catalogs
                    .world
                    .environment()
                    .typecheck_env()
                    .closed_enum(closed_enum_ty)
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                let (ordinal, selected) = schema
                    .variants()
                    .iter()
                    .enumerate()
                    .find(|(_, variant)| variant.name() == name.as_str())
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                if !matches!(selected.payload(), EnumVariantPayload::Unit) {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
                }
                Ok((
                    checked_builtin_closed_owner(schema, closed_enum_ty, owner)?,
                    u32::try_from(ordinal)
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                ))
            }
        }
    }
    fn check_entity_expression_kind(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, FinalSemanticAnalysisError> {
        match expression.kind() {
            HirExprKind::EntityReference(reference) => {
                let reference = reference
                    .as_resolved()
                    .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
                if reference.absolute_family() == Some("entry") {
                    return self
                        .check_entry_reference(owner, reference, expected)
                        .map(Some);
                }
                if matches!(
                    expected,
                    Some(TypeKind::Ref(entity))
                        if entity.kind() == &EntityKind::DialogueLine
                ) {
                    return self
                        .check_dialogue_line_reference(owner, reference, expected)
                        .map(Some);
                }
                let source = expression_span(module, owner)?;
                let item = self
                    .resolve_checked_entity_reference(module, reference, source)
                    .map_err(|error| match error {
                        EntityReferenceResolutionError::Lookup => {
                            FinalSemanticAnalysisError::ValueResolutionFailed { owner }
                        }
                        EntityReferenceResolutionError::WrongFamily => {
                            FinalSemanticAnalysisError::WrongPayloadFamily
                        }
                    })?;
                let ty = item.ty();
                if expected.is_some_and(|expected| !expected.accepts(&ty)) {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
                }
                Ok(CheckedExpression::new(
                    ty,
                    CheckedTypeSelection::Inferred,
                    EffectSet::new(),
                    CheckedExpressionResolution::Value(CheckedValueResolution::ProjectItem(item)),
                ))
            }
            HirExprKind::DialogueContentApplication(application) => {
                self.check_dialogue_content_application(module, owner, application, expected)
            }
            HirExprKind::PostfixBracket(postfix) => {
                self.check_postfix_bracket(owner, postfix, expected)
            }
            HirExprKind::LifetimePath(_) | HirExprKind::Choice(_) => {
                Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })
            }
            HirExprKind::Error(_) => Err(FinalSemanticAnalysisError::RecoveredOwner),
            _ => return Ok(None),
        }
        .map(Some)
    }

    fn check_dialogue_line_reference(
        &self,
        owner: ExprId,
        reference: &HirIdRef,
        expected: Option<&TypeKind>,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let HirIdRef::Absolute(reference) = reference else {
            return Err(FinalSemanticAnalysisError::ValueResolutionFailed { owner });
        };
        let target = arcweft_id::dialogue::DialogueLineId::try_new(reference.as_str())
            .map_err(|_| FinalSemanticAnalysisError::ValueResolutionFailed { owner })?;
        if self.project.dialogue_lines().get(&target).is_none() {
            return Err(FinalSemanticAnalysisError::ValueResolutionFailed { owner });
        }
        let ty = TypeKind::entity_ref(EntityKind::DialogueLine);
        if expected.is_some_and(|expected| !expected.accepts(&ty)) {
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        }
        Ok(CheckedExpression::new(
            ty,
            CheckedTypeSelection::Expected,
            EffectSet::new(),
            CheckedExpressionResolution::DialogueLineReference(target),
        ))
    }

    fn check_entry_reference(
        &self,
        owner: ExprId,
        reference: &HirIdRef,
        expected: Option<&TypeKind>,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let HirIdRef::Absolute(reference) = reference else {
            return Err(FinalSemanticAnalysisError::ValueResolutionFailed { owner });
        };
        let public_id = arcweft_id::PublicId::try_new(reference.as_str())
            .map_err(|_| FinalSemanticAnalysisError::ValueResolutionFailed { owner })?;
        let mut matches = self.executable.items().filter_map(|item| {
            let HirItemKind::Entry(entry) = item.item().kind() else {
                return None;
            };
            let HirIdRef::Absolute(candidate) = entry.id().value()?.as_resolved()? else {
                return None;
            };
            (candidate == reference).then_some(item.id())
        });
        let entry_owner = matches
            .next()
            .ok_or(FinalSemanticAnalysisError::ValueResolutionFailed { owner })?;
        if matches.next().is_some() {
            return Err(FinalSemanticAnalysisError::ValueResolutionFailed { owner });
        }
        let entry = CheckedEntryReference::new(public_id, entry_owner);
        let ty = entry.ty();
        if expected.is_some_and(|expected| !expected.accepts(&ty)) {
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        }
        Ok(CheckedExpression::new(
            ty,
            if expected.is_some() {
                CheckedTypeSelection::Expected
            } else {
                CheckedTypeSelection::Inferred
            },
            EffectSet::new(),
            CheckedExpressionResolution::Value(CheckedValueResolution::Entry(entry)),
        ))
    }

    fn check_postfix_bracket(
        &mut self,
        owner: ExprId,
        postfix: &HirPostfixBracket,
        expected: Option<&TypeKind>,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = postfix.candidates()
        else {
            return Err(FinalSemanticAnalysisError::UnresolvedPostfixBracket { owner });
        };
        let index_id = *index;
        let dialogue_id = *dialogue;
        let index_probe = self.probe_postfix_candidate(index_id, expected);
        let dialogue_probe = self.probe_postfix_candidate(dialogue_id, expected);
        let (checked, projection, resolution) = match (index_probe, dialogue_probe) {
            (Ok((checked, projection)), Err(_)) => (
                checked,
                projection,
                PostfixBracketResolution::Index {
                    candidate: index_id,
                },
            ),
            (Err(_), Ok((checked, projection))) => (
                checked,
                projection,
                PostfixBracketResolution::Dialogue {
                    candidate: dialogue_id,
                },
            ),
            (Ok(_), Ok(_)) => {
                return Err(FinalSemanticAnalysisError::AmbiguousPostfixBracket { owner });
            }
            (Err(_), Err(dialogue_error)) => {
                if dialogue_error.proves_dialogue_postfix_candidate() {
                    return Err(dialogue_error);
                }
                return Err(FinalSemanticAnalysisError::UnresolvedPostfixBracket { owner });
            }
        };
        self.facts.apply_candidate_projection(projection);
        Ok(CheckedExpression::new(
            checked.ty().clone(),
            checked.type_selection(),
            checked.effects().clone(),
            CheckedExpressionResolution::PostfixBracket(resolution),
        ))
    }

    fn probe_postfix_candidate(
        &mut self,
        candidate: ExprId,
        expected: Option<&TypeKind>,
    ) -> Result<(CheckedExpression, CandidateSemanticProjection), FinalSemanticAnalysisError> {
        let checkpoint = self.facts.begin_candidate_transaction();
        let result = self.check_expression(candidate, expected);
        let projection = result
            .is_ok()
            .then(|| self.facts.capture_candidate_projection(checkpoint));
        self.facts.rollback_candidate_transaction(checkpoint);
        match result {
            Ok(checked) => Ok((
                checked,
                projection.expect("successful postfix probe captures a projection"),
            )),
            Err(error) => Err(error),
        }
    }

    fn check_dialogue_content_application(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        application: &arcweft_lang_hir::dialogue_application::HirDialogueContentApplication,
        expected: Option<&TypeKind>,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        // Immediate id/text_key arguments are compile-time application
        // coordinates. Publish their accepted project identities before the
        // shared parenthesized-call resolver evaluates argument facts.
        self.publish_dialogue_coordinates(owner, application)?;
        let target_owner = application.target();
        let target_expression = module
            .resolve_expr(target_owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let checked_target = match target_expression.kind() {
            HirExprKind::Call(call) => {
                let checked =
                    self.check_immediate_character_dialogue_call(module, target_owner, call)?;
                self.facts.set_expression(target_owner, checked.clone());
                checked
            }
            _ => self.check_expression(target_owner, None)?,
        };
        let target = checked_character_dialogue_target(target_owner, &checked_target)
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
        self.publish_dialogue_content_application_call(module, owner, &target, expected)?;
        let application_patch = match checked_target.resolution() {
            CheckedExpressionResolution::CharacterDialogueFactory(factory) => {
                Some(factory.patch().clone())
            }
            CheckedExpressionResolution::CharacterDialogueReconfigure(reconfigure) => {
                Some(reconfigure.patch().clone())
            }
            _ => None,
        };
        let rich_text = RichTextAttributeChecker::check(module, application.content())
            .map_err(|_| FinalSemanticAnalysisError::RichTextSourceQuery { owner })?;
        if !rich_text.is_valid() {
            return Err(FinalSemanticAnalysisError::InvalidRichTextAttributes {
                owner,
                report: Box::new(rich_text),
            });
        }
        let application_children = module
            .resolve_expr(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .kind()
            .direct_expression_children();
        for child in application_children {
            if child != target_owner {
                self.check_expression(child, None)?;
            }
        }

        let ty = TypeKind::DialogueLine(Box::new(TypeKind::Unit));
        let selection = match expected.map(|expected| expected.accepts(&ty)) {
            Some(true) => CheckedTypeSelection::Expected,
            None => CheckedTypeSelection::Inferred,
            Some(false) => {
                return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
            }
        };
        Ok(CheckedExpression::new(
            ty,
            selection,
            EffectSet::new(),
            CheckedExpressionResolution::DialogueApplication {
                target,
                application_patch,
                rich_text: Box::new(rich_text),
            },
        ))
    }

    fn publish_dialogue_content_application_call(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        target: &super::CheckedCharacterDialogueTarget,
        expected: Option<&TypeKind>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let callee = match target {
            super::CheckedCharacterDialogueTarget::Character { character, .. } => {
                DialogueCalleeIdentity::Character {
                    character: character.clone(),
                }
            }
            super::CheckedCharacterDialogueTarget::Dialogue { ty, .. } => {
                DialogueCalleeIdentity::CharacterDialogue {
                    character: ty.character().clone(),
                }
            }
        };
        let prepared = PreparedCallCallee::Dialogue {
            id: DialogueCallableId::ContentApplication,
            callee: &callee,
            patch_context: CharacterDialoguePatchContext::ImmediateContentApplication,
        };
        let authority = CallResolverAuthority::accepted(
            self.project,
            module,
            self.symbols,
            self.catalogs.world,
        );
        let staged = self
            .staged_callables
            .as_ref()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let mut work = ResolverWork::new(self.catalogs.callable_limits.max_query_work());
        let request = CallResolverRequest::try_new_dialogue_application(
            prepared,
            &super::CallResolverContext {
                authority,
                checked: (&staged.builder).into(),
                expected,
                expression: owner,
                cancellation: self.control.cancellation(),
                limits: &self.catalogs.callable_limits,
            },
            &mut work,
        )
        .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed { owner })?;
        let callee = request.classification();
        let outcome = resolve_call_target(request);
        let ResolveCallOutcome::Resolved(ResolvedCallTarget::Candidates(candidates)) = outcome
        else {
            return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner });
        };
        let selected = candidates.first();
        if selected.id()
            != &crate::callable::CallableCandidateId::Dialogue(
                DialogueCallableId::ContentApplication,
            )
            || selected.schema().result() != &TypeKind::DialogueLine(Box::new(TypeKind::Unit))
        {
            return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner });
        }
        let checked = CheckedCallTarget::selected(
            selected,
            candidates.as_slice(),
            Vec::new(),
            selected.schema().result().clone(),
            crate::effect_row::EffectRow::closed(EffectSet::new()),
            CallableGroupIndex::ZERO,
            CallPoison::Clean,
        );
        let enclosing_callable = self.enclosing_ordinary_callable(module, owner)?;
        let facts = CallTargetFacts::try_new(
            CallTargetFactsInput {
                expression: owner,
                enclosing_callable,
                callee: Some(callee),
                checked,
                diagnostics: Vec::new(),
                accounting: work.call_accounting(),
            },
            &self.catalogs.callable_limits,
        )
        .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed { owner })?;
        if self.facts.set_call_fact(owner, facts) {
            return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner });
        }
        Ok(())
    }

    fn publish_dialogue_coordinates(
        &mut self,
        owner: ExprId,
        application: &arcweft_lang_hir::dialogue_application::HirDialogueContentApplication,
    ) -> Result<(), FinalSemanticAnalysisError> {
        if application.coordinates().is_empty() {
            return Ok(());
        }
        let accepted = self
            .project
            .dialogue_lines()
            .for_expr(owner)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        for coordinate in application.coordinates() {
            let (ty, resolution) = match coordinate.kind() {
                arcweft_lang_hir::dialogue_application::HirDialogueCoordinateKind::Id => (
                    TypeKind::entity_ref(EntityKind::DialogueLine),
                    CheckedExpressionResolution::DialogueLineCoordinate(accepted.id().clone()),
                ),
                arcweft_lang_hir::dialogue_application::HirDialogueCoordinateKind::TextKey => (
                    TypeKind::entity_ref(EntityKind::Text),
                    CheckedExpressionResolution::DialogueTextKeyCoordinate(
                        accepted.text_key().clone(),
                    ),
                ),
            };
            self.facts.set_expression(
                coordinate.value(),
                CheckedExpression::new(
                    ty,
                    CheckedTypeSelection::Inferred,
                    EffectSet::new(),
                    resolution,
                ),
            );
        }
        Ok(())
    }

    fn check_expressions(
        &mut self,
        owners: &[ExprId],
        expected: Option<&TypeKind>,
    ) -> Result<Vec<CheckedExpression>, FinalSemanticAnalysisError> {
        owners
            .iter()
            .map(|owner| self.check_expression(*owner, expected))
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

fn expression_placeholder_members(
    module: &HirModule,
    root: ExprId,
    placeholder_kind: HirPlaceholderKind,
) -> Result<(BTreeSet<ExprId>, BTreeSet<ExprId>), FinalSemanticAnalysisError> {
    let mut members = BTreeSet::new();
    let mut placeholders = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(owner) = pending.pop() {
        if !members.insert(owner) {
            continue;
        }
        let expression = module
            .resolve_expr(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if matches!(
            expression.kind(),
            HirExprKind::Placeholder(kind) if *kind == placeholder_kind
        ) {
            placeholders.insert(owner);
        }
        if matches!(expression.kind(), HirExprKind::Closure(_))
            || (placeholder_kind == HirPlaceholderKind::PartialApplication
                && matches!(expression.kind(), HirExprKind::Call(_)))
        {
            continue;
        }
        pending.extend(expression.kind().direct_expression_children());
    }
    Ok((members, placeholders))
}

fn local_is_owned_by_expression_members(
    module: &HirModule,
    local: super::LocalId,
    members: &BTreeSet<ExprId>,
) -> Result<bool, FinalSemanticAnalysisError> {
    let mut scope = module
        .resolve_local(local)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
        .scope();
    loop {
        let current = module
            .resolve_scope(scope)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if matches!(current.owner(), HirScopeOwner::Expr(owner) if members.contains(owner)) {
            return Ok(true);
        }
        let Some(parent) = current.parent() else {
            return Ok(false);
        };
        scope = parent;
    }
}

fn structural_expression(ty: TypeKind, selection: CheckedTypeSelection) -> CheckedExpression {
    CheckedExpression::new(
        ty,
        selection,
        EffectSet::new(),
        CheckedExpressionResolution::Structural,
    )
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
