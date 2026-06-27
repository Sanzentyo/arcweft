//! Retained TextField/TextArea state and style parts.

use crate::text_source::{UiTextByteRange, UiTextSource};
use crate::{HandlerId, TextSourceId};
use arcweft_presentation::text_input::{
    TextCompositionUpdate, TextInputOptions, TextInputSessionId, TextRevision,
};

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
}

#[cfg(test)]
mod tests {
    use super::TextEditState;
    use arcweft_presentation::text_input::{
        TextByteOffset, TextCompositionUpdate, TextInputSessionId, TextRange,
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
}
