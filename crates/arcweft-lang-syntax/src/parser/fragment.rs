use crate::ast::{flow::Stmt, items::Item};
use crate::expr::{Expr, ExprOp, parse_expr};
use crate::parser::recovery::ParseError;
use crate::source::ParsedSource;
use arcweft_source::SourceDocument;
use std::sync::Arc;

/// Fragment parser entrypoint used by REPL and LSP integrations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FragmentKind {
    Expression,
    Statements,
    Items,
}

/// Parser options shared by full documents and fragments.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ParseOptions {}

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

/// Parses a full Arcweft source document.
pub fn parse_document(source: impl Into<String>, options: ParseOptions) -> ParsedSource {
    super::parse_source_with_options(source, options)
}

/// Parses an immutable source document while preserving its exact identity.
pub fn parse_document_with_source(
    document: Arc<SourceDocument>,
    options: ParseOptions,
) -> ParsedSource {
    super::parse_source_document_with_options(document, options)
}

/// Parses a fragment using the same syntax components as full documents.
pub fn parse_fragment(source: &str, kind: FragmentKind, options: ParseOptions) -> ParsedFragment {
    if let Some(expected) = incomplete_syntax_expected_tokens(source) {
        return ParsedFragment::incomplete(expected);
    }
    if let Some(expected) = incomplete_boundary_expected_tokens(source) {
        return ParsedFragment::incomplete(expected);
    }
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

    fn incomplete(expected: Vec<ExpectedToken>) -> Self {
        Self {
            kind: None,
            completion: ParseCompletion::Incomplete { expected },
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

fn incomplete_boundary_expected_tokens(source: &str) -> Option<Vec<ExpectedToken>> {
    let mut stack = Vec::new();
    let mut chars = source.char_indices().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some((_, ch)) = chars.next() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            continue;
        }
        if ch == '/' && chars.peek().is_some_and(|(_, next)| *next == '/') {
            break;
        }
        match ch {
            '(' => stack.push(')'),
            '[' => stack.push(']'),
            '{' => stack.push('}'),
            ')' | ']' | '}' if stack.last().copied() == Some(ch) => {
                stack.pop();
            }
            ')' | ']' | '}' => return None,
            _ => {}
        }
    }

    if in_string {
        return Some(vec![ExpectedToken::new("\"")]);
    }
    (!stack.is_empty()).then(|| {
        stack
            .into_iter()
            .rev()
            .map(|token| ExpectedToken::new(token.to_string()))
            .collect()
    })
}

fn incomplete_syntax_expected_tokens(source: &str) -> Option<Vec<ExpectedToken>> {
    let trimmed = source.trim_end();
    let trimmed_start = trimmed.trim_start();
    if trimmed_start.is_empty() {
        return None;
    }
    if ends_at_block_intro_boundary(trimmed_start) {
        return Some(vec![ExpectedToken::new("{")]);
    }
    if ends_at_expression_boundary(trimmed_start) {
        return Some(vec![ExpectedToken::new("expression")]);
    }
    None
}

fn ends_at_block_intro_boundary(trimmed_start: &str) -> bool {
    trimmed_start == "else"
        || trimmed_start == "with"
        || trimmed_start.ends_with(" else")
        || trimmed_start.ends_with(" with")
}

fn ends_at_expression_boundary(trimmed_start: &str) -> bool {
    trimmed_start == "return"
        || trimmed_start == "try"
        || trimmed_start.ends_with('=')
        || trimmed_start.ends_with('(')
        || trimmed_start.ends_with('[')
        || trimmed_start.ends_with(',')
        || trimmed_start.ends_with('.')
        || trimmed_start.ends_with("= try")
        || trimmed_start.ends_with("(try")
        || trimmed_start.ends_with("[try")
        || trimmed_start.ends_with(", try")
        || MULTI_CHAR_CONTINUATION_OPS
            .iter()
            .any(|op| trimmed_start.ends_with(op.as_str()))
        || trimmed_start.ends_with('+')
        || trimmed_start.ends_with('-')
        || trimmed_start.ends_with('*')
        || trimmed_start.ends_with('/')
        || trimmed_start.ends_with('%')
        || trimmed_start.ends_with('<')
        || trimmed_start.ends_with('>')
        || trimmed_start.ends_with('!')
}

const MULTI_CHAR_CONTINUATION_OPS: &[ExprOp] = &[
    ExprOp::Eq,
    ExprOp::NotEq,
    ExprOp::Gte,
    ExprOp::Lte,
    ExprOp::And,
    ExprOp::Or,
    ExprOp::FatArrow,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragment_reports_incomplete_unclosed_expression_boundaries() {
        let parsed = parse_fragment(
            "wait(signal(@signal.ready).eq(true)",
            FragmentKind::Expression,
            ParseOptions::default(),
        );

        let ParseCompletion::Incomplete { expected } = parsed.completion() else {
            panic!("expected incomplete parse, got {:?}", parsed.completion());
        };
        assert_eq!(
            expected.iter().map(ExpectedToken::text).collect::<Vec<_>>(),
            [")"]
        );
    }

    #[test]
    fn fragment_reports_incomplete_nested_item_boundaries() {
        let parsed = parse_fragment(
            "fn repl() {\n    let frame = try observe()",
            FragmentKind::Items,
            ParseOptions::default(),
        );

        let ParseCompletion::Incomplete { expected } = parsed.completion() else {
            panic!("expected incomplete parse, got {:?}", parsed.completion());
        };
        assert_eq!(
            expected.iter().map(ExpectedToken::text).collect::<Vec<_>>(),
            ["}"]
        );
    }

    #[test]
    fn fragment_reports_incomplete_string_boundary() {
        let parsed = parse_fragment(
            "note(\"unterminated",
            FragmentKind::Expression,
            ParseOptions::default(),
        );

        let ParseCompletion::Incomplete { expected } = parsed.completion() else {
            panic!("expected incomplete parse, got {:?}", parsed.completion());
        };
        assert_eq!(
            expected.iter().map(ExpectedToken::text).collect::<Vec<_>>(),
            ["\""]
        );
    }

    #[test]
    fn fragment_reports_incomplete_expression_after_statement_heads() {
        for source in [
            "let value =",
            "return",
            "try",
            "wait(",
            "all([",
            "any([signal(@signal.ready),",
            "signal(@signal.ready).",
            "metric(@metric.score) >",
            "state(\"route.phase\").eq(",
            "try observe() with { error e =>",
        ] {
            let parsed = parse_fragment(source, FragmentKind::Statements, ParseOptions::default());

            let ParseCompletion::Incomplete { expected } = parsed.completion() else {
                panic!(
                    "expected incomplete parse for {source}, got {:?}",
                    parsed.completion()
                );
            };
            assert_eq!(
                expected.iter().map(ExpectedToken::text).collect::<Vec<_>>(),
                ["expression"]
            );
        }
    }

    #[test]
    fn fragment_reports_incomplete_block_introducers() {
        for source in [
            "try observe() with",
            "if diagnostics().has_error() { return \"bad\" } else",
        ] {
            let parsed = parse_fragment(source, FragmentKind::Statements, ParseOptions::default());

            let ParseCompletion::Incomplete { expected } = parsed.completion() else {
                panic!(
                    "expected incomplete parse for {source}, got {:?}",
                    parsed.completion()
                );
            };
            assert_eq!(
                expected.iter().map(ExpectedToken::text).collect::<Vec<_>>(),
                ["{"]
            );
        }
    }
}
