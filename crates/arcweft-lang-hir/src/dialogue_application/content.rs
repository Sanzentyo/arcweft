//! Complete dialogue-content records owned by one expression.
//!
//! Dialogue content is a source-ordered node stream. Zero-width bracket
//! actions are carried by their node; body-bearing operations are represented
//! by the attached content-call expression that owns their body.

use std::collections::BTreeSet;

use super::rich_text::{
    HirDialogueControl, HirRichTextArgumentIssue, HirRichTextHostEvent, HirRichTextValue,
};
use super::{
    HirDialogueExpressionExpectation, HirDialogueInvariantError, HirDialogueOrdinalError,
    HirDialogueTransactionContext, HirDialogueTransactionError, HirDialogueTransactionRequirement,
    HirRichTextCharge, validate_module,
};
use crate::identity::{ExprId, HirLimit, HirModuleId};
use crate::leaf::HirIdSuffix;
use arcweft_lang_syntax::expressions::{
    SyntaxDialogueContentIssue, SyntaxDialogueControl, SyntaxLineBreakKind,
};

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
    action: HirDialogueNodeId,
}

impl HirDialogueMark {
    const fn new(
        id: HirDialogueMarkId,
        name: HirDialogueMarkName,
        action: HirDialogueNodeId,
    ) -> Self {
        Self { id, name, action }
    }

    /// Returns the content-qualified marker identity.
    pub const fn id(&self) -> HirDialogueMarkId {
        self.id
    }

    /// Returns the marker's validated local name.
    pub const fn name(&self) -> &HirDialogueMarkName {
        &self.name
    }

    /// Returns the point-action node that introduced this marker.
    pub const fn action(&self) -> HirDialogueNodeId {
        self.action
    }
}

/// Opaque decoded body retained by the canonical `#raw()[...]` content call.
///
/// This carrier is intentionally not a dialogue node stream.  Brackets and
/// control-looking bytes therefore remain literal and cannot be reparsed.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRawLiteralBody(Box<str>);

impl HirRawLiteralBody {
    pub(crate) const fn new(value: Box<str>) -> Self {
        Self(value)
    }

    /// Returns the decoded literal body.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Complete typed zero-width bracket action.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialoguePointAction {
    id: HirDialogueNodeId,
    identity: HirDialoguePointActionIdentity,
    arguments: Box<[HirDialoguePointActionArgument]>,
    payload: HirDialoguePointActionPayload,
}

impl HirDialoguePointAction {
    pub(crate) fn try_new(
        id: HirDialogueNodeId,
        identity: HirDialoguePointActionIdentity,
        arguments: Box<[HirDialoguePointActionArgument]>,
        payload: HirDialoguePointActionPayload,
    ) -> Result<Self, HirDialogueInvariantError> {
        for (ordinal, argument) in arguments.iter().enumerate() {
            let expected = u16::try_from(ordinal)
                .map_err(|_| HirDialogueInvariantError::ArithmeticOverflow)?;
            if argument.id().action() != id || argument.id().ordinal() != expected {
                return Err(HirDialogueInvariantError::InvalidArgumentReference);
            }
        }
        let action = Self {
            id,
            identity,
            arguments,
            payload,
        };
        action
            .validate_module(id.content().owner().module())
            .map_err(|actual| HirDialogueInvariantError::ForeignChild {
                expected: id.content().owner().module(),
                actual,
            })?;
        Ok(action)
    }

    /// Returns the point-action node identity.
    pub const fn id(&self) -> HirDialogueNodeId {
        self.id
    }

    /// Returns the typed action identity.
    pub const fn identity(&self) -> &HirDialoguePointActionIdentity {
        &self.identity
    }

    /// Returns source-ordered value arguments.
    pub const fn arguments(&self) -> &[HirDialoguePointActionArgument] {
        &self.arguments
    }

