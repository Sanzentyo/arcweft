//! Shared compiler policy for `#[fx] fn ... -> Fx` graph factories.

use crate::expr::{HirExprKind, HirRecordField, HirUnaryOp};
use crate::identity::ExprId;
use crate::leaf::{
    HirCharacterLiteral, HirDurationLiteral, HirFloatLiteral, HirIntegerLiteral, HirLiteral,
    HirNumericSequenceRecovery, HirStringLiteral, HirUnitNumberLiteral,
};
use crate::module::HirModule;

/// Closed value accepted by an Fx default or `RichText` invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FxConst {
    expr: ExprId,
    kind: FxConstKind,
}

/// Semantic family of a closed Fx value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FxConstKind {
    Literal,
    SignedNumber,
    Selector,
    List,
    Record,
}

/// Deterministic limits for validation and compile-time graph expansion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FxExpansionLimits {
    pub max_depth: usize,
    pub max_visits: usize,
    pub max_nodes: usize,
}

/// Compiler-wide Fx graph resource limits.
pub const FX_EXPANSION_LIMITS: FxExpansionLimits = FxExpansionLimits {
    max_depth: 64,
    max_visits: 16_384,
    max_nodes: 4_096,
};

/// Builtin graph constructor selected through the `Fx` namespace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FxConstructorKind {
    Style,
    Text,
    Color,
    Transform,
    Mask,
    Filter,
    Shader,
    Transition,
    Conditional,
    Stack,
}

impl FxConst {
    /// Classifies a recursively closed constant from one accepted HIR lease.
    pub fn from_expr(module: &HirModule, expr: ExprId) -> Option<Self> {
        let kind = classify_expr(module, expr)?;
        Some(Self { expr, kind })
    }

    /// Returns the exact qualified expression identity that was classified.
    pub const fn expr(self) -> ExprId {
        self.expr
    }

    pub const fn kind(self) -> FxConstKind {
        self.kind
    }
}

fn classify_expr(module: &HirModule, expr: ExprId) -> Option<FxConstKind> {
    let expression = module.resolve_expr(expr).ok()?;
    if expression.is_poisoned() {
        return None;
    }
    Some(match expression.kind() {
        HirExprKind::Literal(literal) if is_closed_literal(literal) => FxConstKind::Literal,
        HirExprKind::Unary(unary)
            if unary.operator() == HirUnaryOp::Negate
                && module
                    .resolve_expr(unary.operand())
                    .ok()
                    .is_some_and(|operand| {
                        !operand.is_poisoned()
                            && matches!(
                                operand.kind(),
                                HirExprKind::Literal(literal) if is_signed_numeric_literal(literal)
                            )
                    }) =>
        {
            FxConstKind::SignedNumber
        }
        HirExprKind::ShortVariant(name) if name.as_resolved().is_some() => FxConstKind::Selector,
        HirExprKind::BracketSequence(sequence)
            if sequence
                .elements()
                .iter()
                .all(|element| classify_expr(module, *element).is_some()) =>
        {
            FxConstKind::List
        }
        HirExprKind::NumericBracketSequence(sequence)
            if sequence.recovery() == &HirNumericSequenceRecovery::Complete =>
        {
            FxConstKind::List
        }
        HirExprKind::RecordLiteral(record)
            if record.fields().iter().all(|field| match field {
                HirRecordField::Explicit { value, .. } => classify_expr(module, *value).is_some(),
                HirRecordField::Shorthand { .. } | HirRecordField::Invalid { .. } => false,
            }) =>
        {
            FxConstKind::Record
        }
        _ => return None,
    })
}

const fn is_closed_literal(literal: &HirLiteral) -> bool {
    match literal {
        HirLiteral::String(HirStringLiteral::Value(_))
        | HirLiteral::Integer(HirIntegerLiteral::Value { .. })
        | HirLiteral::Float(HirFloatLiteral::Value { .. })
        | HirLiteral::UnitNumber(HirUnitNumberLiteral::Value { .. })
        | HirLiteral::Boolean(_)
        | HirLiteral::Duration(HirDurationLiteral::Value(_)) => true,
        HirLiteral::Character(HirCharacterLiteral::Value(_) | HirCharacterLiteral::Invalid(_))
        | HirLiteral::String(HirStringLiteral::Invalid(_))
        | HirLiteral::Integer(HirIntegerLiteral::Invalid(_))
        | HirLiteral::Float(HirFloatLiteral::Invalid(_))
        | HirLiteral::UnitNumber(HirUnitNumberLiteral::Invalid(_))
        | HirLiteral::Duration(HirDurationLiteral::Invalid(_)) => false,
    }
}

const fn is_signed_numeric_literal(literal: &HirLiteral) -> bool {
    matches!(
        literal,
        HirLiteral::Integer(HirIntegerLiteral::Value { .. })
            | HirLiteral::Float(HirFloatLiteral::Value { .. })
            | HirLiteral::UnitNumber(HirUnitNumberLiteral::Value { .. })
            | HirLiteral::Duration(HirDurationLiteral::Value(_))
    )
}

impl FxConstructorKind {
    /// Resolves the only canonical constructors in the `Fx` namespace.
    pub fn from_member(member: &str) -> Option<Self> {
        Some(match member {
            "style" => Self::Style,
            "text" => Self::Text,
            "color" => Self::Color,
            "transform" => Self::Transform,
            "mask" => Self::Mask,
            "filter" => Self::Filter,
            "shader" => Self::Shader,
            "transition" => Self::Transition,
            "conditional" => Self::Conditional,
            "stack" => Self::Stack,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::FxConstructorKind;

    #[test]
    fn fx_constructor_inventory_is_closed() {
        assert_eq!(
            FxConstructorKind::from_member("text"),
            Some(FxConstructorKind::Text)
        );
        assert_eq!(FxConstructorKind::from_member("effect"), None);
    }
}
