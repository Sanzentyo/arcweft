//! Shared token cursor and event sink for the staged document grammar.

#![allow(
    dead_code,
    reason = "the shadow document parser remains private until the atomic syntax switch"
)]

use std::sync::Arc;

use arcweft_source::SourceRange;

use crate::expressions::{
    CandidateNodeIndex, PendingCandidateGraph, PendingCandidateNode, PendingCandidateSemantic,
    PendingExpressionProjection,
};
use crate::grammar::assertion_projection::PendingAssertionProjection;
use crate::grammar::attribute_projection::PendingOuterAttributeProjection;
use crate::grammar::budget::{GrammarBudget, GrammarParserDepths};
use crate::grammar::callable_projection::PendingMethodReceiverProjection;
use crate::grammar::contract_projection::PendingFlowContractClauseProjection;
use crate::grammar::declaration_projection::{
    PendingCharacterDeclarationProjection, PendingLayerDeclarationProjection,
};
use crate::grammar::entry_projection::PendingEntryDeclarationProjection;
use crate::grammar::event::{
    PendingPatternProjection, PendingSyntaxDiagnostic, PendingTypeProjection, SyntaxEvent,
};
use crate::grammar::flow_projection::PendingFlowDeclarationProjection;
use crate::grammar::keyword_statement_projection::PendingKeywordStatementProjection;
use crate::grammar::kinds::{IdentityClass, SyntaxKind, SyntaxRole};
use crate::grammar::source_declaration_projection::PendingSourceDeclarationProjection;
use crate::grammar::source_projection::{
    PendingPathProjection, PendingUseProjection, PendingVisibilityKind,
};
use crate::grammar::style_projection::PendingStyleDeclarationProjection;
use crate::grammar::test_projection::PendingTestKindProjection;
use crate::grammar::view_projection::PendingViewExportProjection;
use crate::incremental::SyntaxLimit;
use crate::patterns::{AuthoredPattern, PatternNodePath};
use crate::types::{AuthoredTypeRef, TypeRefNodePath};

use super::lexer::LexToken;

/// Exact half-open token interval shared by both postfix candidates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CandidateTokenInterval {
    start: usize,
    end: usize,
}

impl CandidateTokenInterval {
    pub(super) const fn start(self) -> usize {
        self.start
    }

    pub(super) const fn end(self) -> usize {
        self.end
    }

    pub(super) const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// Exact parser position before one bounded candidate attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ParserCheckpoint {
    interval: CandidateTokenInterval,
    event_position: usize,
    depths: GrammarParserDepths,
}

/// Balanced candidate event stream over one interval of the shared token list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StagedParserEvents {
    interval: CandidateTokenInterval,
    source: SourceRange,
    events: Vec<SyntaxEvent>,
}

impl StagedParserEvents {
    pub(super) fn events(&self) -> &[SyntaxEvent] {
        &self.events
    }

    pub(super) fn diagnostics(&self) -> impl Iterator<Item = &PendingSyntaxDiagnostic> {
        self.events.iter().filter_map(|event| match event {
            SyntaxEvent::Diagnostic(diagnostic) => Some(diagnostic),
            _ => None,
        })
    }

    pub(super) fn has_recovery(&self) -> bool {
        self.events.iter().any(|event| match event {
            SyntaxEvent::StartNode {
                kind,
                expression_projection,
                assertion_projection,
                keyword_statement_projection,
                ..
            } => {
                kind.is_missing_node()
                    || kind.is_error_node()
                    || expression_projection
                        .as_ref()
                        .is_some_and(PendingExpressionProjection::has_recovery)
                    || assertion_projection
                        .as_ref()
                        .is_some_and(|projection| projection.has_recovery())
                    || keyword_statement_projection
                        .as_ref()
                        .is_some_and(PendingKeywordStatementProjection::has_recovery)
            }
            SyntaxEvent::MissingToken { .. } | SyntaxEvent::Diagnostic(_) => true,
            SyntaxEvent::Token { .. } | SyntaxEvent::FinishNode => false,
        })
    }

