//! Rich-text tag, argument, identity, payload, and issue records.

use super::content::HirDialogueContentId;
use super::{
    HirDialogueExpressionExpectation, HirDialogueInvariantError, HirDialogueOrdinalError,
    HirDialogueTransactionContext, HirDialogueTransactionError, HirDialogueTransactionRequirement,
    HirRichTextCharge, validate_module,
};
use crate::identity::{ExprId, HirModuleId, ItemId};
use crate::leaf::{HirName, HirPath, HirProjectSymbolSegment};
use arcweft_lang_syntax::expressions::{
    SyntaxBuiltinRichTextFx, SyntaxBuiltinRichTextTag, SyntaxRichTextConditionalTag,
    SyntaxRichTextDirectStyle, SyntaxRichTextHostEvent, SyntaxRichTextIssue,
    SyntaxRichTextLayoutSelector, SyntaxRichTextObjectSelector, SyntaxRichTextStyleSelector,
    SyntaxRichTextTransformSelector,
};
use arcweft_lang_syntax::text::RichTextArgumentIssue;

/// Contiguous `RichText`-tag identity local to one dialogue content value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRichTextTagId {
    content: HirDialogueContentId,
    ordinal: u32,
}

impl HirRichTextTagId {
    pub(crate) fn try_new(
        content: HirDialogueContentId,
        ordinal: usize,
    ) -> Result<Self, HirDialogueOrdinalError> {
        u32::try_from(ordinal)
            .map(|ordinal| Self { content, ordinal })
            .map_err(|_| HirDialogueOrdinalError::Tag { ordinal })
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

/// Contiguous argument identity local to one `RichText` tag.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRichTextArgumentId {
    tag: HirRichTextTagId,
    ordinal: u16,
}

impl HirRichTextArgumentId {
    pub(crate) fn try_new(
        tag: HirRichTextTagId,
        ordinal: usize,
    ) -> Result<Self, HirDialogueOrdinalError> {
        if ordinal >= 32 {
            return Err(HirDialogueOrdinalError::Argument { ordinal });
        }
        u16::try_from(ordinal)
            .map(|ordinal| Self { tag, ordinal })
            .map_err(|_| HirDialogueOrdinalError::Argument { ordinal })
    }

    /// Returns the owning `RichText` tag identity.
    pub const fn tag(self) -> HirRichTextTagId {
        self.tag
    }

    /// Returns the zero-based argument ordinal.
    pub const fn ordinal(self) -> u16 {
        self.ordinal
    }
}

/// One authored or inferred end-tag record.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRichTextEndTag {
    paired_start: Option<HirRichTextTagId>,
    identity: Option<HirRichTextTagIdentity>,
    inferred: bool,
    issue: Option<HirRichTextIssue>,
}

impl HirRichTextEndTag {
    pub(crate) const fn new(
        paired_start: Option<HirRichTextTagId>,
        identity: Option<HirRichTextTagIdentity>,
        inferred: bool,
        issue: Option<HirRichTextIssue>,
    ) -> Self {
        Self {
            paired_start,
            identity,
            inferred,
            issue,
        }
    }

    /// Returns the syntax-owned opening tag paired with this end tag.
    ///
    /// This identity is the semantic pairing authority. The authored end-tag
    /// name may denote a family such as `effect`, while the opening tag owns a
    /// concrete selector such as `.wave`; consumers must not compare or
    /// reconstruct those spellings.
    pub const fn paired_start(&self) -> Option<HirRichTextTagId> {
        self.paired_start
    }

    /// Returns the resolved or retained tag identity, when present.
    pub const fn identity(&self) -> Option<&HirRichTextTagIdentity> {
        self.identity.as_ref()
    }

    /// Returns whether this end tag was inserted by typed syntax.
    pub const fn is_inferred(&self) -> bool {
        self.inferred
    }

    /// Returns the retained typed issue, when present.
    pub const fn issue(&self) -> Option<&HirRichTextIssue> {
        self.issue.as_ref()
    }

