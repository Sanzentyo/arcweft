//! Native numeric ranges and closed enum grammars for specified Style values.

use super::{
    Analyzer, AnalyzerExpressionContext, AnalyzerExpressionError, CheckedExpression,
    CheckedExpressionResolution, CheckedStyleOperand, CompileTimeStyleType, ExprId, HirExpr,
    HirExprKind, HirLiteral, HirModule, ViewSpecifiedValue, ViewStyleValueKind,
};
use crate::final_analysis::analyzer::style::grammar::StyleSourceGrammar;
use crate::{
    final_analysis::CheckedStyleNumber,
    types::{StyleNumericType, TypeKind},
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
            let (number, operand) = StyleSourceGrammar::number(module, owner, numeric)
                .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
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
        let value = StyleSourceGrammar::closed_variant(kind, name)
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        Ok(Some(Self::checked_style_value(value)))
    }
}