    /// Consumes the candidate's balanced event stream into its sole retained
    /// tokenless semantic graph. Raw tokens, missing-token events, diagnostics,
    /// Rowan nodes, and source text do not cross this boundary.
    pub(super) fn into_candidate_graph(self) -> PendingCandidateGraph {
        let mut open = Vec::<OpenCandidateNode>::new();
        let mut nodes = Vec::<CandidateNodeDraft>::new();
        let mut cursor = self.source.start();

        for event in self.events {
            match event {
                SyntaxEvent::StartNode {
                    kind,
                    role,
                    transparent_expression_group,
                    expression_projection,
                    assertion_projection,
                    keyword_statement_projection,
                    type_projection,
                    pattern_projection,
                    path_projection,
                    use_projection,
                    visibility_projection,
                    attribute_projection,
                    declaration_header_projection,
                    character_projection,
                    test_kind_projection,
                    layer_projection,
                    entry_projection,
                    style_projection,
                    source_declaration_projection,
                    method_receiver_projection,
                    contract_clause_projection,
                    flow_declaration_projection,
                    view_export_projection,
                } => {
                    assert!(
                        use_projection.is_none()
                            && visibility_projection.is_none()
                            && attribute_projection.is_none()
                            && declaration_header_projection.is_none()
                            && character_projection.is_none()
                            && test_kind_projection.is_none()
                            && layer_projection.is_none()
                            && entry_projection.is_none()
                            && style_projection.is_none()
                            && source_declaration_projection.is_none()
                            && method_receiver_projection.is_none()
                            && contract_clause_projection.is_none()
                            && flow_declaration_projection.is_none()
                            && view_export_projection.is_none(),
                        "postfix candidate grammar cannot retain item-owned projections"
                    );
                    let navigation_role = if kind.is_expression() {
                        open.iter()
                            .rposition(|owner| owner.identity.is_some())
                            .and_then(|parent| {
                                open[parent + 1..]
                                    .iter()
                                    .find(|owner| owner.transparent_expression_group)
                                    .map(|owner| owner.role)
                            })
                            .unwrap_or(role)
                    } else {
                        role
                    };
                    let parent = open.iter().rev().find_map(|owner| owner.identity);
                    let identity = if kind.identity_class() == IdentityClass::IdentityBearing {
                        let semantic = candidate_semantic(
                            expression_projection,
                            assertion_projection,
                            keyword_statement_projection,
                            type_projection,
                            pattern_projection,
                            path_projection,
                        );
                        let index = CandidateNodeIndex::try_new(nodes.len())
                            .expect("candidate node count remains grammar-bounded");
                        nodes.push(CandidateNodeDraft {
                            kind,
                            role: navigation_role,
                            parent,
                            start: cursor,
                            end: None,
                            semantic,
                        });
                        Some(index)
                    } else {
                        None
                    };
                    open.push(OpenCandidateNode {
                        role,
                        transparent_expression_group,
                        identity,
                    });
                }
                SyntaxEvent::Token { range, .. } => {
                    cursor = range.end();
                }
                SyntaxEvent::MissingToken { at, .. } => {
                    cursor = at;
                }
                SyntaxEvent::Diagnostic(_) => {}
                SyntaxEvent::FinishNode => {
                    let owner = open
                        .pop()
                        .expect("validated candidate event stream remains balanced");
                    if let Some(index) = owner.identity {
                        nodes[index.as_usize()].end = Some(cursor);
                    }
                }
            }
        }
        assert!(
            open.is_empty(),
            "validated candidate event stream closes every node"
        );
        assert_eq!(
            cursor,
            self.source.end(),
            "candidate graph consumes its exact source interval"
        );

        PendingCandidateGraph::try_new(
            nodes
                .into_iter()
                .map(|node| {
                    PendingCandidateNode::new(
                        node.kind,
                        node.role,
                        node.parent,
                        SourceRange::new(node.start, node.end.unwrap_or(node.start)),
                        node.semantic,
                    )
                })
                .collect(),
        )
        .expect("parser-produced candidate graph satisfies local adjacency invariants")
    }
}

#[derive(Clone, Copy, Debug)]
struct OpenCandidateNode {
    role: SyntaxRole,
    transparent_expression_group: bool,
    identity: Option<CandidateNodeIndex>,
}

#[derive(Clone, Debug)]
struct CandidateNodeDraft {
    kind: SyntaxKind,
    role: SyntaxRole,
    parent: Option<CandidateNodeIndex>,
    start: usize,
    end: Option<usize>,
    semantic: PendingCandidateSemantic,
}

fn candidate_semantic(
    expression: Option<PendingExpressionProjection>,
    assertion: Option<PendingAssertionProjection>,
    keyword_statement: Option<PendingKeywordStatementProjection>,
    type_ref: Option<PendingTypeProjection>,
    pattern: Option<PendingPatternProjection>,
    path: Option<PendingPathProjection>,
) -> PendingCandidateSemantic {
    let semantic_count = usize::from(expression.is_some())
        + usize::from(assertion.is_some())
        + usize::from(keyword_statement.is_some())
        + usize::from(type_ref.is_some())
        + usize::from(pattern.is_some())
        + usize::from(path.is_some());
    assert!(
        semantic_count <= 1,
        "one candidate node cannot own multiple semantic projections"
    );
    if let Some(projection) = expression {
        PendingCandidateSemantic::Expression(projection)
    } else if let Some(projection) = assertion {
        PendingCandidateSemantic::Assertion(projection)
    } else if let Some(projection) = keyword_statement {
        PendingCandidateSemantic::KeywordStatement(projection)
    } else if let Some(projection) = type_ref {
        PendingCandidateSemantic::Type(projection)
    } else if let Some(projection) = pattern {
        PendingCandidateSemantic::Pattern(projection)
    } else if let Some(projection) = path {
        PendingCandidateSemantic::Path(projection)
    } else {
        PendingCandidateSemantic::KindOnly
    }
}

/// Shared cursor and event sink for every private shadow grammar parser.
pub(super) struct ShadowDocumentParser<'source, 'events> {
    source: &'source str,
    tokens: &'source [LexToken],
    cursor: usize,
    empty_offset: usize,
    events: &'events mut Vec<SyntaxEvent>,
    budget: &'events mut GrammarBudget,
}

