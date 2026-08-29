//! Complete dialogue-content records owned by one expression.

use std::collections::BTreeSet;

use super::rich_text::{
    HirRichTextEndTag, HirRichTextTag, HirRichTextTagId, HirRichTextTagIdentity,
    HirRichTextTagPayload, validate_argument_ids,
};
use super::{
    HirDialogueExpressionExpectation, HirDialogueInvariantError, HirDialogueOrdinalError,
    HirDialogueTransactionContext, HirDialogueTransactionError, HirDialogueTransactionRequirement,
    HirRichTextCharge, validate_module,
};
use crate::identity::{ExprId, HirLimit, HirModuleId};
use crate::leaf::HirIdSuffix;
use arcweft_lang_syntax::expressions::{SyntaxDialogueContentIssue, SyntaxLineBreakKind};

/// Source-ordered ordinal of one marker in dialogue content.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueMarkOrdinal(u32);

impl HirDialogueMarkOrdinal {
    fn try_new(ordinal: usize) -> Result<Self, HirDialogueOrdinalError> {
        u32::try_from(ordinal)
            .map(Self)
            .map_err(|_| HirDialogueOrdinalError::Mark { ordinal })
    }

    /// Returns the zero-based marker ordinal in source content order.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Content-qualified identity of one dialogue marker.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueMarkId {
    content: HirDialogueContentId,
    ordinal: HirDialogueMarkOrdinal,
}

impl HirDialogueMarkId {
    const fn new(content: HirDialogueContentId, ordinal: HirDialogueMarkOrdinal) -> Self {
        Self { content, ordinal }
    }

    /// Returns the exact dialogue content owner.
    pub const fn content(self) -> HirDialogueContentId {
        self.content
    }

    /// Returns the source-ordered marker ordinal.
    pub const fn ordinal(self) -> HirDialogueMarkOrdinal {
        self.ordinal
    }
}

/// Validated one-segment local marker suffix retained for diagnostics and
/// catalog lookup.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueMarkName(HirIdSuffix);

impl HirDialogueMarkName {
    pub(crate) const fn new(suffix: HirIdSuffix) -> Self {
        Self(suffix)
    }

    /// Returns the underlying validated one-segment HIR ID suffix.
    pub const fn suffix(&self) -> &HirIdSuffix {
        &self.0
    }

    /// Returns the marker name spelling retained as HIR semantic evidence.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// One source-ordered row in a dialogue content's marker catalog.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueMark {
    id: HirDialogueMarkId,
    name: HirDialogueMarkName,
    tag: HirRichTextTagId,
}

impl HirDialogueMark {
    const fn new(id: HirDialogueMarkId, name: HirDialogueMarkName, tag: HirRichTextTagId) -> Self {
        Self { id, name, tag }
    }

    /// Returns the content-qualified marker identity.
    pub const fn id(&self) -> HirDialogueMarkId {
        self.id
    }

    /// Returns the marker's validated local name.
    pub const fn name(&self) -> &HirDialogueMarkName {
        &self.name
    }

    /// Returns the marker tag row that introduced this marker.
    pub const fn tag(&self) -> HirRichTextTagId {
        self.tag
    }
}

/// Dialogue content owned one-to-one by an application expression.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueContent {
    id: HirDialogueContentId,
    nodes: Box<[HirDialogueNode]>,
    tags: Box<[HirRichTextTag]>,
    marks: Box<[HirDialogueMark]>,
}

impl HirDialogueContent {
    pub(crate) fn try_new(
        id: HirDialogueContentId,
        nodes: Box<[HirDialogueNode]>,
        tags: Box<[HirRichTextTag]>,
        mark_inputs: Box<[(HirRichTextTagId, HirDialogueMarkName)]>,
    ) -> Result<Self, HirDialogueInvariantError> {
        Self::try_new_with_mark_maximum(
            id,
            nodes,
            tags,
            mark_inputs,
            HirLimit::DialogueMarksPerContent.maximum(),
        )
    }

    fn try_new_with_mark_maximum(
        id: HirDialogueContentId,
        nodes: Box<[HirDialogueNode]>,
        mut tags: Box<[HirRichTextTag]>,
        mark_inputs: Box<[(HirRichTextTagId, HirDialogueMarkName)]>,
        maximum_marks: usize,
    ) -> Result<Self, HirDialogueInvariantError> {
        let marks = mint_mark_catalog(id, &mut tags, &mark_inputs, maximum_marks)?;
        validate_content_ids(id, &nodes, &tags, &marks)?;
        Ok(Self {
            id,
            nodes,
            tags,
            marks,
        })
    }

