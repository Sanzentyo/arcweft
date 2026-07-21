//! Closed authoring schemas owned by the dialogue domain.

mod control;
mod host;

pub use control::{DialogueControlProperty, DialogueRichTextControl};
pub use host::{DialogueHostEventKind, DialogueHostProperty};
