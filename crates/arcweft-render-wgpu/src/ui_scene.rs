//! Renderer-facing UI scene primitives for direct wgpu rendering.
//!
//! The scene is produced after Arcweft style resolution, text layout, and optional
//! Takumi CSS/layout/stacking-scene lowering. It contains no OS/IME handles and
//! no CPU-raster surface fallback. `SharedRenderer` should draw these primitives
//! with persistent GPU buffers/textures/atlases and update paint-only fields
//! without recreating layout data.

use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiScene {
    viewport_width: f32,
    viewport_height: f32,
    contexts: Vec<UiSceneContext>,
    primitives: Vec<UiPrimitive>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiSceneContext {
    pub transform: UiAffine2,
    pub opacity: f32,
    pub clip: Option<UiClip>,
    pub primitive_range: UiPrimitiveRange,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UiPrimitiveRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiPrimitive {
    SolidRect(UiSolidRect),
    RoundedRect(UiRoundedRect),
    Border(UiBorder),
    LinearGradient(UiLinearGradient),
    Image(UiImagePrimitive),
    GlyphRun(UiGlyphRun),
    Selection(UiSelectionPrimitive),
    Caret(UiCaretPrimitive),
    CompositionUnderline(UiCompositionUnderline),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiAffine2 {
    pub m11: f32,
    pub m12: f32,
    pub m21: f32,
    pub m22: f32,
    pub tx: f32,
    pub ty: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiClip {
    Rect(HitRect),
    RoundedRect { bounds: HitRect, radius: f32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiColorRgba8 {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiSolidRect {
    pub bounds: HitRect,
    pub color: UiColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiRoundedRect {
    pub bounds: HitRect,
    pub radius: f32,
    pub color: UiColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiBorder {
    pub bounds: HitRect,
    pub radius: f32,
    pub width: f32,
    pub color: UiColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiLinearGradient {
    pub bounds: HitRect,
    pub angle_degrees: f32,
    pub stops: Vec<UiGradientStop>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiGradientStop {
    pub offset: f32,
    pub color: UiColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiImagePrimitive {
    pub resource_index: u32,
    pub bounds: HitRect,
    pub opacity: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiGlyphRun {
    pub run_index: u32,
    pub bounds: HitRect,
    pub color: UiColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiSelectionPrimitive {
    pub target: Option<InteractionTarget>,
    pub bounds: HitRect,
    pub color: UiColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiCaretPrimitive {
    pub target: Option<InteractionTarget>,
    pub bounds: HitRect,
    pub color: UiColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiCompositionUnderline {
    pub target: Option<InteractionTarget>,
    pub bounds: HitRect,
    pub color: UiColorRgba8,
    pub thickness: f32,
    pub style: UiUnderlineStyle,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UiUnderlineStyle {
    #[default]
    Solid,
    Dotted,
    Dashed,
}

impl Default for UiAffine2 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl UiAffine2 {
    pub const IDENTITY: Self = Self {
        m11: 1.0,
        m12: 0.0,
        m21: 0.0,
        m22: 1.0,
        tx: 0.0,
        ty: 0.0,
    };
}

impl UiScene {
    pub fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            viewport_width,
            viewport_height,
            contexts: Vec::new(),
            primitives: Vec::new(),
        }
    }

    pub fn push_context(&mut self, context: UiSceneContext) {
        self.contexts.push(context);
    }

    pub fn push_primitive(&mut self, primitive: UiPrimitive) {
        self.primitives.push(primitive);
    }

    pub const fn viewport_width(&self) -> f32 {
        self.viewport_width
    }

    pub const fn viewport_height(&self) -> f32 {
        self.viewport_height
    }

    pub fn contexts(&self) -> &[UiSceneContext] {
        &self.contexts
    }

    pub fn primitives(&self) -> &[UiPrimitive] {
        &self.primitives
    }
}

#[cfg(test)]
mod tests {
    use super::{
        UiAffine2, UiColorRgba8, UiPrimitive, UiPrimitiveRange, UiScene, UiSceneContext,
        UiSolidRect,
    };
    use arcweft_presentation::hit::HitRect;

    #[test]
    fn ui_scene_preserves_context_and_primitive_order() {
        let mut scene = UiScene::new(320.0, 180.0);
        scene.push_primitive(UiPrimitive::SolidRect(UiSolidRect {
            bounds: HitRect::new(0.0, 0.0, 10.0, 10.0),
            color: UiColorRgba8 {
                red: 1,
                green: 2,
                blue: 3,
                alpha: 255,
            },
        }));
        scene.push_context(UiSceneContext {
            transform: UiAffine2::IDENTITY,
            opacity: 1.0,
            clip: None,
            primitive_range: UiPrimitiveRange { start: 0, end: 1 },
        });

        assert!((scene.viewport_width() - 320.0).abs() < f32::EPSILON);
        assert!((scene.viewport_height() - 180.0).abs() < f32::EPSILON);
        assert_eq!(scene.primitives().len(), 1);
        assert_eq!(
            scene.contexts()[0].primitive_range,
            UiPrimitiveRange { start: 0, end: 1 }
        );
    }
}
