//! Source-free fragment parsing and explicit source attachment products.

use core::marker::PhantomData;
use std::sync::Arc;

use arcweft_source::SourceSpan;

use crate::attachment::{
    AstKind, AstNode, ExpressionFragmentRootKind, PatternFragmentRootKind,
    StatementFragmentRootKind, SyntaxSnapshotData, SyntaxSnapshotId, TypeFragmentRootKind,
};
use crate::grammar::build::{GrammarBuild, GrammarBuildError};
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::SyntaxKind;

use super::document::{FragmentGrammar, parse_unbound_fragment};
use super::fragment::{ExpectedToken, ParseCompletion, ParseOptions};

/// One source-free grammar diagnostic with fragment-relative ranges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FragmentDiagnostic {
    code: &'static str,
    range: arcweft_source::SourceRange,
    related_range: Option<arcweft_source::SourceRange>,
    message: String,
}

impl FragmentDiagnostic {
    pub const fn code(&self) -> &'static str {
        self.code
    }

    pub const fn range(&self) -> arcweft_source::SourceRange {
        self.range
    }

    pub const fn related_range(&self) -> Option<arcweft_source::SourceRange> {
        self.related_range
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<&PendingSyntaxDiagnostic> for FragmentDiagnostic {
    fn from(diagnostic: &PendingSyntaxDiagnostic) -> Self {
        Self {
            code: diagnostic.code(),
            range: diagnostic.range(),
            related_range: diagnostic.related_range(),
            message: diagnostic.message().to_owned(),
        }
    }
}

/// Validated source-free event tree retained for later attachment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FragmentTree {
    events: Arc<[SyntaxEvent]>,
}

impl FragmentTree {
    fn from_build(build: &GrammarBuild) -> Self {
        Self {
            events: Arc::from(build.events()),
        }
    }

    pub(crate) fn events(&self) -> &[SyntaxEvent] {
        &self.events
    }

    fn primary_kind(&self) -> Option<SyntaxKind> {
        let mut depth = 0_usize;
        for event in self.events.iter() {
            match event {
                SyntaxEvent::StartNode { kind, .. } => {
                    if depth == 1 {
                        return Some(*kind);
                    }
                    depth += 1;
                }
                SyntaxEvent::FinishNode => depth = depth.saturating_sub(1),
                SyntaxEvent::Token { .. }
                | SyntaxEvent::MissingToken { .. }
                | SyntaxEvent::Diagnostic(_) => {}
            }
        }
        None
    }
}

mod sealed {
    pub trait Sealed {}
}

/// Sealed standalone-fragment family.
pub trait FragmentKind: sealed::Sealed + Copy + 'static {
    type AstKind: AstKind;
}

trait FragmentSpec: FragmentKind {
    const GRAMMAR: FragmentGrammar;
    const EXPECTED: &'static str;
}

macro_rules! define_fragment_kinds {
    ($($marker:ident => ($grammar:ident, $root:ident, $expected:literal)),+ $(,)?) => {
        $(
            #[derive(Clone, Copy, Debug, Eq, PartialEq)]
            pub struct $marker;

            impl sealed::Sealed for $marker {}

            impl FragmentSpec for $marker {
                const GRAMMAR: FragmentGrammar = FragmentGrammar::$grammar;
                const EXPECTED: &'static str = $expected;
            }

            impl FragmentKind for $marker {
                type AstKind = $root;
            }
        )+
    };
}

define_fragment_kinds! {
    ExpressionFragment => (Expression, ExpressionFragmentRootKind, "expression"),
    TypeFragment => (Type, TypeFragmentRootKind, "type"),
    PatternFragment => (Pattern, PatternFragmentRootKind, "pattern"),
    StatementFragment => (Statement, StatementFragmentRootKind, "statement"),
}

/// Source-free fragment parse product with no syntax identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnboundFragment<K: FragmentKind> {
    text: Arc<str>,
    tree: FragmentTree,
    diagnostics: Arc<[FragmentDiagnostic]>,
    completion: ParseCompletion,
    marker: PhantomData<fn() -> K>,
}

impl<K: FragmentKind> UnboundFragment<K> {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn diagnostics(&self) -> &[FragmentDiagnostic] {
        &self.diagnostics
    }

    pub const fn completion(&self) -> &ParseCompletion {
        &self.completion
    }

    pub(crate) fn into_parts(self) -> (Arc<str>, FragmentTree, ParseCompletion) {
        (self.text, self.tree, self.completion)
    }
}

