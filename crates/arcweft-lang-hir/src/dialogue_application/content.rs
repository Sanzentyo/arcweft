//! Complete dialogue-content records owned by one expression.

use super::rich_text::{
    HirRichTextEndTag, HirRichTextTag, HirRichTextTagId, validate_argument_ids,
};
use super::{
    HirDialogueExpressionExpectation, HirDialogueInvariantError, HirDialogueOrdinalError,
    HirDialogueTransactionContext, HirDialogueTransactionError, HirDialogueTransactionRequirement,
    HirRichTextCharge, validate_module,
};
use crate::identity::{ExprId, HirModuleId};
use arcweft_lang_syntax::expressions::{SyntaxDialogueContentIssue, SyntaxLineBreakKind};

/// Dialogue content owned one-to-one by an application expression.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueContent {
    id: HirDialogueContentId,
    nodes: Box<[HirDialogueNode]>,
    tags: Box<[HirRichTextTag]>,
}

impl HirDialogueContent {
    pub(crate) fn try_new(
        id: HirDialogueContentId,
        nodes: Box<[HirDialogueNode]>,
        tags: Box<[HirRichTextTag]>,
    ) -> Result<Self, HirDialogueInvariantError> {
        validate_content_ids(id, &nodes, &tags)?;
        Ok(Self { id, nodes, tags })
    }

    /// Returns the application-owned content identity.
    pub const fn id(&self) -> HirDialogueContentId {
        self.id
    }

    /// Returns source-ordered dialogue nodes.
    pub const fn nodes(&self) -> &[HirDialogueNode] {
        &self.nodes
    }

    /// Returns source-ordered `RichText` tags.
    pub const fn tags(&self) -> &[HirRichTextTag] {
        &self.tags
    }

    pub(super) fn validate_module(&self, expected: HirModuleId) -> Result<(), HirModuleId> {
        validate_module(expected, self.id.owner.module())?;
        for node in &self.nodes {
            node.validate_module(expected)?;
        }
        for tag in &self.tags {
            tag.validate_module(expected)?;
        }
        Ok(())
    }

    pub(super) fn validate_transaction<C: HirDialogueTransactionContext>(
        &self,
        context: &mut C,
    ) -> Result<(), HirDialogueTransactionError<C::Error>> {
        context
            .require(HirDialogueTransactionRequirement::RichTextCharge(
                HirRichTextCharge::ContentTags {
                    observed: self.tags.len(),
                },
            ))
            .map_err(HirDialogueTransactionError::Context)?;
        let mut argument_count = 0usize;
        for tag in &self.tags {
            argument_count = argument_count.checked_add(tag.arguments().len()).ok_or(
                HirDialogueTransactionError::Invariant(
                    HirDialogueInvariantError::ArithmeticOverflow,
                ),
            )?;
        }
        context
            .require(HirDialogueTransactionRequirement::RichTextCharge(
                HirRichTextCharge::ContentArguments {
                    observed: argument_count,
                },
            ))
            .map_err(HirDialogueTransactionError::Context)?;
        for node in &self.nodes {
            node.validate_transaction(context)?;
        }
        for tag in &self.tags {
            tag.validate_transaction(context)?;
        }
        Ok(())
    }

    pub(super) fn has_recovery(&self) -> bool {
        self.nodes.iter().any(HirDialogueNode::has_recovery)
            || self.tags.iter().any(HirRichTextTag::has_recovery)
    }
}

/// Identity of one content value; its owner is the application `ExprId`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueContentId {
    owner: ExprId,
}

impl HirDialogueContentId {
    pub(crate) const fn new(owner: ExprId) -> Self {
        Self { owner }
    }

    /// Returns the one application expression that owns this content.
    pub const fn owner(self) -> ExprId {
        self.owner
    }
}

/// Contiguous node identity local to one dialogue content value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueNodeId {
    content: HirDialogueContentId,
    ordinal: u32,
}

impl HirDialogueNodeId {
    pub(crate) fn try_new(
        content: HirDialogueContentId,
        ordinal: usize,
    ) -> Result<Self, HirDialogueOrdinalError> {
        u32::try_from(ordinal)
            .map(|ordinal| Self { content, ordinal })
            .map_err(|_| HirDialogueOrdinalError::Node { ordinal })
    }

    /// Returns the owning content identity.
    pub const fn content(self) -> HirDialogueContentId {
        self.content
    }

    /// Returns the zero-based source-order ordinal.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }
}

/// One dialogue-content node.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueNode {
    id: HirDialogueNodeId,
    kind: HirDialogueNodeKind,
}

impl HirDialogueNode {
    pub(crate) const fn new(id: HirDialogueNodeId, kind: HirDialogueNodeKind) -> Self {
        Self { id, kind }
    }

    /// Returns the content-local node identity.
    pub const fn id(&self) -> HirDialogueNodeId {
        self.id
    }

    /// Returns the typed semantic node payload.
    pub const fn kind(&self) -> &HirDialogueNodeKind {
        &self.kind
    }

    pub(super) fn validate_module(&self, expected: HirModuleId) -> Result<(), HirModuleId> {
        match &self.kind {
            HirDialogueNodeKind::Interpolation(expression) => {
                validate_module(expected, expression.module())
            }
            HirDialogueNodeKind::AuthoredEndTag(tag) | HirDialogueNodeKind::InferredEndTag(tag) => {
                tag.validate_module(expected)
            }
            _ => Ok(()),
        }
    }

