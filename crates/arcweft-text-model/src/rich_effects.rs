//! Renderer-agnostic rich-text presentation data.
//!
//! This module intentionally stores only deterministic, serializable data.

use arcweft_presentation::fx::{FxApplication, FxTarget};
use arcweft_primitives::CheckedLength;
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

/// Sparse authored layout contribution. `None` inherits the active value;
/// `Some` applies even when its value equals the resolved layout's default.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RichTextLayoutUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub writing_mode: Option<RichTextWritingMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<RichTextInlineDirection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical_latin: Option<RichTextVerticalLatinMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruby_position: Option<RichTextRubyPosition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jlreq_strictness: Option<RichTextJlreqStrictness>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column_gap: Option<CheckedLength>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruby_font_size: Option<CheckedLength>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruby_gap: Option<CheckedLength>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruby_overhang: Option<CheckedLength>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruby_collision_gap: Option<CheckedLength>,
}

impl RichTextLayoutUpdate {
    /// Whether this contribution leaves every inherited property untouched.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Applies only present properties, in cascade order. The resolved state
    /// owns defaults; authored contributions never infer presence from them.
    pub fn apply_to(&self, layout: &mut RichTextLayout) {
        if let Some(value) = self.writing_mode {
            layout.writing_mode = value;
        }
        if let Some(value) = self.direction {
            layout.direction = value;
        }
        if let Some(value) = self.vertical_latin {
            layout.vertical_latin = value;
        }
        if let Some(value) = self.ruby_position {
            layout.ruby_position = value;
        }
        if let Some(value) = self.jlreq_strictness {
            layout.jlreq_strictness = value;
        }
        if let Some(value) = self.column_gap {
            layout.column_gap = value;
        }
        if let Some(value) = self.ruby_font_size {
            layout.ruby_font_size = Some(value);
        }
        if let Some(value) = self.ruby_gap {
            layout.ruby_gap = Some(value);
        }
        if let Some(value) = self.ruby_overhang {
            layout.ruby_overhang = Some(value);
        }
        if let Some(value) = self.ruby_collision_gap {
            layout.ruby_collision_gap = Some(value);
        }
    }
}

/// Resolved layout state after authored contributions have been applied.
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
    pub column_gap: CheckedLength,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruby_font_size: Option<CheckedLength>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruby_gap: Option<CheckedLength>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruby_overhang: Option<CheckedLength>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruby_collision_gap: Option<CheckedLength>,
}

const fn default_column_gap() -> CheckedLength {
    CheckedLength::px(8000)
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
    pub id: u32,
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
    FontFamily,
}

/// Declaration-order value for one typed text-proxy field.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextTextProxyField {
    pub id: u32,
    pub name: String,
    pub value: RichTextTextProxyScalar,
}

/// Closed runtime scalar algebra for typed text-proxy values.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextTextProxyScalar {
    Bool {
        value: bool,
    },
    Int {
        value: i64,
    },
    Milli {
        value: Milli,
    },
    Ratio {
        milli: u16,
    },
    Length {
        value: arcweft_primitives::CheckedLength,
    },
    Angle {
        milli_degrees: i32,
    },
    Duration {
        millis: u64,
    },
    ClosedEnum {
        enum_id: String,
        variant: u32,
    },
    PublicId {
        value: String,
    },
    Text {
        value: String,
    },
    Color {
        value: crate::style::RichTextColor,
    },
    FontFamily {
        value: crate::style::RichTextFontFamily,
    },
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
