//! Native text clipboard adapter for Arcweft player windows.

use arcweft_presentation::clipboard::{
    ClipboardText, TextClipboardError, TextClipboardErrorKind, TextClipboardOperation,
    TextClipboardOutcome, TextClipboardRequest,
};
use arcweft_runtime_host::clipboard_host::SyncTextClipboardHostAdapter;

pub struct NativeClipboardAdapter {
    clipboard: Option<arboard::Clipboard>,
}

impl NativeClipboardAdapter {
    pub fn new() -> Self {
        Self { clipboard: None }
    }

    fn clipboard(&mut self) -> Result<&mut arboard::Clipboard, TextClipboardErrorKind> {
        if self.clipboard.is_none() {
            self.clipboard = Some(arboard::Clipboard::new().map_err(map_arboard_error)?);
        }
        self.clipboard
            .as_mut()
            .ok_or(TextClipboardErrorKind::Unavailable)
    }

    fn write_text(&mut self, request: &TextClipboardRequest) -> TextClipboardOutcome {
        let TextClipboardRequest::Write(write) = request else {
            return failed(request, TextClipboardErrorKind::InternalFailure);
        };
        match self.clipboard().and_then(|clipboard| {
            clipboard
                .set_text(write.text().as_str())
                .map_err(map_arboard_error)
        }) {
            Ok(()) => TextClipboardOutcome::WriteCommitted {
                request_id: request.request_id(),
            },
            Err(kind) => failed(request, kind),
        }
    }

    fn read_text(&mut self, request: &TextClipboardRequest) -> TextClipboardOutcome {
        match self
            .clipboard()
            .and_then(|clipboard| clipboard.get_text().map_err(map_arboard_error))
        {
            Ok(text) => TextClipboardOutcome::ReadCommitted {
                request_id: request.request_id(),
                text: ClipboardText::new(text),
            },
            Err(kind) => failed(request, kind),
        }
    }

    fn clear(&mut self, request: &TextClipboardRequest) -> TextClipboardOutcome {
        match self
            .clipboard()
            .and_then(|clipboard| clipboard.clear().map_err(map_arboard_error))
        {
            Ok(()) => TextClipboardOutcome::Cleared {
                request_id: request.request_id(),
            },
            Err(kind) => failed(request, kind),
        }
    }
}

impl Default for NativeClipboardAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncTextClipboardHostAdapter for NativeClipboardAdapter {
    fn apply_clipboard_request_sync(
        &mut self,
        request: TextClipboardRequest,
    ) -> TextClipboardOutcome {
        match request.operation() {
            TextClipboardOperation::Copy | TextClipboardOperation::Cut => self.write_text(&request),
            TextClipboardOperation::Paste => self.read_text(&request),
            TextClipboardOperation::Clear => self.clear(&request),
        }
    }
}

fn failed(request: &TextClipboardRequest, kind: TextClipboardErrorKind) -> TextClipboardOutcome {
    TextClipboardOutcome::Failed {
        request_id: request.request_id(),
        error: TextClipboardError::new(kind, request.operation()),
    }
}

fn map_arboard_error(error: arboard::Error) -> TextClipboardErrorKind {
    match error {
        arboard::Error::ContentNotAvailable => TextClipboardErrorKind::UnsupportedFormat,
        arboard::Error::ClipboardNotSupported => TextClipboardErrorKind::Unavailable,
        arboard::Error::ClipboardOccupied => TextClipboardErrorKind::Busy,
        arboard::Error::ConversionFailure => TextClipboardErrorKind::UnsupportedFormat,
        arboard::Error::Unknown { .. } => TextClipboardErrorKind::InternalFailure,
        _ => TextClipboardErrorKind::InternalFailure,
    }
}
