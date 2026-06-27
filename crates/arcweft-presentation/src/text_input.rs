//! Typed text input and IME composition protocol.
//!
//! This module models committed text edits, IME preedit/composition, surrounding
//! text requests, and candidate-window geometry as Sans I/O data. Platform
//! adapters normalize TSF, `NSTextInputClient`, `UIKit`, Android
//! `InputConnection`, Wayland text-input, and Web `EditContext` events into
//! these values.

use crate::hit::HitRect;
use crate::input::InteractionTarget;

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
    MoveWordLeft { selecting: bool },
    MoveWordRight { selecting: bool },
    MoveLineStart { selecting: bool },
    MoveLineEnd { selecting: bool },
    Backspace,
    Delete,
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

#[derive(Clone, Debug, PartialEq)]
pub enum TextInputHostCommand {
    Activate {
        session: TextInputSessionId,
        target: InteractionTarget,
        snapshot: Box<TextInputClientSnapshot>,
    },
    Update(Box<TextInputClientSnapshot>),
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
        }
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

    pub fn into_operations(self) -> Vec<TextInputOperation> {
        self.operations
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

    pub const fn range(self) -> TextRange<TextByteOffset> {
        self.range
    }

    pub const fn kind(self) -> TextCompositionSegmentKind {
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
}