    /// Returns the optional expression payload.
    pub const fn payload(&self) -> HirDialoguePointActionPayload {
        self.payload
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirModuleId> {
        if let Some(expression) = self.payload.expression() {
            validate_module(expected, expression.module())?;
        }
        Ok(())
    }

    fn validate_transaction<C: HirDialogueTransactionContext>(
        &self,
        context: &mut C,
    ) -> Result<(), HirDialogueTransactionError<C::Error>> {
        for argument in &self.arguments {
            if let Some(name) = argument.name() {
                context
                    .require(HirDialogueTransactionRequirement::RichTextCharge(
                        HirRichTextCharge::ArgumentKeyBytes {
                            observed: name.len(),
                        },
                    ))
                    .map_err(HirDialogueTransactionError::Context)?;
            }
            if let Some(value) = argument.value() {
                context
                    .require(HirDialogueTransactionRequirement::RichTextCharge(
                        HirRichTextCharge::ArgumentValueDecodedBytes {
                            observed: value.as_str().len(),
                        },
                    ))
                    .map_err(HirDialogueTransactionError::Context)?;
            }
        }
        if let Some(expression) = self.payload.expression() {
            context
                .require(HirDialogueTransactionRequirement::Expression {
                    id: expression,
                    expected: HirDialogueExpressionExpectation::Call,
                })
                .map_err(HirDialogueTransactionError::Context)?;
        }
        Ok(())
    }

    fn has_recovery(&self) -> bool {
        self.arguments
            .iter()
            .any(|argument| argument.issue().is_some())
    }
}

/// Typed point-action identity admitted by bracket dialogue syntax.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDialoguePointActionIdentity {
    Control(HirDialogueControl),
    Mark(HirDialogueMarkName),
    Host(HirRichTextHostEvent),
}

/// Optional call payload owned by a point action.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDialoguePointActionPayload {
    None,
    Call(ExprId),
    TimedCue(ExprId),
}

impl HirDialoguePointActionPayload {
    pub const fn expression(self) -> Option<ExprId> {
        match self {
            Self::Call(expression) | Self::TimedCue(expression) => Some(expression),
            Self::None => None,
        }
    }
}

/// Content-local point-action argument identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialoguePointActionArgumentId {
    action: HirDialogueNodeId,
    ordinal: u16,
}

impl HirDialoguePointActionArgumentId {
    pub(crate) fn try_new(
        action: HirDialogueNodeId,
        ordinal: usize,
    ) -> Result<Self, HirDialogueOrdinalError> {
        if ordinal >= 32 {
            return Err(HirDialogueOrdinalError::Argument { ordinal });
        }
        u16::try_from(ordinal)
            .map(|ordinal| Self { action, ordinal })
            .map_err(|_| HirDialogueOrdinalError::Argument { ordinal })
    }

    pub const fn action(self) -> HirDialogueNodeId {
        self.action
    }

    pub const fn ordinal(self) -> u16 {
        self.ordinal
    }
}

/// One point-action argument, retained as typed decoded data.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDialoguePointActionArgument {
    Positional {
        id: HirDialoguePointActionArgumentId,
        value: HirRichTextValue,
    },
    Named {
        id: HirDialoguePointActionArgumentId,
        name: Box<str>,
        value: HirRichTextValue,
    },
    Invalid {
        id: HirDialoguePointActionArgumentId,
        issue: HirRichTextArgumentIssue,
    },
}

impl HirDialoguePointActionArgument {
    pub(crate) const fn positional(
        id: HirDialoguePointActionArgumentId,
        value: HirRichTextValue,
    ) -> Self {
        Self::Positional { id, value }
    }

    pub(crate) const fn named(
        id: HirDialoguePointActionArgumentId,
        name: Box<str>,
        value: HirRichTextValue,
    ) -> Self {
        Self::Named { id, name, value }
    }

    pub(crate) const fn invalid(
        id: HirDialoguePointActionArgumentId,
        issue: HirRichTextArgumentIssue,
    ) -> Self {
        Self::Invalid { id, issue }
    }

