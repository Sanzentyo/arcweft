//! Shared Arcweft text-field editor behavior.
//!
//! Platform adapters normalize OS/browser callbacks into `TextInputOperation`.
//! This module owns the product editing state transitions for `TextField`,
//! `TextArea`, and `SecureField`: movement, ranged selection, deletion,
//! composition replacement/cancel, clipboard policy, and deterministic geometry
//! snapshots. Native object identity remains outside this module.

use crate::hit::HitRect;
use crate::input::InteractionTarget;
use crate::text_index::{TextIndexError, TextIndexSnapshot};
use crate::text_input::{
    CompositionEndReason, PlatformTextSelection, TextByteOffset, TextCharacterBounds, TextCommit,
    TextCompositionSegment, TextCompositionUpdate, TextDeleteUnit, TextEditCommand,
    TextGeometryTransform, TextInput, TextInputClientSnapshot, TextInputGeometrySnapshot,
    TextInputGeometrySnapshotParts, TextInputOptions, TextInputSecurityPolicy, TextInputSessionId,
    TextRange, TextRangeRect, TextRevision, TextSelectionAffinity, TextUtf16Offset,
    TextWritingMode,
};
use core::{cmp::Ordering, fmt};

/// Complete shared editor state for one focused Arcweft text control.
#[derive(Clone, Debug, PartialEq)]
pub struct TextEditorState {
    session: TextInputSessionId,
    target: InteractionTarget,
    text: String,
    selection: TextRange<TextByteOffset>,
    selection_anchor: TextByteOffset,
    caret: TextByteOffset,
    preferred_visual_x: Option<f32>,
    composition: Option<ActiveTextComposition>,
    revision: TextRevision,
    options: TextInputOptions,
}

