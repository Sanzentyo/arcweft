//! Typed text input and IME composition protocol.
//!
//! This module models committed text edits, IME preedit/composition, surrounding
//! text requests, and candidate-window geometry as Sans I/O data. Platform
//! adapters normalize TSF, `NSTextInputClient`, `UIKit`, Android
//! `InputConnection`, Wayland text-input, and Web `EditContext` events into
//! these values.

use crate::hit::HitRect;
use crate::input::InteractionTarget;
use core::fmt;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TextByteOffset(pub u32);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TextUtf16Offset(pub u32);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TextScalarOffset(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TextRange<I> {
    start: I,
    end: I,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct TextInputSessionId(pub u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct TextInputSerial(pub u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct TextRevision(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextInput {
    session: TextInputSessionId,
    serial: TextInputSerial,
    operations: Vec<TextInputOperation>,
    privacy: TextInputPrivacy,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextInputPrivacy {
    #[default]
    Plain,
    Sensitive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextInputOperation {
    StartComposition,
    SetComposition(TextCompositionUpdate),
    Commit(TextCommit),
    EndComposition {
        reason: CompositionEndReason,
    },
    DeleteSurrounding {
        before: u32,
        after: u32,
        unit: TextDeleteUnit,
    },
    SetSelection(PlatformTextSelection),
    Command(TextEditCommand),
}

/// Runtime/product write-back boundary for committed text-control values.
///
/// `Change` is emitted only for committed value mutations, never for IME
/// preedit/composition text. `Submit` is emitted for submit commands and can
/// carry the same committed value without being confused with an ordinary
/// change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextControlWriteBackKind {
    Change,
    Submit,
}

/// Text-control value that carries its diagnostics privacy with the value.
#[derive(Clone, Eq, PartialEq)]
pub struct TextControlValue {
    text: String,
    privacy: TextInputPrivacy,
}

/// Player-to-runtime text-control write-back.
///
/// This is owned typed data, not JSON and not an `InteractionPayload::Text`
/// string tunnel.
#[derive(Clone, Eq, PartialEq)]
pub struct TextControlWriteBack {
    target: InteractionTarget,
    session: TextInputSessionId,
    kind: TextControlWriteBackKind,
    value: TextControlValue,
    selection: TextRange<TextByteOffset>,
    revision: TextRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextCompositionUpdate {
    replacement: Option<TextRange<TextByteOffset>>,
    preedit: String,
    selection: TextRange<TextByteOffset>,
    segments: Vec<TextCompositionSegment>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextCompositionSegment {
    range: TextRange<TextByteOffset>,
    kind: TextCompositionSegmentKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextCompositionSegmentKind {
    RawInput,
    Converted,
    TargetConverted,
    TargetNotConverted,
    InputError,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextCommit {
    text: String,
    replacement: Option<TextRange<TextByteOffset>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositionEndReason {
    Committed,
    Cancelled,
    FocusChanged,
    SessionInvalidated,
    PlatformDisabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextDeleteUnit {
    Utf8Byte,
    Utf16CodeUnit,
    UnicodeScalar,
    GraphemeCluster,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformTextSelection {
    range: TextRange<TextByteOffset>,
    affinity: TextSelectionAffinity,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextSelectionAffinity {
    #[default]
    Downstream,
    Upstream,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextEditCommand {
    MoveLeft { selecting: bool },
    MoveRight { selecting: bool },
    MoveUp { selecting: bool },
    MoveDown { selecting: bool },
    MoveWordLeft { selecting: bool },
    MoveWordRight { selecting: bool },
    MoveLineStart { selecting: bool },
    MoveLineEnd { selecting: bool },
    MoveDocumentStart { selecting: bool },
    MoveDocumentEnd { selecting: bool },
    MovePageUp { selecting: bool },
    MovePageDown { selecting: bool },
    Backspace,
    Delete,
    DeleteWordLeft,
    DeleteWordRight,
    SelectWord,
    SelectLine,
    SelectAll,
    Copy,
    Cut,
    Paste,
    Submit,
    Cancel,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextInputClientSnapshot {
    session: TextInputSessionId,
    target: InteractionTarget,
    revision: TextRevision,
    surrounding_text: String,
    surrounding_start: TextByteOffset,
    selection: TextRange<TextByteOffset>,
    composition: Option<TextCompositionUpdate>,
    control_rect: HitRect,
    caret_rect: HitRect,
    character_bounds: Vec<TextCharacterBounds>,
    options: TextInputOptions,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextCharacterBounds {
    pub range: TextRange<TextByteOffset>,
    pub bounds: HitRect,
}

/// A range-tagged rectangle used for selection and composition fragments.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextRangeRect {
    pub range: TextRange<TextByteOffset>,
    pub bounds: HitRect,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TextInputHostCommand {
    Activate {
        session: TextInputSessionId,
        target: InteractionTarget,
        capabilities: TextInputCapabilities,
        snapshot: Box<TextInputClientSnapshot>,
    },
    Update(Box<TextInputClientSnapshot>),
    UpdateGeometry(Box<TextInputGeometrySnapshot>),
    CommitComposition {
        session: TextInputSessionId,
    },
    CancelComposition {
        session: TextInputSessionId,
    },
    Deactivate {
        session: TextInputSessionId,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextInputCapabilities {
    pub surrounding_text: TextInputCapabilitySupport,
    pub delete_surrounding: TextInputCapabilitySupport,
    pub reconversion: TextInputCapabilitySupport,
    pub composition_segments: TextInputCapabilitySupport,
    pub character_bounds: TextInputCapabilitySupport,
    pub programmatic_commit: TextInputCapabilitySupport,
    pub programmatic_cancel: TextInputCapabilitySupport,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextInputCapabilitySupport {
    #[default]
    Unsupported,
    Supported,
    Limited,
    VersionDependent,
    HostDependent,
    SecureRedacted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextInputOptions {
    purpose: TextInputPurpose,
    autocorrect: TextAssistPolicy,
    spellcheck: TextAssistPolicy,
    capitalization: Capitalization,
    enter_key: EnterKeyHint,
    secure: bool,
    multiline: bool,
    selection: TextSelectionPolicy,
    shortcuts: TextShortcutPolicy,
    tab: TextTabPolicy,
    vertical_navigation: TextVerticalNavigationPolicy,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextInputPurpose {
    #[default]
    Text,
    Search,
    Name,
    Email,
    Url,
    Telephone,
    Number,
    Decimal,
    Password,
    Pin,
    Terminal,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextAssistPolicy {
    #[default]
    PlatformDefault,
    Enabled,
    Disabled,
}

/// Whether a text control accepts ranged selection from pointer, keyboard, or
/// platform-native selection updates.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextSelectionPolicy {
    #[default]
    Enabled,
    Disabled,
}

/// Whether a text control accepts product-level edit shortcuts such as
/// select-all, copy, cut, and paste.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextShortcutPolicy {
    #[default]
    Enabled,
    Disabled,
}

/// Product behavior for the Tab key while a text control has focus.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextTabPolicy {
    #[default]
    FocusNavigation,
    InsertTab,
}

/// Up/Down caret behavior for controls whose rendered text can soft-wrap.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextVerticalNavigationPolicy {
    /// Preserve the historical model: Up/Down moves between newline-delimited
    /// logical lines and ignores renderer soft-wrap geometry.
    #[default]
    LogicalLine,
    /// When renderer glyph geometry is available, Up/Down moves between visual
    /// soft-wrap lines while preserving the preferred visual column.
    VisualLine,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Capitalization {
    #[default]
    None,
    Sentences,
    Words,
    Characters,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EnterKeyHint {
    #[default]
    Default,
    Enter,
    Done,
    Go,
    Next,
    Search,
    Send,
}

impl<I> TextRange<I> {
    pub const fn new(start: I, end: I) -> Self {
        Self { start, end }
    }

    pub const fn start(&self) -> &I {
        &self.start
    }

    pub const fn end(&self) -> &I {
        &self.end
    }
}

impl TextByteOffset {
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl TextUtf16Offset {
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl TextInput {
    pub fn new(
        session: TextInputSessionId,
        serial: TextInputSerial,
        operations: Vec<TextInputOperation>,
    ) -> Self {
        Self {
            session,
            serial,
            operations,
            privacy: TextInputPrivacy::Plain,
        }
    }

    pub fn single(
        session: TextInputSessionId,
        serial: TextInputSerial,
        operation: TextInputOperation,
    ) -> Self {
        Self::new(session, serial, vec![operation])
    }

    pub fn committed(
        session: TextInputSessionId,
        serial: TextInputSerial,
        text: impl Into<String>,
    ) -> Self {
        Self::single(
            session,
            serial,
            TextInputOperation::Commit(TextCommit::new(text)),
        )
    }

    #[must_use]
    pub const fn with_privacy(mut self, privacy: TextInputPrivacy) -> Self {
        self.privacy = privacy;
        self
    }

    pub const fn session(&self) -> TextInputSessionId {
        self.session
    }

    pub const fn serial(&self) -> TextInputSerial {
        self.serial
    }

    pub fn operations(&self) -> &[TextInputOperation] {
        &self.operations
    }

    pub const fn privacy(&self) -> TextInputPrivacy {
        self.privacy
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    pub fn into_operations(self) -> Vec<TextInputOperation> {
        self.operations
    }

    pub fn commits_runtime_text_control_value(&self) -> bool {
        self.operations
            .iter()
            .any(TextInputOperation::commits_runtime_text_control_value)
    }

    pub fn submits_runtime_text_control(&self) -> bool {
        self.operations
            .iter()
            .any(TextInputOperation::submits_runtime_text_control)
    }
}

impl TextInputPrivacy {
    pub const fn is_sensitive(self) -> bool {
        matches!(self, Self::Sensitive)
    }
}

impl TextRevision {
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl TextInputOperation {
    pub fn commit(text: impl Into<String>) -> Self {
        Self::Commit(TextCommit::new(text))
    }

    pub const fn commits_runtime_text_control_value(&self) -> bool {
        matches!(
            self,
            Self::Commit(_)
                | Self::DeleteSurrounding { .. }
                | Self::Command(
                    TextEditCommand::Backspace
                        | TextEditCommand::Delete
                        | TextEditCommand::DeleteWordLeft
                        | TextEditCommand::DeleteWordRight
                        | TextEditCommand::Cut
                        | TextEditCommand::Paste,
                )
        )
    }

    pub const fn submits_runtime_text_control(&self) -> bool {
        matches!(self, Self::Command(TextEditCommand::Submit))
    }
}

impl fmt::Debug for TextControlValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TextControlValue")
            .field("text", &self.redacted_for_diagnostics())
            .field("privacy", &self.privacy)
            .finish()
    }
}

impl fmt::Debug for TextControlWriteBack {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TextControlWriteBack")
            .field("target", &self.target)
            .field("session", &self.session)
            .field("kind", &self.kind)
            .field("value", &self.value)
            .field("selection", &self.selection)
            .field("revision", &self.revision)
            .finish()
    }
}

impl TextControlValue {
    pub fn new(text: impl Into<String>, privacy: TextInputPrivacy) -> Self {
        Self {
            text: text.into(),
            privacy,
        }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(text, TextInputPrivacy::Plain)
    }

    pub fn sensitive(text: impl Into<String>) -> Self {
        Self::new(text, TextInputPrivacy::Sensitive)
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub const fn privacy(&self) -> TextInputPrivacy {
        self.privacy
    }

    pub const fn is_sensitive(&self) -> bool {
        self.privacy.is_sensitive()
    }

    pub fn redacted_for_diagnostics(&self) -> String {
        if self.is_sensitive() {
            "<redacted>".to_owned()
        } else {
            self.text.clone()
        }
    }
}

impl TextControlWriteBack {
    pub fn new(
        target: InteractionTarget,
        session: TextInputSessionId,
        kind: TextControlWriteBackKind,
        value: TextControlValue,
        selection: TextRange<TextByteOffset>,
        revision: TextRevision,
    ) -> Self {
        Self {
            target,
            session,
            kind,
            value,
            selection,
            revision,
        }
    }

    pub fn change(
        target: InteractionTarget,
        session: TextInputSessionId,
        value: TextControlValue,
        selection: TextRange<TextByteOffset>,
        revision: TextRevision,
    ) -> Self {
        Self::new(
            target,
            session,
            TextControlWriteBackKind::Change,
            value,
            selection,
            revision,
        )
    }

    pub fn submit(
        target: InteractionTarget,
        session: TextInputSessionId,
        value: TextControlValue,
        selection: TextRange<TextByteOffset>,
        revision: TextRevision,
    ) -> Self {
        Self::new(
            target,
            session,
            TextControlWriteBackKind::Submit,
            value,
            selection,
            revision,
        )
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }

    pub const fn session(&self) -> TextInputSessionId {
        self.session
    }

    pub const fn kind(&self) -> TextControlWriteBackKind {
        self.kind
    }

    pub const fn value(&self) -> &TextControlValue {
        &self.value
    }

    pub const fn selection(&self) -> TextRange<TextByteOffset> {
        self.selection
    }

    pub const fn revision(&self) -> TextRevision {
        self.revision
    }

    pub const fn is_change(&self) -> bool {
        matches!(self.kind, TextControlWriteBackKind::Change)
    }

    pub const fn is_submit(&self) -> bool {
        matches!(self.kind, TextControlWriteBackKind::Submit)
    }
}

impl TextCompositionUpdate {
    pub fn new(preedit: impl Into<String>, selection: TextRange<TextByteOffset>) -> Self {
        Self {
            replacement: None,
            preedit: preedit.into(),
            selection,
            segments: Vec::new(),
        }
    }

    #[must_use]
    pub const fn with_replacement(mut self, replacement: TextRange<TextByteOffset>) -> Self {
        self.replacement = Some(replacement);
        self
    }

    #[must_use]
    pub fn with_segments(mut self, segments: Vec<TextCompositionSegment>) -> Self {
        self.segments = segments;
        self
    }

    pub const fn replacement(&self) -> Option<TextRange<TextByteOffset>> {
        self.replacement
    }

    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    pub const fn selection(&self) -> TextRange<TextByteOffset> {
        self.selection
    }

    pub fn segments(&self) -> &[TextCompositionSegment] {
        &self.segments
    }
}

impl TextCompositionSegment {
    pub const fn new(range: TextRange<TextByteOffset>, kind: TextCompositionSegmentKind) -> Self {
        Self { range, kind }
    }

    pub const fn range(&self) -> TextRange<TextByteOffset> {
        self.range
    }

    pub const fn kind(&self) -> TextCompositionSegmentKind {
        self.kind
    }
}

impl TextCommit {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            replacement: None,
        }
    }

    #[must_use]
    pub const fn with_replacement(mut self, replacement: TextRange<TextByteOffset>) -> Self {
        self.replacement = Some(replacement);
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn replacement(&self) -> Option<TextRange<TextByteOffset>> {
        self.replacement
    }
}

impl PlatformTextSelection {
    pub const fn new(range: TextRange<TextByteOffset>, affinity: TextSelectionAffinity) -> Self {
        Self { range, affinity }
    }

    pub const fn range(self) -> TextRange<TextByteOffset> {
        self.range
    }

    pub const fn affinity(self) -> TextSelectionAffinity {
        self.affinity
    }
}

impl TextInputClientSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session: TextInputSessionId,
        target: InteractionTarget,
        revision: TextRevision,
        surrounding_text: impl Into<String>,
        surrounding_start: TextByteOffset,
        selection: TextRange<TextByteOffset>,
        control_rect: HitRect,
        caret_rect: HitRect,
        options: TextInputOptions,
    ) -> Self {
        Self {
            session,
            target,
            revision,
            surrounding_text: surrounding_text.into(),
            surrounding_start,
            selection,
            composition: None,
            control_rect,
            caret_rect,
            character_bounds: Vec::new(),
            options,
        }
    }

    #[must_use]
    pub fn with_composition(mut self, composition: TextCompositionUpdate) -> Self {
        self.composition = Some(composition);
        self
    }

    #[must_use]
    pub fn with_character_bounds(mut self, character_bounds: Vec<TextCharacterBounds>) -> Self {
        self.character_bounds = character_bounds;
        self
    }

    pub const fn session(&self) -> TextInputSessionId {
        self.session
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }

    pub const fn revision(&self) -> TextRevision {
        self.revision
    }

    pub fn surrounding_text(&self) -> &str {
        &self.surrounding_text
    }

    pub const fn surrounding_start(&self) -> TextByteOffset {
        self.surrounding_start
    }

    pub const fn selection(&self) -> TextRange<TextByteOffset> {
        self.selection
    }

    pub const fn composition(&self) -> Option<&TextCompositionUpdate> {
        self.composition.as_ref()
    }

    pub const fn control_rect(&self) -> HitRect {
        self.control_rect
    }

    pub const fn caret_rect(&self) -> HitRect {
        self.caret_rect
    }

    pub fn character_bounds(&self) -> &[TextCharacterBounds] {
        &self.character_bounds
    }

    pub const fn options(&self) -> &TextInputOptions {
        &self.options
    }

    #[must_use]
    pub fn transformed(mut self, transform: TextGeometryTransform) -> Self {
        self.control_rect = transform.transform_rect(self.control_rect);
        self.caret_rect = transform.transform_rect(self.caret_rect);
        self.character_bounds = transform_character_bounds(&self.character_bounds, transform);
        self
    }
}

impl Default for TextInputOptions {
    fn default() -> Self {
        Self {
            purpose: TextInputPurpose::Text,
            autocorrect: TextAssistPolicy::PlatformDefault,
            spellcheck: TextAssistPolicy::PlatformDefault,
            capitalization: Capitalization::None,
            enter_key: EnterKeyHint::Default,
            secure: false,
            multiline: false,
            selection: TextSelectionPolicy::Enabled,
            shortcuts: TextShortcutPolicy::Enabled,
            tab: TextTabPolicy::FocusNavigation,
            vertical_navigation: TextVerticalNavigationPolicy::LogicalLine,
        }
    }
}

impl TextInputOptions {
    pub const fn purpose(&self) -> TextInputPurpose {
        self.purpose
    }

    #[must_use]
    pub const fn with_purpose(mut self, purpose: TextInputPurpose) -> Self {
        self.purpose = purpose;
        self
    }

    #[must_use]
    pub const fn with_autocorrect(mut self, autocorrect: TextAssistPolicy) -> Self {
        self.autocorrect = autocorrect;
        self
    }

    #[must_use]
    pub const fn with_spellcheck(mut self, spellcheck: TextAssistPolicy) -> Self {
        self.spellcheck = spellcheck;
        self
    }

    #[must_use]
    pub const fn with_capitalization(mut self, capitalization: Capitalization) -> Self {
        self.capitalization = capitalization;
        self
    }

    #[must_use]
    pub const fn with_enter_key(mut self, enter_key: EnterKeyHint) -> Self {
        self.enter_key = enter_key;
        self
    }

    #[must_use]
    pub const fn secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self
    }

    #[must_use]
    pub const fn multiline(mut self, multiline: bool) -> Self {
        self.multiline = multiline;
        self
    }

    #[must_use]
    pub const fn with_selection_policy(mut self, selection: TextSelectionPolicy) -> Self {
        self.selection = selection;
        self
    }

    #[must_use]
    pub const fn with_shortcut_policy(mut self, shortcuts: TextShortcutPolicy) -> Self {
        self.shortcuts = shortcuts;
        self
    }

    #[must_use]
    pub const fn with_tab_policy(mut self, tab: TextTabPolicy) -> Self {
        self.tab = tab;
        self
    }

    #[must_use]
    pub const fn with_vertical_navigation_policy(
        mut self,
        vertical_navigation: TextVerticalNavigationPolicy,
    ) -> Self {
        self.vertical_navigation = vertical_navigation;
        self
    }

    pub const fn autocorrect(&self) -> TextAssistPolicy {
        self.autocorrect
    }

    pub const fn spellcheck(&self) -> TextAssistPolicy {
        self.spellcheck
    }

    pub const fn capitalization(&self) -> Capitalization {
        self.capitalization
    }

    pub const fn enter_key(&self) -> EnterKeyHint {
        self.enter_key
    }

    pub const fn is_secure(&self) -> bool {
        self.secure
    }

    pub const fn is_multiline(&self) -> bool {
        self.multiline
    }

    pub const fn selection_policy(&self) -> TextSelectionPolicy {
        self.selection
    }

    pub const fn shortcut_policy(&self) -> TextShortcutPolicy {
        self.shortcuts
    }

    pub const fn tab_policy(&self) -> TextTabPolicy {
        self.tab
    }

    pub const fn vertical_navigation_policy(&self) -> TextVerticalNavigationPolicy {
        self.vertical_navigation
    }

    pub const fn selection_enabled(&self) -> bool {
        matches!(self.selection, TextSelectionPolicy::Enabled)
    }

    pub const fn shortcuts_enabled(&self) -> bool {
        matches!(self.shortcuts, TextShortcutPolicy::Enabled)
    }

    pub const fn tab_inserts_text(&self) -> bool {
        matches!(self.tab, TextTabPolicy::InsertTab)
    }

    pub const fn visual_line_vertical_navigation_enabled(&self) -> bool {
        matches!(
            self.vertical_navigation,
            TextVerticalNavigationPolicy::VisualLine
        )
    }
}

/// Monotonic focus generation owned by the runtime host.
///
/// Platform adapters attach this to native IME callbacks so delayed commits or
/// preedit updates from an older focus transaction can be rejected without
/// comparing OS handles in Sans I/O data.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct TextInputFocusGeneration(pub u64);

/// Explicit platform IME API family normalized into Arcweft text-input batches.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TextInputAdapterKind {
    WindowsTsf,
    MacosTextInputClient,
    WaylandTextInputV3,
    AndroidInputConnection,
    IosTextInput,
    WebEditContext,
}

/// Browser IME support decision.
///
/// `UnsupportedNoFallback` is intentionally visible so Web hosts report that
/// the current browser lacks an explicit editing API instead of silently
/// installing a hidden DOM textarea fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebTextInputApiSupport {
    EditContext,
    UnsupportedNoFallback,
}

/// Writing mode used when converting text-local candidate geometry.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextWritingMode {
    #[default]
    HorizontalTb,
    VerticalRl,
    VerticalLr,
}

/// Affine transform used for text-local -> viewport -> screen geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextGeometryTransform {
    pub m11: f32,
    pub m12: f32,
    pub m21: f32,
    pub m22: f32,
    pub dx: f32,
    pub dy: f32,
}

/// Native adapter event context carried with every platform callback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformTextInputContext {
    adapter: TextInputAdapterKind,
    session: TextInputSessionId,
    generation: TextInputFocusGeneration,
    target: InteractionTarget,
    serial: TextInputSerial,
}

/// Platform-normalized IME event before runtime-host validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlatformTextInputEvent {
    StartComposition(PlatformTextInputContext),
    SetComposition {
        context: PlatformTextInputContext,
        update: TextCompositionUpdate,
    },
    Commit {
        context: PlatformTextInputContext,
        commit: TextCommit,
    },
    EndComposition {
        context: PlatformTextInputContext,
        reason: CompositionEndReason,
    },
    DeleteSurrounding {
        context: PlatformTextInputContext,
        before: u32,
        after: u32,
        unit: TextDeleteUnit,
    },
    SetSelection {
        context: PlatformTextInputContext,
        selection: PlatformTextSelection,
    },
    Command {
        context: PlatformTextInputContext,
        command: TextEditCommand,
    },
    Batch {
        context: PlatformTextInputContext,
        operations: Vec<TextInputOperation>,
    },
}

/// Key handling outcome reported by platform IME adapters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextInputKeyDisposition {
    #[default]
    ShortcutCandidate,
    ImeConsumed,
}

/// Policy applied when focus leaves a composing text field.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextInputBlurPolicy {
    CommitComposition,
    CancelComposition,
    #[default]
    PlatformDefault,
}

/// Secure-field redaction policy shared by platform adapters, replay, and UI.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextInputSecurityPolicy {
    #[default]
    Plain,
    SecureRedacted,
}

/// Candidate and character-bound geometry after text-local transforms.
#[derive(Clone, Debug, PartialEq)]
pub struct TextInputGeometrySnapshot {
    session: TextInputSessionId,
    revision: TextRevision,
    writing_mode: TextWritingMode,
    text_local_control_rect: HitRect,
    text_local_caret_rect: HitRect,
    text_local_character_bounds: Vec<TextCharacterBounds>,
    text_local_selection_rects: Vec<TextRangeRect>,
    text_local_composition_rects: Vec<TextRangeRect>,
    viewport_control_rect: HitRect,
    screen_control_rect: HitRect,
    viewport_caret_rect: HitRect,
    viewport_character_bounds: Vec<TextCharacterBounds>,
    viewport_selection_rects: Vec<TextRangeRect>,
    viewport_composition_rects: Vec<TextRangeRect>,
    screen_caret_rect: HitRect,
    screen_character_bounds: Vec<TextCharacterBounds>,
    screen_selection_rects: Vec<TextRangeRect>,
    screen_composition_rects: Vec<TextRangeRect>,
}

/// Text-local geometry inputs used to construct a transformed IME snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct TextInputGeometrySnapshotParts {
    pub session: TextInputSessionId,
    pub revision: TextRevision,
    pub writing_mode: TextWritingMode,
    pub text_local_control_rect: HitRect,
    pub text_local_caret_rect: HitRect,
    pub text_local_character_bounds: Vec<TextCharacterBounds>,
    pub text_local_selection_rects: Vec<TextRangeRect>,
    pub text_local_composition_rects: Vec<TextRangeRect>,
    pub text_local_to_viewport: TextGeometryTransform,
    pub viewport_to_screen: TextGeometryTransform,
}

impl TextInputFocusGeneration {
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl TextGeometryTransform {
    pub const fn identity() -> Self {
        Self {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            dx: 0.0,
            dy: 0.0,
        }
    }

    pub const fn translation(dx: f32, dy: f32) -> Self {
        Self {
            dx,
            dy,
            ..Self::identity()
        }
    }

    pub const fn scale(x: f32, y: f32) -> Self {
        Self {
            m11: x,
            m22: y,
            ..Self::identity()
        }
    }

    #[must_use]
    pub fn then(self, next: Self) -> Self {
        Self {
            m11: next.m11.mul_add(self.m11, next.m21 * self.m12),
            m12: next.m12.mul_add(self.m11, next.m22 * self.m12),
            m21: next.m11.mul_add(self.m21, next.m21 * self.m22),
            m22: next.m12.mul_add(self.m21, next.m22 * self.m22),
            dx: next
                .m11
                .mul_add(self.dx, next.m21.mul_add(self.dy, next.dx)),
            dy: next
                .m12
                .mul_add(self.dx, next.m22.mul_add(self.dy, next.dy)),
        }
    }

    pub fn transform_point(self, x: f32, y: f32) -> (f32, f32) {
        (
            self.m11.mul_add(x, self.m21.mul_add(y, self.dx)),
            self.m12.mul_add(x, self.m22.mul_add(y, self.dy)),
        )
    }

    #[must_use]
    pub fn transform_rect(self, rect: HitRect) -> HitRect {
        let points = [
            self.transform_point(rect.x, rect.y),
            self.transform_point(rect.x + rect.width, rect.y),
            self.transform_point(rect.x, rect.y + rect.height),
            self.transform_point(rect.x + rect.width, rect.y + rect.height),
        ];
        let (min_x, max_x) = points
            .iter()
            .map(|(x, _)| *x)
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(min_x, max_x), x| {
                (min_x.min(x), max_x.max(x))
            });
        let (min_y, max_y) = points
            .iter()
            .map(|(_, y)| *y)
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(min_y, max_y), y| {
                (min_y.min(y), max_y.max(y))
            });
        HitRect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }
}

impl Default for TextGeometryTransform {
    fn default() -> Self {
        Self::identity()
    }
}

impl PlatformTextInputContext {
    pub fn new(
        adapter: TextInputAdapterKind,
        session: TextInputSessionId,
        generation: TextInputFocusGeneration,
        target: InteractionTarget,
        serial: TextInputSerial,
    ) -> Self {
        Self {
            adapter,
            session,
            generation,
            target,
            serial,
        }
    }

    pub const fn adapter(&self) -> TextInputAdapterKind {
        self.adapter
    }

    pub const fn session(&self) -> TextInputSessionId {
        self.session
    }

    pub const fn generation(&self) -> TextInputFocusGeneration {
        self.generation
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }

    pub const fn serial(&self) -> TextInputSerial {
        self.serial
    }
}

impl PlatformTextInputEvent {
    pub const fn context(&self) -> &PlatformTextInputContext {
        match self {
            Self::StartComposition(context)
            | Self::SetComposition { context, .. }
            | Self::Commit { context, .. }
            | Self::EndComposition { context, .. }
            | Self::DeleteSurrounding { context, .. }
            | Self::SetSelection { context, .. }
            | Self::Command { context, .. }
            | Self::Batch { context, .. } => context,
        }
    }

    pub fn into_text_input(self, privacy: TextInputPrivacy) -> TextInput {
        let context = self.context().clone();
        let operations = match self {
            Self::StartComposition(_) => vec![TextInputOperation::StartComposition],
            Self::SetComposition { update, .. } => vec![TextInputOperation::SetComposition(update)],
            Self::Commit { commit, .. } => vec![TextInputOperation::Commit(commit)],
            Self::EndComposition { reason, .. } => {
                vec![TextInputOperation::EndComposition { reason }]
            }
            Self::DeleteSurrounding {
                before,
                after,
                unit,
                ..
            } => vec![TextInputOperation::DeleteSurrounding {
                before,
                after,
                unit,
            }],
            Self::SetSelection { selection, .. } => {
                vec![TextInputOperation::SetSelection(selection)]
            }
            Self::Command { command, .. } => vec![TextInputOperation::Command(command)],
            Self::Batch { operations, .. } => operations,
        };
        TextInput::new(context.session, context.serial, operations).with_privacy(privacy)
    }
}

impl TextInputKeyDisposition {
    pub const fn shortcuts_suppressed(self) -> bool {
        matches!(self, Self::ImeConsumed)
    }
}

impl TextInputSecurityPolicy {
    pub const fn from_options(options: &TextInputOptions) -> Self {
        if options.is_secure() {
            Self::SecureRedacted
        } else {
            Self::Plain
        }
    }

    pub const fn input_privacy(self) -> TextInputPrivacy {
        match self {
            Self::Plain => TextInputPrivacy::Plain,
            Self::SecureRedacted => TextInputPrivacy::Sensitive,
        }
    }

    pub const fn exposes_surrounding_text(self) -> bool {
        matches!(self, Self::Plain)
    }

    pub const fn allows_reconversion(self) -> bool {
        matches!(self, Self::Plain)
    }

    pub const fn allows_clipboard(self) -> bool {
        matches!(self, Self::Plain)
    }

    pub const fn allows_agent_observe_value(self) -> bool {
        matches!(self, Self::Plain)
    }

    pub const fn allows_capture_metadata(self) -> bool {
        matches!(self, Self::Plain)
    }

    pub fn redact_snapshot(self, snapshot: &TextInputClientSnapshot) -> TextInputClientSnapshot {
        match self {
            Self::Plain => snapshot.clone(),
            Self::SecureRedacted => snapshot.redacted_for_secure_input(),
        }
    }

    pub fn redact_geometry(
        self,
        geometry: &TextInputGeometrySnapshot,
    ) -> TextInputGeometrySnapshot {
        match self {
            Self::Plain => geometry.clone(),
            Self::SecureRedacted => geometry.redacted_for_secure_input(),
        }
    }
}

impl TextInputCapabilities {
    pub const fn all_supported() -> Self {
        Self {
            surrounding_text: TextInputCapabilitySupport::Supported,
            delete_surrounding: TextInputCapabilitySupport::Supported,
            reconversion: TextInputCapabilitySupport::Supported,
            composition_segments: TextInputCapabilitySupport::Supported,
            character_bounds: TextInputCapabilitySupport::Supported,
            programmatic_commit: TextInputCapabilitySupport::Supported,
            programmatic_cancel: TextInputCapabilitySupport::Supported,
        }
    }

    pub const fn for_platform_adapter(adapter: TextInputAdapterKind) -> Self {
        match adapter {
            TextInputAdapterKind::WindowsTsf
            | TextInputAdapterKind::MacosTextInputClient
            | TextInputAdapterKind::WaylandTextInputV3
            | TextInputAdapterKind::AndroidInputConnection
            | TextInputAdapterKind::IosTextInput
            | TextInputAdapterKind::WebEditContext => Self::all_supported(),
        }
    }

    pub const fn secure_redacted() -> Self {
        Self {
            surrounding_text: TextInputCapabilitySupport::SecureRedacted,
            delete_surrounding: TextInputCapabilitySupport::Supported,
            reconversion: TextInputCapabilitySupport::SecureRedacted,
            composition_segments: TextInputCapabilitySupport::SecureRedacted,
            character_bounds: TextInputCapabilitySupport::SecureRedacted,
            programmatic_commit: TextInputCapabilitySupport::Supported,
            programmatic_cancel: TextInputCapabilitySupport::Supported,
        }
    }

    pub const fn for_web_support(support: WebTextInputApiSupport) -> Option<Self> {
        match support {
            WebTextInputApiSupport::EditContext => Some(Self::for_platform_adapter(
                TextInputAdapterKind::WebEditContext,
            )),
            WebTextInputApiSupport::UnsupportedNoFallback => None,
        }
    }

    #[must_use]
    pub const fn narrow_for_security(self, security: TextInputSecurityPolicy) -> Self {
        match security {
            TextInputSecurityPolicy::Plain => self,
            TextInputSecurityPolicy::SecureRedacted => Self {
                surrounding_text: TextInputCapabilitySupport::SecureRedacted,
                reconversion: TextInputCapabilitySupport::SecureRedacted,
                composition_segments: TextInputCapabilitySupport::SecureRedacted,
                character_bounds: TextInputCapabilitySupport::SecureRedacted,
                ..self
            },
        }
    }
}

impl TextInputCapabilitySupport {
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Supported | Self::Limited)
    }
}

impl TextCharacterBounds {
    pub const fn new(range: TextRange<TextByteOffset>, bounds: HitRect) -> Self {
        Self { range, bounds }
    }
}

impl TextRangeRect {
    pub const fn new(range: TextRange<TextByteOffset>, bounds: HitRect) -> Self {
        Self { range, bounds }
    }
}

impl TextInputClientSnapshot {
    #[must_use]
    pub fn redacted_for_secure_input(&self) -> Self {
        Self {
            session: self.session,
            target: self.target.clone(),
            revision: self.revision,
            surrounding_text: String::new(),
            surrounding_start: TextByteOffset(0),
            selection: TextRange::new(TextByteOffset(0), TextByteOffset(0)),
            composition: None,
            control_rect: self.control_rect,
            caret_rect: self.control_rect,
            character_bounds: Vec::new(),
            options: self.options.clone().secure(true),
        }
    }
}

impl TextInputGeometrySnapshot {
    pub fn new(parts: TextInputGeometrySnapshotParts) -> Self {
        let TextInputGeometrySnapshotParts {
            session,
            revision,
            writing_mode,
            text_local_control_rect,
            text_local_caret_rect,
            text_local_character_bounds,
            text_local_selection_rects,
            text_local_composition_rects,
            text_local_to_viewport,
            viewport_to_screen,
        } = parts;
        let text_local_to_screen = text_local_to_viewport.then(viewport_to_screen);
        let viewport_character_bounds =
            transform_character_bounds(&text_local_character_bounds, text_local_to_viewport);
        let screen_character_bounds =
            transform_character_bounds(&text_local_character_bounds, text_local_to_screen);
        let viewport_selection_rects =
            transform_range_rects(&text_local_selection_rects, text_local_to_viewport);
        let screen_selection_rects =
            transform_range_rects(&text_local_selection_rects, text_local_to_screen);
        let viewport_composition_rects =
            transform_range_rects(&text_local_composition_rects, text_local_to_viewport);
        let screen_composition_rects =
            transform_range_rects(&text_local_composition_rects, text_local_to_screen);
        Self {
            session,
            revision,
            writing_mode,
            text_local_control_rect,
            text_local_caret_rect,
            text_local_character_bounds,
            text_local_selection_rects,
            text_local_composition_rects,
            viewport_control_rect: text_local_to_viewport.transform_rect(text_local_control_rect),
            screen_control_rect: text_local_to_screen.transform_rect(text_local_control_rect),
            viewport_caret_rect: text_local_to_viewport.transform_rect(text_local_caret_rect),
            viewport_character_bounds,
            viewport_selection_rects,
            viewport_composition_rects,
            screen_caret_rect: text_local_to_screen.transform_rect(text_local_caret_rect),
            screen_character_bounds,
            screen_selection_rects,
            screen_composition_rects,
        }
    }

    pub const fn session(&self) -> TextInputSessionId {
        self.session
    }

    pub const fn revision(&self) -> TextRevision {
        self.revision
    }

    pub const fn writing_mode(&self) -> TextWritingMode {
        self.writing_mode
    }

    pub const fn text_local_control_rect(&self) -> HitRect {
        self.text_local_control_rect
    }

    pub const fn text_local_caret_rect(&self) -> HitRect {
        self.text_local_caret_rect
    }

    pub fn text_local_character_bounds(&self) -> &[TextCharacterBounds] {
        &self.text_local_character_bounds
    }

    pub fn text_local_selection_rects(&self) -> &[TextRangeRect] {
        &self.text_local_selection_rects
    }

    pub fn text_local_composition_rects(&self) -> &[TextRangeRect] {
        &self.text_local_composition_rects
    }

    pub const fn viewport_control_rect(&self) -> HitRect {
        self.viewport_control_rect
    }

    pub const fn screen_control_rect(&self) -> HitRect {
        self.screen_control_rect
    }

    pub const fn viewport_caret_rect(&self) -> HitRect {
        self.viewport_caret_rect
    }

    pub fn viewport_character_bounds(&self) -> &[TextCharacterBounds] {
        &self.viewport_character_bounds
    }

    pub fn viewport_selection_rects(&self) -> &[TextRangeRect] {
        &self.viewport_selection_rects
    }

    pub fn viewport_composition_rects(&self) -> &[TextRangeRect] {
        &self.viewport_composition_rects
    }

    pub const fn screen_caret_rect(&self) -> HitRect {
        self.screen_caret_rect
    }

    pub const fn candidate_anchor_rect(&self) -> HitRect {
        self.screen_caret_rect
    }

    pub fn screen_character_bounds(&self) -> &[TextCharacterBounds] {
        &self.screen_character_bounds
    }

    pub fn screen_selection_rects(&self) -> &[TextRangeRect] {
        &self.screen_selection_rects
    }

    pub fn screen_composition_rects(&self) -> &[TextRangeRect] {
        &self.screen_composition_rects
    }

    #[must_use]
    pub fn transformed_viewport(
        mut self,
        viewport_transform: TextGeometryTransform,
        viewport_to_screen: TextGeometryTransform,
    ) -> Self {
        self.viewport_control_rect = viewport_transform.transform_rect(self.viewport_control_rect);
        self.viewport_caret_rect = viewport_transform.transform_rect(self.viewport_caret_rect);
        self.viewport_character_bounds =
            transform_character_bounds(&self.viewport_character_bounds, viewport_transform);
        self.viewport_selection_rects =
            transform_range_rects(&self.viewport_selection_rects, viewport_transform);
        self.viewport_composition_rects =
            transform_range_rects(&self.viewport_composition_rects, viewport_transform);
        self.screen_control_rect = viewport_to_screen.transform_rect(self.viewport_control_rect);
        self.screen_caret_rect = viewport_to_screen.transform_rect(self.viewport_caret_rect);
        self.screen_character_bounds =
            transform_character_bounds(&self.viewport_character_bounds, viewport_to_screen);
        self.screen_selection_rects =
            transform_range_rects(&self.viewport_selection_rects, viewport_to_screen);
        self.screen_composition_rects =
            transform_range_rects(&self.viewport_composition_rects, viewport_to_screen);
        self
    }

    #[must_use]
    pub fn redacted_for_secure_input(&self) -> Self {
        Self {
            session: self.session,
            revision: self.revision,
            writing_mode: self.writing_mode,
            text_local_control_rect: self.text_local_control_rect,
            text_local_caret_rect: self.text_local_control_rect,
            text_local_character_bounds: Vec::new(),
            text_local_selection_rects: Vec::new(),
            text_local_composition_rects: Vec::new(),
            viewport_control_rect: self.viewport_control_rect,
            screen_control_rect: self.screen_control_rect,
            viewport_caret_rect: self.viewport_control_rect,
            viewport_character_bounds: Vec::new(),
            viewport_selection_rects: Vec::new(),
            viewport_composition_rects: Vec::new(),
            screen_caret_rect: self.screen_control_rect,
            screen_character_bounds: Vec::new(),
            screen_selection_rects: Vec::new(),
            screen_composition_rects: Vec::new(),
        }
    }
}

fn transform_character_bounds(
    bounds: &[TextCharacterBounds],
    transform: TextGeometryTransform,
) -> Vec<TextCharacterBounds> {
    bounds
        .iter()
        .map(|bounds| {
            TextCharacterBounds::new(bounds.range, transform.transform_rect(bounds.bounds))
        })
        .collect()
}

fn transform_range_rects(
    rects: &[TextRangeRect],
    transform: TextGeometryTransform,
) -> Vec<TextRangeRect> {
    rects
        .iter()
        .map(|rect| TextRangeRect::new(rect.range, transform.transform_rect(rect.bounds)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_control_write_back_value_debug_redacts_sensitive_text() {
        let value = TextControlValue::sensitive("secret");

        let debug = format!("{value:?}");

        assert_eq!(value.as_str(), "secret");
        assert!(!debug.contains("secret"));
        assert!(debug.contains("<redacted>"));
    }

    #[test]
    fn text_control_write_back_kind_detection_separates_change_and_submit() {
        let commit = TextInputOperation::commit("x");
        let submit = TextInputOperation::Command(TextEditCommand::Submit);
        let preedit = TextInputOperation::SetComposition(TextCompositionUpdate::new(
            "x",
            TextRange::new(TextByteOffset(0), TextByteOffset(1)),
        ));

        assert!(commit.commits_runtime_text_control_value());
        assert!(!commit.submits_runtime_text_control());
        assert!(submit.submits_runtime_text_control());
        assert!(!submit.commits_runtime_text_control_value());
        assert!(!preedit.commits_runtime_text_control_value());
        assert!(!preedit.submits_runtime_text_control());
    }
}
