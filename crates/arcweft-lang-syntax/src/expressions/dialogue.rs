//! Parser-selected dialogue-content and generic postfix-bracket projections.
//!
//! These records retain semantic values and exact typed recovery selected by
//! the shared document transaction. They never own source text, detached AST
//! nodes, or a second public syntax arena.

use std::collections::BTreeMap;

use arcweft_source::SourceRange;

use super::{PendingExpressionComponent, PendingExpressionProjection, SyntaxExpressionSlot};
use crate::grammar::assertion_projection::PendingAssertionProjection;
use crate::grammar::event::{PendingPatternProjection, PendingTypeProjection};
use crate::grammar::keyword_statement_projection::PendingKeywordStatementProjection;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::grammar::source_projection::PendingPathProjection;
use crate::name::{SyntaxName, SyntaxNameIssue};
use crate::patterns::PatternNodePath;
use crate::text::RichTextArgumentIssue;
use crate::types::TypeRefNodePath;

/// Exact outer close state for a bracket application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxBracketTerminator {
    Closed,
    RecoveredMissing(SyntaxPostfixBracketRecoveryBoundary),
}

impl SyntaxBracketTerminator {
    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::RecoveredMissing(_))
    }
}

/// Boundary selected once when the outer `]` is missing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxPostfixBracketRecoveryBoundary {
    EndOfExpression {
        anchor: usize,
    },
    LineEnding {
        range: SourceRange,
    },
    OwnerEnd {
        anchor: usize,
    },
    Token {
        token: SyntaxPostfixBoundaryToken,
        range: SourceRange,
    },
    PlanKeyword {
        range: SourceRange,
    },
}

/// Parent token that terminates a recovered postfix bracket.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxPostfixBoundaryToken {
    Comma,
    Semicolon,
    CloseParen,
    CloseBracket,
    CloseBrace,
    FatArrow,
}

/// Selected ordinary-index payload for one generic postfix bracket.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxIndexProjection {
    target: SyntaxExpressionSlot,
    index: SyntaxExpressionSlot,
    terminator: SyntaxBracketTerminator,
}

impl SyntaxIndexProjection {
    pub(crate) const fn new(
        target: SyntaxExpressionSlot,
        index: SyntaxExpressionSlot,
        terminator: SyntaxBracketTerminator,
    ) -> Self {
        Self {
            target,
            index,
            terminator,
        }
    }

    pub const fn target(&self) -> SyntaxExpressionSlot {
        self.target
    }

    pub const fn index(&self) -> SyntaxExpressionSlot {
        self.index
    }

    pub const fn terminator(&self) -> &SyntaxBracketTerminator {
        &self.terminator
    }

    pub const fn has_recovery(&self) -> bool {
        self.target.is_missing() || self.index.is_missing() || self.terminator.has_recovery()
    }
}

/// Bracket or colon spelling retained separately from semantic E33 HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxDialogueApplicationForm {
    Bracket { terminator: SyntaxBracketTerminator },
    Colon,
}

/// Complete parser-selected E33 payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxDialogueApplicationProjection {
    form: SyntaxDialogueApplicationForm,
    content: SyntaxDialogueContentProjection,
    has_plan: bool,
}

impl SyntaxDialogueApplicationProjection {
    pub(crate) const fn new(
        form: SyntaxDialogueApplicationForm,
        content: SyntaxDialogueContentProjection,
        has_plan: bool,
    ) -> Self {
        Self {
            form,
            content,
            has_plan,
        }
    }

    pub const fn form(&self) -> &SyntaxDialogueApplicationForm {
        &self.form
    }

    pub const fn content(&self) -> &SyntaxDialogueContentProjection {
        &self.content
    }

    pub const fn has_plan(&self) -> bool {
        self.has_plan
    }

    pub fn has_recovery(&self) -> bool {
        matches!(
            self.form,
            SyntaxDialogueApplicationForm::Bracket {
                terminator: SyntaxBracketTerminator::RecoveredMissing(_)
            }
        ) || self.content.has_recovery()
    }
}

/// Present or source-owned missing content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxDialogueContentProjection {
    Present(SyntaxDialogueContent),
    Missing {
        boundary: SyntaxDialogueContentRecoveryBoundary,
    },
}

impl SyntaxDialogueContentProjection {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Present(content) => content.has_recovery(),
            Self::Missing { .. } => true,
        }
    }
}