    #[cfg(test)]
    pub(crate) fn try_new_with_mark_limit_for_test(
        id: HirDialogueContentId,
        nodes: Box<[HirDialogueNode]>,
        tags: Box<[HirRichTextTag]>,
        mark_inputs: Box<[(HirRichTextTagId, HirDialogueMarkName)]>,
        maximum_marks: usize,
    ) -> Result<Self, HirDialogueInvariantError> {
        Self::try_new_with_mark_maximum(id, nodes, tags, mark_inputs, maximum_marks)
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

    /// Returns the source-ordered marker catalog owned by this content.
    pub const fn marks(&self) -> &[HirDialogueMark] {
        &self.marks
    }

    /// Resolves one marker name within this content's own catalog.
    pub(crate) fn mark_by_name(&self, name: &HirDialogueMarkName) -> Option<HirDialogueMarkId> {
        self.marks
            .iter()
            .find(|mark| mark.name() == name)
            .map(HirDialogueMark::id)
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
    marks: &[HirDialogueMark],
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

        match tag.payload() {
            HirRichTextTagPayload::Marker(mark) => {
                if !matches!(tag.identity(), HirRichTextTagIdentity::Marker)
                    || mark.content() != content
                    || !marks
                        .iter()
                        .any(|row| row.id() == *mark && row.tag() == tag.id())
                {
                    return Err(HirDialogueInvariantError::InvalidMarkReference);
                }
            }
            _ if matches!(tag.identity(), HirRichTextTagIdentity::Marker) => {
                return Err(HirDialogueInvariantError::InvalidMarkReference);
            }
            _ => {}
        }
    }

    let mut names = BTreeSet::new();
    for (ordinal, mark) in marks.iter().enumerate() {
        let expected = HirDialogueMarkOrdinal::try_new(ordinal)
            .map_err(|_| HirDialogueInvariantError::ArithmeticOverflow)?;
        if mark.id().content() != content || mark.id().ordinal() != expected {
            return Err(HirDialogueInvariantError::NonContiguousMarkOrdinal);
        }
        let Some(tag) = tags.get(mark.tag().ordinal() as usize) else {
            return Err(HirDialogueInvariantError::InvalidMarkReference);
        };
        if mark.tag().content() != content
            || tag.id() != mark.tag()
            || !matches!(tag.identity(), HirRichTextTagIdentity::Marker)
            || !matches!(tag.payload(), HirRichTextTagPayload::Marker(id) if *id == mark.id())
        {
            return Err(HirDialogueInvariantError::InvalidMarkReference);
        }
        if !names.insert(mark.name()) {
            return Err(HirDialogueInvariantError::DuplicateMarkName);
        }
    }
    let mut marker_ordinal = 0usize;
    for tag in tags {
        if !matches!(tag.identity(), HirRichTextTagIdentity::Marker) {
            continue;
        }
        let expected = HirDialogueMarkOrdinal::try_new(marker_ordinal)
            .map_err(|_| HirDialogueInvariantError::ArithmeticOverflow)?;
        let Some(mark) = marks.iter().find(|mark| mark.tag() == tag.id()) else {
            return Err(HirDialogueInvariantError::InvalidMarkReference);
        };
        if mark.id().ordinal() != expected {
            return Err(HirDialogueInvariantError::NonContiguousMarkOrdinal);
        }
        marker_ordinal = marker_ordinal
            .checked_add(1)
            .ok_or(HirDialogueInvariantError::ArithmeticOverflow)?;
    }
    if marker_ordinal != marks.len() {
        return Err(HirDialogueInvariantError::InvalidMarkReference);
    }
    Ok(())
}

fn mint_mark_catalog(
    content: HirDialogueContentId,
    tags: &mut [HirRichTextTag],
    inputs: &[(HirRichTextTagId, HirDialogueMarkName)],
    maximum_marks: usize,
) -> Result<Box<[HirDialogueMark]>, HirDialogueInvariantError> {
    let mut charge = HirDialogueMarkCatalogCharge::new(maximum_marks);
    let mut names = BTreeSet::<&HirDialogueMarkName>::new();
    let mut tag_ids = BTreeSet::new();
    let mut marks = Vec::new();
    for (ordinal, (tag_id, name)) in inputs.iter().enumerate() {
        let mark_ordinal = HirDialogueMarkOrdinal::try_new(ordinal)
            .map_err(|_| HirDialogueInvariantError::ArithmeticOverflow)?;
        let id = HirDialogueMarkId::new(content, mark_ordinal);
        let tag = tags
            .get(tag_id.ordinal() as usize)
            .filter(|tag| tag.id() == *tag_id)
            .ok_or(HirDialogueInvariantError::InvalidMarkReference)?;
        if tag_id.content() != content
            || !matches!(tag.identity(), HirRichTextTagIdentity::Marker)
            || !matches!(tag.payload(), HirRichTextTagPayload::None)
            || !tag.arguments().is_empty()
            || tag_ids.contains(tag_id)
        {
            return Err(HirDialogueInvariantError::InvalidMarkReference);
        }
        if tag_id.ordinal() as usize != ordinal {
            return Err(HirDialogueInvariantError::NonContiguousMarkOrdinal);
        }
        if names.contains(name) {
            return Err(HirDialogueInvariantError::DuplicateMarkName);
        }
        charge.charge()?;
        let inserted = names.insert(name);
        debug_assert!(inserted);
        let inserted = tag_ids.insert(*tag_id);
        debug_assert!(inserted);
        marks.push(HirDialogueMark::new(id, name.clone(), *tag_id));
    }
    for mark in &marks {
        tags[mark.tag().ordinal() as usize].set_marker_id(mark.id())?;
    }
    Ok(marks.into_boxed_slice())
}

struct HirDialogueMarkCatalogCharge {
    maximum: usize,
    charged: usize,
}

impl HirDialogueMarkCatalogCharge {
    const fn new(maximum: usize) -> Self {
        Self {
            maximum,
            charged: 0,
        }
    }

    fn charge(&mut self) -> Result<(), HirDialogueInvariantError> {
        let observed = self
            .charged
            .checked_add(1)
            .ok_or(HirDialogueInvariantError::ArithmeticOverflow)?;
        if observed > self.maximum {
            return Err(HirDialogueInvariantError::MarkCatalogLimitExceeded {
                observed,
                maximum: self.maximum,
            });
        }
        self.charged = observed;
        Ok(())
    }
}

#[cfg(test)]
mod mark_limit_tests {
    use super::*;

    #[test]
    fn mark_charge_arithmetic_overflow_is_typed_and_non_mutating() {
        let mut charge = HirDialogueMarkCatalogCharge {
            maximum: usize::MAX,
            charged: usize::MAX,
        };
        assert_eq!(
            charge.charge(),
            Err(HirDialogueInvariantError::ArithmeticOverflow)
        );
        assert_eq!(charge.charged, usize::MAX);
    }
}
