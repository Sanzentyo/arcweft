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
    TextRange, TextRevision, TextSelectionAffinity, TextUtf16Offset, TextWritingMode,
};
use core::fmt;

/// Complete shared editor state for one focused Arcweft text control.
#[derive(Clone, Debug, PartialEq)]
pub struct TextEditorState {
    session: TextInputSessionId,
    target: InteractionTarget,
    text: String,
    selection: TextRange<TextByteOffset>,
    selection_anchor: TextByteOffset,
    caret: TextByteOffset,
    composition: Option<ActiveTextComposition>,
    revision: TextRevision,
    options: TextInputOptions,
}

/// Deterministic layout input used to build IME candidate/caret snapshots.
/// Real renderers should populate this from glyph layout. Tests and minimal
/// samples can use the monospaced defaults without platform-specific code.
#[derive(Clone, Debug, PartialEq)]
pub struct TextEditorLayout {
    text_local_control_rect: HitRect,
    text_origin_x: f32,
    text_origin_y: f32,
    grapheme_advance: f32,
    line_height: f32,
    caret_width: f32,
    writing_mode: TextWritingMode,
    text_local_to_viewport: TextGeometryTransform,
    viewport_to_screen: TextGeometryTransform,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActiveTextComposition {
    range: TextRange<TextByteOffset>,
    original_text: String,
    selection: TextRange<TextByteOffset>,
    segments: Vec<TextCompositionSegment>,
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
            composition: None,
            revision: TextRevision(0),
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
        if input.session() != self.session {
            return Err(TextEditorError::SessionMismatch {
                expected: self.session,
                incoming: input.session(),
            });
        }
        input
            .operations()
            .iter()
            .map(|operation| self.apply_operation(operation, clipboard))
            .collect()
    }

    /// Applies one operation. Prefer `apply_text_input` for real platform
    /// batches so serial grouping remains visible at the caller.
    pub fn apply_operation(
        &mut self,
        operation: &crate::text_input::TextInputOperation,
        clipboard: &mut TextEditorClipboard,
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
                self.apply_command(*command, clipboard)
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

    fn apply_command(
        &mut self,
        command: TextEditCommand,
        clipboard: &mut TextEditorClipboard,
    ) -> Result<TextEditorOutput, TextEditorError> {
        match command {
            TextEditCommand::MoveLeft { selecting } => self.move_left(selecting),
            TextEditCommand::MoveRight { selecting } => self.move_right(selecting),
            TextEditCommand::MoveWordLeft { selecting } => self.move_word_left(selecting),
            TextEditCommand::MoveWordRight { selecting } => self.move_word_right(selecting),
            TextEditCommand::MoveLineStart { selecting } => self.move_line_start(selecting),
            TextEditCommand::MoveLineEnd { selecting } => self.move_line_end(selecting),
            TextEditCommand::Backspace => self.backspace(),
            TextEditCommand::Delete => self.delete_forward(),
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

    fn select_all(&mut self) -> Result<TextEditorOutput, TextEditorError> {
        let end = self.index()?.len_bytes();
        self.selection = TextRange::new(TextByteOffset(0), end);
        self.selection_anchor = TextByteOffset(0);
        self.caret = end;
        self.revision = self.revision.next();
        Ok(TextEditorOutput::None)
    }

    fn copy_selection(
        &self,
        clipboard: &mut TextEditorClipboard,
    ) -> Result<TextEditorOutput, TextEditorError> {
        self.reject_secure_clipboard(TextEditCommand::Copy)?;
        if self.selection_is_collapsed() {
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
        self.selection = selection;
        self.selection_anchor = *selection.start();
        self.caret = *selection.end();
        self.revision = self.revision.next();
        Ok(())
    }

    fn move_caret_to(
        &mut self,
        caret: TextByteOffset,
        selecting: bool,
    ) -> Result<(), TextEditorError> {
        let caret = self.index()?.validate_byte_offset(caret)?;
        if selecting {
            let start = TextByteOffset(self.selection_anchor.0.min(caret.0));
            let end = TextByteOffset(self.selection_anchor.0.max(caret.0));
            self.selection = TextRange::new(start, end);
            self.caret = caret;
        } else {
            self.selection = TextRange::new(caret, caret);
            self.selection_anchor = caret;
            self.caret = caret;
        }
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
        Self {
            text_local_control_rect,
            text_origin_x: text_local_control_rect.x,
            text_origin_y: text_local_control_rect.y,
            grapheme_advance: 10.0,
            line_height: text_local_control_rect.height.max(1.0),
            caret_width: 1.0,
            writing_mode: TextWritingMode::HorizontalTb,
            text_local_to_viewport: TextGeometryTransform::identity(),
            viewport_to_screen: TextGeometryTransform::identity(),
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
        }
    }
}

impl std::error::Error for TextEditorError {}

fn add_offset(start: TextByteOffset, relative: TextByteOffset) -> TextByteOffset {
    TextByteOffset(start.0.saturating_add(relative.0))
}

impl From<TextRange<TextByteOffset>> for PlatformTextSelection {
    fn from(range: TextRange<TextByteOffset>) -> Self {
        Self::new(range, TextSelectionAffinity::Downstream)
    }
}