impl<'source, 'events> ShadowDocumentParser<'source, 'events> {
    pub(super) fn new(
        source: &'source str,
        tokens: &'source [LexToken],
        events: &'events mut Vec<SyntaxEvent>,
        budget: &'events mut GrammarBudget,
    ) -> Self {
        Self {
            source,
            tokens,
            cursor: 0,
            empty_offset: 0,
            events,
            budget,
        }
    }

    pub(super) fn for_fragment(
        source: &'source str,
        tokens: &'source [LexToken],
        empty_offset: usize,
        events: &'events mut Vec<SyntaxEvent>,
        budget: &'events mut GrammarBudget,
    ) -> Self {
        Self {
            source,
            tokens,
            cursor: 0,
            empty_offset,
            events,
            budget,
        }
    }

    pub(super) fn is_at_end(&self) -> bool {
        self.cursor == self.tokens.len()
    }

    pub(super) fn current(&self) -> Option<LexToken> {
        self.tokens.get(self.cursor).copied()
    }

    pub(super) fn current_kind(&self) -> Option<SyntaxKind> {
        self.current().map(LexToken::kind)
    }

    pub(super) fn current_text(&self) -> Option<&'source str> {
        self.current()
            .map(|token| &self.source[token.range().as_range()])
    }

    pub(super) const fn source(&self) -> &'source str {
        self.source
    }

    pub(super) fn current_offset(&self) -> usize {
        self.current().map_or_else(
            || {
                self.tokens
                    .last()
                    .map_or(self.empty_offset, |token| token.range().end())
            },
            |token| token.range().start(),
        )
    }

    pub(super) fn at(&self, spelling: &str) -> bool {
        self.current_text() == Some(spelling)
    }

    pub(super) fn bump(&mut self) -> Option<LexToken> {
        let token = self.current()?;
        let event = SyntaxEvent::token(token.kind(), token.range());
        if self.budget.event(&event) {
            self.events.push(event);
        }
        self.cursor += 1;
        Some(token)
    }

    /// Advances one already-lexed token without emitting it.
    ///
    /// `RichText` uses this only when the same token is partitioned into exact
    /// quote/content ranges in the current event transaction. The caller must
    /// emit lossless replacement token events before building the tree.
    pub(super) fn take_for_partition(&mut self) -> Option<LexToken> {
        let token = self.current()?;
        self.cursor += 1;
        Some(token)
    }

    pub(super) fn start(&mut self, kind: SyntaxKind, role: SyntaxRole) {
        if self.budget.start(kind, role) {
            self.events.push(SyntaxEvent::start(kind, role));
        }
    }

    pub(super) fn start_transparent_expression_group(&mut self, role: SyntaxRole) {
        if self.budget.start(SyntaxKind::DelimitedGroup, role) {
            self.events
                .push(SyntaxEvent::transparent_expression_group(role));
        }
    }

    pub(super) fn start_projected_owner(
        &mut self,
        kind: SyntaxKind,
        role: SyntaxRole,
    ) -> Option<usize> {
        if !self.budget.start(kind, role) {
            return None;
        }
        let position = self.events.len();
        self.events.push(SyntaxEvent::start(kind, role));
        Some(position)
    }

    pub(super) fn set_path_projection(
        &mut self,
        position: Option<usize>,
        projection: PendingPathProjection,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind: SyntaxKind::Path,
            path_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!("path projection marker must point to a Path start event");
        };
        assert!(
            path_projection.is_none(),
            "Path event receives one final semantic owner"
        );
        *path_projection = Some(projection);
    }

    pub(super) fn set_view_export_projection(
        &mut self,
        position: Option<usize>,
        projection: PendingViewExportProjection,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind: SyntaxKind::ViewExportDeclaration,
            view_export_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!("View export projection marker must point to a View export start event");
        };
        assert!(
            view_export_projection.is_none(),
            "View export event receives one parser-selected structural projection"
        );
        *view_export_projection = Some(projection);
    }

    pub(super) fn set_method_receiver_projection(
        &mut self,
        position: Option<usize>,
        projection: PendingMethodReceiverProjection,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind: SyntaxKind::Parameter,
            method_receiver_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!("method receiver projection marker must point to a Parameter start event");
        };
        assert!(
            method_receiver_projection.is_none(),
            "Parameter receives one parser-selected method receiver projection"
        );
        *method_receiver_projection = Some(projection);
    }

    pub(super) fn set_assertion_projection(
        &mut self,
        position: Option<usize>,
        projection: PendingAssertionProjection,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind: SyntaxKind::AssertionStatement,
            assertion_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!("assertion projection marker must point to an AssertionStatement start event");
        };
        assert!(
            assertion_projection.is_none(),
            "AssertionStatement receives one parser-selected mode projection"
        );
        *assertion_projection = Some(projection);
    }

    pub(super) fn set_keyword_statement_projection(
        &mut self,
        position: Option<usize>,
        projection: PendingKeywordStatementProjection,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind,
            keyword_statement_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!("keyword statement projection marker must point to a start event");
        };
        assert!(
            projection.accepts_kind(*kind),
            "keyword statement projection must match its statement family"
        );
        assert!(
            keyword_statement_projection.is_none(),
            "keyword statement event receives one parser-selected projection"
        );
        *keyword_statement_projection = Some(projection);
    }

    pub(super) fn set_character_projection(
        &mut self,
        position: Option<usize>,
        projection: PendingCharacterDeclarationProjection,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind: SyntaxKind::CharacterDeclarationItem,
            character_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!("Character projection marker must point to a Character item start event");
        };
        assert!(
            character_projection.is_none(),
            "Character event receives one parser-selected semantic projection"
        );
        *character_projection = Some(projection);
    }

    pub(super) fn set_test_kind_projection(
        &mut self,
        position: Option<usize>,
        projection: PendingTestKindProjection,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind: SyntaxKind::TestItem,
            test_kind_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!("Test kind projection marker must point to a Test item start event");
        };
        assert!(
            test_kind_projection.is_none(),
            "Test item receives one parser-selected adapter-kind projection"
        );
        *test_kind_projection = Some(projection);
    }

    pub(super) fn set_layer_projection(
        &mut self,
        position: Option<usize>,
        projection: PendingLayerDeclarationProjection,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind: SyntaxKind::LayerDeclarationItem,
            layer_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!("Layer projection marker must point to a Layer item start event");
        };
        assert!(
            layer_projection.is_none(),
            "Layer event receives one parser-selected semantic projection"
        );
        *layer_projection = Some(projection);
    }

    pub(super) fn set_entry_projection(
        &mut self,
        position: Option<usize>,
        projection: PendingEntryDeclarationProjection,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind: SyntaxKind::EntryDeclarationItem,
            entry_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!("Entry projection marker must point to an Entry item start event");
        };
        assert!(
            entry_projection.is_none(),
            "Entry event receives one parser-selected semantic projection"
        );
        *entry_projection = Some(projection);
    }

    pub(super) fn set_style_projection(
        &mut self,
        position: Option<usize>,
        projection: PendingStyleDeclarationProjection,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind: SyntaxKind::StyleItem,
            style_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!("Style projection marker must point to a Style item start event");
        };
        assert!(
            style_projection.is_none(),
            "Style event receives one parser-selected semantic projection"
        );
        *style_projection = Some(projection);
    }

    pub(super) fn set_source_declaration_projection(
        &mut self,
        position: Option<usize>,
        projection: PendingSourceDeclarationProjection,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind: SyntaxKind::SourceItem,
            source_declaration_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!("Source projection marker must point to a Source item start event");
        };
        assert!(
            source_declaration_projection.is_none(),
            "Source item receives one parser-selected semantic projection"
        );
        *source_declaration_projection = Some(projection);
    }

    pub(super) fn set_flow_contract_clause_projection(
        &mut self,
        position: Option<usize>,
        projection: PendingFlowContractClauseProjection,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind,
            contract_clause_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!("Flow contract projection marker must point to a clause start event");
        };
        assert!(
            kind.is_contract_clause() && projection.accepts_kind(*kind),
            "Flow contract projection must match its exact clause kind"
        );
        assert!(
            contract_clause_projection.is_none(),
            "Flow contract clause receives one parser-selected source projection"
        );
        *contract_clause_projection = Some(projection);
    }

    pub(super) fn set_flow_declaration_projection(
        &mut self,
        position: Option<usize>,
        projection: PendingFlowDeclarationProjection,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind: SyntaxKind::FlowItem,
            flow_declaration_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!("Flow projection marker must point to a Flow item start event");
        };
        assert!(
            flow_declaration_projection.is_none(),
            "Flow item receives one parser-selected declaration projection"
        );
        *flow_declaration_projection = Some(projection);
    }

    pub(super) fn expression_projection_at(
        &self,
        position: usize,
    ) -> Option<&PendingExpressionProjection> {
        let SyntaxEvent::StartNode {
            expression_projection,
            ..
        } = self.events.get(position)?
        else {
            return None;
        };
        expression_projection.as_ref()
    }

    /// Returns the outermost completed expression projection with this exact
    /// source range. Parser-owned declaration projections use this after a
    /// shared expression transaction has completed; it does not reopen or
    /// reparse source text.
    pub(super) fn expression_projection_for_range(
        &self,
        range: SourceRange,
    ) -> Option<&PendingExpressionProjection> {
        self.events
            .iter()
            .enumerate()
            .find_map(|(position, event)| {
                let SyntaxEvent::StartNode {
                    expression_projection: Some(projection),
                    ..
                } = event
                else {
                    return None;
                };
                (self.completed_range(position) == Some(range)).then_some(projection)
            })
    }

    pub(super) fn set_declaration_header_projection(
        &mut self,
        position: Option<usize>,
        projection: crate::grammar::declaration_projection::PendingDeclarationHeaderProjection,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind: SyntaxKind::DeclarationHeader | SyntaxKind::ProofItem,
            declaration_header_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!(
                "declaration-header projection marker must point to a declaration or ProofItem start event"
            );
        };
        assert!(
            declaration_header_projection.is_none(),
            "declaration header receives one parser-selected semantic projection"
        );
        *declaration_header_projection = Some(projection);
    }

    pub(super) fn set_attribute_projection(
        &mut self,
        position: Option<usize>,
        projection: PendingOuterAttributeProjection,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind: SyntaxKind::OuterAttribute,
            attribute_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!("attribute projection marker must point to an OuterAttribute start event");
        };
        assert!(
            attribute_projection.is_none(),
            "OuterAttribute event receives one parser-selected semantic projection"
        );
        *attribute_projection = Some(projection);
    }

    pub(super) fn set_expression_projection(
        &mut self,
        position: Option<usize>,
        projection: PendingExpressionProjection,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind,
            expression_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!("expression projection marker must point to a node start event");
        };
        assert!(
            projection.accepts_kind(*kind),
            "expression projection must agree with its exact syntax kind"
        );
        assert!(
            expression_projection.is_none(),
            "expression event receives one parser-selected semantic projection"
        );
        *expression_projection = Some(projection);
    }

    pub(super) fn set_use_projection(
        &mut self,
        position: Option<usize>,
        projection: PendingUseProjection,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind: SyntaxKind::UseDeclaration,
            use_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!("use projection marker must point to a UseDeclaration start event");
        };
        assert!(
            use_projection.is_none(),
            "UseDeclaration event receives one final import-tree owner"
        );
        *use_projection = Some(projection);
    }

    pub(super) fn set_visibility_projection(
        &mut self,
        position: Option<usize>,
        projection: PendingVisibilityKind,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            kind: SyntaxKind::Visibility,
            visibility_projection,
            ..
        }) = self.events.get_mut(position)
        else {
            panic!("visibility projection marker must point to a Visibility start event");
        };
        assert!(
            visibility_projection.is_none(),
            "Visibility event receives one parser-owned semantic projection"
        );
        *visibility_projection = Some(projection);
    }

    pub(super) fn start_type(
        &mut self,
        kind: SyntaxKind,
        role: SyntaxRole,
        tree: u64,
        authored: Arc<AuthoredTypeRef>,
        path: TypeRefNodePath,
    ) {
        if self.budget.start(kind, role) {
            self.events.push(SyntaxEvent::type_start(
                kind,
                role,
                PendingTypeProjection::new(tree, authored, path),
            ));
        }
    }

    pub(super) fn start_pattern(&mut self, kind: SyntaxKind, role: SyntaxRole) -> Option<usize> {
        if !self.budget.start(kind, role) {
            return None;
        }
        let position = self.events.len();
        self.events.push(SyntaxEvent::start(kind, role));
        Some(position)
    }

    pub(super) fn set_pattern_projection(
        &mut self,
        position: Option<usize>,
        tree: u64,
        authored: Arc<AuthoredPattern>,
        path: PatternNodePath,
    ) {
        let Some(position) = position else {
            return;
        };
        let Some(SyntaxEvent::StartNode {
            pattern_projection, ..
        }) = self.events.get_mut(position)
        else {
            panic!("Pattern marker must point to a Pattern node start event");
        };
        assert!(
            pattern_projection.is_none(),
            "Pattern event receives one final semantic owner"
        );
        *pattern_projection = Some(PendingPatternProjection::new(tree, authored, path));
    }

    pub(super) fn event_position(&self) -> usize {
        self.events.len()
    }

    /// Selects one exact lexer interval for both bounded candidate attempts.
    pub(super) fn candidate_interval(&self, end: usize) -> Option<CandidateTokenInterval> {
        (self.cursor <= end && end <= self.tokens.len()).then_some(CandidateTokenInterval {
            start: self.cursor,
            end,
        })
    }

    /// Begins one candidate attempt without cloning or reparsing source.
    pub(super) fn checkpoint_candidate(
        &self,
        interval: CandidateTokenInterval,
    ) -> ParserCheckpoint {
        assert_eq!(
            self.cursor, interval.start,
            "candidate starts at its exact shared token boundary"
        );
        ParserCheckpoint {
            interval,
            event_position: self.events.len(),
            depths: self.budget.parser_depths(),
        }
    }

    /// Removes the candidate's events and restores its token cursor.
    ///
    /// Grammar-budget work remains charged. This is intentional: discarded
    /// postfix candidates still consume the global document work limits, while
    /// their diagnostics and token owners remain unpublished.
    pub(super) fn stage_candidate(
        &mut self,
        checkpoint: ParserCheckpoint,
    ) -> Result<StagedParserEvents, SyntaxLimit> {
        if let Some(limit) = self.budget.failure() {
            self.events.truncate(checkpoint.event_position);
            self.cursor = checkpoint.interval.start;
            return Err(limit);
        }
        assert_eq!(
            self.budget.parser_depths(),
            checkpoint.depths,
            "candidate parser must return to its exact enclosing owner"
        );
        assert_eq!(
            self.cursor, checkpoint.interval.end,
            "candidate parser must consume its exact shared token interval"
        );
        assert!(
            checkpoint.event_position <= self.events.len(),
            "candidate event checkpoint remains live"
        );
        let events = self.events.split_off(checkpoint.event_position);
        self.validate_candidate_events(checkpoint.interval, &events);
        self.cursor = checkpoint.interval.start;
        Ok(StagedParserEvents {
            interval: checkpoint.interval,
            source: SourceRange::new(
                self.offset_at_token_boundary(checkpoint.interval.start)
                    .expect("candidate interval starts at a lexer boundary"),
                self.offset_at_token_boundary(checkpoint.interval.end)
                    .expect("candidate interval ends at a lexer boundary"),
            ),
            events,
        })
    }

    /// Publishes one already charged candidate stream without visiting its
    /// tokens or charging its grammar work a second time.
    pub(super) fn commit_selected(&mut self, staged: StagedParserEvents) {
        assert_eq!(
            self.cursor, staged.interval.start,
            "selected candidate starts at its shared token boundary"
        );
        self.events.extend(staged.events);
        self.cursor = staged.interval.end;
    }

    /// Emits the shared payload once when neither candidate event tree is the
    /// public CST owner (ambiguous or invalid classification).
    pub(super) fn emit_raw_interval(&mut self, interval: CandidateTokenInterval) {
        assert_eq!(
            self.cursor, interval.start,
            "raw candidate payload starts at its shared token boundary"
        );
        while self.cursor < interval.end {
            let _ = self
                .bump()
                .expect("validated candidate interval remains inside the token list");
        }
    }

    /// Publishes diagnostics already charged by a retained candidate without
    /// charging the shared grammar budget a second time.
    pub(super) fn append_precharged_diagnostics<'diagnostic>(
        &mut self,
        diagnostics: impl IntoIterator<Item = &'diagnostic PendingSyntaxDiagnostic>,
    ) {
        self.events.extend(
            diagnostics
                .into_iter()
                .cloned()
                .map(SyntaxEvent::Diagnostic),
        );
    }

    pub(super) fn started_kind_since(&self, position: usize, kind: SyntaxKind) -> bool {
        self.events[position..].iter().any(
            |event| matches!(event, SyntaxEvent::StartNode { kind: actual, .. } if *actual == kind),
        )
    }

    pub(super) fn insert_start(&mut self, position: usize, kind: SyntaxKind, role: SyntaxRole) {
        let _ = self.insert_projected_start(position, kind, role);
    }

    pub(super) fn insert_projected_start(
        &mut self,
        position: usize,
        kind: SyntaxKind,
        role: SyntaxRole,
    ) -> Option<usize> {
        if self.budget.start(kind, role) {
            self.events.insert(position, SyntaxEvent::start(kind, role));
            Some(position)
        } else {
            None
        }
    }

    pub(super) fn completed_kind(&self, position: usize) -> Option<SyntaxKind> {
        match self.events.get(position)? {
            SyntaxEvent::StartNode { kind, .. } => Some(*kind),
            _ => None,
        }
    }

    /// Returns the exact token/insertion range of one balanced completed node.
    pub(super) fn completed_range(&self, position: usize) -> Option<SourceRange> {
        if !matches!(
            self.events.get(position),
            Some(SyntaxEvent::StartNode { .. })
        ) {
            return None;
        }
        let mut depth = 0_usize;
        let mut start = None;
        let mut end = None;
        for event in &self.events[position..] {
            match event {
                SyntaxEvent::StartNode { .. } => depth = depth.checked_add(1)?,
                SyntaxEvent::Token { range, .. } if depth > 0 => {
                    start.get_or_insert(range.start());
                    end = Some(range.end());
                }
                SyntaxEvent::MissingToken { at, .. } if depth > 0 => {
                    start.get_or_insert(*at);
                    end = Some(*at);
                }
                SyntaxEvent::FinishNode => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        let start = start.unwrap_or_else(|| self.current_offset());
                        return Some(SourceRange::new(start, end.unwrap_or(start)));
                    }
                }
                SyntaxEvent::Diagnostic(_)
                | SyntaxEvent::Token { .. }
                | SyntaxEvent::MissingToken { .. } => {}
            }
        }
        None
    }

    pub(super) fn set_start_role(&mut self, position: usize, role: SyntaxRole) {
        if self.budget.failure().is_some() {
            return;
        }
        let Some(SyntaxEvent::StartNode {
            role: current_role, ..
        }) = self.events.get_mut(position)
        else {
            panic!("completed grammar marker must point to a node start event");
        };
        *current_role = role;
    }

    pub(super) fn set_start_kind(&mut self, position: usize, kind: SyntaxKind) {
        if self.budget.failure().is_some() {
            return;
        }
        let Some(SyntaxEvent::StartNode {
            kind: current_kind, ..
        }) = self.events.get_mut(position)
        else {
            panic!("completed grammar marker must point to a node start event");
        };
        *current_kind = kind;
    }

    pub(super) fn finish(&mut self) {
        if self.budget.finish() {
            self.events.push(SyntaxEvent::FinishNode);
        }
    }

    pub(super) fn push(&mut self, event: SyntaxEvent) {
        if self.budget.event(&event) {
            self.events.push(event);
        }
    }

    pub(super) fn charge_assertion_condition(&mut self) {
        self.budget.assertion_condition();
    }

    pub(super) fn charge_grouped_use_member(&mut self) -> bool {
        self.budget.grouped_use_member()
    }

    pub(super) fn charge_source_member(&mut self) -> bool {
        self.budget.source_member()
    }

    pub(super) fn enter_prefix_expression(&mut self) -> bool {
        self.budget.enter_prefix_expression()
    }

    pub(super) fn leave_prefix_expression(&mut self) {
        self.budget.leave_prefix_expression();
    }

    pub(super) const fn budget_failed(&self) -> bool {
        self.budget.failure().is_some()
    }

    pub(super) fn bump_trivia(&mut self) {
        while self.current_kind().is_some_and(is_trivia_kind) {
            self.bump();
        }
    }

    pub(super) fn next_significant(&self) -> Option<(usize, LexToken, &'source str)> {
        self.tokens[self.cursor..]
            .iter()
            .copied()
            .enumerate()
            .find(|(_, token)| !is_trivia_kind(token.kind()))
            .map(|(relative, token)| {
                (
                    self.cursor + relative,
                    token,
                    &self.source[token.range().as_range()],
                )
            })
    }

    pub(super) fn bump_through(&mut self, inclusive_index: usize) {
        while self.cursor <= inclusive_index && !self.is_at_end() {
            self.bump();
        }
    }

    pub(super) const fn cursor(&self) -> usize {
        self.cursor
    }

    pub(super) fn token_at(&self, index: usize) -> Option<LexToken> {
        self.tokens.get(index).copied()
    }

    pub(super) fn offset_at_token_boundary(&self, index: usize) -> Option<usize> {
        if let Some(token) = self.tokens.get(index) {
            return Some(token.range().start());
        }
        (index == self.tokens.len()).then(|| {
            self.tokens
                .last()
                .map_or(self.empty_offset, |token| token.range().end())
        })
    }

    pub(super) fn token_boundary_index(&self, offset: usize) -> Option<usize> {
        if offset
            == self
                .tokens
                .last()
                .map_or(self.empty_offset, |token| token.range().end())
        {
            return Some(self.tokens.len());
        }
        self.tokens[self.cursor..]
            .iter()
            .position(|token| token.range().start() == offset)
            .map(|relative| self.cursor + relative)
    }

    pub(super) fn text_of(&self, token: LexToken) -> &'source str {
        &self.source[token.range().as_range()]
    }

    fn validate_candidate_events(&self, interval: CandidateTokenInterval, events: &[SyntaxEvent]) {
        let expected_start = self
            .offset_at_token_boundary(interval.start)
            .expect("candidate interval starts at a lexer boundary");
        let expected_end = self
            .offset_at_token_boundary(interval.end)
            .expect("candidate interval ends at a lexer boundary");
        let mut depth = 0_usize;
        let mut covered = expected_start;
        for event in events {
            match event {
                SyntaxEvent::StartNode { kind, .. } => {
                    assert_ne!(
                        *kind,
                        SyntaxKind::SourceFile,
                        "candidate event stream cannot own a source root"
                    );
                    depth = depth
                        .checked_add(1)
                        .expect("candidate event nesting remains bounded");
                }
                SyntaxEvent::FinishNode => {
                    depth = depth
                        .checked_sub(1)
                        .expect("candidate event stream cannot close its envelope owner");
                }
                SyntaxEvent::Token { kind, range } => {
                    assert_ne!(
                        *kind,
                        SyntaxKind::EofToken,
                        "candidate event stream cannot own EOF"
                    );
                    assert_eq!(
                        range.start(),
                        covered,
                        "candidate tokens cover the shared interval exactly once"
                    );
                    assert!(
                        range.end() <= expected_end && self.source.is_char_boundary(range.end()),
                        "candidate token range remains inside its UTF-8 interval"
                    );
                    covered = range.end();
                }
                SyntaxEvent::MissingToken { at, .. } => assert!(
                    expected_start <= *at && *at <= expected_end,
                    "candidate recovery insertion remains inside its interval"
                ),
                SyntaxEvent::Diagnostic(diagnostic) => {
                    let range = diagnostic.range();
                    assert!(
                        expected_start <= range.start()
                            && range.end() <= expected_end
                            && self.source.is_char_boundary(range.start())
                            && self.source.is_char_boundary(range.end()),
                        "candidate diagnostic remains inside its UTF-8 interval"
                    );
                }
            }
        }
        assert_eq!(depth, 0, "candidate event stream is structurally balanced");
        assert_eq!(
            covered, expected_end,
            "candidate tokens consume the complete shared interval"
        );
    }
}

