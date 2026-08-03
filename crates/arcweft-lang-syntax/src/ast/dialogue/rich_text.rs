use super::TextRange;
use crate::expr::Expr;
use crate::text::RichTextArgumentIssue;

/// Syntax-owned payload classification for one dialogue tag.
///
/// Calls and conditions are parsed once at the dialogue-text boundary instead
/// of being rediscovered from the raw attribute tail by later compiler layers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueTagPayload {
    /// Ordinary positional and named arguments.
    Arguments,
    /// Dedicated `[fx call(...)]` payload.
    FxCall(DialogueCallSurface),
    /// Dedicated `[call call(...)]` or `[! call(...)]` payload.
    DialogueCall(DialogueCallSurface),
    /// Dedicated `[if expression]` payload.
    Condition(DialogueExprSurface),
    /// A tag with no payload.
    None,
}

/// Lossless source surface for a call-shaped dialogue tag payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueCallSurface {
    expr: Expr,
    source: String,
    range: TextRange,
}

/// Lossless source surface for a condition-shaped dialogue tag payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueExprSurface {
    expr: Expr,
    source: String,
    range: TextRange,
}

/// One positional or named argument in a dialogue tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueTagArg {
    Positional {
        value: DialogueTagArgValueSurface,
        range: TextRange,
    },
    Named {
        name: String,
        name_range: TextRange,
        equals_range: TextRange,
        value: DialogueTagArgValueSurface,
        range: TextRange,
    },
    Invalid {
        source: String,
        range: TextRange,
        issue: RichTextArgumentIssue,
        issue_range: TextRange,
    },
}

/// Present or syntactically missing authored value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueTagArgValueSurface {
    Present(DialogueTagArgValue),
    Missing { range: TextRange },
}

/// Authored value and source range of a dialogue-tag argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueTagArgValue {
    source: String,
    value: String,
    token_range: TextRange,
    content_range: TextRange,
    quote: QuoteStyle,
    opening_quote_range: Option<TextRange>,
    closing_quote_range: Option<TextRange>,
}

/// Quote form retained for an authored tag-argument value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuoteStyle {
    Unquoted,
    Single,
    Double,
}

impl DialogueTagArg {
    /// Named argument key, or `None` for a positional argument.
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Positional { .. } | Self::Invalid { .. } => None,
            Self::Named { name, .. } => Some(name),
        }
    }

    /// Present authored argument value.
    pub const fn value(&self) -> Option<&DialogueTagArgValue> {
        match self {
            Self::Positional {
                value: DialogueTagArgValueSurface::Present(value),
                ..
            }
            | Self::Named {
                value: DialogueTagArgValueSurface::Present(value),
                ..
            } => Some(value),
            Self::Positional { .. } | Self::Named { .. } | Self::Invalid { .. } => None,
        }
    }

    /// Present-or-missing value surface for a valid-shaped argument.
    pub const fn value_surface(&self) -> Option<&DialogueTagArgValueSurface> {
        match self {
            Self::Positional { value, .. } | Self::Named { value, .. } => Some(value),
            Self::Invalid { .. } => None,
        }
    }

    /// Authored named-key range. Positional and invalid arguments have no key range.
    pub const fn name_range(&self) -> Option<TextRange> {
        match self {
            Self::Positional { .. } | Self::Invalid { .. } => None,
            Self::Named { name_range, .. } => Some(*name_range),
        }
    }

    /// Authored `=` range for a named argument.
    pub const fn equals_range(&self) -> Option<TextRange> {
        match self {
            Self::Named { equals_range, .. } => Some(*equals_range),
            Self::Positional { .. } | Self::Invalid { .. } => None,
        }
    }

    /// Full authored argument range.
    pub const fn range(&self) -> TextRange {
        match self {
            Self::Positional { range, .. }
            | Self::Named { range, .. }
            | Self::Invalid { range, .. } => *range,
        }
    }

    /// Exact invalid argument source, when recovery retained one.
    pub fn invalid_source(&self) -> Option<&str> {
        match self {
            Self::Invalid { source, .. } => Some(source),
            Self::Positional { .. } | Self::Named { .. } => None,
        }
    }

    /// Syntax issue attached to an invalid argument.
    pub const fn issue(&self) -> Option<&RichTextArgumentIssue> {
        match self {
            Self::Invalid { issue, .. } => Some(issue),
            Self::Positional { .. } | Self::Named { .. } => None,
        }
    }

    /// Exact source range responsible for an invalid argument issue.
    pub const fn issue_range(&self) -> Option<TextRange> {
        match self {
            Self::Invalid { issue_range, .. } => Some(*issue_range),
            Self::Positional { .. } | Self::Named { .. } => None,
        }
    }
}

impl DialogueTagArgValue {
    pub(crate) fn new(
        source: String,
        value: String,
        token_range: TextRange,
        content_range: TextRange,
        quote: QuoteStyle,
        opening_quote_range: Option<TextRange>,
        closing_quote_range: Option<TextRange>,
    ) -> Self {
        Self {
            source,
            value,
            token_range,
            content_range,
            quote,
            opening_quote_range,
            closing_quote_range,
        }
    }

    /// Exact authored value source, including quotes when present.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Decoded value with one matching outer quote pair removed.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Exact authored value range, including quotes.
    pub const fn range(&self) -> TextRange {
        self.token_range
    }

    /// Exact authored token range, including quotes.
    pub const fn token_range(&self) -> TextRange {
        self.token_range
    }

    /// Decoded-content range, excluding surrounding quotes.
    pub const fn content_range(&self) -> TextRange {
        self.content_range
    }

    /// Retained authored quote form.
    pub const fn quote(&self) -> QuoteStyle {
        self.quote
    }

    /// Opening quote range when the value was quoted.
    pub const fn opening_quote_range(&self) -> Option<TextRange> {
        self.opening_quote_range
    }

    /// Closing quote range when the value was quoted.
    pub const fn closing_quote_range(&self) -> Option<TextRange> {
        self.closing_quote_range
    }
}

impl DialogueTagArgValueSurface {
    /// Present value, or `None` for a missing `key=` surface.
    pub const fn present(&self) -> Option<&DialogueTagArgValue> {
        match self {
            Self::Present(value) => Some(value),
            Self::Missing { .. } => None,
        }
    }

    /// Zero-width insertion range for `key=`.
    pub const fn missing_range(&self) -> Option<TextRange> {
        match self {
            Self::Missing { range } => Some(*range),
            Self::Present(_) => None,
        }
    }
}

impl DialogueCallSurface {
    pub(crate) fn new(expr: Expr, source: String, range: TextRange) -> Self {
        Self {
            expr,
            source,
            range,
        }
    }

    /// Parsed call expression.
    pub const fn expr(&self) -> &Expr {
        &self.expr
    }

    /// Exact authored call source.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Exact authored payload range.
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl DialogueExprSurface {
    pub(crate) fn new(expr: Expr, source: String, range: TextRange) -> Self {
        Self {
            expr,
            source,
            range,
        }
    }

    /// Parsed condition expression.
    pub const fn expr(&self) -> &Expr {
        &self.expr
    }

    /// Exact authored condition source.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Exact authored payload range.
    pub const fn range(&self) -> TextRange {
        self.range
    }
}
