//! Balanced grammar events shared by document and nested parsers.

use arcweft_source::{DiagnosticApplicability, SourceRange};
use std::collections::HashMap;
use std::sync::Arc;

use super::kinds::{SyntaxKind, SyntaxRole};
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
use crate::grammar::source_declaration_projection::PendingSourceDeclarationProjection;
use crate::grammar::style_projection::PendingStyleDeclarationProjection;
use crate::grammar::test_projection::PendingTestKindProjection;
use crate::grammar::view_projection::{PendingViewExportProjection, PendingViewFragmentProjection};
use crate::patterns::{AuthoredPattern, PatternNodePath};
use crate::types::{AuthoredTypeRef, TypeRefNodePath};

/// Semantic pattern projection staged on the exact node-start event that owns it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingPatternProjection {
    tree: u64,
    authored: Arc<AuthoredPattern>,
    path: PatternNodePath,
}

impl PendingPatternProjection {
    pub(crate) const fn new(
        tree: u64,
        authored: Arc<AuthoredPattern>,
        path: PatternNodePath,
    ) -> Self {
        Self {
            tree,
            authored,
            path,
        }
    }

    pub(crate) const fn tree(&self) -> u64 {
        self.tree
    }

    pub(crate) const fn authored(&self) -> &Arc<AuthoredPattern> {
        &self.authored
    }

    pub(crate) const fn path(&self) -> &PatternNodePath {
        &self.path
    }

    fn rebased(&self, offset: usize, context: &mut ProjectionRebaseContext) -> Option<Self> {
        Some(Self::new(
            self.tree,
            context.pattern(&self.authored, offset)?,
            self.path.clone(),
        ))
    }
}

/// Semantic type projection staged on the exact node-start event that owns it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingTypeProjection {
    tree: u64,
    authored: Arc<AuthoredTypeRef>,
    path: TypeRefNodePath,
}

impl PendingTypeProjection {
    pub(crate) const fn new(
        tree: u64,
        authored: Arc<AuthoredTypeRef>,
        path: TypeRefNodePath,
    ) -> Self {
        Self {
            tree,
            authored,
            path,
        }
    }

    pub(crate) const fn tree(&self) -> u64 {
        self.tree
    }

    pub(crate) const fn authored(&self) -> &Arc<AuthoredTypeRef> {
        &self.authored
    }

    pub(crate) const fn path(&self) -> &TypeRefNodePath {
        &self.path
    }

    fn rebased(&self, offset: usize, context: &mut ProjectionRebaseContext) -> Option<Self> {
        Some(Self::new(
            self.tree,
            context.type_ref(&self.authored, offset)?,
            self.path.clone(),
        ))
    }
}

#[derive(Default)]
struct ProjectionRebaseContext {
    patterns: HashMap<usize, Arc<AuthoredPattern>>,
    type_refs: HashMap<usize, Arc<AuthoredTypeRef>>,
}

impl ProjectionRebaseContext {
    fn pattern(
        &mut self,
        authored: &Arc<AuthoredPattern>,
        offset: usize,
    ) -> Option<Arc<AuthoredPattern>> {
        if offset == 0 {
            return Some(Arc::clone(authored));
        }
        let key = Arc::as_ptr(authored) as usize;
        if let Some(rebased) = self.patterns.get(&key) {
            return Some(Arc::clone(rebased));
        }
        authored
            .source()
            .source_at(&PatternNodePath::root())?
            .end()
            .checked_add(offset)?;
        let mut rebased = (**authored).clone();
        rebased.rebase_with_type_children(offset, |child| self.type_ref(child, offset))?;
        let rebased = Arc::new(rebased);
        self.patterns.insert(key, Arc::clone(&rebased));
        Some(rebased)
    }