    pub(super) fn validate_module(&self, expected: HirModuleId) -> Result<(), HirModuleId> {
        if let Some(identity) = &self.identity {
            identity.validate_module(expected)?;
        }
        Ok(())
    }
}

/// One content-owned `RichText` tag and its sole argument slice.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRichTextTag {
    id: HirRichTextTagId,
    identity: HirRichTextTagIdentity,
    arguments: Box<[HirRichTextArgument]>,
    payload: HirRichTextTagPayload,
}

impl HirRichTextTag {
    pub(crate) fn try_new(
        id: HirRichTextTagId,
        identity: HirRichTextTagIdentity,
        arguments: Box<[HirRichTextArgument]>,
        payload: HirRichTextTagPayload,
    ) -> Result<Self, HirDialogueInvariantError> {
        validate_argument_ids(id, &arguments)?;
        let tag = Self {
            id,
            identity,
            arguments,
            payload,
        };
        tag.validate_module(id.content.owner().module())
            .map_err(|actual| HirDialogueInvariantError::ForeignChild {
                expected: id.content.owner().module(),
                actual,
            })?;
        Ok(tag)
    }

    /// Returns this tag's content-local identity.
    pub const fn id(&self) -> HirRichTextTagId {
        self.id
    }

    /// Returns the canonical semantic tag identity.
    pub const fn identity(&self) -> &HirRichTextTagIdentity {
        &self.identity
    }

    /// Returns ordered arguments owned solely by this tag.
    pub const fn arguments(&self) -> &[HirRichTextArgument] {
        &self.arguments
    }

    /// Returns the typed payload family.
    pub const fn payload(&self) -> &HirRichTextTagPayload {
        &self.payload
    }

    pub(super) fn validate_module(&self, expected: HirModuleId) -> Result<(), HirModuleId> {
        validate_module(expected, self.id.content.owner().module())?;
        self.identity.validate_module(expected)?;
        if let Some(expression) = self.payload.expression() {
            validate_module(expected, expression.module())?;
        }
        Ok(())
    }