/// Exact reason no dialogue content was present.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxDialogueContentRecoveryBoundary {
    CloseBracket { range: SourceRange },
    MissingBracketClose { insertion: usize },
    Inline { insertion: usize },
    Indented { insertion: usize },
}

/// Complete ordered content and its tag inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxDialogueContent {
    nodes: Box<[SyntaxDialogueNodeProjection]>,
    tags: Box<[SyntaxRichTextTagProjection]>,
}

impl SyntaxDialogueContent {
    pub(crate) fn new(
        nodes: impl Into<Box<[SyntaxDialogueNodeProjection]>>,
        tags: impl Into<Box<[SyntaxRichTextTagProjection]>>,
    ) -> Self {
        Self {
            nodes: nodes.into(),
            tags: tags.into(),
        }
    }

    pub const fn nodes(&self) -> &[SyntaxDialogueNodeProjection] {
        &self.nodes
    }

    pub const fn tags(&self) -> &[SyntaxRichTextTagProjection] {
        &self.tags
    }

    pub fn has_recovery(&self) -> bool {
        self.nodes
            .iter()
            .any(SyntaxDialogueNodeProjection::has_recovery)
            || self
                .tags
                .iter()
                .any(SyntaxRichTextTagProjection::has_recovery)
    }
}

/// One source-ordered dialogue atom.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxDialogueNodeProjection {
    Text(Box<str>),
    Raw(Box<str>),
    Escape(char),
    Ruby { base: Box<str>, ruby: Box<str> },
    AuthoredStartTag { tag: u32 },
    InferredStartTag { tag: u32 },
    AuthoredEndTag(SyntaxRichTextEndTagProjection),
    InferredEndTag(SyntaxRichTextEndTagProjection),
    Interpolation(SyntaxExpressionSlot),
    Control(SyntaxDialogueControl),
    Mark(Result<SyntaxName, SyntaxNameIssue>),
    LineBreak(SyntaxLineBreakKind),
    Error(SyntaxDialogueContentIssue),
}

