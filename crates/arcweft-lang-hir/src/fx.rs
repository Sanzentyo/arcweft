//! Shared compiler policy for `#[fx] fn ... -> Fx` graph factories.

use arcweft_lang_syntax::expr::{Expr, Literal, UnaryOp};

/// Closed value accepted by an Fx default or `RichText` invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FxConst<'a> {
    expr: &'a Expr,
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

impl<'a> FxConst<'a> {
    /// Classifies a recursively closed constant expression.
    pub fn from_expr(expr: &'a Expr) -> Option<Self> {
        let kind = match expr {
            Expr::Literal(
                Literal::String(_)
                | Literal::Int(_)
                | Literal::Float { .. }
                | Literal::UnitNumber { .. }
                | Literal::Bool(_)
                | Literal::Duration { .. },
            ) => FxConstKind::Literal,
            Expr::Unary {
                op: UnaryOp::Neg,
                expr,
            } if matches!(
                expr.as_ref(),
                Expr::Literal(
                    Literal::Int(_)
                        | Literal::Float { .. }
                        | Literal::UnitNumber { .. }
                        | Literal::Duration { .. }
                )
            ) =>
            {
                FxConstKind::SignedNumber
            }
            Expr::ShortVariant(_) => FxConstKind::Selector,
            Expr::BracketSeq(items) if items.iter().all(|item| Self::from_expr(item).is_some()) => {
                FxConstKind::List
            }
            Expr::NumericBracketSeq(_) => FxConstKind::List,
            Expr::RecordLiteral(fields)
                if fields
                    .iter()
                    .all(|(_, value)| Self::from_expr(value).is_some()) =>
            {
                FxConstKind::Record
            }
            _ => return None,
        };
        Some(Self { expr, kind })
    }

    pub const fn expr(self) -> &'a Expr {
        self.expr
    }

    pub const fn kind(self) -> FxConstKind {
        self.kind
    }
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
    use super::{FxConst, FxConstructorKind};
    use arcweft_lang_syntax::expr::parse_expr;

    #[test]
    fn fx_constants_are_closed_and_exclude_character_literals() {
        for source in ["500ms", "-2px", "\"seed\"", ".glyph", "[1, 2]"] {
            let expr = parse_expr(source).expect("Fx constant parses");
            assert!(FxConst::from_expr(&expr).is_some(), "{source}");
        }
        let character = parse_expr("\"x\"c").expect("character literal parses");
        assert!(FxConst::from_expr(&character).is_none());
    }

    #[test]
    fn fx_constructor_inventory_is_closed() {
        assert_eq!(
            FxConstructorKind::from_member("text"),
            Some(FxConstructorKind::Text)
        );
        assert_eq!(FxConstructorKind::from_member("effect"), None);
    }
}
