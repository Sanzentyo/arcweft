//! A checked native environment test bound to its original HIR clause.

use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::{ExprId, ItemId},
    item::{
        HirItemKind, HirStyleBodyItem, HirStyleEnvironmentComparison, HirStyleEnvironmentField,
    },
    leaf::{HirLiteral, HirPathRoot, HirPathSegment, HirUnitNumberLiteral, HirUnitNumberUnit},
    module::HirModule,
    source_index::HirStyleBodyPath,
};
use arcweft_presentation::appearance::{ColorScheme, ContrastPreference, TextScaleMilli};
use arcweft_view::style::{ViewEnvironmentTest, ViewTextScaleComparison};

use super::CheckedStyleNumber;
use crate::types::StyleNumericType;

/// Source-issued condition operand; it carries no runtime or product source ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedStyleEnvironmentTest {
    style: ItemId,
    path: HirStyleBodyPath,
    environment: u32,
    clause: u32,
    value: ExprId,
    test: ViewEnvironmentTest,
}

impl CheckedStyleEnvironmentTest {
    pub(in crate::final_analysis) fn from_source(
        module: &HirModule,
        style: ItemId,
        path: HirStyleBodyPath,
        environment: u32,
        clause: u32,
    ) -> Option<Self> {
        let HirItemKind::Style(item) = module.resolve_item(style).ok()?.kind() else {
            return None;
        };
        let mut body = item.body();
        for ordinal in path.ordinals() {
            let HirStyleBodyItem::Environment(wrapper) = body.get(*ordinal as usize)? else {
                return None;
            };
            body = wrapper.body();
        }
        let HirStyleBodyItem::Environment(wrapper) = body.get(environment as usize)? else {
            return None;
        };
        let original = wrapper.clauses().get(clause as usize)?;
        let expression = module.resolve_expr(original.value()).ok()?;
        let test = match original.field() {
            HirStyleEnvironmentField::ColorScheme | HirStyleEnvironmentField::Contrast => {
                if original.comparison() != HirStyleEnvironmentComparison::Equal {
                    return None;
                }
                let HirExprKind::Path(path) = expression.kind() else {
                    return None;
                };
                let path = path.as_resolved()?;
                let [HirPathSegment::Identifier(name)] = path.segments() else {
                    return None;
                };
                if path.root() != HirPathRoot::ImplicitCrate {
                    return None;
                }
                match (original.field(), name.as_str()) {
                    (HirStyleEnvironmentField::ColorScheme, "light") => {
                        ViewEnvironmentTest::ColorScheme(ColorScheme::Light)
                    }
                    (HirStyleEnvironmentField::ColorScheme, "dark") => {
                        ViewEnvironmentTest::ColorScheme(ColorScheme::Dark)
                    }
                    (HirStyleEnvironmentField::Contrast, "standard") => {
                        ViewEnvironmentTest::Contrast(ContrastPreference::Standard)
                    }
                    (HirStyleEnvironmentField::Contrast, "more") => {
                        ViewEnvironmentTest::Contrast(ContrastPreference::More)
                    }
                    _ => return None,
                }
            }
            HirStyleEnvironmentField::ReducedMotion => {
                if original.comparison() != HirStyleEnvironmentComparison::Equal {
                    return None;
                }
                let HirExprKind::Literal(HirLiteral::Boolean(value)) = expression.kind() else {
                    return None;
                };
                ViewEnvironmentTest::ReducedMotion(*value)
            }
            HirStyleEnvironmentField::TextScale => {
                let HirExprKind::Literal(literal) = expression.kind() else {
                    return None;
                };
                let magnitude = match literal {
                    HirLiteral::UnitNumber(HirUnitNumberLiteral::Value {
                        decimal,
                        unit: HirUnitNumberUnit::Percent,
                    }) => u32::try_from(decimal.to_scaled_i64(1, 1, false).ok()?).ok()?,
                    _ => CheckedStyleNumber::try_from((StyleNumericType::Scalar, literal))
                        .ok()?
                        .magnitude(),
                };
                let value = TextScaleMilli::try_from(magnitude).ok()?;
                let comparison = match original.comparison() {
                    HirStyleEnvironmentComparison::Equal => ViewTextScaleComparison::Equal,
                    HirStyleEnvironmentComparison::NotEqual => ViewTextScaleComparison::NotEqual,
                    HirStyleEnvironmentComparison::Less => ViewTextScaleComparison::Less,
                    HirStyleEnvironmentComparison::LessOrEqual => {
                        ViewTextScaleComparison::LessOrEqual
                    }
                    HirStyleEnvironmentComparison::Greater => ViewTextScaleComparison::Greater,
                    HirStyleEnvironmentComparison::GreaterOrEqual => {
                        ViewTextScaleComparison::GreaterOrEqual
                    }
                    HirStyleEnvironmentComparison::Recovered(_) => return None,
                };
                ViewEnvironmentTest::TextScale { comparison, value }
            }
            HirStyleEnvironmentField::Recovered(_) => return None,
        };
        Some(Self {
            style,
            path,
            environment,
            clause,
            value: original.value(),
            test,
        })
    }

    pub const fn test(&self) -> ViewEnvironmentTest {
        self.test
    }
    pub const fn value(&self) -> ExprId {
        self.value
    }

    /// Checks the exact authored body and clause, rather than just equal values.
    pub fn matches_clause(
        &self,
        style: ItemId,
        path: &HirStyleBodyPath,
        environment: u32,
        clause: u32,
        value: ExprId,
    ) -> bool {
        self.style == style
            && &self.path == path
            && self.environment == environment
            && self.clause == clause
            && self.value == value
    }

    pub(in crate::final_analysis) fn matches_source(
        &self,
        module: &HirModule,
        value: ExprId,
    ) -> bool {
        self.value == value
            && Self::from_source(
                module,
                self.style,
                self.path.clone(),
                self.environment,
                self.clause,
            )
            .as_ref()
                == Some(self)
    }
}