impl SyntaxDialogueNodeProjection {
    pub const fn has_recovery(&self) -> bool {
        matches!(
            self,
            Self::Interpolation(SyntaxExpressionSlot::Missing)
                | Self::Mark(Err(_))
                | Self::Error(_)
        ) || matches!(
            self,
            Self::AuthoredEndTag(end) | Self::InferredEndTag(end) if end.has_recovery()
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxRichTextEndTagProjection {
    identity: Option<SyntaxRichTextTagIdentity>,
    inferred: bool,
    issue: Option<SyntaxRichTextIssue>,
}

impl SyntaxRichTextEndTagProjection {
    pub(crate) const fn new(
        identity: Option<SyntaxRichTextTagIdentity>,
        inferred: bool,
        issue: Option<SyntaxRichTextIssue>,
    ) -> Self {
        Self {
            identity,
            inferred,
            issue,
        }
    }

    pub const fn identity(&self) -> Option<&SyntaxRichTextTagIdentity> {
        self.identity.as_ref()
    }

    pub const fn is_inferred(&self) -> bool {
        self.inferred
    }

    pub const fn issue(&self) -> Option<&SyntaxRichTextIssue> {
        self.issue.as_ref()
    }

    pub const fn has_recovery(&self) -> bool {
        self.issue.is_some()
    }
}

/// One source-ordered typed `RichText` tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxRichTextTagProjection {
    identity: SyntaxRichTextTagIdentity,
    arguments: Box<[SyntaxRichTextArgumentProjection]>,
    payload: SyntaxRichTextTagPayloadProjection,
    paired_end_node: Option<u32>,
}

impl SyntaxRichTextTagProjection {
    pub(crate) fn new(
        identity: SyntaxRichTextTagIdentity,
        arguments: impl Into<Box<[SyntaxRichTextArgumentProjection]>>,
        payload: SyntaxRichTextTagPayloadProjection,
        paired_end_node: Option<u32>,
    ) -> Self {
        Self {
            identity,
            arguments: arguments.into(),
            payload,
            paired_end_node,
        }
    }

    pub const fn identity(&self) -> &SyntaxRichTextTagIdentity {
        &self.identity
    }

    pub const fn arguments(&self) -> &[SyntaxRichTextArgumentProjection] {
        &self.arguments
    }

    pub const fn payload(&self) -> &SyntaxRichTextTagPayloadProjection {
        &self.payload
    }

    pub const fn paired_end_node(&self) -> Option<u32> {
        self.paired_end_node
    }

    pub(crate) fn pair_with_end_node(&mut self, node: u32) -> bool {
        if self.paired_end_node.replace(node).is_some() {
            return false;
        }
        true
    }

    pub fn has_recovery(&self) -> bool {
        matches!(self.identity, SyntaxRichTextTagIdentity::Invalid(_))
            || self
                .arguments
                .iter()
                .any(SyntaxRichTextArgumentProjection::has_recovery)
            || self.payload.has_recovery()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxRichTextTagIdentity {
    Builtin(SyntaxBuiltinRichTextTag),
    Marker(Result<SyntaxName, SyntaxNameIssue>),
    ProjectSymbol(SyntaxProjectSymbolPath),
    Invalid(SyntaxRichTextIssue),
}

impl SyntaxRichTextTagIdentity {
    /// Classifies one parser-delimited tag head without retaining its spelling.
    pub(crate) fn from_source_name(source: &str) -> Self {
        if let Some(builtin) = SyntaxBuiltinRichTextTag::from_source_name(source) {
            return Self::Builtin(builtin);
        }
        if let Some(marker) = source.strip_prefix('.') {
            return Self::Marker(SyntaxName::try_new(marker));
        }

        let (absolute, path) = source
            .strip_prefix("::")
            .map_or((false, source), |path| (true, path));
        Self::ProjectSymbol(SyntaxProjectSymbolPath::new(
            absolute,
            path.split("::")
                .map(SyntaxName::try_new)
                .collect::<Vec<_>>(),
        ))
    }

    pub(crate) fn opens_span(&self) -> bool {
        match self {
            Self::Builtin(builtin) => builtin.opens_span(),
            Self::Marker(_) | Self::ProjectSymbol(_) => true,
            Self::Invalid(_) => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxProjectSymbolPath {
    absolute: bool,
    segments: Box<[Result<SyntaxName, SyntaxNameIssue>]>,
}

impl SyntaxProjectSymbolPath {
    pub(crate) fn new(
        absolute: bool,
        segments: impl Into<Box<[Result<SyntaxName, SyntaxNameIssue>]>>,
    ) -> Self {
        Self {
            absolute,
            segments: segments.into(),
        }
    }

    pub const fn is_absolute(&self) -> bool {
        self.absolute
    }

    pub const fn segments(&self) -> &[Result<SyntaxName, SyntaxNameIssue>] {
        &self.segments
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxRichTextTagPayloadProjection {
    Arguments,
    FxCall(SyntaxExpressionSlot),
    DialogueCall(SyntaxExpressionSlot),
    Condition(SyntaxExpressionSlot),
    None,
}

impl SyntaxRichTextTagPayloadProjection {
    pub const fn has_recovery(&self) -> bool {
        matches!(
            self,
            Self::FxCall(SyntaxExpressionSlot::Missing)
                | Self::DialogueCall(SyntaxExpressionSlot::Missing)
                | Self::Condition(SyntaxExpressionSlot::Missing)
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxRichTextArgumentProjection {
    Positional {
        value: SyntaxRichTextValue,
    },
    Named {
        name: Result<SyntaxName, SyntaxNameIssue>,
        value: SyntaxRichTextValue,
    },
    Invalid {
        issue: RichTextArgumentIssue,
        authored_parts: SyntaxRichTextArgumentParts,
    },
}

impl SyntaxRichTextArgumentProjection {
    pub const fn has_recovery(&self) -> bool {
        matches!(
            self,
            Self::Named { name: Err(_), .. } | Self::Invalid { .. }
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxRichTextValue(Box<str>);

impl SyntaxRichTextValue {
    pub(crate) fn new(decoded: impl Into<Box<str>>) -> Self {
        Self(decoded.into())
    }

    pub fn decoded(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxRichTextArgumentParts {
    name: bool,
    equals: bool,
    value: bool,
}

impl SyntaxRichTextArgumentParts {
    pub(crate) const fn new(name: bool, equals: bool, value: bool) -> Self {
        Self {
            name,
            equals,
            value,
        }
    }

    pub const fn has_name(self) -> bool {
        self.name
    }

    pub const fn has_equals(self) -> bool {
        self.equals
    }

    pub const fn has_value(self) -> bool {
        self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxRichTextIssue {
    UnknownTag,
    UnknownFx,
    UnknownRegisteredTag,
    InvalidNesting,
    InvalidPayload,
    ForeignNestedExpression,
    Argument(RichTextArgumentIssue),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxDialogueContentIssue {
    UnclassifiedToken,
    InvalidEscape,
    InvalidRuby,
    UnmatchedEndTag,
    UnclosedTag,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxLineBreakKind {
    Line,
    Paragraph,
    Page,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxDialogueControl {
    Wait,
    Reset,
    Clear,
    Erase,
    ClearMessage,
    Speed,
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
    At,
    Call,
    Signal,
    ConditionalIf,
    ConditionalElse,
    ConditionalEnd,
}

impl SyntaxDialogueControl {
    /// Resolves the current grammar-owned spelling of a point or host control.
    ///
    /// Aliases are normalized here, before HIR. They are grammar spellings,
    /// not compatibility identities and never survive as semantic strings.
    pub(crate) const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"w" | b"l" | b"wait" => Some(Self::Wait),
            b"reset" => Some(Self::Reset),
            b"clear" => Some(Self::Clear),
            b"er" => Some(Self::Erase),
            b"cm" => Some(Self::ClearMessage),
            b"speed" => Some(Self::Speed),
            b"voice" => Some(Self::Voice),
            b"face" => Some(Self::Face),
            b"pose" => Some(Self::Pose),
            b"show" => Some(Self::Show),
            b"hide" => Some(Self::Hide),
            b"move" => Some(Self::Move),
            b"scale" => Some(Self::Scale),
            b"rotate" => Some(Self::Rotate),
            b"anim" => Some(Self::Animation),
            b"shake" => Some(Self::StageShake),
            b"at" => Some(Self::At),
            b"call" | b"!" => Some(Self::Call),
            b"signal" => Some(Self::Signal),
            b"if" => Some(Self::ConditionalIf),
            b"else" => Some(Self::ConditionalElse),
            b"endif" => Some(Self::ConditionalEnd),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxBuiltinRichTextTag {
    Page,
    LineWait,
    HardBreak,
    TimedWait,
    Clear,
    Reset,
    Speed,
    DirectStyle(SyntaxRichTextDirectStyle),
    Style(SyntaxRichTextStyleSelector),
    Layout(SyntaxRichTextLayoutSelector),
    Transform(SyntaxRichTextTransformSelector),
    Object(SyntaxRichTextObjectSelector),
    Fx(SyntaxBuiltinRichTextFx),
    HostEvent(SyntaxRichTextHostEvent),
    Conditional(SyntaxRichTextConditionalTag),
}

impl SyntaxBuiltinRichTextTag {
    /// Resolves a source tag or dot-selector to its canonical typed identity.
    pub(crate) const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"p" | b"page" => Some(Self::Page),
            b"l" | b"wait" => Some(Self::LineWait),
            b"r" | b"nl" | b"br" => Some(Self::HardBreak),
            b"w" => Some(Self::TimedWait),
            b"clear" | b"er" | b"cm" => Some(Self::Clear),
            b"reset" => Some(Self::Reset),
            b"speed" => Some(Self::Speed),
            b"em" => Some(Self::DirectStyle(SyntaxRichTextDirectStyle::Emphasis)),
            b"strong" => Some(Self::DirectStyle(SyntaxRichTextDirectStyle::Strong)),
            b"i" | b"italic" => Some(Self::DirectStyle(SyntaxRichTextDirectStyle::Italic)),
            b"oblique" | b"slant" => Some(Self::DirectStyle(SyntaxRichTextDirectStyle::Oblique)),
            b"color" => Some(Self::DirectStyle(SyntaxRichTextDirectStyle::Color)),
            b"font" => Some(Self::DirectStyle(SyntaxRichTextDirectStyle::Font)),
            b"size" => Some(Self::DirectStyle(SyntaxRichTextDirectStyle::Size)),
            b"ruby" | b"rb" => Some(Self::DirectStyle(SyntaxRichTextDirectStyle::Ruby)),
            b".italic" | b".i" => Some(Self::Style(SyntaxRichTextStyleSelector::Italic)),
            b".oblique" | b".slant" => Some(Self::Style(SyntaxRichTextStyleSelector::Oblique)),
            b".opacity" | b".alpha" => Some(Self::Style(SyntaxRichTextStyleSelector::Opacity)),
            b".layer" | b".object_layer" => Some(Self::Style(SyntaxRichTextStyleSelector::Layer)),
            b".z_index" | b".z" => Some(Self::Style(SyntaxRichTextStyleSelector::ZIndex)),
            b".horizontal_tb" => Some(Self::Layout(SyntaxRichTextLayoutSelector::HorizontalTb)),
            b".vertical_rl" | b".vertical" => {
                Some(Self::Layout(SyntaxRichTextLayoutSelector::VerticalRl))
            }
            b".vertical_lr" => Some(Self::Layout(SyntaxRichTextLayoutSelector::VerticalLr)),
            b".dir" => Some(Self::Layout(SyntaxRichTextLayoutSelector::Direction)),
            b".ruby_over" => Some(Self::Layout(SyntaxRichTextLayoutSelector::RubyOver)),
            b".ruby_under" => Some(Self::Layout(SyntaxRichTextLayoutSelector::RubyUnder)),
            b".ruby_inter_character" => Some(Self::Layout(
                SyntaxRichTextLayoutSelector::RubyInterCharacter,
            )),
            b".offset" | b".pos" => Some(Self::Transform(SyntaxRichTextTransformSelector::Offset)),
            b".rotate" => Some(Self::Transform(SyntaxRichTextTransformSelector::Rotate)),
            b".scale" => Some(Self::Transform(SyntaxRichTextTransformSelector::Scale)),
            b".skew" => Some(Self::Transform(SyntaxRichTextTransformSelector::Skew)),
            b".object" | b"object" => Some(Self::Object(SyntaxRichTextObjectSelector::Object)),
            b".wave" => Some(Self::Fx(SyntaxBuiltinRichTextFx::Wave)),
            b".shake" => Some(Self::Fx(SyntaxBuiltinRichTextFx::Shake)),
            b".jitter" => Some(Self::Fx(SyntaxBuiltinRichTextFx::Jitter)),
            b".arc" => Some(Self::Fx(SyntaxBuiltinRichTextFx::Arc)),
            b".spin" => Some(Self::Fx(SyntaxBuiltinRichTextFx::Spin)),
            b".pulse" => Some(Self::Fx(SyntaxBuiltinRichTextFx::Pulse)),
            b".motion" => Some(Self::Fx(SyntaxBuiltinRichTextFx::Motion)),
            b".typewriter" => Some(Self::Fx(SyntaxBuiltinRichTextFx::Typewriter)),
            b".sparkle" => Some(Self::Fx(SyntaxBuiltinRichTextFx::Sparkle)),
            b".shader" | b"shader" => Some(Self::Fx(SyntaxBuiltinRichTextFx::Shader)),
            b"voice" => Some(Self::HostEvent(SyntaxRichTextHostEvent::Voice)),
            b"face" => Some(Self::HostEvent(SyntaxRichTextHostEvent::Face)),
            b"pose" => Some(Self::HostEvent(SyntaxRichTextHostEvent::Pose)),
            b"show" => Some(Self::HostEvent(SyntaxRichTextHostEvent::Show)),
            b"hide" => Some(Self::HostEvent(SyntaxRichTextHostEvent::Hide)),
            b"move" => Some(Self::HostEvent(SyntaxRichTextHostEvent::Move)),
            b"scale" => Some(Self::HostEvent(SyntaxRichTextHostEvent::Scale)),
            b"rotate" => Some(Self::HostEvent(SyntaxRichTextHostEvent::Rotate)),
            b"anim" => Some(Self::HostEvent(SyntaxRichTextHostEvent::Animation)),
            b"shake" => Some(Self::HostEvent(SyntaxRichTextHostEvent::StageShake)),
            b"at" => Some(Self::HostEvent(SyntaxRichTextHostEvent::TimedCue)),
            b"call" | b"!" => Some(Self::HostEvent(SyntaxRichTextHostEvent::Call)),
            b"signal" => Some(Self::HostEvent(SyntaxRichTextHostEvent::Signal)),
            b"if" => Some(Self::Conditional(SyntaxRichTextConditionalTag::If)),
            b"else" => Some(Self::Conditional(SyntaxRichTextConditionalTag::Else)),
            b"endif" => Some(Self::Conditional(SyntaxRichTextConditionalTag::EndIf)),
            _ => None,
        }
    }

    pub(crate) const fn opens_span(self) -> bool {
        matches!(
            self,
            Self::DirectStyle(_)
                | Self::Style(_)
                | Self::Layout(_)
                | Self::Transform(_)
                | Self::Object(_)
                | Self::Fx(_)
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxRichTextDirectStyle {
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
pub enum SyntaxRichTextStyleSelector {
    Italic,
    Oblique,
    Opacity,
    Layer,
    ZIndex,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxRichTextLayoutSelector {
    HorizontalTb,
    VerticalRl,
    VerticalLr,
    Direction,
    RubyOver,
    RubyUnder,
    RubyInterCharacter,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxRichTextTransformSelector {
    Offset,
    Rotate,
    Scale,
    Skew,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxRichTextObjectSelector {
    Object,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxBuiltinRichTextFx {
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
pub enum SyntaxRichTextHostEvent {
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
pub enum SyntaxRichTextConditionalTag {
    If,
    Else,
    EndIf,
}

/// Quality of one viable candidate parse.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxCandidateQuality {
    Clean,
    Recovered,
}

/// Candidate-local index. It is never a source or HIR identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CandidateNodeIndex(u32);

impl CandidateNodeIndex {
    pub(crate) fn try_new(index: usize) -> Option<Self> {
        u32::try_from(index).ok().map(Self)
    }

    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CandidateEdgeRange {
    start: u32,
    len: u32,
}

impl CandidateEdgeRange {
    fn try_new(start: usize, len: usize) -> Option<Self> {
        Some(Self {
            start: u32::try_from(start).ok()?,
            len: u32::try_from(len).ok()?,
        })
    }

    fn as_range(self) -> core::ops::Range<usize> {
        let start = self.start as usize;
        start..start + self.len as usize
    }
}

/// One tokenless candidate semantic node in local preorder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingCandidateNode {
    kind: SyntaxKind,
    role: SyntaxRole,
    parent: Option<CandidateNodeIndex>,
    children: CandidateEdgeRange,
    source: SourceRange,
    semantic: PendingCandidateSemantic,
}

impl PendingCandidateNode {
    pub(crate) const fn new(
        kind: SyntaxKind,
        role: SyntaxRole,
        parent: Option<CandidateNodeIndex>,
        source: SourceRange,
        semantic: PendingCandidateSemantic,
    ) -> Self {
        Self {
            kind,
            role,
            parent,
            children: CandidateEdgeRange { start: 0, len: 0 },
            source,
            semantic,
        }
    }

    pub(crate) const fn kind(&self) -> SyntaxKind {
        self.kind
    }

    pub(crate) const fn role(&self) -> SyntaxRole {
        self.role
    }

    pub(crate) const fn parent(&self) -> Option<CandidateNodeIndex> {
        self.parent
    }

    pub(crate) const fn source(&self) -> SourceRange {
        self.source
    }

    pub(crate) const fn semantic(&self) -> &PendingCandidateSemantic {
        &self.semantic
    }
}

/// Parser-selected semantic payload retained after candidate events are consumed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingCandidateSemantic {
    Expression(PendingExpressionProjection),
    Assertion(PendingAssertionProjection),
    KeywordStatement(PendingKeywordStatementProjection),
    Type(PendingTypeProjection),
    Pattern(PendingPatternProjection),
    Path(PendingPathProjection),
    KindOnly,
}

/// Tokenless local adjacency graph for one discarded-but-retained candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingCandidateGraph {
    roots: Box<[CandidateNodeIndex]>,
    nodes: Box<[PendingCandidateNode]>,
    child_edges: Box<[CandidateNodeIndex]>,
    type_index: BTreeMap<(u64, TypeRefNodePath), CandidateNodeIndex>,
    pattern_index: BTreeMap<(u64, PatternNodePath), CandidateNodeIndex>,
}

impl PendingCandidateGraph {
    pub(crate) fn try_new(
        mut nodes: Vec<PendingCandidateNode>,
    ) -> Result<Self, PendingCandidateGraphError> {
        let mut roots = Vec::new();
        let mut children = vec![Vec::new(); nodes.len()];
        let mut type_index = BTreeMap::new();
        let mut pattern_index = BTreeMap::new();

        for (position, node) in nodes.iter().enumerate() {
            let index = CandidateNodeIndex::try_new(position)
                .ok_or(PendingCandidateGraphError::NodeCountExceeded)?;
            if let Some(parent) = node.parent {
                if parent.as_usize() >= position {
                    return Err(PendingCandidateGraphError::InvalidParent {
                        node: index,
                        parent,
                    });
                }
                children[parent.as_usize()].push(index);
            } else {
                roots.push(index);
            }
            match &node.semantic {
                PendingCandidateSemantic::Type(projection) => {
                    if projection.authored().value_at(projection.path()).is_none() {
                        return Err(PendingCandidateGraphError::InvalidTypeProjection);
                    }
                    if type_index
                        .insert((projection.tree(), projection.path().clone()), index)
                        .is_some()
                    {
                        return Err(PendingCandidateGraphError::DuplicateTypeProjection);
                    }
                }
                PendingCandidateSemantic::Pattern(projection) => {
                    if projection.authored().value_at(projection.path()).is_none() {
                        return Err(PendingCandidateGraphError::InvalidPatternProjection);
                    }
                    if pattern_index
                        .insert((projection.tree(), projection.path().clone()), index)
                        .is_some()
                    {
                        return Err(PendingCandidateGraphError::DuplicatePatternProjection);
                    }
                }
                PendingCandidateSemantic::KeywordStatement(projection) => {
                    if !projection.accepts_kind(node.kind()) {
                        return Err(PendingCandidateGraphError::InvalidKeywordStatementProjection);
                    }
                }
                PendingCandidateSemantic::Assertion(_)
                    if node.kind() != SyntaxKind::AssertionStatement =>
                {
                    return Err(PendingCandidateGraphError::InvalidAssertionProjection);
                }
                PendingCandidateSemantic::Expression(_)
                | PendingCandidateSemantic::Assertion(_)
                | PendingCandidateSemantic::Path(_)
                | PendingCandidateSemantic::KindOnly => {}
            }
        }

        let mut child_edges = Vec::new();
        for (node, child_nodes) in nodes.iter_mut().zip(children) {
            node.children = CandidateEdgeRange::try_new(child_edges.len(), child_nodes.len())
                .ok_or(PendingCandidateGraphError::NodeCountExceeded)?;
            child_edges.extend(child_nodes);
        }

        Ok(Self {
            roots: roots.into_boxed_slice(),
            nodes: nodes.into_boxed_slice(),
            child_edges: child_edges.into_boxed_slice(),
            type_index,
            pattern_index,
        })
    }

    pub(crate) const fn roots(&self) -> &[CandidateNodeIndex] {
        &self.roots
    }

    pub(crate) const fn nodes(&self) -> &[PendingCandidateNode] {
        &self.nodes
    }

    pub(crate) fn node(&self, index: CandidateNodeIndex) -> Option<&PendingCandidateNode> {
        self.nodes.get(index.as_usize())
    }

    pub(crate) fn children(&self, index: CandidateNodeIndex) -> Option<&[CandidateNodeIndex]> {
        let node = self.node(index)?;
        self.child_edges.get(node.children.as_range())
    }

    pub(crate) fn type_node(
        &self,
        tree: u64,
        path: &TypeRefNodePath,
    ) -> Option<CandidateNodeIndex> {
        self.type_index.get(&(tree, path.clone())).copied()
    }

    pub(crate) fn pattern_node(
        &self,
        tree: u64,
        path: &PatternNodePath,
    ) -> Option<CandidateNodeIndex> {
        self.pattern_index.get(&(tree, path.clone())).copied()
    }

    fn primary_expression(&self) -> Option<CandidateNodeIndex> {
        self.nodes.iter().enumerate().find_map(|(position, node)| {
            if !matches!(node.semantic(), PendingCandidateSemantic::Expression(_)) {
                return None;
            }
            let mut parent = node.parent();
            while let Some(index) = parent {
                let ancestor = self.node(index)?;
                if matches!(ancestor.semantic(), PendingCandidateSemantic::Expression(_)) {
                    return None;
                }
                parent = ancestor.parent();
            }
            CandidateNodeIndex::try_new(position)
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingCandidateGraphError {
    NodeCountExceeded,
    InvalidParent {
        node: CandidateNodeIndex,
        parent: CandidateNodeIndex,
    },
    DuplicateTypeProjection,
    DuplicatePatternProjection,
    InvalidTypeProjection,
    InvalidPatternProjection,
    InvalidKeywordStatementProjection,
    InvalidAssertionProjection,
}

/// The ordinary-index interpretation retained only when both candidates win.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxPostfixIndexCandidate {
    quality: SyntaxCandidateQuality,
    index: CandidateNodeIndex,
    graph: PendingCandidateGraph,
}

impl SyntaxPostfixIndexCandidate {
    pub(crate) fn new(
        quality: SyntaxCandidateQuality,
        _candidate_root: CandidateNodeIndex,
        graph: PendingCandidateGraph,
    ) -> Self {
        let index = graph
            .primary_expression()
            .expect("viable ordinary-index candidates retain one semantic expression root");
        Self {
            quality,
            index,
            graph,
        }
    }

    pub const fn quality(&self) -> SyntaxCandidateQuality {
        self.quality
    }

    pub(crate) const fn index(&self) -> CandidateNodeIndex {
        self.index
    }

    pub(crate) const fn graph(&self) -> &PendingCandidateGraph {
        &self.graph
    }
}

/// The dialogue-content interpretation retained only when both candidates win.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxPostfixDialogueCandidate {
    quality: SyntaxCandidateQuality,
    content: SyntaxDialogueContentProjection,
    components: Box<[PendingExpressionComponent]>,
    graph: PendingCandidateGraph,
}

impl SyntaxPostfixDialogueCandidate {
    pub(crate) fn new(
        quality: SyntaxCandidateQuality,
        content: SyntaxDialogueContentProjection,
        components: impl Into<Box<[PendingExpressionComponent]>>,
        graph: PendingCandidateGraph,
    ) -> Self {
        Self {
            quality,
            content,
            components: components.into(),
            graph,
        }
    }

    pub const fn quality(&self) -> SyntaxCandidateQuality {
        self.quality
    }

    pub const fn content(&self) -> &SyntaxDialogueContentProjection {
        &self.content
    }

    pub(crate) const fn components(&self) -> &[PendingExpressionComponent] {
        &self.components
    }

    pub(crate) const fn graph(&self) -> &PendingCandidateGraph {
        &self.graph
    }
}

/// Exact E34 ambiguity or no-match result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxPostfixBracketProjection {
    Ambiguous {
        index: Box<SyntaxPostfixIndexCandidate>,
        dialogue: Box<SyntaxPostfixDialogueCandidate>,
    },
    Invalid {
        index: SyntaxPostfixCandidateFailure,
        dialogue: SyntaxPostfixCandidateFailure,
    },
}

impl SyntaxPostfixBracketProjection {
    pub const fn has_recovery(&self) -> bool {
        match self {
            Self::Ambiguous { index, dialogue } => {
                matches!(index.quality(), SyntaxCandidateQuality::Recovered)
                    || matches!(dialogue.quality(), SyntaxCandidateQuality::Recovered)
            }
            Self::Invalid { .. } => true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxPostfixCandidateFailure {
    kind: SyntaxPostfixCandidateFailureKind,
    site: SyntaxPostfixCandidateFailureSite,
}

impl SyntaxPostfixCandidateFailure {
    pub(crate) const fn new(
        kind: SyntaxPostfixCandidateFailureKind,
        site: SyntaxPostfixCandidateFailureSite,
    ) -> Self {
        Self { kind, site }
    }

    pub const fn kind(&self) -> SyntaxPostfixCandidateFailureKind {
        self.kind
    }

    pub const fn site(&self) -> SyntaxPostfixCandidateFailureSite {
        self.site
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxPostfixCandidateFailureKind {
    EmptyPayload,
    UnexpectedToken,
    MissingOperand,
    TrailingToken,
    InvalidDialogueAtom,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxPostfixCandidateFailureSite {
    Span(SourceRange),
    Insertion(usize),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxDialogueConfigurationArgumentPart {
    Whole,
    Name,
    Value,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxDialogueNodeSourcePart {
    Whole,
    Text,
    Raw,
    Escape,
    RubyBase,
    RubyText,
    Interpolation,
    Control,
    Mark,
    LineBreak,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxRichTextTagSourcePart {
    Whole,
    OpenDelimiter,
    Name,
    Payload,
    CloseDelimiter,
    InferenceInsertion,
    EndTag,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxRichTextArgumentSourcePart {
    Whole,
    Name,
    Equals,
    Value,
}
