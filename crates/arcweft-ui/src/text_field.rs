//! Retained TextField/TextArea state and style parts.

use crate::text_source::{UiTextByteRange, UiTextSource};
use crate::{HandlerId, TextSourceId};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::text_input::{
    CompositionEndReason, TextByteOffset, TextCommit, TextCompositionUpdate, TextDeleteUnit,
    TextEditCommand, TextInput, TextInputOperation, TextInputOptions, TextInputSessionId,
    TextRange, TextRevision,
};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextEditorMode {
    #[default]
    SingleLine,
    MultiLine,
    Secure,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TextFieldId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TextFieldPartId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TextEditorPart {
    Root,
    Content,
    Placeholder,
    Selection,
    Caret,
    Composition,
    CompositionTarget,
    Leading,
    Trailing,
    ClearButton,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextFieldSpec {
    id: TextFieldId,
    value: TextSourceId,
    placeholder: Option<TextSourceId>,
    mode: TextEditorMode,
    options: TextInputOptions,
    submit_handler: Option<HandlerId>,
    change_handler: Option<HandlerId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextEditState {
    document: String,
    selection: UiTextByteRange,
    composition: Option<TextCompositionUpdate>,
    revision: TextRevision,
    session: Option<TextInputSessionId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExternalTextUpdatePolicy {
    DeferUntilCompositionEnd,
    RebaseWhenNonOverlapping,
    ReplaceAndCancelComposition,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TextEditError {
    #[error("stale text-input session: active {active:?}, incoming {incoming:?}")]
    StaleTextInputSession {
        active: Option<TextInputSessionId>,
        incoming: TextInputSessionId,
    },
    #[error("invalid text byte range {range:?} for document length {document_len}")]
    InvalidByteRange {
        range: UiTextByteRange,
        document_len: usize,
    },
    #[error("text byte range {range:?} does not align with UTF-8 boundaries")]
    NonBoundaryRange { range: UiTextByteRange },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextEditOutcome {
    changed: bool,
    submitted: bool,
    revision: TextRevision,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextFieldMetrics {
    pub advance_px: f32,
    pub line_height_px: f32,
    pub caret_width_px: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextFieldPartRect {
    part: TextEditorPart,
    bounds: HitRect,
    range: Option<UiTextByteRange>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextFieldVisualBuffer {
    target: Option<InteractionTarget>,
    bounds: HitRect,
    display_text: String,
    source: UiTextSource,
    parts: Vec<TextFieldPartRect>,
    revision: TextRevision,
    secure: bool,
}

impl TextFieldSpec {
    pub fn new(id: TextFieldId, value: TextSourceId) -> Self {
        Self {
            id,
            value,
            placeholder: None,
            mode: TextEditorMode::SingleLine,
            options: TextInputOptions::default(),
            submit_handler: None,
            change_handler: None,
        }
    }

    #[must_use]
    pub const fn with_placeholder(mut self, placeholder: TextSourceId) -> Self {
        self.placeholder = Some(placeholder);
        self
    }

    #[must_use]
    pub const fn with_mode(mut self, mode: TextEditorMode) -> Self {
        self.mode = mode;
        self
    }

    #[must_use]
    pub const fn with_options(mut self, options: TextInputOptions) -> Self {
        self.options = options;
        self
    }

    #[must_use]
    pub const fn on_submit(mut self, handler: HandlerId) -> Self {
        self.submit_handler = Some(handler);
        self
    }

    #[must_use]
    pub const fn on_change(mut self, handler: HandlerId) -> Self {
        self.change_handler = Some(handler);
        self
    }

    pub const fn id(&self) -> TextFieldId {
        self.id
    }

    pub const fn value(&self) -> TextSourceId {
        self.value
    }

    pub const fn placeholder(&self) -> Option<TextSourceId> {
        self.placeholder
    }

    pub const fn mode(&self) -> TextEditorMode {
        self.mode
    }

    pub const fn options(&self) -> &TextInputOptions {
        &self.options
    }

    pub fn resolved_options(&self) -> TextInputOptions {
        match self.mode {
            TextEditorMode::SingleLine => self.options.clone(),
            TextEditorMode::MultiLine => self.options.clone().multiline(true),
            TextEditorMode::Secure => self.options.clone().secure(true),
        }
    }
}

impl TextEditOutcome {
    pub const fn new(changed: bool, submitted: bool, revision: TextRevision) -> Self {
        Self {
            changed,
            submitted,
            revision,
        }
    }

    pub const fn changed(self) -> bool {
        self.changed
    }

    pub const fn submitted(self) -> bool {
        self.submitted
    }

    pub const fn revision(self) -> TextRevision {
        self.revision
    }
}

impl Default for TextFieldMetrics {
    fn default() -> Self {
        Self {
            advance_px: 8.0,
            line_height_px: 16.0,
            caret_width_px: 1.0,
        }
    }
}

impl TextEditState {
    pub fn new(document: impl Into<String>) -> Self {
        Self {
            document: document.into(),
            selection: UiTextByteRange::new(0, 0),
            composition: None,
            revision: TextRevision::default(),
            session: None,
        }
    }

    pub fn document(&self) -> &str {
        &self.document
    }

    pub const fn selection(&self) -> UiTextByteRange {
        self.selection
    }

    pub const fn composition(&self) -> Option<&TextCompositionUpdate> {
        self.composition.as_ref()
    }

    pub const fn revision(&self) -> TextRevision {
        self.revision
    }

    pub const fn session(&self) -> Option<TextInputSessionId> {
        self.session
    }

    pub fn visual_source(&self) -> UiTextSource {
        if let Some(composition) = &self.composition {
            let mut visual = self.document.clone();
            let range = composition.replacement().map_or(self.selection, |range| {
                UiTextByteRange::new(range.start().0, range.end().0)
            });
            let start = usize::try_from(range.start())
                .unwrap_or(visual.len())
                .min(visual.len());
            let end = usize::try_from(range.end())
                .unwrap_or(start)
                .min(visual.len());
            if start <= end {
                visual.replace_range(start..end, composition.preedit());
            }
            UiTextSource::plain(visual)
        } else {
            UiTextSource::plain(self.document.clone())
        }
    }

    pub fn set_composition(&mut self, composition: TextCompositionUpdate) {
        self.composition = Some(composition);
    }

    pub fn clear_composition(&mut self) {
        self.composition = None;
    }

    pub fn bind_session(&mut self, session: TextInputSessionId) {
        self.session = Some(session);
    }

    pub fn apply_text_input(
        &mut self,
        input: &TextInput,
    ) -> Result<TextEditOutcome, TextEditError> {
        if self.session != Some(input.session()) {
            return Err(TextEditError::StaleTextInputSession {
                active: self.session,
                incoming: input.session(),
            });
        }

        let mut next = self.clone();
        let mut changed = false;
        let mut submitted = false;
        for operation in input.operations() {
            match operation {
                TextInputOperation::StartComposition => next.composition = None,
                TextInputOperation::SetComposition(composition) => {
                    next.composition = Some(composition.clone());
                }
                TextInputOperation::Commit(commit) => {
                    changed |= next.commit_text(commit)?;
                }
                TextInputOperation::EndComposition { reason } => {
                    changed |= next.end_composition(*reason)?;
                }
                TextInputOperation::DeleteSurrounding {
                    before,
                    after,
                    unit,
                } => {
                    changed |= next.delete_surrounding(*before, *after, *unit)?;
                }
                TextInputOperation::SetSelection(selection) => {
                    next.selection = ui_range(selection.range());
                }
                TextInputOperation::Command(command) => {
                    let command_result = next.apply_command(*command)?;
                    changed |= command_result.changed;
                    submitted |= command_result.submitted;
                }
            }
        }
        if changed {
            next.revision = next.revision.next();
        }
        let outcome = TextEditOutcome::new(changed, submitted, next.revision);
        *self = next;
        Ok(outcome)
    }

    pub fn visual_buffer(
        &self,
        target: Option<InteractionTarget>,
        bounds: HitRect,
        metrics: TextFieldMetrics,
        secure: bool,
    ) -> TextFieldVisualBuffer {
        let source = self.visual_source();
        let display_text = match &source {
            UiTextSource::Plain(text) if secure => mask_text(text),
            UiTextSource::Plain(text) => text.clone(),
            UiTextSource::Localized(_)
            | UiTextSource::RichTextDocument(_)
            | UiTextSource::DisplayFrame(_) => String::new(),
        };
        let mut parts = vec![
            TextFieldPartRect::new(TextEditorPart::Root, bounds, None),
            TextFieldPartRect::new(TextEditorPart::Content, bounds, None),
        ];
        if self.selection.start() != self.selection.end() {
            parts.push(TextFieldPartRect::new(
                TextEditorPart::Selection,
                range_rect(bounds, self.selection, metrics),
                Some(self.selection),
            ));
        }
        if let Some(composition) = &self.composition {
            let base = composition.replacement().map_or(self.selection, ui_range);
            let composition_range = UiTextByteRange::new(
                base.start(),
                base.start()
                    .saturating_add(u32::try_from(composition.preedit().len()).unwrap_or(u32::MAX)),
            );
            parts.push(TextFieldPartRect::new(
                TextEditorPart::Composition,
                underline_rect(bounds, composition_range, metrics),
                Some(composition_range),
            ));
        }
        let caret = UiTextByteRange::new(self.selection.end(), self.selection.end());
        parts.push(TextFieldPartRect::new(
            TextEditorPart::Caret,
            caret_rect(bounds, self.selection.end(), metrics),
            Some(caret),
        ));
        TextFieldVisualBuffer {
            target,
            bounds,
            display_text,
            source,
            parts,
            revision: self.revision,
            secure,
        }
    }

    fn commit_text(&mut self, commit: &TextCommit) -> Result<bool, TextEditError> {
        let range = commit.replacement().map_or_else(
            || {
                self.composition
                    .as_ref()
                    .and_then(TextCompositionUpdate::replacement)
                    .map_or(self.selection, ui_range)
            },
            ui_range,
        );
        replace_range(&mut self.document, range, commit.text())?;
        let caret = range
            .start()
            .saturating_add(u32::try_from(commit.text().len()).unwrap_or(u32::MAX));
        self.selection = UiTextByteRange::new(caret, caret);
        self.composition = None;
        Ok(true)
    }

    fn end_composition(&mut self, reason: CompositionEndReason) -> Result<bool, TextEditError> {
        let Some(composition) = self.composition.take() else {
            return Ok(false);
        };
        if reason == CompositionEndReason::Committed {
            let range = composition.replacement().map_or(self.selection, ui_range);
            replace_range(&mut self.document, range, composition.preedit())?;
            let caret = range
                .start()
                .saturating_add(u32::try_from(composition.preedit().len()).unwrap_or(u32::MAX));
            self.selection = UiTextByteRange::new(caret, caret);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn delete_surrounding(
        &mut self,
        before: u32,
        after: u32,
        _unit: TextDeleteUnit,
    ) -> Result<bool, TextEditError> {
        let start = self.selection.start().saturating_sub(before);
        let end = self
            .selection
            .end()
            .saturating_add(after)
            .min(u32::try_from(self.document.len()).unwrap_or(u32::MAX));
        let range = UiTextByteRange::new(start, end);
        replace_range(&mut self.document, range, "")?;
        self.selection = UiTextByteRange::new(start, start);
        Ok(start != end)
    }

    fn apply_command(
        &mut self,
        command: TextEditCommand,
    ) -> Result<TextEditOutcome, TextEditError> {
        match command {
            TextEditCommand::Backspace => Ok(TextEditOutcome::new(
                self.delete_surrounding(1, 0, TextDeleteUnit::Utf16CodeUnit)?,
                false,
                self.revision,
            )),
            TextEditCommand::Delete => Ok(TextEditOutcome::new(
                self.delete_surrounding(0, 1, TextDeleteUnit::Utf16CodeUnit)?,
                false,
                self.revision,
            )),
            TextEditCommand::SelectAll => {
                self.selection =
                    UiTextByteRange::new(0, u32::try_from(self.document.len()).unwrap_or(u32::MAX));
                Ok(TextEditOutcome::new(false, false, self.revision))
            }
            TextEditCommand::Submit => Ok(TextEditOutcome::new(false, true, self.revision)),
            TextEditCommand::Cancel => {
                self.composition = None;
                Ok(TextEditOutcome::new(false, false, self.revision))
            }
            TextEditCommand::MoveLeft { selecting: _ }
            | TextEditCommand::MoveWordLeft { selecting: _ }
            | TextEditCommand::MoveLineStart { selecting: _ } => {
                self.selection = UiTextByteRange::new(0, 0);
                Ok(TextEditOutcome::new(false, false, self.revision))
            }
            TextEditCommand::MoveRight { selecting: _ }
            | TextEditCommand::MoveWordRight { selecting: _ }
            | TextEditCommand::MoveLineEnd { selecting: _ } => {
                let end = u32::try_from(self.document.len()).unwrap_or(u32::MAX);
                self.selection = UiTextByteRange::new(end, end);
                Ok(TextEditOutcome::new(false, false, self.revision))
            }
            TextEditCommand::Copy | TextEditCommand::Cut | TextEditCommand::Paste => {
                Ok(TextEditOutcome::new(false, false, self.revision))
            }
        }
    }
}

impl TextFieldPartRect {
    pub const fn new(
        part: TextEditorPart,
        bounds: HitRect,
        range: Option<UiTextByteRange>,
    ) -> Self {
        Self {
            part,
            bounds,
            range,
        }
    }

    pub const fn part(&self) -> TextEditorPart {
        self.part
    }

    pub const fn bounds(&self) -> HitRect {
        self.bounds
    }

    pub const fn range(&self) -> Option<UiTextByteRange> {
        self.range
    }
}

impl TextFieldVisualBuffer {
    pub fn target(&self) -> Option<&InteractionTarget> {
        self.target.as_ref()
    }

    pub const fn bounds(&self) -> HitRect {
        self.bounds
    }

    pub fn display_text(&self) -> &str {
        &self.display_text
    }

    pub const fn source(&self) -> &UiTextSource {
        &self.source
    }

    pub fn parts(&self) -> &[TextFieldPartRect] {
        &self.parts
    }

    pub const fn revision(&self) -> TextRevision {
        self.revision
    }

    pub const fn is_secure(&self) -> bool {
        self.secure
    }
}

fn ui_range(range: TextRange<TextByteOffset>) -> UiTextByteRange {
    UiTextByteRange::new(range.start().0, range.end().0)
}

fn replace_range(
    document: &mut String,
    range: UiTextByteRange,
    value: &str,
) -> Result<(), TextEditError> {
    let start = usize::try_from(range.start()).unwrap_or(usize::MAX);
    let end = usize::try_from(range.end()).unwrap_or(usize::MAX);
    let len = document.len();
    if start > end || end > len {
        return Err(TextEditError::InvalidByteRange {
            range,
            document_len: len,
        });
    }
    if !document.is_char_boundary(start) || !document.is_char_boundary(end) {
        return Err(TextEditError::NonBoundaryRange { range });
    }
    document.replace_range(start..end, value);
    Ok(())
}

fn mask_text(value: &str) -> String {
    "•".repeat(value.chars().count())
}

fn range_rect(bounds: HitRect, range: UiTextByteRange, metrics: TextFieldMetrics) -> HitRect {
    let start = range.start().min(range.end());
    let end = range.end().max(range.start());
    let x = bounds.x + byte_offset_px(start, metrics.advance_px);
    let width =
        byte_offset_px(end.saturating_sub(start), metrics.advance_px).max(metrics.caret_width_px);
    HitRect::new(
        x,
        bounds.y,
        width,
        metrics.line_height_px.min(bounds.height),
    )
}

fn underline_rect(bounds: HitRect, range: UiTextByteRange, metrics: TextFieldMetrics) -> HitRect {
    let mut rect = range_rect(bounds, range, metrics);
    rect.y = bounds.y + bounds.height - 2.0;
    rect.height = 2.0;
    rect
}

fn caret_rect(bounds: HitRect, offset: u32, metrics: TextFieldMetrics) -> HitRect {
    HitRect::new(
        bounds.x + byte_offset_px(offset, metrics.advance_px),
        bounds.y,
        metrics.caret_width_px,
        metrics.line_height_px.min(bounds.height),
    )
}

fn byte_offset_px(offset: u32, advance_px: f32) -> f32 {
    f32::from(u16::try_from(offset).unwrap_or(u16::MAX)) * advance_px
}

#[cfg(test)]
mod tests {
    use super::{TextEditError, TextEditState, TextEditorPart, TextFieldMetrics};
    use arcweft_presentation::hit::HitRect;
    use arcweft_presentation::text_input::{
        TextByteOffset, TextCommit, TextCompositionUpdate, TextInput, TextInputOperation,
        TextInputSerial, TextInputSessionId, TextRange,
    };

    #[test]
    fn composition_visual_source_does_not_mutate_committed_document() {
        let mut state = TextEditState::new("abc");
        state.bind_session(TextInputSessionId(4));
        state.set_composition(
            TextCompositionUpdate::new(
                "かな",
                TextRange::new(TextByteOffset(0), TextByteOffset(6)),
            )
            .with_replacement(TextRange::new(TextByteOffset(1), TextByteOffset(2))),
        );

        let visual = state.visual_source();

        assert_eq!(state.document(), "abc");
        assert_eq!(state.session(), Some(TextInputSessionId(4)));
        assert_eq!(visual, crate::text_source::UiTextSource::plain("aかなc"));
    }

    #[test]
    fn text_input_batches_reject_stale_sessions_without_mutation() {
        let mut state = TextEditState::new("abc");
        state.bind_session(TextInputSessionId(1));
        let input = TextInput::single(
            TextInputSessionId(2),
            TextInputSerial(1),
            TextInputOperation::Commit(TextCommit::new("x")),
        );

        let error = state
            .apply_text_input(&input)
            .expect_err("stale session rejects");

        assert_eq!(
            error,
            TextEditError::StaleTextInputSession {
                active: Some(TextInputSessionId(1)),
                incoming: TextInputSessionId(2),
            }
        );
        assert_eq!(state.document(), "abc");
    }

    #[test]
    fn committed_text_updates_document_after_session_check() {
        let mut state = TextEditState::new("abc");
        state.bind_session(TextInputSessionId(1));
        let input = TextInput::single(
            TextInputSessionId(1),
            TextInputSerial(1),
            TextInputOperation::Commit(TextCommit::new("x")),
        );

        let outcome = state.apply_text_input(&input).expect("commit applies");

        assert!(outcome.changed());
        assert_eq!(state.document(), "xabc");
    }

    #[test]
    fn visual_buffer_orders_selection_composition_and_caret_parts() {
        let mut state = TextEditState::new("abc");
        state.bind_session(TextInputSessionId(4));
        state.set_composition(TextCompositionUpdate::new(
            "かな",
            TextRange::new(TextByteOffset(0), TextByteOffset(6)),
        ));
        let buffer = state.visual_buffer(
            None,
            HitRect::new(0.0, 0.0, 100.0, 20.0),
            TextFieldMetrics::default(),
            true,
        );

        assert_eq!(buffer.display_text(), "•••••");
        assert!(buffer.is_secure());
        assert_eq!(
            buffer.parts().last().map(super::TextFieldPartRect::part),
            Some(TextEditorPart::Caret)
        );
        assert!(
            buffer
                .parts()
                .iter()
                .any(|part| part.part() == TextEditorPart::Composition)
        );
    }
}
