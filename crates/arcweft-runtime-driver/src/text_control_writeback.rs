use arcweft_bundle::resource_codec::ui::{
    UiRuntimeTextControl, UiRuntimeTextControlHandler, UiRuntimeTextSelection,
};
use arcweft_presentation::text_input::{
    TextControlValue, TextControlWriteBack, TextControlWriteBackKind,
};
use core::fmt;

/// Runtime-owned, typed write-back event emitted by player text controls.
///
/// This is intentionally not an `InteractionPayload::Text` or JSON payload. The
/// value remains typed until an AWBC handler boundary is explicitly approved to
/// receive it. `Debug` redacts sensitive values through `TextControlValue`.
#[derive(Clone, Eq, PartialEq)]
pub struct RuntimeTextControlWriteBack {
    control_id: String,
    target: String,
    session: u64,
    kind: RuntimeTextControlWriteBackKind,
    value: TextControlValue,
    selection: UiRuntimeTextSelection,
    handler: Option<UiRuntimeTextControlHandler>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeTextControlWriteBackKind {
    Change,
    Submit,
}

impl RuntimeTextControlWriteBack {
    pub fn from_control(write_back: &TextControlWriteBack, control: &UiRuntimeTextControl) -> Self {
        let handler = match write_back.kind() {
            TextControlWriteBackKind::Change => control.handlers.change.clone(),
            TextControlWriteBackKind::Submit => control.handlers.submit.clone(),
        };
        Self {
            control_id: control.public_id.clone(),
            target: control.target.clone(),
            session: control.session,
            kind: write_back.kind().into(),
            value: write_back.value().clone(),
            selection: UiRuntimeTextSelection::new(
                write_back.selection().start().get(),
                write_back.selection().end().get(),
            ),
            handler,
        }
    }

    pub fn control_id(&self) -> &str {
        &self.control_id
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub const fn session(&self) -> u64 {
        self.session
    }

    pub const fn kind(&self) -> RuntimeTextControlWriteBackKind {
        self.kind
    }

    pub const fn is_change(&self) -> bool {
        matches!(self.kind, RuntimeTextControlWriteBackKind::Change)
    }

    pub const fn is_submit(&self) -> bool {
        matches!(self.kind, RuntimeTextControlWriteBackKind::Submit)
    }

    /// Returns the committed value for the runtime owner. Do not serialize this
    /// through diagnostics, traces, capture metadata, or replay unless an
    /// explicit secure channel has been approved.
    pub const fn value(&self) -> &TextControlValue {
        &self.value
    }

    pub const fn selection(&self) -> UiRuntimeTextSelection {
        self.selection
    }

    pub const fn handler(&self) -> Option<&UiRuntimeTextControlHandler> {
        self.handler.as_ref()
    }
}

impl fmt::Debug for RuntimeTextControlWriteBack {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeTextControlWriteBack")
            .field("control_id", &self.control_id)
            .field("target", &self.target)
            .field("session", &self.session)
            .field("kind", &self.kind)
            .field("value", &self.value)
            .field("selection", &self.selection)
            .field("handler", &self.handler)
            .finish()
    }
}

impl From<TextControlWriteBackKind> for RuntimeTextControlWriteBackKind {
    fn from(kind: TextControlWriteBackKind) -> Self {
        match kind {
            TextControlWriteBackKind::Change => Self::Change,
            TextControlWriteBackKind::Submit => Self::Submit,
        }
    }
}
