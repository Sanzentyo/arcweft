//! Renderer-agnostic rich-text presentation data.
//!
//! This module intentionally stores only deterministic, serializable data. The
//! native/browser renderers resolve effect IDs, shader IDs, stateful classes,
//! and mutable closures through their own registries.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Fixed-point scalar for stable snapshots and Eq-friendly display sidecars.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Milli(pub i32);

impl Milli {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(1000);

    pub const fn from_units(value: i32) -> Self {
        Self(value.saturating_mul(1000))
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn as_f32(self) -> f32 {
        self.0 as f32 / 1000.0
    }
}

/// Two-dimensional fixed-point vector.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextVec2 {
    pub x: Milli,
    pub y: Milli,
}

impl RichTextVec2 {
    pub const ZERO: Self = Self {
        x: Milli::ZERO,
        y: Milli::ZERO,
    };

    pub const ONE: Self = Self {
        x: Milli::ONE,
        y: Milli::ONE,
    };

    pub const fn new(x: Milli, y: Milli) -> Self {
        Self { x, y }
    }
}

/// Fixed-point angle in degrees.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextAngle {
    pub degrees: Milli,
}

impl RichTextAngle {
    pub const ZERO: Self = Self {
        degrees: Milli::ZERO,
    };

    pub fn as_degrees_f32(self) -> f32 {
        self.degrees.as_f32()
    }
}

/// Text writing mode requested by a rich-text span.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextWritingMode {
    #[default]
    HorizontalTb,
    VerticalRl,
    VerticalLr,
}

/// Inline direction hint.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextInlineDirection {
    #[default]
    Auto,
    Ltr,
    Rtl,
}

/// Latin glyph orientation in vertical text.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextVerticalLatinMode {
    #[default]
    Mixed,
    Upright,
    Sideways,
}

/// Ruby placement hint.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextRubyPosition {
    #[default]
    Auto,
    Over,
    Under,
    InterCharacter,
}

/// Authored JLREQ strictness preset for vertical column planning.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextJlreqStrictness {
    /// Inherit the host textbox/layout configuration.
    #[default]
    Auto,
    /// Looser Japanese punctuation pairing.
    Loose,
    /// Balanced narrative default.
    Normal,
    /// Stricter Japanese punctuation pairing.
    Strict,
}

/// Effect target granularity.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextEffectTarget {
    Document,
    Line,
    Sentence,
    #[default]
    Run,
    Glyph,
    TextBox,
    Screen,
}

/// Execution phase for a rich-text effect descriptor.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextEffectPhase {
    BeforeLayout,
    LayoutTransform,
    #[default]
    GlyphTransform,
    GlyphColor,
    GlyphMask,
    RunOffscreenPass,
    PostProcess,
    HostEvent,
}

/// Renderer-side state sharing scope for one effect descriptor.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextStateScope {
    Glyph,
    #[default]
    Run,
    Line,
    Sentence,
    Paragraph,
    Document,
    DialogueLine,
    Speaker,
    Window,
    Global,
}

/// Transform origin for run/glyph placement.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextTransformOrigin {
    #[default]
    BaselineStart,
    BaselineCenter,
    Center,
    GlyphCenter,
}

/// Serializable parameter value for renderer-resolved effects.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextParam {
    Bool { value: bool },
    Int { value: i64 },
    Milli { value: Milli },
    Vec2 { value: RichTextVec2 },
    Text { value: String },
    Raw { value: String },
    Selector { value: String },
    Expr { source: String },
}

/// Layout directive applied while resolving visual text.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextLayout {
    #[serde(default)]
    pub writing_mode: RichTextWritingMode,
    #[serde(default)]
    pub direction: RichTextInlineDirection,
    #[serde(default)]
    pub vertical_latin: RichTextVerticalLatinMode,
    #[serde(default)]
    pub ruby_position: RichTextRubyPosition,
    #[serde(default)]
    pub jlreq_strictness: RichTextJlreqStrictness,
    #[serde(default = "default_column_gap")]
    pub column_gap: Milli,
}

const fn default_column_gap() -> Milli {
    Milli(8000)
}

