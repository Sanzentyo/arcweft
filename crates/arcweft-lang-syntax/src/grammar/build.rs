//! Validation and lossless Rowan construction for staged grammar events.

#![allow(
    dead_code,
    reason = "the shadow grammar remains crate-private until the atomic syntax switch"
)]

use arcweft_source::SourceDocument;
use rowan::{GreenNode, GreenNodeBuilder};
use std::sync::Arc;
use thiserror::Error;

use super::event::{ExpectedToken, PendingSyntaxDiagnostic, SyntaxEvent};
use super::kinds::{IdentityClass, SyntaxKind, SyntaxRole};
use crate::incremental::SyntaxLimit;

/// Element-index path from the green root to one identity-bearing node.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct GrammarEventPath(Box<[u32]>);

impl GrammarEventPath {
    pub(crate) const fn from_elements(elements: Box<[u32]>) -> Self {
        Self(elements)
    }

    pub(crate) fn elements(&self) -> &[u32] {
        &self.0
    }
}

/// One identity-bearing node awaiting snapshot identity attachment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UnattachedGrammarEntry {
    kind: SyntaxKind,
    role: SyntaxRole,
    path: GrammarEventPath,
}

impl UnattachedGrammarEntry {
    pub(crate) const fn kind(&self) -> SyntaxKind {
        self.kind
    }

    pub(crate) const fn role(&self) -> SyntaxRole {
        self.role
    }

    pub(crate) const fn path(&self) -> &GrammarEventPath {
        &self.path
    }
}

/// Identity-bearing nodes in grammar event order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct UnattachedGrammarIndex {
    entries: Box<[UnattachedGrammarEntry]>,
}

/// Zero-width missing token bound to its enclosing grammatical role node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MissingTokenSite {
    expected: ExpectedToken,
    at: usize,
    owner_path: GrammarEventPath,
}

impl MissingTokenSite {
    pub(crate) const fn expected(&self) -> ExpectedToken {
        self.expected
    }

    pub(crate) const fn at(&self) -> usize {
        self.at
    }

    pub(crate) const fn owner_path(&self) -> &GrammarEventPath {
        &self.owner_path
    }
}

impl UnattachedGrammarIndex {
    pub(crate) fn entries(&self) -> &[UnattachedGrammarEntry] {
        &self.entries
    }
}

/// Validated shadow grammar tree and its not-yet-attached metadata.
#[derive(Clone, Debug)]
pub(crate) struct GrammarBuild {
    events: Arc<[SyntaxEvent]>,
    green: GreenNode,
    index: UnattachedGrammarIndex,
    missing_tokens: Box<[MissingTokenSite]>,
    diagnostics: Box<[PendingSyntaxDiagnostic]>,
}

impl GrammarBuild {
    pub(crate) fn events(&self) -> &[SyntaxEvent] {
        &self.events
    }

    pub(crate) const fn green(&self) -> &GreenNode {
        &self.green
    }

    pub(crate) const fn index(&self) -> &UnattachedGrammarIndex {
        &self.index
    }

    pub(crate) fn missing_tokens(&self) -> &[MissingTokenSite] {
        &self.missing_tokens
    }

    pub(crate) fn diagnostics(&self) -> &[PendingSyntaxDiagnostic] {
        &self.diagnostics
    }

    /// Whether this complete grammar transaction contains recoverable syntax.
    pub(crate) fn has_recovery(&self) -> bool {
        !self.missing_tokens.is_empty()
            || !self.diagnostics.is_empty()
            || self
                .index
                .entries()
                .iter()
                .any(|entry| entry.kind().is_missing_node() || entry.kind().is_error_node())
    }
}