/// Deterministic layout input used to build IME candidate/caret snapshots.
/// Real renderers should populate this from glyph layout. Tests and minimal
/// samples can use the monospaced defaults without platform-specific code.
#[derive(Clone, Debug, PartialEq)]
pub struct TextEditorLayout {
    source: TextEditorLayoutSource,
    text_local_control_rect: HitRect,
    text_origin_x: f32,
    text_origin_y: f32,
    grapheme_advance: f32,
    line_height: f32,
    caret_width: f32,
    writing_mode: TextWritingMode,
    text_local_to_viewport: TextGeometryTransform,
    viewport_to_screen: TextGeometryTransform,
    glyphs: Vec<TextEditorGlyphGeometry>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextEditorLayoutSource {
    Renderer,
    #[default]
    MonospacedFixture,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextEditorGlyphGeometry {
    range: TextRange<TextByteOffset>,
    bounds: HitRect,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextEditorLayoutParts {
    pub source: TextEditorLayoutSource,
    pub text_local_control_rect: HitRect,
    pub glyphs: Vec<TextEditorGlyphGeometry>,
    pub caret_width: f32,
    pub writing_mode: TextWritingMode,
    pub text_local_to_viewport: TextGeometryTransform,
    pub viewport_to_screen: TextGeometryTransform,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextEditorLayoutError {
    RendererLayoutWithoutGlyphs,
    InvalidGlyphRange(TextIndexError),
    OverlappingGlyphRange {
        previous: TextRange<TextByteOffset>,
        next: TextRange<TextByteOffset>,
    },
}

/// Paired snapshots produced after an edit, selection move, scroll, or layout
/// refresh.
#[derive(Clone, Debug, PartialEq)]
pub struct TextEditorSnapshots {
    client: TextInputClientSnapshot,
    geometry: TextInputGeometrySnapshot,
}

/// Small clipboard value object used by `Copy`, `Cut`, and `Paste` command
/// application. Secure editors reject clipboard commands before touching this
/// value.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TextEditorClipboard {
    text: Option<String>,
}

/// Result emitted by editor commands that have host-visible effects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextEditorOutput {
    None,
    ClipboardWrite(String),
    Submitted(String),
    CancelledComposition,
}

/// Shared editor transition error. Invalid native ranges are rejected and are
/// never silently clamped.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextEditorError {
    TextIndex(TextIndexError),
    SessionMismatch {
        expected: TextInputSessionId,
        incoming: TextInputSessionId,
    },
    SecureClipboardCommand(TextEditCommand),
    ClipboardEmpty,
    Layout(TextEditorLayoutError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActiveTextComposition {
    range: TextRange<TextByteOffset>,
    original_text: String,
    selection: TextRange<TextByteOffset>,
    segments: Vec<TextCompositionSegment>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TextEditorVerticalDirection {
    Up,
    Down,
}

const LOGICAL_PAGE_LINE_STEP: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq)]
struct TextEditorVisualLine {
    start: usize,
    end: usize,
    top: f32,
    bottom: f32,
}

impl TextEditorState {
    /// Creates a shared editor from Arcweft text-control state.
    pub fn new(
        session: TextInputSessionId,
        target: InteractionTarget,
        text: impl Into<String>,
        options: TextInputOptions,
    ) -> Result<Self, TextEditorError> {
        let text = text.into();
        let index = TextIndexSnapshot::try_new(text.clone())?;
        let end = index.len_bytes();
        Ok(Self {
            session,
            target,
            text,
            selection: TextRange::new(end, end),
            selection_anchor: end,
            caret: end,
            preferred_visual_x: None,
            composition: None,
            revision: TextRevision(0),
            options,
        })
    }

    /// Creates a shared editor from declarative runtime text-control state.
    ///
    /// The caller-provided selection is validated rather than clamped so the
    /// player, renderer, and platform adapter agree on the exact product-owned
    /// text value that becomes an IME target.
    pub fn from_text_control(
        session: TextInputSessionId,
        target: InteractionTarget,
        text: impl Into<String>,
        selection: TextRange<TextByteOffset>,
        options: TextInputOptions,
    ) -> Result<Self, TextEditorError> {
        let text = text.into();
        let index = TextIndexSnapshot::try_new(text.clone())?;
        let selection = index.validate_byte_range(selection)?;
        let caret = *selection.end();
        Ok(Self {
            session,
            target,
            text,
            selection,
            selection_anchor: *selection.start(),
            caret,
            preferred_visual_x: None,
            composition: None,
            revision: TextRevision::default(),
            options,
        })
    }

    /// Rehydrates the shared editor from an activation snapshot.
    pub fn from_snapshot(snapshot: &TextInputClientSnapshot) -> Result<Self, TextEditorError> {
        let index = TextIndexSnapshot::try_new(snapshot.surrounding_text().to_owned())?;
        let selection = index.validate_byte_range(snapshot.selection())?;
        let caret = *selection.end();
        Ok(Self {
            session: snapshot.session(),
            target: snapshot.target().clone(),
            text: snapshot.surrounding_text().to_owned(),
            selection,
            selection_anchor: *selection.start(),
            caret,
            preferred_visual_x: None,
            composition: snapshot
                .composition()
                .map(|composition| ActiveTextComposition {
                    range: composition
                        .replacement()
                        .unwrap_or_else(|| TextRange::new(*selection.start(), *selection.end())),
                    original_text: String::new(),
                    selection: composition.selection(),
                    segments: composition.segments().to_vec(),
                }),
            revision: snapshot.revision(),
            options: snapshot.options().clone(),
        })
    }

    pub const fn session(&self) -> TextInputSessionId {
        self.session
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn revision(&self) -> TextRevision {
        self.revision
    }

    pub const fn selection(&self) -> TextRange<TextByteOffset> {
        self.selection
    }

    pub const fn caret(&self) -> TextByteOffset {
        self.caret
    }

    pub const fn options(&self) -> &TextInputOptions {
        &self.options
    }

    pub fn composition_range(&self) -> Option<TextRange<TextByteOffset>> {
        self.composition
            .as_ref()
            .map(|composition| composition.range)
    }

    pub fn security(&self) -> TextInputSecurityPolicy {
        TextInputSecurityPolicy::from_options(&self.options)
    }

    /// Applies a platform-normalized batch. A single native callback should
    /// already have been converted to one Arcweft serial by the adapter/host
    /// boundary before this method is called.
    pub fn apply_text_input(
        &mut self,
        input: &TextInput,
        clipboard: &mut TextEditorClipboard,
    ) -> Result<Vec<TextEditorOutput>, TextEditorError> {
        self.apply_text_input_with_layout(input, clipboard, None)
    }

    /// Applies a platform-normalized batch with optional renderer glyph
    /// geometry for visual-line vertical navigation.
    pub fn apply_text_input_with_layout(
        &mut self,
        input: &TextInput,
        clipboard: &mut TextEditorClipboard,
        layout: Option<&TextEditorLayout>,
    ) -> Result<Vec<TextEditorOutput>, TextEditorError> {
        if input.session() != self.session {
            return Err(TextEditorError::SessionMismatch {
                expected: self.session,
                incoming: input.session(),
            });
        }
        input
            .operations()
            .iter()
            .map(|operation| self.apply_operation_with_layout(operation, clipboard, layout))
            .collect()
    }

    /// Applies one operation. Prefer `apply_text_input` for real platform
    /// batches so serial grouping remains visible at the caller.
    pub fn apply_operation(
        &mut self,
        operation: &crate::text_input::TextInputOperation,
        clipboard: &mut TextEditorClipboard,
    ) -> Result<TextEditorOutput, TextEditorError> {
        self.apply_operation_with_layout(operation, clipboard, None)
    }

    fn apply_operation_with_layout(
        &mut self,
        operation: &crate::text_input::TextInputOperation,
        clipboard: &mut TextEditorClipboard,
        layout: Option<&TextEditorLayout>,
    ) -> Result<TextEditorOutput, TextEditorError> {
        match operation {
            crate::text_input::TextInputOperation::StartComposition => self.start_composition(),
            crate::text_input::TextInputOperation::SetComposition(update) => {
                self.set_composition(update)
            }
            crate::text_input::TextInputOperation::Commit(commit) => self.commit_text(commit),
            crate::text_input::TextInputOperation::EndComposition { reason } => {
                self.end_composition(*reason)
            }
            crate::text_input::TextInputOperation::DeleteSurrounding {
                before,
                after,
                unit,
            } => self.delete_surrounding(*before, *after, *unit),
            crate::text_input::TextInputOperation::SetSelection(selection) => {
                self.set_platform_selection(*selection)
            }
            crate::text_input::TextInputOperation::Command(command) => {
                self.apply_command_with_layout(*command, clipboard, layout)
            }
        }
    }

    /// Applies a paste payload provided by the host clipboard path.
    pub fn paste_text(&mut self, text: &str) -> Result<TextEditorOutput, TextEditorError> {
        self.reject_secure_clipboard(TextEditCommand::Paste)?;
        self.replace_selection_or_composition(text)?;
        Ok(TextEditorOutput::None)
    }

    /// Moves pointer coordinates in text-local units to a canonical caret. This
    /// is used by mouse/touch hit-testing and drag selection after the renderer
    /// has supplied the active layout snapshot.
    pub fn set_caret_from_text_local_hit(
        &mut self,
        layout: &TextEditorLayout,
        x: f32,
        selecting: bool,
    ) -> Result<(), TextEditorError> {
        self.set_caret_from_text_local_point(layout, x, layout.text_origin_y, selecting)
    }

    /// Moves pointer coordinates in text-local units to the nearest renderer
    /// glyph boundary. Monospaced fixture layout is retained for deterministic
    /// tests and minimal samples.
    pub fn set_caret_from_text_local_point(
        &mut self,
        layout: &TextEditorLayout,
        x: f32,
        y: f32,
        selecting: bool,
    ) -> Result<(), TextEditorError> {
        if layout.is_renderer_backed() {
            let caret = layout.hit_test_text_local_point(x, y, &self.index()?)?;
            return self.move_caret_to(caret, selecting);
        }
        let index = self.index()?;
        let relative_x = (x - layout.text_origin_x).max(0.0);
        let boundary_count = index.grapheme_boundaries().len();
        let mut relative = 0_usize;
        let mut next_midpoint = layout.grapheme_advance * 0.5;
        while relative.saturating_add(1) < boundary_count && relative_x >= next_midpoint {
            relative = relative.saturating_add(1);
            next_midpoint += layout.grapheme_advance;
        }
        let caret = index.byte_offset_for_grapheme_slot(relative);
        self.move_caret_to(caret, selecting)
    }

    /// Moves the caret to a renderer/platform hit-test offset. The editor still
    /// owns policy enforcement, so selection-disabled controls collapse the
    /// range even when the caller requests extension.
    pub fn set_caret_to_text_offset(
        &mut self,
        caret: TextByteOffset,
        selecting: bool,
    ) -> Result<(), TextEditorError> {
        self.move_caret_to(caret, selecting)
    }

    /// Selects the shared word-like run at a renderer/platform hit-test offset.
    pub fn select_word_at_text_offset(
        &mut self,
        offset: TextByteOffset,
        selecting: bool,
    ) -> Result<(), TextEditorError> {
        let range = self.index()?.word_range_at(offset)?;
        self.select_text_range(range, selecting)
    }

    /// Selects the logical line containing a renderer/platform hit-test offset.
    pub fn select_line_at_text_offset(
        &mut self,
        offset: TextByteOffset,
        selecting: bool,
    ) -> Result<(), TextEditorError> {
        let offset = self.index()?.validate_byte_offset(offset)?;
        let range = TextRange::new(self.line_start_for(offset)?, self.line_end_for(offset)?);
        self.select_text_range(range, selecting)
    }

    /// Moves selected text to a renderer/platform hit-test offset.
    pub fn move_selection_to_text_offset(
        &mut self,
        offset: TextByteOffset,
    ) -> Result<bool, TextEditorError> {
        let index = self.index()?;
        let offset = index.validate_byte_offset(offset)?;
        if !self.options.selection_enabled() || self.selection_is_collapsed() {
            self.move_caret_to(offset, false)?;
            return Ok(false);
        }
        let selection = self.selection;
        if offset.0 >= selection.start().0 && offset.0 <= selection.end().0 {
            return Ok(false);
        }
        let selected = index.slice_byte_range(selection)?.to_owned();
        let selected_len = selection.end().0.saturating_sub(selection.start().0);
        self.replace_range(selection, "")?;
        let adjusted = if offset.0 > selection.end().0 {
            TextByteOffset(offset.0.saturating_sub(selected_len))
        } else {
            offset
        };
        let inserted = self.replace_range(TextRange::new(adjusted, adjusted), &selected)?;
        self.select_text_range(inserted, false)?;
        Ok(true)
    }

    /// Builds fresh client and geometry snapshots from the current state and
    /// renderer layout data.
    pub fn snapshots(
        &self,
        layout: &TextEditorLayout,
    ) -> Result<TextEditorSnapshots, TextEditorError> {
        let geometry = self.geometry_snapshot(layout)?;
        let mut client = TextInputClientSnapshot::new(
            self.session,
            self.target.clone(),
            self.revision,
            self.text.clone(),
            TextByteOffset(0),
            self.selection,
            geometry.viewport_control_rect(),
            geometry.viewport_caret_rect(),
            self.options.clone(),
        )
        .with_character_bounds(geometry.screen_character_bounds().to_vec());
        if let Some(composition) = self.composition_update()? {
            client = client.with_composition(composition);
        }
        Ok(TextEditorSnapshots { client, geometry })
    }

    pub fn geometry_snapshot(
        &self,
        layout: &TextEditorLayout,
    ) -> Result<TextInputGeometrySnapshot, TextEditorError> {
        let caret_rect = self.caret_rect(layout)?;
        Ok(TextInputGeometrySnapshot::new(
            TextInputGeometrySnapshotParts {
                session: self.session,
                revision: self.revision,
                writing_mode: layout.writing_mode,
                text_local_control_rect: layout.text_local_control_rect,
                text_local_caret_rect: caret_rect,
                text_local_character_bounds: self.character_bounds(layout),
                text_local_selection_rects: self.selection_rects(layout),
                text_local_composition_rects: self.composition_rects(layout),
                text_local_to_viewport: layout.text_local_to_viewport,
                viewport_to_screen: layout.viewport_to_screen,
            },
        ))
    }

    fn index(&self) -> Result<TextIndexSnapshot, TextEditorError> {
        Ok(TextIndexSnapshot::try_new(self.text.clone())?)
    }

    fn start_composition(&mut self) -> Result<TextEditorOutput, TextEditorError> {
        if self.composition.is_none() {
            let index = self.index()?;
            let original_text = index.slice_byte_range(self.selection)?.to_owned();
            self.composition = Some(ActiveTextComposition {
                range: self.selection,
                original_text,
                selection: TextRange::new(TextByteOffset(0), TextByteOffset(0)),
                segments: Vec::new(),
            });
        }
        Ok(TextEditorOutput::None)
    }

    fn set_composition(
        &mut self,
        update: &TextCompositionUpdate,
    ) -> Result<TextEditorOutput, TextEditorError> {
        let preedit_index = TextIndexSnapshot::try_new(update.preedit().to_owned())?;
        preedit_index.validate_byte_range(update.selection())?;
        for segment in update.segments() {
            preedit_index.validate_byte_range(segment.range())?;
        }

        let replacement = update
            .replacement()
            .or_else(|| {
                self.composition
                    .as_ref()
                    .map(|composition| composition.range)
            })
            .unwrap_or(self.selection);
        let index = self.index()?;
        let original_text = self
            .composition
            .as_ref()
            .filter(|composition| composition.range == replacement)
            .map_or_else(
                || index.slice_byte_range(replacement).map(str::to_owned),
                |composition| Ok(composition.original_text.clone()),
            )?;
        let inserted = self.replace_range(replacement, update.preedit())?;
        let selection = TextRange::new(
            add_offset(*inserted.start(), *update.selection().start()),
            add_offset(*inserted.start(), *update.selection().end()),
        );
        self.set_selection(selection)?;
        self.composition = Some(ActiveTextComposition {
            range: inserted,
            original_text,
            selection: update.selection(),
            segments: update.segments().to_vec(),
        });
        Ok(TextEditorOutput::None)
    }

    fn commit_text(&mut self, commit: &TextCommit) -> Result<TextEditorOutput, TextEditorError> {
        let replacement = commit
            .replacement()
            .or_else(|| {
                self.composition
                    .as_ref()
                    .map(|composition| composition.range)
            })
            .unwrap_or(self.selection);
        let inserted = self.replace_range(replacement, commit.text())?;
        self.composition = None;
        self.move_caret_to(*inserted.end(), false)?;
        Ok(TextEditorOutput::None)
    }

    fn end_composition(
        &mut self,
        reason: CompositionEndReason,
    ) -> Result<TextEditorOutput, TextEditorError> {
        let Some(composition) = self.composition.take() else {
            return Ok(TextEditorOutput::None);
        };
        if matches!(
            reason,
            CompositionEndReason::Cancelled
                | CompositionEndReason::FocusChanged
                | CompositionEndReason::SessionInvalidated
                | CompositionEndReason::PlatformDisabled
        ) {
            let inserted = self.replace_range(composition.range, &composition.original_text)?;
            self.move_caret_to(*inserted.end(), false)?;
            return Ok(TextEditorOutput::CancelledComposition);
        }
        Ok(TextEditorOutput::None)
    }

    fn delete_surrounding(
        &mut self,
        before: u32,
        after: u32,
        unit: TextDeleteUnit,
    ) -> Result<TextEditorOutput, TextEditorError> {
        if !self.selection_is_collapsed() {
            self.delete_selection()?;
            return Ok(TextEditorOutput::None);
        }
        let range = self.surrounding_range(before, after, unit)?;
        self.replace_range(range, "")?;
        self.move_caret_to(*range.start(), false)?;
        Ok(TextEditorOutput::None)
    }

    fn set_platform_selection(
        &mut self,
        selection: PlatformTextSelection,
    ) -> Result<TextEditorOutput, TextEditorError> {
        let _affinity = selection.affinity();
        self.set_selection(selection.range())?;
        Ok(TextEditorOutput::None)
    }

    fn apply_command_with_layout(
        &mut self,
        command: TextEditCommand,
        clipboard: &mut TextEditorClipboard,
        layout: Option<&TextEditorLayout>,
    ) -> Result<TextEditorOutput, TextEditorError> {
        if !self.options.shortcuts_enabled() && command_is_shortcut(command) {
            return Ok(TextEditorOutput::None);
        }
        match command {
            TextEditCommand::MoveLeft { selecting } => self.move_left(selecting),
            TextEditCommand::MoveRight { selecting } => self.move_right(selecting),
            TextEditCommand::MoveUp { selecting } => self.move_up_with_layout(selecting, layout),
            TextEditCommand::MoveDown { selecting } => {
                self.move_down_with_layout(selecting, layout)
            }
            TextEditCommand::MoveWordLeft { selecting } => self.move_word_left(selecting),
            TextEditCommand::MoveWordRight { selecting } => self.move_word_right(selecting),
            TextEditCommand::MoveLineStart { selecting } => self.move_line_start(selecting),
            TextEditCommand::MoveLineEnd { selecting } => self.move_line_end(selecting),
            TextEditCommand::MoveDocumentStart { selecting } => self.move_document_start(selecting),
            TextEditCommand::MoveDocumentEnd { selecting } => self.move_document_end(selecting),
            TextEditCommand::MovePageUp { selecting } => {
                self.move_page_up_with_layout(selecting, layout)
            }
            TextEditCommand::MovePageDown { selecting } => {
                self.move_page_down_with_layout(selecting, layout)
            }
            TextEditCommand::Backspace => self.backspace(),
            TextEditCommand::Delete => self.delete_forward(),
            TextEditCommand::DeleteWordLeft => self.delete_word_left(),
            TextEditCommand::DeleteWordRight => self.delete_word_right(),
            TextEditCommand::SelectWord => self
                .select_word_at_text_offset(self.caret, false)
                .map(|()| TextEditorOutput::None),
            TextEditCommand::SelectLine => self
                .select_line_at_text_offset(self.caret, false)
                .map(|()| TextEditorOutput::None),
            TextEditCommand::SelectAll => self.select_all(),
            TextEditCommand::Copy => self.copy_selection(clipboard),
            TextEditCommand::Cut => self.cut_selection(clipboard),
            TextEditCommand::Paste => self.paste_from_clipboard(clipboard),
            TextEditCommand::Submit => Ok(TextEditorOutput::Submitted(self.text.clone())),
            TextEditCommand::Cancel => self.end_composition(CompositionEndReason::Cancelled),
        }
    }

    fn move_left(&mut self, selecting: bool) -> Result<TextEditorOutput, TextEditorError> {
        let index = self.index()?;
        let caret = if !selecting && !self.selection_is_collapsed() {
            *self.selection.start()
        } else {
            index.previous_grapheme_boundary(self.caret)?
        };
        self.move_caret_to(caret, selecting)?;
        Ok(TextEditorOutput::None)
    }

    fn move_up_with_layout(
        &mut self,
        selecting: bool,
        layout: Option<&TextEditorLayout>,
    ) -> Result<TextEditorOutput, TextEditorError> {
        if let Some(layout) = layout
            && self.options.visual_line_vertical_navigation_enabled()
            && self.move_visual_vertical(layout, selecting, TextEditorVerticalDirection::Up)?
        {
            return Ok(TextEditorOutput::None);
        }
        self.move_up(selecting)
    }

    fn move_up(&mut self, selecting: bool) -> Result<TextEditorOutput, TextEditorError> {
        let current_start = self.line_start_for(self.caret)?;
        if current_start.0 == 0 {
            self.move_caret_to(TextByteOffset(0), selecting)?;
            return Ok(TextEditorOutput::None);
        }
        let previous_end = TextByteOffset(current_start.0.saturating_sub(1));
        let previous_start = self.line_start_for(previous_end)?;
        let column = self.caret.0.saturating_sub(current_start.0);
        let raw_target =
            TextByteOffset(previous_start.0.saturating_add(column).min(previous_end.0));
        let target = self.grapheme_boundary_at_or_before(raw_target)?;
        self.move_caret_to(target, selecting)?;
        Ok(TextEditorOutput::None)
    }

    fn move_down_with_layout(
        &mut self,
        selecting: bool,
        layout: Option<&TextEditorLayout>,
    ) -> Result<TextEditorOutput, TextEditorError> {
        if let Some(layout) = layout
            && self.options.visual_line_vertical_navigation_enabled()
            && self.move_visual_vertical(layout, selecting, TextEditorVerticalDirection::Down)?
        {
            return Ok(TextEditorOutput::None);
        }
        self.move_down(selecting)
    }

    fn move_down(&mut self, selecting: bool) -> Result<TextEditorOutput, TextEditorError> {
        let current_start = self.line_start_for(self.caret)?;
        let current_end = self.line_end_for(self.caret)?;
        let text_len = self.index()?.len_bytes();
        if current_end.0 >= text_len.0 {
            self.move_caret_to(text_len, selecting)?;
            return Ok(TextEditorOutput::None);
        }
        let next_start = TextByteOffset(current_end.0.saturating_add(1));
        let next_end = self.line_end_for(next_start)?;
        let column = self.caret.0.saturating_sub(current_start.0);
        let raw_target = TextByteOffset(next_start.0.saturating_add(column).min(next_end.0));
        let target = self.grapheme_boundary_at_or_before(raw_target)?;
        self.move_caret_to(target, selecting)?;
        Ok(TextEditorOutput::None)
    }

    fn move_visual_vertical(
        &mut self,
        layout: &TextEditorLayout,
        selecting: bool,
        direction: TextEditorVerticalDirection,
    ) -> Result<bool, TextEditorError> {
        if !layout.supports_visual_vertical_navigation() {
            return Ok(false);
        }
        let index = self.index()?;
        let preferred_x = self.preferred_visual_x.unwrap_or_else(|| {
            layout
                .caret_rect_for_offset(self.caret, &index)
                .map_or(layout.text_local_control_rect.x, |rect| rect.x)
        });
        let Some(target) =
            layout.visual_vertical_target(self.caret, preferred_x, direction, &index)?
        else {
            return Ok(false);
        };
        self.move_caret_to(target, selecting)?;
        self.preferred_visual_x = Some(preferred_x);
        Ok(true)
    }

    fn move_right(&mut self, selecting: bool) -> Result<TextEditorOutput, TextEditorError> {
        let index = self.index()?;
        let caret = if !selecting && !self.selection_is_collapsed() {
            *self.selection.end()
        } else {
            index.next_grapheme_boundary(self.caret)?
        };
        self.move_caret_to(caret, selecting)?;
        Ok(TextEditorOutput::None)
    }

    fn move_word_left(&mut self, selecting: bool) -> Result<TextEditorOutput, TextEditorError> {
        let caret = self.index()?.previous_word_boundary(self.caret)?;
        self.move_caret_to(caret, selecting)?;
        Ok(TextEditorOutput::None)
    }

    fn move_word_right(&mut self, selecting: bool) -> Result<TextEditorOutput, TextEditorError> {
        let caret = self.index()?.next_word_boundary(self.caret)?;
        self.move_caret_to(caret, selecting)?;
        Ok(TextEditorOutput::None)
    }

    fn move_line_start(&mut self, selecting: bool) -> Result<TextEditorOutput, TextEditorError> {
        let caret = self.line_start_for(self.caret)?;
        self.move_caret_to(caret, selecting)?;
        Ok(TextEditorOutput::None)
    }

    fn move_line_end(&mut self, selecting: bool) -> Result<TextEditorOutput, TextEditorError> {
        let caret = self.line_end_for(self.caret)?;
        self.move_caret_to(caret, selecting)?;
        Ok(TextEditorOutput::None)
    }

    fn move_document_start(
        &mut self,
        selecting: bool,
    ) -> Result<TextEditorOutput, TextEditorError> {
        self.move_caret_to(TextByteOffset(0), selecting)?;
        Ok(TextEditorOutput::None)
    }

    fn move_document_end(&mut self, selecting: bool) -> Result<TextEditorOutput, TextEditorError> {
        let caret = self.index()?.len_bytes();
        self.move_caret_to(caret, selecting)?;
        Ok(TextEditorOutput::None)
    }

    fn move_page_up_with_layout(
        &mut self,
        selecting: bool,
        layout: Option<&TextEditorLayout>,
    ) -> Result<TextEditorOutput, TextEditorError> {
        if let Some(layout) = layout
            && self.options.visual_line_vertical_navigation_enabled()
            && self.move_visual_page(layout, selecting, TextEditorVerticalDirection::Up)?
        {
            return Ok(TextEditorOutput::None);
        }
        self.move_logical_page(selecting, TextEditorVerticalDirection::Up)
    }

    fn move_page_down_with_layout(
        &mut self,
        selecting: bool,
        layout: Option<&TextEditorLayout>,
    ) -> Result<TextEditorOutput, TextEditorError> {
        if let Some(layout) = layout
            && self.options.visual_line_vertical_navigation_enabled()
            && self.move_visual_page(layout, selecting, TextEditorVerticalDirection::Down)?
        {
            return Ok(TextEditorOutput::None);
        }
        self.move_logical_page(selecting, TextEditorVerticalDirection::Down)
    }

    fn move_visual_page(
        &mut self,
        layout: &TextEditorLayout,
        selecting: bool,
        direction: TextEditorVerticalDirection,
    ) -> Result<bool, TextEditorError> {
        if !layout.supports_visual_vertical_navigation() {
            return Ok(false);
        }
        let index = self.index()?;
        let preferred_x = self.preferred_visual_x.unwrap_or_else(|| {
            layout
                .caret_rect_for_offset(self.caret, &index)
                .map_or(layout.text_local_control_rect.x, |rect| rect.x)
        });
        let Some(target) = layout.visual_page_target(self.caret, preferred_x, direction, &index)?
        else {
            return Ok(false);
        };
        self.move_caret_to(target, selecting)?;
        self.preferred_visual_x = Some(preferred_x);
        Ok(true)
    }

    fn move_logical_page(
        &mut self,
        selecting: bool,
        direction: TextEditorVerticalDirection,
    ) -> Result<TextEditorOutput, TextEditorError> {
        for _ in 0..LOGICAL_PAGE_LINE_STEP {
            match direction {
                TextEditorVerticalDirection::Up => self.move_up(selecting)?,
                TextEditorVerticalDirection::Down => self.move_down(selecting)?,
            };
        }
        Ok(TextEditorOutput::None)
    }

    fn backspace(&mut self) -> Result<TextEditorOutput, TextEditorError> {
        if !self.selection_is_collapsed() {
            self.delete_selection()?;
            return Ok(TextEditorOutput::None);
        }
        let index = self.index()?;
        let start = index.previous_grapheme_boundary(self.caret)?;
        let range = TextRange::new(start, self.caret);
        self.replace_range(range, "")?;
        self.move_caret_to(start, false)?;
        Ok(TextEditorOutput::None)
    }

    fn delete_forward(&mut self) -> Result<TextEditorOutput, TextEditorError> {
        if !self.selection_is_collapsed() {
            self.delete_selection()?;
            return Ok(TextEditorOutput::None);
        }
        let index = self.index()?;
        let end = index.next_grapheme_boundary(self.caret)?;
        let range = TextRange::new(self.caret, end);
        self.replace_range(range, "")?;
        self.move_caret_to(self.caret, false)?;
        Ok(TextEditorOutput::None)
    }

    fn delete_word_left(&mut self) -> Result<TextEditorOutput, TextEditorError> {
        if !self.selection_is_collapsed() {
            self.delete_selection()?;
            return Ok(TextEditorOutput::None);
        }
        let start = self.index()?.previous_word_boundary(self.caret)?;
        self.replace_range(TextRange::new(start, self.caret), "")?;
        self.move_caret_to(start, false)?;
        Ok(TextEditorOutput::None)
    }

    fn delete_word_right(&mut self) -> Result<TextEditorOutput, TextEditorError> {
        if !self.selection_is_collapsed() {
            self.delete_selection()?;
            return Ok(TextEditorOutput::None);
        }
        let end = self.index()?.next_word_boundary(self.caret)?;
        self.replace_range(TextRange::new(self.caret, end), "")?;
        self.move_caret_to(self.caret, false)?;
        Ok(TextEditorOutput::None)
    }

    fn select_all(&mut self) -> Result<TextEditorOutput, TextEditorError> {
        if !self.options.selection_enabled() {
            return Ok(TextEditorOutput::None);
        }
        let end = self.index()?.len_bytes();
        self.selection = TextRange::new(TextByteOffset(0), end);
        self.selection_anchor = TextByteOffset(0);
        self.caret = end;
        self.preferred_visual_x = None;
        self.revision = self.revision.next();
        Ok(TextEditorOutput::None)
    }

    fn copy_selection(
        &self,
        clipboard: &mut TextEditorClipboard,
    ) -> Result<TextEditorOutput, TextEditorError> {
        self.reject_secure_clipboard(TextEditCommand::Copy)?;
        if !self.options.selection_enabled() || self.selection_is_collapsed() {
            clipboard.clear();
            return Ok(TextEditorOutput::ClipboardWrite(String::new()));
        }
        let selected = self.index()?.slice_byte_range(self.selection)?.to_owned();
        clipboard.write(selected.clone());
        Ok(TextEditorOutput::ClipboardWrite(selected))
    }

    fn cut_selection(
        &mut self,
        clipboard: &mut TextEditorClipboard,
    ) -> Result<TextEditorOutput, TextEditorError> {
        self.reject_secure_clipboard(TextEditCommand::Cut)?;
        let output = self.copy_selection(clipboard)?;
        if !self.selection_is_collapsed() {
            self.delete_selection()?;
        }
        Ok(output)
    }

    fn paste_from_clipboard(
        &mut self,
        clipboard: &TextEditorClipboard,
    ) -> Result<TextEditorOutput, TextEditorError> {
        self.reject_secure_clipboard(TextEditCommand::Paste)?;
        let Some(text) = clipboard.read() else {
            return Err(TextEditorError::ClipboardEmpty);
        };
        self.replace_selection_or_composition(text)?;
        Ok(TextEditorOutput::None)
    }

    fn replace_selection_or_composition(&mut self, text: &str) -> Result<(), TextEditorError> {
        let replacement = self
            .composition
            .as_ref()
            .map_or(self.selection, |composition| composition.range);
        let inserted = self.replace_range(replacement, text)?;
        self.composition = None;
        self.move_caret_to(*inserted.end(), false)
    }

    fn delete_selection(&mut self) -> Result<(), TextEditorError> {
        let start = *self.selection.start();
        self.replace_range(self.selection, "")?;
        self.move_caret_to(start, false)
    }

    fn surrounding_range(
        &self,
        before: u32,
        after: u32,
        unit: TextDeleteUnit,
    ) -> Result<TextRange<TextByteOffset>, TextEditorError> {
        let index = self.index()?;
        match unit {
            TextDeleteUnit::Utf8Byte => {
                let text_len = u32::try_from(self.text.len()).unwrap_or(u32::MAX);
                let start = TextByteOffset(self.caret.0.saturating_sub(before));
                let end = TextByteOffset(self.caret.0.saturating_add(after).min(text_len));
                Ok(index.validate_byte_range(TextRange::new(start, end))?)
            }
            TextDeleteUnit::Utf16CodeUnit => {
                let caret_utf16 = index.utf16_offset_for_byte(self.caret)?;
                let start = TextUtf16Offset(caret_utf16.0.saturating_sub(before));
                let end = TextUtf16Offset(caret_utf16.0.saturating_add(after));
                Ok(index.byte_range_from_utf16(TextRange::new(start, end))?)
            }
            TextDeleteUnit::UnicodeScalar => {
                let mut start = self.caret;
                for _ in 0..before {
                    start = index.previous_scalar_boundary(start)?;
                }
                let mut end = self.caret;
                for _ in 0..after {
                    end = index.next_scalar_boundary(end)?;
                }
                Ok(TextRange::new(start, end))
            }
            TextDeleteUnit::GraphemeCluster => {
                let mut start = self.caret;
                for _ in 0..before {
                    start = index.previous_grapheme_boundary(start)?;
                }
                let mut end = self.caret;
                for _ in 0..after {
                    end = index.next_grapheme_boundary(end)?;
                }
                Ok(TextRange::new(start, end))
            }
        }
    }

    fn set_selection(
        &mut self,
        selection: TextRange<TextByteOffset>,
    ) -> Result<(), TextEditorError> {
        let selection = self.index()?.validate_byte_range(selection)?;
        let caret = *selection.end();
        if self.options.selection_enabled() {
            self.selection = selection;
            self.selection_anchor = *selection.start();
            self.caret = caret;
        } else {
            self.selection = TextRange::new(caret, caret);
            self.selection_anchor = caret;
            self.caret = caret;
        }
        self.preferred_visual_x = None;
        self.revision = self.revision.next();
        Ok(())
    }

    fn select_text_range(
        &mut self,
        range: TextRange<TextByteOffset>,
        selecting: bool,
    ) -> Result<(), TextEditorError> {
        let range = self.index()?.validate_byte_range(range)?;
        if !self.options.selection_enabled() {
            let caret = *range.end();
            self.selection = TextRange::new(caret, caret);
            self.selection_anchor = caret;
            self.caret = caret;
        } else if selecting {
            let anchor = self.selection_anchor;
            if anchor.0 <= range.start().0 {
                self.selection = TextRange::new(anchor, *range.end());
                self.caret = *range.end();
            } else {
                self.selection = TextRange::new(*range.start(), anchor);
                self.caret = *range.start();
            }
        } else {
            self.selection = range;
            self.selection_anchor = *range.start();
            self.caret = *range.end();
        }
        self.preferred_visual_x = None;
        self.revision = self.revision.next();
        Ok(())
    }

    fn move_caret_to(
        &mut self,
        caret: TextByteOffset,
        selecting: bool,
    ) -> Result<(), TextEditorError> {
        let caret = self.index()?.validate_byte_offset(caret)?;
        if selecting && self.options.selection_enabled() {
            let start = TextByteOffset(self.selection_anchor.0.min(caret.0));
            let end = TextByteOffset(self.selection_anchor.0.max(caret.0));
            self.selection = TextRange::new(start, end);
            self.caret = caret;
        } else {
            self.selection = TextRange::new(caret, caret);
            self.selection_anchor = caret;
            self.caret = caret;
        }
        self.preferred_visual_x = None;
        self.revision = self.revision.next();
        Ok(())
    }

    fn replace_range(
        &mut self,
        range: TextRange<TextByteOffset>,
        replacement: &str,
    ) -> Result<TextRange<TextByteOffset>, TextEditorError> {
        let index = self.index()?;
        let range = index.validate_byte_range(range)?;
        let start = range.start().0 as usize;
        let end = range.end().0 as usize;
        let mut text = String::with_capacity(
            self.text
                .len()
                .saturating_sub(end.saturating_sub(start))
                .saturating_add(replacement.len()),
        );
        text.push_str(&self.text[..start]);
        text.push_str(replacement);
        text.push_str(&self.text[end..]);
        let replacement_end = TextByteOffset(
            range
                .start()
                .0
                .saturating_add(u32::try_from(replacement.len()).unwrap_or(u32::MAX)),
        );
        TextIndexSnapshot::try_new(text.clone())?.validate_byte_offset(replacement_end)?;
        self.text = text;
        self.preferred_visual_x = None;
        self.revision = self.revision.next();
        Ok(TextRange::new(*range.start(), replacement_end))
    }

    fn line_start_for(&self, caret: TextByteOffset) -> Result<TextByteOffset, TextEditorError> {
        self.index()?.validate_byte_offset(caret)?;
        let start = self.text[..caret.0 as usize]
            .rfind('\n')
            .map_or(0, |byte| byte.saturating_add(1));
        Ok(TextByteOffset(u32::try_from(start).unwrap_or(0)))
    }

    fn line_end_for(&self, caret: TextByteOffset) -> Result<TextByteOffset, TextEditorError> {
        self.index()?.validate_byte_offset(caret)?;
        let tail_start = caret.0 as usize;
        let end = self.text[tail_start..]
            .find('\n')
            .map_or(self.text.len(), |byte| tail_start.saturating_add(byte));
        Ok(TextByteOffset(u32::try_from(end).unwrap_or(u32::MAX)))
    }

    fn grapheme_boundary_at_or_before(
        &self,
        offset: TextByteOffset,
    ) -> Result<TextByteOffset, TextEditorError> {
        let index = self.index()?;
        Ok(index
            .grapheme_boundaries()
            .into_iter()
            .take_while(|boundary| boundary.0 <= offset.0)
            .last()
            .unwrap_or(TextByteOffset(0)))
    }

    fn composition_update(&self) -> Result<Option<TextCompositionUpdate>, TextEditorError> {
        let Some(composition) = &self.composition else {
            return Ok(None);
        };
        let preedit = self
            .index()?
            .slice_byte_range(composition.range)?
            .to_owned();
        Ok(Some(
            TextCompositionUpdate::new(preedit, composition.selection)
                .with_replacement(composition.range)
                .with_segments(composition.segments.clone()),
        ))
    }

    fn character_bounds(&self, layout: &TextEditorLayout) -> Vec<TextCharacterBounds> {
        if layout.is_renderer_backed() {
            return layout
                .glyphs
                .iter()
                .map(|glyph| TextCharacterBounds::new(glyph.range, glyph.bounds))
                .collect();
        }
        let mut x = layout.text_origin_x;
        let mut bounds = Vec::new();
        for (byte, ch) in self.text.char_indices() {
            let start = TextByteOffset(u32::try_from(byte).unwrap_or(u32::MAX));
            let end = TextByteOffset(
                start
                    .0
                    .saturating_add(u32::try_from(ch.len_utf8()).unwrap_or(u32::MAX)),
            );
            bounds.push(TextCharacterBounds::new(
                TextRange::new(start, end),
                HitRect::new(
                    x,
                    layout.text_origin_y,
                    layout.grapheme_advance,
                    layout.line_height,
                ),
            ));
            x += layout.grapheme_advance;
        }
        bounds
    }

    fn caret_rect(&self, layout: &TextEditorLayout) -> Result<HitRect, TextEditorError> {
        if layout.is_renderer_backed() {
            return layout
                .caret_rect_for_offset(self.caret, &self.index()?)
                .map_err(Into::into);
        }
        let index = self.index()?;
        let slot = index
            .grapheme_boundaries()
            .into_iter()
            .position(|offset| offset == self.caret)
            .unwrap_or_else(|| index.grapheme_boundaries().len().saturating_sub(1));
        let x = (0..slot).fold(layout.text_origin_x, |x, _| x + layout.grapheme_advance);
        Ok(HitRect::new(
            x,
            layout.text_origin_y,
            layout.caret_width,
            layout.line_height,
        ))
    }

    fn selection_rects(&self, layout: &TextEditorLayout) -> Vec<TextRangeRect> {
        if !self.options.selection_enabled() || self.selection_is_collapsed() {
            return Vec::new();
        }
        layout.range_rects(self.selection)
    }

    fn composition_rects(&self, layout: &TextEditorLayout) -> Vec<TextRangeRect> {
        self.composition_range()
            .map_or_else(Vec::new, |range| layout.range_rects(range))
    }

    fn selection_is_collapsed(&self) -> bool {
        self.selection.start() == self.selection.end()
    }

    fn reject_secure_clipboard(&self, command: TextEditCommand) -> Result<(), TextEditorError> {
        if self.security().allows_clipboard() {
            Ok(())
        } else {
            Err(TextEditorError::SecureClipboardCommand(command))
        }
    }
}

impl TextEditorLayout {
    pub fn new(text_local_control_rect: HitRect) -> Self {
        Self::monospaced_fixture(text_local_control_rect)
    }

    pub fn monospaced_fixture(text_local_control_rect: HitRect) -> Self {
        Self {
            source: TextEditorLayoutSource::MonospacedFixture,
            text_local_control_rect,
            text_origin_x: text_local_control_rect.x,
            text_origin_y: text_local_control_rect.y,
            grapheme_advance: 10.0,
            line_height: text_local_control_rect.height.max(1.0),
            caret_width: 1.0,
            writing_mode: TextWritingMode::HorizontalTb,
            text_local_to_viewport: TextGeometryTransform::identity(),
            viewport_to_screen: TextGeometryTransform::identity(),
            glyphs: Vec::new(),
        }
    }

    pub fn from_renderer_parts_for_text(
        text: &str,
        parts: TextEditorLayoutParts,
    ) -> Result<Self, TextEditorLayoutError> {
        if parts.source == TextEditorLayoutSource::Renderer
            && parts.glyphs.is_empty()
            && !text.is_empty()
        {
            return Err(TextEditorLayoutError::RendererLayoutWithoutGlyphs);
        }
        let index = TextIndexSnapshot::try_new(text.to_owned())?;
        let mut previous: Option<TextRange<TextByteOffset>> = None;
        for glyph in &parts.glyphs {
            index.validate_byte_range(glyph.range)?;
            if let Some(previous_range) = previous
                && previous_range.end().0 > glyph.range.start().0
            {
                return Err(TextEditorLayoutError::OverlappingGlyphRange {
                    previous: previous_range,
                    next: glyph.range,
                });
            }
            previous = Some(glyph.range);
        }
        Ok(Self {
            source: parts.source,
            text_local_control_rect: parts.text_local_control_rect,
            text_origin_x: parts.text_local_control_rect.x,
            text_origin_y: parts.text_local_control_rect.y,
            grapheme_advance: 0.0,
            line_height: parts.text_local_control_rect.height.max(1.0),
            caret_width: parts.caret_width.max(1.0),
            writing_mode: parts.writing_mode,
            text_local_to_viewport: parts.text_local_to_viewport,
            viewport_to_screen: parts.viewport_to_screen,
            glyphs: parts.glyphs,
        })
    }

    pub fn from_geometry_snapshot_for_text(
        text: &str,
        geometry: &TextInputGeometrySnapshot,
    ) -> Result<Self, TextEditorLayoutError> {
        let caret_width = match geometry.writing_mode() {
            TextWritingMode::HorizontalTb => geometry.text_local_caret_rect().width,
            TextWritingMode::VerticalRl | TextWritingMode::VerticalLr => {
                geometry.text_local_caret_rect().height
            }
        };
        let glyphs = geometry
            .text_local_character_bounds()
            .iter()
            .map(|bounds| TextEditorGlyphGeometry::new(bounds.range, bounds.bounds))
            .collect();
        Self::from_renderer_parts_for_text(
            text,
            TextEditorLayoutParts {
                source: TextEditorLayoutSource::Renderer,
                text_local_control_rect: geometry.text_local_control_rect(),
                glyphs,
                caret_width,
                writing_mode: geometry.writing_mode(),
                text_local_to_viewport: TextGeometryTransform::identity(),
                viewport_to_screen: TextGeometryTransform::identity(),
            },
        )
    }

    pub const fn source(&self) -> TextEditorLayoutSource {
        self.source
    }

    pub const fn is_renderer_backed(&self) -> bool {
        matches!(self.source, TextEditorLayoutSource::Renderer)
    }

    pub fn glyphs(&self) -> &[TextEditorGlyphGeometry] {
        &self.glyphs
    }

    fn supports_visual_vertical_navigation(&self) -> bool {
        self.is_renderer_backed()
            && self.writing_mode == TextWritingMode::HorizontalTb
            && !self.glyphs.is_empty()
    }

    fn visual_vertical_target(
        &self,
        caret: TextByteOffset,
        preferred_x: f32,
        direction: TextEditorVerticalDirection,
        index: &TextIndexSnapshot,
    ) -> Result<Option<TextByteOffset>, TextEditorLayoutError> {
        let lines = self.horizontal_visual_lines();
        if lines.is_empty() {
            return Ok(None);
        }
        let caret_rect = self.caret_rect_for_offset(caret, index)?;
        let current = current_horizontal_visual_line(&lines, caret_rect);
        let target = match direction {
            TextEditorVerticalDirection::Up if current == 0 => TextByteOffset(0),
            TextEditorVerticalDirection::Up => {
                self.horizontal_line_offset_for_x(lines[current - 1], preferred_x, index)?
            }
            TextEditorVerticalDirection::Down if current + 1 >= lines.len() => index.len_bytes(),
            TextEditorVerticalDirection::Down => {
                self.horizontal_line_offset_for_x(lines[current + 1], preferred_x, index)?
            }
        };
        Ok(Some(target))
    }

    fn visual_page_target(
        &self,
        caret: TextByteOffset,
        preferred_x: f32,
        direction: TextEditorVerticalDirection,
        index: &TextIndexSnapshot,
    ) -> Result<Option<TextByteOffset>, TextEditorLayoutError> {
        let lines = self.horizontal_visual_lines();
        if lines.is_empty() {
            return Ok(None);
        }
        let caret_rect = self.caret_rect_for_offset(caret, index)?;
        let current = current_horizontal_visual_line(&lines, caret_rect);
        let line_height = lines
            .iter()
            .map(|line| line.bottom - line.top)
            .filter(|height| height.is_finite() && *height > 0.0)
            .fold(self.line_height.max(1.0), f32::max);
        let page_lines = visual_page_line_count(
            self.text_local_control_rect.height,
            line_height,
            lines.len(),
        );
        let step = page_lines.saturating_sub(1).max(1);
        let target_line = match direction {
            TextEditorVerticalDirection::Up => current.saturating_sub(step),
            TextEditorVerticalDirection::Down => current
                .saturating_add(step)
                .min(lines.len().saturating_sub(1)),
        };
        let target = if target_line == current {
            match direction {
                TextEditorVerticalDirection::Up => TextByteOffset(0),
                TextEditorVerticalDirection::Down => index.len_bytes(),
            }
        } else {
            self.horizontal_line_offset_for_x(lines[target_line], preferred_x, index)?
        };
        Ok(Some(target))
    }

    fn horizontal_visual_lines(&self) -> Vec<TextEditorVisualLine> {
        let Some(first) = self.glyphs.first() else {
            return Vec::new();
        };
        let mut lines = Vec::new();
        let mut start = 0;
        let mut top = first.bounds.y;
        let mut bottom = first.bounds.y + first.bounds.height;
        for (index, glyph) in self.glyphs.iter().enumerate().skip(1) {
            if horizontal_bounds_overlap(top, bottom, glyph.bounds) {
                top = top.min(glyph.bounds.y);
                bottom = bottom.max(glyph.bounds.y + glyph.bounds.height);
            } else {
                lines.push(TextEditorVisualLine {
                    start,
                    end: index,
                    top,
                    bottom,
                });
                start = index;
                top = glyph.bounds.y;
                bottom = glyph.bounds.y + glyph.bounds.height;
            }
        }
        lines.push(TextEditorVisualLine {
            start,
            end: self.glyphs.len(),
            top,
            bottom,
        });
        lines
    }

    fn horizontal_line_offset_for_x(
        &self,
        line: TextEditorVisualLine,
        preferred_x: f32,
        index: &TextIndexSnapshot,
    ) -> Result<TextByteOffset, TextEditorLayoutError> {
        let mut best = None::<(f32, TextByteOffset)>;
        for glyph in &self.glyphs[line.start..line.end] {
            let candidates = [
                (glyph.bounds.x, *glyph.range.start()),
                (glyph.bounds.x + glyph.bounds.width, *glyph.range.end()),
            ];
            for (x, offset) in candidates {
                let distance = (x - preferred_x).abs();
                best = match best {
                    Some((best_distance, best_offset)) => {
                        match distance.total_cmp(&best_distance) {
                            Ordering::Less => Some((distance, offset)),
                            Ordering::Equal if offset.0 < best_offset.0 => Some((distance, offset)),
                            _ => Some((best_distance, best_offset)),
                        }
                    }
                    _ => Some((distance, offset)),
                };
            }
        }
        let offset = best.map_or(TextByteOffset(0), |(_, offset)| offset);
        Ok(index.validate_byte_offset(offset)?)
    }

    fn caret_rect_for_offset(
        &self,
        offset: TextByteOffset,
        index: &TextIndexSnapshot,
    ) -> Result<HitRect, TextEditorLayoutError> {
        index.validate_byte_offset(offset)?;
        // At a visual line boundary, one source offset can sit after the
        // previous glyph and before the next glyph. Renderer-backed controls do
        // not carry a separate affinity value here, so use the visible glyph end.
        if let Some(glyph) = self.glyphs.windows(2).find_map(|pair| {
            let previous = pair[0];
            let next = pair[1];
            let previous_has_extent = match self.writing_mode {
                TextWritingMode::HorizontalTb => previous.bounds.width > 0.0,
                TextWritingMode::VerticalRl | TextWritingMode::VerticalLr => {
                    previous.bounds.height > 0.0
                }
            };
            let next_starts_visual_line = match self.writing_mode {
                TextWritingMode::HorizontalTb => next.bounds.y > previous.bounds.y,
                TextWritingMode::VerticalRl => next.bounds.x < previous.bounds.x,
                TextWritingMode::VerticalLr => next.bounds.x > previous.bounds.x,
            };
            (*previous.range.end() == offset
                && previous_has_extent
                && *next.range.start() == offset
                && next_starts_visual_line)
                .then_some(previous)
        }) {
            return Ok(self.caret_rect_at_glyph_end(glyph));
        }
        if let Some(glyph) = self
            .glyphs
            .iter()
            .find(|glyph| offset.0 <= glyph.range.start().0)
        {
            return Ok(self.caret_rect_at_glyph_start(*glyph));
        }
        Ok(self.glyphs.last().map_or_else(
            || {
                HitRect::new(
                    self.text_local_control_rect.x,
                    self.text_local_control_rect.y,
                    self.caret_width,
                    self.text_local_control_rect.height.max(1.0),
                )
            },
            |glyph| self.caret_rect_at_glyph_end(*glyph),
        ))
    }

    fn hit_test_text_local_point(
        &self,
        x: f32,
        y: f32,
        index: &TextIndexSnapshot,
    ) -> Result<TextByteOffset, TextEditorLayoutError> {
        if self.glyphs.is_empty() {
            return Ok(TextByteOffset(0));
        }
        let same_line = self
            .glyphs
            .iter()
            .copied()
            .filter(|glyph| y >= glyph.bounds.y && y <= glyph.bounds.y + glyph.bounds.height)
            .collect::<Vec<_>>();
        let candidates = if same_line.is_empty() {
            self.glyphs.as_slice()
        } else {
            same_line.as_slice()
        };
        for glyph in candidates {
            let mid = glyph.bounds.x + glyph.bounds.width * 0.5;
            if x <= mid {
                return Ok(index.validate_byte_offset(*glyph.range.start())?);
            }
        }
        let end = candidates
            .last()
            .map_or(TextByteOffset(0), |glyph| *glyph.range.end());
        index.validate_byte_offset(end).map_err(Into::into)
    }

    fn range_rects(&self, range: TextRange<TextByteOffset>) -> Vec<TextRangeRect> {
        self.glyphs
            .iter()
            .filter(|glyph| ranges_overlap(glyph.range, range))
            .map(|glyph| TextRangeRect::new(glyph.range, glyph.bounds))
            .collect()
    }

    fn caret_rect_at_glyph_start(&self, glyph: TextEditorGlyphGeometry) -> HitRect {
        match self.writing_mode {
            TextWritingMode::HorizontalTb => HitRect::new(
                glyph.bounds.x,
                glyph.bounds.y,
                self.caret_width,
                glyph.bounds.height.max(1.0),
            ),
            TextWritingMode::VerticalRl | TextWritingMode::VerticalLr => HitRect::new(
                glyph.bounds.x,
                glyph.bounds.y,
                glyph.bounds.width.max(1.0),
                self.caret_width,
            ),
        }
    }

    fn caret_rect_at_glyph_end(&self, glyph: TextEditorGlyphGeometry) -> HitRect {
        match self.writing_mode {
            TextWritingMode::HorizontalTb => HitRect::new(
                glyph.bounds.x + glyph.bounds.width,
                glyph.bounds.y,
                self.caret_width,
                glyph.bounds.height.max(1.0),
            ),
            TextWritingMode::VerticalRl | TextWritingMode::VerticalLr => HitRect::new(
                glyph.bounds.x,
                glyph.bounds.y + glyph.bounds.height,
                glyph.bounds.width.max(1.0),
                self.caret_width,
            ),
        }
    }

    #[must_use]
    pub const fn with_text_origin(mut self, x: f32, y: f32) -> Self {
        self.text_origin_x = x;
        self.text_origin_y = y;
        self
    }

    #[must_use]
    pub const fn with_grapheme_advance(mut self, grapheme_advance: f32) -> Self {
        self.grapheme_advance = grapheme_advance;
        self
    }

    #[must_use]
    pub const fn with_line_height(mut self, line_height: f32) -> Self {
        self.line_height = line_height;
        self
    }

    #[must_use]
    pub const fn with_caret_width(mut self, caret_width: f32) -> Self {
        self.caret_width = caret_width;
        self
    }

    #[must_use]
    pub const fn with_writing_mode(mut self, writing_mode: TextWritingMode) -> Self {
        self.writing_mode = writing_mode;
        self
    }

    #[must_use]
    pub const fn with_text_local_to_viewport(mut self, transform: TextGeometryTransform) -> Self {
        self.text_local_to_viewport = transform;
        self
    }

    #[must_use]
    pub const fn with_viewport_to_screen(mut self, transform: TextGeometryTransform) -> Self {
        self.viewport_to_screen = transform;
        self
    }
}

impl Default for TextEditorLayout {
    fn default() -> Self {
        Self::new(HitRect::new(0.0, 0.0, 320.0, 24.0))
    }
}

impl TextEditorGlyphGeometry {
    pub const fn new(range: TextRange<TextByteOffset>, bounds: HitRect) -> Self {
        Self { range, bounds }
    }

    pub const fn range(self) -> TextRange<TextByteOffset> {
        self.range
    }

    pub const fn bounds(self) -> HitRect {
        self.bounds
    }
}

impl TextEditorSnapshots {
    pub const fn client(&self) -> &TextInputClientSnapshot {
        &self.client
    }

    pub const fn geometry(&self) -> &TextInputGeometrySnapshot {
        &self.geometry
    }

    pub fn into_parts(self) -> (TextInputClientSnapshot, TextInputGeometrySnapshot) {
        (self.client, self.geometry)
    }
}

impl TextEditorClipboard {
    pub fn write(&mut self, text: impl Into<String>) {
        self.text = Some(text.into());
    }

    pub fn read(&self) -> Option<&str> {
        self.text.as_deref()
    }

    pub fn clear(&mut self) {
        self.text = None;
    }
}

impl From<TextIndexError> for TextEditorError {
    fn from(error: TextIndexError) -> Self {
        Self::TextIndex(error)
    }
}

impl From<TextEditorLayoutError> for TextEditorError {
    fn from(error: TextEditorLayoutError) -> Self {
        Self::Layout(error)
    }
}

impl From<TextIndexError> for TextEditorLayoutError {
    fn from(error: TextIndexError) -> Self {
        Self::InvalidGlyphRange(error)
    }
}

impl fmt::Display for TextEditorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TextIndex(error) => write!(f, "{error}"),
            Self::SessionMismatch { expected, incoming } => write!(
                f,
                "text editor session mismatch: expected {expected:?}, incoming {incoming:?}"
            ),
            Self::SecureClipboardCommand(command) => {
                write!(
                    f,
                    "secure text editor forbids clipboard command {command:?}"
                )
            }
            Self::ClipboardEmpty => write!(f, "paste requested but editor clipboard is empty"),
            Self::Layout(error) => write!(f, "text editor layout error: {error:?}"),
        }
    }
}

