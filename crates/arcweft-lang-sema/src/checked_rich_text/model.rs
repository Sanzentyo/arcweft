use arcweft_dialogue::rich_text::{
    DialogueControlProperty, DialogueHostEventKind, DialogueHostProperty, DialogueRichTextControl,
};
use arcweft_id::PublicId;
use arcweft_lang_hir::dialogue_application::{
    HirDialogueContentId, HirDialogueMarkName, HirLineBreakKind, HirRichTextArgumentId,
    HirRichTextTagId,
};
use arcweft_lang_hir::identity::ExprId;
use arcweft_lang_hir::source_index::HirSourceSite;
use arcweft_presentation::rich_text::{
    BuiltinRichTextFx, BuiltinRichTextFxPhase, BuiltinRichTextFxProperty, RichTextDirectStyle,
    RichTextDirectStyleProperty, RichTextLayoutProperty, RichTextLayoutSelector,
    RichTextObjectProperty, RichTextStyleProperty, RichTextStyleSelector,
    RichTextTransformProperty, RichTextTransformSelector,
};

use super::{
    CheckedAngle, CheckedColor, CheckedDuration, CheckedEnumValue, CheckedLength,
    CheckedRichTextValue, Milli, RatioMilli, RichTextAttributeDiagnostic,
};
use crate::semantic_coordinate::StableCheckedDialogueMarkCoordinate;

/// Stable identity of one materialized owner-schema default.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RichTextDefaultId(u16);

impl RichTextDefaultId {
    pub(crate) const fn from_schema_ordinal(value: u16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

/// Closed semantic property identity. There is no string-key fallback.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CheckedRichTextProperty {
    Control(DialogueControlProperty),
    Host(DialogueHostProperty),
    DirectStyle(RichTextDirectStyleProperty),
    Style(RichTextStyleProperty),
    Layout(RichTextLayoutProperty),
    Transform(RichTextTransformProperty),
    Object(RichTextObjectProperty),
    BuiltinFx(BuiltinRichTextFxProperty),
}

/// Provenance of one checked field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedFieldOrigin {
    Authored {
        argument: HirRichTextArgumentId,
        key: Option<HirSourceSite>,
        value: HirSourceSite,
    },
    Defaulted {
        default_id: RichTextDefaultId,
    },
}

/// One property-identified typed value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedField {
    property: CheckedRichTextProperty,
    value: CheckedRichTextValue,
    origin: CheckedFieldOrigin,
}

impl CheckedField {
    pub(crate) const fn new(
        property: CheckedRichTextProperty,
        value: CheckedRichTextValue,
        origin: CheckedFieldOrigin,
    ) -> Self {
        Self {
            property,
            value,
            origin,
        }
    }

    pub const fn property(&self) -> CheckedRichTextProperty {
        self.property
    }

    pub const fn value(&self) -> &CheckedRichTextValue {
        &self.value
    }

    pub const fn origin(&self) -> &CheckedFieldOrigin {
        &self.origin
    }
}

/// Deterministically schema-ordered values for one owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedOwnerFields(Box<[CheckedField]>);

impl CheckedOwnerFields {
    pub(crate) fn new(fields: Vec<CheckedField>) -> Self {
        Self(fields.into_boxed_slice())
    }

    pub const fn fields(&self) -> &[CheckedField] {
        &self.0
    }

    pub(crate) fn value(&self, property: CheckedRichTextProperty) -> Option<&CheckedRichTextValue> {
        self.0
            .iter()
            .find(|field| field.property == property)
            .map(|field| &field.value)
    }
}

/// Resolved semantic owner of one checked tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedRichTextOwner {
    Control(DialogueRichTextControl),
    DirectStyle(RichTextDirectStyle),
    Style(RichTextStyleSelector),
    Layout(RichTextLayoutSelector),
    Transform(RichTextTransformSelector),
    Object,
    Marker,
    BuiltinFx {
        effect: BuiltinRichTextFx,
        phase: BuiltinRichTextFxPhase,
    },
    Host(DialogueHostEventKind),
}

/// Exact typed point-control output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedDialogueControl {
    Page,
    LineWait,
    HardBreak,
    TimedWait { duration: CheckedDuration },
    Clear,
    Reset,
    RevealRate { milli_cps: Milli },
}

/// Checked voice-source identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedVoiceSource {
    Auto,
    Identity(PublicId),
}

/// Exact checked host event after owner-schema validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedDialogueHostEvent {
    Voice { source: CheckedVoiceSource },
    Face { expression: PublicId },
    Pose { pose: PublicId },
    Show { entity: PublicId },
    Hide { entity: PublicId },
    Move { x: CheckedLength, y: CheckedLength },
    Scale { x: Milli, y: Milli },
    Rotate { angle: CheckedAngle },
    Animation { animation: PublicId },
    Shake { amplitude: CheckedLength },
    TimedCue { at: CheckedDuration, call: ExprId },
    Call { call: ExprId },
    Signal { signal: PublicId },
    ConditionalStart { condition: ExprId },
    ConditionalElse,
    ConditionalEnd,
}

