//! Validation and lossless Rowan construction for staged grammar events.

use arcweft_source::SourceDocument;
use rowan::{GreenNode, GreenNodeBuilder};
use std::sync::Arc;
use thiserror::Error;

use super::budget::SyntaxParseStats;
use super::event::{
    ExpectedToken, PendingPatternProjection, PendingStartProjection, PendingSyntaxDiagnostic,
    PendingTypeProjection, SyntaxEvent,
};
use super::kinds::{IdentityClass, SyntaxKind, SyntaxRole};
use super::source_projection::{
    PendingPathProjection, PendingUseProjection, PendingVisibilityKind,
};
use crate::expressions::PendingExpressionProjection;
use crate::grammar::assertion_projection::PendingAssertionProjection;
use crate::grammar::attribute_projection::PendingOuterAttributeProjection;
use crate::grammar::callable_projection::PendingMethodReceiverProjection;
use crate::grammar::contract_projection::PendingFlowContractClauseProjection;
use crate::grammar::declaration_projection::{
    PendingCharacterDeclarationProjection, PendingDeclarationHeaderProjection,
    PendingLayerDeclarationProjection,
};
use crate::grammar::entry_projection::PendingEntryDeclarationProjection;
use crate::grammar::flow_projection::PendingFlowDeclarationProjection;
use crate::grammar::keyword_statement_projection::PendingKeywordStatementProjection;
use crate::grammar::style_projection::PendingStyleDeclarationProjection;
use crate::grammar::test_projection::PendingTestKindProjection;
use crate::grammar::view_projection::{PendingViewExportProjection, PendingViewFragmentProjection};
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
    projection: PendingStartProjection,
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

    pub(crate) fn expression_projection(&self) -> Option<&PendingExpressionProjection> {
        match &self.projection {
            PendingStartProjection::Expression(projection) => Some(projection),
            _ => None,
        }
    }

    pub(crate) const fn assertion_projection(&self) -> Option<PendingAssertionProjection> {
        match &self.projection {
            PendingStartProjection::Assertion(projection) => Some(*projection),
            _ => None,
        }
    }

    pub(crate) fn keyword_statement_projection(
        &self,
    ) -> Option<&PendingKeywordStatementProjection> {
        match &self.projection {
            PendingStartProjection::KeywordStatement(projection) => Some(projection),
            _ => None,
        }
    }

    pub(crate) fn type_projection(&self) -> Option<&PendingTypeProjection> {
        match &self.projection {
            PendingStartProjection::Type(projection) => Some(projection),
            _ => None,
        }
    }

    pub(crate) fn pattern_projection(&self) -> Option<&PendingPatternProjection> {
        match &self.projection {
            PendingStartProjection::Pattern(projection) => Some(projection),
            _ => None,
        }
    }

    pub(crate) fn path_projection(&self) -> Option<&PendingPathProjection> {
        match &self.projection {
            PendingStartProjection::Path(projection) => Some(projection),
            _ => None,
        }
    }

    pub(crate) fn use_projection(&self) -> Option<&PendingUseProjection> {
        match &self.projection {
            PendingStartProjection::Use(projection) => Some(projection),
            _ => None,
        }
    }

    pub(crate) const fn visibility_projection(&self) -> Option<PendingVisibilityKind> {
        match &self.projection {
            PendingStartProjection::Visibility(projection) => Some(*projection),
            _ => None,
        }
    }

    pub(crate) fn attribute_projection(&self) -> Option<&PendingOuterAttributeProjection> {
        match &self.projection {
            PendingStartProjection::Attribute(projection) => Some(projection),
            _ => None,
        }
    }

    pub(crate) fn character_projection(&self) -> Option<&PendingCharacterDeclarationProjection> {
        match &self.projection {
            PendingStartProjection::Character(projection) => Some(projection),
            _ => None,
        }
    }

    pub(crate) fn test_kind_projection(&self) -> Option<&PendingTestKindProjection> {
        match &self.projection {
            PendingStartProjection::TestKind(projection) => Some(projection),
            _ => None,
        }
    }

    pub(crate) fn layer_projection(&self) -> Option<&PendingLayerDeclarationProjection> {
        match &self.projection {
            PendingStartProjection::Layer(projection) => Some(projection),
            _ => None,
        }
    }

    pub(crate) fn entry_projection(&self) -> Option<&PendingEntryDeclarationProjection> {
        match &self.projection {
            PendingStartProjection::Entry(projection) => Some(projection),
            _ => None,
        }
    }

    pub(crate) fn style_projection(&self) -> Option<&PendingStyleDeclarationProjection> {
        match &self.projection {
            PendingStartProjection::Style(projection) => Some(projection),
            _ => None,
        }
    }

    pub(crate) fn method_receiver_projection(&self) -> Option<&PendingMethodReceiverProjection> {
        match &self.projection {
            PendingStartProjection::MethodReceiver(projection) => Some(projection),
            _ => None,
        }
    }

    pub(crate) fn contract_clause_projection(
        &self,
    ) -> Option<&PendingFlowContractClauseProjection> {
        match &self.projection {
            PendingStartProjection::ContractClause(projection) => Some(projection),
            _ => None,
        }
    }

    pub(crate) fn flow_declaration_projection(&self) -> Option<&PendingFlowDeclarationProjection> {
        match &self.projection {
            PendingStartProjection::FlowDeclaration(projection) => Some(projection),
            _ => None,
        }
    }

    pub(crate) fn view_export_projection(&self) -> Option<&PendingViewExportProjection> {
        match &self.projection {
            PendingStartProjection::ViewExport(projection) => Some(projection),
            _ => None,
        }
    }

    pub(crate) fn view_fragment_projection(&self) -> Option<&PendingViewFragmentProjection> {
        match &self.projection {
            PendingStartProjection::ViewFragment(projection) => Some(projection),
            _ => None,
        }
    }

    pub(crate) fn declaration_header_projection(
        &self,
    ) -> Option<&PendingDeclarationHeaderProjection> {
        match &self.projection {
            PendingStartProjection::DeclarationHeader(projection) => Some(projection),
            _ => None,
        }
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
    stats: SyntaxParseStats,
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

    pub(crate) const fn stats(&self) -> SyntaxParseStats {
        self.stats
    }

    /// Whether this complete grammar transaction contains recoverable syntax.
    pub(crate) fn has_recovery(&self) -> bool {
        !self.missing_tokens.is_empty()
            || !self.diagnostics.is_empty()
            || self.index.entries().iter().any(|entry| {
                entry.kind().is_missing_node()
                    || entry.kind().is_error_node()
                    || entry
                        .expression_projection()
                        .is_some_and(PendingExpressionProjection::has_recovery)
                    || entry
                        .assertion_projection()
                        .is_some_and(PendingAssertionProjection::has_recovery)
                    || entry
                        .keyword_statement_projection()
                        .is_some_and(PendingKeywordStatementProjection::has_recovery)
                    || entry
                        .attribute_projection()
                        .is_some_and(PendingOuterAttributeProjection::has_recovery)
                    || entry
                        .declaration_header_projection()
                        .is_some_and(PendingDeclarationHeaderProjection::has_recovery)
                    || entry
                        .character_projection()
                        .is_some_and(PendingCharacterDeclarationProjection::has_recovery)
                    || entry
                        .test_kind_projection()
                        .is_some_and(PendingTestKindProjection::has_recovery)
                    || entry
                        .layer_projection()
                        .is_some_and(PendingLayerDeclarationProjection::has_recovery)
                    || entry
                        .entry_projection()
                        .is_some_and(PendingEntryDeclarationProjection::has_recovery)
                    || entry
                        .flow_declaration_projection()
                        .is_some_and(PendingFlowDeclarationProjection::has_recovery)
                    || entry
                        .view_export_projection()
                        .is_some_and(PendingViewExportProjection::has_recovery)
                    || entry
                        .view_fragment_projection()
                        .is_some_and(PendingViewFragmentProjection::has_recovery)
            })
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
    #[error("expression node event {event} with kind {kind:?} has no required semantic projection")]
    MissingExpressionProjection { event: usize, kind: SyntaxKind },
    #[error("node event {event} with kind {kind:?} carries an incompatible expression projection")]
    InvalidExpressionProjection { event: usize, kind: SyntaxKind },
    #[error("AssertionStatement event {event} has no required semantic projection")]
    MissingAssertionProjection { event: usize },
    #[error("node event {event} with kind {kind:?} carries an assertion projection")]
    InvalidAssertionProjection { event: usize, kind: SyntaxKind },
    #[error(
        "keyword statement event {event} with kind {kind:?} has no required semantic projection"
    )]
    MissingKeywordStatementProjection { event: usize, kind: SyntaxKind },
    #[error(
        "node event {event} with kind {kind:?} carries an incompatible keyword-statement projection"
    )]
    InvalidKeywordStatementProjection { event: usize, kind: SyntaxKind },
    #[error("OuterAttribute event {event} has no required semantic projection")]
    MissingAttributeProjection { event: usize },
    #[error("node event {event} with kind {kind:?} carries an outer-attribute projection")]
    InvalidAttributeProjection { event: usize, kind: SyntaxKind },
    #[error("node event {event} with kind {kind:?} carries a declaration-header projection")]
    InvalidDeclarationHeaderProjection { event: usize, kind: SyntaxKind },
    #[error("Character item event {event} has no required semantic projection")]
    MissingCharacterProjection { event: usize },
    #[error("node event {event} with kind {kind:?} carries a Character projection")]
    InvalidCharacterProjection { event: usize, kind: SyntaxKind },
    #[error("Test item event {event} has no required adapter-kind projection")]
    MissingTestKindProjection { event: usize },
    #[error("node event {event} with kind {kind:?} carries a Test adapter-kind projection")]
    InvalidTestKindProjection { event: usize, kind: SyntaxKind },
    #[error("Layer item event {event} has no required semantic projection")]
    MissingLayerProjection { event: usize },
    #[error("node event {event} with kind {kind:?} carries a Layer projection")]
    InvalidLayerProjection { event: usize, kind: SyntaxKind },
    #[error("Entry item event {event} has no required semantic projection")]
    MissingEntryProjection { event: usize },
    #[error("node event {event} with kind {kind:?} carries an Entry projection")]
    InvalidEntryProjection { event: usize, kind: SyntaxKind },
    #[error("Style item event {event} has no required semantic projection")]
    MissingStyleProjection { event: usize },
    #[error("node event {event} with kind {kind:?} carries a Style projection")]
    InvalidStyleProjection { event: usize, kind: SyntaxKind },
    #[error("node event {event} with kind {kind:?} carries a method-receiver projection")]
    InvalidMethodReceiverProjection { event: usize, kind: SyntaxKind },
    #[error("Flow contract event {event} with kind {kind:?} has no source projection")]
    MissingFlowContractProjection { event: usize, kind: SyntaxKind },
    #[error(
        "node event {event} with kind {kind:?} carries an incompatible Flow contract projection"
    )]
    InvalidFlowContractProjection { event: usize, kind: SyntaxKind },
    #[error("Flow item event {event} has no required declaration projection")]
    MissingFlowDeclarationProjection { event: usize },
    #[error("node event {event} with kind {kind:?} carries a Flow declaration projection")]
    InvalidFlowDeclarationProjection { event: usize, kind: SyntaxKind },
    #[error("View export event {event} has no required structural projection")]
    MissingViewExportProjection { event: usize },
    #[error("node event {event} with kind {kind:?} carries a View export projection")]
    InvalidViewExportProjection { event: usize, kind: SyntaxKind },
    #[error("View fragment event {event} has no required structural projection")]
    MissingViewFragmentProjection { event: usize },
    #[error("node event {event} with kind {kind:?} carries a View fragment projection")]
    InvalidViewFragmentProjection { event: usize, kind: SyntaxKind },
    #[error("node event {event} marks non-group kind {kind:?} as a transparent expression group")]
    InvalidTransparentExpressionGroup { event: usize, kind: SyntaxKind },
    #[error("syntax limit {0:?} was exceeded while staging the grammar tree")]
    LimitExceeded(SyntaxLimit),
}

