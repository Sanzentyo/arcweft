//! Shared compiler resource policy for reusable rich-text decorations.

use arcweft_lang_syntax::expr::{Expr, Literal, UnaryOp};

/// Closed compile-time value family accepted by decoration declarations and
/// invocations.
///
/// Construction is intentionally centralized here so semantic validation and
/// runtime-plan lowering cannot disagree about literal families. Character
/// literals are excluded: decoration values are textual/numeric presentation
/// data rather than scalar code points.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecorationConst<'a> {
    expr: &'a Expr,
    kind: DecorationConstKind,
}

/// Semantic family of a closed decoration value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecorationConstKind {
    /// String, boolean, numeric, unit-number, or duration literal.
    Literal,
    /// Negated numeric, unit-number, or duration literal.
    SignedNumber,
    /// Dot-prefixed selector shorthand.
    Selector,
    /// One unqualified identifier token or declaration parameter reference.
    Identifier,
}

impl<'a> DecorationConst<'a> {
    /// Classifies an expression when it belongs to the closed decoration-value
    /// grammar.
    pub fn from_expr(expr: &'a Expr) -> Option<Self> {
        let kind = match expr {
            Expr::Literal(
                Literal::String(_)
                | Literal::Int(_)
                | Literal::Float { .. }
                | Literal::UnitNumber { .. }
                | Literal::Bool(_)
                | Literal::Duration { .. },
            ) => DecorationConstKind::Literal,
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
                DecorationConstKind::SignedNumber
            }
            Expr::ShortVariant(_) => DecorationConstKind::Selector,
            Expr::Path(path) if path.segments().len() == 1 => DecorationConstKind::Identifier,
            _ => return None,
        };
        Some(Self { expr, kind })
    }

    /// Original typed expression.
    pub const fn expr(self) -> &'a Expr {
        self.expr
    }

    /// Closed-value semantic family.
    pub const fn kind(self) -> DecorationConstKind {
        self.kind
    }
}

/// Deterministic limits applied while validating and expanding a decoration
/// composition graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecorationExpansionLimits {
    /// Maximum simultaneously nested declaration chain.
    pub max_depth: usize,
    /// Maximum declaration nodes visited during one expansion.
    pub max_visits: usize,
    /// Maximum concrete rich-text layers produced by one invocation.
    pub max_layers: usize,
}

/// Default limits shared by semantic validation and runtime-plan lowering.
///
/// Depth bounds recursion, visits bound repeated traversal through an acyclic
/// DAG, and layers bound the retained style output. They are compiler policy,
/// not a serialized runtime contract.
pub const DECORATION_EXPANSION_LIMITS: DecorationExpansionLimits = DecorationExpansionLimits {
    max_depth: 64,
    max_visits: 16_384,
    max_layers: 4_096,
};

/// Typed declaration-time rich-text builder inventory shared by semantic and
/// runtime-plan validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecorationBuilderKind {
    /// Emphasis span.
    Em,
    /// Strong-emphasis span.
    Strong,
    /// Foreground color span.
    Color,
    /// Font-family span.
    Font,
    /// Font-size span.
    Size,
    /// Closed presentation-style family.
    Style,
    /// Closed text-layout family.
    Layout,
    /// Closed visual-transform family.
    Transform,
    /// Registry-extensible visual-effect family.
    Effect,
    /// Nested decoration composition.
    Decorate,
}

/// Surface argument family owned by a decoration builder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecorationBuilderShape {
    /// Builder accepts no arguments.
    Empty,
    /// Builder accepts exactly one scalar value.
    Scalar,
    /// Builder requires a selector from a closed builtin inventory.
    ClosedSelector,
    /// Builder requires a registry-extensible selector.
    OpenSelector,
    /// Builder requires another declaration selector and bound arguments.
    NestedDecoration,
}

impl DecorationBuilderKind {
    /// Resolves a canonical builder name without accepting aliases or removed
    /// syntax.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "em" => Some(Self::Em),
            "strong" => Some(Self::Strong),
            "color" => Some(Self::Color),
            "font" => Some(Self::Font),
            "size" => Some(Self::Size),
            "style" => Some(Self::Style),
            "layout" => Some(Self::Layout),
            "transform" => Some(Self::Transform),
            "effect" => Some(Self::Effect),
            "decorate" => Some(Self::Decorate),
            _ => None,
        }
    }

    /// Argument family used by both semantic checking and expansion.
    pub const fn shape(self) -> DecorationBuilderShape {
        match self {
            Self::Em | Self::Strong => DecorationBuilderShape::Empty,
            Self::Color | Self::Font | Self::Size => DecorationBuilderShape::Scalar,
            Self::Style | Self::Layout | Self::Transform => DecorationBuilderShape::ClosedSelector,
            Self::Effect => DecorationBuilderShape::OpenSelector,
            Self::Decorate => DecorationBuilderShape::NestedDecoration,
        }
    }

    /// Whether a selector belongs to this builder's closed builtin family.
    /// Effect and nested-decoration selectors are intentionally open and are
    /// therefore always accepted here.
    pub fn supports_selector(self, selector: &str) -> bool {
        match self {
            Self::Style => matches!(
                selector,
                "italic" | "oblique" | "opacity" | "layer" | "meta" | "z_index"
            ),
            Self::Layout => matches!(
                selector,
                "horizontal_tb"
                    | "vertical_rl"
                    | "vertical_lr"
                    | "dir"
                    | "ruby_over"
                    | "ruby_under"
                    | "ruby_inter_character"
            ),
            Self::Transform => {
                matches!(selector, "offset" | "pos" | "rotate" | "scale" | "skew")
            }
            Self::Effect | Self::Decorate => true,
            Self::Em | Self::Strong | Self::Color | Self::Font | Self::Size => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DecorationBuilderKind, DecorationBuilderShape, DecorationConst};
    use arcweft_lang_syntax::expr::parse_expr;

    #[test]
    fn closed_selector_families_reject_unknown_spellings() {
        assert!(DecorationBuilderKind::Layout.supports_selector("vertical_rl"));
        assert!(!DecorationBuilderKind::Layout.supports_selector("vertcial_rl"));
        assert!(DecorationBuilderKind::Transform.supports_selector("offset"));
        assert!(!DecorationBuilderKind::Transform.supports_selector("offest"));
        assert_eq!(
            DecorationBuilderKind::Effect.shape(),
            DecorationBuilderShape::OpenSelector
        );
        assert!(DecorationBuilderKind::Effect.supports_selector("custom_registry_effect"));
    }

    #[test]
    fn decoration_constants_include_duration_but_exclude_character_literals() {
        for source in ["500ms", "-500ms", "2px", "\"seed\"", ".wave", "seed"] {
            let expr = parse_expr(source).expect("decoration constant parses");
            assert!(
                DecorationConst::from_expr(&expr).is_some(),
                "{source} should be a decoration constant"
            );
        }
        let character = parse_expr("\"x\"c").expect("character literal parses");
        assert!(DecorationConst::from_expr(&character).is_none());
    }
}