    pub(super) fn validate_transaction<C: HirDialogueTransactionContext>(
        &self,
        context: &mut C,
    ) -> Result<(), HirDialogueTransactionError<C::Error>> {
        if let HirDialogueNodeKind::Interpolation(expression) = self.kind {
            context
                .require(HirDialogueTransactionRequirement::Expression {
                    id: expression,
                    expected: HirDialogueExpressionExpectation::Unrestricted,
                })
                .map_err(HirDialogueTransactionError::Context)?;
        }
        Ok(())
    }

    pub(super) fn has_recovery(&self) -> bool {
        match &self.kind {
            HirDialogueNodeKind::AuthoredEndTag(tag) | HirDialogueNodeKind::InferredEndTag(tag) => {
                tag.issue().is_some()
            }
            HirDialogueNodeKind::Error(_) => true,
            _ => false,
        }
    }
}

/// Exhaustive typed dialogue node families.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDialogueNodeKind {
    Text(HirTextFragment),
    Raw(HirTextFragment),
    Escape(char),
    Ruby(HirRuby),
    AuthoredStartTag(HirRichTextTagId),
    InferredStartTag(HirRichTextTagId),
    AuthoredEndTag(HirRichTextEndTag),
    InferredEndTag(HirRichTextEndTag),
    Interpolation(ExprId),
    LineBreak(HirLineBreakKind),
    Error(HirDialogueContentError),
}

/// Decoded semantic text without delimiter or source spelling.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirTextFragment(Box<str>);

impl HirTextFragment {
    pub(crate) const fn new(value: Box<str>) -> Self {
        Self(value)
    }

    /// Returns decoded semantic text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Decoded ruby base and annotation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRuby {
    base: Box<str>,
    ruby: Box<str>,
}

impl HirRuby {
    pub(crate) const fn new(base: Box<str>, ruby: Box<str>) -> Self {
        Self { base, ruby }
    }

    /// Returns the decoded base text.
    pub fn base(&self) -> &str {
        &self.base
    }

    /// Returns the decoded ruby annotation.
    pub fn ruby(&self) -> &str {
        &self.ruby
    }
}

/// Dialogue boundary kind after normalization.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLineBreakKind {
    Line,
    Paragraph,
    Page,
}

impl From<SyntaxLineBreakKind> for HirLineBreakKind {
    fn from(value: SyntaxLineBreakKind) -> Self {
        match value {
            SyntaxLineBreakKind::Line => Self::Line,
            SyntaxLineBreakKind::Paragraph => Self::Paragraph,
            SyntaxLineBreakKind::Page => Self::Page,
        }
    }
}

/// Typed malformed dialogue-content families.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDialogueContentError {
    UnclassifiedToken,
    InvalidEscape,
    InvalidRuby,
    UnmatchedEndTag,
    UnclosedTag,
}

impl From<SyntaxDialogueContentIssue> for HirDialogueContentError {
    fn from(value: SyntaxDialogueContentIssue) -> Self {
        match value {
            SyntaxDialogueContentIssue::UnclassifiedToken => Self::UnclassifiedToken,
            SyntaxDialogueContentIssue::InvalidEscape => Self::InvalidEscape,
            SyntaxDialogueContentIssue::InvalidRuby => Self::InvalidRuby,
            SyntaxDialogueContentIssue::UnmatchedEndTag => Self::UnmatchedEndTag,
            SyntaxDialogueContentIssue::UnclosedTag => Self::UnclosedTag,
        }
    }
}

/// Public semantic issue vocabulary for dialogue content.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDialogueIssue {
    ForeignChild,
    DuplicateNodeId,
    NonContiguousNodeOrdinal,
    InvalidTagReference,
    InvalidArgumentReference,
    InvalidPlan,
}

fn validate_content_ids(
    content: HirDialogueContentId,
    nodes: &[HirDialogueNode],
    tags: &[HirRichTextTag],
) -> Result<(), HirDialogueInvariantError> {
    for (ordinal, node) in nodes.iter().enumerate() {
        let expected =
            u32::try_from(ordinal).map_err(|_| HirDialogueInvariantError::ArithmeticOverflow)?;
        if node.id.content != content || node.id.ordinal != expected {
            return Err(HirDialogueInvariantError::NonContiguousNodeOrdinal);
        }
        match &node.kind {
            HirDialogueNodeKind::AuthoredStartTag(tag)
            | HirDialogueNodeKind::InferredStartTag(tag) => {
                if tag.content() != content
                    || tags.get(tag.ordinal() as usize).map(HirRichTextTag::id) != Some(*tag)
                {
                    return Err(HirDialogueInvariantError::InvalidTagReference);
                }
            }
            HirDialogueNodeKind::AuthoredEndTag(tag) if tag.is_inferred() => {
                return Err(HirDialogueInvariantError::InvalidEndTagInference);
            }
            HirDialogueNodeKind::InferredEndTag(tag) if !tag.is_inferred() => {
                return Err(HirDialogueInvariantError::InvalidEndTagInference);
            }
            HirDialogueNodeKind::AuthoredEndTag(end) | HirDialogueNodeKind::InferredEndTag(end) => {
                if let Some(tag) = end.paired_start()
                    && (tag.content() != content
                        || tags.get(tag.ordinal() as usize).map(HirRichTextTag::id) != Some(tag))
                {
                    return Err(HirDialogueInvariantError::InvalidTagReference);
                }
            }
            _ => {}
        }
    }
    for (ordinal, tag) in tags.iter().enumerate() {
        let expected =
            u32::try_from(ordinal).map_err(|_| HirDialogueInvariantError::ArithmeticOverflow)?;
        if tag.id().content() != content || tag.id().ordinal() != expected {
            return Err(HirDialogueInvariantError::NonContiguousTagOrdinal);
        }
        validate_argument_ids(tag.id(), tag.arguments())?;
    }
    Ok(())
}