    pub(super) fn validate_transaction<C: HirDialogueTransactionContext>(
        &self,
        context: &mut C,
    ) -> Result<(), HirDialogueTransactionError<C::Error>> {
        context
            .require(HirDialogueTransactionRequirement::RichTextCharge(
                HirRichTextCharge::TagArguments {
                    observed: self.arguments.len(),
                },
            ))
            .map_err(HirDialogueTransactionError::Context)?;
        if let HirRichTextTagIdentity::Registered(HirRegisteredRichTextTagId::Project(item)) =
            self.identity
        {
            context
                .require(HirDialogueTransactionRequirement::Item(item))
                .map_err(HirDialogueTransactionError::Context)?;
        }
        for argument in &self.arguments {
            if let Some(name) = argument.name() {
                context
                    .require(HirDialogueTransactionRequirement::RichTextCharge(
                        HirRichTextCharge::ArgumentKeyBytes {
                            observed: name.as_str().len(),
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
            let expected = match self.payload {
                HirRichTextTagPayload::FxCall(_) | HirRichTextTagPayload::DialogueCall(_) => {
                    HirDialogueExpressionExpectation::Call
                }
                HirRichTextTagPayload::Condition(_) => HirDialogueExpressionExpectation::Any,
                HirRichTextTagPayload::Arguments | HirRichTextTagPayload::None => unreachable!(),
            };
            context
                .require(HirDialogueTransactionRequirement::Expression {
                    id: expression,
                    expected,
                })
                .map_err(HirDialogueTransactionError::Context)?;
        }
        Ok(())
    }

    pub(super) fn has_recovery(&self) -> bool {
        matches!(self.identity, HirRichTextTagIdentity::Unresolved(_))
            || self
                .arguments
                .iter()
                .any(|argument| matches!(argument, HirRichTextArgument::Invalid { .. }))
    }
}

/// Canonical semantic `RichText` tag identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRichTextTagIdentity {
    Builtin(HirBuiltinRichTextTag),
    Registered(HirRegisteredRichTextTagId),
    Unresolved(HirUnresolvedRichTextTag),
}

impl HirRichTextTagIdentity {
    pub(super) fn validate_module(&self, expected: HirModuleId) -> Result<(), HirModuleId> {
        if let Self::Registered(HirRegisteredRichTextTagId::Project(item)) = self {
            validate_module(expected, item.module())?;
        }
        Ok(())
    }
}

/// Closed canonical builtin `RichText` inventory.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirBuiltinRichTextTag {
    Page,
    LineWait,
    HardBreak,
    TimedWait,
    Clear,
    Reset,
    Speed,
    Marker,
    DirectStyle(HirRichTextDirectStyle),
    Style(HirRichTextStyleSelector),
    Layout(HirRichTextLayoutSelector),
    Transform(HirRichTextTransformSelector),
    Object(HirRichTextObjectSelector),
    Fx(HirBuiltinRichTextFx),
    HostEvent(HirRichTextHostEvent),
    Conditional(HirRichTextConditionalTag),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRichTextDirectStyle {
    Emphasis,
    Strong,
    Italic,
    Oblique,
    Color,
    Font,
    Size,
    Ruby,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRichTextStyleSelector {
    Italic,
    Oblique,
    Opacity,
    Layer,
    ZIndex,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRichTextLayoutSelector {
    HorizontalTb,
    VerticalRl,
    VerticalLr,
    Direction,
    RubyOver,
    RubyUnder,
    RubyInterCharacter,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRichTextTransformSelector {
    Offset,
    Rotate,
    Scale,
    Skew,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRichTextObjectSelector {
    Object,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirBuiltinRichTextFx {
    Wave,
    Shake,
    Jitter,
    Arc,
    Spin,
    Pulse,
    Motion,
    Typewriter,
    Sparkle,
    Shader,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRichTextHostEvent {
    Voice,
    Face,
    Pose,
    Show,
    Hide,
    Move,
    Scale,
    Rotate,
    Animation,
    StageShake,
    TimedCue,
    Call,
    Signal,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRichTextConditionalTag {
    If,
    Else,
    EndIf,
}

/// Project-local or external registered `RichText` identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRegisteredRichTextTagId {
    Project(ItemId),
    External(HirExternalSymbolId),
}

/// External project segment plus a root-preserving semantic path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirExternalSymbolId {
    project: HirProjectSymbolSegment,
    path: HirPath,
}

impl HirExternalSymbolId {
    /// Returns the external project segment.
    pub const fn project(&self) -> &HirProjectSymbolSegment {
        &self.project
    }

    /// Returns the root-preserving path inside that project.
    pub const fn path(&self) -> &HirPath {
        &self.path
    }
}

/// Validated unresolved tag identity and its typed issue.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirUnresolvedRichTextTag {
    name: HirProjectSymbolSegment,
    issue: HirRichTextIssue,
}

impl HirUnresolvedRichTextTag {
    pub(crate) const fn new(name: HirProjectSymbolSegment, issue: HirRichTextIssue) -> Self {
        Self { name, issue }
    }

    /// Returns the validated unresolved segment.
    pub const fn name(&self) -> &HirProjectSymbolSegment {
        &self.name
    }

    /// Returns the typed resolution issue.
    pub const fn issue(&self) -> &HirRichTextIssue {
        &self.issue
    }
}

/// `RichText` tag payload using the same expression arena as its application.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRichTextTagPayload {
    Arguments,
    FxCall(ExprId),
    DialogueCall(ExprId),
    Condition(ExprId),
    None,
}

impl HirRichTextTagPayload {
    /// Returns the expression owned by expression-bearing payload families.
    ///
    /// The returned identity belongs to the same module arena as the owning
    /// dialogue-content application; consumers must resolve it through that
    /// module rather than reconstructing or reparsing source text.
    pub const fn expression(&self) -> Option<ExprId> {
        match self {
            Self::FxCall(expression)
            | Self::DialogueCall(expression)
            | Self::Condition(expression) => Some(*expression),
            Self::Arguments | Self::None => None,
        }
    }
}

/// One ordered `RichText` argument record.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRichTextArgument {
    Positional {
        id: HirRichTextArgumentId,
        value: HirRichTextValue,
    },
    Named {
        id: HirRichTextArgumentId,
        name: HirName,
        value: HirRichTextValue,
    },
    Invalid {
        id: HirRichTextArgumentId,
        issue: HirRichTextArgumentIssue,
    },
}

impl HirRichTextArgument {
    pub(crate) const fn positional(id: HirRichTextArgumentId, value: HirRichTextValue) -> Self {
        Self::Positional { id, value }
    }

    pub(crate) const fn named(
        id: HirRichTextArgumentId,
        name: HirName,
        value: HirRichTextValue,
    ) -> Self {
        Self::Named { id, name, value }
    }

    pub(crate) const fn invalid(
        id: HirRichTextArgumentId,
        issue: HirRichTextArgumentIssue,
    ) -> Self {
        Self::Invalid { id, issue }
    }

    /// Returns the argument's tag-local identity.
    pub const fn id(&self) -> HirRichTextArgumentId {
        match self {
            Self::Positional { id, .. } | Self::Named { id, .. } | Self::Invalid { id, .. } => *id,
        }
    }

    /// Returns the name of a valid named argument.
    pub const fn name(&self) -> Option<&HirName> {
        match self {
            Self::Named { name, .. } => Some(name),
            Self::Positional { .. } | Self::Invalid { .. } => None,
        }
    }

    /// Returns the decoded value of a valid argument.
    pub const fn value(&self) -> Option<&HirRichTextValue> {
        match self {
            Self::Positional { value, .. } | Self::Named { value, .. } => Some(value),
            Self::Invalid { .. } => None,
        }
    }

    /// Returns the exact invalid-argument issue.
    pub const fn issue(&self) -> Option<HirRichTextArgumentIssue> {
        match self {
            Self::Invalid { issue, .. } => Some(*issue),
            Self::Positional { .. } | Self::Named { .. } => None,
        }
    }
}

/// Opaque decoded `RichText` argument value.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRichTextValue(Box<str>);

impl HirRichTextValue {
    pub(crate) const fn new(value: Box<str>) -> Self {
        Self(value)
    }

    /// Returns decoded UTF-8 without quote or escape spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Exact malformed `RichText` argument families.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRichTextArgumentIssue {
    EmptyKey,
    InvalidKey,
    InvalidEscape,
    UnterminatedQuote,
    KeyTooLong,
    ValueTooLong,
    MissingValue,
    DecoderFailure,
}

/// Typed `RichText` resolution, nesting, and payload issues.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRichTextIssue {
    UnknownTag,
    UnknownFx,
    UnknownRegisteredTag,
    InvalidNesting,
    InvalidPayload,
    ForeignNestedExpression,
    Argument(HirRichTextArgumentIssue),
}

impl From<RichTextArgumentIssue> for HirRichTextArgumentIssue {
    fn from(value: RichTextArgumentIssue) -> Self {
        match value {
            RichTextArgumentIssue::EmptyKey => Self::EmptyKey,
            RichTextArgumentIssue::InvalidKey => Self::InvalidKey,
            RichTextArgumentIssue::InvalidEscape => Self::InvalidEscape,
            RichTextArgumentIssue::UnterminatedQuote => Self::UnterminatedQuote,
            RichTextArgumentIssue::KeyTooLong => Self::KeyTooLong,
            RichTextArgumentIssue::ValueTooLong => Self::ValueTooLong,
            RichTextArgumentIssue::MissingValue => Self::MissingValue,
            RichTextArgumentIssue::DecoderFailure => Self::DecoderFailure,
        }
    }
}

impl From<SyntaxRichTextIssue> for HirRichTextIssue {
    fn from(value: SyntaxRichTextIssue) -> Self {
        match value {
            SyntaxRichTextIssue::UnknownTag => Self::UnknownTag,
            SyntaxRichTextIssue::UnknownFx => Self::UnknownFx,
            SyntaxRichTextIssue::UnknownRegisteredTag => Self::UnknownRegisteredTag,
            SyntaxRichTextIssue::InvalidNesting => Self::InvalidNesting,
            SyntaxRichTextIssue::InvalidPayload => Self::InvalidPayload,
            SyntaxRichTextIssue::ForeignNestedExpression => Self::ForeignNestedExpression,
            SyntaxRichTextIssue::Argument(issue) => Self::Argument(issue.into()),
        }
    }
}

impl From<SyntaxBuiltinRichTextTag> for HirBuiltinRichTextTag {
    fn from(value: SyntaxBuiltinRichTextTag) -> Self {
        match value {
            SyntaxBuiltinRichTextTag::Page => Self::Page,
            SyntaxBuiltinRichTextTag::LineWait => Self::LineWait,
            SyntaxBuiltinRichTextTag::HardBreak => Self::HardBreak,
            SyntaxBuiltinRichTextTag::TimedWait => Self::TimedWait,
            SyntaxBuiltinRichTextTag::Clear => Self::Clear,
            SyntaxBuiltinRichTextTag::Reset => Self::Reset,
            SyntaxBuiltinRichTextTag::Speed => Self::Speed,
            SyntaxBuiltinRichTextTag::Marker => Self::Marker,
            SyntaxBuiltinRichTextTag::DirectStyle(value) => Self::DirectStyle(value.into()),
            SyntaxBuiltinRichTextTag::Style(value) => Self::Style(value.into()),
            SyntaxBuiltinRichTextTag::Layout(value) => Self::Layout(value.into()),
            SyntaxBuiltinRichTextTag::Transform(value) => Self::Transform(value.into()),
            SyntaxBuiltinRichTextTag::Object(value) => Self::Object(value.into()),
            SyntaxBuiltinRichTextTag::Fx(value) => Self::Fx(value.into()),
            SyntaxBuiltinRichTextTag::HostEvent(value) => Self::HostEvent(value.into()),
            SyntaxBuiltinRichTextTag::Conditional(value) => Self::Conditional(value.into()),
        }
    }
}

impl From<SyntaxRichTextDirectStyle> for HirRichTextDirectStyle {
    fn from(value: SyntaxRichTextDirectStyle) -> Self {
        match value {
            SyntaxRichTextDirectStyle::Emphasis => Self::Emphasis,
            SyntaxRichTextDirectStyle::Strong => Self::Strong,
            SyntaxRichTextDirectStyle::Italic => Self::Italic,
            SyntaxRichTextDirectStyle::Oblique => Self::Oblique,
            SyntaxRichTextDirectStyle::Color => Self::Color,
            SyntaxRichTextDirectStyle::Font => Self::Font,
            SyntaxRichTextDirectStyle::Size => Self::Size,
            SyntaxRichTextDirectStyle::Ruby => Self::Ruby,
        }
    }
}

impl From<SyntaxRichTextStyleSelector> for HirRichTextStyleSelector {
    fn from(value: SyntaxRichTextStyleSelector) -> Self {
        match value {
            SyntaxRichTextStyleSelector::Italic => Self::Italic,
            SyntaxRichTextStyleSelector::Oblique => Self::Oblique,
            SyntaxRichTextStyleSelector::Opacity => Self::Opacity,
            SyntaxRichTextStyleSelector::Layer => Self::Layer,
            SyntaxRichTextStyleSelector::ZIndex => Self::ZIndex,
        }
    }
}

impl From<SyntaxRichTextLayoutSelector> for HirRichTextLayoutSelector {
    fn from(value: SyntaxRichTextLayoutSelector) -> Self {
        match value {
            SyntaxRichTextLayoutSelector::HorizontalTb => Self::HorizontalTb,
            SyntaxRichTextLayoutSelector::VerticalRl => Self::VerticalRl,
            SyntaxRichTextLayoutSelector::VerticalLr => Self::VerticalLr,
            SyntaxRichTextLayoutSelector::Direction => Self::Direction,
            SyntaxRichTextLayoutSelector::RubyOver => Self::RubyOver,
            SyntaxRichTextLayoutSelector::RubyUnder => Self::RubyUnder,
            SyntaxRichTextLayoutSelector::RubyInterCharacter => Self::RubyInterCharacter,
        }
    }
}

impl From<SyntaxRichTextTransformSelector> for HirRichTextTransformSelector {
    fn from(value: SyntaxRichTextTransformSelector) -> Self {
        match value {
            SyntaxRichTextTransformSelector::Offset => Self::Offset,
            SyntaxRichTextTransformSelector::Rotate => Self::Rotate,
            SyntaxRichTextTransformSelector::Scale => Self::Scale,
            SyntaxRichTextTransformSelector::Skew => Self::Skew,
        }
    }
}

impl From<SyntaxRichTextObjectSelector> for HirRichTextObjectSelector {
    fn from(value: SyntaxRichTextObjectSelector) -> Self {
        match value {
            SyntaxRichTextObjectSelector::Object => Self::Object,
        }
    }
}

impl From<SyntaxBuiltinRichTextFx> for HirBuiltinRichTextFx {
    fn from(value: SyntaxBuiltinRichTextFx) -> Self {
        match value {
            SyntaxBuiltinRichTextFx::Wave => Self::Wave,
            SyntaxBuiltinRichTextFx::Shake => Self::Shake,
            SyntaxBuiltinRichTextFx::Jitter => Self::Jitter,
            SyntaxBuiltinRichTextFx::Arc => Self::Arc,
            SyntaxBuiltinRichTextFx::Spin => Self::Spin,
            SyntaxBuiltinRichTextFx::Pulse => Self::Pulse,
            SyntaxBuiltinRichTextFx::Motion => Self::Motion,
            SyntaxBuiltinRichTextFx::Typewriter => Self::Typewriter,
            SyntaxBuiltinRichTextFx::Sparkle => Self::Sparkle,
            SyntaxBuiltinRichTextFx::Shader => Self::Shader,
        }
    }
}

impl From<SyntaxRichTextHostEvent> for HirRichTextHostEvent {
    fn from(value: SyntaxRichTextHostEvent) -> Self {
        match value {
            SyntaxRichTextHostEvent::Voice => Self::Voice,
            SyntaxRichTextHostEvent::Face => Self::Face,
            SyntaxRichTextHostEvent::Pose => Self::Pose,
            SyntaxRichTextHostEvent::Show => Self::Show,
            SyntaxRichTextHostEvent::Hide => Self::Hide,
            SyntaxRichTextHostEvent::Move => Self::Move,
            SyntaxRichTextHostEvent::Scale => Self::Scale,
            SyntaxRichTextHostEvent::Rotate => Self::Rotate,
            SyntaxRichTextHostEvent::Animation => Self::Animation,
            SyntaxRichTextHostEvent::StageShake => Self::StageShake,
            SyntaxRichTextHostEvent::TimedCue => Self::TimedCue,
            SyntaxRichTextHostEvent::Call => Self::Call,
            SyntaxRichTextHostEvent::Signal => Self::Signal,
        }
    }
}

impl From<SyntaxRichTextConditionalTag> for HirRichTextConditionalTag {
    fn from(value: SyntaxRichTextConditionalTag) -> Self {
        match value {
            SyntaxRichTextConditionalTag::If => Self::If,
            SyntaxRichTextConditionalTag::Else => Self::Else,
            SyntaxRichTextConditionalTag::EndIf => Self::EndIf,
        }
    }
}

pub(super) fn validate_argument_ids(
    tag: HirRichTextTagId,
    arguments: &[HirRichTextArgument],
) -> Result<(), HirDialogueInvariantError> {
    for (ordinal, argument) in arguments.iter().enumerate() {
        let expected =
            u16::try_from(ordinal).map_err(|_| HirDialogueInvariantError::ArithmeticOverflow)?;
        let id = argument.id();
        if id.tag != tag {
            return Err(HirDialogueInvariantError::InvalidArgumentReference);
        }
        if id.ordinal != expected || ordinal >= 32 {
            return Err(HirDialogueInvariantError::NonContiguousArgumentOrdinal);
        }
    }
    Ok(())
}
