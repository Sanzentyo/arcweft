//! Retained TextField/TextArea state and style parts.

use crate::text_source::{ViewTextByteRange, ViewTextSource};
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
    selection: ViewTextByteRange,
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
        range: ViewTextByteRange,
        document_len: usize,
    },
    #[error("text byte range {range:?} does not align with UTF-8 boundaries")]
    NonBoundaryRange { range: ViewTextByteRange },
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
    range: Option<ViewTextByteRange>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextFieldVisualBuffer {
    target: Option<InteractionTarget>,
    bounds: HitRect,
    display_text: String,
    source: ViewTextSource,
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
            selection: ViewTextByteRange::new(0, 0),
            composition: None,
            revision: TextRevision::default(),
            session: None,
        }
    }

    pub fn document(&self) -> &str {
        &self.document
    }

    pub const fn selection(&self) -> ViewTextByteRange {
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

    pub fn visual_source(&self) -> ViewTextSource {
        if let Some(composition) = &self.composition {
            let mut visual = self.document.clone();
            let range = composition.replacement().map_or(self.selection, |range| {
                ViewTextByteRange::new(range.start().0, range.end().0)
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
            ViewTextSource::plain(visual)
        } else {
            ViewTextSource::plain(self.document.clone())
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
                    next.selection = view_range(selection.range());
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
            ViewTextSource::Plain(text) if secure => mask_text(text),
            ViewTextSource::Plain(text) => text.clone(),
            ViewTextSource::Localized(_)
            | ViewTextSource::RichTextDocument(_)
            | ViewTextSource::DisplayFrame(_) => String::new(),
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
            let base = composition.replacement().map_or(self.selection, view_range);
            let composition_range = ViewTextByteRange::new(
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
        let caret = ViewTextByteRange::new(self.selection.end(), self.selection.end());
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
                    .map_or(self.selection, view_range)
            },
            view_range,
        );
        replace_range(&mut self.document, range, commit.text())?;
        let caret = range
            .start()
            .saturating_add(u32::try_from(commit.text().len()).unwrap_or(u32::MAX));
        self.selection = ViewTextByteRange::new(caret, caret);
        self.composition = None;
        Ok(true)
    }

    fn end_composition(&mut self, reason: CompositionEndReason) -> Result<bool, TextEditError> {
        let Some(composition) = self.composition.take() else {
            return Ok(false);
        };
        if reason == CompositionEndReason::Committed {
            let range = composition.replacement().map_or(self.selection, view_range);
            replace_range(&mut self.document, range, composition.preedit())?;
            let caret = range
                .start()
                .saturating_add(u32::try_from(composition.preedit().len()).unwrap_or(u32::MAX));
            self.selection = ViewTextByteRange::new(caret, caret);
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
        let range = ViewTextByteRange::new(start, end);
        replace_range(&mut self.document, range, "")?;
        self.selection = ViewTextByteRange::new(start, start);
        Ok(start != end)
    }

    fn apply_command(
        &mut self,
        command: TextEditCommand,
    ) -> Result<TextEditOutcome, TextEditError> {
        match command {
            TextEditCommand::Backspace | TextEditCommand::DeleteWordLeft => {
                Ok(TextEditOutcome::new(
                    self.delete_surrounding(1, 0, TextDeleteUnit::Utf16CodeUnit)?,
                    false,
                    self.revision,
                ))
            }
            TextEditCommand::Delete | TextEditCommand::DeleteWordRight => Ok(TextEditOutcome::new(
                self.delete_surrounding(0, 1, TextDeleteUnit::Utf16CodeUnit)?,
                false,
                self.revision,
            )),
            TextEditCommand::SelectAll => {
                self.selection = ViewTextByteRange::new(
                    0,
                    u32::try_from(self.document.len()).unwrap_or(u32::MAX),
                );
                Ok(TextEditOutcome::new(false, false, self.revision))
            }
            TextEditCommand::Submit => Ok(TextEditOutcome::new(false, true, self.revision)),
            TextEditCommand::Cancel => {
                self.composition = None;
                Ok(TextEditOutcome::new(false, false, self.revision))
            }
            TextEditCommand::MoveLeft { selecting: _ }
            | TextEditCommand::MoveUp { selecting: _ }
            | TextEditCommand::MoveWordLeft { selecting: _ }
            | TextEditCommand::MoveLineStart { selecting: _ }
            | TextEditCommand::MoveDocumentStart { selecting: _ }
            | TextEditCommand::MovePageUp { selecting: _ } => {
                self.selection = ViewTextByteRange::new(0, 0);
                Ok(TextEditOutcome::new(false, false, self.revision))
            }
            TextEditCommand::MoveRight { selecting: _ }
            | TextEditCommand::MoveDown { selecting: _ }
            | TextEditCommand::MoveWordRight { selecting: _ }
            | TextEditCommand::MoveLineEnd { selecting: _ }
            | TextEditCommand::MoveDocumentEnd { selecting: _ }
            | TextEditCommand::MovePageDown { selecting: _ } => {
                let end = u32::try_from(self.document.len()).unwrap_or(u32::MAX);
                self.selection = ViewTextByteRange::new(end, end);
                Ok(TextEditOutcome::new(false, false, self.revision))
            }
            TextEditCommand::SelectWord | TextEditCommand::SelectLine => {
                let end = u32::try_from(self.document.len()).unwrap_or(u32::MAX);
                self.selection = ViewTextByteRange::new(0, end);
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
        range: Option<ViewTextByteRange>,
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

    pub const fn range(&self) -> Option<ViewTextByteRange> {
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

    pub const fn source(&self) -> &ViewTextSource {
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

fn view_range(range: TextRange<TextByteOffset>) -> ViewTextByteRange {
    ViewTextByteRange::new(range.start().0, range.end().0)
}

fn replace_range(
    document: &mut String,
    range: ViewTextByteRange,
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

fn range_rect(bounds: HitRect, range: ViewTextByteRange, metrics: TextFieldMetrics) -> HitRect {
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

fn underline_rect(bounds: HitRect, range: ViewTextByteRange, metrics: TextFieldMetrics) -> HitRect {
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

/// Binding update strategy for platform IME edits.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextFieldBindingCommitPolicy {
    #[default]
    OnCommittedEdit,
    OnSubmit,
    Manual,
}

/// Per-field editing policy used by `TextField`, `TextArea`, and `SecureField`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextFieldEditPolicy {
    secure: bool,
    binding_commit: TextFieldBindingCommitPolicy,
}

/// Geometry conversion policy for candidate windows and character bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextFieldGeometryPolicy {
    writing_mode: arcweft_presentation::text_input::TextWritingMode,
    text_local_to_viewport: arcweft_presentation::text_input::TextGeometryTransform,
    viewport_to_screen: arcweft_presentation::text_input::TextGeometryTransform,
}

/// Policy-aware edit failure for secure fields and Unicode delete-surrounding.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TextFieldPolicyEditError {
    #[error(transparent)]
    Edit(#[from] TextEditError),
    #[error("secure text input batch was not marked sensitive")]
    SecureInputNotRedacted,
    #[error("secure text input forbids clipboard command {0:?}")]
    SecureClipboardCommand(TextEditCommand),
}

impl TextFieldEditPolicy {
    pub const fn plain() -> Self {
        Self {
            secure: false,
            binding_commit: TextFieldBindingCommitPolicy::OnCommittedEdit,
        }
    }

    pub const fn secure() -> Self {
        Self {
            secure: true,
            binding_commit: TextFieldBindingCommitPolicy::OnCommittedEdit,
        }
    }

    #[must_use]
    pub const fn with_binding_commit(
        mut self,
        binding_commit: TextFieldBindingCommitPolicy,
    ) -> Self {
        self.binding_commit = binding_commit;
        self
    }

    pub const fn is_secure(self) -> bool {
        self.secure
    }

    pub const fn binding_commit(self) -> TextFieldBindingCommitPolicy {
        self.binding_commit
    }
}

impl Default for TextFieldGeometryPolicy {
    fn default() -> Self {
        Self {
            writing_mode: arcweft_presentation::text_input::TextWritingMode::HorizontalTb,
            text_local_to_viewport:
                arcweft_presentation::text_input::TextGeometryTransform::identity(),
            viewport_to_screen: arcweft_presentation::text_input::TextGeometryTransform::identity(),
        }
    }
}

impl TextFieldGeometryPolicy {
    #[must_use]
    pub const fn with_writing_mode(
        mut self,
        writing_mode: arcweft_presentation::text_input::TextWritingMode,
    ) -> Self {
        self.writing_mode = writing_mode;
        self
    }

    #[must_use]
    pub const fn with_text_local_to_viewport(
        mut self,
        transform: arcweft_presentation::text_input::TextGeometryTransform,
    ) -> Self {
        self.text_local_to_viewport = transform;
        self
    }

    #[must_use]
    pub const fn with_viewport_to_screen(
        mut self,
        transform: arcweft_presentation::text_input::TextGeometryTransform,
    ) -> Self {
        self.viewport_to_screen = transform;
        self
    }

    pub const fn writing_mode(self) -> arcweft_presentation::text_input::TextWritingMode {
        self.writing_mode
    }
}

impl TextEditOutcome {
    pub const fn should_commit_binding(self, policy: TextFieldBindingCommitPolicy) -> bool {
        match policy {
            TextFieldBindingCommitPolicy::OnCommittedEdit => self.changed,
            TextFieldBindingCommitPolicy::OnSubmit => self.submitted,
            TextFieldBindingCommitPolicy::Manual => false,
        }
    }
}

impl TextEditState {
    /// Applies a platform IME batch with secure-field and Unicode deletion policy.
    ///
    /// Preedit changes are visual-only, and callers can use
    /// [`TextEditOutcome::should_commit_binding`] to decide whether to write the
    /// committed document back to the bound value.
    pub fn apply_text_input_with_policy(
        &mut self,
        input: &TextInput,
        policy: TextFieldEditPolicy,
    ) -> Result<TextEditOutcome, TextFieldPolicyEditError> {
        if policy.secure && !input.privacy().is_sensitive() {
            return Err(TextFieldPolicyEditError::SecureInputNotRedacted);
        }
        if self.session != Some(input.session()) {
            return Err(TextEditError::StaleTextInputSession {
                active: self.session,
                incoming: input.session(),
            }
            .into());
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
                    changed |= next.delete_surrounding_by_unit(*before, *after, *unit)?;
                }
                TextInputOperation::SetSelection(selection) => {
                    next.selection = view_range(selection.range());
                }
                TextInputOperation::Command(command) => {
                    let command_result = next.apply_command_with_policy(*command, policy)?;
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

    pub fn text_input_client_snapshot(
        &self,
        session: TextInputSessionId,
        target: InteractionTarget,
        bounds: HitRect,
        metrics: TextFieldMetrics,
        options: TextInputOptions,
        policy: TextFieldEditPolicy,
    ) -> arcweft_presentation::text_input::TextInputClientSnapshot {
        let selection = TextRange::new(
            TextByteOffset(self.selection.start()),
            TextByteOffset(self.selection.end()),
        );
        let mut snapshot = arcweft_presentation::text_input::TextInputClientSnapshot::new(
            session,
            target,
            self.revision,
            self.document.clone(),
            TextByteOffset(0),
            selection,
            bounds,
            caret_rect(bounds, self.selection.end(), metrics),
            options.secure(policy.secure),
        )
        .with_character_bounds(character_bounds_for_visual_text(
            &self.visual_source(),
            bounds,
            metrics,
            arcweft_presentation::text_input::TextWritingMode::HorizontalTb,
        ));
        if let Some(composition) = &self.composition {
            snapshot = snapshot.with_composition(composition.clone());
        }
        if policy.secure {
            snapshot.redacted_for_secure_input()
        } else {
            snapshot
        }
    }

    pub fn text_input_geometry_snapshot(
        &self,
        session: TextInputSessionId,
        bounds: HitRect,
        metrics: TextFieldMetrics,
        policy: TextFieldGeometryPolicy,
    ) -> arcweft_presentation::text_input::TextInputGeometrySnapshot {
        let caret =
            caret_rect_for_writing_mode(bounds, self.selection.end(), metrics, policy.writing_mode);
        let character_bounds = character_bounds_for_visual_text(
            &self.visual_source(),
            bounds,
            metrics,
            policy.writing_mode,
        );
        let text_local_selection_rects =
            selection_rects_for_text_field(self.selection, bounds, metrics, policy.writing_mode);
        let text_local_composition_rects =
            self.composition
                .as_ref()
                .map_or_else(Vec::new, |composition| {
                    let base = composition.replacement().map_or(self.selection, view_range);
                    let range = ViewTextByteRange::new(
                        base.start(),
                        base.start().saturating_add(
                            u32::try_from(composition.preedit().len()).unwrap_or(u32::MAX),
                        ),
                    );
                    selection_rects_for_text_field(range, bounds, metrics, policy.writing_mode)
                });
        arcweft_presentation::text_input::TextInputGeometrySnapshot::new(
            arcweft_presentation::text_input::TextInputGeometrySnapshotParts {
                session,
                revision: self.revision,
                writing_mode: policy.writing_mode,
                text_local_control_rect: bounds,
                text_local_caret_rect: caret,
                text_local_character_bounds: character_bounds,
                text_local_selection_rects,
                text_local_composition_rects,
                text_local_to_viewport: policy.text_local_to_viewport,
                viewport_to_screen: policy.viewport_to_screen,
            },
        )
    }

    fn delete_surrounding_by_unit(
        &mut self,
        before: u32,
        after: u32,
        unit: TextDeleteUnit,
    ) -> Result<bool, TextEditError> {
        let range = surrounding_delete_range(&self.document, self.selection, before, after, unit)?;
        replace_range(&mut self.document, range, "")?;
        self.selection = ViewTextByteRange::new(range.start(), range.start());
        Ok(range.start() != range.end())
    }

    fn apply_command_with_policy(
        &mut self,
        command: TextEditCommand,
        policy: TextFieldEditPolicy,
    ) -> Result<TextEditOutcome, TextFieldPolicyEditError> {
        if policy.secure
            && matches!(
                command,
                TextEditCommand::Copy | TextEditCommand::Cut | TextEditCommand::Paste
            )
        {
            return Err(TextFieldPolicyEditError::SecureClipboardCommand(command));
        }
        match command {
            TextEditCommand::Backspace | TextEditCommand::DeleteWordLeft => {
                Ok(TextEditOutcome::new(
                    self.delete_surrounding_by_unit(1, 0, TextDeleteUnit::GraphemeCluster)?,
                    false,
                    self.revision,
                ))
            }
            TextEditCommand::Delete | TextEditCommand::DeleteWordRight => Ok(TextEditOutcome::new(
                self.delete_surrounding_by_unit(0, 1, TextDeleteUnit::GraphemeCluster)?,
                false,
                self.revision,
            )),
            TextEditCommand::Copy
            | TextEditCommand::Cut
            | TextEditCommand::Paste
            | TextEditCommand::SelectAll
            | TextEditCommand::Submit
            | TextEditCommand::Cancel
            | TextEditCommand::MoveLeft { .. }
            | TextEditCommand::MoveRight { .. }
            | TextEditCommand::MoveUp { .. }
            | TextEditCommand::MoveDown { .. }
            | TextEditCommand::MoveWordLeft { .. }
            | TextEditCommand::MoveWordRight { .. }
            | TextEditCommand::MoveLineStart { .. }
            | TextEditCommand::MoveLineEnd { .. }
            | TextEditCommand::MoveDocumentStart { .. }
            | TextEditCommand::MoveDocumentEnd { .. }
            | TextEditCommand::MovePageUp { .. }
            | TextEditCommand::MovePageDown { .. }
            | TextEditCommand::SelectWord
            | TextEditCommand::SelectLine => Ok(self.apply_command(command)?),
        }
    }
}

fn surrounding_delete_range(
    document: &str,
    selection: ViewTextByteRange,
    before: u32,
    after: u32,
    unit: TextDeleteUnit,
) -> Result<ViewTextByteRange, TextEditError> {
    let start = checked_boundary(document, selection.start())?;
    let end = checked_boundary(document, selection.end())?;
    let start = match unit {
        TextDeleteUnit::Utf8Byte => offset_before_bytes(document, start, before)?,
        TextDeleteUnit::Utf16CodeUnit => offset_before_utf16(document, start, before),
        TextDeleteUnit::UnicodeScalar => offset_before_scalars(document, start, before),
        TextDeleteUnit::GraphemeCluster => offset_before_graphemes(document, start, before),
    };
    let end = match unit {
        TextDeleteUnit::Utf8Byte => offset_after_bytes(document, end, after)?,
        TextDeleteUnit::Utf16CodeUnit => offset_after_utf16(document, end, after),
        TextDeleteUnit::UnicodeScalar => offset_after_scalars(document, end, after),
        TextDeleteUnit::GraphemeCluster => offset_after_graphemes(document, end, after),
    };
    Ok(ViewTextByteRange::new(start, end))
}

fn checked_boundary(document: &str, offset: u32) -> Result<usize, TextEditError> {
    let offset = usize::try_from(offset).unwrap_or(usize::MAX);
    if offset > document.len() {
        return Err(TextEditError::InvalidByteRange {
            range: ViewTextByteRange::new(
                u32::try_from(offset).unwrap_or(u32::MAX),
                u32::try_from(offset).unwrap_or(u32::MAX),
            ),
            document_len: document.len(),
        });
    }
    if !document.is_char_boundary(offset) {
        let offset = u32::try_from(offset).unwrap_or(u32::MAX);
        return Err(TextEditError::NonBoundaryRange {
            range: ViewTextByteRange::new(offset, offset),
        });
    }
    Ok(offset)
}

fn offset_before_bytes(document: &str, start: usize, count: u32) -> Result<u32, TextEditError> {
    let offset = start.saturating_sub(usize::try_from(count).unwrap_or(usize::MAX));
    let offset = u32::try_from(offset).unwrap_or(u32::MAX);
    let _ = checked_boundary(document, offset)?;
    Ok(offset)
}

fn offset_after_bytes(document: &str, end: usize, count: u32) -> Result<u32, TextEditError> {
    let offset = end
        .saturating_add(usize::try_from(count).unwrap_or(usize::MAX))
        .min(document.len());
    let offset = u32::try_from(offset).unwrap_or(u32::MAX);
    let _ = checked_boundary(document, offset)?;
    Ok(offset)
}

fn offset_before_utf16(document: &str, byte: usize, mut units: u32) -> u32 {
    if units == 0 {
        return u32::try_from(byte).unwrap_or(u32::MAX);
    }
    for (index, scalar) in document[..byte].char_indices().rev() {
        let width: u32 = if scalar.len_utf16() == 2 { 2 } else { 1 };
        if units <= width {
            return u32::try_from(index).unwrap_or(0);
        }
        units -= width;
    }
    0
}

fn offset_after_utf16(document: &str, byte: usize, mut units: u32) -> u32 {
    if units == 0 {
        return u32::try_from(byte).unwrap_or(u32::MAX);
    }
    for (relative, scalar) in document[byte..].char_indices() {
        let end = byte + relative + scalar.len_utf8();
        let width: u32 = if scalar.len_utf16() == 2 { 2 } else { 1 };
        if units <= width {
            return u32::try_from(end).unwrap_or(u32::MAX);
        }
        units -= width;
    }
    u32::try_from(document.len()).unwrap_or(u32::MAX)
}

fn offset_before_scalars(document: &str, byte: usize, count: u32) -> u32 {
    if count == 0 {
        return u32::try_from(byte).unwrap_or(u32::MAX);
    }
    document[..byte]
        .char_indices()
        .rev()
        .nth(count.saturating_sub(1) as usize)
        .map_or(0, |(index, _)| u32::try_from(index).unwrap_or(0))
}

fn offset_after_scalars(document: &str, byte: usize, count: u32) -> u32 {
    if count == 0 {
        return u32::try_from(byte).unwrap_or(u32::MAX);
    }
    document[byte..]
        .char_indices()
        .nth(count.saturating_sub(1) as usize)
        .map_or_else(
            || u32::try_from(document.len()).unwrap_or(u32::MAX),
            |(relative, scalar)| {
                u32::try_from(byte + relative + scalar.len_utf8()).unwrap_or(u32::MAX)
            },
        )
}

fn offset_before_graphemes(document: &str, byte: usize, count: u32) -> u32 {
    if count == 0 {
        return u32::try_from(byte).unwrap_or(u32::MAX);
    }
    let prefix = &document[..byte];
    let mut starts = unicode_segmentation::UnicodeSegmentation::grapheme_indices(prefix, true)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    starts.push(prefix.len());
    let target = starts.len().saturating_sub(count as usize + 1);
    u32::try_from(starts[target]).unwrap_or(0)
}

fn offset_after_graphemes(document: &str, byte: usize, count: u32) -> u32 {
    if count == 0 {
        return u32::try_from(byte).unwrap_or(u32::MAX);
    }
    let mut end = byte;
    for (relative, grapheme) in
        unicode_segmentation::UnicodeSegmentation::grapheme_indices(&document[byte..], true)
            .take(count as usize)
    {
        end = byte + relative + grapheme.len();
    }
    u32::try_from(end).unwrap_or(u32::MAX)
}

fn character_bounds_for_visual_text(
    source: &ViewTextSource,
    bounds: HitRect,
    metrics: TextFieldMetrics,
    writing_mode: arcweft_presentation::text_input::TextWritingMode,
) -> Vec<arcweft_presentation::text_input::TextCharacterBounds> {
    let ViewTextSource::Plain(text) = source else {
        return Vec::new();
    };
    text.char_indices()
        .map(|(start, scalar)| {
            let end = start + scalar.len_utf8();
            let range = ViewTextByteRange::new(
                u32::try_from(start).unwrap_or(u32::MAX),
                u32::try_from(end).unwrap_or(u32::MAX),
            );
            arcweft_presentation::text_input::TextCharacterBounds::new(
                TextRange::new(TextByteOffset(range.start()), TextByteOffset(range.end())),
                range_rect_for_writing_mode(bounds, range, metrics, writing_mode),
            )
        })
        .collect()
}

fn selection_rects_for_text_field(
    range: ViewTextByteRange,
    bounds: HitRect,
    metrics: TextFieldMetrics,
    writing_mode: arcweft_presentation::text_input::TextWritingMode,
) -> Vec<arcweft_presentation::text_input::TextRangeRect> {
    if range.start() == range.end() {
        return Vec::new();
    }
    vec![arcweft_presentation::text_input::TextRangeRect::new(
        TextRange::new(TextByteOffset(range.start()), TextByteOffset(range.end())),
        range_rect_for_writing_mode(bounds, range, metrics, writing_mode),
    )]
}

fn range_rect_for_writing_mode(
    bounds: HitRect,
    range: ViewTextByteRange,
    metrics: TextFieldMetrics,
    writing_mode: arcweft_presentation::text_input::TextWritingMode,
) -> HitRect {
    match writing_mode {
        arcweft_presentation::text_input::TextWritingMode::HorizontalTb => {
            range_rect(bounds, range, metrics)
        }
        arcweft_presentation::text_input::TextWritingMode::VerticalRl => {
            let start = range.start().min(range.end());
            let end = range.end().max(range.start());
            let y = bounds.y + byte_offset_px(start, metrics.advance_px);
            let height = byte_offset_px(end.saturating_sub(start), metrics.advance_px)
                .max(metrics.caret_width_px);
            HitRect::new(
                bounds.x + bounds.width - metrics.line_height_px.min(bounds.width),
                y,
                metrics.line_height_px.min(bounds.width),
                height,
            )
        }
        arcweft_presentation::text_input::TextWritingMode::VerticalLr => {
            let start = range.start().min(range.end());
            let end = range.end().max(range.start());
            let y = bounds.y + byte_offset_px(start, metrics.advance_px);
            let height = byte_offset_px(end.saturating_sub(start), metrics.advance_px)
                .max(metrics.caret_width_px);
            HitRect::new(
                bounds.x,
                y,
                metrics.line_height_px.min(bounds.width),
                height,
            )
        }
    }
}

fn caret_rect_for_writing_mode(
    bounds: HitRect,
    offset: u32,
    metrics: TextFieldMetrics,
    writing_mode: arcweft_presentation::text_input::TextWritingMode,
) -> HitRect {
    range_rect_for_writing_mode(
        bounds,
        ViewTextByteRange::new(offset, offset),
        metrics,
        writing_mode,
    )
}

#[cfg(test)]
mod policy_tests;
#[cfg(test)]
mod tests;
