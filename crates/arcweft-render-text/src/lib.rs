//! Sans I/O text source resolution for Arcweft players.
//!
//! This crate owns dialogue resolution, playback validation, and the canonical
//! post-resolution document. Shared authored and frame data live in
//! `arcweft-text-model`.

pub mod resolved_document;

mod resolve_frame;

pub use resolve_frame::{LineDisplayError, RuntimeLineContext, resolve_frame};
pub use resolved_document::{
    LanguageTag, ResolvedTextDocument, ResolvedTextRuby, ResolvedTextRun, ResolvedTextRunSource,
    ResolvedTextStyle, TextColor, TextDocumentRevision, TextFontFamily, TextResolveError,
    TextSlant, TextStyleCascade, TextWeight, resolve_document, resolve_document_with_source,
    resolve_stage_document,
};