impl UnboundFragment<StatementFragment> {
    pub fn root_kind(&self) -> Option<SyntaxKind> {
        self.tree.primary_kind()
    }
}

/// Fragment attached to one fresh database-owned syntax lineage.
#[derive(Clone)]
pub struct AttachedFragment<K: FragmentKind> {
    snapshot: Arc<SyntaxSnapshotData>,
    root: AstNode<K::AstKind>,
    whole: SourceSpan,
}

impl<K: FragmentKind> AttachedFragment<K> {
    pub(crate) fn new(
        snapshot: Arc<SyntaxSnapshotData>,
        root: AstNode<K::AstKind>,
        whole: SourceSpan,
    ) -> Self {
        Self {
            snapshot,
            root,
            whole,
        }
    }

    pub fn snapshot_id(&self) -> &SyntaxSnapshotId {
        self.snapshot.snapshot_id()
    }

    pub fn root(&self) -> AstNode<K::AstKind> {
        self.root.clone()
    }

    /// Exact target span occupied by the complete standalone fragment.
    ///
    /// For a parenthesized expression this includes the ID-less grouping while
    /// `root()` returns the inner identity-bearing semantic expression.
    pub const fn whole_source_span(&self) -> &SourceSpan {
        &self.whole
    }
}

pub fn parse_expression_fragment(
    text: &str,
    options: ParseOptions,
) -> UnboundFragment<ExpressionFragment> {
    parse_fragment_family(text, options)
}

pub fn parse_type_fragment(text: &str, options: ParseOptions) -> UnboundFragment<TypeFragment> {
    parse_fragment_family(text, options)
}

pub fn parse_pattern_fragment(
    text: &str,
    options: ParseOptions,
) -> UnboundFragment<PatternFragment> {
    parse_fragment_family(text, options)
}

pub fn parse_statement_fragment(
    text: &str,
    options: ParseOptions,
) -> UnboundFragment<StatementFragment> {
    parse_fragment_family(text, options)
}

fn parse_fragment_family<K: FragmentSpec>(
    text: &str,
    _options: ParseOptions,
) -> UnboundFragment<K> {
    let build = match parse_unbound_fragment(text, K::GRAMMAR) {
        Ok(build) => build,
        Err(GrammarBuildError::LimitExceeded(_)) => {
            return UnboundFragment {
                text: Arc::from(text),
                tree: FragmentTree {
                    events: Arc::from([]),
                },
                diagnostics: Arc::from([]),
                completion: ParseCompletion::Invalid,
                marker: PhantomData,
            };
        }
        Err(error) => panic!("fragment grammar violated losslessness: {error}"),
    };
    let diagnostics = build
        .diagnostics()
        .iter()
        .map(FragmentDiagnostic::from)
        .collect::<Vec<_>>()
        .into();
    let completion = completion::<K>(text, &build);
    UnboundFragment {
        text: Arc::from(text),
        tree: FragmentTree::from_build(&build),
        diagnostics,
        completion,
        marker: PhantomData,
    }
}

fn completion<K: FragmentSpec>(text: &str, build: &GrammarBuild) -> ParseCompletion {
    let unterminated_string = build.events().iter().any(|event| {
        matches!(
            event,
            SyntaxEvent::Token {
                kind: SyntaxKind::UnterminatedStringToken,
                ..
            }
        )
    });
    if unterminated_string {
        return ParseCompletion::Incomplete {
            expected: vec![ExpectedToken::new("\"")],
        };
    }

    let has_error = build
        .events()
        .iter()
        .any(|event| matches!(event, SyntaxEvent::StartNode { kind, .. } if kind.is_error_node()));
    let missing_family = build
        .events()
        .iter()
        .enumerate()
        .find_map(|(index, event)| {
            let expected = match event {
                SyntaxEvent::StartNode {
                    kind: SyntaxKind::MissingExpression,
                    ..
                } => Some("expression"),
                SyntaxEvent::StartNode {
                    kind: SyntaxKind::MissingType,
                    ..
                } => Some("type"),
                SyntaxEvent::StartNode {
                    kind: SyntaxKind::MissingPattern,
                    ..
                } => Some("pattern"),
                SyntaxEvent::StartNode {
                    kind: SyntaxKind::MissingName,
                    ..
                } if K::GRAMMAR == FragmentGrammar::Expression => Some("expression"),
                _ => None,
            }?;
            fragment_boundary_after_missing(&build.events()[index + 1..], text.len())
                .then_some(expected)
        });
    let missing_at_end = !build.missing_tokens().is_empty()
        && build
            .missing_tokens()
            .iter()
            .all(|missing| missing.at() == text.len());
    let diagnostics_at_end = build
        .diagnostics()
        .iter()
        .all(|diagnostic| diagnostic.range().start() == text.len());

    if !has_error && diagnostics_at_end && (missing_family.is_some() || missing_at_end) {
        let expected = missing_family
            .or_else(|| {
                build.missing_tokens().iter().find_map(|missing| {
                    missing
                        .expected()
                        .spelling()
                        .or_else(|| missing.expected().kind().token_display_name())
                })
            })
            .unwrap_or(K::EXPECTED)
            .to_owned();
        ParseCompletion::Incomplete {
            expected: vec![ExpectedToken::new(expected)],
        }
    } else if build.has_recovery() {
        ParseCompletion::Invalid
    } else {
        ParseCompletion::Complete
    }
}