    fn type_ref(
        &mut self,
        authored: &Arc<AuthoredTypeRef>,
        offset: usize,
    ) -> Option<Arc<AuthoredTypeRef>> {
        if offset == 0 {
            return Some(Arc::clone(authored));
        }
        let key = Arc::as_ptr(authored) as usize;
        if let Some(rebased) = self.type_refs.get(&key) {
            return Some(Arc::clone(rebased));
        }
        authored.root_source().whole().end().checked_add(offset)?;
        let mut rebased = (**authored).clone();
        rebased.rebase(offset);
        let rebased = Arc::new(rebased);
        self.type_refs.insert(key, Arc::clone(&rebased));
        Some(rebased)
    }
}

/// Token class expected at a zero-width recovery insertion.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ExpectedToken {
    kind: SyntaxKind,
    spelling: Option<&'static str>,
}

impl ExpectedToken {
    /// Creates an expected-token value from a real token kind.
    pub(crate) fn try_new(kind: SyntaxKind) -> Option<Self> {
        (kind.is_token() && !matches!(kind, SyntaxKind::MissingToken | SyntaxKind::EofToken))
            .then_some(Self {
                kind,
                spelling: None,
            })
    }

    pub(crate) fn try_with_spelling(kind: SyntaxKind, spelling: &'static str) -> Option<Self> {
        Self::try_new(kind).map(|expected| Self {
            spelling: Some(spelling),
            ..expected
        })
    }

    /// Returns the expected grammar token kind.
    pub(crate) const fn kind(self) -> SyntaxKind {
        self.kind
    }

    pub(crate) const fn spelling(self) -> Option<&'static str> {
        self.spelling
    }
}

/// Diagnostic staged by the event parser before snapshot attachment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingSyntaxDiagnostic {
    code: &'static str,
    range: SourceRange,
    related_range: Option<SourceRange>,
    related_message: Option<String>,
    expected: Box<[String]>,
    suggestions: Box<[PendingSyntaxSuggestion]>,
    message: String,
}

/// Recovery edit staged before the grammar snapshot is source-bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingSyntaxEdit {
    range: SourceRange,
    replacement: String,
}

/// Generic recovery suggestion retained by the attached syntax authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingSyntaxSuggestion {
    message: String,
    applicability: DiagnosticApplicability,
    edits: Box<[PendingSyntaxEdit]>,
}

impl PendingSyntaxDiagnostic {
    pub(crate) fn new(code: &'static str, range: SourceRange, message: impl Into<String>) -> Self {
        Self {
            code,
            range,
            related_range: None,
            related_message: None,
            expected: Box::new([]),
            suggestions: Box::new([]),
            message: message.into(),
        }
    }

    pub(crate) const fn with_related_range(mut self, related_range: SourceRange) -> Self {
        self.related_range = Some(related_range);
        self
    }

    pub(crate) fn with_related_message(mut self, message: impl Into<String>) -> Self {
        self.related_message = Some(message.into());
        self
    }

    pub(crate) fn with_expected(
        mut self,
        expected: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.expected = expected
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        self
    }

    pub(crate) fn with_suggestions(
        mut self,
        suggestions: impl IntoIterator<Item = PendingSyntaxSuggestion>,
    ) -> Self {
        self.suggestions = suggestions
            .into_iter()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        self
    }

    pub(crate) const fn code(&self) -> &'static str {
        self.code
    }

    pub(crate) const fn range(&self) -> SourceRange {
        self.range
    }

    pub(crate) const fn related_range(&self) -> Option<SourceRange> {
        self.related_range
    }

    pub(crate) fn related_message(&self) -> Option<&str> {
        self.related_message.as_deref()
    }

    pub(crate) fn expected(&self) -> &[String] {
        &self.expected
    }

    pub(crate) fn suggestions(&self) -> &[PendingSyntaxSuggestion] {
        &self.suggestions
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            code: self.code,
            range: rebase_range(self.range, offset)?,
            related_range: match self.related_range {
                Some(range) => Some(rebase_range(range, offset)?),
                None => None,
            },
            related_message: self.related_message.clone(),
            expected: self.expected.clone(),
            suggestions: self
                .suggestions
                .iter()
                .map(|suggestion| suggestion.rebased(offset))
                .collect::<Option<Vec<_>>>()?
                .into_boxed_slice(),
            message: self.message.clone(),
        })
    }
}