/// Structural reason an event stream cannot produce a lossless grammar tree.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum GrammarBuildError {
    #[error("grammar event {event} starts a token kind as a node")]
    TokenUsedAsNode { event: usize, kind: SyntaxKind },
    #[error("grammar event {event} emits a node kind as a token")]
    NodeUsedAsToken { event: usize, kind: SyntaxKind },
    #[error("grammar event {event} appears outside the source-file root")]
    EventOutsideRoot { event: usize },
    #[error("grammar event {event} starts a second root")]
    MultipleRoots { event: usize },
    #[error("grammar event {event} nests a second source-file root")]
    NestedSourceFile { event: usize },
    #[error("grammar event {event} gives the source-file root a non-root role")]
    InvalidRootRole { event: usize, role: SyntaxRole },
    #[error("grammar event {event} closes no open node")]
    UnexpectedFinish { event: usize },
    #[error("grammar stream ended with {open_nodes} unclosed nodes")]
    UnclosedNodes { open_nodes: usize },
    #[error("grammar stream has no source-file root")]
    MissingRoot,
    #[error("token event {event} has invalid range {start}..{end} for {source_len} bytes")]
    InvalidTokenRange {
        event: usize,
        start: usize,
        end: usize,
        source_len: usize,
    },
    #[error("token event {event} begins at {actual}, expected exact byte {expected}")]
    TokenCoverageMismatch {
        event: usize,
        expected: usize,
        actual: usize,
    },
    #[error("missing token event {event} is anchored at {actual}, expected {expected}")]
    MissingTokenOutOfOrder {
        event: usize,
        expected: usize,
        actual: usize,
    },
    #[error("EOF token event {event} must be zero-width at source byte {source_len}")]
    InvalidEofPlacement { event: usize, source_len: usize },
    #[error("diagnostic event {event} has an invalid range for {source_len} source bytes")]
    InvalidDiagnosticRange { event: usize, source_len: usize },
    #[error("grammar tokens cover {covered} of {source_len} source bytes")]
    IncompleteTokenCoverage { covered: usize, source_len: usize },
    #[error("grammar child count exceeds the u32 event-path domain")]
    ChildIndexExhausted,
    #[error("syntax limit {0:?} was exceeded while staging the grammar tree")]
    LimitExceeded(SyntaxLimit),
}

#[derive(Clone, Debug)]
struct OpenNode {
    path: Vec<u32>,
    next_element: u32,
}

/// Validates a complete event stream and constructs its lossless green tree.
pub(crate) fn build_grammar(
    document: &SourceDocument,
    events: &[SyntaxEvent],
) -> Result<GrammarBuild, GrammarBuildError> {
    build_grammar_text(document.text(), events)
}

/// Builds one validated grammar tree from source-relative events.
pub(crate) fn build_grammar_text(
    source: &str,
    events: &[SyntaxEvent],
) -> Result<GrammarBuild, GrammarBuildError> {
    validate_events(source, events)?;
    super::budget::validate_events(events).map_err(GrammarBuildError::LimitExceeded)?;

    let mut builder = GreenNodeBuilder::new();
    let mut stack = Vec::<OpenNode>::new();
    let mut entries = Vec::new();
    let mut missing_tokens = Vec::new();
    let mut diagnostics = Vec::new();

    for (event_index, event) in events.iter().enumerate() {
        match event {
            SyntaxEvent::StartNode { kind, role } => {
                let path = if let Some(parent) = stack.last_mut() {
                    let child = parent.next_element;
                    parent.next_element = parent
                        .next_element
                        .checked_add(1)
                        .ok_or(GrammarBuildError::ChildIndexExhausted)?;
                    let mut path = parent.path.clone();
                    path.push(child);
                    path
                } else {
                    Vec::new()
                };
                builder.start_node(rowan::SyntaxKind(*kind as u16));
                if kind.identity_class() == IdentityClass::IdentityBearing {
                    entries.push(UnattachedGrammarEntry {
                        kind: *kind,
                        role: *role,
                        path: GrammarEventPath(path.clone().into_boxed_slice()),
                    });
                }
                stack.push(OpenNode {
                    path,
                    next_element: 0,
                });
            }
            SyntaxEvent::Token { kind, range } => {
                let text = &source[range.as_range()];
                builder.token(rowan::SyntaxKind(*kind as u16), text);
                advance_element(&mut stack, event_index)?;
            }
            SyntaxEvent::MissingToken { expected, at } => {
                builder.token(rowan::SyntaxKind(SyntaxKind::MissingToken as u16), "");
                let owner_path = GrammarEventPath(
                    stack
                        .last()
                        .ok_or(GrammarBuildError::EventOutsideRoot { event: event_index })?
                        .path
                        .clone()
                        .into_boxed_slice(),
                );
                missing_tokens.push(MissingTokenSite {
                    expected: *expected,
                    at: *at,
                    owner_path,
                });
                advance_element(&mut stack, event_index)?;
            }
            SyntaxEvent::Diagnostic(diagnostic) => diagnostics.push(diagnostic.clone()),
            SyntaxEvent::FinishNode => {
                stack.pop();
                builder.finish_node();
            }
        }
    }

    Ok(GrammarBuild {
        events: Arc::from(events),
        green: builder.finish(),
        index: UnattachedGrammarIndex {
            entries: entries.into_boxed_slice(),
        },
        missing_tokens: missing_tokens.into_boxed_slice(),
        diagnostics: diagnostics.into_boxed_slice(),
    })
}