/// Exact checked output for a direct style span.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedDirectStyleSpan {
    Emphasis,
    Strong,
    Italic,
    Oblique { angle: CheckedAngle },
    Color { value: CheckedColor },
    Font { family: String },
    Size { value: CheckedLength },
    Ruby { annotation: String },
}

/// Exact checked output for a presentation style selector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedStyleSpan {
    Italic,
    Oblique { angle: CheckedAngle },
    Opacity { value: RatioMilli },
    Layer { value: PublicId },
    ZIndex { value: i16 },
}

/// Exact checked layout directive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedLayoutSpan {
    selector: RichTextLayoutSelector,
    direction: CheckedEnumValue,
    vertical_latin: CheckedEnumValue,
    jlreq_strictness: CheckedEnumValue,
    column_gap: CheckedLength,
    ruby_font_size: Option<CheckedLength>,
    ruby_gap: Option<CheckedLength>,
    ruby_overhang: Option<CheckedLength>,
    ruby_collision_gap: Option<CheckedLength>,
}

impl CheckedLayoutSpan {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        selector: RichTextLayoutSelector,
        direction: CheckedEnumValue,
        vertical_latin: CheckedEnumValue,
        jlreq_strictness: CheckedEnumValue,
        column_gap: CheckedLength,
        ruby_font_size: Option<CheckedLength>,
        ruby_gap: Option<CheckedLength>,
        ruby_overhang: Option<CheckedLength>,
        ruby_collision_gap: Option<CheckedLength>,
    ) -> Self {
        Self {
            selector,
            direction,
            vertical_latin,
            jlreq_strictness,
            column_gap,
            ruby_font_size,
            ruby_gap,
            ruby_overhang,
            ruby_collision_gap,
        }
    }

    pub const fn selector(&self) -> RichTextLayoutSelector {
        self.selector
    }
}

/// Exact checked transform directive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedTransformSpan {
    Offset {
        x: CheckedLength,
        y: CheckedLength,
        target: CheckedEnumValue,
        origin: CheckedEnumValue,
    },
    Rotate {
        angle: CheckedAngle,
        target: CheckedEnumValue,
        origin: CheckedEnumValue,
    },
    Scale {
        x: Milli,
        y: Milli,
        target: CheckedEnumValue,
        origin: CheckedEnumValue,
    },
    Skew {
        x: CheckedAngle,
        y: CheckedAngle,
        target: CheckedEnumValue,
        origin: CheckedEnumValue,
    },
}

/// Exact checked metadata-only object span.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedObjectSpan {
    selector: PublicId,
    role: Option<PublicId>,
    layer: Option<PublicId>,
    depth: Option<CheckedLength>,
    hit_test: bool,
}

impl CheckedObjectSpan {
    pub(crate) const fn new(
        selector: PublicId,
        role: Option<PublicId>,
        layer: Option<PublicId>,
        depth: Option<CheckedLength>,
        hit_test: bool,
    ) -> Self {
        Self {
            selector,
            role,
            layer,
            depth,
            hit_test,
        }
    }

    pub const fn selector(&self) -> &PublicId {
        &self.selector
    }
}

/// Family-specific typed action consumed by compiler/runtime adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedRichTextAction {
    Control {
        action: CheckedDialogueControl,
        fields: CheckedOwnerFields,
    },
    DirectStyle {
        owner: RichTextDirectStyle,
        action: CheckedDirectStyleSpan,
        fields: CheckedOwnerFields,
    },
    Style {
        owner: RichTextStyleSelector,
        action: CheckedStyleSpan,
        fields: CheckedOwnerFields,
    },
    Layout {
        owner: RichTextLayoutSelector,
        action: CheckedLayoutSpan,
        fields: CheckedOwnerFields,
    },
    Transform {
        owner: RichTextTransformSelector,
        action: CheckedTransformSpan,
        fields: CheckedOwnerFields,
    },
    Object {
        action: CheckedObjectSpan,
        fields: CheckedOwnerFields,
    },
    BuiltinFx {
        effect: BuiltinRichTextFx,
        phase: BuiltinRichTextFxPhase,
        fields: CheckedOwnerFields,
    },
    Host {
        owner: DialogueHostEventKind,
        action: CheckedDialogueHostEvent,
        fields: CheckedOwnerFields,
    },
    Marker(CheckedDialogueMark),
}

impl CheckedRichTextAction {
    pub const fn fields(&self) -> Option<&CheckedOwnerFields> {
        match self {
            Self::Control { fields, .. }
            | Self::DirectStyle { fields, .. }
            | Self::Style { fields, .. }
            | Self::Layout { fields, .. }
            | Self::Transform { fields, .. }
            | Self::Object { fields, .. }
            | Self::BuiltinFx { fields, .. }
            | Self::Host { fields, .. } => Some(fields),
            Self::Marker(_) => None,
        }
    }
}

