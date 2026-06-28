//! Renderer-facing UI scene primitives and compositing graph for direct wgpu rendering.
//!
//! The scene is produced after Arcweft style resolution, text layout, and optional
//! Takumi CSS/layout/stacking-scene lowering. It contains no OS/IME handles and
//! no CPU-raster surface fallback. `SharedRenderer` should draw these paint nodes
//! with persistent GPU buffers/textures/atlases and update paint-only fields
//! without recreating layout data.
//!
//! Direct primitives remain available through [`UiScene::contexts`] and
//! [`UiScene::primitives`]. Subtree effects such as CSS `filter`,
//! `backdrop-filter`, `mask`, `clip-path`, and `mix-blend-mode` are represented
//! by [`compositing::UiPaintNode`] so renderer work can introduce offscreen
//! passes without changing the scene contract again.

mod core;

pub mod compositing;

pub use compositing::{
    UiBlendMode, UiClipPath, UiCompositingEffectClass, UiCompositingEffects, UiCompositingGroup,
    UiCompositingRequirements, UiFillRule, UiFilter, UiFilterList, UiIsolation, UiLength, UiMask,
    UiMaskImage, UiMaskPosition, UiMaskRepeat, UiMaskSize, UiPaintNode, UiPoint, UiShapeRadius,
};
pub use core::{
    UiAffine2, UiBorder, UiCaretPrimitive, UiClip, UiColorRgba8, UiCompositionUnderline,
    UiGlyphRun, UiGradientStop, UiImagePrimitive, UiLinearGradient, UiPrimitive, UiPrimitiveRange,
    UiRoundedRect, UiScene, UiSceneContext, UiSelectionPrimitive, UiSolidRect,
    UiTextFieldSceneStyle, UiUnderlineStyle,
};