    pub const fn id(&self) -> HirDialoguePointActionArgumentId {
        match self {
            Self::Positional { id, .. } | Self::Named { id, .. } | Self::Invalid { id, .. } => *id,
        }
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Named { name, .. } => Some(name),
            Self::Positional { .. } | Self::Invalid { .. } => None,
        }
    }

    pub const fn value(&self) -> Option<&HirRichTextValue> {
        match self {
            Self::Positional { value, .. } | Self::Named { value, .. } => Some(value),
            Self::Invalid { .. } => None,
        }
    }

    pub const fn issue(&self) -> Option<HirRichTextArgumentIssue> {
        match self {
            Self::Invalid { issue, .. } => Some(*issue),
            Self::Positional { .. } | Self::Named { .. } => None,
        }
    }
}

/// Dialogue content owned one-to-one by an application expression.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueContent {
    id: HirDialogueContentId,
    nodes: Box<[HirDialogueNode]>,
    raw_literal: Option<HirRawLiteralBody>,
    marks: Box<[HirDialogueMark]>,
}

impl HirDialogueContent {
    pub(crate) fn try_new(
        id: HirDialogueContentId,
        nodes: Box<[HirDialogueNode]>,
        mark_inputs: Box<[(HirDialogueNodeId, HirDialogueMarkName)]>,
    ) -> Result<Self, HirDialogueInvariantError> {
        Self::try_new_with_mark_maximum(
            id,
            nodes,
            None,
            mark_inputs,
            HirLimit::DialogueMarksPerContent.maximum(),
        )
    }

    pub(crate) fn try_new_raw_literal(
        id: HirDialogueContentId,
        literal: HirRawLiteralBody,
    ) -> Result<Self, HirDialogueInvariantError> {
        Self::try_new_with_mark_maximum(
            id,
            Box::new([]),
            Some(literal),
            Box::new([]),
            HirLimit::DialogueMarksPerContent.maximum(),
        )
    }

    fn try_new_with_mark_maximum(
        id: HirDialogueContentId,
        nodes: Box<[HirDialogueNode]>,
        raw_literal: Option<HirRawLiteralBody>,
        mark_inputs: Box<[(HirDialogueNodeId, HirDialogueMarkName)]>,
        maximum_marks: usize,
    ) -> Result<Self, HirDialogueInvariantError> {
        if raw_literal.is_some() && !nodes.is_empty() {
            return Err(HirDialogueInvariantError::InvalidContentOwner);
        }
        let marks = mint_mark_catalog(id, &nodes, &mark_inputs, maximum_marks)?;
        validate_content_ids(id, &nodes, raw_literal.as_ref(), &marks)?;
        Ok(Self {
            id,
            nodes,
            raw_literal,
            marks,
        })
    }

    /// Returns the application-owned content identity.
    pub const fn id(&self) -> HirDialogueContentId {
        self.id
    }

    /// Returns source-ordered dialogue nodes. Raw literal bodies have no nodes.
    pub const fn nodes(&self) -> &[HirDialogueNode] {
        &self.nodes
    }

    /// Returns the dedicated opaque raw-literal body, when this is `#raw()`.
    pub const fn raw_literal(&self) -> Option<&HirRawLiteralBody> {
        self.raw_literal.as_ref()
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
        for node in &self.nodes {
            node.validate_module(expected)?;
        }
        Ok(())
    }

    pub(super) fn validate_transaction<C: HirDialogueTransactionContext>(
        &self,
        context: &mut C,
    ) -> Result<(), HirDialogueTransactionError<C::Error>> {
        let action_count = self
            .nodes
            .iter()
            .filter(|node| matches!(node.kind(), HirDialogueNodeKind::PointAction(_)))
            .count();
        context
            .require(HirDialogueTransactionRequirement::RichTextCharge(
                HirRichTextCharge::PointActions {
                    observed: action_count,
                },
            ))
            .map_err(HirDialogueTransactionError::Context)?;
        let argument_count = self
            .nodes
            .iter()
            .filter_map(|node| match node.kind() {
                HirDialogueNodeKind::PointAction(action) => Some(action.arguments().len()),
                _ => None,
            })
            .try_fold(0usize, usize::checked_add)
            .ok_or(HirDialogueTransactionError::Invariant(
                HirDialogueInvariantError::ArithmeticOverflow,
            ))?;
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
        Ok(())
    }