impl std::error::Error for TextEditorError {}

fn ranges_overlap(left: TextRange<TextByteOffset>, right: TextRange<TextByteOffset>) -> bool {
    left.start().0 < right.end().0 && right.start().0 < left.end().0
}

fn horizontal_bounds_overlap(line_top: f32, line_bottom: f32, bounds: HitRect) -> bool {
    bounds.y < line_bottom && line_top < bounds.y + bounds.height
}

fn current_horizontal_visual_line(lines: &[TextEditorVisualLine], caret_rect: HitRect) -> usize {
    let caret_y = caret_rect.y + caret_rect.height * 0.5;
    lines
        .iter()
        .position(|line| caret_y >= line.top && caret_y <= line.bottom)
        .unwrap_or_else(|| {
            lines
                .iter()
                .enumerate()
                .min_by(|(_, left), (_, right)| {
                    vertical_distance_to_line(caret_y, left)
                        .total_cmp(&vertical_distance_to_line(caret_y, right))
                })
                .map_or(0, |(index, _)| index)
        })
}

fn visual_page_line_count(control_height: f32, line_height: f32, line_count: usize) -> usize {
    if !control_height.is_finite() || !line_height.is_finite() || line_height <= 0.0 {
        return 1;
    }
    let mut covered = line_height;
    let mut count = 1_usize;
    while count < line_count && covered + line_height <= control_height {
        covered += line_height;
        count = count.saturating_add(1);
    }
    count
}

