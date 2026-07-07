//! Sans I/O clipboard contract for Arcweft runtime text controls.
//!
//! This module intentionally contains no OS, browser, pasteboard, permission API,
//! or runtime configuration access. It defines the typed boundary between editor
//! semantics and host adapters.

use crate::input::InteractionTarget;
use crate::text_input::{TextInputSessionId, TextRevision};
use core::fmt;

/// Arcweft capability names used by runtime policy and diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipboardCapability {
    ReadText,
    WriteText,
    Clear,
}

impl ClipboardCapability {
    pub const fn name(self) -> &'static str {
        match self {
            Self::ReadText => "clipboard.read",
            Self::WriteText => "clipboard.write",
            Self::Clear => "clipboard.clear",
        }
    }
}

/// Text-control clipboard operation requested by editor semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextClipboardOperation {
    Copy,
    Cut,
    Paste,
    Clear,
}

impl TextClipboardOperation {
    pub const fn capability(self) -> ClipboardCapability {
        match self {
            Self::Copy | Self::Cut => ClipboardCapability::WriteText,
            Self::Paste => ClipboardCapability::ReadText,
            Self::Clear => ClipboardCapability::Clear,
        }
    }
}

/// Why clipboard access is being requested.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextClipboardOrigin {
    UserKeyboardShortcut,
    UserPlatformClipboardEvent,
    RuntimeRequest,
}

impl TextClipboardOrigin {
    pub const fn is_user_initiated(self) -> bool {
        matches!(
            self,
            Self::UserKeyboardShortcut | Self::UserPlatformClipboardEvent
        )
    }
}

/// Plain text clipboard format for this implementation slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipboardTextFormat {
    PlainUtf8,
}

/// Redacted clipboard text wrapper.
#[derive(Clone, Eq, PartialEq)]
pub struct ClipboardText {
    text: String,
}

impl ClipboardText {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: normalize_clipboard_newlines(&text.into()),
        }
    }

    pub fn from_editor_selection(text: &str) -> Self {
        Self::new(text)
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn into_string(self) -> String {
        self.text
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn byte_len(&self) -> usize {
        self.text.len()
    }

    pub fn scalar_count(&self) -> usize {
        self.text.chars().count()
    }
}

impl fmt::Debug for ClipboardText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClipboardText")
            .field("payload", &"<redacted>")
            .field("bytes", &self.byte_len())
            .field("scalars", &self.scalar_count())
            .finish()
    }
}

/// Host request id assigned outside the editor.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TextClipboardRequestId(pub u64);

impl TextClipboardRequestId {
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// Clipboard intent emitted by the editor before a host request id exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextClipboardIntent {
    Write(TextClipboardWriteIntent),
    Read(TextClipboardReadIntent),
    Clear(TextClipboardClearIntent),
}

impl TextClipboardIntent {
    pub const fn operation(&self) -> TextClipboardOperation {
        match self {
            Self::Write(intent) => intent.operation,
            Self::Read(intent) => intent.operation,
            Self::Clear(intent) => intent.operation,
        }
    }

    pub const fn capability(&self) -> ClipboardCapability {
        self.operation().capability()
    }

