//! Renderer-facing View scene primitives and compositing graph for direct wgpu rendering.
//!
//! The scene is produced after Arcweft style resolution, text layout, and optional
//! Takumi CSS/layout/stacking-scene lowering. It contains no OS/IME handles and
//! no CPU-raster surface fallback. `SharedRenderer` should draw these paint nodes
//! with persistent GPU buffers/textures/atlases and update paint-only fields
//! without recreating layout data.
//!
//! Direct primitives remain available through [`ViewScene::contexts`] and
//! [`ViewScene::primitives`]. Subtree effects such as CSS `filter`,
//! `backdrop-filter`, `mask`, `clip-path`, and `mix-blend-mode` are represented
//! by [`compositing::ViewPaintNode`] so renderer work can introduce offscreen
//! passes without changing the scene contract again.

mod core;

pub use arcweft_glyphon::PreparedTextId;

pub mod compositing;

pub use compositing::{
    ViewBlendMode, ViewBoxShadow, ViewBoxShadowCorner, ViewBoxShadowCornerRadius,
    ViewBoxShadowKind, ViewBoxShadowList, ViewBoxShadowRadii, ViewBoxShadowRadiusAxis,
    ViewClipPath, ViewCompositingEffectClass, ViewCompositingEffects, ViewCompositingGroup,
    ViewCompositingRequirements, ViewElementMaskSource, ViewFillRule, ViewFilter, ViewFilterList,
    ViewIsolation, ViewLength, ViewMask, ViewMaskGradient, ViewMaskImage, ViewMaskPosition,
    ViewMaskRepeat, ViewMaskSize, ViewPaintNode, ViewPoint, ViewShapeRadius,
};
pub use core::{
    ViewAffine2D, ViewBorder, ViewClip, ViewColorRgba8, ViewCornerRadii, ViewCornerRadius,
    ViewGradientStop, ViewImagePrimitive, ViewImageUvRect, ViewLinearGradient, ViewPrimitive,
    ViewPrimitiveRange, ViewRoundedRect, ViewScene, ViewSceneContext, ViewSolidRect,
    ViewSurfaceBackground, ViewSurfaceBorder, ViewSurfaceClip, ViewSurfacePaint, ViewTextPrimitive,
};