#[derive(Clone, Debug)]
struct OpenNode {
    path: Vec<u32>,
    next_element: u32,
    role: SyntaxRole,
    identity_bearing: bool,
    transparent_expression_group: bool,
}

/// Validates a complete event stream and constructs its lossless green tree.
pub(crate) fn build_grammar(
    document: &SourceDocument,
    events: &[SyntaxEvent],
) -> Result<GrammarBuild, GrammarBuildError> {
    let lexer_tokens = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                SyntaxEvent::Token { kind, .. } if *kind != SyntaxKind::EofToken
            )
        })
        .count();
    build_grammar_text(document.text(), events, lexer_tokens)
}

/// Builds one validated grammar tree from source-relative events.
pub(crate) fn build_grammar_text(
    source: &str,
    events: &[SyntaxEvent],
    lexer_tokens: usize,
) -> Result<GrammarBuild, GrammarBuildError> {
    validate_events(source, events)?;
    let budget =
        super::budget::validate_events(events).map_err(GrammarBuildError::LimitExceeded)?;
    let stats = budget.final_stats(source.len(), lexer_tokens, events.len());

    let mut builder = GreenNodeBuilder::new();
    let mut stack = Vec::<OpenNode>::new();
    let mut entries = Vec::new();
    let mut missing_tokens = Vec::new();
    let mut diagnostics = Vec::new();

    for (event_index, event) in events.iter().enumerate() {
        match event {
            SyntaxEvent::StartNode {
                kind,
                role,
                transparent_expression_group,
                ..
            } => {
                let (navigation_role, path) = begin_node(&mut stack, *kind, *role)?;
                builder.start_node(rowan::SyntaxKind(*kind as u16));
                if let Some(entry) = unattached_entry(event, navigation_role, &path) {
                    entries.push(entry);
                }
                stack.push(OpenNode {
                    path,
                    next_element: 0,
                    role: *role,
                    identity_bearing: kind.identity_class() == IdentityClass::IdentityBearing,
                    transparent_expression_group: *transparent_expression_group,
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
        stats,
    })
}

fn unattached_entry(
    event: &SyntaxEvent,
    role: SyntaxRole,
    path: &[u32],
) -> Option<UnattachedGrammarEntry> {
    let SyntaxEvent::StartNode {
        kind, projection, ..
    } = event
    else {
        return None;
    };
    (kind.identity_class() == IdentityClass::IdentityBearing).then(|| UnattachedGrammarEntry {
        kind: *kind,
        role,
        path: GrammarEventPath(path.into()),
        projection: projection.clone(),
    })
}

fn begin_node(
    stack: &mut [OpenNode],
    kind: SyntaxKind,
    role: SyntaxRole,
) -> Result<(SyntaxRole, Vec<u32>), GrammarBuildError> {
    // `DelimitedGroup` is a lossless structural wrapper, not a second
    // expression identity. Its outer role is therefore the navigation role of
    // the first identity-bearing semantic child below it. Reading the final
    // event here also observes Pratt rewrites that retarget a completed group.
    let navigation_role = if kind.is_expression() {
        stack
            .iter()
            .rposition(|open| open.identity_bearing)
            .and_then(|parent| {
                stack[parent + 1..]
                    .iter()
                    .find(|open| open.transparent_expression_group)
                    .map(|open| open.role)
            })
            .unwrap_or(role)
    } else {
        role
    };
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
    Ok((navigation_role, path))
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
            SyntaxEvent::StartNode { .. } => self.accept_start(event_index, event),
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

    fn accept_start(&mut self, event: usize, start: &SyntaxEvent) -> Result<(), GrammarBuildError> {
        let SyntaxEvent::StartNode {
            kind,
            role,
            transparent_expression_group,
            ..
        } = start
        else {
            unreachable!("accept_start receives only StartNode events")
        };
        let (kind, role, transparent_expression_group) =
            (*kind, *role, *transparent_expression_group);
        if kind.is_token() {
            return Err(GrammarBuildError::TokenUsedAsNode { event, kind });
        }
        if transparent_expression_group && kind != SyntaxKind::DelimitedGroup {
            return Err(GrammarBuildError::InvalidTransparentExpressionGroup { event, kind });
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
        validate_start_projections(event, kind, start)?;
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

fn validate_start_projections(
    event: usize,
    kind: SyntaxKind,
    start: &SyntaxEvent,
) -> Result<(), GrammarBuildError> {
    let SyntaxEvent::StartNode { projection, .. } = start else {
        unreachable!("projection validation receives only StartNode events")
    };
    validate_required_start_projection(event, kind, projection)?;
    validate_selected_start_projection(event, kind, projection)
}

fn validate_required_start_projection(
    event: usize,
    kind: SyntaxKind,
    projection: &PendingStartProjection,
) -> Result<(), GrammarBuildError> {
    let missing = match kind {
        kind if PendingExpressionProjection::kind_requires_projection(kind)
            && !matches!(projection, PendingStartProjection::Expression(_)) =>
        {
            Some(GrammarBuildError::MissingExpressionProjection { event, kind })
        }
        SyntaxKind::AssertionStatement
            if !matches!(projection, PendingStartProjection::Assertion(_)) =>
        {
            Some(GrammarBuildError::MissingAssertionProjection { event })
        }
        kind if PendingKeywordStatementProjection::kind_requires_projection(kind)
            && !matches!(projection, PendingStartProjection::KeywordStatement(_)) =>
        {
            Some(GrammarBuildError::MissingKeywordStatementProjection { event, kind })
        }
        SyntaxKind::InnerAttribute | SyntaxKind::OuterAttribute
            if !matches!(projection, PendingStartProjection::Attribute(_)) =>
        {
            Some(GrammarBuildError::MissingAttributeProjection { event })
        }
        SyntaxKind::CharacterDeclarationItem
            if !matches!(projection, PendingStartProjection::Character(_)) =>
        {
            Some(GrammarBuildError::MissingCharacterProjection { event })
        }
        SyntaxKind::TestItem if !matches!(projection, PendingStartProjection::TestKind(_)) => {
            Some(GrammarBuildError::MissingTestKindProjection { event })
        }
        SyntaxKind::LayerDeclarationItem
            if !matches!(projection, PendingStartProjection::Layer(_)) =>
        {
            Some(GrammarBuildError::MissingLayerProjection { event })
        }
        SyntaxKind::EntryDeclarationItem
            if !matches!(projection, PendingStartProjection::Entry(_)) =>
        {
            Some(GrammarBuildError::MissingEntryProjection { event })
        }
        SyntaxKind::StyleItem if !matches!(projection, PendingStartProjection::Style(_)) => {
            Some(GrammarBuildError::MissingStyleProjection { event })
        }
        kind if flow_only_contract_kind(kind)
            && !matches!(projection, PendingStartProjection::ContractClause(_)) =>
        {
            Some(GrammarBuildError::MissingFlowContractProjection { event, kind })
        }
        SyntaxKind::FlowItem
            if !matches!(projection, PendingStartProjection::FlowDeclaration(_)) =>
        {
            Some(GrammarBuildError::MissingFlowDeclarationProjection { event })
        }
        SyntaxKind::ViewExportDeclaration
            if !matches!(projection, PendingStartProjection::ViewExport(_)) =>
        {
            Some(GrammarBuildError::MissingViewExportProjection { event })
        }
        SyntaxKind::ViewFragment
            if !matches!(projection, PendingStartProjection::ViewFragment(_)) =>
        {
            Some(GrammarBuildError::MissingViewFragmentProjection { event })
        }
        _ => None,
    };
    match missing {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn validate_selected_start_projection(
    event: usize,
    kind: SyntaxKind,
    projection: &PendingStartProjection,
) -> Result<(), GrammarBuildError> {
    let invalid = match projection {
        PendingStartProjection::Expression(projection) if !projection.accepts_kind(kind) => {
            Some(GrammarBuildError::InvalidExpressionProjection { event, kind })
        }
        PendingStartProjection::Assertion(_) if kind != SyntaxKind::AssertionStatement => {
            Some(GrammarBuildError::InvalidAssertionProjection { event, kind })
        }
        PendingStartProjection::KeywordStatement(projection) if !projection.accepts_kind(kind) => {
            Some(GrammarBuildError::InvalidKeywordStatementProjection { event, kind })
        }
        PendingStartProjection::Attribute(_)
            if !matches!(
                kind,
                SyntaxKind::InnerAttribute | SyntaxKind::OuterAttribute
            ) =>
        {
            Some(GrammarBuildError::InvalidAttributeProjection { event, kind })
        }
        PendingStartProjection::DeclarationHeader(_)
            if !matches!(kind, SyntaxKind::DeclarationHeader | SyntaxKind::ProofItem) =>
        {
            Some(GrammarBuildError::InvalidDeclarationHeaderProjection { event, kind })
        }
        PendingStartProjection::Character(_) if kind != SyntaxKind::CharacterDeclarationItem => {
            Some(GrammarBuildError::InvalidCharacterProjection { event, kind })
        }
        PendingStartProjection::TestKind(_) if kind != SyntaxKind::TestItem => {
            Some(GrammarBuildError::InvalidTestKindProjection { event, kind })
        }
        PendingStartProjection::Layer(_) if kind != SyntaxKind::LayerDeclarationItem => {
            Some(GrammarBuildError::InvalidLayerProjection { event, kind })
        }
        PendingStartProjection::Entry(_) if kind != SyntaxKind::EntryDeclarationItem => {
            Some(GrammarBuildError::InvalidEntryProjection { event, kind })
        }
        PendingStartProjection::Style(_) if kind != SyntaxKind::StyleItem => {
            Some(GrammarBuildError::InvalidStyleProjection { event, kind })
        }
        PendingStartProjection::MethodReceiver(_) if kind != SyntaxKind::Parameter => {
            Some(GrammarBuildError::InvalidMethodReceiverProjection { event, kind })
        }
        PendingStartProjection::ContractClause(projection) if !projection.accepts_kind(kind) => {
            Some(GrammarBuildError::InvalidFlowContractProjection { event, kind })
        }
        PendingStartProjection::FlowDeclaration(_) if kind != SyntaxKind::FlowItem => {
            Some(GrammarBuildError::InvalidFlowDeclarationProjection { event, kind })
        }
        PendingStartProjection::ViewExport(_) if kind != SyntaxKind::ViewExportDeclaration => {
            Some(GrammarBuildError::InvalidViewExportProjection { event, kind })
        }
        PendingStartProjection::ViewFragment(_) if kind != SyntaxKind::ViewFragment => {
            Some(GrammarBuildError::InvalidViewFragmentProjection { event, kind })
        }
        _ => None,
    };
    match invalid {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

const fn flow_only_contract_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::InvariantClause
            | SyntaxKind::AssumeClause
            | SyntaxKind::ReadsClause
            | SyntaxKind::EffectsClause
            | SyntaxKind::NoEffectClause
            | SyntaxKind::ModifiesClause
            | SyntaxKind::DecreasesClause
    )
}

#[cfg(test)]
mod tests {
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

    use super::{GrammarBuildError, build_grammar, build_grammar_text};
    use crate::assertion::AssertionMode;
    use crate::expressions::{ExpressionProjection, PendingExpressionProjection};
    use crate::grammar::assertion_projection::PendingAssertionProjection;
    use crate::grammar::event::{
        ExpectedToken, PendingStartProjection, PendingSyntaxDiagnostic, SyntaxEvent,
    };
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
        let events = vec![
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
            SyntaxEvent::expression_start(
                SyntaxKind::PathExpression,
                SyntaxRole::Element(0),
                PendingExpressionProjection::new(ExpressionProjection::Path, Vec::new()),
            ),
            SyntaxEvent::token(SyntaxKind::IdentifierToken, SourceRange::new(0, 1)),
            SyntaxEvent::FinishNode,
            SyntaxEvent::token(SyntaxKind::EofToken, SourceRange::new(1, 1)),
            SyntaxEvent::FinishNode,
        ];

        let built = build_grammar_text("x", &events, 1).expect("source-free grammar build");
        assert_eq!(built.green().to_string(), "x");
        assert_eq!(built.events(), events);
    }

    #[test]
    fn expression_leaf_events_require_one_kind_consistent_projection() {
        let missing = [
            SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
            SyntaxEvent::start(SyntaxKind::LiteralExpression, SyntaxRole::Element(0)),
            SyntaxEvent::token(SyntaxKind::NumberToken, SourceRange::new(0, 1)),
            SyntaxEvent::FinishNode,
            SyntaxEvent::FinishNode,
        ];
        assert_eq!(
            build_grammar_text("1", &missing, 1).unwrap_err(),
            GrammarBuildError::MissingExpressionProjection {
                event: 1,
                kind: SyntaxKind::LiteralExpression,
            }
        );

        let wrong = [
            SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
            SyntaxEvent::expression_start(
                SyntaxKind::CallExpression,
                SyntaxRole::Element(0),
                PendingExpressionProjection::new(ExpressionProjection::Path, Vec::new()),
            ),
            SyntaxEvent::token(SyntaxKind::IdentifierToken, SourceRange::new(0, 1)),
            SyntaxEvent::FinishNode,
            SyntaxEvent::FinishNode,
        ];
        assert_eq!(
            build_grammar_text("x", &wrong, 1).unwrap_err(),
            GrammarBuildError::InvalidExpressionProjection {
                event: 1,
                kind: SyntaxKind::CallExpression,
            }
        );
    }

    #[test]
    fn assertion_events_require_one_projection_on_the_exact_statement_kind() {
        let missing = [
            SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
            SyntaxEvent::start(SyntaxKind::AssertionStatement, SyntaxRole::Statement(0)),
            SyntaxEvent::FinishNode,
            SyntaxEvent::FinishNode,
        ];
        assert_eq!(
            build_grammar_text("", &missing, 0).unwrap_err(),
            GrammarBuildError::MissingAssertionProjection { event: 1 }
        );

        let wrong = [
            SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
            SyntaxEvent::StartNode {
                kind: SyntaxKind::ExpressionList,
                role: SyntaxRole::Element(0),
                transparent_expression_group: false,
                projection: PendingStartProjection::Assertion(PendingAssertionProjection::new(
                    Some(AssertionMode::Check),
                )),
            },
            SyntaxEvent::FinishNode,
            SyntaxEvent::FinishNode,
        ];
        assert_eq!(
            build_grammar_text("", &wrong, 0).unwrap_err(),
            GrammarBuildError::InvalidAssertionProjection {
                event: 1,
                kind: SyntaxKind::ExpressionList,
            }
        );
    }

    #[test]
    fn transparent_expression_group_marker_rejects_every_other_node_kind() {
        let events = [
            SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
            SyntaxEvent::StartNode {
                kind: SyntaxKind::ExpressionList,
                role: SyntaxRole::Element(0),
                transparent_expression_group: true,
                projection: PendingStartProjection::None,
            },
            SyntaxEvent::FinishNode,
            SyntaxEvent::FinishNode,
        ];

        assert_eq!(
            build_grammar_text("", &events, 0).unwrap_err(),
            GrammarBuildError::InvalidTransparentExpressionGroup {
                event: 1,
                kind: SyntaxKind::ExpressionList,
            }
        );
    }
}
