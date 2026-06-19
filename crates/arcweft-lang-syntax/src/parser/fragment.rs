use crate::ast::{flow::Stmt, items::Item};
use crate::expr::{Expr, parse_expr};
use crate::parser::recovery::ParseError;
use crate::source::ParsedSource;

/// Source dialect selected before parsing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SourceDialect {
    /// Ordinary Arcweft game source.
    #[default]
    Game,
    /// Agent controller source using Arcweft syntax plus top-level `agent`.
    Agent,
}

/// Fragment parser entrypoint used by REPL and LSP integrations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FragmentKind {
    Expression,
    Statements,
    Items,
}

/// Parser options shared by full documents and fragments.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ParseOptions {
    pub source_dialect: SourceDialect,
}

/// Completion state for an interactive parse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseCompletion {
    Complete,
    Incomplete { expected: Vec<ExpectedToken> },
    Invalid,
}

/// A syntax token or fragment expected at the parse boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedToken {
    text: String,
}

/// Parsed fragment payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParsedFragmentKind {
    Expression(Expr),
    Statements(Vec<Stmt>),
    Items(Vec<Item>),
}

/// Result of parsing a source fragment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedFragment {
    kind: Option<ParsedFragmentKind>,
    completion: ParseCompletion,
    errors: Vec<ParseError>,
}

/// Parses a full source document using the selected dialect.
pub fn parse_document(source: impl Into<String>, options: ParseOptions) -> ParsedSource {
    super::parse_source_with_options(source, options)
}

/// Parses a fragment using the same syntax components as full documents.
pub fn parse_fragment(source: &str, kind: FragmentKind, options: ParseOptions) -> ParsedFragment {
    match kind {
        FragmentKind::Expression => parse_expr(source).map_or_else(
            |_| ParsedFragment::invalid(),
            |expr| ParsedFragment::complete(ParsedFragmentKind::Expression(expr)),
        ),
        FragmentKind::Statements => ParsedFragment::complete(ParsedFragmentKind::Statements(
            super::control_flow::parse_stmt_lines(source),
        )),
        FragmentKind::Items => {
            let parsed = parse_document(source, options);
            let errors = parsed.errors().to_vec();
            let items = parsed.typed_tree().items().to_vec();
            if errors.is_empty() {
                ParsedFragment::complete(ParsedFragmentKind::Items(items))
            } else {
                ParsedFragment {
                    kind: Some(ParsedFragmentKind::Items(items)),
                    completion: ParseCompletion::Invalid,
                    errors,
                }
            }
        }
    }
}

impl ExpectedToken {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

impl ParsedFragment {
    fn complete(kind: ParsedFragmentKind) -> Self {
        Self {
            kind: Some(kind),
            completion: ParseCompletion::Complete,
            errors: Vec::new(),
        }
    }

    fn invalid() -> Self {
        Self {
            kind: None,
            completion: ParseCompletion::Invalid,
            errors: Vec::new(),
        }
    }

    pub const fn kind(&self) -> Option<&ParsedFragmentKind> {
        self.kind.as_ref()
    }

    pub const fn completion(&self) -> &ParseCompletion {
        &self.completion
    }

    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }
}