impl Default for RichTextLayout {
    fn default() -> Self {
        Self {
            writing_mode: RichTextWritingMode::HorizontalTb,
            direction: RichTextInlineDirection::Auto,
            vertical_latin: RichTextVerticalLatinMode::Mixed,
            ruby_position: RichTextRubyPosition::Auto,
            jlreq_strictness: RichTextJlreqStrictness::Auto,
            column_gap: default_column_gap(),
        }
    }
}

/// Transform directive applied to run/glyph placement.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextTransform {
    #[serde(default)]
    pub translate: RichTextVec2,
    #[serde(default)]
    pub rotate: RichTextAngle,
    #[serde(default = "one_vec2")]
    pub scale: RichTextVec2,
    #[serde(default)]
    pub skew: RichTextVec2,
    #[serde(default)]
    pub origin: RichTextTransformOrigin,
    #[serde(default)]
    pub target: RichTextEffectTarget,
}

const fn one_vec2() -> RichTextVec2 {
    RichTextVec2::ONE
}

impl Default for RichTextTransform {
    fn default() -> Self {
        Self {
            translate: RichTextVec2::ZERO,
            rotate: RichTextAngle::ZERO,
            scale: RichTextVec2::ONE,
            skew: RichTextVec2::ZERO,
            origin: RichTextTransformOrigin::BaselineStart,
            target: RichTextEffectTarget::Run,
        }
    }
}

/// Serializable description of an effect resolved by renderer adapters.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextEffectDescriptor {
    pub id: String,
    #[serde(default)]
    pub params: BTreeMap<String, RichTextParam>,
    #[serde(default)]
    pub target: RichTextEffectTarget,
    #[serde(default)]
    pub phase: RichTextEffectPhase,
    #[serde(default)]
    pub state_scope: RichTextStateScope,
}

/// Shader/filter effect reference. Actual shader code belongs to host registry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextShaderRef {
    pub id: String,
    #[serde(default)]
    pub params: BTreeMap<String, RichTextParam>,
    #[serde(default)]
    pub phase: RichTextEffectPhase,
}

/// Presentation metadata resolved for a text run or ruby annotation.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextPresentation {
    #[serde(default)]
    pub layout: Option<RichTextLayout>,
    #[serde(default)]
    pub transform: Option<RichTextTransform>,
    #[serde(default)]
    pub effects: Vec<RichTextEffectDescriptor>,
    #[serde(default)]
    pub shaders: Vec<RichTextShaderRef>,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub oblique: Option<RichTextAngle>,
    #[serde(default)]
    pub opacity: Option<Milli>,
    #[serde(default)]
    pub z_index: i16,
}

impl RichTextPresentation {
    /// Merges another presentation, with later scalar directives overriding.
    pub fn merge(&mut self, other: Self) {
        if other.layout.is_some() {
            self.layout = other.layout;
        }
        if other.transform.is_some() {
            self.transform = other.transform;
        }
        self.effects.extend(other.effects);
        self.shaders.extend(other.shaders);
        self.italic |= other.italic;
        if other.oblique.is_some() {
            self.oblique = other.oblique;
        }
        if other.opacity.is_some() {
            self.opacity = other.opacity;
        }
        if other.z_index != 0 {
            self.z_index = other.z_index;
        }
    }
}

/// Parses a numeric authoring token into milli-units, stripping common suffixes.
pub fn parse_milli_token(value: &str) -> Milli {
    let trimmed = value
        .trim()
        .trim_end_matches("px")
        .trim_end_matches("deg")
        .trim_end_matches("ch");
    parse_decimal_milli(trimmed).unwrap_or(Milli::ZERO)
}

/// Parses a decimal string into milli-units.
pub fn parse_decimal_milli(value: &str) -> Option<Milli> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let sign = if value.starts_with('-') { -1 } else { 1 };
    let unsigned = value.trim_start_matches(['-', '+']);
    let (whole, frac) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let whole = whole.parse::<i32>().ok()?;
    let frac = frac
        .chars()
        .take(3)
        .chain(std::iter::repeat('0'))
        .take(3)
        .collect::<String>()
        .parse::<i32>()
        .ok()?;
    Some(Milli(
        sign * whole.saturating_mul(1000).saturating_add(frac),
    ))
}