/// Final accepted identity of one dialogue-content marker.
///
/// Equality, ordering, and hashing deliberately exclude the display-only
/// diagnostic name. The accepted-rooted coordinate is the sole semantic
/// identity consumed by compiler projection and transcripts.
#[derive(Clone, Debug)]
pub struct CheckedDialogueMark {
    coordinate: StableCheckedDialogueMarkCoordinate,
    diagnostic_name: HirDialogueMarkName,
}

impl CheckedDialogueMark {
    pub(crate) const fn new(
        coordinate: StableCheckedDialogueMarkCoordinate,
        diagnostic_name: HirDialogueMarkName,
    ) -> Self {
        Self {
            coordinate,
            diagnostic_name,
        }
    }

    pub const fn coordinate(&self) -> &StableCheckedDialogueMarkCoordinate {
        &self.coordinate
    }

    pub const fn diagnostic_name(&self) -> &HirDialogueMarkName {
        &self.diagnostic_name
    }
}

impl PartialEq for CheckedDialogueMark {
    fn eq(&self, other: &Self) -> bool {
        self.coordinate == other.coordinate
    }
}

impl Eq for CheckedDialogueMark {}

impl PartialOrd for CheckedDialogueMark {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CheckedDialogueMark {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.coordinate.cmp(&other.coordinate)
    }
}

impl std::hash::Hash for CheckedDialogueMark {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.coordinate.hash(state);
    }
}

/// One tag that passed identity, argument, value, and default validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRichTextTag {
    id: HirRichTextTagId,
    owner: CheckedRichTextOwner,
    action: CheckedRichTextAction,
    source: HirSourceSite,
}

impl CheckedRichTextTag {
    pub(crate) const fn new(
        id: HirRichTextTagId,
        owner: CheckedRichTextOwner,
        action: CheckedRichTextAction,
        source: HirSourceSite,
    ) -> Self {
        Self {
            id,
            owner,
            action,
            source,
        }
    }

    pub const fn id(&self) -> HirRichTextTagId {
        self.id
    }

    pub const fn owner(&self) -> &CheckedRichTextOwner {
        &self.owner
    }

    pub const fn action(&self) -> &CheckedRichTextAction {
        &self.action
    }

    pub const fn source(&self) -> &HirSourceSite {
        &self.source
    }
}

/// Checked source-owned pairing record for one explicit or inferred close.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRichTextClose {
    open: HirRichTextTagId,
    source: HirSourceSite,
    ordinal: u32,
    synthetic: bool,
}

impl CheckedRichTextClose {
    pub(crate) const fn new(
        open: HirRichTextTagId,
        source: HirSourceSite,
        ordinal: u32,
        synthetic: bool,
    ) -> Self {
        Self {
            open,
            source,
            ordinal,
            synthetic,
        }
    }

    pub const fn open(&self) -> HirRichTextTagId {
        self.open
    }

    pub const fn source(&self) -> &HirSourceSite {
        &self.source
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub const fn is_synthetic(&self) -> bool {
        self.synthetic
    }
}

/// Ordered renderer-neutral dialogue content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedDialogueToken {
    Text(Box<str>),
    RawText(Box<str>),
    Escape(char),
    Ruby {
        base: Box<str>,
        ruby: Box<str>,
    },
    Open(CheckedRichTextTag),
    Close(CheckedRichTextClose),
    InvalidTag {
        tag: HirRichTextTagId,
        source: HirSourceSite,
    },
    Interpolation(ExprId),
    LineBreak(HirLineBreakKind),
}

/// Checked tokens and tags for one final-HIR content owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedDialogueContent {
    id: HirDialogueContentId,
    tokens: Box<[CheckedDialogueToken]>,
    diagnostics_complete: bool,
}

impl CheckedDialogueContent {
    pub(crate) fn new(
        id: HirDialogueContentId,
        tokens: Vec<CheckedDialogueToken>,
        diagnostics_complete: bool,
    ) -> Self {
        Self {
            id,
            tokens: tokens.into_boxed_slice(),
            diagnostics_complete,
        }
    }

    pub const fn id(&self) -> HirDialogueContentId {
        self.id
    }

    pub const fn tokens(&self) -> &[CheckedDialogueToken] {
        &self.tokens
    }

    pub const fn diagnostics_complete(&self) -> bool {
        self.diagnostics_complete
    }
}

/// Typed validation report shared by semantic and rendering consumers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRichTextReport {
    content: CheckedDialogueContent,
    diagnostics: Box<[RichTextAttributeDiagnostic]>,
}

impl CheckedRichTextReport {
    pub(crate) fn new(
        content: CheckedDialogueContent,
        diagnostics: Vec<RichTextAttributeDiagnostic>,
    ) -> Self {
        Self {
            content,
            diagnostics: diagnostics.into_boxed_slice(),
        }
    }

    pub const fn content(&self) -> &CheckedDialogueContent {
        &self.content
    }

    pub const fn diagnostics(&self) -> &[RichTextAttributeDiagnostic] {
        &self.diagnostics
    }

    pub const fn is_valid(&self) -> bool {
        self.content.diagnostics_complete() && self.diagnostics.is_empty()
    }
}
