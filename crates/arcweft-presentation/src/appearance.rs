//! Sans I/O presentation appearance data shared by View, text, renderer adapters, and replay.
//!
//! This module intentionally contains no platform API calls. Native and Web hosts
//! resolve OS/browser preferences into `PresentationEnvironment` and pass that
//! pure data into Arcweft presentation and View evaluation.

use arcweft_id::PublicId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorScheme {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorSchemePreference {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContrastPreference {
    #[default]
    Standard,
    More,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct TextScaleMilli(pub u16);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentRevision(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationEnvironment {
    color_scheme: ColorScheme,
    contrast: ContrastPreference,
    reduce_motion: bool,
    text_scale: TextScaleMilli,
    locale: Option<PublicId>,
    revision: EnvironmentRevision,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemColor {
    Canvas,
    CanvasText,
    Surface,
    SurfaceText,
    RaisedSurface,
    MutedText,
    Border,
    Accent,
    AccentText,
    FocusRing,
    Selection,
    SelectionText,
    Danger,
    Warning,
    Success,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct PresentationColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SystemPalette {
    pub canvas: PresentationColor,
    pub canvas_text: PresentationColor,
    pub surface: PresentationColor,
    pub surface_text: PresentationColor,
    pub raised_surface: PresentationColor,
    pub muted_text: PresentationColor,
    pub border: PresentationColor,
    pub accent: PresentationColor,
    pub accent_text: PresentationColor,
    pub focus_ring: PresentationColor,
    pub selection: PresentationColor,
    pub selection_text: PresentationColor,
    pub danger: PresentationColor,
    pub warning: PresentationColor,
    pub success: PresentationColor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SystemPaletteSet {
    pub light: SystemPalette,
    pub dark: SystemPalette,
}

impl Default for TextScaleMilli {
    fn default() -> Self {
        Self::ONE
    }
}

impl TextScaleMilli {
    pub const ONE: Self = Self(1_000);

    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u16 {
        self.0
    }
}

impl PresentationEnvironment {
    pub fn new(color_scheme: ColorScheme) -> Self {
        Self {
            color_scheme,
            contrast: ContrastPreference::Standard,
            reduce_motion: false,
            text_scale: TextScaleMilli::ONE,
            locale: None,
            revision: EnvironmentRevision::default(),
        }
    }

    #[must_use]
    pub const fn with_contrast(mut self, contrast: ContrastPreference) -> Self {
        self.contrast = contrast;
        self
    }

    #[must_use]
    pub const fn with_reduce_motion(mut self, reduce_motion: bool) -> Self {
        self.reduce_motion = reduce_motion;
        self
    }

    #[must_use]
    pub const fn with_text_scale(mut self, text_scale: TextScaleMilli) -> Self {
        self.text_scale = text_scale;
        self
    }

    #[must_use]
    pub fn with_locale(mut self, locale: PublicId) -> Self {
        self.locale = Some(locale);
        self
    }

    #[must_use]
    pub const fn with_revision(mut self, revision: EnvironmentRevision) -> Self {
        self.revision = revision;
        self
    }

    pub const fn color_scheme(&self) -> ColorScheme {
        self.color_scheme
    }

    pub const fn contrast(&self) -> ContrastPreference {
        self.contrast
    }

    pub const fn reduce_motion(&self) -> bool {
        self.reduce_motion
    }

    pub const fn text_scale(&self) -> TextScaleMilli {
        self.text_scale
    }

    pub const fn locale(&self) -> Option<&PublicId> {
        self.locale.as_ref()
    }

    pub const fn revision(&self) -> EnvironmentRevision {
        self.revision
    }
}

impl PresentationColor {
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self::rgba(red, green, blue, 255)
    }

    /// Linearly interpolates channels with a clamped thousandths progress.
    #[must_use]
    pub fn lerp(self, target: Self, progress_milli: u16) -> Self {
        let progress = i32::from(progress_milli.min(1_000));
        Self {
            red: lerp_channel(self.red, target.red, progress),
            green: lerp_channel(self.green, target.green, progress),
            blue: lerp_channel(self.blue, target.blue, progress),
            alpha: lerp_channel(self.alpha, target.alpha, progress),
        }
    }
}

impl SystemColor {
    pub const ALL: &'static [Self] = &[
        Self::Canvas,
        Self::CanvasText,
        Self::Surface,
        Self::SurfaceText,
        Self::RaisedSurface,
        Self::MutedText,
        Self::Border,
        Self::Accent,
        Self::AccentText,
        Self::FocusRing,
        Self::Selection,
        Self::SelectionText,
        Self::Danger,
        Self::Warning,
        Self::Success,
    ];

    /// Canonical native Style enum shorthand without the leading dot.
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Canvas => "Canvas",
            Self::CanvasText => "CanvasText",
            Self::Surface => "Surface",
            Self::SurfaceText => "SurfaceText",
            Self::RaisedSurface => "RaisedSurface",
            Self::MutedText => "MutedText",
            Self::Border => "Border",
            Self::Accent => "Accent",
            Self::AccentText => "AccentText",
            Self::FocusRing => "FocusRing",
            Self::Selection => "Selection",
            Self::SelectionText => "SelectionText",
            Self::Danger => "Danger",
            Self::Warning => "Warning",
            Self::Success => "Success",
        }
    }

    pub fn from_source_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|role| role.source_name() == value)
    }
}

fn lerp_channel(source: u8, target: u8, progress_milli: i32) -> u8 {
    let source = i32::from(source);
    let delta = i32::from(target) - source;
    let value = source + (delta * progress_milli + 500) / 1_000;
    u8::try_from(value.clamp(0, 255)).unwrap_or_default()
}

impl SystemPalette {
    pub const fn color(self, role: SystemColor) -> PresentationColor {
        match role {
            SystemColor::Canvas => self.canvas,
            SystemColor::CanvasText => self.canvas_text,
            SystemColor::Surface => self.surface,
            SystemColor::SurfaceText => self.surface_text,
            SystemColor::RaisedSurface => self.raised_surface,
            SystemColor::MutedText => self.muted_text,
            SystemColor::Border => self.border,
            SystemColor::Accent => self.accent,
            SystemColor::AccentText => self.accent_text,
            SystemColor::FocusRing => self.focus_ring,
            SystemColor::Selection => self.selection,
            SystemColor::SelectionText => self.selection_text,
            SystemColor::Danger => self.danger,
            SystemColor::Warning => self.warning,
            SystemColor::Success => self.success,
        }
    }
}

impl SystemPaletteSet {
    pub const ENGINE_DEFAULT: Self = Self {
        light: SystemPalette {
            canvas: PresentationColor::rgb(0xF7, 0xF8, 0xFA),
            canvas_text: PresentationColor::rgb(0x18, 0x1B, 0x20),
            surface: PresentationColor::rgb(0xFF, 0xFF, 0xFF),
            surface_text: PresentationColor::rgb(0x18, 0x1B, 0x20),
            raised_surface: PresentationColor::rgb(0xFF, 0xFF, 0xFF),
            muted_text: PresentationColor::rgb(0x62, 0x6A, 0x75),
            border: PresentationColor::rgb(0xD7, 0xDC, 0xE4),
            accent: PresentationColor::rgb(0x25, 0x63, 0xEB),
            accent_text: PresentationColor::rgb(0xFF, 0xFF, 0xFF),
            focus_ring: PresentationColor::rgb(0x1D, 0x4E, 0xD8),
            selection: PresentationColor::rgb(0xBF, 0xDB, 0xFE),
            selection_text: PresentationColor::rgb(0x0B, 0x12, 0x20),
            danger: PresentationColor::rgb(0xB4, 0x23, 0x18),
            warning: PresentationColor::rgb(0xB5, 0x47, 0x08),
            success: PresentationColor::rgb(0x16, 0x6B, 0x3A),
        },
        dark: SystemPalette {
            canvas: PresentationColor::rgb(0x0D, 0x11, 0x17),
            canvas_text: PresentationColor::rgb(0xF0, 0xF3, 0xF6),
            surface: PresentationColor::rgb(0x16, 0x1B, 0x22),
            surface_text: PresentationColor::rgb(0xF0, 0xF3, 0xF6),
            raised_surface: PresentationColor::rgb(0x21, 0x26, 0x2D),
            muted_text: PresentationColor::rgb(0x9D, 0xA7, 0xB3),
            border: PresentationColor::rgb(0x30, 0x36, 0x3D),
            accent: PresentationColor::rgb(0x58, 0xA6, 0xFF),
            accent_text: PresentationColor::rgb(0x07, 0x11, 0x1F),
            focus_ring: PresentationColor::rgb(0x79, 0xC0, 0xFF),
            selection: PresentationColor::rgb(0x1F, 0x6F, 0xB2),
            selection_text: PresentationColor::rgb(0xFF, 0xFF, 0xFF),
            danger: PresentationColor::rgb(0xFF, 0x7B, 0x72),
            warning: PresentationColor::rgb(0xE3, 0xB3, 0x41),
            success: PresentationColor::rgb(0x56, 0xD3, 0x64),
        },
    };

    pub const fn palette(self, scheme: ColorScheme) -> SystemPalette {
        match scheme {
            ColorScheme::Light => self.light,
            ColorScheme::Dark => self.dark,
        }
    }

    pub const fn color(self, scheme: ColorScheme, role: SystemColor) -> PresentationColor {
        self.palette(scheme).color(role)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ColorScheme, ContrastPreference, EnvironmentRevision, PresentationEnvironment, SystemColor,
        SystemPaletteSet, TextScaleMilli,
    };
    use arcweft_id::PublicId;

    #[test]
    fn presentation_environment_carries_accessibility_and_locale_inputs() {
        let locale = PublicId::try_new("locale.ja_jp").expect("locale id");
        let environment = PresentationEnvironment::new(ColorScheme::Dark)
            .with_contrast(ContrastPreference::More)
            .with_reduce_motion(true)
            .with_text_scale(TextScaleMilli::new(1_250))
            .with_locale(locale.clone())
            .with_revision(EnvironmentRevision(7));

        assert_eq!(environment.color_scheme(), ColorScheme::Dark);
        assert_eq!(environment.contrast(), ContrastPreference::More);
        assert!(environment.reduce_motion());
        assert_eq!(environment.text_scale().value(), 1_250);
        assert_eq!(environment.locale(), Some(&locale));
        assert_eq!(environment.revision(), EnvironmentRevision(7));
    }

    #[test]
    fn default_system_palette_resolves_scheme_specific_roles() {
        let light = SystemPaletteSet::ENGINE_DEFAULT.color(ColorScheme::Light, SystemColor::Canvas);
        let dark = SystemPaletteSet::ENGINE_DEFAULT.color(ColorScheme::Dark, SystemColor::Canvas);

        assert_ne!(light, dark);
        assert_eq!(light.alpha, 255);
        assert_eq!(dark.alpha, 255);
    }
}