fn advance_element(stack: &mut [OpenNode], event: usize) -> Result<(), GrammarBuildError> {
    let node = stack
        .last_mut()
        .ok_or(GrammarBuildError::EventOutsideRoot { event })?;
    node.next_element = node
        .next_element
        .checked_add(1)
        .ok_or(GrammarBuildError::ChildIndexExhausted)?;
    Ok(())
}

fn validate_events(source: &str, events: &[SyntaxEvent]) -> Result<(), GrammarBuildError> {
    let mut validator = EventValidator::new(source);
    for (event_index, event) in events.iter().enumerate() {
        validator.accept(event_index, event)?;
    }
    validator.finish()
}

struct EventValidator<'a> {
    source: &'a str,
    depth: usize,
    covered: usize,
    root_seen: bool,
}

impl<'a> EventValidator<'a> {
    const fn new(source: &'a str) -> Self {
        Self {
            source,
            depth: 0,
            covered: 0,
            root_seen: false,
        }
    }

    fn accept(&mut self, event_index: usize, event: &SyntaxEvent) -> Result<(), GrammarBuildError> {
        match event {
            SyntaxEvent::StartNode { kind, role } => self.accept_start(event_index, *kind, *role),
            SyntaxEvent::Token { kind, range } => self.accept_token(event_index, *kind, *range),
            SyntaxEvent::MissingToken { at, .. } => {
                self.require_open_root(event_index)?;
                if *at != self.covered {
                    return Err(GrammarBuildError::MissingTokenOutOfOrder {
                        event: event_index,
                        expected: self.covered,
                        actual: *at,
                    });
                }
                Ok(())
            }
            SyntaxEvent::Diagnostic(diagnostic) => self.accept_diagnostic(event_index, diagnostic),
            SyntaxEvent::FinishNode => {
                if self.depth == 0 {
                    return Err(GrammarBuildError::UnexpectedFinish { event: event_index });
                }
                self.depth -= 1;
                Ok(())
            }
        }
    }

    fn accept_start(
        &mut self,
        event: usize,
        kind: SyntaxKind,
        role: SyntaxRole,
    ) -> Result<(), GrammarBuildError> {
        if kind.is_token() {
            return Err(GrammarBuildError::TokenUsedAsNode { event, kind });
        }
        if self.depth == 0 {
            if self.root_seen {
                return Err(GrammarBuildError::MultipleRoots { event });
            }
            if kind != SyntaxKind::SourceFile {
                return Err(GrammarBuildError::EventOutsideRoot { event });
            }
            if role != SyntaxRole::Root {
                return Err(GrammarBuildError::InvalidRootRole { event, role });
            }
            self.root_seen = true;
        } else if kind == SyntaxKind::SourceFile {
            return Err(GrammarBuildError::NestedSourceFile { event });
        }
        self.depth += 1;
        Ok(())
    }

    fn accept_token(
        &mut self,
        event: usize,
        kind: SyntaxKind,
        range: arcweft_source::SourceRange,
    ) -> Result<(), GrammarBuildError> {
        self.require_open_root(event)?;
        if !kind.is_token() || kind == SyntaxKind::MissingToken {
            return Err(GrammarBuildError::NodeUsedAsToken { event, kind });
        }
        let start = range.start();
        let end = range.end();
        if start > end
            || end > self.source.len()
            || (start == end && kind != SyntaxKind::EofToken)
            || !self.source.is_char_boundary(start)
            || !self.source.is_char_boundary(end)
        {
            return Err(GrammarBuildError::InvalidTokenRange {
                event,
                start,
                end,
                source_len: self.source.len(),
            });
        }
        if start != self.covered {
            return Err(GrammarBuildError::TokenCoverageMismatch {
                event,
                expected: self.covered,
                actual: start,
            });
        }
        if kind == SyntaxKind::EofToken && (start != self.source.len() || end != self.source.len())
        {
            return Err(GrammarBuildError::InvalidEofPlacement {
                event,
                source_len: self.source.len(),
            });
        }
        self.covered = end;
        Ok(())
    }

    fn accept_diagnostic(
        &self,
        event: usize,
        diagnostic: &PendingSyntaxDiagnostic,
    ) -> Result<(), GrammarBuildError> {
        self.require_open_root(event)?;
        if [Some(diagnostic.range()), diagnostic.related_range()]
            .into_iter()
            .flatten()
            .any(|range| {
                range.start() > range.end()
                    || range.end() > self.source.len()
                    || !self.source.is_char_boundary(range.start())
                    || !self.source.is_char_boundary(range.end())
            })
        {
            return Err(GrammarBuildError::InvalidDiagnosticRange {
                event,
                source_len: self.source.len(),
            });
        }
        Ok(())
    }

