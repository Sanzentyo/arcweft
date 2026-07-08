//! Takumi CSS/layout/stacking-scene adapter for Arcweft native View.
//!
//! This crate deliberately uses Takumi as a CSS cascade, layout, and stacking
//! scene source, not as a CPU bitmap renderer. The primary output is
//! [`arcweft_render_wgpu::view_scene::ViewScene`] with direct wgpu primitives,
//! compositing groups, and capture metadata that keeps Arcweft view, part,
//! handler, semantic, and Agent identity attached to the rendered coordinate
//! space.

pub mod adapter;
pub mod cache;
pub mod capture;
pub mod coverage;
pub mod diagnostic;
pub mod evidence;
pub mod lowering;
pub mod metadata;
pub mod paint_extractor;
pub mod style;
pub mod text;

pub use adapter::{TakumiAdapter, TakumiAdapterInput, TakumiAdapterOutput};
pub use cache::{
    ImageRevision, RendererResourceRevision, StyleRevision, TakumiPaintCacheKey,
    TakumiSceneCacheKey, TextLayoutRevision, ViewFragmentRevision, ViewProgramRevision,
    ViewportKey,
};
pub use capture::{
    TakumiCaptureFrame, TakumiCaptureRecord, TakumiCompositingCaptureRecord,
    TakumiCompositingGroupId, TakumiEffectOutsets, TakumiPaintNodeId,
};
pub use coverage::{
    CSS_COVERAGE_MATRIX, CSS_LAYOUT_CASCADE_EVIDENCE_SCHEMA_VERSION, CssAtRuleCoverage,
    CssCascadeLayer, CssCascadePriority, CssComputedStyleEvidence, CssCoverageFeature,
    CssCoverageMatrixRow, CssCoverageReport, CssCoverageStatus, CssDeclarationCoverage,
    CssLayoutBoxEvidence, CssMatchedDeclaration, CssOverflowEvidence, CssSelectorCoverage,
    CssSelectorWinnerEvidence, CssSpecificity, winning_declaration,
};
pub use diagnostic::{TakumiAdapterError, TakumiDiagnostic, TakumiDiagnosticCode};
pub use evidence::{COMPOSITING_EVIDENCE_SCHEMA_VERSION, capture_frame_to_json};
pub use lowering::{
    DirectBoxPaint, DirectPaintCatalog, TakumiCompositingStyle, TakumiCompositingStyleCatalog,
    TakumiSceneInput, TakumiSceneLowerer, TakumiSceneOutput,
};
pub use metadata::{ArcweftNodeMetadata, TakumiMetadataEntry, TakumiMetadataMap, TakumiPath};
pub use paint_extractor::{
    ComputedDirectPaintExtractor, ComputedDirectPaintFrame, ComputedDirectPaintInput,
    DirectPaintEvidenceFrame, DirectPaintEvidenceRecord, DirectPaintLayerEvidence,
    DirectPaintLayerKind, DirectPaintResourceRequirement, DirectPaintResourceTable,
    DirectPaintSource,
};
pub use style::{
    CssInvalidationClass, CssPropertyClass, DirectCssFeature, DirectCssSupport, TakumiCssBundle,
};
pub use text::{
    ArcweftGlyphRun, ArcweftInlineParticipant, ArcweftInlineParticipantKind,
    ArcweftTextLayoutBridge, InlineMeasuredSize,
};
