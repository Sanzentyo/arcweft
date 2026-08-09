//! Renderer-agnostic rich-text presentation data.
//!
//! This module intentionally stores only deterministic, serializable data.

use arcweft_presentation::fx::{FxApplication, FxTarget};
use serde::{Deserialize, Serialize};

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
    /// Inherit the host text-container/layout configuration.
    #[default]
    Auto,
    /// Looser Japanese punctuation pairing.
    Loose,
    /// Balanced narrative default.
    Normal,
    /// Stricter Japanese punctuation pairing.
    Strict,
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
    Selector { value: String },
    Color { value: [u8; 4] },
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruby_font_size: Option<Milli>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruby_gap: Option<Milli>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruby_overhang: Option<Milli>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruby_collision_gap: Option<Milli>,
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
            ruby_font_size: None,
            ruby_gap: None,
            ruby_overhang: None,
            ruby_collision_gap: None,
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
    pub target: FxTarget,
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
            target: FxTarget::Content,
        }
    }
}

/// Typed proxy metadata attached to a span of text presentation objects.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextObjectProxy {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<RichTextTextProxySchema>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration: Option<RichTextObjectProxyDeclaration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<Milli>,
    #[serde(default)]
    pub hit_test: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<RichTextTextProxyField>,
}

/// Source declaration that supplied rich-text object proxy defaults.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextObjectProxyDeclaration {
    pub struct_name: String,
    pub attribute: String,
}

/// Runtime-visible typed text-proxy schema selected by semantic checking.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextTextProxySchema {
    pub id: String,
    pub declaration: RichTextObjectProxyDeclaration,
    pub fields: Vec<RichTextTextProxyFieldSchema>,
}

/// Declaration-order field schema for one typed text proxy.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextTextProxyFieldSchema {
    pub id: u16,
    pub name: String,
    pub kind: RichTextTextProxyFieldKind,
    pub optional: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<RichTextTextProxyScalar>,
}

/// Closed field kind admitted by the runtime text-proxy boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextTextProxyFieldKind {
    Bool,
    Int,
    Milli,
    Ratio,
    Length,
    Angle,
    Duration,
    ClosedEnum {
        enum_id: String,
        variants: Vec<String>,
    },
    PublicId,
    Text,
    Color,
}

/// Declaration-order value for one typed text-proxy field.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextTextProxyField {
    pub id: u16,
    pub name: String,
    pub value: RichTextTextProxyScalar,
}

/// Closed runtime scalar algebra for typed text-proxy values.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextTextProxyScalar {
    Bool { value: bool },
    Int { value: i64 },
    Milli { value: Milli },
    Ratio { milli: u16 },
    Length { value: RichTextTextProxyLength },
    Angle { milli_degrees: i32 },
    Duration { millis: u64 },
    ClosedEnum { enum_id: String, variant: u16 },
    PublicId { value: String },
    Text { value: String },
    Color { value: crate::style::RichTextColor },
}

/// Fixed-point length retained without collapsing its authored semantic unit.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextTextProxyLength {
    pub milli: i32,
    pub unit: RichTextTextProxyLengthUnit,
}

/// Closed unit domain for typed text-proxy lengths.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextTextProxyLengthUnit {
    Px,
    Pt,
    Ch,
    Em,
}

/// Presentation metadata resolved for a text run or ruby annotation.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextPresentation {
    #[serde(default)]
    pub layout: Option<RichTextLayout>,
    #[serde(default)]
    pub transform: Option<RichTextTransform>,
    /// Typed applications evaluated by the shared presentation evaluator.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fx: Vec<FxApplication>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub object_proxies: Vec<RichTextObjectProxy>,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub oblique: Option<RichTextAngle>,
    #[serde(default)]
    pub opacity: Option<Milli>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
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
        self.fx.extend(other.fx);
        self.object_proxies.extend(other.object_proxies);
        self.italic |= other.italic;
        if other.oblique.is_some() {
            self.oblique = other.oblique;
        }
        if other.opacity.is_some() {
            self.opacity = other.opacity;
        }
        if other.layer.is_some() {
            self.layer = other.layer;
        }
        if other.z_index != 0 {
            self.z_index = other.z_index;
        }
    }
}
