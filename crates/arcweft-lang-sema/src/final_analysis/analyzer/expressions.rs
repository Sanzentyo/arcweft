//! Expression-family checking outside ordinary-call resolution.

#[path = "expressions/records.rs"]
mod records;

use super::{
    Analyzer, ArrayLength, BTreeSet, BorrowKind, CandidateSemanticProjection,
    CheckedEntryReference, CheckedExpression, CheckedExpressionResolution, CheckedProjectItem,
    CheckedStyleCallee, CheckedTypeSelection, CheckedValueResolution, CheckedVariantOwner,
    CheckedViewCall, CheckedViewCallee, DeclarationIdentityFamily, EffectId, EffectSet, EntityKind,
    EnumVariantPayload, ExprId, FinalSemanticAnalysisError, GenericTypeOwnerId,
    GenericTypeParameterId, HirAwaitPropagation, HirBinaryOp, HirBorrowKind, HirCallArgument,
    HirComputationBlockKind, HirExpr, HirExprKind, HirExprSourceRole, HirIdRef, HirIntegerLiteral,
    HirItemKind, HirLiteral, HirModule, HirPathRoot, HirPathSegment, HirPostfixBracket,
    HirPostfixBracketCandidates, HirRecordField, HirRecoveredName, HirSelectedMember,
    HirSourcePresence, HirSourceQuery, HirSourceSite, HirTypeSourceRole, HirUnaryOp, ItemId,
    LocalLookup, PostfixBracketResolution, ProjectNominalBody, ProjectNominalDeclaration,
    ProjectNominalType, ProjectTypeTarget, ProjectValueLookup, PropagationOperator,
    RegisteredSemanticValueId, ResolvedProjectSymbol, RichTextAttributeChecker, ScopeId,
    SourceSpan, TypeKind, TypeParameterSubstitutions,
    calls::{checked_project_nominal, nominal_substitutions},
    expression_types::{
        common_type, expected_item, indexed_item, literal_type, value_resolution_type,
    },
    patterns::{checked_builtin_closed_owner, resolve_closed_variant_path},
    statements::{enclosing_item, expression_span},
};
use crate::final_analysis::type_rules::integer_suffix_type;

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
            self.check_expression_kind(module, owner, &expression, expected)
        })();
        self.facts.end_expression(owner);
        let checked = checked?;
        self.facts.set_expression(owner, checked.clone());
        Ok(checked)
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
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: Option<&TypeKind>,
    ) -> Result<Option<CheckedExpression>, FinalSemanticAnalysisError> {
        match expression.kind() {
            HirExprKind::Binary(binary) => {
                let left_is_placeholder = matches!(
                    module
                        .resolve_expr(binary.left())
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                        .kind(),
                    HirExprKind::Placeholder(_)
                );
                let right_is_placeholder = matches!(
                    module
                        .resolve_expr(binary.right())
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                        .kind(),
                    HirExprKind::Placeholder(_)
                );
                if left_is_placeholder || right_is_placeholder {
                    return self
                        .check_partial_binary_expression(
                            owner,
                            binary,
                            expected,
                            left_is_placeholder,
                            right_is_placeholder,
                        )
                        .map(Some);
                }
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

    fn check_partial_binary_expression(
        &mut self,
        owner: ExprId,
        binary: &arcweft_lang_hir::expr::HirBinaryExpr,
        expected: Option<&TypeKind>,
        left_is_placeholder: bool,
        right_is_placeholder: bool,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        if left_is_placeholder == right_is_placeholder {
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        }
        let contextual = match expected {
            Some(TypeKind::Function {
                params,
                return_type,
                ..
            }) if params.len() == 1 => Some((&params[0], return_type.as_ref())),
            Some(_) => {
                return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
            }
            None => None,
        };
        let contextual_parameter = contextual.map(|(parameter, _)| parameter);
        let (left, right, parameter) = if left_is_placeholder {
            let right = self.check_expression(binary.right(), contextual_parameter)?;
            let parameter = contextual_parameter.unwrap_or(right.ty()).clone();
            let left = self.check_expression(binary.left(), Some(&parameter))?;
            (left, right, parameter)
        } else {
            let left = self.check_expression(binary.left(), contextual_parameter)?;
            let parameter = contextual_parameter.unwrap_or(left.ty()).clone();
            let right = self.check_expression(binary.right(), Some(&parameter))?;
            (left, right, parameter)
        };
        let result = match binary.operator() {
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
            | HirBinaryOp::Remainder => common_type(
                [left.ty(), right.ty()],
                contextual.map(|(_, result)| result),
            )
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?,
        };
        if contextual.is_some_and(|(_, expected)| !expected.accepts(&result)) {
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        }
        Ok(structural_expression(
            TypeKind::function([parameter], result),
            if expected.is_some() {
                CheckedTypeSelection::Expected
            } else {
                CheckedTypeSelection::Inferred
            },
        ))
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
                let ty = match operand.ty() {
                    TypeKind::Result { ok, error } => {
                        self.validate_propagation_error(
                            module,
                            owner,
                            expression.scope(),
                            PropagationOperator::Try,
                            error,
                        )?;
                        (**ok).clone()
                    }
                    TypeKind::Option(value) => (**value).clone(),
                    _ => {
                        return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                            owner,
                        });
                    }
                };
                Ok(structural_expression(ty, CheckedTypeSelection::Inferred))
            }
            HirExprKind::Await(operation) => {
                let operand = self.check_expression(operation.operand(), None)?;
                let ty = match operand.ty() {
                    TypeKind::Need { ready, error } => match operation.propagation() {
                        HirAwaitPropagation::PreserveResult => TypeKind::Result {
                            ok: ready.clone(),
                            error: error.clone(),
                        },
                        HirAwaitPropagation::PropagateError => {
                            self.validate_propagation_error(
                                module,
                                owner,
                                expression.scope(),
                                PropagationOperator::Await,
                                error,
                            )?;
                            (**ready).clone()
                        }
                    },
                    TypeKind::ThreadHandle(value) => (**value).clone(),
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
                    CheckedExpressionResolution::Structural,
                ))
            }
            _ => return Ok(None),
        }
        .map(Some)
    }

    fn validate_propagation_error(
        &self,
        module: &HirModule,
        owner: ExprId,
        scope: ScopeId,
        operator: PropagationOperator,
        operand_error: &TypeKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let item_owner = enclosing_item(module, scope)?
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
        let item = module
            .resolve_item(item_owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let return_type = match item.kind() {
            HirItemKind::Function(function) => function.return_type(),
            HirItemKind::Flow(flow) => flow.result().authored_type(),
            _ => None,
        }
        .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
        let return_ty = self
            .types
            .get(&return_type)
            .ok_or(FinalSemanticAnalysisError::TypeResolutionFailed { owner: return_type })?;
        let TypeKind::Result {
            error: return_error,
            ..
        } = return_ty
        else {
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        };
        if return_error.accepts(operand_error) {
            return Ok(());
        }
        let operator_source = source_span_for_role(
            module,
            HirSourceQuery::Expr {
                owner,
                role: HirExprSourceRole::Operator,
            },
        )?;
        let return_source = source_span_for_role(
            module,
            HirSourceQuery::Type {
                owner: return_type,
                role: HirTypeSourceRole::Whole,
            },
        )?;
        Err(FinalSemanticAnalysisError::PropagationErrorMismatch {
            owner,
            operator,
            operand_error: Box::new(operand_error.clone()),
            return_error: Box::new(return_error.as_ref().clone()),
            operator_source,
            return_source,
        })
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
                let tail = self.check_expression(block.tail(), None)?;
                let ty = match block.kind() {
                    HirComputationBlockKind::Result => TypeKind::Result {
                        ok: Box::new(tail.ty().clone()),
                        error: Box::new(TypeKind::Unit),
                    },
                    HirComputationBlockKind::Task => TypeKind::Need {
                        ready: Box::new(tail.ty().clone()),
                        error: Box::new(TypeKind::Unit),
                    },
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
                let body = self.check_expression(closure.body(), body_expected)?;
                let ty = TypeKind::function(parameters, body.ty().clone());
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
                self.check_expression(pipe.left(), None)?;
                let right = self.check_expression(pipe.right(), expected)?;
                Ok(structural_expression(
                    right.ty().clone(),
                    right.type_selection(),
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
            HirExprKind::Placeholder(_) => expected
                .cloned()
                .map(|ty| structural_expression(ty, CheckedTypeSelection::Expected))
                .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner }),
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
        let (ty, resolution) = match target.ty() {
            TypeKind::ProjectNominal(target_nominal) => {
                let declaration = self
                    .symbols
                    .nominal(target_nominal.declaration())
                    .cloned()
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                let ProjectNominalBody::Struct { fields } = declaration.body() else {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
                };
                let field = fields
                    .iter()
                    .find(|field| field.name().as_str() == name.as_str())
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
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
                    .model(type_name)
                    .and_then(|model| model.projection(name.as_str()))
                    .map_or_else(
                        || super::CheckedSelectResolution::Field {
                            nominal: None,
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
                if receiver.ty() != &view_value_type() {
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
            view_value_type(),
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
            (Some((checked, projection)), None) => (
                checked,
                projection,
                PostfixBracketResolution::Index {
                    candidate: index_id,
                },
            ),
            (None, Some((checked, projection))) => (
                checked,
                projection,
                PostfixBracketResolution::Dialogue {
                    candidate: dialogue_id,
                },
            ),
            (Some(_), Some(_)) => {
                return Err(FinalSemanticAnalysisError::AmbiguousPostfixBracket { owner });
            }
            (None, None) => {
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
    ) -> Option<(CheckedExpression, CandidateSemanticProjection)> {
        let checkpoint = self.facts.begin_candidate_transaction();
        let result = self.check_expression(candidate, expected);
        let checked = result.ok();
        let projection = checked
            .as_ref()
            .map(|_| self.facts.capture_candidate_projection(checkpoint));
        self.facts.rollback_candidate_transaction(checkpoint);
        checked.zip(projection)
    }

    fn check_dialogue_content_application(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        application: &arcweft_lang_hir::dialogue_application::HirDialogueContentApplication,
        expected: Option<&TypeKind>,
    ) -> Result<CheckedExpression, FinalSemanticAnalysisError> {
        let (target_owner, configuration) =
            Self::dialogue_target_owner(module, owner, application)?;
        let target = module
            .resolve_expr(target_owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirExprKind::Path(path) = target.kind() else {
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
        };
        let path = path
            .as_resolved()
            .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
        let symbol = self
            .symbols
            .resolve_hir_symbol_target(
                module.key().path(),
                path,
                expression_span(module, target_owner)?,
            )
            .map_err(|_| FinalSemanticAnalysisError::ValueResolutionFailed {
                owner: target_owner,
            })?;
        let ResolvedProjectSymbol::Retained(character) = symbol else {
            return Err(FinalSemanticAnalysisError::ValueResolutionFailed {
                owner: target_owner,
            });
        };
        if character.family() != DeclarationIdentityFamily::Character {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let rich_text = RichTextAttributeChecker::check(module, application.content())
            .map_err(|_| FinalSemanticAnalysisError::RichTextSourceQuery { owner })?;
        if !rich_text.is_valid() {
            return Err(FinalSemanticAnalysisError::InvalidRichTextAttributes {
                owner,
                report: Box::new(rich_text),
            });
        }
        let item = CheckedProjectItem::try_new_retained(
            character.public_id().clone(),
            character.family(),
            character.owner(),
        )
        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let character_owner = character.owner();
        let character_ty = item.ty();
        self.facts.set_expression(
            target_owner,
            CheckedExpression::new(
                character_ty.clone(),
                CheckedTypeSelection::Inferred,
                EffectSet::new(),
                CheckedExpressionResolution::Value(CheckedValueResolution::ProjectItem(item)),
            ),
        );

        if let Some(configuration_owner) = configuration {
            self.publish_dialogue_configuration(
                owner,
                application,
                configuration_owner,
                character_owner,
                character_ty,
            )?;
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

        let ty = TypeKind::entity_ref(EntityKind::DialogueLine);
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
                character: character.owner(),
                rich_text: Box::new(rich_text),
            },
        ))
    }

    fn dialogue_target_owner(
        module: &HirModule,
        owner: ExprId,
        application: &arcweft_lang_hir::dialogue_application::HirDialogueContentApplication,
    ) -> Result<(ExprId, Option<ExprId>), FinalSemanticAnalysisError> {
        let configuration_owner = application.target();
        let target = module
            .resolve_expr(configuration_owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        match target.kind() {
            HirExprKind::Path(_) => Ok((configuration_owner, None)),
            HirExprKind::Call(call) if !application.coordinates().is_empty() => {
                let arcweft_lang_hir::expr::HirCallCallee::Value { value } = call.callee() else {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
                };
                Ok((*value, Some(configuration_owner)))
            }
            _ => Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner }),
        }
    }

    fn publish_dialogue_configuration(
        &mut self,
        owner: ExprId,
        application: &arcweft_lang_hir::dialogue_application::HirDialogueContentApplication,
        configuration_owner: ExprId,
        character_owner: ItemId,
        character_ty: TypeKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        self.facts.set_expression(
            configuration_owner,
            CheckedExpression::new(
                character_ty,
                CheckedTypeSelection::Inferred,
                EffectSet::new(),
                CheckedExpressionResolution::DialogueConfiguration {
                    character: character_owner,
                },
            ),
        );
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

fn view_value_type() -> TypeKind {
    TypeKind::Named("ViewValue".to_owned())
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