    pub(super) fn has_recovery(&self) -> bool {
        self.nodes.iter().any(HirDialogueNode::has_recovery)
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
            HirDialogueNodeKind::Interpolation(expression)
            | HirDialogueNodeKind::ContentApplication(expression) => {
                validate_module(expected, expression.module())
            }
            HirDialogueNodeKind::PointAction(action) => action.validate_module(expected),
            _ => Ok(()),
        }
    }

    pub(super) fn validate_transaction<C: HirDialogueTransactionContext>(
        &self,
        context: &mut C,
    ) -> Result<(), HirDialogueTransactionError<C::Error>> {
        match self.kind {
            HirDialogueNodeKind::Interpolation(expression) => context
                .require(HirDialogueTransactionRequirement::Expression {
                    id: expression,
                    expected: HirDialogueExpressionExpectation::Unrestricted,
                })
                .map_err(HirDialogueTransactionError::Context),
            HirDialogueNodeKind::ContentApplication(expression) => context
                .require(HirDialogueTransactionRequirement::Expression {
                    id: expression,
                    expected: HirDialogueExpressionExpectation::ContentApplication,
                })
                .map_err(HirDialogueTransactionError::Context),
            HirDialogueNodeKind::PointAction(ref action) => action.validate_transaction(context),
            _ => Ok(()),
        }
    }

    pub(super) fn has_recovery(&self) -> bool {
        match &self.kind {
            HirDialogueNodeKind::PointAction(action) => action.has_recovery(),
            HirDialogueNodeKind::Error(_) => true,
            _ => false,
        }
    }
}

/// Exhaustive typed dialogue node families.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDialogueNodeKind {
    Text(HirTextFragment),
    Escape(char),
    Interpolation(ExprId),
    ContentApplication(ExprId),
    PointAction(HirDialoguePointAction),
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
    InvalidPointAction,
}

impl From<SyntaxDialogueContentIssue> for HirDialogueContentError {
    fn from(value: SyntaxDialogueContentIssue) -> Self {
        match value {
            SyntaxDialogueContentIssue::UnclassifiedToken => Self::UnclassifiedToken,
            SyntaxDialogueContentIssue::InvalidPointAction => Self::InvalidPointAction,
        }
    }
}

/// Public semantic issue vocabulary for dialogue content.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDialogueIssue {
    ForeignChild,
    DuplicateNodeId,
    NonContiguousNodeOrdinal,
    InvalidArgumentReference,
    InvalidMarkReference,
    InvalidPlan,
}

