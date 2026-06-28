//! macOS AppKit `NSTextInputClient` adapter core.
//!
//! AppKit objects, selectors, attributed strings, and native object identity
//! stay at the adapter boundary. This module accepts owned strings and
//! adapter-local `MacosNativeRange` values, resolves them through
//! `TextIndexSnapshot`, and emits only canonical Arcweft text-input payloads.

use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::text_index::{TextIndexError, TextIndexSnapshot};
use arcweft_presentation::text_input::{
    CompositionEndReason, PlatformTextInputContext, PlatformTextInputEvent, PlatformTextSelection,
    TextByteOffset, TextCharacterBounds, TextCommit, TextCompositionUpdate, TextInputAdapterKind,
    TextInputClientSnapshot, TextInputFocusGeneration, TextInputGeometrySnapshot,
    TextInputOperation, TextInputSecurityPolicy, TextInputSerial, TextInputSessionId, TextRange,
    TextRevision, TextSelectionAffinity, TextUtf16Offset,
};
use core::fmt;

pub const MACOS_NS_NOT_FOUND: u64 = u64::MAX;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MacosNativeRange {
    location: u64,
    length: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MacosTextInputAdapter {
    session: TextInputSessionId,
    target: InteractionTarget,
    generation: TextInputFocusGeneration,
    revision: TextRevision,
    serial: TextInputSerial,
    index: TextIndexSnapshot,
    selection: TextRange<TextByteOffset>,
    marked_range: Option<TextRange<TextByteOffset>>,
    marked_text: Option<String>,
    security: TextInputSecurityPolicy,
    diagnostics: MacosTextInputActivationDiagnostics,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacosTextInputActivationDiagnostics {
    surrounding_text: MacosCapabilityState,
    marked_text: MacosCapabilityState,
    selected_range: MacosCapabilityState,
    replacement_range: MacosCapabilityState,
    first_rect: MacosCapabilityState,
    character_bounds: MacosCapabilityState,
    reconversion: MacosCapabilityState,
    composition_segments: MacosCapabilityState,
    programmatic_commit: MacosCapabilityState,
    programmatic_cancel: MacosCapabilityState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MacosCapabilityState {
    Supported,
    Limited,
    HostDependent,
    SecureRedacted,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MacosTextInputCallback {
    event: PlatformTextInputEvent,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MacosAppKitRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MacosScreenCoordinateSpace {
    screen_height_points: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MacosFirstRectQueryResult {
    rect: MacosAppKitRect,
    actual_range: MacosNativeRange,
    availability: MacosFirstRectAvailability,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MacosFirstRectAvailability {
    ExactCharacterBounds,
    CaretFallback,
    SecureRedacted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacosAttributedSubstring {
    text: String,
    actual_range: MacosNativeRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MacosTextInputError {
    InvalidNativeRange {
        role: &'static str,
        range: MacosNativeRange,
        reason: MacosNativeRangeError,
    },
    TextIndex {
        role: &'static str,
        source: TextIndexError,
    },
    SecureRedacted {
        request: &'static str,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MacosNativeRangeError {
    NsNotFoundNotAllowed,
    EndOverflow,
    ExceedsU32OffsetModel,
}

impl MacosNativeRange {
    pub const fn new(location: u64, length: u64) -> Self {
        Self { location, length }
    }

    pub const fn not_found() -> Self {
        Self {
            location: MACOS_NS_NOT_FOUND,
            length: 0,
        }
    }

    pub const fn location(self) -> u64 {
        self.location
    }

    pub const fn length(self) -> u64 {
        self.length
    }

    pub const fn is_not_found(self) -> bool {
        self.location == MACOS_NS_NOT_FOUND
    }

    fn checked_utf16_range(
        self,
        role: &'static str,
    ) -> Result<TextRange<TextUtf16Offset>, MacosTextInputError> {
        if self.is_not_found() {
            return Err(MacosTextInputError::InvalidNativeRange {
                role,
                range: self,
                reason: MacosNativeRangeError::NsNotFoundNotAllowed,
            });
        }
        let end = self.location.checked_add(self.length).ok_or(
            MacosTextInputError::InvalidNativeRange {
                role,
                range: self,
                reason: MacosNativeRangeError::EndOverflow,
            },
        )?;
        if end > u64::from(u32::MAX) {
            return Err(MacosTextInputError::InvalidNativeRange {
                role,
                range: self,
                reason: MacosNativeRangeError::ExceedsU32OffsetModel,
            });
        }
        Ok(TextRange::new(
            TextUtf16Offset(self.location as u32),
            TextUtf16Offset(end as u32),
        ))
    }

    fn from_utf16_range(range: TextRange<TextUtf16Offset>) -> Self {
        Self::new(
            u64::from(range.start().0),
            u64::from(range.end().0.saturating_sub(range.start().0)),
        )
    }
}

impl MacosTextInputAdapter {
    pub fn activate(
        snapshot: &TextInputClientSnapshot,
        generation: TextInputFocusGeneration,
    ) -> Result<Self, MacosTextInputError> {
        let security = TextInputSecurityPolicy::from_options(snapshot.options());
        let index = TextIndexSnapshot::try_new(snapshot.surrounding_text().to_owned()).map_err(
            |source| MacosTextInputError::TextIndex {
                role: "activation.surrounding_text",
                source,
            },
        )?;
        Ok(Self {
            session: snapshot.session(),
            target: snapshot.target().clone(),
            generation,
            revision: snapshot.revision(),
            serial: TextInputSerial(0),
            index,
            selection: snapshot.selection(),
            marked_range: snapshot.composition().map(TextCompositionUpdate::selection),
            marked_text: snapshot
                .composition()
                .map(|composition| composition.preedit().to_owned()),
            security,
            diagnostics: MacosTextInputActivationDiagnostics::for_security(security),
        })
    }

    pub const fn session(&self) -> TextInputSessionId {
        self.session
    }

    pub const fn generation(&self) -> TextInputFocusGeneration {
        self.generation
    }

    pub const fn revision(&self) -> TextRevision {
        self.revision
    }

    pub const fn diagnostics(&self) -> &MacosTextInputActivationDiagnostics {
        &self.diagnostics
    }

    pub fn set_marked_text(
        &mut self,
        marked_text: impl Into<String>,
        selected_range: MacosNativeRange,
        replacement_range: MacosNativeRange,
    ) -> Result<MacosTextInputCallback, MacosTextInputError> {
        let marked_text = marked_text.into();
        let marked_index = TextIndexSnapshot::try_new(marked_text.clone()).map_err(|source| {
            MacosTextInputError::TextIndex {
                role: "set_marked_text.marked_text",
                source,
            }
        })?;
        let selection = Self::resolve_range_in(
            "set_marked_text.selected_range",
            &marked_index,
            selected_range,
        )?;
        let replacement = self.resolve_optional_document_range(
            "set_marked_text.replacement_range",
            replacement_range,
        )?;
        let update = match replacement {
            Some(replacement) => TextCompositionUpdate::new(marked_text.clone(), selection)
                .with_replacement(replacement),
            None => TextCompositionUpdate::new(marked_text.clone(), selection),
        };
        let context = self.next_context();
        let event = if self.marked_text.is_some() {
            PlatformTextInputEvent::SetComposition { context, update }
        } else {
            PlatformTextInputEvent::Batch {
                context,
                operations: vec![
                    TextInputOperation::StartComposition,
                    TextInputOperation::SetComposition(update),
                ],
            }
        };
        self.marked_range = replacement.or(Some(self.selection));
        self.marked_text = Some(marked_text);
        Ok(MacosTextInputCallback { event })
    }

    pub fn insert_text(
        &mut self,
        text: impl Into<String>,
        replacement_range: MacosNativeRange,
    ) -> Result<MacosTextInputCallback, MacosTextInputError> {
        let text = text.into();
        let replacement = self
            .resolve_optional_document_range("insert_text.replacement_range", replacement_range)?;
        let commit = match replacement {
            Some(replacement) => TextCommit::new(text).with_replacement(replacement),
            None => TextCommit::new(text),
        };
        self.marked_text = None;
        self.marked_range = None;
        Ok(MacosTextInputCallback {
            event: PlatformTextInputEvent::Commit {
                context: self.next_context(),
                commit,
            },
        })
    }

    pub fn unmark_text(&mut self) -> MacosTextInputCallback {
        self.end_composition(CompositionEndReason::Committed)
    }

    pub fn cancel_marked_text(&mut self) -> MacosTextInputCallback {
        self.end_composition(CompositionEndReason::Cancelled)
    }

    pub fn set_selected_range(
        &mut self,
        selected_range: MacosNativeRange,
        affinity: TextSelectionAffinity,
    ) -> Result<MacosTextInputCallback, MacosTextInputError> {
        let range =
            self.resolve_document_range("set_selected_range.selected_range", selected_range)?;
        self.selection = range;
        Ok(MacosTextInputCallback {
            event: PlatformTextInputEvent::SetSelection {
                context: self.next_context(),
                selection: PlatformTextSelection::new(range, affinity),
            },
        })
    }

    pub fn selected_range_for_appkit(&self) -> Result<MacosNativeRange, MacosTextInputError> {
        if matches!(self.security, TextInputSecurityPolicy::SecureRedacted) {
            return Err(MacosTextInputError::SecureRedacted {
                request: "selectedRange",
            });
        }
        self.index
            .utf16_range_from_byte(self.selection)
            .map(MacosNativeRange::from_utf16_range)
            .map_err(|source| MacosTextInputError::TextIndex {
                role: "selectedRange",
                source,
            })
    }

    pub fn attributed_substring_for_proposed_range(
        &self,
        proposed_range: MacosNativeRange,
    ) -> Result<MacosAttributedSubstring, MacosTextInputError> {
        if matches!(self.security, TextInputSecurityPolicy::SecureRedacted) {
            return Err(MacosTextInputError::SecureRedacted {
                request: "attributedSubstringForProposedRange:actualRange:",
            });
        }
        let range = self.resolve_document_range(
            "attributedSubstringForProposedRange.proposed_range",
            proposed_range,
        )?;
        let text = self
            .index
            .slice_byte_range(range)
            .map_err(|source| MacosTextInputError::TextIndex {
                role: "attributedSubstringForProposedRange.slice",
                source,
            })?
            .to_owned();
        Ok(MacosAttributedSubstring {
            text,
            actual_range: proposed_range,
        })
    }

    pub fn first_rect_for_character_range(
        &self,
        geometry: &TextInputGeometrySnapshot,
        character_range: MacosNativeRange,
        screen: MacosScreenCoordinateSpace,
    ) -> Result<MacosFirstRectQueryResult, MacosTextInputError> {
        if matches!(self.security, TextInputSecurityPolicy::SecureRedacted) {
            return Ok(MacosFirstRectQueryResult {
                rect: screen.rect_to_appkit(geometry.candidate_anchor_rect()),
                actual_range: MacosNativeRange::not_found(),
                availability: MacosFirstRectAvailability::SecureRedacted,
            });
        }
        let range = if character_range.is_not_found() {
            None
        } else {
            Some(self.resolve_document_range(
                "firstRectForCharacterRange.character_range",
                character_range,
            )?)
        };
        let first_bound = range.and_then(|range| {
            geometry
                .screen_character_bounds()
                .iter()
                .find(|bound| ranges_overlap(bound.range, range))
        });
        Ok(first_bound.map_or_else(
            || MacosFirstRectQueryResult {
                rect: screen.rect_to_appkit(geometry.candidate_anchor_rect()),
                actual_range: MacosNativeRange::not_found(),
                availability: MacosFirstRectAvailability::CaretFallback,
            },
            |bound| MacosFirstRectQueryResult {
                rect: screen.rect_to_appkit(bound.bounds),
                actual_range: self
                    .index
                    .utf16_range_from_byte(bound.range)
                    .map(MacosNativeRange::from_utf16_range)
                    .unwrap_or(MacosNativeRange::not_found()),
                availability: MacosFirstRectAvailability::ExactCharacterBounds,
            },
        ))
    }

    pub fn character_bounds_for_appkit(
        &self,
        geometry: &TextInputGeometrySnapshot,
    ) -> Result<&[TextCharacterBounds], MacosTextInputError> {
        if matches!(self.security, TextInputSecurityPolicy::SecureRedacted) {
            return Err(MacosTextInputError::SecureRedacted {
                request: "character bounds",
            });
        }
        Ok(geometry.screen_character_bounds())
    }

    fn end_composition(&mut self, reason: CompositionEndReason) -> MacosTextInputCallback {
        self.marked_text = None;
        self.marked_range = None;
        MacosTextInputCallback {
            event: PlatformTextInputEvent::EndComposition {
                context: self.next_context(),
                reason,
            },
        }
    }

    fn resolve_optional_document_range(
        &self,
        role: &'static str,
        range: MacosNativeRange,
    ) -> Result<Option<TextRange<TextByteOffset>>, MacosTextInputError> {
        if range.is_not_found() {
            Ok(None)
        } else {
            self.resolve_document_range(role, range).map(Some)
        }
    }

    fn resolve_document_range(
        &self,
        role: &'static str,
        range: MacosNativeRange,
    ) -> Result<TextRange<TextByteOffset>, MacosTextInputError> {
        Self::resolve_range_in(role, &self.index, range)
    }

    fn resolve_range_in(
        role: &'static str,
        index: &TextIndexSnapshot,
        range: MacosNativeRange,
    ) -> Result<TextRange<TextByteOffset>, MacosTextInputError> {
        let utf16 = range.checked_utf16_range(role)?;
        index
            .byte_range_from_utf16(utf16)
            .map_err(|source| MacosTextInputError::TextIndex { role, source })
    }

    fn next_context(&mut self) -> PlatformTextInputContext {
        self.serial = TextInputSerial(self.serial.0.saturating_add(1));
        PlatformTextInputContext::new(
            TextInputAdapterKind::MacosTextInputClient,
            self.session,
            self.generation,
            self.target.clone(),
            self.serial,
        )
    }
}

impl MacosTextInputActivationDiagnostics {
    pub const fn for_security(security: TextInputSecurityPolicy) -> Self {
        match security {
            TextInputSecurityPolicy::Plain => Self::plain(),
            TextInputSecurityPolicy::SecureRedacted => Self::secure_redacted(),
        }
    }

    pub const fn plain() -> Self {
        Self {
            surrounding_text: MacosCapabilityState::Supported,
            marked_text: MacosCapabilityState::Supported,
            selected_range: MacosCapabilityState::Supported,
            replacement_range: MacosCapabilityState::Supported,
            first_rect: MacosCapabilityState::Supported,
            character_bounds: MacosCapabilityState::Supported,
            reconversion: MacosCapabilityState::HostDependent,
            composition_segments: MacosCapabilityState::HostDependent,
            programmatic_commit: MacosCapabilityState::Limited,
            programmatic_cancel: MacosCapabilityState::Limited,
        }
    }

    pub const fn secure_redacted() -> Self {
        Self {
            surrounding_text: MacosCapabilityState::SecureRedacted,
            marked_text: MacosCapabilityState::SecureRedacted,
            selected_range: MacosCapabilityState::SecureRedacted,
            replacement_range: MacosCapabilityState::SecureRedacted,
            first_rect: MacosCapabilityState::Limited,
            character_bounds: MacosCapabilityState::SecureRedacted,
            reconversion: MacosCapabilityState::SecureRedacted,
            composition_segments: MacosCapabilityState::SecureRedacted,
            programmatic_commit: MacosCapabilityState::Limited,
            programmatic_cancel: MacosCapabilityState::Limited,
        }
    }
}

impl MacosTextInputCallback {
    pub const fn event(&self) -> &PlatformTextInputEvent {
        &self.event
    }

    pub fn into_event(self) -> PlatformTextInputEvent {
        self.event
    }
}

impl MacosScreenCoordinateSpace {
    pub const fn top_left_screen(screen_height_points: f64) -> Self {
        Self {
            screen_height_points,
        }
    }

    pub fn rect_to_appkit(self, rect: HitRect) -> MacosAppKitRect {
        MacosAppKitRect {
            x: f64::from(rect.x),
            y: self.screen_height_points - f64::from(rect.y + rect.height),
            width: f64::from(rect.width),
            height: f64::from(rect.height),
        }
    }
}

impl MacosFirstRectQueryResult {
    pub const fn rect(&self) -> MacosAppKitRect {
        self.rect
    }

    pub const fn actual_range(&self) -> MacosNativeRange {
        self.actual_range
    }

    pub const fn availability(&self) -> MacosFirstRectAvailability {
        self.availability
    }
}

impl MacosAttributedSubstring {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn actual_range(&self) -> MacosNativeRange {
        self.actual_range
    }
}

impl fmt::Display for MacosTextInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNativeRange {
                role,
                range,
                reason,
            } => write!(
                f,
                "invalid AppKit native range for {role}: location={}, length={}, reason={reason:?}",
                range.location, range.length
            ),
            Self::TextIndex { role, source } => {
                write!(f, "text-index conversion failed for {role}: {source}")
            }
            Self::SecureRedacted { request } => {
                write!(f, "secure text input redacts AppKit request {request}")
            }
        }
    }
}

impl std::error::Error for MacosTextInputError {}

fn ranges_overlap(a: TextRange<TextByteOffset>, b: TextRange<TextByteOffset>) -> bool {
    a.start().0 < b.end().0 && b.start().0 < a.end().0
}
