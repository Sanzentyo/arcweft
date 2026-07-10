//! Runtime-host clipboard adapter and policy boundary.
//!
//! This module is Sans I/O: concrete native/browser APIs are implemented in
//! host/player crates and represented here only by typed requests/outcomes.

use arcweft_presentation::clipboard::{
    ClipboardCapability, TextClipboardError, TextClipboardErrorKind, TextClipboardOperation,
    TextClipboardOrigin, TextClipboardOutcome, TextClipboardRequest,
};
use core::future::{Future, ready};
use core::pin::Pin;

pub type ClipboardHostFuture<'a> = Pin<Box<dyn Future<Output = TextClipboardOutcome> + Send + 'a>>;

pub trait TextClipboardHostAdapter {
    fn apply_clipboard_request(&mut self, request: TextClipboardRequest)
    -> ClipboardHostFuture<'_>;
}

pub trait SyncTextClipboardHostAdapter {
    fn apply_clipboard_request_sync(
        &mut self,
        request: TextClipboardRequest,
    ) -> TextClipboardOutcome;
}

impl<T> TextClipboardHostAdapter for T
where
    T: SyncTextClipboardHostAdapter + Send,
{
    fn apply_clipboard_request(
        &mut self,
        request: TextClipboardRequest,
    ) -> ClipboardHostFuture<'_> {
        Box::pin(ready(self.apply_clipboard_request_sync(request)))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardCapabilityPolicy {
    read: ClipboardAccessPolicy,
    write: ClipboardAccessPolicy,
    clear: ClipboardAccessPolicy,
    secure_paste: SecurePastePolicy,
}

impl Default for ClipboardCapabilityPolicy {
    fn default() -> Self {
        Self {
            read: ClipboardAccessPolicy::UserInitiatedOnly,
            write: ClipboardAccessPolicy::UserInitiatedOnly,
            clear: ClipboardAccessPolicy::Deny,
            secure_paste: SecurePastePolicy::Deny,
        }
    }
}

impl ClipboardCapabilityPolicy {
    pub const fn new(
        read: ClipboardAccessPolicy,
        write: ClipboardAccessPolicy,
        clear: ClipboardAccessPolicy,
        secure_paste: SecurePastePolicy,
    ) -> Self {
        Self {
            read,
            write,
            clear,
            secure_paste,
        }
    }

    pub fn evaluate(
        &self,
        request: &TextClipboardRequest,
        secure_field: bool,
    ) -> Result<(), ClipboardPolicyRejection> {
        if secure_field {
            match request.operation() {
                TextClipboardOperation::Copy | TextClipboardOperation::Cut => {
                    return Err(ClipboardPolicyRejection::SecureFieldBlocked {
                        operation: request.operation(),
                    });
                }
                TextClipboardOperation::Paste if self.secure_paste == SecurePastePolicy::Deny => {
                    return Err(ClipboardPolicyRejection::SecureFieldBlocked {
                        operation: request.operation(),
                    });
                }
                TextClipboardOperation::Paste | TextClipboardOperation::Clear => {}
            }
        }

        let access = match request.capability() {
            ClipboardCapability::ReadText => self.read,
            ClipboardCapability::WriteText => self.write,
            ClipboardCapability::Clear => self.clear,
        };
        access.evaluate(request.origin(), request.operation())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipboardAccessPolicy {
    Deny,
    UserInitiatedOnly,
    AllowProgrammatic,
}

impl ClipboardAccessPolicy {
    fn evaluate(
        self,
        origin: TextClipboardOrigin,
        operation: TextClipboardOperation,
    ) -> Result<(), ClipboardPolicyRejection> {
        match self {
            Self::Deny => Err(ClipboardPolicyRejection::CapabilityDenied { operation }),
            Self::UserInitiatedOnly if origin.is_user_initiated() => Ok(()),
            Self::UserInitiatedOnly => {
                Err(ClipboardPolicyRejection::BackgroundDenied { operation })
            }
            Self::AllowProgrammatic => Ok(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecurePastePolicy {
    Deny,
    UserInitiatedOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardPolicyRejection {
    CapabilityDenied { operation: TextClipboardOperation },
    BackgroundDenied { operation: TextClipboardOperation },
    SecureFieldBlocked { operation: TextClipboardOperation },
}

impl ClipboardPolicyRejection {
    pub const fn error_kind(&self) -> TextClipboardErrorKind {
        match self {
            Self::SecureFieldBlocked { .. } => TextClipboardErrorKind::SecureFieldBlocked,
            Self::CapabilityDenied { .. } | Self::BackgroundDenied { .. } => {
                TextClipboardErrorKind::PolicyDenied
            }
        }
    }

    pub const fn operation(&self) -> TextClipboardOperation {
        match self {
            Self::CapabilityDenied { operation }
            | Self::BackgroundDenied { operation }
            | Self::SecureFieldBlocked { operation } => *operation,
        }
    }

    pub fn into_clipboard_error(self) -> TextClipboardError {
        TextClipboardError::new(self.error_kind(), self.operation())
    }
}