impl PendingSyntaxEdit {
    pub(crate) fn new(range: SourceRange, replacement: impl Into<String>) -> Self {
        Self {
            range,
            replacement: replacement.into(),
        }
    }

    pub(crate) const fn range(&self) -> SourceRange {
        self.range
    }

    pub(crate) fn replacement(&self) -> &str {
        &self.replacement
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self::new(
            rebase_range(self.range, offset)?,
            self.replacement.clone(),
        ))
    }
}

impl PendingSyntaxSuggestion {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            applicability: DiagnosticApplicability::Unspecified,
            edits: Box::new([]),
        }
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    pub(crate) const fn applicability(&self) -> DiagnosticApplicability {
        self.applicability
    }

    pub(crate) fn edits(&self) -> &[PendingSyntaxEdit] {
        &self.edits
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            message: self.message.clone(),
            applicability: self.applicability,
            edits: self
                .edits
                .iter()
                .map(|edit| edit.rebased(offset))
                .collect::<Option<Vec<_>>>()?
                .into_boxed_slice(),
        })
    }
}

/// The sole semantic projection owned by one node-start event.
///
/// A closed variant replaces the former parallel `Option` fields, so parser
/// transactions cannot represent a node with multiple competing semantic
/// owners. Boxed payloads keep the lossless event stream compact while the
/// selected projection retains its final typed schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingStartProjection {
    None,
    Expression(Box<PendingExpressionProjection>),
    Assertion(PendingAssertionProjection),
    KeywordStatement(Box<PendingKeywordStatementProjection>),
    Type(Box<PendingTypeProjection>),
    Pattern(Box<PendingPatternProjection>),
    Path(Box<PendingPathProjection>),
    Use(Box<PendingUseProjection>),
    Visibility(PendingVisibilityKind),
    Attribute(Box<PendingOuterAttributeProjection>),
    DeclarationHeader(Box<PendingDeclarationHeaderProjection>),
    Character(Box<PendingCharacterDeclarationProjection>),
    TestKind(Box<PendingTestKindProjection>),
    Layer(Box<PendingLayerDeclarationProjection>),
    Entry(Box<PendingEntryDeclarationProjection>),
    Style(Box<PendingStyleDeclarationProjection>),
    SourceDeclaration(Box<PendingSourceDeclarationProjection>),
    MethodReceiver(Box<PendingMethodReceiverProjection>),
    ContractClause(Box<PendingFlowContractClauseProjection>),
    FlowDeclaration(Box<PendingFlowDeclarationProjection>),
    ViewExport(Box<PendingViewExportProjection>),
    ViewFragment(Box<PendingViewFragmentProjection>),
}

impl PendingStartProjection {
    pub(crate) const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub(crate) fn select(&mut self, projection: Self) {
        assert!(self.is_none(), "one node event receives one semantic owner");
        assert!(
            !projection.is_none(),
            "semantic projection selection is non-empty"
        );
        *self = projection;
    }

    pub(crate) fn accepts_kind(&self, kind: SyntaxKind) -> bool {
        match self {
            Self::None => true,
            Self::Expression(projection) => projection.accepts_kind(kind),
            Self::Assertion(_) => kind == SyntaxKind::AssertionStatement,
            Self::KeywordStatement(projection) => projection.accepts_kind(kind),
            Self::Type(_) => kind.is_type_node(),
            Self::Pattern(_) => kind.is_pattern_node(),
            Self::Path(_) => kind == SyntaxKind::Path,
            Self::Use(_) => kind == SyntaxKind::UseDeclaration,
            Self::Visibility(_) => kind == SyntaxKind::Visibility,
            Self::Attribute(_) => matches!(
                kind,
                SyntaxKind::InnerAttribute | SyntaxKind::OuterAttribute
            ),
            Self::DeclarationHeader(_) => {
                matches!(kind, SyntaxKind::DeclarationHeader | SyntaxKind::ProofItem)
            }
            Self::Character(_) => kind == SyntaxKind::CharacterDeclarationItem,
            Self::TestKind(_) => kind == SyntaxKind::TestItem,
            Self::Layer(_) => kind == SyntaxKind::LayerDeclarationItem,
            Self::Entry(_) => kind == SyntaxKind::EntryDeclarationItem,
            Self::Style(_) => kind == SyntaxKind::StyleItem,
            Self::SourceDeclaration(_) => kind == SyntaxKind::SourceItem,
            Self::MethodReceiver(_) => kind == SyntaxKind::Parameter,
            Self::ContractClause(projection) => projection.accepts_kind(kind),
            Self::FlowDeclaration(_) => kind == SyntaxKind::FlowItem,
            Self::ViewExport(_) => kind == SyntaxKind::ViewExportDeclaration,
            Self::ViewFragment(_) => kind == SyntaxKind::ViewFragment,
        }
    }

