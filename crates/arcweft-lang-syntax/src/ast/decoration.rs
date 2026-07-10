use crate::expr::Expr;

use super::{
    common::{DocBlock, TextRange},
    items::Attribute,
};

/// Reusable, parameterized rich-text decoration declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecorationItem {
    doc: Option<DocBlock>,
    attrs: Vec<Attribute>,
    name: String,
    params: Vec<DecorationParam>,
    layers: Vec<DecorationLayer>,
    range: TextRange,
}

/// One required, defaulted, or rest parameter of a decoration declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecorationParam {
    name: String,
    default: Option<Expr>,
    default_source: Option<String>,
    rest: bool,
    range: TextRange,
    default_range: Option<TextRange>,
}

/// One rich-text builder expression in a decoration body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecorationLayer {
    expr: Expr,
    source: String,
    range: TextRange,
}

impl DecorationItem {
    pub(crate) const fn new(
        doc: Option<DocBlock>,
        attrs: Vec<Attribute>,
        name: String,
        params: Vec<DecorationParam>,
        layers: Vec<DecorationLayer>,
        range: TextRange,
    ) -> Self {
        Self {
            doc,
            attrs,
            name,
            params,
            layers,
            range,
        }
    }

    /// Markdown documentation attached to this declaration.
    pub const fn doc(&self) -> Option<&DocBlock> {
        self.doc.as_ref()
    }

    /// Outer attributes attached to this declaration.
    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }

    /// Module-local decoration name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Parameters in authored order.
    pub fn params(&self) -> &[DecorationParam] {
        &self.params
    }

    /// Builder layers in outer-to-inner authored order.
    pub fn layers(&self) -> &[DecorationLayer] {
        &self.layers
    }

    /// Full declaration source range.
    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl DecorationParam {
    pub(crate) const fn new(
        name: String,
        default: Option<Expr>,
        default_source: Option<String>,
        rest: bool,
        range: TextRange,
        default_range: Option<TextRange>,
    ) -> Self {
        Self {
            name,
            default,
            default_source,
            rest,
            range,
            default_range,
        }
    }

    /// Parameter name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Parsed default expression, when the parameter is optional.
    pub const fn default(&self) -> Option<&Expr> {
        self.default.as_ref()
    }

    /// Exact trimmed source of the default expression.
    pub fn default_source(&self) -> Option<&str> {
        self.default_source.as_deref()
    }

    /// Whether this final parameter collects undeclared named arguments.
    pub const fn is_rest(&self) -> bool {
        self.rest
    }

    /// Full parameter source range.
    pub const fn range(&self) -> TextRange {
        self.range
    }

    /// Source range of the default expression, excluding `=` and whitespace.
    pub const fn default_range(&self) -> Option<TextRange> {
        self.default_range
    }
}

impl DecorationLayer {
    pub(crate) const fn new(expr: Expr, source: String, range: TextRange) -> Self {
        Self {
            expr,
            source,
            range,
        }
    }

    /// Parsed builder-call expression.
    pub const fn expr(&self) -> &Expr {
        &self.expr
    }

    /// Exact trimmed builder-call source, without an optional semicolon.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Builder-call source range.
    pub const fn range(&self) -> TextRange {
        self.range
    }
}
