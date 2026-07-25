//! Checked source surfaces for generic bracket and colon applications.

use super::{
    DialogueDedentBoundary, DialogueIndentation, DialogueInlineBoundary,
    DialogueSurfaceInvariantError, source_slice, validate_contains, validate_delimiter,
    validate_offset, validate_order,
};
use crate::{ast::common::TextRange, attachment::SyntaxNodeId};
use arcweft_source::SourceDocument;

/// Exact source ownership for a line plan attached to an application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct AttachedLinePlanSurface {
    syntax: SyntaxNodeId,
    range: TextRange,
}

/// Source surface shared by all interpretations of one postfix bracket.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct PostfixBracketSurface {
    syntax: SyntaxNodeId,
    target_syntax: SyntaxNodeId,
    payload_syntax: SyntaxNodeId,
    target_range: TextRange,
    open_bracket: TextRange,
    payload_range: TextRange,
    terminator: BracketTerminatorSyntax,
    plan: Option<AttachedLinePlanSurface>,
}

/// Present or recovered close ownership for a postfix bracket.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) enum BracketTerminatorSyntax {
    Closed {
        close_bracket: TextRange,
    },
    RecoveredMissing {
        insertion: usize,
        boundary: PostfixBracketRecoveryBoundarySyntax,
    },
}

/// Exact grammar boundary used to recover one missing postfix close.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) enum PostfixBracketRecoveryBoundarySyntax {
    EndOfExpression {
        anchor: usize,
    },
    LineEnding {
        range: TextRange,
    },
    OwnerEnd {
        anchor: usize,
    },
    Token {
        token: PostfixBracketBoundaryToken,
        range: TextRange,
    },
    PlanKeyword {
        range: TextRange,
    },
}

/// Token families that end a malformed postfix payload.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in super::super) enum PostfixBracketBoundaryToken {
    Comma,
    Semicolon,
    CloseParen,
    CloseBracket,
    CloseBrace,
    FatArrow,
}

/// Present semantic dialogue content or its exact insertion point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) enum DialogueContentSite {
    Present {
        range: TextRange,
    },
    Missing {
        insertion: usize,
        boundary: DialogueContentRecoveryBoundarySyntax,
    },
}

/// Source boundary explaining why dialogue content is missing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) enum DialogueContentRecoveryBoundarySyntax {
    CloseBracket { range: TextRange },
    MissingBracketClose { insertion: usize },
    Inline(DialogueInlineBoundary),
    Indented(DialogueDedentBoundary),
}

/// Bracket or colon surface for one typed dialogue application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) enum DialogueContentApplicationSurface {
    Bracket(BracketDialogueApplicationSurface),
    Colon(ColonDialogueApplicationSurface),
}

/// Dialogue interpretation of one generic postfix bracket.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct BracketDialogueApplicationSurface {
    bracket: PostfixBracketSurface,
    content: DialogueContentSite,
}

/// Exact source ownership for one colon dialogue application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct ColonDialogueApplicationSurface {
    syntax: SyntaxNodeId,
    target_syntax: SyntaxNodeId,
    head_range: TextRange,
    colon: TextRange,
    content: DialogueContentSite,
    indentation: DialogueIndentation,
    plan: Option<AttachedLinePlanSurface>,
}

impl AttachedLinePlanSurface {
    pub(in super::super) fn try_new(
        document: &SourceDocument,
        syntax: SyntaxNodeId,
        range: TextRange,
    ) -> Result<Self, DialogueSurfaceInvariantError> {
        source_slice(document, range)?;
        Ok(Self { syntax, range })
    }

