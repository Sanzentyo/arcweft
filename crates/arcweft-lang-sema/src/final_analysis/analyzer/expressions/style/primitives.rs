//! Native numeric ranges and closed enum grammars for specified Style values.

use super::{
    Analyzer, AnalyzerExpressionContext, AnalyzerExpressionError, CheckedExpression,
    CheckedExpressionResolution, CheckedStyleOperand, CompileTimeStyleType, ExprId, HirExpr,
    HirExprKind, HirLiteral, HirModule, ViewSpecifiedValue, ViewStyleValueKind,
};
use crate::{
    final_analysis::CheckedStyleNumber,
    types::{StyleNumericType, TypeKind},
};
use arcweft_lang_hir::expr::HirUnaryOp;
use arcweft_view::style::{
    ViewAlignment, ViewBlendMode, ViewBoxAxisMode, ViewClip, ViewDisplay, ViewFlexDirection,
    ViewFlexWrap, ViewFontStyle, ViewMask, ViewOverflow, ViewPosition,
};

impl Analyzer<'_, '_, '_> {
    pub(super) fn check_style_primitive(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: CompileTimeStyleType,
    ) -> Result<Option<CheckedExpression>, AnalyzerExpressionError> {
        if let CompileTimeStyleType::NumericLiteral(kind) = expected {
            let HirExprKind::Literal(literal) = expression.kind() else {
                return Err(AnalyzerExpressionError::rejected(owner));
            };
            let number = CheckedStyleNumber::try_from((kind, literal))
                .map_err(|_| AnalyzerExpressionError::rejected(owner))?;
            return Ok(Some(Self::checked_style_operand(
                CheckedStyleOperand::Number(number),
            )));
        }
        let CompileTimeStyleType::Value(kind) = expected else {
            return Ok(None);
        };
        let numeric = if kind == ViewStyleValueKind::BorderRadii {
            Some(StyleNumericType::Length)
        } else {
            StyleNumericType::try_from(kind).ok()
        };
        if let Some(numeric) = numeric {
            let (literal, operand) = match expression.kind() {
                HirExprKind::Literal(literal) => (literal, None),
                HirExprKind::Unary(unary) if unary.operator() == HirUnaryOp::Negate => {
                    let HirExprKind::Literal(literal) = module
                        .resolve_expr(unary.operand())
                        .map_err(|_| AnalyzerExpressionError::rejected(owner))?
                        .kind()
                    else {
                        return Err(AnalyzerExpressionError::rejected(owner));
                    };
                    (literal, Some(unary.operand()))
                }
                _ => return Err(AnalyzerExpressionError::rejected(owner)),
            };
            let number = CheckedStyleNumber::try_from((numeric, literal))
                .map_err(|_| AnalyzerExpressionError::rejected(owner))?;
            let value = number
                .specified_value(operand.is_some())
                .map_err(|_| AnalyzerExpressionError::rejected(owner))?;
            if let Some(operand) = operand {
                let checked = self.evaluate_expression(
                    context,
                    operand,
                    Some(&TypeKind::CompileTimeStyle(
                        CompileTimeStyleType::NumericLiteral(numeric),
                    )),
                )?;
                if checked.checked_resolution()
                    != Some(&CheckedExpressionResolution::StyleOperand(
                        CheckedStyleOperand::Number(number),
                    ))
                    || !checked.effects().is_empty()
                {
                    return Err(AnalyzerExpressionError::rejected(operand));
                }
            }
            let value = if kind == ViewStyleValueKind::BorderRadii {
                let ViewSpecifiedValue::Length { value } = value else {
                    return Err(AnalyzerExpressionError::rejected(owner));
                };
                ViewSpecifiedValue::BorderRadii {
                    value: value.into(),
                }
            } else {
                value
            };
            return Ok(Some(Self::checked_style_value(value)));
        }
        if let (ViewStyleValueKind::Bool, HirExprKind::Literal(HirLiteral::Boolean(value))) =
            (kind, expression.kind())
        {
            return Ok(Some(Self::checked_style_value(ViewSpecifiedValue::Bool {
                value: *value,
            })));
        }
        if kind == ViewStyleValueKind::Bool {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        let HirExprKind::ShortVariant(name) = expression.kind() else {
            return Ok(None);
        };
        let name = name
            .as_resolved()
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?
            .as_str();
        let value = match kind {
            ViewStyleValueKind::BoxAxes => ViewBoxAxisMode::from_source_name(name)
                .map(|value| ViewSpecifiedValue::BoxAxes { value }),
            ViewStyleValueKind::FontStyle => ViewFontStyle::from_source_name(name)
                .map(|value| ViewSpecifiedValue::FontStyle { value }),
            ViewStyleValueKind::Display => ViewDisplay::from_source_name(name)
                .map(|value| ViewSpecifiedValue::Display { value }),
            ViewStyleValueKind::Position => ViewPosition::from_source_name(name)
                .map(|value| ViewSpecifiedValue::Position { value }),
            ViewStyleValueKind::Overflow => ViewOverflow::from_source_name(name)
                .map(|value| ViewSpecifiedValue::Overflow { value }),
            ViewStyleValueKind::FlexDirection => ViewFlexDirection::from_source_name(name)
                .map(|value| ViewSpecifiedValue::FlexDirection { value }),
            ViewStyleValueKind::FlexWrap => ViewFlexWrap::from_source_name(name)
                .map(|value| ViewSpecifiedValue::FlexWrap { value }),
            ViewStyleValueKind::Alignment => ViewAlignment::from_source_name(name)
                .map(|value| ViewSpecifiedValue::Alignment { value }),
            ViewStyleValueKind::Clip => {
                ViewClip::from_source_name(name).map(|value| ViewSpecifiedValue::Clip { value })
            }
            ViewStyleValueKind::Mask => {
                ViewMask::from_source_name(name).map(|value| ViewSpecifiedValue::Mask { value })
            }
            ViewStyleValueKind::BlendMode => ViewBlendMode::from_source_name(name)
                .map(|value| ViewSpecifiedValue::BlendMode { value }),
            _ => return Ok(None),
        }
        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        Ok(Some(Self::checked_style_value(value)))
    }
}