    fn rebased(&self, offset: usize, context: &mut ProjectionRebaseContext) -> Option<Self> {
        Some(match self {
            Self::None => Self::None,
            Self::Expression(projection) => Self::Expression(Box::new(projection.rebased(offset)?)),
            Self::Assertion(projection) => Self::Assertion(*projection),
            Self::KeywordStatement(projection) => {
                Self::KeywordStatement(Box::new((**projection).clone()))
            }
            Self::Type(projection) => Self::Type(Box::new(projection.rebased(offset, context)?)),
            Self::Pattern(projection) => {
                Self::Pattern(Box::new(projection.rebased(offset, context)?))
            }
            Self::Path(projection) => Self::Path(Box::new(projection.rebased(offset)?)),
            Self::Use(projection) => Self::Use(Box::new(projection.rebased(offset)?)),
            Self::Visibility(projection) => Self::Visibility(*projection),
            Self::Attribute(projection) => Self::Attribute(Box::new(projection.rebased(offset)?)),
            Self::DeclarationHeader(projection) => {
                Self::DeclarationHeader(Box::new(projection.rebased(offset)?))
            }
            Self::Character(projection) => Self::Character(Box::new(projection.rebased(offset)?)),
            Self::TestKind(projection) => Self::TestKind(Box::new(projection.rebased(offset)?)),
            Self::Layer(projection) => Self::Layer(Box::new(projection.rebased(offset)?)),
            Self::Entry(projection) => Self::Entry(Box::new(projection.rebased(offset)?)),
            Self::Style(projection) => Self::Style(Box::new(projection.rebased(offset)?)),
            Self::SourceDeclaration(projection) => {
                Self::SourceDeclaration(Box::new(projection.rebased(offset)?))
            }
            Self::MethodReceiver(projection) => {
                Self::MethodReceiver(Box::new(projection.rebased(offset)?))
            }
            Self::ContractClause(projection) => {
                Self::ContractClause(Box::new(projection.rebased(offset)?))
            }
            Self::FlowDeclaration(projection) => {
                Self::FlowDeclaration(Box::new(projection.rebased(offset)?))
            }
            Self::ViewExport(projection) => Self::ViewExport(Box::new(projection.rebased(offset)?)),
            Self::ViewFragment(projection) => {
                Self::ViewFragment(Box::new(projection.rebased(offset)?))
            }
        })
    }
}

/// One event in the single lossless grammar stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SyntaxEvent {
    StartNode {
        kind: SyntaxKind,
        role: SyntaxRole,
        transparent_expression_group: bool,
        projection: PendingStartProjection,
    },
    Token {
        kind: SyntaxKind,
        range: SourceRange,
    },
    MissingToken {
        expected: ExpectedToken,
        at: usize,
    },
    Diagnostic(PendingSyntaxDiagnostic),
    FinishNode,
}

impl SyntaxEvent {
    pub(crate) const fn start(kind: SyntaxKind, role: SyntaxRole) -> Self {
        Self::StartNode {
            kind,
            role,
            transparent_expression_group: false,
            projection: PendingStartProjection::None,
        }
    }