fn fragment_boundary_after_missing(events: &[SyntaxEvent], source_len: usize) -> bool {
    events.iter().all(|event| match event {
        SyntaxEvent::Token { kind, range } => {
            *kind == SyntaxKind::EofToken
                || (matches!(
                    *kind,
                    SyntaxKind::WhitespaceToken
                        | SyntaxKind::NewlineToken
                        | SyntaxKind::CommentToken
                        | SyntaxKind::DocCommentToken
                ) && range.end() == source_len)
        }
        SyntaxEvent::MissingToken { at, .. } => *at == source_len,
        SyntaxEvent::Diagnostic(diagnostic) => diagnostic.range().start() == source_len,
        SyntaxEvent::StartNode { .. } | SyntaxEvent::FinishNode => true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expression_retains_exact_text_and_shared_grammar_completion() {
        let complete = parse_expression_fragment("call(value)", ParseOptions::default());
        assert_eq!(complete.text(), "call(value)");
        assert_eq!(complete.completion(), &ParseCompletion::Complete);

        let incomplete = parse_expression_fragment("call(value", ParseOptions::default());
        let ParseCompletion::Incomplete { expected } = incomplete.completion() else {
            panic!("expected incomplete expression fragment");
        };
        assert_eq!(
            expected.iter().map(ExpectedToken::text).collect::<Vec<_>>(),
            [")"]
        );
    }

    #[test]
    fn every_family_uses_one_exact_entrypoint() {
        assert_eq!(
            parse_type_fragment("Result<Value>", ParseOptions::default()).completion(),
            &ParseCompletion::Complete
        );
        assert_eq!(
            parse_pattern_fragment("Some(value)", ParseOptions::default()).completion(),
            &ParseCompletion::Complete
        );
        assert_eq!(
            parse_statement_fragment("let value = source;", ParseOptions::default()).completion(),
            &ParseCompletion::Complete
        );
        assert_eq!(
            parse_expression_fragment(")", ParseOptions::default()).completion(),
            &ParseCompletion::Invalid
        );
        assert_eq!(
            parse_expression_fragment("call(, value)", ParseOptions::default()).completion(),
            &ParseCompletion::Invalid
        );
    }

    #[test]
    fn statement_completion_comes_from_typed_grammar_recovery() {
        let source = "let value =";
        let parsed = parse_statement_fragment(source, ParseOptions::default());
        let ParseCompletion::Incomplete { expected } = parsed.completion() else {
            panic!(
                "expected incomplete statement fragment for {source:?}, got {:?}",
                parsed.completion()
            );
        };
        assert_eq!(
            expected.iter().map(ExpectedToken::text).collect::<Vec<_>>(),
            ["expression"]
        );
    }

    #[test]
    fn statement_exposes_its_primary_typed_kind_without_source_identity() {
        for (source, expected_kind) in [
            ("return", SyntaxKind::ReturnStatement),
            ("wait(", SyntaxKind::WaitStatement),
            ("if ready() { return value } else", SyntaxKind::IfStatement),
            ("try", SyntaxKind::ExpressionStatement),
        ] {
            let fragment = parse_statement_fragment(source, ParseOptions::default());
            assert_eq!(fragment.root_kind(), Some(expected_kind), "{source:?}");
        }
    }

    #[test]
    fn limit_failure_is_invalid_without_fabricating_an_attachable_tree() {
        let exact = format!("{}true", "!".repeat(64));
        assert_eq!(
            parse_expression_fragment(&exact, ParseOptions::default()).completion(),
            &ParseCompletion::Complete
        );

        let one_over = format!("{}true", "!".repeat(65));
        assert_eq!(
            parse_expression_fragment(&one_over, ParseOptions::default()).completion(),
            &ParseCompletion::Invalid
        );
    }
}