    fn finish(self) -> Result<(), GrammarBuildError> {
        if self.depth != 0 {
            return Err(GrammarBuildError::UnclosedNodes {
                open_nodes: self.depth,
            });
        }
        if !self.root_seen {
            return Err(GrammarBuildError::MissingRoot);
        }
        if self.covered != self.source.len() {
            return Err(GrammarBuildError::IncompleteTokenCoverage {
                covered: self.covered,
                source_len: self.source.len(),
            });
        }
        Ok(())
    }

    const fn require_open_root(&self, event: usize) -> Result<(), GrammarBuildError> {
        if self.depth == 0 {
            Err(GrammarBuildError::EventOutsideRoot { event })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

    use super::{GrammarBuildError, build_grammar, build_grammar_text};
    use crate::grammar::event::{ExpectedToken, PendingSyntaxDiagnostic, SyntaxEvent};
    use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

    fn document(text: &str) -> SourceDocument {
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/grammar-test").unwrap(),
            SourceName::path("grammar-test.arcw"),
            text,
        )
        .unwrap()
    }

    #[test]
    fn lossless_build_retains_utf8_trivia_and_identity_paths() {
        let document = document("proof π() {} // ok\r\n");
        let events = [
            SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
            SyntaxEvent::start(SyntaxKind::ProofItem, SyntaxRole::Element(0)),
            SyntaxEvent::token(SyntaxKind::KeywordToken, SourceRange::new(0, 5)),
            SyntaxEvent::token(SyntaxKind::WhitespaceToken, SourceRange::new(5, 6)),
            SyntaxEvent::start(SyntaxKind::NameDefinition, SyntaxRole::Name),
            SyntaxEvent::token(SyntaxKind::IdentifierToken, SourceRange::new(6, 8)),
            SyntaxEvent::FinishNode,
            SyntaxEvent::token(SyntaxKind::PunctuationToken, SourceRange::new(8, 10)),
            SyntaxEvent::token(SyntaxKind::WhitespaceToken, SourceRange::new(10, 11)),
            SyntaxEvent::start(SyntaxKind::ProofBlock, SyntaxRole::Body),
            SyntaxEvent::token(SyntaxKind::PunctuationToken, SourceRange::new(11, 13)),
            SyntaxEvent::FinishNode,
            SyntaxEvent::token(SyntaxKind::WhitespaceToken, SourceRange::new(13, 14)),
            SyntaxEvent::token(SyntaxKind::CommentToken, SourceRange::new(14, 19)),
            SyntaxEvent::token(SyntaxKind::NewlineToken, SourceRange::new(19, 21)),
            SyntaxEvent::FinishNode,
            SyntaxEvent::FinishNode,
        ];

        let built = build_grammar(&document, &events).unwrap();
        assert_eq!(built.green().to_string(), document.text());
        let entries = built.index().entries();
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].kind(), SyntaxKind::SourceFile);
        assert!(entries[0].path().elements().is_empty());
        assert_eq!(entries[1].path().elements(), &[0]);
        assert_eq!(entries[2].role(), SyntaxRole::Name);
        assert_eq!(entries[2].path().elements(), &[0, 2]);
        assert_eq!(entries[3].path().elements(), &[0, 5]);
    }

    #[test]
    fn missing_tokens_and_diagnostics_consume_no_source_bytes() {
        let document = document("proof p(");
        let expected = ExpectedToken::try_new(SyntaxKind::PunctuationToken).unwrap();
        assert_eq!(expected.kind(), SyntaxKind::PunctuationToken);
        let events = [
            SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
            SyntaxEvent::token(SyntaxKind::KeywordToken, SourceRange::new(0, 5)),
            SyntaxEvent::token(SyntaxKind::WhitespaceToken, SourceRange::new(5, 6)),
            SyntaxEvent::token(SyntaxKind::IdentifierToken, SourceRange::new(6, 7)),
            SyntaxEvent::token(SyntaxKind::PunctuationToken, SourceRange::new(7, 8)),
            SyntaxEvent::start(SyntaxKind::MissingTokenNode, SyntaxRole::CloseDelimiter),
            SyntaxEvent::MissingToken { expected, at: 8 },
            SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.proof.missing_parameter_close",
                SourceRange::new(8, 8),
                "missing `)`",
            )),
            SyntaxEvent::FinishNode,
            SyntaxEvent::FinishNode,
        ];

        let built = build_grammar(&document, &events).unwrap();
        assert_eq!(built.green().to_string(), document.text());
        assert_eq!(built.diagnostics().len(), 1);
        assert_eq!(built.missing_tokens().len(), 1);
        assert_eq!(built.missing_tokens()[0].expected(), expected);
        assert_eq!(built.missing_tokens()[0].at(), 8);
        assert_eq!(built.missing_tokens()[0].owner_path().elements(), &[4]);
        assert_eq!(
            built.diagnostics()[0].code(),
            "syntax.proof.missing_parameter_close"
        );
        assert_eq!(built.diagnostics()[0].range(), SourceRange::new(8, 8));
        assert_eq!(built.diagnostics()[0].message(), "missing `)`");
    }

    #[test]
    fn missing_token_alone_marks_the_complete_transaction_as_recovered() {
        let document = document("x");
        let expected = ExpectedToken::try_new(SyntaxKind::PunctuationToken).unwrap();
        let events = [
            SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
            SyntaxEvent::start(SyntaxKind::ParameterList, SyntaxRole::Element(0)),
            SyntaxEvent::token(SyntaxKind::IdentifierToken, SourceRange::new(0, 1)),
            SyntaxEvent::MissingToken { expected, at: 1 },
            SyntaxEvent::FinishNode,
            SyntaxEvent::FinishNode,
        ];

        let built = build_grammar(&document, &events).unwrap();
        assert!(built.diagnostics().is_empty());
        assert_eq!(built.missing_tokens().len(), 1);
        assert!(built.has_recovery());
    }

    #[test]
    fn diagnostic_validation_rejects_an_invalid_related_range() {
        let document = document("x");
        let events = [
            SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
            SyntaxEvent::token(SyntaxKind::IdentifierToken, SourceRange::new(0, 1)),
            SyntaxEvent::Diagnostic(
                PendingSyntaxDiagnostic::new(
                    "syntax.test.related_range",
                    SourceRange::new(0, 1),
                    "related range is invalid",
                )
                .with_related_range(SourceRange::new(2, 2)),
            ),
            SyntaxEvent::FinishNode,
        ];

        assert_eq!(
            build_grammar(&document, &events).unwrap_err(),
            GrammarBuildError::InvalidDiagnosticRange {
                event: 2,
                source_len: 1,
            }
        );
    }

    #[test]
    fn event_validation_rejects_gaps_unbalanced_nodes_and_token_kind_misuse() {
        let document = document("ab");
        let gap = [
            SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
            SyntaxEvent::token(SyntaxKind::IdentifierToken, SourceRange::new(1, 2)),
            SyntaxEvent::FinishNode,
        ];
        assert_eq!(
            build_grammar(&document, &gap).unwrap_err(),
            GrammarBuildError::TokenCoverageMismatch {
                event: 1,
                expected: 0,
                actual: 1,
            }
        );

        let unclosed = [
            SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
            SyntaxEvent::token(SyntaxKind::IdentifierToken, SourceRange::new(0, 2)),
        ];
        assert_eq!(
            build_grammar(&document, &unclosed).unwrap_err(),
            GrammarBuildError::UnclosedNodes { open_nodes: 1 }
        );

        let node_as_token = [
            SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
            SyntaxEvent::token(SyntaxKind::PathExpression, SourceRange::new(0, 2)),
            SyntaxEvent::FinishNode,
        ];
        assert_eq!(
            build_grammar(&document, &node_as_token).unwrap_err(),
            GrammarBuildError::NodeUsedAsToken {
                event: 1,
                kind: SyntaxKind::PathExpression,
            }
        );
    }

    #[test]
    fn source_free_build_retains_the_exact_validated_event_transaction() {
        let events = [
            SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
            SyntaxEvent::start(SyntaxKind::PathExpression, SyntaxRole::Element(0)),
            SyntaxEvent::token(SyntaxKind::IdentifierToken, SourceRange::new(0, 1)),
            SyntaxEvent::FinishNode,
            SyntaxEvent::token(SyntaxKind::EofToken, SourceRange::new(1, 1)),
            SyntaxEvent::FinishNode,
        ];

        let built = build_grammar_text("x", &events).expect("source-free grammar build");
        assert_eq!(built.green().to_string(), "x");
        assert_eq!(built.events(), events);
    }
}
