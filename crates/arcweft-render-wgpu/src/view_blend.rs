//! Shader blend-mode classification for UI compositing.

use crate::view_scene::ViewBlendMode;

/// Blend modes implemented by the first compositor shader path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ViewBlendShaderMode {
    Normal = 0,
    Multiply = 1,
    Screen = 2,
    Overlay = 3,
    Darken = 4,
    Lighten = 5,
    ColorDodge = 6,
    ColorBurn = 7,
    HardLight = 8,
    SoftLight = 9,
    Difference = 10,
    Exclusion = 11,
    PlusLighter = 12,
    PlusDarker = 13,
    Hue = 14,
    Saturation = 15,
    Color = 16,
    Luminosity = 17,
}

/// Per-group blend pass contract consumed by `ViewCompositor`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewBlendPassPlan {
    pub mode: ViewBlendMode,
    pub shader_mode: ViewBlendShaderMode,
    pub samples_backdrop: bool,
}

impl ViewBlendPassPlan {
    pub fn from_mode(mode: ViewBlendMode) -> Option<Self> {
        let shader_mode = ViewBlendShaderMode::from_blend_mode(mode)?;
        Some(Self {
            mode,
            shader_mode,
            samples_backdrop: mode != ViewBlendMode::Normal,
        })
    }

    pub const fn is_identity(self) -> bool {
        matches!(self.shader_mode, ViewBlendShaderMode::Normal)
    }
}

impl ViewBlendShaderMode {
    pub fn from_blend_mode(mode: ViewBlendMode) -> Option<Self> {
        Some(match mode {
            ViewBlendMode::Normal => Self::Normal,
            ViewBlendMode::Multiply => Self::Multiply,
            ViewBlendMode::Screen => Self::Screen,
            ViewBlendMode::Overlay => Self::Overlay,
            ViewBlendMode::Darken => Self::Darken,
            ViewBlendMode::Lighten => Self::Lighten,
            ViewBlendMode::ColorDodge => Self::ColorDodge,
            ViewBlendMode::ColorBurn => Self::ColorBurn,
            ViewBlendMode::HardLight => Self::HardLight,
            ViewBlendMode::SoftLight => Self::SoftLight,
            ViewBlendMode::Difference => Self::Difference,
            ViewBlendMode::Exclusion => Self::Exclusion,
            ViewBlendMode::PlusLighter => Self::PlusLighter,
            ViewBlendMode::PlusDarker => Self::PlusDarker,
            ViewBlendMode::Hue => Self::Hue,
            ViewBlendMode::Saturation => Self::Saturation,
            ViewBlendMode::Color => Self::Color,
            ViewBlendMode::Luminosity => Self::Luminosity,
        })
    }

    pub const fn as_shader_u32(self) -> u32 {
        self as u32
    }
}

pub fn supported_blend_modes() -> &'static [ViewBlendMode] {
    &[
        ViewBlendMode::Normal,
        ViewBlendMode::Multiply,
        ViewBlendMode::Screen,
        ViewBlendMode::Overlay,
        ViewBlendMode::Darken,
        ViewBlendMode::Lighten,
        ViewBlendMode::ColorDodge,
        ViewBlendMode::ColorBurn,
        ViewBlendMode::HardLight,
        ViewBlendMode::SoftLight,
        ViewBlendMode::Difference,
        ViewBlendMode::Exclusion,
        ViewBlendMode::PlusLighter,
        ViewBlendMode::PlusDarker,
        ViewBlendMode::Hue,
        ViewBlendMode::Saturation,
        ViewBlendMode::Color,
        ViewBlendMode::Luminosity,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_blend_mode_does_not_need_backdrop_sampling() {
        let plan =
            ViewBlendPassPlan::from_mode(ViewBlendMode::Normal).expect("normal is supported");
        assert!(plan.is_identity());
        assert!(!plan.samples_backdrop);
    }

    #[test]
    fn hsl_family_blends_are_shader_supported() {
        assert_eq!(
            ViewBlendPassPlan::from_mode(ViewBlendMode::Hue).map(|plan| plan.shader_mode),
            Some(ViewBlendShaderMode::Hue)
        );
        assert_eq!(
            ViewBlendPassPlan::from_mode(ViewBlendMode::Luminosity).map(|plan| plan.shader_mode),
            Some(ViewBlendShaderMode::Luminosity)
        );
    }
}