fn vertical_distance_to_line(y: f32, line: &TextEditorVisualLine) -> f32 {
    if y < line.top {
        line.top - y
    } else if y > line.bottom {
        y - line.bottom
    } else {
        0.0
    }
}

const fn command_is_shortcut(command: TextEditCommand) -> bool {
    matches!(
        command,
        TextEditCommand::SelectAll
            | TextEditCommand::Copy
            | TextEditCommand::Cut
            | TextEditCommand::Paste
    )
}

fn add_offset(start: TextByteOffset, relative: TextByteOffset) -> TextByteOffset {
    TextByteOffset(start.0.saturating_add(relative.0))
}

impl From<TextRange<TextByteOffset>> for PlatformTextSelection {
    fn from(range: TextRange<TextByteOffset>) -> Self {
        Self::new(range, TextSelectionAffinity::Downstream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::InteractionTarget;
    use arcweft_id::PublicId;

    fn target() -> InteractionTarget {
        InteractionTarget::new(PublicId::try_new("test.text".to_owned()).unwrap())
    }

    #[test]
    fn delete_surrounding_utf8_byte_unit_deletes_exact_byte_range() {
        let session = TextInputSessionId(1);
        let mut editor =
            TextEditorState::new(session, target(), "a東京b", TextInputOptions::default()).unwrap();
        let caret_after_second_kanji = TextByteOffset(7);
        let input = TextInput::new(
            session,
            crate::text_input::TextInputSerial(1),
            vec![
                crate::text_input::TextInputOperation::SetSelection(PlatformTextSelection::new(
                    TextRange::new(caret_after_second_kanji, caret_after_second_kanji),
                    TextSelectionAffinity::Downstream,
                )),
                crate::text_input::TextInputOperation::DeleteSurrounding {
                    before: 3,
                    after: 0,
                    unit: TextDeleteUnit::Utf8Byte,
                },
            ],
        );

        editor
            .apply_text_input(&input, &mut TextEditorClipboard::default())
            .unwrap();

        assert_eq!(editor.text(), "a東b");
        assert_eq!(
            editor.selection(),
            TextRange::new(TextByteOffset(4), TextByteOffset(4))
        );
    }

    #[test]
    fn delete_surrounding_utf8_byte_unit_rejects_non_boundary_range() {
        let session = TextInputSessionId(1);
        let mut editor =
            TextEditorState::new(session, target(), "a東京b", TextInputOptions::default()).unwrap();
        let caret_after_second_kanji = TextByteOffset(7);
        let input = TextInput::new(
            session,
            crate::text_input::TextInputSerial(1),
            vec![
                crate::text_input::TextInputOperation::SetSelection(PlatformTextSelection::new(
                    TextRange::new(caret_after_second_kanji, caret_after_second_kanji),
                    TextSelectionAffinity::Downstream,
                )),
                crate::text_input::TextInputOperation::DeleteSurrounding {
                    before: 1,
                    after: 0,
                    unit: TextDeleteUnit::Utf8Byte,
                },
            ],
        );

        let error = editor
            .apply_text_input(&input, &mut TextEditorClipboard::default())
            .unwrap_err();

        assert!(matches!(error, TextEditorError::TextIndex(_)));
        assert_eq!(editor.text(), "a東京b");
    }
}
