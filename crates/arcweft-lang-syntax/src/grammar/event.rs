//! Balanced grammar events shared by document and nested parsers.

#![allow(
    dead_code,
    reason = "the shadow grammar remains crate-private until the atomic syntax switch"
)]

use arcweft_source::SourceRange;
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
    PendingCharacterDeclarationProjection, PendingLayerDeclarationProjection,
    PendingRetainedHeaderProjection,
};
use crate::grammar::entry_projection::PendingEntryDeclarationProjection;
use crate::grammar::flow_projection::PendingFlowDeclarationProjection;
use crate::grammar::keyword_statement_projection::PendingKeywordStatementProjection;
use crate::grammar::source_declaration_projection::PendingSourceDeclarationProjection;
use crate::grammar::style_projection::PendingStyleDeclarationProjection;
use crate::grammar::test_projection::PendingTestKindProjection;
use crate::grammar::view_projection::PendingViewExportProjection;
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
    message: String,
}

impl PendingSyntaxDiagnostic {
    pub(crate) fn new(code: &'static str, range: SourceRange, message: impl Into<String>) -> Self {
        Self {
            code,
            range,
            related_range: None,
            message: message.into(),
        }
    }

    pub(crate) const fn with_related_range(mut self, related_range: SourceRange) -> Self {
        self.related_range = Some(related_range);
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
            message: self.message.clone(),
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
        expression_projection: Option<PendingExpressionProjection>,
        assertion_projection: Option<PendingAssertionProjection>,
        keyword_statement_projection: Option<PendingKeywordStatementProjection>,
        type_projection: Option<PendingTypeProjection>,
        pattern_projection: Option<PendingPatternProjection>,
        path_projection: Option<PendingPathProjection>,
        use_projection: Option<PendingUseProjection>,
        visibility_projection: Option<PendingVisibilityKind>,
        attribute_projection: Option<PendingOuterAttributeProjection>,
        retained_header_projection: Option<PendingRetainedHeaderProjection>,
        character_projection: Option<PendingCharacterDeclarationProjection>,
        test_kind_projection: Option<PendingTestKindProjection>,
        layer_projection: Option<PendingLayerDeclarationProjection>,
        entry_projection: Option<PendingEntryDeclarationProjection>,
        style_projection: Option<PendingStyleDeclarationProjection>,
        source_declaration_projection: Option<PendingSourceDeclarationProjection>,
        method_receiver_projection: Option<PendingMethodReceiverProjection>,
        contract_clause_projection: Option<PendingFlowContractClauseProjection>,
        flow_declaration_projection: Option<PendingFlowDeclarationProjection>,
        view_export_projection: Option<PendingViewExportProjection>,
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
            expression_projection: None,
            assertion_projection: None,
            keyword_statement_projection: None,
            type_projection: None,
            pattern_projection: None,
            path_projection: None,
            use_projection: None,
            visibility_projection: None,
            attribute_projection: None,
            retained_header_projection: None,
            character_projection: None,
            test_kind_projection: None,
            layer_projection: None,
            entry_projection: None,
            style_projection: None,
            source_declaration_projection: None,
            method_receiver_projection: None,
            contract_clause_projection: None,
            flow_declaration_projection: None,
            view_export_projection: None,
        }
    }

    /// Starts the one ID-less parenthesized-expression wrapper whose outer
    /// navigation role belongs to its inner semantic expression identity.
    pub(crate) const fn transparent_expression_group(role: SyntaxRole) -> Self {
        Self::StartNode {
            kind: SyntaxKind::DelimitedGroup,
            role,
            transparent_expression_group: true,
            expression_projection: None,
            assertion_projection: None,
            keyword_statement_projection: None,
            type_projection: None,
            pattern_projection: None,
            path_projection: None,
            use_projection: None,
            visibility_projection: None,
            attribute_projection: None,
            retained_header_projection: None,
            character_projection: None,
            test_kind_projection: None,
            layer_projection: None,
            entry_projection: None,
            style_projection: None,
            source_declaration_projection: None,
            method_receiver_projection: None,
            contract_clause_projection: None,
            flow_declaration_projection: None,
            view_export_projection: None,
        }
    }

    pub(crate) const fn type_start(
        kind: SyntaxKind,
        role: SyntaxRole,
        projection: PendingTypeProjection,
    ) -> Self {
        Self::StartNode {
            kind,
            role,
            transparent_expression_group: false,
            expression_projection: None,
            assertion_projection: None,
            keyword_statement_projection: None,
            type_projection: Some(projection),
            pattern_projection: None,
            path_projection: None,
            use_projection: None,
            visibility_projection: None,
            attribute_projection: None,
            retained_header_projection: None,
            character_projection: None,
            test_kind_projection: None,
            layer_projection: None,
            entry_projection: None,
            style_projection: None,
            source_declaration_projection: None,
            method_receiver_projection: None,
            contract_clause_projection: None,
            flow_declaration_projection: None,
            view_export_projection: None,
        }
    }

    pub(crate) const fn expression_start(
        kind: SyntaxKind,
        role: SyntaxRole,
        projection: PendingExpressionProjection,
    ) -> Self {
        Self::StartNode {
            kind,
            role,
            transparent_expression_group: false,
            expression_projection: Some(projection),
            assertion_projection: None,
            keyword_statement_projection: None,
            type_projection: None,
            pattern_projection: None,
            path_projection: None,
            use_projection: None,
            visibility_projection: None,
            attribute_projection: None,
            retained_header_projection: None,
            character_projection: None,
            test_kind_projection: None,
            layer_projection: None,
            entry_projection: None,
            style_projection: None,
            source_declaration_projection: None,
            method_receiver_projection: None,
            contract_clause_projection: None,
            flow_declaration_projection: None,
            view_export_projection: None,
        }
    }

    pub(crate) const fn assertion_start(
        role: SyntaxRole,
        projection: PendingAssertionProjection,
    ) -> Self {
        Self::StartNode {
            kind: SyntaxKind::AssertionStatement,
            role,
            transparent_expression_group: false,
            expression_projection: None,
            assertion_projection: Some(projection),
            keyword_statement_projection: None,
            type_projection: None,
            pattern_projection: None,
            path_projection: None,
            use_projection: None,
            visibility_projection: None,
            attribute_projection: None,
            retained_header_projection: None,
            character_projection: None,
            test_kind_projection: None,
            layer_projection: None,
            entry_projection: None,
            style_projection: None,
            source_declaration_projection: None,
            method_receiver_projection: None,
            contract_clause_projection: None,
            flow_declaration_projection: None,
            view_export_projection: None,
        }
    }

    pub(crate) const fn path_start(
        kind: SyntaxKind,
        role: SyntaxRole,
        projection: PendingPathProjection,
    ) -> Self {
        Self::StartNode {
            kind,
            role,
            transparent_expression_group: false,
            expression_projection: None,
            assertion_projection: None,
            keyword_statement_projection: None,
            type_projection: None,
            pattern_projection: None,
            path_projection: Some(projection),
            use_projection: None,
            visibility_projection: None,
            attribute_projection: None,
            retained_header_projection: None,
            character_projection: None,
            test_kind_projection: None,
            layer_projection: None,
            entry_projection: None,
            style_projection: None,
            source_declaration_projection: None,
            method_receiver_projection: None,
            contract_clause_projection: None,
            flow_declaration_projection: None,
            view_export_projection: None,
        }
    }

    pub(crate) const fn token(kind: SyntaxKind, range: SourceRange) -> Self {
        Self::Token { kind, range }
    }

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
                expression_projection,
                assertion_projection,
                keyword_statement_projection,
                type_projection,
                pattern_projection,
                path_projection,
                use_projection,
                visibility_projection,
                attribute_projection,
                retained_header_projection,
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
            } => Some(Self::StartNode {
                kind: *kind,
                role: *role,
                transparent_expression_group: *transparent_expression_group,
                expression_projection: match expression_projection {
                    Some(projection) => Some(projection.rebased(offset)?),
                    None => None,
                },
                assertion_projection: *assertion_projection,
                keyword_statement_projection: keyword_statement_projection.clone(),
                type_projection: match type_projection {
                    Some(projection) => Some(projection.rebased(offset, context)?),
                    None => None,
                },
                pattern_projection: match pattern_projection {
                    Some(projection) => Some(projection.rebased(offset, context)?),
                    None => None,
                },
                path_projection: match path_projection {
                    Some(projection) => Some(projection.rebased(offset)?),
                    None => None,
                },
                use_projection: match use_projection {
                    Some(projection) => Some(projection.rebased(offset)?),
                    None => None,
                },
                visibility_projection: *visibility_projection,
                attribute_projection: match attribute_projection {
                    Some(projection) => Some(projection.rebased(offset)?),
                    None => None,
                },
                retained_header_projection: match retained_header_projection {
                    Some(projection) => Some(projection.rebased(offset)?),
                    None => None,
                },
                character_projection: match character_projection {
                    Some(projection) => Some(projection.rebased(offset)?),
                    None => None,
                },
                test_kind_projection: match test_kind_projection {
                    Some(projection) => Some(projection.rebased(offset)?),
                    None => None,
                },
                layer_projection: match layer_projection {
                    Some(projection) => Some(projection.rebased(offset)?),
                    None => None,
                },
                entry_projection: match entry_projection {
                    Some(projection) => Some(projection.rebased(offset)?),
                    None => None,
                },
                style_projection: rebase_style_projection(style_projection.as_ref(), offset)
                    .ok()?,
                source_declaration_projection: match source_declaration_projection {
                    Some(projection) => Some(projection.rebased(offset)?),
                    None => None,
                },
                method_receiver_projection: match method_receiver_projection {
                    Some(projection) => Some(projection.rebased(offset)?),
                    None => None,
                },
                contract_clause_projection: match contract_clause_projection {
                    Some(projection) => Some(projection.rebased(offset)?),
                    None => None,
                },
                flow_declaration_projection: match flow_declaration_projection {
                    Some(projection) => Some(projection.rebased(offset)?),
                    None => None,
                },
                view_export_projection: match view_export_projection {
                    Some(projection) => Some(projection.rebased(offset)?),
                    None => None,
                },
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

fn rebase_style_projection(
    projection: Option<&PendingStyleDeclarationProjection>,
    offset: usize,
) -> Result<Option<PendingStyleDeclarationProjection>, ProjectionRebaseFailure> {
    projection.map_or(Ok(None), |projection| {
        projection
            .rebased(offset)
            .map(Some)
            .ok_or(ProjectionRebaseFailure)
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectionRebaseFailure;

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
