//! Takumi CSS/layout/stacking-scene adapter for Arcweft native UI.
//!
//! This crate deliberately uses Takumi as a CSS cascade, layout, and stacking
//! scene source, not as a CPU bitmap renderer. The primary output is
//! [`arcweft_render_wgpu::ui_scene::UiScene`] with direct wgpu primitives and
//! capture metadata that keeps Arcweft component, part, handler, semantic, and
//! Agent identity attached to the rendered coordinate space.

pub mod adapter;
pub mod cache;
pub mod capture;
pub mod diagnostic;
pub mod lowering;
pub mod metadata;
pub mod style;
pub mod text;

pub use adapter::{TakumiAdapter, TakumiAdapterInput, TakumiAdapterOutput};
pub use cache::{
    ImageRevision, RendererResourceRevision, StyleRevision, TakumiPaintCacheKey,
    TakumiSceneCacheKey, TextLayoutRevision, UiProgramRevision, ViewFragmentRevision, ViewportKey,
};
pub use capture::{TakumiCaptureFrame, TakumiCaptureRecord};
pub use diagnostic::{TakumiAdapterError, TakumiDiagnostic, TakumiDiagnosticCode};
pub use lowering::{
    DirectBackground, DirectBorder, DirectBoxPaint, DirectClip, DirectPaintCatalog,
    TakumiSceneInput, TakumiSceneLowerer, TakumiSceneOutput,
};
pub use metadata::{ArcweftNodeMetadata, TakumiMetadataEntry, TakumiMetadataMap, TakumiPath};
pub use style::{
    CssInvalidationClass, CssPropertyClass, DirectCssFeature, DirectCssSupport, TakumiCssBundle,
};
pub use text::{
    ArcweftGlyphRun, ArcweftInlineParticipant, ArcweftInlineParticipantKind,
    ArcweftTextLayoutBridge, InlineMeasuredSize,
};