    /// Starts the one ID-less parenthesized-expression wrapper whose outer
    /// navigation role belongs to its inner semantic expression identity.
    pub(crate) const fn transparent_expression_group(role: SyntaxRole) -> Self {
        Self::StartNode {
            kind: SyntaxKind::DelimitedGroup,
            role,
            transparent_expression_group: true,
            projection: PendingStartProjection::None,
        }
    }

    pub(crate) fn type_start(
        kind: SyntaxKind,
        role: SyntaxRole,
        projection: PendingTypeProjection,
    ) -> Self {
        Self::StartNode {
            kind,
            role,
            transparent_expression_group: false,
            projection: PendingStartProjection::Type(Box::new(projection)),
        }
    }

    #[cfg(test)]
    pub(crate) fn expression_start(
        kind: SyntaxKind,
        role: SyntaxRole,
        projection: PendingExpressionProjection,
    ) -> Self {
        Self::StartNode {
            kind,
            role,
            transparent_expression_group: false,
            projection: PendingStartProjection::Expression(Box::new(projection)),
        }
    }

    #[cfg(test)]
    pub(crate) fn assertion_start(
        role: SyntaxRole,
        projection: PendingAssertionProjection,
    ) -> Self {
        Self::StartNode {
            kind: SyntaxKind::AssertionStatement,
            role,
            transparent_expression_group: false,
            projection: PendingStartProjection::Assertion(projection),
        }
    }

    pub(crate) const fn token(kind: SyntaxKind, range: SourceRange) -> Self {
        Self::Token { kind, range }
    }

    #[cfg(test)]
    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        self.rebased_with(offset, &mut ProjectionRebaseContext::default())
    }

    pub(crate) fn rebased_all(events: &[Self], offset: usize) -> Option<Vec<Self>> {
        let mut context = ProjectionRebaseContext::default();
        events
            .iter()
            .map(|event| event.rebased_with(offset, &mut context))
            .collect()
    }

    fn rebased_with(&self, offset: usize, context: &mut ProjectionRebaseContext) -> Option<Self> {
        match self {
            Self::StartNode {
                kind,
                role,
                transparent_expression_group,
                projection,
            } => Some(Self::StartNode {
                kind: *kind,
                role: *role,
                transparent_expression_group: *transparent_expression_group,
                projection: projection.rebased(offset, context)?,
            }),
            Self::Token { kind, range } => Some(Self::Token {
                kind: *kind,
                range: rebase_range(*range, offset)?,
            }),
            Self::MissingToken { expected, at } => Some(Self::MissingToken {
                expected: *expected,
                at: at.checked_add(offset)?,
            }),
            Self::Diagnostic(diagnostic) => Some(Self::Diagnostic(diagnostic.rebased(offset)?)),
            Self::FinishNode => Some(Self::FinishNode),
        }
    }
}

