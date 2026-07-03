//! Shader blend-mode classification for UI compositing.

use crate::ui_scene::UiBlendMode;

/// Blend modes implemented by the first compositor shader path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum UiBlendShaderMode {
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

/// Per-group blend pass contract consumed by `UiCompositor`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiBlendPassPlan {
    pub mode: UiBlendMode,
    pub shader_mode: UiBlendShaderMode,
    pub samples_backdrop: bool,
}

impl UiBlendPassPlan {
    pub fn from_mode(mode: UiBlendMode) -> Option<Self> {
        let shader_mode = UiBlendShaderMode::from_blend_mode(mode)?;
        Some(Self {
            mode,
            shader_mode,
            samples_backdrop: mode != UiBlendMode::Normal,
        })
    }

    pub const fn is_identity(self) -> bool {
        matches!(self.shader_mode, UiBlendShaderMode::Normal)
    }
}

impl UiBlendShaderMode {
    pub fn from_blend_mode(mode: UiBlendMode) -> Option<Self> {
        Some(match mode {
            UiBlendMode::Normal => Self::Normal,
            UiBlendMode::Multiply => Self::Multiply,
            UiBlendMode::Screen => Self::Screen,
            UiBlendMode::Overlay => Self::Overlay,
            UiBlendMode::Darken => Self::Darken,
            UiBlendMode::Lighten => Self::Lighten,
            UiBlendMode::ColorDodge => Self::ColorDodge,
            UiBlendMode::ColorBurn => Self::ColorBurn,
            UiBlendMode::HardLight => Self::HardLight,
            UiBlendMode::SoftLight => Self::SoftLight,
            UiBlendMode::Difference => Self::Difference,
            UiBlendMode::Exclusion => Self::Exclusion,
            UiBlendMode::PlusLighter => Self::PlusLighter,
            UiBlendMode::PlusDarker => Self::PlusDarker,
            UiBlendMode::Hue => Self::Hue,
            UiBlendMode::Saturation => Self::Saturation,
            UiBlendMode::Color => Self::Color,
            UiBlendMode::Luminosity => Self::Luminosity,
        })
    }

    pub const fn as_shader_u32(self) -> u32 {
        self as u32
    }
}

pub fn supported_blend_modes() -> &'static [UiBlendMode] {
    &[
        UiBlendMode::Normal,
        UiBlendMode::Multiply,
        UiBlendMode::Screen,
        UiBlendMode::Overlay,
        UiBlendMode::Darken,
        UiBlendMode::Lighten,
        UiBlendMode::ColorDodge,
        UiBlendMode::ColorBurn,
        UiBlendMode::HardLight,
        UiBlendMode::SoftLight,
        UiBlendMode::Difference,
        UiBlendMode::Exclusion,
        UiBlendMode::PlusLighter,
        UiBlendMode::PlusDarker,
        UiBlendMode::Hue,
        UiBlendMode::Saturation,
        UiBlendMode::Color,
        UiBlendMode::Luminosity,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_blend_mode_does_not_need_backdrop_sampling() {
        let plan = UiBlendPassPlan::from_mode(UiBlendMode::Normal).expect("normal is supported");
        assert!(plan.is_identity());
        assert!(!plan.samples_backdrop);
    }

    #[test]
    fn hsl_family_blends_are_shader_supported() {
        assert_eq!(
            UiBlendPassPlan::from_mode(UiBlendMode::Hue).map(|plan| plan.shader_mode),
            Some(UiBlendShaderMode::Hue)
        );
        assert_eq!(
            UiBlendPassPlan::from_mode(UiBlendMode::Luminosity).map(|plan| plan.shader_mode),
            Some(UiBlendShaderMode::Luminosity)
        );
    }
}
