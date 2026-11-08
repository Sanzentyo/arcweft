//! Specified Style construction uses native values and source-bound operands.

use super::{
    Analyzer, AnalyzerExpressionContext, AnalyzerExpressionError, AnalyzerExpressionExpectation,
    CheckedCompileTimeCallee, CheckedExpression, CheckedExpressionResolution, CheckedTypeSelection,
    EffectSet, ExprId, FinalSemanticAnalysisError, HirCallArgument, HirExpr, HirExprKind,
    HirIntegerLiteral, HirLiteral, HirModule, HirStringLiteral, TypeKind,
};
use crate::{
    final_analysis::CheckedStyleOperand,
    types::{CompileTimeStyleType, StyleCallableId},
};
use arcweft_lang_hir::expr::HirCallCallee;
use arcweft_lang_hir::project::{HirSemanticPathOwnerId, HirSemanticPathRoot};
use arcweft_view::style::{
    ViewColorValue, ViewFontFamily, ViewFontFamilyList, ViewSpecifiedValue, ViewStyleValueKind,
    ViewSystemFontFamily,
};

#[path = "style/primitives.rs"]
mod primitives;

#[path = "style/composites.rs"]
mod composites;

impl Analyzer<'_, '_, '_> {
    pub(super) fn check_style_expression_kind(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expectation: &AnalyzerExpressionExpectation<'_>,
    ) -> Result<Option<CheckedExpression>, AnalyzerExpressionError> {
        let style_type = self
            .styles
            .value_kind(owner)
            .map(crate::types::CompileTimeStyleType::Value)
            .or_else(|| match expectation.contextual_shape() {
                Some(TypeKind::CompileTimeStyle(style)) => Some(*style),
                _ => None,
            });
        let Some(style_type) = style_type else {
            return Ok(None);
        };
        if expression.is_poisoned() {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::RecoveredOwner,
            ));
        }
        if let HirExprKind::Call(call) = expression.kind()
            && !matches!(
                call.explicit_type_application(),
                arcweft_lang_hir::expr::HirCallTypeApplication::Absent
            )
        {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        if let Some(checked) =
            self.check_style_construction(context, module, owner, expression, style_type)?
        {
            return Ok(Some(checked));
        }
        if let Some(checked) =
            self.check_style_composite(context, module, owner, expression, style_type)?
        {
            return Ok(Some(checked));
        }
        if let Some(checked) =
            self.check_style_primitive(context, module, owner, expression, style_type)?
        {
            return Ok(Some(checked));
        }
        let crate::types::CompileTimeStyleType::Value(expected) = style_type else {
            return Err(AnalyzerExpressionError::rejected(owner));
        };
        match expected {
            arcweft_view::style::ViewStyleValueKind::Color => {}
            _ => return Ok(None),
        }
        let HirExprKind::Call(call) = expression.kind() else {
            return Ok(None);
        };
        let HirCallCallee::Value { value: callee } = call.callee() else {
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
                CheckedExpression::value(
                    TypeKind::CompileTimeCallable(crate::types::CompileTimeCallableType::Style(
                        crate::types::StyleCallableId::Rgba,
                    )),
                    CheckedTypeSelection::Inferred,
                    EffectSet::new(),
                    CheckedExpressionResolution::CompileTimeCallee(
                        CheckedCompileTimeCallee::Style(crate::types::StyleCallableId::Rgba),
                    ),
                ),
            )
            .map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
            })?;
        Ok(Some(CheckedExpression::value(
            TypeKind::CompileTimeStyle(crate::types::CompileTimeStyleType::Value(
                arcweft_view::style::ViewStyleValueKind::Color,
            )),
            CheckedTypeSelection::Inferred,
            effects,
            CheckedExpressionResolution::StyleValue(
                arcweft_view::style::ViewSpecifiedValue::Color { value: color },
            ),
        )))
    }

    pub(super) fn check_style_construction(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: CompileTimeStyleType,
    ) -> Result<Option<CheckedExpression>, AnalyzerExpressionError> {
        let operand = match (expected, expression.kind()) {
            (CompileTimeStyleType::Token(kind), HirExprKind::Path(_) | HirExprKind::Select(_)) => {
                let token = self.style_token_reference(module, owner)?;
                if token.kind() != kind {
                    return Err(AnalyzerExpressionError::rejected(owner));
                }
                Some(CheckedStyleOperand::Token(token))
            }
            (
                CompileTimeStyleType::FontFamily,
                HirExprKind::Literal(HirLiteral::String(HirStringLiteral::Value(name))),
            ) => Some(CheckedStyleOperand::FontFamily(
                ViewFontFamily::named(name.as_ref())
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?,
            )),
            (CompileTimeStyleType::SystemFontFamily, HirExprKind::ShortVariant(name)) => {
                Some(CheckedStyleOperand::SystemFontFamily(
                    name.as_resolved()
                        .and_then(|name| ViewSystemFontFamily::from_source_name(name.as_str()))
                        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?,
                ))
            }
            (CompileTimeStyleType::SystemColor, HirExprKind::ShortVariant(name)) => {
                Some(CheckedStyleOperand::SystemColor(
                    name.as_resolved()
                        .and_then(|name| {
                            arcweft_presentation::appearance::SystemColor::from_source_name(
                                name.as_str(),
                            )
                        })
                        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?,
                ))
            }
            _ => None,
        };
        if let Some(operand) = operand {
            return Ok(Some(Self::checked_style_operand(operand)));
        }
        if let (
            CompileTimeStyleType::Value(ViewStyleValueKind::FontFamilyList),
            HirExprKind::BracketSequence(sequence),
        ) = (expected, expression.kind())
        {
            let mut families = Vec::with_capacity(sequence.elements().len());
            for child in sequence.elements() {
                let checked = self.evaluate_expression(
                    context,
                    *child,
                    Some(&TypeKind::CompileTimeStyle(
                        CompileTimeStyleType::FontFamily,
                    )),
                )?;
                let Some(CheckedExpressionResolution::StyleOperand(
                    CheckedStyleOperand::FontFamily(family),
                )) = checked.checked_resolution()
                else {
                    return Err(AnalyzerExpressionError::rejected(*child));
                };
                families.push(family.clone());
            }
            let value = ViewFontFamilyList::new(families)
                .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
            return Ok(Some(Self::checked_style_value(
                ViewSpecifiedValue::FontFamilyList { value },
            )));
        }
        let HirExprKind::Call(call) = expression.kind() else {
            return Ok(None);
        };
        let HirCallCallee::Value { value: callee } = call.callee() else {
            return Ok(None);
        };
        let (callable, argument_type) = match (
            expected,
            Self::direct_callee_name(module, *callee)?.as_deref(),
        ) {
            (CompileTimeStyleType::Value(kind), Some("token")) => {
                let [argument @ HirCallArgument::Positional { .. }] = call.arguments() else {
                    return Err(AnalyzerExpressionError::rejected(owner));
                };
                let actual = self.style_token_reference(module, argument.value())?.kind();
                if actual != kind
                    && !(kind == ViewStyleValueKind::BorderRadii
                        && actual == ViewStyleValueKind::Length)
                {
                    return Err(AnalyzerExpressionError::rejected(owner));
                }
                (StyleCallableId::Token, CompileTimeStyleType::Token(actual))
            }
            (CompileTimeStyleType::Value(ViewStyleValueKind::Color), Some("system_color")) => (
                StyleCallableId::SystemColor,
                CompileTimeStyleType::SystemColor,
            ),
            (CompileTimeStyleType::FontFamily, Some("system_font")) => (
                StyleCallableId::SystemFont,
                CompileTimeStyleType::SystemFontFamily,
            ),
            _ => return Ok(None),
        };
        let [argument @ HirCallArgument::Positional { .. }] = call.arguments() else {
            return Err(AnalyzerExpressionError::rejected(owner));
        };
        let argument = argument.value();
        let checked = self.evaluate_expression(
            context,
            argument,
            Some(&TypeKind::CompileTimeStyle(argument_type)),
        )?;
        let Some(CheckedExpressionResolution::StyleOperand(operand)) = checked.checked_resolution()
        else {
            return Err(AnalyzerExpressionError::rejected(argument));
        };
        if !checked.effects().is_empty() {
            return Err(AnalyzerExpressionError::rejected(argument));
        }
        let result = match (callable, operand) {
            (StyleCallableId::Token, CheckedStyleOperand::Token(token)) => {
                let value = ViewSpecifiedValue::Token {
                    token: token.id().clone(),
                    value_kind: token.kind(),
                };
                if expected == CompileTimeStyleType::Value(ViewStyleValueKind::BorderRadii)
                    && token.kind() == ViewStyleValueKind::Length
                {
                    let ViewSpecifiedValue::Length { value } =
                        self.resolve_style_constant(context, module, owner, value)?
                    else {
                        return Err(AnalyzerExpressionError::rejected(owner));
                    };
                    Self::checked_style_value(ViewSpecifiedValue::BorderRadii {
                        value: value.into(),
                    })
                } else {
                    Self::checked_style_value(value)
                }
            }
            (StyleCallableId::SystemColor, CheckedStyleOperand::SystemColor(role)) => {
                Self::checked_style_value(ViewSpecifiedValue::Color {
                    value: ViewColorValue::System { role: *role },
                })
            }
            (StyleCallableId::SystemFont, CheckedStyleOperand::SystemFontFamily(family)) => {
                Self::checked_style_operand(CheckedStyleOperand::FontFamily(
                    ViewFontFamily::system(*family),
                ))
            }
            _ => return Err(AnalyzerExpressionError::rejected(owner)),
        };
        self.publish_style_callee(*callee, callable)?;
        Ok(Some(result))
    }

    fn publish_style_callee(
        &mut self,
        callee: ExprId,
        callable: StyleCallableId,
    ) -> Result<(), AnalyzerExpressionError> {
        self.facts
            .publish_new_expression(
                callee,
                CheckedExpression::value(
                    TypeKind::CompileTimeCallable(crate::types::CompileTimeCallableType::Style(
                        callable,
                    )),
                    CheckedTypeSelection::Inferred,
                    EffectSet::new(),
                    CheckedExpressionResolution::CompileTimeCallee(
                        CheckedCompileTimeCallee::Style(callable),
                    ),
                ),
            )
            .map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
            })
    }

    fn style_token_reference(
        &self,
        module: &HirModule,
        owner: ExprId,
    ) -> Result<crate::final_analysis::CheckedStyleTokenReference, AnalyzerExpressionError> {
        let location = self
            .topology
            .semantic_path(HirSemanticPathOwnerId::Expression(owner))
            .map_err(|_| AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner))?
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        let HirSemanticPathRoot::Item { item, .. } = location.root() else {
            return Err(AnalyzerExpressionError::rejected(owner));
        };
        self.styles
            .token(module, *item, owner)
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))
    }

    fn resolve_style_constant(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        mut value: ViewSpecifiedValue,
    ) -> Result<ViewSpecifiedValue, AnalyzerExpressionError> {
        if !matches!(value, ViewSpecifiedValue::Token { .. }) {
            return Ok(value);
        }
        let location = self
            .topology
            .semantic_path(HirSemanticPathOwnerId::Expression(owner))
            .map_err(|_| AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner))?
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        let HirSemanticPathRoot::Item { item: style, .. } = location.root() else {
            return Err(AnalyzerExpressionError::rejected(owner));
        };
        let style = *style;
        let depth = self.styles.constant_depth();
        let result = (|| {
            while let ViewSpecifiedValue::Token { token, value_kind } = value {
                let initializer = self
                    .styles
                    .token_initializer(style, &token)
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                if !self.styles.enter_constant(initializer) {
                    return Err(AnalyzerExpressionError::rejected(owner));
                }
                // Evaluate original HIR with its declared type; no synthetic sheet or source reconstruction.
                let checked = self.evaluate_expression(
                    context,
                    initializer,
                    Some(&TypeKind::CompileTimeStyle(CompileTimeStyleType::Value(
                        value_kind,
                    ))),
                )?;
                let Some(CheckedExpressionResolution::StyleValue(next)) =
                    checked.checked_resolution()
                else {
                    return Err(AnalyzerExpressionError::rejected(initializer));
                };
                if !checked.effects().is_empty()
                    || next.kind() != value_kind
                    || module.resolve_expr(initializer).is_err()
                {
                    return Err(AnalyzerExpressionError::rejected(initializer));
                }
                value = next.clone();
            }
            Ok(value)
        })();
        self.styles.leave_constants(depth);
        result
    }

    fn checked_style_operand(operand: CheckedStyleOperand) -> CheckedExpression {
        CheckedExpression::value(
            TypeKind::CompileTimeStyle(operand.style_type()),
            CheckedTypeSelection::Expected,
            EffectSet::new(),
            CheckedExpressionResolution::StyleOperand(operand),
        )
    }

    fn checked_style_value(value: ViewSpecifiedValue) -> CheckedExpression {
        CheckedExpression::value(
            TypeKind::CompileTimeStyle(CompileTimeStyleType::Value(value.kind())),
            CheckedTypeSelection::Expected,
            EffectSet::new(),
            CheckedExpressionResolution::StyleValue(value),
        )
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