    pub const fn syntax(&self) -> SyntaxNodeId {
        self.syntax
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl PostfixBracketSurface {
    #[allow(clippy::too_many_arguments)]
    pub(in super::super) fn try_new(
        document: &SourceDocument,
        syntax: SyntaxNodeId,
        target_syntax: SyntaxNodeId,
        payload_syntax: SyntaxNodeId,
        target_range: TextRange,
        open_bracket: TextRange,
        payload_range: TextRange,
        terminator: BracketTerminatorSyntax,
        plan: Option<AttachedLinePlanSurface>,
    ) -> Result<Self, DialogueSurfaceInvariantError> {
        if syntax == target_syntax || syntax == payload_syntax || target_syntax == payload_syntax {
            return Err(DialogueSurfaceInvariantError::DuplicateSyntaxIdentity);
        }
        source_slice(document, target_range)?;
        validate_delimiter(document, open_bracket, b'[')?;
        source_slice(document, payload_range)?;
        validate_order(target_range, open_bracket)?;
        if open_bracket.end() != payload_range.start() {
            return Err(DialogueSurfaceInvariantError::BoundaryMismatch);
        }
        let terminator_end = match &terminator {
            BracketTerminatorSyntax::Closed { close_bracket } => {
                validate_delimiter(document, *close_bracket, b']')?;
                if payload_range.end() != close_bracket.start() {
                    return Err(DialogueSurfaceInvariantError::BoundaryMismatch);
                }
                close_bracket.end()
            }
            BracketTerminatorSyntax::RecoveredMissing {
                insertion,
                boundary,
            } => {
                validate_offset(document, *insertion)?;
                if payload_range.end() != *insertion
                    || validate_postfix_recovery_boundary(document, boundary)? != *insertion
                {
                    return Err(DialogueSurfaceInvariantError::BoundaryMismatch);
                }
                *insertion
            }
        };
        if let Some(plan) = &plan {
            source_slice(document, plan.range())?;
            if plan.syntax() == syntax
                || plan.syntax() == target_syntax
                || plan.syntax() == payload_syntax
            {
                return Err(DialogueSurfaceInvariantError::DuplicateSyntaxIdentity);
            }
            if plan.range().start() < terminator_end
                || matches!(
                    &terminator,
                    BracketTerminatorSyntax::RecoveredMissing {
                        boundary: PostfixBracketRecoveryBoundarySyntax::PlanKeyword { range },
                        ..
                    } if range.start() != plan.range().start()
                )
            {
                return Err(DialogueSurfaceInvariantError::PlanOrdering);
            }
        }
        Ok(Self {
            syntax,
            target_syntax,
            payload_syntax,
            target_range,
            open_bracket,
            payload_range,
            terminator,
            plan,
        })
    }

    pub const fn syntax(&self) -> SyntaxNodeId {
        self.syntax
    }

    pub const fn target_syntax(&self) -> SyntaxNodeId {
        self.target_syntax
    }

    pub const fn payload_syntax(&self) -> SyntaxNodeId {
        self.payload_syntax
    }

    pub const fn target_range(&self) -> TextRange {
        self.target_range
    }

    pub const fn open_bracket(&self) -> TextRange {
        self.open_bracket
    }

    pub const fn payload_range(&self) -> TextRange {
        self.payload_range
    }

    pub const fn terminator(&self) -> &BracketTerminatorSyntax {
        &self.terminator
    }

    pub const fn plan(&self) -> Option<&AttachedLinePlanSurface> {
        self.plan.as_ref()
    }

    pub const fn range(&self) -> TextRange {
        let end = match &self.plan {
            Some(plan) => plan.range().end(),
            None => self.terminator.end(),
        };
        TextRange::new(self.target_range.start(), end)
    }
}

impl BracketTerminatorSyntax {
    pub const fn end(&self) -> usize {
        match self {
            Self::Closed { close_bracket } => close_bracket.end(),
            Self::RecoveredMissing { insertion, .. } => *insertion,
        }
    }

    pub const fn close_bracket(&self) -> Option<TextRange> {
        match self {
            Self::Closed { close_bracket } => Some(*close_bracket),
            Self::RecoveredMissing { .. } => None,
        }
    }

    pub const fn insertion(&self) -> Option<usize> {
        match self {
            Self::Closed { .. } => None,
            Self::RecoveredMissing { insertion, .. } => Some(*insertion),
        }
    }
}

impl PostfixBracketRecoveryBoundarySyntax {
    pub const fn start(&self) -> usize {
        match self {
            Self::EndOfExpression { anchor } | Self::OwnerEnd { anchor } => *anchor,
            Self::LineEnding { range }
            | Self::Token { range, .. }
            | Self::PlanKeyword { range } => range.start(),
        }
    }
}

impl DialogueContentSite {
    pub const fn range(&self) -> Option<TextRange> {
        match self {
            Self::Present { range } => Some(*range),
            Self::Missing { .. } => None,
        }
    }

    pub const fn insertion(&self) -> Option<usize> {
        match self {
            Self::Present { .. } => None,
            Self::Missing { insertion, .. } => Some(*insertion),
        }
    }
}

impl BracketDialogueApplicationSurface {
    pub(in super::super) fn try_new(
        document: &SourceDocument,
        bracket: PostfixBracketSurface,
        content: DialogueContentSite,
    ) -> Result<Self, DialogueSurfaceInvariantError> {
        validate_bracket_content_site(document, &bracket, &content)?;
        Ok(Self { bracket, content })
    }

    pub const fn bracket(&self) -> &PostfixBracketSurface {
        &self.bracket
    }

    pub const fn content(&self) -> &DialogueContentSite {
        &self.content
    }
}

impl ColonDialogueApplicationSurface {
    #[allow(clippy::too_many_arguments)]
    pub(in super::super) fn try_new(
        document: &SourceDocument,
        syntax: SyntaxNodeId,
        target_syntax: SyntaxNodeId,
        head_range: TextRange,
        colon: TextRange,
        content: DialogueContentSite,
        indentation: DialogueIndentation,
        plan: Option<AttachedLinePlanSurface>,
    ) -> Result<Self, DialogueSurfaceInvariantError> {
        if syntax == target_syntax {
            return Err(DialogueSurfaceInvariantError::DuplicateSyntaxIdentity);
        }
        source_slice(document, head_range)?;
        validate_delimiter(document, colon, b':')?;
        if head_range.end() != colon.start() {
            return Err(DialogueSurfaceInvariantError::BoundaryMismatch);
        }
        if indentation_head(&indentation).range().end() != head_range.start() {
            return Err(DialogueSurfaceInvariantError::BoundaryMismatch);
        }
        validate_colon_content_site(document, colon, &content, &indentation)?;
        let application_end = colon_application_end(&content, &indentation);
        if let Some(plan) = &plan {
            source_slice(document, plan.range())?;
            if plan.syntax() == syntax || plan.syntax() == target_syntax {
                return Err(DialogueSurfaceInvariantError::DuplicateSyntaxIdentity);
            }
            if plan.range().start() < application_end
                || indentation_plan_start(&indentation)
                    .is_some_and(|start| start != plan.range().start())
            {
                return Err(DialogueSurfaceInvariantError::PlanOrdering);
            }
            if !indentation_plan_matches(&indentation, plan.syntax()) {
                return Err(DialogueSurfaceInvariantError::BoundaryMismatch);
            }
        } else if indentation_has_plan(&indentation) {
            return Err(DialogueSurfaceInvariantError::BoundaryMismatch);
        }
        Ok(Self {
            syntax,
            target_syntax,
            head_range,
            colon,
            content,
            indentation,
            plan,
        })
    }

    pub const fn syntax(&self) -> SyntaxNodeId {
        self.syntax
    }

    pub const fn target_syntax(&self) -> SyntaxNodeId {
        self.target_syntax
    }

    pub const fn head_range(&self) -> TextRange {
        self.head_range
    }

    pub const fn colon(&self) -> TextRange {
        self.colon
    }

    pub const fn content(&self) -> &DialogueContentSite {
        &self.content
    }

    pub const fn indentation(&self) -> &DialogueIndentation {
        &self.indentation
    }

    pub const fn plan(&self) -> Option<&AttachedLinePlanSurface> {
        self.plan.as_ref()
    }

    pub const fn range(&self) -> TextRange {
        let end = match &self.plan {
            Some(plan) => plan.range().end(),
            None => colon_application_end(&self.content, &self.indentation),
        };
        TextRange::new(self.head_range.start(), end)
    }
}

impl DialogueContentApplicationSurface {
    pub const fn range(&self) -> TextRange {
        match self {
            Self::Bracket(surface) => surface.bracket().range(),
            Self::Colon(surface) => surface.range(),
        }
    }
}

fn validate_postfix_recovery_boundary(
    document: &SourceDocument,
    boundary: &PostfixBracketRecoveryBoundarySyntax,
) -> Result<usize, DialogueSurfaceInvariantError> {
    match boundary {
        PostfixBracketRecoveryBoundarySyntax::EndOfExpression { anchor }
        | PostfixBracketRecoveryBoundarySyntax::OwnerEnd { anchor } => {
            validate_offset(document, *anchor)?;
            Ok(*anchor)
        }
        PostfixBracketRecoveryBoundarySyntax::LineEnding { range } => {
            let bytes = source_slice(document, *range)?.as_bytes();
            if bytes == b"\n" || bytes == b"\r\n" {
                Ok(range.start())
            } else {
                Err(DialogueSurfaceInvariantError::InvalidLineEndingBytes { range: *range })
            }
        }
        PostfixBracketRecoveryBoundarySyntax::Token { token, range } => {
            let expected = match token {
                PostfixBracketBoundaryToken::Comma => b",".as_slice(),
                PostfixBracketBoundaryToken::Semicolon => b";".as_slice(),
                PostfixBracketBoundaryToken::CloseParen => b")".as_slice(),
                PostfixBracketBoundaryToken::CloseBracket => b"]".as_slice(),
                PostfixBracketBoundaryToken::CloseBrace => b"}".as_slice(),
                PostfixBracketBoundaryToken::FatArrow => b"=>".as_slice(),
            };
            if source_slice(document, *range)?.as_bytes() == expected {
                Ok(range.start())
            } else {
                Err(DialogueSurfaceInvariantError::BoundaryMismatch)
            }
        }
        PostfixBracketRecoveryBoundarySyntax::PlanKeyword { range } => {
            if source_slice(document, *range)? == "with" {
                Ok(range.start())
            } else {
                Err(DialogueSurfaceInvariantError::BoundaryMismatch)
            }
        }
    }
}

fn validate_bracket_content_site(
    document: &SourceDocument,
    bracket: &PostfixBracketSurface,
    content: &DialogueContentSite,
) -> Result<(), DialogueSurfaceInvariantError> {
    match content {
        DialogueContentSite::Present { range } => {
            source_slice(document, *range)?;
            if range.start() == range.end() {
                return Err(DialogueSurfaceInvariantError::ContentIndentationMismatch);
            }
            validate_contains(bracket.payload_range(), *range)
        }
        DialogueContentSite::Missing {
            insertion,
            boundary,
        } => {
            validate_offset(document, *insertion)?;
            if *insertion < bracket.payload_range().start()
                || *insertion > bracket.payload_range().end()
            {
                return Err(DialogueSurfaceInvariantError::Containment {
                    container: bracket.payload_range(),
                    child: TextRange::new(*insertion, *insertion),
                });
            }
            match (boundary, bracket.terminator()) {
                (
                    DialogueContentRecoveryBoundarySyntax::CloseBracket { range },
                    BracketTerminatorSyntax::Closed { close_bracket },
                ) if range == close_bracket => Ok(()),
                (
                    DialogueContentRecoveryBoundarySyntax::MissingBracketClose {
                        insertion: close_insertion,
                    },
                    BracketTerminatorSyntax::RecoveredMissing {
                        insertion: terminator_insertion,
                        ..
                    },
                ) if close_insertion == terminator_insertion => Ok(()),
                _ => Err(DialogueSurfaceInvariantError::ContentIndentationMismatch),
            }
        }
    }
}

fn validate_colon_content_site(
    document: &SourceDocument,
    colon: TextRange,
    content: &DialogueContentSite,
    indentation: &DialogueIndentation,
) -> Result<(), DialogueSurfaceInvariantError> {
    let first_owned = colon.end();
    match (content, indentation) {
        (DialogueContentSite::Present { range }, DialogueIndentation::Inline(inline)) => {
            source_slice(document, *range)?;
            if range.start() == range.end()
                || inline.separator().start() != first_owned
                || inline.separator().end() > range.start()
                || range.end() > inline.boundary().start()
            {
                return Err(DialogueSurfaceInvariantError::ContentIndentationMismatch);
            }
            Ok(())
        }
        (DialogueContentSite::Present { range }, DialogueIndentation::Indented(indented)) => {
            source_slice(document, *range)?;
            validate_contains(indented.body(), *range)?;
            if range.start() == range.end()
                || indented.head_line_ending().range().start() < first_owned
            {
                return Err(DialogueSurfaceInvariantError::ContentIndentationMismatch);
            }
            Ok(())
        }
        (
            DialogueContentSite::Missing {
                insertion,
                boundary,
            },
            DialogueIndentation::Missing(missing),
        ) if *insertion == missing.insertion()
            && missing_content_boundary_matches(boundary, missing.boundary()) =>
        {
            Ok(())
        }
        _ => Err(DialogueSurfaceInvariantError::ContentIndentationMismatch),
    }
}

const fn missing_content_boundary_matches(
    content: &DialogueContentRecoveryBoundarySyntax,
    indentation: &super::DialogueMissingBoundary,
) -> bool {
    match (content, indentation) {
        (
            DialogueContentRecoveryBoundarySyntax::Inline(content),
            super::DialogueMissingBoundary::Inline(indentation),
        ) => content.start() == indentation.start(),
        (
            DialogueContentRecoveryBoundarySyntax::Indented(content),
            super::DialogueMissingBoundary::Indented(indentation),
        ) => content.start() == indentation.start(),
        _ => false,
    }
}

const fn colon_application_end(
    content: &DialogueContentSite,
    indentation: &DialogueIndentation,
) -> usize {
    let indentation_end = match indentation {
        DialogueIndentation::Inline(inline) => inline.boundary().start(),
        DialogueIndentation::Indented(indented) => indented.body().end(),
        DialogueIndentation::Missing(missing) => match missing.boundary() {
            super::DialogueMissingBoundary::Inline(boundary) => boundary.start(),
            super::DialogueMissingBoundary::Indented(boundary) => boundary.start(),
        },
    };
    let content_end = match content {
        DialogueContentSite::Present { range } => range.end(),
        DialogueContentSite::Missing { insertion, .. } => *insertion,
    };
    if indentation_end >= content_end {
        indentation_end
    } else {
        content_end
    }
}

const fn indentation_head(indentation: &DialogueIndentation) -> super::DialogueIndentationPrefix {
    match indentation {
        DialogueIndentation::Inline(inline) => inline.head(),
        DialogueIndentation::Indented(indented) => indented.head(),
        DialogueIndentation::Missing(missing) => missing.head(),
    }
}

const fn indentation_has_plan(indentation: &DialogueIndentation) -> bool {
    match indentation {
        DialogueIndentation::Inline(inline) => inline.boundary().plan_syntax().is_some(),
        DialogueIndentation::Indented(indented) => indented.dedent().plan_syntax().is_some(),
        DialogueIndentation::Missing(missing) => match missing.boundary() {
            super::DialogueMissingBoundary::Inline(boundary) => boundary.plan_syntax().is_some(),
            super::DialogueMissingBoundary::Indented(boundary) => boundary.plan_syntax().is_some(),
        },
    }
}

fn indentation_plan_matches(indentation: &DialogueIndentation, plan_syntax: SyntaxNodeId) -> bool {
    match indentation {
        DialogueIndentation::Inline(inline) => {
            matches!(inline.boundary().plan_syntax(), Some(value) if value == plan_syntax)
        }
        DialogueIndentation::Indented(indented) => {
            matches!(indented.dedent().plan_syntax(), Some(value) if value == plan_syntax)
        }
        DialogueIndentation::Missing(missing) => match missing.boundary() {
            super::DialogueMissingBoundary::Inline(boundary) => {
                matches!(boundary.plan_syntax(), Some(value) if value == plan_syntax)
            }
            super::DialogueMissingBoundary::Indented(boundary) => {
                matches!(boundary.plan_syntax(), Some(value) if value == plan_syntax)
            }
        },
    }
}

const fn indentation_plan_start(indentation: &DialogueIndentation) -> Option<usize> {
    match indentation {
        DialogueIndentation::Inline(inline) => match inline.boundary() {
            DialogueInlineBoundary::AttachedPlan { at, .. } => Some(*at),
            DialogueInlineBoundary::LineEnding(_)
            | DialogueInlineBoundary::OwnerEnd { .. }
            | DialogueInlineBoundary::EndOfDocument { .. } => None,
        },
        DialogueIndentation::Indented(indented) => match indented.dedent() {
            DialogueDedentBoundary::AttachedPlan { line_start, .. } => Some(*line_start),
            DialogueDedentBoundary::DedentedLine { .. }
            | DialogueDedentBoundary::OwnerEnd { .. }
            | DialogueDedentBoundary::EndOfDocument { .. } => None,
        },
        DialogueIndentation::Missing(missing) => match missing.boundary() {
            super::DialogueMissingBoundary::Inline(DialogueInlineBoundary::AttachedPlan {
                at,
                ..
            }) => Some(*at),
            super::DialogueMissingBoundary::Indented(DialogueDedentBoundary::AttachedPlan {
                line_start,
                ..
            }) => Some(*line_start),
            super::DialogueMissingBoundary::Inline(
                DialogueInlineBoundary::LineEnding(_)
                | DialogueInlineBoundary::OwnerEnd { .. }
                | DialogueInlineBoundary::EndOfDocument { .. },
            )
            | super::DialogueMissingBoundary::Indented(
                DialogueDedentBoundary::DedentedLine { .. }
                | DialogueDedentBoundary::OwnerEnd { .. }
                | DialogueDedentBoundary::EndOfDocument { .. },
            ) => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BracketDialogueApplicationSurface, BracketTerminatorSyntax,
        ColonDialogueApplicationSurface, DialogueContentRecoveryBoundarySyntax,
        DialogueContentSite, PostfixBracketSurface,
    };
    use crate::{
        ast::common::TextRange,
        attachment::SyntaxNodeId,
        expr::dialogue_application::{DialogueIndentation, DialogueIndentationPrefix},
        incremental::SyntaxDatabase,
    };
    use arcweft_source::{
        SourceDocument, SourceDocumentId, SourceName, identity::SourceSnapshotId,
    };
    use std::sync::Arc;

    fn document_and_ids(text: &str) -> (Arc<SourceDocument>, Vec<SyntaxNodeId>) {
        let name = SourceName::Generated;
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcw:/dialogue-surface-test").expect("document ID"),
                name.clone(),
                text,
            )
            .expect("test document"),
        );
        let mut database = SyntaxDatabase::try_new().expect("syntax database identity");
        let parsed = database
            .parse_initial(SourceSnapshotId::initial(name), Arc::clone(&document))
            .expect("source parses transactionally");
        let ids = parsed
            .attached()
            .nodes()
            .map(|node| node.id())
            .collect::<Vec<_>>();
        (document, ids)
    }

    #[test]
    fn bracket_surface_derives_one_root_range_and_keeps_missing_content_insertion() {
        let (document, ids) =
            document_and_ids("target[  ]\nfn identity_a() {}\nfn identity_b() {}\n");
        assert!(ids.len() >= 3, "test source needs three syntax identities");
        let bracket = PostfixBracketSurface::try_new(
            &document,
            ids[0],
            ids[1],
            ids[2],
            TextRange::new(0, 6),
            TextRange::new(6, 7),
            TextRange::new(7, 9),
            BracketTerminatorSyntax::Closed {
                close_bracket: TextRange::new(9, 10),
            },
            None,
        )
        .expect("checked bracket surface");
        let dialogue = BracketDialogueApplicationSurface::try_new(
            &document,
            bracket,
            DialogueContentSite::Missing {
                insertion: 7,
                boundary: DialogueContentRecoveryBoundarySyntax::CloseBracket {
                    range: TextRange::new(9, 10),
                },
            },
        )
        .expect("missing dialogue content retains its own insertion");

        assert_eq!(dialogue.bracket().range(), TextRange::new(0, 10));
        assert_eq!(dialogue.content().insertion(), Some(7));
    }

    #[test]
    fn inline_colon_surface_agrees_with_byte_exact_head_and_content_boundaries() {
        let (document, ids) = document_and_ids("speaker: hello");
        assert!(ids.len() >= 2, "test source needs two syntax identities");
        let indentation = DialogueIndentation::try_inline(
            &document,
            DialogueIndentationPrefix::try_new(&document, TextRange::new(0, 0))
                .expect("line-start head"),
            TextRange::new(8, 9),
            super::DialogueInlineBoundary::EndOfDocument { anchor: 14 },
        )
        .expect("inline indentation");
        let surface = ColonDialogueApplicationSurface::try_new(
            &document,
            ids[0],
            ids[1],
            TextRange::new(0, 7),
            TextRange::new(7, 8),
            DialogueContentSite::Present {
                range: TextRange::new(9, 14),
            },
            indentation,
            None,
        )
        .expect("checked colon surface");

        assert_eq!(surface.range(), TextRange::new(0, 14));
    }
}