fn rebase_range(range: SourceRange, offset: usize) -> Option<SourceRange> {
    Some(SourceRange::new(
        range.start().checked_add(offset)?,
        range.end().checked_add(offset)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::{ExpectedToken, PendingSyntaxDiagnostic, SyntaxEvent};
    use crate::assertion::AssertionMode;
    use crate::expressions::{
        ExpressionComponentRole, ExpressionProjection, PendingExpressionComponent,
        PendingExpressionProjection, SyntaxPlaceholderKind,
    };
    use crate::grammar::assertion_projection::PendingAssertionProjection;
    use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
    use arcweft_source::SourceRange;

    #[test]
    fn fragment_events_rebase_every_source_coordinate_exactly() {
        let expected = ExpectedToken::try_with_spelling(SyntaxKind::PunctuationToken, ")")
            .expect("real expected token");
        let diagnostic = PendingSyntaxDiagnostic::new(
            "syntax.fragment.test",
            SourceRange::new(1, 3),
            "test diagnostic",
        )
        .with_related_range(SourceRange::new(0, 1));

        assert_eq!(
            SyntaxEvent::start(SyntaxKind::CallExpression, SyntaxRole::Element(0)).rebased(8),
            Some(SyntaxEvent::start(
                SyntaxKind::CallExpression,
                SyntaxRole::Element(0)
            ))
        );
        assert_eq!(
            SyntaxEvent::transparent_expression_group(SyntaxRole::Element(1)).rebased(8),
            Some(SyntaxEvent::transparent_expression_group(
                SyntaxRole::Element(1)
            ))
        );
        assert_eq!(
            SyntaxEvent::assertion_start(
                SyntaxRole::Statement(0),
                PendingAssertionProjection::new(Some(AssertionMode::Prove)),
            )
            .rebased(8),
            Some(SyntaxEvent::assertion_start(
                SyntaxRole::Statement(0),
                PendingAssertionProjection::new(Some(AssertionMode::Prove)),
            ))
        );
        assert_eq!(
            SyntaxEvent::expression_start(
                SyntaxKind::PlaceholderExpression,
                SyntaxRole::Element(0),
                PendingExpressionProjection::new(
                    ExpressionProjection::Placeholder(SyntaxPlaceholderKind::PipeLeft),
                    vec![PendingExpressionComponent::new(
                        ExpressionComponentRole::PlaceholderMarker,
                        SourceRange::new(2, 3),
                    )],
                ),
            )
            .rebased(8),
            Some(SyntaxEvent::expression_start(
                SyntaxKind::PlaceholderExpression,
                SyntaxRole::Element(0),
                PendingExpressionProjection::new(
                    ExpressionProjection::Placeholder(SyntaxPlaceholderKind::PipeLeft),
                    vec![PendingExpressionComponent::new(
                        ExpressionComponentRole::PlaceholderMarker,
                        SourceRange::new(10, 11),
                    )],
                ),
            ))
        );
        assert_eq!(
            SyntaxEvent::token(SyntaxKind::IdentifierToken, SourceRange::new(0, 3)).rebased(8),
            Some(SyntaxEvent::token(
                SyntaxKind::IdentifierToken,
                SourceRange::new(8, 11)
            ))
        );
        assert_eq!(
            SyntaxEvent::MissingToken { expected, at: 3 }.rebased(8),
            Some(SyntaxEvent::MissingToken { expected, at: 11 })
        );
        assert_eq!(
            SyntaxEvent::Diagnostic(diagnostic).rebased(8),
            Some(SyntaxEvent::Diagnostic(
                PendingSyntaxDiagnostic::new(
                    "syntax.fragment.test",
                    SourceRange::new(9, 11),
                    "test diagnostic"
                )
                .with_related_range(SourceRange::new(8, 9))
            ))
        );
        assert_eq!(
            SyntaxEvent::FinishNode.rebased(8),
            Some(SyntaxEvent::FinishNode)
        );
    }

    #[test]
    fn fragment_event_rebase_rejects_coordinate_overflow() {
        let expected =
            ExpectedToken::try_new(SyntaxKind::IdentifierToken).expect("real expected token");
        assert_eq!(
            SyntaxEvent::token(
                SyntaxKind::IdentifierToken,
                SourceRange::new(usize::MAX, usize::MAX)
            )
            .rebased(1),
            None
        );
        assert_eq!(
            SyntaxEvent::MissingToken {
                expected,
                at: usize::MAX
            }
            .rebased(1),
            None
        );
        assert_eq!(
            SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.fragment.overflow",
                SourceRange::new(usize::MAX, usize::MAX),
                "overflow"
            ))
            .rebased(1),
            None
        );
        assert_eq!(
            SyntaxEvent::expression_start(
                SyntaxKind::PlaceholderExpression,
                SyntaxRole::Element(0),
                PendingExpressionProjection::new(
                    ExpressionProjection::Placeholder(SyntaxPlaceholderKind::PartialApplication,),
                    vec![PendingExpressionComponent::new(
                        ExpressionComponentRole::PlaceholderMarker,
                        SourceRange::new(usize::MAX, usize::MAX),
                    )],
                ),
            )
            .rebased(1),
            None
        );
    }
}