fn validate_content_ids(
    content: HirDialogueContentId,
    nodes: &[HirDialogueNode],
    raw_literal: Option<&HirRawLiteralBody>,
    marks: &[HirDialogueMark],
) -> Result<(), HirDialogueInvariantError> {
    if raw_literal.is_some() && !nodes.is_empty() {
        return Err(HirDialogueInvariantError::InvalidContentOwner);
    }
    for (ordinal, node) in nodes.iter().enumerate() {
        let expected =
            u32::try_from(ordinal).map_err(|_| HirDialogueInvariantError::ArithmeticOverflow)?;
        if node.id.content != content || node.id.ordinal != expected {
            return Err(HirDialogueInvariantError::NonContiguousNodeOrdinal);
        }
        if let HirDialogueNodeKind::PointAction(action) = node.kind() {
            if action.id() != node.id {
                return Err(HirDialogueInvariantError::InvalidArgumentReference);
            }
        }
    }

    let mut names = BTreeSet::new();
    for (ordinal, mark) in marks.iter().enumerate() {
        let expected = HirDialogueMarkOrdinal::try_new(ordinal)
            .map_err(|_| HirDialogueInvariantError::ArithmeticOverflow)?;
        if mark.id().content() != content || mark.id().ordinal() != expected {
            return Err(HirDialogueInvariantError::NonContiguousMarkOrdinal);
        }
        let Some(node) = nodes.get(mark.action().ordinal() as usize) else {
            return Err(HirDialogueInvariantError::InvalidMarkReference);
        };
        if mark.action().content() != content
            || node.id() != mark.action()
            || !matches!(
                node.kind(),
                HirDialogueNodeKind::PointAction(HirDialoguePointAction {
                    identity: HirDialoguePointActionIdentity::Mark(_),
                    ..
                })
            )
        {
            return Err(HirDialogueInvariantError::InvalidMarkReference);
        }
        if !names.insert(mark.name()) {
            return Err(HirDialogueInvariantError::DuplicateMarkName);
        }
    }
    let mut marker_ordinal = 0usize;
    for node in nodes {
        let HirDialogueNodeKind::PointAction(action) = node.kind() else {
            continue;
        };
        if !matches!(action.identity(), HirDialoguePointActionIdentity::Mark(_)) {
            continue;
        }
        let expected = HirDialogueMarkOrdinal::try_new(marker_ordinal)
            .map_err(|_| HirDialogueInvariantError::ArithmeticOverflow)?;
        let Some(mark) = marks.iter().find(|mark| mark.action() == node.id()) else {
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
    nodes: &[HirDialogueNode],
    inputs: &[(HirDialogueNodeId, HirDialogueMarkName)],
    maximum_marks: usize,
) -> Result<Box<[HirDialogueMark]>, HirDialogueInvariantError> {
    let mut charge = HirDialogueMarkCatalogCharge::new(maximum_marks);
    let mut names = BTreeSet::<&HirDialogueMarkName>::new();
    let mut action_ids = BTreeSet::new();
    let mut marks = Vec::new();
    for (ordinal, (action_id, name)) in inputs.iter().enumerate() {
        let mark_ordinal = HirDialogueMarkOrdinal::try_new(ordinal)
            .map_err(|_| HirDialogueInvariantError::ArithmeticOverflow)?;
        let id = HirDialogueMarkId::new(content, mark_ordinal);
        let action = nodes
            .get(action_id.ordinal() as usize)
            .filter(|node| node.id() == *action_id)
            .ok_or(HirDialogueInvariantError::InvalidMarkReference)?;
        if action_id.content() != content
            || !matches!(
                action.kind(),
                HirDialogueNodeKind::PointAction(HirDialoguePointAction {
                    identity: HirDialoguePointActionIdentity::Mark(_),
                    ..
                })
            )
            || action_ids.contains(action_id)
        {
            return Err(HirDialogueInvariantError::InvalidMarkReference);
        }
        if action_id.ordinal() as usize >= nodes.len() || names.contains(name) {
            return Err(if names.contains(name) {
                HirDialogueInvariantError::DuplicateMarkName
            } else {
                HirDialogueInvariantError::InvalidMarkReference
            });
        }
        charge.charge()?;
        names.insert(name);
        action_ids.insert(*action_id);
        marks.push(HirDialogueMark::new(id, name.clone(), *action_id));
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

impl From<SyntaxDialogueControl> for HirDialogueControl {
    fn from(value: SyntaxDialogueControl) -> Self {
        match value {
            SyntaxDialogueControl::Page => Self::Page,
            SyntaxDialogueControl::LineWait => Self::LineWait,
            SyntaxDialogueControl::HardBreak => Self::HardBreak,
            SyntaxDialogueControl::TimedWait => Self::TimedWait,
            SyntaxDialogueControl::Clear => Self::Clear,
            SyntaxDialogueControl::Reset => Self::Reset,
            SyntaxDialogueControl::Speed => Self::Speed,
        }
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