pub(super) const fn is_trivia_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::WhitespaceToken
            | SyntaxKind::NewlineToken
            | SyntaxKind::CommentToken
            | SyntaxKind::DocCommentToken
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expressions::ExpressionProjection;
    use crate::parser::lexer::DocumentLexer;

    #[test]
    fn selected_candidate_commits_precharged_events_once() {
        let source = "index";
        let tokens = DocumentLexer::new(source).lex();
        let mut events = Vec::new();
        let mut budget = GrammarBudget::default();
        let mut parser = ShadowDocumentParser::new(source, &tokens, &mut events, &mut budget);
        let interval = parser
            .candidate_interval(tokens.len())
            .expect("whole token interval");
        let checkpoint = parser.checkpoint_candidate(interval);

        parser.start(SyntaxKind::PathExpression, SyntaxRole::Argument(0));
        let _ = parser.bump();
        parser.finish();
        let staged = parser
            .stage_candidate(checkpoint)
            .expect("balanced candidate");

        assert_eq!(parser.cursor(), interval.start());
        assert!(parser.events.is_empty());
        assert_eq!(staged.events().len(), 3);
        assert!(!staged.has_recovery());

        parser.commit_selected(staged);
        assert_eq!(parser.cursor(), interval.end());
        assert_eq!(parser.events.len(), 3);
    }

    #[test]
    fn retained_candidate_normalizes_to_one_tokenless_semantic_graph() {
        let source = "index";
        let tokens = DocumentLexer::new(source).lex();
        let mut events = Vec::new();
        let mut budget = GrammarBudget::default();
        let mut parser = ShadowDocumentParser::new(source, &tokens, &mut events, &mut budget);
        let interval = parser
            .candidate_interval(tokens.len())
            .expect("whole token interval");
        let checkpoint = parser.checkpoint_candidate(interval);

        let owner =
            parser.start_projected_owner(SyntaxKind::PathExpression, SyntaxRole::Argument(0));
        let _ = parser.bump();
        parser.set_expression_projection(
            owner,
            PendingExpressionProjection::new(ExpressionProjection::Path, Vec::new()),
        );
        parser.finish();
        let graph = parser
            .stage_candidate(checkpoint)
            .expect("balanced candidate")
            .into_candidate_graph();

        assert_eq!(graph.roots().len(), 1);
        assert_eq!(graph.nodes().len(), 1);
        let root = graph.roots()[0];
        let node = graph.node(root).expect("candidate root");
        assert_eq!(node.kind(), SyntaxKind::PathExpression);
        assert_eq!(node.role(), SyntaxRole::Argument(0));
        assert_eq!(node.parent(), None);
        assert_eq!(node.source(), SourceRange::new(0, source.len()));
        assert!(matches!(
            node.semantic(),
            PendingCandidateSemantic::Expression(projection)
                if matches!(projection.projection(), ExpressionProjection::Path)
        ));
        assert!(graph.children(root).expect("root children").is_empty());
        assert!(parser.events.is_empty());
        assert_eq!(parser.cursor(), interval.start());
    }

    #[test]
    fn discarded_candidate_events_and_diagnostics_stay_unpublished() {
        let source = "value";
        let tokens = DocumentLexer::new(source).lex();
        let mut events = Vec::new();
        let mut budget = GrammarBudget::default();
        let mut parser = ShadowDocumentParser::new(source, &tokens, &mut events, &mut budget);
        let interval = parser
            .candidate_interval(tokens.len())
            .expect("whole token interval");

        let rejected = parser.checkpoint_candidate(interval);
        parser.start(SyntaxKind::ErrorExpression, SyntaxRole::Argument(0));
        let _ = parser.bump();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.expression.candidate_rejected",
            SourceRange::new(0, source.len()),
            "discarded candidate",
        )));
        parser.finish();
        let rejected = parser
            .stage_candidate(rejected)
            .expect("balanced candidate");
        assert!(rejected.has_recovery());
        assert_eq!(rejected.diagnostics().count(), 1);
        assert!(parser.events.is_empty());

        let selected = parser.checkpoint_candidate(interval);
        parser.start(SyntaxKind::PathExpression, SyntaxRole::Argument(0));
        let _ = parser.bump();
        parser.finish();
        let selected = parser
            .stage_candidate(selected)
            .expect("balanced candidate");
        parser.commit_selected(selected);

        assert!(
            !parser
                .events
                .iter()
                .any(|event| matches!(event, SyntaxEvent::Diagnostic(_)))
        );
    }

    #[test]
    fn discarded_candidate_work_still_reaches_the_global_expression_limit() {
        let source = "";
        let tokens = DocumentLexer::new(source).lex();
        let mut events = Vec::new();
        let mut budget = GrammarBudget::default();
        let mut parser = ShadowDocumentParser::new(source, &tokens, &mut events, &mut budget);
        let interval = parser
            .candidate_interval(tokens.len())
            .expect("empty token interval");

        let first = parser.checkpoint_candidate(interval);
        for _ in 0..SyntaxLimit::Expressions.maximum() {
            parser.start(SyntaxKind::MissingExpression, SyntaxRole::Argument(0));
            parser.finish();
        }
        let _discarded = parser
            .stage_candidate(first)
            .expect("exact limit is inclusive");
        assert!(parser.events.is_empty());

        let one_over = parser.checkpoint_candidate(interval);
        parser.start(SyntaxKind::MissingExpression, SyntaxRole::Argument(0));
        assert_eq!(
            parser.stage_candidate(one_over),
            Err(SyntaxLimit::Expressions)
        );
        assert_eq!(parser.cursor(), interval.start());
        assert!(parser.events.is_empty());
    }
}