    pub fn into_request(self, request_id: TextClipboardRequestId) -> TextClipboardRequest {
        match self {
            Self::Write(intent) => TextClipboardRequest::Write(intent.into_request(request_id)),
            Self::Read(intent) => TextClipboardRequest::Read(intent.into_request(request_id)),
            Self::Clear(intent) => TextClipboardRequest::Clear(intent.into_request(request_id)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextClipboardWriteIntent {
    target: InteractionTarget,
    session: TextInputSessionId,
    revision: TextRevision,
    operation: TextClipboardOperation,
    origin: TextClipboardOrigin,
    format: ClipboardTextFormat,
    text: ClipboardText,
}

impl TextClipboardWriteIntent {
    pub fn new(
        target: InteractionTarget,
        session: TextInputSessionId,
        revision: TextRevision,
        operation: TextClipboardOperation,
        origin: TextClipboardOrigin,
        text: ClipboardText,
    ) -> Self {
        debug_assert!(matches!(
            operation,
            TextClipboardOperation::Copy | TextClipboardOperation::Cut
        ));
        Self {
            target,
            session,
            revision,
            operation,
            origin,
            format: ClipboardTextFormat::PlainUtf8,
            text,
        }
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }
    pub const fn session(&self) -> TextInputSessionId {
        self.session
    }
    pub const fn revision(&self) -> TextRevision {
        self.revision
    }
    pub const fn operation(&self) -> TextClipboardOperation {
        self.operation
    }
    pub const fn origin(&self) -> TextClipboardOrigin {
        self.origin
    }
    pub const fn format(&self) -> ClipboardTextFormat {
        self.format
    }
    pub const fn text(&self) -> &ClipboardText {
        &self.text
    }

    fn into_request(self, request_id: TextClipboardRequestId) -> TextClipboardWriteRequest {
        TextClipboardWriteRequest {
            request_id,
            inner: self,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextClipboardReadIntent {
    target: InteractionTarget,
    session: TextInputSessionId,
    revision: TextRevision,
    operation: TextClipboardOperation,
    origin: TextClipboardOrigin,
    format: ClipboardTextFormat,
}

impl TextClipboardReadIntent {
    pub fn paste(
        target: InteractionTarget,
        session: TextInputSessionId,
        revision: TextRevision,
        origin: TextClipboardOrigin,
    ) -> Self {
        Self {
            target,
            session,
            revision,
            operation: TextClipboardOperation::Paste,
            origin,
            format: ClipboardTextFormat::PlainUtf8,
        }
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }
    pub const fn session(&self) -> TextInputSessionId {
        self.session
    }
    pub const fn revision(&self) -> TextRevision {
        self.revision
    }
    pub const fn operation(&self) -> TextClipboardOperation {
        self.operation
    }
    pub const fn origin(&self) -> TextClipboardOrigin {
        self.origin
    }
    pub const fn format(&self) -> ClipboardTextFormat {
        self.format
    }

    fn into_request(self, request_id: TextClipboardRequestId) -> TextClipboardReadRequest {
        TextClipboardReadRequest {
            request_id,
            inner: self,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextClipboardClearIntent {
    target: InteractionTarget,
    session: TextInputSessionId,
    revision: TextRevision,
    operation: TextClipboardOperation,
    origin: TextClipboardOrigin,
}

impl TextClipboardClearIntent {
    pub fn new(
        target: InteractionTarget,
        session: TextInputSessionId,
        revision: TextRevision,
        origin: TextClipboardOrigin,
    ) -> Self {
        Self {
            target,
            session,
            revision,
            operation: TextClipboardOperation::Clear,
            origin,
        }
    }

    fn into_request(self, request_id: TextClipboardRequestId) -> TextClipboardClearRequest {
        TextClipboardClearRequest {
            request_id,
            inner: self,
        }
    }
}

/// Clipboard request consumed by native/web host adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextClipboardRequest {
    Write(TextClipboardWriteRequest),
    Read(TextClipboardReadRequest),
    Clear(TextClipboardClearRequest),
}

impl TextClipboardRequest {
    pub const fn request_id(&self) -> TextClipboardRequestId {
        match self {
            Self::Write(request) => request.request_id,
            Self::Read(request) => request.request_id,
            Self::Clear(request) => request.request_id,
        }
    }

    pub const fn operation(&self) -> TextClipboardOperation {
        match self {
            Self::Write(request) => request.inner.operation,
            Self::Read(request) => request.inner.operation,
            Self::Clear(request) => request.inner.operation,
        }
    }

    pub const fn capability(&self) -> ClipboardCapability {
        self.operation().capability()
    }

    pub const fn origin(&self) -> TextClipboardOrigin {
        match self {
            Self::Write(request) => request.inner.origin,
            Self::Read(request) => request.inner.origin,
            Self::Clear(request) => request.inner.origin,
        }
    }

    pub const fn session(&self) -> TextInputSessionId {
        match self {
            Self::Write(request) => request.inner.session,
            Self::Read(request) => request.inner.session,
            Self::Clear(request) => request.inner.session,
        }
    }

    pub const fn target(&self) -> &InteractionTarget {
        match self {
            Self::Write(request) => &request.inner.target,
            Self::Read(request) => &request.inner.target,
            Self::Clear(request) => &request.inner.target,
        }
    }

    pub const fn revision(&self) -> TextRevision {
        match self {
            Self::Write(request) => request.inner.revision,
            Self::Read(request) => request.inner.revision,
            Self::Clear(request) => request.inner.revision,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextClipboardWriteRequest {
    request_id: TextClipboardRequestId,
    inner: TextClipboardWriteIntent,
}

impl TextClipboardWriteRequest {
    pub const fn request_id(&self) -> TextClipboardRequestId {
        self.request_id
    }
    pub const fn inner(&self) -> &TextClipboardWriteIntent {
        &self.inner
    }
    pub const fn text(&self) -> &ClipboardText {
        &self.inner.text
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextClipboardReadRequest {
    request_id: TextClipboardRequestId,
    inner: TextClipboardReadIntent,
}

impl TextClipboardReadRequest {
    pub const fn request_id(&self) -> TextClipboardRequestId {
        self.request_id
    }
    pub const fn inner(&self) -> &TextClipboardReadIntent {
        &self.inner
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextClipboardClearRequest {
    request_id: TextClipboardRequestId,
    inner: TextClipboardClearIntent,
}

impl TextClipboardClearRequest {
    pub const fn request_id(&self) -> TextClipboardRequestId {
        self.request_id
    }
    pub const fn inner(&self) -> &TextClipboardClearIntent {
        &self.inner
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextClipboardOutcome {
    WriteCommitted {
        request_id: TextClipboardRequestId,
    },
    ReadCommitted {
        request_id: TextClipboardRequestId,
        text: ClipboardText,
    },
    Cleared {
        request_id: TextClipboardRequestId,
    },
    Failed {
        request_id: TextClipboardRequestId,
        error: TextClipboardError,
    },
}

impl TextClipboardOutcome {
    pub const fn request_id(&self) -> TextClipboardRequestId {
        match self {
            Self::WriteCommitted { request_id }
            | Self::ReadCommitted { request_id, .. }
            | Self::Cleared { request_id }
            | Self::Failed { request_id, .. } => *request_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextClipboardError {
    kind: TextClipboardErrorKind,
    operation: TextClipboardOperation,
    capability: ClipboardCapability,
    diagnostic: Option<String>,
}

impl TextClipboardError {
    pub fn new(kind: TextClipboardErrorKind, operation: TextClipboardOperation) -> Self {
        Self {
            kind,
            operation,
            capability: operation.capability(),
            diagnostic: None,
        }
    }

    #[must_use]
    pub fn with_diagnostic(mut self, diagnostic: impl Into<String>) -> Self {
        self.diagnostic = Some(diagnostic.into());
        self
    }

    pub const fn kind(&self) -> TextClipboardErrorKind {
        self.kind
    }
    pub const fn operation(&self) -> TextClipboardOperation {
        self.operation
    }
    pub const fn capability(&self) -> ClipboardCapability {
        self.capability
    }
    pub fn diagnostic(&self) -> Option<&str> {
        self.diagnostic.as_deref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextClipboardErrorKind {
    Unavailable,
    Denied,
    PolicyDenied,
    UnsupportedFormat,
    Busy,
    Stale,
    SecureFieldBlocked,
    InternalFailure,
}

impl TextClipboardErrorKind {
    pub const fn retry_policy(self) -> ClipboardRetryPolicy {
        match self {
            Self::Busy => ClipboardRetryPolicy::HostMayRetryOnce,
            Self::Unavailable
            | Self::Denied
            | Self::PolicyDenied
            | Self::UnsupportedFormat
            | Self::Stale
            | Self::SecureFieldBlocked
            | Self::InternalFailure => ClipboardRetryPolicy::DoNotRetry,
        }
    }

    pub const fn may_use_local_fallback(self) -> bool {
        matches!(
            self,
            Self::Unavailable
                | Self::Denied
                | Self::UnsupportedFormat
                | Self::Busy
                | Self::InternalFailure
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipboardRetryPolicy {
    DoNotRetry,
    HostMayRetryOnce,
}

pub fn normalize_clipboard_newlines(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                normalized.push('\n');
            }
            _ => normalized.push(ch),
        }
    }
    normalized
}
